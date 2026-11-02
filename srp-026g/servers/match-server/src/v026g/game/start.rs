//! A room's match starts with the players who are in (`docs/notes/match.md`
//! section 34; the owner's of 2026-10-20: "isn't it better to just let
//! people play, since rules are in rooms").
//!
//! The clients start the countdown once `floor(0.8 × N)` of the N roster
//! places have entered (`TimelimitRule`), N fixed when the match was made.
//! A place given up before that (its player left by its own word) never
//! comes, nor does a game that never connects: live, room 5209 (2026-10-20,
//! 5 places, 4 needed, 2 given up) waited in "Awaiting other players" with
//! its two players until the rule's 5 minutes. The room's own rule is its
//! minimum (`MatchSetup::room_minimum`), so:
//!
//! - **Stand-ins.** When the places that can still come cannot make the
//!   clients' number, or `RoomRules::match_start_wait_ms` after the first entry, and the
//!   room's minimum is in with a player on each side (any side with a
//!   minimum of one, below), the server enters
//!   as many places as are missing that no game holds (given up first) on
//!   one tick: every game starts its countdown there, as for players who
//!   came. Each such place leaves again once every game has played that
//!   tick (a game that has not by [`STAND_IN_LEAVE_CAP`] is resynced
//!   first): a leave for a tick is taken only by a game whose simulation
//!   has the player in by then (0xEA's `leave_match` 0x2D18A0 reads the
//!   entered mask of the newest played node at or before the tick,
//!   0x2D1B4B..0x2D1B9D, and ignores a player not in it). A countdown never
//!   goes back to waiting (`logic_tick` 0x318100), so it runs on. The
//!   respawn rule spawns an entering player at its tick (0x316BC0,
//!   0x316820), so a stand-in's body stands at a spawn point for that half
//!   second or so; it has no input, no kill, no result, and the results
//!   count it nowhere.
//! - **Called off.** When fewer than the room's minimum, or nobody of a
//!   side, are in or can still come, the match ends before its countdown
//!   as not made ([`MatchEnd::NotMade`]): nothing of it counts, and the
//!   lobby gives its room the vote and the wait again.
//! - **A minimum of one** (a private room, started by its owner with whoever
//!   is in; the owner's of 2026-10-26): no side needs a player, so a match of one starts 1 v 0
//!   (its one place is the clients' whole number, `floor(0.8 × 1)` = 0:
//!   its entry starts the countdown), and a 1 v 1 whose other player gave
//!   up goes on as 1 v 0.

use super::*;
use survarium::v026g::lobby::rooms::rules;

/// A stand-in's leave goes once every game simulating has reported a tick
/// at or past its entry (it played the entry). A game that has not this
/// long after the entry is started anew from a world of a game that played
/// it (a resync), and the leave goes without waiting for it: sent to a game
/// that has not played the entry, the leave would be dropped (`leave_match`
/// 0x2D1B4B..0x2D1B9D), the stand-in left in that game alone and its
/// hashes wrong from then on. A game being resynced gets the entry and the
/// leave with its replay, as any roster change of its time out.
const STAND_IN_LEAVE_CAP: Duration = Duration::from_secs(2);

/// A place the server entered so that the clients start the countdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StandIn {
    pub index: usize,
    /// Its entry's tick (match ms).
    pub entered_at: u32,
    /// Its leave's tick, once sent.
    pub left_at: Option<u32>,
    /// When it was entered.
    pub since: Instant,
}

impl Game {
    /// Whether a roster change is a stand-in's ([`StandIn`]): no player's.
    pub(super) fn is_stand_in_event(&self, at: u32, index: usize, entered: bool) -> bool {
        self.stand_ins.iter().any(|stand_in| {
            stand_in.index == index
                && if entered {
                    stand_in.entered_at == at
                } else {
                    stand_in.left_at == Some(at)
                }
        })
    }

    /// The players' roster changes: the stand-ins' left out. What a player
    /// did (came, left, came back) reads these; the order of a place's
    /// roster changes in the games (each a tick after its last) reads them
    /// all.
    pub(super) fn players_events(&self) -> impl Iterator<Item = &(u32, usize, bool)> {
        self.events
            .iter()
            .filter(|(at, who, entered)| !self.is_stand_in_event(*at, *who, *entered))
    }

    /// The place stands in now: entered, its leave not sent yet. A game of
    /// it that connects meanwhile is held before its start (`start_sync`):
    /// its player enters after the stand-in's leave, from a world after
    /// it; never a second time while the stand-in is in, nor taken out by
    /// the stand-in's leave.
    pub(super) fn stands_in(&self, index: usize) -> bool {
        self.stand_ins
            .iter()
            .any(|stand_in| stand_in.index == index && stand_in.left_at.is_none())
    }

    /// Another roster change than a stand-in's goes after the stand-ins'
    /// entry: on that tick the games find the stand-ins alone (a game takes
    /// a tick's changes by place, entries before leaves, 0x2D2180), so
    /// every game starts the countdown there, as the server does.
    pub(super) fn after_stand_ins(&self, at: u32) -> u32 {
        match self
            .stand_ins
            .iter()
            .map(|stand_in| stand_in.entered_at)
            .max()
        {
            Some(entry) if at <= entry => entry + TICK_MS,
            _ => at,
        }
    }

    /// The player left by its own word (0xE0): its place never comes back
    /// (`Server::receive` frees its seat).
    pub(super) fn give_up(&mut self, index: usize) {
        if self.given_up.len() <= index {
            self.given_up.resize(index + 1, false);
        }
        self.given_up[index] = true;
    }

    /// A room's match before its countdown: starts it with who is in, or
    /// calls it off; and sends the stand-ins' leaves that are due.
    pub(super) fn poll_start(&mut self, now: Instant) {
        self.send_stand_in_leaves(now);
        let Some(minimum) = self.formed.room_minimum else {
            return;
        };
        if self.clock_start.is_none()
            || self.finished_at.is_some()
            || self.ending.is_some()
            || self.rule.state != rules::state::WAITING_FOR_PLAYERS
            || !self.stand_ins.is_empty()
        {
            return;
        }
        let clock = self.clock(now);
        let waited = self
            .first_entered
            .is_some_and(|first| clock >= first.saturating_add(rules().match_start_wait_ms));
        let mask = self.rule.mask;
        let is_in = |index: usize| index < 32 && mask & (1 << index) != 0;
        let mut humans_in = [0usize; 2];
        let mut humans_able = [0usize; 2];
        let mut able = 0usize;
        // Places no game holds: given up ones first.
        let mut free: Vec<(bool, usize)> = Vec::new();
        for (index, place) in self.formed.places.iter().enumerate() {
            let team = usize::from(place.team.min(1));
            // A test bot (`SRP_BOT_ACCOUNTS`) is a member of its room as a
            // player is; the server's own bots are in no room's match.
            let human = !place.driven;
            let connected = self.peers[index]
                .as_ref()
                .is_some_and(|peer| peer.stage != Stage::Gone);
            let given_up = self.given_up.get(index).copied().unwrap_or(false);
            if is_in(index) {
                if human {
                    humans_in[team] += 1;
                    humans_able[team] += 1;
                }
                continue;
            }
            // Never connected, or lost: waited for until the start's wait
            // is over; a game connected (loading) as long as it is, and a
            // place taken into the match until its admission is in or has
            // failed (its own deadline, `late_join`).
            let may_come = !given_up && (connected || !waited || self.coming(index));
            if may_come {
                able += 1;
                if human {
                    humans_able[team] += 1;
                }
            }
            if !connected && self.admissions[index].is_none() {
                free.push((!given_up, index));
            }
        }
        let minimum = usize::from(minimum);
        // A player a side, except in a match of one's minimum (a private
        // room's 1 v 0): one side may be empty there.
        let both_sides = minimum > 1;
        let (now_in, could) = (humans_in[0] + humans_in[1], humans_able[0] + humans_able[1]);
        if could < minimum || (both_sides && humans_able.contains(&0)) {
            log::warn!(
                "Match {:#x}: called off at {clock} ms, before its countdown: {now_in} players \
                 in and {} more able to come ({} v {} at most), the room needs {minimum}",
                self.id,
                could - now_in,
                humans_able[0],
                humans_able[1]
            );
            self.finish(MatchEnd::NotMade, now);
            return;
        }
        let needed = self.rule.params.players_needed() as usize;
        let entered = mask.count_ones() as usize;
        let reachable = entered + able >= needed;
        if (reachable && !waited) || now_in < minimum || (both_sides && humans_in.contains(&0)) {
            return;
        }
        let missing = needed.saturating_sub(entered);
        if missing == 0 || free.len() < missing {
            // Games still loading make up the rest: they enter, or time
            // out and leave their place free.
            return;
        }
        free.sort_unstable();
        let mut chosen: Vec<usize> = free[..missing].iter().map(|(_, index)| *index).collect();
        chosen.sort_unstable();
        let last = self.events.iter().map(|(at, _, _)| *at).max().unwrap_or(0);
        let at = grid_up(clock + ROSTER_LEAD_MS).max(last + TICK_MS);
        log::info!(
            "Match {:#x}: {entered} of the {needed} places the clients' start needs are in \
             ({} v {} players; {}); places {chosen:?} stand in at {at} ms, and the countdown \
             starts with the players who are there (the room needs {minimum})",
            self.id,
            humans_in[0],
            humans_in[1],
            if waited {
                format!(
                    "{} s since the first entry",
                    rules().match_start_wait_ms / 1_000
                )
            } else {
                format!("only {able} more can come")
            }
        );
        for &index in &chosen {
            self.stand_ins.push(StandIn {
                index,
                entered_at: at,
                left_at: None,
                since: now,
            });
        }
        for index in chosen {
            self.roster_change(Some(index), index, true, at, now);
        }
    }

    /// Each stand-in leaves once every game simulating played its entry;
    /// past [`STAND_IN_LEAVE_CAP`], a game that has not is resynced first,
    /// so that no game gets a leave it would drop.
    fn send_stand_in_leaves(&mut self, now: Instant) {
        if self.finished_at.is_some() {
            return;
        }
        let clock = self.clock(now);
        for which in 0..self.stand_ins.len() {
            let stand_in = self.stand_ins[which];
            if stand_in.left_at.is_some() || clock <= stand_in.entered_at {
                continue;
            }
            // A game reports the hash of a tick it played (its event
            // horizon's, behind its present).
            let behind: Vec<usize> = (0..self.peers.len())
                .filter(|&index| {
                    !self.is_bot(index)
                        && self.peers[index].as_ref().is_some_and(|peer| {
                            peer.stage == Stage::Live
                                && peer
                                    .reported_tick
                                    .is_none_or(|tick| tick < stand_in.entered_at)
                        })
                })
                .collect();
            if !behind.is_empty() {
                if now.saturating_duration_since(stand_in.since) < STAND_IN_LEAVE_CAP {
                    continue;
                }
                for index in behind {
                    self.resync_behind_stand_in(index, stand_in, now);
                }
            }
            let at = grid_up(clock + ROSTER_LEAD_MS);
            self.stand_ins[which].left_at = Some(at);
            self.roster_change(Some(stand_in.index), stand_in.index, false, at, now);
            log::info!(
                "Match {:#x}: place {} (a stand-in from {} ms) leaves at {at} ms",
                self.id,
                stand_in.index,
                stand_in.entered_at
            );
            // Its own game, loaded while the stand-in was in and held
            // (`stands_in`), and back in the match: it enters for the
            // others first (`enter_early`), after the leave.
            let index = stand_in.index;
            let held = self.peers[index].as_ref().is_some_and(|peer| {
                peer.resources_loaded
                    && peer.entered_early.is_none()
                    && matches!(peer.stage, Stage::WaitingSnapshot { entering: true, .. })
            });
            if held && self.returns(index) {
                self.enter_early(index, now);
            }
        }
    }

    /// A game that has not played a stand-in's entry [`STAND_IN_LEAVE_CAP`]
    /// after it (a stalled game: its window not active, its machine
    /// hanging) is started anew from a world of the others, as for a
    /// game out of step: the stand-in's leave would be dropped by it.
    fn resync_behind_stand_in(&mut self, index: usize, stand_in: StandIn, now: Instant) {
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        log::warn!(
            "Match {:#x}: player {index} has reported no tick past place {}'s stand-in entry at \
             {} ms in {} s (its last {:?}); resynced, so that it takes the stand-in's leave",
            self.id,
            stand_in.index,
            stand_in.entered_at,
            STAND_IN_LEAVE_CAP.as_secs(),
            peer.reported_tick
        );
        peer.last_resync = Some(now);
        peer.resyncs += 1;
        self.start_sync(index, false, now);
    }
}
