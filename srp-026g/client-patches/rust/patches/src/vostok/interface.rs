//! The interface one key hides (patch 62), as the game has it:
//! - `hide_ui.match_toggle`: the game world's on_keyboard_action at its case
//!   of action 19h, [esp+10h] the game world's input handler (+18h the
//!   replay's controls movie; its hud inline at +4: the hud movie, the
//!   player icons' movie);
//! - `hide_ui.lobby_action`: lobby_menu::on_keyboard_action's call of
//!   get_binded_action, EAX the key binder, EDI the lobby's kind, ESI the
//!   key, [esp] the toggle's place, EBX the lobby menu (its movie);
//! - `hide_ui.match_reset`: initialize_match's Invoke of root.reset, ECX the
//!   hud movie, the call's four arguments on the stack, EBP the hud;
//! - the options' controls: the game's table of 28h rows (0xA9DB28) again
//!   with a 29th, SRP_CONTROLS, which patch_026g.py fills in the build from
//!   the exe's own rows and points game_options' six functions at.
//!
//! What those registers and fields hold is said here, once.

use core::cell::UnsafeCell;
use core::ffi::{CStr, c_void};
use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::gfx::Movie;
use super::hook::Thread;
use super::object::{Object, pointer, read};
use crate::raw;

/// The hud (game_world_ui): its movie and the player icons' movie.
#[derive(Clone, Copy)]
pub struct Hud<'t> {
    hud: Object,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Hud<'t> {
    fn movie_at(&self, holder: *mut u32) -> Option<Movie<'t>> {
        // SAFETY: a hud's holder field: a movie owner or 0, its movie by
        // [[owner+108h]+4].
        unsafe { Movie::of_owner(pointer(holder)?) }
    }

    /// The hud movie.
    pub fn movie(&self) -> Option<Movie<'t>> {
        // SAFETY: a hud's field's address.
        self.movie_at(unsafe { raw::game_world_ui::hud_movie(self.hud.ptr()) })
    }

    /// The player icons' movie.
    pub fn icons_movie(&self) -> Option<Movie<'t>> {
        // SAFETY: as movie.
        self.movie_at(unsafe { raw::game_world_ui::icons_movie(self.hud.ptr()) })
    }
}

/// The match's key case of action 19h.
pub struct MatchToggle<'t> {
    handler: Option<Object>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> MatchToggle<'t> {
    pub fn at(ctx: &'t Context<'_>) -> MatchToggle<'t> {
        MatchToggle { handler: Object::at(ctx.stack(0x10)), thread: PhantomData }
    }

    /// A replay: its controls' movie is there.
    pub fn replay(&self) -> bool {
        // SAFETY: the game world's input handler's field.
        self.handler.is_some_and(|handler| unsafe { read(raw::world_input_handler::replay_movie(handler.ptr())) } != 0)
    }

    /// The handler's hud (inline at +4).
    pub fn hud(&self) -> Option<Hud<'t>> {
        let handler = self.handler?;
        // SAFETY: the hud's address in the handler.
        let hud = Object::at(unsafe { raw::world_input_handler::hud(handler.ptr()) } as usize as u32)?;
        Some(Hud { hud, thread: PhantomData })
    }
}

/// The lobby's call of get_binded_action.
pub struct LobbyAction<'t> {
    binder: u32,
    kind: u32,
    key: u32,
    toggle: u32,
    lobby: Option<Object>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> LobbyAction<'t> {
    pub fn at(ctx: &'t Context<'_>) -> LobbyAction<'t> {
        LobbyAction { binder: ctx.reg(Reg::Eax), kind: ctx.reg(Reg::Edi), key: ctx.reg(Reg::Esi), toggle: ctx.stack(0),
                      lobby: Object::at(ctx.reg(Reg::Ebx)), thread: PhantomData }
    }

    /// The call as the lobby makes it: the key's action of the lobby's kind
    /// (its toggle's place the lobby's).
    pub fn lobby_action(&self) -> u32 {
        // SAFETY: the binder's own get_binded_action, with the site's
        // arguments.
        unsafe {
            raw::engine_binded_action(self.binder as usize as *const c_void, self.kind, self.key,
                                      self.toggle as usize as *mut u32)
        }
    }

    /// The key's action of `kinds`, its toggle written to a place of ours.
    pub fn action(&self, kinds: u32) -> u32 {
        let mut toggle = 0u32;
        // SAFETY: as lobby_action; the toggle's place ours for the call.
        unsafe { raw::engine_binded_action(self.binder as usize as *const c_void, kinds, self.key, &mut toggle) }
    }

    /// The lobby movie.
    pub fn movie(&self) -> Option<Movie<'t>> {
        // SAFETY: the lobby menu's field: a movie owner or 0.
        unsafe { Movie::of_owner(pointer(raw::lobby_menu::movie(self.lobby?.ptr()))?) }
    }
}

/// initialize_match's Invoke of root.reset.
pub struct MatchReset<'t> {
    movie: u32,
    args: [u32; 4],
    hud: Option<Object>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> MatchReset<'t> {
    pub fn at(ctx: &'t Context<'_>) -> MatchReset<'t> {
        MatchReset { movie: ctx.reg(Reg::Ecx), args: [ctx.stack(0), ctx.stack(4), ctx.stack(8), ctx.stack(12)],
                     hud: Object::at(ctx.reg(Reg::Ebp)), thread: PhantomData }
    }

    /// The Invoke as the game makes it; its answer.
    pub fn invoke(&self) -> u32 {
        let [name, result, args, count] = self.args;
        // SAFETY: Movie::Invoke with the game's own movie and arguments.
        unsafe {
            raw::engine_movie_invoke(self.movie as usize as *mut c_void, name as usize as *const u8,
                                     result as usize as *mut c_void, args as usize as *const c_void, count)
        }
    }

    /// The hud (EBP).
    pub fn hud(&self) -> Option<Hud<'t>> {
        self.hud.map(|hud| Hud { hud, thread: PhantomData })
    }
}

/// A buffer_string of the controls: begin, end, the buffer's end, its 32
/// characters.
#[repr(C)]
#[derive(Clone, Copy)]
struct ControlKey {
    begin: u32,
    end: u32,
    capacity: u32,
    buffer: [u8; 32],
}

/// A row of the options' controls (68h bytes): the action, its label's
/// localization key, the group (2 the game, 0 the lobby, 1 the chat), the
/// type, the key being set and the key kept.
#[repr(C)]
#[derive(Clone, Copy)]
struct ControlRow {
    action: u32,
    label: u32,
    group: u32,
    kind: u32,
    set: ControlKey,
    kept: ControlKey,
}

/// The rows: the game's 28h and patch 62's.
pub const CONTROL_ROWS: usize = 0x29;

/// The controls table, then the label of its last row's text.
#[repr(C)]
pub struct Controls {
    rows: UnsafeCell<[ControlRow; CONTROL_ROWS]>,
    label: [u8; 25],
}

// SAFETY: the rows are the game's to read and write (game_options, on its
// thread); no Rust code reads or writes them.
unsafe impl Sync for Controls {}

const EMPTY_KEY: ControlKey = ControlKey { begin: 0, end: 0, capacity: 0, buffer: [0; 32] };
const EMPTY_ROW: ControlRow = ControlRow { action: 0, label: 0, group: 0, kind: 0, set: EMPTY_KEY, kept: EMPTY_KEY };

/// The options' controls (the arena's symbol `SRP_CONTROLS`): its rows are
/// filled in the build by patch_026g.py (the game's 28h rows copied, the
/// 29th patch 62's: toggle_replay_ui, labelled `label`, each row's key
/// pointers into its own buffers), game_options' six functions pointed at
/// it.
#[unsafe(no_mangle)]
pub static SRP_CONTROLS: Controls =
    Controls { rows: UnsafeCell::new([EMPTY_ROW; CONTROL_ROWS]), label: *b"st_hide_interface_action\0" };

const _: () = assert!(size_of::<ControlRow>() == 0x68);

/// The label's text (for a test: the build fills the rows).
pub fn control_label() -> &'static CStr {
    CStr::from_bytes_with_nul(&SRP_CONTROLS.label).unwrap_or(c"")
}
