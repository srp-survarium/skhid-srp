//! Patch 92: a battery's carriers score for bringing it nearer their team's
//! container, paid when it is delivered there (the owner, 2026-11-01;
//! docs/notes/battery_credit.md).
//!
//! Each battery keeps, for each team's container, the nearest it has come in
//! its life (from the match's start, or its last delivery), whoever carried
//! it: in whole steps of [`STEP_METRES`]. While a player carries it, every
//! simulation tick, the steps by which its own team's container's nearest
//! comes nearer are held on the battery for the carrier (its place and kit).
//! Ground covered before, by anyone, earns nothing again: walking back and
//! forth, dropping and taking it again, or handing it to a teammate earns
//! only new ground. Nothing shows while it is carried (the score would tell
//! the enemy who brings a battery in): when the battery is put into a
//! container, every carrier of that container's team is paid what it holds,
//! and the steps of the other team's carriers are dropped. A delivery starts
//! the battery's life again; the match's start drops every life. A step is
//! 40 hundredths of event 6 ("bring victory item"), which srp weighs 5 a
//! delivery's 100: 2 points.

#![forbid(unsafe_code)]

use crate::vostok::{
    statistics, Band, Batteries, Battery, BatteryCarry, Cell, Context, Delivery, Flow, Held, Life, Lock, NoExit,
    Progress, Saving, ScoreEvent, Team, BATTERIES,
};

/// Metres of new progress a step is.
pub const STEP_METRES: f32 = 10.0;
/// A step, in the hundredths of a delivery that event 6 counts: 2 points.
const POINT: u16 = 40;

/// The lives of the batteries of the match being played (the statistics
/// they are of): the battery's own state, which the game's record has no
/// room for.
struct Lives {
    of: Option<Batteries>,
    lives: [Life; BATTERIES],
}

impl Lives {
    const NONE: Lives = Lives { of: None, lives: [Life::NEW; BATTERIES] };

    /// Every life of `batteries` begins.
    fn begin(&mut self, batteries: Batteries) {
        *self = Lives { of: Some(batteries), lives: [Life::NEW; BATTERIES] };
    }

    /// `battery`'s life; its batteries' lives begin first when they are
    /// not the ones kept.
    fn life(&mut self, battery: Battery) -> &mut Life {
        if self.of != Some(battery.of()) {
            self.begin(battery.of());
        }
        &mut self.lives[battery.index()]
    }

    /// The life of the battery being saved; a new one for a record of no
    /// batteries kept.
    fn saved(&self, saving: &Saving<'_>) -> Life {
        self.of.and_then(|of| saving.battery(of)).map_or(Life::NEW, |battery| self.lives[battery.index()])
    }
}

static LIVES: Lock<Lives> = Lock::new(Lives::NONE);

/// The band `distance` metres is in: its whole steps; the farthest band
/// beyond it, and for a distance that is no number (it never comes nearer).
fn band(distance: f32) -> Band {
    if distance.is_finite() {
        // `as` truncates toward 0 and saturates: a distance is >= 0.
        Band::new((distance / STEP_METRES) as u8)
    } else {
        Band::FARTHEST
    }
}

/// The battery is `now` from each container and a player of `carrier`
/// carries it. Each container's nearest band of the life becomes the nearer
/// of it and `now` (the first carry of the life sets both); the steps are
/// those by which the carrier's own team's comes nearer than ever.
fn nearer(progress: Progress, carrier: Team, now: [Band; 2]) -> (Progress, u8) {
    let Progress(nearest) = progress;
    let after = core::array::from_fn(|team| Some(nearest[team].map_or(now[team], |best| best.min(now[team]))));
    let own = carrier.index();
    let steps = nearest[own].map_or(0, |best| best.steps().saturating_sub(now[own].steps()));
    (Progress(after), steps)
}

/// One tick of a carry by `cell`, of `team`: the life's progress, and the
/// steps it earned held for the cell.
fn carried(life: Life, cell: Cell, team: Team, now: [Band; 2]) -> Life {
    let (progress, steps) = nearer(life.progress, team, now);
    Life { progress, held: life.held.add(cell, steps) }
}

/// What a delivery into `into`'s container pays of `held`: each cell of
/// that team its steps; the other team's cells nothing.
fn paid<'a>(held: &'a Held, into: Team, team_of: impl Fn(Cell) -> Option<Team> + 'a) -> impl Iterator<Item = (Cell, u16)> + 'a {
    held.cells().filter(move |&(cell, _)| team_of(cell) == Some(into)).map(|(cell, steps)| (cell, u16::from(steps) * POINT))
}

/// base_player::tick after the held object's tick.
pub fn on_carrier_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(carry) = BatteryCarry::at_tick(ctx) {
        let now = carry.distances().map(band);
        LIVES.with(ctx.thread(), |lives| {
            let life = lives.life(carry.battery());
            *life = carried(*life, carry.cell(), carry.team(), now);
        });
    }
    Flow::Continue
}

/// The match's start: every battery's life begins.
pub fn on_match_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(batteries) = statistics::started(ctx) {
        LIVES.with(ctx.thread(), |lives| lives.begin(batteries));
    }
    Flow::Continue
}

/// A put-in: the container's team's carriers paid what they hold, and the
/// battery's life begins again.
pub fn on_delivered(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(delivery) = Delivery::at(ctx) {
        let life = LIVES.with(ctx.thread(), |lives| core::mem::replace(lives.life(delivery.battery()), Life::NEW));
        if let Some(into) = delivery.container() {
            for (cell, amount) in paid(&life.held, into, |cell| delivery.team_of(cell)) {
                delivery.count(cell, ScoreEvent::BRING_VICTORY_ITEM, amount);
            }
        }
    }
    Flow::Continue
}

/// A battery's record serialized: its life after it.
pub fn on_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(saving) = Saving::at(ctx) {
        let life = LIVES.with(ctx.thread(), |lives| lives.saved(&saving));
        saving.write(&life);
    }
    Flow::Continue
}

/// A battery's record deserialized: its life after it.
pub fn on_load(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some((battery, life)) = statistics::load(ctx) {
        LIVES.with(ctx.thread(), |lives| *lives.life(battery) = life);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bands(zero: f32, one: f32) -> [Band; 2] {
        [band(zero), band(one)]
    }

    fn cell(place: u8, kit: u8) -> Cell {
        Cell::of(place, kit).unwrap()
    }

    /// Ticks of one carrier over the distances (to team 0's container, to
    /// team 1's): the progress after, and the steps earned.
    fn walk(mut progress: Progress, carrier: Team, path: &[(f32, f32)]) -> (Progress, u32) {
        let mut earned = 0;
        for &(zero, one) in path {
            let (after, steps) = nearer(progress, carrier, bands(zero, one));
            progress = after;
            earned += u32::from(steps);
        }
        (progress, earned)
    }

    /// Ticks of `cell` (of `team`) carrying the battery of `life`.
    fn carry(mut life: Life, cell: Cell, team: Team, path: &[(f32, f32)]) -> Life {
        for &(zero, one) in path {
            life = carried(life, cell, team, bands(zero, one));
        }
        life
    }

    /// Places 0..10 are team 0's, 10..20 team 1's.
    fn team_of(cell: Cell) -> Option<Team> {
        let zero = (0..10).flat_map(|place| (0..3).map(move |kit| Cell::of(place, kit))).any(|of| of == Some(cell));
        Some(if zero { Team::Zero } else { Team::One })
    }

    #[test]
    fn a_band_is_whole_steps_of_ten_metres() {
        assert_eq!(band(0.0), Band::new(0));
        assert_eq!(band(9.99), Band::new(0));
        assert_eq!(band(10.0), Band::new(1));
        assert_eq!(band(73.5), Band::new(7));
        assert_eq!(band(1.0e6), Band::FARTHEST);
        assert_eq!(band(f32::NAN), Band::FARTHEST);
        assert_eq!(band(f32::INFINITY), Band::FARTHEST);
    }

    #[test]
    fn the_first_carry_of_a_life_earns_nothing_and_keeps_where_it_was() {
        let (progress, steps) = nearer(Progress::default(), Team::Zero, bands(73.0, 40.0));
        assert_eq!(steps, 0);
        assert_eq!(progress, Progress([Some(Band::new(7)), Some(Band::new(4))]));
    }

    #[test]
    fn a_carrier_earns_a_step_a_ten_metres_nearer() {
        // 73 m from its container to 2 m: from band 7 to band 0.
        let path: Vec<(f32, f32)> = (0..=71).map(|m| (73.0 - m as f32, 40.0)).collect();
        let (progress, earned) = walk(Progress::default(), Team::Zero, &path);
        assert_eq!(earned, 7);
        assert_eq!(progress.0[0], Some(Band::new(0)));
    }

    #[test]
    fn walking_back_and_forth_earns_once() {
        let there_and_back = [(50.0, 0.0), (41.0, 0.0), (55.0, 0.0), (41.0, 0.0), (55.0, 0.0), (39.0, 0.0)];
        let (_, earned) = walk(Progress::default(), Team::Zero, &there_and_back);
        // 50 -> 41: one step; back and forth to 41 again: none; 39: one more.
        assert_eq!(earned, 2);
    }

    #[test]
    fn a_re_pickup_earns_nothing_until_new_ground() {
        let (carried, earned) = walk(Progress::default(), Team::One, &[(0.0, 80.0), (0.0, 61.0)]);
        assert_eq!(earned, 2);
        // Dropped at 61 m and taken again (by the same player, or after it
        // rolled back to 68 m): the life's nearest stays, nothing is earned
        // until it is nearer than 60 m.
        let (carried, earned) = walk(carried, Team::One, &[(0.0, 68.0), (0.0, 62.0), (0.0, 61.0)]);
        assert_eq!(earned, 0);
        let (_, earned) = walk(carried, Team::One, &[(0.0, 59.0)]);
        assert_eq!(earned, 1);
    }

    #[test]
    fn an_enemy_carry_brings_the_nearest_for_both_teams() {
        // Team 1 carries a fresh battery from 60 m of team 0's container to
        // 25 m of it (away from its own, 40 m -> 75 m): it earns nothing,
        // and the 35 m toward team 0's container earn team 0 nothing later.
        let (taken, earned) = walk(Progress::default(), Team::One, &[(60.0, 40.0), (25.0, 75.0)]);
        assert_eq!(earned, 0);
        let (_, earned) = walk(taken, Team::Zero, &[(25.0, 75.0), (40.0, 60.0), (25.0, 75.0), (5.0, 95.0)]);
        assert_eq!(earned, 2);
    }

    #[test]
    fn two_enemies_passing_it_back_and_forth_earn_the_ground_once() {
        let mut progress = Progress::default();
        let mut earned = [0, 0];
        for _ in 0..5 {
            let (after, zero) = walk(progress, Team::Zero, &[(50.0, 50.0), (20.0, 80.0)]);
            let (after, one) = walk(after, Team::One, &[(20.0, 80.0), (50.0, 50.0)]);
            progress = after;
            earned[0] += zero;
            earned[1] += one;
        }
        assert_eq!(earned, [3, 0]);
    }

    #[test]
    fn a_tick_earns_every_step_it_crosses() {
        let (progress, _) = nearer(Progress::default(), Team::Zero, bands(200.0, 0.0));
        let (_, steps) = nearer(progress, Team::Zero, bands(0.0, 0.0));
        assert_eq!(steps, 20);
        assert!(u16::from(Band::FARTHEST.steps()).checked_mul(POINT).is_some());
    }

    #[test]
    fn a_carry_holds_its_steps_for_the_carrier() {
        let life = carry(Life::NEW, cell(2, 1), Team::Zero, &[(73.0, 40.0), (1.0, 90.0)]);
        assert_eq!(life.held.cells().collect::<Vec<_>>(), [(cell(2, 1), 7)]);
        assert_eq!(life.progress.0[0], Some(Band::new(0)));
    }

    #[test]
    fn a_delivery_pays_every_carrier_of_the_containers_team() {
        // A relay: place 2 brings it from 90 m to 61 m, is killed; place 5
        // takes it on to the container. Both are paid at the delivery, 2
        // points (40 hundredths) a step.
        let life = carry(Life::NEW, cell(2, 0), Team::Zero, &[(90.0, 20.0), (61.0, 40.0)]);
        let life = carry(life, cell(5, 2), Team::Zero, &[(61.0, 40.0), (75.0, 30.0), (3.0, 97.0)]);
        let paid: Vec<_> = paid(&life.held, Team::Zero, team_of).collect();
        assert_eq!(paid, [(cell(2, 0), 3 * POINT), (cell(5, 2), 6 * POINT)]);
    }

    #[test]
    fn the_other_teams_delivery_drops_the_steps() {
        // Team 0 brings it from 80 m to 30 m of its container; team 1 kills
        // the carrier and delivers it home: team 0's five steps are paid to
        // nobody, and team 1's own carry pays team 1.
        let life = carry(Life::NEW, cell(1, 0), Team::Zero, &[(80.0, 60.0), (30.0, 70.0)]);
        let life = carry(life, cell(12, 1), Team::One, &[(30.0, 70.0), (95.0, 4.0)]);
        assert_eq!(paid(&life.held, Team::One, team_of).collect::<Vec<_>>(), [(cell(12, 1), 6 * POINT)]);
        assert_eq!(life.held.cells().count(), 2);
    }
}
