//! [fix-team D5] Why a job's flank and pin spot queries give nothing round real held spots: for each job centre of a
//! file of them (the owner-16 diag bench's `none_sampled` no-spot lines: `mode<TAB>level<TAB>role<TAB>x<TAB>y<TAB>z<TAB>n`),
//! the queries `flank::flank_query` and `crossfire::choke_query` make for a job (eight facings for the flank, a reach of
//! 40, 55 and 70 m for the pin, each map side), run through `Tactics::spots_stages`: every sample's stage, and of the
//! queries that give no spot, the stage that lost the most. Prints the tallies; asserts nothing (`--ignored`):
//!
//! ```text
//! SRP_MAPDATA_DIR=<mapdata folder> SRP_JOB_CENTRES=<centres.tsv> <test binary> job_spot_stages --ignored --nocapture
//! ```

use std::collections::BTreeMap;

use mapdata::{Vec3, World};

use crate::v026g::bots::kin;
use crate::v026g::bots::plays::PlayKind;
use crate::v026g::bots::rng::Rng;
use crate::v026g::bots::tactics::{SpotQuery, Tactics};
use crate::v026g::bots::threat::{self, JOB_LEVELS_M};
use crate::v026g::bots::tuning::Tuning;
use crate::v026g::bots::view::still_eye;

/// The time the hazards are read at, ms (two minutes in).
const TIME_MS: u32 = 120_000;
/// The pin's reaches tried, m (`threat::REACH_FLOOR_M` up).
const REACHES_M: [f32; 3] = [40.0, 55.0, 70.0];
/// The pinner's ring's depth (`crossfire::PIN_DEPTH_M`).
const PIN_DEPTH_M: f32 = 15.0;

#[derive(Default)]
struct Tally {
    queries: u32,
    empty: u32,
    /// Every sample's stage.
    stages: BTreeMap<&'static str, u32>,
    /// Of the empty queries, the stage that lost the most.
    empty_by: BTreeMap<&'static str, u32>,
    /// A probe with no floor at the anchor's height: where the column's nearest floor lies, by band.
    column: BTreeMap<&'static str, u32>,
    /// A floor that does not see him (crouched eye to his feet + 1 m): seen from a standing eye; to his feet + 1.5 m.
    no_sight: u32,
    standing_sees: u32,
    sees_higher: u32,
}

/// The band of a column floor's height over the anchor, m.
fn band(dy: Option<f32>) -> &'static str {
    match dy {
        None => "none",
        Some(d) if d < -8.0 => "under 8+",
        Some(d) if d < -4.0 => "under 4-8",
        Some(d) if d < -1.5 => "under 1.5-4",
        Some(d) if d <= 2.0 => "within",
        Some(d) if d <= 4.0 => "over 2-4",
        Some(d) if d <= 8.0 => "over 4-8",
        Some(_) => "over 8+",
    }
}

impl Tally {
    fn add(&mut self, tactics: &Tactics, query: &SpotQuery, n: usize, seed: u64) {
        self.queries += 1;
        let samples = tactics.spots_samples(query, TIME_MS, &mut Rng::new(seed));
        let stages: Vec<&'static str> = samples.iter().map(|s| s.stage).collect();
        for s in &stages {
            *self.stages.entry(s).or_default() += 1;
        }
        let him = query.sight_to.unwrap_or(query.around);
        for sample in &samples {
            if sample.at.is_none() {
                *self.column.entry(band(sample.column)).or_default() += 1;
            }
            if let (Some(at), "no_sight") = (sample.at, sample.stage) {
                self.no_sight += 1;
                self.standing_sees += u32::from(tactics.sees(still_eye(at, false), him + Vec3::new(0.0, 1.0, 0.0)));
                self.sees_higher += u32::from(tactics.sees(still_eye(at, true), him + Vec3::new(0.0, 1.5, 0.0)));
            }
        }
        if tactics.spots(query, TIME_MS, n, &mut Rng::new(seed)).is_empty() {
            self.empty += 1;
            *self.empty_by.entry(threat::most_failed(&stages)).or_default() += 1;
        }
    }

    fn line(&self, name: &str) -> String {
        let total: u32 = self.stages.values().sum();
        let pct = |n: u32, of: u32| 100.0 * f64::from(n) / f64::from(of.max(1));
        let stages: Vec<String> = self.stages.iter().map(|(k, v)| format!("{k} {:.1} %", pct(*v, total))).collect();
        let empty: Vec<String> = self.empty_by.iter().map(|(k, v)| format!("{k} {v}")).collect();
        let column: Vec<String> = self.column.iter().map(|(k, v)| format!("{k} {v}")).collect();
        format!(
            "{name}: queries {}, empty {} ({:.1} %); empty by its top stage: {}; all samples' stages: {}; no floor at the anchor's height, the column's floor: {}; no_sight {}: a standing eye sees {}, his feet + 1.5 m seen {}",
            self.queries,
            self.empty,
            pct(self.empty, self.queries),
            empty.join(", "),
            stages.join(", "),
            column.join(", "),
            self.no_sight,
            self.standing_sees,
            self.sees_higher
        )
    }
}

fn flank_queries(t: &Tuning, him: Vec3, yaw: f32, side: Option<i32>) -> Vec<SpotQuery> {
    let num = |name| t.number(PlayKind::Flank, name);
    let (lo, hi) = (num("spot_min_m"), num("spot_max_m"));
    let bearing = (num("off_facing_deg") + num("flank_extra_deg")).min(170.0);
    let levels = if t.threat.job_spot_levels { JOB_LEVELS_M } else { 0.0 };
    [1.0_f32, -1.0]
        .iter()
        .map(|sign| {
            let dir = kin::forward(yaw + sign * bearing.to_radians());
            let mid = (lo + hi) * 0.5;
            let centre = Vec3::new(him.x + dir[0] * mid, him.y, him.z + dir[1] * mid);
            SpotQuery {
                sight_to: Some(him),
                up_m: levels,
                down_m: levels,
                column_m: if t.threat.job_spot_column { JOB_LEVELS_M } else { 0.0 },
                side,
                samples: num("samples") as usize / 2,
                ..SpotQuery::ring(centre, 0.0, (hi - lo) * 0.5 + 2.0)
            }
        })
        .collect()
}

fn pin_query(t: &Tuning, him: Vec3, reach_m: f32, side: Option<i32>) -> SpotQuery {
    let num = |name| t.number(PlayKind::Crossfire, name);
    let min_m = num("spot_min_m");
    let max_m = reach_m.max(min_m + 5.0);
    SpotQuery {
        sight_to: Some(him),
        prefer_high: num("prefer_high"),
        up_m: num("up_m"),
        down_m: if t.threat.job_spot_levels { JOB_LEVELS_M } else { 0.0 },
        column_m: if t.threat.job_spot_column { JOB_LEVELS_M } else { 0.0 },
        side,
        samples: num("samples") as usize,
        exposure: t.cover.hold_exposure,
        ..SpotQuery::ring(him, (max_m - PIN_DEPTH_M).max(min_m), max_m)
    }
}

#[test]
#[ignore = "a probe; needs SRP_MAPDATA_DIR and SRP_JOB_CENTRES"]
fn job_spot_stages() {
    let (Some(dir), Some(file)) = (super::real_maps::dir(), std::env::var_os("SRP_JOB_CENTRES")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR and SRP_JOB_CENTRES are needed");
        return;
    };
    let text = std::fs::read_to_string(file).unwrap();
    // (mode, level) -> (role, centre)
    let mut groups: BTreeMap<(String, String), Vec<(String, Vec3)>> = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let v = |i: usize| f[i].trim().parse::<f32>().unwrap();
        groups.entry((f[0].to_owned(), f[1].to_owned())).or_default().push((f[2].to_owned(), Vec3::new(v(3), v(4), v(5))));
    }
    let mut t = Tuning::default();
    // `SRP_JOB_COLUMN=0`: the queries as before `threat.job_spot_column`.
    if std::env::var("SRP_JOB_COLUMN").as_deref() == Ok("0") {
        t.threat.job_spot_column = false;
    }
    // `SRP_JOB_LEVELS=level_01,level_07/4`: only these (a level, or a level and its mode).
    let only: Option<Vec<String>> = std::env::var("SRP_JOB_LEVELS").ok().map(|v| v.split(',').map(str::to_owned).collect());
    let (mut flank_all, mut pin_all) = (Tally::default(), Tally::default());
    for ((mode, level), centres) in groups {
        if only.as_ref().is_some_and(|o| !o.contains(&level) && !o.contains(&format!("{level}/{mode}"))) {
            continue;
        }
        let file = dir.join(mapdata::format::file_name(&format!("{level}/day_{mode}")));
        let Ok(bytes) = std::fs::read(&file) else {
            eprintln!("{level} mode {mode}: no map file {}", file.display());
            continue;
        };
        // As the bench loads its maps: from the grid cache when the run has one (`SRP_BENCH_GRID_CACHE`), else built.
        let map_file = {
            let mut digest = mapdata::digest::Digest::new();
            digest.bytes(&bytes);
            digest.finish()
        };
        let map = mapdata::MapData::from_bytes(&bytes).unwrap();
        let extras = mapdata::GridExtras {
            wall_clearance: t.needs_wall_clearance(),
            open_sides: t.needs_open_sides(),
            pull_hold: t.grid.wall_clear_pull.then_some(crate::v026g::bots::tuning::PULL_HOLD_M),
            pull_walk: t.grid.pull_walk,
            walker_shapes: t.grid.walker_shapes,
        };
        let world: World = super::grid_cache::world(map, map_file, mapdata::CELL, t.grid_body(), t.off_mesh_links(), extras);
        let tactics = Tactics::new(std::sync::Arc::new(world), t.hazard.margin_m.get());
        let mut sides: Vec<i32> = tactics.world.map.spawns.iter().map(|s| s.team).collect();
        sides.sort_unstable();
        sides.dedup();
        let (mut flank, mut pin) = (Tally::default(), Tally::default());
        for (i, (role, him)) in centres.iter().enumerate() {
            for &side in &sides {
                let seed = (i as u64) << 8 ^ side as u64;
                if role == "flanker" {
                    for k in 0..8 {
                        let yaw = std::f32::consts::TAU * k as f32 / 8.0;
                        for (j, q) in flank_queries(&t, *him, yaw, Some(side)).iter().enumerate() {
                            flank.add(&tactics, q, 6, seed ^ (k << 16 | j as u64));
                        }
                    }
                } else {
                    for (k, reach) in REACHES_M.iter().enumerate() {
                        let q = pin_query(&t, *him, *reach, Some(side));
                        pin.add(&tactics, &q, q.samples, seed ^ ((k as u64) << 16));
                    }
                }
            }
        }
        eprintln!("job spots {level} mode {mode}: {}", flank.line("flank"));
        eprintln!("job spots {level} mode {mode}: {}", pin.line("pin"));
        for (all, one) in [(&mut flank_all, flank), (&mut pin_all, pin)] {
            all.queries += one.queries;
            all.empty += one.empty;
            for (k, v) in one.stages {
                *all.stages.entry(k).or_default() += v;
            }
            for (k, v) in one.empty_by {
                *all.empty_by.entry(k).or_default() += v;
            }
            for (k, v) in one.column {
                *all.column.entry(k).or_default() += v;
            }
            all.no_sight += one.no_sight;
            all.standing_sees += one.standing_sees;
            all.sees_higher += one.sees_higher;
        }
    }
    eprintln!("job spots ALL: {}", flank_all.line("flank"));
    eprintln!("job spots ALL: {}", pin_all.line("pin"));
}
