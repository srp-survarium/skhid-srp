//! Patch 154: a maps_dict id over a byte (client_patch.md "Map ids over a
//! byte"; static_info.md §6). The match options carry the map as one byte
//! (match_options::deserialize stores it at +577B4h, 0x1F0EAD), and six
//! places read it as one (`movzx`: the hud's game type 0x234CFD, the players'
//! query 0x27AFA7, the static info's log 0x27BC26 and its level loading
//! 0x27BE8B, the quests' ui 0x2A3603, the world core 0x2D01F3). patch_026g.py
//! turns each into a read of the options' spare word +577B8h (the game's
//! "match maker id", logged only), which this patch fills at the options'
//! read (`static_info.options_read`, `replay.options_read`, first in their
//! chains): the whole id srp's block gives ("SRPX", tag 4, a u16 whose low
//! byte is the options' byte), else the options' byte. So an old server's
//! match and an old replay load as before, and the maps past 255 load at
//! all. Nothing a match computes.

#![forbid(unsafe_code)]

use crate::vostok::parse::Reader;
use crate::vostok::{Context, Flow, MapId, NoExit, OptionsRead};

/// srp's block after the options: its magic, then `{u8 tag, u8 len, bytes}`.
const SRPX: [u8; 4] = *b"SRPX";
/// The whole map id's entry: a u16, over a byte.
const TAG_MAP_ID: u8 = 4;

/// The map the options name: the whole id of srp's block's entry, when it is
/// over a byte and its low byte is the options' `byte`; else `byte`. An entry
/// of another tag or length is stepped over, a cut one ends the block.
pub fn map_id_in(mut rest: Reader, byte: u8) -> MapId {
    let mut id = MapId::from(byte);
    if rest.array::<4>() != Some(SRPX) {
        return id;
    }
    while let (Some(tag), Some(len)) = (rest.u8(), rest.u8()) {
        let Some(entry) = rest.take(usize::from(len)) else { break };
        if let (TAG_MAP_ID, &[low, high]) = (tag, entry) {
            let whole = MapId::new(u16::from_le_bytes([low, high]));
            if whole.over_a_byte() && whole.low_byte() == byte {
                id = whole;
            }
        }
    }
    id
}

fn options_read(read: Option<OptionsRead>) -> Flow<NoExit> {
    if let Some(mut read) = read {
        let id = map_id_in(read.rest(), read.map_byte());
        read.set_map_id(id);
    }
    Flow::Continue
}

/// process_static_match_info after its deserialize (0x27B92D).
pub fn on_static_info(ctx: &mut Context) -> Flow<NoExit> {
    options_read(OptionsRead::at_static_info(ctx))
}

/// A replay's chunk 1 after its deserialize (0x1F0971).
pub fn on_replay_options(ctx: &mut Context) -> Flow<NoExit> {
    options_read(OptionsRead::at_replay(ctx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(bytes: &[u8], byte: u8) -> u16 {
        map_id_in(Reader::new(bytes), byte).get()
    }

    #[test]
    fn the_whole_id_or_the_byte() {
        assert_eq!(id(b"SRPX\x04\x02\x2c\x01", 44), 300);
        assert_eq!(id(b"SRPX\x02\x01\x01\x04\x02\x2c\x01", 44), 300);
        assert_eq!(id(b"SRPX\x04\x02\x2c\x01\x03\x02\x1e\x00", 44), 300);
        assert_eq!(id(b"SRPX\x09\x03xyz\x04\x02\x2c\x01", 44), 300);
        // No block, another magic, another low byte, an id of a byte, a cut
        // or other length entry: the options' byte.
        assert_eq!(id(b"", 61), 61);
        assert_eq!(id(b"SRPY\x04\x02\x2c\x01", 44), 44);
        assert_eq!(id(b"SRPX\x04\x02\x2c\x01", 61), 61);
        assert_eq!(id(b"SRPX\x04\x02\x3d\x00", 61), 61);
        assert_eq!(id(b"SRPX\x04\x02\x2c", 44), 44);
        assert_eq!(id(b"SRPX\x04\x03\x2c\x01\x00", 44), 44);
    }
}
