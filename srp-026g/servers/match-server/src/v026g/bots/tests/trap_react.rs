use super::*;

use crate::v026g::bots::trap_react::{
    Class, Look, React, TrapKey, Verdict, armed_of, chance, segment_closest,
};
use crate::v026g::bots::tuning::ReactTuning;
use survarium::v026g::match_msgs::traps::{PlacedTrap, TrapState};

fn t() -> ReactTuning {
    ReactTuning {
        on: true,
        ..ReactTuning::default()
    }
}

fn trap(x: f32, z: f32) -> PlacedTrap {
    PlacedTrap {
        state: TrapState::Armed,
        position: [x, 0.0, z],
        up: [0.0, 1.0, 0.0],
        placed_ms: 1_000,
    }
}

fn look(now: u32) -> Look {
    Look {
        now,
        feet: Vec3::new(0.0, 0.0, 0.0),
        eye: Vec3::new(0.0, 1.6, 0.0),
        yaw: 0.0,
        fighting: false,
    }
}

/// The yaw whose forward points along +z or +x is the stand-in's; the tests
/// read it from the view function, not by hand.
fn facing(from: Vec3, to: Vec3) -> f32 {
    crate::v026g::bots::kin::yaw_of([to.x - from.x, to.z - from.z])
}

#[test]
pub(crate) fn the_chance_is_p0_near_falls_to_nothing_far_and_shrinks_in_a_fight() {
    let t = t();
    assert_eq!(chance(1.0, false, &t), t.p0);
    assert_eq!(chance(t.near_m, false, &t), t.p0);
    let mid = (t.near_m + t.far_m) / 2.0;
    assert!((chance(mid, false, &t) - t.p0 / 2.0).abs() < 1e-6);
    assert_eq!(chance(t.far_m, false, &t), 0.0);
    assert_eq!(chance(t.far_m + 5.0, false, &t), 0.0);
    assert!((chance(1.0, true, &t) - t.p0 / t.fight_div).abs() < 1e-6);
    // Defaults: off, and the numbers are the guesses of the design.
    let d = ReactTuning::default();
    assert!(!d.on);
    assert_eq!((d.p0, d.near_m, d.far_m, d.roll_s), (0.05, 3.0, 8.0, 20.0));
}

#[test]
pub(crate) fn a_roll_needs_range_the_view_and_a_clear_line_and_is_made_once_a_while() {
    let t = t();
    let ahead = trap(0.0, 4.0);
    let mut look = look(10_000);
    look.yaw = facing(look.feet, Vec3::new(0.0, 0.0, 4.0));
    let mut react = React::default();
    // Out of range: none, and no ray is asked for.
    let far = trap(0.0, t.far_m + 1.0);
    assert_eq!(
        react.roll(&look, &far, &t, || 0.0, |_, _| panic!("no ray")),
        None
    );
    // Behind the bot: none.
    let mut away = look;
    away.yaw += std::f32::consts::PI;
    assert_eq!(
        react.roll(&away, &ahead, &t, || 0.0, |_, _| panic!("no ray")),
        None
    );
    // Hidden by a wall: none.
    assert_eq!(react.roll(&look, &ahead, &t, || 0.0, |_, _| false), None);
    assert_eq!(react.counts().rolls, 0);
    // In range, in front, clear: a roll. A draw above the chance misses.
    let missed = react.roll(&look, &ahead, &t, || 0.99, |_, _| true);
    assert_eq!(missed, Some(Verdict::Missed));
    assert_eq!((react.counts().rolls, react.counts().missed), (1, 1));
    // The same trap is not rolled again for `roll_s`.
    let key = TrapKey::of(&ahead);
    let mut later = look;
    later.now += 15_000;
    assert_eq!(
        react.roll(&later, &ahead, &t, || 0.0, |_, _| panic!("no ray")),
        Some(Verdict::Missed)
    );
    assert_eq!(react.counts().rolls, 1);
    // After it, a new roll; a low draw notices, and a noticed trap stays so.
    later.now += 6_000;
    let seen = react.roll(&later, &ahead, &t, || 0.0, |_, _| true);
    assert_eq!(seen, Some(Verdict::Noticed { at: later.now }));
    assert!(react.noticed(key, later.now + 300_000));
    assert_eq!((react.counts().rolls, react.counts().noticed), (2, 1));
    // A new life forgets it.
    react.new_life();
    assert_eq!(react.verdict(key, later.now), None);
}

#[test]
pub(crate) fn a_fight_divides_the_chance() {
    let t = t();
    let ahead = trap(0.0, 2.0);
    let mut look = look(10_000);
    look.yaw = facing(look.feet, Vec3::new(0.0, 0.0, 2.0));
    // A draw just under p0: noticed at peace, missed in a fight.
    let draw = t.p0 * 0.9;
    let mut calm = React::default();
    assert_eq!(
        calm.roll(&look, &ahead, &t, || draw, |_, _| true),
        Some(Verdict::Noticed { at: 10_000 })
    );
    let mut fight = React::default();
    look.fighting = true;
    assert_eq!(
        fight.roll(&look, &ahead, &t, || draw, |_, _| true),
        Some(Verdict::Missed)
    );
}

#[test]
pub(crate) fn a_pass_is_counted_when_the_bot_leaves_the_trap_by_what_it_had_rolled() {
    let at = Vec3::new(5.0, 0.0, 0.0);
    let trap = trap(at.x, at.z);
    let key = TrapKey::of(&trap);
    let armed = [(key, at)];
    // Through: the segments pass over it.
    let mut react = React::default();
    for (now, x) in [
        (0u32, 0.0f32),
        (500, 3.0),
        (1000, 5.1),
        (1500, 8.0),
        (2000, 12.0),
    ] {
        react.walk(now, Vec3::new(x, 0.0, 0.1), &armed);
    }
    let c = react.counts();
    assert_eq!((c.passes_all(), c.through_all()), (1, 1));
    assert_eq!(c.passes[Class::Unrolled as usize], 1);
    // Beside: 0.8 m off the whole way.
    let mut react = React::default();
    for (now, x) in [
        (0u32, 0.0f32),
        (500, 3.0),
        (1000, 5.0),
        (1500, 8.0),
        (2000, 12.0),
    ] {
        react.walk(now, Vec3::new(x, 0.0, 0.8), &armed);
    }
    let c = react.counts();
    assert_eq!((c.passes_all(), c.through_all()), (1, 0));
    // Wide of 1.5 m: no pass at all.
    let mut react = React::default();
    for (now, x) in [(0u32, 0.0f32), (500, 5.0), (1000, 10.0), (1500, 15.0)] {
        react.walk(now, Vec3::new(x, 0.0, 2.5), &armed);
    }
    assert_eq!(react.counts().passes_all(), 0);
    // A trap that goes (fired) closes the pass at once.
    let mut react = React::default();
    for (now, x) in [(0u32, 3.0f32), (500, 4.5)] {
        react.walk(now, Vec3::new(x, 0.0, 0.0), &armed);
    }
    react.prune(&[]);
    assert_eq!(react.counts().passes_all(), 1);
}

#[test]
pub(crate) fn a_pass_is_split_by_noticed_missed_and_never_rolled() {
    let t = t();
    let at = Vec3::new(0.0, 0.0, 3.0);
    let placed = trap(at.x, at.z);
    let key = TrapKey::of(&placed);
    let armed = [(key, at)];
    let mut look = look(0);
    look.yaw = facing(look.feet, at);
    for (draw, class) in [(0.0, Class::Noticed), (0.99, Class::Missed)] {
        let mut react = React::default();
        react.roll(&look, &placed, &t, || draw, |_, _| true);
        for (now, z) in [(100u32, -1.0f32), (600, 3.0), (1100, 7.0), (1600, 12.0)] {
            react.walk(now, Vec3::new(0.0, 0.0, z), &armed);
        }
        react.prune(&[]);
        let c = react.counts();
        assert_eq!(c.passes[class as usize], 1, "{class:?}");
        assert_eq!(c.passes_all(), 1);
        assert_eq!(c.through[class as usize], 1);
    }
}

#[test]
pub(crate) fn only_armed_traps_are_the_worlds_to_roll_for() {
    let mut fired = trap(1.0, 1.0);
    fired.state = TrapState::Fired;
    let mut gone = trap(2.0, 2.0);
    gone.state = TrapState::Disarmed;
    let armed = armed_of(&[trap(0.0, 0.0), fired, gone]);
    assert_eq!(armed.len(), 1);
    assert_eq!(armed[0].position, [0.0, 0.0, 0.0]);
    assert!(
        (segment_closest(
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(10.0, 0.0, 1.0),
            Vec3::new(5.0, 0.0, 0.0)
        ) - 1.0)
            .abs()
            < 1e-5
    );
}

/// The switch off, the bots roll for nothing and count nothing, however
/// many traps lie near; on, a bot with a trap ahead rolls and notices it.
#[test]
pub(crate) fn off_nothing_is_rolled_and_on_a_bot_near_a_trap_rolls() {
    use super::traps::run_with;
    let trap_at = Vec3::new(18.0, 0.0, 12.0);
    let (_, off) = run_with(None, Tuning::default(), 12, Some(trap_at));
    assert_eq!(off.react_counts(), Default::default());

    let mut on = Tuning::default();
    on.traps.react.on = true;
    on.traps.react.p0 = 1.0;
    on.traps.react.cone_deg = 360.0;
    on.traps.react.far_m = 20.0;
    let (_, bots) = run_with(None, on, 12, Some(trap_at));
    let c = bots.react_counts();
    assert!(c.rolls >= 1, "{c:?}");
    assert_eq!(c.rolls, c.noticed + c.missed);
    assert!(c.noticed >= 1, "{c:?}");
}

/// A train turns the response on with a tuning patch, `traps.react`; a name
/// the section does not have is refused, and a bad value is named.
#[test]
pub(crate) fn a_tuning_patch_turns_it_on_and_bad_ones_are_refused() {
    let on: Tuning = serde_json::from_str(r#"{"traps":{"react":{"on":true,"p0":0.1}}}"#).unwrap();
    assert!(on.traps.react.on);
    assert_eq!(on.traps.react.p0, 0.1);
    assert_eq!(on.traps.react.far_m, 8.0);
    assert!(serde_json::from_str::<Tuning>(r#"{"traps":{"react":{"shoot":true}}}"#).is_err());
    let mut bad = Tuning::default();
    bad.traps.react.far_m = bad.traps.react.near_m;
    assert!(bad.validate().is_err());
}
