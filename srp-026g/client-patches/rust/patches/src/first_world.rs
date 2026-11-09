//! Patch 19: a game's first world of a match puts its places' inputs and
//! kit choices into their lists in time order, the kit choice the world's
//! (client_patch.md section 3.19; before its Rust port
//! late_join_026g.asm's first_load_input, first_load_kit, entry_at_time).
//!
//! game_world_core::deserialize, at a game's first world of a match (every
//! game's at a match's start, a newcomer's, a game's that comes back), gives
//! each place an input and a kit choice at the world's time. The game's own
//! put the entry at the newest end of the place's list even when it held
//! newer ones (inputs and kit choices relayed between the server's 0xE5 and
//! the world), where every other writer keeps the lists in time order; and
//! its kit choice was the kit in use, not the one the world has chosen for
//! the next spawn. Each tick takes the place's next kit from the newest
//! entry at or before the tick: the newcomer respawned in another kit than
//! the other games. Here the same entries, the kit choice being the world's,
//! go where their time puts them; an entry of the same time is given the
//! value, as the game's own did.

#![forbid(unsafe_code)]

use crate::vostok::timeline::{FirstWorldPlace, TimedEntry, TimedList};
use crate::vostok::{Context, Flow, NoExit};

/// More entries than a place's list can hold is no list.
const MOST: u32 = 1 << 12;

/// The entry of `time`: the one the list has, or a new one put before the
/// oldest entry that is newer (at the newest end when there is none).
fn entry_at_time<'a>(list: &TimedList<'a>, time: u32) -> Option<TimedEntry<'a>> {
    let mut at = list.newest();
    for _ in 0..MOST {
        match at {
            Some(entry) if entry.time() == time => return Some(entry),
            Some(entry) if entry.time() > time => at = entry.older(),
            _ => break,
        }
    }
    let entry = list.allocate(time)?;
    let mut older = list.newest();
    let mut before = None;
    for _ in 0..MOST {
        match older {
            Some(o) if o.time() < time => {
                before = o.newer();
                break;
            }
            Some(o) => older = o.older(),
            None => {
                before = list.oldest();
                break;
            }
        }
    }
    list.insert_before(entry, before);
    Some(entry)
}

/// `first_world.input`: the current player's input at the world's time,
/// instead of the game's call.
pub fn on_input(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(place) = FirstWorldPlace::at_input(ctx) else { return Flow::Continue };
    let (Some(input), Some(list)) = (place.input(), place.inputs()) else { return Flow::Continue };
    match entry_at_time(&list, place.time()) {
        Some(entry) => {
            entry.set_input(input);
            Flow::Skip
        }
        None => Flow::Continue,
    }
}

/// `first_world.kit`: the kit the world has chosen for the place's next
/// spawn, at the world's time, instead of the game's call.
pub fn on_kit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(place) = FirstWorldPlace::at_kit(ctx) else { return Flow::Continue };
    let (Some(kit), Some(list)) = (place.chosen_kit(), place.kit_choices()) else { return Flow::Continue };
    match entry_at_time(&list, place.time()) {
        Some(entry) => {
            entry.set_kit(kit);
            Flow::Skip
        }
        None => Flow::Continue,
    }
}
