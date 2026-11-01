//! Patch 92: a battery's carrier scores while it brings the battery nearer
//! its team's container, not only at the delivery (the owner, 2026-11-01;
//! docs/notes/battery_credit.md).
//!
//! Each battery keeps, for each team's container, the nearest it has come in
//! its life (from the match's start, or its last delivery), whoever carried
//! it: in whole steps of [`STEP_METRES`]. While a player carries it, every
//! simulation tick, the carrier is paid one point for each step its own
//! team's container's nearest comes nearer. Ground covered before, by anyone,
//! pays nothing again: walking back and forth, dropping and taking it again,
//! or handing it to a teammate earns only new ground. A delivery starts the
//! battery's life again. The point is 10 hundredths of event 6 ("bring
//! victory item"), which srp weighs 10 a delivery's 100.

#![forbid(unsafe_code)]

use crate::vostok::{statistics, Band, BatteryCarry, Context, Flow, NoExit, Progress, ScoreEvent, Team};

/// Metres of new progress a point pays.
pub const STEP_METRES: f32 = 10.0;
/// A point, in the hundredths of a delivery that event 6 counts.
const POINT: u16 = 10;

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

/// One tick of a carry: the battery is `now` from each container and
/// `carrier` carries it. Each container's nearest band of the life becomes
/// the nearer of it and `now` (the first carry of the life sets both); the
/// carrier is paid the steps by which its own team's comes nearer than ever.
fn carry(progress: Progress, carrier: Team, now: [Band; 2]) -> (Progress, u8) {
    let Progress(nearest) = progress;
    let after = core::array::from_fn(|team| Some(nearest[team].map_or(now[team], |best| best.min(now[team]))));
    let own = carrier.index();
    let steps = nearest[own].map_or(0, |best| best.steps().saturating_sub(now[own].steps()));
    (Progress(after), steps)
}

/// base_player::tick after the held object's tick.
pub fn on_carrier_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(battery) = BatteryCarry::at_tick(ctx) {
        let (progress, steps) = carry(battery.progress(), battery.team(), battery.distances().map(band));
        battery.set_progress(progress);
        if steps > 0 {
            battery.count(ScoreEvent::BRING_VICTORY_ITEM, u16::from(steps) * POINT);
        }
    }
    Flow::Continue
}

/// The match's start: every battery's life begins.
pub fn on_match_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    statistics::forget_all(ctx);
    Flow::Continue
}

/// A delivery: the battery's life begins again.
pub fn on_delivered(ctx: &mut Context<'_>) -> Flow<NoExit> {
    statistics::forget_delivered(ctx);
    Flow::Continue
}

/// A battery's record serialized: its progress after it.
pub fn on_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    statistics::save(ctx);
    Flow::Continue
}

/// A battery's record deserialized: its progress after it.
pub fn on_load(ctx: &mut Context<'_>) -> Flow<NoExit> {
    statistics::load(ctx);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bands(zero: f32, one: f32) -> [Band; 2] {
        [band(zero), band(one)]
    }

    /// Ticks of one carrier over the distances (to team 0's container, to
    /// team 1's): the progress after, and the steps paid.
    fn walk(mut progress: Progress, carrier: Team, path: &[(f32, f32)]) -> (Progress, u32) {
        let mut paid = 0;
        for &(zero, one) in path {
            let (after, steps) = carry(progress, carrier, bands(zero, one));
            progress = after;
            paid += u32::from(steps);
        }
        (progress, paid)
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
    fn the_first_carry_of_a_life_pays_nothing_and_keeps_where_it_was() {
        let (progress, steps) = carry(Progress::default(), Team::Zero, bands(73.0, 40.0));
        assert_eq!(steps, 0);
        assert_eq!(progress, Progress([Some(Band::new(7)), Some(Band::new(4))]));
    }

    #[test]
    fn a_carrier_earns_a_point_a_ten_metres_nearer() {
        // 73 m from its container to 2 m: from band 7 to band 0.
        let path: Vec<(f32, f32)> = (0..=71).map(|m| (73.0 - m as f32, 40.0)).collect();
        let (progress, paid) = walk(Progress::default(), Team::Zero, &path);
        assert_eq!(paid, 7);
        assert_eq!(progress.0[0], Some(Band::new(0)));
    }

    #[test]
    fn walking_back_and_forth_pays_once() {
        let there_and_back = [(50.0, 0.0), (41.0, 0.0), (55.0, 0.0), (41.0, 0.0), (55.0, 0.0), (39.0, 0.0)];
        let (_, paid) = walk(Progress::default(), Team::Zero, &there_and_back);
        // 50 -> 41: one step; back and forth to 41 again: none; 39: one more.
        assert_eq!(paid, 2);
    }

    #[test]
    fn a_re_pickup_earns_nothing_until_new_ground() {
        let (carried, paid) = walk(Progress::default(), Team::One, &[(0.0, 80.0), (0.0, 61.0)]);
        assert_eq!(paid, 2);
        // Dropped at 61 m and taken again (by the same player, or after it
        // rolled back to 68 m): the life's nearest stays, nothing is paid
        // until it is nearer than 60 m.
        let (carried, paid) = walk(carried, Team::One, &[(0.0, 68.0), (0.0, 62.0), (0.0, 61.0)]);
        assert_eq!(paid, 0);
        let (_, paid) = walk(carried, Team::One, &[(0.0, 59.0)]);
        assert_eq!(paid, 1);
    }

    #[test]
    fn a_teammate_continuing_a_carry_earns_only_the_extra_distance() {
        // The first carrier brings it from 90 m to 71 m (two points), is
        // killed and the battery lies there; a teammate carries it to the
        // container: the seven steps from 70 m in, not the first carrier's.
        let (first, paid) = walk(Progress::default(), Team::Zero, &[(90.0, 20.0), (71.0, 30.0)]);
        assert_eq!(paid, 2);
        let (_, paid) = walk(first, Team::Zero, &[(71.0, 30.0), (80.0, 30.0), (1.0, 80.0)]);
        assert_eq!(paid, 7);
    }

    #[test]
    fn an_enemy_carry_brings_the_nearest_for_both_teams() {
        // Team 1 carries a fresh battery from 60 m of team 0's container to
        // 25 m of it (away from its own, 40 m -> 75 m): it earns nothing,
        // and the 35 m toward team 0's container pay team 0 nothing later.
        let (taken, paid) = walk(Progress::default(), Team::One, &[(60.0, 40.0), (25.0, 75.0)]);
        assert_eq!(paid, 0);
        let (_, paid) = walk(taken, Team::Zero, &[(25.0, 75.0), (40.0, 60.0), (25.0, 75.0), (5.0, 95.0)]);
        assert_eq!(paid, 2);
    }

    #[test]
    fn two_enemies_passing_it_back_and_forth_earn_the_ground_once() {
        let mut progress = Progress::default();
        let mut paid = [0, 0];
        for _ in 0..5 {
            let (after, zero) = walk(progress, Team::Zero, &[(50.0, 50.0), (20.0, 80.0)]);
            let (after, one) = walk(after, Team::One, &[(20.0, 80.0), (50.0, 50.0)]);
            progress = after;
            paid[0] += zero;
            paid[1] += one;
        }
        assert_eq!(paid, [3, 0]);
    }

    #[test]
    fn a_delivery_starts_the_life_again() {
        let (delivered, paid) = walk(Progress::default(), Team::Zero, &[(30.0, 90.0), (1.0, 95.0)]);
        assert_eq!(paid, 3);
        // The record is reset at the put-in; the enemy steals it out of
        // team 0's container and carries it home.
        assert_ne!(delivered, Progress::default());
        let (_, paid) = walk(Progress::default(), Team::One, &[(1.0, 95.0), (60.0, 35.0), (95.0, 2.0)]);
        assert_eq!(paid, 9);
    }

    #[test]
    fn a_tick_pays_every_step_it_crosses() {
        let (progress, _) = carry(Progress::default(), Team::Zero, bands(200.0, 0.0));
        let (_, steps) = carry(progress, Team::Zero, bands(0.0, 0.0));
        assert_eq!(steps, 20);
        assert!(u16::from(Band::FARTHEST.steps()).checked_mul(POINT).is_some());
    }
}
