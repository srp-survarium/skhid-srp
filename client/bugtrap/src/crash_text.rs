// Crash-dialog copy, independent of Win32 and allocation.
use crate::{details::Failure, notice_text::Language};

pub struct Text {
    pub title: &'static str,
    pub intro: &'static str,
    pub saved: &'static str,
    pub partial: &'static str,
    pub failed: &'static str,
    /// The dump file was made, but its capture was refused (`Failure::Capture`).
    pub blocked: &'static str,
    /// A full-memory dump (`SKHID_CRASH_DUMP=full`) kept beside the report:
    /// the status line before the dump's name, and the instructions, which
    /// name the report to send and the dump to keep.
    pub full_saved: &'static str,
    pub full_instructions: &'static str,
    pub error: &'static str,
    pub file: &'static str,
    pub folder: &'static str,
    pub instructions: &'static str,
    pub no_file: &'static str,
    pub discord: &'static str,
    pub close: &'static str,
    /// The button that copies the crash details for Discord, and its label
    /// once it did.
    pub copy: &'static str,
    pub copied: &'static str,
    /// The game's folder has non-ASCII letters: the text before and after
    /// that folder.
    pub simple_path: [&'static str; 2],
    pub browser_failed: &'static str,
    pub manual: &'static str,
}

/// The dialog's first line: what became of the dump.
pub fn status(text: &Text, saved: bool, partial: bool, failure: Failure) -> &'static str {
    if saved {
        text.saved
    } else if partial {
        text.partial
    } else if failure == Failure::Capture {
        text.blocked
    } else {
        text.failed
    }
}

pub fn text(language: Language) -> &'static Text {
    match language {
        Language::English => &ENGLISH,
        Language::Russian => &RUSSIAN,
        Language::Ukrainian => &UKRAINIAN,
        Language::German => &GERMAN,
        Language::Polish => &POLISH,
    }
}

const ENGLISH: Text = Text {
    title: "Survarium — crash report",
    intro: "The game has crashed.",
    saved: "Crash dump saved:",
    partial: "A crash report was saved, but could not be fully verified.",
    failed: "Could not save a crash dump beside the game or in the temporary folder.",
    blocked: "An antivirus or Windows security feature blocked the crash dump. The crash details below are still useful: copy them to #bugreports.",
    full_saved: "Crash report saved. The full-memory dump is kept beside it, for the developer:",
    error: "Windows error",
    file: "File:",
    folder: "Folder:",
    instructions: "Send this file (in the folder shown above) to #bugreports in Discord, and paste the details copied with “Copy” there too.\r\nClick “Open Discord” below to open our server. Nothing is uploaded automatically.",
    full_instructions: "Click “Open Discord” below, open a new post in the #bugreports forum, attach this crash report (in the folder shown above) and paste the details copied with “Copy”.\r\nDo not post the .dmp: it is too big for Discord and its memory can hold your password. Keep it for the developer.",
    no_file: "Click “Copy”, then “Open Discord” below, paste the details in #bugreports and say what happened. No file was saved.",
    discord: "Open Discord",
    close: "Close",
    copy: "Copy",
    copied: "Copied",
    simple_path: [
        "The game is in a folder with non-English letters (",
        "). Move it to a simple path like C:\\Games\\Survarium and start it from there.",
    ],
    browser_failed: "Could not open your browser.",
    manual: "Open discord.gg/QUmJg3gBhg yourself and send the file, if saved, to #bugreports. Describe what happened.",
};
const RUSSIAN: Text = Text {
    title: "Survarium — отчёт о сбое",
    intro: "В игре произошёл сбой.",
    saved: "Отчёт о сбое сохранён:",
    partial: "Отчёт о сбое сохранён, но его не удалось полностью проверить.",
    failed: "Не удалось сохранить отчёт о сбое ни в папке игры, ни во временной папке.",
    blocked: "Антивирус или защита Windows заблокировали запись дампа сбоя. Сведения о сбое ниже всё равно полезны: скопируйте их в #bugreports.",
    full_saved: "Отчёт о сбое сохранён. Полный дамп памяти оставлен рядом, для разработчика:",
    error: "Ошибка Windows",
    file: "Файл:",
    folder: "Папка:",
    instructions: "Отправьте этот файл (он в папке выше) в канал #bugreports в Discord и вставьте туда сведения, скопированные кнопкой «Копировать».\r\nНажмите «Открыть Discord» внизу, чтобы перейти на наш сервер. Файл не отправляется автоматически.",
    full_instructions: "Нажмите «Открыть Discord» внизу, создайте новую тему в форуме #bugreports, приложите этот отчёт (он в папке выше) и вставьте сведения, скопированные кнопкой «Копировать».\r\nНе выкладывайте файл .dmp: он слишком большой для Discord, а в его памяти может быть ваш пароль. Сохраните его для разработчика.",
    no_file: "Нажмите «Копировать», затем «Открыть Discord» внизу, вставьте сведения в канал #bugreports и опишите, что произошло. Файл не сохранён.",
    discord: "Открыть Discord",
    close: "Закрыть",
    copy: "Копировать",
    copied: "Скопировано",
    simple_path: [
        "Игра находится в папке с нелатинскими буквами (",
        "). Перенесите её по простому пути, например C:\\Games\\Survarium, и запускайте оттуда.",
    ],
    browser_failed: "Не удалось открыть браузер.",
    manual: "Откройте discord.gg/QUmJg3gBhg вручную и отправьте файл, если он сохранён, в канал #bugreports. Опишите, что произошло.",
};
const UKRAINIAN: Text = Text {
    title: "Survarium — звіт про збій",
    intro: "У грі стався збій.",
    saved: "Звіт про збій збережено:",
    partial: "Звіт про збій збережено, але його не вдалося повністю перевірити.",
    failed: "Не вдалося зберегти звіт про збій ані в папці гри, ані в тимчасовій папці.",
    blocked: "Антивірус або захист Windows заблокували запис дампа збою. Відомості про збій нижче все одно корисні: скопіюйте їх у #bugreports.",
    full_saved: "Звіт про збій збережено. Повний дамп пам'яті залишено поруч, для розробника:",
    error: "Помилка Windows",
    file: "Файл:",
    folder: "Папка:",
    instructions: "Надішліть цей файл (він у папці вище) у канал #bugreports у Discord і вставте туди відомості, скопійовані кнопкою «Копіювати».\r\nНатисніть «Відкрити Discord» внизу, щоб перейти на наш сервер. Файл не надсилається автоматично.",
    full_instructions: "Натисніть «Відкрити Discord» внизу, створіть нову тему у форумі #bugreports, додайте цей звіт (він у папці вище) і вставте відомості, скопійовані кнопкою «Копіювати».\r\nНе викладайте файл .dmp: він завеликий для Discord, а в його пам'яті може бути ваш пароль. Збережіть його для розробника.",
    no_file: "Натисніть «Копіювати», потім «Відкрити Discord» внизу, вставте відомості в канал #bugreports та опишіть, що сталося. Файл не збережено.",
    discord: "Відкрити Discord",
    close: "Закрити",
    copy: "Копіювати",
    copied: "Скопійовано",
    simple_path: [
        "Гра в папці з нелатинськими літерами (",
        "). Перенесіть її за простим шляхом, наприклад C:\\Games\\Survarium, і запускайте звідти.",
    ],
    browser_failed: "Не вдалося відкрити браузер.",
    manual: "Відкрийте discord.gg/QUmJg3gBhg вручну та надішліть файл, якщо його збережено, у канал #bugreports. Опишіть, що сталося.",
};
const GERMAN: Text = Text {
    title: "Survarium — Absturzbericht",
    intro: "Das Spiel ist abgestürzt.",
    saved: "Absturzbericht gespeichert:",
    partial: "Ein Absturzbericht wurde gespeichert, konnte aber nicht vollständig geprüft werden.",
    failed: "Der Absturzbericht konnte weder im Spielordner noch im temporären Ordner gespeichert werden.",
    blocked: "Ein Virenschutz oder eine Windows-Sicherheitsfunktion hat den Absturz-Dump blockiert. Die Absturzdetails unten helfen trotzdem: kopiere sie nach #bugreports.",
    full_saved: "Absturzbericht gespeichert. Der vollständige Speicher-Dump bleibt daneben, für die Entwickler:",
    error: "Windows-Fehler",
    file: "Datei:",
    folder: "Ordner:",
    instructions: "Sende diese Datei (im Ordner oben) an #bugreports in Discord und füge dort auch die mit „Kopieren“ kopierten Details ein.\r\nKlicke unten auf „Discord öffnen“, um unseren Server zu öffnen. Es wird nichts automatisch hochgeladen.",
    full_instructions: "Klicke unten auf „Discord öffnen“, erstelle einen neuen Beitrag im Forum #bugreports, hänge diesen Absturzbericht (im Ordner oben) an und füge die mit „Kopieren“ kopierten Details ein.\r\nPoste die .dmp-Datei nicht: Sie ist zu groß für Discord, und ihr Speicher kann dein Passwort enthalten. Behalte sie für die Entwickler.",
    no_file: "Klicke auf „Kopieren“, dann unten auf „Discord öffnen“, füge die Details in #bugreports ein und beschreibe, was passiert ist. Es wurde keine Datei gespeichert.",
    discord: "Discord öffnen",
    close: "Schließen",
    copy: "Kopieren",
    copied: "Kopiert",
    simple_path: [
        "Das Spiel liegt in einem Ordner mit nicht-englischen Buchstaben (",
        "). Verschiebe es in einen einfachen Pfad wie C:\\Games\\Survarium und starte es von dort.",
    ],
    browser_failed: "Der Browser konnte nicht geöffnet werden.",
    manual: "Öffne discord.gg/QUmJg3gBhg selbst und sende die Datei, falls gespeichert, an #bugreports. Beschreibe, was passiert ist.",
};
const POLISH: Text = Text {
    title: "Survarium — raport o awarii",
    intro: "Gra uległa awarii.",
    saved: "Raport o awarii został zapisany:",
    partial: "Raport o awarii został zapisany, ale nie udało się go w pełni zweryfikować.",
    failed: "Nie udało się zapisać raportu o awarii ani w folderze gry, ani w folderze tymczasowym.",
    blocked: "Program antywirusowy lub zabezpieczenia systemu Windows zablokowały zrzut awarii. Poniższe szczegóły awarii nadal są przydatne: skopiuj je na #bugreports.",
    full_saved: "Raport o awarii został zapisany. Pełny zrzut pamięci zostaje obok, dla programisty:",
    error: "Błąd systemu Windows",
    file: "Plik:",
    folder: "Folder:",
    instructions: "Wyślij ten plik (z folderu powyżej) na kanał #bugreports na Discordzie i wklej tam też szczegóły skopiowane przyciskiem „Kopiuj”.\r\nKliknij „Otwórz Discord” poniżej, aby przejść na nasz serwer. Plik nie jest wysyłany automatycznie.",
    full_instructions: "Kliknij „Otwórz Discord” poniżej, utwórz nowy wpis na forum #bugreports, dołącz ten raport (z folderu powyżej) i wklej szczegóły skopiowane przyciskiem „Kopiuj”.\r\nNie publikuj pliku .dmp: jest za duży na Discorda, a jego pamięć może zawierać Twoje hasło. Zachowaj go dla programisty.",
    no_file: "Kliknij „Kopiuj”, a potem „Otwórz Discord” poniżej, wklej szczegóły na kanale #bugreports i opisz, co się stało. Żaden plik nie został zapisany.",
    discord: "Otwórz Discord",
    close: "Zamknij",
    copy: "Kopiuj",
    copied: "Skopiowano",
    simple_path: [
        "Gra jest w folderze z nieangielskimi literami (",
        "). Przenieś ją do prostej ścieżki, np. C:\\Games\\Survarium, i uruchamiaj stamtąd.",
    ],
    browser_failed: "Nie udało się otworzyć przeglądarki.",
    manual: "Otwórz samodzielnie discord.gg/QUmJg3gBhg i wyślij plik, jeśli został zapisany, na kanał #bugreports. Opisz, co się stało.",
};
