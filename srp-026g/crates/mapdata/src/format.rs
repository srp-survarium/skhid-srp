//! The file `mapdata-dump` writes, one per project: everything in
//! [`MapData`], so a server loads maps without the game's `resources.db`.
//!
//! Little endian. A string is `u32 length` + UTF-8; a list is `u32 count` +
//! items; a vector is three `f32`.
//!
//! ```text
//! magic "SRPMAPD\0", u32 version (3)
//! string project
//! vec3 levelmap_min, vec3 levelmap_max
//! list spawns:      vec3 position, vec3 rotation, i32 team, i32 priority,
//!                   i32 point_id, string layer
//! list containers:  vec3 position, i32 team, i32 id, list shapes
//! list spawners:    vec3 position, vec3 item, string layer
//! list stations:    vec3
//! list zones:       u8 kind, string name, i32 team, vec3 position,
//!                   string damage_type, u8 has_damage, [damage], list shapes
//! list anomalies:   string name, vec3 position, list string zones,
//!                   energy, artefacts, list states
//! list artefact_cores (version 3):
//!                   string core, list (u32 dict_id, u32 count) pool,
//!                   list (string name, f32 search_s, f32 cooldown_s) containers
//! list zone_hit_parts (version 4):
//!                   per zone, in the zones' order: list string (the names of
//!                   the parts its hits go to, `hit_parts_filter`)
//! list libraries:   string name, u32 primitives, list vec3 vertices,
//!                   list [u32; 3] triangles, then one u8 walker behaviour
//!                   per triangle, then (version 5) one f32 armor per
//!                   triangle (`physic.armor` of its game material: what a
//!                   bullet meets there, `crate::pierce`; NaN: the material
//!                   is not known)
//! list placements:  u32 library, vec3 position, vec3 rotation,
//!                   u16 cgroup, u16 cmask
//! magic "END\0"
//! shape:            u8 kind (0 sphere, 1 box, 2 cylinder), vec3 center,
//!                   3 × vec3 rotation rows, vec3 half
//! damage:           f32 max_hit, f32 armor_piercing, u32 interval_ms,
//!                   u8 continuous, u8 parts, list (f32 d, f32 value) hit_curve
//! energy:           u8 enabled, 11 × f32: initial, decrease_speed, maximum,
//!                   on_walk, on_run, on_sprint, on_jump, on_shoot,
//!                   on_explosion, on_character_hit, on_character_kill,
//!                   then f32 on_af_container_use
//! artefacts:        u8 enabled, f32 first_spawn_s, f32 respawn_s,
//!                   f32 variation_s, f32 chance, u32 max_count, u32 containers
//! state:            u8 enabled, f32 threshold, u32 active_time_s,
//!                   f32 energy_on_exit, i32 select_priority,
//!                   u8 shoot_trigger, u8 zone_activity_trigger, list groups
//! group:            u8 enabled, u8 keep_all, u8 sequential, u32 min_active_s,
//!                   u32 max_active_s, u32 max_charged, u32 recharge_s,
//!                   list string zones
//! ```
//!
//! Version 2 (2026-10-14) added cylinders, the zones' damage and the
//! anomalies' settings. A version 1 file is refused (its zones would lack
//! both and bots would walk through anomalies): dump the maps again.
//! Version 3 (2026-10-27) added the artefact cores after the anomalies; a
//! version 2 file is still read, with none (what the bots know of the
//! artefact places then stays unknown until the map is dumped again).
//!
//! Version 4 (2026-10-27) added the names of the parts a zone's hits go to
//! after the artefact cores (a zone's pools and the armour it meets depend on
//! them); a version 2 or 3 file is still read, with no names (the zones keep
//! their count of parts, and everything that read the file before reads it
//! the same).
//!
//! Version 5 (2026-10-28) added the bullet armor of every library triangle
//! after the walker behaviours; a version 2 to 4 file is still read, with
//! every face of unknown material (`FaceArmor::Unknown`: it stops every bullet, as the bullet mesh was
//! taken before), so the bots' cover is what it was until the maps are
//! dumped again.
//!
//! Reading checks every count against the bytes left, so a damaged file is
//! an error, never a panic or a huge allocation.

use std::path::Path;

use crate::library::{Face, Library, Surface, WalkerBehaviour};
use crate::pierce::FaceArmor;
use crate::math::{Rotation, Vec3};
use crate::project::{
    Anomaly, AnomalyArtefacts, AnomalyEnergy, AnomalyState, ArtefactContainer, ArtefactCore,
    BatterySpawner, Container, Damage,
    MapData, Placement, Shape, ShapeKind, Spawn, Zone, ZoneGroup, ZoneKind,
};
use crate::resources::Error;

const MAGIC: &[u8; 8] = b"SRPMAPD\0";
const END: &[u8; 4] = b"END\0";
pub const VERSION: u32 = 5;
/// The oldest version [`MapData::from_bytes`] still reads.
pub const OLDEST: u32 = 2;
/// The first version that carries the bullet armor of the faces: an older file is read with every face of unknown
/// material, which stops every bullet (so no pierce rule ever opens a line on it).
pub const FIRST_WITH_ARMOR: u32 = 5;
const _: () = assert!(FIRST_WITH_ARMOR <= VERSION);
/// The file name extension.
pub const EXTENSION: &str = "mapdata";

/// The file name for a project: `level_01/day_4` → `level_01_day_4.mapdata`.
pub fn file_name(project: &str) -> String {
    format!("{}.{EXTENSION}", project.replace(['/', '\\'], "_"))
}

impl MapData {
    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes_of(VERSION)
    }

    /// The file of `version` (the current one, or [`OLDEST`] for a test of the reader).
    fn to_bytes_of(&self, version: u32) -> Vec<u8> {
        let mut w = Writer(Vec::with_capacity(1 << 20));
        w.0.extend_from_slice(MAGIC);
        w.u32(version);
        w.str(&self.project);
        w.vec3(self.levelmap_min);
        w.vec3(self.levelmap_max);
        w.list(&self.spawns, |w, s| {
            w.vec3(s.position);
            w.vec3(s.rotation);
            w.i32(s.team);
            w.i32(s.priority);
            w.i32(s.point_id);
            w.str(&s.layer);
        });
        w.list(&self.containers, |w, c| {
            w.vec3(c.position);
            w.i32(c.team);
            w.i32(c.id);
            w.shapes(&c.use_shapes);
        });
        w.list(&self.battery_spawners, |w, s| {
            w.vec3(s.position);
            w.vec3(s.item);
            w.str(&s.layer);
        });
        w.list(&self.stations, |w, &s| w.vec3(s));
        w.list(&self.zones, |w, z| {
            w.0.push(z.kind as u8);
            w.str(&z.name);
            w.i32(z.team);
            w.vec3(z.position);
            w.str(&z.damage_type);
            w.bool(z.damage.is_some());
            if let Some(d) = &z.damage {
                w.f32(d.max_hit);
                w.f32(d.armor_piercing);
                w.u32(d.interval_ms);
                w.bool(d.continuous);
                w.0.push(d.parts);
                w.list(&d.hit_curve, |w, &(x, v)| {
                    w.f32(x);
                    w.f32(v);
                });
            }
            w.shapes(&z.shapes);
        });
        w.list(&self.anomalies, |w, a| {
            w.str(&a.name);
            w.vec3(a.position);
            w.list(&a.zones, |w, z| w.str(z));
            let e = &a.energy;
            w.bool(e.enabled);
            for v in [
                e.initial,
                e.decrease_speed,
                e.maximum,
                e.on_walk,
                e.on_run,
                e.on_sprint,
                e.on_jump,
                e.on_shoot,
                e.on_explosion,
                e.on_character_hit,
                e.on_character_kill,
                e.on_af_container_use,
            ] {
                w.f32(v);
            }
            let f = &a.artefacts;
            w.bool(f.enabled);
            w.f32(f.first_spawn_s);
            w.f32(f.respawn_s);
            w.f32(f.variation_s);
            w.f32(f.chance);
            w.u32(f.max_count);
            w.u32(f.containers);
            w.list(&a.states, |w, s| {
                w.bool(s.enabled);
                w.f32(s.threshold);
                w.u32(s.active_time_s);
                w.f32(s.energy_on_exit);
                w.i32(s.select_priority);
                w.bool(s.shoot_trigger);
                w.bool(s.zone_activity_trigger);
                w.list(&s.groups, |w, g| {
                    w.bool(g.enabled);
                    w.bool(g.keep_all);
                    w.bool(g.sequential);
                    w.u32(g.min_active_s);
                    w.u32(g.max_active_s);
                    w.u32(g.max_charged);
                    w.u32(g.recharge_s);
                    w.list(&g.zones, |w, z| w.str(z));
                });
            });
        });
        if version >= 3 {
            w.list(&self.artefact_cores, |w, a| {
                w.str(&a.core);
                w.list(&a.pool, |w, &(id, count)| {
                    w.u32(id);
                    w.u32(count);
                });
                w.list(&a.containers, |w, c| {
                    w.str(&c.name);
                    w.f32(c.search_s);
                    w.f32(c.cooldown_s);
                });
            });
        }
        if version >= 4 {
            w.list(&self.zones, |w, z| {
                let names = z.damage.as_ref().map_or(&[][..], |d| &d.part_names[..]);
                w.list(names, |w, n| w.str(n));
            });
        }
        w.list(&self.libraries, |w, l| {
            w.str(&l.name);
            w.u32(l.primitives);
            w.list(&l.vertices, |w, &v| w.vec3(v));
            w.list(&l.faces, |w, f| f.corners.iter().for_each(|&i| w.u32(i)));
            w.0.extend(l.faces.iter().map(|f| f.surface.walker as u8));
            if version >= 5 {
                for face in &l.faces {
                    w.f32(face.surface.armor.to_wire());
                }
            }
        });
        w.list(&self.placements, |w, p| {
            w.u32(p.library);
            w.vec3(p.position);
            w.vec3(p.rotation);
            w.0.extend_from_slice(&p.cgroup.to_le_bytes());
            w.0.extend_from_slice(&p.cmask.to_le_bytes());
        });
        w.0.extend_from_slice(END);
        w.0
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let mut r = Reader { bytes, at: 0 };
        if r.take(8)? != MAGIC {
            return Err(Error("not a map data file".into()));
        }
        let version = r.u32()?;
        if !(OLDEST..=VERSION).contains(&version) {
            return Err(Error(format!(
                "map data version {version}, this build reads {OLDEST} to {VERSION}: \
                 write the file again with this build's mapdata-dump"
            )));
        }
        let mut map = MapData {
            project: r.str()?,
            levelmap_min: r.vec3()?,
            levelmap_max: r.vec3()?,
            ..MapData::default()
        };
        map.spawns = r.list(40, |r| {
            Ok(Spawn {
                position: r.vec3()?,
                rotation: r.vec3()?,
                team: r.i32()?,
                priority: r.i32()?,
                point_id: r.i32()?,
                layer: r.str()?,
            })
        })?;
        map.containers = r.list(24, |r| {
            Ok(Container {
                position: r.vec3()?,
                team: r.i32()?,
                id: r.i32()?,
                use_shapes: r.shapes()?,
            })
        })?;
        map.battery_spawners = r.list(28, |r| {
            Ok(BatterySpawner {
                position: r.vec3()?,
                item: r.vec3()?,
                layer: r.str()?,
            })
        })?;
        map.stations = r.list(12, Reader::vec3)?;
        map.zones = r.list(30, |r| {
            let kind = r.u8()?;
            Ok(Zone {
                kind: ZoneKind::from_u8(kind)
                    .ok_or_else(|| Error(format!("unknown zone kind {kind}")))?,
                name: r.str()?,
                team: r.i32()?,
                position: r.vec3()?,
                damage_type: r.str()?,
                damage: if r.bool()? {
                    Some(Damage {
                        max_hit: r.f32()?,
                        armor_piercing: r.f32()?,
                        interval_ms: r.u32()?,
                        continuous: r.bool()?,
                        parts: r.u8()?,
                        part_names: Vec::new(),
                        hit_curve: r.list(8, |r| Ok((r.f32()?, r.f32()?)))?,
                    })
                } else {
                    None
                },
                shapes: r.shapes()?,
            })
        })?;
        map.anomalies = r.list(98, |r| {
            Ok(Anomaly {
                name: r.str()?,
                position: r.vec3()?,
                zones: r.list(4, Reader::str)?,
                energy: AnomalyEnergy {
                    enabled: r.bool()?,
                    initial: r.f32()?,
                    decrease_speed: r.f32()?,
                    maximum: r.f32()?,
                    on_walk: r.f32()?,
                    on_run: r.f32()?,
                    on_sprint: r.f32()?,
                    on_jump: r.f32()?,
                    on_shoot: r.f32()?,
                    on_explosion: r.f32()?,
                    on_character_hit: r.f32()?,
                    on_character_kill: r.f32()?,
                    on_af_container_use: r.f32()?,
                },
                artefacts: AnomalyArtefacts {
                    enabled: r.bool()?,
                    first_spawn_s: r.f32()?,
                    respawn_s: r.f32()?,
                    variation_s: r.f32()?,
                    chance: r.f32()?,
                    max_count: r.u32()?,
                    containers: r.u32()?,
                },
                states: r.list(23, |r| {
                    Ok(AnomalyState {
                        enabled: r.bool()?,
                        threshold: r.f32()?,
                        active_time_s: r.u32()?,
                        energy_on_exit: r.f32()?,
                        select_priority: r.i32()?,
                        shoot_trigger: r.bool()?,
                        zone_activity_trigger: r.bool()?,
                        groups: r.list(23, |r| {
                            Ok(ZoneGroup {
                                enabled: r.bool()?,
                                keep_all: r.bool()?,
                                sequential: r.bool()?,
                                min_active_s: r.u32()?,
                                max_active_s: r.u32()?,
                                max_charged: r.u32()?,
                                recharge_s: r.u32()?,
                                zones: r.list(4, Reader::str)?,
                            })
                        })?,
                    })
                })?,
            })
        })?;
        if version >= 3 {
            map.artefact_cores = r.list(12, |r| {
                Ok(ArtefactCore {
                    core: r.str()?,
                    pool: r.list(8, |r| Ok((r.u32()?, r.u32()?)))?,
                    containers: r.list(12, |r| {
                        Ok(ArtefactContainer {
                            name: r.str()?,
                            search_s: r.f32()?,
                            cooldown_s: r.f32()?,
                        })
                    })?,
                })
            })?;
        }
        if version >= 4 {
            let names = r.list(4, |r| r.list(4, Reader::str))?;
            if names.len() != map.zones.len() {
                return Err(Error(format!(
                    "map data names the parts of {} zones, the file has {}",
                    names.len(),
                    map.zones.len()
                )));
            }
            for (zone, names) in map.zones.iter_mut().zip(names) {
                if let Some(damage) = &mut zone.damage {
                    damage.part_names = names;
                }
            }
        }
        map.libraries = r.list(16, |r| {
            let name = r.str()?;
            let primitives = r.u32()?;
            let vertices = r.list(12, Reader::vec3)?;
            let triangles = r.list(12, |r| Ok([r.u32()?, r.u32()?, r.u32()?]))?;
            if triangles
                .iter()
                .flatten()
                .any(|&i| i as usize >= vertices.len())
            {
                return Err(Error(format!("{name}: a triangle names no vertex")));
            }
            let walker: Vec<WalkerBehaviour> = r
                .take(triangles.len())?
                .iter()
                .map(|&b| WalkerBehaviour::from_u8(b))
                .collect();
            let mut faces = Vec::with_capacity(triangles.len());
            for (corners, walker) in triangles.into_iter().zip(walker) {
                // Before format 5 nothing says what a face is made of.
                let armor = if version >= 5 {
                    let value = r.f32()?;
                    FaceArmor::from_wire(value)
                        .ok_or_else(|| Error(format!("{name}: armor {value} is no armor")))?
                } else {
                    FaceArmor::Unknown
                };
                faces.push(Face { corners, surface: Surface { walker, armor } });
            }
            Ok(Library { name, vertices, faces, primitives })
        })?;
        map.placements = r.list(32, |r| {
            Ok(Placement {
                library: r.u32()?,
                position: r.vec3()?,
                rotation: r.vec3()?,
                cgroup: r.u16()?,
                cmask: r.u16()?,
            })
        })?;
        if map
            .placements
            .iter()
            .any(|p| p.library as usize >= map.libraries.len())
        {
            return Err(Error("a placement names no library".into()));
        }
        if r.take(4)? != END || r.at != bytes.len() {
            return Err(Error("map data file has a bad end".into()));
        }
        Ok(map)
    }

    pub fn write(&self, path: &Path) -> Result<(), Error> {
        std::fs::write(path, self.to_bytes())
            .map_err(|error| Error(format!("{}: {error}", path.display())))
    }

    pub fn read(path: &Path) -> Result<Self, Error> {
        let bytes =
            std::fs::read(path).map_err(|error| Error(format!("{}: {error}", path.display())))?;
        Self::from_bytes(&bytes).map_err(|error| Error(format!("{}: {error}", path.display())))
    }
}

/// The version a map data file was written in, read from its first twelve bytes alone (the magic and the version).
pub fn file_version(path: &Path) -> Result<u32, Error> {
    use std::io::Read as _;
    let mut head = [0u8; 12];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut head))
        .map_err(|error| Error(format!("{}: {error}", path.display())))?;
    if head[..8] != MAGIC[..] {
        return Err(Error(format!("{}: not a map data file", path.display())));
    }
    Ok(u32::from_le_bytes([head[8], head[9], head[10], head[11]]))
}

struct Writer(Vec<u8>);

impl Writer {
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn bool(&mut self, v: bool) {
        self.0.push(u8::from(v));
    }

    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn vec3(&mut self, v: Vec3) {
        self.f32(v.x);
        self.f32(v.y);
        self.f32(v.z);
    }

    fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
    }

    fn list<T>(&mut self, items: &[T], mut each: impl FnMut(&mut Self, &T)) {
        self.u32(items.len() as u32);
        for item in items {
            each(self, item);
        }
    }

    fn shapes(&mut self, shapes: &[Shape]) {
        self.list(shapes, |w, s| {
            w.0.push(s.kind as u8);
            w.vec3(s.center);
            w.vec3(s.rotation.i);
            w.vec3(s.rotation.j);
            w.vec3(s.rotation.k);
            w.vec3(s.half);
        });
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&end| end <= self.bytes.len())
            .ok_or_else(|| Error(format!("map data file ends early (at {})", self.at)))?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    fn bool(&mut self) -> Result<bool, Error> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(Error(format!("map data flag {other} is not 0 or 1"))),
        }
    }

    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn vec3(&mut self) -> Result<Vec3, Error> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }

    fn str(&mut self) -> Result<String, Error> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec())
            .map_err(|_| Error("map data file has a bad string".into()))
    }

    /// A list whose items take at least `min_size` bytes each.
    fn list<T>(
        &mut self,
        min_size: usize,
        mut each: impl FnMut(&mut Self) -> Result<T, Error>,
    ) -> Result<Vec<T>, Error> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min_size) > self.bytes.len() - self.at {
            return Err(Error(format!(
                "map data list of {n} is longer than the file"
            )));
        }
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(each(self)?);
        }
        Ok(out)
    }

    fn shapes(&mut self) -> Result<Vec<Shape>, Error> {
        self.list(61, |r| {
            let kind = r.u8()?;
            let kind = ShapeKind::from_u8(kind)
                .ok_or_else(|| Error(format!("unknown shape kind {kind}")))?;
            Ok(Shape {
                kind,
                center: r.vec3()?,
                rotation: Rotation {
                    i: r.vec3()?,
                    j: r.vec3()?,
                    k: r.vec3()?,
                },
                half: r.vec3()?,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pierce::Armor;

    fn sample() -> MapData {
        let shape = Shape {
            kind: ShapeKind::Box,
            center: Vec3::new(1.0, 2.0, 3.0),
            rotation: Rotation::from_angles(Vec3::new(0.0, 0.5, 0.0)),
            half: Vec3::new(0.5, 1.0, 0.5),
        };
        MapData {
            project: "level_01/day_4".into(),
            levelmap_min: Vec3::new(-1.0, 0.0, -2.0),
            levelmap_max: Vec3::new(10.0, 1.0, 20.0),
            spawns: vec![Spawn {
                position: Vec3::new(1.0, 2.0, 3.0),
                rotation: Vec3::new(0.0, 1.0, 0.0),
                team: 1,
                priority: 2,
                point_id: 3,
                layer: "default".into(),
            }],
            containers: vec![Container {
                position: Vec3::ZERO,
                team: 0,
                id: 4,
                use_shapes: vec![shape],
            }],
            battery_spawners: vec![BatterySpawner {
                position: Vec3::Y,
                item: Vec3::ONE,
                layer: "mode_2".into(),
            }],
            stations: vec![Vec3::new(5.0, 0.0, 5.0)],
            zones: vec![Zone {
                kind: ZoneKind::Damage,
                name: "logic/zone".into(),
                team: -1,
                position: Vec3::ONE,
                damage_type: "toxic".into(),
                damage: Some(Damage {
                    max_hit: 0.6,
                    armor_piercing: 0.0,
                    interval_ms: 1000,
                    continuous: false,
                    parts: 2,
                    part_names: vec!["face".into(), "left_foot".into()],
                    hit_curve: vec![(0.8, 1.0), (1.0, 0.5)],
                }),
                shapes: vec![
                    shape,
                    Shape {
                        kind: ShapeKind::Cylinder,
                        half: Vec3::new(1.5, 0.25, 1.5),
                        ..shape
                    },
                ],
            }],
            anomalies: vec![Anomaly {
                name: "core".into(),
                position: Vec3::ZERO,
                zones: vec!["logic/zone".into()],
                energy: AnomalyEnergy {
                    enabled: true,
                    initial: 1.0,
                    decrease_speed: 25.0,
                    on_walk: 35.0,
                    on_af_container_use: 100.0,
                    ..AnomalyEnergy::default()
                },
                artefacts: AnomalyArtefacts {
                    enabled: true,
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
                    active_time_s: 0,
                    energy_on_exit: 0.0,
                    select_priority: 0,
                    shoot_trigger: false,
                    zone_activity_trigger: false,
                    groups: vec![ZoneGroup {
                        enabled: true,
                        keep_all: false,
                        sequential: true,
                        min_active_s: 40,
                        max_active_s: 60,
                        max_charged: 255,
                        recharge_s: 15,
                        zones: vec!["logic/zone".into(), "logic/other".into()],
                    }],
                }],
            }],
            libraries: vec![Library {
                name: "models/a.model/walker_collision#[2][2][2]".into(),
                vertices: vec![Vec3::ZERO, Vec3::Y, Vec3::ONE],
                faces: vec![Face {
                    corners: [0, 1, 2],
                    surface: Surface {
                        walker: WalkerBehaviour::DontWalk,
                        armor: FaceArmor::Material(Armor::new(0.37).unwrap()),
                    },
                }],
                primitives: 1,
            }],
            artefact_cores: vec![ArtefactCore {
                core: "core".into(),
                pool: vec![(76, 5), (74, 5)],
                containers: vec![
                    ArtefactContainer {
                        name: "logic/container_1".into(),
                        search_s: 6.0,
                        cooldown_s: 80.0,
                    },
                    ArtefactContainer {
                        name: "logic/container_2".into(),
                        search_s: 2.0,
                        cooldown_s: 80.0,
                    },
                ],
            }],
            placements: vec![Placement {
                library: 0,
                position: Vec3::new(3.0, 4.0, 5.0),
                rotation: Vec3::new(0.1, 0.2, 0.3),
                cgroup: 10,
                cmask: 20,
            }],
        }
    }

    /// A version 2 file (written before the artefact cores) is still read, with none.
    #[test]
    fn a_version_2_file_is_read_without_the_artefact_cores() {
        let map = sample();
        let old = MapData::from_bytes(&map.to_bytes_of(2)).unwrap();
        assert!(old.artefact_cores.is_empty());
        let mut expected = MapData {
            artefact_cores: Vec::new(),
            ..map.clone()
        };
        expected.zones[0].damage.as_mut().unwrap().part_names.clear();
        expected.libraries[0].faces[0].surface.armor = FaceArmor::Unknown;
        assert_eq!(expected, old);
        assert!(MapData::from_bytes(&map.to_bytes_of(1)).is_err());
        assert!(MapData::from_bytes(&map.to_bytes_of(VERSION + 1)).is_err());
    }

    /// A version 3 file (written before the zones' part names) is still read, with none: the rest
    /// of the map is what the version 4 file says.
    #[test]
    fn a_version_3_file_is_read_without_the_part_names() {
        let map = sample();
        let old = MapData::from_bytes(&map.to_bytes_of(3)).unwrap();
        let named = &map.zones[0].damage.as_ref().unwrap().part_names;
        assert_eq!(named.len(), 2, "the sample names its parts");
        assert!(old.zones[0].damage.as_ref().unwrap().part_names.is_empty());
        assert_eq!(old.zones[0].damage.as_ref().unwrap().parts, 2);
        let mut unnamed = map.clone();
        unnamed.zones[0].damage.as_mut().unwrap().part_names.clear();
        unnamed.libraries[0].faces[0].surface.armor = FaceArmor::Unknown;
        assert_eq!(unnamed, old);
        // And the version 4 file keeps them.
        let new = MapData::from_bytes(&map.to_bytes()).unwrap();
        assert_eq!(&new.zones[0].damage.as_ref().unwrap().part_names, named);
    }

    /// A version 4 file (written before the bullet armor) is read with every face of unknown material; the version 5
    /// file keeps each triangle's armor.
    #[test]
    fn a_version_4_file_is_read_with_solid_faces() {
        let map = sample();
        let old = MapData::from_bytes(&map.to_bytes_of(4)).unwrap();
        assert_eq!(old.libraries[0].faces[0].surface.armor, FaceArmor::Unknown);
        assert_eq!(old.zones, map.zones, "the rest of the map is what the file says");
        let new = MapData::from_bytes(&map.to_bytes()).unwrap();
        assert_eq!(new.libraries[0].faces[0].surface.armor, FaceArmor::Material(Armor::new(0.37).unwrap()));
    }

    /// The header names the version without reading the map; a file that is not a map data file is an error.
    #[test]
    fn the_version_is_read_from_the_header_alone() {
        let dir = std::env::temp_dir().join(format!("srp-mapdata-version-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let map = sample();
        for version in [OLDEST, 4, VERSION] {
            let path = dir.join(format!("v{version}.mapdata"));
            std::fs::write(&path, map.to_bytes_of(version)).unwrap();
            assert_eq!(file_version(&path).unwrap(), version);
        }
        let short = dir.join("short.mapdata");
        std::fs::write(&short, b"SRPMAPD\0").unwrap();
        assert!(file_version(&short).is_err());
        let other = dir.join("other.mapdata");
        std::fs::write(&other, b"not a map data file at all").unwrap();
        assert!(file_version(&other).unwrap_err().to_string().contains("not a map data file"));
        assert!(file_version(&dir.join("missing.mapdata")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An armor that is no armor in a file is an error, not a face that anything goes through.
    #[test]
    fn a_bad_armor_is_an_error() {
        let good = sample().to_bytes();
        // The armor (0.37) is the last f32 before the placements: patch its bytes to a negative number.
        let at = good.windows(4).rposition(|w| w == 0.37f32.to_le_bytes()).expect("armor bytes");
        for bad_value in [-1.0f32, f32::INFINITY] {
            let mut bad = good.clone();
            bad[at..at + 4].copy_from_slice(&bad_value.to_le_bytes());
            let error = MapData::from_bytes(&bad).unwrap_err().to_string();
            assert!(error.contains("no armor"), "{error}");
        }
        // NaN is a face of unknown material, not an error.
        let mut unknown = good.clone();
        unknown[at..at + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        let read = MapData::from_bytes(&unknown).unwrap();
        assert_eq!(read.libraries[0].faces[0].surface.armor, FaceArmor::Unknown);
        assert!(MapData::from_bytes(&good).is_ok());
    }

    #[test]
    fn round_trips() {
        let map = sample();
        let bytes = map.to_bytes();
        assert_eq!(MapData::from_bytes(&bytes).unwrap(), map);
    }

    #[test]
    fn damaged_files_are_errors() {
        let bytes = sample().to_bytes();
        for len in 0..bytes.len() {
            assert!(MapData::from_bytes(&bytes[..len]).is_err());
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(MapData::from_bytes(&longer).is_err());
        // A placement naming library 7.
        let mut bad = sample();
        bad.placements[0].library = 7;
        assert!(MapData::from_bytes(&bad.to_bytes()).is_err());
        // A file of version 1 is refused with a word on what to do.
        let mut old = bytes.clone();
        old[8..12].copy_from_slice(&1u32.to_le_bytes());
        let error = MapData::from_bytes(&old).unwrap_err().0;
        assert!(
            error.contains("version 1") && error.contains("mapdata-dump"),
            "{error}"
        );
        // Every single flipped byte: an error or a file, never a panic.
        for at in 0..bytes.len() {
            let mut flipped = bytes.clone();
            flipped[at] ^= 0xFF;
            let _ = MapData::from_bytes(&flipped);
        }
    }

    #[test]
    fn names_files_by_project() {
        assert_eq!(file_name("level_01/day_4"), "level_01_day_4.mapdata");
    }
}
