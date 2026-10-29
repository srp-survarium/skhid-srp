//! The stand-in human's trap habit: a live player now and then puts a bear trap down, and the bots have to meet it. The stand-in human
//! (`SRP_BENCH_HUMAN`) is a bot place whose trap pocket the bots' own habit does not fill, so its traps are laid by the model
//! ([`Game::trap_lay`]) from the live players' numbers (`traps-record`, `fact-traps.md` section 4 and `trap_since_spawn.py`: the 402 live
//! recordings, 987 traps seen in the worlds, 541 with a known placer; PROVEN counts unless said):
//!
//! - **who carries:** 2.6 % of lives have a trap in the kit in use ([`CARRY_PER_LIFE`]); `SRP_BENCH_HUMAN_TRAPS=carrier` makes every life
//!   a carrier's, to read the habit's shares from many placements (a carrier's life is the same either way).
//! - **how many:** a carrier's life places at least one with a chance of 20.5 % ([`ANY_PER_CARRIER`], 371 of 1,811 lives; 0.30 a life), and
//!   then 1 / 2 / 3 / 4 traps in 59 / 36 / 4 / 1 % of those lives ([`COUNT_SHARES`]).
//! - **when:** [`SINCE_SPAWN_S`] after the spawn (P10 18 s, median 44 s, P90 92 s; 1 % within 10 s), and
//! - **where:** the first tick the human stands in the third of the map the trap was drawn for, by the census's depth (distance to the
//!   nearest own spawn over the nearest own plus the nearest enemy spawn: own third under 0.33, enemy third from 0.66): 29 / 42 / 28 %
//!   ([`DEPTH_SHARES`], 159 / 228 / 154 traps). The stand-in's life ends before it reaches the enemy third more often than before the
//!   middle, so a trap's third is drawn from the live shares over each third's completion ([`COMPLETION_BY_THIRD`]) and the traps put
//!   down come out in the live shares (a calibration of the stand-in, as the smoke habit's `COMPLETION`).
//!   The trap lies where the view meets the floor, [`AHEAD_M`] from his feet and to the side of his heading ([`TURN_RAD`]), cut to what
//!   the model's 2 m reach from the eye allows (about 1 m for a standing body; the census's distance from the placer is dead-reckoned
//!   and cannot say). To the side because the stand-in's walk ignores traps, as live passers-by's do, and one laid in front of him
//!   was caught by its placer at once (105 of 141 in the first batch; live placers are 4 % of the catches).
//! - **the trap** is the model's (`standin_traps.rs`): the sensor, the catch, a shot, a hand defuse; the habit only lays it.
//!
//! Each placement is logged with the class drawn and the class scored at the spot, the seconds since spawn, and what became of the
//! trap, so the shares can be read per item (`placement_lines`). Default on with a stand-in human and the trap model;
//! `SRP_BENCH_HUMAN_TRAPS=off` leaves the human without a trap. Bench only.

use mapdata::Vec3;

use super::standin_traps::{Event, Phase};
use super::*;

/// Lives with a trap in the kit in use (2.6 % of 69,520 lives).
pub(crate) const CARRY_PER_LIFE: f32 = 0.026;
/// A carrier's life places at least one trap: 371 of 1,811 lives.
pub(crate) const ANY_PER_CARRIER: f32 = 371.0 / 1811.0;
/// The share of the planned lives that place one, the rest end first (the stand-in's lives are shorter and its walk is its lane's).
/// Measured by the carrier batch (lives that placed over lives that planned), re-measured whenever the rule of "when" changes.
pub(crate) const COMPLETION: f32 = 0.24;
/// The share of planned traps that are put down, by the third they are for (own, middle, enemy): placed over planned in the first
/// carrier batch (38, 84, 19 placed of 141, 202, 137 planned by the live shares), an estimate. A third the stand-in's life seldom
/// reaches is drawn more often so that the traps put down are the live shares.
const COMPLETION_BY_THIRD: [f32; 3] = [0.27, 0.42, 0.14];
/// Traps in a life that places: 1 / 2 / 3 / 4 (220 / 134 / 15 / 2 of 371).
const COUNT_SHARES: [f32; 4] = [220.0 / 371.0, 134.0 / 371.0, 15.0 / 371.0, 2.0 / 371.0];
/// Seconds from the spawn to the placing (fraction of traps, value), linear between; the ends are GUESSES.
const SINCE_SPAWN_S: [(f32, f32); 7] = [(0.0, 3.0), (0.10, 18.2), (0.25, 29.6), (0.5, 44.4), (0.75, 64.8), (0.9, 92.0), (1.0, 160.0)];
/// The thirds of the map by the census's depth: own, middle, enemy (159 / 228 / 154 of 541).
const DEPTH_SHARES: [f32; 3] = [159.0 / 541.0, 228.0 / 541.0, 154.0 / 541.0];
/// The depth under which a spot is in the own third, and from which it is in the enemy's.
const OWN_THIRD: f32 = 0.33;
const ENEMY_THIRD: f32 = 0.66;
/// The spot is looked for only when the human is within this depth of the third (a metre to the side is far inside it): it saves a
/// ray on every tick of a due plan and keeps the refused spots to those in or beside the third.
const NEAR_BAND: f32 = 0.05;
/// How far from its feet the view meets the floor, metres on the ground (GUESS; `Game::trap_aim_ahead` cuts it to the model's reach).
const AHEAD_M: (f32, f32) = (0.9, 1.15);
/// ... and how far from its heading, radians, to the left or the right (GUESS: to the side, 60 to 120 degrees).
const TURN_RAD: (f32, f32) = (1.05, 2.1);

/// A third of the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Third {
    Own,
    Middle,
    Enemy,
}

impl Third {
    const ALL: [Third; 3] = [Third::Own, Third::Middle, Third::Enemy];

    fn of_depth(depth: f32) -> Self {
        if depth < OWN_THIRD {
            Third::Own
        } else if depth >= ENEMY_THIRD {
            Third::Enemy
        } else {
            Third::Middle
        }
    }

    /// How far a depth is from this third's band (0 inside it).
    fn gap(self, depth: f32) -> f32 {
        let (low, high) = match self {
            Third::Own => (0.0, OWN_THIRD),
            Third::Middle => (OWN_THIRD, ENEMY_THIRD),
            Third::Enemy => (ENEMY_THIRD, 1.0),
        };
        (low - depth).max(depth - high).max(0.0)
    }

    fn index(self) -> usize {
        self as usize
    }

    fn name(self) -> &'static str {
        match self {
            Third::Own => "own third",
            Third::Middle => "middle",
            Third::Enemy => "enemy third",
        }
    }
}

/// The census's depth of a spot for a team: the nearest own spawn's distance over the nearest own plus the nearest enemy spawn's (on the
/// ground plane). `None` on a map with no spawns of one of the teams.
fn depth_of(game: &Game, team: u8, at: Vec3) -> Option<f32> {
    let nearest = |own: bool| {
        game.world
            .map
            .spawns
            .iter()
            .filter(|s| (s.team == i32::from(team)) == own)
            .map(|s| s.position.distance_xz(at))
            .min_by(f32::total_cmp)
    };
    let (own, enemy) = (nearest(true)?, nearest(false)?);
    (own + enemy > 0.0).then(|| own / (own + enemy))
}

/// The value at a drawn fraction of a table of (fraction, value) points.
fn at_fraction(table: &[(f32, f32)], p: f32) -> f32 {
    let k = table.iter().position(|(q, _)| *q >= p).unwrap_or(table.len() - 1).max(1);
    let ((q0, v0), (q1, v1)) = (table[k - 1], table[k]);
    if q1 > q0 { v0 + (v1 - v0) * ((p - q0) / (q1 - q0)).clamp(0.0, 1.0) } else { v1 }
}

/// The index drawn from shares that sum to 1.
fn pick(shares: &[f32], p: f32) -> usize {
    let mut left = p;
    for (i, share) in shares.iter().enumerate() {
        if left < *share {
            return i;
        }
        left -= share;
    }
    shares.len() - 1
}

/// Who carries a trap in a life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Carry {
    /// 2.6 % of the lives (the live share).
    Live,
    /// Every life (`SRP_BENCH_HUMAN_TRAPS=carrier`): a carrier's life is the live one, there are more of them.
    Always,
}

/// One trap a life will put down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Plan {
    /// At or after this tick, in the third `want`.
    from: u32,
    want: Third,
    /// How far from its feet it lies, metres, and the turn from his heading, radians (left positive).
    ahead_m: f32,
    turn: f32,
}

/// The shares a trap's third is drawn from: the live shares over each third's completion, normalised.
fn drawn_shares() -> [f32; 3] {
    let weights: [f32; 3] = std::array::from_fn(|i| DEPTH_SHARES[i] / COMPLETION_BY_THIRD[i]);
    let total: f32 = weights.iter().sum();
    weights.map(|w| w / total)
}

/// The traps a planned life puts down, in the order of their times.
fn draw_plans(rng: &mut Rng, spawned: u32) -> Vec<Plan> {
    let n = pick(&COUNT_SHARES, rng.unit()) + 1;
    let shares = drawn_shares();
    let mut plans: Vec<Plan> = (0..n)
        .map(|_| Plan {
            from: spawned + (at_fraction(&SINCE_SPAWN_S, rng.unit()) * 1000.0) as u32,
            want: Third::ALL[pick(&shares, rng.unit())],
            ahead_m: rng.range(AHEAD_M.0, AHEAD_M.1),
            turn: rng.range(TURN_RAD.0, TURN_RAD.1) * if rng.unit() < 0.5 { 1.0 } else { -1.0 },
        })
        .collect();
    plans.sort_by_key(|p| p.from);
    plans
}

/// Where the human is in his life.
#[derive(Clone, Debug, PartialEq)]
enum Habit {
    /// No life seen yet, or this life puts no trap down (or has put them all).
    Idle,
    /// This life puts these traps down, the first next.
    Planned { plans: Vec<Plan> },
}

/// What became of a trap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Outcome {
    /// Caught a body of this kind (by the placer's side), `tick` it happened.
    Caught(Kill),
    ShotOff,
    Defused,
    /// Armed or standing still at the end of the run.
    Armed,
}

/// One trap the human put down.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Placement {
    pub(crate) tick: u32,
    pub(crate) since_s: f32,
    pub(crate) want: Third,
    /// The third of the spot where it lies, by the census's test (`None`: the map has no spawns of a team).
    pub(crate) scored: Option<Third>,
    pub(crate) depth: Option<f32>,
    pub(crate) press_m: f32,
    /// The number of this trap in its life (1-based).
    pub(crate) nth: u8,
}

/// What the habit counted.
#[derive(Clone, Debug, Default)]
pub(crate) struct TrapHabitCounts {
    pub(crate) lives: u32,
    pub(crate) carrying: u32,
    pub(crate) planned_lives: u32,
    pub(crate) planned: u32,
    /// The traps planned for each third (own, middle, enemy).
    pub(crate) planned_by: [u32; 3],
    /// The planned lives by the number of traps they plan (1 to 4).
    pub(crate) planned_n: [u32; 4],
    pub(crate) placements: Vec<Placement>,
    /// Ticks a planned trap was due in its third and the floor in front was no place for one (steep, nothing in reach, on a sensor).
    pub(crate) refused: u32,
    /// Lives by the number of traps they put down: 0 (planned and ended first), 1, 2, 3, 4.
    pub(crate) per_life: [u32; 5],
}

/// A trap the habit put down, for the team to be told of (`Bots::bench_team_knows_trap`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Laid {
    pub(crate) team: u8,
    pub(crate) place: Place,
    pub(crate) at: Vec3,
    pub(crate) tick: u32,
}

pub(crate) struct TrapHabit {
    place: Place,
    /// The traps put down since the bench last took them.
    pub(crate) laid: Vec<Laid>,
    rng: Rng,
    carry: Carry,
    habit: Habit,
    /// The `spawned` tick of the life the habit has seen, and how many traps it has put down.
    life: Option<u32>,
    put: u8,
    /// Whether the life was a planned one (its count is read when it ends).
    life_planned: bool,
    pub(crate) counts: TrapHabitCounts,
}

impl TrapHabit {
    /// The habit of the stand-in human at `place`, unless `SRP_BENCH_HUMAN_TRAPS=off` or the trap model is off.
    pub(crate) fn start(place: Place, seed: u32, level: u8, mode: u8, per_team: usize) -> Option<Self> {
        let setting = bench_settings().var("SRP_BENCH_HUMAN_TRAPS").unwrap_or_else(|_| "live".into());
        let carry = match setting.as_str() {
            "off" => return None,
            "carrier" => Carry::Always,
            _ => Carry::Live,
        };
        super::standin_traps::model_on().then(|| Self {
            place,
            laid: Vec::new(),
            rng: Rng::new(super::standin_smoke::habit_seed(0x7_4A9, seed, level, mode, per_team)),
            carry,
            habit: Habit::Idle,
            life: None,
            put: 0,
            life_planned: false,
            counts: TrapHabitCounts::default(),
        })
    }

    /// The previous life's count of traps put down, read when a new life starts or the habit is read.
    fn close_life(&mut self) {
        if self.life_planned {
            self.counts.per_life[usize::from(self.put).min(4)] += 1;
        }
        self.life_planned = false;
        self.put = 0;
    }

    /// One tick: a new life draws whether it carries a trap and puts it down; a planned trap is laid at the first tick the human
    /// stands in its third and the floor in front of him takes one.
    pub(crate) fn tick(&mut self, game: &mut Game, tick: u32) {
        let Some(body) = game.bodies.get(self.place) else {
            return;
        };
        let (alive, spawned, team, feet) = (body.alive, body.spawned, body.team, body.feet);
        if !alive {
            self.habit = Habit::Idle;
            return;
        }
        if self.life != Some(spawned) {
            self.close_life();
            self.life = Some(spawned);
            self.counts.lives += 1;
            let carries = match self.carry {
                Carry::Live => self.rng.unit() < CARRY_PER_LIFE,
                Carry::Always => true,
            };
            self.counts.carrying += u32::from(carries);
            self.habit = if carries && self.rng.unit() < ANY_PER_CARRIER / COMPLETION {
                let plans = draw_plans(&mut self.rng, spawned);
                self.counts.planned_lives += 1;
                self.counts.planned += plans.len() as u32;
                self.counts.planned_n[plans.len() - 1] += 1;
                for plan in &plans {
                    self.counts.planned_by[plan.want.index()] += 1;
                }
                self.life_planned = true;
                Habit::Planned { plans }
            } else {
                Habit::Idle
            };
        }
        let Habit::Planned { plans } = &mut self.habit else {
            return;
        };
        let plan = plans[0];
        if tick < plan.from {
            return;
        }
        if depth_of(game, team, feet).is_some_and(|d| plan.want.gap(d) > NEAR_BAND) {
            return;
        }
        // The third is the spot's, as the census scores a trap by where it lies: tested at the spot, not at his feet (a metre to the side
        // is outside the third half the time he has just walked into it).
        let (eye, dir) = game.trap_aim_ahead(self.place, plan.ahead_m, plan.turn);
        let Some(at) = game.trap_land(eye, dir) else {
            self.counts.refused += 1;
            return;
        };
        let spot = depth_of(game, team, at);
        if spot.is_some_and(|d| Third::of_depth(d) != plan.want) {
            return;
        }
        game.trap_lay(self.place, at, (-dir.y).asin(), tick);
        self.laid.push(Laid { team, place: self.place, at, tick });
        self.put += 1;
        self.counts.placements.push(Placement {
            tick,
            since_s: tick.saturating_sub(spawned) as f32 / 1000.0,
            want: plan.want,
            scored: spot.map(Third::of_depth),
            depth: spot,
            press_m: feet.distance_xz(at),
            nth: self.put,
        });
        plans.remove(0);
        if plans.is_empty() {
            self.habit = Habit::Idle;
        }
    }

    /// What became of the trap put down at `placement.tick`, from the game's events and its traps.
    pub(crate) fn outcome(&self, game: &Game, placement: &Placement) -> Outcome {
        let ours = |placer: Place, placed: u32| placer == self.place && placed == placement.tick;
        for event in &game.traps.events {
            match *event {
                Event::Caught { placer, placed, kind, .. } if ours(placer, placed) => return Outcome::Caught(kind),
                Event::ShotOff { placer, placed, .. } if ours(placer, placed) => return Outcome::ShotOff,
                Event::DefusedBy { placer, placed, .. } if ours(placer, placed) => return Outcome::Defused,
                _ => {}
            }
        }
        debug_assert!(
            game.traps.list.iter().any(|t| ours(t.placer, t.placed) && t.phase == Phase::Armed),
            "a trap neither armed nor spent"
        );
        Outcome::Armed
    }

    /// The bench's line.
    pub(crate) fn line(&mut self, game: &Game) -> String {
        self.close_life();
        let c = &self.counts;
        let placed = c.placements.len();
        let per_100 = if c.lives > 0 { 100.0 * placed as f64 / f64::from(c.lives) } else { 0.0 };
        let mut scored = [0u32; 3];
        let mut wanted = [0u32; 3];
        for p in &c.placements {
            wanted[p.want.index()] += 1;
            if let Some(third) = p.scored {
                scored[third.index()] += 1;
            }
        }
        let share = |counts: [u32; 3]| -> String {
            let total: u32 = counts.iter().sum();
            counts.map(|n| format!("{:.0}", 100.0 * f64::from(n) / f64::from(total.max(1)))).join("/")
        };
        let mut since: Vec<f32> = c.placements.iter().map(|p| p.since_s).collect();
        since.sort_by(f32::total_cmp);
        let q = |p: f32| since.get(((since.len().saturating_sub(1)) as f32 * p).round() as usize).map(|v| (v * 10.0).round() / 10.0);
        let mut outcomes = [0u32; 6];
        for p in &c.placements {
            outcomes[match self.outcome(game, p) {
                Outcome::Caught(Kill::Enemy) => 0,
                Outcome::Caught(Kill::Mate) => 1,
                Outcome::Caught(Kill::Own) => 2,
                Outcome::ShotOff => 3,
                Outcome::Defused => 4,
                Outcome::Armed => 5,
            }] += 1;
        }
        format!(
            "trap habit (stand-in human): {} lives, {} carrying, {} planned in {} lives (by third {:?}, by count 1/2/3/4 {:?}), {} placed = {per_100:.2} a 100 lives (live 0.78 for carriers 2.6 % x 0.30); lives by traps put down 0/1/2/3/4 {:?} (live of those that placed 59/36/4/1 %); thirds own/middle/enemy % drawn {} scored {} (live 29/42/28); press s from spawn P10/25/50/75/90 {:?} (live 18.2/29.6/44.4/64.8/92.0), {} refused spots; outcomes enemy/mate/own caught, shot off, defused, armed {:?} (live of the fired: enemy 85 %, mate 11 %, own 4 %)",
            c.lives,
            c.carrying,
            c.planned,
            c.planned_lives,
            c.planned_by,
            c.planned_n,
            placed,
            c.per_life,
            share(wanted),
            share(scored),
            [0.1, 0.25, 0.5, 0.75, 0.9].map(q),
            c.refused,
            outcomes,
        )
    }

    /// One line a trap put down: its number in the life, when, the third drawn and the third scored, and what became of it.
    pub(crate) fn placement_lines(&self, game: &Game) -> Vec<String> {
        self.counts
            .placements
            .iter()
            .map(|p| {
                format!(
                    "  trap placement: no. {} of its life at {:.1} s from the spawn (tick {}), {} drawn, {} scored (depth {:?}), {:.2} m in front: {:?}",
                    p.nth,
                    p.since_s,
                    p.tick,
                    p.want.name(),
                    p.scored.map_or("unknown", Third::name),
                    p.depth.map(|d| (d * 100.0).round() / 100.0),
                    p.press_m,
                    self.outcome(game, p),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two teams on the soak field (side 0 spawns at x = 5, side 1 at x = 75: depth is (x - 5) / 70 along it), the human at `place` 0.
    fn field(human_at: Vec3) -> Game {
        let world = soak_world();
        let mut game = Game::new(world.clone(), &[0, 1], 5, Some(30), None);
        for (place, at) in [(0, human_at), (1, Vec3::new(70.0, 0.0, 12.0))] {
            game.bodies[place].node = world.grid.locate(at, 1.0);
            game.bodies[place].feet = at;
            game.bodies[place].alive = true;
        }
        game
    }

    fn habit(plans: Vec<Plan>, spawned: u32) -> TrapHabit {
        TrapHabit {
            place: 0,
            laid: Vec::new(),
            rng: Rng::new(5),
            carry: Carry::Live,
            habit: Habit::Planned { plans },
            life: Some(spawned),
            put: 0,
            life_planned: true,
            counts: TrapHabitCounts::default(),
        }
    }

    #[test]
    fn the_census_depth_names_the_thirds() {
        assert_eq!(Third::of_depth(0.0), Third::Own);
        assert_eq!(Third::of_depth(0.329), Third::Own);
        assert_eq!(Third::of_depth(0.33), Third::Middle);
        assert_eq!(Third::of_depth(0.659), Third::Middle);
        assert_eq!(Third::of_depth(0.66), Third::Enemy);
        assert_eq!(Third::Middle.gap(0.5), 0.0);
        assert!((Third::Middle.gap(0.2) - 0.13).abs() < 1e-6);
        assert!((Third::Enemy.gap(0.5) - 0.16).abs() < 1e-6);
        assert!((Third::Own.gap(0.4) - 0.07).abs() < 1e-6);
        let game = field(Vec3::new(5.0, 0.0, 12.0));
        // The soak field spawns side 0 at x = 5 and side 1 at x = 75.
        assert!((depth_of(&game, 0, Vec3::new(40.0, 0.0, 12.0)).unwrap() - 0.5).abs() < 0.02);
        assert!((depth_of(&game, 1, Vec3::new(40.0, 0.0, 12.0)).unwrap() - 0.5).abs() < 0.02);
        assert!(depth_of(&game, 0, Vec3::new(10.0, 0.0, 12.0)).unwrap() < 0.1);
        assert!(depth_of(&game, 1, Vec3::new(10.0, 0.0, 12.0)).unwrap() > 0.9, "the same spot is deep for the other side");
    }

    /// A planned trap waits for its time and its third, then lies in front of the human, armed, and is logged with both thirds.
    #[test]
    fn a_planned_trap_is_laid_in_its_third_and_logged() {
        let mut game = field(Vec3::new(14.0, 0.0, 12.0));
        game.bodies[0].spawned = 0;
        let plan = Plan { from: 20_000, want: Third::Middle, ahead_m: 1.0, turn: 1.5 };
        let mut h = habit(vec![plan], 0);
        h.tick(&mut game, 10_000);
        assert!(game.traps.list.is_empty(), "not before the time");
        h.tick(&mut game, 20_000);
        assert!(game.traps.list.is_empty(), "x = 14 is the own third, the plan wants the middle");
        game.bodies[0].feet = Vec3::new(40.0, 0.0, 12.0);
        h.tick(&mut game, 20_050);
        assert_eq!(game.traps.list.len(), 1);
        let trap = game.traps.list[0];
        assert_eq!((trap.placer, trap.phase, trap.placed), (0, Phase::Armed, 20_050));
        assert!((trap.at.distance_xz(Vec3::new(40.0, 0.0, 12.0)) - 1.0).abs() < 0.35, "in front: {:?}", trap.at);
        let p = h.counts.placements[0];
        assert_eq!((p.want, p.scored, p.nth), (Third::Middle, Some(Third::Middle), 1));
        assert!((p.since_s - 20.05).abs() < 1e-3);
        assert_eq!(h.laid.len(), 1, "the bench is told of it, for the team");
        assert_eq!((h.laid[0].team, h.laid[0].place, h.laid[0].tick), (0, 0, 20_050));
        assert!(h.laid[0].at.distance_xz(trap.at) < 1e-4);
        assert_eq!(h.habit, Habit::Idle);
        h.tick(&mut game, 20_100);
        assert_eq!(game.traps.list.len(), 1, "one trap a plan");
        assert_eq!(h.outcome(&game, &p), Outcome::Armed);
    }

    /// A planned life of two traps lays them in order, one per plan; what becomes of each is read from the game.
    #[test]
    fn a_life_of_two_traps_lays_both_and_each_outcome_is_read() {
        let mut game = field(Vec3::new(40.0, 0.0, 12.0));
        game.bodies[0].spawned = 0;
        let plans = vec![Plan { from: 1_000, want: Third::Middle, ahead_m: 1.0, turn: 1.5 }, Plan { from: 2_000, want: Third::Middle, ahead_m: 1.0, turn: -1.5 }];
        let mut h = habit(plans, 0);
        h.tick(&mut game, 1_000);
        // The second lies on another floor point: the human has walked on.
        game.bodies[0].feet = Vec3::new(44.0, 0.0, 12.0);
        h.tick(&mut game, 2_000);
        assert_eq!(game.traps.list.len(), 2);
        assert_eq!(h.counts.placements.iter().map(|p| p.nth).collect::<Vec<_>>(), vec![1, 2]);
        // An enemy steps on the first.
        let first = game.traps.list[0];
        game.bodies[1].feet = first.at;
        game.trap_tick(2_050);
        let p1 = h.counts.placements[0];
        let p2 = h.counts.placements[1];
        assert_eq!(h.outcome(&game, &p1), Outcome::Caught(Kill::Enemy));
        assert_eq!(h.outcome(&game, &p2), Outcome::Armed);
        h.close_life();
        assert_eq!(h.counts.per_life, [0, 0, 1, 0, 0]);
        assert!(!h.line(&game).is_empty());
        assert_eq!(h.placement_lines(&game).len(), 2);
    }

    /// The draws are the census's: the number of traps, the seconds from the spawn and the thirds, over many planned lives.
    #[test]
    fn the_plans_follow_the_live_census() {
        let mut rng = Rng::new(21);
        let mut counts = [0u32; 4];
        let mut since = Vec::new();
        let mut thirds = [0u32; 3];
        let lives = 20_000;
        for _ in 0..lives {
            let plans = draw_plans(&mut rng, 0);
            counts[plans.len() - 1] += 1;
            assert!(plans.windows(2).all(|w| w[0].from <= w[1].from), "in order of their times");
            for p in plans {
                since.push(p.from as f32 / 1000.0);
                thirds[p.want.index()] += 1;
            }
        }
        let share = |n: u32, of: u32| f64::from(n) / f64::from(of);
        assert!((share(counts[0], lives) - 0.593).abs() < 0.015, "one trap {}", share(counts[0], lives));
        assert!((share(counts[1], lives) - 0.361).abs() < 0.015, "two traps {}", share(counts[1], lives));
        let all: u32 = thirds.iter().sum();
        // The thirds drawn are the live shares over each third's completion; put down, they are the live ones.
        let drawn = drawn_shares();
        for i in 0..3 {
            assert!((share(thirds[i], all) - f64::from(drawn[i])).abs() < 0.015, "third {i}: {}", share(thirds[i], all));
        }
        let put: Vec<f32> = (0..3).map(|i| drawn[i] * COMPLETION_BY_THIRD[i]).collect();
        let total: f32 = put.iter().sum();
        for (i, live) in [0.294, 0.421, 0.285].iter().enumerate() {
            assert!((f64::from(put[i] / total) - live).abs() < 0.005, "put down {i}: {}", put[i] / total);
        }
        since.sort_by(f32::total_cmp);
        let at = |p: f32| since[((since.len() - 1) as f32 * p) as usize];
        assert!((at(0.1) - 18.2).abs() < 1.5, "P10 {}", at(0.1));
        assert!((at(0.5) - 44.4).abs() < 2.0, "median {}", at(0.5));
        assert!((at(0.9) - 92.0).abs() < 4.0, "P90 {}", at(0.9));
    }

    /// Lives that carry and plan are the live share: 2.6 % x 20.5 % a life, over the completion share.
    #[test]
    fn a_live_run_plans_the_live_share_of_lives() {
        let mut game = field(Vec3::new(40.0, 0.0, 12.0));
        let mut h = habit(vec![], 0);
        h.habit = Habit::Idle;
        h.life = None;
        h.life_planned = false;
        let lives = 200_000u32;
        for life in 0..lives {
            game.bodies[0].spawned = life;
            game.bodies[0].alive = true;
            h.tick(&mut game, life);
            game.bodies[0].alive = false;
            h.tick(&mut game, life);
        }
        let planned = f64::from(h.counts.planned_lives) / f64::from(lives);
        let live = f64::from(CARRY_PER_LIFE * ANY_PER_CARRIER / COMPLETION);
        assert!((planned - live).abs() < 0.0006, "planned {planned} against {live}");
        assert!((f64::from(h.counts.carrying) / f64::from(lives) - f64::from(CARRY_PER_LIFE)).abs() < 0.002);
    }
}
