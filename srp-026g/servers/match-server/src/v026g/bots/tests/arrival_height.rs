use super::*;
use crate::v026g::bots::kin::yaw_of;
use crate::v026g::bots::motor::{Feel, Motor, Pose, Walked};
use crate::v026g::bots::path::{Route, Waypoint};

fn p(x: f32, y: f32) -> Waypoint {
    Waypoint {
        at: Vec3::new(x, y, 0.0),
        node: 0,
        jump: false,
        low: false,
    }
}

fn walked_on(motor: &mut Motor, route: &mut Route, feet: Vec3, exact: bool) -> Walked {
    let pose = Pose {
        feet,
        yaw: yaw_of([1.0, 0.0]),
        ..Default::default()
    };
    motor.exact_end = exact.then_some(motor.feel.arrive_m);
    motor.walk(route, &pose, 5.0, |_| true).1
}

fn walked(route: &mut Route, feet: Vec3, exact: bool) -> Walked {
    let mut rng = Rng::new(17);
    let mut motor = Motor::new(Feel::default(), &mut rng);
    walked_on(&mut motor, route, feet, exact)
}

#[test]
fn a_body_over_an_exact_spots_end_has_not_arrived() {
    // An overhang 1.5 m over the end, right above it: a walk to an exact
    // spot (Collect's to its stand) walks on.
    let mut route = Route::new(vec![p(0.0, 0.0), p(1.0, 0.0)]);
    assert_ne!(walked(&mut route, Vec3::new(1.0, 1.5, 0.0), true), Walked::Arrived);
    // A stair's rise over it still counts.
    let mut route = Route::new(vec![p(0.0, 0.0), p(1.0, 0.0)]);
    assert_eq!(walked(&mut route, Vec3::new(1.0, 0.3, 0.0), true), Walked::Arrived);
}

#[test]
fn a_body_over_any_other_routes_end_has_arrived_as_before() {
    let mut route = Route::new(vec![p(0.0, 0.0), p(1.0, 0.0)]);
    assert_eq!(walked(&mut route, Vec3::new(1.0, 1.5, 0.0), false), Walked::Arrived);
}

#[test]
fn an_exact_walk_then_another_route_over_its_end_is_arrived() {
    // The tick an exact walk is over its end it walks on; the next tick
    // (an escape's or a zone's route, the flag cleared) it is there.
    let mut rng = Rng::new(17);
    let mut motor = Motor::new(Feel::default(), &mut rng);
    let over = Vec3::new(1.0, 1.5, 0.0);
    let mut route = Route::new(vec![p(0.0, 0.0), p(1.0, 0.0)]);
    assert_ne!(walked_on(&mut motor, &mut route, over, true), Walked::Arrived);
    let mut route = Route::new(vec![p(0.0, 0.0), p(1.0, 0.0)]);
    assert_eq!(walked_on(&mut motor, &mut route, over, false), Walked::Arrived);
}

#[test]
fn an_exact_route_past_a_drop_is_arrived_at_only_once_dropped() {
    // Along a ledge 2 m up, then down past its edge: at the top of the
    // drop, 0.3 m across from the end, it walks on; once down, it is there.
    let mut route = Route::new(vec![p(0.0, 2.0), p(0.7, 2.0), p(1.0, 0.0)]);
    assert_ne!(walked(&mut route, Vec3::new(0.7, 2.0, 0.0), true), Walked::Arrived);
    let mut route = Route::new(vec![p(0.0, 2.0), p(0.7, 2.0), p(1.0, 0.0)]);
    assert_eq!(walked(&mut route, Vec3::new(1.0, 0.0, 0.0), true), Walked::Arrived);
}

#[test]
fn a_box_top_reached_is_arrived_and_its_foot_is_not() {
    let mut route = Route::new(vec![p(0.0, 0.0), p(1.0, 1.0)]);
    assert_eq!(walked(&mut route, Vec3::new(1.0, 1.0, 0.0), true), Walked::Arrived);
    let mut route = Route::new(vec![p(0.0, 0.0), p(1.0, 1.0)]);
    assert_ne!(walked(&mut route, Vec3::new(1.0, 0.0, 0.0), true), Walked::Arrived);
}
