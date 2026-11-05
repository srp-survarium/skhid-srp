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

use core::num::NonZeroU32;
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::{Context, Reg};
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
