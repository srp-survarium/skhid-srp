//! The per-tick protocol (`SRP_NETCODE=v069`; `docs/notes/
//! netcode_port_069.md` phase 2): 0.69's network layer around the 0.26g
//! simulation. The server is the clock of every input:
//!
//! - every client sends one update a tick, empty or not (0xE2,
//!   [`ClientUpdate`]): its own inputs of the tick, so the server knows
//!   when a place's inputs of a tick are complete;
//! - the server closes the ticks in order, one by one: a tick closes once
//!   the match clock has reached it and every place that sends has sent its
//!   update of it, or at [`CLOSE_DEADLINE_MS`] past it whatever came. A
//!   place whose updates stopped for that long holds nobody
//!   ([`Lane::flowing`]);
//! - an update lands where `sync_core::land` says: at its own tick while
//!   that is open, else at the first open one, or nowhere. Never in a
//!   closed tick, so every game gets every input of a tick at once, in the
//!   update that closes it;
//! - closing a tick, the server sends each client one update (0xF7,
//!   [`Update`]): the other places' inputs and kit choices at the tick, the
//!   roster changes at it, and the outcomes of the client's own updates
//!   decided since (confirmed, moved, discarded), with how early its
//!   updates came (for its clock, phase 3).
//!
//! This replaces the relays (0xEB/0xEC, 0xF3), the drop and move orders
//! (0xEF..0xF2, 0xF4, 0xF5) and the roster messages (0xE9, 0xEA): with the
//! protocol on, none of them is sent. Everything else is as for the stock
//! protocol, and the stages' rules carry over: an update goes to each
//! client as a relay went (`Game::route_update`), a client started from a
//! world gets the closed updates since it (`Game::closed_updates`) where it
//! got the relays, and an outcome is an answer (`Answer`, held while its
//! client is resynced).

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use survarium::v026g::match_msgs::{
    ClientUpdate, GameTime, Grid, OwnOutcome, Outcome, PlaceInput, PlaceProfile,
    PlayerInput, ServerMessage, Update, WireTime,
};
use sync_core::land::{self, Decision, Maker, Why};
use sync_core::wire::{Input, Place};

use super::{
    Answer, Flight, Game, GateKind, Relayed, Stage, answer_owner, gate, remember_answer, send_kept,
    send_until,
    send_waiting, takes_inputs, track, track_sent, usual_round_trip,
};

/// A tick closes this long past its time on the match clock at the
/// latest, whoever has not sent its update of it: the server's horizon of
/// the stock protocol, past which a late input was moved or dropped.
pub(super) const CLOSE_DEADLINE_MS: u32 = 250;

/// A place whose last update came longer ago than this holds no tick: its
/// game stands still (a stall, a window out of focus), and the others' go
/// on. When its updates come again they are late, and moved or dropped.
const STALLED_AFTER_MS: u64 = CLOSE_DEADLINE_MS as u64;

/// The way back and forth a client's own answer needs, on top of its round
/// trip, before its game's history passes the input it moves.
const REACH_MARGIN_MS: u32 = 50;

/// The match's per-tick state.
pub(crate) struct Ticks {
    pub grid: Grid,
    /// The last tick closed: everything up to it is final for every game.
    /// `None` until the match clock runs.
    closed: Option<GameTime>,
    /// By place.
    lanes: Vec<Lane>,
    /// Roster changes at ticks not closed yet.
    roster: Vec<RosterChange>,
}

/// One place's updates.
#[derive(Default)]
struct Lane {
    /// The ticks after the last closed whose update came.
    heard: BTreeSet<GameTime>,
    /// The newest update's tick, of the game at the place now, and when it
    /// came.
    made_to: Option<(GameTime, Instant)>,
    /// When its last update came.
    heard_at: Option<Instant>,
    landings: land::Lane,
    /// Updates landed at ticks still open.
    open: BTreeMap<GameTime, Landed>,
    /// The least `tick - clock` of its updates since its last 0xF7.
    arrival: Option<i16>,
}

/// An update's inputs, waiting for their tick to close.
struct Landed {
    update: GameTime,
    inputs: Vec<Input>,
    profiles: Vec<u8>,
    by: By,
}

/// Who made an update.
#[derive(Clone, Copy, PartialEq, Eq)]
enum By {
    /// The game at the place: it gets the outcome.
    Client,
    /// The server's own bot: nobody to tell.
    Server,
}

/// A roster change at a tick, and who does not get it.
#[derive(Clone, Copy, Debug)]
pub(super) struct RosterChange {
    pub at: GameTime,
    pub place: usize,
    pub entered: bool,
    pub except: Option<usize>,
}

impl Ticks {
    pub fn new(grid: Grid, places: usize) -> Self {
        Self {
            grid,
            closed: None,
            lanes: (0..places).map(|_| Lane::default()).collect(),
            roster: Vec::new(),
        }
    }

    /// The first tick still open.
    fn first_open(&self) -> Option<GameTime> {
        self.closed.map(|closed| GameTime(closed.0 + self.grid.tick_ms()))
    }

    /// A new game at `place` (it connected): its updates start anew.
    pub fn reset(&mut self, place: usize) {
        if let Some(lane) = self.lanes.get_mut(place) {
            lane.heard.clear();
            lane.made_to = None;
            lane.heard_at = None;
            lane.landings.forget();
            lane.arrival = None;
            // Its inputs already landed stay: the others are to get them.
        }
    }

    /// A roster change for `at`, on the first tick still open at or after
    /// it: the time the games will have it at.
    pub fn roster_at(&self, at: u32) -> u32 {
        let at = self.grid.ceil(GameTime(at));
        self.first_open().map_or(at, |open| at.max(open)).0
    }

    pub fn note_roster(&mut self, change: RosterChange) {
        self.roster.push(change);
    }
}

impl Lane {
    /// Whether its updates come: it sent one lately.
    fn flowing(&self, now: Instant) -> bool {
        self.heard_at.is_some_and(|at| {
            now.saturating_duration_since(at).as_millis() < u128::from(STALLED_AFTER_MS)
        })
    }
}

impl Game {
    /// The match's per-tick state, if it plays the per-tick protocol.
    pub(super) fn ticks(&self) -> Option<&Ticks> {
        self.per_tick.as_ref()
    }

    /// A client's update of a tick (0xE2).
    pub(super) fn client_update(&mut self, index: usize, update: ClientUpdate, now: Instant) {
        let waiting = self.waiting_entry(index);
        let clock = self.clock(now);
        let id = self.id;
        let Some(ticks) = self.per_tick.as_mut() else {
            return;
        };
        let grid = ticks.grid;
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        let tick = update.tick;
        // Sent until acked, with no transport dedup: a copy comes again.
        if !peer.updates_seen.insert(tick.0) {
            return;
        }
        if peer.updates_seen.len() > 1024 {
            let oldest = clock.saturating_sub(super::INPUT_MAX_AGE_MS);
            peer.updates_seen.retain(|seen| *seen >= oldest);
        }
        // Not of the match now: a game that has no place in the others'
        // worlds yet, or whose clock is far off the match's. Its inputs are
        // in its own world, so it hears they are in no other.
        let first_open = ticks.first_open().filter(|_| {
            !waiting
                && takes_inputs(peer)
                && tick.0 <= clock + super::TOO_NEW_MS
                && tick.0 + super::INPUT_MAX_AGE_MS >= clock
        });
        let Some(first_open) = first_open else {
            if update.has_inputs() {
                answer_owner(peer, outcome(tick, Outcome::Discarded), now);
            }
            return;
        };
        // Its game runs its send pass: it simulates.
        peer.simulating_at = Some(now);
        let round_trip = usual_round_trip(&peer.round_trips, peer.round_trip);
        let lane = &mut ticks.lanes[index];
        lane.heard_at = Some(now);
        if lane.made_to.is_none_or(|(made, _)| tick > made) {
            lane.made_to = Some((tick, now));
        }
        if tick >= first_open {
            lane.heard.insert(tick);
        }
        let early = (i64::from(tick.0) - i64::from(clock)).clamp(i16::MIN.into(), i16::MAX.into());
        lane.arrival = Some(lane.arrival.map_or(early as i16, |least| least.min(early as i16)));
        if !update.has_inputs() {
            return;
        }
        peer.inputs_received += 1;
        let late = clock.saturating_sub(tick.0);
        peer.latest_input_ms = peer.latest_input_ms.max(late);
        // An input no game makes (every game leaves such a one out too).
        let refused = update.inputs().iter().any(|input| !input.is_sane());
        let decision = if refused {
            peer.inputs_refused += 1;
            if peer.inputs_refused <= super::REFUSED_LOGGED {
                log::warn!(
                    "Match {id:#x}: player {index}'s update of {} ms has an input no client \
                     makes; discarded",
                    tick.0
                );
            }
            Decision::Discard(Why::Stale)
        } else {
            // Where its game's clock is when the answer comes, at least: its
            // newest update's tick, the time since that came, and a way
            // more (`land`).
            let reached = lane.made_to.map_or(tick, |(made, at)| {
                let since = now.saturating_duration_since(at).as_millis() as u32;
                GameTime(made.0 + since + round_trip / 2)
            });
            let maker = Maker::Client {
                reach_ms: land::reach_ms(round_trip, grid, REACH_MARGIN_MS),
                reached,
            };
            lane.landings.land(tick, first_open, maker)
        };
        let mut answers = Vec::new();
        match decision {
            Decision::Known => {}
            Decision::Discard(why) => {
                peer.inputs_discarded += 1;
                if peer.discarded.len() == super::MISSED_KEPT {
                    peer.discarded.pop_front();
                }
                peer.discarded.push_back(tick.0);
                log::debug!(
                    "Match {id:#x}: player {index}'s update of {} ms came at {clock} ms, the \
                     first open tick {} ms, its newest update {:?} ms: discarded ({why:?})",
                    tick.0,
                    first_open.0,
                    lane.made_to.map(|(made, _)| made.0)
                );
                answers.push(outcome(tick, Outcome::Discarded));
            }
            Decision::Land { at, supersedes } => {
                if let Some(earlier) = supersedes {
                    // Its earlier update of the same tick goes: one a tick.
                    lane.open.remove(&at);
                    answers.push(outcome(earlier, Outcome::Discarded));
                }
                if at != tick {
                    peer.inputs_moved += 1;
                    log::debug!(
                        "Match {id:#x}: player {index}'s update of {} ms came at {clock} ms: \
                         moved to {} ms",
                        tick.0,
                        at.0
                    );
                }
                lane.open.insert(
                    at,
                    Landed {
                        update: tick,
                        inputs: update.inputs().to_vec(),
                        profiles: update.profiles().to_vec(),
                        by: By::Client,
                    },
                );
            }
        }
        for answer in answers {
            answer_owner(peer, answer, now);
        }
    }

    /// The server's own bot at `index` makes `what` for `time` (as a client
    /// would send it in its update of the tick of `time`).
    pub(super) fn bot_inputs(&mut self, index: usize, made: Vec<(u32, Relayed)>) {
        let Some(ticks) = self.per_tick.as_mut() else {
            return;
        };
        let grid = ticks.grid;
        let Some(first_open) = ticks.first_open() else {
            return;
        };
        // One update a tick, as a game sends.
        let mut by_tick: BTreeMap<GameTime, Landed> = BTreeMap::new();
        for (time, what) in made {
            let tick = grid.ceil(GameTime(time));
            let landed = by_tick.entry(tick).or_insert_with(|| Landed {
                update: tick,
                inputs: Vec::new(),
                profiles: Vec::new(),
                by: By::Server,
            });
            match what {
                Relayed::Input(input) => landed.inputs.push(Input {
                    back: tick.since(GameTime(time)) as u8,
                    rotation: input.rotation,
                    actions: input.actions,
                }),
                Relayed::Profile { value, .. } => landed.profiles.push(value),
            }
        }
        let lane = &mut ticks.lanes[index];
        for (tick, landed) in by_tick {
            if let Decision::Land { at, .. } = lane.landings.land(tick, first_open, Maker::Server) {
                lane.open.insert(at, landed);
            }
        }
    }

    /// Closes every tick that is due (`poll`).
    pub(super) fn close_ticks(&mut self, now: Instant) {
        let Some((_, start)) = self.clock_start else {
            return;
        };
        let clock = self.clock(now);
        let waited: Vec<usize> = (0..self.peers.len())
            .filter(|&index| {
                !self.waiting_entry(index)
                    && self.peers[index]
                        .as_ref()
                        .is_some_and(takes_inputs)
            })
            .collect();
        let Some(ticks) = self.per_tick.as_mut() else {
            return;
        };
        let grid = ticks.grid;
        // The world the clock started on is closed: it holds its tick.
        let closed = *ticks.closed.get_or_insert(grid.floor(GameTime(start)));
        let mut due = Vec::new();
        let mut next = GameTime(closed.0 + grid.tick_ms());
        loop {
            // Never ahead of the match clock: a game whose updates have
            // not come yet (it has just loaded its world) is no one's to
            // close a tick before. The games steer their clocks ahead of
            // the match clock (phase 3): their updates are in by then.
            if next.0 > clock {
                break;
            }
            let deadline = next.0 + CLOSE_DEADLINE_MS <= clock;
            let heard = waited.iter().all(|&index| {
                let lane = &ticks.lanes[index];
                !lane.flowing(now) || lane.heard.contains(&next)
            });
            if !deadline && !heard {
                break;
            }
            due.push(next);
            next = GameTime(next.0 + grid.tick_ms());
        }
        for tick in due {
            self.close_tick(tick, now);
        }
    }

    /// Tick `tick` closes: what landed at it is final, and every client
    /// gets its update.
    fn close_tick(&mut self, tick: GameTime, now: Instant) {
        let Some(ticks) = self.per_tick.as_mut() else {
            return;
        };
        ticks.closed = Some(tick);
        let mut landed = Vec::new();
        for (index, lane) in ticks.lanes.iter_mut().enumerate() {
            lane.heard.retain(|heard| *heard > tick);
            if let Some(each) = lane.open.remove(&tick) {
                landed.push((index, each));
            }
        }
        let (roster, later): (Vec<_>, Vec<_>) =
            ticks.roster.drain(..).partition(|change| change.at <= tick);
        ticks.roster = later;
        // Final: the history the replays and the late joiners are made of,
        // and the owners' outcomes.
        for (index, each) in &landed {
            for input in &each.inputs {
                let at = input.time(tick).0;
                let relayed = PlayerInput {
                    time: WireTime::precise(at),
                    rotation: input.rotation,
                    actions: input.actions,
                };
                if each.by == By::Client {
                    self.observe_input(*index, at, &relayed);
                }
                self.inputs.push((at, *index, Relayed::Input(relayed)));
            }
            for value in &each.profiles {
                self.note_kit_choice(*index, tick.0);
                self.inputs.push((
                    tick.0,
                    *index,
                    Relayed::Profile {
                        tick: survarium::v026g::match_msgs::tick_byte(tick.0),
                        value: *value,
                    },
                ));
            }
            if each.by == By::Client
                && let Some(peer) = self.peers[*index].as_mut()
            {
                let result = if each.update == tick {
                    Outcome::Confirmed
                } else {
                    Outcome::Moved { to: tick }
                };
                answer_owner(peer, outcome(each.update, result), now);
            }
        }
        for index in 0..self.peers.len() {
            let Some((update, outcomes)) = self.update_for(index, tick, &landed, &roster) else {
                continue;
            };
            self.route_update(index, tick, update, outcomes, now);
        }
    }

    /// The update of `tick` for the client at `index`, with the answers
    /// whose outcomes it carries (taken from its outbox) and its arrival.
    fn update_for(
        &mut self,
        index: usize,
        tick: GameTime,
        landed: &[(usize, Landed)],
        roster: &[RosterChange],
    ) -> Option<(Update, Vec<Answer>)> {
        let peer = self.peers[index].as_mut()?;
        if peer.stage == Stage::Gone {
            return None;
        }
        let mut update = Update::empty(tick);
        for (place, each) in landed {
            let Some(place_id) = Place::new(*place as u8).filter(|_| *place != index) else {
                continue;
            };
            update
                .inputs
                .extend(each.inputs.iter().map(|input| PlaceInput {
                    place: place_id,
                    input: *input,
                }));
            update
                .profiles
                .extend(each.profiles.iter().map(|value| PlaceProfile {
                    place: place_id,
                    value: *value,
                }));
        }
        for change in roster {
            if change.except == Some(index) {
                continue;
            }
            let Some(place) = Place::new(change.place as u8) else {
                continue;
            };
            if change.entered {
                update.entered = update.entered.with(place);
            } else {
                update.left = update.left.with(place);
            }
        }
        // Its own outcomes, as many as one update holds.
        let going = peer.outbox.len().min(sync_core::wire::MAX_OUTCOMES);
        let sending: Vec<Answer> = peer.outbox.drain(..going).collect();
        update
            .outcomes
            .extend(sending.iter().filter_map(Answer::outcome_of));
        if let Some(ticks) = self.per_tick.as_mut() {
            update.arrival = ticks.lanes[index].arrival.take();
        }
        Some((update, sending))
    }

    /// Sends the update of `tick` to the client at `index` as a relay of
    /// that tick went to it, by its stage (`relay_to_others`,
    /// `roster_change`).
    fn route_update(
        &mut self,
        index: usize,
        tick: GameTime,
        update: Update,
        sending: Vec<Answer>,
        now: Instant,
    ) {
        let clock = self.clock(now);
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        let has_roster = !update.entered.is_empty() || !update.left.is_empty();
        let entries: Vec<usize> = update.entered.iter().map(Place::index).collect();
        let empty = update.is_empty();
        let message = ServerMessage::Update(update);
        let time = tick.0;
        let serial = match peer.stage {
            Stage::Live => {
                let round_trip = usual_round_trip(&peer.round_trips, peer.round_trip);
                let until = super::relay_deadline(round_trip, clock, time, now);
                let serial = send_until(peer, &message, until);
                if let Some(serial) = serial {
                    if has_roster {
                        track(peer, serial, Flight::Roster(message.clone()), time);
                    } else if !empty {
                        track_sent(peer, serial, Flight::Relay, time, Some(now));
                    }
                }
                serial
            }
            // A game with no world yet: what it gets goes with its world's
            // next flush, and its roster changes now, which it keeps
            // through the load (`entries_since`).
            Stage::Syncing {
                time: from,
                entering: true,
                ..
            } if time > from && !empty => {
                if has_roster && peer.time_answered {
                    let serial = send_waiting(peer, &message);
                    if let Some(serial) = serial {
                        for who in entries {
                            peer.early_entries.push((time, who, serial, true));
                        }
                    }
                    serial
                } else {
                    peer.held.push(message);
                    None
                }
            }
            Stage::Syncing { time: from, .. } if time > from && !empty => {
                let serial = send_kept(peer, &message);
                if let Some(serial) = serial
                    && has_roster
                    && peer.resync_from.is_some()
                {
                    track(peer, serial, Flight::Roster(message.clone()), time);
                    gate(peer, serial, now, time, GateKind::Roster);
                }
                serial
            }
            Stage::Timing {
                time: from,
                entering: false,
                ..
            } if time > from && peer.replayed.is_some() && !empty => {
                let serial = send_kept(peer, &message);
                if let Some(serial) = serial {
                    let kind = if has_roster {
                        GateKind::Roster
                    } else {
                        GateKind::Relay
                    };
                    gate(peer, serial, now, time, kind);
                }
                serial
            }
            _ => None,
        };
        // Its outcomes went with it: sent answers, until acked.
        match serial {
            Some(serial) => {
                for mut answer in sending {
                    answer.serial = Some(serial);
                    remember_answer(peer, answer);
                }
            }
            // Not sent (held, or a stage that gets no updates): they wait
            // for its next one.
            None => {
                let waiting = std::mem::take(&mut peer.outbox);
                peer.outbox = sending;
                peer.outbox.extend(waiting);
            }
        }
    }

    /// The closed updates after `from` for the client at `index`, as its
    /// replay or its sync's: the others' inputs (from the history) and the
    /// roster changes `roster` (the ones it is to get), one update a tick
    /// that has any. With whether each has a roster change.
    pub(super) fn closed_updates(
        &self,
        index: usize,
        from: u32,
        roster: &[(u32, usize, bool)],
    ) -> Vec<(u32, ServerMessage, bool)> {
        let Some(ticks) = self.ticks() else {
            return Vec::new();
        };
        let grid = ticks.grid;
        let closed = ticks.closed.map_or(0, |closed| closed.0);
        let mut updates: BTreeMap<GameTime, Update> = BTreeMap::new();
        for (at, place, what) in &self.inputs {
            if *at <= from || *at > closed || *place == index {
                continue;
            }
            let Some(place) = Place::new(*place as u8) else {
                continue;
            };
            let tick = grid.ceil(GameTime(*at));
            let update = updates
                .entry(tick)
                .or_insert_with(|| Update::empty(tick));
            match what {
                Relayed::Input(input) => update.inputs.push(PlaceInput {
                    place,
                    input: Input {
                        back: tick.since(GameTime(*at)) as u8,
                        rotation: input.rotation,
                        actions: input.actions,
                    },
                }),
                Relayed::Profile { value, .. } => update.profiles.push(PlaceProfile {
                    place,
                    value: *value,
                }),
            }
        }
        let mut with_roster = BTreeSet::new();
        for &(at, place, entered) in roster {
            if at <= from || at > closed {
                continue;
            }
            let Some(place) = Place::new(place as u8) else {
                continue;
            };
            let tick = grid.ceil(GameTime(at));
            let update = updates
                .entry(tick)
                .or_insert_with(|| Update::empty(tick));
            if entered {
                update.entered = update.entered.with(place);
            } else {
                update.left = update.left.with(place);
            }
            with_roster.insert(tick);
        }
        updates
            .into_iter()
            .map(|(tick, update)| {
                (
                    tick.0,
                    ServerMessage::Update(update),
                    with_roster.contains(&tick),
                )
            })
            .collect()
    }

    /// `send_replay` of the per-tick protocol: the closed updates since the
    /// snapshot at `time`, each kept until acked and waited for by the
    /// resync's world; the ones still open go as they close
    /// (`route_update`).
    pub(super) fn send_tick_replay(&mut self, index: usize, time: u32, now: Instant) {
        let roster = self.roster_events_since(index, time);
        let replay = self.closed_updates(index, time, &roster);
        let id = self.id;
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        peer.replayed = Some((time, now));
        super::clear_gate(peer, None, now);
        for (at, message, has_roster) in &replay {
            if let Some(serial) = send_kept(peer, message) {
                if *has_roster {
                    track(peer, serial, Flight::Roster(message.clone()), *at);
                    gate(peer, serial, now, *at, GateKind::Roster);
                } else {
                    gate(peer, serial, now, *at, GateKind::Replay);
                }
            }
        }
        log::debug!(
            "Match {id:#x}: player {index} took its time; sent {} updates since the snapshot at \
             {time} ms",
            replay.len()
        );
    }

    /// The newest tick of its own the game at `index` made an update of:
    /// a resync's clock goes past it (`sync_start`).
    pub(super) fn own_made_to(&self, index: usize) -> Option<u32> {
        self.ticks()?.lanes.get(index)?.made_to.map(|(made, _)| made.0)
    }
}

fn outcome(update: GameTime, result: Outcome) -> Answer {
    Answer::outcome(OwnOutcome {
        update,
        outcome: result,
    })
}
