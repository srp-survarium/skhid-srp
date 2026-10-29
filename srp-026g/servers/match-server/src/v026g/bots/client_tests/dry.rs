//! A dry run of a step in the unit tests: the runner given a made-up match, frame by frame, and the rows it wrote. No server, no
//! client and no owner: the test says what the match looked like at each moment, and the step's own start, measures, bars and end do the
//! rest, so a package can show that a step arms, starts, reads and ends before the owner ever sees it.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::v026g::bots::view::Place;

/// A frame with the owner (place 0, team 0) and no bots, batteries or notes: the starting point of a made-up match.
pub(super) fn frame_at(clock: u32, owner: Body) -> Frame {
    Frame { clock: At(clock), playing: true, owner: Some(Owner { body: owner, pools: None }), bots: Vec::new(), batteries: Vec::new(), heard: Vec::new(), sight: None, exits: Default::default(), counters: Default::default() }
}

pub(super) fn body(place: Place, team: u8, at: mapdata::Vec3) -> Body {
    Body { place, team, alive: true, feet: at, health: 1.0, kills: 0, deaths: 0, crouched: false, keys: 0, yaw: 0.0, pitch: 0.0, speed: 0.0 }
}

/// Runs `step` (alone, in a match of its own group) for `seconds` of match time, a frame every 50 ms, `make(clock_ms)` the match then;
/// the text of the results file and the lines said to the owner.
pub(super) fn run(step: Step, seconds: u32, mut make: impl FnMut(u32) -> Frame) -> (String, Vec<String>) {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir: PathBuf = std::env::temp_dir().join(format!("srp-client-dry-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    step.check().unwrap_or_else(|e| panic!("{}: {e}", step.id));
    let per_team = match step.group.size {
        Size::PerTeam(n) => usize::from(n),
        Size::Any => 2,
    };
    let teams: Vec<u8> = std::iter::repeat_n(0, per_team).chain(std::iter::repeat_n(1, per_team)).collect();
    let played = group_of(step.group.level, step.group.mode, &teams);
    let provenance = Provenance { sha: "dry".into(), tuning: 0, tuning_source: "tuning=defaults".into() };
    let output = Arc::new(Output::new(&dir, "dry").unwrap());
    let mut link = Link::new(Runner::new(1, provenance, played, vec![Todo::new(step)], None), output);
    let mut said = Vec::new();
    for clock in (50..=seconds * 1_000).step_by(50) {
        link.tick(&make(clock));
        said.extend(link.take_says());
    }
    let results = std::fs::read_to_string(link.output().results_path()).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    (results, said)
}
