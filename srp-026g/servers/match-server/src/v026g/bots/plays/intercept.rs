//! Play 2, carrier interception, and the answer to a theft
//! (`player_bots_behaviour.md` §3 play 2, §4 "a battery the team picked up
//! disappears" and "an enemy steals from our container"): an enemy carrier
//! is slow, cannot shoot and is bound for his container. The bot meets him
//! at a point on his likely way home that it reaches 1-2 s before him
//! (drawn per run), else chases his last known place.
//!
//! - Where he is comes only through [`Ctx::known_place`]: the bot's
//!   memory, the thief cheat included. A theft whose thief is not known
//!   yet is answered from where the battery was last known.
//! - His likely way is the route from him to his container (off our spawn
//!   area, as his own routes are); points along it are tried from him on,
//!   the first one the bot reaches `lead` seconds before him is the meet.
//!   It is planned again every few seconds and once he is past it.
//! - Who goes is chosen by how well it can stop him (its *reach*, a
//!   factor on its score): a bot that gets ahead of him on his way (the
//!   straight way, cheap; the step plans the route) first; behind him,
//!   one whose weapon's band already reaches him; then one that gets into
//!   its band before he is home; last one that does not.
//! - A recent theft of ours is an alarm (rank Alarm) for the bots of the
//!   team that can stop him (reach `reach_closing` or better): two slot
//!   keys per theft, so at most two take it, the best placed first (the
//!   commander serves bots by their best score). Any other enemy carrier
//!   is an objective for one bot.
//! - **Commitment** (F3 of the owner's match 0x7298fb57, 706-752 s: six
//!   defenders 21-60 m from a carrier took Intercept, PairUp, Cut,
//!   HoldExit in turn, none stopped him): a running intercept is kept at
//!   its target and in the alarm band, so no fight or roam play replaces
//!   it, while he still carries, is short of his container and is known
//!   fairly: the team's own senses (seen, heard, told) placed him lately,
//!   or the theft is recent (the HUD's notice). It may keep either of the
//!   carry's two slots, so a better placed bot can still join it.
//! - It ends when he no longer carries (dropped, home, dead), gives up
//!   when he is lost, gets into his base, or after a time drawn per run.

use super::*;
use crate::v026g::bots::reflex::WeaponKind;
use crate::v026g::bots::view::BatteryState;

pub const NUMBERS: &[(&str, f32)] = &[
    // A theft this recent is an alarm.
    ("theft_recent_s", 30.0),
    // A walk is the ground distance times this (as `Ctx::path_dist`).
    ("path_factor", 1.25),
    // The bot's speed on the way (sprinting where it may; GUESS between
    // walk 3.5 and sprint 5.0 m/s) and a carrier's (he does not sprint,
    // LIKELY, as `collect` assumes).
    ("my_speed", 4.5),
    ("carrier_speed", 3.5),
    // The bot is at the meet this long before him, drawn per run.
    ("lead_min_s", 1.0),
    ("lead_max_s", 2.0),
    // Points along his way are tried this far apart.
    ("meet_step_m", 4.0),
    ("meet_reach_m", 3.0),
    // He is past the meet when this much nearer his home than it.
    ("passed_m", 3.0),
    ("replan_s", 2.0),
    // A meet planned again this near the last one keeps the last one.
    ("keep_m", 5.0),
    // This near him it goes straight at him.
    ("close_m", 8.0),
    ("chase_reach_m", 2.0),
    ("give_up_min_s", 45.0),
    ("give_up_max_s", 60.0),
    // Commitment: the team's own senses placed him within this, seconds
    // (else a theft of ours within `theft_recent_s`) ...
    ("commit_known_s", 10.0),
    // ... and he is farther than this from his container, metres.
    ("commit_home_m", 5.0),
    // The reach, a factor on the score: ahead of him on his way; behind
    // him within its weapon's band; behind him and in band before he is
    // home; he is home first. By the bench (F3, 20 seeds): 1, 0.9, 0.45,
    // 0.15 left fewer bots on intercepts and the carriers killed fell by
    // 19 of 248 (deliveries +48); these hold both.
    ("reach_cut", 1.0),
    ("reach_in_band", 1.0),
    ("reach_closing", 0.8),
    ("reach_late", 0.5),
    // A camping carrier (audit #6: the owner's match 0x4da3b51d, a carrier
    // 8 m from his container for 4 minutes, 29 lone intercepts, 8 dead):
    // the team's senses placed him for `camp_s` all within their error and
    // `camp_m` of one point, past `camp_half` of the way from our container
    // to his. He is no alarm and no commitment, and two bots are sent
    // (`camp_pair` 1: both of the carry's slots offered), not one at a
    // time. His score times `camp_cut` (1: off) was measured and left off
    // (player_bots_behaviour.md §2.7).
    ("camp_s", 20.0),
    ("camp_m", 3.0),
    ("camp_half", 0.5),
    ("camp_cut", 1.0),
    ("camp_pair", 1.0),
];

pub fn tuning() -> PlayTuning {
    PlayTuning {
        rank: Rank::Objective,
        considerations: vec![
            considerations::near(200.0),
            considerations::health_above(0.25),
            considerations::personality(),
            considerations::modifier(Input::ObjectiveUrgency, 0.6),
        ],
        numbers: Default::default(),
    }
}

/// What a target stands for while an enemy carries its battery: the
/// carrier if the view can tell (a theft's thief otherwise).
struct Carry {
    carrier: Option<Place>,
}

impl Ctx<'_> {
    fn intercept_is_enemy(&self, place: Place) -> bool {
        self.view
            .place(place)
            .is_some_and(|p| p.team != self.me.team)
    }

    /// The carry the target stands for, while an enemy carries.
    fn intercept_carry(&self, target: TargetRef) -> Option<Carry> {
        match target {
            TargetRef::Place(p) => (self.intercept_is_enemy(p)
                && self
                    .view
                    .batteries
                    .contains(&BatteryState::Carried(Some(p))))
            .then_some(Carry { carrier: Some(p) }),
            TargetRef::Point(at) => {
                let theft = self.team.thefts.iter().rev().find(|t| t.at == at)?;
                match self.view.batteries.get(theft.battery)? {
                    BatteryState::Carried(carrier)
                        if carrier.is_none_or(|p| self.intercept_is_enemy(p)) =>
                    {
                        Some(Carry {
                            carrier: carrier.or(theft.thief),
                        })
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Where the carrier is as far as the bot knows.
    fn intercept_him(&self, target: TargetRef, carry: &Carry) -> Option<Vec3> {
        let known = carry.carrier.and_then(|p| self.known_place(p));
        match target {
            TargetRef::Point(at) => Some(known.unwrap_or(at)),
            _ => known,
        }
    }

    /// His container: his team's, else the one not ours.
    fn intercept_home(&self, carry: &Carry) -> Option<Vec3> {
        let team = carry
            .carrier
            .and_then(|p| self.view.place(p))
            .map(|p| p.team);
        self.view
            .containers
            .iter()
            .find(|c| team.map_or(c.team != self.me.team, |team| c.team == team))
            .map(|c| c.at)
    }

    /// Whether a run at `target` would fail at its first step, as
    /// [`step`] plans it: no meet ahead of him (the earliest lead drawn)
    /// or he is close, and the chase's goal (he) is in his base or a zone.
    fn intercept_fails_at_once(&self, target: TargetRef) -> bool {
        let num = |name| self.num(PlayKind::Intercept, name);
        let Some(carry) = self.intercept_carry(target) else {
            return false;
        };
        let (Some(him), Some(home)) = (
            self.intercept_him(target, &carry),
            self.intercept_home(&carry),
        ) else {
            return false;
        };
        if self.tactics.fine_goal(him, self.now, self.bot.side) {
            return false;
        }
        self.me.feet.distance_xz(him) < num("close_m")
            || self.intercept_meet(him, home, num("lead_min_s")).is_none()
    }

    /// The battery a (running) intercept's target is about: the one its
    /// carrier carries, or its theft's.
    fn intercept_battery(&self, target: TargetRef) -> Option<usize> {
        match target {
            TargetRef::Place(p) => self
                .view
                .batteries
                .iter()
                .position(|b| *b == BatteryState::Carried(Some(p))),
            TargetRef::Point(at) => self
                .team
                .thefts
                .iter()
                .rev()
                .find(|t| t.at == at)
                .map(|t| t.battery),
            _ => None,
        }
    }

    /// Whether the bot's running intercept at `target` holds (commitment):
    /// he still carries, a run would not fail at once, he is short of his
    /// container, and he is known fairly: the team's own senses placed
    /// him within `commit_known_s`, or a theft of the battery is recent
    /// (`theft_recent`, the HUD's notice). No cheat counts.
    fn intercept_committed(&self, target: TargetRef, theft_recent: bool) -> bool {
        let num = |name| self.num(PlayKind::Intercept, name);
        let Some(carry) = self.intercept_carry(target) else {
            return false;
        };
        if self.intercept_fails_at_once(target) {
            return false;
        }
        let (Some(him), Some(home)) = (
            self.intercept_him(target, &carry),
            self.intercept_home(&carry),
        ) else {
            return false;
        };
        if him.distance_xz(home) <= num("commit_home_m") {
            return false;
        }
        let window = (num("commit_known_s") * 1000.0) as u32;
        let sensed = carry
            .carrier
            .and_then(|p| self.memory.tracks().iter().find(|t| t.who == p))
            .and_then(|t| t.own)
            .is_some_and(|(_, time)| time + window >= self.now);
        sensed || theft_recent
    }

    /// Whether the carrier of `carry` camps deep in his ground (the
    /// numbers `camp_*`): the team's senses ([`TeamKnowledge::carrier_still`])
    /// have him still, past `camp_half` of the way from our container to
    /// his.
    fn intercept_camping(&self, carry: &Carry) -> bool {
        let num = |name| self.num(PlayKind::Intercept, name);
        let Some(who) = carry.carrier else {
            return false;
        };
        let Some(at) = self.team.carrier_still(
            who,
            self.now,
            (num("camp_s") * 1000.0) as u32,
            num("camp_m"),
        ) else {
            return false;
        };
        let (Some(ours), Some(his)) = (self.view.container_of(self.me.team), self.intercept_home(carry))
        else {
            return false;
        };
        let ours = ours.at;
        let (dx, dz) = (his.x - ours.x, his.z - ours.z);
        let length = dx * dx + dz * dz;
        length >= 1.0 && ((at.x - ours.x) * dx + (at.z - ours.z) * dz) / length >= num("camp_half")
    }

    /// The top of the bot's weapon's band, metres (play 11's; its weapon 1,
    /// the rifle's where it is not known).
    fn intercept_band_top(&self) -> f32 {
        let kind = self.bot.weapon.unwrap_or(WeaponKind::Rifle);
        self.tuning.aim.combat.band(kind).1
    }

    /// How well the bot can stop the carrier the target stands for, a
    /// factor on its score: `reach_cut` if it gets ahead of him on his way
    /// (the straight way: cheap, for every bot at every commander tick);
    /// behind him, `reach_in_band` within its weapon's band,
    /// `reach_closing` if it gets into its band before he is home,
    /// `reach_late` if not. 1 when it cannot tell.
    fn intercept_reach(&self, target: TargetRef) -> f32 {
        let num = |name| self.num(PlayKind::Intercept, name);
        let Some(carry) = self.intercept_carry(target) else {
            return 1.0;
        };
        let (Some(him), Some(home)) = (
            self.intercept_him(target, &carry),
            self.intercept_home(&carry),
        ) else {
            return 1.0;
        };
        if self
            .intercept_meet_by(him, home, num("lead_min_s"), false)
            .is_some()
        {
            return num("reach_cut");
        }
        let d = self.me.feet.distance_xz(him);
        let top = self.intercept_band_top();
        if d <= top {
            return num("reach_in_band");
        }
        let gain = (num("my_speed") - num("carrier_speed")).max(0.1);
        let his_s = him.distance_xz(home) * num("path_factor") / num("carrier_speed").max(0.1);
        if (d - top) / gain < his_s {
            num("reach_closing")
        } else {
            num("reach_late")
        }
    }

    /// The point on his way home the bot reaches `lead_s` before him, the
    /// first from him on.
    fn intercept_meet(&self, him: Vec3, home: Vec3, lead_s: f32) -> Option<Vec3> {
        self.intercept_meet_by(him, home, lead_s, true)
    }

    /// [`Self::intercept_meet`], along his route if `routed`, else along
    /// the straight way with the detour factor.
    fn intercept_meet_by(&self, him: Vec3, home: Vec3, lead_s: f32, routed: bool) -> Option<Vec3> {
        let num = |name| self.num(PlayKind::Intercept, name);
        let his_side = self.bot.side.and_then(|side| self.enemy_side(side));
        let step = num("meet_step_m").max(0.5);
        // Points along his way: (his metres to it, the point).
        let along = |length: f32, at: &dyn Fn(f32) -> (f32, Vec3)| -> Vec<(f32, Vec3)> {
            let n = (length / step).ceil() as usize;
            (1..=n).map(|i| at((i as f32 * step).min(length))).collect()
        };
        let route = if routed {
            self.tactics.route(him, home, self.now, his_side)
        } else {
            None
        };
        let points = match route {
            Some(route) => along(route.length(), &|s| (s, route.point_at(s))),
            None => {
                // No route known: the straight way, with a detour factor.
                let length = him.distance_xz(home);
                let factor = num("path_factor");
                along(length, &|s| (s * factor, him + (home - him) * (s / length)))
            }
        };
        let (his_speed, my_speed) = (num("carrier_speed"), num("my_speed"));
        points
            .into_iter()
            .find(|&(his_m, q)| {
                self.path_dist(q) / my_speed + lead_s <= his_m / his_speed
                    && self.tactics.fine_goal(q, self.now, self.bot.side)
            })
            .map(|(_, q)| q)
    }
}

pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    if !cx.view.mode.has_batteries() || cx.me.carrying.is_some() {
        return;
    }
    let recent = (cx.num(PlayKind::Intercept, "theft_recent_s") * 1000.0) as u32;
    let running = match cx.bot.current {
        Some((PlayKind::Intercept, target, _)) => Some(target),
        _ => None,
    };
    // Scored, times the reach.
    let scored = |target: TargetRef, slot: u32, reach: f32| {
        cx.candidate(PlayKind::Intercept, target, Some(slot))
            .map(|mut candidate| {
                candidate.score = Score::new(candidate.score.get() * reach);
                candidate
            })
            .filter(|candidate| !candidate.score.is_zero())
    };
    for (battery, state) in cx.view.batteries.iter().enumerate() {
        let BatteryState::Carried(carrier) = *state else {
            continue;
        };
        if carrier.is_some_and(|p| !cx.intercept_is_enemy(p)) {
            continue;
        }
        let theft = cx
            .team
            .thefts
            .iter()
            .filter(|t| t.battery == battery && t.time + recent >= cx.now)
            .max_by_key(|t| t.time);
        let key = battery as u32 * 2;
        // A camping carrier: no alarm, no commitment, a pair sent.
        let camp_cut = cx.num(PlayKind::Intercept, "camp_cut");
        let camp_pair = cx.num(PlayKind::Intercept, "camp_pair") >= 0.5;
        let camping = (camp_cut < 1.0 || camp_pair) && cx.intercept_camping(&Carry { carrier });
        let cut = if camping { camp_cut } else { 1.0 };
        // Its running intercept of this carry, committed: kept at its
        // target (a theft's spot does not turn into his place and back) in
        // the alarm band, on either of the carry's slots.
        if let Some(target) = running
            && !camping
            && cx.intercept_battery(target) == Some(battery)
            && cx.intercept_committed(target, theft.is_some())
        {
            let reach = cx.intercept_reach(target);
            for slot in [key, key + 1] {
                if let Some(mut candidate) = scored(target, slot, reach) {
                    candidate.rank = Rank::Alarm;
                    out.push(candidate);
                }
            }
            continue;
        }
        // His place if known, else the theft's spot.
        let target = match carrier.filter(|&p| cx.known_place(p).is_some()) {
            Some(p) => TargetRef::Place(p),
            None => match theft {
                Some(theft) => TargetRef::Point(theft.at),
                None => continue,
            },
        };
        if cx.point_of(target).is_none() {
            continue;
        }
        // A chase that fails at its first step (he is in his base, with
        // no meet left on his way) is no candidate: else it is taken and
        // dropped again at once (27 times in the owner's match of
        // 2026-10-16).
        if cx.intercept_fails_at_once(target) {
            continue;
        }
        let reach = cx.intercept_reach(target) * cut;
        if !camping && theft.is_some() && reach >= cx.num(PlayKind::Intercept, "reach_closing") {
            for slot in [key, key + 1] {
                if let Some(mut candidate) = scored(target, slot, reach) {
                    candidate.rank = Rank::Alarm;
                    out.push(candidate);
                }
            }
        } else if camping && camp_pair {
            // Two at once, not one after another into his ground.
            for slot in [key, key + 1] {
                out.extend(scored(target, slot, reach));
            }
        } else {
            out.extend(scored(target, key, reach));
        }
    }
}

/// What every step of a run keeps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub until: u32,
    pub lead_s: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// To the meet, ahead of him on his way.
    Cut { meet: Vec3, planned: u32, run: Run },
    /// At the meet, watching his way.
    Wait { meet: Vec3, planned: u32, run: Run },
    /// After him.
    Chase { planned: u32, run: Run },
}

impl State {
    fn run(&self) -> Run {
        match self {
            Self::Cut { run, .. } | Self::Wait { run, .. } | Self::Chase { run, .. } => *run,
        }
    }
}

pub fn start(_target: TargetRef, cx: &Ctx, rng: &mut Rng) -> State {
    let num = |name| cx.num(PlayKind::Intercept, name);
    let give_up = rng.range(num("give_up_min_s"), num("give_up_max_s"));
    State::Chase {
        // Planned at the first step.
        planned: 0,
        run: Run {
            until: cx.now + (give_up * 1000.0) as u32,
            lead_s: rng.range(num("lead_min_s"), num("lead_max_s")),
        },
    }
}

pub fn step(state: &mut State, target: TargetRef, cx: &Ctx, _rng: &mut Rng) -> Step {
    let num = |name| cx.num(PlayKind::Intercept, name);
    let run = state.run();
    let Some(carry) = cx.intercept_carry(target) else {
        return Step::Done("he no longer carries it");
    };
    if cx.now >= run.until {
        return Step::Failed("he got away");
    }
    let Some(him) = cx.intercept_him(target, &carry) else {
        return Step::Failed("lost him");
    };
    let Some(home) = cx.intercept_home(&carry) else {
        return Step::Failed("no container of his");
    };
    let replan_ms = (num("replan_s") * 1000.0) as u32;
    let me = cx.me.feet;
    let close = me.distance_xz(him) < num("close_m");
    let past = |meet: Vec3| him.distance_xz(home) < meet.distance_xz(home) - num("passed_m");
    // Planned again: a meet, else the chase.
    let replan = |state: &mut State| {
        let last = match *state {
            State::Cut { meet, .. } | State::Wait { meet, .. } => Some(meet),
            State::Chase { .. } => None,
        };
        *state = match cx.intercept_meet(him, home, run.lead_s) {
            Some(meet) if !close => State::Cut {
                // Kept if it hardly moved (the meet drifts as the bot
                // nears it).
                meet: last
                    .filter(|last| last.distance_xz(meet) <= num("keep_m") && !past(*last))
                    .unwrap_or(meet),
                planned: cx.now,
                run,
            },
            _ => State::Chase {
                planned: cx.now,
                run,
            },
        };
    };
    match *state {
        State::Cut { meet, planned, .. } | State::Wait { meet, planned, .. }
            if close || past(meet) || cx.now >= planned + replan_ms =>
        {
            replan(state);
        }
        State::Chase { planned, .. } if cx.now >= planned + replan_ms => replan(state),
        _ => {}
    }
    if let State::Cut { meet, planned, .. } = *state
        && me.distance_xz(meet) <= num("meet_reach_m")
    {
        *state = State::Wait { meet, planned, run };
    }
    match *state {
        State::Cut { meet, .. } => Step::Continue(
            Intent::to(meet, Pace::Run)
                .within(num("meet_reach_m") * 0.7)
                .looking(Look::Path),
        ),
        State::Wait { meet, .. } => Step::Continue(
            Intent::to(meet, Pace::Walk)
                .within(num("meet_reach_m"))
                .crouched()
                .looking(Look::At(him)),
        ),
        State::Chase { .. } => {
            if !cx.tactics.fine_goal(him, cx.now, cx.bot.side) {
                return Step::Failed("he is in his base");
            }
            Step::Continue(Intent::to(him, Pace::Run).within(num("chase_reach_m")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Scenario;
    use super::*;
    use crate::v026g::bots::know::{CheatKind, Source, Theft};
    use crate::v026g::bots::team::{Asking, Assignment, assign};
    use crate::v026g::bots::view::Mode;

    const THIEF: Vec3 = Vec3::new(22.0, 0.0, 36.0);
    const THEIR_HOME: Vec3 = Vec3::new(68.0, 0.0, 30.0);

    /// An enemy (place 1) has just taken battery 0 from our container at
    /// (12, 30) and is at (22, 36), known by the thief cheat; the bot at
    /// (34, 0, 31) stands between him and his container.
    fn scene(theft: bool) -> Scenario {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(34.0, 0.0, 31.0));
        s.enemy(1, 1, THIEF, Some(Source::Cheat(CheatKind::Thief)));
        s.view.places[1].carrying = Some(0);
        s.battery(BatteryState::Carried(Some(1)));
        if theft {
            s.team.thefts.push(Theft {
                battery: 0,
                at: Vec3::new(12.0, 0.0, 30.0),
                time: s.view.clock - 2_000,
                thief: Some(1),
            });
        }
        s
    }

    fn intercepts(s: &Scenario, place: Place) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&s.cx(place), &mut out);
        out
    }

    #[test]
    fn a_theft_is_an_alarm_for_the_bots_that_can_stop_him_and_any_carrier_an_objective() {
        let s = scene(true);
        let found = intercepts(&s, 0);
        assert_eq!(found.len(), 2, "{found:#?}");
        assert!(found.iter().all(|c| c.rank == Rank::Alarm));
        assert!(found.iter().all(|c| c.target == TargetRef::Place(1)));
        assert_ne!(found[0].slot, found[1].slot);
        // No theft: an objective, one slot.
        let found = intercepts(&scene(false), 0);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rank, Rank::Objective);
        // Far behind him, near his home (the flanker's band 35 m): it
        // would be in its band only after he is home; no alarm, and its
        // score is cut to the reach.
        let mut s = scene(true);
        s.enemy(1, 1, Vec3::new(55.0, 0.0, 31.0), Some(Source::Seen));
        s.bot(0, 0, Vec3::new(4.0, 0.0, 31.0));
        let late = intercepts(&s, 0);
        assert!(!late.is_empty() && late.iter().all(|c| c.rank == Rank::Objective));
        assert!(
            (s.cx(0).intercept_reach(TargetRef::Place(1)) - 0.5).abs() < 1e-6,
            "{late:#?}"
        );
        // A friend carrying, or a battery lying about: none.
        let mut s = scene(false);
        s.human(1, 0, THIEF);
        assert!(intercepts(&s, 0).is_empty());
        let mut s = scene(false);
        s.view.batteries[0] = BatteryState::Free(THIEF);
        assert!(intercepts(&s, 0).is_empty());
        // Not known and no theft: nothing to go on.
        let mut s = scene(false);
        s.memory = Default::default();
        assert!(intercepts(&s, 0).is_empty());
        // Not known, a theft: the theft's spot.
        let mut s = scene(true);
        s.memory = Default::default();
        let found = intercepts(&s, 0);
        assert!(
            found
                .iter()
                .all(|c| c.target == TargetRef::Point(Vec3::new(12.0, 0.0, 30.0))),
            "{found:#?}"
        );
    }

    /// A carrier in his base (his spawn area, x 75 here) with no meet
    /// left ahead of him: the chase would fail at its first step, so it is
    /// no candidate (it was taken and dropped at once, in bursts); once he
    /// is out, it is one again, and the step chases.
    #[test]
    fn a_carrier_in_his_base_is_no_candidate() {
        let mut s = scene(false);
        s.bot(0, 0, Vec3::new(50.0, 0.0, 31.0));
        s.enemy(1, 1, Vec3::new(72.0, 0.0, 30.0), Some(Source::Seen));
        let cx = s.cx(0);
        assert!(
            !cx.tactics
                .fine_goal(Vec3::new(72.0, 0.0, 30.0), s.view.clock, Some(0))
        );
        let mut rng = Rng::new(1);
        let mut state = start(TargetRef::Place(1), &cx, &mut rng);
        assert_eq!(
            step(&mut state, TargetRef::Place(1), &cx, &mut rng),
            Step::Failed("he is in his base")
        );
        assert!(intercepts(&s, 0).is_empty());
        s.enemy(1, 1, Vec3::new(45.0, 0.0, 31.0), Some(Source::Seen));
        assert_eq!(intercepts(&s, 0).len(), 1);
    }

    /// Audit #6 (the owner's match 0x4da3b51d: a carrier 8 m from his
    /// container for 4 minutes, 29 lone intercepts, 8 dead): a carrier the
    /// team's senses have had still for 20 s (the thief cheat's places, 7 m
    /// off) past the middle, in his ground, is no alarm and is offered on
    /// both of the carry's slots (a pair; `camp_pair`), his score times
    /// `camp_cut`; one on the move, or still on our side, is as before.
    #[test]
    fn a_carrier_camping_in_his_ground_is_no_alarm_and_gets_a_pair() {
        let offsets = [(7.0f32, 0.0f32), (-5.0, 5.0), (0.0, -7.0), (4.0, 4.0), (-6.0, -3.0)];
        // (the bot, his places over 24 s from `from` to `to`, a theft, the
        // cut, the pair).
        let run = |me: Vec3, from: Vec3, to: Vec3, theft: bool, cut: f32, pair: f32| {
            let mut s = scene(theft);
            s.bot(0, 0, me);
            s.enemy(1, 1, to, Some(Source::Cheat(CheatKind::Thief)));
            let numbers = &mut s
                .tuning
                .plays
                .entry(PlayKind::Intercept)
                .or_insert_with(|| default_tuning(PlayKind::Intercept))
                .numbers;
            numbers.insert("camp_cut".into(), cut);
            numbers.insert("camp_pair".into(), pair);
            for k in 0..9u32 {
                let (dx, dz) = offsets[k as usize % offsets.len()];
                let at = from + (to - from) * (k as f32 / 8.0) + Vec3::new(dx, 0.0, dz);
                let time = s.view.clock - 24_000 + k * 3_000;
                s.team.carrier_at(1, at, 10.0, time, |_| true);
            }
            let found = intercepts(&s, 0);
            assert!(!found.is_empty(), "{found:#?}");
            (found[0].rank, found[0].score.get(), found.len())
        };
        // Within its band of him.
        let me = Vec3::new(30.0, 0.0, 36.0);
        let camper = Vec3::new(50.0, 0.0, 33.0);
        // Both off: the theft's alarm on both slots.
        let (rank, full, n) = run(me, camper, camper, true, 1.0, 0.0);
        assert_eq!((rank, n), (Rank::Alarm, 2));
        // The pair (the default): both slots, no alarm, the score whole.
        assert_eq!(run(me, camper, camper, true, 1.0, 1.0), (Rank::Objective, full, 2));
        // The cut alone: one slot, the score cut.
        let (rank, cut, n) = run(me, camper, camper, true, 0.25, 0.0);
        assert_eq!((rank, n), (Rank::Objective, 1));
        assert!((cut - full * 0.25).abs() < 1e-4, "{cut} {full}");
        // On the move (30 m in 24 s): as before.
        let walking = run(me, Vec3::new(20.0, 0.0, 33.0), camper, true, 0.25, 1.0);
        assert_eq!(walking, (Rank::Alarm, full, 2));
        // Still, on our side of the middle: as before (one slot, no theft).
        let (me, ours) = (Vec3::new(4.0, 0.0, 31.0), Vec3::new(32.0, 0.0, 36.0));
        let near = run(me, ours, ours, false, 1.0, 0.0);
        assert_eq!(near.2, 1);
        assert_eq!(run(me, ours, ours, false, 0.25, 1.0), near);
    }

    #[test]
    fn at_most_two_bots_answer_a_theft() {
        let mut s = scene(true);
        s.bot(2, 0, Vec3::new(30.0, 0.0, 38.0));
        s.bot(3, 0, Vec3::new(40.0, 0.0, 36.0));
        let asking = [0, 2, 3]
            .into_iter()
            .map(|place| Asking {
                place,
                candidates: intercepts(&s, place),
                current: None,
            })
            .collect();
        let out = assign(asking, s.view.clock);
        assert_eq!(out.len(), 2, "{out:?}");
        assert_ne!(out[0].1.slot, out[1].1.slot);
    }

    /// F3 (the owner's match 0x7298fb57, 706-752 s): a running intercept
    /// is kept, at its target and in the alarm band, while he is known
    /// fairly and short of his container; PairUp at 0.98 (bot 7 at
    /// 709.1 s) does not replace it. A cheat's place, an old sight or his
    /// container near end it.
    #[test]
    fn a_running_intercept_is_kept_while_he_is_known_and_short_of_home() {
        // No theft: a carrier of a battery from the field, seen.
        let mut s = scene(false);
        s.enemy(1, 1, THIEF, Some(Source::Seen));
        assert!(intercepts(&s, 0).iter().all(|c| c.rank == Rank::Objective));
        s.current = Some((
            PlayKind::Intercept,
            TargetRef::Place(1),
            s.view.clock - 5_000,
        ));
        let kept = intercepts(&s, 0);
        assert_eq!(kept.len(), 2, "{kept:#?}");
        assert!(
            kept.iter()
                .all(|c| c.rank == Rank::Alarm && c.target == TargetRef::Place(1))
        );
        let current = Assignment {
            kind: PlayKind::Intercept,
            target: TargetRef::Place(1),
            slot: kept[0].slot,
            score: kept[0].score.get(),
            since: s.view.clock - 5_000,
            reason: "the best play",
        };
        let mut all = kept.clone();
        all.push(Candidate {
            kind: PlayKind::PairUp,
            target: TargetRef::Place(1),
            rank: Rank::Fight,
            score: Score::new(0.98),
            why: Default::default(),
            slot: None,
        });
        let out = assign(
            vec![Asking {
                place: 0,
                candidates: all,
                current: Some(&current),
            }],
            s.view.clock,
        );
        assert_eq!((out[0].1.kind, out[0].2), (PlayKind::Intercept, false));
        // Known by the thief cheat alone: not committed.
        let mut cheat = scene(false);
        cheat.current = s.current;
        assert!(
            intercepts(&cheat, 0)
                .iter()
                .all(|c| c.rank == Rank::Objective)
        );
        // Seen 12 s ago, the cheat's place since: not committed.
        let mut old = scene(false);
        old.memory = Default::default();
        let now = old.view.clock;
        for (time, source) in [
            (now - 12_000, Source::Seen),
            (now, Source::Cheat(CheatKind::Thief)),
        ] {
            old.memory.update(
                &[crate::v026g::bots::know::Percept {
                    who: 1,
                    at: THIEF,
                    error_m: 0.0,
                    time,
                    source,
                    busy: false,
                }],
                time,
                |_| 200,
                3_000,
                20_000,
            );
        }
        old.current = s.current;
        assert!(
            intercepts(&old, 0)
                .iter()
                .all(|c| c.rank == Rank::Objective)
        );
        // By his container: not committed.
        let mut home = scene(false);
        home.enemy(1, 1, Vec3::new(64.0, 0.0, 30.0), Some(Source::Seen));
        home.current = s.current;
        assert!(intercepts(&home, 0).iter().all(|c| c.rank != Rank::Alarm));
    }

    /// A run at a theft's spot (the thief not known yet) keeps that target
    /// once he is seen, while the theft is recent: no Point, Place, Point
    /// churn (bot 3 at 693.7-694.8 s).
    #[test]
    fn a_committed_run_at_a_thefts_spot_keeps_its_target() {
        let mut s = scene(true);
        s.memory = Default::default();
        let spot = TargetRef::Point(Vec3::new(12.0, 0.0, 30.0));
        assert!(intercepts(&s, 0).iter().all(|c| c.target == spot));
        s.current = Some((PlayKind::Intercept, spot, s.view.clock - 1_000));
        s.enemy(1, 1, THIEF, Some(Source::Seen));
        let kept = intercepts(&s, 0);
        assert_eq!(kept.len(), 2, "{kept:#?}");
        assert!(
            kept.iter()
                .all(|c| c.target == spot && c.rank == Rank::Alarm)
        );
        // Without the run: his place.
        s.current = None;
        assert!(
            intercepts(&s, 0)
                .iter()
                .all(|c| c.target == TargetRef::Place(1))
        );
    }

    /// The reach by weapon: behind him at 39 m a flanker's Uzi (band top
    /// 35 m) must close in, a support's AK-74U (50 m) has him in its band;
    /// a bot ahead of him on his way cuts him off whatever it carries.
    #[test]
    fn the_reach_reads_the_weapons_band_and_the_cut_off() {
        let mut s = scene(false);
        s.enemy(1, 1, Vec3::new(40.0, 0.0, 31.0), Some(Source::Seen));
        s.bot(0, 0, Vec3::new(1.0, 0.0, 31.0));
        let reach = |s: &Scenario| s.cx(0).intercept_reach(TargetRef::Place(1));
        s.personality = 0;
        assert_eq!(s.tuning.personalities[0].name, "flanker");
        assert!((reach(&s) - 0.8).abs() < 1e-6);
        s.personality = 1;
        assert_eq!(s.tuning.personalities[1].name, "support");
        assert!((reach(&s) - 1.0).abs() < 1e-6);
        // Further back the Uzi would be in band only after he is home.
        s.bot(0, 0, Vec3::new(2.0, 0.0, 31.0));
        s.personality = 0;
        s.enemy(1, 1, Vec3::new(55.0, 0.0, 31.0), Some(Source::Seen));
        assert!((reach(&s) - 0.5).abs() < 1e-6);
        s.personality = 2;
        s.enemy(1, 1, Vec3::new(40.0, 0.0, 31.0), Some(Source::Seen));
        s.bot(0, 0, Vec3::new(52.0, 0.0, 31.0));
        assert!((reach(&s) - 1.0).abs() < 1e-6);
    }

    /// The band follows the weapon the bot holds, not its role's name: a
    /// rusher with an SMG (tiers 9-10 and 13-15) reaches like the SMG, a
    /// rusher with its shotgun like the shotgun.
    #[test]
    fn the_reach_follows_the_weapon_held_not_the_personality() {
        let mut s = scene(false);
        s.enemy(1, 1, Vec3::new(40.0, 0.0, 31.0), Some(Source::Seen));
        s.bot(0, 0, Vec3::new(1.0, 0.0, 31.0));
        let reach = |s: &Scenario| s.cx(0).intercept_reach(TargetRef::Place(1));
        s.personality = 2;
        assert_eq!(s.tuning.personalities[2].name, "rusher");
        s.weapon = Some(WeaponKind::Smg);
        let smg = reach(&s);
        s.personality = 0;
        s.weapon = None;
        assert_eq!(s.tuning.personalities[0].name, "flanker");
        assert!((smg - reach(&s)).abs() < 1e-6, "{smg}");
        s.personality = 2;
        s.weapon = Some(WeaponKind::Shotgun);
        assert!(reach(&s) < smg, "the shotgun's band is shorter");
    }

    #[test]
    fn it_meets_him_ahead_on_his_way_and_watches_him_then_gives_up() {
        let mut s = scene(true);
        let steps = s.run(0, PlayKind::Intercept, TargetRef::Place(1), 300, 300, 4.5);
        let Some(Step::Continue(Intent {
            go: Go::To { at: meet, .. },
            ..
        })) = steps.first()
        else {
            panic!("{:?}", steps.first());
        };
        // Between him and his container.
        assert!(meet.distance_xz(THEIR_HOME) < THIEF.distance_xz(THEIR_HOME));
        assert!(steps.iter().any(|step| matches!(
            step,
            Step::Continue(Intent { crouch: true, look: Look::At(at), .. }) if at.distance_xz(THIEF) < 1.0
        )));
        assert_eq!(steps.last(), Some(&Step::Failed("he got away")));
    }

    #[test]
    fn behind_him_it_chases_and_it_ends_when_he_drops_it() {
        let mut s = scene(true);
        s.bot(0, 0, Vec3::new(18.0, 0.0, 39.0));
        let steps = s.run(0, PlayKind::Intercept, TargetRef::Place(1), 3, 300, 4.5);
        assert!(
            matches!(steps[0], Step::Continue(Intent { go: Go::To { at, .. }, .. }) if at == THIEF),
            "{steps:?}"
        );
        let mut rng = Rng::new(1);
        let mut state = start(TargetRef::Place(1), &s.cx(0), &mut rng);
        s.view.batteries[0] = BatteryState::Free(THIEF);
        assert_eq!(
            step(&mut state, TargetRef::Place(1), &s.cx(0), &mut rng),
            Step::Done("he no longer carries it")
        );
    }
}
