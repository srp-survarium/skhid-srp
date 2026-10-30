//! Where to aim for a bullet that flies: the target's place when the bullet gets there, and the rise that takes
//! the bullet's drop on the way.
//!
//! The client's bullet is not a ray (`bullet::tick` 0x32C180, `bots/` notes `player_bots_weapons.md`, "The
//! bullet's flight"): from the muzzle it follows `p(t) = p0 + v0 (t - k t^2 / 2) + g t^2 / 2` (`compute_parabolic_position`
//! 0x32CB10), `v0` the ammunition's `muzzle_speed`, `k` its `air_resistance`, `g = (0, -9.81, 0)` (the bullet manager's,
//! 0x3219A0), and ends at `t = 1 / (1.1 k)`. PROVEN. A bot that aims at where the target is meets him where he was
//! by the time the bullet arrives: a strafer's 2.6 m/s is 0.25 m (half a torso) past 23-28 m for the slow
//! ammunitions and past 65-78 m for the rifles', and the drop is 0.2-0.8 m at 60-80 m for the slow ones.
//!
//! The lead is the usual one (a predictive aim, the intercept solved by iterating the flight time on the place the
//! target has reached): the target's velocity is the bot's own picture of him ([`super::view::PlaceView::velocity`]),
//! nothing the bot could not see.

use mapdata::Vec3;
use survarium::v026g::damage::AmmoParams;

/// `g`, metres a second squared, the bullet manager's (0x3219A0; `y` is up).
pub const GRAVITY_MPS2: f32 = 9.81;

/// The iterations of the intercept: the flight time on the place the target reaches in the last one's. It
/// settles in two (the target is slow beside the bullet).
const ITERATIONS: usize = 3;

/// How an ammunition flies: only an ammunition with a speed does ([`Flight::of`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    muzzle_speed: f32,
    air_resistance: f32,
}

impl Flight {
    /// The flight of `ammo`; `None` for one with no muzzle speed (the data has none: the bot aims as for a hitscan).
    pub fn of(ammo: &AmmoParams) -> Option<Self> {
        let sane = ammo.muzzle_speed.is_finite()
            && ammo.muzzle_speed > 0.0
            && ammo.air_resistance.is_finite()
            && ammo.air_resistance >= 0.0;
        sane.then_some(Self {
            muzzle_speed: ammo.muzzle_speed,
            air_resistance: ammo.air_resistance,
        })
    }

    /// Seconds for the bullet to go `metres` along its line (`v0 (t - k t^2 / 2) = metres`), `None` past where the
    /// flight ends (`t = 1 / (1.1 k)`).
    pub fn time_to(&self, metres: f32) -> Option<f32> {
        let (v0, k) = (self.muzzle_speed, self.air_resistance);
        if k <= 0.0 {
            return Some(metres / v0);
        }
        let under = 1.0 - 2.0 * k * metres / v0;
        if under < 0.0 {
            return None;
        }
        let t = (1.0 - under.sqrt()) / k;
        (t <= 1.0 / (1.1 * k)).then_some(t)
    }

    /// The fall of the bullet by `seconds` of flight, metres.
    pub fn drop_m(seconds: f32) -> f32 {
        0.5 * GRAVITY_MPS2 * seconds * seconds
    }

    /// How far to aim past `target` (the place the bot's picture has him, led as far as its age) from `eye`, for a
    /// target moving `velocity` (metres a second on the ground): along his velocity by the bullet's flight time,
    /// and up by the drop in that time. `None` when the target is past the flight's end (aim as before).
    pub fn ahead(&self, eye: Vec3, target: Vec3, velocity: [f32; 2]) -> Option<Vec3> {
        let mut seconds = 0.0;
        for _ in 0..ITERATIONS {
            let there = target + Vec3::new(velocity[0] * seconds, 0.0, velocity[1] * seconds);
            seconds = self.time_to((there - eye).length())?;
        }
        Some(Vec3::new(velocity[0] * seconds, Self::drop_m(seconds), velocity[1] * seconds))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bullet of `v0` and `k`.
    fn flight(v0: f32, k: f32) -> Flight {
        Flight { muzzle_speed: v0, air_resistance: k }
    }

    /// The client's path along its line: the distance gone by `t` (`v0 (t - k t^2 / 2)`).
    fn gone(f: &Flight, t: f32) -> f32 {
        f.muzzle_speed * (t - f.air_resistance * t * t / 2.0)
    }

    #[test]
    fn the_time_to_a_place_is_the_path_inverted() {
        for (v0, k) in [(800.0, 1.0), (260.0, 1.0), (450.0, 0.5), (300.0, 0.0)] {
            let f = flight(v0, k);
            for metres in [5.0, 25.0, 60.0, 80.0] {
                let t = f.time_to(metres).expect("within the flight");
                assert!((gone(&f, t) - metres).abs() < 1e-2, "{v0} {k} {metres}: {t}");
            }
        }
    }

    #[test]
    fn the_notes_flight_times_are_met() {
        // `player_bots_weapons.md`: 5.45x39 (800 m/s) 51 / 78 / 106 ms at 40 / 60 / 80 m; .45 ACP (260) 168 / 266 / 380.
        for (v0, ms) in [(800.0, [51.0, 78.0, 106.0]), (260.0, [168.0, 266.0, 380.0])] {
            let f = flight(v0, 1.0);
            for (metres, want) in [40.0, 60.0, 80.0].into_iter().zip(ms) {
                let got = f.time_to(metres).unwrap() * 1000.0;
                assert!((got - want).abs() < 1.5, "{v0} at {metres}: {got} ms against {want}");
            }
        }
    }

    #[test]
    fn nothing_is_reached_past_the_end_of_the_flight() {
        let f = flight(260.0, 1.0);
        // The flight ends at t = 1 / 1.1: 260 (0.909 - 0.413) = 129 m.
        assert!(f.time_to(120.0).is_some());
        assert!(f.time_to(135.0).is_none());
        assert!(f.ahead(Vec3::new(0.0, 1.5, 0.0), Vec3::new(200.0, 1.5, 0.0), [0.0, 2.6]).is_none());
    }

    #[test]
    fn a_target_standing_is_aimed_over_the_drop_alone() {
        let f = flight(260.0, 1.0);
        let eye = Vec3::new(0.0, 1.5, 0.0);
        let ahead = f.ahead(eye, Vec3::new(60.0, 1.3, 0.0), [0.0, 0.0]).unwrap();
        assert_eq!((ahead.x, ahead.z), (0.0, 0.0));
        // 266 ms of flight at 60 m: 0.35 m, the notes' table.
        assert!((ahead.y - 0.35).abs() < 0.02, "{}", ahead.y);
    }

    #[test]
    fn a_strafer_is_led_by_his_speed_and_the_flight_time() {
        let f = flight(260.0, 1.0);
        let eye = Vec3::new(0.0, 1.5, 0.0);
        // Across the line of fire (+z) at 2.6 m/s, 60 m away: the lead is 2.6 x the time to where he will be
        // (a little longer than 266 ms, the bullet goes a bit farther).
        let ahead = f.ahead(eye, Vec3::new(60.0, 1.3, 0.0), [0.0, 2.6]).unwrap();
        assert_eq!(ahead.x, 0.0);
        assert!((ahead.z - 0.69).abs() < 0.03, "{}", ahead.z);
        // Moving away he is led along the line, and the flight is longer.
        let away = f.ahead(eye, Vec3::new(60.0, 1.3, 0.0), [2.6, 0.0]).unwrap();
        assert!(away.x > 0.69 && away.z == 0.0, "{away:?}");
    }

    #[test]
    fn a_faster_bullet_is_led_less() {
        let eye = Vec3::new(0.0, 1.5, 0.0);
        let at = Vec3::new(60.0, 1.3, 0.0);
        let slow = flight(260.0, 1.0).ahead(eye, at, [0.0, 2.6]).unwrap();
        let fast = flight(800.0, 1.0).ahead(eye, at, [0.0, 2.6]).unwrap();
        assert!(fast.z < slow.z / 2.0 && fast.y < slow.y / 4.0, "{fast:?} {slow:?}");
    }

    #[test]
    fn an_ammunition_with_no_muzzle_speed_has_no_flight() {
        let plain = AmmoParams { muzzle_speed: 0.0, ..*survarium::v026g::damage::ammo().first().expect("ammunition") };
        assert!(Flight::of(&plain).is_none());
        let real = survarium::v026g::damage::ammo_named("ammo_5.45x39_7n6m").expect("the AK-74U's ammunition");
        assert!(Flight::of(real).is_some());
    }
}
