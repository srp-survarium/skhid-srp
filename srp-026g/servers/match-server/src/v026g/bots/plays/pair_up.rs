//! Play 13: The top player (`player_bots_behaviour.md` §3, §4 "an enemy on
//! a kill streak"): normal bots never duel the best enemy alone; they pair
//! up.
//!
//! The top player is the enemy with the most kills and streak on the
//! scoreboard (fair: every player sees it). When he is known near (the
//! memory: seen, heard, told), the bot goes to stand within `pair_m` of a
//! teammate who is nearer to him, and stays by that teammate while he is
//! near (the same teammate, though once beside him it may stand the nearer
//! of the two); with no such teammate it waits back, watching where he
//! was, for a drawn while. It never aims or shoots: the reflex does,
//! beside a friend.
//!
//! The slot is the bot pair (the bot and the teammate it joins), so two
//! bots do not run to each other. A hunter squad that goes for the top
//! players on purpose is not built.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};

pub const NUMBERS: &[(&str, f32)] = &[
    // The scoreboard's worth: kills + this times the streak.
    ("streak_weight", 2.0),
    // The top player's worth is at least this.
    ("top_min", 8.0),
    // And this far over the next enemy's.
    ("top_lead", 3.0),
    // He is near when known within this of the bot, metres.
    ("near_m", 45.0),
    // Stand within this of the teammate, metres.
    ("pair_m", 8.0),
    // A teammate is nearer to him by at least this, metres.
    ("nearer_by_m", 3.0),
    // A teammate farther than this from the bot is not joined, metres.
    ("mate_max_m", 50.0),
    // No teammate: waiting this long, seconds.
    ("wait_min_s", 5.0),
    ("wait_max_s", 10.0),
    ("give_up_s", 40.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![
            // Every personality pairs up against him; support most.
            Consideration::new(
                Input::Personality,
                0.0,
                2.0,
                Curve::Piecewise {
                    points: vec![(0.0, 0.5), (0.5, 0.85), (1.0, 1.0)],
                },
            ),
            considerations::cooldown(10.0),
            considerations::runtime(45.0),
        ],
        numbers: Default::default(),
    }
}

/// The scoreboard's top enemy, if he stands out.
fn top_player(cx: &Ctx) -> Option<Place> {
    let num = |name| cx.num(PlayKind::PairUp, name);
    let worth = |p: &PlaceView| p.kills as f32 + num("streak_weight") * p.streak as f32;
    let mut enemies: Vec<(Place, f32)> = cx
        .view
        .places
        .iter()
        .filter(|p| p.present && p.team != cx.me.team)
        .map(|p| (p.place, worth(p)))
        .collect();
    enemies.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (top, best) = *enemies.first()?;
    let next = enemies.get(1).map_or(0.0, |e| e.1);
    (best >= num("top_min") && best - next >= num("top_lead")).then_some(top)
}

/// The teammate to join against him at `at`: alive, nearer to him than
/// the bot, the nearest such to the bot.
fn mate<'a>(cx: &Ctx<'a>, at: Vec3) -> Option<&'a PlaceView> {
    let num = |name| cx.num(PlayKind::PairUp, name);
    let mine = cx.me.feet.distance_xz(at);
    cx.view
        .team(cx.me.team)
        .filter(|p| {
            p.place != cx.me.place
                && p.feet.distance_xz(at) + num("nearer_by_m") <= mine
                && p.feet.distance_xz(cx.me.feet) <= num("mate_max_m")
        })
        .min_by(|a, b| {
            a.feet
                .distance_xz(cx.me.feet)
                .total_cmp(&b.feet.distance_xz(cx.me.feet))
        })
}

/// The pair's slot key: the two places, lower first.
fn pair_key(a: Place, b: Place) -> u32 {
    let (lo, hi) = (a.min(b) as u32, a.max(b) as u32);
    lo * 1_024 + hi
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    let Some(top) = top_player(cx) else {
        return;
    };
    let target = TargetRef::Place(top);
    let running = matches!(cx.bot.current, Some((PlayKind::PairUp, t, _)) if t == target);
    let Some(at) = cx.known_place(top) else {
        return;
    };
    if !running && at.distance_xz(cx.me.feet) > cx.num(PlayKind::PairUp, "near_m") {
        return;
    }
    let slot = mate(cx, at).map(|m| pair_key(cx.me.place, m.place));
    out.extend(cx.candidate(PlayKind::PairUp, target, slot));
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// By the teammate who was nearer to him when it joined him: kept while
    /// he lives and stays within `mate_max_m`. Asking again for one nearer
    /// ended it as soon as it stood beside him a step ahead (the audit of
    /// three real 7v7 matches: "no teammate nearer to him now" 10-29 times
    /// a match, mostly within the second).
    Pair { mate: Place },
    /// No teammate: back, watching.
    Wait { until: u32 },
}

pub fn start(target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    if let Some(m) = cx.point_of(target).and_then(|at| mate(cx, at)) {
        State::Pair { mate: m.place }
    } else {
        let s = rng.range(
            cx.num(PlayKind::PairUp, "wait_min_s"),
            cx.num(PlayKind::PairUp, "wait_max_s"),
        );
        State::Wait {
            until: cx.now + (s * 1000.0) as u32,
        }
    }
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::PairUp, name);
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (num("give_up_s") * 1000.0) as u32
    {
        return Step::Done("paired long enough");
    }
    let Some(him) = cx.point_of(target) else {
        return Step::Done("he is gone from the memory");
    };
    if him.distance_xz(cx.me.feet) > num("near_m") * 1.5 {
        return Step::Done("he is far now");
    }
    match state {
        State::Pair { mate } => {
            let Some(mate) = cx
                .view
                .team(cx.me.team)
                .find(|p| p.place == *mate)
                .map(|p| p.feet)
                .filter(|at| at.distance_xz(cx.me.feet) <= num("mate_max_m"))
            else {
                return Step::Done("its teammate is down or gone");
            };
            let pair = num("pair_m");
            if cx.me.feet.distance_xz(mate) <= pair {
                return Step::Continue(Intent::stay().looking(Look::At(him)));
            }
            Step::Continue(
                Intent::to(mate, cx.pace_toward(mate))
                    .within(pair * 0.6)
                    .looking(Look::At(him)),
            )
        }
        State::Wait { until } => {
            if let Some(m) = mate(cx, him) {
                *state = State::Pair { mate: m.place };
                return Step::Continue(Intent::stay().looking(Look::At(him)));
            }
            if cx.now >= *until {
                return Step::Done("no one came");
            }
            Step::Continue(Intent::stay().crouched().looking(Look::At(him)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::Source;
    use crate::v026g::bots::view::Mode;

    /// Bot 0 at (20, 0, 35); teammate bot 2 at (32, 0, 33); the top enemy
    /// 1 (12 kills, streak 4) known at (50, 0, 32); a plain enemy 3.
    fn top() -> Scenario {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 35.0));
        s.bot(2, 0, Vec3::new(32.0, 0.0, 33.0));
        s.enemy(1, 1, Vec3::new(50.0, 0.0, 32.0), Some(Source::Seen));
        s.enemy(3, 1, Vec3::new(60.0, 0.0, 35.0), None);
        let p = &mut s.view.places[1];
        p.kills = 12;
        p.streak = 4;
        s.view.places[3].kills = 3;
        s
    }

    fn pair_ups(s: &Scenario, place: Place) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(place), &mut out);
        out
    }

    #[test]
    fn the_top_player_known_near_makes_a_pair_slot() {
        let s = top();
        let out = pair_ups(&s, 0);
        assert_eq!(out.len(), 1, "{out:#?}");
        assert_eq!(out[0].target, TargetRef::Place(1));
        assert_eq!(out[0].rank, Rank::Fight);
        assert_eq!(out[0].slot.map(|k| k.key), Some(pair_key(0, 2)));
        // The nearer bot has no one nearer: a candidate to wait, no slot.
        let out = pair_ups(&s, 2);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].slot, None);
    }

    #[test]
    fn it_pairs_up_alike_in_every_mode() {
        let score = |mode| {
            let mut s = top();
            s.view.mode = mode;
            pair_ups(&s, 0)[0].score.get()
        };
        let tdm = score(Mode::TeamDeathmatch);
        assert_eq!(tdm, score(Mode::Batteries));
        assert_eq!(tdm, score(Mode::TimedBatteries));
    }

    #[test]
    fn no_pair_up_without_a_standout_or_when_he_is_unknown_or_far() {
        let mut s = top();
        s.view.places[1].kills = 4;
        s.view.places[1].streak = 0;
        assert!(pair_ups(&s, 0).is_empty(), "no standout");
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 35.0));
        s.enemy(1, 1, Vec3::new(50.0, 0.0, 32.0), None);
        s.view.places[1].kills = 12;
        assert!(pair_ups(&s, 0).is_empty(), "not known");
        let mut s = top();
        s.view.places[0].feet = Vec3::new(2.0, 0.0, 38.0);
        s.view.places[2].feet = Vec3::new(4.0, 0.0, 38.0);
        assert!(pair_ups(&s, 0).is_empty(), "far");
    }

    #[test]
    fn it_joins_the_nearer_teammate_and_waits_when_there_is_none() {
        let mut s = top();
        let steps = s.run(0, PlayKind::PairUp, TargetRef::Place(1), 20, 300, 3.5);
        let mate = Vec3::new(32.0, 0.0, 33.0);
        assert!(s.view.places[0].feet.distance_xz(mate) <= 8.0, "{steps:?}");
        assert!(
            matches!(steps.last(), Some(Step::Continue(i)) if i.go == Go::Stay && i.look == Look::At(Vec3::new(50.0, 0.0, 32.0))),
            "{steps:?}"
        );
        // Beside him a step nearer the top player than he is: kept.
        let mut s = top();
        let mut rng = Rng::new(1);
        let mut state = start(TargetRef::Place(1), &s.cx(0), &mut rng);
        assert_eq!(state, State::Pair { mate: 2 });
        s.view.places[0].feet = Vec3::new(34.0, 0.0, 34.0);
        assert!(super::mate(&s.cx(0), Vec3::new(50.0, 0.0, 32.0)).is_none());
        let st = step(&mut state, TargetRef::Place(1), &s.cx(0), &mut rng);
        assert!(matches!(st, Step::Continue(i) if i.go == Go::Stay), "{st:?}");
        // He dies: done.
        s.view.places[2].alive = false;
        let st = step(&mut state, TargetRef::Place(1), &s.cx(0), &mut rng);
        assert_eq!(st, Step::Done("its teammate is down or gone"));
        // Alone: it waits back, crouched, then is done.
        let mut s = top();
        s.view.places[2].alive = false;
        let steps = s.run(0, PlayKind::PairUp, TargetRef::Place(1), 60, 300, 3.5);
        assert!(
            steps
                .iter()
                .all(|st| !matches!(st, Step::Continue(i) if i.go != Go::Stay)),
            "{steps:?}"
        );
        assert_eq!(steps.last(), Some(&Step::Done("no one came")));
    }
}
