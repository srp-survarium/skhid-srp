//! Ownership and admission of a proposal (the server's E4 extension "LJ"):
//! its descriptor kept, the game's loader asked for its players, the
//! install once they are made, the acknowledgement; and the descriptor kept
//! for as long as any of its players lives.
//!
//! The native adapters (late_join_026g.asm) implement [`Api`]: begin,
//! complete and activate run on the game's thread; a player's destruction
//! may arrive from a resource thread. No adapter is called while the list's
//! lock is held. The asynchronous query owns its result's references through
//! the return from [`complete`], as the game's own queries_result does.
//!
//! The layouts are the first build's C (`#[repr(C)]`): the assembly reads
//! the state's client (+40) and world (+44) and fills its [`Api`] (+8), and
//! the query adapter reads a context's place (+24), kit mask (+44) and
//! profile (+60).

use core::ffi::c_void;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicBool, Ordering};

/// The native adapters (late_join_026g.asm), filled in by its
/// initialize_api before the first activation; cdecl.
#[repr(C)]
pub struct Api {
    pub allocate: Option<unsafe extern "C" fn(size: u32) -> *mut c_void>,
    pub free: Option<unsafe extern "C" fn(pointer: *mut c_void)>,
    /// The descriptor into the profile's storage, with the game's
    /// validation first: nonzero when it was one.
    pub decode: Option<unsafe extern "C" fn(*const u8, u32, *mut c_void, *mut c_void) -> i32>,
    pub retain_world: Option<unsafe extern "C" fn(world: *mut c_void)>,
    pub release_world: Option<unsafe extern "C" fn(world: *mut c_void)>,
    /// Asks the game's loader for the context's players; may complete at once.
    pub query: Option<unsafe extern "C" fn(context: *mut Context)>,
    /// Must take the slot's references before it returns success, may fail
    /// only before it changes a live slot, and does not activate the slot.
    pub install: Option<unsafe extern "C" fn(context: *mut Context) -> i32>,
    pub ack: Option<unsafe extern "C" fn(client: *mut c_void, seed: u32, serial: u32, place: u8, status: u8)>,
}

/// Calls an adapter: one the assembly has not filled in is a crash, as it
/// was.
macro_rules! call {
    ($api:expr, $name:ident ( $($arg:expr),* )) => {
        match $api.$name {
            Some(function) => function($($arg),*),
            None => crash(),
        }
    };
}

fn crash() -> ! {
    loop {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        unsafe {
            core::arch::asm!("ud2", options(noreturn))
        }
    }
}

/// The late join of one match: what the state page holds.
#[repr(C)]
pub struct State {
    /// Guards the list and the destruction notifications.
    lock: AtomicBool,
    pub contexts: *mut Context,
    pub api: Api,
    /// The current match's (null: none, after a deactivation).
    pub client: *mut c_void,
    pub world: *mut c_void,
    pub dictionary: *mut c_void,
    pub seed: u32,
    /// The next place the roster takes.
    pub roster: u32,
}

/// Whether the game's loader still holds a context.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Query {
    Done = 0,
    Running = 1,
}

/// Whether a context belongs to the current match.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    Retired = 0,
    Current = 1,
}

/// What a proposal came to, as the acknowledgement says it.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Installed = 0,
    Failed = 1,
    Busy = 2,
}

/// The profile's bytes (a player_profile).
pub const PROFILE_BYTES: usize = 0x45FC;

/// One proposal, its descriptor after it ([`Context::wire`]).
#[repr(C)]
pub struct Context {
    pub next: *mut Context,
    pub state: *mut State,
    pub client: *mut c_void,
    pub world: *mut c_void,
    pub seed: u32,
    pub serial: u32,
    pub place: u32,
    pub squad: u32,
    pub query: Query,
    pub of_match: Match,
    pub status: Status,
    /// The kits the profile has (bit k: kit k is usable).
    pub kit_mask: u32,
    /// The players the loader made, by kit; null once destroyed.
    pub players: [*mut c_void; 3],
    /// Stable storage for the whole life of every loaded player.
    pub profile: [u32; PROFILE_BYTES / 4],
    pub wire_size: u32,
    wire: [u8; 0],
}

#[cfg(target_pointer_width = "32")]
const _: () = {
    use core::mem::offset_of;
    assert!(offset_of!(State, api) == 8 && offset_of!(State, client) == 40 && offset_of!(State, world) == 44);
    assert!(offset_of!(Context, place) == 24 && offset_of!(Context, kit_mask) == 44);
    assert!(offset_of!(Context, profile) == 60);
};

impl Context {
    /// The descriptor's bytes.
    pub fn wire(&self) -> *const u8 {
        self.wire.as_ptr()
    }
}

/// A proposal as the E4 extension carries it (its 14-byte header, then the
/// descriptor).
struct Proposal<'a> {
    seed: u32,
    serial: u32,
    place: u32,
    squad: u32,
    body: &'a [u8],
}

/// The most bytes of an E4 extension.
pub const MAX_PACKET: usize = 32782;

impl<'a> Proposal<'a> {
    fn parse(packet: &'a [u8]) -> Option<Proposal<'a>> {
        if packet.len() > MAX_PACKET {
            return None;
        }
        let (head, body) = packet.split_first_chunk::<14>()?;
        let word = |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
        (head[..4] == [0xFF, b'L', b'J', 1]).then(|| Proposal {
            seed: word(4),
            serial: word(8),
            place: u32::from(head[12]),
            squad: u32::from(head[13]),
            body,
        })
    }
}

impl State {
    fn lock(&self) {
        while self.lock.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
    }

    fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }
}

/// Whether `c` is of the state's match.
unsafe fn current(c: *const Context) -> bool {
    unsafe {
        let s = &*(*c).state;
        (*c).of_match == Match::Current && s.client == (*c).client && s.world == (*c).world && s.seed == (*c).seed
    }
}

/// The contexts nothing holds any more, taken off the list (under the
/// lock): once detached, no caller can still own or find one.
unsafe fn collect(s: *mut State) -> *mut Context {
    unsafe {
        let mut dead: *mut Context = null_mut();
        let mut link: *mut *mut Context = &raw mut (*s).contexts;
        while !(*link).is_null() {
            let c = *link;
            if (*c).of_match == Match::Retired && (*c).query == Query::Done && (*c).players.iter().all(|p| p.is_null()) {
                *link = (*c).next;
                (*c).next = dead;
                dead = c;
            } else {
                link = &raw mut (*c).next;
            }
        }
        dead
    }
}

unsafe fn free_list(s: *mut State, mut dead: *mut Context) {
    unsafe {
        while !dead.is_null() {
            let next = (*dead).next;
            call!((*s).api, free(dead.cast()));
            dead = next;
        }
    }
}

/// Before the game's client is destroyed or its match replaced. Old query
/// callbacks may still complete afterwards.
///
/// # Safety
/// `s` the state page; the game's thread.
pub unsafe fn deactivate(s: *mut State) {
    unsafe {
        (*s).lock();
        (*s).client = null_mut();
        (*s).world = null_mut();
        (*s).dictionary = null_mut();
        (*s).seed = 0;
        (*s).roster = 0;
        let mut c = (*s).contexts;
        while !c.is_null() {
            (*c).of_match = Match::Retired;
            c = (*c).next;
        }
        let dead = collect(s);
        (*s).unlock();
        free_list(s, dead);
    }
}

/// A match is ready: proposals are of it from here.
///
/// # Safety
/// As [`deactivate`].
pub unsafe fn activate(
    s: *mut State,
    client: *mut c_void,
    world: *mut c_void,
    dictionary: *mut c_void,
    seed: u32,
    roster: u32,
) {
    unsafe {
        deactivate(s);
        (*s).client = client;
        (*s).world = world;
        (*s).dictionary = dictionary;
        (*s).seed = seed;
        (*s).roster = roster;
    }
}

/// The E4 extension, whole (its 14-byte header too).
///
/// # Safety
/// As [`deactivate`]; `packet` the extension's bytes.
pub unsafe fn prepare(s: *mut State, packet: &[u8]) {
    unsafe {
        let st = &*s;
        if st.client.is_null() || st.world.is_null() || st.dictionary.is_null() {
            return;
        }
        let Some(p) = Proposal::parse(packet) else { return };
        if p.seed != st.seed || p.serial == 0 || p.place == 0 || p.place >= 20 {
            return;
        }
        let ack = |serial: u32, status: Status| call!((*s).api, ack((*s).client, p.seed, serial, p.place as u8, status as u8));
        let size = p.body.len() as u32;
        // Game-thread ownership pins every current-match descriptor, a
        // failed one too: a retransmission must not submit another body.
        st.lock();
        let mut found: *mut Context = null_mut();
        let mut c = st.contexts;
        while !c.is_null() {
            if current(c) && (*c).place == p.place {
                found = c;
                break;
            }
            c = (*c).next;
        }
        st.unlock();
        if !found.is_null() {
            let f = &mut *found;
            let mut status = Status::Failed;
            if f.squad == p.squad
                && f.wire_size == size
                && core::slice::from_raw_parts(f.wire(), size as usize) == p.body
            {
                if p.serial > f.serial {
                    f.serial = p.serial;
                }
                status = f.status;
            }
            ack(p.serial, status);
            return;
        }
        if p.place != st.roster {
            ack(p.serial, Status::Failed);
            return;
        }
        let c = call!(st.api, allocate(size_of::<Context>() as u32 + size)).cast::<Context>();
        if c.is_null() {
            ack(p.serial, Status::Busy);
            return;
        }
        // Field by field: the profile's storage is the decoder's to fill.
        (*c).state = s;
        (*c).client = st.client;
        (*c).world = st.world;
        (*c).seed = p.seed;
        (*c).serial = p.serial;
        (*c).place = p.place;
        (*c).squad = p.squad;
        (*c).query = Query::Running;
        (*c).of_match = Match::Current;
        (*c).status = Status::Busy;
        (*c).kit_mask = 0;
        (*c).wire_size = size;
        let wire = (&raw mut (*c).wire).cast::<u8>();
        core::ptr::copy_nonoverlapping(p.body.as_ptr(), wire, p.body.len());
        (*c).players = [null_mut(); 3];
        let profile = (&raw mut (*c).profile).cast::<u8>();
        if call!(st.api, decode(wire, size, profile.cast(), st.dictionary)) == 0 {
            call!(st.api, free(c.cast()));
            ack(p.serial, Status::Failed);
            return;
        }
        // the kits: the profile's count (+57h), each kit's record 704h from
        // +758h, a usable one's first word 0
        for kit in 0..u32::from(*profile.add(0x57)) {
            let at = profile.add(0x758 + 0x704 * kit as usize).cast::<u32>();
            if at.read_unaligned() == 0 {
                (*c).kit_mask |= 1u32.wrapping_shl(kit);
            }
        }
        call!(st.api, retain_world((*c).world));
        st.lock();
        (*c).next = st.contexts;
        (*s).contexts = c;
        st.unlock();
        // The query may complete at once: c is not touched after it
        // (cancellation and an immediate failure can have freed it already).
        call!(st.api, query(c));
    }
}

/// The loader made the players: by kit, null for a kit not made.
///
/// # Safety
/// `c` a context the query was given; the game's thread.
pub unsafe fn complete(c: *mut Context, players: &[*mut c_void; 3]) {
    unsafe {
        let s = (*c).state;
        let world = (*c).world;
        let mut success = true;
        (*s).lock();
        for (kit, &player) in players.iter().enumerate() {
            (*c).players[kit] = player;
            if !player.is_null() != ((*c).kit_mask & 1 << kit != 0) {
                success = false;
            }
        }
        (*s).unlock();
        if current(c) {
            success = success && (*c).place == (*s).roster && call!((*s).api, install(c)) != 0;
            (*c).status = if success { Status::Installed } else { Status::Failed };
            if success {
                (*s).roster = (*s).roster.wrapping_add(1);
            }
            let c = &*c;
            call!((*s).api, ack(c.client, c.seed, c.serial, c.place as u8, c.status as u8));
        }
        (*s).lock();
        (*c).query = Query::Done;
        let dead = collect(s);
        (*s).unlock();
        free_list(s, dead);
        // Releasing the old world may destroy players at once and free c:
        // neither the context nor its profile is touched after this.
        call!((*s).api, release_world(world));
    }
}

/// After the game's player destructor, with the profile the player had.
/// Resource memory may already be gone: neither is read.
///
/// # Safety
/// `s` the state page; any thread.
pub unsafe fn player_destroyed(s: *mut State, player: *mut c_void, profile: *const c_void) {
    unsafe {
        (*s).lock();
        let mut c = (*s).contexts;
        while !c.is_null() {
            if (&raw const (*c).profile).cast::<c_void>() == profile {
                for p in &mut (*c).players {
                    if *p == player {
                        *p = null_mut();
                    }
                }
                break;
            }
            c = (*c).next;
        }
        let dead = collect(s);
        (*s).unlock();
        free_list(s, dead);
    }
}

#[cfg(test)]
mod tests;
