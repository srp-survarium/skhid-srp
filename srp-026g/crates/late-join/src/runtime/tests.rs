//! The asynchronous lifecycle on the host (the first build's
//! test_late_join_runtime_026g.c): ownership, cancellation, retries and
//! cleanup; the native adapters are made up here.

use super::*;
use std::boxed::Box;
use std::sync::Mutex;
use std::sync::atomic::AtomicU32;
use std::vec;

/// The tests share the adapters' counters: one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

struct Counters {
    allocations: AtomicU32,
    frees: AtomicU32,
    queries: AtomicU32,
    retains: AtomicU32,
    releases: AtomicU32,
    installs: AtomicU32,
    acks: AtomicU32,
    last_serial: AtomicU32,
    last_status: AtomicU32,
}

static N: Counters = Counters {
    allocations: AtomicU32::new(0),
    frees: AtomicU32::new(0),
    queries: AtomicU32::new(0),
    retains: AtomicU32::new(0),
    releases: AtomicU32::new(0),
    installs: AtomicU32::new(0),
    acks: AtomicU32::new(0),
    last_serial: AtomicU32::new(0),
    last_status: AtomicU32::new(0),
};

fn get(n: &AtomicU32) -> u32 {
    n.load(Ordering::SeqCst)
}

fn bump(n: &AtomicU32) {
    n.fetch_add(1, Ordering::SeqCst);
}

// what the adapters do this time
static mut ALLOCATION_FAILS: bool = false;
static mut INSTALL_FAILS: bool = false;
/// 0: the query completes later; 1: at once; 2: deactivated and completed with nothing at once.
static mut QUERY_MODE: u32 = 0;
static mut DESTROY_ON_RELEASE: *const c_void = core::ptr::null();
static mut QUERY: *mut Context = null_mut();
static mut STATE: *mut State = null_mut();

// the game's objects, by address
static CLIENT: u8 = 0;
static WORLD: u8 = 0;
static DICTIONARY: u8 = 0;
static OTHER_WORLD: u8 = 0;
static PLAYER_A: u8 = 0;
static PLAYER_B: u8 = 0;

fn at(object: &'static u8) -> *mut c_void {
    (object as *const u8).cast_mut().cast()
}

fn completed() -> [*mut c_void; 3] {
    [at(&PLAYER_A), null_mut(), at(&PLAYER_B)]
}

unsafe extern "C" fn allocate(size: u32) -> *mut c_void {
    if unsafe { ALLOCATION_FAILS } {
        return null_mut();
    }
    bump(&N.allocations);
    let layout = std::alloc::Layout::from_size_align(size as usize + 8, 8).unwrap();
    unsafe {
        let p = std::alloc::alloc(layout);
        p.cast::<usize>().write(size as usize + 8);
        p.add(8).cast()
    }
}

unsafe extern "C" fn release_memory(p: *mut c_void) {
    bump(&N.frees);
    unsafe {
        let p = p.cast::<u8>().sub(8);
        let size = p.cast::<usize>().read();
        std::alloc::dealloc(p, std::alloc::Layout::from_size_align(size, 8).unwrap());
    }
}

unsafe extern "C" fn decode(wire: *const u8, size: u32, profile: *mut c_void, dictionary: *mut c_void) -> i32 {
    assert_eq!(dictionary, at(&DICTIONARY));
    unsafe {
        if size != 1 || *wire != 0x42 {
            return 0;
        }
        let profile = profile.cast::<u8>();
        core::ptr::write_bytes(profile, 0, PROFILE_BYTES);
        *profile.add(0x57) = 3;
        *profile.add(0x758 + 0x704) = 2;
        *profile.add(8) = 0x42;
    }
    1
}

unsafe extern "C" fn retain_world(p: *mut c_void) {
    assert!(p == at(&WORLD) || p == at(&OTHER_WORLD));
    bump(&N.retains);
}

unsafe extern "C" fn release_world(p: *mut c_void) {
    assert!(p == at(&WORLD) || p == at(&OTHER_WORLD));
    bump(&N.releases);
    unsafe {
        if !DESTROY_ON_RELEASE.is_null() {
            let profile = DESTROY_ON_RELEASE;
            DESTROY_ON_RELEASE = core::ptr::null();
            player_destroyed(STATE, at(&PLAYER_A), profile);
            player_destroyed(STATE, at(&PLAYER_B), profile);
        }
    }
}

unsafe extern "C" fn start_query(c: *mut Context) {
    bump(&N.queries);
    unsafe {
        QUERY = c;
        assert_eq!((*c).kit_mask, 5);
        assert_eq!(*(&raw const (*c).profile).cast::<u8>().add(8), 0x42);
        if QUERY_MODE == 1 {
            complete(c, &completed());
        }
        if QUERY_MODE == 2 {
            deactivate(STATE);
            complete(c, &[null_mut(); 3]); // can free c before the query returns
        }
    }
}

unsafe extern "C" fn install(c: *mut Context) -> i32 {
    bump(&N.installs);
    unsafe {
        let c = &*c;
        let s = &*STATE;
        assert!(c.query == Query::Running && c.of_match == Match::Current && c.world == s.world);
        assert!(c.place == s.roster && c.kit_mask == 5);
        assert!(c.players[0] == at(&PLAYER_A) && c.players[2] == at(&PLAYER_B));
        assert_eq!(*(&raw const c.profile).cast::<u8>().add(8), 0x42);
        i32::from(!INSTALL_FAILS)
    }
}

unsafe extern "C" fn ack(nc: *mut c_void, seed: u32, serial: u32, place: u8, status: u8) {
    unsafe {
        assert!(nc == at(&CLIENT) && seed == (*STATE).seed && place > 0 && place < 20);
    }
    bump(&N.acks);
    N.last_serial.store(serial, Ordering::SeqCst);
    N.last_status.store(u32::from(status), Ordering::SeqCst);
}

fn status() -> u32 {
    get(&N.last_status)
}

fn query() -> *mut Context {
    unsafe { QUERY }
}

/// A fresh state, activated for a match at roster place 2.
fn reset() -> *mut State {
    assert_eq!(get(&N.allocations), get(&N.frees));
    assert_eq!(get(&N.retains), get(&N.releases));
    let s = Box::into_raw(Box::new(State {
        lock: AtomicBool::new(false),
        contexts: null_mut(),
        api: Api {
            allocate: Some(allocate),
            free: Some(release_memory),
            decode: Some(decode),
            retain_world: Some(retain_world),
            release_world: Some(release_world),
            query: Some(start_query),
            install: Some(install),
            ack: Some(ack),
        },
        client: null_mut(),
        world: null_mut(),
        dictionary: null_mut(),
        seed: 0,
        roster: 0,
    }));
    for n in [&N.allocations, &N.frees, &N.queries, &N.retains, &N.releases, &N.installs, &N.acks, &N.last_serial,
              &N.last_status] {
        n.store(0, Ordering::SeqCst);
    }
    unsafe {
        ALLOCATION_FAILS = false;
        INSTALL_FAILS = false;
        QUERY_MODE = 0;
        DESTROY_ON_RELEASE = core::ptr::null();
        QUERY = null_mut();
        STATE = s;
        activate(s, at(&CLIENT), at(&WORLD), at(&DICTIONARY), 0x8000_0001, 2);
    }
    s
}

fn packet(serial: u8) -> [u8; 15] {
    let mut bytes = [255, b'L', b'J', 1, 1, 0, 0, 128, 0, 0, 0, 0, 2, 4, 0x42];
    bytes[8] = serial;
    bytes
}

fn profile_of(c: *mut Context) -> *const c_void {
    unsafe { (&raw const (*c).profile).cast() }
}

#[test]
fn successful_load() {
    let _one = SERIAL.lock().unwrap();
    let s = reset();
    unsafe {
        let mut bytes = packet(1);
        prepare(s, &bytes);
        assert!(get(&N.queries) == 1 && get(&N.retains) == 1 && get(&N.acks) == 0);
        assert!(*(*query()).wire() == 0x42 && (*query()).squad == 4);
        prepare(s, &bytes);
        assert!(get(&N.queries) == 1 && status() == Status::Busy as u32);
        bytes[14] = 0x43;
        prepare(s, &bytes);
        assert!(get(&N.queries) == 1 && status() == Status::Failed as u32 && *(*query()).wire() == 0x42);
        prepare(s, &packet(3));
        assert_eq!((*query()).serial, 3);
        // old proposals cannot replace the newest callback's acknowledgement
        prepare(s, &packet(2));
        assert_eq!((*query()).serial, 3);
        complete(query(), &completed());
        assert!(get(&N.installs) == 1 && (*s).roster == 3 && get(&N.releases) == 1);
        assert!(get(&N.last_serial) == 3 && status() == Status::Installed as u32);
        prepare(s, &packet(2));
        assert!(get(&N.last_serial) == 2 && status() == Status::Installed as u32 && get(&N.queries) == 1);
        let profile = profile_of(query());
        deactivate(s);
        assert_eq!(get(&N.frees), 0);
        player_destroyed(s, at(&PLAYER_A), profile);
        assert!(get(&N.frees) == 0 && *profile.cast::<u8>().add(8) == 0x42);
        player_destroyed(s, at(&PLAYER_B), profile);
        assert!(get(&N.frees) == 1 && (*s).contexts.is_null());
    }
}

#[test]
fn cancelled_load() {
    let _one = SERIAL.lock().unwrap();
    let s = reset();
    unsafe {
        prepare(s, &packet(1));
        let profile = profile_of(query());
        activate(s, at(&CLIENT), at(&OTHER_WORLD), at(&DICTIONARY), 0x8000_0002, 2);
        assert!((*query()).of_match == Match::Retired && get(&N.frees) == 0 && get(&N.releases) == 0);
        complete(query(), &completed());
        assert!(get(&N.installs) == 0 && get(&N.acks) == 0 && (*s).roster == 2 && get(&N.releases) == 1);
        player_destroyed(s, at(&PLAYER_B), profile);
        assert_eq!(get(&N.frees), 0);
        player_destroyed(s, at(&PLAYER_A), profile);
        assert_eq!(get(&N.frees), 1);
        deactivate(s);
    }
}

#[test]
fn partial_failure() {
    let _one = SERIAL.lock().unwrap();
    let s = reset();
    unsafe {
        prepare(s, &packet(1));
        let profile = profile_of(query());
        complete(query(), &[at(&PLAYER_A), null_mut(), null_mut()]);
        assert!(get(&N.installs) == 0 && status() == Status::Failed as u32 && (*s).roster == 2);
        player_destroyed(s, at(&PLAYER_A), profile);
        assert_eq!(get(&N.frees), 0); // a failed proposal is kept, unchanged, until the match retires
        prepare(s, &packet(1));
        assert!(get(&N.queries) == 1 && status() == Status::Failed as u32);
        deactivate(s);
        assert_eq!(get(&N.frees), 1);
    }
}

#[test]
fn rejected_install() {
    let _one = SERIAL.lock().unwrap();
    let s = reset();
    unsafe {
        INSTALL_FAILS = true;
        prepare(s, &packet(1));
        let profile = profile_of(query());
        complete(query(), &completed());
        assert!(get(&N.installs) == 1 && status() == Status::Failed as u32 && (*s).roster == 2);
        player_destroyed(s, at(&PLAYER_A), profile);
        player_destroyed(s, at(&PLAYER_B), profile);
        deactivate(s);
    }
}

#[test]
fn bad_requests() {
    let _one = SERIAL.lock().unwrap();
    let s = reset();
    unsafe {
        let bytes = packet(1);
        for n in 0..14 {
            prepare(s, &bytes[..n]);
        }
        assert!(get(&N.queries) == 0 && get(&N.acks) == 0 && get(&N.allocations) == 0);
        let mut bytes = packet(1);
        bytes[4] = 2; // another seed
        prepare(s, &bytes);
        assert_eq!(get(&N.acks), 0);
        let mut bytes = packet(1);
        bytes[12] = 3; // not the roster's next place
        prepare(s, &bytes);
        assert!(status() == Status::Failed as u32 && get(&N.allocations) == 0);
        let mut bytes = packet(1);
        bytes[14] = 0x43; // a descriptor the decoder refuses
        prepare(s, &bytes);
        assert!(status() == Status::Failed as u32 && get(&N.allocations) == get(&N.frees) && get(&N.retains) == 0);
        ALLOCATION_FAILS = true;
        prepare(s, &packet(1));
        assert!(status() == Status::Busy as u32 && get(&N.queries) == 0);
        let big = vec![0u8; MAX_PACKET + 1];
        let acks = get(&N.acks);
        prepare(s, &big);
        assert_eq!(get(&N.acks), acks);
        deactivate(s);
    }
}

#[test]
fn immediate_callbacks() {
    let _one = SERIAL.lock().unwrap();
    let s = reset();
    unsafe {
        QUERY_MODE = 1;
        prepare(s, &packet(1));
        assert!(get(&N.installs) == 1 && status() == Status::Installed as u32);
        let profile = profile_of(query());
        deactivate(s);
        player_destroyed(s, at(&PLAYER_A), profile);
        player_destroyed(s, at(&PLAYER_B), profile);
        let s = reset();
        QUERY_MODE = 2;
        prepare(s, &packet(1));
        assert!(get(&N.frees) == 1 && get(&N.releases) == 1 && get(&N.acks) == 0 && get(&N.installs) == 0);
        let s = reset();
        prepare(s, &packet(1));
        DESTROY_ON_RELEASE = profile_of(query());
        deactivate(s);
        complete(query(), &completed());
        assert!(get(&N.frees) == 1 && get(&N.releases) == 1 && get(&N.installs) == 0);
    }
}

#[test]
fn concurrent_destruction() {
    let _one = SERIAL.lock().unwrap();
    for _ in 0..50 {
        let s = reset();
        unsafe {
            prepare(s, &packet(1));
            complete(query(), &completed());
            let profile = profile_of(query()) as usize;
            let state = s as usize;
            let destroy = move |player: usize| {
                std::thread::spawn(move || player_destroyed(state as *mut State, player as *mut c_void, profile as *const c_void))
            };
            let first = destroy(at(&PLAYER_A) as usize);
            let second = destroy(at(&PLAYER_B) as usize);
            deactivate(s);
            first.join().unwrap();
            second.join().unwrap();
            assert!(get(&N.frees) == 1 && (*s).contexts.is_null());
        }
    }
}
