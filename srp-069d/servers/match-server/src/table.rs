//! The 0.69d MATCH connection message table (PROVEN from the client's table object at `mc+0xA4DA08`, vtable
//! 0x20096F180).
//!
//! `udp_transport::table` only knows the lobby table (S->C 0x80..0xC3, C->S 0x2E..0x62). The match connection
//! uses the same transport but its own table: **S->C 0xDC..=0xE5, C->S 0xC9..=0xD4** (range dword 0xD4C9E5DC).
//! This module is that table: class (unique / ordered / plain), length rule, lifetime and resend interval.
//!
//! "cfg" = a value from the match options: resend = `send_period` (u16 ms, first field of `match_options`),
//! lifetime = 2 x that (0x2001F3864). The caller passes `send_period`.

use std::time::Duration;
pub use udp_transport::{Class, Direction};

pub mod id {
    // C->S
    pub const C9_CONNECT: u8 = 0xC9;
    pub const CA_PORT_PROBE: u8 = 0xCA;
    pub const CB_READY: u8 = 0xCB;
    pub const CC_QUERY_DE: u8 = 0xCC;
    pub const CD_CLIENT_UPDATE: u8 = 0xCD;
    pub const CE_LEAVE: u8 = 0xCE;
    pub const CF_NET_STATS: u8 = 0xCF;
    pub const D0_SETTINGS: u8 = 0xD0;
    pub const D1_KEEPALIVE: u8 = 0xD1;
    pub const D2_BATTLEYE: u8 = 0xD2;
    pub const D3_VOICE: u8 = 0xD3;
    pub const D4_INTEGRITY: u8 = 0xD4;
    // S->C
    pub const DC_PORTS: u8 = 0xDC;
    pub const DD_STATIC_INFO: u8 = 0xDD;
    pub const DE_WORLD: u8 = 0xDE;
    pub const DF_GAME_UPDATE: u8 = 0xDF;
    pub const E0_HASH: u8 = 0xE0;
    pub const E1_UNHANDLED: u8 = 0xE1;
    pub const E2_TIME: u8 = 0xE2;
    pub const E3_BATTLEYE: u8 = 0xE3;
    pub const E4_KICK: u8 = 0xE4;
    pub const E5_VOICE: u8 = 0xE5;
}

/// C->S ids the client's table accepts (inclusive).
pub const C2S_RANGE: (u8, u8) = (0xC9, 0xD4);
/// S->C ids the client's table accepts (inclusive).
pub const S2C_RANGE: (u8, u8) = (0xDC, 0xE5);

/// How long the length of a message is known without a prefix (flag 0x80 "fixed length" in the client's info).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Len {
    /// Fixed number of payload bytes (after id and x): no length prefix in a multi datagram.
    Fixed(usize),
    /// `u8 len` or `FF u16 len` prefix in a multi datagram; the rest of a single datagram.
    Var,
    /// E4 only: "fixed by content", `u8 kind, u8 n, n bytes` = `n + 2` bytes.
    E4,
}

/// How long a message is kept for resending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Life {
    /// Send once, never resend (client info lifetime 0).
    Once,
    /// Resend until acked or until this long after the first send.
    Until(Duration),
    /// Resend until acked (lifetime -1).
    Forever,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    pub class: Class,
    pub len: Len,
    pub life: Life,
    pub resend: Duration,
}

/// Class and length rule of an id, `None` when the client's table rejects it ("invalid message type").
pub fn spec(direction: Direction, id: u8) -> Option<(Class, Len)> {
    use Class::{Ordered, Plain, Unique};
    Some(match direction {
        Direction::ToServer => match id {
            0xC9 => (Plain, Len::Fixed(10)),  // PROVEN: connect, 4 + 4 + 1 + 1 bytes
            0xCA => (Plain, Len::Fixed(4)),   // PROVEN
            0xCB => (Plain, Len::Var),        // PROVEN
            0xCC => (Plain, Len::Fixed(6)),   // PROVEN
            0xCD => (Plain, Len::Var),        // PROVEN
            0xCE => (Plain, Len::Fixed(0)),   // PROVEN
            // PROVEN table value 10; but the writer was read as ONE f32 (4 bytes): unresolved.
            0xCF => (Unique, Len::Fixed(10)),
            0xD0 => (Ordered, Len::Var),      // PROVEN
            0xD1 => (Plain, Len::Fixed(0)),   // PROVEN table, no sender in the client
            0xD2 => (Unique, Len::Var),       // PROVEN
            0xD3 => (Plain, Len::Var),        // PROVEN
            0xD4 => (Plain, Len::Fixed(4)),   // PROVEN
            _ => return None,
        },
        Direction::ToClient => match id {
            0xDC => (Plain, Len::Fixed(4)),   // PROVEN
            0xDD => (Unique, Len::Var),       // PROVEN
            0xDE => (Unique, Len::Var),       // PROVEN
            0xDF => (Plain, Len::Var),        // PROVEN
            0xE0 => (Plain, Len::Var),        // PROVEN
            0xE1 => (Unique, Len::Var),       // PROVEN (client has no handler)
            0xE2 => (Ordered, Len::Fixed(8)), // PROVEN
            0xE3 => (Unique, Len::Var),       // PROVEN
            0xE4 => (Unique, Len::E4),        // PROVEN
            0xE5 => (Plain, Len::Var),        // PROVEN
            _ => return None,
        },
    })
}

/// Full info including lifetime and resend interval. `send_period` is the `cfg` value of the match options.
pub fn info(direction: Direction, id: u8, send_period: Duration) -> Option<Info> {
    let (class, len) = spec(direction, id)?;
    let cfg_life = Life::Until(send_period * 2);
    let ms = Duration::from_millis;
    let (life, resend) = match direction {
        Direction::ToServer => match id {
            0xC9 | 0xCA | 0xCB | 0xCE | 0xD0 => (Life::Forever, ms(500)),
            0xCC | 0xD1 | 0xD3 | 0xD4 => (Life::Once, ms(0)),
            0xCD | 0xCF => (cfg_life, send_period),
            0xD2 => (Life::Until(ms(2000)), ms(250)),
            _ => unreachable!(),
        },
        Direction::ToClient => match id {
            0xDC | 0xDD | 0xE1 | 0xE2 | 0xE4 => (Life::Forever, ms(500)),
            0xDE => (Life::Until(ms(1900)), ms(100)),
            0xDF => (cfg_life, send_period),
            0xE0 => (Life::Until(ms(40)), send_period),
            0xE3 => (Life::Until(ms(2000)), ms(250)),
            0xE5 => (Life::Once, ms(0)),
            _ => unreachable!(),
        },
    };
    Some(Info { class, len, life, resend })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_cover_exactly_the_documented_ids() {
        for id in 0..=255u8 {
            assert_eq!(
                spec(Direction::ToServer, id).is_some(),
                (C2S_RANGE.0..=C2S_RANGE.1).contains(&id),
                "c2s {id:#x}"
            );
            assert_eq!(
                spec(Direction::ToClient, id).is_some(),
                (S2C_RANGE.0..=S2C_RANGE.1).contains(&id),
                "s2c {id:#x}"
            );
        }
    }

    #[test]
    fn classes_as_in_the_client_table() {
        let sp = Duration::from_millis(100);
        // S->C
        assert_eq!(spec(Direction::ToClient, 0xDC), Some((Class::Plain, Len::Fixed(4))));
        assert_eq!(spec(Direction::ToClient, 0xDE).unwrap().0, Class::Unique);
        assert_eq!(spec(Direction::ToClient, 0xDF).unwrap().0, Class::Plain);
        assert_eq!(spec(Direction::ToClient, 0xE2), Some((Class::Ordered, Len::Fixed(8))));
        assert_eq!(spec(Direction::ToClient, 0xE4), Some((Class::Unique, Len::E4)));
        assert_eq!(info(Direction::ToClient, 0xDE, sp).unwrap().life, Life::Until(Duration::from_millis(1900)));
        assert_eq!(info(Direction::ToClient, 0xDE, sp).unwrap().resend, Duration::from_millis(100));
        assert_eq!(info(Direction::ToClient, 0xDF, sp).unwrap().life, Life::Until(Duration::from_millis(200)));
        assert_eq!(info(Direction::ToClient, 0xE5, sp).unwrap().life, Life::Once);
        // C->S
        assert_eq!(spec(Direction::ToServer, 0xC9), Some((Class::Plain, Len::Fixed(10))));
        assert_eq!(spec(Direction::ToServer, 0xCC), Some((Class::Plain, Len::Fixed(6))));
        assert_eq!(spec(Direction::ToServer, 0xD0).unwrap().0, Class::Ordered);
        assert_eq!(spec(Direction::ToServer, 0xCF).unwrap().0, Class::Unique);
        assert_eq!(info(Direction::ToServer, 0xCC, sp).unwrap().life, Life::Once);
        assert_eq!(info(Direction::ToServer, 0xD2, sp).unwrap().resend, Duration::from_millis(250));
    }
}
