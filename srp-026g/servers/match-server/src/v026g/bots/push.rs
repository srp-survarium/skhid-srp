//! [fix-team push `mods.push`] A team behind on points pushes as one (`owner-final3/PUSH-DESIGN.md` §3; the owner's local
//! matches of 2026-11-19: "pushing alone in battery matches when they are behind is not enough"). A push is one team-level
//! state with a target and a group, not a mood that raises scores (P5: nothing held a push to its objective):
//!
//! - **When:** the team is `behind_by` points behind in a battery mode, no enemy carries one of ours (Intercept's, as today),
//!   and it has a target and enough bots: at least `min_group`, or the defenders the team knows near the target.
//! - **What:** the enemy's container (a Raid), the one with the fewest known defenders within 25 m. A contested lying
//!   battery (a Collect) joins with the battery weighing's net count.
//! - **Who:** one more bot than it needs, the nearest to the target, but one bot by our own container when the team has
//!   four or more (one persistent defender; UT2003's squads, CTF Arena's template). The taker is the nearest with health over
//!   `taker_health` and runs the take; the escorts follow it (CoverHuman aimed at the taker, UT's formation centre). A taker
//!   who dies is replaced by the escort nearest the target.
//! - **Gather, then commit:** the escorts meet on the taker; the group commits once `gather_m` of it holds as many as the
//!   defenders known near the target, or after `gather_s`. Gathering and committed, the members' plays are held against
//!   their band; only the alarm band (a theft, a mate's death to trade, a carrier) takes them.
//! - **Home:** the taker carries: the push brings it home (today's Escort plays on our carrier).
//! - **End:** the battery home, the carrier lost, the group outnumbered by the known defenders for `short_s`, no taker left,
//!   the team level again, or the target gone (the container empty). Then it rests `rest_s`, for the respawns: the next
//!   push does not trickle in one by one (owner-16 §2).
//!
//! The members' attack gates count the guard net of the push's own (`Ctx::push_net`, P1).

use mapdata::Vec3;

use super::plays::{PlayKind, TargetRef};
use super::tuning::PushRule;
use super::view::Place;

/// What a push takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// The enemy's container `id`, at `at`: a Raid.
    Container { id: u8, at: Vec3 },
}

impl Target {
    pub fn at(self) -> Vec3 {
        match self {
            Target::Container { at, .. } => at,
        }
    }

    /// The taker's play on it.
    pub fn play(self) -> (PlayKind, TargetRef) {
        match self {
            Target::Container { id, .. } => (PlayKind::Raid, TargetRef::Container(id)),
        }
    }
}

/// One live bot of the team as the push reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    pub place: Place,
    pub at: Vec3,
    pub health: f32,
    pub carrying: bool,
}

/// What the commander knows at its tick, for the push.
pub struct Picture<'a> {
    pub now: u32,
    /// The team is behind by the rule ([`PushRule::behind`]) in a battery mode.
    pub behind: bool,
    /// An enemy carries one of our batteries: Intercept's, the push waits.
    pub enemy_carrier: bool,
    /// The team's live bots.
    pub bots: &'a [Member],
    /// Our container.
    pub home: Vec3,
    /// What a push may take, each with the enemies the team knows within 25 m of it.
    pub targets: &'a [(Target, usize)],
}

impl Picture<'_> {
    fn bot(&self, place: Place) -> Option<&Member> {
        self.bots.iter().find(|b| b.place == place)
    }

    fn defenders(&self, target: Target) -> Option<usize> {
        self.targets.iter().find(|(t, _)| *t == target).map(|&(_, n)| n)
    }
}

/// The team's push.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Push {
    #[default]
    Idle,
    /// After a push, until the respawns are back.
    Resting { until: u32 },
    /// The escorts meet on the taker.
    Gather { target: Target, taker: Place, escorts: Vec<Place>, since: u32 },
    /// The group goes in, held; `short_since`: since when the known defenders outnumber it.
    Commit { target: Target, taker: Place, escorts: Vec<Place>, short_since: Option<u32> },
    /// The taker carries: the group brings it home.
    Home { carrier: Place, escorts: Vec<Place> },
}

/// A bot's part in the push.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    /// It runs the take on the target.
    Taker(Target),
    /// It follows the taker.
    Escort { taker: Place },
    /// It escorts our carrier home (Escort's plays).
    Home { carrier: Place },
}

/// Why a push ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// The carrier came home.
    Home,
    /// The carrier died or dropped it away from home.
    Lost,
    /// The known defenders outnumbered the group for `short_s`.
    Outnumbered,
    /// No live member could take.
    NoTaker,
    /// The team is no longer behind.
    Level,
    /// The target is gone (their container empty).
    TargetGone,
}

/// What a tick of the push did (its log line).
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Started { target: Target, taker: Place, group: usize, defenders: usize },
    Committed { near: usize, defenders: usize, waited: bool },
    Took { carrier: Place },
    Ended(End),
}

impl Event {
    /// [fix-team push, item 3] The event's log line (the team's note, after "team T: "): the target, the group, the defenders
    /// the team knows within 25 m of the target, the net count, the outcome. `before` is the push the tick began with, `after`
    /// the one it left, `p` its picture.
    pub fn line(&self, before: &Push, after: &Push, p: &Picture) -> String {
        let target = |t: Target| match t {
            Target::Container { id, .. } => format!("their container {id}"),
        };
        let net = |defenders: usize, group: usize| defenders.saturating_sub(group);
        match self {
            Event::Started { target: t, taker, group, defenders } => format!(
                "push on {} started: taker {taker}, group {group} {:?}, {defenders} defenders known, net {}",
                target(*t),
                after.members(),
                net(*defenders, *group)
            ),
            Event::Committed { near, defenders, waited } => format!(
                "push on {} committed: {near} on the taker, {defenders} defenders known, net {}, {}",
                before.target().map_or_else(|| "?".to_string(), target),
                net(*defenders, *near),
                if *waited { "after the gather time" } else { "gathered" }
            ),
            Event::Took { carrier } => {
                let group = before.members();
                format!("push: {carrier} took a battery, home with {:?}", group.iter().filter(|&&m| m != *carrier).collect::<Vec<_>>())
            }
            Event::Ended(why) => {
                let members = before.members();
                let alive = members.iter().filter(|&&m| p.bot(m).is_some()).count();
                let defenders = before.target().and_then(|t| p.defenders(t));
                format!(
                    "push{} ended: {why:?} (group {:?}, {alive} alive{})",
                    before.target().map_or_else(String::new, |t| format!(" on {}", target(t))),
                    members,
                    defenders.map_or_else(String::new, |d| format!(", {d} defenders known, net {}", net(d, alive)))
                )
            }
        }
    }
}

impl Push {
    /// `place`'s part in it, if any.
    pub fn role(&self, place: Place) -> Option<Role> {
        match self {
            Push::Gather { target, taker, escorts, .. } | Push::Commit { target, taker, escorts, .. } => {
                if *taker == place {
                    Some(Role::Taker(*target))
                } else {
                    escorts.contains(&place).then_some(Role::Escort { taker: *taker })
                }
            }
            Push::Home { carrier, escorts } => escorts.contains(&place).then_some(Role::Home { carrier: *carrier }),
            Push::Idle | Push::Resting { .. } => None,
        }
    }

    /// The push's members (taker or carrier first), none when idle or resting.
    pub fn members(&self) -> Vec<Place> {
        match self {
            Push::Gather { taker, escorts, .. } | Push::Commit { taker, escorts, .. } => {
                std::iter::once(*taker).chain(escorts.iter().copied()).collect()
            }
            Push::Home { carrier, escorts } => std::iter::once(*carrier).chain(escorts.iter().copied()).collect(),
            Push::Idle | Push::Resting { .. } => Vec::new(),
        }
    }

    /// The target it goes for, gathering or committed.
    pub fn target(&self) -> Option<Target> {
        match self {
            Push::Gather { target, .. } | Push::Commit { target, .. } => Some(*target),
            _ => None,
        }
    }

    /// The push at the commander's tick `p`, and what it did: the one transition of the state.
    pub fn next(self, p: &Picture, rule: &PushRule) -> (Push, Option<Event>) {
        let end = |why| (Push::Resting { until: p.now.saturating_add(ms(rule.rest_s)) }, Some(Event::Ended(why)));
        match self {
            Push::Idle => start(p, rule),
            Push::Resting { until } if p.now >= until => start(p, rule),
            resting @ Push::Resting { .. } => (resting, None),
            Push::Gather { target, taker, escorts, since } => match group(target, taker, escorts, p, rule) {
                Err(done) => done,
                Ok(Group { taker, escorts }) => {
                    let defenders = p.defenders(target).unwrap_or(0);
                    let near = near_taker(taker, &escorts, p, rule.gather_m);
                    let met = near >= defenders.max(1);
                    let waited = p.now >= since.saturating_add(ms(rule.gather_s));
                    if met || waited {
                        let event = Event::Committed { near: near + 1, defenders, waited: !met };
                        (Push::Commit { target, taker, escorts, short_since: None }, Some(event))
                    } else {
                        (Push::Gather { target, taker, escorts, since }, None)
                    }
                }
            },
            Push::Commit { target, taker, escorts, short_since } => match group(target, taker, escorts, p, rule) {
                Err(done) => done,
                Ok(Group { taker, escorts }) => {
                    let defenders = p.defenders(target).unwrap_or(0);
                    let strength = 1 + near_taker(taker, &escorts, p, rule.group_m);
                    match (strength < defenders, short_since) {
                        (true, Some(since)) if p.now >= since.saturating_add(ms(rule.short_s)) => end(End::Outnumbered),
                        (true, since) => {
                            (Push::Commit { target, taker, escorts, short_since: Some(since.unwrap_or(p.now)) }, None)
                        }
                        (false, _) => (Push::Commit { target, taker, escorts, short_since: None }, None),
                    }
                }
            },
            Push::Home { carrier, mut escorts } => {
                escorts.retain(|&e| p.bot(e).is_some());
                match p.bot(carrier) {
                    Some(c) if c.carrying => (Push::Home { carrier, escorts }, None),
                    Some(c) if c.at.distance_xz(p.home) <= rule.home_m => end(End::Home),
                    _ => end(End::Lost),
                }
            }
        }
    }
}

fn ms(s: f32) -> u32 {
    (s * 1000.0) as u32
}

/// The escorts within `m` of the taker.
fn near_taker(taker: Place, escorts: &[Place], p: &Picture, m: f32) -> usize {
    let Some(t) = p.bot(taker) else {
        return 0;
    };
    escorts.iter().filter_map(|&e| p.bot(e)).filter(|e| e.at.distance_xz(t.at) <= m).count()
}

/// A gathering or committed group after this tick's deaths.
struct Group {
    taker: Place,
    escorts: Vec<Place>,
}

/// The group of a push on `target` this tick: the dead escorts out, a dead taker replaced by the escort nearest the target
/// with health over `taker_health`; or the push's end (the team level, the target gone, no taker), or its turn home (a
/// member carries).
fn group(target: Target, taker: Place, mut escorts: Vec<Place>, p: &Picture, rule: &PushRule) -> Result<Group, (Push, Option<Event>)> {
    let end = |why| (Push::Resting { until: p.now.saturating_add(ms(rule.rest_s)) }, Some(Event::Ended(why)));
    if !p.behind {
        return Err(end(End::Level));
    }
    if p.defenders(target).is_none() {
        return Err(end(End::TargetGone));
    }
    escorts.retain(|&e| p.bot(e).is_some());
    let members = std::iter::once(taker).chain(escorts.iter().copied());
    if let Some(carrier) = members.filter_map(|m| p.bot(m)).find(|m| m.carrying).map(|m| m.place) {
        escorts.retain(|&e| e != carrier);
        if carrier != taker && p.bot(taker).is_some() {
            escorts.push(taker);
        }
        return Err((Push::Home { carrier, escorts }, Some(Event::Took { carrier })));
    }
    if p.bot(taker).is_some() {
        return Ok(Group { taker, escorts });
    }
    let next = escorts
        .iter()
        .filter_map(|&e| p.bot(e))
        .filter(|e| e.health > rule.taker_health)
        .min_by(|a, b| a.at.distance_xz(target.at()).total_cmp(&b.at.distance_xz(target.at())))
        .map(|e| e.place);
    match next {
        Some(taker) => {
            escorts.retain(|&e| e != taker);
            Ok(Group { taker, escorts })
        }
        None => Err(end(End::NoTaker)),
    }
}

/// A push from idle: behind, no enemy carrier, a target, and the bots for it.
fn start(p: &Picture, rule: &PushRule) -> (Push, Option<Event>) {
    if !p.behind || p.enemy_carrier {
        return (Push::Idle, None);
    }
    let mut free: Vec<&Member> = p.bots.iter().filter(|b| !b.carrying).collect();
    if free.len() >= 4
        && let Some(guard) = (0..free.len()).min_by(|&a, &b| free[a].at.distance_xz(p.home).total_cmp(&free[b].at.distance_xz(p.home)))
    {
        free.remove(guard);
    }
    if free.is_empty() {
        return (Push::Idle, None);
    }
    let centre = free.iter().fold(Vec3::new(0.0, 0.0, 0.0), |s, b| s + b.at) * (1.0 / free.len() as f32);
    let Some(&(target, defenders)) = p.targets.iter().min_by(|a, b| {
        a.1.cmp(&b.1).then(a.0.at().distance_xz(centre).total_cmp(&b.0.at().distance_xz(centre)))
    }) else {
        return (Push::Idle, None);
    };
    let need = usize::from(rule.min_group).max(defenders);
    if free.len() < need {
        return (Push::Idle, None);
    }
    free.sort_by(|a, b| a.at.distance_xz(target.at()).total_cmp(&b.at.distance_xz(target.at())));
    free.truncate(need + 1);
    let Some(taker) = free.iter().find(|b| b.health > rule.taker_health).map(|b| b.place) else {
        return (Push::Idle, None);
    };
    let escorts: Vec<Place> = free.iter().map(|b| b.place).filter(|&b| b != taker).collect();
    let event = Event::Started { target, taker, group: escorts.len() + 1, defenders };
    (Push::Gather { target, taker, escorts, since: p.now }, Some(event))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    const THEIRS: Target = Target::Container { id: 1, at: Vec3::new(100.0, 0.0, 0.0) };

    fn bot(place: Place, x: f32) -> Member {
        Member { place, at: Vec3::new(x, 0.0, 0.0), health: 1.0, carrying: false }
    }

    fn picture<'a>(now: u32, bots: &'a [Member], targets: &'a [(Target, usize)]) -> Picture<'a> {
        Picture { now, behind: true, enemy_carrier: false, bots, home: HOME, targets }
    }

    /// Behind with five bots and two defenders known by their container: one stays home, the four nearest the target push
    /// (two needed but `min_group` 3, one more), the nearest healthy one takes; they gather, then commit once two are on it.
    #[test]
    fn a_team_behind_pushes_with_a_taker_and_escorts_and_commits_once_gathered() {
        let rule = PushRule::default();
        let bots = [bot(1, 5.0), bot(2, 40.0), bot(3, 50.0), bot(4, 60.0), bot(5, 70.0)];
        let targets = [(THEIRS, 2)];
        let (push, event) = Push::Idle.next(&picture(0, &bots, &targets), &rule);
        let Push::Gather { taker, ref escorts, .. } = push else { panic!("{push:?}") };
        assert_eq!(taker, 5, "the nearest the target takes");
        assert_eq!(escorts.len(), 3);
        assert!(!push.members().contains(&1), "the one by our container stays");
        assert!(matches!(event, Some(Event::Started { group: 4, defenders: 2, .. })), "{event:?}");
        // Not met yet (the escorts are 10-30 m off the taker): it gathers.
        let (push, event) = push.next(&picture(1_000, &bots, &targets), &rule);
        assert!(matches!(push, Push::Gather { .. }) && event.is_none(), "{push:?}");
        // Two escorts on the taker: it commits.
        let met = [bot(1, 5.0), bot(2, 40.0), bot(3, 65.0), bot(4, 66.0), bot(5, 70.0)];
        let (push, event) = push.next(&picture(2_000, &met, &targets), &rule);
        assert!(matches!(push, Push::Commit { .. }), "{push:?}");
        assert_eq!(event, Some(Event::Committed { near: 3, defenders: 2, waited: false }));
    }

    /// Committed: outnumbered for `short_s` it ends and rests; a dead taker is replaced by the escort nearest the target.
    #[test]
    fn a_committed_push_replaces_its_taker_and_ends_outnumbered() {
        let rule = PushRule::default();
        let push = Push::Commit { target: THEIRS, taker: 5, escorts: vec![3, 4], short_since: None };
        // The taker dead: escort 4 (nearer the target) takes.
        let bots = [bot(3, 60.0), bot(4, 66.0)];
        let (push, _) = push.next(&picture(0, &bots, &[(THEIRS, 1)]), &rule);
        assert_eq!(push.role(4), Some(Role::Taker(THEIRS)));
        assert_eq!(push.role(3), Some(Role::Escort { taker: 4 }));
        // Five defenders against two: short from now, ended `short_s` later.
        let (push, event) = push.next(&picture(1_000, &bots, &[(THEIRS, 5)]), &rule);
        assert!(matches!(push, Push::Commit { short_since: Some(1_000), .. }) && event.is_none(), "{push:?}");
        let later = 1_000 + (rule.short_s * 1000.0) as u32;
        let (push, event) = push.next(&picture(later, &bots, &[(THEIRS, 5)]), &rule);
        assert_eq!(event, Some(Event::Ended(End::Outnumbered)));
        assert!(matches!(push, Push::Resting { .. }));
    }

    /// The taker picks one up: the group brings it home; at home the push ends, and rests.
    #[test]
    fn a_push_brings_its_battery_home() {
        let rule = PushRule::default();
        let push = Push::Commit { target: THEIRS, taker: 5, escorts: vec![4], short_since: None };
        let mut bots = [bot(4, 90.0), bot(5, 100.0)];
        bots[1].carrying = true;
        let (push, event) = push.next(&picture(0, &bots, &[(THEIRS, 0)]), &rule);
        assert_eq!(event, Some(Event::Took { carrier: 5 }));
        assert_eq!(push.role(4), Some(Role::Home { carrier: 5 }));
        let home = [bot(4, 8.0), Member { at: Vec3::new(2.0, 0.0, 0.0), ..bot(5, 0.0) }];
        let (push, event) = push.next(&picture(30_000, &home, &[(THEIRS, 0)]), &rule);
        assert_eq!(event, Some(Event::Ended(End::Home)));
        assert!(matches!(push, Push::Resting { .. }));
    }

    /// [item 3] The push's lines name the target, the group, the known defenders, the net count and the outcome.
    #[test]
    fn a_push_logs_its_target_group_defenders_net_and_outcome() {
        let rule = PushRule::default();
        let bots = [bot(1, 5.0), bot(2, 40.0), bot(3, 50.0), bot(4, 60.0), bot(5, 70.0)];
        let targets = [(THEIRS, 2)];
        let p = picture(0, &bots, &targets);
        let (push, event) = Push::Idle.next(&p, &rule);
        let line = event.expect("started").line(&Push::Idle, &push, &p);
        assert_eq!(line, "push on their container 1 started: taker 5, group 4 [5, 4, 3, 2], 2 defenders known, net 0");
        let gone = [bot(1, 5.0)];
        let p = picture(1_000, &gone, &targets);
        let (after, event) = push.clone().next(&p, &rule);
        let line = event.expect("ended").line(&push, &after, &p);
        assert_eq!(line, "push on their container 1 ended: NoTaker (group [5, 4, 3, 2], 0 alive, 2 defenders known, net 2)");
    }

    /// No push level on points, with an enemy carrying ours (Intercept's), or with too few bots.
    #[test]
    fn no_push_level_with_a_thief_out_or_too_few() {
        let rule = PushRule::default();
        let bots = [bot(2, 40.0), bot(3, 50.0), bot(4, 60.0)];
        let targets = [(THEIRS, 1)];
        let level = Picture { behind: false, ..picture(0, &bots, &targets) };
        assert_eq!(Push::Idle.next(&level, &rule).0, Push::Idle);
        let thief = Picture { enemy_carrier: true, ..picture(0, &bots, &targets) };
        assert_eq!(Push::Idle.next(&thief, &rule).0, Push::Idle);
        let two = [bot(3, 50.0), bot(4, 60.0)];
        assert_eq!(Push::Idle.next(&picture(0, &two, &targets), &rule).0, Push::Idle);
    }
}
