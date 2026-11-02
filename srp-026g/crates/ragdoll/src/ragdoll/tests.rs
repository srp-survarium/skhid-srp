//! The solver's tests: a made-up standing body on flat ground (Y up).

use super::*;
extern crate std;
use crate::limits::{angle_between, signed_angle};
use crate::skeleton::{BONES, Bone};

/// A plane at height `floor` (and, if set, a wall at z = `wall` facing -Z).
pub(crate) struct Ground {
    pub floor: f32,
    pub wall: Option<f32>,
    pub queries: usize,
}

impl Ground {
    pub(crate) fn flat() -> Ground {
        Ground {
            floor: 0.0,
            wall: None,
            queries: 0,
        }
    }
}

impl Level for Ground {
    fn sweep(&mut self, from: Vec3, to: Vec3, radius: f32) -> Sweep {
        self.queries += 1;
        let mut best: Option<(f32, Vec3, Vec3)> = None;
        // the floor: the sphere's bottom reaches it
        let (a, b) = (from.y - radius - self.floor, to.y - radius - self.floor);
        if a <= 0.0 || b < 0.0 {
            let t = if a <= 0.0 { 0.0 } else { a / (a - b) };
            let p = from.lerp(to, t);
            best = Some((t, Vec3::new(p.x, self.floor, p.z), Vec3::Y));
        }
        if let Some(wall) = self.wall {
            let (a, b) = (wall - (from.z + radius), wall - (to.z + radius));
            if a <= 0.0 || b < 0.0 {
                let t = if a <= 0.0 { 0.0 } else { a / (a - b) };
                if best.is_none_or(|(bt, ..)| t < bt) {
                    let p = from.lerp(to, t);
                    best = Some((t, Vec3::new(p.x, p.y, wall), -Vec3::Z));
                }
            }
        }
        match best {
            Some((_, point, normal)) => Sweep::Hit { point, normal },
            None => Sweep::Clear,
        }
    }
}

fn at(name: &str) -> Vec3 {
    let (x, y, z) = match name {
        "Root" => (0.0, 0.0, 0.0),
        "Hip" => (0.0, 1.0, 0.0),
        "LeftUpLeg" => (0.1, 0.95, 0.0),
        "LeftKnee" => (0.1, 0.53, 0.0),
        "LeftLeg" => (0.1, 0.52, 0.0),
        "LeftFoot" => (0.1, 0.09, 0.0),
        "LeftToe" => (0.1, 0.03, 0.14),
        "Spin" => (0.0, 1.1, 0.0),
        "Spin_1" => (0.0, 1.22, 0.0),
        "Chest" => (0.0, 1.34, 0.0),
        "LeftClavicle" => (0.04, 1.48, 0.0),
        "LeftArm" => (0.19, 1.45, 0.0),
        "LeftForeArm" => (0.21, 1.17, 0.03),
        "LeftHand" => (0.22, 0.93, 0.06),
        "Neck" => (0.0, 1.53, 0.0),
        "Head" => (0.0, 1.62, 0.0),
        "Weapon" => (-0.2, 1.0, 0.3),
        "Item" => (0.0, 1.64, 0.08),
        _ => return Vec3::new(f32::NAN, 0.0, 0.0),
    };
    Vec3::new(x, y, z)
}

/// The made-up skeleton: every bone at its place (Right mirrors Left; a
/// bone not listed sits 2 cm from its parent), every rotation `turn`.
pub(crate) fn standing(turn: Quat, shift: Vec3) -> Skeleton {
    let mut out = [Pose::IDENTITY; BONE_COUNT];
    for (i, (name, parent)) in BONES.iter().enumerate() {
        let mirrored = name.strip_prefix("Right").map(|rest| {
            let p = at(&["Left", rest].concat());
            Vec3::new(-p.x, p.y, p.z)
        });
        let p = mirrored.unwrap_or_else(|| at(name));
        let p = if p.is_finite() {
            p
        } else {
            out[parent.expect("the root is listed") as usize].position + Vec3::new(0.0, 0.0, 0.02)
        };
        out[i] = Pose::new(turn, p);
    }
    for pose in out.iter_mut() {
        pose.position = turn.rotate(pose.position) + shift;
    }
    out
}

fn start<'a>(
    now: &'a Skeleton,
    before: &'a DriverBones,
    cause: Cause,
    gun: Option<GunShape>,
) -> Start<'a> {
    Start {
        now,
        before,
        dt: 1.0 / 60.0,
        up: Vec3::Y,
        cause,
        gun,
        headwear: None,
        stood_on: Some((Vec3::ZERO, Vec3::Y)),
        hits: &[],
        load: [0.0; PARTS],
        velocity: Vec3::ZERO,
    }
}

const SHOT: Cause = Cause::Body {
    part: Part::Torso,
    direction: Vec3::new(0.0, 0.0, -1.0),
    heavy: false,
};
const GUN: GunShape = GunShape::RIFLE;

#[test]
fn headwear_starts_on_the_head_then_falls_separately_and_settles_above_ground() {
    let now = standing(Quat::IDENTITY, Vec3::ZERO);
    let before = driver_bones(&now);
    let mut input = start(&now, &before, Cause::Unknown, None);
    assert!(Ragdoll::new(&input).unwrap().headwear_pose().is_none());
    input.headwear = Some(HeadwearShape);
    let mut body = Ragdoll::new(&input).unwrap();
    assert!((body.headwear_pose().unwrap().position - now[HEAD.index()].position).length() < 1e-5);
    let mut ground = Ground::flat();
    run(&mut body, &mut ground, None, |r| {
        let hat = r.headwear.unwrap();
        assert!(hat.body.pose.is_finite());
        for end in [
            hat.body.shape.ends(hat.body.pose).0,
            hat.body.shape.ends(hat.body.pose).1,
        ] {
            assert!(
                end.y >= hat.body.shape.radius - 0.02,
                "helmet penetrated the floor"
            );
        }
    });
    let mut drawn = now;
    let (parts, gun) = body.poses();
    body.rig().draw(&parts, gun, &mut drawn);
    assert!(
        (body.headwear_pose().unwrap().position - drawn[HEAD.index()].position).length() > 0.15
    );
    assert!(body.headwear.unwrap().body.pose.position.y < 0.2);
}

#[test]
fn a_headshot_body_stays_down_while_its_muscles_fade() {
    // The headshot's muscles fade fastest; a body still creeping when their
    // strength's exponent passed 50 was flung metres up (exp_neg grew there).
    let hits = [looks5()[2].1];
    let now = standing(Quat::IDENTITY, Vec3::ZERO);
    let before = driver_bones(&now);
    let mut input = start(&now, &before, Cause::Unknown, Some(GUN));
    input.hits = &hits;
    let mut body = Ragdoll::new(&input).unwrap();
    let mut ground = Ground::flat();
    let mut t = 0.0;
    while t < 8.0 {
        if body.step(1.0 / 60.0, None, &mut ground) == Motion::Settled {
            break;
        }
        t += 1.0 / 60.0;
        if t > 1.5 {
            let top = body
                .bodies
                .iter()
                .map(|b| b.pose.position.y)
                .fold(0.0, f32::max);
            assert!(top < 0.6, "a part {top:.2} m up at {t:.2} s");
        }
    }
}

/// Runs at 60 frames a second until settled; the seconds it took.
fn run(
    ragdoll: &mut Ragdoll,
    ground: &mut Ground,
    clip: Option<&DriverBones>,
    mut each: impl FnMut(&Ragdoll),
) -> f32 {
    let mut t = 0.0;
    loop {
        let motion = ragdoll.step(1.0 / 60.0, clip, ground);
        t += 1.0 / 60.0;
        each(ragdoll);
        if motion == Motion::Settled {
            return t;
        }
        assert!(t < 10.0, "never settled");
    }
}

fn joint_gaps(r: &Ragdoll) -> f32 {
    r.joints
        .iter()
        .map(|j| {
            let p = &r.bodies[j.parent.index()];
            let c = &r.bodies[j.child.index()];
            (p.pose.apply(j.anchor_parent) - c.pose.apply(j.anchor_child)).length()
        })
        .fold(0.0, max)
}

#[test]
fn the_rig_draws_the_death_pose_back_and_parts_of_inverts_it() {
    let now = standing(
        Quat::from_axis_angle(Vec3::Y, 0.7),
        Vec3::new(3.0, 2.0, -1.0),
    );
    let before = driver_bones(&now);
    let ragdoll = Ragdoll::new(&start(&now, &before, Cause::Unknown, Some(GUN))).unwrap();
    let (parts, gun) = ragdoll.poses();
    let mut drawn = [Pose::IDENTITY; BONE_COUNT];
    ragdoll.rig().draw(&parts, gun, &mut drawn);
    for i in 0..BONE_COUNT {
        assert!(
            (drawn[i].position - now[i].position).length() < 1e-4,
            "{}",
            Bone(i as u8).name()
        );
        assert!(
            drawn[i].rotation.dot(now[i].rotation).abs() > 0.9999,
            "{}",
            Bone(i as u8).name()
        );
    }
    let back = ragdoll.rig().parts_of(&driver_bones(&now));
    for (a, b) in back.iter().zip(parts.iter()) {
        assert!((a.position - b.position).length() < 1e-4);
    }
    assert!(joint_gaps(&ragdoll) < 1e-4);
    // the start's motion: none, the poses being the same
    assert!(
        ragdoll
            .bodies
            .iter()
            .all(|b| b.velocity.length() < 1e-3 && b.spin.length() < 1e-3)
    );
}

#[test]
fn a_shot_body_falls_and_settles_on_flat_ground() {
    let mut times = std::vec::Vec::new();
    for (turn, cause) in [
        (0.0, SHOT),
        (1.3, SHOT),
        (
            2.5,
            Cause::Body {
                part: Part::LeftThigh,
                direction: Vec3::X,
                heavy: true,
            },
        ),
        (
            0.4,
            Cause::Head {
                direction: Vec3::new(0.0, 0.0, 1.0),
            },
        ),
        (
            -1.0,
            Cause::Blast {
                centre: Vec3::new(1.0, 0.0, 0.5),
            },
        ),
    ] {
        let now = standing(Quat::from_axis_angle(Vec3::Y, turn), Vec3::ZERO);
        let before = driver_bones(&now);
        let mut ragdoll = Ragdoll::new(&start(&now, &before, cause, Some(GUN))).unwrap();
        let mut ground = Ground::flat();
        let mut lowest = f32::MAX;
        let mut landed = None;
        let t = run(&mut ragdoll, &mut ground, None, |r| {
            for b in r.bodies.iter().chain(r.gun.iter()) {
                let (a, c) = b.shape.ends(b.pose);
                lowest = lowest.min(a.y.min(c.y) - b.shape.radius);
            }
            if landed.is_none() && r.bodies[Part::Pelvis.index()].pose.position.y < 0.35 {
                landed = Some(r.age);
            }
        });
        times.push((landed.expect("it fell"), t));
        // it fell: the pelvis near the ground, nothing deep in it
        let pelvis = ragdoll.bodies[Part::Pelvis.index()].pose.position;
        assert!(pelvis.y < 0.45, "{cause:?}: pelvis at {pelvis:?}");
        assert!(lowest > -0.04, "{cause:?}: sank to {lowest}");
        assert!(
            joint_gaps(&ragdoll) < 0.02,
            "{cause:?}: joints apart by {}",
            joint_gaps(&ragdoll)
        );
    }
    // (landed, settled): down within a second; at rest within 3.5 s of
    // landing, and within 4 s of the death
    assert!(
        times
            .iter()
            .all(|&(landed, settled)| landed < 1.0 && settled - landed < 3.5 && settled < 4.0),
        "{times:?} s"
    );
}

/// Every joint within its limit (a few degrees of give).
fn within_limits(r: &Ragdoll, slack: f32) -> Result<(), (Part, f32, f32)> {
    for j in r.joints.iter() {
        let p = &r.bodies[j.parent.index()];
        let c = &r.bodies[j.child.index()];
        match j.limit {
            Limit::Swing {
                cone,
                axis_parent,
                forward_parent,
                axis_child,
                ..
            } => {
                let a = p.pose.rotation.rotate(axis_parent);
                let b = c.pose.rotation.rotate(axis_child);
                let angle = angle_between(a, b);
                if let Some(toward) = b.reject(a).direction() {
                    let reach = cone.reach(toward, p.pose.rotation.rotate(forward_parent));
                    if angle > reach + slack {
                        return Err((j.child, angle, reach));
                    }
                }
            }
            Limit::Hinge {
                hinge_parent,
                limb_parent,
                limb_child,
                min,
                max,
                ..
            } => {
                let h = p.pose.rotation.rotate(hinge_parent);
                let a = p
                    .pose
                    .rotation
                    .rotate(limb_parent)
                    .reject(h)
                    .direction()
                    .unwrap();
                let b = c
                    .pose
                    .rotation
                    .rotate(limb_child)
                    .reject(h)
                    .direction()
                    .unwrap();
                let phi = signed_angle(a, b, h);
                if phi < min - slack || phi > max + slack {
                    return Err((j.child, phi, max));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn joint_limits_hold_through_a_violent_fall() {
    // dropped from 3 m, spun hard, against a wall
    let now = standing(
        Quat::from_axis_angle(Vec3::Y, 0.3),
        Vec3::new(0.0, 3.0, 0.0),
    );
    let mut before = driver_bones(&now);
    for (i, b) in before.iter_mut().enumerate() {
        // each part was elsewhere a frame ago: wild start speeds (clamped)
        b.position += Vec3::new((i as f32 * 1.7).sin(), (i as f32).cos(), 0.5) * 0.4;
    }
    let mut ragdoll = Ragdoll::new(&start(&now, &before, SHOT, Some(GUN))).unwrap();
    // the start speed is clamped; the shot adds 2 m/s to the torso and 0.4 to each
    assert!(
        ragdoll
            .bodies
            .iter()
            .all(|b| b.velocity.length() <= START_SPEED + 2.4 + 1e-3)
    );
    let mut ground = Ground {
        floor: 0.0,
        wall: Some(0.6),
        queries: 0,
    };
    let mut worst = 0.0f32;
    run(&mut ragdoll, &mut ground, None, |r| {
        if let Err((part, angle, limit)) = within_limits(r, degrees(8.0)) {
            panic!(
                "{part:?} at {} degrees, limit {}",
                angle.to_degrees(),
                limit.to_degrees()
            );
        }
        worst = worst.max(joint_gaps(r));
        for b in r.bodies.iter() {
            assert!(
                b.pose.position.z < 0.6,
                "through the wall: {:?}",
                b.pose.position
            );
        }
    });
    // (a moment's stretch on the impact into the wall's foot, each capsule
    // end on its own plane; together again at rest)
    assert!(worst < 0.07, "joints apart by {worst}");
    assert!(
        joint_gaps(&ragdoll) < 0.01,
        "apart at rest by {}",
        joint_gaps(&ragdoll)
    );
    within_limits(&ragdoll, degrees(3.0)).unwrap();
}

#[test]
fn the_drive_follows_the_clip_then_lets_go() {
    // the clip bends the left elbow: with the arms' drive the forearm turns with it
    let now = standing(Quat::IDENTITY, Vec3::ZERO);
    let before = driver_bones(&now);
    let mut bent = before;
    let fore = Part::LeftForearm.index();
    bent[fore].rotation = Quat::from_axis_angle(Vec3::X, -1.2) * bent[fore].rotation;
    let mut ragdoll = Ragdoll::new(&start(&now, &before, Cause::Unknown, None)).unwrap();
    let mut ground = Ground::flat();
    let relative = |r: &Ragdoll| {
        let upper = r.bodies[Part::LeftUpperArm.index()].pose.rotation;
        let fore = r.bodies[Part::LeftForearm.index()].pose.rotation;
        upper.conjugate() * fore
    };
    let rest = relative(&ragdoll);
    for _ in 0..6 {
        ragdoll.step(1.0 / 60.0, Some(&bent), &mut ground);
    }
    let turned = relative(&ragdoll).rotation_from(rest).length();
    assert!(turned > 0.3, "the elbow followed the clip by {turned} rad");
    // the muscles: every one weakens from the start, all the time (no
    // phase); the legs are strongest and last longest; a headshot is limp
    let m = |cause, kind, child| muscle(cause, kind, child, 0.0);
    let knee = m(Cause::Unknown, JointKind::Knee, Part::LeftShin);
    let waist = m(Cause::Unknown, JointKind::Waist, Part::Torso);
    let elbow = m(Cause::Unknown, JointKind::Elbow, Part::LeftForearm);
    for t in [0.05f32, 0.2, 0.4, 0.8] {
        assert!(
            knee.strength(t) < knee.strength(t - 0.05),
            "the legs weaken all the time"
        );
        assert!(knee.strength(t) * knee.stiffness > waist.strength(t) * waist.stiffness);
        assert!(waist.strength(t) * waist.stiffness > elbow.strength(t) * elbow.stiffness);
    }
    let hit = Cause::Body {
        part: Part::LeftThigh,
        direction: Vec3::X,
        heavy: false,
    };
    let left = muscle(hit, JointKind::Knee, Part::LeftShin, 0.0);
    let right = muscle(hit, JointKind::Knee, Part::RightShin, 0.0);
    assert!(
        left.strength(0.3) < right.strength(0.3),
        "the hit leg gives first"
    );
    assert!(
        muscle(hit, JointKind::Knee, Part::RightShin, 60.0).strength(0.0) < right.strength(0.0),
        "a strong hit weakens"
    );
    let head = m(
        Cause::Head { direction: Vec3::X },
        JointKind::Knee,
        Part::LeftShin,
    );
    assert_eq!(head.strength(0.35), 0.0);
}

/// A death by `hits` (the killing one last) with `load` kg on the parts:
/// the ragdoll and its parts' poses every 1/60 s for `frames` frames.
fn fall(hits: &[crate::shot::Hit], load: [f32; PARTS], frames: usize) -> std::vec::Vec<PartPoses> {
    let now = standing(Quat::IDENTITY, Vec3::ZERO);
    let before = driver_bones(&now);
    let mut s = start(&now, &before, Cause::Unknown, None);
    s.hits = hits;
    s.load = load;
    let mut ragdoll = Ragdoll::new(&s).unwrap();
    let mut ground = Ground::flat();
    let mut out = std::vec![ragdoll.poses().0];
    for _ in 0..frames {
        ragdoll.step(1.0 / 60.0, None, &mut ground);
        out.push(ragdoll.poses().0);
    }
    out
}

fn moved(poses: &[PartPoses], part: Part, frame: usize) -> Vec3 {
    poses[frame][part.index()].position - poses[0][part.index()].position
}

/// The knee's bend, rad (the angle between thigh and shin).
fn knee_bend(poses: &PartPoses) -> f32 {
    let thigh = poses[Part::LeftThigh.index()].rotation.rotate(Vec3::X);
    let shin = poses[Part::LeftShin.index()].rotation.rotate(Vec3::X);
    angle_between(thigh, shin)
}

/// The owner's asks (2026-10-26): the bullet's momentum decides the fall
/// (away from the shooter, the part hit leading), and the legs move as a
/// powered ragdoll: they give way progressively, no stilts.
#[test]
fn the_killing_shot_decides_the_fall() {
    let looks = looks();
    let none = [0.0; PARTS];
    // from the front: backwards (-z), the pelvis and the head both
    let front = fall(&[looks[0].1], none, 120);
    assert!(
        moved(&front, Part::Head, 60).z < -0.4,
        "a front shot falls back: {:?}",
        moved(&front, Part::Head, 60)
    );
    assert!(moved(&front, Part::Pelvis, 90).z < -0.05);
    // from the side: sideways (+x)
    let side = fall(&[looks[1].1], none, 120);
    assert!(
        moved(&side, Part::Head, 60).x > 0.3,
        "a side shot falls sideways: {:?}",
        moved(&side, Part::Head, 60)
    );
    assert!(moved(&side, Part::Head, 60).x.abs() > moved(&side, Part::Head, 60).z.abs());
    // the head: snapped back at once, ahead of the chest
    let head = fall(&[looks[2].1], none, 30);
    let snap = moved(&head, Part::Head, 6).z - moved(&head, Part::Torso, 6).z;
    assert!(snap < -0.03, "a headshot snaps the head back: {snap}");
    // a leg hit kicks that leg
    let leg = crate::shot::Hit {
        point: Vec3::new(0.1, 0.3, 0.08),
        impulse: crate::shot::impulse(Vec3::new(0.0, 0.0, -1.0), 0.02, 800.0),
    };
    let kicked = fall(&[leg], none, 12);
    assert!(
        moved(&kicked, Part::LeftShin, 6).z < moved(&kicked, Part::RightShin, 6).z - 0.02,
        "the left shin is kicked back: {:?} against {:?}",
        moved(&kicked, Part::LeftShin, 6),
        moved(&kicked, Part::RightShin, 6)
    );
    // no shot: no push (the motion the clip had, none here)
    let still = fall(&[], none, 1);
    assert!(
        Part::ALL
            .iter()
            .all(|&p| moved(&still, p, 1).length() < 0.01)
    );
}

#[test]
fn a_running_body_carries_its_momentum() {
    // shot from the front while running forward (+z) at 5 m/s: it goes on
    // forward past where it died, instead of dropping there
    let front = looks()[0].1;
    let now = standing(Quat::IDENTITY, Vec3::ZERO);
    let before = driver_bones(&now);
    let hits = [front];
    let run = |velocity: Vec3| {
        let mut s = start(&now, &before, Cause::Unknown, None);
        s.hits = &hits;
        s.velocity = velocity;
        let mut ragdoll = Ragdoll::new(&s).unwrap();
        let mut ground = Ground::flat();
        for _ in 0..90 {
            ragdoll.step(1.0 / 60.0, None, &mut ground);
        }
        ragdoll.poses().0[Part::Pelvis.index()].position
    };
    let standing_still = run(Vec3::ZERO);
    let running = run(Vec3::new(0.0, 0.0, 5.0));
    assert!(
        running.z > standing_still.z + 0.5 && running.z > 0.3,
        "the running body slides on: {running:?} against {standing_still:?}"
    );
}

#[test]
fn the_legs_give_way_progressively_within_a_second() {
    // a body shot from the side (the legs buckle under it as it goes)
    let side = fall(&[looks()[1].1], [0.0; PARTS], 90);
    let pelvis = |f: usize| side[f][Part::Pelvis.index()].position.y;
    assert!(
        pelvis(60) < 0.6,
        "down by 1 s: the pelvis at {}",
        pelvis(60)
    );
    // the knee bends on every 0.1 s step of the first 0.6 s, none frozen
    let bends: std::vec::Vec<f32> = (0..=6).map(|k| knee_bend(&side[k * 6])).collect();
    std::println!(
        "knee every 0.1 s {:.2?} rad, pelvis {:.2?}",
        bends,
        (0..=6).map(|k| pelvis(k * 6)).collect::<std::vec::Vec<_>>()
    );
    for w in bends.windows(2) {
        assert!(
            w[1] > w[0] + degrees(2.0),
            "the knee bends steadily (no stilts): {bends:?}"
        );
    }
}

#[test]
fn equipment_and_ammo_change_the_knock() {
    let looks = looks();
    let front = looks[0].1;
    // a heavy kit (an exo-suit's 25 kg on the chest, 10 on the pelvis) is
    // knocked less than none
    let mut heavy = [0.0; PARTS];
    heavy[Part::Torso.index()] = 25.0;
    heavy[Part::Pelvis.index()] = 10.0;
    let light = fall(&[front], [0.0; PARTS], 12);
    let loaded = fall(&[front], heavy, 12);
    assert!(
        moved(&loaded, Part::Torso, 12).z > moved(&light, Part::Torso, 12).z + 0.01,
        "the heavy kit is knocked less: {:?} against {:?}",
        moved(&loaded, Part::Torso, 12),
        moved(&light, Part::Torso, 12)
    );
    // a sniper round (7.62x54) knocks harder than a pistol's (9x18)
    let shot = |mass: f32, speed: f32| crate::shot::Hit {
        point: front.point,
        impulse: crate::shot::impulse(Vec3::new(0.0, 0.0, -1.0), mass, speed),
    };
    let sniper = fall(&[shot(0.1, 800.0)], [0.0; PARTS], 12);
    let pistol = fall(&[shot(0.02, 315.0)], [0.0; PARTS], 12);
    assert!(moved(&sniper, Part::Torso, 12).z < moved(&pistol, Part::Torso, 12).z - 0.02);
}

#[test]
fn degenerate_input_gives_no_nan() {
    // every bone at one point, zero rotations
    let mut collapsed = [Pose::new(Quat::IDENTITY, Vec3::new(1.0, 0.5, 1.0)); BONE_COUNT];
    let before = driver_bones(&collapsed);
    let mut ragdoll = Ragdoll::new(&start(&collapsed, &before, SHOT, Some(GUN))).unwrap();
    let mut ground = Ground::flat();
    for dt in [0.0, -1.0, f32::NAN, 1e9, 1.0 / 60.0, 1.0 / 60.0] {
        ragdoll.step(dt, None, &mut ground);
        let (parts, gun) = ragdoll.poses();
        assert!(parts.iter().all(|p| p.is_finite()) && gun.is_none_or(|g| g.is_finite()));
    }
    run(&mut ragdoll, &mut ground, None, |r| {
        assert!(r.poses().0.iter().all(|p| p.is_finite()))
    });
    // a NaN or an infinite bone: no ragdoll (the game's clip is drawn)
    collapsed[5].position.x = f32::NAN;
    assert!(Ragdoll::new(&start(&collapsed, &before, SHOT, None)).is_none());
    let now = standing(Quat::IDENTITY, Vec3::ZERO);
    let mut bad = driver_bones(&now);
    bad[2].rotation.w = f32::INFINITY;
    assert!(Ragdoll::new(&start(&now, &bad, SHOT, None)).is_none());
    // a NaN clip target is ignored, a zero up is refused
    let mut ragdoll = Ragdoll::new(&start(&now, &driver_bones(&now), SHOT, None)).unwrap();
    let mut nan_clip = driver_bones(&now);
    nan_clip[0].position.y = f32::NAN;
    ragdoll.step(1.0 / 60.0, Some(&nan_clip), &mut ground);
    assert!(ragdoll.poses().0.iter().all(|p| p.is_finite()));
    let mut s = start(&now, &before, SHOT, None);
    s.up = Vec3::ZERO;
    assert!(Ragdoll::new(&s).is_none());
}

#[test]
fn queries_a_frame_are_bounded_and_few_on_the_ground() {
    let now = standing(Quat::IDENTITY, Vec3::ZERO);
    let before = driver_bones(&now);
    let mut ground = Ground::flat();
    let mut ragdoll = Ragdoll::new(&start(&now, &before, SHOT, Some(GUN))).unwrap();
    let mut per_frame = std::vec::Vec::new();
    loop {
        let asked = ground.queries;
        let motion = ragdoll.step(1.0 / 60.0, None, &mut ground);
        per_frame.push(ground.queries - asked);
        if motion == Motion::Settled {
            break;
        }
    }
    // at most both ends of every body, and fewer than one a body over the
    // last half second (on the ground, most bodies keep their plane)
    assert!(
        per_frame.iter().all(|&n| n <= 2 * (PARTS + 1)),
        "{per_frame:?}"
    );
    let tail = &per_frame[per_frame.len() - 30..];
    assert!(tail.iter().sum::<usize>() < 30 * (PARTS + 1), "{tail:?}");
}

#[test]
fn closest_points_of_segments() {
    let (a, b) = closest_points(
        Vec3::ZERO,
        Vec3::X,
        Vec3::new(0.5, 1.0, -1.0),
        Vec3::new(0.5, 1.0, 1.0),
    );
    assert!((a - Vec3::new(0.5, 0.0, 0.0)).length() < 1e-6);
    assert!((b - Vec3::new(0.5, 1.0, 0.0)).length() < 1e-6);
    let (a, b) = closest_points(Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
    assert_eq!((a, b), (Vec3::ZERO, Vec3::ZERO));
}

#[test]
#[ignore = "a trace for tuning: cargo test -p ragdoll trace -- --ignored --nocapture"]
fn trace() {
    extern crate std;
    use std::println;
    let now = standing(Quat::from_axis_angle(Vec3::Y, 2.5), Vec3::ZERO);
    let before = driver_bones(&now);
    let mut ragdoll = Ragdoll::new(&start(
        &now,
        &before,
        Cause::Body {
            part: Part::LeftThigh,
            direction: Vec3::X,
            heavy: true,
        },
        Some(GUN),
    ))
    .unwrap();
    let mut ground = Ground::flat();
    let mut frame = 0;
    run(&mut ragdoll, &mut ground, None, |r| {
        frame += 1;
        if (0..0).contains(&frame) {
            for i in [7usize, 8, 9, 10] {
                let b = &r.bodies[i];
                println!(
                    "  {i} pos {:.4} {:.4} {:.4} spin {:.3} {:.3} {:.3} contact {:?}",
                    b.pose.position.x,
                    b.pose.position.y,
                    b.pose.position.z,
                    b.spin.x,
                    b.spin.y,
                    b.spin.z,
                    matches!(b.contacts[0], Contact::Plane { .. })
                );
            }
        }
        if frame % 3 == 0 {
            let mut fastest = (0.0, 0.0, 0);
            for (i, b) in r.bodies.iter().chain(r.gun.iter()).enumerate() {
                let (v, w) = (b.velocity.length(), b.spin.length());
                if v + w * 0.1 > fastest.0 + fastest.1 * 0.1 {
                    fastest = (v, w, i);
                }
            }
            let mut kinetic = 0.0;
            let mut potential = 0.0;
            for b in r.bodies.iter() {
                let m = 1.0 / b.inverse_mass;
                let local = b.pose.rotation.conjugate().rotate(b.spin);
                let inertia = Vec3::new(
                    1.0 / b.inverse_inertia.x,
                    1.0 / b.inverse_inertia.y,
                    1.0 / b.inverse_inertia.z,
                );
                kinetic += 0.5 * m * b.velocity.length_squared()
                    + 0.5 * local.dot(local.mul_each(inertia));
                potential += m * GRAVITY * b.pose.position.y;
            }
            println!(
                "t {:.2} pelvis y {:.2} fastest {:?} kinetic {kinetic:.1} total {:.1}",
                r.age,
                r.bodies[0].pose.position.y,
                fastest,
                kinetic + potential
            );
        }
    });
}

#[test]
#[ignore = "a measurement: cargo test -p ragdoll rest_report -- --ignored --nocapture"]
fn rest_report() {
    use crate::ragdoll::scenes::{largest_move, scenes};
    for jitter in [false, true] {
        for mut scene in scenes() {
            let now = standing(Quat::IDENTITY, Vec3::ZERO);
            let before = driver_bones(&now);
            let cause = Cause::Body {
                part: Part::Torso,
                direction: Vec3::new(0.0, 0.0, 1.0),
                heavy: false,
            };
            let mut r = Ragdoll::new(&start(&now, &before, cause, Some(GUN))).unwrap();
            let mut last = r.poses().0;
            let mut t = 0.0;
            let mut moves = std::vec::Vec::new();
            let mut settled = None;
            let mut n = 0;
            while t < 8.0 {
                n += 1;
                let dt = if jitter {
                    [1.0 / 30.0, 1.0 / 144.0, 1.0 / 60.0, 1.0 / 45.0][n % 4]
                } else {
                    1.0 / 60.0
                };
                let m = r.step(dt, None, &mut scene);
                t += dt;
                let p = r.poses().0;
                moves.push((t, largest_move(&last, &p, &r.bodies) / dt));
                if std::env::var("SPIKES").is_ok()
                    && t > 1.8
                    && largest_move(&last, &p, &r.bodies) / dt > 0.8
                {
                    let worst = (0..PARTS)
                        .max_by(|&a, &b| {
                            (p[a].position - last[a].position)
                                .length()
                                .total_cmp(&(p[b].position - last[b].position).length())
                        })
                        .unwrap();
                    std::println!(
                        "  {} t {t:.3} dt {dt:.4} part {:?} moved {:.4} at {:?}",
                        scene.name,
                        Part::ALL[worst],
                        (p[worst].position - last[worst].position).length(),
                        p[worst].position
                    );
                }
                last = p;
                if m == Motion::Settled {
                    settled = Some(t);
                    break;
                }
            }
            let window = |a: f32, b: f32| {
                moves
                    .iter()
                    .filter(|m| m.0 >= a && m.0 < b)
                    .map(|m| m.1)
                    .fold(0.0f32, f32::max)
            };
            std::println!(
                "{:8} jitter {jitter:5}: settled {settled:?}, fastest end speed 1-1.5 s {:.2}, 1.5-2 s {:.2}, 2-3 s {:.2}, 3-5 s {:.2}, pelvis y {:.2}",
                scene.name,
                window(1.0, 1.5),
                window(1.5, 2.0),
                window(2.0, 3.0),
                window(3.0, 5.0),
                r.bodies[0].pose.position.y
            );
        }
    }
}

#[test]
#[ignore = "a trace: cargo test -p ragdoll rest_trace -- --ignored --nocapture"]
fn rest_trace() {
    use crate::ragdoll::scenes::scenes;
    let which = std::env::var("SCENE").unwrap_or("flat".into());
    let mut scene = scenes().into_iter().find(|s| s.name == which).unwrap();
    let now = standing(Quat::IDENTITY, Vec3::ZERO);
    let before = driver_bones(&now);
    let cause = Cause::Body {
        part: Part::Torso,
        direction: Vec3::new(0.0, 0.0, 1.0),
        heavy: false,
    };
    let mut r = Ragdoll::new(&start(&now, &before, cause, Some(GUN))).unwrap();
    let jitter = std::env::var("JITTER").is_ok();
    for n in 0..300 {
        let dt = if jitter {
            [1.0 / 30.0, 1.0 / 144.0, 1.0 / 60.0, 1.0 / 45.0][(n + 1) % 4]
        } else {
            1.0 / 60.0
        };
        let m = r.step(dt, None, &mut scene);
        if n >= 90 && n % 3 == 0 {
            let mut line = std::format!("{:.2}", r.age);
            for b in r.bodies.iter() {
                let cs = |c: Contact| match c {
                    Contact::Plane { normal, point } => std::format!(
                        "{:.0}{:.0}{:.0}@{:.2}",
                        normal.x * 9.0,
                        normal.y * 9.0,
                        normal.z * 9.0,
                        point.y
                    ),
                    Contact::Free => "free".into(),
                };
                let c = std::format!("{}/{}", cs(b.contacts[0]), cs(b.contacts[1]));
                line += &std::format!(
                    " | {:.2} {c} {:.3},{:.3},{:.3}",
                    b.end_speed(),
                    b.pose.position.x,
                    b.pose.position.y,
                    b.pose.position.z
                );
            }
            std::println!("{line}");
        }
        if m == Motion::Settled {
            std::println!("settled {:.2}", r.age);
            break;
        }
    }
}

/// The three looks the owner judges: a rifle round (5.45x39) in the chest
/// from the front, in the chest from the side, in the head from the front.
pub(crate) fn looks() -> [(&'static str, crate::shot::Hit); 3] {
    let rifle = |d: Vec3| crate::shot::impulse(d, 0.02, 800.0);
    [
        (
            "front",
            crate::shot::Hit {
                point: Vec3::new(0.0, 1.3, 0.12),
                impulse: rifle(Vec3::new(0.0, 0.0, -1.0)),
            },
        ),
        (
            "side",
            crate::shot::Hit {
                point: Vec3::new(-0.15, 1.3, 0.0),
                impulse: rifle(Vec3::new(1.0, 0.0, 0.0)),
            },
        ),
        (
            "head",
            crate::shot::Hit {
                point: Vec3::new(0.0, 1.66, 0.09),
                impulse: rifle(Vec3::new(0.0, 0.0, -1.0)),
            },
        ),
    ]
}

#[test]
#[ignore = "render dump: RAGDOLL_DUMP=<file> cargo test -p ragdoll render_dump -- --ignored"]
fn render_dump() {
    use std::io::Write;
    let out = std::env::var("RAGDOLL_DUMP").unwrap();
    let mut file = std::fs::File::create(out).unwrap();
    for (name, hit) in looks() {
        let now = standing(Quat::IDENTITY, Vec3::ZERO);
        let before = driver_bones(&now);
        let hits = [hit];
        let mut s = start(&now, &before, Cause::Unknown, Some(GUN));
        s.hits = &hits;
        let mut r = Ragdoll::new(&s).unwrap();
        let mut ground = Ground::flat();
        let mut frames = std::vec::Vec::new();
        for _ in 0..240 {
            let mut caps = std::vec::Vec::new();
            for b in r.bodies.iter().chain(r.gun.iter()) {
                let (a, c) = b.shape.ends(b.pose);
                caps.push(std::format!(
                    "[{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.3}]",
                    a.x,
                    a.y,
                    a.z,
                    c.x,
                    c.y,
                    c.z,
                    b.shape.radius
                ));
            }
            frames.push(std::format!("[{}]", caps.join(",")));
            r.step(1.0 / 60.0, None, &mut ground);
        }
        writeln!(
            file,
            "{{\"name\":\"{name}\",\"frames\":[{}]}}",
            frames.join(",")
        )
        .unwrap();
    }
}

#[test]
fn the_start_report_says_what_pushed_it() {
    let now = standing(Quat::IDENTITY, Vec3::ZERO);
    let before = driver_bones(&now);
    let hits = [looks()[0].1];
    let mut s = start(&now, &before, Cause::Unknown, None);
    s.hits = &hits;
    s.velocity = Vec3::new(0.0, 0.0, 3.0);
    let r = Ragdoll::new(&s).unwrap().report();
    assert_eq!((r.cause, r.hits), (1, 1));
    assert!(
        (r.push - 12.8).abs() < 0.01 && r.knock > 0.2 && (r.speed - 3.0).abs() < 1e-4,
        "{r:?}"
    );
    assert!(r.mass > 60.0 && r.strengths[0] > 0.5);
    let r = Ragdoll::new(&start(&now, &before, Cause::Unknown, None))
        .unwrap()
        .report();
    assert_eq!(
        (r.cause, r.hits, r.push, r.knock),
        (0, 0, 0.0, 0.0),
        "no bullet: no push"
    );
    s.hits = &[];
    s.cause = Cause::Head { direction: Vec3::X };
    assert_eq!(Ragdoll::new(&s).unwrap().report().cause, 2);
}

/// The owner's five looks for 464d: a rifle round from the front, from the
/// side, in the head, a sniper's round in the chest from the front, and a
/// rifle round from the front into a body running forward at 5 m/s.
pub(crate) fn looks5() -> [(&'static str, crate::shot::Hit, Vec3); 5] {
    let rifle = |d: Vec3| crate::shot::impulse(d, 0.02, 800.0);
    let back = Vec3::new(0.0, 0.0, -1.0);
    [
        (
            "front",
            crate::shot::Hit {
                point: Vec3::new(0.0, 1.3, 0.12),
                impulse: rifle(back),
            },
            Vec3::ZERO,
        ),
        (
            "side",
            crate::shot::Hit {
                point: Vec3::new(-0.15, 1.3, 0.0),
                impulse: rifle(Vec3::X),
            },
            Vec3::ZERO,
        ),
        (
            "head",
            crate::shot::Hit {
                point: Vec3::new(0.0, 1.66, 0.09),
                impulse: rifle(back),
            },
            Vec3::ZERO,
        ),
        (
            "sniper",
            crate::shot::Hit {
                point: Vec3::new(0.0, 1.3, 0.12),
                impulse: crate::shot::impulse(back, 0.1, 800.0),
            },
            Vec3::ZERO,
        ),
        (
            "running",
            crate::shot::Hit {
                point: Vec3::new(0.0, 1.3, 0.12),
                impulse: rifle(back),
            },
            Vec3::new(0.0, 0.0, 5.0),
        ),
    ]
}

#[test]
#[ignore = "render dump: RAGDOLL_DUMP=<file> cargo test -p ragdoll render_dump5 -- --ignored"]
fn render_dump5() {
    use std::io::Write;
    let out = std::env::var("RAGDOLL_DUMP").unwrap();
    let mut file = std::fs::File::create(out).unwrap();
    for (name, hit, velocity) in looks5() {
        let now = standing(Quat::IDENTITY, Vec3::ZERO);
        let before = driver_bones(&now);
        let hits = [hit];
        let mut s = start(&now, &before, Cause::Unknown, Some(GUN));
        s.hits = &hits;
        s.velocity = velocity;
        let mut r = Ragdoll::new(&s).unwrap();
        let mut ground = Ground::flat();
        let mut frames = std::vec::Vec::new();
        for _ in 0..240 {
            let mut caps = std::vec::Vec::new();
            for b in r.bodies.iter().chain(r.gun.iter()) {
                let (a, c) = b.shape.ends(b.pose);
                caps.push(std::format!(
                    "[{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.3}]",
                    a.x,
                    a.y,
                    a.z,
                    c.x,
                    c.y,
                    c.z,
                    b.shape.radius
                ));
            }
            frames.push(std::format!("[{}]", caps.join(",")));
            r.step(1.0 / 60.0, None, &mut ground);
        }
        writeln!(
            file,
            "{{\"name\":\"{name}\",\"frames\":[{}]}}",
            frames.join(",")
        )
        .unwrap();
    }
}

/// The game's ammo configs, (mass, muzzle speed): every distinct pair of
/// 0.26g's 34 (items/weapons/ammo: the damage calculator's table): buckshot
/// pellets, pistols and intermediate rounds, the full-power rifles, the
/// 12.7x55 and the slug, the 12.7x54.
const AMMO: [(f32, f32); 21] = [
    (0.01, 250.0),
    (0.02, 260.0),
    (0.02, 270.0),
    (0.02, 280.0),
    (0.02, 300.0),
    (0.02, 315.0),
    (0.02, 410.0),
    (0.02, 420.0),
    (0.02, 430.0),
    (0.02, 450.0),
    (0.02, 520.0),
    (0.02, 700.0),
    (0.02, 725.0),
    (0.02, 800.0),
    (0.1, 350.0),
    (0.1, 450.0),
    (0.1, 700.0),
    (0.1, 760.0),
    (0.1, 800.0),
    (0.1, 850.0),
    (0.2, 320.0),
];

/// 464f (the owner: "sometimes ragdoll gets too much impulse"): no single
/// hit of any ammo, at its muzzle speed, on any part, reaches a cap: the
/// whole body's knock stays below `BODY_KNOCK_MAX` and no part is seen faster
/// than `SHOT_MAX_SPEED` in the hit's first frames. The 464d trace's 0.1
/// rounds (68 N s) capped the knock at once.
#[test]
fn no_single_hit_reaches_a_cap() {
    let spots = [
        Vec3::new(0.0, 1.3, 0.15),   // the chest
        Vec3::new(0.0, 1.0, 0.12),   // the pelvis
        Vec3::new(0.0, 1.62, 0.1),   // the head
        Vec3::new(0.2, 1.32, 0.07),  // an upper arm
        Vec3::new(0.21, 1.05, 0.08), // a forearm
        Vec3::new(0.1, 0.75, 0.09),  // a thigh
        Vec3::new(0.1, 0.3, 0.07),   // a shin
    ];
    let back = Vec3::new(0.0, 0.0, -1.0);
    for (mass, speed) in AMMO {
        for point in spots {
            let hits = [crate::shot::Hit {
                point,
                impulse: crate::shot::impulse(back, mass, speed),
            }];
            let now = standing(Quat::IDENTITY, Vec3::ZERO);
            let before = driver_bones(&now);
            let mut s = start(&now, &before, Cause::Unknown, None);
            s.hits = &hits;
            let mut r = Ragdoll::new(&s).unwrap();
            let report = r.report;
            assert!(
                !report.capped && report.knock < tuning::BODY_KNOCK_MAX,
                "{mass} kg at {speed} m/s on {point:?}: the body's knock {} capped",
                report.knock
            );
            let mut ground = Ground::flat();
            let mut last = r.poses().0;
            for frame in 0..6 {
                r.step(1.0 / 60.0, None, &mut ground);
                let now = r.poses().0;
                for part in Part::ALL {
                    let seen =
                        (now[part.index()].position - last[part.index()].position).length() * 60.0;
                    assert!(
                        seen <= tuning::SHOT_MAX_SPEED,
                        "{mass} kg at {speed} m/s on {point:?}: {part:?} at {seen} m/s in frame {frame}"
                    );
                }
                last = now;
            }
        }
    }
    // the heavy classes count as SHOT_MAX_MASS
    assert_eq!(
        crate::shot::impulse(back, 0.1, 850.0),
        crate::shot::impulse(back, tuning::SHOT_MAX_MASS, 850.0)
    );
    assert!(
        crate::shot::impulse(back, 0.1, 850.0).length()
            > 2.0 * crate::shot::impulse(back, 0.02, 800.0).length()
    );
}

/// How far under Rudnya's drawn ground each settled body's axis lies (its
/// two ends and its centre, the deepest; under 0 when above it), the gun's
/// first, on the level made of `meshes` (`rudnya_floor`).
fn under_rudnya_drawn_ground(
    meshes: &[&'static [super::rudnya_floor::Triangle]],
) -> std::vec::Vec<(f32, f32)> {
    use super::rudnya_floor::{DRAWN, Floor};
    let mut out = std::vec::Vec::new();
    for (turn, cause) in [
        (0.0, SHOT),
        (1.3, SHOT),
        (
            2.5,
            Cause::Body {
                part: Part::LeftThigh,
                direction: Vec3::X,
                heavy: true,
            },
        ),
        (
            0.4,
            Cause::Head {
                direction: Vec3::new(0.0, 0.0, 1.0),
            },
        ),
        (-2.0, SHOT),
    ] {
        let now = standing(Quat::from_axis_angle(Vec3::Y, turn), Vec3::ZERO);
        let before = driver_bones(&now);
        let mut ragdoll = Ragdoll::new(&start(&now, &before, cause, Some(GUN))).unwrap();
        let mut floor = Floor {
            meshes: meshes.to_vec(),
        };
        let mut t = 0.0;
        while ragdoll.step(1.0 / 60.0, None, &mut floor) != Motion::Settled {
            t += 1.0 / 60.0;
            assert!(t < 10.0, "{cause:?}: never settled");
        }
        let under = |b: &Body| {
            let (a, c) = b.shape.ends(b.pose);
            [a, b.pose.position, c]
                .iter()
                .map(|p| {
                    let drawn = Floor::height(&DRAWN, p.x, p.z)
                        .unwrap_or_else(|| panic!("{cause:?}: off the piece of ground at {p:?}"));
                    drawn - p.y
                })
                .fold(f32::MIN, f32::max)
        };
        let gun = under(ragdoll.gun.as_ref().expect("the gun dropped"));
        let body = ragdoll.bodies.iter().map(under).fold(f32::MIN, f32::max);
        out.push((gun, body));
    }
    out
}

#[test]
fn on_rudnya_a_dropped_gun_lies_on_the_drawn_ground_not_in_it() {
    use super::rudnya_floor::{HITTABLE, WALKER};
    // the owner: "on Rudnya, weapons quite often fall under the textures".
    // On the walker mesh alone (patch 76's rays before) the gun's axis lay
    // 6 cm under the drawn ground, deeper than its radius: all of it hidden
    let walker = under_rudnya_drawn_ground(&[&WALKER]);
    std::println!("walker mesh alone (gun, body): {walker:?}");
    assert!(
        walker.iter().all(|&(gun, _)| gun > GUN.radius),
        "{walker:?}"
    );
    // with the collision bullets hit (what the rays meet now) the gun's
    // axis lies 3 cm above it (the collision is 1 cm under it here), every
    // body's 4 cm
    let both = under_rudnya_drawn_ground(&[&WALKER, &HITTABLE]);
    std::println!("walker and hittable collision (gun, body): {both:?}");
    assert!(
        both.iter().all(|&(gun, body)| gun < -0.02 && body < -0.03),
        "{both:?}"
    );
}

/// A lone gun (its body gone: the player respawned) stepped at 60 frames a
/// second until it rests: the frames it took and the highest its capsule's
/// bottom went under the floor (m, positive below).
fn lone_until_rest(gun: &mut LoneGun, ground: &mut Ground) -> (usize, f32) {
    let mut sunk = 0.0f32;
    for n in 1..=600 {
        let motion = gun.step(1.0 / 60.0, ground);
        let (a, b) = gun.body.shape.ends(gun.body.pose);
        sunk = sunk.max(gun.body.shape.radius - a.y.min(b.y));
        if motion == Motion::Settled {
            return (n, sunk);
        }
    }
    panic!("a lone gun never rested");
}

#[test]
fn a_lone_gun_in_the_air_falls_and_rests_on_the_ground() {
    let now = standing(Quat::IDENTITY, Vec3::ZERO);
    let before = driver_bones(&now);
    let ragdoll = Ragdoll::new(&start(&now, &before, SHOT, Some(GUN))).unwrap();
    // the body goes at once: the gun still at the hand's height
    let mut gun = ragdoll.lone_gun().unwrap();
    assert!(gun.centre().y > 0.8, "{:?}", gun.centre());
    let drawn = {
        let mut s = now;
        let (parts, g) = ragdoll.poses();
        ragdoll.rig().draw(&parts, g, &mut s);
        s[WEAPON.index()]
    };
    assert!((gun.weapon().position - drawn.position).length() < 1e-5, "the hand-off is where it was drawn");
    let mut ground = Ground::flat();
    let (frames, sunk) = lone_until_rest(&mut gun, &mut ground);
    let (a, b) = gun.body.shape.ends(gun.body.pose);
    assert!(a.y < 0.07 && b.y < 0.07 && a.y > 0.0 && b.y > 0.0, "on the ground: {a:?} {b:?}");
    assert!(sunk < 0.02, "sunk {sunk}");
    assert!(gun.speed() < SLOW && frames < 300, "{frames} frames");
    // at rest it stays: the next steps keep it where it lies
    let rest = gun.body.pose.position;
    for _ in 0..60 {
        gun.step(1.0 / 60.0, &mut ground);
    }
    assert!((gun.body.pose.position - rest).length() < 0.01);
}

#[test]
fn a_lone_gun_sliding_on_the_ground_stops() {
    let now = standing(Quat::IDENTITY, Vec3::ZERO);
    let before = driver_bones(&now);
    let ragdoll = Ragdoll::new(&start(&now, &before, SHOT, Some(GUN))).unwrap();
    let mut gun = ragdoll.lone_gun().unwrap();
    // lying flat on the floor, sliding at 3 m/s along its length's side
    let radius = gun.body.shape.radius;
    gun.body.pose = Pose::new(Quat::IDENTITY, Vec3::new(0.0, radius + 0.002, 0.0));
    gun.body.previous = gun.body.pose;
    gun.body.velocity = Vec3::new(0.0, 0.0, 3.0);
    gun.body.contacts = [Contact::Plane { point: Vec3::ZERO, normal: Vec3::Y }; 2];
    let mut ground = Ground::flat();
    let (frames, sunk) = lone_until_rest(&mut gun, &mut ground);
    let at = gun.body.pose.position;
    assert!(at.z > 0.05, "it slid on: {at:?}");
    assert!(sunk < 0.02 && (at.y - radius).abs() < 0.03, "on the ground: {at:?}, sunk {sunk}");
    assert!(gun.speed() < SLOW && frames < 300, "{frames} frames");
}
