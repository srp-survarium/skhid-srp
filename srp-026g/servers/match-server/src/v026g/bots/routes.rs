//! Play 17's data: the routes humans took, per map, mode and side
//! (`player_bots_behaviour.md` §3 play 17).
//!
//! The table is private (it is made from the players' saved worlds), so it
//! is a file the server reads, never part of the repo: `bot-routes.json` in
//! the mapdata folder (`SRP_MAPDATA_DIR`), or the file `SRP_BOT_ROUTES`
//! names (empty or `off`: none).
//! Each row is one group of human lives that went the same way out of
//! their base: the group's median sideways offset at checkpoints 15-90 %
//! of the way along the base-to-base line, turned into map points (x, z),
//! and what the group gave a life (kills and assists, and twice the
//! batteries found and put in, in the battery modes). Without the file the
//! bots roam to the enemy's exits as before (`plays::roam`).
//!
//! The file (format 2):
//!
//! ```json
//! {"format": 2, "routes": [{"level": "level_01", "mode": 2, "team": 1,
//!   "lives": 22, "reward": 3.3, "fight": 2.5, "battery": 0.4,
//!   "kd": 2.9, "died": 0.9, "points": [[-22.7, -81.2], [4.6, -69.4]]}]}
//! ```
//!
//! `team` is the map side (spawn team) the route leaves from. What a route
//! is good for, so a bot takes the one that fits its task (play 17): `fight`
//! is the kills and assists a life on it gave, `battery` the batteries found
//! and put in (battery modes; 0 in Team Deathmatch), `reward` = `fight` + 2
//! × `battery`. A row may say whose lives it is made of (`by`: every
//! player's, or the top players' of matches of 10+ players, put on the
//! every-player scale by the side's average life); the bots do not read it.
//! Format 1 files (no `fight`, `battery`) are read too: their reward counts
//! as fight.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use mapdata::{Vec3, WalkGrid};
use serde::Deserialize;

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
    fight: Option<f32>,
    #[serde(default)]
    battery: Option<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    kd: f32,
    #[serde(default)]
    died: f32,
    points: Vec<[f32; 2]>,
}

/// Every map's human routes, as read.
#[derive(Debug, Default)]
pub struct RouteTable {
    rows: Vec<Row>,
}

/// One human route of a map side, its points on the walk grid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HumanRoute {
    /// The map side it leaves from.
    pub team: i32,
    /// Lives in its group (how much its reward is worth).
    pub lives: u32,
    /// What a life on it gave: `fight` + 2 × `battery`.
    pub reward: f32,
    /// Kills and assists a life on it gave.
    pub fight: f32,
    /// Batteries found and put in a life on it gave (0 in Team
    /// Deathmatch, and in a format 1 table).
    pub battery: f32,
    /// The share of its lives that ended in a death on the way.
    pub died: f32,
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
        if !(1..=2).contains(&file.format) {
            return Err(format!("format {} (1 and 2 are known)", file.format));
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
    pub fn for_map(&self, level: u8, mode: u8) -> Vec<HumanRoute> {
        let name = format!("level_{level:02}");
        self.rows
            .iter()
            .filter(|row| row.level == name && row.mode == mode && row.lives > 0)
            .map(|row| HumanRoute {
                team: row.team,
                lives: row.lives,
                reward: row.reward.max(0.0),
                fight: row.fight.unwrap_or(row.reward).max(0.0),
                battery: row.battery.unwrap_or(0.0).max(0.0),
                died: row.died.clamp(0.0, 1.0),
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
    /// each (then 5 m) a body stands on and a way is planned to (none in a
    /// pocket with no way out, [`WalkGrid::in_shut_pocket`]), the nearest of
    /// the lowest (within 1 m of the lowest). A group's median path runs on
    /// the ground, not on the roofs over it, nor in a pit under the floor it
    /// walks (level_05's, under the walkway at (-7.8, -21.3)); a point with
    /// no floor near (off the walkable map) is dropped.
    pub fn on_floors(mut self, grid: &WalkGrid) -> Self {
        let floor = |p: Vec3| {
            [2.0f32, 5.0].iter().find_map(|&r| {
                let floors: Vec<Vec3> = grid
                    .floors_in([p.x - r, p.z - r, p.x + r, p.z + r])
                    .filter(|&node| grid.can_stand(node) && !grid.in_shut_pocket(node))
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

/// The table's name in the mapdata folder (`SRP_MAPDATA_DIR`), read when
/// `SRP_BOT_ROUTES` is unset.
pub const DEFAULT_FILE: &str = "bot-routes.json";

/// Where a match's route table comes from.
#[derive(Debug, PartialEq, Eq)]
pub enum Source {
    /// `SRP_BOT_ROUTES` names a file.
    Set(PathBuf),
    /// `SRP_BOT_ROUTES` unset, and the mapdata folder has `bot-routes.json`.
    Default(PathBuf),
    /// `SRP_BOT_ROUTES` empty or `off`.
    Off,
    /// `SRP_BOT_ROUTES` unset, and no `bot-routes.json` (or no mapdata folder).
    Missing,
}

impl Source {
    /// `var` is `SRP_BOT_ROUTES`, `mapdata` the mapdata folder.
    pub fn choose(var: Option<&OsStr>, mapdata: Option<&Path>) -> Self {
        match var {
            Some(var) if var.is_empty() || var == "off" => Self::Off,
            Some(var) => Self::Set(PathBuf::from(var)),
            None => match mapdata.map(|dir| dir.join(DEFAULT_FILE)) {
                Some(path) if path.is_file() => Self::Default(path),
                _ => Self::Missing,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is read from the mapdata folder unless `SRP_BOT_ROUTES`
    /// says otherwise: a path, or empty or `off` for none.
    #[test]
    fn the_table_is_the_mapdata_folders_unless_the_variable_says_otherwise() {
        let with = std::env::temp_dir().join(format!("srp-routes-source-{}", std::process::id()));
        let without = with.join("empty");
        std::fs::create_dir_all(&without).unwrap();
        let default = with.join(DEFAULT_FILE);
        std::fs::write(&default, r#"{"format": 2, "routes": []}"#).unwrap();
        assert_eq!(Source::choose(None, Some(&with)), Source::Default(default.clone()));
        assert_eq!(Source::choose(None, Some(&without)), Source::Missing);
        assert_eq!(Source::choose(None, None), Source::Missing);
        for off in ["", "off"] {
            assert_eq!(Source::choose(Some(OsStr::new(off)), Some(&with)), Source::Off);
        }
        assert_eq!(
            Source::choose(Some(OsStr::new("/elsewhere/routes.json")), Some(&with)),
            Source::Set(PathBuf::from("/elsewhere/routes.json"))
        );
        assert!(RouteTable::load(&default).is_ok_and(|table| table.len() == 0));
        std::fs::remove_file(&default).unwrap();
        std::fs::remove_dir(&without).unwrap();
        std::fs::remove_dir(&with).unwrap();
    }

    /// Format 2 says what a route is good for; format 1's reward counts as
    /// fight.
    #[test]
    fn format_2_reads_fight_and_battery_and_format_1_still_reads() {
        let text = r#"{"format": 2, "routes": [
            {"level": "level_03", "mode": 2, "team": 0, "lives": 12, "reward": 2.6,
             "fight": 1.4, "battery": 0.6, "kd": 1.5, "died": 0.8, "by": "top",
             "points": [[1.0, 2.0]]}]}"#;
        let route = &RouteTable::parse(text).unwrap().for_map(3, 2)[0];
        assert_eq!(
            (route.fight, route.battery, route.died, route.reward),
            (1.4, 0.6, 0.8, 2.6)
        );
        let old = r#"{"format": 1, "routes": [{"level": "level_03", "mode": 2, "team": 0,
            "lives": 12, "reward": 2.6, "points": [[1.0, 2.0]]}]}"#;
        let route = &RouteTable::parse(old).unwrap().for_map(3, 2)[0];
        assert_eq!((route.fight, route.battery), (2.6, 0.0));
    }

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
        let routes = table.for_map(1, 2);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].team, 1);
        assert_eq!(routes[0].points[1], Vec3::new(4.6, 0.0, -69.4));
        assert!(table.for_map(3, 2).is_empty());
        assert!(RouteTable::parse(r#"{"format": 3, "routes": []}"#).is_err());
    }

    /// How walkable each route of the real table is for a bot (private
    /// files): from its side's spawn centre through its points, each leg
    /// planned as a bot of its side plans (off hazards and the other
    /// side's spawn area): the legs with no way, the jumps on the way, the
    /// way's length against the straight one, and the points lost (no
    /// floor near, or not allowed). With `SRP_ROUTES_REACH_OUT` a TSV of
    /// it, a row a route in the file's order. Run in release.
    #[test]
    #[ignore = "needs the private map files and route table"]
    fn how_walkable_the_real_routes_are() {
        use crate::v026g::bots::tactics::Tactics;
        let (Some(dir), Some(path)) = (
            std::env::var_os("SRP_MAPDATA_DIR"),
            std::env::var_os("SRP_BOT_ROUTES"),
        ) else {
            panic!("set SRP_MAPDATA_DIR and SRP_BOT_ROUTES");
        };
        let table = RouteTable::load(path.as_ref()).unwrap();
        let mut out = String::from(
            "row	level	mode	team	points	lost	legs	no_way	jumps	length_m	straight_m
",
        );
        let mut worlds = std::collections::HashMap::new();
        for (index, row) in table.rows.iter().enumerate() {
            let level: u8 = row.level.trim_start_matches("level_").parse().unwrap();
            let tactics = worlds.entry((level, row.mode)).or_insert_with(|| {
                let path = std::path::Path::new(&dir)
                    .join(format!("level_{level:02}_day_{}.mapdata", row.mode));
                let map = mapdata::MapData::read(&path).unwrap();
                let world = mapdata::World::build(map, mapdata::CELL, mapdata::Body::default());
                Tactics::new(std::sync::Arc::new(world), 1.0)
            });
            let raw = HumanRoute {
                team: row.team,
                points: row
                    .points
                    .iter()
                    .map(|p| Vec3::new(p[0], 0.0, p[1]))
                    .collect(),
                ..Default::default()
            };
            let total = raw.points.len();
            let route = raw.on_floors(&tactics.world.grid);
            let points: Vec<Vec3> = route
                .points
                .iter()
                .copied()
                .filter(|&p| tactics.fine_goal(p, 0, Some(row.team)))
                .collect();
            let spawns: Vec<Vec3> = tactics
                .world
                .map
                .spawns
                .iter()
                .filter(|s| s.team == row.team)
                .map(|s| s.position)
                .collect();
            let centre =
                spawns.iter().fold(Vec3::ZERO, |a, b| a + *b) * (1.0 / spawns.len() as f32);
            let start = spawns
                .iter()
                .copied()
                .min_by(|a, b| a.distance_xz(centre).total_cmp(&b.distance_xz(centre)))
                .unwrap();
            let (mut no_way, mut jumps, mut length, mut straight) = (0, 0, 0.0f32, 0.0f32);
            let mut from = start;
            for &to in &points {
                straight += from.distance_xz(to);
                match tactics.route(from, to, 0, Some(row.team)) {
                    Some(way) => {
                        jumps += way.points().iter().filter(|w| w.jump).count();
                        length += way.length();
                    }
                    None => no_way += 1,
                }
                from = to;
            }
            out.push_str(&format!(
                "{index}	{}	{}	{}	{total}	{}	{}	{no_way}	{jumps}	{length:.1}	{straight:.1}
",
                row.level,
                row.mode,
                row.team,
                total - points.len(),
                points.len(),
            ));
        }
        match std::env::var_os("SRP_ROUTES_REACH_OUT") {
            Some(file) => std::fs::write(file, out).unwrap(),
            None => eprint!("{out}"),
        }
    }

    /// Each route of the real table against the map's damage zones
    /// (private files), as a match has them (anomalies attached): its
    /// points on a zone's floors, of them those of a zone on all match, the
    /// farthest any of them is from a floor clear of every zone (−1: none
    /// within 6 m), and its legs whose straight line crosses a zone's
    /// floors (all match). With `SRP_ROUTES_ZONES_OUT` a TSV, a row a route
    /// in the file's order; with `SRP_ZONE_FLOORS_DIR` each map's zone
    /// floors (`x,z,y,fixed`, a floor a metre cell and metre of height) for the lives' check. Run in
    /// release.
    #[test]
    #[ignore = "needs the private map files and route table"]
    fn real_routes_and_the_damage_zones() {
        use crate::v026g::bots::anomalies::Anomalies;
        use crate::v026g::bots::tactics::Tactics;
        let (Some(dir), Some(path)) = (
            std::env::var_os("SRP_MAPDATA_DIR"),
            std::env::var_os("SRP_BOT_ROUTES"),
        ) else {
            panic!("set SRP_MAPDATA_DIR and SRP_BOT_ROUTES");
        };
        let floors_dir = std::env::var_os("SRP_ZONE_FLOORS_DIR");
        let table = RouteTable::load(path.as_ref()).unwrap();
        let mut out = String::from(
            "row	level	mode	team	points	in_zone	in_fixed	clear_m	legs	cross	cross_fixed
",
        );
        let mut maps = std::collections::HashMap::new();
        for (index, row) in table.rows.iter().enumerate() {
            let level: u8 = row.level.trim_start_matches("level_").parse().unwrap();
            let tactics = maps.entry((level, row.mode)).or_insert_with(|| {
                let path = std::path::Path::new(&dir)
                    .join(format!("level_{level:02}_day_{}.mapdata", row.mode));
                let map = mapdata::MapData::read(&path).unwrap();
                let world = mapdata::World::build(map, mapdata::CELL, mapdata::Body::default());
                let mut tactics = Tactics::new(std::sync::Arc::new(world), 1.0);
                let anomalies = Anomalies::new(&tactics.world.map, 0x8000_0001);
                tactics.hazards.attach(anomalies);
                if let Some(floors_dir) = &floors_dir {
                    let grid = &tactics.world.grid;
                    let mut seen = std::collections::HashSet::new();
                    let mut csv = String::from(
                        "x,z,y,fixed
",
                    );
                    for node in 0..grid.len() as mapdata::NodeId {
                        if tactics.hazards.zone_of(node).is_none() {
                            continue;
                        }
                        let at = grid.position(node);
                        let fixed = tactics.hazards.fixed_at(node);
                        let cell = (
                            at.x.floor() as i32,
                            at.z.floor() as i32,
                            at.y.round() as i32,
                        );
                        if seen.insert((cell, fixed)) {
                            csv.push_str(&format!(
                                "{},{},{},{}
",
                                cell.0,
                                cell.1,
                                cell.2,
                                u8::from(fixed)
                            ));
                        }
                    }
                    std::fs::write(
                        std::path::Path::new(floors_dir)
                            .join(format!("level_{level:02}_day_{}.csv", row.mode)),
                        csv,
                    )
                    .unwrap();
                }
                tactics
            });
            let grid = &tactics.world.grid;
            let hazards = &tactics.hazards;
            let points = HumanRoute {
                team: row.team,
                points: row
                    .points
                    .iter()
                    .map(|p| Vec3::new(p[0], 0.0, p[1]))
                    .collect(),
                ..Default::default()
            }
            .on_floors(grid)
            .points;
            let (mut in_zone, mut in_fixed, mut clear_m) = (0, 0, 0.0f32);
            for &p in &points {
                let Some(node) = grid.locate(p, 1.0) else {
                    continue;
                };
                if hazards.zone_of(node).is_none() {
                    continue;
                }
                in_zone += 1;
                in_fixed += usize::from(hazards.fixed_at(node));
                let r = 6.0;
                let clear = grid
                    .floors_in([p.x - r, p.z - r, p.x + r, p.z + r])
                    .filter(|&n| {
                        grid.can_stand(n) && tactics.walkable(n) && hazards.zone_of(n).is_none()
                    })
                    .map(|n| grid.position(n))
                    .filter(|at| (at.y - p.y).abs() < 1.5)
                    .map(|at| at.distance_xz(p))
                    .fold(f32::INFINITY, f32::min);
                clear_m = if clear <= r && clear_m >= 0.0 {
                    clear_m.max(clear)
                } else {
                    -1.0
                };
            }
            let (mut cross, mut cross_fixed) = (0, 0);
            for leg in points.windows(2) {
                let (a, b) = (leg[0], leg[1]);
                let steps = (a.distance_xz(b) / 0.5).ceil().max(1.0) as usize;
                let (mut zone, mut fixed) = (false, false);
                for i in 0..=steps {
                    let t = i as f32 / steps as f32;
                    let at = a + (b - a) * t;
                    if let Some(node) = grid.locate(at + Vec3::new(0.0, 0.5, 0.0), 1.0)
                        && hazards.zone_of(node).is_some()
                    {
                        zone = true;
                        fixed |= hazards.fixed_at(node);
                    }
                }
                cross += usize::from(zone);
                cross_fixed += usize::from(fixed);
            }
            out.push_str(&format!(
                "{index}	{}	{}	{}	{}	{in_zone}	{in_fixed}	{clear_m:.1}	{}	{cross}	{cross_fixed}
",
                row.level,
                row.mode,
                row.team,
                points.len(),
                points.len().saturating_sub(1),
            ));
        }
        match std::env::var_os("SRP_ROUTES_ZONES_OUT") {
            Some(file) => std::fs::write(file, out).unwrap(),
            None => eprint!("{out}"),
        }
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
            for mode in 2..=4u8 {
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
