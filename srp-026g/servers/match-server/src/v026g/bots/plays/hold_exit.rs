//! Play 15: Hold the exits (`player_bots_behaviour.md` §3): hold a forward
//! choke on the routes out of the enemy base, as humans do (their best
//! kill spots lie 30-60 m from the enemy base, never in the middle).
//!
//! - One bot per exit (`Tactics::exits` of the enemy side; the slot is the
//!   exit's index).
//! - The spot: 15-30 m from the exit with a sight line to it (the median
//!   kill is at 19 m), higher ground preferred, outside every spawn area
//!   (the owner's rule) and at least `base_min_m` from the enemy's spawns;
//!   one of the best few, drawn, so it is not always the same.
//! - Held crouched, looking at the exit. Team Deathmatch: a long hold
//!   (60-120 s); the battery modes: a brief one on the way (10-20 s), as
//!   holding there brings little there.
//! - Move on after 1-2 kills (its own kills in the view rising) or the
//!   drawn time; never straight back: the play then cools down, and a new
//!   spot is at least `moved_min_m` from where the bot stands.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input, Score};
use crate::v026g::bots::tactics::{SpotQuery, Tactics};

pub const NUMBERS: &[(&str, f32)] = &[
    // The spot: this far from the exit, metres.
    ("spot_min_m", 15.0),
    ("spot_max_m", 30.0),
    // Higher ground: weight per metre above the exit.
    ("prefer_high", 0.5),
    // Spots on the storeys over the anchor are looked at up to this many metres
    // over it (0: only the floor the grid finds, at most 2 m over it). GUESS: 8 m,
    // a storey or two (the players' heights span 9-18 m a map); V-S2a.
    ("up_m", 8.0),
    // At least this far from every enemy spawn point, metres.
    ("base_min_m", 30.0),
    // A new spot is at least this far from where the bot stands, metres.
    ("moved_min_m", 10.0),
    // Drawn among this many of the best spots.
    ("pick_of", 3.0),
    ("reach_m", 2.0),
    ("sneak_m", 10.0),
    // Team Deathmatch: held this long, seconds (the long holds there:
    // enemy third K/D x1.6).
    ("hold_tdm_min_s", 60.0),
    ("hold_tdm_max_s", 120.0),
    // The battery modes: held this long, seconds (holding brings little
    // there; a brief hold on the way).
    ("hold_batteries_min_s", 10.0),
    ("hold_batteries_max_s", 20.0),
    // The score times this by mode.
    ("weight_tdm", 1.0),
    ("weight_batteries", 0.6),
    // Timed batteries: holding in the enemy third brought nothing there
    // (0.00 batteries a life, against 0.60 for those who kept moving).
    ("weight_timed", 0.3),
    // Moved on after this many kills (drawn between, whole).
    ("kills_min", 1.0),
    ("kills_max", 2.0),
    // Not at its spot after this, it gives up, seconds.
    ("give_up_s", 60.0),
    ("samples", 64.0),
    // An exit's spots (in sight of it, out of the spawn areas, far from
    // the enemy's spawns) are looked for once and kept this long for the
    // whole team, seconds: the play is offered only where one is fine for
    // the bot, as the commander polls each second. GUESS, not tuned: the
    // sight rays cast once a quarter-minute an exit, and a hazard on a
    // spot is still checked at each poll (`fine_spots`).
    ("spots_keep_s", 15.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![
            considerations::personality(),
            considerations::holding(),
            considerations::not_behind_late(),
            considerations::near(250.0),
            // One bot per exit is the slot; fewer friends there is better.
            Consideration::new(Input::FriendsNearTarget, 0.0, 3.0, Curve::Linear).inverted(),
            considerations::health_above(0.3),
            considerations::cooldown(25.0),
            considerations::runtime(150.0),
        ],
        numbers: Default::default(),
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    let Some(enemy) = cx.bot.side.and_then(|side| cx.enemy_side(side)) else {
        return;
    };
    // A carrier brings the battery home first.
    if cx.me.carrying.is_some() {
        return;
    }
    let running = match cx.bot.current {
        Some((PlayKind::HoldExit, TargetRef::Exit(i), _)) => Some(i),
        _ => None,
    };
    for (index, &exit) in cx.tactics.exits(enemy).iter().enumerate() {
        // Offered only where a spot is fine for it now: taking it to find
        // none ended it within the second (the audit's churn), and the
        // set-aside that followed let it be taken again 8 s later.
        if running != Some(index) && super::super::cpu_trace::span("he fine_spots", || fine_spots(cx, exit)).is_empty() {
            continue;
        }
        out.extend(
            cx.candidate(
                PlayKind::HoldExit,
                TargetRef::Exit(index),
                Some(index as u32),
            )
            .and_then(|c| weighed(cx, PlayKind::HoldExit, c)),
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// No spot was found.
    NoSpot,
    Going {
        spot: Vec3,
    },
    Hold {
        until: u32,
        /// The bot's kills when the hold began, and how many more end it.
        kills_at: u32,
        kills: u32,
    },
}

/// The candidate's score times the mode's weight (`weight_tdm`,
/// `weight_batteries`): the players' research favours the play in one mode.
fn weighed(cx: &Ctx, kind: PlayKind, mut c: Candidate) -> Option<Candidate> {
    let weight = cx.num(kind, cx.view.mode.weight_key());
    c.score = Score::new(c.score.get() * weight);
    (!c.score.is_zero()).then_some(c)
}

/// The spots to hold `exit` from that stay so for a while, best first: in
/// sight of it, outside every spawn area and `base_min_m` from the enemy's
/// spawns; found once for the side and kept `spots_keep_s`.
fn spots_by(cx: &Ctx, exit: Vec3) -> std::sync::Arc<Vec<Vec3>> {
    let num = |name| cx.num(PlayKind::HoldExit, name);
    let key = Tactics::spot_key(PlayKind::HoldExit as u32, cx.bot.side, exit);
    let keep = (num("spots_keep_s") * 1000.0) as u32;
    cx.tactics.spots_kept(key, cx.now, keep, || {
        let enemy = cx.bot.side.and_then(|side| cx.enemy_side(side));
        let spawns: Vec<Vec3> = cx
            .tactics
            .world
            .map
            .spawns
            .iter()
            .filter(|s| Some(s.team) == enemy)
            .map(|s| s.position)
            .collect();
        let query = SpotQuery {
            sight_to: Some(exit),
            prefer_high: num("prefer_high"),
            up_m: num("up_m"),
            side: cx.bot.side,
            samples: num("samples") as usize,
            ..SpotQuery::ring(exit, num("spot_min_m"), num("spot_max_m"))
        };
        let mut rng = Rng::new(key ^ u64::from(cx.now));
        cx.tactics
            .spots(&query, cx.now, query.samples, &mut rng)
            .into_iter()
            .map(|s| s.at)
            .filter(|at| {
                !cx.tactics.in_spawn_area(*at, None)
                    && spawns
                        .iter()
                        .all(|s| s.distance_xz(*at) >= num("base_min_m"))
            })
            .collect()
    })
}

/// Of [`spots_by`], those this bot may take now, best first: fine goals it
/// has a way to ([`Ctx::reachable`], no search lately proved none),
/// `moved_min_m` from where it stands.
fn fine_spots(cx: &Ctx, exit: Vec3) -> Vec<Vec3> {
    let moved = cx.num(PlayKind::HoldExit, "moved_min_m");
    spots_by(cx, exit)
        .iter()
        .copied()
        .filter(|at| {
            cx.tactics.fine_goal(*at, cx.now, cx.bot.side)
                && at.distance_xz(cx.me.feet) >= moved
                && !cx.spot_taken(*at, super::goals::APART_M)
                && cx.can_go(*at)
                && !cx.exposed_climb(*at, exit)
        })
        .collect()
}

/// A spot to hold `exit` from: one of the best `pick_of` fine ones.
fn pick(cx: &Ctx, exit: Vec3, rng: &mut Rng) -> Option<Vec3> {
    let fine = goals::perch_first(cx, exit, rng, fine_spots(cx, exit));
    let n = fine.len().min(cx.num(PlayKind::HoldExit, "pick_of").max(1.0) as usize);
    (n > 0).then(|| fine[rng.below(n)])
}

pub fn start(target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    match cx.point_of(target).and_then(|exit| pick(cx, exit, rng)) {
        Some(spot) => State::Going { spot },
        None => State::NoSpot,
    }
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::HoldExit, name);
    let Some(exit) = cx.point_of(target) else {
        return Step::Failed("no such exit");
    };
    match state {
        State::NoSpot => Step::Failed("no spot by the exit"),
        State::Going { spot } => {
            // Its spot proved no way to: another of the fine ones (none:
            // kept, and the think leaves the play, `goal_shut`).
            goals::repick_if_shut(cx, spot, || pick(cx, exit, rng));
            let spot = *spot;
            if let Some((_, _, started)) = cx.bot.current
                && cx.now > started + (num("give_up_s") * 1000.0) as u32
            {
                return Step::Failed("could not reach its spot");
            }
            let d = cx.me.feet.distance_xz(spot);
            if d <= num("reach_m") {
                let (lo, hi) = if cx.view.mode.is_deathmatch() {
                    (num("hold_tdm_min_s"), num("hold_tdm_max_s"))
                } else {
                    (num("hold_batteries_min_s"), num("hold_batteries_max_s"))
                };
                let kills = rng
                    .range(num("kills_min"), num("kills_max") + 1.0)
                    .floor()
                    .clamp(num("kills_min"), num("kills_max"))
                    .max(1.0) as u32;
                *state = State::Hold {
                    until: cx.now + (rng.range(lo, hi) * 1000.0) as u32,
                    kills_at: cx.me.kills,
                    kills,
                };
                return Step::Continue(Intent::stay().crouched().looking(Look::At(exit)));
            }
            let pace = if d < num("sneak_m") {
                Pace::Sneak
            } else {
                cx.pace_toward(spot)
            };
            Step::Continue(Intent::to(spot, pace).within(num("reach_m") * 0.7))
        }
        State::Hold {
            until,
            kills_at,
            kills,
        } => {
            if cx.me.kills >= *kills_at + *kills {
                return Step::Done("moved on after its kills");
            }
            if cx.now >= *until {
                return Step::Done("held long enough");
            }
            Step::Continue(Intent::stay().crouched().looking(Look::At(exit)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::view::Mode;

    fn holds(s: &Scenario) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        out
    }

    #[test]
    fn one_slot_per_exit_and_none_on_cooldown_or_carrying() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 35.0));
        let out = holds(&s);
        assert_eq!(out.len(), s.tactics.exits(1).len());
        for (i, c) in out.iter().enumerate() {
            assert_eq!(c.target, TargetRef::Exit(i));
            assert_eq!(c.rank, Rank::Fight);
            assert_eq!(c.slot.map(|k| k.key), Some(i as u32));
        }
        s.last_ran.push((PlayKind::HoldExit, s.view.clock - 10_000));
        assert!(holds(&s).is_empty(), "cooling down");
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 35.0)).carrying = Some(0);
        assert!(holds(&s).is_empty(), "carrying");
    }

    /// An exit with no spot fine for the bot is not offered (taken, it
    /// ended within the second); the one it already holds stays offered.
    #[test]
    fn an_exit_with_no_spot_is_not_offered_unless_held() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 35.0));
        // Every spot too near the enemy's spawns.
        s.tuning
            .plays
            .entry(PlayKind::HoldExit)
            .or_insert_with(|| crate::v026g::bots::plays::default_tuning(PlayKind::HoldExit))
            .numbers
            .insert("base_min_m".into(), 10_000.0);
        assert!(holds(&s).is_empty());
        s.current = Some((PlayKind::HoldExit, TargetRef::Exit(0), s.view.clock));
        let out = holds(&s);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, TargetRef::Exit(0));
    }

    /// Steps the play by hand from its start; returns the steps and the
    /// state, the bot moved to each goal at once.
    fn walk(s: &mut Scenario, steps: usize, ms: u32) -> (Vec<Step>, State) {
        let mut rng = Rng::new(7);
        s.current = Some((PlayKind::HoldExit, TargetRef::Exit(0), s.view.clock));
        let mut state = start(TargetRef::Exit(0), &s.cx(0), &mut rng);
        let mut out = Vec::new();
        for _ in 0..steps {
            let st = step(&mut state, TargetRef::Exit(0), &s.cx(0), &mut rng);
            if let Step::Continue(Intent {
                go: Go::To { at, .. },
                ..
            }) = &st
            {
                s.view.places[0].feet = *at;
            }
            let end = !matches!(st, Step::Continue(_));
            out.push(st);
            s.view.clock += ms;
            if end {
                break;
            }
        }
        (out, state)
    }

    #[test]
    fn it_holds_crouched_by_the_exit_out_of_spawn_and_moves_on_after_a_kill() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 35.0));
        let exit = s.tactics.exits(1)[0];
        let (steps, state) = walk(&mut s, 10, 500);
        assert!(matches!(state, State::Hold { .. }), "{steps:?}");
        let at = s.view.places[0].feet;
        let d = at.distance_xz(exit);
        assert!((13.0..=32.0).contains(&d), "{at:?} {d}");
        assert!(!s.tactics.in_spawn_area(at, None));
        for spawn in s.tactics.world.map.spawns.iter().filter(|p| p.team == 1) {
            assert!(spawn.position.distance_xz(at) >= 30.0, "{at:?}");
        }
        let held = steps.last().unwrap();
        assert!(
            matches!(held, Step::Continue(i) if i.crouch && i.go == Go::Stay && i.look == Look::At(exit)),
            "{held:?}"
        );
        // A long hold in Team Deathmatch: still there 30 s on...
        let mut rng = Rng::new(1);
        let mut state = state;
        s.view.clock += 30_000;
        assert!(matches!(
            step(&mut state, TargetRef::Exit(0), &s.cx(0), &mut rng),
            Step::Continue(_)
        ));
        // ...until two kills at most.
        s.view.places[0].kills += 2;
        assert_eq!(
            step(&mut state, TargetRef::Exit(0), &s.cx(0), &mut rng),
            Step::Done("moved on after its kills")
        );
    }

    #[test]
    fn team_deathmatch_scores_a_hold_higher_than_the_battery_modes() {
        let score = |mode| {
            let mut s = Scenario::field(mode);
            s.bot(0, 0, Vec3::new(20.0, 0.0, 35.0));
            holds(&s)[0].score.get()
        };
        let (tdm, batteries) = (score(Mode::TeamDeathmatch), score(Mode::Batteries));
        assert!(batteries < tdm * 0.75, "{tdm} {batteries}");
        assert!(score(Mode::TimedBatteries) < tdm);
    }

    #[test]
    fn a_battery_mode_hold_is_brief() {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 35.0));
        let (steps, state) = walk(&mut s, 10, 500);
        let State::Hold { until, .. } = state else {
            panic!("{steps:?}")
        };
        let left = until - s.view.clock;
        assert!((9_000..=20_000).contains(&left), "{left}");
        // Never straight back: a new spot is away from where it stands.
        let mut rng = Rng::new(9);
        let exit = s.tactics.exits(1)[0];
        let here = s.view.places[0].feet;
        for _ in 0..5 {
            if let Some(spot) = pick(&s.cx(0), exit, &mut rng) {
                assert!(spot.distance_xz(here) >= 10.0);
            }
        }
    }

    /// A spot its bot proved no way to is left for another fine one.
    #[test]
    fn a_spot_proved_no_way_to_is_left_for_another() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 35.0));
        let mut rng = Rng::new(3);
        let exit = TargetRef::Exit(0);
        let mut state = start(exit, &s.cx(0), &mut rng);
        let State::Going { spot: first } = state else {
            panic!("{state:?}")
        };
        s.no_way_to = vec![(first, s.view.clock + 5_000)];
        step(&mut state, exit, &s.cx(0), &mut rng);
        let State::Going { spot } = state else {
            panic!("{state:?}")
        };
        assert!(!s.cx(0).no_way_lately(spot), "{first:?} {spot:?}");
    }
}
