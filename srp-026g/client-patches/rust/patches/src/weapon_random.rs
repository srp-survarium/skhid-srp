//! Patch 18: the weapons' random state at a spawn (client_patch.md section
//! 3.18; patch_026g.py WEAPON_RANDOM).
//!
//! A weapon's random state (weapon_core+5BCh, boost's minstd_rand, drawn
//! from by the spread of its shots alone) is the constructor's 1 and is
//! otherwise only read from a world. A world holds the current kit's player
//! alone, so a game that loaded one during a match had the other kits'
//! weapons at 1 where the games that played them had drawn from them: a
//! respawn into such a kit, or a rollback over one, put the games out of
//! step. At every spawn each weapon of the new kit's player gets a state
//! every game works out alike from its place, kit, slot and the spawn's
//! time, before it is put into the world.

#![forbid(unsafe_code)]

use crate::vostok::spawn::Spawned;
use crate::vostok::{Context, Flow, NoExit};

/// The state a weapon gets: from the player's place and kit (the bytes the
/// game holds), its inventory slot and the spawn's time,
///   h = ((place << 16 | kit << 8 | slot) * 0x85EBCA6B) ^ (time * 0x9E3779B1)
///   state = (h ^ h >> 15) mod (2^31 - 2) + 1, in 1..2^31-2
/// (0 would stay 0 for ever, 2^31-1 would become 0), u32 arithmetic.
pub fn state(place: u8, kit: u8, slot: u32, time: u32) -> u32 {
    let key = (u32::from(place) << 16 | u32::from(kit) << 8) | slot;
    let h = key.wrapping_mul(KEY_FACTOR) ^ time.wrapping_mul(TIME_FACTOR);
    (h ^ h >> 15) % STATES + 1
}

// The constants in halves: the layer check takes a literal of 10000h or
// more for a game address, and these are none.
/// 0x85EBCA6B (murmur3's first mixing constant).
const KEY_FACTOR: u32 = 0x85EB << 16 | 0xCA6B;
/// 0x9E3779B1 (2^32 over the golden ratio, rounded to an odd number).
const TIME_FACTOR: u32 = 0x9E37 << 16 | 0x79B1;
/// 2^31 - 2: the states minstd_rand can be in but its fixed point.
const STATES: u32 = (1 << 31) - 2;

/// `spawn.weapon_random`: the new kit's player is initialised, not yet in
/// the world. Each weapon of its inventory gets its state; then the two
/// instructions replaced (Continue).
pub fn on_spawn(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let spawn = Spawned::at(ctx);
    if let Some(player) = spawn.player() {
        for (slot, weapon) in player.weapons() {
            weapon.set_random_state(state(player.place(), player.kit(), slot, spawn.time().0));
        }
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    /// patch_026g.py's weapon_random_state, the model the checks use.
    fn model(place: u64, kit: u64, slot: u64, time: u64) -> u64 {
        let h = ((((place & 0xFF) << 16 | (kit & 0xFF) << 8 | slot) * 0x85EB_CA6B) ^ (time * 0x9E37_79B1)) & 0xFFFF_FFFF;
        (h ^ h >> 15) % (0x7FFF_FFFF - 1) + 1
    }

    #[test]
    fn the_model_of_the_checks() {
        for place in [0u8, 1, 3, 8, 19, 255] {
            for kit in [0u8, 1, 2, 255] {
                for slot in [0u32, 7, 10, 22] {
                    for time in [0u32, 50, 21_750, 148_800, 729_200, 0xFFFF_FFCE, u32::MAX] {
                        let want = model(place.into(), kit.into(), slot.into(), time.into());
                        assert_eq!(u64::from(state(place, kit, slot, time)), want);
                    }
                }
            }
        }
    }

    #[test]
    fn states_are_in_range_and_apart() {
        let mut seen = std::collections::HashSet::new();
        for place in 0..20u8 {
            for kit in 0..3u8 {
                for slot in [7u32, 10] {
                    for time in [0u32, 50, 100, 21_750, 900_000] {
                        let s = state(place, kit, slot, time);
                        assert!((1..=0x7FFF_FFFE).contains(&s));
                        seen.insert(s);
                    }
                }
            }
        }
        assert_eq!(seen.len(), 20 * 3 * 2 * 5);
    }
}
