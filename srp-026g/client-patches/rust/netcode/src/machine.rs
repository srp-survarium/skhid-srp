//! The per-tick protocol's client side, as a machine over parsed events
//! (no game memory here: `game.rs` reads it once and hands the parsed
//! values in). Host-tested.
//!
//! - [`Netcode::static_info`]: 0xE4's per-tick trailer (or none) decides
//!   the protocol of the match: [`Protocol::Stock`] or
//!   [`Protocol::Ticks`] on its grid.
//! - [`Netcode::own_input`], [`Netcode::own_profile`]: what the stock
//!   sender would have sent (0xDC/0xDD/0xDE), captured instead.
//! - [`Netcode::due`]: one update a tick since the last, each with the own
//!   inputs of its tick (0xE2).
//! - [`Netcode::update`]: the server's update of a tick (0xF7), made into
//!   the stock messages that do each of its changes ([`crate::stock`]):
//!   the outcomes of the own updates first (`sync_core::confirm`), the
//!   leaves (kept until the world is loaded: the stock 0xEA handler reads
//!   the match's view, which a game has only then), the entries, the
//!   others' inputs and kit choices; then the clock's steering
//!   (`sync_core::clock`, a 0xE7).

use sync_core::clock::{Order, Steering};
use sync_core::confirm::{Action, OwnInputs};
use sync_core::wire::{ClientUpdate, Input, Place, ServerUpdateView};
use sync_core::{GameTime, Grid};

use crate::stock::Stock;

/// The game's `time_floating_factor` with the client patch (0.3: patch 15,
/// `match_msgs::clock::FAST_FACTOR`), per mille.
pub const CLOCK_FACTOR_PERMILLE: u32 = 300;

/// Own inputs captured and not yet in an update, at most.
pub const MADE_KEPT: usize = 32;

/// Updates sent in one pass at most: a game that stood still catches up a
/// second, the rest of what it missed is empty.
pub const DUE_MAX: usize = 20;

/// The game as a message finds it (parsed by `game.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum World {
    /// No match core.
    NoCore,
    /// The core waits for its first world (+18FDFh).
    Frozen,
    /// The core runs: its time (+18FC0h), its last world's time (+18FD4h),
    /// its own place (+18FDCh; none for 20).
    Running {
        now: GameTime,
        resynced: GameTime,
        local: Option<Place>,
    },
}

/// What a captured own input is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Made {
    Input { time: GameTime, input: Input },
    Profile { time: GameTime, value: u8 },
}

impl Made {
    fn time(self) -> GameTime {
        match self {
            Made::Input { time, .. } | Made::Profile { time, .. } => time,
        }
    }
}

/// A match on the per-tick protocol.
#[derive(Clone, Copy, Debug)]
pub struct Session {
    pub grid: Grid,
    book: OwnInputs,
    steering: Steering,
    made: [Option<Made>; MADE_KEPT],
    /// The last tick sent an update of.
    sent_to: Option<GameTime>,
    /// Leaves taken before the world was loaded, by place: their time.
    leaves: [Option<GameTime>; sync_core::wire::MAX_PLACES],
}

/// The protocol of the game's match.
// The machine is a static, never moved, and there is no allocator to box
// the session with. Its tag first, `Stock` 0: the static starts all zeros
// (the arena's state block, not bytes in the file).
#[allow(clippy::large_enum_variant)]
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum Protocol {
    /// The stock 0.26g protocol: the patch does nothing.
    Stock,
    Ticks(Session),
}

/// The machine.
#[derive(Clone, Copy, Debug)]
pub struct Netcode {
    pub protocol: Protocol,
    /// Own inputs given up (the book or the capture full), for the log.
    pub lost: u32,
}

impl Default for Netcode {
    fn default() -> Self {
        Self::new()
    }
}

impl Netcode {
    pub const fn new() -> Netcode {
        Netcode {
            protocol: Protocol::Stock,
            lost: 0,
        }
    }

    /// 0xE4 came (the static info of a match): its trailer's grid, if it
    /// has one. A match without is of the stock protocol.
    pub fn static_info(&mut self, ticks: Option<Grid>) {
        self.protocol = match ticks {
            None => Protocol::Stock,
            Some(grid) => Protocol::Ticks(Session {
                grid,
                book: OwnInputs::new(),
                steering: Steering::new(),
                made: [None; MADE_KEPT],
                sent_to: None,
                leaves: [None; sync_core::wire::MAX_PLACES],
            }),
        };
    }

    /// The match's grid, with the per-tick protocol.
    pub fn grid(&self) -> Option<Grid> {
        match &self.protocol {
            Protocol::Stock => None,
            Protocol::Ticks(session) => Some(session.grid),
        }
    }

    /// The stock sender would send this own input (its node's time, the
    /// payload with its flag in bit 31): whether it is taken (and the
    /// stock send is skipped). The stock protocol takes nothing.
    pub fn own_input(&mut self, time: GameTime, rotation: [u8; 4], actions: u32) -> bool {
        let input = Input {
            back: 0,
            rotation,
            actions,
        };
        self.capture(Made::Input { time, input })
    }

    /// The same for a kit choice (0xDE's value).
    pub fn own_profile(&mut self, time: GameTime, value: u8) -> bool {
        self.capture(Made::Profile { time, value })
    }

    fn capture(&mut self, made: Made) -> bool {
        let Protocol::Ticks(session) = &mut self.protocol else {
            return false;
        };
        // A copy of one taken (the stock sender's own resends do not come
        // here; a node it sends again is the same input): kept once.
        if session.made.iter().flatten().any(|each| *each == made) {
            return true;
        }
        match session.made.iter_mut().find(|slot| slot.is_none()) {
            Some(slot) => *slot = Some(made),
            None => self.lost += 1,
        }
        true
    }

    /// The updates due by the world's time `now`: one a tick after the
    /// last, to `out` in order. The first pass sends none: it starts at the
    /// world's tick.
    pub fn due(&mut self, now: GameTime, out: &mut impl FnMut(&ClientUpdate)) -> usize {
        let Protocol::Ticks(session) = &mut self.protocol else {
            return 0;
        };
        let grid = session.grid;
        let tick_ms = grid.tick_ms();
        let floor = grid.floor(now);
        let Some(last) = session.sent_to else {
            session.sent_to = Some(floor);
            return 0;
        };
        if floor <= last {
            return 0;
        }
        // The empty ticks of a long stall are not sent: none of its inputs
        // is lost (a tick with one always goes).
        let oldest_empty = GameTime(floor.0.saturating_sub(DUE_MAX as u32 * tick_ms));
        let mut sent = 0;
        let mut tick = GameTime(last.0 + tick_ms);
        while tick <= floor {
            let low = GameTime(tick.0 - tick_ms);
            let in_tick = |made: &Made| made.time() > low && made.time() <= tick;
            let has = session.made.iter().flatten().any(in_tick);
            if has || tick >= oldest_empty {
                let mut update = ClientUpdate::empty(tick);
                for made in session.made.iter().flatten().filter(|made| in_tick(made)) {
                    let pushed = match *made {
                        Made::Input { time, input } => update.push_input(Input {
                            back: tick.since(time) as u8,
                            ..input
                        }),
                        Made::Profile { value, .. } => update.push_profile(value),
                    };
                    if pushed.is_err() {
                        self.lost += 1;
                    }
                }
                for slot in session.made.iter_mut() {
                    if slot.is_some_and(|made| made.time() <= tick) {
                        *slot = None;
                    }
                }
                if let sync_core::confirm::Sent::Evicted { .. } = session.book.sent(&update) {
                    self.lost += 1;
                }
                out(&update);
                sent += 1;
            }
            tick = GameTime(tick.0 + tick_ms);
        }
        // Captured for ticks already sent (none in the game's order): out.
        for slot in session.made.iter_mut() {
            if slot.is_some_and(|made| made.time() <= floor) {
                *slot = None;
                self.lost += 1;
            }
        }
        session.sent_to = Some(floor);
        sent
    }

    /// The server's update (0xF7, parsed against the match's grid) for the
    /// world `world`: the stock messages that make its changes, to `out` in
    /// order. Nothing with the stock protocol.
    pub fn update(&mut self, update: &ServerUpdateView<'_>, world: World, out: &mut impl FnMut(Stock)) {
        let Protocol::Ticks(session) = &mut self.protocol else {
            return;
        };
        let (now, resynced, local) = match world {
            World::Running {
                now,
                resynced,
                local,
            } => (Some(now), resynced, local),
            // Before a world: the lists keep what comes, as the stock
            // handlers do (0xE8 loads in mode 1).
            World::Frozen | World::NoCore => (None, GameTime(0), None),
        };
        let tick = update.tick;
        // What became of its own updates.
        for own in update.outcomes() {
            session.book.resolve(own, resynced, &mut |action| match action {
                Action::DiscardInput { time } => out(Stock::Discard { time }),
                Action::DiscardProfile { time } => out(Stock::DiscardProfile { time }),
                Action::MoveInput { from, to } => out(Stock::Move { from, to }),
                Action::MoveProfile { from, to } => out(Stock::MoveProfile { from, to }),
                Action::InsertInput { time, input } => {
                    if let Some(place) = local {
                        out(Stock::Relay { place, time, input });
                    }
                }
                Action::InsertProfile { time, value } => {
                    if let Some(place) = local {
                        out(Stock::Profile { place, time, value });
                    }
                }
            });
        }
        // Leaves: once the world is loaded; those kept before it first.
        for place in update.left.iter() {
            session.leaves[place.index()] = Some(tick);
        }
        if now.is_some() {
            for (index, slot) in session.leaves.iter_mut().enumerate() {
                if let (Some(time), Some(place)) = (slot.take(), Place::new(index as u8)) {
                    out(Stock::Left { place, time });
                }
            }
        }
        for place in update.entered.iter() {
            out(Stock::Entered { place, time: tick });
        }
        for each in update.inputs() {
            // Every game leaves out the same: an input no game makes.
            if Some(each.place) != local && each.input.is_sane() {
                out(Stock::Relay {
                    place: each.place,
                    time: each.input.time(tick),
                    input: Input {
                        back: 0,
                        ..each.input
                    },
                });
            }
        }
        for each in update.profiles() {
            if Some(each.place) != local {
                out(Stock::Profile {
                    place: each.place,
                    time: tick,
                    value: each.value,
                });
            }
        }
        // The clock, from how early its updates came.
        if let Some(now) = now {
            match session
                .steering
                .sample(update.arrival, now.0, CLOCK_FACTOR_PERMILLE)
            {
                Order::Keep => {}
                Order::Shift { ms } | Order::Jump { ms } => {
                    out(Stock::ClockOffset(ms.clamp(i16::MIN.into(), i16::MAX.into()) as i16));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;
    use sync_core::wire::{
        OwnOutcome, Outcome, PlaceInput, PlaceProfile, Places, SERVER_UPDATE_MAX, ServerUpdate,
    };

    const GRID: Grid = Grid::STOCK;

    fn t(ms: u32) -> GameTime {
        GameTime(ms)
    }

    fn place(index: u8) -> Place {
        Place::new(index).unwrap()
    }

    fn running(now: u32) -> World {
        World::Running {
            now: t(now),
            resynced: t(50),
            local: Some(place(1)),
        }
    }

    fn ticks() -> Netcode {
        let mut netcode = Netcode::new();
        netcode.static_info(Some(GRID));
        netcode
    }

    fn sent(netcode: &mut Netcode, now: u32) -> Vec<ClientUpdate> {
        let mut out = Vec::new();
        netcode.due(t(now), &mut |update| out.push(*update));
        out
    }

    struct Bytes(std::vec::Vec<u8>);

    fn server_update(update: ServerUpdate<'_>) -> Bytes {
        let mut bytes = std::vec![0; SERVER_UPDATE_MAX];
        let len = update.write(&mut bytes).unwrap();
        bytes.truncate(len);
        Bytes(bytes)
    }

    fn stocks(netcode: &mut Netcode, bytes: &Bytes, world: World) -> Vec<Stock> {
        let view = ServerUpdate::parse(&bytes.0, GRID).unwrap();
        let mut out = Vec::new();
        netcode.update(&view, world, &mut |stock| out.push(stock));
        out
    }

    fn input(actions: u32) -> Input {
        Input {
            back: 0,
            rotation: [0, 0x3C, 0, 0],
            actions,
        }
    }

    #[test]
    fn the_stock_protocol_takes_nothing_and_sends_nothing() {
        let mut netcode = Netcode::new();
        assert!(!netcode.own_input(t(1000), [0; 4], 1));
        assert!(sent(&mut netcode, 2000).is_empty());
        netcode.static_info(Some(GRID));
        netcode.static_info(None);
        assert!(!netcode.own_profile(t(1000), 1));
        assert_eq!(netcode.grid(), None);
    }

    #[test]
    fn an_update_a_tick_with_the_inputs_of_its_tick() {
        let mut netcode = ticks();
        assert!(sent(&mut netcode, 1010).is_empty(), "the first pass starts");
        assert!(netcode.own_input(t(1047), [1, 2, 3, 4], 0x21));
        assert!(netcode.own_input(t(1050), [5, 6, 7, 8], 0x20));
        assert!(netcode.own_profile(t(1100), 2));
        let updates = sent(&mut netcode, 1120);
        assert_eq!(
            updates.iter().map(|update| update.tick).collect::<Vec<_>>(),
            [t(1050), t(1100)]
        );
        assert_eq!(updates[0].inputs().len(), 2);
        assert_eq!(updates[0].inputs()[0].back, 3);
        assert_eq!(updates[1].profiles(), [2]);
        assert!(sent(&mut netcode, 1140).is_empty());
        assert_eq!(sent(&mut netcode, 1150).len(), 1, "empty, but sent");
        // A stall of 3 s: the empty ticks of its last second, a tick with
        // an input whatever its age.
        netcode.own_input(t(1200), [0; 4], 1);
        let updates = sent(&mut netcode, 4150);
        assert_eq!(updates[0].tick, t(1200));
        assert_eq!(updates.len(), 1 + DUE_MAX + 1);
        assert_eq!(netcode.lost, 0);
    }

    #[test]
    fn outcomes_move_and_discard_its_own_inputs_through_the_stock_messages() {
        let mut netcode = ticks();
        sent(&mut netcode, 1000);
        netcode.own_input(t(1047), [0; 4], 1);
        netcode.own_input(t(1100), [0; 4], 2);
        netcode.own_input(t(1150), [0; 4], 3);
        sent(&mut netcode, 1150);
        let outcomes = [
            OwnOutcome {
                update: t(1050),
                outcome: Outcome::Moved { to: t(1200) },
            },
            OwnOutcome {
                update: t(1100),
                outcome: Outcome::Discarded,
            },
            OwnOutcome {
                update: t(1150),
                outcome: Outcome::Confirmed,
            },
        ];
        let bytes = server_update(ServerUpdate {
            tick: t(1200),
            entered: Places::EMPTY,
            left: Places::EMPTY,
            arrival: None,
            inputs: &[],
            profiles: &[],
            outcomes: &outcomes,
        });
        assert_eq!(
            stocks(&mut netcode, &bytes, running(1300)),
            [
                Stock::Move {
                    from: t(1047),
                    to: t(1197)
                },
                Stock::Discard { time: t(1100) },
            ]
        );
    }

    #[test]
    fn the_others_changes_become_relays_entries_and_leaves_once_loaded() {
        let mut netcode = ticks();
        let inputs = [
            PlaceInput {
                place: place(0),
                input: Input { back: 5, ..input(7) },
            },
            // No game makes it: left out by every game.
            PlaceInput {
                place: place(2),
                input: input(1 << 30),
            },
        ];
        let profiles = [PlaceProfile {
            place: place(3),
            value: 1,
        }];
        let bytes = server_update(ServerUpdate {
            tick: t(1200),
            entered: Places::EMPTY.with(place(4)),
            left: Places::EMPTY.with(place(5)),
            arrival: None,
            inputs: &inputs,
            profiles: &profiles,
            outcomes: &[],
        });
        // Before its world: no leave yet.
        let before = stocks(&mut netcode, &bytes, World::Frozen);
        assert_eq!(
            before,
            [
                Stock::Entered {
                    place: place(4),
                    time: t(1200)
                },
                Stock::Relay {
                    place: place(0),
                    time: t(1195),
                    input: input(7)
                },
                Stock::Profile {
                    place: place(3),
                    time: t(1200),
                    value: 1
                },
            ]
        );
        // The next update after the load has it first.
        let empty = server_update(ServerUpdate {
            tick: t(1250),
            entered: Places::EMPTY,
            left: Places::EMPTY,
            arrival: None,
            inputs: &[],
            profiles: &[],
            outcomes: &[],
        });
        assert_eq!(
            stocks(&mut netcode, &empty, running(1300)),
            [Stock::Left {
                place: place(5),
                time: t(1200)
            }]
        );
        assert!(stocks(&mut netcode, &empty, running(1300)).is_empty());
        // Its own place's input (the server never sends one): not taken.
        let own = [PlaceInput {
            place: place(1),
            input: input(8),
        }];
        let bytes = server_update(ServerUpdate {
            tick: t(1300),
            entered: Places::EMPTY,
            left: Places::EMPTY,
            arrival: None,
            inputs: &own,
            profiles: &[],
            outcomes: &[],
        });
        assert!(stocks(&mut netcode, &bytes, running(1300)).is_empty());
    }

    #[test]
    fn late_updates_steer_the_clock_forward() {
        let mut netcode = ticks();
        let mut orders = Vec::new();
        for n in 0..20u32 {
            let bytes = server_update(ServerUpdate {
                tick: t(1000 + 50 * n),
                entered: Places::EMPTY,
                left: Places::EMPTY,
                arrival: Some(-60),
                inputs: &[],
                profiles: &[],
                outcomes: &[],
            });
            orders.extend(stocks(&mut netcode, &bytes, running(1000 + 50 * n)));
        }
        assert_eq!(orders, [Stock::ClockOffset(65)]);
    }
}
