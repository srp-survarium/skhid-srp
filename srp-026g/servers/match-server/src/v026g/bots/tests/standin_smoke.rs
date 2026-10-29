//! The stand-in human's smoke habit: a live player throws a smoke now and then, and the bots have to meet the cloud. The stand-in human
//! (`SRP_BENCH_HUMAN`) is a bot place, which the tracker reads as a bot (only a frag key throws for it), so its smokes are put in as
//! clouds ([`Tracker::add_cloud`]) at the rate and the places of the live players':
//!
//! - **how often:** 2.97 throws per 100 lives (the 402-match census, `fx-smoke-data.md`): a life draws once, at its spawn; if it throws,
//!   it does so [`MIN_LIFE_MS`] or more into the life, the first tick an enemy is within [`NEAREST_MAX_M`] of the thrower.
//! - **where:** 69 % between the thrower and the nearest enemy (here the point [`BETWEEN_AT`] of the way), else near the thrower
//!   ([`ELSEWHERE_M`]) (GUESS for the point: the census gives the share, not the spot).
//! - **the cloud** is `SmokeThrow::cloud_at`'s: the game data's smoke grenade, centred on the floor under the point, forming after its
//!   fuse. The thrower's own arc is not modelled (the cloud is placed where the live throws were counted to land).
//!
//! Default on with a stand-in human; `SRP_BENCH_HUMAN_SMOKE=off` leaves the human with no smoke.

use mapdata::Vec3;

use super::*;
use crate::v026g::bots::smoke::{MatchMs, SmokeGrenade, SmokeThrow, floor_under};

/// Live throws per life (2.97 per 100).
pub(crate) const THROW_PER_LIFE: f32 = 0.0297;
/// The share of the throws that land between the thrower and the nearest enemy.
pub(crate) const BETWEEN_SHARE: f32 = 0.69;
/// A life throws no sooner than this after its spawn, ms (GUESS).
const MIN_LIFE_MS: u32 = 5_000;
/// The nearest enemy is within this of the thrower for the throw, m (GUESS).
const NEAREST_MAX_M: f32 = 40.0;
/// How far along the way to the nearest enemy a throw "between" lands (GUESS).
const BETWEEN_AT: f32 = 0.5;
/// A throw not between lands this far from the thrower at most, m (GUESS).
const ELSEWHERE_M: f32 = 12.0;

/// Where the human is in his life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Habit {
    /// No life seen yet, or this life throws no smoke.
    Idle,
    /// This life will throw: at or after `from`, the first tick an enemy is near enough.
    Waiting { from: u32 },
}

/// What the habit counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SmokeHabitCounts {
    pub(crate) lives: u32,
    pub(crate) planned: u32,
    pub(crate) thrown: u32,
    pub(crate) between: u32,
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
    pub(crate) fn start(place: Place, seed: u32) -> Option<Self> {
        if bench_settings().var("SRP_BENCH_HUMAN_SMOKE").is_ok_and(|v| v == "off") {
            return None;
        }
        Some(Self { place, rng: Rng::new(0x5_B0CE + u64::from(seed)), habit: Habit::Idle, life: None, counts: SmokeHabitCounts::default() })
    }

    /// One tick: a new life draws whether it throws; a waiting life throws when an enemy is near.
    pub(crate) fn tick(&mut self, game: &Game, tracker: &mut Tracker, tick: u32) {
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
            self.habit = if self.rng.unit() < THROW_PER_LIFE {
                self.counts.planned += 1;
                Habit::Waiting { from: body.spawned + MIN_LIFE_MS }
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
            .filter(|b| b.alive && b.team != body.team)
            .map(|b| (b.feet, (b.feet - body.feet).length()))
            .filter(|(_, d)| *d <= NEAREST_MAX_M)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((enemy, _)) = nearest else {
            return;
        };
        let between = self.rng.unit() < BETWEEN_SHARE;
        let rest = if between {
            body.feet + (enemy - body.feet) * BETWEEN_AT
        } else {
            let angle = self.rng.range(0.0, std::f32::consts::TAU);
            let r = self.rng.range(2.0, ELSEWHERE_M);
            body.feet + Vec3::new(r * angle.cos(), 0.0, r * angle.sin())
        };
        let throw =
            SmokeThrow { by: self.place, team: body.team, press: MatchMs(tick), grenade: *SmokeGrenade::of_game_data() };
        let cloud = throw.cloud_at(rest, |at| floor_under(&game.world.sight, at));
        tracker.add_cloud(cloud);
        self.counts.thrown += 1;
        self.counts.between += u32::from(between);
        self.habit = Habit::Idle;
    }

    /// The bench's line.
    pub(crate) fn line(&self) -> String {
        let c = self.counts;
        let per_100 = if c.lives > 0 { 100.0 * f64::from(c.thrown) / f64::from(c.lives) } else { 0.0 };
        format!(
            "smoke habit (stand-in human): {} lives, {} drawn, {} thrown = {per_100:.2} a 100 lives (live 2.97), {} between him and the nearest enemy (live 69 %)",
            c.lives, c.planned, c.thrown, c.between
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A life that draws a throw puts one cloud when an enemy is near, between the two; a life that does not, none.
    #[test]
    fn a_planned_life_puts_a_cloud_between_him_and_the_nearest_enemy() {
        let mut game = Game::new(super::super::soak_world(), &[0, 1], 1, None, None);
        let mut tracker = Tracker::new(2, &[0, 1], &[true, true], vec![]);
        let mut habit = SmokeHabit { place: 0, rng: Rng::new(1), habit: Habit::Waiting { from: 5_000 }, life: Some(0), counts: SmokeHabitCounts::default() };
        game.bodies[0].feet = Vec3::new(10.0, 0.0, 10.0);
        game.bodies[1].feet = Vec3::new(30.0, 0.0, 10.0);
        habit.tick(&game, &mut tracker, 1_000);
        assert!(tracker.clouds().is_empty(), "not before the life is old enough");
        habit.rng = Rng::new(7);
        for tick in [6_000, 6_050] {
            habit.tick(&game, &mut tracker, tick);
        }
        assert_eq!(tracker.clouds().len(), 1, "one throw a life");
        assert_eq!(habit.counts.thrown, 1);
        if habit.counts.between == 1 {
            let c = tracker.clouds()[0].centre;
            assert!((c.x - 20.0).abs() < 0.5 && (c.z - 10.0).abs() < 0.5, "{c:?}");
        }
    }
}
