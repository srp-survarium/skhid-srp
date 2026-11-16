//! Patch 153: a step sounds on what the body stands on, as 0.69d does
//! (docs/notes/footsteps.md, "Silent ground"; the owner, 2026-11-16: "even
//! right now when walking on rails, sometimes no sound is heard at all").
//!
//! on_step's one ray (from 1 m above the foot, 2 m down) meets only the
//! collision bullets hit, so a step on a walker-only floor (a wagon's top, a
//! wall top) or beside a thin one (the Chemical Plant's loading station
//! walkway: a 32 cm walker strip over an 8 cm metal beam, the foot's ray past
//! both) meets nothing and is silent. 0.69d starts from the material its
//! character controller touched (+612Eh, from its step-down sweep), lets the
//! ray's hit replace it, and is silent when it has neither. So here:
//! [`crate::vostok::contact`] keeps what each controller's step-down sweep
//! touched (the sweep's own hit, read where its callback takes it); a step
//! whose ray met nothing steps on that, at the foot. A touched face without a
//! material (most walker faces: `walker_col_s`) takes the hittable surface
//! under the touch, up to 4 m (the strip's beam, a wagon's metal body), else
//! `default`, as material_pair steps it. No touch (the sweep met nothing: in
//! the air): silent. A step the game's ray found is the game's, byte for byte.
//!
//! A step in water wades (the owner, 2026-11-16, of the pond in the middle of
//! Rudnya: "I want them there"): the map's drawn water surfaces are collision
//! of their own (content pack water-steps, a bit no other filter of the game
//! has), and a look at them comes first. Water at least WADE deep over the
//! ground under the foot is the step's ground: its faces' deep_water, at the
//! surface over the foot. 0.69d's Rudnya wades because its pond's bed is
//! deep_water; 0.26g's own Rudnya has mud there.
//!
//! With SKHID_STEP_LOG=on (read once, at the first step) each step writes a
//! line to the run's log: whose, its caller's movement id, the foot, the body
//! (its root, the foot's offset from it, the controller's support normal,
//! velocity, on_ground and what it touched), what each ray met, the ground and
//! what its pair plays; `footsteps` writes a line for each step event at its
//! guard.

#![forbid(unsafe_code)]

use crate::vostok::contact::{self, Touch};
use crate::vostok::log::{parts, Piece, Piece::*};
use crate::vostok::memory_profile::environment;
use crate::vostok::step::{Body, Caller, Hit, Look, Next, OnStep, PairSound, Placed, View};
use crate::vostok::{Context, Flow, GameCell, NoExit, StepGroundOnGroundExit, Thread};

/// How far under a touched face without a material the look for the
/// hittable surface reaches: a storey (a wagon's metal body 1.25 m under its
/// walker top; deeper is another floor's or the ground's).
const BELOW: f32 = 4.0;

/// How deep water over the ground under the foot makes a step wade, metres:
/// a film over the ground (a puddle's, the shallow edge of a pond) is the
/// ground's step. The pond of 0.69d's Rudnya is deep_water at its bed where it
/// is 0.3 m deep or more, its ground in the band less deep than 0.2 m.
const WADE: f32 = 0.2;

/// The ground a step goes on with, and how it was found.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ground {
    /// A patch's own step ([`crate::vostok::step::play`]): its ground.
    Armed(Placed),
    /// Water at least [`WADE`] deep over the ground under the foot: its face.
    Wading(Hit),
    /// on_step's own ray met it: the game's path.
    Game,
    /// on_step's ray met nothing: what the controller touched, and for a
    /// face without a material the hittable surface under the touch (None:
    /// none within [`BELOW`]).
    Touched { touch: Touch, under: Option<Hit> },
    /// Neither: the game's silent return.
    Air,
}

/// The ground of a step: `armed` a patch's own, `game` how far under the
/// foot on_step's ray met something (None: nothing), `touch` what the
/// stepping body's controller touched (None: nothing, or no body), `look` our
/// looks (each taken only when needed).
fn ground(armed: Option<Placed>, game: Option<f32>, touch: Option<Touch>, mut look: impl FnMut(Look) -> Option<Hit>)
    -> Ground {
    if let Some(placed) = armed {
        return Ground::Armed(placed);
    }
    if let Some(water) = look(Look::Water) {
        // what the foot stands on: what on_step's ray met, else the foot's own height
        if game.unwrap_or(0.0) - water.depth >= WADE {
            return Ground::Wading(water);
        }
    }
    if game.is_some() {
        return Ground::Game;
    }
    let Some(touch) = touch else {
        return Ground::Air;
    };
    let under = if touch.material.named() {
        None
    } else {
        look(Look::Under { at: touch.at, reach: BELOW }).filter(|hit| hit.material.named())
    };
    Ground::Touched { touch, under }
}

/// How on_step goes on with `ground`, a found one's sound at `foot`.
fn next(ground: Ground, foot: [f32; 3]) -> Next {
    match ground {
        Ground::Game | Ground::Air => Next::Game,
        Ground::Armed(placed) => Next::Ground(placed),
        Ground::Wading(water) => {
            let [x, y, z] = foot;
            Next::Ground(Placed { material: water.material, at: [x, y - water.depth, z], normal: water.normal })
        }
        Ground::Touched { touch, under } => match under {
            Some(hit) => Next::Ground(Placed { material: hit.material, at: foot, normal: hit.normal }),
            None => Next::Ground(Placed { material: touch.material, at: foot, normal: [0.0, 1.0, 0.0] }),
        },
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

/// SKHID_STEP_LOG: the steps' lines here and the step events' (`footsteps`).
pub(crate) fn log_on(thread: &Thread) -> bool {
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
    let body = step.body(thread);
    let mut water = None;
    let ground = ground(step.armed(thread), step.game_depth(), body.and_then(|body| body.touch), |look| {
        let hit = step.look(thread, look);
        if look == Look::Water {
            water = Some(hit);
        }
        hit
    });
    if log_on(thread) {
        log(thread, &step, ground, water, body);
    }
    let next = next(ground, step.foot());
    step.go_on(ctx, next)
}

/// The step-down sweep's callback takes a hit as its first (`step_down.first`).
pub fn on_sweep_first(ctx: &mut Context<'_>) -> Flow<NoExit> {
    contact::first(ctx);
    Flow::Continue
}

/// The same, as its second (`step_down.second`).
pub fn on_sweep_second(ctx: &mut Context<'_>) -> Flow<NoExit> {
    contact::second(ctx);
    Flow::Continue
}

/// The step-down sweep's answer (`step_down.result`).
pub fn on_sweep_result(ctx: &mut Context<'_>) -> Flow<NoExit> {
    contact::result(ctx);
    Flow::Continue
}

/// The step-down sweep answered from its cache (`step_down.cached`).
pub fn on_sweep_cached(ctx: &mut Context<'_>) -> Flow<NoExit> {
    contact::cached(ctx);
    Flow::Continue
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
/// <body>; game ...; water ...; [under ...; ]ground <material> (<how>):
/// <pair>`. `water` the water look's (None: not taken).
fn log(thread: &Thread, step: &OnStep, ground: Ground, water: Option<Option<Hit>>, body: Option<Body>) {
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
    let mut body_text = [Text(""); BODY_PIECES];
    let body: &[Piece] = match body {
        Some(found) => body_pieces(found, step.foot(), &mut body_text),
        None => &[Text("; no body")],
    };
    let (mut game, mut wet, mut under) = ([Text(""); 8], [Text(""); 8], [Text(""); 8]);
    let game: &[Piece] = match ground {
        Ground::Armed(_) => &[Text("; armed")],
        _ => ray("game", step.game_hit(thread), &mut game),
    };
    let water: &[Piece] = match water {
        Some(hit) => ray("water", hit, &mut wet),
        None => &[],
    };
    let under: &[Piece] = match ground {
        Ground::Touched { touch, under: hit } if !touch.material.named() => ray("under the touch", hit, &mut under),
        _ => &[],
    };
    let (material, how) = match ground {
        Ground::Armed(placed) => (Some(placed.material), "armed"),
        Ground::Wading(hit) => (Some(hit.material), "wading"),
        Ground::Game => (step.game_hit(thread).map(|hit| hit.material), "game"),
        Ground::Touched { under: Some(hit), .. } => (Some(hit.material), "under the touch"),
        Ground::Touched { touch, under: None } => (Some(touch.material), "touched"),
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
        None => [Text("; nothing under the foot, no contact: silent"), Text(""), Text(""), Text(""), Text(""), Text("")],
    };
    parts(&[&head, movement.as_slice(), &foot, body, game, water, under, &tail]);
}

/// The step log's body's pieces.
const BODY_PIECES: usize = 39;

/// The step log's body: `; root x y z cm, foot-root dx dy dz cm; support y
/// <x100>, velocity y <cm/s>, [jump flag, ]on_ground|off the ground; contact
/// <material> at x y z cm|no contact` (`jump flag, ` only while the
/// controller's +203h is set).
fn body_pieces<'o>(body: Body, foot: [f32; 3], out: &'o mut [Piece<'static>; BODY_PIECES]) -> &'o [Piece<'static>] {
    let [rx, ry, rz] = body.root;
    let ([a, b], [c, d], [e, f]) = (centimetres(rx), centimetres(ry), centimetres(rz));
    let ([g, h], [i, j], [k, l]) = (centimetres(foot[0] - rx), centimetres(foot[1] - ry), centimetres(foot[2] - rz));
    let [m, n] = centimetres(body.support[1]);
    let [o, p] = centimetres(body.rising);
    *out = [
        Text("; root "), a, b, Text(" "), c, d, Text(" "), e, f,
        Text(" cm, foot-root "), g, h, Text(" "), i, j, Text(" "), k, l,
        Text(" cm; support y x100 "), m, n,
        Text(", velocity y "), o, p,
        Text(" cm/s, "),
        Text(if body.jump_flag { "jump flag, " } else { "" }),
        Text(if body.on_ground() { "on_ground" } else { "off the ground" }),
        Text(""), Text(""), Text(""), Text(""), Text(""), Text(""), Text(""), Text(""), Text(""), Text(""), Text(""),
        Text(""),
    ];
    match body.touch {
        Some(touch) => {
            let [tx, ty, tz] = touch.at;
            let ([a, b], [c, d], [e, f]) = (centimetres(tx), centimetres(ty), centimetres(tz));
            out[27..].copy_from_slice(&[Text("; contact "), Dec(u32::from(touch.material.id())), Text(" at "), a, b,
                                        Text(" "), c, d, Text(" "), e, f, Text(" cm")]);
            &out[..]
        }
        None => {
            out[27] = Text("; no contact");
            &out[..28]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::step::GroundMaterial;

    const FOOT: [f32; 3] = [1.0, 2.0, 3.0];

    fn hit(material: u16, depth: f32) -> Hit {
        Hit { material: GroundMaterial::of(material), depth, normal: [0.0, 1.0, 0.0] }
    }

    fn touched(material: u16) -> Touch {
        Touch { material: GroundMaterial::of(material), at: [1.1, 1.9, 3.0] }
    }

    /// A step on dry ground (no water), `game` on_step's ray, `touch` the
    /// controller's, `under` what the look under the touch meets.
    fn step(game: Option<f32>, touch: Option<Touch>, under: Option<Hit>) -> (Ground, u32) {
        let mut looks = 0;
        let ground = ground(None, game, touch, |look| match look {
            Look::Water => None,
            Look::Under { at, reach } => {
                assert_eq!((at, reach), (touch.expect("a touch").at, BELOW));
                looks += 1;
                under
            }
        });
        (ground, looks)
    }

    #[test]
    fn a_ray_that_met_the_ground_is_the_games_whatever_was_touched() {
        assert_eq!(step(Some(0.1), Some(touched(18)), None), (Ground::Game, 0));
        assert_eq!(step(Some(0.1), None, None), (Ground::Game, 0));
    }

    #[test]
    fn a_miss_steps_on_the_named_face_the_controller_touched() {
        // a hopper wagon's walker top given metall_thick
        let (ground, looks) = step(None, Some(touched(18)), Some(hit(9, 2.0)));
        assert_eq!((ground, looks), (Ground::Touched { touch: touched(18), under: None }, 0));
        assert_eq!(next(ground, FOOT), Next::Ground(Placed { material: GroundMaterial::of(18), at: FOOT,
                                                            normal: [0.0, 1.0, 0.0] }),
                   "at the foot");
    }

    #[test]
    fn an_unnamed_touch_takes_the_hittable_surface_under_it() {
        // the loading station's walker strip (no material) over its metal beam
        let (ground, looks) = step(None, Some(touched(0x3FFF)), Some(hit(18, 0.01)));
        assert_eq!((ground, looks), (Ground::Touched { touch: touched(0x3FFF), under: Some(hit(18, 0.01)) }, 1));
        assert_eq!(next(ground, FOOT), Next::Ground(Placed { material: GroundMaterial::of(18), at: FOOT,
                                                            normal: [0.0, 1.0, 0.0] }));
        // nothing named under it: the face's own, which material_pair steps as default
        let (ground, _) = step(None, Some(touched(0x3FFF)), None);
        assert_eq!(next(ground, FOOT), Next::Ground(Placed { material: GroundMaterial::of(0x3FFF), at: FOOT,
                                                            normal: [0.0, 1.0, 0.0] }));
        let (ground, _) = step(None, Some(touched(0x3FFF)), Some(hit(0x3FFF, 0.5)));
        assert_eq!(ground, Ground::Touched { touch: touched(0x3FFF), under: None }, "an unnamed one is no better");
    }

    #[test]
    fn no_touch_and_no_hit_is_silent() {
        // in the air (the sweep met nothing), whatever is near: a handrail,
        // a ledge, a beam beside the foot
        assert_eq!(step(None, None, Some(hit(18, 0.0))), (Ground::Air, 0));
        assert_eq!(next(Ground::Air, FOOT), Next::Game);
    }

    #[test]
    fn water_over_the_ground_wades_at_its_surface() {
        let water = hit(32, -0.4);
        let ground = ground(None, Some(0.1), Some(touched(9)), |look| match look {
            Look::Water => Some(water),
            other => panic!("{other:?} taken"),
        });
        assert_eq!(ground, Ground::Wading(water));
        assert_eq!(next(ground, FOOT), Next::Ground(Placed { material: GroundMaterial::of(32), at: [1.0, 2.4, 3.0],
                                                            normal: [0.0, 1.0, 0.0] }));
        // over a walker-only bed the ray missed: the foot's height is the ground
        let wades = super::ground(None, None, None, |look| match look {
            Look::Water => Some(water),
            other => panic!("{other:?} taken"),
        });
        assert_eq!(wades, Ground::Wading(water));
    }

    #[test]
    fn a_film_of_water_or_water_under_the_ground_is_the_grounds() {
        let wade = |water: Option<Hit>, game: Option<f32>| {
            ground(None, game, Some(touched(18)), |look| match look {
                Look::Water => water,
                other => panic!("{other:?} taken"),
            })
        };
        assert_eq!(wade(Some(hit(32, -0.05)), Some(0.1)), Ground::Game);
        assert_eq!(wade(Some(hit(32, 0.5)), Some(0.0)), Ground::Game);
        assert_eq!(wade(Some(hit(32, 0.0)), Some(0.0)), Ground::Game);
        assert_eq!(wade(None, Some(0.1)), Ground::Game);
        assert_eq!(wade(None, None), Ground::Touched { touch: touched(18), under: None });
    }

    #[test]
    fn an_armed_ground_takes_no_look() {
        let placed = Placed { material: GroundMaterial::of(17), at: [5.0, 6.0, 7.0], normal: [0.0, 0.0, 1.0] };
        for game in [None, Some(0.1)] {
            let ground = ground(Some(placed), game, Some(touched(18)), |look| panic!("{look:?} taken"));
            assert_eq!(ground, Ground::Armed(placed));
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
