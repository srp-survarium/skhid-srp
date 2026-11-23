//! The aimed gun at `hands_ik.aim_alignment` (sites_026g.tsv): process_hands_ik
//! 0x2959D0 in the first person with the aim's progress above 0, about to
//! call calculate_aim_alignment 0x244D40, which makes the gun's aim locator
//! (the weapon's +7A8h, on a bone of the gun or on none) the camera; and at
//! `hands_ik.aligned`, right after that call.

use core::marker::PhantomData;

use super::context::Context;
use super::world::{floats, write_floats};
use crate::raw;

/// A weapon state's id (+124h) of the three idle classes.
const IDLE: u32 = 3;
/// A locator's bone index for none (calculate_locator 0x243C05).
const NO_BONE: u16 = 0xFFFF;

/// A bone of the gun's skeleton, by its index in the gun's bones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bone(u16);

#[cfg(test)]
impl GunId {
    pub fn test(id: u32) -> GunId {
        GunId(id)
    }
}

impl Bone {
    /// A locator's bone index: None for the index that means no bone.
    pub fn parse(index: u16) -> Option<Bone> {
        (index != NO_BONE).then_some(Bone(index))
    }
}

/// How a gun's aim locator is placed (a row-vector 4x4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AimLocator {
    /// On a bone: the camera follows that bone as the clips move it.
    OnBone { bone: Bone, place: [f32; 16] },
    /// On no bone: a place in the gun's model, whatever its bones do.
    Posed { place: [f32; 16] },
}

/// Which weapon a gun is (its address: a key, never read through), for as
/// long as it lives; a later weapon may be made at the same one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunId(u32);

/// The gun whose aim the hands are about to be aligned to.
pub struct AimedGun<'c> {
    weapon: raw::Weapon,
    bones: raw::Float4x4,
    site: PhantomData<&'c ()>,
}

/// The gun's bones' matrices of this frame.
pub struct Bones<'g>(&'g raw::Float4x4);

impl<'c> AimedGun<'c> {
    pub fn at_alignment(ctx: &'c Context<'_>) -> Option<AimedGun<'c>> {
        use raw::site::hands_ik_aim_alignment as site;
        // SAFETY: the context is this site's: process_hands_ik's frame holds
        // the user's held weapon (kept at 0x295A09, tested at 0x295A0F) and
        // ECX its bones' matrices of this frame (loaded at 0x295A9C), both
        // alive while mt_draw's bone pass runs on this thread.
        let (weapon, bones) = unsafe { (site::frame(ctx)?.weapon()?, site::bones(ctx)?) };
        Some(AimedGun { weapon, bones, site: PhantomData })
    }

    pub fn id(&self) -> GunId {
        GunId(self.weapon.address())
    }

    pub fn locator(&self) -> AimLocator {
        // SAFETY: the live weapon's own locator, read on the one thread
        // that reads it (calculate_aim_alignment, called next).
        unsafe { locator_of(self.weapon) }
    }

    /// How far the sight is raised: the aim's progress (+5FCh), above 0 here,
    /// 1 fully aimed.
    pub fn progress(&self) -> f32 {
        // SAFETY: the live weapon's own float, which process_hands_ik read
        // just before the site (0x295A2x) on this thread.
        unsafe { *self.weapon.aim_progress() }
    }

    /// The bone the gun's body hangs on: its fire point's (the model's
    /// `barrel_point`, the weapon's +6E0h), none for one on no bone.
    pub fn body_bone(&self) -> Option<Bone> {
        // SAFETY: the live weapon's own locator record, written at its load
        // only (load_weapon 0x243A30).
        Bone::parse(unsafe { *self.weapon.barrel_locator().bone() })
    }

    /// The gun's bones as its clips pose them this frame.
    pub fn bones(&self) -> Bones<'_> {
        Bones(&self.bones)
    }

    /// The gun's bones, when what poses them this frame is its idle clip
    /// alone: the weapon is in its idle state and its user's mixing tree
    /// holds exactly one animation of this gun (the idle state's item clip;
    /// a clip fading out, at any weight, is a second one). The game's own
    /// state changes give the tree its new target in the same call
    /// (select_animations 0x2D9690), so an idle weapon's tree holds its idle
    /// clip; a world loaded from another game has both as that game sent them.
    pub fn idle_bones(&self) -> Option<Bones<'_>> {
        // SAFETY: the live weapon, its state and its user, and the tree the
        // engine mixed this gun's bones from in this same pass
        // (update_bones_matrices 0x2FF60F..0x2FF63E): its animation records
        // and their nodes are alive and unchanged while the pass runs.
        let mixed = unsafe {
            if *self.weapon.state()?.id() != IDLE {
                return None;
            }
            let tree = self.weapon.user()?.animation_tree()?;
            let animations = tree.animations()?;
            let mut mixed = 0u32;
            for n in 0..*tree.animation_count() as usize {
                if *animations.at(n).node()?.animated_object() == self.weapon.address() {
                    mixed += 1;
                }
            }
            mixed
        };
        (mixed == 1).then_some(Bones(&self.bones))
    }

    /// The aim locator placed in the gun's model, on no bone, for the
    /// alignment's call that follows: [`AlignedGun::hang_locator`] puts the
    /// weapon's own back after it.
    pub fn pose_locator(&mut self, place: [f32; 16]) {
        // SAFETY: as `locator`; the write is whole before its one reader runs.
        unsafe {
            let locator = self.weapon.aim_locator();
            write_floats(locator.offset(), place);
            *locator.bone() = NO_BONE;
        }
    }
}

/// The gun whose alignment was just made (`hands_ik.aligned`).
pub struct AlignedGun<'c> {
    weapon: raw::Weapon,
    site: PhantomData<&'c ()>,
}

impl<'c> AlignedGun<'c> {
    pub fn at_aligned(ctx: &'c Context<'_>) -> Option<AlignedGun<'c>> {
        // SAFETY: the context is this site's: process_hands_ik's frame still
        // holds the weapon calculate_aim_alignment was just called for
        // (hands_ik.aim_alignment's, its pushed argument popped).
        let weapon = unsafe { raw::site::hands_ik_aligned::frame(ctx)?.weapon()? };
        Some(AlignedGun { weapon, site: PhantomData })
    }

    pub fn id(&self) -> GunId {
        GunId(self.weapon.address())
    }

    pub fn locator(&self) -> AimLocator {
        // SAFETY: the live weapon's own locator, on mt_draw's thread, after
        // its reader of this frame returned.
        unsafe { locator_of(self.weapon) }
    }

    /// The aim locator back on a bone of the gun, as its model has it.
    pub fn hang_locator(&mut self, bone: Bone, place: [f32; 16]) {
        // SAFETY: as `locator`: nothing else reads the record while mt_draw
        // runs (the simulation's steps are over before the frame's drawing
        // and wait for its tasks, game_world::tick 0x1F7C37..0x1F7D7F).
        unsafe {
            let locator = self.weapon.aim_locator();
            write_floats(locator.offset(), place);
            *locator.bone() = bone.0;
        }
    }
}

/// A weapon's aim locator as its record holds it.
///
/// # Safety
/// `weapon` is alive and no other thread writes its record.
unsafe fn locator_of(weapon: raw::Weapon) -> AimLocator {
    // SAFETY: the caller's: the record's 64h bytes are the weapon's own.
    let (index, place) = unsafe {
        let locator = weapon.aim_locator();
        (*locator.bone(), floats(locator.offset()))
    };
    match Bone::parse(index) {
        Some(bone) => AimLocator::OnBone { bone, place },
        None => AimLocator::Posed { place },
    }
}

impl Bones<'_> {
    /// A bone's matrix in the gun's model (a row-vector 4x4).
    pub fn bone(&self, bone: Bone) -> [f32; 16] {
        // SAFETY: the index is one of this gun's locators' (the aim's or the
        // fire point's), which calculate_locator indexes these same matrices
        // with (0x243C73; weapon::draw's at 0x244E68) when left on its bone.
        unsafe { floats(self.0.at(usize::from(bone.0)).value()) }
    }
}
