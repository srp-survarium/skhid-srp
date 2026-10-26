//! Play 22: Heal behind cover (`player_bots_behaviour.md` §3): a wounded
//! bot out of every known enemy's line of sight uses a medkit, then the
//! commander sends it back (play 10 takes a bot that cannot heal).
//!
//! - **When:** health under `below`; no enemy seen now; no enemy it knows
//!   of (by any sense, a callout or the radar) with a clear line to where
//!   its crouched eyes are (`Tactics::sees`); a medkit in its kit's slot.
//! - **How:** the quick-use key of the kit's medkit (its slot's place in
//!   the client's order, `stock::Held::quick`: the first filled slot of
//!   the bots' kits, `survarium::v026g::loadouts::BOT_MEDKIT`) pressed
//!   once, then still and crouched for `wait_min_s` to `wait_max_s`, fire
//!   held; an enemy seen meanwhile ends it.
//! - **Medkits per kit:** the kit's medkits are the match's, not a life's
//!   (the game takes a used one out of the kit's storage and gives none
//!   back at a spawn): `BotFacts::pantry` counts them, corrected by the
//!   worlds (`stock.rs`), and a press is only offered with one in the
//!   slot, so an empty slot is never pressed.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};
use crate::v026g::bots::stock::Drug;
use crate::v026g::bots::view::EYE_CROUCHED;
use crate::v026g::drive::key;

pub const NUMBERS: &[(&str, f32)] = &[
    ("below", 0.6),
    ("wait_min_s", 1.5),
    ("wait_max_s", 3.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![
            // The more wounded, the sooner; never 0.
            Consideration::new(
                Input::Health,
                0.0,
                1.0,
                Curve::Piecewise {
                    points: vec![(0.0, 1.0), (1.0, 0.3)],
                },
            ),
            considerations::cooldown(5.0),
        ],
        numbers: Default::default(),
    }
}

/// Medkits in the slots of its kit now.
pub fn medkits_left(cx: &Ctx) -> u32 {
    u32::from(cx.bot.pantry.in_hand(Drug::Medkit))
}

/// Whether an enemy it knows of has a clear line to it (crouched), or is
/// seen now.
pub fn in_known_sight(cx: &Ctx) -> bool {
    if !cx.memory.all_seen(cx.now).is_empty() {
        return true;
    }
    let me = cx.me.feet + Vec3::new(0.0, EYE_CROUCHED, 0.0);
    cx.memory.tracks().iter().any(|track| {
        let eye = track.at + Vec3::new(0.0, crate::v026g::bots::view::EYE_STANDING, 0.0);
        cx.tactics.sees(eye, me)
    })
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    let running = matches!(cx.bot.current, Some((PlayKind::Heal, _, _)));
    if !running
        && (cx.me.health >= cx.num(PlayKind::Heal, "below")
            || cx.me.carrying.is_some()
            || medkits_left(cx) == 0)
    {
        return;
    }
    // Seen or in a known line of sight: not here (play 10 takes it).
    if in_known_sight(cx) {
        return;
    }
    out.extend(cx.candidate(PlayKind::Heal, TargetRef::None, None));
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Press the medkit's key.
    Use,
    /// Still and crouched while it works.
    Wait { until: u32 },
}

pub fn start(_target: TargetRef, _cx: &Ctx, _rng: &mut Rng) -> State {
    State::Use
}

pub fn step(state: &mut State, _target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    if !cx.memory.all_seen(cx.now).is_empty() {
        return Step::Failed("an enemy is in sight");
    }
    let still = Intent::stay().crouched().hold_fire();
    match state {
        State::Use => {
            let Some(held) = cx.bot.pantry.first(Drug::Medkit) else {
                return Step::Failed("no medkit left");
            };
            let wait = rng.range(
                cx.num(PlayKind::Heal, "wait_min_s"),
                cx.num(PlayKind::Heal, "wait_max_s"),
            );
            *state = State::Wait {
                until: cx.now + (wait * 1000.0) as u32,
            };
            Step::Continue(still.pressing(key::QUICK[held.quick]))
        }
        State::Wait { until } => {
            if cx.now >= *until {
                return Step::Done("healed");
            }
            Step::Continue(still)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::know::Source;
    use crate::v026g::bots::plays::tests::Scenario;
    use crate::v026g::bots::view::Mode;

    #[test]
    fn a_wounded_bot_out_of_sight_heals_once_and_never_in_sight() {
        // The field's wall across x = 40 (z 0 to 25) between the bot and
        // an enemy it heard.
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(30.0, 0.0, 10.0)).health = 0.5;
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 10.0), Some(Source::Heard));
        let best = s.candidates(0);
        assert_eq!(best[0].kind, PlayKind::Heal, "{best:#?}");
        let steps = s.run(0, PlayKind::Heal, TargetRef::None, 20, 300, 3.5);
        let presses: Vec<u32> = steps
            .iter()
            .filter_map(|s| match s {
                Step::Continue(i) => Some(i.press),
                _ => None,
            })
            .collect();
        assert_eq!(
            presses.iter().filter(|p| **p & key::QUICK[0] != 0).count(),
            1
        );
        assert!(steps.iter().all(|s| match s {
            Step::Continue(i) => i.crouch && !i.fire && i.go == Go::Stay,
            _ => true,
        }));
        assert_eq!(steps.last(), Some(&Step::Done("healed")));
        // 1.5 to 3 s of waiting.
        assert!((6..=12).contains(&steps.len()), "{}", steps.len());
        // The cooldown holds it back for a while, then the kit's second
        // medkit is offered; with both gone it never is, in any life.
        s.last_ran.push((PlayKind::Heal, s.view.clock));
        s.current = None;
        s.pantry.pressed(0, s.view.clock);
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Heal));
        s.view.clock += 6_000;
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 10.0), Some(Source::Heard));
        assert!(s.candidates(0).iter().any(|c| c.kind == PlayKind::Heal));
        s.pantry.pressed(0, s.view.clock);
        s.view.clock += 6_000;
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 10.0), Some(Source::Heard));
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Heal));
        s.pantry.new_life(0);
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Heal));
    }

    #[test]
    fn no_heal_in_a_known_line_of_sight_or_when_healthy() {
        // Past the wall's end: the heard enemy has a clear line.
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(30.0, 0.0, 35.0)).health = 0.5;
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 35.0), Some(Source::Heard));
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Heal));
        // Seen: not either.
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(30.0, 0.0, 10.0)).health = 0.5;
        s.enemy(1, 1, Vec3::new(60.0, 0.0, 10.0), Some(Source::Seen));
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Heal));
        // Healthy: not either.
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(30.0, 0.0, 10.0)).health = 0.9;
        assert!(s.candidates(0).iter().all(|c| c.kind != PlayKind::Heal));
    }
}
