//! The look's few numbers, in one place (the owner's tuning round).
//!
//! A body shot (the owner, 2026-10-25: "similar to Bodycam, where the model
//! tries to still stand on its legs when it drops"): the legs hold the body
//! up at full strength for `LEG_HOLD`, then give way over `LEG_FADE` (the
//! knees buckle under the load, then fold); the trunk and arms go limp
//! first (`TRUNK_FADE`, `ARM_FADE`), so the upper body slumps while the
//! legs still try to carry it. The legs drive toward a supporting stance
//! (knees bent `KNEE_BEND`, feet under the body), its feet steered toward
//! the point under the centre of mass by `BALANCE_ASSIST` (0: under the
//! hips, 1: under the centre of mass), fading with the legs.
//!
//! A headshot is limp at once, an explosion throws a weak body: their
//! fades are in `ragdoll::strength`.

use crate::limits::degrees;

/// The legs at full strength after a body shot, s.
pub const LEG_HOLD: f32 = 0.25;
/// Then the legs' fade to limp, s (the whole collapse: LEG_HOLD + LEG_FADE).
pub const LEG_FADE: f32 = 0.8;
/// The waist and the neck fade to limp over this from the death, s.
pub const TRUNK_FADE: f32 = 0.4;
/// The arms fade to limp over this from the death, s.
pub const ARM_FADE: f32 = 0.35;
/// The knees' bend in the supporting stance, rad.
pub const KNEE_BEND: f32 = degrees(15.0);
/// How far the feet's target moves from under the hips toward under the
/// centre of mass (0 to 1).
pub const BALANCE_ASSIST: f32 = 0.8;
/// Half the feet's spread in the stance, m (about the hips' own).
pub const STANCE_HALF_WIDTH: f32 = 0.1;

// A bullet through a corpse (`shot.rs`): the push on the body part it hits.

/// The push of a shot, N s, for a weapon of `SHOT_STOPPING_POWER` (the head
/// of 6.5 kg takes about 1.8 m/s, the torso about 0.7, a forearm the cap;
/// the joints share it out).
pub const SHOT_IMPULSE: f32 = 12.0;
/// A push is lifted by this much of its size (a shot from a standing
/// shooter goes down into a body on the ground, which the ground would
/// stop dead: the jolt games give a hit corpse).
pub const SHOT_LIFT: f32 = 0.35;
/// The stopping power that pushes `SHOT_IMPULSE` (the configs' weapons have
/// 0.1 to 0.8).
pub const SHOT_STOPPING_POWER: f32 = 0.4;
/// The push's scale by stopping power stays within these.
pub const SHOT_SCALE_MIN: f32 = 0.4;
pub const SHOT_SCALE_MAX: f32 = 2.0;
/// A push's change of a body's speed (m/s) and spin (rad/s) at most.
pub const SHOT_MAX_SPEED: f32 = 6.0;
pub const SHOT_MAX_SPIN: f32 = 15.0;
/// After a push the body is not slowed toward rest for this long, s (the
/// slowing that ends a fall's creep would take a small push away at once).
pub const SHOT_GRACE: f32 = 0.3;
