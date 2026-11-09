//! A game-effect presenter: what its search found for an event's effect
//! (patch 117), read once from the register the native code goes on with;
//! and room for its particle effects (patch 122).
//!
//! A particle presenter's list is a buffer_vector<effect_data>: begin, end
//! and capacity end, with room for six entries inside the presenter that
//! nothing grows (push_back 0x2ACC20 asserts when it is full). The list's
//! code reads only those three pointers, so a [`Room`] of ours takes the
//! inline room's place: the list's entries there, and as many for the local
//! list its deserialize reads a saved world's entries into, which the swap
//! 0x2ACD00 then exchanges with the list ([`swap`], in place: the game's
//! own swap goes through a stack room of six).
use core::ptr::{self, NonNull};

use super::{Context, Reg, Thread};
use crate::raw;

/// The presenter's entry for the event's effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Search {
    /// It plays the effect.
    Held,
    /// It holds no entry of it: a clear (base_player::remove 0x2DA81C, the
    /// player's presenter) forgot it while the effect played on.
    Missing,
}

/// A searched index: the search starts it at -1.
fn index(raw: u32) -> Search {
    if raw == u32::MAX { Search::Missing } else { Search::Held }
}

/// A searched entry's address: the search starts it at 0.
fn entry(raw: u32) -> Search {
    if raw == 0 { Search::Missing } else { Search::Held }
}

/// `effect_presenter.sound_stop`: EDI the index.
pub fn sound_stop(ctx: &Context<'_>) -> Search {
    index(ctx.reg(Reg::Edi))
}

/// `effect_presenter.sound_update`: ESI the entry.
pub fn sound_update(ctx: &Context<'_>) -> Search {
    entry(ctx.reg(Reg::Esi))
}

/// `effect_presenter.particle_stop`: EAX the index.
pub fn particle_stop(ctx: &Context<'_>) -> Search {
    index(ctx.reg(Reg::Eax))
}

/// A particle presenter's identity: its address (a key, never read through).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresenterId(u32);

impl PresenterId {
    pub fn address(self) -> u32 {
        self.0
    }
}

/// Room of ours for a particle presenter's entries: `entries` for its list,
/// then as many for the local list its deserialize reads into. From the
/// process heap, zeroed, never freed: the presenters given one are the game
/// world's, a static object, so each is built again at the same address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    first: NonNull<u8>,
    entries: u32,
}

// SAFETY: a Room is an address of memory ours alone that is never freed;
// the game's lists that are given it are the only ones that write it.
unsafe impl Send for Room {}

#[cfg(test)]
impl PresenterId {
    pub fn for_test(address: u32) -> PresenterId {
        PresenterId(address)
    }
}

#[cfg(test)]
impl Room {
    /// A room the tests tell apart by `tag` (never read through).
    pub fn for_test(tag: u32) -> Option<Room> {
        NonNull::new((tag as usize * 0x1000) as *mut u8).map(|first| Room { first, entries: 32 })
    }
}

/// HeapAlloc's HEAP_ZERO_MEMORY.
const HEAP_ZERO_MEMORY: u32 = 0x8;

impl Room {
    /// `entries` entries for a list and as many for its reads; None when the
    /// heap has no room (or for no entries).
    pub fn allocate(entries: u32) -> Option<Room> {
        let bytes = (entries as usize).checked_mul(2 * raw::ParticleEffectData::SIZE)?;
        if bytes == 0 || bytes > u32::MAX as usize {
            return None;
        }
        // SAFETY: kernel32's imports (the exe's IAT slots): the process heap
        // and a zeroed block of it, which is ours from here on.
        let first = unsafe { (raw::engine_heap_alloc)((raw::engine_get_process_heap)(), HEAP_ZERO_MEMORY, bytes as u32) };
        NonNull::new(first.cast::<u8>()).map(|first| Room { first, entries })
    }

    pub fn entries(self) -> u32 {
        self.entries
    }

    fn half(self) -> usize {
        self.entries as usize * raw::ParticleEffectData::SIZE
    }

    /// The list's half: (begin, capacity end).
    fn list(self) -> (*mut u8, *mut u8) {
        let begin = self.first.as_ptr();
        (begin, begin.wrapping_add(self.half()))
    }

    /// The reads' half.
    fn reads(self) -> (*mut u8, *mut u8) {
        let begin = self.first.as_ptr().wrapping_add(self.half());
        (begin, begin.wrapping_add(self.half()))
    }
}

/// Why a presenter keeps its inline room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotGiven {
    /// Its list already holds entries (not one just built).
    NotEmpty,
}

/// A presenter of the game world's, just built: its particle list empty in
/// its inline room.
#[derive(Clone, Copy, Debug)]
pub struct Built(raw::ParticleGameEffectPresenter);

impl Built {
    /// `game_world.first_person_presenter_made`: the game world's (EBP)
    /// first-person presenter.
    pub fn first_person(ctx: &Context<'_>) -> Option<Built> {
        // SAFETY: at the site EBP is the game world being constructed, its
        // first-person presenter built the instructions before; its particle
        // presenter is part of it.
        unsafe {
            raw::site::game_world_first_person_presenter_made::world(ctx)
                .map(|world| Built(world.first_person_presenter().particles()))
        }
    }

    /// `game_world.third_person_presenter_made`: EAX the presenter.
    pub fn third_person(ctx: &Context<'_>) -> Option<Built> {
        // SAFETY: at the site EAX is the presenter its constructor just
        // built (the constructor keeps EAX); its particle presenter is part of it.
        unsafe { raw::site::game_world_third_person_presenter_made::presenter(ctx).map(|p| Built(p.particles())) }
    }

    pub fn id(self) -> PresenterId {
        PresenterId(self.0.address())
    }

    /// Its list in `room`: begin = end = the room's list half, its capacity
    /// that half's end.
    pub fn give(self, room: Room) -> Result<(), NotGiven> {
        // SAFETY: the presenter its constructor built at this site (alive:
        // the game world's); its list's three pointers are its own.
        unsafe {
            let list = self.0.effects();
            if list.begin() != list.end() {
                return Err(NotGiven::NotEmpty);
            }
            let (begin, end) = room.list();
            list.set_begin(begin);
            list.set_end(begin);
            list.set_capacity(end);
        }
        Ok(())
    }
}

/// deserialize 0x2AC870 at `effect_presenter.particle_read_room`: its
/// presenter, and the local list it is about to read the saved entries into.
pub struct Read<'c, 'h> {
    ctx: &'c mut Context<'h>,
    frame: raw::ParticleReadFrame,
}

impl<'c, 'h> Read<'c, 'h> {
    pub fn at(ctx: &'c mut Context<'h>) -> Option<Read<'c, 'h>> {
        // SAFETY: at the site ESP is deserialize's frame (its locals by ESP).
        let frame = unsafe { raw::site::effect_presenter_particle_read_room::frame(ctx) }?;
        Some(Read { ctx, frame })
    }

    pub fn thread(&self) -> &Thread {
        self.ctx.thread()
    }

    pub fn presenter(&self) -> Option<PresenterId> {
        // SAFETY: the frame's ESP+18h, the presenter stored the instruction before.
        unsafe { self.frame.presenter() }.map(|p| PresenterId(p.address()))
    }

    /// The local list in `room`'s reads half: begin = end there now; its
    /// capacity end is EAX, which the site's second instruction stores.
    pub fn give(self, room: Room) {
        let (begin, end) = room.reads();
        // SAFETY: the frame's local list, set up the instructions before
        // (begin = end = its inline room); nothing has been read into it.
        unsafe {
            let read = self.frame.read();
            read.set_begin(begin);
            read.set_end(begin);
        }
        self.ctx.set_reg(Reg::Eax, end as usize as u32);
    }
}

/// What [`swap`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Swap {
    /// The list holds what was read, the local what the list held.
    Swapped { kept: u32, read: u32 },
    /// One side has no room for the other's entries: both left as they were.
    NoRoom { kept: u32, kept_room: u32, read: u32, read_room: u32 },
    /// A register held no list (0): nothing done.
    NoList,
}

/// A list's entries and room, in entries.
///
/// # Safety
/// `list` is a live particle list.
unsafe fn counts(list: raw::ParticleEffectList) -> (usize, usize) {
    let size = raw::ParticleEffectData::SIZE;
    // SAFETY: the caller's.
    let (begin, end, capacity) = unsafe { (list.begin() as usize, list.end() as usize, list.capacity() as usize) };
    (end.wrapping_sub(begin) / size, capacity.wrapping_sub(begin) / size)
}

/// `effect_presenter.particle_swap`: exchanges the presenter's list (EDI)
/// and deserialize's local (ESI) entry for entry, in place, as the game's
/// swap would (each entry moved whole: its references counted as before).
pub fn swap(ctx: &Context<'_>) -> Swap {
    // SAFETY: at the site EDI and ESI are the two lists the swap's one
    // caller passes (0x2ACC00): the presenter's and its frame's local.
    let lists = unsafe {
        raw::site::effect_presenter_particle_swap::kept(ctx).zip(raw::site::effect_presenter_particle_swap::read(ctx))
    };
    let Some((kept, read)) = lists else { return Swap::NoList };
    let size = raw::ParticleEffectData::SIZE;
    // SAFETY: two live lists of distinct rooms (the presenter's, the
    // frame's); each moved range is inside its room (the counts checked).
    unsafe {
        let ((p, p_room), (l, l_room)) = (counts(kept), counts(read));
        if l > p_room || p > l_room {
            return Swap::NoRoom { kept: p as u32, kept_room: p_room as u32, read: l as u32, read_room: l_room as u32 };
        }
        let (kb, rb) = (kept.begin(), read.begin());
        let both = p.min(l);
        ptr::swap_nonoverlapping(kb, rb, both * size);
        if l > p {
            ptr::copy_nonoverlapping(rb.add(p * size), kb.add(p * size), (l - p) * size);
        } else if p > l {
            ptr::copy_nonoverlapping(kb.add(l * size), rb.add(l * size), (p - l) * size);
        }
        kept.set_end(kb.add(l * size));
        read.set_end(rb.add(p * size));
        Swap::Swapped { kept: l as u32, read: p as u32 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_searches_start_values_are_missing() {
        assert_eq!(index(u32::MAX), Search::Missing);
        assert_eq!(entry(0), Search::Missing);
        for raw in [0, 1, 15, 0x7FFF_FFFF] {
            assert_eq!(index(raw), Search::Held);
        }
        for raw in [1, 0x00A9_9474, u32::MAX] {
            assert_eq!(entry(raw), Search::Held);
        }
    }
}
