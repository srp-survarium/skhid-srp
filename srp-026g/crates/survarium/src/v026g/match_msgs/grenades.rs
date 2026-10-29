//! The thrown grenades a world holds, in flight or at rest: where each body is and how it moves, read from the
//! world's "game objects" section ([`super::snapshot::Sections::objects`]), as [`super::traps`] reads the traps.
//!
//! Every grenade (a frag `rgd-5` or a smoke `rdg-m`: the one `grenade_core` class) writes
//! `grenade_core::serialize` 0x303DA0 (PROVEN, disassembly; `deserialize` 0x303F80 reads it back):
//!
//! ```text
//! u8   state   bit0: the body is in the physics world (launched, not yet blown); bit1: the fuse is lit; bit2: blown
//! if bit1: u32 the match ms it goes off at (the press + its ttl_ms, on the 50 ms grid), u32 its id
//! if bit0: f32 x16 the transform (rows i, j, k, then the place; the fourth column 0, 0, 0, 1),
//!          f32 x3 the linear velocity, f32 x3 the angular velocity (game frame)
//! ```
//!
//! So a grenade in the hand is 9 bytes (state 2) and one launched 97 (state 3, this module's); after the blast the body
//! and the fuse are cleared, so no world holds the blast point: the last sighting before it is the source. Counted:
//! 139 grenades with a body in 9,053 bench worlds, 21 in 4,297 live ones at their 5 s cadence; once at rest a grenade
//! moves on a median 0.29 m before its blast (P95 2.7 m) (`fx-smoke-rest-source.md`, PROVEN counts).
//!
//! Found by what it looks like, not walked: the transform's closing `1.0` is searched for (record offset 9 + 60), then
//! the state byte 3, a blast time after the world's and at most a fuse ahead, a transform whose rotation rows have
//! length 1, and a sane place and speed. Parsed once into [`GrenadeBody`]: its blast, id, place and [`Motion`].

/// A launched grenade's bytes in the objects section.
pub const RECORD_LEN: usize = 97;

/// A blast this far after the world at most, ms: the longest fuse of the game data (4000) and a margin.
pub const FUSE_MOST_MS: u32 = 4_200;

/// A body slower than this is at rest, m/s: Bullet's linear sleeping threshold (`btRigidBodyConstructionInfo::
/// m_linearSleepingThreshold`, 0.8 by default; PROVEN in Bullet's source, the game's grenade on the default LIKELY: no
/// sleeping threshold is set in its config). Faster, it is still flying or rolling, and its place is not where it
/// stops.
pub const RESTING_MPS: f32 = 0.8;

/// The match ms a grenade goes off at (its press + its fuse, on the 50 ms grid): the key to the throw the server saw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlastMs(pub u32);

/// The game's id of a grenade object (a number that grows through the match).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GrenadeId(pub u32);

/// A place in the world, metres (the game's frame: y up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldPoint(pub [f32; 3]);

/// A velocity, metres a second (the game's frame).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity(pub [f32; 3]);

impl Velocity {
    /// Its speed, metres a second.
    pub fn speed(self) -> f32 {
        let [x, y, z] = self.0;
        (x * x + y * y + z * z).sqrt()
    }
}

/// How a grenade's body moves in a world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    /// Under [`RESTING_MPS`]: where it lies is where it goes off (P95 2.7 m later).
    Resting,
    /// Flying or rolling: where it is now is not where it stops.
    Moving(Velocity),
}

/// One launched grenade of a world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrenadeBody {
    pub blast: BlastMs,
    pub id: GrenadeId,
    /// Where the body is (the transform's place).
    pub place: WorldPoint,
    pub motion: Motion,
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn f32_at(bytes: &[u8], at: usize) -> Option<f32> {
    Some(f32::from_bits(u32_at(bytes, at)?))
}

/// The grenade record that starts at `at` in `objects`, if the bytes are one.
fn record_at(objects: &[u8], at: usize, world_ms: u32) -> Option<GrenadeBody> {
    let record = objects.get(at..at.checked_add(RECORD_LEN)?)?;
    if record[0] != 3 {
        return None;
    }
    let blast = u32_at(record, 1)?;
    if blast < world_ms || blast > world_ms.saturating_add(FUSE_MOST_MS) {
        return None;
    }
    let id = u32_at(record, 5)?;
    let mut m = [0.0_f32; 16];
    for (i, v) in m.iter_mut().enumerate() {
        *v = f32_at(record, 9 + 4 * i)?;
    }
    if m[3] != 0.0 || m[7] != 0.0 || m[11] != 0.0 || m[15] != 1.0 {
        return None;
    }
    for row in 0..3 {
        let [x, y, z] = [m[row * 4], m[row * 4 + 1], m[row * 4 + 2]];
        if !(0.98..1.02).contains(&(x * x + y * y + z * z).sqrt()) {
            return None;
        }
    }
    let place = [m[12], m[13], m[14]];
    let velocity = [f32_at(record, 73)?, f32_at(record, 77)?, f32_at(record, 81)?];
    let sane = |v: &[f32; 3], most: f32| v.iter().all(|x| x.is_finite() && x.abs() <= most);
    if !(sane(&place, 2000.0) && sane(&velocity, 60.0)) {
        return None;
    }
    let velocity = Velocity(velocity);
    Some(GrenadeBody {
        blast: BlastMs(blast),
        id: GrenadeId(id),
        place: WorldPoint(place),
        motion: if velocity.speed() < RESTING_MPS {
            Motion::Resting
        } else {
            Motion::Moving(velocity)
        },
    })
}

/// The launched grenades in a world's objects section, in the order they lie there. `world_ms` is the world's time.
pub fn find(objects: &[u8], world_ms: u32) -> Vec<GrenadeBody> {
    const ONE: [u8; 4] = 1.0_f32.to_le_bytes();
    const ONE_AT: usize = 9 + 60;
    let mut out = Vec::new();
    let mut from = ONE_AT;
    while let Some(found) = objects
        .get(from..)
        .and_then(|rest| rest.windows(4).position(|w| w == ONE))
    {
        let one = from + found;
        if let Some(grenade) = record_at(objects, one - ONE_AT, world_ms) {
            out.push(grenade);
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

    fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.trim().len() / 2)
            .map(|i| u8::from_str_radix(&hex.trim()[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }

    fn record(state: u8, blast: u32, place: [f32; 3], velocity: [f32; 3]) -> Vec<u8> {
        let mut out = vec![state];
        out.extend(blast.to_le_bytes());
        out.extend(4858_u32.to_le_bytes());
        let matrix = [
            1.0_f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, place[0], place[1], place[2], 1.0,
        ];
        for v in matrix.into_iter().chain(velocity).chain([0.1, 0.0, -0.2]) {
            out.extend(v.to_le_bytes());
        }
        assert_eq!(out.len(), RECORD_LEN);
        out
    }

    /// Two saved bench worlds (`pvb-final-local/worlds`, match 13766414, bot grenades): at 125,100 ms a grenade lies at
    /// rest (0.18 m/s) at (2.478, 21.795, -72.466), going off at 125,700; at 271,900 one flies at 11.7 m/s through
    /// (41.325, 20.387, -40.0), going off at 273,900.
    #[test]
    fn two_saved_worlds_give_a_grenade_at_rest_and_one_in_flight() {
        let rest = find(&bytes(include_str!("grenades_world_rest.hex")), 125_100);
        assert_eq!(rest.len(), 1, "{rest:?}");
        assert_eq!((rest[0].blast, rest[0].id, rest[0].motion), (BlastMs(125_700), GrenadeId(1645), Motion::Resting));
        let [x, y, z] = rest[0].place.0;
        assert!((x - 2.478).abs() < 1e-3 && (y - 21.795).abs() < 1e-3 && (z + 72.466).abs() < 1e-3);
        let fast = find(&bytes(include_str!("grenades_world_fast.hex")), 271_900);
        assert_eq!(fast.len(), 1, "{fast:?}");
        assert_eq!((fast[0].blast, fast[0].id), (BlastMs(273_900), GrenadeId(4379)));
        let Motion::Moving(v) = fast[0].motion else {
            panic!("{fast:?}");
        };
        assert!((v.speed() - 11.73).abs() < 0.01);
    }

    #[test]
    fn a_launched_grenade_is_found_among_other_objects_with_its_place_and_motion() {
        let mut objects = vec![0u8, 2];
        objects.extend(288_900_u32.to_le_bytes());
        objects.extend(17_u32.to_le_bytes());
        objects.extend(record(3, 288_900, [14.35, 19.47, 20.32], [0.05, 0.0, 0.02]));
        objects.extend(record(3, 289_400, [1.0, 2.0, 3.0], [15.0, 0.0, 0.0]));
        objects.extend([0u8; 3]);
        let found = find(&objects, 288_000);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!((found[0].blast, found[0].id, found[0].motion), (BlastMs(288_900), GrenadeId(4858), Motion::Resting));
        assert_eq!(found[1].motion, Motion::Moving(Velocity([15.0, 0.0, 0.0])));
    }

    #[test]
    fn a_blown_or_stale_record_and_a_trap_are_not_grenades() {
        // Blown (bit2), or its blast before the world, or more than a fuse ahead.
        for (state, blast, world) in [(7, 288_900, 288_000), (3, 287_000, 288_000), (3, 293_000, 288_000)] {
            assert!(find(&record(state, blast, [1.0, 2.0, 3.0], [0.0; 3]), world).is_empty());
        }
        // The live world of `traps.rs` (two placed traps, no grenade).
        assert!(find(&bytes(include_str!("traps_world.hex")), 254_600).is_empty());
    }
}
