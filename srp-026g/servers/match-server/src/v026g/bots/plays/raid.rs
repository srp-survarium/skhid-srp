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

/// [A296 items 4 and 5] What the switches did (the bench's tally): raids offered early by `mods.behind_open` (before the play's
/// own `after_s`), regroup stops by `mods.raid_regroup`, those a mate came to, those that waited the whole `wait_s`.
/// [fix-team `mods.raid_approach`] ... and the approaches chosen among the human routes, or round the container.
pub mod raid_counts {
    #[cfg(test)]
    use std::sync::atomic::{AtomicU64, Ordering};

    #[cfg(test)]
    static COUNTS: [AtomicU64; 9] = [const { AtomicU64::new(0) }; 9];

    pub enum Count {
        EarlyOffered,
        RegroupStop,
        RegroupMate,
        RegroupTimedOut,
        ApproachRoute,
        ApproachRing,
        /// [fix-team raid re-plan] The approach priced again on the way.
        Repriced,
        /// Another approach taken by the margin.
        Switched,
        /// Another approach taken at once, the lane having turned bad.
        SwitchedLane,
    }

    #[cfg(test)]
    pub fn count(what: Count) {
        COUNTS[what as usize].fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(not(test))]
    pub fn count(_: Count) {}

    #[cfg(test)]
    pub fn counts() -> [u64; 9] {
        [0, 1, 2, 3, 4, 5, 6, 7, 8].map(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !cx.view.objective().has_homes() || cx.view.objective().is_actor(cx.me) {
        return;
    }
    let elapsed = cx.view.total_s - cx.view.time_left_s;
    let Some(theirs) = cx.view.containers.iter().find(|c| c.team != cx.me.team) else {
        return;
    };
    if theirs.batteries == 0 {
        return;
    }
    // [A296 item 4, `mods.behind_open`] A team behind raids from earlier, into a thin guard only.
    let behind = cx.tuning.mods.behind_open.filter(|open| open.behind(cx.sig.score_gap));
    if elapsed < cx.num(PlayKind::Raid, "after_s") {
        let Some(open) = behind else {
            return;
        };
        let guard = cx.enemies_near(theirs.at, cx.num(PlayKind::Raid, "guard_m"));
        if elapsed < open.raid_after_s || guard > open.thin_guard as usize {
            return;
        }
        #[cfg(test)]
        raid_counts::count(raid_counts::Count::EarlyOffered);
    }
    let target = TargetRef::Container(theirs.id);
    out.extend(raid_candidate(cx, target, u32::from(theirs.id)));
    // Behind late: a second bot raids too (holding gains nothing then). [A296 item 5, `mods.raid_regroup.call`] Behind with the
    // regroup's call: the second slot too, a mate for the stop.
    let call = cx.tuning.mods.raid_regroup.is_some_and(|r| r.call)
        && cx.sig.score_gap <= -cx.tuning.mods.behind_open.map_or(1.0, |open| open.by);
    // [fix-team push, second taker] The push's two takers on their container: one a slot.
    let push_pair = cx.push_takers(target) >= 2;
    if (call || push_pair || cx.sig.behind_late() >= cx.num(PlayKind::Raid, "pair_behind")) && theirs.batteries >= 2 {
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
    // [fix-team push `mods.push`] In a push the guard is counted net of the bot's group (`Ctx::push_net`), for the score and the
    // running raid's `guarded` test alike.
    let net = cx.point_of(target).and_then(|at| cx.push_net(target, at));
    let committed = running
        && cx.point_of(target).is_some_and(|at| {
            let near = net.unwrap_or_else(|| cx.enemies_near(at, cx.num(PlayKind::Raid, "guard_m")) as f32);
            near < cx.num(PlayKind::Raid, "guarded")
        });
    if committed {
        cx.candidate_given(
            PlayKind::Raid,
            target,
            Some(slot),
            &[(Input::EnemiesDead, 1.0), (Input::EnemiesNearTarget, 0.0)],
        )
    } else if let Some(net) = net {
        cx.candidate_given(PlayKind::Raid, target, Some(slot), &[(Input::EnemiesNearTarget, net)])
    } else {
        cx.candidate(PlayKind::Raid, target, Some(slot))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Approach,
    /// [fix-team `mods.raid_approach`] On its chosen approach: to `points[next]`, the last its gather point; there it regroups
    /// (`mods.raid_regroup`) or goes in. `priced`: when it was last priced ([fix-team `mods.raid_replan`]).
    Via { points: Vec<Vec3>, next: usize, priced: u32 },
    /// [A296 item 5, `mods.raid_regroup`] Stopped short of their container for a teammate to come, to `until`.
    Regroup { until: u32 },
    /// In after a regroup (or with no mate to wait for): no second stop.
    GoIn,
    Take { tries: u8, last_use: u32 },
}

pub fn start(target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> State {
    match (cx.tuning.mods.raid_approach, cx.point_of(target)) {
        (Some(rule), Some(container)) => match approach(cx, container, &rule) {
            Some(points) => State::Via { points, next: 0, priced: cx.now },
            None => State::Approach,
        },
        _ => State::Approach,
    }
}

/// [fix-team `mods.raid_approach`] The way's risk by what the team knows (an influence map priced along the way, Mark's
/// threat maps; Killzone 3's squads path round the danger): each `sample_m` of the way weighs the known enemies within
/// `enemy_m` (its own senses and callouts, fading over `known_s`), the team's deaths within `death_m` (the losses, fading
/// over their 120 s), and the enemy humans' recent lanes within `lane_m` (`HumanLanes`, their own decay), each at most 1.
fn way_risk(cx: &Ctx, way: &[Vec3], rule: &crate::v026g::bots::tuning::RaidApproach) -> f32 {
    let known_ms = rule.known_s * 1000.0;
    let enemies: Vec<(Vec3, f32)> = cx
        .memory
        .tracks()
        .iter()
        .filter_map(|t| t.own)
        .filter_map(|(at, time)| {
            let age = cx.now.saturating_sub(time) as f32;
            (age < known_ms).then(|| (at, 1.0 - age / known_ms))
        })
        .collect();
    let keep = crate::v026g::bots::know::LOSS_KEEP_MS as f32;
    let deaths: Vec<(Vec3, f32)> = cx
        .team
        .losses
        .iter()
        .filter(|l| l.time <= cx.now)
        .map(|l| (l.at, (1.0 - (cx.now - l.time) as f32 / keep).max(0.0)))
        .collect();
    let lanes: Vec<(Vec3, f32)> =
        cx.team.humans.humans.iter().flat_map(|h| h.cells.iter().map(|c| (c.at, c.count_at(cx.now).min(1.0)))).collect();
    let near = |p: Vec3, sources: &[(Vec3, f32)], reach: f32| -> f32 {
        sources.iter().map(|&(at, w)| w * (1.0 - at.distance_xz(p) / reach).max(0.0)).sum::<f32>().min(1.0)
    };
    let mut risk = 0.0;
    for leg in way.windows(2) {
        let (a, b) = (leg[0], leg[1]);
        let steps = (a.distance_xz(b) / rule.sample_m).ceil().max(1.0) as usize;
        for k in 1..=steps {
            let p = a + (b - a) * (k as f32 / steps as f32);
            risk += (near(p, &enemies, rule.enemy_m) + near(p, &deaths, rule.death_m) + near(p, &lanes, rule.lane_m)) * rule.sample_m;
        }
    }
    risk
}

/// [fix-team `mods.raid_approach`] The approach to the container at `container` that the team's knowledge covers least (the
/// owner's "the less protected flank, or a path the enemies weren't using"), as the points to walk before the container, the
/// last the gather point: among the side's human routes that pass within `route_near_m` of the container (a real lane, from
/// the route's point nearest the bot to its point nearest the container) where there are any, else among `bearings` points
/// round the container at `stage_m` (the regroup's ring) on a floor the bot can go to. Each priced as its length plus
/// `risk_m` times its risk ([`way_risk`]) over the whole way, the last leg to the container included. None: no approach.
fn approach(cx: &Ctx, container: Vec3, rule: &crate::v026g::bots::tuning::RaidApproach) -> Option<Vec<Vec3>> {
    let (best, kind) = cheapest(cx, container, rule)?;
    raid_counts::count(kind);
    Some(best.1)
}

/// [fix-team `mods.raid_approach`] The price of walking `points` (an approach's points, the last its gather point) from the bot
/// to the container: the bot's path distance to the first point (as `Input::PathDist`), the approach's own legs and the last
/// leg to the container, and `risk_m` times the whole way's risk ([`way_risk`]). One price for an approach chosen at the start
/// and for one priced again on the way ([`replan`]).
fn price(cx: &Ctx, container: Vec3, rule: &crate::v026g::bots::tuning::RaidApproach, points: &[Vec3]) -> f32 {
    let way: Vec<Vec3> = std::iter::once(cx.me.feet).chain(points.iter().copied()).chain(std::iter::once(container)).collect();
    let length = cx.path_dist(points[0]) + way[1..].windows(2).map(|w| w[0].distance_xz(w[1])).sum::<f32>();
    length + rule.risk_m * way_risk(cx, &way, rule)
}

/// [fix-team `mods.raid_approach`] The cheapest approach from where the bot is ([`approach`]'s candidates, [`price`]), with its
/// price and whether it is a route's or the ring's.
fn cheapest(cx: &Ctx, container: Vec3, rule: &crate::v026g::bots::tuning::RaidApproach) -> Option<((f32, Vec<Vec3>), raid_counts::Count)> {
    let feet = cx.me.feet;
    let routes: Vec<Vec<Vec3>> = cx
        .bot
        .side
        .into_iter()
        .flat_map(|side| cx.tactics.routes_of(side))
        .filter_map(|route| {
            let d = |i: usize| route.points[i].distance_xz(container);
            let gather = (0..route.points.len()).min_by(|&a, &b| d(a).total_cmp(&d(b))).filter(|&i| d(i) <= rule.route_near_m)?;
            let from = (0..route.points.len())
                .min_by(|&a, &b| route.points[a].distance_xz(feet).total_cmp(&route.points[b].distance_xz(feet)))?;
            // [review of #2195] Never back first: the route's points farther from the container than the bot are left out.
            let ahead = feet.distance_xz(container);
            let points: Vec<Vec3> = (from <= gather)
                .then(|| route.points[from..=gather].iter().copied().skip_while(|p| p.distance_xz(container) > ahead).collect())?;
            (!points.is_empty()).then_some(points)
        })
        .filter(|points| points.last().is_some_and(|&g| cx.can_go_into_base(g)))
        .collect();
    // The ring's points round the container (none where no floor the bot can go to), by bearing; none for a bot already
    // inside the ring (going out to it would be going back).
    let ring = || -> Vec<Vec<Vec3>> {
        if feet.distance_xz(container) <= rule.stage_m {
            return Vec::new();
        }
        let grid = &cx.tactics.world.grid;
        let n = usize::from(rule.bearings);
        let points: Vec<Option<Vec3>> = (0..n)
            .map(|k| {
                let yaw = std::f32::consts::TAU * k as f32 / n as f32;
                let p = container + Vec3::new(yaw.cos() * rule.stage_m, 0.0, yaw.sin() * rule.stage_m);
                let floor = grid.position(grid.locate(p, 3.0)?);
                cx.can_go_into_base(floor).then_some(floor)
            })
            .collect();
        // [review of #2195] The bot comes onto the ring at its point nearest the bot and walks round it to the gather point,
        // the shorter way, never across the container.
        let Some(entry) = (0..n).filter(|&k| points[k].is_some()).min_by(|&a, &b| {
            let d = |k: usize| points[k].map_or(f32::INFINITY, |p| p.distance_xz(feet));
            d(a).total_cmp(&d(b))
        }) else {
            return Vec::new();
        };
        (0..n)
            .filter(|&g| points[g].is_some())
            .map(|g| {
                let forward = (g + n - entry) % n;
                let steps: Vec<usize> =
                    if forward <= n / 2 { (0..=forward).map(|i| (entry + i) % n).collect() } else { (0..=n - forward).map(|i| (entry + n - i) % n).collect() };
                steps.into_iter().filter_map(|k| points[k]).collect()
            })
            .collect()
    };
    let (candidates, kind) = if routes.is_empty() { (ring(), raid_counts::Count::ApproachRing) } else { (routes, raid_counts::Count::ApproachRoute) };
    let best = candidates.into_iter().map(|p| (price(cx, container, rule, &p), p)).min_by(|a, b| a.0.total_cmp(&b.0))?;
    Some((best, kind))
}

/// [fix-team `mods.raid_replan`] Whether the lane of the way `way` turned bad since `since`: an enemy the team's own senses put
/// within `enemy_m` of it, or a team death within `death_m` of it, both after `since` (Killzone 3's influence map counts the
/// enemies and the recent deaths).
fn lane_turned(cx: &Ctx, way: &[Vec3], since: u32, rule: &crate::v026g::bots::tuning::RaidApproach) -> bool {
    let near = |p: Vec3, reach: f32| way.windows(2).any(|w| crate::v026g::bots::team::to_segment(p, w[0], w[1]) <= reach);
    let enemy = cx.memory.tracks().iter().filter_map(|t| t.own).any(|(at, time)| time > since && near(at, rule.enemy_m));
    let death = cx.team.losses.iter().any(|l| l.time > since && l.time <= cx.now && near(l.at, rule.death_m));
    enemy || death
}

/// [fix-team `mods.raid_replan`] The approach on the way: priced again every `every_s` or at once when its lane turned bad
/// ([`lane_turned`]), never within `lock_m` of its gather point; another approach from where the bot is taken when it costs at
/// most `1 - margin` of what is left of this one (any cheaper one when the lane turned bad). True when it switched.
fn replan(cx: &Ctx, container: Vec3, points: &mut Vec<Vec3>, next: &mut usize, priced: &mut u32) -> bool {
    let (Some(rule), Some(again)) = (cx.tuning.mods.raid_approach, cx.tuning.mods.raid_replan) else {
        return false;
    };
    let (Some(rest), Some(&gather)) = (points.get(*next..).filter(|r| !r.is_empty()), points.last()) else {
        return false;
    };
    if cx.me.feet.distance_xz(gather) <= again.lock_m {
        return false;
    }
    let way: Vec<Vec3> = std::iter::once(cx.me.feet).chain(rest.iter().copied()).chain(std::iter::once(container)).collect();
    let turned = lane_turned(cx, &way, *priced, &rule);
    if !turned && cx.now < priced.saturating_add((again.every_s * 1000.0) as u32) {
        return false;
    }
    *priced = cx.now;
    raid_counts::count(raid_counts::Count::Repriced);
    let left = price(cx, container, &rule, rest);
    let Some(((cost, best), _)) = cheapest(cx, container, &rule) else {
        return false;
    };
    let margin = if turned { 0.0 } else { again.margin };
    if cost < left * (1.0 - margin) && best != rest {
        raid_counts::count(if turned { raid_counts::Count::SwitchedLane } else { raid_counts::Count::Switched });
        (*points, *next) = (best, 0);
        return true;
    }
    false
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
    // [fix-team `mods.raid_approach`] Its chosen approach first; at its gather point the regroup (or in).
    if let State::Via { points, next, priced } = state {
        let reach = num("reach_m");
        while *next < points.len() && cx.me.feet.distance_xz(points[*next]) <= reach {
            *next += 1;
        }
        replan(cx, at, points, next, priced);
        if let Some(&p) = points.get(*next) {
            return Step::Continue(Intent::to(p, cx.pace_toward(p)).within(reach * 0.7).into_base());
        }
        let mate_by = |m: f32| {
            cx.view
                .team(cx.me.team)
                .any(|p| p.place != cx.me.place && p.alive && p.feet.distance_xz(cx.me.feet) <= m)
        };
        *state = match cx.tuning.mods.raid_regroup {
            Some(rule) if mate_by(rule.call_m) && !mate_by(rule.mate_m) => {
                raid_counts::count(raid_counts::Count::RegroupStop);
                State::Regroup { until: cx.now + (rule.wait_s * 1000.0) as u32 }
            }
            _ => State::GoIn,
        };
    }
    // [A296 item 5] First within the stage ring with a teammate coming: wait for him there.
    if let Some(rule) = cx.tuning.mods.raid_regroup {
        let d = cx.me.feet.distance_xz(at);
        let mate_by = |m: f32| {
            cx.view
                .team(cx.me.team)
                .any(|p| p.place != cx.me.place && p.alive && p.feet.distance_xz(cx.me.feet) <= m)
        };
        match *state {
            State::Approach if d <= rule.stage_m => {
                *state = if mate_by(rule.call_m) && !mate_by(rule.mate_m) {
                    #[cfg(test)]
                    raid_counts::count(raid_counts::Count::RegroupStop);
                    State::Regroup { until: cx.now + (rule.wait_s * 1000.0) as u32 }
                } else {
                    State::GoIn
                };
            }
            State::Regroup { until } if cx.now >= until || mate_by(rule.mate_m) => {
                #[cfg(test)]
                raid_counts::count(if mate_by(rule.mate_m) { raid_counts::Count::RegroupMate } else { raid_counts::Count::RegroupTimedOut });
                *state = State::GoIn;
            }
            _ => {}
        }
        if matches!(state, State::Regroup { .. }) {
            return Step::Continue(Intent::stay().crouched().looking(Look::At(at)));
        }
    }
    match state {
        State::Approach | State::GoIn => {
            let d = cx.me.feet.distance_xz(at);
            if d <= num("reach_m") {
                *state = State::Take {
                    tries: 0,
                    last_use: 0,
                };
                return Step::Continue(Intent::stay().crouched().looking(Look::use_box(at)));
            }
            // [A296 item 4, `behind_open.run_in`] A team behind runs the approach outside `sneak_m`.
            let run_in = cx.tuning.mods.behind_open.is_some_and(|open| open.run_in && open.behind(cx.sig.score_gap));
            let pace = if d < num("sneak_m") {
                Pace::Sneak
            } else if run_in {
                Pace::Run
            } else {
                cx.pace_toward(at)
            };
            Step::Continue(
                Intent::to(at, pace)
                    .within(num("reach_m") * 0.7)
                    .into_base(),
            )
        }
        State::Regroup { .. } | State::Via { .. } => Step::Continue(Intent::stay().crouched().looking(Look::At(at))),
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
        // The straight way (the approach's own test sets it).
        s.tuning.mods.raid_approach = None;
        s
    }

    /// [fix-team `mods.raid_approach`] With no human route, the gather point is the ring point round the container the team's
    /// knowledge covers least: a known enemy east of the container pushes it off the east; it walks there, then goes in.
    #[test]
    fn a_raider_gathers_on_the_approach_its_team_knows_least_covered() {
        use crate::v026g::bots::tuning::RaidApproach;
        let at = Vec3::new(68.0, 0.0, 30.0);
        let target = TargetRef::Container(1);
        let mut rng = Rng::new(1);
        let mut s = raid(2);
        s.tuning.mods.raid_approach = Some(RaidApproach::default());
        s.tuning.mods.raid_regroup = None;
        // A known enemy (heard) 25 m east of their container; the raider 63 m west of it, outside the ring.
        s.enemy(5, 1, Vec3::new(93.0, 0.0, 30.0), Some(Source::Heard));
        s.view.places[0].feet = Vec3::new(5.0, 0.0, 30.0);
        let mut state = start(target, &s.cx(0), &mut rng);
        let State::Via { points, next: 0, .. } = &state else { panic!("{state:?}") };
        let points = points.clone();
        let gather = *points.last().unwrap();
        assert!(points.iter().all(|p| (p.distance_xz(at) - 40.0).abs() < 3.5), "round the ring: {points:?}");
        assert!(gather.x < at.x, "away from the known enemy's side: {gather:?}");
        // It walks its points in turn, never across the container; at the last, it goes in.
        for &p in &points {
            let Step::Continue(intent) = step(&mut state, target, &s.cx(0), &mut rng) else { panic!("{state:?}") };
            assert!(matches!(intent.go, Go::To { at: q, .. } if q == p), "{intent:?}");
            s.view.places[0].feet = p;
        }
        let _ = step(&mut state, target, &s.cx(0), &mut rng);
        assert_eq!(state, State::GoIn);
        // Already inside the ring: no ring approach (it would walk back out), the straight way.
        s.view.places[0].feet = Vec3::new(40.0, 0.0, 30.0);
        assert_eq!(start(target, &s.cx(0), &mut rng), State::Approach);
    }

    /// [fix-team `mods.raid_replan`] On the way the approach is priced again every `every_s`: a much dearer one (a detour) is
    /// left for the cheapest from where the bot is; before `every_s` it is kept. A death on the lane makes the pricing due at once.
    #[test]
    fn a_raider_prices_its_approach_again_on_the_way() {
        use crate::v026g::bots::tuning::{RaidApproach, RaidReplan};
        let target = TargetRef::Container(1);
        let mut rng = Rng::new(1);
        let mut s = raid(2);
        s.tuning.mods.raid_approach = Some(RaidApproach::default());
        s.tuning.mods.raid_replan = Some(RaidReplan::default());
        s.tuning.mods.raid_regroup = None;
        s.view.places[0].feet = Vec3::new(5.0, 0.0, 30.0);
        let t0 = s.view.clock;
        // A detour: far north, then a gather point north of the container, 40 m off it.
        let detour = vec![Vec3::new(5.0, 0.0, 90.0), Vec3::new(68.0, 0.0, 90.0), Vec3::new(68.0, 0.0, 70.0)];
        let mut state = State::Via { points: detour.clone(), next: 0, priced: t0 };
        s.view.clock = t0 + 500;
        let _ = step(&mut state, target, &s.cx(0), &mut rng);
        assert!(matches!(&state, State::Via { points, priced, .. } if *points == detour && *priced == t0), "kept before every_s: {state:?}");
        s.view.clock = t0 + 2_000;
        let _ = step(&mut state, target, &s.cx(0), &mut rng);
        let State::Via { points, next: 0, priced } = &state else { panic!("{state:?}") };
        assert_ne!(*points, detour, "the detour left");
        assert_eq!(*priced, t0 + 2_000);
        // A team death on the lane since: priced at once, before `every_s`.
        let first = points[0];
        s.view.clock = t0 + 2_300;
        s.team.losses.push(crate::v026g::bots::know::Loss { at: first, time: s.view.clock, spawner: None });
        let _ = step(&mut state, target, &s.cx(0), &mut rng);
        assert!(matches!(&state, State::Via { priced, .. } if *priced == t0 + 2_300), "{state:?}");
    }

    /// [fix-team push `mods.push`] Three defenders known by their container: the taker of the team's push on it, with three
    /// members of the push by it, is offered the raid (the guard net of its group is 0); with no push, or its mates not in the
    /// push, or the switch off, it is not.
    #[test]
    fn a_pushing_group_counts_the_guard_net_of_its_own() {
        use crate::v026g::bots::push::{Push, Target};
        let mut s = raid(2);
        s.tuning.mods.push = Some(crate::v026g::bots::tuning::PushRule::default());
        s.time_left(100.0).points(0, 2);
        for (p, x) in [(5, 66.0), (7, 70.0), (9, 68.0)] {
            s.enemy(p, 1, Vec3::new(x, 0.0, 36.0), Some(Source::Heard));
        }
        for (p, z) in [(2, 26.0), (4, 30.0), (6, 34.0)] {
            s.bot(p, 0, Vec3::new(38.0, 0.0, z));
        }
        let raid_offered = |s: &Scenario| raids(s).iter().any(|c| c.kind == PlayKind::Raid);
        assert!(!raid_offered(&s), "no push: the guard of 3 scores it 0");
        let target = Target::Container { id: 1, at: Vec3::new(68.0, 0.0, 30.0), batteries: 1 };
        s.team.push = Push::Commit { target, taker: 0, escorts: vec![2, 4, 6], short_since: None, second: None };
        assert!(raid_offered(&s), "a push of 4: the net guard is 0");
        s.team.push = Push::Commit { target, taker: 0, escorts: vec![], short_since: None, second: None };
        assert!(!raid_offered(&s), "its mates not in the push: the defenders alone");
        s.team.push = Push::Commit { target, taker: 0, escorts: vec![2, 4, 6], short_since: None, second: None };
        s.tuning.mods.push = None;
        assert!(!raid_offered(&s), "off: the defenders alone");
    }

    /// [fix-team push, second taker] The push's taker on their container, holding three, with a second taker on it: both raid
    /// slots are offered (one each); with no second taker, one.
    #[test]
    fn a_push_with_a_second_taker_offers_the_second_raid_slot() {
        use crate::v026g::bots::push::{Push, Second, Target};
        let mut s = raid(2);
        s.tuning.mods.push = Some(crate::v026g::bots::tuning::PushRule::default());
        s.time_left(400.0).points(0, 1);
        s.bot(2, 0, Vec3::new(42.0, 0.0, 30.0));
        let slots = |s: &Scenario| raids(s).iter().filter(|c| c.kind == PlayKind::Raid).count();
        let target = Target::Container { id: 1, at: Vec3::new(68.0, 0.0, 30.0), batteries: 3 };
        s.team.push = Push::Commit { target, taker: 0, escorts: vec![], short_since: None, second: None };
        assert_eq!(slots(&s), 1);
        s.team.push = Push::Commit { target, taker: 0, escorts: vec![], short_since: None, second: Some(Second { place: 2, target }) };
        assert_eq!(slots(&s), 2);
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

    /// [mods.behind_open] 60 s into the match: no raid, level or behind, with the switch off; with it, a team one point behind
    /// raids, a level one does not.
    #[test]
    fn a_team_behind_raids_early_with_behind_open() {
        use crate::v026g::bots::tuning::BehindOpen;
        let early = |ours: u32, theirs: u32, on: bool| {
            let mut s = raid(1);
            s.time_left(s.view.total_s - 60.0).points(ours, theirs);
            s.tuning.mods.behind_open = on.then(BehindOpen::default);
            !raids(&s).is_empty()
        };
        assert!(!early(0, 1, false), "off");
        assert!(early(0, 1, true), "behind, on");
        assert!(!early(1, 1, true), "level, on");
        // An enemy known by their container: no early raid (a thin guard only).
        let mut s = raid(1);
        s.time_left(s.view.total_s - 60.0).points(0, 1);
        s.tuning.mods.behind_open = Some(BehindOpen::default());
        s.enemy(3, 1, Vec3::new(70.0, 0.0, 31.0), Some(Source::Seen));
        assert!(raids(&s).is_empty(), "guarded");
        // Behind with `run_in` (off by default: a running raider was heard), it runs the approach outside sneak_m.
        let mut s = raid(1);
        s.points(0, 1);
        s.tuning.mods.behind_open = Some(BehindOpen { run_in: true, ..BehindOpen::default() });
        let target = TargetRef::Container(1);
        let mut rng = Rng::new(1);
        let mut state = start(target, &s.cx(0), &mut rng);
        let Step::Continue(intent) = step(&mut state, target, &s.cx(0), &mut rng) else { panic!("{state:?}") };
        assert_eq!(intent.pace, Pace::Run, "{intent:?}");
    }

    /// [mods.raid_regroup.call] A team one point behind with the regroup on offers the raid's second slot (a mate for the
    /// stop); level, one slot.
    #[test]
    fn a_team_behind_calls_a_second_raider_with_the_regroup() {
        use crate::v026g::bots::tuning::RaidRegroup;
        let slots = |ours: u32, theirs: u32| {
            let mut s = raid(1);
            s.points(ours, theirs);
            s.tuning.mods.raid_regroup = Some(RaidRegroup::default());
            raids(&s).len()
        };
        assert_eq!(slots(0, 1), 2, "behind: a mate called");
        assert_eq!(slots(1, 1), 1, "level");
    }

    /// [mods.raid_regroup] Within the stage ring with a teammate 30 m off: it stops, crouched, looking at their container, until
    /// he comes within 12 m (or the wait ends), then goes in; with nobody near it goes in at once; off, no stop.
    #[test]
    fn a_raider_waits_for_a_teammate_short_of_their_container() {
        use crate::v026g::bots::tuning::RaidRegroup;
        let at = Vec3::new(68.0, 0.0, 30.0);
        let staged = |mate: Option<Vec3>, on: bool| {
            let mut s = raid(1);
            s.bot(0, 0, Vec3::new(35.0, 0.0, 30.0));
            if let Some(m) = mate {
                s.bot(2, 0, m);
            }
            s.tuning.mods.raid_regroup = on.then(RaidRegroup::default);
            s
        };
        let target = TargetRef::Container(1);
        let mut rng = Rng::new(1);
        // A mate 30 m behind: a stop.
        let s = staged(Some(Vec3::new(5.0, 0.0, 30.0)), true);
        let mut state = start(target, &s.cx(0), &mut rng);
        let Step::Continue(intent) = step(&mut state, target, &s.cx(0), &mut rng) else { panic!("{state:?}") };
        assert!(matches!(state, State::Regroup { .. }), "{state:?}");
        assert!(matches!(intent.go, Go::Stay) && intent.crouch && matches!(intent.look, Look::At(p) if p == at), "{intent:?}");
        // He comes within 12 m: in.
        let s = staged(Some(Vec3::new(28.0, 0.0, 30.0)), true);
        let _ = step(&mut state, target, &s.cx(0), &mut rng);
        assert_eq!(state, State::GoIn);
        // Alone: in at once; off: no stop.
        for (mate, on) in [(None, true), (Some(Vec3::new(5.0, 0.0, 30.0)), false)] {
            let s = staged(mate, on);
            let mut state = start(target, &s.cx(0), &mut rng);
            let Step::Continue(intent) = step(&mut state, target, &s.cx(0), &mut rng) else { panic!("{state:?}") };
            assert!(!matches!(state, State::Regroup { .. }), "{state:?}");
            assert!(matches!(intent.go, Go::To { .. }), "{intent:?}");
        }
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
