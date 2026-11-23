//! The world's events as the hud walks them (patch 191,
//! docs/notes/kill-marker.md): nodes of the core's list (+12744h), each
//! made by game_world_core::new_world_event (0x2D43B0) in the simulation
//! and dropped by a world load after its time (0x2D3700).

use super::context::Context;
use super::world::read;
use crate::raw;

/// A world event's type (+18h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// generate_killed_event (0x2DA3A8).
    Kill,
    /// A victory item's (a battery's) user (0x300B4A, 0x301004).
    VictoryItemUser,
    /// generate_hit_event (0x2DA054).
    Hit,
}

impl Kind {
    fn parse(raw: u32) -> Option<Kind> {
        match raw {
            0 => Some(Kind::Kill),
            1 => Some(Kind::VictoryItemUser),
            2 => Some(Kind::Hit),
            _ => None,
        }
    }
}

/// The type of the event on_kill_cross_event just handled
/// (`kill_cross.walked`).
pub fn walked(ctx: &Context<'_>) -> Option<Kind> {
    // SAFETY: the context is this site's: EDI the world event the walker
    // gave the handler, a node of the world's list, alive while it walks.
    unsafe { Kind::parse(read(raw::site::kill_cross_walked::event(ctx)?.kind())) }
}
