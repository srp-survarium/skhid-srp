//! Replay sessions (`docs/notes/replay-stream.md`): a recorded match played
//! to a player's own game in the running client, through the spectators'
//! machine ([`spectate`]): the "replay match" of path A in
//! `research/replay-plan.md`. The lobby starts a session and sends the game
//! here; what the two share of it is `session::v026g::replay_watch`.
//!
//! The recording is the match's `.srprec`, made in memory into what the
//! converter makes of it (`sur_convert`, no mode config): its 0xE4 (every
//! place of the match, as the replay viewer has them), its key frames as the
//! worlds a game may start from, and its inputs, kit choices, entries and
//! leaves as the relays and roster changes, at their recorded times. It is
//! read on a thread of its own at the lowest priority ([`read`]), one at a
//! time, and shared by the sessions of its match.
//!
//! A playback clock says how far the match has gone. What is due by it
//! ([`RELAY_LEAD_MS`] ahead, as a live game gets an input a little before
//! its tick) goes to the game as from a live match. It starts at the first
//! key frame after an entry (the asked place's, else anyone's), or later
//! when asked ([`Recording::start`]): the game has someone to follow from
//! its first world. Pause stops the clock; speed scales it; a seek, and
//! play after a pause, start the game again (0xE5 of its next generation,
//! the world) from the key frame at or before the time, put as far past it
//! as the game's input lists reach ([`Recording::reach`]). The end of the
//! recording lets the game go (0xEA naming no place), as the end of a live
//! match does. The controls come from the game ([`Control::decode`]).

use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use session::v026g::replay_watch::WatchRequest;
use session::v026g::replays::{DEFAULT_MAX_RECORDING_MB, Refusal, Replays, Status};
use survarium::v026g::match_msgs::static_info::{self, NO_LOCAL_PLACE, StaticInfo};
use survarium::v026g::match_msgs::{PlayerInput, WireTime, mask_bytes};
use udp_transport::table::{Direction, Table};
use udp_transport::wire;

use super::spectate::{Heard, Spectator, Watched};
use super::*;
use crate::v026g::sur::{self, record_type};
use crate::v026g::{record, replay, sur_convert};

/// What is due this long past the playback clock goes already: a live game
/// gets a player's input about this much before its tick (the server's
/// event horizon, [`HORIZON_MS`]).
const RELAY_LEAD_MS: u32 = HORIZON_MS;
/// How often the dev control file is read (`SRP_DEV_REPLAY_CONTROL`).
const CONTROL_EVERY: Duration = Duration::from_millis(250);
/// A paused game is pinged this often: nothing else goes to it, and a game
/// ends a match link silent for 5 s.
const PAUSED_PING_EVERY: Duration = Duration::from_secs(1);

/// A recording made into what a game is shown.
pub(crate) struct Recording {
    match_id: u32,
    info: StaticInfo,
    /// Every place's account, and each place's game's patch revision (the
    /// recording's notes): who may watch it, and which games play it.
    players: Vec<u32>,
    revisions: Vec<Option<u8>>,
    /// The key frames, in time order.
    worlds: Vec<Snapshot>,
    /// In time order.
    inputs: Vec<(u32, usize, Relayed)>,
    /// In time order.
    events: Vec<(u32, usize, bool)>,
    /// The time of its last record.
    end: u32,
}

impl Recording {
    /// A match's `.srprec`, converted here (with no mode config: the
    /// `.sur`'s chunk 4 is the viewer's alone).
    pub(crate) fn load(path: &Path, match_id: u32) -> Result<Self, String> {
        let notes = record::read_notes(path).map_err(|error| error.to_string())?;
        let settings = sur_convert::Settings {
            // As many key frames as a file may hold: more places to seek to.
            key_frame_every_ms: 1_000,
            ..sur_convert::Settings::default()
        };
        let (file, _) = sur_convert::convert_file(path, &notes, &[], settings)
            .map_err(|error| error.to_string())?;
        let mut recording = Self::from_sur(&file, match_id)?;
        (recording.players, recording.revisions) = replay::places(&notes);
        Ok(recording)
    }

    /// A `.sur` (of no known players: the tests').
    pub(crate) fn from_sur(file: &[u8], match_id: u32) -> Result<Self, String> {
        let contents = sur::read(file)?;
        let mut payload = vec![NO_LOCAL_PLACE];
        payload.extend(&contents.options_lzma2);
        let dictionary = Dictionary::shared();
        let (_, options) = static_info::parse_payload(&payload, dictionary)
            .map_err(|error| format!("the options: {error}"))?;
        let info = StaticInfo::new(&options, dictionary)
            .map_err(|error| format!("the options: {error}"))?;
        let worlds: Vec<Snapshot> = contents
            .key_frames
            .iter()
            .map(|frame| Snapshot {
                time: frame.time(),
                body: frame.data[4..].to_vec(),
            })
            .collect();
        let mut inputs = Vec::new();
        let mut events = Vec::new();
        for record in &contents.records {
            if record.kind < record_type::FIRST_EVENT {
                continue;
            }
            let event = &record.event;
            let time = event.time;
            for player in 0..sur::MAX_PLAYERS {
                if event.enter & (1 << player) != 0 {
                    events.push((time, player, true));
                }
                if event.leave & (1 << player) != 0 {
                    events.push((time, player, false));
                }
            }
            for &(player, value) in &event.profiles {
                let tick = tick_byte(time);
                inputs.push((time, usize::from(player), Relayed::Profile { tick, value }));
            }
            for &(player, value) in &event.inputs {
                let input = PlayerInput {
                    time: if time % TICK_MS == 0 {
                        WireTime::tick(time)
                    } else {
                        WireTime::precise(time)
                    },
                    rotation: value.rotation,
                    actions: value.actions,
                };
                inputs.push((time, usize::from(player), Relayed::Input(input)));
            }
        }
        let end = contents
            .records
            .last()
            .map_or(0, |record| record.event.time);
        if worlds.is_empty() {
            return Err("no key frame".into());
        }
        Ok(Self {
            match_id,
            info,
            players: Vec::new(),
            revisions: Vec::new(),
            worlds,
            inputs,
            events,
            end,
        })
    }

    /// The key frame a game shown `time` starts from: the last at or before
    /// it, else the first.
    fn world_for(&self, time: u32) -> &Snapshot {
        let after = self.worlds.partition_point(|world| world.time <= time);
        &self.worlds[after.saturating_sub(1)]
    }

    /// Where a session asked for `at` with `place` in view starts: the first
    /// key frame after the place's first entry (anyone's when it never
    /// entered, or none is asked), or `at` when that is later (from the key
    /// frame at or before it, [`Self::seek_to`]). A world before any entry
    /// has nobody in it: the game would follow no one.
    pub(crate) fn start(&self, at: u32, place: Option<u8>) -> u32 {
        let entry = |wanted: Option<usize>| {
            self.events
                .iter()
                .find(|&&(_, who, entered)| entered && wanted.is_none_or(|place| who == place))
                .map(|&(time, _, _)| time)
        };
        let first_view = entry(place.map(usize::from))
            .or_else(|| entry(None))
            .map_or(self.worlds[0].time, |entered| {
                self.worlds
                    .iter()
                    .find(|world| world.time > entered)
                    .unwrap_or_else(|| self.world_for(entered))
                    .time
            });
        if at > first_view {
            self.seek_to(at)
        } else {
            first_view
        }
    }

    /// Where a game sought to `time` is put: from the key frame at or before
    /// it, as far as [`Self::reach`] lets it.
    fn seek_to(&self, time: u32) -> u32 {
        let from = self.world_for(time).time;
        self.reach(from, time.min(self.end).max(from))
    }

    /// How far past the world at `from` a game started from it can be put,
    /// at most `to`: its inputs since the world come with it, read against
    /// the clock it is put at (a tick byte reaches 6.35 s: no more than
    /// [`TICK_REACH_MS`] back), and a game keeps [`INPUT_RING`] of a
    /// player's (one fewer while the world is on its way,
    /// `INPUT_RING_SAFE`); one older than those would be played wrong. So no
    /// player may have more inputs in (`from`, the time and
    /// [`RELAY_LEAD_MS`]].
    fn reach(&self, from: u32, to: u32) -> u32 {
        let to = to.min(from + TICK_REACH_MS);
        let mut counts = [0usize; sur::MAX_PLAYERS];
        let first = self.inputs.partition_point(|(at, _, _)| *at <= from);
        for &(at, player, what) in &self.inputs[first..] {
            if at > to + RELAY_LEAD_MS {
                break;
            }
            if let (Relayed::Input(_), Some(count)) = (what, counts.get_mut(player)) {
                *count += 1;
                if *count > INPUT_RING_SAFE {
                    return to.min(at.saturating_sub(RELAY_LEAD_MS + 1)).max(from);
                }
            }
        }
        to
    }

    /// Place `place`'s first entry moved to `time` (the tests' recordings
    /// have everyone in before their second key frame).
    #[cfg(test)]
    pub(crate) fn move_entry(&mut self, place: usize, time: u32) {
        if let Some(event) = self
            .events
            .iter_mut()
            .find(|(_, who, entered)| *who == place && *entered)
        {
            event.0 = time;
        }
        self.events.sort_by_key(|event| event.0);
    }

    /// How many of its inputs and roster changes are due by `time`.
    fn due(&self, time: u32) -> (usize, usize) {
        let horizon = time + RELAY_LEAD_MS;
        (
            self.inputs.partition_point(|(at, _, _)| *at <= horizon),
            self.events.partition_point(|(at, _, _)| *at <= horizon),
        )
    }
}

/// How fast a recording plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Speed {
    Half,
    Normal,
    Double,
    Quadruple,
}

impl Speed {
    /// Match ms in `elapsed` of the wall clock.
    fn of(self, elapsed: Duration) -> u32 {
        let ms = elapsed.as_millis() as u32;
        match self {
            Self::Half => ms / 2,
            Self::Normal => ms,
            Self::Double => ms * 2,
            Self::Quadruple => ms * 4,
        }
    }

    /// From a control's percent: 50, 100, 200 or 400.
    fn of_percent(percent: u32) -> Option<Self> {
        Some(match percent {
            50 => Self::Half,
            100 => Self::Normal,
            200 => Self::Double,
            400 => Self::Quadruple,
            _ => return None,
        })
    }

    #[cfg(test)]
    fn percent(self) -> u32 {
        match self {
            Self::Half => 50,
            Self::Normal => 100,
            Self::Double => 200,
            Self::Quadruple => 400,
        }
    }
}

/// The first bytes of a control's 0xE0 payload ([`Control::decode`]): no
/// upload chunk (more than 8 bytes) nor install ack (`ff ff ...`, 20 bytes)
/// starts so.
const CONTROL_MAGIC: [u8; 2] = [0xFF, b'R'];

/// A playback control of the watcher's game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Control {
    /// op 1
    Pause,
    /// op 2
    Play,
    /// op 3, the percent: 50, 100, 200, 400
    Speed(Speed),
    /// op 4, match time in ms
    Seek(u32),
}

impl Control {
    /// A replay session's game's 0xE0 (alone in its datagram, as an upload
    /// chunk is) with `ff 52 <u8 seq> <u8 op> <u32 arg LE>`: its sequence
    /// number and the control. The game numbers its controls (wrapping);
    /// one with the number of the last taken is a resend, not taken again.
    pub(crate) fn decode(payload: &[u8]) -> Option<(u8, Self)> {
        let [m0, m1, seq, op, a0, a1, a2, a3] = *payload else {
            return None;
        };
        if [m0, m1] != CONTROL_MAGIC {
            return None;
        }
        let arg = u32::from_le_bytes([a0, a1, a2, a3]);
        let control = match op {
            1 => Self::Pause,
            2 => Self::Play,
            3 => Self::Speed(Speed::of_percent(arg)?),
            4 => Self::Seek(arg),
            _ => return None,
        };
        Some((seq, control))
    }

    /// The 0xE0 payload of this control as its game sends it.
    #[cfg(test)]
    pub(crate) fn encode(self, seq: u8) -> [u8; 8] {
        let (op, arg) = match self {
            Self::Pause => (1, 0),
            Self::Play => (2, 0),
            Self::Speed(speed) => (3, speed.percent()),
            Self::Seek(ms) => (4, ms),
        };
        let [a0, a1, a2, a3] = u32::to_le_bytes(arg);
        [CONTROL_MAGIC[0], CONTROL_MAGIC[1], seq, op, a0, a1, a2, a3]
    }

    /// A line of the dev control file (`SRP_DEV_REPLAY_CONTROL`): `pause`,
    /// `play`, `speed 0.5|1|2|4`, `seek <seconds>`.
    pub(crate) fn parse(line: &str) -> Option<Self> {
        let mut words = line.split_whitespace();
        let control = match (words.next()?, words.next()) {
            ("pause", None) => Self::Pause,
            ("play", None) => Self::Play,
            ("speed", Some("0.5")) => Self::Speed(Speed::Half),
            ("speed", Some("1")) => Self::Speed(Speed::Normal),
            ("speed", Some("2")) => Self::Speed(Speed::Double),
            ("speed", Some("4")) => Self::Speed(Speed::Quadruple),
            ("seek", Some(seconds)) => {
                let seconds: f64 = seconds.parse().ok().filter(|s: &f64| *s >= 0.0)?;
                Self::Seek((seconds * 1000.0) as u32)
            }
            _ => return None,
        };
        words.next().is_none().then_some(control)
    }
}

/// The playback clock: match time, by the wall clock.
#[derive(Clone, Copy, Debug)]
enum Clock {
    /// Its game loads its level: the clock waits at `at`.
    Loading {
        at: u32,
    },
    /// At `at` at `since`, running at the playback's speed.
    Playing {
        since: Instant,
        at: u32,
    },
    /// Stopped at `at`; its game pinged next at `ping`.
    Paused { at: u32, ping: Instant },
}

/// What became of a playback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ended {
    /// Its game left by its own word, or its link is gone: forgotten.
    Left,
    /// The recording is over: its game is let go (and then forgotten).
    Over,
}

/// One session's game shown its recording.
struct Playback {
    recording: Arc<Recording>,
    request: WatchRequest,
    spectator: Spectator,
    clock: Clock,
    speed: Speed,
    /// The sequence number of the last control taken from its game.
    last_control: Option<u8>,
    /// How many of the recording's inputs and roster changes have gone to
    /// it as they came due (those since its world go with the world).
    inputs: usize,
    events: usize,
    created: Instant,
    settings: Settings,
}

impl Playback {
    fn new(
        recording: Arc<Recording>,
        request: WatchRequest,
        spectator: Spectator,
        start: u32,
        settings: Settings,
        now: Instant,
    ) -> Self {
        let (inputs, events) = recording.due(start);
        Self {
            recording,
            request,
            spectator,
            clock: Clock::Loading { at: start },
            speed: Speed::Normal,
            last_control: None,
            inputs,
            events,
            created: now,
            settings,
        }
    }

    /// The playback clock.
    fn position(&self, now: Instant) -> u32 {
        match self.clock {
            Clock::Loading { at } | Clock::Paused { at, .. } => at,
            Clock::Playing { since, at } => {
                at + self.speed.of(now.saturating_duration_since(since))
            }
        }
    }

    fn watched<'a>(&self, recording: &'a Recording, clock: u32) -> Watched<'a> {
        Watched {
            id: recording.match_id,
            clock,
            info: &recording.info,
            local_place: NO_LOCAL_PLACE,
            places: usize::from(recording.info.roster()),
            roster: usize::from(recording.info.roster()),
            worlds: recording.worlds.iter().collect(),
            inputs: &recording.inputs[..self.inputs],
            events: &recording.events[..self.events],
            compress: self.settings.compress_state,
            entry_state: self.settings.timing.entry_state,
        }
    }

    /// A datagram of its game: `Some` once its game has left.
    fn receive(&mut self, datagram: &[u8], now: Instant) -> Option<Ended> {
        let recording = Arc::clone(&self.recording);
        let session_id = self.spectator.session_id();
        log::trace!(
            "Replay {session_id:#x}: in {} bytes: {}",
            datagram.len(),
            messages(Direction::ToServer, datagram)
        );
        for event in self.spectator.receive(datagram, now) {
            let Event::Message { id, payload } = event else {
                log::info!("Replay {session_id:#x}: its game disconnected");
                return Some(Ended::Left);
            };
            if id == survarium::v026g::match_msgs::id::LEAVE
                && let Some((seq, control)) = Control::decode(&payload)
            {
                if self.last_control != Some(seq) && !self.spectator.is_leaving() {
                    self.last_control = Some(seq);
                    self.control(control, now);
                }
                continue;
            }
            let before = self.spectator.stage();
            let watched = self.watched(&recording, self.position(now));
            let heard = self.spectator.hear(&watched, id, &payload, now);
            let after = self.spectator.stage();
            if after != before {
                log::debug!(
                    "Replay {session_id:#x}: heard {id:#04x} ({} bytes): {before} -> {after}",
                    payload.len()
                );
            }
            match heard {
                Heard::Left => {
                    log::info!("Replay {session_id:#x}: its game left (0xE0)");
                    return Some(Ended::Left);
                }
                // It has every place from its 0xE4: nothing is installed.
                Heard::Installed(_) | Heard::Nothing => {}
            }
        }
        None
    }

    fn control(&mut self, control: Control, now: Instant) {
        let at = self.position(now);
        log::info!(
            "Replay {:#x} of match {:#x}: {control:?} at {at} ms",
            self.spectator.session_id(),
            self.recording.match_id
        );
        match control {
            Control::Pause => {
                if let Clock::Playing { .. } | Clock::Loading { .. } = self.clock {
                    self.clock = Clock::Paused {
                        at,
                        ping: now + PAUSED_PING_EVERY,
                    };
                }
            }
            // Its game's clock ran on while paused: it starts again from
            // where the playback stopped.
            Control::Play => {
                if let Clock::Paused { at, .. } = self.clock {
                    self.clock = Clock::Playing { since: now, at };
                    self.seek(at, now);
                }
            }
            Control::Speed(speed) => {
                if let Clock::Playing { .. } = self.clock {
                    self.clock = Clock::Playing { since: now, at };
                }
                self.speed = speed;
            }
            Control::Seek(time) => self.seek(time, now),
        }
    }

    /// The game starts again from the key frame at or before `time`, put as
    /// far past it as it can be.
    fn seek(&mut self, time: u32, now: Instant) {
        let at = self.recording.seek_to(time);
        self.clock = match self.clock {
            Clock::Loading { .. } => Clock::Loading { at },
            Clock::Playing { .. } => Clock::Playing { since: now, at },
            Clock::Paused { ping, .. } => Clock::Paused { at, ping },
        };
        (self.inputs, self.events) = self.recording.due(at);
        self.spectator.wait_again();
    }

    /// Its turn: what came due goes, it is started from a world when it
    /// waits for one, and the end of the recording lets it go.
    fn poll(&mut self, now: Instant) -> (Vec<(SocketAddr, Vec<u8>)>, Option<Ended>) {
        let session_id = self.spectator.session_id();
        if self.spectator.timed_out(&self.settings.timing, now) {
            log::info!(
                "Replay {session_id:#x}: its game sent nothing for too long at {}; forgotten",
                self.spectator.stage()
            );
            return (Vec::new(), Some(Ended::Left));
        }
        let before = self.spectator.stage();
        match self.clock {
            Clock::Loading { at } if self.spectator.has_loaded() => {
                self.clock = Clock::Playing { since: now, at };
            }
            Clock::Paused { at, ping } if now >= ping => {
                self.spectator.ping();
                self.clock = Clock::Paused {
                    at,
                    ping: now + PAUSED_PING_EVERY,
                };
            }
            _ => {}
        }
        let at = self.position(now);
        let mut ended = None;
        if at >= self.recording.end && !self.spectator.is_leaving() {
            log::info!("Replay {session_id:#x}: the recording is over");
            self.spectator.farewell(at, now);
            ended = Some(Ended::Over);
        }
        let recording = Arc::clone(&self.recording);
        let (inputs, events) = recording.due(at);
        for &(time, index, entered) in &recording.events[self.events..events] {
            let message = if entered {
                ServerMessage::PlayerEntered {
                    tick: tick_byte(time),
                    player: index as u8,
                }
            } else {
                ServerMessage::PlayerLeft {
                    tick: tick_byte(time),
                    player: index as u8,
                }
            };
            self.spectator.roster_change(index, entered, time, &message);
        }
        for (time, from, what) in &recording.inputs[self.inputs..inputs] {
            self.spectator
                .relay(*from, &relay(*from, what), *time, at, now);
        }
        (self.inputs, self.events) = (inputs, events);
        let echo = now.saturating_duration_since(self.created).as_millis() as u16;
        let start = recording.world_for(at).time;
        self.spectator
            .step(&self.watched(&recording, at), Some(start), echo, now);
        let after = self.spectator.stage();
        if after != before {
            log::debug!("Replay {session_id:#x}: {before} -> {after} at {at} ms");
        }
        let datagrams = self.spectator.poll(now);
        for (_, datagram) in &datagrams {
            log::trace!(
                "Replay {session_id:#x}: out {} bytes: {}",
                datagram.len(),
                messages(Direction::ToClient, datagram)
            );
        }
        (datagrams, ended)
    }
}

/// A datagram's messages for the log: id, part and length.
fn messages(direction: Direction, datagram: &[u8]) -> String {
    match wire::parse(
        Table::Match,
        direction,
        datagram,
        mask_bytes(sur::MAX_PLAYERS as u8),
    ) {
        Ok(parsed) if parsed.messages.is_empty() => "acks only".to_owned(),
        Ok(parsed) => parsed
            .messages
            .iter()
            .map(|message| match message.part {
                Some(part) => format!(
                    "{:02X} part {} ({} bytes)",
                    message.id,
                    part.index,
                    message.payload.len()
                ),
                None => format!("{:02X} ({} bytes)", message.id, message.payload.len()),
            })
            .collect::<Vec<_>>()
            .join(", "),
        Err(error) => format!("unparsed: {error:?}"),
    }
}

/// Why a session's recording is not shown: its answer to the lobby.
type Refused = (Status, Option<Refusal>);

/// A match's recording read for a session ([`ReplayWatch`]'s reader): not
/// found (`gone`), over `SRP_REPLAY_MAX_RECORDING_MB` or unreadable
/// (`failed`), or not for this player ([`Watches::judge`], before it is
/// converted). The heavy part: the lowest priority's thread.
///
/// [`Watches::judge`]: session::v026g::replay_watch::Watches::judge
fn read(
    replays: &Replays,
    record_dir: &Path,
    max_bytes: u64,
    request: &WatchRequest,
) -> Result<Recording, Refused> {
    let match_id = request.match_id;
    let path = replay::recording_in(record_dir, match_id).ok_or((Status::Gone, None))?;
    let failed = |why: String| {
        log::warn!("Replay sessions: the recording of match {match_id:#x}: {why}");
        (Status::Failed, None)
    };
    let size = std::fs::metadata(&path)
        .map_err(|error| failed(error.to_string()))?
        .len();
    if size > max_bytes {
        return Err(failed(format!(
            "{size} bytes, over SRP_REPLAY_MAX_RECORDING_MB"
        )));
    }
    let notes = record::read_notes(&path).map_err(|error| failed(error.to_string()))?;
    let (players, revisions) = replay::places(&notes);
    replays
        .watches()
        .judge(request, &players, &revisions, replays.oldest())?;
    Recording::load(&path, match_id).map_err(failed)
}

/// A recording being read on its own thread, for a session.
struct Reading {
    request: WatchRequest,
    answer: Receiver<Result<Recording, Refused>>,
}

/// `SRP_DEV_REPLAY_CONTROL` (dev aid): controls written to a file, a line
/// each, to every playback.
struct DevControl {
    path: PathBuf,
    next: Instant,
}

/// The replay sessions this match server plays (the module's doc): their
/// recordings read, the games of the ones read not here yet, and those
/// that watch.
pub(crate) struct ReplayWatch {
    replays: Arc<Replays>,
    record_dir: PathBuf,
    max_recording_bytes: u64,
    /// The match port (0xE3), and the server's settings.
    port: u16,
    settings: Settings,
    reading: Option<Reading>,
    /// Read, their games not here yet: by session.
    ready: HashMap<u32, (WatchRequest, Arc<Recording>)>,
    playbacks: HashMap<SocketAddr, Playback>,
    dev_control: Option<DevControl>,
}

impl ReplayWatch {
    pub(crate) fn new(
        replays: Arc<Replays>,
        record_dir: PathBuf,
        port: u16,
        settings: &Settings,
        now: Instant,
    ) -> Self {
        let max_recording_bytes = replays
            .settings()
            .map_or(DEFAULT_MAX_RECORDING_MB << 20, |settings| {
                settings.max_recording_bytes
            });
        let dev_control = settings.replay_control.clone().map(|path| {
            log::warn!(
                "SRP_DEV_REPLAY_CONTROL: every replay takes the controls of {}",
                path.display()
            );
            DevControl { path, next: now }
        });
        Self {
            replays,
            record_dir,
            max_recording_bytes,
            port,
            settings: settings.clone(),
            reading: None,
            ready: HashMap::new(),
            playbacks: HashMap::new(),
            dev_control,
        }
    }

    /// The addresses of the games shown a recording.
    pub(crate) fn addrs(&self) -> impl Iterator<Item = SocketAddr> + '_ {
        self.playbacks.keys().copied()
    }

    /// The sessions whose games watch: seen, as a match's players are.
    pub(crate) fn sessions(&self) -> impl Iterator<Item = u32> + '_ {
        self.playbacks
            .values()
            .map(|playback| playback.spectator.session_id())
    }

    pub(crate) fn owns(&self, addr: SocketAddr) -> bool {
        self.playbacks.contains_key(&addr)
    }

    /// The game at `addr` of `session_id` connects: shown its session's
    /// recording when it was sent to watch one; else its transport back.
    pub(crate) fn connect(
        &mut self,
        addr: SocketAddr,
        session_id: u32,
        transport: Transport,
        now: Instant,
    ) -> Result<(), Box<Transport>> {
        let Some((request, start)) = self.replays.watches().connected(session_id, now) else {
            return Err(Box::new(transport));
        };
        // Its game again from another address: the same session, anew.
        let again = self
            .playbacks
            .iter()
            .find(|(_, playback)| playback.request == request)
            .map(|(addr, playback)| (*addr, Arc::clone(&playback.recording)));
        let recording = match again {
            Some((old, recording)) => {
                self.playbacks.remove(&old);
                recording
            }
            None => match self.ready.remove(&session_id) {
                Some((_, recording)) => recording,
                None => {
                    // Read by a match server before a restart.
                    self.replays.watches().ended(&request);
                    return Err(Box::new(transport));
                }
            },
        };
        log::info!(
            "Replay {session_id:#x}: account {} at {addr} watches match {:#x} from {start} ms",
            request.account_id,
            request.match_id
        );
        let spectator = Spectator::connect(
            addr,
            session_id,
            transport,
            sur::MAX_PLAYERS as u8,
            self.port,
        );
        let playback = Playback::new(
            recording,
            request,
            spectator,
            start,
            self.settings.clone(),
            now,
        );
        self.playbacks.insert(addr, playback);
        Ok(())
    }

    /// A datagram of the game at `addr`.
    pub(crate) fn receive(&mut self, addr: SocketAddr, datagram: &[u8], now: Instant) {
        let Some(playback) = self.playbacks.get_mut(&addr) else {
            return;
        };
        // One let go is silent until it is forgotten.
        if playback.spectator.is_leaving() {
            return;
        }
        if playback.receive(datagram, now).is_some()
            && let Some(playback) = self.playbacks.remove(&addr)
        {
            self.replays.watches().ended(&playback.request);
        }
    }

    /// The turn: the next recording read, each playback's datagrams, and
    /// the sessions that are over told.
    pub(crate) fn poll(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        self.take_read(now);
        self.read_next(now);
        self.dev_controls(now);
        // A session the lobby gave up (its game never came) keeps nothing.
        let watches = self.replays.watches();
        self.ready
            .retain(|_, (request, _)| watches.waits_for_game(request));
        let mut datagrams = Vec::new();
        let replays = &self.replays;
        self.playbacks.retain(|_, playback| {
            let (out, ended) = playback.poll(now);
            datagrams.extend(out);
            let kept = ended != Some(Ended::Left) && !playback.spectator.is_forgotten(now);
            if !kept {
                replays.watches().ended(&playback.request);
            }
            kept
        });
        datagrams
    }

    /// A recording read: ready for its session, or why not.
    fn take_read(&mut self, now: Instant) {
        let Some(reading) = &self.reading else {
            return;
        };
        let read = match reading.answer.try_recv() {
            Ok(read) => read,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err((Status::Failed, None)),
        };
        let request = reading.request;
        self.reading = None;
        match read {
            Ok(recording) => self.prepare(request, Arc::new(recording), now),
            Err((status, refusal)) => {
                self.replays
                    .watches()
                    .refuse(&request, status, refusal, now);
            }
        }
    }

    /// The recording read for the session: where it starts, told to the
    /// lobby, which sends its game.
    fn prepare(&mut self, request: WatchRequest, recording: Arc<Recording>, now: Instant) {
        let start = recording.start(request.at_ms, request.place);
        if self.replays.watches().loaded(&request, start, now) {
            log::info!(
                "Replay sessions: match {:#x} ready for account {}: from {start} ms of {} ms, {} \
                 key frames",
                request.match_id,
                request.account_id,
                recording.end,
                recording.worlds.len()
            );
            self.ready.insert(request.session_id, (request, recording));
        }
    }

    /// The next session's recording: one already here for another session
    /// of its match is shared (judged here, cheap); else read on its own
    /// thread, one at a time.
    fn read_next(&mut self, now: Instant) {
        if self.reading.is_some() {
            return;
        }
        let Some(request) = self.replays.watches().next_job(now) else {
            return;
        };
        let shared = self
            .ready
            .values()
            .map(|(_, recording)| recording)
            .chain(self.playbacks.values().map(|playback| &playback.recording))
            .find(|recording| recording.match_id == request.match_id)
            .cloned();
        if let Some(recording) = shared {
            match self.replays.watches().judge(
                &request,
                &recording.players,
                &recording.revisions,
                self.replays.oldest(),
            ) {
                Ok(()) => self.prepare(request, recording, now),
                Err((status, refusal)) => {
                    self.replays
                        .watches()
                        .refuse(&request, status, refusal, now);
                }
            }
            return;
        }
        let (send, answer) = channel();
        let (replays, dir, max) = (
            Arc::clone(&self.replays),
            self.record_dir.clone(),
            self.max_recording_bytes,
        );
        let spawned = std::thread::Builder::new()
            .name("srp-replay-read".into())
            .spawn(move || {
                replay::lowest_priority();
                let started = Instant::now();
                let read = read(&replays, &dir, max, &request);
                log::info!(
                    "Replay sessions: match {:#x} read in {} ms",
                    request.match_id,
                    started.elapsed().as_millis()
                );
                let _ = send.send(read);
            });
        match spawned {
            Ok(_) => self.reading = Some(Reading { request, answer }),
            Err(error) => {
                log::error!("Replay sessions: no thread to read a recording: {error}");
                self.replays
                    .watches()
                    .refuse(&request, Status::Failed, None, now);
            }
        }
    }

    fn dev_controls(&mut self, now: Instant) {
        let Some(dev) = self.dev_control.as_mut() else {
            return;
        };
        if now < dev.next {
            return;
        }
        dev.next = now + CONTROL_EVERY;
        let Ok(text) = std::fs::read_to_string(&dev.path) else {
            return;
        };
        let _ = std::fs::remove_file(&dev.path);
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            match Control::parse(line) {
                Some(control) => {
                    for playback in self.playbacks.values_mut() {
                        playback.control(control, now);
                    }
                }
                None => log::warn!("SRP_DEV_REPLAY_CONTROL: no such control: {line:?}"),
            }
        }
    }

    /// A session's game shown `recording` from `start`, as if read for it
    /// (the tests' recordings are `.sur` files of no known players).
    #[cfg(test)]
    pub(crate) fn prepare_for_test(
        &mut self,
        request: WatchRequest,
        recording: Recording,
        now: Instant,
    ) {
        self.prepare(request, Arc::new(recording), now);
    }

    #[cfg(test)]
    pub(crate) fn position(&self, session_id: u32, now: Instant) -> Option<u32> {
        self.playbacks
            .values()
            .find(|playback| playback.spectator.session_id() == session_id)
            .map(|playback| playback.position(now))
    }
}
