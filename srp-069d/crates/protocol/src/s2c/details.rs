//! 0x93 player details (reply to `38 00` on the results screen).
//!
//! Reader 0x20018B550 (block 0x20018EC40), PROVEN from the exe:
//!
//! 1. a statistics block, read by 0x182350 into a stack object (+0x330 of the 0x743C0-byte frame):
//!    `u64 id`, then (0x200182387 calls 0x200181C20 right after the id) 3 awards
//!    `{str64, u8, 9 x f32 [!= 0: u32, u16, u16]}` and 11 x 9 score components `{u32, u16, u16}` (906 bytes when zero,
//!    the same reader as the 97 human block), then `u32, u32, u16, u16, u32, u16, u8, 9 x u16, 9 x u8, u32 mask, 27 x { u8, u8, u8 }` (each triple is
//!    present only when its bit of `mask` is set, else it reads as zeros), `u32, u32, u32, u8`, then 75 x f32
//!    (9 x 3, 18, 9 x 3, 3), then `u8 n`, n x `{ u32, u16, u8, u8 k, k x u8 }`, then `u8, u16, u8, u8`;
//! 2. the account body (0x125460, the same body as in 87 / 88).
//!
//! Only the account body reaches the screen: it is passed to 0x1431C0, which builds Flash
//! `root.fill_victory_screen_player` from the body (keys name, level, state, item_slots, artifact_slots). The
//! statistics object is never handed on; after the body is read, 0x20018B627 only destroys its 18 vectors
//! (0x20018B609..0x20018B62C). So the client does not use any of the statistics values.
//!
//! The encoder therefore writes the statistics block with every value 0 (mask 0, empty lists): no statistic of the
//! player is claimed, and the account body is the real stored state.

use crate::items::AccountBody;
use crate::wire::{Decode, Encode, Reader, Result, Writer};

/// Message id of the player details push.
pub const ID: u8 = 0x93;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlayerDetails93 {
    /// Public account id of the player (`u64` at the start of the statistics block).
    pub id: u64,
    pub body: AccountBody,
}

impl Encode for PlayerDetails93 {
    fn encode(&self, w: &mut Writer) {
        w.u64(self.id);
        // 3 empty awards (str 0, u8 0, 9 x f32 0 = 38 bytes each) + 11 x 9 zero score components (8 bytes each)
        w.zeros(3 * 38 + 11 * 9 * 8);
        w.u32(0).u32(0).u16(0).u16(0).u32(0).u16(0).u8(0);
        w.zeros(9 * 2); // 9 x u16
        w.zeros(9); // 9 x u8
        w.u32(0); // mask: no { u8, u8, u8 } triple follows
        w.zeros(12 + 1); // 3 x u32, u8
        w.zeros(75 * 4); // 75 x f32
        w.u8(0); // n = 0
        w.u8(0).u16(0).u8(0).u8(0);
        self.body.encode(w);
    }
}

impl Decode for PlayerDetails93 {
    fn decode(r: &mut Reader) -> Result<Self> {
        let id = r.u64()?;
        for _ in 0..3 {
            crate::s2c::results::Award::decode(r)?;
        }
        r.skip(11 * 9 * 8)?;
        r.skip(4 + 4 + 2 + 2 + 4 + 2 + 1 + 18 + 9)?;
        let mask = r.u32()?;
        r.skip(3 * (mask & 0x7FF_FFFF).count_ones() as usize)?;
        r.skip(12 + 1 + 75 * 4)?;
        let n = r.u8()?;
        for _ in 0..n {
            r.skip(4 + 2 + 1)?;
            let k = r.u8()? as usize;
            r.skip(k)?;
        }
        r.skip(1 + 2 + 1 + 1)?;
        Ok(PlayerDetails93 { id, body: AccountBody::decode(r)? })
    }
}
