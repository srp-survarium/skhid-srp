//! The game's console as the patches extend it: a command of ours, and a
//! line of the game's own log, which the console shows (its tick reads the
//! logging core's lines, console_impl::tick 0x1D0D0) and the game's
//! Survarium_<user>.log keeps.
//!
//! A command is a cc_delegate (cc_delegate::cc_delegate 0xEC780) whose vtable
//! is a generated copy of cc_delegate's (a patch's `[[vtable]]`): its execute
//! slot the patch's handler, the rest the game's. console_commands::execute
//! (0xEB980) finds it by name in the list and calls that slot (0xEBB98) on
//! the game's thread, for a console line (mask 2) or a user.cfg line.

use core::cell::UnsafeCell;
use core::ffi::CStr;
use core::marker::PhantomData;

use super::hook::{Handle, Thread};
use super::project::NativeVtable;
use super::settings::Registration;
use super::ui::{TEXT_LIMIT, c_string};
use crate::raw;

/// The kinds a console line runs (on_text_commit 0x1D6D6: mask 2), and
/// user.cfg's (mask 2 too). A cc_delegate's save_to writes nothing
/// (0x15A4B0), so the command is never saved.
const CONSOLE: u32 = 2;

/// A command of ours in the engine's list, once registered.
pub struct Command {
    storage: UnsafeCell<raw::CcDelegateStorage>,
    registration: UnsafeCell<Registration>,
}

// SAFETY: registered inside a hook and read by the console's execute, both
// on the game's one thread (Thread is not Send).
unsafe impl Sync for Command {}

impl Command {
    pub const fn new() -> Command {
        Command {
            storage: UnsafeCell::new(raw::CcDelegateStorage::zeroed()),
            registration: UnsafeCell::new(Registration::Unregistered),
        }
    }

    /// Puts the command in the engine's list once, under `name`, its methods
    /// `methods` (the patch's generated copy of cc_delegate's vtable). Its
    /// delegate is empty: the copy's execute is the patch's and never calls
    /// it. Its status byte is 0, so the name alone executes it too
    /// (console_commands::execute 0xEBACF).
    pub fn register(&'static self, _: &Thread, name: &'static CStr, methods: NativeVtable<raw::CcDelegateMethods>) {
        // SAFETY: on the game's thread; the object (a cc_delegate's 60h bytes)
        // is this static's, never moved or freed; the name lives as long; the
        // delegate the constructor copies (0x28F040) is an empty one, its
        // manager 0, read before it returns. The constructor leaves
        // cc_delegate's vtable (0x9E7C0C), which the copy then replaces, as
        // the game writes cc_bool's after cc_value<bool>'s.
        unsafe {
            if matches!(*self.registration.get(), Registration::Registered) {
                return;
            }
            let object = (*self.storage.get()).handle();
            let empty = raw::LogFunctionStorage::zeroed();
            raw::engine_cc_delegate(Some(object), name.as_ptr().cast(), (&raw const empty).cast(), 0, CONSOLE);
            *object.vtable() = methods.address();
            *self.registration.get() = Registration::Registered;
        }
    }
}

impl Default for Command {
    fn default() -> Self {
        Self::new()
    }
}

/// The command the console executes (a [`Command`]'s object), on its thread.
pub struct ConsoleCommand<'t> {
    native: raw::CcDelegate,
    thread: &'t Thread,
}

impl<'t> Handle<'t> for ConsoleCommand<'t> {
    type Raw = raw::CcDelegate;

    unsafe fn from_raw(thread: &'t Thread, native: Option<raw::CcDelegate>) -> Option<Self> {
        Some(ConsoleCommand { native: native?, thread })
    }
}

impl<'t> ConsoleCommand<'t> {
    pub(in crate::vostok) fn raw(&self) -> raw::CcDelegate {
        self.native
    }

    /// The hook's thread it borrows.
    pub fn thread(&self) -> &'t Thread {
        self.thread
    }
}

/// The text after the command's name and its space, or none for a line of
/// the name alone (console_commands::execute passes 0 then).
pub struct ConsoleArguments<'t> {
    at: *const u8,
    thread: PhantomData<&'t Thread>,
}

impl<'t> ConsoleArguments<'t> {
    /// # Safety
    /// `at` is 0 or a C string alive while the hook runs, on its thread.
    pub(in crate::vostok) unsafe fn from_raw(_: &'t Thread, at: *const u8) -> ConsoleArguments<'t> {
        ConsoleArguments { at, thread: PhantomData }
    }

    pub(in crate::vostok) fn raw(&self) -> *const u8 {
        self.at
    }

    /// The text, when the line had any (and it is shorter than 200h bytes).
    pub fn text(&self) -> Option<&CStr> {
        // SAFETY: from_raw's: 0 or the line's C string, alive for the call.
        unsafe { c_string(self.at, TEXT_LIMIT) }
    }
}

/// The level of a line: info (as the console's own echo of a line, 0x1D67F).
const INFO: u32 = 4;

/// Writes `text` as a line of the game's log, channel "game", info, as
/// log_message (0x111ED0) writes its callers' with the format "%s": only when
/// the filters pass it (has_passed_filters, asked only while the log is
/// filtered), through the log's sink (none: nothing written).
pub fn game_line(_: &Thread, text: &CStr) {
    let state = raw::image::log_state();
    let mut function = raw::LogFunctionStorage::zeroed();
    let mut helper = raw::LogHelperStorage::zeroed();
    // SAFETY: the logging globals are the image's, read as every caller of
    // log_message reads them, on the game's thread. The function and the
    // helper are ours, alive until helper::write returns, which is the
    // line's whole life (log_message's own, on its stack): the function is
    // {its manager | 1, 0, the sink} (trivially copied) or empty, as the
    // callers make it; the helper is the template's copy (helper::helper
    // 0x111C90). The format "%s" takes one C string, the va_list's one word.
    unsafe {
        if *state.filtered() != 0 && raw::engine_log_passes(c"game".as_ptr().cast(), INFO) == 0 {
            return;
        }
        let f = function.handle();
        let sink = *state.sink();
        if sink != 0 {
            *f.manager() = raw::image::LOG_FUNCTION_MANAGER | 1;
            *f.sink() = sink;
        }
        let line = raw::engine_log_helper(
            Some(f),
            *state.context(),
            Some(helper.handle()),
            Some(raw::image::log_template()),
            c"srp".as_ptr().cast(),
            0,
            c"srp".as_ptr().cast(),
            c"game:".as_ptr().cast(),
            INFO,
        );
        let arguments = [text.as_ptr() as usize as u32];
        raw::engine_log_write(line, c"%s".as_ptr().cast(), arguments.as_ptr().cast());
    }
}
