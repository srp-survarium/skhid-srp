//! The free space the game needs on the drive of the player's Documents
//! folder, where patch 70 keeps up to 3 GB of baked textures: below it PLAY
//! stays off and the updater's box says so.

use alloc::format;
use alloc::string::String;

/// The free bytes the game needs on the Documents drive.
pub const NEEDED: u64 = 5 << 30;

/// The drive of a folder as a player knows it: `C:` for `C:\Users\...`,
/// else the folder itself (a network share).
pub fn drive(folder: &str) -> &str {
    match folder.as_bytes() {
        [letter, b':', ..] if letter.is_ascii_alphabetic() => &folder[..2],
        _ => folder,
    }
}

/// The box's words in the game's language (`crate::replay::LANGUAGES`
/// order; English for an unknown one), for the drive of `folder`. The
/// buttons are Windows' own Retry and Cancel: the words do not name them.
pub fn too_little(language: Option<u32>, folder: &str) -> String {
    let (drive, gigabytes) = (drive(folder), NEEDED >> 30);
    match language.filter(|&n| n < 5).unwrap_or(0) {
        1 => format!(
            "На диске {drive} нужно не менее {gigabytes} ГБ свободного места для плавной работы игры. До этого игра не запустится."
        ),
        2 => format!(
            "На диску {drive} потрібно щонайменше {gigabytes} ГБ вільного місця для плавної роботи гри. Доти гра не запуститься."
        ),
        3 => format!(
            "Das Spiel braucht mindestens {gigabytes} GB freien Speicherplatz auf Laufwerk {drive}, um flüssig zu laufen. Bis dahin startet das Spiel nicht."
        ),
        4 => format!(
            "Gra wymaga co najmniej {gigabytes} GB wolnego miejsca na dysku {drive}, aby działać płynnie. Do tego czasu gra się nie uruchomi."
        ),
        _ => format!(
            "The game requires at least {gigabytes} GB of free disk space on drive {drive} to run smoothly. PLAY stays off until then."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drive_is_named_by_its_letter() {
        assert_eq!(drive("C:\\Users\\player\\Documents"), "C:");
        assert_eq!(drive("d:\\Documents"), "d:");
        assert_eq!(drive("\\\\server\\share\\Documents"), "\\\\server\\share\\Documents");
        assert_eq!(drive(""), "");
    }

    #[test]
    fn every_language_says_the_gigabytes_and_the_drive() {
        for language in [None, Some(0), Some(1), Some(2), Some(3), Some(4), Some(9)] {
            let text = too_little(language, "E:\\Documents");
            assert!(text.contains('5') && text.contains("E:"), "{text}");
        }
        assert!(too_little(Some(9), "C:\\x").starts_with("The game requires at least 5 GB"));
    }
}
