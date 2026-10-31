//! Patch 79: the game language is a setting (docs/notes/language-option.md).
//!
//! user.cfg keeps the choice as the console variable `localization`, which
//! this patch registers before game::on_configs_loaded runs the file
//! (`configs.start`). After the file, before the dictionary loads
//! (`configs.texts`), the game language is resolved once: a registered
//! `-localization=` of the command line, else the saved choice, else the
//! language of the player's Windows user locale, else English. It becomes
//! the engine's language (with the C locale a start with that flag gives) and
//! the saved choice, so the game's save writes it to user.cfg. The Settings'
//! Language list (main_menu) calls `srp_language` (`options.callback`): with a
//! registered name it saves that choice for the next start; with or without,
//! the movie is answered with its texts, the saved and the running language
//! and every registered language by its own name.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::language::{ConsoleString, LangId, PrimaryLanguage, current, set_current, user_locale};
use crate::vostok::settings::{OptionsCall, save_user_config};
use crate::vostok::{Context, Flow, NoExit, LanguageOnOptionsCallExit};

/// A game language: its resource name (`-localization=`,
/// `resources/localization/<name>`), its own name and the Windows primary
/// language whose players start in it.
#[derive(Debug, PartialEq, Eq)]
pub struct Language {
    pub name: &'static CStr,
    pub native: &'static CStr,
    pub windows: PrimaryLanguage,
}

// LANGUAGES, generated from localization_026g.py's registry.
include!("../../../languages_026g.rs");

/// The registered language of this name.
pub fn registered(name: &[u8]) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.name.to_bytes() == name)
}

/// The registered language of a Windows language, by its primary language
/// (any country or script of it).
pub fn of_windows(language: LangId) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.windows == language.primary())
}

/// The game language of a start: a registered `flag` (the command line's),
/// else a registered `saved` choice, else the language of Windows' user
/// locale (asked only then), else English.
pub fn resolve(flag: &[u8], saved: &[u8], system: impl FnOnce() -> LangId) -> &'static Language {
    registered(flag)
        .or_else(|| registered(saved))
        .or_else(|| of_windows(system()))
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
    let language = resolve(flag.as_bytes(), SAVED.get(thread), || user_locale(thread));
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
    fn every_registered_name_fits_the_engine() {
        assert_eq!(LANGUAGES[0].name, c"english");
        for language in &LANGUAGES {
            assert!(!language.name.is_empty() && language.name.count_bytes() < 16, "{language:?}");
        }
    }

    #[test]
    fn a_windows_language_is_its_primary_language_any_country() {
        let of = |id| of_windows(LangId(id)).map(|l| l.name);
        assert_eq!(of(0x0419), Some(c"russian"));
        assert_eq!(of(0x0422), Some(c"ukrainian"));
        assert_eq!(of(0x0423), Some(c"belarusian"));
        assert_eq!(of(0x0405), Some(c"czech"));
        assert_eq!(of(0x0807), Some(c"german"));
        assert_eq!(of(0x0809), Some(c"english"));
        for serbo_croatian in [0x041A, 0x081A, 0x0C1A, 0x101A, 0x141A, 0x241A] {
            assert_eq!(of(serbo_croatian), Some(c"serbian"), "{serbo_croatian:#06x}");
        }
        assert_eq!(of(0x0411), None);
        assert_eq!(of(0x041B), None);
    }

    #[test]
    fn the_flag_wins_then_the_saved_choice_then_windows_then_english() {
        let never = || -> LangId { panic!("Windows is asked only without a flag and a choice") };
        let windows = |id| move || LangId(id);
        assert_eq!(resolve(b"czech", b"german", never).name, c"czech");
        assert_eq!(resolve(b"original", b"german", never).name, c"german");
        assert_eq!(resolve(b"original", b"", windows(0x0419)).name, c"russian");
        assert_eq!(resolve(b"original", b"klingon", windows(0x0405)).name, c"czech");
        assert_eq!(resolve(b"original", b"", windows(0x041A)).name, c"serbian");
        assert_eq!(resolve(b"original", b"", windows(0x0411)).name, c"english");
        assert_eq!(resolve(b"original", b"", windows(0x0C00)).name, c"english");
        assert_eq!(resolve(b"Czech", b"", windows(0x0409)).name, c"english");
    }
}
