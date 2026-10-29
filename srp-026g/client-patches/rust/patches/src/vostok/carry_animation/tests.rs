//! Rust ownership gate. The original constructors/binder/destructors are
//! independently exercised by tools/re-kit/v026g/check_portable_carry.py.
use super::*;
use std::boxed::Box;
use std::cell::RefCell;
use std::vec::Vec;

#[derive(Debug, PartialEq)]
enum Call {
    Allocate,
    Construct(u32, u32),
    Bind(u32),
    DestroyHit(u32),
    DestroySelector(u32),
    Free(u32),
}

#[derive(Default)]
enum Heap {
    #[default]
    Available,
    Exhausted,
}

std::thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static HEAP: RefCell<Heap> = const { RefCell::new(Heap::Available) };
}

fn record(call: Call) {
    CALLS.with_borrow_mut(|calls| calls.push(call));
}

#[unsafe(no_mangle)]
unsafe extern "C" fn engine_malloc(_: *mut c_void, size: u32, _: *const c_void) -> *mut c_void {
    record(Call::Allocate);
    assert_eq!(size as usize, size_of::<Storage>());
    HEAP.with_borrow(|heap| match heap {
        Heap::Available => Box::into_raw(Box::new(Storage([0; 0x200]))).cast(),
        Heap::Exhausted => core::ptr::null_mut(),
    })
}

#[unsafe(no_mangle)]
unsafe extern "C" fn engine_portable_construct(storage: *mut c_void, seed: u32) -> *mut c_void {
    record(Call::Construct(storage as u32, seed));
    storage
}

#[unsafe(no_mangle)]
unsafe extern "C" fn engine_portable_bind_resource(core: *mut c_void, resource: *const u32) {
    record(Call::Bind(core as u32));
    // SAFETY: the caller supplies its live fixture resource and allocation.
    unsafe {
        let resource = resource.read() as *mut u32;
        *resource += 2;
        raw::portable_core::user_animations(core).write(resource as u32);
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn engine_hit_selector_destroy(selector: *mut c_void) {
    // SAFETY: the selector is the member of this fixture's owned storage.
    let core = unsafe { selector.byte_sub(0x60) };
    record(Call::DestroyHit(core as u32));
    // SAFETY: retained fixture resource; the hit selector releases one ref.
    unsafe {
        let resource = raw::portable_core::user_animations(core).read() as *mut u32;
        *resource -= 1;
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn engine_selector_destroy(selector: *mut c_void) {
    // SAFETY: as engine_hit_selector_destroy; the other subobject.
    let core = unsafe { selector.byte_sub(8) };
    record(Call::DestroySelector(core as u32));
    // SAFETY: as above, the remaining selector resource reference.
    unsafe {
        let resource = raw::portable_core::user_animations(core).read() as *mut u32;
        *resource -= 1;
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn engine_free(_: *mut c_void, memory: *mut c_void, pointer: *mut *mut c_void) {
    record(Call::Free(memory as u32));
    // SAFETY: the sole owner passes the same pointer allocated by our heap.
    unsafe {
        assert_eq!(pointer.read(), memory);
        drop(Box::from_raw(memory.cast::<Storage>()));
        pointer.write(core::ptr::null_mut());
    }
}

fn object<T>(value: &mut T) -> Object {
    Object::at(core::ptr::from_mut(value) as u32).unwrap()
}

#[test]
fn independent_owners_release_resources_in_native_order() {
    // SAFETY: fixture objects and the simulated hook outlive both owners.
    let thread = unsafe { Thread::new() };
    CALLS.with_borrow_mut(Vec::clear);
    let (mut references, mut allocator) = (1u32, 0u32);
    let resource = object(&mut references);
    let allocator = object(&mut allocator);
    let first = Prepared::prepare(resource, allocator, 123, &thread).unwrap();
    let second = Prepared::prepare(resource, allocator, 456, &thread).unwrap();
    let (a, b) = (first.storage.id(), second.storage.id());
    assert_ne!(a, b);
    assert_eq!(references, 5);
    drop(first);
    assert_eq!(references, 3);
    drop(second);
    assert_eq!(references, 1);
    CALLS.with_borrow(|calls| {
        assert_eq!(
            calls.as_slice(),
            &[
                Call::Allocate,
                Call::Construct(a, 123),
                Call::Bind(a),
                Call::Allocate,
                Call::Construct(b, 456),
                Call::Bind(b),
                Call::DestroyHit(a),
                Call::DestroySelector(a),
                Call::Free(a),
                Call::DestroyHit(b),
                Call::DestroySelector(b),
                Call::Free(b),
            ]
        );
    });
}

#[test]
fn allocation_failure_never_binds_or_releases_the_source() {
    // SAFETY: bounded fixture hook.
    let thread = unsafe { Thread::new() };
    CALLS.with_borrow_mut(Vec::clear);
    HEAP.with_borrow_mut(|heap| *heap = Heap::Exhausted);
    let (mut references, mut allocator) = (1u32, 0u32);
    assert!(
        Prepared::prepare(
            object(&mut references),
            object(&mut allocator),
            123,
            &thread
        )
        .is_none()
    );
    HEAP.with_borrow_mut(|heap| *heap = Heap::Available);
    assert_eq!(references, 1);
    CALLS.with_borrow(|calls| assert_eq!(calls.as_slice(), &[Call::Allocate]));
}

#[test]
fn held_interface_is_parsed_before_weapon_fields_are_read() {
    let mut player = [0u32; 0x130 / 4];
    // A non-weapon allocation has only its vtable: reading +0x278 would
    // already exceed it. The parser must stop at the known-vtable check.
    let mut non_weapon = 0x1234u32;
    let mut weapon = [0u32; 0x280 / 4];
    let mut portable = Storage([0; 0x200]);
    let mut resource = 1u32;
    let player = object(&mut player);
    assert!(Prepared::resource_for(player).is_none());
    // SAFETY: all fields below belong to these live, sized fixture buffers.
    unsafe {
        raw::player::held_item(player.ptr()).write(object(&mut non_weapon).id());
        assert!(Prepared::resource_for(player).is_none());
        let weapon = object(&mut weapon);
        raw::player::held_item(player.ptr()).write(weapon.id());
        for table in [raw::image::WEAPON_VTABLE, raw::image::WEAPON_CORE_VTABLE] {
            raw::active_item::vtable(weapon.ptr()).write(table);
            assert!(Prepared::resource_for(player).is_none());
            raw::weapon_core::portable(weapon.ptr()).write(object(&mut portable).id());
            assert!(Prepared::resource_for(player).is_none());
            raw::portable_core::user_animations(object(&mut portable).ptr())
                .write(object(&mut resource).id());
            assert_eq!(Prepared::resource_for(player), Some(object(&mut resource)));
            raw::portable_core::user_animations(object(&mut portable).ptr()).write(0);
            raw::weapon_core::portable(weapon.ptr()).write(0);
        }
    }
}
