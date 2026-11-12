//! Stepping out of a live grenade (`AvoidTuning::grenade_avoid`; the design is `grenades-record/avoid-design.md`,
//! the client's side `docs/notes/grenade_visibility.md`).
//!
//! Root cause: no bot knew a grenade in the air or on the ground. `MatchView`, `hazards.rs`, `escape.rs` and the
//! plays carried none, so the thrower's own teammates walked into its blast (54 % of the mates inside the radius at
//! the blast were outside it at the throw) and every bot stood where a grenade lay. The server's list
//! ([`super::live_grenades`], from the throw keys) gives each grenade's thrower, team, fuse and predicted rest point;
//! this reads it for one bot.
//!
//! One bot, one grenade at a time ([`GrenadeThreat`], one transition function [`step`], total over the events):
//! - **Unaware** → it looks at the grenades every [`AvoidTuning::look_ms`]. A grenade matters only if the bot's
//!   chest is within the blast's radius and a margin, or its walk of the next [`AvoidTuning::ahead_ms`] would put it
//!   there (the walk-ins). Whether it knows of it: the thrower's team from the launch (**Told**: the owner's rule, a
//!   justified cheat: a human team can call a landing, few do; in the live matches mates are no quicker to leave
//!   than enemies); the other team once it lies on the ground and the bot is within [`AvoidTuning::hear_m`] of it
//!   (**Heard**, the first bounce) or within [`AvoidTuning::see_m`] of it, in its view cone with a line to it
//!   (**Saw**). The client shows no warning and plays no fuse cue; what a player has is the model and the bounce.
//! - **Noticed** → it acts after a reaction time drawn from [`AvoidTuning::told_ms`] / [`AvoidTuning::notice_ms`].
//!   The live matches show no human reaction at all, so these are the published reaction times (Rabin, Game AI Pro 2
//!   ch. 5), swept, not copied.
//! - **Stepping** → the legs go to the floor [`Surroundings::way_out`] gives (the cheapest free route out of the
//!   blast, or as far as it gets in the time left, `Tactics::step_out`) until the blast: the play's goal is put aside
//!   as the reflex's legs are, the reflex still aims and fires. At the floor it holds there; the play resumes after
//!   the blast.
//! - **Clear** → nothing to do for this grenade (it was out when it came to act, or no floor is in reach): looked at
//!   again every [`AvoidTuning::look_ms`], so a bot that walks in or finds a way acts then.
//!
//! Best practice: a grenade is one timed danger area, shared as a stimulus and read as a high-priority interrupt over
//! the play, which resumes after the blast (Halo 2's "avoid" and "dive impulse", Half-Life 2's danger sound,
//! Killzone's self-preservation branch); the way out is a search for the nearest floor outside the radius that is
//! reachable in time, and none reachable means it stays, not runs in (F.E.A.R., Jack's tactical position query);
//! knowledge follows perception; and the reaction is delayed, per agent, by 0.2-0.4 s and more.

use mapdata::Vec3;

use super::hazards::{ARRIVE_MPS, escape_reach_m};
use super::tactics::StepOutPick;
use super::kin;
use super::live_grenades::{LiveGrenade, Phase, Whose};
use super::rng::Rng;
use super::tuning::AvoidTuning;
use super::view::{Place, PlaceView};

/// Which grenade: its thrower and the press (a place throws one at a time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrenadeId {
    by: Place,
    press: u32,
}

impl GrenadeId {
    fn of(g: &LiveGrenade) -> Self {
        Self {
            by: g.by,
            press: g.press,
        }
    }

    fn is(self, g: &LiveGrenade) -> bool {
        g.by == self.by && g.press == self.press
    }

    /// The thrower's place and the match ms of the press, for a trace line.
    pub fn trace_key(self) -> (Place, u32) {
        (self.by, self.press)
    }
}

/// Whose side the thrower is on, from the bot that knows of the grenade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thrower {
    /// A teammate (the owner's rule: told from the launch).
    Mate,
    Enemy,
}

impl Thrower {
    fn of(me: &PlaceView, g: &LiveGrenade) -> Self {
        if g.team == me.team { Self::Mate } else { Self::Enemy }
    }
}

/// How a bot knows of a grenade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    /// The thrower's teammate: from the launch.
    Told,
    /// An enemy that sees it lie.
    Saw,
    /// An enemy that hears it land.
    Heard,
}

impl Notice {
    pub const fn index(self) -> usize {
        match self {
            Self::Told => 0,
            Self::Saw => 1,
            Self::Heard => 2,
        }
    }
}

/// A grenade a bot knows of: which, where it will lie, when it goes off, how the bot knows, and when it could first
/// have known (the launch for a teammate, the landing for an enemy).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Known {
    pub id: GrenadeId,
    pub rest: Vec3,
    /// The blast's radius, metres: the grenade's `splash_radius` (10 for the RGD-5, PROVEN).
    pub radius_m: f32,
    pub blast_at: u32,
    pub how: Notice,
    pub first: u32,
    /// Whose grenade it is (the counts are split by it).
    pub whose: Whose,
    /// On whose side the thrower is.
    pub thrower: Thrower,
}

/// One bot's state towards the grenades.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GrenadeThreat {
    /// Knows of none that matters; looks again at `look_at`.
    Unaware { look_at: u32 },
    /// Knows of `known`, acts at `act_at`.
    Noticed { known: Known, act_at: u32 },
    /// Walking out of `known`'s blast to `to`.
    Stepping { known: Known, to: Vec3 },
    /// Nothing to do for `known`; looks again at `recheck`.
    Clear { known: Known, recheck: u32 },
}

impl Default for GrenadeThreat {
    fn default() -> Self {
        Self::Unaware { look_at: 0 }
    }
}

/// What the state asks of the bot's legs this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    None,
    /// Walk to this floor, now, whatever the play wants.
    Go(Vec3),
}

/// A tick of a living bot.
#[derive(Clone, Copy)]
pub struct Tick<'a> {
    pub now: u32,
    pub me: &'a PlaceView,
    pub grenades: &'a [LiveGrenade],
}

pub enum Event<'a> {
    Tick(Tick<'a>),
    /// It died: a new life knows nothing.
    Died,
}

/// A blast to be out of: where, how far (the radius and the margin), when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blast {
    pub rest: Vec3,
    pub radius: f32,
    pub blast_at: u32,
}

/// What the bot's surroundings answer: the map's.
pub trait Surroundings {
    /// Whether nothing lies between `from` and `to`.
    fn sees(&self, from: Vec3, to: Vec3) -> bool;
    /// The floor for `me` to step to, to be out of `blast`, by at most `reach_m` of walking (with `routed`, by a planned route of
    /// that length, else by link distance), and which floor of the search it was; none if no better place is in reach. With
    /// none out of the blast within `reach_m`, the floor farthest from it within `far_m`.
    fn way_out(&self, me: &PlaceView, blast: &Blast, reach_m: f32, far_m: f32, routed: bool) -> Option<(Vec3, StepOutPick)>;
}

/// What the avoidance did, by [`Notice`] (Told, Saw, Heard); summed over a match by the bench.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AvoidStats {
    /// A grenade that mattered became known.
    pub noticed: [u32; 3],
    /// Acted on it and had a floor to go to that was away from the blast ...
    pub stepped: [u32; 3],
    /// ... or was already out of it (a walk-in held at the edge).
    pub held: [u32; 3],
    /// Came to act and was no longer in the blast's way.
    pub out_at_act: [u32; 3],
    /// No floor in reach better than where it stood.
    pub no_way: [u32; 3],
    /// The floor a step went to, by how it was found: the nearest outside the radius, a later one (the nearer routes were
    /// longer than the reach), the farthest in reach (none outside by its route).
    pub way: [u32; 3],
    /// A step begun from `Clear` (it walked in, or a way opened).
    pub retried: [u32; 3],
    /// At the blast, a bot that was stepping: out of its radius, or not.
    pub escaped: [u32; 3],
    pub caught: [u32; 3],
    /// Of the grenades noticed, those a teammate threw.
    pub noticed_mate: u32,
    /// Grenades listed from a bot's key alone that the bot would have known of and did not (`bot_pockets` on), once each.
    pub refused: u32,
    /// From the first chance to know to the start of the step: up to 0.5, 1, 1.5, 2 s, more.
    pub react: [[u32; 5]; 3],
}

impl AvoidStats {
    /// What was counted since `before` (every count of `self` is at least that of `before`).
    pub fn since(&self, before: &Self) -> Self {
        fn minus<const N: usize>(a: &[u32; N], b: &[u32; N]) -> [u32; N] {
            std::array::from_fn(|i| a[i] - b[i])
        }
        Self {
            noticed: minus(&self.noticed, &before.noticed),
            stepped: minus(&self.stepped, &before.stepped),
            held: minus(&self.held, &before.held),
            out_at_act: minus(&self.out_at_act, &before.out_at_act),
            no_way: minus(&self.no_way, &before.no_way),
            way: minus(&self.way, &before.way),
            retried: minus(&self.retried, &before.retried),
            escaped: minus(&self.escaped, &before.escaped),
            caught: minus(&self.caught, &before.caught),
            noticed_mate: self.noticed_mate - before.noticed_mate,
            refused: self.refused - before.refused,
            react: std::array::from_fn(|k| minus(&self.react[k], &before.react[k])),
        }
    }

    pub fn add(&mut self, other: &Self) {
        fn plus<const N: usize>(a: &mut [u32; N], b: &[u32; N]) {
            for (a, b) in a.iter_mut().zip(b) {
                *a += b;
            }
        }
        plus(&mut self.noticed, &other.noticed);
        plus(&mut self.stepped, &other.stepped);
        plus(&mut self.held, &other.held);
        plus(&mut self.out_at_act, &other.out_at_act);
        plus(&mut self.no_way, &other.no_way);
        plus(&mut self.way, &other.way);
        plus(&mut self.retried, &other.retried);
        plus(&mut self.escaped, &other.escaped);
        plus(&mut self.caught, &other.caught);
        self.noticed_mate += other.noticed_mate;
        self.refused += other.refused;
        for (a, b) in self.react.iter_mut().zip(&other.react) {
            plus(a, b);
        }
    }
}

/// What the avoidance decided on a tick, for the bot's trace (one note for each decision: it noticed a grenade, set out
/// to step out of one, found no way out of one, or refused to know of one a bot's key alone listed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Decision {
    Noticed(Known),
    Stepping(Known),
    NoWay(Known),
    Refused(GrenadeId),
}

impl Decision {
    /// The trace line's fields: what was decided, about whose grenade (thrower place and press ms), and how it knew.
    pub fn trace(self) -> serde_json::Value {
        let known = |what: &str, k: Known| {
            let (by, press) = k.id.trace_key();
            serde_json::json!({
                "decision": what, "grenade_by": by, "press": press, "whose": format!("{:?}", k.whose),
                "how": format!("{:?}", k.how), "thrower": format!("{:?}", k.thrower),
                "blast_at": k.blast_at, "rest": k.rest.to_array(),
            })
        };
        match self {
            Self::Noticed(k) => known("noticed", k),
            Self::Stepping(k) => known("stepping", k),
            Self::NoWay(k) => known("no way out", k),
            Self::Refused(id) => {
                let (by, press) = id.trace_key();
                serde_json::json!({ "decision": "refused", "grenade_by": by, "press": press, "whose": "Assumed" })
            }
        }
    }
}

/// The grenades a bot refused to know of (a bot's throw key whose pockets were read and held no grenade, with
/// `bot_pockets` on), each counted once however many looks it takes while the grenade is listed.
#[derive(Clone, Debug, Default)]
pub struct Refusals {
    counted: Vec<GrenadeId>,
    /// The grenade refused first in this tick, if any (for the trace).
    fresh: Option<GrenadeId>,
}

impl Refusals {
    /// A tick begins: a grenade off the list is forgotten, and nothing is fresh.
    fn begin(&mut self, grenades: &[LiveGrenade]) {
        self.fresh = None;
        self.counted.retain(|id| grenades.iter().any(|g| id.is(g)));
    }

    /// Whether `id` is refused for the first time.
    fn refuse(&mut self, id: GrenadeId) -> bool {
        let new = !self.counted.contains(&id);
        if new {
            self.counted.push(id);
            self.fresh.get_or_insert(id);
        }
        new
    }
}

/// A bot's state and its counts.
#[derive(Clone, Debug, Default)]
pub struct Avoid {
    pub state: GrenadeThreat,
    pub stats: AvoidStats,
    /// The same counts split by whose grenade it was ([`Whose::index`]): a human's, a bot's from its pockets, one
    /// listed from a bot's key alone. Sums to [`Self::stats`].
    pub by_whose: [AvoidStats; Whose::ALL.len()],
    /// What the last tick decided, if anything; the drive takes it for the trace.
    pub decided: Option<Decision>,
    pub refusals: Refusals,
}

impl GrenadeThreat {
    /// The grenade the state is about, if any.
    pub fn known(&self) -> Option<&Known> {
        match self {
            Self::Unaware { .. } => None,
            Self::Noticed { known, .. } | Self::Stepping { known, .. } | Self::Clear { known, .. } => Some(known),
        }
    }
}

impl Avoid {
    /// One tick: where the legs are to go, if the bot is stepping out of a grenade.
    pub fn tick(
        &mut self,
        tick: Tick,
        c: &AvoidTuning,
        rng: &mut Rng,
        around: &dyn Surroundings,
    ) -> Action {
        let before = self.stats;
        self.refusals.begin(tick.grenades);
        let (state, action) = step(
            self.state,
            Event::Tick(tick),
            c,
            rng,
            around,
            &mut self.stats,
            &mut self.refusals,
        );
        let mut delta = self.stats.since(&before);
        // A refusal is about a grenade the bot did not take up: it belongs to the bot-key basis whatever it is about now.
        self.by_whose[Whose::Assumed.index()].refused += std::mem::take(&mut delta.refused);
        // What this tick counted belongs to the grenade the bot was about at its end (or, if it dropped it, at its start).
        if let Some(known) = state.known().or_else(|| self.state.known()) {
            self.by_whose[known.whose.index()].add(&delta);
        }
        self.decided = decision(self.state, state, &delta, self.refusals.fresh);
        self.state = state;
        action
    }

    /// A new life.
    pub fn died(&mut self) {
        self.state = GrenadeThreat::default();
        self.refusals = Refusals::default();
    }
}

/// What a tick decided: the transition from `was` to `now_state`, with what it counted.
fn decision(was: GrenadeThreat, now_state: GrenadeThreat, delta: &AvoidStats, refused: Option<GrenadeId>) -> Option<Decision> {
    match (was, now_state) {
        (GrenadeThreat::Unaware { .. }, GrenadeThreat::Noticed { known, .. }) => Some(Decision::Noticed(known)),
        (GrenadeThreat::Noticed { .. } | GrenadeThreat::Clear { .. }, GrenadeThreat::Stepping { known, .. }) => {
            Some(Decision::Stepping(known))
        }
        (_, GrenadeThreat::Clear { known, .. }) if delta.no_way.iter().any(|&n| n > 0) => Some(Decision::NoWay(known)),
        (_, GrenadeThreat::Unaware { .. }) if refused.is_some() => refused.map(Decision::Refused),
        _ => None,
    }
}

/// The transition: the state and the event give the new state and what the legs are asked.
pub fn step(
    state: GrenadeThreat,
    event: Event,
    c: &AvoidTuning,
    rng: &mut Rng,
    around: &dyn Surroundings,
    stats: &mut AvoidStats,
    refusals: &mut Refusals,
) -> (GrenadeThreat, Action) {
    let Event::Tick(Tick { now, me, grenades }) = event else {
        return (GrenadeThreat::default(), Action::None);
    };
    let listed = |known: &Known| grenades.iter().any(|g| known.id.is(g));
    match state {
        GrenadeThreat::Unaware { look_at } => {
            if now < look_at {
                return (state, Action::None);
            }
            // The nearest blast of those that matter and that it knows of.
            let mut pick: Option<Known> = None;
            for g in grenades {
                let Phase::Flying {
                    launched,
                    lands,
                    rest,
                } = g.phase
                else {
                    continue;
                };
                if now >= g.blast_at()
                    || pick.is_some_and(|p| p.blast_at <= g.blast_at())
                    || !relevant(me, rest, g.grenade.splash_m, c)
                {
                    continue;
                }
                let Some((how, first)) = knows(me, g, launched, lands, rest, now, c, around) else {
                    continue;
                };
                // A team that reads the bots' pockets (`bot_pockets`) does not dodge a grenade the list holds from a bot's
                // key whose pockets were read and held none: the client throws nothing then. Counted once for each such
                // grenade a bot would have known of. A key of a bot whose pockets are unread may throw a frag: dodged.
                if g.whose == Whose::Assumed && c.bot_pockets {
                    stats.refused += u32::from(refusals.refuse(GrenadeId::of(g)));
                    continue;
                }
                pick = Some(Known {
                    id: GrenadeId::of(g),
                    rest,
                    radius_m: g.grenade.splash_m,
                    blast_at: g.blast_at(),
                    how,
                    first,
                    whose: g.whose,
                    thrower: Thrower::of(me, g),
                });
            }
            let Some(known) = pick else {
                return (
                    GrenadeThreat::Unaware {
                        look_at: now + c.look_ms,
                    },
                    Action::None,
                );
            };
            let (low, high) = match known.how {
                Notice::Told => c.told_ms,
                Notice::Saw | Notice::Heard => c.notice_ms,
            };
            stats.noticed[known.how.index()] += 1;
            stats.noticed_mate += u32::from(known.thrower == Thrower::Mate);
            (
                GrenadeThreat::Noticed {
                    known,
                    act_at: now + rng.range(low as f32, high as f32) as u32,
                },
                Action::None,
            )
        }
        GrenadeThreat::Noticed { known, act_at } => {
            if now >= known.blast_at || !listed(&known) {
                return (GrenadeThreat::Unaware { look_at: now }, Action::None);
            }
            if now < act_at {
                return (state, Action::None);
            }
            if !relevant(me, known.rest, known.radius_m, c) {
                stats.out_at_act[known.how.index()] += 1;
                return (clear(known, now, c), Action::None);
            }
            attempt(now, me, known, c, around, stats, true)
        }
        GrenadeThreat::Stepping { known, to } => {
            if now >= known.blast_at {
                let k = known.how.index();
                if (me.chest() - known.rest).length() >= known.radius_m {
                    stats.escaped[k] += 1;
                } else {
                    stats.caught[k] += 1;
                }
                return (GrenadeThreat::Unaware { look_at: now }, Action::None);
            }
            if !listed(&known) {
                return (GrenadeThreat::Unaware { look_at: now }, Action::None);
            }
            (state, Action::Go(to))
        }
        GrenadeThreat::Clear { known, recheck } => {
            if now >= known.blast_at || !listed(&known) {
                return (GrenadeThreat::Unaware { look_at: now }, Action::None);
            }
            if now < recheck {
                return (state, Action::None);
            }
            if relevant(me, known.rest, known.radius_m, c) {
                attempt(now, me, known, c, around, stats, false)
            } else {
                (clear(known, now, c), Action::None)
            }
        }
    }
}

fn clear(known: Known, now: u32, c: &AvoidTuning) -> GrenadeThreat {
    GrenadeThreat::Clear {
        known,
        recheck: now + c.look_ms,
    }
}

/// Acts on `known`: a floor to walk to, or nothing better in reach.
fn attempt(
    now: u32,
    me: &PlaceView,
    known: Known,
    c: &AvoidTuning,
    around: &dyn Surroundings,
    stats: &mut AvoidStats,
    first_try: bool,
) -> (GrenadeThreat, Action) {
    let blast = Blast {
        rest: known.rest,
        radius: known.radius_m + c.margin_m,
        blast_at: known.blast_at,
    };
    let left_ms = known.blast_at.saturating_sub(now);
    let far_m = ARRIVE_MPS * left_ms as f32 / 1000.0;
    let routed = c.escape;
    let reach_m = if routed { escape_reach_m(left_ms) } else { far_m };
    let k = known.how.index();
    match around.way_out(me, &blast, reach_m, far_m, routed) {
        Some((to, pick)) => {
            stats.way[pick as usize] += 1;
            if !first_try {
                stats.retried[k] += 1;
            } else if (me.chest() - known.rest).length() >= blast.radius {
                stats.held[k] += 1;
            } else {
                stats.stepped[k] += 1;
            }
            if first_try {
                let late = now.saturating_sub(known.first);
                let bin = [500, 1_000, 1_500, 2_000]
                    .iter()
                    .position(|&to| late <= to)
                    .unwrap_or(4);
                stats.react[k][bin] += 1;
            }
            (GrenadeThreat::Stepping { known, to }, Action::Go(to))
        }
        None => {
            if first_try {
                stats.no_way[k] += 1;
            }
            (clear(known, now, c), Action::None)
        }
    }
}

/// Whether a grenade that will lie at `rest` is the bot's business: its chest is within the blast's radius and the
/// margin, or its walk of the next `ahead_ms` would put it within the radius.
fn relevant(me: &PlaceView, rest: Vec3, radius_m: f32, c: &AvoidTuning) -> bool {
    let chest = me.chest();
    if (chest - rest).length() < radius_m + c.margin_m {
        return true;
    }
    let ahead = c.ahead_ms as f32 / 1000.0;
    let walked = chest + Vec3::new(me.velocity[0] * ahead, 0.0, me.velocity[1] * ahead);
    (walked - rest).length() < radius_m
}

/// How the bot knows of grenade `g` now, if it does, and when it could first have known.
#[allow(clippy::too_many_arguments)]
fn knows(
    me: &PlaceView,
    g: &LiveGrenade,
    launched: u32,
    lands: u32,
    rest: Vec3,
    now: u32,
    c: &AvoidTuning,
    around: &dyn Surroundings,
) -> Option<(Notice, u32)> {
    if g.team == me.team {
        return (now >= launched).then_some((Notice::Told, launched));
    }
    if now < lands {
        return None;
    }
    let d = (me.chest() - rest).length();
    if d <= c.hear_m {
        return Some((Notice::Heard, lands));
    }
    let sees = d <= c.see_m
        && in_cone(me, rest, c.cone_deg)
        && around.sees(me.eye(), rest + Vec3::new(0.0, 0.1, 0.0));
    sees.then_some((Notice::Saw, lands))
}

/// Whether `at` is within the view cone (the whole angle `cone_deg`) round the bot's yaw.
fn in_cone(me: &PlaceView, at: Vec3, cone_deg: f32) -> bool {
    let (dx, dz) = (at.x - me.feet.x, at.z - me.feet.z);
    if dx.hypot(dz) < 1e-3 {
        return true;
    }
    let off = (kin::yaw_of([dx, dz]) - me.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    off.abs() <= cone_deg.to_radians() / 2.0
}

impl super::features::Feature for Avoid {
    fn new_life(&mut self, _life: &super::features::Life) {
        self.died();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::arc;

    /// The surroundings of a test: whether a line is clear, and the floor to step to.
    struct Around {
        sees: bool,
        out: Option<Vec3>,
    }

    impl Surroundings for Around {
        fn sees(&self, _: Vec3, _: Vec3) -> bool {
            self.sees
        }

        fn way_out(&self, _: &PlaceView, _: &Blast, _: f32, _: f32, _: bool) -> Option<(Vec3, StepOutPick)> {
            self.out.map(|to| (to, StepOutPick::Nearest))
        }
    }

    fn view(team: u8, feet: Vec3, velocity: [f32; 2]) -> PlaceView {
        PlaceView {
            place: 0,
            team,
            bot: true,
            present: true,
            alive: true,
            feet,
            yaw: 0.0,
            pitch: 0.0,
            velocity,
            keys: 0,
            health: 1.0,
            spawned: 0,
            kills: 0,
            deaths: 0,
            streak: 0,
            carrying: None,
            seen_at: 0,
            late_ms: 0,
            known_at: 0,
        }
    }

    /// A grenade thrown by place 7 of team 0 at press 1000: let go at 2100, on the ground at 2600, goes off at
    /// 5000, resting 12 m from the origin along +z.
    fn grenade() -> LiveGrenade {
        LiveGrenade {
            by: 7,
            team: 0,
            press: 1_000,
            phase: Phase::Flying {
                launched: 2_100,
                lands: 2_600,
                rest: Vec3::new(0.0, 0.0, 12.0),
            },
            grenade: *arc::Grenade::bots(),
            whose: Whose::Human,
        }
    }

    fn tuning() -> AvoidTuning {
        AvoidTuning {
            grenade_avoid: true,
            told_ms: (300, 300),
            notice_ms: (600, 600),
            ..AvoidTuning::default()
        }
    }

    fn run(
        state: GrenadeThreat,
        now: u32,
        me: &PlaceView,
        grenades: &[LiveGrenade],
        around: &Around,
        stats: &mut AvoidStats,
    ) -> (GrenadeThreat, Action) {
        let mut rng = Rng::new(1);
        step(
            state,
            Event::Tick(Tick { now, me, grenades }),
            &tuning(),
            &mut rng,
            around,
            stats,
            &mut Refusals::default(),
        )
    }

    const OUT: Vec3 = Vec3::new(0.0, 0.0, -2.0);

    /// A grenade listed from a bot's key alone (the client throws nothing) is dodged only by a team that does not read
    /// the bots' pockets (`bot_pockets` off, the old behaviour); a team with it on does not notice it. The counts are
    /// split by whose grenade it was.
    #[test]
    fn an_assumed_grenade_is_dodged_only_with_bot_pockets_off_and_counted_apart() {
        let me = view(0, Vec3::new(0.0, 0.0, 9.0), [0.0, 0.0]);
        let around = Around {
            sees: false,
            out: Some(OUT),
        };
        let assumed = [LiveGrenade {
            whose: Whose::Assumed,
            ..grenade()
        }];
        let tick = |c: &AvoidTuning, avoid: &mut Avoid, now: u32| {
            let mut rng = Rng::new(1);
            avoid.tick(Tick { now, me: &me, grenades: &assumed }, c, &mut rng, &around)
        };
        let on = tuning();
        let off = AvoidTuning { bot_pockets: false, ..tuning() };
        assert!(on.bot_pockets);
        let mut avoid = Avoid::default();
        let mut decided = Vec::new();
        for now in [2_200, 2_500, 3_000] {
            assert_eq!(tick(&on, &mut avoid, now), Action::None);
            decided.push(avoid.decided);
        }
        assert_eq!(avoid.stats.noticed, [0, 0, 0], "not noticed with bot_pockets on");
        // The refusal is counted once for the grenade, under the bot-key basis, and decided once.
        assert_eq!(avoid.stats.refused, 1);
        assert_eq!(avoid.by_whose[Whose::Assumed.index()].refused, 1);
        assert!(matches!(decided[0], Some(Decision::Refused(_))), "{decided:?}");
        assert_eq!(decided[1..], [None, None]);
        let mut avoid = Avoid::default();
        tick(&off, &mut avoid, 2_200);
        assert!(matches!(avoid.decided, Some(Decision::Noticed(k)) if k.thrower == Thrower::Mate && k.whose == Whose::Assumed));
        assert_eq!(tick(&off, &mut avoid, 2_500), Action::Go(OUT));
        assert!(matches!(avoid.decided, Some(Decision::Stepping(_))));
        assert_eq!(avoid.decided.map(|d| d.trace()["decision"].clone()), Some(serde_json::json!("stepping")));
        assert_eq!(avoid.stats.noticed, [1, 0, 0]);
        assert_eq!(avoid.stats.noticed_mate, 1, "the thrower's team is the bot's");
        assert_eq!(avoid.stats.refused, 0);
        // The counts sit under the grenade's owner and sum to the total.
        assert_eq!(avoid.by_whose[Whose::Assumed.index()].noticed, [1, 0, 0]);
        assert_eq!(avoid.by_whose[Whose::Assumed.index()].stepped, [1, 0, 0]);
        assert_eq!(avoid.by_whose[Whose::Human.index()], AvoidStats::default());
        let mut sum = AvoidStats::default();
        for each in &avoid.by_whose {
            sum.add(each);
        }
        assert_eq!(sum, avoid.stats);
    }

    /// The reach asked of the map for a way out is the pace a body can count on over the time left ([`escape_reach_m`]), not
    /// the sprint's 5 m/s over all of it; the floor's pick is counted.
    #[test]
    fn the_reach_asked_for_a_way_out_is_the_escape_pace_of_the_time_left() {
        struct Reach(std::cell::Cell<f32>, std::cell::Cell<bool>);
        impl Surroundings for Reach {
            fn sees(&self, _: Vec3, _: Vec3) -> bool {
                false
            }

            fn way_out(&self, _: &PlaceView, _: &Blast, reach_m: f32, _: f32, routed: bool) -> Option<(Vec3, StepOutPick)> {
                self.1.set(routed);
                self.0.set(reach_m);
                Some((OUT, StepOutPick::Later))
            }
        }
        let me = view(0, Vec3::new(0.0, 0.0, 9.0), [0.0, 0.0]);
        let around = Reach(std::cell::Cell::new(0.0), std::cell::Cell::new(false));
        let off = AvoidTuning { bot_pockets: false, escape: true, ..tuning() };
        let assumed = [LiveGrenade { whose: Whose::Assumed, ..grenade() }];
        let mut avoid = Avoid::default();
        let mut rng = Rng::new(1);
        avoid.tick(Tick { now: 2_200, me: &me, grenades: &assumed }, &off, &mut rng, &around);
        let acts = avoid.tick(Tick { now: 2_500, me: &me, grenades: &assumed }, &off, &mut rng, &around);
        assert_eq!(acts, Action::Go(OUT));
        let Some(Decision::Stepping(known)) = avoid.decided else { panic!("{:?}", avoid.decided) };
        let left = known.blast_at - 2_500;
        assert_eq!(around.0.get(), escape_reach_m(left));
        assert!(around.0.get() < 5.0 * left as f32 / 1000.0, "less than the sprint's pace over all of it");
        assert_eq!(avoid.stats.way, [0, 1, 0]);
        assert!(around.1.get(), "by the planned route");
        // With the escape off: the sprint's pace over all the time left, by link distance, as it was.
        let old = AvoidTuning { escape: false, ..off };
        let mut avoid = Avoid::default();
        avoid.tick(Tick { now: 2_200, me: &me, grenades: &assumed }, &old, &mut rng, &around);
        avoid.tick(Tick { now: 2_500, me: &me, grenades: &assumed }, &old, &mut rng, &around);
        let Some(Decision::Stepping(known)) = avoid.decided else { panic!("{:?}", avoid.decided) };
        assert_eq!(around.0.get(), ARRIVE_MPS * (known.blast_at - 2_500) as f32 / 1000.0);
        assert!(!around.1.get());
    }

    /// Two assumed grenades known at once (a volley, two throwers) are each refused and counted once however many looks the
    /// bot takes, and a refusal is not forgotten when the bot's state changes in between; a grenade that left the list is
    /// forgotten.
    #[test]
    fn each_refused_grenade_is_counted_once_even_with_two_listed() {
        let me = view(0, Vec3::new(0.0, 0.0, 9.0), [0.0, 0.0]);
        let around = Around { sees: false, out: Some(OUT) };
        let one = LiveGrenade { whose: Whose::Assumed, ..grenade() };
        let two = LiveGrenade { by: one.by + 1, press: one.press + 100, ..one };
        let on = tuning();
        let mut avoid = Avoid::default();
        let mut tick = |now: u32, grenades: &[LiveGrenade]| {
            let mut rng = Rng::new(1);
            avoid.tick(Tick { now, me: &me, grenades }, &on, &mut rng, &around);
        };
        for now in [2_200, 2_500, 2_800, 3_100] {
            tick(now, &[one, two]);
        }
        assert_eq!(avoid.stats.refused, 2, "two grenades, two refusals, not one for each look");
        assert_eq!(avoid.by_whose[Whose::Assumed.index()].refused, 2);
        let mut tick = |now: u32, grenades: &[LiveGrenade]| {
            let mut rng = Rng::new(1);
            avoid.tick(Tick { now, me: &me, grenades }, &on, &mut rng, &around);
        };
        tick(3_400, &[two]);
        tick(3_700, &[]);
        assert_eq!(avoid.stats.refused, 2);
        assert!(avoid.refusals.counted.is_empty(), "grenades off the list are forgotten");
    }

    /// A throw key of a bot whose pockets are unread may throw a frag: a team with `bot_pockets` on dodges it like any other.
    #[test]
    fn an_unread_grenade_is_dodged_by_a_team_that_reads_the_bots_pockets() {
        let me = view(0, Vec3::new(0.0, 0.0, 9.0), [0.0, 0.0]);
        let around = Around { sees: false, out: Some(OUT) };
        let unread = [LiveGrenade { whose: Whose::Unread, ..grenade() }];
        let on = tuning();
        assert!(on.bot_pockets);
        let mut avoid = Avoid::default();
        let mut rng = Rng::new(1);
        avoid.tick(Tick { now: 2_200, me: &me, grenades: &unread }, &on, &mut rng, &around);
        assert!(matches!(avoid.decided, Some(Decision::Noticed(k)) if k.whose == Whose::Unread));
        assert_eq!(avoid.stats.refused, 0);
        assert_eq!(avoid.by_whose[Whose::Unread.index()].noticed, [1, 0, 0]);
    }

    /// A teammate of the thrower 3 m from the rest point is told at the launch, acts after its reaction time and
    /// walks to the floor out of the blast until it goes off.
    #[test]
    fn a_teammate_knows_at_the_launch_and_steps_out_after_its_reaction_time() {
        let me = view(0, Vec3::new(0.0, 0.0, 9.0), [0.0, 0.0]);
        let around = Around {
            sees: false,
            out: Some(OUT),
        };
        let mut stats = AvoidStats::default();
        let gs = [grenade()];
        // In the hand: nothing known.
        let (s, a) = run(GrenadeThreat::default(), 2_000, &me, &gs, &around, &mut stats);
        assert_eq!((s, a), (GrenadeThreat::Unaware { look_at: 2_200 }, Action::None));
        // Let go: told, to act 300 ms on.
        let (s, a) = run(s, 2_200, &me, &gs, &around, &mut stats);
        let GrenadeThreat::Noticed { known, act_at } = s else {
            panic!("{s:?}");
        };
        assert_eq!((a, act_at, known.how, known.first), (Action::None, 2_500, Notice::Told, 2_100));
        assert_eq!(stats.noticed, [1, 0, 0]);
        // Not yet.
        assert_eq!(run(s, 2_400, &me, &gs, &around, &mut stats).1, Action::None);
        // Then: the way out, every tick till the blast.
        let (s, a) = run(s, 2_500, &me, &gs, &around, &mut stats);
        assert_eq!(a, Action::Go(OUT));
        assert!(matches!(s, GrenadeThreat::Stepping { .. }));
        assert_eq!(stats.stepped, [1, 0, 0]);
        assert_eq!(stats.react[0], [1, 0, 0, 0, 0], "400 ms after it could first know");
        let (s, a) = run(s, 3_500, &me, &gs, &around, &mut stats);
        assert_eq!(a, Action::Go(OUT));
        // At the blast it is out, or not: here it has not moved (a test), so caught.
        let (s, a) = run(s, 5_000, &me, &gs, &around, &mut stats);
        assert_eq!((s, a), (GrenadeThreat::Unaware { look_at: 5_000 }, Action::None));
        assert_eq!((stats.escaped, stats.caught), ([0, 0, 0], [1, 0, 0]));
        // The listing runs 300 ms past the blast: not a grenade to act on again.
        let (s, _) = run(s, 5_100, &me, &gs, &around, &mut stats);
        assert!(matches!(s, GrenadeThreat::Unaware { .. }));
        assert_eq!(stats.noticed, [1, 0, 0]);
    }

    /// An enemy knows of a grenade on the ground only by hearing it land close or seeing it lie with a line to it
    /// in its view, and not at all before it lands.
    #[test]
    fn an_enemy_knows_only_by_hearing_or_seeing_it_on_the_ground() {
        let gs = [grenade()];
        let mut stats = AvoidStats::default();
        // 9 m from the rest point, facing +z toward it (yaw 0), the line clear.
        let me = view(1, Vec3::new(0.0, 0.0, 3.0), [0.0, 0.0]);
        let sees = Around {
            sees: true,
            out: Some(OUT),
        };
        let blind = Around {
            sees: false,
            out: Some(OUT),
        };
        // In the air: no.
        assert!(matches!(
            run(GrenadeThreat::default(), 2_300, &me, &gs, &sees, &mut stats).0,
            GrenadeThreat::Unaware { .. }
        ));
        // On the ground, 9 m: beyond hearing (8 m), seen.
        let (s, _) = run(GrenadeThreat::default(), 2_700, &me, &gs, &sees, &mut stats);
        let GrenadeThreat::Noticed { known, act_at } = s else {
            panic!("{s:?}");
        };
        assert_eq!((known.how, known.first, act_at), (Notice::Saw, 2_600, 3_300));
        // A wall between: not seen.
        assert!(matches!(
            run(GrenadeThreat::default(), 2_700, &me, &gs, &blind, &mut stats).0,
            GrenadeThreat::Unaware { .. }
        ));
        // Back turned: not in the cone.
        let turned = PlaceView {
            yaw: std::f32::consts::PI,
            ..me.clone()
        };
        assert!(matches!(
            run(GrenadeThreat::default(), 2_700, &turned, &gs, &sees, &mut stats).0,
            GrenadeThreat::Unaware { .. }
        ));
        // 5 m: heard, wall or no wall, back turned or not.
        let close = view(1, Vec3::new(0.0, 0.0, 7.0), [0.0, 0.0]);
        let (s, _) = run(GrenadeThreat::default(), 2_700, &close, &gs, &blind, &mut stats);
        assert!(matches!(s, GrenadeThreat::Noticed { known, .. } if known.how == Notice::Heard));
        assert_eq!(stats.noticed, [0, 1, 1]);
    }

    /// A bot outside the blast that is not walking in has nothing to do; one walking in counts as in it and holds
    /// at the edge (its own floor); one inside with no floor in reach is Clear and looks again.
    #[test]
    fn only_a_bot_in_the_blast_or_walking_into_it_acts_and_none_in_reach_means_clear() {
        let gs = [grenade()];
        let mut stats = AvoidStats::default();
        let t = |me: &PlaceView, around: &Around, stats: &mut AvoidStats| {
            let (s, _) = run(GrenadeThreat::default(), 2_300, me, &gs, around, stats);
            s
        };
        let around = Around {
            sees: true,
            out: Some(OUT),
        };
        // 20 m off the rest point and still: unconcerned.
        let far = view(0, Vec3::new(0.0, 0.0, -8.0), [0.0, 0.0]);
        assert!(matches!(t(&far, &around, &mut stats), GrenadeThreat::Unaware { .. }));
        // The same walking in at 4 m/s: 11 m out now, 7 m in a second: noticed.
        let walking = view(0, Vec3::new(0.0, 0.0, -1.0), [0.0, 4.0]);
        let s = t(&walking, &around, &mut stats);
        assert!(matches!(s, GrenadeThreat::Noticed { .. }), "{s:?}");
        let (s, a) = run(s, 2_700, &walking, &gs, &Around { sees: true, out: Some(Vec3::new(0.0, 0.0, -1.0)) }, &mut stats);
        assert_eq!(a, Action::Go(Vec3::new(0.0, 0.0, -1.0)));
        assert!(matches!(s, GrenadeThreat::Stepping { .. }));
        assert_eq!((stats.held, stats.stepped), ([1, 0, 0], [0, 0, 0]), "held at the edge");
        // Inside with no floor in reach: Clear, looks again after the look time, then finds one.
        let inside = view(0, Vec3::new(0.0, 0.0, 9.0), [0.0, 0.0]);
        let s = t(&inside, &around, &mut stats);
        let (s, a) = run(s, 2_700, &inside, &gs, &Around { sees: true, out: None }, &mut stats);
        assert!(matches!(s, GrenadeThreat::Clear { recheck: 2_900, .. }), "{s:?}");
        assert_eq!((a, stats.no_way), (Action::None, [1, 0, 0]));
        assert_eq!(run(s, 2_800, &inside, &gs, &around, &mut stats).1, Action::None);
        let (s, a) = run(s, 2_900, &inside, &gs, &around, &mut stats);
        assert_eq!(a, Action::Go(OUT));
        assert!(matches!(s, GrenadeThreat::Stepping { .. }));
        assert_eq!(stats.retried, [1, 0, 0]);
    }

    /// A step that gets out in time is counted as an escape at the blast; the death of the bot, or the grenade
    /// leaving the list, ends it.
    #[test]
    fn a_step_that_got_out_is_an_escape_and_a_death_or_a_gone_grenade_ends_it() {
        let gs = [grenade()];
        let mut stats = AvoidStats::default();
        let around = Around {
            sees: false,
            out: Some(OUT),
        };
        let inside = view(0, Vec3::new(0.0, 0.0, 9.0), [0.0, 0.0]);
        let known = Known {
            id: GrenadeId::of(&gs[0]),
            rest: Vec3::new(0.0, 0.0, 12.0),
            radius_m: arc::Grenade::bots().splash_m,
            blast_at: 5_000,
            how: Notice::Told,
            first: 2_100,
            whose: Whose::Human,
            thrower: Thrower::Mate,
        };
        let stepping = GrenadeThreat::Stepping { known, to: OUT };
        // It got to the floor 14 m from the rest point by the blast.
        let out = view(0, Vec3::new(0.0, 0.0, -2.0), [0.0, 0.0]);
        let (s, _) = run(stepping, 5_000, &out, &gs, &around, &mut stats);
        assert!(matches!(s, GrenadeThreat::Unaware { .. }));
        assert_eq!((stats.escaped, stats.caught), ([1, 0, 0], [0, 0, 0]));
        // The grenade is gone from the list before the blast: nothing more to walk to.
        let (s, a) = run(stepping, 3_000, &inside, &[], &around, &mut stats);
        assert_eq!((s, a), (GrenadeThreat::Unaware { look_at: 3_000 }, Action::None));
        // Dead: a new life knows nothing.
        let mut rng = Rng::new(1);
        let (s, a) = step(stepping, Event::Died, &tuning(), &mut rng, &around, &mut stats, &mut Refusals::default());
        assert_eq!((s, a), (GrenadeThreat::default(), Action::None));
    }

    #[test]
    fn the_stats_add_up() {
        let a = AvoidStats {
            noticed: [1, 2, 3],
            react: [[0; 5], [0, 1, 0, 0, 2], [0; 5]],
            ..AvoidStats::default()
        };
        let mut b = AvoidStats::default();
        b.add(&a);
        b.add(&a);
        assert_eq!(b.noticed, [2, 4, 6]);
        assert_eq!(b.react[1], [0, 2, 0, 0, 4]);
    }
}
