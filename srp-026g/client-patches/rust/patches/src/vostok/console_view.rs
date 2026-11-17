//! The console's output: the engine's scroll view (console_impl +18h, a
//! `vostok::ui::ui_scroll_view`) that console_impl::tick (0x1D0D0) fills
//! from the game's log every frame the console is open (docs/notes/console-scroll.md).
//!
//! The view's pad holds every line of the log as a 20-pixel row, the rows
//! off screen as one spacer above and one tail below; the pad's position is
//! minus the pixels scrolled. Following the last line (its flag), the tick
//! keeps the pad at the end; not following, it fills the rows at the pad's
//! position and, while the pad stays where it is, only lengthens the tail
//! for new lines (0x1D209). So the engine's flag and the pad's position are
//! the whole state of a scroll: nothing is kept here.
//!
//! The three moments the console can be scrolled, each on the game's thread:
//! a key the open console takes (`console.key`), a mouse move it takes
//! (`console.wheel`), and its opening (`console.activate`).

use core::marker::PhantomData;

use super::context::Context;
use super::hook::Thread;
use super::keyboard::{Dik, KeyKind};
use crate::raw;

/// The console's scroll view, on its thread.
pub struct ConsoleView<'t> {
    view: raw::UiScrollView,
    thread: PhantomData<&'t Thread>,
}

/// Where the view stands, read once (pixels down the pad).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    /// The pad's row at the view's top: 0 the log's first line.
    pub top: f32,
    /// The furthest `top` goes: the newest line the view's last.
    pub end: f32,
    /// A line's height (the tick's step, 20).
    pub line: f32,
    /// The lines the view shows (floor(height / line), as the tick fills it).
    pub lines: u32,
}

/// Where the view goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// The newest lines, following the new ones: the engine's own follow.
    Newest,
    /// Held `top` pixels down the pad; new lines come in below, unseen.
    At(f32),
}

/// The pad as it stands.
struct Pad {
    top: f32,
    length: f32,
    height: f32,
}

impl Pad {
    fn end(&self) -> f32 {
        (self.length - self.height).max(0.0)
    }
}

impl<'t> ConsoleView<'t> {
    /// # Safety
    /// `console` is the live game console, on the game's thread `'t`.
    unsafe fn of(_: &'t Thread, console: raw::ConsoleImpl) -> Option<ConsoleView<'t>> {
        // SAFETY: the caller's: the console's field, its view made by its
        // constructor and kept until its destructor.
        let view = unsafe { console.view() }?;
        Some(ConsoleView { view, thread: PhantomData })
    }

    fn pad(&self) -> Option<Pad> {
        // SAFETY: the live view's floats (its window's height, its pad's
        // position and length), on its thread.
        let pad = unsafe {
            Pad { top: -*self.view.pad_top(), length: *self.view.pad_length(), height: *self.view.height() }
        };
        [pad.top, pad.length, pad.height].iter().all(|v| v.is_finite()).then_some(pad)
    }

    /// Where it stands; none before the tick first filled it (no step yet).
    pub fn shown(&self) -> Option<Shown> {
        let pad = self.pad()?;
        // SAFETY: the view's v scroll, inside it.
        let line = unsafe { *self.view.source().step() };
        if !(line.is_finite() && line > 0.0) {
            return None;
        }
        let lines = (pad.height / line) as u32; // a float to u32 saturates; floor for >= 0
        Some(Shown { top: pad.top.max(0.0), end: pad.end(), line, lines })
    }

    /// Puts the view at `place`: the flag set or cleared, the pad moved
    /// (through the v scroll, whose bar follows). The tick fills it again at
    /// its next frame, the pad having moved; at the newest lines the pad
    /// goes to the end, where the tick's own follow keeps it.
    pub fn show(&mut self, place: Place) {
        let Some(pad) = self.pad() else { return };
        let (follow, top) = match place {
            Place::Newest => (1, pad.end()),
            Place::At(top) => (0, top.clamp(0.0, pad.end())),
        };
        // SAFETY: the live view and its v scroll, on the game's thread (the
        // console's input and opening run where its tick does); both
        // functions thiscall, set_follow_last_line a flag bit, set_position
        // the pad's y.
        unsafe {
            raw::engine_scroll_view_set_follow(Some(self.view), follow);
            raw::engine_scroll_set_position(Some(self.view.source()), -top);
        }
    }
}

/// A key the open console takes (`console.key`), before it handles it.
pub struct ConsoleKey<'c> {
    pub view: ConsoleView<'c>,
    pub key: Dik,
    pub kind: KeyKind,
}

impl<'c> ConsoleKey<'c> {
    /// None for a code or a kind the keyboard does not give.
    pub fn at(ctx: &'c Context<'_>) -> Option<ConsoleKey<'c>> {
        use raw::site::console_key as site;
        // SAFETY: the context is this site's: ECX the open console,
        // EAX the key's kind and the function's stack arguments, on the
        // game's thread.
        let (console, kind, key) = unsafe { (site::console(ctx)?, site::kind(ctx), *site::frame(ctx)?.key()) };
        let (key, kind) = (Dik::of(key)?, KeyKind::of(kind)?);
        // SAFETY: the site's console, alive while it runs.
        let view = unsafe { ConsoleView::of(ctx.thread(), console) }?;
        Some(ConsoleKey { view, key, kind })
    }
}

/// A mouse move the open console takes (`console.wheel`).
pub struct ConsoleWheel<'c> {
    pub view: ConsoleView<'c>,
    /// The wheel's turn since the last poll: 120 a notch, forward (away
    /// from the player) positive.
    pub turn: i32,
}

impl<'c> ConsoleWheel<'c> {
    pub fn at(ctx: &'c Context<'_>) -> Option<ConsoleWheel<'c>> {
        use raw::site::console_wheel as site;
        // SAFETY: the context is this site's: ECX the open console's input
        // handler (its +268h) and the function's stack arguments, on the
        // game's thread.
        let (console, z) = unsafe { (site::input(ctx)?.console(), *site::frame(ctx)?.z()) };
        // SAFETY: the handler's console, alive while it runs.
        let view = unsafe { ConsoleView::of(ctx.thread(), console) }?;
        Some(ConsoleWheel { view, turn: z as i32 })
    }
}

/// The console about to open (`console.activate`): its view.
pub fn opening<'c>(ctx: &'c Context<'_>) -> Option<ConsoleView<'c>> {
    // SAFETY: the context is this site's: ECX the game console, on the
    // game's thread.
    unsafe { ConsoleView::of(ctx.thread(), raw::site::console_activate::console(ctx)?) }
}
