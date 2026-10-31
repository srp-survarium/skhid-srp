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

use mapdata::{Damage, NodeId, Shape, Vec3, WalkGrid, World, ZoneKind};

use super::anomalies::{Anomalies, Control, Schedule, SyncReport};
pub use super::live_zones::ZoneSet;
use super::live_zones::{FieldBoard, WorldField};
use super::zone_cost::{Cost, ZoneArmour};

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
/// The fastest a body goes, m/s: a sprint (`kin`, PROVEN 5.0). A floor `d`
/// metres off is reached no sooner than `d / ARRIVE_MPS` seconds from now,
/// so a route's floor must stay off for [`Hazards::pass_ms`] from then
/// ([`Hazards::allowed_arriving`]), not from now.
pub const ARRIVE_MPS: f32 = 5.0;
/// The pace a body can count on walking out of a blast, m/s ([`Pace::Escape`](super::plays::Pace)): the sprint where the way
/// allows (the bench's out-of-blast walks: 2.8 m/s unaimed walking, 5 m/s sprinting; a route with corners and a start-up in
/// between). GUESS, set from the register #30 data and judged by the bodies caught.
pub const ESCAPE_MPS: f32 = 4.0;
/// ... after this long from the decision, s: the turn to the way and the speed built up.
pub const ESCAPE_START_S: f32 = 0.3;

/// How far a body can walk out of a blast in `left_ms` of fuse: [`ESCAPE_MPS`] after the start-up [`ESCAPE_START_S`].
pub fn escape_reach_m(left_ms: u32) -> f32 {
    ESCAPE_MPS * (left_ms as f32 / 1000.0 - ESCAPE_START_S).max(0.0)
}

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
    /// Its trigger shapes (`mapdata::Zone::shapes`): what the hit at a point is made from ([`Self::intensity_at`],
    /// [`Self::hit_rate_at`]).
    pub shapes: Vec<Shape>,
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

    /// The share of its full hit a body whose centre is at `point` takes, `[0, 1]`: the hit curve at the normalised
    /// distance from its centre ([`mapdata::Damage::intensity`], as `mapdata::Zone::intensity_at`), 1 for a zone with no hit
    /// given.
    pub fn intensity_at(&self, point: Vec3) -> f32 {
        self.damage
            .as_ref()
            .map_or(1.0, |damage| damage.intensity(Shape::least_normalised_distance(&self.shapes, point)))
    }

    /// What a body whose centre is at `point` takes a second in the zone while it is on, before armour, by the client's
    /// rule ([`mapdata::Damage::rate_at`] at the normalised distance from the zone's centre, the interval included): what
    /// the bench's stand-in hurts its bodies with, and [`Self::hit_per_s`] times [`Self::intensity_at`]. Infinite when the
    /// map file gives no hit ([`Hazard::damage`]).
    pub fn hit_rate_at(&self, point: Vec3) -> f32 {
        self.damage
            .as_ref()
            .map_or(f32::INFINITY, |damage| damage.rate_at(Shape::least_normalised_distance(&self.shapes, point)))
    }

    /// Its hit a second while on, at full intensity, the client's rate (`mapdata::Damage::rate_per_s`: the hit over the
    /// interval, for a continuous zone as for the others; it was the raw hit for a continuous zone, which is the same for
    /// every shipped one but Vector Laboratory's `death_damage_zone`, 500 ms: 1.6 a second, priced 0.8). Infinite when the
    /// map file gives none (taken as harmful, [`Hazard::damage`]).
    pub fn hit_per_s(&self) -> f32 {
        self.damage.as_ref().map_or(f32::INFINITY, mapdata::Damage::rate_per_s)
    }
}

/// The feet heights a standing body fills (the top, and the heights from the floor up to it, ≤ 0.25 m apart): what
/// [`near_floor`] tests a floor at.
pub(super) fn standing_heights(grid: &WalkGrid) -> (f32, Vec<f32>) {
    let top = grid.body().stand_height.max(0.1);
    let steps = ((top - 0.05) / 0.25).ceil().max(1.0) as usize;
    let heights = (0..=steps)
        .map(|i| 0.05 + (top - 0.05) * i as f32 / steps as f32)
        .collect();
    (top, heights)
}

/// The centres of the spheres a standing body's capsule is made of (`radius` apart from the ends, ≤ 0.25 m apart): what
/// [`near_floor`] tests a floor at for [`HazardMarks::hurt_set`] with `margin` the radius. Returns the capsule's height.
pub(super) fn capsule_heights(grid: &WalkGrid) -> (f32, Vec<f32>) {
    let body = grid.body();
    let top = body.stand_height.max(0.1);
    let (low, high) = (body.radius, (top - body.radius).max(body.radius));
    let steps = ((high - low) / 0.25).ceil().max(1.0) as usize;
    let heights = (0..=steps).map(|i| low + (high - low) * i as f32 / steps as f32).collect();
    (top, heights)
}

/// Whether a standing body on the floor at `feet` comes within `margin` of `shape` (whose bounds are `bounds`): the
/// test that marks a floor, for the baked zones ([`HazardMarks::build`]) and the live fields alike.
pub(super) fn near_floor(
    shape: &Shape,
    bounds: (Vec3, Vec3),
    feet: Vec3,
    top: f32,
    heights: &[f32],
    margin: f32,
) -> bool {
    let (lo, hi) = bounds;
    if feet.y + top < lo.y - margin || feet.y + 0.05 > hi.y + margin {
        return false;
    }
    heights
        .iter()
        .any(|&up| shape.distance(feet + Vec3::new(0.0, up, 0.0)) <= margin)
}

/// The floors the damage zones mark, for one margin: the same for every
/// match on the map, so matches share them ([`Hazards`] clones hold one
/// `Arc`).
#[derive(Debug, Default)]
pub struct HazardMarks {
    /// By floor: 0 none, else an index + 1 into `sets`.
    floor_set: Vec<u16>,
    /// By floor, the zones that hurt a body standing there: those its capsule (the grid body's width and height)
    /// overlaps, where `floor_set` is the floors within the margin of a zone. The same indices as `floor_set`. The
    /// floors of `floor_set` are where routes keep off ([`Hazards::allowed`]); these are where a body loses
    /// health ([`Hazards::hurt_per_s`]).
    hurt_set: Vec<u16>,
    /// Sets of zone indices marking a floor.
    sets: Vec<Vec<u16>>,
}

impl HazardMarks {
    /// Each floor of `world` within `margin` metres of one of `zones`
    /// marked, and each floor a body standing there overlaps one of them
    /// with its capsule marked as hurt. Each shape visits only the floors under its
    /// bounds: a few ms to a few tens on the largest maps.
    pub fn build(world: &World, zones: &[Hazard], margin: f32) -> Self {
        let mut sets: Vec<Vec<u16>> = Vec::new();
        let mut known: HashMap<Vec<u16>, u16> = HashMap::new();
        let steering = standing_heights(&world.grid);
        let floor_set = Self::mark(world, zones, margin, steering, &mut sets, &mut known);
        let hurt = capsule_heights(&world.grid);
        let hurt_set = Self::mark(world, zones, world.grid.body().radius, hurt, &mut sets, &mut known);
        Self { floor_set, hurt_set, sets }
    }

    /// The floors within `margin` of the zones from the body points `(top, heights)` ([`near_floor`]), as indices
    /// into `sets` (made as it goes).
    fn mark(
        world: &World,
        zones: &[Hazard],
        margin: f32,
        (top, heights): (f32, Vec<f32>),
        sets: &mut Vec<Vec<u16>>,
        known: &mut HashMap<Vec<u16>, u16>,
    ) -> Vec<u16> {
        let map = &world.map;
        let grid = &world.grid;
        let mut floor_set = vec![0u16; grid.len()];
        for (h, hazard) in zones.iter().enumerate() {
            let h = h as u16;
            for shape in &map.zones[hazard.zone].shapes {
                let (lo, hi) = shape.bounds();
                let area = [lo.x - margin, lo.z - margin, hi.x + margin, hi.z + margin];
                for node in grid.floors_in(area) {
                    let feet = grid.position(node);
                    let current = floor_set[node as usize];
                    if current > 0 && sets[usize::from(current) - 1].contains(&h) {
                        continue;
                    }
                    if !near_floor(shape, (lo, hi), feet, top, &heights, margin) {
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
        floor_set
    }

    /// Into a built file (`mapdata::built`).
    pub fn write_body(&self, out: &mut mapdata::built::Out) -> std::io::Result<()> {
        out.plain(&self.floor_set, |v| v.to_le_bytes())?;
        out.plain(&self.hurt_set, |v| v.to_le_bytes())?;
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
        let hurt_set: Vec<u16> = input.plain(u16::from_le_bytes)?;
        let lengths: Vec<u32> = input.plain(u32::from_le_bytes)?;
        let flat: Vec<u16> = input.plain(u16::from_le_bytes)?;
        let total: u64 = lengths.iter().map(|&len| u64::from(len)).sum();
        if floor_set.len() != floors
            || hurt_set.len() != floors
            || total != flat.len() as u64
            || floor_set
                .iter()
                .chain(&hurt_set)
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
        Ok(Self { floor_set, hurt_set, sets })
    }

    /// Bytes held.
    pub fn memory_bytes(&self) -> usize {
        self.floor_set.len() * 4
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
            .chain(&self.hurt_set)
            .for_each(|&set| digest.u32(u32::from(set)));
        digest.len(self.sets.len());
        for set in &self.sets {
            digest.len(set.len());
            set.iter().for_each(|&h| digest.u32(u32::from(h)));
        }
    }
}

/// Which floors hurt, for [`HurtTest`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HurtFloors {
    /// The zone's full hit on every floor routes keep off, the margin's marks, as it was (`HazardTuning::hurt_by_shape`
    /// off; what a test with the all-off tuning has).
    #[default]
    Margin,
    /// The full hit on the floors a standing body's capsule overlaps the zone's shape from
    /// ([`HazardMarks::hurt_set`]); on the margin's band round them `band_permille` thousandths of it, the share of the
    /// time the motor's drift puts a body walking there inside.
    Shape { band_permille: u16 },
}

/// What a body on a floor loses to the zones there ([`Hazards::hurt_per_s`]). The rule is a team's (its tuning,
/// `HazardTuning::hurt_by_shape`, `zone_intensity`), so it is an argument of the question, not a field of the shared
/// zones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HurtTest {
    pub floors: HurtFloors,
    /// The hit is the zone's at the body's place in it ([`Hazard::intensity_at`]), not the full hit
    /// (`HazardTuning::zone_intensity`).
    pub by_intensity: bool,
}

impl HurtTest {
    /// The margin's marks at the full hit: the rule before the shape and the intensity.
    pub const MARGIN: Self = Self { floors: HurtFloors::Margin, by_intensity: false };

    /// The rule of `tuning` for a body of `radius` metres: the band is the margin less the radius wide, a body in it
    /// drifts `band_drift_m * sqrt(2/pi)` on average (half-normal), so it is inside that share of the band's width of
    /// the time (at most all of it).
    pub fn of(tuning: &super::tuning::HazardTuning, radius: f32) -> Self {
        let floors = if tuning.hurt_by_shape {
            let band = tuning.margin_m.get() - radius;
            let mean_drift = tuning.band_drift_m.get() * (2.0 / std::f32::consts::PI).sqrt();
            let share = if band > 0.0 { (mean_drift / band).min(1.0) } else { 1.0 };
            HurtFloors::Shape { band_permille: (share * 1000.0).round() as u16 }
        } else {
            HurtFloors::Margin
        };
        Self { floors, by_intensity: tuning.zone_intensity }
    }

    /// The share of a zone's hit a body on a floor only the margin marks loses.
    pub fn band_share(self) -> f32 {
        match self.floors {
            HurtFloors::Margin => 1.0,
            HurtFloors::Shape { band_permille } => f32::from(band_permille) / 1000.0,
        }
    }

    /// The share of `zone`'s hit a body standing on `node` loses to where in the zone it stands: 1 unless
    /// [`Self::by_intensity`], else the intensity at the body's centre (the grid body's half height over the floor).
    fn intensity(self, zone: &Hazard, grid: &WalkGrid, node: NodeId) -> f32 {
        if !self.by_intensity {
            return 1.0;
        }
        let centre = grid.position(node) + Vec3::new(0.0, 0.5 * grid.body().stand_height, 0.0);
        zone.intensity_at(centre)
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
    /// The fields of this match's worlds (`Tuning::live_zones`), next to the baked zones: empty unless the game
    /// feeds it. Floors only: the regions, the fixed zones and the crossings read the baked zones alone.
    pub live: FieldBoard,
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
            // The zones of a timed core are the board's (`field_source`), not baked.
            if super::field_source::live_only(map, &zone.name) {
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
                shapes: zone.shapes.clone(),
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
            live: FieldBoard::default(),
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

    /// The zones that hurt a body standing on `node` ([`HurtTest`]): the zones whose shape it overlaps, or all that mark
    /// it.
    pub fn hurt_zones_at(&self, node: NodeId, rule: HurtTest) -> impl Iterator<Item = &Hazard> + '_ {
        let floor_set = match rule.floors {
            HurtFloors::Shape { .. } => &self.marks.hurt_set,
            HurtFloors::Margin => &self.marks.floor_set,
        };
        let set = match floor_set.get(node as usize) {
            Some(&id) if id > 0 => self.marks.sets[usize::from(id) - 1].as_slice(),
            _ => &[],
        };
        set.iter().map(|&h| &self.zones[usize::from(h)])
    }

    /// The zones marking `node` and how much of their hit a body there loses: all of it where the body is hurt
    /// ([`Self::hurt_zones_at`]), the band's share ([`HurtFloors::Shape`]) where only the margin marks it, either times
    /// the intensity of its place in the zone ([`HurtTest::by_intensity`]).
    fn weighted_zones_at<'a>(
        &'a self,
        grid: &'a WalkGrid,
        node: NodeId,
        rule: HurtTest,
    ) -> impl Iterator<Item = (&'a Hazard, f32)> + 'a {
        let band = rule.band_share();
        self.zones_at(node).map(move |zone| {
            let hurts = self.hurt_zones_at(node, rule).any(|hurt| hurt.zone == zone.zone);
            (zone, if hurts { 1.0 } else { band } * rule.intensity(zone, grid, node))
        })
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

    /// Whether every zone at `node`, baked or live, is off from `time` for `ms`.
    pub fn allowed_for(&self, node: NodeId, time: u32, ms: u32) -> bool {
        self.zones_at(node)
            .all(|zone| zone.activity.off_for(time, ms))
            && self.live.allowed_for(node, time, ms)
    }

    /// [`Self::allowed`] for a body `d_m` metres off `node` (in a line):
    /// its zones off for [`Self::pass_ms`] from the soonest it gets there
    /// ([`ARRIVE_MPS`]). A zone that is on now and off by then lets it
    /// through; one that is off now and on by then does not. At 0 m,
    /// [`Self::allowed`].
    pub fn allowed_arriving(&self, node: NodeId, time: u32, d_m: f32) -> bool {
        self.allowed(node, arrival(time, d_m))
    }

    /// Whether a zone at `node` is on at `time` (no margin of time).
    pub fn on_at(&self, node: NodeId, time: u32) -> bool {
        self.zones_at(node).any(|zone| zone.activity.active(time)) || self.live.on_at(node, time)
    }

    /// The zones' hit a second at `node` while they are on, summed
    /// ([`Hazard::hit_per_s`]); 0 on a floor no zone marks.
    pub fn hit_per_s(&self, node: NodeId) -> f32 {
        self.zones_at(node).map(Hazard::hit_per_s).sum::<f32>() + self.live.hit_per_s(node)
    }

    /// What a body standing on `node` loses a second while the zones are on, summed ([`Hazard::hit_per_s`]): the full
    /// hit of the zones that hurt there, the band's share of the others' that only mark it ([`Self::weighted_zones_at`]),
    /// where [`Self::hit_per_s`] counts every floor routes keep off at the full hit (the gate of what may be crossed,
    /// `crossing::crossable`). A floor over a cylinder's top, in its margin only, costs the band's share.
    pub fn hurt_per_s(&self, grid: &WalkGrid, node: NodeId, rule: HurtTest) -> f32 {
        self.weighted_zones_at(grid, node, rule).map(|(zone, w)| zone.hit_per_s() * w).sum::<f32>()
            + self.live.hit_per_s(node)
    }

    /// [`Self::hurt_per_s`] in the pools that kill, under `armour` (`None`: the raw hit in the body pool).
    pub fn hurt_cost_per_s(&self, grid: &WalkGrid, node: NodeId, armour: Option<&ZoneArmour>, rule: HurtTest) -> Cost {
        match armour {
            None => Cost::body(self.hurt_per_s(grid, node, rule)),
            Some(armour) => {
                self.weighted_zones_at(grid, node, rule).map(|(zone, w)| zone.cost_per_s(armour) * w).sum::<Cost>()
                    + Cost::body(self.live.hit_per_s(node))
            }
        }
    }

    /// What walking over `node` costs a second in the pools that kill, under `armour`: the margin's price of every floor
    /// routes keep off, the motor's own (`crossing::hurt`), each zone at the intensity of the floor's place in it where
    /// the rule asks ([`HurtTest::by_intensity`]), else [`Self::cost_per_s`].
    pub fn leg_cost_per_s(&self, grid: &WalkGrid, node: NodeId, armour: Option<&ZoneArmour>, rule: HurtTest) -> Cost {
        if !rule.by_intensity {
            return self.cost_per_s(node, armour);
        }
        let live = self.live.hit_per_s(node);
        match armour {
            None => {
                let baked: f32 = self.zones_at(node).map(|zone| zone.hit_per_s() * rule.intensity(zone, grid, node)).sum();
                Cost::body(baked + live)
            }
            Some(armour) => {
                self.zones_at(node).map(|zone| zone.cost_per_s(armour) * rule.intensity(zone, grid, node)).sum::<Cost>()
                    + Cost::body(live)
            }
        }
    }

    /// [`Self::hit_per_s`] in the pools that kill, under `armour` (`None`: the raw hit in the body
    /// pool, the number it always was). The live fields count by their raw hit in the body pool.
    pub fn cost_per_s(&self, node: NodeId, armour: Option<&ZoneArmour>) -> Cost {
        match armour {
            None => Cost::body(self.hit_per_s(node)),
            Some(armour) => {
                self.zones_at(node).map(|zone| zone.cost_per_s(armour)).sum::<Cost>()
                    + Cost::body(self.live.hit_per_s(node))
            }
        }
    }

    /// The set of zones marking `node`, baked and live (none: no zone);
    /// floors of one set are shut and open together.
    pub fn set_of(&self, node: NodeId) -> ZoneSet {
        let baked = self
            .marks
            .floor_set
            .get(node as usize)
            .copied()
            .unwrap_or(0);
        ZoneSet::new(baked, self.live.set_of(node))
    }

    /// Whether a zone that never switches off marks `node`
    /// ([`Hazard::switches`]): the floor is shut all match. Every zone
    /// until [`Self::attach`].
    pub fn fixed_at(&self, node: NodeId) -> bool {
        self.zones_at(node).any(|zone| !zone.switches())
    }

    /// Whether zones mark `node` and none of them ever switches off
    /// ([`Hazard::switches`]): a floor shut all match and by nothing else, which
    /// a crossing may walk (`crossing.rs`).
    pub fn only_fixed_at(&self, node: NodeId) -> bool {
        // A live field is never crossable: its floors are shut by more than a zone that stays on.
        if self.live.set_of(node) != 0 {
            return false;
        }
        let mut any = false;
        for zone in self.zones_at(node) {
            if zone.switches() {
                return false;
            }
            any = true;
        }
        any
    }

    /// Which zones never switch off, by index: what [`Self::fixed_at`]
    /// reads; it changes only at [`Self::attach`].
    pub fn fixed_zones(&self) -> Vec<bool> {
        self.zones.iter().map(|zone| !zone.switches()).collect()
    }

    /// Whether no zone, baked or live, marks `node` at all (a place to stay).
    pub fn hazard_free(&self, node: NodeId) -> bool {
        self.zone_of(node).is_none() && self.live.set_of(node) == 0
    }

    /// Whether a zone, baked or live, marks `node`: [`Self::zone_of`] is the baked zone alone (the regions' cut).
    pub fn marked_at(&self, node: NodeId) -> bool {
        self.zone_of(node).is_some() || self.live.set_of(node) != 0
    }

    /// When the zones at `node`, baked and live, next switch, the soonest
    /// (match ms): for a bot waiting to cross.
    pub fn next_switch(&self, node: NodeId) -> Option<u32> {
        let baked = self.anomalies.as_ref().and_then(|anomalies| {
            self.zones_at(node)
                .filter_map(|zone| anomalies.next_switch(zone.zone))
                .min()
        });
        [baked, self.live.next_switch(node)].into_iter().flatten().min()
    }

    /// The soonest match time from `time` at which a body may be on `node` ([`Self::allowed`]) by the schedule alone, if the
    /// zones there are all baked, switching ones with a predicted end to their present span: a gate that opens. `None` for
    /// a floor a live field marks, or a zone on all match, or one whose span runs past the prediction.
    pub fn gate_end(&self, node: NodeId, time: u32) -> Option<u32> {
        if self.live.set_of(node) != 0 {
            return None;
        }
        if self.allowed(node, time) {
            return Some(time);
        }
        let mut ends: Vec<u32> = Vec::new();
        for zone in self.zones_at(node) {
            let Activity::Spans(spans) = &zone.activity else {
                return None;
            };
            ends.extend(spans.iter().map(|&(_, to)| to).filter(|&to| to > time && to != u32::MAX));
        }
        ends.sort_unstable();
        ends.into_iter()
            .find(|&at| self.zones_at(node).all(|zone| zone.activity.off_for(at, self.pass_ms)))
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
        self.seen + self.live.version()
    }

    /// The fields of a world (match time `time`) are the live zones from now on: new ones mark their floors, those
    /// not listed go at once. Returns whether the board changed ([`Self::version`] then changed too).
    pub fn set_fields(&mut self, fields: &[WorldField], grid: &WalkGrid, time: u32) -> bool {
        let margin = self.margin;
        self.live.update(fields, grid, margin, time)
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

/// What the damage-zone fixes of the crossings and the escort posts did (the owner tally's `damage_zone_fixes` topic,
/// `tests/tallies/owner/damage_zones.rs`): each event once, where it is decided, filed under the team that asked. These are
/// the plays of the battery modes (a crossing to a battery, an escort post for its carrier), so their counts are of the
/// objective kind [`KIND`]; a mode with other plays over damage zones gets its own kind and keys. They have nothing to do
/// with a research mode's capture zones.
#[cfg(test)]
pub mod damage_zone_fixes {
    use std::cell::{Cell, RefCell};

    /// The objective kind these counts are of (the key prefix `dzf_<kind>_`).
    pub const KIND: &str = "bat";

    #[derive(Clone, Copy)]
    pub enum Ev {
        /// A battery's crossing asked (a memo miss), found, found at no cost (the fix of the hurt test), none.
        CrossingAsked,
        CrossingFound,
        CrossingFree,
        CrossingNone,
        /// The motor's route searched for a crossing: no route, a route meeting a zone floor before the stop, a route
        /// only `RouteCrossing::Total` offers (the fix).
        RoutedNoRoute,
        RoutedStrict,
        RoutedTotalOnly,
        /// An escort play begun: its post on the line, pulled to a dry floor that sees him, none (a failed start).
        PostLine,
        PostPulled,
        PostNone,
        /// A routed crossing's way out searched: found (priced), none (priced as the way in).
        OutPriced,
        OutNone,
        /// A crossing found whose price at the zones' intensity (`HazardTuning::zone_intensity`) is under 99 % of its price
        /// at the full hit: the fix changed the price (counted by pricing it both ways).
        CrossingCheaper,
        /// An escort offered for a carrier by the old rule and refused by `escort_reach` (the post on his line is a goal no
        /// walk of the escort reaches, or the escort stands in such a pocket), once a carrier and a bot a 2 s.
        EscortUnreachable,
        /// A drive tick whose walking pace was made a run because the body stood on a floor a zone hurts on now
        /// (`HazardTuning::run_out`).
        RunOut,
        /// A carrier whose way home is shut entered the stranded state (`collect::State::Stranded`), and one that let go of the battery when its time ran out.
        HomeStranded,
        HomeLetGo,
        /// A stranded carrier whose gate was found (`Strand::Gate`): entered, the wait it was promised (ms, summed), the way found while it waited, the gate's time out
        /// with the way still shut (it goes on to the bound as `Strand::Missed`), and the let-go of such a one (`HomeLetGo` is the plain no-way's).
        HomeGate,
        HomeGateMs,
        HomeGateOpened,
        HomeGateMissed,
        HomeLetGoMissed,
    }

    /// The events' key suffixes, in order.
    pub const NAMES: [&str; N] = [
        "cross_asked",
        "cross_found",
        "cross_free",
        "cross_none",
        "routed_noroute",
        "routed_strict",
        "routed_total_only",
        "post_line",
        "post_pulled",
        "post_none",
        "out_priced",
        "out_none",
        "cross_cheaper",
        "esc_unreachable",
        "run_out_ticks",
        "home_stranded",
        "home_let_go",
        "home_gate",
        "home_gate_ms",
        "home_gate_opened",
        "home_gate_missed",
        "home_let_go_missed",
    ];

    pub const N: usize = 22;
    pub type Counts = [[u64; N]; 2];

    // A thread's own counts: a test, or one bench match, counts alone (tests run in parallel threads).
    thread_local! {
        static COUNTS: RefCell<Counts> = const { RefCell::new([[0; N]; 2]) };
        static TEAM: Cell<u8> = const { Cell::new(0) };
        static ONCE: RefCell<std::collections::HashSet<(u8, usize, u32, u32)>> = RefCell::new(std::collections::HashSet::new());
    }

    /// `ask` with the events it counts filed under `team`.
    pub fn asked_by<R>(team: u8, ask: impl FnOnce() -> R) -> R {
        let before = TEAM.replace(team.min(1));
        let out = ask();
        TEAM.set(before);
        out
    }

    /// `ask` with its events left uncounted: a question asked a second time only to compare (the counts and the team stay).
    pub fn quietly<R>(ask: impl FnOnce() -> R) -> R {
        let before = COUNTS.with_borrow(|counts| *counts);
        let out = ask();
        COUNTS.with_borrow_mut(|counts| *counts = before);
        out
    }

    /// [`count`] once for a team's bot and subject (a carrier) in each 2 s: a refusal asked at every think counts as one.
    pub fn count_once(ev: Ev, subject: u32, now: u32) {
        let key = (TEAM.get(), ev as usize, subject, now / 2_000);
        if ONCE.with_borrow_mut(|seen| seen.insert(key)) {
            count(ev);
        }
    }

    /// Whether the escort refusal was counted for this carrier of `team` in `[from_ms, to_ms]` (the bench's trace of a zone take).
    pub fn refused_for(team: u8, carrier: u32, from_ms: u32, to_ms: u32) -> bool {
        ONCE.with_borrow(|seen| {
            seen.iter().any(|&(t, ev, subject, bucket)| {
                t == team.min(1)
                    && ev == Ev::EscortUnreachable as usize
                    && subject / 64 == carrier
                    && bucket >= from_ms / 2_000
                    && bucket <= to_ms / 2_000
            })
        })
    }

    pub fn count(ev: Ev) {
        count_by(ev, 1);
    }

    /// `n` more of an event (a sum of milliseconds).
    pub fn count_by(ev: Ev, n: u64) {
        let team = usize::from(TEAM.get());
        COUNTS.with_borrow_mut(|counts| counts[team][ev as usize] += n);
    }

    /// This thread's counts so far, by team.
    pub fn counts() -> Counts {
        COUNTS.with_borrow(|counts| *counts)
    }
}

/// The soonest match time a body `d_m` metres off gets there from `time`
/// ([`ARRIVE_MPS`]).
pub fn arrival(time: u32, d_m: f32) -> u32 {
    time.saturating_add((d_m.max(0.0) / ARRIVE_MPS * 1000.0) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapdata::{MapData, RayTree, Rotation, Shape, ShapeKind, WalkGrid, Zone};

    use crate::v026g::bots::anomalies::tests::{cylinder, electric_map, zone};
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

    /// A floor over a box's top, inside the margin only, is shut to routes and hurts nobody: the shape's own test
    /// ([`HurtFloors::Shape`]) prices it at the band's share of the zone's hit, the margin's ([`HurtFloors::Margin`]) at all
    /// of it; a floor the capsule overlaps the zone from costs all of it either way.
    #[test]
    fn a_floor_over_a_zones_top_is_shut_to_routes_and_priced_as_a_band() {
        let mut map = electric_map();
        map.zones.clear();
        let slab = Shape {
            kind: ShapeKind::Box,
            center: Vec3::new(20.0, -0.27 - 1.0, 20.0),
            rotation: Rotation::IDENTITY,
            half: Vec3::new(4.0, 1.0, 4.0),
        };
        map.zones.push(zone("slab", slab, 0.1));
        let world = world_of(map);
        let hazards = Hazards::build(&world, 1.0);
        let grid = &world.grid;
        let over = grid.locate(Vec3::new(20.0, 0.0, 20.0), 0.3).unwrap();
        assert!(!hazards.allowed(over, 0) && hazards.zone_of(over).is_some(), "shut to routes");
        let full = hazards.hit_per_s(over);
        assert!(full > 0.0);
        assert_eq!(hazards.hurt_per_s(grid, over, HurtTest::default()), full, "the margin's marks hurt, as they did");
        let shape = HurtTest { floors: HurtFloors::Shape { band_permille: 250 }, by_intensity: false };
        assert_eq!(hazards.hurt_zones_at(over, shape).count(), 0, "the capsule does not reach it");
        assert!((hazards.hurt_per_s(grid, over, shape) - 0.25 * full).abs() < 1e-6, "the band's share");
        assert_eq!(hazards.hit_per_s(over), full, "the gate of what may be crossed is the full hit");
        // A box whose top reaches the floor does hurt there, whichever test.
        let mut map = electric_map();
        map.zones.clear();
        let deep = Shape { center: Vec3::new(20.0, 0.5, 20.0), ..slab };
        map.zones.push(zone("deep", deep, 0.1));
        let world = world_of(map);
        let hazards = Hazards::build(&world, 1.0);
        let on = world.grid.locate(Vec3::new(20.0, 0.0, 20.0), 0.3).unwrap();
        let shape = HurtTest { floors: HurtFloors::Shape { band_permille: 250 }, by_intensity: false };
        assert_eq!(hazards.hurt_per_s(&world.grid, on, shape), hazards.hit_per_s(on));
    }

    /// A zone whose hit falls off to its rim is priced at the hit of the floor's place in it ([`HurtTest::by_intensity`]):
    /// the hurt floors, the legs of a crossing and the stand; the gate of what may be crossed
    /// ([`Hazards::hit_per_s`]) stays at the full hit, and a rule without it prices the full hit everywhere.
    #[test]
    fn a_zones_floors_are_priced_at_the_intensity_of_their_place_in_it() {
        let mut map = electric_map();
        map.zones.clear();
        let mut radiation = zone("radiation", cylinder(20.0, 20.0, 8.0), 0.1);
        radiation.shapes[0].half.y = 3.0;
        radiation.damage.as_mut().unwrap().hit_curve = vec![(0.0, 1.0), (1.0, 0.0)];
        map.zones.push(radiation);
        let world = world_of(map);
        let hazards = Hazards::build(&world, 1.0);
        let grid = &world.grid;
        let at = |x: f32| grid.locate(Vec3::new(x, 0.0, 20.0), 0.3).unwrap();
        let (near, rim) = (at(21.0), at(26.0));
        let full = hazards.hit_per_s(near);
        assert!(full > 0.0 && hazards.hit_per_s(rim) == full, "the gate is the full hit on every marked floor");
        let flat = HurtTest { floors: HurtFloors::Shape { band_permille: 250 }, by_intensity: false };
        let falling = HurtTest { by_intensity: true, ..flat };
        for node in [near, rim] {
            assert_eq!(hazards.hurt_per_s(grid, node, flat), full, "without the intensity, the full hit");
            assert_eq!(hazards.leg_cost_per_s(grid, node, None, flat).body, full);
            let centre = grid.position(node) + Vec3::new(0.0, 0.5 * grid.body().stand_height, 0.0);
            let share = hazards.zones[0].intensity_at(centre);
            assert!((hazards.hurt_per_s(grid, node, falling) - full * share).abs() < 1e-6, "the zone's hit times the intensity");
            assert!((hazards.leg_cost_per_s(grid, node, None, falling).body - full * share).abs() < 1e-6);
            assert_eq!(hazards.hit_per_s(node), full, "the gate does not move");
        }
        assert!(
            hazards.hurt_per_s(grid, rim, falling) < 0.5 * hazards.hurt_per_s(grid, near, falling),
            "the rim is dearer than the middle: 6 m of 8 against 1 m"
        );
    }

    /// The client's rate at a point is the zone's rate at full intensity times its intensity there, whatever the interval; and
    /// the bots' full hit is that rate (the interval included: Vector Laboratory's `death_damage_zone` takes 1.6 a second at
    /// its centre, not 0.8), in the pools that kill too.
    #[test]
    fn a_zones_rate_at_a_point_is_its_hit_per_s_times_its_intensity() {
        use crate::v026g::bots::zone_cost::ZoneArmour;
        let mut map = electric_map();
        map.zones.clear();
        let mut death = zone("death", cylinder(20.0, 20.0, 8.0), 0.2);
        death.shapes[0].half.y = 3.0;
        death.damage_type = "radiation".into();
        {
            let damage = death.damage.as_mut().unwrap();
            damage.parts = 4;
            damage.part_names = ["face", "body", "left_hand", "right_hand"].map(String::from).to_vec();
            damage.interval_ms = 500;
            damage.continuous = true;
            damage.hit_curve = vec![(0.0, 1.0), (1.0, 0.0)];
        }
        map.zones.push(death);
        let world = world_of(map);
        let hazards = Hazards::build(&world, 1.0);
        let zone = &hazards.zones[0];
        assert!((zone.hit_per_s() - 1.6).abs() < 1e-6, "0.2 x 4 parts every 500 ms: {}", zone.hit_per_s());
        let armour = ZoneArmour::default();
        for x in [20.0, 23.0, 26.0] {
            let at = Vec3::new(x, 0.9, 20.0);
            assert!((zone.hit_rate_at(at) - zone.hit_per_s() * zone.intensity_at(at)).abs() < 1e-6, "x {x}");
            let (client, full) = (zone.client_cost_per_s(&armour, at), zone.cost_per_s(&armour));
            let share = zone.intensity_at(at);
            assert!((client.most() - full.most() * share).abs() < 1e-5, "x {x}: {} against {} x {share}", client.most(), full.most());
        }
    }

    /// The band's share from the tuning: the margin less the body's radius wide, the drift's mean over it.
    #[test]
    fn the_bands_share_is_the_drifts_mean_over_its_width() {
        let tuning = super::super::tuning::Tuning::default().hazard;
        let HurtFloors::Shape { band_permille } = HurtTest::of(&tuning, 0.38).floors else {
            panic!("on by default");
        };
        assert_eq!(band_permille, (1000.0 * 0.2 * (2.0f32 / std::f32::consts::PI).sqrt() / 0.62).round() as u16);
        let off = super::super::tuning::Tuning::all_off().hazard;
        assert_eq!(HurtTest::of(&off, 0.38), HurtTest::MARGIN);
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

    /// Per zone of each level's day projects on the real maps (`SRP_MAPDATA_DIR`), the floors routes keep off and those
    /// a body's capsule overlaps the zone from: the margin ring is the difference. Prints only.
    #[test]
    #[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
    fn hurt_floors_against_marked_floors_on_the_real_maps() {
        let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
            eprintln!("skipping: SRP_MAPDATA_DIR is not set");
            return;
        };
        for level in 1..=8 {
            for mode in [2, 4] {
                let project = format!("level_{level:02}/day_{mode}");
                let Ok(map) = MapData::read(&dir.join(mapdata::format::file_name(&project))) else {
                    continue;
                };
                let world = World::build(map, mapdata::CELL, mapdata::Body::default());
                let hazards = Hazards::build(&world, 1.0);
                for (h, zone) in hazards.zones.iter().enumerate() {
                    let (mut marked, mut hurt) = (0, 0);
                    for node in 0..world.grid.len() as NodeId {
                        marked += usize::from(hazards.zones_at(node).any(|z| z.zone == zone.zone));
                        hurt += usize::from(hazards.hurt_zones_at(node, HurtTest { floors: HurtFloors::Shape { band_permille: 0 }, by_intensity: false }).any(|z| z.zone == zone.zone));
                    }
                    let shapes = &world.map.zones[zone.zone].shapes;
                    let tops = shapes.iter().map(|s| s.bounds()).map(|(lo, hi)| (lo.y, hi.y)).collect::<Vec<_>>();
                    eprintln!(
                        "hurt_vs_marked {project} zone {h} {} ({} {:.3}/s): marked {marked}, hurt {hurt}; shapes y {:?}",
                        zone.name,
                        zone.damage_type,
                        zone.hit_per_s(),
                        tops
                    );
                }
            }
        }
    }

    /// The zones of each level's day project on the real maps, only with
    /// `SRP_MAPDATA_DIR=<mapdata-dump files>`: marking time, floors, and
    /// on the timer maps the switching zones' floors open and shut.
    ///
    /// ```text
    /// SRP_MAPDATA_DIR=<dir> cargo test -p match-server --release real_hazards -- --ignored --nocapture
    /// ```
    /// Where on a real map's floors a zone's hit has fallen to a given share: for the zone over Vector Laboratory's lower ring
    /// (the client-test step `zones-rim-26`), the floors at the ring's height by their normalised distance and intensity
    /// ([`Hazard::intensity_at`] at the body's centre), nearest the rim first. Prints only.
    ///
    /// ```text
    /// SRP_MAPDATA_DIR=<dir> cargo test -p match-server --release rim_floors -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
    fn rim_floors_of_the_zone_over_vector_laboratorys_lower_ring() {
        let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
            eprintln!("skipping: SRP_MAPDATA_DIR is not set");
            return;
        };
        let map = MapData::read(&dir.join(mapdata::format::file_name("level_07/day_2"))).unwrap();
        let world = World::build(map, mapdata::CELL, mapdata::Body::default());
        let hazards = Hazards::build(&world, 1.0);
        let grid = &world.grid;
        let ring = Vec3::new(3.8, 13.8, -8.6);
        let up = Vec3::new(0.0, 0.5 * grid.body().stand_height, 0.0);
        let Some(zone) = hazards
            .zones
            .iter()
            .min_by(|a, b| {
                let d = |z: &Hazard| Shape::least_normalised_distance(&z.shapes, ring + up);
                d(a).total_cmp(&d(b))
            })
        else {
            panic!("no zone");
        };
        eprintln!("zone {} ({} {:?}), curve {:?}", zone.name, zone.damage_type, zone.shapes, zone.damage.as_ref().map(|d| &d.hit_curve));
        eprintln!("ring: d {:.3}, intensity {:.3}", Shape::least_normalised_distance(&zone.shapes, ring + up), zone.intensity_at(ring + up));
        let mut rows: Vec<(f32, Vec3, f32, bool)> = (0..grid.len() as NodeId)
            .filter_map(|n| {
                let feet = grid.position(n);
                if (feet.y - ring.y).abs() > 0.6 || !hazards.zones_at(n).any(|z| z.zone == zone.zone) {
                    return None;
                }
                let d = Shape::least_normalised_distance(&zone.shapes, feet + up);
                let hurt = hazards.hurt_zones_at(n, HurtTest { floors: HurtFloors::Shape { band_permille: 0 }, by_intensity: false }).any(|z| z.zone == zone.zone);
                Some((d, feet, zone.intensity_at(feet + up), hurt))
            })
            .collect();
        rows.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (d, feet, share, hurt) in rows.iter().filter(|r| r.3 && r.0 < 0.97).take(16) {
            eprintln!("rim floor ({:.1}, {:.2}, {:.1}): d {d:.3}, intensity {share:.3}, in the capsule's reach {hurt}", feet.x, feet.y, feet.z);
        }
    }

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
