//! The crossers' play of a Breach (`bots/breach.rs`, design-p1c §1): the team's job gives this bot crosser `k` of the Breach on a
//! held spot; the play walks it by the job's phase: to its gather point out of his lanes and out of his sight, and waits there
//! crouched; on the go, the run to its cover point (picked before the go, from a bearing out of his lanes); there, it holds,
//! looking at his spot. It never aims or shoots: the reflex fires on sight. The job ends it (his death, a failure, the spot no
//! longer held), not the play.

use super::*;
use crate::v026g::bots::breach::{self, BreachRole, Phase};
use crate::v026g::bots::threat::Role;

/// The slot keys of a Breach's crossers start here: two a held spot.
pub const JOB_KEY: u32 = 3_000_000;
/// A crosser goes above this health: the play's own gate (as a flanker's).
const JOB_HEALTH: f32 = 0.4;

pub const NUMBERS: &[(&str, f32)] = &[
    // At a point within this, metres.
    ("reach_m", 1.0),
    // A play this old is given up, seconds (the job's own times end it first: gather 8, go 6, hold 10).
    ("give_up_s", 45.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![considerations::health_above(JOB_HEALTH), considerations::runtime(45.0)],
        numbers: Default::default(),
    }
}

/// The crosser's job: its Breach and its number there, while the job runs.
fn crosser<'a>(cx: &Ctx<'a>) -> Option<(&'a breach::Breach, usize)> {
    match breach::role_in(&cx.team.breaches, cx.me.place) {
        Some((b, BreachRole::Crosser(k))) if !b.phase.ended() => Some((b, k)),
        _ => None,
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if cx.tuning.threat.breach.is_none() {
        return;
    }
    let Some((b, k)) = crosser(cx) else {
        return;
    };
    out.extend(cx.job_offer(Role::Crosser, PlayKind::Breach, TargetRef::ThreatSpot(b.spot), JOB_KEY + b.spot * 2 + k as u32, JOB_HEALTH, true));
}

/// The play keeps nothing of its own: the job's phase and points are its state.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Crossing,
}

pub fn start(_target: TargetRef, _cx: &Ctx, _rng: &mut Rng) -> State {
    State::Crossing
}

pub fn step(_state: &mut State, target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Breach, name);
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (num("give_up_s") * 1000.0) as u32
    {
        return Step::Failed("crossed too long");
    }
    let Some((b, k)) = crosser(cx).filter(|(b, _)| target == TargetRef::ThreatSpot(b.spot)) else {
        return Step::Done("the breach ended");
    };
    let c = b.crossers[k];
    // His spot, at his chest.
    let him = b.at + Vec3::new(0.0, 1.3, 0.0);
    let reach = num("reach_m");
    match b.phase {
        // Out of his sight: crouched, facing the way it will go.
        Phase::Gather { .. } if cx.me.feet.distance_xz(c.gather) <= breach::AT_GATHER_M => {
            Step::Continue(Intent::stay().crouched().looking(Look::At(c.cover + Vec3::new(0.0, 1.3, 0.0))))
        }
        Phase::Gather { .. } => Step::Continue(Intent::to(c.gather, Pace::Cruise).within(reach)),
        // The go: the run to its cover point, his spot in view.
        Phase::Go { .. } => Step::Continue(Intent::to(c.cover, Pace::Run).within(reach).looking(Look::At(him))),
        Phase::Hold { .. } if cx.me.feet.distance_xz(c.cover) <= breach::AT_COVER_M => Step::Continue(Intent::stay().looking(Look::At(him))),
        Phase::Hold { .. } => Step::Continue(Intent::to(c.cover, Pace::Run).within(reach).looking(Look::At(him))),
        Phase::Done { .. } | Phase::Failed { .. } => Step::Done("the breach ended"),
    }
}
