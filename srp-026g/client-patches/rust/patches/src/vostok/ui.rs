//! The match's interface: survarium::game_world_ui (the hud) and the
//! Scaleform movies it holds, the hud's and the player icons', at the site
//! `player_icons.update_start` (update_player_icons' start, 0x23AA10: the ui
//! its argument, by the site's ESP).

use core::ffi::{CStr, c_void};
use core::marker::PhantomData;
use core::ptr::NonNull;

use super::context::{Context, Reg};
use super::hook::Thread;
use crate::raw;

/// A character event delivered after a text field lost its AS3 receiver.
/// Only this detached state permits skipping native dispatch/insertion.
pub struct DetachedTextInput<'c, 'm> {
    ctx: &'c mut Context<'m>,
}

impl<'c, 'm> DetachedTextInput<'c, 'm> {
    pub fn at_char_event(ctx: &'c mut Context<'m>) -> Option<Self> {
        // EDI is the live AvmTextField interface.
        let avm = ctx.reg(Reg::Edi) as usize as *mut c_void;
        // SAFETY: this site's native code reads these exact two fields next.
        // The collectible low bit is the GC mark, not part of its pointer.
        let target = unsafe {
            let raw = *raw::avm_text_field::as3_raw(avm);
            if raw != 0 { raw } else { *raw::avm_text_field::as3_collectible(avm) }
        } & !1;
        if target != 0 { return None; }
        Some(Self { ctx })
    }

    pub fn ignore(self) {
        // The original epilogue returns BL as AL; its stack restores EBX.
        self.ctx.set_reg8(Reg::Ebx, 0);
    }
}

/// A teammate's name decision at `player_names.visibility` (0x1F2F10).
/// The handle can only be obtained for a valid same-team player, and cannot
/// escape the hook. The engine already checked life, visibility and projection.
pub struct NameVisibility<'c, 'm> {
    ctx: &'c mut Context<'m>,
}

impl<'c, 'm> NameVisibility<'c, 'm> {
    pub fn for_teammate(ctx: &'c mut Context<'m>) -> Option<Self> {
        let player = NonNull::new(ctx.reg(Reg::Esi) as usize as *mut c_void)?;
        let frame = ctx.reg(Reg::Esp).wrapping_add(8) as usize as *mut c_void;
        // SAFETY: at this site ESI is render_name's live player, and ESP+14h
        // holds its local-team snapshot. The profile is the same one the
        // original code reads at 0x1F2F33, before any call can change it.
        let (local, team) = unsafe {
            let profile = NonNull::new(*raw::player::profile(player.as_ptr()) as usize as *mut c_void)?;
            (*raw::render_name_frame::local_team(frame), *raw::player_profile::team(profile.as_ptr()))
        };
        if local > 1 || local != team {
            return None;
        }
        Some(Self { ctx })
    }

    /// Show this eligible teammate's name; never modify input or player state.
    pub fn show(self) {
        self.ctx.set_reg8(Reg::Eax, 1);
    }
}

/// The hud, survarium::game_world_ui, as a hook's site has it.
pub struct GameWorldUi<'t> {
    this: NonNull<c_void>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> GameWorldUi<'t> {
    /// At `player_icons.update_start`.
    pub fn at_update_icons(ctx: &'t Context<'_>) -> Option<GameWorldUi<'t>> {
        // the site's ESP: the entry pushed its dispatch slot and the flags
        // before `pushad` saved ESP (hooks_026g.lohook_entry)
        let frame = ctx.reg(Reg::Esp).wrapping_add(8) as usize as *mut c_void;
        // SAFETY: the function's argument at its start (update_player_icons_frame).
        let ui = unsafe { *raw::update_player_icons_frame::ui(frame) };
        NonNull::new(ui as usize as *mut c_void).map(|this| GameWorldUi { this, thread: PhantomData })
    }

    /// The hud movie (hud.swf), when the hud has it.
    pub fn hud_movie(&self) -> Option<Movie<'t>> {
        // SAFETY: the handle is a live game_world_ui; its holder is a movie holder or 0.
        unsafe { movie_of(*raw::game_world_ui::hud_movie(self.this.as_ptr())) }
    }

    /// The player icons movie (player_icons.swf, the game world's own), when the hud has it.
    pub fn icons_movie(&self) -> Option<Movie<'t>> {
        // SAFETY: as hud_movie.
        unsafe { movie_of(*raw::game_world_ui::icons_movie(self.this.as_ptr())) }
    }
}

/// # Safety
/// `holder` is a movie holder or 0, alive on the hook's thread.
pub(super) unsafe fn movie_of<'t>(holder: u32) -> Option<Movie<'t>> {
    let holder = NonNull::new(holder as usize as *mut c_void)?;
    // SAFETY: a holder's view is a movie view or 0, and a view's movie a GFx movie or 0.
    let view = NonNull::new(unsafe { *raw::movie_holder::view(holder.as_ptr()) } as usize as *mut c_void)?;
    let movie = NonNull::new(unsafe { *raw::movie_view::movie(view.as_ptr()) } as usize as *mut c_void)?;
    Some(Movie { this: movie, thread: PhantomData })
}

/// A Scaleform movie the hud holds.
pub struct Movie<'t> {
    this: NonNull<c_void>,
    thread: PhantomData<&'t Thread>,
}

/// A GFx value (Scaleform::GFx::Value, 24 bytes): its type at +4, a bool's
/// value at +8 (0x2384E2).
#[repr(C)]
struct GfxValue {
    object: u32,
    kind: u32,
    value: u32,
    rest: [u32; 3],
}

const GFX_BOOL: u32 = 2;

impl Movie<'_> {
    /// Which movie this is (a new game world's is another), to tell it once.
    pub fn id(&self) -> usize {
        self.this.as_ptr() as usize
    }

    /// Invokes the movie's `method` with one bool (Movie::Invoke 0x511600, no
    /// result); whether the movie took it (it has that method: its root is
    /// made).
    pub fn invoke_bool(&self, method: &CStr, value: bool) -> bool {
        let argument = GfxValue { object: 0, kind: GFX_BOOL, value: u32::from(value), rest: [0; 3] };
        // SAFETY: a live movie on the hook's thread; the name is a C string and
        // the one argument a GFx bool, as the game's own Invokes pass them.
        let taken = unsafe {
            raw::engine_movie_invoke(self.this.as_ptr(), method.as_ptr().cast(), core::ptr::null_mut(),
                                     (&raw const argument).cast(), 1)
        };
        taken & 0xFF != 0
    }
}

impl Movie<'_> {
    /// Plain unsigned GFx values; the caller owns this synchronous argument array.
    pub(super) fn invoke_uints(&self, method: &CStr, values: &[u32; 4]) {
        let arguments = values.map(|value| GfxValue { object: 0, kind: 4, value, rest: [0; 3] });
        // SAFETY: live movie, NUL-terminated method and four POD GFx UInt values.
        unsafe { raw::engine_movie_invoke(self.this.as_ptr(), method.as_ptr().cast(),
            core::ptr::null_mut(), arguments.as_ptr().cast(), 4); }
    }
}
