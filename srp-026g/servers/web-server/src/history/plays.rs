//! The stats page's Maps and Modes boards (`/api/history/maps`,
//! `/api/history/modes`): the counted public matches of a period, as
//! [`MatchHistory::plays`] adds them up, by map and mode. A map is its
//! place ([`maps::Place`], as the rooms' vote cards have it): every
//! version and weather of it one map, each of them told apart under it.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use session::v026g::persist::history::{PlayWindow, Played, Plays, Winner};
use survarium::v026g::localization;
use survarium::v026g::maps::{self, Build, MapId, MapMode, Place};
use survarium::v026g::twist::LobbyMode;

use super::{DAY, HOUR, round};
use crate::icons::{self, IconSet};
use crate::{Lang, launcher};

const WEEK: u64 = 7 * DAY;
/// A Monday, 1970-01-05, in Unix seconds: a week starts on one (UTC).
const MONDAY: u64 = 4 * DAY;
/// How many map + mode pairs the Modes board lists, the most played first.
pub(super) const PAIRS: usize = 10;

/// What the boards add up: the matches of the last day (24 hours, by the
/// hour), the last 7 or 30 days (UTC days, today's too), or all of them
/// (the trend: the last [`Span::WEEKS`] weeks, from Mondays).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Span {
    Day,
    Week,
    Month,
    All,
}

impl Span {
    /// How many weeks the trend of all time shows.
    const WEEKS: u64 = 26;

    pub(super) fn parse(text: Option<&str>) -> Option<Self> {
        Some(match text {
            None | Some("all") => Self::All,
            Some("day") => Self::Day,
            Some("week") => Self::Week,
            Some("month") => Self::Month,
            Some(_) => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::All => "all",
        }
    }

    /// The trend's step's name and length, and how many steps it shows.
    fn steps(self) -> (&'static str, u64, u64) {
        match self {
            Self::Day => ("hour", HOUR, 24),
            Self::Week => ("day", DAY, 7),
            Self::Month => ("day", DAY, 30),
            Self::All => ("week", WEEK, Self::WEEKS),
        }
    }

    /// What it adds up at `now`: the steps end with the one `now` is in,
    /// and a period's matches are those of its steps (all time's: every
    /// match).
    pub(super) fn window(self, now: u64) -> PlayWindow {
        let (_, step, count) = self.steps();
        let current = match self {
            Self::All => now.saturating_sub(MONDAY) / WEEK * WEEK + MONDAY,
            _ => now - now % step,
        };
        let steps_from = current.saturating_sub((count - 1) * step);
        PlayWindow {
            since: if self == Self::All { 0 } else { steps_from },
            steps_from,
            step,
        }
    }
}

/// Matches added up: how many, their lengths, and how they ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Tally {
    matches: u64,
    duration_ms: u64,
    /// Won by team 1 (the first team, 0) and team 2.
    wins: [u64; 2],
    draws: u64,
    /// A team match no trusted snapshot told the end of.
    unknown: u64,
    /// Free-for-alls: no team wins one.
    solo: u64,
}

impl Tally {
    fn add(&mut self, played: &Played) {
        self.matches += played.matches;
        self.duration_ms += played.duration_ms;
        match played.winner {
            Winner::FirstTeam => self.wins[0] += played.matches,
            Winner::SecondTeam => self.wins[1] += played.matches,
            Winner::Draw => self.draws += played.matches,
            Winner::Unknown => self.unknown += played.matches,
            Winner::MostKills => self.solo += played.matches,
        }
    }

    /// Its matches, their average length, and how the team matches of them
    /// ended: each side's wins, the draws and those of no known end, as
    /// counts and as shares of them (three decimals); `sides` is `null`
    /// when all were free-for-alls.
    fn json(&self) -> Value {
        let teams = self.matches - self.solo;
        let share = |count: u64| round(count as f64 / teams as f64, 3);
        json!({
            "matches": self.matches,
            "duration_s": average_s(self.duration_ms, self.matches),
            "sides": (teams > 0).then(|| json!({
                "wins": self.wins,
                "draws": self.draws,
                "unknown": self.unknown,
                "win_rates": self.wins.map(share),
                "draw_rate": share(self.draws),
            })),
        })
    }
}

/// An average length in whole seconds.
fn average_s(duration_ms: u64, matches: u64) -> u64 {
    if matches == 0 {
        0
    } else {
        (duration_ms as f64 / matches as f64 / 1000.0).round() as u64
    }
}

/// A match's map as the boards group it: its place, or for an id the game
/// data no longer has, the id alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Spot {
    Place(Place),
    Gone(MapId),
}

impl Spot {
    fn of(map_id: MapId) -> Self {
        maps::map_mode(map_id).map_or(Self::Gone(map_id), |map| Self::Place(map.place()))
    }

    /// The version that names the place and draws it: its 0.26 one where
    /// there is one, else its first by id, of the playable ones (else of
    /// every id).
    fn face(self) -> Option<&'static MapMode> {
        let Self::Place(place) = self else {
            return None;
        };
        let of = |only_playable: bool| {
            maps::all()
                .iter()
                .filter(move |map| map.place() == place && (map.is_playable() || !only_playable))
                .min_by_key(|map| (map.build != Build::V026, map.id))
        };
        of(true).or_else(|| of(false))
    }
}

/// A map's key in the wiki's Maps page: its project's folder.
fn map_key(map: &MapMode) -> Option<&'static str> {
    map.project.split('/').next()
}

/// The rooms' picture of a map's level and weather.
fn map_icon(map: &MapMode) -> Option<String> {
    icons::url(IconSet::Map, &format!("{}_{}", map.level, map.weather))
}

/// The game's name of a weather (the rooms' cards'), in `lang`.
fn weather_name(lang: Lang, weather: u8) -> Option<&'static str> {
    let key = match weather {
        maps::weather::DAY => "st_srp_weather_day",
        maps::weather::RAIN => "st_srp_weather_rain",
        maps::weather::EVENING => "st_srp_weather_evening",
        maps::weather::NIGHT => "st_srp_weather_night",
        maps::weather::PRERAIN => "st_srp_weather_before_rain",
        _ => return None,
    };
    localization::text(lang.game(), key)
}

/// A mode as the boards name it: as the rooms do, its section of the
/// wiki's Modes page and its rules' number (the Maps page's mode).
fn mode_fields(mode: LobbyMode, lang: Lang) -> Value {
    json!({
        "mode": launcher::lobby_mode_name(lang, mode),
        "mode_url": crate::modes::url(lang, mode),
        "rules": mode.mode().rules().number(),
    })
}

/// Adds `more`'s fields to `value` (both objects).
fn with(mut value: Value, more: Value) -> Value {
    super::extend(&mut value, more);
    value
}

/// The largest first; ties in `key`'s order.
fn by_matches<K: Ord + Copy>(tallies: &BTreeMap<K, Tally>) -> Vec<(K, Tally)> {
    let mut list: Vec<(K, Tally)> = tallies.iter().map(|(key, tally)| (*key, *tally)).collect();
    list.sort_by_key(|(key, tally)| (std::cmp::Reverse(tally.matches), *key));
    list
}

/// The Maps board: each map (place) with its matches and average length,
/// the most played first; under it each mode played on it, with how its
/// team matches ended (a map's sides fare otherwise in each mode), and
/// under that each version and weather of it played in that mode.
pub(super) fn maps(plays: &Plays, span: Span, lang: Lang) -> Value {
    let mut places: BTreeMap<Spot, Tally> = BTreeMap::new();
    let mut modes: BTreeMap<(Spot, u8), Tally> = BTreeMap::new();
    let mut versions: BTreeMap<(Spot, u8, MapId), Tally> = BTreeMap::new();
    let mut all = Tally::default();
    for played in &plays.played {
        let spot = Spot::of(played.map_id);
        let mode = played.mode.number();
        places.entry(spot).or_default().add(played);
        modes.entry((spot, mode)).or_default().add(played);
        versions.entry((spot, mode, played.map_id)).or_default().add(played);
        all.add(played);
    }
    let list: Vec<Value> = by_matches(&places)
        .into_iter()
        .map(|(spot, tally)| {
            let face = spot.face();
            let of_modes: Vec<Value> = by_matches(&modes)
                .into_iter()
                .filter(|((at, _), _)| *at == spot)
                .map(|((_, mode), tally)| {
                    let lobby = LobbyMode::of_number(mode).expect("read as a lobby mode");
                    let of_versions: Vec<(MapId, Tally)> = by_matches(&versions)
                        .into_iter()
                        .filter(|((at, of, _), _)| *at == spot && *of == mode)
                        .map(|((_, _, id), tally)| (id, tally))
                        .collect();
                    // The wiki's Maps page shows the mode on its most played version.
                    let key = of_versions
                        .first()
                        .and_then(|(id, _)| maps::map_mode(*id))
                        .and_then(map_key);
                    let rows: Vec<Value> = of_versions
                        .iter()
                        .map(|(id, tally)| {
                            let map = maps::map_mode(*id);
                            with(
                                json!({
                                    "map_id": id.get(),
                                    "map": map.map(|map| launcher::map_name(lang, map)),
                                    "weather": map.and_then(|map| weather_name(lang, map.weather)),
                                    "map_key": map.and_then(map_key),
                                    "map_icon": map.and_then(map_icon),
                                }),
                                tally.json(),
                            )
                        })
                        .collect();
                    with(
                        with(mode_fields(lobby, lang), json!({ "map_key": key, "versions": rows })),
                        tally.json(),
                    )
                })
                .collect();
            json!({
                "map": face.map(|map| launcher::map_name(lang, map)),
                "map_key": face.and_then(map_key),
                "map_icon": face.and_then(map_icon),
                "matches": tally.matches,
                "duration_s": average_s(tally.duration_ms, tally.matches),
                "modes": of_modes,
            })
        })
        .collect();
    json!({ "period": span.name(), "matches": all.matches, "maps": list })
}

/// The Modes board: each mode with its matches, its share of them all and
/// its average length, the most played first (ties: the lobby's order),
/// with its matches in each step of the trend (`steps`: each step's start,
/// Unix seconds); then the [`PAIRS`] most played map + mode pairs (every
/// version of a map one).
pub(super) fn modes(plays: &Plays, span: Span, now: u64, lang: Lang) -> Value {
    let window = span.window(now);
    let (step_name, _, count) = span.steps();
    let mut modes: BTreeMap<u8, Tally> = BTreeMap::new();
    let mut pairs: BTreeMap<(Spot, u8), Tally> = BTreeMap::new();
    let mut all = Tally::default();
    for played in &plays.played {
        modes.entry(played.mode.number()).or_default().add(played);
        pairs
            .entry((Spot::of(played.map_id), played.mode.number()))
            .or_default()
            .add(played);
        all.add(played);
    }
    let mut trend: BTreeMap<u8, Vec<u64>> = BTreeMap::new();
    for step in &plays.steps {
        if step.step < count {
            trend
                .entry(step.mode.number())
                .or_insert_with(|| vec![0; count as usize])[step.step as usize] += step.matches;
        }
    }
    let share = |matches: u64| {
        if all.matches == 0 {
            0.0
        } else {
            round(matches as f64 / all.matches as f64, 3)
        }
    };
    let list: Vec<Value> = by_matches(&modes)
        .into_iter()
        .map(|(mode, tally)| {
            let lobby = LobbyMode::of_number(mode).expect("read as a lobby mode");
            with(
                mode_fields(lobby, lang),
                json!({
                    "matches": tally.matches,
                    "share": share(tally.matches),
                    "duration_s": average_s(tally.duration_ms, tally.matches),
                    "trend": trend.get(&mode).cloned().unwrap_or_else(|| vec![0; count as usize]),
                }),
            )
        })
        .collect();
    let pairs: Vec<Value> = by_matches(&pairs)
        .into_iter()
        .take(PAIRS)
        .map(|((spot, mode), tally)| {
            let lobby = LobbyMode::of_number(mode).expect("read as a lobby mode");
            let face = spot.face();
            with(
                mode_fields(lobby, lang),
                json!({
                    "map": face.map(|map| launcher::map_name(lang, map)),
                    "map_key": face.and_then(map_key),
                    "map_icon": face.and_then(map_icon),
                    "matches": tally.matches,
                    "share": share(tally.matches),
                }),
            )
        })
        .collect();
    let steps: Vec<u64> = (0..count).map(|nth| window.steps_from + nth * window.step).collect();
    json!({
        "period": span.name(),
        "matches": all.matches,
        "step": step_name,
        "steps": steps,
        "modes": list,
        "pairs": pairs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use session::v026g::persist::history::ModeStep;
    use survarium::v026g::maps::GameMode;

    const NOW: u64 = 1_793_500_000;

    fn played(map_id: MapId, mode: LobbyMode, winner: Winner, matches: u64) -> Played {
        Played {
            map_id,
            mode,
            winner,
            matches,
            duration_ms: matches * 600_000,
        }
    }

    /// Two playable versions of one place in a mode, by id, and a map of
    /// another place in that mode.
    fn two_versions(mode: GameMode) -> (MapId, MapId, MapId) {
        let of_mode: Vec<&MapMode> = maps::playable().filter(|map| map.mode == mode).collect();
        let (first, second) = of_mode
            .iter()
            .flat_map(|a| of_mode.iter().map(move |b| (a, b)))
            .find(|(a, b)| a.id < b.id && a.place() == b.place())
            .expect("a place with two ids in the mode");
        let other = of_mode
            .iter()
            .find(|map| map.place() != first.place())
            .expect("another place");
        (first.id, second.id, other.id)
    }

    #[test]
    fn a_span_ends_with_the_step_now_is_in() {
        let day = Span::Day.window(NOW);
        assert_eq!(day.step, HOUR);
        assert_eq!(day.since, day.steps_from);
        assert_eq!(day.steps_from, NOW - NOW % HOUR - 23 * HOUR);
        let month = Span::Month.window(NOW);
        assert_eq!(month.steps_from, NOW - NOW % DAY - 29 * DAY);
        let all = Span::All.window(NOW);
        assert_eq!(all.since, 0, "all time is every match");
        assert_eq!((all.steps_from - MONDAY) % WEEK, 0, "weeks start on a Monday");
        assert!(NOW - all.steps_from < Span::WEEKS * WEEK && NOW - all.steps_from >= (Span::WEEKS - 1) * WEEK);
        assert_eq!(Span::parse(None), Some(Span::All));
        assert_eq!(Span::parse(Some("week")), Some(Span::Week));
        assert_eq!(Span::parse(Some("hour")), None);
    }

    /// Every version of a map is one map; its modes apart, each with its
    /// sides' wins and draws as counts and shares, and its versions under
    /// it; a free-for-all has no sides.
    #[test]
    fn a_maps_versions_are_one_map_and_its_sides_are_by_mode() {
        let batteries = LobbyMode::Plain(GameMode::Batteries);
        let (first, second, other) = two_versions(GameMode::Batteries);
        let free = maps::playable().find(|map| map.mode == GameMode::FreeForAll).unwrap();
        let free_place = Spot::of(free.id);
        let plays = Plays {
            played: vec![
                played(first, batteries, Winner::FirstTeam, 3),
                played(first, batteries, Winner::SecondTeam, 1),
                played(second, batteries, Winner::FirstTeam, 2),
                played(second, batteries, Winner::Draw, 2),
                played(other, batteries, Winner::SecondTeam, 1),
                played(free.id, LobbyMode::Racket, Winner::MostKills, 5),
            ],
            steps: Vec::new(),
        };
        let answer = maps(&plays, Span::All, Lang::En);
        assert_eq!(answer["matches"], 14);
        assert_eq!(answer["period"], "all");
        let list = answer["maps"].as_array().unwrap();
        let place_of = |id: MapId| -> &Value {
            let face = Spot::of(id).face().unwrap();
            list.iter()
                .find(|map| map["map_key"] == json!(map_key(face)) && map["map"] == json!(launcher::map_name(Lang::En, face)))
                .unwrap()
        };
        let versions = place_of(first);
        assert_eq!(versions["matches"], if free_place == Spot::of(first) { 13 } else { 8 });
        let mode = versions["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["rules"] == json!(GameMode::Batteries.rules().number()))
            .unwrap();
        assert_eq!(mode["matches"], 8);
        assert_eq!(mode["sides"]["wins"], json!([5, 1]));
        assert_eq!(mode["sides"]["draws"], 2);
        assert_eq!(mode["sides"]["win_rates"], json!([0.625, 0.125]));
        assert_eq!(mode["sides"]["draw_rate"], 0.25);
        assert_eq!(mode["duration_s"], 600);
        let rows = mode["versions"].as_array().unwrap();
        assert_eq!(rows.len(), 2, "each version apart under it");
        assert_eq!(rows[0]["matches"], 4);
        assert_eq!(rows[0]["map_id"], first.get(), "the most played first, ties by id");
        assert_eq!(rows[1]["sides"]["wins"], json!([2, 0]));
        // Each its weather as the rooms' cards name it (srp's own words, which the tests' game
        // data lacks) and its picture.
        assert!(rows.iter().all(|row| row.get("weather").is_some() && row["map_icon"].is_string()));
        assert_ne!(rows[0]["map_icon"], rows[1]["map_icon"], "two weathers");
        // The free-for-all: no side wins one.
        let racket = place_of(free.id)["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["matches"] == 5)
            .unwrap();
        assert!(racket["sides"].is_null());
        // The most played map first.
        let counts: Vec<u64> = list.iter().map(|map| map["matches"].as_u64().unwrap()).collect();
        assert!(counts.windows(2).all(|pair| pair[0] >= pair[1]), "{counts:?}");
        assert_eq!(counts.iter().sum::<u64>(), 14);
    }

    /// A map the game data no longer has is a map of its own, unnamed.
    #[test]
    fn a_map_gone_from_the_game_data_is_kept_apart() {
        let plays = Plays {
            played: vec![played(MapId::new(60_000), LobbyMode::Plain(GameMode::TeamDeathmatch), Winner::Unknown, 2)],
            steps: Vec::new(),
        };
        let answer = maps(&plays, Span::Week, Lang::En);
        let map = &answer["maps"][0];
        assert!(map["map"].is_null() && map["map_icon"].is_null());
        assert_eq!(map["modes"][0]["sides"]["unknown"], 2);
        assert_eq!(map["modes"][0]["sides"]["win_rates"], json!([0.0, 0.0]));
    }

    /// Each mode's matches and share of them all, the most played first,
    /// with its trend by step; the map + mode pairs, every version one map.
    #[test]
    fn modes_have_their_shares_trend_and_most_played_maps() {
        let tdm = LobbyMode::Plain(GameMode::TeamDeathmatch);
        let blackout = LobbyMode::Plain(GameMode::Blackout);
        let (first, second, other) = two_versions(GameMode::TeamDeathmatch);
        let blackout_map = maps::playable().find(|map| map.mode == GameMode::Blackout).unwrap().id;
        let plays = Plays {
            played: vec![
                played(first, tdm, Winner::FirstTeam, 3),
                played(second, tdm, Winner::SecondTeam, 2),
                played(other, tdm, Winner::Draw, 1),
                played(blackout_map, blackout, Winner::FirstTeam, 2),
            ],
            steps: vec![
                ModeStep { mode: tdm, step: 0, matches: 1 },
                ModeStep { mode: tdm, step: 6, matches: 5 },
                ModeStep { mode: blackout, step: 3, matches: 2 },
                // Past the last step (none should be): left out.
                ModeStep { mode: blackout, step: 7, matches: 9 },
            ],
        };
        let answer = modes(&plays, Span::Week, NOW, Lang::En);
        assert_eq!(answer["matches"], 8);
        assert_eq!(answer["step"], "day");
        let steps = answer["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 7);
        assert_eq!(steps[6], NOW - NOW % DAY, "today last");
        let list = answer["modes"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["mode"], json!(launcher::lobby_mode_name(Lang::En, tdm)), "the most played first");
        assert_eq!(list[0]["matches"], 6);
        assert_eq!(list[0]["share"], 0.75);
        assert_eq!(list[1]["share"], 0.25);
        assert_eq!(list[0]["trend"], json!([1, 0, 0, 0, 0, 0, 5]));
        assert_eq!(list[1]["trend"], json!([0, 0, 0, 2, 0, 0, 0]));
        let pairs = answer["pairs"].as_array().unwrap();
        assert_eq!(pairs[0]["matches"], 5, "both versions of a map one pair");
        assert_eq!(pairs[0]["share"], 0.625);
        assert_eq!(pairs.len(), 3);
        assert!(pairs.iter().all(|pair| pair["map"].is_string() && pair["mode"].is_string()));
        // Nothing played: no shares to divide.
        let empty = modes(&Plays::default(), Span::Day, NOW, Lang::En);
        assert_eq!(empty["matches"], 0);
        assert_eq!(empty["steps"].as_array().unwrap().len(), 24);
        assert!(empty["modes"].as_array().unwrap().is_empty());
    }
}
