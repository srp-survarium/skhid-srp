//! Patch 197: the drawn camera and the aim.
//!
//! The first-person view is the eyes' transform (bone "Item"), which the
//! mouse, the weapon's kick and the shot clips turn; the gun and its sights
//! hang on it, and a shot leaves the gun (patch 200). Two things are drawn
//! apart from that aim here, at update_camera's eye (`camera.eye`, after the
//! ragdoll's and the head clearance's). Only the view moves: the eyes'
//! transform, the aim, the shots and the worlds are the game's. Nothing a
//! match computes.
//!
//! **The weapon's kick is the gun's** (the recoil on top of patch 200's shots
//! from the gun; always): of the lift the kick gives the eye the view takes
//! `KICK_SHOWN`, from the hip too, and the gun, on the aim, rises over it
//! and comes back as the kick does. Not while the scope's own view is on.
//!
//! **The view trails the aim** (a prototype, only with `SKHID_CAMERA_TRAIL`
//! set): inside `DEAD` of where it rests it holds still, past it it follows,
//! never more than `MOST` behind. The zones are times how far the sight is
//! raised (patch 172's site tells: `aim_pose::take_aim_progress`), so from
//! the hip the view rests where the kick's share puts it. With the switch
//! the aimed gun also shows less of its shot clip (`aim_pose::Kick::Back`):
//! its sights stay on the aim, and of its kick `BACK_SHARE` back along it.
//! Not done: with picture-in-picture scopes (patch 198) the frame is not
//! zoomed while the handler's factor says it is, so the zones are then that
//! much smaller on the screen than without.

#![forbid(unsafe_code)]

use ragdoll::math::{Vec3, abs, atan2, clamp, exp_neg, min, sin_cos, sqrt};

use crate::aim_pose::{self, Kick};
use crate::vostok::world::PlayerId;
use crate::vostok::{Context, Flow, GameCell, NoExit, camera, clock, renderer, shot};

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
/// The share of the gun's own kick back along the view the aimed view shows.
const BACK_SHARE: f32 = 0.35;
/// The share of the lift the weapon's kick gives the eye that the view takes;
/// the gun takes all of it.
const KICK_SHOWN: f32 = 0.4;
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

    #[cfg(test)]
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
    /// No trail: the view rests where the kick's share puts it.
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
        Trail::Unread if renderer::environment_set(thread, c"SKHID_CAMERA_TRAIL") => {
            aim_pose::show(thread, Kick::Back { share: BACK_SHARE });
            Trail::Waiting
        }
        Trail::Unread => Trail::Off,
        state => state,
    };
    let eye = camera::given_eye(ctx);
    let aimed = match camera::eye_player(ctx) {
        Some((player, true)) => Look::of(row(&eye, 2)).map(|aim| (player.id(), aim, rested(aim, shot::free_kick_lift(player)))),
        _ => None,
    };
    let Some((player, aim, rest)) = aimed else {
        TRAIL.set(thread, if state == Trail::Off { Trail::Off } else { Trail::Waiting });
        return Flow::Continue;
    };
    let look = if state == Trail::Off {
        TRAIL.set(thread, Trail::Off);
        rest
    } else {
        let zoom = camera::eye_fov_factor(ctx).filter(|factor| factor.is_finite() && *factor > 0.0).map_or(1.0, |factor| min(factor, 1.0));
        let raised = aim_pose::take_aim_progress(thread);
        let zoom = zoom * if raised.is_finite() { clamp(raised, 0.0, 1.0) } else { 0.0 };
        let now = clock::milliseconds(thread);
        let look = shown(state, player, rest, zoom, now);
        TRAIL.set(thread, Trail::Following { player, look, at: now });
        look
    };
    // On the aim (from the hip with no kick always): the eye as it is given.
    if look != aim {
        camera::look_from(ctx, turned(&eye, aim, look));
    }
    Flow::Continue
}

/// Where the view rests under an `aim` whose weapon's kick lifts the eye by
/// `lift` (rad): the aim less the kick's share the view does not take.
fn rested(aim: Look, lift: f32) -> Look {
    Look { yaw: aim.yaw, pitch: aim.pitch - (1.0 - KICK_SHOWN) * lift }
}

/// Where `player`'s camera looks this frame: on from where it looked last
/// frame toward the `aim`, or along the aim for a player it did not follow.
/// `zoom` sizes the zones: the view's field-of-view factor times how far the
/// sight is raised (0 from the hip: the view is the aim).
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

    const YAW: Zone = Zone { dead: DEAD.yaw, soft: SOFT.yaw, most: MOST.yaw };
    const FRAME: f32 = 1.0 / 60.0;

    fn player(n: u32) -> PlayerId {
        PlayerId::for_test(n)
    }

    #[test]
    fn the_view_takes_a_share_of_the_kicks_lift() {
        let aim = Look { yaw: 0.3, pitch: 10.0 * DEGREE };
        assert_eq!(rested(aim, 0.0), aim);
        let rest = rested(aim, 5.0 * DEGREE);
        assert_eq!(rest.yaw, aim.yaw);
        assert!((rest.pitch - (10.0 - 5.0 * (1.0 - KICK_SHOWN)) * DEGREE).abs() < 1e-6);
        // From the hip (no zones) the view is that rest at once.
        let following = Trail::Following { player: player(1), look: aim, at: 0 };
        assert_eq!(shown(following, player(1), rest, 0.0, 16), rest);
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
    fn the_view_is_the_eye_turned_to_the_look() {
        let aim = Look { yaw: 0.4, pitch: -0.2 };
        // an eye along the aim, rolled a little, at a place
        let level = turned(&IDENTITY, Look { yaw: 0.0, pitch: 0.0 }, aim);
        assert!((row(&level, 2) - aim.forward()).length() < 1e-6, "{level:?}");
        let mut eye = level;
        let (right, up) = (row(&level, 0), row(&level, 1));
        for (at, axis) in [right * 0.995 + up * 0.0998, up * 0.995 - right * 0.0998].into_iter().enumerate() {
            eye[at * 4..at * 4 + 3].copy_from_slice(&[axis.x, axis.y, axis.z]);
        }
        eye[12..16].copy_from_slice(&[3.0, 4.0, 5.0, 1.0]);
        assert_eq!(turned(&eye, aim, aim), eye);
        let look = Look { yaw: 0.43, pitch: -0.17 };
        let view = turned(&eye, aim, look);
        let (right, up, forward) = (row(&view, 0), row(&view, 1), row(&view, 2));
        assert!((forward - look.forward()).length() < 1e-6, "{forward:?}");
        // still a frame of the eye's hand, with the eye's roll and place
        assert!(right.dot(up).abs() < 1e-6 && right.dot(forward).abs() < 1e-6);
        let handed = |x: Vec3, y: Vec3, z: Vec3| (x.cross(y) - z).length() < 1e-5;
        assert_eq!(handed(right, up, forward), handed(row(&eye, 0), row(&eye, 1), row(&eye, 2)));
        assert!((right.y - row(&eye, 0).y).abs() < 0.01, "the roll: {} from {}", right.y, row(&eye, 0).y);
        assert_eq!(view[12..16], [3.0, 4.0, 5.0, 1.0]);
    }

    const IDENTITY: [f32; 16] = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];
}
