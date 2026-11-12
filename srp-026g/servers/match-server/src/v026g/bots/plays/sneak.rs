//! Play 4, sneak into their base (`player_bots_behaviour.md` §3 play 4;
//! §4 "several enemies dead at once: push or sneak window"): when several
//! enemies are dead (respawn timers) or none is known near their
//! container, and it holds batteries, go round their known places, crouch
//! the last metres, steal one, and leave by another way.
//!
//! - The way in goes by a waypoint off the straight way, drawn per run as
//!   roam's detour, the one farthest from every known enemy of a few
//!   tries; the waypoint is never in their spawn area (`fine_goal`). From
//!   it, the way to their container is the objective's (`into_base`: their
//!   spawn area is open for it).
//! - The last `sneak_m` are crouched (`Pace::Sneak`), fire held: a sneak
//!   fires only once found, and a seen enemy within `contact_m` aborts it
//!   (`Step::Failed("contact")`), the reflex taking over.
//! - At the container: use (bit 21) every ~1.2 s, at most `tries`. With
//!   the battery, out by a waypoint toward our container, the farthest from
//!   the way it came of a few tries; there the play is done and `collect`
//!   brings it home. While it carries, its candidate is an alarm at the
//!   top, so the getaway is not cut short by `collect`'s own alarm.
//! - One bot per container (its slot); a personality of the sneak family
//!   takes it most.

use super::*;
use crate::v026g::bots::curve::{Consideration, Curve, Input};
use crate::v026g::drive::key;

pub const NUMBERS: &[(&str, f32)] = &[
    // The share of their team dead that opens the window...
    ("dead_share", 0.4),
    // ...or no enemy known this near their container.
    ("far_m", 50.0),
    // The way in: a waypoint this far off the straight way, of this many
    // tries.
    ("detour_min_m", 15.0),
    ("detour_max_m", 35.0),
    ("detour_tries", 8.0),
    ("via_reach_m", 4.0),
    // Crouched this near their container.
    ("sneak_m", 20.0),
    ("reach_m", 2.0),
    ("use_every_s", 1.2),
    ("tries", 4.0),
    // A seen enemy this near aborts it.
    ("contact_m", 25.0),
    // The way out: a waypoint this far from their container, toward ours
    // (within `leave_spread_deg` of that way), of `detour_tries` tries.
    ("leave_min_m", 30.0),
    ("leave_max_m", 45.0),
    ("leave_spread_deg", 70.0),
    ("give_up_min_s", 100.0),
    ("give_up_max_s", 130.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Objective,
        considerations: vec![
            considerations::personality(),
            considerations::pushing(),
            // Enemies dead make it; none dead leaves it a third (the
            // window was opened by their being far).
            Consideration::new(
                Input::EnemiesDead,
                0.0,
                1.0,
                Curve::Piecewise {
                    points: vec![(0.0, 0.35), (0.5, 1.0), (1.0, 1.0)],
                },
            ),
            Consideration::new(Input::EnemiesNearTarget, 0.0, 3.0, Curve::Linear).inverted(),
            considerations::near(250.0),
            considerations::health_above(0.5),
            considerations::cooldown(45.0),
        ],
        numbers: Default::default(),
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !cx.view.objective().has_homes() {
        return;
    }
    let Some(theirs) = cx.view.containers.iter().find(|c| c.team != cx.me.team) else {
        return;
    };
    let target = TargetRef::Container(theirs.id);
    let slot = Some(u32::from(theirs.id));
    let running = matches!(cx.bot.current, Some((PlayKind::Sneak, t, _)) if t == target);
    if running && cx.me.carrying.is_some() {
        // The getaway: at the top until it is out (the step ends it).
        out.push(Candidate {
            kind: PlayKind::Sneak,
            target,
            rank: Rank::Alarm,
            score: Score::ONE,
            why: Explain::default(),
            slot: slot.map(|key| SlotKey {
                kind: PlayKind::Sneak,
                key,
            }),
        });
        return;
    }
    if cx.me.carrying.is_some() || theirs.batteries == 0 {
        return;
    }
    let num = |name| cx.num(PlayKind::Sneak, name);
    let window =
        cx.sig.enemies_dead >= num("dead_share") || cx.enemies_near(theirs.at, num("far_m")) == 0;
    if running || window {
        out.extend(cx.candidate(PlayKind::Sneak, target, slot));
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// To the waypoint off the straight way.
    Round { via: Vec3, until: u32 },
    /// To their container; `came` is where the way in came from.
    Approach { came: Vec3, until: u32 },
    Steal {
        came: Vec3,
        tries: u8,
        last_use: u32,
        until: u32,
    },
    /// Out with it, by another way.
    Leave { via: Vec3, until: u32 },
}

impl State {
    fn until(&self) -> u32 {
        match *self {
            Self::Round { until, .. }
            | Self::Approach { until, .. }
            | Self::Steal { until, .. }
            | Self::Leave { until, .. } => until,
        }
    }
}

impl Ctx<'_> {
    /// The nearest known enemy to `at`, metres (far if none).
    fn sneak_clearance(&self, at: Vec3) -> f32 {
        self.memory
            .tracks()
            .iter()
            .map(|t| t.at.distance_xz(at))
            .fold(1e6, f32::min)
    }

    /// The way in: a waypoint off the straight way to `goal`, the clearest
    /// of a few drawn.
    fn sneak_via(&self, goal: Vec3, rng: &mut Rng) -> Option<Vec3> {
        let num = |name| self.num(PlayKind::Sneak, name);
        let me = self.me.feet;
        let (dx, dz) = (goal.x - me.x, goal.z - me.z);
        let length = dx.hypot(dz);
        if length < num("sneak_m") * 1.5 {
            return None;
        }
        let mut best: Option<(f32, Vec3)> = None;
        for _ in 0..num("detour_tries") as usize {
            let offset = rng.range(num("detour_min_m"), num("detour_max_m"))
                * if rng.unit() < 0.5 { -1.0 } else { 1.0 };
            let t = rng.range(0.35, 0.65);
            let via = Vec3::new(
                me.x + dx * t - dz / length * offset,
                me.y,
                me.z + dz * t + dx / length * offset,
            );
            if !self.tactics.fine_goal(via, self.now, self.bot.side) {
                continue;
            }
            let clear = self.sneak_clearance(via);
            if best.is_none_or(|(c, _)| clear > c) {
                best = Some((clear, via));
            }
        }
        best.map(|(_, via)| via)
    }

    /// The way out from their container at `from`: a waypoint toward our
    /// container, the farthest from `came` (and from known enemies) of a
    /// few drawn.
    fn sneak_out(&self, from: Vec3, came: Vec3, rng: &mut Rng) -> Option<Vec3> {
        let num = |name| self.num(PlayKind::Sneak, name);
        let home = self.view.container_of(self.me.team)?.at;
        let toward = (home.z - from.z).atan2(home.x - from.x);
        let spread = num("leave_spread_deg").to_radians();
        let mut best: Option<(f32, Vec3)> = None;
        for _ in 0..num("detour_tries") as usize {
            let angle = toward + rng.range(-spread, spread);
            let r = rng.range(num("leave_min_m"), num("leave_max_m"));
            let via = Vec3::new(from.x + r * angle.cos(), from.y, from.z + r * angle.sin());
            if !self.tactics.fine_goal(via, self.now, self.bot.side) {
                continue;
            }
            let worth = via.distance_xz(came) + self.sneak_clearance(via).min(30.0);
            if best.is_none_or(|(w, _)| worth > w) {
                best = Some((worth, via));
            }
        }
        best.map(|(_, via)| via)
    }
}

pub fn start(target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    let num = |name| cx.num(PlayKind::Sneak, name);
    let until = cx.now + (rng.range(num("give_up_min_s"), num("give_up_max_s")) * 1000.0) as u32;
    let via = cx.point_of(target).and_then(|goal| cx.sneak_via(goal, rng));
    match via {
        Some(via) => State::Round { via, until },
        None => State::Approach {
            came: cx.me.feet,
            until,
        },
    }
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Sneak, name);
    if cx.now >= state.until() {
        return Step::Failed("took too long");
    }
    let me = cx.me.feet;
    if cx
        .memory
        .all_seen(cx.now, cx.view.clock)
        .iter()
        .any(|t| t.at().distance_xz(me) < num("contact_m"))
    {
        return Step::Failed("contact");
    }
    let Some(goal) = cx.point_of(target) else {
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
    let until = state.until();
    let reach = num("reach_m");
    // With the battery: out.
    if cx.me.carrying.is_some()
        && let State::Approach { came, .. } | State::Steal { came, .. } = *state
    {
        let Some(via) = cx.sneak_out(goal, came, rng) else {
            return Step::Done("stole it; no way out but the straight one");
        };
        *state = State::Leave { via, until };
    }
    match *state {
        State::Round { via, .. } => {
            if me.distance_xz(via) <= num("via_reach_m") {
                *state = State::Approach { came: via, until };
            }
            Step::Continue(Intent::to(via, cx.pace_toward(via)).within(num("via_reach_m") * 0.7))
        }
        State::Approach { came, .. } => {
            if count == 0 {
                return Step::Failed("their container is empty");
            }
            let d = me.distance_xz(goal);
            if d <= reach {
                *state = State::Steal {
                    came,
                    tries: 0,
                    last_use: 0,
                    until,
                };
                return Step::Continue(
                    Intent::stay()
                        .crouched()
                        .looking(Look::use_box(goal))
                        .hold_fire()
                        .into_base(),
                );
            }
            let intent = if d < num("sneak_m") {
                Intent::to(goal, Pace::Sneak).hold_fire()
            } else {
                Intent::to(goal, cx.pace_toward(goal))
            };
            Step::Continue(intent.within(reach * 0.7).into_base())
        }
        State::Steal {
            ref mut tries,
            ref mut last_use,
            ..
        } => {
            if count == 0 {
                return Step::Failed("their container is empty");
            }
            if *tries >= num("tries") as u8 {
                return Step::Failed("took nothing: blocked");
            }
            let mut intent = Intent::stay()
                .crouched()
                .looking(Look::use_box(goal))
                .hold_fire()
                .into_base();
            if cx.now >= *last_use + (num("use_every_s") * 1000.0) as u32 {
                *tries += 1;
                *last_use = cx.now;
                intent = intent.pressing(key::USE);
            }
            Step::Continue(intent)
        }
        State::Leave { via, .. } => {
            if cx.me.carrying.is_none() {
                return Step::Failed("lost it on the way out");
            }
            if me.distance_xz(via) <= num("via_reach_m") {
                return Step::Done("got out with it");
            }
            // A carrier cannot shoot, and does not sprint (LIKELY); the way
            // out may start in their spawn area.
            Step::Continue(
                Intent::to(via, Pace::Walk)
                    .within(num("via_reach_m") * 0.7)
                    .hold_fire()
                    .into_base(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::Source;
    use crate::v026g::bots::team::{Asking, assign};
    use crate::v026g::bots::view::Mode;

    const THEIRS: Vec3 = Vec3::new(68.0, 0.0, 30.0);

    /// Two of their three dead, their container (68, 30) holding two
    /// batteries, the bot at (20, 35).
    fn scene() -> Scenario {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 35.0));
        for place in [1, 2] {
            s.enemy(place, 1, Vec3::new(75.0, 0.0, 15.0), None);
            s.view.places[place].alive = false;
        }
        s.enemy(3, 1, Vec3::new(70.0, 0.0, 5.0), None);
        s.container_count(1, 2);
        s.signals();
        s
    }

    fn sneaks(s: &Scenario, place: Place) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(place), &mut out);
        out
    }

    #[test]
    fn a_sneak_when_they_are_dead_or_far_and_their_container_holds_batteries() {
        let s = scene();
        let found = sneaks(&s, 0);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].target, TargetRef::Container(1));
        // Their container empty: none.
        let mut s = scene();
        s.container_count(1, 0);
        assert!(sneaks(&s, 0).is_empty());
        // All alive and one known by their container: none.
        let mut s = scene();
        for place in [1, 2] {
            s.view.places[place].alive = true;
        }
        s.signals();
        assert!(!sneaks(&s, 0).is_empty(), "far: still a window");
        s.enemy(3, 1, Vec3::new(62.0, 0.0, 30.0), Some(Source::Heard));
        assert!(sneaks(&s, 0).is_empty());
        // Team Deathmatch: never.
        let mut s = scene();
        s.view.mode = Mode::TeamDeathmatch;
        assert!(sneaks(&s, 0).is_empty());
        // Running and carrying: an alarm, the getaway.
        let mut s = scene();
        s.current = Some((PlayKind::Sneak, TargetRef::Container(1), s.view.clock));
        s.view.places[0].carrying = Some(0);
        let found = sneaks(&s, 0);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rank, Rank::Alarm);
    }

    #[test]
    fn one_sneak_per_container() {
        let mut s = scene();
        s.bot(4, 0, Vec3::new(20.0, 0.0, 38.0));
        let asking = [0, 4]
            .into_iter()
            .map(|place| Asking {
                place,
                candidates: sneaks(&s, place),
                current: None,
                committed: false,
            })
            .collect();
        assert_eq!(assign(asking, s.view.clock).len(), 1);
    }

    #[test]
    fn it_goes_round_crouches_the_last_metres_into_their_base_and_tries_four_uses() {
        let mut s = scene();
        let steps = s.run(0, PlayKind::Sneak, TargetRef::Container(1), 300, 300, 3.5);
        // The first goal is the waypoint, off the straight way.
        let Step::Continue(Intent {
            go: Go::To { at: via, .. },
            objective: false,
            ..
        }) = steps[0]
        else {
            panic!("{:?}", steps[0]);
        };
        let (me, dir) = (
            Vec3::new(20.0, 0.0, 35.0),
            THEIRS - Vec3::new(20.0, 0.0, 35.0),
        );
        let off = ((via.x - me.x) * dir.z - (via.z - me.z) * dir.x).abs() / dir.x.hypot(dir.z);
        assert!(off >= 10.0, "{via:?} {off}");
        // Crouched, fire held and into their base for the last metres.
        assert!(steps.iter().any(|step| matches!(
            step,
            Step::Continue(Intent {
                go: Go::To { .. },
                pace: Pace::Sneak,
                fire: false,
                objective: true,
                ..
            })
        )));
        let uses = steps
            .iter()
            .filter(|s| matches!(s, Step::Continue(i) if i.press & key::USE != 0))
            .count();
        assert_eq!(uses, 4, "{steps:?}");
        assert_eq!(steps.last(), Some(&Step::Failed("took nothing: blocked")));
    }

    #[test]
    fn it_aborts_on_contact_and_leaves_with_the_battery_by_another_way() {
        let mut s = scene();
        let mut rng = Rng::new(4);
        s.current = Some((PlayKind::Sneak, TargetRef::Container(1), s.view.clock));
        let mut state = start(TargetRef::Container(1), &s.cx(0), &mut rng);
        let State::Round { via: came, .. } = state else {
            panic!("{state:?}");
        };
        // At the container, it steals one.
        s.view.places[0].feet = Vec3::new(67.0, 0.0, 31.0);
        state = State::Approach {
            came,
            until: state.until(),
        };
        let _ = step(&mut state, TargetRef::Container(1), &s.cx(0), &mut rng);
        assert!(matches!(state, State::Steal { .. }), "{state:?}");
        s.view.places[0].carrying = Some(0);
        let out = step(&mut state, TargetRef::Container(1), &s.cx(0), &mut rng);
        let State::Leave { via, .. } = state else {
            panic!("{state:?}");
        };
        assert!(via.distance_xz(came) > 15.0, "{via:?} {came:?}");
        assert!(s.tactics.fine_goal(via, s.view.clock, Some(0)));
        assert!(matches!(out, Step::Continue(Intent { fire: false, .. })));
        s.view.places[0].feet = via;
        assert_eq!(
            step(&mut state, TargetRef::Container(1), &s.cx(0), &mut rng),
            Step::Done("got out with it")
        );
        // A seen enemy near: contact.
        s.enemy(5, 1, via + Vec3::new(10.0, 0.0, 0.0), Some(Source::Seen));
        assert_eq!(
            step(&mut state, TargetRef::Container(1), &s.cx(0), &mut rng),
            Step::Failed("contact")
        );
    }
}
