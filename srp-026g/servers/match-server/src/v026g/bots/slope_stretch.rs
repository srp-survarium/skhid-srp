//! [A322] The reckoned body's speed up stairs and ramps by the grade of the stretch ahead (`kin.slope_speed: stretch`).
//!
//! The client slows going up, on every map and every kind of climb, and not going down (15 client-test recordings, steady keys,
//! 1 s pairs; fx-smoke a322/climb_speed.py). Its forward walk over the ground is 0.95x the flat speed at a grade of 0.15-0.3,
//! 0.90x at 0.3-0.5 and 0.79x over 0.5 (a sprint 0.96 / 0.90 / 0.83). Down it keeps 0.95-0.99x. The fit
//! `1 - 0.3 x grade` gives 0.93, 0.88 and 0.79 there.
//!
//! The grade is the walk's over a stretch ahead, not one tick's rise. A 50 ms tick over a stair crosses a riser (a cliff) or a
//! tread (flat), so a per-tick rule (`SlopeSpeed::Up`) slowed stairs in jerks, and the lab found it worse on Vostok Radar
//! Station's and Chemical Plant's stairs. Yet the client slows on those maps too (0.90x and 0.83x at 0.3-0.5).

use mapdata::{LinkKind, NodeId, Vec3, WalkGrid};

/// How far ahead along the walk the grade is taken, m.
pub const STRETCH_M: f32 = 1.0;
/// The client's slow-down per unit of grade going up.
pub const SLOW_PER_GRADE: f32 = 0.3;
/// The least share of the ground step covered (a grade of 1.67 and steeper).
pub const MIN_SHARE: f32 = 0.5;

/// The share of a ground step the body covers walking along `dir` (the walk's ground direction, any length) from `feet` on
/// `node`: the grid's walk links followed from `node` toward `dir` for [`STRETCH_M`], and the rise to the floor reached over
/// that run gives the grade. 1 where the stretch does not rise (or no walk link leads on).
pub fn share(grid: &WalkGrid, node: NodeId, feet: Vec3, dir: [f32; 2]) -> f32 {
    let len = dir[0].hypot(dir[1]);
    if len < 1e-4 {
        return 1.0;
    }
    let (dx, dz) = (dir[0] / len, dir[1] / len);
    let mut at = node;
    let mut run = 0.0f32;
    let mut end = grid.position(node);
    while run < STRETCH_M {
        let here = grid.position(at);
        // The walk link that goes on most along the walk (at least half its length along it).
        let next = grid
            .links(at)
            .filter(|&(_, kind)| kind == LinkKind::Walk)
            .map(|(n, _)| {
                let p = grid.position(n);
                let (ox, oz) = (p.x - here.x, p.z - here.z);
                let step = ox.hypot(oz).max(1e-4);
                (n, p, (ox * dx + oz * dz) / step, ox * dx + oz * dz)
            })
            .filter(|&(_, _, cos, _)| cos >= 0.5)
            .max_by(|a, b| a.2.total_cmp(&b.2));
        let Some((n, p, _, along)) = next else { break };
        run += along;
        end = p;
        at = n;
    }
    let rise = end.y - feet.y;
    if run < 0.25 || rise <= 0.0 {
        return 1.0;
    }
    (1.0 - SLOW_PER_GRADE * rise / run).max(MIN_SHARE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapdata::{Triangle, WalkerBehaviour};

    fn quad(tris: &mut Vec<Triangle>, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        tris.push(Triangle { v: [a, b, c], walker: WalkerBehaviour::Walk });
        tris.push(Triangle { v: [a, c, d], walker: WalkerBehaviour::Walk });
    }

    /// A flat floor x 0..4, then a ramp rising 0.5 m a metre x 4..8 (forced walk faces), then flat again.
    fn ramp() -> WalkGrid {
        let mut t = Vec::new();
        let (z0, z1) = (-2.0, 2.0);
        quad(&mut t, Vec3::new(0.0, 0.0, z0), Vec3::new(4.0, 0.0, z0), Vec3::new(4.0, 0.0, z1), Vec3::new(0.0, 0.0, z1));
        quad(&mut t, Vec3::new(4.0, 0.0, z0), Vec3::new(8.0, 2.0, z0), Vec3::new(8.0, 2.0, z1), Vec3::new(4.0, 0.0, z1));
        quad(&mut t, Vec3::new(8.0, 2.0, z0), Vec3::new(12.0, 2.0, z0), Vec3::new(12.0, 2.0, z1), Vec3::new(8.0, 2.0, z1));
        WalkGrid::build(&t, [0.0, z0, 12.0, z1], mapdata::CELL, mapdata::Body::default())
    }

    #[test]
    fn up_a_ramp_the_step_shrinks_by_its_grade_and_flat_or_down_it_does_not() {
        let g = ramp();
        let height = |x: f32| ((x - 4.0) * 0.5).clamp(0.0, 2.0);
        let on = |x: f32| g.locate(Vec3::new(x, height(x) + 0.3, 0.1), 0.3).expect("a floor");
        let up = share(&g, on(5.0), g.position(on(5.0)), [1.0, 0.0]);
        assert!((up - (1.0 - 0.3 * 0.5)).abs() < 0.03, "up the 0.5 grade: {up}");
        assert_eq!(share(&g, on(1.0), g.position(on(1.0)), [1.0, 0.0]), 1.0, "flat ahead");
        assert_eq!(share(&g, on(6.0), g.position(on(6.0)), [-1.0, 0.0]), 1.0, "down the ramp");
        assert_eq!(share(&g, on(5.0), g.position(on(5.0)), [0.0, 0.0]), 1.0, "no walk");
    }
}
