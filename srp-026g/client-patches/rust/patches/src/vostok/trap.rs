//! A hit a booby trap takes, at `trap.hit`: booby_trap_core::hit's entry
//! (0x305540, the slot bullet::try_pierce calls on what its ray met). The
//! game defuses the trap when the hit's type is its set's defuse_by_hit
//! (ranged in the one trap's data) and drops any other. Every game runs it
//! for every bullet of its simulation, so what is decided there is decided
//! alike everywhere (docs/notes/trap-stock-hit.md). The field is
//! fields_026g.tsv's.

use super::context::Context;
use super::drug::HitType;
use super::world::read;
use crate::raw;

/// The type of the hit at `trap.hit`; None for one the game has no name for.
pub fn hit_kind(ctx: &Context<'_>) -> Option<HitType> {
    // SAFETY: the context is this site's: hit's entry, its arguments above
    // the return address at ESP, on the thread running the hit.
    let kind = unsafe { read(raw::site::trap_hit::frame(ctx)?.kind()) };
    HitType::parse(kind)
}
