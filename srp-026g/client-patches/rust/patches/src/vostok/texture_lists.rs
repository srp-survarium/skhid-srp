//! The render's fixed lists of textures (patch 163): where their entries
//! are, and moving them into a room of ours when the game's is full.
//!
//! Each is a fixed_vector: begin, end and capacity end, then its own room
//! inside its owner, which nothing grows (its push_back asserts when end
//! reaches the capacity end). The game's code reads only the three
//! pointers, so the entries can move to a larger room of ours: copied, and
//! in each entry whose first member is a fixed_string<260>, its own three
//! pointers (into its 104h bytes) moved with it.
//!
//! - The resource manager's texture names (+1Ch, 2048): the names of the
//!   textures its effects asked for while each is registered.
//! - A scene's streamable textures (+126158h, 2048), with what streaming
//!   keeps one an entry of: the async worker's candidates (+98054Ch,
//!   2048, one for each streamable texture). The requests (+1CA164h,
//!   1024, one for each candidate that changes its mips) keep their room:
//!   process_streaming makes room on the stack for all those candidates
//!   at once (14Ch each, on the main thread's 1 MiB), so a batch keeps to
//!   the requests' room and the rest wait for the next.
//! - A texture pool's textures (+64h, 1024): the texture storage's pools
//!   by size and format, a slot each for a texture alive at that size.
//!
//! All of them are the main thread's, which renders (the crashes' thread:
//! its stack the exe's 1 MiB): the names' push (from
//! effect_cook::on_binary_shaders) and erase (resource_manager::release),
//! a pool's add_texture, gather_streamable_textures and process_streaming
//! run on it; the scene's async worker reads the streamable textures and
//! writes the candidates, and gather and process_streaming wait for it
//! (0x1551E4, 0x15405B) before the sites.
//! A room is the process heap's; the scene's are given back in the scene's
//! destructor, the manager's lives as long as the game, a pool's as long
//! as its texture storage (the game's: its destructor, at the game's end,
//! does not give it back).

use core::ptr::NonNull;

use super::Context;
use crate::raw;

/// HeapAlloc's HEAP_ZERO_MEMORY.
const HEAP_ZERO_MEMORY: u32 = 0x8;

/// Where a list's entries are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// In its owner's room: the game's capacity.
    Own,
    /// In a room of ours.
    Ours,
}

/// A list's entries: how many, the room's, and where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fill {
    pub held: u32,
    pub room: u32,
    pub place: Place,
}

impl Fill {
    pub fn full(self) -> bool {
        self.held == self.room
    }
}

/// What a list's entries are.
#[derive(Clone, Copy, Debug)]
struct Shape {
    size: usize,
    /// The first member is a fixed_string, whose three pointers point
    /// into the entry itself.
    named: bool,
}

const NAME: Shape = Shape { size: raw::TextureName::SIZE, named: true };
const STREAMABLE: Shape = Shape { size: raw::StreamableTexture::SIZE, named: true };
const REQUESTED: Shape = Shape { size: raw::RequestedTexture::SIZE, named: true };
const CANDIDATE: Shape = Shape { size: raw::PotentialRequest::SIZE, named: false };
const SLOT: Shape = Shape { size: raw::TexturePoolSlot::SIZE, named: false };

/// A fixed_vector of entries of one shape.
#[derive(Clone, Copy)]
struct List {
    vector: raw::FixedVector,
    shape: Shape,
}

/// A room of ours, from the process heap, zeroed.
struct Room {
    first: NonNull<u8>,
    entries: u32,
}

impl Room {
    fn allocate(entries: u32, shape: Shape) -> Option<Room> {
        let bytes = (entries as usize).checked_mul(shape.size).filter(|&b| b != 0 && b <= u32::MAX as usize)?;
        // SAFETY: kernel32's imports (the exe's IAT slots): the process heap
        // and a zeroed block of it, which is ours from here on.
        let first = unsafe { (raw::engine_heap_alloc)((raw::engine_get_process_heap)(), HEAP_ZERO_MEMORY, bytes as u32) };
        NonNull::new(first.cast::<u8>()).map(|first| Room { first, entries })
    }

    fn free(self) {
        // SAFETY: a block of the process heap that nothing points into.
        unsafe { (raw::engine_heap_free)((raw::engine_get_process_heap)(), 0, self.first.as_ptr().cast()) };
    }
}

impl List {
    fn own(self) -> *mut u8 {
        // SAFETY: the list is alive (its owner's, read on its thread); the
        // room is inside it.
        unsafe { self.vector.items().address() as usize as *mut u8 }
    }

    fn fill(self) -> Fill {
        // SAFETY: as `own`; three pointers of the list.
        let (begin, end, capacity) = unsafe { (self.vector.begin(), self.vector.end(), self.vector.capacity()) };
        let count = |to: *mut u8| ((to as usize).wrapping_sub(begin as usize) / self.shape.size) as u32;
        Fill { held: count(end), room: count(capacity), place: if begin == self.own() { Place::Own } else { Place::Ours } }
    }

    /// Its entries into `room`, which takes the list's place.
    ///
    /// # Safety
    /// Nothing else reads or writes the list while it moves, and nothing
    /// keeps an address of one of its entries; `room` holds at least as many
    /// entries as the list.
    unsafe fn move_to(self, room: &Room) {
        let size = self.shape.size;
        let held = self.fill().held as usize;
        // SAFETY: the caller's; the list's entries are `held` of `size`
        // bytes from begin, the room as many and more, apart.
        unsafe {
            let from = self.vector.begin();
            let to = room.first.as_ptr();
            core::ptr::copy_nonoverlapping(from, to, held * size);
            if self.shape.named {
                for i in 0..held {
                    let (old, new) = (from.add(i * size) as usize, to.add(i * size));
                    for word in 0..3 {
                        let place = new.add(word * 4).cast::<usize>();
                        place.write_unaligned(place.read_unaligned().wrapping_sub(old).wrapping_add(new as usize));
                    }
                }
            }
            self.vector.set_begin(to);
            self.vector.set_end(to.add(held * size));
            self.vector.set_capacity(to.add(room.entries as usize * size));
        }
    }

    /// The room of ours it is in back to the heap, the list empty in its own.
    ///
    /// # Safety
    /// The list is empty and its owner is being destroyed.
    unsafe fn give_back(self, own_entries: u32) {
        if self.fill().place == Place::Own {
            return;
        }
        // SAFETY: the caller's; begin is a room of ours (`Room::allocate`).
        unsafe {
            if let Some(first) = NonNull::new(self.vector.begin()) {
                Room { first, entries: 0 }.free();
            }
            let own = self.own();
            self.vector.set_begin(own);
            self.vector.set_end(own);
            self.vector.set_capacity(own.add(own_entries as usize * self.shape.size));
        }
    }
}

/// Why a list stays in the game's room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotMoved {
    /// The heap gave none.
    NoMemory,
}

/// The game's room of each list, in entries.
pub const OWN_NAMES: u32 = 2048;
pub const OWN_STREAMABLE: u32 = 2048;
pub const OWN_REQUESTED: u32 = 1024;
pub const OWN_CANDIDATES: u32 = 2048;
pub const OWN_POOL_SLOTS: u32 = 1024;

/// The resource manager's texture names at `texture_lists.name_push`.
pub struct Names(List);

impl Names {
    pub fn at(ctx: &Context<'_>) -> Option<Names> {
        // SAFETY: at the site EBX is the manager's list (lea ebx,[eax+1Ch]
        // at 0x3DF79F, eax the manager's global); the manager lives as long
        // as the game.
        unsafe { raw::site::texture_lists_name_push::names(ctx) }.map(|vector| Names(List { vector, shape: NAME }))
    }

    pub fn fill(&self) -> Fill {
        self.0.fill()
    }

    /// Into a room of `entries` of ours.
    pub fn move_to_room(&self, entries: u32) -> Result<(), NotMoved> {
        let room = Room::allocate(entries, NAME).ok_or(NotMoved::NoMemory)?;
        // SAFETY: the main thread's list, at a push (no entry's address
        // kept: the predicate's find has returned); the room is larger.
        unsafe { self.0.move_to(&room) };
        Ok(())
    }
}

/// A texture pool's textures at `texture_lists.pool_slot_push`.
pub struct PoolSlots {
    pool: raw::TexturePool,
    list: List,
}

impl PoolSlots {
    pub fn at(ctx: &Context<'_>) -> Option<PoolSlots> {
        // SAFETY: at the site ESI is add_texture's pool (its first
        // argument, mov esi,[esp+64h] at 0x3D3199); the texture storage
        // keeps its pools as long as it lives.
        let pool = unsafe { raw::site::texture_lists_pool_slot_push::pool(ctx) }?;
        // SAFETY: the pool is alive; its list is inside it.
        Some(PoolSlots { pool, list: List { vector: unsafe { pool.slots() }, shape: SLOT } })
    }

    pub fn fill(&self) -> Fill {
        self.list.fill()
    }

    /// The pool's name ("64x64 dxt1").
    pub fn name(&self) -> &str {
        // SAFETY: the pool's name is a fixed_string<64>, a C string its
        // constructor wrote; it lives as long as the pool.
        unsafe { super::ui::c_string(self.pool.name(), 64) }.and_then(|name| name.to_str().ok()).unwrap_or("?")
    }

    /// Into a room of `entries` of ours.
    pub fn move_to_room(&self, entries: u32) -> Result<(), NotMoved> {
        let room = Room::allocate(entries, SLOT).ok_or(NotMoved::NoMemory)?;
        // SAFETY: the main thread's pool, at add_texture's push: get and
        // try_release find a slot by walking the list, nothing keeps one's
        // address past its call; the room is larger.
        unsafe { self.list.move_to(&room) };
        Ok(())
    }
}

/// A scene's streaming lists.
pub struct Streaming {
    streamable: List,
    candidates: List,
}

impl Streaming {
    fn of(scene: raw::Scene) -> Streaming {
        // SAFETY: the scene the site holds, alive; its lists are inside it.
        unsafe {
            Streaming {
                streamable: List { vector: scene.streaming_textures(), shape: STREAMABLE },
                candidates: List { vector: scene.potential_requests(), shape: CANDIDATE },
            }
        }
    }

    /// At `texture_lists.streaming_push`.
    pub fn at_push(ctx: &Context<'_>) -> Option<Streaming> {
        // SAFETY: at the site EBX is gather's scene (mov ebx,[esp+178h] at 0x155188).
        unsafe { raw::site::texture_lists_streaming_push::scene(ctx) }.map(Streaming::of)
    }

    /// At `texture_lists.scene_lists_destroyed`.
    pub fn at_destroyed(ctx: &Context<'_>) -> Option<Streaming> {
        // SAFETY: at the site EBP is the scene its destructor destroys.
        unsafe { raw::site::texture_lists_scene_lists_destroyed::scene(ctx) }.map(Streaming::of)
    }

    pub fn streamable(&self) -> Fill {
        self.streamable.fill()
    }

    /// The streamable textures and the candidates (one each) into rooms of
    /// `entries` of ours, both or neither.
    pub fn move_to_rooms(&self, entries: u32) -> Result<(), NotMoved> {
        let rooms = (Room::allocate(entries, STREAMABLE), Room::allocate(entries, CANDIDATE));
        let (streamable, candidates) = match rooms {
            (Some(s), Some(c)) => (s, c),
            (s, c) => {
                // the one the heap gave, back
                [s, c].into_iter().flatten().for_each(Room::free);
                return Err(NotMoved::NoMemory);
            }
        };
        // SAFETY: the main thread's lists at gather's push, the async
        // worker done: the game keeps indices into the streamable textures
        // (the candidates' first word), never an entry's address; each
        // room is larger than its list's.
        unsafe {
            self.streamable.move_to(&streamable);
            self.candidates.move_to(&candidates);
        }
        Ok(())
    }

    /// The rooms of ours back, at the scene's destructor (each list empty).
    pub fn give_back(&self) {
        // SAFETY: the destructor emptied the lists and nothing of the scene
        // reads them after (the site's).
        unsafe {
            self.streamable.give_back(OWN_STREAMABLE);
            self.candidates.give_back(OWN_CANDIDATES);
        }
    }
}

/// process_streaming's candidates whose mips change, at
/// `texture_lists.request_batch`: it asks a request for each into the
/// requests (empty there), after making room on the main thread's stack
/// for all of them (14Ch a candidate).
pub struct Batch {
    candidates: List,
    requests: List,
}

impl Batch {
    pub fn at(ctx: &Context<'_>) -> Option<Batch> {
        // SAFETY: at the site ESI is the scene's candidates (lea esi,[edi+98054Ch]
        // at 0x1543E7) and EDI the scene (mov edi,[ebp+8] at 0x1543D0).
        let (candidates, scene) = unsafe {
            (raw::site::texture_lists_request_batch::candidates(ctx)?, raw::site::texture_lists_request_batch::scene(ctx)?)
        };
        // SAFETY: the scene is alive; its requests are inside it.
        let requests = unsafe { scene.requested_textures() };
        Some(Batch { candidates: List { vector: candidates, shape: CANDIDATE }, requests: List { vector: requests, shape: REQUESTED } })
    }

    /// The candidates, and the requests' room (they are empty here).
    pub fn sizes(&self) -> (u32, u32) {
        (self.candidates.fill().held, self.requests.fill().room)
    }

    /// The first `kept` candidates only: the rest change their mips in a
    /// later batch (the async worker makes the candidates again each time).
    pub fn keep_first(&self, kept: u32) {
        let fill = self.candidates.fill();
        if kept >= fill.held {
            return;
        }
        // SAFETY: the main thread's list, process_streaming's own: the
        // async worker that writes it is done (0x15405B waited for it), and
        // it reads the list's end after the site; the candidates are plain
        // values, nothing to release.
        unsafe {
            let begin = self.candidates.vector.begin();
            self.candidates.vector.set_end(begin.add(kept as usize * self.candidates.shape.size));
        }
    }
}
