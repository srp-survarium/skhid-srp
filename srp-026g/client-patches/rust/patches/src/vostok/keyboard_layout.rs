//! The keyboard layout the engine types text with.
//!
//! The window procedure (`message_processor`, 0x181E0, the main thread) gets
//! a layout change as WM_INPUTLANGCHANGEREQUEST (`window.lang_request`) and
//! WM_INPUTLANGCHANGE (`window.lang_change`), the HKL in lParam. The text a
//! key makes comes from `keyboard::translate_text` (0x3B92D0) on the engine's
//! logic thread, which asks `GetKeyboardLayout(0)`, its own thread's layout
//! (`keyboard.text_layout`). On Windows the engine's
//! `ActivateKeyboardLayout(hkl, KLF_SETFORPROCESS)` gives the logic thread
//! the layout too; Wine does not support that flag, so there the logic
//! thread's layout never changes.
//!
//! [`ActiveLayout`] carries the window procedure's layout to the logic
//! thread: one atomic, written by the window procedure, read by the text.
//!
//! The console's text edit types a key with an insert_char_action
//! (`console.char_typed`, 0x34B4DA): a fixed character, or the first letter
//! of the key's DirectInput name, never the layout's text. [`ConsoleKey`]
//! asks the keyboard for the key's text as the chat does
//! (`swf_input_translator::process_keyboard`, 0x1FDDE1: the input world's
//! keyboard, translate_text). [`ConsoleShown`] tells the window procedure
//! the console is open, a text input as the chat is.

use core::marker::PhantomData;
use core::num::NonZeroU32;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use super::console::ConsoleChar;
use super::context::{Context, Reg};
use super::hook::Thread;
use crate::raw;

/// A keyboard layout's handle (an HKL): never 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hkl(NonZeroU32);

impl Hkl {
    /// The HKL in a message's lParam or a register: none for 0.
    fn of(raw: u32) -> Option<Hkl> {
        NonZeroU32::new(raw).map(Hkl)
    }

    fn raw(self) -> u32 {
        self.0.get()
    }
}

/// The layout the window procedure was last told of, shared by its thread
/// and the logic thread. A `static` of the patch that owns it.
pub struct ActiveLayout(AtomicU32);

impl ActiveLayout {
    /// No layout told yet.
    pub const fn none() -> ActiveLayout {
        ActiveLayout(AtomicU32::new(0))
    }

    /// The window procedure was told of `hkl`.
    pub fn set(&self, hkl: Hkl) {
        // One word, read alone: no other memory is published with it.
        self.0.store(hkl.raw(), Ordering::Relaxed);
    }

    /// The layout last told, if any (0 is no HKL: never stored by `set`).
    pub fn get(&self) -> Option<Hkl> {
        Hkl::of(self.0.load(Ordering::Relaxed))
    }
}

/// Runs `f` with `hkl` the calling thread's layout, then gives the thread
/// its own back (`ActivateKeyboardLayout(hkl, 0)`, then again with the
/// layout it answered: the thread's GetKeyNameTextW and GetKeyboardLayout(0)
/// say the same after as before). None, `f` not run, when the system
/// refuses `hkl`.
pub fn with_thread_layout<R>(hkl: Hkl, f: impl FnOnce() -> R) -> Option<R> {
    // SAFETY: user32's ActivateKeyboardLayout, the game's import (its
    // delay-loaded slot, which the window procedure calls too): an HKL and
    // no flags, on the calling thread. It answers the thread's layout
    // before, 0 when it refused.
    let own = Hkl::of(unsafe { (raw::engine_activate_keyboard_layout)(hkl.raw(), 0) })?;
    let result = f();
    // SAFETY: as above, the layout the thread had.
    unsafe { (raw::engine_activate_keyboard_layout)(own.raw(), 0) };
    Some(result)
}

/// The layout a WM_INPUTLANGCHANGEREQUEST asks for (`window.lang_request`),
/// before the game's check that a text input is focused.
pub fn requested_layout(ctx: &Context<'_>) -> Option<Hkl> {
    // SAFETY: the context is this site's, inside message_processor with its
    // three registers pushed: the site's ESP is its frame, on the window's
    // thread; the lParam is a value, read once.
    let lparam = unsafe { raw::site::window_lang_request::frame(ctx).map(|frame| *frame.lparam()) };
    lparam.and_then(Hkl::of)
}

/// The layout a WM_INPUTLANGCHANGE reports (`window.lang_change`), before
/// the same check guards the engine's ActivateKeyboardLayout.
pub fn activated_layout(ctx: &Context<'_>) -> Option<Hkl> {
    // SAFETY: as requested_layout, at this site: the lParam the original code
    // passes to ActivateKeyboardLayout (0x183F0).
    let lparam = unsafe { raw::site::window_lang_change::frame(ctx).map(|frame| *frame.lparam()) };
    lparam.and_then(Hkl::of)
}

/// `keyboard::translate_text` after its `GetKeyboardLayout(0)`
/// (`keyboard.text_layout`): the layout its key is typed with.
pub struct TextLayout<'c, 'm> {
    ctx: &'c mut Context<'m>,
}

impl<'c, 'm> TextLayout<'c, 'm> {
    pub fn at(ctx: &'c mut Context<'m>) -> TextLayout<'c, 'm> {
        TextLayout { ctx }
    }

    /// Type with `hkl` instead of the calling thread's layout (EAX, which
    /// the replayed `mov edi,eax` hands MapVirtualKeyExA and ToUnicodeEx).
    pub fn type_with(self, hkl: Hkl) {
        self.ctx.set_reg(Reg::Eax, hkl.raw());
    }
}

/// Whether the game's console is shown: set by its on_activate and
/// on_deactivate (`console.opened`, `console.closed`, the logic thread),
/// read by the window procedure. A `static` of the patch that owns it.
pub struct ConsoleShown(AtomicBool);

impl ConsoleShown {
    pub const fn hidden() -> ConsoleShown {
        ConsoleShown(AtomicBool::new(false))
    }

    pub fn set(&self, shown: bool) {
        // One byte, read alone.
        self.0.store(shown, Ordering::Relaxed);
    }

    pub fn get(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// A key the console's text edit types (`console.char_typed`, the start of
/// insert_char_action::execute 0x34B4D0), with the platform keyboard its
/// input world has.
pub struct ConsoleKey<'t> {
    frame: raw::InsertCharFrame,
    key: u32,
    keyboard: raw::Keyboard,
    thread: PhantomData<&'t Thread>,
}

impl<'t> ConsoleKey<'t> {
    /// None when the action has no input world, or it or its keyboard is
    /// not the game's (their vtables): the game's own path then.
    pub fn at(ctx: &'t Context<'_>) -> Option<ConsoleKey<'t>> {
        use raw::site::console_char_typed as site;
        // SAFETY: the context is this site's, inside execute on the logic
        // thread: EDI the action, alive while it runs, the site's ESP its
        // frame. The input world and the keyboard are read past their
        // vtables only, each checked first.
        let (frame, key, keyboard) = unsafe {
            let (action, frame) = (site::action(ctx)?, site::frame(ctx)?);
            let input = action.input()?;
            if *input.vtable() != raw::image::INPUT_WORLD_VTABLE {
                return None;
            }
            let keyboard = input.keyboard()?;
            if *keyboard.vtable() != raw::image::KEYBOARD_VTABLE {
                return None;
            }
            (frame, *action.key(), keyboard)
        };
        Some(ConsoleKey { frame, key, keyboard, thread: PhantomData })
    }

    /// The key's text as the chat gets it: the keyboard's translate_text
    /// (0x3B92D0; its layout patch 107's), if it gives a character the
    /// console shows.
    pub fn text(&self) -> Option<ConsoleChar> {
        let mut text = 0u16;
        // SAFETY: the input world's platform keyboard, alive while the game
        // runs, on the logic thread (the chat's caller's); the WCHAR is ours.
        let typed = unsafe { raw::engine_key_text(Some(self.keyboard), self.key, &mut text) };
        if typed == 0 {
            return None;
        }
        ConsoleChar::of(text)
    }

    /// The console types `character` (the site's `typed` exit inserts it).
    pub fn type_char(self, character: ConsoleChar) {
        // SAFETY: execute's character, in its frame on this thread.
        unsafe { *self.frame.typed() = character.byte() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_layout_until_one_is_set() {
        let layout = ActiveLayout::none();
        assert_eq!(layout.get(), None);
        let russian = Hkl::of(0x0419_0409).unwrap();
        layout.set(russian);
        assert_eq!(layout.get(), Some(russian));
        let polish = Hkl::of(0x0415_0409).unwrap();
        layout.set(polish);
        assert_eq!(layout.get(), Some(polish));
    }

    #[test]
    fn zero_is_no_layout() {
        assert_eq!(Hkl::of(0), None);
    }
}
