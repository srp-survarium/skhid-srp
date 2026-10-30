//! Inventory resource cooking through the native synchronous query wrapper.
//! Query inputs and callback storage stay alive until the native completion
//! proxy returns. A successful resource owns one retained native reference.
//! Prepared resources attach through one installation/restoration owner.
//! Its unsafe boundary still requires a match owner to establish native
//! lifetimes and snapshot ordering; no gameplay hook calls it yet.

use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, Ordering};
use weapon_pickup::state::Kit;

use super::hook::{Handle, Thread};
use super::world::{Object, pointer, read};
use crate::raw;

mod profiles;
mod match_context;
pub use match_context::{MatchPrepareError, MatchResources};
mod weapon_profile;
pub(in crate::vostok) use profiles::{ProfileBank, ProfileCheckout};
pub use profiles::{ProfileError, ProfileSource};

mod prepared;
pub use prepared::SnapshotWeapon;
pub(in crate::vostok) use prepared::decode_pickup_slots;
pub use prepared::{
    AmmunitionSupply, CrateCompletion, CrateInventory, PrepareError, PreparedWeapon, ResupplyError,
};
pub use prepared::{CrateAvailability, CrateError};
pub use prepared::{DecodedPickups, InstallError, InstalledPickup};
pub(in crate::vostok) use prepared::{complete_crate, crate_availability};

/// The original inventory cook's arguments, borrowed from a valid native
/// hook. Their profile and context objects must outlive this hook.
pub struct InventoryRecipe<'t> {
    words: [u32; 7],
    _thread: &'t Thread,
}

impl<'t> Handle<'t> for InventoryRecipe<'t> {
    unsafe fn from_raw(thread: &'t Thread, address: u32) -> Option<Self> {
        let object = Object::at(address)?;
        // SAFETY: the hook supplies a live inventory_cooker_data (1Ch bytes).
        // Read only initialized fields; native padding is not copied.
        let words = unsafe {
            [
                read(raw::inventory_recipe::profile(object.ptr())),
                u32::from(read(raw::inventory_recipe::kit(object.ptr()))),
                read(raw::inventory_recipe::dictionary(object.ptr())),
                read(raw::inventory_recipe::physics_world(object.ptr())),
                read(raw::inventory_recipe::game_world(object.ptr())),
                read(raw::inventory_recipe::match_seed(object.ptr())),
                u32::from(read(raw::inventory_recipe::flag(object.ptr()))),
            ]
        };
        if words[0] == 0 || words[1] >= 3 || words[2] == 0 {
            return None;
        }
        Some(Self {
            words,
            _thread: thread,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CookError {
    NoAllocator,
    OutOfMemory,
    MissingCook,
    ReusableCook,
    NoResource,
    WrongResource,
    MissingCallback,
}

/// Independent original equipment data for a match participant. Native
/// profile assignment copies attachments/upgrades into this allocation's
/// inline buffers; no pointer into the donor's profile is retained. The
/// match owner must drop templates before the engine allocator shuts down.
pub struct ProfileTemplate {
    this: Object,
    allocator: Object,
}

// player_profile's native allocation size: constructor 2F73F0, original
// lobby allocations, and check_pickup_profile.py's guarded copy/teardown.
const PROFILE_BYTES: usize = 0x4634;

impl ProfileTemplate {
    /// # Safety
    /// `source` is a live constructed 45FCh profile body; `allocator` is the
    /// initialized game allocator. This operation does not dispatch queries.
    unsafe fn copy_from(source: *const c_void, allocator: Object) -> Result<Self, CookError> {
        // SAFETY: the native allocator returns stable storage suitable for
        // native objects; constructors establish all self-relative vectors.
        let this = unsafe {
            Object::at(raw::engine_malloc(
                allocator.ptr(),
                PROFILE_BYTES as u32,
                c"weapon pickup profile".as_ptr().cast(),
            ) as u32)
        }
        .ok_or(CookError::OutOfMemory)?;
        unsafe {
            this.ptr().cast::<u8>().write_bytes(0, PROFILE_BYTES);
            raw::engine_profile_construct(this.ptr());
            // Late join's profile allocation has only the 45FCh body.
            // Copying the client-only tail would read past that allocation.
            raw::engine_profile_copy_body(this.ptr(), source);
        }
        Ok(Self { this, allocator })
    }
}

impl Drop for ProfileTemplate {
    fn drop(&mut self) {
        // SAFETY: one native allocation owned by this object, with a fully
        // constructed profile. Its destructor empties the inline vectors;
        // the same allocator then frees their containing allocation once.
        unsafe {
            raw::engine_profile_destroy(self.this.ptr());
            let mut pointer = self.this.ptr();
            raw::engine_free(self.allocator.ptr(), pointer, &mut pointer);
        }
    }
}

/// Owns the resource, rather than borrowing the queries_result's reference.
/// Object's NonNull keeps this !Send/!Sync; it stays on the native hook thread.
pub struct OwnedInventory {
    this: Object,
}

impl OwnedInventory {
    pub fn same_instance(&self, other: &Self) -> bool {
        self.this == other.this
    }
}

impl Drop for OwnedInventory {
    fn drop(&mut self) {
        // SAFETY: this owner holds one reference until this point; the native
        // resource manager handles the last-reference destruction path.
        unsafe {
            let count = raw::unmanaged_resource::references(self.this.ptr());
            if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_object(self.this.ptr(), count);
            }
        }
    }
}

impl InventoryRecipe<'_> {
    /// Capture original equipment while the native recipe's profile is
    /// live. The resulting template can outlive the donor and their loadout.
    pub fn snapshot_profile(&self) -> Result<ProfileTemplate, CookError> {
        let allocator = game_allocator()?;
        // SAFETY: Handle's live recipe contract includes its profile; native
        // copying does not run resource callbacks or keep source pointers.
        unsafe { ProfileTemplate::copy_from(self.words[0] as *const c_void, allocator) }
    }

    /// Cook original equipment using a currently live native resource context.
    ///
    /// # Safety
    /// As `cook`; this recipe's dictionary/context must belong to the same
    /// match as the template. The template stays borrowed until completion.
    pub unsafe fn cook_from_template(
        &self,
        template: &ProfileTemplate,
        kit: Kit,
    ) -> Result<OwnedInventory, CookError> {
        let recipe = self.using_template(template, kit);
        // SAFETY: caller establishes the dispatch point and match context;
        // using_template ties the profile pointer to template's borrow.
        unsafe { recipe.cook() }
    }

    fn using_template<'a>(
        &'a self,
        template: &'a ProfileTemplate,
        kit: Kit,
    ) -> InventoryRecipe<'a> {
        let mut words = self.words;
        words[0] = template.this.id();
        words[1] = u32::from(kit.get());
        InventoryRecipe {
            words,
            _thread: self._thread,
        }
    }

    /// Low-level preparation boundary. Gameplay must reach this through a
    /// Vostok owner at a verified resource-dispatch point.
    ///
    /// # Safety
    /// The current native hook must permit resource callback dispatch. No
    /// simulation/ownership lock or borrow that those callbacks can reenter
    /// may be held. The recipe's profile and context objects must remain
    /// alive throughout that dispatch, including nested native callbacks.
    /// A generic Thread token alone does not establish these conditions.
    pub unsafe fn cook(&self) -> Result<OwnedInventory, CookError> {
        independent_cooks()?;
        self.query(game_allocator()?.id())
    }

    fn query(&self, allocator: u32) -> Result<OwnedInventory, CookError> {
        let mut variant = [0u32; 12];
        variant[0] = raw::image::INVENTORY_RECIPE_HELPER;
        variant[2] = self.words.as_ptr() as u32;
        variant[10] = variant.as_ptr() as u32;
        // SAFETY: native process-local type ID, same function as the recipe's
        // builder and inventory cook's variant extractor.
        variant[11] = unsafe { raw::engine_inventory_recipe_type() };
        let user = variant.as_ptr() as u32;
        let request = [c"inventory".as_ptr() as u32, 0x41];
        let mut completion = Completion::Pending;
        let mut parameters = [0u32; 25];
        parameters[0] = request.as_ptr() as u32;
        parameters[2] = 1;
        // boost::function (32 bytes): trivial-copy tagged vtable, then its
        // function buffer at +8. The native proof exercises this exact ABI.
        parameters[4] = (&INVENTORY_CALLBACK as *const CallbackVtable as u32) | 1;
        parameters[6] = (&raw mut completion) as u32;
        parameters[12] = allocator;
        parameters[15] = (&raw const user) as u32;
        parameters[24] = 1;
        // SAFETY: all requests, variant/cookie data and callback state remain
        // live on this stack until the native wrapper completes the callback.
        // The callback retains the result before native query references go.
        unsafe { raw::engine_query_resources_wait(parameters.as_ptr().cast()) };
        match completion {
            Completion::Pending => Err(CookError::MissingCallback),
            Completion::Finished(result) => result,
        }
    }
}

/// Inventory, weapon and mutable ammo items must all be separate resources.
/// Class 43h is reusable immutable config; class 48h creates ammo instances.
fn independent_cooks() -> Result<(), CookError> {
    for class_id in [0x41, 0x46, 0x48] {
        // SAFETY: fixed valid native class IDs. Cook registration precedes
        // profile/inventory preparation; lookup can return null, handled here.
        let cook = unsafe { Object::at(raw::engine_resource_cook(class_id) as u32) }
            .ok_or(CookError::MissingCook)?;
        // SAFETY: the registered resource cook's native reuse policy.
        if unsafe { read(raw::resource_cook::reuse(cook.ptr())) } != 0 {
            return Err(CookError::ReusableCook);
        }
    }
    Ok(())
}

#[cfg(not(test))]
fn game_allocator() -> Result<Object, CookError> {
    // SAFETY: engine global initialized before an inventory recipe exists.
    let allocator = unsafe { read(raw::image::GAME_ALLOCATOR as *mut u32) };
    Object::at(allocator).ok_or(CookError::NoAllocator)
}

#[cfg(test)]
fn game_allocator() -> Result<Object, CookError> {
    // Unit fixtures replace native heap/profile calls below. Original-exe
    // fixtures exercise the actual global allocator boundary separately.
    Ok(Object::at(7).unwrap())
}

enum Completion {
    Pending,
    Finished(Result<OwnedInventory, CookError>),
}

/// A native trivial-copy boost::function vtable. The tagged bit causes all
/// copy/destruction paths to skip manager; only invoke is callable.
#[repr(C)]
struct CallbackVtable {
    manager: usize,
    invoke: unsafe extern "C" fn(*const u32, *mut c_void),
}

static INVENTORY_CALLBACK: CallbackVtable = CallbackVtable {
    manager: 0,
    invoke: raw::callback_inventory_ready,
};

pub(super) unsafe fn inventory_done(buffer: *const u32, result: *mut c_void) {
    // SAFETY: the native wrapper invokes the supplied function buffer and
    // live one-request queries_result before returning to InventoryRecipe.
    let completion = unsafe { &mut *(*buffer as *mut Completion) };
    assert!(
        matches!(completion, Completion::Pending),
        "resource callback invoked twice"
    );
    let resource = if result.is_null() {
        None
    } else {
        unsafe { pointer(raw::queries_result::first_unmanaged(result)) }
    };
    let outcome = match resource {
        None => Err(CookError::NoResource),
        Some(this) => {
            // SAFETY: a successful query owns this live unmanaged resource.
            if unsafe { read(this.ptr().cast::<u32>()) } != raw::image::INVENTORY_VTABLE {
                Err(CookError::WrongResource)
            } else {
                // SAFETY: retain while the native query still holds its ref.
                unsafe {
                    AtomicU32::from_ptr(raw::unmanaged_resource::references(this.ptr()))
                        .fetch_add(1, Ordering::AcqRel);
                }
                Ok(OwnedInventory { this })
            }
        }
    };
    *completion = Completion::Finished(outcome);
}

#[cfg(test)]
pub(in crate::vostok) use profiles::fixture as profile_fixture;

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::{Cell, RefCell};
    use std::boxed::Box;
    use std::vec::Vec;

    std::thread_local! {
        static RESULT: Cell<u32> = const { Cell::new(0) };
        static RELEASED: Cell<u32> = const { Cell::new(0) };
        static PROFILE_STEPS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
        static FAIL_ALLOCATION: Cell<bool> = const { Cell::new(false) };
        static QUERIED_PROFILE: Cell<u32> = const { Cell::new(0) };
        static QUERIED_SLOTS: RefCell<Option<[u32; 69]>> = const { RefCell::new(None) };
        static COOKS: RefCell<[[u32; 4]; 3]> = const { RefCell::new([[0; 4]; 3]) };
        static MISSING_COOK: Cell<u32> = const { Cell::new(0) };
    }

    #[unsafe(no_mangle)]
    extern "C" fn engine_resource_cook(class_id: u32) -> *mut c_void {
        let index = match class_id {
            0x41 => 0,
            0x46 => 1,
            0x48 => 2,
            _ => panic!("unexpected resource class"),
        };
        if MISSING_COOK.get() == class_id {
            core::ptr::null_mut()
        } else {
            // Stable thread-local storage, borrowed only during this call.
            COOKS.with_borrow_mut(|cooks| cooks[index].as_mut_ptr().cast())
        }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_malloc(
        allocator: *mut c_void,
        bytes: u32,
        _description: *const c_void,
    ) -> *mut c_void {
        assert_eq!(allocator as u32, 7);
        assert_eq!(bytes as usize, PROFILE_BYTES);
        PROFILE_STEPS.with_borrow_mut(|steps| steps.push("allocate"));
        if FAIL_ALLOCATION.get() {
            core::ptr::null_mut()
        } else {
            Box::into_raw(Box::new([0u32; PROFILE_BYTES / 4])).cast()
        }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_profile_construct(profile: *mut c_void) -> *mut c_void {
        PROFILE_STEPS.with_borrow_mut(|steps| steps.push("construct"));
        unsafe { profile.cast::<u32>().write(0xC07) };
        profile
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_profile_copy_body(
        target: *mut c_void,
        source: *const c_void,
    ) -> *mut c_void {
        PROFILE_STEPS.with_borrow_mut(|steps| steps.push("copy"));
        // The original native copy is exercised by check_pickup_profile.py.
        // This stub checks constructor-before-copy and independently owned
        // storage, then copies a sentinel through the actual pointer ABI.
        unsafe {
            assert_eq!(*target.cast::<u32>(), 0xC07);
            assert_ne!(source, target);
            target.cast::<u32>().write(*source.cast::<u32>());
            target
                .cast::<u32>()
                .add(1)
                .write(*source.cast::<u32>().add(1));
            // Scalar directories used by weapon-profile filtering. Native
            // self-relative item-buffer copying is covered by the exe fixture.
            let mut source = source.cast_mut();
            let mut target = target;
            raw::player_profile::kit_count(target)
                .write(read(raw::player_profile::kit_count(source)));
            raw::player_profile::item_count(target)
                .write(read(raw::player_profile::item_count(source)));
            for _ in 0..3 {
                for slot in 0..23 {
                    raw::player_profile::kit_slots(target)
                        .add(slot)
                        .write(read(raw::player_profile::kit_slots(source).add(slot)));
                }
                source = raw::profile_kit::end(source).cast();
                target = raw::profile_kit::end(target).cast();
            }
        }
        target
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_profile_destroy(profile: *mut c_void) {
        PROFILE_STEPS.with_borrow_mut(|steps| steps.push("destroy"));
        unsafe {
            assert_eq!(*profile.cast::<u32>(), 0xC0FFEE);
            profile.cast::<u32>().write(0xDEAD);
        }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_free(
        allocator: *mut c_void,
        memory: *mut c_void,
        pointer: *mut *mut c_void,
    ) {
        assert_eq!(allocator as u32, 7);
        PROFILE_STEPS.with_borrow_mut(|steps| steps.push("free"));
        unsafe {
            assert_eq!(*pointer, memory);
            assert_eq!(*memory.cast::<u32>(), 0xDEAD);
            drop(Box::from_raw(memory.cast::<[u32; PROFILE_BYTES / 4]>()));
            *pointer = core::ptr::null_mut();
        }
    }

    // Native builds use the generated RUST_CDECL entry. This Rust-only
    // fixture enters with Rust's ABI alignment; Unicorn covers the native
    // entry separately, including all four native stack alignments.
    #[unsafe(no_mangle)]
    unsafe extern "C" fn callback_inventory_ready(buffer: *const u32, result: *mut c_void) {
        unsafe { inventory_done(buffer, result) }
    }

    #[unsafe(no_mangle)]
    extern "C" fn engine_inventory_recipe_type() -> u32 {
        73
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_release_object(object: *mut c_void, count: *mut u32) {
        assert_eq!(unsafe { *count }, 0);
        RELEASED.set(object as u32);
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_query_resources_wait(parameters: *const c_void) {
        // SAFETY: query() supplied these fixed-size stack arrays. This fixture
        // models only submission/result lifetime; the separate Unicorn test
        // runs the original callback copying and waiting machinery.
        unsafe {
            let p = &*parameters.cast::<[u32; 25]>();
            assert_eq!(p[2], 1);
            assert_eq!(p[24], 1);
            assert_eq!(*(p[0] as *const u32).add(1), 0x41);
            let user = *(p[15] as *const u32) as *const u32;
            assert_eq!(*user.add(11), 73);
            assert_eq!(*user.add(10), user as u32);
            let recipe = *user.add(2) as *const u32;
            QUERIED_PROFILE.set(*recipe);
            QUERIED_SLOTS.with_borrow_mut(|capture| {
                if let Some(slots) = capture {
                    let mut base = *recipe as *mut c_void;
                    for kit in 0..3 {
                        for slot in 0..23 {
                            slots[kit * 23 + slot] =
                                read(raw::player_profile::kit_slots(base).add(slot));
                        }
                        base = raw::profile_kit::end(base).cast();
                    }
                }
            });
            assert_eq!(*recipe.add(1), 2);
            assert_eq!(*recipe.add(6), 1);
            let resource = RESULT.get();
            let mut results = [0u32; 80];
            raw::queries_result::first_unmanaged(results.as_mut_ptr().cast()).write(resource);
            let table = &*((p[4] & !1) as *const CallbackVtable);
            assert_eq!(p[4] & 1, 1);
            (table.invoke)(p.as_ptr().add(6), results.as_mut_ptr().cast());
            if resource != 0 {
                // Original query releases its own ref after callback finishes.
                let count = raw::unmanaged_resource::references(resource as *mut c_void);
                *count -= 1;
            }
        }
    }

    #[test]
    fn owns_the_result_after_query_teardown_and_releases_exactly_once() {
        let mut resource = Box::new([0u32; 110]);
        resource[0] = raw::image::INVENTORY_VTABLE;
        let object = resource.as_mut_ptr().cast::<c_void>();
        unsafe {
            raw::unmanaged_resource::references(object).write(1);
        }
        RESULT.set(object as u32);
        RELEASED.set(0);
        let thread = unsafe { Thread::new() };
        let recipe = InventoryRecipe {
            words: [1, 2, 1, 0, 0, 0, 1],
            _thread: &thread,
        };
        let owned = recipe.query(1).unwrap();
        assert_eq!(unsafe { *raw::unmanaged_resource::references(object) }, 1);
        assert_eq!(RELEASED.get(), 0);
        drop(owned);
        assert_eq!(RELEASED.get(), object as u32);
        assert_eq!(unsafe { *raw::unmanaged_resource::references(object) }, 0);
    }

    #[test]
    fn rejects_failed_or_wrong_type_results_without_retaining_them() {
        let thread = unsafe { Thread::new() };
        let recipe = InventoryRecipe {
            words: [1, 2, 1, 0, 0, 0, 1],
            _thread: &thread,
        };
        RESULT.set(0);
        assert!(matches!(recipe.query(1), Err(CookError::NoResource)));
        let mut resource = Box::new([0u32; 110]);
        let object = resource.as_mut_ptr().cast::<c_void>();
        unsafe {
            raw::unmanaged_resource::references(object).write(1);
        }
        RESULT.set(object as u32);
        assert!(matches!(recipe.query(1), Err(CookError::WrongResource)));
        assert_eq!(unsafe { *raw::unmanaged_resource::references(object) }, 0);
    }

    #[test]
    fn copies_only_initialized_native_recipe_fields() {
        let mut native: [u32; 7] = [1, 0xFFFF_FF02, 1, 17, 19, 23, 0xFFFF_FF01];
        let thread = unsafe { Thread::new() };
        let recipe =
            unsafe { InventoryRecipe::from_raw(&thread, native.as_mut_ptr() as u32) }.unwrap();
        assert_eq!(recipe.words, [1, 2, 1, 17, 19, 23, 1]);
        native[1] = 3;
        assert!(
            unsafe { InventoryRecipe::from_raw(&thread, native.as_mut_ptr() as u32) }.is_none()
        );
    }

    #[test]
    fn template_survives_donor_and_uses_current_context_then_destroys_before_free() {
        PROFILE_STEPS.with_borrow_mut(Vec::clear);
        let mut source = Box::new([0u32; PROFILE_BYTES / 4]);
        source[0] = 0xC0FFEE;
        let template =
            unsafe { ProfileTemplate::copy_from(source.as_ptr().cast(), Object::at(7).unwrap()) }
                .unwrap();
        source[0] = 123;
        drop(source);
        let thread = unsafe { Thread::new() };
        let context = InventoryRecipe {
            words: [1, 0, 1, 17, 19, 23, 1],
            _thread: &thread,
        };
        let recipe = context.using_template(&template, Kit::parse(2).unwrap());
        assert_eq!(&recipe.words[1..], &[2, 1, 17, 19, 23, 1]);
        RESULT.set(0);
        assert!(matches!(recipe.query(7), Err(CookError::NoResource)));
        assert_eq!(QUERIED_PROFILE.get(), template.this.id());
        assert_eq!(unsafe { *template.this.ptr().cast::<u32>() }, 0xC0FFEE);
        drop(template);
        PROFILE_STEPS.with_borrow(|steps| {
            assert_eq!(steps, &["allocate", "construct", "copy", "destroy", "free"]);
        });
    }

    #[test]
    fn weapon_query_owns_filtered_profile_until_failure_then_releases_only_its_copy() {
        use weapon_pickup::{AmmoKind, Chamber, DroppedAmmo, Handedness, WeaponSlot};
        let mut source = Box::new([0u32; PROFILE_BYTES / 4]);
        source[0] = 0xC0FFEE;
        let template = unsafe {
            let profile = source.as_mut_ptr().cast();
            raw::player_profile::kit_count(profile).write(3);
            raw::player_profile::item_count(profile).write(69);
            let mut base = profile;
            for kit in 0..3 {
                for slot in 0..23 {
                    raw::player_profile::kit_slots(base)
                        .add(slot)
                        .write((120 << 16) | (kit * 23 + slot) as u32);
                }
                base = raw::profile_kit::end(base).cast();
            }
            ProfileTemplate::copy_from(profile, Object::at(7).unwrap()).unwrap()
        };
        let thread = unsafe { Thread::new() };
        let context = InventoryRecipe {
            words: [1, 0, 1, 17, 19, 23, 1],
            _thread: &thread,
        };
        PROFILE_STEPS.with_borrow_mut(Vec::clear);
        QUERIED_SLOTS.with_borrow_mut(|capture| *capture = Some([0; 69]));
        RESULT.set(0);
        let ammo = DroppedAmmo::parse(30, 7, Chamber::Loaded, AmmoKind::parse(0).unwrap()).unwrap();
        let result = unsafe {
            context.prepare_weapon(
                &template,
                Kit::parse(2).unwrap(),
                WeaponSlot::First,
                Handedness::Two,
                ammo,
            )
        };
        assert!(matches!(
            result,
            Err(PrepareError::Cook(CookError::NoResource))
        ));
        assert_ne!(QUERIED_PROFILE.get(), template.this.id());
        let slots = QUERIED_SLOTS.with_borrow_mut(Option::take).unwrap();
        for kit in 0..3 {
            for slot in 0..23 {
                let keep =
                    kit == 2 && (slot == 7 || weapon_profile::AMMO_SLOTS.contains(&(slot as u32)));
                assert_eq!(
                    slots[kit * 23 + slot],
                    if keep {
                        (120 << 16) | (kit * 23 + slot) as u32
                    } else {
                        u32::from(u16::MAX)
                    }
                );
            }
        }
        PROFILE_STEPS.with_borrow(|steps| {
            assert_eq!(steps, &["allocate", "construct", "copy", "destroy", "free"])
        });
        unsafe {
            // The bank's original source still contains unrelated equipment.
            assert_eq!(
                read(raw::player_profile::kit_slots(template.this.ptr())),
                120 << 16
            );
        }
        drop(template);
    }

    #[test]
    fn template_allocation_failure_never_constructs_or_frees_a_null_profile() {
        PROFILE_STEPS.with_borrow_mut(Vec::clear);
        FAIL_ALLOCATION.set(true);
        let source = 0xC0FFEEu32;
        let result = unsafe {
            ProfileTemplate::copy_from((&raw const source).cast(), Object::at(7).unwrap())
        };
        FAIL_ALLOCATION.set(false);
        assert!(matches!(result, Err(CookError::OutOfMemory)));
        PROFILE_STEPS.with_borrow(|steps| assert_eq!(steps, &["allocate"]));
    }

    #[test]
    fn refuses_missing_or_reusable_inventory_weapon_and_ammo_cooks() {
        assert_eq!(independent_cooks(), Ok(()));
        for index in 0..3 {
            COOKS.with_borrow_mut(|cooks| cooks[index][3] = 1);
            assert_eq!(independent_cooks(), Err(CookError::ReusableCook));
            COOKS.with_borrow_mut(|cooks| cooks[index][3] = 0);
        }
        for class_id in [0x41, 0x46, 0x48] {
            MISSING_COOK.set(class_id);
            assert_eq!(independent_cooks(), Err(CookError::MissingCook));
        }
        MISSING_COOK.set(0);
        assert_eq!(independent_cooks(), Ok(()));
    }
}
