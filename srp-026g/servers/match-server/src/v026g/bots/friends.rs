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
//! line the bullets may take, short of the target and up to
//! [`FriendlyFireTuning::past_target_m`] past it (a miss goes on). The
//! lines are the view now and the view it turns to
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
//! Near the shooter the margin is at least [`FriendlyFireTuning::near_m`]:
//! the body and a semi-automatic's or a bolt's thin spread alone gave
//! 0.35-0.5 m within 10 m, and in the soak half the team hits came through
//! that band (12 seeds each, the floor off and on: TDM 6 against 6, team
//! hits 187 -> 101 and team kills 24 -> 8, kills 1367 -> 1425; batteries
//! 4 against 4, 165 -> 82 and 22 -> 7, kills 1733 -> 1757).
//!
//! What a team kill costs (PROVEN, 0.26g): the game credits a kill only
//! when the killer is neither the victim nor of his team
//! (`shared_statistics::on_player_killed` 0x2F4EB0, the team test at
//! 0x2F5049): a team kill is a death with no one's kill, and pays and
//! scores nothing. In the saved bot matches, bots team-killed 10.5 per 100
//! deaths before this module (79 of 753, 2026-10-16) and 3.1 with it
//! (6 of 194, 2026-10-23); human matches 0.7-2.5.
//!
//! With a teammate in the way the bot does not fire and steps to the side
//! away from him ([`Blocked::away`]), as a player would to clear his line.
//!
//! A bot decides its input up to two ticks before the game plays it
//! ([`super::encode::LEAD_TICKS`]), from a view of its teammates as of the
//! clock: a teammate walking across the line in that time was missed (in
//! 6da44bbb at 437.95 s the owner was 0.86 m off a bot's line 2 m on,
//! walking toward it, and that bot killed him within the second). Each
//! teammate is also taken where his velocity puts him at the tick decided
//! ([`FriendlyFireTuning::lead`]). In the real-map bench (16 seeds, the
//! game's spread), team hits between bodies 0.3 m or more apart went 2.8 ->
//! 1.6 a seed in batteries and 11.1 -> 5.9 in team deathmatch, team kills
//! 6.4 -> 4.3 and 8.4 -> 7.2, kills unchanged (`player_bots_behaviour.md`
//! §2.9).
//!
//! A teammate within [`FriendlyFireTuning::touch_m`] of the shooter
//! blocks whatever his angle off the line, if any of his body is in the
//! front half of it and level with the eye. The server reckons each body on
//! from its last world with no other body in the way (`body.rs`), so two
//! bodies that touch or are wedged are half a metre to a few metres off in
//! the bot's picture, and a teammate in front of it there can be beside or
//! behind it, or stand in it, where the line test does not look. In the
//! owner's matches of 2026-10-24 (0x5e7d0fc0, 0x31632b5a, per-tick
//! recordings) 6 of the 10 team kills were at 0-2.5 m: 5 inside the line
//! test's margin on the true bodies, and in one the two stood in one
//! another. In 5 of them the shooter's or the victim's reckoning was
//! 0.45-4.8 m off about then (its trace, the worlds' corrections)
//! (`player_bots_behaviour.md` §2.13).

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
    /// Teammates this far past the target count too, metres: a miss goes
    /// on, and so do the rounds after a kill (in 0x60aeff4f two bots were
    /// killed on the line 3.5 m and 11.5 m behind the enemy their teammate
    /// fired at; 2 m before 2026-10-24).
    pub past_target_m: f32,
    /// The least margin, metres, however near the shooter: the body's
    /// half width and the spread alone let a line pass a hand's width
    /// from a teammate a few metres on, and the game's bullet met him
    /// there (0: none).
    pub near_m: f32,
    /// Each teammate is also taken where his velocity puts him at the
    /// tick being decided: a bot decides its input up to two ticks before
    /// the game plays it, from a view of the teammates as of the clock (a
    /// human's reckoned to the clock from his last input). Off: where the
    /// view has him.
    pub lead: bool,
    /// Added to that lead, ms. GUESS.
    pub lead_extra_ms: u32,
    /// A teammate whose feet are this near the shooter's, metres, blocks
    /// whatever his angle off the line, if any of his body is in the front
    /// half of it and level with the eye ([`TOUCH_HEIGHT_M`]): the server
    /// reckons each body on from its world without the others in the way,
    /// so bodies that touch are often half a metre or more off in the
    /// bot's picture, and one beside or in front of it can be behind it
    /// there (0: off).
    pub touch_m: f32,
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
            past_target_m: 15.0,
            near_m: 0.63,
            lead: true,
            lead_extra_ms: 0,
            touch_m: 2.0,
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

/// A bot holding fire for a teammate in its line, as its fire step saw
/// it this tick ([`super::reflex::Reflex::held_for`]): the real-map
/// bench's blocked spells (`player_bots_behaviour.md` §2.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held {
    pub who: Place,
    /// It would have fired but for him (aimed, in range, a round in).
    pub ready: bool,
    /// Its legs run.
    pub running: bool,
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
/// metres) and [`FriendlyFireTuning::past_target_m`] past it, or who
/// stands within [`FriendlyFireTuning::touch_m`] in front of it; `spread`:
/// the shot's spread from the line, radians ([`FriendlyFireTuning::spread`]);
/// `ahead_s`: each teammate is taken where the view has him, half way and
/// this far along his velocity ([`FriendlyFireTuning::lead`]).
pub fn blocking(
    me: &PlaceView,
    view: &MatchView,
    dirs: &[Vec3],
    distance: f32,
    spread: f32,
    ahead_s: f32,
    t: &FriendlyFireTuning,
) -> Option<Blocked> {
    if !t.on {
        return None;
    }
    let mut best: Option<(f32, Place, f32)> = None;
    for other in mates(me, view) {
        for near in assess(me, other, dirs, distance, spread, ahead_s, t) {
            if near.blocks() && best.is_none_or(|(d, _, _)| near.s < d) {
                best = Some((near.s, other.place, side(me, other)));
            }
        }
    }
    best.map(|(_, who, right)| Blocked {
        who,
        away: if right >= 0.0 { key::LEFT } else { key::RIGHT },
    })
}

/// The teammate nearest to blocking `me`'s shot as [`blocking`] weighs
/// him (the least [`Near::room`]), for the trace: where the bot had him
/// and how near the line he was. `None` with friends off, or with no
/// teammate short of the target or touching.
pub fn closest(
    me: &PlaceView,
    view: &MatchView,
    dirs: &[Vec3],
    distance: f32,
    spread: f32,
    ahead_s: f32,
    t: &FriendlyFireTuning,
) -> Option<Near> {
    if !t.on {
        return None;
    }
    mates(me, view)
        .flat_map(|other| assess(me, other, dirs, distance, spread, ahead_s, t))
        .min_by(|a, b| a.room().total_cmp(&b.room()))
}

/// A teammate weighed against one line of fire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Near {
    pub who: Place,
    /// Where the bot had his feet (led, [`FriendlyFireTuning::lead`]).
    pub feet: Vec3,
    /// How far along the line his nearest approach is, metres; for one
    /// touching, at most his feet's distance.
    pub s: f32,
    /// His body's nearest approach to the line, metres (none: behind the
    /// eye).
    pub gap: Option<f32>,
    /// The margin the line keeps from him there, metres.
    pub margin: f32,
    /// Within [`FriendlyFireTuning::touch_m`], in front.
    pub touch: bool,
}

impl Near {
    pub fn blocks(&self) -> bool {
        self.touch || self.gap.is_some_and(|gap| gap <= self.margin)
    }

    /// How much more than the margin the line keeps from him, metres
    /// (negative: he blocks; touching: -1).
    pub fn room(&self) -> f32 {
        if self.touch {
            -1.0
        } else {
            self.gap.map_or(f32::INFINITY, |gap| gap - self.margin)
        }
    }
}

/// The eye must be within this of a touching teammate's body in height,
/// metres, for him to block: one a floor below or above is not in the way
/// of a level shot.
pub const TOUCH_HEIGHT_M: f32 = 1.0;

/// `me`'s living, present teammates.
fn mates<'a>(me: &'a PlaceView, view: &'a MatchView) -> impl Iterator<Item = &'a PlaceView> {
    view.team(me.team)
        .filter(move |other| other.place != me.place && other.alive && other.present)
}

/// Which side of `me` `other` stands on, seen from above: his offset across
/// `me`'s view, to the right.
fn side(me: &PlaceView, other: &PlaceView) -> f32 {
    let [rx, rz] = forward(me.yaw - std::f32::consts::FRAC_PI_2);
    let to = other.feet - me.feet;
    to.x * rx + to.z * rz
}

/// `other` against each line of `dirs`, where the view has him and where
/// his velocity takes him in `ahead_s` (half way and all the way): one
/// [`Near`] for each line and place where his nearest approach is ahead of
/// the eye, or where he touches.
fn assess(
    me: &PlaceView,
    other: &PlaceView,
    dirs: &[Vec3],
    distance: f32,
    spread: f32,
    ahead_s: f32,
    t: &FriendlyFireTuning,
) -> Vec<Near> {
    let eye = me.eye();
    let reach = distance + t.past_target_m;
    let grow = spread.tan();
    let (low, high) = if other.crouched() {
        BODY_CROUCHED
    } else {
        BODY_STANDING
    };
    let width = t.body_m + if other.bot { 0.0 } else { t.human_m };
    let [vx, vz] = other.velocity;
    let lead = Vec3::new(vx, 0.0, vz) * ahead_s.max(0.0);
    let leads: &[f32] = if lead.length() > 0.05 {
        &[0.0, 0.5, 1.0]
    } else {
        &[0.0]
    };
    let mut out = Vec::new();
    for &dir in dirs {
        for &share in leads {
            let feet = other.feet + lead * share;
            let foot = feet + Vec3::new(0.0, low, 0.0);
            let head = feet + Vec3::new(0.0, high, 0.0);
            let along = ray_to_segment(eye, dir, reach, foot, head);
            let touch = touching(me, eye, dir, feet, (foot.y, head.y), width, t.touch_m);
            if along.is_none() && !touch {
                continue;
            }
            let apart = feet.distance_xz(me.feet);
            let (s, gap) = match along {
                Some((s, gap)) if touch => (s.min(apart), Some(gap)),
                Some((s, gap)) => (s, Some(gap)),
                None => (apart, None),
            };
            out.push(Near {
                who: other.place,
                feet,
                s,
                gap,
                margin: (width + grow * s).max(t.near_m),
                touch,
            });
        }
    }
    out
}

/// A teammate's body (`feet`, its height `span` and half `width`) within
/// `touch_m` of `me`'s feet, some of it in the front half of the line
/// `dir` seen from above, and level with the eye ([`TOUCH_HEIGHT_M`]).
fn touching(
    me: &PlaceView,
    eye: Vec3,
    dir: Vec3,
    feet: Vec3,
    span: (f32, f32),
    width: f32,
    touch_m: f32,
) -> bool {
    if touch_m <= 0.0 || feet.distance_xz(me.feet) > touch_m {
        return false;
    }
    if eye.y < span.0 - TOUCH_HEIGHT_M || eye.y > span.1 + TOUCH_HEIGHT_M {
        return false;
    }
    // The line's way over the ground; a line near straight up or down
    // goes the view's way.
    let flat = dir.x.hypot(dir.z);
    let [hx, hz] = if flat > 0.1 {
        [dir.x / flat, dir.z / flat]
    } else {
        forward(me.yaw)
    };
    let to = feet - me.feet;
    to.x * hx + to.z * hz >= -width
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
            late_ms: 0,
            known_at: 0,
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
            grenades: Vec::new(),
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
            (0.0, 50.0, false),
        ] {
            let mate = body(2, 0, Vec3::new(x, 0.0, z), true);
            let v = view(vec![me.clone(), enemy.clone(), mate]);
            let got = blocking(&me, &v, &[dir], 30.0, spread, 0.0, &t);
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
        assert_eq!(blocking(&me, &v, &[dir], 30.0, 0.05, 0.0, &t), None);
    }

    #[test]
    fn a_human_teammate_gets_a_wider_berth() {
        let t = FriendlyFireTuning::default();
        let me = body(0, 0, Vec3::ZERO, true);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 30.0), true);
        let dir = line_to(&me, enemy.chest());
        // 0.7 m off the line 15 m on, a bolt action's thin spread.
        let spread = t.spread_deg.bolt.to_radians();
        let near = Vec3::new(0.7, 0.0, 15.0);
        let bot = view(vec![me.clone(), enemy.clone(), body(2, 0, near, true)]);
        let human = view(vec![me.clone(), enemy, body(2, 0, near, false)]);
        assert_eq!(blocking(&me, &bot, &[dir], 30.0, spread, 0.0, &t), None);
        assert!(blocking(&me, &human, &[dir], 30.0, spread, 0.0, &t).is_some());
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
        let away = |v: &MatchView| blocking(&me, v, &[dir], 30.0, 0.05, 0.0, &t).map(|b| b.away);
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
        assert_eq!(blocking(&me, &v, &[dir], 23.0, 0.03, 0.0, &t), None);
    }

    #[test]
    fn near_the_shooter_the_margin_is_at_least_near_m() {
        let t = FriendlyFireTuning::default();
        let me = body(0, 0, Vec3::ZERO, true);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 30.0), true);
        let dir = line_to(&me, enemy.chest());
        let spread = t.spread_deg.bolt.to_radians();
        // 0.5 m off the line 3 m on: the body and the bolt's spread alone
        // (0.39 m) let it pass; the floor does not.
        let mate = view(vec![
            me.clone(),
            enemy,
            body(2, 0, Vec3::new(0.5, 0.0, 3.0), true),
        ]);
        assert!(blocking(&me, &mate, &[dir], 30.0, spread, 0.0, &t).is_some());
        let off = FriendlyFireTuning { near_m: 0.0, ..t };
        assert_eq!(blocking(&me, &mate, &[dir], 30.0, spread, 0.0, &off), None);
    }

    /// 6da44bbb at 437.95 s: the owner 0.86 m off a bot's line 2 m on,
    /// walking toward it. Where the view has him he is clear; a tenth of a
    /// second on, at the tick decided, he is in it.
    #[test]
    fn a_teammate_walking_into_the_line_is_taken_where_he_will_be() {
        let t = FriendlyFireTuning::default();
        let me = body(0, 0, Vec3::ZERO, true);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 14.0), true);
        let dir = line_to(&me, enemy.chest());
        let mut mate = body(2, 0, Vec3::new(0.9, 0.0, 2.0), false);
        mate.velocity = [-4.0, 0.0];
        let v = view(vec![me.clone(), enemy.clone(), mate.clone()]);
        assert_eq!(blocking(&me, &v, &[dir], 14.0, 0.05, 0.0, &t), None);
        assert_eq!(
            blocking(&me, &v, &[dir], 14.0, 0.05, 0.1, &t).map(|b| b.who),
            Some(2)
        );
        // Walking away from the line: clear either way.
        mate.velocity = [4.0, 0.0];
        let v = view(vec![me.clone(), enemy, mate]);
        assert_eq!(blocking(&me, &v, &[dir], 14.0, 0.05, 0.1, &t), None);
    }

    /// The touch rule off: the gate as it was before it.
    fn untouched(t: FriendlyFireTuning) -> FriendlyFireTuning {
        FriendlyFireTuning { touch_m: 0.0, ..t }
    }

    /// Match 0x31632b5a ("match 7") at 481.1 s: bot 15, crouched and
    /// backing off, fired at an enemy 9.7 m beyond the owner, who stood
    /// 0.54 m from it, 35 degrees off its view and 0.35 m higher; the bot
    /// reckoned itself 0.45 m off (its trace against the per-tick
    /// positions). Seen right the line passes 0.31 m from him and both
    /// gates hold; where the bot had itself, a little past him along its
    /// view, he stands beside its eye, the line's nearest approach is
    /// behind it and only the touch rule holds.
    #[test]
    fn the_owner_beside_a_bot_that_reckons_itself_past_him_holds_its_fire() {
        let t = FriendlyFireTuning::default();
        let mut me = body(0, 0, Vec3::ZERO, true);
        me.keys = key::CROUCH;
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 9.7), true);
        let dir = line_to(&me, enemy.chest());
        let spread = t.spread_deg.auto.to_radians();
        // Yaw 0 faces +z: 0.44 m on, 0.31 m to the side, 0.35 m up.
        let truly = body(2, 0, Vec3::new(-0.31, 0.35, 0.44), false);
        let v = view(vec![me.clone(), enemy.clone(), truly]);
        assert!(blocking(&me, &v, &[dir], 9.7, spread, 0.0, &t).is_some());
        assert!(blocking(&me, &v, &[dir], 9.7, spread, 0.0, &untouched(t)).is_some());
        // The bot 0.45 m further along its view than it is: he is 0.01 m
        // behind its eye.
        let seen = body(2, 0, Vec3::new(-0.31, 0.35, -0.01), false);
        let v = view(vec![me.clone(), enemy, seen]);
        assert_eq!(
            blocking(&me, &v, &[dir], 9.7, spread, 0.0, &untouched(t)),
            None
        );
        assert_eq!(
            blocking(&me, &v, &[dir], 9.7, spread, 0.0, &t).map(|b| b.who),
            Some(2)
        );
    }

    /// Match 0x31632b5a at 643.3 s: bot 6 killed bot 8 1.2-1.5 m away,
    /// 20-24 degrees off its view (0.42-0.59 m off the line, inside the
    /// 0.63 m floor). The reckoning of bodies that touch is half a metre
    /// or more off: 0.5 m further to the side, he is 1 m off the line, and
    /// only the touch rule holds.
    #[test]
    fn a_teammate_a_step_off_the_line_at_arms_length_holds_the_fire() {
        let t = FriendlyFireTuning::default();
        let me = body(0, 0, Vec3::ZERO, true);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 26.3), true);
        let dir = line_to(&me, enemy.chest());
        let spread = t.spread_deg.auto.to_radians();
        for (x, z, old) in [(0.42, 1.16, true), (0.59, 1.35, true), (1.0, 1.2, false)] {
            let v = view(vec![
                me.clone(),
                enemy.clone(),
                body(2, 0, Vec3::new(x, 0.0, z), true),
            ]);
            assert_eq!(
                blocking(&me, &v, &[dir], 26.3, spread, 0.0, &untouched(t)).is_some(),
                old,
                "the gate before, a teammate at ({x}, {z})"
            );
            assert!(
                blocking(&me, &v, &[dir], 26.3, spread, 0.0, &t).is_some(),
                "at ({x}, {z})"
            );
        }
    }

    /// Match 0x5e7d0fc0 ("match 6") at 207.6 s: bots 8 and 4 stood in one
    /// another (0.02 m apart, five worlds running) and 8 fired up at the
    /// owner 20 m off, killing 4. A teammate in the shooter's own body
    /// blocks.
    #[test]
    fn a_teammate_standing_in_the_shooter_blocks_whatever_the_line() {
        let t = FriendlyFireTuning::default();
        let me = body(0, 0, Vec3::ZERO, true);
        let enemy = body(1, 1, Vec3::new(0.0, 8.8, 20.5), true);
        let dir = line_to(&me, enemy.chest());
        let mate = body(2, 0, Vec3::new(0.02, 0.0, -0.1), true);
        let v = view(vec![me.clone(), enemy, mate]);
        assert_eq!(
            blocking(&me, &v, &[dir], 22.0, 0.05, 0.0, &untouched(t)),
            None
        );
        assert!(blocking(&me, &v, &[dir], 22.0, 0.05, 0.0, &t).is_some());
    }

    #[test]
    fn the_touch_rule_holds_for_the_front_half_level_with_the_eye_and_near() {
        let t = FriendlyFireTuning::default();
        let me = body(0, 0, Vec3::ZERO, true);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 30.0), true);
        let dir = line_to(&me, enemy.chest());
        for (x, y, z, blocks, why) in [
            (1.9, 0.0, 0.3, true, "beside, 81 degrees off, 1.9 m"),
            (
                1.2,
                0.0,
                -0.3,
                true,
                "his body reaching into the front half",
            ),
            (0.8, 0.0, -0.8, false, "behind"),
            (2.0, 0.0, 1.5, false, "2.5 m off"),
            (1.2, -3.0, 0.5, false, "a floor below"),
            (1.2, 2.6, 0.5, false, "a floor above"),
            (1.2, -0.8, 0.5, true, "a step down"),
        ] {
            let v = view(vec![
                me.clone(),
                enemy.clone(),
                body(2, 0, Vec3::new(x, y, z), true),
            ]);
            assert_eq!(
                blocking(&me, &v, &[dir], 30.0, 0.0, 0.0, &t).is_some(),
                blocks,
                "{why}"
            );
        }
    }

    #[test]
    fn the_nearest_to_blocking_is_named_for_the_trace() {
        let t = FriendlyFireTuning::default();
        let me = body(0, 0, Vec3::ZERO, true);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 30.0), true);
        let dir = line_to(&me, enemy.chest());
        let spread = t.spread_deg.bolt.to_radians();
        // 1.5 m off the line 10 m on, and 3 m off it 20 m on: clear, the
        // first the nearer to blocking.
        let v = view(vec![
            me.clone(),
            enemy,
            body(2, 0, Vec3::new(3.0, 0.0, 20.0), true),
            body(3, 0, Vec3::new(1.5, 0.0, 10.0), true),
        ]);
        assert_eq!(blocking(&me, &v, &[dir], 30.0, spread, 0.0, &t), None);
        let near = closest(&me, &v, &[dir], 30.0, spread, 0.0, &t).unwrap();
        assert_eq!(near.who, 3);
        assert!(!near.blocks() && !near.touch);
        assert!((near.gap.unwrap() - 1.5).abs() < 0.05, "{near:?}");
        assert!(near.room() > 0.5 && near.room() < 1.0, "{near:?}");
    }

    /// Match 0x60aeff4f at 556 s: bot 1 fired at enemy 15 11 m off and
    /// killed him and bot 2, who stood on the line 3.5 m behind him; at
    /// 239 s bot 14 fired at enemy 13 5.8 m off and killed bot 18 on the
    /// line 11.5 m behind him. The misses go on, and the rounds after a
    /// kill.
    #[test]
    fn a_teammate_on_the_line_behind_the_target_blocks() {
        let t = FriendlyFireTuning::default();
        let short = FriendlyFireTuning {
            past_target_m: 2.0,
            ..t
        };
        let me = body(0, 0, Vec3::ZERO, true);
        let spread = t.spread_deg.auto.to_radians();
        for (target, x, z) in [(11.0, 0.3, 14.5), (5.8, -0.22, 17.3)] {
            let enemy = body(1, 1, Vec3::new(0.0, 0.0, target), true);
            let dir = line_to(&me, enemy.chest());
            let v = view(vec![
                me.clone(),
                enemy,
                body(2, 0, Vec3::new(x, 0.0, z), true),
            ]);
            assert_eq!(blocking(&me, &v, &[dir], target, spread, 0.0, &short), None);
            assert!(
                blocking(&me, &v, &[dir], target, spread, 0.0, &t).is_some(),
                "a teammate {:.1} m past the target",
                z - target
            );
        }
    }
}
