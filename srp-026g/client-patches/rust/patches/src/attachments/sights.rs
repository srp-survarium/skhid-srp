//! The first-person view of a second sight (patch 206's switch, its
//! simulation; `docs/notes/second-sights.md`, "The view (0.26g)"), as
//! 0.69d0 shows its aim-slot switch:
//!
//! - **the eye** (`align_weapon_bone_to_aim` 0x2002b8cf0 of 0.69d0): the
//!   eye record the main eye blended to the second one by the switch
//!   ([`eye`]), written at `hands_ik.aim_alignment` before patch 172 poses
//!   it, whenever it changes;
//! - **the world's zoom and near plane** (`current_camera_fov_factor`
//!   0x2006cdb40): the two sights' factors blended by the switch, then
//!   from 1 by the effective aim ([`Factors`]), at `sights.zoom`;
//! - **the weapon's own projection**: patch 161's lens by the effective aim
//!   ([`switched`], from `weapon_view.camera`);
//! - **the main optic's scope view** (+181Eh) off while the second sight is
//!   shown, at `sights.scope_view`: its idle model, the body and the hands;
//! - **the second sight's reticle**: its aimed model in place of its own in
//!   first person while it is shown ([`shown`], at `attachments.draw`).
//!
//! The blend reads the switch at a frame's time ([`FrameClock`]): the
//! simulation steps by 50 ms ticks, the frame between them. The scope view
//! is the world's (the weapon writes it), so it reads the tick's time.

#![forbid(unsafe_code)]

use second_sight::{GameTime, Sight, View, TICK_MS};
use weapon_modules::Placement;

use super::{EYE, bare};
use crate::vostok::attachments::{Shown, WeaponId};
use crate::vostok::sight_view::{self as engine, Clocks, GunZoom, SightedGun};
use crate::vostok::weapon_view::HeldGun;
use crate::vostok::{AttachmentsOnScopeViewExit, Context, Flow, Lock, NoExit, Thread};

/// A sight's two factors of update_camera's (rifle_scope's `fov_factor` and
/// `near_plane_factor`, or a weapon's own without an optic): the camera's
/// zoom and its near plane, each from 1 at the hip to the factor aimed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Factors {
    fov: f32,
    near: f32,
}

impl Factors {
    /// As the game holds them.
    pub fn new(fov: f32, near: f32) -> Factors {
        Factors { fov, near }
    }

    /// From content: each in (0, 1].
    pub fn parse(fov: f32, near: f32) -> Option<Factors> {
        let good = |factor: f32| factor > 0.0 && factor <= 1.0;
        (good(fov) && good(near)).then_some(Factors { fov, near })
    }

    /// The way to `other` at `s`: 0.69d0's `(to - from) * s + from`.
    fn toward(self, other: Factors, s: f32) -> Factors {
        Factors { fov: (other.fov - self.fov) * s + self.fov, near: (other.near - self.near) * s + self.near }
    }

    /// The zoom and the near plane's factor at `aim` (update_camera's
    /// `(f - 1) * aim + 1`, 0x1F350B..0x1F3517, 0x1F3548..0x1F355A).
    fn at(self, aim: f32) -> (f32, f32) {
        ((self.fov - 1.0) * aim + 1.0, (self.near - 1.0) * aim + 1.0)
    }
}

/// The time a frame shows: its weapon's simulation time and the frame
/// milliseconds since that time was first seen, under one tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FrameClock {
    weapon: WeaponId,
    simulation: GameTime,
    /// The frame time at which `simulation` was first seen.
    since: u32,
}

/// The frame time of `weapon` with `clocks`, and the clock kept after it.
fn frame_time(kept: Option<FrameClock>, weapon: WeaponId, clocks: Clocks) -> (FrameClock, GameTime) {
    match kept {
        Some(clock) if clock.weapon == weapon && clock.simulation == clocks.simulation => {
            let into = clocks.frame.wrapping_sub(clock.since).min(TICK_MS - 1);
            (clock, clocks.simulation.after(into))
        }
        _ => (FrameClock { weapon, simulation: clocks.simulation, since: clocks.frame }, clocks.simulation),
    }
}

/// The weapons draw on the game's thread or a task's, one at a time.
static CLOCK: Lock<Option<FrameClock>> = Lock::new(None);

/// What `gun` shows of its sights this frame: None for a gun of one sight.
fn view_now(gun: &SightedGun<'_>, thread: &Thread) -> Option<View> {
    let clocks = gun.clocks()?;
    gun.view_at(clocks.simulation)?;
    let now = CLOCK.with(thread, |kept| {
        let (clock, now) = frame_time(*kept, gun.id(), clocks);
        *kept = Some(clock);
        now
    });
    gun.view_at(now)
}

/// The second sight's share of the eye: its weight in the blend.
fn weight(view: &View) -> f32 {
    let of = |sight| if sight == Sight::Second { 1.0 } else { 0.0 };
    of(view.from) * (1.0 - view.s) + of(view.to) * view.s
}

/// The eye record of a weapon whose main eye is `main` and second `second`
/// while `view` shows: the one blended to the other by the second sight's
/// share; where the two cannot blend (other bones, a skewed offset), the
/// one shown.
fn eye(main: &Placement, second: &Placement, view: &View) -> Placement {
    main.toward(second, weight(view)).unwrap_or(match view.shown() {
        Sight::Main => *main,
        Sight::Second => *second,
    })
}

/// The eye this patch wrote last, of which weapon: the record is written
/// only when it changes, so patch 172's pose of it at rest stays.
static EYES: Lock<Option<(WeaponId, Placement)>> = Lock::new(None);

/// `weapon`'s eye record is as its load left it (`attachments.loaded`).
pub(crate) fn forget_eye(thread: &Thread, weapon: WeaponId) {
    EYES.with(thread, |kept| {
        if kept.is_some_and(|(id, _)| id == weapon) {
            *kept = None;
        }
    });
}

/// `hands_ik.aim_alignment` (before patch 172): the eye of a first-person
/// gun with a second sight.
pub fn on_sight_eye(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut gun) = SightedGun::at_eye(ctx) else { return Flow::Continue };
    let (Some(kept), Some(view)) = (gun.kept(), view_now(&gun, ctx.thread())) else { return Flow::Continue };
    let mut placed = None;
    bare(ctx.thread(), kept.named, |table, bare| {
        let Some(second) = &bare.second else { return };
        let main = bare.eye.map_or(table.own_aim(), |eye| eye.placed(EYE));
        placed = Some(eye(&main, &second.eye.placed(EYE), &view));
    });
    let Some(placed) = placed else { return Flow::Continue };
    let id = gun.id();
    let changed = EYES.with(ctx.thread(), |kept| {
        let changed = *kept != Some((id, placed));
        *kept = Some((id, placed));
        changed
    });
    if changed {
        gun.set_eye(&placed);
    }
    Flow::Continue
}

/// What `zoom`'s camera takes in `view`: None settled on the main sight
/// (the game's own values).
fn zoomed(main: Factors, second: Factors, view: &View, progress: f32) -> Option<(f32, f32)> {
    if view.from == Sight::Main && view.to == Sight::Main {
        return None;
    }
    let of = |sight| match sight {
        Sight::Main => main,
        Sight::Second => second,
    };
    Some(of(view.from).toward(of(view.to), view.s).at(view.effective_aim(progress)))
}

/// `sights.zoom`: the world's zoom and near plane of a first-person gun
/// with a second sight.
pub fn on_sight_zoom(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut zoom) = GunZoom::at(ctx) else { return Flow::Continue };
    let Some(view) = view_now(zoom.gun(), ctx.thread()) else { return Flow::Continue };
    // the optic's own factors as a second sight; the gun's own without a row
    let second = zoom.gun().kept().and_then(|kept| kept.named.wanted.second_sight)
        .and_then(|(sight, _)| engine::factors(ctx.thread(), sight)).unwrap_or(zoom.own_factors());
    if let Some((fov, near)) = zoomed(zoom.main_factors(), second, &view, zoom.gun().progress()) {
        zoom.set(fov, near);
    }
    Flow::Continue
}

/// `weapon_view.camera`'s gun with its aim as the switch lowers it, and the
/// second sight's share of the view (0 for a gun of one sight): patch 218's
/// angle and eye move blend by it as the eye does.
pub(crate) fn switched(ctx: &Context<'_>, gun: HeldGun) -> (HeldGun, f32) {
    let view = SightedGun::at_view_camera(ctx).and_then(|sighted| view_now(&sighted, ctx.thread()));
    match view {
        Some(view) => (HeldGun { progress: view.effective_aim(gun.progress), ..gun }, weight(&view)),
        None => (gun, 0.0),
    }
}

/// Whether the main optic is looked through in `view` at the aim's
/// `progress`: the game's rule (the progress past the optic's
/// `change_scope`) over the switch's effective aim, on the main sight. So a
/// switch takes the optic's overlay off on the way out and puts it back on
/// the way in as an aim-in does, never half way with the gun still turned.
fn looked_through(view: &View, progress: f32, change_scope: f32) -> bool {
    view.shown() == Sight::Main && view.effective_aim(progress) >= change_scope
}

/// `sights.scope_view` (at the tick's time: the byte is the world's):
/// settled on the main sight, the game's rule; else the scope view off
/// unless [`looked_through`].
pub fn on_scope_view(ctx: &mut Context<'_>) -> Flow<AttachmentsOnScopeViewExit> {
    let Some(mut gun) = SightedGun::at_scope_view(ctx) else { return Flow::Continue };
    let (Some(view), Some(change_scope)) = (gun.view_at_tick(), gun.change_scope_factor()) else {
        return Flow::Continue;
    };
    let settled_on_main = view.from == Sight::Main && view.to == Sight::Main;
    if settled_on_main || looked_through(&view, gun.progress(), change_scope) {
        return Flow::Continue;
    }
    gun.clear_scope_view();
    Flow::Exit(AttachmentsOnScopeViewExit::Decided)
}

/// `attachments.draw`, a first-person gun with an aimed part: the second
/// sight's aimed model while it is shown.
pub(crate) fn shown(ctx: &Context<'_>) -> Shown {
    let view = SightedGun::at_draw(ctx).and_then(|gun| view_now(&gun, ctx.thread()));
    match view.map(|view| view.shown()) {
        Some(Sight::Second) => Shown::Aimed,
        Some(Sight::Main) | None => Shown::Own,
    }
}

#[cfg(test)]
mod tests {
    use weapon_modules::{Affine, BoneIndex};

    use super::*;

    fn view(from: Sight, to: Sight, s: f32) -> View {
        View { from, to, s }
    }

    #[test]
    fn the_second_sights_share_follows_the_switch() {
        assert_eq!(weight(&view(Sight::Main, Sight::Main, 0.0)), 0.0);
        assert_eq!(weight(&view(Sight::Second, Sight::Second, 0.0)), 1.0);
        assert_eq!(weight(&view(Sight::Main, Sight::Second, 0.3)), 0.3);
        assert!((weight(&view(Sight::Second, Sight::Main, 0.3)) - 0.7).abs() < 1e-6);
    }

    fn turned_y(angle: f32, x: f32) -> Placement {
        let (s, c) = angle.sin_cos();
        Placement { bone: BoneIndex(0), offset: Affine { x: [c, 0.0, -s], y: [0.0, 1.0, 0.0], z: [s, 0.0, c], t: [x, 0.1, 0.0] } }
    }

    #[test]
    fn the_eye_blends_on_one_bone_and_jumps_between_two() {
        let (main, second) = (turned_y(0.0, 0.0), turned_y(core::f32::consts::FRAC_PI_4, -0.2));
        assert_eq!(eye(&main, &second, &view(Sight::Main, Sight::Main, 0.0)), main.toward(&second, 0.0).unwrap());
        let settled = eye(&main, &second, &view(Sight::Second, Sight::Second, 0.0));
        assert!(settled.offset.floats().iter().zip(second.offset.floats()).all(|(a, b)| (a - b).abs() < 1e-5));
        let half = eye(&main, &second, &view(Sight::Main, Sight::Second, 0.5));
        assert!((half.offset.t[0] + 0.1).abs() < 1e-6);
        let other = Placement { bone: BoneIndex(1), ..second };
        assert_eq!(eye(&main, &other, &view(Sight::Main, Sight::Second, 0.4)), main);
        assert_eq!(eye(&main, &other, &view(Sight::Main, Sight::Second, 0.6)), other);
    }

    #[test]
    fn the_zoom_blends_the_factors_then_the_effective_aim() {
        let (main, second) = (Factors::new(0.25, 0.5), Factors::new(0.8, 0.5));
        assert_eq!(zoomed(main, second, &view(Sight::Main, Sight::Main, 0.0), 1.0), None);
        // settled on the second: its own factors at the aim
        let (fov, near) = zoomed(main, second, &view(Sight::Second, Sight::Second, 0.0), 1.0).unwrap();
        assert!((fov - 0.8).abs() < 1e-6 && (near - 0.5).abs() < 1e-6);
        // half way: the factors' middle, the aim down by 0.5
        let (fov, _) = zoomed(main, second, &view(Sight::Main, Sight::Second, 0.5), 1.0).unwrap();
        assert!((fov - (1.0 + (0.525 - 1.0) * 0.5)).abs() < 1e-6);
        // before the first tick of a switch: the main's, as the game's
        let (fov, near) = zoomed(main, second, &view(Sight::Main, Sight::Second, 0.0), 0.7).unwrap();
        assert_eq!((fov, near), main.at(0.7));
        assert!(Factors::parse(0.0, 0.5).is_none() && Factors::parse(0.5, 1.5).is_none());
        assert_eq!(Factors::parse(0.8, 0.5), Some(Factors::new(0.8, 0.5)));
    }

    #[test]
    fn the_optic_is_looked_through_only_as_an_aim_in_reaches_it() {
        let through = |from, to, s| looked_through(&view(from, to, s), 1.0, 0.9);
        assert!(through(Sight::Main, Sight::Second, 0.05), "on the way out, before the aim is lowered past it");
        assert!(!through(Sight::Main, Sight::Second, 0.2), "on the way out, lowered");
        assert!(!through(Sight::Second, Sight::Second, 0.0));
        assert!(!through(Sight::Second, Sight::Main, 0.6), "back past half way, the gun still turned");
        assert!(through(Sight::Second, Sight::Main, 0.95), "back, the aim raised past it");
    }

    #[test]
    fn a_frame_between_ticks_is_ahead_of_its_tick_by_under_one() {
        let weapon = WeaponId::numbered(7);
        let at = |simulation, frame| Clocks { simulation: GameTime(simulation), frame };
        let (clock, now) = frame_time(None, weapon, at(1000, 77_000));
        assert_eq!(now, GameTime(1000));
        let (clock, now) = frame_time(Some(clock), weapon, at(1000, 77_016));
        assert_eq!(now, GameTime(1016));
        let (_, now) = frame_time(Some(clock), weapon, at(1000, 77_300));
        assert_eq!(now, GameTime(1049), "a late tick: under one tick ahead");
        let (clock, now) = frame_time(Some(clock), weapon, at(1050, 77_310));
        assert_eq!((now, clock.since), (GameTime(1050), 77_310), "the next tick starts again");
        let (_, now) = frame_time(Some(clock), WeaponId::numbered(8), at(1050, 77_320));
        assert_eq!(now, GameTime(1050), "another weapon");
    }
}
