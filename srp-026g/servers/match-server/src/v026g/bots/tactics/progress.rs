//! How far forward a place is: the walk's cost from a side's spawns to it
//! against the cost from the other side's, per region, worked out once a
//! map, at the first ask (`TacticsBase::progress`): not at map load, not in
//! the stored base.
//!
//! `p = d_a / (d_a + d_b)`, `d_a` and `d_b` the cheapest walk-grid path
//! from the lowest and from the next team's spawn floors: 0 at the
//! first side's spawns, 1 at the other's, about 0.5 on the line between.
//! A side reads its own number (`for_side`): 0 at home, 1 at the enemy's.
//!
//! Root cause (`push-record.md` P1): "forward" was never measured by the
//! way a body walks. The straight distance puts a spot behind a wall on the
//! far side of a map's middle, and the humans' forward share (26.9 % of
//! their time past the middle, by path) could not be compared with the
//! bots' without it. The same table gives the forward controller its input
//! and the roam routes their "behind me" test.

use mapdata::regions::Regions;
use mapdata::{NodeId, Spawn, Vec3, WalkGrid};

/// What a region's number is where no walk leads to one of the sides.
pub const UNKNOWN: f32 = 0.5;

/// The progress of every region of a map.
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    /// The team whose spawns are `p` = 0 (the lowest); `None` when the
    /// map has fewer than two teams.
    low: Option<i32>,
    /// By region id.
    p: Vec<f32>,
}

/// The two sides of a map's spawns: the two lowest teams (a map may have
/// a third, level_02's team deathmatch); `None` for fewer than two.
fn teams(spawns: &[Spawn]) -> Option<(i32, i32)> {
    let mut teams: Vec<i32> = spawns.iter().map(|s| s.team).collect();
    teams.sort_unstable();
    teams.dedup();
    match teams[..] {
        [a, b, ..] => Some((a, b)),
        _ => None,
    }
}

impl Progress {
    /// The table of `regions` of `grid` for a map with `spawns`.
    pub fn build(grid: &WalkGrid, regions: &Regions, spawns: &[Spawn]) -> Self {
        let Some((a, b)) = teams(spawns) else {
            return Self {
                low: None,
                p: vec![UNKNOWN; regions.len()],
            };
        };
        let floors = |team: i32| -> Vec<NodeId> {
            spawns
                .iter()
                .filter(|s| s.team == team)
                .filter_map(|s| grid.locate(s.position, 1.5))
                .collect()
        };
        let (from_a, from_b) = (grid.distances_to(&floors(a)), grid.distances_to(&floors(b)));
        let p = (0..regions.len() as u32)
            .map(|r| {
                let node = regions.middle(r);
                let (da, db) = (from_a.get(node), from_b.get(node));
                if da.is_finite() && db.is_finite() && da + db > 0.0 {
                    da / (da + db)
                } else {
                    UNKNOWN
                }
            })
            .collect();
        Self { low: Some(a), p }
    }

    /// Region `r`'s progress for the bot of `side`: 0 at its own spawns, 1 at
    /// the enemy's. [`UNKNOWN`] for a side the map does not have.
    pub fn of_region(&self, r: u32, side: i32) -> f32 {
        let p = self.p.get(r as usize).copied().unwrap_or(UNKNOWN);
        match self.low {
            Some(low) if side == low => p,
            Some(_) => 1.0 - p,
            None => UNKNOWN,
        }
    }

    /// The progress at `point` for `side`; [`UNKNOWN`] off the grid.
    pub fn at(&self, grid: &WalkGrid, regions: &Regions, point: Vec3, side: i32) -> f32 {
        grid.locate(point, 1.5)
            .map_or(UNKNOWN, |node| self.of_region(regions.of(node), side))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::tactics::tests::field;

    #[test]
    fn progress_runs_from_0_at_a_sides_spawns_to_1_at_the_others_by_the_walk() {
        let world = field();
        let regions = Regions::build(&world.grid, None);
        let table = Progress::build(&world.grid, &regions, &world.map.spawns);
        let at = |x: f32, z: f32, side: i32| {
            table.at(&world.grid, &regions, Vec3::new(x, 0.0, z), side)
        };
        assert!(at(5.0, 10.0, 0) < 0.05, "home of side 0");
        assert!(at(75.0, 10.0, 0) > 0.95, "home of side 1, for side 0");
        assert!(at(5.0, 10.0, 1) > 0.95, "side 1 reads it the other way");
        let sum = at(30.0, 10.0, 0) + at(30.0, 10.0, 1);
        assert!((sum - 1.0).abs() < 1e-5, "the two sides' numbers add to 1");
    }

    #[test]
    fn a_wall_between_puts_a_spot_behind_it_back_by_the_walk_not_the_straight_line() {
        // The field's wall across x = 40 (z 0..25): the way round is at
        // z > 25. A spot just before the wall at z = 10 is farther from the
        // enemy's spawns by the walk than the straight line says.
        let world = field();
        let regions = Regions::build(&world.grid, None);
        let table = Progress::build(&world.grid, &regions, &world.map.spawns);
        let before_wall = table.at(&world.grid, &regions, Vec3::new(38.0, 0.0, 10.0), 0);
        let round_it = table.at(&world.grid, &regions, Vec3::new(38.0, 0.0, 32.0), 0);
        assert!(
            round_it > before_wall,
            "{round_it} should be farther on than {before_wall}"
        );
    }

    #[test]
    fn a_map_with_one_team_is_unknown_everywhere() {
        let world = field();
        let regions = Regions::build(&world.grid, None);
        let one_team: Vec<Spawn> = world
            .map
            .spawns
            .iter()
            .filter(|s| s.team == 0)
            .cloned()
            .collect();
        let table = Progress::build(&world.grid, &regions, &one_team);
        assert_eq!(table.at(&world.grid, &regions, Vec3::new(5.0, 0.0, 10.0), 0), UNKNOWN);
        assert_eq!(table.of_region(0, 7), UNKNOWN);
    }
}
