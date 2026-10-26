//! Play 24, raid (the battery modes): take a battery out of the enemy's
//! container and bring it home, at any time of the match when their
//! container is weakly held. The statistics take one off their team when a
//! battery is taken out and give one to ours when it is put in, so a raid
//! that comes home swings the score by two.
//!
//! Why (the players' matches of 6+ players, 2026-10-14, private study): the
//! winners took batteries out of the enemy's container in 13 of the 16
//! matches where either side did, about 2.6 times as often as the losers,
//! and mostly in the middle and last thirds of the match, not only at its
//! end (play 16, late denial, drops them there instead, for the last
//! minutes). The battery modes were won by the batteries (the winners
//! picked up more in 20 of 25 matches and lost fewer on the way home), not
//! by the kills (more kills in only 64 % of them).
//!
//! It wants the enemy thinned out (some of them dead, waiting to respawn)
//! and none known by their container. Steps: go there, crouched for the
//! last metres, use (bit 21) until it carries one, then it is done: a
//! carrier's `Collect` (the alarm band) brings it home, and the partner
//! (play 25) and escorts (play 3) come along.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};
use crate::v026g::bots::view;
use crate::v026g::drive::key;

pub const NUMBERS: &[(&str, f32)] = &[
    ("reach_m", 2.0),
    ("sneak_m", 20.0),
    ("use_every_s", 1.2),
    // At the container the eye steps in to this from the use box's
    // middle (the use reaches 2 m).
    ("use_near_m", 1.6),
    ("tries", 4.0),
    // Not in the first this many seconds of the match (the batteries are
    // out on the map then, not in containers).
    ("after_s", 120.0),
    // A team this far behind late in the match (`Input::BehindLate`)
    // sends a second raider (two slots).
    ("pair_behind", 0.5),
    ("give_up_s", 90.0),
    // A running raid is left once this many enemies are known within
    // `guard_m` of their container (a take needs fewer than two within
    // 25 m); 2 within 15 m or 3 within 15 m measured no better.
    ("guarded", 3.0),
    ("guard_m", 25.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Objective,
        considerations: vec![
            // Their team thinned out: some dead and waiting (none dead: a
            // fifth, for a team behind enough to push, `pushing`).
            Consideration::new(
                Input::EnemiesDead,
                0.0,
                1.0,
                Curve::Piecewise {
                    points: vec![(0.0, 0.2), (0.2, 0.5), (0.5, 1.0), (1.0, 1.0)],
                },
            ),
            considerations::pushing(),
            Consideration::new(Input::EnemiesNearTarget, 0.0, 2.0, Curve::Linear).inverted(),
            considerations::near(180.0),
            considerations::health_above(0.5),
            considerations::personality(),
            considerations::cooldown(30.0),
        ],
        numbers: Default::default(),
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !cx.view.mode.has_batteries() || cx.me.carrying.is_some() {
        return;
    }
    let elapsed = cx.view.total_s - cx.view.time_left_s;
    if elapsed < cx.num(PlayKind::Raid, "after_s") {
        return;
    }
    let Some(theirs) = cx.view.containers.iter().find(|c| c.team != cx.me.team) else {
        return;
    };
    if theirs.batteries == 0 {
        return;
    }
    let target = TargetRef::Container(theirs.id);
    out.extend(raid_candidate(cx, target, u32::from(theirs.id)));
    // Behind late: a second bot raids too (holding gains nothing then).
    if cx.sig.behind_late() >= cx.num(PlayKind::Raid, "pair_behind") && theirs.batteries >= 2 {
        out.extend(raid_candidate(cx, target, 0x100 | u32::from(theirs.id)));
    }
}

/// The raid at their container `target` on `slot`: scored by all its
/// considerations to be taken; once it runs, committed while it is
/// healthy and their container is not `guarded`, with the window it was
/// taken in left out (their dead, none known by their container).
///
/// Why (PROVEN, 2026-10-23): a raid's candidate held only while the
/// window it was taken in held, and the walk there takes 15-60 s. Live
/// (2026-10-20..23) 11,387 raids ran a median of 4.5 s and 43 stole; the
/// commander replaced them (Roam, Buddy, Collect, ...) as soon as a second
/// enemy was known within 25 m of their container or their dead came
/// back; a raid rarely ended by its own steps. In the stand-in's whole
/// matches 372 of 469 raids left were no candidate any more, 42 more
/// fell under the keep bar. Committed (real maps, 8 maps, 4v4 and 7v7,
/// 5 seeds, 600 s): raids that came within 2 m 10 -> 32, raid steals
/// 10 -> 30 (battery cheat on: 8 -> 23); 30 of the 32 that came within
/// 2 m stole. The stand-in field still gives none: its defenders stand by
/// their container (3 or more within 25 m most of the time) and their
/// spawn is 7 m from it.
fn raid_candidate(cx: &Ctx, target: TargetRef, slot: u32) -> Option<Candidate> {
    let running = matches!(cx.bot.current, Some((PlayKind::Raid, t, _)) if t == target);
    let committed = running
        && cx.point_of(target).is_some_and(|at| {
            let near = cx.enemies_near(at, cx.num(PlayKind::Raid, "guard_m"));
            (near as f32) < cx.num(PlayKind::Raid, "guarded")
        });
    if committed {
        cx.candidate_given(
            PlayKind::Raid,
            target,
            Some(slot),
            &[(Input::EnemiesDead, 1.0), (Input::EnemiesNearTarget, 0.0)],
        )
    } else {
        cx.candidate(PlayKind::Raid, target, Some(slot))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Approach,
    Take { tries: u8, last_use: u32 },
}

pub fn start(_target: TargetRef, _cx: &Ctx, _rng: &mut Rng) -> State {
    State::Approach
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Raid, name);
    if cx.me.carrying.is_some() {
        return Step::Done("took one; home with it");
    }
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (num("give_up_s") * 1000.0) as u32
    {
        return Step::Failed("took too long");
    }
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
    if count == 0 {
        return Step::Done("their container is empty");
    }
    match state {
        State::Approach => {
            let d = cx.me.feet.distance_xz(at);
            if d <= num("reach_m") {
                *state = State::Take {
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
        State::Take { tries, last_use } => {
            if *tries >= num("tries") as u8 {
                return Step::Failed("took nothing: blocked");
            }
            let mut intent = Intent::stay().crouched().looking(Look::use_box(at));
            // The use is a 2 m ray from the eye to the use box's middle
            // (`Look::use_box`): from where the approach stopped (up to
            // `reach_m` off) it can fall short, so it steps in as a
            // put-in does (`collect::USE_NEAR_M`) to `use_near_m`.
            let eye = view::still_eye(cx.me.feet, true);
            if (at + Vec3::new(0.0, 1.0, 0.0) - eye).length() > num("use_near_m") {
                intent = Intent::to(at, Pace::Sneak)
                    .within(0.5)
                    .crouched()
                    .looking(Look::use_box(at));
            }
            if cx.now >= *last_use + (num("use_every_s") * 1000.0) as u32 {
                *tries += 1;
                *last_use = cx.now;
                intent = intent.pressing(key::USE);
            }
            Step::Continue(intent)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::Source;
    use crate::v026g::bots::view::Mode;

    /// Bot 0 at (40, 0, 30); the enemy's container (68, 0, 30) holds 3;
    /// enemies 1 and 3, `dead` of them dead.
    fn raid(dead: usize) -> Scenario {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(40.0, 0.0, 30.0));
        s.enemy(1, 1, Vec3::new(80.0, 0.0, 70.0), None);
        s.enemy(3, 1, Vec3::new(85.0, 0.0, 75.0), None);
        for p in [1, 3].into_iter().take(dead) {
            s.view.places[p].alive = false;
        }
        s.container_count(1, 3);
        s.signals();
        s
    }

    fn raids(s: &Scenario) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        out
    }

    /// Far behind late in the match a second bot raids (a second slot),
    /// while their container holds two or more; not when level.
    #[test]
    fn a_team_far_behind_late_sends_two_raiders() {
        let slots = |s: &Scenario| raids(s).iter().map(|c| c.slot).collect::<Vec<_>>();
        let mut s = raid(1);
        s.time_left(100.0).points(0, 3);
        let two = slots(&s);
        assert_eq!(two.len(), 2, "{two:?}");
        assert_ne!(two[0], two[1]);
        s.container_count(1, 1);
        assert_eq!(slots(&s).len(), 1);
        let mut s = raid(1);
        s.time_left(100.0).points(3, 3);
        assert_eq!(slots(&s).len(), 1);
    }

    #[test]
    fn a_raid_wants_batteries_there_the_enemy_thinned_and_their_container_clear() {
        let one_dead = raids(&raid(1));
        assert_eq!(one_dead.len(), 1);
        assert_eq!(one_dead[0].target, TargetRef::Container(1));
        // None of them dead: a raid is a fifth as good (a team behind
        // pushes anyway, `considerations::pushing`).
        let none_dead = raids(&raid(0));
        assert!(
            none_dead[0].score.get() < one_dead[0].score.get() * 0.5,
            "none of them dead"
        );
        // A team behind on points raids more, and holds less.
        let mut even = raid(1);
        even.mods.aggression = 0.5;
        let mut behind = raid(1);
        behind.mods.aggression = 1.0;
        assert!(raids(&behind)[0].score.get() > raids(&even)[0].score.get() * 1.1);
        let holding = crate::v026g::bots::plays::considerations::holding();
        let at = |aggression: f32| holding.eval(aggression).get();
        assert!(at(1.0) < at(0.5) * 0.6 && at(0.5) < at(0.0));
        assert!(raids(&raid(2))[0].score.get() >= one_dead[0].score.get());
        // An enemy known by their container: much less.
        let mut s = raid(1);
        s.enemy(3, 1, Vec3::new(70.0, 0.0, 31.0), Some(Source::Seen));
        let guarded = raids(&s);
        assert!(guarded.is_empty() || guarded[0].score.get() < one_dead[0].score.get() * 0.7);
        // Empty, or too early, or a carrier: none.
        let mut s = raid(1);
        s.container_count(1, 0);
        assert!(raids(&s).is_empty());
        let mut s = raid(1);
        s.view.time_left_s = s.view.total_s - 60.0;
        assert!(raids(&s).is_empty());
        let mut s = raid(1);
        s.view.places[0].carrying = Some(0);
        assert!(raids(&s).is_empty());
        // Never in Team Deathmatch.
        let mut s = raid(1);
        s.view.mode = Mode::TeamDeathmatch;
        assert!(raids(&s).is_empty());
    }

    #[test]
    fn it_goes_there_takes_one_and_is_done_once_it_carries() {
        let mut s = raid(1);
        let steps = s.run(0, PlayKind::Raid, TargetRef::Container(1), 30, 250, 5.0);
        let used = steps
            .iter()
            .any(|s| matches!(s, Step::Continue(i) if i.press & key::USE != 0));
        assert!(used, "{steps:?}");
        assert!(matches!(steps.last(), Some(Step::Continue(_))), "{steps:?}");
        s.view.places[0].carrying = Some(2);
        let steps = s.run(0, PlayKind::Raid, TargetRef::Container(1), 2, 250, 5.0);
        assert_eq!(steps[0], Step::Done("took one; home with it"));
        // A carrier's next play brings it home.
        assert_eq!(s.candidates(0)[0].kind, PlayKind::Collect);
    }
}
