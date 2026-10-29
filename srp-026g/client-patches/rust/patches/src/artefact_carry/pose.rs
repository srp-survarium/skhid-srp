//! Shared first/third-person arm layer over native locomotion. Model-space
//! children are rebuilt from their original local transforms after a parent
//! changes; no hand, finger, roll or attached weapon is left behind.
#![forbid(unsafe_code)]

use artefact_carry::carry::Presentation;
use ragdoll::math::{Pose, Quat, Vec3, sin_cos, sqrt, clamp};
use crate::vostok::draw::{DrawnBones, MAX_BONES};
use crate::ragdoll::{pose_of, matrix_of};

#[path = "curves.rs"]
mod curves;

fn rotation([x, y, z]: [f32; 3]) -> Quat {
    let (sx, cx) = sin_cos(x);
    let (sy, cy) = sin_cos(y);
    let (sz, cz) = sin_cos(z);
    // Native spline's Euler convention, also used by the offline preview.
    Quat::from_basis(Vec3::new(cy * cz, -cy * sz, sy),
        Vec3::new(sy * sx * cz + cx * sz, -sx * sy * sz + cx * cz, -sx * cy),
        Vec3::new(-cx * sy * cz + sx * sz, cx * sy * sz + sx * cz, cx * cy))
}

/// Two-bone aim in model space. The eye is simulation-owned, so first-
/// person presentation, remote players and hit collision share one target.
/// Reach is bounded below full extension: neither limb is stretched.
fn raised_arm(eye: Pose, shoulder: Vec3, upper: f32, fore: f32) -> Option<[Quat; 3]> {
    if upper < 0.001 || fore < 0.001 { return None; }
    let delta = eye.apply(Vec3::new(0.10, -0.14, 0.34)) - shoulder;
    let direction = delta.direction()?;
    let distance = clamp(delta.length(), (upper - fore).abs() + 0.001, (upper + fore) * 0.96);
    let along = (upper * upper - fore * fore + distance * distance) / (2.0 * distance);
    let height = sqrt(upper * upper - along * along);
    let pole = eye.rotation.rotate(Vec3::new(0.75, -1.0, 0.0)).reject(direction).direction_or(direction.any_perpendicular());
    let elbow = shoulder + direction * along + pole * height;
    let wrist = shoulder + direction * distance;
    let arm_x = (elbow - shoulder).direction()?;
    let fore_x = (wrist - elbow).direction()?;
    let z = arm_x.cross(fore_x).direction_or(direction.any_perpendicular());
    let arm = Quat::from_basis(arm_x, z.cross(arm_x), z);
    let forearm = Quat::from_basis(fore_x, z.cross(fore_x), z);
    // Fingers extend along -X, palm normal +Y, thumb on the inside.
    let hand = eye.rotation * Quat::from_basis(Vec3::new(0.0, -0.6, -0.8), Vec3::new(0.0, 0.8, -0.6), Vec3::X);
    Some([arm, forearm, hand])
}

pub fn apply(bones: &DrawnBones<'_>, presentation: Presentation) -> Option<[f32; 16]> {
    if presentation.arms == 0.0 { return None; }
    let player = bones.player();
    let skeleton = bones.skeleton();
    let count = bones.count() as usize;
    if count > MAX_BONES || skeleton.bone_count() as usize != count + 1 { return None; }
    let mut original = [Pose::IDENTITY; MAX_BONES];
    let mut changed = original;
    let mut parents = [None; MAX_BONES];
    let mut targets = [None; MAX_BONES];
    let (mut right_arm, mut right_forearm, mut right_hand, mut weapon) = (None, None, None, None);
    for i in 0..count {
        if !bones.computed(i as u32) { continue; }
        original[i] = pose_of(&bones.get(i as u32)?)?;
        let bone = i as u32 + 1;
        let parent = skeleton.bone_parent(bone)?;
        if parent >= bone { return None; }
        parents[i] = parent.checked_sub(1).map(|n| n as usize);
        for (name, lower, upper) in curves::CURVES {
            if skeleton.bone_named(bone, name) {
                let angles = core::array::from_fn(|n| lower[n] + (upper[n] - lower[n]) * presentation.raised);
                targets[i] = Some(rotation(angles));
            }
        }
        if skeleton.bone_named(bone, "RightArm") { right_arm = Some(i); }
        if skeleton.bone_named(bone, "RightForeArm") { right_forearm = Some(i); }
        if skeleton.bone_named(bone, "RightHand") { right_hand = Some(i); }
        if skeleton.bone_named(bone, "Weapon") { weapon = Some(i); }
    }
    let right_hand = right_hand?;
    for i in 0..count {
        if !bones.computed(i as u32) { continue; }
        if parents[i].is_some_and(|p| !bones.computed(p as u32)) { return None; }
        let old_parent = parents[i].map_or(Pose::IDENTITY, |p| original[p]);
        let new_parent = parents[i].map_or(Pose::IDENTITY, |p| changed[p]);
        let mut local = old_parent.inverse().then(original[i]);
        if let Some(target) = targets[i] { local.rotation = local.rotation.nlerp(target, presentation.arms); }
        changed[i] = new_parent.then(local);
    }
    let world = pose_of(&player.transform())?;
    let eye = world.inverse().then(pose_of(&player.eye_transform())?);
    if let (Some(arm), Some(forearm)) = (right_arm, right_forearm) {
        let upper = (original[forearm].position - original[arm].position).length();
        let fore = (original[right_hand].position - original[forearm].position).length();
        if let Some(rotations) = raised_arm(eye, changed[arm].position, upper, fore) {
            let authored = changed;
            for i in 0..count {
                if !bones.computed(i as u32) { continue; }
                let old_parent = parents[i].map_or(Pose::IDENTITY, |p| authored[p]);
                let new_parent = parents[i].map_or(Pose::IDENTITY, |p| changed[p]);
                changed[i] = new_parent.then(old_parent.inverse().then(authored[i]));
                if let Some(joint) = [arm, forearm, right_hand].iter().position(|j| *j == i) {
                    changed[i].rotation = changed[i].rotation.nlerp(rotations[joint], presentation.raised);
                }
            }
        }
    }
    // Weapon is parented to Root in this rig. Preserve its grasp transform
    // while lowering/restoring, then the render owner removes its model.
    if let Some(weapon) = weapon {
        changed[weapon] = changed[right_hand].then(original[right_hand].inverse().then(original[weapon]));
    }
    for (i, p) in changed[..count].iter().enumerate() {
        if bones.computed(i as u32) { bones.set(i as u32, matrix_of(*p)); }
    }
    let hand = world.then(changed[right_hand]);
    // The model's base floats clear of the cupped fingers. Its orientation
    // follows the palm (including look pitch); only this model is scaled,
    // never the player, damage shapes, or the ground artefact.
    let seconds = (player.carry_time().0 % 4000) as f32 * (core::f32::consts::TAU / 4000.0);
    let hover = 0.12 + 0.012 * sin_cos(seconds).0;
    let centre = hand.then(Pose::new(Quat::IDENTITY, Vec3::new(-0.13, hover, 0.0)));
    Some(matrix_of(centre))
}
