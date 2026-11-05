//! Patch 103: a bullet meets a player where he stood when it crossed him,
//! not where he stands at the end of the step (the owner, 2026-11-05: piece
//! 1 of pixel_correct_shooting_069.md section 8; the spec
//! docs/notes/subtick_bullet_hits.md).
//!
//! The game ticks bullets on the 50 ms grid only: at a grid step a bullet's
//! one ray spans its flight since the last (from its spawn's ms, or the last
//! step), and the players it is tested against are where the step left
//! them, up to 50 ms after the bullet passed (25 cm at 5 m/s). Here each
//! player near the ray is tested where he stood at the ms the bullet
//! reached him: put back along the move his character controller made in
//! the step, by the share of the step between that ms and the step's end.
//!
//! - The ms a player is crossed is the game's own hit time of the ray's
//!   point nearest his middle (start + (end - start) x along). One pass: in
//!   a step the bullet flies ~35 m and the player moves ~25 cm.
//! - The move is the controller's last step (m_last_pos to its position),
//!   which the world serializes, so every game, a late joiner's and a
//!   resynced one's too, puts him back alike. A step the world took in parts
//!   (an off-grid input splits it) is the last part's: a crossing before
//!   it puts him back by that part's whole move, no further.
//! - Translation only: his bones keep the step's pose about his root.
//! - Nothing of the physics world is moved: he is tested at a copy of his
//!   transform (vostok::ray_test). The level is tested at the step's end as
//!   before; a ray whose closest hit the game gave is a hit model is traced
//!   again for the level alone, and every player near it is tested, each
//!   where he stood, the closest hit winning (ties to the level, then to
//!   the lower place).
//! - Nobody moved (or nobody near the ray did): the game's own result,
//!   untouched.
//! - Patch 105 (`past_poses`): a target is tested that much further back
//!   again, the age his shooter declared for him when the shot was fired
//!   (capped by the match): back from the step's start along his ring of
//!   grid roots, held at its end and at his life's start. An age of 0 is
//!   the test above, bit for bit.

#![forbid(unsafe_code)]

use rewind::{Age, Knot, Path, Ring, Time};

use crate::past_poses;
use crate::vostok::ray_test::{Body, Flight, RayTest, Step, Struck};
use crate::vostok::types::Place;
use crate::vostok::{Context, Flow, NoExit, Thread};

type Vec3 = [f32; 3];

/// A body's middle, above its root (the feet): where its crossing is timed
/// and its reach measured from.
const MIDDLE: f32 = 0.9;
/// From the middle, every hit target of every pose (19 on 16 bones, a body
/// standing 1.9 m tall or lying about as long): a body whose middle is
/// further from the ray is not tested.
const REACH: f32 = 2.5;

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The fraction of the ray, 0..=1, nearest `point` (the ray from `origin`
/// along the unit `direction`, `length` long): where the bullet crosses a
/// body there. A ray of no length crosses everything at its start.
pub fn along(origin: Vec3, direction: Vec3, length: f32, point: Vec3) -> f32 {
    // NaN (a ray of no length, or of no number) compares false: its start
    let along = if length > 0.0 { dot(sub(point, origin), direction) / length } else { 0.0 };
    if along > 0.0 { along.min(1.0) } else { 0.0 }
}

/// The seconds before the step's end the bullet was `along` its ray: the
/// game's own hit time there (check_collision 0x32D7D6: start + (end -
/// start) x along), at least 0.
pub fn age(flight: Flight, along: f32) -> f32 {
    let at = flight.start + (flight.end - flight.start) * along;
    let age = flight.step_end - at;
    if age > 0.0 { age } else { 0.0 }
}

/// How many ms before the step's end a body is tested: the bullet's
/// crossing, `age` seconds before it but never before the step's start (the
/// step's move is all a crossing's time knows of), and the shooter's
/// declared `rewind` more (patch 105).
pub fn tested_ms(age: f32, step: Step, rewind: Age) -> f32 {
    let crossed = age * 1000.0;
    let step_ms = step.ms() as f32;
    (if crossed < step_ms { crossed } else { step_ms }) + rewind.ms() as f32
}

/// The share of the step's move a body is put back by when tested `ms`
/// before the step's end: the ms over the step, at most the step.
pub fn share(ms: f32, step: Step) -> f32 {
    let share = ms / step.ms() as f32;
    if share < 1.0 { share } else { 1.0 }
}

/// How far back a body is tested `ms` before the step's end, which moved
/// `motion` in the step (its share of it), and before the step's start, from
/// where it started the step back along `path` (patch 105's ring); none when
/// that is where it is.
pub fn tested_back(motion: Vec3, step: Step, ms: f32, path: impl FnOnce(Knot) -> Path) -> Option<Vec3> {
    if ms <= step.ms() as f32 {
        back(motion, share(ms, step))
    } else {
        back(path(Knot { ms: step.ms(), back: motion }).back_at(ms), 1.0)
    }
}

/// How far back a body that moved `motion` in the step stood at `share`
/// of the step before its end; none when that is where it is.
pub fn back(motion: Vec3, share: f32) -> Option<Vec3> {
    let back = motion.map(|m| m * share);
    (back != [0.0; 3]).then_some(back)
}

/// Whether a body whose middle is at `middle` can meet the ray: within
/// REACH of the ray's point nearest it.
pub fn near(origin: Vec3, direction: Vec3, length: f32, middle: Vec3) -> bool {
    let at = along(origin, direction, length, middle) * length;
    let nearest = [origin[0] + direction[0] * at, origin[1] + direction[1] * at, origin[2] + direction[2] * at];
    let off = sub(middle, nearest);
    dot(off, off) <= REACH * REACH
}

/// A body near the ray, and how far back it stood when crossed.
struct Near<'t> {
    body: Body<'t>,
    back: Option<Vec3>,
}

/// check_collision after the game's ray test (`bullet.ray_traced`, first
/// of the site's chain: the ragdoll and the lamps read the hit it leaves).
pub fn on_ray(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Some(mut ray) = RayTest::at(ctx) {
        test(&mut ray, thread);
    }
    Flow::Continue
}

fn test(ray: &mut RayTest<'_>, thread: &Thread) {
    let (Some(flight), Some(step)) = (ray.flight(), ray.step()) else { return };
    let (origin, direction, length) = (ray.origin(), ray.direction(), ray.length());
    let (shot, now) = (ray.shot(), Time(ray.now().0));
    let mut near_ray: [Option<Near<'_>>; Place::COUNT] = core::array::from_fn(|_| None);
    let mut count = 0;
    ray.bodies(|body| {
        let root = body.root();
        let middle = [root[0], root[1] + MIDDLE, root[2]];
        let crossed = age(flight, along(origin, direction, length, middle));
        let back = body.motion(step).and_then(|motion| {
            let rewind = body.place().map(|target| past_poses::rewind(thread, shot, target));
            let ms = tested_ms(crossed, step, rewind.as_ref().map_or(Age::ZERO, |r| r.age));
            tested_back(motion, step, ms, |first| {
                let ring = rewind.map_or(Ring::EMPTY, |r| r.ring);
                Path::new(&ring, now, body.feet(), first, Time(body.life().0))
            })
        });
        let stood = back.map_or(middle, |back| sub(middle, back));
        if count < near_ray.len() && near(origin, direction, length, stood) {
            near_ray[count] = Some(Near { body, back });
            count += 1;
        }
    });
    let near_ray = &near_ray[..count];
    if !near_ray.iter().flatten().any(|near| near.back.is_some()) {
        return;
    }
    // the game's hit is a body: the level behind it is not known yet
    let every = match ray.struck() {
        Struck::Body(id) => {
            if !near_ray.iter().flatten().any(|near| near.body.id() == id) {
                return;
            }
            ray.trace_level();
            true
        }
        Struck::Nothing | Struck::Level => false,
    };
    for near in near_ray.iter().flatten() {
        if every || near.back.is_some() {
            ray.trace_body(&near.body, near.back);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::types::GameTime;

    fn step(from: u32, to: u32) -> Step {
        Step::new(GameTime(from), GameTime(to)).unwrap()
    }

    /// A first segment fired at the step's start: 0..0.05 s, the step
    /// ending at 0.05.
    const WHOLE: Flight = Flight { start: 0.0, end: 0.05, step_end: 0.05 };

    #[test]
    fn the_crossing_is_the_games_hit_time() {
        // a crossing a fifth of the way: 10 ms into the step, 40 ms before its end
        assert_eq!(along([0.0; 3], [1.0, 0.0, 0.0], 35.0, [7.0, 1.0, 0.0]), 0.2);
        let age = age(WHOLE, 0.2);
        assert!((age - 0.04).abs() < 1e-7, "{age}");
        // a piece of the flight ending before the step's end (the trajectory's
        // next piece is another ray): the end of the piece is still before it
        let piece = Flight { start: 0.0, end: 0.03, step_end: 0.05 };
        assert!((super::age(piece, 1.0) - 0.02).abs() < 1e-7);
        // a segment after a pierce: its times since the pierce
        let pierced = Flight { start: 0.0, end: 0.012, step_end: 0.012 };
        assert_eq!(super::age(pierced, 1.0), 0.0);
    }

    #[test]
    fn the_crossing_is_on_the_ray() {
        assert_eq!(along([0.0; 3], [1.0, 0.0, 0.0], 10.0, [-3.0, 0.0, 0.0]), 0.0, "behind the start");
        assert_eq!(along([0.0; 3], [1.0, 0.0, 0.0], 10.0, [30.0, 0.0, 0.0]), 1.0, "past the end");
        assert_eq!(along([0.0; 3], [1.0, 0.0, 0.0], 0.0, [3.0, 0.0, 0.0]), 0.0, "a ray of no length");
        assert_eq!(along([0.0; 3], [1.0, 0.0, 0.0], f32::NAN, [3.0, 0.0, 0.0]), 0.0);
        assert_eq!(along([f32::NAN; 3], [1.0, 0.0, 0.0], 1.0, [3.0, 0.0, 0.0]), 0.0);
    }

    #[test]
    fn put_back_along_the_steps_move() {
        // 25 cm in the step (5 m/s), crossed 40 ms before its end: 20 cm back
        let share = share(tested_ms(0.04, step(100, 150), Age::ZERO), step(100, 150));
        assert!((share - 0.8).abs() < 1e-6, "{share}");
        let back = back([0.25, 0.0, 0.0], share).unwrap();
        assert!((back[0] - 0.2).abs() < 1e-6 && back[1] == 0.0 && back[2] == 0.0, "{back:?}");
        // crossed at the step's end: where he is
        assert_eq!(back_at(0.0, step(100, 150)), None);
        // still: where he is
        assert_eq!(super::back([0.0, -0.0, 0.0], 0.8), None);
    }

    fn back_at(age: f32, step: Step) -> Option<Vec3> {
        back([0.25, 0.0, 0.0], share(tested_ms(age, step, Age::ZERO), step))
    }

    #[test]
    fn never_before_the_step() {
        // a split step (an off-grid input 20 ms before the grid): its move is
        // the last 20 ms'; a crossing 40 ms before puts him back by all of it
        let at = |age: f32, step: Step| share(tested_ms(age, step, Age::ZERO), step);
        assert_eq!(at(0.04, step(130, 150)), 1.0);
        assert_eq!(at(0.01, step(130, 150)), 0.5);
        // a bullet older than the step (emitted late): no further than the step
        assert_eq!(at(0.12, step(100, 150)), 1.0);
        // and across the wrap of the game's clock
        assert_eq!(at(0.01, step(u32::MAX - 9, 10)), 0.5);
    }

    #[test]
    fn the_steps_end_is_the_steps_end() {
        // a ray starting at the step's end (start = end = step_end): crossed now
        let now = Flight { start: 0.05, end: 0.05, step_end: 0.05 };
        assert_eq!(age(now, 0.0), 0.0);
        assert_eq!(age(now, 1.0), 0.0);
        assert_eq!(back_at(age(now, 0.5), step(100, 150)), None);
    }

    #[test]
    fn a_bullet_spawned_in_the_step() {
        // fired 20 ms before the step's end: its flight is 0..0.02 s; crossed
        // halfway, 10 ms before the end, a fifth of a whole step's move
        let fired = Flight { start: 0.0, end: 0.02, step_end: 0.02 };
        let share = share(tested_ms(age(fired, 0.5), step(100, 150), Age::ZERO), step(100, 150));
        assert!((share - 0.2).abs() < 1e-6, "{share}");
        // its whole flight is in the step: never back by more than its age
        assert!(super::share(tested_ms(age(fired, 0.0), step(100, 150), Age::ZERO), step(100, 150)) <= 0.4 + 1e-6);
    }

    #[test]
    fn a_zero_length_segment() {
        // check_collision returns before its ray test for a ray under its
        // epsilon; were one tested, its crossing is its start
        let ray = Flight { start: 0.01, end: 0.01, step_end: 0.05 };
        let a = age(ray, along([1.0; 3], [0.0, 0.0, 1.0], 0.0, [5.0; 3]));
        assert!((a - 0.04).abs() < 1e-7);
    }

    /// A runner along +x at 5 m/s: his ring's roots at 1000..=1150 (x 0,
    /// 0.25, 0.5, 0.75), the step 1150 -> 1200 his move of 0.25 to x 1.0.
    fn runner() -> Ring {
        let mut ring = Ring::EMPTY;
        for n in 0..4 {
            ring.record(Time(1000 + 50 * n), [0.25 * n as f32, 0.0, 0.0]);
        }
        ring
    }

    fn ms(age: u32) -> Age {
        Age::new(age).unwrap()
    }

    #[test]
    fn rewound_by_the_declared_age_along_the_ring() {
        let ring = runner();
        let step = step(1150, 1200);
        let path = |first| Path::new(&ring, Time(1200), [1.0, 0.0, 0.0], first, Time(0));
        // crossed 10 ms before the step's end, 90 declared: 100 ms back, x 0.5
        let at = tested_ms(0.01, step, ms(90));
        assert!((at - 100.0).abs() < 1e-4, "{at}");
        let back = tested_back([0.25, 0.0, 0.0], step, at, path).unwrap();
        assert!((back[0] - 0.5).abs() < 1e-4 && back[1] == 0.0 && back[2] == 0.0, "{back:?}");
        // still in the step (10 + 30): the step's move's share, as patch 103
        let at = tested_ms(0.01, step, ms(30));
        assert_eq!(tested_back([0.25, 0.0, 0.0], step, at, path), back_of(0.8));
        // the most (150) and a crossing at the step's start: 200 back, the
        // ring's oldest (x 0); further is held there
        let at = tested_ms(0.05, step, Age::MAX);
        assert_eq!(at, 200.0);
        assert_eq!(tested_back([0.25, 0.0, 0.0], step, at, path), Some([1.0, 0.0, 0.0]));
        // a target spawned at 1075: his roots of 1000 and 1050 are not his
        let spawned = |first| Path::new(&ring, Time(1200), [1.0, 0.0, 0.0], first, Time(1075));
        assert_eq!(tested_back([0.25, 0.0, 0.0], step, 200.0, spawned), Some([0.5, 0.0, 0.0]));
        // a step in parts (1180 -> 1200, his move 0.1): crossed 30 ms before
        // its end, held at its start (20) as patch 103, 60 more: 80 back,
        // between 1150 (50 back, 0.25) and 1100 (100 back, 0.5)
        let part = self::step(1180, 1200);
        let at = tested_ms(0.03, part, ms(60));
        assert_eq!(at, 80.0);
        let back = tested_back([0.1, 0.0, 0.0], part, at, path).unwrap();
        assert!((back[0] - 0.4).abs() < 1e-4, "{back:?}");
    }

    fn back_of(share: f32) -> Option<Vec3> {
        back([0.25, 0.0, 0.0], share)
    }

    #[test]
    fn an_age_of_0_is_patch_103s_test_bit_for_bit() {
        // patch 103's own: the crossing's age over the step, at most the step
        let piece_1 = |age: f32, step: Step| {
            let share = age * 1000.0 / step.ms() as f32;
            back([0.25, -0.0, 0.1], if share < 1.0 { share } else { 1.0 })
        };
        let ring = runner();
        let path = |first| Path::new(&ring, Time(1200), [1.0, 0.0, 0.0], first, Time(0));
        for step in [step(1150, 1200), step(1180, 1200), step(u32::MAX - 9, 10)] {
            for age in [0.0, 0.0001, 0.013, 0.02, 0.0357, 0.05, 0.12, f32::NAN] {
                let ours = tested_back([0.25, -0.0, 0.1], step, tested_ms(age, step, Age::ZERO), path);
                assert_eq!(ours.map(|b| b.map(f32::to_bits)), piece_1(age, step).map(|b| b.map(f32::to_bits)),
                           "{age} in {step:?}");
            }
        }
    }

    #[test]
    fn near_the_ray() {
        let (o, d) = ([0.0; 3], [0.0, 0.0, -1.0]);
        assert!(near(o, d, 30.0, [2.4, 0.0, -10.0]));
        assert!(!near(o, d, 30.0, [2.6, 0.0, -10.0]));
        assert!(near(o, d, 30.0, [0.0, 0.0, 2.0]), "behind the start, within reach of it");
        assert!(!near(o, d, 30.0, [0.0, 0.0, 3.0]));
        assert!(!near(o, d, 30.0, [0.0, 0.0, -33.0]), "past the end");
    }
}
