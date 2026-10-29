//! Patch 79: the game language is a setting (docs/notes/language-option.md).
//!
//! user.cfg keeps the choice as the console variable `localization`, which
//! this patch registers before game::on_configs_loaded runs the file
//! (`configs.start`). After the file, before the dictionary loads
//! (`configs.texts`), the game language is resolved once: a registered
//! `-localization=` of the command line, else the saved choice, else the
//! launcher's menu language (its registry value), else English. It becomes
//! the engine's language (with the C locale a start with that flag gives) and
//! the saved choice, so the game's save writes it to user.cfg. The Settings'
//! Language list (main_menu) calls `srp_language` (`options.callback`): with a
//! registered name it saves that choice for the next start; with or without,
//! the movie is answered with its texts, the saved and the running language
//! and every registered language by its own name.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::language::{ConsoleString, OptionsCall, current, launcher_language, save_user_config,
                              set_current};
use crate::vostok::{Context, Flow, NoExit, LanguageOnOptionsCallExit};

/// A game language: its resource name (`-localization=`,
/// `resources/localization/<name>`) and its own name.
#[derive(Debug, PartialEq, Eq)]
pub struct Language {
    pub name: &'static CStr,
    pub native: &'static CStr,
}

// LANGUAGES, generated from localization_026g.py's registry.
include!("../../../languages_026g.rs");

/// The launcher's menu, by its index (launcher.md section 10.1).
const LAUNCHER: [&CStr; 5] = [c"english", c"russian", c"ukrainian", c"german", c"polish"];

/// The registered language of this name.
pub fn registered(name: &[u8]) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.name.to_bytes() == name)
}

/// The game language of a start: a registered `flag` (the command line's),
/// else a registered `saved` choice, else the launcher's menu language
/// (asked only then), else English.
pub fn resolve(flag: &[u8], saved: &[u8], launcher: impl FnOnce() -> Option<u32>) -> &'static Language {
    registered(flag)
        .or_else(|| registered(saved))
        .or_else(|| launcher().and_then(|i| LAUNCHER.get(i as usize)).and_then(|n| registered(n.to_bytes())))
        .unwrap_or(&LANGUAGES[0])
}

/// The choice user.cfg keeps (`localization <name>`). A value longer than
/// its buffer is a hand-made line; 256 bytes, as the engine's own string
/// variable (r_current_render_configuration).
static SAVED: ConsoleString<256> = ConsoleString::new();

pub fn on_configs_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    SAVED.register(ctx.thread(), c"localization");
    Flow::Continue
}

pub fn on_configs_texts(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let flag = current(thread);
    let language = resolve(flag.as_bytes(), SAVED.get(thread), || launcher_language(thread));
    if flag.as_bytes() != language.name.to_bytes() {
        set_current(thread, language.name);
    }
    SAVED.set(thread, language.name.to_bytes());
    Flow::Continue
}

/// What the Settings' Language list shows: its label, the note, the saved
/// choice, the running language, then each registered language's resource
/// name and own name.
pub fn on_options_call(ctx: &mut Context<'_>) -> Flow<LanguageOnOptionsCallExit> {
    let Some(call) = OptionsCall::at(ctx) else { return Flow::Continue };
    if call.method() != c"srp_language" {
        return Flow::Continue;
    }
    let thread = ctx.thread();
    if let Some(chosen) = call.string_argument(0).and_then(|name| registered(name.to_bytes())) {
        SAVED.set(thread, chosen.name.to_bytes());
        save_user_config(thread);
    }
    let (mut label, mut note) = ([0; 0x200], [0; 0x200]);
    let label = call.translate(c"st_srp_language_option", &mut label);
    let note = call.translate(c"st_srp_language_restart", &mut note);
    let saved = registered(SAVED.get(thread)).unwrap_or(&LANGUAGES[0]);
    let running = registered(current(thread).as_bytes()).unwrap_or(&LANGUAGES[0]);
    let mut values: [&CStr; 2 * LANGUAGES.len() + 4] = [c""; 2 * LANGUAGES.len() + 4];
    values[..4].copy_from_slice(&[label, note, saved.name, running.name]);
    for (pair, language) in values[4..].chunks_mut(2).zip(LANGUAGES.iter()) {
        pair.copy_from_slice(&[language.name, language.native]);
    }
    call.answer(c"root.srp_language", &values);
    Flow::Exit(LanguageOnOptionsCallExit::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_name_fits_the_engine_and_the_launcher_names_are_registered() {
        assert_eq!(LANGUAGES[0].name, c"english");
        for language in &LANGUAGES {
            assert!(!language.name.is_empty() && language.name.count_bytes() < 16, "{language:?}");
        }
        for name in LAUNCHER {
            assert!(registered(name.to_bytes()).is_some(), "{name:?}");
        }
    }

    #[test]
    fn the_flag_wins_then_the_saved_choice_then_the_launcher_then_english() {
        let never = || -> Option<u32> { panic!("the launcher is asked only without a flag and a choice") };
        assert_eq!(resolve(b"czech", b"german", never).name, c"czech");
        assert_eq!(resolve(b"original", b"german", never).name, c"german");
        assert_eq!(resolve(b"original", b"", || Some(2)).name, c"ukrainian");
        assert_eq!(resolve(b"original", b"klingon", || Some(4)).name, c"polish");
        assert_eq!(resolve(b"original", b"", || Some(5)).name, c"english");
        assert_eq!(resolve(b"original", b"", || None).name, c"english");
        assert_eq!(resolve(b"Czech", b"", || None).name, c"english");
    }
}
