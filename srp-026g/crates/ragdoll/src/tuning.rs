//! The look's few numbers, in one place (the owner's tuning round).
//!
//! The powered ragdoll (the owner, 2026-10-26: the Bodycam look, "the
//! legs don't move" in the first try): every joint has a muscle that pulls
//! toward the death clip's pose (the pose at the death without one), as a
//! compliant constraint of `*_STIFFNESS` N m/rad times a strength
//! s0 exp(-t / `*_DECAY`). The legs are strongest and last longest, so they
//! carry the body while the trunk and the arms go; they weaken all the time
//! and bend under the weight more as they do, so the knees buckle by
//! themselves, progressively, and the body falls the way the hit's impulse
//! and its weight take it. No timed phase, no stance held.

/// The legs' muscles (hips, knees): stiffness, N m/rad, and decay, s.
pub const LEG_STIFFNESS: f32 = 500.0;
pub const LEG_DECAY: f32 = 0.5;
/// The waist's.
pub const WAIST_STIFFNESS: f32 = 250.0;
pub const WAIST_DECAY: f32 = 0.25;
/// The neck's.
pub const NECK_STIFFNESS: f32 = 60.0;
pub const NECK_DECAY: f32 = 0.3; // 464c 0.25
/// The arms' (shoulders, elbows).
pub const ARM_STIFFNESS: f32 = 40.0;
pub const ARM_DECAY: f32 = 0.2;
/// A hit limb's side decays this much faster (its decay times this).
pub const HIT_SIDE_DECAY: f32 = 0.55;
/// A hit this strong (N s) takes `HIT_WEAKENING` of every muscle's
/// strength at once; a weaker one, in proportion.
pub const HIT_POWER_FULL: f32 = 60.0;
pub const HIT_WEAKENING: f32 = 0.4;
/// A headshot: limp at once.
pub const HEADSHOT_STRENGTH: f32 = 0.2;
pub const HEADSHOT_DECAY: f32 = 0.07;
/// A blast: a weak body, thrown.
pub const BLAST_STRENGTH: f32 = 0.4;
pub const BLAST_DECAY: f32 = 0.15;
/// A weakening knee's target bends by up to this, rad, as its strength
/// goes from full to nothing.
pub const KNEE_GIVE: f32 = 1.4;
/// A bullet that kills pushes this many times its momentum's push (the
/// knock-back games show; a corpse's shot pushes `1`).
pub const DEATH_KNOCK: f32 = 3.5; // 464c 3.0
/// And this share of it moves the whole body, level, at most
/// `BODY_KNOCK_MAX` m/s.
pub const BODY_SHARE: f32 = 0.6;
pub const BODY_KNOCK_MAX: f32 = 1.6; // 464c 1.2
/// The body's velocity at the death (a running player's) carried into the
/// ragdoll: this share of it, at most `DEATH_VELOCITY_MAX` m/s (464c: none).
pub const DEATH_VELOCITY_SHARE: f32 = 0.7;
pub const DEATH_VELOCITY_MAX: f32 = 7.0;
/// The hits just before the killing one push with this share of theirs.
pub const EARLIER_HIT: f32 = 0.5;
/// A killing hit this strong (N s) or more is a heavy one (a sniper's).
pub const HEAVY_HIT: f32 = 40.0;
/// The equipment a part carries, kg, at most.
pub const MAX_LOAD: f32 = 40.0;

/// A bullet's push, N s: its momentum at the hit (the ammo's `mass` x its
/// speed then) times this (the game's masses are a gameplay scale: 0.02 kg
/// for a 5.45 mm round, 0.1 for a 7.62x54, 0.01 a buckshot pellet).
pub const SHOT_MOMENTUM: f32 = 0.8;
/// A push when the bullet gives no mass or speed, N s (a rifle's).
pub const SHOT_DEFAULT: f32 = 12.0;

// A bullet through a corpse (`shot.rs`): the push on the body part it hits.

/// A push is lifted by this much of its size (a shot from a standing
/// shooter goes down into a body on the ground, which the ground would
/// stop dead: the jolt games give a hit corpse).
pub const SHOT_LIFT: f32 = 0.35;
/// A push's change of a body's speed (m/s) and spin (rad/s) at most.
pub const SHOT_MAX_SPEED: f32 = 6.0;
pub const SHOT_MAX_SPIN: f32 = 15.0;
/// After a push the body is not slowed toward rest for this long, s (the
/// slowing that ends a fall's creep would take a small push away at once).
pub const SHOT_GRACE: f32 = 0.3;
