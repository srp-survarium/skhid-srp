//! The bushes a human cannot see through and the bots' sight does.
//!
//! A bush the map file draws but gives no collision object is not in the
//! bullet mesh (`mapdata::sight`), which is all the bots' sight and the
//! bullets know: a bot sees and shoots through it, a player's eye is
//! stopped by its leaves. The game has no rule for it (`default.player`'s
//! `stealth_params` hold stance and sound factors only).
//!
//! `bot-foliage.json`, made from the game's data by
//! `scripts/foliage-volumes/foliage_volumes.py`, lists per project the box
//! round each such bush (its centre, its half extents along its own axes,
//! its angles as the map file gives them). [`Foliage`] is one project's
//! boxes in a 4 m bucket, and answers how many metres of a line lie inside
//! them ([`Foliage::metres`]). Like the route table the file is private
//! data read from the mapdata folder (`SRP_BOT_FOLIAGE` names another file;
//! empty or `off`: none); without it no bush hides anyone.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use mapdata::{Rotation, Vec3};
use serde::Deserialize;

/// The file's name in the mapdata folder (`SRP_MAPDATA_DIR`), read when
/// `SRP_BOT_FOLIAGE` is unset.
pub const DEFAULT_FILE: &str = "bot-foliage.json";

/// A line to a body ends at his chest, metres over his feet.
pub const CHEST_M: f32 = 1.2;

/// The bucket's cell, metres: a bush's box lies in the cells its bounding
/// circle covers, so a line's cells hold every bush it can cross.
const CELL_M: f32 = 4.0;

/// How far apart, metres, a line is sampled for its cells: a quarter of a
/// cell, so no cell it crosses for more than a metre is missed.
const SAMPLE_M: f32 = 1.0;

/// Where a match's bush boxes come from.
#[derive(Debug, PartialEq, Eq)]
pub enum Source {
    /// `SRP_BOT_FOLIAGE` names a file.
    Set(PathBuf),
    /// `SRP_BOT_FOLIAGE` unset, and the mapdata folder has `bot-foliage.json`.
    Default(PathBuf),
    /// `SRP_BOT_FOLIAGE` empty or `off`, or unset with no file in the mapdata folder.
    None,
}

impl Source {
    /// `var` is `SRP_BOT_FOLIAGE`, `mapdata` the mapdata folder.
    pub fn choose(var: Option<&OsStr>, mapdata: Option<&Path>) -> Self {
        match var {
            Some(var) if var.is_empty() || var == "off" => Self::None,
            Some(var) => Self::Set(PathBuf::from(var)),
            None => match mapdata.map(|dir| dir.join(DEFAULT_FILE)) {
                Some(path) if path.is_file() => Self::Default(path),
                _ => Self::None,
            },
        }
    }
}

/// The file: per project, its bushes.
#[derive(Deserialize)]
struct File {
    version: u32,
    projects: HashMap<String, ProjectBushes>,
}

#[derive(Deserialize)]
struct ProjectBushes {
    bushes: Vec<Bush>,
}

/// One bush as the file gives it.
#[derive(Deserialize)]
struct Bush {
    /// Centre of the box.
    p: [f32; 3],
    /// Half extents along the box's own axes.
    h: [f32; 3],
    /// The placement's angles (`mapdata::Rotation::from_angles`).
    r: [f32; 3],
}

/// A bush's box.
#[derive(Clone, Copy, Debug)]
struct Volume {
    centre: Vec3,
    half: Vec3,
    /// Rows `i`, `j`, `k` of the box's rotation: a map-frame offset `d`
    /// is `(i·d, j·d, k·d)` in the box's own frame.
    axes: Rotation,
}

impl Volume {
    /// The metres of the segment `from`-`to` inside the box (the slab
    /// method in the box's frame).
    fn metres(&self, from: Vec3, to: Vec3) -> f32 {
        let (o, d) = (from - self.centre, to - from);
        let local = |v: Vec3| [self.axes.i.dot(v), self.axes.j.dot(v), self.axes.k.dot(v)];
        let (o, d) = (local(o), local(d));
        let half = [self.half.x, self.half.y, self.half.z];
        let (mut near, mut far) = (0.0f32, 1.0f32);
        for k in 0..3 {
            if d[k].abs() < 1e-9 {
                if o[k].abs() > half[k] {
                    return 0.0;
                }
                continue;
            }
            let (a, b) = ((-half[k] - o[k]) / d[k], (half[k] - o[k]) / d[k]);
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near >= far {
                return 0.0;
            }
        }
        (far - near) * (to - from).length()
    }
}

/// One project's bush boxes ([module](self)).
#[derive(Clone, Debug, Default)]
pub struct Foliage {
    volumes: Vec<Volume>,
    /// Volume indices by cell (x, z).
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl Foliage {
    /// No bushes.
    pub fn none() -> Self {
        Self::default()
    }

    /// `project`'s bushes in `text` (the file); a project the file does not
    /// list has none.
    pub fn parse(text: &str, project: &str) -> Result<Self, String> {
        let mut file: File = serde_json::from_str(text).map_err(|error| error.to_string())?;
        if file.version != 1 {
            return Err(format!("version {} (this reads 1)", file.version));
        }
        let bushes = file
            .projects
            .remove(project)
            .map_or_else(Vec::new, |p| p.bushes);
        Ok(Self::of(bushes))
    }

    /// `project`'s bushes in the file at `path`.
    pub fn load(path: &Path, project: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        Self::parse(&text, project).map_err(|error| format!("{}: {error}", path.display()))
    }

    fn of(bushes: Vec<Bush>) -> Self {
        let mut foliage = Self::default();
        for bush in bushes {
            let (centre, half) = (
                Vec3::new(bush.p[0], bush.p[1], bush.p[2]),
                Vec3::new(bush.h[0], bush.h[1], bush.h[2]),
            );
            let index = foliage.volumes.len() as u32;
            foliage.volumes.push(Volume {
                centre,
                half,
                axes: Rotation::from_angles(Vec3::new(bush.r[0], bush.r[1], bush.r[2])),
            });
            let radius = half.length();
            let cell = |v: f32| (v / CELL_M).floor() as i32;
            for x in cell(centre.x - radius)..=cell(centre.x + radius) {
                for z in cell(centre.z - radius)..=cell(centre.z + radius) {
                    foliage.cells.entry((x, z)).or_default().push(index);
                }
            }
        }
        foliage
    }

    pub fn is_empty(&self) -> bool {
        self.volumes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.volumes.len()
    }

    /// The metres of the line `from`-`to` inside bushes (a point inside two
    /// overlapping bushes counts twice).
    pub fn metres(&self, from: Vec3, to: Vec3) -> f32 {
        if self.volumes.is_empty() {
            return 0.0;
        }
        let length = (to - from).length();
        let steps = (length / SAMPLE_M).ceil().max(1.0) as u32;
        let mut seen: Vec<u32> = Vec::new();
        let mut last = None;
        for step in 0..=steps {
            let at = from + (to - from) * (step as f32 / steps as f32);
            let cell = (
                (at.x / CELL_M).floor() as i32,
                (at.z / CELL_M).floor() as i32,
            );
            if last == Some(cell) {
                continue;
            }
            last = Some(cell);
            if let Some(list) = self.cells.get(&cell) {
                for &index in list {
                    if !seen.contains(&index) {
                        seen.push(index);
                    }
                }
            }
        }
        seen.iter()
            .map(|&index| self.volumes[index as usize].metres(from, to))
            .sum()
    }
}

/// The noticing delay `base_ms` of a sight that begins through `metres` of bush:
/// times `1 + per_m * metres`, at most `cap` times (`SenseTuning::bush_notice`).
pub fn noticing_ms(base_ms: u32, metres: f32, per_m: f32, cap: f32) -> u32 {
    let factor = (1.0 + per_m * metres).clamp(1.0, cap.max(1.0));
    (base_ms as f32 * factor).round() as u32
}

/// The bench's count of the sights that began ([`crate::v026g::bots::think::remember`]) and
/// the metres of bush on their lines.
#[cfg(test)]
pub mod through {
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Sights begun, those through half a metre of bush or more, the
    /// bush metres of all, in millimetres, and the delay the bush added to
    /// them, ms.
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

    const FILE: &str = r#"{"version":1,"projects":{
        "level_01/day_2":{"bushes":[
            {"lib":"a","p":[10.0,1.0,0.0],"h":[1.0,1.0,2.0],"r":[0.0,0.0,0.0]},
            {"lib":"b","p":[20.0,1.0,0.0],"h":[1.0,1.0,2.0],"r":[0.0,1.5707964,0.0]}]},
        "level_03/day_2":{"bushes":[]}}}"#;

    fn foliage() -> Foliage {
        Foliage::parse(FILE, "level_01/day_2").unwrap()
    }

    /// A line through a box is cut by it: its length inside is the box's
    /// extent along the line.
    #[test]
    fn a_line_through_a_bush_has_its_width_in_metres() {
        let f = foliage();
        let m = f.metres(Vec3::new(0.0, 1.0, 0.0), Vec3::new(15.0, 1.0, 0.0));
        assert!((m - 2.0).abs() < 1e-4, "{m}");
        // Over it, beside it, short of it.
        assert_eq!(
            f.metres(Vec3::new(0.0, 3.0, 0.0), Vec3::new(15.0, 3.0, 0.0)),
            0.0
        );
        assert_eq!(
            f.metres(Vec3::new(0.0, 1.0, 3.0), Vec3::new(15.0, 1.0, 3.0)),
            0.0
        );
        assert_eq!(
            f.metres(Vec3::new(0.0, 1.0, 0.0), Vec3::new(5.0, 1.0, 0.0)),
            0.0
        );
        // Ending inside it counts the part inside.
        let m = f.metres(Vec3::new(0.0, 1.0, 0.0), Vec3::new(10.0, 1.0, 0.0));
        assert!((m - 1.0).abs() < 1e-4, "{m}");
    }

    /// The second bush is turned a quarter: its long side (4 m) lies along x
    /// where the first's is along z.
    #[test]
    fn a_turned_bush_is_turned() {
        let f = foliage();
        let across_x = f.metres(Vec3::new(15.0, 1.0, 0.0), Vec3::new(25.0, 1.0, 0.0));
        assert!((across_x - 4.0).abs() < 1e-3, "{across_x}");
        let across_z = f.metres(Vec3::new(20.0, 1.0, -5.0), Vec3::new(20.0, 1.0, 5.0));
        assert!((across_z - 2.0).abs() < 1e-3, "{across_z}");
    }

    /// The bucket loses no bush: against every box tried one by one, on
    /// lines of all directions.
    #[test]
    fn the_bucket_finds_what_a_search_of_all_finds() {
        let f = foliage();
        let mut seed = 12345u32;
        let mut next = move || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        for _ in 0..500 {
            let a = Vec3::new(next() * 40.0 - 5.0, next() * 3.0, next() * 20.0 - 10.0);
            let b = Vec3::new(next() * 40.0 - 5.0, next() * 3.0, next() * 20.0 - 10.0);
            let all: f32 = f.volumes.iter().map(|v| v.metres(a, b)).sum();
            assert!((f.metres(a, b) - all).abs() < 1e-3, "{a:?} {b:?}");
        }
    }

    /// The delay grows with the bush on the line, up to the cap; no bush, or
    /// the setting at 0, leaves it as it was.
    #[test]
    fn the_noticing_delay_grows_with_the_bush_up_to_the_cap() {
        assert_eq!(noticing_ms(300, 0.0, 0.5, 3.0), 300);
        assert_eq!(noticing_ms(300, 2.0, 0.5, 3.0), 600);
        assert_eq!(noticing_ms(300, 10.0, 0.5, 3.0), 900, "capped");
        assert_eq!(noticing_ms(300, 2.0, 0.0, 3.0), 300, "off");
        assert_eq!(
            noticing_ms(300, 2.0, 0.5, 0.5),
            300,
            "a cap under 1 is no shortening"
        );
    }

    /// A sight through a bush is noticed later; one that goes on (the enemy
    /// steps into a bush in sight) is not delayed again, and one that
    /// begins again is asked again.
    #[test]
    fn a_sight_is_delayed_when_it_begins_and_not_again_while_it_goes_on() {
        use crate::v026g::bots::know::{Memory, Percept, Source};
        let f = foliage();
        let seen = |x: f32, time: u32| Percept {
            who: 3,
            at: Vec3::new(x, 0.0, 0.0),
            error_m: 0.0,
            time,
            source: Source::Seen,
            busy: false,
        };
        let eye = Vec3::new(0.0, 1.6, 0.0);
        let notice = |p: &Percept| {
            let metres = f.metres(eye, p.at + Vec3::new(0.0, 1.0, 0.0));
            noticing_ms(300, metres, 0.5, 3.0)
        };
        let mut memory = Memory::default();
        // The bush (x 9-11) between: 2 m of it on the line.
        memory.update(&[seen(15.0, 1_000)], 1_000, notice, 3_000, 20_000);
        assert_eq!(memory.tracks()[0].notice_ms, 600);
        // He steps out of it, the sight going on: the delay stays.
        memory.update(&[seen(5.0, 1_200)], 1_200, notice, 3_000, 20_000);
        assert_eq!(memory.tracks()[0].notice_ms, 600);
        // Out of sight a while, and seen again in the open: a new, shorter one.
        memory.update(&[seen(5.0, 3_000)], 3_000, notice, 3_000, 20_000);
        assert_eq!(memory.tracks()[0].notice_ms, 300);
    }

    /// The cost of a line's metres on a real level's bushes (`SRP_BOT_FOLIAGE`, `MK_PROJECT`,
    /// default Rudnya): lines of 10-60 m at random over the level's bushes. Run with
    /// `cargo test -p match-server --release foliage_cost -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement; needs SRP_BOT_FOLIAGE"]
    fn foliage_cost() {
        let path = std::env::var_os("SRP_BOT_FOLIAGE").expect("SRP_BOT_FOLIAGE");
        let project = std::env::var("MK_PROJECT").unwrap_or("level_01/day_2".into());
        let f = Foliage::load(Path::new(&path), &project).unwrap();
        let (mut lo, mut hi) = (Vec3::new(1e9, 0.0, 1e9), Vec3::new(-1e9, 0.0, -1e9));
        for v in &f.volumes {
            lo = Vec3::new(lo.x.min(v.centre.x), 0.0, lo.z.min(v.centre.z));
            hi = Vec3::new(hi.x.max(v.centre.x), 0.0, hi.z.max(v.centre.z));
        }
        let mut seed = 99u32;
        let mut next = move || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        let n = 200_000;
        let mut total = 0.0f32;
        let started = std::time::Instant::now();
        for _ in 0..n {
            let a = Vec3::new(
                lo.x + next() * (hi.x - lo.x),
                30.0,
                lo.z + next() * (hi.z - lo.z),
            );
            let angle = next() * std::f32::consts::TAU;
            let length = 10.0 + next() * 50.0;
            let b = a + Vec3::new(angle.cos() * length, 0.0, angle.sin() * length);
            total += f.metres(a, b);
        }
        let us = started.elapsed().as_secs_f64() * 1e6 / f64::from(n);
        println!(
            "foliage_cost: {} bushes, {us:.2} us a line (mean {:.3} m of bush; checksum)",
            f.len(),
            total / n as f32
        );
    }

    #[test]
    fn a_project_the_file_does_not_list_or_that_lists_none_has_no_bushes() {
        assert!(Foliage::parse(FILE, "level_09/day_2").unwrap().is_empty());
        let none = Foliage::parse(FILE, "level_03/day_2").unwrap();
        assert!(none.is_empty());
        assert_eq!(none.metres(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)), 0.0);
        assert_eq!(foliage().len(), 2);
    }

    #[test]
    fn a_file_of_another_version_or_not_json_is_refused() {
        assert!(Foliage::parse(r#"{"version":2,"projects":{}}"#, "x").is_err());
        assert!(Foliage::parse("not json", "x").is_err());
    }

    /// The file is read from the mapdata folder unless `SRP_BOT_FOLIAGE`
    /// says otherwise: a path, or empty or `off` for none.
    #[test]
    fn the_file_is_the_mapdata_folders_unless_the_variable_says_otherwise() {
        let with = std::env::temp_dir().join(format!("srp-foliage-source-{}", std::process::id()));
        let without = with.join("empty");
        std::fs::create_dir_all(&without).unwrap();
        let default = with.join(DEFAULT_FILE);
        std::fs::write(&default, FILE).unwrap();
        assert_eq!(
            Source::choose(None, Some(&with)),
            Source::Default(default.clone())
        );
        assert_eq!(Source::choose(None, Some(&without)), Source::None);
        assert_eq!(Source::choose(None, None), Source::None);
        for off in ["", "off"] {
            assert_eq!(
                Source::choose(Some(OsStr::new(off)), Some(&with)),
                Source::None
            );
        }
        assert_eq!(
            Source::choose(Some(OsStr::new("/elsewhere/foliage.json")), Some(&with)),
            Source::Set(PathBuf::from("/elsewhere/foliage.json"))
        );
        assert_eq!(Foliage::load(&default, "level_01/day_2").unwrap().len(), 2);
        std::fs::remove_file(&default).unwrap();
        std::fs::remove_dir(&without).unwrap();
        std::fs::remove_dir(&with).unwrap();
    }
}
