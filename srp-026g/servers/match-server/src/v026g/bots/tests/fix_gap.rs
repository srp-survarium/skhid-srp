//! The time since the world before, kept with the stuck and off-reckoning notes (`stuck::FIX_GAP_MS`), and the tuned end of the stuck check
//! after one (`UnstickTuning::fix_gap_ms`).

use super::*;

use crate::v026g::bots::route_filter::RouteFilter;
use crate::v026g::bots::tactics::Tactics;

/// A bot with a route whose body the worlds show standing with the walk key held: the notes of fixes at `times` (ms).
fn notes_of(fix_gap_ms: Option<u32>, times: &[u32]) -> Vec<(u32, String)> {
    let world = soak_world();
    let mut tuning = Tuning::default();
    tuning.unstick.fix_gap_ms = fix_gap_ms;
    let teams = [0u8];
    let mut bots = Bots::new(0xF1C9, survarium::v026g::maps::mode::TEAM_DEATHMATCH, 0, &teams, &[true], &[None], Arc::new(tuning), Trace::none(0));
    bots.map_ready(world.clone());
    bots.entered(0, true, 0);
    let at = Vec3::new(14.0, 0.0, 12.0);
    let fix = |time: u32| Fix { time, alive: true, inserted: 7, feet: Some(at), yaw: 0.0, pitch: 0.0, keys: key::FORWARD, speed: Some(0.0) };
    // The first world starts the life; then the bot has a route.
    bots.fixed(0, fix(500), Some(1.0));
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
    let steady = notes_of(None, &[1_000, 1_500, 2_000, 2_500, 3_000, 3_500, 4_000, 4_500]);
    assert_eq!(steady.len(), 1, "{steady:?}");
    assert!(!steady[0].1.contains("fix gap"), "{steady:?}");
    let gapped = [1_000, 1_500, 2_000, 9_000, 9_500, 10_000, 12_500, 13_000, 13_500];
    let plain = notes_of(None, &gapped);
    assert_eq!(plain.first().map(|n| n.0), Some(9_000), "{plain:?}");
    assert!(plain[0].1.contains("[fix gap 7000 ms]"), "{plain:?}");
    let tuned = notes_of(Some(3_000), &gapped);
    assert_eq!(tuned.first().map(|n| n.0), Some(12_500), "{tuned:?}");
    assert!(!tuned[0].1.contains("fix gap"), "{tuned:?}");
    // A gap under the tuned one ends nothing.
    let short = notes_of(Some(10_000), &gapped);
    assert_eq!(short.first().map(|n| n.0), Some(9_000), "{short:?}");
}
