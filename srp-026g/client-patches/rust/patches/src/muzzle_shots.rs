//! Patch 200: a shot leaves the gun's muzzle, the way the gun looks
//! (docs/notes/muzzle-shots.md).
//!
//! The game fires from the eyes: instant_fire (0x2E23A0) copies the user's
//! eyes' transform (player+44Ch, the bone Item) and takes the bullet's start
//! and direction from it, wherever the gun is. Here the copy is replaced, at
//! `shot.eye`, by the gun's own:
//!
//! - its place is the gun's fire point (the model's `barrel_point`), on the
//!   gun as the simulation poses it: the hands' Weapon bone, and as the sight
//!   is raised the aim's alignment the drawing makes (the aim locator the
//!   eye), blended by the aim's progress;
//! - its forward is the way the gun looks: along the barrel from the hip,
//!   along the sights, which the alignment lays on the eye's line, fully
//!   aimed, and between them as the sight is raised;
//! - a wall between the eye and the muzzle keeps the shot at the eye: no
//!   bullet starts beyond what the barrel pokes through.
//!
//! The gun is posed at the shot's own time (the tree's, inside the animation
//! event), where the game's copy is the eye of the step before: a burst's
//! bullets no longer take the hands clip's camera turn of the pass before.
//!
//! The spread, the kick and the bullets are the game's, from that transform.
//! Every game computes it for every player from the simulation's own state,
//! so a match plays out otherwise than before: simulation_changes.tsv.
//!
//! The screen's centre is then no longer where a shot goes, so the HUD's
//! crosshair is put where the own player's gun looks: after each frame's
//! simulation (`game_world.after_move`) the place the shot's line first
//! meets the level, or `SIGHT_RANGE` along it, is kept; the frame's camera
//! (`camera.apply`) puts it on the screen, and the HUD movie is told
//! (`player_icons.update_start`, `srp_crosshair_at`) when it moves. Drawing
//! only.
//!
//! `SKHID_SHOT_LOG` set in the game's environment (read once, after a
//! frame's simulation) writes a line a shot to the patches' log: how far the
//! sight is raised, how far the shot's way is from the eyes' copy's and
//! where the muzzle is from that eye. Nothing without it.

#![forbid(unsafe_code)]

use ragdoll::math::{Quat, Vec3, atan2};

use crate::affine::{inverse, product, row, with_row};
use crate::vostok::camera::Director;
use crate::vostok::log::{self, Piece::{self, Dec, Text}};
use crate::vostok::shot::{HeldGun, Shot};
use crate::vostok::world::GameWorld;
use crate::vostok::{Context, Flow, GameWorldUi, Lock, NoExit, renderer};

/// How far along the gun's look the crosshair is put when the level stops
/// nothing nearer (m).
const SIGHT_RANGE: f32 = 150.0;

/// instant_fire after its copy of the eyes' transform, before the spread.
pub fn on_shot(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut shot) = Shot::at(ctx)
        && let Some(leaving) = Leaving::of(shot.gun())
    {
        if SHOT_LOG.with(ctx.thread(), |log| *log) == ShotLog::On {
            say(&shot.eye(), shot.gun().raised(), &leaving);
        }
        shot.leave_from(shot_from(&leaving.eye, leaving.place, leaving.along));
    }
    Flow::Continue
}

/// Whether shots are logged (`SKHID_SHOT_LOG`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum ShotLog {
    /// The environment is not read yet (it is after a frame's simulation,
    /// not in a shot).
    Unread,
    Off,
    On,
}

static SHOT_LOG: Lock<ShotLog> = Lock::new(ShotLog::Unread);

/// A shot's line: against the eyes' copy `was`, the angle of its way and its
/// place (the copy's right, up and forward).
fn say(was: &[f32; 16], raised: f32, leaving: &Leaving) {
    let (right, up, forward) = (row(was, 0), row(was, 1), row(was, 2));
    let turned = atan2(forward.cross(leaving.along).length(), forward.dot(leaving.along)).to_degrees();
    let from = leaving.place - row(was, 3);
    let signed = |value: f32| -> [Piece<'static>; 2] { [Text(if value < 0.0 { "-" } else { "" }), Dec((value.abs() + 0.5) as u32)] };
    let [r, u, f] = [from.dot(right), from.dot(up), from.dot(forward)].map(|metres| signed(metres * 1000.0));
    log::line(&[Text("muzzle_shots: raised "), Dec((raised.clamp(0.0, 1.0) * 100.0) as u32), Text(" %, the way "),
                Dec((turned * 100.0 + 0.5) as u32), Text(" hundredths of a degree off the eye's, from "), r[0], r[1], Text(" right "), u[0],
                u[1], Text(" up "), f[0], f[1], Text(" ahead of it (mm)")]);
}

/// What a shot of a held gun leaves from: a place and a way, and the eyes'
/// transform they are laid out by.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Leaving {
    eye: [f32; 16],
    place: Vec3,
    along: Vec3,
}

impl Leaving {
    /// None (the eyes' copy stays) for a gun or a user whose pose cannot be
    /// made.
    fn of(gun: &HeldGun<'_>) -> Option<Leaving> {
        let rig = Rig {
            world: gun.world(),
            hand: gun.user_bone("Weapon")?,
            eye: gun.user_bone("Item")?,
            aim_offset: gun.aim_offset(),
            aim: gun.aim()?.0,
            barrel: gun.barrel()?.0,
            raised: gun.raised(),
        };
        let pose = rig.pose()?;
        let muzzle = pose.muzzle()?;
        let eye = row(&pose.eye, 3);
        let reach = muzzle.place - eye;
        let walled = reach.direction().is_some_and(|toward| gun.first_met(eye, toward, reach.length()).is_some());
        Some(Leaving { eye: pose.eye, place: if walled { eye } else { muzzle.place }, along: muzzle.along })
    }
}

/// The crosshair between the frame's passes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Crosshair {
    /// Where the own player's gun looks, in the world; none with no gun in
    /// its hands (the crosshair is the game's, at the centre).
    looked: Option<Vec3>,
    /// The frame's camera: its transform (rows right, up, forward, place)
    /// and its projection.
    camera: Option<([f32; 16], [f32; 16])>,
    told: Told,
}

/// What the HUD movie was last told.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Told {
    Nothing,
    /// The movie `movie`: the crosshair at these parts of the screen.
    At { movie: usize, x: f32, y: f32 },
}

/// The simulation's thread writes the look, the frame's the rest.
static CROSSHAIR: Lock<Crosshair> = Lock::new(Crosshair { looked: None, camera: None, told: Told::Nothing });

/// The screen's centre.
const CENTRE: (f32, f32) = (0.5, 0.5);

/// game_world::tick after the simulation's move: where the own player's gun
/// looks now.
pub fn on_after_move(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if SHOT_LOG.with(thread, |log| *log) == ShotLog::Unread {
        let on = renderer::environment_set(thread, c"SKHID_SHOT_LOG");
        SHOT_LOG.with(thread, |log| *log = if on { ShotLog::On } else { ShotLog::Off });
    }
    let looked = GameWorld::at_after_move(ctx)
        .and_then(|world| world.network_client()?.local_player())
        .filter(|player| player.alive())
        .and_then(HeldGun::of)
        .and_then(|gun| {
            let leaving = Leaving::of(&gun)?;
            Some(gun.first_met(leaving.place, leaving.along, SIGHT_RANGE).unwrap_or(leaving.place + leaving.along * SIGHT_RANGE))
        });
    CROSSHAIR.with(ctx.thread(), |crosshair| crosshair.looked = looked);
    Flow::Continue
}

/// base_game_scene::apply_camera: the camera the frame is drawn from.
pub fn on_camera(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let camera = Director::at_apply(ctx).map(|director| (director.view(), director.projection()));
    CROSSHAIR.with(ctx.thread(), |crosshair| crosshair.camera = camera);
    Flow::Continue
}

/// update_player_icons' start, every frame of a match: the HUD movie told
/// where its crosshair is, when that changed.
pub fn on_hud(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(movie) = GameWorldUi::at_update_icons(ctx).and_then(|ui| ui.hud_movie()) else { return Flow::Continue };
    let (told, wanted) = CROSSHAIR.with(ctx.thread(), |crosshair| {
        let (x, y) = crosshair.looked.zip(crosshair.camera).and_then(|(place, (view, projection))| on_screen(&view, &projection, place)).unwrap_or(CENTRE);
        (crosshair.told, Told::At { movie: movie.id(), x, y })
    });
    if told != wanted
        && let Told::At { x, y, .. } = wanted
        && movie.invoke_numbers(c"root.srp_crosshair_at", &[f64::from(x), f64::from(y)])
    {
        CROSSHAIR.with(ctx.thread(), |crosshair| crosshair.told = wanted);
    }
    Flow::Continue
}

/// Where a place of the world is on the screen, in parts of its width and
/// its height from the top left, through a camera's transform and its
/// projection (D3D's, rows; its jitter left out); none for a place not
/// ahead of the camera.
fn on_screen(view: &[f32; 16], projection: &[f32; 16], place: Vec3) -> Option<(f32, f32)> {
    let from = place - row(view, 3);
    let (x, y, z) = (from.dot(row(view, 0)), from.dot(row(view, 1)), from.dot(row(view, 2)));
    let depth = z * projection[11] + projection[15];
    if depth.is_nan() || depth <= 1e-4 {
        return None;
    }
    let across = (x * projection[0] + y * projection[4]) / depth;
    let up = (x * projection[1] + y * projection[5]) / depth;
    (across.is_finite() && up.is_finite()).then_some((0.5 + 0.5 * across, 0.5 - 0.5 * up))
}

/// A half turn about the up (rotation_y(pi)): the game's between a locator
/// and what looks along it (calculate_locator 0x243BC0), and of the user's
/// Weapon bone before the gun hangs on it (update_bones_matrices 0x2FF70C).
const HALF_TURN: [f32; 16] = [-1., 0., 0., 0., 0., 1., 0., 0., 0., 0., -1., 0., 0., 0., 0., 1.];

/// What the simulation has of a gun in its user's hands at a shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rig {
    /// The user in the world (its +40Ch).
    pub world: [f32; 16],
    /// The user's Weapon bone and Item bone (the eyes), in the user's space.
    pub hand: [f32; 16],
    pub eye: [f32; 16],
    /// The aim's offset (a move along the view), its locator and the fire
    /// point's, each in the gun's model.
    pub aim_offset: [f32; 16],
    pub aim: [f32; 16],
    pub barrel: [f32; 16],
    /// The aim's progress.
    pub raised: f32,
}

impl Rig {
    /// The gun as the shooter's own view draws it: on the turned Weapon bone
    /// from the hip; fully aimed where its aim (offset x half turn x locator)
    /// is the eye, process_hands_ik's alignment; between them by the
    /// sight's raising. None for an aim that flattens space.
    pub fn pose(&self) -> Option<Pose> {
        let hip = product(&HALF_TURN, &self.hand);
        let sight = product(&product(&self.aim_offset, &HALF_TURN), &self.aim);
        let aimed = product(&inverse(&sight)?, &self.eye);
        let held = blend(&hip, &aimed, self.raised);
        Some(Pose { eye: product(&self.eye, &self.world), gun: product(&held, &self.world), barrel: self.barrel, raised: self.raised })
    }
}

/// `from` at 0, `to` at 1: the turn between along the shorter arc, the place
/// along the line (what the drawing's interpolate 0xEF780 does, not bit for
/// bit). A progress that is no number is the hip's.
fn blend(from: &[f32; 16], to: &[f32; 16], progress: f32) -> [f32; 16] {
    if progress.is_nan() || progress <= 0.0 {
        return *from;
    }
    if progress >= 1.0 {
        return *to;
    }
    let turn = |m: &[f32; 16]| Quat::from_basis(row(m, 0), row(m, 1), row(m, 2));
    let turned = turn(from).nlerp(turn(to), progress);
    let place = row(from, 3) + (row(to, 3) - row(from, 3)) * progress;
    let axes = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)].map(|axis| turned.rotate(axis));
    [axes[0], axes[1], axes[2], place].into_iter().enumerate().fold(*from, |m, (at, v)| with_row(m, at, v))
}

/// What a shot is taken from, all in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// The eyes' transform (the bone Item): rows right, up, forward, place.
    pub eye: [f32; 16],
    /// The gun's model: the hands' Weapon bone, with the aim's alignment as
    /// far as the sight is raised.
    pub gun: [f32; 16],
    /// The fire point in the gun's model (its locator on its bone): it looks
    /// along its -z.
    pub barrel: [f32; 16],
    /// How far the sight is raised, 0 from the hip to 1 fully aimed.
    pub raised: f32,
}

/// Where the muzzle is and the way the gun looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Muzzle {
    pub place: Vec3,
    pub along: Vec3,
}

impl Pose {
    /// The muzzle: at the fire point; looking along the barrel from the hip
    /// and along the eye's line fully aimed (the sights are on it then), by
    /// the sight's raising between. None for a gun or an eye with no way.
    pub fn muzzle(&self) -> Option<Muzzle> {
        let fire = product(&self.barrel, &self.gun);
        let (barrel, eye) = ((row(&fire, 2) * -1.0).direction()?, row(&self.eye, 2).direction()?);
        let raised = if self.raised.is_finite() { self.raised.clamp(0.0, 1.0) } else { 0.0 };
        let along = (barrel * (1.0 - raised) + eye * raised).direction()?;
        let place = row(&fire, 3);
        (place.is_finite() && along.is_finite()).then_some(Muzzle { place, along })
    }
}

/// The transform a shot leaves from, laid out as the eyes' (rows right, up,
/// forward, place) and of its hand: from `place` along the muzzle's way, its
/// right and up the eye's made square to that. `eye` itself when the eye's
/// axes cannot be squared to it.
pub fn shot_from(eye: &[f32; 16], place: Vec3, along: Vec3) -> [f32; 16] {
    let across = |v: Vec3, of: Vec3| v - of * v.dot(of);
    let Some(right) = across(row(eye, 0), along).direction() else { return *eye };
    let Some(up) = across(across(row(eye, 1), along), right).direction() else { return *eye };
    [right, up, along, place].into_iter().enumerate().fold(*eye, |m, (at, v)| with_row(m, at, v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::affine::IDENTITY;

    fn at(x: f32, y: f32, z: f32) -> [f32; 16] {
        with_row(IDENTITY, 3, Vec3::new(x, y, z))
    }

    /// A fire point 0.6 ahead of the gun's origin, looking along the gun's z
    /// (its own -z): a half turn about y.
    fn barrel() -> [f32; 16] {
        [-1., 0., 0., 0., 0., 1., 0., 0., 0., 0., -1., 0., 0., 0., 0.6, 1.]
    }

    /// A gun held 0.1 under and 0.05 right of the eye, turned `sine` of an
    /// angle to the left about the up.
    fn gun(sine: f32) -> [f32; 16] {
        let cosine = (1.0 - sine * sine).sqrt();
        [cosine, 0., sine, 0., 0., 1., 0., 0., -sine, 0., cosine, 0., 0.05, -0.1, 0., 1.]
    }

    /// A user 1.5 up the world's x-shifted origin, eyes 1.6 over its feet;
    /// a gun whose aim is 0.05 over its origin and 0.1 behind its fire point.
    fn rig(raised: f32) -> Rig {
        Rig {
            world: at(10.0, 0.0, 0.0),
            // held low and to the right, the Weapon bone as the clips give it (a half turn off the gun)
            hand: product(&HALF_TURN, &at(0.2, 1.3, 0.3)),
            eye: at(0.0, 1.6, 0.0),
            aim_offset: at(0.0, 0.0, -0.1),
            // the aim looks along the gun's z: its locator a half turn (calculate_locator turns it back)
            aim: product(&HALF_TURN, &at(0.0, 0.05, 0.0)),
            barrel: barrel(),
            raised,
        }
    }

    #[test]
    fn from_the_hip_the_gun_is_on_the_hands() {
        let pose = rig(0.0).pose().unwrap();
        assert!(near(&pose.gun, &at(10.2, 1.3, 0.3)), "{:?}", pose.gun);
        assert!(near(&pose.eye, &at(10.0, 1.6, 0.0)));
        let muzzle = pose.muzzle().unwrap();
        assert!((muzzle.place - Vec3::new(10.2, 1.3, 0.9)).length() < 1e-5 && (muzzle.along - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-5, "{muzzle:?}");
    }

    #[test]
    fn fully_aimed_the_guns_aim_is_the_eye() {
        let rig = rig(1.0);
        let pose = rig.pose().unwrap();
        // the alignment: offset x half turn x locator, carried by the gun, is the eye
        let sight = product(&product(&rig.aim_offset, &HALF_TURN), &rig.aim);
        assert!(near(&product(&sight, &pose.gun), &pose.eye), "{:?}", product(&sight, &pose.gun));
        // so the gun is 0.05 under the eye's line and its origin 0.1 ahead of the eye
        assert!(near(&pose.gun, &at(10.0, 1.55, 0.1)), "{:?}", pose.gun);
        let muzzle = pose.muzzle().unwrap();
        assert!((muzzle.place - Vec3::new(10.0, 1.55, 0.7)).length() < 1e-5 && muzzle.along == Vec3::new(0.0, 0.0, 1.0), "{muzzle:?}");
    }

    #[test]
    fn the_gun_comes_up_as_the_sight_is_raised() {
        let (hip, half, aimed) = (rig(0.0).pose().unwrap(), rig(0.5).pose().unwrap(), rig(1.0).pose().unwrap());
        let place = |pose: &Pose| row(&pose.gun, 3);
        assert!((place(&half) - (place(&hip) + place(&aimed)) * 0.5).length() < 1e-5, "{:?}", half.gun);
        assert!(near(&rig(f32::NAN).pose().unwrap().gun, &hip.gun) && near(&rig(3.0).pose().unwrap().gun, &aimed.gun));
    }

    #[test]
    fn a_turn_is_blended_along_its_arc() {
        // a quarter turn about the up, halfway: an eighth
        let quarter = [0., 0., -1., 0., 0., 1., 0., 0., 1., 0., 0., 0., 2.0, 0.0, 0.0, 1.];
        let half = blend(&IDENTITY, &quarter, 0.5);
        let s = core::f32::consts::FRAC_1_SQRT_2;
        assert!(near(&half, &[s, 0., -s, 0., 0., 1., 0., 0., s, 0., s, 0., 1.0, 0.0, 0.0, 1.]), "{half:?}");
        assert_eq!((blend(&IDENTITY, &quarter, 0.0), blend(&IDENTITY, &quarter, 1.0)), (IDENTITY, quarter));
    }

    #[test]
    fn an_aim_that_flattens_space_gives_no_pose() {
        let flat = Rig { aim: with_row(IDENTITY, 2, Vec3::new(0.0, 0.0, 0.0)), ..rig(1.0) };
        assert_eq!(flat.pose(), None);
    }

    fn near(a: &[f32; 16], b: &[f32; 16]) -> bool {
        a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5)
    }

    /// A projection of 90 degrees up and down on a screen twice as wide as
    /// high: x by 0.5, y by 1, the depth the place's own.
    const WIDE: [f32; 16] = [0.5, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 1., 0., 0., -0.1, 0.];

    #[test]
    fn a_place_ahead_is_at_the_screens_centre() {
        let camera = at(3.0, 2.0, 1.0);
        assert_eq!(on_screen(&camera, &WIDE, Vec3::new(3.0, 2.0, 11.0)), Some((0.5, 0.5)));
    }

    #[test]
    fn a_place_right_and_up_is_right_and_up_on_the_screen() {
        let camera = at(3.0, 2.0, 1.0);
        // 5 right and 5 up at 10 ahead: half the half-width's half, half the half-height
        let (x, y) = on_screen(&camera, &WIDE, Vec3::new(8.0, 7.0, 11.0)).unwrap();
        assert!((x - 0.625).abs() < 1e-6 && (y - 0.25).abs() < 1e-6, "{x} {y}");
    }

    #[test]
    fn a_place_behind_the_camera_is_nowhere_on_it() {
        let camera = at(3.0, 2.0, 1.0);
        assert_eq!(on_screen(&camera, &WIDE, Vec3::new(3.0, 2.0, -4.0)), None);
        assert_eq!(on_screen(&camera, &WIDE, Vec3::new(3.0, 2.0, 1.0)), None);
    }

    fn pose(raised: f32) -> Pose {
        Pose { eye: at(0.0, 1.5, 0.0), gun: product(&gun(0.1), &at(0.0, 1.5, 0.0)), barrel: barrel(), raised }
    }

    #[test]
    fn from_the_hip_the_shot_goes_along_the_barrel() {
        let muzzle = pose(0.0).muzzle().unwrap();
        // 0.6 along the turned gun from 0.05 right and 0.1 under the eye
        assert!((muzzle.place - Vec3::new(0.05 - 0.06, 1.4, 0.6 * 0.994_987_4)).length() < 1e-6, "{muzzle:?}");
        assert!((muzzle.along - Vec3::new(-0.1, 0.0, 0.994_987_4)).length() < 1e-6, "{muzzle:?}");
    }

    #[test]
    fn fully_aimed_it_goes_along_the_eyes_line_from_the_muzzle() {
        let muzzle = pose(1.0).muzzle().unwrap();
        assert_eq!(muzzle.along, Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(muzzle.place, pose(0.0).muzzle().unwrap().place);
    }

    #[test]
    fn between_it_turns_with_the_sights_raising() {
        let (hip, half, aimed) = (pose(0.0).muzzle().unwrap(), pose(0.5).muzzle().unwrap(), pose(1.0).muzzle().unwrap());
        assert!(half.along.x < aimed.along.x && half.along.x > hip.along.x, "{half:?}");
        assert!((half.along.length() - 1.0).abs() < 1e-6);
        for odd in [f32::NAN, -3.0] {
            assert_eq!(pose(odd).muzzle(), pose(0.0).muzzle());
        }
        assert_eq!(pose(7.0).muzzle(), pose(1.0).muzzle());
    }

    #[test]
    fn a_gun_with_no_way_gives_no_muzzle() {
        let flat = Pose { barrel: with_row(barrel(), 2, Vec3::new(0.0, 0.0, 0.0)), ..pose(0.0) };
        assert_eq!(flat.muzzle(), None);
    }

    #[test]
    fn the_shot_is_laid_out_as_the_eye() {
        let eye = at(0.0, 1.5, 0.0);
        let muzzle = pose(0.0).muzzle().unwrap();
        let shot = shot_from(&eye, muzzle.place, muzzle.along);
        let (right, up, forward) = (row(&shot, 0), row(&shot, 1), row(&shot, 2));
        assert_eq!((forward, row(&shot, 3)), (muzzle.along, muzzle.place));
        assert!(right.dot(up).abs() < 1e-6 && right.dot(forward).abs() < 1e-6 && up.dot(forward).abs() < 1e-6);
        assert!((right.cross(up) - forward).length() < 1e-6, "the eye's hand");
        assert!((up - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-6 && right.x > 0.9);
        assert_eq!(shot[15], 1.0);
        // along the eye's own right: nothing to square, the eye as it is
        assert_eq!(shot_from(&eye, muzzle.place, Vec3::new(1.0, 0.0, 0.0)), eye);
    }
}
