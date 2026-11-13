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
//! (`replay_control::LeftPlace::Farewell`), then silence for [`SILENCE`]. A link closed by the
//! server ends nothing in the game (0xFD reaches `close_current_match` as
//! reason 4, its return, `match.md` section 24), and an 0xEA ends a game's
//! match only for its own place (0x27C72C: with no local place the handler
//! returns), so the spectator patch (143, `player_left.local`) takes an 0xEA
//! naming 0xFF as its game's end: `close_current_match(0)`, the world
//! ended, no results screen. The silence after it is for a game whose 0xEA
//! was lost: its match link ends after 5 s of it, the same way (`match.md`
//! section 34). A game without the patch takes the 0xEA harmlessly
//! (`leave_match` 0x2D18A0 tests bit `0xFF & 31` of the entered mask, never
//! set, and drops it, 0x2D1B8D..0x2D1B9D) and ends by the silence. A game of its session seated meanwhile (its place
//! came) connects again from that address as a player, with an 0xD8: until
//! then what it sends is swallowed too (`Server::receive`), its farewell
//! still resent.
//!
//! It starts again from a world (back to wait for one) when its reports say
//! it is out of step, as rarely as a player is resynced; when a relay or
//! roster change of a place it has not installed comes; when a world is not
//! loaded in [`WORLD_WAIT`] or an 0xE5 not answered in [`TIMING_WAIT`]
//! (a replay's also when its viewer seeks). What went for its last start
//! goes no more once the new 0xE5 does ([`Spectator::forget_timeline`]).

use survarium::v026g::match_msgs::static_info::{MatchProfile, StaticInfo};

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
/// A game with more than this many messages of its own not acked (queued or
/// sent) is let go: a world's parts and a ring of every place's inputs come
/// to some hundreds, a game that acks keeps a round trip of them, and twenty
/// places' inputs come to this in the 5 s of silence that end a game's own
/// link. One that stops acking (and asks for more: a replay viewer's
/// controls, a place's install) would grow its queue for ever, and every
/// poll of the match thread, which follows every datagram the server takes,
/// walks it.
pub(super) const BACKLOG_MOST: usize = 2048;

/// A spectator is this far behind the match at least, whatever delay it was
/// given: a relay it can be fed comes this far before the time the match's
/// list holds it for (the `HORIZON_MS` of lead), and an input the match
/// takes late comes up to `HORIZON_MS` after: nearer than this, what is
/// on its way would be behind what it has been fed.
const LEAST_BEHIND: Duration = Duration::from_secs(1);

/// A session's game starts from a world (an 0xE5 of a new generation, then
/// the world packed on the match thread) no sooner than this after its last,
/// whatever starts it again (a seek, a drift, a world not in, its ring, its
/// game connecting anew): what is asked meanwhile waits.
pub(super) const START_EVERY: Duration = Duration::from_secs(1);

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
    /// Its world (`serial`, the snapshot at `time`) is on its way; `fill`:
    /// each place's inputs after the world sent to it (its game keeps a ring
    /// of them for the world, [`Spectator::ring_overrun`]).
    Syncing {
        serial: u64,
        time: u32,
        since: Instant,
        fill: Vec<usize>,
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
    /// It answered: its world went (`serial`, the snapshot at `time`), with
    /// `fill` inputs of each place after it.
    WorldSent {
        serial: u64,
        time: u32,
        now: Instant,
        fill: Vec<usize>,
    },
    /// Its world is in.
    WorldIn,
    /// Back to wait for a world (a new generation).
    WaitAgain,
    /// The server lets it go: its farewell went.
    LetGo { quiet_from: Instant, until: Instant },
}

impl Watch {
    /// The one transition of a spectator's game; `step` not meant for this
    /// stage leaves it as it is.
    fn step(self, step: Step) -> Self {
        match (self, step) {
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
                    fill,
                },
            ) => Self::Syncing {
                serial,
                time,
                since: now,
                fill,
            },
            (Self::Syncing { time, .. }, Step::WorldIn) => Self::Live {
                from: time,
                wrong: 0,
                reports: VecDeque::new(),
            },
            (
                Self::Timing { .. } | Self::Syncing { .. } | Self::Live { .. },
                Step::WaitAgain,
            ) => Self::Waiting,
            (stage @ Self::Leaving { .. }, Step::LetGo { .. }) => stage,
            (_, Step::LetGo { quiet_from, until }) => Self::Leaving { quiet_from, until },
            (stage, _) => stage,
        }
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
    /// Let go, silent: its datagrams are swallowed, unless its game connects
    /// again from there (an 0xD8: as the player its session was seated as
    /// meanwhile, or anew).
    LetGo,
}

/// A place's install asked of the spectator's game.
struct Installing {
    proposal: Proposal,
    sent: Instant,
    /// Its 0xE4: one asked again replaces it (one not acked is kept for
    /// ever, and a game that never acks would pile them up).
    serial: Option<u64>,
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
    /// Its session's starts from a world, of every connection of its game.
    starts: Starts,
    /// The place its watcher asked to see, still to be told: in a view note
    /// (`replay_control::ServerNote::View`) once it first watches.
    view: Option<u8>,
    /// The mask of kits it has been told of each place (a live match's chooser).
    kits_told: Vec<u8>,
    /// The relays and roster changes sent for its current start (since its
    /// last 0xE5 of a new generation) that may still go again: its next
    /// start forgets them ([`Self::forget_timeline`]).
    timeline: Vec<u64>,
    /// Its room called it back ([`Self::recall`]).
    recall: Option<Recall>,
    /// How often [`Self::feed`] walked the match's lists.
    #[cfg(test)]
    list_walks: u32,
}

/// Its room called a spectator back ([`Spectator::recall`]): let go at
/// `due`, [`replay_control::RECALL_SECONDS`] after, and its game told so in
/// a note (`ServerNote::Recall`), which goes only while it is live.
#[derive(Clone, Copy, Debug)]
enum Recall {
    /// Its game was not live yet (its world on its way): the note is still
    /// to go, with the seconds left then.
    Untold {
        due: Instant,
    },
    Told {
        due: Instant,
    },
}

/// What a session's game has had of starts from a world. Each packs a world
/// on the match thread, so it outlives the game's connection: a game that
/// connects again (from another port: `Game::watch`, `ReplayWatch::connect`)
/// is its session's spectator anew, with what its last one had of them.
#[derive(Debug, Default)]
pub(super) struct Starts {
    /// When it last started from a world (an 0xE5 of a new generation
    /// went): the next no sooner than [`START_EVERY`] after.
    last: Option<Instant>,
    /// When it was started anew for drifting, the latest
    /// [`RESYNCS_PER_MINUTE`] ([`Spectator::take_drift_restart`]).
    drift: VecDeque<Instant>,
    /// Its starts done again in a row since it last watched, its world's
    /// inputs past its game's ring ([`Spectator::ring_overrun`]).
    ring_redos: u32,
}

/// A spectator's list of what went for its current start
/// ([`Spectator::track`]) is cleared of what is in once it is longer than
/// twice what its game has not acked and this.
const TIMELINE_SLACK: usize = 64;

/// A message sent may still go again: neither acked nor given up.
fn in_flight(transport: &Transport, serial: u64) -> bool {
    !transport.is_delivered(serial) && !transport.is_lost(serial)
}

/// One more input of `place` in a world's ring count: its count now.
fn count_input(fill: &mut Vec<usize>, place: usize) -> usize {
    if fill.len() <= place {
        fill.resize(place + 1, 0);
    }
    fill[place] += 1;
    fill[place]
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
    /// Their 0xE8s past the generation byte, by index, each made once for
    /// every game shown it (a recording's key frames: its sessions share
    /// them, `playback::Recording`; a delayed spectator's past worlds);
    /// none: made for each start.
    pub(super) sent_worlds: Vec<&'a OnceLock<Vec<u8>>>,
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
        // One game a session, one session an address. Its session's game
        // again (from another port): anew, with what it had of starts.
        let starts = self
            .spectators
            .iter()
            .position(|spectator| spectator.session_id == session_id)
            .map(|at| self.spectators.remove(at).into_starts())
            .unwrap_or_default();
        self.spectators.retain(|spectator| spectator.addr != addr);
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
            starts,
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
        if self.past.back().is_some_and(|(last, _)| last.time >= world.time) {
            return;
        }
        self.past.push_back((world.clone(), OnceLock::new()));
        let keep_from = world.time.saturating_sub(INPUTS_KEPT_MS);
        while self.past.front().is_some_and(|(old, _)| old.time < keep_from) {
            self.past.pop_front();
        }
    }

    /// What a delayed spectator at `clock` (behind the match's) is shown:
    /// the worlds at or before it, the relays and roster changes due by it.
    fn watched_delayed(&self, clock: u32) -> Watched<'_> {
        Watched {
            // Oldest first: the same index in both.
            worlds: self
                .past
                .iter()
                .map(|(world, _)| world)
                .filter(|world| world.time <= clock)
                .collect(),
            sent_worlds: self
                .past
                .iter()
                .filter(|(world, _)| world.time <= clock)
                .map(|(_, sent)| sent)
                .collect(),
            until: Some(clock + HORIZON_MS),
            ..self.watched(clock)
        }
    }

    /// The world a delayed spectator at `clock` starts from: the newest
    /// trusted one far enough behind it, and fresh enough that the inputs
    /// since are read right and its game holds them all: no player has more
    /// than [`INPUT_RING_SAFE`] up to what goes with it, its clock and the
    /// horizon (as `Recording::reach` puts a replay's; those that come while
    /// it is on its way, [`Spectator::ring_overrun`]).
    fn delayed_start(&self, clock: u32) -> Option<Snapshot> {
        self.past
            .iter()
            .map(|(world, _)| world)
            .filter(|world| world.time + HORIZON_MS <= clock)
            .filter(|world| clock - world.time <= FRESH_SNAPSHOT_MS)
            .max_by_key(|world| world.time)
            .filter(|world| self.ring_fill(world.time, clock + HORIZON_MS) <= INPUT_RING_SAFE)
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
            sent_worlds: Vec::new(),
            inputs: &self.inputs,
            events: &self.events,
            until: None,
            compress: self.settings.compress_state,
            entry_state: self.settings.timing.entry_state,
        }
    }

    /// The spectator at `addr`, if any: one that watches takes its
    /// datagrams; one let go swallows them (silence) unless its game
    /// connects again from there (an 0xD8).
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
    /// The spectator's room calls it back: its game is told once, when it is
    /// live (`Spectator::recall`); whether the seconds since the first call
    /// are over (a spectator not here any more: at once).
    pub(crate) fn recall_spectator(&mut self, session_id: u32, now: Instant) -> bool {
        let Some(spectator) = self
            .spectators
            .iter_mut()
            .find(|spectator| spectator.session_id == session_id)
        else {
            return true;
        };
        if !spectator.is_recalled() {
            log::info!(
                "Match {:#x}: spectator {session_id:#x} is called back by its room",
                self.id
            );
        }
        spectator.recall(now)
    }

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
        let id = self.id;
        for spectator in &mut self.spectators {
            if spectator.flooded(BACKLOG_MOST) {
                log::warn!(
                    "Match {id:#x}: spectator {:#x} has {} messages not acked; it is let go",
                    spectator.session_id,
                    spectator.transport.backlog()
                );
                // Out of its queue as one gone, and let go.
                self.spectators_left.push(spectator.session_id);
                spectator.farewell(clock, now);
            }
        }
        let roster = self.spectator_roster();
        for at in 0..self.spectators.len() {
            self.prepare_for_spectator(at, now);
        }
        let echo = self.mono(now) as u16;
        let mut spectators = std::mem::take(&mut self.spectators);
        for spectator in &mut spectators {
            let view = self.view_of(spectator.delay, clock);
            let ticked = spectator.feed(&view, now);
            // From the worlds the match keeps anyway: a spectator asks the
            // players for nothing. Looked for once a tick, as the lists are
            // walked (`delayed_start` counts the inputs since a world).
            let start = spectator
                .delay
                .filter(|_| ticked && spectator.waits_for_world(roster))
                .and_then(|delay| self.delayed_start(clock.saturating_sub(delay.ms)))
                .map(|world| world.time);
            spectator.step(&view, start, echo, now);
            spectator.tell_kits(&self.formed.profiles);
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
    /// (kept for as far back as the longest delay, `Game::settled_hashes`)
    /// is right or wrong; one with no majority, or older than those kept,
    /// decides nothing, and so does one of a tick up to the end of the
    /// players' last install (`hashes_unjudged_until`): its game installs a
    /// place when they do, a delay before it plays that time, and their
    /// majorities before were of worlds without it. [`DRIFT_REPORTS`] wrong
    /// in a row: it is
    /// started anew from a world (its next generation), as a player out of
    /// step is resynced, and no more often: [`RESYNCS_PER_MINUTE`] in
    /// [`RESYNC_WINDOW`], `Timing::resync_interval` apart. Its reports never
    /// go into `Game::hashes`: nothing of the match reads them.
    fn judge_spectators(&mut self, now: Instant) {
        let checked = self.hashes_checked_until;
        let unjudged_until = self.hashes_unjudged_until;
        let interval = self.settings.timing.resync_interval;
        let id = self.id;
        let settled = &self.settled_hashes;
        let majority = |time: u32| {
            settled
                .binary_search_by_key(&time, |&(at, _)| at)
                .ok()
                .map(|at| settled[at].1)
        };
        for spectator in &mut self.spectators {
            let Watch::Live { wrong, reports, .. } = &mut spectator.stage else {
                continue;
            };
            let judged = std::mem::take(reports);
            for (time, crc) in judged {
                if time >= checked {
                    reports.push_back((time, crc));
                    continue;
                }
                if time <= unjudged_until {
                    continue;
                }
                match majority(time) {
                    Some(right) if right == crc => *wrong = 0,
                    Some(_) => *wrong += 1,
                    None => {}
                }
            }
            let wrong = *wrong;
            if wrong < DRIFT_REPORTS || !spectator.take_drift_restart(interval, now) {
                continue;
            }
            log::info!(
                "Match {id:#x}: spectator {:#x} is out of step ({wrong} reports in a row); it \
                 starts again from a world",
                spectator.session_id
            );
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
        if let Some(serial) = spectator
            .installing
            .as_ref()
            .and_then(|installing| installing.serial)
        {
            spectator.transport.forget(serial, now);
        }
        let serial = spectator.transport.send(id::STATIC_INFO, &bytes).ok();
        spectator.installing = Some(Installing {
            proposal,
            sent: now,
            serial,
        });
    }

    #[cfg(test)]
    pub(crate) fn spectator_backlog(&self, session_id: u32) -> Option<usize> {
        self.spectators
            .iter()
            .find(|spectator| spectator.session_id == session_id)
            .map(|spectator| spectator.transport.backlog())
    }

    #[cfg(test)]
    pub(crate) fn spectator_list_walks(&self, session_id: u32) -> Option<u32> {
        self.spectators
            .iter()
            .find(|spectator| spectator.session_id == session_id)
            .map(|spectator| spectator.list_walks)
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
    /// place it is to follow first, when one was asked; `starts` what its
    /// session had of starts (a game that connects again).
    pub(super) fn connect(
        addr: SocketAddr,
        session_id: u32,
        mut transport: Transport,
        roster_size: u8,
        port: u16,
        view: Option<u8>,
        starts: Starts,
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
            starts,
            view,
            kits_told: Vec::new(),
            timeline: Vec::new(),
            recall: None,
            #[cfg(test)]
            list_walks: 0,
        }
    }

    pub(super) fn session_id(&self) -> u32 {
        self.session_id
    }

    /// Its game connects again: what its session had of starts, for the new
    /// one ([`Self::connect`]).
    pub(super) fn into_starts(self) -> Starts {
        self.starts
    }

    /// It may start from a world now ([`START_EVERY`]).
    pub(super) fn may_start(&self, now: Instant) -> bool {
        self.starts
            .last
            .is_none_or(|last| now.saturating_duration_since(last) >= START_EVERY)
    }

    pub(super) fn at(&self) -> SpectatorAt {
        match self.stage {
            Watch::Leaving { .. } => SpectatorAt::LetGo,
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
            self.wait_again();
            return;
        }
        let clock = watched.clock;
        let Some((index, world)) = watched
            .worlds
            .iter()
            .enumerate()
            .find(|(_, world)| world.time == time)
        else {
            self.go(Step::WaitAgain);
            return;
        };
        // Walked back from its end (a replay's list is the whole match up to
        // its clock), as far as one after the world can be listed.
        let since = watched.inputs.len()
            - watched
                .inputs
                .iter()
                .rev()
                .take_while(|(at, _, _)| *at + RELAYED_DISORDER_MS > time)
                .count();
        let replay: Vec<ServerMessage> = watched.inputs[since..]
            .iter()
            .filter(|(at, _, _)| *at > time && watched.due(*at))
            .map(|(_, from, what)| relay(*from, what))
            .collect();
        let mut fill = Vec::new();
        let most = replay
            .iter()
            .filter_map(|message| match message {
                ServerMessage::InputRelay { player, .. } => {
                    Some(count_input(&mut fill, usize::from(*player)))
                }
                _ => None,
            })
            .max();
        if self.ring_overrun(most.unwrap_or(0)) {
            return;
        }
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
        for message in replay.iter().chain(&entries) {
            self.send_waiting(message, now);
        }
        // Its generation, then the rest: the same for every game shown it.
        let rest = || {
            let mut payload = if watched.compress {
                ServerMessage::packed_snapshot(generation, world.time, &world.body)
            } else {
                ServerMessage::snapshot(generation, world.time, &world.body)
            }
            .payload();
            payload.split_off(1)
        };
        let mut payload = vec![generation];
        match watched.sent_worlds.get(index) {
            Some(made) => payload.extend_from_slice(made.get_or_init(rest)),
            None => payload.extend(rest()),
        }
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
                    fill,
                });
            }
            Err(_) => self.go(Step::WaitAgain),
        }
    }

    /// A delayed spectator's turn: the relays and roster changes its clock
    /// reached since the last, from the match's lists. Whether they were
    /// walked (its clock reached a new tick).
    fn feed(&mut self, view: &Watched<'_>, now: Instant) -> bool {
        let (Some(delay), Some(until)) = (self.delay, view.until) else {
            return false;
        };
        let from = delay.fed;
        // Once a tick: the lists are walked whole, and this runs for every
        // datagram the server takes. What is due has its lead (`HORIZON_MS`)
        // to spare.
        if until / TICK_MS <= from / TICK_MS {
            return false;
        }
        #[cfg(test)]
        {
            self.list_walks += 1;
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
        true
    }

    /// What its game may show of the kits: every place's playable ones, as a replay's viewer is
    /// told; a place is told once, and again when its kits change (a late joiner's). Only once
    /// it watches.
    pub(super) fn tell_kits(&mut self, profiles: &[MatchProfile]) {
        if !matches!(self.stage, Watch::Live { .. }) {
            return;
        }
        self.kits_told.resize(profiles.len(), 0);
        for note in super::kit_view::all_kits(profiles) {
            let told = &mut self.kits_told[usize::from(note.place)];
            if note.mask != *told {
                *told = note.mask;
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
                self.wait_again();
            }
            Watch::Live { .. } => {
                let until = relay_deadline(0, clock, time, now);
                let sent = self
                    .transport
                    .send_until(message.id(), &message.payload(), until);
                self.track(sent);
            }
            Watch::Syncing {
                time: from_time, ..
            } if time > from_time => {
                if let ServerMessage::InputRelay { .. } = message
                    && let Watch::Syncing { fill, .. } = &mut self.stage
                {
                    let count = count_input(fill, from);
                    if self.ring_overrun(count) {
                        return;
                    }
                }
                self.send_waiting(message, now);
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
                self.wait_again();
            }
            Watch::Live { .. } => {
                let sent = self.transport.send(message.id(), &message.payload());
                self.track(sent);
            }
            Watch::Syncing { time, .. } if entered && at > time => {
                self.send_waiting(message, now);
            }
            _ => {}
        }
    }

    /// Its link silent past its timeout (not one being let go).
    pub(super) fn timed_out(&self, timing: &super::super::Timing, now: Instant) -> bool {
        let timeout = match self.stage {
            Watch::Connecting | Watch::Loading => timing.loading_timeout,
            Watch::Leaving { .. } => return false,
            _ => timing.client_timeout,
        };
        self.transport.is_gone_after(now, timeout)
    }

    /// It waits for a world and has the `roster` places its start needs.
    fn waits_for_world(&self, roster: usize) -> bool {
        self.stage == Watch::Waiting && self.installed_roster >= roster
    }

    /// Its turn of a poll: one that waits starts from the world at `start`
    /// (0xE5 of its next generation), once it may ([`Self::may_start`]); one timed gets its 0xE5 again; one
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
                if let Some(time) =
                    start.filter(|_| self.installed_roster >= watched.roster && self.may_start(now))
                {
                    self.starts.last = Some(now);
                    self.generation = self.generation.next();
                    self.forget_timeline(now);
                    self.send_time(self.generation.0, echo, clock);
                    self.go(Step::TimeAsked { time, echo, now });
                }
            }
            Watch::Timing {
                echo, last, since, ..
            } => {
                if now.saturating_duration_since(since) >= TIMING_WAIT {
                    self.wait_again();
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
                        let sent = self.transport.send(leave.id(), &leave.payload());
                        self.track(sent);
                    }
                    self.go(Step::WorldIn);
                    self.starts.ring_redos = 0;
                    log::info!(
                        "Match {:#x}: spectator {:#x} watches",
                        watched.id,
                        self.session_id
                    );
                    // Its camera is on someone now (patch 143, the world's
                    // load): the asked place instead, once.
                    if let Some(place) = self.view.take() {
                        self.note(&replay_control::ServerNote::view(place));
                    }
                } else if now.saturating_duration_since(since) >= WORLD_WAIT {
                    self.wait_again();
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

    /// It waits for a world: none is on its way.
    pub(super) fn is_waiting(&self) -> bool {
        self.stage == Watch::Waiting
    }

    /// It plays from a world it took.
    pub(super) fn is_live(&self) -> bool {
        matches!(self.stage, Watch::Live { .. })
    }

    /// Its world went, with its last 0xE5 (on its way or in): its game's
    /// clock runs from that 0xE5's time, through the load too (`match.md`
    /// section 27).
    pub(super) fn world_sent(&self) -> bool {
        matches!(self.stage, Watch::Syncing { .. } | Watch::Live { .. })
    }

    /// Whether its room called it back ([`Self::recall`]).
    pub(super) fn is_recalled(&self) -> bool {
        self.recall.is_some()
    }

    /// Its room calls it back: asked each turn while it does. Let go
    /// [`replay_control::RECALL_SECONDS`] after the first call, and its game
    /// told the seconds left (`ServerNote::Recall`) once, as soon as it is
    /// live: one called back while its world is on its way hears it when the
    /// world is in. Whether those seconds are over.
    pub(super) fn recall(&mut self, now: Instant) -> bool {
        let due = match self.recall {
            Some(Recall::Told { due }) => return now >= due,
            Some(Recall::Untold { due }) => due,
            None => now + Duration::from_secs(replay_control::RECALL_SECONDS.into()),
        };
        // Whole seconds, rounded up: never told less than is left.
        let left = due
            .saturating_duration_since(now)
            .as_millis()
            .div_ceil(1_000);
        let seconds = u8::try_from(left).unwrap_or(u8::MAX);
        let told = seconds > 0
            && self
                .note(&replay_control::ServerNote::recall(seconds))
                .is_some();
        self.recall = Some(if told {
            Recall::Told { due }
        } else {
            Recall::Untold { due }
        });
        now >= due
    }

    /// A replay viewer's note (`replay_control::ServerNote`), in an 0xF6:
    /// only while live, never to a game let go. Its serial, for
    /// [`Self::unsend`]: one is kept until acked.
    pub(super) fn note(&mut self, payload: &[u8]) -> Option<u64> {
        if !self.is_live() {
            return None;
        }
        self.transport.send(id::UPLOAD_REQUEST, payload).ok()
    }

    /// A note sent ([`Self::note`]) that a newer one makes of no use goes
    /// no more, acked or not: only the newest matters.
    pub(super) fn unsend(&mut self, serial: u64, now: Instant) {
        self.transport.forget(serial, now);
    }

    /// Whether a note sent ([`Self::note`]) is in: its game acked it (or it
    /// was unsent).
    pub(super) fn is_delivered(&self, serial: u64) -> bool {
        self.transport.is_delivered(serial)
    }

    /// The latest round trip of its link, in ms (its game's delay in acking
    /// included).
    pub(super) fn round_trip(&self) -> Option<u32> {
        self.transport.last_round_trip()
    }

    /// Its game, not let go, has more than `most` messages not acked (a
    /// live spectator's [`BACKLOG_MOST`]): it is let go.
    pub(super) fn flooded(&self, most: usize) -> bool {
        !self.is_leaving() && self.transport.backlog() > most
    }

    #[cfg(test)]
    pub(super) fn backlog(&self) -> usize {
        self.transport.backlog()
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

    /// Kept until acked, at the pace of a game without a world, but no
    /// longer than a world is waited for: a game that never acks must not
    /// pile them up, and every poll of the match thread walks the queue.
    fn send_waiting(&mut self, message: &ServerMessage, now: Instant) {
        let sent = self.transport.send_kept_until(
            message.id(),
            &message.payload(),
            WAITING_RELAY_RESEND,
            now + WORLD_WAIT,
        );
        self.track(sent);
    }

    /// A relay or roster change of its current start went: kept until its
    /// next start ([`Self::forget_timeline`]). Those in or given up are
    /// dropped from the list as more go, so it stays about as long as what
    /// its game has not acked.
    fn track(&mut self, sent: Result<u64, SendError>) {
        let Ok(serial) = sent else {
            return;
        };
        if self.timeline.len() > 2 * self.transport.backlog() + TIMELINE_SLACK {
            let transport = &self.transport;
            self.timeline.retain(|&serial| in_flight(transport, serial));
        }
        self.timeline.push(serial);
    }

    /// Its game starts from a world again: what went for its last start
    /// goes no more. A copy of a relay or roster change carries a tick byte
    /// and no generation, so one that came after the new 0xE5 would be read
    /// against the new clock (a leave of then taking a place out of the new
    /// world, an input of then played in it); the new start sends what is
    /// due after its own world, as its first did. As a player's resync
    /// forgets its roster changes (`clear_gate`).
    fn forget_timeline(&mut self, now: Instant) {
        for serial in std::mem::take(&mut self.timeline) {
            if in_flight(&self.transport, serial) {
                self.transport.forget(serial, now);
            }
        }
    }

    /// One step of its game's machine: the only place its stage is set.
    fn go(&mut self, step: Step) {
        let stage = std::mem::replace(&mut self.stage, Watch::Waiting);
        self.stage = stage.step(step);
    }

    /// Its world's inputs, `fill` of some place, are past what its game
    /// keeps of them while the world is on its way ([`INPUT_RING_SAFE`]):
    /// loaded, it would be played wrong from its first tick. It starts
    /// again from a newer one (true), as a player's slow load is done again
    /// ([`SLOW_LOADS_REDONE`] times in a row at most: then it goes on).
    fn ring_overrun(&mut self, fill: usize) -> bool {
        if fill <= INPUT_RING_SAFE || self.starts.ring_redos >= SLOW_LOADS_REDONE {
            return false;
        }
        self.starts.ring_redos += 1;
        log::info!(
            "Spectator {:#x}: a place has {fill} inputs after its world, more than its game \
             keeps ({INPUT_RING_SAFE}; {} of {SLOW_LOADS_REDONE} times), so again from a newer one",
            self.session_id,
            self.starts.ring_redos
        );
        self.wait_again();
        true
    }

    /// Whether it may be started anew now for being out of step, as rarely
    /// as a player is resynced: [`RESYNCS_PER_MINUTE`] in [`RESYNC_WINDOW`],
    /// `interval` apart. If so, this start is counted.
    pub(super) fn take_drift_restart(&mut self, interval: Duration, now: Instant) -> bool {
        let drift = &mut self.starts.drift;
        while drift
            .front()
            .is_some_and(|at| now.saturating_duration_since(*at) >= RESYNC_WINDOW)
        {
            drift.pop_front();
        }
        let busy = drift.len() >= RESYNCS_PER_MINUTE
            || drift
                .back()
                .is_some_and(|at| now.saturating_duration_since(*at) < interval);
        if !busy {
            drift.push_back(now);
        }
        !busy
    }

    /// Back to wait for a world: its next start is of a new generation.
    pub(super) fn wait_again(&mut self) {
        self.go(Step::WaitAgain);
    }

    /// The server lets it go: an 0xEA naming no place, its game's end, sent
    /// for [`FAREWELL_GOES`]; then silence (see the module).
    pub(super) fn farewell(&mut self, clock: u32, now: Instant) {
        let farewell = ServerMessage::PlayerLeft {
            tick: tick_byte(clock),
            player: replay_control::LeftPlace::Farewell.byte(),
        };
        let _ = self.transport.send(farewell.id(), &farewell.payload());
        self.go(Step::LetGo {
            quiet_from: now + FAREWELL_GOES,
            until: now + SILENCE,
        });
    }
}
