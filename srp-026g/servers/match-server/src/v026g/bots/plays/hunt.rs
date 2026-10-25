//! The Team Deathmatch endgame: the enemy's tickets are out, they can no
//! longer respawn, and the match ends when the last of them dies
//! (`rules::tdm_finished`). The bots go after the survivors.
//!
//! Each step heads for the nearest enemy the bot knows of, however it
//! knows (seen, heard, told, the radar cheat): the radar speaks while
//! nothing is sensed, so the hunt never stalls for long. Near the last
//! known place it walks and looks about; the reflex shoots once one is
//! seen and noticed. Rank Alarm: it ends the match.
//!
//! The other side of it, a team that can no longer respawn, plays it safe:
//! its aggression is held low by the commander (`Bots::commander`).

use super::*;

pub const NUMBERS: &[(&str, f32)] = &[("near_m", 25.0), ("search_s", 6.0)];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Alarm,
        considerations: vec![considerations::health_above(0.2)],
        numbers: Default::default(),
    }
}

/// Whether the enemies of `cx`'s bot can no longer respawn in a Team
/// Deathmatch.
fn endgame(cx: &Ctx) -> bool {
    cx.view.mode.is_deathmatch() && !cx.view.can_respawn[usize::from(!cx.me.team & 1)]
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !endgame(cx) || cx.view.enemies_of(cx.me.team).next().is_none() {
        return;
    }
    out.extend(cx.candidate(PlayKind::Hunt, TargetRef::None, None));
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Seek,
    /// At a last known place, looking about until.
    Search {
        until: u32,
    },
}

pub fn start(_target: TargetRef, _cx: &Ctx, _rng: &mut Rng) -> State {
    State::Seek
}

pub fn step(state: &mut State, _target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    if !endgame(cx) || cx.view.enemies_of(cx.me.team).next().is_none() {
        return Step::Done("no one left to hunt");
    }
    let nearest = cx.threats().first().map(|t| t.at);
    match state {
        State::Seek => {
            let Some(at) = nearest else {
                // Nothing known yet: the radar will speak; look about.
                return Step::Continue(Intent::stay());
            };
            if cx.me.feet.distance_xz(at) < 3.0 {
                let search = rng.range(0.6, 1.4) * cx.num(PlayKind::Hunt, "search_s");
                *state = State::Search {
                    until: cx.now + (search * 1000.0) as u32,
                };
                return Step::Continue(Intent::stay());
            }
            let pace = if cx.me.feet.distance_xz(at) < cx.num(PlayKind::Hunt, "near_m") {
                Pace::Walk
            } else {
                Pace::Run
            };
            Step::Continue(Intent::to(at, pace).within(2.0).into_base())
        }
        State::Search { until } => {
            if cx.now >= *until || nearest.is_some_and(|at| cx.me.feet.distance_xz(at) > 6.0) {
                *state = State::Seek;
            }
            Step::Continue(Intent::stay())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::Source;
    use crate::v026g::bots::view::Mode;

    #[test]
    fn the_hunt_is_on_only_in_a_deathmatch_whose_enemies_cannot_respawn() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 30.0), Some(Source::Heard));
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Hunt));
        s.view.can_respawn = [true, false];
        let best = s.candidates(0);
        assert_eq!(best[0].kind, PlayKind::Hunt, "{best:#?}");
        // It heads for the last one it knows of.
        let steps = s.run(0, PlayKind::Hunt, TargetRef::None, 3, 300, 3.5);
        assert!(
            matches!(steps[0], Step::Continue(Intent { go: Go::To { at, .. }, .. }) if at.distance_xz(Vec3::new(60.0, 0.0, 30.0)) < 1.0),
            "{steps:?}"
        );
        // Not in the battery modes.
        let mut b = Scenario::field(Mode::Batteries);
        b.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
        b.enemy(1, 1, Vec3::new(60.0, 0.0, 30.0), Some(Source::Heard));
        b.view.can_respawn = [true, false];
        assert!(b.candidates(0).iter().all(|c| c.kind != PlayKind::Hunt));
    }
}
