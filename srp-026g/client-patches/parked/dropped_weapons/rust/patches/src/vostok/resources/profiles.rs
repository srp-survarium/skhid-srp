//! Original equipment retained by exact source life, independently of donor
//! presence. A checkout moves the unique profile owner out of the registry so
//! native resource callbacks run with neither a registry lock nor its borrow.

use super::{CookError, ProfileTemplate, game_allocator};
use crate::raw;
use crate::vostok::pickup_input::{CoreId, InputCore};
use crate::vostok::types::{GameTime, Place};
use crate::vostok::world::{Object, pointer, read};
use crate::vostok::{Context, Handle, Reg, Thread};
use core::ptr::NonNull;
use weapon_pickup::state::{Life, Place as SharedPlace};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileError {
    MissingWorld,
    MissingTemplate,
    Busy,
    Cook(CookError),
}

/// One live original profile and its identity, borrowed from a player hook.
/// Calling capture copies the profile; it never retains this source pointer.
pub struct ProfileSource<'t> {
    core: CoreId,
    life: Life,
    profile: Object,
    thread: &'t Thread,
}

impl<'t> ProfileSource<'t> {
    /// Client player::on_player_death (1F28E0) and the base-player variant
    /// (2D77C0), before either prologue. Both supply ECX = live player,
    /// before removing it from the alive list or notifying its inventory.
    pub fn at_player_death(ctx: &'t Context<'_>) -> Option<Self> {
        Self::read(Object::at(ctx.reg(Reg::Ecx))?, ctx.thread())
    }

    pub(in crate::vostok) fn read(player: Object, thread: &'t Thread) -> Option<Self> {
        // SAFETY: Player supplies its live object and this hook's thread.
        // Native profile describes the original loadout even when inventory
        // slots have temporarily been replaced with borrowed weapons.
        unsafe {
            if read(raw::player::inserted(player.ptr())) == 0 {
                return None;
            }
            let spawned = GameTime::from_raw(read(raw::player::spawned(player.ptr())))?;
            let place = Place::new(u32::from(read(raw::player::place(player.ptr()))))?;
            let core = InputCore::from_raw(thread, read(raw::player::core(player.ptr())))?;
            Some(Self {
                core: core.identity(),
                life: Life {
                    place: SharedPlace::parse(place.get()).unwrap(),
                    spawned: spawned.0,
                },
                profile: pointer(raw::player::profile(player.ptr()))?,
                thread,
            })
        }
    }

    pub fn core_id(&self) -> CoreId {
        self.core
    }
    pub fn life(&self) -> Life {
        self.life
    }
    pub(in crate::vostok) fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// Allocation/copy happens before taking any match-registry lock.
    pub(in crate::vostok) fn capture(&self) -> Result<CapturedProfile, ProfileError> {
        let allocator = game_allocator().map_err(ProfileError::Cook)?;
        // SAFETY: borrowed source remains live throughout this synchronous
        // constructor/body-copy operation, which does not dispatch queries.
        let profile = unsafe { ProfileTemplate::copy_from(self.profile.ptr(), allocator) }
            .map_err(ProfileError::Cook)?;
        CapturedProfile::new(self.core, self.life, profile).map_err(ProfileError::Cook)
    }
}

enum ProfileState {
    Ready(ProfileTemplate),
    CheckedOut,
}

struct Entry {
    core: CoreId,
    source: Life,
    state: ProfileState,
    next: Option<CapturedProfile>,
}

/// A preallocated entry, consumed by the matching bank. Its Drop never
/// dispatches resource queries. It must run outside the registry lock.
pub(in crate::vostok) struct CapturedProfile {
    entry: NonNull<Entry>,
    #[cfg(not(test))]
    allocator: Object,
}

impl CapturedProfile {
    fn new(core: CoreId, source: Life, profile: ProfileTemplate) -> Result<Self, CookError> {
        #[cfg(not(test))]
        let allocator = profile.allocator;
        let entry = Entry {
            core,
            source,
            state: ProfileState::Ready(profile),
            next: None,
        };
        #[cfg(not(test))]
        unsafe {
            // SAFETY: initialized native allocator, stable aligned storage for
            // one Rust entry. The profile has its own independent allocation.
            let ptr = raw::engine_malloc(
                allocator.ptr(),
                size_of::<Entry>() as u32,
                c"weapon pickup template bank".as_ptr().cast(),
            )
            .cast::<Entry>();
            let ptr = NonNull::new(ptr).ok_or(CookError::OutOfMemory)?;
            ptr.as_ptr().write(entry);
            Ok(Self {
                entry: ptr,
                allocator,
            })
        }
        #[cfg(test)]
        {
            Ok(Self {
                entry: NonNull::from(std::boxed::Box::leak(std::boxed::Box::new(entry))),
            })
        }
    }

    fn get(&self) -> &Entry {
        // SAFETY: unique constructed allocation owned by this entry.
        unsafe { self.entry.as_ref() }
    }
    fn get_mut(&mut self) -> &mut Entry {
        // SAFETY: exclusive detached ownership or the match registry lock.
        unsafe { self.entry.as_mut() }
    }
}

impl Drop for CapturedProfile {
    fn drop(&mut self) {
        assert!(
            self.get().next.is_none(),
            "template entry must be detached before release"
        );
        assert!(
            matches!(self.get().state, ProfileState::Ready(_)),
            "template still checked out at teardown"
        );
        #[cfg(not(test))]
        unsafe {
            // SAFETY: detached exclusive entry. Drop the profile first, then
            // release entry storage through its original native allocator.
            self.entry.as_ptr().drop_in_place();
            let mut ptr = self.entry.as_ptr().cast();
            raw::engine_free(self.allocator.ptr(), ptr, &mut ptr);
        }
        #[cfg(test)]
        unsafe {
            drop(std::boxed::Box::from_raw(self.entry.as_ptr()))
        };
    }
}

pub(in crate::vostok) struct ProfileBank {
    core: CoreId,
    head: Option<CapturedProfile>,
}

/// Private to the Vostok match owner, whose RAII guard must restore it on
/// every return. Safe patch code never gets a manual checkout/restore pair.
pub(in crate::vostok) struct ProfileCheckout {
    pub source: Life,
    pub profile: ProfileTemplate,
}

impl ProfileBank {
    pub fn new(core: CoreId) -> Self {
        Self { core, head: None }
    }

    pub fn contains(&self, source: Life) -> bool {
        self.entries().any(|entry| entry.source == source)
    }

    /// No allocation, native calls or destruction inside this operation.
    /// A duplicate is returned to the caller for release outside the lock.
    pub fn insert(&mut self, mut candidate: CapturedProfile) -> Option<CapturedProfile> {
        assert_eq!(
            candidate.get().core,
            self.core,
            "profile belongs to another native core"
        );
        if self.contains(candidate.get().source) {
            return Some(candidate);
        }
        candidate.get_mut().next = self.head.take();
        self.head = Some(candidate);
        None
    }

    fn entries(&self) -> impl Iterator<Item = &Entry> {
        core::iter::successors(self.head.as_ref().map(CapturedProfile::get), |entry| {
            entry.next.as_ref().map(CapturedProfile::get)
        })
    }

    fn find_mut(&mut self, source: Life) -> Option<&mut Entry> {
        let mut cursor = self.head.as_mut();
        while let Some(entry) = cursor {
            if entry.get().source == source {
                return Some(entry.get_mut());
            }
            cursor = entry.get_mut().next.as_mut();
        }
        None
    }

    pub fn checkout(&mut self, source: Life) -> Result<ProfileCheckout, ProfileError> {
        let entry = self.find_mut(source).ok_or(ProfileError::MissingTemplate)?;
        match core::mem::replace(&mut entry.state, ProfileState::CheckedOut) {
            ProfileState::Ready(profile) => Ok(ProfileCheckout { source, profile }),
            ProfileState::CheckedOut => Err(ProfileError::Busy),
        }
    }

    pub fn restore(&mut self, checkout: ProfileCheckout) {
        let entry = self
            .find_mut(checkout.source)
            .expect("checked out profile lost bank entry");
        assert!(
            matches!(entry.state, ProfileState::CheckedOut),
            "profile restored twice"
        );
        entry.state = ProfileState::Ready(checkout.profile);
    }

    pub fn can_destroy(&self) -> bool {
        self.entries()
            .all(|entry| matches!(entry.state, ProfileState::Ready(_)))
    }
}

impl Drop for ProfileBank {
    fn drop(&mut self) {
        assert!(self.can_destroy(), "profile query outlived its match owner");
        // Iterative teardown: a long match must not recurse once per death.
        while let Some(mut entry) = self.head.take() {
            self.head = entry.get_mut().next.take();
        }
    }
}

#[cfg(test)]
pub(in crate::vostok) mod fixture {
    use super::*;

    pub struct Source {
        pub player: std::vec::Vec<u32>,
        pub profile: std::vec::Vec<u32>,
    }

    impl Source {
        pub fn new(core: u32, place: u8, spawned: u32, equipment: u32) -> Self {
            let mut player = std::vec![0u32; 0x6BC / 4];
            let mut profile = std::vec![0u32; super::super::PROFILE_BYTES / 4];
            profile[0] = 0xC0FFEE;
            profile[1] = equipment;
            unsafe {
                let p = player.as_mut_ptr().cast();
                raw::player::core(p).write(core);
                raw::player::place(p).write(place);
                raw::player::spawned(p).write(spawned);
                raw::player::inserted(p).write(1);
                raw::player::profile(p).write(profile.as_mut_ptr() as u32);
            }
            Self { player, profile }
        }

        pub fn source<'t>(&'t mut self, thread: &'t Thread) -> Option<ProfileSource<'t>> {
            ProfileSource::read(Object::at(self.player.as_mut_ptr() as u32).unwrap(), thread)
        }
    }

    pub fn equipment(profile: &ProfileTemplate) -> u32 {
        unsafe { *profile.this.ptr().cast::<u32>().add(1) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixture::{Source, equipment};

    #[test]
    fn retains_first_profile_per_source_life_across_donor_changes_and_reuse() {
        let thread = unsafe { Thread::new() };
        let mut source = Source::new(1, 2, 100, 17);
        let borrowed = source.source(&thread).unwrap();
        let first = borrowed.life();
        let mut bank = ProfileBank::new(borrowed.core_id());
        assert!(bank.insert(borrowed.capture().unwrap()).is_none());
        source.profile[1] = 99;
        let duplicate = bank.insert(source.source(&thread).unwrap().capture().unwrap());
        assert!(duplicate.is_some());
        drop(duplicate);
        // New life in the same roster place gets a separate snapshot.
        unsafe { raw::player::spawned(source.player.as_mut_ptr().cast()).write(200) };
        let borrowed = source.source(&thread).unwrap();
        let second = borrowed.life();
        assert!(bank.insert(borrowed.capture().unwrap()).is_none());
        drop(source); // donor leaves and all native source memory goes away
        for (life, expected) in [(first, 17), (second, 99)] {
            let checkout = bank.checkout(life).unwrap();
            assert_eq!(equipment(&checkout.profile), expected);
            assert!(!bank.can_destroy());
            assert!(matches!(bank.checkout(life), Err(ProfileError::Busy)));
            bank.restore(checkout);
            assert!(bank.can_destroy());
        }
        assert!(matches!(
            bank.checkout(Life {
                spawned: 300,
                ..first
            }),
            Err(ProfileError::MissingTemplate)
        ));
        // Teardown destroys each retained profile exactly once; stubs verify
        // native destructor-before-free, including the duplicate candidate.
        drop(bank);
    }

    #[test]
    fn rejects_unbound_players_and_never_uses_another_places_profile() {
        let thread = unsafe { Thread::new() };
        for (place, spawned, valid) in [
            (0, 0, true),
            (19, 100, true),
            (20, 100, false),
            (1, u32::MAX, false),
        ] {
            let mut source = Source::new(1, place, spawned, 17);
            assert_eq!(source.source(&thread).is_some(), valid);
        }
        let mut source = Source::new(1, 2, 100, 17);
        unsafe { raw::player::inserted(source.player.as_mut_ptr().cast()).write(0) };
        assert!(source.source(&thread).is_none());
        unsafe {
            raw::player::inserted(source.player.as_mut_ptr().cast()).write(1);
            raw::player::core(source.player.as_mut_ptr().cast()).write(0);
        }
        assert!(source.source(&thread).is_none());
        unsafe {
            raw::player::core(source.player.as_mut_ptr().cast()).write(1);
            raw::player::profile(source.player.as_mut_ptr().cast()).write(0);
        }
        assert!(source.source(&thread).is_none());
    }
}
