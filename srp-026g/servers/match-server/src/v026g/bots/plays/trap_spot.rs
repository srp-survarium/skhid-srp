//! [A324] The camper trap job's play (`traps.camper_job`): the team sent this bot to put a trap on a held spot's point (his way
//! back to the spot, or his death point there, `traps::camper_tick`); it walks there and stays on it while the trap habit looks
//! down and presses (`traps::think`, with the job's own gate). It never aims or shoots: the reflex fires on sight. The team ends
//! it (the trap down, the point's time over, the bot let go), not the play.

use super::*;

/// The slot keys of the camper trap jobs start here: one a held spot.
pub const JOB_KEY: u32 = 4_000_000;
/// A job's bot goes above this health (a flanker's gate).
const JOB_HEALTH: f32 = 0.4;

pub const NUMBERS: &[(&str, f32)] = &[
    // At its point within this, metres: the habit then plans the trap there (its look lands about 1.3 m out).
    ("reach_m", 1.0),
    // A play this old is given up, seconds (the team lets the bot go after `camper_job.give_up_s` first).
    ("give_up_s", 40.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![considerations::health_above(JOB_HEALTH), considerations::runtime(40.0)],
        numbers: Default::default(),
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if cx.tuning.traps.camper_job.is_none() {
        return;
    }
    let Some(point) = crate::v026g::bots::traps::camper_point(cx.team, cx.me.place) else {
        return;
    };
    out.extend(cx.job_candidate(PlayKind::TrapSpot, TargetRef::ThreatSpot(point.spot), JOB_KEY + point.spot, JOB_HEALTH));
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
    let Some(point) = crate::v026g::bots::traps::camper_point(cx.team, cx.me.place).filter(|p| target == TargetRef::ThreatSpot(p.spot)) else {
        return Step::Done("the camper trap job ended");
    };
    let reach = num("reach_m");
    if cx.me.feet.distance_xz(point.at) <= reach {
        // On the point: the habit looks down and presses; the body stays.
        return Step::Continue(Intent::stay());
    }
    Step::Continue(Intent::to(point.at, Pace::Run).within(reach))
}
