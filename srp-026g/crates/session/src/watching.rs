//! What each session is watching: one state per session, so that a session
//! is never both a match's spectator and a replay's watcher, and whoever
//! keeps a player out of a queue asks one place.
//!
//! - [`Watching::Match`]: a running match, from its room's queue (Queue and
//!   spectate) or the Live window; [`crate::WatchKind`] says which
//!   ([`crate::Matchmaking`] keeps these).
//! - [`Watching::Replay`]: a replay session and its stage
//!   ([`crate::v026g::replay_watch`] keeps these).
//!
//! Both work on the same table, shared through an `Arc`: the process's
//! [`crate::Matchmaking`] makes it and its replay sessions are given it.
//! The lock is the last taken: nothing holds it while asking for another.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use crate::LockExt;
use crate::matchmaking::Watcher;
use crate::v026g::replay_watch::ReplaySession;

/// What a session is watching.
pub(crate) enum Watching {
    Match(Watcher),
    Replay(ReplaySession),
}

impl Watching {
    /// Whether it holds its session: a refused replay waits only to be told.
    pub(crate) fn is_open(&self) -> bool {
        match self {
            Self::Match(_) => true,
            Self::Replay(replay) => replay.is_open(),
        }
    }
}

/// The table of what every session is watching, by session id.
#[derive(Default)]
pub struct WatchTable {
    sessions: Mutex<HashMap<u32, Watching>>,
}

impl WatchTable {
    pub(crate) fn lock(&self) -> MutexGuard<'_, HashMap<u32, Watching>> {
        self.sessions.locked()
    }
}
