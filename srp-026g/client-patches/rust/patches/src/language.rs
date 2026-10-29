//! Patch 79: the game language is a setting (docs/notes/language-option.md).
//!
//! user.cfg keeps the choice as the console variable `localization`, which
//! this patch registers before game::on_configs_loaded runs the file
//! (`configs.start`). After the file, before the dictionary loads
//! (`configs.texts`), the game language is resolved once: a registered
//! `-localization=` of the command line, else the saved choice, else a
//! language picked in the launcher's menu (its registry value), else the
//! language of the player's Windows user locale, else English. It becomes
//! the engine's language (with the C locale a start with that flag gives) and
//! the saved choice, so the game's save writes it to user.cfg. The Settings'
//! Language list (main_menu) calls `srp_language` (`options.callback`): with a
//! registered name it saves that choice for the next start; with or without,
//! the movie is answered with its texts, the saved and the running language
//! and every registered language by its own name.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::language::{ConsoleString, LangId, OptionsCall, PrimaryLanguage, current, launcher_language,
                              save_user_config, set_current, user_locale};
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

/// The launcher's menu, by its index (launcher.md section 10.1), as a
/// player's pick. Index 0, English, is none: the launcher's first start
/// writes it when nothing was chosen (section 10.2; Skhid starts it without
/// `-language=`), so a new player's registry says English before he chose.
const LAUNCHER: [Option<&CStr>; 5] = [None, Some(c"russian"), Some(c"ukrainian"), Some(c"german"), Some(c"polish")];

/// The registered language of this name.
pub fn registered(name: &[u8]) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.name.to_bytes() == name)
}

/// The registered language a launcher's `language` value picked.
fn launcher_pick(index: u32) -> Option<&'static Language> {
    LAUNCHER.get(index as usize).copied().flatten().and_then(|name| registered(name.to_bytes()))
}

/// The registered language of a Windows language, by its primary language
/// (any country or script of it).
pub fn of_windows(language: LangId) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.windows == language.primary())
}

/// The game language of a start: a registered `flag` (the command line's),
/// else a registered `saved` choice, else a language picked in the
/// launcher's menu, else the language of Windows' user locale (each asked
/// only when the ones before give none), else English.
pub fn resolve(flag: &[u8], saved: &[u8], launcher: impl FnOnce() -> Option<u32>,
               system: impl FnOnce() -> LangId) -> &'static Language {
    registered(flag)
        .or_else(|| registered(saved))
        .or_else(|| launcher().and_then(launcher_pick))
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
    let language = resolve(flag.as_bytes(), SAVED.get(thread), || launcher_language(thread),
                           || user_locale(thread));
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
        for name in LAUNCHER.into_iter().flatten() {
            assert!(registered(name.to_bytes()).is_some(), "{name:?}");
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
    fn the_flag_wins_then_the_saved_choice_then_the_launcher_then_windows_then_english() {
        let never = || -> Option<u32> { panic!("the launcher is asked only without a flag and a choice") };
        let no_windows = || -> LangId { panic!("Windows is asked only without a launcher's pick") };
        let windows = |id| move || LangId(id);
        assert_eq!(resolve(b"czech", b"german", never, no_windows).name, c"czech");
        assert_eq!(resolve(b"original", b"german", never, no_windows).name, c"german");
        assert_eq!(resolve(b"original", b"", || Some(2), no_windows).name, c"ukrainian");
        assert_eq!(resolve(b"original", b"klingon", || Some(4), no_windows).name, c"polish");
        // the launcher's English is what its first start writes: Windows decides
        assert_eq!(resolve(b"original", b"", || Some(0), windows(0x0419)).name, c"russian");
        assert_eq!(resolve(b"original", b"", || Some(5), windows(0x0405)).name, c"czech");
        assert_eq!(resolve(b"original", b"", || None, windows(0x041A)).name, c"serbian");
        assert_eq!(resolve(b"original", b"", || None, windows(0x0411)).name, c"english");
        assert_eq!(resolve(b"original", b"", || None, windows(0x0C00)).name, c"english");
        assert_eq!(resolve(b"Czech", b"", || None, windows(0x0409)).name, c"english");
    }
}
