//! The bear traps a world holds: where each is and what state it is in,
//! read from the world's "game objects" section ([`super::snapshot::Sections::objects`]).
//!
//! The clients run every trap (lockstep: placing, the sensor, the firing and
//! the timers are in each game's own world, `booby_trap_core` 0x3043C0..,
//! `client_patch.md` 3.14), so the match server learns of a trap only from
//! a world. The objects section is the games' objects one after another,
//! each reading as many bytes as it wrote, in an order the late join fixes
//! (`sur_convert::kit_objects`): a slot's grenades and traps first, the map's
//! objects after. A trap nobody has touched writes one zero byte; a placed
//! one writes `booby_trap_core::serialize` 0x3052E0, which is:
//!
//! ```text
//! u8   state      1 armed, 2 fired, 3 disarmed (0: a free trap, one byte, nothing more)
//! u32  0xFFFFFFFF in all 47,844 sightings of 987 traps (the game's scheduler id; not read)
//! f32 x16         the transform: rows i, j (up), k, then the place; the fourth column 0, 0, 0, 1
//! u32  0          (not read)
//! u32  the match time in ms at which it was placed
//! ```
//!
//! 77 bytes. The section is not walked object by object (that needs the
//! grenades' layout too); a placed trap is found by what it looks like: a
//! state byte, a transform whose three rotation rows have length 1 and whose
//! fourth column is `0, 0, 0, 1` (the closing `1.0` is searched for, so
//! this is a few `memchr`s over 300-600 bytes), and a placing time not
//! after the world's. PROVEN (the same rule in Python) on the 987 traps of 402 live matches of 12+
//! players (`traps-record`, 2026-10-26): every placed trap was found in
//! every world that held it, and the placer's team-kill and entrap counters
//! agreed with the states.
//!
//! What a world shows of a trap's life: armed from the placing for ever
//! (`armed_life_time` 0); fired for 8 s (`fired_life_time`), then gone;
//! disarmed (a hand defuse of 5 s, or a ranged hit) for 8 s, then gone; or
//! gone from armed when its owner placed another with all his traps out
//! (the game recycles the oldest). A death or a respawn removes nothing.

/// A trap's bytes in the objects section.
pub const RECORD_LEN: usize = 77;

/// The state byte of a placed trap (`booby_trap_state`; 0 is "removed").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapState {
    /// Waiting: whoever steps within its reach is caught, a friend too.
    Armed,
    /// It has caught somebody; the spent model stands for 8 s.
    Fired,
    /// Defused by use or a shot; the model stands for 8 s.
    Disarmed,
}

impl TrapState {
    fn of(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(Self::Armed),
            2 => Some(Self::Fired),
            3 => Some(Self::Disarmed),
            _ => None,
        }
    }
}

/// One placed trap of a world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedTrap {
    pub state: TrapState,
    /// Where it lies (the floor point, the transform's place), world metres.
    pub position: [f32; 3],
    /// Its up axis (the floor's normal where it was placed).
    pub up: [f32; 3],
    /// The match time at which it was placed, ms.
    pub placed_ms: u32,
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn f32_at(bytes: &[u8], at: usize) -> Option<f32> {
    Some(f32::from_bits(u32_at(bytes, at)?))
}

/// The trap record that starts at `at` in `objects`, if the bytes are one.
fn record_at(objects: &[u8], at: usize, world_ms: u32) -> Option<PlacedTrap> {
    let record = objects.get(at..at.checked_add(RECORD_LEN)?)?;
    let state = TrapState::of(record[0])?;
    let mut m = [0.0_f32; 16];
    for (i, v) in m.iter_mut().enumerate() {
        *v = f32_at(record, 5 + 4 * i)?;
    }
    // Rows i, j, k are unit axes, the fourth column is (0, 0, 0, 1).
    if m[3] != 0.0 || m[7] != 0.0 || m[11] != 0.0 || m[15] != 1.0 {
        return None;
    }
    for row in 0..3 {
        let [x, y, z] = [m[row * 4], m[row * 4 + 1], m[row * 4 + 2]];
        if !(0.98..1.02).contains(&(x * x + y * y + z * z).sqrt()) {
            return None;
        }
    }
    let position = [m[12], m[13], m[14]];
    if position.iter().any(|v| !v.is_finite() || v.abs() > 2000.0) {
        return None;
    }
    let placed_ms = u32_at(record, 73)?;
    (placed_ms <= world_ms).then_some(PlacedTrap {
        state,
        position,
        up: [m[4], m[5], m[6]],
        placed_ms,
    })
}

/// The placed traps in a world's objects section, in the order they lie
/// there. `world_ms` is the world's time (a trap cannot be placed after it).
pub fn find(objects: &[u8], world_ms: u32) -> Vec<PlacedTrap> {
    // The matrix's closing 1.0 is at record offset 5 + 60 .. 5 + 64.
    const ONE: [u8; 4] = 1.0_f32.to_le_bytes();
    const ONE_AT: usize = 5 + 60;
    let mut out = Vec::new();
    let mut from = ONE_AT;
    while let Some(found) = objects
        .get(from..)
        .and_then(|rest| rest.windows(4).position(|w| w == ONE))
    {
        let one = from + found;
        if let Some(trap) = record_at(objects, one - ONE_AT, world_ms) {
            out.push(trap);
            // The next record cannot start inside this one.
            from = one - ONE_AT + RECORD_LEN + ONE_AT;
        } else {
            from = one + 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The objects section of a live match's world at 254,600 ms (484
    /// bytes; recording 0289c453, a 12+ match): three free objects, two
    /// placed traps and the map's objects around them.
    const WORLD: &str = include_str!("traps_world.hex");

    fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.trim().len() / 2)
            .map(|i| u8::from_str_radix(&hex.trim()[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }

    fn record(state: u8, position: [f32; 3], placed_ms: u32) -> Vec<u8> {
        let mut out = vec![state];
        out.extend(u32::MAX.to_le_bytes());
        let matrix = [
            1.0_f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, position[0],
            position[1], position[2], 1.0,
        ];
        for v in matrix {
            out.extend(v.to_le_bytes());
        }
        out.extend(0_u32.to_le_bytes());
        out.extend(placed_ms.to_le_bytes());
        assert_eq!(out.len(), RECORD_LEN);
        out
    }

    #[test]
    fn a_live_world_gives_its_two_traps_with_their_places_and_times() {
        let traps = find(&bytes(WORLD), 254_600);
        assert_eq!(traps.len(), 2, "{traps:?}");
        assert_eq!(traps[0].state, TrapState::Armed);
        assert_eq!(traps[0].placed_ms, 101_850);
        assert!((traps[0].position[0] - 83.8394).abs() < 1e-3);
        assert!((traps[0].position[1] - 21.2948).abs() < 1e-3);
        assert!((traps[0].position[2] - 80.9087).abs() < 1e-3);
        assert!(traps[0].up[1] > 0.99, "the floor is level: {:?}", traps[0].up);
        assert_eq!(traps[1].state, TrapState::Armed);
        assert_eq!(traps[1].placed_ms, 248_600);
        assert!((traps[1].position[0] - 81.9418).abs() < 1e-3);
        assert!((traps[1].position[2] - 78.5215).abs() < 1e-3);
    }

    #[test]
    fn a_trap_placed_after_the_worlds_time_is_not_one() {
        assert_eq!(find(&bytes(WORLD), 200_000).len(), 1);
        assert!(find(&bytes(WORLD), 100_000).is_empty());
    }

    #[test]
    fn the_three_states_and_only_them() {
        for (byte, state) in [
            (1, TrapState::Armed),
            (2, TrapState::Fired),
            (3, TrapState::Disarmed),
        ] {
            let mut section = vec![0, 0];
            section.extend(record(byte, [1.0, 2.0, 3.0], 500));
            section.extend([0, 7, 7]);
            let traps = find(&section, 1_000);
            assert_eq!(traps.len(), 1);
            assert_eq!(traps[0].state, state);
        }
        for byte in [0_u8, 4, 255] {
            let section = record(byte, [1.0, 2.0, 3.0], 500);
            assert!(find(&section, 1_000).is_empty(), "state byte {byte}");
        }
    }

    #[test]
    fn free_objects_and_other_bytes_are_not_traps() {
        assert!(find(&[], 1_000).is_empty());
        assert!(find(&[0; 40], 1_000).is_empty());
        assert!(find(&[0, 0, 0x80, 0x3f, 1, 1, 1, 0x80, 0x3f], 1_000).is_empty());
        // A matrix whose rows are not unit axes, or whose last column is not 0, 0, 0, 1.
        let mut skewed = record(1, [1.0, 2.0, 3.0], 500);
        skewed[5..9].copy_from_slice(&3.0_f32.to_le_bytes());
        assert!(find(&skewed, 1_000).is_empty());
        let mut open = record(1, [1.0, 2.0, 3.0], 500);
        open[5 + 12..5 + 16].copy_from_slice(&0.5_f32.to_le_bytes());
        assert!(find(&open, 1_000).is_empty());
        // Cut short: the record is 77 bytes.
        let whole = record(1, [1.0, 2.0, 3.0], 500);
        assert!(find(&whole[..RECORD_LEN - 1], 1_000).is_empty());
    }

    #[test]
    fn several_traps_in_a_row_are_all_found() {
        let mut section = vec![0; 5];
        for k in 0..6_u32 {
            section.extend(record(1, [k as f32, 0.0, -(k as f32)], 100 + k));
        }
        let traps = find(&section, 1_000);
        assert_eq!(traps.len(), 6);
        assert_eq!(traps[5].placed_ms, 105);
        assert_eq!(traps[5].position, [5.0, 0.0, -5.0]);
    }
}
