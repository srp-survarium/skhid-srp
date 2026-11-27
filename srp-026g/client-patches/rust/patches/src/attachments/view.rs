//! The first-person projection as 0.69d0's camera makes it
//! (`first_person_view_camera_controller::update_camera` 0x2002C2D70 of its
//! exe; `docs/notes/attachments-scopes.md`, "Why the EOTech looks small on
//! the A-545"): the world at the view's FOV and the sight's zoom, and
//! everything held, the gun, its parts and its optic with its reticle, at a
//! projection of its own, 65 degrees times the gun item's factor, from its
//! hip one to its aimed one by the aim; anything that is no gun at 65
//! degrees. The camera samples that lens each frame; the scene's draw
//! command carries it to the render thread, whose foreground pass takes it
//! in place of the world's.
//!
//! One exception: a gun with an optic of its own (0.26g's PSO-1, Leupold,
//! Giperon, EOTech and Walther of the SVD, VSS, VSK-94, Rem 700, SV-98,
//! VSSK, HK416, MSBS, Kriss Vector and P90) is drawn at the world's
//! projection, as before: 0.26g made those models and their eye relief
//! (the gun's `aim_offset`) for it, and 0.69d0 has none of them.
//!
//! Patch 218's player settings (`crate::weapon_view_options`,
//! `docs/notes/weapon-view-settings.md`) scale those angles (1 by default:
//! 0.69d0's exactly), or draw everything held at the world's projection as
//! revision 73 did; and its sight distance moves the eye of iron sights and
//! of sights in the gun's eye plane along its own line toward revision 73's
//! eye: the alignment the game makes at the hands' IK is moved by it
//! ([`eye_shift`], [`eye_aligned`]), the camera never.
#![forbid(unsafe_code)]

use weapon_modules::Relief;

use crate::vostok::{Context, Flow, Lock, NoExit, attachments::WeaponId, trig, weapon_view as engine};
use crate::weapon_view_options::{Projection, Scale, SightDistance, ViewOptions};
use engine::{HeldGun, Optic, ViewId};

/// A projection's vertical focal length, 1 / tan(fov / 2).
#[derive(Clone, Copy)]
pub(crate) struct Focal(f32);

/// A gun item's two factors of 0.69d0's 65 degrees: at the hip, fully aimed.
#[derive(Clone, Copy)]
pub(crate) struct Profile {
    hip: f32,
    aimed: f32,
}

/// A gun's row of `srp_weapon_views`: its profile, and its iron sights'
/// `aim_offset` of revision 73 where the content moved them to 0.69d0's eye
/// (none for a gun with an optic of its own).
#[derive(Clone, Copy)]
pub(crate) struct GunView {
    profile: Profile,
    aim_offset_026g: Option<f32>,
}

impl GunView {
    /// None for an offset that is no number of a metre or less.
    pub(crate) fn parse(profile: Profile, aim_offset_026g: Option<f32>) -> Option<GunView> {
        match aim_offset_026g {
            Some(offset) if offset.is_nan() || offset.abs() > 1.0 => None,
            _ => Some(GunView { profile, aim_offset_026g }),
        }
    }
}

impl Profile {
    /// What 0.69d0 holds anything with that gives no factors: 1 and 1
    /// (update_camera's 1.0, 0x2002C2DC6).
    const NONE: Profile = Profile { hip: 1.0, aimed: 1.0 };

    pub(crate) fn parse([hip, aimed]: [f32; 2]) -> Option<Self> {
        (hip > 0.0 && hip <= 1.0 && aimed > 0.0 && aimed <= 1.0).then_some(Self { hip, aimed })
    }

    /// The factors, each times the player's scale (1: 0.69d0's own, the
    /// same floats), interpolated by the aim, then the angle (0.69d0's order).
    fn focal_at(self, progress: f32, hip: Scale, aimed: Scale) -> Option<Focal> {
        let (hip, aimed) = (self.hip * hip.get(), self.aimed * aimed.get());
        let factor = hip + (aimed - hip) * progress.clamp(0.0, 1.0);
        trig::focal_length(f64::from(factor) * 32.5_f64.to_radians()).map(Focal)
    }
}

impl Focal {
    /// The command's field: 0 for no lens.
    pub(crate) fn decode(value: f32) -> Option<Self> {
        (value > 0.0).then_some(Self(value))
    }

    pub(crate) fn value(self) -> f32 {
        self.0
    }

    /// The two scale terms in place of the world projection's, its aspect kept.
    pub(crate) fn diagonal(self, world_x: f32, world_y: f32) -> [f32; 2] {
        [self.0 * (world_x / world_y), self.0]
    }
}

/// How the sight distance moves a sight's eye.
#[derive(Clone, Copy, Debug, PartialEq)]
enum EyeMove {
    /// The gun's own iron sights: `step` metres along the line take its eye
    /// from the content's `aim_offset` to revision 73's.
    Iron { step: f32 },
    /// A sight in the gun's eye plane: the table's two eyes.
    InPlane(Relief),
    /// Nothing known of another eye: it stays.
    Fixed,
}

/// What a gun is aimed through, for the player's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Aim {
    /// Iron sights, a collimator, the rail iron sight: the sights' angle.
    Sights(EyeMove),
    /// A magnified scope: the scopes' angle, its own eye.
    Scope,
}

impl Aim {
    /// The eye of `relief` (a part's, the table's).
    fn of(relief: Relief) -> Aim {
        match relief {
            Relief::Own => Aim::Scope,
            in_plane @ Relief::InGunPlane { .. } => Aim::Sights(EyeMove::InPlane(in_plane)),
        }
    }

    fn scale(self, sights: Scale, scopes: Scale) -> Scale {
        match self {
            Aim::Sights(_) => sights,
            Aim::Scope => scopes,
        }
    }

    /// How far the eye goes along its line (an `aim_offset` more).
    fn shift(self, distance: SightDistance) -> f32 {
        let share = distance.share();
        if share.is_nan() || share <= 0.0 {
            return 0.0;
        }
        match self {
            Aim::Sights(EyeMove::Iron { step }) => step * share.min(1.0),
            Aim::Sights(EyeMove::InPlane(relief)) => relief.shift(share),
            Aim::Sights(EyeMove::Fixed) | Aim::Scope => 0.0,
        }
    }
}

/// The gun's main sight and its second one (patch 206), with the second
/// one's share of the view (0 none, 1 all).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Aims {
    main: Aim,
    second: Option<(Aim, f32)>,
}

impl Aims {
    fn scale(self, sights: Scale, scopes: Scale) -> Scale {
        let main = self.main.scale(sights, scopes);
        match self.second {
            Some((second, share)) => main.toward(second.scale(sights, scopes), share),
            None => main,
        }
    }

    fn shift(self, distance: SightDistance) -> f32 {
        let main = self.main.shift(distance);
        match self.second {
            Some((second, share)) if share > 0.0 => main + (second.shift(distance) - main) * share.min(1.0),
            _ => main,
        }
    }
}

/// What the player's settings make of a held gun this frame: the lens the
/// foreground takes (None: the world's projection) and the eye's move.
#[derive(Clone, Copy)]
struct Held {
    lens: Option<Focal>,
    shift: f32,
}

/// The aims of `gun`, whose second sight has `share` of the view; None for
/// a gun with an optic of its own, which keeps the world's projection and
/// its own eye.
fn aims(gun: &HeldGun, eyes: Option<(Option<Relief>, Option<Relief>)>, share: f32) -> Option<Aims> {
    let (main_eye, second_eye) = eyes.unwrap_or((None, None));
    let main = match gun.optic {
        Optic::Own => return None,
        Optic::None => {
            let step = gun.view.and_then(|view| view.aim_offset_026g).map(|then| then - gun.aim_offset);
            Aim::Sights(step.filter(|step| step.is_finite()).map_or(EyeMove::Fixed, |step| EyeMove::Iron { step }))
        }
        Optic::Fitted => main_eye.map_or(Aim::Sights(EyeMove::Fixed), Aim::of),
    };
    Some(Aims { main, second: second_eye.map(|eye| (Aim::of(eye), share)) })
}

/// The lens and the eye's move of `gun`.
fn held(gun: &HeldGun, aims: Option<Aims>, options: ViewOptions) -> Held {
    let Some(aims) = aims else { return Held { lens: None, shift: 0.0 } };
    let lens = match options.projection {
        Projection::World => None,
        Projection::Own { hip, sights, scopes } => {
            let profile = gun.view.map_or(Profile::NONE, |view| view.profile);
            profile.focal_at(gun.progress, hip, aims.scale(sights, scopes))
        }
    };
    Held { lens, shift: aims.shift(options.distance) }
}

/// The lens of anything that is no gun: 65 degrees times the hip's scale.
fn bare_lens(options: ViewOptions) -> Option<Focal> {
    match options.projection {
        Projection::World => None,
        Projection::Own { hip, .. } => Profile::NONE.focal_at(0.0, hip, Scale::ONE),
    }
}

/// This frame's lens and the view it is for: written by the camera on the
/// game's thread, taken by the scene's draw command on it.
static CAMERA: Lock<Option<(ViewId, Focal)>> = Lock::new(None);
/// The lens of the draw command the render thread runs.
static DRAW: Lock<Option<Focal>> = Lock::new(None);
/// The held gun's eye move, written by the camera (the console's values
/// are the game thread's), taken by the hands' alignment of that gun on the
/// thread mt_draw runs on: none for anything else.
static EYE: Lock<Option<(WeaponId, f32)>> = Lock::new(None);
/// The move of the alignment being made, the camera's line times it: taken
/// before calculate_aim_alignment, given back at its return on the same thread.
static PENDING: Lock<Option<[f32; 3]>> = Lock::new(None);

/// `game.tick`: a frame without a first-person camera has no lens.
pub fn on_view_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    CAMERA.with(ctx.thread(), |camera| *camera = None);
    Flow::Continue
}

/// `weapon_view.camera`: a gun in first-person hands.
pub fn on_view_camera(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let options = ViewOptions::now(ctx.thread());
    let Some((view, gun)) = engine::gun_camera(ctx) else {
        CAMERA.with(ctx.thread(), |camera| *camera = None);
        return Flow::Continue;
    };
    let (gun, share) = super::sights::switched(ctx, gun);
    let mut eyes = None;
    if let Some(named) = gun.named {
        super::bare(ctx.thread(), named, |_, bare| {
            eyes = Some((bare.eye.map(|eye| eye.relief()), bare.second.map(|second| second.eye.relief())));
        });
    }
    let held = held(&gun, aims(&gun, eyes, share), options);
    CAMERA.with(ctx.thread(), |camera| *camera = held.lens.map(|lens| (view, lens)));
    let moved = (held.shift != 0.0).then_some((gun.weapon, held.shift));
    EYE.with(ctx.thread(), |eye| *eye = moved);
    Flow::Continue
}

/// `weapon_view.no_gun`: anything else in first-person hands, or nothing.
pub fn on_view_no_gun(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let options = ViewOptions::now(ctx.thread());
    let sample = engine::bare_camera(ctx).and_then(|view| Some((view, bare_lens(options)?)));
    CAMERA.with(ctx.thread(), |camera| *camera = sample);
    EYE.with(ctx.thread(), |eye| *eye = None);
    Flow::Continue
}

pub fn on_view_submit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let sample = CAMERA.with(ctx.thread(), |camera| *camera);
    engine::submit(ctx, sample);
    Flow::Continue
}

pub fn on_view_begin(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let focal = engine::queued(ctx);
    DRAW.with(ctx.thread(), |draw| *draw = focal);
    Flow::Continue
}

pub fn on_view_end(ctx: &mut Context<'_>) -> Flow<NoExit> {
    DRAW.with(ctx.thread(), |draw| *draw = None);
    Flow::Continue
}

pub fn on_foreground(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(focal) = DRAW.with(ctx.thread(), |draw| *draw) {
        engine::project(ctx, focal);
    }
    Flow::Continue
}

/// The alignment's move for an eye `shift` metres further along its line
/// (an `aim_offset` that much larger), the camera's line `line` (the Item
/// bone's third row): calculate_aim_alignment gives inverse(T(0, 0, a) x
/// half turn x locator x bone) x Item, and with a + shift that is the same
/// times Item^-1 x T(0, 0, -shift) x Item, the translation -shift x `line`.
fn alignment_move(shift: f32, line: [f32; 3]) -> [f32; 3] {
    line.map(|axis| -shift * axis)
}

/// `hands_ik.aim_alignment` (patch 218, after patch 172): the move of this
/// alignment, when the camera has one for this weapon.
pub(crate) fn eye_shift(ctx: &Context<'_>) {
    let aligning = engine::aligning(ctx);
    let wanted = EYE.with(ctx.thread(), |eye| *eye);
    let by = match (aligning, wanted) {
        (Some((weapon, line)), Some((moved, shift))) if weapon == moved => Some(alignment_move(shift, line)),
        _ => None,
    };
    PENDING.with(ctx.thread(), |pending| *pending = by);
}

/// `hands_ik.aim_aligned`: the alignment just made, moved.
pub(crate) fn eye_aligned(ctx: &Context<'_>) {
    if let Some(by) = PENDING.with(ctx.thread(), |pending| pending.take()) {
        engine::aligned(ctx, by);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gun(optic: Optic, progress: f32) -> HeldGun {
        HeldGun {
            weapon: WeaponId::numbered(1),
            named: None,
            view: GunView::parse(Profile::parse([1.0, 0.5]).unwrap(), Some(-0.05)),
            progress,
            optic,
            aim_offset: -0.4789,
        }
    }

    fn focal(degrees: f32) -> f32 {
        1.0 / (degrees.to_radians() / 2.0).tan()
    }

    fn lens(gun: &HeldGun, eyes: Option<(Option<Relief>, Option<Relief>)>, options: ViewOptions) -> Option<Focal> {
        held(gun, aims(gun, eyes, 0.0), options).lens
    }

    #[test]
    fn the_factors_interpolate_before_the_angle_and_the_aspect_stays() {
        for (progress, angle) in [(0.0, 65.0_f32), (0.5, 48.75), (1.0, 32.5)] {
            let focal = Profile::parse([1.0, 0.5]).unwrap().focal_at(progress, Scale::ONE, Scale::ONE).unwrap();
            assert!((focal.value() - 1.0 / (angle.to_radians() / 2.0).tan()).abs() < 1e-6);
            for aspect in [4.0 / 3.0, 16.0 / 9.0, 21.0 / 9.0] {
                let [x, y] = focal.diagonal(2.0 / aspect, 2.0);
                assert!((x * aspect - y).abs() < 1e-6);
            }
        }
    }

    /// Revision 74's projection, the same floats: the scales are 1 exactly.
    #[test]
    fn the_defaults_are_revision_74_s_lens_bit_for_bit() {
        let rev74 = |profile: Profile, progress: f32| {
            let factor = profile.hip + (profile.aimed - profile.hip) * progress.clamp(0.0, 1.0);
            trig::focal_length(f64::from(factor) * 32.5_f64.to_radians()).unwrap().to_bits()
        };
        for factors in [[1.0, 0.5], [0.75, 0.6], [1.0, 0.35], [0.4, 0.4], [0.65, 0.55]] {
            let profile = Profile::parse(factors).unwrap();
            for progress in [0.0, 0.25, 0.5, 0.9, 1.0] {
                let ours = profile.focal_at(progress, Scale::ONE, Scale::ONE).unwrap().value().to_bits();
                assert_eq!(ours, rev74(profile, progress));
            }
        }
        let magnified = Some((Some(Relief::Own), None));
        for (optic, eyes) in [(Optic::None, None), (Optic::Fitted, magnified)] {
            let held = held(&gun(optic, 0.7), aims(&gun(optic, 0.7), eyes, 0.0), ViewOptions::DEFAULT);
            assert_eq!(held.lens.unwrap().value().to_bits(), rev74(Profile::parse([1.0, 0.5]).unwrap(), 0.7));
            assert_eq!(held.shift.to_bits(), 0.0_f32.to_bits(), "no eye moved");
        }
        assert_eq!(bare_lens(ViewOptions::DEFAULT).unwrap().value().to_bits(), rev74(Profile::NONE, 0.0));
    }

    #[test]
    fn a_gun_with_its_own_optic_keeps_the_world_projection() {
        let aimed = 1.0 / 16.25_f32.to_radians().tan();
        assert!(lens(&gun(Optic::None, 1.0), None, ViewOptions::DEFAULT).is_some_and(|focal| (focal.value() - aimed).abs() < 1e-5));
        let collimator = Some((Some(Relief::InGunPlane { along: -0.2, along_026g: -0.1 }), None));
        assert!(lens(&gun(Optic::Fitted, 1.0), collimator, ViewOptions::DEFAULT).is_some_and(|focal| (focal.value() - aimed).abs() < 1e-5));
        assert!(lens(&gun(Optic::Own, 1.0), None, ViewOptions::DEFAULT).is_none());
        let unknown = HeldGun { view: None, ..gun(Optic::None, 1.0) };
        assert!(lens(&unknown, None, ViewOptions::DEFAULT).is_some_and(|focal| (focal.value() - 1.0 / 32.5_f32.to_radians().tan()).abs() < 1e-5));
        assert_eq!(held(&gun(Optic::Own, 1.0), aims(&gun(Optic::Own, 1.0), None, 0.0), ViewOptions::of(65.0, 32.5, 32.5, false, 100.0)).shift, 0.0);
    }

    #[test]
    fn the_settings_scale_each_angle_by_what_is_held() {
        let options = ViewOptions::of(90.0, 52.0, 20.0, false, 0.0);
        let iron = gun(Optic::None, 1.0);
        assert!((lens(&iron, None, options).unwrap().value() - focal(52.0)).abs() < 1e-5);
        assert!((lens(&gun(Optic::None, 0.0), None, options).unwrap().value() - focal(90.0)).abs() < 1e-5);
        let scope = Some((Some(Relief::Own), None));
        assert!((lens(&gun(Optic::Fitted, 1.0), scope, options).unwrap().value() - focal(20.0)).abs() < 1e-5);
        // a pistol keeps its ratio: 0.6 of 65 aimed at 0.69d0's, 52 x 1.2 here
        let pistol = HeldGun { view: GunView::parse(Profile::parse([0.75, 0.6]).unwrap(), None), ..iron };
        assert!((lens(&pistol, None, options).unwrap().value() - focal(62.4)).abs() < 1e-4);
        assert!((bare_lens(options).unwrap().value() - focal(90.0)).abs() < 1e-5);
        // the world's projection: no lens at all, for anything held
        let world = ViewOptions::of(65.0, 32.5, 32.5, true, 0.0);
        assert!(lens(&iron, None, world).is_none() && bare_lens(world).is_none());
        // a switch half way from a scope to a collimator: the angle half way
        let both = Some((Some(Relief::Own), Some(Relief::InGunPlane { along: -0.2, along_026g: -0.1 })));
        let half = held(&gun(Optic::Fitted, 1.0), aims(&gun(Optic::Fitted, 1.0), both, 0.5), options).lens.unwrap();
        let want = 1.0 / (f64::from((20.0_f32 / 32.5 + 52.0 / 32.5) / 2.0 * 0.5 * 32.5).to_radians()).tan();
        assert!((f64::from(half.value()) - want).abs() < 1e-4);
    }

    /// The checkbox and 100 %: revision 73's eye, for iron sights by the
    /// content's two aim_offsets, in the gun's eye plane by the table's.
    #[test]
    fn at_100_percent_the_eye_is_revision_73_s() {
        let rev73 = ViewOptions::of(65.0, 32.5, 32.5, true, 100.0);
        let pm = gun(Optic::None, 1.0);
        let held_pm = held(&pm, aims(&pm, None, 0.0), rev73);
        assert!(held_pm.lens.is_none());
        assert!((pm.aim_offset + held_pm.shift - -0.05).abs() < 1e-6, "the PM's iron sights at -0.05, revision 73's");
        let m4 = Relief::InGunPlane { along: -0.20781, along_026g: -0.0799 };
        let fitted = gun(Optic::Fitted, 1.0);
        let shift = held(&fitted, aims(&fitted, Some((Some(m4), None)), 0.0), rev73).shift;
        assert!((-0.20781 + shift - -0.0799).abs() < 1e-6);
        let half = held(&fitted, aims(&fitted, Some((Some(m4), None)), 0.0), ViewOptions::of(65.0, 32.5, 32.5, false, 50.0));
        assert!((half.shift - (0.20781 - 0.0799) / 2.0).abs() < 1e-6);
        // a magnified scope and a gun without a row: no move
        let scope = held(&fitted, aims(&fitted, Some((Some(Relief::Own), None)), 0.0), rev73);
        assert_eq!(scope.shift, 0.0);
        let unknown = HeldGun { view: None, ..pm };
        assert_eq!(held(&unknown, aims(&unknown, None, 0.0), rev73).shift, 0.0);
    }

    #[test]
    fn the_alignment_moves_back_along_the_camera_s_line() {
        assert_eq!(alignment_move(0.1, [0.0, 0.0, 1.0]), [-0.0, -0.0, -0.1]);
        assert_eq!(alignment_move(0.0, [0.3, 0.4, 0.5]).map(f32::abs), [0.0; 3]);
        assert!(GunView::parse(Profile::NONE, Some(1.5)).is_none() && GunView::parse(Profile::NONE, Some(f32::NAN)).is_none());
    }
}
