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

#![forbid(unsafe_code)]

use crate::vostok::ray_test::{Body, Flight, RayTest, Step, Struck};
use crate::vostok::types::Place;
use crate::vostok::{Context, Flow, NoExit};

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

/// The share of the step's move a body is put back by for a crossing `age`
/// seconds before the step's end: the age over the step, at most the step.
pub fn share(age: f32, step: Step) -> f32 {
    let share = age * 1000.0 / step.ms() as f32;
    if share < 1.0 { share } else { 1.0 }
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
    if let Some(mut ray) = RayTest::at(ctx) {
        test(&mut ray);
    }
    Flow::Continue
}

fn test(ray: &mut RayTest<'_>) {
    let (Some(flight), Some(step)) = (ray.flight(), ray.step()) else { return };
    let (origin, direction, length) = (ray.origin(), ray.direction(), ray.length());
    let mut near_ray: [Option<Near<'_>>; Place::COUNT] = core::array::from_fn(|_| None);
    let mut count = 0;
    ray.bodies(|body| {
        let root = body.root();
        let middle = [root[0], root[1] + MIDDLE, root[2]];
        let crossed = share(age(flight, along(origin, direction, length, middle)), step);
        let back = body.motion(step).and_then(|motion| back(motion, crossed));
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
        let share = share(0.04, step(100, 150));
        assert!((share - 0.8).abs() < 1e-6, "{share}");
        let back = back([0.25, 0.0, 0.0], share).unwrap();
        assert!((back[0] - 0.2).abs() < 1e-6 && back[1] == 0.0 && back[2] == 0.0, "{back:?}");
        // crossed at the step's end: where he is
        assert_eq!(back_at(0.0, step(100, 150)), None);
        // still: where he is
        assert_eq!(super::back([0.0, -0.0, 0.0], 0.8), None);
    }

    fn back_at(age: f32, step: Step) -> Option<Vec3> {
        back([0.25, 0.0, 0.0], share(age, step))
    }

    #[test]
    fn never_before_the_step() {
        // a split step (an off-grid input 20 ms before the grid): its move is
        // the last 20 ms'; a crossing 40 ms before puts him back by all of it
        assert_eq!(share(0.04, step(130, 150)), 1.0);
        assert_eq!(share(0.01, step(130, 150)), 0.5);
        // a bullet older than the step (emitted late): no further than the step
        assert_eq!(share(0.12, step(100, 150)), 1.0);
        // and across the wrap of the game's clock
        assert_eq!(share(0.01, step(u32::MAX - 9, 10)), 0.5);
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
        let share = share(age(fired, 0.5), step(100, 150));
        assert!((share - 0.2).abs() < 1e-6, "{share}");
        // its whole flight is in the step: never back by more than its age
        assert!(super::share(age(fired, 0.0), step(100, 150)) <= 0.4 + 1e-6);
    }

    #[test]
    fn a_zero_length_segment() {
        // check_collision returns before its ray test for a ray under its
        // epsilon; were one tested, its crossing is its start
        let ray = Flight { start: 0.01, end: 0.01, step_end: 0.05 };
        let a = age(ray, along([1.0; 3], [0.0, 0.0, 1.0], 0.0, [5.0; 3]));
        assert!((a - 0.04).abs() < 1e-7);
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
