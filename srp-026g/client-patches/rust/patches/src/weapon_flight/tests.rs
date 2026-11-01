use super::*;
use std::vec;
use std::vec::Vec;

const DT: f32 = 1.0 / 60.0;

fn pose(point: Vec3, yaw: f32, pitch: f32, distance: f32) -> CameraPose {
    CameraPose { point, yaw, pitch, distance, side: 0.0, zoom: 1.0 }
}

/// The classic lobby's bench (its project's srp_lobby: (4.25, 1.30, 3.05), -pi/2).
fn bench() -> BenchView {
    BenchView { point: Vec3::new(4.25, 1.3, 3.05), yaw: -PI / 2.0, pitch: -0.25, side: 0.37, zoom: 1.0 }
}

fn dist(a: Vec3, b: Vec3) -> f32 {
    let d = a - b;
    ragdoll::math::sqrt(d.dot(d))
}

/// Runs ticks until `done` or `limit`; the poses given and the last state.
fn fly(mut flight: Flight, target: f32, limit: usize, done: impl Fn(&Flight) -> bool) -> (Vec<Action>, Flight) {
    let mut out = Vec::new();
    for _ in 0..limit {
        let (next, action) = flight.step(Event::Tick { dt: DT, distance_target: target });
        out.push(action);
        flight = next;
        if done(&flight) {
            break;
        }
    }
    (out, flight)
}

/// No jolt: the eye's velocity changes from one tick to the next by no more
/// than the smoothstep's own over the way (its acceleration at most 6 L / T^2)
/// and a little for the arc (round 7's bump over a character changed the
/// height's velocity by 0.31 m a tick in the Room, the owner's jump of
/// 2026-10-31; the arc's by 0.03).
fn assert_smooth(eyes: &[Vec3]) {
    let way = dist(*eyes.last().unwrap(), eyes[0]);
    let allowed = 6.0 * way * (DT / DURATION) * (DT / DURATION) * 1.1 + 0.035;
    let v: Vec<Vec3> = eyes.windows(2).map(|w| w[1] - w[0]).collect();
    for (n, w) in v.windows(2).enumerate() {
        let jolt = dist(w[1], w[0]);
        assert!(jolt < allowed, "tick {n}: the velocity changed by {jolt} m a tick (at most {allowed})");
    }
}

fn posed(a: &Action) -> CameraPose {
    match a {
        Action::Hold(p) | Action::Release(p) => *p,
        Action::Leave => panic!("no pose"),
    }
}

#[test]
fn to_the_bench_on_a_straight_line_eased_and_exactly_there() {
    let start = pose(Vec3::new(0.0, 1.2, 0.0), 0.4, 0.1, 1.96);
    let view = bench();
    let (flight, _) = Flight::Stock.step(Event::Asked { goal: Goal::Bench { view, distance: 1.2 }, now: start });
    assert_eq!(flight.word(), FlightWord::Moving);
    let (actions, end) = fly(flight, 1.2, 200, |f| matches!(f, Flight::AtBench { .. }));
    assert_eq!(end, Flight::AtBench { view, distance: 1.2 });
    assert_eq!(end.word(), FlightWord::Resting);
    let frames = actions.len() as f32 * DT;
    assert!((frames - DURATION).abs() < 1.5 * DT, "{frames}");
    let last = posed(actions.last().unwrap());
    assert_eq!(last, view.at(1.2), "exactly the bench's view at the end");
    // the eye on the line between the two eyes, its steps growing then shrinking
    let (e0, e1) = (start.eye(), view.at(1.2).eye());
    let axis = e1 - e0;
    let mut eyes = vec![e0];
    eyes.extend(actions.iter().map(|a| posed(a).eye()));
    // across the floor on the line; up by one arc where it passes over a character (here the
    // squad's right member's spot, 0.9 m)
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let (f0, faxis) = (flat(e0), flat(axis));
    let flen = dist(flat(e1), f0);
    for e in &eyes {
        let along = (flat(*e) - f0).dot(faxis) / flen;
        let off = dist(flat(*e), f0 + faxis * (along / flen));
        assert!(off < 1e-4, "off the line by {off}");
        let on = e0 + axis * (along / flen);
        assert!(e.y >= on.y - 1e-4 && e.y <= TOP + 1e-4, "{} against {}", e.y, on.y);
    }
    assert_smooth(&eyes);
    let steps: Vec<f32> = eyes.windows(2).map(|w| dist(w[1], w[0])).collect();
    let top = steps.iter().cloned().fold(0.0, f32::max);
    let peak = steps.iter().position(|s| *s == top).unwrap();
    assert!(steps[0] < 0.1 * top && *steps.last().unwrap() < 0.1 * top, "eased at both ends: {steps:?}");
    assert!(steps[..peak].windows(2).all(|w| w[1] >= w[0] - 1e-5), "speeding up to the middle");
    assert!(steps[peak..].windows(2).all(|w| w[1] <= w[0] + 1e-5), "slowing down after it");
    // the view's yaw goes the short way, monotonic
    let yaws: Vec<f32> = actions.iter().map(|a| posed(a).yaw).collect();
    assert!(yaws.windows(2).all(|w| w[1] <= w[0] + 1e-6) && yaws.iter().all(|y| *y >= -PI / 2.0 - 1e-6));
}

#[test]
fn back_to_the_stock_pose_exactly_then_the_stock_camera() {
    let at = bench().at(1.2);
    let home = pose(Vec3::new(0.0, 1.2, 0.0), 0.4 + 2.0 * PI, 0.1, 2.2);
    let (flight, _) = Flight::AtBench { view: bench(), distance: 1.2 }.step(Event::Asked { goal: Goal::Home(home), now: at });
    let (actions, end) = fly(flight, 4.6, 200, |f| matches!(f, Flight::Landed { .. }));
    assert_eq!((end, end.word()), (Flight::Landed { at: home }, FlightWord::Moving), "one tick more with no ray");
    assert_eq!(actions.last(), Some(&Action::Hold(home)), "the stock pose exactly");
    assert!(actions.iter().all(|a| matches!(a, Action::Hold(_))));
    // the yaw turned the short way (the kept yaw is a turn away from the bench's)
    let second = posed(&actions[actions.len() - 2]);
    assert!((second.yaw - (0.4)).abs() < 0.05, "{}", second.yaw);
    assert_eq!(end.step(Event::Tick { dt: DT, distance_target: 4.6 }), (Flight::Stock, Action::Release(home)),
               "then its steps released, at the same pose");
    assert_eq!(Flight::Stock.step(Event::Tick { dt: DT, distance_target: 4.6 }), (Flight::Stock, Action::Leave));
}

#[test]
fn a_request_mid_flight_starts_from_where_the_camera_is() {
    let start = pose(Vec3::new(0.0, 1.2, 0.0), 0.0, 0.0, 1.96);
    let (mut flight, _) = Flight::Stock.step(Event::Asked { goal: Goal::Bench { view: bench(), distance: 1.2 }, now: start });
    let mut last = start;
    for _ in 0..10 {
        let (next, a) = flight.step(Event::Tick { dt: DT, distance_target: 1.2 });
        flight = next;
        last = posed(&a);
    }
    let (back, _) = flight.step(Event::Asked { goal: Goal::Home(start), now: last });
    let (next, a) = back.step(Event::Tick { dt: DT, distance_target: 1.96 });
    assert!(matches!(next, Flight::Home { .. }));
    assert!(dist(posed(&a).eye(), last.eye()) < 0.02, "no jump at the turn back");
}

#[test]
fn the_bench_distance_follows_its_target_by_the_glide() {
    let view = bench();
    let mut flight = Flight::AtBench { view, distance: 1.2 };
    let mut prev = 1.2;
    for n in 0..400 {
        let (next, a) = flight.step(Event::Tick { dt: DT, distance_target: 0.8 });
        flight = next;
        let d = posed(&a).distance;
        assert!(d <= prev && prev - d < 0.04, "tick {n}: {prev} -> {d}");
        prev = d;
    }
    assert_eq!(flight, Flight::AtBench { view, distance: 0.8 });
    // a new target mid-flight (the weapon framed): the eye moves on, no jump
    let start = pose(Vec3::new(0.0, 1.2, 0.0), 0.4, 0.1, 1.96);
    let (mut flight, _) = Flight::Stock.step(Event::Asked { goal: Goal::Bench { view, distance: 1.2 }, now: start });
    let mut eye = start.eye();
    for n in 0..60 {
        let target = if n < 10 { 1.2 } else { 0.9 };
        let (next, a) = flight.step(Event::Tick { dt: DT, distance_target: target });
        flight = next;
        let e = posed(&a).eye();
        assert!(dist(e, eye) < 0.4, "tick {n}");
        eye = e;
    }
}

#[test]
fn the_home_goal_takes_the_rays_cut_and_the_ticks_clamp() {
    let request = TableRequest { count: 2, table_on: false, x: 0.0, z: 0.0, yaw: 0.3, pitch: 0.2, zoom: 1.0 };
    let targets = StepTargets { height: 1.2, side: 0.0, distance: 4.6 };
    for (hit, want) in [(2.2, 2.2), (0.9, LEAST), (9.0, 4.6)] {
        let mut asked = None;
        let goal = goal_of(&request, &targets, |p, f, d| {
            asked = Some((p, f, d));
            ragdoll::math::min(hit, d)
        });
        let (p, f, d) = asked.unwrap();
        assert_eq!((p, d), (Vec3::new(0.0, 1.2, 0.0), 4.6));
        assert!(dist(f, forward(0.3, 0.2)) < 1e-6);
        match goal {
            Goal::Home(to) => assert_eq!((to.distance, to.yaw, to.pitch), (want, 0.3, 0.2)),
            Goal::Bench { .. } => panic!("table off"),
        }
    }
    let on = TableRequest { table_on: true, x: 4.25, z: 3.05, yaw: -PI / 2.0, pitch: -0.25, ..request };
    let goal = goal_of(&on, &StepTargets { height: 1.3, side: 0.37, distance: 1.2 }, |_, _, _| panic!("no ray"));
    assert_eq!(goal, Goal::Bench { view: BenchView { point: Vec3::new(4.25, 1.3, 3.05), yaw: -PI / 2.0, pitch: -0.25,
                                                        side: 0.37, zoom: 1.0 }, distance: 1.2 });
}

#[test]
fn every_sequence_stays_finite_and_leaves_only_the_stock_camera() {
    let homes = [pose(Vec3::new(0.0, 1.2, 0.0), 0.0, 0.0, 1.96), pose(Vec3::new(0.0, 1.2, 0.0), 3.0, -1.0, 4.6)];
    let events = |k: usize| match k {
        0 => Event::Asked { goal: Goal::Bench { view: bench(), distance: 1.2 }, now: homes[0] },
        1 => Event::Asked { goal: Goal::Home(homes[1]), now: bench().at(1.2) },
        2 => Event::Tick { dt: 0.1, distance_target: 0.7 },
        _ => Event::Tick { dt: DT, distance_target: 4.6 },
    };
    for seq in 0..4usize.pow(7) {
        let mut flight = Flight::Stock;
        let mut s = seq;
        for _ in 0..7 {
            let (next, a) = flight.step(events(s % 4));
            s /= 4;
            match a {
                Action::Leave => assert_eq!(flight, Flight::Stock),
                Action::Hold(p) | Action::Release(p) => {
                    assert!(p.point.is_finite() && p.yaw.is_finite() && p.pitch.is_finite() && p.distance > 0.0)
                }
            }
            flight = next;
        }
    }
}

#[test]
fn a_line_through_a_character_goes_over_its_head() {
    // the camp's: the bench's eye behind the character, the start in front of it
    let start = pose(Vec3::new(0.0, 1.2, 0.0), 0.0, 0.0, 1.55);
    let view = BenchView { point: Vec3::new(-1.6, 1.37, 3.255), yaw: PI / 2.0, pitch: -0.25, side: 0.42, zoom: 1.0 };
    let (flight, _) = Flight::Stock.step(Event::Asked { goal: Goal::Bench { view, distance: 1.2 }, now: start });
    let (actions, _) = fly(flight, 1.2, 200, |f| matches!(f, Flight::AtBench { .. }));
    let mut eyes = vec![start.eye()];
    eyes.extend(actions.iter().map(|a| posed(a).eye()));
    for e in &eyes {
        for (x, z) in CHARACTERS {
            let across = ((e.x - x) * (e.x - x) + (e.z - z) * (e.z - z)).sqrt();
            assert!(across > 0.45 || e.y > 2.0, "the eye {e:?} in a character at ({x}, {z})");
        }
    }
    let steps: Vec<f32> = eyes.windows(2).map(|w| dist(w[1], w[0])).collect();
    assert!(steps.iter().all(|s| *s < 0.35), "no jump: {steps:?}");
    assert_smooth(&eyes);
    assert_eq!(posed(actions.last().unwrap()), view.at(1.2), "exactly there");
}


#[test]
fn every_lobby_flight_is_smooth_there_and_back() {
    // the four lobbies' spots (their projects' srp_lobby), from the stock start and from round
    // the orbit, there and back
    let spots = [
        (Vec3::new(4.25, 1.3, 3.05), -PI / 2.0),
        (Vec3::new(-1.6, 1.37, 3.255), PI / 2.0),
        (Vec3::new(2.289, 1.24, 3.925), -2.3562),
        (Vec3::new(-0.95, 1.35, -0.8), 0.0),
    ];
    for (point, yaw) in spots {
        let view = BenchView { point, yaw, pitch: -0.25, side: 0.37, zoom: 1.0 };
        for k in 0..12 {
            let start = pose(Vec3::new(0.0, 1.2, 0.0), k as f32 * PI / 6.0, 0.0, if k % 2 == 0 { 1.96 } else { 3.5 });
            let (flight, _) = Flight::Stock.step(Event::Asked { goal: Goal::Bench { view, distance: 1.2 }, now: start });
            let (actions, _) = fly(flight, 1.2, 200, |f| matches!(f, Flight::AtBench { .. }));
            let mut eyes = vec![start.eye()];
            eyes.extend(actions.iter().map(|a| posed(a).eye()));
            assert_smooth(&eyes);
            // over the heads on the way (an end beside one is where the stock camera put it)
            let (first, last) = (eyes[0], *eyes.last().unwrap());
            for e in eyes.iter().filter(|e| dist(**e, first) > CLEAR && dist(**e, last) > CLEAR) {
                for (x, z) in CHARACTERS {
                    let across = ((e.x - x) * (e.x - x) + (e.z - z) * (e.z - z)).sqrt();
                    assert!(across > 0.4 || e.y > 1.85, "the eye {e:?} in a character at ({x}, {z})");
                }
            }
            let (flight, _) = Flight::AtBench { view, distance: 1.2 }.step(Event::Asked { goal: Goal::Home(start), now: view.at(1.2) });
            let (actions, _) = fly(flight, 1.96, 200, |f| matches!(f, Flight::Landed { .. }));
            let mut eyes = vec![view.at(1.2).eye()];
            eyes.extend(actions.iter().map(|a| posed(a).eye()));
            assert_smooth(&eyes);
        }
    }
}
