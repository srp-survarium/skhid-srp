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

// ---- R2: the shot ---------------------------------------------------------

use crate::v026g::bots::trap_react::{End, Out, Reaction, Tick, mate_firing, step, view_error};

fn shoot_t() -> ReactTuning {
    ReactTuning {
        on: true,
        shot: true,
        ..ReactTuning::default()
    }
}

fn tick(now: u32) -> Tick {
    Tick {
        now,
        err_rad: 0.0,
        calm: true,
        ready: true,
        mate_firing: false,
    }
}

fn aim_at(since: u32) -> Reaction {
    let trap = trap(5.0, 0.0);
    Reaction::Aim {
        key: TrapKey::of(&trap),
        at: Vec3::new(5.0, 0.0, 0.0),
        since,
    }
}

#[test]
pub(crate) fn the_aim_waits_for_the_sights_then_the_first_shot_goes_with_the_key_down_one_tick() {
    let t = shoot_t();
    // Before the sights are up: looking, no shot.
    let (state, out) = step(aim_at(1_000), tick(1_000 + t.aim_settle_ms - 50), true, &t);
    assert!(matches!(state, Reaction::Aim { .. }));
    assert!(out.look.is_some() && !out.fire && !out.shot);
    // Settled but the view is off: still looking.
    let off = Tick {
        err_rad: t.aim_ok_rad * 3.0,
        ..tick(1_000 + t.aim_settle_ms)
    };
    assert!(matches!(
        step(aim_at(1_000), off, true, &t).0,
        Reaction::Aim { .. }
    ));
    // Settled and on it: the first shot, key down.
    let now = 1_000 + t.aim_settle_ms;
    let (state, out) = step(aim_at(1_000), tick(now), true, &t);
    assert!(out.fire && out.shot);
    let Reaction::Shoot {
        shots, last, down, ..
    } = state
    else {
        panic!("{state:?}");
    };
    assert_eq!((shots, last, down), (1, now, true));
    // The next tick the key is up, no new shot.
    let (state, out) = step(state, tick(now + 50), true, &t);
    assert!(!out.fire && !out.shot);
    assert!(matches!(
        state,
        Reaction::Shoot {
            down: false,
            shots: 1,
            ..
        }
    ));
    // Not before the gap; at the gap the second.
    let (state, out) = step(state, tick(now + t.shot_gap_ms - 50), true, &t);
    assert!(!out.fire);
    let (state, out) = step(state, tick(now + t.shot_gap_ms), true, &t);
    assert!(out.fire && out.shot);
    assert!(matches!(
        state,
        Reaction::Shoot {
            shots: 2,
            down: true,
            ..
        }
    ));
}

#[test]
pub(crate) fn the_shot_ends_when_the_trap_goes_the_shots_are_spent_or_something_calls_the_bot_away()
{
    let t = shoot_t();
    let shoot = |shots: u8| Reaction::Shoot {
        key: TrapKey::of(&trap(5.0, 0.0)),
        at: Vec3::new(5.0, 0.0, 0.0),
        since: 10_000,
        shots,
        last: 10_800,
        down: false,
    };
    let ended = |state: Reaction, tick: Tick, armed: bool| {
        let (next, Out { ended, .. }) = step(state, tick, armed, &t);
        assert_eq!(next, Reaction::Idle);
        ended
    };
    // The world no longer shows it armed: gone, with the shots and when.
    assert_eq!(
        ended(shoot(2), tick(11_000), false),
        Some(End::Gone {
            shots: 2,
            last: 10_800
        })
    );
    // A fight, an empty weapon: given up.
    let fight = Tick {
        calm: false,
        ..tick(11_000)
    };
    assert_eq!(ended(shoot(2), fight, true), Some(End::GaveUp));
    let empty = Tick {
        ready: false,
        ..tick(11_000)
    };
    assert_eq!(ended(shoot(2), empty, true), Some(End::GaveUp));
    // The last shot is spent (key up): given up. Too long: given up.
    assert_eq!(
        ended(shoot(t.max_shots), tick(12_000), true),
        Some(End::GaveUp)
    );
    let long = tick(10_000 + (t.max_shoot_s * 1000.0) as u32 + 1);
    assert_eq!(ended(shoot(2), long, true), Some(End::GaveUp));
    // A mate at it: held for.
    let mate = Tick {
        mate_firing: true,
        ..tick(11_000)
    };
    assert_eq!(ended(shoot(2), mate, true), Some(End::HeldForMate));
    // Idle stays idle; an aim that never finds the trap is given up.
    assert_eq!(
        step(Reaction::Idle, tick(0), true, &t),
        (Reaction::Idle, Out::default())
    );
    let lost = Tick {
        err_rad: 1.0,
        ..tick(1_000 + t.aim_timeout_ms + 1)
    };
    assert_eq!(ended(aim_at(1_000), lost, true), Some(End::GaveUp));
}

#[test]
pub(crate) fn a_mate_at_the_trap_is_waited_for_a_while_and_then_left_to() {
    let t = shoot_t();
    let mate = Tick {
        mate_firing: true,
        ..tick(1_500)
    };
    let (state, out) = step(aim_at(1_000), mate, true, &t);
    assert!(matches!(state, Reaction::Aim { .. }) && out.look.is_some() && !out.fire);
    let late = Tick {
        mate_firing: true,
        ..tick(1_000 + t.hold_ms + 1)
    };
    assert_eq!(
        step(aim_at(1_000), late, true, &t).1.ended,
        Some(End::HeldForMate)
    );
}

fn mate_at(place: usize, feet: Vec3, yaw: f32) -> PlaceView {
    PlaceView {
        place,
        team: 0,
        bot: true,
        present: true,
        alive: true,
        feet,
        yaw,
        pitch: 0.0,
        velocity: [0.0, 0.0],
        keys: 0,
        health: 1.0,
        spawned: 0,
        kills: 0,
        deaths: 0,
        streak: 0,
        carrying: None,
        seen_at: 0,
        known_at: 0,
    }
}

#[test]
pub(crate) fn a_mate_is_firing_at_the_trap_only_if_it_is_near_looks_at_it_and_just_shot() {
    use crate::v026g::bots::view::{Sound, SoundKind};
    let t = shoot_t();
    let trap_at = Vec3::new(0.0, 0.0, 6.0);
    let me_at = Vec3::new(0.0, 0.0, 0.0);
    let toward = facing(Vec3::new(2.0, 0.0, 0.0), trap_at);
    let mate = mate_at(7, Vec3::new(2.0, 0.0, 0.0), toward);
    let shot = |from: usize, time: u32| Sound {
        from,
        kind: SoundKind::Shot,
        at: Vec3::new(2.0, 0.0, 0.0),
        time,
    };
    let firing = |sounds: &[Sound], mates: &[&PlaceView], now: u32| {
        mate_firing(sounds, mates, 1, trap_at, me_at, now, &t)
    };
    assert!(firing(&[shot(7, 9_500)], &[&mate], 10_000));
    // An old shot, a shot of a bot that is not at hand, my own shot: no.
    assert!(!firing(&[shot(7, 5_000)], &[&mate], 10_000));
    assert!(!firing(&[shot(8, 9_500)], &[&mate], 10_000));
    assert!(!firing(&[shot(1, 9_500)], &[&mate], 10_000));
    // A mate looking elsewhere: no. A mate far from both: no.
    let away = mate_at(7, Vec3::new(2.0, 0.0, 0.0), toward + 2.0);
    assert!(!firing(&[shot(7, 9_500)], &[&away], 10_000));
    let far = mate_at(
        7,
        Vec3::new(40.0, 0.0, 0.0),
        facing(Vec3::new(40.0, 0.0, 0.0), trap_at),
    );
    assert!(!firing(&[shot(7, 9_500)], &[&far], 10_000));
}

#[test]
pub(crate) fn the_view_error_is_the_angle_to_the_point() {
    let eye = Vec3::new(0.0, 1.6, 0.0);
    let point = Vec3::new(0.0, 0.12, 4.0);
    let yaw = facing(eye, point);
    let pitch = ((0.12f32 - 1.6) / 4.0).atan();
    assert!(view_error(eye, yaw, pitch, point) < 1e-4);
    assert!((view_error(eye, yaw + 0.1, pitch, point) - 0.1).abs() < 1e-3);
    assert!((view_error(eye, yaw, pitch + 0.2, point) - 0.2).abs() < 1e-3);
    // Across the wrap of the yaw the error stays small.
    let wide = Vec3::new(0.0, 1.6, -4.0);
    let behind = facing(eye, wide);
    assert!(view_error(eye, behind + std::f32::consts::TAU, 0.0, wide) < 1e-3);
}

/// A noticed trap in front of a bot with a loaded weapon: the aim comes on,
/// shots go out one tick at a time, at most `max_shots`, and with nothing
/// disarming the trap (the stand-in has no shot disarm) the bot gives up;
/// with the shot switch off it only notices.
#[test]
pub(crate) fn a_noticed_trap_is_shot_a_few_times_and_given_up() {
    use super::traps::run_with;
    let trap_at = Vec3::new(18.0, 0.0, 12.0);
    let mut on = Tuning::default();
    on.traps.react.on = true;
    on.traps.react.p0 = 1.0;
    on.traps.react.cone_deg = 360.0;
    on.traps.react.far_m = 20.0;
    let (_, quiet) = run_with(None, on.clone(), 12, Some(trap_at));
    assert!(quiet.react_counts().noticed >= 1);
    assert_eq!(quiet.react_counts().shoot_started, 0);

    on.traps.react.shot = true;
    let (sent, bots) = run_with(None, on, 14, Some(trap_at));
    let c = bots.react_counts();
    assert!(c.shoot_started >= 1, "{c:?}");
    assert!(
        c.shots_fired >= 1 && c.shots_fired <= 4 * c.shoot_started,
        "{c:?}"
    );
    assert!(c.gave_up >= 1 || c.disarmed_by_shot >= 1, "{c:?}");
    assert!(sent.iter().any(|s| s.1 & key::AIM != 0), "the aim is held");
    assert!(sent.iter().any(|s| s.1 & key::FIRE != 0), "a shot goes out");
}
