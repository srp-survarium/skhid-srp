//! The stats page's Maps and Modes boards (`/api/history/maps`,
//! `/api/history/modes`): the counted public matches of a period, as
//! [`MatchHistory::plays`] adds them up, by map and mode. A map is its
//! level, as the rooms' create window lists maps: every ported version its
//! own map, its weathers told apart under it.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use session::v026g::persist::history::{KeptMode, PlayWindow, Played, Plays, Winner};
use survarium::v026g::maps::{self, Build, GameMode, MapId, MapMode};
use survarium::v026g::roulette::Band;
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

/// The maps the boards read: the game data's ([`maps::all`]), or a test's
/// (`fixtures::table_of`), every id once, by id.
pub(super) struct Atlas<'a> {
    all: &'a [MapMode],
}

impl Atlas<'static> {
    pub(super) fn shared() -> Self {
        Self { all: maps::all() }
    }
}

impl<'a> Atlas<'a> {
    fn map(&self, id: MapId) -> Option<&'a MapMode> {
        self.all
            .binary_search_by_key(&id, |map| map.id)
            .ok()
            .map(|index| &self.all[index])
    }
}

/// A match's map as the boards tell maps apart: its level
/// (`match_making_params.map_id`, as the rooms' create window has it: every
/// ported version its own, "Rudnya (v0.69)" apart from Rudnya; the owner,
/// 2026-11-26: "those are separate maps with separate balancing"), its
/// weathers and modes under it; or, for an id the game data no longer has,
/// the id alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Spot {
    Level(u8),
    Gone(MapId),
}

/// The order of the rooms' create window, for the Maps board: its maps
/// (levels) where they come in it, then the others played by build and
/// level, then those gone from the game data.
struct Order<'a> {
    atlas: &'a Atlas<'a>,
    /// The window's ids, in its order.
    rooms: &'a [MapId],
}

impl<'a> Order<'a> {
    fn spot(&self, id: MapId) -> Spot {
        self.atlas.map(id).map_or(Spot::Gone(id), |map| Spot::Level(map.level))
    }

    /// Where `spot` stands among the maps.
    fn rank(&self, spot: Spot) -> (u8, usize, Option<Build>, u64) {
        match spot {
            Spot::Level(level) => match self
                .rooms
                .iter()
                .position(|&id| self.spot(id) == spot)
            {
                Some(at) => (0, at, None, 0),
                None => (1, 0, self.face(spot).map(|map| map.build), u64::from(level)),
            },
            Spot::Gone(id) => (2, 0, None, u64::from(id.get())),
        }
    }

    /// The id that names a map and draws it: its first in the window, else
    /// its first playable one (day first), else any.
    fn face(&self, spot: Spot) -> Option<&'a MapMode> {
        let Spot::Level(level) = spot else {
            return None;
        };
        self.rooms
            .iter()
            .filter_map(|&id| self.atlas.map(id))
            .find(|map| map.level == level)
            .or_else(|| {
                self.atlas
                    .all
                    .iter()
                    .filter(|map| map.level == level)
                    .min_by_key(|map| (!map.is_playable(), maps::weather::list_place(map.weather), map.id))
            })
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

/// A weather as the rooms' cards name it, in the site's own words (the
/// game's texts, `st_srp_weather_*`); `None` for one the rooms do not name.
pub(super) fn weather_name(lang: Lang, weather: u8) -> Option<String> {
    Some(match weather {
        maps::weather::DAY => lang.tc("weather", "day"),
        maps::weather::RAIN => lang.tc("weather", "rain"),
        maps::weather::EVENING => lang.tc("weather", "evening"),
        maps::weather::NIGHT => lang.tc("weather", "night"),
        maps::weather::PRERAIN => lang.tc("weather", "before rain"),
        _ => return None,
    })
}

/// A mode as the boards name it: as the rooms do, its section of the
/// wiki's Modes page and its rules' number (the Maps page's mode); a mode
/// srp no longer knows by its number, with neither.
fn mode_fields(mode: KeptMode, lang: Lang) -> Value {
    match mode {
        KeptMode::Known(mode) => json!({
            "mode": launcher::lobby_mode_name(lang, mode),
            "mode_url": crate::modes::url(lang, mode),
            "rules": mode.mode().rules().number(),
        }),
        KeptMode::Unknown(number) => json!({
            "mode": launcher::unknown_mode_name(lang, number),
            "mode_url": null,
            "rules": null,
        }),
    }
}

/// Whether a mode is a free-for-all, plain or with a twist.
fn is_free_for_all(mode: KeptMode) -> bool {
    matches!(mode, KeptMode::Known(lobby) if lobby.mode() == GameMode::FreeForAll)
}

/// A match's mode as the boards count it: its own, but every free-for-all
/// (plain, the Racket, the Saloon, the Roulette) one, the free-for-all (the
/// owner, 2026-11-26: all of them one Free for all, each version under it).
/// The match lists still name a match by its own.
fn counted_as(mode: KeptMode) -> KeptMode {
    if is_free_for_all(mode) {
        KeptMode::Known(LobbyMode::Plain(GameMode::FreeForAll))
    } else {
        mode
    }
}

/// A free-for-all's version as the boards tell them apart under it: its own
/// mode, the Roulette's bands one (they are one name, "FFA: Roulette").
fn version_of(mode: KeptMode) -> KeptMode {
    match mode {
        KeptMode::Known(LobbyMode::Roulette(_)) => KeptMode::Known(LobbyMode::Roulette(Band::Low)),
        other => other,
    }
}

/// The modes in the order the rooms' create window lists them
/// (`RoomsCreate.as`, `MODE_ORDER`; a test holds the two together): the
/// protective device's three kinds together, then Team Deathmatch, Research,
/// Freeze Tag, the free-for-all and Artifact Hunt (the owner, 2026-11-26:
/// "Blackout next to the other Protective Device").
const MODE_ORDER: [GameMode; 8] = [
    GameMode::Batteries,
    GameMode::TimedBatteries,
    GameMode::Blackout,
    GameMode::TeamDeathmatch,
    GameMode::Research,
    GameMode::FreezeTag,
    GameMode::FreeForAll,
    GameMode::ArtefactHunt,
];

/// Where a mode stands in the boards' lists: its mode's place in
/// [`MODE_ORDER`], a free-for-all's twists right after it (by their lobby
/// numbers); a mode srp no longer knows last.
fn mode_rank(mode: KeptMode) -> (usize, u8) {
    match mode {
        KeptMode::Known(lobby) => (
            MODE_ORDER
                .iter()
                .position(|&each| each == lobby.mode())
                .unwrap_or(MODE_ORDER.len()),
            match lobby {
                LobbyMode::Plain(_) => 0,
                twist => twist.number(),
            },
        ),
        KeptMode::Unknown(number) => (MODE_ORDER.len() + 1, number),
    }
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

/// A map's name and pictures' fields: `null` for one gone from the game data.
fn map_fields(face: Option<&MapMode>, lang: Lang) -> Value {
    json!({
        "map": face.map(|map| launcher::map_name(lang, map)),
        "map_key": face.and_then(map_key),
        "map_icon": face.and_then(map_icon),
    })
}

/// Whether a match had sides: not a free-for-all (nor any of its twists).
fn team_match(mode: KeptMode, winner: Winner) -> bool {
    !is_free_for_all(mode) && winner != Winner::MostKills
}

/// The weathers of a map's matches in `ids`' tallies (each its map id), day,
/// evening, rain, as the window lists them.
fn weather_rows(of: &BTreeMap<MapId, Tally>, atlas: &Atlas, lang: Lang) -> Vec<Value> {
    let mut rows: Vec<(MapId, Tally)> = of.iter().map(|(id, tally)| (*id, *tally)).collect();
    rows.sort_by_key(|(id, _)| (atlas.map(*id).map(|map| maps::weather::list_place(map.weather)), *id));
    rows.iter()
        .map(|(id, tally)| {
            let map = atlas.map(*id);
            with(
                json!({
                    "map_id": id.get(),
                    "weather": map.and_then(|map| weather_name(lang, map.weather)),
                    "map_key": map.and_then(map_key),
                    "map_icon": map.and_then(map_icon),
                }),
                tally.json(),
            )
        })
        .collect()
}

/// A mode's tallies by key, in the create window's order ([`mode_rank`]).
fn in_mode_order(tallies: &BTreeMap<u8, (KeptMode, Tally)>) -> Vec<(KeptMode, Tally)> {
    let mut list: Vec<(KeptMode, Tally)> = tallies.values().copied().collect();
    list.sort_by_key(|(mode, _)| mode_rank(*mode));
    list
}

/// The Maps board: each map played (every ported version its own, by
/// level), in the order of the rooms' create window (`rooms`: its ids in its
/// order; the owner, 2026-11-26), with its team matches (the
/// free-for-alls left out: no side wins one) added up: their count, average
/// length and how its sides fared (the owner: "a map just shows general team
/// 1 vs team 2"). Under it, for more, each mode played on it in the create
/// window's order ([`MODE_ORDER`]; every free-for-all one, its versions
/// under it), with its own, and under that each weather of it (each its map
/// id).
pub(super) fn maps(plays: &Plays, span: Span, lang: Lang, atlas: &Atlas, rooms: &[MapId]) -> Value {
    let order = Order { atlas, rooms };
    let mut spots: BTreeMap<Spot, Tally> = BTreeMap::new();
    // By map and mode as counted; by map, mode and version; each one's weathers.
    let mut modes: BTreeMap<Spot, BTreeMap<u8, (KeptMode, Tally)>> = BTreeMap::new();
    let mut versions: BTreeMap<(Spot, u8), BTreeMap<u8, (KeptMode, Tally)>> = BTreeMap::new();
    let mut mode_weathers: BTreeMap<(Spot, u8), BTreeMap<MapId, Tally>> = BTreeMap::new();
    let mut version_weathers: BTreeMap<(Spot, u8), BTreeMap<MapId, Tally>> = BTreeMap::new();
    let mut all = Tally::default();
    for played in &plays.played {
        let spot = order.spot(played.map_id);
        let counted = counted_as(played.mode);
        let version = version_of(played.mode);
        let of_map = spots.entry(spot).or_default();
        if team_match(counted, played.winner) {
            of_map.add(played);
        }
        let mode = counted.number();
        modes.entry(spot).or_default().entry(mode).or_insert((counted, Tally::default())).1.add(played);
        versions
            .entry((spot, mode))
            .or_default()
            .entry(version.number())
            .or_insert((version, Tally::default()))
            .1
            .add(played);
        mode_weathers.entry((spot, mode)).or_default().entry(played.map_id).or_default().add(played);
        version_weathers
            .entry((spot, version.number()))
            .or_default()
            .entry(played.map_id)
            .or_default()
            .add(played);
        all.add(played);
    }
    let mut list: Vec<(Spot, Tally)> = spots.into_iter().collect();
    list.sort_by_key(|(spot, _)| order.rank(*spot));
    let list: Vec<Value> = list
        .into_iter()
        .map(|(spot, tally)| {
            let of_modes: Vec<Value> = in_mode_order(&modes[&spot])
                .into_iter()
                .map(|(mode, tally)| {
                    let weathers = &mode_weathers[&(spot, mode.number())];
                    // The wiki's Maps page shows the mode on the map's first weather of it.
                    let key = weather_rows(weathers, atlas, lang)
                        .first()
                        .and_then(|row| row["map_key"].as_str().map(str::to_owned));
                    let mut value = with(
                        with(
                            mode_fields(mode, lang),
                            json!({ "map_key": key, "weathers": weather_rows(weathers, atlas, lang) }),
                        ),
                        tally.json(),
                    );
                    if is_free_for_all(mode) {
                        let rows: Vec<Value> = in_mode_order(&versions[&(spot, mode.number())])
                            .into_iter()
                            .map(|(version, tally)| {
                                let weathers = &version_weathers[&(spot, version.number())];
                                with(
                                    with(mode_fields(version, lang), json!({ "weathers": weather_rows(weathers, atlas, lang) })),
                                    tally.json(),
                                )
                            })
                            .collect();
                        super::extend(&mut value, json!({ "versions": rows }));
                    }
                    value
                })
                .collect();
            with(with(map_fields(order.face(spot), lang), tally.json()), json!({ "modes": of_modes }))
        })
        .collect();
    json!({ "period": span.name(), "matches": all.matches, "maps": list })
}

/// The Modes board: each mode with its matches, its share of them all, its
/// popularity's `rank` (1: the most played) and its average length, in the
/// create window's order ([`MODE_ORDER`]), with its matches in each step of the
/// trend (`steps`: each step's start, Unix seconds); every free-for-all one
/// mode, its versions under it (`versions`, each with its own); then the
/// [`PAIRS`] most played map + mode pairs (every ported version its own map,
/// its weathers one; the free-for-alls one).
pub(super) fn modes(plays: &Plays, span: Span, now: u64, lang: Lang, atlas: &Atlas) -> Value {
    let order = Order { atlas, rooms: &[] };
    let window = span.window(now);
    let (step_name, _, count) = span.steps();
    let mut kept: BTreeMap<u8, KeptMode> = BTreeMap::new();
    let mut modes: BTreeMap<u8, Tally> = BTreeMap::new();
    // The free-for-all's versions, by their own numbers.
    let mut versions: BTreeMap<u8, (KeptMode, Tally)> = BTreeMap::new();
    let mut pairs: BTreeMap<(Spot, u8), Tally> = BTreeMap::new();
    let mut all = Tally::default();
    for played in &plays.played {
        let counted = counted_as(played.mode);
        kept.insert(counted.number(), counted);
        modes.entry(counted.number()).or_default().add(played);
        if is_free_for_all(counted) {
            let version = version_of(played.mode);
            versions.entry(version.number()).or_insert((version, Tally::default())).1.add(played);
        }
        pairs
            .entry((order.spot(played.map_id), counted.number()))
            .or_default()
            .add(played);
        all.add(played);
    }
    let mut trend: BTreeMap<u8, Vec<u64>> = BTreeMap::new();
    let mut version_trend: BTreeMap<u8, Vec<u64>> = BTreeMap::new();
    for step in &plays.steps {
        if step.step < count {
            let at = step.step as usize;
            trend.entry(counted_as(step.mode).number()).or_insert_with(|| vec![0; count as usize])[at] += step.matches;
            if is_free_for_all(step.mode) {
                version_trend
                    .entry(version_of(step.mode).number())
                    .or_insert_with(|| vec![0; count as usize])[at] += step.matches;
            }
        }
    }
    let share = |matches: u64| {
        if all.matches == 0 {
            0.0
        } else {
            round(matches as f64 / all.matches as f64, 3)
        }
    };
    let numbers = |mode: u8, tally: &Tally, trend: &BTreeMap<u8, Vec<u64>>| {
        json!({
            "matches": tally.matches,
            "share": share(tally.matches),
            "duration_s": average_s(tally.duration_ms, tally.matches),
            "trend": trend.get(&mode).cloned().unwrap_or_else(|| vec![0; count as usize]),
        })
    };
    // Ranked by matches (ties: the order), listed in the order.
    let mut by_rank: Vec<(u8, Tally)> = modes.iter().map(|(mode, tally)| (*mode, *tally)).collect();
    by_rank.sort_by_key(|(mode, tally)| (std::cmp::Reverse(tally.matches), mode_rank(kept[mode])));
    let mut in_order: Vec<(u8, Tally)> = by_rank.clone();
    in_order.sort_by_key(|(mode, _)| mode_rank(kept[mode]));
    let list: Vec<Value> = in_order
        .into_iter()
        .map(|(mode, tally)| {
            let rank = by_rank.iter().position(|(each, _)| *each == mode).map_or(0, |at| at + 1);
            let mut value = with(
                with(mode_fields(kept[&mode], lang), json!({ "rank": rank })),
                numbers(mode, &tally, &trend),
            );
            if is_free_for_all(kept[&mode]) {
                let rows: Vec<Value> = in_mode_order(&versions)
                    .into_iter()
                    .map(|(version, tally)| {
                        with(mode_fields(version, lang), numbers(version.number(), &tally, &version_trend))
                    })
                    .collect();
                super::extend(&mut value, json!({ "versions": rows }));
            }
            value
        })
        .collect();
    let pairs: Vec<Value> = by_matches(&pairs)
        .into_iter()
        .take(PAIRS)
        .map(|((spot, mode), tally)| {
            with(
                with(mode_fields(kept[&mode], lang), map_fields(order.face(spot), lang)),
                json!({ "matches": tally.matches, "share": share(tally.matches) }),
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
    use survarium::v026g::maps::{GameMode, fixtures};
    use survarium::v026g::twist::LobbyMode;

    const NOW: u64 = 1_793_500_000;

    fn played(map_id: MapId, mode: LobbyMode, winner: Winner, matches: u64) -> Played {
        Played {
            map_id,
            mode: KeptMode::Known(mode),
            winner,
            matches,
            duration_ms: matches * 600_000,
        }
    }

    /// Two playable weathers of one map in a mode, by id, and a map of
    /// another level in that mode.
    fn two_weathers(mode: GameMode) -> (MapId, MapId, MapId) {
        let of_mode: Vec<&MapMode> = maps::playable().filter(|map| map.mode == mode).collect();
        let (first, second) = of_mode
            .iter()
            .flat_map(|a| of_mode.iter().map(move |b| (a, b)))
            .find(|(a, b)| a.id < b.id && a.level == b.level)
            .expect("a map with two ids in the mode");
        let other = of_mode
            .iter()
            .find(|map| map.level != first.level)
            .expect("another map");
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

    /// A ported version of a map is a map of its own (the owner, 2026-11-26:
    /// "separate maps with separate balancing"), its weathers under it; the
    /// maps come in the rooms' create window's order, not by their matches;
    /// each mode apart, with its sides' wins and draws as counts and shares;
    /// a free-for-all has no sides.
    #[test]
    fn ported_versions_are_maps_of_their_own_in_the_create_windows_order() {
        // Rudnya (v0.69), day and rain, laid over the test data's stock maps.
        let port = |id, weather, project| fixtures::AddedMap {
            id: MapId::new(id),
            config: match id {
                204 => "v069d_level_01_day_4.options",
                _ => "v069d_level_01_rain_4.options",
            },
            level: 14,
            weather,
            project,
            ..fixtures::AddedMap::level_12_tdm()
        };
        let table = fixtures::table_of(&fixtures::with_maps(&[
            port(204, maps::weather::DAY, "v069d_level_01/day_4"),
            port(205, maps::weather::RAIN, "v069d_level_01/rain_4"),
        ]))
        .unwrap();
        let atlas = Atlas { all: &table };
        let of = |id: u16| atlas.map(MapId::new(id)).unwrap();
        let (port_day, port_rain) = (of(204), of(205));
        assert_eq!((port_day.build, port_day.level), (Build::V069, 14));
        let tdm = LobbyMode::Plain(GameMode::TeamDeathmatch);
        // Stock Rudnya's Team Deathmatch, day and rain; a free-for-all on Mamayev Kurgan.
        let rudnya: Vec<&MapMode> = table.iter().filter(|map| map.level == 1 && map.mode == GameMode::TeamDeathmatch).collect();
        let (day, rain) = (
            rudnya.iter().find(|map| map.weather == maps::weather::DAY).unwrap(),
            rudnya.iter().find(|map| map.weather == maps::weather::RAIN).unwrap(),
        );
        let free = table.iter().find(|map| map.mode == GameMode::FreeForAll && map.level == 6).unwrap();
        // The window: the pool in its order, as the lobby sorts it.
        let pool = vec![port_rain, port_day, free, *rain, *day];
        let rooms: Vec<MapId> = maps::in_room_order(pool, |map| (map.build, map.level, map.weather))
            .into_iter()
            .map(|map| map.id)
            .collect();
        let plays = Plays {
            played: vec![
                played(day.id, tdm, Winner::FirstTeam, 3),
                played(day.id, tdm, Winner::SecondTeam, 1),
                played(rain.id, tdm, Winner::FirstTeam, 2),
                played(rain.id, tdm, Winner::Draw, 2),
                played(port_day.id, tdm, Winner::SecondTeam, 1),
                played(port_rain.id, tdm, Winner::FirstTeam, 1),
                played(free.id, LobbyMode::Racket, Winner::MostKills, 5),
            ],
            steps: Vec::new(),
        };
        let answer = maps(&plays, Span::All, Lang::En, &atlas, &rooms);
        assert_eq!(answer["matches"], 15);
        assert_eq!(answer["period"], "all");
        let list = answer["maps"].as_array().unwrap();
        // 0.26's maps first, each where its first id comes in the pool (Mamayev Kurgan's
        // free-for-all before Rudnya), then 0.69's: not the most played first.
        let names: Vec<&Value> = list.iter().map(|map| &map["map"]).collect();
        assert_eq!(
            names,
            [free, *day, port_day].map(|map| json!(launcher::map_name(Lang::En, map))).iter().collect::<Vec<_>>()
        );
        // A map's row: its team matches (Mamayev Kurgan's Racket has no sides: none).
        assert_eq!(list.iter().map(|map| map["matches"].as_u64().unwrap()).collect::<Vec<_>>(), [0, 8, 2]);
        assert!(list[0]["sides"].is_null());
        assert_eq!(list[1]["sides"]["wins"], json!([5, 1]));
        assert_eq!(list[1]["sides"]["draws"], 2);
        assert_eq!(list[1]["duration_s"], 600);
        // Stock Rudnya: its two weathers one map, each apart under its mode, day first.
        let mode = &list[1]["modes"][0];
        assert_eq!(mode["mode"], "Team Deathmatch");
        assert_eq!(mode["matches"], 8);
        assert_eq!(mode["sides"]["wins"], json!([5, 1]));
        assert_eq!(mode["sides"]["draws"], 2);
        assert_eq!(mode["sides"]["win_rates"], json!([0.625, 0.125]));
        assert_eq!(mode["sides"]["draw_rate"], 0.25);
        assert_eq!(mode["duration_s"], 600);
        let rows = mode["weathers"].as_array().unwrap();
        assert_eq!(rows.iter().map(|row| row["map_id"].as_u64().unwrap()).collect::<Vec<_>>(), [u64::from(day.id.get()), u64::from(rain.id.get())]);
        assert_eq!(rows.iter().map(|row| row["weather"].as_str().unwrap()).collect::<Vec<_>>(), ["day", "rain"], "the rooms' words, from the code");
        assert_eq!(rows[1]["sides"]["wins"], json!([2, 0]));
        assert!(rows.iter().all(|row| row["map_icon"].is_string()));
        // Rudnya (v0.69): its own sides.
        let ported = &list[2]["modes"][0];
        assert_eq!(ported["sides"]["wins"], json!([1, 1]));
        assert_eq!(ported["weathers"].as_array().unwrap().len(), 2);
        assert_eq!(list[2]["map_key"], "v069d_level_01");
        // The free-for-all: under its map, its matches with no sides, the Racket one version of it.
        assert!(list[0]["modes"][0]["sides"].is_null());
        assert_eq!(list[0]["modes"][0]["matches"], 5);
        assert_eq!(list[0]["modes"][0]["mode"], "Free for all");
        assert_eq!(list[0]["modes"][0]["versions"][0]["mode"], "FFA: Racket");
    }

    /// Maps the window does not list come after it, by build and level; a
    /// map the game data no longer has last, unnamed; a mode srp no longer
    /// knows by its number.
    #[test]
    fn maps_off_the_window_and_gone_come_after_it() {
        let (first, _, other) = two_weathers(GameMode::TeamDeathmatch);
        let tdm = LobbyMode::Plain(GameMode::TeamDeathmatch);
        let plays = Plays {
            played: vec![
                played(MapId::new(60_000), tdm, Winner::Unknown, 2),
                played(first, tdm, Winner::FirstTeam, 1),
                Played { mode: KeptMode::Unknown(200), ..played(other, tdm, Winner::FirstTeam, 1) },
            ],
            steps: Vec::new(),
        };
        let atlas = Atlas::shared();
        let answer = maps(&plays, Span::Week, Lang::En, &atlas, &[other]);
        let list = answer["maps"].as_array().unwrap();
        assert_eq!(list[0]["map"], json!(launcher::map_name(Lang::En, atlas.map(other).unwrap())), "the window's first");
        assert_eq!(list[0]["modes"][0]["mode"], "Mode 200");
        assert!(list[0]["modes"][0]["mode_url"].is_null());
        assert_eq!(list[1]["map"], json!(launcher::map_name(Lang::En, atlas.map(first).unwrap())));
        let gone = &list[2];
        assert!(gone["map"].is_null() && gone["map_icon"].is_null());
        assert_eq!(gone["modes"][0]["sides"]["unknown"], 2);
        assert_eq!(gone["modes"][0]["sides"]["win_rates"], json!([0.0, 0.0]));
        assert_eq!(gone["modes"][0]["weathers"][0]["weather"], Value::Null);
    }

    /// Each mode's matches and share of them all, the most played first,
    /// with its trend by step; the map + mode pairs, a map's weathers one.
    #[test]
    fn modes_have_their_shares_trend_and_most_played_maps() {
        let tdm = LobbyMode::Plain(GameMode::TeamDeathmatch);
        let blackout = LobbyMode::Plain(GameMode::Blackout);
        let (first, second, other) = two_weathers(GameMode::TeamDeathmatch);
        let blackout_map = maps::playable().find(|map| map.mode == GameMode::Blackout).unwrap().id;
        let plays = Plays {
            played: vec![
                played(first, tdm, Winner::FirstTeam, 3),
                played(second, tdm, Winner::SecondTeam, 2),
                played(other, tdm, Winner::Draw, 1),
                played(blackout_map, blackout, Winner::FirstTeam, 2),
            ],
            steps: vec![
                ModeStep { mode: KeptMode::Known(tdm), step: 0, matches: 1 },
                ModeStep { mode: KeptMode::Known(tdm), step: 6, matches: 5 },
                ModeStep { mode: KeptMode::Known(blackout), step: 3, matches: 2 },
                // Past the last step (none should be): left out.
                ModeStep { mode: KeptMode::Known(blackout), step: 7, matches: 9 },
            ],
        };
        let answer = modes(&plays, Span::Week, NOW, Lang::En, &Atlas::shared());
        assert_eq!(answer["matches"], 8);
        assert_eq!(answer["step"], "day");
        let steps = answer["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 7);
        assert_eq!(steps[6], NOW - NOW % DAY, "today last");
        let list = answer["modes"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        // In the create window's order, Blackout before Team Deathmatch; ranked by matches.
        assert_eq!(list[0]["mode"], "Protective device: blackout", "named by the code");
        assert_eq!(list[1]["mode"], "Team Deathmatch");
        assert_eq!((list[0]["rank"].as_u64(), list[1]["rank"].as_u64()), (Some(2), Some(1)));
        assert_eq!(list[1]["matches"], 6);
        assert_eq!(list[1]["share"], 0.75);
        assert_eq!(list[0]["share"], 0.25);
        assert_eq!(list[1]["trend"], json!([1, 0, 0, 0, 0, 0, 5]));
        assert_eq!(list[0]["trend"], json!([0, 0, 0, 2, 0, 0, 0]));
        let pairs = answer["pairs"].as_array().unwrap();
        assert_eq!(pairs[0]["matches"], 5, "a map's two weathers one pair");
        assert_eq!(pairs[0]["share"], 0.625);
        assert_eq!(pairs.len(), 3);
        assert!(pairs.iter().all(|pair| pair["map"].is_string() && pair["mode"].is_string()));
        // Nothing played: no shares to divide.
        let empty = modes(&Plays::default(), Span::Day, NOW, Lang::En, &Atlas::shared());
        assert_eq!(empty["matches"], 0);
        assert_eq!(empty["steps"].as_array().unwrap().len(), 24);
        assert!(empty["modes"].as_array().unwrap().is_empty());
    }

    /// Every free-for-all (plain, the Racket, the Roulette's bands) is one
    /// Free for all on both boards, its trend and its map + mode pair too, with
    /// each version under it (the Roulette's bands one) with its own: its
    /// matches, share and trend, and on a map its weathers.
    #[test]
    fn every_free_for_all_is_one_with_its_versions_under_it() {
        let free = maps::playable().find(|map| map.mode == GameMode::FreeForAll).unwrap();
        let tdm = maps::playable().find(|map| map.mode == GameMode::TeamDeathmatch && map.level == free.level);
        let plain = LobbyMode::Plain(GameMode::FreeForAll);
        let mut played_list = vec![
            played(free.id, plain, Winner::MostKills, 2),
            played(free.id, LobbyMode::Roulette(Band::Low), Winner::MostKills, 1),
            played(free.id, LobbyMode::Roulette(Band::High), Winner::MostKills, 3),
            played(free.id, LobbyMode::Racket, Winner::MostKills, 1),
        ];
        if let Some(tdm) = tdm {
            played_list.push(played(tdm.id, LobbyMode::Plain(GameMode::TeamDeathmatch), Winner::FirstTeam, 1));
        }
        let plays = Plays {
            played: played_list,
            steps: vec![
                ModeStep { mode: KeptMode::Known(plain), step: 6, matches: 2 },
                ModeStep { mode: KeptMode::Known(LobbyMode::Roulette(Band::Mid)), step: 6, matches: 4 },
                ModeStep { mode: KeptMode::Known(LobbyMode::Racket), step: 5, matches: 1 },
            ],
        };
        let atlas = Atlas::shared();
        let board = modes(&plays, Span::Week, NOW, Lang::En, &atlas);
        let ffa = board["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["mode"] == "Free for all")
            .unwrap();
        assert!(board["modes"].as_array().unwrap().iter().all(|mode| !mode["mode"].as_str().unwrap().starts_with("FFA:")), "no version a mode of its own");
        assert_eq!(ffa["matches"], 7, "the plain, the Roulette's and the Racket's");
        assert_eq!(ffa["trend"][6], 6);
        assert_eq!(ffa["trend"][5], 1);
        assert_eq!(ffa["mode_url"], json!(crate::modes::url(Lang::En, plain)));
        let versions: Vec<(&str, u64, u64)> = ffa["versions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| (row["mode"].as_str().unwrap(), row["matches"].as_u64().unwrap(), row["trend"][6].as_u64().unwrap()))
            .collect();
        assert_eq!(versions, [("Free for all", 2, 2), ("FFA: Racket", 1, 0), ("FFA: Roulette", 4, 4)], "each version, the Roulette's bands one");
        assert_eq!(ffa["versions"][1]["trend"][5], 1);
        let shares: f64 = ffa["versions"].as_array().unwrap().iter().map(|row| row["share"].as_f64().unwrap()).sum();
        assert!((shares - ffa["share"].as_f64().unwrap()).abs() < 0.002);
        let pair = board["pairs"].as_array().unwrap().iter().find(|pair| pair["mode"] == "Free for all").unwrap();
        assert_eq!(pair["matches"], 7);
        // On its map: one Free for all, its versions under it, each with its weathers.
        let board = maps(&plays, Span::Week, Lang::En, &atlas, &[]);
        let map = board["maps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|map| map["modes"].as_array().unwrap().iter().any(|mode| mode["mode"] == "Free for all"))
            .unwrap();
        assert_eq!(map["matches"], if tdm.is_some() { 1 } else { 0 }, "a free-for-all has no sides on the map's row");
        let ffa = map["modes"].as_array().unwrap().iter().find(|mode| mode["mode"] == "Free for all").unwrap();
        assert_eq!(ffa["matches"], 7);
        assert!(ffa["sides"].is_null());
        assert_eq!(ffa["weathers"][0]["matches"], 7);
        let versions: Vec<(&str, u64)> = ffa["versions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| (row["mode"].as_str().unwrap(), row["matches"].as_u64().unwrap()))
            .collect();
        assert_eq!(versions, [("Free for all", 2), ("FFA: Racket", 1), ("FFA: Roulette", 4)]);
        assert_eq!(ffa["versions"][2]["weathers"][0]["matches"], 4);
        assert!(map["modes"].as_array().unwrap().iter().all(|mode| mode["mode"] == "Free for all" || mode.get("versions").is_none()), "only the free-for-all has versions");
    }

    /// The modes come in the rooms' create window's order (`RoomsCreate.as`'s
    /// `MODE_ORDER`, read here so that the two cannot drift): Blackout right
    /// after the protective device's other kinds, a free-for-all's twists
    /// right after it, a mode srp no longer knows last.
    #[test]
    fn modes_come_in_the_create_windows_order() {
        let create = include_str!("../../../../flash/lobby_menu/src/survarium/rooms/RoomsCreate.as");
        let constants = include_str!("../../../../flash/lobby_menu/src/survarium/constants/GameMode.as");
        let list = create
            .split_once("MODE_ORDER:Array = [")
            .and_then(|(_, rest)| rest.split_once("];"))
            .expect("MODE_ORDER")
            .0;
        let number = |name: &str| -> u8 {
            let head = format!("public static const {name}:int = ");
            let at = constants.find(&head).unwrap_or_else(|| panic!("{name}")) + head.len();
            constants[at..].split(';').next().unwrap().trim().parse().unwrap()
        };
        let window: Vec<u8> = list
            .split(',')
            .map(|item| number(item.trim().strip_prefix("GameMode.").unwrap()))
            .collect();
        assert_eq!(window, MODE_ORDER.map(GameMode::lobby_number));
        let mut all: Vec<KeptMode> = LobbyMode::ALL.into_iter().map(|mode| counted_as(KeptMode::Known(mode))).collect();
        all.push(KeptMode::Unknown(200));
        all.sort_by_key(|mode| mode_rank(*mode));
        all.dedup();
        let names: Vec<String> = all
            .iter()
            .map(|mode| match mode {
                KeptMode::Known(mode) => launcher::lobby_mode_name(Lang::En, *mode),
                KeptMode::Unknown(number) => launcher::unknown_mode_name(Lang::En, *number),
            })
            .collect();
        assert_eq!(
            names,
            [
                "Protective device",
                "Protective device: chargers",
                "Protective device: blackout",
                "Team Deathmatch",
                "Research",
                "Freeze Tag",
                "Free for all",
                "Artifact Hunt",
                "Mode 200",
            ],
            "every free-for-all one"
        );
        // The free-for-all's versions in their own order under it.
        let mut versions: Vec<KeptMode> = LobbyMode::ALL
            .into_iter()
            .map(KeptMode::Known)
            .filter(|mode| is_free_for_all(*mode))
            .map(version_of)
            .collect();
        versions.sort_by_key(|mode| mode_rank(*mode));
        versions.dedup();
        let names: Vec<String> = versions
            .iter()
            .map(|mode| match mode {
                KeptMode::Known(mode) => launcher::lobby_mode_name(Lang::En, *mode),
                KeptMode::Unknown(_) => unreachable!(),
            })
            .collect();
        assert_eq!(names, ["Free for all", "FFA: Racket", "FFA: Saloon", "FFA: Saloon (Winchester)", "FFA: Roulette"]);
    }

    /// Every mode and weather is named in the site's own words, and those
    /// are the game's own texts of the rooms (`client-patches/localization`)
    /// in each of the twelve languages: the stats never hang on the game data.
    #[test]
    fn modes_and_weathers_are_the_games_words_in_every_language() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../client-patches/localization");
        for lang in Lang::ALL {
            let file = lang.game().path().split('/').nth(1).unwrap().to_owned();
            let text = std::fs::read_to_string(dir.join(format!("{file}.tsv"))).unwrap();
            let game = |key: &str| -> String {
                text.lines()
                    .find_map(|line| line.strip_prefix(key).and_then(|rest| rest.strip_prefix('\t')))
                    .unwrap_or_else(|| panic!("{file}: {key}"))
                    .to_owned()
            };
            for mode in LobbyMode::ALL {
                assert_eq!(launcher::lobby_mode_name(lang, mode), game(mode.name_key()), "{file}: {mode:?}");
            }
            for (weather, key) in [
                (maps::weather::DAY, "st_srp_weather_day"),
                (maps::weather::RAIN, "st_srp_weather_rain"),
                (maps::weather::EVENING, "st_srp_weather_evening"),
                (maps::weather::NIGHT, "st_srp_weather_night"),
                (maps::weather::PRERAIN, "st_srp_weather_before_rain"),
            ] {
                assert_eq!(weather_name(lang, weather), Some(game(key)), "{file}: {key}");
            }
            assert!(!launcher::unknown_mode_name(lang, 200).contains("{0}"));
        }
    }
}
