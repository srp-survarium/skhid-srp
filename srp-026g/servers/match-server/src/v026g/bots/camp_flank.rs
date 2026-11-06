//! [A320 `threat.camp_flank`] Flank routes made ahead of time for the camp spots players are known to farm from, per map,
//! mode and the camper's side (`traces-20261028/fx-fights-a320-flank-routes-design.md`).
//!
//! The table is private (the spots and the human wins it is made from come from the players' saved worlds), so it is a
//! file the server reads, never part of the repo, as the human routes are (`routes.rs`): `bot-flank-routes.json` in the
//! mapdata folder (`SRP_MAPDATA_DIR`), or the file `SRP_BOT_FLANK_ROUTES` names (empty or `off`: none). The bench's
//! writer makes it offline from the spots, the worlds' sight trees and the walk grids.
//!
//! A route is kept as feet points 0.5-1 m apart, not as grid floors, so the file outlives a grid rebuild: each point is
//! put on the floor of its cell when the map is ready ([`SpotRow::on_grid`]), and a route whose point finds no floor, or whose two
//! points in a row are not one walk apart (a straight walk, or one link: a drop or a jump the builder kept), is refused
//! and the bots keep their own flank search.
//!
//! The file (format 1):
//!
//! ```json
//! {"format": 1, "spots": [{"level": "level_03", "mode": 2, "camper_side": 0, "at": [51.8, 32.8, 18.1],
//!   "match_radius_m": 5.0, "follow_radius_m": 10.0,
//!   "routes": [{"origin": "human", "points": [[x, y, z], ...], "fire": 41, "exposed_m": 4.5}]}]}
//! ```
//!
//! `camper_side` is the map side (spawn team) the camper plays for: the routes are for the other side's bots. `origin`
//! is `human` (a human's winning path as it was walked), `astar_near_win` (the builder's search, its firing point
//! within 5 m of a human win's) or `astar` (the builder's search at a spot no human won at: unproven). `fire` is the
//! index of the point the flanker stops at with its sights on the spot.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use mapdata::{NodeId, Vec3, WalkGrid};
use serde::Deserialize;
use survarium::v026g::maps::RuleMode;

/// The table's name in the mapdata folder (`SRP_MAPDATA_DIR`), read when `SRP_BOT_FLANK_ROUTES` is unset.
pub const DEFAULT_FILE: &str = "bot-flank-routes.json";

/// The variable that names another table, or `off`.
pub const VARIABLE: &str = "SRP_BOT_FLANK_ROUTES";

/// How far over a route point a floor may be and still be its floor, m (a point is a feet point on the floor).
const SNAP_ABOVE_M: f32 = 0.5;

/// How far below a route point its floor may be, m.
const SNAP_BELOW_M: f32 = 0.5;

/// The longest straight walk between two points of a route in a row, m (the points are 0.5-1 m apart).
const MAX_STEP_M: f32 = 1.5;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    format: u32,
    spots: Vec<SpotFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpotFile {
    level: String,
    mode: u8,
    camper_side: i32,
    at: [f32; 3],
    match_radius_m: f32,
    follow_radius_m: f32,
    routes: Vec<RouteFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteFile {
    origin: Origin,
    points: Vec<[f32; 3]>,
    fire: usize,
    exposed_m: f32,
}

/// Where a route comes from: what backs it.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// A human's winning path round this spot, as it was walked.
    Human,
    /// The builder's search, its firing point within 5 m of a human win's.
    AstarNearWin,
    /// The builder's search at a spot no human won at: unproven.
    Astar,
}

/// Every map's camp spots and their flank routes, as read.
#[derive(Debug, Default)]
pub struct FlankTable {
    spots: Vec<SpotRow>,
}

/// A camp spot of a map and mode, its routes as points (before [`Self::on_grid`]).
#[derive(Clone, Debug, PartialEq)]
pub struct SpotRow {
    level: u8,
    mode: RuleMode,
    /// The map side the camper plays for.
    pub camper_side: i32,
    /// The spot's centre, feet.
    pub at: Vec3,
    /// Two kills from within this of `at` are this spot's (the cue's match, 5 m in the data).
    pub match_radius_m: f32,
    /// A run's kills are followed within this of its moving centre (8-10 m in the data).
    pub follow_radius_m: f32,
    routes: Vec<RouteRow>,
}

#[derive(Clone, Debug, PartialEq)]
struct RouteRow {
    origin: Origin,
    points: Vec<Vec3>,
    fire: usize,
    exposed_m: f32,
}

/// A camp spot on the built map: what the cue matches and the routes round it on the grid.
#[derive(Clone, Debug, PartialEq)]
pub struct CampSpot {
    pub camper_side: i32,
    pub at: Vec3,
    pub match_radius_m: f32,
    pub follow_radius_m: f32,
    pub routes: Vec<FlankRoute>,
}

/// A flank route on the grid: its floors from the start, the one it fires from, and what backs it.
#[derive(Clone, Debug, PartialEq)]
pub struct FlankRoute {
    pub origin: Origin,
    /// One floor a point, from the start; two in a row are one walk apart.
    pub floors: Vec<NodeId>,
    /// The index in `floors` of the floor it fires from (the route's last leg starts there).
    pub fire: usize,
    /// The metres of it the spot's eyes see (crouched through them).
    pub exposed_m: f32,
}

/// How far over or under a held spot a known camp spot may be and still be it, m (a storey).
pub const MATCH_STOREY_M: f32 = 3.0;

/// The known camp spot a team's held spot at `at` is on (A320; the one rule the flank routes and the camper traps, A324,
/// share): a spot of the other map side than `our_side`'s, within its `match_radius_m` and a storey of `at`. Its index in
/// `Tactics::camp_spots` is its id for the match. Whether it has routes is the caller's question.
pub fn camp_spot_at(spots: &[CampSpot], at: Vec3, our_side: i32) -> Option<usize> {
    spots.iter().position(|c| {
        c.camper_side != our_side && c.at.distance_xz(at) <= c.match_radius_m && (c.at.y - at.y).abs() <= MATCH_STOREY_M
    })
}

impl FlankRoute {
    /// The floor it fires from.
    pub fn fire_floor(&self) -> NodeId {
        self.floors[self.fire]
    }
}

/// Why a route was not put on the grid (it is left out; the bots keep their own flank search for that spot).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refused {
    /// The point at this index has no floor within half a cell's diagonal (and [`SNAP_ABOVE_M`] over, [`SNAP_BELOW_M`] under).
    NoFloor(usize),
    /// The points at this index and the next are not one walk apart.
    NoLink(usize),
}

impl FlankTable {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The table, every row checked here: a level named `level_NN`, finite places, positive radii, a route of two or
    /// more points whose `fire` is one of them.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: File = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if file.format != 1 {
            return Err(format!("format {} (1 is known)", file.format));
        }
        let spots = file
            .spots
            .into_iter()
            .enumerate()
            .map(|(i, spot)| SpotRow::parse(spot).map_err(|e| format!("spot {i}: {e}")))
            .collect::<Result<_, _>>()?;
        Ok(Self { spots })
    }

    pub fn len(&self) -> usize {
        self.spots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spots.is_empty()
    }

    /// The spots of a map (`level` 1-8) in a mode, their routes as points until [`SpotRow::on_grid`].
    pub fn for_map(&self, level: u8, mode: RuleMode) -> Vec<SpotRow> {
        self.spots
            .iter()
            .filter(|s| s.level == level && s.mode == mode)
            .cloned()
            .collect()
    }
}

fn point(p: [f32; 3]) -> Result<Vec3, String> {
    let v = Vec3::new(p[0], p[1], p[2]);
    if v.is_finite() { Ok(v) } else { Err(format!("a point not finite: {p:?}")) }
}

impl SpotRow {
    fn parse(spot: SpotFile) -> Result<Self, String> {
        let level = spot
            .level
            .strip_prefix("level_")
            .and_then(|n| n.parse::<u8>().ok())
            .ok_or_else(|| format!("level {:?} (level_NN)", spot.level))?;
        for (name, r) in [("match_radius_m", spot.match_radius_m), ("follow_radius_m", spot.follow_radius_m)] {
            if !(r.is_finite() && r > 0.0) {
                return Err(format!("{name} {r}"));
            }
        }
        let routes = spot
            .routes
            .into_iter()
            .enumerate()
            .map(|(k, r)| {
                let points = r.points.into_iter().map(point).collect::<Result<Vec<_>, _>>()?;
                if points.len() < 2 || r.fire >= points.len() {
                    return Err(format!("route {k}: {} points, fire {}", points.len(), r.fire));
                }
                if !(r.exposed_m.is_finite() && r.exposed_m >= 0.0) {
                    return Err(format!("route {k}: exposed_m {}", r.exposed_m));
                }
                Ok(RouteRow { origin: r.origin, points, fire: r.fire, exposed_m: r.exposed_m })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            level,
            mode: RuleMode::of_config(spot.mode).ok_or_else(|| format!("mode {} (2, 3, 4, ...)", spot.mode))?,
            camper_side: spot.camper_side,
            at: point(spot.at)?,
            match_radius_m: spot.match_radius_m,
            follow_radius_m: spot.follow_radius_m,
            routes,
        })
    }

    /// The spot on the built map: each route's points put on the grid's floors, the routes that do not fit left out
    /// (with why, for the map's report).
    pub fn on_grid(&self, grid: &WalkGrid) -> (CampSpot, Vec<Refused>) {
        let mut refused = Vec::new();
        let routes = self
            .routes
            .iter()
            .filter_map(|r| match r.on_grid(grid) {
                Ok(route) => Some(route),
                Err(why) => {
                    refused.push(why);
                    None
                }
            })
            .collect();
        let spot = CampSpot {
            camper_side: self.camper_side,
            at: self.at,
            match_radius_m: self.match_radius_m,
            follow_radius_m: self.follow_radius_m,
            routes,
        };
        (spot, refused)
    }
}

impl RouteRow {
    fn on_grid(&self, grid: &WalkGrid) -> Result<FlankRoute, Refused> {
        // Half a cell's diagonal: a floor is at its cell's middle.
        let reach = grid.cell_size() * std::f32::consts::FRAC_1_SQRT_2 + 0.01;
        let floors = self
            .points
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                grid.locate_within(p, reach, SNAP_ABOVE_M)
                    .filter(|&n| grid.position(n).y >= p.y - SNAP_BELOW_M)
                    .ok_or(Refused::NoFloor(i))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(i) = floors.windows(2).position(|w| !one_walk(grid, w[0], w[1])) {
            return Err(Refused::NoLink(i));
        }
        Ok(FlankRoute { origin: self.origin, floors, fire: self.fire, exposed_m: self.exposed_m })
    }
}

/// Whether `b` is one walk from `a`: the same floor, a link of `a`'s (a step, a drop, a jump), or a straight walk.
pub(crate) fn one_walk(grid: &WalkGrid, a: NodeId, b: NodeId) -> bool {
    a == b || grid.links(a).any(|(n, _)| n == b) || grid.walks_straight(a, b, MAX_STEP_M, |_| true)
}

/// Where a match's flank table comes from.
#[derive(Debug, PartialEq, Eq)]
pub enum Source {
    /// [`VARIABLE`] names a file.
    Set(PathBuf),
    /// [`VARIABLE`] unset, and the mapdata folder has [`DEFAULT_FILE`].
    Default(PathBuf),
    /// [`VARIABLE`] empty or `off`.
    Off,
    /// [`VARIABLE`] unset, and no [`DEFAULT_FILE`] (or no mapdata folder).
    Missing,
}

impl Source {
    /// `var` is [`VARIABLE`], `mapdata` the mapdata folder.
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
    use crate::v026g::bots::body::tests::walled;

    fn table(routes: &str) -> String {
        format!(
            r#"{{"format": 1, "spots": [{{"level": "level_03", "mode": 2, "camper_side": 0, "at": [30.0, 0.0, 10.0],
                "match_radius_m": 5.0, "follow_radius_m": 10.0, "routes": [{routes}]}}]}}"#
        )
    }

    /// Points `step` m apart along the straight line from `a` to `b` (both included), on the floor.
    fn line(a: (f32, f32), b: (f32, f32), step: f32) -> String {
        let n = ((b.0 - a.0).hypot(b.1 - a.1) / step).ceil() as usize;
        (0..=n)
            .map(|i| {
                let t = i as f32 / n as f32;
                format!("[{:.3}, 0.0, {:.3}]", a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The table is read for its map and mode only, and a bad row stops the read (a route's `fire` off its points, a
    /// format not known, a field not known).
    #[test]
    fn the_table_is_read_by_map_and_mode_and_a_bad_row_stops_it() {
        let ok = table(r#"{"origin": "human", "points": [[1.0, 0.0, 1.0], [1.5, 0.0, 1.0]], "fire": 1, "exposed_m": 0.0}"#);
        let t = FlankTable::parse(&ok).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t.for_map(3, RuleMode::Batteries).len(), 1);
        assert!(t.for_map(4, RuleMode::Batteries).is_empty());
        let bad_fire = table(r#"{"origin": "astar", "points": [[1.0, 0.0, 1.0], [1.5, 0.0, 1.0]], "fire": 2, "exposed_m": 0.0}"#);
        assert!(FlankTable::parse(&bad_fire).is_err());
        assert!(FlankTable::parse(&ok.replace("\"format\": 1", "\"format\": 2")).is_err());
        assert!(FlankTable::parse(&ok.replace("\"exposed_m\"", "\"seen_m\"")).is_err());
        assert!(FlankTable::parse(&ok.replace("\"human\"", "\"guess\"")).is_err());
        assert!(FlankTable::parse(&ok.replace("\"mode\": 2", "\"mode\": 9")).is_err());
    }

    /// A route round the wall's end is put on the grid point by point; one straight through the wall is refused at the
    /// wall, and one off the map at its first point.
    #[test]
    fn a_route_round_the_wall_fits_and_one_through_it_is_refused() {
        let grid = walled();
        let round = format!("{}, {}", line((10.0, 25.0), (10.0, 34.0), 0.75), line((10.75, 34.0), (30.0, 34.0), 0.75));
        let through = line((10.0, 10.0), (30.0, 10.0), 0.75);
        let off = line((-5.0, 10.0), (1.0, 10.0), 0.75);
        let routes = [&round, &through, &off]
            .map(|p| format!(r#"{{"origin": "astar", "points": [{p}], "fire": 1, "exposed_m": 1.0}}"#))
            .join(", ");
        let t = FlankTable::parse(&table(&routes)).unwrap();
        let row = &t.for_map(3, RuleMode::Batteries)[0];
        let (spot, refused) = row.on_grid(&grid);
        assert_eq!(spot.routes.len(), 1, "{refused:?}");
        assert_eq!(spot.routes[0].floors.len(), round.matches('[').count());
        // At the wall: no floor in its eroded cells, or none walked across it.
        assert!(matches!(refused[0], Refused::NoLink(i) | Refused::NoFloor(i) if (12..=14).contains(&i)), "{refused:?}");
        assert_eq!(refused[1], Refused::NoFloor(0));
    }

    /// The table is read from the mapdata folder unless the variable says otherwise: a path, or empty or `off` for none.
    #[test]
    fn the_table_is_the_mapdata_folders_unless_the_variable_says_otherwise() {
        let with = std::env::temp_dir().join(format!("srp-flank-source-{}", std::process::id()));
        let without = with.join("empty");
        std::fs::create_dir_all(&without).unwrap();
        let default = with.join(DEFAULT_FILE);
        std::fs::write(&default, r#"{"format": 1, "spots": []}"#).unwrap();
        assert_eq!(Source::choose(None, Some(&with)), Source::Default(default.clone()));
        assert_eq!(Source::choose(None, Some(&without)), Source::Missing);
        for off in ["", "off"] {
            assert_eq!(Source::choose(Some(OsStr::new(off)), Some(&with)), Source::Off);
        }
        assert!(FlankTable::load(&default).is_ok_and(|t| t.is_empty()));
        std::fs::remove_file(&default).unwrap();
        std::fs::remove_dir(&without).unwrap();
        std::fs::remove_dir(&with).unwrap();
    }
}
