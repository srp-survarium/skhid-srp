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
//! | `SRP_BENCH_FIELD_PLACES` | `spread`: floors of the map [`PLACE_GAP_M`] apart; `lanes`: points on the walks between the containers and the battery spawners (where the bots go, so a field costs them something) |
//! | `SRP_BENCH_FIELD_PACE` | the release's; `fast`: the first try at 5 s, then every 4-8 s (a stress arm: more fields in a short match) |
//!
//! The fields hurt the stand-in's bodies at the kind's hit while damaging or holding ([`Game::zone_tick`]) whatever the
//! bots know: the truth is kept apart from what the worlds showed (`lag`).

use super::*;

use std::collections::HashMap;

use mapdata::NodeId;

use crate::v026g::bots::live_zones::{self, hit_per_s, FieldBoard, FieldId, FieldKind, FieldPhase, WorldField};

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

/// How often a try is made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Pace {
    /// The release's: the first at 30 s, then every 20-40 s.
    Release,
    /// A stress: the first at 5 s, then every 4-8 s.
    Fast,
}

impl Pace {
    fn first(self) -> u32 {
        match self {
            Self::Release => FIRST_TRY_MS,
            Self::Fast => 5_000,
        }
    }

    fn gap(self) -> (u32, u32) {
        match self {
            Self::Release => TRY_GAP_MS,
            Self::Fast => (4_000, 8_000),
        }
    }
}

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
            FieldPhase::Holding
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
    pace: Pace,
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
            pace: Pace::Release,
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

    /// A generator that tries at the release's pace, or [`Pace::Fast`].
    pub(crate) fn paced(mut self, pace: Pace) -> Self {
        self.pace = pace;
        self.next_try = pace.first();
        self
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
        let mut generator = Self::new(
            world,
            seed,
            max,
            taker,
            settings.number("SRP_BENCH_FIELD_WORLD_MS").unwrap_or(WORLD_MS),
            settings.number("SRP_BENCH_FIELD_RADIUS_M").unwrap_or(RADIUS_M),
        );
        if settings.var("SRP_BENCH_FIELD_PLACES").as_deref() == Ok("lanes") {
            generator.places = lanes_of(world);
        }
        if settings.var("SRP_BENCH_FIELD_PACE").as_deref() == Ok("fast") {
            generator = generator.paced(Pace::Fast);
        }
        generator
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

    /// The places a try draws from.
    pub(crate) fn places(&self) -> &[Vec3] {
        &self.places
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
        let gap = self.pace.gap();
        self.next_try = now + gap.0 + self.rng.below((gap.1 - gap.0) as usize) as u32;
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
                shape: mapdata::Shape {
                    kind: mapdata::ShapeKind::Cylinder,
                    center: Vec3::new(f.at.x, f.at.y + 1.0, f.at.z),
                    rotation: mapdata::Rotation::IDENTITY,
                    half: Vec3::new(self.radius, 2.0, self.radius),
                },
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


/// `world` with a damage zone, always on, at each of `places` (the baked-all alternative to the board, for what it
/// costs: `SRP_BENCH_BAKE_FIELD_PLACES`): the tactics' build, their regions and the searches pay for zones that
/// exist all match. The bots' behaviour is not the live zones' (they keep off every one for the whole match).
pub(crate) fn baked_places(world: &World, places: &[Vec3], radius: f32) -> World {
    use mapdata::{Damage, Rotation, Shape, ShapeKind, Zone, ZoneKind};
    let mut map = world.map.clone();
    for (k, at) in places.iter().enumerate() {
        map.zones.push(Zone {
            kind: ZoneKind::Damage,
            name: format!("baked_field_{k}"),
            team: -1,
            position: *at,
            damage_type: "toxic".into(),
            damage: Some(Damage { max_hit: hit_per_s(FieldKind::Lifebone) / 2.0, parts: 2, continuous: true, ..Damage::default() }),
            shapes: vec![Shape {
                kind: ShapeKind::Cylinder,
                center: Vec3::new(at.x, at.y + 1.0, at.z),
                rotation: Rotation::IDENTITY,
                half: Vec3::new(radius, 2.0, radius),
            }],
        });
    }
    World::assemble(map, world.grid.clone(), world.sight.clone())
}

/// The places on the walks bots make: every container to every battery spawner and to the other side's container, a point
/// every [`PLACE_GAP_M`] along each walk, and none within [`PLACE_CLEAR_M`] of a spawn or container.
fn lanes_of(world: &World) -> Vec<Vec3> {
    let grid = &world.grid;
    let containers: Vec<Vec3> = world.map.containers.iter().map(|c| c.use_point()).collect();
    let spawners: Vec<Vec3> = world.map.battery_spawners.iter().map(|s| s.item).collect();
    let avoid: Vec<Vec3> = world.map.spawns.iter().map(|s| s.position).chain(containers.iter().copied()).collect();
    let mut places: Vec<Vec3> = Vec::new();
    let ends: Vec<(Vec3, Vec3)> = containers
        .iter()
        .flat_map(|&c| spawners.iter().map(move |&s| (c, s)))
        .chain(containers.iter().flat_map(|&a| containers.iter().map(move |&b| (a, b))))
        .collect();
    for (a, b) in ends {
        let (Some(from), Some(to)) = (grid.locate(a, 3.0), grid.locate(b, 3.0)) else {
            continue;
        };
        let Some(path) = grid.path(from, to) else {
            continue;
        };
        let mut last: Option<Vec3> = None;
        for node in path.nodes {
            let at = grid.position(node);
            if last.is_some_and(|l| l.distance_xz(at) < PLACE_GAP_M) {
                continue;
            }
            last = Some(at);
            if avoid.iter().any(|v| v.distance_xz(at) < PLACE_CLEAR_M) || places.iter().any(|p| p.distance_xz(at) < PLACE_GAP_M) {
                continue;
            }
            places.push(at);
        }
    }
    places
}

/// The line of what the live zones cost (the bench prints it after the `routes:` line): the tactics' build (the
/// regions, the zones baked, the floors they mark), what a changed board cost to make (`field_board_build_ms`), and
/// what one `allowed_for` costs on a floor no zone marks and on one a live field does (`field_query_ns`).
pub(crate) fn cost_line(bots: &Bots) -> String {
    let Some(tactics) = bots.tactics.as_ref() else {
        return String::new();
    };
    let hazards = &tactics.hazards;
    let mut builds = live_zones::timing::take();
    builds.sort_unstable();
    let ms = |us: u32| f64::from(us) / 1000.0;
    let grid = &tactics.world.grid;
    let pass = hazards.pass_ms;
    let query = |nodes: &[NodeId]| -> f64 {
        if nodes.is_empty() {
            return 0.0;
        }
        const REPS: usize = 200;
        let started = std::time::Instant::now();
        let mut shut = 0usize;
        for _ in 0..REPS {
            for &node in nodes {
                shut += usize::from(!std::hint::black_box(hazards.allowed_for(node, 100_000, pass)));
            }
        }
        std::hint::black_box(shut);
        started.elapsed().as_nanos() as f64 / (REPS * nodes.len()) as f64
    };
    let stride = (grid.len() / 4096).max(1);
    let free: Vec<NodeId> = (0..grid.len() as NodeId).step_by(stride).filter(|&n| hazards.set_of(n).is_none()).collect();
    let marked: Vec<NodeId> = (0..grid.len() as NodeId).filter(|&n| hazards.live.set_of(n) != 0).take(4096).collect();
    format!(
        "live zones: tactics_regions {} hazard_zones {} marked_floors {} field_board_builds {} field_board_build_ms_p50 {:.3} field_board_build_ms_max {:.3} field_query_ns_free {:.0} field_query_ns_marked {:.0}",
        tactics.regions().len(),
        hazards.zones.len(),
        hazards.marked(),
        builds.len(),
        builds.get(builds.len() / 2).map_or(0.0, |&us| ms(us)),
        builds.last().map_or(0.0, |&us| ms(us)),
        query(&free),
        query(&marked)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::v026g::bots::tactics::tests::field_with;

    fn run(fg: &mut FieldGen, world: &World, to: u32) {
        let mut now = 0;
        while now <= to {
            fg.tick(now, world);
            now += TICK_MS;
        }
    }

    /// The release's rule: nothing before 30 s; a field charges 20 s, damages 4 s, then holds; no two within 15 m;
    /// never more than the cap; nobody takes one, so it stays.
    #[test]
    fn the_generator_follows_the_release_rule() {
        let world = field_with(|_| {});
        let mut fg = FieldGen::new(&world, 7, 3, Taker::None, WORLD_MS, RADIUS_M);
        run(&mut fg, &world, FIRST_TRY_MS - TICK_MS);
        assert_eq!(fg.active(), 0, "nothing before the first try");
        run(&mut fg, &world, 400_000);
        assert!((1..=3).contains(&fg.active()), "{}", fg.active());
        assert_eq!(fg.active(), 3.min(fg.stats.made as usize), "the cap holds, and fields stay with no taker");
        for (i, a) in fg.active.iter().enumerate() {
            assert!(a.born >= FIRST_TRY_MS);
            for b in &fg.active[i + 1..] {
                assert!(a.at.distance_xz(b.at) >= APART_M, "{:?} {:?}", a.at, b.at);
            }
        }
        let f = &fg.active[0];
        assert_eq!(f.phase(f.born), FieldPhase::Charging { damage_from: f.born + CHARGE_MS });
        assert_eq!(f.phase(f.born + CHARGE_MS), FieldPhase::Damaging { until: Some(f.born + CHARGE_MS + DAMAGE_MS) });
        assert!(matches!(f.phase(f.born + CHARGE_MS + DAMAGE_MS), FieldPhase::Holding));
        assert!(fg.stats.held_ms > 0);
    }

    /// A scripted taker takes the artefact 30 s into the holding: the field goes and its place rests a minute.
    #[test]
    fn a_taken_field_goes_and_its_place_rests() {
        let world = field_with(|_| {});
        let mut fg = FieldGen::new(&world, 7, 1, Taker::Scripted(TAKE_AFTER_MS), WORLD_MS, RADIUS_M);
        let mut now = 0;
        let mut born = None;
        while now < 200_000 {
            fg.tick(now, &world);
            if born.is_none() && fg.active() == 1 {
                born = Some((now, fg.active[0].at));
            }
            now += TICK_MS;
        }
        let (at, place) = born.expect("a field was made");
        assert!(at >= FIRST_TRY_MS);
        // It was taken at born + 20 + 4 + 30 s, and the place rests 60 s after.
        assert!(fg.stats.made >= 1);
        let gone = at + CHARGE_MS + DAMAGE_MS + TAKE_AFTER_MS;
        assert!(fg.resting.iter().any(|&(p, until)| p == place && until >= gone + REST_MS - TICK_MS) || fg.active.iter().all(|f| f.born > at));
    }

    /// The world lists the fields every `world_ms`, and the lag is from the making to the first listing.
    #[test]
    fn worlds_list_the_fields_and_the_lag_is_counted() {
        let world = field_with(|_| {});
        let mut fg = FieldGen::new(&world, 3, 4, Taker::None, 5_000, RADIUS_M);
        let mut now = 0;
        let mut listings = 0;
        while now < 120_000 {
            fg.tick(now, &world);
            if let Some(listed) = fg.world_due(now) {
                listings += 1;
                assert_eq!(listed.len(), fg.active());
            }
            now += TICK_MS;
        }
        assert_eq!(listings, 120_000 / 5_000, "a world at 0, 5 s, ... 115 s");
        assert!(!fg.stats.lag_ms.is_empty());
        assert!(fg.stats.lag_ms.iter().all(|&lag| lag <= 5_000), "{:?}", fg.stats.lag_ms);
    }

    /// A field hurts a floor under it only once it damages: the charging one is harmless.
    #[test]
    fn a_field_hurts_only_once_it_damages() {
        let world = field_with(|_| {});
        let mut fg = FieldGen::new(&world, 11, 1, Taker::None, WORLD_MS, RADIUS_M);
        let mut now = 0;
        while fg.active() == 0 {
            fg.tick(now, &world);
            now += TICK_MS;
        }
        let f = fg.active[0].clone();
        let node = world.grid.locate(f.at, 0.5).unwrap();
        assert_eq!(fg.hit_per_s_at(node), 0.0, "charging");
        assert_eq!(fg.here(node), Some((f.id, false)));
        while now < f.born + CHARGE_MS + TICK_MS {
            fg.tick(now, &world);
            now += TICK_MS;
        }
        assert_eq!(fg.hit_per_s_at(node), 0.5);
        assert_eq!(fg.here(node), Some((f.id, true)));
        fg.sample_shut(world.grid.len());
        assert!(fg.stats.shut.0 > 0.0);
    }
}
