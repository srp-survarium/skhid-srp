//! The sprint gate at the bot's level (`drive::pace_gate`): the ring is looked at along the route the bot keeps, the straight line only without one,
//! and a new life starts with no walk in progress.

use super::*;
use crate::v026g::bots::drive::pace_gate;
use crate::v026g::bots::know::{CheatKind, Memory, Track};
use crate::v026g::bots::pace::{Found, Gate, Walk};
use crate::v026g::bots::path::Waypoint;

fn bot() -> Bots {
    Bots::new(
        0x417,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &[0, 1],
        &[true, true],
        &[Some("flanker"), Some("rusher")],
        Arc::new(Tuning::default()),
        Trace::none(0),
    )
}

fn track(who: usize, x: f32, z: f32, source: Source) -> Track {
    Track { who, at: Vec3::new(x, 0.0, z), error_m: 0.5, time: 1_000, source, sight_since: None, notice_ms: 0, own: None, missed: None, held_until: 0 }
}

fn leg(points: &[(f32, f32)]) -> (Route, Vec3) {
    let route = Route::new(points.iter().map(|&(x, z)| Waypoint { at: Vec3::new(x, 0.0, z), node: 0, jump: None, low: false }).collect());
    let goal = route.end();
    (route, goal)
}

/// A play that may run: calm all the way it runs; a known enemy 28 m from a point 12 m on the route walks it from 12 m back, the radar's track as
/// the plays' rule counted it; with `ring_radar` off the radar's track does not walk it. Without a route the line to the goal is looked along.
#[test]
pub(crate) fn a_cruise_walks_for_a_danger_along_its_route_and_runs_when_the_way_is_calm() {
    let mut bots = bot();
    let bot = bots.bots[0].as_mut().unwrap();
    let feet = Vec3::new(0.0, 0.0, 0.0);
    let intent = Intent::to(Vec3::new(50.0, 0.0, 0.0), Pace::Cruise);
    let tuning = Tuning::default();
    bot.route = Some(leg(&[(0.0, 0.0), (50.0, 0.0)]));
    bot.memory = Memory::default();
    assert_eq!(pace_gate(bot, feet, &intent, &tuning, 1_000), Gate::Run, "no one known");
    // An enemy 65 m off to the east: farther than the ring and the look-ahead (30 + 12) from the feet: the path is calm.
    bot.memory = Memory::with_tracks(vec![track(2, 65.0, 0.0, Source::Seen)]);
    assert_eq!(pace_gate(bot, feet, &intent, &tuning, 1_100), Gate::Run);
    // One at 40 m: the 12 m look-ahead's end is 28 m from it, in the ring.
    bot.memory = Memory::with_tracks(vec![track(2, 40.0, 0.0, Source::Seen)]);
    assert!(matches!(pace_gate(bot, feet, &intent, &tuning, 1_200), Gate::Walk(Walk::Ring { at: Found::Path, .. })));
    // The radar cheat's track, and the strict reading.
    bot.pace.new_life();
    bot.memory = Memory::with_tracks(vec![track(2, 40.0, 0.0, Source::Cheat(CheatKind::Radar))]);
    assert!(matches!(pace_gate(bot, feet, &intent, &tuning, 1_300), Gate::Walk(_)));
    let strict = Tuning { pace: crate::v026g::bots::tuning::PaceTuning { ring_radar: false, ..tuning.pace.clone() }, ..Tuning::default() };
    bot.pace.new_life();
    assert_eq!(pace_gate(bot, feet, &intent, &strict, 1_400), Gate::Run);
    // No route: the line to the goal (50 m east) is looked along, the same.
    bot.pace.new_life();
    bot.route = None;
    bot.memory = Memory::with_tracks(vec![track(2, 40.0, 0.0, Source::Seen)]);
    assert!(matches!(pace_gate(bot, feet, &intent, &tuning, 1_500), Gate::Walk(Walk::Ring { at: Found::Path, .. })));
    // A new life: no walk in progress.
    bot.memory = Memory::default();
    bot.pace.new_life();
    assert_eq!(pace_gate(bot, feet, &intent, &tuning, 1_600), Gate::Run);
}

/// The route's turn: a danger the straight line to the goal passes, which the route (round a corner) never nears, does not walk the bot.
#[test]
pub(crate) fn a_danger_the_line_passes_but_the_route_leaves_does_not_walk_the_bot() {
    let mut bots = bot();
    let bot = bots.bots[0].as_mut().unwrap();
    let feet = Vec3::new(0.0, 0.0, 0.0);
    let tuning = Tuning::default();
    // The route goes 60 m north then 60 m east; the goal is 60 m east, 60 m north. An enemy at (40, 0) is 40 m from the feet.
    bot.route = Some(leg(&[(0.0, 0.0), (0.0, 60.0), (60.0, 60.0)]));
    bot.memory = Memory::with_tracks(vec![track(2, 40.0, 0.0, Source::Seen)]);
    let intent = Intent::to(Vec3::new(60.0, 0.0, 60.0), Pace::Cruise);
    assert_eq!(pace_gate(bot, feet, &intent, &tuning, 1_000), Gate::Run, "the route goes north, away from it");
    bot.route = None;
    bot.pace.new_life();
    let line = Intent::to(Vec3::new(90.0, 0.0, 0.0), Pace::Cruise);
    assert!(matches!(pace_gate(bot, feet, &line, &tuning, 1_100), Gate::Walk(_)), "the line east does go to it");
}
