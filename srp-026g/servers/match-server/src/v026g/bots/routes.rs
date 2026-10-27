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
//!
//! A row may give its points' heights, `"heights": [20.6, null, ...]`, one a
//! point: the feet height (m) the group's players stood at there, `null`
//! where too few of them passed. A point with a height is put on the floor
//! nearest it, so a route over a walkway, a bridge or an upper floor stays
//! on it ([`HumanRoute::on_floors`]); one without keeps the lowest floor.
//! The field is optional, so a server older than it reads such a table as
//! before, without the heights.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use mapdata::{Vec3, WalkGrid};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct File {
    format: u32,
    routes: Vec<Row>,
    /// Where humans stand up high ([`Perch`]); absent in older tables.
    #[serde(default)]
    perches: Vec<PerchRow>,
    #[serde(default)]
    perch_maps: Vec<PerchMapRow>,
    // Anything else (the source's note) is not the bots' business.
}

#[derive(Clone, Debug, Deserialize)]
struct PerchRow {
    level: String,
    x: f32,
    /// The feet height.
    y: f32,
    z: f32,
    /// Visits per 1000 alive player-minutes on the whole map.
    rate: f32,
    /// The median dwell, seconds.
    #[serde(default)]
    dwell: f32,
    /// The visits behind the rate.
    #[serde(default)]
    n: f32,
}

#[derive(Clone, Debug, Deserialize)]
struct PerchMapRow {
    level: String,
    /// The map's feet-height p75, the "high" line.
    p75: f32,
    /// The humans' share of alive time on a perch visit, 0-1, all modes and
    /// by mode group.
    share: f32,
    #[serde(default)]
    share_batteries: Option<f32>,
    #[serde(default)]
    share_tdm: Option<f32>,
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
    /// The players' feet height at each point, `null` where unknown.
    #[serde(default)]
    heights: Option<Vec<Option<f32>>>,
}

/// Every map's human routes, as read.
#[derive(Debug, Default)]
pub struct RouteTable {
    rows: Vec<Row>,
    perches: Vec<PerchRow>,
    perch_maps: Vec<PerchMapRow>,
}

/// A place humans stand up high for a while, from the live matches of 12 or
/// more (`bot-routes.json` `perches`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Perch {
    /// On the floor, feet.
    pub at: Vec3,
    /// Visits per 1000 alive player-minutes.
    pub rate: f32,
    /// The median dwell, seconds.
    pub dwell_s: f32,
    /// The visits behind the rate.
    pub visits: f32,
}

/// What the humans' perches add up to on a map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerchShare {
    /// The map's feet-height p75.
    pub p75: f32,
    /// Their share of alive time on a perch visit for the mode asked.
    pub share: f32,
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
    /// Before [`Self::on_floors`] a point's y is the players' feet height
    /// there, NaN where the table gives none.
    pub points: Vec<Vec3>,
}

/// A point with a height goes on the floor nearest it only when that floor
/// is within this of it; farther, the height is of a floor the grid does
/// not have there (a roof's edge, a prop), and the lowest floor is taken.
const HEIGHT_OFF_M: f32 = 3.0;

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
        if let Some((index, row)) = file.routes.iter().enumerate().find(|(_, row)| {
            row.heights
                .as_ref()
                .is_some_and(|h| h.len() != row.points.len())
        }) {
            return Err(format!(
                "route {index} ({} mode {} team {}): {} heights for {} points",
                row.level,
                row.mode,
                row.team,
                row.heights.as_ref().map_or(0, Vec::len),
                row.points.len()
            ));
        }
        Ok(Self {
            rows: file.routes,
            perches: file.perches,
            perch_maps: file.perch_maps,
        })
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The perches of a map (`level` 1-8), the most visited first.
    pub fn perches_for(&self, level: u8) -> Vec<Perch> {
        let name = format!("level_{level:02}");
        let mut out: Vec<Perch> = self
            .perches
            .iter()
            .filter(|p| p.level == name && p.x.is_finite() && p.y.is_finite() && p.z.is_finite() && p.rate > 0.0)
            .map(|p| Perch {
                at: Vec3::new(p.x, p.y, p.z),
                rate: p.rate,
                dwell_s: p.dwell,
                visits: p.n,
            })
            .collect();
        out.sort_by(|a, b| b.rate.total_cmp(&a.rate));
        out
    }

    /// The humans' perch share of a map (`level` 1-8) for a mode (2 and 3:
    /// batteries; 4: deathmatch), the all-modes share when the table has
    /// none for the group.
    pub fn perch_share(&self, level: u8, mode: u8) -> Option<PerchShare> {
        let name = format!("level_{level:02}");
        let row = self.perch_maps.iter().find(|r| r.level == name)?;
        let by_mode = if mode == 4 { row.share_tdm } else { row.share_batteries };
        Some(PerchShare {
            p75: row.p75,
            share: by_mode.unwrap_or(row.share).clamp(0.0, 1.0),
        })
    }

    /// The routes of a map (`level` 1-8) in a mode, each point's y the
    /// players' feet height (NaN: none given) until
    /// [`HumanRoute::on_floors`].
    pub fn for_map(&self, level: u8, mode: u8) -> Vec<HumanRoute> {
        let name = format!("level_{level:02}");
        self.rows
            .iter()
            .filter(|row| row.level == name && row.mode == mode && row.lives > 0)
            .map(Row::route)
            .collect()
    }
}

impl Row {
    fn route(&self) -> HumanRoute {
        let height = |i: usize| {
            self.heights
                .as_ref()
                .and_then(|h| h.get(i).copied().flatten())
                .filter(|y| y.is_finite())
                .unwrap_or(f32::NAN)
        };
        HumanRoute {
            team: self.team,
            lives: self.lives,
            reward: self.reward.max(0.0),
            fight: self.fight.unwrap_or(self.reward).max(0.0),
            battery: self.battery.unwrap_or(0.0).max(0.0),
            died: self.died.clamp(0.0, 1.0),
            points: self
                .points
                .iter()
                .enumerate()
                .filter(|(_, p)| p[0].is_finite() && p[1].is_finite())
                .map(|(i, p)| Vec3::new(p[0], height(i), p[1]))
                .collect(),
        }
    }
}

impl HumanRoute {
    /// Its points put on the grid's floors, of the floors within 2 m of
    /// each (then 5 m) a body stands on and a way is planned to (none in a
    /// pocket with no way out, [`WalkGrid::in_shut_pocket`]):
    /// - a point with the players' height: the floor nearest that height
    ///   (of those within 0.5 m of the nearest, the nearest in xz), so a
    ///   route over a walkway, a bridge or an upper floor stays on it
    ///   (level_07's middle deck, 20.6 m over the ground under it);
    /// - one without, or whose height no floor within [`HEIGHT_OFF_M`] has:
    ///   the nearest of the lowest (within 1 m of the lowest). A group's
    ///   median path most often runs on the ground, not on the roofs over
    ///   it, nor in a pit under the floor it walks (level_05's, under the
    ///   walkway at (-7.8, -21.3)).
    ///
    /// A point with no floor near (off the walkable map) is dropped.
    pub fn on_floors(mut self, grid: &WalkGrid) -> Self {
        self.points = self
            .points
            .iter()
            .filter_map(|&p| Self::floors_for(grid, p).first().copied())
            .collect();
        self
    }

    /// The floors [`Self::on_floors`] chooses from for the point `p`, nearest `p` (in x, z) first: the floors of the
    /// level its height names (the nearest in height and those within 0.5 m of it), or without a usable height the
    /// lowest ones. Several, since a thin wall puts floors of two sides of it within a metre of the point
    /// (`Tactics::set_routes_with` takes the side the route walks).
    pub fn floors_for(grid: &WalkGrid, p: Vec3) -> Vec<Vec3> {
        let near = |p: Vec3, r: f32| -> Vec<Vec3> {
            grid.floors_in([p.x - r, p.z - r, p.x + r, p.z + r])
                .filter(|&node| grid.can_stand(node) && !grid.in_shut_pocket(node))
                .map(|node| grid.position(node))
                .filter(|at| at.distance_xz(p) <= r)
                .collect()
        };
        let nearest_xz =
            |p: Vec3| move |a: &Vec3, b: &Vec3| a.distance_xz(p).total_cmp(&b.distance_xz(p));
        let by_height = |p: Vec3| {
            [2.0f32, 5.0].iter().find_map(|&r| {
                let floors = near(p, r);
                let off = floors
                    .iter()
                    .map(|at| (at.y - p.y).abs())
                    .fold(f32::INFINITY, f32::min);
                (off <= HEIGHT_OFF_M).then_some(())?;
                let mut level: Vec<Vec3> = floors
                    .into_iter()
                    .filter(|at| (at.y - p.y).abs() <= off + 0.5)
                    .collect();
                level.sort_by(nearest_xz(p));
                (!level.is_empty()).then_some(level)
            })
        };
        let lowest = |p: Vec3| {
            [2.0f32, 5.0].iter().find_map(|&r| {
                let floors = near(p, r);
                let lowest = floors.iter().map(|at| at.y).fold(f32::INFINITY, f32::min);
                let mut level: Vec<Vec3> = floors
                    .into_iter()
                    .filter(|at| at.y <= lowest + 1.0)
                    .collect();
                level.sort_by(nearest_xz(p));
                (!level.is_empty()).then_some(level)
            })
        };
        p.y.is_finite()
            .then(|| by_height(p))
            .flatten()
            .or_else(|| lowest(p))
            .unwrap_or_default()
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
        assert_eq!(
            Source::choose(None, Some(&with)),
            Source::Default(default.clone())
        );
        assert_eq!(Source::choose(None, Some(&without)), Source::Missing);
        assert_eq!(Source::choose(None, None), Source::Missing);
        for off in ["", "off"] {
            assert_eq!(
                Source::choose(Some(OsStr::new(off)), Some(&with)),
                Source::Off
            );
        }
        assert_eq!(
            Source::choose(Some(OsStr::new("/elsewhere/routes.json")), Some(&with)),
            Source::Set(PathBuf::from("/elsewhere/routes.json"))
        );
        assert!(RouteTable::load(&default).is_ok_and(|table| table.is_empty()));
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

    /// Perches are optional: a table without them reads as before; with them
    /// they are read by map, most visited first, and the share by mode group.
    #[test]
    fn perches_are_read_by_map_and_a_table_without_them_reads_as_before() {
        let none = RouteTable::parse(r#"{"format": 2, "routes": []}"#).unwrap();
        assert!(none.perches_for(1).is_empty());
        assert_eq!(none.perch_share(1, 2), None);
        let table = RouteTable::parse(
            r#"{"format": 2, "routes": [], "source": "x",
                "perches": [
                  {"level": "level_01", "x": 1.0, "y": 26.0, "z": 2.0, "rate": 5.0, "dwell": 10.0, "n": 50},
                  {"level": "level_01", "x": 3.0, "y": 26.5, "z": 4.0, "rate": 15.5, "dwell": 5.0, "n": 155},
                  {"level": "level_02", "x": 9.0, "y": 20.0, "z": 9.0, "rate": 7.0, "dwell": 5.0, "n": 70}],
                "perch_maps": [
                  {"level": "level_01", "p75": 22.3, "share": 0.0575, "share_batteries": 0.0521,
                   "share_tdm": 0.0651, "visits": 1462, "minutes": 6701.0},
                  {"level": "level_02", "p75": 19.5, "share": 0.05, "visits": 10, "minutes": 100.0}]}"#,
        )
        .unwrap();
        let one = table.perches_for(1);
        assert_eq!(one.len(), 2);
        assert_eq!(one[0].at, Vec3::new(3.0, 26.5, 4.0));
        assert_eq!(one[0].rate, 15.5);
        assert_eq!(table.perches_for(3).len(), 0);
        assert_eq!(table.perch_share(1, 2).unwrap().share, 0.0521);
        assert_eq!(table.perch_share(1, 3).unwrap().share, 0.0521);
        assert_eq!(table.perch_share(1, 4).unwrap().share, 0.0651);
        assert_eq!(table.perch_share(1, 4).unwrap().p75, 22.3);
        // No mode group in the row: the all-modes share.
        assert_eq!(table.perch_share(2, 4).unwrap().share, 0.05);
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
        let p = routes[0].points[1];
        assert_eq!((p.x, p.z), (4.6, -69.4));
        assert!(p.y.is_nan(), "no height given: {p:?}");
        assert!(table.for_map(3, 2).is_empty());
        assert!(RouteTable::parse(r#"{"format": 3, "routes": []}"#).is_err());
    }

    /// A row's `heights` give its points' y (NaN where `null`); a row
    /// without them reads as before, and one with too few is refused.
    #[test]
    fn heights_give_the_points_y_and_a_wrong_count_is_refused() {
        let row = |heights: &str| {
            format!(
                r#"{{"format": 2, "routes": [{{"level": "level_07", "mode": 2, "team": 0,
                "lives": 12, "reward": 1.0, "points": [[1.0, 2.0], [3.0, 4.0]]{heights}}}]}}"#
            )
        };
        let route = &RouteTable::parse(&row(r#", "heights": [20.6, null]"#))
            .unwrap()
            .for_map(7, 2)[0];
        assert_eq!(route.points[0], Vec3::new(1.0, 20.6, 2.0));
        assert!(route.points[1].y.is_nan());
        let route = &RouteTable::parse(&row("")).unwrap().for_map(7, 2)[0];
        assert!(route.points.iter().all(|p| p.y.is_nan()));
        let why = RouteTable::parse(&row(r#", "heights": [20.6]"#)).unwrap_err();
        assert!(why.contains("1 heights for 2 points"), "{why}");
    }

    /// How walkable each route of the real table is for a bot (private
    /// files): from its side's spawn centre through its points, each leg
    /// planned as a bot of its side plans (off hazards and the other
    /// side's spawn area): the legs with no way, the jumps on the way, the
    /// way's length against the straight one, and the points lost (no
    /// floor near, or not allowed). With `SRP_ROUTES_REACH_OUT` a TSV of
    /// it, a row a route in the file's order. With `SRP_ROUTES_POINTS_OUT`
    /// a TSV a row a point: its height (if given), the floor it gets
    /// without it (`y_lowest`) and with it (`y_3d`; "lost": none), and the
    /// lowest and highest usable floors within 1 m, 2 m and 5 m (a stacked spot
    /// spans more than a storey). Run in release.
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
        let mut points_out = String::from(
            "row\tpoint\tx\tz\theight\ty_lowest\ty_3d\tlo_1m\thi_1m\tlo_2m\thi_2m\tlo_5m\thi_5m\n",
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
            let raw = row.route();
            let total = raw.points.len();
            let grid = &tactics.world.grid;
            for (i, &p) in raw.points.iter().enumerate() {
                let one = |y: f32| {
                    HumanRoute {
                        points: vec![Vec3::new(p.x, y, p.z)],
                        ..Default::default()
                    }
                    .on_floors(grid)
                    .points
                    .first()
                    .map_or(String::from("lost"), |at| format!("{:.2}", at.y))
                };
                let span = |r: f32| {
                    let ys: Vec<f32> = grid
                        .floors_in([p.x - r, p.z - r, p.x + r, p.z + r])
                        .filter(|&node| grid.can_stand(node) && !grid.in_shut_pocket(node))
                        .map(|node| grid.position(node))
                        .filter(|at| at.distance_xz(p) <= r)
                        .map(|at| at.y)
                        .collect();
                    let (lo, hi) = ys
                        .iter()
                        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &y| {
                            (lo.min(y), hi.max(y))
                        });
                    if ys.is_empty() {
                        String::from("\t")
                    } else {
                        format!("{lo:.2}\t{hi:.2}")
                    }
                };
                points_out.push_str(&format!(
                    "{index}\t{i}\t{:.2}\t{:.2}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    p.x,
                    p.z,
                    if p.y.is_finite() {
                        format!("{:.2}", p.y)
                    } else {
                        String::new()
                    },
                    one(f32::NAN),
                    if p.y.is_finite() {
                        one(p.y)
                    } else {
                        one(f32::NAN)
                    },
                    span(1.0),
                    span(2.0),
                    span(5.0),
                ));
            }
            let route = raw.on_floors(grid);
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
                        jumps += way.points().iter().filter(|w| w.jump.is_some()).count();
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
        if let Some(file) = std::env::var_os("SRP_ROUTES_POINTS_OUT") {
            std::fs::write(file, points_out).unwrap();
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
            let points = row.route().on_floors(grid).points;
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

    /// How far a walk over the grid's links from a floor goes to say which floors are on its side, m (x, z).
    const SIDE_M: f32 = 3.0;
    /// The human samples round a point that count for it, m (x, z).
    const SAMPLES_M: f32 = 3.0;
    /// A point with fewer samples than this round it is not judged.
    const MIN_SAMPLES: usize = 10;
    /// A floor needs this many samples on its side to be one the players stood on.
    const MIN_SUPPORT: usize = 3;
    /// The floors a point may move to (the point's level) are within this of it, m; any floor (another level) within
    /// [`WIDE_M`].
    const NEAR_M: f32 = 1.0;
    const WIDE_M: f32 = 2.5;

    /// What the players' samples say of a route point's floor.
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Verdict {
        /// Fewer than [`MIN_SAMPLES`] samples round the point: not judged.
        Thin,
        /// A player stood on the side of the floor the server puts the point on.
        Walked,
        /// No player did; a floor with [`MIN_SUPPORT`] or more samples on its side lies within [`NEAR_M`] (the point's
        /// level) or within [`WIDE_M`] (`wide`: another level): the point goes there.
        Move { to: Vec3, support: usize, wide: bool },
        /// No player did, and no floor within [`WIDE_M`] has any: the point is dropped.
        Drop,
    }

    /// The samples that stand on a floor reached from `at` by a walk over the grid's links within [`SIDE_M`] of it (x, z):
    /// `floors` are the floors the samples stand on.
    fn samples_on_side(grid: &WalkGrid, at: Vec3, floors: &[mapdata::NodeId]) -> usize {
        let Some(start) = grid.locate(at, 0.3) else {
            return 0;
        };
        let mut seen = std::collections::HashSet::from([start]);
        let mut queue = vec![start];
        while let Some(node) = queue.pop() {
            for (to, _) in grid.links(node) {
                if grid.position(to).distance_xz(at) <= SIDE_M && seen.insert(to) {
                    queue.push(to);
                }
            }
        }
        floors.iter().filter(|node| seen.contains(node)).count()
    }

    /// The verdict on the point `p` (its y the players' feet height, NaN where the table gives none) against the feet
    /// positions `samples` of the players of its map and mode, and the number of samples it was judged on.
    fn judge(grid: &WalkGrid, p: Vec3, samples: &[Vec3]) -> Option<(Verdict, usize)> {
        let level: Vec<Vec3> = HumanRoute::floors_for(grid, p)
            .into_iter()
            .filter(|at| at.distance_xz(p) <= NEAR_M)
            .collect();
        let nearest = *level.first()?;
        let near: Vec<Vec3> = samples.iter().copied().filter(|s| s.distance_xz(p) <= SAMPLES_M).collect();
        if near.len() < MIN_SAMPLES {
            return Some((Verdict::Thin, near.len()));
        }
        let floors: Vec<mapdata::NodeId> = near.iter().filter_map(|&s| grid.locate(s, 0.6)).collect();
        if samples_on_side(grid, nearest, &floors) > 0 {
            return Some((Verdict::Walked, near.len()));
        }
        let best = |candidates: Vec<Vec3>| {
            let mut chosen: Option<(usize, Vec3)> = None;
            for at in candidates {
                let support = samples_on_side(grid, at, &floors);
                if support >= MIN_SUPPORT && chosen.is_none_or(|(most, _)| support > most) {
                    chosen = Some((support, at));
                }
            }
            chosen
        };
        if let Some((support, to)) = best(level) {
            return Some((Verdict::Move { to, support, wide: false }, near.len()));
        }
        let mut wide: Vec<Vec3> = grid
            .floors_in([p.x - WIDE_M, p.z - WIDE_M, p.x + WIDE_M, p.z + WIDE_M])
            .filter(|&node| grid.can_stand(node) && !grid.in_shut_pocket(node))
            .map(|node| grid.position(node))
            .filter(|at| at.distance_xz(p) <= WIDE_M)
            .collect();
        wide.sort_by(|a, b| a.distance_xz(p).total_cmp(&b.distance_xz(p)));
        Some(match best(wide) {
            Some((support, to)) => (Verdict::Move { to, support, wide: true }, near.len()),
            None => (Verdict::Drop, near.len()),
        })
    }

    fn feet(points: &[(f32, f32)]) -> Vec<Vec3> {
        points.iter().map(|&(x, z)| Vec3::new(x, 0.0, z)).collect()
    }

    /// A point just past a thin wall (the 40 x 40 m floor with the wall along x = 20), with the players on the near side,
    /// goes to the near side's floor; with them on its own side it stays; with few samples it is not judged; with samples
    /// on neither floor's side it is dropped (Tarakanovsky Fort route 88's point 2 was 0.36 m past a wall).
    #[test]
    fn a_point_past_a_wall_is_judged_by_the_side_the_players_stood_on() {
        let grid = crate::v026g::bots::body::tests::walled();
        let p = Vec3::new(20.15, f32::NAN, 10.0);
        let nearest = HumanRoute::floors_for(&grid, p)[0];
        assert!(nearest.x > 20.0, "{nearest:?}: the nearest floor is the far side's");
        let near_side = feet(&[
            (19.7, 8.5),
            (19.5, 9.0),
            (19.3, 9.5),
            (19.1, 10.0),
            (18.9, 10.5),
            (18.7, 11.0),
            (19.6, 11.5),
            (19.4, 12.0),
            (18.5, 9.0),
            (18.3, 10.0),
            (18.1, 11.0),
            (17.9, 10.0),
        ]);
        let (verdict, n) = judge(&grid, p, &near_side).unwrap();
        assert_eq!(n, 12);
        let Verdict::Move { to, support, wide } = verdict else {
            panic!("{verdict:?}")
        };
        assert!(to.x < 20.0 && support >= MIN_SUPPORT && !wide, "{to:?} {support}");
        let far_side: Vec<Vec3> = near_side.iter().map(|s| Vec3::new(40.0 - s.x, 0.0, s.z)).collect();
        assert_eq!(judge(&grid, p, &far_side).unwrap().0, Verdict::Walked);
        assert_eq!(judge(&grid, p, &near_side[..5]).unwrap(), (Verdict::Thin, 5));
        // Samples round the point that stand on no floor (above the 0.6 m the grid is searched): on neither side.
        let aloft: Vec<Vec3> = near_side.iter().map(|s| Vec3::new(s.x, 9.0, s.z)).collect();
        assert_eq!(judge(&grid, p, &aloft).unwrap().0, Verdict::Drop);
    }

    /// The route points of a table against the players' samples (private files): per point the verdict
    /// ([`Verdict`]). With `SRP_ROUTES_WALKED_OUT` a TSV a row a point (`scripts/bot-routes/walked_floors.py` applies it to
    /// the table); else the counts. `SRP_ROUTES_HUMAN_DIR` is the folder of the samples, `level_XX_day_M.tsv` with the
    /// columns `x`, `feet`, `z`, `jumping` (the routes tool's world positions of the live matches of 12 or more
    /// players). The grid is the default one, with the gap links on `SRP_ROUTES_GAP_LINKS=1` and the clients' numbers on
    /// `SRP_ROUTES_CLIENT_NUMBERS=1`. Run in release.
    #[test]
    #[ignore = "needs the private map files, route table and human samples"]
    fn route_points_stand_where_the_players_stood() {
        let (Some(dir), Some(path), Some(human)) = (
            std::env::var_os("SRP_MAPDATA_DIR"),
            std::env::var_os("SRP_BOT_ROUTES"),
            std::env::var_os("SRP_ROUTES_HUMAN_DIR"),
        ) else {
            panic!("set SRP_MAPDATA_DIR, SRP_BOT_ROUTES and SRP_ROUTES_HUMAN_DIR");
        };
        let on = |name: &str| std::env::var(name).is_ok_and(|v| v == "1");
        let body = if on("SRP_ROUTES_CLIENT_NUMBERS") { mapdata::Body::client() } else { mapdata::Body::default() };
        let links = if on("SRP_ROUTES_GAP_LINKS") { mapdata::OffMeshLinks::Gaps } else { mapdata::OffMeshLinks::None };
        let table = RouteTable::load(path.as_ref()).unwrap();
        let mut out = String::from("row\tpoint\tlevel\tmode\tx\tz\theight\tsamples\tverdict\tto_x\tto_y\tto_z\tsupport\n");
        let mut worlds = std::collections::HashMap::new();
        let mut humans: std::collections::HashMap<(String, u8), Vec<Vec3>> = Default::default();
        let mut counts = std::collections::BTreeMap::<&str, usize>::new();
        for (index, row) in table.rows.iter().enumerate() {
            let level: u8 = row.level.trim_start_matches("level_").parse().unwrap();
            let world = worlds.entry((level, row.mode)).or_insert_with(|| {
                let path = std::path::Path::new(&dir).join(format!("level_{level:02}_day_{}.mapdata", row.mode));
                let map = mapdata::MapData::read(&path).unwrap();
                mapdata::World::build_with_links(map, mapdata::CELL, body, links)
            });
            let samples = humans.entry((row.level.clone(), row.mode)).or_insert_with(|| {
                let file = std::path::Path::new(&human).join(format!("{}_day_{}.tsv", row.level, row.mode));
                let text = std::fs::read_to_string(&file).unwrap_or_default();
                let mut lines = text.lines();
                let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
                let col = |name: &str| head.iter().position(|h| *h == name);
                let (Some(cx), Some(cy), Some(cz), Some(cj)) = (col("x"), col("feet"), col("z"), col("jumping")) else {
                    return Vec::new();
                };
                lines
                    .filter_map(|line| {
                        let f: Vec<&str> = line.split('\t').collect();
                        (f.get(cj) != Some(&"1")).then_some(())?;
                        Some(Vec3::new(f.get(cx)?.parse().ok()?, f.get(cy)?.parse().ok()?, f.get(cz)?.parse().ok()?))
                    })
                    .collect()
            });
            for (k, &p) in row.route().points.iter().enumerate() {
                let Some((verdict, n)) = judge(&world.grid, p, samples) else {
                    continue;
                };
                let (name, to, support) = match verdict {
                    Verdict::Thin => ("thin", None, 0),
                    Verdict::Walked => ("walked", None, 0),
                    Verdict::Move { to, support, wide: false } => ("move", Some(to), support),
                    Verdict::Move { to, support, wide: true } => ("move_wide", Some(to), support),
                    Verdict::Drop => ("drop", None, 0),
                };
                *counts.entry(name).or_default() += 1;
                let at = to.map_or(String::from("\t\t"), |t| format!("{:.2}\t{:.2}\t{:.2}", t.x, t.y, t.z));
                out.push_str(&format!(
                    "{index}\t{k}\t{}\t{}\t{:.2}\t{:.2}\t{}\t{n}\t{name}\t{at}\t{support}\n",
                    row.level,
                    row.mode,
                    p.x,
                    p.z,
                    if p.y.is_finite() { format!("{:.2}", p.y) } else { String::new() }
                ));
            }
        }
        eprintln!("route points by verdict: {counts:?}");
        match std::env::var_os("SRP_ROUTES_WALKED_OUT") {
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
