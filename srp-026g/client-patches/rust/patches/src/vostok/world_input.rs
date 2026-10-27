//! The game world's keys: its on_keyboard_action (0x25CD00, thiscall: ECX
//! its input handler, game_world+1F8h; the input world, the DirectInput key,
//! the kind; `ret 0Ch`), which a key reaches before the player's handler.
//! At `world_keyboard.kind_test` (sites_026g.tsv, after its Tab test) EDI is
//! the input handler (its hud at +4) and EBP the function's frame: the key
//! at [ebp+0Ch], the kind at [ebp+10h].
//! What those registers hold is said here, once.

use core::ffi::c_void;
use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::gfx::Movie;
use super::hook::Thread;
use super::object::{Object, pointer, read};
use crate::raw;

/// A DirectInput key code (DIK_*).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key(pub u32);

impl Key {
    /// The digit key 1..9 this is (DIK_1..DIK_9 are 2..10), or none.
    pub fn digit(self) -> Option<u32> {
        (2..=10).contains(&self.0).then(|| self.0 - 1)
    }
}

/// A key's event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    Down,
    Held,
    Up,
    /// A value the game does not give.
    Other(u32),
}

impl KeyKind {
    fn parse(raw: u32) -> KeyKind {
        match raw {
            1 => KeyKind::Down,
            2 => KeyKind::Held,
            3 => KeyKind::Up,
            other => KeyKind::Other(other),
        }
    }
}

/// The key `world_keyboard.kind_test` looks at.
pub struct WorldKey<'t> {
    key: Key,
    kind: KeyKind,
    handler: Option<Object>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> WorldKey<'t> {
    pub fn at(ctx: &'t Context<'_>) -> WorldKey<'t> {
        let frame = ctx.reg(Reg::Ebp) as usize as *mut c_void;
        // SAFETY: EBP at the site is on_keyboard_action's frame, its
        // arguments at +0Ch and +10h.
        let (key, kind) = unsafe { (read(raw::world_keyboard_frame::key(frame)), read(raw::world_keyboard_frame::kind(frame))) };
        WorldKey { key: Key(key), kind: KeyKind::parse(kind), handler: Object::at(ctx.reg(Reg::Edi)), thread: PhantomData }
    }

    pub fn key(&self) -> Key {
        self.key
    }

    pub fn kind(&self) -> KeyKind {
        self.kind
    }

    /// The hud shows the cursor (as with the kits: the death screen, and
    /// the kits before the start).
    pub fn cursor_shown(&self) -> bool {
        // SAFETY: EDI at the site, the game world's input handler.
        self.handler.is_some_and(|handler| unsafe { read(raw::world_input_handler::cursor(handler.ptr())) } != 0)
    }

    /// The hud's movie, or none.
    pub fn hud_movie(&self) -> Option<Movie<'t>> {
        // SAFETY: the input handler's +0Ch is the hud's movie's owner or 0.
        let owner = unsafe { pointer(raw::world_input_handler::hud_movie(self.handler?.ptr())) }?;
        // SAFETY: a live movie owner.
        unsafe { Movie::of_owner(owner) }
    }
}

/// A mouse button's event (vostok::input::platform::mouse::process
/// 0x3B9D90), by its kind's low byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseKind {
    /// 0: gone down.
    Down,
    /// 1: let go.
    Up,
    /// 2: still held (and any other byte).
    Held,
}

/// The mouse's first button's code (0x3B9CA0: 151h + the button's bit).
const FIRST_BUTTON: u32 = 0x151;
/// Its buttons, 151h..158h.
const BUTTONS: u32 = 8;

/// The key `warmup_keys.mouse_key` is told: the game world's
/// on_mouse_key_action at its start.
#[derive(Clone, Copy)]
pub struct MouseKey<'t> {
    code: u32,
    kind: u8,
    shown: bool,
    thread: PhantomData<&'t Thread>,
}

impl<'t> MouseKey<'t> {
    pub fn at(ctx: &'t Context<'_>) -> MouseKey<'t> {
        let frame = ctx.site_esp() as usize as *mut c_void;
        // SAFETY: the function's arguments (mouse_key_frame, by the site's
        // ESP); ECX its this, the game world's input handler.
        unsafe {
            let shown = Object::at(ctx.reg(Reg::Ecx))
                .is_some_and(|handler| read(raw::world_input_handler::cursor(handler.ptr())) != 0);
            MouseKey { code: read(raw::mouse_key_frame::code(frame)), kind: read(raw::mouse_key_frame::kind(frame)),
                       shown, thread: PhantomData }
        }
    }

    /// The mouse's button (0..7), or none (a key of the keyboard's).
    pub fn button(&self) -> Option<u32> {
        let button = self.code.wrapping_sub(FIRST_BUTTON);
        (button < BUTTONS).then_some(button)
    }

    pub fn kind(&self) -> MouseKind {
        match self.kind {
            0 => MouseKind::Down,
            1 => MouseKind::Up,
            _ => MouseKind::Held,
        }
    }

    /// The hud shows the cursor (the game world takes the mouse's keys).
    pub fn cursor_shown(&self) -> bool {
        self.shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_and_kinds() {
        assert_eq!((Key(2).digit(), Key(4).digit(), Key(10).digit()), (Some(1), Some(3), Some(9)));
        assert_eq!((Key(1).digit(), Key(11).digit(), Key(0x1C).digit()), (None, None, None));
        assert_eq!((KeyKind::parse(1), KeyKind::parse(3), KeyKind::parse(0)), (KeyKind::Down, KeyKind::Up, KeyKind::Other(0)));
    }
}
