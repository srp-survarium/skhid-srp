//! Patch 23: a battery under the floor one stands on is taken
//! (client_patch.md section 3.23).
//!
//! The use scan casts its ray from the eye twice as far as the detection
//! distance and walks every hit: a wall or a floor no longer ends the walk
//! but marks it crossed. A usable object the original would take (within
//! the distance and nothing crossed) is taken as before. Otherwise only a
//! battery may be taken, and only one under the player's feet: its centre
//! below the feet and within 1 m across; then the scan measures it from
//! the feet. Anything else is not taken (80h out of the groups).

#![forbid(unsafe_code)]

use crate::vostok::use_scan::{Candidate, Position, ScanFrame, ScanPlayer, detection_distance};
use crate::vostok::{Context, Flow, NoExit, PickupBelowOnCandidateExit, PickupBelowOnSolidExit, Reg};

/// The most across (squared, m^2) a battery under the feet may be.
const ACROSS_SQUARED: f32 = 1.0;
/// A wall or a floor among a hit's collision groups (both bits).
const SOLID: u8 = 0x0A;
/// What a crossing keeps of the groups' low word: the walk's stop (solid,
/// 0Ah) out.
const CROSSED_KEEPS: u16 = !(SOLID as u16);
/// What a refused object keeps of the groups: the game's own refusal's
/// mask (0x2D85D4: usable, 80h, out, and the high word).
const REFUSED_KEEPS: u32 = 0xFF7F;

/// `use_scan.reach`: nothing crossed yet; the ray twice the distance (the
/// fld it replaces, then doubled, on the x87 stack).
pub fn on_reach(ctx: &mut Context<'_>) -> Flow<NoExit> {
    ScanFrame::at(ctx).set_crossed(false);
    ctx.push_st(f64::from(detection_distance()) * 2.0);
    Flow::Skip
}

/// `use_scan.solid`: a wall or a floor marks the walk crossed (and takes
/// its stop out of the groups) instead of ending it.
pub fn on_solid(ctx: &mut Context<'_>) -> Flow<PickupBelowOnSolidExit> {
    let groups = (ctx.reg(Reg::Eax) as u8) & SOLID;
    ctx.set_reg8(Reg::Eax, groups);
    if groups == SOLID {
        let frame = ScanFrame::at(ctx);
        frame.set_crossed(true);
        frame.mask_groups_word(CROSSED_KEEPS);
    }
    Flow::Exit(PickupBelowOnSolidExit::Next)
}

/// `a` above `b`, as the original's `comiss a,b / ja`: never when
/// unordered (a NaN).
fn above(a: f32, b: f32) -> bool {
    a > b
}

/// `a` at most `b`, as `comiss b,a / jae`: never when unordered.
fn at_most(a: f32, b: f32) -> bool {
    a <= b
}

/// The square of the distance from `a` to `b`, summed x, y, z.
fn distance_squared(a: Position, b: Position) -> f32 {
    let (x, y, z) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    x * x + y * y + z * z
}

/// `use_scan.candidate`: a usable object the original takes goes on; a
/// battery under the feet goes on measured from the feet; anything else is
/// refused.
pub fn on_candidate(ctx: &mut Context<'_>) -> Flow<PickupBelowOnCandidateExit> {
    let (Some(player), Some(object)) = (ScanPlayer::at_candidate(ctx), Candidate::at(ctx)) else {
        return Flow::Continue;
    };
    let reach = detection_distance();
    if !above(distance_squared(object.position(), player.eye()), reach * reach) && !ScanFrame::at(ctx).crossed() {
        return Flow::Continue;
    }
    if !object.is_battery() {
        return refuse(ctx);
    }
    let centre = object.transform();
    let feet = player.feet();
    let below = feet[1] - centre[13];
    let (x, z) = (centre[12] - feet[0], centre[14] - feet[2]);
    let across = x * x + z * z;
    if !above(below, 0.0) || !at_most(across, ACROSS_SQUARED) {
        return refuse(ctx);
    }
    object.set_position(feet);
    Flow::Continue
}

fn refuse(ctx: &mut Context<'_>) -> Flow<PickupBelowOnCandidateExit> {
    ScanFrame::at(ctx).mask_groups(REFUSED_KEEPS);
    Flow::Exit(PickupBelowOnCandidateExit::Reject)
}
