//! Patch 197: the view and the gun apart (docs/notes/muzzle-shots.md).
//!
//! The first-person view is the eyes' transform (bone "Item"), which the
//! mouse, the weapon's kick and the shot clips turn; the gun and its sights
//! hang on it, and a shot leaves the gun (patch 200). Two things are drawn
//! apart from that here. Drawing only: the eyes' transform, the aim, the
//! shots and the worlds are the game's. Nothing a match computes.
//!
//! **The weapon's kick is the gun's** (`camera.eye`, update_camera's eye
//! after the ragdoll's and the head clearance's): of the lift the kick gives
//! the eye the view takes `KICK_SHOWN`, and the gun, on the aim, rises over
//! it and comes back as the kick does. Not while the scope's own view is on.
//!
//! **The hip shake is the gun's** (both sites): camerashake_v2, the recoil's
//! third clip, turns the eye and not the gun, and the game's view shook with
//! it. Patch 200 puts it on the gun for the shot; here the view is drawn
//! without it (the eye in the frame of the eye mixed without the shake,
//! `muzzle_shots::shake_of`) and the gun held from the hip with it, so the
//! screen is still and the gun shakes where the shot goes.
//!
//! **The gun has weight on a turn** (`hands_ik.solve`, the first person's
//! hands before their IK): the view stays on the mouse, turn for turn; the
//! gun held from the hip is drawn behind the turn and comes up to the view
//! when the turn ends. Its lag is the arms' of Bodycam's camera motion as the
//! STALKER bodycam fork reproduces it (bodycam_reference/camera.cpp): the
//! eye's way of each frame (not the player's yaw and look pitch, which the
//! simulation moves a step at a time: a gun after them moves in jerks, the
//! owner: "weapons lost smooth movement") smoothed at `CONTROL_SMOOTHING`,
//! the arms after it at `ARMS_LAG`; `SWAY` of it is shown, at most `MOST`, and less the further
//! the sight is raised (none fully aimed: the sights stay true).
//!
//! The owner's plays, 2026-11-23: "weapons don't have momentum when turn",
//! then, of a view that followed the gun as that reference's camera does:
//! "camera is bobbling", "the camera is being flung w/weapon",
//! "uncomfortably". So the view never lags the mouse. The gun drawn behind a
//! turn is not where a shot goes while it trails: the shot is the aim's (the
//! HUD's crosshair shows it). A gun that trailed in the simulation would be
//! a state every game has to agree on.

#![forbid(unsafe_code)]

use ragdoll::math::{Vec3, atan2, clamp, min, sin_cos, sqrt};

use crate::affine::{inverse, product};
use crate::muzzle_shots::{self, Shake};
use crate::vostok::aim::DrawnHands;
use crate::vostok::world::PlayerId;
use crate::vostok::{Context, Flow, Lock, NoExit, camera, clock, shot};

const DEGREE: f32 = core::f32::consts::PI / 180.0;
/// The share of the lift the weapon's kick gives the eye that the view takes;
/// the gun takes all of it.
const KICK_SHOWN: f32 = 0.4;
/// The eye's way is smoothed at this rate (1/s): `controlSmoothSpeed`.
const CONTROL_SMOOTHING: f32 = 25.0;
/// The arms follow the smoothed aim at this rate: `kArmsLagSpeed`.
const ARMS_LAG: f32 = 20.0;
/// A lag's longest step (s): `kMaxLagStepSeconds`.
const LONGEST_LAG_STEP: f32 = 1.0 / 30.0;
/// A longer frame starts the gun at rest (s): `kClassicMaxFrameSeconds`.
const LONGEST_FRAME: f32 = 0.4;
/// The share of the arms' lag the gun is drawn behind the view by.
const SWAY: f32 = 0.5;
/// The most the gun is drawn behind the view.
const MOST: Look = Look { yaw: 5.0 * DEGREE, pitch: 3.5 * DEGREE };

/// A direction, or a pair of angles about the two axes (rad): `yaw` about the
/// world's up (growing to the right), `pitch` up from the level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    yaw: f32,
    pitch: f32,
}

impl Look {
    const NONE: Look = Look { yaw: 0.0, pitch: 0.0 };

    /// The direction `forward` looks; none for no direction.
    fn of(forward: Vec3) -> Option<Look> {
        let forward = forward.direction()?;
        let level = sqrt(forward.x * forward.x + forward.z * forward.z);
        Some(Look { yaw: atan2(forward.x, forward.z), pitch: atan2(forward.y, level) })
    }

    #[cfg(test)]
    fn forward(self) -> Vec3 {
        let ((sin_yaw, cos_yaw), (sin_pitch, cos_pitch)) = (sin_cos(self.yaw), sin_cos(self.pitch));
        Vec3::new(sin_yaw * cos_pitch, sin_pitch, cos_yaw * cos_pitch)
    }

    /// From `self` the share `by` of the way to `to`.
    fn toward(self, to: Look, by: f32) -> Look {
        Look { yaw: turn(self.yaw + turn(to.yaw - self.yaw) * by), pitch: self.pitch + (to.pitch - self.pitch) * by }
    }
}

/// One player's arms behind its eye's way, between frames.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Arms {
    player: PlayerId,
    /// The eye's way, smoothed, and where the arms are.
    control: Look,
    smooth: Look,
    arms: Look,
    /// The performance counter's ticks at that frame.
    at: u64,
}

impl Arms {
    /// At rest on `control`.
    fn on(player: PlayerId, control: Look, at: u64) -> Arms {
        Arms { player, control, smooth: control, arms: control, at }
    }

    /// A frame of `dt` s on, under `control`.
    fn stepped(self, control: Look, dt: f32, at: u64) -> Arms {
        let smooth = self.smooth.toward(control, clamp(dt * CONTROL_SMOOTHING, 0.0, 1.0));
        let arms = self.arms.toward(smooth, clamp(min(dt, LONGEST_LAG_STEP) * ARMS_LAG, 0.0, 1.0));
        Arms { player: self.player, control, smooth, arms, at }
    }

    /// How far behind the view the gun is drawn, with the sight `raised` (0
    /// from the hip to 1 fully aimed: none then).
    fn behind(&self, raised: f32) -> Look {
        let share = SWAY * (1.0 - if raised.is_finite() { clamp(raised, 0.0, 1.0) } else { 1.0 });
        Look {
            yaw: clamp(turn(self.arms.yaw - self.control.yaw) * share, -MOST.yaw, MOST.yaw),
            pitch: clamp((self.arms.pitch - self.control.pitch) * share, -MOST.pitch, MOST.pitch),
        }
    }
}

/// No first-person eye was shown last frame, or its player's arms: written at
/// the frame's camera, read by its bone pass.
static ARMS: Lock<Option<Arms>> = Lock::new(None);

/// update_camera's eye (`camera.eye`, after the head clearance's).
pub fn on_eye(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some((player, true)) = camera::eye_player(ctx) else {
        ARMS.with(thread, |arms| *arms = None);
        return Flow::Continue;
    };
    // The view without the hip shake (the gun has it).
    let given = camera::given_eye(ctx);
    let still = if shot::scope_view(player) { None } else { muzzle_shots::shake_of(thread, player.id()).and_then(|shake| calmed(&given, &shake)) };
    let eye = still.unwrap_or(given);
    let Some(aim) = Look::of(row(&eye, 2)) else {
        ARMS.with(thread, |arms| *arms = None);
        return Flow::Continue;
    };
    // The arms: after the eye's way of this frame.
    let (now, second) = (clock::counter(thread), clock::frequency(thread).max(1) as f32);
    ARMS.with(thread, |arms| {
        *arms = Some(match *arms {
            Some(last) if last.player == player.id() => match now.wrapping_sub(last.at) as f32 / second {
                dt if dt > LONGEST_FRAME => Arms::on(player.id(), aim, now),
                dt if dt > 0.0 => last.stepped(aim, dt, now),
                _ => last,
            },
            _ => Arms::on(player.id(), aim, now),
        });
    });
    // The view: the eye less the kick's share it does not take.
    let lift = shot::free_kick_lift(player);
    if lift != 0.0 {
        camera::look_from(ctx, turned(&eye, aim, rested(aim, lift)));
    } else if still.is_some() {
        camera::look_from(ctx, eye);
    }
    Flow::Continue
}

/// The first person's hands before their IK (`hands_ik.solve`): the gun drawn
/// behind the view's turn.
pub fn on_solve(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut hands) = DrawnHands::at_solve(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let behind = ARMS.with(thread, |arms| arms.filter(|arms| arms.player == hands.who()).map(|arms| arms.behind(hands.raised()))).unwrap_or(Look::NONE);
    let shake = muzzle_shots::shake_of(thread, hands.who());
    if (behind != Look::NONE || shake.is_some())
        && let (Some(gun), Some(eye)) = (hands.bone("Weapon"), hands.bone("Item"))
    {
        let shaken = shake.and_then(|shake| shaken(&gun, &shake, hands.raised())).unwrap_or(gun);
        hands.set_bone("Weapon", swung(&shaken, &eye, behind));
    }
    Flow::Continue
}

/// The view's eye (in the world) without the hip shake: the eye in the frame
/// of the eye mixed without it. None for a shake that flattens space.
fn calmed(eye: &[f32; 16], shake: &Shake) -> Option<[f32; 16]> {
    Some(product(&product(&shake.without, &inverse(&shake.with)?), eye))
}

/// The gun (its holder's space) with the hip shake, as far as it is held
/// from the hip: fully aimed it is laid on the eye, which has the shake.
fn shaken(gun: &[f32; 16], shake: &Shake, raised: f32) -> Option<[f32; 16]> {
    let with = product(&product(gun, &inverse(&shake.without)?), &shake.with);
    let hip = 1.0 - if raised.is_finite() { clamp(raised, 0.0, 1.0) } else { 1.0 };
    Some(muzzle_shots::blend(gun, &with, hip))
}

/// The gun (a row-vector 4x4 in its holder's space) swung about the eye's
/// place by `behind`: about the up by its yaw (to the right), about the eye's
/// right by its pitch (up).
pub fn swung(gun: &[f32; 16], eye: &[f32; 16], behind: Look) -> [f32; 16] {
    let (pivot, right) = (place(eye), row(eye, 0));
    let swing = |v: Vec3| about(about(v, Vec3::new(0.0, 1.0, 0.0), behind.yaw), right, -behind.pitch);
    let mut swung = *gun;
    for at in 0..3 {
        let axis = swing(row(gun, at));
        swung[at * 4..at * 4 + 3].copy_from_slice(&[axis.x, axis.y, axis.z]);
    }
    let at = pivot + swing(place(gun) - pivot);
    swung[12..15].copy_from_slice(&[at.x, at.y, at.z]);
    swung
}

/// Where the view rests under an `aim` whose weapon's kick lifts the eye by
/// `lift` (rad): the aim less the kick's share the view does not take.
fn rested(aim: Look, lift: f32) -> Look {
    Look { yaw: aim.yaw, pitch: aim.pitch - (1.0 - KICK_SHOWN) * lift }
}

/// An angle between two directions' angles, in -pi..pi.
fn turn(angle: f32) -> f32 {
    use core::f32::consts::{PI, TAU};
    if angle > PI {
        angle - TAU
    } else if angle < -PI {
        angle + TAU
    } else {
        angle
    }
}

/// The eye's transform turned from the `aim` to `look`: about the world's up
/// by the yaws' difference, then about the level axis across `look` by the
/// pitches'. Its place and its roll are the eye's.
fn turned(eye: &[f32; 16], aim: Look, look: Look) -> [f32; 16] {
    let (sin_yaw, cos_yaw) = sin_cos(look.yaw);
    let across = Vec3::new(-cos_yaw, 0.0, sin_yaw);
    let mut view = *eye;
    for at in 0..3 {
        let axis = about(about(row(eye, at), Vec3::new(0.0, 1.0, 0.0), turn(look.yaw - aim.yaw)), across, look.pitch - aim.pitch);
        view[at * 4..at * 4 + 3].copy_from_slice(&[axis.x, axis.y, axis.z]);
    }
    view
}

/// `v` turned about the unit `axis` by `angle` (right-handed).
fn about(v: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let (sin, cos) = sin_cos(angle);
    v * cos + axis.cross(v) * sin + axis * (axis.dot(v) * (1.0 - cos))
}

/// A row-vector 4x4's row (0 right, 1 up, 2 forward).
fn row(m: &[f32; 16], at: usize) -> Vec3 {
    Vec3::new(m[at * 4], m[at * 4 + 1], m[at * 4 + 2])
}

/// A row-vector 4x4's place.
fn place(m: &[f32; 16]) -> Vec3 {
    Vec3::new(m[12], m[13], m[14])
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 60.0;
    const AHEAD: Look = Look { yaw: 0.3, pitch: 0.1 };

    fn arms() -> Arms {
        Arms::on(PlayerId::for_test(1), AHEAD, 0)
    }

    /// The arms after `frames` frames of a turn to the right of `rate` rad/s.
    fn turning(mut arms: Arms, rate: f32, frames: u32) -> Arms {
        for _ in 0..frames {
            let control = Look { yaw: turn(arms.control.yaw + rate * FRAME), pitch: arms.control.pitch };
            arms = arms.stepped(control, FRAME, arms.at + 16);
        }
        arms
    }

    #[test]
    fn the_view_takes_a_share_of_the_kicks_lift() {
        let aim = Look { yaw: 0.3, pitch: 10.0 * DEGREE };
        assert_eq!(rested(aim, 0.0), aim);
        let rest = rested(aim, 5.0 * DEGREE);
        assert_eq!(rest.yaw, aim.yaw);
        assert!((rest.pitch - (10.0 - 5.0 * (1.0 - KICK_SHOWN)) * DEGREE).abs() < 1e-6);
    }

    #[test]
    fn a_still_gun_is_on_the_view() {
        assert_eq!(turning(arms(), 0.0, 120).behind(0.0), Look::NONE);
    }

    #[test]
    fn on_a_turn_the_gun_trails_the_view_and_comes_up_to_it_after() {
        // 60 degrees a second to the right: behind, to the left; steady after a moment
        let turn = turning(arms(), 60.0 * DEGREE, 60);
        let behind = turn.behind(0.0);
        assert!(behind.yaw < -1.5 * DEGREE && behind.yaw > -MOST.yaw && behind.pitch == 0.0, "{behind:?}");
        assert!((turning(turn, 60.0 * DEGREE, 30).behind(0.0).yaw - behind.yaw).abs() < 0.05 * DEGREE);
        // half of the arms' own lag (at 60 frames a second, 57 ms of the turn)
        assert!((behind.yaw + SWAY * 60.0 * DEGREE * 0.057).abs() < 0.1 * DEGREE, "{behind:?}");
        let stopped = turning(turn, 0.0, 30);
        assert!(stopped.behind(0.0).yaw.abs() < 0.01 * DEGREE, "{:?}", stopped.behind(0.0));
    }

    #[test]
    fn the_gun_is_never_more_than_the_most_behind_and_not_at_all_fully_aimed() {
        let fast = turning(arms(), 400.0 * DEGREE, 30);
        assert_eq!(fast.behind(0.0).yaw, -MOST.yaw);
        assert_eq!(fast.behind(1.0), Look::NONE);
        assert_eq!(fast.behind(f32::NAN), Look::NONE);
        assert!((fast.behind(0.5).yaw + MOST.yaw).abs() < 1e-6, "still past the most at half the share");
        let slow = turning(arms(), 30.0 * DEGREE, 60);
        assert!((slow.behind(0.5).yaw - 0.5 * slow.behind(0.0).yaw).abs() < 1e-6);
    }

    #[test]
    fn a_turn_across_the_yaws_seam_is_a_turn() {
        let seam = turning(Arms::on(PlayerId::for_test(1), Look { yaw: core::f32::consts::PI - 0.01, pitch: 0.0 }, 0), 60.0 * DEGREE, 30);
        let plain = turning(Arms::on(PlayerId::for_test(1), Look::NONE, 0), 60.0 * DEGREE, 30);
        assert!((seam.behind(0.0).yaw - plain.behind(0.0).yaw).abs() < 1e-4);
    }

    #[test]
    fn the_hip_shake_goes_from_the_view_to_the_gun() {
        // the eye mixed without the shake, and with it: turned a little about the up
        let (sine, cosine) = (0.015f32, (1.0f32 - 0.015 * 0.015).sqrt());
        let without = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 1.6, 0.1, 1.];
        let with = [cosine, 0., -sine, 0., 0., 1., 0., 0., sine, 0., cosine, 0., 0., 1.6, 0.1, 1.];
        let shake = Shake { player: PlayerId::for_test(1), without, with };
        // the view: a world's eye made of the shaken eye is the unshaken one's again
        let world = [0., 0., 1., 0., 0., 1., 0., 0., -1., 0., 0., 0., 10., 0., 20., 1.];
        let still = calmed(&product(&with, &world), &shake).unwrap();
        assert!(still.iter().zip(&product(&without, &world)).all(|(a, b)| (a - b).abs() < 1e-5), "{still:?}");
        // the gun from the hip: turned as the eye was; fully aimed: as it is
        let gun = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0.1, 1.5, 0.5, 1.];
        let hip = shaken(&gun, &shake, 0.0).unwrap();
        assert!((row(&hip, 2).x - sine).abs() < 1e-5 && (row(&hip, 2).length() - 1.0).abs() < 1e-5, "{:?}", row(&hip, 2));
        assert_eq!(shaken(&gun, &shake, 1.0).unwrap(), gun);
        assert_eq!(shaken(&gun, &shake, f32::NAN).unwrap(), gun);
    }

    #[test]
    fn the_gun_swings_about_the_eye() {
        // an eye at (0, 1.6, 0) looking along z; a gun 0.5 ahead of it and 0.1 under
        let eye = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 1.6, 0., 1.];
        let gun = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 1.5, 0.5, 1.];
        // 5 degrees behind a turn to the right: to the left
        let left = swung(&gun, &eye, Look { yaw: -5.0 * DEGREE, pitch: 0.0 });
        assert!(row(&left, 2).x < -0.08 && (row(&left, 2).length() - 1.0).abs() < 1e-6 && row(&left, 1) == Vec3::new(0.0, 1.0, 0.0));
        assert!(place(&left).x < -0.04 && (place(&left).y - 1.5).abs() < 1e-6, "{:?}", place(&left));
        // 3 degrees under a turn upward: down
        let low = swung(&gun, &eye, Look { yaw: 0.0, pitch: -3.0 * DEGREE });
        assert!(row(&low, 2).y < -0.05 && place(&low).y < 1.5 - 0.02, "{:?}", place(&low));
        assert_eq!(swung(&gun, &eye, Look::NONE), gun);
    }

    #[test]
    fn the_view_is_the_eye_turned_to_the_look() {
        let aim = Look { yaw: 0.4, pitch: 0.2 };
        let forward = aim.forward();
        let right = Vec3::new(0.0, 1.0, 0.0).cross(forward).direction().unwrap();
        let up = forward.cross(right);
        let eye = [right.x, right.y, right.z, 0.0, up.x, up.y, up.z, 0.0, forward.x, forward.y, forward.z, 0.0, 3.0, 2.0, 1.0, 1.0];
        let look = Look { yaw: 0.4, pitch: 0.17 };
        let view = turned(&eye, aim, look);
        assert!((row(&view, 2) - look.forward()).length() < 1e-5);
        assert!(row(&view, 0).y.abs() < 1e-5, "no roll: the right stays level");
        assert_eq!(view[12..], eye[12..]);
        let back = Look::of(look.forward()).unwrap();
        assert!((back.yaw - look.yaw).abs() < 1e-5 && (back.pitch - look.pitch).abs() < 1e-5);
        assert_eq!(Look::of(Vec3::new(0.0, 0.0, 0.0)), None);
    }
}
