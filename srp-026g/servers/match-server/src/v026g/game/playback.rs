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
//! its tick) goes to the game as from a live match. It opens at the end of
//! the match's countdown, where the action is (the first key frame after an
//! entry when that is later: the game has someone to follow from its first
//! world), at the asked place's first view when that entered later, or later
//! when asked ([`Recording::start`]). Pause stops the clock; speed scales
//! it; a seek, and play after a pause, start the game again (0xE5 of its
//! next generation, the world) from the key frame at or before the time,
//! put as far past it as the game's input lists reach ([`Recording::reach`]).
//! The end of the recording lets the game go (0xEA naming no place), as
//! the end of a live match does. The controls come from the game (`replay_control::Request`), and the
//! game is told the playback (`replay_control::Playback`).

use std::path::Path;

use replay_control::{ITEMS_PER_KIT, KitsNote,
    EVENTS_PER_NOTE, Event as TimelineEvent, EventKind, EventsNote, MatchMs, Op, Playback as Note,
    Request, Speed,
};
use session::v026g::GameAccounts;
use session::v026g::persist::match_events::{self, MatchEvent, Timing};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use session::v026g::replay_watch::{DEFAULT_MAX_RECORDING_MB, WatchFrom, WatchRequest};
use session::v026g::replays::{Refusal, ReplayId, Replays, Status};
use survarium::v026g::match_msgs::client_model::EVENT_HORIZON_MS;
use survarium::v026g::match_msgs::players::{self, PlayerState};
use survarium::v026g::match_msgs::static_info::{self, KIT_USABLE, MatchProfile, NO_LOCAL_PLACE, StaticInfo};
use survarium::v026g::match_msgs::{
    ClientMessage, PlayerInput, WireTime, mask_bytes, parse_client,
};
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
/// "My deaths" starts this long before the death.
const DEATH_LEAD_MS: u32 = 4_000;
/// A paused game is pinged this often: nothing else goes to it, and a game
/// ends a match link silent for 5 s.
const PAUSED_PING_EVERY: Duration = Duration::from_secs(1);
/// A game whose reported world is this far from where the playback has it
/// ([`DRIFT_REPORTS`] in a row) is started again from a world: the
/// fallback for a game that did not freeze or scale its clock as told.
const DRIFT_MS: i64 = 2_000;
const DRIFT_REPORTS: u8 = 3;

/// A recording made into what a game is shown.
pub(crate) struct Recording {
    match_id: u32,
    /// The history's events of it, the viewer's timeline (none without a
    /// history, for a match not kept in it, or the tests' `.sur`).
    timeline: Vec<TimelineEvent>,
    /// Each death in the history's events: the victim's place and the
    /// earliest it can have been, by time.
    deaths: Vec<(u8, u32)>,
    info: StaticInfo,
    /// Each place's account, and each place's game's patch revision (the
    /// recording's notes): who may watch it, which games play it, and
    /// whose deaths a player's are.
    accounts: Vec<Option<u32>>,
    revisions: Vec<Option<u8>>,
    /// Each place's kits used, for the viewer's kit view.
    kits: Vec<PlaceKits>,
    /// The key frames, in time order.
    worlds: Vec<Snapshot>,
    /// In time order.
    inputs: Vec<(u32, usize, Relayed)>,
    /// In time order.
    events: Vec<(u32, usize, bool)>,
    /// The time of its last record.
    end: u32,
    /// When its countdown ended: the start of the time rule's "in process"
    /// ([`countdown_end`]); `None` in a recording that has none.
    countdown_end: Option<u32>,
}

impl Recording {
    /// A match's `.srprec`, converted here (with no mode config: the
    /// `.sur`'s chunk 4 is the viewer's alone).
    pub(crate) fn load(path: &Path, match_id: u32) -> Result<Self, String> {
        let notes = record::read_notes(path).map_err(|error| error.to_string())?;
        let file = sur_of(path, &notes)?;
        let mut recording = Self::from_sur(&file, match_id)?;
        (recording.accounts, recording.revisions) = replay::by_place(&notes);
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
        let countdown_end = countdown_end(&worlds, &options);
        let picks: Vec<(usize, u8)> = inputs
            .iter()
            .filter_map(|(_, player, what)| match what {
                Relayed::Profile { value, .. } => Some((*player, *value)),
                Relayed::Input(_) => None,
            })
            .collect();
        let kits = used_kits(&options.players, &first_kits(&worlds, &options.players), &picks, dictionary);
        Ok(Self {
            match_id,
            kits,
            timeline: Vec::new(),
            deaths: Vec::new(),
            info,
            accounts: Vec::new(),
            revisions: Vec::new(),
            worlds,
            inputs,
            events,
            end,
            countdown_end,
        })
    }

    /// The key frame a game shown `time` starts from: the last at or before
    /// it, else the first.
    fn world_for(&self, time: u32) -> &Snapshot {
        let after = self.worlds.partition_point(|world| world.time <= time);
        &self.worlds[after.saturating_sub(1)]
    }

    /// Where a session of `account_id` asked from `from` starts.
    ///
    /// "My deaths": [`DEATH_LEAD_MS`] before the earliest its place's first
    /// death can have been (a death known within a window, the window's
    /// start), its place in view; with none known, from where its place is
    /// first in view.
    pub(crate) fn start(&self, from: WatchFrom, account_id: u32) -> u32 {
        match from {
            WatchFrom::At { ms, place } => self.start_at(ms, place),
            WatchFrom::MyDeaths => {
                let place = self.place_of(account_id);
                let death = self
                    .deaths
                    .iter()
                    .find(|(victim, _)| Some(*victim) == place)
                    .map(|(_, earliest)| *earliest);
                let at = death.map_or(0, |earliest| earliest.saturating_sub(DEATH_LEAD_MS));
                self.start_at(at, place)
            }
        }
    }

    /// The history's events of this recording's match (`events_of_replay`
    /// of its replay id) taken as its timeline ([`timeline_of`]), and each
    /// death's earliest time, for "my deaths".
    pub(crate) fn take_events(&mut self, events: &[MatchEvent]) {
        self.timeline = timeline_of(events);
        let earliest = |timing: Timing| match timing {
            Timing::Exact(at) => at,
            Timing::Between { after, .. } => after,
        };
        self.deaths = events
            .iter()
            .filter_map(|event| match event.kind {
                match_events::EventKind::Kill {
                    victim: Some(victim),
                    ..
                }
                | match_events::EventKind::Death { victim } => {
                    Some((victim.place, earliest(event.timing)))
                }
                _ => None,
            })
            .collect();
        self.deaths.sort_by_key(|(_, at)| *at);
    }

    /// The account's place (its first, if it had two).
    fn place_of(&self, account_id: u32) -> Option<u8> {
        let place = self
            .accounts
            .iter()
            .position(|account| *account == Some(account_id))?;
        u8::try_from(place).ok()
    }

    /// Every place's account.
    fn players(&self) -> Vec<u32> {
        self.accounts.iter().flatten().copied().collect()
    }

    /// Where a session asked for `at` with `place` in view starts: the
    /// replay's opening, or the place's first view when that is later (it
    /// entered late), or `at` when that is later still; as a seek puts the
    /// game there ([`Self::seek_to`]).
    pub(crate) fn start_at(&self, at: u32, place: Option<u8>) -> u32 {
        self.seek_to(at.max(self.first_view(place)))
    }

    /// The first key frame after the place's first entry (anyone's when it
    /// never entered, or none is asked). A world before any entry has nobody
    /// in it: the game would follow no one.
    fn first_view(&self, place: Option<u8>) -> u32 {
        let entry = |wanted: Option<usize>| {
            self.events
                .iter()
                .find(|&&(_, who, entered)| entered && wanted.is_none_or(|place| who == place))
                .map(|&(time, _, _)| time)
        };
        entry(place.map(usize::from))
            .or_else(|| entry(None))
            .map_or(self.worlds[0].time, |entered| {
                self.worlds
                    .iter()
                    .find(|world| world.time > entered)
                    .unwrap_or_else(|| self.world_for(entered))
                    .time
            })
    }

    /// Where the replay opens for a viewer: the end of the countdown, where
    /// the action is (the first view of anyone when that is later, or the
    /// recording has no countdown end). Its bar starts there, and no seek
    /// goes before it.
    pub(crate) fn opening(&self) -> u32 {
        let view = self.first_view(None);
        self.countdown_end.map_or(view, |end| end.max(view))
    }

    /// Where a game sought to `time` is put: from the key frame at or before
    /// it (never before [`Self::opening`]), as far as [`Self::reach`] lets
    /// it.
    fn seek_to(&self, time: u32) -> u32 {
        let time = time.max(self.opening());
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

    /// A recording with no countdown's end.
    #[cfg(test)]
    pub(crate) fn forget_countdown(&mut self) {
        self.countdown_end = None;
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

/// When the match's countdown ended: the start of the time rule's "in
/// process" in the first key frame that is in it.
fn countdown_end(worlds: &[Snapshot], options: &static_info::MatchOptions) -> Option<u32> {
    let mode = options.map.mode().rules();
    let batteries = options.map.map().victory_items_count as usize;
    // A match that lets players in late writes 20 places' rules.
    let rosters = [options.players.len(), sur::MAX_PLAYERS];
    worlds.iter().find_map(|world| {
        let timelimit = rosters
            .iter()
            .find_map(|&players| {
                snapshot::decode(world.time, &world.body, players, mode, batteries)?.rules
            })?
            .timelimit;
        (timelimit.state == rules::state::IN_PROCESS)
            .then_some(timelimit.start)
            .flatten()
    })
}

/// Match ms in `elapsed` of the wall clock at `speed`.
fn played(speed: Speed, elapsed: Duration) -> u32 {
    (elapsed.as_millis() * u128::from(speed.percent()) / 100) as u32
}

/// A line of the dev control file (`SRP_DEV_REPLAY_CONTROL`): `pause`,
/// `play`, `speed 0.25|0.5|1|2|4|8`, `seek <seconds>`.
pub(crate) fn dev_control(line: &str) -> Option<Op> {
    let mut words = line.split_whitespace();
    let op = match (words.next()?, words.next()) {
        ("pause", None) => Op::Pause(true),
        ("play", None) => Op::Pause(false),
        ("speed", Some(factor)) => {
            let factor: f64 = factor.parse().ok()?;
            Op::Speed(Speed::of_percent((factor * 100.0).round() as u16)?)
        }
        ("seek", Some(seconds)) => {
            let seconds: f64 = seconds.parse().ok().filter(|s: &f64| *s >= 0.0)?;
            Op::Seek(MatchMs((seconds * 1000.0) as u32))
        }
        _ => return None,
    };
    words.next().is_none().then_some(op)
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
    /// The `seq` of the last request taken from its game.
    last_request: Option<u8>,
    /// The playback note its game was last sent since it came live (or
    /// started again): `None`, the note and the events are due.
    told: Option<Note>,
    /// The last note's revision (the game takes only a newer one).
    revision: u8,
    /// Its reports in a row far from the playback ([`DRIFT_MS`]).
    drift: u8,
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
            last_request: None,
            told: None,
            revision: 0,
            drift: 0,
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
                at + played(self.speed, now.saturating_duration_since(since))
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
            until: u32::MAX,
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
                && let Some(request) = Request::parse(&payload)
            {
                let newer = self
                    .last_request
                    .is_none_or(|last| replay_control::newer(request.seq, last));
                if newer && !self.spectator.is_leaving() {
                    self.last_request = Some(request.seq);
                    self.control(request.op, now);
                }
                continue;
            }
            if let Ok(ClientMessage::Hash { tick, .. }) = parse_client(id, &payload) {
                self.judge_drift(tick, now);
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

    fn control(&mut self, op: Op, now: Instant) {
        let at = self.position(now);
        log::info!(
            "Replay {:#x} of match {:#x}: {op:?} at {at} ms",
            self.spectator.session_id(),
            self.recording.match_id
        );
        match op {
            Op::Pause(true) => {
                if let Clock::Playing { .. } | Clock::Loading { .. } = self.clock {
                    self.clock = Clock::Paused {
                        at,
                        ping: now + PAUSED_PING_EVERY,
                    };
                }
            }
            // Its game's clock stood still too (the viewer's patch pauses
            // it): the relays go on from where they stopped. One whose
            // clock ran on is caught by its reports ([`DRIFT_MS`]).
            Op::Pause(false) => {
                if let Clock::Paused { at, .. } = self.clock {
                    self.clock = Clock::Playing { since: now, at };
                }
            }
            Op::Speed(speed) => {
                if let Clock::Playing { .. } = self.clock {
                    self.clock = Clock::Playing { since: now, at };
                }
                self.speed = speed;
            }
            Op::Seek(time) => self.seek(time.0, now),
            // As at the recording's end: its game goes back to the lobby
            // on the farewell (patch 121), then the session is over.
            Op::Leave => self.spectator.farewell(at, now),
        }
    }

    /// A hash report of its game (its event-horizon world, [`EVENT_HORIZON_MS`]
    /// behind its clock) against the playback: [`DRIFT_REPORTS`] in a row
    /// [`DRIFT_MS`] off and it starts again from where the playback is.
    fn judge_drift(&mut self, tick: u8, now: Instant) {
        if !self.spectator.is_live() {
            self.drift = 0;
            return;
        }
        let at = self.position(now);
        let Some(reported) = tick_time(tick, at) else {
            return;
        };
        let off = i64::from(reported) + i64::from(EVENT_HORIZON_MS) - i64::from(at);
        if off.abs() < DRIFT_MS {
            self.drift = 0;
            return;
        }
        self.drift += 1;
        if self.drift >= DRIFT_REPORTS {
            log::info!(
                "Replay {:#x}: its game is {off} ms off the playback at {at} ms; started again",
                self.spectator.session_id()
            );
            self.drift = 0;
            self.seek(at, now);
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
        self.told = None;
    }

    /// The playback note (and, once live anew, the events) when its game
    /// is live and was not told this playback yet: after its first world,
    /// each change and each start again.
    fn tell(&mut self) {
        if !self.spectator.is_live() {
            self.told = None;
            return;
        }
        let paused = matches!(self.clock, Clock::Paused { .. });
        let fresh = self.told.is_none();
        if self
            .told
            .is_some_and(|told| told.paused == paused && told.speed == self.speed)
        {
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        let note = Note {
            revision: self.revision,
            paused,
            speed: self.speed,
            start: MatchMs(self.recording.opening()),
            end: MatchMs(self.recording.end),
        };
        self.spectator.note(&note.encode());
        if fresh {
            // The timeline, in notes of up to 64 (an empty one for none).
            let timeline = &self.recording.timeline;
            let total = timeline.len().min(usize::from(u16::MAX)) as u16;
            let mut buffer = [0; 6 + 7 * EVENTS_PER_NOTE];
            let mut first = 0;
            loop {
                let chunk = &timeline[usize::from(first)..]
                    [..usize::from(total - first).min(EVENTS_PER_NOTE)];
                if let Some(len) = EventsNote::encode(first, total, chunk, &mut buffer) {
                    self.spectator.note(&buffer[..len]);
                }
                first += chunk.len() as u16;
                if first >= total {
                    break;
                }
            }
            // Each place's kits used: the viewer's kit view.
            let mut buffer = [0; KITS_NOTE_ROOM];
            for place in &self.recording.kits {
                let names: Vec<(u8, Vec<&[u8]>)> = place
                    .kits
                    .iter()
                    .map(|(kit, items)| (*kit, items.iter().map(|name| name.as_bytes()).collect()))
                    .collect();
                let kits: Vec<(u8, &[&[u8]])> = names.iter().map(|(kit, items)| (*kit, items.as_slice())).collect();
                if let Some(len) = KitsNote::encode(place.place, &kits, &mut buffer) {
                    self.spectator.note(&buffer[..len]);
                } else {
                    log::warn!("Replay: place {}'s kits do not fit their note", place.place);
                }
            }
        }
        self.told = Some(note);
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
        self.tell();
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

/// The `.sur` a replay plays of a match's `.srprec` (its notes read):
/// what `sur_convert` makes of it with no mode config (the `.sur`'s chunk
/// 4 is the viewer's alone) and as many key frames as a file may hold
/// (more places to seek to). Replay sessions play it; Save's file holds it
/// (`super::super::replay_file`).
pub(crate) fn sur_of(path: &Path, notes: &[record::Record]) -> Result<Vec<u8>, String> {
    let settings = sur_convert::Settings {
        key_frame_every_ms: 1_000,
        ..sur_convert::Settings::default()
    };
    let (file, _) = sur_convert::convert_file(path, notes, &[], settings)
        .map_err(|error| error.to_string())?;
    Ok(file)
}

/// The history's events of a match as a replay's timeline: kills and
/// deaths (a death no kill was paired with, no actor), batteries found and
/// delivered.
pub(crate) fn timeline_of(events: &[MatchEvent]) -> Vec<TimelineEvent> {
    events
        .iter()
        .filter_map(|event| {
            let time = MatchMs(event.timing.at());
            let (kind, actor, victim) = match event.kind {
                match_events::EventKind::Kill { killer, victim } => {
                    (EventKind::Kill, Some(killer.place), victim.map(|v| v.place))
                }
                match_events::EventKind::Death { victim } => {
                    (EventKind::Kill, None, Some(victim.place))
                }
                match_events::EventKind::BatteryFound { by }
                | match_events::EventKind::BatteryDelivered { by } => {
                    (EventKind::Objective, Some(by.place), None)
                }
                match_events::EventKind::Capture { .. }
                | match_events::EventKind::Score { .. }
                | match_events::EventKind::End { .. } => return None,
            };
            Some(TimelineEvent {
                time,
                kind,
                actor,
                victim,
            })
        })
        .collect()
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
    accounts: Option<&GameAccounts>,
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
    let mut recording = Recording::load(&path, match_id).map_err(failed)?;
    // Its file's stem is its replay id: the history's match it is.
    let replay_id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(ReplayId::parse);
    if let (Some(history), Some(replay_id)) = (accounts.and_then(GameAccounts::history), replay_id)
    {
        match history.events_of_replay(&replay_id) {
            Ok(Some(kept)) => recording.take_events(&kept.events),
            Ok(None) => {}
            Err(error) => log::warn!(
                "Replay sessions: the events of {} not read: {error}",
                replay_id.as_str()
            ),
        }
    }
    Ok(recording)
}

/// A kits note's room: three kits of [`ITEMS_PER_KIT`] items of the longest name.
const KITS_NOTE_ROOM: usize = 4 + 3 * (2 + ITEMS_PER_KIT * (1 + replay_control::ITEM_NAME_MAX));

/// A place's kits used in the match, by number, with their items' names (the viewer's kit view).
#[derive(Clone, Debug, PartialEq, Eq)]
struct PlaceKits {
    place: u8,
    kits: Vec<(u8, Vec<String>)>,
}

/// Each place's starting kit: the kit its player object has in the first key frame that holds it
/// (`current_index`, players.rs). None for a place in no key frame the parser reads.
fn first_kits(worlds: &[Snapshot], profiles: &[MatchProfile]) -> Vec<Option<u8>> {
    let mut first = vec![None; profiles.len()];
    for world in worlds {
        let Some(blocks) = players::blocks(&world.body) else {
            continue;
        };
        for block in blocks {
            let place = usize::from(block.place);
            let Some(profile) = profiles.get(place) else {
                continue;
            };
            if first[place].is_none() {
                first[place] = PlayerState::read(block.state, profile.items.len()).map(|head| head.current_index);
            }
        }
        if first.iter().all(Option::is_some) {
            break;
        }
    }
    first
}

/// The kits each place used: the kit it started with and every kit it picked (an accepted kit
/// choice, `event.profiles`), as the history counts a kit used, never another. A kit the
/// profile cannot play is left out; a place with no used kit has no entry. The item names,
/// in the kit's order and once each, are those a kits note carries.
fn used_kits(
    profiles: &[MatchProfile],
    first: &[Option<u8>],
    picks: &[(usize, u8)],
    dictionary: &Dictionary,
) -> Vec<PlaceKits> {
    let mut out = Vec::new();
    for (place, profile) in profiles.iter().enumerate() {
        let picked = picks.iter().filter(|(player, _)| *player == place).map(|(_, kit)| *kit);
        let kits: std::collections::BTreeSet<u8> = first.get(place).copied().flatten().into_iter().chain(picked).collect();
        let mut used = Vec::new();
        for number in kits {
            let Some(kit) = profile.kits.get(usize::from(number)).filter(|kit| kit.state == KIT_USABLE) else {
                continue;
            };
            let mut names: Vec<String> = Vec::new();
            for &(item, count) in kit.slots.iter().flatten() {
                if count == 0 {
                    continue;
                }
                let name = profile
                    .items
                    .get(usize::from(item))
                    .and_then(|item| dictionary.item(item.dict_id))
                    .map(|item| item.name.clone())
                    .filter(|name| replay_control::is_item_name(name.as_bytes()));
                if let Some(name) = name
                    && !names.contains(&name)
                    && names.len() < ITEMS_PER_KIT
                {
                    names.push(name);
                }
            }
            used.push((number, names));
        }
        if !used.is_empty() {
            out.push(PlaceKits { place: place as u8, kits: used });
        }
    }
    out
}

/// A session's recording read: its game not here yet.
struct Prepared {
    request: WatchRequest,
    recording: Arc<Recording>,
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
    ready: HashMap<u32, Prepared>,
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
            .watches()
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
            .map(|(addr, _)| *addr);
        let prepared = match again.and_then(|old| self.playbacks.remove(&old)) {
            Some(old) => Prepared {
                request,
                recording: old.recording,
            },
            None => match self.ready.remove(&session_id) {
                Some(prepared) => prepared,
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
        // The place asked to be in view (the camera follows it), as a live
        // match's spectator is told it.
        let view = match request.from {
            WatchFrom::At { place, .. } => place,
            WatchFrom::MyDeaths => prepared.recording.place_of(request.account_id),
        };
        let spectator = Spectator::connect(
            addr,
            session_id,
            transport,
            sur::MAX_PLAYERS as u8,
            self.port,
            view,
        );
        let playback = Playback::new(
            prepared.recording,
            prepared.request,
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
            .retain(|_, prepared| watches.waits_for_game(&prepared.request));
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
        let start = recording.start(request.from, request.account_id);
        if self.replays.watches().loaded(&request, start, now) {
            log::info!(
                "Replay sessions: match {:#x} ready for account {}: from {start} ms of {} ms, {} \
                 key frames",
                request.match_id,
                request.account_id,
                recording.end,
                recording.worlds.len()
            );
            self.ready
                .insert(request.session_id, Prepared { request, recording });
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
            .map(|prepared| &prepared.recording)
            .chain(self.playbacks.values().map(|playback| &playback.recording))
            .find(|recording| recording.match_id == request.match_id)
            .cloned();
        if let Some(recording) = shared {
            match self.replays.watches().judge(
                &request,
                &recording.players(),
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
        let (replays, accounts, dir, max) = (
            Arc::clone(&self.replays),
            self.settings.accounts.clone(),
            self.record_dir.clone(),
            self.max_recording_bytes,
        );
        let spawned = std::thread::Builder::new()
            .name("srp-replay-read".into())
            .spawn(move || {
                replay::lowest_priority();
                let started = Instant::now();
                let accounts = accounts.as_ref().map(|accounts| &*accounts.0);
                let read = read(&replays, accounts, &dir, max, &request);
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
            match dev_control(line) {
                Some(op) => {
                    for playback in self.playbacks.values_mut() {
                        playback.control(op, now);
                    }
                }
                None => log::warn!("SRP_DEV_REPLAY_CONTROL: no such control: {line:?}"),
            }
        }
    }

    /// A session's game shown `recording` as if read for it, with the
    /// history's `events` of it (the tests' recordings are `.sur` files of
    /// no known players and no history).
    #[cfg(test)]
    pub(crate) fn prepare_for_test(
        &mut self,
        request: WatchRequest,
        mut recording: Recording,
        events: &[MatchEvent],
        now: Instant,
    ) {
        recording.take_events(events);
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

#[cfg(test)]
mod kit_tests {
    use super::*;
    use survarium::dictionary::PROFILE_SLOTS;
    use survarium::v026g::match_msgs::static_info::{MatchItem, MatchKit};
    use survarium::v026g::skills::MODIFIERS;

    /// A profile of three kits, each holding the dictionary's item at `dict_ids[n]`, and a fourth kit slot unused.
    fn profile(dict_ids: [u16; 3]) -> MatchProfile {
        let items = dict_ids
            .iter()
            .enumerate()
            .map(|(at, &dict_id)| MatchItem {
                instance_id: at as u32 + 1,
                dict_id,
                condition_or_stack: 1,
                repair_time: 0,
                reroll_time: 0,
                modifications: Vec::new(),
                upgrades: Vec::new(),
            })
            .collect();
        let kit = |item: u16, state: u8| {
            let mut slots = [None; PROFILE_SLOTS];
            slots[0] = Some((item, 1));
            MatchKit { state, id: u32::from(item) + 1, name: String::new(), slots, modifiers: [0.0; MODIFIERS] }
        };
        MatchProfile {
            team: 0,
            name: "p".into(),
            clan_tag: String::new(),
            protection: 0,
            kits: vec![kit(0, KIT_USABLE), kit(1, KIT_USABLE), kit(2, 2)],
            items,
            squad: 0,
        }
    }

    fn dict_with_names() -> (&'static Dictionary, [u16; 3], [String; 3]) {
        let dictionary = Dictionary::shared();
        let mut named = dictionary.items.iter().filter(|item| replay_control::is_item_name(item.name.as_bytes()));
        let mut next = || named.next().expect("an item with a plain name");
        let (a, b, c) = (next(), next(), next());
        (dictionary, [a.dict_id, b.dict_id, c.dict_id], [a.name.clone(), b.name.clone(), c.name.clone()])
    }

    #[test]
    fn only_the_starting_and_picked_kits_are_used_and_a_locked_kit_is_never_shown() {
        let (dictionary, ids, names) = dict_with_names();
        let profiles = [profile(ids), profile(ids)];
        // Place 0 started in kit 0 and picked 1 and the locked 2; place 1 started in 1, no picks.
        let used = used_kits(&profiles, &[Some(0), Some(1)], &[(0, 1), (0, 2), (0, 1)], dictionary);
        assert_eq!(used.len(), 2);
        assert_eq!(used[0], PlaceKits { place: 0, kits: vec![(0, vec![names[0].clone()]), (1, vec![names[1].clone()])] });
        assert_eq!(used[1], PlaceKits { place: 1, kits: vec![(1, vec![names[1].clone()])] });
    }

    #[test]
    fn a_place_unseen_and_without_picks_has_no_entry() {
        let (dictionary, ids, _) = dict_with_names();
        let profiles = [profile(ids)];
        assert_eq!(used_kits(&profiles, &[None], &[], dictionary), vec![]);
    }

    #[test]
    fn the_note_of_a_recordings_kits_parses() {
        let (dictionary, ids, names) = dict_with_names();
        let used = used_kits(&[profile(ids)], &[Some(1)], &[], dictionary);
        let items: Vec<&[u8]> = used[0].kits[0].1.iter().map(|name| name.as_bytes()).collect();
        let mut room = [0; KITS_NOTE_ROOM];
        let len = KitsNote::encode(used[0].place, &[(used[0].kits[0].0, items.as_slice())], &mut room).unwrap();
        let Some(replay_control::ServerNote::Kits(note)) = replay_control::ServerNote::parse(&room[..len]) else {
            panic!();
        };
        let kit = note.kits().next().unwrap();
        assert_eq!((note.place, kit.kit, kit.items().next()), (0, 1, Some(names[1].as_bytes())));
    }
}
