//! [A322] The reckoned body's speed up stairs and ramps by the grade of the stretch ahead (`kin.slope_speed: stretch`).
//!
//! The client's walk tick (`mapdata::controller::tick`, proven against the exe in Unicorn) keeps a walk's length and turns it
//! along what the body climbs: up a ramp the move goes along the face (`corrected_walk` / `get_wall_slide_vector` 0x7AC50), up
//! stairs the step-up lifts the body inside the same move. So its ground speed going up is the walk speed times the cosine of
//! the climb's grade, and going down (`step_down`) the whole of it. Probe (fx-smoke a322, climb_tick):
//! - ramps of grade 0.1-1.2: up = cos(atan(grade)) to three decimals (0.894 at 0.5, 0.707 at 1.0);
//! - stairs: the cosine of the stair's mean grade (riser / tread; 0.800 at 0.15 / 0.20, where cos is 0.800);
//! - down: 1.000 on all of them;
//! - real ground (the 15 client-test recordings' steady forward climbs, the tick walked from each world's feet): 0.958 / 0.909
//!   / 0.835 at grades 0.15-0.3 / 0.3-0.5 / 0.5+, against the client's 0.951 / 0.895 / 0.792.
//!
//! The grade is the walk's over a stretch ahead, not one tick's rise. A 50 ms tick over a stair crosses a riser (a cliff) or a
//! tread (flat), so a per-tick rule (`SlopeSpeed::Up`) slowed stairs in jerks, and the lab found it worse on Vostok Radar
//! Station's and Chemical Plant's stairs. Yet the client slows on those maps too.

use mapdata::{LinkKind, NodeId, WalkGrid};

/// How far ahead along the walk the grade is taken, m.
pub const STRETCH_M: f32 = 1.0;

/// The share of a ground step the body covers walking along `dir` (the walk's ground direction, any length) from its floor
/// `node` (heights from the grid's floors, never the body's feet, which a fall may hold above the floor): the grid's walk links followed from `node` toward `dir` for [`STRETCH_M`], and the rise to the floor reached over
/// that run gives the grade; the share is its cosine. 1 where the stretch does not rise (or no walk link leads on).
pub fn share(grid: &WalkGrid, node: NodeId, dir: [f32; 2]) -> f32 {
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
    let rise = end.y - grid.position(node).y;
    if run < 0.25 || rise <= 0.0 {
        return 1.0;
    }
    run / run.hypot(rise)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapdata::{Triangle, Vec3, WalkerBehaviour};

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
        let up = share(&g, on(5.0), [1.0, 0.0]);
        assert!((up - 0.5f32.atan().cos()).abs() < 0.02, "up the 0.5 grade: its cosine, {up}");
        assert_eq!(share(&g, on(1.0), [1.0, 0.0]), 1.0, "flat ahead");
        assert_eq!(share(&g, on(6.0), [-1.0, 0.0]), 1.0, "down the ramp");
        assert_eq!(share(&g, on(5.0), [0.0, 0.0]), 1.0, "no walk");
    }
}
