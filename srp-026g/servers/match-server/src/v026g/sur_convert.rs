//! A match recording ([`super::record`]) made into a `.sur` replay
//! ([`super::sur`]): by `srp-recording-to-sur`, and by the server itself for
//! the lobby's "Watch replay" ([`super::replay`], on a thread of its own,
//! from the file as it is read: [`convert_file`]).
//!
//! What goes where:
//! - the options (chunk 1): the stream of a 0xE4 the server sent, without
//!   its first byte, from the place that got the largest roster;
//! - the events (chunk 2): what the server relayed to the players (0xEB,
//!   0xEC, 0xED, 0xEE, 0xF3, 0xE9, 0xEA), every message of every place put
//!   together, a copy of an event dropped. Its tick byte is read against the
//!   match clock the recording's own times give, at the time the server sent
//!   the event: the first copy of the same message to any place (a copy to a
//!   place whose link is backed up goes out seconds later, and a tick byte
//!   read more than 6.35 s from its own time is read as another tick); an
//!   event the clock still puts in the future is dropped;
//! - the key frames (chunk 3): the worlds the clients uploaded (0xE0
//!   chunks), the same upload of several clients as one when they agree,
//!   thinned to what the client can hold;
//! - the config (chunk 4): given by the caller.
//!
//! Every choice is listed in [`Report`].

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::item_params::item_params;
use survarium::v026g::lzma2;
use survarium::v026g::maps;
use survarium::v026g::match_msgs::snapshot;
use survarium::v026g::match_msgs::static_info;
use survarium::v026g::match_msgs::upload::{Chunk, Pushed, Reassembler};
use survarium::v026g::match_msgs::{
    ClientUpdate, GameTime, Grid, Update, mask_bytes, tick_time,
};
use sync_core::confirm::{Action, OwnInputs};
use sync_core::wire::Input;
use udp_transport::table::{CLIENT_UPDATE, Direction, SERVER_UPDATE, Table};
use udp_transport::wire;

use super::record::{self, IN, NOTE, OUT, Record, RecordRef};
use super::sur::{
    self, Coding, Event, InputValue, KeyFrame, MAX_KEY_FRAMES, MAX_PLAYERS, Parts, TICK_MS, Writer,
};

/// An event the clock puts this far ahead of the recording is dropped: the
/// server itself refuses inputs more than 2.5 s ahead.
const FUTURE_SLACK_MS: u32 = 1_000;
/// A copy of a relay is a sign that its place's link is backed up when no
/// other place got the same message (id and payload) within `BEHIND_MS`
/// before it (or `NEAR_AFTER_MS` after), but another place got it within
/// `LOOKBACK_MS` before. The server sends an event to every other place at
/// once, so on sound links the copies of one event are a few hundred ms
/// apart (a resend: under 1.1 s in every sample). The same bytes come back
/// 12.8 s later at the earliest; a copy that meets them then is read as
/// that later event, which has the same value: a copy of it.
const BEHIND_MS: u32 = 2_000;
const NEAR_AFTER_MS: u32 = 1_000;
const LOOKBACK_MS: u32 = 60_000;
/// Every copy a place gets this long before or after a sign that its link
/// is backed up is dropped: the copies of a backlog more than 6 s old are
/// read as other ticks, and match nothing in the window. The other places
/// got the same events on time.
const LAG_HOLD_MS: u32 = 15_000;
/// Bit 31 of a relayed actions mask is a flag the client splits off before
/// the input goes to the world (0xEB/0xEC 0x318479, 0xED/0xEE 0x27C501);
/// the replay passes the record's u64 as it is with the flag 0 (0x1EE2A1),
/// so left in, the bit would be an action.
const INPUT_FLAG: u32 = 1 << 31;
/// The least gap between two key frames, unless the caller gives one.
const DEFAULT_KEY_FRAME_EVERY_MS: u32 = 10_000;
/// The world kept for a late joiner is the first one this long after its
/// entry: the item must be later than the entry's tick.
const LATE_JOINER_WORLD_AFTER_MS: u32 = 300;
/// How long before its entry a late joiner's install is looked for when
/// no 0xE4 with its place was seen.
const INSTALL_WINDOW_MS: u32 = 60_000;
/// A late joiner's install is looked for from this long before the first
/// 0xE4 that had its place (the recording's clock less the offset).
const OFFER_SLACK_MS: u32 = 1_000;
/// A world is the one before it with zero bytes put in front when, past
/// those bytes, at least this many bytes (or the whole of the world before)
/// agree with the world before, and more than without them.
const MIN_SHIFTED_PREFIX: usize = 8;

/// What the caller chooses.
#[derive(Clone, Debug)]
pub struct Settings {
    pub coding: Coding,
    /// The least gap between two key frames; larger if 128 would not do.
    pub key_frame_every_ms: u32,
    /// Key frames at these times are left out (for finding what the
    /// client chokes on).
    pub drop_key_frames: Vec<u32>,
    /// Only the first world as a key frame (the replay is never put
    /// right).
    pub first_key_frame_only: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            coding: Coding::Packed,
            key_frame_every_ms: DEFAULT_KEY_FRAME_EVERY_MS,
            drop_key_frames: Vec::new(),
            first_key_frame_only: false,
        }
    }
}

/// What the conversion found and chose.
#[derive(Debug, Default)]
pub struct Report {
    pub match_id: String,
    pub map: String,
    pub mode: u64,
    pub places: usize,
    /// Recording ms less match ms.
    pub clock_offset_ms: i64,
    pub datagrams_out: usize,
    pub datagrams_in: usize,
    pub unparsed: usize,
    /// What the transport parser said, by direction, for those.
    pub unparsed_why: BTreeMap<String, usize>,
    pub inputs: usize,
    /// The per-tick protocol: the server's updates (0xF7) and the places'
    /// own (0xE2) read, copies once.
    pub updates: usize,
    pub own_updates: usize,
    pub input_copies: usize,
    pub input_conflicts: usize,
    pub dropped_future: usize,
    /// Relay copies dropped because their place's link was backed up, by
    /// place; and the messages none of whose copies was kept.
    pub lagging_copies: BTreeMap<u8, usize>,
    pub messages_lost: usize,
    /// Inputs whose actions mask had the flag bit (cleared).
    pub flagged_inputs: usize,
    /// The match let players in late: the masks of 0xED/0xEE are of 20.
    pub growing_roster: bool,
    pub profiles: usize,
    pub entered: usize,
    pub left: usize,
    pub roster: u8,
    /// Places that entered beyond the roster the first 0xE4 had.
    pub late_joiners: usize,
    /// `(place, world time, objects)` of each late place with grenades or
    /// traps: how many its kits have (one game object each) and the first
    /// world that has them.
    pub installs: Vec<(u8, u32, usize)>,
    /// Late places whose install was not found in the worlds, or not for
    /// sure: the key frames from their first 0xE4 to their entry are left
    /// out.
    pub installs_unsure: Vec<u8>,
    /// Installs whose world is exactly the one before with zero bytes in
    /// front; each agrees with the count of its place(s).
    pub installs_exact: usize,
    /// Key frames left out because a joiner's install was not found.
    pub dropped_unsure_key_frames: usize,
    /// Key frames given zero bytes for joiners not installed yet.
    pub padded_key_frames: usize,
    pub worlds_complete: usize,
    pub world_times: usize,
    pub worlds_disagreeing: usize,
    pub first_world_ms: Option<u32>,
    pub start_ms: u32,
    pub end_ms: u32,
    pub key_frames: Vec<u32>,
    /// The size of each key frame's parts: players, bullets, statistics,
    /// objects, physics, rules, effects (`None`: the world did not walk).
    pub key_frame_parts: Vec<(u32, Option<[usize; 7]>)>,
    /// `(time, 'E' entered / 'L' left, place)`.
    pub roster_events: Vec<(u32, char, u8)>,
    /// Inputs in each minute of the match.
    pub inputs_per_minute: Vec<usize>,
    pub body_bytes: usize,
    pub decoded_bytes: usize,
    pub notes: Vec<String>,
}

/// Why a recording cannot be converted.
#[derive(Debug)]
pub enum ConvertError {
    /// The recording file could not be read ([`convert_file`]).
    Read(std::io::Error),
    NoHeader,
    NoOptions,
    NoWorlds,
    /// A match on another tick than the game's 50 ms (the per-tick
    /// protocol's `SRP_NETCODE_TICK_MS`): the game's replay player reads on
    /// the 50 ms grid (`replay_sur.md`), so no replay is made of it.
    Tick(u64),
    Sur(sur::SurError),
    /// The file made fails its own check: a bug.
    Verify(String),
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(error) => write!(f, "the recording: {error}"),
            Self::NoHeader => write!(f, "no header note"),
            Self::NoOptions => write!(f, "no 0xE4 (static info) in the recording"),
            Self::NoWorlds => write!(f, "no complete uploaded world in the recording"),
            Self::Tick(ms) => write!(
                f,
                "a match of a {ms} ms tick: the game's replay player is on 50 ms"
            ),
            Self::Sur(error) => write!(f, "{error}"),
            Self::Verify(why) => write!(f, "the file made does not check out: {why}"),
        }
    }
}

impl std::error::Error for ConvertError {}

impl From<sur::SurError> for ConvertError {
    fn from(error: sur::SurError) -> Self {
        Self::Sur(error)
    }
}

/// The name of the recording's mode config under `gameplay/game_modes/`
/// (`level_01_day_2.options`): the `maps_dict` row of its map id.
pub fn config_name(records: &[Record]) -> Option<String> {
    let id = u8::try_from(header(records)?.get("map_id")?.as_u64()?).ok()?;
    maps::map_mode(id).map(|row| row.config.to_string())
}

fn header(records: &[Record]) -> Option<serde_json::Value> {
    records
        .iter()
        .find(|r| r.kind == NOTE)
        .and_then(|r| serde_json::from_slice(&r.bytes).ok())
}

/// recording ms less match ms: the median over every note with both.
fn clock_offset(records: &[Record]) -> Option<i64> {
    let mut offsets: Vec<i64> = records
        .iter()
        .filter(|r| r.kind == NOTE)
        .filter_map(|r| {
            let note: serde_json::Value = serde_json::from_slice(&r.bytes).ok()?;
            let match_ms = note.get("match_ms")?.as_u64()?;
            Some(i64::from(r.at_ms) - match_ms as i64)
        })
        .collect();
    offsets.sort_unstable();
    offsets.get(offsets.len() / 2).copied()
}

/// The world of an item time that most uploads agree on.
fn majority(bodies: &[Vec<u8>]) -> (usize, usize) {
    let mut counts: Vec<(usize, usize)> = Vec::new();
    for (i, body) in bodies.iter().enumerate() {
        match counts.iter_mut().find(|(first, _)| bodies[*first] == *body) {
            Some((_, n)) => *n += 1,
            None => counts.push((i, 1)),
        }
    }
    let best = counts
        .iter()
        .max_by_key(|(_, n)| *n)
        .copied()
        .unwrap_or((0, 0));
    (best.0, counts.len())
}

#[derive(Default)]
struct PlaceState {
    /// The roster of the 0xE4 this place got.
    roster: u8,
    /// 0xE4 parts by unique id: (count, parts).
    options: HashMap<u8, (u8, BTreeMap<u8, Vec<u8>>)>,
    uploads: Reassembler,
}

/// Makes the `.sur` of a recording. `config` is the mode's `.options` file.
pub fn convert(
    records: &[Record],
    config: &[u8],
    settings: Settings,
) -> Result<(Vec<u8>, Report), ConvertError> {
    let mut gather = Gather::new(records)?;
    for record in records {
        gather.push(record.borrowed());
    }
    gather.finish(config, settings)
}

/// [`convert`] of a recording file read as it goes ([`record::read_each`]):
/// the datagrams are never all in memory, only what is kept of them (the
/// relays once each, the uploaded worlds, the options). `notes` are the
/// file's notes ([`record::read_notes`]: the header, the clock).
///
/// Measured on the largest recording of 2026-10-14 (13.5 MB, 1.36 million
/// datagrams, 20 places): [`convert`] of the file read whole peaks at
/// 431 MiB, this at less than half of that (`docs/notes/replay_watch.md`).
pub fn convert_file(
    path: &Path,
    notes: &[Record],
    config: &[u8],
    settings: Settings,
) -> Result<(Vec<u8>, Report), ConvertError> {
    let mut gather = Gather::new(notes)?;
    record::read_each(path, |record| gather.push(record)).map_err(ConvertError::Read)?;
    gather.finish(config, settings)
}

/// A list that grows a piece at a time: it is never copied whole, as a
/// `Vec` is when it outgrows its room (a 20-place match relays 4.4 million
/// copies; the copy would hold the old and the new list at once).
struct Pieces<T> {
    pieces: Vec<Vec<T>>,
}

impl<T> Default for Pieces<T> {
    fn default() -> Self {
        Self { pieces: Vec::new() }
    }
}

impl<T> Pieces<T> {
    const PIECE: usize = 1 << 16;

    fn push(&mut self, item: T) {
        match self.pieces.last_mut() {
            Some(piece) if piece.len() < Self::PIECE => piece.push(item),
            _ => {
                let mut piece = Vec::with_capacity(Self::PIECE);
                piece.push(item);
                self.pieces.push(piece);
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = &T> {
        self.pieces.iter().flatten()
    }
}

/// What [`convert`] keeps of the datagrams as it reads them, in order.
struct Gather {
    report: Report,
    /// Recording ms less match ms.
    offset: i64,
    places: HashMap<u8, PlaceState>,
    /// The relays, by message: (id, mask bytes, payload) -> id in `sent`;
    /// when and to which place each copy went, to read them once it is
    /// known which places' links were backed up.
    messages: HashMap<(u8, usize, Vec<u8>), usize>,
    /// The copies of each message.
    counts: Vec<u32>,
    /// Every copy in the recording's order: `(ms, place, message)`.
    copies: Pieces<(u32, u8, u32)>,
    worlds: BTreeMap<u32, Vec<Vec<u8>>>,
    /// The 0xE4 streams (without the index byte), by roster.
    options: BTreeMap<u8, Vec<u8>>,
    /// The options of each roster, and when a 0xE4 of it first went out.
    rosters: BTreeMap<u8, (u32, static_info::MatchOptions)>,
    /// A match of the per-tick protocol: its updates.
    ticks: TickRecords,
}

/// What is read of a match of the per-tick protocol (`game/ticks.rs`):
/// every time is a u32 there, so nothing is read against a clock. The
/// others' inputs and the roster changes come in the server's updates
/// (0xF7); a place's own inputs in its own updates (0xE2), put where the
/// outcomes the server sent it say (the place's own game does the same,
/// `sync_core::confirm`): so a match of one player against the server's
/// bots has the player's inputs too.
#[derive(Default)]
struct TickRecords {
    /// From the 0xE4's per-tick trailer.
    grid: Option<Grid>,
    /// The parts of the 0xF7s, by place and unique id.
    parts: HashMap<(u8, u8), Assembly>,
    own: HashMap<u8, OwnRecords>,
    inputs: BTreeMap<(u32, u8), InputValue>,
    profiles: BTreeMap<(u32, u8), u8>,
    entered: BTreeMap<(u32, u8), ()>,
    left: BTreeMap<(u32, u8), ()>,
}

/// A split message's parts: how many, and those come, by index.
type Assembly = (u8, BTreeMap<u8, Vec<u8>>);

/// A place's own updates and what became of them.
#[derive(Default)]
struct OwnRecords {
    book: OwnInputs,
    seen: HashSet<u32>,
    inputs: BTreeMap<u32, InputValue>,
    profiles: BTreeMap<u32, u8>,
}

impl TickRecords {
    /// A place's own update (0xE2), once.
    fn own_update(&mut self, place: u8, payload: &[u8]) -> bool {
        let Some(grid) = self.grid else {
            return false;
        };
        let Ok(update) = ClientUpdate::parse(payload, grid) else {
            return false;
        };
        let own = self.own.entry(place).or_default();
        if !own.seen.insert(update.tick.0) {
            return false;
        }
        own.book.sent(&update);
        for input in update.inputs() {
            own.inputs
                .insert(input.time(update.tick).0, input_value(input));
        }
        for value in update.profiles() {
            own.profiles.insert(update.tick.0, *value);
        }
        true
    }

    /// The server's update to `place` (0xF7), whole.
    fn server_update(&mut self, place: u8, payload: &[u8]) -> bool {
        let Some(grid) = self.grid else {
            return false;
        };
        let Ok(update) = Update::parse(payload, grid) else {
            return false;
        };
        let tick = update.tick.0;
        for each in &update.inputs {
            let time = each.input.time(update.tick).0;
            self.inputs
                .entry((time, each.place.get()))
                .or_insert_with(|| input_value(&each.input));
        }
        for each in &update.profiles {
            self.profiles
                .entry((tick, each.place.get()))
                .or_insert(each.value);
        }
        for who in update.entered.iter() {
            self.entered.insert((tick, who.get()), ());
        }
        for who in update.left.iter() {
            self.left.insert((tick, who.get()), ());
        }
        let own = self.own.entry(place).or_default();
        for outcome in &update.outcomes {
            let mut actions = Vec::new();
            own.book
                .resolve(*outcome, GameTime(0), &mut |action| actions.push(action));
            for action in actions {
                match action {
                    Action::DiscardInput { time } => {
                        own.inputs.remove(&time.0);
                    }
                    Action::DiscardProfile { time } => {
                        own.profiles.remove(&time.0);
                    }
                    Action::MoveInput { from, to } => {
                        if let Some(value) = own.inputs.remove(&from.0) {
                            own.inputs.insert(to.0, value);
                        }
                    }
                    Action::MoveProfile { from, to } => {
                        if let Some(value) = own.profiles.remove(&from.0) {
                            own.profiles.insert(to.0, value);
                        }
                    }
                    Action::InsertInput { time, input } => {
                        own.inputs.insert(time.0, input_value(&input));
                    }
                    Action::InsertProfile { time, value } => {
                        own.profiles.insert(time.0, value);
                    }
                }
            }
        }
        true
    }
}

fn input_value(input: &Input) -> InputValue {
    InputValue {
        rotation: input.rotation,
        actions: input.actions & !INPUT_FLAG,
    }
}

impl Gather {
    /// `notes`: the recording's notes, or all of its records.
    fn new(notes: &[Record]) -> Result<Self, ConvertError> {
        let head = header(notes).ok_or(ConvertError::NoHeader)?;
        // A recording from before the key is of the game's own 50 ms.
        let tick_ms = head["tick_ms"].as_u64().unwrap_or(u64::from(TICK_MS));
        if tick_ms != u64::from(TICK_MS) {
            return Err(ConvertError::Tick(tick_ms));
        }
        let mut report = Report {
            match_id: head["match_id"].as_str().unwrap_or("?").to_string(),
            map: head["map"].as_str().unwrap_or("?").to_string(),
            mode: head["mode"].as_u64().unwrap_or(0),
            places: head["places"].as_array().map_or(0, Vec::len),
            growing_roster: head["growing_roster"].as_bool().unwrap_or(false),
            ..Report::default()
        };
        let offset = clock_offset(notes).unwrap_or(0);
        report.clock_offset_ms = offset;
        Ok(Self {
            report,
            offset,
            places: HashMap::new(),
            messages: HashMap::new(),
            counts: Vec::new(),
            copies: Pieces::default(),
            worlds: BTreeMap::new(),
            options: BTreeMap::new(),
            rosters: BTreeMap::new(),
            ticks: TickRecords::default(),
        })
    }

    /// One record of the recording, in order.
    fn push(&mut self, record: RecordRef<'_>) {
        if record.kind != IN && record.kind != OUT {
            return;
        }
        let dictionary = Dictionary::shared();
        let report = &mut self.report;
        let place = self.places.entry(record.place).or_default();
        if record.kind == IN {
            report.datagrams_in += 1;
            // The per-tick protocol's table is the stock one and 0xE2, 0xF7.
            let Ok(datagram) =
                wire::parse(Table::MatchTicks, Direction::ToServer, record.bytes, 0)
            else {
                report.unparsed += 1;
                return;
            };
            for message in &datagram.messages {
                if message.id == CLIENT_UPDATE {
                    if self.ticks.own_update(record.place, message.payload) {
                        report.own_updates += 1;
                    }
                    continue;
                }
                if message.id != wire::UPLOAD_CARRIER
                    || message.part.is_some()
                    || message.payload.is_empty()
                {
                    continue;
                }
                let Ok(chunk) = Chunk::parse(message.payload) else {
                    continue;
                };
                if let Pushed::Complete(upload) = place.uploads.push(&chunk) {
                    report.worlds_complete += 1;
                    self.worlds
                        .entry(upload.item_time)
                        .or_default()
                        .push(upload.body);
                }
            }
            return;
        }
        report.datagrams_out += 1;
        // A match that lets players in late sends its masks for 20 from
        // the start (`Game::snapshot_players`).
        let mask_len = if report.growing_roster {
            mask_bytes(MAX_PLAYERS as u8)
        } else if place.roster == 0 {
            0
        } else {
            mask_bytes(place.roster)
        };
        let parsed = wire::parse(Table::MatchTicks, Direction::ToClient, record.bytes, mask_len);
        let datagram = match parsed {
            Ok(datagram) => datagram,
            Err(error) => {
                report.unparsed += 1;
                *report.unparsed_why.entry(format!("{error:?}")).or_default() += 1;
                return;
            }
        };
        for message in &datagram.messages {
            let payload = message.payload;
            match message.id {
                0xE4 => {
                    let Some(x) = message.x else { continue };
                    let (count, parts) = place.options.entry(x).or_insert((1, BTreeMap::new()));
                    match message.part {
                        Some(part) => {
                            if let Some(n) = part.count {
                                *count = n;
                            }
                            parts.insert(part.index, payload.to_vec());
                            if parts.len() < usize::from(*count) {
                                continue;
                            }
                        }
                        None => {
                            parts.insert(0, payload.to_vec());
                        }
                    }
                    let whole: Vec<u8> = parts.values().flatten().copied().collect();
                    place.options.remove(&x);
                    // The per-tick protocol's trailer: the match's grid, and
                    // no part of the options' stream.
                    let Ok((whole, grid)) = static_info::split_ticks_trailer(&whole) else {
                        continue;
                    };
                    if let Some(grid) = grid {
                        self.ticks.grid.get_or_insert(grid);
                    }
                    if let Ok((_, parsed)) = static_info::parse_payload(whole, dictionary) {
                        let roster = parsed.players.len() as u8;
                        place.roster = roster;
                        self.options
                            .entry(roster)
                            .or_insert_with(|| whole[1..].to_vec());
                        let clock = (i64::from(record.at_ms) - self.offset).max(0) as u32;
                        let first = self.rosters.entry(roster).or_insert((clock, parsed));
                        first.0 = first.0.min(clock);
                    }
                    continue;
                }
                // The per-tick protocol's update, whole: read as it comes.
                SERVER_UPDATE => {
                    let Some(x) = message.x else { continue };
                    let key = (record.place, x);
                    let (count, parts) = self.ticks.parts.entry(key).or_insert((1, BTreeMap::new()));
                    match message.part {
                        Some(part) => {
                            if let Some(n) = part.count {
                                *count = n;
                            }
                            parts.insert(part.index, payload.to_vec());
                            if parts.len() < usize::from(*count) {
                                continue;
                            }
                        }
                        None => {
                            parts.insert(0, payload.to_vec());
                        }
                    }
                    let whole: Vec<u8> = parts.values().flatten().copied().collect();
                    self.ticks.parts.remove(&key);
                    if self.ticks.server_update(record.place, &whole) {
                        report.updates += 1;
                    }
                    continue;
                }
                // A 0xEA to the leaver itself is its farewell (match.md: the
                // server closes the game with it, at tick 0 when the game
                // never loaded); the others' copies are the leave.
                0xEA if payload.len() == 2 && payload[1] == record.place => continue,
                // The relays, read below.
                0xE9 | 0xEA if payload.len() == 2 => {}
                0xEB if payload.len() == 10 => {}
                0xEC if payload.len() == 11 => {}
                0xED | 0xEE => {}
                0xF3 if payload.len() == 3 => {}
                _ => continue,
            }
            let next = self.counts.len();
            let id = *self
                .messages
                .entry((message.id, mask_len, payload.to_vec()))
                .or_insert(next);
            if id == next {
                self.counts.push(0);
            }
            self.counts[id] += 1;
            self.copies.push((record.at_ms, record.place, id as u32));
        }
    }

    /// The file, from what was gathered.
    fn finish(self, config: &[u8], settings: Settings) -> Result<(Vec<u8>, Report), ConvertError> {
        finish(self, config, settings)
    }
}

/// The second half of [`convert`]: the relays read, the events, the key
/// frames, the file.
fn finish(
    gathered: Gather,
    config: &[u8],
    settings: Settings,
) -> Result<(Vec<u8>, Report), ConvertError> {
    let Gather {
        mut report,
        offset,
        places,
        messages,
        counts,
        copies,
        worlds,
        options,
        rosters,
        ticks,
    } = gathered;
    // The places' half-read uploads and options are of no use now.
    drop(places);
    // When and to which place each copy of each message went: message
    // `id`'s are `sent[starts[id]..starts[id + 1]]`, in time order.
    let mut starts = Vec::with_capacity(counts.len() + 1);
    let mut total = 0usize;
    for &count in &counts {
        starts.push(total);
        total += count as usize;
    }
    starts.push(total);
    let mut sent = vec![(0u32, 0u8); total];
    {
        let mut fill = starts.clone();
        for &(at_ms, place, id) in copies.iter() {
            let at = &mut fill[id as usize];
            sent[*at] = (at_ms, place);
            *at += 1;
        }
    }
    for id in 0..counts.len() {
        sent[starts[id]..starts[id + 1]].sort_unstable();
    }
    let times_of = |id: usize| &sent[starts[id]..starts[id + 1]];
    // Events by (time, player), the first copy kept.
    let mut inputs: BTreeMap<(u32, u8), InputValue> = BTreeMap::new();
    let mut profiles: BTreeMap<(u32, u8), u8> = BTreeMap::new();
    let mut entered: BTreeMap<(u32, u8), ()> = BTreeMap::new();
    let mut left: BTreeMap<(u32, u8), ()> = BTreeMap::new();

    // The signs of a backed-up link: a copy well behind the same message
    // to another place.
    let mut by_id: Vec<(u8, usize, &[u8])> = vec![(0, 0, &[]); counts.len()];
    for ((message_id, mask_len, payload), &id) in &messages {
        by_id[id] = (*message_id, *mask_len, payload);
    }
    let mut behind: BTreeMap<u8, Vec<u32>> = BTreeMap::new();
    for id in 0..counts.len() {
        let times = times_of(id);
        for &(at_ms, place) in times {
            let from = times.partition_point(|&(t, _)| t + LOOKBACK_MS < at_ms);
            let to = times.partition_point(|&(t, _)| t <= at_ms + NEAR_AFTER_MS);
            let others = || times[from..to].iter().filter(|&&(_, other)| other != place);
            if others().any(|&(t, _)| t + BEHIND_MS < at_ms)
                && !others().any(|&(t, _)| t + BEHIND_MS >= at_ms)
            {
                behind.entry(place).or_default().push(at_ms);
            }
        }
    }
    for signs in behind.values_mut() {
        signs.sort_unstable();
    }
    // The copies by message are read; what follows takes them in order.
    drop(sent);
    drop(starts);
    let lagging = |place: u8, at_ms: u32| {
        behind.get(&place).is_some_and(|signs| {
            let next = signs.partition_point(|&t| t + LAG_HOLD_MS < at_ms);
            signs.get(next).is_some_and(|&t| t <= at_ms + LAG_HOLD_MS)
        })
    };
    let mut kept = vec![false; counts.len()];
    for &(at_ms, place, id) in copies.iter() {
        let id = id as usize;
        if lagging(place, at_ms) {
            *report.lagging_copies.entry(place).or_default() += 1;
            continue;
        }
        kept[id] = true;
        let clock = (i64::from(at_ms) - offset).max(0) as u32;
        let (message_id, mask_len, payload) = by_id[id];
        match message_id {
            0xE9 | 0xEA => {
                let Some(time) = resolve(payload[0], 0, clock, &mut report) else {
                    continue;
                };
                let key = (time, payload[1]);
                if message_id == 0xE9 {
                    entered.entry(key).or_insert(());
                } else {
                    left.entry(key).or_insert(());
                }
            }
            0xEB => {
                let Some(time) = resolve(payload[0], 0, clock, &mut report) else {
                    continue;
                };
                keep_input(&mut inputs, &mut report, time, payload[1], &payload[2..10]);
            }
            0xEC => {
                let Some(time) = resolve(payload[0], payload[1], clock, &mut report) else {
                    continue;
                };
                keep_input(&mut inputs, &mut report, time, payload[2], &payload[3..11]);
            }
            0xED | 0xEE => {
                let precise = message_id == 0xEE;
                let head = if precise { 2 } else { 1 };
                if payload.len() < head + mask_len {
                    continue;
                }
                let ms = if precise { payload[1] } else { 0 };
                let Some(time) = resolve(payload[0], ms, clock, &mut report) else {
                    continue;
                };
                let mask = &payload[head..head + mask_len];
                let players: Vec<u8> = (0..(mask_len * 8) as u8)
                    .filter(|p| mask[usize::from(p / 8)] & (1 << (p % 8)) != 0)
                    .collect();
                let n = players.len();
                let rest = &payload[head + mask_len..];
                if rest.len() < 8 * n {
                    continue;
                }
                let (actions, rest) = rest.split_at(4 * n);
                let (rot_x, rot_y) = rest.split_at(2 * n);
                for (i, &player) in players.iter().enumerate() {
                    let mut bytes = [0u8; 8];
                    bytes[0..2].copy_from_slice(&rot_x[2 * i..2 * i + 2]);
                    bytes[2..4].copy_from_slice(&rot_y[2 * i..2 * i + 2]);
                    bytes[4..8].copy_from_slice(&actions[4 * i..4 * i + 4]);
                    keep_input(&mut inputs, &mut report, time, player, &bytes);
                }
            }
            0xF3 => {
                let Some(time) = resolve(payload[0], 0, clock, &mut report) else {
                    continue;
                };
                profiles.entry((time, payload[1])).or_insert(payload[2]);
            }
            _ => unreachable!("only relays are kept"),
        }
    }
    drop(copies);
    report.messages_lost = kept.iter().filter(|kept| !**kept).count();
    // The 0xE4's tick, when the header has none.
    if let Some(grid) = ticks.grid.filter(|grid| grid.tick_ms() != TICK_MS) {
        return Err(ConvertError::Tick(grid.tick_ms().into()));
    }
    // The per-tick protocol's events: the others' as the server sent them,
    // each place's own as its outcomes left them.
    let TickRecords {
        inputs: tick_inputs,
        profiles: tick_profiles,
        entered: tick_entered,
        left: tick_left,
        own,
        ..
    } = ticks;
    for ((time, player), value) in tick_inputs {
        keep_value(&mut inputs, &mut report, time, player, value);
    }
    for (place, own) in own {
        for (time, value) in own.inputs {
            keep_value(&mut inputs, &mut report, time, place, value);
        }
        for (time, value) in own.profiles {
            profiles.entry((time, place)).or_insert(value);
        }
    }
    for (key, value) in tick_profiles {
        profiles.entry(key).or_insert(value);
    }
    entered.extend(tick_entered);
    left.extend(tick_left);

    report.inputs = inputs.len();
    report.profiles = profiles.len();
    report.entered = entered.len();
    report.left = left.len();
    report.world_times = worlds.len();
    for &(time, player) in entered.keys() {
        report.roster_events.push((time, 'E', player));
    }
    for &(time, player) in left.keys() {
        report.roster_events.push((time, 'L', player));
    }
    report.roster_events.sort_unstable();
    for &(time, _) in inputs.keys() {
        let minute = (time / 60_000) as usize;
        if report.inputs_per_minute.len() <= minute {
            report.inputs_per_minute.resize(minute + 1, 0);
        }
        report.inputs_per_minute[minute] += 1;
    }

    let (&roster, options_stream) = options.iter().next_back().ok_or(ConvertError::NoOptions)?;
    report.roster = roster;
    let options_decoded = lzma2::unpack(options_stream, 0x578A0)
        .map(|bytes| bytes.len())
        .unwrap_or(0);
    if options.len() > 1 {
        report.notes.push(format!(
            "the roster was {:?} in the 0xE4s the places got; used {roster}",
            options.keys().collect::<Vec<_>>()
        ));
    }

    // One world for each time.
    let mut chosen: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    for (time, bodies) in &worlds {
        let (best, kinds) = majority(bodies);
        if kinds > 1 {
            report.worlds_disagreeing += 1;
        }
        chosen.insert(*time, bodies[best].clone());
    }
    let first_world = *chosen.keys().next().ok_or(ConvertError::NoWorlds)?;
    report.first_world_ms = Some(first_world);

    // Events by time.
    let mut events: BTreeMap<u32, Event> = BTreeMap::new();
    for (&(time, player), value) in &inputs {
        events
            .entry(time)
            .or_insert_with(|| Event::at(time))
            .inputs
            .push((player, *value));
    }
    for (&(time, player), &value) in &profiles {
        events
            .entry(time)
            .or_insert_with(|| Event::at(time))
            .profiles
            .push((player, value));
    }
    for &(time, player) in entered.keys() {
        events.entry(time).or_insert_with(|| Event::at(time)).enter |= 1 << player;
    }
    for &(time, player) in left.keys() {
        events.entry(time).or_insert_with(|| Event::at(time)).leave |= 1 << player;
    }

    // The replay starts with the first world (50 ms in every sample, the
    // time a fresh world core has, 0x2D0765), or with an event before it.
    let start = first_world.min(events.keys().next().copied().unwrap_or(first_world));
    report.start_ms = start;
    let last_event = events.keys().next_back().copied().unwrap_or(start);
    let last_world = *chosen.keys().next_back().expect("a world");
    let end = last_event.max(last_world);
    report.end_ms = end;

    let mut writer = Writer::new(start);
    for event in events.values() {
        // An input of a player outside the roster the client knows.
        let mut event = event.clone();
        event
            .inputs
            .retain(|&(player, _)| usize::from(player) < MAX_PLAYERS);
        writer.push(&event)?;
    }
    let body = writer.finish(end);
    report.body_bytes = body.len();
    let records_read = sur::read_body(&body).expect("the writer's own body reads back");

    // Key frames: the first world; one soon after each late joiner enters
    // (the client has no late join to run in a replay, so the world that
    // holds the newcomer is what puts it there); and one at least every
    // `every` ms in between, the gap raised until at most 128 do.
    let initial = options.keys().next().copied().unwrap_or(roster);
    let mut must: Vec<u32> = entered
        .keys()
        .filter(|&&(_, player)| player >= initial)
        .filter_map(|&(time, _)| {
            chosen
                .range(time + LATE_JOINER_WORLD_AFTER_MS..)
                .next()
                .map(|(&t, _)| t)
        })
        .collect();
    must.sort_unstable();
    must.dedup();
    report.late_joiners = entered.keys().filter(|&&(_, p)| p >= initial).count();
    let span = last_world.saturating_sub(first_world);
    let room = (MAX_KEY_FRAMES - 2).saturating_sub(must.len()).max(1) as u32;
    let every = settings.key_frame_every_ms.max(span / room);
    let mut times = vec![first_world];
    for &time in chosen.keys() {
        if time >= times[times.len() - 1] + every {
            times.push(time);
        }
    }
    times.extend(&must);
    times.sort_unstable();
    times.dedup();
    times.truncate(MAX_KEY_FRAMES);
    times.retain(|time| !settings.drop_key_frames.contains(time));
    if settings.first_key_frame_only {
        times.truncate(1);
    }
    // The viewer binds every place of chunk 1 at its start, so its list of
    // game objects holds each late joiner's grenades and traps from the
    // first tick; the recorded games got them only at the install, as zero
    // bytes put in front of the saved objects (client_patch.md 3.14). A
    // world from before an install is given those bytes here, or every
    // object of the viewer reads out of step (docs/notes/replay_sur.md).
    let joiners = late_places(&rosters, &entered, initial, &mut report);
    let (installs, unsure) = find_installs(&chosen, &joiners, &mut report);
    // A key frame from while a joiner's install is not known may lack its
    // objects' bytes or have them: it is left out.
    let before = times.len();
    times.retain(|&time| {
        time == first_world || !unsure.iter().any(|&(from, to)| from <= time && time <= to)
    });
    report.dropped_unsure_key_frames = before - times.len();
    let mut frames = Vec::new();
    for &time in &times {
        // Where the reading goes on: the first record after the world's time.
        let offset = records_read
            .iter()
            .find(|record| record.event.time > time)
            .map_or(body.len(), |record| record.offset);
        let missing: usize = installs
            .iter()
            .filter(|install| install.at > time)
            .map(|install| install.objects)
            .sum();
        let world = if missing == 0 {
            chosen[&time].clone()
        } else {
            pad_objects(&chosen[&time], missing).ok_or(ConvertError::Verify(format!(
                "the world at {time} ms does not split into its parts"
            )))?
        };
        if missing != 0 {
            report.padded_key_frames += 1;
        }
        frames.push(KeyFrame::new(time, &world, offset));
    }
    report.key_frame_parts = times
        .iter()
        .map(|time| {
            let parts = snapshot::Sections::walk(&chosen[time]).map(|s| {
                [
                    s.players.len(),
                    s.bullets.len(),
                    s.statistics.len(),
                    s.objects.len(),
                    s.physics.len(),
                    s.rules.len(),
                    s.effects.len(),
                ]
            });
            (*time, parts)
        })
        .collect();
    report.key_frames = times;

    let parts = Parts {
        options_lzma2: options_stream,
        body: &body,
        key_frames: &frames,
        config,
    };
    report.decoded_bytes = sur::decoded_size(options_decoded, &parts);
    let file = sur::write(&parts, options_decoded, settings.coding)?;
    sur::verify(&file).map_err(ConvertError::Verify)?;
    Ok((file, report))
}

/// The game objects a profile's kits put into the world core's list: one
/// for each grenade of a grenade stack and for each trap of a trap stack, in
/// every kit (the kit's bind 0x2D75B0 binds each of the slot's kits; the
/// grenade item's bind 0x309EC0 adds each grenade of its stack, the trap
/// item's 0x3067A0 each trap; client_patch.md 3.14). A stack's number is
/// the kit slot's count, as the client binds it.
pub fn kit_objects(profile: &static_info::MatchProfile) -> usize {
    profile
        .kits
        .iter()
        .flat_map(|kit| kit.slots.iter().flatten())
        .filter(|&&(index, _)| {
            profile
                .items
                .get(usize::from(index))
                .and_then(|item| item_params(item.dict_id))
                .is_some_and(|params| matches!(params.kind, "grenade" | "trap"))
        })
        .map(|&(_, count)| usize::from(count))
        .sum()
}

/// A place beyond the first roster, as the viewer binds it from chunk 1.
struct LatePlace {
    place: u8,
    /// Its kits' grenades and traps ([`kit_objects`]).
    objects: usize,
    /// The 0xE4s did not all give it the same number.
    count_unsure: bool,
    /// Its install is not before this: its first 0xE4, less a slack.
    from: u32,
    /// Its first entry; `None`: it never entered.
    entry: Option<u32>,
}

/// The places of the largest roster beyond the first one. A place's
/// profile never changes (appended places are never reused, `registry`),
/// so the largest roster's is the one the recorded games installed.
fn late_places(
    rosters: &BTreeMap<u8, (u32, static_info::MatchOptions)>,
    entered: &BTreeMap<(u32, u8), ()>,
    initial: u8,
    report: &mut Report,
) -> Vec<LatePlace> {
    let Some((_, (_, options))) = rosters.iter().next_back() else {
        return Vec::new();
    };
    let mut places = Vec::new();
    for (index, profile) in options
        .players
        .iter()
        .enumerate()
        .skip(usize::from(initial))
    {
        let place = index as u8;
        let objects = kit_objects(profile);
        let counts: Vec<usize> = rosters
            .values()
            .filter_map(|(_, other)| other.players.get(index))
            .map(kit_objects)
            .collect();
        let count_unsure = counts.iter().any(|&count| count != objects);
        if count_unsure {
            report.notes.push(format!(
                "place {place}: the 0xE4s give its kits {counts:?} grenades and traps"
            ));
        }
        let entry = entered
            .keys()
            .find(|&&(_, p)| p == place)
            .map(|&(time, _)| time);
        let offered = rosters
            .iter()
            .filter(|&(&roster, _)| roster > place)
            .map(|(_, (time, _))| time.saturating_sub(OFFER_SLACK_MS))
            .min();
        let from = match (offered, entry) {
            (Some(offered), Some(entry)) => offered.min(entry),
            (Some(offered), None) => offered,
            (None, Some(entry)) => entry.saturating_sub(INSTALL_WINDOW_MS),
            (None, None) => 0,
        };
        places.push(LatePlace {
            place,
            objects,
            count_unsure,
            from,
            entry,
        });
    }
    places
}

/// A late place's grenades and traps put into the recorded games.
struct Install {
    /// The first world that has them.
    at: u32,
    objects: usize,
}

/// How many zero bytes `after`'s game objects have in front of `before`'s,
/// and whether the rest is `before` exactly. The rest may differ further
/// on (a grenade thrown, a door opened in the same seconds), so it is
/// enough that past the zeros at least [`MIN_SHIFTED_PREFIX`] bytes agree,
/// and more than without them.
fn front_zeros(before: &[u8], after: &[u8]) -> Option<(usize, bool)> {
    let grown = after.len().checked_sub(before.len()).filter(|&n| n > 0);
    if let Some(n) = grown
        && after[..n].iter().all(|&b| b == 0)
        && after[n..] == *before
    {
        return Some((n, true));
    }
    let prefix = |a: &[u8], b: &[u8]| a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let unshifted = prefix(after, before);
    let leading = after.iter().take_while(|&&b| b == 0).count();
    let (zeros, shifted) = (1..=leading)
        .map(|n| (n, prefix(&after[n..], before)))
        .max_by_key(|&(n, agree)| (agree, std::cmp::Reverse(n)))?;
    (shifted > unshifted && shifted >= MIN_SHIFTED_PREFIX.min(before.len()))
        .then_some((zeros, false))
}

/// Where each late place's grenades and traps first are in the worlds:
/// between two worlds in a row whose game objects are the first one's with
/// as many zero bytes in front as the place's kits have objects, after its
/// first 0xE4 and by the first world after its entry. Two places installed
/// at once share one such step, with the sum of their objects.
///
/// Returns the installs and the spans `(from, to)` (both in) of the places
/// whose install is not known for sure: no such step or more than one, a
/// step back (a world without the bytes after one with them), or an exact
/// step no place's count explains (then the counts are wrong). Their key
/// frames in the span are to be left out; before it the bytes are missing,
/// after it (past the entry) they are there.
fn find_installs(
    chosen: &BTreeMap<u32, Vec<u8>>,
    places: &[LatePlace],
    report: &mut Report,
) -> (Vec<Install>, Vec<(u32, u32)>) {
    let worlds: Vec<(u32, &[u8])> = chosen
        .iter()
        .filter_map(|(&time, body)| Some((time, snapshot::Sections::walk(body)?.objects)))
        .collect();
    // (time of the later world, zeros, exact), and the steps back.
    let mut steps: Vec<(u32, usize, bool)> = Vec::new();
    let mut backs: Vec<u32> = Vec::new();
    for pair in worlds.windows(2) {
        let ((_, before), (at, after)) = (pair[0], pair[1]);
        if let Some((zeros, exact)) = front_zeros(before, after) {
            steps.push((at, zeros, exact));
        } else if front_zeros(after, before).is_some() {
            backs.push(at);
        }
    }
    // The span a place's step is looked for in.
    let span = |place: &LatePlace| {
        let to = place.entry.map_or(u32::MAX, |entry| {
            chosen.range(entry + 1..).next().map_or(entry, |(&t, _)| t)
        });
        (place.from, to)
    };
    let mut claimed = vec![false; steps.len()];
    let mut found: Vec<Option<u32>> = vec![None; places.len()];
    for (j, place) in places.iter().enumerate() {
        if place.objects == 0 || place.count_unsure {
            continue;
        }
        let (from, to) = span(place);
        let candidates: Vec<usize> = (0..steps.len())
            .filter(|&i| !claimed[i] && from < steps[i].0 && steps[i].0 <= to)
            .filter(|&i| steps[i].1 == place.objects)
            .collect();
        if let [i] = candidates[..] {
            claimed[i] = true;
            found[j] = Some(steps[i].0);
        }
    }
    // Places installed at once.
    for (i, &(at, zeros, _)) in steps.iter().enumerate() {
        if claimed[i] {
            continue;
        }
        let together: Vec<usize> = (0..places.len())
            .filter(|&j| found[j].is_none() && places[j].objects > 0 && !places[j].count_unsure)
            .filter(|&j| {
                let (from, to) = span(&places[j]);
                from < at && at <= to
            })
            .collect();
        let sum: usize = together.iter().map(|&j| places[j].objects).sum();
        if together.len() > 1 && sum == zeros {
            claimed[i] = true;
            for j in together {
                found[j] = Some(at);
            }
        }
    }
    let mut unsure: Vec<bool> = places
        .iter()
        .zip(&found)
        .map(|(place, found)| place.count_unsure || (place.objects > 0 && found.is_none()))
        .collect();
    // An exact step no count explains: the counts of the places it could
    // be are wrong.
    for (i, &(at, zeros, exact)) in steps.iter().enumerate() {
        if !exact {
            continue;
        }
        if claimed[i] {
            report.installs_exact += 1;
            continue;
        }
        let mut blamed = false;
        for (j, place) in places.iter().enumerate() {
            let (from, to) = span(place);
            if from < at && at <= to {
                unsure[j] = true;
                blamed = true;
            }
        }
        report.notes.push(format!(
            "the worlds got {zeros} zero bytes in front of the game objects at {at} ms, \
             which no late place's grenades and traps are{}",
            if blamed {
                ""
            } else {
                " (no late place was due then)"
            }
        ));
    }
    // A step back while a place's install is due: the worlds of games that
    // had installed it and of games that had not take turns.
    for (j, place) in places.iter().enumerate() {
        let (from, to) = span(place);
        if place.objects > 0 && backs.iter().any(|&at| from < at && at <= to) {
            unsure[j] = true;
        }
    }
    let mut installs = Vec::new();
    let mut spans = Vec::new();
    for (j, place) in places.iter().enumerate() {
        if unsure[j] {
            report.installs_unsure.push(place.place);
            let to = place.entry.unwrap_or(u32::MAX);
            spans.push((place.from, to));
            if place.objects > 0 {
                installs.push(Install {
                    at: to.saturating_add(1),
                    objects: place.objects,
                });
            }
        } else if let Some(at) = found[j] {
            report.installs.push((place.place, at, place.objects));
            installs.push(Install {
                at,
                objects: place.objects,
            });
        }
    }
    (installs, spans)
}

/// `body` with `count` zero bytes in front of its game objects.
fn pad_objects(body: &[u8], count: usize) -> Option<Vec<u8>> {
    let sections = snapshot::Sections::walk(body)?;
    let mut objects = vec![0u8; count];
    objects.extend_from_slice(sections.objects);
    snapshot::Sections {
        objects: &objects,
        ..sections
    }
    .to_body()
}

/// An input's time: the tick byte read against the match clock; an event
/// the clock puts well into the future is a late copy of an older one.
fn resolve(tick: u8, ms: u8, clock: u32, report: &mut Report) -> Option<u32> {
    let time = tick_time(tick, clock)? + u32::from(ms);
    if time > clock + FUTURE_SLACK_MS {
        report.dropped_future += 1;
        return None;
    }
    Some(time)
}

fn keep_input(
    inputs: &mut BTreeMap<(u32, u8), InputValue>,
    report: &mut Report,
    time: u32,
    player: u8,
    bytes: &[u8],
) {
    let actions = u32::from_le_bytes(bytes[4..8].try_into().expect("4 bytes"));
    if actions & INPUT_FLAG != 0 && !inputs.contains_key(&(time, player)) {
        report.flagged_inputs += 1;
    }
    let value = InputValue {
        rotation: bytes[0..4].try_into().expect("4 bytes"),
        actions: actions & !INPUT_FLAG,
    };
    keep_value(inputs, report, time, player, value);
}

/// An input's value kept for its time and player, once.
fn keep_value(
    inputs: &mut BTreeMap<(u32, u8), InputValue>,
    report: &mut Report,
    time: u32,
    player: u8,
    value: InputValue,
) {
    match inputs.get(&(time, player)) {
        None => {
            inputs.insert((time, player), value);
        }
        Some(known) if *known == value => report.input_copies += 1,
        Some(_) => report.input_conflicts += 1,
    }
}

/// The start of the tick a time is in; here for the CLI's summary.
pub fn tick_of(time: u32) -> u32 {
    time / TICK_MS
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::dictionary::PROFILE_SLOTS;
    use survarium::v026g::match_msgs::static_info::{
        MatchItem, MatchKit, MatchOptions, MatchProfile, NO_PROTECTION, SCORE_EVENTS, StaticInfo,
    };
    use survarium::v026g::match_msgs::upload;

    fn profile(name: &str, team: u8) -> MatchProfile {
        MatchProfile {
            team,
            name: name.into(),
            clan_tag: String::new(),
            protection: NO_PROTECTION,
            kits: vec![MatchKit::empty(1, "kit")],
            items: vec![MatchItem::default()],
            squad: 0,
        }
    }

    fn note(at_ms: u32, json: &str) -> Record {
        Record {
            kind: NOTE,
            place: 0xFF,
            at_ms,
            bytes: json.as_bytes().to_vec(),
        }
    }

    /// `seq rseq ack0 id [x] payload`, a single datagram.
    fn datagram(kind: u8, place: u8, at_ms: u32, id: u8, x: Option<u8>, payload: &[u8]) -> Record {
        let mut bytes = vec![0, 0, 0, id];
        bytes.extend(x);
        bytes.extend_from_slice(payload);
        Record {
            kind,
            place,
            at_ms,
            bytes,
        }
    }

    fn relay(tick: u8, player: u8, rotation: [u8; 4], actions: u32) -> Vec<u8> {
        let mut payload = vec![tick, player];
        payload.extend(rotation);
        payload.extend(actions.to_le_bytes());
        payload
    }

    /// Two places, a match clock 1300 ms behind the recording.
    fn recording() -> Vec<Record> {
        let map = survarium::v026g::maps::playable().next().unwrap();
        let options = MatchOptions {
            players: vec![profile("a", 0), profile("b", 1)],
            map_id: map.id,
            match_id: 1,
            match_level: 1,
            score_weights: [0.0; SCORE_EVENTS],
        };
        let info = StaticInfo::new(&options, Dictionary::shared()).unwrap();
        let mut records = vec![
            note(
                0,
                &format!(
                    r#"{{"note":"header","map":"{}","map_id":{},"mode":{},"match_id":"0x1","places":[{{"place":0}},{{"place":1}}]}}"#,
                    map.name, map.id, map.mode
                ),
            ),
            note(1350, r#"{"note":"clock","match_ms":50}"#),
        ];
        for place in 0..2u8 {
            records.push(datagram(
                OUT,
                place,
                1400,
                0xE4,
                Some(0),
                &info.payload(place).unwrap(),
            ));
        }
        // Both places hear of the entries; place 1 also hears the leave of 0.
        for place in 0..2u8 {
            records.push(datagram(OUT, place, 1700, 0xE9, None, &[8, 0]));
        }
        // Player 1's input at 1000 ms (tick 20), twice to place 0 (a copy),
        // player 0's at 1050 ms (tick 21) to place 1; one more of player 1
        // at 1000 ms + 7 ms through 0xEC.
        records.push(datagram(
            OUT,
            0,
            2400,
            0xEB,
            None,
            &relay(20, 1, [1, 2, 3, 4], 0x10),
        ));
        records.push(datagram(
            OUT,
            0,
            2450,
            0xEB,
            None,
            &relay(20, 1, [1, 2, 3, 4], 0x10),
        ));
        records.push(datagram(
            OUT,
            1,
            2420,
            0xEB,
            None,
            &relay(21, 0, [5, 6, 7, 8], 0x20),
        ));
        let mut precise = vec![20, 7, 1];
        precise.extend([9, 9, 9, 9]);
        precise.extend(0x30u32.to_le_bytes());
        records.push(datagram(OUT, 0, 2460, 0xEC, None, &precise));
        // A kit choice, and a leave at tick 40 (a farewell to the leaver
        // itself is not one).
        records.push(datagram(OUT, 0, 2500, 0xF3, None, &[22, 1, 3]));
        records.push(datagram(OUT, 0, 2600, 0xEA, None, &[40, 1]));
        records.push(datagram(OUT, 1, 2600, 0xEA, None, &[0, 1]));
        // Worlds at 50 ms and 1050 ms, uploaded by place 0.
        for (time, fill) in [(50u32, 1u8), (1050, 2)] {
            for chunk in upload::split(time, &vec![fill; 700]).unwrap() {
                records.push(datagram(IN, 0, 3000 + time, 0xE0, None, &chunk));
            }
        }
        records
    }

    #[test]
    fn a_recording_becomes_a_checked_file() {
        let records = recording();
        let config = b"config bytes";
        for coding in [Coding::Stored, Coding::Packed] {
            let settings = Settings {
                coding,
                key_frame_every_ms: 10_000,
                drop_key_frames: Vec::new(),
                first_key_frame_only: false,
            };
            let (file, report) = convert(&records, config, settings).unwrap();
            assert_eq!(report.clock_offset_ms, 1300);
            assert_eq!(report.inputs, 3);
            assert_eq!(report.input_copies, 1);
            assert_eq!(report.unparsed, 0, "{:?}", report.unparsed_why);
            assert_eq!(report.roster, 2);
            assert_eq!(report.worlds_complete, 2);
            let verified = sur::verify(&file).unwrap();
            assert_eq!(verified.start_ms, 50);
            assert_eq!(
                verified.key_frames.iter().map(|k| k.0).collect::<Vec<_>>(),
                [50]
            );
            let chunks = sur::chunks(&file).unwrap();
            let body = lzma2::unpack(chunks[2].1, 1 << 20).unwrap();
            let events = sur::read_body(&body).unwrap();
            let seen: Vec<(u8, u32)> = events.iter().map(|r| (r.kind, r.event.time)).collect();
            // marker, enter at 400 (tick 8), input of 1 at 1000 and its
            // precise one at 1007, the kit choice at 1100, the input of 0
            // at 1050, the leave at 2000; each tick closed.
            assert_eq!(seen[0], (4, 50));
            assert_eq!(seen[1], (5 + 4, 400));
            let inputs: Vec<_> = events
                .iter()
                .flat_map(|r| {
                    r.event
                        .inputs
                        .iter()
                        .map(move |i| (r.event.time, i.0, i.1.actions))
                })
                .collect();
            assert_eq!(inputs, [(1000, 1, 0x10), (1007, 1, 0x30), (1050, 0, 0x20)]);
            let profiles: Vec<_> = events
                .iter()
                .flat_map(|r| r.event.profiles.clone())
                .collect();
            assert_eq!(profiles, [(1, 3)]);
            let leaves: Vec<_> = events
                .iter()
                .filter(|r| r.event.leave != 0)
                .map(|r| (r.event.time, r.event.leave))
                .collect();
            assert_eq!(leaves, [(2000, 2)]);
        }
    }

    /// `(time, player, actions)` of every input in a file.
    fn inputs_of(file: &[u8]) -> Vec<(u32, u8, u32)> {
        let chunks = sur::chunks(file).unwrap();
        let body = lzma2::unpack(chunks[2].1, 1 << 20).unwrap();
        sur::read_body(&body)
            .unwrap()
            .iter()
            .flat_map(|r| {
                r.event
                    .inputs
                    .iter()
                    .map(move |i| (r.event.time, i.0, i.1.actions))
            })
            .collect()
    }

    /// A match of the per-tick protocol, one player and a bot: the
    /// player's own inputs are only in its updates (0xE2) and the outcomes
    /// it was sent; the bot's in the server's updates to the player.
    #[test]
    fn a_per_tick_recording_has_the_own_inputs_where_the_outcomes_put_them() {
        use survarium::v026g::match_msgs::{
            OwnOutcome, Outcome, PlaceInput, Places, ServerMessage,
        };
        use sync_core::wire::{CLIENT_UPDATE_MAX, Place};
        let mut records: Vec<Record> = recording()
            .into_iter()
            .filter(|record| record.kind != OUT || record.bytes[3] == 0xE4)
            .collect();
        // Its 0xE4s with the trailer.
        for record in &mut records {
            if record.kind == OUT && record.bytes[3] == 0xE4 {
                record
                    .bytes
                    .extend(static_info::ticks_trailer(Grid::STOCK));
            }
        }
        let own = |tick: u32, back: u8, actions: u32| {
            let mut update = ClientUpdate::empty(GameTime(tick));
            if actions != 0 {
                update
                    .push_input(Input {
                        back,
                        rotation: [1, 2, 3, 4],
                        actions,
                    })
                    .unwrap();
            }
            let mut bytes = vec![0; CLIENT_UPDATE_MAX];
            let len = update.write(&mut bytes).unwrap();
            bytes.truncate(len);
            bytes
        };
        // Player 0: 1000 confirmed; 1050 (3 ms back) moved to 1150; 1100
        // discarded; a copy of 1000.
        for (at, tick, back, actions) in [
            (2300, 1000, 0, 0x10),
            (2310, 1000, 0, 0x10),
            (2350, 1050, 3, 0x11),
            (2400, 1100, 0, 0x12),
            (2450, 1150, 0, 0),
        ] {
            records.push(datagram(IN, 0, at, CLIENT_UPDATE, None, &own(tick, back, actions)));
        }
        let mut update = Update::empty(GameTime(1150));
        update.entered = Places::EMPTY.with(Place::new(1).unwrap());
        update.inputs.push(PlaceInput {
            place: Place::new(1).unwrap(),
            input: Input {
                back: 0,
                rotation: [5, 6, 7, 8],
                actions: 0x20,
            },
        });
        for (update_tick, outcome) in [
            (1000, Outcome::Confirmed),
            (1050, Outcome::Moved { to: GameTime(1150) }),
            (1100, Outcome::Discarded),
        ] {
            update.outcomes.push(OwnOutcome {
                update: GameTime(update_tick),
                outcome,
            });
        }
        let payload = ServerMessage::Update(update).payload();
        records.push(datagram(OUT, 0, 2500, SERVER_UPDATE, Some(7), &payload));
        records.sort_by_key(|record| record.at_ms);
        let (file, report) = convert(&records, b"config", Settings::default()).unwrap();
        assert_eq!(report.unparsed, 0, "{:?}", report.unparsed_why);
        assert_eq!((report.updates, report.own_updates), (1, 4));
        assert_eq!(
            inputs_of(&file),
            [(1000, 0, 0x10), (1147, 0, 0x11), (1150, 1, 0x20)]
        );
        assert!(report.roster_events.contains(&(1150, 'E', 1)));
    }

    /// Phase 4: a match on another tick has no replay (the game's replay
    /// player is on 50 ms); the key's absence is the game's own 50.
    #[test]
    fn a_recording_of_another_tick_is_refused() {
        let mut records = recording();
        let head = String::from_utf8(records[0].bytes.clone()).unwrap();
        records[0].bytes = head.replace("\"places\"", "\"tick_ms\":20,\"places\"").into_bytes();
        assert!(matches!(
            convert(&records, b"config", Settings::default()),
            Err(ConvertError::Tick(20))
        ));
        let mut records = recording();
        records[0].bytes = head.replace("\"places\"", "\"tick_ms\":50,\"places\"").into_bytes();
        assert!(convert(&records, b"config", Settings::default()).is_ok());
    }

    #[test]
    fn a_copy_from_a_backed_up_link_is_not_read_as_another_tick() {
        let mut records = recording();
        // Player 0's input of 1050 ms (tick 21) reaches place 0 18.6 s
        // after place 1 got it: read against that clock its tick byte is
        // 13 850 ms, a time the server never meant.
        records.push(datagram(
            OUT,
            0,
            21_000,
            0xEB,
            None,
            &relay(21, 0, [5, 6, 7, 8], 0x20),
        ));
        let (file, report) = convert(&records, b"config", Settings::default()).unwrap();
        assert_eq!(
            inputs_of(&file),
            [(1000, 1, 0x10), (1007, 1, 0x30), (1050, 0, 0x20)]
        );
        assert_eq!(report.lagging_copies, BTreeMap::from([(0, 1)]));
        assert_eq!(report.messages_lost, 0);
        assert_eq!(report.input_conflicts, 0);
    }

    #[test]
    fn the_flag_bit_of_an_input_is_not_an_action() {
        let mut records = recording();
        records.push(datagram(
            OUT,
            0,
            2470,
            0xEB,
            None,
            &relay(22, 1, [1, 2, 3, 4], 0x8000_0010),
        ));
        let (file, report) = convert(&records, b"config", Settings::default()).unwrap();
        assert!(inputs_of(&file).contains(&(1100, 1, 0x10)));
        assert_eq!(report.flagged_inputs, 1);
    }

    #[test]
    fn a_growing_roster_reads_masks_of_twenty() {
        let mut records = recording();
        let mut header: serde_json::Value = serde_json::from_slice(&records[0].bytes).unwrap();
        header["growing_roster"] = true.into();
        records[0].bytes = serde_json::to_vec(&header).unwrap();
        // 0xED at tick 23: players 0 and 1, a mask of three bytes, the
        // actions, then the first and the second halves of the rotations.
        let mut payload = vec![23, 0b11, 0, 0];
        payload.extend(0x40u32.to_le_bytes());
        payload.extend(0x41u32.to_le_bytes());
        payload.extend([1, 2, 5, 6]);
        payload.extend([3, 4, 7, 8]);
        records.push(datagram(OUT, 0, 2480, 0xED, None, &payload));
        let (file, report) = convert(&records, b"config", Settings::default()).unwrap();
        assert!(report.growing_roster);
        assert_eq!(report.unparsed, 0, "{:?}", report.unparsed_why);
        let inputs = inputs_of(&file);
        assert!(inputs.contains(&(1150, 0, 0x40)), "{inputs:?}");
        assert!(inputs.contains(&(1150, 1, 0x41)), "{inputs:?}");
    }

    fn item(name: &str) -> u16 {
        Dictionary::shared().item_named(name).unwrap().dict_id
    }

    /// A kit slot that takes `dict_id`, other than those `taken`.
    fn slot_for(dict_id: u16, taken: &[usize]) -> usize {
        (0..PROFILE_SLOTS)
            .find(|&slot| !taken.contains(&slot) && Dictionary::shared().fits(dict_id, slot as u8))
            .unwrap()
    }

    /// Two grenades and a trap in the first kit, the same two grenades in
    /// the second: five game objects, though the stacks hold 10 and 7.
    fn armed(name: &str, team: u8) -> MatchProfile {
        let (grenade, trap) = (item("rgd-5"), item("base_trap"));
        let stack = |dict_id: u16, count: u32| MatchItem {
            instance_id: 100 + u32::from(dict_id),
            dict_id,
            condition_or_stack: count,
            ..MatchItem::default()
        };
        let grenades = slot_for(grenade, &[]);
        let traps = slot_for(trap, &[grenades]);
        let mut first = MatchKit::empty(1, "a");
        first.slots[grenades] = Some((0, 2));
        first.slots[traps] = Some((1, 1));
        let mut second = MatchKit::empty(2, "b");
        second.slots[grenades] = Some((0, 2));
        MatchProfile {
            kits: vec![first, second],
            items: vec![stack(grenade, 10), stack(trap, 7)],
            ..profile(name, team)
        }
    }

    #[test]
    fn a_kit_has_a_game_object_for_each_grenade_and_trap_of_its_slots() {
        assert_eq!(kit_objects(&armed("c", 0)), 5);
        assert_eq!(kit_objects(&profile("a", 0)), 0);
    }

    #[test]
    fn zero_bytes_in_front_are_found_past_later_changes() {
        let before: Vec<u8> = [4, 0xFF].into_iter().chain(1..=20).collect();
        let with = |zeros: usize, rest: &[u8]| [vec![0; zeros], rest.to_vec()].concat();
        assert_eq!(front_zeros(&before, &with(3, &before)), Some((3, true)));
        // A grenade thrown further on: a byte changed, eight more.
        let mut changed = before.clone();
        changed[15] = 2;
        changed.splice(16..16, [9; 8]);
        assert_eq!(front_zeros(&before, &with(3, &changed)), Some((3, false)));
        assert_eq!(front_zeros(&before, &before), None);
        assert_eq!(front_zeros(&before, &changed), None);
        // Objects that begin with zero bytes already.
        let zeros_first = with(2, &before);
        assert_eq!(
            front_zeros(&zeros_first, &with(1, &zeros_first)),
            Some((1, true))
        );
        // Too little agrees past the zeros.
        let mut early = with(1, &before);
        early[5] = 0x55;
        assert_eq!(front_zeros(&before, &early), None);
    }

    /// A world with no player and `objects`.
    fn world(objects: &[u8]) -> Vec<u8> {
        snapshot::Sections {
            active_clients_mask: 0,
            players: &[],
            bullets: &[],
            statistics: &[],
            objects,
            physics: &[],
            rules: &[],
            effects: &[],
        }
        .to_body()
        .unwrap()
    }

    /// The first world's objects, before a late joiner's install.
    fn map_objects() -> Vec<u8> {
        [4, 0xFF].into_iter().chain(1..=20).collect()
    }

    /// [`recording`] whose roster grows: place 2 ([`armed`]) is offered at
    /// match time 18 700 ms and enters at 25 000 ms; the worlds every 5 s
    /// get `zeros` zero bytes in front of their objects at 20 050 ms, and a
    /// grenade of the map thrown at once unless `exact`.
    fn late_recording(zeros: usize, exact: bool) -> Vec<Record> {
        let mut records: Vec<Record> = recording().into_iter().filter(|r| r.kind != IN).collect();
        let mut header: serde_json::Value = serde_json::from_slice(&records[0].bytes).unwrap();
        header["growing_roster"] = true.into();
        records[0].bytes = serde_json::to_vec(&header).unwrap();
        let map = survarium::v026g::maps::playable().next().unwrap();
        let options = MatchOptions {
            players: vec![profile("a", 0), profile("b", 1), armed("c", 0)],
            map_id: map.id,
            match_id: 1,
            match_level: 1,
            score_weights: [0.0; SCORE_EVENTS],
        };
        let info = StaticInfo::new(&options, Dictionary::shared()).unwrap();
        for place in 0..3u8 {
            records.push(datagram(
                OUT,
                place,
                20_000,
                0xE4,
                Some(1),
                &info.payload(place).unwrap(),
            ));
        }
        // Tick 500 is byte 244.
        for place in 0..2u8 {
            records.push(datagram(OUT, place, 26_400, 0xE9, None, &[244, 2]));
        }
        let before = map_objects();
        let mut after = vec![0; zeros];
        after.extend(&before);
        if !exact {
            let last = after.len() - 1;
            after[last] = 0x77;
        }
        for time in (50..=30_050u32).step_by(5_000) {
            let objects = if time < 20_050 { &before } else { &after };
            for chunk in upload::split(time, &world(objects)).unwrap() {
                records.push(datagram(IN, 0, 1_300 + time + 500, 0xE0, None, &chunk));
            }
        }
        records
    }

    /// `(time, game objects)` of every key frame of a file.
    fn key_frame_objects(file: &[u8]) -> Vec<(u32, Vec<u8>)> {
        let chunks = sur::chunks(file).unwrap();
        let frames = chunks
            .iter()
            .find(|(id, _)| *id == sur::chunk::KEY_FRAMES)
            .unwrap()
            .1;
        let table = lzma2::unpack(&frames[2..], 1 << 20).unwrap();
        let mut out = Vec::new();
        let mut at = 0;
        while at < table.len() {
            let len = usize::from(u16::from_le_bytes([table[at], table[at + 1]]));
            let data = &table[at + 6..at + 6 + len];
            let time = u32::from_le_bytes(data[..4].try_into().unwrap());
            out.push((
                time,
                snapshot::Sections::walk(&data[4..])
                    .unwrap()
                    .objects
                    .to_vec(),
            ));
            at += 6 + len;
        }
        out
    }

    #[test]
    fn key_frames_before_a_late_install_get_the_joiners_objects_counted_from_its_kits() {
        let (file, report) =
            convert(&late_recording(5, false), b"config", Settings::default()).unwrap();
        assert_eq!(report.roster_events.last(), Some(&(25_000, 'E', 2)));
        assert_eq!(report.installs, [(2, 20_050, 5)]);
        assert_eq!(report.installs_exact, 0);
        assert!(report.installs_unsure.is_empty());
        assert_eq!(report.dropped_unsure_key_frames, 0);
        assert_eq!(report.padded_key_frames, 2);
        let padded = [vec![0; 5], map_objects()].concat();
        let frames = key_frame_objects(&file);
        let times: Vec<u32> = frames.iter().map(|f| f.0).collect();
        assert_eq!(times, [50, 10_050, 20_050, 30_050]);
        for (time, objects) in &frames[..2] {
            assert_eq!(*objects, padded, "{time}");
        }
        for (time, objects) in &frames[2..] {
            assert_eq!(objects[..5], [0; 5], "{time}");
            assert_eq!(objects.len(), padded.len(), "{time}");
        }
    }

    #[test]
    fn an_install_no_kit_count_explains_leaves_out_the_key_frames_while_it_was_due() {
        let (file, report) =
            convert(&late_recording(4, true), b"config", Settings::default()).unwrap();
        assert!(report.installs.is_empty());
        assert_eq!(report.installs_unsure, [2]);
        assert!(
            report.notes.iter().any(|n| n.contains("4 zero bytes")),
            "{:?}",
            report.notes
        );
        // From the 0xE4 that offered place 2 (17 700 ms) to its entry.
        assert_eq!(report.dropped_unsure_key_frames, 1);
        let frames = key_frame_objects(&file);
        let times: Vec<u32> = frames.iter().map(|f| f.0).collect();
        assert_eq!(times, [50, 10_050, 30_050]);
        // Before the span the viewer's five objects are given; after the
        // entry the world is as the games had it.
        assert_eq!(frames[0].1, [vec![0; 5], map_objects()].concat());
        assert_eq!(frames[2].1, [vec![0; 4], map_objects()].concat());
    }
}
