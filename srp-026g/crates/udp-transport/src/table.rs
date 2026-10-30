//! The per-id message tables of the two connection kinds (transport.md §4
//! for matches, §4.1 for the lobby). The client asks its table object for
//! every message: the payload length (fixed, or variable = length-prefixed
//! in a multi datagram), the info flags (unique, ordered) and, for what it
//! sends, the lifetime and resend time.
//!
//! - **Match** (client vtable 0xA08414): server ids 0xE3..=0xF6, client ids
//!   0xD8..=0xE1. Only E4, E8 and F6 are variable; six server ids are
//!   unique; nothing is ordered; ED/EE have a length that depends on their
//!   content and the roster size.
//! - **Lobby** (client vtable 0xA07FA8): server ids 0x80..=0xAA, client ids
//!   0x2E..=0x50. Every id is variable. Every client id is unique. Server
//!   ids are unique, or unique and ordered (flags 0x60: the x byte is then
//!   the order id), except 0x9F which is plain and sent once.
//!
//! The entries are the client's own values, read from its tables (addresses
//! in the note). `info` answers for both directions, so the same table
//! drives the server's parser, its sender and the test model of the client.

use std::time::Duration;

/// Which connection a transport serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Table {
    Match,
    Lobby,
}

/// Who sends the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Server to client (the client's "received" ids).
    ToClient,
    /// Client to server (the client's "sending" ids).
    ToServer,
}

/// How a message is delivered (the info flags, transport.md §3.1, §3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// No x byte. The receiver does not dedup: a resent copy is delivered
    /// again.
    Plain,
    /// Flags 0x20: the x byte is a unique id; the receiver drops ids it has
    /// in its 128-wide window.
    Unique,
    /// Flags 0x60: the x byte is an order id on the ordered channel (its own
    /// counter); the receiver delivers in order id order, each exactly once.
    Ordered,
}

/// The payload length of a message, as `get_message_length` gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Length {
    /// Exactly this many bytes; no length prefix anywhere.
    Fixed(usize),
    /// Variable: the rest of a single datagram, length-prefixed in a multi
    /// one. The reader needs at least `first_min` bytes left for a whole
    /// message or part 0 and `part_min` for a later part (E4, F6: 1 and 1;
    /// E8: 3 and 0; client 0x27970E, 0x2799C1).
    Variable { first_min: usize, part_min: usize },
    /// Match ED/EE: `head` bytes, a player mask of `B` bytes, then 8 bytes
    /// per set bit; at least two bits set (client 0x27975B). `B` comes from
    /// the roster size. No length prefix.
    PlayerInputs { head: usize },
}

/// How long the sender keeps resending (the info's `lifetime_ms`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifetime {
    /// 0: sent once, never tracked.
    Once,
    /// Resent until acked or until this long after the first send, then
    /// dropped silently.
    For(Duration),
    /// 0xFFFFFFFF: resent until acked.
    Forever,
}

/// One table entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageInfo {
    pub length: Length,
    pub class: Class,
    pub lifetime: Lifetime,
    /// How soon an unacked copy goes again (fixed, no jitter).
    pub resend: Duration,
}

/// What the table says about an id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lookup {
    Info(MessageInfo),
    /// Outside the id range: the client's parser fails and drops the whole
    /// datagram ("invalid message type", 0x3FA412).
    OutOfRange,
    /// Inside the range, but the client's jump table entry is 0: asking for
    /// its info jumps to address 0 (lobby 0x86 and 0xA7, reached from the
    /// parser at 0x3FA5E4). Never put it on the wire. (Match E2 is outside
    /// both ranges.)
    Null,
}

const fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

const fn fixed(len: usize, lifetime: Lifetime, resend: u64) -> MessageInfo {
    MessageInfo {
        length: Length::Fixed(len),
        class: Class::Plain,
        lifetime,
        resend: ms(resend),
    }
}

const fn short(len: usize) -> MessageInfo {
    fixed(len, Lifetime::For(ms(250)), 50)
}

const fn unique_once(len: usize) -> MessageInfo {
    MessageInfo {
        length: Length::Fixed(len),
        class: Class::Unique,
        lifetime: Lifetime::Once,
        resend: ms(0),
    }
}

const fn unique_variable(first_min: usize, part_min: usize, lifetime: Lifetime) -> MessageInfo {
    MessageInfo {
        length: Length::Variable {
            first_min,
            part_min,
        },
        class: Class::Unique,
        lifetime,
        resend: ms(500),
    }
}

const fn lobby(class: Class) -> MessageInfo {
    MessageInfo {
        length: Length::Variable {
            first_min: 0,
            part_min: 0,
        },
        class,
        lifetime: Lifetime::Forever,
        resend: ms(1000),
    }
}

impl Table {
    /// The id range of a direction (the table object's bytes +4..+7).
    pub fn range(self, direction: Direction) -> (u8, u8) {
        match (self, direction) {
            (Table::Match, Direction::ToClient) => (0xE3, 0xF6),
            (Table::Match, Direction::ToServer) => (0xD8, 0xE1),
            (Table::Lobby, Direction::ToClient) => (0x80, 0xAA),
            (Table::Lobby, Direction::ToServer) => (0x2E, 0x50),
        }
    }

    pub fn lookup(self, direction: Direction, id: u8) -> Lookup {
        let (low, high) = self.range(direction);
        if id < low || id > high {
            return Lookup::OutOfRange;
        }
        let info = match (self, direction) {
            (Table::Match, Direction::ToServer) => match_to_server(id),
            (Table::Match, Direction::ToClient) => match_to_client(id),
            (Table::Lobby, Direction::ToServer) => Some(lobby(Class::Unique)),
            (Table::Lobby, Direction::ToClient) => lobby_to_client(id),
        };
        info.map_or(Lookup::Null, Lookup::Info)
    }

    /// The entry of an id, if it has one.
    pub fn info(self, direction: Direction, id: u8) -> Option<MessageInfo> {
        match self.lookup(direction, id) {
            Lookup::Info(info) => Some(info),
            Lookup::OutOfRange | Lookup::Null => None,
        }
    }

    /// Ids that have an entry but must still never be sent to a client:
    /// lobby 0x81 and 0xA0 (PLAN-026G step 3: they crash the client or make
    /// it drop the datagram at the game layer).
    pub fn is_forbidden(self, id: u8) -> bool {
        self == Table::Lobby && matches!(id, 0x81 | 0xA0)
    }

    /// The idle time of the client on this connection (conn+0x148): it
    /// sends a keep-alive `FC` when it has nothing else for this long.
    /// Match 100 ms (0x27A120), lobby 500 ms (0x26F0E8).
    pub fn client_idle(self) -> Duration {
        match self {
            Table::Match => ms(100),
            Table::Lobby => ms(500),
        }
    }
}

/// Client → server, match (table 0x279AE0, lengths 0x2799E0).
fn match_to_server(id: u8) -> Option<MessageInfo> {
    use Lifetime::{Forever, Once};
    Some(match id {
        0xD8 => fixed(5, Forever, 500),
        0xD9 => fixed(4, Forever, 500),
        0xDA => fixed(0, Forever, 500),
        0xDB => fixed(6, Once, 0),
        0xDC => fixed(9, Lifetime::For(ms(500)), 50),
        0xDD => fixed(10, Lifetime::For(ms(500)), 50),
        0xDE => fixed(2, Forever, 50),
        0xDF => fixed(5, Once, 0),
        0xE0 => fixed(0, Forever, 500),
        0xE1 => fixed(0, Once, 0),
        _ => return None,
    })
}

/// Server → client, match (table 0x279BF0, lengths 0x2799E0).
fn match_to_client(id: u8) -> Option<MessageInfo> {
    use Lifetime::{Forever, Once};
    let inputs = |head| MessageInfo {
        length: Length::PlayerInputs { head },
        class: Class::Plain,
        lifetime: Once,
        resend: ms(0),
    };
    Some(match id {
        0xE3 => fixed(4, Forever, 500),
        0xE4 => unique_variable(1, 1, Forever),
        0xE5 => unique_once(7),
        0xE6 => unique_once(4),
        0xE7 => unique_once(2),
        0xE8 => unique_variable(3, 0, Lifetime::For(ms(1750))),
        0xE9 | 0xEA => short(2),
        0xEB => short(10),
        0xEC => short(11),
        0xED => inputs(1),
        0xEE => inputs(2),
        0xEF => short(2),
        0xF0 => short(4),
        0xF1 => short(1),
        0xF2 => short(2),
        0xF3 => fixed(3, Forever, 50),
        0xF4 => fixed(2, Forever, 50),
        0xF5 => fixed(1, Forever, 50),
        0xF6 => unique_variable(1, 1, Forever),
        _ => return None,
    })
}

/// Server → client, lobby (class bytes 0x26E2F0, jump table 0x26E2D8).
fn lobby_to_client(id: u8) -> Option<MessageInfo> {
    Some(match id {
        0x80 | 0x81 | 0x82 | 0x9A | 0xA0 | 0xA1 | 0xA2 | 0xA4 | 0xAA => lobby(Class::Unique),
        0x86 | 0xA7 => return None,
        0x9F => MessageInfo {
            length: Length::Variable {
                first_min: 0,
                part_min: 0,
            },
            class: Class::Plain,
            lifetime: Lifetime::Once,
            resend: ms(0),
        },
        0x83..=0xA9 => lobby(Class::Ordered),
        _ => return None,
    })
}

/// The mask bytes of ED/EE for a roster: `(players - 1) / 8 + 1` (client
/// 0x279610). Zero players gives 0, the client's value before it knows the
/// roster.
pub fn mask_bytes(players: u8) -> usize {
    if players == 0 {
        0
    } else {
        (usize::from(players) - 1) / 8 + 1
    }
}

/// The length of an ED/EE payload from its own bytes, as the client works it
/// out (0x27975B..0x2799A2): `available` bytes are left in the message area.
/// `None` where the client reports an error.
pub fn player_inputs_len(head: usize, mask_bytes: usize, available: &[u8]) -> Option<usize> {
    if available.len() < head + mask_bytes + 16 {
        return None;
    }
    let players: usize = available[head..head + mask_bytes]
        .iter()
        .map(|byte| byte.count_ones() as usize)
        .sum();
    let len = head + mask_bytes + 8 * players;
    (available.len() >= len).then_some(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_match_tables_match_the_note() {
        let to_client = |id| Table::Match.info(Direction::ToClient, id).unwrap();
        assert_eq!(
            Table::Match.lookup(Direction::ToClient, 0xE2),
            Lookup::OutOfRange
        );
        assert_eq!(
            Table::Match.lookup(Direction::ToClient, 0xF7),
            Lookup::OutOfRange
        );
        assert_eq!(to_client(0xE3).length, Length::Fixed(4));
        assert_eq!(to_client(0xE3).class, Class::Plain);
        for id in [0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xF6] {
            assert_eq!(to_client(id).class, Class::Unique, "{id:#04x}");
        }
        for id in [0xE4, 0xE8, 0xF6] {
            assert!(matches!(to_client(id).length, Length::Variable { .. }));
        }
        let lengths = [
            (0xE5, 7),
            (0xE6, 4),
            (0xE7, 2),
            (0xE9, 2),
            (0xEA, 2),
            (0xEB, 10),
            (0xEC, 11),
            (0xEF, 2),
            (0xF0, 4),
            (0xF1, 1),
            (0xF2, 2),
            (0xF3, 3),
            (0xF4, 2),
            (0xF5, 1),
        ];
        for (id, len) in lengths {
            assert_eq!(to_client(id).length, Length::Fixed(len), "{id:#04x}");
        }
        assert_eq!(to_client(0xE8).lifetime, Lifetime::For(ms(1750)));
        assert_eq!(to_client(0xEB).lifetime, Lifetime::For(ms(250)));
        assert_eq!(to_client(0xF3).resend, ms(50));
        assert_eq!(to_client(0xED).length, Length::PlayerInputs { head: 1 });
        assert_eq!(to_client(0xEE).length, Length::PlayerInputs { head: 2 });

        let to_server = |id| Table::Match.info(Direction::ToServer, id).unwrap();
        let lengths = [
            (0xD8, 5),
            (0xD9, 4),
            (0xDA, 0),
            (0xDB, 6),
            (0xDC, 9),
            (0xDD, 10),
            (0xDE, 2),
            (0xDF, 5),
            (0xE0, 0),
            (0xE1, 0),
        ];
        for (id, len) in lengths {
            assert_eq!(to_server(id).length, Length::Fixed(len), "{id:#04x}");
            assert_eq!(to_server(id).class, Class::Plain);
        }
        assert_eq!(
            Table::Match.lookup(Direction::ToServer, 0xE2),
            Lookup::OutOfRange
        );
    }

    #[test]
    fn the_lobby_tables_match_the_note() {
        for id in 0x2E..=0x50 {
            let info = Table::Lobby.info(Direction::ToServer, id).unwrap();
            assert_eq!(info.class, Class::Unique);
            assert_eq!(info.resend, ms(1000));
        }
        assert_eq!(
            Table::Lobby.lookup(Direction::ToServer, 0x51),
            Lookup::OutOfRange
        );
        assert_eq!(
            Table::Lobby.lookup(Direction::ToClient, 0x7F),
            Lookup::OutOfRange
        );
        assert_eq!(
            Table::Lobby.lookup(Direction::ToClient, 0xAB),
            Lookup::OutOfRange
        );
        assert_eq!(Table::Lobby.lookup(Direction::ToClient, 0x86), Lookup::Null);
        assert_eq!(Table::Lobby.lookup(Direction::ToClient, 0xA7), Lookup::Null);
        let class = |id| Table::Lobby.info(Direction::ToClient, id).unwrap().class;
        for id in [0x80, 0x81, 0x82, 0x9A, 0xA0, 0xA1, 0xA2, 0xA4, 0xAA] {
            assert_eq!(class(id), Class::Unique, "{id:#04x}");
        }
        for id in (0x83..=0x85).chain(0x87..=0x99).chain(0x9B..=0x9E) {
            assert_eq!(class(id), Class::Ordered, "{id:#04x}");
        }
        for id in [0xA3, 0xA5, 0xA6, 0xA8, 0xA9] {
            assert_eq!(class(id), Class::Ordered, "{id:#04x}");
        }
        assert_eq!(class(0x9F), Class::Plain);
        assert!(Table::Lobby.is_forbidden(0x81) && Table::Lobby.is_forbidden(0xA0));
        assert!(!Table::Match.is_forbidden(0xE3));
    }

    #[test]
    fn the_player_inputs_length_counts_the_mask() {
        assert_eq!(mask_bytes(0), 0);
        assert_eq!(mask_bytes(8), 1);
        assert_eq!(mask_bytes(9), 2);
        assert_eq!(mask_bytes(20), 3);
        // ED, 20 players, players 0 and 9: 1 + 3 + 16.
        let mut payload = vec![7, 0b1, 0b10, 0];
        payload.extend([0; 16]);
        assert_eq!(player_inputs_len(1, 3, &payload), Some(20));
        // One player only: the client wants room for two.
        assert_eq!(player_inputs_len(1, 3, &payload[..12]), None);
        // Three bits set but data for two.
        payload[3] = 1;
        assert_eq!(player_inputs_len(1, 3, &payload), None);
    }
}
