//! Timed batteries (mode 3) only: be at a charging station when its
//! battery comes. In mode 3 the batteries are not lying about at the
//! start: one appears every 60 ± 10 s in the spawn boxes by the 2-3
//! stations (`docs/notes/modes_maps.md`; the world gives the next one's
//! time, `MatchView::next_battery`). The players' worlds say what pays
//! there: fights 6-20 m from a station go best, and lives that keep moving
//! bring the most (0.60 batteries a life against 0.00 for those holding in
//! the enemy third), so this is a short wait at a station, not a hold.
//!
//! Steps: a spot 6-20 m from the station with a sight line to it (higher
//! ground preferred), there before the battery is due, crouched and
//! looking at the station until a few seconds after it came; then the
//! commander picks again (the new battery makes `collect` the best play
//! for the nearest bot). Two bots a station at most (two slots).

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};
use crate::v026g::bots::objective::ObjectiveClock;
use crate::v026g::bots::tactics::SpotQuery;

pub const NUMBERS: &[(&str, f32)] = &[
    ("spot_min_m", 6.0),
    ("spot_max_m", 20.0),
    // Offered this long before the battery is due, seconds.
    ("ahead_s", 25.0),
    // Kept on after it came, seconds.
    ("after_s", 4.0),
    ("give_up_s", 60.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Objective,
        considerations: vec![
            considerations::near(150.0),
            considerations::health_above(0.3),
            considerations::modifier(Input::ObjectiveUrgency, 0.6),
            Consideration::new(Input::FriendsNearTarget, 0.0, 3.0, Curve::Linear).inverted(),
        ],
        numbers: Default::default(),
    }
}

/// Seconds until the next battery; `None` outside mode 3 or before the
/// world says.
fn due_in(cx: &Ctx) -> Option<f32> {
    match cx.view.objective().clock(cx.view) {
        ObjectiveClock::Next { at_ms } => Some(at_ms as f32 / 1000.0 - cx.now as f32 / 1000.0),
        ObjectiveClock::Never | ObjectiveClock::Awaited => None,
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    let running = matches!(cx.bot.current, Some((PlayKind::Station, ..)));
    let Some(due) = due_in(cx) else {
        return;
    };
    if cx.me.carrying.is_some() || (!running && due > cx.num(PlayKind::Station, "ahead_s")) {
        return;
    }
    for (index, &at) in cx.tactics.world.map.stations.iter().enumerate() {
        if !cx.tactics.fine_goal(at, cx.now, None) && !cx.tactics.fine_goal(at, cx.now, cx.bot.side)
        {
            continue;
        }
        for k in 0..2 {
            out.extend(cx.candidate(
                PlayKind::Station,
                TargetRef::Point(at),
                Some(index as u32 * 2 + k),
            ));
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Going { spot: Vec3 },
    Waiting { spot: Vec3 },
}

pub fn start(target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    let station = cx.point_of(target).unwrap_or(cx.me.feet);
    let query = SpotQuery {
        sight_to: Some(station),
        prefer_high: 0.3,
        side: cx.bot.side,
        ..SpotQuery::ring(
            station,
            cx.num(PlayKind::Station, "spot_min_m"),
            cx.num(PlayKind::Station, "spot_max_m"),
        )
    };
    let spots = cx.tactics.spots(&query, cx.now, 3, rng);
    let spot = if spots.is_empty() {
        station
    } else {
        spots[rng.below(spots.len())].at
    };
    State::Going { spot }
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> Step {
    let Some(station) = cx.point_of(target) else {
        return Step::Failed("no such station");
    };
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (cx.num(PlayKind::Station, "give_up_s") * 1000.0) as u32
    {
        return Step::Done("waited long enough");
    }
    // The battery came a few seconds ago: the commander picks again.
    if due_in(cx).is_none_or(|due| due < -cx.num(PlayKind::Station, "after_s")) {
        return Step::Done("the battery came");
    }
    match state {
        State::Going { spot } => {
            let spot = *spot;
            if cx.me.feet.distance_xz(spot) < 1.5 {
                *state = State::Waiting { spot };
            }
            Step::Continue(Intent::to(spot, cx.pace_toward(spot)).within(1.0))
        }
        State::Waiting { .. } => {
            Step::Continue(Intent::stay().crouched().looking(Look::At(station)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::view::Mode;

    fn with_station(s: &mut Scenario) {
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            map.stations = vec![Vec3::new(40.0, 0.0, 35.0)];
        });
        s.tactics = crate::v026g::bots::tactics::Tactics::new(world, 1.0);
    }

    #[test]
    fn a_station_is_contested_in_timed_batteries_only_when_a_battery_is_due() {
        let mut s = Scenario::field(Mode::TimedBatteries);
        with_station(&mut s);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 30.0));
        // Not due yet: no.
        s.view.next_battery = Some(s.view.clock + 50_000);
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Station));
        // Due in 15 s: the best play.
        s.view.next_battery = Some(s.view.clock + 15_000);
        let best = s.candidates(0);
        assert_eq!(best[0].kind, PlayKind::Station, "{best:#?}");
        // The plain battery mode has no stations to wait at.
        s.view.mode = Mode::Batteries;
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Station));
    }

    #[test]
    fn it_waits_by_the_station_and_is_done_once_the_battery_came() {
        let mut s = Scenario::field(Mode::TimedBatteries);
        with_station(&mut s);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 30.0));
        s.view.next_battery = Some(s.view.clock + 12_000);
        let target = TargetRef::Point(Vec3::new(40.0, 0.0, 35.0));
        let steps = s.run(0, PlayKind::Station, target, 80, 300, 3.5);
        let waited = steps
            .iter()
            .filter(|s| matches!(s, Step::Continue(i) if i.go == Go::Stay && matches!(i.look, Look::At(_))))
            .count();
        assert!(waited > 5, "{steps:?}");
        let d = s.view.places[0]
            .feet
            .distance_xz(Vec3::new(40.0, 0.0, 35.0));
        assert!((5.0..=21.0).contains(&d), "{d}");
        assert_eq!(steps.last(), Some(&Step::Done("the battery came")));
    }
}
