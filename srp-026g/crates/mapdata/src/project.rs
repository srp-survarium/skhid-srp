//! One map project (`projects/<level>/<weather>_<mode>/client_project`):
//! its objectives and its placed collision.
//!
//! The project is a binary config. What a bot needs of it:
//! - `server_objects.respawn_points[]`: `position`, `rotation`, `team`,
//!   `priority`, `point_id`, `layer`;
//! - `server_objects.victory_items_containers[]` (modes 2, 3): `position`,
//!   `team`, `id`, and use boxes named in `collision_geometries[].name`;
//! - `server_objects.victory_item_spawners[]` (modes 2, 3): where a battery
//!   lies (`spawned_item.position`);
//! - `server_objects.victory_item_spawn_points[]` (mode 3): stations;
//! - `client_objects[]` by `game_object_type`: `resupply_zone`,
//!   `damage_zone`, `effect_zone`, `artefact_container` with their
//!   `collision_geometries`, which name `collision_geometry` objects whose
//!   `meshes[]` are spheres (type 0), boxes (type 1, half sizes) and
//!   cylinders (type 2, radius and half height); a damage zone's hit
//!   (`max_hit`, `interval_in_msec`, ...);
//! - `generic_anomaly_core`: the damage zones it drives, its states and
//!   groups (which zones are on, when), energy and artefact settings;
//! - `collision_objects[]`: `lib_name`, `position`, `rotation`, `scale`,
//!   `cgroup`, `cmask`;
//! - `levelmap_min` / `levelmap_max`: the play area's x/z box (its y means
//!   nothing).
//!
//! All positions are in the map frame ([`crate::math`]).

use std::collections::HashMap;

use binary_config_parser::ConfigValue;

use crate::config::{children, float3, float3_or, float3_req, i32_or, str_or};
use crate::library::{Library, Materials, WalkerBehaviour};
use crate::math::{Rotation, Transform, Vec3};
use crate::resources::{Error, Resources};

/// A model's walking body's collision and its bullets' (`lib_name`s
/// `<model>.model/walker_collision#[..]` and `<model>.model/collision#[..]`).
const WALKER_COLLISION: &str = ".model/walker_collision";
const BULLET_COLLISION: &str = ".model/collision";

/// A body's collision filter: it meets an object when
/// `object.cgroup & body.mask != 0` and `object.cmask & body.group != 0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollisionFilter {
    pub group: u16,
    pub mask: u16,
}

impl CollisionFilter {
    /// The character controller (`character_controller.cpp:45-51`): meets
    /// `cgroup & 2` (the walker meshes and shared props). PROVEN.
    pub const WALKER: Self = Self { group: 4, mask: 2 };
    /// The bullet ray (`bullet.cpp:402`): meets `cgroup & 8`. PROVEN.
    pub const BULLET: Self = Self { group: 16, mask: 8 };

    pub fn meets(self, cgroup: u16, cmask: u16) -> bool {
        cgroup & self.mask != 0 && cmask & self.group != 0
    }
}

/// A respawn point. `team` is 0 or 1 (once 2, on level_02 TDM).
#[derive(Clone, Debug, PartialEq)]
pub struct Spawn {
    pub position: Vec3,
    /// Euler angles ([`Rotation::from_angles`]); [`crate::math::forward`]
    /// gives the facing.
    pub rotation: Vec3,
    pub team: i32,
    pub priority: i32,
    pub point_id: i32,
    pub layer: String,
}

/// A team's battery container (modes 2 and 3).
#[derive(Clone, Debug, PartialEq)]
pub struct Container {
    pub position: Vec3,
    pub team: i32,
    pub id: i32,
    /// Where a carrier must stand to hand a battery in.
    pub use_shapes: Vec<Shape>,
}

impl Container {
    /// Where a battery is put in or taken out, as a body's feet: a metre
    /// under the centre of its first use box (else its position), so the
    /// floor found there is the one beside the box, not its lid, and a look
    /// a metre over it is at the box. The use is a 2 m ray from the eye that
    /// must hit the use box (`base_player::detect_usable_object`, PROVEN);
    /// on level_02, 04, 05 and 06 the position lies 13 to 152 m from the
    /// box (on level_06 in the fire, both teams' within 4 m of each other),
    /// and players' put-ins are at the box (783 of them, 2026-10-14 worlds).
    pub fn use_point(&self) -> Vec3 {
        self.use_shapes.first().map_or(self.position, |shape| {
            shape.center - Vec3::new(0.0, 1.0, 0.0)
        })
    }
}

/// Where a battery lies at the start (modes 2 and 3).
#[derive(Clone, Debug, PartialEq)]
pub struct BatterySpawner {
    pub position: Vec3,
    /// `spawned_item.position`: the battery itself.
    pub item: Vec3,
    pub layer: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ZoneKind {
    Resupply = 0,
    Damage = 1,
    Effect = 2,
    ArtefactContainer = 3,
}

impl ZoneKind {
    pub fn from_game_object_type(name: &str) -> Option<Self> {
        Some(match name {
            "resupply_zone" => Self::Resupply,
            "damage_zone" => Self::Damage,
            "effect_zone" => Self::Effect,
            "artefact_container" => Self::ArtefactContainer,
            _ => return None,
        })
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Resupply,
            1 => Self::Damage,
            2 => Self::Effect,
            3 => Self::ArtefactContainer,
            _ => return None,
        })
    }
}

/// A zone: resupply (one per team), damage (anomaly, toxic), effect, or an
/// artefact container's pick-up place.
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub kind: ZoneKind,
    /// `full_name`; anomalies name their damage zones by it.
    pub name: String,
    /// -1 when the zone has no team.
    pub team: i32,
    pub position: Vec3,
    /// `damage_type` of a damage zone (`toxic`, ...), else empty.
    pub damage_type: String,
    /// What a damage zone does to a body in it; `None` for other kinds.
    pub damage: Option<Damage>,
    pub shapes: Vec<Shape>,
}

/// A damage zone's hit (`damage_zone` config; the zone side of the hit is
/// read in the client, 0x319AF0 / 0x31A410, PROVEN):
/// - `continuous`: every tick `max_hit × intensity × dt / interval` to each
///   of the `parts`;
/// - else one hit of `max_hit × intensity` to each part, at most once per
///   `interval_ms` per zone, the first on entry;
/// - `intensity = hit_curve(d)`, `d` 0 at the centre and 1 at the edge
///   (LIKELY); every shipped speed curve is 1.
///
/// Raw values before armour; health is 1.0.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Damage {
    /// `max_hit`; 0 for a zone that only shows or sounds (harmless).
    pub max_hit: f32,
    /// `max_armor_piercing`.
    pub armor_piercing: f32,
    /// `interval_in_msec`.
    pub interval_ms: u32,
    /// `continuous_hit`.
    pub continuous: bool,
    /// How many body parts each hit goes to (`hit_parts_filter`).
    pub parts: u8,
    /// `hit_curve` keys as `(d, value)`, sorted by `d`.
    pub hit_curve: Vec<(f32, f32)>,
}

impl Damage {
    /// Whether the zone hurts at all.
    pub fn harmful(&self) -> bool {
        self.max_hit > 0.0 && self.parts > 0
    }

    /// Raw damage a body takes in a second inside at full intensity
    /// (continuous zones), or from one hit (the others).
    pub fn raw(&self) -> f32 {
        self.max_hit * f32::from(self.parts)
    }

    /// The hit curve at `d` (0 centre, 1 edge): linear between keys,
    /// clamped to [0, 1]. Keys are Hermite in the game (GUESS: close
    /// enough for a cost); no keys is 1.
    pub fn intensity(&self, d: f32) -> f32 {
        let d = d.clamp(0.0, 1.0);
        let curve = &self.hit_curve;
        let value = match curve.iter().position(|&(x, _)| x >= d) {
            None => curve.last().map_or(1.0, |&(_, v)| v),
            Some(0) => curve[0].1,
            Some(i) => {
                let ((x0, v0), (x1, v1)) = (curve[i - 1], curve[i]);
                if x1 > x0 {
                    v0 + (v1 - v0) * (d - x0) / (x1 - x0)
                } else {
                    v1
                }
            }
        };
        value.clamp(0.0, 1.0)
    }
}

/// A `generic_anomaly_core`: its centre, the damage zones it drives and the
/// settings that decide which of them are on (`ANOMALIES` research of
/// 2026-10-14; the client's `generic_anomaly_core::tick` 0x31B5B0).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Anomaly {
    /// `full_name`: also what seeds its random stream (the CRC-32 of it).
    pub name: String,
    pub position: Vec3,
    /// Every zone its states' groups name, once each, in config order.
    pub zones: Vec<String>,
    pub energy: AnomalyEnergy,
    pub artefacts: AnomalyArtefacts,
    /// In config order (the client tries them by threshold, highest first).
    pub states: Vec<AnomalyState>,
}

/// An anomaly core's energy: it falls `decrease_speed` a second and rises
/// with what players do in its zones.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnomalyEnergy {
    /// `energy_enabled`.
    pub enabled: bool,
    /// `energy_initial`.
    pub initial: f32,
    /// `energy_decrease_speed`, a second.
    pub decrease_speed: f32,
    /// `energy_maximum` (0 in every shipped core).
    pub maximum: f32,
    /// `energy_on_walk`, `_run`, `_sprint` (a second, LIKELY), `_jump`.
    pub on_walk: f32,
    pub on_run: f32,
    pub on_sprint: f32,
    pub on_jump: f32,
    /// `energy_on_shoot`, `_explosion`, `_character_hit`,
    /// `_character_kill`, `energy_af_container_use`.
    pub on_shoot: f32,
    pub on_explosion: f32,
    pub on_character_hit: f32,
    pub on_character_kill: f32,
    pub on_af_container_use: f32,
}

/// An anomaly core's artefacts: their spawns draw from the core's random
/// stream too.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnomalyArtefacts {
    /// `artefacts_enabled`.
    pub enabled: bool,
    /// `artefacts_first_spawn_time_sec`, `_respawn_time_sec`,
    /// `_respawn_variation_sec`.
    pub first_spawn_s: f32,
    pub respawn_s: f32,
    pub variation_s: f32,
    /// `artefacts_respawn_chance`.
    pub chance: f32,
    /// `max_artefacts_count`.
    pub max_count: u32,
    /// How many `artefact_containers` it names.
    pub containers: u32,
}

/// One state of an anomaly core.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnomalyState {
    pub enabled: bool,
    /// `energy_threshold`: the state is on when the energy reaches it.
    pub threshold: f32,
    /// `active_time_sec`: 0 = while the energy holds; else the state
    /// lasts this long and then sets the energy to `energy_on_exit`.
    pub active_time_s: u32,
    pub energy_on_exit: f32,
    pub select_priority: i32,
    pub shoot_trigger: bool,
    pub zone_activity_trigger: bool,
    pub groups: Vec<ZoneGroup>,
}

/// A group of zones in a state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ZoneGroup {
    pub enabled: bool,
    /// `keep_all_zones_active`: every zone on while the state is.
    pub keep_all: bool,
    /// `sequential_activation`: one zone after another, each on for
    /// `min_active_s..max_active_s` (drawn).
    pub sequential: bool,
    pub min_active_s: u32,
    pub max_active_s: u32,
    /// `max_charged_count`.
    pub max_charged: u32,
    /// `recharge_time_sec`.
    pub recharge_s: u32,
    /// Zone `full_name`s, in config order (the order of the world's mask
    /// bits).
    pub zones: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ShapeKind {
    Sphere = 0,
    Box = 1,
    /// Round about the local y axis: radius `half.x`, half height `half.y`
    /// (mesh type 2; axis and half height LIKELY: the editor draws
    /// `radius, half_length, radius`).
    Cylinder = 2,
}

impl ShapeKind {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Sphere,
            1 => Self::Box,
            2 => Self::Cylinder,
            _ => return None,
        })
    }
}

/// A trigger shape of a `collision_geometry`, placed in the map frame:
/// the mesh's own rotation and position, then the object's scale, rotation
/// and position (the order of the engine's matrices; LIKELY, every shape
/// of the shipped maps has scale 1 and most no rotation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub kind: ShapeKind,
    pub center: Vec3,
    /// Local x, y, z axes in the map frame.
    pub rotation: Rotation,
    /// A box's half sizes; a sphere's radius is `half.x`; a cylinder's
    /// radius `half.x` and half height `half.y`.
    pub half: Vec3,
}

impl Shape {
    pub fn contains(&self, point: Vec3) -> bool {
        self.distance(point) <= 0.0
    }

    /// Metres from `point` to the shape, 0 inside.
    pub fn distance(&self, point: Vec3) -> f32 {
        let d = point - self.center;
        match self.kind {
            ShapeKind::Sphere => (d.length() - self.half.x).max(0.0),
            ShapeKind::Box => {
                let local = self.rotation.transposed().apply(d);
                Vec3::new(
                    (local.x.abs() - self.half.x).max(0.0),
                    (local.y.abs() - self.half.y).max(0.0),
                    (local.z.abs() - self.half.z).max(0.0),
                )
                .length()
            }
            ShapeKind::Cylinder => {
                let local = self.rotation.transposed().apply(d);
                let radial =
                    ((local.x * local.x + local.z * local.z).sqrt() - self.half.x).max(0.0);
                let vertical = (local.y.abs() - self.half.y).max(0.0);
                (radial * radial + vertical * vertical).sqrt()
            }
        }
    }

    /// The axis-aligned box round the shape, `(min, max)`.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let r = &self.rotation;
        // Each map axis's reach: the sum over the local axes of
        // |that axis' map component| × the local extent.
        let reach = |extent: Vec3| {
            let axis = |a: Vec3| a.x.abs() * extent.x + a.y.abs() * extent.y + a.z.abs() * extent.z;
            // The local axes are the rows i, j, k; a map axis gathers the
            // same component of each.
            Vec3::new(
                axis(Vec3::new(r.i.x, r.j.x, r.k.x)),
                axis(Vec3::new(r.i.y, r.j.y, r.k.y)),
                axis(Vec3::new(r.i.z, r.j.z, r.k.z)),
            )
        };
        let reach = match self.kind {
            ShapeKind::Sphere => Vec3::new(self.half.x, self.half.x, self.half.x),
            ShapeKind::Box => reach(self.half),
            ShapeKind::Cylinder => reach(Vec3::new(self.half.x, self.half.y, self.half.x)),
        };
        (self.center - reach, self.center + reach)
    }
}

/// One `collision_objects[]` entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Index into [`MapData::libraries`].
    pub library: u32,
    pub position: Vec3,
    pub rotation: Vec3,
    pub cgroup: u16,
    pub cmask: u16,
}

impl Placement {
    pub fn transform(&self) -> Transform {
        Transform::new(self.rotation, self.position)
    }
}

/// Metres added around the play area when a project has no `levelmap`.
pub const NO_LEVELMAP_MARGIN: f32 = 15.0;

/// A triangle placed in the map frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triangle {
    pub v: [Vec3; 3],
    pub walker: WalkerBehaviour,
}

/// Everything this crate knows of one project; what [`crate::format`]
/// stores.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapData {
    /// `level_01/day_4`.
    pub project: String,
    /// The `levelmap` box (zero when the project has none, as level_06).
    pub levelmap_min: Vec3,
    pub levelmap_max: Vec3,
    pub spawns: Vec<Spawn>,
    pub containers: Vec<Container>,
    pub battery_spawners: Vec<BatterySpawner>,
    /// Mode 3's charging stations.
    pub stations: Vec<Vec3>,
    pub zones: Vec<Zone>,
    pub anomalies: Vec<Anomaly>,
    /// Each collision library a placement uses, once, in its own space.
    pub libraries: Vec<Library>,
    pub placements: Vec<Placement>,
}

impl MapData {
    /// Loads `projects/<project>/client_project` and every library its
    /// walker and bullet placements use.
    pub fn load(
        resources: &mut Resources,
        materials: &Materials,
        project: &str,
    ) -> Result<Self, Error> {
        let root = resources.config(&format!("projects/{project}/client_project"))?;
        let mut map = Self::from_config(project, &root)?;
        let mut names: HashMap<String, u32> = HashMap::new();
        let mut placements = Vec::new();
        for object in children(&root, "collision_objects") {
            let filter_value = |key: &str| -> Result<u16, Error> {
                u16::try_from(object.i32_at(key)?)
                    .map_err(|_| Error(format!("collision object {key} out of range")))
            };
            let (cgroup, cmask) = (filter_value("cgroup")?, filter_value("cmask")?);
            if !CollisionFilter::WALKER.meets(cgroup, cmask)
                && !CollisionFilter::BULLET.meets(cgroup, cmask)
            {
                continue;
            }
            let lib_name = object.str_at("lib_name")?;
            let library = match names.get(lib_name) {
                Some(&index) => index,
                None => {
                    let index = map.libraries.len() as u32;
                    map.libraries
                        .push(Library::load(resources, materials, lib_name)?);
                    names.insert(lib_name.to_owned(), index);
                    index
                }
            };
            placements.push(Placement {
                library,
                position: float3_req(object, "position")?,
                rotation: float3_req(object, "rotation")?,
                cgroup,
                cmask,
            });
        }
        map.placements = placements;
        Ok(map)
    }

    /// The objectives and the level box of a parsed `client_project`
    /// (no collision).
    pub fn from_config(project: &str, root: &ConfigValue) -> Result<Self, Error> {
        let mut map = Self {
            project: project.to_owned(),
            levelmap_min: point_at(root, "levelmap_min"),
            levelmap_max: point_at(root, "levelmap_max"),
            ..Self::default()
        };
        // `collision_geometry` client objects by `full_name`.
        let geometries: HashMap<&str, &ConfigValue> = children(root, "client_objects")
            .into_iter()
            .filter(|o| str_or(o, "game_object_type") == "collision_geometry")
            .map(|o| (str_or(o, "full_name"), o))
            .collect();
        let shapes_of = |object: &ConfigValue| -> Vec<Shape> {
            let mut shapes = Vec::new();
            for reference in children(object, "collision_geometries") {
                let name = reference
                    .as_str()
                    .unwrap_or_else(|| str_or(reference, "name"));
                if let Some(geometry) = geometries.get(name) {
                    shapes.extend(geometry_shapes(geometry));
                }
            }
            shapes
        };

        let empty = ConfigValue::Named(Vec::new());
        let server = root.get("server_objects").unwrap_or(&empty);
        for point in children(server, "respawn_points") {
            map.spawns.push(Spawn {
                position: point_at(point, "position"),
                rotation: point_at(point, "rotation"),
                team: i32_or(point, "team", -1),
                priority: i32_or(point, "priority", 0),
                point_id: i32_or(point, "point_id", 0),
                layer: str_or(point, "layer").to_owned(),
            });
        }
        for container in children(server, "victory_items_containers") {
            map.containers.push(Container {
                position: point_at(container, "position"),
                team: i32_or(container, "team", -1),
                id: i32_or(container, "id", 0),
                use_shapes: shapes_of(container),
            });
        }
        for spawner in children(server, "victory_item_spawners") {
            let position = point_at(spawner, "position");
            map.battery_spawners.push(BatterySpawner {
                position,
                item: spawner
                    .get("spawned_item")
                    .and_then(|item| float3(item, "position"))
                    .unwrap_or(position),
                layer: str_or(spawner, "layer").to_owned(),
            });
        }
        // Stations sit under `server_objects`; a few projects keep an
        // (empty) copy at the top too.
        for source in [server, root] {
            for station in children(source, "victory_item_spawn_points") {
                map.stations.push(point_at(station, "position"));
            }
        }
        for object in children(root, "client_objects") {
            let kind = str_or(object, "game_object_type");
            if let Some(kind) = ZoneKind::from_game_object_type(kind) {
                map.zones.push(Zone {
                    kind,
                    name: str_or(object, "full_name").to_owned(),
                    team: i32_or(object, "team", -1),
                    position: point_at(object, "position"),
                    damage_type: str_or(object, "damage_type").to_owned(),
                    damage: (kind == ZoneKind::Damage).then(|| damage_of(object)),
                    shapes: shapes_of(object),
                });
            } else if kind == "generic_anomaly_core" {
                map.anomalies.push(anomaly_of(object));
            }
        }
        Ok(map)
    }

    /// Every triangle a body with `filter` meets, placed in the map frame.
    ///
    /// A walking body's placement of a `walker_collision` the archive does
    /// not have (fruit trees, level 3's anomalous trees, a bench, a wheel:
    /// 0-60 placements a map) meets the model's own `collision` instead,
    /// when the map has it. LIKELY: in the live logs of 2026-10-17..23
    /// bots stood caught against those trees' bullet meshes with nothing
    /// of the walker mesh within reach, and on level_02 two bodies at once
    /// stayed stuck for minutes inside an anomalous tree
    /// (`player_bots_stuck.md`); the engine's loading of a missing one is
    /// not read.
    pub fn triangles(&self, filter: CollisionFilter) -> Vec<Triangle> {
        let walking = filter == CollisionFilter::WALKER;
        let by_name: HashMap<&str, usize> = if walking {
            self.libraries
                .iter()
                .enumerate()
                .map(|(i, library)| (library.name.as_str(), i))
                .collect()
        } else {
            HashMap::new()
        };
        let library_of = |placement: &Placement| -> &Library {
            let library = &self.libraries[placement.library as usize];
            if walking && library.triangles.is_empty() {
                let stand_in = library
                    .name
                    .contains(WALKER_COLLISION)
                    .then(|| library.name.replacen(WALKER_COLLISION, BULLET_COLLISION, 1))
                    .and_then(|name| by_name.get(name.as_str()).copied());
                if let Some(index) = stand_in {
                    return &self.libraries[index];
                }
            }
            library
        };
        let count = self
            .placements
            .iter()
            .filter(|p| filter.meets(p.cgroup, p.cmask))
            .map(|p| library_of(p).triangles.len())
            .sum();
        let mut out = Vec::with_capacity(count);
        for placement in &self.placements {
            if !filter.meets(placement.cgroup, placement.cmask) {
                continue;
            }
            let library = library_of(placement);
            let transform = placement.transform();
            let world: Vec<Vec3> = library
                .vertices
                .iter()
                .map(|&v| transform.apply(v))
                .collect();
            for (t, &walker) in library.triangles.iter().zip(&library.walker) {
                out.push(Triangle {
                    v: t.map(|i| world[i as usize]),
                    walker,
                });
            }
        }
        out
    }

    /// Where a body is known to get to, for [`crate::WalkGrid::build_kept`]:
    /// every spawn, battery, container and charging station (an objective
    /// a body takes from a floor no spawn reaches still keeps its floor).
    pub fn grid_seeds(&self) -> Vec<Vec3> {
        let mut seeds: Vec<Vec3> = self.spawns.iter().map(|s| s.position).collect();
        seeds.extend(self.battery_spawners.iter().map(|b| b.item));
        seeds.extend(self.containers.iter().map(|c| c.position));
        seeds.extend(self.stations.iter().copied());
        seeds
    }

    /// The walker world: what the walking body stands on and bumps into.
    pub fn walker_triangles(&self) -> Vec<Triangle> {
        self.triangles(CollisionFilter::WALKER)
    }

    /// The bullet world: what stops shots and sight.
    pub fn bullet_triangles(&self) -> Vec<Triangle> {
        self.triangles(CollisionFilter::BULLET)
    }

    /// The area a walk grid should cover, as `[x0, z0, x1, z1]`: the
    /// `levelmap` box (the walker mesh reaches far outside play, to -666 m),
    /// widened by `margin` and to hold every spawn and objective. A box
    /// that is no box (level_03's is 2 m; level_06, 07 and 08 have none)
    /// gives way to where the collision objects stand (the middle 96 % of
    /// their positions on each axis), widened by a further
    /// [`NO_LEVELMAP_MARGIN`]: the objectives alone leave out ground
    /// between the bases (level_06 mode 3).
    pub fn play_area(&self, margin: f32) -> [f32; 4] {
        let mut lo = self.levelmap_min;
        let mut hi = self.levelmap_max;
        let mut points: Vec<Vec3> = self.spawns.iter().map(|s| s.position).collect();
        points.extend(self.containers.iter().map(|c| c.position));
        points.extend(self.battery_spawners.iter().map(|s| s.item));
        points.extend(self.stations.iter().copied());
        points.extend(self.zones.iter().map(|z| z.position));
        let mut margin = margin;
        if ((hi.x - lo.x) < 20.0 || (hi.z - lo.z) < 20.0) && !points.is_empty() {
            lo = points[0];
            hi = points[0];
            margin += NO_LEVELMAP_MARGIN;
            if !self.placements.is_empty() {
                let mut xs: Vec<f32> = self.placements.iter().map(|p| p.position.x).collect();
                let mut zs: Vec<f32> = self.placements.iter().map(|p| p.position.z).collect();
                xs.sort_by(f32::total_cmp);
                zs.sort_by(f32::total_cmp);
                let at = |v: &[f32], share: f32| v[((v.len() - 1) as f32 * share) as usize];
                points.push(Vec3::new(at(&xs, 0.02), lo.y, at(&zs, 0.02)));
                points.push(Vec3::new(at(&xs, 0.98), lo.y, at(&zs, 0.98)));
            }
        }
        for p in points {
            lo = lo.min(p);
            hi = hi.max(p);
        }
        [lo.x - margin, lo.z - margin, hi.x + margin, hi.z + margin]
    }
}

fn geometry_shapes(geometry: &ConfigValue) -> Vec<Shape> {
    let object_scale = float3_or(geometry, "scale", Vec3::ONE);
    let object = Transform::new(
        point_at(geometry, "rotation"),
        point_at(geometry, "position"),
    );
    let mut shapes = Vec::new();
    for mesh in children(geometry, "meshes") {
        // Type 3 (capsule, `primitive_type`) is in no shipped trigger.
        let Some(kind) = u8::try_from(i32_or(mesh, "type", -1))
            .ok()
            .and_then(ShapeKind::from_u8)
        else {
            continue;
        };
        let local = Rotation::from_angles(point_at(mesh, "rotation"));
        shapes.push(Shape {
            kind,
            center: object.apply(point_at(mesh, "position").scaled(object_scale)),
            rotation: local.then(&object.rotation),
            half: point_at(mesh, "scale").scaled(object_scale),
        });
    }
    shapes
}

/// A position or angles, zero when missing.
fn point_at(value: &ConfigValue, key: &str) -> Vec3 {
    float3_or(value, key, Vec3::ZERO)
}

fn f32_or(value: &ConfigValue, key: &str, default: f32) -> f32 {
    value
        .get(key)
        .and_then(ConfigValue::as_f32)
        .unwrap_or(default)
}

fn bool_or(value: &ConfigValue, key: &str, default: bool) -> bool {
    value
        .get(key)
        .and_then(ConfigValue::as_bool)
        .unwrap_or(default)
}

/// A whole number of seconds or a count, 0 when missing or negative.
fn u32_or_0(value: &ConfigValue, key: &str) -> u32 {
    u32::try_from(i32_or(value, key, 0)).unwrap_or(0)
}

fn damage_of(object: &ConfigValue) -> Damage {
    let mut hit_curve: Vec<(f32, f32)> = object
        .get("hit_curve")
        .map(|curve| {
            curve
                .children()
                .into_iter()
                .filter_map(|key| match key.get("position")? {
                    ConfigValue::Float2([d, v]) => Some((*d, *v)),
                    ConfigValue::Indexed(items) if items.len() == 2 => {
                        Some((items[0].as_f32()?, items[1].as_f32()?))
                    }
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    hit_curve.sort_by(|a, b| a.0.total_cmp(&b.0));
    Damage {
        max_hit: f32_or(object, "max_hit", 0.0),
        armor_piercing: f32_or(object, "max_armor_piercing", 0.0),
        interval_ms: u32_or_0(object, "interval_in_msec"),
        continuous: bool_or(object, "continuous_hit", false),
        parts: u8::try_from(children(object, "hit_parts_filter").len()).unwrap_or(u8::MAX),
        hit_curve,
    }
}

fn anomaly_of(object: &ConfigValue) -> Anomaly {
    let mut zones: Vec<String> = Vec::new();
    let states: Vec<AnomalyState> = children(object, "states")
        .into_iter()
        .map(|state| AnomalyState {
            enabled: bool_or(state, "enabled", true),
            threshold: f32_or(state, "energy_threshold", 0.0),
            active_time_s: u32_or_0(state, "active_time_sec"),
            energy_on_exit: f32_or(state, "energy_on_exit", 0.0),
            select_priority: i32_or(state, "select_priority", 0),
            shoot_trigger: bool_or(state, "shoot_trigger", false),
            zone_activity_trigger: bool_or(state, "zone_activity_trigger", false),
            groups: children(state, "groups")
                .into_iter()
                .map(|group| {
                    let names: Vec<String> = children(group, "zones")
                        .into_iter()
                        .filter_map(|zone| zone.as_str().map(str::to_owned))
                        .collect();
                    for name in &names {
                        if !zones.contains(name) {
                            zones.push(name.clone());
                        }
                    }
                    ZoneGroup {
                        enabled: bool_or(group, "enabled", true),
                        keep_all: bool_or(group, "keep_all_zones_active", false),
                        sequential: bool_or(group, "sequential_activation", false),
                        min_active_s: u32_or_0(group, "min_active_time_sec"),
                        max_active_s: u32_or_0(group, "max_active_time_sec"),
                        max_charged: u32_or_0(group, "max_charged_count"),
                        recharge_s: u32_or_0(group, "recharge_time_sec"),
                        zones: names,
                    }
                })
                .collect(),
        })
        .collect();
    let num = |key: &str| f32_or(object, key, 0.0);
    Anomaly {
        name: str_or(object, "full_name").to_owned(),
        position: point_at(object, "position"),
        zones,
        energy: AnomalyEnergy {
            enabled: bool_or(object, "energy_enabled", false),
            initial: num("energy_initial"),
            decrease_speed: num("energy_decrease_speed"),
            maximum: num("energy_maximum"),
            on_walk: num("energy_on_walk"),
            on_run: num("energy_on_run"),
            on_sprint: num("energy_on_sprint"),
            on_jump: num("energy_on_jump"),
            on_shoot: num("energy_on_shoot"),
            on_explosion: num("energy_on_explosion"),
            on_character_hit: num("energy_on_character_hit"),
            on_character_kill: num("energy_on_character_kill"),
            on_af_container_use: num("energy_af_container_use"),
        },
        artefacts: AnomalyArtefacts {
            enabled: bool_or(object, "artefacts_enabled", false),
            first_spawn_s: num("artefacts_first_spawn_time_sec"),
            respawn_s: num("artefacts_respawn_time_sec"),
            variation_s: num("artefacts_respawn_variation_sec"),
            chance: num("artefacts_respawn_chance"),
            max_count: u32_or_0(object, "max_artefacts_count"),
            containers: u32::try_from(children(object, "artefact_containers").len())
                .unwrap_or(u32::MAX),
        },
        states,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_follow_the_engine_groups() {
        // (8, 16) bullet-only, (2, 4) walker-only, (10, 20) both.
        assert!(CollisionFilter::BULLET.meets(8, 16));
        assert!(!CollisionFilter::WALKER.meets(8, 16));
        assert!(CollisionFilter::WALKER.meets(2, 4));
        assert!(!CollisionFilter::BULLET.meets(2, 4));
        assert!(CollisionFilter::WALKER.meets(10, 20));
        assert!(CollisionFilter::BULLET.meets(10, 20));
    }

    fn project_json() -> serde_json::Value {
        serde_json::json!({
            "levelmap_min": [-10.0, 0.0, -10.0],
            "levelmap_max": [10.0, 1.0, 10.0],
            "collision_objects": [],
            "server_objects": {
                "respawn_points": [
                    { "position": [1.0, 2.0, 3.0], "rotation": [0.0, 1.0, 0.0], "team": 1, "priority": 2, "point_id": 5, "layer": "default" },
                ],
                "victory_items_containers": [
                    { "position": [4.0, 0.0, 4.0], "team": 0, "id": 7, "collision_geometries": [ { "name": "use_box" } ] },
                ],
                "victory_item_spawners": [
                    { "position": [0.0, 0.0, 0.0], "spawned_item": { "position": [0.5, 0.2, 0.5] }, "layer": "mode_2" },
                ],
            },
            "client_objects": [
                { "game_object_type": "collision_geometry", "full_name": "use_box", "position": [4.0, 1.0, 4.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0],
                  "meshes": [ { "type": 1, "position": [0.0, 0.0, 1.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 0.5, 1.0] } ] },
                { "game_object_type": "collision_geometry", "full_name": "pool", "position": [-5.0, 0.0, -5.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0],
                  "meshes": [ { "type": 0, "position": [0.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0], "scale": [2.0, 2.0, 2.0] } ] },
                { "game_object_type": "collision_geometry", "full_name": "pillar", "position": [0.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0],
                  "meshes": [ { "type": 2, "position": [2.0, 1.0, 0.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 0.5, 1.0] } ] },
                { "game_object_type": "damage_zone", "full_name": "logic/pool_zone", "damage_type": "toxic", "position": [-5.0, 0.0, -5.0],
                  "max_hit": 0.07, "max_armor_piercing": 0, "interval_in_msec": 1000, "continuous_hit": true,
                  "hit_parts_filter": ["head", "left_foot"],
                  "hit_curve": { "key1": { "position": [1.0, 0.5] }, "key0": { "position": [0.8, 1.0] } },
                  "collision_geometries": [ { "name": "pool" }, { "name": "pillar" } ] },
                { "game_object_type": "resupply_zone", "full_name": "resupply", "team": 1, "position": [8.0, 0.0, 8.0], "collision_geometries": [] },
                { "game_object_type": "generic_anomaly_core", "full_name": "core", "position": [-5.0, 0.0, -5.0],
                  "energy_enabled": true, "energy_initial": 1, "energy_decrease_speed": 25,
                  "artefacts_enabled": true, "artefacts_first_spawn_time_sec": 60, "max_artefacts_count": 1,
                  "artefact_containers": ["a", "b"],
                  "states": [ { "energy_threshold": 1, "groups": [ { "zones": ["logic/pool_zone", "logic/other"],
                    "sequential_activation": true, "keep_all_zones_active": false,
                    "min_active_time_sec": 40, "max_active_time_sec": 60, "max_charged_count": 255 } ] } ] },
                { "game_object_type": "ambient_light", "full_name": "light", "position": [0.0, 0.0, 0.0] },
            ],
        })
    }

    /// A battery goes in at the use box (as on level_06, where the
    /// position lies 46-56 m off it in the fire): a metre under its centre,
    /// as a body's feet; the position only for a container with no box.
    #[test]
    fn a_container_is_used_at_its_use_box_not_its_position() {
        let config = binary_config_parser::BinaryConfig::parse_json(&project_json());
        let root = ConfigValue::parse(config.as_bytes()).unwrap();
        let mut container = MapData::from_config("test/day_4", &root)
            .unwrap()
            .containers[0]
            .clone();
        let centre = container.use_shapes[0].center;
        assert_eq!(centre, Vec3::new(4.0, 1.0, 5.0));
        assert_ne!(container.position.distance_xz(centre), 0.0);
        assert_eq!(container.use_point(), Vec3::new(4.0, 0.0, 5.0));
        container.use_shapes.clear();
        assert_eq!(container.use_point(), container.position);
    }

    #[test]
    fn reads_objectives_from_a_project() {
        let config = binary_config_parser::BinaryConfig::parse_json(&project_json());
        let root = ConfigValue::parse(config.as_bytes()).unwrap();
        let map = MapData::from_config("test/day_4", &root).unwrap();
        assert_eq!(map.spawns.len(), 1);
        assert_eq!(map.spawns[0].team, 1);
        assert_eq!(map.spawns[0].position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(map.containers[0].id, 7);
        let use_box = map.containers[0].use_shapes[0];
        assert!(use_box.contains(Vec3::new(4.5, 1.2, 5.5)));
        assert!(!use_box.contains(Vec3::new(4.0, 1.0, 3.5)));
        assert_eq!(map.battery_spawners[0].item, Vec3::new(0.5, 0.2, 0.5));
        assert_eq!(map.zones.len(), 2);
        let pool = &map.zones[0];
        assert_eq!(
            (pool.kind, pool.damage_type.as_str()),
            (ZoneKind::Damage, "toxic")
        );
        assert!(pool.shapes[0].contains(Vec3::new(-6.5, 0.0, -5.0)));
        assert!(!pool.shapes[0].contains(Vec3::new(-7.5, 0.0, -5.0)));
        // The cylinder: radius 1 round x = 2, z = 0, y 0.5..1.5.
        let pillar = pool.shapes[1];
        assert_eq!(pillar.kind, ShapeKind::Cylinder);
        assert!(pillar.contains(Vec3::new(2.7, 1.4, 0.7)));
        assert!(
            !pillar.contains(Vec3::new(2.8, 1.0, 0.8)),
            "outside the radius"
        );
        assert!(!pillar.contains(Vec3::new(2.0, 1.6, 0.0)), "above it");
        let damage = pool.damage.as_ref().unwrap();
        assert_eq!(
            (
                damage.max_hit,
                damage.interval_ms,
                damage.continuous,
                damage.parts
            ),
            (0.07, 1000, true, 2)
        );
        assert_eq!(damage.hit_curve, [(0.8, 1.0), (1.0, 0.5)]);
        assert!((damage.intensity(0.9) - 0.75).abs() < 1e-6);
        assert_eq!(damage.intensity(0.2), 1.0);
        assert_eq!(map.zones[1].team, 1);
        assert_eq!(map.zones[1].damage, None);
        let core = &map.anomalies[0];
        assert_eq!(core.zones, ["logic/pool_zone", "logic/other"]);
        assert_eq!(
            (
                core.energy.enabled,
                core.energy.initial,
                core.energy.decrease_speed
            ),
            (true, 1.0, 25.0)
        );
        assert_eq!(
            (core.artefacts.enabled, core.artefacts.first_spawn_s),
            (true, 60.0)
        );
        assert_eq!(
            (core.artefacts.max_count, core.artefacts.containers),
            (1, 2)
        );
        let group = &core.states[0].groups[0];
        assert_eq!(core.states[0].threshold, 1.0);
        assert!(group.sequential && !group.keep_all && group.enabled);
        assert_eq!(
            (group.min_active_s, group.max_active_s, group.max_charged),
            (40, 60, 255)
        );
        assert_eq!(map.play_area(5.0), [-15.0, -15.0, 15.0, 15.0]);
    }

    #[test]
    fn shapes_measure_distances_and_bounds() {
        let turned = Rotation::from_angles(Vec3::new(0.0, std::f32::consts::FRAC_PI_4, 0.0));
        let shape = |kind, half| Shape {
            kind,
            center: Vec3::new(1.0, 2.0, 3.0),
            rotation: turned,
            half,
        };
        let sphere = shape(ShapeKind::Sphere, Vec3::new(2.0, 2.0, 2.0));
        assert_eq!(sphere.distance(Vec3::new(1.0, 2.0, 3.0)), 0.0);
        assert!((sphere.distance(Vec3::new(1.0, 5.0, 3.0)) - 1.0).abs() < 1e-6);
        let cylinder = shape(ShapeKind::Cylinder, Vec3::new(1.0, 0.25, 1.0));
        // Round about y: turning it changes nothing.
        assert!((cylinder.distance(Vec3::new(3.0, 2.0, 3.0)) - 1.0).abs() < 1e-5);
        assert!((cylinder.distance(Vec3::new(1.0, 3.25, 3.0)) - 1.0).abs() < 1e-5);
        let corner = cylinder.distance(Vec3::new(3.0, 3.25, 3.0));
        assert!((corner - 2.0_f32.sqrt()).abs() < 1e-5, "{corner}");
        let (lo, hi) = cylinder.bounds();
        assert!((hi.y - lo.y - 0.5).abs() < 1e-5);
        assert!((hi.x - 1.0 - 2.0_f32.sqrt()).abs() < 1e-5, "{hi:?}");
        // A turned box's bounds hold its corners.
        let block = shape(ShapeKind::Box, Vec3::new(2.0, 0.5, 1.0));
        let (lo, hi) = block.bounds();
        for sx in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                let corner = block.center + turned.apply(Vec3::new(2.0 * sx, 0.5, 1.0 * sz));
                assert_eq!(block.distance(corner).round(), 0.0);
                assert!(corner.x >= lo.x - 1e-5 && corner.x <= hi.x + 1e-5);
                assert!(corner.z >= lo.z - 1e-5 && corner.z <= hi.z + 1e-5);
            }
        }
    }

    #[test]
    fn places_library_triangles_without_the_object_scale() {
        let library = Library {
            name: "lib".into(),
            vertices: vec![
                Vec3::ZERO,
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            triangles: vec![[0, 1, 2]],
            walker: vec![WalkerBehaviour::Walk],
            primitives: 0,
        };
        let map = MapData {
            libraries: vec![library],
            placements: vec![
                Placement {
                    library: 0,
                    position: Vec3::new(10.0, 1.0, 0.0),
                    rotation: Vec3::new(0.0, std::f32::consts::PI, 0.0),
                    cgroup: 2,
                    cmask: 4,
                },
                Placement {
                    library: 0,
                    position: Vec3::ZERO,
                    rotation: Vec3::ZERO,
                    cgroup: 8,
                    cmask: 16,
                },
            ],
            ..MapData::default()
        };
        let walker = map.walker_triangles();
        assert_eq!(walker.len(), 1);
        assert_eq!(walker[0].walker, WalkerBehaviour::Walk);
        let b = walker[0].v[1];
        assert!((b - Vec3::new(9.0, 1.0, 0.0)).length() < 1e-5, "{b:?}");
        assert_eq!(map.bullet_triangles().len(), 1);
    }

    /// A tree whose `walker_collision` the archive lacks: the walking body
    /// meets its `collision` (the same scale), the bullets as before; a
    /// model with neither has nothing.
    #[test]
    fn a_missing_walker_collision_is_stood_in_for_by_the_models_collision() {
        let empty = |name: &str| Library {
            name: name.into(),
            ..Library::default()
        };
        let trunk = Library {
            name: "resources/models/flora/tree.model/collision#[1.20][1.20][1.20]".into(),
            vertices: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::new(0.3, 0.0, 0.0),
            ],
            triangles: vec![[0, 1, 2]],
            walker: vec![WalkerBehaviour::BySlope],
            primitives: 0,
        };
        let placement = |library: u32, cgroup: u16, cmask: u16| Placement {
            library,
            position: Vec3::new(5.0, 0.0, 5.0),
            rotation: Vec3::ZERO,
            cgroup,
            cmask,
        };
        let map = MapData {
            libraries: vec![
                empty("resources/models/flora/tree.model/walker_collision#[1.20][1.20][1.20]"),
                trunk,
                empty("resources/models/flora/bush.model/walker_collision"),
                // Another scale's collision is not this placement's.
                empty("resources/models/flora/tree.model/walker_collision"),
            ],
            placements: vec![
                placement(0, 2, 4),
                placement(1, 8, 16),
                placement(2, 2, 4),
                placement(3, 2, 4),
            ],
            ..MapData::default()
        };
        let walker = map.walker_triangles();
        assert_eq!(walker.len(), 1, "the trunk, once");
        assert!((walker[0].v[1] - Vec3::new(5.0, 2.0, 5.0)).length() < 1e-5);
        assert_eq!(map.bullet_triangles().len(), 1);
    }
}
