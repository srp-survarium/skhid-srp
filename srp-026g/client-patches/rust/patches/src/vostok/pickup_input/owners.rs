//! Match-lifetime ownership for native pickup input journals. Each core gets
//! a separate allocation; destruction detaches it before native cleanup.
//! No Rust state is appended to the engine's embedded core object.
use super::ordinary::OrdinaryRelay;
use super::*;
use crate::vostok::resources::{
    ProfileBank, ProfileCheckout, ProfileError, ProfileSource, ProfileTemplate,
};
use crate::vostok::{Context, Lock, Reg};
use core::ptr::NonNull;
use weapon_pickup::targeting::{FrameEvent, Frames};

pub struct InputJournals<const N: usize> {
    worlds: Lock<Worlds<N>>,
}

impl<const N: usize> Default for InputJournals<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> InputJournals<N> {
    pub const fn new() -> Self {
        Self {
            worlds: Lock::new(Worlds { head: None }),
        }
    }

    /// 2D020A: the core constructor has completed, EBX is the live core.
    pub fn constructed(&self, ctx: &Context<'_>) {
        let core = core_at(ctx, Reg::Ebx);
        // Allocate outside the registry lock. Native allocation cannot hold
        // a mutable borrow of the registry across an allocator callback.
        let owner = Owned::new(core.identity()).expect("cannot allocate pickup input journal");
        self.worlds
            .with(ctx.thread(), |worlds| worlds.insert(owner));
    }

    /// 2D02A6: EBP is core, before any destructor has run. Detach first, then
    /// free outside the lock. Address reuse starts with a fresh empty journal.
    pub fn destroying(&self, ctx: &Context<'_>) {
        let core = core_at(ctx, Reg::Ebp);
        let removed = self
            .worlds
            .with(ctx.thread(), |worlds| worlds.remove(core.identity()));
        assert!(
            removed.is_some(),
            "destroyed native core has no pickup journal"
        );
        drop(removed);
    }

    pub fn begin_draw(&self, ctx: &Context<'_>) {
        self.draw_frame(ctx, FrameEvent::Begin);
    }

    pub fn complete_draw(&self, ctx: &Context<'_>) {
        self.draw_frame(ctx, FrameEvent::Complete);
    }

    fn draw_frame(&self, ctx: &Context<'_>, event: FrameEvent) {
        let Some(core) = super::drawn::drawing_core(ctx) else {
            return;
        };
        self.worlds.with(ctx.thread(), |worlds| {
            if let Some(world) = worlds.find_mut(core) {
                world.views.transition(event);
            }
        });
    }

    pub fn drawn(&self, ctx: &Context<'_>) {
        let Some(drawn) = DrawnWeapon::at_held_draw(ctx) else {
            return;
        };
        self.worlds.with(ctx.thread(), |worlds| {
            if let Some(world) = worlds.find_mut(drawn.core) {
                world.views.transition(FrameEvent::Draw(drawn.view));
            }
        });
    }

    /// Retain the first original equipment snapshot for this source life.
    /// Donor respawn/leave never replaces it; history and borrowed weapons
    /// can still refer to that life. Allocation/copy/destruction stay outside
    /// the registry lock, including the duplicate-after-copy case.
    pub fn remember_profile(&self, source: &ProfileSource<'_>) -> Result<bool, ProfileError> {
        let present = self.worlds.with(source.thread(), |worlds| {
            worlds
                .find_mut(source.core_id())
                .map(|world| world.profiles.contains(source.life()))
                .ok_or(ProfileError::MissingWorld)
        })?;
        if present {
            return Ok(false);
        }
        let mut candidate = Some(source.capture()?);
        let result = self.worlds.with(source.thread(), |worlds| {
            let world = worlds
                .find_mut(source.core_id())
                .ok_or(ProfileError::MissingWorld)?;
            candidate = world.profiles.insert(candidate.take().unwrap());
            Ok(candidate.is_none())
        });
        drop(candidate);
        result
    }

    /// Run preparation with an independently owned source profile. No match
    /// registry borrow or lock crosses the callback. The guard restores the
    /// profile after success, error or unwind; nested checkout of that same
    /// source returns Busy, while other sources/worlds remain accessible.
    pub fn with_profile<R>(
        &self,
        core: CoreId,
        source: weapon_pickup::state::Life,
        thread: &Thread,
        action: impl FnOnce(&ProfileTemplate) -> R,
    ) -> Result<R, ProfileError> {
        let checkout = self.worlds.with(thread, |worlds| {
            worlds
                .find_mut(core)
                .ok_or(ProfileError::MissingWorld)?
                .profiles
                .checkout(source)
        })?;
        let pending = PendingProfile {
            owner: self,
            thread,
            core,
            checkout: Some(checkout),
        };
        Ok(action(&pending.checkout.as_ref().unwrap().profile))
    }

    /// Attach only after native local placement completes. The caller selects
    /// the target from this core's published frame and current ownership state.
    pub fn attach_local(
        &self,
        receipt: CompletedLocalInput<'_>,
        target: DropId,
    ) -> Result<AcceptedPickup, LocalAttachError> {
        self.worlds.with(receipt.core._thread, |worlds| {
            let state = &mut worlds.get_mut(receipt.core_id()).history;
            if matches!(state, Tracking::Dormant) {
                *state = Tracking::Ready(InputHistory::new());
            }
            receipt.attach(
                target,
                state.observed().expect("active local pickup journal"),
            )
        })
    }

    pub fn retimed(&self, receipt: RetimedInput<'_>) {
        let id = receipt.core_id();
        self.worlds.with(receipt.core._thread, |worlds| {
            if let Some(history) = worlds.get_mut(id).history.observed() {
                receipt.apply(history);
            }
        });
    }

    pub fn moved(&self, receipt: CompletedMove<'_>) {
        let id = receipt.core_id();
        self.worlds.with(receipt.core._thread, |worlds| {
            let state = &mut worlds.get_mut(id).history;
            match core::mem::replace(state, Tracking::Dormant) {
                Tracking::Dormant => {}
                Tracking::Moving {
                    mut history,
                    source,
                } => {
                    assert_eq!(
                        source,
                        receipt.source(),
                        "native move changed source identity"
                    );
                    let _ = receipt.apply(&mut history);
                    *state = Tracking::Ready(history);
                }
                _ => panic!("native move completed without its journal transaction"),
            }
        });
    }

    /// 27D7F5, after source extraction but before inner placement can evict
    /// and reuse its storage. The completion hook handles every return here.
    pub fn moving(&self, ctx: &Context<'_>) {
        let core = core_at(ctx, Reg::Eax);
        let source = weapon_pickup::input_history::NodeKey::new(ctx.reg(Reg::Ebx)).unwrap();
        self.worlds.with(ctx.thread(), |worlds| {
            let state = &mut worlds.get_mut(core.identity()).history;
            *state = match core::mem::replace(state, Tracking::Dormant) {
                Tracking::Dormant => Tracking::Dormant,
                Tracking::Ready(history) => Tracking::Moving { history, source },
                _ => panic!("nested native pickup input transaction"),
            };
        });
    }

    pub fn discarded(&self, receipt: CompletedDiscard<'_>) {
        let id = receipt.core_id();
        self.worlds.with(receipt.core._thread, |worlds| {
            if let Some(history) = worlds.get_mut(id).history.observed() {
                receipt.apply(history);
            }
        });
    }

    pub fn end_relay(&self, ctx: &Context<'_>) {
        let Some((relay, result)) = OrdinaryRelay::completed(ctx) else {
            return;
        };
        self.worlds.with(ctx.thread(), |worlds| {
            if let Some(history) = worlds.get_mut(relay.core.identity()).history.observed() {
                relay.finish(result, history);
            }
        });
    }

    /// Replace one ordinary outgoing input with its targeted form. Packet
    /// allocation/writing/queueing happen only after releasing the journal lock.
    pub fn send(&self, ctx: &Context<'_>) -> bool {
        let input = super::sending::SendingInput::at(ctx);
        let target = self.worlds.with(ctx.thread(), |worlds| {
            worlds
                .get_mut(input.core_id())
                .history
                .observed()
                .and_then(|history| input.target(history))
        });
        let Some(target) = target else {
            return false;
        };
        input.send(target);
        true
    }

    /// 2D48C7: nonempty generic list, before its oldest node is unlinked.
    pub fn evicting(&self, ctx: &Context<'_>) {
        self.worlds.with(ctx.thread(), |worlds| {
            let mut cursor = worlds.head.as_mut();
            while let Some(entry) = cursor {
                let world = entry.world_mut();
                // SAFETY: registry membership lasts exactly the native core
                // lifetime; this boundary reads no native callbacks.
                let core =
                    unsafe { InputCore::from_raw(ctx.thread(), world.core.0.get()) }.unwrap();
                if core.owns_list(ctx.reg(Reg::Eax)) {
                    if let Tracking::Ready(history) = &mut world.history {
                        let list = Object::at(ctx.reg(Reg::Eax)).unwrap();
                        // SAFETY: this site is reached only with a nonempty
                        // list, before unlinking or returning its head to pool.
                        let node = unsafe { *raw::input_list::first(list.ptr()) };
                        history
                            .apply(Change::Retire(
                                weapon_pickup::input_history::NodeKey::new(node).unwrap(),
                            ))
                            .unwrap();
                    }
                    // Receiving owns its journal outside the lock. Its native
                    // placement transaction reconciles surviving nodes and the
                    // reused node generation before attaching its new target.
                    // Moving likewise preserves the extracted original until
                    // its completion can transfer it, even if its source is
                    // the very node evicted by the inner placement.
                    // Dormant worlds have no targets to retire.
                    return;
                }
                cursor = world.next.as_mut();
            }
        });
    }

    /// The match owner coordinates target placement and journal ownership.
    /// No registry borrow/lock is held while calling native input code.
    pub fn receive(
        &self,
        core: &mut InputCore<'_>,
        picker: Place,
        input: WireInput,
        clock: GameTime,
    ) -> Result<Acceptance, ReceiveError> {
        self.receiving(core.identity(), core._thread, |history| {
            core.receive(picker, input, clock, history)
        })?
    }

    fn receiving<R>(
        &self,
        core: CoreId,
        thread: &Thread,
        action: impl FnOnce(&mut InputHistory<N>) -> R,
    ) -> Result<R, ReceiveError> {
        let history = self.worlds.with(thread, |worlds| {
            let state = &mut worlds.get_mut(core).history;
            if matches!(state, Tracking::Receiving | Tracking::Moving { .. }) {
                return Err(ReceiveError::ReceiveInProgress);
            }
            Ok(match core::mem::replace(state, Tracking::Receiving) {
                Tracking::Dormant => InputHistory::new(),
                Tracking::Ready(history) => history,
                Tracking::Receiving | Tracking::Moving { .. } => unreachable!(),
            })
        })?;
        let mut pending = PendingReceive {
            owner: self,
            thread,
            core,
            history: Some(history),
        };
        // The guard restores ownership on every return, including native
        // refusal and Rust test unwinding. A nested receive cannot manufacture
        // an empty replacement journal while this one is checked out.
        Ok(action(pending.history.as_mut().unwrap()))
    }
}

struct PendingReceive<'a, const N: usize> {
    owner: &'a InputJournals<N>,
    thread: &'a Thread,
    core: CoreId,
    history: Option<InputHistory<N>>,
}

struct PendingProfile<'a, const N: usize> {
    owner: &'a InputJournals<N>,
    thread: &'a Thread,
    core: CoreId,
    checkout: Option<ProfileCheckout>,
}

impl<const N: usize> Drop for PendingProfile<'_, N> {
    fn drop(&mut self) {
        let checkout = self.checkout.take().unwrap();
        self.owner.worlds.with(self.thread, |worlds| {
            worlds.get_mut(self.core).profiles.restore(checkout);
        });
    }
}

impl<const N: usize> Drop for PendingReceive<'_, N> {
    fn drop(&mut self) {
        let history = self.history.take().unwrap();
        self.owner.worlds.with(self.thread, |worlds| {
            let state = &mut worlds.get_mut(self.core).history;
            assert!(
                matches!(state, Tracking::Receiving),
                "pickup receive lost journal ownership"
            );
            *state = Tracking::Ready(history);
        });
    }
}

fn core_at<'t>(ctx: &'t Context<'_>, register: Reg) -> InputCore<'t> {
    // SAFETY: the named constructor/destructor sites supply their core in
    // these registers. Neither operation accesses already-destroyed fields.
    unsafe { InputCore::from_raw(ctx.thread(), ctx.reg(register)) }
        .expect("native core lifecycle hook has no core")
}

struct World<const N: usize> {
    core: CoreId,
    views: Frames,
    profiles: ProfileBank,
    // Dormant until the first targeted input. Ordinary gameplay must not
    // scan thousands of empty journal slots on every local input merge.
    history: Tracking<N>,
    next: Option<Owned<N>>,
}

enum Tracking<const N: usize> {
    Dormant,
    Ready(InputHistory<N>),
    Moving {
        history: InputHistory<N>,
        source: weapon_pickup::input_history::NodeKey,
    },
    Receiving,
}

impl<const N: usize> Tracking<N> {
    fn observed(&mut self) -> Option<&mut InputHistory<N>> {
        match self {
            Self::Dormant => None,
            Self::Ready(history) => Some(history),
            Self::Receiving | Self::Moving { .. } => {
                panic!("unexpected timing mutation during pickup input placement")
            }
        }
    }
}

struct Owned<const N: usize> {
    ptr: NonNull<World<N>>,
    #[cfg(not(test))]
    allocator: Object,
}

#[derive(Debug)]
enum AllocationError {
    #[cfg(not(test))]
    NoAllocator,
    #[cfg(not(test))]
    OutOfMemory,
}

impl<const N: usize> Owned<N> {
    fn new(core: CoreId) -> Result<Self, AllocationError> {
        let world = World {
            core,
            views: Frames::new(),
            profiles: ProfileBank::new(core),
            history: Tracking::Dormant,
            next: None,
        };
        #[cfg(not(test))]
        {
            // SAFETY: lifecycle hook runs after engine allocator startup.
            // Its binding returns aligned native object storage. The private
            // owner initializes exactly one World there and frees it once.
            let allocator =
                Object::at(unsafe { (raw::image::GAME_ALLOCATOR as *const u32).read() })
                    .ok_or(AllocationError::NoAllocator)?;
            let ptr = NonNull::new(
                unsafe {
                    raw::engine_malloc(
                        allocator.ptr(),
                        size_of::<World<N>>() as u32,
                        c"weapon pickup input journal".as_ptr().cast(),
                    )
                }
                .cast::<World<N>>(),
            )
            .ok_or(AllocationError::OutOfMemory)?;
            unsafe { ptr.as_ptr().write(world) };
            Ok(Self { ptr, allocator })
        }
        #[cfg(test)]
        {
            // Host tests exercise ownership with the host allocator; native
            // malloc/free calling conventions are checked by the ABI suite.
            Ok(Self {
                ptr: NonNull::from(std::boxed::Box::leak(std::boxed::Box::new(world))),
            })
        }
    }

    fn world(&self) -> &World<N> {
        // SAFETY: this owner uniquely owns a constructed, stable allocation.
        unsafe { self.ptr.as_ref() }
    }

    fn world_mut(&mut self) -> &mut World<N> {
        // SAFETY: exclusive access through the registry lock or detached owner.
        unsafe { self.ptr.as_mut() }
    }
}

impl<const N: usize> Drop for Owned<N> {
    fn drop(&mut self) {
        assert!(
            self.world().next.is_none(),
            "journal must be detached before release"
        );
        #[cfg(not(test))]
        unsafe {
            // SAFETY: the node is detached and uniquely owned. Drop its Rust
            // state before releasing storage with its original allocator.
            self.ptr.as_ptr().drop_in_place();
            let mut pointer = self.ptr.as_ptr().cast();
            raw::engine_free(self.allocator.ptr(), pointer, &mut pointer);
        }
        #[cfg(test)]
        unsafe {
            drop(std::boxed::Box::from_raw(self.ptr.as_ptr()));
        }
    }
}

struct Worlds<const N: usize> {
    head: Option<Owned<N>>,
}

impl<const N: usize> Worlds<N> {
    fn insert(&mut self, mut owner: Owned<N>) {
        let core = owner.world().core;
        let mut cursor = self.head.as_ref();
        while let Some(entry) = cursor {
            assert_ne!(
                entry.world().core,
                core,
                "native core constructed twice without teardown"
            );
            cursor = entry.world().next.as_ref();
        }
        owner.world_mut().next = self.head.take();
        self.head = Some(owner);
    }

    fn find_mut(&mut self, core: CoreId) -> Option<&mut World<N>> {
        let mut cursor = self.head.as_mut();
        while let Some(entry) = cursor {
            if entry.world().core == core {
                return Some(entry.world_mut());
            }
            cursor = entry.world_mut().next.as_mut();
        }
        None
    }

    fn get_mut(&mut self, core: CoreId) -> &mut World<N> {
        self.find_mut(core)
            .expect("native input mutation has no matching world journal")
    }

    fn remove(&mut self, core: CoreId) -> Option<Owned<N>> {
        let mut cursor = &mut self.head;
        loop {
            let current = cursor.as_ref()?;
            if current.world().core == core {
                assert!(
                    current.world().profiles.can_destroy(),
                    "native world destroyed during pickup profile query"
                );
                assert!(
                    !matches!(
                        current.world().history,
                        Tracking::Receiving | Tracking::Moving { .. }
                    ),
                    "native world destroyed during pickup input placement"
                );
                let mut removed = cursor.take().unwrap();
                *cursor = removed.world_mut().next.take();
                return Some(removed);
            }
            cursor = &mut cursor.as_mut().unwrap().world_mut().next;
        }
    }
}

impl<const N: usize> Drop for Worlds<N> {
    fn drop(&mut self) {
        while let Some(mut head) = self.head.take() {
            self.head = head.world_mut().next.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::{Regs, context::Frame};
    use weapon_pickup::input_history::NodeKey;
    use weapon_pickup::state::Life;

    fn id(n: u32) -> CoreId {
        CoreId(core::num::NonZeroU32::new(n).unwrap())
    }

    #[test]
    fn profile_checkout_releases_registry_and_restores_after_failure_and_unwind() {
        use crate::vostok::resources::profile_fixture::{Source, equipment};
        let thread = unsafe { Thread::new() };
        let journals = InputJournals::<2>::new();
        let mut donor = Source::new(1, 2, 100, 17);
        let life = donor.source(&thread).unwrap().life();
        assert_eq!(
            journals.remember_profile(&donor.source(&thread).unwrap()),
            Err(ProfileError::MissingWorld)
        );
        journals.worlds.with(&thread, |worlds| {
            worlds.insert(Owned::new(id(1)).unwrap());
            worlds.insert(Owned::new(id(2)).unwrap());
        });
        assert_eq!(
            journals.remember_profile(&donor.source(&thread).unwrap()),
            Ok(true)
        );
        donor.profile[1] = 99;
        assert_eq!(
            journals.remember_profile(&donor.source(&thread).unwrap()),
            Ok(false)
        );
        let mut other = Source::new(2, 2, 100, 42);
        journals
            .remember_profile(&other.source(&thread).unwrap())
            .unwrap();
        drop(other);
        drop(donor);
        let result = journals.with_profile(id(1), life, &thread, |profile| {
            assert_eq!(equipment(profile), 17);
            // This would deadlock if the registry lock crossed the callback.
            assert_eq!(
                journals.with_profile(id(1), life, &thread, |_| ()),
                Err(ProfileError::Busy)
            );
            assert_eq!(
                journals.with_profile(id(2), life, &thread, equipment),
                Ok(42)
            );
            journals.worlds.with(&thread, |worlds| {
                assert!(!worlds.get_mut(id(1)).profiles.can_destroy());
                assert!(worlds.get_mut(id(2)).profiles.can_destroy());
            });
            // Another life may be captured during native callback dispatch.
            let mut respawned = Source::new(1, 2, 200, 99);
            assert_eq!(
                journals.remember_profile(&respawned.source(&thread).unwrap()),
                Ok(true)
            );
            Err::<(), _>("resource query failed")
        });
        assert_eq!(result, Ok(Err("resource query failed")));
        assert_eq!(
            journals.with_profile(id(1), life, &thread, equipment),
            Ok(17)
        );
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = journals.with_profile(id(1), life, &thread, |_| panic!("query unwind"));
        }));
        assert!(unwind.is_err());
        assert_eq!(
            journals.with_profile(id(1), life, &thread, equipment),
            Ok(17)
        );
        assert_eq!(
            journals.with_profile(
                id(1),
                Life {
                    spawned: 200,
                    ..life
                },
                &thread,
                equipment
            ),
            Ok(99)
        );
        let removed = journals
            .worlds
            .with(&thread, |worlds| worlds.remove(id(1)).unwrap());
        drop(removed);
        assert_eq!(
            journals.with_profile(id(1), life, &thread, equipment),
            Err(ProfileError::MissingWorld)
        );
        journals
            .worlds
            .with(&thread, |worlds| worlds.insert(Owned::new(id(1)).unwrap()));
        assert_eq!(
            journals.with_profile(id(1), life, &thread, equipment),
            Err(ProfileError::MissingTemplate)
        );
        assert_eq!(
            journals.with_profile(id(2), life, &thread, equipment),
            Ok(42)
        );
    }

    #[test]
    fn presentation_is_match_owned_and_address_reuse_starts_empty() {
        use weapon_pickup::state::{HeldAtDeath, Kit, World as Simulation};
        use weapon_pickup::targeting::{RenderedDrop, Segment, WeaponBox};
        use weapon_pickup::{AmmoKind, Chamber, DroppedAmmo, Handedness, WeaponSlot};
        let life = Life {
            place: SharedPlace::parse(1).unwrap(),
            spawned: 700,
        };
        let mut simulation = Simulation::new();
        simulation.spawn(life, Kit::parse(0).unwrap());
        simulation.die(
            life,
            Some(HeldAtDeath {
                slot: WeaponSlot::First,
                handedness: Handedness::Two,
                ammo: DroppedAmmo::parse(30, 17, Chamber::Loaded, AmmoKind::parse(0).unwrap())
                    .unwrap(),
            }),
        );
        let view = RenderedDrop {
            id: DropId(life),
            bounds: WeaponBox::parse(
                [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
                [
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                ],
            )
            .unwrap(),
        };
        let ray = Segment::parse([-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]).unwrap();
        let mut worlds = Worlds::<2> { head: None };
        worlds.insert(Owned::new(id(1)).unwrap());
        worlds.insert(Owned::new(id(2)).unwrap());
        for event in [
            FrameEvent::Begin,
            FrameEvent::Draw(view),
            FrameEvent::Complete,
        ] {
            worlds.get_mut(id(1)).views.transition(event);
        }
        assert_eq!(
            worlds.get_mut(id(1)).views.select(&simulation, ray),
            Some(view.id)
        );
        assert_eq!(worlds.get_mut(id(2)).views.select(&simulation, ray), None);
        drop(worlds.remove(id(1)).unwrap());
        assert!(worlds.find_mut(id(1)).is_none());
        worlds.insert(Owned::new(id(1)).unwrap());
        assert_eq!(worlds.get_mut(id(1)).views.select(&simulation, ray), None);
    }

    #[test]
    fn eviction_filters_lists_and_preserves_extracted_target_until_move_finishes() {
        use core::ffi::c_void;
        let thread = unsafe { Thread::new() };
        let journals = InputJournals::<3>::new();
        let mut storage = std::vec![0u32; 0x19000 / 4];
        let mut lists = std::vec![0u32; Place::COUNT * 0x480 / 4];
        let mut source = [0u32; 10];
        let mut survivor = [0u32; 10];
        let core = storage.as_mut_ptr().cast::<c_void>();
        let list = lists.as_mut_ptr().cast::<c_void>();
        let source = source.as_mut_ptr().cast::<c_void>();
        let survivor = survivor.as_mut_ptr().cast::<c_void>();
        unsafe {
            raw::game_core::input_lists(core).write(list as u32);
            raw::input_list::first(list).write(source as u32);
            raw::input_node::time(source).write(800);
            raw::input_node::time(survivor).write(850);
            raw::input_node::next(source).write(survivor as u32);
        }
        let picker = SharedPlace::parse(0).unwrap();
        let target = |spawned| {
            DropId(Life {
                place: picker,
                spawned,
            })
        };
        let mut history = InputHistory::new();
        for (node, time, part, spawned) in [
            (source, 800, Part::Original, 100),
            (source, 800, Part::Corrected, 200),
            (survivor, 850, Part::Original, 300),
        ] {
            history
                .apply(Change::Accept {
                    node: NodeKey::new(node as u32).unwrap(),
                    picker,
                    time: Time(time),
                    part,
                    combine: Combine::Merge,
                    target: Some(target(spawned)),
                })
                .unwrap();
        }
        journals.worlds.with(&thread, |worlds| {
            worlds.insert(Owned::new(id(core as u32)).unwrap());
            worlds.get_mut(id(core as u32)).history = Tracking::Ready(history.clone());
        });
        let invoke = |eax, ebx, ebp, esp, hook: fn(&InputJournals<3>, &Context<'_>)| {
            let mut regs = Regs {
                eax,
                ebx,
                ebp,
                esp,
                ecx: 0,
                edx: 0,
                edi: 0,
                esi: 0,
                eflags: 0,
            };
            let saved = regs;
            let mut fxsave = [0u8; 512];
            let mut frame = Frame {
                regs: &mut regs,
                fxsave: fxsave.as_mut_ptr(),
            };
            hook(&journals, &unsafe { Context::new(&mut frame) });
            assert_eq!(regs, saved);
        };
        for unrelated in [
            1,
            list as u32 + 1,
            list as u32 + (Place::COUNT * 0x480) as u32,
        ] {
            invoke(unrelated, 0, 0, 0, InputJournals::evicting);
        }
        journals.worlds.with(&thread, |worlds| {
            assert_eq!(
                worlds.get_mut(id(core as u32)).history.observed().unwrap(),
                &history
            );
        });
        invoke(list as u32, 0, 0, 0, InputJournals::evicting);
        journals.worlds.with(&thread, |worlds| {
            let state = &mut worlds.get_mut(id(core as u32)).history;
            let remaining = state.observed().unwrap();
            assert_eq!(remaining.at(Time(800)).count(), 0);
            assert_eq!(
                remaining.at(Time(850)).collect::<std::vec::Vec<_>>(),
                [(picker, target(300))]
            );
            *state = Tracking::Ready(history.clone());
        });

        // Inner native placement evicts its own source's corrected portion
        // and reuses the same address. The extracted original must survive.
        invoke(core as u32, source as u32, 0, 0, InputJournals::moving);
        invoke(list as u32, 0, 0, 0, InputJournals::evicting);
        assert_eq!(
            journals.receiving(id(core as u32), &thread, |_| ()),
            Err(ReceiveError::ReceiveInProgress)
        );
        journals.worlds.with(&thread, |worlds| {
            let Tracking::Moving {
                history: pending, ..
            } = &worlds.get_mut(id(core as u32)).history
            else {
                panic!("move lost ownership")
            };
            assert_eq!(pending, &history);
        });
        unsafe {
            raw::input_list::first(list).write(survivor as u32);
            raw::input_node::next(survivor).write(source as u32);
            raw::input_node::next(source).write(0);
            raw::input_node::time(source).write(900);
        }
        let mut frame = [0u32, 0, core as u32, 0];
        let mut stack = [0u32; 8];
        stack[7] = 900;
        invoke(
            0,
            source as u32,
            frame.as_mut_ptr() as u32,
            (stack.as_mut_ptr() as u32).wrapping_sub(8),
            |owner, ctx| owner.moved(CompletedMove::at_end(ctx)),
        );
        journals.worlds.with(&thread, |worlds| {
            let remaining = worlds.get_mut(id(core as u32)).history.observed().unwrap();
            assert_eq!(remaining.at(Time(800)).count(), 0);
            assert_eq!(
                remaining.at(Time(850)).collect::<std::vec::Vec<_>>(),
                [(picker, target(300))]
            );
            assert_eq!(
                remaining.at(Time(900)).collect::<std::vec::Vec<_>>(),
                [(picker, target(100))]
            );
        });
        // A targeted receive also owns reconciliation outside the lock.
        journals
            .receiving(id(core as u32), &thread, |checked_out| {
                let before = checked_out.clone();
                invoke(list as u32, 0, 0, 0, InputJournals::evicting);
                assert_eq!(checked_out, &before);
            })
            .unwrap();
    }

    #[test]
    fn ordinary_completion_replaces_original_only_and_keeps_refused_inputs() {
        use core::ffi::c_void;
        let thread = unsafe { Thread::new() };
        let journals = InputJournals::<2>::new();
        let mut wrapper = std::vec![0u32; 0x19400 / 4];
        let mut client = std::vec![0u32; 0x1F1C68 / 4];
        let mut lists = std::vec![0u32; Place::COUNT * 0x480 / 4];
        let mut node = [0u32; 10];
        let mut input = [0u32; 4];
        let mut stack = [0u32; 6];
        let mut caller_frame = [0u32; 8];
        let wrapper = wrapper.as_mut_ptr().cast::<c_void>();
        let client = client.as_mut_ptr().cast::<c_void>();
        let node = node.as_mut_ptr().cast::<c_void>();
        let core = unsafe { raw::input_world_wrapper::core(wrapper) }.cast::<c_void>();
        unsafe {
            raw::network_client::match_world(client).write(wrapper as u32);
            raw::game_core::input_lists(core).write(lists.as_mut_ptr() as u32);
            raw::game_core::local_input_place(core).write(3);
            raw::input_list::first(lists.as_mut_ptr().cast::<u8>().add(3 * 0x480).cast())
                .write(node as u32);
            raw::input_node::time(node).write(923);
            raw::input_node::corrected(node).write(1);
            raw::player_input::place(input.as_mut_ptr().cast()).write(3);
        }
        journals.worlds.with(&thread, |worlds| {
            worlds.insert(Owned::new(id(core as u32)).unwrap())
        });
        for return_address in [0x27C32D, 0x27C5CD] {
            for status in [0, 1, 2, 3, 4, 6] {
                let picker = SharedPlace::parse(3).unwrap();
                let original = DropId(Life {
                    place: picker,
                    spawned: 50,
                });
                let corrected = DropId(Life {
                    place: picker,
                    spawned: 60,
                });
                journals.worlds.with(&thread, |worlds| {
                    let mut history = InputHistory::new();
                    for (part, target) in [(Part::Original, original), (Part::Corrected, corrected)]
                    {
                        history
                            .apply(Change::Accept {
                                node: NodeKey::new(node as u32).unwrap(),
                                picker,
                                time: Time(923),
                                part,
                                combine: Combine::Merge,
                                target: Some(target),
                            })
                            .unwrap();
                    }
                    worlds.get_mut(id(core as u32)).history = Tracking::Ready(history);
                });
                stack[0] = return_address;
                stack[5] = if [0, 2, 3].contains(&status) {
                    923
                } else {
                    GameTime::NONE_RAW
                };
                caller_frame[0] = stack[5];
                let mut regs = Regs {
                    eax: status,
                    edx: client as u32,
                    esi: input.as_mut_ptr() as u32,
                    esp: (stack.as_mut_ptr() as u32).wrapping_sub(8),
                    ebp: caller_frame.as_mut_ptr() as u32 + 16,
                    edi: 0,
                    ebx: 0,
                    ecx: 0,
                    eflags: 0,
                };
                let saved = regs;
                let mut fxsave = [0u8; 512];
                let mut frame = Frame {
                    regs: &mut regs,
                    fxsave: fxsave.as_mut_ptr(),
                };
                journals.end_relay(&unsafe { Context::new(&mut frame) });
                assert_eq!(regs, saved);
                journals.worlds.with(&thread, |worlds| {
                    let targets = worlds
                        .get_mut(id(core as u32))
                        .history
                        .observed()
                        .unwrap()
                        .at(Time(923))
                        .collect::<std::vec::Vec<_>>();
                    if status == 0 {
                        assert_eq!(targets, [(picker, corrected)]);
                    } else {
                        assert_eq!(targets, [(picker, original), (picker, corrected)]);
                    }
                });
            }
        }
        // Stats for unrelated native operations must not inspect a client or input.
        stack[0] = 0x1234;
        let mut regs = Regs {
            eax: 999,
            edx: 0,
            esi: 0,
            esp: (stack.as_mut_ptr() as u32).wrapping_sub(8),
            ebp: 0,
            edi: 0,
            ebx: 0,
            ecx: 0,
            eflags: 0,
        };
        let mut fxsave = [0u8; 512];
        let mut frame = Frame {
            regs: &mut regs,
            fxsave: fxsave.as_mut_ptr(),
        };
        journals.end_relay(&unsafe { Context::new(&mut frame) });
    }

    #[test]
    fn dispatch_consumes_relays_without_losing_targets_on_duplicate_or_rejection() {
        use super::super::tests::{NATIVE, input};
        use core::ffi::c_void;
        let thread = unsafe { Thread::new() };
        let journals = InputJournals::<2>::new();
        let mut wrapper = std::vec![0u32; 0x19400 / 4];
        let mut client = std::vec![0u32; 0x1F1C68 / 4];
        let mut lists = std::vec![0u32; Place::COUNT * 0x480 / 4];
        let mut node = [0u32; 10];
        let wrapper = wrapper.as_mut_ptr().cast::<c_void>();
        let client = client.as_mut_ptr().cast::<c_void>();
        let node = node.as_mut_ptr().cast::<c_void>();
        let core = unsafe { raw::input_world_wrapper::core(wrapper) }.cast::<c_void>();
        unsafe {
            raw::network_client::match_world(client).write(wrapper as u32);
            raw::network_client::match_state(client).write(2);
            raw::input_world_wrapper::clock(wrapper).write(1000);
            raw::game_core::input_lists(core).write(lists.as_mut_ptr() as u32);
            raw::input_list::first(lists.as_mut_ptr().cast::<u8>().add(3 * 0x480).cast())
                .write(node as u32);
            raw::input_node::time(node).write(923);
        }
        journals.worlds.with(&thread, |worlds| {
            worlds.insert(Owned::new(id(core as u32)).unwrap())
        });
        let dispatch = |message, bytes: &[u8]| {
            let mut reader = [
                bytes.as_ptr() as u32,
                bytes.as_ptr() as u32,
                bytes.len() as u32,
            ];
            let mut regs = Regs {
                eax: message,
                esi: client as u32,
                ebx: reader.as_mut_ptr() as u32,
                edi: 0,
                ebp: 0,
                esp: 0,
                edx: 0,
                ecx: 0,
                eflags: 0,
            };
            let original = regs;
            let mut fxsave = [0u8; 512];
            let mut frame = Frame {
                regs: &mut regs,
                fxsave: fxsave.as_mut_ptr(),
            };
            let ctx = unsafe { Context::new(&mut frame) };
            let result = journals.packet(&ctx);
            assert_eq!(regs, original);
            assert_eq!(
                reader[1],
                reader[0]
                    + if message == transport::RELAY_ID {
                        bytes.len() as u32
                    } else {
                        0
                    }
            );
            result
        };
        let mut payload = [0u8; weapon_pickup::input::BYTES + 1];
        payload[0] = 3;
        payload[1..].copy_from_slice(&input().bytes());
        NATIVE.with_borrow_mut(|v| *v = (0, Some(923), std::vec::Vec::new()));
        assert_eq!(dispatch(0xEB, &payload), None);
        unsafe { raw::network_client::match_state(client).write(1) };
        assert_eq!(
            dispatch(transport::RELAY_ID, &payload),
            Some(Ok(PacketOutcome::OutsideMatch))
        );
        unsafe {
            raw::network_client::match_state(client).write(2);
            raw::network_client::match_world(client).write(0);
        }
        assert_eq!(
            dispatch(transport::RELAY_ID, &payload),
            Some(Ok(PacketOutcome::OutsideMatch))
        );
        NATIVE.with_borrow(|v| assert!(v.2.is_empty()));
        unsafe { raw::network_client::match_world(client).write(wrapper as u32) };
        assert_eq!(
            dispatch(transport::RELAY_ID, &payload),
            Some(Ok(PacketOutcome::Input(Acceptance::Placed(
                AcceptedPickup {
                    picker: Place::new(3).unwrap(),
                    time: GameTime(923),
                    target: input().target
                }
            ))))
        );
        NATIVE.with_borrow(|v| assert_eq!(v.2.len(), 1));
        let preserved = || {
            journals.worlds.with(&thread, |worlds| {
                assert_eq!(
                    worlds
                        .get_mut(id(core as u32))
                        .history
                        .observed()
                        .unwrap()
                        .at(Time(923))
                        .collect::<std::vec::Vec<_>>(),
                    [(SharedPlace::parse(3).unwrap(), input().target)]
                );
            })
        };
        preserved();
        for (status, accepted) in [
            (1, Acceptance::Duplicate),
            (4, Acceptance::Refused(Refused::OutsideHistory)),
            (6, Acceptance::Refused(Refused::BeforeResync)),
        ] {
            NATIVE.with_borrow_mut(|v| *v = (status, None, std::vec::Vec::new()));
            let mut retry = payload;
            retry[12..].copy_from_slice(&100u32.to_le_bytes());
            assert_eq!(
                dispatch(transport::RELAY_ID, &retry),
                Some(Ok(PacketOutcome::Input(accepted)))
            );
            NATIVE.with_borrow(|v| assert_eq!(v.2.len(), 1));
            preserved();
        }
        NATIVE.with_borrow_mut(|v| *v = (0, Some(923), std::vec::Vec::new()));
        for len in 0..payload.len() {
            assert_eq!(
                dispatch(transport::RELAY_ID, &payload[..len]),
                Some(Err(PacketError::Payload))
            );
        }
        assert_eq!(
            dispatch(transport::RELAY_ID, &[0; 17]),
            Some(Err(PacketError::Payload))
        );
        for (offset, value, error) in [
            (0, 20, PacketError::Picker),
            (2, 50, PacketError::Payload),
            (11, 32, PacketError::Payload),
            (
                11,
                20,
                PacketError::Placement(ReceiveError::TargetOutsideRoster),
            ),
            (
                4,
                0x7C,
                PacketError::Placement(ReceiveError::InvalidRotation),
            ),
        ] {
            let mut bad = payload;
            bad[offset] = value;
            assert_eq!(dispatch(transport::RELAY_ID, &bad), Some(Err(error)));
            preserved();
        }
        NATIVE.with_borrow(|v| assert!(v.2.is_empty()));
        journals.worlds.with(&thread, |worlds| {
            worlds
                .get_mut(id(core as u32))
                .history
                .observed()
                .unwrap()
                .apply(Change::Accept {
                    node: NodeKey::new(node as u32).unwrap(),
                    picker: SharedPlace::parse(3).unwrap(),
                    time: Time(923),
                    part: Part::Corrected,
                    combine: Combine::Merge,
                    target: Some(input().target),
                })
                .unwrap();
        });
        assert_eq!(
            dispatch(transport::RELAY_ID, &payload),
            Some(Err(PacketError::Placement(ReceiveError::HistoryFull)))
        );
        NATIVE.with_borrow(|v| assert!(v.2.is_empty()));
        preserved();
    }

    #[test]
    fn receive_releases_registry_and_restores_journal_after_refusal_and_unwind() {
        let journals = InputJournals::<2>::new();
        let thread = unsafe { Thread::new() };
        journals.worlds.with(&thread, |worlds| {
            worlds.insert(Owned::new(id(1)).unwrap());
            worlds.insert(Owned::new(id(2)).unwrap());
        });
        let picker = SharedPlace::parse(0).unwrap();
        let target = DropId(Life {
            place: SharedPlace::parse(1).unwrap(),
            spawned: 50,
        });
        let result = journals.receiving(id(1), &thread, |history| {
            history
                .apply(Change::Accept {
                    node: NodeKey::new(7).unwrap(),
                    picker,
                    time: Time(800),
                    part: Part::Original,
                    combine: Combine::Merge,
                    target: Some(target),
                })
                .unwrap();
            // Native callbacks can reach the registry while this core's
            // journal is checked out. Other worlds remain independent.
            journals.worlds.with(&thread, |worlds| {
                assert!(matches!(worlds.get_mut(id(1)).history, Tracking::Receiving));
                assert!(matches!(worlds.get_mut(id(2)).history, Tracking::Dormant));
            });
            assert_eq!(
                journals.receiving(id(1), &thread, |_| panic!("nested receive ran")),
                Err::<(), _>(ReceiveError::ReceiveInProgress)
            );
            journals
                .receiving(id(2), &thread, |other| {
                    assert_eq!(other.at(Time(800)).count(), 0);
                })
                .unwrap();
            Err::<(), _>(ReceiveError::HistoryFull)
        });
        assert_eq!(result, Ok(Err(ReceiveError::HistoryFull)));
        let check = || {
            journals.worlds.with(&thread, |worlds| {
                let history = worlds.get_mut(id(1)).history.observed().unwrap();
                assert_eq!(
                    history.at(Time(800)).collect::<std::vec::Vec<_>>(),
                    [(picker, target)]
                );
            })
        };
        check();
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = journals.receiving(id(1), &thread, |_| panic!("placement failed"));
        }));
        assert!(unwind.is_err());
        check();
        // Restoration also makes teardown legal again.
        journals.worlds.with(&thread, |worlds| {
            drop(worlds.remove(id(1)).unwrap());
            drop(worlds.remove(id(2)).unwrap());
        });
    }

    #[test]
    fn independent_worlds_detach_in_any_order_and_address_reuse_is_empty() {
        let mut worlds = Worlds::<2> { head: None };
        for n in 1..=3 {
            worlds.insert(Owned::new(id(n)).unwrap());
            worlds.get_mut(id(n)).history = Tracking::Ready(InputHistory::new());
            worlds
                .get_mut(id(n))
                .history
                .observed()
                .unwrap()
                .apply(Change::Accept {
                    node: NodeKey::new(n).unwrap(),
                    picker: SharedPlace::parse(0).unwrap(),
                    time: Time(800),
                    part: Part::Original,
                    combine: Combine::Merge,
                    target: Some(DropId(Life {
                        place: SharedPlace::parse(1).unwrap(),
                        spawned: n,
                    })),
                })
                .unwrap();
        }
        // Middle removal leaves both neighbours and their targets untouched.
        drop(worlds.remove(id(2)).unwrap());
        worlds.insert(Owned::new(id(2)).unwrap());
        assert!(matches!(worlds.get_mut(id(2)).history, Tracking::Dormant));
        for n in [1, 3] {
            assert_eq!(
                worlds
                    .get_mut(id(n))
                    .history
                    .observed()
                    .unwrap()
                    .at(Time(800))
                    .next()
                    .unwrap()
                    .1
                    .0
                    .spawned,
                n
            );
        }
        drop(worlds.remove(id(1)).unwrap()); // tail
        drop(worlds.remove(id(2)).unwrap()); // head
        assert!(worlds.remove(id(2)).is_none());
        drop(worlds.remove(id(3)).unwrap());
        assert!(worlds.head.is_none());
    }
}
