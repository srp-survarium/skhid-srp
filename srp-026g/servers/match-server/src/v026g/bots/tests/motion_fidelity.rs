//! [body physics S0] The motion-fidelity suite's first parts: one model of the client's body for every layer that predicts how a
//! body moves (the builder's flights, the motor's forecast, the reckoning, the bench stand-in) is the plan, and this measures where
//! today's layers stand before any of them moves.
//!
//! - [`no_new_body_physics_copies`] (runs in every test run): the copy guard. Each call of a body-motion entry point (a controller
//!   made or stepped, a flight flown, a fall or a lattice walk that sets a body's feet) is counted per file and held to the counts
//!   of `data/motion_copies.tsv`. A new caller fails the test; a caller taken away only asks for the file to be written again
//!   (`SRP_MOTION_COPIES_WRITE=1`), so the list shrinks as the layers move onto the shared model and a fifth copy cannot start.
//! - [`walk_truth`] (ignored; needs the private traces): the walk vector the reckoning (and the stand-in, which shares it) makes
//!   from the keys, laid beside the walk vector the game handed its controller on the same step (`walk_truth.py`'s tables of
//!   traced steps: each step's recorded inputs and the game's own `.walk`). No world, no grid: this is the keys -> walk model
//!   alone (`kin::Kinematics::target_local`, `kin::Blend::tick_mixed` with the kit's mixer, `kin::SpeedCap::capped`), counted
//!   for each input lag and yaw so the counts say which the game uses.
//!
//! `SRP_MOTION_WALK_TRUTH`: the tables (`walk-truth-<tag>.tsv`, comma-separated paths); `SRP_MOTION_WALK_OUT`: a per-step TSV
//! written for the classifier (optional).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::super::kin::{Blend, Kinematics, Local, Mixer, SpeedCap, to_world};

/// The body-motion entry points the guard counts: each is a way a layer makes, steps or places a body's feet by its own code.
/// The pattern is matched as a call (`pattern(`), outside comments and outside the line that defines it; a pattern whose name other
/// code also uses for something else is counted only under `scope` (a path prefix of the file, from this crate's manifest folder)
/// or written qualified.
const ENTRY_POINTS: &[(&str, &str)] = &[
    // A controller made by a layer (a start state of its own).
    ("standing_at", ""),
    ("deserialized", ""),
    ("kept_controller_at", ""),
    ("kept_controller_saved", ""),
    ("body_at", MAPDATA),
    ("controlled::body_at", ""),
    // A controller stepped by a layer.
    ("kept_tick", ""),
    ("kept_step", ""),
    (".client_step", ""),
    ("client_step", MAPDATA),
    ("walk_step", MAPDATA),
    ("walk_on_at", ""),
    ("step_as", ""),
    // A flight flown.
    ("kept_fly_step", ""),
    ("fly_as", ""),
    ("run_up", MAPDATA),
    ("jump::run_up", ""),
    ("run_up_slow", ""),
    ("run_and_fly", ""),
    ("held_flight", ""),
    ("climb_flight", ""),
    ("neighbour_flights", ""),
    ("neighbour_flights_for", ""),
    // A rest or a fall played by a layer's own rule.
    ("settle", MAPDATA),
    ("rests_at", ""),
    ("client_fall_tick", ""),
    ("fall_step", ""),
    // The lattice setting a body's feet.
    ("slide_supported", ""),
    ("slide_supported_by", ""),
    ("slide_supported_with", ""),
    ("slide_capped", ""),
    ("walk_slide", ""),
    ("kept_climb", ""),
    ("kept_off_ground", ""),
];

/// The `mapdata` crate's sources, from this crate's manifest folder.
const MAPDATA: &str = "../../crates/mapdata/src/";
/// The shared model of the client's body (the `client-body` crate): the one place the entry points are called from once the layers move.
const SHARED_MODEL: &str = "../../crates/client-body/src";

/// The source trees the guard reads, from this crate's manifest folder.
const TREES: &[&str] = &["src", "../../crates/mapdata/src"];

/// The list of known callers, beside this file.
fn copies_file() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/v026g/bots/tests/data/motion_copies.tsv")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Whether `line` calls `name`: `name(` after a character that cannot be part of an identifier, and not the definition itself.
fn calls(line: &str, name: &str) -> usize {
    let code = line.split("//").next().unwrap_or("");
    let bare = name.rsplit(['.', ':']).next().unwrap_or(name);
    if code.contains(&format!("fn {bare}(")) || code.contains(&format!("fn {bare}<")) {
        return 0;
    }
    let pattern = format!("{name}(");
    // A pattern that starts with a name needs no identifier character before it (`unsettle(` is not `settle(`); one that starts
    // with `.` or a path is whole already.
    let whole = name.starts_with(|c: char| c.is_alphanumeric() || c == '_');
    code.match_indices(&pattern)
        .filter(|&(at, _)| {
            !whole || code[..at].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
        })
        .count()
}

/// Every (file, entry point) with its call count, the file relative to the crate's manifest folder, `/` separated.
fn count_calls() -> BTreeMap<(String, String), usize> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for tree in TREES {
        rust_files(&root.join(tree), &mut files);
    }
    let mut counts = BTreeMap::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let rel = file.strip_prefix(root).unwrap_or(&file).to_string_lossy().replace('\\', "/");
        // This file names the entry points; it calls none of them. The shared model (the `client-body` crate) is where they belong.
        if rel.ends_with("bots/tests/motion_fidelity.rs") || rel.starts_with(SHARED_MODEL) {
            continue;
        }
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            for &(name, scope) in ENTRY_POINTS {
                if !rel.starts_with(scope) {
                    continue;
                }
                let n = calls(line, name);
                if n > 0 {
                    *counts.entry((rel.clone(), (*name).to_owned())).or_insert(0) += n;
                }
            }
        }
    }
    counts
}

fn read_copies(path: &Path) -> BTreeMap<(String, String), usize> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let mut w = l.split('\t');
            Some(((w.next()?.to_owned(), w.next()?.to_owned()), w.next()?.trim().parse().ok()?))
        })
        .collect()
}

fn write_copies(path: &Path, counts: &BTreeMap<(String, String), usize>) {
    let mut text = String::from(
        "# [body physics] The known callers of the body-motion entry points (motion_fidelity.rs, ENTRY_POINTS): file, entry point, \
         calls.\n# Written by `SRP_MOTION_COPIES_WRITE=1 cargo test -p match-server no_new_body_physics_copies`; it may only shrink.\n",
    );
    for ((file, name), n) in counts {
        text.push_str(&format!("{file}\t{name}\t{n}\n"));
    }
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

/// The copy guard: no file calls a body-motion entry point more often than the known list allows. A count that fell is
/// reported (write the list again); a count that rose, or a new (file, entry point), fails.
#[test]
fn no_new_body_physics_copies() {
    let counts = count_calls();
    let path = copies_file();
    if std::env::var_os("SRP_MOTION_COPIES_WRITE").is_some() {
        write_copies(&path, &counts);
        eprintln!("motion copies: wrote {} rows to {}", counts.len(), path.display());
        return;
    }
    let known = read_copies(&path);
    assert!(!known.is_empty(), "{}: no known callers (write it with SRP_MOTION_COPIES_WRITE=1)", path.display());
    let mut new = Vec::new();
    let mut fewer = Vec::new();
    for (key, &n) in &counts {
        match known.get(key) {
            Some(&k) if n <= k => {
                if n < k {
                    fewer.push(format!("{} {}: {k} -> {n}", key.0, key.1));
                }
            }
            Some(&k) => new.push(format!("{} {}: {k} -> {n}", key.0, key.1)),
            None => new.push(format!("{} {}: new, {n}", key.0, key.1)),
        }
    }
    for (key, &k) in &known {
        if !counts.contains_key(key) {
            fewer.push(format!("{} {}: {k} -> 0", key.0, key.1));
        }
    }
    if !fewer.is_empty() {
        eprintln!("motion copies: fewer callers than the list (write it again with SRP_MOTION_COPIES_WRITE=1):\n  {}", fewer.join("\n  "));
    }
    assert!(
        new.is_empty(),
        "new body-motion callers outside the shared model (a body moved by a layer's own code; use the shared model, see \
         motion_fidelity.rs):\n  {}",
        new.join("\n  ")
    );
}

#[test]
fn the_copy_guard_counts_calls_not_definitions_or_comments() {
    assert_eq!(calls("    let c = Controller::standing_at(feet);", "standing_at"), 1);
    assert_eq!(calls("pub fn standing_at(feet: Vec3) -> Self {", "standing_at"), 0);
    assert_eq!(calls("    x.kept_tick(a); // kept_tick(b)", "kept_tick"), 1);
    assert_eq!(calls("    not_settle(a); settle(b); unsettle(c)", "settle"), 1);
    assert_eq!(calls("    walk_step_m(a)", "walk_step"), 0);
    // A qualified pattern counts only its own path; a bare name counts it too (a bare name of another meaning is scoped instead).
    assert_eq!(calls("    let r = jump::run_up(w, s, p);", "run_up"), 1);
    assert_eq!(calls("    let r = traps::run_up(w, s, p);", "jump::run_up"), 0);
    assert_eq!(calls("    let r = jump::run_up(w, s, p);", "jump::run_up"), 1);
    assert_eq!(calls("    grid.client_step(feet, by)", ".client_step"), 1);
    assert_eq!(calls("    standin::client_step(grid)", ".client_step"), 0);
}

// ---------------------------------------------------------------------------------------------------------------------------
// walk truth

/// One traced step of one place (`walk_truth.py`'s row), what the walk model needs.
#[derive(Clone, Copy, Debug)]
struct TruthStep {
    t: u32,
    place: u32,
    /// The game's walk vector, flat (m over the step), map frame.
    walk: [f32; 2],
    /// The pre-step basis's yaw.
    yaw: f32,
    /// The inputs at t-150, t-100, t-50, t (none where the place had none).
    act: [Option<u32>; 4],
    jumping: bool,
    capsule_crouch: bool,
    sprinting: bool,
}

fn hex_or_none(s: &str) -> Option<u32> {
    let s = s.trim();
    (!s.is_empty()).then(|| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok()).flatten()
}

fn read_truth(path: &str) -> Vec<TruthStep> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().expect("a header").split('\t').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("{path}: no column {name}"));
    let (ct, cp, cx, cz, cy) = (col("t"), col("place"), col("wx_map"), col("wz_map"), col("yaw"));
    let acts = [col("act_m150"), col("act_m100"), col("act_m50"), col("act_0")];
    let (cj, cc, cs) = (col("jumping"), col("f_capsule_crouch"), col("f_sprinting"));
    lines
        .filter_map(|line| {
            let w: Vec<&str> = line.split('\t').collect();
            let f = |i: usize| w.get(i).and_then(|v| v.parse::<f32>().ok());
            Some(TruthStep {
                t: w.get(ct)?.parse().ok()?,
                place: w.get(cp)?.parse().ok()?,
                walk: [f(cx)?, f(cz)?],
                yaw: f(cy)?,
                act: acts.map(|i| w.get(i).and_then(|v| hex_or_none(v))),
                jumping: w.get(cj) == Some(&"1"),
                capsule_crouch: w.get(cc) == Some(&"1"),
                sprinting: w.get(cs) == Some(&"1"),
            })
        })
        .collect()
}

/// Which input a step's walk is made from, and which yaw turns it (each counted; the game's is the one that matches).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Variant {
    /// The keys of the input this many 50 ms before the step's end (0..=3: t, t-50, t-100, t-150).
    keys_back: usize,
    /// The yaw of the step itself (false) or of the step before (true).
    yaw_before: bool,
}

const VARIANTS: [Variant; 6] = [
    Variant { keys_back: 0, yaw_before: false },
    Variant { keys_back: 1, yaw_before: false },
    Variant { keys_back: 2, yaw_before: false },
    Variant { keys_back: 0, yaw_before: true },
    Variant { keys_back: 1, yaw_before: true },
    Variant { keys_back: 2, yaw_before: true },
];

/// The steps of a chain are warmed up this many before they count (the blend's layers are not known at a chain's start).
const WARM_STEPS: usize = 8;
/// After a jump the landing clips' root motion is not the walk model's: this many steps after the last airborne one are left out.
const AFTER_JUMP_STEPS: usize = 20;

/// The movement keys of an input, the ones the walk model reads.
const MOVE_MASK: u32 = 0x1 | 0x2 | 0x4 | 0x8 | 0x80 | 0x100 | 0x200 | 0x20;

/// How a step's keys stood: held unchanged over the last 6 inputs (the blend's whole ramp), or changing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    Still,
    Steady,
    Changing,
}

#[derive(Default)]
struct Tally {
    n: usize,
    within_1e4: usize,
    within_1e3: usize,
    within_1e2: usize,
    sum: f64,
    worst: f32,
}

impl Tally {
    fn add(&mut self, gap: f32) {
        self.n += 1;
        self.within_1e4 += usize::from(gap <= 1e-4);
        self.within_1e3 += usize::from(gap <= 1e-3);
        self.within_1e2 += usize::from(gap <= 1e-2);
        self.sum += f64::from(gap);
        self.worst = self.worst.max(gap);
    }

    fn line(&self) -> String {
        let pct = |k: usize| if self.n == 0 { 0.0 } else { 100.0 * k as f64 / self.n as f64 };
        format!(
            "n {} | <=0.1 mm {:.1} % | <=1 mm {:.1} % | <=1 cm {:.1} % | mean {:.2} mm | worst {:.1} mm",
            self.n,
            pct(self.within_1e4),
            pct(self.within_1e3),
            pct(self.within_1e2),
            if self.n == 0 { 0.0 } else { 1000.0 * self.sum / self.n as f64 },
            1000.0 * self.worst
        )
    }
}

/// The walk (m over a 50 ms step, map frame) the reckoning's model makes for a body at `yaw`: the keys' target, blended by the
/// kit's mixer, capped by the sprint state of the input one ahead of the keys (`cap_keys`).
fn model_step(kin: &Kinematics, blend: &mut Blend, keys: u32, cap_keys: u32, level: f32, yaw: f32) -> [f32; 2] {
    let target = kin.target_local(keys, level);
    let local: Local = blend.tick_mixed(target, kin.blend_ticks, kin.mixer);
    let local = SpeedCap::Client.capped(local, cap_keys, level);
    let v = to_world(local, yaw);
    [v[0] * 0.05, v[1] * 0.05]
}

/// Each place's speed level, measured: the median over its steady steps (forward-only keys held 6 inputs, not jumping) of the
/// game's walk over the level-1 target's. A measurement of the trace (the kit and the weapon in hands are not in it), not a rule.
fn measured_levels(kin: &Kinematics, chains: &BTreeMap<u32, Vec<TruthStep>>) -> BTreeMap<u32, f32> {
    let mut out = BTreeMap::new();
    for (&place, steps) in chains {
        let mut ratios: Vec<f32> = Vec::new();
        for (i, s) in steps.iter().enumerate() {
            if i < 6 || s.jumping {
                continue;
            }
            let Some(keys) = s.act[2] else { continue };
            if keys & MOVE_MASK != 0x1 {
                continue;
            }
            let held = (1..=5).all(|b| steps[i - b].act[2].map(|k| k & MOVE_MASK) == Some(0x1) && steps[i - b].t + 50 * b as u32 == s.t);
            if !held {
                continue;
            }
            let nominal = kin.target_local(keys, 1.0)[0] * 0.05;
            let game = s.walk[0].hypot(s.walk[1]);
            if nominal > 0.0 && game > 0.0 {
                ratios.push(game / nominal);
            }
        }
        if ratios.len() >= 5 {
            ratios.sort_by(f32::total_cmp);
            out.insert(place, ratios[ratios.len() / 2]);
        }
    }
    out
}

#[test]
#[ignore = "body physics S0: needs the private walk-truth tables (SRP_MOTION_WALK_TRUTH)"]
fn walk_truth() {
    let Some(paths) = std::env::var("SRP_MOTION_WALK_TRUTH").ok().filter(|v| !v.is_empty()) else {
        eprintln!("skipping: SRP_MOTION_WALK_TRUTH is not set");
        return;
    };
    let kin = Kinematics::default();
    assert_eq!(kin.mixer, Mixer::Layers, "the reckoning's mixer");
    let mut out = std::env::var("SRP_MOTION_WALK_OUT").ok().filter(|v| !v.is_empty()).map(|p| {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&p).unwrap_or_else(|e| panic!("{p}: {e}")));
        use std::io::Write as _;
        writeln!(f, "table\tt\tplace\tclass\tkeys\tlevel\tgame_x\tgame_z\tmodel_x\tmodel_z\tgap_mm\tsprinting\tcrouched").unwrap();
        f
    });
    for path in paths.split(',') {
        let rows = read_truth(path);
        let mut chains: BTreeMap<u32, Vec<TruthStep>> = BTreeMap::new();
        for r in rows {
            chains.entry(r.place).or_default().push(r);
        }
        for steps in chains.values_mut() {
            steps.sort_by_key(|s| s.t);
        }
        let levels = measured_levels(&kin, &chains);
        let mut tallies: BTreeMap<(usize, Class), Tally> = BTreeMap::new();
        let mut skipped = BTreeMap::<&str, usize>::new();
        for (vi, v) in VARIANTS.iter().enumerate() {
            for (&place, steps) in &chains {
                let Some(&level) = levels.get(&place) else {
                    *skipped.entry("place with no steady walk (no level)").or_default() += steps.len();
                    continue;
                };
                let mut blend = Blend::default();
                let mut run = 0usize;
                let mut since_jump = usize::MAX;
                let mut prev: Option<TruthStep> = None;
                for (at, s) in steps.iter().enumerate() {
                    if prev.is_none_or(|p| p.t + 50 != s.t) {
                        blend = Blend::at(game_local(s.walk, s.yaw));
                        run = 0;
                    }
                    let yaw = if v.yaw_before { prev.map_or(s.yaw, |p| p.yaw) } else { s.yaw };
                    prev = Some(*s);
                    since_jump = if s.jumping { 0 } else { since_jump.saturating_add(1) };
                    let keys = s.act[3 - v.keys_back];
                    let cap_keys = s.act[(3 - v.keys_back + 1).min(3)];
                    let (Some(keys), Some(cap_keys)) = (keys, cap_keys) else {
                        if vi == 0 {
                            *skipped.entry("no input at the lag").or_default() += 1;
                        }
                        blend = Blend::at(game_local(s.walk, s.yaw));
                        run = 0;
                        continue;
                    };
                    let model = model_step(&kin, &mut blend, keys, cap_keys, level, yaw);
                    run += 1;
                    if run <= WARM_STEPS {
                        continue;
                    }
                    if since_jump <= AFTER_JUMP_STEPS {
                        if vi == 0 {
                            *skipped.entry("in or after a jump").or_default() += 1;
                        }
                        continue;
                    }
                    let gap = (model[0] - s.walk[0]).hypot(model[1] - s.walk[1]);
                    let class = class_of(steps, at, v.keys_back);
                    tallies.entry((vi, class)).or_default().add(gap);
                    if vi == 1
                        && let Some(f) = out.as_mut()
                    {
                        use std::io::Write as _;
                        writeln!(
                            f,
                            "{}\t{}\t{}\t{:?}\t{:#x}\t{:.4}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.3}\t{}\t{}",
                            Path::new(path).file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned()),
                            s.t,
                            place,
                            class,
                            keys,
                            level,
                            s.walk[0],
                            s.walk[1],
                            model[0],
                            model[1],
                            gap * 1000.0,
                            u8::from(s.sprinting),
                            u8::from(s.capsule_crouch)
                        )
                        .unwrap();
                    }
                }
            }
        }
        // [S1] The shared model's walk (the `client-body` crate: the same layers and the sprint start's group rate blend), at the game's
        // input and yaw (keys t-50, the yaw before), against the reckoning's at the same: every step, and the steps within six
        // after a forward-to-sprint change.
        let mut shared: BTreeMap<(&str, Class), Tally> = BTreeMap::new();
        for (&place, steps) in &chains {
            let Some(&level) = levels.get(&place) else { continue };
            let mut kin_blend = Blend::default();
            let mut blend = client_body::WalkBlend::still();
            let mut run = 0usize;
            let mut since_jump = usize::MAX;
            let mut since_sprint = usize::MAX;
            let mut prev: Option<TruthStep> = None;
            for (at, s) in steps.iter().enumerate() {
                if prev.is_none_or(|p| p.t + 50 != s.t) {
                    let local = game_local(s.walk, s.yaw);
                    kin_blend = Blend::at(local);
                    blend = client_body::WalkBlend::at(local, client_body::Clip::OnSite);
                    run = 0;
                }
                let yaw = prev.map_or(s.yaw, |p| p.yaw);
                let last_keys = prev.and_then(|p| p.act[2]);
                prev = Some(*s);
                since_jump = if s.jumping { 0 } else { since_jump.saturating_add(1) };
                let (Some(keys), Some(cap_keys)) = (s.act[2], s.act[3]) else {
                    kin_blend = Blend::at(game_local(s.walk, s.yaw));
                    blend = client_body::WalkBlend::at(game_local(s.walk, s.yaw), client_body::Clip::OnSite);
                    run = 0;
                    continue;
                };
                since_sprint = if last_keys.is_some_and(|k| !client_body::sprints(k)) && client_body::sprints(keys) {
                    0
                } else {
                    since_sprint.saturating_add(1)
                };
                let kin_model = model_step(&kin, &mut kin_blend, keys, cap_keys, level, yaw);
                let local = blend.tick(client_body::target_local(keys, level), client_body::clip_of(keys));
                let v = client_body::to_world(client_body::capped(local, cap_keys, level), yaw);
                let model = [v[0] * 0.05, v[1] * 0.05];
                run += 1;
                if run <= WARM_STEPS || since_jump <= AFTER_JUMP_STEPS {
                    continue;
                }
                let class = class_of(steps, at, 1);
                for (who, m) in [("reckoning", kin_model), ("shared", model)] {
                    let gap = (m[0] - s.walk[0]).hypot(m[1] - s.walk[1]);
                    shared.entry((who, class)).or_default().add(gap);
                    if since_sprint < 6 {
                        shared.entry((if who == "shared" { "shared, sprint start" } else { "reckoning, sprint start" }, class)).or_default().add(gap);
                    }
                }
            }
        }
        for ((who, class), t) in &shared {
            eprintln!("MOTION walk-truth model {who} {class:?}: {}", t.line());
        }
        eprintln!("MOTION walk-truth {path}: places {} with a level {}; skipped {skipped:?}", chains.len(), levels.len());
        for (vi, v) in VARIANTS.iter().enumerate() {
            for class in [Class::Still, Class::Steady, Class::Changing] {
                if let Some(t) = tallies.get(&(vi, class)) {
                    eprintln!(
                        "MOTION walk-truth keys t-{} ms yaw {} {:?}: {}",
                        50 * v.keys_back,
                        if v.yaw_before { "before" } else { "own" },
                        class,
                        t.line()
                    );
                }
            }
        }
    }
}

/// The game's walk (m over the step, map frame) as a body-frame velocity at `yaw` (for a chain's first blend).
fn game_local(walk: [f32; 2], yaw: f32) -> Local {
    let f = super::super::kin::forward(yaw);
    let l = [-f[1], f[0]];
    let (vx, vz) = (walk[0] / 0.05, walk[1] / 0.05);
    [vx * f[0] + vz * f[1], vx * l[0] + vz * l[1]]
}

/// A step's class by its keys at the variant's lag: none held (still), the same movement keys over the last 6 inputs (steady),
/// else changing.
fn class_of(steps: &[TruthStep], at: usize, keys_back: usize) -> Class {
    let lag = 3 - keys_back;
    let s = &steps[at];
    let keys = s.act[lag].unwrap_or(0) & MOVE_MASK;
    let same = at >= 5 && (1..=5).all(|b| steps[at - b].t + 50 * b as u32 == s.t && steps[at - b].act[lag].map(|k| k & MOVE_MASK) == Some(keys));
    match (keys & 0xF == 0, same) {
        (true, true) => Class::Still,
        (_, true) => Class::Steady,
        _ => Class::Changing,
    }
}
