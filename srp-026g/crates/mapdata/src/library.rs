//! Collision libraries: one `<model>.model/{collision,walker_collision}`
//! mesh in its own space, read the way the engine's
//! `collision_shape_cook.cpp` does.
//!
//! A library folder holds:
//! - `vertices`: chunk 0x19, `float3[]`;
//! - `indices`: chunk 0x1a, `u32[]` triangles;
//! - `face_data` (optional): chunk 0x1b = `u16 n` and `n` NUL-terminated
//!   shading group names, chunk 0x1c = one `u16` name index per triangle;
//! - `exported_primitives` (optional): a binary config
//!   `primitives[{type, position, rotation, scale, mtl}]` and `mtl_list`.
//!
//! The model's `settings` map a shading group to a game material
//! (`game_material_settings[group].game_material_id`), and
//! `game_materials/game.materials` gives the material's
//! `physic.walker_behaviour` and `physic.armor` (what a bullet meets,
//! [`crate::pierce`]). A library name may end in `#[sx][sy][sz]`: the
//! engine strips it and scales the mesh. A chunk file is a sequence of
//! `u32 id, u32 size, body`, little endian.

use std::collections::HashMap;

use binary_config_parser::ConfigValue;

use crate::bullet::{is_lone, mesh_scaling, Mesh, PrimitiveRow, Primitives, Shape};
use crate::config::{float3, float3_req};
use crate::math::{Rotation, Vec3};
use crate::pierce::{Armor, FaceArmor};
use crate::resources::{Error, Resources};

const CHUNK_VERTICES: u32 = 0x19;
const CHUNK_INDICES: u32 = 0x1a;
const CHUNK_FACE_DATA_HEADER: u32 = 0x1b;
const CHUNK_FACE_DATA: u32 = 0x1c;

/// How the walking body treats a face: `physic.walker_behaviour` of its game
/// material. In the client the cooked face keeps it in the top 2 bits of its
/// u16 (`get_walker_behaviour`, 0x356d00), and the ground test (0x84420)
/// reads it (PROVEN). The mapping from the cooker's material to the value
/// is LIKELY, not proven.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WalkerBehaviour {
    /// Ground where the slope is at most 60° (LIKELY 60°).
    #[default]
    BySlope = 0,
    /// Always ground (`walk` faces: stairs, steep paths).
    Walk = 1,
    /// Never ground: the body slides off (`dont_walk`).
    DontWalk = 2,
}

impl WalkerBehaviour {
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Walk,
            2 => Self::DontWalk,
            _ => Self::BySlope,
        }
    }
}

/// What a face is made of, as far as the bots need: how the walking body treats it and what a
/// bullet meets in it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surface {
    pub walker: WalkerBehaviour,
    pub armor: FaceArmor,
}

/// A triangle of a library: its corners (indices of the library's vertices) and what it is made of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    pub corners: [u32; 3],
    pub surface: Surface,
}

impl Face {
    /// A face of unknown material that the walking body treats as `walker` (a test's floor, a wall).
    pub fn unknown(corners: [u32; 3], walker: WalkerBehaviour) -> Self {
        Self { corners, surface: Surface { walker, armor: FaceArmor::Unknown } }
    }
}

/// `game_materials/game.materials`: game material id → walker behaviour and armor.
#[derive(Clone, Debug, Default)]
pub struct Materials {
    walker: HashMap<u16, WalkerBehaviour>,
    armor: HashMap<u16, Armor>,
}

impl Materials {
    pub fn load(resources: &mut Resources) -> Result<Self, Error> {
        Ok(Self::from_config(
            &resources.config("game_materials/game.materials")?,
        ))
    }

    pub fn from_config(config: &ConfigValue) -> Self {
        let mut walker = HashMap::new();
        let mut armor = HashMap::new();
        if let Some(materials) = config.get("materials") {
            for material in materials.children() {
                let Some(id) = material.get("id").and_then(ConfigValue::as_i32) else {
                    continue;
                };
                let physic = material.get("physic");
                let behaviour = physic
                    .and_then(|physic| physic.get("walker_behaviour"))
                    .and_then(ConfigValue::as_i32)
                    .unwrap_or(0);
                // A material without `armor`, or with one that is no armor, is left out: its
                // faces are [`FaceArmor::Unknown`] and stop every bullet, as the whole mesh did before.
                let resistance = physic
                    .and_then(|physic| physic.get("armor"))
                    .and_then(ConfigValue::as_f32)
                    .and_then(Armor::new);
                if let Ok(id) = u16::try_from(id) {
                    walker.insert(id, WalkerBehaviour::from_u8(behaviour as u8));
                    if let Some(resistance) = resistance {
                        armor.insert(id, resistance);
                    }
                }
            }
        }
        Self { walker, armor }
    }

    /// A material the table lacks (or no material) walks by slope.
    pub fn walker_behaviour(&self, material: Option<u16>) -> WalkerBehaviour {
        material
            .and_then(|id| self.walker.get(&id).copied())
            .unwrap_or_default()
    }

    /// A material the table lacks (or no material) is [`FaceArmor::Unknown`].
    pub fn armor(&self, material: Option<u16>) -> FaceArmor {
        material
            .and_then(|id| self.armor.get(&id).copied())
            .map_or(FaceArmor::Unknown, FaceArmor::Material)
    }

    /// Both of a face's material.
    pub fn surface(&self, material: Option<u16>) -> Surface {
        Surface {
            walker: self.walker_behaviour(material),
            armor: self.armor(material),
        }
    }
}

/// One library in its own space, its `#[sx][sy][sz]` scale applied.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    /// The `lib_name` as the project names it (with the scale suffix).
    pub name: String,
    pub vertices: Vec<Vec3>,
    /// The triangles with what each is made of: one list, so a corner list, its walker behaviour and its armor can
    /// never part.
    pub faces: Vec<Face>,
    /// Primitives (box, sphere, cylinder, capsule) that were turned into
    /// triangles; they are part of `faces`.
    pub primitives: u32,
}

impl Library {
    /// Loads `lib_name` (`resources/...#[sx][sy][sz]`). A library whose
    /// files are missing is empty, not an error: the engine skips it too.
    pub fn load(
        resources: &mut Resources,
        materials: &Materials,
        lib_name: &str,
    ) -> Result<(Self, Shape), Error> {
        let (path, scale) = split_scale(lib_name);
        let settings_path = match path.find(".model") {
            Some(at) => format!("{}/settings", &path[..at + ".model".len()]),
            None => format!("{path}/settings"),
        };
        let material_settings = if resources.exists(&settings_path) {
            resources
                .config(&settings_path)?
                .get("game_material_settings")
                .cloned()
        } else {
            None
        };
        let surface_of = |shading_group: &str| -> Surface {
            let material = material_settings
                .as_ref()
                .and_then(|m| m.get(shading_group))
                .and_then(|m| m.get("game_material_id"))
                .and_then(ConfigValue::as_i32)
                .and_then(|id| u16::try_from(id).ok());
            materials.surface(material)
        };

        let mut library = Self {
            name: lib_name.to_owned(),
            ..Self::default()
        };

        let mut mesh = None;
        let vertices_path = format!("{path}/vertices");
        if resources.exists(&vertices_path) {
            let vertices = resources.read(&vertices_path)?;
            let indices = resources.read(&format!("{path}/indices"))?;
            let face_data_path = format!("{path}/face_data");
            let face_data = if resources.exists(&face_data_path) {
                Some(resources.read(&face_data_path)?)
            } else {
                None
            };
            mesh = Some(library.add_mesh(
                &vertices,
                &indices,
                face_data.as_deref(),
                scale,
                &surface_of,
            )?);
        }

        let primitives_path = format!("{path}/exported_primitives");
        let primitives = if resources.exists(&primitives_path) {
            let config = resources.config(&primitives_path)?;
            library.add_primitives(&config, scale, &surface_of)?
        } else {
            Primitives::None
        };
        let shape = primitives
            .with_mesh(mesh)
            .map_err(|e| Error(format!("{lib_name}: {e}")))?;
        Ok((library, shape))
    }

    /// Adds the chunked mesh files' triangles; the game's mesh shape of them. A triangle that names a vertex the
    /// file does not have is an error: the game's mesh would read past its vertices.
    pub fn add_mesh(
        &mut self,
        vertices: &[u8],
        indices: &[u8],
        face_data: Option<&[u8]>,
        scale: Vec3,
        surface_of: &dyn Fn(&str) -> Surface,
    ) -> Result<Mesh, Error> {
        let vertices = chunk(vertices, CHUNK_VERTICES)
            .ok_or_else(|| Error(format!("{}: no vertex chunk", self.name)))?;
        let indices = chunk(indices, CHUNK_INDICES)
            .ok_or_else(|| Error(format!("{}: no index chunk", self.name)))?;
        let base = self.vertices.len() as u32;
        let count = (vertices.len() / 12) as u32;
        let word = |b: &[u8], at: usize| [b[at], b[at + 1], b[at + 2], b[at + 3]];
        self.vertices
            .extend(vertices.as_chunks::<12>().0.iter().map(|v| {
                let f = |at: usize| f32::from_le_bytes(word(v, at));
                Vec3::new(f(0), f(4), f(8)).scaled(scale)
            }));
        let faces = face_data
            .map(|bytes| face_surfaces(bytes, surface_of))
            .unwrap_or_default();
        let first = self.faces.len() as u32;
        for (n, tri) in indices.as_chunks::<12>().0.iter().enumerate() {
            let t = [0, 4, 8].map(|at| u32::from_le_bytes(word(tri, at)));
            if t.iter().any(|&i| i >= count) {
                return Err(Error(format!("{}: triangle {n} names no vertex", self.name)));
            }
            let surface = faces.get(n).copied().unwrap_or_default();
            self.faces.push(Face { corners: t.map(|i| i + base), surface });
        }
        Ok(Mesh { scaling: mesh_scaling(scale), faces: first..self.faces.len() as u32 })
    }

    /// Adds the `exported_primitives` as triangles; the game's shape of them ([`Primitives::build`]). A type the
    /// game does not make is an error.
    pub fn add_primitives(
        &mut self,
        config: &ConfigValue,
        scale: Vec3,
        surface_of: &dyn Fn(&str) -> Surface,
    ) -> Result<Primitives, Error> {
        let names: Vec<String> = config
            .get("mtl_list")
            .map(|list| {
                list.children()
                    .iter()
                    .map(|name| name.as_str().unwrap_or_default().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        let primitives = config.req("primitives")?.children();
        // A lone primitive at the origin is built without its rotation and position (the cook's 0x83600).
        let single = primitives.len() == 1
            && float3(primitives[0], "position").is_some_and(|p| is_lone(1, p));
        let mut rows = Vec::with_capacity(primitives.len());
        for primitive in primitives {
            let kind = primitive.i32_at("type")?;
            let dim = float3_req(primitive, "scale")?;
            let Some(shape) = PrimitiveShape::from_engine(kind, dim, scale) else {
                return Err(Error(format!("{}: primitive type {kind} is none the game makes", self.name)));
            };
            let (angles, at) = if single {
                (Vec3::ZERO, Vec3::ZERO)
            } else {
                (float3_req(primitive, "rotation")?, float3_req(primitive, "position")?)
            };
            let (rotation, position) = if single {
                (Rotation::IDENTITY, Vec3::ZERO)
            } else {
                (Rotation::from_angles(angles), at)
            };
            let shading_group = primitive
                .get("mtl")
                .and_then(ConfigValue::as_i32)
                .and_then(|i| names.get(i as usize))
                .map(String::as_str)
                .unwrap_or("");
            let surface = surface_of(shading_group);
            let (points, triangles) = shape.tessellate();
            // Bullet's scaling of a primitive (2.78, `create_compound_shape` 0x7A460 /
            // `create_primitive_shape` 0x7A390 call `setLocalScaling(scale)`): the child keeps its
            // rotation, its origin is multiplied by the scale and its own dims by the scale in its
            // own axes (in `from_engine`), so a rotated child is not sheared.
            let offset = position.scaled(scale);
            let base = self.vertices.len() as u32;
            self.vertices
                .extend(points.iter().map(|&p| rotation.apply(p) + offset));
            let first = self.faces.len() as u32;
            for t in triangles {
                self.faces.push(Face { corners: t.map(|i| i + base), surface });
            }
            rows.push(PrimitiveRow {
                kind,
                dim,
                position: at,
                rotation: angles,
                walker: surface.walker,
                faces: first..self.faces.len() as u32,
            });
            self.primitives += 1;
        }
        Primitives::build(rows, scale).map_err(|e| Error(format!("{}: {e}", self.name)))
    }
}

/// `lib_name` without `resources/` and the scale suffix, and that scale.
pub fn split_scale(lib_name: &str) -> (&str, Vec3) {
    let name = lib_name.strip_prefix("resources/").unwrap_or(lib_name);
    match name.find("#[") {
        Some(at) => (&name[..at], parse_scale(&name[at..])),
        None => (name, Vec3::ONE),
    }
}

fn parse_scale(suffix: &str) -> Vec3 {
    let values: Vec<f32> = suffix
        .split(['#', '[', ']'])
        .filter_map(|part| part.trim().parse().ok())
        .collect();
    match values[..] {
        [x, y, z, ..] => Vec3::new(x, y, z),
        _ => Vec3::ONE,
    }
}

/// The body of chunk `id` in a sequential chunk file (`u32 id, u32 size`).
pub fn chunk(bytes: &[u8], id: u32) -> Option<&[u8]> {
    let mut at = 0usize;
    while at.checked_add(8)? <= bytes.len() {
        let chunk_id = u32::from_le_bytes(bytes[at..at + 4].try_into().ok()?);
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        let body = bytes.get(at + 8..at.checked_add(8 + size)?)?;
        if chunk_id == id {
            return Some(body);
        }
        at += 8 + size;
    }
    None
}

/// Per-triangle surface from `face_data`.
fn face_surfaces(bytes: &[u8], surface_of: &dyn Fn(&str) -> Surface) -> Vec<Surface> {
    let (Some(header), Some(faces)) = (
        chunk(bytes, CHUNK_FACE_DATA_HEADER),
        chunk(bytes, CHUNK_FACE_DATA),
    ) else {
        return Vec::new();
    };
    if header.len() < 2 {
        return Vec::new();
    }
    let count = u16::from_le_bytes([header[0], header[1]]) as usize;
    let mut names = header[2..].split(|&b| b == 0);
    let remap: Vec<Surface> = (0..count)
        .map(|_| surface_of(&String::from_utf8_lossy(names.next().unwrap_or_default())))
        .collect();
    faces
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&f| {
            let index = u16::from_le_bytes(f) as usize;
            remap.get(index).copied().unwrap_or_default()
        })
        .collect()
}

/// An exported primitive's shape as Bullet casts against it (all along local y), the
/// library's scale applied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PrimitiveShape {
    Sphere {
        radius: f32,
    },
    Box {
        half: Vec3,
    },
    Cylinder {
        radius: f32,
        half_height: f32,
    },
    /// `btCapsuleShape(radius, height)`: `height` between the caps.
    Capsule {
        radius: f32,
        half_height: f32,
    },
}

/// Bullet's collision margin of a box, cylinder and capsule: `m_collisionMargin` (+30h, read by
/// `getMargin` 0x74190), 0.04 (0xA2D7DC, stored by the capsule's ctor 0xA5E20). PROVEN.
const BULLET_MARGIN: f32 = 0.04;

impl PrimitiveShape {
    /// `type` and `scale` (its dims) of an exported primitive under the library's `scale`, as
    /// 0.26g's Bullet (2.78) leaves the shape after `setLocalScaling(scale)` (`create_primitive_shape`
    /// 0x7A390 on a lone primitive; `create_compound_shape` 0x7A460 on the compound, which passes
    /// the scale to every child in the child's own axes). All PROVEN in the exe:
    /// - a box's, cylinder's and capsule's `setLocalScaling` (vtable slot 5, 0xA9BE0) keeps the
    ///   margin: each implicit dim becomes `(dim + margin) × scale - margin`. A box and a cylinder
    ///   hold their dims less the margin and their support adds it back, so their outer size is
    ///   `dim × scale`; a capsule holds its radius and half height as they are;
    /// - a cylinder's radius is its x half extent (`CylinderLocalSupportY` 0xA9F10): the z scale
    ///   is not used;
    /// - a capsule's radius is its implicit dim on axis (up + 2) % 3 = z (support 0xA5E90);
    /// - a sphere's `setLocalScaling` (0x8DEF0) only stores the scale; its radius is
    ///   `dim.x × scale.x`.
    pub fn from_engine(kind: i32, dim: Vec3, scale: Vec3) -> Option<Self> {
        // `btConvexInternalShape::setLocalScaling` takes the scale's absolute value.
        let scale = Vec3::new(scale.x.abs(), scale.y.abs(), scale.z.abs());
        let kept = |dim: f32, scale: f32| ((dim + BULLET_MARGIN) * scale - BULLET_MARGIN).max(0.0);
        match kind {
            0 => Some(Self::Sphere {
                radius: dim.x * scale.x,
            }),
            1 => Some(Self::Box {
                half: dim.scaled(scale),
            }),
            2 => Some(Self::Cylinder {
                radius: dim.x * scale.x,
                half_height: dim.y * scale.y,
            }),
            3 => Some(Self::Capsule {
                radius: kept(dim.x, scale.z),
                half_height: kept(dim.y * 0.5, scale.y),
            }),
            _ => None,
        }
    }

    /// Triangles that stand in for the shape, never inside it (the walk casts meet the game's
    /// exact shape, so a facet inside it would leave a gap the game does not have): exact for a
    /// box; a cylinder a prism drawn around its circle; a sphere and a capsule's caps a UV
    /// sphere drawn around the ball. A round shape gets as many sides as it needs for its
    /// facets to stand at most [`ROUND_OUT`] past the game's shape (12 to 96).
    pub fn tessellate(&self) -> (Vec<Vec3>, Vec<[u32; 3]>) {
        match *self {
            Self::Box { half } => {
                let points = (0..8)
                    .map(|c| {
                        Vec3::new(
                            if c & 1 != 0 { half.x } else { -half.x },
                            if c & 2 != 0 { half.y } else { -half.y },
                            if c & 4 != 0 { half.z } else { -half.z },
                        )
                    })
                    .collect();
                // Corners by bit: x 1, y 2, z 4.
                let quads = [
                    [0, 2, 3, 1], // -z
                    [4, 5, 7, 6], // +z
                    [0, 1, 5, 4], // -y
                    [2, 6, 7, 3], // +y
                    [0, 4, 6, 2], // -x
                    [1, 3, 7, 5], // +x
                ];
                let triangles = quads
                    .iter()
                    .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
                    .collect();
                (points, triangles)
            }
            Self::Cylinder {
                radius,
                half_height,
            } => {
                let sides = round_sides(radius, prism_inner);
                let outer = around(radius, prism_inner(sides));
                let mut points = Vec::new();
                for &y in &[-half_height, half_height] {
                    for s in 0..sides {
                        let a = s as f32 * std::f32::consts::TAU / sides as f32;
                        points.push(Vec3::new(outer * a.cos(), y, outer * a.sin()));
                    }
                }
                points.push(Vec3::new(0.0, -half_height, 0.0));
                points.push(Vec3::new(0.0, half_height, 0.0));
                let (bottom, top) = (2 * sides, 2 * sides + 1);
                let mut triangles = Vec::new();
                for s in 0..sides {
                    let n = (s + 1) % sides;
                    triangles.push([s, n, sides + n]);
                    triangles.push([s, sides + n, sides + s]);
                    triangles.push([bottom, n, s]);
                    triangles.push([top, sides + s, sides + n]);
                }
                (points, triangles)
            }
            Self::Sphere { radius } => rounded(radius, 0.0),
            Self::Capsule {
                radius,
                half_height,
            } => rounded(radius, half_height),
        }
    }
}

/// How far at most a round primitive's facets stand past the game's shape, m, while 96 sides
/// are enough (a sphere to about 9 m, a cylinder to about 18 m). An export precision, not a game value.
pub const ROUND_OUT: f32 = 0.01;
const SIDES_MIN: u32 = 12;
const SIDES_MAX: u32 = 96;
/// Sides go up by 4: a UV sphere has `sides / 2` rings, an even number, so the equator is a ring.
const SIDES_STEP: u32 = 4;

/// The fewest sides (12 to 96) for which a shape of `radius` drawn around its circle stands at
/// most [`ROUND_OUT`] out; `inner` is the distance from the centre to the nearest facet of the
/// unit shape with that many sides drawn through the circle.
fn round_sides(radius: f32, inner: fn(u32) -> f32) -> u32 {
    (SIDES_MIN..=SIDES_MAX)
        .step_by(SIDES_STEP as usize)
        .find(|&sides| radius * (1.0 / inner(sides) - 1.0) <= ROUND_OUT)
        .unwrap_or(SIDES_MAX)
}

/// The radius that puts the nearest facet at `radius` (and a hair more, for the float rounding
/// of the corners' sines and cosines).
fn around(radius: f32, inner: f32) -> f32 {
    radius / inner * (1.0 + 1e-5)
}

/// A regular polygon's inradius over its circumradius.
fn prism_inner(sides: u32) -> f32 {
    (std::f32::consts::PI / sides as f32).cos()
}

/// The nearest facet of a unit [`uv_sphere`] of `sides`, from its centre.
fn sphere_inner(sides: u32) -> f32 {
    static INNER: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    let table = INNER.get_or_init(|| {
        (SIDES_MIN..=SIDES_MAX)
            .step_by(SIDES_STEP as usize)
            .map(|sides| {
                let (points, triangles) = uv_sphere(sides, 1.0, 0.0);
                triangles
                    .iter()
                    .filter_map(|t| {
                        let [a, b, c] = t.map(|i| points[i as usize]);
                        let normal = (b - a).cross(c - a);
                        let area = normal.length();
                        // The sphere's equator is two rows at the same place: no facet there.
                        (area > 1e-9).then(|| a.dot(normal).abs() / area)
                    })
                    .fold(f32::INFINITY, f32::min)
            })
            .collect()
    });
    table[((sides - SIDES_MIN) / SIDES_STEP) as usize]
}

/// A sphere of `radius` drawn around (never inside), cut at its equator and pulled apart by
/// `2 × half_height` along y (a capsule; a sphere when 0).
fn rounded(radius: f32, half_height: f32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let sides = round_sides(radius, sphere_inner);
    uv_sphere(sides, around(radius, sphere_inner(sides)), half_height)
}

/// A UV sphere through `radius` (`sides` around, `sides / 2` rings), cut at its equator and
/// pulled apart by `2 × half_height` along y.
fn uv_sphere(sides: u32, radius: f32, half_height: f32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let rings = sides / 2; // even: the equator is a ring
    let mut points = vec![Vec3::new(0.0, -radius - half_height, 0.0)];
    for r in 1..rings {
        let polar = r as f32 * std::f32::consts::PI / rings as f32;
        let (y, ring) = (-polar.cos() * radius, polar.sin() * radius);
        let shift = if r * 2 < rings {
            -half_height
        } else if r * 2 > rings {
            half_height
        } else {
            0.0
        };
        let rows: &[f32] = if r * 2 == rings {
            &[-half_height, half_height]
        } else {
            &[shift]
        };
        for &dy in rows {
            for s in 0..sides {
                let a = s as f32 * std::f32::consts::TAU / sides as f32;
                points.push(Vec3::new(ring * a.cos(), y + dy, ring * a.sin()));
            }
        }
    }
    let top = points.len() as u32;
    points.push(Vec3::new(0.0, radius + half_height, 0.0));
    let rows = (top - 1) / sides;
    let mut triangles = Vec::new();
    for s in 0..sides {
        let n = (s + 1) % sides;
        triangles.push([0, 1 + n, 1 + s]);
        let last = 1 + (rows - 1) * sides;
        triangles.push([top, last + s, last + n]);
    }
    for r in 0..rows - 1 {
        let (a, b) = (1 + r * sides, 1 + (r + 1) * sides);
        for s in 0..sides {
            let n = (s + 1) % sides;
            triangles.push([a + s, a + n, b + n]);
            triangles.push([a + s, b + n, b + s]);
        }
    }
    (points, triangles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk_file(id: u32, body: &[u8]) -> Vec<u8> {
        let mut out = id.to_le_bytes().to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body);
        out
    }

    fn floats(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn reads_a_chunked_mesh_with_face_groups_and_scale() {
        let vertices = chunk_file(
            CHUNK_VERTICES,
            &floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]),
        );
        let indices = chunk_file(
            CHUNK_INDICES,
            &[0u32, 1, 2, 1, 3, 2]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>(),
        );
        let mut face_data = chunk_file(CHUNK_FACE_DATA_HEADER, b"\x02\x00walk\0dont_walk\0");
        face_data.extend(chunk_file(CHUNK_FACE_DATA, &[1, 0, 0, 0, 1, 0]));
        let mut library = Library {
            name: "test#[2][1][3]".into(),
            ..Library::default()
        };
        let (_, scale) = split_scale(&library.name);
        let surface = |group: &str| match group {
            "walk" => Surface {
                walker: WalkerBehaviour::Walk,
                armor: FaceArmor::Material(Armor::new(0.1).unwrap()),
            },
            "dont_walk" => Surface {
                walker: WalkerBehaviour::DontWalk,
                armor: FaceArmor::FREE,
            },
            _ => Surface::default(),
        };
        let mesh = library
            .add_mesh(&vertices, &indices, Some(&face_data), scale, &surface)
            .unwrap();
        assert_eq!(mesh, Mesh { scaling: [2.0, 1.0, -3.0], faces: 0..2 });
        let corners: Vec<[u32; 3]> = library.faces.iter().map(|f| f.corners).collect();
        assert_eq!(corners, [[0, 1, 2], [1, 3, 2]]);
        let walker: Vec<WalkerBehaviour> = library.faces.iter().map(|f| f.surface.walker).collect();
        assert_eq!(walker, [WalkerBehaviour::DontWalk, WalkerBehaviour::Walk]);
        let armor: Vec<FaceArmor> = library.faces.iter().map(|f| f.surface.armor).collect();
        assert_eq!(armor, [FaceArmor::FREE, FaceArmor::Material(Armor::new(0.1).unwrap())]);
        assert_eq!(library.vertices[3], Vec3::new(2.0, 1.0, 3.0));
        // A triangle that names a vertex the file does not have is an error (the game would read past them).
        let bad = chunk_file(
            CHUNK_INDICES,
            &[0u32, 1, 2, 0, 1, 9].iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>(),
        );
        let error = Library::default().add_mesh(&vertices, &bad, None, scale, &surface).unwrap_err();
        assert!(error.to_string().contains("triangle 1 names no vertex"), "{error}");
    }

    #[test]
    fn splits_the_scale_suffix() {
        let (path, scale) = split_scale("resources/models/a.model/collision#[1.5][2][0.5]");
        assert_eq!(path, "models/a.model/collision");
        assert_eq!(scale, Vec3::new(1.5, 2.0, 0.5));
        assert_eq!(split_scale("models/b").1, Vec3::ONE);
    }

    #[test]
    fn truncated_chunks_are_not_found() {
        let file = chunk_file(7, &[1, 2, 3]);
        assert_eq!(chunk(&file, 7), Some(&[1u8, 2, 3][..]));
        assert_eq!(chunk(&file[..file.len() - 1], 7), None);
        assert_eq!(chunk(&file, 8), None);
    }

    #[test]
    fn primitives_are_closed_and_sized() {
        for shape in [
            PrimitiveShape::Box {
                half: Vec3::new(1.0, 2.0, 3.0),
            },
            PrimitiveShape::Cylinder {
                radius: 0.5,
                half_height: 2.0,
            },
            PrimitiveShape::Sphere { radius: 1.5 },
            PrimitiveShape::Capsule {
                radius: 0.4,
                half_height: 1.0,
            },
        ] {
            let (points, triangles) = shape.tessellate();
            // Closed: every edge is used once each way.
            let mut edges = std::collections::HashMap::new();
            for t in &triangles {
                for k in 0..3 {
                    *edges.entry((t[k], t[(k + 1) % 3])).or_insert(0) += 1;
                }
            }
            for (&(a, b), &n) in &edges {
                assert_eq!(n, 1, "{shape:?}");
                assert_eq!(edges.get(&(b, a)), Some(&1), "{shape:?}");
            }
            // Outward: the signed volume is positive and near the shape's.
            let volume: f32 = triangles
                .iter()
                .map(|t| {
                    let [a, b, c] = t.map(|i| points[i as usize]);
                    a.dot(b.cross(c)) / 6.0
                })
                .sum();
            let expected = match shape {
                PrimitiveShape::Box { half } => 8.0 * half.x * half.y * half.z,
                PrimitiveShape::Cylinder {
                    radius,
                    half_height,
                } => std::f32::consts::PI * radius * radius * 2.0 * half_height,
                PrimitiveShape::Sphere { radius } => {
                    4.0 / 3.0 * std::f32::consts::PI * radius.powi(3)
                }
                PrimitiveShape::Capsule {
                    radius,
                    half_height,
                } => {
                    std::f32::consts::PI
                        * radius
                        * radius
                        * (2.0 * half_height + 4.0 / 3.0 * radius)
                }
            };
            // Clockwise or not, the facets stay within 15 % of the volume.
            assert!(
                (volume.abs() / expected - 1.0).abs() < 0.15,
                "{shape:?}: {volume} vs {expected}"
            );
        }
    }

    #[test]
    fn primitive_config_uses_its_material_and_placement() {
        let json = serde_json::json!({
            "mtl_list": ["col_0", "walk"],
            "primitives": [
                { "type": 1, "position": [0.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0], "mtl": 1 },
                { "type": 1, "position": [5.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0], "mtl": 0 },
            ],
        });
        let config = binary_config_parser::BinaryConfig::parse_json(&json);
        let config = ConfigValue::parse(config.as_bytes()).unwrap();
        let mut library = Library::default();
        let surface = |group: &str| Surface {
            walker: if group == "walk" {
                WalkerBehaviour::Walk
            } else {
                WalkerBehaviour::BySlope
            },
            armor: if group == "walk" { FaceArmor::FREE } else { FaceArmor::Unknown },
        };
        library
            .add_primitives(&config, Vec3::new(2.0, 1.0, 1.0), &surface)
            .unwrap();
        assert_eq!(library.primitives, 2);
        assert_eq!(library.faces.len(), 24);
        assert_eq!(library.faces[0].surface.walker, WalkerBehaviour::Walk);
        assert_eq!(library.faces[12].surface.walker, WalkerBehaviour::BySlope);
        assert_eq!(library.faces[0].surface.armor, FaceArmor::FREE);
        assert_eq!(library.faces[12].surface.armor, FaceArmor::Unknown);
        let max_x = library
            .vertices
            .iter()
            .map(|v| v.x)
            .fold(f32::MIN, f32::max);
        assert_eq!(max_x, 12.0); // (5 + 1) × 2
    }

    fn bounds(points: &[Vec3]) -> (Vec3, Vec3) {
        points.iter().fold(
            (Vec3::new(f32::MAX, f32::MAX, f32::MAX), Vec3::new(f32::MIN, f32::MIN, f32::MIN)),
            |(lo, hi), &p| (lo.min(p), hi.max(p)),
        )
    }

    #[test]
    fn a_compound_child_is_scaled_in_its_own_axes() {
        // A box of half (1, 2, 1) turned a quarter about z (its x along -y, its y along x), at
        // (0, 3, 0), under the library scale (2, 1, 1): Bullet scales its dims in its own axes,
        // (2, 2, 1), and its origin, (0, 3, 0); scaling the turned points would make it 4 × 1.
        let json = serde_json::json!({
            "primitives": [
                { "type": 1, "position": [0.0, 3.0, 0.0], "rotation": [0.0, 0.0, std::f32::consts::FRAC_PI_2], "scale": [1.0, 2.0, 1.0] },
                { "type": 1, "position": [9.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0] },
            ],
        });
        let config = binary_config_parser::BinaryConfig::parse_json(&json);
        let config = ConfigValue::parse(config.as_bytes()).unwrap();
        let mut library = Library::default();
        library
            .add_primitives(&config, Vec3::new(2.0, 1.0, 1.0), &|_| Surface::default())
            .unwrap();
        let (lo, hi) = bounds(&library.vertices[..8]);
        let close = |a: Vec3, b: Vec3| (a - b).length() < 1e-5;
        assert!(close(lo, Vec3::new(-2.0, 1.0, -1.0)), "{lo:?}");
        assert!(close(hi, Vec3::new(2.0, 5.0, 1.0)), "{hi:?}");
        let (lo, hi) = bounds(&library.vertices[8..]);
        assert!(close(lo, Vec3::new(16.0, -1.0, -1.0)), "{lo:?}");
        assert!(close(hi, Vec3::new(20.0, 1.0, 1.0)), "{hi:?}");
    }

    #[test]
    fn scaled_round_primitives_follow_bullet() {
        let scale = Vec3::new(2.0, 3.0, 0.5);
        assert_eq!(
            PrimitiveShape::from_engine(0, Vec3::new(1.5, 9.0, 9.0), scale),
            Some(PrimitiveShape::Sphere { radius: 3.0 })
        );
        // A cylinder's radius by its x scale only.
        assert_eq!(
            PrimitiveShape::from_engine(2, Vec3::new(0.5, 1.0, 0.5), scale),
            Some(PrimitiveShape::Cylinder {
                radius: 1.0,
                half_height: 3.0
            })
        );
        // A capsule keeps its margin: (0.4 + 0.04) × 0.5 - 0.04 and (1 + 0.04) × 3 - 0.04.
        let Some(PrimitiveShape::Capsule {
            radius,
            half_height,
        }) = PrimitiveShape::from_engine(3, Vec3::new(0.4, 2.0, 0.4), scale)
        else {
            panic!()
        };
        assert!((radius - 0.18).abs() < 1e-6 && (half_height - 3.08).abs() < 1e-5);
        assert_eq!(
            PrimitiveShape::from_engine(1, Vec3::new(1.0, 1.0, 1.0), -scale),
            Some(PrimitiveShape::Box { half: scale })
        );
    }

    #[test]
    fn round_primitives_are_never_inside_the_game_shape() {
        // Directions spread over the sphere.
        let count = 4000;
        let directions: Vec<Vec3> = (0..count)
            .map(|i| {
                let y = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
                let ring = (1.0 - y * y).sqrt();
                let a = i as f32 * 2.399_963;
                Vec3::new(ring * a.cos(), y, ring * a.sin())
            })
            .collect();
        for radius in [0.05, 0.3, 1.0, 2.5, 5.0, 8.0] {
            for shape in [
                PrimitiveShape::Sphere { radius },
                PrimitiveShape::Cylinder {
                    radius,
                    half_height: 1.5,
                },
                PrimitiveShape::Capsule {
                    radius,
                    half_height: 0.7,
                },
            ] {
                let (points, _) = shape.tessellate();
                for &d in &directions {
                    let across = (d.x * d.x + d.z * d.z).sqrt();
                    let game = match shape {
                        PrimitiveShape::Sphere { radius } => radius,
                        PrimitiveShape::Cylinder {
                            radius,
                            half_height,
                        } => half_height * d.y.abs() + radius * across,
                        PrimitiveShape::Capsule {
                            radius,
                            half_height,
                        } => half_height * d.y.abs() + radius,
                        PrimitiveShape::Box { .. } => unreachable!(),
                    };
                    let export = points.iter().map(|p| p.dot(d)).fold(f32::MIN, f32::max);
                    assert!(export >= game - 1e-6, "{shape:?} inside along {d:?}: {export} < {game}");
                    assert!(
                        export <= game + ROUND_OUT + 1e-4,
                        "{shape:?} out along {d:?}: {export} > {game}"
                    );
                }
            }
        }
    }
}
