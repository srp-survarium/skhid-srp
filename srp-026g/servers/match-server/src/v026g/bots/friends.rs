//! Holding fire while a teammate is in the line of fire (the owner,
//! 2026-10-23: bots shoot at an enemy through a teammate, bot or player,
//! standing between them).
//!
//! The game has friendly fire: a bullet hits whatever body it meets first,
//! a teammate's too (PROVEN by the saved worlds: each place's damage from
//! each other place, and the team-kill counter, event 4). The aim's own
//! test ([`super::aimpoint::Sight::clear_shot`]) looks at the world only,
//! so a bot fired with a teammate's body on the line.
//!
//! A teammate blocks the shot when his body comes within a margin of a
//! line the bullets may take, short of the target (and a little past it:
//! a miss goes on). The lines are the view now and the view it turns to
//! this tick, as the wall test's; the margin is the body's half width and,
//! growing with the distance along the line, the weapon's spread
//! ([`FriendlyFireTuning::spread_deg`]: an automatic weapon's burst
//! bloom, a shotgun's pellets) and the aim's error to come (the view
//! wanders about the target from tick to tick). A human teammate's body is
//! reckoned on from his last input, so his margin is a little wider.
//!
//! The body is the upright segment from the shins to the top of the head,
//! as [`super::aimpoint::Part`]'s heights: the 0.100b model's, a GUESS for
//! 0.26g's.
//!
//! With a teammate in the way the bot does not fire and steps to the side
//! away from him ([`Blocked::away`]), as a player would to clear his line.

use mapdata::Vec3;
use serde::{Deserialize, Serialize};

use super::guns::{Gun, Handling};
use super::kin::forward;
use super::view::{MatchView, Place, PlaceView};
use crate::v026g::drive::key;

/// How wide a berth a bot gives a teammate when it fires.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FriendlyFireTuning {
    /// Off: the bot fires whoever is in the way (as before 2026-10-23).
    pub on: bool,
    /// The body's half width, metres (the upper body capsule's 0.215 m
    /// and the arms).
    pub body_m: f32,
    /// Added for a human teammate, whose body is reckoned between his
    /// inputs, metres.
    pub human_m: f32,
    /// The spread of the shot, by handling, degrees from the line: an
    /// automatic weapon's bloom over a burst, one round of a semi-automatic
    /// or bolt weapon, a shotgun's pellets.
    pub spread_deg: SpreadDeg,
    /// Teammates this far past the target count too, metres (a miss goes
    /// on).
    pub past_target_m: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpreadDeg {
    pub auto: f32,
    pub semi: f32,
    pub bolt: f32,
    pub shotgun: f32,
}

impl Default for FriendlyFireTuning {
    fn default() -> Self {
        Self {
            on: true,
            body_m: 0.35,
            human_m: 0.25,
            spread_deg: SpreadDeg {
                auto: 3.0,
                semi: 1.5,
                bolt: 0.75,
                shotgun: 4.0,
            },
            past_target_m: 2.0,
        }
    }
}

impl FriendlyFireTuning {
    /// The spread of `gun`'s shot, radians: a shotgun's (category 14)
    /// pellets whatever its action, else by its handling.
    pub fn spread(&self, gun: &Gun) -> f32 {
        let deg = if gun.category == SHOTGUN_CATEGORY {
            self.spread_deg.shotgun
        } else {
            match gun.handling {
                Handling::Burst(_) => self.spread_deg.auto,
                Handling::Semi | Handling::Barrels => self.spread_deg.semi,
                Handling::Bolt => self.spread_deg.bolt,
            }
        };
        deg.to_radians()
    }
}

/// A teammate in the line of fire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blocked {
    pub who: Place,
    /// The side key that steps away from him (the shooter's left or right
    /// of the line).
    pub away: u32,
}

/// The dictionary's category of shotguns ([`Gun::category`]).
const SHOTGUN_CATEGORY: u8 = 14;

/// The body's segment over the feet, standing and crouched, metres: the
/// shins to the top of the head.
const BODY_STANDING: (f32, f32) = (0.25, 1.75);
const BODY_CROUCHED: (f32, f32) = (0.15, 1.15);

/// A direction of the view `(yaw, pitch)`.
pub fn along(yaw: f32, pitch: f32) -> Vec3 {
    let [fx, fz] = forward(yaw);
    let (c, s) = (pitch.cos(), pitch.sin());
    Vec3::new(fx * c, s, fz * c)
}

/// The nearest teammate of `me` whose body comes within the margin of any
/// of the lines `dirs` from `me`'s eye, short of `distance` (the target's,
/// metres) and [`FriendlyFireTuning::past_target_m`] past it; `spread`:
/// the shot's spread from the line, radians ([`FriendlyFireTuning::spread`]).
pub fn blocking(
    me: &PlaceView,
    view: &MatchView,
    dirs: &[Vec3],
    distance: f32,
    spread: f32,
    t: &FriendlyFireTuning,
) -> Option<Blocked> {
    if !t.on {
        return None;
    }
    let eye = me.eye();
    let reach = distance + t.past_target_m;
    let grow = spread.tan();
    let mut best: Option<(f32, Place, f32)> = None;
    for other in view.team(me.team) {
        if other.place == me.place || !other.alive || !other.present {
            continue;
        }
        let (low, high) = if other.crouched() {
            BODY_CROUCHED
        } else {
            BODY_STANDING
        };
        let foot = other.feet + Vec3::new(0.0, low, 0.0);
        let head = other.feet + Vec3::new(0.0, high, 0.0);
        let width = t.body_m + if other.bot { 0.0 } else { t.human_m };
        for &dir in dirs {
            let Some((s, gap)) = ray_to_segment(eye, dir, reach, foot, head) else {
                continue;
            };
            if gap <= width + grow * s && best.is_none_or(|(d, _, _)| s < d) {
                // Which side of the line he stands on, seen from above:
                // his offset across the line, to the shooter's right.
                let [rx, rz] = forward(me.yaw - std::f32::consts::FRAC_PI_2);
                let to = other.feet - me.feet;
                best = Some((s, other.place, to.x * rx + to.z * rz));
            }
        }
    }
    best.map(|(_, who, right)| Blocked {
        who,
        away: if right >= 0.0 { key::LEFT } else { key::RIGHT },
    })
}

/// The nearest approach of the ray `from + dir * s` (`dir` of unit length,
/// `0 <= s <= reach`) to the segment `a..b`: `(s, gap)`. `None` when the
/// nearest approach is behind the eye.
fn ray_to_segment(from: Vec3, dir: Vec3, reach: f32, a: Vec3, b: Vec3) -> Option<(f32, f32)> {
    let u = b - a;
    let w = from - a;
    let (uu, ud) = (u.dot(u), u.dot(dir));
    let (uw, dw) = (u.dot(w), dir.dot(w));
    // Minimise |from + dir s - (a + u v)| over s in [0, reach], v in [0, 1].
    let denom = uu - ud * ud;
    let mut s = if denom > 1e-9 {
        (ud * uw - uu * dw) / denom
    } else {
        -dw
    }
    .clamp(0.0, reach);
    let v = ((uw + ud * s) / uu).clamp(0.0, 1.0);
    // The segment's point fixed, the ray's nearest to it.
    s = (dir.dot(a + u * v - from)).clamp(0.0, reach);
    if s <= 0.0 {
        return None;
    }
    let gap = (from + dir * s - (a + u * v)).length();
    Some((s, gap))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::view::Mode;

    fn body(place: Place, team: u8, feet: Vec3, bot: bool) -> PlaceView {
        PlaceView {
            place,
            team,
            bot,
            present: true,
            alive: true,
            feet,
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
        }
    }

    fn view(places: Vec<PlaceView>) -> MatchView {
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
        }
    }

    /// The line from `me`'s eye to `at`.
    fn line_to(me: &PlaceView, at: Vec3) -> Vec3 {
        let to = at - me.eye();
        to * (1.0 / to.length())
    }

    #[test]
    fn a_teammate_on_the_line_blocks_and_one_beside_it_does_not() {
        let t = FriendlyFireTuning::default();
        let me = body(0, 0, Vec3::ZERO, true);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 30.0), true);
        let dir = line_to(&me, enemy.chest());
        let spread = t.spread_deg.auto.to_radians();
        for (x, z, blocks) in [
            // Right on the line, halfway.
            (0.0, 15.0, true),
            // Just off it, near: within the body and the bloom.
            (0.5, 10.0, true),
            // A step to the side, near: clear.
            (1.2, 5.0, false),
            // 1.5 m to the side 25 m on: inside the bloom's 1.3 m and the body.
            (1.5, 25.0, true),
            // Behind the shooter.
            (0.0, -3.0, false),
            // Far past the enemy.
            (0.0, 40.0, false),
        ] {
            let mate = body(2, 0, Vec3::new(x, 0.0, z), true);
            let v = view(vec![me.clone(), enemy.clone(), mate]);
            let got = blocking(&me, &v, &[dir], 30.0, spread, &t);
            assert_eq!(got.is_some(), blocks, "a teammate at ({x}, {z})");
        }
    }

    #[test]
    fn an_enemy_in_the_way_is_no_teammate_and_a_dead_one_is_not_in_the_way() {
        let t = FriendlyFireTuning::default();
        let me = body(0, 0, Vec3::ZERO, true);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 30.0), true);
        let other_enemy = body(2, 1, Vec3::new(0.0, 0.0, 15.0), true);
        let mut dead = body(3, 0, Vec3::new(0.0, 0.0, 10.0), true);
        dead.alive = false;
        let dir = line_to(&me, enemy.chest());
        let v = view(vec![me.clone(), enemy, other_enemy, dead]);
        assert_eq!(blocking(&me, &v, &[dir], 30.0, 0.05, &t), None);
    }

    #[test]
    fn a_human_teammate_gets_a_wider_berth() {
        let t = FriendlyFireTuning::default();
        let me = body(0, 0, Vec3::ZERO, true);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 30.0), true);
        let dir = line_to(&me, enemy.chest());
        // 0.5 m off the line 3 m on, a bolt action's thin spread.
        let spread = t.spread_deg.bolt.to_radians();
        let near = Vec3::new(0.5, 0.0, 3.0);
        let bot = view(vec![me.clone(), enemy.clone(), body(2, 0, near, true)]);
        let human = view(vec![me.clone(), enemy, body(2, 0, near, false)]);
        assert_eq!(blocking(&me, &bot, &[dir], 30.0, spread, &t), None);
        assert!(blocking(&me, &human, &[dir], 30.0, spread, &t).is_some());
    }

    #[test]
    fn it_steps_away_from_the_side_he_stands_on() {
        let t = FriendlyFireTuning::default();
        let me = body(0, 0, Vec3::ZERO, true);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 30.0), true);
        let dir = line_to(&me, enemy.chest());
        // Yaw 0 faces +z; the right is +x (`forward(yaw - π/2)`).
        let right = view(vec![
            me.clone(),
            enemy.clone(),
            body(2, 0, Vec3::new(0.2, 0.0, 10.0), true),
        ]);
        let left = view(vec![
            me.clone(),
            enemy,
            body(2, 0, Vec3::new(-0.2, 0.0, 10.0), true),
        ]);
        let away = |v: &MatchView| blocking(&me, v, &[dir], 30.0, 0.05, &t).map(|b| b.away);
        assert_eq!(away(&right), Some(key::LEFT));
        assert_eq!(away(&left), Some(key::RIGHT));
    }

    #[test]
    fn a_crouched_teammate_under_a_level_line_does_not_block_a_standing_shot_far_over_him() {
        let t = FriendlyFireTuning::default();
        let me = body(0, 0, Vec3::ZERO, true);
        // The line climbs to an enemy on a roof 10 m up.
        let enemy = body(1, 1, Vec3::new(0.0, 10.0, 20.0), true);
        let dir = line_to(&me, enemy.chest());
        let mut mate = body(2, 0, Vec3::new(0.0, 0.0, 12.0), true);
        mate.keys = key::CROUCH;
        let v = view(vec![me.clone(), enemy, mate]);
        assert_eq!(blocking(&me, &v, &[dir], 23.0, 0.03, &t), None);
    }
}
