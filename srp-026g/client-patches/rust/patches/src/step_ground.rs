//! Patch 153: a step whose body stands on something sounds
//! (docs/notes/footsteps.md, "Silent ground"; the owner, 2026-11-16: "even
//! right now when walking on rails, sometimes no sound is heard at all").
//!
//! on_step's one ray (from 1 m above the foot, 2 m down) meets only the
//! collision bullets hit, so a step on a walker-only floor (a wagon's top,
//! a loading station, a wall top) whose hittable surface is more than 1 m
//! under it meets nothing and is silent. 0.69d keeps the material the
//! character controller stands on and steps on it when the ray misses;
//! 0.26g's controller keeps none, so this asks the same question when the ray
//! misses: what the body stands on, the walker collision within the ray's
//! reach. Its face's material when it has one (a wagon's walker top is metal,
//! a wall's brick); when it has none (most walker faces), the hittable surface
//! under it, up to 4 m under the foot (the station's metal floor, the
//! school's concrete); else `default`, as material_pair steps a face without
//! a material. The sound is at the foot, as 0.69d places it. Nothing under the
//! foot at all: the body is in the air, silent as before. A step the game's
//! ray found is the game's, byte for byte.
//!
//! With SKHID_STEP_LOG=on (read once, at the first step) each step writes a
//! line to the run's log: whose, its caller's movement id, the foot, what
//! each ray met, the ground and what its pair plays.

#![forbid(unsafe_code)]

use crate::vostok::log::{parts, Piece, Piece::*};
use crate::vostok::memory_profile::environment;
use crate::vostok::step::{Caller, Hit, Look, Next, OnStep, PairSound, Placed, View};
use crate::vostok::{Context, Flow, GameCell, StepGroundOnGroundExit, Thread};

/// How far under the foot the look under a walker-only floor reaches: a
/// storey. on_step's own ray reaches 1 m. Under the walker-only standing
/// room of the eight measured maps the hittable surface is 1 to 4 m down on
/// 2,180 m², 4 to 10 m on 1,378 m² (a storey or more: another floor's
/// material, or the ground's).
const BELOW: f32 = 4.0;

/// The ground a step goes on with, and how it was found.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ground {
    /// A patch's own step ([`crate::vostok::step::play`]): its ground.
    Armed(Placed),
    /// on_step's own ray met it: the game's path.
    Game,
    /// on_step's ray met nothing; the walker collision under the foot, its
    /// face's material.
    Walker(Hit),
    /// The walker face has no material: the hittable surface under it.
    Below { walker: Hit, below: Hit },
    /// Neither: the walker face's (none), which material_pair steps as
    /// `default`.
    Unnamed(Hit),
    /// Nothing under the foot: the game's silent return.
    Air,
}

/// The ground of a step: `armed` a patch's own, `met` whether on_step's ray
/// met anything, `look` our looks after a miss (each taken only when needed).
fn ground(armed: Option<Placed>, met: bool, mut look: impl FnMut(Look) -> Option<Hit>) -> Ground {
    if let Some(placed) = armed {
        return Ground::Armed(placed);
    }
    if met {
        return Ground::Game;
    }
    let Some(walker) = look(Look::Walker) else {
        return Ground::Air;
    };
    if walker.material.named() {
        return Ground::Walker(walker);
    }
    match look(Look::Below { reach: BELOW }) {
        Some(below) => Ground::Below { walker, below },
        None => Ground::Unnamed(walker),
    }
}

/// How on_step goes on with `ground`, a found one's sound at `foot`.
fn next(ground: Ground, foot: [f32; 3]) -> Next {
    match ground {
        Ground::Game | Ground::Air => Next::Game,
        Ground::Armed(placed) => Next::Ground(placed),
        Ground::Walker(hit) | Ground::Unnamed(hit) | Ground::Below { below: hit, .. } => {
            Next::Ground(Placed { material: hit.material, at: foot, normal: hit.normal })
        }
    }
}

/// SKHID_STEP_LOG as the game was started with it, read at the first step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Switch {
    Unread,
    Off,
    On,
}

const VARIABLE: &[u8] = b"SKHID_STEP_LOG\0";

impl Switch {
    /// `on` or `1` (any case) is on; anything else, or nothing, off.
    fn parse(value: Option<&[u8]>) -> Switch {
        match value {
            Some(value) if value.eq_ignore_ascii_case(b"on") || value == b"1" => Switch::On,
            _ => Switch::Off,
        }
    }
}

static LOG: GameCell<Switch> = GameCell::new(Switch::Unread);

fn log_on(thread: &Thread) -> bool {
    let switch = match LOG.get(thread) {
        Switch::Unread => {
            let mut value = [0u8; 8];
            let read = Switch::parse(environment(thread, VARIABLE, &mut value));
            LOG.set(thread, read);
            read
        }
        known => known,
    };
    switch == Switch::On
}

/// on_step after its ground ray (`step.ground`).
pub fn on_ground(ctx: &mut Context<'_>) -> Flow<StepGroundOnGroundExit> {
    let Some(step) = OnStep::at(ctx) else {
        return Flow::Continue;
    };
    let thread = ctx.thread();
    let ground = ground(step.armed(thread), step.met(), |look| step.look(thread, look));
    if log_on(thread) {
        log(thread, &step, ground);
    }
    let next = next(ground, step.foot());
    step.go_on(ctx, next)
}

/// Signed centimetres: the sign, then the magnitude.
fn centimetres(metres: f32) -> [Piece<'static>; 2] {
    let cm = (metres * 100.0) as i32;
    [Text(if cm < 0 { "-" } else { "" }), Dec(cm.unsigned_abs())]
}

/// `name` and what a ray met: `; <name> met <material> at <depth> cm`, or
/// missed.
fn ray<'a, 'o>(name: &'a str, hit: Option<Hit>, out: &'o mut [Piece<'a>; 8]) -> &'o [Piece<'a>] {
    match hit {
        Some(hit) => {
            let [sign, depth] = centimetres(hit.depth);
            *out = [Text("; "), Text(name), Text(" met "), Dec(u32::from(hit.material.id())), Text(" at "), sign, depth,
                    Text(" cm")];
            &out[..]
        }
        None => {
            out[..3].copy_from_slice(&[Text("; "), Text(name), Text(" missed")]);
            &out[..3]
        }
    }
}

/// One line: `step: <whose> <caller> <view>, foot <material> at x y z cm;
/// game ...; walker ...; below ...; ground <material> (<how>): <pair>`.
fn log(thread: &Thread, step: &OnStep, ground: Ground) {
    let (whose, caller, movement) = match step.caller(thread) {
        Caller::Foot { own, movement: Some(id) } => (Some(own), "foot, movement ", Some(id)),
        Caller::Foot { own, movement: None } => (Some(own), "foot, no logic state", None),
        Caller::Landing { own, movement } => (Some(own), "landing, movement ", Some(movement)),
        Caller::Other => (None, "a patch's step", None),
    };
    let movement = movement.map(Dec);
    let [x, y, z] = step.foot();
    let ([xs, xv], [ys, yv], [zs, zv]) = (centimetres(x), centimetres(y), centimetres(z));
    let head = [
        Text("step: "),
        Text(match whose {
            Some(true) => "own ",
            Some(false) => "other's ",
            None => "",
        }),
        Text(caller),
    ];
    let foot = [
        Text(match step.view() {
            View::First => ", first person, foot ",
            View::Third => ", third person, foot ",
        }),
        Dec(u32::from(step.foot_material().id())),
        Text(" at "), xs, xv, Text(" "), ys, yv, Text(" "), zs, zv, Text(" cm"),
    ];
    // each ray: on_step's, then the looks the ground needed (none for a hit or an armed one)
    let (game_ray, walker_ray, below_ray) = match ground {
        Ground::Armed(_) => (None, None, None),
        Ground::Game => (Some(step.game_hit(thread)), None, None),
        Ground::Air => (Some(None), Some(None), None),
        Ground::Walker(walker) => (Some(None), Some(Some(walker)), None),
        Ground::Below { walker, below } => (Some(None), Some(Some(walker)), Some(Some(below))),
        Ground::Unnamed(walker) => (Some(None), Some(Some(walker)), Some(None)),
    };
    let (mut game, mut walker, mut below) = ([Text(""); 8], [Text(""); 8], [Text(""); 8]);
    let game: &[Piece] = match game_ray {
        Some(hit) => ray("game", hit, &mut game),
        None => &[Text("; armed")],
    };
    let walker: &[Piece] = match walker_ray {
        Some(hit) => ray("walker", hit, &mut walker),
        None => &[],
    };
    let below: &[Piece] = match below_ray {
        Some(hit) => ray("below", hit, &mut below),
        None => &[],
    };
    let (material, how) = match ground {
        Ground::Armed(placed) => (Some(placed.material), "armed"),
        Ground::Game => (step.game_hit(thread).map(|hit| hit.material), "game"),
        Ground::Walker(hit) => (Some(hit.material), "walker"),
        Ground::Below { below, .. } => (Some(below.material), "below"),
        Ground::Unnamed(hit) => (Some(hit.material), "walker, no material"),
        Ground::Air => (None, "air"),
    };
    let tail = match material {
        Some(material) => [
            Text("; ground "),
            Dec(u32::from(material.id())),
            Text(" ("),
            Text(how),
            Text("): "),
            Text(match step.pair(thread, material) {
                PairSound::Sound => "plays",
                PairSound::Silent => "silent, the pair has no sound",
                PairSound::NoPair => "silent, no pair",
            }),
        ],
        None => [Text("; nothing under the foot: silent"), Text(""), Text(""), Text(""), Text(""), Text("")],
    };
    parts(&[&head, movement.as_slice(), &foot, game, walker, below, &tail]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::step::GroundMaterial;

    const FOOT: [f32; 3] = [1.0, 2.0, 3.0];

    fn hit(material: u16, depth: f32) -> Hit {
        Hit { material: GroundMaterial::of(material), depth, normal: [0.0, 1.0, 0.0] }
    }

    /// The looks a step takes, in order, each answered from `answers`.
    fn decide(armed: Option<Placed>, met: bool, walker: Option<Hit>, below: Option<Hit>) -> (Ground, [bool; 2]) {
        let mut asked = [false; 2];
        let ground = ground(armed, met, |look| match look {
            Look::Walker => {
                asked[0] = true;
                walker
            }
            Look::Below { reach } => {
                assert_eq!(reach, BELOW);
                assert!(asked[0], "below only after the walker look");
                asked[1] = true;
                below
            }
        });
        (ground, asked)
    }

    #[test]
    fn a_ray_that_met_the_ground_is_the_games_and_looks_no_more() {
        let (ground, asked) = decide(None, true, Some(hit(18, 0.1)), Some(hit(9, 2.0)));
        assert_eq!(ground, Ground::Game);
        assert_eq!(asked, [false, false]);
        assert_eq!(next(ground, FOOT), Next::Game);
    }

    #[test]
    fn a_named_walker_face_is_the_ground_at_the_foot() {
        // a hopper wagon's walker top, metall_thick, the hittable body 2 m under it
        let (ground, asked) = decide(None, false, Some(hit(18, 0.1)), Some(hit(9, 2.0)));
        assert_eq!(ground, Ground::Walker(hit(18, 0.1)));
        assert_eq!(asked, [true, false], "a named face needs no look under it");
        assert_eq!(next(ground, FOOT), Next::Ground(Placed { material: GroundMaterial::of(18), at: FOOT, normal: [0.0, 1.0, 0.0] }));
    }

    #[test]
    fn an_unnamed_walker_face_takes_the_hittable_surface_under_it() {
        let (ground, asked) = decide(None, false, Some(hit(0x3FFF, 0.1)), Some(hit(18, 2.5)));
        assert_eq!(ground, Ground::Below { walker: hit(0x3FFF, 0.1), below: hit(18, 2.5) });
        assert_eq!(asked, [true, true]);
        assert_eq!(next(ground, FOOT), Next::Ground(Placed { material: GroundMaterial::of(18), at: FOOT, normal: [0.0, 1.0, 0.0] }),
                   "the sound at the foot, not 2.5 m under it");
    }

    #[test]
    fn nothing_under_an_unnamed_face_steps_as_its_default() {
        let (ground, _) = decide(None, false, Some(hit(0xFFFF, 0.2)), None);
        assert_eq!(ground, Ground::Unnamed(hit(0xFFFF, 0.2)));
        let Next::Ground(placed) = next(ground, FOOT) else { panic!("a ground") };
        assert_eq!(placed.material, GroundMaterial::of(0xFFFF), "material_pair makes it default");
    }

    #[test]
    fn nothing_under_the_foot_is_the_games_silent_step() {
        let (ground, asked) = decide(None, false, None, Some(hit(9, 3.0)));
        assert_eq!(ground, Ground::Air);
        assert_eq!(asked, [true, false]);
        assert_eq!(next(ground, FOOT), Next::Game);
    }

    #[test]
    fn an_armed_ground_wins_over_any_ray() {
        let placed = Placed { material: GroundMaterial::of(17), at: [5.0, 6.0, 7.0], normal: [0.0, 0.0, 1.0] };
        for met in [false, true] {
            let (ground, asked) = decide(Some(placed), met, Some(hit(18, 0.1)), None);
            assert_eq!(ground, Ground::Armed(placed));
            assert_eq!(asked, [false, false]);
            assert_eq!(next(ground, FOOT), Next::Ground(placed));
        }
    }

    #[test]
    fn the_switch_reads_on_or_one() {
        assert_eq!(Switch::parse(Some(b"on")), Switch::On);
        assert_eq!(Switch::parse(Some(b"ON")), Switch::On);
        assert_eq!(Switch::parse(Some(b"1")), Switch::On);
        assert_eq!(Switch::parse(Some(b"off")), Switch::Off);
        assert_eq!(Switch::parse(Some(b"")), Switch::Off);
        assert_eq!(Switch::parse(None), Switch::Off);
    }

    #[test]
    fn centimetres_keep_their_sign() {
        let text = |pieces: [Piece<'static>; 2]| match pieces {
            [Text(sign), Dec(value)] => (sign, value),
            _ => panic!(),
        };
        assert_eq!(text(centimetres(-1.234)), ("-", 123));
        assert_eq!(text(centimetres(0.5)), ("", 50));
        assert_eq!(text(centimetres(0.0)), ("", 0));
    }
}
