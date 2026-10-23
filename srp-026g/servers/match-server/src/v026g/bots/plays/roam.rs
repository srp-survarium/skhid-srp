//! Play 17, every bot's default: go forward toward the enemy base (where
//! humans fight best, `player_bots_behaviour.md` §3 play 15).
//!
//! **By the routes humans took** (`routes.rs`, `SRP_BOT_ROUTES`), when the
//! map has them: each route out of the bot's base is weighed by what a
//! human life on it gave (shrunk toward the side's mean when few lives
//! went that way), and one is drawn by weight, not always the best, so the
//! bots stay hard to read (research B2 "dual utility"). What the bot knows
//! shifts the weights:
//! - a known enemy on a route (any sense, a callout, the radar): less, or
//!   more for a bot that likes a fight (push at least `fighter_push`);
//! - a teammate killed on it in the last 20 s: less;
//! - teammates already out on it: less, so the team spreads;
//! - a sneaky bot (sneak at least `wide_sneak`) likes the wide ones.
//!
//! Points in the enemy's spawn area or a live hazard are left out. A known
//! battery, a thief or a carrier are other plays' business (collect,
//! intercept, escort win over a roam), so they do not weigh here. A route
//! that turns bad on the way (a teammate killed by its next point) ends
//! the run, and the commander picks again from where the bot stands.
//!
//! **Without routes**, to the exits of the enemy base, by a way of its
//! own: a waypoint off the straight way, drawn per run. The team spreads
//! over the exits (`FriendsNearTarget` inverted).
//!
//! It sprints only far from known danger ([`Ctx::pace_toward`]). At the
//! end it looks about a few seconds, then the commander picks again.
//!
//! **No headway:** on its way, a bot that has not got `headway_m` further
//! in `headway_s` ends the run and the commander picks again (another
//! detour, route or play). On level_06's Team Deathmatch whole teams stood
//! at one point by their base with a way open to the goal, unseen by the
//! stuck watch (it pushed nowhere); only the plays that failed at once
//! there (HoldExit and Crossfire with no spot) restarted them now and then.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};

pub const NUMBERS: &[(&str, f32)] = &[
    ("detour_min_m", 10.0),
    ("detour_max_m", 30.0),
    ("look_min_s", 3.0),
    ("look_max_s", 8.0),
    ("give_up_s", 90.0),
    // Human routes: lives worth of the side's mean reward added to each
    // route's own (a group of few lives says little).
    ("prior_lives", 10.0),
    // Within this of a route's point, a thing is "on" it, metres.
    ("on_route_m", 20.0),
    // Weight factors per thing on a route.
    ("enemy_factor", 0.6),
    ("fighter_enemy_factor", 1.3),
    ("fighter_push", 1.3),
    ("death_factor", 0.4),
    ("friend_factor", 0.6),
    ("wide_sneak", 1.3),
    // A route's point counts as reached within this, metres.
    ("reach_m", 5.0),
    // On its way, not this much further in `headway_s`: the run ends,
    // metres and seconds. GUESS: past a pair's wait for its follower (6 s)
    // and a short fight, well short of the 90 s give-up; measured on the
    // real-map bench (level_06 Team Deathmatch, where teams stood).
    ("headway_m", 3.0),
    ("headway_s", 10.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Roam,
        considerations: vec![
            considerations::near(250.0),
            Consideration::new(Input::FriendsNearTarget, 0.0, 3.0, Curve::Linear).inverted(),
        ],
        numbers: Default::default(),
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    // A pair's follower keeps with its leader instead (play 25).
    if super::buddy::leader(cx).is_some() {
        return;
    }
    // Human routes: one candidate; the route is drawn when it starts.
    if cx
        .bot
        .side
        .is_some_and(|side| cx.tactics.routes_of(side).next().is_some())
    {
        out.extend(cx.candidate(PlayKind::Roam, TargetRef::None, None));
        return;
    }
    let Some(enemy) = cx.bot.side.and_then(|side| cx.enemy_side(side)) else {
        // No side known yet: anywhere ahead will do.
        out.extend(cx.candidate(PlayKind::Roam, TargetRef::None, None));
        return;
    };
    let exits = cx.tactics.exits(enemy);
    for index in 0..exits.len() {
        out.extend(cx.candidate(PlayKind::Roam, TargetRef::Exit(index), None));
    }
    if exits.is_empty() {
        out.extend(cx.candidate(PlayKind::Roam, TargetRef::None, None));
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Along a human route's points; `next` is the one it heads for.
    Route {
        points: Vec<Vec3>,
        next: usize,
        /// Until when it waits for its pair's follower (0: not yet).
        wait_until: u32,
        headway: Headway,
    },
    /// To the waypoint off the straight way, then to the goal.
    Via {
        via: Vec3,
        goal: Vec3,
        headway: Headway,
    },
    Going {
        goal: Vec3,
        headway: Headway,
    },
    Looking {
        until: u32,
    },
}

/// Where the bot last got `headway_m` further on its way, and when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Headway {
    at: Vec3,
    time: u32,
}

impl Headway {
    fn new(cx: &Ctx) -> Self {
        Self {
            at: cx.me.feet,
            time: cx.now,
        }
    }

    /// Whether the bot has got nowhere for `headway_s` (it stood or went
    /// round within `headway_m`).
    fn none(&mut self, cx: &Ctx) -> bool {
        let num = |name| cx.num(PlayKind::Roam, name);
        if cx.me.feet.distance_xz(self.at) > num("headway_m") {
            *self = Self::new(cx);
            return false;
        }
        cx.now > self.time + (num("headway_s") * 1000.0) as u32
    }
}

impl Ctx<'_> {
    /// Run (sprint where the way allows) unless a known danger is within
    /// 30 m of the bot or of `at`; walk then (play 17's sprint rule).
    pub fn pace_toward(&self, at: Vec3) -> Pace {
        let near = |p: Vec3| {
            self.memory
                .tracks()
                .iter()
                .any(|t| t.at.distance_xz(p) < 30.0)
                || self
                    .team
                    .danger_within(self.now, 20_000)
                    .any(|d| d.at.distance_xz(p) < 30.0)
        };
        if near(self.me.feet) || near(at) {
            Pace::Walk
        } else {
            Pace::Run
        }
    }

    /// Where roaming heads for `target`: the exit, else a floor ahead
    /// toward the other side's spawns.
    fn roam_goal(&self, target: TargetRef) -> Option<Vec3> {
        self.point_of(target).or_else(|| {
            let enemy = self.bot.side.and_then(|side| self.enemy_side(side))?;
            let spawns: Vec<Vec3> = self
                .tactics
                .world
                .map
                .spawns
                .iter()
                .filter(|s| s.team == enemy)
                .map(|s| s.position)
                .collect();
            let first = *spawns.first()?;
            // Halfway there: never into their spawn area.
            Some(self.me.feet + (first - self.me.feet) * 0.5)
        })
    }
}

/// A human route of the bot's side, drawn by weight (the module's doc);
/// its points from the first one ahead of the bot.
fn draw_route(cx: &Ctx, rng: &mut Rng) -> Option<Vec<Vec3>> {
    let side = cx.bot.side?;
    let num = |name| cx.num(PlayKind::Roam, name);
    let all: Vec<_> = cx.tactics.routes_of(side).collect();
    let lives: f32 = all.iter().map(|r| r.lives as f32).sum();
    if lives <= 0.0 {
        return None;
    }
    let mean = all.iter().map(|r| r.reward * r.lives as f32).sum::<f32>() / lives;
    let routes: Vec<Vec<Vec3>> = all
        .iter()
        .map(|route| {
            route
                .points
                .iter()
                .copied()
                .filter(|&p| cx.tactics.fine_goal(p, cx.now, Some(side)))
                .collect()
        })
        .collect();
    // How wide a route swings off the straight line from its first point
    // to its last (for the sneaky).
    let width = |points: &[Vec3]| -> f32 {
        let (Some(&a), Some(&b)) = (points.first(), points.last()) else {
            return 0.0;
        };
        let (dx, dz) = (b.x - a.x, b.z - a.z);
        let length = dx.hypot(dz).max(1.0);
        points
            .iter()
            .map(|p| ((p.x - a.x) * dz - (p.z - a.z) * dx).abs() / length)
            .fold(0.0, f32::max)
    };
    let widest = routes.iter().map(|p| width(p)).fold(1.0, f32::max);
    let on = num("on_route_m");
    let fighter = cx.bot.personality.push >= num("fighter_push");
    let sneaky = cx.bot.personality.sneak >= num("wide_sneak");
    let mine = cx.me.feet;
    let weights: Vec<f32> = all
        .iter()
        .zip(&routes)
        .map(|(route, points)| {
            if points.is_empty() {
                return 0.0;
            }
            let near = |at: Vec3| points.iter().any(|p| p.distance_xz(at) < on);
            let prior = num("prior_lives");
            let mut w =
                (route.reward * route.lives as f32 + mean * prior) / (route.lives as f32 + prior);
            let enemies = cx.memory.tracks().iter().filter(|t| near(t.at)).count();
            let factor = if fighter {
                num("fighter_enemy_factor")
            } else {
                num("enemy_factor")
            };
            w *= factor.powi(enemies as i32);
            let deaths = cx
                .team
                .danger_within(cx.now, 20_000)
                .filter(|d| near(d.at))
                .count();
            w *= num("death_factor").powi(deaths as i32);
            // Teammates out on it (not those still about the bot).
            let friends = cx
                .view
                .team(cx.me.team)
                .filter(|p| {
                    p.place != cx.me.place
                        && Some(p.place) != cx.bot.partner
                        && p.feet.distance_xz(mine) > on
                        && near(p.feet)
                })
                .count();
            w *= num("friend_factor").powi(friends as i32);
            if sneaky {
                w *= 1.0 + 0.5 * width(points) / widest;
            }
            w.max(0.0)
        })
        .collect();
    let total: f32 = weights.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let mut pick = rng.unit() * total;
    let index = weights
        .iter()
        .position(|&w| {
            pick -= w;
            w > 0.0 && pick < 0.0
        })
        .or_else(|| weights.iter().rposition(|&w| w > 0.0))?;
    let points = &routes[index];
    // From the point nearest the bot; the next one if it is there already.
    let nearest = (0..points.len()).min_by(|&a, &b| {
        points[a]
            .distance_xz(mine)
            .total_cmp(&points[b].distance_xz(mine))
    })?;
    let first = if points[nearest].distance_xz(mine) < num("reach_m") * 3.0 {
        nearest + 1
    } else {
        nearest
    };
    (first < points.len()).then(|| points[first..].to_vec())
}

pub fn start(target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    if target == TargetRef::None
        && let Some(points) = draw_route(cx, rng)
    {
        return State::Route {
            points,
            next: 0,
            wait_until: 0,
            headway: Headway::new(cx),
        };
    }
    // The exit or the way ahead; with no side known, a floor 20-60 m off.
    let goal = cx.roam_goal(target).or_else(|| {
        let grid = &cx.tactics.world.grid;
        (0..32).find_map(|_| {
            let point = grid.position(rng.below(grid.len()) as mapdata::NodeId);
            ((20.0..=60.0).contains(&point.distance_xz(cx.me.feet))
                && cx.tactics.fine_goal(point, cx.now, cx.bot.side))
            .then_some(point)
        })
    });
    let Some(goal) = goal else {
        return State::Looking {
            until: cx.now + 2_000,
        };
    };
    let me = cx.me.feet;
    let (dx, dz) = (goal.x - me.x, goal.z - me.z);
    let length = dx.hypot(dz);
    let headway = Headway::new(cx);
    if length < 30.0 {
        return State::Going { goal, headway };
    }
    for _ in 0..6 {
        let offset = rng.range(
            cx.num(PlayKind::Roam, "detour_min_m"),
            cx.num(PlayKind::Roam, "detour_max_m"),
        ) * if rng.unit() < 0.5 { -1.0 } else { 1.0 };
        let t = rng.range(0.35, 0.65);
        let via = Vec3::new(
            me.x + dx * t - dz / length * offset,
            me.y,
            me.z + dz * t + dx / length * offset,
        );
        if cx.tactics.fine_goal(via, cx.now, cx.bot.side) {
            return State::Via { via, goal, headway };
        }
    }
    State::Going { goal, headway }
}

pub fn step(state: &mut State, _target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    if let Some((_, _, started)) = cx.bot.current
        && cx.now > started + (cx.num(PlayKind::Roam, "give_up_s") * 1000.0) as u32
    {
        return Step::Done("roamed long enough");
    }
    match state {
        State::Route {
            points,
            next,
            wait_until,
            headway,
        } => {
            let reach = cx.num(PlayKind::Roam, "reach_m");
            while *next < points.len() && cx.me.feet.distance_xz(points[*next]) < reach {
                *next += 1;
            }
            let Some(&at) = points.get(*next) else {
                let look = rng.range(
                    cx.num(PlayKind::Roam, "look_min_s"),
                    cx.num(PlayKind::Roam, "look_max_s"),
                );
                *state = State::Looking {
                    until: cx.now + (look * 1000.0) as u32,
                };
                return Step::Continue(Intent::stay());
            };
            // A teammate killed by the next point lately: the route has
            // turned bad; the commander picks again from here.
            let on = cx.num(PlayKind::Roam, "on_route_m") * 0.75;
            if cx
                .team
                .danger_within(cx.now, 10_000)
                .any(|d| d.at.distance_xz(at) < on)
            {
                return Step::Done("the route turned bad");
            }
            // The pair's follower left behind: wait a little, once a run
            // (play 25).
            if super::buddy::lagging(cx).is_some() {
                if *wait_until == 0 {
                    *wait_until = cx.now + (cx.num(PlayKind::Buddy, "wait_s") * 1000.0) as u32;
                }
                if cx.now < *wait_until {
                    *headway = Headway::new(cx);
                    return Step::Continue(Intent::stay().looking(Look::Path));
                }
            }
            if headway.none(cx) {
                return Step::Done("made no headway");
            }
            Step::Continue(Intent::to(at, cx.pace_toward(at)).within(reach * 0.6))
        }
        State::Via { via, goal, headway } => {
            let (via, goal) = (*via, *goal);
            if headway.none(cx) {
                return Step::Done("made no headway");
            }
            if cx.me.feet.distance_xz(via) < 4.0 {
                *state = State::Going {
                    goal,
                    headway: *headway,
                };
            }
            Step::Continue(Intent::to(via, cx.pace_toward(via)).within(3.0))
        }
        State::Going { goal, headway } => {
            let goal = *goal;
            if cx.me.feet.distance_xz(goal) < 3.0 {
                let look = rng.range(
                    cx.num(PlayKind::Roam, "look_min_s"),
                    cx.num(PlayKind::Roam, "look_max_s"),
                );
                *state = State::Looking {
                    until: cx.now + (look * 1000.0) as u32,
                };
                return Step::Continue(Intent::stay());
            }
            if headway.none(cx) {
                return Step::Done("made no headway");
            }
            Step::Continue(Intent::to(goal, cx.pace_toward(goal)).within(2.0))
        }
        State::Looking { until } => {
            if cx.now >= *until {
                return Step::Done("looked about at its goal");
            }
            Step::Continue(Intent::stay().crouched())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::DangerSpot;
    use crate::v026g::bots::routes::HumanRoute;
    use crate::v026g::bots::view::Mode;

    /// Side 0's two human routes on the field: north past the wall's end
    /// (reward 3), south (reward 1), 20 lives each.
    fn with_routes(s: &mut Scenario) {
        let route = |reward: f32, points: &[(f32, f32)]| HumanRoute {
            team: 0,
            lives: 20,
            reward,
            points: points.iter().map(|&(x, z)| Vec3::new(x, 0.0, z)).collect(),
        };
        s.tactics.set_routes(vec![
            route(3.0, &[(20.0, 30.0), (35.0, 33.0), (50.0, 33.0)]),
            route(1.0, &[(20.0, 8.0), (30.0, 5.0), (52.0, 5.0)]),
        ]);
    }

    /// How often each route is drawn in `n` starts (by its last point's z).
    fn draws(s: &Scenario, n: u64) -> (usize, usize) {
        let (mut north, mut south) = (0, 0);
        for seed in 0..n {
            let mut rng = Rng::new(seed);
            match start(TargetRef::None, &s.cx(0), &mut rng) {
                State::Route { points, .. } if points.last().unwrap().z > 20.0 => north += 1,
                State::Route { .. } => south += 1,
                other => panic!("{other:?}"),
            }
        }
        (north, south)
    }

    #[test]
    fn human_routes_are_drawn_by_their_reward_not_always_the_best() {
        let mut s = Scenario::field(Mode::Batteries);
        with_routes(&mut s);
        s.bot(0, 0, Vec3::new(5.0, 0.0, 15.0));
        // One roam candidate, the route drawn at its start.
        let roams: Vec<_> = s
            .candidates(0)
            .into_iter()
            .filter(|c| c.kind == PlayKind::Roam)
            .collect();
        assert_eq!(roams.len(), 1, "{roams:#?}");
        assert_eq!(roams[0].target, TargetRef::None);
        // Weights 2.67 : 1.33 (shrunk toward the mean of 2).
        let (north, south) = draws(&s, 400);
        assert!((220..=310).contains(&north), "{north} {south}");
        assert!(south >= 90, "{north} {south}");
    }

    #[test]
    fn a_teammate_killed_on_a_route_turns_bots_to_the_other() {
        let mut s = Scenario::field(Mode::Batteries);
        with_routes(&mut s);
        s.bot(0, 0, Vec3::new(5.0, 0.0, 15.0));
        let now = s.view.clock;
        s.team.danger.push(DangerSpot {
            at: Vec3::new(36.0, 0.0, 33.0),
            time: now - 5_000,
            victim: 3,
        });
        let (north, south) = draws(&s, 400);
        assert!(south > north, "{north} {south}");
    }

    /// A bot that gets nowhere on its way for `headway_s` ends the run, so
    /// the commander picks again; one that walks on does not.
    #[test]
    fn a_bot_that_gets_nowhere_on_its_way_ends_the_run() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(5.0, 0.0, 15.0));
        let mut rng = Rng::new(1);
        let goal = Vec3::new(50.0, 0.0, 15.0);
        let mut state = State::Going {
            goal,
            headway: Headway::new(&s.cx(0)),
        };
        // Standing (or going round within 3 m): done after 10 s.
        let mut steps = Vec::new();
        for second in 1..=12 {
            s.view.clock += 1_000;
            s.view.places[0].feet.x = 5.0 + (second % 2) as f32;
            steps.push(step(&mut state, TargetRef::Exit(0), &s.cx(0), &mut rng));
        }
        assert!(
            steps[..9].iter().all(|st| matches!(st, Step::Continue(_))),
            "{steps:?}"
        );
        assert_eq!(steps.last(), Some(&Step::Done("made no headway")));
        // Walking 1.5 m a second: it goes on.
        s.view.places[0].feet.x = 5.0;
        let mut state = State::Going {
            goal,
            headway: Headway::new(&s.cx(0)),
        };
        for _ in 0..20 {
            s.view.clock += 1_000;
            s.view.places[0].feet.x += 1.5;
            let st = step(&mut state, TargetRef::Exit(0), &s.cx(0), &mut rng);
            assert!(matches!(st, Step::Continue(_)), "{st:?}");
        }
    }

    #[test]
    fn a_bot_walks_its_human_route_point_by_point_then_looks_about() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        with_routes(&mut s);
        s.bot(0, 0, Vec3::new(5.0, 0.0, 15.0));
        let steps = s.run(0, PlayKind::Roam, TargetRef::None, 200, 300, 3.5);
        let goals: Vec<Vec3> = steps
            .iter()
            .filter_map(|s| match s {
                Step::Continue(Intent {
                    go: Go::To { at, .. },
                    ..
                }) => Some(*at),
                _ => None,
            })
            .collect();
        // Every goal is one of the route's points, in order.
        let mut last = None;
        for at in &goals {
            if last != Some(*at) {
                assert!(
                    [8.0f32, 5.0, 30.0, 33.0]
                        .iter()
                        .any(|z| (at.z - z).abs() < 0.6),
                    "{at:?}"
                );
                last = Some(*at);
            }
        }
        assert!(!goals.is_empty());
        assert_eq!(steps.last(), Some(&Step::Done("looked about at its goal")));
        let end = s.view.places[0].feet;
        assert!(end.x > 45.0, "{end:?}");
    }
}
