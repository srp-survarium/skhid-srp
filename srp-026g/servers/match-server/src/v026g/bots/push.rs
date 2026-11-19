//! [fix-team push `mods.push`] A team behind on points pushes as one (`owner-final3/PUSH-DESIGN.md` §3; the owner's local
//! matches of 2026-11-19: "pushing alone in battery matches when they are behind is not enough"). A push is one team-level
//! state with a target and a group, not a mood that raises scores (P5: nothing held a push to its objective):
//!
//! - **When:** the team is `behind_by` points behind in a battery mode, no enemy carries one of ours (Intercept's, as today),
//!   and it has a target and enough bots: at least `min_group`, or the defenders the team knows near the target.
//! - **What:** the enemy's container (a Raid) or a contested lying battery (a Collect: one with a defender known within 25 m),
//!   the one with the fewest known defenders, then the nearest. A carried one is Intercept's, as today.
//! - **Who:** one more bot than it needs, the nearest to the target, but one bot by our own container when the team has
//!   four or more (one persistent defender; UT2003's squads, CTF Arena's template). The taker is the nearest with health over
//!   `taker_health` and runs the take; the escorts follow it (CoverHuman aimed at the taker, UT's formation centre). A taker
//!   who dies is replaced by the escort nearest the target.
//! - **Gather, then commit:** the escorts meet on the taker; the group commits once `gather_m` of it holds as many as the
//!   defenders known near the target, or after `gather_s`. Gathering and committed, the members' plays are held against
//!   their band; only the alarm band (a theft, a mate's death to trade, a carrier) takes them.
//! - **A second taker** (`second`): committed on a target that holds another battery (their container two or more, or a
//!   second lying battery within 25 m), one escort takes that one too when its chance of getting away ([`Escape::chance`]:
//!   the escorts left against the enemies by the target, those who can cut its way home off and the known shooters' lanes on
//!   it) is at least `escape_min`. The two carriers go home under the escorts left.
//! - **Home:** a taker carries: the push brings it home (today's Escort plays on our carrier), the other taker still taking.
//! - **End:** the batteries home, the carriers lost, the group outnumbered by the known defenders for `short_s`, no taker left,
//!   or the target gone (the container empty). Being level again does not end a push (the owner, 2026-11-19); starting one
//!   still needs the team behind. Then it rests `rest_s`, for the respawns: the next push does not trickle in one by one
//!   (owner-16 §2).
//!
//! The members' attack gates count the guard net of the push's own (`Ctx::push_net`, P1).

use mapdata::Vec3;

use super::plays::{ENEMIES_NEAR_M, PlayKind, TargetRef};
use super::tuning::{PushRule, SecondTaker};
use super::view::Place;

/// What a push takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// The enemy's container `id`, at `at`, holding `batteries`: a Raid.
    Container { id: u8, at: Vec3, batteries: usize },
    /// Battery `index` lying at `at` as the team knows it, defended: a Collect.
    Battery { index: usize, at: Vec3 },
}

impl Target {
    pub fn at(self) -> Vec3 {
        match self {
            Target::Container { at, .. } | Target::Battery { at, .. } => at,
        }
    }

    /// The taker's play on it.
    pub fn play(self) -> (PlayKind, TargetRef) {
        match self {
            Target::Container { id, .. } => (PlayKind::Raid, TargetRef::Container(id)),
            Target::Battery { index, .. } => (PlayKind::Collect, TargetRef::Battery(index)),
        }
    }

    /// Whether `other` is the same container or battery (where the team knows a battery lies moves with what it sees).
    pub fn same(self, other: Target) -> bool {
        self.play().1 == other.play().1
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

/// [second taker] What stands between a carrier at the push's target and our container: the inputs of [`Escape::chance`],
/// each from the world, the team's knowledge or the game data ([`Escape::of`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Escape {
    /// The carrier's way home, metres (the side's route, else the straight way times Intercept's `path_factor`).
    pub way_m: f32,
    /// Known enemies within 25 m of the target ([`ENEMIES_NEAR_M`], the push's defenders).
    pub near: usize,
    /// The other known enemies within 25 m of a point of that way they can be on no later than the carrier ([`cuts_off`]).
    pub cut_offs: usize,
    /// The team's held threat lines (its threat map, `threat::Picture::held`) whose lanes cover a point of the way, their
    /// shooters not among the cut-offs: a known shooter who need not come to the way to stop it.
    pub lanes: usize,
}

impl Escape {
    /// The escape of a carrier walking `way` (`(metres along it, point)`, its first point the target, its last our container,
    /// `way_m` long) at `carrier_mps`, from the known enemies `enemies` (each `(place, at)`: those within 25 m of the target,
    /// the others who cut the way off sprinting at `hunter_mps`), and the shooters of the held lines that cover a point of it
    /// (`covering`); each man counted once.
    ///
    /// The speeds are the game data's (PROVEN): a carrier does not sprint (the battery's hands refuse it, 0x2DBF9B ->
    /// 0x397C20; Intercept's `carrier_speed`), so it walks at the body's forward walk, 3.5 m/s; a man after it sprints,
    /// 4.9587 m/s (`kin::Kinematics::default()`, F5 on traced steps).
    pub fn of(
        way: &[(f32, Vec3)],
        way_m: f32,
        enemies: &[(Place, Vec3)],
        covering: &[Place],
        carrier_mps: f32,
        hunter_mps: f32,
    ) -> Escape {
        let target = way.first().map(|&(_, q)| q);
        let by_target = |at: Vec3| target.is_some_and(|t| at.distance_xz(t) < ENEMIES_NEAR_M);
        let near: Vec<Place> = enemies.iter().filter(|&&(_, at)| by_target(at)).map(|&(place, _)| place).collect();
        let cut: Vec<Place> = enemies
            .iter()
            .filter(|&&(place, at)| !near.contains(&place) && cuts_off(at, way, carrier_mps, hunter_mps))
            .map(|&(place, _)| place)
            .collect();
        let mut lanes: Vec<Place> =
            covering.iter().copied().filter(|shooter| !near.contains(shooter) && !cut.contains(shooter)).collect();
        lanes.sort_unstable();
        lanes.dedup();
        Escape { way_m, near: near.len(), cut_offs: cut.len(), lanes: lanes.len() }
    }

    /// The chance a carrier gets away with `cover` escorts by it: the escorts' share of all who meet on its way,
    /// `cover / (cover + near + cut_offs + lanes)`, 1 when nobody can stop it. Each who can stop it counts one, each escort one: the
    /// push's own rule of numbers (it commits when its escorts on the taker match the defenders, it ends outnumbered).
    ///
    /// Measured on the 51 carries of the owner's 3 matches of 2026-11-19 (recordings, true positions, the way straight times
    /// 1.25): of the enemies alive at the pick-up, those within 25 m of it came within 20 m of the carrier in 25 of 30, the
    /// cut-offs in 6 of 21, all others in 15 of 323; carries this scored 0.66 or more came home 26 of 36, under it 5 of 15
    /// (under 0.5: 1 of 8). `wsl-out/push/st_carries.py`.
    pub fn chance(&self, cover: usize) -> f32 {
        let against = self.near + self.cut_offs + self.lanes;
        if against == 0 {
            return 1.0;
        }
        cover as f32 / (cover + against) as f32
    }
}

/// Whether a man at `at` is within 25 m ([`ENEMIES_NEAR_M`], the ring a target's defenders are counted in) of a point of `way`
/// he can be at, sprinting at `hunter_mps`, no later than a carrier that walks it from its start at `carrier_mps`. His walk is
/// the straight line (a lower bound of his). The ring is measured: anyone who could sprint to some point first (the test
/// without it) took in 142 enemies of the owner's carries of whom 13 came at the carrier; within 25 m of the way, 6 of 21
/// ([`Escape::chance`]).
pub fn cuts_off(at: Vec3, way: &[(f32, Vec3)], carrier_mps: f32, hunter_mps: f32) -> bool {
    way.iter().any(|&(s, q)| {
        let d = at.distance_xz(q);
        d < ENEMIES_NEAR_M && d / hunter_mps.max(0.1) <= s / carrier_mps.max(0.1)
    })
}

/// What the commander knows at its tick, for the push.
pub struct Picture<'a> {
    pub now: u32,
    /// The team is behind by the rule ([`PushRule::behind`]) in a battery mode: a push starts only then.
    pub behind: bool,
    /// An enemy carries one of our batteries: Intercept's, the push waits.
    pub enemy_carrier: bool,
    /// The team's live bots.
    pub bots: &'a [Member],
    /// Our container.
    pub home: Vec3,
    /// What a push may take, each with the enemies the team knows within 25 m of it.
    pub targets: &'a [(Target, usize)],
    /// [second taker] The escape from the push's target, when its tick may pick a second taker ([`Push::second_wanted`]).
    pub escape: Option<Escape>,
}

impl Picture<'_> {
    fn bot(&self, place: Place) -> Option<&Member> {
        self.bots.iter().find(|b| b.place == place)
    }

    fn defenders(&self, target: Target) -> Option<usize> {
        self.targets.iter().find(|(t, _)| t.same(target)).map(|&(_, n)| n)
    }

    /// `target` as this tick sees it (a container's batteries, where a battery lies), if it is still there.
    fn now_of(&self, target: Target) -> Option<Target> {
        self.targets.iter().find(|(t, _)| t.same(target)).map(|&(t, _)| t)
    }
}

/// [second taker] A second battery of `target` for a second taker: their container itself when it holds two or more, else the
/// lying battery nearest `target` within 25 m of it (the ring its defenders are counted in, [`ENEMIES_NEAR_M`]).
fn second_target(targets: &[(Target, usize)], target: Target) -> Option<Target> {
    let now = targets.iter().find(|(t, _)| t.same(target)).map(|&(t, _)| t)?;
    match now {
        Target::Container { batteries, .. } => (batteries >= 2).then_some(now),
        Target::Battery { at, .. } => targets
            .iter()
            .map(|&(t, _)| t)
            .filter(|t| matches!(t, Target::Battery { .. }) && !t.same(now) && t.at().distance_xz(at) < ENEMIES_NEAR_M)
            .min_by(|a, b| a.at().distance_xz(at).total_cmp(&b.at().distance_xz(at))),
    }
}

/// [second taker] A member taking a second battery of the push's target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Second {
    pub place: Place,
    pub target: Target,
}

/// [second taker] Bringing one home, the push's other taker.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Other {
    /// It carries one too: home with the first.
    Carrying(Place),
    /// It still takes its own.
    Taking(Second),
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
    /// The group goes in, held; `short_since`: since when the known defenders outnumber it; `second`: an escort taking a
    /// second battery of the target.
    Commit { target: Target, taker: Place, escorts: Vec<Place>, short_since: Option<u32>, second: Option<Second> },
    /// A taker carries: the group brings it home; `other`: the push's other taker, carrying or still taking.
    Home { carrier: Place, other: Option<Other>, escorts: Vec<Place> },
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
    /// The (last) carrier came home.
    Home,
    /// The (last) carrier died or dropped it away from home.
    Lost,
    /// The known defenders outnumbered the group for `short_s`.
    Outnumbered,
    /// No live member could take.
    NoTaker,
    /// The target is gone (their container empty).
    TargetGone,
}

/// [second taker] A second carrier's chance of getting away and what it was made of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chance {
    pub chance: f32,
    /// The escorts left by the taker (within `group_m`).
    pub cover: usize,
    pub escape: Escape,
}

/// What a tick of the push did (its log line).
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Started { target: Target, taker: Place, group: usize, defenders: usize },
    Committed { near: usize, defenders: usize, waited: bool },
    /// [second taker] An escort takes a second battery: its chance of getting away, what it was made of, and the bar `min` it
    /// met.
    Second { second: Second, chance: Chance, min: f32 },
    Took { carrier: Place },
    /// [second taker] One of two carriers let go, home or not; the push goes on with the other.
    Carried { carrier: Place, home: bool },
    Ended(End),
}

fn target_name(t: Target) -> String {
    match t {
        Target::Container { id, .. } => format!("their container {id}"),
        Target::Battery { index, .. } => format!("battery {index}"),
    }
}

impl Event {
    /// [fix-team push, item 3] The event's log line (the team's note, after "team T: "): the target, the group, the defenders
    /// the team knows within 25 m of the target, the net count, the outcome. `before` is the push the tick began with, `after`
    /// the one it left, `p` its picture.
    pub fn line(&self, before: &Push, after: &Push, p: &Picture) -> String {
        let net = |defenders: usize, group: usize| defenders.saturating_sub(group);
        match self {
            Event::Started { target: t, taker, group, defenders } => format!(
                "push on {} started: taker {taker}, group {group} {:?}, {defenders} defenders known, net {}",
                target_name(*t),
                after.members(),
                net(*defenders, *group)
            ),
            Event::Committed { near, defenders, waited } => format!(
                "push on {} committed: {near} on the taker, {defenders} defenders known, net {}, {}",
                before.target().map_or_else(|| "?".to_string(), target_name),
                net(*defenders, *near),
                if *waited { "after the gather time" } else { "gathered" }
            ),
            Event::Second { second, chance: c, min } => format!(
                "push on {}: second taker {} on {}, escape {:.2} (at least {min:.2}): cover {} against {} by the target, {} \
                 cut-offs and {} lanes on a {:.0} m way home",
                after.target().map_or_else(|| "?".to_string(), target_name),
                second.place,
                target_name(second.target),
                c.chance,
                c.cover,
                c.escape.near,
                c.escape.cut_offs,
                c.escape.lanes,
                c.escape.way_m
            ),
            Event::Took { carrier } => {
                let group = before.members();
                format!("push: {carrier} took a battery, home with {:?}", group.iter().filter(|&&m| m != *carrier).collect::<Vec<_>>())
            }
            Event::Carried { carrier, home } => {
                format!("push: {carrier} {}, the push goes on with {:?}", if *home { "brought one home" } else { "lost its battery" }, after.members())
            }
            Event::Ended(why) => {
                let members = before.members();
                let alive = members.iter().filter(|&&m| p.bot(m).is_some()).count();
                let defenders = before.target().and_then(|t| p.defenders(t));
                format!(
                    "push{} ended: {why:?} (group {:?}, {alive} alive{})",
                    before.target().map_or_else(String::new, |t| format!(" on {}", target_name(t))),
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
            Push::Gather { target, taker, escorts, .. } => {
                if *taker == place {
                    Some(Role::Taker(*target))
                } else {
                    escorts.contains(&place).then_some(Role::Escort { taker: *taker })
                }
            }
            Push::Commit { target, taker, escorts, second, .. } => {
                if *taker == place {
                    Some(Role::Taker(*target))
                } else if let Some(s) = second.filter(|s| s.place == place) {
                    Some(Role::Taker(s.target))
                } else {
                    escorts.contains(&place).then_some(Role::Escort { taker: *taker })
                }
            }
            Push::Home { carrier, other, escorts } => match other {
                Some(Other::Taking(s)) if s.place == place => Some(Role::Taker(s.target)),
                _ => escorts.contains(&place).then_some(Role::Home { carrier: *carrier }),
            },
            Push::Idle | Push::Resting { .. } => None,
        }
    }

    /// The push's members (taker or carrier first, then the other taker), none when idle or resting.
    pub fn members(&self) -> Vec<Place> {
        match self {
            Push::Gather { taker, escorts, .. } => std::iter::once(*taker).chain(escorts.iter().copied()).collect(),
            Push::Commit { taker, escorts, second, .. } => {
                std::iter::once(*taker).chain(second.map(|s| s.place)).chain(escorts.iter().copied()).collect()
            }
            Push::Home { carrier, other, escorts } => {
                let other = other.map(|o| match o {
                    Other::Carrying(c) => c,
                    Other::Taking(s) => s.place,
                });
                std::iter::once(*carrier).chain(other).chain(escorts.iter().copied()).collect()
            }
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

    /// Its takers still taking and their targets: the taker, a second taker, the other taker of a push bringing one home.
    pub fn takers(&self) -> Vec<(Place, Target)> {
        match self {
            Push::Gather { target, taker, .. } => vec![(*taker, *target)],
            Push::Commit { target, taker, second, .. } => {
                std::iter::once((*taker, *target)).chain(second.map(|s| (s.place, s.target))).collect()
            }
            Push::Home { other: Some(Other::Taking(s)), .. } => vec![(s.place, s.target)],
            _ => Vec::new(),
        }
    }

    /// [second taker] The target whose way home this tick's pick of a second taker reads ([`Picture::escape`]): a committed
    /// push with no second taker on a target that holds another battery; none otherwise (no way searched).
    pub fn second_wanted(&self, targets: &[(Target, usize)], rule: &PushRule) -> Option<Target> {
        match self {
            Push::Commit { target, second: None, escorts, .. } if rule.second.is_some() && !escorts.is_empty() => {
                second_target(targets, *target).map(|_| *target)
            }
            _ => None,
        }
    }

    /// The push at the commander's tick `p`, and what it did: the one transition of the state.
    pub fn next(self, p: &Picture, rule: &PushRule) -> (Push, Vec<Event>) {
        let end = |why| (Push::Resting { until: p.now.saturating_add(ms(rule.rest_s)) }, vec![Event::Ended(why)]);
        match self {
            Push::Idle => start(p, rule),
            Push::Resting { until } if p.now >= until => start(p, rule),
            resting @ Push::Resting { .. } => (resting, Vec::new()),
            Push::Gather { target, taker, escorts, since } => match group(target, taker, escorts, None, p, rule) {
                Err(done) => done,
                Ok(Group { target, taker, escorts, .. }) => {
                    let defenders = p.defenders(target).unwrap_or(0);
                    let near = near_taker(taker, escorts.iter().copied(), p, rule.gather_m);
                    let met = near >= defenders.max(1);
                    let waited = p.now >= since.saturating_add(ms(rule.gather_s));
                    if met || waited {
                        let event = Event::Committed { near: near + 1, defenders, waited: !met };
                        (Push::Commit { target, taker, escorts, short_since: None, second: None }, vec![event])
                    } else {
                        (Push::Gather { target, taker, escorts, since }, Vec::new())
                    }
                }
            },
            Push::Commit { target, taker, escorts, short_since, second } => {
                match group(target, taker, escorts, second, p, rule) {
                    Err(done) => done,
                    Ok(Group { target, taker, mut escorts, second }) => {
                        let defenders = p.defenders(target).unwrap_or(0);
                        let strength = 1 + near_taker(taker, escorts.iter().copied().chain(second.map(|s| s.place)), p, rule.group_m);
                        let short_since = match (strength < defenders, short_since) {
                            (true, Some(since)) if p.now >= since.saturating_add(ms(rule.short_s)) => return end(End::Outnumbered),
                            (true, since) => Some(since.unwrap_or(p.now)),
                            (false, _) => None,
                        };
                        let picked = match second {
                            None => pick_second(target, taker, &escorts, p, rule),
                            Some(_) => None,
                        };
                        match picked {
                            Some((second, event)) => {
                                escorts.retain(|&e| e != second.place);
                                (Push::Commit { target, taker, escorts, short_since, second: Some(second) }, vec![event])
                            }
                            None => (Push::Commit { target, taker, escorts, short_since, second }, Vec::new()),
                        }
                    }
                }
            }
            Push::Home { carrier, other, escorts } => home(carrier, other, escorts, p, rule),
        }
    }
}

fn ms(s: f32) -> u32 {
    (s * 1000.0) as u32
}

/// The members of `of` within `m` of the taker.
fn near_taker(taker: Place, of: impl Iterator<Item = Place>, p: &Picture, m: f32) -> usize {
    let Some(t) = p.bot(taker) else {
        return 0;
    };
    of.filter_map(|e| p.bot(e)).filter(|e| e.at.distance_xz(t.at) <= m).count()
}

/// A gathering or committed group after this tick's deaths.
struct Group {
    target: Target,
    taker: Place,
    escorts: Vec<Place>,
    second: Option<Second>,
}

/// The group of a push on `target` this tick: the dead out, a dead taker replaced by the escort nearest the target with health
/// over `taker_health` (else by the second taker, on its own battery); or the push's turn home (a member carries), or its end
/// (the target gone, no taker). A second taker whose battery is gone escorts again; the first's gone, the second's battery is
/// the push's target.
fn group(
    target: Target,
    taker: Place,
    mut escorts: Vec<Place>,
    second: Option<Second>,
    p: &Picture,
    rule: &PushRule,
) -> Result<Group, (Push, Vec<Event>)> {
    let end = |why| (Push::Resting { until: p.now.saturating_add(ms(rule.rest_s)) }, vec![Event::Ended(why)]);
    escorts.retain(|&e| p.bot(e).is_some());
    let mut second = second.filter(|s| p.bot(s.place).is_some());
    // A member that carries took it (a battery taken lies no more: before the target's gone).
    let members: Vec<Place> = std::iter::once(taker).chain(second.map(|s| s.place)).chain(escorts.iter().copied()).collect();
    let carriers: Vec<Place> = members.iter().copied().filter(|&m| p.bot(m).is_some_and(|b| b.carrying)).collect();
    if let Some(&carrier) = carriers.first() {
        // The other: a second carrier, else a live taker whose target is still there takes on.
        let other = carriers.get(1).map(|&c| Other::Carrying(c)).or_else(|| {
            std::iter::once((taker, target))
                .chain(second.map(|s| (s.place, s.target)))
                .filter(|&(t, _)| !carriers.contains(&t) && p.bot(t).is_some())
                .find_map(|(place, target)| p.now_of(target).map(|target| Other::Taking(Second { place, target })))
        });
        let in_other = other.map(|o| match o {
            Other::Carrying(c) => c,
            Other::Taking(s) => s.place,
        });
        let escorts = members.into_iter().filter(|&m| m != carrier && Some(m) != in_other).collect();
        let mut events = vec![Event::Took { carrier }];
        if let Some(Other::Carrying(c)) = other {
            events.push(Event::Took { carrier: c });
        }
        return Err((Push::Home { carrier, other, escorts }, events));
    }
    // The second's battery gone: it escorts again.
    if let Some(s) = second.filter(|s| p.now_of(s.target).is_none()) {
        escorts.push(s.place);
        second = None;
    }
    let (target, taker, second) = match (p.now_of(target), second) {
        (Some(now), second) => (now, taker, second),
        // The first's gone, the second's still there: the push's target, the first taker its escort.
        (None, Some(s)) => {
            if p.bot(taker).is_some() {
                escorts.push(taker);
            }
            (s.target, s.place, None)
        }
        (None, None) => return Err(end(End::TargetGone)),
    };
    if p.bot(taker).is_some() {
        return Ok(Group { target, taker, escorts, second });
    }
    let next = escorts
        .iter()
        .filter_map(|&e| p.bot(e))
        .filter(|e| e.health > rule.taker_health)
        .min_by(|a, b| a.at.distance_xz(target.at()).total_cmp(&b.at.distance_xz(target.at())))
        .map(|e| e.place);
    match (next, second) {
        (Some(taker), second) => {
            escorts.retain(|&e| e != taker);
            Ok(Group { target, taker, escorts, second })
        }
        (None, Some(s)) => Ok(Group { target: s.target, taker: s.place, escorts, second: None }),
        (None, None) => Err(end(End::NoTaker)),
    }
}

/// [second taker] The escort that takes a second battery of `target` this tick, if any: the one nearest that battery with health
/// over `taker_health`, when its chance of getting away with the escorts left by the taker ([`Escape::chance`]) is at least
/// `escape_min`.
fn pick_second(target: Target, taker: Place, escorts: &[Place], p: &Picture, rule: &PushRule) -> Option<(Second, Event)> {
    let second: &SecondTaker = rule.second.as_ref()?;
    let aim = second_target(p.targets, target)?;
    let place = escorts
        .iter()
        .filter_map(|&e| p.bot(e))
        .filter(|e| !e.carrying && e.health > rule.taker_health)
        .min_by(|a, b| a.at.distance_xz(aim.at()).total_cmp(&b.at.distance_xz(aim.at())))?
        .place;
    let cover = near_taker(taker, escorts.iter().copied().filter(|&e| e != place), p, rule.group_m);
    let escape = p.escape?;
    let chance = Chance { chance: escape.chance(cover), cover, escape };
    if chance.chance < second.escape_min {
        return None;
    }
    let pick = Second { place, target: aim };
    Some((pick, Event::Second { second: pick, chance, min: second.escape_min }))
}

/// Bringing it home: the other taker's take or its own carry, each carrier that lets go home or lost; once none carries the
/// push ends (`Home` when the last came home), or goes back to the other taker's take.
fn home(carrier: Place, other: Option<Other>, mut escorts: Vec<Place>, p: &Picture, rule: &PushRule) -> (Push, Vec<Event>) {
    let carrying = |c: Place| p.bot(c).is_some_and(|b| b.carrying);
    let at_home = |c: Place| p.bot(c).is_some_and(|b| b.at.distance_xz(p.home) <= rule.home_m);
    let mut events = Vec::new();
    escorts.retain(|&e| p.bot(e).is_some());
    let other = match other {
        Some(Other::Taking(s)) if carrying(s.place) => {
            events.push(Event::Took { carrier: s.place });
            Some(Other::Carrying(s.place))
        }
        Some(Other::Taking(s)) if p.bot(s.place).is_some() && p.now_of(s.target).is_some() => Some(Other::Taking(s)),
        Some(Other::Taking(s)) => {
            if p.bot(s.place).is_some() {
                escorts.push(s.place);
            }
            None
        }
        other => other,
    };
    let taking = match other {
        Some(Other::Taking(s)) => Some(s),
        _ => None,
    };
    let (mut carriers, mut done) = (Vec::new(), Vec::new());
    for c in std::iter::once(carrier).chain(match other {
        Some(Other::Carrying(c)) => Some(c),
        _ => None,
    }) {
        if carrying(c) {
            carriers.push(c);
        } else {
            done.push((c, at_home(c)));
        }
    }
    match (carriers.first().copied(), taking) {
        (Some(carrier), taking) => {
            events.extend(done.iter().map(|&(carrier, home)| Event::Carried { carrier, home }));
            let other = carriers.get(1).map(|&c| Other::Carrying(c)).or(taking.map(Other::Taking));
            (Push::Home { carrier, other, escorts }, events)
        }
        (None, Some(s)) => {
            events.extend(done.iter().map(|&(carrier, home)| Event::Carried { carrier, home }));
            (Push::Commit { target: s.target, taker: s.place, escorts, short_since: None, second: None }, events)
        }
        (None, None) => {
            if done.len() > 1 {
                events.extend(done.iter().map(|&(carrier, home)| Event::Carried { carrier, home }));
            }
            let why = if done.last().is_some_and(|&(_, home)| home) { End::Home } else { End::Lost };
            events.push(Event::Ended(why));
            (Push::Resting { until: p.now.saturating_add(ms(rule.rest_s)) }, events)
        }
    }
}

/// A push from idle: behind, no enemy carrier, a target, and the bots for it.
fn start(p: &Picture, rule: &PushRule) -> (Push, Vec<Event>) {
    if !p.behind || p.enemy_carrier {
        return (Push::Idle, Vec::new());
    }
    let mut free: Vec<&Member> = p.bots.iter().filter(|b| !b.carrying).collect();
    if free.len() >= 4
        && let Some(guard) = (0..free.len()).min_by(|&a, &b| free[a].at.distance_xz(p.home).total_cmp(&free[b].at.distance_xz(p.home)))
    {
        free.remove(guard);
    }
    if free.is_empty() {
        return (Push::Idle, Vec::new());
    }
    let centre = free.iter().fold(Vec3::new(0.0, 0.0, 0.0), |s, b| s + b.at) * (1.0 / free.len() as f32);
    // A lying battery nobody is known to defend is plain Collect's, not a push's.
    let contested = |&&(t, defenders): &&(Target, usize)| !matches!(t, Target::Battery { .. }) || defenders > 0;
    let Some(&(target, defenders)) = p.targets.iter().filter(contested).min_by(|a, b| {
        a.1.cmp(&b.1).then(a.0.at().distance_xz(centre).total_cmp(&b.0.at().distance_xz(centre)))
    }) else {
        return (Push::Idle, Vec::new());
    };
    let need = usize::from(rule.min_group).max(defenders);
    if free.len() < need {
        return (Push::Idle, Vec::new());
    }
    free.sort_by(|a, b| a.at.distance_xz(target.at()).total_cmp(&b.at.distance_xz(target.at())));
    free.truncate(need + 1);
    let Some(taker) = free.iter().find(|b| b.health > rule.taker_health).map(|b| b.place) else {
        return (Push::Idle, Vec::new());
    };
    let escorts: Vec<Place> = free.iter().map(|b| b.place).filter(|&b| b != taker).collect();
    let event = Event::Started { target, taker, group: escorts.len() + 1, defenders };
    (Push::Gather { target, taker, escorts, since: p.now }, vec![event])
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    const THEIRS: Target = Target::Container { id: 1, at: Vec3::new(100.0, 0.0, 0.0), batteries: 1 };

    fn bot(place: Place, x: f32) -> Member {
        Member { place, at: Vec3::new(x, 0.0, 0.0), health: 1.0, carrying: false }
    }

    fn picture<'a>(now: u32, bots: &'a [Member], targets: &'a [(Target, usize)]) -> Picture<'a> {
        Picture { now, behind: true, enemy_carrier: false, bots, home: HOME, targets, escape: None }
    }

    fn commit(target: Target, taker: Place, escorts: Vec<Place>) -> Push {
        Push::Commit { target, taker, escorts, short_since: None, second: None }
    }

    /// Behind with five bots and two defenders known by their container: one stays home, the four nearest the target push
    /// (two needed but `min_group` 3, one more), the nearest healthy one takes; they gather, then commit once two are on it.
    #[test]
    fn a_team_behind_pushes_with_a_taker_and_escorts_and_commits_once_gathered() {
        let rule = PushRule::default();
        let bots = [bot(1, 5.0), bot(2, 40.0), bot(3, 50.0), bot(4, 60.0), bot(5, 70.0)];
        let targets = [(THEIRS, 2)];
        let (push, events) = Push::Idle.next(&picture(0, &bots, &targets), &rule);
        let Push::Gather { taker, ref escorts, .. } = push else { panic!("{push:?}") };
        assert_eq!(taker, 5, "the nearest the target takes");
        assert_eq!(escorts.len(), 3);
        assert!(!push.members().contains(&1), "the one by our container stays");
        assert!(matches!(events[..], [Event::Started { group: 4, defenders: 2, .. }]), "{events:?}");
        // Not met yet (the escorts are 10-30 m off the taker): it gathers.
        let (push, events) = push.next(&picture(1_000, &bots, &targets), &rule);
        assert!(matches!(push, Push::Gather { .. }) && events.is_empty(), "{push:?}");
        // Two escorts on the taker: it commits.
        let met = [bot(1, 5.0), bot(2, 40.0), bot(3, 65.0), bot(4, 66.0), bot(5, 70.0)];
        let (push, events) = push.next(&picture(2_000, &met, &targets), &rule);
        assert!(matches!(push, Push::Commit { .. }), "{push:?}");
        assert_eq!(events, vec![Event::Committed { near: 3, defenders: 2, waited: false }]);
    }

    /// Committed: outnumbered for `short_s` it ends and rests; a dead taker is replaced by the escort nearest the target.
    #[test]
    fn a_committed_push_replaces_its_taker_and_ends_outnumbered() {
        let rule = PushRule::default();
        let push = commit(THEIRS, 5, vec![3, 4]);
        // The taker dead: escort 4 (nearer the target) takes.
        let bots = [bot(3, 60.0), bot(4, 66.0)];
        let (push, _) = push.next(&picture(0, &bots, &[(THEIRS, 1)]), &rule);
        assert_eq!(push.role(4), Some(Role::Taker(THEIRS)));
        assert_eq!(push.role(3), Some(Role::Escort { taker: 4 }));
        // Five defenders against two: short from now, ended `short_s` later.
        let (push, events) = push.next(&picture(1_000, &bots, &[(THEIRS, 5)]), &rule);
        assert!(matches!(push, Push::Commit { short_since: Some(1_000), .. }) && events.is_empty(), "{push:?}");
        let later = 1_000 + (rule.short_s * 1000.0) as u32;
        let (push, events) = push.next(&picture(later, &bots, &[(THEIRS, 5)]), &rule);
        assert_eq!(events, vec![Event::Ended(End::Outnumbered)]);
        assert!(matches!(push, Push::Resting { .. }));
    }

    /// The taker picks one up: the group brings it home; at home the push ends, and rests.
    #[test]
    fn a_push_brings_its_battery_home() {
        let rule = PushRule::default();
        let push = commit(THEIRS, 5, vec![4]);
        let mut bots = [bot(4, 90.0), bot(5, 100.0)];
        bots[1].carrying = true;
        let (push, events) = push.next(&picture(0, &bots, &[(THEIRS, 0)]), &rule);
        assert_eq!(events, vec![Event::Took { carrier: 5 }]);
        assert_eq!(push.role(4), Some(Role::Home { carrier: 5 }));
        let home = [bot(4, 8.0), Member { at: Vec3::new(2.0, 0.0, 0.0), ..bot(5, 0.0) }];
        let (push, events) = push.next(&picture(30_000, &home, &[(THEIRS, 0)]), &rule);
        assert_eq!(events, vec![Event::Ended(End::Home)]);
        assert!(matches!(push, Push::Resting { .. }));
    }

    /// [item 3] The push's lines name the target, the group, the known defenders, the net count and the outcome.
    #[test]
    fn a_push_logs_its_target_group_defenders_net_and_outcome() {
        let rule = PushRule::default();
        let bots = [bot(1, 5.0), bot(2, 40.0), bot(3, 50.0), bot(4, 60.0), bot(5, 70.0)];
        let targets = [(THEIRS, 2)];
        let p = picture(0, &bots, &targets);
        let (push, events) = Push::Idle.next(&p, &rule);
        let line = events[0].line(&Push::Idle, &push, &p);
        assert_eq!(line, "push on their container 1 started: taker 5, group 4 [5, 4, 3, 2], 2 defenders known, net 0");
        let gone = [bot(1, 5.0)];
        let p = picture(1_000, &gone, &targets);
        let (after, events) = push.clone().next(&p, &rule);
        let line = events[0].line(&push, &after, &p);
        assert_eq!(line, "push on their container 1 ended: NoTaker (group [5, 4, 3, 2], 0 alive, 2 defenders known, net 2)");
    }

    /// A contested lying battery with fewer defenders than their container is the target, its taker on Collect; a battery
    /// nobody defends is not (plain Collect's). Once the taker carries it the push turns home, though it lies no more.
    #[test]
    fn a_push_takes_a_contested_battery_and_brings_it_home() {
        let rule = PushRule::default();
        let battery = Target::Battery { index: 3, at: Vec3::new(60.0, 0.0, 20.0) };
        let free = Target::Battery { index: 4, at: Vec3::new(30.0, 0.0, 0.0) };
        let bots = [bot(2, 40.0), bot(3, 50.0), bot(4, 55.0)];
        let targets = [(THEIRS, 3), (battery, 1), (free, 0)];
        let (push, _) = Push::Idle.next(&picture(0, &bots, &targets), &rule);
        assert_eq!(push.target(), Some(battery), "{push:?}");
        let Push::Gather { taker, .. } = push else { panic!("{push:?}") };
        assert_eq!(push.role(taker).map(|r| matches!(r, Role::Taker(t) if t.play().0 == PlayKind::Collect)), Some(true));
        // It is seen a metre off where it was thought: the same battery.
        let moved = [(THEIRS, 3), (Target::Battery { index: 3, at: Vec3::new(61.0, 0.0, 20.0) }, 1)];
        let (push, events) = push.next(&picture(1_000, &bots, &moved), &rule);
        assert!(matches!(push, Push::Gather { .. } | Push::Commit { .. }), "{push:?} {events:?}");
        // The taker picks it up: it lies no more, and the push turns home.
        let mut carrying = bots;
        carrying.iter_mut().find(|b| b.place == taker).expect("taker").carrying = true;
        let (push, events) = push.next(&picture(2_000, &carrying, &[(THEIRS, 3)]), &rule);
        assert_eq!(events, vec![Event::Took { carrier: taker }]);
        assert!(matches!(push, Push::Home { .. }), "{push:?}");
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

    /// A push under way goes on once the team is level again (no `Level` end): gathering and committed alike.
    #[test]
    fn a_push_goes_on_when_the_team_is_level_again() {
        let rule = PushRule::default();
        let bots = [bot(3, 95.0), bot(4, 96.0), bot(5, 100.0)];
        let targets = [(THEIRS, 1)];
        let level = Picture { behind: false, ..picture(0, &bots, &targets) };
        let gather = Push::Gather { target: THEIRS, taker: 5, escorts: vec![3, 4], since: 0 };
        let (push, events) = gather.next(&level, &rule);
        assert!(matches!(push, Push::Commit { .. }), "{push:?} {events:?}");
        let (push, events) = push.next(&level, &rule);
        assert!(matches!(push, Push::Commit { .. }) && events.is_empty(), "{push:?} {events:?}");
    }

    /// The rule with the second taker's bar at 2/3 (two escorts for each who can stop the carrier).
    fn second_rule() -> PushRule {
        PushRule { second: Some(SecondTaker { escape_min: 0.66 }), ..PushRule::default() }
    }

    /// A second taker on their container holding two, its escape high (two escorts left against one defender, 0.67): the
    /// escort nearest takes, its Raid on the same container; the line says who, on what, the escape and its parts. One
    /// holding one battery gets none; no escape known, none.
    #[test]
    fn a_second_taker_raids_a_container_of_two_when_it_would_get_away() {
        let rule = second_rule();
        let two = Target::Container { id: 1, at: Vec3::new(100.0, 0.0, 0.0), batteries: 2 };
        let bots = [bot(2, 90.0), bot(3, 95.0), bot(4, 97.0), bot(5, 100.0)];
        let one = [(THEIRS, 1)];
        let escape = Some(Escape { way_m: 125.0, near: 1, cut_offs: 0, lanes: 0 });
        let p = Picture { escape, ..picture(0, &bots, &one) };
        let (push, events) = commit(THEIRS, 5, vec![2, 3, 4]).next(&p, &rule);
        assert!(events.is_empty() && matches!(push, Push::Commit { second: None, .. }), "{push:?} {events:?}");
        let targets = [(two, 1)];
        let (push, events) = commit(THEIRS, 5, vec![2, 3, 4]).next(&picture(0, &bots, &targets), &rule);
        assert!(events.is_empty() && matches!(push, Push::Commit { second: None, .. }), "no escape: {push:?} {events:?}");
        let p = Picture { escape, ..picture(1_000, &bots, &targets) };
        let before = commit(THEIRS, 5, vec![2, 3, 4]);
        let (push, events) = before.clone().next(&p, &rule);
        let Push::Commit { second: Some(second), ref escorts, .. } = push else { panic!("{push:?}") };
        assert_eq!(second, Second { place: 4, target: two });
        assert_eq!(escorts, &vec![2, 3]);
        assert_eq!(push.role(4), Some(Role::Taker(two)));
        assert_eq!(push.takers().len(), 2);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].line(&before, &push, &p),
            "push on their container 1: second taker 4 on their container 1, escape 0.67 (at least 0.66): cover 2 against 1 by \
             the target, 0 cut-offs and 0 lanes on a 125 m way home"
        );
    }

    /// In a crowd no second taker: one escort left against two by the target and one cut-off is 0.25; the nearer of two lying
    /// batteries is the second's when two escorts are left against one cut-off (0.67).
    #[test]
    fn no_second_taker_in_a_crowd() {
        let rule = second_rule();
        let first = Target::Battery { index: 3, at: Vec3::new(100.0, 0.0, 0.0) };
        let near = Target::Battery { index: 6, at: Vec3::new(100.0, 0.0, 10.0) };
        let far = Target::Battery { index: 7, at: Vec3::new(100.0, 0.0, 40.0) };
        let bots = [bot(2, 92.0), bot(3, 94.0), bot(4, 96.0), bot(5, 100.0)];
        let targets = [(first, 2), (near, 2), (far, 0)];
        let crowd = Escape { way_m: 120.0, near: 2, cut_offs: 1, lanes: 0 };
        let p = Picture { escape: Some(crowd), ..picture(0, &bots, &targets) };
        let (push, events) = commit(first, 5, vec![2, 3]).next(&p, &rule);
        assert!(events.is_empty() && matches!(push, Push::Commit { second: None, .. }), "{push:?} {events:?}");
        let clear = Escape { way_m: 120.0, near: 0, cut_offs: 1, lanes: 0 };
        let p = Picture { escape: Some(clear), ..picture(0, &bots, &targets) };
        let (push, events) = commit(first, 5, vec![2, 3, 4]).next(&p, &rule);
        let Push::Commit { second: Some(second), .. } = push else { panic!("{push:?} {events:?}") };
        assert_eq!(second, Second { place: 4, target: near }, "the nearer battery, by the escort nearest it");
    }

    /// Both take: the first carries, the second still takes (the push turns home, the second held on its battery); it
    /// carries too, home with the first; the first home, the push goes on with the second; the second home ends it.
    #[test]
    fn two_carriers_go_home_together_and_the_last_ends_it() {
        let rule = PushRule::default();
        let first = Target::Battery { index: 3, at: Vec3::new(100.0, 0.0, 0.0) };
        let near = Target::Battery { index: 6, at: Vec3::new(100.0, 0.0, 10.0) };
        let push = Push::Commit { target: first, taker: 5, escorts: vec![2], short_since: None, second: Some(Second { place: 4, target: near }) };
        let mut bots = [bot(2, 92.0), bot(4, 98.0), bot(5, 100.0)];
        bots[2].carrying = true;
        let (push, events) = push.next(&picture(0, &bots, &[(near, 1)]), &rule);
        assert_eq!(events, vec![Event::Took { carrier: 5 }]);
        assert_eq!(push, Push::Home { carrier: 5, other: Some(Other::Taking(Second { place: 4, target: near })), escorts: vec![2] });
        assert_eq!(push.role(4), Some(Role::Taker(near)));
        bots[1].carrying = true;
        let (push, events) = push.next(&picture(1_000, &bots, &[]), &rule);
        assert_eq!(events, vec![Event::Took { carrier: 4 }]);
        assert_eq!(push, Push::Home { carrier: 5, other: Some(Other::Carrying(4)), escorts: vec![2] });
        // The first lets go at home: the push goes on with the second carrier.
        let first_home = [bot(2, 30.0), Member { carrying: true, ..bot(4, 20.0) }, bot(5, 3.0)];
        let p = picture(20_000, &first_home, &[]);
        let before = push.clone();
        let (push, events) = push.next(&p, &rule);
        assert_eq!(events, vec![Event::Carried { carrier: 5, home: true }]);
        assert_eq!(push, Push::Home { carrier: 4, other: None, escorts: vec![2] });
        assert_eq!(events[0].line(&before, &push, &p), "push: 5 brought one home, the push goes on with [4, 2]");
        let both_home = [bot(2, 10.0), bot(4, 2.0), bot(5, 3.0)];
        let (push, events) = push.next(&picture(25_000, &both_home, &[]), &rule);
        assert_eq!(events, vec![Event::Ended(End::Home)]);
        assert!(matches!(push, Push::Resting { .. }));
    }

    /// The first taker dead with no escort to take: the second takes on its own battery as the push's target; the first's
    /// battery gone, the same.
    #[test]
    fn the_second_taker_takes_over_the_push() {
        let rule = PushRule::default();
        let first = Target::Battery { index: 3, at: Vec3::new(100.0, 0.0, 0.0) };
        let near = Target::Battery { index: 6, at: Vec3::new(100.0, 0.0, 10.0) };
        let second = Some(Second { place: 4, target: near });
        let push = Push::Commit { target: first, taker: 5, escorts: vec![], short_since: None, second };
        let bots = [bot(4, 98.0)];
        let (push, _) = push.next(&picture(0, &bots, &[(first, 1), (near, 1)]), &rule);
        assert_eq!((push.target(), push.role(4)), (Some(near), Some(Role::Taker(near))));
        let push = Push::Commit { target: first, taker: 5, escorts: vec![], short_since: None, second };
        let bots = [bot(4, 98.0), bot(5, 100.0)];
        let (push, _) = push.next(&picture(0, &bots, &[(near, 1)]), &rule);
        assert_eq!((push.target(), push.role(4), push.role(5)), (Some(near), Some(Role::Taker(near)), Some(Role::Escort { taker: 4 })));
    }

    /// [second taker] The escape from game data: a carrier walks 3.5 m/s, a man sprints 4.9587. A man 20 m off the way's 60th
    /// metre is there in 4 s against its 17 s: a cut-off; one 150 m off it is not, nor one 30 m behind the target (no point of
    /// the way within 25 m of him); one by the target counts as near, not as a cut-off; a lane whose shooter is already
    /// counted counts once, and each man once.
    #[test]
    fn the_escape_counts_cut_offs_and_lanes_from_the_game_speeds() {
        let (carrier, hunter) = (3.5, 4.9587);
        let way: Vec<(f32, Vec3)> = (0..=25).map(|i| (i as f32 * 4.0, Vec3::new(100.0 - i as f32 * 4.0, 0.0, 0.0))).collect();
        assert!(cuts_off(Vec3::new(40.0, 0.0, 20.0), &way, carrier, hunter));
        assert!(!cuts_off(Vec3::new(50.0, 0.0, 150.0), &way, carrier, hunter));
        assert!(!cuts_off(Vec3::new(130.0, 0.0, 0.0), &way, carrier, hunter));
        let enemies = [(7, Vec3::new(40.0, 0.0, 20.0)), (8, Vec3::new(50.0, 0.0, 150.0)), (6, Vec3::new(105.0, 0.0, 5.0))];
        let escape = Escape::of(&way, 100.0, &enemies, &[7, 6, 9, 9], carrier, hunter);
        assert_eq!(escape, Escape { way_m: 100.0, near: 1, cut_offs: 1, lanes: 1 });
        assert!((escape.chance(3) - 0.5).abs() < 1e-6);
        assert_eq!(Escape { way_m: 100.0, near: 0, cut_offs: 0, lanes: 0 }.chance(0), 1.0);
    }
}
