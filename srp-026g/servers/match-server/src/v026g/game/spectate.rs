//! Spectators: games that watch the match from a room's queue ("Queue and
//! spectate", `docs/notes/queue-spectate.md`, `match.md` section 36).
//!
//! A spectator is no player. It has no roster place, so it is kept apart
//! from the places: not a [`Peer`] of `Game::peers` (indexed by place, beside
//! every per-place vector), but a [`Spectator`] of `Game::spectators`, found
//! by its address. Nothing of the match's own machinery (hashes, uploads,
//! admission, the start, the end, the results, the live view) looks at it, so
//! nothing of it can hold a player up or change what a player sees: it is
//! only ever sent things.
//!
//! What it gets is what a newcomer gets, on a short path of its own:
//! - the ports (0xE3), then on its probe the static info with no place of its
//!   own (0xE4, first byte `NO_LOCAL_PLACE`) and the whole roster;
//! - the places appended later, as every game is asked to install them
//!   (`late_join`'s `Prepare`), and acked the same way;
//! - once it has loaded (0xDA) and has every place, a trusted world young
//!   enough to start from: 0xE5 of its next generation until it answers,
//!   then 0xE5 again with the inputs and entries since the world and the
//!   world (0xE8);
//! - from then on every relay and roster change the players get (a leave
//!   only after its world is loaded: the client's 0xEA handler reads the
//!   match's view, `Game::roster_change`).
//!
//! What it sends is heard only for its own handshake and its own step:
//! inputs, kit choices and uploads are dropped (it would have no inputs
//! anyway; a world of its own is no witness), and its hash reports are judged
//! against the players' alone, never one of theirs (`judge_spectators`). Its
//! own 0xE0, or its link gone, is its
//! game leaving by itself: reported (`take_spectators_left`), so the lobby
//! takes it out of its queue.
//!
//! Let go by the server (its watch ended: its place came, it left the queue;
//! the match over), not reported: one 0xEA naming no place
//! (`NO_LOCAL_PLACE`), then silence for [`SILENCE`]. A link closed by the
//! server ends nothing in the game (0xFD reaches `close_current_match` as
//! reason 4, its return, `match.md` section 24), and an 0xEA ends a game's
//! match only for its own place (0x27C72C: with no local place the handler
//! returns). A stock game takes this 0xEA harmlessly: `leave_match`
//! 0x2D18A0 tests bit `0xFF & 31` of the entered mask, never set, and drops
//! it (0x2D1B8D..0x2D1B9D), and so does a game with the spectator patch (103)
//! as it stands; one that takes it as its end would end at once. What ends
//! the game's match today is the silence: its match link ends after 5 s of
//! it (`close_current_match(0)`: the world ended, no results screen,
//! `match.md` section 34). A game of its session seated meanwhile (its place
//! came) connects again from that address as a player (`Server::receive`).
//!
//! It starts again from a world (back to wait for one) when its reports say
//! it is out of step, as rarely as a player is resynced; when a relay or
//! roster change of a place it has not installed comes; when a world is not
//! loaded in [`WORLD_WAIT`] or an 0xE5 not answered in [`TIMING_WAIT`].

use survarium::v026g::match_msgs::static_info::NO_LOCAL_PLACE;

use super::*;
use survarium::v026g::match_msgs::id;
use survarium::v026g::match_msgs::late_join::{Prepare, Prepared, Proposal, Status};

/// An 0xE5 not answered in this is given up: the spectator waits for a
/// world again.
const TIMING_WAIT: Duration = Duration::from_secs(5);
/// A world not acked in this: the same.
const WORLD_WAIT: Duration = Duration::from_secs(15);
/// After its farewell a spectator is sent nothing, and what it sends is
/// swallowed, for this long: more than the 5 s of silence that end a game's
/// match link (`match.md` section 34). Then it is forgotten.
const SILENCE: Duration = Duration::from_secs(7);
/// A farewell is sent (its resends too) for this long before the silence.
const FAREWELL_GOES: Duration = Duration::from_millis(250);
/// A place's install is asked again after this.
const PREPARE_RETRY: Duration = Duration::from_secs(1);
/// A spectator's hash reports kept for judging, at most.
const REPORTS_KEPT: usize = 64;
/// Its reports in a row the players' majority contradicts before it is
/// started anew from a world: one alone may be a tick it played before its
/// world was in.
const DRIFT_REPORTS: u32 = 2;

/// Where a spectator's game is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Watch {
    /// Sent 0xE3; waits for the port probe.
    Connecting,
    /// Sent 0xE4; its level loads until it is ready (0xDA).
    Loading,
    /// Loaded: waits for a world it can start from.
    Waiting,
    /// A world (the snapshot at `time`) is there: 0xE5 of its generation
    /// with this echo, again every [`SET_TIME_EVERY`], until it answers.
    Timing {
        time: u32,
        echo: u16,
        last: Instant,
        since: Instant,
    },
    /// Its world (`serial`, the snapshot at `time`) is on its way.
    Syncing {
        serial: u64,
        time: u32,
        since: Instant,
    },
    /// It has the world (the snapshot at `from`): gets every relay and roster
    /// change. `wrong`: its reports in a row that the players' majority
    /// contradicts ([`Game::judge_spectators`]).
    Live { from: u32, wrong: u32 },
    /// Let go by the server: its farewell sent, silent until `until`, then
    /// forgotten.
    Leaving { quiet_from: Instant, until: Instant },
}

/// What a spectator at an address is to the server's routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpectatorAt {
    /// It watches: its datagrams are its own.
    Watching,
    /// Let go, silent: its datagrams are swallowed, unless its session (the
    /// one here) was seated meanwhile and its game connects as a player.
    LetGo(u32),
}

/// A place's install asked of the spectator's game.
struct Installing {
    proposal: Proposal,
    sent: Instant,
}

pub(crate) struct Spectator {
    addr: SocketAddr,
    session_id: u32,
    transport: Transport,
    stage: Watch,
    generation: Generation,
    /// The places its game has: the static info's, then each it installed.
    installed_roster: usize,
    installing: Option<Installing>,
    /// Its hash reports not judged yet, while it is live: tick time, CRC
    /// ([`REPORTS_KEPT`] at most).
    reports: VecDeque<(u32, u32)>,
    /// When it was started anew for drifting, the latest
    /// [`RESYNCS_PER_MINUTE`].
    restarts: VecDeque<Instant>,
}

impl Game {
    /// The game at `addr` of `session_id` connects as a spectator (its
    /// session watches this match: the lobby's Queue and spectate).
    pub(crate) fn watch(
        &mut self,
        addr: SocketAddr,
        session_id: u32,
        mut transport: Transport,
        now: Instant,
    ) {
        if self.finished_at.is_some() || !self.growing_roster() {
            transport.disconnect(now);
            return;
        }
        // One game a session, one session an address.
        self.spectators
            .retain(|spectator| spectator.addr != addr && spectator.session_id != session_id);
        transport.set_roster_size(self.snapshot_players() as u8);
        let ports = ServerMessage::Ports {
            min: self.port,
            max: self.port,
        };
        let _ = transport.send(ports.id(), &ports.payload());
        log::info!(
            "Match {:#x}: a spectator (session {session_id:#x}) connected from {addr}",
            self.id
        );
        self.spectators.push(Spectator {
            addr,
            session_id,
            transport,
            stage: Watch::Connecting,
            generation: Generation::AT_READY,
            installed_roster: 0,
            installing: None,
            reports: VecDeque::new(),
            restarts: VecDeque::new(),
        });
    }

    /// The spectator at `addr`, if any: one that watches takes its
    /// datagrams; one let go swallows them (silence) unless its game
    /// connects again from there as a player.
    pub(crate) fn spectator_at(&self, addr: SocketAddr) -> Option<SpectatorAt> {
        self.spectators
            .iter()
            .find(|spectator| spectator.addr == addr)
            .map(|spectator| match spectator.stage {
                Watch::Leaving { .. } => SpectatorAt::LetGo(spectator.session_id),
                _ => SpectatorAt::Watching,
            })
    }

    /// Forget the spectator at `addr` at once (its address is a new
    /// connection's now).
    pub(crate) fn drop_spectator(&mut self, addr: SocketAddr) {
        self.spectators.retain(|spectator| spectator.addr != addr);
    }

    /// The sessions of the spectators not being let go.
    pub(crate) fn spectator_sessions(&self) -> Vec<u32> {
        self.spectators
            .iter()
            .filter(|spectator| !spectator.is_leaving())
            .map(|spectator| spectator.session_id)
            .collect()
    }

    /// The spectators whose games left by themselves since the last call:
    /// their sessions.
    pub(crate) fn take_spectators_left(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.spectators_left)
    }

    /// The server lets go of the spectator of `session_id` (its watch
    /// ended): its farewell, then its link closes. Not reported as left.
    pub(crate) fn let_spectator_go(&mut self, session_id: u32, now: Instant) {
        let clock = self.clock(now);
        for spectator in &mut self.spectators {
            if spectator.session_id == session_id && !spectator.is_leaving() {
                log::info!("Match {:#x}: spectator {session_id:#x} is let go", self.id);
                spectator.farewell(clock, now);
            }
        }
    }

    /// A datagram from the spectator at `addr`.
    pub(crate) fn spectator_receive(&mut self, addr: SocketAddr, datagram: &[u8], now: Instant) {
        let Some(at) = self
            .spectators
            .iter()
            .position(|spectator| spectator.addr == addr)
        else {
            return;
        };
        let events = self.spectators[at].transport.receive(datagram, now);
        for event in events {
            match event {
                Event::Message { id, payload } => self.spectator_message(at, id, &payload, now),
                Event::Disconnected => self.spectator_gone(at),
            }
            if at >= self.spectators.len() || self.spectators[at].addr != addr {
                return;
            }
        }
    }

    fn spectator_message(&mut self, at: usize, message_id: u8, payload: &[u8], now: Instant) {
        if self.spectators[at].is_leaving() {
            return;
        }
        // 0xE0 carries a place's install ack, an upload's chunk, or (empty)
        // the game's own leave.
        if message_id == id::LEAVE
            && let Ok(ack) = Prepared::decode(payload)
        {
            self.spectator_installed(at, ack, now);
            return;
        }
        let Ok(message) = parse_client(message_id, payload) else {
            return;
        };
        match message {
            ClientMessage::PortProbe { .. } => {
                let spectator = &mut self.spectators[at];
                if spectator.stage == Watch::Connecting {
                    let info = self.formed.info.spectator_message();
                    let _ = spectator.transport.send(info.id(), &info.payload());
                    spectator.installed_roster = self.formed.places.len();
                    spectator.stage = Watch::Loading;
                }
            }
            ClientMessage::Ready => {
                let spectator = &mut self.spectators[at];
                if spectator.stage == Watch::Loading {
                    // The client set its own generation to 0xFF with it.
                    spectator.generation = Generation::AT_READY;
                    spectator.stage = Watch::Waiting;
                    log::info!(
                        "Match {:#x}: spectator {:#x} has loaded",
                        self.id,
                        spectator.session_id
                    );
                }
            }
            ClientMessage::TimeReply(reply) => {
                if let Watch::Timing { time, echo, .. } = self.spectators[at].stage
                    && reply.stamp == echo
                {
                    self.spectator_world(at, time, echo, now);
                }
            }
            ClientMessage::Leave => {
                log::info!(
                    "Match {:#x}: spectator {:#x} left by its own word (0xE0)",
                    self.id,
                    self.spectators[at].session_id
                );
                self.spectator_gone(at);
            }
            // Heard only to find it out of step (`judge_spectators`): never
            // one of the match's reports.
            ClientMessage::Hash { tick, crc } => {
                let clock = self.clock(now);
                let spectator = &mut self.spectators[at];
                if let Watch::Live { from, .. } = spectator.stage
                    && crc != FORCED_MISMATCH
                    && let Some(time) = tick_time(tick, clock)
                    && time > from
                    && time <= clock + HASH_MAX_AHEAD_MS
                {
                    if spectator.reports.len() == REPORTS_KEPT {
                        spectator.reports.pop_front();
                    }
                    spectator.reports.push_back((time, crc));
                }
            }
            // Not heard from a spectator: it is no player and no witness.
            ClientMessage::Connect { .. }
            | ClientMessage::Input(_)
            | ClientMessage::ProfileInput { .. }
            | ClientMessage::Chunk(_)
            | ClientMessage::KeepAlive => {}
        }
    }

    /// The spectator's game left by itself: gone, and reported.
    fn spectator_gone(&mut self, at: usize) {
        let spectator = self.spectators.remove(at);
        if !spectator.is_leaving() {
            self.spectators_left.push(spectator.session_id);
            log::info!(
                "Match {:#x}: spectator {:#x} is gone",
                self.id,
                spectator.session_id
            );
        }
    }

    fn spectator_installed(&mut self, at: usize, ack: Prepared, now: Instant) {
        let id = self.id;
        let spectator = &mut self.spectators[at];
        if spectator
            .installing
            .as_ref()
            .is_none_or(|installing| installing.proposal != ack.proposal)
        {
            return;
        }
        match ack.status {
            Status::Installed => {
                spectator.installed_roster = usize::from(ack.proposal.place) + 1;
                spectator.installing = None;
            }
            Status::Busy => {}
            Status::Failed => {
                // Its game cannot have the place: it can watch no more. Its
                // queue place is the lobby's to take.
                log::warn!(
                    "Match {id:#x}: spectator {:#x} failed to install place {}; it is let go",
                    spectator.session_id,
                    ack.proposal.place
                );
                // Out of its queue as one gone, and let go.
                self.spectators_left.push(spectator.session_id);
                let clock = self.clock(now);
                self.spectators[at].farewell(clock, now);
            }
        }
    }

    /// A trusted world the spectator may start from now: final, young
    /// enough for its 1-byte ticks, taken when every game had every place
    /// (`after_preparation`) and no game is installing one (a world with a
    /// place's kit objects is another than one without).
    fn spectator_start(&self, clock: u32) -> Option<Snapshot> {
        if self.installing() {
            return None;
        }
        let first_time = self.clock_start.map(|(_, at)| at);
        let recent = self.recent.as_ref().filter(|recent| {
            self.live() <= 1 || self.vouchers(recent.time, snapshot::crc(&recent.body)) >= 2
        });
        [self.snapshot.as_ref(), recent]
            .into_iter()
            .flatten()
            .filter(|world| {
                (0..self.players()).all(|place| self.after_preparation(place, world.time))
            })
            .filter(|world| clock.saturating_sub(world.time) <= FRESH_SNAPSHOT_MS)
            .filter(|world| Some(world.time) == first_time || world.time + HORIZON_MS <= clock)
            .max_by_key(|world| world.time)
            .cloned()
    }

    /// The spectator answered its 0xE5: the time again, the inputs and the
    /// entries since the world, and the world.
    fn spectator_world(&mut self, at: usize, time: u32, echo: u16, now: Instant) {
        let clock = self.clock(now);
        let Some(world) = [self.snapshot.as_ref(), self.recent.as_ref()]
            .into_iter()
            .flatten()
            .find(|world| world.time == time)
            .cloned()
        else {
            self.spectators[at].stage = Watch::Waiting;
            return;
        };
        let replay: Vec<ServerMessage> = self
            .inputs
            .iter()
            .filter(|(at, _, _)| *at > time)
            .map(|(_, from, what)| relay(*from, what))
            .collect();
        let entries: Vec<ServerMessage> = self
            .events
            .iter()
            .filter(|(at, _, entered)| *entered && *at > time)
            .map(|&(at, who, _)| ServerMessage::PlayerEntered {
                tick: tick_byte(at),
                player: who as u8,
            })
            .collect();
        let lifetime = entry_lifetime(self.settings.timing.entry_state, clock, clock);
        let compress = self.settings.compress_state;
        let id = self.id;
        let spectator = &mut self.spectators[at];
        let generation = spectator.generation.0;
        spectator.send_time(generation, echo, clock);
        for message in replay.iter().chain(&entries) {
            spectator.send_waiting(message);
        }
        let payload = if compress {
            ServerMessage::packed_snapshot(generation, world.time, &world.body)
        } else {
            ServerMessage::snapshot(generation, world.time, &world.body)
        }
        .payload();
        let sent = match lifetime {
            Some(lifetime) => spectator
                .transport
                .send_lasting(id::SNAPSHOT, &payload, lifetime),
            None => spectator.transport.send(id::SNAPSHOT, &payload),
        };
        match sent {
            Ok(serial) => {
                log::info!(
                    "Match {id:#x}: spectator {:#x} starts from the snapshot at {} ms \
                     (generation {generation}) at {clock} ms, {} inputs since",
                    spectator.session_id,
                    world.time,
                    replay.len()
                );
                spectator.stage = Watch::Syncing {
                    serial,
                    time: world.time,
                    since: now,
                };
            }
            Err(_) => spectator.stage = Watch::Waiting,
        }
    }

    /// A relay of the place `from` went to the players: to the spectators
    /// too (`Game::relay_to_others`).
    pub(super) fn relay_to_spectators(
        &mut self,
        from: usize,
        message: &ServerMessage,
        time: u32,
        now: Instant,
    ) {
        let clock = self.clock(now);
        for spectator in &mut self.spectators {
            match spectator.stage {
                Watch::Live { .. } | Watch::Syncing { .. }
                    if from >= spectator.installed_roster =>
                {
                    spectator.wait_again();
                }
                Watch::Live { .. } => {
                    let until = relay_deadline(0, clock, time, now);
                    let _ = spectator
                        .transport
                        .send_until(message.id(), &message.payload(), until);
                }
                Watch::Syncing {
                    time: from_time, ..
                } if time > from_time => {
                    spectator.send_waiting(message);
                }
                _ => {}
            }
        }
    }

    /// A roster change of the place `index` at `at` went to the players: to
    /// the spectators too. An entry also to one whose world is on its way;
    /// a leave only to one that has its world (`Game::roster_change`).
    pub(super) fn roster_to_spectators(
        &mut self,
        index: usize,
        entered: bool,
        at: u32,
        message: &ServerMessage,
    ) {
        for spectator in &mut self.spectators {
            match spectator.stage {
                Watch::Live { .. } | Watch::Syncing { .. }
                    if index >= spectator.installed_roster =>
                {
                    spectator.wait_again();
                }
                Watch::Live { .. } => {
                    let _ = spectator.transport.send(message.id(), &message.payload());
                }
                Watch::Syncing { time, .. } if entered && at > time => {
                    spectator.send_waiting(message);
                }
                _ => {}
            }
        }
    }

    /// The match is over: its spectators are let go with its players.
    pub(super) fn spectators_finish(&mut self, now: Instant) {
        let clock = self.clock(now);
        for spectator in &mut self.spectators {
            if !spectator.is_leaving() {
                spectator.farewell(clock, now);
            }
        }
    }

    /// The spectators' turn of `Game::poll`: their installs, their syncs,
    /// their timeouts; then their datagrams.
    pub(super) fn poll_spectators(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        let clock = self.clock(now);
        let timing = self.settings.timing.clone();
        // Over with no end of its own (everyone left, or nobody came back):
        // the spectators are let go before the match is closed (`is_over`
        // waits for them).
        if self.finished_at.is_none() && self.over_unfinished(now) {
            self.spectators_finish(now);
        }
        self.judge_spectators(now);
        let mut gone = Vec::new();
        for at in 0..self.spectators.len() {
            let stage = self.spectators[at].stage;
            let timeout = match stage {
                Watch::Connecting | Watch::Loading => timing.loading_timeout,
                _ => timing.client_timeout,
            };
            if !matches!(stage, Watch::Leaving { .. })
                && self.spectators[at].transport.is_gone_after(now, timeout)
            {
                gone.push(at);
                continue;
            }
            self.prepare_for_spectator(at, now);
            let start = match stage {
                Watch::Waiting
                    if self.spectators[at].installed_roster >= self.players()
                        && self.clock_start.is_some() =>
                {
                    let start = self.spectator_start(clock);
                    if start.is_none() {
                        self.request_upload(usize::MAX, now, RESYNC_REQUEST_GAP);
                    }
                    start
                }
                _ => None,
            };
            let leaves: Vec<ServerMessage> = match stage {
                Watch::Syncing { time, .. } => self
                    .events
                    .iter()
                    .filter(|(at, _, entered)| !*entered && *at > time)
                    .map(|&(at, who, _)| ServerMessage::PlayerLeft {
                        tick: tick_byte(at),
                        player: who as u8,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let echo = self.mono(now) as u16;
            let spectator = &mut self.spectators[at];
            match stage {
                Watch::Waiting => {
                    if let Some(world) = start {
                        spectator.generation = spectator.generation.next();
                        spectator.send_time(spectator.generation.0, echo, clock);
                        spectator.stage = Watch::Timing {
                            time: world.time,
                            echo,
                            last: now,
                            since: now,
                        };
                    }
                }
                Watch::Timing {
                    time,
                    echo,
                    last,
                    since,
                } => {
                    if now.saturating_duration_since(since) >= TIMING_WAIT {
                        spectator.wait_again();
                    } else if now.saturating_duration_since(last) >= SET_TIME_EVERY {
                        spectator.send_time(spectator.generation.0, echo, clock);
                        spectator.stage = Watch::Timing {
                            time,
                            echo,
                            last: now,
                            since,
                        };
                    }
                }
                Watch::Syncing {
                    serial,
                    time,
                    since,
                } => {
                    if spectator.transport.is_delivered(serial) {
                        for message in &leaves {
                            let _ = spectator.transport.send(message.id(), &message.payload());
                        }
                        spectator.stage = Watch::Live {
                            from: time,
                            wrong: 0,
                        };
                        log::info!(
                            "Match {:#x}: spectator {:#x} watches",
                            self.id,
                            spectator.session_id
                        );
                    } else if now.saturating_duration_since(since) >= WORLD_WAIT {
                        spectator.wait_again();
                    }
                }
                Watch::Connecting | Watch::Loading | Watch::Live { .. } | Watch::Leaving { .. } => {
                }
            }
        }
        for at in gone.into_iter().rev() {
            self.spectator_gone(at);
        }
        // One let go: its farewell goes, then silence.
        let mut datagrams = Vec::new();
        for spectator in &mut self.spectators {
            if matches!(spectator.stage, Watch::Leaving { quiet_from, .. } if now >= quiet_from) {
                continue;
            }
            let addr = spectator.addr;
            datagrams.extend(
                spectator
                    .transport
                    .poll(now)
                    .into_iter()
                    .map(|datagram| (addr, datagram)),
            );
        }
        self.spectators.retain(
            |spectator| !matches!(spectator.stage, Watch::Leaving { until, .. } if now >= until),
        );
        datagrams
    }

    /// Each live spectator's reports against the players': a tick the match
    /// has judged (`hashes_checked_until`) and its players' majority names
    /// is right or wrong; one with no majority, or gone from the reports
    /// kept, decides nothing. [`DRIFT_REPORTS`] wrong in a row: it is
    /// started anew from a world (its next generation), as a player out of
    /// step is resynced, and no more often: [`RESYNCS_PER_MINUTE`] in
    /// [`RESYNC_WINDOW`], `Timing::resync_interval` apart. Its reports never
    /// go into `Game::hashes`: nothing of the match reads them.
    fn judge_spectators(&mut self, now: Instant) {
        let checked = self.hashes_checked_until;
        let keep_from = self.clock(now).saturating_sub(HASH_KEEP_MS);
        let interval = self.settings.timing.resync_interval;
        for at in 0..self.spectators.len() {
            let Watch::Live { from, mut wrong } = self.spectators[at].stage else {
                continue;
            };
            let reports = std::mem::take(&mut self.spectators[at].reports);
            let mut kept = VecDeque::new();
            for (time, crc) in reports {
                if time < keep_from {
                    continue;
                }
                if time >= checked {
                    kept.push_back((time, crc));
                    continue;
                }
                match self.majority(time) {
                    Some(right) if right == crc => wrong = 0,
                    Some(_) => wrong += 1,
                    None => {}
                }
            }
            let id = self.id;
            let spectator = &mut self.spectators[at];
            spectator.reports = kept;
            spectator.stage = Watch::Live { from, wrong };
            if wrong < DRIFT_REPORTS {
                continue;
            }
            while spectator
                .restarts
                .front()
                .is_some_and(|at| now.saturating_duration_since(*at) >= RESYNC_WINDOW)
            {
                spectator.restarts.pop_front();
            }
            let busy = spectator.restarts.len() >= RESYNCS_PER_MINUTE
                || spectator
                    .restarts
                    .back()
                    .is_some_and(|at| now.saturating_duration_since(*at) < interval);
            if busy {
                continue;
            }
            log::info!(
                "Match {id:#x}: spectator {:#x} is out of step ({wrong} reports in a row); it \
                 starts again from a world",
                spectator.session_id
            );
            spectator.restarts.push_back(now);
            spectator.wait_again();
        }
    }

    /// The next place the spectator's game lacks, asked as every game is
    /// (`late_join`'s `poll_roster`), once its level is loaded.
    fn prepare_for_spectator(&mut self, at: usize, now: Instant) {
        let count = self.players();
        let spectator = &self.spectators[at];
        if matches!(
            spectator.stage,
            Watch::Connecting | Watch::Loading | Watch::Leaving { .. }
        ) || spectator.installed_roster >= count
            || spectator.installing.as_ref().is_some_and(|installing| {
                now.saturating_duration_since(installing.sent) < PREPARE_RETRY
            })
        {
            return;
        }
        let place = spectator.installed_roster;
        if self.admissions[place]
            .as_ref()
            .is_some_and(Admission::install_stopped)
        {
            return;
        }
        let proposal = match &spectator.installing {
            Some(installing) => installing.proposal,
            None => {
                let Some(serial) = self.next_proposal.checked_add(1) else {
                    return;
                };
                self.next_proposal = serial;
                Proposal {
                    match_id: super::super::registry::static_id(self.id),
                    serial,
                    place: place as u8,
                }
            }
        };
        let prepare = Prepare {
            proposal,
            profile: self.formed.profiles[place].clone(),
        };
        let Ok(bytes) = prepare.encode(Dictionary::shared()) else {
            return;
        };
        let spectator = &mut self.spectators[at];
        let _ = spectator.transport.send(id::STATIC_INFO, &bytes);
        spectator.installing = Some(Installing {
            proposal,
            sent: now,
        });
    }

    #[cfg(test)]
    pub(crate) fn spectator_generation(&self, session_id: u32) -> Option<u8> {
        self.spectators
            .iter()
            .find(|spectator| spectator.session_id == session_id)
            .map(|spectator| spectator.generation.0)
    }

    #[cfg(test)]
    pub(crate) fn spectator_watches(&self, session_id: u32) -> bool {
        self.spectators.iter().any(|spectator| {
            spectator.session_id == session_id && matches!(spectator.stage, Watch::Live { .. })
        })
    }
}

impl Spectator {
    fn is_leaving(&self) -> bool {
        matches!(self.stage, Watch::Leaving { .. })
    }

    /// 0xE5 of `generation`: its game's clock at the match clock.
    fn send_time(&mut self, generation: u8, echo: u16, clock: u32) {
        let set_time = ServerMessage::SetTime {
            generation,
            echo,
            match_time: clock,
        };
        let _ = self.transport.send(set_time.id(), &set_time.payload());
    }

    /// Kept until acked, at the pace of a game without a world.
    fn send_waiting(&mut self, message: &ServerMessage) {
        let _ =
            self.transport
                .send_kept_every(message.id(), &message.payload(), WAITING_RELAY_RESEND);
    }

    /// Back to wait for a world: its next start is of a new generation.
    fn wait_again(&mut self) {
        self.stage = Watch::Waiting;
        self.reports.clear();
    }

    /// The server lets it go: an 0xEA naming no place, its game's end, sent
    /// for [`FAREWELL_GOES`]; then silence (see the module).
    fn farewell(&mut self, clock: u32, now: Instant) {
        let farewell = ServerMessage::PlayerLeft {
            tick: tick_byte(clock),
            player: NO_LOCAL_PLACE,
        };
        let _ = self.transport.send(farewell.id(), &farewell.payload());
        self.stage = Watch::Leaving {
            quiet_from: now + FAREWELL_GOES,
            until: now + SILENCE,
        };
    }
}
