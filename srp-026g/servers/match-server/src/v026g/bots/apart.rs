//! A teammate inside the body (A175): the side to step to.
//!
//! There is no collision between teammates, so two bots that want the same ground walk in file a metre apart or stand in each other. With `spacing.apart` a bot with a living
//! teammate within `apart_m` steps aside, away from him; the drive tests the ground that side with the fight's own step test before it presses the key. Reynolds' separation
//! (*Steering Behaviors For Autonomous Characters*, GDC 1999) is the same rule as a steering force; here it is one strafe key, since the motor steers by keys.

use super::*;
use crate::v026g::drive::key;

/// Teammates this far apart in height are not in each other's way (a floor above).
const SAME_FLOOR_M: f32 = 2.0;

/// The strafe key that takes `me` away from the nearest living teammate within `within_m` (none: 0). A teammate straight ahead or behind (within 0.15 m of the line)
/// is stepped away from to the side the lower place number takes, so two bodies never step the same way into each other.
pub(super) fn key_away(me: &PlaceView, view: &MatchView, within_m: f32) -> u32 {
    let mut nearest: Option<(f32, &PlaceView)> = None;
    for mate in view.team(me.team) {
        if mate.place == me.place || (mate.feet.y - me.feet.y).abs() >= SAME_FLOOR_M {
            continue;
        }
        let d = mate.feet.distance_xz(me.feet);
        if d < within_m && nearest.is_none_or(|(best, _)| d < best) {
            nearest = Some((d, mate));
        }
    }
    let Some((_, mate)) = nearest else {
        return 0;
    };
    let [lx, lz] = kin::forward(me.yaw + std::f32::consts::FRAC_PI_2);
    let left = (mate.feet.x - me.feet.x) * lx + (mate.feet.z - me.feet.z) * lz;
    if left > 0.15 {
        key::RIGHT
    } else if left < -0.15 {
        key::LEFT
    } else if me.place < mate.place {
        key::LEFT
    } else {
        key::RIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::view::Mode;

    fn view_of(places: Vec<PlaceView>) -> MatchView {
        MatchView {
            clock: 0,
            mode: Mode::TeamDeathmatch,
            time_left_s: 600.0,
            total_s: 900.0,
            points: [0, 0],
            places,
            batteries: Vec::new(),
            marks: Vec::new(),
            containers: Vec::new(),
            sounds: Vec::new(),
            deaths: Vec::new(),
            world_time: 0,
            can_respawn: [true, true],
            next_battery: None,
            grenades: Vec::new(),
            smoke: Default::default(),
        }
    }

    fn at(place: usize, team: u8, x: f32, z: f32) -> PlaceView {
        PlaceView {
            place,
            team,
            bot: true,
            present: true,
            alive: true,
            feet: Vec3::new(x, 0.0, z),
            yaw: 0.0,
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
            late_ms: 0,
            known_at: 0,
        }
    }

    #[test]
    fn a_bot_steps_away_from_the_teammate_inside_it_and_not_from_one_outside_it_or_an_enemy() {
        // Yaw 0 faces +z; the left of it is -x (kin::forward(yaw + 90 deg) = [-1, 0]).
        let me = at(0, 0, 0.0, 0.0);
        let on_left = view_of(vec![me.clone(), at(1, 0, -0.8, 0.0)]);
        assert_eq!(key_away(&me, &on_left, 1.1), key::RIGHT);
        let on_right = view_of(vec![me.clone(), at(1, 0, 0.8, 0.0)]);
        assert_eq!(key_away(&me, &on_right, 1.1), key::LEFT);
        let outside = view_of(vec![me.clone(), at(1, 0, 1.4, 0.0)]);
        assert_eq!(key_away(&me, &outside, 1.1), 0);
        let enemy = view_of(vec![me.clone(), at(1, 1, 0.5, 0.0)]);
        assert_eq!(key_away(&me, &enemy, 1.1), 0);
        let above = view_of(vec![me.clone(), PlaceView { feet: Vec3::new(0.3, 3.0, 0.0), ..at(1, 0, 0.3, 0.0) }]);
        assert_eq!(key_away(&me, &above, 1.1), 0);
    }

    #[test]
    fn two_bodies_on_one_spot_step_to_opposite_sides() {
        let a = at(0, 0, 0.0, 0.0);
        let b = at(1, 0, 0.0, 0.0);
        let view = view_of(vec![a.clone(), b.clone()]);
        assert_ne!(key_away(&a, &view, 1.1), key_away(&b, &view, 1.1));
    }
}
