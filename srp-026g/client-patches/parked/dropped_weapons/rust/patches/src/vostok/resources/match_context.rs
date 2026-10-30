//! Loading context borrowed from the current match, never a saved cook stage.
//! Retain the native match resource so its owned physics world survives query
//! callbacks. The outer game/world/dictionary still borrow the native tick.

use super::{InventoryRecipe, PrepareError, PreparedWeapon, ProfileError, ProfileTemplate};
use crate::raw;
use crate::vostok::pickup_input::{CoreId, InputCore, InputJournals};
use crate::vostok::world::{Object, pointer, read};
use crate::vostok::{Handle, Thread};
use core::sync::atomic::{AtomicU32, Ordering};
use weapon_pickup::state::{Kit, Template};
use weapon_pickup::{DroppedAmmo, Handedness};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchPrepareError {
    Profile(ProfileError),
    Weapon(PrepareError),
}

/// A retained match plus its tick-borrowed loading inputs. It cannot escape
/// the hook or move to another thread. Retention protects the core/physics
/// owner; it does not establish callback reentrancy for an arbitrary hook.
pub struct MatchResources<'t> {
    owner: Object,
    core: CoreId,
    dictionary: Object,
    physics: Object,
    world: Object,
    seed: u32,
    thread: &'t Thread,
}

impl<'t> MatchResources<'t> {
    pub(in crate::vostok) fn read(world: Object, thread: &'t Thread) -> Option<Self> {
        // SAFETY: GameWorld supplies its live tick-borrowed world. The match
        // pointer is held by that world; its concrete type is checked before
        // reading pvp_match_core fields. No callback runs while inspecting it.
        unsafe {
            let owner = pointer(raw::game_world::match_wrapper(world.ptr()))?;
            if read(owner.ptr().cast::<u32>()) != raw::image::PVP_MATCH_VTABLE {
                return None;
            }
            let options = pointer(raw::input_world_wrapper::options(owner.ptr()))?;
            let physics = pointer(raw::input_world_wrapper::physics_world(owner.ptr()))?;
            let game = pointer(raw::game_world::game(world.ptr()))?;
            let dictionary = pointer(raw::game::items_dictionary(game.ptr()))?;
            let seed = read(raw::match_options::seed(options.ptr()));
            let core =
                InputCore::from_raw(thread, raw::input_world_wrapper::core(owner.ptr()) as u32)?
                    .identity();
            AtomicU32::from_ptr(raw::unmanaged_resource::references(owner.ptr()))
                .fetch_add(1, Ordering::AcqRel);
            Some(Self {
                owner,
                core,
                dictionary,
                physics,
                world,
                seed,
                thread,
            })
        }
    }

    pub fn core_id(&self) -> CoreId {
        self.core
    }

    fn recipe<'a>(&'a self, profile: &'a ProfileTemplate, kit: Kit) -> InventoryRecipe<'a> {
        InventoryRecipe {
            // The original map cook sets the player/inventory recipe flag
            // to zero at2CC6F5. No padding or old creation-stage fields remain.
            words: [
                profile.this.id(),
                u32::from(kit.get()),
                self.dictionary.id(),
                self.physics.id(),
                self.world.id(),
                self.seed,
                0,
            ],
            _thread: self.thread,
        }
    }

    /// Prepare with current match inputs and an independently owned profile.
    ///
    /// # Safety
    /// Caller must establish a resource-dispatch phase with no simulation or
    /// owner lock/borrow that callbacks can reenter. The outer game, world and
    /// dictionary must stay alive across dispatch. The retained match alone
    /// does not prove these facts. Source lookup is confined to this core's
    /// bank and the checkout restores it after success, failure or unwind.
    pub unsafe fn prepare_weapon<const N: usize>(
        &self,
        profiles: &InputJournals<N>,
        template: Template,
        handedness: Handedness,
        ammo: DroppedAmmo,
    ) -> Result<PreparedWeapon, MatchPrepareError> {
        profiles
            .with_profile(self.core, template.source, self.thread, |profile| {
                // SAFETY: caller establishes dispatch phase; this core's profile
                // is checked out without retaining any registry lock or borrow.
                unsafe {
                    self.recipe(profile, template.kit).prepare_weapon(
                        profile,
                        template.kit,
                        template.slot,
                        handedness,
                        ammo,
                    )
                }
            })
            .map_err(MatchPrepareError::Profile)?
            .map_err(MatchPrepareError::Weapon)
    }
}

impl Drop for MatchResources<'_> {
    fn drop(&mut self) {
        // SAFETY: this guard retained one native resource reference. The
        // native cook owns physics destruction after core/player teardown.
        unsafe {
            let count = raw::unmanaged_resource::references(self.owner.ptr());
            if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_object(self.owner.ptr(), count);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrows_current_match_retains_owner_and_refuses_incomplete_contexts() {
        let mut world = vec![0u32; 5000];
        let mut game = vec![0u32; 6000];
        let mut owner = vec![0u32; 26000];
        let mut options = vec![0u32; 90000];
        let mut dictionary = 7u32;
        let mut physics = [0u32; 24];
        let thread = unsafe { Thread::new() };
        let world = Object::at(world.as_mut_ptr() as u32).unwrap();
        let game = Object::at(game.as_mut_ptr() as u32).unwrap();
        let owner = Object::at(owner.as_mut_ptr() as u32).unwrap();
        let options = Object::at(options.as_mut_ptr() as u32).unwrap();
        unsafe {
            raw::game_world::game(world.ptr()).write(game.id());
            raw::game_world::match_wrapper(world.ptr()).write(owner.id());
            owner
                .ptr()
                .cast::<u32>()
                .write(raw::image::PVP_MATCH_VTABLE);
            raw::unmanaged_resource::references(owner.ptr()).write(1);
            raw::input_world_wrapper::options(owner.ptr()).write(options.id());
            raw::input_world_wrapper::physics_world(owner.ptr()).write(physics.as_mut_ptr() as u32);
            raw::match_options::seed(options.ptr()).write(12345);
            // No dictionary: failure must not acquire a reference.
            assert!(MatchResources::read(world, &thread).is_none());
            assert_eq!(read(raw::unmanaged_resource::references(owner.ptr())), 1);
            raw::game::items_dictionary(game.ptr()).write((&raw mut dictionary) as u32);
            let current = MatchResources::read(world, &thread).unwrap();
            assert_eq!(read(raw::unmanaged_resource::references(owner.ptr())), 2);
            assert_eq!(current.seed, 12345);
            assert_eq!(current.physics.id(), physics.as_mut_ptr() as u32);
            assert_eq!(current.dictionary.id(), (&raw mut dictionary) as u32);
            assert_eq!(current.world, world);
            let mut source = vec![0u32; super::super::PROFILE_BYTES / 4];
            source[0] = 0xC0FFEE;
            let profile = ProfileTemplate::copy_from(
                source.as_ptr().cast(),
                super::super::game_allocator().unwrap(),
            )
            .unwrap();
            assert_eq!(
                current.recipe(&profile, Kit::parse(2).unwrap()).words,
                [
                    profile.this.id(),
                    2,
                    (&raw mut dictionary) as u32,
                    physics.as_mut_ptr() as u32,
                    world.id(),
                    12345,
                    0
                ]
            );
            drop(profile);
            let source_life = weapon_pickup::state::Life {
                place: weapon_pickup::state::Place::parse(0).unwrap(),
                spawned: 1,
            };
            let ammo = DroppedAmmo::parse(
                30,
                7,
                weapon_pickup::Chamber::Loaded,
                weapon_pickup::AmmoKind::parse(0).unwrap(),
            )
            .unwrap();
            assert!(matches!(
                current.prepare_weapon(
                    &InputJournals::<4>::new(),
                    Template {
                        source: source_life,
                        kit: Kit::parse(0).unwrap(),
                        slot: weapon_pickup::WeaponSlot::First
                    },
                    Handedness::Two,
                    ammo
                ),
                Err(MatchPrepareError::Profile(ProfileError::MissingWorld))
            ));
            assert_eq!(
                current.core_id(),
                InputCore::from_raw(&thread, raw::input_world_wrapper::core(owner.ptr()) as u32)
                    .unwrap()
                    .identity()
            );
            drop(current);
            assert_eq!(read(raw::unmanaged_resource::references(owner.ptr())), 1);
            // A new observation reads the current options, not a saved stage.
            raw::match_options::seed(options.ptr()).write(0);
            assert_eq!(MatchResources::read(world, &thread).unwrap().seed, 0);
            for field in [
                raw::input_world_wrapper::options(owner.ptr()),
                raw::input_world_wrapper::physics_world(owner.ptr()),
                raw::game_world::game(world.ptr()),
                raw::game_world::match_wrapper(world.ptr()),
                owner.ptr().cast::<u32>(),
            ] {
                let original = read(field);
                field.write(0);
                assert!(MatchResources::read(world, &thread).is_none());
                field.write(original);
                assert_eq!(read(raw::unmanaged_resource::references(owner.ptr())), 1);
            }
        }
    }
}
