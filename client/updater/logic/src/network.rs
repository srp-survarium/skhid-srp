//! The update server could not be reached: which of the usual ways it
//! failed, by WinHTTP's error code, and the words for the player in the
//! launcher's language. Players in Russia meet this most: their provider
//! blocks the server, and a VPN with the game's three programs in its split
//! tunnelling gets them through (`VPN.txt` and `Skhid-network-setup.cmd` of
//! the players' archive).
//!
//! It is said in a box of the updater's own with Retry and Cancel, as a stop
//! of `file_error` is: the launcher shows the updater's words only in ASCII.

use alloc::format;
use alloc::string::String;

/// WinHTTP's codes (`winhttp.h`).
pub const ERROR_WINHTTP_TIMEOUT: u32 = 12002;
pub const ERROR_WINHTTP_NAME_NOT_RESOLVED: u32 = 12007;
pub const ERROR_WINHTTP_CANNOT_CONNECT: u32 = 12029;
pub const ERROR_WINHTTP_CONNECTION_ERROR: u32 = 12030;

/// How the server could not be reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The name was not found (DNS).
    NotFound,
    /// The connection was refused (or reset at once).
    Refused,
    /// No answer in time.
    Timeout,
    /// The connection was cut off after it was made.
    CutOff,
}

impl Failure {
    /// The failure a WinHTTP error code is; `None` for any other error.
    pub fn of(code: u32) -> Option<Self> {
        match code {
            ERROR_WINHTTP_NAME_NOT_RESOLVED => Some(Self::NotFound),
            ERROR_WINHTTP_CANNOT_CONNECT => Some(Self::Refused),
            ERROR_WINHTTP_TIMEOUT => Some(Self::Timeout),
            ERROR_WINHTTP_CONNECTION_ERROR => Some(Self::CutOff),
            _ => None,
        }
    }
}

/// The first sentence: what happened, in English (the log's, and the
/// launcher's line, which is ASCII only).
pub fn english(failure: Failure, host: &str) -> String {
    first(0, failure, host)
}

fn first(language: u32, failure: Failure, host: &str) -> String {
    use Failure::*;
    match (language, failure) {
        (1, NotFound) => format!("Не удаётся найти {host}."),
        (1, Refused) => format!("{host} отклонил соединение."),
        (1, Timeout) => format!("{host} не отвечает."),
        (1, CutOff) => format!("Соединение с {host} прервано."),
        (2, NotFound) => format!("Не вдається знайти {host}."),
        (2, Refused) => format!("{host} відхилив з'єднання."),
        (2, Timeout) => format!("{host} не відповідає."),
        (2, CutOff) => format!("З'єднання з {host} перервано."),
        (3, NotFound) => format!("{host} wurde nicht gefunden."),
        (3, Refused) => format!("{host} hat die Verbindung abgelehnt."),
        (3, Timeout) => format!("{host} antwortet nicht."),
        (3, CutOff) => format!("Die Verbindung zu {host} wurde unterbrochen."),
        (4, NotFound) => format!("Nie można znaleźć {host}."),
        (4, Refused) => format!("{host} odrzucił połączenie."),
        (4, Timeout) => format!("{host} nie odpowiada."),
        (4, CutOff) => format!("Połączenie z {host} zostało przerwane."),
        (_, NotFound) => format!("Couldn't find {host}."),
        (_, Refused) => format!("{host} refused the connection."),
        (_, Timeout) => format!("{host} didn't answer."),
        (_, CutOff) => format!("The connection to {host} was cut off."),
    }
}

/// The box's words in the launcher's language (`crate::replay::LANGUAGES`
/// order; English for an unknown one): what happened, why it may have, and
/// what to do. The buttons are Windows' own Retry and Cancel, in Windows'
/// language: the words do not name them.
pub fn message(language: Option<u32>, failure: Failure, host: &str) -> String {
    let language = language.filter(|&n| n < 5).unwrap_or(0);
    let (why, what) = match language {
        1 => (
            "Возможно, его блокирует ваш интернет-провайдер (часто в России) или брандмауэр блокирует игру.",
            "Разрешите игру в брандмауэре (Skhid-network-setup.cmd); в России включите VPN и добавьте в него \
             launcher_skhid.exe, survarium_skhid.exe и updater_skhid.exe (см. VPN.txt). Затем попробуйте снова.",
        ),
        2 => (
            "Можливо, його блокує ваш інтернет-провайдер (часто в Росії) або брандмауер блокує гру.",
            "Дозвольте гру в брандмауері (Skhid-network-setup.cmd); у Росії увімкніть VPN і додайте до нього \
             launcher_skhid.exe, survarium_skhid.exe і updater_skhid.exe (див. VPN.txt). Потім спробуйте знову.",
        ),
        3 => (
            "Dein Internetanbieter blockiert ihn vielleicht (häufig in Russland), oder eine Firewall blockiert das Spiel.",
            "Erlaube das Spiel in deiner Firewall (Skhid-network-setup.cmd); in Russland nutze ein VPN und füge \
             launcher_skhid.exe, survarium_skhid.exe und updater_skhid.exe hinzu (siehe VPN.txt). Versuche es dann erneut.",
        ),
        4 => (
            "Być może blokuje go twój dostawca internetu (często w Rosji) albo zapora blokuje grę.",
            "Zezwól na grę w zaporze (Skhid-network-setup.cmd); w Rosji użyj VPN i dodaj do niego \
             launcher_skhid.exe, survarium_skhid.exe i updater_skhid.exe (zob. VPN.txt). Następnie spróbuj ponownie.",
        ),
        _ => (
            "Your internet provider may be blocking it (common in Russia), or a firewall is blocking the game.",
            "Allow the game in your firewall (Skhid-network-setup.cmd); in Russia, use a VPN and add \
             launcher_skhid.exe, survarium_skhid.exe and updater_skhid.exe to it (see VPN.txt). Then try again.",
        ),
    };
    format!("{}\n{why}\n{what}", first(language, failure, host))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Failure; 4] = [
        Failure::NotFound,
        Failure::Refused,
        Failure::Timeout,
        Failure::CutOff,
    ];

    #[test]
    fn winhttp_codes_are_told_apart() {
        assert_eq!(Failure::of(12007), Some(Failure::NotFound));
        assert_eq!(Failure::of(12029), Some(Failure::Refused));
        assert_eq!(Failure::of(12002), Some(Failure::Timeout));
        assert_eq!(Failure::of(12030), Some(Failure::CutOff));
        // A certificate, a proxy, anything else: not a blocked server.
        for code in [0, 5, 12175, 12038, 12045, 12057] {
            assert_eq!(Failure::of(code), None, "{code}");
        }
    }

    #[test]
    fn the_english_box_says_what_why_and_what_to_do() {
        assert_eq!(
            message(None, Failure::Timeout, "skhid.link"),
            "skhid.link didn't answer.\n\
             Your internet provider may be blocking it (common in Russia), or a firewall is blocking the game.\n\
             Allow the game in your firewall (Skhid-network-setup.cmd); in Russia, use a VPN and add \
             launcher_skhid.exe, survarium_skhid.exe and updater_skhid.exe to it (see VPN.txt). Then try again."
        );
        // An unknown language is English.
        assert_eq!(
            message(Some(9), Failure::Timeout, "skhid.link"),
            message(Some(0), Failure::Timeout, "skhid.link")
        );
        assert_eq!(
            english(Failure::NotFound, "skhid.link"),
            "Couldn't find skhid.link."
        );
        assert_eq!(
            english(Failure::Refused, "skhid.link"),
            "skhid.link refused the connection."
        );
    }

    #[test]
    fn every_language_names_the_host_the_programs_and_the_files() {
        for failure in ALL {
            let english = message(Some(0), failure, "skhid.link");
            for language in 0..5 {
                let text = message(Some(language), failure, "skhid.link");
                assert!(language == 0 || text != english, "{language} {failure:?}");
                // Three short lines: what happened, why, what to do.
                assert_eq!(text.lines().count(), 3, "{text}");
                assert!(
                    text.lines().next().unwrap().contains("skhid.link"),
                    "{text}"
                );
                for name in [
                    "launcher_skhid.exe",
                    "survarium_skhid.exe",
                    "updater_skhid.exe",
                    "Skhid-network-setup.cmd",
                    "VPN.txt",
                    "VPN ",
                ] {
                    assert!(text.contains(name), "{language} {failure:?}: {name}");
                }
            }
            // The four failures each have their own first line.
            for other in ALL {
                if other != failure {
                    for language in 0..5 {
                        assert_ne!(
                            message(Some(language), failure, "h").lines().next(),
                            message(Some(language), other, "h").lines().next()
                        );
                    }
                }
            }
        }
    }
}
