//! A client's own updates, from the send until the server says what became
//! of them (the client side of `land`; `netcode_port_069.md` 8.2, the
//! `OwnInputs` machine). The client predicts: its own inputs are in its
//! world at their own times from the moment it makes them. Each update with
//! inputs then gets exactly one [`Outcome`] from the server, and the client
//! makes its lists what every other game has: nothing for a confirmed one,
//! the inputs moved for a moved one, taken out for a discarded one. The
//! server decides; this only follows, so every game applies the same inputs
//! at the same ticks.
//!
//! Each pending update is **in place** (its nodes at its own times in the
//! game's lists) or **lifted** (taken out of them). An update is lifted when
//! an earlier one is moved onto its tick: the server never lands two of a
//! place's updates on one tick, so the one of that tick will be moved
//! later or discarded, and must not hold the tick meanwhile. A lifted
//! update moved is put back by the game's network `change_input`, as
//! `move_input` itself does (0x27D2D0 -> 0x1FAB50).
//!
//! The game refuses to move or discard a node at or before the time of the
//! last world it loaded ("older than last resync", core +18FD4h): that node
//! is not played any more, and the loaded world has what the server
//! decided. So an update up to that time counts as lifted.

use crate::time::GameTime;
use crate::wire::{ClientUpdate, Input, OwnOutcome, Outcome};

/// The own updates a client keeps waiting for their outcome.
pub const PENDING_KEPT: usize = 64;

/// One call of the game's own functions for the client's own place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// `discard_input<player_input>` (0x27D820) at `time`.
    DiscardInput { time: GameTime },
    /// `discard_input<profile_input>` (0x27E200) at `time`.
    DiscardProfile { time: GameTime },
    /// `move_input<player_input>` (0x27D2D0).
    MoveInput { from: GameTime, to: GameTime },
    /// `move_input<profile_input>` (0x27DCD0).
    MoveProfile { from: GameTime, to: GameTime },
    /// The network `change_input<player_input>` (0x1FAB50) for its own
    /// place.
    InsertInput { time: GameTime, input: Input },
    /// The network `change_input<profile_input>` (0x1FB730) for its own
    /// place.
    InsertProfile { time: GameTime, value: u8 },
}

/// Where a pending update's inputs are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spot {
    InPlace,
    Lifted,
}

#[derive(Clone, Copy, Debug)]
struct Pending {
    update: ClientUpdate,
    spot: Spot,
}

/// What [`OwnInputs::sent`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sent {
    Kept,
    /// The book was full: the oldest pending update was given up (its
    /// outcome will find nothing).
    Evicted { oldest: GameTime },
    /// Nothing to wait for: no inputs.
    Nothing,
}

/// What [`OwnInputs::resolve`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolved {
    Done,
    /// No pending update of that tick (a copy, or one given up).
    Unknown,
}

/// The client's book of its own pending updates.
#[derive(Clone, Copy, Debug)]
pub struct OwnInputs {
    pending: [Option<Pending>; PENDING_KEPT],
}

impl Default for OwnInputs {
    fn default() -> Self {
        Self::new()
    }
}

impl OwnInputs {
    pub const fn new() -> OwnInputs {
        OwnInputs {
            pending: [None; PENDING_KEPT],
        }
    }

    /// How many wait.
    pub fn len(&self) -> usize {
        self.pending.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn find(&self, update: GameTime) -> Option<usize> {
        self.pending
            .iter()
            .position(|each| each.is_some_and(|each| each.update.tick == update))
    }

    /// An update went out: one with inputs waits for its outcome.
    pub fn sent(&mut self, update: &ClientUpdate) -> Sent {
        if !update.has_inputs() {
            return Sent::Nothing;
        }
        let entry = Pending {
            update: *update,
            spot: Spot::InPlace,
        };
        if let Some(free) = self.pending.iter().position(Option::is_none) {
            self.pending[free] = Some(entry);
            return Sent::Kept;
        }
        let mut oldest = 0;
        for (index, each) in self.pending.iter().enumerate() {
            if let (Some(each), Some(old)) = (each, &self.pending[oldest])
                && each.update.tick < old.update.tick
            {
                oldest = index;
            }
        }
        let given_up = self.pending[oldest].map_or(GameTime(0), |each| each.update.tick);
        self.pending[oldest] = Some(entry);
        Sent::Evicted { oldest: given_up }
    }

    /// The outcome `own` came; the game's last world is of `resynced`. The
    /// calls to make go to `act`, in order. Total: an outcome for nothing
    /// pending does nothing.
    pub fn resolve(
        &mut self,
        own: OwnOutcome,
        resynced: GameTime,
        act: &mut impl FnMut(Action),
    ) -> Resolved {
        let Some(index) = self.find(own.update) else {
            return Resolved::Unknown;
        };
        let Some(pending) = self.pending[index].take() else {
            return Resolved::Unknown;
        };
        let from = pending.update.tick;
        let in_place = pending.spot == Spot::InPlace && from > resynced;
        match own.outcome {
            Outcome::Confirmed => {}
            Outcome::Discarded => {
                if in_place {
                    take_out(&pending.update, act);
                }
            }
            Outcome::Moved { to } => {
                // The own update of `to` will not be at `to`: out of the way.
                if let Some(at) = self.find(to)
                    && let Some(there) = &mut self.pending[at]
                    && there.spot == Spot::InPlace
                {
                    if to > resynced {
                        take_out(&there.update, act);
                    }
                    there.spot = Spot::Lifted;
                }
                if to > resynced {
                    for input in pending.update.inputs() {
                        act(if in_place {
                            Action::MoveInput {
                                from: input.time(from),
                                to: input.time(to),
                            }
                        } else {
                            Action::InsertInput {
                                time: input.time(to),
                                input: *input,
                            }
                        });
                    }
                    for value in pending.update.profiles() {
                        act(if in_place {
                            Action::MoveProfile { from, to }
                        } else {
                            Action::InsertProfile {
                                time: to,
                                value: *value,
                            }
                        });
                    }
                }
                // Else moved into the loaded world (and so from before
                // it): the world has it there.
            }
        }
        Resolved::Done
    }
}

/// The update's nodes out of the lists.
fn take_out(update: &ClientUpdate, act: &mut impl FnMut(Action)) {
    for input in update.inputs() {
        act(Action::DiscardInput {
            time: input.time(update.tick),
        });
    }
    for _ in update.profiles() {
        act(Action::DiscardProfile { time: update.tick });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    fn t(ms: u32) -> GameTime {
        GameTime(ms)
    }

    fn update(tick: u32, backs: &[u8]) -> ClientUpdate {
        let mut update = ClientUpdate::empty(t(tick));
        for &back in backs {
            update
                .push_input(Input {
                    back,
                    rotation: [0; 4],
                    actions: back.into(),
                })
                .unwrap();
        }
        update
    }

    fn resolve(book: &mut OwnInputs, tick: u32, outcome: Outcome, resynced: u32) -> Vec<Action> {
        let mut actions = Vec::new();
        book.resolve(
            OwnOutcome {
                update: t(tick),
                outcome,
            },
            t(resynced),
            &mut |action| actions.push(action),
        );
        actions
    }

    #[test]
    fn confirmed_does_nothing_discarded_takes_out_moved_moves() {
        let mut book = OwnInputs::new();
        for tick in [1000, 1050, 1100] {
            assert_eq!(book.sent(&update(tick, &[0, 20])), Sent::Kept);
        }
        assert_eq!(book.sent(&update(1150, &[])), Sent::Nothing);
        assert_eq!(book.len(), 3);
        assert!(resolve(&mut book, 1000, Outcome::Confirmed, 0).is_empty());
        assert_eq!(
            resolve(&mut book, 1050, Outcome::Discarded, 0),
            [
                Action::DiscardInput { time: t(1050) },
                Action::DiscardInput { time: t(1030) }
            ]
        );
        assert_eq!(
            resolve(&mut book, 1100, Outcome::Moved { to: t(1250) }, 0),
            [
                Action::MoveInput {
                    from: t(1100),
                    to: t(1250)
                },
                Action::MoveInput {
                    from: t(1080),
                    to: t(1230)
                }
            ]
        );
        assert!(book.is_empty());
        // A copy of an outcome: nothing.
        assert!(resolve(&mut book, 1100, Outcome::Discarded, 0).is_empty());
    }

    #[test]
    fn a_move_onto_a_pending_tick_lifts_that_update_and_its_own_move_puts_it_back() {
        let mut book = OwnInputs::new();
        book.sent(&update(1000, &[0]));
        book.sent(&update(1100, &[0]));
        // 1000 moved onto 1100: 1100 goes out of the way first.
        assert_eq!(
            resolve(&mut book, 1000, Outcome::Moved { to: t(1100) }, 0),
            [
                Action::DiscardInput { time: t(1100) },
                Action::MoveInput {
                    from: t(1000),
                    to: t(1100)
                }
            ]
        );
        // 1100 moved on later: put back from its payload.
        assert_eq!(
            resolve(&mut book, 1100, Outcome::Moved { to: t(1200) }, 0),
            [Action::InsertInput {
                time: t(1200),
                input: Input {
                    back: 0,
                    rotation: [0; 4],
                    actions: 0
                }
            }]
        );
        // Or discarded: it is out already.
        book.sent(&update(1300, &[0]));
        book.sent(&update(1400, &[0]));
        resolve(&mut book, 1300, Outcome::Moved { to: t(1400) }, 0);
        assert!(resolve(&mut book, 1400, Outcome::Discarded, 0).is_empty());
    }

    #[test]
    fn nodes_up_to_the_loaded_world_are_never_touched() {
        let mut book = OwnInputs::new();
        book.sent(&update(1000, &[0]));
        book.sent(&update(1050, &[0]));
        book.sent(&update(1100, &[0]));
        // The world loaded is of 1050.
        assert!(resolve(&mut book, 1000, Outcome::Discarded, 1050).is_empty());
        assert_eq!(
            resolve(&mut book, 1050, Outcome::Moved { to: t(1150) }, 1050),
            [Action::InsertInput {
                time: t(1150),
                input: Input::default()
            }]
        );
        // Moved into the world: it is there already; out of its old tick.
        assert_eq!(
            resolve(&mut book, 1100, Outcome::Moved { to: t(1150) }, 1200),
            []
        );
    }

    #[test]
    fn a_full_book_gives_up_its_oldest() {
        let mut book = OwnInputs::new();
        for n in 0..PENDING_KEPT as u32 {
            assert_eq!(book.sent(&update(1000 + 50 * n, &[0])), Sent::Kept);
        }
        assert_eq!(
            book.sent(&update(9000, &[0])),
            Sent::Evicted { oldest: t(1000) }
        );
        assert!(resolve(&mut book, 1000, Outcome::Discarded, 0).is_empty());
        assert_eq!(book.len(), PENDING_KEPT);
    }
}
