//! Real worlds checked offline exactly as the match server checks an upload
//! ([`world_validation::check`]): every complete upload (0xE0) of a match
//! recording ([`record`]) against that match's own roster (its 0xE4s and
//! header), and the kept dumps of uploads (`SRP_SNAPSHOT_DIR`,
//! `match026g_<id>_<ms>_p<place>_<kind>_r<run>.bin`) of a match whose
//! recording is checked with them. What `srp-world-check` runs, and the
//! release before its go (`release/release.sh`): a change of what the games
//! write, of the check, or of the game data that makes them disagree shows
//! as a refusal here, not as players with no world in a live match.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use srp_validate::MAX_PLAYERS;
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::match_msgs::motion::Motion;
use survarium::v026g::match_msgs::players::{self, ItemLeft, Supplies};
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

/// How the bots' former readers of a world (`Motion::find` and `Supplies::read`, byte searches over each player's
/// block) compare with the typed views the world check reads ([`world_validation::check_kept`]), which the bots read
/// since: what `srp-world-check --reread` counts. A place is counted when its block is in the world and the roster has
/// its profile; the body as `Game::observe_driven` read it (`Motion::find`), the supplies as `Game::observe` did
/// (`Supplies::read`).
#[derive(Clone, Debug, Default)]
pub struct Reread {
    pub worlds: usize,
    /// The distinct times of the recordings' uploads: the worlds the match thread could observe (it observes one
    /// upload of a time, the first that matches the majority).
    pub world_times: usize,
    pub places: usize,
    /// Places whose typed views give the body (the state read to its end with the kits).
    pub typed: usize,
    /// `Motion::find` gave no body where the typed views give one: the bots lost the place for that world.
    pub old_dropped: usize,
    /// `Motion::find` gave the body with no damage model (its search found no single place for it).
    pub old_no_damage: usize,
    /// Both gave a body, and they differ: by field.
    pub old_differs: BTreeMap<&'static str, usize>,
    /// Places where both gave a body and every field is equal.
    pub old_same: usize,
    /// `Motion::find` gave a body the typed views do not (the kits not known).
    pub typed_missing: usize,
    /// `Supplies::read` gave nothing for the place.
    pub supplies_old_none: usize,
    /// Both read the supplies, and they differ: by field, ` dead` added for a player the head gives dead.
    pub supplies_differ: BTreeMap<String, usize>,
    pub supplies_same: usize,
    /// The first examples, `p<place> t<time>: what`.
    pub shown: Vec<String>,
    /// The first examples of supplies that differ, alive and dead apart: `p<place> t<time>: old / typed`.
    pub supplies_shown: Vec<String>,
    /// Time of the check that keeps nothing (`world_validation::check`), summed over the worlds: each the fastest
    /// of [`REREAD_CALLS`] calls.
    pub check_time: Duration,
    /// The same for the check that keeps the typed views (`world_validation::check_kept`), what the server runs
    /// on every upload now.
    pub kept_time: Duration,
    /// The same for the former readers of a world, as the match thread ran them on each world it observed: the
    /// player blocks walked twice (`players::walk`, in `Game::observe` and `Game::observe_driven`), each place's
    /// `Supplies::read` and `Motion::find`.
    pub old_time: Duration,
}

impl Reread {
    pub fn add(&mut self, other: &Reread) {
        self.worlds += other.worlds;
        self.world_times += other.world_times;
        self.places += other.places;
        self.typed += other.typed;
        self.old_dropped += other.old_dropped;
        self.old_no_damage += other.old_no_damage;
        self.old_same += other.old_same;
        self.typed_missing += other.typed_missing;
        self.supplies_old_none += other.supplies_old_none;
        self.supplies_same += other.supplies_same;
        self.check_time += other.check_time;
        self.kept_time += other.kept_time;
        self.old_time += other.old_time;
        for (field, n) in &other.old_differs {
            *self.old_differs.entry(field).or_default() += n;
        }
        for (field, n) in &other.supplies_differ {
            *self.supplies_differ.entry(field.clone()).or_default() += n;
        }
        for line in &other.supplies_shown {
            if self.supplies_shown.len() < 2 * REREAD_SHOWN {
                self.supplies_shown.push(line.clone());
            }
        }
    }
}

/// The fields in which two bodies differ.
fn motion_fields(old: &Motion, typed: &Motion) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut field = |name, same: bool| {
        if !same {
            out.push(name);
        }
    };
    field("alive", old.alive == typed.alive);
    field("time", old.time_ms == typed.time_ms);
    field("rotation", old.rotation_y.to_bits() == typed.rotation_y.to_bits());
    field("pitch", old.look_pitch.to_bits() == typed.look_pitch.to_bits());
    field("keys", old.actions_mask == typed.actions_mask);
    field("inserted", old.inserted_time_ms == typed.inserted_time_ms);
    field("recent_hits", old.recent_hits == typed.recent_hits);
    field("body", old.body == typed.body);
    match (&old.damage, &typed.damage) {
        (Some(a), Some(b)) => {
            field("jump_type", a.jump_type == b.jump_type);
            field("speed", a.linear_horizontal_speed.to_bits() == b.linear_horizontal_speed.to_bits());
            field("last_hit", a.last_hit_time_ms == b.last_hit_time_ms);
            field("part_health", a.part_health == b.part_health);
            field("leg_wounds", a.leg_wounds == b.leg_wounds);
            field("part_speed", a.part_speed_modifiers == b.part_speed_modifiers);
        }
        (a, b) => field("damage", a.is_none() == b.is_none()),
    }
    out
}

/// The fields in which two readings of the supplies differ.
fn supplies_fields(old: &Supplies, typed: &Supplies) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut field = |name, same: bool| {
        if !same {
            out.push(name);
        }
    };
    let items = |f: &dyn Fn(&ItemLeft, &ItemLeft) -> bool| {
        old.items.len() == typed.items.len() && old.items.iter().zip(&typed.items).all(|(a, b)| f(a, b))
    };
    field("alive", old.alive == typed.alive);
    field("kit", old.kit == typed.kit);
    field("inventory_read", old.inventory_read == typed.inventory_read);
    field("stored", items(&|a, b| a.stored == b.stored));
    field("in_hand", items(&|a, b| a.in_hand == b.in_hand));
    field("condition", items(&|a, b| a.condition.map(f32::to_bits) == b.condition.map(f32::to_bits)));
    field("magazine", items(&|a, b| a.magazine == b.magazine));
    field("slot", items(&|a, b| a.slot == b.slot));
    field("hands_slot", old.hands_slot == typed.hands_slot);
    field("carried_weight", old.carried_weight.map(f32::to_bits) == typed.carried_weight.map(f32::to_bits));
    field("carrying", old.carrying == typed.carrying);
    out
}

/// Examples [`Reread`] keeps.
const REREAD_SHOWN: usize = 5;

/// One reading of the supplies, short: alive, inventory read, hands slot, carrying, carried weight, and each item
/// `slot:in_hand|condition|magazine`.
fn supplies_line(supplies: &Supplies) -> String {
    let items: Vec<String> = supplies
        .items
        .iter()
        .filter(|item| item.slot.is_some())
        .map(|item| format!("{}:{:?}|{:?}|{:?}", item.slot.unwrap_or(0), item.in_hand, item.condition, item.magazine))
        .collect();
    format!(
        "alive {} read {} hands {:?} carrying {:?} weight {:?} items [{}]",
        supplies.alive,
        supplies.inventory_read,
        supplies.hands_slot,
        supplies.carrying,
        supplies.carried_weight,
        items.join(" ")
    )
}

/// Calls of each reader a world's time is the fastest of.
const REREAD_CALLS: usize = 3;

/// The fastest of [`REREAD_CALLS`] calls of `f`.
fn fastest<T>(mut f: impl FnMut() -> T) -> Duration {
    (0..REREAD_CALLS)
        .map(|_| {
            let start = Instant::now();
            std::hint::black_box(f());
            start.elapsed()
        })
        .min()
        .unwrap_or_default()
}

/// The former readers of a world as the match thread ran them on an observed world.
fn old_readers(body: &[u8], time: u32, roster: Roster, dictionary: &Dictionary) -> usize {
    let mut read = 0;
    if let Some((blocks, _)) = players::walk(body) {
        for block in &blocks {
            if let Some(profile) = roster.profiles.get(usize::from(block.place)) {
                read += usize::from(Supplies::read(block, profile, dictionary).is_some());
            }
        }
    }
    if let Some((blocks, _)) = players::walk(body) {
        for block in &blocks {
            read += usize::from(Motion::find(block.state, time).is_some());
        }
    }
    read
}

impl Recording {
    /// Adds `body`, a world of this match at `time`, to `into` ([`Reread`]); `Err` for a world the check refuses.
    pub fn reread(&self, body: &[u8], time: u32, into: &mut Reread) -> Result<(), Unchecked> {
        let roster = self.roster().ok_or(Unchecked::NoOptions)?;
        let checked =
            world_validation::check_kept(body, time, self.places(), roster).map_err(Unchecked::Refused)?;
        into.worlds += 1;
        let dictionary = Dictionary::shared();
        into.check_time += fastest(|| world_validation::check(body, time, self.places(), roster).is_ok());
        into.kept_time += fastest(|| world_validation::check_kept(body, time, self.places(), roster).is_ok());
        into.old_time += fastest(|| old_readers(body, time, roster, dictionary));
        let Some((blocks, _)) = players::walk(body) else {
            return Ok(());
        };
        for block in blocks {
            let place = usize::from(block.place);
            let Some(profile) = roster.profiles.get(place) else {
                continue;
            };
            into.places += 1;
            let typed = checked.places.get(place).and_then(Option::as_ref);
            let typed_motion = typed.and_then(|typed| typed.motion.as_ref());
            let old = Motion::find(block.state, time).map(|(_, motion)| motion);
            let shown = &mut into.shown;
            let mut show = |what: String| {
                if shown.len() < REREAD_SHOWN {
                    shown.push(format!("p{place} t{time}: {what}"));
                }
            };
            match (&old, typed_motion) {
                (None, Some(_)) => {
                    into.typed += 1;
                    into.old_dropped += 1;
                    show("Motion::find read nothing".into());
                }
                (Some(old), Some(typed)) => {
                    into.typed += 1;
                    if old.damage.is_none() {
                        into.old_no_damage += 1;
                    }
                    let fields = motion_fields(old, typed);
                    if fields.is_empty() {
                        into.old_same += 1;
                    } else {
                        for field in &fields {
                            *into.old_differs.entry(field).or_default() += 1;
                        }
                        show(format!("body differs in {fields:?}"));
                    }
                }
                (Some(_), None) => into.typed_missing += 1,
                (None, None) => {}
            }
            match (Supplies::read(&block, profile, dictionary), typed.map(|typed| &typed.supplies)) {
                (None, _) => into.supplies_old_none += 1,
                (Some(old), Some(typed)) => {
                    let fields = supplies_fields(&old, typed);
                    if fields.is_empty() {
                        into.supplies_same += 1;
                    } else {
                        let dead = if typed.alive { "" } else { " dead" };
                        for field in &fields {
                            *into.supplies_differ.entry(format!("{field}{dead}")).or_default() += 1;
                        }
                        // Alive and dead examples apart: at most REREAD_SHOWN of each.
                        let kind = into.supplies_shown.iter().filter(|line| line.contains("alive true") == typed.alive);
                        if kind.count() < REREAD_SHOWN {
                            into.supplies_shown.push(format!(
                                "p{place} t{time}: {fields:?}: old {} / typed {}",
                                supplies_line(&old),
                                supplies_line(typed)
                            ));
                        }
                    }
                }
                (Some(_), None) => {}
            }
        }
        Ok(())
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
