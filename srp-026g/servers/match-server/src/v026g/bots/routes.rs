//! Play 17's data: the routes humans took, per map, mode and side
//! (`player_bots_behaviour.md` §3 play 17).
//!
//! The table is private (it is made from the players' saved worlds), so it
//! is a file the server reads, `SRP_BOT_ROUTES`, never part of the repo.
//! Each row is one group of human lives that went the same way out of
//! their base: the group's median sideways offset at checkpoints 15-90 %
//! of the way along the base-to-base line, turned into map points (x, z),
//! and what the group gave a life (kills and assists, and twice the
//! batteries found and put in, in the battery modes). Without the file the
//! bots roam to the enemy's exits as before (`plays::roam`).
//!
//! The file (format 1):
//!
//! ```json
//! {"format": 1, "routes": [{"level": "level_01", "mode": 2, "team": 1,
//!   "lives": 22, "reward": 3.3, "kd": 2.9, "died": 0.9,
//!   "points": [[-22.7, -81.2], [4.6, -69.4]]}]}
//! ```
//!
//! `team` is the map side (spawn team) the route leaves from.

use std::path::Path;

use mapdata::{Vec3, WalkGrid};
use serde::Deserialize;
use survarium::v026g::maps::RuleMode;

#[derive(Debug, Deserialize)]
struct File {
    format: u32,
    routes: Vec<Row>,
    // Anything else (the source's note) is not the bots' business.
}

#[derive(Clone, Debug, Deserialize)]
struct Row {
    level: String,
    mode: u8,
    team: i32,
    lives: u32,
    reward: f32,
    #[serde(default)]
    #[allow(dead_code)]
    kd: f32,
    #[serde(default)]
    #[allow(dead_code)]
    died: f32,
    points: Vec<[f32; 2]>,
}

/// Every map's human routes, as read.
#[derive(Debug, Default)]
pub struct RouteTable {
    rows: Vec<Row>,
}

/// One human route of a map side, its points on the walk grid.
#[derive(Clone, Debug, PartialEq)]
pub struct HumanRoute {
    /// The map side it leaves from.
    pub team: i32,
    /// Lives in its group (how much its reward is worth).
    pub lives: u32,
    /// What a life on it gave.
    pub reward: f32,
    /// From its base outward. A point with no floor near is left out.
    pub points: Vec<Vec3>,
}

impl RouteTable {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let file: File = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if file.format != 1 {
            return Err(format!("format {} (1 is known)", file.format));
        }
        Ok(Self { rows: file.routes })
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The routes of a map (`level` 1-8) in a mode, points still on the
    /// ground plane (y 0) until [`HumanRoute::on_floors`].
    pub fn for_map(&self, level: u8, mode: RuleMode) -> Vec<HumanRoute> {
        let name = format!("level_{level:02}");
        self.rows
            .iter()
            .filter(|row| row.level == name && row.mode == mode.number() && row.lives > 0)
            .map(|row| HumanRoute {
                team: row.team,
                lives: row.lives,
                reward: row.reward.max(0.0),
                points: row
                    .points
                    .iter()
                    .filter(|p| p[0].is_finite() && p[1].is_finite())
                    .map(|p| Vec3::new(p[0], 0.0, p[1]))
                    .collect(),
            })
            .collect()
    }
}

impl HumanRoute {
    /// Its points put on the grid's floors: of the floors within 2 m of
    /// each (then 5 m) a body stands on, the nearest of the lowest (within
    /// 1 m of the lowest). A group's median path runs on the ground, not on
    /// the roofs over it; a point with no floor near (off the walkable map)
    /// is dropped.
    pub fn on_floors(mut self, grid: &WalkGrid) -> Self {
        let floor = |p: Vec3| {
            [2.0f32, 5.0].iter().find_map(|&r| {
                let floors: Vec<Vec3> = grid
                    .floors_in([p.x - r, p.z - r, p.x + r, p.z + r])
                    .filter(|&node| grid.can_stand(node))
                    .map(|node| grid.position(node))
                    .filter(|at| at.distance_xz(p) <= r)
                    .collect();
                let lowest = floors.iter().map(|at| at.y).fold(f32::INFINITY, f32::min);
                floors
                    .into_iter()
                    .filter(|at| at.y <= lowest + 1.0)
                    .min_by(|a, b| a.distance_xz(p).total_cmp(&b.distance_xz(p)))
            })
        };
        self.points = self.points.iter().filter_map(|&p| floor(p)).collect();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_is_read_by_map_and_mode_and_a_strange_format_is_refused() {
        let text = r#"{"format": 1, "source": "test", "routes": [
            {"level": "level_01", "mode": 2, "team": 1, "lives": 22, "reward": 3.3,
             "kd": 2.9, "died": 0.9, "points": [[-22.7, -81.2], [4.6, -69.4]]},
            {"level": "level_01", "mode": 4, "team": 0, "lives": 9, "reward": 1.0,
             "points": [[1.0, 2.0]]},
            {"level": "level_02", "mode": 2, "team": 0, "lives": 9, "reward": 1.0,
             "points": [[1.0, 2.0]]}]}"#;
        let table = RouteTable::parse(text).unwrap();
        assert_eq!(table.len(), 3);
        let routes = table.for_map(1, RuleMode::Batteries);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].team, 1);
        assert_eq!(routes[0].points[1], Vec3::new(4.6, 0.0, -69.4));
        assert!(table.for_map(3, RuleMode::Batteries).is_empty());
        assert!(RouteTable::parse(r#"{"format": 2, "routes": []}"#).is_err());
    }

    /// The real table on the real maps (private files): every route's
    /// points land on floors and it leaves from its own side. Run with
    /// `SRP_MAPDATA_DIR` and `SRP_BOT_ROUTES` set, in release.
    #[test]
    #[ignore = "needs the private map files and route table"]
    fn the_real_routes_land_on_the_real_maps_and_leave_from_their_own_side() {
        use crate::v026g::bots::tactics::Tactics;
        let (Some(dir), Some(table)) = (
            std::env::var_os("SRP_MAPDATA_DIR"),
            std::env::var_os("SRP_BOT_ROUTES"),
        ) else {
            panic!("set SRP_MAPDATA_DIR and SRP_BOT_ROUTES");
        };
        let table = RouteTable::load(table.as_ref()).unwrap();
        let (mut routes, mut points, mut kept, mut allowed, mut wrong_side) = (0, 0, 0, 0, 0);
        for level in 1..=8u8 {
            for mode in [
                RuleMode::Batteries,
                RuleMode::TimedBatteries,
                RuleMode::TeamDeathmatch,
            ] {
                let path =
                    std::path::Path::new(&dir).join(format!("level_{level:02}_day_{mode}.mapdata"));
                let map = mapdata::MapData::read(&path).unwrap();
                let world = mapdata::World::build(map, mapdata::CELL, mapdata::Body::default());
                let mut tactics = Tactics::new(std::sync::Arc::new(world), 1.0);
                let raw = table.for_map(level, mode);
                points += raw.iter().map(|r| r.points.len()).sum::<usize>();
                routes += raw.len();
                tactics.set_routes(raw);
                let centre = |team: i32| {
                    let s: Vec<Vec3> = tactics
                        .world
                        .map
                        .spawns
                        .iter()
                        .filter(|s| s.team == team)
                        .map(|s| s.position)
                        .collect();
                    s.iter().fold(Vec3::ZERO, |a, b| a + *b) * (1.0 / s.len().max(1) as f32)
                };
                for route in &tactics.routes {
                    kept += route.points.len();
                    allowed += route
                        .points
                        .iter()
                        .filter(|&&p| tactics.fine_goal(p, 0, Some(route.team)))
                        .count();
                    let other = tactics
                        .world
                        .map
                        .spawns
                        .iter()
                        .map(|s| s.team)
                        .find(|t| *t != route.team)
                        .unwrap();
                    let first = route.points[0];
                    if first.distance_xz(centre(route.team)) > first.distance_xz(centre(other)) {
                        wrong_side += 1;
                        eprintln!(
                            "level {level} mode {mode} team {}: starts nearer the other side {first:?}",
                            route.team
                        );
                    }
                }
                eprintln!("level {level} mode {mode}: {} routes", tactics.routes.len());
            }
        }
        eprintln!(
            "{routes} routes, {points} points: {kept} on floors, {allowed} allowed; {wrong_side} from the wrong side"
        );
        assert!(kept * 10 >= points * 9, "{kept} of {points}");
        assert_eq!(wrong_side, 0);
    }
}
