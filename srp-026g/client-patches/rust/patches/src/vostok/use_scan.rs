//! base_player's use scan (0x2D84C0): a ray from the eye finds the usable
//! object a player may use, walking its hits. Its three sites
//! (sites_026g.tsv `use_scan.*`) share one frame: at each, ESP is the
//! scan's, its hits' collision groups at +0Ch and patch 23's `crossed`
//! byte at +40h (fields_026g.tsv `use_scan_frame`); EAX at `reach` and
//! [EBP+8] at `candidate` are the player, EDI at `candidate` the object's
//! position (the scan's copy, x y z), ESI the usable object.

use core::ffi::c_void;
use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::object::{Object, floats, pointer, read, vtable, write};
use crate::raw;

/// A position, x y z, as the game keeps it.
pub type Position = [f32; 3];

/// The scan's frame at one of its sites (the site's ESP).
pub struct ScanFrame {
    esp: u32,
}

impl ScanFrame {
    pub fn at(ctx: &Context<'_>) -> ScanFrame {
        ScanFrame { esp: ctx.site_esp() }
    }

    fn local<T>(&self, at: unsafe fn(*mut c_void) -> *mut T) -> *mut T {
        // SAFETY: the scan's frame at its sites (the site's ESP); the field
        // functions only add their offset.
        unsafe { at(self.esp as usize as *mut c_void) }
    }

    /// Whether a wall or a floor was crossed this scan (patch 23's byte).
    pub fn crossed(&self) -> bool {
        // SAFETY: the frame's byte, the scan's own stack.
        unsafe { read(self.local(raw::use_scan_frame::crossed)) != 0 }
    }

    pub fn set_crossed(&self, crossed: bool) {
        // SAFETY: as crossed.
        unsafe { write(self.local(raw::use_scan_frame::crossed), u8::from(crossed)) }
    }

    /// Keeps only `mask` of the hits' collision groups, the dword (the
    /// scan's own `and dword [esp+0Ch],mask`).
    pub fn mask_groups(&self, mask: u32) {
        let at = self.local(raw::use_scan_frame::groups);
        // SAFETY: the frame's dword.
        unsafe { write(at, read(at) & mask) }
    }

    /// Keeps only `mask` of the groups' low word (an `and word [esp+0Ch]`).
    pub fn mask_groups_word(&self, mask: u16) {
        let at = self.local(raw::use_scan_frame::groups).cast::<u16>();
        // SAFETY: the frame's dword's low word.
        unsafe { write(at, read(at) & mask) }
    }
}

/// The ray's length: the console variable usable_objects_detection_distance
/// as the scan reads it, a float.
pub fn detection_distance() -> f32 {
    // SAFETY: the exe's own console variable, a float (fields_026g.tsv).
    unsafe { read(raw::image::USABLE_OBJECTS_DETECTION_DISTANCE as usize as *mut f32) }
}

/// The player the scan is for, at `candidate` ([EBP+8]).
pub struct ScanPlayer<'t> {
    player: Object,
    thread: PhantomData<&'t Thread>,
}

impl<'t> ScanPlayer<'t> {
    pub fn at_candidate(ctx: &'t Context<'_>) -> Option<ScanPlayer<'t>> {
        // SAFETY: the scan's frame pointer at the site: its first argument,
        // the player, at [EBP+8].
        let player = unsafe { pointer((ctx.reg(Reg::Ebp).wrapping_add(8)) as usize as *mut u32) }?;
        Some(ScanPlayer { player, thread: PhantomData })
    }

    pub fn eye(&self) -> Position {
        // SAFETY: the player's eye, three floats.
        unsafe { floats(raw::base_player::eye(self.player.ptr())) }
    }

    pub fn feet(&self) -> Position {
        // SAFETY: the player's feet, three floats.
        unsafe { floats(raw::base_player::feet(self.player.ptr())) }
    }
}

/// The usable object at `candidate` (ESI) and its position as the scan
/// keeps it (EDI).
pub struct Candidate<'t> {
    usable: Object,
    position: *mut f32,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Candidate<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<Candidate<'t>> {
        let usable = Object::at(ctx.reg(Reg::Esi))?;
        Some(Candidate { usable, position: ctx.reg(Reg::Edi) as usize as *mut f32, thread: PhantomData })
    }

    /// The position the scan measures, x y z.
    pub fn position(&self) -> Position {
        // SAFETY: the scan's position of the object, three floats.
        unsafe { floats(self.position) }
    }

    /// Moves the position the scan goes on with.
    pub fn set_position(&self, at: Position) {
        for (i, value) in at.into_iter().enumerate() {
            // SAFETY: as position; the scan's own copy, written float by float.
            unsafe { write(self.position.add(i), value) }
        }
    }

    /// Whether it is a battery: its can_use (its vtable's +30h) is
    /// victory_item_core's.
    pub fn is_battery(&self) -> bool {
        // SAFETY: a usable object's vtable, its +30h slot.
        let can_use = unsafe { read(raw::usable_vtable::can_use(vtable(self.usable) as usize as *mut c_void)) };
        can_use == raw::image::VICTORY_ITEM_CAN_USE
    }

    /// Its transform (a 4x4 of floats, row by row; the translation at
    /// +30h..+38h), through its vtable's get_transform.
    pub fn transform(&self) -> [f32; 16] {
        let mut matrix = [0f32; 16];
        // SAFETY: the usable object, its vtable's get_transform (thiscall,
        // the out matrix, `ret 4`), our matrix for the call.
        unsafe { raw::engine_usable_get_transform(self.usable.ptr(), matrix.as_mut_ptr()) };
        matrix
    }
}
