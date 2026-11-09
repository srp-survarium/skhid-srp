//! Replay sessions: a recorded match played to its watcher's own game, in
//! the running client, as a spectated match (path A of
//! `research/replay-plan.md`; `docs/notes/replay-stream.md`). What the lobby
//! and the match server share of them, in memory: one [`Watches`] for the
//! srp process, in its [`super::replays::Replays`].
//!
//! - The lobby starts a session on the movie's request (`3D 00
//!   <REPLAY_WATCH | place << 16 | seconds> <match>`, [`Watches::start`]).
//! - The match server reads the match's recording on a thread of its own
//!   ([`Watches::next_job`]), judges the watcher against it
//!   ([`Watches::judge`]: the players of the match and the moderators, or
//!   anyone, `SRP_REPLAY_WATCH`; and the simulation epoch) and says where
//!   it starts ([`Watches::loaded`]) or why not ([`Watches::refuse`]).
//! - The lobby sends the game to the match server as it sends a spectator
//!   ([`Watches::to_send`], [`Watches::sent`]); the match server plays the
//!   recording to it ([`Watches::connected`]) until it leaves or the
//!   recording ends ([`Watches::ended`]).
//!
//! A session never has a seat, a match of the queue, a result or a reward.
//! While it is not over its player is in none of its room's matches
//! ([`Watches::holds`]). The movie's polls get its answer
//! ([`Watches::answer`]): `preparing`, `watching` with the time it starts
//! at, or why it was refused.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::replays::{MIN_REVISION, Refusal, Status, refusal_of};
use crate::LockExt;

/// `SRP_REPLAY_WATCH_MAX` when unset: sessions at once (loading, on their
/// way or playing). Each costs about a player's traffic and its recording
/// in memory (a few MB to a few tens); the host has one CPU and 512 MB.
pub const DEFAULT_MAX_SESSIONS: usize = 4;
/// `SRP_REPLAY_MAX_RECORDING_MB` when unset: a larger recording is not
/// read (`failed`). Reading one keeps it in memory with what is made of it
/// (`docs/notes/replay-stream.md`); the container has 512 MB for everything.
pub const DEFAULT_MAX_RECORDING_MB: u64 = 24;
/// A session whose recording is not read within this is refused `failed`.
pub const LOAD_WITHIN: Duration = Duration::from_secs(2 * 60);
/// A session made ready whose game does not reach the match server within
/// this is over (the lobby's link went, or the game never connected).
pub const CONNECT_WITHIN: Duration = Duration::from_secs(90);
/// A refusal the movie never polled for is forgotten after this.
const REFUSAL_KEPT: Duration = Duration::from_secs(15 * 60);

/// Who may watch a match's replay (`SRP_REPLAY_WATCH`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// `match`: its players and the moderators.
    Match,
    /// `all` (the default): every signed-in player.
    All,
}

/// `SRP_REPLAY_WATCH`, `SRP_REPLAY_MODERATORS`, `SRP_REPLAY_WATCH_MAX`,
/// `SRP_REPLAY_MAX_RECORDING_MB`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchSettings {
    pub access: Access,
    /// Accounts that may watch any match's replay
    /// (`SRP_REPLAY_MODERATORS`: account ids, comma-separated).
    pub moderators: HashSet<u32>,
    pub max_sessions: usize,
    /// A larger recording is not read.
    pub max_recording_bytes: u64,
}

impl Default for WatchSettings {
    fn default() -> Self {
        Self {
            access: Access::All,
            moderators: HashSet::new(),
            max_sessions: DEFAULT_MAX_SESSIONS,
            max_recording_bytes: DEFAULT_MAX_RECORDING_MB << 20,
        }
    }
}

impl WatchSettings {
    /// Whether `account_id` may watch a match's replay, having `played` it
    /// or not.
    pub fn may_watch(&self, account_id: u32, played: bool) -> bool {
        match self.access {
            Access::All => true,
            Access::Match => played || self.moderators.contains(&account_id),
        }
    }

    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let access = match var("SRP_REPLAY_WATCH").as_deref().map(str::trim) {
            None | Some("all") => Access::All,
            Some("match") => Access::Match,
            Some(other) => {
                log::warn!("SRP_REPLAY_WATCH={other:?} is neither `match` nor `all`: `match`");
                Access::Match
            }
        };
        let moderators = var("SRP_REPLAY_MODERATORS")
            .map(|list| {
                list.split(',')
                    .filter_map(|id| {
                        let parsed = id.trim().parse().ok();
                        if parsed.is_none() {
                            log::warn!("SRP_REPLAY_MODERATORS: {id:?} is no account id");
                        }
                        parsed
                    })
                    .collect()
            })
            .unwrap_or_default();
        let max_sessions = var("SRP_REPLAY_WATCH_MAX")
            .and_then(|max| max.trim().parse().ok())
            .unwrap_or(DEFAULT_MAX_SESSIONS);
        let max_recording_mb = match var("SRP_REPLAY_MAX_RECORDING_MB") {
            None => DEFAULT_MAX_RECORDING_MB,
            Some(value) => value
                .trim()
                .parse::<u64>()
                .ok()
                .filter(|&mb| mb <= u64::MAX >> 20)
                .unwrap_or_else(|| {
                    log::error!(
                        "SRP_REPLAY_MAX_RECORDING_MB is `{value}`; it must be a number. Using \
                         {DEFAULT_MAX_RECORDING_MB}"
                    );
                    DEFAULT_MAX_RECORDING_MB
                }),
        };
        Self {
            access,
            moderators,
            max_sessions,
            max_recording_bytes: max_recording_mb << 20,
        }
    }
}

/// A player's request to watch a match's replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchRequest {
    pub account_id: u32,
    pub session_id: u32,
    pub match_id: u32,
    pub from: WatchFrom,
    /// Its game's patch revision, when the server knows it.
    pub revision: Option<u8>,
}

/// Where a session starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchFrom {
    /// At this much of the match (snapped to a key frame at or before it,
    /// and never before the first view with a player in it).
    pub ms: u32,
    /// First in view (the start waits for its first entry; `None`: anyone's).
    pub place: Option<u8>,
}

/// Why a session is not given: what its player is told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchRefusal {
    Off,
    /// The game's revision is unknown, or older than [`MIN_REVISION`].
    OldGame,
    NotYours,
    /// No recording of it (or no more).
    Gone,
    /// Another simulation epoch's, and why (for the log; the movie says it).
    Incompatible(Refusal),
    Busy,
    Failed,
}

impl WatchRefusal {
    pub fn status(self) -> Status {
        match self {
            Self::Off => Status::Off,
            Self::OldGame => Status::OldGame,
            Self::NotYours => Status::NotYours,
            Self::Gone => Status::Gone,
            Self::Incompatible(_) => Status::Incompatible,
            Self::Busy => Status::Busy,
            Self::Failed => Status::Failed,
        }
    }
}

/// Where a session is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Waiting for the match server's reader.
    Queued,
    /// Its recording is being read.
    Loading,
    /// Read: the lobby sends its game, which starts at `start_ms`.
    Ready { start_ms: u32 },
    /// Its game was sent to the match server.
    Sent { start_ms: u32 },
    /// Its game watches.
    Playing { start_ms: u32 },
    /// Not given, until the movie is told.
    Refused(WatchRefusal),
}

impl Stage {
    /// Not refused: a session that counts, and holds its player out of its
    /// room's matches.
    fn is_open(self) -> bool {
        !matches!(self, Self::Refused(_))
    }

    /// How long a session may stay here before [`Event::TimedOut`]. Never
    /// for one that plays: its playback ends it, and the match server
    /// bounds a pause.
    fn patience(self) -> Option<Duration> {
        match self {
            Self::Queued | Self::Loading => Some(LOAD_WITHIN),
            Self::Ready { .. } | Self::Sent { .. } => Some(CONNECT_WITHIN),
            Self::Playing { .. } => None,
            Self::Refused(_) => Some(REFUSAL_KEPT),
        }
    }
}

/// What can happen to a session: [`step`]'s input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    /// The match server's reader takes it.
    Taken,
    /// Its recording is read.
    Loaded { start_ms: u32 },
    /// It is not given.
    Refuse(WatchRefusal),
    /// The lobby sent its game.
    SentGame,
    /// Its game reached the match server.
    Connected,
    /// Its game left, or its recording ended.
    Ended,
    /// Its game says it is back in the lobby.
    GameBack,
    /// It stayed longer than [`Stage::patience`].
    TimedOut,
    /// The movie's poll was told its refusal.
    Told,
}

/// Where an [`Event`] takes a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    To(Stage),
    Over,
}

/// The session's machine, the one place its stage changes. `None`: the
/// event means nothing in this stage (a word that came late), and nothing
/// changes.
fn step(stage: Stage, event: Event) -> Option<Step> {
    use Stage::{Loading, Playing, Queued, Ready, Refused, Sent};
    Some(match (stage, event) {
        (Queued, Event::Taken) => Step::To(Loading),
        (Loading, Event::Loaded { start_ms }) => Step::To(Ready { start_ms }),
        (Ready { start_ms }, Event::SentGame) => Step::To(Sent { start_ms }),
        (Sent { start_ms } | Playing { start_ms }, Event::Connected) => {
            Step::To(Playing { start_ms })
        }
        (Loading | Ready { .. }, Event::Refuse(refusal)) => Step::To(Refused(refusal)),
        (Queued | Loading, Event::TimedOut) => Step::To(Refused(WatchRefusal::Failed)),
        (Ready { .. } | Sent { .. } | Refused(_), Event::TimedOut) => Step::Over,
        (Refused(_), Event::Told) => Step::Over,
        (Sent { .. } | Playing { .. }, Event::GameBack) => Step::Over,
        (Queued | Loading | Ready { .. } | Sent { .. } | Playing { .. }, Event::Ended) => {
            Step::Over
        }
        _ => return None,
    })
}

/// What the movie's poll gets of a session ([`Watches::answer`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchAnswer {
    pub status: Status,
    pub match_id: u32,
    /// With [`Status::Watching`], the match time it starts at; with
    /// [`Status::Preparing`], 1 being read now, 2 waiting; else 0.
    pub figure: u32,
}

struct Session {
    request: WatchRequest,
    stage: Stage,
    /// When it came to its stage.
    since: Instant,
}

/// The replay sessions (the module's doc). `None` settings: off (no
/// recordings, `SRP_RECORD_DIR`), every request answered `off`.
pub struct Watches {
    settings: Option<WatchSettings>,
    /// By account: one session each.
    sessions: Mutex<HashMap<u32, Session>>,
}

impl Watches {
    pub fn new(settings: Option<WatchSettings>) -> Self {
        Self {
            settings,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn settings(&self) -> Option<&WatchSettings> {
        self.settings.as_ref()
    }

    // ------------------------------------------------------- the lobby

    /// A player asks to watch a match: `preparing` (its recording is read
    /// next), or why not at once. A player with a session not over gets
    /// `busy`, as does one past [`WatchSettings::max_sessions`].
    pub fn start(&self, request: WatchRequest, now: Instant) -> Status {
        let Some(settings) = &self.settings else {
            return Status::Off;
        };
        if !request.revision.is_some_and(|revision| revision >= MIN_REVISION) {
            return Status::OldGame;
        }
        let mut sessions = self.sessions.locked();
        if sessions
            .get(&request.account_id)
            .is_some_and(|session| session.stage.is_open())
        {
            return Status::Busy;
        }
        let open = sessions.values().filter(|s| s.stage.is_open()).count();
        if open >= settings.max_sessions {
            log::info!(
                "Replay sessions: account {} may not watch match {:#x}: {open} sessions run",
                request.account_id,
                request.match_id
            );
            return Status::Busy;
        }
        log::info!(
            "Replay sessions: account {} asks to watch match {:#x} from {:?}",
            request.account_id,
            request.match_id,
            request.from
        );
        sessions.insert(
            request.account_id,
            Session {
                request,
                stage: Stage::Queued,
                since: now,
            },
        );
        Status::Preparing
    }

    /// The answer for the movie's poll while the account has a session or
    /// a refusal not told yet (told now, and forgotten).
    pub fn answer(&self, account_id: u32) -> Option<WatchAnswer> {
        let mut sessions = self.sessions.locked();
        let session = sessions.get(&account_id)?;
        let match_id = session.request.match_id;
        let (status, figure) = match session.stage {
            Stage::Queued => (Status::Preparing, 2),
            Stage::Loading => (Status::Preparing, 1),
            Stage::Ready { start_ms } | Stage::Sent { start_ms } | Stage::Playing { start_ms } => {
                (Status::Watching, start_ms)
            }
            Stage::Refused(refusal) => (refusal.status(), 0),
        };
        if step(session.stage, Event::Told) == Some(Step::Over) {
            sessions.remove(&account_id);
        }
        Some(WatchAnswer {
            status,
            match_id,
            figure,
        })
    }

    /// Whether the account's session is not over: its player is in none of
    /// its room's matches meanwhile.
    pub fn holds(&self, account_id: u32) -> bool {
        self.sessions
            .locked()
            .get(&account_id)
            .is_some_and(|session| session.stage.is_open())
    }

    /// Whether the session's game is on its way to the match server or
    /// watches there: its lobby link may be quiet while it loads.
    pub fn in_replay(&self, session_id: u32) -> bool {
        self.sessions.locked().values().any(|session| {
            session.request.session_id == session_id
                && matches!(session.stage, Stage::Sent { .. } | Stage::Playing { .. })
        })
    }

    /// Whether the request's session is read and its game not here yet
    /// (ready, or sent): the match server keeps its recording meanwhile.
    pub fn waits_for_game(&self, request: &WatchRequest) -> bool {
        self.sessions
            .locked()
            .get(&request.account_id)
            .is_some_and(|session| {
                session.request == *request
                    && matches!(session.stage, Stage::Ready { .. } | Stage::Sent { .. })
            })
    }

    /// The sessions read and ready, whose games the lobby sends now
    /// ([`Self::sent`]), or turns down ([`Self::refuse`]).
    pub fn to_send(&self) -> Vec<(WatchRequest, u32)> {
        self.sessions
            .locked()
            .values()
            .filter_map(|session| match session.stage {
                Stage::Ready { start_ms } => Some((session.request, start_ms)),
                _ => None,
            })
            .collect()
    }

    /// The lobby sent the session's game to the match server.
    pub fn sent(&self, account_id: u32, now: Instant) {
        if let Some(session) = self.sessions.locked().get_mut(&account_id)
            && let Some(Step::To(stage)) = step(session.stage, Event::SentGame)
        {
            session.stage = stage;
            session.since = now;
        }
    }

    /// Sessions that waited too long: not read within [`LOAD_WITHIN`]
    /// (`failed`), or whose game did not come within [`CONNECT_WITHIN`]
    /// (over); refusals nobody polled for.
    pub fn expire(&self, now: Instant) {
        self.sessions.locked().retain(|account_id, session| {
            let waited = now.saturating_duration_since(session.since);
            if !session.stage.patience().is_some_and(|most| waited >= most) {
                return true;
            }
            let match_id = session.request.match_id;
            match session.stage {
                Stage::Queued | Stage::Loading => log::warn!(
                    "Replay sessions: match {match_id:#x} for account {account_id} was not read \
                     in time"
                ),
                Stage::Ready { .. } | Stage::Sent { .. } => log::info!(
                    "Replay sessions: account {account_id}'s game never came to watch match \
                     {match_id:#x}"
                ),
                Stage::Playing { .. } | Stage::Refused(_) => {}
            }
            match step(session.stage, Event::TimedOut) {
                Some(Step::To(stage)) => {
                    session.stage = stage;
                    session.since = now;
                    true
                }
                Some(Step::Over) => false,
                None => true,
            }
        });
    }

    // ------------------------------------------------ the match server

    /// The next session to read the recording of (oldest first), now
    /// [`Stage::Loading`].
    pub fn next_job(&self, now: Instant) -> Option<WatchRequest> {
        let mut sessions = self.sessions.locked();
        let session = sessions
            .values_mut()
            .filter(|session| session.stage == Stage::Queued)
            .min_by_key(|session| session.since)?;
        let Some(Step::To(stage)) = step(session.stage, Event::Taken) else {
            return None;
        };
        session.stage = stage;
        session.since = now;
        Some(session.request)
    }

    /// Whether the request's player may watch a recording of `players`
    /// (every place's account) made by games of `revisions` (by place):
    /// `not_yours` for one who did not play it and is no moderator (unless
    /// anyone may), `incompatible` for another simulation epoch than the
    /// player's game (`replays::refusal_of`, `oldest` the oldest
    /// replayable revision).
    pub fn judge(
        &self,
        request: &WatchRequest,
        players: &[u32],
        revisions: &[Option<u8>],
        oldest: u8,
    ) -> Result<(), WatchRefusal> {
        let settings = self.settings.as_ref().ok_or(WatchRefusal::Off)?;
        if !settings.may_watch(request.account_id, players.contains(&request.account_id)) {
            return Err(WatchRefusal::NotYours);
        }
        let revision = request.revision.ok_or(WatchRefusal::OldGame)?;
        match refusal_of(revisions, revision, oldest) {
            Some(refusal) => Err(WatchRefusal::Incompatible(refusal)),
            None => Ok(()),
        }
    }

    /// The session's recording is read: it starts at `start_ms`. False when
    /// the session is gone meanwhile.
    pub fn loaded(&self, request: &WatchRequest, start_ms: u32, now: Instant) -> bool {
        self.advance(request, Event::Loaded { start_ms }, now)
    }

    /// The session is not given (its recording, its player, the epoch).
    pub fn refuse(&self, request: &WatchRequest, refusal: WatchRefusal, now: Instant) {
        log::info!(
            "Replay sessions: account {} is not shown match {:#x}: {refusal:?}",
            request.account_id,
            request.match_id,
        );
        self.advance(request, Event::Refuse(refusal), now);
    }

    /// A game of `session_id` connects to the match server: its session's
    /// request and start, now [`Stage::Playing`], if it was sent to watch.
    pub fn connected(&self, session_id: u32, now: Instant) -> Option<(WatchRequest, u32)> {
        let mut sessions = self.sessions.locked();
        let session = sessions
            .values_mut()
            .find(|session| session.request.session_id == session_id)?;
        let Some(Step::To(stage @ Stage::Playing { start_ms })) =
            step(session.stage, Event::Connected)
        else {
            return None;
        };
        session.stage = stage;
        session.since = now;
        Some((session.request, start_ms))
    }

    /// The session is over (its game left, its recording ended, or its game
    /// never came): its player is free again.
    pub fn ended(&self, request: &WatchRequest) {
        let mut sessions = self.sessions.locked();
        if sessions.get(&request.account_id).is_some_and(|session| {
            session.request == *request
                && step(session.stage, Event::Ended) == Some(Step::Over)
        }) {
            sessions.remove(&request.account_id);
            log::info!(
                "Replay sessions: account {}'s session of match {:#x} is over",
                request.account_id,
                request.match_id
            );
        }
    }

    /// Whether the request's session is still open: the match server drops the playback of one
    /// that is not ([`Watches::game_back`]).
    pub fn is_open(&self, request: &WatchRequest) -> bool {
        self.sessions
            .locked()
            .get(&request.account_id)
            .is_some_and(|session| session.request == *request && session.stage.is_open())
    }

    /// The game of `session_id` says it is back in the lobby (`31 .. 1`). A game that watched
    /// leaves through the match server and says nothing here, so this one quit on its way to the
    /// match: its session ends now, or it would hold the account until the match server gave up
    /// on it. A session not sent yet is no business of this word.
    pub fn game_back(&self, session_id: u32) -> bool {
        let mut sessions = self.sessions.locked();
        let account = sessions.iter().find_map(|(account, session)| {
            (session.request.session_id == session_id
                && step(session.stage, Event::GameBack) == Some(Step::Over))
            .then_some(*account)
        });
        let Some(account) = account else { return false };
        if let Some(session) = sessions.remove(&account) {
            log::info!(
                "Replay sessions: account {account}'s game came back from match {:#x} on its own; \
                 its session is over",
                session.request.match_id
            );
        }
        true
    }

    /// The event happens to the account's session, if it is still this
    /// request's and the event means something in its stage. Whether it did.
    fn advance(&self, request: &WatchRequest, event: Event, now: Instant) -> bool {
        let mut sessions = self.sessions.locked();
        let Some(session) = sessions
            .get_mut(&request.account_id)
            .filter(|session| session.request == *request)
        else {
            return false;
        };
        match step(session.stage, event) {
            Some(Step::To(stage)) => {
                session.stage = stage;
                session.since = now;
                true
            }
            Some(Step::Over) => {
                sessions.remove(&request.account_id);
                true
            }
            None => false,
        }
    }

    /// The account's session's stage, if it has one.
    pub fn stage(&self, account_id: u32) -> Option<Stage> {
        self.sessions
            .locked()
            .get(&account_id)
            .map(|session| session.stage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: u32 = 0x1_0001;
    const MOD: u32 = 0x1_0009;

    fn request(account_id: u32) -> WatchRequest {
        WatchRequest {
            account_id,
            session_id: account_id + 0x100,
            match_id: 0x1F07_C8FB,
            from: WatchFrom { ms: 0, place: None },
            revision: Some(40),
        }
    }

    fn watches(access: Access, max_sessions: usize) -> Watches {
        Watches::new(Some(WatchSettings {
            access,
            moderators: HashSet::from([MOD]),
            max_sessions,
            ..WatchSettings::default()
        }))
    }

    #[test]
    fn anyone_watches_by_default_and_match_keeps_it_to_players_and_moderators() {
        let players = [ALICE];
        let revisions = [Some(40)];
        let open = Watches::new(Some(WatchSettings::default()));
        assert_eq!(
            open.judge(&request(0x2_0000), &players, &revisions, 30),
            Ok(())
        );
        let watches = watches(Access::Match, 4);
        assert_eq!(
            watches.judge(&request(ALICE), &players, &revisions, 30),
            Ok(())
        );
        assert_eq!(
            watches.judge(&request(MOD), &players, &revisions, 30),
            Ok(())
        );
        assert_eq!(
            watches.judge(&request(0x2_0000), &players, &revisions, 30),
            Err(WatchRefusal::NotYours)
        );
        // Another epoch: refused with why, for a player of it too.
        assert_eq!(
            watches.judge(&request(ALICE), &players, &[Some(20)], 30),
            Err(WatchRefusal::Incompatible(Refusal::Older {
                from: 20,
                to: 20,
                oldest: 30
            }))
        );
    }

    /// A game whose revision the server does not know is no game of
    /// revision 0: it may not start, and is told so, not "recorded by a
    /// newer game".
    #[test]
    fn a_game_of_unknown_revision_gets_old_game() {
        let now = Instant::now();
        let watches = watches(Access::All, 4);
        let unknown = WatchRequest {
            revision: None,
            ..request(ALICE)
        };
        assert_eq!(watches.start(unknown, now), Status::OldGame);
        assert!(!watches.holds(ALICE));
        assert_eq!(
            watches.judge(&unknown, &[ALICE], &[Some(40)], 30),
            Err(WatchRefusal::OldGame)
        );
    }

    #[test]
    fn a_session_goes_from_its_request_to_its_end_and_counts_against_the_cap() {
        let now = Instant::now();
        let watches = watches(Access::Match, 1);
        let alice = request(ALICE);
        assert_eq!(watches.start(alice, now), Status::Preparing);
        assert!(watches.holds(ALICE));
        // One session at a time: the cap, and the same player again.
        assert_eq!(watches.start(request(MOD), now), Status::Busy);
        assert_eq!(watches.start(alice, now), Status::Busy);
        assert_eq!(watches.next_job(now), Some(alice));
        assert_eq!(watches.next_job(now), None);
        assert_eq!(watches.answer(ALICE).unwrap().status, Status::Preparing);
        assert!(watches.loaded(&alice, 14_500, now));
        assert_eq!(watches.to_send(), [(alice, 14_500)]);
        assert!(!watches.in_replay(alice.session_id));
        watches.sent(ALICE, now);
        assert!(watches.in_replay(alice.session_id));
        assert_eq!(
            watches.connected(alice.session_id, now),
            Some((alice, 14_500))
        );
        let answer = watches.answer(ALICE).unwrap();
        assert_eq!((answer.status, answer.figure), (Status::Watching, 14_500));
        watches.ended(&alice);
        assert!(!watches.holds(ALICE));
        assert_eq!(watches.answer(ALICE), None);
        assert_eq!(watches.start(request(MOD), now), Status::Preparing);
    }

    #[test]
    fn a_game_back_in_the_lobby_ends_its_session_once_it_was_sent() {
        let now = Instant::now();
        let watches = Watches::new(Some(WatchSettings::default()));
        let alice = request(ALICE);
        assert_eq!(watches.start(alice, now), Status::Preparing);
        // Asked, not sent: the word is about something else.
        assert!(!watches.game_back(alice.session_id));
        assert!(watches.holds(ALICE));
        assert_eq!(watches.next_job(now), Some(alice));
        assert!(watches.loaded(&alice, 14_500, now));
        assert!(!watches.game_back(alice.session_id));
        assert!(watches.holds(ALICE));
        watches.sent(ALICE, now);
        assert!(watches.is_open(&alice));
        // Sent: over, and the account may ask again at once.
        assert!(watches.game_back(alice.session_id));
        assert!(!watches.holds(ALICE));
        assert!(!watches.is_open(&alice));
        assert_eq!(watches.start(alice, now), Status::Preparing);
        assert_eq!(watches.next_job(now), Some(alice));
        assert!(watches.loaded(&alice, 14_500, now));
        watches.sent(ALICE, now);
        assert_eq!(watches.connected(alice.session_id, now), Some((alice, 14_500)));
        // At the match server too: the game quit while connecting.
        assert!(watches.game_back(alice.session_id));
        assert!(!watches.holds(ALICE));
        // Another's word touches nothing, and there is nothing left to end.
        assert!(!watches.game_back(alice.session_id));
    }

    #[test]
    fn a_refusal_is_told_once_and_holds_nothing() {
        let now = Instant::now();
        let watches = watches(Access::Match, 1);
        let alice = request(ALICE);
        watches.start(alice, now);
        watches.next_job(now);
        let why = Refusal::Older {
            from: 20,
            to: 20,
            oldest: 30,
        };
        watches.refuse(&alice, WatchRefusal::Incompatible(why), now);
        assert!(!watches.holds(ALICE));
        assert_eq!(watches.start(request(MOD), now), Status::Preparing);
        assert_eq!(watches.answer(ALICE).unwrap().status, Status::Incompatible);
        assert_eq!(watches.answer(ALICE), None);
    }

    /// A refusal is for a session not given yet: one that plays is not
    /// taken back by a word that came late, and nothing but an event of its
    /// stage moves a session.
    #[test]
    fn a_session_moves_only_as_its_stage_allows() {
        let now = Instant::now();
        let watches = watches(Access::All, 4);
        let alice = request(ALICE);
        watches.start(alice, now);
        // Not read yet: neither ready nor refused.
        assert!(!watches.loaded(&alice, 50, now));
        watches.refuse(&alice, WatchRefusal::Failed, now);
        assert_eq!(watches.stage(ALICE), Some(Stage::Queued));
        watches.next_job(now);
        assert!(watches.loaded(&alice, 50, now));
        watches.sent(ALICE, now);
        assert_eq!(watches.connected(alice.session_id, now), Some((alice, 50)));
        watches.refuse(&alice, WatchRefusal::Busy, now);
        assert_eq!(watches.stage(ALICE), Some(Stage::Playing { start_ms: 50 }));
        // A session that plays does not expire by waiting.
        watches.expire(now + LOAD_WITHIN + CONNECT_WITHIN + REFUSAL_KEPT);
        assert_eq!(watches.stage(ALICE), Some(Stage::Playing { start_ms: 50 }));
        assert_eq!(step(Stage::Playing { start_ms: 50 }, Event::Told), None);
        assert_eq!(
            step(Stage::Refused(WatchRefusal::Busy), Event::Ended),
            None,
            "a refused session is the movie's to be told"
        );
    }

    #[test]
    fn stuck_sessions_end_and_off_or_old_games_get_nothing() {
        let now = Instant::now();
        let watches = watches(Access::Match, 4);
        let (alice, other) = (request(ALICE), request(MOD));
        watches.start(alice, now);
        watches.start(other, now);
        watches.next_job(now);
        watches.next_job(now);
        assert!(watches.loaded(&other, 50, now));
        watches.sent(MOD, now);
        watches.expire(now + CONNECT_WITHIN);
        assert!(!watches.holds(MOD));
        assert!(watches.holds(ALICE));
        watches.expire(now + LOAD_WITHIN);
        assert_eq!(watches.answer(ALICE).unwrap().status, Status::Failed);
        let old = WatchRequest {
            revision: Some(MIN_REVISION - 1),
            ..alice
        };
        assert_eq!(watches.start(old, now), Status::OldGame);
        assert_eq!(Watches::new(None).start(alice, now), Status::Off);
    }
}
