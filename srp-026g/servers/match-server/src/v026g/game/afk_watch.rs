//! Anti-AFK in the match (`docs/notes/anti-afk.md`, the rule in
//! [`super::afk`]): what each player is doing at a poll, and the removal of
//! a player idle too long, the way a player who leaves goes.

use std::time::Instant;

use survarium::v026g::match_msgs::ServerMessage;
use survarium::v026g::match_msgs::rules;

use super::afk::{Action, Presence};
use super::{Game, IdleRemoved, Stage, afk, farewell_tick, send};

/// A player's alive or dead is read from the newest trusted world while
/// that world is at most this far behind the match clock (worlds come
/// every few seconds); with none so fresh, its time does not count.
const ALIVE_FRESH_MS: u32 = 10_000;

impl Game {
    /// Each player's time without input goes on or pauses; a player idle
    /// too long is removed.
    pub(super) fn check_idle(&mut self, now: Instant) {
        if self.afk.is_exempt() {
            return;
        }
        let mut idle = Vec::new();
        for index in 0..self.peers.len() {
            // A bot's place is never looked at.
            if self.is_bot(index) {
                continue;
            }
            let presence = self.presence(index, now);
            if self.afk.poll(index, presence, now) == Action::Remove {
                idle.push(index);
            }
        }
        for index in idle {
            self.remove_idle(index, now);
        }
    }

    /// Whether the player at `index` plays now: alive in a match in play
    /// (not loading, not before play or after it), its game in the world
    /// (not entering, not being resynced) and heard from.
    fn presence(&self, index: usize, now: Instant) -> Presence {
        let in_play = self.finished_at.is_none()
            && self.ending.is_none()
            && self.rule.state == rules::state::IN_PROCESS;
        let Some(peer) = self.peers[index].as_ref() else {
            return Presence::Away;
        };
        let in_world =
            peer.stage == Stage::Live && peer.resync_from.is_none() && !self.waiting_entry(index);
        let clock = self.clock(now);
        let alive = self
            .state
            .as_ref()
            .filter(|state| clock.saturating_sub(state.time) <= ALIVE_FRESH_MS)
            .and_then(|state| state.supplies.get(index)?.as_ref())
            .is_some_and(|supplies| supplies.alive);
        if in_play && in_world && alive && afk::heard(peer.simulating_at, now) {
            Presence::Playing
        } else {
            Presence::Away
        }
    }

    /// The players removed as idle since the last call: the lobby's chat
    /// says so (`ResultsSink::idle_removed`).
    pub fn take_idle_removed(&mut self) -> Vec<IdleRemoved> {
        std::mem::take(&mut self.idle_removed)
    }

    /// The player at `index` gave no input for the limit: out of the match
    /// as a player who leaves by its own word (0xE0) is, its place given up
    /// (`take_quits`), and its game told it left (0xEA with its own index,
    /// as at the end), which closes the match on it. The chat is told once
    /// (`take_idle_removed`).
    fn remove_idle(&mut self, index: usize, now: Instant) {
        let clock = self.clock(now);
        let place = self
            .formed
            .places
            .get(index)
            .map(|place| (place.account_id, place.name.clone()));
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        if peer.stage == Stage::Gone {
            return;
        }
        log::info!(
            "Match {:#x}: player {index} gave no input in its play for the anti-AFK limit; \
             removed as a player who leaves",
            self.id
        );
        let message = ServerMessage::PlayerLeft {
            tick: farewell_tick(peer, clock),
            player: index as u8,
        };
        peer.farewell = send(peer, &message).map(|serial| (serial, message));
        if let Some((account_id, name)) = place {
            self.quits.push((peer.session_id, account_id));
            self.idle_removed.push(IdleRemoved {
                match_id: self.id,
                account_id,
                name,
            });
        }
        self.give_up(index);
        self.leave(index, now);
    }
}
