use std::format;
use std::vec;
use std::vec::Vec;

use super::*;

/// A body of `blocks` (a slot and its state, not animated) and `sections`.
fn body(blocks: &[(u8, &[u8])], sections: [&[u8]; 6]) -> Vec<u8> {
    let mask = blocks.iter().fold(0u32, |mask, (slot, _)| mask | 1 << slot);
    let mut body = mask.to_le_bytes().to_vec();
    for (_, state) in blocks {
        body.extend(u16::try_from(state.len()).unwrap().to_le_bytes());
        body.extend_from_slice(state);
        body.push(0);
    }
    for section in sections {
        body.extend(u16::try_from(section.len()).unwrap().to_le_bytes());
        body.extend_from_slice(section);
    }
    body
}

const STATE: &[u8] = &[0xAB; 33];

fn world() -> Vec<u8> {
    body(
        &[(0, STATE), (1, STATE)],
        [&[1; 7], &[2; 9], &[3; 3], &[], &[5; 11], &[6; 2]],
    )
}

#[test]
fn a_world_reads_into_its_slots_and_sections() {
    let world = world();
    let frame = Frame::parse(&world, Places::of(2)).unwrap();
    assert_eq!(frame.slots().mask(), 0b11);
    assert_eq!(
        frame.slots().iter().map(Slot::index).collect::<Vec<_>>(),
        [0, 1]
    );
    let rules = frame.section(Section::Rules);
    assert_eq!(rules.bytes, &[5; 11]);
    assert_eq!(
        frame.states().map(|(slot, state)| (slot.index(), state)).collect::<Vec<_>>(),
        [(0, STATE), (1, STATE)]
    );
    assert_eq!(&world[rules.at..rules.at + 11], rules.bytes);
    assert!(frame.section(Section::Physics).bytes.is_empty());
    // The game's state has the world's time first.
    let mut state = 4500u32.to_le_bytes().to_vec();
    state.extend(&world);
    assert_eq!(Frame::parse_state(&state, Places::ALL), Ok(frame));
    assert_eq!(
        Frame::parse_state(&[1, 2, 3], Places::ALL),
        Err(Refusal::NoTime)
    );
    // Without the time it does not read: the mask would be the time.
    assert!(Frame::parse_state(&world, Places::ALL).is_err());
}

#[test]
fn a_state_at_its_cap_and_sections_past_the_old_inline_buffers_go() {
    let state = vec![0; PLAYER_STATE_CAP];
    // Each section over the inline buffer the original game had for it
    // (0x2400, 0xC00, 0x2000, 0x4000, 0x800, 0xC00): patch 98 grows them.
    let sections = [0x2401, 0xC01, 0x2001, 0x4001, 0x801, 0xC01].map(|len| vec![0; len]);
    let full = body(&[(19, &state)], sections.each_ref().map(Vec::as_slice));
    assert!(full.len() < MAX_BODY);
    assert!(Frame::parse(&full, Places::ALL).is_ok());
}

#[test]
fn a_body_over_the_world_buffer_is_refused() {
    // 300 bullets in flight and twenty players fit; the bullets alone at
    // the world buffer do not.
    let state = vec![0; 0x3A8];
    let blocks: Vec<(u8, &[u8])> = (0..20).map(|slot| (slot, &state[..])).collect();
    let bullets = vec![0; 300 * 69];
    let world = body(&blocks, [&bullets, &[], &[], &[], &[], &[]]);
    assert!(Frame::parse(&world, Places::ALL).is_ok());
    let fill = |len: usize| body(&[], [&vec![0; len], &[], &[], &[], &[], &[]]);
    // mask 4, six u16 lengths 12.
    let at = fill(MAX_BODY - 16);
    assert_eq!(at.len(), MAX_BODY);
    assert!(Frame::parse(&at, Places::ALL).is_ok());
    assert_eq!(
        Frame::parse(&fill(MAX_BODY - 15), Places::ALL),
        Err(Refusal::OverBuffer { len: MAX_BODY + 1 })
    );
}

#[test]
fn a_state_over_its_buffer_is_refused() {
    let state = vec![0; PLAYER_STATE_CAP + 1];
    let world = body(&[(0, STATE), (3, &state)], [&[]; 6]);
    assert_eq!(
        Frame::parse(&world, Places::ALL),
        Err(Refusal::StateOverCap {
            slot: Slot(3),
            at: 4 + 2 + 33 + 1 + 2,
            len: PLAYER_STATE_CAP + 1
        })
    );
}

#[test]
fn a_slot_past_the_places_is_refused() {
    let world = body(&[(0, STATE), (2, STATE)], [&[]; 6]);
    assert!(Frame::parse(&world, Places::of(3)).is_ok());
    assert_eq!(
        Frame::parse(&world, Places::of(2)),
        Err(Refusal::SlotPastPlaces {
            bit: 2,
            places: Places(2)
        })
    );
    // Past the 20 slots of any game; a roster over 20 is 20.
    let mut high = world.clone();
    high[..4].copy_from_slice(&(1u32 << 31 | 1).to_le_bytes());
    assert_eq!(
        Frame::parse(&high, Places::of(64)),
        Err(Refusal::SlotPastPlaces {
            bit: 31,
            places: Places::ALL
        })
    );
}

#[test]
fn a_body_that_does_not_walk_to_its_end_is_refused() {
    let world = world();
    // A mask naming a block that is not there: a section's length read as
    // a state's.
    let mut torn = world.clone();
    torn[..4].copy_from_slice(&0b111u32.to_le_bytes());
    assert!(Frame::parse(&torn, Places::ALL).is_err());
    // Cut anywhere.
    for len in 0..world.len() {
        assert!(
            matches!(
                Frame::parse(&world[..len], Places::ALL),
                Err(Refusal::Short { .. })
            ),
            "cut at {len}"
        );
    }
    let mut longer = world.clone();
    longer.extend([0, 0, 0]);
    assert_eq!(
        Frame::parse(&longer, Places::ALL),
        Err(Refusal::Trailing {
            at: world.len(),
            len: 3
        })
    );
    assert_eq!(
        Frame::parse(&[0xEE; 777], Places::ALL).map(|_| ()),
        Err(Refusal::SlotPastPlaces {
            bit: 21,
            places: Places::ALL
        })
    );
}

#[test]
fn an_animated_block_ends_where_its_bits_do() {
    // T = 0 (no tree), no times, floats: not indexed, one: 16 + 10 + 1 +
    // 10 + 32 = 69 bits, 9 bytes.
    let mut animation = [0u8; 9];
    // bits 27..36: the floats' count, 1 (bit 36 set).
    animation[36 / 8] |= 0x80 >> (36 % 8);
    assert_eq!(animation_len(&animation), Some(9));
    assert_eq!(animation_len(&animation[..8]), None);
    let mut world = 1u32.to_le_bytes().to_vec();
    world.extend(33u16.to_le_bytes());
    world.extend(STATE);
    world.push(1);
    world.extend(animation);
    world.extend([0; 12]);
    // The length walk ends the block here, but a set animation flag means a
    // real tree: `T` = 0 cannot be one, so the content validator refuses it.
    assert_eq!(
        Frame::parse(&world, Places::of(1)),
        Err(Refusal::Animation {
            slot: Slot(0),
            at: 40,
            fault: AnimFault::TreeLength,
        })
    );
}

/// Writes bits most significant first, like the animation encoder, to test
/// [`animation_validate`] against hand-built streams.
struct BitW {
    bytes: Vec<u8>,
    at: usize,
}

impl BitW {
    fn new() -> Self {
        BitW { bytes: Vec::new(), at: 0 }
    }

    fn put(&mut self, value: u32, bits: usize) -> &mut Self {
        for i in (0..bits).rev() {
            if self.at.is_multiple_of(8) {
                self.bytes.push(0);
            }
            let bit = (value.wrapping_shr(i as u32) & 1) as u8;
            let shift = (7 ^ (self.at % 8)) as u32;
            *self.bytes.last_mut().unwrap() |= bit.wrapping_shl(shift);
            self.at = self.at.wrapping_add(1);
        }
        self
    }

    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// The header of a tree with `nodes` nodes and `channels_mask` set channel
/// bits, behind a `tree`-bit length.
fn tree_header(w: &mut BitW, tree: u32, nodes: u32, channels_mask: u32) {
    w.put(tree, 16).put(nodes, 6).put(0, 1).put(channels_mask, 4);
}

#[test]
fn animation_validate_accepts_a_minimal_tree() {
    // No nodes (the 11-bit header is the whole tree), no times, and the 12
    // floats the matrix init always pops, written plainly.
    let mut w = BitW::new();
    tree_header(&mut w, 11, 0, 0);
    w.put(0, 10); // times: count 0
    w.put(0, 1).put(12, 10); // floats: not indexed, 12 of them
    for _ in 0..12 {
        w.put(0, 32);
    }
    assert_eq!(animation_validate(w.bytes()), Ok(()));
}

#[test]
fn animation_validate_rejects_a_wrong_tree_length() {
    // The same 11-bit tree, but `T` claims 10: the structure outruns it.
    let mut w = BitW::new();
    tree_header(&mut w, 10, 0, 0);
    w.put(0, 10);
    w.put(0, 1).put(12, 10);
    for _ in 0..12 {
        w.put(0, 32);
    }
    assert_eq!(animation_validate(w.bytes()), Err(AnimFault::TreeLength));
}

#[test]
fn animation_validate_rejects_a_table_under_run() {
    // No floats, but the matrix init pops 12.
    let mut w = BitW::new();
    tree_header(&mut w, 11, 0, 0);
    w.put(0, 10); // times: count 0
    w.put(0, 1).put(0, 10); // floats: not indexed, count 0
    assert_eq!(animation_validate(w.bytes()), Err(AnimFault::TableUnderrun));
}

/// The minimal tree's 12 floats as a table of two unique ones, the last
/// float's index `last` (in width(2) = 2 bits).
fn indexed_floats(last: u32) -> Vec<u8> {
    let mut w = BitW::new();
    tree_header(&mut w, 11, 0, 0);
    w.put(0, 10); // times: count 0
    w.put(1, 1).put(12, 10); // floats: indexed, count 12
    w.put(2, 4); // unique = 2, in width(12) = 4 bits
    w.put(0, 32).put(0x3F80_0000, 32); // the two table floats
    for _ in 0..11 {
        w.put(1, 2);
    }
    w.put(last, 2);
    w.bytes().to_vec()
}

#[test]
fn animation_validate_bounds_a_floats_index_by_the_unique_count() {
    assert_eq!(animation_validate(&indexed_floats(0)), Ok(()));
    // The unique count: the index `save_floats` (0x397040) writes for a NaN,
    // which its `find` never matches (a real stream of
    // rec026g_20261106-211841_03439962 has three).
    assert_eq!(animation_validate(&indexed_floats(2)), Ok(()));
    // Past it: no encoder writes this.
    assert_eq!(
        animation_validate(&indexed_floats(3)),
        Err(AnimFault::FloatsIndex)
    );
}

#[test]
fn animation_validate_rejects_a_times_index_at_the_unique_count() {
    // A time is a u32, always found by the encoder's search: an index at the
    // count is no stream the game writes.
    let mut w = BitW::new();
    tree_header(&mut w, 11, 0, 0);
    w.put(2, 10); // times: count 2
    w.put(1, 1).put(0, 32); // indexed, base 0
    w.put(0, 5); // each delta 0 bits
    w.put(1, 2); // unique = 1, in width(2) = 2 bits
    w.put(0, 1).put(1, 1); // indexes in width(1) = 1 bit: 0, then 1
    w.put(0, 1).put(12, 10); // floats: not indexed, 12 of them
    for _ in 0..12 {
        w.put(0, 32);
    }
    assert_eq!(animation_validate(w.bytes()), Err(AnimFault::TimesIndex));
}

#[test]
fn animation_validate_rejects_a_channel_index() {
    // A node that picks a channel from an empty channel set.
    let mut w = BitW::new();
    tree_header(&mut w, 0, 1, 0); // 1 node, 0 channels
    // Base tracks (0x3A8C10): four bits, a zero time track, a flag track.
    w.put(0, 1).put(0, 1).put(0, 1).put(0, 1);
    w.put(0, 6); // time-track count 0
    w.put(0, 1).put(0, 1); // flag track
    // Node record (0x3A8F40) up to its first channel pick.
    w.put(0, 4).put(0, 1).put(0, 1); // the record head, M = 1
    w.put(0, 10).put(0, 1); // the one M entry
    w.put(0, 1); // before the first pick
    assert_eq!(animation_validate(w.bytes()), Err(AnimFault::ChannelIndex));
}

#[test]
fn animation_validate_rejects_runaway_recursion() {
    // One node with one expression root, then only internal ops: the tree
    // never closes, and the node cap stops it before a stack would.
    let mut w = BitW::new();
    tree_header(&mut w, 0, 1, 1); // 1 node, 1 channel (a pick reads 0 bits)
    w.put(0, 1).put(0, 1).put(0, 1).put(0, 1);
    w.put(0, 6);
    w.put(0, 1).put(0, 1);
    // Node record, M = 1, then the fixed fields and exprs = 1.
    w.put(0, 4).put(0, 1).put(0, 1);
    w.put(0, 10).put(0, 1);
    w.put(0, 1);
    for width in [1, 5, 2, 1, 3, 3, 2, 1, 1] {
        w.put(0, width);
    }
    w.put(1, 2); // exprs = 1
    for _ in 0..70_000 {
        w.put(2, 2); // op 2: two children, forever -- past the node cap
    }
    assert_eq!(animation_validate(w.bytes()), Err(AnimFault::Recursion));
}

#[test]
fn a_refusal_says_part_offset_and_reason() {
    let line = |refusal: Refusal| format!("{refusal}");
    assert_eq!(
        line(Refusal::OverBuffer { len: 0xF001 }),
        "body at 0x0: 0xf001 bytes, over 0xf000"
    );
    assert_eq!(
        line(Refusal::StateOverCap {
            slot: Slot(3),
            at: 6,
            len: 0x1001
        }),
        "player 3 at 0x6: a state of 0x1001 bytes, over 0x1000"
    );
    assert_eq!(
        line(Refusal::SlotPastPlaces {
            bit: 2,
            places: Places(2)
        }),
        "mask at 0x0: a player in slot 2, past the 2 places"
    );
    assert_eq!(
        line(Refusal::Short {
            part: Part::Section(Section::Rules),
            at: 0x40
        }),
        "rules at 0x40: the bytes end inside it"
    );
    assert_eq!(
        line(Refusal::Trailing { at: 0x50, len: 3 }),
        "end at 0x50: 3 bytes after the effects"
    );
}
