//! Research (mode 5) units against the floors of their maps: where a
//! standing player counts as inside a unit (exe patch 53's test,
//! `research_rule_026g.c` `inside_shape`), on what floors.
//!
//! ```text
//! mapdata-zones --db <resources.db> --patch <resources.db.patch_12> ... [--config <path>]...
//! ```
//!
//! With no `--config`, every `gameplay/game_modes/*.options` of mode 5 the
//! archives give. For each unit it samples its shapes' footprints (widened by
//! the capsule's reach) every `--step` metres, finds every walkable face of
//! the walker mesh in each column (`walk` faces, and `by slope` ones up to
//! 60°), and stands a player there (his transform is his feet, his capsule
//! 1.8 m tall and the config's `capture_reach` round, as patch 53 tests).
//! The heights at which he counts as inside are grouped into floors (faces
//! within `--gap` of one another, so a ramp or stairs are one floor). The
//! lowest floor is the unit's own; any other is printed as `ABOVE`: a report
//! (0.27d's units do the same), a failure (exit 1) with `--strict`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use binary_config_parser::ConfigValue;
use clap::Parser;
use mapdata::{MapData, Materials, Resources, WalkerBehaviour};

#[derive(Parser)]
#[command(about = "Research units against the floors of their maps")]
struct Cli {
    /// The game's resources.db.
    #[arg(long)]
    db: PathBuf,
    /// Patches mounted over it, in order; repeatable.
    #[arg(long = "patch")]
    patches: Vec<PathBuf>,
    /// A mode config (`gameplay/game_modes/level_02_day_5.options`); repeatable.
    #[arg(long = "config")]
    configs: Vec<String>,
    /// The sampling step, metres.
    #[arg(long, default_value_t = 0.2)]
    step: f32,
    /// Faces closer than this in height are one floor, metres.
    #[arg(long, default_value_t = 1.0)]
    gap: f32,
    /// Also print each unit's shapes and its faces' heights by 0.25 m.
    #[arg(long)]
    verbose: bool,
    /// Fail (exit 1) when a unit takes a player on a floor above its own.
    #[arg(long)]
    strict: bool,
}

/// A player's capsule (`grid::Body`, research_rule_026g.c's defaults).
const HALF_HEIGHT: f32 = 0.9;
const MIN_GROUND_NORMAL_Y: f32 = 0.5;

struct Shape {
    kind: i32,
    centre: [f32; 3],
    axes: [[f32; 3]; 3],
    half: [f32; 3],
}

impl Shape {
    /// The squared distance from a point to the shape.
    fn distance2(&self, p: [f32; 3]) -> f32 {
        let d = [
            p[0] - self.centre[0],
            p[1] - self.centre[1],
            p[2] - self.centre[2],
        ];
        let l: Vec<f32> = self
            .axes
            .iter()
            .map(|a| a[0] * d[0] + a[1] * d[1] + a[2] * d[2])
            .collect();
        match self.kind {
            0 => {
                let out =
                    ((l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt() - self.half[0]).max(0.0);
                out * out
            }
            1 => (0..3)
                .map(|i| (l[i].abs() - self.half[i]).max(0.0).powi(2))
                .sum(),
            _ => {
                let radial = ((l[0] * l[0] + l[2] * l[2]).sqrt() - self.half[0]).max(0.0);
                let axial = (l[1].abs() - self.half[1]).max(0.0);
                radial * radial + axial * axial
            }
        }
    }

    /// research_rule_026g.c's inside_shape: a capsule standing on `feet`,
    /// `2 * half_height` tall and `reach` round, overlaps the shape.
    fn inside(&self, feet: [f32; 3], reach: f32, half_height: f32) -> bool {
        let (mut lo, mut hi) = (feet[1] + reach, feet[1] + 2.0 * half_height - reach);
        if hi < lo {
            lo = feet[1] + half_height;
            hi = lo;
        }
        let at = |y: f32| self.distance2([feet[0], y, feet[2]]);
        for _ in 0..40 {
            if hi - lo <= 0.0005 {
                break;
            }
            let (a, b) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
            if at(a) <= at(b) {
                hi = b;
            } else {
                lo = a;
            }
        }
        at((lo + hi) / 2.0) <= reach * reach
    }

    /// The x/z box the shape's points (widened by `reach`) can be in.
    fn footprint(&self, reach: f32) -> [f32; 4] {
        let mut ext = [0.0f32; 3];
        for (j, e) in ext.iter_mut().enumerate() {
            for (i, axis) in self.axes.iter().enumerate() {
                let h = if self.kind == 1 {
                    self.half[i]
                } else {
                    self.half[0].max(self.half[1])
                };
                *e += (axis[j] * h).abs();
            }
        }
        [
            self.centre[0] - ext[0] - reach,
            self.centre[2] - ext[2] - reach,
            self.centre[0] + ext[0] + reach,
            self.centre[2] + ext[2] + reach,
        ]
    }
}

fn float3(value: &ConfigValue, key: &str) -> Result<[f32; 3], Box<dyn std::error::Error>> {
    Ok(value.float3_at(key)?)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(clean) if clean || !cli.strict => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: &Cli) -> Result<bool, Box<dyn std::error::Error>> {
    let mut layers = vec![cli.db.clone()];
    layers.extend(cli.patches.iter().cloned());
    let mut resources = Resources::open_layers(&layers)?;
    let materials = Materials::load(&mut resources)?;
    let mut configs = cli.configs.clone();
    if configs.is_empty() {
        let candidates: Vec<String> = resources
            .paths()
            .filter(|p| p.starts_with("gameplay/game_modes/") && p.ends_with(".options"))
            .map(str::to_owned)
            .collect();
        for path in candidates {
            let root = resources.config(&path)?;
            let mode = root
                .get("mode")
                .and_then(|m| m.get("mode"))
                .and_then(ConfigValue::as_i32);
            if mode == Some(5) {
                configs.push(path);
            }
        }
    }
    configs.sort();
    let mut clean = true;
    for path in &configs {
        let root = resources.config(path)?;
        let mode = root.req("mode")?;
        let project = mode
            .req("match_making_params")?
            .str_at("map_name")?
            .to_owned();
        let parameters = mode.req("parameters")?;
        let reach = parameters
            .get("capture_reach")
            .and_then(ConfigValue::as_f32)
            .unwrap_or(0.4);
        let half_height = parameters
            .get("capture_half_height")
            .and_then(ConfigValue::as_f32)
            .unwrap_or(HALF_HEIGHT);
        let map = MapData::load(&mut resources, &materials, &project)?;
        let walker: Vec<_> = map
            .walker_triangles()
            .into_iter()
            .filter(|t| t.walker != WalkerBehaviour::DontWalk)
            .collect();
        println!("{path} ({project}): {} walker faces", walker.len());
        for zone in parameters.req("capture_zones")?.children() {
            let cp = zone.i32_at("cp_id")?;
            let position = float3(zone, "position")?;
            let mut shapes = Vec::new();
            for s in zone.req("shapes")?.children() {
                shapes.push(Shape {
                    kind: s.i32_at("type")?,
                    centre: float3(s, "position")?,
                    axes: [
                        float3(s, "axis_x")?,
                        float3(s, "axis_y")?,
                        float3(s, "axis_z")?,
                    ],
                    half: float3(s, "half")?,
                });
            }
            // heights (cm) of the faces a standing player counts inside on -> columns
            let mut heights: BTreeMap<i32, usize> = BTreeMap::new();
            let mut every: BTreeMap<i32, usize> = BTreeMap::new();
            for shape in &shapes {
                let [x0, z0, x1, z1] = shape.footprint(reach);
                let near: Vec<_> = walker
                    .iter()
                    .filter(|t| {
                        let (mut a, mut b, mut c, mut d) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                        for v in &t.v {
                            a = a.min(v.x);
                            b = b.min(v.z);
                            c = c.max(v.x);
                            d = d.max(v.z);
                        }
                        c >= x0 && a <= x1 && d >= z0 && b <= z1
                    })
                    .collect();
                let mut x = x0;
                while x <= x1 {
                    let mut z = z0;
                    while z <= z1 {
                        for t in &near {
                            let Some(y) = face_height(t, x, z) else {
                                continue;
                            };
                            if cli.verbose {
                                *every
                                    .entry(((y * 100.0).round() as i32).div_euclid(25))
                                    .or_default() += 1;
                            }
                            let feet = [x, y, z];
                            if shapes.iter().any(|s| s.inside(feet, reach, half_height)) {
                                *heights.entry((y * 100.0).round() as i32).or_default() += 1;
                            }
                        }
                        z += cli.step;
                    }
                    x += cli.step;
                }
            }
            // floors: heights within `gap` of one another
            let mut floors: Vec<(f32, f32, usize)> = Vec::new();
            for (&cm, &n) in &heights {
                let y = cm as f32 / 100.0;
                match floors.last_mut() {
                    Some(last) if y - last.1 <= cli.gap => {
                        last.1 = y;
                        last.2 += n;
                    }
                    _ => floors.push((y, y, n)),
                }
            }
            let letter = (b'A' + cp as u8) as char;
            let tops: Vec<String> = shapes
                .iter()
                .map(|s| format!("{:.2}", s.centre[1] + s.half[1]))
                .collect();
            println!(
                "  {letter} at ({:.2}, {:.2}, {:.2}), shapes' tops {}: {} floors",
                position[0],
                position[1],
                position[2],
                tops.join(" "),
                floors.len()
            );
            for (i, (lo, hi, n)) in floors.iter().enumerate() {
                let what = if i == 0 { "its floor" } else { "ABOVE" };
                println!("    {what}: faces {lo:.2}..{hi:.2} m, {n} samples");
                if i > 0 {
                    clean = false;
                }
            }
            if cli.verbose {
                for s in &shapes {
                    println!(
                        "    shape {} at ({:.2}, {:.2}, {:.2}) half ({:.2}, {:.2}, {:.2})",
                        s.kind,
                        s.centre[0],
                        s.centre[1],
                        s.centre[2],
                        s.half[0],
                        s.half[1],
                        s.half[2]
                    );
                }
                let mut bins: BTreeMap<i32, usize> = BTreeMap::new();
                for (&cm, &n) in &heights {
                    *bins.entry(cm.div_euclid(25)).or_default() += n;
                }
                for (bin, n) in bins {
                    println!("      {:.2}: {n}", bin as f32 * 0.25);
                }
                println!("    every ground face in the footprint, by 0.25 m:");
                for (bin, n) in every.range(-10000..) {
                    println!("      {:.2}: {n}", *bin as f32 * 0.25);
                }
            }
        }
    }
    Ok(clean)
}

/// The height of a face over (x, z), if (x, z) is over it and the face is
/// ground (`walk`, or `by slope` up to 60°).
fn face_height(t: &mapdata::Triangle, x: f32, z: f32) -> Option<f32> {
    let [a, b, c] = t.v;
    let (e1, e2) = (
        (b.x - a.x, b.y - a.y, b.z - a.z),
        (c.x - a.x, c.y - a.y, c.z - a.z),
    );
    let n = (
        e1.1 * e2.2 - e1.2 * e2.1,
        e1.2 * e2.0 - e1.0 * e2.2,
        e1.0 * e2.1 - e1.1 * e2.0,
    );
    let len = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt();
    if len == 0.0 || n.1.abs() < 1e-6 {
        return None;
    }
    if t.walker == WalkerBehaviour::BySlope && n.1.abs() / len < MIN_GROUND_NORMAL_Y {
        return None;
    }
    let det = (b.x - a.x) * (c.z - a.z) - (c.x - a.x) * (b.z - a.z);
    if det.abs() < 1e-9 {
        return None;
    }
    let u = ((x - a.x) * (c.z - a.z) - (c.x - a.x) * (z - a.z)) / det;
    let v = ((b.x - a.x) * (z - a.z) - (x - a.x) * (b.z - a.z)) / det;
    if u < 0.0 || v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(a.y + u * (b.y - a.y) + v * (c.y - a.y))
}
