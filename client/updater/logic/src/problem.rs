//! The four problems a player meets most, each in a box of its own whose
//! first line is a fixed title the site's "If something goes wrong" quotes
//! (the home page, `web-server`), so that a player can match the box to the
//! line: game files an antivirus removed, a download that failed, no
//! connection, a server this network blocks. Each title begins with the
//! same words for its problem and goes on with what failed (the file, the
//! host, how); the box's caption is that beginning in English, whatever the
//! launcher's language. Then what happened, one line of what to run (a file
//! beside `Skhid.bat`), and the log's path.
//!
//! A failure of every address of something ([`Failed`]) becomes its problem
//! in [`Problem::of`]: an address that answered with no usable file is a
//! failed download; none that answered is no connection, or, when a site
//! that is always up answers ([`Probe`]), a server this network blocks.

use alloc::format;
use alloc::string::String;

use crate::network::Failure;

/// What could not be reached: the update server, by its host, or a file,
/// by its name, at every one of its addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Where {
    Server(String),
    File(String),
}

/// How an address that answered gave no usable file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// Another checksum than the one expected.
    Damaged,
    /// The connection ended before the file did.
    CutShort,
    /// Slower than the updater waits for.
    TooSlow,
    /// A web page (a sign-in, a quota page), not the file.
    WebPage,
    /// An HTTP status other than the file.
    Status(u32),
    /// Any other answer that is not the file (a redirect too many, a wrong
    /// range).
    Other,
}

/// How one address of a file failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Miss {
    /// No answer at all (`crate::network`).
    Unreached(Failure),
    /// It answered, with no usable file.
    Answered(Fault),
}

/// Every address of something failed: how, before the probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failed {
    /// No address answered.
    Unreached { what: Where, failure: Failure },
    /// An address answered, with no usable file (the first that did: how);
    /// `addresses`: how many there were.
    Download {
        file: String,
        fault: Fault,
        addresses: usize,
    },
}

impl Failed {
    /// Every address of `file` failed, each as `misses` says, in their
    /// order: a download that failed when one of them answered (the first
    /// that did), else none reached (the first's way). `None` for no
    /// address.
    pub fn of_file(file: &str, misses: &[Miss]) -> Option<Self> {
        let answered = misses.iter().find_map(|miss| match miss {
            Miss::Answered(fault) => Some(*fault),
            Miss::Unreached(_) => None,
        });
        match (answered, misses.first()) {
            (Some(fault), _) => Some(Self::Download {
                file: file.into(),
                fault,
                addresses: misses.len(),
            }),
            (None, Some(Miss::Unreached(failure))) => Some(Self::Unreached {
                what: Where::File(file.into()),
                failure: *failure,
            }),
            (None, _) => None,
        }
    }
}

/// What a site that is always up did when asked, after nothing of ours
/// answered: answered (this PC is online, so this network blocks ours), or
/// not (a firewall, or no connection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    Answered,
    Silent,
}

/// Why an antivirus is named: the file it took is blocked now (PLAY stays
/// off), or was gone at the start and has been put back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    Blocked,
    PutBack,
}

/// A problem the player is told of in its box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// An antivirus removed or blocked a game file (its name); `folder`: the
    /// one to exclude (`crate::file_error::exclusion_folder`).
    Antivirus {
        file: String,
        folder: String,
        removal: Removal,
    },
    /// Every address of a file failed, one of them with an answer.
    Download {
        file: String,
        fault: Fault,
        addresses: usize,
    },
    /// Nothing answered, and neither did a site that is always up.
    NoConnection { what: Where, failure: Failure },
    /// Nothing of ours answered, but a site that is always up did.
    Blocked { what: Where, failure: Failure },
}

impl Problem {
    /// The problem of a failure; `probe` is asked only when nothing
    /// answered, and once.
    pub fn of(failed: Failed, probe: impl FnOnce() -> Probe) -> Self {
        match failed {
            Failed::Download {
                file,
                fault,
                addresses,
            } => Self::Download {
                file,
                fault,
                addresses,
            },
            Failed::Unreached { what, failure } => match probe() {
                Probe::Answered => Self::Blocked { what, failure },
                Probe::Silent => Self::NoConnection { what, failure },
            },
        }
    }

    /// The box's caption: the title's beginning, in English always.
    pub fn caption(&self) -> String {
        format!("Skhid updater: {}", self.kind(0))
    }

    /// The fixed beginning of the title, in `language`.
    fn kind(&self, language: u32) -> &'static str {
        match (language, self) {
            (1, Self::Antivirus { .. }) => "Файлы игры удалены антивирусом",
            (1, Self::Download { .. }) => "Загрузка не удалась",
            (1, Self::NoConnection { .. }) => "Нет соединения",
            (1, Self::Blocked { .. }) => "Заблокировано в этой сети",
            (2, Self::Antivirus { .. }) => "Файли гри видалено антивірусом",
            (2, Self::Download { .. }) => "Завантаження не вдалося",
            (2, Self::NoConnection { .. }) => "Немає з'єднання",
            (2, Self::Blocked { .. }) => "Заблоковано в цій мережі",
            (3, Self::Antivirus { .. }) => "Spieldateien vom Virenschutz entfernt",
            (3, Self::Download { .. }) => "Download fehlgeschlagen",
            (3, Self::NoConnection { .. }) => "Keine Verbindung",
            (3, Self::Blocked { .. }) => "In diesem Netzwerk gesperrt",
            (4, Self::Antivirus { .. }) => "Pliki gry usunięte przez antywirusa",
            (4, Self::Download { .. }) => "Pobieranie nie powiodło się",
            (4, Self::NoConnection { .. }) => "Brak połączenia",
            (4, Self::Blocked { .. }) => "Zablokowane w tej sieci",
            (_, Self::Antivirus { .. }) => "Game files removed by antivirus",
            (_, Self::Download { .. }) => "Download failed",
            (_, Self::NoConnection { .. }) => "No connection",
            (_, Self::Blocked { .. }) => "Blocked on this network",
        }
    }

    /// The title: the fixed beginning, then what failed. One line.
    pub fn title(&self, language: Option<u32>) -> String {
        let language = known(language);
        let what = match self {
            Self::Antivirus { file, .. } => file.clone(),
            Self::Download {
                file,
                fault,
                addresses,
            } => {
                let every = if *addresses > 1 {
                    every_address(language)
                } else {
                    ""
                };
                format!("{file}, {}{every}", fault_words(language, *fault))
            }
            Self::NoConnection { what, failure } | Self::Blocked { what, failure } => {
                unreached(language, what, *failure)
            }
        };
        format!("{}: {what}", self.kind(language))
    }

    /// The box's words in the launcher's language (`crate::replay::LANGUAGES`
    /// order; English for an unknown one): the title, what happened, what to
    /// run, the log's path (`log`). The buttons are Windows' own, in
    /// Windows' language: the words do not name them.
    pub fn message(&self, language: Option<u32>, log: &str) -> String {
        let title = self.title(language);
        let language = known(language);
        let log = match language {
            1 | 2 => format!("Журнал: {log}"),
            3 => format!("Protokoll: {log}"),
            4 => format!("Dziennik: {log}"),
            _ => format!("Log: {log}"),
        };
        match self {
            Self::Antivirus {
                folder, removal, ..
            } => format!(
                "{title}\n{}\n{}\n{log}",
                removal_words(language, *removal),
                defender(language, folder)
            ),
            Self::Download { .. } => format!("{title}\n{}\n{log}", download(language)),
            Self::NoConnection { .. } => format!(
                "{title}\n{}\n{}\n{log}",
                match language {
                    1 => "Другие сайты тоже не отвечают: брандмауэр блокирует игру, или компьютер не в сети.",
                    2 => "Інші сайти теж не відповідають: брандмауер блокує гру, або комп'ютер не в мережі.",
                    3 => "Auch andere Seiten antworten nicht: eine Firewall blockiert das Spiel, oder dieser PC ist offline.",
                    4 => "Inne strony też nie odpowiadają: zapora blokuje grę albo komputer jest offline.",
                    _ => "No other site answered either: a firewall blocks the game, or this PC is offline.",
                },
                match language {
                    1 => "Запустите Skhid-network-setup.cmd (рядом со Skhid.bat), чтобы разрешить игру в брандмауэре, затем попробуйте снова.",
                    2 => "Запустіть Skhid-network-setup.cmd (поруч зі Skhid.bat), щоб дозволити гру в брандмауері, потім спробуйте знову.",
                    3 => "Starte Skhid-network-setup.cmd (neben Skhid.bat), um das Spiel in der Firewall zu erlauben, und versuche es dann erneut.",
                    4 => "Uruchom Skhid-network-setup.cmd (obok Skhid.bat), aby zezwolić na grę w zaporze, następnie spróbuj ponownie.",
                    _ => "Run Skhid-network-setup.cmd (beside Skhid.bat) to allow the game in the firewall, then try again.",
                }
            ),
            Self::Blocked { .. } => format!(
                "{title}\n{}\n{}\n{log}",
                match language {
                    1 => "Другие сайты отвечают: его блокирует эта сеть (интернет-провайдер или включённый VPN).",
                    2 => "Інші сайти відповідають: його блокує ця мережа (інтернет-провайдер або увімкнений VPN).",
                    3 => "Andere Seiten antworten: dieses Netzwerk sperrt ihn (der Internetanbieter oder das aktive VPN).",
                    4 => "Inne strony odpowiadają: blokuje go ta sieć (dostawca internetu lub włączony VPN).",
                    _ => "Other sites answer: this network blocks it (the internet provider, or the VPN in use).",
                },
                match language {
                    1 => "Включите VPN и добавьте в его раздельное туннелирование launcher_skhid.exe, survarium_skhid.exe и updater_skhid.exe, или выключите VPN (см. VPN.txt). Затем попробуйте снова.",
                    2 => "Увімкніть VPN і додайте до його роздільного тунелювання launcher_skhid.exe, survarium_skhid.exe і updater_skhid.exe, або вимкніть VPN (див. VPN.txt). Потім спробуйте знову.",
                    3 => "Nutze ein VPN mit launcher_skhid.exe, survarium_skhid.exe und updater_skhid.exe im Split-Tunneling, oder schalte das VPN aus (siehe VPN.txt). Versuche es dann erneut.",
                    4 => "Użyj VPN z launcher_skhid.exe, survarium_skhid.exe i updater_skhid.exe w tunelowaniu dzielonym albo wyłącz VPN (zob. VPN.txt). Następnie spróbuj ponownie.",
                    _ => "Use a VPN with launcher_skhid.exe, survarium_skhid.exe and updater_skhid.exe in its split tunnelling, or turn the VPN off (see VPN.txt). Then try again.",
                }
            ),
        }
    }
}

/// A language the boxes have (`crate::replay::LANGUAGES` order), else
/// English.
fn known(language: Option<u32>) -> u32 {
    language.filter(|&n| n < 5).unwrap_or(0)
}

fn every_address(language: u32) -> &'static str {
    match language {
        1 => " по всем адресам",
        2 => " за всіма адресами",
        3 => " bei jeder Adresse",
        4 => " pod każdym adresem",
        _ => " at every address",
    }
}

fn fault_words(language: u32, fault: Fault) -> String {
    use Fault::*;
    match (language, fault) {
        (1, Damaged) => "файл повреждён".into(),
        (1, CutShort) => "загрузка оборвалась".into(),
        (1, TooSlow) => "слишком медленно".into(),
        (1, WebPage) => "веб-страница вместо файла".into(),
        (1, Status(status)) => format!("отказ сервера (HTTP {status})"),
        (1, Other) => "файл не подошёл".into(),
        (2, Damaged) => "файл пошкоджено".into(),
        (2, CutShort) => "завантаження обірвалося".into(),
        (2, TooSlow) => "надто повільно".into(),
        (2, WebPage) => "вебсторінка замість файлу".into(),
        (2, Status(status)) => format!("відмова сервера (HTTP {status})"),
        (2, Other) => "файл не підійшов".into(),
        (3, Damaged) => "Datei beschädigt".into(),
        (3, CutShort) => "Download abgebrochen".into(),
        (3, TooSlow) => "zu langsam".into(),
        (3, WebPage) => "Webseite statt Datei".into(),
        (3, Status(status)) => format!("vom Server abgelehnt (HTTP {status})"),
        (3, Other) => "Datei unbrauchbar".into(),
        (4, Damaged) => "plik uszkodzony".into(),
        (4, CutShort) => "pobieranie przerwane".into(),
        (4, TooSlow) => "zbyt wolno".into(),
        (4, WebPage) => "strona WWW zamiast pliku".into(),
        (4, Status(status)) => format!("serwer odmówił (HTTP {status})"),
        (4, Other) => "plik nie nadaje się".into(),
        (_, Damaged) => "damaged".into(),
        (_, CutShort) => "cut short".into(),
        (_, TooSlow) => "too slow".into(),
        (_, WebPage) => "a web page instead of the file".into(),
        (_, Status(status)) => format!("refused (HTTP {status})"),
        (_, Other) => "not usable".into(),
    }
}

/// How a connection failed, in a few words.
fn how(language: u32, failure: Failure) -> &'static str {
    use Failure::*;
    match (language, failure) {
        (1, NotFound) => "имя не найдено",
        (1, Refused) => "соединение отклонено",
        (1, Timeout) => "время ожидания истекло",
        (1, CutOff) => "соединение прервано",
        (2, NotFound) => "ім'я не знайдено",
        (2, Refused) => "з'єднання відхилено",
        (2, Timeout) => "час очікування вичерпано",
        (2, CutOff) => "з'єднання перервано",
        (3, NotFound) => "Name nicht gefunden",
        (3, Refused) => "Verbindung abgelehnt",
        (3, Timeout) => "Zeitüberschreitung",
        (3, CutOff) => "Verbindung abgebrochen",
        (4, NotFound) => "nie znaleziono nazwy",
        (4, Refused) => "połączenie odrzucone",
        (4, Timeout) => "przekroczono czas",
        (4, CutOff) => "połączenie przerwane",
        (_, NotFound) => "name not found",
        (_, Refused) => "connection refused",
        (_, Timeout) => "timed out",
        (_, CutOff) => "connection cut off",
    }
}

/// What did not answer, and how.
fn unreached(language: u32, what: &Where, failure: Failure) -> String {
    let how = how(language, failure);
    match (language, what) {
        (1, Where::Server(host)) => format!("{host} не отвечает ({how})"),
        (1, Where::File(file)) => format!("ни один адрес {file} не отвечает ({how})"),
        (2, Where::Server(host)) => format!("{host} не відповідає ({how})"),
        (2, Where::File(file)) => format!("жодна адреса {file} не відповідає ({how})"),
        (3, Where::Server(host)) => format!("{host} antwortet nicht ({how})"),
        (3, Where::File(file)) => format!("keine Adresse von {file} antwortet ({how})"),
        (4, Where::Server(host)) => format!("{host} nie odpowiada ({how})"),
        (4, Where::File(file)) => format!("żaden adres {file} nie odpowiada ({how})"),
        (_, Where::Server(host)) => format!("{host} did not answer ({how})"),
        (_, Where::File(file)) => format!("no address of {file} answered ({how})"),
    }
}

fn removal_words(language: u32, removal: Removal) -> &'static str {
    match (language, removal) {
        (1, Removal::Blocked) => "Безопасность Windows или другой антивирус принял его за угрозу (ложное срабатывание). До этого игра не запустится.",
        (1, Removal::PutBack) => "Его не было, и он возвращён на место; антивирус может удалить его снова.",
        (2, Removal::Blocked) => "Безпека Windows або інший антивірус сприйняв його як загрозу (хибне спрацювання). Доти гра не запуститься.",
        (2, Removal::PutBack) => "Його не було, і його повернуто на місце; антивірус може видалити його знову.",
        (3, Removal::Blocked) => "Windows-Sicherheit oder ein anderer Virenschutz hält sie für eine Bedrohung (ein Fehlalarm). Bis dahin startet das Spiel nicht.",
        (3, Removal::PutBack) => "Sie fehlte und wurde wiederhergestellt; der Virenschutz kann sie wieder entfernen.",
        (4, Removal::Blocked) => "Zabezpieczenia Windows lub inny antywirus uznał go za zagrożenie (fałszywy alarm). Do tego czasu gra się nie uruchomi.",
        (4, Removal::PutBack) => "Brakowało go i został przywrócony; antywirus może go znowu usunąć.",
        (_, Removal::Blocked) => "Windows Security or another antivirus took it for a threat (a false positive). PLAY stays off until it is let through.",
        (_, Removal::PutBack) => "It was gone and has been put back; the antivirus may remove it again.",
    }
}

fn defender(language: u32, folder: &str) -> String {
    match language {
        1 => format!(
            "Запустите Skhid-defender-setup.cmd (рядом со Skhid.bat); с другим антивирусом добавьте в исключения папку: {folder}"
        ),
        2 => format!(
            "Запустіть Skhid-defender-setup.cmd (поруч зі Skhid.bat); з іншим антивірусом додайте до винятків теку: {folder}"
        ),
        3 => format!(
            "Starte Skhid-defender-setup.cmd (neben Skhid.bat); bei einem anderen Virenschutz schließe diesen Ordner aus: {folder}"
        ),
        4 => format!(
            "Uruchom Skhid-defender-setup.cmd (obok Skhid.bat); przy innym antywirusie wyklucz ten folder: {folder}"
        ),
        _ => format!(
            "Run Skhid-defender-setup.cmd (beside Skhid.bat); with another antivirus, exclude this folder: {folder}"
        ),
    }
}

fn download(language: u32) -> &'static str {
    match language {
        1 => "Запустите Skhid.bat снова: он снова попробует все адреса, а загрузка игры продолжится с того места, где остановилась.",
        2 => "Запустіть Skhid.bat знову: він знову спробує всі адреси, а завантаження гри продовжиться з того місця, де зупинилося.",
        3 => "Starte Skhid.bat erneut: es versucht jede Adresse erneut, und der Download des Spiels setzt dort fort, wo er stehen blieb.",
        4 => "Uruchom Skhid.bat ponownie: spróbuje znów każdego adresu, a pobieranie gry będzie kontynuowane od miejsca, w którym się zatrzymało.",
        _ => "Start Skhid.bat again: it tries every address again, and the game's download goes on from where it stopped.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const FAILURES: [Failure; 4] = [
        Failure::NotFound,
        Failure::Refused,
        Failure::Timeout,
        Failure::CutOff,
    ];
    const FAULTS: [Fault; 6] = [
        Fault::Damaged,
        Fault::CutShort,
        Fault::TooSlow,
        Fault::WebPage,
        Fault::Status(404),
        Fault::Other,
    ];
    const LOG: &str = "E:\\S\\game\\binaries\\x86\\updater_skhid.log";
    const FOLDER: &str = "E:\\S\\game\\binaries\\x86";

    fn server() -> Where {
        Where::Server("skhid.link".into())
    }

    /// One of each problem, with every way it can be.
    fn every() -> Vec<Problem> {
        let mut all = Vec::new();
        for removal in [Removal::Blocked, Removal::PutBack] {
            all.push(Problem::Antivirus {
                file: "survarium_skhid.exe".into(),
                folder: FOLDER.into(),
                removal,
            });
        }
        for fault in FAULTS {
            for addresses in [1, 3] {
                all.push(Problem::Download {
                    file: "resources.db.patch_44".into(),
                    fault,
                    addresses,
                });
            }
        }
        for failure in FAILURES {
            for what in [server(), Where::File("resources.db.patch_44".into())] {
                all.push(Problem::NoConnection {
                    what: what.clone(),
                    failure,
                });
                all.push(Problem::Blocked { what, failure });
            }
        }
        all
    }

    #[test]
    fn a_file_that_answered_anywhere_is_a_failed_download() {
        let timeout = Miss::Unreached(Failure::Timeout);
        assert_eq!(
            Failed::of_file(
                "resources.db.patch_44",
                &[timeout, Miss::Answered(Fault::Damaged), Miss::Answered(Fault::TooSlow)]
            ),
            Some(Failed::Download {
                file: "resources.db.patch_44".into(),
                fault: Fault::Damaged,
                addresses: 3,
            })
        );
        assert_eq!(
            Failed::of_file("survarium-026g.7z", &[Miss::Answered(Fault::WebPage)]),
            Some(Failed::Download {
                file: "survarium-026g.7z".into(),
                fault: Fault::WebPage,
                addresses: 1,
            })
        );
        // None answered: the first's way.
        assert_eq!(
            Failed::of_file(
                "resources.db.patch_44",
                &[
                    Miss::Unreached(Failure::Refused),
                    timeout,
                    Miss::Unreached(Failure::NotFound)
                ]
            ),
            Some(Failed::Unreached {
                what: Where::File("resources.db.patch_44".into()),
                failure: Failure::Refused,
            })
        );
        assert_eq!(Failed::of_file("x", &[]), None);
    }

    #[test]
    fn nothing_answered_is_blocked_when_another_site_answers_else_no_connection() {
        let failed = Failed::Unreached {
            what: server(),
            failure: Failure::Timeout,
        };
        assert_eq!(
            Problem::of(failed.clone(), || Probe::Answered),
            Problem::Blocked {
                what: server(),
                failure: Failure::Timeout
            }
        );
        assert_eq!(
            Problem::of(failed, || Probe::Silent),
            Problem::NoConnection {
                what: server(),
                failure: Failure::Timeout
            }
        );
        // A download that failed is not probed.
        let download = Failed::Download {
            file: "skhid.link/download".into(),
            fault: Fault::CutShort,
            addresses: 1,
        };
        assert_eq!(
            Problem::of(download, || panic!("probed")),
            Problem::Download {
                file: "skhid.link/download".into(),
                fault: Fault::CutShort,
                addresses: 1,
            }
        );
    }

    #[test]
    fn the_english_boxes() {
        let blocked = Problem::Blocked {
            what: server(),
            failure: Failure::Timeout,
        };
        assert_eq!(blocked.caption(), "Skhid updater: Blocked on this network");
        assert_eq!(
            blocked.message(None, LOG),
            "Blocked on this network: skhid.link did not answer (timed out)\n\
             Other sites answer: this network blocks it (the internet provider, or the VPN in use).\n\
             Use a VPN with launcher_skhid.exe, survarium_skhid.exe and updater_skhid.exe in its \
             split tunnelling, or turn the VPN off (see VPN.txt). Then try again.\n\
             Log: E:\\S\\game\\binaries\\x86\\updater_skhid.log"
        );
        assert_eq!(
            Problem::NoConnection {
                what: Where::File("resources.db.patch_44".into()),
                failure: Failure::Refused,
            }
            .message(Some(0), LOG),
            "No connection: no address of resources.db.patch_44 answered (connection refused)\n\
             No other site answered either: a firewall blocks the game, or this PC is offline.\n\
             Run Skhid-network-setup.cmd (beside Skhid.bat) to allow the game in the firewall, then try again.\n\
             Log: E:\\S\\game\\binaries\\x86\\updater_skhid.log"
        );
        assert_eq!(
            Problem::Download {
                file: "resources.db.patch_44".into(),
                fault: Fault::Damaged,
                addresses: 3,
            }
            .message(None, LOG),
            "Download failed: resources.db.patch_44, damaged at every address\n\
             Start Skhid.bat again: it tries every address again, and the game's download goes on \
             from where it stopped.\n\
             Log: E:\\S\\game\\binaries\\x86\\updater_skhid.log"
        );
        assert_eq!(
            Problem::Antivirus {
                file: "survarium_skhid.exe".into(),
                folder: FOLDER.into(),
                removal: Removal::Blocked,
            }
            .message(None, LOG),
            "Game files removed by antivirus: survarium_skhid.exe\n\
             Windows Security or another antivirus took it for a threat (a false positive). PLAY \
             stays off until it is let through.\n\
             Run Skhid-defender-setup.cmd (beside Skhid.bat); with another antivirus, exclude this \
             folder: E:\\S\\game\\binaries\\x86\n\
             Log: E:\\S\\game\\binaries\\x86\\updater_skhid.log"
        );
        assert_eq!(
            Problem::Download {
                file: "skhid.link/download".into(),
                fault: Fault::Status(503),
                addresses: 1,
            }
            .title(None),
            "Download failed: skhid.link/download, refused (HTTP 503)"
        );
    }

    #[test]
    fn every_language_has_each_box_with_its_title_its_file_to_run_and_the_log() {
        let all = every();
        for problem in &all {
            let english = problem.message(Some(0), LOG);
            // An unknown language is English.
            assert_eq!(problem.message(Some(9), LOG), english);
            assert_eq!(problem.message(None, LOG), english);
            let run = match problem {
                Problem::Antivirus { .. } => "Skhid-defender-setup.cmd",
                Problem::Download { .. } => "Skhid.bat",
                Problem::NoConnection { .. } => "Skhid-network-setup.cmd",
                Problem::Blocked { .. } => "VPN.txt",
            };
            for language in 0..5 {
                let text = problem.message(Some(language), LOG);
                assert!(language == 0 || text != english, "{language} {problem:?}");
                let lines: Vec<&str> = text.lines().collect();
                // The title, what happened (but for a download), what to
                // run, the log.
                let count = if let Problem::Download { .. } = problem {
                    3
                } else {
                    4
                };
                assert_eq!(lines.len(), count, "{text}");
                assert_eq!(lines[0], problem.title(Some(language)));
                assert!(
                    lines[0].starts_with(problem.kind(language)),
                    "{language}: {text}"
                );
                assert!(lines[count - 2].contains(run), "{language}: {text}");
                assert!(lines[count - 1].ends_with(LOG), "{language}: {text}");
                // The object of the title: the file or the host.
                let object = match problem {
                    Problem::Antivirus { file, .. } | Problem::Download { file, .. } => file,
                    Problem::NoConnection { what, .. } | Problem::Blocked { what, .. } => {
                        match what {
                            Where::Server(host) => host,
                            Where::File(file) => file,
                        }
                    }
                };
                assert!(lines[0].contains(object.as_str()), "{language}: {text}");
                // Nothing about countries.
                for word in ["Russia", "Росси", "Росії", "Russland", "Rosji"] {
                    assert!(!text.contains(word), "{language}: {text}");
                }
                if let Problem::Antivirus { .. } = problem {
                    assert!(text.contains(FOLDER), "{text}");
                }
                if let Problem::Blocked { .. } = problem {
                    for name in ["launcher_skhid.exe", "survarium_skhid.exe", "updater_skhid.exe"]
                    {
                        assert!(text.contains(name), "{language}: {name}");
                    }
                }
            }
            // The caption is the English beginning, in every language.
            assert_eq!(
                problem.caption(),
                format!("Skhid updater: {}", problem.kind(0))
            );
        }
        // Each problem, each way it failed, has a title of its own.
        for language in 0..5 {
            let mut titles: Vec<String> = all
                .iter()
                .map(|problem| problem.title(Some(language)))
                .collect();
            titles.sort();
            titles.dedup();
            // The two antivirus boxes share a title: the file is the object.
            assert_eq!(titles.len(), all.len() - 1, "{language}");
        }
        // The four beginnings differ from each other in every language.
        for language in 0..5 {
            let mut kinds: Vec<&str> = all.iter().map(|problem| problem.kind(language)).collect();
            kinds.sort();
            kinds.dedup();
            assert_eq!(kinds.len(), 4, "{kinds:?}");
        }
    }
}
