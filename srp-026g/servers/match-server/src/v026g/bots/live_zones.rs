//! Anomaly fields that appear in the live match (`Tuning::live_zones`): a per-match second source of damage zones
//! next to the baked ones.
//!
//! The baked zones are a fact of the map file: [`super::hazards::HazardMarks`] marks the floors once, for every match on
//! the map. A field made at run time (the dynamic anomalies of the redesign) is not in the file, so the world tells
//! the bots: a source (`field_source`, the only place that knows how a world shows a field) makes the typed
//! [`WorldField`]s of a world, and [`FieldBoard::update`] marks the floors they cover, for this match only. The board answers the questions the baked marks answer, in the baked code's
//! own terms: when a field is on ([`Activity`], the same two functions), its hit a second, and which floors it shares
//! with another field ([`ZoneSet`]).
//!
//! What a field is not: it is never in the map's zone list (`Hazard::zone` indexes `MapData::zones`; a field has no
//! index there, so it is its own type), never "fixed" (a floor a live field marks is not crossable), and never part of
//! the regions' cut (floors only: a region does not shut because a field lies on a few of its floors).

use std::collections::HashMap;

use mapdata::{NodeId, Shape, WalkGrid};

use super::hazards::{near_floor, standing_heights, Activity, SWITCH_MARGIN_MS};

/// A field's id, stable for the field's life.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldId(pub u32);

/// What the field is made of (the pack's artefact kinds that lie in a lethal anomaly).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldKind {
    Lifebone,
    Spring,
    Onyx,
}

/// Where a field is in its life, in match milliseconds. A field that rests or idles is not a field: a source does not
/// return it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldPhase {
    /// Harmless until `damage_from` (the release: 20 s after it was made).
    Charging { damage_from: u32 },
    /// Hurting; `until` its end when it is known (the release: 4 s of damage, then [`Self::Holding`]).
    Damaging { until: Option<u32> },
    /// The artefact waits and the zone stays on, with no known end (until the artefact is taken).
    Holding,
}

/// One field of a world: what a source makes of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldField {
    pub id: FieldId,
    pub kind: FieldKind,
    /// Where it hurts, in the map's frame.
    pub shape: Shape,
    pub phase: FieldPhase,
}

/// A field's hit a second, of a body's health, by kind (GUESS: the pack's lethal anomalies take an unarmoured body in
/// 2 s at full intensity, the release's lifebone, spring and onyx alike; the map's zones are read the same way,
/// `Hazard::hit_per_s`).
pub fn hit_per_s(_kind: FieldKind) -> f32 {
    0.5
}

/// One field the board holds.
#[derive(Clone, Debug)]
pub struct LiveHazard {
    pub id: FieldId,
    pub kind: FieldKind,
    /// What the world said of it last (a changed field is the same id with another value).
    field: WorldField,
    /// Its hit a second while on ([`hit_per_s`]).
    pub hit_per_s: f32,
    /// When it hurts, widened by [`SWITCH_MARGIN_MS`].
    pub activity: Activity,
    /// The floors within the hazard margin of its shape, sorted.
    floors: Vec<NodeId>,
}

impl LiveHazard {
    /// The activity a phase gives: charging, off until its damage starts (`damage_from`); damaging, on until its
    /// end if it has one; holding, on, with no known end (until a world no longer lists the field).
    fn activity_of(phase: FieldPhase) -> Activity {
        match phase {
            FieldPhase::Charging { damage_from } => Activity::Spans(vec![(
                damage_from.saturating_sub(SWITCH_MARGIN_MS),
                u32::MAX,
            )]),
            FieldPhase::Damaging { until: Some(until) } => {
                Activity::Spans(vec![(0, until.saturating_add(SWITCH_MARGIN_MS))])
            }
            FieldPhase::Damaging { until: None } | FieldPhase::Holding => Activity::Always,
        }
    }

    /// Where it hurts.
    pub fn shape(&self) -> Shape {
        self.field.shape
    }

    /// What the world said of its phase last.
    pub fn phase(&self) -> FieldPhase {
        self.field.phase
    }

    /// The soonest match time after `now` at which it switches on or off, if it will.
    fn next_switch(&self, now: u32) -> Option<u32> {
        match &self.activity {
            Activity::Always => None,
            Activity::Spans(spans) => spans
                .iter()
                .flat_map(|&(from, to)| [from, to])
                .filter(|&t| t > now && t != u32::MAX)
                .min(),
        }
    }
}

/// The floors of `grid` within `margin` metres of `shape` where a standing body fits ([`near_floor`], the baked
/// marks' own test), sorted.
fn floors_of(grid: &WalkGrid, shape: &Shape, margin: f32) -> Vec<NodeId> {
    let (top, heights) = standing_heights(grid);
    let (lo, hi) = shape.bounds();
    let area = [lo.x - margin, lo.z - margin, hi.x + margin, hi.z + margin];
    let mut floors: Vec<NodeId> = grid
        .floors_in(area)
        .filter(|&node| near_floor(shape, (lo, hi), grid.position(node), top, &heights, margin))
        .collect();
    floors.sort_unstable();
    floors
}

/// A set of floors' fields, as the bots compare them: `0` none, else an id interned for the fields' ids, so it is
/// stable for as long as the same fields lie on a floor (a second escape from the same field is the same set).
type LiveSet = u16;

/// The live fields of one match.
#[derive(Clone, Debug, Default)]
pub struct FieldBoard {
    /// By id.
    fields: Vec<LiveHazard>,
    /// By floor: the live set (0 none). Empty until a field covers a floor.
    floor_set: Vec<LiveSet>,
    /// The fields (indices into `fields`) of each live set now in use.
    sets: HashMap<LiveSet, Vec<usize>>,
    /// Every set of field ids seen, with its id: kept for the match so an id means one set of fields.
    interned: HashMap<Vec<FieldId>, LiveSet>,
    /// Counts the changes: a field appears, changes phase, or goes.
    version: u64,
    /// The match time of the last update (for [`Self::next_switch`]).
    now: u32,
}

impl FieldBoard {
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn fields(&self) -> &[LiveHazard] {
        &self.fields
    }

    /// Changes whenever a field appears, changes phase or goes.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Takes the fields of a world (match time `now`) as the board: new ones marked, changed ones made again, absent
    /// ones dropped at once. A field unchanged since the last world keeps its floors. Returns whether anything
    /// changed. `margin` is the hazard margin.
    pub fn update(&mut self, world: &[WorldField], grid: &WalkGrid, margin: f32, now: u32) -> bool {
        self.now = now;
        let mut sorted: Vec<&WorldField> = world.iter().collect();
        sorted.sort_by_key(|f| f.id);
        sorted.dedup_by_key(|f| f.id);
        if sorted.len() == self.fields.len() && sorted.iter().zip(&self.fields).all(|(w, h)| **w == h.field) {
            return false;
        }
        let mut next: Vec<LiveHazard> = Vec::with_capacity(sorted.len());
        for field in sorted {
            let held = self.fields.iter().position(|h| h.id == field.id);
            let hazard = match held {
                // The same place: its floors stay, its phase is made again.
                Some(k) if same_place(&self.fields[k].field, field) => LiveHazard {
                    field: *field,
                    activity: LiveHazard::activity_of(field.phase),
                    ..self.fields[k].clone()
                },
                _ => LiveHazard {
                    id: field.id,
                    kind: field.kind,
                    field: *field,
                    hit_per_s: hit_per_s(field.kind),
                    activity: LiveHazard::activity_of(field.phase),
                    floors: floors_of(grid, &field.shape, margin),
                },
            };
            next.push(hazard);
        }
        self.remark(&next, grid.len());
        self.fields = next;
        self.version += 1;
        true
    }

    /// Marks the floors of `fields` (and unmarks those of the old ones) with their sets.
    fn remark(&mut self, fields: &[LiveHazard], floors: usize) {
        for old in &self.fields {
            for &node in &old.floors {
                if let Some(slot) = self.floor_set.get_mut(node as usize) {
                    *slot = 0;
                }
            }
        }
        self.sets.clear();
        if fields.iter().all(|f| f.floors.is_empty()) {
            return;
        }
        if self.floor_set.len() != floors {
            self.floor_set = vec![0; floors];
        }
        // Each covered floor's fields, in order of id.
        let mut at: HashMap<NodeId, Vec<usize>> = HashMap::new();
        for (k, field) in fields.iter().enumerate() {
            for &node in &field.floors {
                at.entry(node).or_default().push(k);
            }
        }
        for (node, members) in at {
            let ids: Vec<FieldId> = members.iter().map(|&k| fields[k].id).collect();
            let next = (self.interned.len() + 1).min(usize::from(LiveSet::MAX)) as LiveSet;
            let id = *self.interned.entry(ids).or_insert(next);
            self.floor_set[node as usize] = id;
            self.sets.insert(id, members);
        }
    }

    /// The floors some field covers (a floor under two counts once).
    pub fn floors_covered(&self) -> usize {
        self.floor_set.iter().filter(|&&set| set != 0).count()
    }

    /// The live set of `node` (0: no field lies on it).
    pub fn set_of(&self, node: NodeId) -> LiveSet {
        self.floor_set.get(node as usize).copied().unwrap_or(0)
    }

    /// The fields lying on `node`.
    pub fn at(&self, node: NodeId) -> impl Iterator<Item = &LiveHazard> + '_ {
        let members: &[usize] = match self.set_of(node) {
            0 => &[],
            id => self.sets.get(&id).map_or(&[], Vec::as_slice),
        };
        members.iter().map(|&k| &self.fields[k])
    }

    /// Whether every field at `node` is off from `time` for `ms` ([`Activity::off_for`]).
    pub fn allowed_for(&self, node: NodeId, time: u32, ms: u32) -> bool {
        self.at(node).all(|f| f.activity.off_for(time, ms))
    }

    /// Whether a field at `node` is on at `time` (no margin of time).
    pub fn on_at(&self, node: NodeId, time: u32) -> bool {
        self.at(node).any(|f| f.activity.active(time))
    }

    /// The fields' hit a second at `node` while they are on, summed.
    pub fn hit_per_s(&self, node: NodeId) -> f32 {
        self.at(node).map(|f| f.hit_per_s).sum()
    }

    /// When the fields at `node` next switch, the soonest (match ms).
    pub fn next_switch(&self, node: NodeId) -> Option<u32> {
        self.at(node).filter_map(|f| f.next_switch(self.now)).min()
    }
}

/// Whether the world's field is where the board's was (only its phase differs): its floors stay.
fn same_place(old: &WorldField, new: &WorldField) -> bool {
    old.kind == new.kind && old.shape == new.shape
}

/// The set of zones marking a floor, baked and live: what the bots compare to tell "the same zone as before" (`0` none).
/// The low 16 bits are the baked set (`HazardMarks`), the high 16 the live set ([`FieldBoard`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ZoneSet(u32);

impl ZoneSet {
    pub fn new(baked: u16, live: u16) -> Self {
        Self(u32::from(baked) | u32::from(live) << 16)
    }

    /// The baked set (the regions' label, the exit field).
    pub fn baked(self) -> u16 {
        self.0 as u16
    }

    pub fn live(self) -> u16 {
        (self.0 >> 16) as u16
    }

    /// No zone, baked or live.
    pub fn is_none(self) -> bool {
        self.0 == 0
    }
}
