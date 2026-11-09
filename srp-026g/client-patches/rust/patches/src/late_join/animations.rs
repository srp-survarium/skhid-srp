//! A player's animations given ids that a later place does not move (before
//! the Rust port `lj_native_animations`, late_join_install_026g.c).
//!
//! The registry gives each animation an id, its index in its ids' vector,
//! which the game's worlds carry. The game makes the vector once, sorted by
//! name, so a player whose animations come in later (a newcomer's kit) would
//! renumber every animation after its first new one: the games that had the
//! match before and the newcomer's would name animations apart. Here the
//! ids already given stay; the entries the vector does not hold yet go after
//! them, sorted by name among themselves, in every game alike (each loads
//! the roster in order, and a newcomer's places after).

use crate::vostok::late_join::{AnimatedPlayer, AnimationRegistry};

/// The game's registration of `player`'s animations, then the new entries'
/// ids after the ones given.
pub fn register(registry: &AnimationRegistry<'_>, player: AnimatedPlayer<'_>) {
    registry.register(player);
    let first = registry.len();
    for entry in registry.in_order() {
        let id = registry.id(entry);
        if id >= first || registry.at(id) != Some(entry) {
            registry.append(entry);
        }
    }
    let count = registry.len();
    // insertion sort of the appended ones, by the game's order of names
    for i in first.saturating_add(1)..count {
        let Some(entry) = registry.at(i) else { break };
        let mut j = i;
        while j > first {
            let Some(before) = registry.at(j - 1) else { break };
            if !registry.less(entry, before) {
                break;
            }
            registry.set_at(j, before);
            j -= 1;
        }
        registry.set_at(j, entry);
    }
    for i in first..count {
        if let Some(entry) = registry.at(i) {
            registry.set_id(entry, i);
        }
    }
    registry.set_actualized();
}
