//! Item and preset pushes: 86, 90, BB (docs/notes/items.md).

use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

/// 0x86 matchmaking tip (inline 0x20018E340 -> `set_playing_order_tip(u8 kind, u32 arg)` 0x2001CB2C0, log
/// "arrived playing order tips : %u(%s) %u"). PROVEN. This is NOT an item-use message (an older note guessed so):
/// `kind` indexes the table at 0x2009674F0: 0 `st_game_servers_more_select`, 1 `st_game_modes_more_select`,
/// 2 `st_squad_elo_warning`, 3 `change_squad_or_modes_mask`; `arg` is a number for the text. Kinds above 3 read
/// past the table, so the encoder refuses them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayingOrderTip {
    pub kind: u8,
    pub arg: u32,
}

impl Encode for PlayingOrderTip {
    fn encode(&self, w: &mut Writer) {
        if self.kind > 3 {
            w.fail(Error::Invalid("86: kind must be 0..=3"));
        }
        w.u8(self.kind).u32(self.arg);
    }
}

impl Decode for PlayingOrderTip {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(PlayingOrderTip { kind: r.u8()?, arg: r.u32()? })
    }
}

/// 0x90 inventory counts (inline 0x20018E3C5). PROVEN: `u8 n, n x { u32 item uid, u8 amount }`. For each entry
/// the client looks the uid up in its inventory; when found it subtracts `amount` from the count at record +0xC
/// (`count - amount`, a result of 0 or less becomes 0); a count of 0 erases the record (0x197A90 + 0x249B20),
/// otherwise the UI row is refreshed (0x248E20). Unknown uids are skipped. An empty list is legal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemCounts90 {
    pub entries: Vec<(u32, u8)>,
}

impl Encode for ItemCounts90 {
    fn encode(&self, w: &mut Writer) {
        w.count_u8(self.entries.len(), "90 entries");
        for &(uid, amount) in &self.entries {
            w.u32(uid).u8(amount);
        }
    }
}

impl Decode for ItemCounts90 {
    fn decode(r: &mut Reader) -> Result<Self> {
        let n = r.u8()? as usize;
        r.check_count(n, 5)?;
        Ok(ItemCounts90 { entries: (0..n).map(|_| Ok((r.u32()?, r.u8()?))).collect::<Result<_>>()? })
    }
}

/// One preset of the BB list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetEntry {
    /// Preset id, sent back by the 58 requests (+0x79FE2C).
    pub id: u32,
    /// Stored at +0x79FE30 and never read by the client; the server sends 0.
    pub flag: u8,
    /// Name, at most 127 bytes (`str128`, +0x79FE31).
    pub name: String,
}

/// 0xBB equipment presets (reader 0x200195CA0). PROVEN: `u8 n (<= 100), n x { u32 id, u8 flag, str128 name }`; the
/// client zeroes its table first, so the message is the full list (a 58 op is answered with a fresh BB). Flash gets
/// the names (`root.set_presets`) and `root.allow_save_preset(n != 0)`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PresetsBB {
    pub presets: Vec<PresetEntry>,
}

impl Encode for PresetsBB {
    fn encode(&self, w: &mut Writer) {
        if self.presets.len() > 100 {
            w.fail(Error::Limit("BB presets (100)"));
        }
        w.count_u8(self.presets.len(), "BB presets");
        for p in &self.presets {
            w.u32(p.id).u8(p.flag).str_max(&p.name, 127);
        }
    }
}

impl Decode for PresetsBB {
    fn decode(r: &mut Reader) -> Result<Self> {
        let n = r.u8()? as usize;
        if n > 100 {
            return Err(Error::Limit("BB presets (100)"));
        }
        r.check_count(n, 6)?;
        let mut presets = Vec::with_capacity(n);
        for _ in 0..n {
            presets.push(PresetEntry { id: r.u32()?, flag: r.u8()?, name: r.str()? });
        }
        Ok(PresetsBB { presets })
    }
}
