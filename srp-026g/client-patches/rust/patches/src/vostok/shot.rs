//! A gun in its user's hands as the simulation has it, and a shot of it at
//! `weapon.shot_origin` (sites_026g.tsv): weapon_core::instant_fire 0x2E23A0
//! has copied its user's eyes' transform into its frame and is about to take
//! the bullet's direction and start from that copy. The site is in a
//! simulation step (the animation event `shoot`), for every player of the
//! world, so everything a [`HeldGun`] reads is the simulation's own state:
//! the same in every game at that step.
//!
//! The gun's pose is what the drawing makes of the same state for the
//! shooter's own view (update_bones_matrices 0x2FF600, process_hands_ik
//! 0x2959D0), without what is the drawing's alone (the third-person clips,
//! the first-view test, the hands' IK): the user's Weapon bone turned half
//! round its own up, the aim's alignment, the gun's bones from its user's
//! tree.

use core::ffi::c_void;
use core::marker::PhantomData;

use ragdoll::math::Vec3;

use super::context::Context;
use super::world::{Player, floats, read, write_floats};
use crate::raw;

/// A client weapon held by a player whose animations are mixed.
pub struct HeldGun<'c> {
    weapon: raw::Weapon,
    user: raw::Player,
    player: Player<'c>,
}

/// The shot being fired: the gun and what it leaves from.
pub struct Shot<'c> {
    gun: HeldGun<'c>,
    eye: raw::Float4x4,
    site: PhantomData<&'c ()>,
}

/// A weapon's locator in its gun's model as the simulation poses the gun: its
/// place on its bone times that bone's matrix (its place alone on no bone).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Located(pub [f32; 16]);

/// An empty boost::function (its first dword 0): the clips' resolver that
/// takes each animation's own clip, as the simulation's own calls give
/// (computed_head_transform 0x2DA4CB).
static OWN_CLIPS: [u32; 8] = [0; 8];

impl<'c> Shot<'c> {
    /// The shot of a client weapon held by a player whose animations are
    /// mixed; none for any other (a bare weapon_core has no locators).
    pub fn at(ctx: &'c Context<'_>) -> Option<Shot<'c>> {
        use raw::site::weapon_shot_origin as site;
        // SAFETY: the context is this site's: EBX instant_fire's weapon,
        // alive for the call, EAX the copy in its frame; a weapon's first
        // dword is its vtable, its +8 its user (read by instant_fire itself
        // at 0x2E243D), the user's tree its animation player's.
        unsafe {
            let (weapon, eye) = (site::weapon(ctx)?, site::eye(ctx)?);
            if read(weapon.vtable()) != raw::image::WEAPON_VTABLE {
                return None;
            }
            let user = weapon.user()?;
            user.animation_tree()?;
            Some(Shot { gun: HeldGun { weapon, user, player: Player::of(ctx, Some(user))? }, eye, site: PhantomData })
        }
    }

    pub fn gun(&self) -> &HeldGun<'c> {
        &self.gun
    }

    /// The eyes' copy the game made: what the shot would leave from.
    pub fn eye(&self) -> [f32; 16] {
        // SAFETY: the copy in instant_fire's frame, 16 floats, whole since
        // its rep movsd (0x2E2453).
        unsafe { floats(self.eye.value()) }
    }

    /// What the shot leaves from, in place of the eyes' copy: a transform
    /// laid out as it (rows right, up, forward, place).
    pub fn leave_from(&mut self, from: [f32; 16]) {
        // SAFETY: the copy in instant_fire's frame, 16 floats, read after
        // the site only (0x2E245C, 0x2E250F) on this thread.
        unsafe { write_floats(self.eye.value(), from) }
    }
}

impl<'c> HeldGun<'c> {
    /// The client weapon `player` holds, on the thread the simulation runs
    /// on (the tree's reference count is a plain one: animation_player
    /// 0x391A4A); none for anything else in its hands, or with no tree.
    pub fn of(player: Player<'c>) -> Option<HeldGun<'c>> {
        // SAFETY: the live player's held item: an item's first dword is its
        // vtable, and the client weapon's interface there is the weapon
        // itself (its vtable +70h returns `this`, 0x246380).
        unsafe {
            let item = player.this.held_item()?;
            if read(item.vtable()) != raw::image::WEAPON_VTABLE {
                return None;
            }
            player.this.animation_tree()?;
            Some(HeldGun { weapon: item.weapon(), user: player.this, player })
        }
    }

    /// Whose gun, and when in the simulation: the user's place in the match
    /// roster and its simulation milliseconds (player+324h, tick 0x2DAA7F).
    pub fn user_at(&self) -> (u8, u32) {
        // SAFETY: the live player's own word.
        (self.player.place(), unsafe { read(self.user.time()) })
    }

    /// How far the sight is raised: the aim's progress, 0 from the hip to 1
    /// fully aimed (ticked by the weapon's own tick, in the world).
    pub fn raised(&self) -> f32 {
        // SAFETY: the live weapon's own float.
        unsafe { read(self.weapon.aim_progress()) }
    }

    /// The user's transform in the world (its +40Ch).
    pub fn world(&self) -> [f32; 16] {
        // SAFETY: the live player's 16 floats.
        unsafe { floats(self.user.transform()) }
    }

    /// A bone of the user (`Weapon`, `Item`) in the user's own space, at the
    /// tree's time: the shot's.
    pub fn user_bone(&self, name: &str) -> Option<[f32; 16]> {
        // SAFETY: the live player's simulation skeleton and its records; the
        // animation player is the player's own (+750h), the animated object
        // the player, as computed_head_transform asks for Item (0x2DA4E8).
        unsafe {
            let skeleton = self.user.core_skeleton()?;
            let bone = named(skeleton, name)?;
            Some(self.bone(skeleton, bone, self.user.address()))
        }
    }

    /// The aim's offset: a move along the view before the aim's locator
    /// (calculate_aim_alignment 0x244D49).
    pub fn aim_offset(&self) -> [f32; 16] {
        // SAFETY: the live weapon's 16 floats, written at its making only.
        unsafe { floats(self.weapon.aim_offset()) }
    }

    /// The aim's locator in the gun's model (the record the weapon was
    /// loaded with: patch 172 puts it back after each alignment it draws).
    pub fn aim(&self) -> Option<Located> {
        // SAFETY: the live weapon's own record.
        unsafe { self.located(self.weapon.aim_locator()) }
    }

    /// The fire point's locator in the gun's model.
    pub fn barrel(&self) -> Option<Located> {
        // SAFETY: the live weapon's own record.
        unsafe { self.located(self.weapon.barrel_locator()) }
    }

    /// Where the level first stops a shot from `from` along the unit
    /// `along` within `range` (the collision bullets hit).
    pub fn first_met(&self, from: Vec3, along: Vec3, range: f32) -> Option<Vec3> {
        let world = self.player.physics_world()?;
        let hit = world.shot_ray_test_past([from.x, from.y, from.z], [along.x, along.y, along.z], range, |_| false)?;
        Some(Vec3::new(hit.point[0], hit.point[1], hit.point[2]))
    }

    /// # Safety
    /// `locator` is one of this weapon's records.
    unsafe fn located(&self, locator: raw::WeaponLocator) -> Option<Located> {
        // SAFETY: the caller's record; the gun's skeleton is the weapon's own
        // (+590h), its bones mixed in its user's tree with the weapon as the
        // animated object (weapon_core::compute_bones_matrices 0x2E2F50). A
        // locator's bone index counts from the first bone a matrix array
        // holds, the first record's first child.
        unsafe {
            let place: [f32; 16] = floats(locator.offset());
            let index = read(locator.bone());
            if index == NO_BONE {
                return Some(Located(place));
            }
            let skeleton = self.weapon.skeleton()?;
            let records = skeleton.bones();
            let first = records.at(0).children()?.index_from(records)?;
            let at = first + usize::from(index);
            if at >= read(skeleton.bone_count()) as usize {
                return None;
            }
            let bone = self.bone(skeleton, records.at(at), self.weapon.address());
            Some(Located(crate::affine::product(&place, &bone)))
        }
    }

    /// One bone's matrix in its skeleton's own space from the user's tree.
    ///
    /// # Safety
    /// `bone` is a record of `skeleton`, alive, and `object` the address of
    /// the object the tree animates over that skeleton.
    unsafe fn bone(&self, skeleton: raw::Skeleton, bone: raw::SkeletonBone, object: u32) -> [f32; 16] {
        let mut out = [0.0f32; 16];
        // SAFETY: the caller's; the animation player is the user's own, with
        // a tree (`at`); the engine asks a bone inside an animation event
        // itself (release_grenade 0x33A785 -> get_bone_transform 0x2DA540).
        unsafe {
            raw::engine_bone_matrix(self.user.animation().cast::<c_void>(), Some(bone), out.as_mut_ptr(), Some(skeleton),
                                    object as usize as *mut c_void, OWN_CLIPS.as_ptr().cast::<c_void>());
        }
        out
    }
}

/// The pitch (rad) the gun in `player`'s hands is lifted by its kick while
/// the view need not be: the eye's pitch with the weapon's kick against
/// without it. 0 with no client weapon in hand, and while the scope's own
/// view is on (the optic's picture is the screen, so the view is the gun's).
pub fn free_kick_lift(player: Player<'_>) -> f32 {
    // SAFETY: the live player's held item (its first dword its vtable; the
    // client weapon's interface is the weapon itself, as `HeldGun::of`), the
    // weapon's and the player's own floats and byte.
    unsafe {
        let Some(item) = player.this.held_item() else { return 0.0 };
        if read(item.vtable()) != raw::image::WEAPON_VTABLE {
            return 0.0;
        }
        let weapon = item.weapon();
        if read(weapon.scope_view()) != 0 {
            return 0.0;
        }
        kick_lift(read(player.this.look_pitch()), read(weapon.kick_top()))
    }
}

/// Where `player`'s input aims, without its weapon's kick and without what
/// its clips add (the steps' bob, the breath): the yaw (rad, growing to the
/// right: the player's +490h grows to the left, apply_rotation 0x2D8AE2) and
/// the eye's pitch of its look pitch (rad, up).
pub fn input_aim(player: Player<'_>) -> (f32, f32) {
    // SAFETY: the live player's own floats.
    let (yaw, look) = unsafe { (read(player.this.yaw()), read(player.this.look_pitch())) };
    (-yaw, eye_pitch(look))
}

/// Whether the scope's own view of the gun in `player`'s hands is on: the
/// optic's picture is the screen (the weapon's +181Eh, weapon::tick 0x244B26).
pub fn scope_view(player: Player<'_>) -> bool {
    // SAFETY: as `free_kick_lift`: the live player's held item and, a client
    // weapon's, its own byte.
    unsafe {
        let Some(item) = player.this.held_item() else { return false };
        read(item.vtable()) == raw::image::WEAPON_VTABLE && read(item.weapon().scope_view()) != 0
    }
}

/// The eye's pitch (rad) at a look pitch: the look clips turn bone Item 60
/// degrees a unit above the level and 67.5 below (look_time_factor_calculator
/// 0x2FBF90 scrubs them at (pitch + 1) / 2; decoded from the clip).
fn eye_pitch(look: f32) -> f32 {
    let look = look.clamp(-1.0, 1.0);
    (look * if look < 0.0 { 67.5 } else { 60.0 }).to_radians()
}

/// The eye's lift by a `kick` over the look pitch `base`
/// (weapon_core::look_pitch 0x2E0F60: the clamped sum); 0 for no number.
fn kick_lift(base: f32, kick: f32) -> f32 {
    let lift = eye_pitch(base + kick) - eye_pitch(base);
    if lift.is_finite() { lift } else { 0.0 }
}

/// A locator's bone index for none (calculate_locator 0x243C05).
const NO_BONE: u16 = 0xFFFF;

/// The record of the bone named `name`.
///
/// # Safety
/// `skeleton` is alive.
unsafe fn named(skeleton: raw::Skeleton, name: &str) -> Option<raw::SkeletonBone> {
    // SAFETY: the skeleton's run of records, each name a C string or null.
    unsafe {
        let records = skeleton.bones();
        (0..read(skeleton.bone_count()) as usize).map(|at| records.at(at)).find(|bone| {
            let text = bone.name();
            !text.is_null()
                && name.bytes().enumerate().all(|(n, b)| text.add(n).read_volatile() == b)
                && text.add(name.len()).read_volatile() == 0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kick_lifts_the_eye_sixty_degrees_a_unit_above_the_level() {
        assert!((kick_lift(0.0, 0.1).to_degrees() - 6.0).abs() < 1e-4);
        assert!((kick_lift(-0.2, 0.1).to_degrees() - 6.75).abs() < 1e-4);
        // Across the level: 67.5 a unit below it, 60 above.
        assert!((kick_lift(-0.1, 0.2).to_degrees() - 12.75).abs() < 1e-4);
    }

    #[test]
    fn a_kick_past_the_looks_end_lifts_no_further() {
        assert!((kick_lift(0.95, 0.2).to_degrees() - 3.0).abs() < 1e-4);
        assert_eq!(kick_lift(1.0, 0.5), 0.0);
        assert_eq!(kick_lift(0.0, 0.0), 0.0);
        assert_eq!(kick_lift(f32::NAN, 0.1), 0.0);
    }
}
