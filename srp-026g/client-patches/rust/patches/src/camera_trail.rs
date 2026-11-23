//! Patch 197: the drawn camera and the aim (docs/notes/muzzle-shots.md).
//!
//! The first-person view is the eyes' transform (bone "Item"), which the
//! mouse, the weapon's kick and the shot clips turn; the gun and its sights
//! hang on it, and a shot leaves the gun (patch 200). Here the view drawn is
//! taken apart from that aim, at update_camera's eye (`camera.eye`, after the
//! ragdoll's and the head clearance's). Only the view moves: the eyes'
//! transform, the aim, the gun, the shots and the worlds are the game's.
//! Nothing a match computes, and nothing between the mouse and the gun.
//!
//! **The weapon's kick is the gun's**: of the lift the kick gives the eye the
//! view takes `KICK_SHOWN`, and the gun, on the aim, rises over it and comes
//! back as the kick does.
//!
//! **The gun turns first, the view follows** (the owner, 2026-11-23: "weapons
//! don't have momentum when turn", "bodycam should have perfected the
//! technique"). Bodycam's camera motion, as its reproduction in the STALKER
//! bodycam fork has it (bodycam_reference/camera.cpp, `CameraMotion::Update`):
//! the control (here: where the player's input aims, and nothing else: the
//! weapon's kick, and what the clips add to the eye, the steps' bob, the
//! breath and a shot's wobble, move the view as they did before the follow,
//! the owner: "camera is bobbling", "the weapon also throws the camera") is
//! smoothed; the arms
//! follow the smoothed control quickly and the camera slowly, the faster the
//! faster the turn; a turn rolls the view. There the arms carry the gun behind the
//! control; here the gun stays on the aim, so the mouse moves it at once and
//! a bullet goes where it points, and the view is drawn as far from the gun
//! as that camera is from those arms. So on a slow turn the gun swings ahead
//! over a view that comes after it; on a fast one the view keeps up.
//!
//! Neither while the scope's own view is on (the optic's picture is the
//! screen). The view is never more than `MOST_HIP` off the gun's aim, and
//! `MOST_AIMED` fully aimed.

#![forbid(unsafe_code)]

use ragdoll::math::{Vec3, abs, atan2, clamp, min, sin_cos, sqrt};

use crate::aim_pose;
use crate::vostok::world::PlayerId;
use crate::vostok::{Context, Flow, GameCell, NoExit, camera, clock, shot};

const DEGREE: f32 = core::f32::consts::PI / 180.0;
/// The share of the lift the weapon's kick gives the eye that the view takes;
/// the gun takes all of it.
const KICK_SHOWN: f32 = 0.4;
/// The control is smoothed at this rate (1/s): `controlSmoothSpeed`.
const CONTROL_SMOOTHING: f32 = 25.0;
/// The arms follow the smoothed control at this rate: `kArmsLagSpeed`.
const ARMS_LAG: f32 = 20.0;
/// The camera follows it at a rate between these two (`cameraLagSlowSpeed`,
/// `cameraLagFastSpeed`), over turns of `SLOW_TURN` to `FAST_TURN` (yaw and
/// pitch summed; the reference's rates 1 and 2: a rate is 125 degrees a
/// second), and that rate goes to its target at `CAMERA_LAG_RESPONSE`.
const CAMERA_LAG: (f32, f32) = (2.0, 16.0);
const SLOW_TURN: f32 = 125.0 * DEGREE;
const FAST_TURN: f32 = 250.0 * DEGREE;
const CAMERA_LAG_RESPONSE: f32 = 5.0;
/// A lag's longest step (s): `kMaxLagStepSeconds`.
const LONGEST_LAG_STEP: f32 = 1.0 / 30.0;
/// A longer frame starts the follow anew (s): `kClassicMaxFrameSeconds`.
const LONGEST_FRAME: f32 = 0.4;
/// A turn of `ROLL_TURN` (yaw, a second; the reference's rate 3) rolls the
/// view `TURN_ROLL` (its 8 degrees, at the fork's head roll share of 0.43),
/// against the turn; the roll goes to its target at `ROLL_RATE`.
const TURN_ROLL: f32 = 8.0 * 0.43 * DEGREE;
const ROLL_TURN: f32 = 375.0 * DEGREE;
const ROLL_RATE: f32 = 8.0;
/// The most the view is off the gun's aim, from the hip and fully aimed.
const MOST_HIP: Look = Look { yaw: 10.0 * DEGREE, pitch: 5.0 * DEGREE };
const MOST_AIMED: Look = Look { yaw: 3.0 * DEGREE, pitch: 2.0 * DEGREE };

/// A direction, or a pair of angles about the two axes (rad): `yaw` about the
/// world's up (growing to the right), `pitch` up from the level.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Look {
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

    /// `self` less `of`: a turn.
    fn from(self, of: Look) -> Look {
        Look { yaw: turn(self.yaw - of.yaw), pitch: self.pitch - of.pitch }
    }
}

/// The follow of one player's view, between frames.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rig {
    player: PlayerId,
    /// The control last frame, and smoothed.
    control: Look,
    smooth: Look,
    /// Where the arms and the camera are.
    arms: Look,
    camera: Look,
    /// The camera's lag rate (1/s).
    lag: f32,
    /// The view off the gun's aim, and its roll (its up leaning left, rad).
    off: Look,
    roll: f32,
    /// The counter's milliseconds of that frame (wrapping).
    at: u32,
}

impl Rig {
    /// At rest on `control`.
    fn on(player: PlayerId, control: Look, at: u32) -> Rig {
        Rig { player, control, smooth: control, arms: control, camera: control, lag: CAMERA_LAG.0, off: Look::NONE, roll: 0.0, at }
    }

    /// A frame of `dt` s on, under `control`; the view at most `most` off the
    /// gun. The arms' place of the frame before is the gun's, the camera's of
    /// this frame the view's, as the reference shows them.
    fn stepped(self, control: Look, dt: f32, most: Look, at: u32) -> Rig {
        let turned = control.from(self.control);
        let (yaw_rate, pitch_rate) = (turned.yaw / dt, turned.pitch / dt);
        let smooth = self.smooth.toward(control, clamp(dt * CONTROL_SMOOTHING, 0.0, 1.0));
        let step = min(dt, LONGEST_LAG_STEP);
        let arms = self.arms.toward(smooth, clamp(step * ARMS_LAG, 0.0, 1.0));
        let fast = clamp((abs(yaw_rate) + abs(pitch_rate) - SLOW_TURN) / (FAST_TURN - SLOW_TURN), 0.0, 1.0);
        let lag = self.lag + (CAMERA_LAG.0 + (CAMERA_LAG.1 - CAMERA_LAG.0) * fast - self.lag) * clamp(dt * CAMERA_LAG_RESPONSE, 0.0, 1.0);
        let behind = self.camera.toward(smooth, clamp(step * lag, 0.0, 1.0)).from(self.arms);
        let off = Look { yaw: clamp(behind.yaw, -most.yaw, most.yaw), pitch: clamp(behind.pitch, -most.pitch, most.pitch) };
        let camera = Look { yaw: turn(self.arms.yaw + off.yaw), pitch: self.arms.pitch + off.pitch };
        let leaning = TURN_ROLL * clamp(yaw_rate / ROLL_TURN, -1.0, 1.0);
        let roll = self.roll + (leaning - self.roll) * clamp(dt * ROLL_RATE, 0.0, 1.0);
        Rig { player: self.player, control, smooth, arms, camera, lag, off, roll, at }
    }
}

/// No first-person eye was shown last frame, or one player's follow.
static FOLLOW: GameCell<Option<Rig>> = GameCell::new(None);

/// update_camera's eye (`camera.eye`, after the head clearance's).
pub fn on_eye(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let eye = camera::given_eye(ctx);
    let shown = match camera::eye_player(ctx) {
        Some((player, true)) if !shot::scope_view(player) => Look::of(row(&eye, 2)).map(|aim| (player, aim)),
        _ => None,
    };
    let Some((player, aim)) = shown else {
        FOLLOW.set(thread, None);
        return Flow::Continue;
    };
    let lift = shot::free_kick_lift(player);
    let rest = rested(aim, lift);
    // What the follow follows: the input's aim alone.
    let (yaw, pitch) = shot::input_aim(player);
    let control = Look { yaw: wrapped(yaw), pitch };
    let raised = aim_pose::take_aim_progress(thread);
    let most = most(if raised.is_finite() { clamp(raised, 0.0, 1.0) } else { 0.0 });
    let now = clock::milliseconds(thread);
    let rig = match FOLLOW.get(thread) {
        Some(rig) if rig.player == player.id() => match now.wrapping_sub(rig.at) as f32 / 1000.0 {
            dt if dt > LONGEST_FRAME => Rig::on(player.id(), control, now),
            dt if dt > 0.0 => rig.stepped(control, dt, most, now),
            _ => rig,
        },
        _ => Rig::on(player.id(), control, now),
    };
    FOLLOW.set(thread, Some(rig));
    let look = Look { yaw: turn(rest.yaw + rig.off.yaw), pitch: rest.pitch + rig.off.pitch };
    // On the aim and level (a still gun with no kick): the eye as it is given.
    if look != aim || rig.roll != 0.0 {
        camera::look_from(ctx, rolled(turned(&eye, aim, look), rig.roll));
    }
    Flow::Continue
}

/// Where the view rests under an `aim` whose weapon's kick lifts the eye by
/// `lift` (rad): the aim less the kick's share the view does not take.
fn rested(aim: Look, lift: f32) -> Look {
    Look { yaw: aim.yaw, pitch: aim.pitch - (1.0 - KICK_SHOWN) * lift }
}

/// The most the view is off the gun's aim with the sight `raised` (0 from
/// the hip to 1).
fn most(raised: f32) -> Look {
    Look { yaw: MOST_HIP.yaw + (MOST_AIMED.yaw - MOST_HIP.yaw) * raised, pitch: MOST_HIP.pitch + (MOST_AIMED.pitch - MOST_HIP.pitch) * raised }
}

/// Any angle as a direction's, in -pi..pi (the player's yaw grows with every
/// turn; no remainder: the arena has no libm).
fn wrapped(angle: f32) -> f32 {
    use core::f32::consts::TAU;
    if angle.is_finite() { turn(angle - (angle / TAU) as i32 as f32 * TAU) } else { 0.0 }
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

/// A view rolled about its own forward: its up leaning to its left by `roll`.
fn rolled(view: [f32; 16], roll: f32) -> [f32; 16] {
    let forward = row(&view, 2);
    let mut rolled = view;
    for at in 0..2 {
        let axis = about(row(&view, at), forward, roll);
        rolled[at * 4..at * 4 + 3].copy_from_slice(&[axis.x, axis.y, axis.z]);
    }
    rolled
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

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 60.0;
    const AHEAD: Look = Look { yaw: 0.3, pitch: 0.1 };

    fn rig() -> Rig {
        Rig::on(PlayerId::for_test(1), AHEAD, 0)
    }

    /// The rig after `frames` frames of a turn to the right of `rate` rad/s.
    fn turning(mut rig: Rig, rate: f32, frames: u32) -> Rig {
        for _ in 0..frames {
            let control = Look { yaw: turn(rig.control.yaw + rate * FRAME), pitch: rig.control.pitch };
            rig = rig.stepped(control, FRAME, MOST_HIP, rig.at + 16);
        }
        rig
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
    fn a_still_gun_has_the_view_on_it() {
        let still = turning(rig(), 0.0, 120);
        assert_eq!((still.off, still.roll), (Look::NONE, 0.0));
    }

    #[test]
    fn on_a_slow_turn_the_gun_swings_ahead_of_the_view() {
        // 20 degrees a second to the right: the view falls behind (to the left of the gun)
        let half = turning(rig(), 20.0 * DEGREE, 30);
        assert!(half.off.yaw < -DEGREE && half.off.pitch == 0.0, "{:?}", half.off);
        let second = turning(half, 20.0 * DEGREE, 30);
        assert!(second.off.yaw < half.off.yaw && second.off.yaw > -MOST_HIP.yaw, "{:?}", second.off);
        // and comes up to it once the turn stops
        let stopped = turning(second, 0.0, 60);
        assert!(stopped.off.yaw > second.off.yaw * 0.5 && stopped.off.yaw <= 0.0, "{:?}", stopped.off);
        assert!(turning(stopped, 0.0, 600).off.yaw.abs() < 0.05 * DEGREE);
    }

    #[test]
    fn on_a_fast_turn_the_view_keeps_up() {
        // per degree a second of turn the view is far less behind than on a slow one
        let slow = turning(rig(), 60.0 * DEGREE, 60);
        let fast = turning(rig(), 400.0 * DEGREE, 60);
        assert!(fast.lag > 15.0 && slow.lag == CAMERA_LAG.0, "{} {}", fast.lag, slow.lag);
        assert!(fast.off.yaw.abs() / 400.0 < 0.25 * slow.off.yaw.abs() / 60.0, "{:?} {:?}", fast.off, slow.off);
    }

    #[test]
    fn the_view_is_never_more_than_the_most_off_the_gun() {
        let long = turning(rig(), 100.0 * DEGREE, 240);
        assert!((long.off.yaw + MOST_HIP.yaw).abs() < 1e-5, "{:?}", long.off);
        // the camera is held there, not left further behind: it comes back when the turn ends
        assert!(turning(long, 0.0, 60).off.yaw > 0.5 * long.off.yaw);
        let aimed = most(1.0);
        assert_eq!((most(0.0), aimed), (MOST_HIP, MOST_AIMED));
        let mut rig = rig();
        for _ in 0..240 {
            let control = Look { yaw: rig.control.yaw, pitch: rig.control.pitch + 20.0 * DEGREE * FRAME };
            rig = rig.stepped(control, FRAME, aimed, rig.at + 16);
        }
        assert!((rig.off.pitch + MOST_AIMED.pitch).abs() < 1e-5, "{:?}", rig.off);
    }

    #[test]
    fn a_turn_rolls_the_view_against_it() {
        let right = turning(rig(), 375.0 * DEGREE, 60);
        assert!((right.roll - TURN_ROLL).abs() < 0.01 * DEGREE, "{}", right.roll);
        let left = turning(rig(), -200.0 * DEGREE, 60);
        assert!(left.roll < -0.4 * TURN_ROLL && left.roll > -TURN_ROLL, "{}", left.roll);
        assert!(turning(right, 0.0, 120).roll.abs() < 0.01 * DEGREE);
    }

    #[test]
    fn a_turn_across_the_yaws_seam_is_a_turn() {
        let seam = Rig::on(PlayerId::for_test(1), Look { yaw: core::f32::consts::PI - 0.01, pitch: 0.0 }, 0);
        let over = turning(seam, 60.0 * DEGREE, 30);
        let plain = turning(Rig::on(PlayerId::for_test(1), Look { yaw: 0.0, pitch: 0.0 }, 0), 60.0 * DEGREE, 30);
        assert!((over.off.yaw - plain.off.yaw).abs() < 1e-4, "{:?} {:?}", over.off, plain.off);
    }

    #[test]
    fn a_yaw_of_many_turns_is_its_direction() {
        use core::f32::consts::{PI, TAU};
        for (angle, direction) in [(0.3, 0.3), (TAU + 0.3, 0.3), (-5.0 * TAU - 0.3, -0.3), (PI + 0.1, -PI + 0.1), (9.0 * TAU + PI + 0.1, -PI + 0.1)] {
            assert!((wrapped(angle) - direction).abs() < 1e-4, "{angle}: {}", wrapped(angle));
        }
        assert_eq!(wrapped(f32::NAN), 0.0);
    }

    #[test]
    fn a_look_is_its_direction() {
        let look = Look { yaw: 0.7, pitch: -0.3 };
        let back = Look::of(look.forward()).unwrap();
        assert!((back.yaw - look.yaw).abs() < 1e-5 && (back.pitch - look.pitch).abs() < 1e-5);
        assert_eq!(Look::of(Vec3::new(0.0, 0.0, 0.0)), None);
    }

    #[test]
    fn the_view_is_the_eye_turned_to_the_look() {
        let aim = Look { yaw: 0.4, pitch: 0.2 };
        let forward = aim.forward();
        let right = Vec3::new(0.0, 1.0, 0.0).cross(forward).direction().unwrap();
        let up = forward.cross(right);
        let eye = [right.x, right.y, right.z, 0.0, up.x, up.y, up.z, 0.0, forward.x, forward.y, forward.z, 0.0, 3.0, 2.0, 1.0, 1.0];
        let look = Look { yaw: 0.35, pitch: 0.17 };
        let view = turned(&eye, aim, look);
        assert!((row(&view, 2) - look.forward()).length() < 1e-5);
        assert!(row(&view, 0).y.abs() < 1e-5, "no roll: the right stays level");
        assert_eq!(view[12..], eye[12..]);
        assert_eq!(turned(&eye, aim, aim)[..12].iter().zip(&eye[..12]).filter(|(a, b)| (**a - **b).abs() > 1e-6).count(), 0);
        // rolled: the up leans to the left, the forward stays
        let leant = rolled(view, 3.0 * DEGREE);
        assert!((row(&leant, 2) - row(&view, 2)).length() < 1e-6);
        assert!(row(&leant, 1).dot(row(&view, 0)) < -0.05 && row(&leant, 0).dot(row(&leant, 1)).abs() < 1e-5);
    }
}
