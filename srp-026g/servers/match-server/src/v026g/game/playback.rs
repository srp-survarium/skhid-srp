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
//! ([`relay_lead`] ahead, as a live game gets an input a little before
//! its tick) goes to the game as from a live match. It opens at the end of
//! the match's countdown, where the action is (the first key frame after an
//! entry when that is later: the game has someone to follow from its first
//! world), at the asked place's first view when that entered later, or later
//! when asked ([`Recording::start`]). It waits there until the world goes
//! with its 0xE5, then runs from that time as the game's clock does, through
//! the load too. Pause stops the clock; speed scales
//! it; a seek, and play after a pause, start the game again (0xE5 of its
//! next generation, the world; no sooner than `spectate::START_EVERY` after
//! the last start) from the key frame at or before the time,
//! put as far past it as the game's input lists reach ([`Recording::reach`]).
//! The end of the recording lets the game go (0xEA naming no place), as
//! the end of a live match does. The controls come from the game (`replay_control::Request`), and the
//! game is told the playback (`replay_control::Playback`).

use std::path::Path;

use replay_control::{KitsNote,
    EVENTS_PER_NOTE, Event as TimelineEvent, EventKind, EventsNote, MatchMs, Op, Playback as Note,
    Request, Speed,
};
use session::v026g::GameAccounts;
use session::v026g::persist::match_events::{self, MatchEvent};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use session::v026g::replay_watch::{
    DEFAULT_MAX_RECORDING_MB, WatchFrom, WatchRefusal, WatchRequest,
};
use session::v026g::replays::{ReplayId, Replays};
use survarium::v026g::match_msgs::client_model::EVENT_HORIZON_MS;
use survarium::v026g::match_msgs::static_info::{self, NO_LOCAL_PLACE, StaticInfo};
use survarium::v026g::match_msgs::{
    ClientMessage, PlayerInput, WireTime, mask_bytes, parse_client,
};
use udp_transport::table::{Direction, Table};
use udp_transport::wire;

use super::kit_view::all_kits;
use super::spectate::{Heard, Spectator, Starts, Watched};
use super::*;
use crate::v026g::sur::{self, record_type};
use crate::v026g::{record, replay, sur_convert};

/// What is due this long past the playback clock goes already: a live game
/// gets a player's input about this much before its tick (the server's
/// event horizon, [`HORIZON_MS`]). At a higher speed as much more
/// ([`relay_lead`]): its way and a lost copy's take real time, and the
/// game's clock runs that much faster meanwhile.
const RELAY_LEAD_MS: u32 = HORIZON_MS;
/// ... but no more than this: a game moves an input more than
/// [`TOO_NEW_MS`] ahead of its clock back, and a relay may get there sooner
/// than the 0xE5 its clock was set by did (1 s of that limit to spare, 125 ms
/// of the way at 8x).
const RELAY_LEAD_MOST_MS: u32 = TOO_NEW_MS - 1_000;
/// The playback clock runs from the 0xE5 a world goes with, through the
/// load, as the game's does: what comes due in a load this long (real time)
/// goes to the game's rings too, and [`Recording::reach`] leaves room for it.
const LOAD_MS: u32 = 500;
/// A paused game is pinged this often: nothing else goes to it, and a game
/// ends a match link silent for 5 s.
const PAUSED_PING_EVERY: Duration = Duration::from_secs(1);
/// A game paused this long is shown the recording's end: its session is one
/// of a few ([`DEFAULT_MAX_SESSIONS`]) and nobody at the keyboard would keep
/// the others' out for as long as the game stays open.
///
/// [`DEFAULT_MAX_SESSIONS`]: session::v026g::replay_watch::DEFAULT_MAX_SESSIONS
pub(crate) const PAUSED_MOST: Duration = Duration::from_secs(5 * 60);
/// A game whose reported world is this far from where the playback has it
/// ([`DRIFT_REPORTS`] in a row) is started again from a world: the
/// fallback for a game that did not freeze or scale its clock as told. No
/// more often than a live match's spectator is for being out of step
/// (`Spectator::take_drift_restart`).
const DRIFT_MS: i64 = 2_000;
const DRIFT_REPORTS: u8 = 3;
/// A session lasts at most its recording's length and this (paused, sought
/// back), or twice its length when that is more: then it is shown the end,
/// as at the recording's. Its session is one of a few.
const SESSION_SPARE: Duration = Duration::from_secs(15 * 60);

/// A recording made into what a game is shown.
pub(crate) struct Recording {
    match_id: u32,
    /// The history's events of it, the viewer's timeline (none without a
    /// history, for a match not kept in it, or the tests' `.sur`).
    timeline: Vec<TimelineEvent>,
    info: StaticInfo,
    /// Each place's account, and each place's game's patch revision (the
    /// recording's notes): who may watch it and which games play it.
    accounts: Vec<Option<u32>>,
    revisions: Vec<Option<u8>>,
    /// Every place's playable kits, for the viewer's kit view.
    kits: Vec<KitsNote>,
    /// The key frames, in time order.
    worlds: Vec<Snapshot>,
    /// Each key frame's 0xE8 (past its generation byte), packed the first
    /// time a game starts from it and shared by every session after: no
    /// bigger than its body, and only of the frames started from.
    sent_worlds: Vec<OnceLock<Vec<u8>>>,
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
        let file = sur_of(path, &notes, match_id)?;
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
        let kits = all_kits(&options.players);
        Ok(Self {
            match_id,
            kits,
            timeline: Vec::new(),
            info,
            accounts: Vec::new(),
            revisions: Vec::new(),
            sent_worlds: worlds.iter().map(|_| OnceLock::new()).collect(),
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

    /// Where a session asked from `from` starts.
    pub(crate) fn start(&self, from: WatchFrom) -> u32 {
        self.start_at(from.ms, from.place)
    }

    /// The history's events of this recording's match (`events_of_replay`
    /// of its replay id) taken as its timeline ([`timeline_of`]).
    pub(crate) fn take_events(&mut self, events: &[MatchEvent]) {
        self.timeline = timeline_of(events);
    }

    /// Every place's account.
    fn players(&self) -> Vec<u32> {
        self.accounts.iter().flatten().copied().collect()
    }

    /// Where a session asked for `at` with `place` in view starts: the
    /// replay's opening, or the place's first view when that is later (it
    /// entered late), or `at` when that is later still; as a seek puts the
    /// game there ([`Self::seek_to`], at the speed a session starts at).
    pub(crate) fn start_at(&self, at: u32, place: Option<u8>) -> u32 {
        self.seek_to(at.max(self.first_view(place)), ring_ahead(Speed::Normal))
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
    /// it with its rings holding what comes `ahead` ([`ring_ahead`]).
    fn seek_to(&self, time: u32, ahead: u32) -> u32 {
        let time = time.max(self.opening());
        let from = self.world_for(time).time;
        self.reach(from, time.min(self.end).max(from), ahead)
    }

    /// How far past the world at `from` a game started from it can be put,
    /// at most `to`: its inputs since the world come with it, read against
    /// the clock it is put at (a tick byte reaches 6.35 s: no more than
    /// [`TICK_REACH_MS`] back), and a game keeps [`INPUT_RING`] of a
    /// player's (one fewer while the world is on its way,
    /// `INPUT_RING_SAFE`); one older than those would be played wrong. So no
    /// player may have more inputs in (`from`, the time and `ahead`]: what
    /// is due with the world and what comes due while it loads. (One that
    /// loads slower is started again, `Spectator::ring_overrun`.)
    fn reach(&self, from: u32, to: u32, ahead: u32) -> u32 {
        let to = to.min(from + TICK_REACH_MS);
        let mut counts = [0usize; sur::MAX_PLAYERS];
        let first = self.inputs.partition_point(|(at, _, _)| *at <= from);
        for &(at, player, what) in &self.inputs[first..] {
            if at > to + ahead {
                break;
            }
            if let (Relayed::Input(_), Some(count)) = (what, counts.get_mut(player)) {
                *count += 1;
                if *count > INPUT_RING_SAFE {
                    return to.min(at.saturating_sub(ahead + 1)).max(from);
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

    /// The longest a session of it lasts ([`SESSION_SPARE`]).
    pub(crate) fn session_most(&self) -> Duration {
        let length = Duration::from_millis(self.end.into());
        (length + SESSION_SPARE).max(length * 2)
    }

    /// How many of its inputs and roster changes are due by `time`, `lead`
    /// ahead ([`relay_lead`]).
    fn due(&self, time: u32, lead: u32) -> (usize, usize) {
        let horizon = time + lead;
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

/// How far past the playback clock what is due goes at `speed`
/// ([`RELAY_LEAD_MS`]): as much real time ahead as at 1x (never less), at
/// most [`RELAY_LEAD_MOST_MS`].
fn relay_lead(speed: Speed) -> u32 {
    (RELAY_LEAD_MS * u32::from(speed.percent()) / 100).clamp(RELAY_LEAD_MS, RELAY_LEAD_MOST_MS)
}

/// How far past the time a game is put at its rings must hold each player's
/// inputs at `speed`: what goes with its world ([`relay_lead`]) and what
/// comes due while the world loads ([`LOAD_MS`]).
fn ring_ahead(speed: Speed) -> u32 {
    relay_lead(speed) + played(speed, Duration::from_millis(LOAD_MS.into()))
}

/// The playback clock: match time, by the wall clock.
#[derive(Clone, Copy, Debug)]
enum Clock {
    /// No world has gone to its game yet (it loads its level, or starts
    /// again from one): the clock waits at `at`, where [`Recording::reach`]
    /// put it, until the world goes with its 0xE5 of that time; from then it
    /// plays, as the game's own clock runs from that 0xE5 through the load
    /// (`match.md` section 27).
    Starting {
        at: u32,
    },
    /// At `at` at `since`, running at the playback's speed.
    Playing {
        since: Instant,
        at: u32,
    },
    /// Stopped at `at`; its game pinged next at `ping`.
    /// `since`: when it paused (a seek keeps it: [`PAUSED_MOST`] counts
    /// from there).
    Paused { at: u32, ping: Instant, since: Instant },
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
    /// The lobby called its player back (`Watches::recall`): its game is
    /// told (`ServerNote::Recall`) and let go at `recall_at`.
    recalled: bool,
    recall_at: Option<Instant>,
    speed: Speed,
    /// The `seq` of the last request taken from its game.
    last_request: Option<u8>,
    /// The playback note its game was last sent since it came live (or
    /// started again): `None`, the note and the events are due.
    told: Option<Note>,
    /// The last note's revision (the game takes only a newer one).
    revision: u8,
    /// The serials of the notes of its last tell: the playback note, and
    /// the timeline's and the kits' of the last fresh one. A newer tell
    /// makes them of no use, and they go no more ([`Spectator::unsend`]):
    /// one kept until acked would pile up in a game that asks for changes
    /// and acks nothing.
    playback_note: Option<u64>,
    timeline_notes: Vec<u64>,
    /// Its reports in a row far from the playback ([`DRIFT_MS`]).
    drift: u8,
    /// The latest seek its game asked for while a world was on its way to
    /// it, or before it may start again (`Spectator::may_start`): taken
    /// once that world is in or given up and it may ([`Self::poll`]), so
    /// that its game starts again at most once a world.
    sought: Option<u32>,
    /// How many of the recording's inputs and roster changes have gone to
    /// it as they came due (those since its world go with the world).
    inputs: usize,
    events: usize,
    /// When its session's game first connected (again from another address,
    /// the same): the session ends [`Recording::session_most`] after.
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
        created: Instant,
    ) -> Self {
        let (inputs, events) = recording.due(start, relay_lead(Speed::Normal));
        Self {
            recording,
            request,
            spectator,
            clock: Clock::Starting { at: start },
            sought: None,
            recalled: false,
            recall_at: None,
            speed: Speed::Normal,
            last_request: None,
            told: None,
            revision: 0,
            playback_note: None,
            timeline_notes: Vec::new(),
            drift: 0,
            inputs,
            events,
            created,
            settings,
        }
    }

    /// The playback clock.
    fn position(&self, now: Instant) -> u32 {
        match self.clock {
            Clock::Starting { at } | Clock::Paused { at, .. } => at,
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
            places: usize::from(recording.info.roster()),
            roster: usize::from(recording.info.roster()),
            worlds: recording.worlds.iter().collect(),
            sent_worlds: &recording.sent_worlds,
            inputs: &recording.inputs[..self.inputs],
            events: &recording.events[..self.events],
            until: None,
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
            self.follow_world(now);
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
        log::debug!(
            "Replay {:#x} of match {:#x}: {op:?} at {at} ms",
            self.spectator.session_id(),
            self.recording.match_id
        );
        match op {
            Op::Pause(true) => {
                if let Clock::Playing { .. } | Clock::Starting { .. } = self.clock {
                    self.clock = Clock::Paused {
                        at,
                        ping: now + PAUSED_PING_EVERY,
                        since: now,
                    };
                }
            }
            // Its game's clock stood still too (the viewer's patch pauses
            // it): the relays go on from where they stopped. One whose
            // clock ran on is caught by its reports ([`DRIFT_MS`]).
            Op::Pause(false) => {
                if let Clock::Paused { at, .. } = self.clock {
                    self.clock = if self.spectator.world_sent() {
                        Clock::Playing { since: now, at }
                    } else {
                        Clock::Starting { at }
                    };
                }
            }
            Op::Speed(speed) => {
                if let Clock::Playing { .. } = self.clock {
                    self.clock = Clock::Playing { since: now, at };
                }
                self.speed = speed;
            }
            // Taken at its next turn, or once the world on its way is in.
            Op::Seek(time) => self.sought = Some(time.0),
            // As at the recording's end: its game goes back to the lobby
            // on the farewell (patch 143), then the session is over.
            Op::Leave => self.spectator.farewell(at, now),
        }
    }

    /// A hash report of its game (its event-horizon world, [`EVENT_HORIZON_MS`]
    /// behind its clock) against the playback: [`DRIFT_REPORTS`] in a row
    /// [`DRIFT_MS`] off and it starts again from where the playback is, once
    /// it may (`Spectator::may_start`, and as rarely as a live match's
    /// spectator); until then each report more off tries again.
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
        self.drift = self.drift.saturating_add(1);
        if self.drift >= DRIFT_REPORTS
            && self.spectator.may_start(now)
            && self
                .spectator
                .take_drift_restart(self.settings.timing.resync_interval, now)
        {
            log::info!(
                "Replay {:#x}: its game is {off} ms off the playback at {at} ms; started again",
                self.spectator.session_id()
            );
            self.drift = 0;
            self.restart(at);
        }
    }

    /// The game starts again from the key frame at or before `time`, put as
    /// far past it as it can be; its clock waits there until that world goes
    /// (a paused one stays paused, since it paused).
    fn restart(&mut self, time: u32) {
        let lead = relay_lead(self.speed);
        let at = self.recording.seek_to(time, ring_ahead(self.speed));
        self.clock = match self.clock {
            Clock::Starting { .. } | Clock::Playing { .. } => Clock::Starting { at },
            Clock::Paused { ping, since, .. } => Clock::Paused { at, ping, since },
        };
        (self.inputs, self.events) = self.recording.due(at, lead);
        self.spectator.wait_again();
        self.told = None;
    }

    /// Its world went with its 0xE5 of the time the clock waits at: the
    /// clock runs from now, as its game's does from that 0xE5.
    fn follow_world(&mut self, now: Instant) {
        if let Clock::Starting { at } = self.clock
            && self.spectator.world_sent()
        {
            self.clock = Clock::Playing { since: now, at };
        }
    }

    /// The playback note (and, once live anew, the events) when its game
    /// is live and was not told this playback yet: after its first world,
    /// each change and each start again.
    fn tell(&mut self, now: Instant) {
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
        if let Some(serial) = self.playback_note.take() {
            self.spectator.unsend(serial, now);
        }
        self.playback_note = self.spectator.note(&note.encode());
        if fresh {
            for serial in std::mem::take(&mut self.timeline_notes) {
                self.spectator.unsend(serial, now);
            }
            // The timeline, in notes of up to 64 (an empty one for none).
            let timeline = &self.recording.timeline;
            let total = timeline.len().min(usize::from(u16::MAX)) as u16;
            let mut buffer = [0; 6 + 7 * EVENTS_PER_NOTE];
            let mut first = 0;
            loop {
                let chunk = &timeline[usize::from(first)..]
                    [..usize::from(total - first).min(EVENTS_PER_NOTE)];
                if let Some(len) = EventsNote::encode(first, total, chunk, &mut buffer) {
                    self.timeline_notes
                        .extend(self.spectator.note(&buffer[..len]));
                }
                first += chunk.len() as u16;
                if first >= total {
                    break;
                }
            }
            // Every place's kits: the viewer's chooser.
            for place in &self.recording.kits {
                self.timeline_notes
                    .extend(self.spectator.note(&place.encode()));
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
        // A seek once no world is on its way and the last start again is
        // far enough behind: one start again a world, the latest asked for.
        if (self.spectator.is_live() || self.spectator.is_waiting())
            && self.spectator.may_start(now)
            && let Some(time) = self.sought.take()
        {
            self.restart(time);
        }
        self.follow_world(now);
        match self.clock {
            Clock::Paused { at, ping, since } if now >= ping => {
                self.spectator.ping();
                self.clock = Clock::Paused {
                    at,
                    ping: now + PAUSED_PING_EVERY,
                    since,
                };
            }
            _ => {}
        }
        let at = self.position(now);
        let mut ended = None;
        let paused_out = matches!(
            self.clock,
            Clock::Paused { since, .. } if now.saturating_duration_since(since) >= PAUSED_MOST
        );
        // Called back: told once, and let go `RECALL_SECONDS` later.
        if self.recalled && self.recall_at.is_none() {
            log::info!("Replay {session_id:#x}: its player's room calls it back");
            self.spectator.note(&replay_control::ServerNote::recall(replay_control::RECALL_SECONDS));
            self.recall_at = Some(now + Duration::from_secs(replay_control::RECALL_SECONDS.into()));
        }
        let recall_due = self.recall_at.is_some_and(|due| now >= due);
        let flooded = self.spectator.flooded();
        let too_long = now.saturating_duration_since(self.created) >= self.recording.session_most();
        if (at >= self.recording.end || paused_out || too_long || recall_due || flooded)
            && !self.spectator.is_leaving()
        {
            log::info!(
                "Replay {session_id:#x}: {}",
                if flooded {
                    "its game acks too little of what it is sent; let go"
                } else if recall_due {
                    "let go, back to its player's room"
                } else if paused_out {
                    "paused too long, its session goes to someone else"
                } else if too_long {
                    "its session ran its longest, it goes to someone else"
                } else {
                    "the recording is over"
                }
            );
            self.spectator.farewell(at, now);
            ended = Some(Ended::Over);
        }
        let recording = Arc::clone(&self.recording);
        // Never fewer than have gone: the lead shrinks with the speed.
        let (inputs, events) = recording.due(at, relay_lead(self.speed));
        let (inputs, events) = (inputs.max(self.inputs), events.max(self.events));
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
            self.spectator
                .roster_change(index, entered, time, &message, now);
        }
        for (time, from, what) in &recording.inputs[self.inputs..inputs] {
            self.spectator
                .relay(*from, &relay(*from, what), *time, at, now);
        }
        (self.inputs, self.events) = (inputs, events);
        // Back to wait for a world by itself while it played (a relay of a
        // place it lacks): from where the playback is, as far as its rings
        // let it be put.
        if let Clock::Playing { .. } = self.clock
            && self.spectator.is_waiting()
        {
            self.restart(at);
        }
        let at = self.position(now);
        let echo = now.saturating_duration_since(self.created).as_millis() as u16;
        let start = recording.world_for(at).time;
        self.spectator
            .step(&self.watched(&recording, at), Some(start), echo, now);
        let after = self.spectator.stage();
        if after != before {
            log::debug!("Replay {session_id:#x}: {before} -> {after} at {at} ms");
        }
        self.tell(now);
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
/// (`super::super::replay_file`). Uploaded worlds the world check refused as
/// key frames are said in the log.
pub(crate) fn sur_of(
    path: &Path,
    notes: &[record::Record],
    match_id: u32,
) -> Result<Vec<u8>, String> {
    let settings = sur_convert::Settings {
        key_frame_every_ms: 1_000,
        ..sur_convert::Settings::default()
    };
    let (file, report) =
        sur_convert::convert_file(path, notes, &[], settings).map_err(|error| error.to_string())?;
    if let Some((time, why)) = report.refusals.first() {
        log::warn!(
            "Replays: match {match_id:#x}: {} uploaded worlds of its recording are no key \
             frames, the world check refused the first at {time} ms: {why}",
            report.worlds_untaken
        );
    }
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
) -> Result<Recording, WatchRefusal> {
    let match_id = request.match_id;
    let path = replay::recording_in(record_dir, match_id).ok_or(WatchRefusal::Gone)?;
    let failed = |why: String| {
        log::warn!("Replay sessions: the recording of match {match_id:#x}: {why}");
        WatchRefusal::Failed
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
        match history.site().events_of_replay(&replay_id) {
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

/// A session's recording read: its game not here yet.
struct Prepared {
    request: WatchRequest,
    recording: Arc<Recording>,
}

/// A recording being read on its own thread, for a session.
struct Reading {
    request: WatchRequest,
    answer: Receiver<Result<Recording, WatchRefusal>>,
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
}

impl ReplayWatch {
    pub(crate) fn new(
        replays: Arc<Replays>,
        record_dir: PathBuf,
        port: u16,
        settings: &Settings,
    ) -> Self {
        let max_recording_bytes = replays
            .watches()
            .settings()
            .map_or(DEFAULT_MAX_RECORDING_MB << 20, |settings| {
                settings.max_recording_bytes
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

    /// The game at `addr` was let go: silent until it is forgotten.
    pub(crate) fn is_let_go(&self, addr: SocketAddr) -> bool {
        self.playbacks
            .get(&addr)
            .is_some_and(|playback| playback.spectator.is_leaving())
    }

    /// Forget the game at `addr` at once (its address is a new
    /// connection's now): its session is over.
    pub(crate) fn forget(&mut self, addr: SocketAddr) {
        if let Some(playback) = self.playbacks.remove(&addr) {
            self.replays.watches().ended(&playback.request);
        }
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
        // Its length counts from its first connection, and its starts from a
        // world go on from its last (each packs one).
        let (prepared, created, starts) = match again.and_then(|old| self.playbacks.remove(&old)) {
            Some(old) => (
                Prepared {
                    request,
                    recording: old.recording,
                },
                old.created,
                old.spectator.into_starts(),
            ),
            None => match self.ready.remove(&session_id) {
                Some(prepared) => (prepared, now, Starts::default()),
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
        let view = request.from.place;
        let spectator = Spectator::connect(
            addr,
            session_id,
            transport,
            sur::MAX_PLAYERS as u8,
            self.port,
            view,
            starts,
        );
        let playback = Playback::new(
            prepared.recording,
            prepared.request,
            spectator,
            start,
            self.settings.clone(),
            created,
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
        // A session the lobby gave up (its game never came) keeps nothing.
        let watches = self.replays.watches();
        self.ready
            .retain(|_, prepared| watches.waits_for_game(&prepared.request));
        let mut datagrams = Vec::new();
        let replays = &self.replays;
        self.playbacks.retain(|_, playback| {
            // Its player's room wants it back: let go as at the recording's end.
            playback.recalled = playback.recalled || replays.watches().is_recalled(&playback.request);
            let (out, ended) = playback.poll(now);
            datagrams.extend(out);
            let kept = ended != Some(Ended::Left)
                && !playback.spectator.is_forgotten(now)
                && replays.watches().is_open(&playback.request);
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
            Err(TryRecvError::Disconnected) => Err(WatchRefusal::Failed),
        };
        let request = reading.request;
        self.reading = None;
        match read {
            Ok(recording) => self.prepare(request, Arc::new(recording), now),
            Err(refusal) => self.replays.watches().refuse(&request, refusal, now),
        }
    }

    /// The recording read for the session: where it starts, told to the
    /// lobby, which sends its game.
    fn prepare(&mut self, request: WatchRequest, recording: Arc<Recording>, now: Instant) {
        let start = recording.start(request.from);
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
                Err(refusal) => self.replays.watches().refuse(&request, refusal, now),
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
                    .refuse(&request, WatchRefusal::Failed, now);
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

    /// `count` notes more for the session's game (each kept until acked).
    #[cfg(test)]
    pub(crate) fn note_for_test(&mut self, session_id: u32, count: usize) {
        for playback in self.playbacks.values_mut() {
            if playback.spectator.session_id() == session_id {
                for _ in 0..count {
                    playback
                        .spectator
                        .note(&replay_control::ServerNote::view(0));
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn backlog(&self, session_id: u32) -> Option<usize> {
        self.playbacks
            .values()
            .find(|playback| playback.spectator.session_id() == session_id)
            .map(|playback| playback.spectator.backlog())
    }

    #[cfg(test)]
    pub(crate) fn position(&self, session_id: u32, now: Instant) -> Option<u32> {
        self.playbacks
            .values()
            .find(|playback| playback.spectator.session_id() == session_id)
            .map(|playback| playback.position(now))
    }
}
