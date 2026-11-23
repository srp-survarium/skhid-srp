//! Patch 197, a prototype (off unless `SKHID_CAMERA_TRAIL` is set): the drawn
//! camera trails the aim.
//!
//! The first-person view is the eyes' transform (bone "Item"), which the
//! mouse, the weapon's kick and the shot clips turn, and a bullet leaves along
//! it: the screen's centre is the bullet. Here the view drawn turns after
//! that aim instead of with it: inside `DEAD` of the aim it holds still, past
//! it it follows, never more than `MOST` behind. The gun and its sights stay
//! on the aim (they hang on the eyes' transform), so they move over a calm
//! screen and show where the bullet goes; the screen's centre no longer does.
//!
//! Only the view moves, at update_camera's eye (`camera.eye`, after the
//! ragdoll's and the head clearance's): the eyes' transform, the aim, the
//! shots and the worlds are the game's. Nothing a match computes.
//!
//! Not done: the HUD's crosshair stays at the screen's centre.

#![forbid(unsafe_code)]

use ragdoll::math::{Vec3, abs, atan2, clamp, exp_neg, min, sin_cos, sqrt};

use crate::vostok::world::PlayerId;
use crate::vostok::{Context, Flow, GameCell, NoExit, camera, clock, renderer};

const DEGREE: f32 = core::f32::consts::PI / 180.0;
/// How far the aim turns from the camera before the camera follows (rad,
/// unzoomed; every zone is times the view's field-of-view factor, so it is
/// the same size on the screen through a scope).
const DEAD: Look = Look { yaw: 0.8 * DEGREE, pitch: 0.55 * DEGREE };
/// Past `DEAD`, the further turn over which the camera's pull grows to full.
const SOFT: Look = Look { yaw: 1.8 * DEGREE, pitch: 1.2 * DEGREE };
/// The most the camera is behind the aim.
const MOST: Look = Look { yaw: 3.0 * DEGREE, pitch: 2.0 * DEGREE };
/// The share of the way to where it is pulled the camera goes in a second
/// (1 - e^(-rate t)).
const RATE: f32 = 14.0;
/// The longest frame the camera steps by (s).
const LONGEST_FRAME: f32 = 0.1;

/// A direction, or a pair of angles about the two axes (rad): `yaw` about the
/// world's up, `pitch` up from the level.
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

    fn forward(self) -> Vec3 {
        let ((sin_yaw, cos_yaw), (sin_pitch, cos_pitch)) = (sin_cos(self.yaw), sin_cos(self.pitch));
        Vec3::new(sin_yaw * cos_pitch, sin_pitch, cos_yaw * cos_pitch)
    }
}

/// The camera, between frames.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Trail {
    /// The switch is not read yet.
    Unread,
    Off,
    /// On, with no first-person eye shown last frame.
    Waiting,
    /// `player`'s view looked along `look` at `at` (the counter's
    /// milliseconds, wrapping).
    Following { player: PlayerId, look: Look, at: u32 },
}

static TRAIL: GameCell<Trail> = GameCell::new(Trail::Unread);

/// update_camera's eye (`camera.eye`, after the head clearance's).
pub fn on_eye(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let state = match TRAIL.get(thread) {
        Trail::Unread if renderer::environment_set(thread, c"SKHID_CAMERA_TRAIL") => Trail::Waiting,
        Trail::Unread => Trail::Off,
        state => state,
    };
    if state == Trail::Off {
        TRAIL.set(thread, Trail::Off);
        return Flow::Continue;
    }
    let eye = camera::given_eye(ctx);
    let aimed = match camera::eye_player(ctx) {
        Some((player, true)) => Look::of(row(&eye, 2)).map(|aim| (player.id(), aim)),
        _ => None,
    };
    let Some((player, aim)) = aimed else {
        TRAIL.set(thread, Trail::Waiting);
        return Flow::Continue;
    };
    let zoom = camera::eye_fov_factor(ctx).filter(|factor| factor.is_finite() && *factor > 0.0).map_or(1.0, |factor| min(factor, 1.0));
    let now = clock::milliseconds(thread);
    let look = shown(state, player, aim, zoom, now);
    TRAIL.set(thread, Trail::Following { player, look, at: now });
    if let Some(view) = turned(&eye, look) {
        camera::look_from(ctx, view);
    }
    Flow::Continue
}

/// Where `player`'s camera looks this frame: on from where it looked last
/// frame toward the `aim`, or along the aim for a player it did not follow.
fn shown(state: Trail, player: PlayerId, aim: Look, zoom: f32, now: u32) -> Look {
    match state {
        Trail::Following { player: last, look, at } if last == player => {
            let dt = clamp(now.wrapping_sub(at) as f32 / 1000.0, 0.0, LONGEST_FRAME);
            Look {
                yaw: followed(look.yaw, aim.yaw, Zone::of(|zone| zone.yaw, zoom), dt),
                pitch: followed(look.pitch, aim.pitch, Zone::of(|zone| zone.pitch, zoom), dt),
            }
        }
        _ => aim,
    }
}

/// One axis's zones (rad).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Zone {
    dead: f32,
    soft: f32,
    most: f32,
}

impl Zone {
    fn of(axis: impl Fn(Look) -> f32, zoom: f32) -> Zone {
        Zone { dead: axis(DEAD) * zoom, soft: axis(SOFT) * zoom, most: axis(MOST) * zoom }
    }
}

/// The camera's angle after `dt` s, from `camera` toward the `aim`: still
/// while the aim is within `dead`, pulled to `dead` short of it past that
/// (fully once `soft` further), never more than `most` behind.
fn followed(camera: f32, aim: f32, zone: Zone, dt: f32) -> f32 {
    let behind = turn(aim - camera);
    let past = abs(behind) - zone.dead;
    let pulled = if past <= 0.0 { 0.0 } else { (behind - clamp(behind, -zone.dead, zone.dead)) * ramp(past / zone.soft) };
    let next = camera + pulled * (1.0 - exp_neg(RATE * dt));
    let behind = turn(aim - next);
    if abs(behind) > zone.most { turn(aim - clamp(behind, -zone.most, zone.most)) } else { turn(next) }
}

/// 0 to 1 over 0 to 1, level at both ends.
fn ramp(t: f32) -> f32 {
    let t = clamp(t, 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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

/// The eye's transform looking along `look`: its place, level (no roll), of
/// the eye's handedness. None when the look is along the world's up.
fn turned(eye: &[f32; 16], look: Look) -> Option<[f32; 16]> {
    let forward = look.forward();
    let right = like(Vec3::new(0.0, 1.0, 0.0).cross(forward).direction()?, row(eye, 0));
    let up = like(forward.cross(right), row(eye, 1));
    let mut view = *eye;
    for (at, axis) in [right, up, forward].into_iter().enumerate() {
        view[at * 4..at * 4 + 3].copy_from_slice(&[axis.x, axis.y, axis.z]);
    }
    Some(view)
}

/// `axis`, or its opposite: whichever is nearer `as_this`.
fn like(axis: Vec3, as_this: Vec3) -> Vec3 {
    if axis.dot(as_this) < 0.0 { axis * -1.0 } else { axis }
}

/// A row-vector 4x4's row (0 right, 1 up, 2 forward).
fn row(m: &[f32; 16], at: usize) -> Vec3 {
    Vec3::new(m[at * 4], m[at * 4 + 1], m[at * 4 + 2])
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAW: Zone = Zone { dead: DEAD.yaw, soft: SOFT.yaw, most: MOST.yaw };
    const FRAME: f32 = 1.0 / 60.0;

    fn player(n: u32) -> PlayerId {
        PlayerId::for_test(n)
    }

    #[test]
    fn a_look_is_its_direction() {
        for look in [Look { yaw: 0.0, pitch: 0.0 }, Look { yaw: 2.5, pitch: 0.9 }, Look { yaw: -3.0, pitch: -1.0 }] {
            let back = Look::of(look.forward() * 4.0).unwrap();
            assert!((back.yaw - look.yaw).abs() < 1e-4 && (back.pitch - look.pitch).abs() < 1e-4, "{look:?} {back:?}");
        }
        assert_eq!(Look::of(Vec3::new(0.0, 0.0, 0.0)), None);
    }

    #[test]
    fn the_camera_holds_inside_the_dead_zone() {
        assert_eq!(followed(0.2, 0.2 + YAW.dead * 0.9, YAW, FRAME), 0.2);
        assert_eq!(followed(0.2, 0.2 - YAW.dead * 0.9, YAW, FRAME), 0.2);
    }

    #[test]
    fn the_camera_follows_to_the_dead_zone_of_a_still_aim() {
        let aim = 2.0 * DEGREE;
        let mut camera = 0.0;
        let first = followed(camera, aim, YAW, FRAME);
        assert!(first > 0.0 && first < aim, "{first}");
        for _ in 0..600 {
            camera = followed(camera, aim, YAW, FRAME);
        }
        let behind = aim - camera;
        assert!((YAW.dead..YAW.dead * 1.3).contains(&behind), "{} deg", behind / DEGREE);
    }

    #[test]
    fn the_camera_is_never_more_than_the_most_behind() {
        let camera = followed(0.0, 0.5, YAW, FRAME);
        assert!((0.5 - camera - YAW.most).abs() < 1e-6, "{camera}");
        let across = followed(3.1, -3.1, YAW, FRAME);
        assert!((turn(-3.1 - across) - YAW.most).abs() < 1e-5, "{across}");
        assert_eq!(followed(0.0, 0.5, YAW, 0.0), 0.5 - YAW.most);
    }

    #[test]
    fn a_zoomed_view_has_smaller_zones() {
        let zone = Zone::of(|zone| zone.yaw, 0.25);
        assert!((zone.dead - YAW.dead / 4.0).abs() < 1e-7 && (zone.most - YAW.most / 4.0).abs() < 1e-7);
        assert_ne!(followed(0.0, YAW.dead * 0.9, zone, FRAME), 0.0);
    }

    #[test]
    fn a_new_player_is_shown_along_the_aim() {
        let aim = Look { yaw: 1.0, pitch: 0.2 };
        assert_eq!(shown(Trail::Waiting, player(1), aim, 1.0, 5), aim);
        let other = Trail::Following { player: player(2), look: Look { yaw: 0.0, pitch: 0.0 }, at: 0 };
        assert_eq!(shown(other, player(1), aim, 1.0, 16), aim);
        let same = Trail::Following { player: player(1), look: Look { yaw: 0.99, pitch: 0.2 }, at: 0 };
        assert_eq!(shown(same, player(1), aim, 1.0, 16), Look { yaw: 0.99, pitch: 0.2 });
    }

    #[test]
    fn the_view_keeps_the_eyes_place_and_hand() {
        let mut eye = [0.0; 16];
        eye[0] = 1.0;
        eye[5] = 1.0;
        eye[10] = 1.0;
        eye[12..16].copy_from_slice(&[3.0, 4.0, 5.0, 1.0]);
        assert_eq!(turned(&eye, Look { yaw: 0.0, pitch: 0.0 }), Some(eye));
        let view = turned(&eye, Look { yaw: 0.3, pitch: 0.2 }).unwrap();
        let (right, up, forward) = (row(&view, 0), row(&view, 1), row(&view, 2));
        assert!((forward - Look { yaw: 0.3, pitch: 0.2 }.forward()).length() < 1e-6);
        assert!(right.dot(up).abs() < 1e-6 && right.dot(forward).abs() < 1e-6 && (right.cross(up) - forward).length() < 1e-5);
        assert!(right.y.abs() < 1e-6, "level: {right:?}");
        assert_eq!(view[12..16], [3.0, 4.0, 5.0, 1.0]);
        // the game's eye is of the other hand when its right is mirrored
        eye[0] = -1.0;
        let mirrored = turned(&eye, Look { yaw: 0.3, pitch: 0.2 }).unwrap();
        assert!((row(&mirrored, 0) + right).length() < 1e-6 && (row(&mirrored, 1) - up).length() < 1e-6);
        assert_eq!(turned(&eye, Look { yaw: 0.0, pitch: core::f32::consts::FRAC_PI_2 }), None);
    }
}
