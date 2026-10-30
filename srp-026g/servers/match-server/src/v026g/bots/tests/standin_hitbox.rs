//! The stand-in's hit volume: what a shot is tested against.
//!
//! **Before** ([`HitVolume::Disc`]): a shot hit the nearest body whose chest point the line passed within
//! 0.45 m, a disc of 0.64 m2 about one point with no head, arms, legs or crouch benefit
//! (`fact-standin-calibration.md` C1). **Now** ([`HitVolume::Boxes`], with `SRP_BENCH_HIT_MODEL=on`): the shot is a ray
//! tested against the 19 shapes of the game's player hit model, the nearest hit along the ray wins, and the
//! shape that was hit says which of the body's hit boxes it was ([`HitParam`]).
//!
//! What the game does (`fact-hitboxes-026g.md`, from the 0.26g exe and `resources.db`):
//! - a bullet is a ray without thickness: `bullet::check_collision` 0x32D4E0 calls
//!   `bullet_physics_world::ray_test` 0x75E20, a Bullet closest-hit ray callback, and each player is a compound of
//!   the shapes of `models/character/human/base/base_character_full.skinned_model/hit_targets`, each tied to a
//!   bone and posed by the animation: 1 sphere (the head), boxes (the torso's front and back pairs, the face,
//!   the hands), 1 cylinder (the neck), capsules (thighs, shins, feet, arms, forearms). PROVEN (exe and data);
//! - the shapes' sizes (`create_bt_primitive` 0x7A1F0: sphere radius, box half extents, cylinder radius and half
//!   height, capsule radius and the cylinder length between the hemisphere centres). PROVEN;
//! - which hit box a shape is (`hit_param`, a name of `human_hit_params`, whose pools and coefficients
//!   [`super::standin_damage`] already applies): LIKELY, the field the 0.100b server reads, and the shares fit
//!   the live hit shares;
//! - the pose the shapes are placed in. **LIKELY**: [`TABLE`] is the shapes placed by the `on_site_idle` clips
//!   of the rifles' list, standing and crouched, a Python port of the 0.100b server's pose code; it was checked
//!   by bone lengths and a picture, not against a live capture. The move clips give the same hit rate within
//!   0.01 (`fact-hitboxes-026g.md` 5.6), so one static pose per stance stands for a body in motion.
//!
//! The shapes are in the model's frame: the feet at y = 0 on the player's position, x to the player's right, +z
//! the way it faces, as the map frame is (left-handed, Y up). The idle stance is bladed (a foot forward, the
//! torso turned), so the body is not centred on the position: the torso lies about 0.23 m behind it.
//!
//! **C3, the position scatter** ([`POSITION_SIGMA_M`]): the bots' shot lines are off the bodies they aim at by
//! more than their aim says, because a bot reckons where the bodies are from worlds up to a second old and
//! the game's bodies move by the client's own rules (`fact-own-eye.md` 5: the live bullet's origin against the
//! bots' own replay is off by a median 0.17-0.31 m, at every distance). The stand-in's bodies move by the
//! bots' own model, so it has none of it; with the boxes it is added to the shot line as one random offset
//! across the line. GUESS: the size (a median 0.20 m, the short ages of the data) and that it adds in
//! quadrature to the aim error the bench already has (`fact-hitboxes-026g.md` 5.4); the C0 counts check the
//! residual (`shot_off_cells`, `hit_cells`).
//!
//! One setting, `SRP_BENCH_HIT_MODEL`, turns the whole cut (C1 boxes, C2 pellets, C3 scatter) on together with
//! `on`. **Unset it is `off`**: the old disc, the centre ray and no scatter, exactly, so every bench and identity
//! run stays what it was until the setting is set (the phase-end smoke sets it in both arms; the bench becomes
//! `on` after the release with a new base log set). `boxes` (C1 and C2 alone) and `scatter=<m>` (another sigma)
//! are for diagnosis ([`HitVolume::bench`]).

use mapdata::Vec3;

use super::bench_settings;
use super::standin_damage::Part;

/// What a shot is tested against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum HitVolume {
    /// The old rule: within [`DISC_M`] of the target's chest point, the part drawn by chance; a shotgun's shot
    /// is its centre ray; no scatter.
    Disc,
    /// The body's 19 hit shapes: the nearest on the ray, the part from the shape; a shotgun's shot is its
    /// pellets, each a ray. `scatter_m`: the sigma, metres, of the position scatter of the shot line (C3), if
    /// there is one.
    Boxes { scatter_m: Option<f32> },
}

/// The sigma, metres per axis across the line, of the position scatter (C3): an isotropic Gaussian of this
/// sigma has the radial median 0.20 m, the live bullet-origin error against the bots' replay at the short ages
/// (0.17 m at 0.4-0.8 s, 0.20 m at 0.8-1.2 s, 0.31 m at 1.8-2.5 s; `fact-own-eye.md` 5). GUESS: the age of a
/// shot's information is not tracked, so it is the one value for all.
pub(crate) const POSITION_SIGMA_M: f32 = 0.17;

/// The bullet manager's gravity, m/s^2: `g = (0, -9.81, 0)` (its constructor 0x3219A0: PROVEN, fx-cover's read in
/// `player_bots_weapons.md`).
pub(crate) const BULLET_G: f32 = 9.81;

/// How a shot reaches its target in the bench's shape hit model (`SRP_BENCH_HIT_FLIGHT`, A182).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum BulletPath {
    /// A ray at where the bodies stood at the shot (the hit model before the flight).
    Instant,
    /// The client's path, muzzle speed, drag and gravity, each body found where its last step carries it by the time.
    #[default]
    Flown,
    /// The client's path against the bodies where they stood at the shot: the flight's drop and time with no target motion
    /// (a diagnosis: what part of the flight's loss is the bodies moving).
    FlownBodiesStill,
}

impl BulletPath {
    /// From the setting: unset or `on` is [`Self::Flown`], `0` or `off` the instant ray, `still` the diagnosis.
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("on" | "1") => Ok(Self::Flown),
            Some("0" | "off") => Ok(Self::Instant),
            Some("still") => Ok(Self::FlownBodiesStill),
            Some(other) => Err(format!("SRP_BENCH_HIT_FLIGHT={other:?}: on, 0 or still")),
        }
    }

    pub(crate) fn bench() -> Self {
        Self::parse(bench_settings().var("SRP_BENCH_HIT_FLIGHT").ok().as_deref()).unwrap_or_else(|e| panic!("{e}"))
    }
}

/// Where the client's bullet is `t` s after the shot (`bullet::tick` 0x32C180: `p(t) = p0 + v0 (t - k t^2 / 2) + g t^2 / 2`,
/// `v0` the shot direction times the ammunition's muzzle speed, `k` its `air_resistance`: PROVEN).
pub(crate) fn bullet_at(origin: Vec3, dir: Vec3, ammo: &survarium::v026g::damage::AmmoParams, t: f32) -> Vec3 {
    let along = ammo.muzzle_speed * (t - ammo.air_resistance * t * t / 2.0);
    origin + dir * along + Vec3::new(0.0, -BULLET_G * t * t / 2.0, 0.0)
}

/// Where the flight ends, s after the shot: `t = 1 / (1.1 k)` (0x32C523..0x32C55B: PROVEN); no end without drag.
pub(crate) fn flight_end_s(ammo: &survarium::v026g::damage::AmmoParams) -> f32 {
    if ammo.air_resistance <= 0.0 { f32::MAX } else { 1.0 / (1.1 * ammo.air_resistance) }
}

/// The old rule's radius, metres (the same number the shot offsets count against).
pub(crate) use super::tallies::shot_offset::DISC_M;

impl HitVolume {
    /// The boxes and pellets without the scatter (the tests' volume, and `SRP_BENCH_HIT_MODEL=boxes`).
    pub(crate) const BOXES: Self = Self::Boxes { scatter_m: None };

    /// The whole cut: boxes, pellets and the scatter (`SRP_BENCH_HIT_MODEL=on`).
    pub(crate) const ON: Self = Self::Boxes {
        scatter_m: Some(POSITION_SIGMA_M),
    };

    /// The bench's: the old disc unless `SRP_BENCH_HIT_MODEL` says otherwise ([`Self::parse`]).
    pub(crate) fn bench() -> Self {
        Self::parse(bench_settings().var("SRP_BENCH_HIT_MODEL").ok().as_deref())
    }

    /// `on`: boxes, pellets and the scatter; `None` or `off`: the old disc; `boxes`: no scatter; `scatter=<m>`:
    /// the scatter's sigma. Anything else refuses the run.
    pub(crate) fn parse(value: Option<&str>) -> Self {
        match value {
            Some("on") => Self::ON,
            None | Some("off") => Self::Disc,
            Some("boxes") => Self::BOXES,
            Some(other) => {
                let sigma = other.strip_prefix("scatter=").and_then(|m| m.parse::<f32>().ok());
                match sigma.filter(|m| m.is_finite() && *m >= 0.0) {
                    Some(m) => Self::Boxes { scatter_m: Some(m) },
                    None => panic!("SRP_BENCH_HIT_MODEL={other}: on, off, boxes or scatter=<metres>"),
                }
            }
        }
    }
}

/// The hit box a shape is (`hit_param` of `human_hit_params`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HitParam {
    Brain,
    Face,
    Front,
    Back,
    RightHip,
    LeftHip,
    RightFoot,
    LeftFoot,
    RightForearm,
    LeftForearm,
    RightArm,
    LeftArm,
}

impl HitParam {
    pub(crate) fn of(name: &str) -> Option<Self> {
        Some(match name {
            "brain" => Self::Brain,
            "face" => Self::Face,
            "front" => Self::Front,
            "back" => Self::Back,
            "right_hip" => Self::RightHip,
            "left_hip" => Self::LeftHip,
            "right_foot" => Self::RightFoot,
            "left_foot" => Self::LeftFoot,
            "right_forearm" => Self::RightForearm,
            "left_forearm" => Self::LeftForearm,
            "right_arm" => Self::RightArm,
            "left_arm" => Self::LeftArm,
            _ => return None,
        })
    }

    /// The name `human_hit_params` has it under.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Brain => "brain",
            Self::Face => "face",
            Self::Front => "front",
            Self::Back => "back",
            Self::RightHip => "right_hip",
            Self::LeftHip => "left_hip",
            Self::RightFoot => "right_foot",
            Self::LeftFoot => "left_foot",
            Self::RightForearm => "right_forearm",
            Self::LeftForearm => "left_forearm",
            Self::RightArm => "right_arm",
            Self::LeftArm => "left_arm",
        }
    }

    /// The part of the body it is, as the damage model and the live tables group them.
    pub(crate) fn part(self) -> Part {
        match self {
            Self::Brain | Self::Face => Part::Head,
            Self::Front | Self::Back => Part::Torso,
            Self::RightForearm | Self::LeftForearm | Self::RightArm | Self::LeftArm => Part::Arm,
            Self::RightHip | Self::LeftHip | Self::RightFoot | Self::LeftFoot => Part::Leg,
        }
    }

    /// The stand-in's leg a leg shape is (`Body::legs`), if it is one.
    pub(crate) fn leg(self) -> Option<usize> {
        match self {
            Self::RightHip | Self::RightFoot => Some(0),
            Self::LeftHip | Self::LeftFoot => Some(1),
            _ => None,
        }
    }
}

/// A shape's kind and size, in its own frame (y its axis).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Primitive {
    Sphere { radius: f32 },
    /// Half extents along the shape's x, y, z.
    Box { half: Vec3 },
    /// The radius and the half height.
    Cylinder { radius: f32, half_height: f32 },
    /// The radius and the length of the cylinder between the hemisphere centres.
    Capsule { radius: f32, length: f32 },
}

impl Primitive {
    /// The radius of a sphere about the shape's centre that holds it.
    fn bound(self) -> f32 {
        match self {
            Self::Sphere { radius } => radius,
            Self::Box { half } => half.length(),
            Self::Cylinder { radius, half_height } => radius.hypot(half_height),
            Self::Capsule { radius, length } => radius + length / 2.0,
        }
    }
}

/// One hit shape placed in the model frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shape {
    pub(crate) param: HitParam,
    pub(crate) primitive: Primitive,
    pub(crate) centre: Vec3,
    /// The shape's x, y, z axes in the model frame (unit, at right angles).
    pub(crate) axes: [Vec3; 3],
}

/// A body's shapes in one stance, and a sphere that holds them all.
#[derive(Clone, Debug)]
pub(crate) struct Pose {
    pub(crate) shapes: Vec<Shape>,
    bound_centre: Vec3,
    bound_radius: f32,
}

impl Pose {
    fn new(shapes: Vec<Shape>) -> Self {
        let n = shapes.len() as f32;
        let sum = shapes.iter().fold(Vec3::ZERO, |sum, s| sum + s.centre);
        let bound_centre = sum * (1.0 / n);
        let bound_radius = shapes
            .iter()
            .map(|s| (s.centre - bound_centre).length() + s.primitive.bound())
            .fold(0.0, f32::max);
        Self {
            shapes,
            bound_centre,
            bound_radius,
        }
    }

    /// The nearest hit of the ray from `origin` along `dir` (unit), both in the model frame, within `reach`:
    /// its distance and the hit box.
    pub(crate) fn ray(&self, origin: Vec3, dir: Vec3, reach: f32) -> Option<(f32, HitParam)> {
        // The sphere that holds the body first: most rays pass a body by.
        let to = self.bound_centre - origin;
        let along = to.dot(dir);
        if along < -self.bound_radius || along > reach + self.bound_radius {
            return None;
        }
        if (to - dir * along).length() > self.bound_radius {
            return None;
        }
        self.shapes
            .iter()
            .filter_map(|shape| Some((shape.ray(origin, dir)?, shape.param)))
            .filter(|(t, _)| *t <= reach)
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }
}

impl Shape {
    /// The distance along the ray (`dir` unit) to where it enters the shape (0 if it starts inside), both in
    /// the model frame.
    fn ray(&self, origin: Vec3, dir: Vec3) -> Option<f32> {
        let [ax, ay, az] = self.axes;
        let rel = origin - self.centre;
        let o = Vec3::new(rel.dot(ax), rel.dot(ay), rel.dot(az));
        let d = Vec3::new(dir.dot(ax), dir.dot(ay), dir.dot(az));
        match self.primitive {
            Primitive::Sphere { radius } => ray_sphere(o, d, radius),
            Primitive::Box { half } => ray_box(o, d, half),
            Primitive::Cylinder { radius, half_height } => ray_cylinder(o, d, radius, half_height),
            Primitive::Capsule { radius, length } => ray_capsule(o, d, radius, length / 2.0),
        }
    }
}

/// A ray (`d` unit) against the ball of `radius` about the origin.
fn ray_sphere(o: Vec3, d: Vec3, radius: f32) -> Option<f32> {
    let b = o.dot(d);
    let c = o.dot(o) - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let root = disc.sqrt();
    let enter = -b - root;
    if enter >= 0.0 {
        Some(enter)
    } else if -b + root >= 0.0 {
        Some(0.0)
    } else {
        None
    }
}

/// A ray against the box of `half` extents about the origin, the slabs method.
fn ray_box(o: Vec3, d: Vec3, half: Vec3) -> Option<f32> {
    let (mut near, mut far) = (f32::NEG_INFINITY, f32::INFINITY);
    for axis in 0..3 {
        let (o, d, h) = (o.axis(axis), d.axis(axis), half.axis(axis));
        if d.abs() < 1e-9 {
            if o.abs() > h {
                return None;
            }
            continue;
        }
        let (a, b) = ((-h - o) / d, (h - o) / d);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    (near <= far && far >= 0.0).then(|| near.max(0.0))
}

/// A ray against the finite cylinder about the y axis, flat caps (the neck).
fn ray_cylinder(o: Vec3, d: Vec3, radius: f32, half_height: f32) -> Option<f32> {
    if o.y.abs() <= half_height && o.x * o.x + o.z * o.z <= radius * radius {
        return Some(0.0);
    }
    let mut best: Option<f32> = None;
    let mut take = |t: f32| best = Some(best.map_or(t, |b: f32| b.min(t)));
    let a = d.x * d.x + d.z * d.z;
    if a > 1e-12 {
        let b = o.x * d.x + o.z * d.z;
        let c = o.x * o.x + o.z * o.z - radius * radius;
        let disc = b * b - a * c;
        if disc >= 0.0 {
            let t = (-b - disc.sqrt()) / a;
            if t >= 0.0 && (o.y + t * d.y).abs() <= half_height {
                take(t);
            }
        }
    }
    if d.y.abs() > 1e-9 {
        for cap in [-half_height, half_height] {
            let t = (cap - o.y) / d.y;
            let (x, z) = (o.x + t * d.x, o.z + t * d.z);
            if t >= 0.0 && x * x + z * z <= radius * radius {
                take(t);
            }
        }
    }
    best
}

/// A ray against the capsule about the y axis: the cylinder of `radius` between y = -`half` and `half`, a
/// hemisphere on each end.
fn ray_capsule(o: Vec3, d: Vec3, radius: f32, half: f32) -> Option<f32> {
    let along = o.y.clamp(-half, half);
    let (x, y, z) = (o.x, o.y - along, o.z);
    if x * x + y * y + z * z <= radius * radius {
        return Some(0.0);
    }
    let mut best: Option<f32> = None;
    let mut take = |t: f32| best = Some(best.map_or(t, |b: f32| b.min(t)));
    let a = d.x * d.x + d.z * d.z;
    if a > 1e-12 {
        let b = o.x * d.x + o.z * d.z;
        let c = o.x * o.x + o.z * o.z - radius * radius;
        let disc = b * b - a * c;
        if disc >= 0.0 {
            let t = (-b - disc.sqrt()) / a;
            if t >= 0.0 && (o.y + t * d.y).abs() <= half {
                take(t);
            }
        }
    }
    for end in [-half, half] {
        if let Some(t) = ray_sphere(o - Vec3::new(0.0, end, 0.0), d, radius) {
            take(t);
        }
    }
    best
}

/// The shapes of the 0.26g player, standing and crouched, placed by the idle clips: a copy of
/// `fighting-record/scripts/hitbox/hitboxes-026g-idle.tsv` (tab-separated, a header and comment lines first).
pub(crate) const TABLE: &str = include_str!("hitboxes-026g-idle.tsv");

/// Whether the forearm and hand shapes are in the pose the shots are tested against (`SRP_BENCH_HIT_ARMS`, a diagnosis of
/// A182: the bench's hits land on the arms twice as often as the live shares, 52 % against 23 %, because the idle pose
/// holds the forearms across the chest).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Arms {
    /// The table's pose, forearms and hands in it.
    Posed,
    /// The arm shapes taken out of every pose: a line through the arms' place meets what is behind them.
    Away,
}

impl Arms {
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("posed") => Ok(Self::Posed),
            Some("away") => Ok(Self::Away),
            Some(other) => Err(format!("SRP_BENCH_HIT_ARMS={other:?}: posed or away")),
        }
    }

    fn bench() -> Self {
        Self::parse(bench_settings().var("SRP_BENCH_HIT_ARMS").ok().as_deref()).unwrap_or_else(|e| panic!("{e}"))
    }
}

/// The poses of [`TABLE`], by `[crouched][aimed]`.
pub(crate) struct Poses([[Pose; 2]; 2]);

impl Poses {
    /// Parse `table` once; a row that is not what the table says is a panic here, never in a shot. `arms`: whether the
    /// forearm and hand shapes are in the pose ([`Arms`]).
    fn read(table: &str, arms: Arms) -> Self {
        let mut sets: [[Vec<Shape>; 2]; 2] = Default::default();
        for line in table.lines().filter(|l| !l.starts_with('#') && !l.starts_with("stance")) {
            let cells: Vec<&str> = line.split('\t').collect();
            assert_eq!(cells.len(), 20, "a hit box row has 20 columns: {line}");
            let crouched = match cells[0] {
                "stand" => 0,
                "crouch" => 1,
                other => panic!("stance {other}"),
            };
            let aimed = match cells[1] {
                "0" => 0,
                "1" => 1,
                other => panic!("aimed {other}"),
            };
            let param = HitParam::of(cells[2]).unwrap_or_else(|| panic!("hit_param {}", cells[2]));
            if arms == Arms::Away && param.part() == Part::Arm {
                continue;
            }
            let n: Vec<f32> = cells[5..]
                .iter()
                .map(|c| c.parse().unwrap_or_else(|_| panic!("a number: {c}")))
                .collect();
            let v = |i: usize| Vec3::new(n[i], n[i + 1], n[i + 2]);
            let (dim0, dim1, dim2) = (n[12], n[13], n[14]);
            let primitive = match cells[4] {
                "0" => Primitive::Sphere { radius: dim0 },
                "1" => Primitive::Box {
                    half: Vec3::new(dim0, dim1, dim2),
                },
                "2" => Primitive::Cylinder {
                    radius: dim0,
                    half_height: dim1,
                },
                "3" => Primitive::Capsule {
                    radius: dim0,
                    length: dim1,
                },
                other => panic!("shape kind {other}"),
            };
            sets[crouched][aimed].push(Shape {
                param,
                primitive,
                centre: v(0),
                axes: [v(3), v(6), v(9)],
            });
        }
        let [[a, b], [c, d]] = sets;
        Self([[Pose::new(a), Pose::new(b)], [Pose::new(c), Pose::new(d)]])
    }

    pub(crate) fn shared() -> &'static Self {
        static POSES: std::sync::OnceLock<Poses> = std::sync::OnceLock::new();
        POSES.get_or_init(|| Self::read(TABLE, Arms::bench()))
    }

    pub(crate) fn of(&self, crouched: bool, aimed: bool) -> &Pose {
        &self.0[usize::from(crouched)][usize::from(aimed)]
    }
}

/// A body's hit boxes against a ray: the player standing at `feet`, facing `forward` (the ground direction, as
/// `kin::forward` gives it), in the stance `crouched`/`aimed`. The ray starts at `eye` along `dir` (unit); the
/// nearest hit within `reach` metres is its distance and the hit box.
pub(crate) fn ray_at_body(
    feet: Vec3,
    forward: [f32; 2],
    crouched: bool,
    aimed: bool,
    eye: Vec3,
    dir: Vec3,
    reach: f32,
) -> Option<(f32, HitParam)> {
    // The model's x is the player's right, which is (fz, -fx) on the ground in the map frame.
    let [fx, fz] = forward;
    let to_model = |v: Vec3| Vec3::new(v.x * fz - v.z * fx, v.y, v.x * fx + v.z * fz);
    let pose = Poses::shared().of(crouched, aimed);
    pose.ray(to_model(eye - feet), to_model(dir), reach)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    const FACING_Z: [f32; 2] = [0.0, 1.0];

    /// The table parses into 19 shapes in each of the four poses, each shape with axes at right angles.
    #[test]
    fn the_table_is_19_shapes_in_each_pose() {
        let poses = Poses::shared();
        for crouched in [false, true] {
            for aimed in [false, true] {
                let pose = poses.of(crouched, aimed);
                assert_eq!(pose.shapes.len(), 19, "{crouched} {aimed}");
                for shape in &pose.shapes {
                    let [x, y, z] = shape.axes;
                    for axis in [x, y, z] {
                        assert!(near(axis.length(), 1.0), "{:?}", shape.param);
                    }
                    assert!(x.dot(y).abs() < 2e-3 && y.dot(z).abs() < 2e-3 && x.dot(z).abs() < 2e-3);
                }
            }
        }
        let count = |kind: fn(&Primitive) -> bool| {
            poses.of(false, true).shapes.iter().filter(|s| kind(&s.primitive)).count()
        };
        assert_eq!(count(|p| matches!(p, Primitive::Sphere { .. })), 1);
        assert_eq!(count(|p| matches!(p, Primitive::Cylinder { .. })), 1);
        assert_eq!(count(|p| matches!(p, Primitive::Box { .. })), 7);
        assert_eq!(count(|p| matches!(p, Primitive::Capsule { .. })), 10);
        // The head sphere is the game's r 0.10 at 1.55 m standing, 1.10 m crouched.
        let head = |crouched| {
            poses.of(crouched, true).shapes.iter().find(|s| s.param == HitParam::Brain).copied().unwrap()
        };
        assert_eq!(head(false).primitive, Primitive::Sphere { radius: 0.1 });
        assert!(near(head(false).centre.y, 1.5535) && near(head(true).centre.y, 1.0969));
        // The aimed rows equal the hip rows in this table (the clips differ only in the arms' recoil layers).
        let rows = |aimed| poses.of(false, aimed).shapes.iter().map(|s| (s.param, s.centre)).collect::<Vec<_>>();
        assert_eq!(rows(false), rows(true));
    }

    #[test]
    fn a_ray_meets_a_sphere_at_its_near_side_and_not_beside_it() {
        let o = Vec3::new(0.0, 0.0, -5.0);
        let d = Vec3::new(0.0, 0.0, 1.0);
        assert!(near(ray_sphere(o, d, 0.1).unwrap(), 4.9));
        assert!(ray_sphere(Vec3::new(0.11, 0.0, -5.0), d, 0.1).is_none());
        // Grazing inside the radius; behind the origin; starting inside.
        assert!(ray_sphere(Vec3::new(0.099, 0.0, -5.0), d, 0.1).is_some());
        assert!(ray_sphere(Vec3::new(0.0, 0.0, 5.0), d, 0.1).is_none());
        assert_eq!(ray_sphere(Vec3::ZERO, d, 0.1), Some(0.0));
    }

    #[test]
    fn a_ray_meets_a_box_by_its_slabs() {
        let half = Vec3::new(0.1, 0.2, 0.3);
        let d = Vec3::new(0.0, 0.0, 1.0);
        assert!(near(ray_box(Vec3::new(0.05, 0.1, -5.0), d, half).unwrap(), 4.7));
        assert!(ray_box(Vec3::new(0.11, 0.1, -5.0), d, half).is_none());
        assert!(ray_box(Vec3::new(0.05, 0.21, -5.0), d, half).is_none());
        // A slanted ray into the box, and one that points away.
        let slant = Vec3::new(0.2, 0.0, 1.0);
        let slant = slant * (1.0 / slant.length());
        assert!(ray_box(Vec3::new(-0.8, 0.0, -4.0), slant, half).is_some());
        assert!(ray_box(Vec3::new(0.0, 0.0, 5.0), d, half).is_none());
        assert_eq!(ray_box(Vec3::ZERO, d, half), Some(0.0));
    }

    #[test]
    fn a_ray_meets_a_capsule_on_its_side_and_its_caps() {
        // A capsule of radius 0.1 and 0.4 between the hemisphere centres: 0.6 tall, standing.
        let (r, half) = (0.1, 0.2);
        let side = ray_capsule(Vec3::new(0.0, 0.1, -3.0), Vec3::new(0.0, 0.0, 1.0), r, half).unwrap();
        assert!(near(side, 2.9));
        // Over the cylinder's end the hemisphere answers: at y = 0.25 the sphere centred at 0.2 is 0.0866 m
        // wide on the ray.
        let cap = ray_capsule(Vec3::new(0.0, 0.25, -3.0), Vec3::new(0.0, 0.0, 1.0), r, half).unwrap();
        assert!(near(cap, 3.0 - (0.01f32 - 0.0025).sqrt()));
        // Above the top of the cap: none. From the top along the axis: the tip.
        assert!(ray_capsule(Vec3::new(0.0, 0.31, -3.0), Vec3::new(0.0, 0.0, 1.0), r, half).is_none());
        let down = ray_capsule(Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, -1.0, 0.0), r, half).unwrap();
        assert!(near(down, 1.7));
        // Beside it by more than the radius.
        assert!(ray_capsule(Vec3::new(0.101, 0.0, -3.0), Vec3::new(0.0, 0.0, 1.0), r, half).is_none());
    }

    #[test]
    fn a_ray_meets_a_cylinder_on_its_side_and_its_flat_caps() {
        let (r, hh) = (0.07, 0.056);
        let side = ray_cylinder(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 1.0), r, hh).unwrap();
        assert!(near(side, 1.93));
        // Above the side's height: none; from above, down onto the cap.
        assert!(ray_cylinder(Vec3::new(0.0, 0.06, -2.0), Vec3::new(0.0, 0.0, 1.0), r, hh).is_none());
        let cap = ray_cylinder(Vec3::new(0.03, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0), r, hh).unwrap();
        assert!(near(cap, 1.0 - hh));
        assert!(ray_cylinder(Vec3::new(0.08, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0), r, hh).is_none());
    }

    /// Each primitive's analytic entry against a march along random rays to a point-in-shape test.
    #[test]
    fn the_rays_agree_with_a_march() {
        let inside_capsule = |p: Vec3, r: f32, half: f32| {
            let y = p.y.clamp(-half, half);
            p.x * p.x + (p.y - y) * (p.y - y) + p.z * p.z <= r * r
        };
        let inside_cylinder = |p: Vec3, r: f32, hh: f32| p.y.abs() <= hh && p.x * p.x + p.z * p.z <= r * r;
        let inside_box = |p: Vec3, h: Vec3| p.x.abs() <= h.x && p.y.abs() <= h.y && p.z.abs() <= h.z;
        let inside_sphere = |p: Vec3, r: f32| p.dot(p) <= r * r;
        let mut rng = crate::v026g::bots::rng::Rng::new(77);
        let mut checked = [0usize; 4];
        for _ in 0..4000 {
            // From a point 2 m off, toward a point near the origin.
            let angle = rng.unit() * std::f32::consts::TAU;
            let lift = rng.unit() * 2.0 - 1.0;
            let from = Vec3::new(angle.cos(), lift, angle.sin()) * 2.0;
            let aim = Vec3::new(rng.unit() - 0.5, rng.unit() - 0.5, rng.unit() - 0.5) * 0.5;
            let d = aim - from;
            let d = d * (1.0 / d.length());
            let march = |inside: &dyn Fn(Vec3) -> bool| {
                let mut t = 0.0;
                while t < 4.0 {
                    if inside(from + d * t) {
                        return Some(t);
                    }
                    t += 0.0005;
                }
                None
            };
            let half = Vec3::new(0.12, 0.2, 0.15);
            let cases: [(usize, Option<f32>, Option<f32>); 4] = [
                (0, ray_sphere(from, d, 0.2), march(&|p| inside_sphere(p, 0.2))),
                (1, ray_box(from, d, half), march(&|p| inside_box(p, half))),
                (2, ray_cylinder(from, d, 0.15, 0.1), march(&|p| inside_cylinder(p, 0.15, 0.1))),
                (3, ray_capsule(from, d, 0.1, 0.2), march(&|p| inside_capsule(p, 0.1, 0.2))),
            ];
            for (kind, analytic, marched) in cases {
                match (analytic, marched) {
                    (Some(a), Some(m)) => {
                        assert!((a - m).abs() < 2e-3, "kind {kind}: {a} against {m}");
                        checked[kind] += 1;
                    }
                    // A graze the march steps over.
                    (Some(_), None) | (None, None) => {}
                    (None, Some(m)) => panic!("kind {kind}: the march found {m}, the ray none"),
                }
            }
        }
        assert!(checked.iter().all(|&n| n > 300), "{checked:?}");
    }

    /// A shot from `d` m in front of a body at the origin facing +z, at height `y`, `across` m to its right.
    fn from_the_front(d: f32, y: f32, across: f32, crouched: bool) -> Option<(f32, HitParam)> {
        let eye = Vec3::new(across, 1.5, d);
        let aim = Vec3::new(across, y, 0.0);
        let dir = aim - eye;
        ray_at_body(Vec3::ZERO, FACING_Z, crouched, true, eye, dir * (1.0 / dir.length()), 100.0)
    }

    /// The bullet path setting: unset or `on` flies, `0` is the instant ray, `still` flies against bodies that do not move.
    #[test]
    fn the_flight_setting_reads_on_off_and_still() {
        assert_eq!(BulletPath::parse(None), Ok(BulletPath::Flown));
        assert_eq!(BulletPath::parse(Some("on")), Ok(BulletPath::Flown));
        assert_eq!(BulletPath::parse(Some("0")), Ok(BulletPath::Instant));
        assert_eq!(BulletPath::parse(Some("still")), Ok(BulletPath::FlownBodiesStill));
        assert!(BulletPath::parse(Some("maybe")).is_err());
    }

    /// The bullet's path is the client's: at the muzzle speed along the shot, slowed by the drag (`v0 (t - k t^2 / 2)`) and
    /// pulled down by gravity (`g t^2 / 2`): an 800 m/s bullet is 80 m down the line after 0.1056 s and 5.5 cm below it.
    #[test]
    fn the_bullet_flies_the_clients_path() {
        let ammo = super::super::standin_damage::PLAIN_AMMO;
        let dir = Vec3::new(1.0, 0.0, 0.0);
        let near = bullet_at(Vec3::ZERO, dir, &ammo, 0.001);
        assert!((near.x - 0.8).abs() < 0.01 && near.y.abs() < 1e-4, "{near:?}");
        let far = bullet_at(Vec3::ZERO, dir, &ammo, 0.1056);
        assert!((far.x - 80.0).abs() < 0.2, "{far:?}");
        assert!((far.y + 0.0547).abs() < 0.002, "{far:?}");
        assert!((flight_end_s(&ammo) - 1.0 / 1.1).abs() < 1e-4);
        let slow = survarium::v026g::damage::AmmoParams { muzzle_speed: 280.0, ..ammo };
        let t = 0.31;
        let at = bullet_at(Vec3::ZERO, dir, &slow, t);
        assert!(at.y < -0.4, "a VSK-94 bullet drops about half a metre by 80 m: {at:?}");
    }

    /// The arm shapes taken away (`SRP_BENCH_HIT_ARMS=away`) leave 13 shapes in each pose and no arm hit: a line at the chest
    /// height from the front, where the posed forearm is first, meets the torso.
    #[test]
    fn with_the_arms_away_a_chest_line_meets_the_torso_and_the_pose_has_no_arm_shape() {
        let away = Poses::read(TABLE, Arms::Away);
        for crouched in [false, true] {
            for aimed in [false, true] {
                let pose = away.of(crouched, aimed);
                assert_eq!(pose.shapes.len(), 13, "{crouched} {aimed}");
                assert!(pose.shapes.iter().all(|s| s.param.part() != Part::Arm));
            }
        }
        let eye = Vec3::new(0.0, 1.5, 10.0);
        let dir = Vec3::new(0.0, 1.22, 0.0) - eye;
        let dir = dir * (1.0 / dir.length());
        let to_model = |v: Vec3| Vec3::new(v.x * FACING_Z[1] - v.z * FACING_Z[0], v.y, v.x * FACING_Z[0] + v.z * FACING_Z[1]);
        let (_, param) = away.of(false, true).ray(to_model(eye), to_model(dir), 100.0).expect("the torso");
        assert_eq!(param.part(), Part::Torso);
        assert_eq!(Arms::parse(Some("away")), Ok(Arms::Away));
        assert_eq!(Arms::parse(None), Ok(Arms::Posed));
        assert!(Arms::parse(Some("gone")).is_err());
    }

    /// A ray at the chest hits the front of the torso; at the head sphere's centre, the head; a ray over the
    /// head or beside the body misses.
    #[test]
    fn a_shot_at_the_chest_hits_the_torso_and_one_at_the_head_the_head() {
        // From the front the rifle arms are in the way (the left forearm lies across the chest at 0.24 m
        // ahead of the position): the first shape is an arm. From behind the back boxes are first.
        let (t, param) = from_the_front(10.0, 1.22, 0.0, false).expect("the chest");
        assert!(matches!(param.part(), Part::Arm | Part::Torso));
        assert!(t > 9.5 && t < 10.5);
        let back = ray_at_body(
            Vec3::ZERO,
            FACING_Z,
            false,
            true,
            Vec3::new(0.0, 1.5, -10.0),
            Vec3::new(0.0, -0.28, 10.0) * (1.0 / Vec3::new(0.0, -0.28, 10.0).length()),
            100.0,
        );
        assert_eq!(back.map(|b| b.1.part()), Some(Part::Torso));
        // The head sphere: centre (0.055, 1.5535, -0.11) in the model frame; the body faces +z, so the model's
        // x is the map's.
        let (_, head) = from_the_front(10.0, 1.5535, 0.055, false).expect("the head");
        assert_eq!(head.part(), Part::Head);
        // From behind nothing is in front of the sphere: the brain itself.
        let behind = ray_at_body(
            Vec3::ZERO,
            FACING_Z,
            false,
            true,
            Vec3::new(0.055, 1.5535, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            100.0,
        );
        assert_eq!(behind.map(|b| b.1), Some(HitParam::Brain));
        assert!(from_the_front(10.0, 1.80, 0.0, false).is_none());
        assert!(from_the_front(10.0, 1.2, 0.55, false).is_none());
        assert!(from_the_front(10.0, 1.2, -0.55, false).is_none());
    }

    /// The old disc's edge: 0.40 m off the chest point (the old rule hit at any line under 0.45 m) is beside
    /// the arms on either side and over the head.
    #[test]
    fn a_line_at_the_old_discs_edge_misses() {
        let chest = 1.30;
        assert!(from_the_front(10.0, chest, 0.40, false).is_none());
        assert!(from_the_front(10.0, chest, -0.40, false).is_none());
        assert!(from_the_front(10.0, chest + 0.40, 0.0, false).is_none());
        // 0.40 m under the chest point is the belly and the thigh: still the body.
        assert!(from_the_front(10.0, chest - 0.40, 0.0, false).is_some());
    }

    /// A crouched body is 1.2 m tall: a line at 1.4 m passes over it and hits the standing one.
    #[test]
    fn standing_and_crouched_differ() {
        assert!(from_the_front(8.0, 1.40, 0.0, false).is_some());
        assert!(from_the_front(8.0, 1.40, 0.0, true).is_none());
        assert!(from_the_front(8.0, 1.05, 0.0, true).is_some());
        // The head is lower: the sphere's centre 1.10 m crouched, 0.065 m across.
        let (_, head) = from_the_front(8.0, 1.0969, 0.065, true).expect("crouched head");
        assert_eq!(head.part(), Part::Head);
    }

    /// The body turns with its yaw: the bladed torso lies about 0.2 m behind the position, so it is nearer to a
    /// shooter behind it than to one in front.
    #[test]
    fn the_body_faces_its_yaw() {
        let dir = Vec3::new(0.0, 0.0, -1.0);
        let eye = Vec3::new(0.0, 1.3, 10.0);
        let faces_the_shooter = ray_at_body(Vec3::ZERO, [0.0, 1.0], false, true, eye, dir, 100.0).unwrap();
        let back_to_the_shooter = ray_at_body(Vec3::ZERO, [0.0, -1.0], false, true, eye, dir, 100.0).unwrap();
        assert!(faces_the_shooter.0 > back_to_the_shooter.0 + 0.1, "{faces_the_shooter:?} {back_to_the_shooter:?}");
        // Facing +x, side on to the shooter: still a body.
        assert!(ray_at_body(Vec3::ZERO, [1.0, 0.0], false, true, eye, dir, 100.0).is_some());
    }

    /// Past the reach nothing is hit.
    #[test]
    fn the_reach_ends_the_ray() {
        let eye = Vec3::new(0.0, 1.22, 10.0);
        let dir = Vec3::new(0.0, 0.0, -1.0);
        assert!(ray_at_body(Vec3::ZERO, FACING_Z, false, true, eye, dir, 100.0).is_some());
        assert!(ray_at_body(Vec3::ZERO, FACING_Z, false, true, eye, dir, 9.0).is_none());
        // Behind the shooter.
        assert!(ray_at_body(Vec3::ZERO, FACING_Z, false, true, eye, -dir, 100.0).is_none());
    }

    /// The box hit's rate and part shares for a chest-aimed shot with the bench's scatter (model B of
    /// `fact-hitboxes-026g.md` 5.1: an isotropic Gaussian of 0.38 m at 10 m, 8 azimuths): 0.323 a shot, torso
    /// about half, the arms a quarter, the head a tenth, the legs the rest (section 3's 0.52 / 0.27 / 0.11 /
    /// 0.10).
    #[test]
    fn the_first_hit_shares_follow_the_documented_ones() {
        let mut rng = crate::v026g::bots::rng::Rng::new(5);
        let mut gauss = move || {
            let (u1, u2) = (rng.unit().max(1e-9), rng.unit());
            (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
        };
        let (mut parts, mut hits, mut shots) = ([0u32; 4], 0u32, 0u32);
        for azimuth in 0..8 {
            let yaw = azimuth as f32 * std::f32::consts::TAU / 8.0;
            let (s, c) = yaw.sin_cos();
            for _ in 0..4000 {
                let eye = Vec3::new(10.0 * s, 1.47, 10.0 * c);
                let aim = Vec3::new(0.0, 1.30, 0.0)
                    + Vec3::new(c, 0.0, -s) * (0.38 * gauss())
                    + Vec3::new(0.0, 0.38 * gauss(), 0.0);
                let dir = aim - eye;
                let dir = dir * (1.0 / dir.length());
                shots += 1;
                if let Some((_, param)) = ray_at_body(Vec3::ZERO, FACING_Z, false, true, eye, dir, 100.0) {
                    hits += 1;
                    parts[match param.part() {
                        Part::Torso => 0,
                        Part::Head => 1,
                        Part::Arm => 2,
                        Part::Leg => 3,
                    }] += 1;
                }
            }
        }
        let rate = hits as f32 / shots as f32;
        assert!((rate - 0.323).abs() < 0.03, "per shot at 10 m {rate}, the note's 0.323");
        let share = |i: usize| parts[i] as f32 / hits as f32;
        assert!((share(0) - 0.52).abs() < 0.08, "torso {}", share(0));
        assert!((share(2) - 0.27).abs() < 0.08, "arms {}", share(2));
        assert!((share(1) - 0.11).abs() < 0.05, "head {}", share(1));
        assert!((share(3) - 0.10).abs() < 0.06, "legs {}", share(3));
    }

    #[test]
    fn a_hit_param_is_a_part_and_a_name_both_ways() {
        for name in [
            "brain", "face", "front", "back", "right_hip", "left_hip", "right_foot", "left_foot",
            "right_forearm", "left_forearm", "right_arm", "left_arm",
        ] {
            let param = HitParam::of(name).unwrap();
            assert_eq!(param.name(), name);
            assert!(param.part().boxes().contains(&name));
            assert_eq!(param.leg().is_some(), param.part() == Part::Leg);
        }
        assert_eq!(HitParam::of("tail"), None);
    }
}

/// The stand-in's shots through [`Game::shot`]: the old disc against the boxes, the head, the pellets, and the
/// switch off against the shot as it was.
#[cfg(test)]
mod game_tests {
    use super::*;
    use crate::v026g::bots::guns::gun;
    use super::super::*;

    fn put(game: &mut Game, place: Place, x: f32, z: f32, keys: u32) {
        let body = &mut game.bodies[place];
        body.alive = true;
        body.health = 1.0;
        body.pools = super::super::standin_damage::Pools::default();
        body.feet = Vec3::new(x, 0.0, z);
        body.yaw = 0.0;
        body.keys = keys;
        body.step_m = 0.0;
        body.last_hit = 0;
    }

    /// Shooter 0 at (10, 10) facing +z with the view on `aim` (a map point) from where its eye is.
    fn look_at(game: &mut Game, aim: Vec3) {
        for _ in 0..3 {
            let eye = Game::eye(&game.bodies[0]);
            let to = aim - eye;
            game.bodies[0].yaw = crate::v026g::bots::kin::yaw_of([to.x, to.z]);
            game.bodies[0].pitch = to.y.atan2(to.x.hypot(to.z));
        }
    }

    fn two_bodies(volume: HitVolume) -> Game {
        let mut game = Game::new(soak_world(), &[0, 1], 1, None, None);
        game.volume = volume;
        put(&mut game, 0, 10.0, 10.0, 0);
        game.bodies[0].target = Some(1);
        game
    }

    /// A line 0.40 m beside the chest and a little above it is inside the old 0.45 m disc and between the arm
    /// and nothing for the shapes.
    #[test]
    fn a_shot_at_the_old_discs_edge_hits_the_disc_and_misses_the_boxes() {
        for (volume, hit) in [(HitVolume::Disc, true), (HitVolume::BOXES, false)] {
            let mut game = two_bodies(volume);
            put(&mut game, 1, 10.40, 29.0, 0);
            let chest = Game::chest(&game.bodies[1]);
            look_at(&mut game, Vec3::new(10.0, chest.y, chest.z));
            game.shot(0, 0.0);
            let fact = game.shot_facts[0];
            let miss = fact.offset.and_then(|o| o.line).expect("a line").miss;
            assert!(miss > 0.39 && miss < DISC_M, "the premise: {miss}");
            assert_eq!(fact.hit.is_some(), hit, "{volume:?}");
            assert_eq!(game.hits, u32::from(hit), "{volume:?}");
            assert_eq!(game.bodies[1].health < 1.0, hit, "{volume:?}");
        }
    }

    /// A shot at the head sphere takes the head pool and the pain, not the body, and counts a head hit.
    #[test]
    fn a_shot_at_the_head_hurts_the_head() {
        let mut game = two_bodies(HitVolume::BOXES);
        // The target faces away (yaw 0, +z) from the shooter: the sphere's centre is (0.055, 1.5535, -0.11)
        // from its feet, with nothing in front of it from behind.
        put(&mut game, 1, 10.0 - 0.055, 29.11, 0);
        look_at(&mut game, Vec3::new(10.0, 1.5535, 29.0));
        let full = super::super::standin_damage::Pools::default();
        game.shot(0, 0.0);
        let fact = game.shot_facts[0];
        assert_eq!(fact.hit, Some(1));
        assert_eq!(fact.landed.parts, [0, 1, 0, 0], "{:?}", fact.landed);
        // The head pool is 0.4 and a rifle's bullet takes it all: the game's headshot, a kill.
        let target = &game.bodies[1];
        assert!(target.pools.head < full.head, "{:?}", target.pools);
        assert!(target.pools.pain < full.pain);
        assert!(!target.alive && game.kill_log.len() == 1, "a headshot kills");
    }

    /// A line through the legs hurts a leg; the thigh of the right leg is the stand-in's leg 0.
    #[test]
    fn a_shot_at_a_leg_hurts_that_leg() {
        let mut game = two_bodies(HitVolume::BOXES);
        put(&mut game, 1, 10.0, 29.0, 0);
        // The right shin's centre: (0.1655, 0.2582, -0.3612) in the model frame.
        look_at(&mut game, Vec3::new(10.0 + 0.1655, 0.2582, 29.0 - 0.3612));
        game.shot(0, 0.0);
        let target = &game.bodies[1];
        assert_eq!(game.shot_facts[0].landed.parts, [0, 0, 0, 1]);
        // One leg only: the line meets one leg's shape first, whichever (the bladed stance puts the left leg
        // forward).
        assert!(target.legs.iter().filter(|&&pool| pool < 0.5).count() == 1, "{:?}", target.legs);
        assert_eq!(target.health, 1.0);
    }

    /// Crouched, the same line over the head passes; standing, it hits.
    #[test]
    fn a_crouched_target_is_lower() {
        let mut hit = Vec::new();
        for keys in [0, key::CROUCH] {
            let mut game = two_bodies(HitVolume::BOXES);
            put(&mut game, 1, 10.0, 29.0, keys);
            look_at(&mut game, Vec3::new(10.0, 1.40, 29.0));
            game.shot(0, 0.0);
            hit.push(game.shot_facts[0].hit.is_some());
        }
        assert_eq!(hit, [true, false]);
    }

    /// A wall between the eye and the body stops a shot whose line is on the body (the old rule only needed the
    /// eye to see the chest).
    #[test]
    fn the_bullet_mesh_stops_a_shot_before_the_body() {
        let world = super::super::soak_world_with(&wall_triangles());
        let mut game = Game::new(world, &[0, 1], 1, None, None);
        game.volume = HitVolume::BOXES;
        put(&mut game, 0, 10.0, 10.0, 0);
        game.bodies[0].target = Some(1);
        put(&mut game, 1, 10.0, 29.0, 0);
        look_at(&mut game, Vec3::new(10.0, 1.22, 29.0));
        game.shot(0, 0.0);
        assert_eq!(game.shot_facts[0].hit, None);
        assert_eq!(game.bodies[1].health, 1.0);
    }

    /// The game's penetration in the stand-in: a bullet goes through a crate of wood (armor 0.1) at full damage and is
    /// counted as one that crossed cover; a stone wall (0.7) stops it; brick (0.2) lets an SMG's bullet through with less.
    #[test]
    fn a_bullet_crosses_cover_by_its_pierce_and_the_materials() {
        use mapdata::{Armor, FaceArmor, RayTree, World};
        let wall = |armor: f32| {
            let (points, triangles) = mapdata::library::PrimitiveShape::Box { half: Vec3::new(5.0, 2.0, 0.2) }.tessellate();
            let centre = Vec3::new(10.0, 2.0, 20.0);
            RayTree::from_armored_corners(
                triangles
                    .iter()
                    .map(|t| (t.map(|i| points[i as usize] + centre), FaceArmor::Material(Armor::new(armor).unwrap()))),
            )
        };
        let shot = |armor: Option<f32>, item: &str| {
            let base = soak_world_with(&[]);
            let world = match armor {
                Some(armor) => std::sync::Arc::new(World::assemble(base.map.clone(), base.grid.clone(), wall(armor).into())),
                None => base,
            };
            let mut game = Game::new(world, &[0, 1], 1, None, None);
            game.volume = HitVolume::BOXES;
            put(&mut game, 0, 10.0, 10.0, 0);
            game.bodies[0].target = Some(1);
            game.bodies[0].gun = gun(item).expect("the gun");
            put(&mut game, 1, 10.0, 29.0, 0);
            look_at(&mut game, Vec3::new(10.0, 1.22, 29.0));
            game.shot(0, 0.0);
            (game.shot_facts[0].hit.is_some(), 1.0 - game.bodies[1].health, game.cover_hits_by[0])
        };
        let (open_hit, open_damage, open_cover) = shot(None, "ak_74u");
        assert!(open_hit && open_damage > 0.0 && open_cover == 0, "{open_hit} {open_damage} {open_cover}");
        // Wood: all of the damage (the share is 1, so it is not counted as a hit through cover).
        let (hit, damage, _) = shot(Some(0.1), "ak_74u");
        assert!(hit && (damage - open_damage).abs() < 1e-6, "{hit} {damage} {open_damage}");
        // Stone stops it.
        let (hit, damage, cover) = shot(Some(0.7), "ak_74u");
        assert!(!hit && damage == 0.0 && cover == 0, "{hit} {damage} {cover}");
        // A wall at 0.7 of the gun's own pierce: k = 1 - (1 - 3/7)^2 = 0.67, so less than the open shot reaches the body,
        // and the hit counts as one through cover.
        let probe = Game::new(soak_world_with(&[]), &[0, 1], 1, None, None);
        let pierce = super::super::standin_damage::pierce_of(&probe.damage, gun("ak_74u").unwrap()).expect("its pierce");
        let (hit, damage, cover) = shot(Some(0.7 * pierce.value()), "ak_74u");
        assert!(hit && damage > 0.0 && damage < open_damage && cover == 1, "{hit} {damage} {open_damage} {cover}");
    }

    /// A wall across z = 20 from x = 5 to 15, 4 m high.
    fn wall_triangles() -> Vec<mapdata::Triangle> {
        let walker = mapdata::WalkerBehaviour::BySlope;
        let corner = |x: f32, y: f32| Vec3::new(x, y, 20.0);
        let (a, b, c, d) = (corner(5.0, 0.0), corner(15.0, 0.0), corner(15.0, 4.0), corner(5.0, 4.0));
        vec![
            mapdata::Triangle { v: [a, b, c], walker },
            mapdata::Triangle { v: [a, c, d], walker },
        ]
    }

    /// The shotgun's ten pellets in the ammunition's cone (4 degrees, `buck`), each its own ray: the mean landed
    /// on a standing target with the bench's shot-line scatter matches the note's (`fact-hitboxes-026g.md` 4:
    /// 3.8 at 5 m, 3.0 at 10 m, the boxes, mean of 4 azimuths), and an aim on the torso with none lands nearly all
    /// of them at 5 m. Fixed seeds.
    #[test]
    fn ten_pellets_land_as_many_as_the_cone_and_the_scatter_say() {
        let shotgun = gun("spas_12").expect("a shotgun");
        let mut rng = crate::v026g::bots::rng::Rng::new(11);
        let mut mean = |distance: f32, sigma: f32| {
            let (mut landed, mut shots) = (0u32, 0u32);
            for azimuth in 0..4 {
                let mut game = two_bodies(HitVolume::BOXES);
                game.bodies[0].gun = shotgun;
                put(&mut game, 1, 10.0, 10.0 + distance, 0);
                game.bodies[1].yaw = azimuth as f32 * std::f32::consts::FRAC_PI_2;
                for _ in 0..700 {
                    let n = |rng: &mut crate::v026g::bots::rng::Rng| {
                        let (u1, u2) = (rng.unit().max(1e-9), rng.unit());
                        (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
                    };
                    let aim = Vec3::new(10.0 + sigma * n(&mut rng), 1.30 + sigma * n(&mut rng), 10.0 + distance);
                    look_at(&mut game, aim);
                    game.shot(0, 0.0);
                    landed += u32::from(game.shot_facts.pop().unwrap().landed.on_enemy);
                    shots += 1;
                    put(&mut game, 1, 10.0, 10.0 + distance, 0);
                }
            }
            landed as f32 / shots as f32
        };
        let (five, ten) = (mean(5.0, 0.32), mean(10.0, 0.38));
        assert!((five - 3.8).abs() < 0.5, "5 m: {five}");
        assert!((ten - 3.0).abs() < 0.5, "10 m: {ten}");
        assert!(five > ten);
        let sure = mean(5.0, 0.0);
        assert!(sure >= 8.0, "5 m, aimed on the torso: {sure}");
        let ammo = super::super::standin_damage::ammo_of(&super::super::standin_damage::Model::default(), shotgun);
        assert_eq!((ammo.buck_shot, ammo.buck_dispersion), (10, 4.0));
    }

    /// Pellets that land on one body hurt it together: ten torso pellets from close by are many hits at once.
    #[test]
    fn the_pellets_of_a_shot_hurt_together() {
        let mut game = two_bodies(HitVolume::BOXES);
        game.bodies[0].gun = gun("spas_12").unwrap();
        put(&mut game, 1, 10.0, 12.0, 0);
        look_at(&mut game, Vec3::new(10.0, 1.2, 12.0));
        game.shot(0, 0.0);
        let fact = game.shot_facts[0];
        assert_eq!(fact.landed.fired, 10);
        assert!(fact.landed.on_enemy >= 8, "{:?}", fact.landed);
        assert_eq!(game.hits, 1, "one shot, one hit on an enemy");
        assert_eq!(game.hit_log.len(), 1);
        assert!(game.bodies[1].health < 0.5 || !game.bodies[1].alive);
    }

    /// The shot as it was before the hit volume (`fighting/c0-shot-offset` 297a0885, `Game::shot`), word for
    /// word but for `self` -> `game` and the new field of the fact: what `SRP_BENCH_HIT_MODEL=off` must equal.
    fn old_shot(game: &mut Game, shooter: Place, at: f32) {
        game.shots += 1;
        game.shots_hits[shooter].0 += 1;
        let body = &game.bodies[shooter];
        let eye = Game::eye(body);
        let [fx, fz] = forward(body.yaw);
        let pitch = body.view_elevation();
        let (c, s) = (pitch.cos(), pitch.sin());
        let dir = Vec3::new(fx * c, s, fz * c);
        let kick = body.recoil.kick();
        let top = kick.top(game.rng.unit());
        let (gun, keys) = (body.gun, body.keys);
        game.bodies[shooter].recoil.shot(at, top, 0.0);
        let (aim_state, cone) = game.bodies[shooter].spread.shot(gun, keys, at);
        let view_dir = dir;
        let dir = if game.spread {
            bench_spread::deflect(dir, cone, &mut game.rng)
        } else {
            dir
        };
        let tick = game.tick;
        let body = &game.bodies[shooter];
        // A shot at the bot's target with nothing of him showing
        // (`aimpoint`'s parts), or whose ray meets the world short of
        // him.
        if let Some(target) = body.target.map(|t| &game.bodies[t])
            && target.alive
        {
            let before = super::super::aimpoint::rays_cast();
            let crouched = target.keys & key::CROUCH != 0;
            let team = usize::from(body.team).min(1);
            if !super::super::aimpoint::shows(eye, target.feet, crouched, &game.world.sight, || true) {
                game.hidden += 1;
                game.hidden_by[team] += 1;
            }
            let reach = (Game::chest(target) - eye).length() - 0.5;
            if game.world.sight.ray_cast(eye, dir, reach).is_some() {
                game.walled += 1;
                game.walled_by[team] += 1;
            }
            game.metric_rays += super::super::aimpoint::rays_cast() - before;
        }
        let mut best: Option<(Place, f32)> = None;
        for (target, other) in game.bodies.iter().enumerate() {
            if !other.alive || target == shooter {
                continue;
            }
            let to = Game::chest(other) - eye;
            let along = to.dot(dir);
            if along <= 0.0 || along > 100.0 {
                continue;
            }
            let miss = (to - dir * along).length();
            if miss < 0.45
                && best.is_none_or(|(_, d)| along < d)
                && game.world.sight.visible(eye, Game::chest(other))
            {
                best = Some((target, along));
            }
        }
        let shooter_body = &game.bodies[shooter];
        let (moving, aimed, crouched) = fight_tally::stance(shooter_body.keys);
        let aimed_at = shooter_body
            .target
            .filter(|&t| game.bodies[t].alive)
            .map(|t| {
                let (d, off) = fight_tally::aim_off(eye, dir, Game::chest(&game.bodies[t]));
                (t, d, off)
            });
        // [counts only] Where the shot line passed the target's chest, for the owner counts' `shot_offsets`.
        let offset = aimed_at.map(|(t, distance, _)| {
            let target = &game.bodies[t];
            super::super::tallies::shot_offset::ShotOffset::of(
                eye,
                dir,
                Game::chest(target),
                distance,
                super::super::tallies::shot_offset::ShotClass::of(gun.class()),
                target.keys & key::CROUCH != 0,
            )
        });
        game.shot_facts.push(fight_tally::ShotFact {
            shooter,
            tick,
            hit: best
                .map(|b| b.0)
                .filter(|&t| game.bodies[t].team != shooter_body.team),
            target: aimed_at,
            moving,
            aimed,
            crouched,
            aim_state,
            cone,
            offset,
            rise: aimed_at.map(|(t, ..)| fight_tally::Rise::of(eye, view_dir, Game::chest(&game.bodies[t]))),
            landed: Default::default(),
        });
        let Some((target, along)) = best else {
            return;
        };
        let team_hit = game.bodies[target].team == body.team;
        if team_hit {
            game.team_hits += 1;
            game.team_hit_shots.push((shooter, target, eye, dir, cone, view_dir));
        } else {
            game.hits += 1;
            game.shots_hits[shooter].1 += 1;
            game.hit_log.push((tick, shooter, target));
        }
        let pain_before = game.bodies[target].pools.pain;
        let other = &mut game.bodies[target];
        standin_damage::hit(&game.damage, other, gun, along, &mut game.rng, tick);
        other.last_hit = tick;
        other.stagger.hit(tick, standin_stagger::power_of(&game.damage, gun));
        if let Some(leg) = stand_in_leg_hit(tick, shooter, target) {
            other.legs[leg] = (other.legs[leg] - STAND_IN_LEG_HIT).max(0.0);
            if other.legs[leg] <= 0.0 && other.leg_broken_until == 0 {
                other.leg_broken_until = tick + STAND_IN_LEG_BROKEN_MS;
            }
        }
        let lethal = other.health <= 1e-3;
        if team_hit {
            game.team_hit_pains.push((pain_before, lethal));
        }
        if lethal {
            let kind = if team_hit { Kill::Mate } else { Kill::Enemy };
            game.kill(target, shooter, kind, tick);
        }
    }

    /// A table of shots at three bodies (two enemies, a teammate) in many places and facings, with spread: the
    /// disc volume's `Game::shot` equals the old function in every outcome: bodies, counters, the facts, the
    /// next draw of the generator.
    #[test]
    fn the_disc_is_the_old_shot_on_a_table_of_shots() {
        let teams = [0, 0, 1, 1];
        let mut now = Game::new(soak_world(), &teams, 5, None, None);
        let mut old = Game::new(soak_world(), &teams, 5, None, None);
        for game in [&mut now, &mut old] {
            game.spread = true;
            assert_eq!(game.volume, HitVolume::Disc, "the default of a game outside the bench");
        }
        let mut table = crate::v026g::bots::rng::Rng::new(99);
        let (mut hits, mut mates) = (0, 0);
        for shot in 0..600u32 {
            for game in [&mut now, &mut old] {
                game.tick = 1_000 + shot * 50;
            }
            // The same placement of everyone in both.
            let draw = |table: &mut crate::v026g::bots::rng::Rng, lo: f32, hi: f32| lo + (hi - lo) * table.unit();
            let mut layout: Vec<(f32, f32, f32, u32)> = Vec::new();
            for place in 0..4 {
                let keys = if table.unit() < 0.3 { key::CROUCH } else { 0 } | if table.unit() < 0.4 { key::AIM } else { 0 };
                layout.push(if place == 0 {
                    (10.0, 10.0, 0.0, key::AIM)
                } else {
                    (draw(&mut table, 6.0, 14.0), draw(&mut table, 12.0, 40.0), draw(&mut table, -3.0, 3.0), keys)
                });
            }
            let (yaw, pitch) = (draw(&mut table, -0.25, 0.25), draw(&mut table, -0.12, 0.05));
            let at = draw(&mut table, 0.0, 49.0);
            let target = [None, Some(1usize), Some(2), Some(3)][(table.unit() * 4.0) as usize % 4];
            for game in [&mut now, &mut old] {
                for (place, &(x, z, body_yaw, keys)) in layout.iter().enumerate() {
                    put(game, place, x, z, keys);
                    game.bodies[place].yaw = body_yaw;
                }
                game.bodies[0].yaw = yaw;
                game.bodies[0].pitch = pitch;
                game.bodies[0].target = target;
                game.bodies[0].keys = key::AIM | key::FIRE;
            }
            now.shot(0, at);
            old_shot(&mut old, 0, at);
            assert_eq!(format!("{:?}", now.rng), format!("{:?}", old.rng), "shot {shot}: the generator");
            assert_eq!(
                (now.shots, now.hits, now.team_hits, now.team_kills, now.shots_hits.clone(), now.hit_log.clone()),
                (old.shots, old.hits, old.team_hits, old.team_kills, old.shots_hits.clone(), old.hit_log.clone()),
                "shot {shot}"
            );
            for place in 0..4 {
                let (a, b) = (&now.bodies[place], &old.bodies[place]);
                assert_eq!(
                    (a.alive, a.health, a.pools, a.legs, a.leg_broken_until, a.last_hit),
                    (b.alive, b.health, b.pools, b.legs, b.leg_broken_until, b.last_hit),
                    "shot {shot}, body {place}"
                );
            }
            let key_of = |f: &fight_tally::ShotFact| {
                format!(
                    "{:?}",
                    (f.shooter, f.tick, f.hit, f.target, f.moving, f.aimed, f.crouched, f.aim_state, f.cone, f.offset)
                )
            };
            let (a, b) = (now.shot_facts.pop().unwrap(), old.shot_facts.pop().unwrap());
            assert_eq!(key_of(&a), key_of(&b), "shot {shot}: the fact");
            assert!(now.shot_facts.is_empty() && old.shot_facts.is_empty());
            assert_eq!(now.team_hit_shots.len(), old.team_hit_shots.len());
            hits += i32::from(a.hit.is_some());
            mates += i32::from(now.team_hits > 0);
        }
        // The table does reach hits and a teammate, or it proves nothing.
        assert!(hits > 20 && mates > 5, "hits {hits} team {mates}");
    }

    /// A deterministic soak with the whole cut (`SRP_BENCH_HIT_MODEL=on`): it repeats itself exactly, and hits fewer of
    /// the shots fired than the disc does, over eight soaks (one 60 s soak is a few hundred shots: any change of the
    /// bots' play, such as the route-search queue's, can turn its sample either way).
    #[test]
    fn a_soak_with_the_whole_cut_repeats_and_hits_less_than_with_the_disc() {
        let run = |volume, seed| soak_volume(4, 0, 12, None, 60, seed, bench_tuning(), volume);
        let on = HitVolume::ON;
        let (a, b) = (run(on, 3), run(on, 3));
        assert_eq!((a.shots, a.hits, a.kills), (b.shots, b.hits, b.kills));
        let summed = |volume| {
            (3..11).map(|seed| run(volume, seed)).fold((0, 0), |(shots, hits), o| (shots + o.shots, hits + o.hits))
        };
        let (boxes, disc) = (summed(on), summed(HitVolume::Disc));
        assert!(boxes.0 > 200 && disc.0 > 200, "{} {}", boxes.0, disc.0);
        let rate = |(shots, hits): (u32, u32)| hits as f32 / shots as f32;
        assert!(rate(boxes) < rate(disc), "boxes {} disc {}", rate(boxes), rate(disc));
    }

    /// The setting's values.
    #[test]
    fn the_setting_names_the_whole_cut_or_the_old_disc() {
        assert_eq!(HitVolume::parse(None), HitVolume::Disc, "unset is the old disc");
        assert_eq!(HitVolume::parse(Some("on")), HitVolume::Boxes { scatter_m: Some(POSITION_SIGMA_M) });
        assert_eq!(HitVolume::parse(Some("off")), HitVolume::Disc);
        assert_eq!(HitVolume::parse(Some("boxes")), HitVolume::BOXES);
        assert_eq!(HitVolume::parse(Some("scatter=0.3")), HitVolume::Boxes { scatter_m: Some(0.3) });
        assert!(std::panic::catch_unwind(|| HitVolume::parse(Some("scatter=-1"))).is_err());
        assert!(std::panic::catch_unwind(|| HitVolume::parse(Some("yes"))).is_err());
    }

    /// The position scatter is one random offset across the shot line, isotropic, of the sigma given: the radial
    /// median is 1.177 sigma (0.20 m for the default), and the offset is perpendicular to the line. The old
    /// disc and the boxes without the scatter leave the line where it is and draw nothing.
    #[test]
    fn the_position_scatter_is_a_gaussian_across_the_line() {
        let mut game = two_bodies(HitVolume::ON);
        let eye = Vec3::new(10.0, 1.5, 10.0);
        let dir = Vec3::new(0.0, 0.6, 0.8);
        let mut radii = Vec::new();
        let (mut sum_across, mut sum_along) = (Vec3::ZERO, 0.0f32);
        for _ in 0..4000 {
            let (origin, d) = game.shot_line(eye, dir, 0.0);
            assert_eq!(d, dir, "no weapon cone, no spread: the direction stays");
            let off = origin - eye;
            sum_along += off.dot(dir).abs();
            sum_across = sum_across + off;
            radii.push(off.length());
        }
        radii.sort_by(f32::total_cmp);
        let median = radii[radii.len() / 2];
        assert!((median - 0.2).abs() < 0.015, "median {median}");
        assert!(sum_along < 1e-2, "perpendicular to the line: {sum_along}");
        assert!(sum_across.length() / 4000.0 < 0.01, "centred: {:?}", sum_across);
        for volume in [HitVolume::Disc, HitVolume::BOXES] {
            let mut game = two_bodies(volume);
            let before = format!("{:?}", game.rng);
            assert_eq!(game.shot_line(eye, dir, 0.0), (eye, dir));
            assert_eq!(format!("{:?}", game.rng), before, "{volume:?}: nothing drawn");
        }
    }

    /// The scatter lowers the rate at which a line aimed at the chest at 10 m hits (the boxes alone hit it
    /// nearly always, a median offset of 0.20 m puts a share of the shots by the body).
    #[test]
    fn the_scatter_costs_hits_on_a_line_aimed_at_the_chest() {
        let rate = |volume| {
            let mut hits = 0;
            let mut game = two_bodies(volume);
            for _ in 0..600 {
                put(&mut game, 1, 10.0, 20.0, 0);
                game.bodies[1].yaw = std::f32::consts::PI;
                look_at(&mut game, Vec3::new(10.0, 1.22, 20.0));
                game.shot(0, 0.0);
                hits += usize::from(game.shot_facts.pop().expect("a shot").hit.is_some());
            }
            hits as f32 / 600.0
        };
        let (plain, scattered) = (rate(HitVolume::BOXES), rate(HitVolume::ON));
        assert!(plain > 0.95, "{plain}");
        assert!(scattered < plain - 0.1 && scattered > 0.4, "{scattered} against {plain}");
    }
}
