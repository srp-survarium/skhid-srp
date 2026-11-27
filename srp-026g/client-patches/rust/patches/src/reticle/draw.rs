//! The reticle drawn in the local player's aimed optic (`docs/notes/reticle-choice.md`, "The
//! drawing"; the game's side is `vostok::reticle`).
//!
//! - **When.** At each draw of a weapon (`attachments.draw`, after the game added or removed its
//!   aimed optic, and patch 161 its parts): while the fitted optic's aimed model is in the scene
//!   (only the local player's gun, aimed, in the first person), or a second sight's (patch 161
//!   draws it in its place while it is looked through), and the table has that optic's dict id,
//!   the reticle model is in the scene too; otherwise it is out. It leaves with the weapon
//!   (`attachments.hidden`, `attachments.culled`, `attachments.release`, and Artefact carry's
//!   hiding of the gun, [`weapon_hidden`]).
//! - **Where.** The optic's `aim` place in its model's frame (the modules table's locator: the
//!   reticle's place, where 0.69d0 draws it in the optic's `reticle` slot), times the size's
//!   scale about it, times the aimed model's frame this draw ([`place`]).
//! - **How.** The pattern's model with its material appended once, and the appended material's
//!   emission the colour times the brightness's multiplier ([`emission`]), set when it changes.
//! - **Which models.** A pattern is asked for the first time a draw wants it, on the game's
//!   thread ([`on_tick`]), one query at a time, and kept for the process ([`Patterns`]).
//!
//! What a match computes does not change: the reticle is drawn alone.

use core::ffi::CStr;

use weapon_modules::{Affine, AttachEye, DictId, Kind};

use super::table::{self, Brightness};
use crate::vostok::attachments::{self, WeaponId};
use crate::vostok::resources::{Arrived, QueryResults, QueryTag};
use crate::vostok::reticle::{self, DrawnGun, Emission, Emitter, Leaving, Pattern, ReticleScene};
use crate::vostok::{Context, Flow, Lock, NoExit, Thread};

/// The patterns kept at once: every pattern of every scope's list (0.69d0 has ten).
const MOST: usize = 16;
/// The pattern models' one material (pack `weapon-modules`), as their settings name it.
const MATERIAL: &CStr = c"weapons/reticles_069/reticle";
/// The emission's multiplier for each brightness: 6, 0.69d0's default, is the players' collimator
/// emitter's (20); each step is a factor of the square root of two, so 0 is an eighth of it and
/// 10 four times.
const MULTIPLIERS: [f32; 11] = [2.5, 3.535_534, 5.0, 7.071_068, 10.0, 14.142_136, 20.0, 28.284_271, 40.0, 56.568_542, 80.0];

/// The appended material's emission for a colour and a brightness.
pub fn emission(rgb: [f32; 3], brightness: Brightness) -> Emission {
    let multiplier = MULTIPLIERS[usize::from(brightness.get()).min(MULTIPLIERS.len() - 1)];
    Emission([rgb[0] * multiplier, rgb[1] * multiplier, rgb[2] * multiplier, 1.0])
}

/// The reticle's frame: the pattern scaled by `scale` about its aiming point, at the optic's `aim`
/// place `aim` in its model's frame, in the aimed model's frame `aimed`.
pub fn place(scale: f32, aim: &Affine, aimed: &Affine) -> Affine {
    let scaled = Affine { x: [scale, 0.0, 0.0], y: [0.0, scale, 0.0], z: [0.0, 0.0, scale], t: [0.0; 3] };
    scaled.then(aim).then(aimed)
}

/// What a draw wants shown: a pattern's model, where, and its emission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Want {
    pub model: &'static CStr,
    pub at: Affine,
    pub emission: Emission,
}

/// A pattern's load.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Load<P> {
    /// A draw wants it; not asked yet.
    Wanted,
    /// Its query is out.
    Asked,
    /// Here; its material appended to its model or not yet, and the emission set last.
    Ready { pattern: P, prepared: Prepared },
    /// Its model or material did not load: never drawn.
    Missing,
}

/// A loaded pattern's material on its model.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Prepared {
    NotAppended,
    Appended { emitting: Option<Emission> },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Slot<P> {
    model: &'static CStr,
    load: Load<P>,
}

/// The reticle shown: whose weapon's aimed optic it is in, and which pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shown<P> {
    weapon: WeaponId,
    pattern: P,
}

/// What a draw does to the scene, in this order.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Steps<P> {
    /// A pattern out of the scene.
    pub hide: Option<P>,
    /// The pattern's material appended to its model.
    pub append: Option<P>,
    /// The appended material's emission.
    pub emit: Option<(P, Emission)>,
    /// A pattern in the scene at a frame.
    pub show: Option<(P, Affine)>,
}

/// The patterns and the one reticle shown. `P` is a loaded pattern ([`Pattern`] in the game).
#[derive(Debug)]
pub struct Patterns<P> {
    slots: [Option<Slot<P>>; MOST],
    shown: Option<Shown<P>>,
}

impl<P: Copy + PartialEq> Patterns<P> {
    pub const NEW: Patterns<P> = Patterns { slots: [const { None }; MOST], shown: None };

    /// The place of `model`'s slot, a new one (wanted) when it has none; None when every place
    /// is taken.
    fn slot(&mut self, model: &'static CStr) -> Option<usize> {
        if let Some(at) = self.slots.iter().position(|slot| slot.is_some_and(|slot| slot.model == model)) {
            return Some(at);
        }
        let free = self.slots.iter().position(Option::is_none)?;
        self.slots[free] = Some(Slot { model, load: Load::Wanted });
        Some(free)
    }

    /// A draw of `weapon`: `want` is what its aimed optic shows, None when it shows no reticle.
    pub fn draw(&mut self, weapon: WeaponId, want: Option<Want>) -> Steps<P> {
        let mut steps = Steps { hide: None, append: None, emit: None, show: None };
        let ready = want.and_then(|want| {
            let at = self.slot(want.model)?;
            match self.slots[at]?.load {
                Load::Ready { pattern, prepared } => Some((at, pattern, prepared, want)),
                Load::Wanted | Load::Asked | Load::Missing => None,
            }
        });
        let Some((at, pattern, prepared, want)) = ready else {
            // nothing to show for this weapon: its own reticle goes, another weapon's stays
            // unless this one wanted one (one aimed view at a time)
            if let Some(shown) = self.shown
                && (shown.weapon == weapon || want.is_some())
            {
                steps.hide = Some(shown.pattern);
                self.shown = None;
            }
            return steps;
        };
        if let Some(shown) = self.shown
            && shown.pattern != pattern
        {
            steps.hide = Some(shown.pattern);
        }
        let emitting = match prepared {
            Prepared::NotAppended => {
                steps.append = Some(pattern);
                None
            }
            Prepared::Appended { emitting } => emitting,
        };
        if emitting != Some(want.emission) {
            steps.emit = Some((pattern, want.emission));
        }
        self.slots[at] = Some(Slot {
            model: want.model,
            load: Load::Ready { pattern, prepared: Prepared::Appended { emitting: Some(want.emission) } },
        });
        steps.show = Some((pattern, want.at));
        self.shown = Some(Shown { weapon, pattern });
        steps
    }

    /// `weapon` leaves the scene: its reticle with it.
    pub fn leave(&mut self, weapon: WeaponId) -> Option<P> {
        let shown = self.shown.filter(|shown| shown.weapon == weapon)?;
        self.shown = None;
        Some(shown.pattern)
    }

    /// The next pattern to ask for, when no query is out.
    pub fn next_ask(&mut self) -> Option<&'static CStr> {
        if self.slots.iter().flatten().any(|slot| slot.load == Load::Asked) {
            return None;
        }
        let slot = self.slots.iter_mut().flatten().find(|slot| slot.load == Load::Wanted)?;
        slot.load = Load::Asked;
        Some(slot.model)
    }

    /// The query out came back: its pattern, or None when it did not load.
    pub fn arrived(&mut self, pattern: Option<P>) {
        if let Some(slot) = self.slots.iter_mut().flatten().find(|slot| slot.load == Load::Asked) {
            slot.load = match pattern {
                Some(pattern) => Load::Ready { pattern, prepared: Prepared::NotAppended },
                None => Load::Missing,
            };
        }
    }
}

/// The game's patterns and the emission parameter, registered with the first append.
struct Reticles {
    patterns: Patterns<Pattern>,
    emitter: Option<Emitter>,
}

static RETICLES: Lock<Reticles> = Lock::new(Reticles { patterns: Patterns::NEW, emitter: None });

/// What `gun`'s aimed optic shows this draw: the fitted optic's aimed model, else a second sight's,
/// with its reticle from the table.
fn wanted(gun: &DrawnGun<'_>) -> Option<Want> {
    let thread = gun.thread();
    let named = attachments::kept(thread, gun.id())?.named;
    let (dict, kind, aimed) = match (gun.optic_aimed(), named.wanted.scope) {
        (Some(aimed), Some(scope)) => (scope, Kind::Scope, aimed),
        _ => (named.wanted.second_sight?.0, Kind::SecondSight, gun.part_aimed()?),
    };
    let aim = aim_place(thread, named.gun, dict, kind)?;
    let drawn = table::drawn_for_dict(thread, dict.get())?;
    Some(Want {
        model: drawn.model,
        at: place(drawn.scale, &aim, &aimed),
        emission: emission(drawn.rgb, drawn.brightness),
    })
}

/// The optic's `aim` place in its model's frame: the modules table's, the same in every slot it
/// takes (the porter refuses a collimator with two).
fn aim_place(thread: &Thread, gun: DictId, dict: DictId, kind: Kind) -> Option<Affine> {
    let module = attachments::modules(thread)?.gun(gun)?.fit(dict, kind)?;
    let eye = module.attaches().find_map(|attach| attach.eye())?;
    let (AttachEye::Own(aim) | AttachEye::InGunPlane(aim)) = eye;
    Some(aim)
}

fn run(scene: &ReticleScene<'_>, emitter: &mut Option<Emitter>, steps: Steps<Pattern>) {
    if let Some(pattern) = steps.hide {
        scene.hide(pattern);
    }
    if steps.append.is_some() || steps.emit.is_some() {
        *emitter = emitter.or_else(|| scene.register());
    }
    if let Some(pattern) = steps.append {
        scene.append(pattern);
    }
    if let (Some((pattern, emission)), Some(emitter)) = (steps.emit, *emitter) {
        scene.emit(emitter, pattern, emission);
    }
    if let Some((pattern, at)) = steps.show {
        scene.show(pattern, &at);
    }
}

/// `attachments.draw` (after patch 161's): the reticle in or out with the aimed optic.
pub fn on_draw(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(gun) = DrawnGun::at_draw(ctx) else { return Flow::Continue };
    let Some(scene) = gun.scene() else { return Flow::Continue };
    let want = wanted(&gun);
    RETICLES.with(gun.thread(), |reticles| {
        let steps = reticles.patterns.draw(gun.id(), want);
        run(&scene, &mut reticles.emitter, steps);
    });
    Flow::Continue
}

fn leave(thread: &Thread, weapon: WeaponId, scene: Option<&ReticleScene<'_>>) {
    RETICLES.with(thread, |reticles| {
        if let (Some(pattern), Some(scene)) = (reticles.patterns.leave(weapon), scene) {
            scene.hide(pattern);
        }
    });
}

/// `attachments.hidden`.
pub fn on_hidden(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some((weapon, scene)) = reticle::leaving(ctx, Leaving::Hidden) {
        leave(ctx.thread(), weapon, Some(&scene));
    }
    Flow::Continue
}

/// `attachments.culled`.
pub fn on_culled(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some((weapon, scene)) = reticle::leaving(ctx, Leaving::Culled) {
        leave(ctx.thread(), weapon, Some(&scene));
    }
    Flow::Continue
}

/// `attachments.release`: the weapon goes; its reticle out of the scene while the scene lives.
pub fn on_release(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some((weapon, scene)) = reticle::released(ctx) {
        leave(ctx.thread(), weapon, scene.as_ref());
    }
    Flow::Continue
}

/// Artefact carry took `weapon`'s models out of the scene and skips its draw: its reticle goes too
/// (the next draw puts it back).
pub fn weapon_hidden(thread: &Thread, weapon: WeaponId, scene: &ReticleScene<'_>) {
    leave(thread, weapon, Some(scene));
}

/// `game.tick`: the next pattern a draw wanted is asked for, on the game's thread.
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Some(model) = RETICLES.with(thread, |reticles| reticles.patterns.next_ask()) {
        reticle::ask(thread, model, MATERIAL, crate::vostok::reticle_on_loaded_entry());
    }
    Flow::Continue
}

/// The patch's queries come back (on the game's thread).
pub fn on_loaded(results: QueryResults<'_>, tag: QueryTag) {
    let thread = results.thread();
    if let Arrived::Assets(assets) = results.arrived(tag) {
        let pattern = reticle::arrived(&assets);
        RETICLES.with(thread, |reticles| reticles.patterns.arrived(pattern));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOT: &CStr = c"weapons/reticles_069/dot";
    const CROSS: &CStr = c"weapons/reticles_069/ncstar_cross";
    const GUN: WeaponId = WeaponId::numbered(1);
    const OTHER: WeaponId = WeaponId::numbered(2);

    fn want(model: &'static CStr, red: f32) -> Want {
        Want { model, at: Affine::translation(0.0, 0.0, 1.0), emission: Emission([red, 0.0, 0.0, 1.0]) }
    }

    fn loaded(model: &'static CStr, pattern: u32) -> Patterns<u32> {
        let mut patterns = Patterns::NEW;
        assert_eq!(patterns.draw(GUN, Some(want(model, 1.0))), Steps::default());
        assert_eq!(patterns.next_ask(), Some(model));
        patterns.arrived(Some(pattern));
        patterns
    }

    #[test]
    fn brightness_six_is_the_emitters_twenty() {
        let at = |b| emission([1.0, 0.5, 0.0], Brightness::new(b).unwrap()).0;
        assert_eq!(at(6), [20.0, 10.0, 0.0, 1.0]);
        assert!((at(10)[0] - 80.0).abs() < 1e-3 && (at(0)[0] - 2.5).abs() < 1e-3);
        assert!((0..10).all(|b| at(b)[0] < at(b + 1)[0]));
    }

    #[test]
    fn the_size_scales_about_the_aim_place() {
        let aim = Affine::translation(0.0, 0.05, 0.1);
        let aimed = Affine::translation(1.0, 2.0, 3.0);
        let at = place(0.5, &aim, &aimed);
        assert_eq!(at.t, [1.0, 2.05, 3.1]);
        assert_eq!((at.x[0], at.y[1], at.z[2]), (0.5, 0.5, 0.5));
    }

    #[test]
    fn a_pattern_is_asked_once_and_one_query_at_a_time() {
        let mut patterns = Patterns::<u32>::NEW;
        patterns.draw(GUN, Some(want(DOT, 1.0)));
        patterns.draw(GUN, Some(want(CROSS, 1.0)));
        assert_eq!(patterns.next_ask(), Some(DOT));
        assert_eq!(patterns.next_ask(), None);
        patterns.arrived(None);
        assert_eq!(patterns.next_ask(), Some(CROSS));
        patterns.arrived(Some(7));
        // the dot did not load: it is never asked again, and nothing is shown for it
        patterns.draw(GUN, Some(want(DOT, 1.0)));
        assert_eq!(patterns.next_ask(), None);
        assert_eq!(patterns.draw(GUN, Some(want(CROSS, 1.0))).show.map(|(p, _)| p), Some(7));
    }

    #[test]
    fn appended_once_emitted_on_change_and_moved_each_draw() {
        let mut patterns = loaded(DOT, 7);
        let first = patterns.draw(GUN, Some(want(DOT, 1.0)));
        assert_eq!((first.hide, first.append), (None, Some(7)));
        assert_eq!(first.emit, Some((7, Emission([1.0, 0.0, 0.0, 1.0]))));
        assert!(first.show.is_some());
        let again = patterns.draw(GUN, Some(want(DOT, 1.0)));
        assert_eq!((again.hide, again.append, again.emit), (None, None, None));
        assert!(again.show.is_some());
        let brighter = patterns.draw(GUN, Some(want(DOT, 2.0)));
        assert_eq!((brighter.append, brighter.emit), (None, Some((7, Emission([2.0, 0.0, 0.0, 1.0])))));
    }

    #[test]
    fn the_reticle_leaves_with_its_aimed_optic_and_its_weapon() {
        let mut patterns = loaded(DOT, 7);
        patterns.draw(GUN, Some(want(DOT, 1.0)));
        // another weapon drawn without an aimed optic leaves it
        assert_eq!(patterns.draw(OTHER, None), Steps::default());
        assert_eq!(patterns.leave(OTHER), None);
        // the aimed optic out: the reticle out, once
        assert_eq!(patterns.draw(GUN, None).hide, Some(7));
        assert_eq!(patterns.draw(GUN, None).hide, None);
        // shown again, then the weapon hidden
        patterns.draw(GUN, Some(want(DOT, 1.0)));
        assert_eq!(patterns.leave(GUN), Some(7));
        assert_eq!(patterns.leave(GUN), None);
    }

    #[test]
    fn a_new_pattern_replaces_the_shown_one() {
        let mut patterns = loaded(DOT, 7);
        patterns.draw(GUN, Some(want(DOT, 1.0)));
        // the cross not loaded yet: the dot goes, nothing stands in its place
        assert_eq!(patterns.draw(GUN, Some(want(CROSS, 1.0))).hide, Some(7));
        assert_eq!(patterns.next_ask(), Some(CROSS));
        patterns.arrived(Some(8));
        let steps = patterns.draw(GUN, Some(want(CROSS, 1.0)));
        assert_eq!((steps.hide, steps.append), (None, Some(8)));
        let back = patterns.draw(GUN, Some(want(DOT, 1.0)));
        assert_eq!((back.hide, back.append, back.emit), (Some(8), None, None));
        assert_eq!(back.show.map(|(p, _)| p), Some(7));
    }
}
