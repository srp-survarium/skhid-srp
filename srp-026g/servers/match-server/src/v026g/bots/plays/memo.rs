//! Answers to the plays' costly questions of the map, kept for the match
//! ([`Memo`], owned by `Bots`, reached as `Ctx::memo`): a question whose
//! answer depends only on the map and on what each zone is (its control
//! and its hit, set at `Hazards::attach`) is worked out once and read
//! after.
//!
//! The first is a battery's crossing ([`collect::crossing_to`]): about
//! 0.47 ms a call, asked about 70 times in a slow commander pass (Collect's
//! and Ambush's candidates and the notes' `take_blocks`, each for every
//! bot and battery in zones on all match), 93 % of that pass.
//!
//! [`collect::crossing_to`]: super::collect::crossing_to

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Mutex, OnceLock, PoisonError};

use mapdata::stands::{EYE_CROUCHED, EYE_STANDING, GUESSED_EYE_CROUCHED, GUESSED_EYE_STANDING};
use mapdata::{UsePose, UseStand, UseStands, Vec3};

use super::Ctx;
use super::collect::{Crossing, CrossingAsk, crossing_to, routed_crossing_ask};
use crate::v026g::bots::hazards::HurtTest;
use crate::v026g::bots::tuning::RouteCrossing;

/// What a crossing's question asks of the zones and the motor's route over them: a team's tuning
/// (`HazardTuning::route_crossing`, `price_way_out`, `hurt_by_shape`), so a question is kept per rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RoutedRule {
    pub route: RouteCrossing,
    pub way_out: bool,
    pub hurt: HurtTest,
}

impl RoutedRule {
    /// The rule of `tuning` for a body of `radius` metres.
    pub fn of(tuning: &crate::v026g::bots::tuning::HazardTuning, radius: f32) -> Self {
        Self { route: tuning.route_crossing, way_out: tuning.price_way_out, hurt: HurtTest::of(tuning, radius) }
    }

    /// Only the straight crossings, priced by the margin.
    pub const OFF: Self = Self { route: RouteCrossing::Off, way_out: false, hurt: HurtTest::MARGIN };
}
use crate::v026g::bots::hazards::Hazards;
use crate::v026g::bots::tactics::Tactics;
use crate::v026g::bots::zone_cost::ZoneArmour;

/// Answers one question keeps at most; past them it starts afresh (a
/// match asks a few dozen: the stands of batteries in zones on all match,
/// by side).
const MEMO_MOST: usize = 4096;

/// What each zone is, as the answers depend on it: whether it switches
/// (its control, `Hazard::switches`) and its hit a second
/// (`Hazard::hit_per_s`). It changes only at `Hazards::attach` (and in
/// tests that set a zone's control).
fn zones_stamp(hazards: &Hazards) -> impl Iterator<Item = u64> + Clone + '_ {
    hazards
        .zones
        .iter()
        .map(|zone| u64::from(zone.switches()) << 32 | u64::from(zone.hit_per_s().to_bits()))
}

/// One question's answers by key, and the stamp ([`zones_stamp`]) they
/// were made for: another stamp drops them all.
pub struct QueryMemo<K, V> {
    kept: Mutex<Kept<K, V>>,
}

struct Kept<K, V> {
    made_for: Vec<u64>,
    answers: HashMap<K, V>,
}

impl<K, V> Default for QueryMemo<K, V> {
    fn default() -> Self {
        Self {
            kept: Mutex::new(Kept {
                made_for: Vec::new(),
                answers: HashMap::new(),
            }),
        }
    }
}

impl<K: Eq + Hash, V: Clone> QueryMemo<K, V> {
    /// The answer for `key` under `stamp`: the kept one, else `ask`'s,
    /// kept. `ask` runs with nothing locked.
    pub fn get(
        &self,
        stamp: impl Iterator<Item = u64> + Clone,
        key: K,
        ask: impl FnOnce() -> V,
    ) -> V {
        {
            let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
            if !kept.made_for.iter().copied().eq(stamp.clone()) {
                kept.answers.clear();
                kept.made_for = stamp.collect();
            } else if let Some(answer) = kept.answers.get(&key) {
                return answer.clone();
            }
        }
        let answer = ask();
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        if kept.answers.len() >= MEMO_MOST {
            kept.answers.clear();
        }
        kept.answers.insert(key, answer.clone());
        answer
    }

    /// [entry slices] The kept answer for `key` under `stamp`, if there is one; another stamp drops them all, as [`Self::get`].
    pub fn peek(&self, stamp: impl Iterator<Item = u64> + Clone, key: &K) -> Option<V> {
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        if !kept.made_for.iter().copied().eq(stamp.clone()) {
            kept.answers.clear();
            kept.made_for = stamp.collect();
            return None;
        }
        kept.answers.get(key).cloned()
    }

    /// [entry slices] `answer` kept for `key` under `stamp`, as [`Self::get`] keeps its ask's.
    pub fn insert(&self, stamp: impl Iterator<Item = u64> + Clone, key: K, answer: V) {
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        if !kept.made_for.iter().copied().eq(stamp.clone()) {
            kept.answers.clear();
            kept.made_for = stamp.collect();
        }
        if kept.answers.len() >= MEMO_MOST {
            kept.answers.clear();
        }
        kept.answers.insert(key, answer);
    }

    /// [sliced load] Every kept answer, for the tests.
    #[cfg(test)]
    pub(crate) fn all(&self) -> Vec<(K, V)>
    where
        K: Clone,
    {
        let kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        kept.answers.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    /// Answers kept.
    pub fn kept(&self) -> usize {
        self.kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .answers
            .len()
    }
}

/// [crossing prewarm] `ask` with the bench's damage-zone counters left as they were (tests); `ask` elsewhere.
#[cfg(test)]
fn quiet<R>(ask: impl FnOnce() -> R) -> R {
    crate::v026g::bots::hazards::damage_zone_fixes::quietly(ask)
}

#[cfg(not(test))]
fn quiet<R>(ask: impl FnOnce() -> R) -> R {
    ask()
}

/// A crossing's question: the stand's x, y, z bits, the side, the stop's
/// bits, the armour it is worked out with (`ZoneArmour::key`; 0: none), and whether a route over
/// the zones' floors answers where no straight walk does (`HazardTuning::route_crossing`, `total_crossing`).
type CrossingKey = (u32, u32, u32, Option<i32>, u32, u64, RoutedRule);

/// An escort's post off the line: the carrier's floor, the wanted post in whole metres, the side, a time bucket.
type PostKey = (Option<mapdata::NodeId>, i32, i32, Option<i32>, u32);

/// How long an escort's post off the line is kept, ms: the zones that switch are asked again after it.
const POST_KEEP_MS: u32 = 2_000;

/// The plays' kept answers for one match on one map (`Bots` makes it anew
/// with the tactics).
#[derive(Default)]
pub struct Memo {
    crossings: QueryMemo<CrossingKey, Option<Crossing>>,
    /// [entry slices] [`crossing_to`]'s answers by the crossing's question: asked again while a routed crossing's search is
    /// pending, they are not worked out again.
    straights: QueryMemo<CrossingKey, Option<Crossing>>,
    /// [entry slices] A routed crossing's entry way by its search's question less the armour ([`SharedEntryKey`]): the way of
    /// the first armour searched in play, every other armour's bound ([`Memo::entry_ask`]).
    shared_entries: QueryMemo<SharedEntryKey, Option<std::sync::Arc<Vec<mapdata::NodeId>>>>,
    /// [entry slices] The entry searches under way, the poll's pop budget and the counts ([`Memo::new_poll`]).
    slices: std::sync::Mutex<EntrySlices>,
    /// The escorts' posts off the line, by the carrier's floor ([`Memo::escort_post`]).
    posts: QueryMemo<PostKey, Option<Vec3>>,
    /// The containers' stands: they depend on the map alone, so they are
    /// made at the first ask and kept for the match.
    stands: OnceLock<UseStands>,
    /// [take.zone_look] A dry floor to look at a zone spawner's spot from, by the spot ([`Memo::zone_look`]).
    zone_looks: QueryMemo<(u32, u32, u32), Option<Vec3>>,
    /// [anomaly batteries] The way into a battery's stand inside an energy core's zones, by the stand's floor and the side
    /// ([`Memo::zone_way`]).
    zone_ways: Mutex<ZoneWays>,
    /// [crossing prewarm] A routed crossing's entry search ([`Memo::entry`]): made at map load for the battery spawners'
    /// stands ([`Memo::prewarm_entries`]), else at the first ask.
    entries: QueryMemo<EntryKey, Option<std::sync::Arc<Vec<mapdata::NodeId>>>>,
    /// [entry steps] The last searches' sources and steps (`zone_search::EntrySteps`), each with the hazards' version and the
    /// zones' stamp it was made under: another armour's search for the same spawn floor, stand and side walks them again
    /// (the 4 kits' armours of a team ask in one commander pass), the same answers.
    entry_steps: Mutex<Vec<KeptSteps>>,
    /// [crossing prewarm] [`Memo::entry`]'s asks in play, and of them the searches made then (not kept from the load).
    entry_asks: std::sync::atomic::AtomicU32,
    entry_searches: std::sync::atomic::AtomicU32,
}

/// [crossing prewarm] A routed crossing's entry search ([`crate::v026g::bots::zone_search::cheapest_entry`]): the side's spawn floor,
/// the stand's floor, the side, the armour's key (`ZoneArmour::key`; 0: none) and the hurt rule. Everything its price reads but
/// the zones, which the stamp holds.
type EntryKey = (mapdata::NodeId, mapdata::NodeId, i32, u64, HurtTest);

/// [entry slices] An entry search's question less the armour: the side's spawn floor, the stand's floor, the side, the hurt rule.
type SharedEntryKey = (mapdata::NodeId, mapdata::NodeId, i32, HurtTest);

/// [entry slices] Floors an entry search may pop a poll, all the poll's searches together (`zone_search::PopBudget`): a count,
/// not a clock, so runs repeat. A pop is about 0.35 us on this PC (batt-m1-s7's entry searches: 111,000 pops in 37-44 ms), so
/// the budget is about 4-5 ms a poll, with a search's sources (about 2 ms) laid at its first ask.
pub const ENTRY_POPS_A_POLL: u64 = 12_000;

/// [entry slices] What an entry search's ask gets ([`Memo::entry_ask`]): a way (or none), or not yet.
#[derive(Clone, Debug, PartialEq)]
pub enum EntryAsk {
    Way(Option<std::sync::Arc<Vec<mapdata::NodeId>>>),
    Pending,
}

/// [entry slices] The entry searches under way and the poll's budget.
#[derive(Default)]
struct EntrySlices {
    /// What is left of this poll's pops; `None` before the first poll (the map's load, a test): a search then runs to its end.
    budget: Option<u64>,
    /// Polls made ([`Memo::new_poll`]).
    poll: u64,
    /// The zones' stamp the searches under way were begun for: another drops them.
    made_for: Vec<u64>,
    pending: Vec<PendingEntry>,
    counts: SliceCounts,
}

/// [entry slices] An entry search under way: its question, the armour it prices the way with (the first asker's), the poll it
/// was asked at and the runs it has had.
struct PendingEntry {
    /// The hazards' version it was begun under (`Hazards::version`): another drops it, as the kept steps.
    version: u64,
    shared: SharedEntryKey,
    key: EntryKey,
    armour: Option<ZoneArmour>,
    search: crate::v026g::bots::zone_search::EntrySearch,
    asked: u64,
    runs: u32,
}

/// [entry slices] The match's counts, for its end line: asks answered by another armour's way (searches the bound cut), searches
/// that ran over more than one poll, their runs, the searches dropped as the zones changed under them, and the polls their askers
/// waited (all, and the longest).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SliceCounts {
    pub bound: u32,
    /// Searches under way dropped because the hazards' version moved (begun again at the next ask).
    pub restarts: u32,
    pub sliced: u32,
    pub runs: u32,
    pub waited_polls: u64,
    pub longest_wait: u64,
}

/// [entry steps] A search's kept sources and steps and what it read the zones as: the hazards' version
/// (`Hazards::version`: the zones' activities and the live fields) and the zones' stamp ([`zones_stamp`]).
struct KeptSteps {
    version: u64,
    made_for: Vec<u64>,
    steps: crate::v026g::bots::zone_search::EntrySteps,
}

/// [entry steps] The routed entry searches made on another armour's kept steps, and the steps made anew, on this server (every
/// match), for the match-end nav line: decision-neutral.
pub(crate) mod entry_steps_counts {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    static REUSED: AtomicU64 = AtomicU64::new(0);
    static MADE: AtomicU64 = AtomicU64::new(0);

    pub(crate) fn reused() {
        REUSED.fetch_add(1, Relaxed);
    }

    pub(crate) fn made() {
        MADE.fetch_add(1, Relaxed);
    }

    /// (searched on kept steps, steps made) since the last call, and zero again.
    pub(crate) fn take() -> (u64, u64) {
        (REUSED.swap(0, Relaxed), MADE.swap(0, Relaxed))
    }
}

/// [entry steps] How many searches' steps are kept: the searches of one stand by both sides (about 600 thousand steps each
/// on batt m1's stand at (-23.7, -2.1)).
const ENTRY_STEPS_KEPT: usize = 2;

/// [core ways] Floors an energy core's way-in search may pop a poll, all the poll's searches together
/// (`zone_search::PopBudget`): a count, not a clock, so runs repeat. On batt-m2-s14 a way in is a 23-34 ms search; this is about
/// 4-5 ms of it a poll (the same budget as the routed entry searches').
pub const WAY_POPS_A_POLL: u64 = ENTRY_POPS_A_POLL;

/// [core ways] What an energy core's way-in ask gets ([`Memo::zone_way_ask`]): a way (or none), or not yet.
#[derive(Clone, Debug, PartialEq)]
pub enum ZoneWayAsk {
    Way(Option<std::sync::Arc<Vec<mapdata::NodeId>>>),
    Pending,
}

/// [core ways] The energy cores' ways in by stand floor and side, kept with the live fields of their boxes, and the searches
/// under way.
#[derive(Default)]
struct ZoneWays {
    /// The zones' stamp the ways were made for: another drops them all.
    made_for: Vec<u64>,
    kept: HashMap<(mapdata::NodeId, i32), KeptWay>,
    pending: Vec<PendingWay>,
    /// What is left of this poll's pops; `None` before the first poll (the map's load, a test): a search then runs to its end.
    budget: Option<u64>,
    poll: u64,
    counts: ZoneWayCounts,
}

/// [core ways] A way in kept: the way, the live fields of its search's box when it was made (`FieldBoard::fields_in`), and the
/// board's version they were last found the same at.
struct KeptWay {
    way: Option<std::sync::Arc<Vec<mapdata::NodeId>>>,
    fields: Vec<crate::v026g::bots::live_zones::WorldField>,
    checked: u64,
}

/// [core ways] A way-in search under way: its stand and side, the live fields of its box when it began and the board's version
/// they were last found the same at, the poll it was asked at, its runs.
struct PendingWay {
    key: (mapdata::NodeId, i32),
    search: crate::v026g::bots::switch_take::WayInSearch,
    fields: Vec<crate::v026g::bots::live_zones::WorldField>,
    checked: u64,
    asked: u64,
    runs: u32,
}

/// [core ways] The match's counts, for its end line.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ZoneWayCounts {
    /// Made at map load (stands times sides), and its time.
    pub prewarmed: u32,
    pub prewarm_ms: f32,
    /// Asks in play, of them answered by a kept way, ways dropped as the live fields in their box changed, searches begun in
    /// play, of them over more than one poll, their runs, searches begun again as the fields changed under them, and the polls
    /// their askers waited (all, and the longest).
    pub asks: u32,
    pub kept_answers: u32,
    pub dropped: u32,
    pub searches: u32,
    pub sliced: u32,
    pub runs: u32,
    pub restarts: u32,
    pub waited_polls: u64,
    pub longest_wait: u64,
}

/// [core ways] The live fields of `stand`'s way-in box now.
fn way_fields(tactics: &Tactics, stand: mapdata::NodeId) -> Vec<crate::v026g::bots::live_zones::WorldField> {
    let grid = &tactics.world.grid;
    tactics.hazards.live.fields_in(grid, crate::v026g::bots::switch_take::way_box(grid, stand, crate::v026g::bots::switch_take::WAY_WITHIN_M))
}

/// [crossing prewarm] What [`Memo::prewarm_entries`] made, for the map's log line.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EntryPrewarm {
    /// The battery spawners' stands that need a crossing.
    pub stands: usize,
    /// The (armour, rule) pairs asked, and the map's sides.
    pub asks: usize,
    pub sides: usize,
    /// Entry searches made, and of them ways found.
    pub searches: usize,
    pub found: usize,
    pub ms: f32,
}

/// [entry slices] Whether the way a search under `searched`'s key found answers another armour's ask (`asked`).
fn slices_key_differs(asked: EntryKey, searched: &EntryKey) -> bool {
    asked.3 != searched.3
}

impl Memo {
    /// [crossing prewarm] [`crate::v026g::bots::zone_search::cheapest_entry`] from the floor `from` (the side's spawn) to the stand's
    /// floor `goal` for a body of side `side`, priced as a routed crossing's leg is (`Hazards::leg_cost_per_s` under `armour` and
    /// `hurt`), within [`super::collect::ENTRY_WITHIN_M`]: kept while the zones stay what they are. The same answer as the search's.
    ///
    /// [entry slices] In play: the armour's own kept way (the load's, or its own search's); else the way another armour's search
    /// found for the same spawn, stand, side and rule, priced by the caller for this armour (an upper bound of its own way's price:
    /// the search's own way is the least, `zone_search::entry_price`); else the search, begun with this armour's price or joined
    /// where another armour's is under way, run with what is left of the poll's pops ([`ENTRY_POPS_A_POLL`]): not yet, if they
    /// run out first. Before the first poll (the load, a test) the search runs to its end at once, as before.
    pub fn entry_ask(
        &self,
        tactics: &Tactics,
        from: mapdata::NodeId,
        goal: mapdata::NodeId,
        side: i32,
        armour: Option<&ZoneArmour>,
        hurt: HurtTest,
    ) -> EntryAsk {
        use std::sync::atomic::Ordering;
        self.entry_asks.fetch_add(1, Ordering::Relaxed);
        let stamp = zones_stamp(&tactics.hazards);
        let key = (from, goal, side, armour.map_or(0, ZoneArmour::key), hurt);
        if let Some(way) = self.entries.peek(stamp.clone(), &key) {
            return EntryAsk::Way(way);
        }
        let shared = (from, goal, side, hurt);
        let mut slices = self.slices.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(way) = self.shared_entries.peek(stamp.clone(), &shared) {
            slices.counts.bound += 1;
            return EntryAsk::Way(way);
        }
        if !slices.made_for.iter().copied().eq(stamp.clone()) {
            slices.pending.clear();
            slices.made_for = stamp.clone().collect();
        }
        let version = tactics.hazards.version();
        let before = slices.pending.len();
        slices.pending.retain(|p| p.version == version);
        slices.counts.restarts += (before - slices.pending.len()) as u32;
        let at = match slices.pending.iter().position(|p| p.shared == shared) {
            Some(at) => at,
            None => {
                self.entry_searches.fetch_add(1, Ordering::Relaxed);
                let search = self.with_entry_steps(tactics, from, goal, side, |steps| {
                    crate::v026g::bots::zone_search::EntrySearch::begin(tactics, steps)
                });
                let asked = slices.poll;
                slices.pending.push(PendingEntry { version, shared, key, armour: armour.cloned(), search, asked, runs: 0 });
                slices.pending.len() - 1
            }
        };
        match self.run_pending(tactics, &mut slices, at) {
            Some(way) => {
                if slices_key_differs(key, &way.1) {
                    slices.counts.bound += 1;
                }
                EntryAsk::Way(way.0)
            }
            None => EntryAsk::Pending,
        }
    }

    /// [entry slices] Runs the search under way at `at` with the poll's pops: its way (kept for its armour and as every armour's
    /// bound) and the searcher's key, once it ends.
    fn run_pending(
        &self,
        tactics: &Tactics,
        slices: &mut EntrySlices,
        at: usize,
    ) -> Option<(Option<std::sync::Arc<Vec<mapdata::NodeId>>>, EntryKey)> {
        use crate::v026g::bots::zone_search::{EntryStep, PopBudget};
        let mut budget = slices.budget.map_or(PopBudget::Unbounded, PopBudget::Left);
        let grid = &tactics.world.grid;
        let poll = slices.poll;
        let step = {
            let pending = &mut slices.pending[at];
            pending.runs += 1;
            let PendingEntry { key, armour, search, .. } = pending;
            let (from, goal, side, hurt) = (key.0, key.1, key.2, key.4);
            let armour = armour.as_ref();
            self.with_entry_steps(tactics, from, goal, side, |steps| {
                search.run(tactics, steps, |node| tactics.hazards.leg_cost_per_s(grid, node, armour, hurt).most(), &mut budget)
            })
        };
        if let PopBudget::Left(left) = budget {
            slices.budget = Some(left);
        }
        let EntryStep::Done(way) = step else {
            return None;
        };
        let done = slices.pending.remove(at);
        let way = way.map(std::sync::Arc::new);
        let stamp = zones_stamp(&tactics.hazards);
        self.entries.insert(stamp.clone(), done.key, way.clone());
        self.shared_entries.insert(stamp, done.shared, way.clone());
        let waited = poll.saturating_sub(done.asked);
        let counts = &mut slices.counts;
        if done.runs > 1 {
            counts.sliced += 1;
        }
        counts.runs += done.runs;
        counts.waited_polls += waited;
        counts.longest_wait = counts.longest_wait.max(waited);
        Some((way, done.key))
    }

    /// [entry slices] A new poll: its pops ([`ENTRY_POPS_A_POLL`]), and the entry searches under way run with them, the oldest
    /// first, so each is answered by itself while nobody asks (the commander asks at its passes only).
    pub fn new_poll(&self, tactics: Option<&Tactics>, pops: u64) {
        let mut slices = self.slices.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        slices.poll += 1;
        slices.budget = Some(pops);
        let Some(tactics) = tactics else {
            return;
        };
        if !slices.made_for.iter().copied().eq(zones_stamp(&tactics.hazards)) {
            slices.pending.clear();
            return;
        }
        let version = tactics.hazards.version();
        let before = slices.pending.len();
        slices.pending.retain(|p| p.version == version);
        slices.counts.restarts += (before - slices.pending.len()) as u32;
        while !slices.pending.is_empty() && slices.budget.is_some_and(|left| left > 0) {
            if self.run_pending(tactics, &mut slices, 0).is_none() {
                break;
            }
        }
    }

    /// [entry slices] The match's slice counts.
    pub fn slice_counts(&self) -> SliceCounts {
        self.slices.lock().unwrap_or_else(std::sync::PoisonError::into_inner).counts
    }

    /// [`Self::entry`] for `key` (its armour `armour`): the answer, and whether the search ran (`searched` called).
    #[cfg(test)]
    fn entry_made(
        &self,
        tactics: &Tactics,
        key: EntryKey,
        armour: Option<&ZoneArmour>,
        searched: impl FnOnce(),
    ) -> (Option<std::sync::Arc<Vec<mapdata::NodeId>>>, bool) {
        let ran = std::cell::Cell::new(false);
        let (from, goal, side, _, hurt) = key;
        let answer = self.entries.get(zones_stamp(&tactics.hazards), key, || {
            ran.set(true);
            searched();
            let grid = &tactics.world.grid;
            let leg_per_s = |node| tactics.hazards.leg_cost_per_s(grid, node, armour, hurt).most();
            self.with_entry_steps(tactics, from, goal, side, |steps| {
                crate::v026g::bots::zone_search::cheapest_entry_on(tactics, steps, leg_per_s)
            })
            .map(std::sync::Arc::new)
        });
        (answer, ran.get())
    }

    /// [entry steps] `search` on the kept steps of the entry search from `from` to `goal` for side `side` made under the zones
    /// as they are now, else on new ones, kept after (the oldest let go past [`ENTRY_STEPS_KEPT`]).
    fn with_entry_steps<R>(
        &self,
        tactics: &Tactics,
        from: mapdata::NodeId,
        goal: mapdata::NodeId,
        side: i32,
        search: impl FnOnce(&mut crate::v026g::bots::zone_search::EntrySteps) -> R,
    ) -> R {
        use crate::v026g::bots::zone_search::EntrySteps;
        let within_m = super::collect::ENTRY_WITHIN_M;
        let version = tactics.hazards.version();
        let mut kept = self.entry_steps.lock().unwrap_or_else(PoisonError::into_inner);
        kept.retain(|k| k.version == version && k.made_for.iter().copied().eq(zones_stamp(&tactics.hazards)));
        let at = match kept.iter().position(|k| k.steps.of_search(from, goal, Some(side), within_m)) {
            Some(at) => {
                entry_steps_counts::reused();
                at
            }
            None => {
                entry_steps_counts::made();
                if kept.len() >= ENTRY_STEPS_KEPT {
                    kept.remove(0);
                }
                kept.push(KeptSteps { version, made_for: zones_stamp(&tactics.hazards).collect(), steps: EntrySteps::new(from, goal, Some(side), within_m) });
                kept.len() - 1
            }
        };
        search(&mut kept[at].steps)
    }

    /// [crossing prewarm] [`Self::entry`]'s asks in play and the searches made then (the rest were kept from the load).
    pub fn entry_counts(&self) -> (u32, u32) {
        use std::sync::atomic::Ordering;
        (self.entry_asks.load(Ordering::Relaxed), self.entry_searches.load(Ordering::Relaxed))
    }

    /// [crossing prewarm] The routed crossings' entry searches ([`Self::entry`]) made now, off the match's ticks, for every battery
    /// spawner's stand (`Tactics::stand_to_take` of its item) that needs a crossing ([`super::collect::needs_crossing_at`]) and that
    /// no straight walk crosses to ([`crossing_to`], from the spawner's item: the play's own test before a routed one), for each of
    /// the map's sides and each (armour, rule) in `asks` whose rule routes crossings. A battery the worlds show where its spawner put
    /// it asks the same floors, side, armour and rule: its first commander pass reads the kept way (Vostok batteries: 0.45-0.56 s
    /// at 2 s, the first pass's entry searches). Another ask still searches, as before: the answers are the search's either way.
    #[cfg(test)]
    pub fn prewarm_entries(&self, tactics: &Tactics, asks: &[(Option<std::sync::Arc<ZoneArmour>>, RoutedRule)]) -> EntryPrewarm {
        let started = std::time::Instant::now();
        let grid = &tactics.world.grid;
        let mut sides: Vec<i32> = Vec::new();
        for spawn in &tactics.world.map.spawns {
            if !sides.contains(&spawn.team) {
                sides.push(spawn.team);
            }
        }
        let mut out = EntryPrewarm { asks: asks.len(), sides: sides.len(), ..EntryPrewarm::default() };
        for spawner in &tactics.world.map.battery_spawners {
            let Some(stand) = tactics.stand_to_take(spawner.item) else {
                continue;
            };
            if !super::collect::needs_crossing_at(tactics, stand) {
                continue;
            }
            out.stands += 1;
            let stop_m = super::collect::stop_short(stand, spawner.item);
            let Some(goal) = grid.locate(stand, 3.0) else {
                continue;
            };
            for &side in &sides {
                let Some(from) = tactics.world.map.spawns.iter().find(|p| p.team == side).and_then(|p| grid.locate(p.position, 3.0)) else {
                    continue;
                };
                for (armour, rule) in asks {
                    if rule.route == RouteCrossing::Off {
                        continue;
                    }
                    let armour = armour.as_deref();
                    let straight = quiet(|| crossing_to(tactics, stand, Some(side), stop_m, armour, rule.hurt));
                    if straight.is_some() {
                        continue;
                    }
                    let key = (from, goal, side, armour.map_or(0, ZoneArmour::key), rule.hurt);
                    let (way, searched) = self.entry_made(tactics, key, armour, || {});
                    if searched {
                        out.searches += 1;
                        out.found += usize::from(way.is_some());
                    }
                }
            }
        }
        out.ms = started.elapsed().as_secs_f32() * 1000.0;
        out
    }
    /// [anomaly batteries] [`crate::v026g::bots::switch_take::way_in`], kept for each stand and side while the zones stay what
    /// they are and the live fields of its box stay what they were when it was made (`FieldBoard::fields_in`, read again only
    /// when the board's version moved): the answer a search gives now. [core ways] Not kept, the search begins (or goes on) with
    /// what is left of the poll's pops ([`WAY_POPS_A_POLL`]): not yet, if they run out first; before the first poll (the map's
    /// load, a test) it runs to its end.
    pub fn zone_way_ask(
        &self,
        tactics: &Tactics,
        core: crate::v026g::bots::anomalies::EnergyCore,
        side: i32,
        stand: mapdata::NodeId,
    ) -> ZoneWayAsk {
        let mut guard = self.zone_ways.lock().unwrap_or_else(PoisonError::into_inner);
        let ways: &mut ZoneWays = &mut guard;
        if ways.budget.is_some() {
            ways.counts.asks += 1;
        }
        Self::ways_for_stamp(tactics, ways);
        let key = (stand, side);
        let version = tactics.hazards.live.version();
        let stale = match ways.kept.get_mut(&key) {
            Some(kept) if kept.checked != version => {
                let same = kept.fields == way_fields(tactics, stand);
                if same {
                    kept.checked = version;
                }
                !same
            }
            _ => false,
        };
        if stale {
            ways.kept.remove(&key);
            ways.counts.dropped += 1;
        }
        if let Some(kept) = ways.kept.get(&key) {
            if ways.budget.is_some() {
                ways.counts.kept_answers += 1;
            }
            return ZoneWayAsk::Way(kept.way.clone());
        }
        let at = match ways.pending.iter().position(|p| p.key == key) {
            Some(at) => at,
            None => {
                if ways.budget.is_some() {
                    ways.counts.searches += 1;
                }
                let search = crate::v026g::bots::switch_take::WayInSearch::begin(tactics, core, side, stand, crate::v026g::bots::switch_take::WAY_WITHIN_M);
                let (fields, asked) = (way_fields(tactics, stand), ways.poll);
                ways.pending.push(PendingWay { key, search, fields, checked: version, asked, runs: 0 });
                ways.pending.len() - 1
            }
        };
        match Self::run_way(tactics, core, ways, at) {
            Some(way) => ZoneWayAsk::Way(way),
            None => ZoneWayAsk::Pending,
        }
    }

    /// [core ways] Drops every way and search when the zones' stamp is another.
    fn ways_for_stamp(tactics: &Tactics, ways: &mut ZoneWays) {
        if !ways.made_for.iter().copied().eq(zones_stamp(&tactics.hazards)) {
            ways.kept.clear();
            ways.pending.clear();
            ways.made_for = zones_stamp(&tactics.hazards).collect();
        }
    }

    /// [core ways] Runs the search under way at `at` with the poll's pops (begun again first if the live fields of its box
    /// changed under it): its way, kept, once it ends.
    fn run_way(
        tactics: &Tactics,
        core: crate::v026g::bots::anomalies::EnergyCore,
        ways: &mut ZoneWays,
        at: usize,
    ) -> Option<Option<std::sync::Arc<Vec<mapdata::NodeId>>>> {
        use crate::v026g::bots::switch_take::{WAY_WITHIN_M, WayInSearch, WayStep};
        use crate::v026g::bots::zone_search::PopBudget;
        let version = tactics.hazards.live.version();
        let mut pops = ways.budget.map_or(PopBudget::Unbounded, PopBudget::Left);
        let step = {
            let pending = &mut ways.pending[at];
            let (stand, side) = pending.key;
            let mut restarted = false;
            if pending.checked != version {
                let now = way_fields(tactics, stand);
                if now != pending.fields {
                    pending.search = WayInSearch::begin(tactics, core, side, stand, WAY_WITHIN_M);
                    pending.fields = now;
                    restarted = true;
                }
                pending.checked = version;
            }
            pending.runs += 1;
            let step = pending.search.run(tactics, core, &mut pops);
            (step, restarted)
        };
        if step.1 {
            ways.counts.restarts += 1;
        }
        if let PopBudget::Left(left) = pops {
            ways.budget = Some(left);
        }
        let WayStep::Done(way) = step.0 else {
            return None;
        };
        let done = ways.pending.remove(at);
        let way = way.map(std::sync::Arc::new);
        ways.kept.insert(done.key, KeptWay { way: way.clone(), fields: done.fields, checked: done.checked });
        if ways.budget.is_some() {
            let waited = ways.poll.saturating_sub(done.asked);
            let counts = &mut ways.counts;
            if done.runs > 1 {
                counts.sliced += 1;
            }
            counts.runs += done.runs;
            counts.waited_polls += waited;
            counts.longest_wait = counts.longest_wait.max(waited);
        }
        Some(way)
    }

    /// [core ways] The ways in of every battery spawner's stand that lies in an energy core's zones that switch, for each of the
    /// map's sides, worked out now (off the match's ticks; `zone_take::decide`'s question for a battery where its spawner put
    /// it): stands and sides asked, and the time.
    #[cfg(test)]
    pub fn prewarm_zone_ways(&self, tactics: &Tactics) -> (usize, f32) {
        let started = std::time::Instant::now();
        let grid = &tactics.world.grid;
        let mut sides: Vec<i32> = Vec::new();
        for spawn in &tactics.world.map.spawns {
            if !sides.contains(&spawn.team) {
                sides.push(spawn.team);
            }
        }
        let mut asked = 0;
        for spawner in &tactics.world.map.battery_spawners {
            let Some(stand) = tactics.stand_to_take(spawner.item).and_then(|stand| grid.locate(stand, 3.0)) else {
                continue;
            };
            let Some(core) = crate::v026g::bots::switch_take::core_of(tactics, stand) else {
                continue;
            };
            for &side in &sides {
                let _ = self.zone_way_ask(tactics, core, side, stand);
                asked += 1;
            }
        }
        let ms = started.elapsed().as_secs_f32() * 1000.0;
        let mut ways = self.zone_ways.lock().unwrap_or_else(PoisonError::into_inner);
        ways.counts.prewarmed = asked as u32;
        ways.counts.prewarm_ms = ms;
        (asked, ms)
    }

    /// [core ways] A new poll: its pops ([`WAY_POPS_A_POLL`]), and the way-in searches under way run with them, the oldest first.
    pub fn new_poll_ways(&self, tactics: Option<&Tactics>, pops: u64) {
        let mut guard = self.zone_ways.lock().unwrap_or_else(PoisonError::into_inner);
        let ways: &mut ZoneWays = &mut guard;
        ways.poll += 1;
        ways.budget = Some(pops);
        let Some(tactics) = tactics else {
            return;
        };
        Self::ways_for_stamp(tactics, ways);
        while !ways.pending.is_empty() && ways.budget.is_some_and(|left| left > 0) {
            let stand = ways.pending[0].key.0;
            let Some(core) = crate::v026g::bots::switch_take::core_of(tactics, stand) else {
                ways.pending.remove(0);
                continue;
            };
            if Self::run_way(tactics, core, ways, 0).is_none() {
                break;
            }
        }
    }

    /// [core ways] The match's counts.
    pub fn zone_way_counts(&self) -> ZoneWayCounts {
        self.zone_ways.lock().unwrap_or_else(PoisonError::into_inner).counts
    }

    /// [`crossing_to`], kept: the same answer, worked out once a match for
    /// each stand, side and stop while the zones stay what they are.
    pub fn crossing(
        &self,
        tactics: &Tactics,
        to: Vec3,
        side: Option<i32>,
        stop_m: f32,
        armour: Option<&ZoneArmour>,
        routed: RoutedRule,
    ) -> Option<Crossing> {
        self.crossing_ask(tactics, to, side, stop_m, armour, routed).answer()
    }

    /// [entry slices] [`Self::crossing`], or not yet: its routed crossing's entry search is under way ([`Self::entry_ask`]).
    /// No answer is kept while it is.
    pub fn crossing_ask(
        &self,
        tactics: &Tactics,
        to: Vec3,
        side: Option<i32>,
        stop_m: f32,
        armour: Option<&ZoneArmour>,
        routed: RoutedRule,
    ) -> CrossingAsk {
        // The armour is part of the question: crossings are worked out once a match for each kit's.
        let key = (
            to.x.to_bits(),
            to.y.to_bits(),
            to.z.to_bits(),
            side,
            stop_m.to_bits(),
            armour.map_or(0, ZoneArmour::key),
            routed,
        );
        let stamp = zones_stamp(&tactics.hazards);
        if let Some(found) = self.crossings.peek(stamp.clone(), &key) {
            return CrossingAsk::Known(found);
        }
        let straight = self.straights.get(stamp.clone(), key, || crossing_to(tactics, to, side, stop_m, armour, routed.hurt));
        let found = match straight {
            Some(found) => Some(found),
            None => match routed_crossing_ask(tactics, Some(self), to, side, stop_m, armour, routed) {
                CrossingAsk::Known(found) => found,
                CrossingAsk::Pending => return CrossingAsk::Pending,
            },
        };
        {
            #[cfg(test)]
            {
                use crate::v026g::bots::hazards::damage_zone_fixes::{Ev, count};
                count(Ev::CrossingAsked);
                match found {
                    None => count(Ev::CrossingNone),
                    Some(c) => {
                        count(Ev::CrossingFound);
                        if c.cost().most() == 0.0 {
                            count(Ev::CrossingFree);
                        }
                        if routed.hurt.by_intensity && crate::v026g::bots::hazards::damage_zone_fixes::comparing_full() {
                            let full = RoutedRule { hurt: HurtTest { by_intensity: false, ..routed.hurt }, ..routed };
                            let at_full = crate::v026g::bots::hazards::damage_zone_fixes::quietly(|| {
                                crossing_to(tactics, to, side, stop_m, armour, full.hurt)
                                    .or_else(|| super::collect::routed_crossing_to(tactics, to, side, stop_m, armour, full))
                            });
                            if at_full.is_some_and(|f| c.cost().most() < 0.99 * f.cost().most()) {
                                count(Ev::CrossingCheaper);
                            }
                        }
                    }
                }
            }
        }
        self.crossings.insert(stamp, key, found);
        CrossingAsk::Known(found)
    }

    /// An escort's post off the line (`escort::ring_post`), kept by the carrier's floor and the post asked for, for
    /// [`POST_KEEP_MS`]: the ring search (up to 80 floors and sight rays) runs when the carrier moves to another floor,
    /// not at every think of every escort.
    pub fn escort_post(
        &self,
        tactics: &Tactics,
        from: Vec3,
        wanted: Vec3,
        now: u32,
        side: Option<i32>,
        ask: impl FnOnce() -> Option<Vec3>,
    ) -> Option<Vec3> {
        let key = (
            tactics.world.grid.locate(from, 1.5),
            wanted.x.round() as i32,
            wanted.z.round() as i32,
            side,
            now / POST_KEEP_MS,
        );
        self.posts.get(zones_stamp(&tactics.hazards), key, ask)
    }

    /// [take.zone_look] Where to look at the spot `item` from when the battery there cannot be taken (a zone): the nearest floor
    /// within `within_m` across that is walkable, out of every zone now and sees the spot in a clear line from a crouched eye
    /// (12 ways round it, rings from 4 m out), kept while the zones stay what they are. [C6] A floor of the margin's band no zone
    /// lists a standing body on (`Hazards::band_stand`) is out of every zone too: the margin alone marked the floors round 21
    /// spawners on 7 maps, 12 of them where the client lists no zone at the spawner at all, and the search dropped them.
    pub fn zone_look(&self, tactics: &Tactics, item: Vec3, within_m: f32) -> Option<Vec3> {
        let key = (item.x.to_bits(), item.y.to_bits(), item.z.to_bits());
        self.zone_looks.get(zones_stamp(&tactics.hazards), key, || {
            let grid = &tactics.world.grid;
            let target = item + Vec3::new(0.0, 0.24, 0.0);
            let mut radius = 4.0f32;
            while radius <= within_m {
                let mut best: Option<(f32, Vec3)> = None;
                for k in 0..12 {
                    let a = k as f32 * std::f32::consts::TAU / 12.0;
                    let p = Vec3::new(item.x + radius * a.cos(), item.y, item.z + radius * a.sin());
                    let Some(node) = grid.locate(p, 1.5).or_else(|| grid.locate(p + Vec3::new(0.0, 3.0, 0.0), 1.5)) else {
                        continue;
                    };
                    if !tactics.walkable(node) || !(tactics.hazards.hazard_free(node) || tactics.hazards.band_stand(node)) {
                        continue;
                    }
                    let feet = grid.position(node);
                    let eye = crate::v026g::bots::view::still_eye(feet, true);
                    if !tactics.sees(eye, target) {
                        continue;
                    }
                    let d = feet.distance_xz(item);
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, feet));
                    }
                }
                if let Some((_, feet)) = best {
                    return Some(feet);
                }
                radius += 4.0;
            }
            None
        })
    }

    /// The stands of every container of the map, worked out at the first
    /// ask ([`UseStands::build`]: the game's use test, a few milliseconds a
    /// map).
    pub fn use_stands(&self, tactics: &Tactics) -> &UseStands {
        self.stands.get_or_init(|| {
            let ray_m = crate::v026g::bots::view::use_ray_m();
            log_stands_rule(ray_m);
            let (standing, crouched) = stand_eyes(crate::v026g::bots::view::live_eye());
            UseStands::build_ray(&tactics.world, standing, crouched, ray_m)
        })
    }

    /// Whether the use stands are worked out ([`Self::use_stands`] asked once): the load's prewarm test.
    #[cfg(test)]
    pub fn use_stands_built(&self) -> bool {
        self.stands.get().is_some()
    }

    /// Crossings kept.
    pub fn crossings_kept(&self) -> usize {
        self.crossings.kept()
    }
}

/// [sliced load] Floors the use stands' build may look at a poll (`mapdata::stands::UseStandsBuild::run`): a count, not a
/// clock, so runs repeat. The whole build took 100-101 ms here for 5,577 floors (batt-m2) and 7,428 (batt-m1), so about
/// 3.4-4.5 ms a poll.
pub const USE_FLOORS_A_POLL: u64 = 250;

/// [sliced load] The straight-walk checks the crossing entries' part may make a poll (`collect::crossing_to`, one for each
/// stand, side and (armour, rule) before a search is asked): batt-m1's 32 took 24.5 ms here at once, about 3 ms a poll.
pub const STRAIGHTS_A_POLL: u64 = 4;

/// [sliced load] What the load may do in one poll, of each kind: floors its way-in searches pop, floors the use stands' build
/// looks at, floors its entry searches pop, straight walks its entries' part checks.
#[derive(Clone, Copy, Debug)]
pub struct LoadBudget {
    pub way_pops: u64,
    pub stand_floors: u64,
    pub entry_pops: u64,
    pub straight_checks: u64,
}

impl LoadBudget {
    /// A server poll's: the in-play searches' budgets ([`WAY_POPS_A_POLL`], [`ENTRY_POPS_A_POLL`]), [`USE_FLOORS_A_POLL`] and
    /// [`STRAIGHTS_A_POLL`].
    pub const POLL: Self = Self {
        way_pops: WAY_POPS_A_POLL,
        stand_floors: USE_FLOORS_A_POLL,
        entry_pops: ENTRY_POPS_A_POLL,
        straight_checks: STRAIGHTS_A_POLL,
    };
}

/// The eye heights the use stands are made for: the measured ones with `live_eye` (`aim.combat.live_eye`), else the guessed
/// ones (the bench's off switch).
pub fn stand_eyes(live_eye: bool) -> ([f32; 2], [f32; 2]) {
    if live_eye { (EYE_STANDING, EYE_CROUCHED) } else { (GUESSED_EYE_STANDING, GUESSED_EYE_CROUCHED) }
}

/// The use stands' rule, for the log, once a match when they are made.
fn log_stands_rule(ray_m: f32) {
    log::info!(
        "bots: use stands within the view's elevations -{:.2} to +{:.2} degrees (the pitch unit's clamp, 0x2E0F60), ray {ray_m} m",
        mapdata::stands::VIEW_DOWN_DEG,
        mapdata::stands::VIEW_UP_DEG
    );
}

/// [sliced load] A routed crossing's entry search the load makes: its question and the armour it prices the way with.
type LoadEntry = (EntryKey, Option<std::sync::Arc<ZoneArmour>>);

/// [sliced load] An energy core's way in the load makes: the stand's floor and the side.
type LoadWay = (mapdata::NodeId, i32);

/// [sliced load] A straight-walk check before an entry search is asked: the stand, how far short of it the walk stops, the
/// search's question if no straight walk crosses, and the index of its (armour, rule) in the load's asks.
type LoadCheck = (Vec3, f32, EntryKey, usize);

/// [sliced load] Where a map's load is: its three parts in the order the load made them before, each with what is left of it.
enum LoadPart {
    /// The energy cores' ways in ([`Memo::zone_way_ask`]'s answers): the stands and sides left, and the search under way.
    CoreWays {
        left: std::collections::VecDeque<LoadWay>,
        running: Option<(LoadWay, crate::v026g::bots::switch_take::WayInSearch)>,
    },
    /// The containers' and resupply zones' use stands ([`Memo::use_stands`]'s).
    UseStands(Box<mapdata::stands::UseStandsBuild>),
    /// The routed crossings' entry searches ([`Memo::entry`]'s): the battery spawners left (indices of the map's), the
    /// straight-walk checks of the one looked at, the searches they asked, and the search under way.
    Entries {
        spawners: std::collections::VecDeque<usize>,
        checks: std::collections::VecDeque<LoadCheck>,
        left: std::collections::VecDeque<LoadEntry>,
        running: Option<(LoadEntry, crate::v026g::bots::zone_search::EntrySearch)>,
    },
    Done,
}

impl LoadPart {
    /// The part's name in the load's line.
    fn name(&self) -> &'static str {
        match self {
            Self::CoreWays { .. } => "the cores' ways in",
            Self::UseStands(_) => "the use stands",
            Self::Entries { .. } => "the crossing entries",
            Self::Done => "none",
        }
    }
}

/// [sliced load] The crossing entries' part under way, borrowed for a run.
struct EntryWork<'a> {
    spawners: &'a mut std::collections::VecDeque<usize>,
    checks: &'a mut std::collections::VecDeque<LoadCheck>,
    left: &'a mut std::collections::VecDeque<LoadEntry>,
    running: &'a mut Option<(LoadEntry, crate::v026g::bots::zone_search::EntrySearch)>,
}

/// [sliced load] What a map's load made and what it took, for its lines.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadCounts {
    /// Polls it ran in, the most one of them took (and the part it ran) and all of them, ms (the wall clock).
    pub polls: u32,
    pub most_ms: f32,
    pub most_in: &'static str,
    /// [CPU] The thread's CPU of the poll that took the most of it (and its part) and of all of them, ms (`cpu_time::thread`;
    /// TEST ONLY: the bench reads it, the server keeps the wall clock alone).
    #[cfg(test)]
    pub most_cpu_ms: f32,
    #[cfg(test)]
    pub most_cpu_in: &'static str,
    #[cfg(test)]
    pub cpu_ms: f32,
    pub work_ms: f32,
    /// The match clock of its first and its last poll.
    pub from_ms: Option<u32>,
    pub to_ms: u32,
    /// The energy cores' ways in asked (stands times sides), and their time.
    pub core_ways: usize,
    pub core_ways_ms: f32,
    /// The use stands: floors looked at, and their time.
    pub stand_floors: u64,
    pub stands_ms: f32,
    /// The routed crossings' entries.
    pub entries: EntryPrewarm,
}

/// [sliced load] The work a map's load does for the match's memo (the energy cores' ways in, the use stands, the routed
/// crossings' entries), run in parts by the polls before the bots enter ([`Self::run`]) with a budget of each kind a poll, or
/// whole. Its searches read the hazards as they were when it began, as the load read them when it made them at once, so its
/// answers are those: the same parts in the same order, each search the whole one in slices.
pub struct MapLoad {
    /// The hazards when it began: its searches read these, not the match's own, which go on with the clock and the worlds.
    hazards: Hazards,
    sides: Vec<i32>,
    asks: Vec<(Option<std::sync::Arc<ZoneArmour>>, RoutedRule)>,
    /// The use stands' eye heights and ray.
    stands: (([f32; 2], [f32; 2]), f32),
    part: LoadPart,
    counts: LoadCounts,
}

impl MapLoad {
    /// The load of `tactics` (its anomalies attached): the energy cores' ways in for each battery spawner whose stand lies in a
    /// core's switching zones and each of the map's sides; the use stands with `eyes` and the use ray `ray_m`; the routed
    /// crossings' entries for each (armour, rule) in `asks`.
    pub fn begin(
        tactics: &Tactics,
        asks: Vec<(Option<std::sync::Arc<ZoneArmour>>, RoutedRule)>,
        eyes: ([f32; 2], [f32; 2]),
        ray_m: f32,
    ) -> Self {
        let grid = &tactics.world.grid;
        let mut sides: Vec<i32> = Vec::new();
        for spawn in &tactics.world.map.spawns {
            if !sides.contains(&spawn.team) {
                sides.push(spawn.team);
            }
        }
        let mut left = std::collections::VecDeque::new();
        for spawner in &tactics.world.map.battery_spawners {
            let Some(stand) = tactics.stand_to_take(spawner.item).and_then(|stand| grid.locate(stand, 3.0)) else {
                continue;
            };
            if crate::v026g::bots::switch_take::core_of(tactics, stand).is_none() {
                continue;
            }
            left.extend(sides.iter().map(|&side| (stand, side)));
        }
        let counts = LoadCounts {
            entries: EntryPrewarm { asks: asks.len(), sides: sides.len(), ..EntryPrewarm::default() },
            ..LoadCounts::default()
        };
        Self {
            hazards: tactics.hazards.clone(),
            sides,
            asks,
            stands: (eyes, ray_m),
            part: LoadPart::CoreWays { left, running: None },
            counts,
        }
    }

    /// What it made and took so far.
    pub fn counts(&self) -> &LoadCounts {
        &self.counts
    }

    /// Runs the load at the match clock `clock` (its poll): with `budget` of each kind ([`LoadBudget::POLL`] a server poll's; a
    /// part that ends leaves the next to the next poll), or with none to its end. Whether it is done.
    pub fn run(&mut self, memo: &Memo, tactics: &mut Tactics, budget: Option<LoadBudget>, clock: u32) -> bool {
        let started = std::time::Instant::now();
        self.counts.polls += 1;
        self.counts.from_ms.get_or_insert(clock);
        self.counts.to_ms = clock;
        let part = self.part.name();
        #[cfg(test)]
        let cpu = crate::v026g::bots::cpu_time::thread();
        std::mem::swap(&mut tactics.hazards, &mut self.hazards);
        let done = self.run_parts(memo, tactics, budget);
        std::mem::swap(&mut tactics.hazards, &mut self.hazards);
        let ms = started.elapsed().as_secs_f32() * 1000.0;
        self.counts.work_ms += ms;
        if ms > self.counts.most_ms {
            self.counts.most_ms = ms;
            self.counts.most_in = part;
        }
        #[cfg(test)]
        {
            let cpu = crate::v026g::bots::cpu_time::thread().saturating_sub(cpu).as_secs_f32() * 1000.0;
            self.counts.cpu_ms += cpu;
            if cpu > self.counts.most_cpu_ms {
                self.counts.most_cpu_ms = cpu;
                self.counts.most_cpu_in = part;
            }
        }
        done
    }

    fn run_parts(&mut self, memo: &Memo, tactics: &Tactics, budget: Option<LoadBudget>) -> bool {
        use crate::v026g::bots::zone_search::PopBudget;
        let pops = |n: fn(&LoadBudget) -> u64| budget.as_ref().map_or(PopBudget::Unbounded, |b| PopBudget::Left(n(b)));
        loop {
            let started = std::time::Instant::now();
            let Self { hazards: _, sides, asks, stands, part, counts } = self;
            let ended = match part {
                LoadPart::CoreWays { left, running } => {
                    let ended = Self::run_ways(memo, tactics, left, running, &mut pops(|b| b.way_pops), counts);
                    counts.core_ways_ms += started.elapsed().as_secs_f32() * 1000.0;
                    ended
                }
                LoadPart::UseStands(build) => {
                    let mut floors = budget.map_or(u64::MAX, |b| b.stand_floors);
                    let made = build.run(&tactics.world, &mut floors);
                    counts.stand_floors = build.looked();
                    counts.stands_ms += started.elapsed().as_secs_f32() * 1000.0;
                    match made {
                        Some(made) => {
                            memo.keep_load_stands(made, stands.1);
                            true
                        }
                        None => false,
                    }
                }
                LoadPart::Entries { spawners, checks, left, running } => {
                    let mut pops = pops(|b| b.entry_pops);
                    let mut straights = budget.map_or(u64::MAX, |b| b.straight_checks);
                    let work = EntryWork { spawners, checks, left, running };
                    let ended = Self::run_entries(memo, tactics, sides, asks, work, &mut pops, &mut straights, &mut counts.entries);
                    counts.entries.ms += started.elapsed().as_secs_f32() * 1000.0;
                    ended
                }
                LoadPart::Done => return true,
            };
            if !ended {
                return false;
            }
            *part = match part {
                LoadPart::CoreWays { .. } => {
                    memo.set_way_prewarm(counts.core_ways, counts.core_ways_ms);
                    let ((standing, crouched), ray_m) = *stands;
                    LoadPart::UseStands(Box::new(mapdata::stands::UseStandsBuild::begin(standing, crouched, ray_m)))
                }
                LoadPart::UseStands(_) => LoadPart::Entries {
                    spawners: (0..tactics.world.map.battery_spawners.len()).collect(),
                    checks: Default::default(),
                    left: Default::default(),
                    running: None,
                },
                LoadPart::Entries { .. } | LoadPart::Done => LoadPart::Done,
            };
            if matches!(part, LoadPart::Done) {
                return true;
            }
            if budget.is_some() {
                return false;
            }
        }
    }

    /// The ways in left, one search at a time with `pops`: whether they are all made. A way kept already (two spawners on one
    /// stand) is not searched again, as the ask reads it.
    fn run_ways(
        memo: &Memo,
        tactics: &Tactics,
        left: &mut std::collections::VecDeque<LoadWay>,
        running: &mut Option<(LoadWay, crate::v026g::bots::switch_take::WayInSearch)>,
        pops: &mut crate::v026g::bots::zone_search::PopBudget,
        counts: &mut LoadCounts,
    ) -> bool {
        use crate::v026g::bots::switch_take::{WAY_WITHIN_M, WayInSearch, WayStep, core_of};
        loop {
            if running.is_none() {
                let Some((stand, side)) = left.pop_front() else {
                    return true;
                };
                counts.core_ways += 1;
                if memo.load_way_kept(tactics, (stand, side)) {
                    continue;
                }
                let Some(core) = core_of(tactics, stand) else {
                    continue;
                };
                *running = Some(((stand, side), WayInSearch::begin(tactics, core, side, stand, WAY_WITHIN_M)));
            }
            let Some((key, search)) = running.as_mut() else {
                continue;
            };
            let Some(core) = core_of(tactics, key.0) else {
                *running = None;
                continue;
            };
            match search.run(tactics, core, pops) {
                WayStep::Done(way) => {
                    memo.keep_load_way(tactics, *key, way);
                    *running = None;
                }
                WayStep::Pending => return false,
            }
        }
    }

    /// The entry searches left, one at a time with `budget`, the spawners' searches listed as [`Memo::prewarm_entries`] lists
    /// them, a spawner at a time (its straight-walk checks first, `straights` of them at most, then its searches): whether they
    /// are all made. A way kept already (the same question again) is not searched.
    #[allow(clippy::too_many_arguments)]
    fn run_entries(
        memo: &Memo,
        tactics: &Tactics,
        sides: &[i32],
        asks: &[(Option<std::sync::Arc<ZoneArmour>>, RoutedRule)],
        work: EntryWork<'_>,
        budget: &mut crate::v026g::bots::zone_search::PopBudget,
        straights: &mut u64,
        out: &mut EntryPrewarm,
    ) -> bool {
        use crate::v026g::bots::zone_search::{EntrySearch, EntryStep};
        let grid = &tactics.world.grid;
        let EntryWork { spawners, checks, left, running } = work;
        loop {
            if running.is_none() {
                if let Some(&(stand, stop_m, key, ask)) = checks.front() {
                    if *straights == 0 {
                        return false;
                    }
                    *straights -= 1;
                    checks.pop_front();
                    let (armour, rule) = &asks[ask];
                    let (_, _, side, _, _) = key;
                    let straight = quiet(|| crossing_to(tactics, stand, Some(side), stop_m, armour.as_deref(), rule.hurt));
                    if straight.is_none() {
                        left.push_back((key, armour.clone()));
                    }
                    continue;
                }
                let Some(entry) = left.pop_front() else {
                    let Some(at) = spawners.pop_front() else {
                        return true;
                    };
                    Self::spawner_entries(tactics, sides, asks, at, checks, out);
                    continue;
                };
                if memo.entries.peek(zones_stamp(&tactics.hazards), &entry.0).is_some() {
                    continue;
                }
                let (from, goal, side, _, _) = entry.0;
                let search = memo.with_entry_steps(tactics, from, goal, side, |steps| EntrySearch::begin(tactics, steps));
                *running = Some((entry, search));
            }
            let Some(((key, armour), search)) = running.as_mut() else {
                continue;
            };
            let (from, goal, side, _, hurt) = *key;
            let armour = armour.as_deref();
            let step = memo.with_entry_steps(tactics, from, goal, side, |steps| {
                search.run(tactics, steps, |node| tactics.hazards.leg_cost_per_s(grid, node, armour, hurt).most(), budget)
            });
            match step {
                EntryStep::Done(way) => {
                    out.searches += 1;
                    out.found += usize::from(way.is_some());
                    memo.entries.insert(zones_stamp(&tactics.hazards), *key, way.map(std::sync::Arc::new));
                    *running = None;
                }
                EntryStep::Pending => return false,
            }
        }
    }

    /// The straight-walk checks of the battery spawner at `at` (of the map's), as [`Memo::prewarm_entries`] makes them: its
    /// stand if it needs a crossing, for each side and each (armour, rule) that routes crossings; a check no straight walk
    /// passes asks its search.
    fn spawner_entries(
        tactics: &Tactics,
        sides: &[i32],
        asks: &[(Option<std::sync::Arc<ZoneArmour>>, RoutedRule)],
        at: usize,
        checks: &mut std::collections::VecDeque<LoadCheck>,
        out: &mut EntryPrewarm,
    ) {
        let grid = &tactics.world.grid;
        let spawner = &tactics.world.map.battery_spawners[at];
        let Some(stand) = tactics.stand_to_take(spawner.item) else {
            return;
        };
        if !super::collect::needs_crossing_at(tactics, stand) {
            return;
        }
        out.stands += 1;
        let stop_m = super::collect::stop_short(stand, spawner.item);
        let Some(goal) = grid.locate(stand, 3.0) else {
            return;
        };
        for &side in sides {
            let Some(from) = tactics.world.map.spawns.iter().find(|p| p.team == side).and_then(|p| grid.locate(p.position, 3.0)) else {
                continue;
            };
            for (ask, (armour, rule)) in asks.iter().enumerate() {
                if rule.route == RouteCrossing::Off {
                    continue;
                }
                let key = (from, goal, side, armour.as_deref().map_or(0, ZoneArmour::key), rule.hurt);
                checks.push_back((stand, stop_m, key, ask));
            }
        }
    }

    /// The parts of the load line, as the load's line gave them: the cores' ways in (when there were any), the use stands, the
    /// crossing entries (when a stand needed one).
    pub fn parts_line(&self, tactics: &Tactics) -> String {
        let c = &self.counts;
        let mut line = String::new();
        if c.core_ways > 0 {
            line += &format!("; core ways in prewarmed: {} stands and sides in {:.1} ms", c.core_ways, c.core_ways_ms);
        }
        let map = &tactics.world.map;
        line += &format!(
            "; use stands prewarmed in {:.1} ms ({} containers, {} zones; {} floors looked at)",
            c.stands_ms,
            map.containers.len(),
            map.zones.len(),
            c.stand_floors
        );
        let e = &c.entries;
        if e.stands > 0 {
            line += &format!(
                "; crossing entries made at load: {} searches ({} ways) for {} stands, {} sides, {} armour and rule pairs in {:.1} ms",
                e.searches, e.found, e.stands, e.sides, e.asks, e.ms
            );
        }
        line
    }

    /// The line of a load in polls, once it is done (in test builds with its thread CPU, after the wall times).
    pub fn done_line(&self, tactics: &Tactics) -> String {
        let c = &self.counts;
        #[cfg(test)]
        let cpu = format!(" (thread CPU: the most in one poll {:.1} ms ({}), {:.1} ms in all)", c.most_cpu_ms, c.most_cpu_in, c.cpu_ms);
        #[cfg(not(test))]
        let cpu = "";
        format!(
            "the map's load ran in {} polls, from {} to {} ms of the clock: the most in one poll {:.1} ms ({}), {:.1} ms in all{cpu}{}",
            c.polls,
            c.from_ms.unwrap_or(c.to_ms),
            c.to_ms,
            c.most_ms,
            c.most_in,
            c.work_ms,
            self.parts_line(tactics)
        )
    }
}

impl Memo {
    /// [sliced load] Whether the way in of `key` is kept (the zones' stamp first, as [`Self::zone_way_ask`] reads it).
    fn load_way_kept(&self, tactics: &Tactics, key: LoadWay) -> bool {
        let mut guard = self.zone_ways.lock().unwrap_or_else(PoisonError::into_inner);
        let ways: &mut ZoneWays = &mut guard;
        Self::ways_for_stamp(tactics, ways);
        ways.kept.contains_key(&key)
    }

    /// [sliced load] The way in of `key` the load's search made, kept as [`Self::zone_way_ask`] keeps a search's: with the live
    /// fields of its box and the board's version now.
    fn keep_load_way(&self, tactics: &Tactics, key: LoadWay, way: Option<Vec<mapdata::NodeId>>) {
        let mut guard = self.zone_ways.lock().unwrap_or_else(PoisonError::into_inner);
        let ways: &mut ZoneWays = &mut guard;
        Self::ways_for_stamp(tactics, ways);
        let kept = KeptWay { way: way.map(std::sync::Arc::new), fields: way_fields(tactics, key.0), checked: tactics.hazards.live.version() };
        ways.kept.insert(key, kept);
    }

    /// [sliced load] The ways in the load made and its time, for the match-end line ([`ZoneWayCounts`]).
    fn set_way_prewarm(&self, asked: usize, ms: f32) {
        let mut ways = self.zone_ways.lock().unwrap_or_else(PoisonError::into_inner);
        ways.counts.prewarmed = asked as u32;
        ways.counts.prewarm_ms = ms;
    }

    /// [sliced load] The use stands the load made, kept as [`Self::use_stands`] keeps its first ask's.
    fn keep_load_stands(&self, stands: UseStands, ray_m: f32) {
        log_stands_rule(ray_m);
        let _ = self.stands.set(stands);
    }

    /// [sliced load] The test of a load in parts: today's load (made at once: [`Self::prewarm_zone_ways`], [`Self::use_stands`],
    /// [`Self::prewarm_entries`], in that order) and [`MapLoad`] run in polls with `budget` keep the same, whole: every way in
    /// with its fields and version, the use stands, every entry. `between` runs on the tactics after each poll (a world, the
    /// clock): the load's searches read the hazards of its beginning, as the load made at once did. The load's polls.
    #[cfg(test)]
    pub(crate) fn assert_load_is_the_prewarm(
        tactics: &mut Tactics,
        asks: &[(Option<std::sync::Arc<ZoneArmour>>, RoutedRule)],
        budget: LoadBudget,
        mut between: impl FnMut(&mut Tactics, u32),
    ) -> LoadCounts {
        let ray_m = crate::v026g::bots::view::use_ray_m();
        let eyes = stand_eyes(crate::v026g::bots::view::live_eye());
        let whole = Memo::default();
        whole.prewarm_zone_ways(tactics);
        whole.use_stands(tactics);
        whole.prewarm_entries(tactics, asks);
        let at_once = Memo::default();
        let mut load = MapLoad::begin(tactics, asks.to_vec(), eyes, ray_m);
        assert!(load.run(&at_once, tactics, None, 0), "made at once in one run");
        assert_eq!(at_once.load_kept(), whole.load_kept(), "made at once");
        let sliced = Memo::default();
        let mut load = MapLoad::begin(tactics, asks.to_vec(), eyes, ray_m);
        let mut poll = 0;
        while !load.run(&sliced, tactics, Some(budget), poll) {
            poll += 1;
            assert!(poll < 10_000_000, "it ends");
            between(tactics, poll);
        }
        assert_eq!(sliced.load_kept(), whole.load_kept(), "in polls of {budget:?}");
        assert_eq!(load.counts().polls, poll + 1);
        load.counts().clone()
    }

    /// [sliced load] Everything the load's three parts keep, whole, for the tests: the ways in (with their fields and version),
    /// the use stands, the entries.
    #[cfg(test)]
    #[allow(clippy::type_complexity)]
    pub(crate) fn load_kept(
        &self,
    ) -> (
        Vec<(LoadWay, Option<std::sync::Arc<Vec<mapdata::NodeId>>>, Vec<crate::v026g::bots::live_zones::WorldField>, u64)>,
        Option<UseStands>,
        Vec<(EntryKey, Option<std::sync::Arc<Vec<mapdata::NodeId>>>)>,
    ) {
        let ways = self.zone_ways.lock().unwrap_or_else(PoisonError::into_inner);
        let mut kept: Vec<_> = ways.kept.iter().map(|(k, w)| (*k, w.way.clone(), w.fields.clone(), w.checked)).collect();
        kept.sort_by_key(|k| k.0);
        let mut entries = self.entries.all();
        entries.sort_by_cached_key(|e| format!("{:?}", e.0));
        (kept, self.stands.get().cloned(), entries)
    }
}

impl Ctx<'_> {
    /// The stands of container `container` (its id): floors from which the
    /// game's use ray meets its box, nearest the box first. One list for a
    /// put-in, a theft and the holder's sight.
    pub fn use_stands(&self, container: u8) -> &[UseStand] {
        self.memo.use_stands(self.tactics).of(container)
    }

    /// The stand of `container` in `pose` the bot reaches soonest
    /// (`path_dist`), if there is one, leaving out the floors in `passed`
    /// (stands it stood on and could not press from).
    pub fn nearest_use_stand(
        &self,
        container: u8,
        pose: UsePose,
        passed: &[Vec3],
    ) -> Option<UseStand> {
        self.use_stands(container)
            .iter()
            .filter(|stand| stand.fits(pose) && !passed.contains(&stand.feet))
            .min_by(|a, b| {
                self.path_dist(a.feet)
                    .total_cmp(&self.path_dist(b.feet))
                    .then(a.feet.x.total_cmp(&b.feet.x))
                    .then(a.feet.z.total_cmp(&b.feet.z))
            })
            .copied()
    }

    /// Where on the box of `container` a use ray from `eye` can meet it by
    /// the game's rule (a point of the box), if one can: the press test for
    /// the moment of the press. Out of reach of the box (its farthest
    /// corner and the ray) no ray is cast.
    pub fn use_press(&self, eye: Vec3, container: u8) -> Option<Vec3> {
        let shape = self
            .tactics
            .world
            .map
            .containers
            .iter()
            .find(|c| c.id == i32::from(container))?
            .use_shapes
            .first()?;
        self.press_on(eye, shape)
    }

    /// [`Ctx::use_press`] against the box of the resupply zone at `zone`
    /// (an index of `MapData::zones`).
    pub fn zone_press(&self, eye: Vec3, zone: usize) -> Option<Vec3> {
        let shape = self.tactics.world.map.zones.get(zone)?.shapes.first()?;
        self.press_on(eye, shape)
    }

    fn press_on(&self, eye: Vec3, shape: &mapdata::Shape) -> Option<Vec3> {
        if (shape.center - eye).length() > shape.half.length() + crate::v026g::bots::view::use_ray_m() {
            return None;
        }
        mapdata::stands::press(&self.tactics.world.sight, eye, shape, crate::v026g::bots::view::use_ray_m()).map(|(aim, _)| aim)
    }

    /// The stands of the resupply zone at `zone`: floors from which the
    /// game's use ray meets its box, nearest the box first.
    pub fn zone_stands(&self, zone: usize) -> &[UseStand] {
        self.memo.use_stands(self.tactics).of_zone(zone)
    }

    /// The stand of the resupply zone at `zone` the bot reaches soonest
    /// (`path_dist`), standing, leaving out the floors in `passed`.
    pub fn nearest_zone_stand(&self, zone: usize, passed: &[Vec3]) -> Option<UseStand> {
        self.zone_stands(zone)
            .iter()
            .filter(|stand| stand.fits(UsePose::Standing) && !passed.contains(&stand.feet))
            .min_by(|a, b| {
                self.path_dist(a.feet)
                    .total_cmp(&self.path_dist(b.feet))
                    .then(a.feet.x.total_cmp(&b.feet.x))
                    .then(a.feet.z.total_cmp(&b.feet.z))
            })
            .copied()
    }

    /// Whether the game's use ray from `eye` can meet the box of
    /// `container` ([`Ctx::use_press`]).
    pub fn use_press_ok(&self, eye: Vec3, container: u8) -> bool {
        self.use_press(eye, container).is_some()
    }
}

/// A storey, metres: floors this far apart are on different levels (the
/// grid's `locate` window; a ramp or a ledge is under it).
pub const STOREY_GAP_M: f32 = 2.5;

impl Ctx<'_> {
    /// The walk to `at` when the bot and `at` are a storey or more apart
    /// ([`STOREY_GAP_M`]): the regions' way between the floors
    /// ([`Tactics::walk_estimate`]) if longer than [`Ctx::straight_dist`]'s
    /// guess, which ignores the floor (a battery a storey up
    /// looked near to everyone under it: 105 plays on School's battery 3,
    /// 58 % of the Collect time in the match). `None` on one level, or
    /// without a floor under either end: the straight guess stands.
    pub fn storey_walk_dist(&self, at: Vec3) -> Option<f32> {
        let grid = &self.tactics.world.grid;
        let from = grid.locate(self.me.feet, 1.5)?;
        let to = crate::v026g::bots::path::goal_floor(grid, at)?;
        if (grid.position(from).y - grid.position(to).y).abs() < STOREY_GAP_M {
            return None;
        }
        let walk = self.tactics.walk_estimate(from, to)?;
        Some(walk.max(self.straight_dist(at)))
    }
}

#[cfg(test)]
mod tests {
    use mapdata::{Rotation, Shape, ShapeKind, Vec3, Zone, ZoneKind};

    use super::*;
    use crate::v026g::bots::anomalies::Control;

    /// The field with a zone of radius 4 m at (25, 32) that hurts `max_hit`
    /// a second, on all match.
    fn hurting(max_hit: f32) -> Tactics {
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            map.zones.push(Zone {
                kind: ZoneKind::Damage,
                name: "toxic".into(),
                team: -1,
                position: Vec3::new(25.0, 0.0, 32.0),
                protection: None,
                damage_type: "toxic".into(),
                damage: Some(mapdata::Damage {
                    max_hit,
                    interval_ms: 1000,
                    parts: 2,
                    ..mapdata::Damage::default()
                }),
                shapes: vec![Shape {
                    kind: ShapeKind::Sphere,
                    center: Vec3::new(25.0, 1.0, 32.0),
                    rotation: Rotation::IDENTITY,
                    half: Vec3::new(4.0, 4.0, 4.0),
                }],
            });
        });
        Tactics::new(world, 1.0)
    }

    /// A solid box (12 triangles) for the sight tree.
    fn solid_box(center: Vec3, half: Vec3) -> Vec<mapdata::Triangle> {
        let corner = |i: usize| {
            let pick = |bit: usize, h: f32| if i >> bit & 1 == 1 { h } else { -h };
            center + Vec3::new(pick(0, half.x), pick(1, half.y), pick(2, half.z))
        };
        [
            [0, 1, 3, 2],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 3, 7, 5],
        ]
        .iter()
        .flat_map(|&[a, b, c, d]| {
            [[a, b, c], [a, c, d]].map(|v| mapdata::Triangle {
                v: v.map(corner),
                walker: mapdata::WalkerBehaviour::BySlope,
            })
        })
        .collect()
    }

    /// The stands of a container come from the game's use test: the list is
    /// made once, nearest the box first, a bot asks for its nearest in a
    /// pose, and the press test agrees with the list.
    #[test]
    fn a_containers_stands_are_one_list_for_every_play() {
        use super::super::tests::Scenario;
        use crate::v026g::bots::view::Mode;
        let ours = Vec3::new(12.0, 0.0, 30.0);
        let middle = ours + Vec3::new(0.0, 1.0, 0.0);
        let half = Vec3::new(1.0, 0.85, 0.65);
        let body = Shape {
            kind: ShapeKind::Box,
            center: middle,
            rotation: Rotation::IDENTITY,
            half,
        };
        let world = crate::v026g::bots::tactics::tests::field_with_solids(
            &solid_box(middle, half),
            |map| {
                map.containers.push(mapdata::Container {
                    position: ours,
                    team: 0,
                    id: 0,
                    use_shapes: vec![body],
                });
            },
        );
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 30.0));
        s.tactics = Tactics::new(world, 1.0);
        let cx = s.cx(0);
        let list = cx.use_stands(0);
        assert!(!list.is_empty(), "a box on the field has stands");
        assert!(
            std::ptr::eq(list.as_ptr(), cx.use_stands(0).as_ptr()),
            "made once"
        );
        assert!(cx.use_stands(7).is_empty(), "no such container");
        let d: Vec<f32> = list.iter().map(|st| st.feet.distance_xz(middle)).collect();
        assert!(d.windows(2).all(|w| w[0] <= w[1]), "nearest first");
        let near = cx
            .nearest_use_stand(0, UsePose::Standing, &[])
            .expect("a stand to walk to");
        assert!(near.standing);
        assert!(
            near.feet.distance_xz(cx.me.feet)
                <= list
                    .iter()
                    .map(|st| st.feet.distance_xz(cx.me.feet))
                    .fold(f32::MAX, f32::min)
                    + 1.0,
            "the bot's nearest side"
        );
        assert!(cx.use_press_ok(near.feet + Vec3::new(0.0, 1.6, 0.0), 0));
        assert!(!cx.use_press_ok(Vec3::new(40.0, 1.6, 5.0), 0), "far away");
        assert!(
            !cx.use_press_ok(near.feet + Vec3::new(0.0, 1.6, 0.0), 9),
            "no such container"
        );
    }

    /// `CombatTuning::live_eye` off: a container's stands are the use test's at the old guessed eyes
    /// (`GUESSED_EYE_STANDING`, `GUESSED_EYE_CROUCHED`); on, at the game's (`EYE_STANDING`, `EYE_CROUCHED`).
    #[test]
    fn with_the_switch_off_the_stands_are_found_at_the_guessed_eyes() {
        use super::super::tests::Scenario;
        use crate::v026g::bots::view::{Mode, set_live_eye, use_ray_m};
        struct On;
        impl Drop for On {
            fn drop(&mut self) {
                set_live_eye(true);
            }
        }
        let _on = On;
        let ours = Vec3::new(12.0, 0.0, 30.0);
        let middle = ours + Vec3::new(0.0, 1.0, 0.0);
        let half = Vec3::new(1.0, 0.85, 0.65);
        let body = Shape { kind: ShapeKind::Box, center: middle, rotation: Rotation::IDENTITY, half };
        let world = crate::v026g::bots::tactics::tests::field_with_solids(&solid_box(middle, half), |map| {
            map.containers.push(mapdata::Container { position: ours, team: 0, id: 0, use_shapes: vec![body] });
        });
        let built = |on: bool| {
            set_live_eye(on);
            let mut s = Scenario::field(Mode::Batteries);
            s.bot(0, 0, Vec3::new(20.0, 0.0, 30.0));
            s.tactics = Tactics::new(world.clone(), 1.0);
            s.cx(0).use_stands(0).to_vec()
        };
        let (off, on) = (built(false), built(true));
        let guessed = UseStands::build_ray(&world, GUESSED_EYE_STANDING, GUESSED_EYE_CROUCHED, use_ray_m()).of(0).to_vec();
        let measured = UseStands::build_ray(&world, EYE_STANDING, EYE_CROUCHED, use_ray_m()).of(0).to_vec();
        assert_ne!(guessed, measured, "the premise: the two eyes find other stands at this box");
        assert_eq!(off, guessed);
        assert_eq!(on, measured);
    }

    const IN_ZONE: Vec3 = Vec3::new(25.0, 0.0, 32.0);

    /// Kept answers are the question's own, for each stand, side and stop
    /// (a crossing and none alike), and asked again they are read, not
    /// worked out.
    #[test]
    fn a_kept_crossing_is_the_one_worked_out() {
        let tactics = hurting(0.01);
        let memo = Memo::default();
        let asks = [
            (IN_ZONE, Some(0), 1.0),
            (IN_ZONE, Some(1), 1.0),
            (IN_ZONE, None, 0.0),
            (Vec3::new(26.0, 0.0, 33.0), Some(0), 1.0),
            // Dry floor far from the zone: still a question with an answer.
            (Vec3::new(10.0, 0.0, 10.0), Some(0), 0.0),
        ];
        assert!(crossing_to(&tactics, IN_ZONE, Some(0), 1.0, None, HurtTest::MARGIN).is_some());
        for round in 0..2 {
            for &(to, side, stop) in &asks {
                assert_eq!(
                    memo.crossing(&tactics, to, side, stop, None, RoutedRule::OFF),
                    crossing_to(&tactics, to, side, stop, None, HurtTest::MARGIN),
                    "round {round}: {to:?} {side:?} {stop}"
                );
            }
            assert_eq!(memo.crossings_kept(), asks.len());
        }
    }

    /// A zone that becomes another (its control at `Hazards::attach`, its
    /// hit) drops what was kept: the answers are worked out again for it.
    #[test]
    fn a_zone_changed_drops_the_kept_crossings() {
        let mut tactics = hurting(0.01);
        let memo = Memo::default();
        let mild = memo
            .crossing(&tactics, IN_ZONE, Some(0), 1.0, None, RoutedRule::OFF)
            .expect("a crossing");
        // Harsher.
        tactics.hazards.zones[0].damage = Some(mapdata::Damage {
            max_hit: 0.05,
            interval_ms: 1000,
            parts: 2,
            ..mapdata::Damage::default()
        });
        let harsh = memo.crossing(&tactics, IN_ZONE, Some(0), 1.0, None, RoutedRule::OFF);
        assert_eq!(harsh, crossing_to(&tactics, IN_ZONE, Some(0), 1.0, None, HurtTest::MARGIN));
        assert!(
            harsh.is_some_and(|h| h.leg.body > mild.leg.body),
            "{harsh:?} {mild:?}"
        );
        assert_eq!(memo.crossings_kept(), 1);
        // Switching (a timer's, as `Hazards::attach` makes it): its floor is
        // past the zones on all match now, and so a crossing's edge may be.
        tactics.hazards.zones[0].control = Control::Timer;
        assert_eq!(
            memo.crossing(&tactics, IN_ZONE, Some(0), 1.0, None, RoutedRule::OFF),
            crossing_to(&tactics, IN_ZONE, Some(0), 1.0, None, HurtTest::MARGIN)
        );
        assert_eq!(memo.crossings_kept(), 1);
    }

    /// The notes' `take_blocks` given the bot's candidates says what it says
    /// asking each battery afresh: offered, in a zone, too hurt for it.
    #[test]
    fn take_blocks_given_the_candidates_is_take_blocks_asked() {
        use crate::v026g::bots::plays::collect::{note_take_blocks, take_blocks};
        use crate::v026g::bots::plays::tests::Scenario;
        use crate::v026g::bots::view::{BatteryState, Mode};
        let mut s = Scenario::field(Mode::Batteries);
        s.tactics = hurting(0.01);
        s.battery(BatteryState::Free(IN_ZONE));
        s.battery(BatteryState::Free(Vec3::new(36.6, 0.0, 10.0)));
        s.battery(BatteryState::Free(Vec3::new(70.0, 0.0, 35.0)));
        for health in [1.0, 0.3, 0.05] {
            s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0)).health = health;
            let cx = s.cx(0);
            let mut offered = Vec::new();
            crate::v026g::bots::plays::candidates(&cx, &mut offered);
            let (mut given, mut asked) = (s.team.clone(), s.team.clone());
            assert_eq!(
                note_take_blocks(&mut given, 0, &take_blocks(&cx, &offered)),
                note_take_blocks(&mut asked, 0, &take_blocks(&cx, &[])),
                "health {health}"
            );
            assert_eq!(given.take_blocked, asked.take_blocked, "health {health}");
            assert!(!given.take_blocked.is_empty());
        }
    }

    /// Past [`MEMO_MOST`] answers it starts afresh, and still answers right.
    #[test]
    fn a_full_memo_starts_afresh() {
        let memo: QueryMemo<usize, usize> = QueryMemo::default();
        let asked = std::cell::Cell::new(0);
        let get = |stamp: u64, k: usize| {
            memo.get([stamp].into_iter(), k, || {
                asked.set(asked.get() + 1);
                k * 2
            })
        };
        for k in 0..MEMO_MOST + 3 {
            assert_eq!(get(7, k), k * 2);
        }
        assert_eq!((memo.kept(), asked.get()), (3, MEMO_MOST + 3));
        // Kept: read.
        assert_eq!(get(7, MEMO_MOST + 1), 2 * (MEMO_MOST + 1));
        assert_eq!(asked.get(), MEMO_MOST + 3);
        // Dropped: worked out again.
        assert_eq!(get(7, 1), 2);
        assert_eq!(asked.get(), MEMO_MOST + 4);
        // Another stamp: all dropped.
        assert_eq!(get(8, MEMO_MOST + 1), 2 * (MEMO_MOST + 1));
        assert_eq!((memo.kept(), asked.get()), (1, MEMO_MOST + 5));
    }

    /// A platform a storey up (3.4 m) over the middle of the field, its
    /// only way up a ramp at the far end (10 m wide): a battery on it, a bot on the
    /// ground right under it is a long walk from it, and the straight
    /// guess said ten metres (School's battery 3).
    #[test]
    fn a_battery_a_storey_up_is_as_far_as_the_way_up() {
        use super::super::tests::Scenario;
        use crate::v026g::bots::view::Mode;
        use mapdata::{Triangle, WalkerBehaviour};
        let tri = |v: [Vec3; 3]| Triangle {
            v,
            walker: WalkerBehaviour::BySlope,
        };
        let mut solids: Vec<Triangle> = crate::v026g::bots::body::tests::floor(40.0, 0.0, 60.0, 40.0, 3.4)
            .into_iter()
            .collect();
        let (a, b, c, d) = (
            Vec3::new(78.0, 0.0, 0.0),
            Vec3::new(78.0, 0.0, 10.0),
            Vec3::new(60.0, 3.4, 10.0),
            Vec3::new(60.0, 3.4, 0.0),
        );
        solids.extend([tri([a, b, c]), tri([a, c, d])]);
        let world = crate::v026g::bots::tactics::tests::field_with_solids(&solids, |_| {});
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(45.0, 0.0, 30.0));
        s.tactics = Tactics::new(world, 1.0);
        // It ships off; this is the switch on.
        assert!(!crate::v026g::bots::tuning::Tuning::all_off().grid.floor_paths);
        s.tuning.grid.floor_paths = true;
        let battery = Vec3::new(50.0, 3.4, 30.0);
        let cx = s.cx(0);
        let walk = cx.storey_walk_dist(battery).expect("a storey apart");
        assert!(walk > 40.0, "the way up is the ramp's: {walk}");
        assert!(cx.straight_dist(battery) < 10.0);
        assert_eq!(cx.path_dist(battery), walk, "path_dist is the floor-aware walk");
        // The bench's switch off: the straight ground distance.
        s.tuning.grid.floor_paths = false;
        assert_eq!(s.cx(0).path_dist(battery), s.cx(0).straight_dist(battery));
        s.tuning.grid.floor_paths = true;
        // On the platform, one level: the straight guess stands.
        s.view.places[0].feet = Vec3::new(45.0, 3.4, 30.0);
        assert_eq!(s.cx(0).storey_walk_dist(battery), None);
        // On the ground away from the platform, a battery on the ground.
        s.view.places[0].feet = Vec3::new(10.0, 0.0, 10.0);
        assert_eq!(s.cx(0).storey_walk_dist(Vec3::new(20.0, 0.0, 10.0)), None);
    }
}
