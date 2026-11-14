//! A Skhid file of the game that Windows does not let the updater read,
//! write, rename or delete: what its error code means for the update, and
//! the words for the player when the update stops on it.
//!
//! - Gone (`ERROR_FILE_NOT_FOUND`, `ERROR_PATH_NOT_FOUND`): it is downloaded.
//! - Blocked by Windows Security (`ERROR_VIRUS_INFECTED`,
//!   `ERROR_VIRUS_DELETED`): the update stops, PLAY stays off, and the
//!   player is told how to let the file through ([`Stop::Virus`]). Writing
//!   it again would be blocked again (2026-10-21: a player's
//!   survarium_skhid.exe of revision 27, error 225 at its write, then at its
//!   read; the updater said "up to date" and the launcher crashed at PLAY).
//! - Held by another program that lets nobody read it
//!   (`ERROR_SHARING_VIOLATION`, `ERROR_LOCK_VIOLATION`), still after a few
//!   tries: the update stops, PLAY stays off, and the player is told to close
//!   it ([`Stop::InUse`]). A download would not get past it either.
//! - Anything else (`ERROR_ACCESS_DENIED`, a disk error): the file is
//!   repaired, the archive installed again; when that fails, the install's
//!   error names the file and Windows' words.
//! - A file the updater has just written that is gone or denied to it when
//!   read back ([`Stop::after_write`]): an antivirus took it as it was
//!   written, the same stop as Windows Security's.
//!
//! A stop is said in a box of the updater's own with Retry and Cancel:
//! Retry runs the update again. Windows Security's is the box of
//! `crate::problem` (`Problem::Antivirus`); a held file's is [`in_use`].

use alloc::format;
use alloc::string::String;

pub const ERROR_FILE_NOT_FOUND: u32 = 2;
pub const ERROR_PATH_NOT_FOUND: u32 = 3;
pub const ERROR_ACCESS_DENIED: u32 = 5;
pub const ERROR_SHARING_VIOLATION: u32 = 32;
pub const ERROR_LOCK_VIOLATION: u32 = 33;
/// "Operation did not complete successfully because the file contains a
/// virus or potentially unwanted software."
pub const ERROR_VIRUS_INFECTED: u32 = 225;
/// "This file contains a virus or potentially unwanted software and cannot
/// be opened. Due to the nature of this virus or potentially unwanted
/// software, the file has been removed from this location."
pub const ERROR_VIRUS_DELETED: u32 = 226;

/// What an error code says of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Missing,
    /// Held by another program: worth a few more tries.
    InUse,
    Virus,
    Other,
}

pub fn kind(code: u32) -> Kind {
    match code {
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => Kind::Missing,
        ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION => Kind::InUse,
        ERROR_VIRUS_INFECTED | ERROR_VIRUS_DELETED => Kind::Virus,
        _ => Kind::Other,
    }
}

/// Why the update stops with PLAY off until the player has done something;
/// then it runs again (Retry).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// Windows Security blocked the file (a false positive).
    Virus,
    /// Another program holds the file.
    InUse,
}

impl Stop {
    /// The stop an error of a read, a write, a rename or a delete is; `None`
    /// for one that is not.
    pub fn of(code: u32) -> Option<Self> {
        match kind(code) {
            Kind::Virus => Some(Self::Virus),
            Kind::InUse => Some(Self::InUse),
            Kind::Missing | Kind::Other => None,
        }
    }

    /// The stop an error of reading back a file the updater has just
    /// written is: gone, or access denied, is an antivirus that acted on
    /// it as it was written, as Windows Security's own codes are.
    pub fn after_write(code: u32) -> Option<Self> {
        match kind(code) {
            Kind::Missing | Kind::Virus => Some(Self::Virus),
            Kind::Other if code == ERROR_ACCESS_DENIED => Some(Self::Virus),
            Kind::InUse => Some(Self::InUse),
            Kind::Other => None,
        }
    }
}

/// The folder Windows Security is to leave alone for a blocked `file` (its
/// name, or its archive name or path): `programs` (`game\binaries\x86`,
/// where the programs it flags are), or `game` (the game folder) for a patch
/// of resources.db, which is kept there. Never the folder above the game,
/// which may be the player's Downloads (`Skhid-defender-setup.cmd` excludes
/// `game\binaries\x86` alone too).
pub fn exclusion_folder<'a>(file: &str, programs: &'a str, game: &'a str) -> &'a str {
    match Exclusion::of(file) {
        Exclusion::Programs => programs,
        Exclusion::Game => game,
    }
}

/// Which folder Windows Security is to leave alone for a blocked file:
/// `game\binaries\x86`, which `Skhid-defender-setup.cmd` excludes, or the
/// game folder (a patch of resources.db), which it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exclusion {
    Programs,
    Game,
}

impl Exclusion {
    /// The folder of `file` (its name, or its archive name or path).
    pub fn of(file: &str) -> Self {
        let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
        if name.starts_with(crate::patches::FILE_PREFIX) {
            Self::Game
        } else {
            Self::Programs
        }
    }
}

/// The words of the updater's box for a file another program holds, in the
/// launcher's language (`crate::replay::LANGUAGES` order; English for an
/// unknown one). `file`: the file's name alone. A file Windows Security
/// blocked is told in the box of `crate::problem` (`Problem::Antivirus`).
/// The box's buttons are Windows' own Retry and Cancel, in Windows'
/// language, which may not be the launcher's: the words do not name them.
pub fn in_use(language: Option<u32>, file: &str) -> String {
    match language.filter(|&n| n < 5).unwrap_or(0) {
        1 => format!(
            "{file} занят другой программой. Закройте игру, затем попробуйте снова. До этого игра не запустится."
        ),
        2 => format!(
            "{file} зайнятий іншою програмою. Закрийте гру, потім спробуйте знову. Доти гра не запуститься."
        ),
        3 => format!(
            "{file} wird von einem anderen Programm verwendet. Schließe das Spiel und versuche es dann erneut. Bis dahin startet das Spiel nicht."
        ),
        4 => format!(
            "{file} jest używany przez inny program. Zamknij grę, następnie spróbuj ponownie. Do tego czasu gra się nie uruchomi."
        ),
        _ => format!(
            "{file} is in use by another program. Close the game, then try again. PLAY stays off until then."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_sorted_into_what_the_update_does() {
        assert_eq!(kind(2), Kind::Missing);
        assert_eq!(kind(3), Kind::Missing);
        assert_eq!(kind(32), Kind::InUse);
        assert_eq!(kind(33), Kind::InUse);
        assert_eq!(kind(225), Kind::Virus);
        assert_eq!(kind(226), Kind::Virus);
        // Access denied, a disk error, anything else: a repair.
        for code in [5, 19, 21, 23, 1117, 0] {
            assert_eq!(kind(code), Kind::Other, "{code}");
            assert_eq!(Stop::of(code), None, "{code}");
        }
        assert_eq!(Stop::of(225), Some(Stop::Virus));
        assert_eq!(Stop::of(226), Some(Stop::Virus));
        assert_eq!(Stop::of(32), Some(Stop::InUse));
        assert_eq!(Stop::of(2), None);
    }

    #[test]
    fn a_file_just_written_and_gone_or_denied_is_an_antivirus() {
        for code in [2, 3, 5, 225, 226] {
            assert_eq!(Stop::after_write(code), Some(Stop::Virus), "{code}");
        }
        assert_eq!(Stop::after_write(32), Some(Stop::InUse));
        for code in [19, 23, 1117, 0] {
            assert_eq!(Stop::after_write(code), None, "{code}");
        }
    }

    #[test]
    fn the_folder_excluded_is_the_programs_or_for_a_patch_the_games() {
        let (programs, game) = ("E:\\S\\game\\binaries\\x86", "E:\\S\\game");
        for file in [
            "survarium_skhid.exe",
            "game/binaries/x86/bugtrap.dll",
            "E:\\S\\game\\binaries\\x86\\launcher_skhid.exe",
        ] {
            assert_eq!(exclusion_folder(file, programs, game), programs, "{file}");
            assert_eq!(Exclusion::of(file), Exclusion::Programs, "{file}");
        }
        for file in [
            "resources.db.patch_14",
            "E:\\S\\game\\resources.db.patch_33",
        ] {
            assert_eq!(exclusion_folder(file, programs, game), game, "{file}");
            assert_eq!(Exclusion::of(file), Exclusion::Game, "{file}");
        }
    }

    #[test]
    fn every_language_names_the_file_held() {
        let english = in_use(None, "survarium_skhid.exe");
        assert_eq!(
            english,
            "survarium_skhid.exe is in use by another program. Close the game, then try again. \
             PLAY stays off until then."
        );
        assert_eq!(in_use(Some(9), "survarium_skhid.exe"), english);
        for language in 1..5 {
            let text = in_use(Some(language), "survarium_skhid.exe");
            assert_ne!(text, english, "{language}");
            assert!(text.starts_with("survarium_skhid.exe "), "{text}");
            assert_eq!(text.lines().count(), 1, "{text}");
        }
    }
}
