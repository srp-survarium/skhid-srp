//! Why the server cannot be reached, told apart so that the player is given
//! the one thing that helps: no internet, the server offline or restarting
//! (nothing to do but wait), this network blocking it while the relay
//! answers (the launcher's "Relay" row, `crate::route`), the relay down while the
//! server answers directly, or none of these to be told (a firewall). Asked
//! after a failure: the server's site and game both ways, then Windows
//! whether this PC is online.

use alloc::format;
use alloc::string::String;

use crate::route::Choice;

/// A way to the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Direct,
    Relay,
}

/// What answered by one way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// The site, and the game where it was asked.
    Up,
    /// The site, saying the game server behind it is down (HTTP 502, 503,
    /// 504 from its proxy): a release or a restart.
    Restarting,
    /// The site, but not the game's ports: this network blocks them.
    Partly,
    /// Nothing.
    Silent,
}

/// Whether this PC is online, as Windows says (its own connectivity test,
/// the network icon's), else as a DNS lookup of the server's name says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Internet {
    Online,
    Offline,
}

/// What the player is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diagnosis {
    NoInternet,
    /// Nothing of the server answers either way, and this PC is online.
    ServerDown,
    /// Back in a minute: the updater waits for it.
    Restarting,
    /// Not directly, but through the relay: the "Relay" row turns to auto.
    Blocked,
    /// The relay was used and does not answer, but the server does directly.
    RelayDown,
    /// Not reached, for no reason the updater can tell (no relay to ask, or
    /// the game's ports silent both ways): a firewall, most often.
    Unclear,
}

/// The one function from what answered to what the player is told; `None`
/// when the way used answers now (a failure that has passed: try again).
/// `route` is the player's, which the failed request went by (direct
/// without a relay; Automatic goes either way). `relayed` is `None` without
/// a relay. `internet` is asked only when nothing answered.
pub fn diagnose(
    route: Choice,
    direct: Reach,
    relayed: Option<Reach>,
    internet: impl FnOnce() -> Internet,
) -> Option<Diagnosis> {
    use Reach::*;
    let (this, other) = match route {
        // Either way does: the relay answering now is no failure.
        Choice::Automatic if relayed == Some(Up) => return None,
        Choice::Automatic | Choice::Direct => (direct, relayed),
        Choice::Relay => (relayed.unwrap_or(Silent), Some(direct)),
    };
    Some(match (this, other) {
        (Up, _) => return None,
        (Restarting, _) | (_, Some(Restarting)) => Diagnosis::Restarting,
        (Partly | Silent, Some(Up)) => match route {
            Choice::Automatic | Choice::Direct => Diagnosis::Blocked,
            Choice::Relay => Diagnosis::RelayDown,
        },
        (Partly, _) | (_, Some(Partly)) => Diagnosis::Unclear,
        (Silent, Some(Silent)) => match internet() {
            Internet::Offline => Diagnosis::NoInternet,
            Internet::Online => Diagnosis::ServerDown,
        },
        (Silent, None) => match internet() {
            Internet::Offline => Diagnosis::NoInternet,
            Internet::Online => Diagnosis::Unclear,
        },
    })
}

impl Diagnosis {
    /// The box's title, in the launcher's language (`crate::replay::LANGUAGES`
    /// order, English for another).
    pub fn title(self, language: u32) -> &'static str {
        use Diagnosis::*;
        match (language, self) {
            (1, NoInternet) => "Нет подключения к интернету",
            (1, ServerDown) => "Сервер недоступен",
            (1, Restarting) => "Сервер перезапускается",
            (1, Blocked) => "Заблокировано в этой сети",
            (1, RelayDown) => "Релей не отвечает",
            (1, Unclear) => "Нет соединения",
            (2, NoInternet) => "Немає підключення до інтернету",
            (2, ServerDown) => "Сервер недоступний",
            (2, Restarting) => "Сервер перезапускається",
            (2, Blocked) => "Заблоковано в цій мережі",
            (2, RelayDown) => "Релей не відповідає",
            (2, Unclear) => "Немає з'єднання",
            (3, NoInternet) => "Keine Internetverbindung",
            (3, ServerDown) => "Der Server ist offline",
            (3, Restarting) => "Der Server startet neu",
            (3, Blocked) => "In diesem Netzwerk gesperrt",
            (3, RelayDown) => "Das Relay antwortet nicht",
            (3, Unclear) => "Keine Verbindung",
            (4, NoInternet) => "Brak połączenia z internetem",
            (4, ServerDown) => "Serwer jest offline",
            (4, Restarting) => "Serwer uruchamia się ponownie",
            (4, Blocked) => "Zablokowane w tej sieci",
            (4, RelayDown) => "Relay nie odpowiada",
            (4, Unclear) => "Brak połączenia",
            (_, NoInternet) => "No internet connection",
            (_, ServerDown) => "The server is offline",
            (_, Restarting) => "The server is restarting",
            (_, Blocked) => "Blocked on this network",
            (_, RelayDown) => "The relay does not answer",
            (_, Unclear) => "No connection",
        }
    }

    /// What to do, one line. No VPN, relay or firewall advice where the
    /// server is down or the PC offline.
    pub fn advice(self, language: u32) -> &'static str {
        use Diagnosis::*;
        match (language, self) {
            (1, NoInternet) => "Проверьте сеть и попробуйте снова.",
            (1, ServerDown) => "Он не отвечает ни напрямую, ни через релей. Попробуйте позже.",
            (1, Restarting) => "Он вернётся через минуту.",
            (1, Blocked) => "Провайдер блокирует сервер, но релей отвечает: «Релей» теперь на авто (в лаунчере слева внизу). Попробуйте снова.",
            (1, RelayDown) => "Сервер отвечает напрямую: поставьте «Релей» на авто или выкл в лаунчере слева внизу и попробуйте снова.",
            (1, Unclear) => "Сервер здесь не отвечает: возможно, игру блокирует брандмауэр. Запустите Skhid-network-setup.cmd (рядом со Skhid.bat) и попробуйте снова.",
            (2, NoInternet) => "Перевірте мережу й спробуйте знову.",
            (2, ServerDown) => "Він не відповідає ні напряму, ні через релей. Спробуйте пізніше.",
            (2, Restarting) => "Він повернеться за хвилину.",
            (2, Blocked) => "Провайдер блокує сервер, але релей відповідає: «Релей» тепер на авто (у лаунчері ліворуч унизу). Спробуйте знову.",
            (2, RelayDown) => "Сервер відповідає напряму: поставте «Релей» на авто або вимк у лаунчері ліворуч унизу і спробуйте знову.",
            (2, Unclear) => "Сервер тут не відповідає: можливо, гру блокує брандмауер. Запустіть Skhid-network-setup.cmd (поруч зі Skhid.bat) і спробуйте знову.",
            (3, NoInternet) => "Prüfe dein Netzwerk und versuche es erneut.",
            (3, ServerDown) => "Er antwortet weder direkt noch über das Relay. Versuche es später.",
            (3, Restarting) => "Er ist in einer Minute zurück.",
            (3, Blocked) => "Dein Anbieter sperrt den Server, das Relay antwortet aber: „Relay“ steht jetzt auf Auto (im Launcher unten links). Versuche es erneut.",
            (3, RelayDown) => "Der Server antwortet direkt: stelle „Relay“ im Launcher unten links auf Auto oder Aus und versuche es erneut.",
            (3, Unclear) => "Der Server antwortet hier nicht: vielleicht blockiert eine Firewall das Spiel. Starte Skhid-network-setup.cmd (neben Skhid.bat) und versuche es erneut.",
            (4, NoInternet) => "Sprawdź sieć i spróbuj ponownie.",
            (4, ServerDown) => "Nie odpowiada ani bezpośrednio, ani przez relay. Spróbuj później.",
            (4, Restarting) => "Wróci za minutę.",
            (4, Blocked) => "Dostawca blokuje serwer, ale relay odpowiada: „Relay” jest teraz na auto (w launcherze na dole po lewej). Spróbuj ponownie.",
            (4, RelayDown) => "Serwer odpowiada bezpośrednio: ustaw „Relay” w launcherze na dole po lewej na auto lub wył. i spróbuj ponownie.",
            (4, Unclear) => "Serwer tu nie odpowiada: być może zapora blokuje grę. Uruchom Skhid-network-setup.cmd (obok Skhid.bat) i spróbuj ponownie.",
            (_, NoInternet) => "Check your network, then try again.",
            (_, ServerDown) => "It does not answer, directly or through the relay. Try again later.",
            (_, Restarting) => "It is back in a minute.",
            (_, Blocked) => "Your provider blocks the server, but the relay answers: \"Relay\" is now on auto (at the launcher's bottom left). Try again.",
            (_, RelayDown) => "The server answers directly: set \"Relay\" to auto or off at the launcher's bottom left, then try again.",
            (_, Unclear) => "The server does not answer here: a firewall may block the game. Run Skhid-network-setup.cmd (beside Skhid.bat), then try again.",
        }
    }
}

/// The last line of the box that waits while the server restarts: the
/// seconds to the next try, counted down.
pub fn next_try(language: u32, seconds: u32) -> String {
    match language {
        1 => format!("Следующая попытка через {seconds} с."),
        2 => format!("Наступна спроба за {seconds} с."),
        3 => format!("Nächster Versuch in {seconds} s."),
        4 => format!("Następna próba za {seconds} s."),
        _ => format!("Next try in {seconds} s."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Reach::*;

    const ALL: [Diagnosis; 6] = [
        Diagnosis::NoInternet,
        Diagnosis::ServerDown,
        Diagnosis::Restarting,
        Diagnosis::Blocked,
        Diagnosis::RelayDown,
        Diagnosis::Unclear,
    ];

    fn online() -> Internet {
        Internet::Online
    }

    fn offline() -> Internet {
        Internet::Offline
    }

    fn unasked() -> Internet {
        panic!("the internet asked while something answered")
    }

    #[test]
    fn blocked_only_when_the_relay_answers_and_direct_does_not() {
        for direct in [Partly, Silent] {
            assert_eq!(diagnose(Choice::Direct, direct, Some(Up), unasked), Some(Diagnosis::Blocked));
        }
        // Without a relay nothing tells a block from a firewall.
        assert_eq!(diagnose(Choice::Direct, Partly, None, unasked), Some(Diagnosis::Unclear));
        assert_eq!(diagnose(Choice::Direct, Silent, None, online), Some(Diagnosis::Unclear));
    }

    #[test]
    fn a_site_that_says_the_game_is_down_is_a_restart_either_way() {
        for (direct, relayed) in [
            (Restarting, Some(Up)),
            (Restarting, None),
            (Silent, Some(Restarting)),
            (Partly, Some(Restarting)),
        ] {
            assert_eq!(diagnose(Choice::Direct, direct, relayed, unasked), Some(Diagnosis::Restarting));
        }
        assert_eq!(diagnose(Choice::Relay, Restarting, Some(Silent), unasked), Some(Diagnosis::Restarting));
    }

    #[test]
    fn nothing_answering_is_the_server_down_or_no_internet() {
        assert_eq!(diagnose(Choice::Direct, Silent, Some(Silent), online), Some(Diagnosis::ServerDown));
        assert_eq!(diagnose(Choice::Relay, Silent, Some(Silent), online), Some(Diagnosis::ServerDown));
        assert_eq!(diagnose(Choice::Direct, Silent, Some(Silent), offline), Some(Diagnosis::NoInternet));
        assert_eq!(diagnose(Choice::Direct, Silent, None, offline), Some(Diagnosis::NoInternet));
    }

    #[test]
    fn the_relay_down_while_direct_answers() {
        for relayed in [Some(Silent), Some(Partly), None] {
            assert_eq!(diagnose(Choice::Relay, Up, relayed, unasked), Some(Diagnosis::RelayDown));
        }
        // The way used answers now: nothing to tell.
        assert_eq!(diagnose(Choice::Relay, Silent, Some(Up), unasked), None);
        assert_eq!(diagnose(Choice::Direct, Up, Some(Silent), unasked), None);
    }

    #[test]
    fn automatic_goes_either_way_and_is_never_told_to_change_the_box() {
        for direct in [Up, Partly, Silent, Restarting] {
            assert_eq!(diagnose(Choice::Automatic, direct, Some(Up), unasked), None);
        }
        assert_eq!(diagnose(Choice::Automatic, Up, Some(Silent), unasked), None);
        assert_eq!(diagnose(Choice::Automatic, Silent, Some(Silent), online), Some(Diagnosis::ServerDown));
        assert_eq!(diagnose(Choice::Automatic, Silent, Some(Silent), offline), Some(Diagnosis::NoInternet));
        assert_eq!(diagnose(Choice::Automatic, Silent, Some(Restarting), unasked), Some(Diagnosis::Restarting));
        assert_eq!(diagnose(Choice::Automatic, Partly, Some(Partly), unasked), Some(Diagnosis::Unclear));
    }

    #[test]
    fn the_games_ports_silent_both_ways_is_unclear() {
        assert_eq!(diagnose(Choice::Direct, Partly, Some(Partly), unasked), Some(Diagnosis::Unclear));
        assert_eq!(diagnose(Choice::Direct, Silent, Some(Partly), unasked), Some(Diagnosis::Unclear));
    }

    #[test]
    fn every_language_tells_each_apart_and_advises_only_what_helps() {
        for language in 0..6 {
            let mut titles: alloc::vec::Vec<&str> = ALL.iter().map(|d| d.title(language)).collect();
            titles.sort();
            titles.dedup();
            assert_eq!(titles.len(), ALL.len(), "{language}");
            for diagnosis in ALL {
                let advice = diagnosis.advice(language);
                assert!(!advice.contains("VPN"), "{advice}");
                let names_the_box = advice.contains("\"Relay\"") || advice.contains("«Релей»")
                    || advice.contains("„Relay“") || advice.contains("„Relay”");
                assert_eq!(
                    names_the_box,
                    matches!(diagnosis, Diagnosis::Blocked | Diagnosis::RelayDown),
                    "{language} {diagnosis:?}: {advice}"
                );
                assert_eq!(
                    advice.contains("Skhid-network-setup.cmd"),
                    diagnosis == Diagnosis::Unclear,
                    "{advice}"
                );
                if language > 0 && language < 5 {
                    assert_ne!(advice, diagnosis.advice(0));
                }
            }
        }
        assert_eq!(Diagnosis::Blocked.title(9), "Blocked on this network");
        // The waiting line keeps its width while it counts (one digit).
        for language in 0..5 {
            assert_eq!(next_try(language, 5).chars().count(), next_try(language, 1).chars().count());
            assert!(next_try(language, 3).contains('3'));
        }
    }
}
