//! The time since the world before, kept with the stuck and off-reckoning notes (`stuck::FIX_GAP_MS`), and the tuned end of the stuck check
//! after one (`UnstickTuning::fix_gap_ms`).

use super::*;

use crate::v026g::bots::route_filter::RouteFilter;
use crate::v026g::bots::tactics::Tactics;

/// A bot with a route whose body the worlds show standing with the walk key held: the notes of fixes at `times` (ms).
fn notes_of(fix_gap_ms: Option<u32>, times: &[u32], beside_m: Option<f32>) -> Vec<(u32, String)> {
    let world = soak_world();
    let mut tuning = Tuning::default();
    tuning.unstick.fix_gap_ms = fix_gap_ms;
    let teams = [0u8, 1];
    let mut bots = Bots::new(0xF1C9, survarium::v026g::maps::mode::TEAM_DEATHMATCH, 0, &teams, &[true, false], &[None, None], Arc::new(tuning), Trace::none(0));
    bots.map_ready(world.clone());
    bots.entered(0, true, 0);
    bots.entered(1, true, 0);
    let at = Vec3::new(14.0, 0.0, 12.0);
    let fix = |time: u32| Fix { time, alive: true, inserted: 7, feet: Some(at), yaw: 0.0, pitch: 0.0, keys: key::FORWARD, speed: Some(0.0) };
    // The first world starts the life; then the bot has a route.
    bots.fixed(0, fix(500), Some(1.0));
    if let Some(d) = beside_m {
        let other = Fix { time: 500, alive: true, inserted: 8, feet: Some(at + Vec3::new(d, 0.0, 0.0)), yaw: 0.0, pitch: 0.0, keys: 0, speed: Some(0.0) };
        bots.fixed(1, other, Some(1.0));
    }
    let tactics = Tactics::new(world.clone(), Tuning::default().hazard.margin_m.get());
    let from = world.grid.locate(at, 1.0).expect("a floor");
    let goal = world.grid.locate(Vec3::new(24.0, 0.0, 30.0), 3.0).expect("a floor");
    let route = tactics.route_nodes(from, goal, 0, Some(0), false, &RouteFilter::open()).expect("a route");
    if let Some(Some(bot)) = bots.bots.get_mut(0) {
        bot.route = Some((route, world.grid.position(goal)));
    }
    let mut out = Vec::new();
    for &time in times {
        for note in bots.fixed(0, fix(time), Some(1.0)) {
            out.push((time, note.text));
        }
    }
    out
}

/// Walk keys held and the body still: stuck after 3 s of worlds. Worlds every half second: the note comes at 3.5 s with no gap word. After a
/// long gap the first world is the one that shows 3 s: the note carries the gap; tuned, the check ends there and the note comes 3 s later.
#[test]
fn a_stuck_note_after_a_gap_in_the_worlds_carries_it_and_a_tuned_gap_ends_the_check() {
    let steady = notes_of(None, &[1_000, 1_500, 2_000, 2_500, 3_000, 3_500, 4_000, 4_500], None);
    assert_eq!(steady.len(), 1, "{steady:?}");
    assert!(!steady[0].1.contains("fix gap"), "{steady:?}");
    let gapped = [1_000, 1_500, 2_000, 9_000, 9_500, 10_000, 12_500, 13_000, 13_500];
    let plain = notes_of(None, &gapped, None);
    assert_eq!(plain.first().map(|n| n.0), Some(9_000), "{plain:?}");
    assert!(plain[0].1.contains("[fix gap 7000 ms]"), "{plain:?}");
    let tuned = notes_of(Some(3_000), &gapped, None);
    assert_eq!(tuned.first().map(|n| n.0), Some(12_500), "{tuned:?}");
    assert!(!tuned[0].1.contains("fix gap"), "{tuned:?}");
    // A gap under the tuned one ends nothing.
    let short = notes_of(Some(10_000), &gapped, None);
    assert_eq!(short.first().map(|n| n.0), Some(9_000), "{short:?}");
}

/// A body within 1.2 m of a stuck one is named in its note, one farther is not.
#[test]
fn a_stuck_note_names_the_body_beside_it() {
    let times = [1_000, 1_500, 2_000, 2_500, 3_000, 3_500, 4_000, 4_500];
    let near = notes_of(None, &times, Some(0.8));
    assert_eq!(near.len(), 1, "{near:?}");
    assert!(near[0].1.contains("[beside place 1, 0.8 m]"), "{near:?}");
    let far = notes_of(None, &times, Some(3.0));
    assert_eq!(far.len(), 1, "{far:?}");
    assert!(!far[0].1.contains("beside"), "{far:?}");
}
