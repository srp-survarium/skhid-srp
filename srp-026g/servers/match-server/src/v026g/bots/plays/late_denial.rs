//! Play 16, late denial (the battery modes; players' word, 2026-10-14):
//! in the last minutes, with the score close or behind, take batteries out
//! of the enemy's container and drop them there. Their count falls (the
//! statistics take one off the other team when a battery is taken out), so
//! a narrow lead turns; dropped batteries can be taken back, so it pays only
//! near the end. The worked example of `player_bots_research.md` B6.
//!
//! Steps: go there (crouched for the last metres), use (bit 21) to take
//! one, drop it (bit 22) at once, the next one; done when the container is
//! empty, given up after a few uses that take nothing.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};
use crate::v026g::drive::key;

pub const NUMBERS: &[(&str, f32)] = &[
    ("reach_m", 2.0),
    ("sneak_m", 20.0),
    ("use_every_s", 1.2),
    ("tries", 4.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Objective,
        considerations: vec![
            // 0 above 4 minutes left, full in the last two; a team far
            // behind reads the time left as up to half
            // (`Input::TimeLeftBehind`): from 8 minutes left at 3 points
            // behind (in the owner's match of 2026-10-16 the bots 1-5
            // behind came to it only at 740 s of 922).
            Consideration::new(
                Input::TimeLeftBehind,
                0.0,
                240.0,
                Curve::Piecewise {
                    points: vec![(0.0, 1.0), (0.5, 1.0), (0.75, 0.3), (1.0, 0.0)],
                },
            ),
            // High behind or level, 0 two ahead.
            Consideration::new(
                Input::ScoreGap,
                -3.0,
                2.0,
                Curve::Piecewise {
                    points: vec![(0.0, 1.0), (0.6, 0.9), (0.8, 0.3), (1.0, 0.0)],
                },
            ),
            considerations::near(150.0),
            Consideration::new(Input::EnemiesNearTarget, 0.0, 3.0, Curve::Linear).inverted(),
            considerations::health_above(0.35),
            considerations::modifier(Input::Aggression, 0.5),
            considerations::cooldown(20.0),
        ],
        numbers: Default::default(),
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !cx.view.objective().has_homes() || cx.view.objective().is_actor(cx.me) {
        return;
    }
    let Some(theirs) = cx.view.containers.iter().find(|c| c.team != cx.me.team) else {
        return;
    };
    if theirs.batteries == 0 {
        return;
    }
    out.extend(cx.candidate(
        PlayKind::LateDenial,
        TargetRef::Container(theirs.id),
        Some(u32::from(theirs.id)),
    ));
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Approach,
    Steal { tries: u8, last_use: u32 },
    Drop,
}

pub fn start(_target: TargetRef, _cx: &Ctx, _rng: &mut Rng) -> State {
    State::Approach
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::LateDenial, name);
    let Some(at) = cx.point_of(target) else {
        return Step::Failed("no such container");
    };
    let count = match target {
        TargetRef::Container(id) => cx
            .view
            .containers
            .iter()
            .find(|c| c.id == id)
            .map_or(0, |c| c.batteries),
        _ => 0,
    };
    match state {
        State::Approach => {
            let d = cx.me.feet.distance_xz(at);
            if d <= num("reach_m") {
                *state = State::Steal {
                    tries: 0,
                    last_use: 0,
                };
                return Step::Continue(Intent::stay().crouched().looking(Look::use_box(at)));
            }
            let pace = if d < num("sneak_m") {
                Pace::Sneak
            } else {
                cx.pace_toward(at)
            };
            Step::Continue(
                Intent::to(at, pace)
                    .within(num("reach_m") * 0.7)
                    .into_base(),
            )
        }
        State::Steal { .. } if cx.me.carrying.is_some() => {
            *state = State::Drop;
            Step::Continue(Intent::stay().pressing(key::DROP).hold_fire())
        }
        State::Steal { .. } if count == 0 => Step::Done("container emptied"),
        State::Steal { tries, last_use } => {
            if *tries >= num("tries") as u8 {
                return Step::Failed("took nothing: empty or blocked");
            }
            let mut intent = Intent::stay().crouched().looking(Look::use_box(at));
            if cx.now >= *last_use + (num("use_every_s") * 1000.0) as u32 {
                *tries += 1;
                *last_use = cx.now;
                intent = intent.pressing(key::USE);
            }
            Step::Continue(intent)
        }
        State::Drop => {
            if cx.me.carrying.is_some() {
                return Step::Continue(Intent::stay().pressing(key::DROP).hold_fire());
            }
            *state = State::Steal {
                tries: 0,
                last_use: 0,
            };
            Step::Continue(Intent::stay().crouched().looking(Look::use_box(at)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::view::Mode;

    fn denials(s: &Scenario) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        out
    }

    /// From 4 minutes left with the score close; a team 3 behind from 8
    /// (it reads the time left as half).
    #[test]
    fn a_team_far_behind_starts_late_denial_earlier() {
        let scene = |left: f32, ours: u32, theirs: u32| {
            let mut s = Scenario::field(Mode::Batteries);
            s.bot(0, 0, Vec3::new(40.0, 0.0, 30.0));
            s.container_count(1, 3);
            s.time_left(left).points(ours, theirs);
            denials(&s)
        };
        assert!(scene(400.0, 1, 1).is_empty());
        assert!(scene(400.0, 1, 2).is_empty());
        assert_eq!(scene(400.0, 0, 3).len(), 1);
        assert_eq!(scene(200.0, 1, 1).len(), 1);
        assert!(scene(500.0, 0, 3).is_empty());
    }
}
