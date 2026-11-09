//! A place's timed lists (an input list and a kit choice list a place,
//! core+2FCCh and core+89D8h, 480h each): entries {+0 the next newer, +4 the
//! next older, +8 the value, +20h the time} from the oldest (+4) to the
//! newest (+8), their nodes from the list's pool (allocate_node 0x1FCB60,
//! which may give the oldest back to the pool first) and linked by
//! insert_before (0x2D4850). And the record game_world_core::deserialize
//! (0x2D0B70) gives them at its first world's entries (`first_world.input`,
//! `first_world.kit`: EDI -> {core, the world's time, place}).

use core::marker::PhantomData;

use super::context::Context;
use super::hook::Thread;
use super::world::read;
use crate::raw;

/// The first world's record for one place, at `first_world.input` or
/// `first_world.kit` (the same EDI).
pub struct FirstWorldPlace<'t> {
    core: raw::CoreWorld,
    time: u32,
    place: usize,
    _thread: PhantomData<&'t Thread>,
}

impl<'t> FirstWorldPlace<'t> {
    pub fn at_input(ctx: &'t Context<'_>) -> Option<FirstWorldPlace<'t>> {
        // SAFETY: the site's EDI: deserialize's record, its core the world
        // being loaded, alive on the game's thread.
        let entry = unsafe { raw::site::first_world_input::entry(ctx)? };
        Self::of(entry)
    }

    pub fn at_kit(ctx: &'t Context<'_>) -> Option<FirstWorldPlace<'t>> {
        // SAFETY: as at_input.
        let entry = unsafe { raw::site::first_world_kit::entry(ctx)? };
        Self::of(entry)
    }

    fn of(entry: raw::FirstLoadFrame) -> Option<FirstWorldPlace<'t>> {
        // SAFETY: the record's plain fields and its core pointer.
        unsafe {
            let place = usize::from(read(entry.place()));
            (place < PLACES).then_some(())?;
            Some(FirstWorldPlace { core: entry.core()?, time: read(entry.time()), place, _thread: PhantomData })
        }
    }

    /// The world's time.
    pub fn time(&self) -> u32 {
        self.time
    }

    fn slot(&self) -> Option<raw::CorePlace> {
        // SAFETY: the core's places vector, one a place of the match's
        // (the record's places are the world's).
        unsafe { Some(self.core.places()?.at(self.place)) }
    }

    /// The input of the place's player in its current kit (+32Ch, +330h).
    pub fn input(&self) -> Option<(u32, u32)> {
        // SAFETY: the place's slot and its current kit's player, live.
        unsafe {
            let slot = self.slot()?;
            let player = match read(slot.current_kit()) {
                0 => slot.kit0(),
                1 => slot.kit1(),
                2 => slot.kit2(),
                _ => None,
            }?;
            Some((read(player.input()), read(player.actions())))
        }
    }

    /// The kit the world has chosen for the place's next spawn (+0Dh).
    pub fn chosen_kit(&self) -> Option<u8> {
        // SAFETY: the place's slot's plain byte.
        Some(unsafe { read(self.slot()?.next_kit()) })
    }

    /// The place's input list.
    pub fn inputs(&self) -> Option<TimedList<'_>> {
        // SAFETY: the core's lists, one a place.
        unsafe { self.core.inputs() }.map(|first| TimedList { this: unsafe { first.at(self.place) }, _place: PhantomData })
    }

    /// The place's kit choice list.
    pub fn kit_choices(&self) -> Option<TimedList<'_>> {
        // SAFETY: as inputs.
        unsafe { self.core.kit_choices() }
            .map(|first| TimedList { this: unsafe { first.at(self.place) }, _place: PhantomData })
    }
}

const PLACES: usize = 20;

/// A place's timed list.
pub struct TimedList<'a> {
    this: raw::TimedList,
    _place: PhantomData<&'a ()>,
}

/// An entry of a timed list.
#[derive(Clone, Copy)]
pub struct TimedEntry<'a> {
    this: raw::TimedNode,
    _list: PhantomData<&'a ()>,
}

impl<'a> TimedList<'a> {
    fn entry(&self, node: Option<raw::TimedNode>) -> Option<TimedEntry<'a>> {
        node.map(|this| TimedEntry { this, _list: PhantomData })
    }

    pub fn newest(&self) -> Option<TimedEntry<'a>> {
        // SAFETY: the live list's pointer.
        self.entry(unsafe { self.this.newest() })
    }

    pub fn oldest(&self) -> Option<TimedEntry<'a>> {
        // SAFETY: as newest.
        self.entry(unsafe { self.this.oldest() })
    }

    /// A new entry of the list's pool, at `time`, not yet linked (the pool
    /// may give the oldest back first).
    pub fn allocate(&self, time: u32) -> Option<TimedEntry<'a>> {
        // SAFETY: the game's allocate_node of the live list.
        let node = self.entry(unsafe { raw::engine_input_node(Some(self.this)) })?;
        // SAFETY: the node it gave, ours to fill.
        unsafe { *node.this.time() = time };
        Some(node)
    }

    /// Links `entry` (allocate's) before `before` (none: at the newest end).
    pub fn insert_before(&self, entry: TimedEntry<'a>, before: Option<TimedEntry<'a>>) {
        // SAFETY: the game's insert_before of the live list, a node of its
        // pool and one of its entries (or none).
        unsafe { raw::engine_insert_before(Some(entry.this), before.map(|b| b.this), Some(self.this)) };
    }
}

impl<'a> TimedEntry<'a> {
    pub fn time(&self) -> u32 {
        // SAFETY: a live entry's plain field.
        unsafe { read(self.this.time()) }
    }

    pub fn older(&self) -> Option<TimedEntry<'a>> {
        // SAFETY: a live entry's link.
        unsafe { self.this.older() }.map(|this| TimedEntry { this, _list: PhantomData })
    }

    pub fn newer(&self) -> Option<TimedEntry<'a>> {
        // SAFETY: as older.
        unsafe { self.this.newer() }.map(|this| TimedEntry { this, _list: PhantomData })
    }

    /// An input entry's value.
    pub fn set_input(&self, (first, second): (u32, u32)) {
        // SAFETY: a live entry's plain fields.
        unsafe {
            *self.this.first() = first;
            *self.this.second() = second;
        }
    }

    /// A kit choice entry's value.
    pub fn set_kit(&self, kit: u8) {
        // SAFETY: as set_input.
        unsafe { *self.this.kit() = kit }
    }
}
