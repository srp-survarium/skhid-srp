//! The match's interface: survarium::game_world_ui (the hud) and the
//! Scaleform movies it holds, the hud's and the player icons'.

use core::ffi::{CStr, c_void};
use core::marker::PhantomData;
use core::ptr::NonNull;

use super::hook::{Handle, Thread};
use crate::raw;

/// The hud, survarium::game_world_ui. A [`Handle`]: a hook is given it.
pub struct GameWorldUi<'t> {
    this: NonNull<c_void>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Handle<'t> for GameWorldUi<'t> {
    /// # Safety
    /// `raw` is a game_world_ui (or 0), alive for the hook, on `_`'s thread.
    unsafe fn from_raw(_: &'t Thread, raw: u32) -> Option<Self> {
        NonNull::new(raw as usize as *mut c_void).map(|this| GameWorldUi { this, thread: PhantomData })
    }
}

impl<'t> GameWorldUi<'t> {
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
unsafe fn movie_of<'t>(holder: u32) -> Option<Movie<'t>> {
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
