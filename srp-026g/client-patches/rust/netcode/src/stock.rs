//! The stock match messages the per-tick handler hands the game's own
//! dispatch (`network_client::on_match_packet_received` 0x27A300): every
//! change an update makes goes through the handler the stock protocol had
//! for it, live-proven since the first release (match.md section 1.2):
//!
//! | change | id | handler | the game's function |
//! |---|---|---|---|
//! | another place's input | 0xEB, 0xEC precise | 0x27C280 | `change_input<player_input>` 0x1FAB50 |
//! | another place's kit choice | 0xF3 | 0x27C980 | `change_input<profile_input>` 0x1FB730 |
//! | a place enters / leaves | 0xE9 / 0xEA | 0x27C5F0 / 0x27C690 | `enter_match` 0x2D1470 / `leave_match` 0x2D18A0 |
//! | an own input moved | 0xEF, 0xF0 precise | 0x27C7B0 | `move_input<player_input>` 0x27D2D0 |
//! | an own input discarded | 0xF1, 0xF2 precise | 0x27C8D0 | `discard_input<player_input>` 0x27D820 |
//! | an own kit choice moved / discarded | 0xF4 / 0xF5 | 0x27CA30 / 0x27CB50 | 0x27DCD0 / 0x27E200 |
//! | an own input put back (lifted) | 0xEB, 0xEC with its own place | 0x27C280 | 0x1FAB50, as `move_input` does |
//! | the clock steered | 0xE7 | 0x27B7F0 | `floating_timer::set_offset` 0x1EE9E0 |
//!
//! A time goes as the stock server sent it: its tick on the match's grid,
//! low 8 bits, which the handler reads against the game's own clock (the
//! reader 0x318416, ±127 ticks), and the ms into it for the precise ids.

use sync_core::wire::{Input, Place};
use sync_core::{GameTime, Grid};

/// A stock message, by what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stock {
    /// A place's input at `time` (0xEB / 0xEC).
    Relay { place: Place, time: GameTime, input: Input },
    /// A place's kit choice at the tick of `time` (0xF3).
    Profile { place: Place, time: GameTime, value: u8 },
    /// A place enters at `time` (0xE9).
    Entered { place: Place, time: GameTime },
    /// A place leaves at `time` (0xEA).
    Left { place: Place, time: GameTime },
    /// The own input at `from` goes to `to` (0xEF / 0xF0).
    Move { from: GameTime, to: GameTime },
    /// The own input at `time` goes (0xF1 / 0xF2).
    Discard { time: GameTime },
    /// The own kit choice at `from` goes to `to` (0xF4).
    MoveProfile { from: GameTime, to: GameTime },
    /// The own kit choice at `time` goes (0xF5).
    DiscardProfile { time: GameTime },
    /// The clock corrected by so many ms (0xE7).
    ClockOffset(i16),
}

/// A stock message's bytes: its id, and its payload's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Encoded {
    pub id: u8,
    pub payload: [u8; 11],
    pub len: usize,
}

impl Encoded {
    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.len]
    }
}

struct Out {
    bytes: [u8; 11],
    len: usize,
}

impl Out {
    fn new() -> Out {
        Out {
            bytes: [0; 11],
            len: 0,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        for byte in bytes {
            // The longest message here is 11 bytes (0xEC).
            if let Some(slot) = self.bytes.get_mut(self.len) {
                *slot = *byte;
                self.len += 1;
            }
        }
    }

    fn done(self, id: u8) -> Encoded {
        Encoded {
            id,
            payload: self.bytes,
            len: self.len,
        }
    }
}

/// A time's tick byte on the grid (`serialize_local_fixed_time`).
fn tick(grid: Grid, time: GameTime) -> u8 {
    grid.ticks(time) as u8
}

/// The ms of a time into its tick (`serialize_local_time`).
fn ms(grid: Grid, time: GameTime) -> u8 {
    grid.into_tick(time) as u8
}

impl Stock {
    /// Its bytes for a game on `grid`.
    pub fn encode(self, grid: Grid) -> Encoded {
        let mut out = Out::new();
        match self {
            Stock::Relay { place, time, input } => {
                let precise = !grid.on_grid(time);
                out.push(&[tick(grid, time)]);
                if precise {
                    out.push(&[ms(grid, time)]);
                }
                out.push(&[place.get()]);
                out.push(&input.rotation);
                out.push(&input.actions.to_le_bytes());
                out.done(if precise { 0xEC } else { 0xEB })
            }
            Stock::Profile { place, time, value } => {
                out.push(&[tick(grid, time), place.get(), value]);
                out.done(0xF3)
            }
            Stock::Entered { place, time } => {
                out.push(&[tick(grid, time), place.get()]);
                out.done(0xE9)
            }
            Stock::Left { place, time } => {
                out.push(&[tick(grid, time), place.get()]);
                out.done(0xEA)
            }
            Stock::Move { from, to } => {
                let precise = !grid.on_grid(from) || !grid.on_grid(to);
                for time in [from, to] {
                    out.push(&[tick(grid, time)]);
                    if precise {
                        out.push(&[ms(grid, time)]);
                    }
                }
                out.done(if precise { 0xF0 } else { 0xEF })
            }
            Stock::Discard { time } => {
                let precise = !grid.on_grid(time);
                out.push(&[tick(grid, time)]);
                if precise {
                    out.push(&[ms(grid, time)]);
                }
                out.done(if precise { 0xF2 } else { 0xF1 })
            }
            Stock::MoveProfile { from, to } => {
                out.push(&[tick(grid, from), tick(grid, to)]);
                out.done(0xF4)
            }
            Stock::DiscardProfile { time } => {
                out.push(&[tick(grid, time)]);
                out.done(0xF5)
            }
            Stock::ClockOffset(ms) => {
                out.push(&ms.to_le_bytes());
                out.done(0xE7)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sync_core::TickLen;

    const GRID: Grid = Grid::STOCK;

    fn place(index: u8) -> Place {
        Place::new(index).unwrap()
    }

    fn input() -> Input {
        Input {
            back: 0,
            rotation: [1, 2, 3, 4],
            actions: 0x8000_0021,
        }
    }

    /// The layouts of match.md section 1.2 (the server's `ServerMessage`).
    #[test]
    fn each_change_is_the_stock_message_of_the_stock_server() {
        let at = |ms| GameTime(ms);
        let cases: [(Stock, u8, &[u8]); 12] = [
            (
                Stock::Relay { place: place(3), time: at(539_650), input: input() },
                0xEB,
                &[0x29, 3, 1, 2, 3, 4, 0x21, 0, 0, 0x80],
            ),
            (
                Stock::Relay { place: place(3), time: at(539_657), input: input() },
                0xEC,
                &[0x29, 7, 3, 1, 2, 3, 4, 0x21, 0, 0, 0x80],
            ),
            (Stock::Profile { place: place(2), time: at(1050), value: 1 }, 0xF3, &[21, 2, 1]),
            (Stock::Entered { place: place(5), time: at(400) }, 0xE9, &[8, 5]),
            (Stock::Left { place: place(5), time: at(2000) }, 0xEA, &[40, 5]),
            (Stock::Move { from: at(1000), to: at(1100) }, 0xEF, &[20, 22]),
            (Stock::Move { from: at(997), to: at(1097) }, 0xF0, &[19, 47, 21, 47]),
            (Stock::Move { from: at(1000), to: at(1097) }, 0xF0, &[20, 0, 21, 47]),
            (Stock::Discard { time: at(1000) }, 0xF1, &[20]),
            (Stock::Discard { time: at(1003) }, 0xF2, &[20, 3]),
            (Stock::MoveProfile { from: at(1000), to: at(1100) }, 0xF4, &[20, 22]),
            (Stock::ClockOffset(-300), 0xE7, &(-300i16).to_le_bytes()),
        ];
        for (stock, id, payload) in cases {
            let encoded = stock.encode(GRID);
            assert_eq!((encoded.id, encoded.payload()), (id, payload), "{stock:?}");
        }
        assert_eq!(Stock::DiscardProfile { time: at(1000) }.encode(GRID).payload(), [20]);
    }

    #[test]
    fn ticks_are_of_the_matchs_grid() {
        let grid = Grid::of(TickLen::Ms20);
        let encoded = Stock::Relay { place: place(0), time: GameTime(1007), input: input() }.encode(grid);
        assert_eq!((encoded.id, &encoded.payload()[..2]), (0xEC, &[50, 7][..]));
        let encoded = Stock::Discard { time: GameTime(1000) }.encode(grid);
        assert_eq!((encoded.id, encoded.payload()), (0xF1, &[50][..]));
    }
}
