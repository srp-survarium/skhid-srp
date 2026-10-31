//! The game's language as the engine keeps it (patch 79,
//! docs/notes/language-option.md): the name every dictionary reader takes
//! (`raw::image::LOCALIZATION`), the console variable that user.cfg keeps a
//! choice in, the C locale the engine's start-up sets from the name, the
//! Windows user locale's language, and the Settings' call
//! (`game_options::scaleform_callback`) with the movie it answers.

use core::cell::UnsafeCell;
use core::ffi::CStr;

use super::context::Context;
use super::hook::Thread;
use super::ui::{GFX_MANAGED, GFX_STRING, movie_of};
use crate::raw;

pub use super::ui::MAX_STRINGS;

/// The C string at `text`, read up to `limit` bytes for its zero (none in
/// them: not a string here). core's `CStr::from_ptr` would call a strlen the
/// arena does not have.
///
/// # Safety
/// `text` is readable up to its zero or `limit` bytes, alive for `'a`.
pub(in crate::vostok) unsafe fn c_string<'a>(text: *const u8, limit: usize) -> Option<&'a CStr> {
    if text.is_null() {
        return None;
    }
    let mut len = 0;
    // SAFETY: the caller's: readable up to the zero or `limit`.
    while len < limit && unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: as above; `len` bytes and the zero after them.
    (len < limit).then(|| unsafe { CStr::from_bytes_with_nul_unchecked(core::slice::from_raw_parts(text, len + 1)) })
}

/// The longest text a GFx argument or the engine's name may be here.
const TEXT_LIMIT: usize = 0x200;

/// A copy of the game language's name (the engine keeps at most 15 bytes).
pub struct LanguageName {
    bytes: [u8; 16],
}

impl LanguageName {
    pub fn as_bytes(&self) -> &[u8] {
        let end = self.bytes.iter().position(|&b| b == 0).unwrap_or(self.bytes.len());
        &self.bytes[..end]
    }
}

/// The game language's name, the engine's buffer_string at
/// `raw::image::LOCALIZATION` (16 bytes): "original" or the command line's
/// `-localization=` until the patch writes it, before the dictionary loads.
pub fn current(_: &Thread) -> LanguageName {
    let mut bytes = [0u8; 16];
    // SAFETY: a static buffer_string of the image whose begin is its 16-byte
    // buffer, zero-terminated (its initializer 0x84E3E0, every writer
    // buffer_string::assign); the game's own thread reads it here.
    let name = unsafe { c_string(raw::image::localization().begin(), 16) };
    let name = name.map_or(&[][..], CStr::to_bytes);
    let len = name.len().min(15);
    bytes[..len].copy_from_slice(&name[..len]);
    LanguageName { bytes }
}

/// Makes `name` the game language before the dictionary loads
/// (`configs.texts`), with the C locale the engine's start-up would have set
/// for it: what a start with `-localization=<name>` gives.
pub fn set_current(_: &Thread, name: &CStr) {
    // SAFETY: buffer_string::assign on the image's string; a registered name
    // has at most 15 bytes (localization_026g.py), so it fits the buffer.
    unsafe { raw::engine_string_assign(Some(raw::image::localization()), name.as_ptr().cast()) };
    apply_locale(name);
}

const LC_ALL: u32 = 0;
const LC_COLLATE: u32 = 1;
const LC_NUMERIC: u32 = 4;

/// core::preinitialize's code page of a language (0xE3C5D..0xE3D17):
/// original and russian 1251, english 1252, polish 1250, any other none.
fn code_page(name: &CStr) -> Option<&'static CStr> {
    match name.to_bytes() {
        b"original" | b"russian" => Some(c".1251"),
        b"english" => Some(c".1252"),
        b"polish" => Some(c".1250"),
        _ => None,
    }
}

/// The C locale core::preinitialize sets for a language (0xE3C20..0xE3D2C):
/// LC_COLLATE ".ACP", then LC_ALL to its code page when it has one, then
/// LC_NUMERIC "C"; from "C", as the process started.
fn apply_locale(name: &CStr) {
    let set = |category: u32, locale: &CStr| {
        // SAFETY: the CRT's setlocale with a category it takes and a C string.
        unsafe { raw::engine_setlocale(category, locale.as_ptr().cast()) };
    };
    set(LC_ALL, c"C");
    set(LC_COLLATE, c".ACP");
    if let Some(page) = code_page(name) {
        set(LC_ALL, page);
    }
    set(LC_NUMERIC, c"C");
}

/// A Windows language id (LANGID): the primary language in its low ten bits,
/// the sublanguage (country, script) in the high six.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LangId(pub u16);

/// A Windows primary language id (LANG_RUSSIAN 0x19, ...): a LANGID without
/// its sublanguage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrimaryLanguage(pub u16);

impl LangId {
    pub const fn primary(self) -> PrimaryLanguage {
        PrimaryLanguage(self.0 & 0x3FF)
    }
}

/// The language of the player's Windows user locale (kernel32's
/// GetUserDefaultLCID, the game's import; under Wine the Unix locale's
/// LC_MESSAGES, which Wine's UI language is too).
pub fn user_locale(_: &Thread) -> LangId {
    // SAFETY: kernel32's GetUserDefaultLCID through the exe's import slot:
    // no arguments, an LCID whose low word is its LANGID.
    let lcid = unsafe { (raw::engine_get_user_default_lcid)() };
    LangId(lcid as u16)
}

/// Writes user.cfg now (game::save_user_config 0x1E9030, what the game does
/// when it closes), so a choice survives a crash.
pub fn save_user_config(_: &Thread) {
    // SAFETY: the game's own save of every saved console variable.
    unsafe { raw::engine_save_user_config() }
}

/// Whether a console variable is in the engine's list yet.
enum Registration {
    Unregistered,
    Registered,
}

/// The console variable object, a cc_string (cc_string::cc_string 0xEC860),
/// and its value's buffer.
#[repr(C)]
struct Storage<const N: usize> {
    object: raw::CcStringStorage,
    buffer: [u8; N],
}

/// A console variable of text the engine saves in user.cfg and sets from it
/// (`<name> <value>`, a line game::on_configs_loaded runs before the
/// dictionary loads). The engine writes the buffer itself (executing the
/// line); a patch reads and writes it inside a hook. `N` bounds a value: the
/// engine's execute (0xEC8C0) copies with strncpy_s(buffer, N, value, N).
pub struct ConsoleString<const N: usize> {
    storage: UnsafeCell<Storage<N>>,
    registration: UnsafeCell<Registration>,
}

// SAFETY: the game drives every hook, and the console's reads and writes of
// the buffer, on its one thread (Thread is not Send).
unsafe impl<const N: usize> Sync for ConsoleString<N> {}

impl<const N: usize> ConsoleString<N> {
    pub const fn new() -> ConsoleString<N> {
        ConsoleString {
            storage: UnsafeCell::new(Storage { object: raw::CcStringStorage::zeroed(), buffer: [0; N] }),
            registration: UnsafeCell::new(Registration::Unregistered),
        }
    }

    /// Puts the variable in the engine's list once, saved in user.cfg, so the
    /// file's line sets it and the game's save writes it.
    pub fn register(&'static self, _: &Thread, name: &'static CStr) {
        // SAFETY: on the game's thread; the object and the buffer are this
        // static's (never moved or freed), a cc_string's size and the size
        // given; the name lives as long.
        unsafe {
            if matches!(*self.registration.get(), Registration::Registered) {
                return;
            }
            let storage = self.storage.get();
            raw::engine_cc_string(Some((*storage).object.handle()), name.as_ptr().cast(),
                                  (&raw mut (*storage).buffer).cast(), N as u32, 1);
            *self.registration.get() = Registration::Registered;
        }
    }

    /// The value: the bytes before its zero (all of them when none).
    pub fn get(&self, _: &Thread) -> &[u8] {
        // SAFETY: on the game's thread, the buffer's only user while the
        // borrow lasts (a hook).
        let buffer = unsafe { &(*self.storage.get()).buffer };
        let end = buffer.iter().position(|&b| b == 0).unwrap_or(N);
        &buffer[..end]
    }

    /// Sets the value (cut to fit, zero-terminated).
    pub fn set(&self, _: &Thread, value: &[u8]) {
        // SAFETY: as get.
        let buffer = unsafe { &mut (*self.storage.get()).buffer };
        let len = value.len().min(N - 1);
        buffer[..len].copy_from_slice(&value[..len]);
        buffer[len..].fill(0);
    }
}

impl<const N: usize> Default for ConsoleString<N> {
    fn default() -> Self {
        Self::new()
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
    pub fn at(ctx: &'c Context<'_>) -> Option<OptionsCall<'c>> {
        use raw::site::options_callback as site;
        // SAFETY: the context is this site's: the options' interface and the
        // callback's frame, alive for the call, on the game's thread; the
        // method's name a C string the movie gave.
        let (options, frame, method) = unsafe {
            let (options, frame) = (site::options(ctx)?, site::frame(ctx)?);
            (options, frame, c_string(frame.method(), TEXT_LIMIT)?)
        };
        Some(OptionsCall { options, frame, method, _thread: ctx.thread() })
    }

    pub fn method(&self) -> &CStr {
        self.method
    }

    /// The call's argument `index` when it is a string.
    pub fn string_argument(&self, index: u32) -> Option<&CStr> {
        // SAFETY: the callback's arguments and their count, alive for the
        // call; a string's text (managed: its node's) a C string as long.
        unsafe {
            let count = *self.frame.count();
            let values = self.frame.arguments()?;
            if index >= count {
                return None;
            }
            let value = values.at(index as usize);
            let kind = *value.kind();
            if kind & 0x3F != GFX_STRING {
                return None;
            }
            let text = if kind & GFX_MANAGED != 0 {
                value.managed().map_or(core::ptr::null_mut(), |node| node.text())
            } else {
                value.text()
            };
            c_string(text, TEXT_LIMIT)
        }
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
    /// [`MAX_STRINGS`]; whether the movie took it.
    pub fn answer(&self, method: &CStr, values: &[&CStr]) -> bool {
        // SAFETY: the options' menu, a movie holder alive for the call, on the
        // hook's thread (the options' own Invoke reads it so, 0x1FF98F).
        let Some(movie) = (unsafe { movie_of(self.options.menu()) }) else { return false };
        movie.invoke_strings(method, values)
    }
}
