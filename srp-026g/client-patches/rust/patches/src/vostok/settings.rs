//! The game's Settings as the patches extend them: the main menu's call to the
//! exe (`game_options::scaleform_callback`) with the movie it answers, the
//! save of user.cfg, and a console variable of ours that user.cfg keeps
//! (patch 79's Language, patch 83's "Hide hit markers" and "Hide player
//! scores").

use core::cell::UnsafeCell;
use core::ffi::CStr;

use super::context::Context;
use super::site;
use super::hook::Thread;
use super::ui::{TEXT_LIMIT, c_string, movie_of, string_argument};
use crate::raw;

/// Writes user.cfg now (game::save_user_config 0x1E9030, what the game does
/// when it closes), so a choice survives a crash.
pub fn save_user_config(_: &Thread) {
    // SAFETY: the game's own save of every saved console variable.
    unsafe { raw::engine_save_user_config() }
}

/// The console variable object, a cc_bool (cc_value<bool>::cc_value 0x1DDB0),
/// and its value.
#[repr(C)]
struct BoolStorage {
    object: raw::CcBoolStorage,
    value: u8,
}

/// Whether a console variable is in the engine's list yet (ConsoleBool's
/// and language's ConsoleString's).
pub(super) enum Registration {
    Unregistered,
    Registered,
}

/// The console's kinds a variable answers to: user.cfg's lines run with mask
/// 2 (load_cc_script, on_configs_loaded 0x1EA2C7), so a kind-2 variable is
/// set from the file (the hud's own show_quests_progress, patch 58's
/// g_private_messages_in_game).
const USER_CFG: u32 = 2;

/// A console variable on or off that the engine saves in user.cfg and sets
/// from it (`<name> on|off`, a line game::on_configs_loaded runs). The engine
/// writes the value itself (executing the line); a patch reads and writes it
/// inside a hook. Off until the file says otherwise.
pub struct ConsoleBool {
    storage: UnsafeCell<BoolStorage>,
    registration: UnsafeCell<Registration>,
}

// SAFETY: the game drives every hook, and the console's reads and writes of
// the value, on its one thread (Thread is not Send).
unsafe impl Sync for ConsoleBool {}

impl ConsoleBool {
    pub const fn new() -> ConsoleBool {
        ConsoleBool {
            storage: UnsafeCell::new(BoolStorage { object: raw::CcBoolStorage::zeroed(), value: 0 }),
            registration: UnsafeCell::new(Registration::Unregistered),
        }
    }

    /// Puts the variable in the engine's list once, saved in user.cfg, so the
    /// file's line sets it and the game's save writes it. The constructor
    /// leaves its base's vtable; a cc_bool's is written after it, as the
    /// game's own variables have it.
    pub fn register(&'static self, _: &Thread, name: &'static CStr) {
        // SAFETY: on the game's thread; the object and the value are this
        // static's (never moved or freed), a cc_bool's size and its bool; the
        // name lives as long.
        unsafe {
            if matches!(*self.registration.get(), Registration::Registered) {
                return;
            }
            let storage = self.storage.get();
            let object = (*storage).object.handle();
            raw::engine_cc_bool(Some(object), name.as_ptr().cast(), &raw mut (*storage).value, 1, USER_CFG);
            *object.vtable() = raw::image::CC_BOOL_VTABLE;
            *self.registration.get() = Registration::Registered;
        }
    }

    pub fn get(&self, _: &Thread) -> bool {
        // SAFETY: on the game's thread, the value's only user while it runs.
        unsafe { (*self.storage.get()).value != 0 }
    }

    pub fn set(&self, _: &Thread, on: bool) {
        // SAFETY: as get.
        unsafe { (*self.storage.get()).value = u8::from(on) }
    }
}

impl Default for ConsoleBool {
    fn default() -> Self {
        Self::new()
    }
}

/// The console variable object, a cc_float (made as the game makes its own:
/// cc_value<bool>::cc_value's base, then the cc_float's vtable and range, as
/// s_general_volume's static initializer 0x84D440 has them), and its value.
#[repr(C)]
struct FloatStorage {
    object: raw::CcFloatStorage,
    value: f32,
}

/// A float console variable that the engine saves in user.cfg and sets from
/// it (`<name> <value>`), within its range: a value out of it, or not a
/// number, stores the least (the game's own setter 0xECB70). The stock
/// Settings' sliders name theirs by name (options_item_float), so a stock
/// slider whose variable the game lacks (`s_music_volume`) works once it is
/// registered. Its default until the file says otherwise.
pub struct ConsoleFloat {
    storage: UnsafeCell<FloatStorage>,
    range: (f32, f32),
    registration: UnsafeCell<Registration>,
}

// SAFETY: as ConsoleBool's: the hooks and the console's reads and writes of
// the value run on the game's one thread.
unsafe impl Sync for ConsoleFloat {}

impl ConsoleFloat {
    /// `least..=most`, `default` within it.
    pub const fn new(default: f32, least: f32, most: f32) -> ConsoleFloat {
        ConsoleFloat {
            storage: UnsafeCell::new(FloatStorage { object: raw::CcFloatStorage::zeroed(), value: default }),
            range: (least, most),
            registration: UnsafeCell::new(Registration::Unregistered),
        }
    }

    /// Puts the variable in the engine's list once, saved in user.cfg, as
    /// ConsoleBool::register; the cc_float's vtable and range written after
    /// the base constructor.
    pub fn register(&'static self, _: &Thread, name: &'static CStr) {
        // SAFETY: on the game's thread; the object (a cc_float's size) and the
        // value are this static's, never moved or freed; the name lives as
        // long. The base constructor writes the list link, the name, the
        // kind, saved and the value's address (+40h) and two bytes at +44h,
        // which the range then overwrites.
        unsafe {
            if matches!(*self.registration.get(), Registration::Registered) {
                return;
            }
            let storage = self.storage.get();
            let object = (*storage).object.handle();
            raw::engine_cc_float(Some(object), name.as_ptr().cast(), &raw mut (*storage).value, 1, USER_CFG);
            *object.vtable() = raw::image::CC_FLOAT_VTABLE;
            *object.least() = self.range.0;
            *object.most() = self.range.1;
            *self.registration.get() = Registration::Registered;
        }
    }

    pub fn get(&self, _: &Thread) -> f32 {
        // SAFETY: on the game's thread, the value's only user while it runs.
        unsafe { (*self.storage.get()).value }
    }
}

/// The Settings' call to the exe at `options.callback`: the movie's
/// `ExternalInterface.call(method, ...)` at game_options::scaleform_callback's
/// start (0x1FF5D0), before the options match the method's name.
pub struct OptionsCall<'c> {
    options: raw::GameOptionsInterface,
    frame: raw::OptionsCallbackFrame,
    method: &'c CStr,
    _thread: &'c Thread,
}

impl<'c> OptionsCall<'c> {
    pub fn at(ctx: &'c Context<'_, site::OptionsCallback>) -> Option<OptionsCall<'c>> {
        use raw::site::options_callback as at;
        let (options, frame) = (at::options(ctx)?, at::frame(ctx)?);
        // SAFETY: the site's options interface and the callback's frame, alive
        // for the call, on the game's thread; the method's name a C string
        // the movie gave.
        let method = unsafe { c_string(frame.method(), TEXT_LIMIT)? };

        Some(OptionsCall { options, frame, method, _thread: ctx.thread() })
    }

    pub fn method(&self) -> &CStr {
        self.method
    }

    /// The call's argument `index` when it is a string.
    pub fn string_argument(&self, index: u32) -> Option<&CStr> {
        // SAFETY: the callback's arguments and their count, alive for the call.
        unsafe { string_argument(self.frame.arguments(), *self.frame.count(), index) }
    }

    /// A dictionary text of the running language (text_translator::
    /// translate_text 0x1FF0F0, the key itself when the dictionary has none)
    /// into `out`.
    pub fn translate<'o>(&self, key: &CStr, out: &'o mut [u8; 0x200]) -> &'o CStr {
        out[0] = 0;
        // SAFETY: the options' game and its text translator, alive for the
        // call; translate_text copies a text shorter than 200h bytes with its
        // zero into `out` (0x1FF15D) and leaves it alone otherwise.
        unsafe {
            if let Some(game) = self.options.game() {
                raw::engine_translate_text(Some(game.text_translator()), key.as_ptr().cast(), out.as_mut_ptr());
            }
        }
        out[0x1FF] = 0;
        CStr::from_bytes_until_nul(out).unwrap_or(c"")
    }

    /// Answers the main menu (the movie that called): invokes its `method`
    /// with these strings (UTF-8, as the dictionaries are), at most
    /// [`super::ui::MAX_STRINGS`]; whether the movie took it.
    pub fn answer(&self, method: &CStr, values: &[&CStr]) -> bool {
        // SAFETY: the options' menu, a movie holder alive for the call, on the
        // hook's thread (the options' own Invoke reads it so, 0x1FF98F).
        let Some(movie) = (unsafe { movie_of(self.options.menu()) }) else { return false };
        movie.invoke_strings(method, values)
    }
}
