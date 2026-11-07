//! A match recording ([`super::record`]) made into the history's match
//! with its events (`session::v026g::persist::history`), for the matches
//! played before the history was kept: `srp-recording-to-history`.
//!
//! What the recording gives, and what it does not:
//! - the header (map, level, places and their accounts, whether against
//!   bots or private) and the result note (the end, the standing, each
//!   place's entry, presence at the end, score, kills and deaths);
//! - the worlds the clients uploaded: the events ([`super::events`], the
//!   same as live, over every time's world most uploads of it agree on)
//!   and, from the last of them, each player's counters and kits used;
//! - not the rating after the match (none is kept), nor the clan then
//!   (none); and the header says whether a room had a PIN only since the
//!   events were added: a recording from before is kept as private (never
//!   served, never counted), since it cannot say it was public.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::Deserialize;
use session::v026g::persist::history::{
    self, Counters, Eligibility, GearSlot, KitCounts, KitItem, KitLine, MatchInfo, MatchKind,
    MatchRecord, PlayerLine, PlayerRecord, Presence, Winner,
};
use session::v026g::replays::ReplayId;
use survarium::v026g::maps::{GameMode, MatchMap};
use survarium::v026g::match_msgs::MAX_PLAYERS;
use survarium::v026g::match_msgs::rules::DRAW;
use survarium::v026g::match_msgs::snapshot::{self, KITS, Statistics};
use survarium::v026g::match_msgs::static_info::MatchProfile;

use super::events::{Extractor, Seat, World};
use super::record;
use super::world_check;

/// Why a recording gives no match.
#[derive(Debug)]
pub enum Unread {
    Io(io::Error),
    /// Its file name is not a replay id.
    Name,
    /// No header note, or one this server does not read.
    Header,
    /// Its map is not one a match is played on here.
    Map(u8),
    /// No result note: its server stopped before the match ended.
    NoResult,
    /// Nobody came, or a room's match was called off: the history keeps
    /// none of those.
    NotPlayed,
    /// No account played in it.
    NoPlayers,
}

impl std::fmt::Display for Unread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "can't read it: {error}"),
            Self::Name => f.write_str("its name is not a replay id"),
            Self::Header => f.write_str("no header note this server reads"),
            Self::Map(id) => write!(f, "its map {id} is not played here"),
            Self::NoResult => f.write_str("no result: its server stopped before the end"),
            Self::NotPlayed => f.write_str("nobody came, or the match was called off"),
            Self::NoPlayers => f.write_str("no account played in it"),
        }
    }
}

impl From<io::Error> for Unread {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// A place of the header or of a `places` note.
#[derive(Deserialize)]
struct PlaceNote {
    place: usize,
    account_id: u32,
    team: u8,
    #[serde(default)]
    bot: bool,
}

#[derive(Deserialize)]
struct Header {
    map_id: u8,
    match_level: u16,
    #[serde(default)]
    growing_roster: bool,
    #[serde(default)]
    vs_bots: bool,
    /// Since the events: `None` in a recording made before.
    private: Option<bool>,
    began_unix_ms: u64,
    places: Vec<PlaceNote>,
}

#[derive(Deserialize)]
struct ResultPlayer {
    place: usize,
    account_id: u32,
    team: u8,
    entered: bool,
    present_at_end: bool,
    score: u32,
    kills: u16,
    deaths: u16,
}

#[derive(Deserialize)]
struct ResultNote {
    end: String,
    winner: Option<u8>,
    team_points: [u16; 2],
    duration_ms: u32,
    played_ms: Option<u32>,
    players: Vec<ResultPlayer>,
}

/// The match of the recording at `path`, as the history keeps it.
pub fn match_of_recording(path: &Path) -> Result<MatchRecord, Unread> {
    let replay_id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(ReplayId::parse)
        .ok_or(Unread::Name)?;
    let mut header: Option<Header> = None;
    let mut seats: BTreeMap<usize, (Seat, bool)> = BTreeMap::new();
    let mut result: Option<(u32, ResultNote)> = None;
    for note in record::read_notes(path)? {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&note.bytes) else {
            continue;
        };
        let places = match value["note"].as_str() {
            Some("header") => {
                let read: Header = serde_json::from_value(value).map_err(|_| Unread::Header)?;
                let places = std::mem::take(&mut header.insert(read).places);
                Some(places)
            }
            Some("places") => serde_json::from_value(value["places"].clone()).ok(),
            Some("result") => {
                result = serde_json::from_value(value)
                    .ok()
                    .map(|read| (note.at_ms, read));
                None
            }
            _ => None,
        };
        for place in places.unwrap_or_default() {
            let seat = Seat {
                account_id: place.account_id,
                team: place.team,
            };
            seats.insert(place.place, (seat, place.bot || place.account_id == 0));
        }
    }
    let header = header.ok_or(Unread::Header)?;
    let (result_at, result) = result.ok_or(Unread::NoResult)?;
    if matches!(result.end.as_str(), "NoShow" | "NotMade") {
        return Err(Unread::NotPlayed);
    }
    let map = MatchMap::of(header.map_id).ok_or(Unread::Map(header.map_id))?;
    let mode = map.mode();
    let rules = mode.rules();
    let batteries = if rules.has_batteries() {
        map.map().victory_items_count as usize
    } else {
        0
    };
    let roster: Vec<Seat> = (0..seats.keys().next_back().map_or(0, |last| last + 1))
        .map(|place| {
            seats.get(&place).map_or(
                // A place no note named: nobody's.
                Seat {
                    account_id: 0,
                    team: DRAW,
                },
                |(seat, _)| *seat,
            )
        })
        .collect();
    let teams: Vec<u8> = roster.iter().map(|seat| seat.team).collect();
    let places = if header.growing_roster {
        MAX_PLAYERS
    } else {
        roster.len()
    };

    // Each time's world: the one most of its uploads agree on.
    let recording = world_check::read_recording(path)?;
    let mut by_time: BTreeMap<u32, Vec<&[u8]>> = BTreeMap::new();
    for upload in &recording.uploads {
        by_time.entry(upload.time).or_default().push(&upload.body);
    }
    let mut extractor = Extractor::new(rules);
    let mut last: Option<Statistics> = None;
    for (time, bodies) in by_time {
        let body = bodies
            .iter()
            .max_by_key(|body| bodies.iter().filter(|other| other == body).count())
            .expect("a time has an upload");
        let Some(decoded) = snapshot::decode(time, body, places, rules, batteries) else {
            continue;
        };
        let Some(statistics) = decoded.statistics.as_ref() else {
            continue;
        };
        let world = World {
            time,
            statistics,
            standing: decoded.standing(rules, &teams),
            research: decoded
                .rules
                .as_ref()
                .and_then(|read| read.research.as_ref()),
        };
        if extractor.observe(&world, &roster) {
            last = decoded.statistics;
        }
    }

    let profiles = recording
        .options
        .as_ref()
        .map_or(&[][..], |options| &options.players[..]);
    let research = mode == GameMode::Research;
    // An account's latest place is its result (`MatchResult::latest_places`).
    let latest = |player: &ResultPlayer| {
        !result
            .players
            .iter()
            .any(|later| later.account_id == player.account_id && later.place > player.place)
    };
    let mut players: Vec<PlayerRecord> = Vec::new();
    for player in result.players.iter().filter(|player| latest(player)) {
        let presence = match (result.played_ms, player.entered, player.present_at_end) {
            (Some(_), true, true) => Presence::Finished,
            (Some(_), true, false) => Presence::Left,
            _ => continue,
        };
        let bot = seats.get(&player.place).is_none_or(|(_, bot)| *bot);
        if bot || player.account_id == 0 {
            continue;
        }
        let events = last
            .as_ref()
            .and_then(|statistics| statistics.players.get(player.place))
            .map(|statistics| statistics.events);
        players.push(PlayerRecord {
            account_id: player.account_id,
            clan_id: None,
            line: PlayerLine {
                team: player.team,
                place: 0,
                presence,
                rating: None,
                score: u64::from(player.score),
                kills: u64::from(player.kills),
                deaths: u64::from(player.deaths),
                counters: events.map_or(Counters::default(), |events| {
                    Counters::of(&events, research)
                }),
            },
            kits: match (events, profiles.get(player.place)) {
                (Some(events), Some(profile)) => kits_used(&events, profile),
                _ => Vec::new(),
            },
        });
    }
    if players.is_empty() {
        return Err(Unread::NoPlayers);
    }
    history::rank_places(&mut players);
    let bots = seats.values().any(|(_, bot)| *bot);
    let kind = match (header.vs_bots, header.private) {
        (true, _) => MatchKind::VsBots,
        (false, Some(false)) => {
            MatchKind::Public(Eligibility::of(bots, players.len(), result.played_ms))
        }
        (false, Some(true) | None) => MatchKind::Private,
    };
    Ok(MatchRecord {
        replay_id,
        kind,
        info: MatchInfo {
            ended_at: (header.began_unix_ms + u64::from(result_at)) / 1000,
            duration_ms: result.duration_ms,
            map_id: header.map_id,
            game_mode: mode,
            match_level: header.match_level,
            winner: Winner::of(result.winner, DRAW),
            team_points: result.team_points,
        },
        players,
        events: extractor.finish(result.duration_ms, result.team_points),
    })
}

/// The kits a player used, by its statistics' events: every kit with a
/// counter above 0, with what its profile's kit held.
fn kits_used(events: &[[u16; KITS]; snapshot::EVENTS], profile: &MatchProfile) -> Vec<KitLine> {
    (0..KITS.min(profile.kits.len()))
        .filter(|&kit| events.iter().any(|counts| counts[kit] > 0))
        .map(|kit| KitLine {
            kit: kit as u8,
            counts: KitCounts::of(events, kit),
            items: profile.kits[kit]
                .slots
                .iter()
                .enumerate()
                .filter_map(|(slot, held)| {
                    let (index, _) = (*held)?;
                    Some(KitItem {
                        slot: GearSlot::of(slot)?,
                        dict_id: profile.items.get(usize::from(index))?.dict_id,
                    })
                    .filter(|item| item.dict_id != 0)
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use session::v026g::persist::match_events::{EventKind, Player, Timing};
    use session::v026g::persist::{GameRepository, SqliteGameRepository};

    use super::*;

    /// Two local matches of two players on level_01_4 (Team Deathmatch),
    /// each with one kill: its killer's `last_killed_enemy_ms` in the world
    /// that first shows it, and the kd stats of the result note.
    const FIRST: &str = "rec026g_20261107-135509_7fa850a0.srprec";
    const SECOND: &str = "rec026g_20261107-142132_5b294769.srprec";

    fn data(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name)
    }

    fn kills(record: &MatchRecord) -> Vec<(Timing, EventKind)> {
        record
            .events
            .iter()
            .filter(|event| matches!(event.kind, EventKind::Kill { .. }))
            .map(|event| (event.timing, event.kind))
            .collect()
    }

    #[test]
    fn a_recordings_kills_are_at_their_ticks() {
        let first = match_of_recording(&data(FIRST)).unwrap();
        // Place 0 (account 2) killed place 1 (account 1) between the worlds
        // of 94 550 and 99 600 ms, at the tick of 99 100.
        assert_eq!(
            kills(&first),
            [(
                Timing::Exact(99_100),
                EventKind::Kill {
                    killer: Player {
                        place: 0,
                        account_id: 2
                    },
                    victim: Some(Player {
                        place: 1,
                        account_id: 1
                    }),
                }
            )]
        );
        let second = match_of_recording(&data(SECOND)).unwrap();
        assert_eq!(
            kills(&second),
            [(
                Timing::Exact(134_550),
                EventKind::Kill {
                    killer: Player {
                        place: 1,
                        account_id: 4
                    },
                    victim: Some(Player {
                        place: 0,
                        account_id: 7
                    }),
                }
            )]
        );
        // The result note says the same kills and deaths.
        for record in [&first, &second] {
            let kills: u64 = record.players.iter().map(|player| player.line.kills).sum();
            let deaths: u64 = record.players.iter().map(|player| player.line.deaths).sum();
            assert_eq!((kills, deaths), (1, 1));
        }
        // Nothing else happened but the end, at the match clock's end.
        assert_eq!(first.events.len(), 2);
        assert_eq!(
            first.events.last().unwrap().kind,
            EventKind::End {
                points: first.info.team_points
            }
        );
        assert_eq!(
            first.events.last().unwrap().timing,
            Timing::Exact(first.info.duration_ms)
        );
        assert_eq!(first.replay_id.as_str(), FIRST.trim_end_matches(".srprec"));
        assert_eq!(first.info.game_mode, GameMode::TeamDeathmatch);
        // Recorded before the header said whether the room had a PIN.
        assert_eq!(first.kind, MatchKind::Private);
        assert!(
            first
                .players
                .iter()
                .all(|player| player.line.rating.is_none())
        );
    }

    #[test]
    fn a_recording_is_kept_once() {
        let dir = std::env::temp_dir().join(format!("srp-backfill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut repository = SqliteGameRepository::open_in(&dir).unwrap();
        let first = match_of_recording(&data(FIRST)).unwrap();
        assert!(repository.record_match(&first).unwrap());
        assert!(!repository.record_match(&first).unwrap());
        assert!(
            !repository
                .record_match(&match_of_recording(&data(FIRST)).unwrap())
                .unwrap()
        );
        assert!(
            repository
                .record_match(&match_of_recording(&data(SECOND)).unwrap())
                .unwrap()
        );
        let history = repository.history().unwrap().unwrap();
        let kept = history.events(first.replay_id.match_id()).unwrap().unwrap();
        assert_eq!(kept.replay_id, first.replay_id);
        assert_eq!(kept.events, first.events);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
