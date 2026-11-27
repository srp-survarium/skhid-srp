//! Matches of before the history, made again from their recordings
//! (`srp-history-backfill`; `docs/notes/match_history.md`, "Matches of
//! before the history").
//!
//! Two steps, as the tool runs them:
//!
//! 1. [`extract`]: a recording ([`record`]) read with one server game data
//!    (the match options' kits are read with it, so a period's recordings
//!    are read with its own data): the header, the result note, and from the
//!    world the match server trusted last (the upload of the result's
//!    `trusted_snapshot_ms`) every place's counters, with the kits each
//!    place was given and its clan tag. The trusted world must say every
//!    place's kills and deaths as the result note does, or the match is
//!    dropped ([`Dropped::Disagrees`]): what is kept is what srp would have
//!    read at the match's end.
//! 2. [`MatchFacts::result`]: the match as srp's settlement had it
//!    ([`MatchResult`]), from which the history's record is made by the same
//!    code srp writes a match with (`rewards::history_record`).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use session::v026g::persist::history::KitItem;
use session::v026g::replays::ReplayId;
use session::v026g::rewards::{MatchEnd, MatchResult, PlayerResult};
use survarium::v026g::maps::{MapId, MatchMap, RuleMode};
use survarium::v026g::match_msgs::snapshot::{
    self, BatteryRecords, EVENTS, KITS, PlayerStatistics,
};

use super::record::{self, ACCEPTED_WORLD, NOTE};
use super::world_check;

/// Why a recording gives no match to the history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "why")]
pub enum Dropped {
    /// It does not read, or has no header or no result note (a server that
    /// stopped mid-match).
    Unreadable {
        error: String,
    },
    NoResult,
    /// One player against the server's bots: never served.
    VsBots,
    /// Its map is not in the game data any more.
    UnknownMap {
        map_id: u16,
    },
    /// A snapshot was trusted, and no upload of its time reads.
    TrustedWorldUnreadable,
    /// The trusted world's kills or deaths of a place are not the result's.
    Disagrees {
        place: u8,
    },
}

/// One place of a match as its trusted world and options had it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceFacts {
    pub place: u8,
    /// The name it played under (the header's, or a late joiner's).
    pub name: String,
    /// Its counters by event, then kit (`events[event][kit]`); `None`
    /// without a trusted world.
    pub events: Option<Vec<[u16; KITS]>>,
    pub killstreak: u8,
    pub longest_killstreak: u16,
    /// Each of its kits' items by profile slot (`(slot, dict id)`), as the
    /// match options gave them; empty when no options read.
    pub kits: Vec<Vec<(u8, u16)>>,
    /// Its clan's tag as the options showed it (`""`: none).
    pub clan_tag: String,
}

/// A place of the result note (`game.rs::record_result`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotePlayer {
    pub place: u8,
    pub account_id: u32,
    pub team: u8,
    pub entered: bool,
    pub present_at_end: bool,
    pub left: bool,
    pub time_in_match_ms: u32,
    pub score: u32,
    pub kills: u16,
    pub deaths: u16,
    #[serde(default)]
    pub longest_killstreak: u16,
}

/// The result note.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultNote {
    pub end: String,
    pub winner: Option<u8>,
    pub team_points: [u16; 2],
    pub duration_ms: u32,
    pub played_ms: Option<u32>,
    pub trusted_snapshot_ms: Option<u32>,
    pub trusted_by: u8,
    pub players: Vec<NotePlayer>,
}

/// What a recording says of its match.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchFacts {
    /// The file's stem: the match's replay id.
    pub stem: String,
    pub map_id: u16,
    pub rules: u8,
    pub match_level: u16,
    /// When the match ended (Unix seconds): the recording's start and the
    /// result note's time in it.
    pub ended_at: u64,
    pub result: ResultNote,
    /// By roster place.
    pub places: Vec<PlaceFacts>,
    /// Whether the match options read with this game data (the kits).
    pub kits_read: bool,
    /// The battery records' layout its trusted world read with.
    pub layout: Option<String>,
}

/// What [`extract`] makes of one recording.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extract {
    pub stem: String,
    pub facts: Option<MatchFacts>,
    pub dropped: Option<Dropped>,
}

/// Reads a recording ([`MatchFacts`]), with the game data `init`ed.
pub fn extract(path: &Path) -> Extract {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    match facts(path, &stem) {
        Ok(facts) => Extract {
            stem,
            facts: Some(facts),
            dropped: None,
        },
        Err(dropped) => Extract {
            stem,
            facts: None,
            dropped: Some(dropped),
        },
    }
}

fn facts(path: &Path, stem: &str) -> Result<MatchFacts, Dropped> {
    let mut header = serde_json::Value::Null;
    let mut result = None;
    let mut result_at = 0u32;
    let mut names: BTreeMap<u8, String> = BTreeMap::new();
    record::read_each(path, |record| {
        if record.kind == ACCEPTED_WORLD || record.kind != NOTE {
            return;
        }
        let Ok(note) = serde_json::from_slice::<serde_json::Value>(record.bytes) else {
            return;
        };
        match note["note"].as_str() {
            Some("header") | Some("places") => {
                for place in note["places"].as_array().into_iter().flatten() {
                    if let (Some(at), Some(name)) =
                        (place["place"].as_u64(), place["name"].as_str())
                    {
                        names.insert(at as u8, name.to_owned());
                    }
                }
                if note["note"] == "header" {
                    header = note;
                }
            }
            Some("result") => {
                result = Some(note);
                result_at = record.at_ms;
            }
            _ => {}
        }
    })
    .map_err(|error| Dropped::Unreadable {
        error: error.to_string(),
    })?;
    if header.is_null() {
        return Err(Dropped::Unreadable {
            error: "no header".to_owned(),
        });
    }
    let result: ResultNote = result.ok_or(Dropped::NoResult).and_then(|note| {
        serde_json::from_value(note).map_err(|error| Dropped::Unreadable {
            error: error.to_string(),
        })
    })?;
    if header["vs_bots"].as_bool() == Some(true) {
        return Err(Dropped::VsBots);
    }
    let map_id = header["map_id"].as_u64().unwrap_or(0) as u16;
    let map = MatchMap::of(MapId::new(map_id)).ok_or(Dropped::UnknownMap { map_id })?;
    let rules = header["mode"].as_u64().unwrap_or(0) as u8;
    let mode = RuleMode::of_config(rules).ok_or(Dropped::UnknownMap { map_id })?;
    let began_ms = header["began_unix_ms"].as_u64().unwrap_or(0);
    let recording = world_check::read_recording(path).map_err(|error| Dropped::Unreadable {
        error: error.to_string(),
    })?;
    let batteries = if mode.has_batteries() {
        map.map().victory_items_count as usize
    } else {
        0
    };
    let options = recording.options.as_ref();
    // The trusted world, as the match server read it at the end: the
    // upload of the trusted time.
    let mut statistics = None;
    let mut layout = None;
    if let Some(trusted) = result.trusted_snapshot_ms {
        let free_for_all = mode == RuleMode::FreeForAll;
        let mut disagrees = None;
        'uploads: for upload in recording
            .uploads
            .iter()
            .filter(|upload| upload.time == trusted)
        {
            for records in [BatteryRecords::Patch92, BatteryRecords::BeforePatch92] {
                let decoded = snapshot::decode_as(
                    upload.time,
                    &upload.body,
                    recording.places(),
                    mode,
                    batteries,
                    records,
                );
                let Some(read) = decoded.and_then(|decoded| decoded.statistics) else {
                    continue;
                };
                match disagreement(&read, &result.players, free_for_all) {
                    None => {
                        statistics = Some(read);
                        layout = Some(format!("{records:?}"));
                        break 'uploads;
                    }
                    Some(place) => {
                        disagrees.get_or_insert(place);
                    }
                }
                break;
            }
        }
        if statistics.is_none() {
            return Err(disagrees.map_or(Dropped::TrustedWorldUnreadable, |place| {
                Dropped::Disagrees { place }
            }));
        }
    }
    let mut places = Vec::new();
    for player in &result.players {
        let at = usize::from(player.place);
        let read = statistics.as_ref().and_then(|read| read.players.get(at));
        let profile = options.and_then(|options| options.players.get(at));
        let kits = profile.map_or_else(Vec::new, |profile| {
            profile
                .kits
                .iter()
                .map(|kit| {
                    kit.slots
                        .iter()
                        .enumerate()
                        .filter_map(|(slot, held)| {
                            let (index, count) = (*held)?;
                            let item = profile.items.get(usize::from(index))?;
                            (count > 0 && item.dict_id != 0).then_some((slot as u8, item.dict_id))
                        })
                        .collect()
                })
                .collect()
        });
        places.push(PlaceFacts {
            place: player.place,
            name: names.get(&player.place).cloned().unwrap_or_default(),
            events: read.map(|read| read.events.to_vec()),
            killstreak: read.map_or(0, |read| read.killstreak),
            longest_killstreak: read.map_or(player.longest_killstreak, |read| {
                read.longest_killstreak.max(player.longest_killstreak)
            }),
            kits,
            clan_tag: profile
                .map(|profile| profile.clan_tag.clone())
                .unwrap_or_default(),
        });
    }
    Ok(MatchFacts {
        stem: stem.to_owned(),
        map_id,
        rules,
        match_level: header["match_level"].as_u64().unwrap_or(0) as u16,
        ended_at: (began_ms + u64::from(result_at)) / 1000,
        result,
        places,
        kits_read: options.is_some(),
        layout,
    })
}

/// The first place whose kills or deaths in `read` are not the result's.
/// A free-for-all's standing is its first terminal world's, kept through
/// the closing delay (`game.rs::observe`), while its counters are the
/// trusted world's (as srp wrote its lines): a kill of the delay counts in
/// them and not in the standing, so there the counters may be ahead of the
/// standing by [`FREE_FOR_ALL_DELAY_KILLS`], never behind.
fn disagreement(
    read: &snapshot::Statistics,
    players: &[NotePlayer],
    free_for_all: bool,
) -> Option<u8> {
    let agrees = |counted: u32, shown: u16| {
        let shown = u32::from(shown);
        if free_for_all {
            counted >= shown && counted - shown <= FREE_FOR_ALL_DELAY_KILLS
        } else {
            counted == shown
        }
    };
    players
        .iter()
        .find(|player| {
            read.players
                .get(usize::from(player.place))
                .is_none_or(|place| {
                    !agrees(place.total(snapshot::event::KILL), player.kills)
                        || !agrees(place.total(snapshot::event::DEATH), player.deaths)
                })
        })
        .map(|player| player.place)
}

/// What a free-for-all's counters may be ahead of its standing by, a place.
const FREE_FOR_ALL_DELAY_KILLS: u32 = 2;

impl MatchFacts {
    /// The match as srp's settlement had it, the game data `init`ed (its
    /// mode, by its map). `None`: its replay id does not read, or the map is
    /// not in the game data. Never private nor against bots: a private
    /// room's match is told by the caller (its report), one against bots is
    /// dropped by [`extract`].
    pub fn result(&self) -> Option<MatchResult> {
        let replay_id = ReplayId::parse(&self.stem)?;
        let map = MatchMap::of(MapId::new(self.map_id))?;
        let end = match self.result.end.as_str() {
            "ModeRule" => MatchEnd::ModeRule,
            "TimeLimit" => MatchEnd::TimeLimit,
            "Deserted" => MatchEnd::Deserted,
            "NoShow" => MatchEnd::NoShow,
            _ => MatchEnd::NotMade,
        };
        let players = self
            .result
            .players
            .iter()
            .zip(&self.places)
            .map(|(player, place)| PlayerResult {
                place: player.place,
                session_id: 0,
                account_id: player.account_id,
                name: place.name.clone(),
                team: player.team,
                // The match server's own bots have no account.
                driven: player.account_id == 0,
                entered: player.entered,
                present_at_end: player.present_at_end,
                left: player.left,
                time_in_match_ms: player.time_in_match_ms,
                statistics: place.events.as_ref().map(|events| {
                    let mut statistics = PlayerStatistics::default();
                    for (row, read) in statistics.events.iter_mut().zip(events) {
                        *row = *read;
                    }
                    statistics.killstreak = place.killstreak;
                    statistics.longest_killstreak = place.longest_killstreak;
                    statistics
                }),
                pay_statistics: None,
                score: player.score,
                kills: player.kills,
                deaths: player.deaths,
                longest_killstreak: player.longest_killstreak,
                kit_changes: 0,
                supplies: None,
                // No game of before exe patch 165 counted by weapon.
                weapons: Vec::new(),
            })
            .collect();
        Some(MatchResult {
            match_id: replay_id.match_id(),
            replay_id,
            map_id: MapId::new(self.map_id),
            mode: map.mode(),
            twist: None,
            match_level: self.match_level,
            end,
            winner: self.result.winner,
            team_points: self.result.team_points,
            round_starts: 0,
            duration_ms: self.result.duration_ms,
            played_ms: self.result.played_ms,
            trusted_snapshot_ms: self.result.trusted_snapshot_ms,
            trusted_by: self.result.trusted_by,
            players,
            vs_bots: false,
            private: false,
        })
    }

    /// What the place's kit `index` held, as the history keeps it.
    pub fn kit_items(&self, place: u8, index: usize) -> Vec<KitItem> {
        self.places
            .iter()
            .find(|facts| facts.place == place)
            .and_then(|facts| facts.kits.get(index))
            .map_or_else(Vec::new, |kit| {
                kit.iter()
                    .filter_map(|&(slot, dict_id)| {
                        Some(KitItem {
                            slot: session::v026g::persist::history::GearSlot::of(usize::from(
                                slot,
                            ))?,
                            dict_id,
                        })
                    })
                    .collect()
            })
    }

    /// How many kits the place had (the options'), at most [`KITS`]; with no
    /// options read, every kit a counter can be of (their items unknown).
    pub fn kits_of(&self, place: u8) -> usize {
        self.places
            .iter()
            .find(|facts| facts.place == place)
            .filter(|facts| !facts.kits.is_empty())
            .map_or(KITS, |facts| facts.kits.len().min(KITS))
    }
}

/// The events a [`PlaceFacts`] keeps: all of them.
const _: () = assert!(EVENTS == 28);

#[cfg(test)]
mod tests {
    use super::*;

    fn note(place: u8, kills: u16, deaths: u16) -> NotePlayer {
        NotePlayer {
            place,
            account_id: 10 + u32::from(place),
            team: place % 2,
            entered: true,
            present_at_end: true,
            left: false,
            time_in_match_ms: 600_000,
            score: 100,
            kills,
            deaths,
            longest_killstreak: 0,
        }
    }

    fn statistics(counts: &[(u16, u16)]) -> snapshot::Statistics {
        let players = counts
            .iter()
            .map(|&(kills, deaths)| {
                let mut player = PlayerStatistics::default();
                player.events[snapshot::event::KILL][0] = kills;
                player.events[snapshot::event::DEATH][1] = deaths;
                player
            })
            .collect();
        snapshot::Statistics {
            players,
            victory_items: Vec::new(),
            tail: snapshot::StatisticsTail::default(),
            weapons: None,
        }
    }

    /// A team match's trusted world says its standing exactly; a
    /// free-for-all's counters may be ahead of its frozen standing by a kill
    /// or two of the closing delay, never behind.
    #[test]
    fn the_trusted_world_must_say_the_standing() {
        let players = [note(0, 5, 2), note(1, 2, 5)];
        assert_eq!(
            disagreement(&statistics(&[(5, 2), (2, 5)]), &players, false),
            None
        );
        assert_eq!(
            disagreement(&statistics(&[(6, 2), (2, 5)]), &players, false),
            Some(0)
        );
        assert_eq!(
            disagreement(&statistics(&[(5, 2)]), &players, false),
            Some(1)
        );
        assert_eq!(
            disagreement(&statistics(&[(6, 2), (2, 6)]), &players, true),
            None
        );
        assert_eq!(
            disagreement(&statistics(&[(8, 2), (2, 5)]), &players, true),
            Some(0)
        );
        assert_eq!(
            disagreement(&statistics(&[(4, 2), (2, 5)]), &players, true),
            Some(0)
        );
    }
}
