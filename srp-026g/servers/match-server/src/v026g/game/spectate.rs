//! Spectators: games that watch the match from a room's queue ("Queue and
//! spectate", `docs/notes/queue-spectate.md`, `match.md` section 37) or the
//! lobby's Live window, behind the match by the Live delay: one rule for all
//! live watching.
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
//!   enough to start from (the worlds the match keeps, behind its delay:
//!   it never asks the players for one): 0xE5 of its next generation until
//!   it answers, then 0xE5 again with the inputs and entries since the
//!   world and the world (0xE8);
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
//! returns), so the spectator patch (121, `player_left.local`) takes an 0xEA
//! naming 0xFF as its game's end: `close_current_match(0)`, the world
//! ended, no results screen. The silence after it is for a game whose 0xEA
//! was lost: its match link ends after 5 s of it, the same way (`match.md`
//! section 34). A game without the patch takes the 0xEA harmlessly
//! (`leave_match` 0x2D18A0 tests bit `0xFF & 31` of the entered mask, never
//! set, and drops it, 0x2D1B8D..0x2D1B9D) and ends by the silence. A game of its session seated meanwhile (its place
//! came) connects again from that address as a player (`Server::receive`).
//!
//! It starts again from a world (back to wait for one) when its reports say
//! it is out of step, as rarely as a player is resynced; when a relay or
//! roster change of a place it has not installed comes; when a world is not
//! loaded in [`WORLD_WAIT`] or an 0xE5 not answered in [`TIMING_WAIT`].

use survarium::v026g::match_msgs::static_info::{MatchProfile, NO_LOCAL_PLACE, StaticInfo};

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
/// A spectator whose queue holds more messages than this is let go. It is
/// over what a game that acks at all can leave there (the relays of a whole
/// [`WORLD_WAIT`] at 20 players' 400 messages a second, and a world's parts):
/// every poll of the match thread walks it.
const MOST_QUEUED: usize = 8_192;
/// A spectator's hash reports kept for judging, at most.
const REPORTS_KEPT: usize = 64;
/// Its reports in a row the players' majority contradicts before it is
/// started anew from a world: one alone may be a tick it played before its
/// world was in.
const DRIFT_REPORTS: u32 = 2;

/// A spectator is this far behind the match at least, whatever delay it was
/// given: a relay it can be fed comes this far before the time the match's
/// list holds it for (the `HORIZON_MS` of lead), and an input the match
/// takes late comes up to `HORIZON_MS` after: nearer than this, what is
/// on its way would be behind what it has been fed.
const LEAST_BEHIND: Duration = Duration::from_secs(1);

/// Where a spectator's game is. [`Spectator::go`] alone moves it, by
/// [`Watch::step`].
#[derive(Debug, PartialEq, Eq)]
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
    /// Its world (`serial`, the snapshot at `time`) is on its way; `kept`
    /// the relays and entries sent to wait with it.
    Syncing {
        serial: u64,
        time: u32,
        since: Instant,
        kept: Vec<u64>,
    },
    /// It has the world (the snapshot at `from`): gets every relay and roster
    /// change. `wrong`: its reports in a row that the players' majority
    /// contradicts ([`Game::judge_spectators`]); `reports`: its hash reports
    /// not judged yet, tick time and CRC, [`REPORTS_KEPT`] at most (they end
    /// with the stage).
    Live {
        from: u32,
        wrong: u32,
        reports: VecDeque<(u32, u32)>,
    },
    /// Let go by the server: its farewell sent, silent until `until`, then
    /// forgotten.
    Leaving { quiet_from: Instant, until: Instant },
}

/// What moves a spectator's game on.
#[derive(Debug)]
enum Step {
    /// Its port probe: it has been sent its 0xE4.
    Probed,
    /// It loaded (0xDA).
    Loaded,
    /// A world is there: its 0xE5 went, of this echo.
    TimeAsked { time: u32, echo: u16, now: Instant },
    /// It answered: its world went (`serial`, the snapshot at `time`).
    WorldSent {
        serial: u64,
        time: u32,
        now: Instant,
        kept: Vec<u64>,
    },
    /// Its world is in.
    WorldIn,
    /// Back to wait for a world (a new generation).
    WaitAgain { now: Instant },
    /// The server lets it go: its farewell went.
    LetGo { quiet_from: Instant, until: Instant },
}

/// What a [`Step`] leaves for its owner to do.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Nothing,
    /// The relays kept for a world given up: they would hold its next one
    /// back, and queue for ever for a game that never acks.
    Forget { kept: Vec<u64>, now: Instant },
}

impl Watch {
    /// The one transition of a spectator's game; `step` not meant for this
    /// stage leaves it as it is.
    fn step(self, step: Step) -> (Self, Action) {
        let stage = match (self, step) {
            (Self::Connecting, Step::Probed) => Self::Loading,
            (Self::Loading, Step::Loaded) => Self::Waiting,
            (Self::Waiting, Step::TimeAsked { time, echo, now }) => Self::Timing {
                time,
                echo,
                last: now,
                since: now,
            },
            (
                Self::Timing { .. },
                Step::WorldSent {
                    serial,
                    time,
                    now,
                    kept,
                },
            ) => Self::Syncing {
                serial,
                time,
                since: now,
                kept,
            },
            (Self::Syncing { time, .. }, Step::WorldIn) => Self::Live {
                from: time,
                wrong: 0,
                reports: VecDeque::new(),
            },
            (Self::Syncing { kept, .. }, Step::WaitAgain { now }) => {
                return (Self::Waiting, Action::Forget { kept, now });
            }
            (Self::Timing { .. } | Self::Live { .. }, Step::WaitAgain { .. }) => Self::Waiting,
            (Self::Leaving { quiet_from, until }, Step::LetGo { .. }) => {
                Self::Leaving { quiet_from, until }
            }
            (_, Step::LetGo { quiet_from, until }) => Self::Leaving { quiet_from, until },
            (stage, _) => stage,
        };
        (stage, Action::Nothing)
    }
}

/// A spectator of a match, `ms` behind it: its relays and roster changes
/// come from the match's lists as its clock reaches them, not as they are
/// played; those up to `fed` have gone to it.
#[derive(Clone, Copy, Debug)]
struct Delay {
    ms: u32,
    fed: u32,
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
    /// A match's spectator's: behind the match (`None`: a replay's, which
    /// its playback drives).
    delay: Option<Delay>,
    transport: Transport,
    stage: Watch,
    generation: Generation,
    /// The places its game has: the static info's, then each it installed.
    installed_roster: usize,
    installing: Option<Installing>,
    /// When it was started anew for drifting, the latest
    /// [`RESYNCS_PER_MINUTE`].
    restarts: VecDeque<Instant>,
    /// The place its watcher asked to see, still to be told: in a view note
    /// (`replay_control::ServerNote::View`) once it first watches.
    view: Option<u8>,
    /// The mask of kits it has been told of each place (a live match's chooser).
    kits_told: Vec<u8>,
}

/// What a spectator's machine is shown of what it watches, as of `clock`: a
/// match played here ([`Game::watched`]), or a recording played back
/// (`super::playback`, dev only). The machine reads nothing else.
pub(super) struct Watched<'a> {
    /// For the log.
    pub(super) id: u32,
    /// The match clock: the time its game is put at.
    pub(super) clock: u32,
    pub(super) info: &'a StaticInfo,
    /// The places its static info names.
    pub(super) places: usize,
    /// The places its game must have before it starts from a world.
    pub(super) roster: usize,
    /// The worlds a start may be from.
    pub(super) worlds: Vec<&'a Snapshot>,
    /// Everything relayed so far: its time, whose it is, what.
    pub(super) inputs: &'a [(u32, usize, Relayed)],
    /// The roster changes so far: time, place, entered (else left).
    pub(super) events: &'a [(u32, usize, bool)],
    /// Of those, what is due: up to this time, a delayed spectator's clock
    /// and the horizon (the rest come later); `None`: all of it (a replay's,
    /// whose lists are cut at what is due).
    pub(super) until: Option<u32>,
    pub(super) compress: bool,
    pub(super) entry_state: Duration,
}

impl Watched<'_> {
    /// Whether what is listed at `at` is due.
    fn due(&self, at: u32) -> bool {
        self.until.is_none_or(|until| at <= until)
    }
}

/// What a spectator's game said that its owner has to act on.
pub(super) enum Heard {
    Nothing,
    /// A place's install ack.
    Installed(Prepared),
    /// Its game left by its own word (0xE0).
    Left,
}

impl Game {
    /// The game at `addr` of `session_id` connects as a spectator (its
    /// session watches this match: the lobby's Queue and spectate). A match
    /// that is over, or not of growing rosters, takes none: its transport
    /// back, for the server to turn it away.
    pub(crate) fn watch(
        &mut self,
        addr: SocketAddr,
        session_id: u32,
        transport: Transport,
        delay: Duration,
        view: Option<u8>,
        now: Instant,
    ) -> Result<(), Box<Transport>> {
        // Over with no end of its own too: a spectator taken now would hold
        // its close back (`is_over` waits for them), and one that keeps
        // coming back would hold it for ever.
        if self.finished_at.is_some() || self.over_unfinished(now) || !self.growing_roster() {
            return Err(Box::new(transport));
        }
        // One game a session, one session an address.
        self.spectators
            .retain(|spectator| spectator.addr != addr && spectator.session_id != session_id);
        log::info!(
            "Match {:#x}: a spectator (session {session_id:#x}) connected from {addr}, {} s behind",
            self.id,
            delay.as_secs()
        );
        let mut spectator = Spectator::connect(
            addr,
            session_id,
            transport,
            self.snapshot_players() as u8,
            self.port,
            view,
        );
        spectator.delay = Some(Delay {
            ms: delay.max(LEAST_BEHIND).as_millis() as u32,
            fed: 0,
        });
        self.spectators.push(spectator);
        Ok(())
    }

    /// A trusted world, kept for a delayed spectator to start from: the
    /// last [`INPUTS_KEPT_MS`] (its inputs since are kept as long).
    pub(super) fn keep_past(&mut self, world: &Snapshot) {
        if self.past.back().is_some_and(|last| last.time >= world.time) {
            return;
        }
        self.kits_seen.record(world, &self.formed.profiles);
        self.past.push_back(world.clone());
        let keep_from = world.time.saturating_sub(INPUTS_KEPT_MS);
        while self.past.front().is_some_and(|old| old.time < keep_from) {
            self.past.pop_front();
        }
    }

    /// What a delayed spectator at `clock` (behind the match's) is shown:
    /// the worlds at or before it, the relays and roster changes due by it.
    fn watched_delayed(&self, clock: u32) -> Watched<'_> {
        Watched {
            worlds: self.past.iter().filter(|world| world.time <= clock).collect(),
            until: Some(clock + HORIZON_MS),
            ..self.watched(clock)
        }
    }

    /// The world a delayed spectator at `clock` starts from: the newest
    /// trusted one far enough behind it, and fresh enough that the inputs
    /// since are read right.
    fn delayed_start(&self, clock: u32) -> Option<Snapshot> {
        self.past
            .iter()
            .filter(|world| world.time + HORIZON_MS <= clock)
            .filter(|world| clock - world.time <= FRESH_SNAPSHOT_MS)
            .max_by_key(|world| world.time)
            .cloned()
    }

    /// The view of the spectator `at` (behind the match or not), at the
    /// match clock `clock`.
    fn view_of(&self, delay: Option<Delay>, clock: u32) -> Watched<'_> {
        match delay {
            Some(delay) => self.watched_delayed(clock.saturating_sub(delay.ms)),
            None => self.watched(clock),
        }
    }

    /// What its spectators are shown, at `clock`.
    fn watched(&self, clock: u32) -> Watched<'_> {
        Watched {
            id: self.id,
            clock,
            info: &self.formed.info,
            places: self.formed.places.len(),
            roster: self.spectator_roster(),
            worlds: [self.snapshot.as_ref(), self.recent.as_ref()]
                .into_iter()
                .flatten()
                .collect(),
            inputs: &self.inputs,
            events: &self.events,
            until: None,
            compress: self.settings.compress_state,
            entry_state: self.settings.timing.entry_state,
        }
    }

    /// The spectator at `addr`, if any: one that watches takes its
    /// datagrams; one let go swallows them (silence) unless its game
    /// connects again from there as a player.
    pub(crate) fn spectator_at(&self, addr: SocketAddr) -> Option<SpectatorAt> {
        self.spectators
            .iter()
            .find(|spectator| spectator.addr == addr)
            .map(Spectator::at)
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
        let clock = self.clock(now);
        let mut spectators = std::mem::take(&mut self.spectators);
        let view = self.view_of(spectators[at].delay, clock);
        let heard = spectators[at].hear(&view, message_id, payload, now);
        self.spectators = spectators;
        match heard {
            Heard::Nothing => {}
            Heard::Installed(ack) => self.spectator_installed(at, ack, now),
            Heard::Left => {
                log::info!(
                    "Match {:#x}: spectator {:#x} left by its own word (0xE0)",
                    self.id,
                    self.spectators[at].session_id
                );
                self.spectator_gone(at);
            }
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

    /// The places a spectator's game must have: all but those from the
    /// first no game installs any more on (`Admission::install_stopped`:
    /// none after it can be installed, `late_join`), which nobody asks it
    /// to install either (`prepare_for_spectator`).
    fn spectator_roster(&self) -> usize {
        self.admissions
            .iter()
            .position(|admission| admission.as_ref().is_some_and(Admission::install_stopped))
            .unwrap_or(self.players())
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
        // Over with no end of its own (everyone left, or nobody came back):
        // the spectators are let go before the match is closed (`is_over`
        // waits for them).
        if self.finished_at.is_none() && self.over_unfinished(now) {
            self.spectators_finish(now);
        }
        self.judge_spectators(now);
        for at in (0..self.spectators.len()).rev() {
            if self.spectators[at].timed_out(&self.settings.timing, now) {
                self.spectator_gone(at);
            }
        }
        let roster = self.spectator_roster();
        let mut starts = Vec::with_capacity(self.spectators.len());
        for at in 0..self.spectators.len() {
            self.prepare_for_spectator(at, now);
            // From the worlds the match keeps anyway: a spectator asks the
            // players for nothing.
            let start = self.spectators[at]
                .delay
                .filter(|_| self.spectators[at].waits_for_world(roster))
                .and_then(|delay| self.delayed_start(clock.saturating_sub(delay.ms)))
                .map(|world| world.time);
            starts.push(start);
        }
        let echo = self.mono(now) as u16;
        let mut spectators = std::mem::take(&mut self.spectators);
        for (spectator, start) in spectators.iter_mut().zip(starts) {
            let view = self.view_of(spectator.delay, clock);
            spectator.feed(&view, now);
            spectator.step(&view, start, echo, now);
            spectator.tell_kits(&self.kits_seen, &self.formed.profiles, view.clock);
        }
        self.spectators = spectators;
        let mut datagrams = Vec::new();
        for spectator in &mut self.spectators {
            datagrams.extend(spectator.poll(now));
        }
        self.spectators
            .retain(|spectator| !spectator.is_forgotten(now));
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
        let id = self.id;
        // Taken out for the time of the judging: the majority reads the match.
        let mut spectators = std::mem::take(&mut self.spectators);
        for spectator in &mut spectators {
            let Watch::Live { wrong, reports, .. } = &mut spectator.stage else {
                continue;
            };
            let judged = std::mem::take(reports);
            for (time, crc) in judged {
                if time < keep_from {
                    continue;
                }
                if time >= checked {
                    reports.push_back((time, crc));
                    continue;
                }
                match self.majority(time) {
                    Some(right) if right == crc => *wrong = 0,
                    Some(_) => *wrong += 1,
                    None => {}
                }
            }
            let wrong = *wrong;
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
            spectator.wait_again(now);
        }
        self.spectators = spectators;
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
    pub(crate) fn spectator_backlog(&self, session_id: u32) -> Option<usize> {
        self.spectators
            .iter()
            .find(|spectator| spectator.session_id == session_id)
            .map(|spectator| spectator.transport.backlog())
    }

    /// Fills the queue of the spectator of `session_id` to past [`MOST_QUEUED`].
    #[cfg(test)]
    pub(crate) fn flood_spectator(&mut self, session_id: u32) {
        for spectator in &mut self.spectators {
            if spectator.session_id == session_id {
                let message = ServerMessage::Ping { a: 0, b: 0 };
                for _ in 0..=MOST_QUEUED {
                    let _ = spectator.transport.send_kept(message.id(), &message.payload());
                }
            }
        }
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
    /// A game that connects to watch: told the ports (0xE3); `view` the
    /// place it is to follow first, when one was asked.
    pub(super) fn connect(
        addr: SocketAddr,
        session_id: u32,
        mut transport: Transport,
        roster_size: u8,
        port: u16,
        view: Option<u8>,
    ) -> Self {
        transport.set_roster_size(roster_size);
        let ports = ServerMessage::Ports {
            min: port,
            max: port,
        };
        let _ = transport.send(ports.id(), &ports.payload());
        Self {
            addr,
            session_id,
            delay: None,
            transport,
            stage: Watch::Connecting,
            generation: Generation::AT_READY,
            installed_roster: 0,
            installing: None,
            restarts: VecDeque::new(),
            view,
            kits_told: Vec::new(),
        }
    }

    pub(super) fn session_id(&self) -> u32 {
        self.session_id
    }

    pub(super) fn at(&self) -> SpectatorAt {
        match self.stage {
            Watch::Leaving { .. } => SpectatorAt::LetGo(self.session_id),
            _ => SpectatorAt::Watching,
        }
    }

    /// Its game has loaded its level (0xDA): it can be started from a world.
    /// Its stage, for the log.
    pub(super) fn stage(&self) -> String {
        match self.stage {
            Watch::Connecting => "Connecting".to_owned(),
            Watch::Loading => "Loading".to_owned(),
            Watch::Waiting => "Waiting".to_owned(),
            Watch::Timing { time, echo, .. } => format!("Timing(world {time} ms, echo {echo})"),
            Watch::Syncing { serial, time, .. } => {
                format!("Syncing(world {time} ms, serial {serial})")
            }
            Watch::Live { from, wrong, .. } => format!("Live(from {from} ms, {wrong} wrong)"),
            Watch::Leaving { .. } => "Leaving".to_owned(),
        }
    }

    pub(super) fn has_loaded(&self) -> bool {
        !matches!(self.stage, Watch::Connecting | Watch::Loading)
    }

    pub(super) fn receive(&mut self, datagram: &[u8], now: Instant) -> Vec<Event> {
        self.transport.receive(datagram, now)
    }

    /// One message of its game.
    pub(super) fn hear(
        &mut self,
        watched: &Watched<'_>,
        message_id: u8,
        payload: &[u8],
        now: Instant,
    ) -> Heard {
        if self.is_leaving() {
            return Heard::Nothing;
        }
        // 0xE0 carries a place's install ack, an upload's chunk, or (empty)
        // the game's own leave.
        if message_id == id::LEAVE
            && let Ok(ack) = Prepared::decode(payload)
        {
            return Heard::Installed(ack);
        }
        let Ok(message) = parse_client(message_id, payload) else {
            return Heard::Nothing;
        };
        match message {
            ClientMessage::PortProbe { .. } => {
                if self.stage == Watch::Connecting {
                    log::info!(
                        "Match {:#x}: spectator {:#x} gets the 0xE4 of {} places",
                        watched.id,
                        self.session_id,
                        watched.places
                    );
                    let info = watched.info.spectator_message();
                    let _ = self.transport.send(info.id(), &info.payload());
                    self.installed_roster = watched.places;
                    self.go(Step::Probed);
                }
            }
            ClientMessage::Ready => {
                if self.stage == Watch::Loading {
                    // The client set its own generation to 0xFF with it.
                    self.generation = Generation::AT_READY;
                    self.go(Step::Loaded);
                    log::info!(
                        "Match {:#x}: spectator {:#x} has loaded",
                        watched.id,
                        self.session_id
                    );
                }
            }
            ClientMessage::TimeReply(reply) => {
                if let Watch::Timing { time, echo, .. } = self.stage
                    && reply.stamp == echo
                {
                    self.start_from_world(watched, time, echo, now);
                }
            }
            ClientMessage::Leave => return Heard::Left,
            // Heard only to find it out of step (`judge_spectators`): never
            // one of the match's reports.
            ClientMessage::Hash { tick, crc } => {
                let clock = watched.clock;
                if let Watch::Live { from, reports, .. } = &mut self.stage
                    && crc != FORCED_MISMATCH
                    && let Some(time) = tick_time(tick, clock)
                    && time > *from
                    && time <= clock + HASH_MAX_AHEAD_MS
                {
                    if reports.len() == REPORTS_KEPT {
                        reports.pop_front();
                    }
                    reports.push_back((time, crc));
                }
            }
            // Not heard from a spectator: it is no player and no witness.
            ClientMessage::Connect { .. }
            | ClientMessage::Input(_)
            | ClientMessage::ProfileInput { .. }
            | ClientMessage::Chunk(_)
            | ClientMessage::KeepAlive => {}
        }
        Heard::Nothing
    }

    /// It answered its 0xE5: the time again, the inputs and the entries
    /// since the world, and the world.
    fn start_from_world(&mut self, watched: &Watched<'_>, time: u32, echo: u16, now: Instant) {
        // A place appended while it was timed, not installed yet: the
        // entries and inputs since the world could be of it.
        if self.installed_roster < watched.roster {
            self.wait_again(now);
            return;
        }
        let clock = watched.clock;
        let Some(world) = watched.worlds.iter().find(|world| world.time == time) else {
            self.go(Step::WaitAgain { now });
            return;
        };
        let replay: Vec<ServerMessage> = watched
            .inputs
            .iter()
            .filter(|(at, _, _)| *at > time && watched.due(*at))
            .map(|(_, from, what)| relay(*from, what))
            .collect();
        if let (Some(delay), Some(until)) = (self.delay.as_mut(), watched.until) {
            delay.fed = until;
        }
        let entries: Vec<ServerMessage> = watched
            .events
            .iter()
            .filter(|(at, _, entered)| *entered && *at > time && watched.due(*at))
            .map(|&(at, who, _)| ServerMessage::PlayerEntered {
                tick: tick_byte(at),
                player: who as u8,
            })
            .collect();
        let lifetime = entry_lifetime(watched.entry_state, clock, clock);
        let id = watched.id;
        let generation = self.generation.0;
        self.send_time(generation, echo, clock);
        let kept: Vec<u64> = replay
            .iter()
            .chain(&entries)
            .filter_map(|message| self.send_waiting(message, now))
            .collect();
        let payload = if watched.compress {
            ServerMessage::packed_snapshot(generation, world.time, &world.body)
        } else {
            ServerMessage::snapshot(generation, world.time, &world.body)
        }
        .payload();
        let sent = match lifetime {
            Some(lifetime) => self
                .transport
                .send_lasting(id::SNAPSHOT, &payload, lifetime),
            None => self.transport.send(id::SNAPSHOT, &payload),
        };
        match sent {
            Ok(serial) => {
                log::info!(
                    "Match {id:#x}: spectator {:#x} starts from the snapshot at {} ms \
                     (generation {generation}) at {clock} ms, {} inputs since",
                    self.session_id,
                    world.time,
                    replay.len()
                );
                self.go(Step::WorldSent {
                    serial,
                    time: world.time,
                    now,
                    kept,
                });
            }
            Err(_) => {
                self.transport.forget_all(&kept, now);
                self.go(Step::WaitAgain { now });
            }
        }
    }

    /// A delayed spectator's turn: the relays and roster changes its clock
    /// reached since the last, from the match's lists.
    fn feed(&mut self, view: &Watched<'_>, now: Instant) {
        let (Some(delay), Some(until)) = (self.delay, view.until) else {
            return;
        };
        let from = delay.fed;
        if until <= from {
            return;
        }
        for &(time, place, entered) in view.events {
            if time > from && time <= until {
                let message = if entered {
                    ServerMessage::PlayerEntered {
                        tick: tick_byte(time),
                        player: place as u8,
                    }
                } else {
                    ServerMessage::PlayerLeft {
                        tick: tick_byte(time),
                        player: place as u8,
                    }
                };
                self.roster_change(place, entered, time, &message, now);
            }
        }
        for (time, place, what) in view.inputs {
            if *time > from && *time <= until {
                self.relay(*place, &relay(*place, what), *time, view.clock, now);
            }
        }
        if let Some(delay) = self.delay.as_mut() {
            delay.fed = until;
        }
    }

    /// What its game may show of the kits played by `clock`: each place whose mask grew, told
    /// again whole. Only once it watches.
    pub(super) fn tell_kits(&mut self, seen: &super::kit_view::KitsSeen, profiles: &[MatchProfile], clock: u32) {
        if !matches!(self.stage, Watch::Live { .. }) {
            return;
        }
        self.kits_told.resize(profiles.len(), 0);
        for place in 0..profiles.len() {
            let Some(note) = seen.note_by(place, clock, profiles) else {
                continue;
            };
            if note.mask != self.kits_told[place] {
                self.kits_told[place] = note.mask;
                self.note(&note.encode());
            }
        }
    }

    /// A relay of the place `from` for `time`, at the match clock `clock`.
    pub(super) fn relay(
        &mut self,
        from: usize,
        message: &ServerMessage,
        time: u32,
        clock: u32,
        now: Instant,
    ) {
        match self.stage {
            Watch::Live { .. } | Watch::Syncing { .. } if from >= self.installed_roster => {
                self.wait_again(now);
            }
            Watch::Live { .. } => {
                let until = relay_deadline(0, clock, time, now);
                let _ = self
                    .transport
                    .send_until(message.id(), &message.payload(), until);
            }
            Watch::Syncing {
                time: from_time, ..
            } if time > from_time => {
                self.keep_for_world(message, now);
            }
            _ => {}
        }
    }

    /// A roster change of the place `index` at `at`. An entry also to one
    /// whose world is on its way; a leave only to one that has its world
    /// (`Game::roster_change`).
    pub(super) fn roster_change(
        &mut self,
        index: usize,
        entered: bool,
        at: u32,
        message: &ServerMessage,
        now: Instant,
    ) {
        match self.stage {
            Watch::Live { .. } | Watch::Syncing { .. } if index >= self.installed_roster => {
                self.wait_again(now);
            }
            Watch::Live { .. } => {
                let _ = self.transport.send(message.id(), &message.payload());
            }
            Watch::Syncing { time, .. } if entered && at > time => {
                self.keep_for_world(message, now);
            }
            _ => {}
        }
    }

    /// Its link silent past its timeout, or its queue past what a game
    /// that acks leaves there (not one being let go).
    pub(super) fn timed_out(&self, timing: &super::super::Timing, now: Instant) -> bool {
        let timeout = match self.stage {
            Watch::Connecting | Watch::Loading => timing.loading_timeout,
            Watch::Leaving { .. } => return false,
            _ => timing.client_timeout,
        };
        let queued = self.transport.backlog();
        if queued > MOST_QUEUED {
            log::warn!(
                "Spectator {:#x} at {} has {queued} messages queued and acks none of them; it is let go",
                self.session_id,
                self.addr
            );
            return true;
        }
        self.transport.is_gone_after(now, timeout)
    }

    /// It waits for a world and has the `roster` places its start needs.
    fn waits_for_world(&self, roster: usize) -> bool {
        self.stage == Watch::Waiting && self.installed_roster >= roster
    }

    /// Its turn of a poll: one that waits starts from the world at `start`
    /// (0xE5 of its next generation); one timed gets its 0xE5 again; one
    /// whose world is in watches (the leaves since its world). Each waits
    /// for a world again past its wait.
    pub(super) fn step(
        &mut self,
        watched: &Watched<'_>,
        start: Option<u32>,
        echo: u16,
        now: Instant,
    ) {
        let clock = watched.clock;
        match self.stage {
            Watch::Waiting => {
                if let Some(time) = start.filter(|_| self.installed_roster >= watched.roster) {
                    self.generation = self.generation.next();
                    self.send_time(self.generation.0, echo, clock);
                    self.go(Step::TimeAsked { time, echo, now });
                }
            }
            Watch::Timing {
                echo, last, since, ..
            } => {
                if now.saturating_duration_since(since) >= TIMING_WAIT {
                    self.wait_again(now);
                } else if now.saturating_duration_since(last) >= SET_TIME_EVERY {
                    self.send_time(self.generation.0, echo, clock);
                    if let Watch::Timing { last, .. } = &mut self.stage {
                        *last = now;
                    }
                }
            }
            Watch::Syncing {
                serial,
                time,
                since,
                ..
            } => {
                if self.transport.is_delivered(serial) {
                    for &(at, who, _) in watched
                        .events
                        .iter()
                        .filter(|(at, _, entered)| !*entered && *at > time && watched.due(*at))
                    {
                        let leave = ServerMessage::PlayerLeft {
                            tick: tick_byte(at),
                            player: who as u8,
                        };
                        let _ = self.transport.send(leave.id(), &leave.payload());
                    }
                    self.go(Step::WorldIn);
                    log::info!(
                        "Match {:#x}: spectator {:#x} watches",
                        watched.id,
                        self.session_id
                    );
                    // Its camera is on someone now (patch 121, the world's
                    // load): the asked place instead, once.
                    if let Some(place) = self.view.take() {
                        self.note(&replay_control::ServerNote::view(place));
                    }
                } else if now.saturating_duration_since(since) >= WORLD_WAIT {
                    self.wait_again(now);
                }
            }
            Watch::Connecting | Watch::Loading | Watch::Live { .. } | Watch::Leaving { .. } => {}
        }
    }

    /// Its datagrams: none once its farewell has gone (silence).
    pub(super) fn poll(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        if matches!(self.stage, Watch::Leaving { quiet_from, .. } if now >= quiet_from) {
            return Vec::new();
        }
        let addr = self.addr;
        self.transport
            .poll(now)
            .into_iter()
            .map(|datagram| (addr, datagram))
            .collect()
    }

    /// Let go, and its silence is over: forgotten.
    pub(super) fn is_forgotten(&self, now: Instant) -> bool {
        matches!(self.stage, Watch::Leaving { until, .. } if now >= until)
    }

    /// An 0xE6 (its game answers 0xDB, which nothing here waits for while
    /// it is live): traffic, so that its game's silence check never ends
    /// the link while nothing else is sent (a paused replay).
    pub(super) fn ping(&mut self) {
        if matches!(self.stage, Watch::Live { .. }) {
            let ping = ServerMessage::Ping { a: 0, b: 0 };
            let _ = self.transport.send(ping.id(), &ping.payload());
        }
    }

    /// It plays from a world it took.
    pub(super) fn is_live(&self) -> bool {
        matches!(self.stage, Watch::Live { .. })
    }

    /// A replay viewer's note (`replay_control::ServerNote`), in an 0xF6:
    /// only while live, never to a game let go.
    pub(super) fn note(&mut self, payload: &[u8]) {
        if self.is_live() {
            let _ = self.transport.send(id::UPLOAD_REQUEST, payload);
        }
    }

    pub(super) fn is_leaving(&self) -> bool {
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

    /// Kept until acked, at the pace of a game without a world, for as long
    /// as a world is waited for: its serial, sent.
    fn send_waiting(&mut self, message: &ServerMessage, now: Instant) -> Option<u64> {
        self.transport
            .send_kept_until(
                message.id(),
                &message.payload(),
                WAITING_RELAY_RESEND,
                now + WORLD_WAIT,
            )
            .ok()
    }

    /// [`Self::send_waiting`] of what comes while its world is on its way.
    fn keep_for_world(&mut self, message: &ServerMessage, now: Instant) {
        let serial = self.send_waiting(message, now);
        if let (Some(serial), Watch::Syncing { kept, .. }) = (serial, &mut self.stage) {
            kept.push(serial);
        }
    }

    /// One step of its game's machine: the only place its stage is set.
    fn go(&mut self, step: Step) {
        let stage = std::mem::replace(&mut self.stage, Watch::Waiting);
        let (stage, action) = stage.step(step);
        self.stage = stage;
        if let Action::Forget { kept, now } = action {
            self.transport.forget_all(&kept, now);
        }
    }

    /// Back to wait for a world: its next start is of a new generation.
    pub(super) fn wait_again(&mut self, now: Instant) {
        self.go(Step::WaitAgain { now });
    }

    /// The server lets it go: an 0xEA naming no place, its game's end, sent
    /// for [`FAREWELL_GOES`]; then silence (see the module).
    pub(super) fn farewell(&mut self, clock: u32, now: Instant) {
        let farewell = ServerMessage::PlayerLeft {
            tick: tick_byte(clock),
            player: NO_LOCAL_PLACE,
        };
        let _ = self.transport.send(farewell.id(), &farewell.payload());
        self.go(Step::LetGo {
            quiet_from: now + FAREWELL_GOES,
            until: now + SILENCE,
        });
    }
}
