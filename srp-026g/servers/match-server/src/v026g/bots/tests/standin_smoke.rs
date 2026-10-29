//! The stand-in human's smoke habit: a live player throws a smoke now and then, and the bots have to meet the cloud. The stand-in human
//! (`SRP_BENCH_HUMAN`) is a bot place, which the tracker reads as a bot (only a frag key throws for it), so its smokes are put in as
//! clouds ([`Tracker::add_cloud`]) from the live players' numbers (`fx-smoke-data.md` §5, `smoke_use.py`: the 402 live recordings, 39,547
//! lives, LIKELY unless said):
//!
//! - **how often:** 2.97 throws per 100 lives: a life draws once, at its spawn, at that rate over [`COMPLETION`] (the share of the draws
//!   that end in a throw; the rest end with the life before the press can be made).
//! - **when:** [`SINCE_SPAWN_S`] after the spawn (P10 7 s, median 27 s, P90 97 s), at the first tick the nearest enemy is at the distance
//!   the life drew from [`NEAREST_M`] (live P10 14 m, median 32 m, P90 60 m: a smoke is thrown at range), within [`BAND_M`], and is one
//!   the thrower has seen or heard within 15 s (the fairness rule of BENCH-DECISIONS D: the recordings have no sight data).
//! - **where:** the rest point is [`REST_M`] from the thrower (median 10 m; 40 % within 8 m) and never past the throw's reach (the
//!   smoke's 17 m/s, `v^2 / g` = 29 m): about 69 % on the line to the nearest enemy and closer than he is (a screen), 6 % on him (within
//!   8 m), the rest in another direction. A throw draws its class, then a point that the census's own test ([`Placement::of`]) scores as
//!   that class; the habit counts the scored shares of the rest points reached.
//! - **the cloud** is `SmokeThrow::cloud_at`'s: the game data's smoke grenade, centred on the floor under the point. The thrower's own arc
//!   is not modelled (the cloud is put where the live throws were counted to land).
//!
//! Default on with a stand-in human; `SRP_BENCH_HUMAN_SMOKE=off` leaves the human with no smoke.

use mapdata::Vec3;

use super::*;
use crate::v026g::bots::smoke::{MatchMs, SmokeGrenade, SmokeThrow, floor_under};

/// Live throws per life (2.97 per 100).
pub(crate) const THROW_PER_LIFE: f32 = 0.0297;
/// The share of the lives that draw a throw and make it: the rest end before the nearest enemy is at the drawn distance and fresh at or
/// after the press time. A life draws at [`THROW_PER_LIFE`] over this so that the throws are the live rate. Measured by the rate batch
/// (thrown over drawn), re-measured whenever the rule of "when" changes (see the PR body for the batch).
pub(crate) const COMPLETION: f32 = 0.67;
/// The census's shares of the throws by [`Placement::of`]: between him and the thrower, on him; the rest elsewhere. A throw draws its
/// class from them and then a point that the same test scores as that class, so the scored shares are these.
pub(crate) const BETWEEN_SHARE: f32 = 0.689;
pub(crate) const ON_HIM_SHARE: f32 = 0.062;
/// Seconds from the spawn to the press: (fraction of throws, value), linear between; the ends are GUESSES.
const SINCE_SPAWN_S: [(f32, f32); 7] = [(0.0, 1.0), (0.10, 7.2), (0.25, 12.8), (0.5, 26.8), (0.75, 50.8), (0.9, 96.5), (1.0, 150.0)];
/// Metres from the thrower to the rest point, the same way (the ends are GUESSES).
const REST_M: [(f32, f32); 7] = [(0.0, 0.5), (0.10, 1.8), (0.25, 4.2), (0.5, 10.4), (0.75, 18.7), (0.9, 24.4), (1.0, 30.0)];
/// Metres from the thrower to the nearest enemy at the press (live P10 14.0, P25 22.1, median 32.3, P75 45.1, P90 60.0; the ends are
/// GUESSES).
const NEAREST_M: [(f32, f32); 7] = [(0.0, 6.0), (0.10, 14.0), (0.25, 22.1), (0.5, 32.3), (0.75, 45.1), (0.9, 60.0), (1.0, 75.0)];
/// The nearest enemy is at the drawn distance within this, m.
const BAND_M: f32 = 4.0;
/// A throw "on him" rests within this of the enemy, m (the census's test).
const ON_HIM_M: f32 = 8.0;
/// A throw "between" rests within this of the thrower-enemy segment, m (the census's cylinder).
const CYLINDER_M: f32 = 8.0;
/// Gravity, m/s^2, for the throw's reach.
const G: f32 = 9.81;

/// The farthest a throw lands on level ground (45 degrees), m: `v^2 / g` of the smoke grenade's throw speed.
fn reach_max_m() -> f32 {
    SmokeGrenade::of_game_data().flight.speed.powi(2) / G
}

/// Where a throw rested against the nearest enemy, by the census's test: between the thrower and him (nearer the thrower than he is and
/// inside the 8 m cylinder on the segment), on him (within 8 m of him), else elsewhere. A rest that is both is between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Placement {
    Between,
    OnHim,
    Elsewhere,
}

impl Placement {
    pub(crate) fn of(thrower: Vec3, enemy: Vec3, rest: Vec3) -> Self {
        let (to_rest, to_enemy) = (rest - thrower, enemy - thrower);
        let along = to_enemy.length().max(1e-3);
        let unit = to_enemy * (1.0 / along);
        let t = to_rest.dot(unit);
        let lateral = (to_rest - unit * t).length();
        if to_rest.length() < along && t > 0.0 && lateral <= CYLINDER_M {
            Placement::Between
        } else if (rest - enemy).length() <= ON_HIM_M {
            Placement::OnHim
        } else {
            Placement::Elsewhere
        }
    }
}

/// The value at a drawn fraction of a table of (fraction, value) points.
fn at_fraction(table: &[(f32, f32)], p: f32) -> f32 {
    let k = table.iter().position(|(q, _)| *q >= p).unwrap_or(table.len() - 1).max(1);
    let ((q0, v0), (q1, v1)) = (table[k - 1], table[k]);
    if q1 > q0 { v0 + (v1 - v0) * ((p - q0) / (q1 - q0)).clamp(0.0, 1.0) } else { v1 }
}

/// The fraction of a table's draws that are below `value` (the inverse of [`at_fraction`]).
fn fraction_below(table: &[(f32, f32)], value: f32) -> f32 {
    let k = table.iter().position(|(_, v)| *v >= value).unwrap_or(table.len() - 1).max(1);
    let ((q0, v0), (q1, v1)) = (table[k - 1], table[k]);
    if v1 > v0 { (q0 + (q1 - q0) * ((value - v0) / (v1 - v0))).clamp(0.0, 1.0) } else { q1 }
}

/// Where the human is in his life.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Habit {
    /// No life seen yet, or this life throws no smoke.
    Idle,
    /// This life will throw: at or after `from`, the first tick the nearest enemy is `want_m` off (within [`BAND_M`]).
    Waiting { from: u32, want_m: f32 },
}

/// What the habit counted.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SmokeHabitCounts {
    pub(crate) lives: u32,
    pub(crate) planned: u32,
    pub(crate) thrown: u32,
    /// By [`Placement::of`] on the rest point reached: between, on him, elsewhere.
    pub(crate) between: u32,
    pub(crate) on_him: u32,
    pub(crate) elsewhere: u32,
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

    /// One tick: a new life draws whether it throws, and when and at what range; a waiting life throws when the nearest enemy is at that
    /// range and fresh in the thrower's memory (`fresh(enemy)`: seen or heard, his team's own senses, within 15 s).
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
                let want_m = at_fraction(&NEAREST_M, self.rng.unit());
                Habit::Waiting { from: body.spawned + (after_s * 1000.0) as u32, want_m }
            } else {
                Habit::Idle
            };
        }
        let Habit::Waiting { from, want_m } = self.habit else {
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
        let Some((_, enemy, to_enemy)) = nearest.filter(|(place, _, _)| fresh(*place)) else {
            return;
        };
        if (to_enemy - want_m).abs() > BAND_M {
            return;
        }
        let rest = self.rest_point(body.feet, enemy, to_enemy);
        let throw = SmokeThrow { by: self.place, team: body.team, press: MatchMs(tick), grenade: *SmokeGrenade::of_game_data() };
        let cloud = throw.cloud_at(rest, |at| floor_under(&game.world.sight, at));
        tracker.add_cloud(cloud);
        self.counts.thrown += 1;
        match Placement::of(body.feet, enemy, rest) {
            Placement::Between => self.counts.between += 1,
            Placement::OnHim => self.counts.on_him += 1,
            Placement::Elsewhere => self.counts.elsewhere += 1,
        }
        self.counts.press_s.push(tick.saturating_sub(body.spawned) as f32 / 1000.0);
        self.counts.rest_m.push((rest - body.feet).length());
        self.counts.enemy_m.push(to_enemy);
        self.habit = Habit::Idle;
    }

    /// Where this throw rests: the census's class is drawn first (live shares), then a point that [`Placement::of`] scores as that class,
    /// at a distance from the thrower drawn from [`REST_M`] and never past the throw's reach. A class with no such point (an enemy past
    /// the reach cannot be hit "on him") falls to the next class that has one, so the scored shares are the live ones where they can be.
    fn rest_point(&mut self, feet: Vec3, enemy: Vec3, to_enemy: f32) -> Vec3 {
        const TRIES: usize = 24;
        let reach_max = reach_max_m();
        // A rest on him is beside or past him and within the reach, so only an enemy inside the reach can have one: his chance there is
        // the live share over the share of throws whose enemy is that near, which keeps the whole at the live 6.2 %.
        let near = fraction_below(&NEAREST_M, reach_max - ON_HIM_M / 2.0);
        let on_him = if to_enemy < reach_max - ON_HIM_M / 2.0 { ON_HIM_SHARE / near } else { 0.0 };
        let draw = self.rng.unit();
        let class = if draw < on_him {
            Placement::OnHim
        } else if self.rng.unit() < BETWEEN_SHARE / (1.0 - ON_HIM_SHARE) {
            Placement::Between
        } else {
            Placement::Elsewhere
        };
        let reach = at_fraction(&REST_M, self.rng.unit()).min(reach_max);
        let sample = |class: Placement, rng: &mut Rng| -> Vec3 {
            match class {
                // On the line to him, closer than he is.
                Placement::Between => feet + (enemy - feet) * (reach.min(0.9 * to_enemy) / to_enemy.max(0.1)),
                // Within 8 m of him, no farther than the throw reaches.
                Placement::OnHim => {
                    let angle = rng.range(0.0, std::f32::consts::TAU);
                    let r = rng.range(0.0, ON_HIM_M);
                    let point = enemy + Vec3::new(r * angle.cos(), 0.0, r * angle.sin());
                    let away = point - feet;
                    if away.length() > reach_max { feet + away * (reach_max / away.length()) } else { point }
                }
                // In another direction, at the drawn distance.
                Placement::Elsewhere => {
                    let angle = rng.range(0.0, std::f32::consts::TAU);
                    feet + Vec3::new(reach * angle.cos(), 0.0, reach * angle.sin())
                }
            }
        };
        let order = match class {
            Placement::Between => [Placement::Between, Placement::Elsewhere, Placement::OnHim],
            Placement::OnHim => [Placement::OnHim, Placement::Elsewhere, Placement::Between],
            Placement::Elsewhere => [Placement::Elsewhere, Placement::Between, Placement::OnHim],
        };
        for class in order {
            for _ in 0..TRIES {
                let point = sample(class, &mut self.rng);
                if Placement::of(feet, enemy, point) == class {
                    return point;
                }
            }
        }
        sample(Placement::Between, &mut self.rng)
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
            "smoke habit (stand-in human): {} lives, {} drawn, {} thrown = {per_100:.2} a 100 lives (live 2.97), scored by the census's test: {} between him and the thrower (live 68.9 %), {} on him (live 6.2 %), {} elsewhere (live 25.0 %); press s from spawn P10/median/P90 {} (live 7.2/26.8/96.5); rest m from the thrower {} (live 1.8/10.4/24.4); nearest enemy m {} (live 14/32.3/60); press {:?}; rest {:?}; enemy {:?}",
            c.lives,
            c.planned,
            c.thrown,
            c.between,
            c.on_him,
            c.elsewhere,
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

    fn habit_with(rng: u64, habit: Habit) -> SmokeHabit {
        SmokeHabit { place: 0, rng: Rng::new(rng), habit, life: Some(0), counts: SmokeHabitCounts::default() }
    }

    /// A drawn fraction reads its value off the table, linear between the points.
    #[test]
    fn a_fraction_reads_the_live_table() {
        assert!((at_fraction(&REST_M, 0.5) - 10.4).abs() < 1e-4);
        assert!((at_fraction(&REST_M, 0.0) - 0.5).abs() < 1e-4);
        assert!((at_fraction(&REST_M, 1.0) - 30.0).abs() < 1e-4);
        assert!((at_fraction(&SINCE_SPAWN_S, 0.175) - 10.0).abs() < 1e-3);
        assert!((at_fraction(&NEAREST_M, 0.5) - 32.3).abs() < 1e-4);
        assert!((at_fraction(&NEAREST_M, 0.1) - 14.0).abs() < 1e-4);
        assert!((at_fraction(&NEAREST_M, 0.9) - 60.0).abs() < 1e-4);
        assert!((fraction_below(&NEAREST_M, 32.3) - 0.5).abs() < 1e-4);
        assert!((fraction_below(&NEAREST_M, 14.0) - 0.1).abs() < 1e-4);
    }

    /// The census's placement test: between needs a rest nearer than the enemy and inside the cylinder; on him is within 8 m of him.
    #[test]
    fn the_censuss_test_places_a_rest() {
        let (me, enemy) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 0.0, 0.0));
        assert_eq!(Placement::of(me, enemy, Vec3::new(10.0, 0.0, 5.0)), Placement::Between);
        assert_eq!(Placement::of(me, enemy, Vec3::new(10.0, 0.0, 20.0)), Placement::Elsewhere, "outside the cylinder");
        assert_eq!(Placement::of(me, enemy, Vec3::new(-10.0, 0.0, 0.0)), Placement::Elsewhere, "behind the thrower");
        assert_eq!(Placement::of(me, enemy, Vec3::new(44.0, 0.0, 3.0)), Placement::OnHim);
        assert_eq!(Placement::of(me, enemy, Vec3::new(35.0, 0.0, 0.0)), Placement::Between, "near him but nearer than he is: between");
    }

    /// A planned life puts one cloud once the nearest enemy is at the drawn distance and fresh in memory, not before; a stale enemy is no
    /// reason; one at another distance is none.
    #[test]
    fn a_planned_life_puts_one_cloud_when_the_nearest_enemy_is_at_the_drawn_range_and_fresh() {
        let mut game = Game::new(super::super::soak_world(), &[0, 1], 1, None, None);
        let mut tracker = Tracker::new(2, &[0, 1], &[true, true], vec![]);
        let mut habit = habit_with(7, Habit::Waiting { from: 5_000, want_m: 30.0 });
        game.bodies[0].feet = Vec3::new(10.0, 0.0, 10.0);
        game.bodies[1].feet = Vec3::new(40.0, 0.0, 10.0);
        habit.tick(&game, &mut tracker, 1_000, |_| true);
        assert!(tracker.clouds().is_empty(), "not before the time");
        habit.tick(&game, &mut tracker, 6_000, |_| false);
        assert!(tracker.clouds().is_empty(), "an enemy he has not seen or heard is no reason");
        game.bodies[1].feet = Vec3::new(80.0, 0.0, 10.0);
        habit.tick(&game, &mut tracker, 6_025, |_| true);
        assert!(tracker.clouds().is_empty(), "an enemy 70 m off is not the drawn range");
        game.bodies[1].feet = Vec3::new(40.0, 0.0, 10.0);
        habit.tick(&game, &mut tracker, 6_050, |_| true);
        habit.tick(&game, &mut tracker, 6_100, |_| true);
        assert_eq!(tracker.clouds().len(), 1, "one throw a life");
        assert_eq!(habit.counts.thrown, 1);
    }

    /// Over many throws, with the nearest enemy at distances drawn from the live table, the placement shares scored by the census's
    /// test are the live ones (68.9 / 6.2 / 25.0 %); the rest distances are near the live ones (P10 1.8 m) and never past the throw's
    /// reach, on him included.
    #[test]
    fn the_placements_follow_the_live_shares() {
        let mut game = Game::new(super::super::soak_world(), &[0, 1], 1, None, None);
        let mut tracker = Tracker::new(2, &[0, 1], &[true, true], vec![]);
        game.bodies[0].feet = Vec3::new(10.0, 0.0, 10.0);
        let mut habit = habit_with(99, Habit::Idle);
        let mut distances = Rng::new(5);
        let throws = 6_000;
        for _ in 0..throws {
            let d = at_fraction(&NEAREST_M, distances.unit());
            game.bodies[1].feet = Vec3::new(10.0 + d, 0.0, 10.0);
            habit.habit = Habit::Waiting { from: 0, want_m: d };
            habit.tick(&game, &mut tracker, 6_000, |_| true);
        }
        let c = &habit.counts;
        let share = |n: u32| f64::from(n) / f64::from(c.thrown);
        assert_eq!(c.thrown, throws);
        assert_eq!(c.between + c.on_him + c.elsewhere, c.thrown);
        assert!((share(c.between) - 0.689).abs() < 0.03, "between {}", share(c.between));
        assert!((share(c.on_him) - 0.062).abs() < 0.02, "on him {}", share(c.on_him));
        assert!((share(c.elsewhere) - 0.249).abs() < 0.03, "elsewhere {}", share(c.elsewhere));
        let mut rest = c.rest_m.clone();
        rest.sort_by(f32::total_cmp);
        let at = |p: f32| rest[((rest.len() - 1) as f32 * p) as usize];
        assert!(at(0.5) > 7.0 && at(0.5) < 20.0, "median {}", at(0.5));
        assert!(at(0.1) < 4.0, "P10 {}", at(0.1));
        let farthest = rest.last().copied().unwrap_or(0.0);
        assert!(farthest <= reach_max_m() + 0.01, "the farthest rest {farthest}");
    }

    /// An enemy far past the throw's reach does not put the cloud out of reach: "on him" is cut to the reach.
    #[test]
    fn a_cloud_on_a_far_enemy_is_cut_to_the_throws_reach() {
        let mut game = Game::new(super::super::soak_world(), &[0, 1], 1, None, None);
        let mut tracker = Tracker::new(2, &[0, 1], &[true, true], vec![]);
        game.bodies[0].feet = Vec3::new(10.0, 0.0, 10.0);
        game.bodies[1].feet = Vec3::new(70.0, 0.0, 10.0);
        let mut habit = habit_with(3, Habit::Idle);
        for _ in 0..500 {
            habit.habit = Habit::Waiting { from: 0, want_m: 60.0 };
            habit.tick(&game, &mut tracker, 6_000, |_| true);
        }
        assert_eq!(habit.counts.thrown, 500);
        let farthest = habit.counts.rest_m.iter().copied().fold(0.0f32, f32::max);
        assert!(farthest <= reach_max_m() + 0.01, "{farthest}");
    }

    /// Lives draw their throw distances from the live table: over many planned lives the quantiles are the live ones.
    #[test]
    fn the_drawn_distances_follow_the_live_table() {
        let mut rng = Rng::new(11);
        let mut drawn: Vec<f32> = (0..5_000).map(|_| at_fraction(&NEAREST_M, rng.unit())).collect();
        drawn.sort_by(f32::total_cmp);
        let at = |p: f32| drawn[((drawn.len() - 1) as f32 * p) as usize];
        assert!((at(0.1) - 14.0).abs() < 2.0, "P10 {}", at(0.1));
        assert!((at(0.5) - 32.3).abs() < 2.0, "median {}", at(0.5));
        assert!((at(0.9) - 60.0).abs() < 2.0, "P90 {}", at(0.9));
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
