//! Where a body gets hurt by the map: the damage zones (anomalies, toxic
//! ground, fire) of the project, from `mapdata` (`Zone`, kind `Damage`,
//! with its trigger shapes and its hit; an anomaly core names the zones it
//! drives). Zones whose hit is 0 (sound and sight only) are left out.
//!
//! Each floor of the walk grid where a standing body (feet to head) comes
//! within [`Hazards::margin`] of a zone's shape (sphere, box, cylinder) is
//! marked with that zone (a floor may lie in several). Routes keep off the
//! marked floors while a zone there is active (`path::plan` with
//! [`Hazards::allowed`]), the motor never steers through one, and goals
//! are never put in one.
//!
//! When a zone is active ([`Activity`]):
//! - a zone no core drives, or a core's that never switches: always;
//! - a zone of a timer group (level_03's electric pairs, ≈ lethal per
//!   hit): by the simulation of [`Anomalies`] on the match clock, corrected
//!   from every checked world, its on spans widened by
//!   [`SWITCH_MARGIN_MS`] on each side; a bot may cross it only while it
//!   stays off for [`Hazards::pass_ms`];
//! - a zone of an energy core (level_02's fruit cloud) that a body
//!   walking in its zones would switch on within [`ENERGY_REACH_S`]
//!   (the low states): always (walking in raises the energy); the high
//!   states' zones by the energy the worlds give, falling at its speed.
//!
//! Until [`Hazards::attach`] gives it the cores, every zone counts as
//! active: the bots keep off all of them, which costs a detour now and
//! then and never a life.

use std::collections::HashMap;
use std::sync::Arc;

use mapdata::{Damage, NodeId, Vec3, World, ZoneKind};

use super::anomalies::{Anomalies, Control, Schedule, SyncReport};

/// A switching zone counts as on this long before it switches on and
/// after it switches off, ms: the client's off-grid ticks (≤ 50 ms) and a
/// world's drift, with room to spare.
pub const SWITCH_MARGIN_MS: u32 = 1_000;
/// A zone of an energy core is avoided always when a body moving in the
/// core's zones raises the energy to its threshold within this, seconds
/// (level_02's fruit: lvl1 at 20 and lvl2 at 400 always, lvl3-5 from 1000
/// by the worlds).
pub const ENERGY_REACH_S: f32 = 10.0;
/// How long a switching zone must stay off for a route to cross it, ms
/// ([`Hazards::pass_ms`]'s default; GUESS: reaching and crossing a zone a
/// few metres wide).
pub const PASS_MS: u32 = 4_000;

/// Whether a damage zone hurts at a match time.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Activity {
    /// Always (toxic ground, or an anomaly whose timing is not known).
    #[default]
    Always,
    /// On in these match-time spans, `[from, to)` ms, off between them.
    Spans(Vec<(u32, u32)>),
}

impl Activity {
    pub fn active(&self, time: u32) -> bool {
        match self {
            Self::Always => true,
            Self::Spans(spans) => spans.iter().any(|&(from, to)| (from..to).contains(&time)),
        }
    }

    /// Whether it stays off from `time` for `ms`.
    pub fn off_for(&self, time: u32, ms: u32) -> bool {
        match self {
            Self::Always => false,
            Self::Spans(spans) => !spans
                .iter()
                .any(|&(from, to)| from < time.saturating_add(ms) && to > time),
        }
    }

    /// From a prediction: its on spans widened by `margin` ms each side,
    /// and on from where it ends.
    pub fn from_schedule(schedule: &Schedule, margin: u32) -> Self {
        let mut spans: Vec<(u32, u32)> = Vec::with_capacity(schedule.spans.len() + 1);
        let widened = schedule
            .spans
            .iter()
            .map(|&(from, to)| (from.saturating_sub(margin), to.saturating_add(margin)))
            .chain([(schedule.until.saturating_sub(margin), u32::MAX)]);
        for (from, to) in widened {
            match spans.last_mut() {
                Some(last) if from <= last.1 => last.1 = last.1.max(to),
                _ => spans.push((from, to)),
            }
        }
        Self::Spans(spans)
    }
}

/// One damage zone.
#[derive(Clone, Debug)]
pub struct Hazard {
    /// The zone's `full_name`.
    pub name: String,
    /// Its index in `MapData::zones`.
    pub zone: usize,
    /// The anomaly core that drives it, if one does.
    pub anomaly: Option<String>,
    pub damage_type: String,
    /// Its hit (`None`: not in the map file; taken as harmful).
    pub damage: Option<Damage>,
    pub centre: Vec3,
    pub control: Control,
    pub activity: Activity,
}

impl Hazard {
    /// Whether its activity follows its core's prediction (a timer's, an
    /// energy core's high states); else it is on all match.
    pub fn switches(&self) -> bool {
        match self.control {
            Control::Static => false,
            Control::Timer => true,
            Control::Energy {
                threshold,
                per_second,
            } => threshold > per_second * ENERGY_REACH_S,
        }
    }
}

/// The floors the damage zones mark, for one margin: the same for every
/// match on the map, so matches share them ([`Hazards`] clones hold one
/// `Arc`).
#[derive(Debug, Default)]
pub struct HazardMarks {
    /// By floor: 0 none, else an index + 1 into `sets`.
    floor_set: Vec<u16>,
    /// Sets of zone indices marking a floor.
    sets: Vec<Vec<u16>>,
}

impl HazardMarks {
    /// Each floor of `world` within `margin` metres of one of `zones`
    /// marked. Each shape visits only the floors under its bounds: a few
    /// ms to a few tens on the largest maps.
    pub fn build(world: &World, zones: &[Hazard], margin: f32) -> Self {
        let map = &world.map;
        let grid = &world.grid;
        // Heights over the feet that a standing body fills, ≤ 0.25 m apart.
        let top = grid.body().stand_height.max(0.1);
        let steps = ((top - 0.05) / 0.25).ceil().max(1.0) as usize;
        let heights: Vec<f32> = (0..=steps)
            .map(|i| 0.05 + (top - 0.05) * i as f32 / steps as f32)
            .collect();
        let mut floor_set = vec![0u16; grid.len()];
        let mut sets: Vec<Vec<u16>> = Vec::new();
        let mut known: HashMap<Vec<u16>, u16> = HashMap::new();
        for (h, hazard) in zones.iter().enumerate() {
            let h = h as u16;
            for shape in &map.zones[hazard.zone].shapes {
                let (lo, hi) = shape.bounds();
                let area = [lo.x - margin, lo.z - margin, hi.x + margin, hi.z + margin];
                for node in grid.floors_in(area) {
                    let feet = grid.position(node);
                    if feet.y + top < lo.y - margin || feet.y + 0.05 > hi.y + margin {
                        continue;
                    }
                    let current = floor_set[node as usize];
                    if current > 0 && sets[usize::from(current) - 1].contains(&h) {
                        continue;
                    }
                    let near = heights
                        .iter()
                        .any(|&up| shape.distance(feet + Vec3::new(0.0, up, 0.0)) <= margin);
                    if !near {
                        continue;
                    }
                    let mut set = match current {
                        0 => Vec::new(),
                        c => sets[usize::from(c) - 1].clone(),
                    };
                    set.push(h);
                    let id = *known.entry(set.clone()).or_insert_with(|| {
                        sets.push(set);
                        sets.len() as u16
                    });
                    floor_set[node as usize] = id;
                }
            }
        }
        Self { floor_set, sets }
    }

    /// Into a built file (`mapdata::built`).
    pub fn write_body(&self, out: &mut mapdata::built::Out) -> std::io::Result<()> {
        out.plain(&self.floor_set, |v| v.to_le_bytes())?;
        out.plain(&self.sets, |set| (set.len() as u32).to_le_bytes())?;
        let flat: Vec<u16> = self.sets.iter().flatten().copied().collect();
        out.plain(&flat, |v| v.to_le_bytes())
    }

    /// From a built file, for a grid of `floors` floors and `zones` zones:
    /// every index checked.
    pub fn read_body(
        input: &mut mapdata::built::In,
        floors: usize,
        zones: usize,
    ) -> Result<Self, String> {
        let floor_set: Vec<u16> = input.plain(u16::from_le_bytes)?;
        let lengths: Vec<u32> = input.plain(u32::from_le_bytes)?;
        let flat: Vec<u16> = input.plain(u16::from_le_bytes)?;
        let total: u64 = lengths.iter().map(|&len| u64::from(len)).sum();
        if floor_set.len() != floors
            || total != flat.len() as u64
            || floor_set
                .iter()
                .any(|&set| usize::from(set) > lengths.len())
            || flat.iter().any(|&zone| usize::from(zone) >= zones)
        {
            return Err("hazard marks out of range".into());
        }
        let mut rest = flat.as_slice();
        let sets = lengths
            .iter()
            .map(|&len| {
                let (set, after) = rest.split_at(len as usize);
                rest = after;
                set.to_vec()
            })
            .collect();
        Ok(Self { floor_set, sets })
    }

    /// Bytes held.
    pub fn memory_bytes(&self) -> usize {
        self.floor_set.len() * 2
            + self
                .sets
                .iter()
                .map(|set| 24 + set.len() * 2)
                .sum::<usize>()
    }

    /// Into `digest`: every floor's set and every set.
    pub fn digest_into(&self, digest: &mut mapdata::digest::Digest) {
        digest.len(self.floor_set.len());
        self.floor_set
            .iter()
            .for_each(|&set| digest.u32(u32::from(set)));
        digest.len(self.sets.len());
        for set in &self.sets {
            digest.len(set.len());
            set.iter().for_each(|&h| digest.u32(u32::from(h)));
        }
    }
}

/// The map's damage zones and the floors they cover. A clone shares the
/// marks and copies the zones (a few dozen): a match's own clocks.
#[derive(Clone, Debug, Default)]
pub struct Hazards {
    pub zones: Vec<Hazard>,
    marks: Arc<HazardMarks>,
    pub margin: f32,
    /// How long a switching zone must stay off for [`Self::allowed`].
    pub pass_ms: u32,
    anomalies: Option<Anomalies>,
    /// The prediction the activities were made from.
    seen: u64,
}

impl Hazards {
    /// The damage zones of `world`, each floor within `margin` metres of
    /// one marked ([`HazardMarks::build`]).
    pub fn build(world: &World, margin: f32) -> Self {
        let zones = Self::zones_of(world);
        let marks = HazardMarks::build(world, &zones, margin);
        Self::with_marks(zones, Arc::new(marks), margin)
    }

    /// The damage zones of `world` that hurt, each always active till
    /// [`Self::attach`]: what [`HazardMarks`] index.
    pub fn zones_of(world: &World) -> Vec<Hazard> {
        let map = &world.map;
        let mut zones = Vec::new();
        for (index, zone) in map.zones.iter().enumerate() {
            if zone.kind != ZoneKind::Damage || zone.damage.as_ref().is_some_and(|d| !d.harmful()) {
                continue;
            }
            let anomaly = map
                .anomalies
                .iter()
                .find(|anomaly| anomaly.zones.contains(&zone.name))
                .map(|anomaly| anomaly.name.clone());
            zones.push(Hazard {
                name: zone.name.clone(),
                zone: index,
                anomaly,
                damage_type: zone.damage_type.clone(),
                damage: zone.damage.clone(),
                centre: zone.position,
                control: Control::Static,
                activity: Activity::Always,
            });
        }
        zones
    }

    /// The zones and the floors they mark (built for them, or read).
    pub fn with_marks(zones: Vec<Hazard>, marks: Arc<HazardMarks>, margin: f32) -> Self {
        Self {
            zones,
            marks,
            margin,
            pass_ms: PASS_MS,
            anomalies: None,
            seen: 0,
        }
    }

    /// The zones marking `node`.
    pub fn zones_at(&self, node: NodeId) -> impl Iterator<Item = &Hazard> + '_ {
        let set = match self.marks.floor_set.get(node as usize) {
            Some(&id) if id > 0 => self.marks.sets[usize::from(id) - 1].as_slice(),
            _ => &[],
        };
        set.iter().map(|&h| &self.zones[usize::from(h)])
    }

    /// A zone marking `node`, if any (the first).
    pub fn zone_of(&self, node: NodeId) -> Option<&Hazard> {
        self.zones_at(node).next()
    }

    /// Whether a body may be on `node` at `time`: every zone there is off
    /// and stays off for [`Self::pass_ms`].
    pub fn allowed(&self, node: NodeId, time: u32) -> bool {
        self.allowed_for(node, time, self.pass_ms)
    }

    /// Whether every zone at `node` is off from `time` for `ms`.
    pub fn allowed_for(&self, node: NodeId, time: u32, ms: u32) -> bool {
        self.zones_at(node)
            .all(|zone| zone.activity.off_for(time, ms))
    }

    /// The set of zones marking `node` (0: none); floors of one set are
    /// shut and open together.
    pub fn set_of(&self, node: NodeId) -> u16 {
        self.marks
            .floor_set
            .get(node as usize)
            .copied()
            .unwrap_or(0)
    }

    /// Whether a zone that never switches off marks `node`
    /// ([`Hazard::switches`]): the floor is shut all match. Every zone
    /// until [`Self::attach`].
    pub fn fixed_at(&self, node: NodeId) -> bool {
        self.zones_at(node).any(|zone| !zone.switches())
    }

    /// Which zones never switch off, by index: what [`Self::fixed_at`]
    /// reads; it changes only at [`Self::attach`].
    pub fn fixed_zones(&self) -> Vec<bool> {
        self.zones.iter().map(|zone| !zone.switches()).collect()
    }

    /// Whether no zone marks `node` at all (a place to stay).
    pub fn hazard_free(&self, node: NodeId) -> bool {
        self.zone_of(node).is_none()
    }

    /// When the zones at `node` next switch, the soonest (match ms): for a
    /// bot waiting to cross.
    pub fn next_switch(&self, node: NodeId) -> Option<u32> {
        let anomalies = self.anomalies.as_ref()?;
        self.zones_at(node)
            .filter_map(|zone| anomalies.next_switch(zone.zone))
            .min()
    }

    /// Floors marked, for the logs.
    pub fn marked(&self) -> usize {
        self.marks
            .floor_set
            .iter()
            .filter(|&&mark| mark > 0)
            .count()
    }

    /// Whether `point` is near a zone (the floor under it is marked).
    pub fn near(&self, world: &World, point: Vec3, time: u32) -> bool {
        world
            .grid
            .locate(point, 1.0)
            .is_some_and(|node| !self.allowed(node, time))
    }

    /// The shared marks.
    pub fn marks(&self) -> &HazardMarks {
        &self.marks
    }

    /// The map's anomaly cores (`Anomalies::new(&world.map, static_seed)`)
    /// drive the switching zones from now on.
    pub fn attach(&mut self, anomalies: Anomalies) {
        for hazard in &mut self.zones {
            hazard.control = anomalies.control(hazard.zone);
        }
        self.anomalies = Some(anomalies);
        self.refresh();
    }

    pub fn anomalies(&self) -> Option<&Anomalies> {
        self.anomalies.as_ref()
    }

    /// Changes whenever the zones' activities are made again: a route
    /// search left under way while it changed is not gone on with (the
    /// floors it lets through are no longer those it started with).
    pub fn version(&self) -> u64 {
        self.seen
    }

    /// The cores run to the match clock (`match_ms`); the zones' activity
    /// follows when a prediction changed.
    pub fn advance(&mut self, match_ms: u32) {
        let Some(anomalies) = self.anomalies.as_mut() else {
            return;
        };
        anomalies.advance(match_ms);
        if anomalies.version() != self.seen {
            self.refresh();
        }
    }

    /// A checked world's objects section (`Sections::objects`) of time
    /// `world_ms` sets the cores.
    pub fn sync_from_world(&mut self, objects: &[u8], world_ms: u32) -> SyncReport {
        let Some(anomalies) = self.anomalies.as_mut() else {
            return SyncReport::default();
        };
        let report = anomalies.sync_from_world(objects, world_ms);
        if anomalies.version() != self.seen {
            self.refresh();
        }
        report
    }

    fn refresh(&mut self) {
        let Some(anomalies) = self.anomalies.as_ref() else {
            return;
        };
        for hazard in &mut self.zones {
            let follows = hazard.switches();
            hazard.activity = match anomalies.schedule(hazard.zone) {
                Some(schedule) if follows => Activity::from_schedule(schedule, SWITCH_MARGIN_MS),
                _ => Activity::Always,
            };
        }
        self.seen = anomalies.version();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapdata::{MapData, RayTree, Rotation, Shape, ShapeKind, WalkGrid, Zone};

    use crate::v026g::bots::anomalies::tests::{electric_map, zone};
    use crate::v026g::bots::body::tests::floor;

    fn world_of(map: MapData) -> World {
        let grid = WalkGrid::build(
            &floor(0.0, 0.0, 40.0, 40.0, 0.0),
            [0.0, 0.0, 40.0, 40.0],
            0.25,
            mapdata::Body::default(),
        );
        World::assemble(map, grid.into(), RayTree::build(&[]).into())
    }

    pub fn with_anomaly() -> World {
        world_of(MapData {
            zones: vec![Zone {
                kind: ZoneKind::Damage,
                name: "logic/pool_zone".into(),
                team: -1,
                position: Vec3::new(20.0, 0.0, 20.0),
                damage_type: "fire".into(),
                damage: None,
                shapes: vec![Shape {
                    kind: ShapeKind::Sphere,
                    center: Vec3::new(20.0, 1.0, 20.0),
                    rotation: Rotation::IDENTITY,
                    half: Vec3::new(3.0, 3.0, 3.0),
                }],
            }],
            anomalies: vec![mapdata::Anomaly {
                name: "core".into(),
                position: Vec3::new(20.0, 0.0, 20.0),
                zones: vec!["logic/pool_zone".into()],
                ..mapdata::Anomaly::default()
            }],
            ..MapData::default()
        })
    }

    #[test]
    fn floors_in_and_near_an_anomaly_are_marked_and_routes_go_round() {
        let world = with_anomaly();
        let hazards = Hazards::build(&world, 1.0);
        assert_eq!(hazards.zones.len(), 1);
        assert_eq!(hazards.zones[0].anomaly.as_deref(), Some("core"));
        let grid = &world.grid;
        let inside = grid.locate(Vec3::new(20.0, 0.0, 20.0), 1.0).unwrap();
        let margin = grid.locate(Vec3::new(23.6, 0.0, 20.0), 1.0).unwrap();
        let clear = grid.locate(Vec3::new(26.0, 0.0, 20.0), 1.0).unwrap();
        assert!(!hazards.allowed(inside, 0));
        assert!(!hazards.allowed(margin, 0), "within the margin");
        assert!(hazards.allowed(clear, 0));
        let a = grid.locate(Vec3::new(10.0, 0.0, 20.0), 1.0).unwrap();
        let b = grid.locate(Vec3::new(30.0, 0.0, 20.0), 1.0).unwrap();
        let path = grid.path_where(a, b, |n| hazards.allowed(n, 0)).unwrap();
        assert!(path.nodes.iter().all(|&n| hazards.allowed(n, 0)));
    }

    #[test]
    fn a_zone_with_known_spans_is_open_while_it_is_off() {
        let activity = Activity::Spans(vec![(10_000, 20_000), (40_000, 50_000)]);
        assert!(!activity.active(5_000));
        assert!(activity.active(15_000));
        assert!(activity.off_for(20_000, 15_000));
        assert!(!activity.off_for(30_000, 15_000));
        assert!(!Activity::Always.off_for(0, 1));
        // From a prediction: widened, and on past its end.
        let schedule = Schedule {
            from: 0,
            spans: vec![(5_000, 10_000), (10_500, 20_000)],
            until: 60_000,
        };
        assert_eq!(
            Activity::from_schedule(&schedule, 1_000),
            Activity::Spans(vec![(4_000, 21_000), (59_000, u32::MAX)])
        );
    }

    #[test]
    fn cylinders_mark_the_floors_they_stand_on_and_harmless_zones_none() {
        let mut map = electric_map();
        // A flat cylinder over the floor, one above a standing head, and
        // a harmless zone.
        let high = Shape {
            center: Vec3::new(20.0, 3.5, 5.0),
            ..crate::v026g::bots::anomalies::tests::cylinder(20.0, 5.0, 2.0)
        };
        map.zones.push(zone("high", high, 0.1));
        map.zones.push(zone(
            "fx",
            crate::v026g::bots::anomalies::tests::cylinder(20.0, 35.0, 3.0),
            0.0,
        ));
        let world = world_of(map);
        let hazards = Hazards::build(&world, 0.5);
        assert_eq!(hazards.zones.len(), 3, "the harmless one left out");
        let grid = &world.grid;
        let at = |x: f32, z: f32| grid.locate(Vec3::new(x, 0.0, z), 0.3).unwrap();
        assert_eq!(hazards.zone_of(at(10.0, 20.0)).unwrap().name, "a");
        assert_eq!(hazards.zone_of(at(12.3, 20.0)).unwrap().name, "a", "margin");
        assert!(hazards.hazard_free(at(12.8, 20.0)));
        assert!(hazards.hazard_free(at(20.0, 35.0)), "harmless");
        // 3.25 m up: 1.45 m over a standing head, beyond the margin.
        assert!(hazards.hazard_free(at(20.0, 5.0)));
        let hazards = Hazards::build(&world, 1.5);
        assert_eq!(hazards.zone_of(at(20.0, 5.0)).unwrap().name, "high");
    }

    #[test]
    fn an_electric_pair_opens_and_shuts_with_the_simulation() {
        let world = world_of(electric_map());
        let mut hazards = Hazards::build(&world, 0.5);
        let grid = &world.grid;
        let a = grid.locate(Vec3::new(10.0, 0.0, 20.0), 0.3).unwrap();
        let b = grid.locate(Vec3::new(30.0, 0.0, 20.0), 0.3).unwrap();
        // Without the cores: both always active.
        assert!(!hazards.allowed(a, 100_000) && !hazards.allowed(b, 100_000));
        let anomalies = Anomalies::new(&world.map, 0x8000_0042);
        hazards.attach(anomalies);
        assert_eq!(hazards.zones[0].control, Control::Timer);
        hazards.advance(2_000);
        let anomalies = hazards.anomalies().unwrap();
        let on_a = anomalies.zone_on_now(0);
        assert_ne!(on_a, anomalies.zone_on_now(1), "one of the pair is on");
        let (on, off) = if on_a { (a, b) } else { (b, a) };
        let switch = hazards.next_switch(a).unwrap();
        assert_eq!(hazards.next_switch(b), Some(switch));
        assert!((40_000..=60_050).contains(&switch), "{switch}");
        assert!(!hazards.allowed(on, 2_000));
        assert!(hazards.allowed(off, 2_000));
        // Not within the pass time, nor the margin, before it switches on.
        assert!(!hazards.allowed(off, switch - PASS_MS));
        assert!(hazards.allowed_for(off, switch - SWITCH_MARGIN_MS - 500, 400));
        assert!(!hazards.allowed_for(off, switch - SWITCH_MARGIN_MS + 100, 0));
        // And the other way round once it has switched (plus the margin).
        hazards.advance(switch + 100);
        assert!(!hazards.allowed(off, switch + 100));
        assert!(!hazards.allowed_for(on, switch + 100, 0), "the margin");
        assert!(hazards.allowed(on, switch + SWITCH_MARGIN_MS));
        // A route from one side of the pair to the other crosses the one
        // that is off, not the one that is on.
        let from = grid.locate(Vec3::new(20.0, 0.0, 20.0), 0.3).unwrap();
        let to = grid.locate(Vec3::new(10.0, 0.0, 30.0), 0.3).unwrap();
        let time = switch + SWITCH_MARGIN_MS;
        let path = grid
            .path_where(from, to, |n| hazards.allowed(n, time))
            .unwrap();
        assert!(path.nodes.iter().all(|&n| hazards.allowed(n, time)));
    }

    /// The zones of each level's day project on the real maps, only with
    /// `SRP_MAPDATA_DIR=<mapdata-dump files>`: marking time, floors, and
    /// on the timer maps the switching zones' floors open and shut.
    ///
    /// ```text
    /// SRP_MAPDATA_DIR=<dir> cargo test -p match-server --release real_hazards -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
    fn real_hazards() {
        let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
            eprintln!("skipping: SRP_MAPDATA_DIR is not set");
            return;
        };
        for level in 1..=8 {
            let project = format!("level_{level:02}/day_4");
            let map = MapData::read(&dir.join(mapdata::format::file_name(&project))).unwrap();
            let world = World::build(map, mapdata::CELL, mapdata::Body::default());
            let started = std::time::Instant::now();
            let mut hazards = Hazards::build(&world, 0.5);
            let took = started.elapsed().as_secs_f32() * 1e3;
            hazards.attach(Anomalies::new(&world.map, 0x8000_0001));
            let timers: Vec<usize> = (0..hazards.zones.len())
                .filter(|&h| hazards.zones[h].control == Control::Timer)
                .collect();
            let floors = |h: usize| {
                (0..world.grid.len() as NodeId)
                    .filter(|&n| hazards.zones_at(n).any(|z| z.zone == hazards.zones[h].zone))
                    .count()
            };
            eprintln!(
                "{project}: {} zones ({} of them switching), {} floors marked in {took:.0} ms; \
                 floors per zone {:?}",
                hazards.zones.len(),
                timers.len(),
                hazards.marked(),
                (0..hazards.zones.len()).map(floors).collect::<Vec<_>>(),
            );
            for &h in &timers {
                assert!(floors(h) > 0, "{project}: a switching zone on no floor");
            }
            if timers.is_empty() {
                continue;
            }
            // Over ten minutes every switching zone's floors are open at
            // times and shut at times.
            let node_of = |h: usize| {
                (0..world.grid.len() as NodeId)
                    .find(|&n| {
                        hazards.zones_at(n).count() == 1
                            && hazards.zones_at(n).any(|z| z.zone == hazards.zones[h].zone)
                    })
                    .unwrap()
            };
            let nodes: Vec<NodeId> = timers.iter().map(|&h| node_of(h)).collect();
            let mut seen = vec![(false, false); timers.len()];
            let mut t = 0;
            while t < 600_000 {
                hazards.advance(t);
                for (i, &node) in nodes.iter().enumerate() {
                    if hazards.allowed(node, t) {
                        seen[i].0 = true;
                    } else {
                        seen[i].1 = true;
                    }
                }
                t += 1_000;
            }
            assert!(seen.iter().all(|&(open, shut)| open && shut), "{seen:?}");
        }
    }
}
