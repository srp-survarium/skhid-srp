//! The game's language as the engine keeps it (patch 79,
//! docs/notes/language-option.md): the name every dictionary reader takes
//! (`raw::image::LOCALIZATION`), the console variable that user.cfg keeps a
//! choice in, the C locale the engine's start-up sets from the name, the
//! Windows user locale's language. The Settings' call and the save of
//! user.cfg are [`super::settings`]'s.

use core::cell::UnsafeCell;
use core::ffi::CStr;

use super::hook::Thread;
use super::settings::Registration;
use super::ui::c_string;
use crate::raw;

pub use super::ui::MAX_STRINGS;

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
