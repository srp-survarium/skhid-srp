//! The console's commands (vostok::console_commands): a cc_value<bool>
//! (0x1DDB0: the object, its name, the byte it sets, saved to user.cfg, its
//! kind) links itself at the head of the commands' list (0x1F1D66C), where
//! the options' look-up and user.cfg's lines find it by name.

use core::cell::UnsafeCell;

use super::hook::Thread;
use crate::raw;

/// An option of the game's menus that names a console command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionName {
    /// "g_private_messages_in_game", the gameplay options' option 4, which
    /// names no command in the original.
    PrivateMessagesInGame,
}

impl OptionName {
    fn text(self) -> *const u8 {
        match self {
            OptionName::PrivateMessagesInGame => raw::image::PRIVATE_MESSAGES_OPTION as usize as *const u8,
        }
    }
}

/// A console variable of a byte, made by the patch once and kept by the
/// game's list for the process: the command (cc_bool, 48h bytes), its byte,
/// and whether it was made. Zeros at load (the arena's patches.state).
#[repr(C)]
pub struct ConsoleBool {
    command: UnsafeCell<[u32; 0x12]>,
    value: UnsafeCell<u8>,
    made: UnsafeCell<u8>,
}

const _: () = assert!(size_of::<ConsoleBool>() == 0x4C);

// SAFETY: the game's thread alone touches it: the hooks (a Thread proves
// it), and the console's command, which writes the byte from the game's
// own thread (the options, user.cfg's lines).
unsafe impl Sync for ConsoleBool {}

impl ConsoleBool {
    pub const fn new() -> ConsoleBool {
        ConsoleBool { command: UnsafeCell::new([0; 0x12]), value: UnsafeCell::new(0), made: UnsafeCell::new(0) }
    }

    /// Makes the command `name` on this byte, once: a saved variable of the
    /// hud's kind (2, saved to user.cfg), as the hud's show_quests_progress
    /// (0x2339C9), a cc_bool (its vtable written over cc_value<bool>'s).
    pub fn make_once(&'static self, _: &Thread, name: OptionName) {
        // SAFETY: on the game's thread (the Thread); the fields are ours,
        // the command lives as long as the process (a static), as the
        // console's list keeps it.
        unsafe {
            if *self.made.get() != 0 {
                return;
            }
            *self.made.get() = 1;
            raw::engine_cc_bool(self.command.get().cast(), name.text(), self.value.get(), 1, 2);
            (*self.command.get())[0] = raw::image::CC_BOOL_VTABLE;
        }
    }

    /// The byte, as the console last set it.
    pub fn get(&self, _: &Thread) -> bool {
        // SAFETY: as make_once.
        unsafe { self.value.get().read_volatile() != 0 }
    }
}

impl Default for ConsoleBool {
    fn default() -> ConsoleBool {
        ConsoleBool::new()
    }
}
