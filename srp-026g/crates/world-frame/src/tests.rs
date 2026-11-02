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
fn at_their_caps_sections_and_states_go() {
    let state = vec![0; PLAYER_STATE_CAP];
    let caps = Section::ALL.map(|section| vec![0; section.cap()]);
    let full = body(
        &[(19, &state)],
        [&caps[0], &caps[1], &caps[2], &caps[3], &caps[4], &caps[5]],
    );
    assert!(Frame::parse(&full, Places::ALL).is_ok());
}

#[test]
fn a_section_over_its_buffer_is_refused() {
    for (index, section) in Section::ALL.into_iter().enumerate() {
        let over = vec![0; section.cap() + 1];
        let mut sections: [&[u8]; 6] = [&[]; 6];
        sections[index] = &over;
        let world = body(&[(0, STATE)], sections);
        let Err(Refusal::SectionOverCap { section: what, at, len }) =
            Frame::parse(&world, Places::ALL)
        else {
            panic!("{section:?} over its cap");
        };
        assert_eq!((what, len), (section, section.cap() + 1));
        assert_eq!(world[at - 2..at], (len as u16).to_le_bytes());
    }
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
    assert!(Frame::parse(&world, Places::of(1)).is_ok());
}

#[test]
fn a_refusal_says_part_offset_and_reason() {
    let line = |refusal: Refusal| format!("{refusal}");
    assert_eq!(
        line(Refusal::SectionOverCap {
            section: Section::Bullets,
            at: 0x2C,
            len: 0x2401
        }),
        "bullets at 0x2c: 0x2401 bytes, over 0x2400"
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
