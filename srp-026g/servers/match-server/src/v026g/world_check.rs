//! Real worlds checked offline exactly as the match server checks an upload
//! ([`world_validation::check`]): every complete upload (0xE0) of a match
//! recording ([`record`]) against that match's own roster (its 0xE4s and
//! header), and the kept dumps of uploads (`SRP_SNAPSHOT_DIR`,
//! `match026g_<id>_<ms>_p<place>_<kind>_r<run>.bin`) of a match whose
//! recording is checked with them. What `srp-world-check` runs, and the
//! release before its go (`release/release.sh`): a change of what the games
//! write, of the check, or of the game data that makes them disagree shows
//! as a refusal here, not as players with no world in a live match.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use srp_validate::MAX_PLAYERS;
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::match_msgs::static_info::{self, MatchOptions};
use survarium::v026g::match_msgs::upload::{Chunk, Pushed, Reassembler};
use udp_transport::table::{Direction, Table};
use udp_transport::wire;

use super::record::{self, IN, NOTE, OUT};
use super::sur_convert::{OptionsParts, mask_len};
use super::world_validation::{self, Refused, Roster};

/// One complete upload of a recording.
#[derive(Clone, Debug)]
pub struct Upload {
    pub place: u8,
    /// Its world's time (the chunks' item time).
    pub time: u32,
    pub body: Vec<u8>,
}

/// What a recording gives to check its uploads with, and the uploads.
#[derive(Debug, Default)]
pub struct Recording {
    /// From its file name.
    pub match_id: Option<u32>,
    pub growing_roster: bool,
    /// The header's `initial_players`; else the first roster a 0xE4 gave.
    pub initial_players: Option<usize>,
    /// The match options of the largest roster any place was sent: every
    /// place a world can name.
    pub options: Option<MatchOptions>,
    /// 0xE4s whose options did not read: the parts of resends that share a
    /// unique id (seen in live recordings), or an item this server data
    /// lacks. The largest roster read is what the worlds are checked with.
    pub unparsed_options: usize,
    /// The revisions of the games that played it (the `revision` notes).
    pub revisions: BTreeSet<u64>,
    pub uploads: Vec<Upload>,
}

/// Reads `path`, a recording: its notes, the 0xE4s it sent and the uploads
/// it was sent.
pub fn read_recording(path: &Path) -> io::Result<Recording> {
    let dictionary = Dictionary::shared();
    let mut recording = Recording {
        match_id: record::match_of_file(path),
        ..Recording::default()
    };
    let mut first_roster = None;
    let mut places: Vec<(u8, OptionsParts, Reassembler)> = Vec::new();
    record::read_each(path, |record| {
        if record.kind == NOTE {
            let Ok(note) = serde_json::from_slice::<serde_json::Value>(record.bytes) else {
                return;
            };
            match note["note"].as_str() {
                Some("header") => {
                    recording.growing_roster = note["growing_roster"].as_bool().unwrap_or(false);
                    recording.initial_players =
                        note["initial_players"].as_u64().map(|n| n as usize);
                }
                Some("revision") => {
                    if let Some(revision) = note["revision"].as_u64() {
                        recording.revisions.insert(revision);
                    }
                }
                _ => {}
            }
            return;
        }
        if record.kind != IN && record.kind != OUT {
            return;
        }
        let at = usize::from(record.place);
        if at >= MAX_PLAYERS {
            return;
        }
        if places.len() <= at {
            places.resize_with(at + 1, || {
                (0, OptionsParts::default(), Reassembler::default())
            });
        }
        let (roster, options, uploads) = &mut places[at];
        if record.kind == IN {
            let Ok(datagram) = wire::parse(Table::Match, Direction::ToServer, record.bytes, 0)
            else {
                return;
            };
            for message in &datagram.messages {
                if message.id != wire::UPLOAD_CARRIER
                    || message.part.is_some()
                    || message.payload.is_empty()
                {
                    continue;
                }
                let Ok(chunk) = Chunk::parse(message.payload) else {
                    continue;
                };
                if let Pushed::Complete(upload) = uploads.push(&chunk) {
                    recording.uploads.push(Upload {
                        place: record.place,
                        time: upload.item_time,
                        body: upload.body,
                    });
                }
            }
            return;
        }
        let masks = mask_len(recording.growing_roster, *roster);
        let Ok(datagram) = wire::parse(Table::Match, Direction::ToClient, record.bytes, masks)
        else {
            return;
        };
        for message in datagram.messages.iter().filter(|m| m.id == 0xE4) {
            let Some(whole) = options.push(message) else {
                continue;
            };
            match static_info::parse_payload(&whole, dictionary) {
                Ok((_, parsed)) => {
                    *roster = parsed.players.len() as u8;
                    first_roster.get_or_insert(parsed.players.len());
                    let larger = recording
                        .options
                        .as_ref()
                        .is_none_or(|known| known.players.len() < parsed.players.len());
                    if larger {
                        recording.options = Some(parsed);
                    }
                }
                Err(_) => recording.unparsed_options += 1,
            }
        }
    })?;
    if recording.initial_players.is_none() {
        recording.initial_players = first_roster;
    }
    Ok(recording)
}

impl Recording {
    /// The match its uploads are checked as, if a 0xE4 was read.
    pub fn roster(&self) -> Option<Roster<'_>> {
        let options = self.options.as_ref()?;
        Some(Roster {
            map: options.map,
            profiles: &options.players,
            initial_players: self.initial_players.unwrap_or(options.players.len()),
        })
    }

    /// The places its worlds are for (`Game::snapshot_players`).
    pub fn places(&self) -> usize {
        match &self.options {
            _ if self.growing_roster => MAX_PLAYERS,
            Some(options) => options.players.len(),
            None => 0,
        }
    }

    /// [`check`] of `body`, a world of this match at `time`.
    pub fn check(&self, body: &[u8], time: u32) -> Result<(), Unchecked> {
        let roster = self.roster().ok_or(Unchecked::NoOptions)?;
        world_validation::check(body, time, self.places(), roster).map_err(Unchecked::Refused)
    }
}

/// Why a world did not pass.
#[derive(Clone, Copy, Debug)]
pub enum Unchecked {
    /// The match server would refuse it.
    Refused(Refused),
    /// Its recording gave no match options to check it with: none was
    /// recorded, or none parsed with this server data (an item of its kits
    /// the data lacks).
    NoOptions,
}

impl std::fmt::Display for Unchecked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(refused) => refused.fmt(f),
            Self::NoOptions => {
                f.write_str("not checked: its recording has no 0xE4 this server data reads")
            }
        }
    }
}

/// A kept world dump's name, `match026g_<id hex>_<ms>_p<place>_<kind>_r<run>.bin`
/// (`Game::dump`): its match, time and place.
pub fn dump_of_name(name: &str) -> Option<(u32, u32, u8)> {
    let rest = name.strip_prefix("match026g_")?.strip_suffix(".bin")?;
    let mut fields = rest.split('_');
    let id = u32::from_str_radix(fields.next()?, 16).ok()?;
    let time = fields.next()?.parse().ok()?;
    let place = fields.next()?.strip_prefix('p')?.parse().ok()?;
    Some((id, time, place))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dump_is_named_by_its_match_time_and_place() {
        assert_eq!(
            dump_of_name("match026g_1c0ebd31_104750_p3_unloadable_r1730000000.bin"),
            Some((0x1c0e_bd31, 104_750, 3))
        );
        assert_eq!(
            dump_of_name("match026g_1c0ebd31_104750_p3_live.bin"),
            Some((0x1c0e_bd31, 104_750, 3))
        );
        assert_eq!(dump_of_name("rec026g_x.srprec"), None);
        assert_eq!(dump_of_name("match026g_zz_1_p0_x.bin"), None);
    }
}
