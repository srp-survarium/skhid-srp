//! The stand-in human's smoke habit: a live player throws a smoke now and then, and the bots have to meet the cloud. The stand-in human
//! (`SRP_BENCH_HUMAN`) is a bot place, which the tracker reads as a bot (only a frag key throws for it), so its smokes are put in as
//! clouds ([`Tracker::add_cloud`]) from the live players' numbers (`fx-smoke-data.md` §5, `smoke_use.py`: the 402 live recordings, 39,547
//! lives, LIKELY unless said):
//!
//! - **how often:** 2.97 throws per 100 lives: a life draws once, at its spawn, at that rate over [`COMPLETION`] (a third of the draws end
//!   with the life before the press can be made).
//! - **when:** [`SINCE_SPAWN_S`] after the spawn (P10 7 s, median 27 s, P90 97 s), at the first tick the nearest enemy is within
//!   [`NEAREST_MAX_M`] (live P90 60 m) and is one the thrower has seen or heard within 15 s (the fairness rule of BENCH-DECISIONS D: the
//!   recordings have no sight data).
//! - **where:** the rest point is [`REST_M`] from the thrower (median 10 m; 40 % within 8 m): 68.9 % on the line to the nearest enemy and
//!   closer than he is (a screen), 6.2 % on him (within 8 m), 25 % in another direction.
//! - **the cloud** is `SmokeThrow::cloud_at`'s: the game data's smoke grenade, centred on the floor under the point. The thrower's own arc
//!   is not modelled (the cloud is put where the live throws were counted to land).
//!
//! Default on with a stand-in human; `SRP_BENCH_HUMAN_SMOKE=off` leaves the human with no smoke.

use mapdata::Vec3;

use super::*;
use crate::v026g::bots::smoke::{MatchMs, SmokeGrenade, SmokeThrow, floor_under};

/// Live throws per life (2.97 per 100).
pub(crate) const THROW_PER_LIFE: f32 = 0.0297;
/// The share of the lives that draw a throw and make it: the rest end before an enemy is near and fresh at or after the press time (30
/// of 45 draws in 1,508 lives of 112 matches, 2026-10-29). A life draws at [`THROW_PER_LIFE`] over this so that the throws are the live rate.
pub(crate) const COMPLETION: f32 = 0.67;
/// The shares of the throws by where the rest point is: on the line to the nearest enemy and closer, on him; the rest elsewhere.
pub(crate) const BETWEEN_SHARE: f32 = 0.689;
pub(crate) const ON_HIM_SHARE: f32 = 0.062;
/// Seconds from the spawn to the press: (fraction of throws, value), linear between; the ends are GUESSES.
const SINCE_SPAWN_S: [(f32, f32); 7] = [(0.0, 1.0), (0.10, 7.2), (0.25, 12.8), (0.5, 26.8), (0.75, 50.8), (0.9, 96.5), (1.0, 150.0)];
/// Metres from the thrower to the rest point, the same way (the ends are GUESSES).
const REST_M: [(f32, f32); 7] = [(0.0, 0.5), (0.10, 1.8), (0.25, 4.2), (0.5, 10.4), (0.75, 18.7), (0.9, 24.4), (1.0, 30.0)];
/// The nearest enemy is within this of the thrower for the throw, m (the live P90).
const NEAREST_MAX_M: f32 = 60.0;
/// ... and at least this far, m: the live P10 is 14 m, a smoke is thrown at range and not in a close fight (the nearest enemy was
/// firing at the press in 0.2 % of the live throws). Without it a near enemy caps the rest distance below the live one.
const NEAREST_MIN_M: f32 = 14.0;
/// A throw "on him" rests within this of the enemy, m.
const ON_HIM_M: f32 = 8.0;

/// The value at a drawn fraction of a table of (fraction, value) points.
fn at_fraction(table: &[(f32, f32)], p: f32) -> f32 {
    let k = table.iter().position(|(q, _)| *q >= p).unwrap_or(table.len() - 1).max(1);
    let ((q0, v0), (q1, v1)) = (table[k - 1], table[k]);
    if q1 > q0 { v0 + (v1 - v0) * ((p - q0) / (q1 - q0)).clamp(0.0, 1.0) } else { v1 }
}

/// Where the human is in his life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Habit {
    /// No life seen yet, or this life throws no smoke.
    Idle,
    /// This life will throw: at or after `from`, the first tick an enemy is near enough.
    Waiting { from: u32 },
}

/// What the habit counted.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SmokeHabitCounts {
    pub(crate) lives: u32,
    pub(crate) planned: u32,
    pub(crate) thrown: u32,
    pub(crate) between: u32,
    pub(crate) on_him: u32,
    /// Seconds from the spawn to each press, and metres from the thrower to each rest point.
    pub(crate) press_s: Vec<f32>,
    pub(crate) rest_m: Vec<f32>,
    /// Metres from the thrower to the nearest enemy at each press.
    pub(crate) enemy_m: Vec<f32>,
}

pub(crate) struct SmokeHabit {
    place: Place,
    rng: Rng,
    habit: Habit,
    /// The `spawned` tick of the life the habit has seen.
    life: Option<u32>,
    pub(crate) counts: SmokeHabitCounts,
}

impl SmokeHabit {
    /// The habit of the stand-in human at `place`, unless `SRP_BENCH_HUMAN_SMOKE=off`.
    pub(crate) fn start(place: Place, seed: u32, level: u8, mode: u8, per_team: usize) -> Option<Self> {
        if bench_settings().var("SRP_BENCH_HUMAN_SMOKE").is_ok_and(|v| v == "off") {
            return None;
        }
        Some(Self { place, rng: Rng::new(Self::seed_of(seed, level, mode, per_team)), habit: Habit::Idle, life: None, counts: SmokeHabitCounts::default() })
    }

    /// The habit's random seed: the bench seed and the map, the mode and the size, so that two cells of one seed on two maps do not
    /// draw the same lives (they did: a batch of 4 maps x 28 seeds was 28 draws repeated, not 112).
    fn seed_of(seed: u32, level: u8, mode: u8, per_team: usize) -> u64 {
        0x5_B0CE + u64::from(seed) + (u64::from(level) << 40) + (u64::from(mode) << 48) + ((per_team as u64) << 56)
    }

    /// One tick: a new life draws whether it throws; a waiting life throws when the nearest enemy is near enough and fresh in the
    /// thrower's memory (`fresh(enemy)`: seen or heard, his team's own senses, within 15 s).
    pub(crate) fn tick(&mut self, game: &Game, tracker: &mut Tracker, tick: u32, fresh: impl Fn(Place) -> bool) {
        let Some(body) = game.bodies.get(self.place) else {
            return;
        };
        if !body.alive {
            self.habit = Habit::Idle;
            return;
        }
        if self.life != Some(body.spawned) {
            self.life = Some(body.spawned);
            self.counts.lives += 1;
            self.habit = if self.rng.unit() < THROW_PER_LIFE / COMPLETION {
                self.counts.planned += 1;
                let after_s = at_fraction(&SINCE_SPAWN_S, self.rng.unit());
                Habit::Waiting { from: body.spawned + (after_s * 1000.0) as u32 }
            } else {
                Habit::Idle
            };
        }
        let Habit::Waiting { from } = self.habit else {
            return;
        };
        if tick < from {
            return;
        }
        let nearest = game
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.alive && b.team != body.team)
            .map(|(place, b)| (place, b.feet, (b.feet - body.feet).length()))
            .min_by(|a, b| a.2.total_cmp(&b.2));
        let Some((_, enemy, to_enemy)) = nearest.filter(|(place, _, d)| (NEAREST_MIN_M..=NEAREST_MAX_M).contains(d) && fresh(*place)) else {
            return;
        };
        let draw = self.rng.unit();
        let reach = at_fraction(&REST_M, self.rng.unit());
        let (rest, between, on_him) = if draw < BETWEEN_SHARE {
            // On the line to him and closer than he is.
            (body.feet + (enemy - body.feet) * (reach.min(0.9 * to_enemy) / to_enemy.max(0.1)), true, false)
        } else if draw < BETWEEN_SHARE + ON_HIM_SHARE {
            let angle = self.rng.range(0.0, std::f32::consts::TAU);
            let r = self.rng.range(0.0, ON_HIM_M);
            (enemy + Vec3::new(r * angle.cos(), 0.0, r * angle.sin()), false, true)
        } else {
            let angle = self.rng.range(0.0, std::f32::consts::TAU);
            (body.feet + Vec3::new(reach * angle.cos(), 0.0, reach * angle.sin()), false, false)
        };
        let throw =
            SmokeThrow { by: self.place, team: body.team, press: MatchMs(tick), grenade: *SmokeGrenade::of_game_data() };
        let cloud = throw.cloud_at(rest, |at| floor_under(&game.world.sight, at));
        tracker.add_cloud(cloud);
        self.counts.thrown += 1;
        self.counts.between += u32::from(between);
        self.counts.on_him += u32::from(on_him);
        self.counts.press_s.push(tick.saturating_sub(body.spawned) as f32 / 1000.0);
        self.counts.rest_m.push((rest - body.feet).length());
        self.counts.enemy_m.push(to_enemy);
        self.habit = Habit::Idle;
    }

    /// The stand-in human's place.
    pub(crate) fn place(&self) -> Place {
        self.place
    }

    /// The bench's line.
    pub(crate) fn line(&self) -> String {
        let c = &self.counts;
        let per_100 = if c.lives > 0 { 100.0 * f64::from(c.thrown) / f64::from(c.lives) } else { 0.0 };
        let quantiles = |values: &[f32]| -> String {
            let mut sorted = values.to_vec();
            sorted.sort_by(f32::total_cmp);
            let at = |p: f32| sorted.get(((sorted.len().saturating_sub(1)) as f32 * p).round() as usize).copied();
            format!("{:?}", [0.1, 0.5, 0.9].map(|p| at(p).map(|v| (v * 10.0).round() / 10.0)))
        };
        format!(
            "smoke habit (stand-in human): {} lives, {} drawn, {} thrown = {per_100:.2} a 100 lives (live 2.97), {} on the line to the nearest enemy (live 68.9 %), {} on him (live 6.2 %); press s from spawn P10/median/P90 {} (live 7.2/26.8/96.5); rest m from the thrower {} (live 1.8/10.4/24.4); nearest enemy m {} (live 14/32/60); press {:?}; rest {:?}; enemy {:?}",
            c.lives,
            c.planned,
            c.thrown,
            c.between,
            c.on_him,
            quantiles(&c.press_s),
            quantiles(&c.rest_m),
            quantiles(&c.enemy_m),
            c.press_s.iter().map(|v| (v * 10.0).round() / 10.0).collect::<Vec<_>>(),
            c.rest_m.iter().map(|v| (v * 10.0).round() / 10.0).collect::<Vec<_>>(),
            c.enemy_m.iter().map(|v| (v * 10.0).round() / 10.0).collect::<Vec<_>>(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drawn fraction reads its value off the table, linear between the points.
    #[test]
    fn a_fraction_reads_the_live_table() {
        assert!((at_fraction(&REST_M, 0.5) - 10.4).abs() < 1e-4);
        assert!((at_fraction(&REST_M, 0.0) - 0.5).abs() < 1e-4);
        assert!((at_fraction(&REST_M, 1.0) - 30.0).abs() < 1e-4);
        assert!((at_fraction(&SINCE_SPAWN_S, 0.175) - 10.0).abs() < 1e-3);
    }

    /// A planned life puts one cloud once the nearest enemy is near and fresh in memory, not before; a stale enemy is no reason.
    #[test]
    fn a_planned_life_puts_one_cloud_when_the_nearest_enemy_is_fresh() {
        let mut game = Game::new(super::super::soak_world(), &[0, 1], 1, None, None);
        let mut tracker = Tracker::new(2, &[0, 1], &[true, true], vec![]);
        let mut habit = SmokeHabit {
            place: 0,
            rng: Rng::new(7),
            habit: Habit::Waiting { from: 5_000 },
            life: Some(0),
            counts: SmokeHabitCounts::default(),
        };
        game.bodies[0].feet = Vec3::new(10.0, 0.0, 10.0);
        game.bodies[1].feet = Vec3::new(40.0, 0.0, 10.0);
        habit.tick(&game, &mut tracker, 1_000, |_| true);
        assert!(tracker.clouds().is_empty(), "not before the time");
        habit.tick(&game, &mut tracker, 6_000, |_| false);
        assert!(tracker.clouds().is_empty(), "an enemy he has not seen or heard is no reason");
        habit.tick(&game, &mut tracker, 6_050, |_| true);
        habit.tick(&game, &mut tracker, 6_100, |_| true);
        assert_eq!(tracker.clouds().len(), 1, "one throw a life");
        assert_eq!(habit.counts.thrown, 1);
    }

    /// Over many throws the placement shares are the live ones: 68.9 % on the line, 6.2 % on him, the rest elsewhere.
    #[test]
    fn the_placements_follow_the_live_shares() {
        let mut game = Game::new(super::super::soak_world(), &[0, 1], 1, None, None);
        let mut tracker = Tracker::new(2, &[0, 1], &[true, true], vec![]);
        game.bodies[0].feet = Vec3::new(10.0, 0.0, 10.0);
        game.bodies[1].feet = Vec3::new(40.0, 0.0, 10.0);
        let mut habit = SmokeHabit {
            place: 0,
            rng: Rng::new(99),
            habit: Habit::Idle,
            life: Some(0),
            counts: SmokeHabitCounts::default(),
        };
        let throws = 4_000;
        for _ in 0..throws {
            habit.habit = Habit::Waiting { from: 0 };
            habit.tick(&game, &mut tracker, 6_000, |_| true);
        }
        let c = &habit.counts;
        let share = |n: u32| f64::from(n) / f64::from(c.thrown);
        assert_eq!(c.thrown, throws);
        assert!((share(c.between) - 0.689).abs() < 0.03, "on the line {}", share(c.between));
        assert!((share(c.on_him) - 0.062).abs() < 0.02, "on him {}", share(c.on_him));
        // With the enemy far (30 m) the rest distances are the live ones (P10 1.8, median 10.4, P90 24.4 m), not capped by him.
        let mut rest = c.rest_m.clone();
        rest.sort_by(f32::total_cmp);
        let at = |p: f32| rest[((rest.len() - 1) as f32 * p) as usize];
        assert!(at(0.5) > 8.0 && at(0.5) < 20.0, "median {}", at(0.5));
        assert!(at(0.1) < 4.0, "P10 {}", at(0.1));
    }

    /// A nearest enemy closer than the live range or farther than 60 m is no reason to throw.
    #[test]
    fn an_enemy_inside_the_live_range_only_is_a_reason() {
        let mut game = Game::new(super::super::soak_world(), &[0, 1], 1, None, None);
        let mut tracker = Tracker::new(2, &[0, 1], &[true, true], vec![]);
        game.bodies[0].feet = Vec3::new(10.0, 0.0, 10.0);
        let mut habit = SmokeHabit { place: 0, rng: Rng::new(5), habit: Habit::Idle, life: Some(0), counts: SmokeHabitCounts::default() };
        for x in [20.0, 80.0] {
            game.bodies[1].feet = Vec3::new(x, 0.0, 10.0);
            habit.habit = Habit::Waiting { from: 0 };
            habit.tick(&game, &mut tracker, 6_000, |_| true);
            assert_eq!(habit.counts.thrown, 0, "an enemy {} m off", x - 10.0);
        }
        game.bodies[1].feet = Vec3::new(40.0, 0.0, 10.0);
        habit.tick(&game, &mut tracker, 6_050, |_| true);
        assert_eq!(habit.counts.thrown, 1);
    }

    /// Two cells of one seed on different maps or modes draw different lives.
    #[test]
    fn the_habit_draws_differently_on_another_map() {
        let seeds = [(1, 8, 2, 8), (1, 2, 2, 8), (1, 8, 4, 7), (1, 8, 2, 4), (2, 8, 2, 8)].map(|(s, l, m, p)| SmokeHabit::seed_of(s, l, m, p));
        for (i, a) in seeds.iter().enumerate() {
            assert!(seeds[i + 1..].iter().all(|b| a != b), "{seeds:?}");
        }
    }
}
