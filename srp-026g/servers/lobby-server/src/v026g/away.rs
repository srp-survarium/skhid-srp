//! The players whose link went silent: when a client is really gone.
//!
//! What the client does (`docs/notes/transport.md` §6.1): its
//! logic thread ticks the lobby link every 500 ms in every state (in the
//! lobby, in a match, with its window out of focus, while it waits for
//! its renderer), and the link then sends a keep-alive when it has nothing
//! else. So a client is silent only while that thread is held up (loading
//! a level [LIKELY]), its machine sleeps, or its network or its game is
//! gone. When the server is silent for 5 s the client drops the link,
//! shows its sync window and connects again with the session it has, every
//! 5 s; after the fifth failure it signs out to its login screen.
//!
//! So the lobby's rule has three steps:
//!
//! 1. **The link goes** after [`Settings::link_timeout`] of silence
//!    ([`Settings::match_link_timeout`] for a player in a match that runs).
//!    The player is out of the lobby: out of its clan team's wait, and its
//!    friends see it offline. Its room keeps its place until its session
//!    ends (step 3: about 90 s of silence in all, the owner's of 2026-10-18).
//!    A client that leaves in words (`FA`) ends its session at once
//!    instead.
//! 2. **The session waits** for [`Settings::session_grace`]: a client that
//!    connects again with it is taken back as it was, in its match if that
//!    still runs. While the match of a player who was in one runs, the
//!    wait does not count: it starts when the match is over, so the client
//!    comes back to the lobby after the match without signing in again.
//!    (The store takes a player nothing was heard of for 30 s out of its
//!    squad: `SessionStore::drop_gone_players`.)
//! 3. **The session ends** when the wait is over, or at once when the
//!    account signs in anew.
//!
//! [`Settings::link_timeout`]: super::Settings::link_timeout
//! [`Settings::match_link_timeout`]: super::Settings::match_link_timeout
//! [`Settings::session_grace`]: super::Settings::session_grace

use std::net::SocketAddr;
use std::time::Instant;

use super::core::{Core, Stage};

/// A session whose link went silent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Away {
    pub account_id: u32,
    /// The match (the queue's id) its player was in.
    pub in_match: Option<u32>,
    /// When the wait began: when the link went, or the match ended.
    pub since: Instant,
}

impl Core<'_> {
    /// Nothing came from the client of `addr` for its timeout: step 1.
    pub(super) fn link_went_silent(&mut self, addr: SocketAddr, now: Instant) {
        let player = match self.links.get(&addr).map(|link| &link.stage) {
            Some(Stage::Player(player)) => {
                Some((player.session_id, player.account_id, player.in_match))
            }
            _ => None,
        };
        // Its session stays for now, unless nothing is to be waited for.
        let wait = !self.settings.session_grace.is_zero();
        let in_match =
            player.is_some_and(|(_, _, in_match)| in_match.is_some_and(|id| self.runs(id)));
        self.retire(addr, "nothing came from its client", !(wait || in_match));
        self.links.remove(&addr);
        if let Some((session_id, account_id, in_match)) = player
            && !self.store.has_ended(session_id)
        {
            self.away.insert(
                session_id,
                Away {
                    account_id,
                    in_match,
                    since: now,
                },
            );
        }
    }

    /// A sign-in with `session_id` of `account_id`: the session is not away
    /// any more, and the account's other sessions that were are ended (its
    /// player signed in anew). Returns the match the session's player was
    /// in when its link went, if that still runs.
    pub(super) fn back_from_away(&mut self, session_id: u32, account_id: u32) -> Option<u32> {
        let stale: Vec<u32> = self
            .away
            .iter()
            .filter(|(session, away)| **session != session_id && away.account_id == account_id)
            .map(|(session, _)| *session)
            .collect();
        for session in stale {
            self.away.remove(&session);
            self.store.end_session(session);
        }
        let away = self.away.remove(&session_id)?;
        away.in_match.filter(|id| self.runs(*id))
    }

    /// Steps 2 and 3, on every tick.
    pub(super) fn collect_away(&mut self, now: Instant) {
        let grace = self.settings.session_grace;
        let mut ended = Vec::new();
        let matches = &self.matches;
        let matchmaking = self.store.matchmaking();
        for (session_id, away) in &mut self.away {
            // The wait of a player in a match starts when the match is over.
            // A spectator has no seat to wait for: its wait counts from its
            // link going, and its session's end gives its place among the
            // match's few spectators back (its game may never have got there).
            if away.in_match.is_some_and(|id| matches.contains_key(&id))
                && matchmaking.watched(*session_id).is_none()
            {
                away.since = now;
            } else if now.saturating_duration_since(away.since) >= grace {
                ended.push(*session_id);
            }
        }
        for session_id in ended {
            // Its room loses it with its session (`rooms`), unless its
            // account is in the lobby again under another one.
            if let Some(away) = self.away.remove(&session_id)
                && !self.by_account.contains_key(&away.account_id)
            {
                self.room_gone(away.account_id);
            }
            if self.store.end_session(session_id) {
                log::info!(
                    "Session {session_id:#x} ended: its client did not connect again in {} s",
                    grace.as_secs()
                );
            }
        }
    }

    /// Whether the lobby has the match as one that runs.
    fn runs(&self, match_id: u32) -> bool {
        self.matches.contains_key(&match_id)
    }
}
