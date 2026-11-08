//! "Save replay" (`updater_skhid.exe replay <id> <token> <sha256> <size>`):
//! what needs no Windows. The patched game starts the updater with the link
//! the server sent in the lobby (patch revision 14, `replay_link_026g.asm`):
//! the updater fetches `<site>/replays/<token>` into
//! `<game>\replays\srp_<id>.srpreplay.part`, checks its size and SHA-256,
//! reads the file's header ([`Header`]) and names it
//! `<date>_<time>_<map>_<id>.srpreplay` ([`saved_name`]). Saved replays are
//! the player's: never deleted here.
//!
//! The arguments and the file come from a server and are treated as
//! hostile: each must be exactly the form expected, or nothing is done.

use alloc::format;
use alloc::string::String;

use crate::sha256::from_hex;

/// The largest replay the updater takes (the server's file cap).
pub const MAX_SIZE: u64 = 12 << 20;
/// The folder of the game (`<game>`, the one with `resources.db`) the
/// replays go into.
pub const FOLDER: &str = "replays";
/// The lock of a replay download, beside the updater: not the update's, so
/// that neither waits for the other.
pub const LOCK_FILE: &str = "updater_skhid.replay.lock";
/// A saved replay's extension.
pub const EXTENSION: &str = ".srpreplay";
/// The file's first bytes, then its header's length (u32 LE).
pub const MAGIC: &[u8; 8] = b"SRPRPL01";
/// The longest header a file may have.
pub const MAX_HEADER: usize = 8192;
/// The longest map name in a file's name.
const MAX_SLUG: usize = 32;
/// The most names tried for one replay (`_2` ... `_99`).
pub const MAX_COPIES: u32 = 99;

/// The languages the messages are written in, by their number. The language
/// is the game's: user.cfg's `localization <name>` line, which the game's
/// Settings and the launcher's menu write ([`language_of_user_cfg`]).
pub const LANGUAGES: [&str; 5] = ["english", "russian", "ukrainian", "german", "polish"];

/// The number in [`LANGUAGES`] of the language of a user.cfg: its first
/// `localization <name>` line, as the launcher reads it. None without one, or
/// for a language the messages are not written in (they are then English).
pub fn language_of_user_cfg(text: &[u8]) -> Option<u32> {
    let line = text.split(|&byte| byte == b'\n').find_map(|line| {
        let rest = line.strip_prefix(b"localization")?;
        rest.first().filter(|&&byte| byte == b' ' || byte == b'\t')?;
        Some(rest)
    })?;
    let name = line.split(|&byte| byte <= b' ').find(|word| !word.is_empty())?;
    LANGUAGES.iter().position(|language| language.as_bytes() == name).map(|at| at as u32)
}

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
/// message box, the outcome in the log and the exit code alone (as
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

/// 1 to `max` ASCII digits, as a number.
fn digits(text: &str, max: usize) -> Option<u64> {
    (!text.is_empty() && text.len() <= max && text.bytes().all(|c| c.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
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
    let size = digits(size, 10).ok_or(ArgumentError::Size)?;
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
    /// `srp_<id>.srpreplay.part`: the download until it is checked and named.
    pub fn part_name(&self) -> String {
        format!("srp_{}{EXTENSION}.part", self.id)
    }

    /// The path asked of the site.
    pub fn route(&self) -> String {
        format!("/replays/{}", self.token)
    }

    /// What the log may say of the token: its first 6 hex digits.
    pub fn token_hint(&self) -> &str {
        &self.token[..6]
    }

    /// Whether `name` (of the replays folder) is a replay saved of this
    /// match: `<anything>_<id>.srpreplay` or `<anything>_<id>_<n>.srpreplay`.
    pub fn is_saved_name(&self, name: &str) -> bool {
        let Some(stem) = name.strip_suffix(EXTENSION) else { return false };
        let stem = match stem.rsplit_once('_') {
            Some((before, n)) if digits(n, 2).is_some() => before,
            _ => stem,
        };
        stem.strip_suffix(self.id.as_str()).is_some_and(|before| before.len() > 1 && before.ends_with('_'))
    }
}

/// A download of another match left over: `srp_<8 lower hex>.srpreplay.part`
/// of an id that is not `current`. A name that is not the updater's own is
/// never named.
pub fn is_stale_part(name: &str, current: &str) -> bool {
    name.strip_prefix("srp_")
        .and_then(|rest| rest.strip_suffix(".srpreplay.part"))
        .is_some_and(|id| lower_hex(id, 8) && id != current)
}

/// What the updater reads of a replay file's header: the file starts with
/// [`MAGIC`], a u32 LE length (at most [`MAX_HEADER`]) and that many bytes
/// of UTF-8 lines `key value\n`. Other keys are the server's and the game's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// `match`: 8 lower-case hex digits.
    pub match_id: String,
    /// `made`: when the match started, Unix seconds (UTC).
    pub made: u64,
    /// `map`: the map's `maps_dict` id.
    pub map: u32,
    /// `map_slug`, if the file has one.
    pub map_slug: Option<String>,
}

/// Why a file's header was refused (the file is then taken as damaged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderError {
    Magic,
    Length,
    /// Not UTF-8, a control byte other than the line ends, a line with no
    /// space, a key twice.
    Text,
    /// `match`, `made` or `map` missing or not their form.
    Field(&'static str),
}

/// The bytes of a file to read for [`header_length`], then the header.
pub const PREFIX: usize = MAGIC.len() + 4;

/// The header's length from a file's first [`PREFIX`] bytes.
pub fn header_length(prefix: &[u8]) -> Result<usize, HeaderError> {
    let (magic, rest) = prefix.split_at_checked(MAGIC.len()).ok_or(HeaderError::Magic)?;
    if magic != MAGIC {
        return Err(HeaderError::Magic);
    }
    let length = rest.first_chunk::<4>().map(|bytes| u32::from_le_bytes(*bytes) as usize).ok_or(HeaderError::Length)?;
    if length == 0 || length > MAX_HEADER {
        return Err(HeaderError::Length);
    }
    Ok(length)
}

/// The header's text (the bytes after the length).
pub fn parse_header(text: &[u8]) -> Result<Header, HeaderError> {
    let text = core::str::from_utf8(text).map_err(|_| HeaderError::Text)?;
    if text.chars().any(|c| c.is_control() && c != '\n') {
        return Err(HeaderError::Text);
    }
    let (mut match_id, mut made, mut map, mut map_slug) = (None, None, None, None);
    for line in text.split('\n').filter(|line| !line.is_empty()) {
        let (key, value) = line.split_once(' ').ok_or(HeaderError::Text)?;
        let slot = match key {
            "match" => &mut match_id,
            "made" => &mut made,
            "map" => &mut map,
            "map_slug" => &mut map_slug,
            _ => continue,
        };
        if slot.replace(value).is_some() {
            return Err(HeaderError::Text);
        }
    }
    let match_id = match_id.filter(|id| lower_hex(id, 8)).ok_or(HeaderError::Field("match"))?;
    Ok(Header {
        match_id: match_id.into(),
        made: made.and_then(|made| digits(made, 10)).ok_or(HeaderError::Field("made"))?,
        map: map.and_then(|map| digits(map, 5)).ok_or(HeaderError::Field("map"))? as u32,
        map_slug: map_slug.map(String::from),
    })
}

/// A local date and time, to the minute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
}

/// The map's part of a file name: the header's `map_slug` kept to
/// `[A-Za-z0-9-]`, at most 32 of them; else `map<id>`.
pub fn slug(header: &Header) -> String {
    let kept: String = header
        .map_slug
        .as_deref()
        .unwrap_or("")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(MAX_SLUG)
        .collect();
    if kept.trim_matches('-').is_empty() { format!("map{}", header.map) } else { kept }
}

/// A saved replay's name: `<YYYY-MM-DD>_<HHMM>_<map>_<id>.srpreplay`, the
/// match's start in local time; `copy` 2 and up adds `_<copy>` (another
/// replay already has the name).
pub fn saved_name(header: &Header, start: Stamp, copy: u32) -> String {
    let Stamp { year, month, day, hour, minute } = start;
    let copy = if copy > 1 { format!("_{copy}") } else { String::new() };
    format!("{year:04}-{month:02}-{day:02}_{hour:02}{minute:02}_{}_{}{copy}{EXTENSION}", slug(header), header.match_id)
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
    /// Wrong size or SHA-256, or not a replay file.
    Damaged,
    /// The replays folder or the file could not be written (Windows' words).
    Disk(String),
}

/// The message for a player in the game's language (`LANGUAGES` order);
/// English for another one.
pub fn message(language: Option<u32>, problem: &Problem) -> String {
    use Problem::*;
    let language = language.filter(|&n| n < 5).unwrap_or(0);
    match (language, problem) {
        (1, BadLink) => "Ссылка на повтор недействительна. Сохраните повтор снова в лобби.".into(),
        (2, BadLink) => "Посилання на повтор недійсне. Збережіть повтор знову в лобі.".into(),
        (3, BadLink) => "Dieser Wiederholungslink ist ungültig. Speichere die Wiederholung erneut in der Lobby.".into(),
        (4, BadLink) => "Ten link do powtórki jest nieprawidłowy. Zapisz powtórkę ponownie w lobby.".into(),
        (_, BadLink) => "This replay link is not valid. Save the replay again in the lobby.".into(),
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
        (1, Expired) => "Ссылка на повтор устарела или уже использована. Сохраните повтор снова в лобби.".into(),
        (2, Expired) => "Посилання на повтор застаріло або вже використане. Збережіть повтор знову в лобі.".into(),
        (3, Expired) => "Der Wiederholungslink ist abgelaufen oder wurde bereits benutzt. Speichere die Wiederholung erneut in der Lobby.".into(),
        (4, Expired) => "Link do powtórki wygasł lub został już użyty. Zapisz powtórkę ponownie w lobby.".into(),
        (_, Expired) => "The replay link has expired or was already used. Save the replay again in the lobby.".into(),
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
    }
}

/// The message of a replay saved (or saved before) at `path`, in the game's
/// language.
pub fn saved_message(language: Option<u32>, path: &str) -> String {
    match language.filter(|&n| n < 5).unwrap_or(0) {
        1 => format!("Повтор сохранён:\n{path}"),
        2 => format!("Повтор збережено:\n{path}"),
        3 => format!("Die Wiederholung ist gespeichert:\n{path}"),
        4 => format!("Powtórka została zapisana:\n{path}"),
        _ => format!("The replay is saved:\n{path}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;
    use alloc::vec::Vec;

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
        assert_eq!(request.part_name(), "srp_0a1b2c3d.srpreplay.part");
        assert_eq!(request.route(), format!("/replays/{TOKEN}"));
        assert_eq!(request.token_hint(), "012345");
        for size in ["1", "0000000001", "12582912"] {
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
        for words in [&["u.exe"][..], &[], &["u.exe", "Replay", ID], &["u.exe", "REPLAY"], &["u.exe", "replay "],
                      &["u.exe", "check", "replay"], &["replay", ID, TOKEN, SHA, "1"]] {
            assert!(command(&line(words)).is_none(), "{words:?}");
        }
    }

    #[test]
    fn hostile_arguments_are_refused() {
        use ArgumentError::*;
        let cases: Vec<(Vec<String>, ArgumentError)> = vec![
            (vec![], Count(0)),
            (args(ID, TOKEN, SHA, "1")[..3].to_vec(), Count(3)),
            ([args(ID, TOKEN, SHA, "1"), vec!["x".into()]].concat(), Count(5)),
            (args("0a1b2c3", TOKEN, SHA, "1"), Id),
            (args("0A1B2C3D", TOKEN, SHA, "1"), Id),
            (args("..\\..\\x", TOKEN, SHA, "1"), Id),
            (args("0a1b2c3\u{e9}", TOKEN, SHA, "1"), Id),
            (args(ID, &TOKEN[1..], SHA, "1"), Token),
            (args(ID, &TOKEN.to_uppercase(), SHA, "1"), Token),
            (args(ID, "0123456789abcdef0123456789abc?=&", SHA, "1"), Token),
            (args(ID, TOKEN, &SHA[1..], "1"), Sha256),
            (args(ID, TOKEN, &SHA.to_uppercase(), "1"), Sha256),
            (args(ID, TOKEN, SHA, ""), Size),
            (args(ID, TOKEN, SHA, "+1"), Size),
            (args(ID, TOKEN, SHA, " 1"), Size),
            (args(ID, TOKEN, SHA, "0x10"), Size),
            (args(ID, TOKEN, SHA, "12345678901"), Size),
            (args(ID, TOKEN, SHA, "\u{661}"), Size),
            (args(ID, TOKEN, SHA, "0"), SizeOutOfRange),
            // One byte over the 12 MiB cap.
            (args(ID, TOKEN, SHA, "12582913"), SizeOutOfRange),
            (args(ID, TOKEN, SHA, "9999999999"), SizeOutOfRange),
        ];
        for (arguments, expected) in cases {
            assert_eq!(parse(&arguments), Err(expected), "{arguments:?}");
        }
        // The error says which field, never what it held.
        assert!(!format!("{:?}", parse(&args(ID, "x", SHA, "1"))).contains('x'));
    }

    fn file(header: &str) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        out.extend((header.len() as u32).to_le_bytes());
        out.extend(header.as_bytes());
        out.extend(b"\x00\x01events and the replay");
        out
    }

    fn read(bytes: &[u8]) -> Result<Header, HeaderError> {
        let length = header_length(&bytes[..PREFIX.min(bytes.len())])?;
        parse_header(bytes.get(PREFIX..PREFIX + length).ok_or(HeaderError::Length)?)
    }

    const GOOD: &str = "format 1\nmatch 1ce45cbe\nmade 1794174000\nlength 912\nmap 24\nmap_slug Rudnya\nmode 4\n\
                        score 87 100\nplace 0 0 4 7 Some One\n";

    #[test]
    fn the_header_gives_the_match_its_start_and_its_map() {
        let header = read(&file(GOOD)).unwrap();
        assert_eq!(header, Header { match_id: "1ce45cbe".into(), made: 1_794_174_000, map: 24, map_slug: Some("Rudnya".into()) });
        // No slug: the map's number names it.
        let header = read(&file("match 1ce45cbe\nmade 0\nmap 7")).unwrap();
        assert_eq!((header.map_slug, header.map), (None, 7));
    }

    #[test]
    fn a_hostile_header_is_refused() {
        use HeaderError::*;
        let mut long = file(GOOD);
        long[8..12].copy_from_slice(&(MAX_HEADER as u32 + 1).to_le_bytes());
        let mut zero = file(GOOD);
        zero[8..12].copy_from_slice(&0u32.to_le_bytes());
        let mut past = file(GOOD);
        past[8..12].copy_from_slice(&4000u32.to_le_bytes());
        let mut magic = file(GOOD);
        magic[7] = b'2';
        for (bytes, error) in [
            (magic, Magic), (b"SRPR".to_vec(), Magic), (MAGIC.to_vec(), Length), (long, Length), (zero, Length),
            (past, Length),
            (file("match 1ce45cbe\nmade 1\nmap 1\nmap_slug a\rb\n"), Text),
            (file("match 1ce45cbe\nmade 1\nmap 1\x00\n"), Text),
            (file("match 1ce45cbe\nmade 1\nmapless\n"), Text),
            (file("match 1ce45cbe\nmade 1\nmap 1\nmap 2\n"), Text),
            (file("match 1CE45CBE\nmade 1\nmap 1\n"), Field("match")),
            (file("match ../x\nmade 1\nmap 1\n"), Field("match")),
            (file("made 1\nmap 1\n"), Field("match")),
            (file("match 1ce45cbe\nmade -1\nmap 1\n"), Field("made")),
            (file("match 1ce45cbe\nmade 12345678901\nmap 1\n"), Field("made")),
            (file("match 1ce45cbe\nmade 1\nmap x\n"), Field("map")),
            (file("match 1ce45cbe\nmade 1\n"), Field("map")),
        ] {
            assert_eq!(read(&bytes), Err(error), "{bytes:?}");
        }
        let mut bad_utf8 = file("match 1ce45cbe\nmade 1\nmap 1\nmap_slug \u{e9}\n");
        let at = bad_utf8.iter().position(|&b| b == 0xC3).unwrap();
        bad_utf8[at] = 0xFF;
        assert_eq!(read(&bad_utf8), Err(Text));
    }

    #[test]
    fn a_saved_name_is_the_start_the_map_and_the_match() {
        let start = Stamp { year: 2026, month: 11, day: 8, hour: 21, minute: 40 };
        let header = |slug: Option<&str>| Header { match_id: "1ce45cbe".into(), made: 0, map: 24, map_slug: slug.map(String::from) };
        assert_eq!(saved_name(&header(Some("Rudnya")), start, 1), "2026-11-08_2140_Rudnya_1ce45cbe.srpreplay");
        assert_eq!(saved_name(&header(Some("Rudnya")), start, 2), "2026-11-08_2140_Rudnya_1ce45cbe_2.srpreplay");
        let early = Stamp { year: 999, month: 1, day: 2, hour: 3, minute: 4 };
        assert_eq!(saved_name(&header(Some("Vostok-Radar-Station")), early, 1),
                   "0999-01-02_0304_Vostok-Radar-Station_1ce45cbe.srpreplay");
        // Only [A-Za-z0-9-], 32 at most; nothing left: the map's number.
        for (given, slug) in [
            (Some("..\\..\\Windows"), "Windows"), (Some("C:/a b\"c<>|?*"), "Cabc"), (Some("Рудня"), "map24"),
            (Some("---"), "map24"), (None, "map24"), (Some(&"a".repeat(40)[..]), &"a".repeat(32)[..]),
        ] {
            assert_eq!(super::slug(&header(given)), slug, "{given:?}");
        }
    }

    #[test]
    fn only_this_matchs_saved_files_and_other_matches_parts_are_named() {
        let request = parse(&args("1ce45cbe", TOKEN, SHA, "1")).unwrap();
        for name in ["2026-11-08_2140_Rudnya_1ce45cbe.srpreplay", "x_1ce45cbe_2.srpreplay", "a_1ce45cbe_99.srpreplay"] {
            assert!(request.is_saved_name(name), "{name}");
        }
        for name in [
            "1ce45cbe.srpreplay", "_1ce45cbe.srpreplay", "x_1ce45cbe_100.srpreplay", "x_1ce45cbf.srpreplay",
            "x_1ce45cbe.srpreplay.part", "x_1ce45cbe.SRPREPLAY", "x1ce45cbe.srpreplay", "srp_1ce45cbe.srpreplay.part",
        ] {
            assert!(!request.is_saved_name(name), "{name}");
        }
        assert!(is_stale_part("srp_0a1b2c3d.srpreplay.part", "1ce45cbe"));
        for name in [
            "srp_1ce45cbe.srpreplay.part", "srp_0A1B2C3D.srpreplay.part", "srp_0a1b2c3d.sur.part", "srp_0a1b2c3d.sur",
            "x_0a1b2c3d.srpreplay", "srp_0a1b2c3.srpreplay.part", "notes.txt",
        ] {
            assert!(!is_stale_part(name, "1ce45cbe"), "{name}");
        }
    }

    #[test]
    fn the_language_is_the_launchers() {
        let problems = [
            Problem::BadLink, Problem::AlreadyRunning, Problem::Network("h.example".into()), Problem::Expired,
            Problem::Busy("h.example".into()), Problem::Status("h.example".into(), 302), Problem::Damaged,
            Problem::Disk("Access is denied".into()),
        ];
        let path = r"D:\Games\Skhid\game\replays\x.srpreplay";
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
                let saved = saved_message(Some(language), path);
                assert!(saved.ends_with(&format!("\n{path}")) && saved != saved_message(None, path), "{saved}");
            }
        }
        assert_eq!(saved_message(Some(9), path), format!("The replay is saved:\n{path}"));
        assert_eq!(message(None, &Problem::Disk("x".to_string())), "Cannot save the replay in the game's replays folder: x.");
    }

    #[test]
    fn the_language_is_user_cfg_s_localization_line() {
        let of = |text: &str| language_of_user_cfg(text.as_bytes());
        assert_eq!(of("account_name x\r\nlocalization russian\r\nr_fullscreen on\r\n"), Some(1));
        assert_eq!(of("localization\tpolish"), Some(4));
        assert_eq!(of("localization   german  \n"), Some(3));
        // The game's other languages: the messages are English.
        assert_eq!(of("localization czech\r\n"), None);
        assert_eq!(of("localizations russian\r\n# localization russian\r\n"), None);
        assert_eq!(of("localization Russian\r\n"), None);
        assert_eq!(of("localization\r\n"), None);
        assert_eq!(of(""), None);
    }
}
