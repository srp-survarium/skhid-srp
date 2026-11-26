//! Patch 197: the view and the gun apart (docs/notes/muzzle-shots.md).
//!
//! In the game the mouse turns the player, and the eye, the hands and the gun
//! turn as one; a shot leaves the gun (patch 200). Here the view and the gun
//! come apart, the gun staying what the simulation has, so the gun drawn, the
//! HUD's crosshair and the shot are one line always.
//!
//! **The gun has weight on a turn** (`input_builder.end`, the frame's input
//! built, the mouse's turn of the frame in it): the turn is not
//! given to the player at once. It is kept as pending, and each frame a share
//! of what is pending goes into the input, the larger the more is pending
//! (`RATE`, `EASY`) and the further the sight is raised (`AIMED`). So the
//! player, its eye, its gun and its shots come after the mouse; every game
//! sees the same inputs, as ever. The view is drawn ahead of the eye by what
//! is still pending (`camera.eye`), so the view is the mouse's, turn for
//! turn, and the gun swings behind it and comes up to it: Bodycam's arms
//! behind its control, with the camera on the control (the owner, of a view
//! that lagged: "the camera is being flung w/weapon"; of a gun only drawn
//! behind: "weapon and cross are not aligned", "the weapon model visible and
//! internal are misaligned"; of its range: "the gun should have way more
//! freedom of motion", "the best one is in bodycam").
//!
//! **The weapon's kick is the gun's** (`camera.eye`): of the lift the kick
//! gives the eye the view takes `KICK_SHOWN`.
//!
//! **The hip shake is the gun's** (`camera.eye`, `hands_ik.solve`):
//! camerashake_v2 turns the eye and not the gun; patch 200 puts it on the gun
//! for the shot, and here the view is drawn without it and the gun held from
//! the hip with it (`muzzle_shots::shake_of`).
//!
//! None of the three while the scope's own view is on (the optic's picture is
//! the screen).
//!
//! `SKHID_SWAY_LOG` set in the game's environment (read at the first frame)
//! writes a line a frame to the patches' log, `LOGGED` frames: the frame's
//! length, the turn taken from the mouse, what is pending after it. Nothing
//! without it.

#![forbid(unsafe_code)]

use ragdoll::math::{Vec3, atan2, clamp, max, min, sin_cos, sqrt};

use crate::affine::{inverse, product};
use crate::muzzle_shots::{self, Shake};
use crate::vostok::aim::DrawnHands;
use crate::vostok::log::{self, Piece::{self, Dec, Text}};
use crate::vostok::mouse::FrameTurn;
use crate::vostok::{Context, Flow, Lock, NoExit, camera, clock, renderer, shot};

const DEGREE: f32 = core::f32::consts::PI / 180.0;
/// The share of the lift the weapon's kick gives the eye that the view takes;
/// the gun takes all of it.
const KICK_SHOWN: f32 = 0.4;
/// What a unit of the look pitch turns the eye by, up and down (the look
/// clip, `vostok::mouse::View`).
const UNIT_UP: f32 = 60.0 * DEGREE;
const UNIT_DOWN: f32 = 67.5 * DEGREE;
/// The share of a small pending turn the gun takes in a second (1/s).
const RATE: f32 = 7.0;
/// The pending turn at which the gun takes it twice as fast: its rate is
/// `RATE` times 1 + (pending / `EASY`) squared, so the gun is about 5 degrees
/// behind a turn of 40 degrees a second, 10 behind 115, 17 behind 350 (the
/// range of the Bodycam fork's arms), and there is no stop to stay at.
const EASY: f32 = 12.0 * DEGREE;
/// Fully aimed the gun takes the turn this many times faster again.
const AIMED: f32 = 5.0;
/// The most ever pending: more goes into the input at once.
const MOST: f32 = 30.0 * DEGREE;
/// A longer frame gives the gun all that is pending, and a turn no frame took
/// for as long is no longer shown (the menu is up, the input is another's) (s).
const LONGEST_FRAME: f32 = 0.25;

/// A direction, or a pair of angles about the two axes (rad): `yaw` about the
/// world's up (growing to the right), `pitch` up from the level.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Look {
    yaw: f32,
    pitch: f32,
}

impl Look {
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
}

/// A turn as the input has it: the yaw in radians (growing to the left, as
/// the player's own) and the pitch in the look pitch's units (up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turn {
    pub yaw: f32,
    pub pitch: f32,
}

impl Turn {
    const NONE: Turn = Turn { yaw: 0.0, pitch: 0.0 };

    /// How far it turns the eye (rad).
    fn angle(self) -> f32 {
        sqrt(self.yaw * self.yaw + self.pitch * UNIT_UP * self.pitch * UNIT_UP)
    }

    fn plus(self, other: Turn) -> Turn {
        Turn { yaw: self.yaw + other.yaw, pitch: self.pitch + other.pitch }
    }

    fn less(self, other: Turn) -> Turn {
        Turn { yaw: self.yaw - other.yaw, pitch: self.pitch - other.pitch }
    }

    /// With no more pitch than a look pitch of `look` has left to its ends
    /// (one unit up, one down): the game drops the rest.
    fn within(self, look: f32) -> Turn {
        if !look.is_finite() {
            return self;
        }
        Turn { yaw: self.yaw, pitch: clamp(self.pitch, min(-1.0 - look, 0.0), max(1.0 - look, 0.0)) }
    }

    fn times(self, share: f32) -> Turn {
        Turn { yaw: self.yaw * share, pitch: self.pitch * share }
    }

    /// As the handler keeps it: two half floats.
    fn halves(self) -> (u16, u16) {
        (half_of(self.yaw), half_of(self.pitch))
    }

    fn of_halves((yaw, pitch): (u16, u16)) -> Turn {
        Turn { yaw: of_half(yaw), pitch: of_half(pitch) }
    }
}

/// The mouse's turn the gun has not taken yet, and when the last frame's
/// turn was taken (the performance counter's ticks; none before the first).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Weight {
    pending: Turn,
    at: Option<u64>,
}

/// Written where the game takes the frame's turn, read by the frame's camera.
static WEIGHT: Lock<Weight> = Lock::new(Weight { pending: Turn::NONE, at: None });

/// What of `pending` the gun takes in a frame of `dt` s with the sight
/// `raised`: all of it in a frame too long or with no length, and whatever is
/// over `MOST`.
fn taken(pending: Turn, dt: f32, raised: f32) -> Turn {
    let angle = pending.angle();
    if !(dt > 0.0 && dt <= LONGEST_FRAME) || !angle.is_finite() || angle == 0.0 {
        return pending;
    }
    let raised = if raised.is_finite() { clamp(raised, 0.0, 1.0) } else { 1.0 };
    let rate = RATE * (1.0 + (angle / EASY) * (angle / EASY)) * (1.0 + AIMED * raised);
    let share = clamp(rate * dt, 0.0, 1.0);
    let left = angle * (1.0 - share);
    pending.times(if left > MOST { 1.0 - MOST / angle } else { share })
}

/// The frames `SKHID_SWAY_LOG` logs.
const LOGGED: u32 = 30_000;

/// Whether the frames are logged, and how many are left to.
#[derive(Clone, Copy, Debug, PartialEq)]
enum SwayLog {
    Unread,
    Off,
    On { left: u32 },
}

static SWAY_LOG: Lock<SwayLog> = Lock::new(SwayLog::Unread);

/// A frame's line: its length, the turn the gun took and what is pending.
fn say(dt: f32, took: Turn, pending: Turn) {
    let signed = |value: f32| -> [Piece<'static>; 2] { [Text(if value < 0.0 { "-" } else { "" }), Dec((value.abs() + 0.5) as u32)] };
    let milli = |angle: f32| signed(angle / DEGREE * 1000.0);
    let [yaw, pitch, left, low] = [milli(took.yaw), milli(took.pitch * UNIT_UP), milli(pending.yaw), milli(pending.pitch * UNIT_UP)];
    log::line(&[Text("camera_trail: frame "), Dec((dt * 1e6) as u32), Text(" us, the gun took "), yaw[0], yaw[1], Text(" "), pitch[0], pitch[1],
                Text(" mdeg, pending "), left[0], left[1], Text(" "), low[0], low[1], Text(" mdeg")]);
}

/// The frame's input built (`input_builder.end`): the gun's share of what is
/// pending goes into its turn, the rest stays pending.
pub fn on_turn(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let ctx = &*ctx;
    let thread = ctx.thread();
    let Some(mut turn) = FrameTurn::at(ctx) else { return Flow::Continue };
    let frame = Turn::of_halves(turn.halves());
    // A player who is not there or not alive, and the scope's own view: the turn as the game has it, with all that was pending.
    let Some(holder) = turn.holder() else {
        let all = WEIGHT.with(thread, |weight| core::mem::replace(weight, Weight { pending: Turn::NONE, at: None }).pending.plus(frame));
        if all != frame {
            turn.set(all.halves());
        }
        return Flow::Continue;
    };
    let now = clock::counter(thread);
    let second = clock::frequency(thread).max(1) as f32;
    let (dt, took, pending) = WEIGHT.with(thread, |weight| {
        let dt = weight.at.map_or(0.0, |at| now.wrapping_sub(at) as f32 / second);
        let all = weight.pending.plus(frame).within(holder.look_pitch);
        let took = Turn::of_halves(taken(all, dt, holder.raised).halves());
        let pending = all.less(took);
        *weight = Weight { pending, at: Some(now) };
        (dt, took, pending)
    });
    if took != frame {
        turn.set(took.halves());
    }
    let logged = SWAY_LOG.with(thread, |state| {
        if *state == SwayLog::Unread {
            *state = if renderer::environment_set(thread, c"SKHID_SWAY_LOG") { SwayLog::On { left: LOGGED } } else { SwayLog::Off };
        }
        match *state {
            SwayLog::On { left } if left > 0 => {
                *state = SwayLog::On { left: left - 1 };
                true
            }
            _ => false,
        }
    });
    if logged {
        say(dt, took, pending);
    }
    Flow::Continue
}

/// update_camera's eye (`camera.eye`, after the head clearance's).
pub fn on_eye(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    // The first person's, also when the head clearance has already given the view an eye of its own.
    let Some(player) = camera::first_person(ctx) else { return Flow::Continue };
    if shot::scope_view(player) {
        return Flow::Continue;
    }
    // The view without the hip shake (the gun has it).
    let given = camera::given_eye(ctx);
    let still = muzzle_shots::shake_of(thread, player.id()).and_then(|shake| calmed(&given, &shake));
    let eye = still.unwrap_or(given);
    let Some(aim) = Look::of(row(&eye, 2)) else { return Flow::Continue };
    // The view: ahead of the eye by the turn the gun has not taken, less the kick's share it does not take.
    let now = clock::counter(thread);
    let second = clock::frequency(thread).max(1) as f32;
    let pending = WEIGHT.with(thread, |weight| {
        if weight.at.is_none_or(|at| now.wrapping_sub(at) as f32 / second > LONGEST_FRAME) {
            weight.pending = Turn::NONE;
        }
        weight.pending
    });
    let look = ahead(rested(aim, shot::free_kick_lift(player)), pending);
    if look != aim {
        camera::look_from(ctx, turned(&eye, aim, look));
    } else if still.is_some() {
        camera::look_from(ctx, eye);
    }
    Flow::Continue
}

/// The first person's hands before their IK (`hands_ik.solve`): the gun held
/// from the hip drawn with the hip shake.
pub fn on_solve(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut hands) = DrawnHands::at_solve(ctx) else { return Flow::Continue };
    if let Some(shake) = muzzle_shots::shake_of(ctx.thread(), hands.who())
        && let Some(gun) = hands.bone("Weapon")
        && let Some(shaken) = shaken(&gun, &shake, hands.raised())
    {
        hands.set_bone("Weapon", shaken);
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

/// Where the view rests under an `aim` whose weapon's kick lifts the eye by
/// `lift` (rad): the aim less the kick's share the view does not take.
fn rested(aim: Look, lift: f32) -> Look {
    Look { yaw: aim.yaw, pitch: aim.pitch - (1.0 - KICK_SHOWN) * lift }
}

/// A look turned on by a turn the input has not taken: the input's yaw grows
/// to the left, its pitch turns by the unit of the look's side; a view never
/// looks past straight up or down.
fn ahead(look: Look, pending: Turn) -> Look {
    const STEEPEST: f32 = 89.0 * DEGREE;
    let unit = if look.pitch < 0.0 { UNIT_DOWN } else { UNIT_UP };
    Look { yaw: turn(look.yaw - pending.yaw), pitch: clamp(look.pitch + pending.pitch * unit, -STEEPEST, STEEPEST) }
}

/// A half float's value (IEEE 754 binary16, as the handler keeps a turn).
fn of_half(half: u16) -> f32 {
    let (sign, exponent, mantissa) = (u32::from(half >> 15), u32::from(half >> 10 & 0x1F), u32::from(half & 0x3FF));
    let bits = match (exponent, mantissa) {
        (0, 0) => 0,
        (0, _) => {
            let shift = mantissa.leading_zeros() - 21;
            (113 - shift) << 23 | (mantissa << shift & 0x3FF) << 13
        }
        (31, _) => 0x7F80_0000 | mantissa << 13,
        _ => (exponent + 112) << 23 | mantissa << 13,
    };
    f32::from_bits(sign << 31 | bits)
}

/// The half float nearest a value (ties to the even one; past its range the
/// infinity).
fn half_of(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = (bits >> 16 & 0x8000) as u16;
    let (exponent, mantissa) = ((bits >> 23 & 0xFF) as i32 - 112, bits & 0x7F_FFFF);
    if exponent >= 31 {
        return sign | 0x7C00 | if bits & 0x7FFF_FFFF > 0x7F80_0000 { 0x200 } else { 0 };
    }
    // The mantissa with its hidden bit, the bits that fall off, and where a half of the last kept bit is.
    let (kept, lost, half) = if exponent <= 0 {
        if exponent < -10 {
            return sign;
        }
        let shift = (14 - exponent) as u32;
        let whole = mantissa | 0x80_0000;
        (whole >> shift, whole & ((1 << shift) - 1), 1u32 << (shift - 1))
    } else {
        ((exponent as u32) << 10 | mantissa >> 13, mantissa & 0x1FFF, 0x1000)
    };
    let up = lost > half || (lost == half && kept & 1 == 1);
    sign | (kept + u32::from(up)) as u16
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::world::PlayerId;

    const FRAME: f32 = 1.0 / 60.0;

    /// What is pending after `frames` frames of a turn of `rate` rad/s to the right (the input's yaw falls).
    fn turning(mut pending: Turn, rate: f32, frames: u32, raised: f32) -> Turn {
        for _ in 0..frames {
            let all = pending.plus(Turn { yaw: -rate * FRAME, pitch: 0.0 });
            pending = all.less(taken(all, FRAME, raised));
        }
        pending
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
    fn a_still_mouse_leaves_nothing_pending() {
        assert_eq!(turning(Turn::NONE, 0.0, 60, 0.0), Turn::NONE);
        assert_eq!(taken(Turn::NONE, FRAME, 0.0), Turn::NONE);
    }

    #[test]
    fn the_gun_comes_after_a_turn_and_up_to_it() {
        // what is pending before a frame's share is taken, at 60 frames a second
        let before = |rate: f32, after: Turn| -(after.yaw - rate * DEGREE * FRAME) / DEGREE;
        // 40 degrees a second: the gun settles some 5 degrees behind
        let slow = turning(Turn::NONE, 40.0 * DEGREE, 180, 0.0);
        assert!((before(40.0, slow) - 5.0).abs() < 0.5 && slow.pitch == 0.0, "{}", before(40.0, slow));
        assert!((turning(slow, 40.0 * DEGREE, 30, 0.0).yaw - slow.yaw).abs() < 0.05 * DEGREE);
        // the faster the turn the further, with no stop: 115 and 350 degrees a second
        let (quick, fast) = (turning(Turn::NONE, 115.0 * DEGREE, 180, 0.0), turning(Turn::NONE, 350.0 * DEGREE, 180, 0.0));
        assert!((before(115.0, quick) - 10.0).abs() < 0.7 && (before(350.0, fast) - 17.0).abs() < 1.0, "{} {}", before(115.0, quick), before(350.0, fast));
        // the turn over, the gun comes up to the view: each frame nearer, nothing left in a second
        let mut last = fast;
        for _ in 0..10 {
            let next = turning(last, 0.0, 1, 0.0);
            assert!(next.yaw > last.yaw && next.yaw < 0.0, "{} {}", last.yaw, next.yaw);
            last = next;
        }
        assert!(turning(fast, 0.0, 60, 0.0).angle() < 0.05 * DEGREE);
    }

    #[test]
    fn fully_aimed_the_gun_is_close_behind() {
        let hip = turning(Turn::NONE, 60.0 * DEGREE, 120, 0.0);
        let aimed = turning(Turn::NONE, 60.0 * DEGREE, 120, 1.0);
        assert!(aimed.yaw.abs() < 0.25 * hip.yaw.abs() && aimed.yaw < 0.0, "{} {}", aimed.yaw / DEGREE, hip.yaw / DEGREE);
        assert_eq!(turning(Turn::NONE, 60.0 * DEGREE, 30, f32::NAN), turning(Turn::NONE, 60.0 * DEGREE, 30, 1.0));
    }

    #[test]
    fn a_long_frame_and_too_much_pending_go_to_the_gun_at_once() {
        let pending = Turn { yaw: 5.0 * DEGREE, pitch: 0.02 };
        assert_eq!(taken(pending, 0.3, 0.0), pending);
        assert_eq!(taken(pending, 0.0, 0.0), pending);
        // 60 degrees thrown in one frame: 30 stay pending
        let flick = Turn { yaw: -60.0 * DEGREE, pitch: 0.0 };
        let left = flick.less(taken(flick, 0.001, 0.0));
        assert!((left.angle() - MOST).abs() < 0.01 * DEGREE, "{}", left.angle() / DEGREE);
    }

    #[test]
    fn the_view_is_ahead_of_the_eye_by_what_is_pending() {
        let aim = Look { yaw: 0.4, pitch: 0.1 };
        assert_eq!(ahead(aim, Turn::NONE), aim);
        // the input's yaw grows to the left: 5 degrees to the right pending
        let look = ahead(aim, Turn { yaw: -5.0 * DEGREE, pitch: 0.05 });
        assert!((look.yaw - aim.yaw - 5.0 * DEGREE).abs() < 1e-6 && (look.pitch - aim.pitch - 0.05 * UNIT_UP).abs() < 1e-6);
        let down = ahead(Look { yaw: 0.0, pitch: -0.1 }, Turn { yaw: 0.0, pitch: -0.05 });
        assert!((down.pitch + 0.1 + 0.05 * UNIT_DOWN).abs() < 1e-6);
        assert!(ahead(Look { yaw: 0.0, pitch: 85.0 * DEGREE }, Turn { yaw: 0.0, pitch: 0.5 }).pitch <= 89.0 * DEGREE);
        // a look pitch at its end has no pitch left to wait for
        let up = Turn { yaw: 0.1, pitch: 0.3 };
        assert_eq!(up.within(0.9).pitch, 1.0 - 0.9);
        assert_eq!((up.within(1.0).pitch, up.within(1.2).pitch, up.within(0.0), up.within(f32::NAN)), (0.0, 0.0, up, up));
        assert_eq!(Turn { yaw: 0.0, pitch: -0.3 }.within(-0.8).pitch, -1.0 + 0.8);
    }

    #[test]
    fn a_turn_is_kept_as_the_handlers_halves() {
        for (value, half) in [(0.0f32, 0x0000u16), (1.0, 0x3C00), (-2.0, 0xC000), (0.5, 0x3800), (65504.0, 0x7BFF), (6.1035156e-5, 0x0400), (5.9604645e-8, 0x0001)] {
            assert_eq!(half_of(value), half, "{value}");
            assert_eq!(of_half(half), value, "{half:#x}");
        }
        assert_eq!((half_of(1.0e6), half_of(-1.0e6), half_of(1.0e-9)), (0x7C00, 0xFC00, 0));
        assert!(of_half(0x7C00).is_infinite() && of_half(half_of(f32::NAN)).is_nan());
        // nearest, ties to even: 1 + 1/2048 is halfway between 0x3C00 and 0x3C01
        assert_eq!((half_of(1.0 + 1.0 / 2048.0), half_of(1.0 + 3.0 / 2048.0), half_of(1.0 + 1.1 / 2048.0)), (0x3C00, 0x3C02, 0x3C01));
        // what the gun takes is what the halves hold, so nothing is lost between the frames
        let turn = Turn { yaw: -0.012_345_6, pitch: 0.003_21 };
        let kept = Turn::of_halves(turn.halves());
        assert!((kept.yaw - turn.yaw).abs() < 1e-5 && (kept.pitch - turn.pitch).abs() < 3e-6);
        assert_eq!(Turn::of_halves(kept.halves()), kept);
    }

    #[test]
    fn the_hip_shake_goes_from_the_view_to_the_gun() {
        // the eye mixed without the shake, and with it: turned a little about the up
        let (sine, cosine) = (0.015f32, (1.0f32 - 0.015 * 0.015).sqrt());
        let without = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 1.6, 0.1, 1.];
        let with = [cosine, 0., -sine, 0., 0., 1., 0., 0., sine, 0., cosine, 0., 0., 1.6, 0.1, 1.];
        let shake = Shake { player: PlayerId::for_test(1), without, with };
        let world = [0., 0., 1., 0., 0., 1., 0., 0., -1., 0., 0., 0., 10., 0., 20., 1.];
        let still = calmed(&product(&with, &world), &shake).unwrap();
        assert!(still.iter().zip(&product(&without, &world)).all(|(a, b)| (a - b).abs() < 1e-5), "{still:?}");
        let gun = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0.1, 1.5, 0.5, 1.];
        let hip = shaken(&gun, &shake, 0.0).unwrap();
        assert!((row(&hip, 2).x - sine).abs() < 1e-5 && (row(&hip, 2).length() - 1.0).abs() < 1e-5, "{:?}", row(&hip, 2));
        assert_eq!(shaken(&gun, &shake, 1.0).unwrap(), gun);
        assert_eq!(shaken(&gun, &shake, f32::NAN).unwrap(), gun);
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
        let back = Look::of(look.forward()).unwrap();
        assert!((back.yaw - look.yaw).abs() < 1e-5 && (back.pitch - look.pitch).abs() < 1e-5);
        assert_eq!(Look::of(Vec3::new(0.0, 0.0, 0.0)), None);
    }
}
