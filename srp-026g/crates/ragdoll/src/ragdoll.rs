//! One ragdoll: eleven capsules and ten joints made from the pose of a
//! death, stepped once a frame (six XPBD substeps), and the drawn skeleton
//! made from them.
//!
//! The recipe is the Bodycam study's (`docs/notes/ragdoll.md` section 7):
//! the bodies start with the motion the player had, a joint-space drive
//! pulls them toward the game's death clip with a strength that fades per
//! joint group, the last hit adds its impulse, and the level is read by
//! one swept query per moving body a frame, whose plane the substeps keep.

use crate::body::{Body, Capsule, Contact, Pair, close_gap, share_turn};
use crate::limits::{Cone, Limit, degrees};
use crate::math::{Pose, Quat, Vec3, clamp, exp_neg, finite, max, min};
use crate::shot::{Shot, ShotHit, hit_on_capsule, ray_capsule};
use crate::skeleton::{
    BONE_COUNT, Bone, HEAD, ITEM, JOINT_TABLE, JOINTS, JointKind, LEFT_ARM, LEFT_FOOT,
    LEFT_FORE_ARM, LEFT_HAND, LEFT_LEG, LEFT_TOE, LEFT_UP_LEG, NECK, PARTS, Part, RIGHT_ARM,
    RIGHT_FOOT, RIGHT_FORE_ARM, RIGHT_HAND, RIGHT_LEG, RIGHT_TOE, RIGHT_UP_LEG, ROOT, SPIN, WEAPON,
    adjacent,
};
use crate::tuning;

/// The drawn skeleton: every bone's world pose, in [`crate::skeleton::BONES`]' order.
pub type Skeleton = [Pose; BONE_COUNT];

/// The parts' world poses.
pub type PartPoses = [Pose; PARTS];

/// The world poses of the bones the parts move ([`Part::bone`]), by part.
pub type DriverBones = [Pose; PARTS];

/// The driver bones of a skeleton.
pub fn driver_bones(skeleton: &Skeleton) -> DriverBones {
    Part::ALL.map(|p| skeleton[p.bone().index()])
}

// The recipe's numbers (to tune in the game).
pub const GRAVITY: f32 = 9.81;
pub const MAX_DT: f32 = 1.0 / 30.0;
/// The substep, s: the solver steps in this fixed time whatever the frame's
/// (a frame's time is carried over to the next in part), so its answer does
/// not depend on the frame rate (6 a frame at 60 frames a second).
pub const SUBSTEP: f32 = 1.0 / 360.0;
/// The substeps of a frame at most (MAX_DT's).
const MAX_SUBSTEPS: usize = 12;
const LINEAR_DAMPING: f32 = 0.05;
const ANGULAR_DAMPING: f32 = 0.3;
/// The joints' and limits' compliance (m/N, rad/Nm).
const JOINT_COMPLIANCE: f32 = 1e-6;
const STATIC_FRICTION: f32 = 0.9;
const KINETIC_FRICTION: f32 = 0.7;
/// A body touching the ground loses this much of its spin a second (a
/// capsule would roll on for ever otherwise).
const ROLLING_RESISTANCE: f32 = 6.0;
/// The start's speed limit, m/s, and its spin's, rad/s.
const START_SPEED: f32 = 8.0;
const START_SPIN: f32 = 20.0;
/// The muscles' relative damping: s * 0.5 of a joint's relative spin a
/// substep (at least the joints' friction).
const DRIVE_DAMPING: f32 = 0.5;
/// The bodies are at rest while no capsule end moves more than this, m,
/// from where it was when the rest began (the solver's jitter turns a
/// limb on the ground to and fro by nothing one sees: speeds would never
/// fall below the recipe's 0.05 m/s and 0.3 rad/s).
const STILL_REACH: f32 = 0.015;
/// Bodies all slower than this (m/s) are at rest even if they creep out of
/// that reach (the recipe's speed).
const CREEP: f32 = 0.05;
/// A capsule end slower than this (m/s) on a plane needs no query.
const QUERY_SPEED: f32 = 0.08;
/// A depth a body was in the level already leaves it by this much a
/// substep at most, m (about 1 m/s).
const RECOVERY: f32 = 0.006;
/// The joints' friction: the share of a joint's relative spin taken away
/// each substep at least (the drive's damping is s * 0.5, more while it
/// holds).
const JOINT_FRICTION: f32 = 0.025;
/// Every body this slow, one of them on the ground: the rest's damping, per s.
const NEAR_REST: f32 = 1.0;
const REST_DAMPING: f32 = 8.0;
/// At rest this long: settled; and settled at the latest after `MAX_AGE`.
const STILL_TIME: f32 = 0.4;
/// Every capsule end slower than SLOW (m/s) for SLOW_TIME: settled even if
/// it still drifts.
const SLOW: f32 = 0.35;
const SLOW_TIME: f32 = 1.0;
const MAX_AGE: f32 = 5.0;
/// A sweep reaches this much past a body's motion of the frame.
const SWEEP_MARGIN: f32 = 0.05;
/// The pelvis stays within this of where the player died (horizontally), so
/// the body stays by its battery and in its model's bounds.
const TRAVEL: f32 = 2.5;
/// The gun: its mass and the shape when its model gives none.
const GUN_MASS: f32 = 3.5;

/// How the player died, as the drive's fade and the impulse need it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cause {
    /// A bullet in the body (or no hit known): `part` hit along `direction`
    /// (unit), `heavy` for shotguns and sniper rifles.
    Body {
        part: Part,
        direction: Vec3,
        heavy: bool,
    },
    /// A bullet in the head along `direction`.
    Head { direction: Vec3 },
    /// A blast at `centre`.
    Blast { centre: Vec3 },
    /// Nothing known of the hit: the drive's body-shot fade, no impulse.
    Unknown,
}

/// The joint groups (their muscles' strength and decay, tuning.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Legs,
    Waist,
    Neck,
    Arms,
}

impl Group {
    fn of(kind: JointKind) -> Group {
        match kind {
            JointKind::Hip | JointKind::Knee => Group::Legs,
            JointKind::Waist => Group::Waist,
            JointKind::Neck => Group::Neck,
            JointKind::Shoulder | JointKind::Elbow => Group::Arms,
        }
    }
}

/// A joint's muscle (the powered ragdoll): a stiffness toward the target
/// pose, N m/rad, times a strength that decays from the death,
/// s0 exp(-t / tau), and is nothing below 1 %. No phases: every joint
/// weakens all the time, the legs strongest and longest; the hit side and a
/// strong hit weaken faster.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Muscle {
    pub s0: f32,
    pub tau: f32,
    pub stiffness: f32,
}

impl Muscle {
    pub fn strength(&self, age: f32) -> f32 {
        let s = self.s0 * exp_neg(age / self.tau);
        if s < 0.01 { 0.0 } else { s }
    }
}

/// The muscle of a joint for a death of `cause`: its group's numbers, the
/// hit side's decay sped up, all of them weaker the harder the hit.
pub fn muscle(cause: Cause, kind: JointKind, child: Part, power: f32) -> Muscle {
    use crate::tuning::*;
    let (stiffness, tau) = match Group::of(kind) {
        Group::Legs => (LEG_STIFFNESS, LEG_DECAY),
        Group::Waist => (WAIST_STIFFNESS, WAIST_DECAY),
        Group::Neck => (NECK_STIFFNESS, NECK_DECAY),
        Group::Arms => (ARM_STIFFNESS, ARM_DECAY),
    };
    let weaker = 1.0 - HIT_WEAKENING * clamp(power / HIT_POWER_FULL, 0.0, 1.0);
    match cause {
        Cause::Head { .. } => Muscle {
            s0: HEADSHOT_STRENGTH,
            tau: HEADSHOT_DECAY,
            stiffness,
        },
        Cause::Blast { .. } => Muscle {
            s0: BLAST_STRENGTH,
            tau: BLAST_DECAY,
            stiffness,
        },
        Cause::Body { part, .. } => {
            let hit_side = part.side().is_some() && part.side() == child.side();
            Muscle {
                s0: weaker,
                tau: if hit_side { tau * HIT_SIDE_DECAY } else { tau },
                stiffness,
            }
        }
        Cause::Unknown => Muscle {
            s0: 1.0,
            tau,
            stiffness,
        },
    }
}

/// What the level answers a body's query.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sweep {
    /// Nothing within reach.
    Clear,
    /// A surface at `point` facing `normal` (unit).
    Hit { point: Vec3, normal: Vec3 },
    /// Not asked this frame (the frame's query budget is spent): the body
    /// keeps what it had.
    Deferred,
}

/// The level as the ragdoll reads it: a sphere of `radius` moved from
/// `from` to `to`, the first surface it meets. Read-only.
pub trait Level {
    fn sweep(&mut self, from: Vec3, to: Vec3, radius: f32) -> Sweep;
}

/// What one end's query found.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Probe {
    /// A surface `distance` from the end.
    Surface {
        distance: f32,
        point: Vec3,
        normal: Vec3,
    },
    /// Nothing below or ahead of the end within reach.
    Clear,
    /// Not asked (the frame's budget is spent): what the body had stays.
    Unknown,
}

/// One end's query: along its motion of the frame with the ground below it
/// added (an end that moves sideways still finds the floor it slides on),
/// that long plus a margin.
fn probe(level: &mut impl Level, (end, motion): (Vec3, Vec3), down: Vec3, radius: f32) -> Probe {
    let reach = motion + down * (radius + SWEEP_MARGIN);
    let dir = reach.direction_or(down);
    let length = reach.length() + SWEEP_MARGIN;
    match level.sweep(end, end + dir * length, radius) {
        Sweep::Hit { point, normal } => match normal.direction() {
            Some(normal) => Probe::Surface {
                distance: (point - end).length(),
                point,
                normal,
            },
            None => Probe::Unknown,
        },
        Sweep::Clear => Probe::Clear,
        Sweep::Deferred => Probe::Unknown,
    }
}

/// The dropped gun's capsule. Its axis is the `Weapon` bone's own axis
/// nearest the body's forward at the death (the held gun points ahead).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunShape {
    /// The capsule's centre ahead of the bone along that axis, m.
    pub ahead: f32,
    pub half: f32,
    pub radius: f32,
}

impl GunShape {
    /// A rifle's size (the item's own bounds are not read yet).
    pub const RIFLE: GunShape = GunShape {
        ahead: 0.15,
        half: 0.35,
        radius: 0.04,
    };
}

/// A cosmetic render channel, present only with the headwear content installed.
/// Assets choose which surfaces use it; no inventory or simulation item is moved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadwearShape;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Headwear {
    body: Body,
    /// The Head-equivalent render bone relative to the body's centre.
    bone: Pose,
}

/// The unit axis of `frame` (one of its six) nearest `toward`.
fn nearest_axis(frame: Quat, toward: Vec3) -> Vec3 {
    let mut best = frame.rotate(Vec3::X);
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        let world = frame.rotate(axis);
        for candidate in [world, -world] {
            if candidate.dot(toward) > best.dot(toward) {
                best = candidate;
            }
        }
    }
    best
}

/// How each bone is drawn from the bodies, fixed at the death.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Link {
    /// Moved by a body: its pose is the body's times `offset`.
    Part { part: Part, offset: Pose },
    /// Moved by the dropped gun.
    Gun { offset: Pose },
    /// Kept where its parent puts it (the pose relative to the parent at the death).
    Parent { parent: Bone, local: Pose },
    /// Kept where it was at the death (the root).
    Fixed { pose: Pose },
}

/// The bones' links: how the drawn skeleton follows the bodies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rig {
    links: [Link; BONE_COUNT],
    /// The camera's bone ("Item", +44Ch's) relative to the head at the death.
    eye: Pose,
}

impl Rig {
    /// The drawn skeleton of bodies at `parts` and the gun at `gun`.
    pub fn draw(&self, parts: &PartPoses, gun: Option<Pose>, out: &mut Skeleton) {
        for i in 0..BONE_COUNT {
            out[i] = match self.links[i] {
                Link::Part { part, offset } => parts[part.index()].then(offset),
                Link::Gun { offset } => match gun {
                    Some(g) => g.then(offset),
                    None => out[ROOT.index()],
                },
                Link::Parent { parent, local } => out[parent.index()].then(local),
                Link::Fixed { pose } => pose,
            };
        }
    }

    /// The `Weapon` bone relative to the dropped gun, if a gun dropped.
    fn gun_offset(&self) -> Option<Pose> {
        match self.links[WEAPON.index()] {
            Link::Gun { offset } => Some(offset),
            _ => None,
        }
    }

    /// The camera's bone at a drawn head's pose.
    pub fn eye(&self, head: Pose) -> Pose {
        head.then(self.eye)
    }

    /// The parts' poses that their bones at `bones` give (the death clip's,
    /// as the drive's targets; the frame's before the death).
    pub fn parts_of(&self, bones: &DriverBones) -> PartPoses {
        let mut out = [Pose::IDENTITY; PARTS];
        for part in Part::ALL {
            if let Link::Part { offset, .. } = self.links[part.bone().index()] {
                out[part.index()] = bones[part.index()].then(offset.inverse());
            }
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Joint {
    parent: Part,
    child: Part,
    anchor_parent: Vec3,
    anchor_child: Vec3,
    limit: Limit,
    kind: JointKind,
    muscle: Muscle,
}

/// The two skeletons a ragdoll starts from: the last drawn and the one
/// before, `dt` s apart.
pub struct Start<'a> {
    pub now: &'a Skeleton,
    /// The parts' bones in the skeleton before.
    pub before: &'a DriverBones,
    pub dt: f32,
    /// The level's up (unit).
    pub up: Vec3,
    pub cause: Cause,
    /// The held gun's shape, if a gun is held and drops.
    pub gun: Option<GunShape>,
    /// The optional dedicated render channel supplied by the content.
    pub headwear: Option<HeadwearShape>,
    /// The ground the player stood on (a point and its unit normal): every
    /// body's plane until its first query finds its own.
    pub stood_on: Option<(Vec3, Vec3)>,
    /// The bullets that hit the living player just before the death, the
    /// killing one last: they push the body as it starts (an empty list: no
    /// push but the motion it had).
    pub hits: &'a [crate::shot::Hit],
    /// What the player wore and carried, kg on each part (its equipment).
    pub load: [f32; PARTS],
    /// How fast the player moved just before the death (its transform's
    /// motion while alive: a running body carries it; the clip's own motion
    /// after the death has none of it).
    pub velocity: Vec3,
}

/// Whether the bodies have stayed put: since when, and where they were then.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "inline in its ragdoll, no allocation"
)]
enum Rest {
    /// Some capsule end faster than SLOW.
    Moving,
    /// Every end slower than SLOW since `slow`; the bodies within
    /// STILL_REACH of `anchor` (or creeping) since `since`.
    Slow {
        slow: f32,
        since: f32,
        anchor: (PartPoses, Option<Pose>, Option<Pose>),
    },
}

/// What a ragdoll started with (the opt-in diagnostics' line of a death).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StartReport {
    /// 0 no bullet known, 1 a body shot, 2 a headshot, 3 a blast.
    pub cause: u8,
    /// The bullets that pushed it, the killing one's push (N s, the round's
    /// own), the whole body's knock (m/s).
    pub hits: u8,
    pub push: f32,
    pub knock: f32,
    /// The body's mass (kg, the kit's weight in), its speed at the death
    /// (m/s, carried).
    pub mass: f32,
    pub speed: f32,
    /// The muscles' strength at the start: legs, waist, neck, arms.
    pub strengths: [f32; 4],
    /// The fastest part after the start and its speed (m/s), and whether a
    /// hit's knock of the whole body was capped (`BODY_KNOCK_MAX`): the
    /// diagnostics' spike (464f). A part's own change is capped often (a limb
    /// shot by any rifle) and is not a spike.
    pub peak: Part,
    pub peak_speed: f32,
    pub capped: bool,
}

/// A shot's push on one part of a ragdoll (the diagnostics' spike): the
/// push (N s, the lift in), the part's mass (kg, its kit in), its speed
/// after (m/s), whether its change of speed was capped (`SHOT_MAX_SPEED`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knock {
    pub part: Part,
    pub push: f32,
    pub mass: f32,
    pub speed: f32,
    pub capped: bool,
}

/// Whether a frame's step left the ragdoll moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Moving,
    /// At rest (or out of time): freeze it.
    Settled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ragdoll {
    bodies: [Body; PARTS],
    gun: Option<Body>,
    headwear: Option<Headwear>,
    joints: [Joint; JOINTS],
    rig: Rig,
    cause: Cause,
    /// The parts' poses at the death: the muscles' target while the game
    /// draws no clip.
    at_death: PartPoses,
    up: Vec3,
    origin: Vec3,
    /// The pairs of bodies that collide ([`pair_bit`]): those that
    /// share no joint and did not overlap at the death (an arm across the
    /// chest holding the gun).
    colliding: u64,
    age: f32,
    /// The frame time not stepped yet (less than a substep).
    pending: f32,
    /// The time since it last started moving: its death, or a shot that
    /// woke it (MAX_AGE caps it).
    moving_for: f32,
    /// Woken by a shot after it settled: no drive (the clip's end pose would
    /// pull a body lying elsewhere along the ground).
    limp: bool,
    /// The time left after a shot's push before the near-rest slowing may
    /// take the push away again.
    grace: f32,
    rest: Rest,
    report: StartReport,
    /// The last poses a step left finite (what a broken step falls back to).
    last_good: (PartPoses, Option<Pose>, Option<Pose>),
}

/// The pose of a body along `from` -> `to`: its X axis the segment, its Y
/// the bone's Y made normal to it, its centre the segment's middle.
fn segment_pose(bone: Pose, from: Vec3, to: Vec3) -> (Pose, f32) {
    let bone_x = bone.rotation.rotate(Vec3::X);
    let x = (to - from).direction_or(bone_x);
    let y = bone
        .rotation
        .rotate(Vec3::Y)
        .reject(x)
        .direction()
        .unwrap_or_else(|| x.any_perpendicular());
    let z = x.cross(y);
    let length = (to - from).length();
    (
        Pose::new(Quat::from_basis(x, y, z), (from + to) * 0.5),
        length,
    )
}

fn finite_skeleton(s: &Skeleton) -> bool {
    s.iter().all(|p| p.is_finite())
}

impl Ragdoll {
    /// The ragdoll of a death; `None` when the pose is not finite (the
    /// game's clip is drawn then).
    pub fn new(start: &Start<'_>) -> Option<Ragdoll> {
        if !finite_skeleton(start.now)
            || !start.before.iter().all(|p| p.is_finite())
            || !start.up.is_finite()
        {
            return None;
        }
        let now = start.now;
        let up = start.up.direction()?;
        let at = |bone: Bone| now[bone.index()].position;
        // the body's own directions: left (the legs), forward (the toes)
        let left = (at(LEFT_UP_LEG) - at(RIGHT_UP_LEG)).reject(up).direction();
        let toes = ((at(LEFT_TOE) - at(LEFT_FOOT)) + (at(RIGHT_TOE) - at(RIGHT_FOOT))).reject(up);
        let forward = toes
            .direction()
            .or_else(|| left.map(|l| up.cross(l)))
            .unwrap_or_else(|| up.any_perpendicular());
        let left = left.unwrap_or_else(|| forward.cross(up).direction_or(up.any_perpendicular()));
        let along = |from: Bone, to: Bone, extra: f32| {
            let a = at(from);
            let b = at(to);
            (a, b + (b - a).direction_or(up) * extra)
        };
        // the segments: (from, to) of each part
        let segments: [(Vec3, Vec3); PARTS] = [
            (at(RIGHT_UP_LEG), at(LEFT_UP_LEG)),
            {
                let (a, b) = along(SPIN, NECK, 0.0);
                (a, b - (b - a).direction_or(up) * 0.08)
            },
            along(NECK, HEAD, 0.18),
            (at(LEFT_UP_LEG), at(LEFT_LEG)),
            (at(LEFT_LEG), at(LEFT_FOOT)),
            (at(RIGHT_UP_LEG), at(RIGHT_LEG)),
            (at(RIGHT_LEG), at(RIGHT_FOOT)),
            (at(LEFT_ARM), at(LEFT_FORE_ARM)),
            along(LEFT_FORE_ARM, LEFT_HAND, 0.1),
            (at(RIGHT_ARM), at(RIGHT_FORE_ARM)),
            along(RIGHT_FORE_ARM, RIGHT_HAND, 0.1),
        ];
        let mut bodies = [Body::new(
            Pose::IDENTITY,
            Capsule {
                half: 0.0,
                radius: 0.0,
            },
            1.0,
        ); PARTS];
        let mut links = [Link::Fixed {
            pose: Pose::IDENTITY,
        }; BONE_COUNT];
        for part in Part::ALL {
            let (from, to) = segments[part.index()];
            let bone = now[part.bone().index()];
            let (pose, length) = segment_pose(bone, from, to);
            let radius = part.radius();
            let half = max(0.02, length * 0.5 - radius * 0.5);
            let load = start.load[part.index()];
            let mass = part.mass()
                + if load.is_finite() {
                    clamp(load, 0.0, tuning::MAX_LOAD)
                } else {
                    0.0
                };
            bodies[part.index()] = Body::new(pose, Capsule { half, radius }, mass);
            links[part.bone().index()] = Link::Part {
                part,
                offset: pose.inverse().then(bone),
            };
        }
        // the gun
        let weapon = now[WEAPON.index()];
        let gun = start.gun.map(|shape| {
            let x = nearest_axis(weapon.rotation, forward);
            let y = x.any_perpendicular();
            let pose = Pose::new(
                Quat::from_basis(x, y, x.cross(y)),
                weapon.position + x * shape.ahead,
            );
            let capsule = Capsule {
                half: clamp(shape.half, 0.05, 0.7),
                radius: clamp(shape.radius, 0.02, 0.12),
            };
            links[WEAPON.index()] = Link::Gun {
                offset: pose.inverse().then(weapon),
            };
            Body::new(pose, capsule, GUN_MASS)
        });
        // every other bone keeps its place on its parent
        for i in 0..BONE_COUNT {
            if matches!(links[i], Link::Part { .. } | Link::Gun { .. }) {
                continue;
            }
            links[i] = match Bone(i as u8).parent() {
                Some(parent) => Link::Parent {
                    parent,
                    local: now[parent.index()].inverse().then(now[i]),
                },
                None => Link::Fixed { pose: now[i] },
            };
        }
        let rig = Rig {
            links,
            eye: now[HEAD.index()].inverse().then(now[ITEM.index()]),
        };
        // the motion the player had: the parts' poses a frame before
        let before = rig.parts_of(start.before);
        let dt = clamp(start.dt, 1.0 / 240.0, 0.25);
        for (body, was) in bodies.iter_mut().zip(before.iter()) {
            let v = (body.pose.position - was.position) * (1.0 / dt);
            let carried = if start.velocity.is_finite() {
                limit_length(start.velocity, tuning::DEATH_VELOCITY_MAX)
                    * tuning::DEATH_VELOCITY_SHARE
            } else {
                Vec3::ZERO
            };
            body.velocity = limit_length(v + carried, START_SPEED);
            body.spin = limit_length(
                body.pose.rotation.rotation_from(was.rotation) * (1.0 / dt),
                START_SPIN,
            );
        }
        // the hits: each on the part nearest its point; the killing one (the
        // last) names the death's cause when the game gave none
        let hits = start.hits;
        let part_of = |point: Vec3| {
            Part::ALL
                .into_iter()
                .map(|p| {
                    let b = &bodies[p.index()];
                    let (a, c) = b.shape.ends(b.pose);
                    let (q, _) = closest_points(a, c, point, point);
                    (p, (q - point).length() - b.shape.radius)
                })
                .fold(
                    (Part::Torso, f32::MAX),
                    |best, x| if x.1 < best.1 { x } else { best },
                )
                .0
        };
        let cause = match (start.cause, hits.last()) {
            (Cause::Unknown, Some(hit)) if hit.impulse.is_finite() && hit.point.is_finite() => {
                let direction = hit.impulse.direction_or(forward * -1.0);
                match part_of(hit.point) {
                    Part::Head => Cause::Head { direction },
                    part => Cause::Body {
                        part,
                        direction,
                        heavy: hit.impulse.length() > tuning::HEAVY_HIT,
                    },
                }
            }
            (cause, _) => cause,
        };
        let power = hits.last().map_or(0.0, |h| h.impulse.length());
        let power = if power.is_finite() { power } else { 0.0 };
        let joints = JOINT_TABLE.map(|(parent, child, bone, kind)| {
            make_joint(
                &bodies,
                parent,
                child,
                at(bone),
                kind,
                up,
                forward,
                left,
                muscle(cause, kind, child, power),
            )
        });
        let mut colliding = 0u64;
        for i in 0..PARTS {
            for k in (i + 1)..PARTS {
                if !adjacent(Part::ALL[i], Part::ALL[k])
                    && separation(&bodies[i], &bodies[k]).is_none()
                {
                    colliding |= pair_bit(i, k);
                }
            }
        }
        let mut ragdoll = Ragdoll {
            bodies,
            gun,
            headwear: start.headwear.map(|_| {
                let head = now[HEAD.index()];
                let pose = Pose::new(head.rotation, head.position + up * 0.10);
                Headwear {
                    body: Body::new(
                        pose,
                        Capsule {
                            half: 0.04,
                            radius: 0.10,
                        },
                        1.0,
                    ),
                    bone: pose.inverse().then(head),
                }
            }),
            joints,
            at_death: [Pose::IDENTITY; PARTS],
            rig,
            cause,
            up,
            origin: at(ROOT),
            colliding,
            age: 0.0,
            pending: 0.0,
            moving_for: 0.0,
            limp: false,
            grace: 0.0,
            rest: Rest::Moving,
            report: StartReport {
                cause: match cause {
                    Cause::Unknown => 0,
                    Cause::Body { .. } => 1,
                    Cause::Head { .. } => 2,
                    Cause::Blast { .. } => 3,
                },
                hits: hits.len().min(255) as u8,
                push: power,
                knock: 0.0,
                mass: 0.0,
                speed: if start.velocity.is_finite() {
                    start.velocity.length()
                } else {
                    0.0
                },
                strengths: [
                    JointKind::Knee,
                    JointKind::Waist,
                    JointKind::Neck,
                    JointKind::Elbow,
                ]
                .map(|kind| muscle(cause, kind, Part::Torso, power).s0),
                peak: Part::Torso,
                peak_speed: 0.0,
                capped: false,
            },
            last_good: ([Pose::IDENTITY; PARTS], None, None),
        };
        if let Some(gun) = ragdoll.gun.as_mut() {
            let hand = &ragdoll.bodies[Part::RightForearm.index()];
            gun.velocity = hand.point_velocity(gun.pose.position - hand.pose.position);
        }
        if let Some((point, normal)) = start.stood_on.and_then(|(p, n)| Some((p, n.direction()?)))
            && point.is_finite()
        {
            for body in ragdoll
                .bodies
                .iter_mut()
                .chain(ragdoll.gun.iter_mut())
                .chain(ragdoll.headwear.iter_mut().map(|h| &mut h.body))
            {
                body.contacts = [Contact::Plane { point, normal }; 2];
            }
        }
        ragdoll.at_death = ragdoll.poses().0;
        if hits.is_empty() {
            ragdoll.impulse();
        } else {
            // the killing hit in full, the ones just before it in part
            for (i, hit) in hits.iter().enumerate() {
                if !hit.point.is_finite() || !hit.impulse.is_finite() {
                    continue;
                }
                let weight = if i + 1 == hits.len() {
                    1.0
                } else {
                    tuning::EARLIER_HIT
                };
                let j = hit.impulse * (weight * tuning::DEATH_KNOCK);
                ragdoll.knock(part_of(hit.point), hit.point, j);
                // and the body as a whole goes with it (level: the legs
                // stay on the ground), capped
                let mass: f32 = ragdoll.bodies.iter().map(|b| 1.0 / b.inverse_mass).sum();
                let level = j.reject(up) * (tuning::BODY_SHARE / mass);
                let dv = limit_length(level, tuning::BODY_KNOCK_MAX);
                for body in ragdoll.bodies.iter_mut() {
                    body.velocity += dv;
                }
                ragdoll.report.knock += dv.length();
                ragdoll.report.capped |= level.length() > tuning::BODY_KNOCK_MAX;
            }
        }
        for part in Part::ALL {
            let speed = ragdoll.bodies[part.index()].velocity.length();
            if speed > ragdoll.report.peak_speed {
                (ragdoll.report.peak, ragdoll.report.peak_speed) = (part, speed);
            }
        }
        ragdoll.report.mass = ragdoll.bodies.iter().map(|b| 1.0 / b.inverse_mass).sum();
        if let Some(hat) = &mut ragdoll.headwear {
            let head = &ragdoll.bodies[Part::Head.index()];
            hat.body.velocity = limit_length(
                head.point_velocity(hat.body.pose.position - head.pose.position)
                    + up * 0.8
                    + left * 0.5,
                START_SPEED,
            );
            hat.body.spin = limit_length(head.spin + forward * 3.0, START_SPIN);
        }
        ragdoll.last_good = ragdoll.all_poses();
        Some(ragdoll)
    }

    /// The last hit's push (the recipe's numbers).
    fn impulse(&mut self) {
        let up = self.up;
        match self.cause {
            Cause::Body {
                part,
                direction,
                heavy,
            } => {
                let kick = if heavy { 3.5 } else { 2.0 };
                self.bodies[part.index()].velocity += direction * kick;
                let level = direction.reject(up).direction_or(Vec3::ZERO) * 0.4;
                for body in self.bodies.iter_mut() {
                    body.velocity += level;
                }
            }
            Cause::Head { direction } => {
                self.bodies[Part::Head.index()].velocity += direction * 2.75;
            }
            Cause::Blast { centre } => {
                for body in self.bodies.iter_mut() {
                    let away = body.pose.position - centre;
                    let distance = away.length();
                    let push = clamp(6.0 - distance, 3.0, 6.0);
                    body.velocity += away.direction_or(up) * push + up * 1.5;
                }
            }
            Cause::Unknown => {}
        }
    }

    /// The parts' poses and the gun's now.
    pub fn poses(&self) -> (PartPoses, Option<Pose>) {
        let mut out = [Pose::IDENTITY; PARTS];
        for (o, b) in out.iter_mut().zip(self.bodies.iter()) {
            *o = b.pose;
        }
        (out, self.gun.map(|g| g.pose))
    }

    /// The dedicated render bone, in world space, if the content supplies it.
    pub fn headwear_pose(&self) -> Option<Pose> {
        self.headwear.map(|h| h.body.pose.then(h.bone))
    }

    fn all_poses(&self) -> (PartPoses, Option<Pose>, Option<Pose>) {
        let (parts, gun) = self.poses();
        (parts, gun, self.headwear.map(|h| h.body.pose))
    }

    /// The plane the head lies on, if it touches one.
    pub fn head_ground(&self) -> Option<(Vec3, Vec3)> {
        self.bodies[Part::Head.index()]
            .contacts
            .iter()
            .find_map(|c| match *c {
                Contact::Plane { point, normal } => Some((point, normal)),
                Contact::Free => None,
            })
    }

    /// What it started with.
    pub fn report(&self) -> StartReport {
        self.report
    }

    pub fn rig(&self) -> &Rig {
        &self.rig
    }

    pub fn age(&self) -> f32 {
        self.age
    }

    /// Its dropped gun going on alone (its body went away: the player
    /// respawned), as it is now, if a gun dropped.
    pub fn lone_gun(&self) -> Option<LoneGun> {
        Some(LoneGun { body: self.gun?, weapon: self.rig.gun_offset()?, pending: self.pending, age: 0.0, rest: GunRest::Moving })
    }

    /// One frame of `dt` s (capped at 1/30): the level's queries, then six
    /// substeps. `clip` is the death clip's skeleton this frame (the drive's
    /// target), if the game drew one.
    pub fn step(&mut self, dt: f32, clip: Option<&DriverBones>, level: &mut impl Level) -> Motion {
        let Some((substeps, dt)) = substeps(&mut self.pending, dt) else {
            return Motion::Moving;
        };
        // the muscles' target: the death clip as the game draws it, else
        // the pose at the death; none for a body a shot woke (limp)
        let targets = match clip.filter(|c| c.iter().all(|p| p.is_finite())) {
            _ if self.limp => None,
            Some(c) => Some(self.rig.parts_of(c)),
            None => Some(self.at_death),
        };
        let gravity = self.up * -GRAVITY;
        self.query(dt, gravity, level);
        let h = SUBSTEP;
        let alpha_h = JOINT_COMPLIANCE / (h * h);
        let mut contacts = [[0.0f32; 2]; PARTS + 2];
        for _ in 0..substeps {
            for body in self
                .bodies
                .iter_mut()
                .chain(self.gun.iter_mut())
                .chain(self.headwear.iter_mut().map(|h| &mut h.body))
            {
                body.integrate(h, gravity, LINEAR_DAMPING, ANGULAR_DAMPING);
            }
            self.solve_joints(alpha_h);
            self.solve_drive(targets.as_ref(), h);
            for j in 0..JOINTS {
                let joint = self.joints[j];
                let (parent, child) = pair(&mut self.bodies, joint.parent, joint.child);
                joint.limit.solve(parent, child, alpha_h);
            }
            self.solve_self_collision();
            self.solve_travel();
            for (i, body) in self
                .bodies
                .iter_mut()
                .chain(self.gun.iter_mut())
                .chain(self.headwear.iter_mut().map(|h| &mut h.body))
                .enumerate()
            {
                contacts[i] = solve_contact(body);
            }
            for body in self
                .bodies
                .iter_mut()
                .chain(self.gun.iter_mut())
                .chain(self.headwear.iter_mut().map(|h| &mut h.body))
            {
                body.update_velocity(h);
            }
            for (i, body) in self
                .bodies
                .iter_mut()
                .chain(self.gun.iter_mut())
                .chain(self.headwear.iter_mut().map(|h| &mut h.body))
                .enumerate()
            {
                friction(body, contacts[i], h);
            }
            self.damp_drive();
        }
        self.age += dt;
        self.moving_for += dt;
        let (poses, gun, headwear) = self.all_poses();
        if !poses.iter().all(|p| p.is_finite())
            || !gun.is_none_or(|g| g.is_finite())
            || !headwear.is_none_or(|h| h.is_finite())
        {
            // a broken step: back to the last good poses, at rest
            self.set_poses(self.last_good);
            return Motion::Settled;
        }
        self.last_good = (poses, gun, headwear);
        // near rest on the ground, every body slowed toward rest (what is left
        // is the solver's creep, not a fall): it settles instead of crawling
        let resting = |b: &Body| {
            b.end_speed() < NEAR_REST
                && b.contacts
                    .iter()
                    .any(|c| matches!(c, Contact::Plane { .. }))
        };
        self.grace = max(0.0, self.grace - dt);
        if self.grace <= 0.0
            && self.bodies.iter().all(|b| b.end_speed() < NEAR_REST)
            && self.bodies.iter().any(resting)
        {
            let keep = exp_neg(REST_DAMPING * dt);
            for body in self
                .bodies
                .iter_mut()
                .chain(self.gun.iter_mut())
                .chain(self.headwear.iter_mut().map(|h| &mut h.body))
            {
                body.velocity = body.velocity * keep;
                body.spin = body.spin * keep;
            }
        }
        let now = self.all_poses();
        let slow = self
            .bodies
            .iter()
            .chain(self.gun.iter())
            .chain(self.headwear.iter().map(|h| &h.body))
            .all(|b| b.end_speed() < SLOW);
        self.rest = match self.rest {
            _ if !slow => Rest::Moving,
            Rest::Moving => Rest::Slow {
                slow: self.age,
                since: self.age,
                anchor: now,
            },
            // put, or creeping slower than the recipe's 0.05 m/s (the solver's
            // pushes between a limb, the ground and the other leg)
            Rest::Slow {
                slow,
                since,
                anchor,
            } if self.within(&anchor) || self.creeping() => Rest::Slow {
                slow,
                since,
                anchor,
            },
            Rest::Slow { slow, .. } => Rest::Slow {
                slow,
                since: self.age,
                anchor: now,
            },
        };
        match self.rest {
            Rest::Slow { since, .. } if self.age - since >= STILL_TIME => Motion::Settled,
            // slow for a second but still drifting: the solver's fight
            // between constraints, not a fall
            Rest::Slow { slow, .. } if self.age - slow >= SLOW_TIME => Motion::Settled,
            _ if self.moving_for >= MAX_AGE => Motion::Settled,
            _ => Motion::Moving,
        }
    }

    /// The box around every body (and the gun): a shot that misses it misses
    /// them all.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut hi = -lo;
        for body in self
            .bodies
            .iter()
            .chain(self.gun.iter())
            .chain(self.headwear.iter().map(|h| &h.body))
        {
            let (a, b) = body.shape.ends(body.pose);
            let r = body.shape.radius;
            for p in [a, b] {
                lo = Vec3::new(min(lo.x, p.x - r), min(lo.y, p.y - r), min(lo.z, p.z - r));
                hi = Vec3::new(max(hi.x, p.x + r), max(hi.y, p.y + r), max(hi.z, p.z + r));
            }
        }
        (lo, hi)
    }

    /// The nearest body part the shot meets, if any (the gun is not hit).
    pub fn trace(&self, shot: &Shot) -> Option<ShotHit> {
        let mut best: Option<ShotHit> = None;
        for part in Part::ALL {
            let body = &self.bodies[part.index()];
            let (a, b) = body.shape.ends(body.pose);
            if let Some(t) = ray_capsule(shot, a, b, body.shape.radius)
                && best.is_none_or(|h| t < h.distance)
            {
                best = Some(hit_on_capsule(shot, a, b, t, part));
            }
        }
        best
    }

    /// A hit's impulse `j` (N s) on `part` at the world `point`, the
    /// part's speed and spin changes capped (the joints carry it to the
    /// rest of the body). True if the speed's cap held it.
    fn knock(&mut self, part: Part, point: Vec3, j: Vec3) -> bool {
        let body = &mut self.bodies[part.index()];
        let r = point - body.pose.position;
        let dv = j * body.inverse_mass;
        let dw = body.inverse_inertia_times(r.cross(j));
        body.velocity += limit_length(dv, tuning::SHOT_MAX_SPEED);
        body.spin += limit_length(dw, tuning::SHOT_MAX_SPIN);
        dv.length() > tuning::SHOT_MAX_SPEED
    }

    /// A shot's push on `part`: the impulse `j` (N s) at the world `point`,
    /// the body's speed and spin changes capped. The ragdoll moves again from
    /// here (and settles again by the same rules). What it did, if anything.
    pub fn push(&mut self, part: Part, point: Vec3, j: Vec3) -> Option<Knock> {
        if !point.is_finite() || !j.is_finite() {
            return None;
        }
        let j = j + self.up * (j.length() * tuning::SHOT_LIFT);
        let capped = self.knock(part, point, j);
        self.rest = Rest::Moving;
        self.moving_for = 0.0;
        self.grace = tuning::SHOT_GRACE;
        let body = &self.bodies[part.index()];
        Some(Knock {
            part,
            push: j.length(),
            mass: 1.0 / body.inverse_mass,
            speed: body.velocity.length(),
            capped,
        })
    }

    /// A settled ragdoll made to move again: at rest where it lay, limp.
    pub fn wake(&mut self) {
        for body in self
            .bodies
            .iter_mut()
            .chain(self.gun.iter_mut())
            .chain(self.headwear.iter_mut().map(|h| &mut h.body))
        {
            body.velocity = Vec3::ZERO;
            body.spin = Vec3::ZERO;
        }
        self.limp = true;
        self.rest = Rest::Moving;
        self.moving_for = 0.0;
    }

    /// Whether every body's centre moves slower than CREEP.
    fn creeping(&self) -> bool {
        self.bodies
            .iter()
            .chain(self.gun.iter())
            .chain(self.headwear.iter().map(|h| &h.body))
            .all(|b| b.velocity.length() < CREEP)
    }

    /// Whether every capsule end (and the gun's) is within STILL_REACH of
    /// where it was at `anchor`.
    fn within(&self, (parts, gun, headwear): &(PartPoses, Option<Pose>, Option<Pose>)) -> bool {
        let near = |body: &Body, then: Pose| {
            let (a, b) = body.shape.ends(body.pose);
            let (c, d) = body.shape.ends(then);
            (a - c).length() < STILL_REACH && (b - d).length() < STILL_REACH
        };
        self.bodies
            .iter()
            .zip(parts.iter())
            .all(|(b, p)| near(b, *p))
            && match (self.gun.as_ref(), gun) {
                (Some(b), Some(p)) => near(b, *p),
                _ => true,
            }
            && match (self.headwear.as_ref(), headwear) {
                (Some(h), Some(p)) => near(&h.body, *p),
                _ => true,
            }
    }

    fn set_poses(&mut self, (poses, gun, headwear): (PartPoses, Option<Pose>, Option<Pose>)) {
        for (b, p) in self.bodies.iter_mut().zip(poses.iter()) {
            b.pose = *p;
            b.velocity = Vec3::ZERO;
            b.spin = Vec3::ZERO;
        }
        if let (Some(b), Some(p)) = (self.gun.as_mut(), gun) {
            b.pose = p;
        }
        if let (Some(h), Some(p)) = (self.headwear.as_mut(), headwear) {
            h.body.pose = p;
            h.body.velocity = Vec3::ZERO;
            h.body.spin = Vec3::ZERO;
        }
    }

    /// The level's queries: each moving capsule end asks for the surface
    /// under it, along its motion of the frame with the ground below it
    /// added (v dt + 5 cm); an end at rest on a plane keeps it. Each end's
    /// plane is its own, kept for the substeps.
    fn query(&mut self, dt: f32, gravity: Vec3, level: &mut impl Level) {
        let down = -self.up;
        for body in self
            .bodies
            .iter_mut()
            .chain(self.gun.iter_mut())
            .chain(self.headwear.iter_mut().map(|h| &mut h.body))
        {
            query_body(body, dt, gravity, down, level);
        }
    }

    fn solve_joints(&mut self, alpha_h: f32) {
        for j in 0..JOINTS {
            let joint = self.joints[j];
            let (parent, child) = pair(&mut self.bodies, joint.parent, joint.child);
            let ra = parent.pose.rotation.rotate(joint.anchor_parent);
            let rb = child.pose.rotation.rotate(joint.anchor_child);
            let gap = (child.pose.position + rb) - (parent.pose.position + ra);
            close_gap(Pair::Two(parent, child), ra, rb, gap, alpha_h);
        }
    }

    /// The muscles: each joint turned toward the target's relative pose by
    /// a compliant constraint, its stiffness the muscle's times its
    /// strength now (a weak knee bends under the body's weight, more as it
    /// weakens: it buckles by itself, no phase ends).
    fn solve_drive(&mut self, targets: Option<&PartPoses>, h: f32) {
        let Some(targets) = targets else { return };
        for j in 0..JOINTS {
            let joint = self.joints[j];
            let s = joint.muscle.strength(self.age);
            if s <= 0.0 {
                continue;
            }
            let alpha_h = 1.0 / (s * joint.muscle.stiffness * h * h);
            let (parent, child) = pair(&mut self.bodies, joint.parent, joint.child);
            let tp = targets[joint.parent.index()].rotation;
            let tc = targets[joint.child.index()].rotation;
            let mut wanted = (parent.pose.rotation * (tp.conjugate() * tc)).normalized();
            // a weakening knee cannot hold the leg straight: its target
            // bends toward flexion as its strength goes (it gives way
            // under the body by itself, more and more)
            if let (JointKind::Knee, Limit::Hinge { hinge_parent, .. }) = (joint.kind, joint.limit)
            {
                // (a limp body's, a headshot's, hardly: it has no tone to lose)
                let s0 = joint.muscle.s0.max(1e-3);
                let give = tuning::KNEE_GIVE * min(1.0, s0) * (1.0 - s / s0);
                let axis = parent.pose.rotation.rotate(hinge_parent);
                wanted = (Quat::from_axis_angle(axis, give) * wanted).normalized();
            }
            let turn = wanted.rotation_from(child.pose.rotation);
            share_turn(child, parent, turn, alpha_h);
        }
    }

    /// The muscles' relative damping: the joints' relative spin taken down
    /// by s * 0.5 a substep, at least the joints' friction.
    fn damp_drive(&mut self) {
        for j in 0..JOINTS {
            let joint = self.joints[j];
            let s = if self.limp {
                0.0
            } else {
                joint.muscle.strength(self.age)
            };
            let f = min(1.0, max(s * DRIVE_DAMPING, JOINT_FRICTION));
            let (parent, child) = pair(&mut self.bodies, joint.parent, joint.child);
            let relative = child.spin - parent.spin;
            let Some(n) = relative.direction() else {
                continue;
            };
            let w = child.turn_weight(n) + parent.turn_weight(n);
            let l = relative.length() * f / w;
            child.spin -= child.inverse_inertia_times(n * l);
            parent.spin += parent.inverse_inertia_times(n * l);
        }
    }

    /// Bodies that share no joint do not pass through each other.
    fn solve_self_collision(&mut self) {
        for i in 0..PARTS {
            for k in (i + 1)..PARTS {
                if self.colliding & pair_bit(i, k) == 0 {
                    continue;
                }
                let up = self.up;
                let (a, b) = pair(&mut self.bodies, Part::ALL[i], Part::ALL[k]);
                let Some((pa, pb, distance)) = separation(a, b) else {
                    continue;
                };
                let n = (pb - pa).direction_or(up);
                let reach = a.shape.radius + b.shape.radius;
                let depth = reach - distance;
                // the depth the substep's moves made goes out at once (it
                // stops them); what overlapped before goes out a little a
                // substep and without a speed (two limbs wedged between each
                // other and the ground were pushed apart in full each
                // substep, the ground pushed them back: they shook for ever)
                let (a0, a1) = a.shape.ends(a.previous);
                let (b0, b1) = b.shape.ends(b.previous);
                let (qa, qb) = closest_points(a0, a1, b0, b1);
                let before = max(0.0, reach - (qb - qa).length());
                let push = max(0.0, depth - before);
                let recovery = min(min(before, depth), RECOVERY);
                let ra = pa + n * a.shape.radius - a.pose.position;
                let rb = pb - n * b.shape.radius - b.pose.position;
                if push > 0.0 {
                    close_gap(Pair::Two(a, b), ra, rb, n * -push, 0.0);
                }
                if recovery > 0.0 {
                    let (was_a, was_b) = (a.pose, b.pose);
                    close_gap(Pair::Two(a, b), ra, rb, n * -recovery, 0.0);
                    carry_previous(a, was_a);
                    carry_previous(b, was_b);
                }
            }
        }
    }

    /// The pelvis stays within TRAVEL of the death's place, horizontally.
    fn solve_travel(&mut self) {
        let pelvis = &mut self.bodies[Part::Pelvis.index()];
        let away = (pelvis.pose.position - self.origin).reject(self.up);
        let distance = away.length();
        if distance > TRAVEL {
            let back = away * ((TRAVEL - distance) / distance);
            close_gap(Pair::One(pelvis), Vec3::ZERO, Vec3::ZERO, back, 0.0);
        }
    }
}

/// The two parts' bodies, mutably.
/// The frame's whole substeps of `dt` s (capped at 1/30) and their time,
/// the rest kept in `pending` for the next frame; None when there is none.
fn substeps(pending: &mut f32, dt: f32) -> Option<(usize, f32)> {
    let dt = clamp(dt, 0.0, MAX_DT);
    if dt <= 0.0 {
        return None;
    }
    *pending = min(*pending + dt, MAX_SUBSTEPS as f32 * SUBSTEP);
    let substeps = ((*pending / SUBSTEP + 1e-3) as usize).min(MAX_SUBSTEPS);
    if substeps == 0 {
        return None;
    }
    *pending = max(0.0, *pending - substeps as f32 * SUBSTEP);
    Some((substeps, substeps as f32 * SUBSTEP))
}

/// A dropped gun whose body went away (its player respawned before the
/// ragdoll settled): the same gun body, stepped alone by the ragdoll's own
/// rules (the level's queries, the contacts, friction, the near-rest
/// slowing) until it rests. The corpse never touched it (the gun meets the
/// level alone), so it falls and slides as it would have with the body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoneGun {
    body: Body,
    /// The `Weapon` bone relative to the gun (the rig's, from the death).
    weapon: Pose,
    pending: f32,
    /// Its time alone (MAX_AGE caps it).
    age: f32,
    rest: GunRest,
}

/// A lone gun's way to rest: as the ragdoll's [`Rest`], for one body.
#[derive(Clone, Copy, Debug, PartialEq)]
enum GunRest {
    Moving,
    Slow { slow: f32, since: f32, anchor: Pose },
}

impl LoneGun {
    /// One frame of `dt` s (`up` the level's): the level's query, then the substeps.
    pub fn step(&mut self, dt: f32, up: Vec3, level: &mut impl Level) -> Motion {
        let Some((substeps, dt)) = substeps(&mut self.pending, dt) else {
            return Motion::Moving;
        };
        let gravity = up * -GRAVITY;
        let before = self.body.pose;
        let body = &mut self.body;
        query_body(body, dt, gravity, -up, level);
        let h = SUBSTEP;
        for _ in 0..substeps {
            body.integrate(h, gravity, LINEAR_DAMPING, ANGULAR_DAMPING);
            let contacts = solve_contact(body);
            body.update_velocity(h);
            friction(body, contacts, h);
        }
        self.age += dt;
        if !self.body.pose.is_finite() {
            // a broken step: back to the frame's start, at rest
            self.body.pose = before;
            return Motion::Settled;
        }
        let body = &mut self.body;
        if body.end_speed() < NEAR_REST && body.contacts.iter().any(|c| matches!(c, Contact::Plane { .. })) {
            let keep = exp_neg(REST_DAMPING * dt);
            body.velocity = body.velocity * keep;
            body.spin = body.spin * keep;
        }
        let now = self.body.pose;
        let still = |then: Pose| {
            let (a, b) = self.body.shape.ends(now);
            let (c, d) = self.body.shape.ends(then);
            (a - c).length() < STILL_REACH && (b - d).length() < STILL_REACH
        };
        self.rest = match self.rest {
            _ if self.body.end_speed() >= SLOW => GunRest::Moving,
            GunRest::Moving => GunRest::Slow { slow: self.age, since: self.age, anchor: now },
            GunRest::Slow { slow, since, anchor } if still(anchor) || self.body.velocity.length() < CREEP => {
                GunRest::Slow { slow, since, anchor }
            }
            GunRest::Slow { slow, .. } => GunRest::Slow { slow, since: self.age, anchor: now },
        };
        match self.rest {
            GunRest::Slow { since, .. } if self.age - since >= STILL_TIME => Motion::Settled,
            GunRest::Slow { slow, .. } if self.age - slow >= SLOW_TIME => Motion::Settled,
            _ if self.age >= MAX_AGE => Motion::Settled,
            _ => Motion::Moving,
        }
    }

    /// The `Weapon` bone's world pose: where the game draws the held item.
    pub fn weapon(&self) -> Pose {
        self.body.pose.then(self.weapon)
    }
}

/// One body's level query (`Ragdoll::query`'s, the lone gun's): each moving
/// capsule end asks for the surface under it, along its motion of the frame
/// with the ground below it added; an end at rest on a plane keeps it.
fn query_body(body: &mut Body, dt: f32, gravity: Vec3, down: Vec3, level: &mut impl Level) {
    let (a, b) = body.shape.ends(body.pose);
    let radius = body.shape.radius;
    let mut clear = [false; 2];
    for (i, end) in [a, b].into_iter().enumerate() {
        let r = end - body.pose.position;
        let v = body.point_velocity(r);
        if v.length() < QUERY_SPEED && matches!(body.contacts[i], Contact::Plane { .. }) {
            continue;
        }
        let motion = v * dt + gravity * (dt * dt);
        match probe(level, (end, motion), down, radius) {
            Probe::Surface { point, normal, .. } => {
                body.contacts[i] = Contact::Plane { point, normal }
            }
            Probe::Clear => clear[i] = true,
            Probe::Unknown => {}
        }
    }
    // an end with nothing under it while the other end lies on a
    // plane, at that plane's height or above: the capsule lies across
    // an edge (a battery, a ledge) and that plane holds it there (an
    // end freed alone would see-saw on the edge). An end below it
    // hangs down past the edge (a step down): free, it falls to its
    // own ground (held up to the higher plane it was pushed up and
    // fell back, again and again)
    let ends = [a, b];
    for i in 0..2 {
        if clear[i] {
            body.contacts[i] = match body.contacts[1 - i] {
                plane @ Contact::Plane { point, normal }
                    if !clear[1 - i] && (ends[i] - point).dot(normal) >= 0.0 =>
                {
                    plane
                }
                _ => Contact::Free,
            };
        }
    }
}

fn pair(bodies: &mut [Body; PARTS], a: Part, b: Part) -> (&mut Body, &mut Body) {
    let (i, k) = (a.index(), b.index());
    debug_assert_ne!(i, k);
    if i < k {
        let (lo, hi) = bodies.split_at_mut(k);
        (&mut lo[i], &mut hi[0])
    } else {
        let (lo, hi) = bodies.split_at_mut(i);
        (&mut hi[0], &mut lo[k])
    }
}

/// The bit of the pair i < k of the parts (55 pairs, a triangle).
fn pair_bit(i: usize, k: usize) -> u64 {
    1 << (i * (2 * PARTS - i - 1) / 2 + (k - i - 1))
}

/// Where two capsules overlap: their segments' closest points and the
/// distance between them; `None` when they do not touch.
fn separation(a: &Body, b: &Body) -> Option<(Vec3, Vec3, f32)> {
    let (a0, a1) = a.shape.ends(a.pose);
    let (b0, b1) = b.shape.ends(b.pose);
    let (pa, pb) = closest_points(a0, a1, b0, b1);
    let distance = (pb - pa).length();
    (distance < a.shape.radius + b.shape.radius).then_some((pa, pb, distance))
}

/// The body's substep start moved as the body just moved from `was`: that
/// move makes no speed.
fn carry_previous(body: &mut Body, was: Pose) {
    let moved = body.pose.position - was.position;
    let turned = (body.pose.rotation * was.rotation.conjugate()).normalized();
    body.previous.position += moved;
    body.previous.rotation = (turned * body.previous.rotation).normalized();
}

pub(crate) fn limit_length(v: Vec3, most: f32) -> Vec3 {
    let l = v.length();
    if l > most && finite(l) {
        v * (most / l)
    } else if finite(l) {
        v
    } else {
        Vec3::ZERO
    }
}

/// The closest points of the segments p0-p1 and q0-q1.
pub fn closest_points(p0: Vec3, p1: Vec3, q0: Vec3, q1: Vec3) -> (Vec3, Vec3) {
    let d1 = p1 - p0;
    let d2 = q1 - q0;
    let r = p0 - q0;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    let (s, t);
    if a <= 1e-9 && e <= 1e-9 {
        return (p0, q0);
    }
    if a <= 1e-9 {
        s = 0.0;
        t = clamp(f / e, 0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e <= 1e-9 {
            t = 0.0;
            s = clamp(-c / a, 0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let s0 = if denom > 1e-9 {
                clamp((b * f - c * e) / denom, 0.0, 1.0)
            } else {
                0.0
            };
            let t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t = 0.0;
                s = clamp(-c / a, 0.0, 1.0);
            } else if t0 > 1.0 {
                t = 1.0;
                s = clamp((b - c) / a, 0.0, 1.0);
            } else {
                t = t0;
                s = s0;
            }
        }
    }
    (p0 + d1 * s, q0 + d2 * t)
}

/// The capsule's ends against its plane: each end pushed out, the slide of
/// the end in the substep undone while static friction holds it. Returns the
/// normal impulses (for the kinetic friction).
fn solve_contact(body: &mut Body) -> [f32; 2] {
    let mut out = [0.0; 2];
    let (pa, pb) = body.shape.ends(body.previous);
    for (i, was) in [pa, pb].into_iter().enumerate() {
        let Contact::Plane { point, normal } = body.contacts[i] else {
            continue;
        };
        // the end as it is now (the other end's push moved it too)
        let (a, b) = body.shape.ends(body.pose);
        let end = if i == 0 { a } else { b };
        let depth = body.shape.radius - (end - point).dot(normal);
        if depth <= 0.0 {
            continue;
        }
        // what the substep's move pushed in goes out at once; what was in
        // already (a plane found late) a little a substep
        let before = max(0.0, body.shape.radius - (was - point).dot(normal));
        let push = max(0.0, depth - before);
        let recovery = min(min(before, depth), RECOVERY);
        let touch = end - normal * body.shape.radius;
        let r = touch - body.pose.position;
        let lambda = close_gap(Pair::One(body), r, r, normal * push, 0.0);
        out[i] = lambda;
        if recovery > 0.0 {
            // the old depth goes out without a speed: the substep's start
            // moves with it (a push out counted as motion would throw the
            // body, more the shorter the substep)
            let was_pose = body.pose;
            let r = end - normal * body.shape.radius - body.pose.position;
            close_gap(Pair::One(body), r, r, normal * recovery, 0.0);
            carry_previous(body, was_pose);
            out[i] = max(out[i], 1e-9);
        }
        // static friction: the end's slide along the plane in this substep
        let slide = (end - was).reject(normal);
        if slide.length() < STATIC_FRICTION * depth {
            let r = touch - body.pose.position;
            close_gap(Pair::One(body), r, r, -slide, 0.0);
        }
    }
    out
}

/// The kinetic friction and no bounce, at the ends that touched, and the
/// rolling resistance of a body on the ground.
fn friction(body: &mut Body, lambdas: [f32; 2], h: f32) {
    if lambdas[0] > 0.0 || lambdas[1] > 0.0 {
        body.spin = body.spin * (1.0 - min(1.0, ROLLING_RESISTANCE * h));
    }
    let (a, b) = body.shape.ends(body.pose);
    for (i, (end, lambda)) in [(a, lambdas[0]), (b, lambdas[1])].into_iter().enumerate() {
        let Contact::Plane { normal, .. } = body.contacts[i] else {
            continue;
        };
        if lambda <= 0.0 {
            continue;
        }
        let r = end - normal * body.shape.radius - body.pose.position;
        let v = body.point_velocity(r);
        let vn = v.dot(normal);
        let vt = v - normal * vn;
        let mut dv = Vec3::ZERO;
        if let Some(t) = vt.direction() {
            let normal_force = lambda / (h * h);
            dv -= t * min(h * KINETIC_FRICTION * normal_force, vt.length());
        }
        if vn < 0.0 {
            dv -= normal * vn;
        }
        if let Some(n) = dv.direction() {
            let w = body.push_weight(r, n);
            body.apply_impulse(r, n * (dv.length() / w));
        }
    }
}

/// The joint between `parent` and `child` at the world point `at`, its limit
/// set up from the death's pose (`up`, `forward`, `left`: the body's own
/// directions then).
#[allow(clippy::too_many_arguments)]
fn make_joint(
    bodies: &[Body; PARTS],
    parent: Part,
    child: Part,
    at: Vec3,
    kind: JointKind,
    up: Vec3,
    forward: Vec3,
    left: Vec3,
    muscle: Muscle,
) -> Joint {
    let p = &bodies[parent.index()];
    let c = &bodies[child.index()];
    let into_p = |v: Vec3| p.pose.rotation.conjugate().rotate(v);
    let into_c = |v: Vec3| c.pose.rotation.conjugate().rotate(v);
    // the limb directions: from the joint out along each body's capsule
    let child_axis = c.pose.rotation.rotate(Vec3::X);
    let child_out = if (c.pose.position - at).dot(child_axis) >= 0.0 {
        child_axis
    } else {
        -child_axis
    };
    let parent_axis = p.pose.rotation.rotate(Vec3::X);
    let parent_in = if (at - p.pose.position).dot(parent_axis) >= 0.0 {
        parent_axis
    } else {
        -parent_axis
    };
    let side = if (c.pose.position - p.pose.position).dot(left) >= 0.0 {
        left
    } else {
        -left
    };
    let swing = |cone: Cone, centre: Vec3, twist: f32| {
        let centre = centre.direction_or(child_out);
        let reference = forward
            .reject(centre)
            .direction_or(centre.any_perpendicular());
        let carried = crate::limits::swing_carry(reference, centre, child_out);
        Limit::Swing {
            cone,
            axis_parent: into_p(centre),
            forward_parent: into_p(forward.reject(centre).direction_or(reference)),
            axis_child: into_c(child_out),
            reference_parent: into_p(reference),
            reference_child: into_c(
                carried
                    .reject(child_out)
                    .direction_or(child_out.any_perpendicular()),
            ),
            twist,
        }
    };
    let hinge = |default: Vec3, min: f32, max: f32| {
        let bent = parent_in.cross(child_out);
        let axis = if bent.length() > 0.26 {
            let b = bent.direction_or(default);
            // a knee bent the wrong way at the death keeps the anatomical axis
            if b.dot(default) >= 0.0 || kind == JointKind::Elbow {
                b
            } else {
                default
            }
        } else {
            default
        };
        let axis = axis.reject(parent_in).direction_or(default);
        Limit::Hinge {
            hinge_parent: into_p(axis),
            hinge_child: into_c(axis),
            limb_parent: into_p(parent_in),
            limb_child: into_c(child_out),
            min,
            max,
        }
    };
    let limit = match kind {
        JointKind::Waist => swing(Cone::Round(degrees(35.0)), up, degrees(25.0)),
        JointKind::Neck => swing(Cone::Round(degrees(45.0)), parent_in, degrees(50.0)),
        JointKind::Hip => swing(
            Cone::Shaped {
                forward: degrees(100.0),
                back: degrees(30.0),
                side: degrees(45.0),
            },
            -up,
            degrees(30.0),
        ),
        // the knee bends the shin back: about up x forward
        JointKind::Knee => hinge(
            up.cross(forward).direction_or(left),
            degrees(-3.0),
            degrees(140.0),
        ),
        JointKind::Shoulder => swing(
            Cone::Round(degrees(110.0)),
            side * 0.7 - up * 0.7,
            degrees(70.0),
        ),
        // the elbow bends the forearm forward: about forward x up
        JointKind::Elbow => hinge(forward.cross(up).direction_or(-left), 0.0, degrees(145.0)),
    };
    Joint {
        parent,
        child,
        anchor_parent: into_p(at - p.pose.position),
        anchor_child: into_c(at - c.pose.position),
        limit,
        kind,
        muscle,
    }
}

#[cfg(test)]
pub(crate) mod rudnya_floor;
#[cfg(test)]
pub(crate) mod scenes;
#[cfg(test)]
pub(crate) mod tests;
