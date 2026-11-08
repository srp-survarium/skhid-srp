//! The game world's keys, in game_world::on_keyboard_action (0x25CD00). At its
//! start (`tab_cursor.key`), the key and its kind. At `keyboard.tab_state`,
//! after it asked the keyboard for Tab (0x25CD26: BL Tab down,
//! stored at [esp+14h] for show_player_list). At 0x25CD39 `test bl,bl / jnz`
//! sends every key to the function's end while Tab is down (the Tab list
//! takes the keyboard); with BL clear the key goes on, to patch 58's kit keys
//! (0x25CD41) and the binder's actions. Before that ask, at `keyboard.key_state`
//! (0x25CD1C), the input world's keyboard: its keys' state.
//!
//! The platform keyboard's own poll, keyboard::execute (0x3B8D20): at
//! `keyboard.events` (0x3B914B) the events GetDeviceData buffered ([`KeyEvents`]),
//! which a desktop's moved keys ([`KeyMoves`]) can move before anything reads them.

use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::hud_input::Hud;
use crate::raw;

/// The key being handled, after the game's ask of Tab.
pub struct TabTest<'c, 'm> {
    ctx: &'c mut Context<'m>,
}

impl<'c, 'm> TabTest<'c, 'm> {
    pub fn at(ctx: &'c mut Context<'m>) -> TabTest<'c, 'm> {
        TabTest { ctx }
    }

    /// Tab is down.
    pub fn tab_down(&self) -> bool {
        self.ctx.reg(Reg::Ebx) & 0xFF != 0
    }

    /// The key goes on as with Tab up (show_player_list still gets Tab's byte, stored before).
    pub fn key_goes_on(&mut self) {
        self.ctx.set_reg8(Reg::Ebx, 0);
    }
}

/// A key by its DirectInput code (`DIK_*`), the index of the keyboard's state
/// and the code the game world's keys come with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dik(u8);

impl Dik {
    pub const TAB: Dik = Dik(0x0F);
    pub const LEFT_CONTROL: Dik = Dik(0x1D);
    pub const RIGHT_CONTROL: Dik = Dik(0x9D);
    /// DIK_LMENU: the left Alt, the key the game shows the names over the players by (render_name 0x1F2DCD).
    pub const LEFT_ALT: Dik = Dik(0x38);
    /// DIK_RMENU: the right Alt (alt_is_held 0x241200 asks both).
    pub const RIGHT_ALT: Dik = Dik(0xB8);
    pub const ESCAPE: Dik = Dik(0x01);
    pub const LEFT_BRACKET: Dik = Dik(0x1A);
    pub const RIGHT_BRACKET: Dik = Dik(0x1B);
    pub const H: Dik = Dik(0x23);
    pub const LEFT_SHIFT: Dik = Dik(0x2A);
    pub const RIGHT_SHIFT: Dik = Dik(0x36);
    pub const SPACE: Dik = Dik(0x39);
    pub const F11: Dik = Dik(0x57);
    pub const LEFT: Dik = Dik(0xCB);
    pub const RIGHT: Dik = Dik(0xCD);

    /// A key code as the game passes one (a u32): None past the 256 codes.
    fn of(raw: u32) -> Option<Dik> {
        u8::try_from(raw).ok().map(Dik)
    }
}

/// A key's event (the keyboard's process 0x3B9AF0): down and up once each,
/// held at every tick while it is down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    Down,
    Up,
    Held,
}

impl KeyKind {
    fn of(raw: u32) -> Option<KeyKind> {
        match raw {
            1 => Some(KeyKind::Down),
            2 => Some(KeyKind::Up),
            3 => Some(KeyKind::Held),
            _ => None,
        }
    }
}

/// A key at the game world's on_keyboard_action's start (`tab_cursor.key`).
pub struct GameWorldKey<'c, 'm> {
    ctx: &'c mut Context<'m>,
    hud: Hud<'c>,
    key: Dik,
    kind: KeyKind,
}

impl<'c, 'm> GameWorldKey<'c, 'm> {
    /// None for a code or a kind the keyboard does not give.
    pub fn at_start(ctx: &'c mut Context<'m>) -> Option<GameWorldKey<'c, 'm>> {
        use raw::site::tab_cursor_key as site;
        // SAFETY: the context is this site's: the game world's input handler
        // (its hud inside it) and the function's arguments, alive while it
        // runs on the game's thread.
        let (ui, key, kind) = unsafe {
            let (input, frame) = (site::input(&*ctx)?, site::frame(&*ctx)?);
            (input.ui(), *frame.key(), *frame.state())
        };
        let (key, kind) = (Dik::of(key)?, KeyKind::of(kind)?);
        // SAFETY: the hud inside the game world, alive for the hook, on its thread.
        let hud = unsafe { Hud::of(ui) };
        Some(GameWorldKey { ctx, hud, key, kind })
    }

    pub fn key(&self) -> Dik {
        self.key
    }

    pub fn kind(&self) -> KeyKind {
        self.kind
    }

    pub fn hud(&self) -> &Hud<'c> {
        &self.hud
    }

    pub fn thread(&self) -> &Thread {
        self.ctx.thread()
    }

    /// A Shift key is down in the keyboard this key came from (the input
    /// world of the function's first argument, as it asks it for Tab at
    /// 0x25CD11..0x25CD24): false unless that is the platform's input world
    /// and keyboard (both vtables checked).
    pub fn shift_down(&self) -> bool {
        use raw::site::tab_cursor_key as site;
        // SAFETY: the context is this site's: the function's first argument,
        // the input world, and its keyboard, alive while it runs on the
        // game's thread; their fields are read only after their vtables say
        // what they are (a platform keyboard's 256 states at +4).
        unsafe {
            let Some(world) = site::frame(&*self.ctx).and_then(|frame| frame.input_world()) else { return false };
            if *world.vtable() != raw::image::INPUT_WORLD_VTABLE {
                return false;
            }
            let Some(keyboard) = world.keyboard() else { return false };
            if *keyboard.vtable() != raw::image::KEYBOARD_VTABLE {
                return false;
            }
            [Dik::LEFT_SHIFT, Dik::RIGHT_SHIFT].iter().any(|key| *keyboard.keys().at(usize::from(key.0)).state() & 0x80 != 0)
        }
    }

    /// The key is taken here: no later handler (the player's) gets it. The
    /// caller then leaves by the site's `taken` exit (its `ret 0Ch`).
    pub fn take(self) {
        self.ctx.set_reg8(Reg::Eax, 1);
    }
}

/// The game world's keys at `keyboard.key_state` (0x25CD1C): on_keyboard_action
/// with the input world's keyboard, before it asks it for Tab. The game world
/// gets every key before the player's handler, a held one every tick (the
/// keyboard's process 0x3B9AF0, kind 3), so the state read here is this tick's
/// poll. Only the platform keyboard's state is read (its vtable checked).
pub struct GameWorldKeys<'t> {
    keys: raw::KeyState,
    hud: Hud<'t>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> GameWorldKeys<'t> {
    pub fn at_key_state(ctx: &'t Context<'_>) -> Option<GameWorldKeys<'t>> {
        use raw::site::keyboard_key_state as site;
        // SAFETY: the context is this site's: the input world's live keyboard
        // (EAX, its vtable+24h's answer) and the game world's input handler
        // (EDI, the hud inside it), on the game's thread. The keys are read
        // only after the vtable says it is the platform keyboard, whose state
        // is its +4.
        let (keys, ui) = unsafe {
            let (keyboard, input) = (site::keyboard(ctx)?, site::input(ctx)?);
            if *keyboard.vtable() != raw::image::KEYBOARD_VTABLE {
                return None;
            }
            (keyboard.keys(), input.ui())
        };
        // SAFETY: the hud inside the game world, alive for the hook, on its thread.
        let hud = unsafe { Hud::of(ui) };
        Some(GameWorldKeys { keys, hud, thread: PhantomData })
    }

    /// The key is down in this tick's poll (bit 80h, as is_key_down 0x3B8D00 reads it).
    pub fn down(&self, key: Dik) -> bool {
        // SAFETY: a platform keyboard's 256 states, the key one of them (a u8 code).
        unsafe { *self.keys.at(usize::from(key.0)).state() & 0x80 != 0 }
    }

    /// The hud the game world's input handler has.
    pub fn hud(&self) -> &Hud<'t> {
        &self.hud
    }
}

/// The keys a desktop's keyboard options move (xkb's caps:swapescape,
/// caps:escape, ctrl:nocaps, ctrl:swapcaps, altwin:swap_alt_win, ...), each
/// with the X keysym Wine's Wayland driver names it by: its layouts' key
/// names are the keysyms of each physical key's first level
/// (wayland_keyboard.c add_xkb_layout, xkb_keysym_get_name). Windows' own
/// names ("Esc", "Caps Lock") and the X11 driver's ("Control" for both
/// Ctrl keys) are none of these.
const FUNCTION_KEYS: [(Dik, &[u8]); 14] = [
    (Dik(0x01), b"Escape"),
    (Dik(0x0E), b"BackSpace"),
    (Dik(0x0F), b"Tab"),
    (Dik(0x1C), b"Return"),
    (Dik::LEFT_CONTROL, b"Control_L"),
    (Dik(0x2A), b"Shift_L"),
    (Dik(0x36), b"Shift_R"),
    (Dik::LEFT_ALT, b"Alt_L"),
    (Dik(0x3A), b"Caps_Lock"),
    (Dik::RIGHT_CONTROL, b"Control_R"),
    (Dik::RIGHT_ALT, b"Alt_R"),
    (Dik(0xDB), b"Super_L"),
    (Dik(0xDC), b"Super_R"),
    (Dik(0xDD), b"Menu"),
];

/// The function key a key's name is (`FUNCTION_KEYS`), if any.
fn function_key(name: &[u8]) -> Option<Dik> {
    FUNCTION_KEYS.iter().find(|(_, keysym)| *keysym == name).map(|&(dik, _)| dik)
}

/// Which function key each function key acts as: the one its name says.
/// Wine's Wayland driver gives DirectInput the physical keys' codes, and
/// moves nothing a desktop's options move but the keys' names (and the text):
/// with caps:swapescape the Caps Lock key is named Escape, and DirectInput
/// still says Caps Lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyMoves([Dik; FUNCTION_KEYS.len()]);

impl KeyMoves {
    /// Every key is itself.
    pub const NONE: KeyMoves = {
        let mut keys = [Dik(0); FUNCTION_KEYS.len()];
        let mut i = 0;
        while i < keys.len() {
            keys[i] = FUNCTION_KEYS[i].0;
            i += 1;
        }
        KeyMoves(keys)
    };

    /// Each function key as `name_of` names it: a key named another
    /// function key acts as that one; a key named otherwise, or not at all,
    /// stays itself.
    pub fn named(mut name_of: impl FnMut(Dik) -> Option<KeyName>) -> KeyMoves {
        let mut moves = KeyMoves::NONE;
        for (slot, &(key, _)) in moves.0.iter_mut().zip(FUNCTION_KEYS.iter()) {
            *slot = name_of(key).and_then(|name| function_key(name.bytes())).unwrap_or(key);
        }
        moves
    }

    /// The key `key` acts as.
    pub fn key(&self, key: Dik) -> Dik {
        FUNCTION_KEYS.iter().position(|&(k, _)| k == key).map_or(key, |i| self.0[i])
    }
}

/// A key's name as DirectInput gives it (DIPROP_KEYNAME, multibyte): at most
/// `KeyName::ROOM - 1` bytes.
#[derive(Clone, Copy)]
pub struct KeyName {
    bytes: [u8; KeyName::ROOM],
    len: usize,
}

impl KeyName {
    const ROOM: usize = 64;

    /// The name in a NUL-terminated buffer; None for an empty one.
    fn of(buffer: [u8; KeyName::ROOM]) -> Option<KeyName> {
        let len = buffer.iter().position(|&b| b == 0)?;
        (len > 0).then_some(KeyName { bytes: buffer, len })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// The events of this poll in keyboard::execute (0x3B8D20) at
/// `keyboard.events` (0x3B914B): what GetDeviceData buffered, before the
/// loop that sets the keys' states and modifier flags from them and before
/// process (0x3B9AF0) hands each to the handlers. A key moved here is moved
/// for all of them.
pub struct KeyEvents<'t> {
    keyboard: raw::Keyboard,
    count: usize,
    thread: PhantomData<&'t Thread>,
}

impl<'t> KeyEvents<'t> {
    /// GetDeviceData's buffer: at most 40h events (execute's
    /// DIPROP_BUFFERSIZE is 100h, its read 40h, 0x3B8D48).
    const MOST: usize = 0x40;

    /// None when the keyboard is not the platform one (its vtable) or the
    /// count is past the buffer.
    pub fn at(ctx: &'t Context<'_>) -> Option<KeyEvents<'t>> {
        use raw::site::keyboard_events as site;
        // SAFETY: the context is this site's, inside execute on the game's
        // thread: EDI its keyboard (`this`), EAX GetDeviceData's count. The
        // vtable is read only to check it is the platform keyboard.
        let (keyboard, count) = unsafe {
            let keyboard = site::keyboard(ctx)?;
            if *keyboard.vtable() != raw::image::KEYBOARD_VTABLE {
                return None;
            }
            (keyboard, site::count(ctx))
        };
        let count = usize::try_from(count).ok().filter(|&n| n <= Self::MOST)?;
        Some(KeyEvents { keyboard, count, thread: PhantomData })
    }

    /// The name DirectInput gives `key`: GetKeyNameTextW of its scan code in
    /// the calling thread's layout under Wine (dinput's keyboard objects),
    /// read by the game's own get_dik_name (0x3B9C00).
    pub fn name(&self, key: Dik) -> Option<KeyName> {
        let mut buffer = [0u8; KeyName::ROOM];
        // SAFETY: the platform keyboard of this poll (its DirectInput device
        // at +914h, alive while it polls), on its thread; the buffer is ours,
        // its size given.
        let named = unsafe {
            raw::engine_key_name(Some(self.keyboard), u32::from(key.0), buffer.as_mut_ptr(), KeyName::ROOM as u32)
        };
        if named == 0 {
            return None;
        }
        KeyName::of(buffer)
    }

    /// Every event's key becomes the one `moves` says it acts as.
    pub fn apply(&mut self, moves: &KeyMoves) {
        // SAFETY: the keyboard's buffer, `count` events of it filled by this
        // poll's GetDeviceData (at most its 40h), on its thread; each key
        // read once and written back.
        let events = unsafe { self.keyboard.events() };
        for i in 0..self.count {
            // SAFETY: as above, the event `i` < count.
            let key = unsafe { events.at(i).key() };
            // SAFETY: as above.
            if let Some(dik) = Dik::of(unsafe { *key }) {
                let moved = moves.key(dik);
                // SAFETY: as above.
                unsafe { *key = u32::from(moved.0) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(names: &[(u8, &[u8])]) -> KeyMoves {
        KeyMoves::named(|key| {
            let (_, name) = names.iter().find(|(k, _)| *k == key.0)?;
            let mut buffer = [0u8; KeyName::ROOM];
            buffer[..name.len()].copy_from_slice(name);
            KeyName::of(buffer)
        })
    }

    #[test]
    fn swapped_escape_and_caps_lock() {
        // caps:swapescape: the Caps Lock key's keysym is Escape, Escape's Caps_Lock.
        let moves = named(&[(0x01, b"Caps_Lock"), (0x3A, b"Escape"), (0x1D, b"Control_L"), (0x11, b"w")]);
        assert_eq!(moves.key(Dik(0x3A)), Dik(0x01));
        assert_eq!(moves.key(Dik(0x01)), Dik(0x3A));
        assert_eq!(moves.key(Dik(0x1D)), Dik(0x1D));
        assert_eq!(moves.key(Dik(0x11)), Dik(0x11), "a letter is never moved");
    }

    #[test]
    fn caps_lock_as_control() {
        // ctrl:nocaps: both keys are Control_L.
        let moves = named(&[(0x3A, b"Control_L"), (0x1D, b"Control_L")]);
        assert_eq!(moves.key(Dik(0x3A)), Dik::LEFT_CONTROL);
        assert_eq!(moves.key(Dik(0x1D)), Dik::LEFT_CONTROL);
    }

    #[test]
    fn windows_and_unknown_names_move_nothing() {
        // Windows' names, the X11 driver's stripped ones, AltGr's keysym, none.
        let moves = named(&[(0x01, b"Esc"), (0x3A, b"Caps Lock"), (0x1D, b"Control"), (0xB8, b"ISO_Level3_Shift")]);
        assert_eq!(moves, KeyMoves::NONE);
        assert_eq!(KeyMoves::named(|_| None), KeyMoves::NONE);
    }

    #[test]
    fn a_name_is_its_bytes_to_the_nul() {
        assert!(KeyName::of([0u8; KeyName::ROOM]).is_none());
        assert!(KeyName::of([b'x'; KeyName::ROOM]).is_none(), "no NUL: no name");
        let mut buffer = [0u8; KeyName::ROOM];
        buffer[..6].copy_from_slice(b"Escape");
        assert_eq!(KeyName::of(buffer).unwrap().bytes(), b"Escape");
    }
}
