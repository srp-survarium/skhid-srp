//! Which anomaly zones are on, and when they switch, as the clients'
//! simulation has them (the research of 2026-10-14 on the 0.26g client,
//! kept private; addresses are the 0.26g exe's).
//!
//! Every `generic_anomaly_core` of a map ([`mapdata::Anomaly`]) is run here
//! on the match clock, tick for tick, with the client's own rules
//! (`generic_anomaly_core::tick` 0x31B5B0), so a server-driven bot knows
//! which damage zones hurt now and which will soon:
//!
//! - **Random stream.** Each core has its own LCG `x = x·0x08088405 + 1`
//!   (core+0x1B8), seeded with `static_seed ^ crc32(full_name)`, the
//!   static seed being the match's static info id
//!   (`registry::static_id(match_id)` = `match_id | 0x80000000`). An index
//!   among `n` is `(x·n) >> 32`, a duration `(x·span) >> 32`, a fraction
//!   `((x·2^20) >> 32) / 2^20`. PROVEN (code and 36 matches of worlds).
//! - **Ticks** on the 50 ms grid of the match clock (worlds show rare
//!   off-grid ticks, 10-50 ms off; the sync below corrects them). Per tick:
//!   energy falls, a state is picked, its groups run (a sequential group
//!   switches to its other zone when its time comes, the next time drawn
//!   40-60 s on), then artefacts spawn (their draws come from the same
//!   stream). PROVEN.
//! - **Timer cores** (level_03's three electric anomalies: two zones each,
//!   strictly alternating): exact from the seed and the clock for a whole
//!   match, but a core with artefacts draws again when a player picks one
//!   up, which the server does not see: its periods after that drift, never
//!   the alternation. [`Anomalies::sync_from_world`] takes each core's
//!   exact state (its random value, zone mask, next switch) from every
//!   checked world, so the drift lasts until the next world.
//! - **Energy cores** (level_02's fruit cloud): the state follows the
//!   energy, which players inside raise; the server sees only the worlds.
//!   Between worlds the energy falls at its speed (25 a second). Bots keep
//!   out of such a core's low states' zones always (walking in raises it)
//!   and follow its high states by the decay (`hazards`).
//!
//! What a world holds of a core (objects section, `generic_anomaly_core::
//! serialize` 0x31AF20): `[artefacts: u8 pending, u32 next if pending]
//! f32 energy, u8 state (0xFF none; states sorted by threshold, highest
//! first), [u32 finish, per group: (not keep-all: u32 next, mask bytes),
//! 5 bytes per active zone], u8 events, u32 random`. Offsets move with the
//! map's other objects, so a core is found by its random value: the last
//! known one stepped until it appears in the section (unique), then read
//! backwards.

use mapdata::MapData;

/// The anomaly tick, ms.
pub const TICK_MS: u32 = 50;
/// How far a schedule is predicted, ms.
pub const PREDICT_MS: u32 = 300_000;
/// A prediction is made again this long after the last at the latest.
const REPREDICT_MS: u32 = 30_000;
/// Spans that ended this recently are kept in a new prediction, ms.
const RECENT_MS: u32 = 5_000;
/// Draws looked through for a core's random value in a world.
const MAX_DRAWS: usize = 4096;

const LCG_MUL: u32 = 0x0808_8405;

/// The cores' random step (0x31BF18).
pub fn lcg(x: u32) -> u32 {
    x.wrapping_mul(LCG_MUL).wrapping_add(1)
}

/// A core's first random value: the static seed XOR the CRC-32 of its
/// `full_name` (`generic_anomaly_core_cook` 0x2EA1C0..0x2EA264).
pub fn core_seed(static_seed: u32, full_name: &str) -> u32 {
    static_seed ^ crc32fast::hash(full_name.as_bytes())
}

/// An index among `n` from a drawn value.
fn pick(x: u32, n: u32) -> u32 {
    ((u64::from(x) * u64::from(n)) >> 32) as u32
}

/// A fraction in [0, 1) from a drawn value.
fn unit(x: u32) -> f32 {
    ((u64::from(x) * 0x10_0000) >> 32) as f32 / 1_048_576.0
}

/// `round((base + variation × (2u - 1)) × 1000)` ms, u from `x`.
fn delay_ms(x: u32, base_s: f32, variation_s: f32) -> u32 {
    ((base_s + variation_s * (2.0 * unit(x) - 1.0)) * 1000.0)
        .round()
        .max(0.0) as u32
}

fn grid_ceil(ms: u32) -> u32 {
    ms.div_ceil(TICK_MS).saturating_mul(TICK_MS)
}

/// How a damage zone's activity is decided.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Control {
    /// No core switches it: on all match.
    Static,
    /// A timer group switches it (level_03's electric pairs).
    Timer,
    /// A core's energy does: on in states of at least `threshold`, and the
    /// energy rises up to `per_second` a second while a body moves in the
    /// core's zones.
    Energy { threshold: f32, per_second: f32 },
}

/// The on spans of a zone predicted from a time on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schedule {
    /// The prediction's start (the clock when it was made).
    pub from: u32,
    /// On in `[start, end)`, ms, in order.
    pub spans: Vec<(u32, u32)>,
    /// Unknown from here on.
    pub until: u32,
}

impl Schedule {
    /// On at `time`: `None` past [`Self::until`]. Times before
    /// [`Self::from`] are answered as at `from`.
    pub fn on(&self, time: u32) -> Option<bool> {
        let time = time.max(self.from);
        (time < self.until).then(|| {
            self.spans
                .iter()
                .any(|&(from, to)| (from..to).contains(&time))
        })
    }
}

#[derive(Clone, Debug, Default)]
struct Group {
    enabled: bool,
    keep_all: bool,
    sequential: bool,
    min_ms: u32,
    max_ms: u32,
    max_charged: u32,
    recharge_ms: u32,
    /// Map zone index of each config zone (`None`: no such zone).
    zones: Vec<Option<usize>>,
    on: Vec<bool>,
    /// `m_next_recharge_time`, 0 none.
    next: u32,
}

impl Group {
    /// Bytes the world writes for it, for at most all zones on.
    fn max_bytes(&self) -> usize {
        let n = self.zones.len();
        if self.keep_all {
            5 * n
        } else {
            4 + n.div_ceil(8) + 5 * n
        }
    }
}

#[derive(Clone, Debug, Default)]
struct State {
    enabled: bool,
    /// A trigger (shot, zone act) is needed: the server never sees one.
    triggered: bool,
    threshold: f32,
    active_ms: u32,
    energy_on_exit: f32,
    groups: Vec<Group>,
}

#[derive(Clone, Debug, Default)]
struct Artefacts {
    enabled: bool,
    pending: bool,
    next: u32,
    respawn_s: f32,
    variation_s: f32,
    chance: f32,
    max: u32,
    spawned: u32,
    /// One may lie in the world: a pickup (unseen) re-arms and draws.
    unseen: bool,
}

/// One core's simulated state and settings.
#[derive(Clone, Debug, Default)]
struct Sim {
    rng: u32,
    energy_enabled: bool,
    decrease: f32,
    energy: f32,
    /// Sorted as the client sorts them: highest threshold first.
    states: Vec<State>,
    current: Option<usize>,
    finish: u32,
    art: Artefacts,
    /// The last tick run (0: none yet; the first is at 50 ms).
    tick: u32,
    /// Counts zone switches and state changes.
    changes: u64,
}

impl Sim {
    /// Whether no tick can change anything until [`Self::next_event`].
    fn idle(&self) -> bool {
        self.current.is_some() && !(self.energy_enabled && self.decrease > 0.0 && self.energy > 0.0)
    }

    fn next_event(&self) -> u32 {
        let mut at = u32::MAX;
        if let Some(state) = self.current.map(|c| &self.states[c]) {
            for group in &state.groups {
                if group.enabled && !group.keep_all && group.next != 0 {
                    at = at.min(group.next);
                }
            }
        }
        if self.finish != 0 {
            at = at.min(self.finish);
        }
        if self.art.enabled && self.art.pending {
            at = at.min(self.art.next);
        }
        at
    }

    /// Runs every tick up to `to`.
    fn advance(&mut self, to: u32) {
        loop {
            if self.tick.saturating_add(TICK_MS) > to {
                return;
            }
            if self.idle() {
                let at = grid_ceil(self.next_event()).max(self.tick + TICK_MS);
                if at > to {
                    self.tick = self.tick.max(to / TICK_MS * TICK_MS);
                    return;
                }
                self.tick = at - TICK_MS;
            }
            let t = self.tick + TICK_MS;
            self.step(t);
            self.tick = t;
        }
    }

    /// Runs ticks up to `to` until one changes a zone or the state:
    /// that tick's time, or `None` (then the clock stands at `to`).
    fn advance_to_change(&mut self, to: u32) -> Option<u32> {
        let changes = self.changes;
        loop {
            if self.tick.saturating_add(TICK_MS) > to {
                return None;
            }
            if self.idle() {
                let at = grid_ceil(self.next_event()).max(self.tick + TICK_MS);
                if at > to {
                    self.tick = self.tick.max(to / TICK_MS * TICK_MS);
                    return None;
                }
                self.tick = at - TICK_MS;
            }
            let t = self.tick + TICK_MS;
            self.step(t);
            self.tick = t;
            if self.changes != changes {
                return Some(t);
            }
        }
    }

    /// One tick (0x31B5B0): energy, state, groups, artefacts.
    fn step(&mut self, t: u32) {
        if self.energy_enabled {
            self.energy = (self.energy - self.decrease * TICK_MS as f32 / 1000.0).max(0.0);
        }
        // `select_state` 0x31BA90: a state with a finish time stays.
        if !(self.current.is_some() && self.finish != 0) && !self.states.is_empty() {
            let pick = self
                .states
                .iter()
                .position(|s| s.enabled && !s.triggered && self.energy >= s.threshold)
                .unwrap_or(0);
            if self.current != Some(pick) {
                if let Some(old) = self.current {
                    for group in &mut self.states[old].groups {
                        group.on.iter_mut().for_each(|on| *on = false);
                        group.next = 0;
                    }
                }
                self.current = Some(pick);
                self.changes += 1;
                // `anomaly_state::initialize` 0x31BC10.
                for g in 0..self.states[pick].groups.len() {
                    let group = &mut self.states[pick].groups[g];
                    if !group.enabled {
                        continue;
                    }
                    if group.keep_all {
                        group.on.iter_mut().for_each(|on| *on = true);
                        group.next = 0;
                    } else {
                        // Time 0, not now (0x31BC61).
                        self.recharge(pick, g, 0);
                    }
                }
                let active_ms = self.states[pick].active_ms;
                self.finish = if active_ms > 0 { t + active_ms } else { 0 };
            }
        }
        // `anomaly_state::execute` 0x31BCB0.
        if let Some(current) = self.current {
            for g in 0..self.states[current].groups.len() {
                let group = &self.states[current].groups[g];
                if group.enabled && !group.keep_all && group.next != 0 && t >= group.next {
                    self.recharge(current, g, t);
                }
            }
            if self.finish != 0 && t >= self.finish {
                self.energy = self.states[current].energy_on_exit;
                self.finish = 0;
            }
        }
        if self.art.enabled && self.art.pending && t >= self.art.next {
            self.spawn_artefact(t);
        }
    }

    /// `zone_group::recharge` 0x31BE60.
    fn recharge(&mut self, state: usize, g: usize, t: u32) {
        let group = &mut self.states[state].groups[g];
        let n = group.on.len();
        let inactive: Vec<usize> = (0..n).filter(|&i| !group.on[i]).collect();
        let active = (0..n).rev().find(|&i| group.on[i]);
        if !group.sequential && (n - inactive.len()) as u32 == group.max_charged {
            group.next = 0;
            return;
        }
        if inactive.is_empty() {
            // Never in the shipped maps (GUESS: the client would index
            // past its list).
            group.next = 0;
            return;
        }
        self.rng = lcg(self.rng);
        let chosen = inactive[pick(self.rng, inactive.len() as u32) as usize];
        if group.sequential
            && let Some(active) = active
        {
            group.on[active] = false;
        }
        group.on[chosen] = true;
        if group.sequential {
            self.rng = lcg(self.rng);
            let span = group.max_ms.saturating_sub(group.min_ms);
            group.next = t
                .saturating_add(group.min_ms)
                .saturating_add(pick(self.rng, span));
        } else {
            let on = group.on.iter().filter(|&&on| on).count() as u32;
            group.next = if on == group.max_charged {
                0
            } else {
                t.saturating_add(group.recharge_ms)
            };
        }
        self.changes += 1;
    }

    /// `spawn_artefact` 0x31B6C0, every container taken as free.
    fn spawn_artefact(&mut self, t: u32) {
        let art = &mut self.art;
        self.rng = lcg(self.rng);
        if unit(self.rng) >= art.chance {
            self.rng = lcg(self.rng);
            art.next = t.saturating_add(delay_ms(self.rng, art.respawn_s, art.variation_s));
            return;
        }
        art.spawned += 1;
        art.unseen = true;
        // Not a zone, but what may be predicted changes.
        self.changes += 1;
        if art.spawned >= art.max {
            art.pending = false;
        }
        // The container, the artefact, the next delay.
        self.rng = lcg(lcg(lcg(self.rng)));
        art.next = t.saturating_add(delay_ms(self.rng, art.respawn_s, art.variation_s));
    }

    /// Whether every draw to come is known (no artefact a player could
    /// pick up unseen).
    fn exact(&self) -> bool {
        !self.art.enabled || !self.art.unseen
    }

    fn zone_on(&self, slot: &Slot) -> bool {
        self.current == Some(slot.state)
            && self.states[slot.state].groups[slot.group].on[slot.index]
    }
}

/// Where a map zone sits in a core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Slot {
    core: usize,
    state: usize,
    group: usize,
    index: usize,
}

/// One anomaly core.
#[derive(Clone, Debug, Default)]
pub struct Core {
    /// Its `full_name`.
    pub name: String,
    sim: Sim,
    /// The state the last checked world gave (or the start): where the
    /// next world's search begins.
    base: Sim,
    synced_at: Option<u32>,
    /// Energy one body adds a second at most.
    per_second: f32,
}

impl Core {
    pub fn energy(&self) -> f32 {
        self.sim.energy
    }

    /// The current state as the worlds number it (sorted, highest
    /// threshold first); `None` before the first tick.
    pub fn state(&self) -> Option<usize> {
        self.sim.current
    }

    /// The current state's energy threshold.
    pub fn threshold(&self) -> Option<f32> {
        self.sim.current.map(|c| self.sim.states[c].threshold)
    }

    /// The random value now.
    pub fn random(&self) -> u32 {
        self.sim.rng
    }

    /// When a checked world last set it.
    pub fn synced_at(&self) -> Option<u32> {
        self.synced_at
    }

    /// Whether its future draws are all known.
    pub fn exact(&self) -> bool {
        self.sim.exact()
    }

    /// Whether the artefact spawn is pending, and when.
    pub fn artefact(&self) -> Option<(bool, u32)> {
        self.sim
            .art
            .enabled
            .then_some((self.sim.art.pending, self.sim.art.next))
    }
}

/// What a world said of a core.
#[derive(Clone, Debug, PartialEq)]
pub struct Synced {
    pub core: usize,
    /// Where its random value lies in the section.
    pub offset: usize,
    /// Draws since the last world.
    pub draws: usize,
    /// Whether the simulation had the same zones on and the same next
    /// switch times (within a tick) at the world's time.
    pub matched: bool,
}

/// What came of a world: the cores found, and those not found (or found
/// at more than one place).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SyncReport {
    pub synced: Vec<Synced>,
    pub missed: Vec<usize>,
}

/// A core as read from a world.
#[derive(Clone, Debug, PartialEq)]
struct Decoded {
    state: Option<usize>,
    energy: f32,
    finish: u32,
    /// Per group of the state: next switch, zones on.
    groups: Vec<(u32, Vec<bool>)>,
    /// `Some((pending, next))` when read.
    artefact: Option<(bool, u32)>,
}

/// A map's anomaly cores on the match clock.
#[derive(Clone, Debug, Default)]
pub struct Anomalies {
    cores: Vec<Core>,
    /// By map zone: where the cores hold it.
    slots: Vec<Vec<Slot>>,
    /// By map zone: the prediction, for zones a core drives.
    schedules: Vec<Option<Schedule>>,
    now: u32,
    predicted_at: u32,
    predicted_changes: u64,
    /// Counts new predictions.
    version: u64,
}

impl Anomalies {
    /// The cores of `map`, seeded with the match's static seed
    /// (`registry::static_id(match_id)`), loaded (each draws its first
    /// artefact time) and not yet ticked.
    pub fn new(map: &MapData, static_seed: u32) -> Self {
        let mut slots: Vec<Vec<Slot>> = vec![Vec::new(); map.zones.len()];
        let mut cores = Vec::new();
        for (c, anomaly) in map.anomalies.iter().enumerate() {
            let mut states: Vec<State> = anomaly
                .states
                .iter()
                .map(|state| State {
                    enabled: state.enabled,
                    triggered: state.shoot_trigger || state.zone_activity_trigger,
                    threshold: state.threshold,
                    active_ms: state.active_time_s.saturating_mul(1000),
                    energy_on_exit: state.energy_on_exit,
                    groups: state
                        .groups
                        .iter()
                        .map(|group| Group {
                            enabled: group.enabled,
                            keep_all: group.keep_all,
                            sequential: group.sequential,
                            min_ms: group.min_active_s.saturating_mul(1000),
                            max_ms: group.max_active_s.saturating_mul(1000),
                            max_charged: group.max_charged,
                            recharge_ms: group.recharge_s.saturating_mul(1000),
                            zones: group
                                .zones
                                .iter()
                                .map(|name| map.zones.iter().position(|z| &z.name == name))
                                .collect(),
                            on: vec![false; group.zones.len()],
                            next: 0,
                        })
                        .collect(),
                })
                .collect();
            // `resolve_links`: highest threshold first (stable).
            states.sort_by(|a, b| b.threshold.total_cmp(&a.threshold));
            for (s, state) in states.iter().enumerate() {
                for (g, group) in state.groups.iter().enumerate() {
                    for (index, zone) in group.zones.iter().enumerate() {
                        if let Some(zone) = *zone {
                            slots[zone].push(Slot {
                                core: c,
                                state: s,
                                group: g,
                                index,
                            });
                        }
                    }
                }
            }
            let a = &anomaly.artefacts;
            let mut sim = Sim {
                rng: core_seed(static_seed, &anomaly.name),
                energy_enabled: anomaly.energy.enabled,
                decrease: anomaly.energy.decrease_speed,
                energy: anomaly.energy.initial,
                states,
                art: Artefacts {
                    enabled: a.enabled,
                    pending: a.enabled && a.max_count > 0,
                    respawn_s: a.respawn_s,
                    variation_s: a.variation_s,
                    chance: a.chance,
                    max: a.max_count,
                    ..Artefacts::default()
                },
                ..Sim::default()
            };
            // `load` 0x33BD50: the first artefact time, drawn for every core.
            sim.rng = lcg(sim.rng);
            sim.art.next = delay_ms(sim.rng, a.first_spawn_s, a.variation_s);
            let e = &anomaly.energy;
            cores.push(Core {
                name: anomaly.name.clone(),
                base: sim.clone(),
                sim,
                synced_at: None,
                per_second: e.on_walk.max(e.on_run).max(e.on_sprint),
            });
        }
        let mut anomalies = Self {
            cores,
            schedules: vec![None; map.zones.len()],
            slots,
            ..Self::default()
        };
        anomalies.predict();
        anomalies
    }

    pub fn cores(&self) -> &[Core] {
        &self.cores
    }

    /// The clock the cores stand at.
    pub fn now(&self) -> u32 {
        self.now
    }

    /// Counts the predictions made; a new one means the schedules changed.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The cores run to `match_ms` (the match clock, the worlds' time).
    /// The clock never goes back.
    pub fn advance(&mut self, match_ms: u32) {
        if match_ms <= self.now {
            return;
        }
        self.now = match_ms;
        for core in &mut self.cores {
            core.sim.advance(match_ms);
        }
        let changes = self.changes();
        if changes != self.predicted_changes
            || match_ms >= self.predicted_at.saturating_add(REPREDICT_MS)
        {
            self.predict();
        }
    }

    fn changes(&self) -> u64 {
        self.cores.iter().map(|core| core.sim.changes).sum()
    }

    /// Takes each core's state from a checked world's objects section
    /// (`Sections::objects`) of time `world_ms`, then runs the cores to the
    /// clock again. A world older than a core's last is passed over.
    pub fn sync_from_world(&mut self, objects: &[u8], world_ms: u32) -> SyncReport {
        if self.cores.is_empty() {
            return SyncReport::default();
        }
        let mut words: Vec<(u32, usize)> = objects
            .windows(4)
            .enumerate()
            .map(|(at, w)| (u32::from_le_bytes([w[0], w[1], w[2], w[3]]), at))
            .collect();
        words.sort_unstable();
        let mut report = SyncReport::default();
        for (c, core) in self.cores.iter_mut().enumerate() {
            if core.synced_at.is_some_and(|at| at >= world_ms) {
                continue;
            }
            // The simulation from the last world to this one.
            let mut predicted = core.base.clone();
            predicted.advance(world_ms.saturating_sub(1));
            let mut x = core.base.rng;
            let mut found: Option<(usize, usize, Decoded)> = None;
            let mut ambiguous = false;
            for draws in 0..MAX_DRAWS {
                let first = words.partition_point(|&(w, _)| w < x);
                let mut hits = Vec::new();
                for &(_, at) in words[first..].iter().take_while(|&&(w, _)| w == x) {
                    if let Some(decoded) = decode(objects, at, &predicted, world_ms) {
                        hits.push((at, decoded));
                    }
                }
                if hits.len() > 1 {
                    ambiguous = true;
                    break;
                }
                if let Some((at, decoded)) = hits.pop() {
                    found = Some((at, draws, decoded));
                    break;
                }
                x = lcg(x);
            }
            let Some((offset, draws, decoded)) = found.filter(|_| !ambiguous) else {
                report.missed.push(c);
                continue;
            };
            let matched = agrees(&predicted, &decoded);
            let mut sim = predicted;
            sim.rng = x;
            sim.energy = decoded.energy;
            if sim.current != decoded.state {
                if let Some(old) = sim.current {
                    for group in &mut sim.states[old].groups {
                        group.on.iter_mut().for_each(|on| *on = false);
                        group.next = 0;
                    }
                }
                sim.current = decoded.state;
            }
            sim.finish = decoded.finish;
            if let Some(state) = decoded.state {
                for (group, (next, on)) in sim.states[state].groups.iter_mut().zip(&decoded.groups)
                {
                    group.next = *next;
                    group.on.clone_from(on);
                }
            }
            if let Some((pending, next)) = decoded.artefact {
                sim.art.pending = pending;
                if pending {
                    sim.art.next = next;
                }
            }
            sim.changes += 1;
            // The world stands before its own tick (its first, at 50 ms,
            // is written before any).
            sim.tick = world_ms.saturating_sub(1) / TICK_MS * TICK_MS;
            core.base = sim.clone();
            sim.advance(self.now);
            core.sim = sim;
            core.synced_at = Some(world_ms);
            report.synced.push(Synced {
                core: c,
                offset,
                draws,
                matched,
            });
        }
        if !report.synced.is_empty() {
            self.predict();
        }
        report
    }

    /// Whether `zone` (an index of `MapData::zones`) is on at `match_ms`
    /// as predicted. A zone no core drives is always on; a time past the
    /// prediction counts as on.
    pub fn zone_active(&self, zone: usize, match_ms: u32) -> bool {
        match self.schedules.get(zone) {
            Some(Some(schedule)) => schedule.on(match_ms).unwrap_or(true),
            _ => true,
        }
    }

    /// Whether `zone` is on now, by the cores' state.
    pub fn zone_on_now(&self, zone: usize) -> bool {
        let slots = self.slots.get(zone).map_or(&[][..], Vec::as_slice);
        slots.is_empty()
            || slots
                .iter()
                .any(|slot| self.cores[slot.core].sim.zone_on(slot))
    }

    /// When `zone` next switches on or off (a tick time), if the
    /// prediction knows: `None` for a zone no core drives or one that
    /// stays as it is through the prediction.
    pub fn next_switch(&self, zone: usize) -> Option<u32> {
        let schedule = self.schedules.get(zone)?.as_ref()?;
        let now = self.now.max(schedule.from);
        schedule
            .spans
            .iter()
            .flat_map(|&(from, to)| [from, to])
            .find(|&t| t > now && t < schedule.until)
    }

    /// The prediction for `zone`, if a core drives it.
    pub fn schedule(&self, zone: usize) -> Option<&Schedule> {
        self.schedules.get(zone)?.as_ref()
    }

    /// How `zone`'s activity is decided.
    pub fn control(&self, zone: usize) -> Control {
        let slots = self.slots.get(zone).map_or(&[][..], Vec::as_slice);
        let Some(first) = slots.first() else {
            return Control::Static;
        };
        let core = &self.cores[first.core];
        if slots.iter().any(|slot| {
            let group = &self.cores[slot.core].sim.states[slot.state].groups[slot.group];
            !group.keep_all
        }) {
            return Control::Timer;
        }
        let states = &core.sim.states;
        if states.iter().filter(|s| s.enabled).count() <= 1 {
            return Control::Static;
        }
        let threshold = slots
            .iter()
            .map(|slot| states[slot.state].threshold)
            .fold(f32::INFINITY, f32::min);
        Control::Energy {
            threshold,
            per_second: core.per_second,
        }
    }

    /// Every zone's on spans from the clock on, by running a copy of the
    /// cores ahead. A core whose draws a pickup can change is predicted
    /// only as far as is certain: its timer groups' next switch, then at
    /// least their shortest period.
    fn predict(&mut self) {
        let now = self.now;
        let mut schedules: Vec<Option<Schedule>> = vec![None; self.slots.len()];
        let mut by_core: Vec<Vec<usize>> = vec![Vec::new(); self.cores.len()];
        for (zone, slots) in self.slots.iter().enumerate() {
            if let Some(slot) = slots.first() {
                by_core[slot.core].push(zone);
            }
        }
        for (c, core) in self.cores.iter().enumerate() {
            let zones = &by_core[c];
            if zones.is_empty() {
                continue;
            }
            let mut sim = core.sim.clone();
            let mut until = now.saturating_add(PREDICT_MS);
            if !sim.exact()
                && let Some(state) = sim.current.map(|s| &sim.states[s])
            {
                for group in state.groups.iter().filter(|g| g.enabled && !g.keep_all) {
                    let next = if group.next == 0 { now } else { group.next };
                    until = until.min(grid_ceil(next).saturating_add(group.min_ms));
                }
            }
            let on_now =
                |sim: &Sim, zone: usize| self.slots[zone].iter().any(|slot| sim.zone_on(slot));
            let mut open: Vec<Option<u32>> = zones
                .iter()
                .map(|&zone| on_now(&sim, zone).then_some(now))
                .collect();
            let mut spans: Vec<Vec<(u32, u32)>> = vec![Vec::new(); zones.len()];
            while let Some(t) = sim.advance_to_change(until.saturating_sub(1)) {
                for (i, &zone) in zones.iter().enumerate() {
                    let on = on_now(&sim, zone);
                    match (open[i], on) {
                        (None, true) => open[i] = Some(t),
                        (Some(from), false) => {
                            spans[i].push((from, t));
                            open[i] = None;
                        }
                        _ => {}
                    }
                }
            }
            for (i, &zone) in zones.iter().enumerate() {
                if let Some(from) = open[i] {
                    spans[i].push((from, until));
                }
                // Spans that ended just now stay (a margin after a switch
                // must outlast a new prediction).
                let mut kept: Vec<(u32, u32)> = self
                    .schedules
                    .get(zone)
                    .and_then(Option::as_ref)
                    .map(|old| {
                        old.spans
                            .iter()
                            .filter(|&&(from, to)| {
                                from < now && to <= now && to.saturating_add(RECENT_MS) > now
                            })
                            .copied()
                            .collect()
                    })
                    .unwrap_or_default();
                kept.append(&mut spans[i]);
                schedules[zone] = Some(Schedule {
                    from: now,
                    spans: kept,
                    until,
                });
            }
        }
        self.schedules = schedules;
        self.predicted_at = now;
        self.predicted_changes = self.changes();
        self.version += 1;
    }
}

/// Whether the simulation had what the world has: the same state, zones
/// and next switches (a tick of slack: the client's off-grid ticks).
fn agrees(sim: &Sim, decoded: &Decoded) -> bool {
    if sim.current != decoded.state {
        return false;
    }
    let Some(state) = decoded.state else {
        return true;
    };
    sim.states[state]
        .groups
        .iter()
        .zip(&decoded.groups)
        .all(|(group, (next, on))| group.on == *on && group.next.abs_diff(*next) <= TICK_MS)
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// Reads the core whose random value is at `rng_at`, backwards (layout in
/// the module notes); `sim` gives its settings and, where the bytes allow
/// two readings, the likelier one.
fn decode(objects: &[u8], rng_at: usize, sim: &Sim, world_ms: u32) -> Option<Decoded> {
    let events_at = rng_at.checked_sub(1)?;
    if objects[events_at] > 3 {
        return None;
    }
    let max_len = sim
        .states
        .iter()
        .map(|state| 4 + state.groups.iter().map(Group::max_bytes).sum::<usize>())
        .max()
        .unwrap_or(0);
    let mut readings: Vec<(usize, Decoded)> = Vec::new();
    // The state byte at q, its data after it up to the events byte.
    for q in events_at.saturating_sub(max_len + 1)..events_at {
        let state = objects[q];
        let parsed = if state == 0xFF {
            // No state: only before the first tick (a core never goes
            // back to none).
            (q + 1 == events_at && sim.current.is_none()).then(|| (None, 0, Vec::new()))
        } else {
            let index = usize::from(state);
            sim.states
                .get(index)
                .and_then(|s| read_state(objects, q + 1, events_at, s, world_ms))
                .map(|(finish, groups)| (Some(index), finish, groups))
        };
        let Some((state, finish, groups)) = parsed else {
            continue;
        };
        let Some(energy_at) = q.checked_sub(4) else {
            continue;
        };
        let energy = f32::from_le_bytes(objects[energy_at..q].try_into().ok()?);
        if !(energy == 0.0 || (energy.is_normal() && (1.0e-3..=1.0e7).contains(&energy))) {
            continue;
        }
        let artefact = if sim.art.enabled {
            read_artefact(objects, energy_at, sim, world_ms)
        } else {
            None
        };
        readings.push((
            q,
            Decoded {
                state,
                energy,
                finish,
                groups,
                artefact,
            },
        ));
    }
    match readings.len() {
        0 => None,
        1 => readings.pop().map(|(_, decoded)| decoded),
        // Two readings: the one in the simulation's state, if only one is.
        _ => {
            let mut same: Vec<Decoded> = readings
                .into_iter()
                .map(|(_, decoded)| decoded)
                .filter(|decoded| decoded.state == sim.current)
                .collect();
            (same.len() == 1).then(|| same.pop()).flatten()
        }
    }
}

/// A state's data from `at`, which must end at `end`: the finish time and
/// each group's next switch and zones on.
#[allow(clippy::type_complexity)]
fn read_state(
    objects: &[u8],
    mut at: usize,
    end: usize,
    state: &State,
    world_ms: u32,
) -> Option<(u32, Vec<(u32, Vec<bool>)>)> {
    let finish = u32_at(objects, at)?;
    at += 4;
    if finish != 0
        && finish
            > world_ms
                .saturating_add(state.active_ms)
                .saturating_add(1000)
    {
        return None;
    }
    let mut groups = Vec::with_capacity(state.groups.len());
    for group in &state.groups {
        let n = group.zones.len();
        let (next, on) = if group.keep_all {
            (0, vec![true; n])
        } else {
            let next = u32_at(objects, at)?;
            at += 4;
            let reach = group.max_ms.max(group.recharge_ms);
            if next != 0 && next > world_ms.saturating_add(reach).saturating_add(1000) {
                return None;
            }
            let mask = objects.get(at..at + n.div_ceil(8))?;
            at += n.div_ceil(8);
            let on: Vec<bool> = (0..n).map(|i| mask[i / 8] & (1 << (i % 8)) != 0).collect();
            // No bit past the zones.
            let bits: u32 = mask.iter().map(|b| b.count_ones()).sum();
            if bits as usize != on.iter().filter(|&&on| on).count() {
                return None;
            }
            (next, on)
        };
        // Each active zone: u32 last hit (-1 never), u8 someone inside.
        for _ in on.iter().filter(|&&on| on) {
            let last_hit = u32_at(objects, at)?;
            let hitting = *objects.get(at + 4)?;
            // A bool written as 0 or 0xFF (seen; 1 taken too).
            let flag = matches!(hitting, 0 | 1 | 0xFF);
            if !flag || (last_hit != u32::MAX && last_hit > world_ms.saturating_add(1000)) {
                return None;
            }
            at += 5;
        }
        groups.push((next, on));
    }
    (at == end).then_some((finish, groups))
}

/// The artefact prefix before the energy at `energy_at`: `u8 pending`
/// then, when set, `u32 next`. The flag byte of an idle core and the top
/// byte of a pending one's time are both 0, so the simulation's word
/// settles it when both readings fit.
fn read_artefact(
    objects: &[u8],
    energy_at: usize,
    sim: &Sim,
    world_ms: u32,
) -> Option<(bool, u32)> {
    let idle = energy_at >= 1 && objects[energy_at - 1] == 0;
    let reach = (sim.art.respawn_s.max(1.0) + sim.art.variation_s) * 1000.0 + 120_000.0;
    let pending = (energy_at >= 5 && objects[energy_at - 5] != 0)
        .then(|| u32_at(objects, energy_at - 4))
        .flatten()
        .filter(|&next| next > 0 && (next as f32) <= world_ms as f32 + reach);
    match (pending, idle) {
        (Some(next), true) if sim.art.pending => Some((true, next)),
        (_, true) => Some((false, 0)),
        (Some(next), false) => Some((true, next)),
        (None, false) => None,
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use mapdata::{
        Anomaly, AnomalyArtefacts, AnomalyEnergy, AnomalyState, Damage, Rotation, Shape, ShapeKind,
        Vec3, Zone, ZoneGroup, ZoneKind,
    };

    /// A damage zone of one shape.
    pub fn zone(name: &str, shape: Shape, max_hit: f32) -> Zone {
        Zone {
            kind: ZoneKind::Damage,
            name: name.into(),
            team: -1,
            position: shape.center,
            damage_type: "shock".into(),
            damage: Some(Damage {
                max_hit,
                interval_ms: 1000,
                parts: 2,
                ..Damage::default()
            }),
            shapes: vec![shape],
        }
    }

    pub fn cylinder(x: f32, z: f32, radius: f32) -> Shape {
        Shape {
            kind: ShapeKind::Cylinder,
            center: Vec3::new(x, 0.1, z),
            rotation: Rotation::IDENTITY,
            half: Vec3::new(radius, 0.25, radius),
        }
    }

    /// An electric anomaly as level_03 has them: two zones taking turns,
    /// 40-60 s each; with artefacts if `artefacts`.
    pub fn electric(name: &str, zones: [&str; 2], artefacts: bool) -> Anomaly {
        Anomaly {
            name: name.into(),
            position: Vec3::ZERO,
            zones: zones.iter().map(|z| (*z).into()).collect(),
            energy: AnomalyEnergy {
                enabled: true,
                initial: 1.0,
                ..AnomalyEnergy::default()
            },
            artefacts: AnomalyArtefacts {
                enabled: artefacts,
                first_spawn_s: 60.0,
                respawn_s: 60.0,
                variation_s: 30.0,
                chance: 1.0,
                max_count: 1,
                containers: 6,
            },
            states: vec![AnomalyState {
                enabled: true,
                threshold: 1.0,
                groups: vec![ZoneGroup {
                    enabled: true,
                    keep_all: false,
                    sequential: true,
                    min_active_s: 40,
                    max_active_s: 60,
                    max_charged: 255,
                    recharge_s: 15,
                    zones: zones.iter().map(|z| (*z).into()).collect(),
                }],
                ..AnomalyState::default()
            }],
        }
    }

    /// A cloud like level_02's fruit: states by energy 0 / 20 / 1000, a
    /// zone each (keep-all), energy falling 25 a second.
    fn cloud() -> Anomaly {
        let state = |threshold: f32, zone: &str| AnomalyState {
            enabled: true,
            threshold,
            select_priority: 1,
            groups: vec![ZoneGroup {
                enabled: true,
                keep_all: true,
                max_charged: 255,
                zones: vec![zone.into()],
                ..ZoneGroup::default()
            }],
            ..AnomalyState::default()
        };
        Anomaly {
            name: "cloud".into(),
            position: Vec3::ZERO,
            zones: vec!["low".into(), "lvl1".into(), "lvl3".into()],
            energy: AnomalyEnergy {
                enabled: true,
                decrease_speed: 25.0,
                on_walk: 35.0,
                on_run: 45.0,
                on_sprint: 70.0,
                ..AnomalyEnergy::default()
            },
            artefacts: AnomalyArtefacts::default(),
            states: vec![
                state(0.0, "low"),
                state(20.0, "lvl1"),
                state(1000.0, "lvl3"),
            ],
        }
    }

    pub fn electric_map() -> MapData {
        MapData {
            zones: vec![
                zone("a", cylinder(10.0, 20.0, 2.0), 0.6),
                zone("b", cylinder(30.0, 20.0, 2.0), 0.6),
            ],
            anomalies: vec![electric("core", ["a", "b"], false)],
            ..MapData::default()
        }
    }

    /// §5 of the research, written out plainly: the switch times and the
    /// zone switched on at each, for a two-zone sequential core without
    /// artefacts, on the 50 ms grid.
    fn reference(seed: u32, until: u32) -> Vec<(u32, usize)> {
        let mut rng = lcg(seed); // load
        let mut out = Vec::new();
        // First tick: recharge(0).
        rng = lcg(rng);
        let mut on = ((u64::from(rng) * 2) >> 32) as usize;
        rng = lcg(rng);
        let mut next = 40_000 + ((u64::from(rng) * 20_000) >> 32) as u32;
        out.push((50, on));
        let mut t = 50;
        while t <= until {
            if t >= next {
                rng = lcg(rng); // the pick among the one inactive zone
                on = 1 - on;
                rng = lcg(rng);
                next = t + 40_000 + ((u64::from(rng) * 20_000) >> 32) as u32;
                out.push((t, on));
            }
            t += 50;
        }
        out
    }

    #[test]
    fn random_stream_and_seed_follow_the_client() {
        assert_eq!(lcg(0), 1);
        assert_eq!(lcg(1), 0x0808_8406);
        assert_eq!(
            lcg(0xFFFF_FFFF),
            0u32.wrapping_sub(0x0808_8405).wrapping_add(1)
        );
        // The standard CRC-32 check value.
        assert_eq!(core_seed(0, "123456789"), 0xCBF4_3926);
        assert_eq!(core_seed(0x8000_0000, "123456789"), 0x4BF4_3926);
        assert_eq!(pick(0xFFFF_FFFF, 2), 1);
        assert_eq!(pick(0x7FFF_FFFF, 2), 0);
        assert_eq!(pick(0x8000_0000, 20_000), 10_000);
        assert_eq!(unit(0x8000_0000), 0.5);
        assert_eq!(delay_ms(0x8000_0000, 60.0, 30.0), 60_000);
        assert_eq!(delay_ms(0, 60.0, 30.0), 30_000);
    }

    #[test]
    fn an_electric_pair_takes_turns_as_the_client_draws() {
        let map = electric_map();
        let seed = 0x8123_4567;
        let mut anomalies = Anomalies::new(&map, seed);
        assert!(!anomalies.zone_on_now(0) && !anomalies.zone_on_now(1));
        let expected = reference(core_seed(seed, "core"), 900_000);
        assert!(expected.len() > 14, "{expected:?}");
        let mut seen = Vec::new();
        let mut last = (false, false);
        let mut t = 0;
        while t <= 900_000 {
            anomalies.advance(t);
            let on = (anomalies.zone_on_now(0), anomalies.zone_on_now(1));
            if on != last {
                assert!(on.0 != on.1, "exactly one on: {on:?} at {t}");
                seen.push((t, usize::from(on.1)));
                last = on;
            }
            t += 50;
        }
        assert_eq!(seen, expected);
        // Periods of 40-60 s.
        for pair in expected.windows(2).skip(1) {
            let period = pair[1].0 - pair[0].0;
            assert!((40_000..=60_050).contains(&period), "{period}");
        }
        // Advancing in big steps lands on the same state.
        let mut jumpy = Anomalies::new(&map, seed);
        for t in [10, 47_000, 300_000, 655_555, 900_000] {
            jumpy.advance(t);
        }
        assert_eq!(jumpy.cores[0].random(), anomalies.cores[0].random());
        assert_eq!(jumpy.zone_on_now(1), anomalies.zone_on_now(1));
    }

    #[test]
    fn the_schedule_predicts_the_switches_and_next_switch_names_them() {
        let map = electric_map();
        let seed = 0x8000_0042;
        let expected = reference(core_seed(seed, "core"), 400_000);
        let mut anomalies = Anomalies::new(&map, seed);
        anomalies.advance(1_000);
        // From 1 s on: the zone on at each switch, in turn.
        for &(at, on) in expected.iter().skip(1).take(5) {
            assert!(anomalies.zone_active(on, at), "{on} at {at}");
            assert!(!anomalies.zone_active(on, at - 50), "{on} at {}", at - 50);
            assert!(!anomalies.zone_active(1 - on, at));
        }
        assert_eq!(anomalies.next_switch(0), Some(expected[1].0));
        assert_eq!(anomalies.next_switch(1), Some(expected[1].0));
        assert_eq!(anomalies.control(0), Control::Timer);
        let schedule = anomalies.schedule(0).unwrap();
        assert_eq!(schedule.until, 1_000 + PREDICT_MS);
        // Past the prediction: on (unknown).
        assert!(anomalies.zone_active(0, schedule.until + 1));
    }

    #[test]
    fn a_core_with_artefacts_draws_four_more_at_its_first_spawn() {
        let mut map = electric_map();
        map.anomalies[0].artefacts.enabled = true;
        let seed = 0x8765_4321;
        let mut anomalies = Anomalies::new(&map, seed);
        let first = anomalies.cores[0].artefact().unwrap();
        // The load draw sets it, 30-90 s.
        let x = lcg(core_seed(seed, "core"));
        assert_eq!(first, (true, delay_ms(x, 60.0, 30.0)));
        assert!(anomalies.cores[0].exact());
        anomalies.advance(first.1 + 50);
        let (pending, _) = anomalies.cores[0].artefact().unwrap();
        assert!(!pending, "one spawned, the most there may be");
        assert!(!anomalies.cores[0].exact(), "a pickup would draw unseen");
        // The draws so far: load 1, first tick 2, the spawn 4, and 2 per
        // switch before it.
        let switches = reference(core_seed(seed, "core"), first.1 + 50)
            .iter()
            .skip(1)
            .filter(|&&(t, _)| t < first.1)
            .count();
        let mut x = core_seed(seed, "core");
        for _ in 0..(1 + 2 + 2 * switches + 4) {
            x = lcg(x);
        }
        // (A switch on the spawn's own tick draws before it.)
        let mut y = x;
        y = lcg(lcg(y));
        let random = anomalies.cores[0].random();
        assert!(random == x || random == y, "{random:#x}");
        // Inexact: the prediction stops a shortest period past the next
        // switch.
        let schedule = anomalies.schedule(0).unwrap();
        let next = anomalies.next_switch(0).unwrap();
        assert_eq!(schedule.until, next + 40_000);
    }

    /// The bytes a world writes for a core (layout in the module notes),
    /// with `junk` objects before it.
    fn world_bytes(junk: &[u8], sim: &Sim, events: u8) -> (Vec<u8>, usize) {
        let mut out = junk.to_vec();
        if sim.art.enabled {
            out.push(if sim.art.pending { 0xFF } else { 0 });
            if sim.art.pending {
                out.extend(sim.art.next.to_le_bytes());
            }
        }
        out.extend(sim.energy.to_le_bytes());
        match sim.current {
            None => out.push(0xFF),
            Some(s) => {
                out.push(s as u8);
                out.extend(sim.finish.to_le_bytes());
                for group in &sim.states[s].groups {
                    if !group.keep_all {
                        out.extend(group.next.to_le_bytes());
                        let mut mask = vec![0u8; group.on.len().div_ceil(8)];
                        for (i, &on) in group.on.iter().enumerate() {
                            if on {
                                mask[i / 8] |= 1 << (i % 8);
                            }
                        }
                        out.extend(mask);
                    }
                    for _ in group.on.iter().filter(|&&on| on) {
                        out.extend(u32::MAX.to_le_bytes());
                        out.push(0);
                    }
                }
            }
        }
        out.push(events);
        let at = out.len();
        out.extend(sim.rng.to_le_bytes());
        out.extend([0x00, 0x00, 0x07]);
        (out, at)
    }

    #[test]
    fn a_world_sets_the_core_and_the_schedule_follows() {
        let map = electric_map();
        let seed = 0x8000_1234;
        let mut anomalies = Anomalies::new(&map, seed);
        anomalies.advance(65_500);
        // The world (at 65 s) as the client had it: our simulation, but a
        // switch a few draws further on (as after an artefact pickup).
        let mut world = anomalies.cores[0].base.clone();
        world.advance(64_999);
        let truth_on = world.states[0].groups[0].on.clone();
        world.rng = lcg(lcg(lcg(world.rng)));
        world.states[0].groups[0].next = 66_020;
        let junk = [5u8, 0xFF, 0xFF, 0xFF, 0xFF, 0, 1, 0, 0, 0];
        let (objects, at) = world_bytes(&junk, &world, 0);
        let report = anomalies.sync_from_world(&objects, 65_000);
        assert_eq!(report.missed, Vec::<usize>::new());
        let synced = &report.synced[0];
        assert_eq!(synced.offset, at);
        assert!(!synced.matched, "the next switch moved");
        // From the world on: the switch at the first tick past 66 020.
        assert_eq!(anomalies.cores[0].synced_at(), Some(65_000));
        let on = truth_on.iter().position(|&on| on).unwrap();
        assert!(anomalies.zone_on_now(on));
        assert!(anomalies.zone_active(on, 66_000));
        assert!(!anomalies.zone_active(on, 66_050));
        assert!(anomalies.zone_active(1 - on, 66_050));
        assert_eq!(anomalies.next_switch(on), Some(66_050));
        anomalies.advance(70_000);
        assert!(!anomalies.zone_on_now(on), "switched at 66 050");
        assert!(anomalies.zone_on_now(1 - on));
        // The same world again, or an older one: nothing to do.
        assert!(
            anomalies
                .sync_from_world(&objects, 65_000)
                .synced
                .is_empty()
        );
        // A world without the core: missed.
        let report = anomalies.sync_from_world(&junk, 80_000);
        assert_eq!(report.missed, vec![0]);
    }

    #[test]
    fn a_world_of_the_simulation_itself_matches() {
        let mut map = electric_map();
        map.anomalies[0].artefacts.enabled = true;
        let seed = 0x8000_0777;
        let mut anomalies = Anomalies::new(&map, seed);
        for world_ms in [50, 5_050, 41_000, 47_000, 95_000, 140_000] {
            let mut world = anomalies.cores[0].base.clone();
            world.advance(world_ms - 1);
            let (objects, at) = world_bytes(&[1, 2, 3], &world, 1);
            anomalies.advance(world_ms + 3_000);
            let report = anomalies.sync_from_world(&objects, world_ms);
            assert_eq!(report.synced.len(), 1, "at {world_ms}");
            assert_eq!(report.synced[0].offset, at);
            assert!(report.synced[0].matched, "at {world_ms}");
        }
    }

    #[test]
    fn a_cloud_follows_its_energy_down() {
        let map = MapData {
            zones: vec![
                zone("low", cylinder(0.0, 0.0, 1.0), 0.07),
                zone("lvl1", cylinder(0.0, 0.0, 5.0), 0.07),
                zone("lvl3", cylinder(0.0, 0.0, 10.0), 0.1),
            ],
            anomalies: vec![cloud()],
            ..MapData::default()
        };
        let mut anomalies = Anomalies::new(&map, 0x8000_0001);
        anomalies.advance(1_000);
        assert!(anomalies.zone_on_now(0) && !anomalies.zone_on_now(2));
        assert_eq!(
            anomalies.control(2),
            Control::Energy {
                threshold: 1000.0,
                per_second: 70.0
            }
        );
        assert_eq!(
            anomalies.control(0),
            Control::Energy {
                threshold: 0.0,
                per_second: 70.0
            }
        );
        // A world at 10 s: energy 1100 (state 0: lvl3).
        let mut world = anomalies.cores[0].base.clone();
        world.advance(9_999);
        world.energy = 1100.0;
        world.current = Some(0);
        world.states[2].groups[0].on = vec![false];
        world.states[0].groups[0].on = vec![true];
        let (objects, _) = world_bytes(&[9, 9], &world, 0);
        anomalies.advance(11_000);
        let report = anomalies.sync_from_world(&objects, 10_000);
        assert_eq!(report.synced.len(), 1);
        assert_eq!(anomalies.cores[0].state(), Some(0));
        assert!(anomalies.zone_on_now(2));
        // 100 above 1000 at 25 a second: off 4 s after the world.
        assert!(anomalies.zone_active(2, 13_900));
        assert!(!anomalies.zone_active(2, 14_100));
        assert!(anomalies.zone_active(1, 14_100), "lvl1 follows");
        // From the world's own tick to 11 s: 21 ticks of 1.25.
        let energy = anomalies.cores[0].energy();
        assert!((energy - (1100.0 - 1.25 * 21.0)).abs() < 0.01, "{energy}");
    }

    /// The saved worlds of real matches on level_02 and level_03, only with
    /// `SRP_MAPDATA_DIR=<mapdata-dump files>` and `SRP_WORLDS_DIR=<a folder
    /// with matches.txt and snapshots/>` (private; lines of matches.txt:
    /// `date time match_id map_id name mode players`, files
    /// `snapshots/match026g_<id>_<ms>_p<place>_checked_*.bin` = world
    /// bodies):
    ///
    /// ```text
    /// SRP_MAPDATA_DIR=... SRP_WORLDS_DIR=... cargo test -p match-server --release real_worlds -- --nocapture
    /// ```
    ///
    /// Every world is synced the way the match server does it (the cores
    /// already a few seconds past it); before each, the simulation from the
    /// last world is compared with it.
    #[test]
    fn real_worlds_agree_with_the_simulation() {
        use std::collections::BTreeMap;
        use std::path::PathBuf;

        use survarium::v026g::match_msgs::snapshot::Sections;

        let (Some(maps), Some(worlds)) = (
            std::env::var_os("SRP_MAPDATA_DIR").map(PathBuf::from),
            std::env::var_os("SRP_WORLDS_DIR").map(PathBuf::from),
        ) else {
            eprintln!("skipping: SRP_MAPDATA_DIR or SRP_WORLDS_DIR is not set");
            return;
        };
        let Ok(list) = std::fs::read_to_string(worlds.join("matches.txt")) else {
            eprintln!("skipping: no matches.txt in SRP_WORLDS_DIR");
            return;
        };
        let mut files: BTreeMap<String, BTreeMap<u32, PathBuf>> = BTreeMap::new();
        for entry in std::fs::read_dir(worlds.join("snapshots")).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() < 4 || parts[0] != "match026g" {
                continue;
            }
            if let Ok(ms) = parts[2].parse::<u32>() {
                files
                    .entry(parts[1].to_owned())
                    .or_default()
                    .entry(ms)
                    .or_insert(path);
            }
        }
        // By core name: (synced, matched, missed).
        let mut tally: BTreeMap<String, (u32, u32, u32)> = BTreeMap::new();
        // By core name: (worlds, zones as the never-synced simulation).
        let mut pure_tally: BTreeMap<String, (u32, u32)> = BTreeMap::new();
        // Timer cores without artefacts: nothing unseen draws from them.
        let mut exact_timers: Vec<String> = Vec::new();
        let mut matches = 0;
        for line in list.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (Some(id), Some(map_id)) = (fields.get(2), fields.get(3)) else {
                continue;
            };
            let (Ok(match_id), Ok(map_id)) = (u32::from_str_radix(id, 16), map_id.parse::<u8>())
            else {
                continue;
            };
            let Some(project) = survarium::v026g::maps::map_mode(map_id).map(|m| m.project) else {
                continue;
            };
            if !(project.starts_with("level_02") || project.starts_with("level_03")) {
                continue;
            }
            let Some(times) = files.get(*id) else {
                continue;
            };
            let map = MapData::read(&maps.join(mapdata::format::file_name(project))).unwrap();
            let mut anomalies = Anomalies::new(&map, match_id | 0x8000_0000);
            if match_id == 0x6375_5501 {
                // The research's first artefact times of this match.
                let mut firsts: Vec<u32> = anomalies
                    .cores()
                    .iter()
                    .filter_map(|core| core.artefact().map(|(_, at)| at))
                    .collect();
                firsts.sort_unstable();
                eprintln!("{id}: first artefact times {firsts:?}");
                for at in [47_710, 64_861, 73_058] {
                    assert!(firsts.contains(&at), "{at} in {firsts:?}");
                }
            }
            for zone in 0..map.zones.len() {
                if anomalies.control(zone) == Control::Timer
                    && let Some(slot) = anomalies.slots[zone].first()
                {
                    let core = &anomalies.cores()[slot.core];
                    if core.artefact().is_none() && !exact_timers.contains(&core.name) {
                        exact_timers.push(core.name.clone());
                    }
                }
            }
            matches += 1;
            let mut pure = anomalies.clone();
            for (&ms, path) in times {
                let body = std::fs::read(path).unwrap();
                let Some(sections) = Sections::walk(&body) else {
                    continue;
                };
                anomalies.advance(ms + 3_000);
                let report = anomalies.sync_from_world(sections.objects, ms);
                pure.advance(ms - 1);
                for synced in &report.synced {
                    let (world, sim) = (
                        &anomalies.cores[synced.core].base,
                        &pure.cores[synced.core].sim,
                    );
                    if let Some(state) = world.current.filter(|_| sim.current == world.current) {
                        let timers = world.states[state].groups.iter().any(|g| !g.keep_all);
                        if timers {
                            let same = world.states[state]
                                .groups
                                .iter()
                                .zip(&sim.states[state].groups)
                                .all(|(a, b)| a.on == b.on);
                            let entry = pure_tally
                                .entry(anomalies.cores[synced.core].name.clone())
                                .or_default();
                            entry.0 += 1;
                            entry.1 += u32::from(same);
                        }
                    }
                }
                for synced in &report.synced {
                    let entry = tally
                        .entry(anomalies.cores()[synced.core].name.clone())
                        .or_default();
                    entry.0 += 1;
                    entry.1 += u32::from(synced.matched);
                    if !synced.matched && std::env::var_os("SRP_WORLDS_VERBOSE").is_some() {
                        eprintln!(
                            "{id} {project} {ms}: {} drifted",
                            anomalies.cores()[synced.core].name
                        );
                    }
                }
                for &missed in &report.missed {
                    if std::env::var_os("SRP_WORLDS_VERBOSE").is_some() {
                        eprintln!(
                            "{id} {project} {ms}: {} not found",
                            anomalies.cores()[missed].name
                        );
                    }
                    tally
                        .entry(anomalies.cores()[missed].name.clone())
                        .or_default()
                        .2 += 1;
                }
            }
        }
        eprintln!("{matches} matches");
        for (name, (worlds, same)) in &pure_tally {
            eprintln!("{name}: never synced, the same zones on in {same} of {worlds} worlds");
        }
        for (name, (synced, matched, missed)) in &tally {
            eprintln!("{name}: {synced} worlds read, {matched} as simulated, {missed} not found");
        }
        if matches == 0 {
            return;
        }
        for (name, (synced, matched, missed)) in &tally {
            // Every core is found in (nearly) every world.
            assert!(
                *missed * 50 <= *synced,
                "{name}: {missed} of {synced} not found"
            );
            // The electric cores without artefacts are exact but for the
            // client's rare off-grid ticks.
            if exact_timers.contains(name) {
                assert!(
                    *matched * 20 >= *synced * 19,
                    "{name}: {matched} of {synced}"
                );
                let (worlds, same) = pure_tally[name];
                assert!(same * 20 >= worlds * 19, "{name}: alone {same} of {worlds}");
            }
        }
    }
}
