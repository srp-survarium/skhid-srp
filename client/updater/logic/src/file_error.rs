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
//!
//! A stop is said in a box of the updater's own with Retry and Cancel:
//! Retry runs the update again.

use alloc::format;
use alloc::string::String;

pub const ERROR_FILE_NOT_FOUND: u32 = 2;
pub const ERROR_PATH_NOT_FOUND: u32 = 3;
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
}

/// The folder Windows Security is to leave alone for a blocked `file` (its
/// name, or its archive name or path): `programs` (`game\binaries\x86`,
/// where the programs it flags are), or `game` (the game folder) for a patch
/// of resources.db, which is kept there. Never the folder above the game,
/// which may be the player's Downloads (`Skhid-network-setup.cmd` excludes
/// `game\binaries\x86` alone too).
pub fn exclusion_folder<'a>(file: &str, programs: &'a str, game: &'a str) -> &'a str {
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
    if name.starts_with(crate::patches::FILE_PREFIX) {
        game
    } else {
        programs
    }
}

/// The words of the updater's box for a stop, in the launcher's language
/// (`crate::replay::LANGUAGES` order; English for an unknown one). `file`:
/// the file's name alone; `folder`: the one to exclude ([`exclusion_folder`]).
/// The box's buttons are Windows' own Retry and Cancel, in Windows' language,
/// which may not be the launcher's: the words do not name them.
pub fn message(language: Option<u32>, stop: Stop, file: &str, folder: &str) -> String {
    let language = language.filter(|&n| n < 5).unwrap_or(0);
    match (language, stop) {
        (1, Stop::Virus) => format!(
            "Безопасность Windows заблокировала {file}: это ложное срабатывание.\n\
             1. Безопасность Windows > Журнал защиты > этот элемент > Разрешить на устройстве (или Восстановить).\n\
             2. Добавьте эту папку в исключения (Защита от вирусов и угроз > Управление настройками > Исключения):\n\
             {folder}\n\
             Затем попробуйте снова. До этого игра не запустится."
        ),
        (2, Stop::Virus) => format!(
            "Безпека Windows заблокувала {file}: це хибне спрацювання.\n\
             1. Безпека Windows > Журнал захисту > цей елемент > Дозволити на пристрої (або Відновити).\n\
             2. Додайте цю теку до винятків (Захист від вірусів і загроз > Керування параметрами > Винятки):\n\
             {folder}\n\
             Потім спробуйте знову. Доти гра не запуститься."
        ),
        (3, Stop::Virus) => format!(
            "Windows-Sicherheit hat {file} blockiert: ein Fehlalarm.\n\
             1. Windows-Sicherheit > Schutzverlauf > dieser Eintrag > Auf Gerät zulassen (oder Wiederherstellen).\n\
             2. Füge diesen Ordner als Ausschluss hinzu (Viren- & Bedrohungsschutz > Einstellungen verwalten > Ausschlüsse):\n\
             {folder}\n\
             Versuche es dann erneut. Bis dahin startet das Spiel nicht."
        ),
        (4, Stop::Virus) => format!(
            "Zabezpieczenia Windows zablokowały {file}: to fałszywy alarm.\n\
             1. Zabezpieczenia Windows > Historia ochrony > ten element > Zezwalaj na urządzeniu (lub Przywróć).\n\
             2. Dodaj ten folder do wykluczeń (Ochrona przed wirusami i zagrożeniami > Zarządzaj ustawieniami > Wykluczenia):\n\
             {folder}\n\
             Następnie spróbuj ponownie. Do tego czasu gra się nie uruchomi."
        ),
        (_, Stop::Virus) => format!(
            "Windows Security blocked {file}: a false positive.\n\
             1. Windows Security > Protection history > this item > Allow on device (or Restore).\n\
             2. Add this folder as an exclusion (Virus & threat protection > Manage settings > Exclusions):\n\
             {folder}\n\
             Then try again. PLAY stays off until then."
        ),
        (1, Stop::InUse) => format!(
            "{file} занят другой программой. Закройте игру, затем попробуйте снова. До этого игра не запустится."
        ),
        (2, Stop::InUse) => format!(
            "{file} зайнятий іншою програмою. Закрийте гру, потім спробуйте знову. Доти гра не запуститься."
        ),
        (3, Stop::InUse) => format!(
            "{file} wird von einem anderen Programm verwendet. Schließe das Spiel und versuche es dann erneut. Bis dahin startet das Spiel nicht."
        ),
        (4, Stop::InUse) => format!(
            "{file} jest używany przez inny program. Zamknij grę, następnie spróbuj ponownie. Do tego czasu gra się nie uruchomi."
        ),
        (_, Stop::InUse) => format!(
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
    fn the_folder_excluded_is_the_programs_or_for_a_patch_the_games() {
        let (programs, game) = ("E:\\S\\game\\binaries\\x86", "E:\\S\\game");
        for file in [
            "survarium_skhid.exe",
            "game/binaries/x86/bugtrap.dll",
            "E:\\S\\game\\binaries\\x86\\launcher_skhid.exe",
        ] {
            assert_eq!(exclusion_folder(file, programs, game), programs, "{file}");
        }
        for file in [
            "resources.db.patch_14",
            "E:\\S\\game\\resources.db.patch_33",
        ] {
            assert_eq!(exclusion_folder(file, programs, game), game, "{file}");
        }
    }

    #[test]
    fn every_language_names_the_file_the_folder_and_the_way_out() {
        let folder = "E:\\Survarium_SkhidSRP_0_26\\game\\binaries\\x86";
        let english = message(None, Stop::Virus, "survarium_skhid.exe", folder);
        assert_eq!(
            english,
            "Windows Security blocked survarium_skhid.exe: a false positive.\n\
             1. Windows Security > Protection history > this item > Allow on device (or Restore).\n\
             2. Add this folder as an exclusion (Virus & threat protection > Manage settings > Exclusions):\n\
             E:\\Survarium_SkhidSRP_0_26\\game\\binaries\\x86\n\
             Then try again. PLAY stays off until then."
        );
        assert_eq!(
            message(Some(9), Stop::Virus, "survarium_skhid.exe", folder),
            english
        );
        for stop in [Stop::Virus, Stop::InUse] {
            let english = message(Some(0), stop, "survarium_skhid.exe", folder);
            for language in 1..5 {
                let text = message(Some(language), stop, "survarium_skhid.exe", folder);
                assert_ne!(text, english, "{language} {stop:?}");
                assert!(
                    text.starts_with("survarium_skhid.exe")
                        || text.contains(" survarium_skhid.exe"),
                    "{text}"
                );
                assert_eq!(text.contains(folder), stop == Stop::Virus, "{text}");
                // Two steps, one line each, for Windows Security.
                assert_eq!(
                    text.contains("\n1. ") && text.contains("\n2. "),
                    stop == Stop::Virus,
                    "{text}"
                );
                assert_eq!(text.lines().count(), english.lines().count(), "{text}");
            }
        }
    }
}
