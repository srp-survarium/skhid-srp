//! The climbs of the stand-in's bodies, counted against what the client's body can climb (A199: a body got from a ledge back up 1.14 m onto
//! the platform it fell from, in 2-16 s, where the live bodies stayed 50 s to 9 min).
//!
//! A body gains height in three ways in the stand-in: it walks up a link of the lattice (`slide_supported`, at most the grid body's step, the
//! client's `cc_step_height` 0.80), the client's sprint lift carries it up a low top (`standin_jump::lift_check`, at most
//! `motor::LIFT_MAX_RISE_M` 1.11), or a jump lands it on a floor higher than the one it left (`standin_jump::fly`). Each is counted by its way and
//! the rise it made (the floor left to the floor reached); a rise over what that way can make in the client is an *over-limit climb* and is
//! kept as a line with where, when and how. Counting only: nothing here changes a decision of a bot or a move of a body.

use mapdata::Vec3;

/// The ways a body gains height, in the order of [`Climbs::counts`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Way {
    /// A walk link of the lattice.
    Walk,
    /// The sprint lift.
    Lift,
    /// A jump that is not a sprint jump (in place, short).
    ShortJump,
    /// A sprint jump.
    SprintJump,
}

impl Way {
    const ALL: [Way; 4] = [Way::Walk, Way::Lift, Way::ShortJump, Way::SprintJump];

    fn name(self) -> &'static str {
        match self {
            Way::Walk => "walk",
            Way::Lift => "lift",
            Way::ShortJump => "short_jump",
            Way::SprintJump => "sprint_jump",
        }
    }

    /// The highest rise the client's body makes this way, m: the step (`startup.cfg` `cc_step_height` 0.80); the sprint lift (1.11); the
    /// plain jump's apex (0.998, the jump law); the sprint jump's reach (1.30, `tallies/owner/jumps.rs` `REACH_M`).
    pub(crate) fn limit_m(self) -> f32 {
        match self {
            Way::Walk => 0.80,
            Way::Lift => 1.11,
            Way::ShortJump => 1.03,
            Way::SprintJump => 1.30,
        }
    }
}

/// A climb is counted from this rise on, m: below it the lattice's own small steps of a slope.
const MIN_RISE_M: f32 = 0.3;

/// Most over-limit climbs kept as lines.
const KEPT: usize = 400;

/// What the bodies of one game climbed.
#[derive(Default)]
pub(crate) struct Climbs {
    /// Body-ticks alive on a floor (the denominator of a rate).
    pub(crate) alive_ticks: u64,
    /// Climbs of [`MIN_RISE_M`] or more, by [`Way`].
    pub(crate) made: [u32; 4],
    /// Of those, the ones over the way's [`Way::limit_m`].
    pub(crate) over: [u32; 4],
    /// The ticks a jump walked on the floor before leaving it, and those whose move the client's capsule would have held at a wall
    /// (`SRP_BENCH_STAND_IN_CORNER=on` only; held for real with `SRP_BENCH_STAND_IN_JUMP_WALL=on`).
    pub(crate) prewalk_ticks: u32,
    pub(crate) prewalk_held: u32,
    lines: Vec<String>,
}

impl Climbs {
    /// A body of `team` at `place` went from `from` to `to` by `way` at `tick`; `note` says how (the jump's clip, the keys).
    pub(crate) fn note(&mut self, way: Way, tick: u32, place: usize, team: u8, from: Vec3, to: Vec3, note: &str) {
        let rise = to.y - from.y;
        if rise < MIN_RISE_M {
            return;
        }
        let i = Way::ALL.iter().position(|&w| w == way).expect("a way");
        self.made[i] += 1;
        if rise > way.limit_m() + 0.01 {
            self.over[i] += 1;
            if self.lines.len() < KEPT {
                self.lines.push(format!(
                    "climb_over: tick {tick} place {place} team {team} {} rise {rise:.2} limit {:.2} from ({:.2}, {:.2}, {:.2}) to ({:.2}, {:.2}, {:.2}) {note}",
                    way.name(),
                    way.limit_m(),
                    from.x,
                    from.y,
                    from.z,
                    to.x,
                    to.y,
                    to.z
                ));
            }
        }
    }

    /// The counts for a cell's log.
    pub(crate) fn line(&self) -> String {
        let mut parts: Vec<String> = Way::ALL
            .iter()
            .enumerate()
            .map(|(i, w)| format!("{} {}/{}", w.name(), self.over[i], self.made[i]))
            .collect();
        let hours = self.alive_ticks as f64 * f64::from(survarium::v026g::match_msgs::TICK_MS) / 3_600_000.0;
        parts.push(format!("jump_prewalk_held {}/{}", self.prewalk_held, self.prewalk_ticks));
        parts.push(format!("body_hours {hours:.2}"));
        format!("climbs (over limit/made): {}", parts.join(" "))
    }

    /// The over-limit climbs kept.
    pub(crate) fn lines(&self) -> &[String] {
        &self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::tests::standin_jump;
    use crate::v026g::drive::key;

    #[test]
    fn a_rise_over_the_ways_limit_is_kept_and_a_small_one_is_not_counted() {
        let mut c = Climbs::default();
        let at = |y| Vec3::new(1.0, y, 2.0);
        c.note(Way::Walk, 100, 3, 1, at(0.0), at(0.2), "");
        c.note(Way::Walk, 100, 3, 1, at(0.0), at(0.7), "");
        c.note(Way::Lift, 100, 3, 1, at(0.0), at(1.14), "");
        c.note(Way::SprintJump, 100, 3, 1, at(0.0), at(1.14), "");
        assert_eq!(c.made, [1, 1, 0, 1]);
        assert_eq!(c.over, [0, 1, 0, 0]);
        assert_eq!(c.lines().len(), 1);
        assert!(c.lines()[0].contains("lift rise 1.14"), "{}", c.lines()[0]);
        assert!(c.line().contains("lift 1/1") && c.line().contains("walk 0/1"), "{}", c.line());
    }

    /// A ledge 3 m up from x 10, and a slab's face over it that the lattice does not know (x 10.6, 3.65 to 4.14 m, the proportions of the real
    /// slab over the real ledge; the grid is built without it, as the real grid has the ledge's cells up to the platform's edge): a body that
    /// presses jump at the face walks off the edge and falls unless the jump's walk is held by the capsule rule, and then it jumps in place
    /// (A199, Chemical Plant day 4's pipe-top ledge under the side-1 platform).
    #[test]
    fn a_jump_at_a_slab_over_an_edge_does_not_walk_through_it_when_held() {
        use crate::v026g::bots::body::tests::wall_x;
        use crate::v026g::bots::tests::standin_capsule::Sections;
        let ledge = standin_jump::ledge_triangles(3.0, mapdata::WalkerBehaviour::BySlope);
        // The face looks east (toward the body), as the walker mesh's faces look out of their objects.
        let slab: Vec<_> = wall_x(10.6, 0.0, 10.0)
            .map(|mut t| {
                t.v.iter_mut().for_each(|v| v.y = 3.65 + v.y * 0.49 / 3.0);
                t.v.swap(1, 2);
                t
            })
            .to_vec();
        let all: Vec<_> = ledge.iter().chain(&slab).copied().collect();
        let grid = mapdata::WalkGrid::build(&ledge, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::client());
        let walls = mapdata::RayTree::build(&all);
        let sections = Sections::new(&all, grid.body().min_ground_normal_y);
        let mut body = standin_jump::honest_body(&grid, Vec3::new(11.1, 3.0, 5.0));
        body.yaw = std::f32::consts::FRAC_PI_2;
        let jump = key::FORWARD | key::JUMP;
        let (free, _) = standin_jump::honest_jump_in(&grid, &walls, None, body.clone(), jump, 0.0);
        let (held, _) = standin_jump::honest_jump_in(&grid, &walls, Some(&sections), body, jump, 0.0);
        assert!(free.y < 0.5, "unheld, the body walks off the edge and falls: {free:?}");
        assert!((held.y - 3.0).abs() < 0.1 && held.x > 10.9, "held, it stays on the ledge: {held:?}");
    }

    /// The height of `t` over the point (x, z) and its slope in degrees, when the point is inside the face's footprint and the face is not
    /// upright.
    fn face_at(t: &mapdata::Triangle, x: f32, z: f32) -> Option<(f32, f32)> {
        let [a, b, c] = t.v;
        let det = (b.z - c.z) * (a.x - c.x) + (c.x - b.x) * (a.z - c.z);
        if det.abs() < 1e-9 {
            return None;
        }
        let l1 = ((b.z - c.z) * (x - c.x) + (c.x - b.x) * (z - c.z)) / det;
        let l2 = ((c.z - a.z) * (x - c.x) + (a.x - c.x) * (z - c.z)) / det;
        let l3 = 1.0 - l1 - l2;
        if l1 < -1e-4 || l2 < -1e-4 || l3 < -1e-4 {
            return None;
        }
        let n = (b - a).cross(c - a);
        let deg = (n.y.abs() / n.length()).clamp(-1.0, 1.0).acos().to_degrees();
        Some((l1 * a.y + l2 * b.y + l3 * c.y, deg))
    }

    /// A measurement (A199): the lattice's floors and walk links around a point of a real map, how high a chain of walk links (each at most the
    /// grid body's step) reaches within a radius, and the height a body walking straight west from the point makes on `slide_supported` and
    /// `slide_capped`. `SRP_MAPDATA_DIR=<mapdata folder> SRP_CLIMB_PROJECT=level_04/day_4 SRP_CLIMB_AT=108.62,11.54,73.82 SRP_CLIMB_DIR=-1,0
    /// cargo test -p match-server --release climb_probe -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
    fn climb_probe() {
        use crate::v026g::bots::body::{slide_capped, slide_supported};
        use crate::v026g::bots::tests::bench_tuning;
        use mapdata::World;
        let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
            eprintln!("skipping: SRP_MAPDATA_DIR is not set");
            return;
        };
        let project = std::env::var("SRP_CLIMB_PROJECT").unwrap_or_else(|_| "level_04/day_4".to_owned());
        let nums = |name: &str, default: &str| -> Vec<f32> {
            std::env::var(name).unwrap_or_else(|_| default.to_owned()).split(',').map(|s| s.trim().parse().unwrap()).collect()
        };
        let at = nums("SRP_CLIMB_AT", "108.62,11.54,73.82");
        let dir_xz = nums("SRP_CLIMB_DIR", "-1,0");
        let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name(&project))).unwrap();
        let links = bench_tuning().off_mesh_links();
        let world = World::build_with(map, mapdata::CELL, bench_tuning().grid_body(), links, false);
        let grid = &world.grid;
        let start = grid.locate(Vec3::new(at[0], at[1], at[2]), 0.3).expect("a floor at the point");
        let p0 = grid.position(start);
        println!("grid body step {} cell {}; start floor {:.3} {:.3} {:.3}", grid.body().step, grid.cell_size(), p0.x, p0.y, p0.z);
        // The highest floor a chain of walk links, each of a rise within the step, reaches within 2.5 m of the start.
        let mut seen = std::collections::HashMap::new();
        let mut queue = std::collections::VecDeque::from([(start, 0u32)]);
        seen.insert(start, (start, 0u32));
        while let Some((node, depth)) = queue.pop_front() {
            let a = grid.position(node);
            for (next, kind) in grid.links(node) {
                if kind.is_leap() || seen.contains_key(&next) {
                    continue;
                }
                let b = grid.position(next);
                if b.distance_xz(p0) > 2.5 || b.y - a.y > grid.body().step {
                    continue;
                }
                seen.insert(next, (node, depth + 1));
                queue.push_back((next, depth + 1));
            }
        }
        let mut top = start;
        for &n in seen.keys() {
            if grid.position(n).y > grid.position(top).y {
                top = n;
            }
        }
        let t = grid.position(top);
        println!("walk-chain reach within 2.5 m: {} floors, the highest {:.3} {:.3} {:.3} (+{:.2} m)", seen.len(), t.x, t.y, t.z, t.y - p0.y);
        let mut chain = vec![top];
        while let Some(&(from, _)) = seen.get(chain.last().unwrap()) {
            if from == *chain.last().unwrap() {
                break;
            }
            chain.push(from);
        }
        for n in chain.iter().rev() {
            let p = grid.position(*n);
            println!("  chain floor {:.3} {:.3} {:.3}", p.x, p.y, p.z);
        }
        // The walker faces along the chain's first metre, every 0.125 m: each face whose footprint holds the point, within 1.5 m of the
        // floors' heights there, with its height and slope. A real stair shows flat treads (slope under 50 deg, the client's
        // `impassable_slope`) at the floors' heights; one slanted face steeper than 50 deg is ground the client neither walks nor steps.
        {
            let faces = world.map.walker_triangles();
            let ordered: Vec<Vec3> = chain.iter().rev().map(|&n| grid.position(n)).collect();
            let mut walked = 0.0;
            for pair in ordered.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let d = a.distance_xz(b);
                let parts = (d / 0.125).ceil().max(1.0) as usize;
                for k in 0..parts {
                    let s = k as f32 / parts as f32;
                    let at = Vec3::new(a.x + (b.x - a.x) * s, a.y + (b.y - a.y) * s, a.z + (b.z - a.z) * s);
                    let mut under: Vec<(f32, f32)> = faces
                        .iter()
                        .filter_map(|t| face_at(t, at.x, at.z))
                        .filter(|&(y, _)| (y - at.y).abs() <= 1.5)
                        .collect();
                    under.sort_by(|p, q| p.0.total_cmp(&q.0));
                    let list: Vec<String> = under.iter().map(|(y, deg)| format!("{y:.2}@{deg:.0}deg")).collect();
                    println!("  face at {:.2} {:.2} (along {walked:.2} m, chain {:.2}): {}", at.x, at.z, at.y, list.join(" "));
                    walked += d / parts as f32;
                }
                if walked > 1.0 {
                    break;
                }
            }
        }
        // A body walking along `dir`, 0.2 m a tick, 30 ticks.
        let (len, ) = (dir_xz[0].hypot(dir_xz[1]),);
        let step = Vec3::new(0.2 * dir_xz[0] / len, 0.0, 0.2 * dir_xz[1] / len);
        for (name, supported) in [("slide_supported", true), ("slide_capped", false)] {
            let (mut node, mut feet) = (start, p0);
            let mut line = String::new();
            for tick in 0..30 {
                let to = feet + step;
                (node, feet, _) = if supported {
                    slide_supported(grid, node, feet, to, grid.body().step, crate::v026g::bots::body::SUPPORT_M)
                } else {
                    slide_capped(grid, node, feet, to, grid.body().step)
                };
                if tick % 3 == 2 {
                    line.push_str(&format!(" ({:.2},{:.2})", feet.x, feet.y));
                }
            }
            println!("{name} walking {dir_xz:?}:{line}");
        }
        // The same walk under the client's capsule rule (`SRP_BENCH_STAND_IN_CORNER=on`): the move the walker mesh lets the body make, then the lattice.
        {
            use crate::v026g::bots::tests::standin_capsule::{RADIUS_M, Sections, Seg, client_move};
            let sections = Sections::new(&world.map.walker_triangles(), grid.body().min_ground_normal_y);
            let (mut node, mut feet) = (start, p0);
            let (mut line, mut segs) = (String::new(), Vec::<Seg>::new());
            for tick in 0..30 {
                let mv = (step.x, step.z);
                let reach = mv.0.hypot(mv.1) + RADIUS_M + 0.6;
                sections.near((feet.x, feet.z), feet.y, false, reach, &mut segs);
                let held = client_move(&segs, (feet.x, feet.z), mv, RADIUS_M);
                let target = Vec3::new(held.to.0, feet.y, held.to.1);
                (node, feet, _) = slide_supported(grid, node, feet, target, grid.body().step, crate::v026g::bots::body::SUPPORT_M);
                if tick % 3 == 2 {
                    line.push_str(&format!(" ({:.2},{:.2})", feet.x, feet.y));
                }
            }
            println!("capsule + slide_supported walking {dir_xz:?}:{line}");
        }
        // The honest jump from the point: eight ways, the short jump (forward + jump, at rest) and the sprint jump (at 5 m/s).
        let walls = mapdata::RayTree::build(&world.map.walker_triangles());
        let mut from_body = super::super::standin_jump::honest_body(grid, p0);
        from_body.feet = p0;
        for (name, keys, speed) in [
            ("short", key::FORWARD | key::JUMP, 0.0),
            ("sprint", key::FORWARD | key::SPRINT | key::JUMP, 5.0),
            ("in place", key::JUMP, 0.0),
        ] {
            let mut line = String::new();
            for k in 0..8 {
                let mut body = from_body.clone();
                body.yaw = k as f32 * std::f32::consts::FRAC_PI_4;
                let (to, top) = super::super::standin_jump::honest_jump(grid, &walls, body, keys, speed);
                line.push_str(&format!(" [yaw {k}: ({:.2},{:.2},{:.2}) top {top:.2}]", to.x, to.y, to.z));
            }
            println!("jump {name}:{line}");
        }
        // The same jumps with the walk before them held by the capsule rule (`SRP_BENCH_STAND_IN_JUMP_WALL=on`).
        let sections = crate::v026g::bots::tests::standin_capsule::Sections::new(&world.map.walker_triangles(), grid.body().min_ground_normal_y);
        for (name, keys, speed) in [("short", key::FORWARD | key::JUMP, 0.0), ("sprint", key::FORWARD | key::SPRINT | key::JUMP, 5.0)] {
            let mut line = String::new();
            for k in 0..8 {
                let mut body = from_body.clone();
                body.yaw = k as f32 * std::f32::consts::FRAC_PI_4;
                let (to, top) = super::super::standin_jump::honest_jump_in(grid, &walls, Some(&sections), body, keys, speed);
                line.push_str(&format!(" [yaw {k}: ({:.2},{:.2},{:.2}) top {top:.2}]", to.x, to.y, to.z));
            }
            println!("jump held {name}:{line}");
        }
    }
}
