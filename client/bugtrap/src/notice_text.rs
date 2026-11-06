// The start-up notice "the game cannot save its files": its texts, and the
// two decisions that need no Windows call. Tested on any host:
//   rustc --edition 2024 --test src/notice_text.rs -o target/notice-text-test
#![allow(dead_code)]

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Language {
    English,
    Russian,
    Ukrainian,
    German,
    Polish,
}

/// The message is `before_folder`, the folder, `before_exe`, the game exe,
/// `after_exe`, and a last line `(<error> <Windows error code>)`.
pub struct Notice {
    pub title: &'static str,
    pub before_folder: &'static str,
    pub before_exe: &'static str,
    pub after_exe: &'static str,
    pub error: &'static str,
}

// The names in quotation marks are the labels of the Windows Security window.
// The English ones are Windows 11's. The others are that window's labels in
// the language as far as known here: correct them from a Windows in that
// language when one differs.
const ENGLISH: Notice = Notice {
    title: "Survarium cannot save its files",
    before_folder: "Windows is blocking the game from saving files to this folder:\r\n",
    before_exe: "\r\n\r\nYou can still play, but your settings (graphics, sound, controls) will not be kept for next time.\r\n\r\nTo fix it:\r\n1. Open the Start menu, type \"Ransomware protection\" and open it.\r\n2. Click \"Allow an app through Controlled folder access\", then \"Add an allowed app\", and choose:\r\n",
    after_exe: "\r\n3. Start the game again.\r\n\r\nIf this does not help, ask us in Discord: discord.gg/QUmJg3gBhg",
    error: "Windows error",
};
const RUSSIAN: Notice = Notice {
    title: "Survarium не может сохранять свои файлы",
    before_folder: "Windows не даёт игре сохранять файлы в эту папку:\r\n",
    before_exe: "\r\n\r\nИграть можно, но настройки (графика, звук, управление) не сохранятся до следующего запуска.\r\n\r\nКак исправить:\r\n1. Откройте меню «Пуск», введите «Защита от программ-шантажистов» и откройте этот раздел.\r\n2. Нажмите «Разрешить работу приложения через контролируемый доступ к папкам», затем «Добавление разрешенного приложения» и выберите:\r\n",
    after_exe: "\r\n3. Запустите игру снова.\r\n\r\nЕсли это не помогло, напишите нам в Discord: discord.gg/QUmJg3gBhg",
    error: "Ошибка Windows",
};
const UKRAINIAN: Notice = Notice {
    title: "Survarium не може зберігати свої файли",
    before_folder: "Windows не дає грі зберігати файли в цю папку:\r\n",
    before_exe: "\r\n\r\nГрати можна, але налаштування (графіка, звук, керування) не збережуться до наступного запуску.\r\n\r\nЯк виправити:\r\n1. Відкрийте меню «Пуск», введіть «Захист від зловмисних програм з вимогою викупу» і відкрийте цей розділ.\r\n2. Натисніть «Дозволити програмі доступ через контрольований доступ до папок», потім «Додати дозволену програму» і виберіть:\r\n",
    after_exe: "\r\n3. Запустіть гру знову.\r\n\r\nЯкщо це не допомогло, напишіть нам у Discord: discord.gg/QUmJg3gBhg",
    error: "Помилка Windows",
};
const GERMAN: Notice = Notice {
    title: "Survarium kann seine Dateien nicht speichern",
    before_folder: "Windows verhindert, dass das Spiel Dateien in diesem Ordner speichert:\r\n",
    before_exe: "\r\n\r\nDu kannst trotzdem spielen, aber deine Einstellungen (Grafik, Sound, Steuerung) bleiben nicht bis zum nächsten Start erhalten.\r\n\r\nSo behebst du es:\r\n1. Öffne das Startmenü, gib „Ransomware-Schutz“ ein und öffne ihn.\r\n2. Klicke auf „App durch überwachten Ordnerzugriff zulassen“, dann auf „Zulässige App hinzufügen“ und wähle:\r\n",
    after_exe: "\r\n3. Starte das Spiel erneut.\r\n\r\nWenn das nicht hilft, frag uns auf Discord: discord.gg/QUmJg3gBhg",
    error: "Windows-Fehler",
};
const POLISH: Notice = Notice {
    title: "Survarium nie może zapisać swoich plików",
    before_folder: "System Windows blokuje grze zapisywanie plików w tym folderze:\r\n",
    before_exe: "\r\n\r\nMożesz grać, ale ustawienia (grafika, dźwięk, sterowanie) nie zostaną zachowane do następnego uruchomienia.\r\n\r\nJak to naprawić:\r\n1. Otwórz menu Start, wpisz „Ochrona przed oprogramowaniem wymuszającym okup” i otwórz ją.\r\n2. Kliknij „Zezwalaj aplikacji na dostęp przez funkcję Kontrolowany dostęp do folderu”, potem „Dodaj dozwoloną aplikację” i wybierz:\r\n",
    after_exe: "\r\n3. Uruchom grę ponownie.\r\n\r\nJeśli to nie pomoże, napisz do nas na Discordzie: discord.gg/QUmJg3gBhg",
    error: "Błąd systemu Windows",
};

pub fn notice(language: Language) -> &'static Notice {
    match language {
        Language::English => &ENGLISH,
        Language::Russian => &RUSSIAN,
        Language::Ukrainian => &UKRAINIAN,
        Language::German => &GERMAN,
        Language::Polish => &POLISH,
    }
}

/// Automatically use the Windows user's UI language for every BugTrap dialog.
/// Ignore regional variants; unsupported languages fall back to English.
pub fn language(windows_language: u16) -> Language {
    match windows_language & 0x03ff {
        0x19 => Language::Russian,
        0x22 => Language::Ukrainian,
        0x07 => Language::German,
        0x15 => Language::Polish,
        _ => Language::English,
    }
}

/// The launcher, the updater and the service load this DLL too, from the same
/// folder. The notice is the game's alone: Windows judges each program by
/// itself, and the text names the program to allow.
pub fn is_game(exe_path: &[u16]) -> bool {
    let name = exe_path
        .iter()
        .rposition(|&c| c == b'\\' as u16 || c == b'/' as u16)
        .map_or(exe_path, |i| &exe_path[i + 1..]);
    let contains = |word: &[u8]| {
        name.windows(word.len()).any(|part| {
            part.iter()
                .zip(word)
                .all(|(&c, &w)| c < 128 && (c as u8).to_ascii_lowercase() == w)
        })
    };
    !name.is_empty() && !contains(b"launcher") && !contains(b"updater") && !contains(b"service")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn automatically_uses_the_windows_ui_language() {
        for (windows, expected) in [
            (0x0419, Language::Russian),
            (0x0415, Language::Polish),
            (0x0422, Language::Ukrainian),
            (0x0407, Language::German),
            (0x0807, Language::German),
            (0x0409, Language::English),
            (0x0809, Language::English),
            (0x040c, Language::English),
            (0, Language::English),
        ] {
            assert_eq!(language(windows), expected, "{windows:04x}");
        }
    }

    #[test]
    fn only_the_game_is_told() {
        for path in [
            r"C:\Games\Survarium\game\binaries\x86\survarium_skhid.exe",
            r"D:\x\survarium.exe",
            "survarium_srp.exe",
            "Z:/home/user/game/binaries/x86/survarium_skhid.exe",
        ] {
            assert!(is_game(&wide(path)), "{path}");
        }
        for path in [
            r"C:\Games\Survarium\game\binaries\x86\launcher_skhid.exe",
            r"C:\Games\Survarium\game\binaries\x86\survarium_launcher.exe",
            r"C:\Games\Survarium\game\binaries\x86\Survarium_Updater.exe",
            r"C:\Games\Survarium\game\binaries\x86\updater_skhid.exe",
            r"C:\Games\Survarium\game\binaries\x86\survarium_service.exe",
            r"C:\launcher\",
            "",
        ] {
            assert!(!is_game(&wide(path)), "{path}");
        }
        // A folder's name does not count, only the file's.
        assert!(is_game(&wide(r"C:\launcher\survarium_skhid.exe")));
    }

    #[test]
    fn every_language_has_the_whole_message() {
        for language in [
            Language::English,
            Language::Russian,
            Language::Ukrainian,
            Language::German,
            Language::Polish,
        ] {
            let text = notice(language);
            assert!(!text.title.is_empty() && !text.error.is_empty());
            // The folder and the exe stand on lines of their own.
            assert!(text.before_folder.ends_with(":\r\n"), "{language:?}");
            assert!(text.before_exe.starts_with("\r\n\r\n") && text.before_exe.ends_with(":\r\n"));
            assert!(text.after_exe.starts_with("\r\n3. "), "{language:?}");
            assert!(text.before_exe.contains("\r\n1. ") && text.before_exe.contains("\r\n2. "));
            assert!(
                text.after_exe.ends_with("discord.gg/QUmJg3gBhg"),
                "{language:?}"
            );
            // It fits the buffer of notice.rs with two long paths.
            let units = |s: &str| s.encode_utf16().count();
            assert!(
                units(text.before_folder)
                    + units(text.before_exe)
                    + units(text.after_exe)
                    + 2 * 1024
                    + 64
                    < 4096,
                "{language:?}"
            );
            for part in [text.before_folder, text.before_exe, text.after_exe] {
                assert!(
                    !part.contains('\n')
                        || part.matches('\n').count() == part.matches("\r\n").count()
                );
            }
        }
    }
}
