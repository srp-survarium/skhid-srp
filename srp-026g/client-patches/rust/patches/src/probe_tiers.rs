//! Patch 203: a 0.69d scene's local environment probes are composed as
//! 0.69d composes them (docs/notes/probe-tiers-069d.md).
//!
//! 0.69d draws its probes in tiers: in a tier each probe adds its light
//! times its weight and the weight, the tier's light is the sum over
//! max(W, 1), and the tier below shows only where 1 - W remains. 0.26g lerps
//! every probe over the ones before it, largest first, so between two local
//! probes that meet with their falloff bands the light under both (the
//! global probes, outdoor light) shows through.
//!
//! The 0.69d converter writes a local probe's radius below 0 (its whole
//! part's magnitude the shape: 1 a box, 2 a cylinder), and a 0.69d scene
//! carries its exposure profile's marker (33000 + its 0.69d level, 33100 the
//! lobby room) in its environment's mie_fog_base_height. A local probe is one
//! with a radius below 0 in a scene so marked: stock level_07 has a probe of
//! radius -0.4 in 18 projects, which stays 0.26g's. The sort (largest first)
//! puts the locals after every global probe, so the loop
//! draws the globals as ever and then, from the first local probe on, runs
//! the locals three times with techniques the four box effects compile in
//! addition (programs of the lighting pack, `environment_probe_tier_*`):
//!
//! - Weight: rt 32 (accum_ambient_lights, cleared to 1 by the stage's own
//!   clear) minus w/4 each, alone bound (ONE/ONE, reverse subtract);
//! - Damp: the light so far times (1 - min(W, 1))^(w/W) (ZERO/SRC_ALPHA):
//!   the product over the probes at a pixel is 1 - min(W, 1);
//! - Light: each probe's own light times w/max(W, 1) (ONE/ONE).
//!
//! Then the stage's clear puts rt 32 back to 1 for the ambient lights and the
//! loop's targets are bound again. A scene with no local probe never leaves
//! the native draw. Presentation only: nothing a match computes reads it.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::effect_compile::CompilerId;
use crate::vostok::probe_tiers::{self, Blend, CompileSite, LoopSite, ProbeEffect, Side, Targets};
use crate::vostok::{self, Context, Flow, Lock, NoExit};

/// A 0.69d scene's marker: an exposure profile's, 33000 + the 0.69d level
/// (the lobby room 33100), a whole number (three-maps-021-lighting's
/// `exposure_profiles`).
fn marks_069d_scene(marker: f32) -> bool {
    (33000.0..34000.0).contains(&marker) && marker == (marker as i32) as f32
}

/// A probe the tiers draw: below 0 in a 0.69d scene.
fn is_local(marker: f32, radius: f32) -> bool {
    marks_069d_scene(marker) && radius < 0.0
}

/// rt 32's name and the slot the tier programs read it by.
const WEIGHTS_SLOT: &CStr = c"t_probe_weights";
const WEIGHTS_TARGET: &CStr = c"$user$accum_ambient_lights";

/// One of the three runs of a scene's local probes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tier {
    Weight,
    Damp,
    Light,
}

impl Tier {
    const FIRST: Tier = Tier::Weight;

    fn next(self) -> Option<Tier> {
        match self {
            Tier::Weight => Some(Tier::Damp),
            Tier::Damp => Some(Tier::Light),
            Tier::Light => None,
        }
    }

    /// Its techniques after the effect's own 0 and 1: 2/3, 4/5, 6/7.
    fn technique(self, pass: u32) -> u32 {
        let first = match self {
            Tier::Weight => 2,
            Tier::Damp => 4,
            Tier::Light => 6,
        };
        first + pass
    }

    fn pixel_shader(self) -> &'static CStr {
        match self {
            Tier::Weight => c"environment_probe_tier_weight",
            Tier::Damp => c"environment_probe_tier_damp",
            Tier::Light => c"environment_probe_tier_light",
        }
    }

    fn blend(self) -> Blend {
        match self {
            Tier::Weight => Blend { source: Blend::ONE, destination: Blend::ONE, operation: Blend::REVERSE_SUBTRACT },
            Tier::Damp => Blend { source: Blend::ZERO, destination: Blend::SOURCE_ALPHA, operation: Blend::ADD },
            Tier::Light => Blend { source: Blend::ONE, destination: Blend::ONE, operation: Blend::ADD },
        }
    }
}

// ---------------------------------------------------------------------------
// The compiles: each of the four box effects compiles its two techniques three
// more times after its own, one round a tier.

/// Compiles at once: one per resource thread at most; past this many the
/// game stops (a round lost would leave an effect without its tiers).
const ROUNDS: usize = 64;

/// The compilers in a tier round, and which.
struct Rounds([Option<(CompilerId, Tier)>; ROUNDS]);

impl Rounds {
    const fn new() -> Rounds {
        Rounds([None; ROUNDS])
    }

    fn tier(&self, compiler: CompilerId) -> Option<Tier> {
        self.0.iter().flatten().find(|(c, _)| *c == compiler).map(|(_, t)| *t)
    }

    /// The compile by `compiler` is at its end: the round to compile next, or
    /// None when the last is done.
    fn end(&mut self, compiler: CompilerId) -> Option<Tier> {
        match self.0.iter().position(|slot| matches!(slot, Some((c, _)) if *c == compiler)) {
            None => {
                let free = self.0.iter_mut().find(|slot| slot.is_none()).expect("fewer compiles at once than ROUNDS");
                *free = Some((compiler, Tier::FIRST));
                Some(Tier::FIRST)
            }
            Some(i) => {
                let (_, tier) = self.0[i].expect("a round");
                let next = tier.next();
                self.0[i] = next.map(|t| (compiler, t));
                next
            }
        }
    }
}

static ROUNDS_NOW: Lock<Rounds> = Lock::new(Rounds::new());

fn round(ctx: &Context<'_>, site: CompileSite) -> Option<Tier> {
    let compiler = probe_tiers::compiler_id(ctx, site);
    ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.tier(compiler))
}

fn begin_pass(ctx: &mut Context<'_>, effect: ProbeEffect, side: Side) -> Flow<NoExit> {
    if let Some(tier) = round(ctx, CompileSite::Pass(effect, side)) {
        probe_tiers::set_pixel_shader(ctx, effect, side, tier.pixel_shader());
    }
    Flow::Continue
}

fn blend(ctx: &mut Context<'_>, effect: ProbeEffect, side: Side) -> Flow<NoExit> {
    if let Some(tier) = round(ctx, CompileSite::Blend(effect, side)) {
        probe_tiers::set_blend(ctx, effect, side, tier.blend());
    }
    Flow::Continue
}

fn texture(ctx: &mut Context<'_>, effect: ProbeEffect, side: Side) -> Flow<NoExit> {
    if round(ctx, CompileSite::Texture(effect, side)).is_some() {
        probe_tiers::bind_texture(ctx, effect, side, WEIGHTS_SLOT, WEIGHTS_TARGET);
    }
    Flow::Continue
}

/// True: back to technique 0's start for the next round.
fn compiled(ctx: &mut Context<'_>, effect: ProbeEffect) -> bool {
    let compiler = probe_tiers::compiler_id(ctx, CompileSite::End(effect));
    ROUNDS_NOW.with(ctx.thread(), |rounds| rounds.end(compiler)).is_some()
}

macro_rules! effect_hooks {
    ($($effect:ident: $p0:ident, $p1:ident, $b0:ident, $b1:ident, $t0:ident, $t1:ident, $end:ident, $exit:ident;)*) => {$(
        pub fn $p0(ctx: &mut Context<'_>) -> Flow<NoExit> {
            begin_pass(ctx, ProbeEffect::$effect, Side::Front)
        }
        pub fn $p1(ctx: &mut Context<'_>) -> Flow<NoExit> {
            begin_pass(ctx, ProbeEffect::$effect, Side::Back)
        }
        pub fn $b0(ctx: &mut Context<'_>) -> Flow<NoExit> {
            blend(ctx, ProbeEffect::$effect, Side::Front)
        }
        pub fn $b1(ctx: &mut Context<'_>) -> Flow<NoExit> {
            blend(ctx, ProbeEffect::$effect, Side::Back)
        }
        pub fn $t0(ctx: &mut Context<'_>) -> Flow<NoExit> {
            texture(ctx, ProbeEffect::$effect, Side::Front)
        }
        pub fn $t1(ctx: &mut Context<'_>) -> Flow<NoExit> {
            texture(ctx, ProbeEffect::$effect, Side::Back)
        }
        pub fn $end(ctx: &mut Context<'_>) -> Flow<vostok::$exit> {
            if compiled(ctx, ProbeEffect::$effect) { Flow::Exit(vostok::$exit::Technique) } else { Flow::Continue }
        }
    )*};
}

effect_hooks! {
    P1000: on_pass0_1000, on_pass1_1000, on_blend0_1000, on_blend1_1000, on_texture0_1000, on_texture1_1000, on_compiled_1000, ProbeTiersOnCompiled1000Exit;
    P1010: on_pass0_1010, on_pass1_1010, on_blend0_1010, on_blend1_1010, on_texture0_1010, on_texture1_1010, on_compiled_1010, ProbeTiersOnCompiled1010Exit;
    P1001: on_pass0_1001, on_pass1_1001, on_blend0_1001, on_blend1_1001, on_texture0_1001, on_texture1_1001, on_compiled_1001, ProbeTiersOnCompiled1001Exit;
    P1011: on_pass0_1011, on_pass1_1011, on_blend0_1011, on_blend1_1011, on_texture0_1011, on_texture1_1011, on_compiled_1011, ProbeTiersOnCompiled1011Exit;
}

// ---------------------------------------------------------------------------
// The draw.

/// The probe loop of the frame being drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Draw {
    /// The globals, every probe of a scene without local ones.
    Native,
    /// The locals, from the sorted list's probe number `first` to its end,
    /// the run `tier`; `targets` the loop's own, bound again after the weights.
    Tiers { tier: Tier, first: u32, targets: Targets },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DrawEvent {
    /// The loop is at a probe: a local one (radius below 0) or not.
    Probe { index: u32, local: bool, targets: Targets },
    /// The loop drew the list's last probe.
    Last,
}

/// What the draw does on a transition.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    None,
    /// The first local probe: rt 32 cleared to 1 and bound alone.
    StartWeights,
    /// After the weights: the loop's targets again, the loop back to the
    /// first local probe.
    EndWeights { first: u32, targets: Targets },
    /// Back to the first local probe.
    Rewind { first: u32 },
    /// The last run done: rt 32 cleared to 1 again, the loop's targets bound.
    Finish { targets: Targets },
}

impl Draw {
    fn step(self, event: DrawEvent) -> (Draw, Action) {
        match (self, event) {
            (Draw::Native, DrawEvent::Probe { local: false, .. }) => (Draw::Native, Action::None),
            (Draw::Native, DrawEvent::Probe { index, local: true, targets }) => {
                (Draw::Tiers { tier: Tier::FIRST, first: index, targets }, Action::StartWeights)
            }
            (Draw::Tiers { .. }, DrawEvent::Probe { local: false, index, .. }) => {
                panic!("probe_tiers: probe {index} of the sorted list is global after a local one")
            }
            (tiers @ Draw::Tiers { .. }, DrawEvent::Probe { local: true, .. }) => (tiers, Action::None),
            (Draw::Native, DrawEvent::Last) => (Draw::Native, Action::None),
            (Draw::Tiers { tier, first, targets }, DrawEvent::Last) => match tier.next() {
                Some(next) => {
                    let action = match tier {
                        Tier::Weight => Action::EndWeights { first, targets },
                        Tier::Damp | Tier::Light => Action::Rewind { first },
                    };
                    (Draw::Tiers { tier: next, first, targets }, action)
                }
                None => (Draw::Native, Action::Finish { targets }),
            },
        }
    }

    /// The technique of a probe's pass (0 front, 1 back).
    fn technique(self, pass: u32) -> u32 {
        match self {
            Draw::Native => pass,
            Draw::Tiers { tier, .. } => tier.technique(pass),
        }
    }
}

static DRAW: Lock<Draw> = Lock::new(Draw::Native);

/// env_probes.next: the loop at a probe.
pub fn on_probe(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let probe = probe_tiers::probe_at_next(ctx);
    let event = DrawEvent::Probe { index: probe.index, local: is_local(probe.marker, probe.radius), targets: probe_tiers::bound_targets() };
    let action = DRAW.with(ctx.thread(), |draw| {
        let (next, action) = draw.step(event);
        *draw = next;
        action
    });
    if action == Action::StartWeights {
        probe_tiers::clear_ambient_surfaces(ctx, LoopSite::Next);
        probe_tiers::bind_targets(probe_tiers::ambient_lights_target(ctx, LoopSite::Next));
    }
    Flow::Continue
}

/// env_probes.technique: a local probe's pass draws with its run's technique.
pub fn on_technique(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let draw = DRAW.with(ctx.thread(), |draw| *draw);
    if let Draw::Tiers { .. } = draw {
        let (probe, index) = probe_tiers::technique_at(ctx);
        if ProbeEffect::of_index(index).is_none() {
            panic!(
                "probe_tiers: the local probe {:#x} (number {} of the sorted list, radius bits {:#x}) draws with effect {} of 16 (8 geometry + 4 smart_attenuation + 2 blend + affect_diffuse), which has no tiers: a 0.69d local probe is a box without smart attenuation",
                probe.address, probe.index, probe.radius.to_bits(), index
            );
        }
        let pass = probe_tiers::pass_at(ctx);
        probe_tiers::set_technique(ctx, draw.technique(pass));
    }
    Flow::Continue
}

/// env_probes.done: after a probe's passes; at the list's end, the next run.
pub fn on_probe_done(ctx: &mut Context<'_>) -> Flow<vostok::ProbeTiersOnProbeDoneExit> {
    if !probe_tiers::last_at_done(ctx) {
        return Flow::Continue;
    }
    let action = DRAW.with(ctx.thread(), |draw| {
        let (next, action) = draw.step(DrawEvent::Last);
        *draw = next;
        action
    });
    match action {
        Action::None | Action::StartWeights => Flow::Continue,
        Action::EndWeights { first, targets } => {
            probe_tiers::bind_targets(targets);
            probe_tiers::rewind(ctx, first);
            Flow::Exit(vostok::ProbeTiersOnProbeDoneExit::Again)
        }
        Action::Rewind { first } => {
            probe_tiers::rewind(ctx, first);
            Flow::Exit(vostok::ProbeTiersOnProbeDoneExit::Again)
        }
        Action::Finish { targets } => {
            probe_tiers::clear_ambient_surfaces(ctx, LoopSite::Done);
            probe_tiers::bind_targets(targets);
            Flow::Continue
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn compiler(n: u32) -> CompilerId {
        CompilerId::for_test(n)
    }

    #[test]
    fn a_compile_runs_three_rounds_then_returns() {
        let mut rounds = Rounds::new();
        let (a, b) = (compiler(1), compiler(2));
        assert_eq!(rounds.end(a), Some(Tier::Weight));
        assert_eq!(rounds.tier(a), Some(Tier::Weight));
        assert_eq!(rounds.end(b), Some(Tier::Weight));
        assert_eq!(rounds.end(a), Some(Tier::Damp));
        assert_eq!(rounds.end(a), Some(Tier::Light));
        assert_eq!(rounds.tier(a), Some(Tier::Light));
        assert_eq!(rounds.end(a), None);
        assert_eq!(rounds.tier(a), None);
        assert_eq!(rounds.tier(b), Some(Tier::Weight));
    }

    #[test]
    fn the_runs_use_their_own_techniques() {
        assert_eq!([0, 1].map(|p| Tier::Weight.technique(p)), [2, 3]);
        assert_eq!([0, 1].map(|p| Tier::Damp.technique(p)), [4, 5]);
        assert_eq!([0, 1].map(|p| Tier::Light.technique(p)), [6, 7]);
        assert_eq!(Draw::Native.technique(1), 1);
    }

    fn probe(index: u32, local: bool, targets: Targets) -> DrawEvent {
        DrawEvent::Probe { index, local, targets }
    }

    #[test]
    fn only_a_069d_scenes_probes_below_0_are_local() {
        assert!(is_local(33001.0, -1.4));
        assert!(is_local(33100.0, -2.3));
        // stock level_07's indoor(28), radius -0.4, in a native scene
        assert!(!is_local(30.0, -0.4));
        assert!(!is_local(0.0, -0.4));
        // the 0.21 ports' and London's markers
        assert!(!is_local(-32281.0, -1.4));
        assert!(!is_local(32909.0, -1.4));
        assert!(!is_local(33001.0, 3.0));
        assert!(!is_local(33001.5, -1.4));
    }

    #[test]
    fn a_scene_without_local_probes_draws_natively() {
        let targets = Targets::for_test([31, 34, 0, 0]);
        let mut draw = Draw::Native;
        for i in 0..3 {
            let (next, action) = draw.step(probe(i, false, targets));
            assert_eq!((next, action), (Draw::Native, Action::None));
            draw = next;
        }
        assert_eq!(draw.step(DrawEvent::Last), (Draw::Native, Action::None));
        assert_eq!(draw.technique(0), 0);
    }

    #[test]
    fn the_local_probes_run_three_times_then_the_draw_is_native() {
        let targets = Targets::for_test([31, 34, 0, 0]);
        let (draw, action) = Draw::Native.step(probe(2, true, targets));
        assert_eq!(action, Action::StartWeights);
        assert_eq!(draw.technique(0), 2);
        let (draw, action) = draw.step(probe(3, true, targets));
        assert_eq!(action, Action::None);
        let (draw, action) = draw.step(DrawEvent::Last);
        assert_eq!(action, Action::EndWeights { first: 2, targets });
        assert_eq!(draw.technique(1), 5);
        let (draw, action) = draw.step(DrawEvent::Last);
        assert_eq!(action, Action::Rewind { first: 2 });
        assert_eq!(draw.technique(0), 6);
        let (draw, action) = draw.step(DrawEvent::Last);
        assert_eq!((draw, action), (Draw::Native, Action::Finish { targets }));
    }

    #[test]
    #[should_panic(expected = "global after a local one")]
    fn a_global_probe_after_a_local_one_stops() {
        let targets = Targets::for_test([31, 34, 0, 0]);
        let (draw, _) = Draw::Native.step(probe(0, true, targets));
        let _ = draw.step(probe(1, false, targets));
    }
}
