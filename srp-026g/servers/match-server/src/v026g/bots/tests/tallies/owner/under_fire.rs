//! A230 (the owner, 2026-11-02): do bots under fire switch to the fight, or keep their team play? A220 (the live bots, the
//! recorded PvB matches a human ended with KD 10 or more): the bots he killed died in PairUp 22 %, Roam 20 %, Collect 11 %,
//! Buddy 10 %, HoldExit 9 %, Trade 7 %, a play a median 3 s old, and 26 % fired nothing in their last 5 s (A222: 68 % of
//! those within about 1 s of his first hit). Our tip fights in the reflex whatever the play (it aims and fires at its
//! target), so a team play kept under fire is not by itself no fight. Two counts, the stand-in human's hits and kills apart:
//!
//! - **the first enemy hit of a bot's life**, by the play at the hit: followed to the bot's death or [`FOLLOW_MS`]: the view
//!   off the shooter at the hit (bands as `look.rs`), whether its own sight of him (`Track::sight_since`) began before the
//!   hit, and the times from the hit until its reflex targets him, it presses fire, it fires with him the target, and its
//!   play changes (and to what); the time from its own first sight of him to its first shot at him; whether it died within
//!   1 s, died in the window, and died in the play it was hit in;
//! - **each bot's death**, by the play at the death: whether it pressed fire in its last 5 s (A220's measure) and whether its
//!   first hit of the life came within 1 s of the death.
//!
//! The times are in [`BIN_MS`] bins up to [`FOLLOW_MS`] (the last bin: never in the window). Nothing decides by these counts.
//!
//! A274 (the owner, 2026-11-02): where the time from its own first sight to its first shot at him goes. Each tick of the first
//! hit's episode, from that sight (at most [`BEFORE_MS`] before the hit) to the shot, is put in the [`Stage`] of the fire chain
//! that held it (its reflex's target and fire gates); kept apart before the hit and after, for the episodes that shot and
//! those that never did (and the stage they ended in), with the turn's view degrees a tick and each gate's ticks.

use super::*;
use crate::v026g::bots::kin;
use crate::v026g::bots::know::Source;
use crate::v026g::bots::reflex::Gates;
use std::collections::VecDeque;

/// A first hit is followed this long, ms.
const FOLLOW_MS: u32 = 10_000;
/// A death this soon after the first hit, ms (A222's window).
const QUICK_MS: u32 = 1_000;
/// A220's window: a shot this long before the death, ms.
const LAST_MS: u32 = 5_000;
/// The times' bin, ms.
const BIN_MS: u32 = 250;
/// The bins: up to [`FOLLOW_MS`], then one for never.
const BINS: usize = (FOLLOW_MS / BIN_MS) as usize + 1;
/// The view bands off the shooter at the hit, degrees: up to each, then past the last (as `look.rs`).
const BANDS_DEG: [f32; 3] = [15.0, 45.0, 90.0];

/// The bin of a time `ms` after its start; none: never in the window.
fn bin(ms: Option<u32>) -> usize {
    ms.map_or(BINS - 1, |ms| ((ms / BIN_MS) as usize).min(BINS - 2))
}

/// [A274] Where a first-hit episode's time goes, tick by tick, from the bot's own first sight of the shooter (at most
/// [`BEFORE_MS`] before the hit) to its first shot at him, its death or [`FOLLOW_MS`]: the first stage of the fire chain that
/// holds the shot (`reflex::gate`, the gates its reflex failed that tick on its target), in the chain's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// Not in its own sight.
    Unseen,
    /// In sight, inside the noticing delay (`Track::notice_ms`).
    Noticing,
    /// Noticed, not its target (and no other).
    NotTaken,
    /// Its target another enemy.
    OtherTarget,
    /// Its target him, but the reflex did not aim this tick (getting unstuck, the play's hold on fire, a jump, a use, a throw).
    Outer,
    /// A threat's look had the view (a hit's shooter, a near shot).
    ThreatLook,
    /// The reaction after noticing (`first_shot_at`), first or again.
    Reaction,
    /// The view farther off than the fire gate (the turn to him).
    Turn,
    /// The sights not yet up (aiming in, first or again).
    AimIn,
    /// Running: no aim while it runs, and the hip cone does not fit.
    Running,
    /// A reload, the switch to the pistol, or no round in the weapon.
    Weapon,
    /// Farther than the weapon's range.
    Range,
    /// A teammate in the line of fire.
    Friend,
    /// `clear_shot`: no part of him shows, or the view's or the aim's ray meets the world.
    ClearShot,
    /// The shot waits for the aim to settle.
    Settling,
    /// Every gate open, the weapon's own cycle (between bursts, between presses, a burst cut).
    Cycle,
    /// Every gate open and no shot (none of the above).
    Open,
}

impl Stage {
    const ALL: [Stage; 17] = [
        Stage::Unseen,
        Stage::Noticing,
        Stage::NotTaken,
        Stage::OtherTarget,
        Stage::Outer,
        Stage::ThreatLook,
        Stage::Reaction,
        Stage::Turn,
        Stage::AimIn,
        Stage::Running,
        Stage::Weapon,
        Stage::Range,
        Stage::Friend,
        Stage::ClearShot,
        Stage::Settling,
        Stage::Cycle,
        Stage::Open,
    ];

    fn name(self) -> &'static str {
        match self {
            Stage::Unseen => "unseen",
            Stage::Noticing => "noticing",
            Stage::NotTaken => "not_taken",
            Stage::OtherTarget => "other_target",
            Stage::Outer => "outer",
            Stage::ThreatLook => "threat_look",
            Stage::Reaction => "reaction",
            Stage::Turn => "turn",
            Stage::AimIn => "aim_in",
            Stage::Running => "running",
            Stage::Weapon => "weapon",
            Stage::Range => "range",
            Stage::Friend => "friend",
            Stage::ClearShot => "clear_shot",
            Stage::Settling => "settling",
            Stage::Cycle => "cycle",
            Stage::Open => "open",
        }
    }

    /// The stage of one tick at `now` on shooter `shooter`: its reflex's `target` and the `gates` it last failed, its own sight
    /// of him (since, noticing delay) if it is up.
    fn of(target: Option<Place>, shooter: Place, sight: Option<(u32, u32)>, gates: Gates, now: u32) -> Stage {
        use crate::v026g::bots::reflex::gate as g;
        if target != Some(shooter) {
            return match (target, sight) {
                (Some(_), _) => Stage::OtherTarget,
                (None, None) => Stage::Unseen,
                (None, Some((since, notice))) if now < since.saturating_add(notice) => Stage::Noticing,
                (None, Some(_)) => Stage::NotTaken,
            };
        }
        let bits = gates.bits;
        let any = |mask: u32| bits & mask != 0;
        if gates.at != now || gates.target != Some(shooter) {
            return Stage::Outer;
        }
        let outer = g::UNSTICK | g::PLAY | g::JUMP_ROUTE | g::JUMP_HOP | g::JUMP_UNSTICK | g::JUMP_OTHER | g::USING | g::THROW;
        for (mask, stage) in [
            (outer, Stage::Outer),
            (g::THREAT, Stage::ThreatLook),
            (g::REACTION | g::REACTION_AGAIN, Stage::Reaction),
            (g::WITHIN, Stage::Turn),
            (g::AIMING_IN | g::AIMING_AGAIN, Stage::AimIn),
            (g::RUNNING, Stage::Running),
            (g::RELOAD | g::SWITCH | g::ROUNDS, Stage::Weapon),
            (g::RANGE, Stage::Range),
            (g::FRIEND, Stage::Friend),
            (g::HIDDEN | g::VIEW_RAY | g::AIM_RAY, Stage::ClearShot),
            (g::SETTLING, Stage::Settling),
            (g::PAUSE | g::TRIGGER | g::CUT, Stage::Cycle),
        ] {
            if any(mask) {
                return stage;
            }
        }
        Stage::Open
    }
}

/// [A274] Before the hit, the episode's time from its own first sight of the shooter is taken back this far, ms (the log each
/// bot keeps of its ticks).
const BEFORE_MS: u32 = 6_000;
/// [A274] Firing before the hit: a fire key this long before it, ms (`live_firstshot.py`'s split).
const FIRING_MS: u32 = 1_000;
const STAGES: usize = Stage::ALL.len();
/// [A274] One logged tick of a bot: the tick, its reflex target, its fire gates, its fire key.
type Logged = (u32, Option<Place>, Gates, bool);

/// [A274] What an episode is: it had shot at him already in its sight before the hit (a fight under way), it had not and shot at
/// him in the window (it fired back), or it had not and never did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Engaged,
    FiredBack,
    Never,
}

impl Kind {
    const ALL: [Kind; 3] = [Kind::Engaged, Kind::FiredBack, Kind::Never];

    fn name(self) -> &'static str {
        match self {
            Kind::Engaged => "engaged",
            Kind::FiredBack => "fired_back",
            Kind::Never => "never",
        }
    }

    fn of(hit: &Open) -> Kind {
        match (hit.engaged, hit.fire_him_at) {
            (true, _) => Kind::Engaged,
            (false, Some(_)) => Kind::FiredBack,
            (false, None) => Kind::Never,
        }
    }
}

/// [A274] The episodes of one [`Kind`]: ticks in each stage before the hit (from the bot's own first sight of him to the hit, or
/// to its first shot at him before it) and after it (to the first shot at him, the death or the window's end); the stage of the
/// last tick and the deaths of those that never shot at him after the hit; the turn's view degrees and ticks; how many saw him
/// and targeted him, the ticks it fired at another; the times to its first shot at him after the hit, from its own first sight
/// and from the hit.
#[derive(Clone, Debug)]
struct KindCounts {
    episodes: u64,
    before: [u64; STAGES],
    after: [u64; STAGES],
    ended_in: [u64; STAGES],
    ended_dead: [u64; STAGES],
    turn_deg: f64,
    turn_ticks: u64,
    seen: u64,
    targeted: u64,
    fired_other_ticks: u64,
    sight_to_shot: [u64; BINS],
    hit_to_shot: [u64; BINS],
}

impl Default for KindCounts {
    fn default() -> Self {
        Self {
            episodes: 0,
            before: [0; STAGES],
            after: [0; STAGES],
            ended_in: [0; STAGES],
            ended_dead: [0; STAGES],
            turn_deg: 0.0,
            turn_ticks: 0,
            seen: 0,
            targeted: 0,
            fired_other_ticks: 0,
            sight_to_shot: [0; BINS],
            hit_to_shot: [0; BINS],
        }
    }
}

/// [A274] The episodes of one shooter's side (the stand-in human's hits or the bots'), by [`Kind`]; each gate's ticks; those not
/// firing just before the hit.
#[derive(Clone, Debug, Default)]
struct StageCounts {
    kinds: [KindCounts; 3],
    /// The gates' bits, ticks each failed with him the target (`reflex::gate::NAMES`' order).
    gate_ticks: Vec<u64>,
    /// The episodes not firing in the [`FIRING_MS`] before the hit (as the live tables' `live_firstshot.py`): those that pressed
    /// fire (at anyone) in the window, those that died before any, the time from the hit to the first fire key.
    fresh: u64,
    fresh_fired: u64,
    fresh_died_first: u64,
    fresh_to_fire: Vec<u64>,
}

impl StageCounts {
    fn add(&mut self, hit: &Open, died: bool) {
        let k = &mut self.kinds[Kind::of(hit) as usize];
        k.episodes += 1;
        for (s, (b, a)) in hit.before.iter().zip(&hit.after).enumerate() {
            k.before[s] += u64::from(*b);
            k.after[s] += u64::from(*a);
        }
        if hit.fire_him_at.is_none()
            && let Some(last) = hit.last_stage
        {
            k.ended_in[last as usize] += 1;
            k.ended_dead[last as usize] += u64::from(died);
        }
        k.turn_deg += f64::from(hit.turn_deg);
        k.turn_ticks += u64::from(hit.turn_ticks);
        let seen = hit.seen_before.or(hit.seen_after);
        k.seen += u64::from(seen.is_some());
        k.targeted += u64::from(hit.target_at.is_some());
        k.fired_other_ticks += u64::from(hit.fired_other_ticks);
        k.sight_to_shot[bin(seen.zip(hit.fire_him_at).map(|(s, f)| f.saturating_sub(s)))] += 1;
        k.hit_to_shot[bin(hit.fire_him_at.map(|f| f.saturating_sub(hit.at)))] += 1;
        if !hit.firing_before {
            self.fresh += 1;
            self.fresh_fired += u64::from(hit.fire_at.is_some());
            self.fresh_died_first += u64::from(died && hit.fire_at.is_none());
            if self.fresh_to_fire.is_empty() {
                self.fresh_to_fire = vec![0; BINS];
            }
            self.fresh_to_fire[bin(hit.fire_at.map(|f| f.saturating_sub(hit.at)))] += 1;
        }
    }
}

/// [A274] `gates` as of `now`: taken by the bot's think within a tick or two of it they are this tick's, else none (`Stage::Outer`:
/// the reflex did not aim).
fn fresh(gates: Gates, now: u32) -> Gates {
    if now.saturating_sub(gates.at) <= 2 * TICK_MS && gates.at <= now + TICK_MS {
        Gates { at: now, ..gates }
    } else {
        Gates { at: u32::MAX, ..gates }
    }
}

/// The band of the view `yaw` at `from` off the bearing to `to`.
fn band(yaw: f32, from: Vec3, to: Vec3) -> usize {
    let bearing = kin::yaw_of([to.x - from.x, to.z - from.z]);
    let off = ((yaw - bearing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI).abs().to_degrees();
    BANDS_DEG.iter().position(|&b| off <= b).unwrap_or(BANDS_DEG.len())
}

/// A first hit of a life, followed to the death or [`FOLLOW_MS`]: when each answer first came (none: not yet).
#[derive(Clone, Copy, Debug)]
struct Open {
    at: u32,
    shooter: Place,
    play: u16,
    band: usize,
    /// When its own sight of him began, if it had begun at the hit.
    seen_before: Option<u32>,
    /// When its own sight of him first began after the hit.
    seen_after: Option<u32>,
    target_at: Option<u32>,
    fire_at: Option<u32>,
    fire_him_at: Option<u32>,
    /// The first other play, and when.
    switch: Option<(u32, u16)>,
    /// [A274] Its ticks by [`Stage`] before the hit (from its own first sight of him) and after it (to the first shot at
    /// him); the stage of the last tick counted; the turn's view degrees and ticks.
    before: [u32; STAGES],
    after: [u32; STAGES],
    last_stage: Option<Stage>,
    turn_deg: f32,
    turn_ticks: u32,
    /// [A274] It pressed fire in the [`FIRING_MS`] before the hit (the live tables' split).
    firing_before: bool,
    /// [A274] It had shot at him in its sight before the hit ([`Kind::Engaged`]); its ticks firing at another after the hit (to
    /// the first shot at him).
    engaged: bool,
    fired_other_ticks: u32,
}

/// The first hits of one play (the play at the hit), by the stand-in human's or a bot's.
#[derive(Clone, Debug)]
struct ByPlay {
    hits: u64,
    by_band: [u64; BANDS_DEG.len() + 1],
    seen_before: u64,
    died_1s: u64,
    died: u64,
    /// Died in the window in the play it was hit in.
    died_same_play: u64,
    targeted: u64,
    fired: u64,
    fired_him: u64,
    switched: u64,
    switched_to: BTreeMap<u16, u64>,
    /// From the hit: to the reflex's target on him, to a fire key, to a shot at him, to another play; from its own first sight
    /// of him (before the hit or after) to a shot at him.
    to_target: [u64; BINS],
    to_fire: [u64; BINS],
    to_fire_him: [u64; BINS],
    to_switch: [u64; BINS],
    sight_to_fire_him: [u64; BINS],
}

impl Default for ByPlay {
    fn default() -> Self {
        Self {
            hits: 0,
            by_band: [0; BANDS_DEG.len() + 1],
            seen_before: 0,
            died_1s: 0,
            died: 0,
            died_same_play: 0,
            targeted: 0,
            fired: 0,
            fired_him: 0,
            switched: 0,
            switched_to: BTreeMap::new(),
            to_target: [0; BINS],
            to_fire: [0; BINS],
            to_fire_him: [0; BINS],
            to_switch: [0; BINS],
            sight_to_fire_him: [0; BINS],
        }
    }
}

impl ByPlay {
    fn add(&mut self, hit: &Open, died_at: Option<u32>) {
        let after = |t: Option<u32>| t.map(|t| t.saturating_sub(hit.at));
        self.hits += 1;
        self.by_band[hit.band] += 1;
        self.seen_before += u64::from(hit.seen_before.is_some());
        self.died_1s += u64::from(died_at.is_some_and(|t| t.saturating_sub(hit.at) <= QUICK_MS));
        self.died += u64::from(died_at.is_some());
        self.died_same_play += u64::from(died_at.is_some() && hit.switch.is_none());
        self.targeted += u64::from(hit.target_at.is_some());
        self.fired += u64::from(hit.fire_at.is_some());
        self.fired_him += u64::from(hit.fire_him_at.is_some());
        if let Some((_, to)) = hit.switch {
            self.switched += 1;
            *self.switched_to.entry(to).or_default() += 1;
        }
        self.to_target[bin(after(hit.target_at))] += 1;
        self.to_fire[bin(after(hit.fire_at))] += 1;
        self.to_fire_him[bin(after(hit.fire_him_at))] += 1;
        self.to_switch[bin(after(hit.switch.map(|s| s.0)))] += 1;
        let seen = hit.seen_before.or(hit.seen_after);
        self.sight_to_fire_him[bin(seen.zip(hit.fire_him_at).map(|(s, f)| f.saturating_sub(s)))] += 1;
    }
}

/// The deaths of one play (the play at the death): fired in the last [`LAST_MS`], the life's first hit within [`QUICK_MS`].
#[derive(Clone, Copy, Debug, Default)]
struct Deaths {
    deaths: u64,
    fired_5s: u64,
    first_hit_1s: u64,
}

#[derive(Default)]
pub(super) struct UnderFireCounts {
    human: Option<Place>,
    /// Each place's reflex target, its last fire gates and its own sights (who, since, noticing delay), at this tick's read;
    /// [A274] its last [`BEFORE_MS`] of ticks (tick, target, gates, fire key).
    target: Vec<Option<Place>>,
    gates: Vec<Gates>,
    sights: Vec<Vec<(Place, u32, u32)>>,
    log: Vec<VecDeque<Logged>>,
    /// The open first hit of each place's life; whether it has had one; its last fire key; its life's first hit.
    open: Vec<Option<Open>>,
    hit_this_life: Vec<bool>,
    last_fire: Vec<Option<u32>>,
    /// [A274] Its fire key before the last one's tick.
    prior_fire: Vec<Option<u32>>,
    first_hit: Vec<Option<u32>>,
    hits_seen: usize,
    kills_seen: usize,
    /// By the stand-in human's ([1]) or a bot's ([0]) hit, the play at the hit.
    by_play: [BTreeMap<u16, ByPlay>; 2],
    /// By the stand-in human's ([1]) or a bot's ([0]) kill, the play at the death.
    deaths: [BTreeMap<u16, Deaths>; 2],
    /// [A274] By the stand-in human's ([1]) or a bot's ([0]) hit, the episodes' stages.
    stages: [StageCounts; 2],
}

impl UnderFireCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            target: vec![None; n],
            gates: vec![Gates::default(); n],
            sights: vec![Vec::new(); n],
            log: vec![VecDeque::new(); n],
            open: vec![None; n],
            hit_this_life: vec![false; n],
            last_fire: vec![None; n],
            prior_fire: vec![None; n],
            first_hit: vec![None; n],
            ..Self::default()
        }
    }

    pub(super) fn set_human(&mut self, human: Option<Place>) {
        self.human = human;
    }

    fn grow(&mut self, n: usize) {
        if self.open.len() < n {
            self.target.resize(n, None);
            self.gates.resize(n, Gates::default());
            self.sights.resize(n, Vec::new());
            self.log.resize(n, VecDeque::new());
            self.open.resize(n, None);
            self.hit_this_life.resize(n, false);
            self.last_fire.resize(n, None);
            self.prior_fire.resize(n, None);
            self.first_hit.resize(n, None);
        }
    }

    fn close(&mut self, place: Place, died_at: Option<u32>) {
        let Some(hit) = self.open.get_mut(place).and_then(Option::take) else {
            return;
        };
        let by_human = usize::from(self.human == Some(hit.shooter));
        self.by_play[by_human].entry(hit.play).or_default().add(&hit, died_at);
        self.stages[by_human].add(&hit, died_at.is_some());
    }

    /// Whether `place`'s own sight of `who` is up now, and since when.
    fn sight_of(&self, place: Place, who: Place) -> Option<u32> {
        self.sight_notice(place, who).map(|(since, _)| since)
    }

    /// The same with its noticing delay.
    fn sight_notice(&self, place: Place, who: Place) -> Option<(u32, u32)> {
        self.sights.get(place)?.iter().find(|(w, _, _)| *w == who).map(|&(_, since, notice)| (since, notice))
    }
}

impl Topic for UnderFireCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        self.grow(place + 1);
        self.target[place] = bot.and_then(|b| b.reflex.target());
        self.gates[place] = bot.map(|b| b.reflex.gates()).unwrap_or_default();
        let sights = &mut self.sights[place];
        sights.clear();
        if let Some(bot) = bot {
            sights.extend(
                bot.memory
                    .tracks()
                    .iter()
                    .filter(|t| t.source == Source::Seen)
                    .filter_map(|t| t.sight_since.map(|since| (t.who, since, t.notice_ms))),
            );
        }
    }

    fn body(&mut self, run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        let (rec, place, tick) = (at.rec, at.place, at.tick);
        self.grow(place + 1);
        if rec.alive && !at.prev.alive {
            self.hit_this_life[place] = false;
            self.first_hit[place] = None;
            self.last_fire[place] = None;
            self.prior_fire[place] = None;
        }
        let firing = rec.keys & key::FIRE != 0;
        let log = &mut self.log[place];
        log.push_back((tick, self.target[place], self.gates[place], rec.alive && firing));
        while log.front().is_some_and(|&(t, _, _, _)| tick.saturating_sub(t) > BEFORE_MS) {
            log.pop_front();
        }
        if !rec.alive || !run.is_bot.get(place).copied().unwrap_or(false) || self.human == Some(place) {
            return;
        }
        if firing {
            if self.last_fire[place] != Some(tick) {
                self.prior_fire[place] = self.last_fire[place];
            }
            self.last_fire[place] = Some(tick);
        }
        let target = self.target[place];
        let gates = self.gates[place];
        let sight_notice = self.open[place].and_then(|h| self.sight_notice(place, h.shooter));
        let sight = sight_notice.map(|(since, _)| since);
        let by_human = self.open[place].is_some_and(|h| self.human == Some(h.shooter));
        let Some(hit) = self.open[place].as_mut() else {
            return;
        };
        if tick <= hit.at {
            return;
        }
        // [A274] The tick's stage, while no shot at him has gone yet (the shot's own tick not counted).
        if hit.fire_him_at.is_none() && !(firing && target == Some(hit.shooter)) {
            let gates = fresh(gates, tick);
            let stage = Stage::of(target, hit.shooter, sight_notice, gates, tick);
            hit.after[stage as usize] += 1;
            hit.last_stage = Some(stage);
            hit.fired_other_ticks += u32::from(firing && target.is_some());
            if stage == Stage::Turn {
                let d = ((rec.yaw - at.prev.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI).abs();
                hit.turn_deg += d.to_degrees();
                hit.turn_ticks += 1;
            }
            if target == Some(hit.shooter) && gates.at == tick && gates.target == Some(hit.shooter) {
                let counts = &mut self.stages[usize::from(by_human)];
                if counts.gate_ticks.is_empty() {
                    counts.gate_ticks = vec![0; gate::NAMES.len()];
                }
                for (i, (bit, _)) in gate::NAMES.iter().enumerate() {
                    counts.gate_ticks[i] += u64::from(gates.bits & bit != 0);
                }
            }
        }
        if hit.seen_before.is_none() && hit.seen_after.is_none() {
            hit.seen_after = sight;
        }
        if hit.target_at.is_none() && target == Some(hit.shooter) {
            hit.target_at = Some(tick);
        }
        if firing {
            hit.fire_at.get_or_insert(tick);
            if target == Some(hit.shooter) {
                hit.fire_him_at.get_or_insert(tick);
            }
        }
        if hit.switch.is_none() && rec.play != hit.play {
            hit.switch = Some((tick, rec.play));
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        self.grow(game.bodies.len());
        for &(t, shooter, victim) in &run.hits[self.hits_seen.min(run.hits.len())..] {
            let (Some(v), Some(s)) = (game.bodies.get(victim), game.bodies.get(shooter)) else {
                continue;
            };
            if v.team == s.team
                || !run.is_bot.get(victim).copied().unwrap_or(false)
                || self.human == Some(victim)
                || self.hit_this_life[victim]
            {
                continue;
            }
            let (Some(vr), Some(sr)) = (run.recs[victim].last(), run.recs[shooter].last()) else {
                continue;
            };
            self.hit_this_life[victim] = true;
            self.first_hit[victim] = Some(t);
            let firing = vr.keys & key::FIRE != 0;
            let targets = self.target[victim] == Some(shooter);
            let fired_before = if self.last_fire[victim] == Some(t) { self.prior_fire[victim] } else { self.last_fire[victim] };
            self.open[victim] = Some(Open {
                at: t,
                shooter,
                play: vr.play,
                band: band(vr.yaw, vr.feet, sr.feet),
                seen_before: self.sight_of(victim, shooter),
                seen_after: None,
                target_at: targets.then_some(t),
                fire_at: firing.then_some(t),
                fire_him_at: (firing && targets).then_some(t),
                switch: None,
                before: [0; STAGES],
                after: [0; STAGES],
                last_stage: None,
                turn_deg: 0.0,
                turn_ticks: 0,
                firing_before: fired_before.is_some_and(|f| f < t && t - f <= FIRING_MS),
                engaged: false,
                fired_other_ticks: 0,
            });
            // [A274] Before the hit: its logged ticks from its own first sight of him (the sight unbroken since then) to the hit, or
            // to its first shot at him before it (then the fight was under way: `Kind::Engaged`).
            if let Some(sight) = self.sight_notice(victim, shooter) {
                let mut before = Vec::new();
                let mut engaged = false;
                for &(lt, target, gates, firing) in self.log[victim].iter().filter(|e| e.0 >= sight.0 && e.0 < t) {
                    if firing && target == Some(shooter) {
                        engaged = true;
                        break;
                    }
                    before.push(Stage::of(target, shooter, Some(sight), fresh(gates, lt), lt));
                }
                if let Some(open) = self.open[victim].as_mut() {
                    for stage in before {
                        open.before[stage as usize] += 1;
                    }
                    open.engaged = engaged;
                }
            }
        }
        self.hits_seen = run.hits.len();
        let deaths: Vec<(u32, Place, Place)> = run.kills[self.kills_seen.min(run.kills.len())..].to_vec();
        self.kills_seen = run.kills.len();
        for (t, killer, victim) in deaths {
            if run.is_bot.get(victim).copied().unwrap_or(false) && self.human != Some(victim) {
                let play = run.recs[victim].last().map_or(0, |r| r.play);
                let by_human = usize::from(self.human == Some(killer));
                let d = self.deaths[by_human].entry(play).or_default();
                d.deaths += 1;
                d.fired_5s += u64::from(self.last_fire[victim].is_some_and(|f| t.saturating_sub(f) <= LAST_MS));
                d.first_hit_1s += u64::from(self.first_hit[victim].is_some_and(|h| t.saturating_sub(h) <= QUICK_MS));
            }
            self.close(victim, Some(t));
        }
        let now = game.tick;
        let stale: Vec<Place> =
            (0..self.open.len()).filter(|&p| self.open[p].is_some_and(|h| now.saturating_sub(h.at) > FOLLOW_MS)).collect();
        for place in stale {
            self.close(place, None);
        }
    }

    fn parts(&mut self, run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let open: Vec<Place> = (0..self.open.len()).filter(|&p| self.open[p].is_some()).collect();
        for place in open {
            self.close(place, None);
        }
        let name = |play: u16| run.plays.get(usize::from(play)).cloned().unwrap_or_else(|| "none".to_owned());
        let mut lines = vec![format!(
            "owner-test under fire (A230: the first enemy hit of a bot's life by the play at the hit, followed to its death or \
             {FOLLOW_MS} ms: view bands up to {BANDS_DEG:?} deg; times in {BIN_MS} ms bins, the last never; each death by its play: \
             fired in the last {LAST_MS} ms, its first hit within {QUICK_MS} ms; the stand-in human's apart)"
        )];
        let mut json = serde_json::Map::new();
        let by_stage = |a: &[u64; STAGES]| -> serde_json::Map<String, serde_json::Value> {
            Stage::ALL.iter().zip(a).filter(|(_, n)| **n > 0).map(|(s, n)| (s.name().to_owned(), serde_json::Value::from(*n))).collect()
        };
        for (who, ((by_play, deaths), st)) in
            ["by_bot", "by_human"].iter().zip(self.by_play.iter().zip(&self.deaths).zip(&self.stages))
        {
            let mut hits = serde_json::Map::new();
            for (play, c) in by_play {
                let to: serde_json::Map<String, serde_json::Value> =
                    c.switched_to.iter().map(|(p, n)| (name(*p), serde_json::Value::from(*n))).collect();
                hits.insert(
                    name(*play),
                    serde_json::json!({
                        "hits": c.hits, "by_band": c.by_band, "seen_before": c.seen_before, "died_1s": c.died_1s, "died": c.died,
                        "died_same_play": c.died_same_play, "targeted": c.targeted, "fired": c.fired, "fired_him": c.fired_him,
                        "switched": c.switched, "switched_to": to, "to_target": c.to_target.to_vec(), "to_fire": c.to_fire.to_vec(),
                        "to_fire_him": c.to_fire_him.to_vec(), "to_switch": c.to_switch.to_vec(),
                        "sight_to_fire_him": c.sight_to_fire_him.to_vec(),
                    }),
                );
                lines.push(format!(
                    "  {who} {}: first hits {} {:?}, seen before {}, died within 1 s {}, died {} ({} in that play), targeted him {}, \
                     fired {}, at him {}, switched {}",
                    name(*play),
                    c.hits,
                    c.by_band,
                    c.seen_before,
                    c.died_1s,
                    c.died,
                    c.died_same_play,
                    c.targeted,
                    c.fired,
                    c.fired_him,
                    c.switched
                ));
            }
            let dead: serde_json::Map<String, serde_json::Value> = deaths
                .iter()
                .map(|(play, d)| (name(*play), serde_json::json!({"deaths": d.deaths, "fired_5s": d.fired_5s, "first_hit_1s": d.first_hit_1s})))
                .collect();
            lines.push(format!(
                "  {who} deaths by play: {}",
                deaths.iter().map(|(p, d)| format!("{} {} (fired 5 s {}, hit 1 s {})", name(*p), d.deaths, d.fired_5s, d.first_hit_1s)).collect::<Vec<_>>().join(", ")
            ));
            let gate_ticks: serde_json::Map<String, serde_json::Value> = gate::NAMES
                .iter()
                .zip(&st.gate_ticks)
                .filter(|(_, n)| **n > 0)
                .map(|((_, name), n)| ((*name).to_owned(), serde_json::Value::from(*n)))
                .collect();
            let top = |a: &[u64; STAGES]| {
                let mut v: Vec<(Stage, u64)> = Stage::ALL.iter().copied().zip(a.iter().copied()).filter(|(_, n)| *n > 0).collect();
                v.sort_by_key(|&(s, n)| (std::cmp::Reverse(n), s as usize));
                v.iter().map(|(s, n)| format!("{} {n}", s.name())).collect::<Vec<_>>().join(", ")
            };
            let mut kinds = serde_json::Map::new();
            for (kind, c) in Kind::ALL.iter().zip(&st.kinds) {
                kinds.insert(
                    kind.name().to_owned(),
                    serde_json::json!({
                        "episodes": c.episodes, "before": by_stage(&c.before), "after": by_stage(&c.after),
                        "ended_in": by_stage(&c.ended_in), "ended_dead": by_stage(&c.ended_dead),
                        "turn_deg": (c.turn_deg * 1000.0).round() / 1000.0, "turn_ticks": c.turn_ticks, "seen": c.seen,
                        "targeted": c.targeted, "fired_other_ticks": c.fired_other_ticks, "sight_to_shot": c.sight_to_shot.to_vec(),
                        "hit_to_shot": c.hit_to_shot.to_vec(),
                    }),
                );
                lines.push(format!(
                    "  {who} stages (A274) {}: episodes {}, seen {}, targeted {}; ticks before the hit: {}; after: {}; ended in: {}; \
                     turn {:.1} deg over {} ticks",
                    kind.name(),
                    c.episodes,
                    c.seen,
                    c.targeted,
                    top(&c.before),
                    top(&c.after),
                    top(&c.ended_in),
                    c.turn_deg,
                    c.turn_ticks
                ));
            }
            let stages = serde_json::json!({
                "kinds": kinds, "gate_ticks": gate_ticks,
                "fresh": {"episodes": st.fresh, "fired": st.fresh_fired, "died_first": st.fresh_died_first,
                    "to_fire": st.fresh_to_fire},
            });
            json.insert((*who).to_owned(), serde_json::json!({ "first_hits": hits, "deaths": dead, "stages": stages }));
        }
        vec![("under_fire", Part::new(serde_json::json!({ "under_fire": json }), lines.join("\n")))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_falls_in_its_bin_and_never_in_the_last() {
        assert_eq!([bin(Some(0)), bin(Some(249)), bin(Some(250)), bin(Some(9_999))], [0, 0, 1, BINS - 2]);
        assert_eq!(bin(Some(FOLLOW_MS + 5_000)), BINS - 2, "late but in the window's last bin");
        assert_eq!(bin(None), BINS - 1);
    }

    #[test]
    fn a_first_hit_counts_its_answers_from_the_hit_and_its_death_in_the_same_play() {
        let hit = Open {
            at: 1_000,
            shooter: 7,
            play: 2,
            band: 1,
            seen_before: Some(400),
            seen_after: None,
            target_at: Some(1_300),
            fire_at: Some(1_600),
            fire_him_at: Some(1_600),
            switch: None,
            before: [0; STAGES],
            after: [0; STAGES],
            last_stage: None,
            turn_deg: 0.0,
            turn_ticks: 0,
            firing_before: false,
            engaged: false,
            fired_other_ticks: 0,
        };
        let mut c = ByPlay::default();
        c.add(&hit, Some(1_800));
        assert_eq!((c.hits, c.died_1s, c.died, c.died_same_play, c.fired_him, c.switched), (1, 1, 1, 1, 1, 0));
        assert_eq!(c.to_target[1], 1, "300 ms after the hit");
        assert_eq!(c.to_fire_him[2], 1, "600 ms after the hit");
        assert_eq!(c.sight_to_fire_him[4], 1, "1.2 s from the first sight");
        assert_eq!(c.to_switch[BINS - 1], 1, "never switched");
        let switched = Open { switch: Some((3_500, 5)), seen_before: None, ..hit };
        c.add(&switched, None);
        assert_eq!((c.hits, c.died, c.died_same_play, c.switched, c.switched_to.get(&5)), (2, 1, 1, 1, Some(&1)));
        assert_eq!(c.to_switch[10], 1, "2.5 s after the hit");
        // [A274] The episode's kind and its times: fired back 600 ms after the hit, 1.2 s after its own sight.
        let mut st = StageCounts::default();
        st.add(&hit, true);
        st.add(&Open { engaged: true, ..hit }, false);
        st.add(&Open { fire_him_at: None, last_stage: Some(Stage::OtherTarget), ..hit }, true);
        let [engaged, back, never] = &st.kinds;
        assert_eq!((engaged.episodes, back.episodes, never.episodes), (1, 1, 1));
        assert_eq!((back.hit_to_shot[2], back.sight_to_shot[4]), (1, 1));
        assert_eq!((never.ended_in[Stage::OtherTarget as usize], never.ended_dead[Stage::OtherTarget as usize]), (1, 1));
        assert_eq!(never.hit_to_shot[BINS - 1], 1, "never shot at him");
        assert_eq!((st.fresh, st.fresh_fired), (3, 3), "none fired in the second before the hit");
    }

    #[test]
    fn a_tick_takes_the_first_stage_of_the_fire_chain_that_holds_the_shot() {
        use crate::v026g::bots::reflex::gate as g;
        let now = 1_000;
        let on = |bits| Gates { at: now, bits, target: Some(7) };
        let sight = Some((now - 600, 200));
        assert_eq!(Stage::of(None, 7, None, on(0), now), Stage::Unseen);
        assert_eq!(Stage::of(None, 7, Some((now - 100, 200)), on(0), now), Stage::Noticing);
        assert_eq!(Stage::of(None, 7, sight, on(0), now), Stage::NotTaken);
        assert_eq!(Stage::of(Some(3), 7, sight, on(0), now), Stage::OtherTarget);
        assert_eq!(Stage::of(Some(7), 7, sight, fresh(on(0), now + 3 * TICK_MS), now + 3 * TICK_MS), Stage::Outer, "stale gates");
        assert_eq!(Stage::of(Some(7), 7, sight, Gates { target: Some(3), ..on(0) }, now), Stage::Outer, "gates on another");
        assert_eq!(Stage::of(Some(7), 7, sight, on(g::PLAY | g::WITHIN), now), Stage::Outer);
        assert_eq!(Stage::of(Some(7), 7, sight, on(g::WITHIN | g::FRIEND), now), Stage::Turn);
        assert_eq!(Stage::of(Some(7), 7, sight, on(g::FRIEND | g::HIDDEN), now), Stage::Friend);
        assert_eq!(Stage::of(Some(7), 7, sight, on(g::AIM_RAY | g::TRIGGER), now), Stage::ClearShot);
        assert_eq!(Stage::of(Some(7), 7, sight, on(g::TRIGGER), now), Stage::Cycle);
        assert_eq!(Stage::of(Some(7), 7, sight, fresh(on(0), now + TICK_MS), now + TICK_MS), Stage::Open, "a tick-old think");
    }

    #[test]
    fn the_view_band_off_the_shooter_wraps_and_ignores_height() {
        let at = Vec3::new(0.0, 0.0, 0.0);
        let yaw = kin::yaw_of([0.0, 1.0]);
        assert_eq!(band(yaw, at, Vec3::new(0.0, 4.0, 10.0)), 0);
        assert_eq!(band(yaw, at, Vec3::new(10.0, 0.0, 0.0)), 2);
        assert_eq!(band(yaw, at, Vec3::new(0.0, 0.0, -10.0)), 3);
    }
}
