//! The stand-in's anomaly fields: a generator of the dynamic anomalies of the redesign, which the maps do not hold (the
//! bench has no world bytes, so it makes the typed fields the bots read: `Bots::fields_from_world`).
//!
//! It runs the release's rule on the match clock and its own seeded stream: the first try at 30 s, then one every 20 to
//! 40 s; a try draws a place that is no nearer than 15 m to a field (a resting one included) while fewer than 16 are
//! alive. A field is charging for 20 s (harmless), damaging for 4 s, then holds (the artefact waits, the zone stays on)
//! until the artefact is taken; it then goes, and its place rests 60 s. Nobody takes one in the bot matches
//! (`FIELD_TAKER=none`, the real situation: a field is permanent), or a scripted taker does after a hold
//! (`scripted`, as a human would).
//!
//! | switch | when unset |
//! |---|---|
//! | `SRP_BENCH_LIVE_FIELDS=<n>` | off; else at most `n` fields alive at once (the release: 16) |
//! | `SRP_BENCH_FIELD_TAKER` | `none`; `scripted` takes a field's artefact [`TAKE_AFTER_MS`] into its holding |
//! | `SRP_BENCH_FIELD_WORLD_MS` | [`WORLD_MS`]: how often a world lists the fields to the bots (a lag arm: 5000) |
//! | `SRP_BENCH_FIELD_RADIUS_M` | [`RADIUS_M`]: the fields' radius |
//!
//! The fields hurt the stand-in's bodies at the kind's hit while damaging or holding ([`Game::zone_tick`]) whatever the
//! bots know: the truth is kept apart from what the worlds showed (`lag`).

use super::*;

use std::collections::HashMap;

use mapdata::NodeId;

use survarium::v026g::match_msgs::anomaly_fields::{FieldId, FieldKind, FieldPhase, FieldShape, WorldField};

use crate::v026g::bots::live_zones::{hit_per_s, FieldBoard};

/// Charging, then damaging, ms (the release: 20 s, 4 s).
pub(crate) const CHARGE_MS: u32 = 20_000;
pub(crate) const DAMAGE_MS: u32 = 4_000;
/// A place rests this long after its artefact was taken, ms.
pub(crate) const REST_MS: u32 = 60_000;
/// The first try, and the gap between tries, ms.
pub(crate) const FIRST_TRY_MS: u32 = 30_000;
pub(crate) const TRY_GAP_MS: (u32, u32) = (20_000, 40_000);
/// A new field is no nearer than this to a field alive or resting, m.
pub(crate) const APART_M: f32 = 15.0;
/// How often a world lists the fields, ms (bot matches: 1-2 s).
pub(crate) const WORLD_MS: u32 = 1_000;
/// A scripted taker takes the artefact this long into the holding, ms.
pub(crate) const TAKE_AFTER_MS: u32 = 30_000;
/// A field's radius, m (the pack's: indoors 2.5, outdoors 4.0; the maps' places are not marked in or out).
pub(crate) const RADIUS_M: f32 = 3.0;
/// The places a try draws from lie this far apart, m, and this far from a spawn and a container.
const PLACE_GAP_M: f32 = 12.0;
const PLACE_CLEAR_M: f32 = 18.0;

/// Who takes a field's artefact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Taker {
    /// Nobody: a field stays for the match.
    None,
    /// After this long holding, ms.
    Scripted(u32),
}

/// One field alive.
#[derive(Clone, Debug)]
struct Truth {
    id: FieldId,
    at: Vec3,
    born: u32,
}

impl Truth {
    fn phase(&self, now: u32) -> FieldPhase {
        let age = now.saturating_sub(self.born);
        if age < CHARGE_MS {
            FieldPhase::Charging { damage_from: self.born + CHARGE_MS }
        } else if age < CHARGE_MS + DAMAGE_MS {
            FieldPhase::Damaging { until: Some(self.born + CHARGE_MS + DAMAGE_MS) }
        } else {
            FieldPhase::Holding { since: self.born + CHARGE_MS + DAMAGE_MS }
        }
    }

    fn held_for(&self, now: u32) -> Option<u32> {
        now.checked_sub(self.born + CHARGE_MS + DAMAGE_MS)
    }
}

/// Whether a phase hurts.
pub(crate) fn hurts(phase: FieldPhase) -> bool {
    !matches!(phase, FieldPhase::Charging { .. })
}

/// The generator's counts, for the tally (`tallies/route/fields.rs`).
#[derive(Clone, Debug, Default)]
pub(crate) struct FieldStats {
    /// Fields made.
    pub(crate) made: u32,
    /// For each field, from its making to the world that first listed it, ms.
    pub(crate) lag_ms: Vec<u32>,
    /// Field-milliseconds spent holding.
    pub(crate) held_ms: u64,
    /// The share of the grid's floors under a field: its largest, the sum of the samples, the samples.
    pub(crate) shut: (f32, f64, u32),
    /// What the fields took of the bodies: deaths, health (of 1.0 each).
    pub(crate) deaths: u32,
    pub(crate) hp: f32,
}

/// The generator.
pub(crate) struct FieldGen {
    max_active: usize,
    taker: Taker,
    world_ms: u32,
    radius: f32,
    places: Vec<Vec3>,
    rng: Rng,
    next_try: u32,
    active: Vec<Truth>,
    /// Places resting, till when.
    resting: Vec<(Vec3, u32)>,
    next_id: u32,
    /// The truth's floors, margin 0, for the hits ([`Self::hit_per_s_at`]).
    truth: FieldBoard,
    last_world: Option<u32>,
    first_shown: HashMap<FieldId, u32>,
    pub(crate) stats: FieldStats,
}

impl FieldGen {
    /// Off (no field ever), the default.
    pub(crate) fn off() -> Self {
        Self {
            max_active: 0,
            taker: Taker::None,
            world_ms: WORLD_MS,
            radius: RADIUS_M,
            places: Vec::new(),
            rng: Rng::new(0),
            next_try: u32::MAX,
            active: Vec::new(),
            resting: Vec::new(),
            next_id: 1,
            truth: FieldBoard::default(),
            last_world: None,
            first_shown: HashMap::new(),
            stats: FieldStats::default(),
        }
    }

    /// From the bench's switches, for a match on `world` of `seed`.
    pub(crate) fn from_env(world: &World, seed: u64) -> Self {
        let settings = bench_settings();
        let Some(max) = settings.number::<usize>("SRP_BENCH_LIVE_FIELDS").filter(|&n| n > 0) else {
            return Self::off();
        };
        let taker = match settings.var("SRP_BENCH_FIELD_TAKER").as_deref() {
            Ok("scripted") => Taker::Scripted(TAKE_AFTER_MS),
            _ => Taker::None,
        };
        Self::new(
            world,
            seed,
            max,
            taker,
            settings.number("SRP_BENCH_FIELD_WORLD_MS").unwrap_or(WORLD_MS),
            settings.number("SRP_BENCH_FIELD_RADIUS_M").unwrap_or(RADIUS_M),
        )
    }

    /// A generator of at most `max` fields at once, on `world`.
    pub(crate) fn new(world: &World, seed: u64, max: usize, taker: Taker, world_ms: u32, radius: f32) -> Self {
        Self {
            max_active: max,
            taker,
            world_ms,
            radius,
            places: places_of(world),
            rng: Rng::new(seed ^ 0xF1E1_D5A0_17E5),
            next_try: FIRST_TRY_MS,
            ..Self::off()
        }
    }

    pub(crate) fn on(&self) -> bool {
        self.max_active > 0
    }

    pub(crate) fn active(&self) -> usize {
        self.active.len()
    }

    /// The fields' lives to the match time `now`: artefacts taken, a try made; the truth's floors put right.
    pub(crate) fn tick(&mut self, now: u32, world: &World) {
        if !self.on() {
            return;
        }
        if let Taker::Scripted(after) = self.taker {
            let mut gone = Vec::new();
            self.active.retain(|f| {
                let taken = f.held_for(now).is_some_and(|held| held >= after);
                if taken {
                    gone.push(f.at);
                }
                !taken
            });
            self.resting.extend(gone.into_iter().map(|at| (at, now + REST_MS)));
        }
        self.try_one(now);
        self.finish(now, world);
    }

    fn try_one(&mut self, now: u32) {
        if now < self.next_try {
            return;
        }
        self.next_try = now + TRY_GAP_MS.0 + self.rng.below((TRY_GAP_MS.1 - TRY_GAP_MS.0) as usize) as u32;
        self.resting.retain(|&(_, until)| until > now);
        if self.active.len() >= self.max_active || self.places.is_empty() {
            return;
        }
        let free: Vec<Vec3> = self
            .places
            .iter()
            .copied()
            .filter(|p| {
                self.active.iter().all(|f| f.at.distance_xz(*p) >= APART_M)
                    && self.resting.iter().all(|(r, _)| r.distance_xz(*p) >= APART_M)
            })
            .collect();
        if free.is_empty() {
            return;
        }
        let at = free[self.rng.below(free.len())];
        self.active.push(Truth { id: FieldId(self.next_id), at, born: now });
        self.next_id += 1;
        self.stats.made += 1;
    }

    fn finish(&mut self, now: u32, world: &World) {
        self.stats.held_ms += self.active.iter().filter(|f| f.held_for(now).is_some()).count() as u64
            * u64::from(TICK_MS);
        let fields = self.fields(now);
        self.truth.update(&fields, &world.grid, 0.0, now);
    }

    /// The fields as a world lists them at `now`.
    fn fields(&self, now: u32) -> Vec<WorldField> {
        self.active
            .iter()
            .map(|f| WorldField {
                id: f.id,
                kind: FieldKind::Lifebone,
                centre: [f.at.x, f.at.y + 1.0, f.at.z],
                shape: FieldShape::Cylinder { radius: self.radius, half_height: 2.0 },
                phase: f.phase(now),
            })
            .collect()
    }

    /// The fields a world of this tick lists, if one is due (every `world_ms`); the lag of those it lists first.
    pub(crate) fn world_due(&mut self, now: u32) -> Option<Vec<WorldField>> {
        if !self.on() || self.last_world.is_some_and(|at| now < at + self.world_ms) {
            return None;
        }
        self.last_world = Some(now);
        let listed = self.fields(now);
        for field in &listed {
            if let std::collections::hash_map::Entry::Vacant(entry) = self.first_shown.entry(field.id) {
                entry.insert(now);
                let born = self.active.iter().find(|f| f.id == field.id).map_or(now, |f| f.born);
                self.stats.lag_ms.push(now - born);
            }
        }
        Some(listed)
    }

    /// The samples of the share of floors under a field, once a world.
    pub(crate) fn sample_shut(&mut self, floors: usize) {
        if !self.on() || floors == 0 {
            return;
        }
        let share = self.truth.floors_covered() as f32 / floors as f32;
        let shut = &mut self.stats.shut;
        shut.0 = shut.0.max(share);
        shut.1 += f64::from(share);
        shut.2 += 1;
    }

    /// The hit a second at a floor, health of 1.0: the fields there that hurt now.
    pub(crate) fn hit_per_s_at(&self, node: NodeId) -> f32 {
        self.truth
            .at(node)
            .filter(|f| hurts(f.phase()))
            .map(|f| hit_per_s(f.kind))
            .sum()
    }

    /// The field on a floor and whether it hurts now (one that hurts first).
    pub(crate) fn here(&self, node: NodeId) -> Option<(FieldId, bool)> {
        self.truth
            .at(node)
            .map(|f| (f.id, hurts(f.phase())))
            .max_by_key(|&(_, hurts)| hurts)
    }
}

/// The places a try draws from: floors of the grid [`PLACE_GAP_M`] apart and [`PLACE_CLEAR_M`] from the spawns and
/// containers (the map holds the count of the artefact containers, not their places).
fn places_of(world: &World) -> Vec<Vec3> {
    let grid = &world.grid;
    let avoid: Vec<Vec3> = world
        .map
        .spawns
        .iter()
        .map(|s| s.position)
        .chain(world.map.containers.iter().map(|c| c.use_point()))
        .collect();
    let mut taken: HashMap<(i32, i32), Vec<Vec3>> = HashMap::new();
    let mut places = Vec::new();
    for node in 0..grid.len() as NodeId {
        if !grid.can_stand(node) {
            continue;
        }
        let at = grid.position(node);
        if avoid.iter().any(|a| a.distance_xz(at) < PLACE_CLEAR_M) {
            continue;
        }
        let cell = ((at.x / PLACE_GAP_M).floor() as i32, (at.z / PLACE_GAP_M).floor() as i32);
        let near = (-1..=1).any(|dx| {
            (-1..=1).any(|dz| {
                taken
                    .get(&(cell.0 + dx, cell.1 + dz))
                    .is_some_and(|ps| ps.iter().any(|p| p.distance_xz(at) < PLACE_GAP_M))
            })
        });
        if !near {
            taken.entry(cell).or_default().push(at);
            places.push(at);
        }
    }
    places
}


#[cfg(test)]
mod tests {
    use super::*;

    use crate::v026g::bots::tactics::tests::field_with;

    fn run(gen: &mut FieldGen, world: &World, to: u32) {
        let mut now = 0;
        while now <= to {
            gen.tick(now, world);
            now += TICK_MS;
        }
    }

    /// The release's rule: nothing before 30 s; a field charges 20 s, damages 4 s, then holds; no two within 15 m;
    /// never more than the cap; nobody takes one, so it stays.
    #[test]
    fn the_generator_follows_the_release_rule() {
        let world = field_with(|_| {});
        let mut gen = FieldGen::new(&world, 7, 3, Taker::None, WORLD_MS, RADIUS_M);
        run(&mut gen, &world, FIRST_TRY_MS - TICK_MS);
        assert_eq!(gen.active(), 0, "nothing before the first try");
        run(&mut gen, &world, 400_000);
        assert!((1..=3).contains(&gen.active()), "{}", gen.active());
        assert_eq!(gen.active(), 3.min(gen.stats.made as usize), "the cap holds, and fields stay with no taker");
        for (i, a) in gen.active.iter().enumerate() {
            assert!(a.born >= FIRST_TRY_MS);
            for b in &gen.active[i + 1..] {
                assert!(a.at.distance_xz(b.at) >= APART_M, "{:?} {:?}", a.at, b.at);
            }
        }
        let f = &gen.active[0];
        assert_eq!(f.phase(f.born), FieldPhase::Charging { damage_from: f.born + CHARGE_MS });
        assert_eq!(f.phase(f.born + CHARGE_MS), FieldPhase::Damaging { until: Some(f.born + CHARGE_MS + DAMAGE_MS) });
        assert!(matches!(f.phase(f.born + CHARGE_MS + DAMAGE_MS), FieldPhase::Holding { .. }));
        assert!(gen.stats.held_ms > 0);
    }

    /// A scripted taker takes the artefact 30 s into the holding: the field goes and its place rests a minute.
    #[test]
    fn a_taken_field_goes_and_its_place_rests() {
        let world = field_with(|_| {});
        let mut gen = FieldGen::new(&world, 7, 1, Taker::Scripted(TAKE_AFTER_MS), WORLD_MS, RADIUS_M);
        let mut now = 0;
        let mut born = None;
        while now < 200_000 {
            gen.tick(now, &world);
            if born.is_none() && gen.active() == 1 {
                born = Some((now, gen.active[0].at));
            }
            now += TICK_MS;
        }
        let (at, place) = born.expect("a field was made");
        assert!(at >= FIRST_TRY_MS);
        // It was taken at born + 20 + 4 + 30 s, and the place rests 60 s after.
        assert!(gen.stats.made >= 1);
        let gone = at + CHARGE_MS + DAMAGE_MS + TAKE_AFTER_MS;
        assert!(gen.resting.iter().any(|&(p, until)| p == place && until >= gone + REST_MS - TICK_MS) || gen.active.iter().all(|f| f.born > at));
    }

    /// The world lists the fields every `world_ms`, and the lag is from the making to the first listing.
    #[test]
    fn worlds_list_the_fields_and_the_lag_is_counted() {
        let world = field_with(|_| {});
        let mut gen = FieldGen::new(&world, 3, 4, Taker::None, 5_000, RADIUS_M);
        let mut now = 0;
        let mut listings = 0;
        while now < 120_000 {
            gen.tick(now, &world);
            if let Some(listed) = gen.world_due(now) {
                listings += 1;
                assert_eq!(listed.len(), gen.active());
            }
            now += TICK_MS;
        }
        assert_eq!(listings, 120_000 / 5_000 + 1);
        assert!(!gen.stats.lag_ms.is_empty());
        assert!(gen.stats.lag_ms.iter().all(|&lag| lag <= 5_000), "{:?}", gen.stats.lag_ms);
    }

    /// A field hurts a floor under it only once it damages: the charging one is harmless.
    #[test]
    fn a_field_hurts_only_once_it_damages() {
        let world = field_with(|_| {});
        let mut gen = FieldGen::new(&world, 11, 1, Taker::None, WORLD_MS, RADIUS_M);
        let mut now = 0;
        while gen.active() == 0 {
            gen.tick(now, &world);
            now += TICK_MS;
        }
        let f = gen.active[0].clone();
        let node = world.grid.locate(f.at, 0.5).unwrap();
        assert_eq!(gen.hit_per_s_at(node), 0.0, "charging");
        assert_eq!(gen.here(node), Some((f.id, false)));
        while now < f.born + CHARGE_MS + TICK_MS {
            gen.tick(now, &world);
            now += TICK_MS;
        }
        assert_eq!(gen.hit_per_s_at(node), 0.5);
        assert_eq!(gen.here(node), Some((f.id, true)));
        gen.sample_shut(world.grid.len());
        assert!(gen.stats.shut.0 > 0.0);
    }
}
