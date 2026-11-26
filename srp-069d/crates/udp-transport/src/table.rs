//! The 0.69d lobby message classes (PROVEN from the client's table at 0x200180460 + byte table
//! 0x2001804D0).
//!
//! * unique:  `id x payload`, x = unique id (flags 0x20)
//! * ordered: `id x payload`, x = order id (flags 0x60)
//! * plain:   `id payload`, no x byte, sent once
//!
//! Every lobby id is variable length (0x200180590 returns 0xFFFF for all of them). Unique and ordered
//! ids count separately; plain ids use neither. Resend interval of unique/ordered: 1000 ms, lifetime
//! infinite; plain: lifetime 0.

use std::time::Duration;

/// Which way a datagram travels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    ToServer,
    ToClient,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    Unique,
    Ordered,
    Plain,
}

impl Class {
    /// The id carries an x byte.
    pub fn has_x(self) -> bool {
        self != Class::Plain
    }

    /// Resend interval (client table: 1000 ms for unique and ordered; plain is not resent).
    pub fn resend(self) -> Option<Duration> {
        match self {
            Class::Plain => None,
            _ => Some(Duration::from_millis(1000)),
        }
    }
}

/// C->S ids the client's table accepts (inclusive): 0x2E..=0x62.
pub const C2S_RANGE: (u8, u8) = (0x2E, 0x62);
/// S->C ids the client's table accepts (inclusive): 0x80..=0xC3.
pub const S2C_RANGE: (u8, u8) = (0x80, 0xC3);

/// The class of `id` in the given direction, `None` when the client's table rejects the id
/// ("invalid message type": the whole datagram is dropped).
pub fn class_of(direction: Direction, id: u8) -> Option<Class> {
    match direction {
        Direction::ToServer => match id {
            0x2E..=0x35 | 0x37..=0x3B | 0x45..=0x60 | 0x62 => Some(Class::Unique),
            0x36 | 0x3C..=0x44 | 0x61 => Some(Class::Plain),
            _ => None,
        },
        Direction::ToClient => match id {
            0x80..=0x82 | 0x9A | 0x9E | 0xAD..=0xAF | 0xB1 | 0xB7 | 0xB9..=0xBF | 0xC1 | 0xC2 => {
                Some(Class::Unique)
            }
            0xA2 | 0xA3 | 0xAC | 0xB4 | 0xC0 | 0xC3 => Some(Class::Plain),
            0x83..=0xC3 => Some(Class::Ordered),
            _ => None,
        },
    }
}

/// Low-level message ids (one byte, no payload): FA..=FD (0xFC = keepalive in the logs). 0xFE and
/// anything above except the multipart prefix 0xFF is "invalid message type".
pub mod low_level {
    pub const INITIATE_DISCONNECTION: u8 = 0xFA;
    pub const CONFIRM_DISCONNECTION: u8 = 0xFB;
    pub const CONTINUOUS_FLOW: u8 = 0xFC;
    pub const INSTANT_DISCONNECT: u8 = 0xFD;

    pub fn is_low_level(t: u8) -> bool {
        (0xFA..=0xFD).contains(&t)
    }
}

/// The multipart prefix: `FF id x part [count]`.
pub const MULTIPART: u8 = 0xFF;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s2c_table_matches_the_client_table() {
        let unique = [
            0x80, 0x81, 0x82, 0x9A, 0x9E, 0xAD, 0xAE, 0xAF, 0xB1, 0xB7, 0xB9, 0xBA, 0xBB, 0xBC,
            0xBD, 0xBE, 0xBF, 0xC1, 0xC2,
        ];
        let plain = [0xA2, 0xA3, 0xAC, 0xB4, 0xC0, 0xC3];
        for id in S2C_RANGE.0..=S2C_RANGE.1 {
            let c = class_of(Direction::ToClient, id).unwrap();
            let want = if unique.contains(&id) {
                Class::Unique
            } else if plain.contains(&id) {
                Class::Plain
            } else {
                Class::Ordered
            };
            assert_eq!(c, want, "id {id:#x}");
        }
        assert_eq!(class_of(Direction::ToClient, 0x7F), None);
        assert_eq!(class_of(Direction::ToClient, 0xC4), None);
        // ids whose class is easy to get wrong: 9E unique, AC plain, B0 ordered, 83 ordered
        assert_eq!(class_of(Direction::ToClient, 0x9E), Some(Class::Unique));
        assert_eq!(class_of(Direction::ToClient, 0xAC), Some(Class::Plain));
        assert_eq!(class_of(Direction::ToClient, 0xB0), Some(Class::Ordered));
        assert_eq!(class_of(Direction::ToClient, 0x83), Some(Class::Ordered));
    }

    #[test]
    fn c2s_table() {
        for id in C2S_RANGE.0..=C2S_RANGE.1 {
            assert!(class_of(Direction::ToServer, id).is_some());
        }
        for id in [0x2E, 0x32, 0x4F, 0x50, 0x53, 0x5B] {
            assert_eq!(class_of(Direction::ToServer, id), Some(Class::Unique));
        }
        for id in [0x36, 0x3C, 0x44, 0x61] {
            assert_eq!(class_of(Direction::ToServer, id), Some(Class::Plain));
        }
        assert_eq!(class_of(Direction::ToServer, 0x2D), None);
        assert_eq!(class_of(Direction::ToServer, 0x63), None);
    }
}
