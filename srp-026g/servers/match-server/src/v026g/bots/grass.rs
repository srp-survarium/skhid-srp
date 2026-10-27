//! The grass a human cannot see a crouched body through and the bots' sight does.
//!
//! The map's grass is drawn (the `vegetation` layers, `.veg` files of
//! instance positions) and has no collision object: it is not in the bullet
//! mesh (`mapdata::sight`), which is all the bots' sight and the bullets
//! know. A line through 1.0-1.8 m grass is clear for a bot, and a player
//! cannot switch grass off. The client draws a patch (16 m) only within
//! 100 m of the camera (`grass_world::process_culling`, the constant at
//! 0xa2da30), so beyond [`DRAW_M`] no grass hides anyone.
//!
//! `bot-grass.json`, made from the game's data by
//! `scripts/foliage-volumes/foliage_volumes.py --grass`, lists per level
//! the 1 m cells that hold grass: the ground height, the mean height of
//! the tallest layer in the cell and the number of instances (clumps).
//! [`Grass`] is one level's cells, and answers how much of a line lies
//! hidden in them ([`Grass::metres`]): the metres where the line runs
//! lower than the blades, each counted by the share of the cell the clumps
//! cover. Like the bushes the file is private data read from the mapdata
//! folder (`SRP_BOT_GRASS` names another file; empty or `off`: none);
//! without it no grass hides anyone.

use std::collections::HashMap;
use std::path::Path;

use mapdata::Vec3;
use serde::Deserialize;

pub use super::foliage::Source;

/// The file's name in the mapdata folder (`SRP_MAPDATA_DIR`), read when
/// `SRP_BOT_GRASS` is unset.
pub const DEFAULT_FILE: &str = "bot-grass.json";

/// How far from the eye, metres, the client still draws grass.
pub const DRAW_M: f32 = 100.0;

/// A chest over the feet of a body that stands is [`super::foliage::CHEST_M`];
/// of a crouched one, metres (the aim point's chest, `aimpoint::Part`).
pub const CROUCHED_CHEST_M: f32 = 0.72;

/// How much of a cell's square the clumps in it cover, each: a clump is
/// 0.4-0.9 m wide, so about a third of the square metre. A GUESS: nothing
/// in the game's data says how thick a blade stands.
const COVER_PER_CLUMP: f32 = 0.35;

/// How far apart, metres, a line is sampled: half a cell.
const STEP_M: f32 = 0.5;

/// The file: per level, its grass.
#[derive(Deserialize)]
struct File {
    version: u32,
    levels: HashMap<String, LevelGrass>,
}

#[derive(Deserialize)]
struct LevelGrass {
    /// `[x, z, ground_y, height_cm, clumps]`, x and z the cell's floor indices.
    cells: Vec<(i32, i32, f32, u32, u32)>,
}

/// What one cell holds.
#[derive(Clone, Copy, Debug)]
struct Patch {
    /// The top of the blades, metres (the ground and the height).
    top: f32,
    /// The share of the cell the grass covers, 0..=1.
    cover: f32,
}

/// One level's grass cells ([module](self)).
#[derive(Clone, Debug, Default)]
pub struct Grass {
    cells: HashMap<(i32, i32), Patch>,
}

impl Grass {
    /// No grass.
    pub fn none() -> Self {
        Self::default()
    }

    /// The level of `project` (`level_01/day_2`): every weather of a level
    /// has the same grass.
    fn level_of(project: &str) -> &str {
        project.split('/').next().unwrap_or(project)
    }

    /// `project`'s grass in `text` (the file); a level the file does not
    /// list has none.
    pub fn parse(text: &str, project: &str) -> Result<Self, String> {
        let mut file: File = serde_json::from_str(text).map_err(|error| error.to_string())?;
        if file.version != 1 {
            return Err(format!("version {} (this reads 1)", file.version));
        }
        let cells = file
            .levels
            .remove(Self::level_of(project))
            .map_or_else(Vec::new, |level| level.cells);
        Ok(Self::of(cells))
    }

    /// `project`'s grass in the file at `path`.
    pub fn load(path: &Path, project: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        Self::parse(&text, project).map_err(|error| format!("{}: {error}", path.display()))
    }

    fn of(cells: Vec<(i32, i32, f32, u32, u32)>) -> Self {
        Self {
            cells: cells
                .into_iter()
                .map(|(x, z, ground, height_cm, clumps)| {
                    let patch = Patch {
                        top: ground + height_cm as f32 / 100.0,
                        cover: (clumps as f32 * COVER_PER_CLUMP).min(1.0),
                    };
                    ((x, z), patch)
                })
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// The metres of the line `from`-`to` hidden in grass, the part within
    /// [`DRAW_M`] of `from`: each sample (every [`STEP_M`]) below its cell's
    /// blade tops counts its step times the cell's cover.
    pub fn metres(&self, from: Vec3, to: Vec3) -> f32 {
        if self.cells.is_empty() {
            return 0.0;
        }
        let length = (to - from).length().min(DRAW_M);
        let steps = (length / STEP_M).ceil() as u32;
        if steps == 0 {
            return 0.0;
        }
        let step = length / steps as f32;
        let along = (to - from) * (1.0 / (to - from).length());
        let mut metres = 0.0;
        for k in 0..steps {
            let at = from + along * ((k as f32 + 0.5) * step);
            let cell = (at.x.floor() as i32, at.z.floor() as i32);
            if let Some(patch) = self.cells.get(&cell)
                && at.y < patch.top
            {
                metres += step * patch.cover;
            }
        }
        metres
    }
}

/// The bench's count of the sights that began ([`crate::v026g::bots::think::remember`]) and
/// the metres of grass on their lines.
#[cfg(test)]
pub mod through {
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Sights begun with the setting on, those through half a metre of
    /// grass or more, the grass metres of all, in millimetres, and the
    /// delay the grass added to them, ms.
    static COUNTS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];

    pub fn count(metres: f32, added_ms: u32) {
        COUNTS[0].fetch_add(1, Ordering::Relaxed);
        COUNTS[1].fetch_add(u64::from(metres >= 0.5), Ordering::Relaxed);
        COUNTS[2].fetch_add((metres * 1000.0) as u64, Ordering::Relaxed);
        COUNTS[3].fetch_add(u64::from(added_ms), Ordering::Relaxed);
    }

    pub fn counts() -> [u64; 4] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Four cells in a row along x at z 0 (x 10..14): ground at 1 m, blades
    /// 1.4 m high; the first two thick (3 clumps), the others thin (1).
    const FILE: &str = r#"{"version":1,"levels":{
        "level_01":{"cells":[
            [10,0,1.0,140,3],[11,0,1.0,140,3],[12,0,1.0,140,1],[13,0,1.0,140,1]]},
        "level_03":{"cells":[]}}}"#;

    fn grass() -> Grass {
        Grass::parse(FILE, "level_01/rain_3").unwrap()
    }

    /// A line inside the blades counts the metres of the cells it runs
    /// through, each by its cover; above them, or beside, it counts none.
    #[test]
    fn a_line_in_the_blades_counts_its_covered_metres() {
        let g = grass();
        // x 10 to 12: two full cells (3 clumps cover it all).
        let m = g.metres(Vec3::new(10.0, 1.8, 0.5), Vec3::new(12.0, 1.8, 0.5));
        assert!((m - 2.0).abs() < 1e-4, "{m}");
        // x 12 to 14: two thin cells, 0.35 each.
        let m = g.metres(Vec3::new(12.0, 1.8, 0.5), Vec3::new(14.0, 1.8, 0.5));
        assert!((m - 0.7).abs() < 1e-4, "{m}");
        // Over the blades (2.4 m = 1.0 + 1.4), beside the row, short of it.
        assert_eq!(g.metres(Vec3::new(10.0, 2.5, 0.5), Vec3::new(14.0, 2.5, 0.5)), 0.0);
        assert_eq!(g.metres(Vec3::new(10.0, 1.8, 3.5), Vec3::new(14.0, 1.8, 3.5)), 0.0);
        assert_eq!(g.metres(Vec3::new(0.0, 1.8, 0.5), Vec3::new(9.0, 1.8, 0.5)), 0.0);
    }

    /// A line sinking into the grass counts from where it goes under the tops.
    #[test]
    fn a_sinking_line_counts_from_the_tops() {
        let g = grass();
        // From 3.4 m at x 6 down to 1.4 m at x 14: under 2.4 m from x 10 on.
        let m = g.metres(Vec3::new(6.0, 3.4, 0.5), Vec3::new(14.0, 1.4, 0.5));
        // Under the top from x = 6 + 8 * (1.0 / 2.0) = 10: the four cells.
        assert!((m - (2.0 + 0.7)).abs() < 0.2, "{m}");
    }

    /// Grass 0.9 m high covers a crouched chest (0.72 m over the feet) and not a standing
    /// one (1.26 m): the same eye and feet, the other chest.
    #[test]
    fn low_grass_hides_a_crouched_chest_and_not_a_standing_one() {
        let g = Grass::parse(
            r#"{"version":1,"levels":{"level_01":{"cells":[[10,0,1.0,90,3],[11,0,1.0,90,3]]}}}"#,
            "level_01/day_2",
        )
        .unwrap();
        let eye = Vec3::new(0.0, 2.6, 0.5);
        let feet = Vec3::new(12.0, 1.0, 0.5);
        let crouched = g.metres(eye, feet + Vec3::new(0.0, CROUCHED_CHEST_M, 0.0));
        let standing = g.metres(eye, feet + Vec3::new(0.0, super::super::foliage::CHEST_M, 0.0));
        assert!(crouched > 0.9, "{crouched}");
        assert!(standing < crouched, "{standing} {crouched}");
    }

    /// Nothing farther than the client draws hides anyone.
    #[test]
    fn grass_beyond_the_draw_distance_hides_nobody() {
        let g = grass();
        // The eye 105 m west of the row: the 100 m the client draws end short of it.
        let m = g.metres(Vec3::new(-95.0, 1.8, 0.5), Vec3::new(14.0, 1.8, 0.5));
        assert_eq!(m, 0.0);
        // 100 m east, looking back to the row's far end: the row lies 86-90 m out.
        let m = g.metres(Vec3::new(100.0, 1.8, 0.5), Vec3::new(10.0, 1.8, 0.5));
        assert!(m > 0.0, "{m}");
    }

    /// The file is the mapdata folder's `bot-grass.json` unless `SRP_BOT_GRASS` says otherwise.
    #[test]
    fn the_file_is_the_mapdata_folders_unless_the_variable_says_otherwise() {
        let dir = std::env::temp_dir().join(format!("srp-grass-source-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let default = dir.join(DEFAULT_FILE);
        std::fs::write(&default, FILE).unwrap();
        let choose = |var: Option<&str>| Source::choose_file(var.map(std::ffi::OsStr::new), Some(&dir), DEFAULT_FILE);
        assert_eq!(choose(None), Source::Default(default.clone()));
        assert_eq!(choose(Some("off")), Source::None);
        assert_eq!(choose(Some("/else/grass.json")), Source::Set("/else/grass.json".into()));
        assert_eq!(Grass::load(&default, "level_01/day_2").unwrap().len(), 4);
        std::fs::remove_file(&default).unwrap();
        assert_eq!(choose(None), Source::None);
        std::fs::remove_dir(&dir).unwrap();
    }

    /// Weathers of a level share the grass; a level the file lacks, or no
    /// file, has none; the version is checked.
    #[test]
    fn the_level_is_read_whatever_the_weather() {
        assert_eq!(Grass::parse(FILE, "level_01/day_2").unwrap().len(), 4);
        assert!(Grass::parse(FILE, "level_03/day_2").unwrap().is_empty());
        assert!(Grass::parse(FILE, "level_09/day_2").unwrap().is_empty());
        assert_eq!(Grass::none().metres(Vec3::new(0.0, 1.0, 0.0), Vec3::new(9.0, 1.0, 0.0)), 0.0);
        assert!(Grass::parse(r#"{"version":2,"levels":{}}"#, "level_01/day_2").is_err());
    }
}
