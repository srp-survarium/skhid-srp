//! S->C `82` connect_to_game_server: the lobby tells the client to join an online match (docs/notes/online.md).
//!
//! PROVEN layout (handler `lobby_client::on_message_received` case 0x82 at 0x20018E0FA, reads 0x20018E0FA..0x20018E164):
//! `str host` (`u8 len`, `FF u16 len` for 255 and more; read by 0x20011DE30 into a 0x40-byte buffer, so at most 63 bytes
//! are usable) -> `L+0x7A5294`, `u16 port` -> `+0x7A52D4`, `u32 match_id` -> `+0x7A52D8`, `u8 side` -> `+0x7A52DC` (and
//! `L+0x6B0D1C`), `u32 order_id` -> `L+0x6B0D18`. The handler then (0x20018DF30) stores match_id at `L+0x6B0D14`, sets the
//! queue state `L+0x6B0D08 = 2` and calls `network_client::connect_to_match(host, port, match_id, order_id)`
//! (0x200124CE0). `order_id` is what the client puts in its first match datagram, C9; `match_id` is only the "already in
//! this match" test and the log line; `side` fills the match-found UI record at `L+0x6B0A60 + 0xB0 / 0xB4` (side 0 ->
//! first word, side 1 -> second word, 0x20018DFFA..0x20018E014). Message class: unique (the udp-transport table).

use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

/// Longest host the client's buffer takes (0x40 bytes with the terminator).
pub const MAX_HOST: usize = 63;

// The match-type byte of the match options blob (+0x107D: the 4th flag byte after packed_inputs, single_player and
// b3; reader store 0x2001251C8). Only the server writes it (the client never does, not even for local matches).
/// Match type 0: nothing in the client reads it (PROVEN: every reader of +0x107D tests 1, 3, 4 or 6).
/// What every match but ranked and clan battles gets.
pub const MATCH_TYPE_PLAIN: u8 = 0;
/// Match type 1 = a normal queue (random) match: its only effect in the client is the inactivity logic (PROVEN, every
/// reader of +0x107D): the world's per-player inactive flag (stats block +0x9F0, set after 300 s without an activity
/// stamp) and the HUD's 3-minute warning / 5-minute texts. Sent for normal matches (CHOICE: AFK penalties applied to
/// normal matches, OFFICIAL 0.56b; the client tracks inactivity only for 1 and 3, and 3 also takes away the killcam
/// and the markers, which only ranked matches lose (`st_rating_match_descr`)).
pub const MATCH_TYPE_RANDOM: u8 = 1;
/// Match type 3 = ranked (LIKELY name; the 97 header's `b3 == 3` is the client's `rating_match`, PROVEN 0x200142749):
/// no killcam (0x2001E47CF), no hit / kill / protection markers (0x2001A546C, 0x20023830E, 0x2001A8FC2), and the
/// client's inactivity warning after 180 s / 300 s (0x2001AA150, `gameplay/fines/inactivity_fine.options`) plus its
/// per-player inactive flag; the client neither kicks nor charges (the fine is the server's: OPEN, no inactivity source).
pub const MATCH_TYPE_RANKED: u8 = 3;
/// Match type 6 = clan battle (LIKELY name): the same killcam / marker switches as 3, no inactivity code. Official
/// 0.65b: "Clan matches follow the same rules as ranked matches: there's no killcam and no markers".
pub const MATCH_TYPE_CLAN: u8 = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectToMatch {
    /// Host name or IP text of the match server.
    pub host: String,
    pub port: u16,
    pub match_id: u32,
    /// Team index of the player, 0 or 1 (PROVEN use: the match-found record; LIKELY meaning).
    pub side: u8,
    /// The per-player token the client answers with in C9.
    pub order_id: u32,
}

impl ConnectToMatch {
    /// As a message of the lobby connection.
    pub fn message(&self) -> Result<super::S2c> {
        Ok(super::S2c::Raw { id: crate::ids::s2c::CONNECT_TO_MATCH, payload: self.to_bytes()? })
    }
}

impl Encode for ConnectToMatch {
    fn encode(&self, w: &mut Writer) {
        if self.host.len() > MAX_HOST {
            w.fail(Error::Limit("82 host (63 bytes)"));
        }
        w.str(&self.host).u16(self.port).u32(self.match_id).u8(self.side).u32(self.order_id);
    }
}

impl Decode for ConnectToMatch {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(ConnectToMatch { host: r.str()?, port: r.u16()?, match_id: r.u32()?, side: r.u8()?, order_id: r.u32()? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_and_round_trip() {
        let m = ConnectToMatch { host: "127.0.0.1".into(), port: 1236, match_id: 0x1020_3040, side: 1, order_id: 0xAABB_CCDD };
        let b = m.to_bytes().unwrap();
        // len 9, text, port LE, match id LE, side, order id LE
        assert_eq!(&b[..10], &[9, b'1', b'2', b'7', b'.', b'0', b'.', b'0', b'.', b'1']);
        assert_eq!(&b[10..], &[0xD4, 0x04, 0x40, 0x30, 0x20, 0x10, 1, 0xDD, 0xCC, 0xBB, 0xAA]);
        assert_eq!(ConnectToMatch::from_bytes(&b).unwrap(), m);
        let s = m.message().unwrap();
        assert_eq!(s.id(), 0x82);
    }

    #[test]
    fn host_over_the_buffer_is_refused() {
        let m = ConnectToMatch { host: "h".repeat(64), port: 1, match_id: 1, side: 0, order_id: 1 };
        assert!(m.to_bytes().is_err());
        let ok = ConnectToMatch { host: "h".repeat(63), ..m };
        assert!(ok.to_bytes().is_ok());
    }
}
