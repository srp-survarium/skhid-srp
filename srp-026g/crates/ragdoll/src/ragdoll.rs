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
pub const SUBSTEPS: usize = 6;
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
/// The drive's stiffness: a fraction 1 - exp(-h * 40 * s) of the way to the
/// clip a substep; its relative damping s * 0.5.
const DRIVE_RATE: f32 = 40.0;
const DRIVE_DAMPING: f32 = 0.5;
/// The stance's turn of a hip or a knee a substep at most, rad (a hard
/// landing does not throw the legs back into the stance at once).
const STANCE_TURN: f32 = 0.05;
/// The bodies are at rest while no capsule end moves more than this, m,
/// from where it was when the rest began (the solver's jitter turns a
/// limb on the ground to and fro by nothing one sees: speeds would never
/// fall below the recipe's 0.05 m/s and 0.3 rad/s).
const STILL_REACH: f32 = 0.015;
/// Bodies all slower than this (m/s) are at rest even if they creep out of
/// that reach (the recipe's speed).
const CREEP: f32 = 0.05;
/// A body slower than this (its ends, m/s) with a plane needs no query;
/// one faster than `FAST` has both its ends queried.
const QUERY_SPEED: f32 = 0.08;
const FAST: f32 = 1.5;
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

/// The joint groups whose drive fades together.
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

fn smoothstep(edge0: f32, edge1: f32, t: f32) -> f32 {
    let u = clamp((t - edge0) / (edge1 - edge0), 0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// The drive's strength of a joint group `age` s after the death:
/// s0 (1 - smoothstep(t0, t0 + T, t)), floored at s_dead.
pub fn strength(cause: Cause, group: Group, age: f32) -> f32 {
    let (s0, s_dead, t0, length) = match cause {
        Cause::Head { .. } => (1.0, 0.0, 0.0, 0.06),
        Cause::Blast { .. } => (0.4, 0.03, 0.0, 0.15),
        // a body shot: the legs hold longest (they try to carry the body,
        // then give way), the trunk and the arms go first (tuning.rs)
        Cause::Body { .. } | Cause::Unknown => {
            let (t0, length) = match group {
                Group::Legs => (tuning::LEG_HOLD, tuning::LEG_FADE),
                Group::Waist => (0.0, tuning::TRUNK_FADE),
                Group::Neck => (0.05, tuning::TRUNK_FADE),
                Group::Arms => (0.0, tuning::ARM_FADE),
            };
            (1.0, 0.03, t0, length)
        }
    };
    max(s_dead, s0 * (1.0 - smoothstep(t0, t0 + length, age)))
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

impl Probe {
    /// Two ends' answers as one: the nearer surface; clear only if both are.
    fn or(self, other: Probe) -> Probe {
        match (self, other) {
            (Probe::Surface { distance: d1, .. }, Probe::Surface { distance: d2, .. }) => {
                if d2 < d1 { other } else { self }
            }
            (Probe::Surface { .. }, _) => self,
            (_, Probe::Surface { .. }) => other,
            (Probe::Clear, Probe::Clear) => Probe::Clear,
            _ => Probe::Unknown,
        }
    }
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
    group: Group,
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
    /// The ground the player stood on (a point and its unit normal): every
    /// body's plane until its first query finds its own.
    pub stood_on: Option<(Vec3, Vec3)>,
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
        anchor: (PartPoses, Option<Pose>),
    },
}

/// Where a body shot's legs put their feet (Ragdoll::support).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Support {
    /// The point on the feet's level under the centre of mass.
    below_centre: Vec3,
    /// The feet's middle.
    feet: Vec3,
    /// The body's left, level.
    lateral: Vec3,
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
    joints: [Joint; JOINTS],
    rig: Rig,
    cause: Cause,
    up: Vec3,
    origin: Vec3,
    /// The pairs of bodies that collide ([`pair_bit`]): those that
    /// share no joint and did not overlap at the death (an arm across the
    /// chest holding the gun).
    colliding: u64,
    age: f32,
    rest: Rest,
    /// The last poses a step left finite (what a broken step falls back to).
    last_good: (PartPoses, Option<Pose>),
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
            bodies[part.index()] = Body::new(pose, Capsule { half, radius }, part.mass());
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
            body.velocity = limit_length(v, START_SPEED);
            body.spin = limit_length(
                body.pose.rotation.rotation_from(was.rotation) * (1.0 / dt),
                START_SPIN,
            );
        }
        let joints = JOINT_TABLE.map(|(parent, child, bone, kind)| {
            make_joint(&bodies, parent, child, at(bone), kind, up, forward, left)
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
            joints,
            rig,
            cause: start.cause,
            up,
            origin: at(ROOT),
            colliding,
            age: 0.0,
            rest: Rest::Moving,
            last_good: ([Pose::IDENTITY; PARTS], None),
        };
        if let Some(gun) = ragdoll.gun.as_mut() {
            let hand = &ragdoll.bodies[Part::RightForearm.index()];
            gun.velocity = hand.point_velocity(gun.pose.position - hand.pose.position);
        }
        if let Some((point, normal)) = start.stood_on.and_then(|(p, n)| Some((p, n.direction()?)))
            && point.is_finite()
        {
            for body in ragdoll.bodies.iter_mut().chain(ragdoll.gun.iter_mut()) {
                body.contact = Contact::Plane { point, normal };
            }
        }
        ragdoll.impulse();
        ragdoll.last_good = ragdoll.poses();
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

    /// The plane the head lies on, if it touches one.
    pub fn head_ground(&self) -> Option<(Vec3, Vec3)> {
        match self.bodies[Part::Head.index()].contact {
            Contact::Plane { point, normal } => Some((point, normal)),
            Contact::Free => None,
        }
    }

    pub fn rig(&self) -> &Rig {
        &self.rig
    }

    pub fn age(&self) -> f32 {
        self.age
    }

    /// One frame of `dt` s (capped at 1/30): the level's queries, then six
    /// substeps. `clip` is the death clip's skeleton this frame (the drive's
    /// target), if the game drew one.
    pub fn step(&mut self, dt: f32, clip: Option<&DriverBones>, level: &mut impl Level) -> Motion {
        let dt = clamp(dt, 0.0, MAX_DT);
        if dt <= 0.0 {
            return Motion::Moving;
        }
        let targets = clip
            .filter(|c| c.iter().all(|p| p.is_finite()))
            .map(|c| self.rig.parts_of(c));
        let gravity = self.up * -GRAVITY;
        self.query(dt, gravity, level);
        let h = dt / SUBSTEPS as f32;
        let alpha_h = JOINT_COMPLIANCE / (h * h);
        let mut contacts = [[0.0f32; 2]; PARTS + 1];
        for _ in 0..SUBSTEPS {
            for body in self.bodies.iter_mut().chain(self.gun.iter_mut()) {
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
                .enumerate()
            {
                contacts[i] = solve_contact(body);
            }
            for body in self.bodies.iter_mut().chain(self.gun.iter_mut()) {
                body.update_velocity(h);
            }
            for (i, body) in self
                .bodies
                .iter_mut()
                .chain(self.gun.iter_mut())
                .enumerate()
            {
                friction(body, contacts[i], h);
            }
            self.damp_drive(h);
        }
        self.age += dt;
        let (poses, gun) = self.poses();
        if !poses.iter().all(|p| p.is_finite()) || !gun.is_none_or(|g| g.is_finite()) {
            // a broken step: back to the last good poses, at rest
            self.set_poses(self.last_good);
            return Motion::Settled;
        }
        self.last_good = (poses, gun);
        // near rest on the ground, every body slowed toward rest (what is left
        // is the solver's creep, not a fall): it settles instead of crawling
        let resting =
            |b: &Body| b.end_speed() < NEAR_REST && matches!(b.contact, Contact::Plane { .. });
        if self.bodies.iter().all(|b| b.end_speed() < NEAR_REST) && self.bodies.iter().any(resting)
        {
            let keep = exp_neg(REST_DAMPING * dt);
            for body in self.bodies.iter_mut().chain(self.gun.iter_mut()) {
                body.velocity = body.velocity * keep;
                body.spin = body.spin * keep;
            }
        }
        let now = self.poses();
        let slow = self
            .bodies
            .iter()
            .chain(self.gun.iter())
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
            _ if self.age >= MAX_AGE => Motion::Settled,
            _ => Motion::Moving,
        }
    }

    /// Whether every body's centre moves slower than CREEP.
    fn creeping(&self) -> bool {
        self.bodies
            .iter()
            .chain(self.gun.iter())
            .all(|b| b.velocity.length() < CREEP)
    }

    /// Whether every capsule end (and the gun's) is within STILL_REACH of
    /// where it was at `anchor`.
    fn within(&self, (parts, gun): &(PartPoses, Option<Pose>)) -> bool {
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
    }

    fn set_poses(&mut self, (poses, gun): (PartPoses, Option<Pose>)) {
        for (b, p) in self.bodies.iter_mut().zip(poses.iter()) {
            b.pose = *p;
            b.velocity = Vec3::ZERO;
            b.spin = Vec3::ZERO;
        }
        if let (Some(b), Some(p)) = (self.gun.as_mut(), gun) {
            b.pose = p;
        }
    }

    /// One swept query per moving body: its leading end along the frame's
    /// motion (or down, when it hardly moves), v dt + 5 cm. The plane found
    /// is kept for the substeps.
    fn query(&mut self, dt: f32, gravity: Vec3, level: &mut impl Level) {
        let down = -self.up;
        for body in self.bodies.iter_mut().chain(self.gun.iter_mut()) {
            let moving = body.end_speed() >= QUERY_SPEED;
            if !moving && matches!(body.contact, Contact::Plane { .. }) {
                continue;
            }
            let (a, b) = body.shape.ends(body.pose);
            let motion_of = |end: Vec3| {
                body.point_velocity(end - body.pose.position) * dt + gravity * (dt * dt)
            };
            let (ma, mb) = (motion_of(a), motion_of(b));
            // the end ahead of the other along its motion first; both ends of a
            // fast body (a spinning one may land on the other first)
            let a_leads = (a - b).dot(ma.direction_or(down)) >= (b - a).dot(mb.direction_or(down));
            let (first, second) = if a_leads {
                ((a, ma), (b, mb))
            } else {
                ((b, mb), (a, ma))
            };
            let radius = body.shape.radius;
            let mut answer = probe(level, first, down, radius);
            if body.end_speed() >= FAST || answer == Probe::Clear {
                // the other end too: the nearer surface of two; a plane is
                // dropped only when neither end has ground under it (a body
                // half over a ledge still lies on it)
                answer = answer.or(probe(level, second, down, radius));
            }
            match answer {
                Probe::Surface { point, normal, .. } => {
                    body.contact = Contact::Plane { point, normal }
                }
                Probe::Clear => body.contact = Contact::Free,
                Probe::Unknown => {}
            }
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

    /// The drive: each joint turned toward the clip's relative pose by a
    /// fraction of the way, its strength fading per group.
    fn solve_drive(&mut self, targets: Option<&PartPoses>, h: f32) {
        // a body shot's legs drive toward a supporting stance, not the clip
        let support = match self.cause {
            // only while the legs carry it: after their fade the floor's weak drive
            // follows the clip, as every other joint's (a body lying down would
            // keep pushing toward a stance)
            Cause::Body { .. } | Cause::Unknown
                if self.age < tuning::LEG_HOLD + tuning::LEG_FADE =>
            {
                Some(self.support())
            }
            Cause::Body { .. } | Cause::Unknown => None,
            Cause::Head { .. } | Cause::Blast { .. } => None,
        };
        for j in 0..JOINTS {
            let joint = self.joints[j];
            let s = strength(self.cause, joint.group, self.age);
            if s <= 0.0 {
                continue;
            }
            let k = 1.0 - exp_neg(h * DRIVE_RATE * s);
            let up = self.up;
            let (parent, child) = pair(&mut self.bodies, joint.parent, joint.child);
            match (support, joint.kind, joint.limit) {
                (
                    Some(_),
                    JointKind::Knee,
                    Limit::Hinge {
                        hinge_parent,
                        limb_parent,
                        limb_child,
                        ..
                    },
                ) => {
                    // the knee toward the stance's bend
                    let h_axis = parent.pose.rotation.rotate(hinge_parent);
                    let p = parent
                        .pose
                        .rotation
                        .rotate(limb_parent)
                        .reject(h_axis)
                        .direction();
                    let c = child
                        .pose
                        .rotation
                        .rotate(limb_child)
                        .reject(h_axis)
                        .direction();
                    if let (Some(p), Some(c)) = (p, c) {
                        let phi = crate::limits::signed_angle(p, c, h_axis);
                        let turn = clamp((tuning::KNEE_BEND - phi) * k, -STANCE_TURN, STANCE_TURN);
                        share_turn(child, parent, h_axis * turn, 0.0);
                    }
                }
                (Some(support), JointKind::Hip, Limit::Swing { axis_child, .. }) => {
                    // the thigh toward its foot's place: under the hip, moved
                    // toward under the centre of mass (the balance assist)
                    let hip = parent.pose.apply(joint.anchor_parent);
                    let side = if (hip - parent.pose.position).dot(support.lateral) >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    let below_hip = hip - up * (hip - support.feet).dot(up);
                    let under =
                        support.below_centre + support.lateral * (side * tuning::STANCE_HALF_WIDTH);
                    let foot = below_hip + (under - below_hip) * tuning::BALANCE_ASSIST;
                    let current = child.pose.rotation.rotate(axis_child);
                    if let Some(wanted) = (foot - hip).direction() {
                        let angle = crate::limits::angle_between(current, wanted);
                        if let Some(axis) = current.cross(wanted).direction() {
                            share_turn(child, parent, axis * min(angle * k, STANCE_TURN), 0.0);
                        }
                    }
                }
                _ => {
                    let Some(targets) = targets else { continue };
                    let tp = targets[joint.parent.index()].rotation;
                    let tc = targets[joint.child.index()].rotation;
                    let wanted = (parent.pose.rotation * (tp.conjugate() * tc)).normalized();
                    let turn = wanted.rotation_from(child.pose.rotation) * k;
                    share_turn(child, parent, turn, 0.0);
                }
            }
        }
    }

    /// Where the body's weight is over its feet: the point on the feet's
    /// level under the centre of mass, the feet's middle, and the body's
    /// left (the pelvis's axis, right hip to left).
    fn support(&self) -> Support {
        let mut mass = 0.0;
        let mut centre = Vec3::ZERO;
        for body in self.bodies.iter() {
            let m = 1.0 / body.inverse_mass;
            mass += m;
            centre += body.pose.position * m;
        }
        let centre = centre * (1.0 / mass);
        let mut feet = Vec3::ZERO;
        for (thigh, shin) in [
            (Part::LeftThigh, Part::LeftShin),
            (Part::RightThigh, Part::RightShin),
        ] {
            let shin_body = &self.bodies[shin.index()];
            let at_knee = self
                .joints
                .iter()
                .find(|j| j.parent == thigh && j.child == shin)
                .map(|j| j.anchor_child);
            let toward_foot = match at_knee {
                Some(a) if a.x > 0.0 => -1.0,
                _ => 1.0,
            };
            feet += shin_body
                .pose
                .apply(Vec3::X * (toward_foot * shin_body.shape.half))
                * 0.5;
        }
        let below_centre = centre - self.up * (centre - feet).dot(self.up);
        let pelvis = &self.bodies[Part::Pelvis.index()];
        let lateral = pelvis
            .pose
            .rotation
            .rotate(Vec3::X)
            .reject(self.up)
            .direction_or(Vec3::X);
        Support {
            below_centre,
            feet,
            lateral,
        }
    }

    /// The drive's relative damping: the joints' relative spin taken down by
    /// s * 0.5 a substep.
    fn damp_drive(&mut self, _h: f32) {
        for j in 0..JOINTS {
            let joint = self.joints[j];
            let s = strength(self.cause, joint.group, self.age);
            let f = min(1.0, max(s * DRIVE_DAMPING, JOINT_FRICTION));
            if f <= 0.0 {
                continue;
            }
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
                let ra = pa + n * a.shape.radius - a.pose.position;
                let rb = pb - n * b.shape.radius - b.pose.position;
                // push them apart: close the (negative) gap between the surfaces
                close_gap(Pair::Two(a, b), ra, rb, n * (distance - reach), 0.0);
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

fn limit_length(v: Vec3, most: f32) -> Vec3 {
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
    let Contact::Plane { point, normal } = body.contact else {
        return [0.0; 2];
    };
    let mut out = [0.0; 2];
    let ends = |pose: Pose| body.shape.ends(pose);
    let (a, b) = ends(body.pose);
    let (pa, pb) = ends(body.previous);
    for (i, (end, was)) in [(a, pa), (b, pb)].into_iter().enumerate() {
        let depth = body.shape.radius - (end - point).dot(normal);
        if depth <= 0.0 {
            continue;
        }
        // what the substep's move pushed in goes out at once; what was in
        // already (a plane found late) a little a substep: pushed out at once
        // it would throw the body
        let before = max(0.0, body.shape.radius - (was - point).dot(normal));
        let push = max(0.0, depth - before) + min(min(before, depth), RECOVERY);
        let touch = end - normal * body.shape.radius;
        let r = touch - body.pose.position;
        let lambda = close_gap(Pair::One(body), r, r, normal * push, 0.0);
        out[i] = lambda;
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
    let Contact::Plane { normal, .. } = body.contact else {
        return;
    };
    if lambdas[0] > 0.0 || lambdas[1] > 0.0 {
        body.spin = body.spin * (1.0 - min(1.0, ROLLING_RESISTANCE * h));
    }
    let (a, b) = body.shape.ends(body.pose);
    for (end, lambda) in [(a, lambdas[0]), (b, lambdas[1])] {
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
        group: Group::of(kind),
    }
}

#[cfg(test)]
pub(crate) mod tests;
