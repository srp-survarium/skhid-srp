//! [A278] The match-wide keys of the tuning: everything `TeamTunings::match_wide` feeds, which the match reads from team 0's
//! tuning for both teams. The bench's one-sided runs (`SRP_BENCH_ONE_SIDE`) refuse two tunings that set one apart
//! (`bench.rs`, `one_sided_refusal`): one side's value would run for both, and the two runs of a head-to-head would not be
//! alike but for what is under test. What reads each key:
//! - `grid` (all of it: the map's grid is built once, the bench's from team 0's body and off-mesh links);
//! - `kin` (all of it: the tracker reckons every body of the shared view with it, `view::Tracker::view` / `fixed`);
//! - `hazard.margin_m`, `hazard.escape`, `hazard.leave_islands`, `hazard.artefact_know` (the tactics, `Bots::map_ready` /
//!   `tactics_ready`), `hazard.live_zones` (the board of live zones), `hazard.zone_pools` (the bench's game);
//! - `aim.combat.clear_step_sees_lip` (the tactics), `aim.combat.live_eye` and `sense.use_ray_m` (view statics),
//!   `aim.combat.fire.weapon_class_by_data` (the stand-in camper's gun);
//! - `sense.sprint_body_mps` (the held sounds);
//! - `traps.react.on`, `traps.react.hear`, `traps.react.snap_near_m` (the traps the tracker reads);
//! - `avoid.bot_pockets`, `avoid.kit_in_use` (the live list's notes; the bench's throws);
//! - `think.rays_per_poll` (a poll's ray budget), `think.search_queue`, `think.search_floors_per_poll` (a poll's route
//!   searches: A278 found team 0's `first_come` serving team 0 first in every head-to-head that gave it the OFF arm).
//!
//! Read from it by the tests' tallies only, as a measure (not refused): `sense.sight_m`, `sense.view_deg`, `cheats.batteries`.
//!
//! The list lives in a file only the tests compile: a change to a file the server compiles, even to its test code, changes
//! the release build (the `.llvm.<hash>` suffixes of its symbols), and this file leaves it byte for byte.

use super::*;

/// The match-wide keys: a dotted path into the tuning's JSON, a section's name standing for every key under it.
const MATCH_WIDE: [&str; 21] = [
    "grid",
    "kin",
    "hazard.margin_m",
    "hazard.escape",
    "hazard.leave_islands",
    "hazard.artefact_know",
    "hazard.live_zones",
    "hazard.zone_pools",
    "aim.combat.clear_step_sees_lip",
    "aim.combat.live_eye",
    "aim.combat.fire.weapon_class_by_data",
    "sense.use_ray_m",
    "sense.sprint_body_mps",
    "traps.react.on",
    "traps.react.hear",
    "traps.react.snap_near_m",
    "avoid.bot_pockets",
    "avoid.kit_in_use",
    "think.rays_per_poll",
    "think.search_queue",
    "think.search_floors_per_poll",
];

/// Keys only the tests' tallies read from the match-wide tuning, as a measure: not refused in a one-sided run.
const MATCH_WIDE_MEASURES: [&str; 3] = ["sense.sight_m", "sense.view_deg", "cheats.batteries"];

/// The match-wide keys `a` and `b` set apart, each with the two values (JSON): what a one-sided run would take from team 0
/// alone.
pub(super) fn match_wide_differences(a: &Tuning, b: &Tuning) -> Vec<(String, String, String)> {
    let (ja, jb) = (serde_json::to_value(a).unwrap_or_default(), serde_json::to_value(b).unwrap_or_default());
    let at = |v: &serde_json::Value, path: &str| path.split('.').try_fold(v.clone(), |v, key| v.get(key).cloned());
    let mut out = Vec::new();
    for path in MATCH_WIDE {
        let (va, vb) = (at(&ja, path), at(&jb, path));
        if va != vb {
            out.extend(leaf_differences(path, va.as_ref(), vb.as_ref()));
        }
    }
    out
}

/// The leaves under `path` where `a` and `b` differ (a section names each of its keys that differs).
fn leaf_differences(path: &str, a: Option<&serde_json::Value>, b: Option<&serde_json::Value>) -> Vec<(String, String, String)> {
    let show = |v: Option<&serde_json::Value>| v.map_or("(none)".to_owned(), |v| v.to_string());
    match (a, b) {
        (Some(serde_json::Value::Object(ma)), Some(serde_json::Value::Object(mb))) => {
            let keys: std::collections::BTreeSet<&String> = ma.keys().chain(mb.keys()).collect();
            keys.into_iter().flat_map(|k| leaf_differences(&format!("{path}.{k}"), ma.get(k), mb.get(k))).collect()
        }
        _ if a == b => Vec::new(),
        _ => vec![(path.to_owned(), show(a), show(b))],
    }
}

/// Every match-wide key names a value of the tuning (a section or a leaf), once.
#[test]
fn every_match_wide_key_names_a_value_of_the_tuning() {
    let json = serde_json::to_value(Tuning::default()).unwrap();
    for path in MATCH_WIDE.iter().chain(&MATCH_WIDE_MEASURES) {
        let found = path.split('.').try_fold(&json, |v, key| v.get(key));
        assert!(found.is_some(), "{path} is no key of the tuning");
    }
    let mut sorted: Vec<&str> = MATCH_WIDE.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), MATCH_WIDE.len(), "no key twice");
}

/// Every read of the match-wide tuning in the source (`match_wide()` / `match_wide_tuning()` followed by a field path) is a
/// listed key, under one, or a section a listed key is in (a binding such as `let avoid = &...avoid`). A new read must be
/// added to [`MATCH_WIDE`] (or [`MATCH_WIDE_MEASURES`] for a tally's measure).
#[test]
fn every_read_of_the_match_wide_tuning_in_the_source_is_listed() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![root.clone()];
    let mut reads = std::collections::BTreeSet::new();
    while let Some(path) = files.pop() {
        if path.is_dir() {
            files.extend(std::fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
            continue;
        }
        // `teams.rs`'s own tests read per-team keys from it to show whose tuning it is.
        if path.extension().is_none_or(|e| e != "rs") || path.ends_with("bots/tuning/teams.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for marker in ["match_wide()", "match_wide_tuning()"] {
            for (at, _) in text.match_indices(marker) {
                let mut rest = text[at + marker.len()..].chars().peekable();
                let mut keys = Vec::new();
                loop {
                    while rest.peek().is_some_and(|c| c.is_whitespace()) {
                        rest.next();
                    }
                    if rest.peek() != Some(&'.') {
                        break;
                    }
                    rest.next();
                    let key: String = std::iter::from_fn(|| rest.next_if(|c| c.is_ascii_alphanumeric() || *c == '_')).collect();
                    if key.is_empty() || rest.peek() == Some(&'(') {
                        break;
                    }
                    keys.push(key);
                }
                if !keys.is_empty() {
                    reads.insert(keys.join("."));
                }
            }
        }
    }
    assert!(reads.contains("think.search_queue"), "the scan finds the reads: {reads:?}");
    let listed: Vec<&str> = MATCH_WIDE.iter().chain(&MATCH_WIDE_MEASURES).copied().collect();
    let unlisted: Vec<&String> = reads
        .iter()
        .filter(|read| {
            !listed.iter().any(|key| {
                read.as_str() == *key || read.starts_with(&format!("{key}.")) || key.starts_with(&format!("{read}."))
            })
        })
        .collect();
    assert!(unlisted.is_empty(), "reads of the match-wide tuning not in MATCH_WIDE: {unlisted:?}");
}

/// The defaults and the all-off tuning set these match-wide keys apart: a head-to-head of the two (the bench's OFF arm on
/// one side) must give its OFF side the defaults' values for them, or the bench refuses it.
#[test]
fn the_defaults_and_all_off_differ_in_these_match_wide_keys() {
    let keys: Vec<String> = match_wide_differences(&Tuning::default(), &Tuning::all_off()).into_iter().map(|d| d.0).collect();
    assert!(keys.contains(&"think.search_queue".to_owned()), "{keys:?}");
    assert_eq!(keys, ALL_OFF_MATCH_WIDE, "update ALL_OFF_MATCH_WIDE (and the bench's OFF-arm notes) with all_off.json");
}

/// The match-wide keys all_off.json sets apart from the defaults (pinned by the test above).
const ALL_OFF_MATCH_WIDE: [&str; 22] = [
    "grid.crouch_floors",
    "grid.drop_room_walk_faces",
    "grid.exact_erosion",
    "grid.floor_paths",
    "grid.gap_links",
    "grid.honest_drops",
    "grid.jump_cap_m",
    "grid.landing_carry",
    "grid.ramp_faces",
    "grid.walk_off_lips",
    "kin.edge",
    "kin.margin_edge",
    "kin.slope_speed",
    "kin.speed_terms",
    "hazard.artefact_know",
    "hazard.live_zones",
    "aim.combat.fire.weapon_class_by_data",
    "traps.react.on",
    "traps.react.hear",
    "avoid.bot_pockets",
    "avoid.kit_in_use",
    "think.search_queue",
];

/// The differences are named leaf by leaf, with both values; equal tunings have none.
#[test]
fn a_match_wide_difference_is_named_with_both_values() {
    assert!(match_wide_differences(&Tuning::default(), &Tuning::default()).is_empty());
    let mut other = Tuning::default();
    other.think.search_queue = tuning::SearchQueue::FirstCome;
    other.think.stamina = !other.think.stamina;
    let diffs = match_wide_differences(&Tuning::default(), &other);
    assert_eq!(diffs, vec![("think.search_queue".to_owned(), "\"fair\"".to_owned(), "\"first_come\"".to_owned())]);
}
