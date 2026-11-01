//! Play 28, answer a sniper (the owner, 2026-10-26: "teammates with longer weapons answer a sniper", TASKS C3 / X3 part 2).
//! An enemy sniper the team sensed lately outranges the short weapons (the outranged rule, `reflex.rs`); a free teammate whose
//! weapon reaches as far as he does (a rifle, a sniper's own) takes a spot that sees his last sensed place within its own
//! reach, and fires at him from there. Counter-sniper doctrine: take cover, then counter-fire from a covered position at range.
//!
//! - Who he is: the enemy whose weapon 1 the match knows is a sniper's ([`BotFacts::weapons`], as a live match knows it: the
//!   rifle in his hands is seen), and where he is: [`Track::own`], what the bot's or its team's own senses gave (seen, heard,
//!   told by a callout), never a cheat's place. Sensed within `recent_s`; after that, where the team last sensed him while
//!   its sniper memory holds his spot (`threat.sniper_memory`, #25b) and that sense is within `remembered_s` (A210: the
//!   bot's own sense went stale on the way to the spot, and a third of the runs ended so).
//! - On its way to the spot the play is a hold walking to its spot (`Running::travelling`, `commit.hold_to_arrival`):
//!   kept against the plays of its band (A210: a quarter of the runs were taken by one on the way).
//! - Who goes: not a carrier or a thief (`Ctx::me` an objective actor: carriers never stop), not dry, healthy, whose own reach
//!   (the top of its weapon's band and the slack) is at least `reach_min_m`. Two bots a sniper at most (two slots); the gang-up
//!   cap of the commander keeps the rest of the team off him.
//! - The camper decision map (`camper-decision-map.md` §3): a team job on him (K2: a flank and its pin) goes first, so only
//!   that job's pinner may answer him while it runs; an area the team conceded refuses the play, and a try like the last
//!   failed one there is refused (`Ctx::concede_weight`, `threat.concede`).
//! - The spot: `spot_min_m` to the bot's own reach from his place, a sight line to it, higher ground preferred, out of every held
//!   threat line (his lanes: `ThreatTuning` K1), a fine goal out of the spawn areas; reached by a way point hidden from him
//!   (`via`), crouched for the last metres, so the way there is not across his line.
//! - At the spot: crouched, looking at him for a few seconds; the reflex shoots once he is seen and noticed. Then it is done and
//!   the commander picks again (the cooldown keeps the same bot off him for a while).

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input, Score};
use crate::v026g::bots::reflex::WeaponKind;
use crate::v026g::bots::tactics::SpotQuery;
use crate::v026g::bots::threat::{Eyes, Role};
use crate::v026g::bots::view;

pub const NUMBERS: &[(&str, f32)] = &[
    // Sensed by the team's own senses this recently, seconds.
    ("recent_s", 10.0),
    // Else the team's remembered spot of him, its last sense this recent, seconds: the play's own runtime (a walk of 20-40 m
    // and a peek); the memory keeps a live sniper on his area 6 times in 10 at 30 s, 3 at a minute (A199 §3.6).
    ("remembered_s", 45.0),
    // The bot's own reach (band top and slack) at least this, metres: a rifle's, a sniper's.
    ("reach_min_m", 45.0),
    // Only a sniper this near the bot is answered, metres.
    ("max_m", 120.0),
    // The spot: at least this far from him, metres (the top is the bot's own reach).
    ("spot_min_m", 22.0),
    // The way point: this far from the spot, hidden from him, metres.
    ("via_min_m", 6.0),
    ("via_max_m", 15.0),
    ("sneak_m", 12.0),
    ("reach_m", 2.0),
    // At the spot: watching him this long, seconds.
    ("peek_min_s", 4.0),
    ("peek_max_s", 8.0),
    ("give_up_s", 40.0),
    // The score times this by mode (as Flank's).
    ("weight_tdm", 1.0),
    ("weight_batteries", 0.8),
    ("weight_timed", 0.8),
    ("samples", 64.0),
    // Regions with a clear sight to him tested before it gives up, nearest the bot first. 24 gave up in half the starts: every
    // one of them had over 24 free regions, and a dense search found a free floor that sees him in 79 of 83 (PROVEN, bench
    // `dense-38c316ae`); a start is a few a match, so the rays are few.
    ("tries", 400.0),
];

// [measure] How the play ended and what it reached, this thread's match (the bench's `answer` counts; nothing decides by
// them): `done` / `failed` by the reason the step gave, why no spot at the start, and once at the spot.
#[cfg(test)]
thread_local! {
    static ENDS: std::cell::RefCell<std::collections::BTreeMap<&'static str, u32>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

#[cfg(test)]
fn noted(what: &'static str) {
    ENDS.with(|m| *m.borrow_mut().entry(what).or_default() += 1);
}

#[cfg(not(test))]
fn noted(_: &'static str) {}

/// The sampling note of a start that found no spot (root cause #12, the cap on the tests was the cause): how many region
/// middles were free. Test-only.
#[cfg(test)]
fn sampling_note(ring: usize, free_n: usize, tested: usize) {
    noted(match (ring, free_n, tested) {
        (0, _, _) => "sampling: no region in the ring",
        (_, 0, _) => "sampling: regions in the ring, none free",
        _ => "sampling: free regions, all tested",
    });
}

/// A running answer was replaced by the commander: `offered` is whether the play was still a candidate of the bot (then
/// another was better), else its conditions had lapsed. Test-only (the end counts a play that is never stepped to its end).
#[cfg(test)]
pub fn replaced(offered: bool, by: impl std::fmt::Debug) {
    let what: &'static str = Box::leak(
        format!("replaced by {by:?}, {}", if offered { "still offered, beaten" } else { "no longer offered" }).into_boxed_str(),
    );
    noted(what);
}

/// The counts so far, and a new match's reset.
#[cfg(test)]
pub fn ends() -> Vec<(&'static str, u32)> {
    ENDS.with(|m| m.borrow().iter().map(|(k, v)| (*k, *v)).collect())
}

#[cfg(test)]
pub fn reset_ends() {
    ENDS.with(|m| m.borrow_mut().clear());
}

fn done(why: &'static str) -> Step {
    noted(why);
    Step::Done(why)
}

fn failed(why: &'static str) -> Step {
    noted(why);
    Step::Failed(why)
}

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Fight,
        considerations: vec![
            considerations::personality(),
            considerations::holding(),
            considerations::near(150.0),
            considerations::health_above(0.5),
            // Fewer friends already by him: one pair answers, not the team.
            Consideration::new(Input::FriendsNearTarget, 0.0, 3.0, Curve::Linear).inverted(),
            considerations::cooldown(20.0),
            considerations::runtime(45.0),
        ],
        numbers: Default::default(),
    }
}

/// The bot's own reach, metres: the top of its weapon's band and the slack (`CombatTuning::band`); none: no weapon known.
fn own_reach(cx: &Ctx) -> Option<f32> {
    let c = &cx.tuning.aim.combat;
    cx.bot.weapon.map(|kind| c.band(kind).1 + c.band_slack_m)
}

/// Where the team's own senses last put the enemy at `who`, when he is a sniper they sensed within `recent_s`; else where the
/// team last sensed him, while its sniper memory holds his spot and that sense is within `remembered_s`.
fn sniper_at(cx: &Ctx, who: Place) -> Option<Vec3> {
    if cx.bot.weapons.get(who).copied().flatten() != Some(WeaponKind::Sniper) || !cx.view.place(who).is_some_and(|p| p.alive) {
        return None;
    }
    let within = |time: u32, name: &str| cx.now.saturating_sub(time) <= (cx.num(PlayKind::Answer, name) * 1000.0) as u32;
    let own = cx.memory.tracks().iter().find(|t| t.who == who).and_then(|t| t.own);
    own.filter(|&(_, time)| within(time, "recent_s")).map(|(at, _)| at).or_else(|| {
        let spot = cx.team.threat.sniper(who)?;
        spot.sensed.is_some_and(|time| within(time, "remembered_s")).then_some(spot.at)
    })
}

/// The inputs reckoned from where the play takes him to be (his track may be gone): the way there and the friends by him.
fn inputs_at(cx: &Ctx, at: Vec3) -> [(Input, f32); 2] {
    [(Input::PathDist, cx.path_dist(at)), (Input::FriendsNearTarget, cx.friends_near(at, 25.0) as f32)]
}

/// A team job (K2) runs on `who` and the bot is not its pinner: the job goes first (`camper-decision-map.md` §3).
fn job_on(cx: &Ctx, who: Place) -> bool {
    cx.team.threat.jobs.iter().any(|j| j.shooter == who && j.role_of(cx.me.place) != Some(Role::Pinner))
}

/// Why a running answer on `target` is no longer a candidate, counted once the commander drops it (A210: a quarter of the
/// runs ended so, with no reason told). The first of [`candidates`]' conditions that fails, else the consideration at 0.
/// Test-only.
#[cfg(test)]
pub fn left_out(cx: &Ctx, target: TargetRef) {
    let why = why_left_out(cx, target);
    noted(Box::leak(format!("left out: {why}").into_boxed_str()));
}

#[cfg(test)]
fn why_left_out(cx: &Ctx, target: TargetRef) -> String {
    let TargetRef::Place(who) = target else {
        return "not a place".to_string();
    };
    if cx.view.objective().is_actor(cx.me) || cx.me.carrying.is_some() {
        return "an objective actor".to_string();
    }
    if cx.bot.weapons.get(who).copied().flatten() != Some(WeaponKind::Sniper) {
        return "his weapon is not known a sniper's".to_string();
    }
    if !cx.view.place(who).is_some_and(|p| p.alive) {
        return "he is dead".to_string();
    }
    let Some(at) = sniper_at(cx, who) else {
        return match cx.team.threat.sniper(who).and_then(|s| s.sensed) {
            Some(_) => "the bot's sense old, the team's past remembered_s".to_string(),
            None => "the bot's sense old, no remembered spot of him".to_string(),
        };
    };
    if job_on(cx, who) {
        return "a team job on him".to_string();
    }
    if cx.bot.set_aside.iter().any(|&(k, t, until)| k == PlayKind::Answer && t == target && cx.now < until) {
        return "set aside".to_string();
    }
    if cx.bot.dry {
        return "dry".to_string();
    }
    let given = inputs_at(cx, at);
    let (score, why) = crate::v026g::bots::curve::score(&cx.play(PlayKind::Answer).considerations, |input| {
        given.iter().find(|(i, _)| *i == input).map_or_else(|| cx.input(input, PlayKind::Answer, target), |&(_, v)| v)
    });
    if score.is_zero() {
        return why.parts.iter().find(|(_, _, s)| *s <= 0.0).map_or("zero score".to_string(), |(i, _, _)| format!("zero {i:?}"));
    }
    "still scored: the concede rules or the mode weight".to_string()
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !cx.tuning.threat.answer_sniper || cx.view.objective().is_actor(cx.me) || cx.me.carrying.is_some() {
        return;
    }
    let Some(reach) = own_reach(cx) else {
        return;
    };
    if reach < cx.num(PlayKind::Answer, "reach_min_m") {
        return;
    }
    let weight = cx.num(PlayKind::Answer, cx.view.objective().weight_key());
    // The enemies the bot tracks, and the snipers the team remembers whose tracks it has dropped.
    let mut whos: Vec<Place> = cx.memory.tracks().iter().map(|t| t.who).collect();
    whos.extend(cx.team.threat.snipers.iter().map(|s| s.shooter).filter(|&p| cx.memory.tracks().iter().all(|t| t.who != p)));
    for who in whos {
        let target = TargetRef::Place(who);
        let running = matches!(cx.bot.current, Some((PlayKind::Answer, t, _)) if t == target);
        let Some(at) = sniper_at(cx, who) else {
            continue;
        };
        if !running && cx.me.feet.distance_xz(at) > cx.num(PlayKind::Answer, "max_m") {
            continue;
        }
        if job_on(cx, who) {
            continue;
        }
        let given = inputs_at(cx, at);
        for k in 0..2 {
            out.extend(
                cx.candidate_given(PlayKind::Answer, target, Some(who as u32 * 2 + k), &given)
                    .map(|mut c| {
                        c.score = Score::new(c.score.get() * weight);
                        c
                    })
                    .filter(|c| !c.score.is_zero()),
            );
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// To a way point hidden from him, then to the spot.
    Via { via: Vec3, spot: Vec3 },
    Approach { spot: Vec3 },
    /// At the spot, watching him.
    Peek { until: u32 },
    /// No spot that sees him within the reach: the play ends at once, and is set aside.
    NoSpot,
}

pub fn start(target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    let num = |name| cx.num(PlayKind::Answer, name);
    let (TargetRef::Place(who), Some(reach)) = (target, own_reach(cx)) else {
        return State::NoSpot;
    };
    let Some(him) = sniper_at(cx, who) else {
        return State::NoSpot;
    };
    // The spots are the regions' middles (every floor of every height) 22 m to the bot's reach from him, nearest the bot
    // first, the first `tries` that pass the filters tested for a sight line to him from a crouched eye. A ring of probes at
    // one height found no floor in half the starts (`floor_off_height`, 56 % of the 48 % with no spot; three ring fixes
    // did not change it): a sniper sits high, and the ground that sees him is not at any one height. The cap on the tests
    // (`tries`) was the cause that was left.
    let (lo, hi) = (num("spot_min_m"), reach);
    let mut ring: Vec<Vec3> = Eyes::region_points(cx.tactics)
        .into_iter()
        .map(|(_, at)| at)
        .filter(|at| (lo..=hi).contains(&at.distance_xz(him)))
        .collect();
    ring.sort_by(|a, b| a.distance_xz(cx.me.feet).total_cmp(&b.distance_xz(cx.me.feet)));
    let free = |at: Vec3| {
        cx.team.threat.line_at(at).is_none()
            && cx.tactics.fine_goal(at, cx.now, cx.bot.side)
            && !cx.tactics.in_spawn_area(at, None)
    };
    let (crouched, chest) = (view::eye_height(true, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
    let mut tested = 0;
    let mut spot = None;
    for &at in ring.iter().filter(|&&at| free(at)) {
        if tested >= num("tries") as usize {
            break;
        }
        tested += 1;
        if cx.tactics.sees(at + Vec3::new(0.0, crouched, 0.0), him + chest) {
            spot = Some(at);
            break;
        }
    }
    let Some(spot) = spot else {
        #[cfg(test)]
        sampling_note(ring.len(), ring.iter().filter(|&&at| free(at)).count(), tested);
        noted("no spot at the start");
        noted(if ring.is_empty() {
            "no region in the ring"
        } else if tested == 0 {
            "all regions in a held line, not a fine goal or in the spawn"
        } else {
            "none of the nearest sees him"
        });
        return State::NoSpot;
    };
    // A way point near the spot, out of his sight, so the way there is not across his line.
    let via_query = SpotQuery {
        unseen_by: Some(him),
        side: cx.bot.side,
        samples: num("samples") as usize,
        ..SpotQuery::ring(spot, num("via_min_m"), num("via_max_m"))
    };
    let via = cx
        .tactics
        .spots(&via_query, cx.now, 6, rng)
        .into_iter()
        .map(|s| s.at)
        .filter(|&at| cx.tactics.fine_goal(at, cx.now, cx.bot.side))
        .min_by(|a, b| a.distance_xz(cx.me.feet).total_cmp(&b.distance_xz(cx.me.feet)));
    match via {
        Some(via) if via.distance_xz(cx.me.feet) < spot.distance_xz(cx.me.feet) => State::Via { via, spot },
        _ => State::Approach { spot },
    }
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Answer, name);
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (num("give_up_s") * 1000.0) as u32
    {
        return failed("answered too long");
    }
    let TargetRef::Place(who) = target else {
        return failed("no sniper");
    };
    if matches!(state, State::NoSpot) {
        return failed("no spot that sees him within the reach");
    }
    // He is dead, or the team's senses lost him: nothing to answer.
    let Some(him) = sniper_at(cx, who) else {
        return done("the sniper is gone from the team's senses");
    };
    // The bot took a battery meanwhile: carriers never stop.
    if cx.view.objective().is_actor(cx.me) || cx.me.carrying.is_some() {
        return done("it carries now");
    }
    let reach = num("reach_m");
    match state {
        State::NoSpot => failed("no spot"),
        State::Via { via, spot } => {
            let (via, spot) = (*via, *spot);
            if cx.me.feet.distance_xz(via) < reach * 1.5 {
                *state = State::Approach { spot };
            }
            Step::Continue(Intent::to(via, Pace::Walk).within(reach))
        }
        State::Approach { spot } => {
            let spot = *spot;
            let d = cx.me.feet.distance_xz(spot);
            if d <= reach {
                let s = rng.range(num("peek_min_s"), num("peek_max_s"));
                noted("at the spot");
                *state = State::Peek { until: cx.now + (s * 1000.0) as u32 };
                return Step::Continue(Intent::stay().crouched().looking(Look::At(him)));
            }
            let sneaking = d < num("sneak_m");
            let mut intent = Intent::to(spot, if sneaking { Pace::Sneak } else { Pace::Walk }).within(reach * 0.7);
            if sneaking {
                intent = intent.looking(Look::At(him));
            }
            Step::Continue(intent)
        }
        State::Peek { until } => {
            if cx.now >= *until {
                return done("watched him from the spot");
            }
            Step::Continue(Intent::stay().crouched().looking(Look::At(him)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::{Percept, Source};
    use crate::v026g::bots::view::Mode;

    /// Bot 0 (a rifle) at (25, 0, 32); the enemy 1 at (85, 0, 30) with a sniper's weapon, seen now.
    fn sniper() -> Scenario {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(25.0, 0.0, 32.0));
        s.enemy(1, 1, Vec3::new(85.0, 0.0, 30.0), None);
        s.weapon = Some(WeaponKind::Rifle);
        s.weapons = vec![None, Some(WeaponKind::Sniper)];
        s.tuning.threat.answer_sniper = true;
        let now = s.view.clock;
        let at = s.view.places[1].feet;
        s.memory.update(
            &[Percept { who: 1, at, error_m: 0.0, time: now, source: Source::Seen, busy: false }],
            now,
            |_| 200,
            3_000,
            20_000,
        );
        s
    }

    fn answers(s: &Scenario) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(0), &mut out);
        out
    }

    #[test]
    fn a_rifle_answers_a_sensed_sniper_with_two_slots_and_a_short_weapon_does_not() {
        let s = sniper();
        let c = answers(&s);
        assert_eq!(c.len(), 2, "{c:#?}");
        assert!(c.iter().all(|c| c.target == TargetRef::Place(1) && c.rank == Rank::Fight));
        let mut smg = sniper();
        smg.weapon = Some(WeaponKind::Smg);
        assert!(answers(&smg).is_empty(), "a short weapon does not reach him");
    }

    #[test]
    fn it_is_off_without_its_switch_and_for_a_carrier_and_a_rifleman_he_is_not() {
        let mut off = sniper();
        off.tuning.threat.answer_sniper = false;
        assert!(answers(&off).is_empty());
        let mut not_a_sniper = sniper();
        not_a_sniper.weapons = vec![None, Some(WeaponKind::Rifle)];
        assert!(answers(&not_a_sniper).is_empty(), "he is not a sniper");
        let mut unknown = sniper();
        unknown.weapons = Vec::new();
        assert!(answers(&unknown).is_empty(), "his weapon is not known");
    }

    #[test]
    fn a_sniper_not_sensed_lately_is_not_answered() {
        let mut s = sniper();
        s.view.clock += 20_000;
        assert!(answers(&s).is_empty());
    }

    /// A210: the bot's own sense of him is old, but the team's sniper memory holds his spot: still answered; once the team's
    /// last sense of him is older than `remembered_s`, not.
    #[test]
    fn a_sniper_the_team_remembers_is_answered_after_the_bots_own_sense_is_old() {
        use crate::v026g::bots::threat::{HeldLane, Picture};
        let mut s = sniper();
        let sensed = s.view.clock;
        s.view.clock += 20_000;
        let held = HeldLane { at: Vec3::new(85.0, 0.0, 30.0), shooter: 1, reach_m: 98.0, sensed: Some(sensed), area_m: 10.0 };
        s.team.threat = std::sync::Arc::new(Picture { snipers: vec![held], ..Picture::default() });
        assert_eq!(answers(&s).len(), 2, "remembered by the team");
        s.view.clock = sensed + 46_000;
        assert!(answers(&s).is_empty(), "the team's last sense of him is past remembered_s");
    }

    /// The camper decision map §3: a team job on him goes first; only its pinner may still answer him, and a job on another
    /// shooter changes nothing.
    #[test]
    fn a_team_job_on_him_goes_first_but_its_pinner_may_answer() {
        use crate::v026g::bots::threat::{Holder, Job, Picture};
        let mut s = sniper();
        let job = |shooter, pinner| Job::of(4, shooter, Some(Holder { place: 3, since: 0 }), Some(Holder { place: pinner, since: 0 }));
        s.team.threat = std::sync::Arc::new(Picture { jobs: vec![job(1, 2)], ..Picture::default() });
        assert!(answers(&s).is_empty(), "a flank and its pin run on him");
        s.team.threat = std::sync::Arc::new(Picture { jobs: vec![job(1, 0)], ..Picture::default() });
        assert_eq!(answers(&s).len(), 2, "the bot is the job's pinner");
        s.team.threat = std::sync::Arc::new(Picture { jobs: vec![job(5, 2)], ..Picture::default() });
        assert_eq!(answers(&s).len(), 2, "the job is on another shooter");
    }

    /// A210: the walk to the spot is kept against the plays of its band (`commit.hold_to_arrival`); the peek is not a walk.
    #[test]
    fn its_walk_to_the_spot_is_a_hold_walking_to_its_spot() {
        let spot = Vec3::new(1.0, 0.0, 1.0);
        assert!(Running::Answer(State::Via { via: spot, spot }).travelling());
        assert!(Running::Answer(State::Approach { spot }).travelling());
        assert!(!Running::Answer(State::Peek { until: 0 }).travelling());
        assert!(!Running::Answer(State::NoSpot).travelling());
    }

    #[test]
    fn it_goes_to_a_spot_that_sees_him_within_its_reach_and_watches_him() {
        let mut s = sniper();
        let steps = s.run(0, PlayKind::Answer, TargetRef::Place(1), 3, 300, 3.5);
        let Step::Continue(Intent { go: Go::To { at, .. }, .. }) = &steps[0] else {
            panic!("{steps:?}");
        };
        let d = at.distance_xz(Vec3::new(85.0, 0.0, 30.0));
        let reach = s.tuning.aim.combat.band(WeaponKind::Rifle).1 + s.tuning.aim.combat.band_slack_m;
        // The first leg may be the way point hidden from him, up to `via_max_m` from the spot.
        assert!((22.0 - 15.0..=reach + 15.0).contains(&d), "the first leg is {d} m from him, reach {reach}");
    }

    /// The bench retunes the play's numbers by a JSON patch of the tuning (`SRP_BENCH_TUNING`).
    #[test]
    fn its_numbers_are_set_by_a_tuning_patch() {
        let patch = serde_json::json!({ "plays": { "answer": { "numbers": { "recent_s": 25.0 } } } });
        let tuning = crate::v026g::bots::tuning::Tuning::patched(patch).unwrap();
        assert_eq!(tuning.number(PlayKind::Answer, "recent_s"), 25.0);
        assert_eq!(tuning.number(PlayKind::Answer, "peek_min_s"), 4.0);
    }
}
