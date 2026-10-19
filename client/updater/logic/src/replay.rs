//! "Watch replay" (`updater_skhid.exe replay <id> <token> <sha256> <size>`):
//! what needs no Windows. The patched game starts the updater with the link
//! the server sent in the lobby (patch revision 14, `replay_link_026g.asm`):
//! the updater fetches `<site>/replays/<token>` into
//! `<game>\replays\srp_<id>.sur`, checks its size and SHA-256, and starts a
//! second game that plays it (`-match_replay=`).
//!
//! The arguments come from a server and are treated as hostile: each must
//! be exactly the form the game's patch lets through, or nothing is done.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::sha256::from_hex;

/// The largest replay the updater takes (the server's limit is the same).
pub const MAX_SIZE: u64 = 64 << 20;
/// How many replays are kept in the folder; older ones are deleted.
pub const KEEP: usize = 10;
/// The folder of the game (`<game>`, the one with `resources.db`) the
/// replays go into.
pub const FOLDER: &str = "replays";
/// The lock of a replay download, beside the updater: not the update's, so
/// that neither waits for the other.
pub const LOCK_FILE: &str = "updater_skhid.replay.lock";
/// What the launcher's PLAY starts, beside the updater.
pub const GAME_EXE: &str = "survarium_skhid.exe";
/// What the launcher's PLAY gives the game after its language (the
/// `arguments` value of `launcher_values_026g.asm`). The game exe has its
/// login server inside; a replay connects to none.
pub const GAME_ARGUMENTS: &str = "-mt_allocator=768 -minimum_resources_memory_size=768";
/// The game cuts a replay's name at 259 bytes (0x1F02EC: a 0x104-byte
/// buffer) and reads its command line as ANSI text.
pub const MAX_GAME_PATH: usize = 259;

/// The launcher's languages, by the number it keeps
/// (`HKCU\Software\Vostok Games\Survarium`, `language`): the game's
/// `-localization=` word.
pub const LANGUAGES: [&str; 5] = ["english", "russian", "ukrainian", "german", "polish"];

/// A replay link that passed every check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The match id, 8 lower-case hex digits.
    pub id: String,
    token: String,
    pub sha256: [u8; 32],
    pub size: u64,
}

/// Why the arguments were refused. It never holds the arguments themselves
/// (the token must not reach the log).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentError {
    /// Not four arguments after `replay`.
    Count(usize),
    Id,
    Token,
    Sha256,
    Size,
    /// A size of 0 or over `MAX_SIZE`.
    SizeOutOfRange,
}

/// The word after the replay's arguments that makes the run silent: no
/// message box, the problem in the log and the exit code alone (as
/// `check quiet`). The game never sends it: its patch passes the server's
/// text only when that text ends with the size's digits.
pub const QUIET: &str = "quiet";

/// The replay command in a whole command line (the program first): the
/// arguments after `replay` without a trailing `quiet`, and whether it was
/// there. `None` when the action is not exactly `replay`, as the game's
/// patch writes it.
pub fn command<S: AsRef<str>>(line: &[S]) -> Option<(&[S], bool)> {
    let rest = match line {
        [_, action, rest @ ..] if action.as_ref() == "replay" => rest,
        _ => return None,
    };
    Some(match rest {
        [arguments @ .., last] if last.as_ref() == QUIET => (arguments, true),
        _ => (rest, false),
    })
}

fn lower_hex(text: &str, length: usize) -> bool {
    text.len() == length && text.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

/// The arguments after `replay`: exactly `<8 hex> <32 hex> <64 hex>
/// <1..10 digits>`, hex in lower case, the size 1..=`MAX_SIZE`.
pub fn parse<S: AsRef<str>>(arguments: &[S]) -> Result<Request, ArgumentError> {
    let [id, token, sha256, size] = arguments else {
        return Err(ArgumentError::Count(arguments.len()));
    };
    let (id, token, sha256, size) = (id.as_ref(), token.as_ref(), sha256.as_ref(), size.as_ref());
    if !lower_hex(id, 8) {
        return Err(ArgumentError::Id);
    }
    if !lower_hex(token, 32) {
        return Err(ArgumentError::Token);
    }
    if !lower_hex(sha256, 64) {
        return Err(ArgumentError::Sha256);
    }
    if size.is_empty() || size.len() > 10 || !size.bytes().all(|c| c.is_ascii_digit()) {
        return Err(ArgumentError::Size);
    }
    let size: u64 = size.parse().map_err(|_| ArgumentError::Size)?;
    if size == 0 || size > MAX_SIZE {
        return Err(ArgumentError::SizeOutOfRange);
    }
    Ok(Request {
        id: id.into(),
        token: token.into(),
        sha256: from_hex(sha256.as_bytes()).ok_or(ArgumentError::Sha256)?,
        size,
    })
}

impl Request {
    /// `srp_<id>.sur`
    pub fn file_name(&self) -> String {
        format!("srp_{}.sur", self.id)
    }

    /// `srp_<id>.sur.part`: the download until it is checked.
    pub fn part_name(&self) -> String {
        format!("{}.part", self.file_name())
    }

    /// The path asked of the site.
    pub fn route(&self) -> String {
        format!("/replays/{}", self.token)
    }

    /// What the log may say of the token: its first 6 hex digits.
    pub fn token_hint(&self) -> &str {
        &self.token[..6]
    }
}

/// A name of the replays folder that the updater made: `srp_<8 lower hex>.sur`
/// or that with `.part`. Its id and whether it is a download left over.
pub fn replay_file(name: &str) -> Option<(&str, bool)> {
    let (rest, part) = match name.strip_suffix(".sur.part") {
        Some(rest) => (rest, true),
        None => (name.strip_suffix(".sur")?, false),
    };
    let id = rest.strip_prefix("srp_")?;
    lower_hex(id, 8).then_some((id, part))
}

/// What to delete of the folder's files (`(name, last written)`), so that
/// at most `keep` replays stay: the newest, `current` always among them.
/// Downloads left over (`.part`) of another id go too. A name that is not
/// the updater's own is never named.
pub fn to_delete(files: &[(String, u64)], current: &str, keep: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut replays: Vec<(&str, u64)> = Vec::new();
    for (name, time) in files {
        match replay_file(name) {
            Some((_, true)) => {
                if name.strip_suffix(".part") != Some(current) {
                    out.push(name.clone());
                }
            }
            Some((_, false)) if name != current => replays.push((name, *time)),
            _ => {}
        }
    }
    // Newest first; of the same time, the greater name first, so that the
    // choice does not depend on the folder's order.
    replays.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(a.0)));
    let room = keep.saturating_sub(usize::from(files.iter().any(|(name, _)| name == current)));
    out.extend(replays.into_iter().skip(room).map(|(name, _)| String::from(name)));
    out
}

/// The game's command line: the launcher's PLAY with `-match_replay=` after
/// it. The game reads a value in double quotes up to the next quote (its
/// command-line parser, 0xDF1E3), so a path with spaces is one value. `None`
/// for a path the game would read otherwise: one with a quote or a control
/// character, one that ends with a backslash, one too long, one that is
/// not a full path (the game takes a name without `:` or `\\` for a name in
/// its own replays folder).
pub fn game_command(game: &str, localization: &str, replay: &str) -> Option<String> {
    let bad = |text: &str| text.chars().any(|c| c == '"' || c.is_control());
    if bad(game)
        || bad(replay)
        || replay.ends_with('\\')
        || replay.len() > MAX_GAME_PATH
        || !(replay.starts_with("\\\\") || replay.get(1..3) == Some(":\\"))
        || !LANGUAGES.contains(&localization)
    {
        return None;
    }
    Some(format!(
        "\"{game}\" -localization={localization} {GAME_ARGUMENTS} -match_replay=\"{replay}\""
    ))
}

/// The game's `-localization=` word for the launcher's language number.
pub fn localization(language: Option<u32>) -> &'static str {
    LANGUAGES[language.filter(|&n| (n as usize) < LANGUAGES.len()).unwrap_or(0) as usize]
}

/// What went wrong, for the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    BadLink,
    AlreadyRunning,
    /// The site could not be reached (its host).
    Network(String),
    /// 404 or 410: the token is unknown, expired or used up.
    Expired,
    /// 429 or 503 (its host).
    Busy(String),
    /// Another status (the host, the status).
    Status(String, u32),
    /// Wrong size or SHA-256.
    Damaged,
    /// The replays folder or the file could not be written (Windows' words).
    Disk(String),
    NoGame,
    /// The game could not be started (Windows' words).
    Start(String),
    /// The replay's path has characters the game's ANSI command line loses.
    Path,
    /// The game's files are not in place (`updater_skhid.exe ready` says
    /// no: the last patch step did not end `ready`, or the
    /// `resources.db.patch_<n>` files are not what its list says), so the
    /// game is not started, as `play_skhid.sh` does not start it.
    NotReady,
}

/// The message for a player in the launcher's language (`LANGUAGES` order);
/// English for an unknown one.
pub fn message(language: Option<u32>, problem: &Problem) -> String {
    use Problem::*;
    let language = language.filter(|&n| n < 5).unwrap_or(0);
    match (language, problem) {
        (1, BadLink) => "Ссылка на повтор недействительна. Запросите повтор снова в Истории лобби.".into(),
        (2, BadLink) => "Посилання на повтор недійсне. Попросіть повтор знову в Історії лобі.".into(),
        (3, BadLink) => "Dieser Wiederholungslink ist ungültig. Fordere die Wiederholung im Verlauf der Lobby erneut an.".into(),
        (4, BadLink) => "Ten link do powtórki jest nieprawidłowy. Poproś o powtórkę ponownie w Historii w lobby.".into(),
        (_, BadLink) => "This replay link is not valid. Ask for the replay again in the lobby's History.".into(),
        (1, AlreadyRunning) => "Повтор уже загружается. Дождитесь окончания и попробуйте снова.".into(),
        (2, AlreadyRunning) => "Повтор уже завантажується. Дочекайтеся і спробуйте знову.".into(),
        (3, AlreadyRunning) => "Eine Wiederholung wird bereits heruntergeladen. Warte, bis sie fertig ist, und versuche es erneut.".into(),
        (4, AlreadyRunning) => "Powtórka jest już pobierana. Poczekaj na koniec i spróbuj ponownie.".into(),
        (_, AlreadyRunning) => "A replay is already being downloaded. Wait for it, then try again.".into(),
        (1, Network(host)) => format!("Не удаётся соединиться с {host}, чтобы загрузить повтор. Проверьте подключение к интернету и попробуйте снова."),
        (2, Network(host)) => format!("Не вдається з'єднатися з {host}, щоб завантажити повтор. Перевірте з'єднання з інтернетом і спробуйте знову."),
        (3, Network(host)) => format!("{host} ist nicht erreichbar, um die Wiederholung herunterzuladen. Prüfe die Internetverbindung und versuche es erneut."),
        (4, Network(host)) => format!("Nie można połączyć się z {host}, aby pobrać powtórkę. Sprawdź połączenie z internetem i spróbuj ponownie."),
        (_, Network(host)) => format!("Cannot reach {host} to download the replay. Check the internet connection and try again."),
        (1, Expired) => "Ссылка на повтор устарела или уже использована. Запросите повтор снова в Истории лобби.".into(),
        (2, Expired) => "Посилання на повтор застаріло або вже використане. Попросіть повтор знову в Історії лобі.".into(),
        (3, Expired) => "Der Wiederholungslink ist abgelaufen oder wurde bereits benutzt. Fordere die Wiederholung im Verlauf der Lobby erneut an.".into(),
        (4, Expired) => "Link do powtórki wygasł lub został już użyty. Poproś o powtórkę ponownie w Historii w lobby.".into(),
        (_, Expired) => "The replay link has expired or was already used. Ask for the replay again in the lobby's History.".into(),
        (1, Busy(host)) => format!("{host} занят. Попробуйте через минуту."),
        (2, Busy(host)) => format!("{host} зайнятий. Спробуйте за хвилину."),
        (3, Busy(host)) => format!("{host} ist ausgelastet. Versuche es in einer Minute erneut."),
        (4, Busy(host)) => format!("{host} jest zajęty. Spróbuj ponownie za minutę."),
        (_, Busy(host)) => format!("{host} is busy. Try again in a minute."),
        (1, Status(host, code)) => format!("{host} ответил на загрузку повтора ошибкой {code}. Попробуйте позже."),
        (2, Status(host, code)) => format!("{host} відповів на завантаження повтору помилкою {code}. Спробуйте пізніше."),
        (3, Status(host, code)) => format!("{host} hat den Download der Wiederholung mit Fehler {code} beantwortet. Versuche es später erneut."),
        (4, Status(host, code)) => format!("{host} odpowiedział na pobieranie powtórki błędem {code}. Spróbuj później."),
        (_, Status(host, code)) => format!("{host} answered the replay download with error {code}. Try again later."),
        (1, Damaged) => "Загруженный повтор повреждён (неверный размер или контрольная сумма). Попробуйте снова.".into(),
        (2, Damaged) => "Завантажений повтор пошкоджений (неправильний розмір або контрольна сума). Спробуйте знову.".into(),
        (3, Damaged) => "Die heruntergeladene Wiederholung ist beschädigt (falsche Größe oder Prüfsumme). Versuche es erneut.".into(),
        (4, Damaged) => "Pobrana powtórka jest uszkodzona (zły rozmiar lub suma kontrolna). Spróbuj ponownie.".into(),
        (_, Damaged) => "The downloaded replay is damaged (wrong size or checksum). Try again.".into(),
        (1, Disk(detail)) => format!("Не удаётся сохранить повтор в папке replays игры: {detail}."),
        (2, Disk(detail)) => format!("Не вдається зберегти повтор у теці replays гри: {detail}."),
        (3, Disk(detail)) => format!("Die Wiederholung kann nicht im Ordner replays des Spiels gespeichert werden: {detail}."),
        (4, Disk(detail)) => format!("Nie można zapisać powtórki w folderze replays gry: {detail}."),
        (_, Disk(detail)) => format!("Cannot save the replay in the game's replays folder: {detail}."),
        (1, NoGame) => "survarium_skhid.exe не найден рядом с программой обновления. Запустите игру один раз через лаунчер, он его установит.".into(),
        (2, NoGame) => "survarium_skhid.exe не знайдено поруч з оновлювачем. Запустіть гру один раз через лаунчер, він його встановить.".into(),
        (3, NoGame) => "survarium_skhid.exe wurde neben dem Updater nicht gefunden. Starte das Spiel einmal mit dem Launcher, der es installiert.".into(),
        (4, NoGame) => "Nie znaleziono survarium_skhid.exe obok aktualizatora. Uruchom grę raz przez launcher, który go zainstaluje.".into(),
        (_, NoGame) => "survarium_skhid.exe was not found beside the updater. Start the game once with the launcher, which installs it.".into(),
        (1, Start(detail)) => format!("Повтор загружен, но игру не удалось запустить: {detail}."),
        (2, Start(detail)) => format!("Повтор завантажено, але гру не вдалося запустити: {detail}."),
        (3, Start(detail)) => format!("Die Wiederholung ist heruntergeladen, aber das Spiel konnte nicht gestartet werden: {detail}."),
        (4, Start(detail)) => format!("Powtórka jest pobrana, ale nie udało się uruchomić gry: {detail}."),
        (_, Start(detail)) => format!("The replay is downloaded but the game could not be started: {detail}."),
        (1, Path) => "Путь к папке игры содержит символы, которые игра не читает. Переместите игру в папку, путь к которой состоит только из латинских букв.".into(),
        (2, Path) => "Шлях до теки гри містить символи, яких гра не читає. Перемістіть гру в теку, шлях до якої має лише латинські літери.".into(),
        (3, Path) => "Der Pfad des Spielordners enthält Zeichen, die das Spiel nicht lesen kann. Verschiebe das Spiel in einen Ordner, dessen Pfad nur lateinische Buchstaben hat.".into(),
        (4, Path) => "Ścieżka folderu gry zawiera znaki, których gra nie odczyta. Przenieś grę do folderu, którego ścieżka ma tylko litery łacińskie.".into(),
        (_, Path) => "The game folder's path has characters the game cannot read. Move the game to a folder whose path has Latin letters only.".into(),
        (1, NotReady) => "Skhid ещё не закончил обновлять файлы игры (game\\resources.db.patch_*), поэтому повтор не запущен. Запустите game\\binaries\\x86\\launcher_skhid.exe (лаунчер): он закончит обновление или скажет, что мешает. Затем запросите повтор снова в Истории лобби. Подробности: game\\binaries\\x86\\updater_skhid.log.".into(),
        (2, NotReady) => "Skhid ще не завершив оновлення файлів гри (game\\resources.db.patch_*), тому повтор не запущено. Запустіть game\\binaries\\x86\\launcher_skhid.exe (лаунчер): він завершить оновлення або скаже, що заважає. Потім попросіть повтор знову в Історії лобі. Подробиці: game\\binaries\\x86\\updater_skhid.log.".into(),
        (3, NotReady) => "Skhid hat die Dateien des Spiels (game\\resources.db.patch_*) noch nicht fertig aktualisiert, daher wird die Wiederholung nicht gestartet. Starte game\\binaries\\x86\\launcher_skhid.exe (den Launcher): Er schließt die Aktualisierung ab oder sagt, was im Weg ist. Fordere die Wiederholung danach im Verlauf der Lobby erneut an. Details: game\\binaries\\x86\\updater_skhid.log.".into(),
        (4, NotReady) => "Skhid nie zakończył jeszcze aktualizacji plików gry (game\\resources.db.patch_*), więc powtórka nie została uruchomiona. Uruchom game\\binaries\\x86\\launcher_skhid.exe (launcher): dokończy aktualizację albo powie, co przeszkadza. Potem poproś o powtórkę ponownie w Historii w lobby. Szczegóły: game\\binaries\\x86\\updater_skhid.log.".into(),
        (_, NotReady) => "Skhid has not finished updating the game's files (game\\resources.db.patch_*), so the replay is not started. Start game\\binaries\\x86\\launcher_skhid.exe (the launcher): it finishes the update, or says what is in the way. Then ask for the replay again in the lobby's History. Details: game\\binaries\\x86\\updater_skhid.log.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    const ID: &str = "0a1b2c3d";
    const TOKEN: &str = "0123456789abcdef0123456789abcdef";
    const SHA: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

    fn args(id: &str, token: &str, sha: &str, size: &str) -> Vec<String> {
        vec![id.into(), token.into(), sha.into(), size.into()]
    }

    #[test]
    fn a_replay_link_is_read() {
        let request = parse(&args(ID, TOKEN, SHA, "1048576")).unwrap();
        assert_eq!(request.id, ID);
        assert_eq!(request.size, 1 << 20);
        assert_eq!(request.sha256[0], 0xfe);
        assert_eq!(request.file_name(), "srp_0a1b2c3d.sur");
        assert_eq!(request.part_name(), "srp_0a1b2c3d.sur.part");
        assert_eq!(request.route(), format!("/replays/{TOKEN}"));
        assert_eq!(request.token_hint(), "012345");
        for size in ["1", "0000000001", "67108864"] {
            assert!(parse(&args(ID, TOKEN, SHA, size)).is_ok(), "{size}");
        }
    }

    #[test]
    fn the_command_is_exactly_replay_and_quiet_is_the_last_word_only() {
        let line = |words: &[&str]| -> Vec<String> { words.iter().map(|w| String::from(*w)).collect() };
        let full = line(&["u.exe", "replay", ID, TOKEN, SHA, "1"]);
        let (arguments, quiet) = command(&full).unwrap();
        assert_eq!((arguments.len(), quiet), (4, false));
        let silent = line(&["u.exe", "replay", ID, TOKEN, SHA, "1", "quiet"]);
        let (arguments, quiet) = command(&silent).unwrap();
        assert_eq!((arguments, quiet), (&silent[2..6], true));
        assert!(parse(arguments).is_ok());
        // Only the last word, and only exactly that word.
        for words in [&["u.exe", "replay", "quiet", ID, TOKEN, SHA, "1"][..], &["u.exe", "replay", ID, TOKEN, SHA, "1", "Quiet"]] {
            let words = line(words);
            let (arguments, quiet) = command(&words).unwrap();
            assert!(!quiet && parse(arguments).is_err(), "{words:?}");
        }
        let alone = line(&["u.exe", "replay", "quiet"]);
        let (arguments, quiet) = command(&alone).unwrap();
        assert!(quiet && arguments.is_empty());
        for words in [&["u.exe"][..], &[], &["u.exe", "Replay", ID], &["u.exe", "REPLAY"], &["u.exe", "replay "],
                      &["u.exe", "check", "replay"], &["replay", ID, TOKEN, SHA, "1"]] {
            assert!(command(&line(words)).is_none(), "{words:?}");
        }
        // The program's own name is never taken for the action.
        assert_eq!(command(&line(&["replay", "replay"])).map(|(a, q)| (a.len(), q)), Some((0, false)));
    }

    #[test]
    fn hostile_arguments_are_refused() {
        use ArgumentError::*;
        let cases: Vec<(Vec<String>, ArgumentError)> = vec![
            (vec![], Count(0)),
            (args(ID, TOKEN, SHA, "1")[..3].to_vec(), Count(3)),
            ([args(ID, TOKEN, SHA, "1"), vec!["x".into()]].concat(), Count(5)),
            (args("0a1b2c3", TOKEN, SHA, "1"), Id),
            (args("0a1b2c3d0", TOKEN, SHA, "1"), Id),
            (args("0A1B2C3D", TOKEN, SHA, "1"), Id),
            (args("0a1b2c3g", TOKEN, SHA, "1"), Id),
            (args("..\\..\\x", TOKEN, SHA, "1"), Id),
            (args("0a1b/c3d", TOKEN, SHA, "1"), Id),
            (args("0a1b2c3\u{e9}", TOKEN, SHA, "1"), Id),
            (args(ID, &TOKEN[1..], SHA, "1"), Token),
            (args(ID, &(TOKEN.to_string() + "0"), SHA, "1"), Token),
            (args(ID, &TOKEN.to_uppercase(), SHA, "1"), Token),
            (args(ID, "0123456789abcdef0123456789abcde%", SHA, "1"), Token),
            (args(ID, "0123456789abcdef0123456789abc?=&", SHA, "1"), Token),
            (args(ID, TOKEN, &SHA[1..], "1"), Sha256),
            (args(ID, TOKEN, &(SHA.to_string() + "f"), "1"), Sha256),
            (args(ID, TOKEN, &SHA.to_uppercase(), "1"), Sha256),
            (args(ID, TOKEN, SHA, ""), Size),
            (args(ID, TOKEN, SHA, "-1"), Size),
            (args(ID, TOKEN, SHA, "+1"), Size),
            (args(ID, TOKEN, SHA, " 1"), Size),
            (args(ID, TOKEN, SHA, "1e6"), Size),
            (args(ID, TOKEN, SHA, "0x10"), Size),
            (args(ID, TOKEN, SHA, "12345678901"), Size),
            (args(ID, TOKEN, SHA, "\u{661}"), Size),
            (args(ID, TOKEN, SHA, "0"), SizeOutOfRange),
            (args(ID, TOKEN, SHA, "67108865"), SizeOutOfRange),
            (args(ID, TOKEN, SHA, "9999999999"), SizeOutOfRange),
        ];
        for (arguments, expected) in cases {
            assert_eq!(parse(&arguments), Err(expected), "{arguments:?}");
        }
        // The error says which field, never what it held.
        assert!(!format!("{:?}", parse(&args(ID, "x", SHA, "1"))).contains('x'));
    }

    #[test]
    fn only_its_own_names_are_replays() {
        assert_eq!(replay_file("srp_0a1b2c3d.sur"), Some(("0a1b2c3d", false)));
        assert_eq!(replay_file("srp_0a1b2c3d.sur.part"), Some(("0a1b2c3d", true)));
        for name in [
            "srp_0a1b2c3d.SUR", "SRP_0a1b2c3d.sur", "srp_0A1B2C3D.sur", "srp_0a1b2c3.sur",
            "srp_0a1b2c3d0.sur", "srp_0a1b2c3d.sur.bak", "srp_0a1b2c3d.part", "x_srp_0a1b2c3d.sur",
            "srp_0a1b2c3d .sur", "srp_.sur", "match.sur", "notes.txt", "srp_0a1b2c3d.sur.part.part",
        ] {
            assert_eq!(replay_file(name), None, "{name}");
        }
    }

    fn found(names: &[(&str, u64)]) -> Vec<(String, u64)> {
        names.iter().map(|(name, time)| (String::from(*name), *time)).collect()
    }

    #[test]
    fn the_newest_ten_are_kept_and_only_the_updaters_own_files_go() {
        let mut files: Vec<(String, u64)> = (0..14_u64)
            .map(|n| (format!("srp_{n:08x}.sur"), 100 + n))
            .collect();
        files.extend(found(&[
            ("notes.txt", 1), ("SRP_00000001.SUR", 1), ("srp_zz.sur", 1), ("my.sur", 1),
            ("srp_000000ff.sur.part", 5), ("srp_00000000.sur.part", 7),
        ]));
        // The current one is the oldest: it stays, and nine others with it.
        let mut gone = to_delete(&files, "srp_00000000.sur", KEEP);
        gone.sort();
        let mut expected: Vec<String> = (1..5_u64).map(|n| format!("srp_{n:08x}.sur")).collect();
        expected.push("srp_000000ff.sur.part".into());
        expected.sort();
        assert_eq!(gone, expected);
        // The current one is the newest: the ten newest stay.
        let mut gone = to_delete(&files, "srp_0000000d.sur", KEEP);
        gone.sort();
        let mut expected: Vec<String> = (0..4_u64).map(|n| format!("srp_{n:08x}.sur")).collect();
        expected.extend(["srp_00000000.sur.part".into(), "srp_000000ff.sur.part".into()]);
        expected.sort();
        assert_eq!(gone, expected);
        // Ten or fewer: nothing but stray downloads.
        assert_eq!(to_delete(&found(&[("srp_00000001.sur", 1), ("a.txt", 0)]), "srp_00000002.sur", KEEP),
                   Vec::<String>::new());
        // The same time: the choice does not depend on the order found.
        let same: Vec<(String, u64)> = (0..12_u64).map(|n| (format!("srp_{n:08x}.sur"), 7)).collect();
        let mut reversed = same.clone();
        reversed.reverse();
        assert_eq!(to_delete(&same, "srp_00000005.sur", KEEP).len(), 2);
        let (mut a, mut b) = (to_delete(&same, "srp_00000005.sur", KEEP), to_delete(&reversed, "srp_00000005.sur", KEEP));
        a.sort();
        b.sort();
        assert_eq!(a, b);
    }

    #[test]
    fn the_game_is_started_as_play_does_with_the_replay_after() {
        let game = r"D:\My Games\Survarium\game\binaries\x86\survarium_skhid.exe";
        let replay = r"D:\My Games\Survarium\game\replays\srp_0a1b2c3d.sur";
        assert_eq!(
            game_command(game, "ukrainian", replay).unwrap(),
            format!(
                "\"{game}\" -localization=ukrainian -mt_allocator=768 -minimum_resources_memory_size=768 \
                 -match_replay=\"{replay}\""
            )
        );
        assert!(game_command(game, "english", r"\\server\share\game\replays\srp_0a1b2c3d.sur").is_some());
        assert!(game_command(game, "english", r"D:\GAMES~1\SURVAR~1\game\replays\SRP_0A~1.SUR").is_some());
        for replay in [
            "D:\\a\"b\\srp_0a1b2c3d.sur", "D:\\a\\b\\", "srp_0a1b2c3d", "replays\\srp_0a1b2c3d.sur",
            "D:\\a\tb\\x.sur", "D:\\a\nb\\x.sur",
        ] {
            assert_eq!(game_command(game, "english", replay), None, "{replay}");
        }
        let long = format!("C:\\{}", "d".repeat(MAX_GAME_PATH - 3));
        assert!(game_command(game, "english", &long).is_some());
        assert!(game_command(game, "english", &(long + "d")).is_none());
        assert!(game_command("C:\\a\"b.exe", "english", replay).is_none());
        assert!(game_command(game, "klingon", replay).is_none());
    }

    #[test]
    fn the_language_is_the_launchers() {
        assert_eq!(localization(None), "english");
        assert_eq!(localization(Some(2)), "ukrainian");
        assert_eq!(localization(Some(4)), "polish");
        assert_eq!(localization(Some(5)), "english");
        assert_eq!(localization(Some(u32::MAX)), "english");
        let problems = [
            Problem::BadLink, Problem::AlreadyRunning, Problem::Network("h.example".into()), Problem::Expired,
            Problem::Busy("h.example".into()), Problem::Status("h.example".into(), 302), Problem::Damaged,
            Problem::Disk("Access is denied".into()), Problem::NoGame, Problem::Start("Access is denied".into()),
            Problem::Path, Problem::NotReady,
        ];
        for problem in &problems {
            let english = message(None, problem);
            assert_eq!(message(Some(0), problem), english);
            assert_eq!(message(Some(9), problem), english);
            for language in 1..5 {
                let text = message(Some(language), problem);
                assert!(!text.is_empty() && text != english, "{language} {problem:?}");
                for part in ["h.example", "302", "Access is denied"] {
                    assert_eq!(text.contains(part), english.contains(part), "{language} {problem:?} {part}");
                }
            }
        }
    }

    #[test]
    fn a_game_whose_files_are_not_updated_is_sent_to_the_launcher_in_every_language() {
        // As `updater_skhid.exe ready` tells a player: the
        // file, the launcher that finishes the update, and the log.
        for language in [None, Some(1), Some(2), Some(3), Some(4)] {
            let text = message(language, &Problem::NotReady);
            for part in ["game\\resources.db", "launcher_skhid.exe", "game\\binaries\\x86\\updater_skhid.log"] {
                assert!(text.contains(part), "{language:?}: {part} in {text}");
            }
            assert!(!text.contains("\\\\"), "{text}");
        }
    }
}
