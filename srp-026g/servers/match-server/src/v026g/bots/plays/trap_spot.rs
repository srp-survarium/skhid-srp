//! [A324] The camper trap job's play (`traps.camper_job`): the team sent this bot to put a trap on the way in of a place an
//! enemy holds (`traps::camper_tick`); it walks there and stays on it, crouched, while the trap habit looks down and presses
//! (`traps::think`, with the job's own gate). It never aims or shoots: the reflex fires on sight. The team ends it (the trap
//! down, the point's time over, the bot let go), not the play.

use super::*;

/// The slot keys of the camper trap jobs start here: one a held spot.
pub const JOB_KEY: u32 = 4_000_000;
/// A job's bot goes above this health (a flanker's gate).
const JOB_HEALTH: f32 = 0.4;

pub const NUMBERS: &[(&str, f32)] = &[
    // At its point within this, metres: the habit then plans the trap there (its look lands about 1.3 m out).
    ("reach_m", 1.0),
    // A play this old is given up, seconds (the team lets the bot go after `camper_job.give_up_s` first).
    ("give_up_s", 60.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![considerations::health_above(JOB_HEALTH), considerations::runtime(60.0)],
        numbers: Default::default(),
    }
}

/// [bot traps T3] The play is offered on each open point of a place still held within the job's `max_walk_m` to a bot that may
/// place a trap (`BotFacts::may_trap`), alive and not carrying; and on its own point to the bot running it. The commander's pass
/// gives it (one bot a point, its slot) by its bands, commits and margin; the job reads who runs it (`traps::camper_tick`).
pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    use crate::v026g::bots::know::{CamperHold, CamperTrapState};
    let Some(j) = cx.tuning.traps.camper_job else {
        return;
    };
    let running = match cx.bot.current {
        Some((PlayKind::TrapSpot, TargetRef::ThreatSpot(spot), _)) => crate::v026g::bots::traps::camper_job_point(cx.team, cx.me.place, spot),
        _ => None,
    };
    if let Some(point) = running {
        out.extend(cx.job_candidate(PlayKind::TrapSpot, TargetRef::ThreatSpot(point.spot), JOB_KEY + point.spot, JOB_HEALTH));
        return;
    }
    if !cx.bot.may_trap || !cx.me.alive || cx.me.carrying.is_some() {
        return;
    }
    for point in cx.team.camper_traps.iter().filter(|c| c.state == CamperTrapState::Open && c.hold == CamperHold::Held) {
        if cx.me.feet.distance_xz(point.at) <= j.max_walk_m {
            out.extend(cx.job_candidate(PlayKind::TrapSpot, TargetRef::ThreatSpot(point.spot), JOB_KEY + point.spot, JOB_HEALTH));
        }
    }
}

/// The play keeps nothing of its own: the team's point is its state.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Going,
}

pub fn start(_target: TargetRef, _cx: &Ctx, _rng: &mut Rng) -> State {
    State::Going
}

pub fn step(_state: &mut State, target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::TrapSpot, name);
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (num("give_up_s") * 1000.0) as u32
    {
        return Step::Failed("trapped too long");
    }
    let TargetRef::ThreatSpot(spot) = target else {
        return Step::Done("the camper trap job ended");
    };
    let Some(point) = crate::v026g::bots::traps::camper_job_point(cx.team, cx.me.place, spot) else {
        return Step::Done("the camper trap job ended");
    };
    let reach = num("reach_m");
    if cx.me.feet.distance_xz(point.at) <= reach {
        // On the point: the habit looks down and presses; the body stays, crouched, the pose the point was found out of his
        // sight for (`traps::way_in`).
        return Step::Continue(Intent::stay().crouched());
    }
    // On the way: crouched (`Pace::Sneak`, the CROUCH key) where that hides it from him (`traps::hidden_by_crouching`), here or
    // where its walk takes it by its next think at the latest (`ThinkTuning::think_ms`, the longer gap): the crouch is down
    // before it comes into his sight standing.
    let sight = &cx.tactics.world.sight;
    let ahead_s = cx.tuning.think.think_ms.1 as f32 / 1000.0;
    let ahead = cx.me.feet + Vec3::new(cx.me.velocity[0] * ahead_s, 0.0, cx.me.velocity[1] * ahead_s);
    let crouch = [cx.me.feet, ahead].iter().any(|&at| crate::v026g::bots::traps::hidden_by_crouching(sight, point.held, at));
    Step::Continue(Intent::to(point.at, if crouch { Pace::Sneak } else { Pace::Run }).within(reach))
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::{CamperHold, CamperTrap, CamperTrapKind, CamperTrapState, SeenRefusals};
    use crate::v026g::bots::view::Mode;

    /// [bot traps T3] The camper trap job's play is offered on an open point of a held place within `max_walk_m` to a bot that may
    /// place a trap, to no bot that may not, on no kept point (its place no longer held), and to none farther off.
    #[test]
    fn the_job_play_is_offered_to_a_bot_that_may_place_within_its_walk() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.tuning.traps = crate::v026g::bots::tuning::TrapTuning::default();
        s.tuning.threat.job_score = crate::v026g::bots::tuning::Tuning::default().threat.job_score;
        s.bot(0, 0, Vec3::new(20.0, 0.0, 30.0));
        let point = |at: Vec3, hold: CamperHold| CamperTrap {
            spot: 7,
            shooter: 11,
            held: Vec3::new(45.0, 0.0, 30.0),
            at,
            kind: CamperTrapKind::Job,
            state: CamperTrapState::Open,
            hold,
            seen: SeenRefusals::default(),
        };
        s.team.camper_traps.push(point(Vec3::new(30.0, 0.0, 30.0), CamperHold::Held));
        let offered = |s: &Scenario| {
            let mut out = Vec::new();
            candidates(&s.cx(0), &mut out);
            out.iter().any(|c| c.kind == PlayKind::TrapSpot && c.target == TargetRef::ThreatSpot(7))
        };
        assert!(!offered(&s), "may not place");
        s.may_trap = true;
        assert!(offered(&s), "may place, 10 m off");
        s.team.camper_traps[0] = point(Vec3::new(30.0, 0.0, 30.0), CamperHold::Over { since: 0 });
        assert!(!offered(&s), "a kept point");
        s.team.camper_traps[0] = point(Vec3::new(60.0, 0.0, 30.0), CamperHold::Held);
        assert!(!offered(&s), "past its walk");
    }
}
