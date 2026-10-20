//! What the patched 0.26g launcher and its updater ask this server for
//! (`client-patches/launcher/README.md`).
//!
//! - `GET /api/client-files`: which archive `/download` hands out now, as its
//!   SHA-256 and size. The updater compares it with what it installed last.
//! - `GET /resources/<sha256>`: the original bytes of a `resources.db`
//!   region an earlier updater wrote (the resource step, gone with the
//!   patches), for the updater's one-time restore of the stock db when it
//!   has no copy of its own; served as `/download` is (`resource_file` in
//!   `lib.rs`).
//! - `GET /launcher/…`: the page the launcher shows in its window, in the
//!   place of the original game's news page: the best players and the
//!   matches being played, as two tabs, what the server counts, and the
//!   legal notice. It is made for the launcher's embedded browser (Internet
//!   Explorer's engine, 899 by 335 pixels): fixed sizes, old CSS only, and
//!   embedded original background artwork. Its one script shows the tab that
//!   is clicked, and keeps the board, the matches and the numbers current
//!   while the window is open.
//! - `GET /launcher/places/<language>`: what changes on that page, each part
//!   behind the name of its place, which that script asks for.
//! - `GET /launcher/fresh/<language>`: the same for a page opened before the
//!   matches' tab was there, in that page's own form.

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use session::SessionStore;
use session::v026g::Board;
use survarium::v026g::localization::{self, Language};
use survarium::v026g::maps;

use crate::{
    CONNECT, ContentType, Game, Lang, LiveMatch, LivePhase, Response, available_download, escape,
    locked,
};

/// The players the page lists.
const TOP_PLAYERS: usize = 10;
/// The lines the matches' tab has room for: with more matches than that,
/// the last line counts the rest.
const MOST_MATCHES: usize = 10;
/// The same invite as the crash reporter; opened externally only on click.
const DISCORD_LINK: &str = include_str!("../launcher-skin/discord-link.html");
/// How long the page's numbers are kept before they are counted again, so
/// that asking often costs nothing.
const BOARD_AGE: Duration = Duration::from_secs(15);

/// The name by which an archive is known to hold the launcher: a ZIP keeps
/// its files' names as they are, in each file's header and in the list at
/// its end.
const LAUNCHER_NAME: &[u8] = b"game/binaries/x86/launcher_skhid.exe";

/// The name by which an archive is known to install the original game
/// itself: the extractor its updater unpacks that game with
/// (`client/updater`). An archive from before it has an updater that
/// only keeps our files current.
const EXTRACTOR_NAME: &[u8] = b"game/binaries/x86/7zr.exe";

/// What was read of the download file the last time: it is read again only
/// when the file's size or time changes (a new archive is put in its place
/// without a restart).
struct Checked {
    file: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    sha256: String,
    /// The archive holds the launcher, so the site's page tells players to
    /// start that.
    launcher: bool,
    /// It holds the extractor too: its updater brings the original game,
    /// so the page tells players to unpack into an empty folder.
    extractor: bool,
}

/// Where the page's script asks for what changes on the page, with the
/// page's language behind it. The answer names each place before what it
/// holds, so a page that is open while the server gets another one takes
/// the places it has and passes over the rest.
pub(super) const FRESH: &str = "/launcher/places/";
/// Where the script of a page from before the matches' tab asks. A launcher
/// can stay open for hours, and that script takes nothing but its three
/// places in its own order ([`BEFORE`]), with no names: answered here, such
/// a page still learns of a planned maintenance.
pub(super) const FRESH_BEFORE: &str = "/launcher/fresh/";
/// The places on the page that are filled again, by their `id`: the two
/// tabs of the left side, then the two tiles of numbers.
const PLACES: [&str; 4] = ["board", "live", "now", "totals"];
/// The places of [`FRESH_BEFORE`], as that page's script has them.
const BEFORE: [usize; 3] = [0, 2, 3];
/// What stands between a place's name and what it holds, and between two
/// places, in those answers. No place holds it: their markup has no such
/// tag, and a name is text ([`escape`]).
const BETWEEN: &str = "<hr>";
/// The page's one script. A click on a tab's label shows that tab: it sets
/// the left side's class and nothing else. Every ten seconds it asks for
/// [`FRESH`] and puts what changed into its place, so nothing else is drawn
/// again: the tab that was chosen stays, and a link is never taken from
/// under the pointer. A request that fails leaves the page as it is, and the
/// next one is made all the same. It is written for the oldest engine the
/// launcher's browser can be, and names no other host.
const SCRIPT: &str = r#"(function(){
var shown={},asking=false,left=document.getElementById("left");
function tab(id,name){
var label=document.getElementById(id);
if(label)label.onclick=function(){left.className=name;};
}
tab("tab-board","");
tab("tab-live","live");
function fresh(){
if(asking)return;
try{
var ask=new XMLHttpRequest();
ask.open("GET","/launcher/places/"+document.documentElement.lang,true);
ask.onreadystatechange=function(){
if(ask.readyState!=4)return;
asking=false;
try{
if(ask.status!=200)return;
var parts=ask.responseText.split("<hr>"),at,place;
for(at=0;at+1<parts.length;at+=2){
place=document.getElementById(parts[at]);
if(place&&parts[at+1]!==shown[parts[at]]){
place.innerHTML=parts[at+1];
shown[parts[at]]=parts[at+1];
}
}
}catch(e){}
};
asking=true;
ask.send(null);
}catch(e){asking=false;}
}
setInterval(fresh,10000);
})();"#;

static CHECKED: Mutex<Option<Checked>> = Mutex::new(None);
static BOARD: Mutex<Option<(Instant, Board)>> = Mutex::new(None);

/// The file's SHA-256, and whether [`LAUNCHER_NAME`] and [`EXTRACTOR_NAME`]
/// are in it, in one read.
fn read_through(file: &PathBuf) -> Option<(String, bool, bool)> {
    let mut open = File::open(file).ok()?;
    let mut hash = openssl::sha::Sha256::new();
    // The end of what was read before stays in front of each new piece, so
    // a name that lies across two pieces is found too.
    let keep = LAUNCHER_NAME.len().max(EXTRACTOR_NAME.len()) - 1;
    let mut buffer = vec![0_u8; keep + 64 * 1024];
    let (mut kept, mut launcher, mut extractor) = (0, false, false);
    let holds = |read: &[u8], name: &[u8]| read.windows(name.len()).any(|part| part == name);
    loop {
        match open.read(&mut buffer[kept..]) {
            Ok(0) => break,
            Ok(read) => {
                hash.update(&buffer[kept..kept + read]);
                let filled = kept + read;
                launcher = launcher || holds(&buffer[..filled], LAUNCHER_NAME);
                extractor = extractor || holds(&buffer[..filled], EXTRACTOR_NAME);
                kept = filled.min(keep);
                buffer.copy_within(filled - kept..filled, 0);
            }
            Err(_) => return None,
        }
    }
    let sha256 = hash
        .finish()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Some((sha256, launcher, extractor))
}

/// What the download file is, if there is one.
struct Offered {
    sha256: String,
    size: u64,
    launcher: bool,
    extractor: bool,
}

fn offered(file: &PathBuf) -> Option<Offered> {
    let meta = std::fs::metadata(file).ok().filter(|meta| meta.is_file())?;
    let (size, modified) = (meta.len(), meta.modified().ok());
    let mut checked = locked(&CHECKED);
    if let Some(known) = checked.as_ref()
        && known.file == *file
        && known.size == size
        && known.modified == modified
    {
        return Some(Offered {
            sha256: known.sha256.clone(),
            size,
            launcher: known.launcher,
            extractor: known.extractor,
        });
    }
    let (sha256, launcher, extractor) = read_through(file)?;
    // The file may have been replaced while it was read: what is said must
    // be of one file.
    let after = std::fs::metadata(file).ok()?;
    if after.len() != size || after.modified().ok() != modified {
        return None;
    }
    *checked = Some(Checked {
        file: file.clone(),
        size,
        modified,
        sha256: sha256.clone(),
        launcher,
        extractor,
    });
    Some(Offered {
        sha256,
        size,
        launcher,
        extractor,
    })
}

/// Whether the archive `/download` hands out holds the launcher: the home
/// page then tells players to start it, and else the start script, so the
/// page is right before and after the archive is replaced.
pub(super) fn offers_launcher() -> bool {
    available_download()
        .and_then(offered)
        .is_some_and(|offered| offered.launcher)
}

/// Whether the archive `/download` hands out brings the original game
/// itself: it holds the launcher, whose updater does that, and the
/// extractor the updater needs for it. The home page then tells players to
/// unpack into an empty folder, and else to get the game first.
pub(super) fn offers_game_install() -> bool {
    available_download()
        .and_then(offered)
        .is_some_and(|offered| offered.launcher && offered.extractor)
}

fn text(status: &'static str, body: String) -> Response {
    Response {
        status,
        content_type: ContentType::Text,
        body,
    }
}

/// `GET /api/client-files`: the format of `client/updater`
/// (`skhid-client-files 1`, then a word and its value on each line).
pub(super) fn client_files() -> Response {
    client_files_of(available_download())
}

fn client_files_of(file: Option<&PathBuf>) -> Response {
    let Some(file) = file else {
        return text("404 Not Found", "no client files are offered here\n".into());
    };
    match offered(file) {
        Some(Offered { sha256, size, .. }) => text(
            "200 OK",
            format!("skhid-client-files 1\nsha256 {sha256}\nsize {size}\n"),
        ),
        // Being replaced right now, or unreadable: the updater tries again.
        None => text(
            "503 Service Unavailable",
            "the client files cannot be read now\n".into(),
        ),
    }
}

/// The page's language from the launcher's own. The launcher asks for
/// `/launcher/v2/survarium/?lang=<en|ru|ua|de|pl>`: `ua` and `en` are read
/// with the request ([`Lang::pick`]), and for the three languages the site
/// has no text in, the browser's language decides as on every page. A code
/// as the last part of the path (`/launcher/ua`, `/launcher/v2/index-ua.html`)
/// is taken too.
pub(super) fn language(path: &str, chosen: Lang) -> Lang {
    let last = path.rsplit('/').next().unwrap_or("").to_ascii_lowercase();
    let last = last.strip_suffix(".html").unwrap_or(&last);
    let code = last.rsplit('-').next().unwrap_or("");
    match code {
        "ua" | "uk" | "ukrainian" => Lang::Uk,
        "en" | "ru" | "de" | "pl" | "english" | "russian" | "german" | "polish" => Lang::En,
        _ => chosen,
    }
}

/// What the page counts, beside the board.
#[derive(Clone, Copy)]
pub(super) struct Counts {
    pub online: usize,
    pub registered: usize,
    pub matches: usize,
    /// Rooms that exist, and the players in them.
    pub rooms: usize,
    pub in_rooms: usize,
    pub clans: usize,
    /// The minutes to a planned maintenance.
    pub maintenance: Option<u32>,
}

/// Whether the path is the launcher's: its page, [`FRESH`] or
/// [`FRESH_BEFORE`].
pub(super) fn serves(path: &str) -> bool {
    path == "/launcher" || path.starts_with("/launcher/")
}

/// The `Content-Security-Policy` of the launcher's page: the site's own, and
/// the page's images (they are in its style as `data:`), its one script (by
/// its SHA-256, so no other text on the page can run) and that script's
/// request to this server.
pub(super) fn policy() -> &'static str {
    static POLICY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    POLICY.get_or_init(|| {
        format!(
            "{}; img-src data:; script-src 'sha256-{}'; connect-src 'self'",
            crate::POLICY,
            openssl::base64::encode_block(&openssl::sha::sha256(SCRIPT.as_bytes()))
        )
    })
}

/// The board, the matches and the counts as they are now.
fn current(store: &SessionStore, game: &Game) -> (Board, Vec<LiveMatch>, Counts) {
    let board = {
        let mut kept = locked(&BOARD);
        match kept.as_ref() {
            Some((at, board)) if at.elapsed() < BOARD_AGE => board.clone(),
            _ => {
                let board = game.accounts.board(TOP_PLAYERS);
                *kept = Some((Instant::now(), board.clone()));
                board
            }
        }
    };
    let counts = Counts {
        online: store.players_present(),
        registered: store.registered_accounts(),
        matches: game.matches.active_matches(),
        rooms: store.room_counts().rooms,
        in_rooms: store.room_counts().in_rooms,
        clans: game.accounts.clans().len(),
        maintenance: store.minutes_to_maintenance(),
    };
    (board, game.matches.live_matches(), counts)
}

/// `GET /launcher/…`.
pub(super) fn page(lang: Lang, store: &SessionStore, game: &Game) -> Response {
    let (board, live, counts) = current(store, game);
    let test = CONNECT.get().is_some_and(|connect| connect.test_server);
    Response {
        status: "200 OK",
        content_type: ContentType::Html,
        body: render(lang, &board, &live, counts, test),
    }
}

/// `GET /launcher/places/<language>`: each of [`PLACES`] and what it holds
/// now, with [`BETWEEN`] between them all.
pub(super) fn fresh(lang: Lang, store: &SessionStore, game: &Game) -> Response {
    let (board, live, counts) = current(store, game);
    Response {
        status: "200 OK",
        content_type: ContentType::Html,
        body: named(&places(lang, &board, &live, counts)),
    }
}

fn named(parts: &[String; 4]) -> String {
    let mut answer = Vec::new();
    for (place, part) in PLACES.iter().zip(parts) {
        answer.extend([*place, part.as_str()]);
    }
    answer.join(BETWEEN)
}

/// `GET /launcher/fresh/<language>`: what the [`BEFORE`] places hold now,
/// with [`BETWEEN`] between them.
pub(super) fn fresh_before(lang: Lang, store: &SessionStore, game: &Game) -> Response {
    let (board, live, counts) = current(store, game);
    let parts = places(lang, &board, &live, counts);
    Response {
        status: "200 OK",
        content_type: ContentType::Html,
        body: BEFORE.map(|place| parts[place].as_str()).join(BETWEEN),
    }
}

/// A number with its thousands apart: 12 345.
fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push_str("&nbsp;");
        }
        out.push(digit);
    }
    out
}

/// The game's language of the page's.
fn game_language(lang: Lang) -> Language {
    match lang {
        Lang::En => Language::English,
        Lang::Uk => Language::Ukrainian,
    }
}

/// The game's own name of a map: the text of its config's `ui.map_name`
/// (`st_level_0N_name`) in the page's language, else in English
/// ([`localization::text`]), as the game data has it (a map a patch adds
/// comes with its name in the patch's localizations); without one, its
/// project's folder (`level_12`).
fn map_name(lang: Lang, map: &maps::MapMode) -> String {
    match localization::text(game_language(lang), map.map_name_key) {
        Some(name) => name.to_owned(),
        None => map.project.split('/').next().unwrap_or("?").to_owned(),
    }
}

/// The game's own name of a mode (`ui.mode_name`, `st_mode_0N_name`): it
/// has one name for both modes with batteries. Without one, its number.
fn mode_name(lang: Lang, map: &maps::MapMode) -> String {
    match localization::text(game_language(lang), map.mode_name_key) {
        Some(name) => name.to_owned(),
        None => lang.tr(
            &format!("mode {}", map.mode),
            &format!("режим {}", map.mode),
        ),
    }
}

/// The matches' tab: a line for each match that is not over, the oldest
/// first. No player is named: the page is open to everyone.
///
/// - Players: each team's, of those who have not left.
/// - Started: how long ago play began; before that, what the match waits for.
/// - Time left: to the time limit, in whole minutes rounded up. A match can
///   end sooner (the tickets or the batteries, or everyone leaving), so it
///   is "at most".
/// - Score: tickets left in a Team Deathmatch, batteries brought otherwise,
///   the leader's in bold; only of a match in play, and only as fresh as
///   [`session::v026g::matches_now::SCORE_AGE`].
///
/// Which match is listed and which score is told are the rules the lobby's
/// list of the matches has too ([`session::v026g::matches_now`]).
fn matches(lang: Lang, live: &[LiveMatch]) -> String {
    const NONE: &str = "&mdash;";
    let open: Vec<&LiveMatch> = live.iter().filter(|each| each.phase.is_listed()).collect();
    if open.is_empty() {
        return format!(
            "<p class=none>{}</p>",
            lang.tr(
                "No match is being played now.",
                "Зараз не грається жодного матчу."
            )
        );
    }
    let listed = if open.len() > MOST_MATCHES {
        MOST_MATCHES - 1
    } else {
        open.len()
    };
    let mut rows = String::new();
    for each in &open[..listed] {
        let (map, mode) = match maps::map_mode(each.map_id) {
            Some(map) => (map_name(lang, map), mode_name(lang, map)),
            None => ("?".to_owned(), "?".to_owned()),
        };
        let started = match (each.phase, each.played) {
            (LivePhase::Gathering, _) => lang.tr("waiting", "очікування"),
            (LivePhase::Countdown, _) => lang.tr("starting", "старт"),
            (_, Some(played)) => match played.as_secs() / 60 {
                0 => lang.tr("just now", "щойно"),
                minutes => lang.tr(&format!("{minutes} min ago"), &format!("{minutes} хв тому")),
            },
            (_, None) => NONE.to_owned(),
        };
        let left = each.time_left.map_or(NONE.to_owned(), |left| {
            let minutes = left.as_secs().div_ceil(60).max(1);
            lang.tr(
                &format!("&le; {minutes} min"),
                &format!("&le; {minutes} хв"),
            )
        });
        let score = each
            .standing
            .filter(|standing| each.phase.shows_score(standing.age))
            .map_or(NONE.to_owned(), |standing| {
                let [first, second] = standing.points;
                match standing.leader {
                    Some(0) => format!("<b>{first}</b> : {second}"),
                    Some(_) => format!("{first} : <b>{second}</b>"),
                    None => format!("{first} : {second}"),
                }
            });
        rows += &format!(
            "<tr><td class=m title=\"{}\">{} <i>&middot; {}</i></td><td class=s>{}</td>\
             <td class=a>{started}</td><td class=e>{left}</td><td class=c>{score}</td></tr>",
            escape(&format!("{map} · {mode}")),
            escape(&map),
            escape(&mode),
            lang.tr(
                &format!("{} v {}", each.players[0], each.players[1]),
                &format!("{} на {}", each.players[0], each.players[1]),
            ),
        );
    }
    if listed < open.len() {
        let more = open.len() - listed;
        rows += &format!(
            "<tr><td class=m colspan=5><i>{}</i></td></tr>",
            lang.tr(&format!("and {more} more"), &format!("і ще {more}"))
        );
    }
    format!(
        "<table cellspacing=0 cellpadding=0><tr><th class=m>{}</th><th class=s>{}</th>\
         <th class=a>{}</th><th class=e>{}</th><th class=c>{}</th></tr>{rows}</table>",
        lang.tr("Match", "Матч"),
        lang.tr("Players", "Гравці"),
        lang.tr("Started", "Початок"),
        lang.tr("Time left", "До кінця"),
        lang.tr("Score", "Рахунок"),
    )
}

/// What each of [`PLACES`] holds: the board, the matches, the server's
/// numbers of this moment with the line under them, and the numbers since
/// it opened with theirs.
fn places(lang: Lang, board: &Board, live: &[LiveMatch], counts: Counts) -> [String; 4] {
    let mut rows = String::new();
    for (index, row) in board.top.iter().enumerate() {
        let clan = if row.clan_tag.is_empty() {
            String::new()
        } else {
            format!("<i>[{}]</i> ", escape(&row.clan_tag))
        };
        rows += &format!(
            "<tr{}><td class=n>{}</td><td class=p>{clan}{}</td><td class=l>{}</td><td class=r>{}</td></tr>",
            if index % 2 == 1 { " class=o" } else { "" },
            index + 1,
            escape(&row.name),
            row.level,
            row.rating,
        );
    }
    let table = if board.top.is_empty() {
        format!(
            "<p class=none>{}</p>",
            lang.tr(
                "No battle has been fought yet. Be the first.",
                "Ще не відбулося жодного бою. Будьте першими."
            )
        )
    } else {
        format!(
            "<table cellspacing=0 cellpadding=0><tr><th class=n>#</th><th class=p>{}</th>\
             <th class=l>{}</th><th class=r>{}</th></tr>{rows}</table>",
            lang.tr("Player", "Гравець"),
            lang.tr("Level", "Рівень"),
            lang.tr("Rating", "Рейтинг"),
        )
    };
    let cell = |value: u64, label: String| {
        format!("<td><b>{}</b><span>{label}</span></td>", grouped(value))
    };
    let now = format!(
        "<table class=nums cellspacing=0 cellpadding=0><tr>{}{}{}{}</tr></table>",
        cell(counts.online as u64, lang.tr("online", "онлайн")),
        cell(counts.rooms as u64, lang.tr("rooms", "кімнат")),
        cell(counts.in_rooms as u64, lang.tr("in rooms", "у кімнатах")),
        cell(
            counts.matches as u64,
            lang.tr("matches now", "матчів зараз")
        ),
    );
    let totals = format!(
        "<table class=nums cellspacing=0 cellpadding=0><tr>{}{}{}</tr></table>",
        cell(counts.registered as u64, lang.tr("players", "гравців")),
        cell(board.battles, lang.tr("battles fought", "проведено боїв")),
        cell(counts.clans as u64, lang.tr("clans", "кланів")),
    );
    let status = match counts.maintenance {
        Some(0) => lang.tr(
            "Maintenance: the server is down",
            "Технічні роботи: сервер вимкнено",
        ),
        Some(minutes) => lang.tr(
            &format!("Maintenance in {minutes} min"),
            &format!("Технічні роботи через {minutes} хв"),
        ),
        None => lang.tr("On the server now", "Зараз на сервері"),
    };
    let status_class = if counts.maintenance.is_some() {
        "cap warn"
    } else {
        "cap"
    };
    [
        table,
        matches(lang, live),
        format!("{now}<div class=\"{status_class}\">{status}</div>"),
        format!(
            "{totals}<div class=cap>{}</div>",
            lang.tr("Since the server opened", "Від відкриття сервера")
        ),
    ]
}

fn render(lang: Lang, board: &Board, live: &[LiveMatch], counts: Counts, test: bool) -> String {
    let [table, matches, now, totals] = places(lang, board, live, counts);
    let badge = if test {
        format!(" <em>{}</em>", lang.tr("TEST SERVER", "ТЕСТОВИЙ СЕРВЕР"))
    } else {
        String::new()
    };
    let legal = lang.tr(
        "Unofficial community server. Not affiliated with or endorsed by Vostok Games. \
         Survarium is a trademark of its owners. An early test: accounts and progress \
         can be reset at any time.",
        "Неофіційний ком'юніті сервер. Не пов'язаний із Vostok Games і не схвалений ними. \
         Survarium є торговельною маркою її власників. Раннє тестування: акаунти та \
         прогрес можуть бути скинуті будь-коли.",
    );
    format!(
        "<!DOCTYPE html><html lang={code}><head><meta http-equiv=\"X-UA-Compatible\" content=\"IE=edge\">\
         <meta http-equiv=\"content-type\" content=\"text/html; charset=UTF-8\">\
         <title>Skhid</title><style type=\"text/css\">{style}</style></head><body>\
         <div id=container>\
         <div id=left><h1><span id=tab-{board}>{top}</span><span id=tab-{live}>{playing}</span>\
         {badge}</h1><div id={board}>{table}</div><div id={live}>{matches}</div>\
         <div class=cap>{server}</div></div>\
         <div id=right>\
         <div class=\"tile first\" id={first}>{now}</div>\
         <div class=\"tile second\" id={second}>{totals}</div>\
         <div class=\"tile third\"><p class=legal>{legal}</p><div class=cap>{discord}{notice}</div></div>\
         </div></div><script type=\"text/javascript\">{SCRIPT}</script></body></html>",
        style = style(),
        board = PLACES[0],
        live = PLACES[1],
        first = PLACES[2],
        second = PLACES[3],
        discord = DISCORD_LINK.trim(),
        code = lang.code(),
        top = lang.tr("Top 10 players", "Топ-10 гравців"),
        playing = lang.tr("Matches now", "Матчі зараз"),
        server = lang.tr(
            "Skhid &middot; Survarium <i>0.26g</i> community server",
            "Схід &middot; ком'юніті сервер Survarium <i>0.26g</i>"
        ),
        notice = lang.tr("Legal notice", "Правова інформація"),
        legal = escape(&legal),
    )
}

/// The same skin as the loading/offline pages. Keep the original artwork
/// inline: this also works at every launcher URL, with no extra requests.
fn style() -> &'static str {
    static STYLE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    STYLE.get_or_init(|| {
        include_str!("../launcher-skin/style.css")
            .replace(
                "@BACKGROUND@",
                &format!(
                    "data:image/jpeg;base64,{}",
                    openssl::base64::encode_block(include_bytes!("../launcher-skin/back.jpg"))
                ),
            )
            .replace(
                "@SHADOW@",
                &format!(
                    "data:image/png;base64,{}",
                    openssl::base64::encode_block(include_bytes!("../launcher-skin/shadow.png"))
                ),
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LiveStanding;
    use session::v026g::BoardRow;
    use session::v026g::matches_now::SCORE_AGE;

    /// Team Deathmatch and batteries on Rudnya, Team Deathmatch on School.
    const RUDNYA_TDM: u8 = 117;
    const RUDNYA_BATTERIES: u8 = 61;

    fn playing(map_id: u8, points: [u16; 2], leader: Option<usize>) -> LiveMatch {
        LiveMatch {
            map_id,
            phase: LivePhase::Playing,
            players: [4, 3],
            played: Some(Duration::from_secs(6 * 60 + 20)),
            time_left: Some(Duration::from_secs(8 * 60 + 40)),
            standing: Some(LiveStanding {
                points,
                leader,
                age: Duration::from_secs(4),
            }),
        }
    }

    /// A match in play, one whose countdown runs and one still gathering.
    fn live() -> Vec<LiveMatch> {
        vec![
            playing(RUDNYA_TDM, [88, 78], Some(0)),
            LiveMatch {
                map_id: RUDNYA_BATTERIES,
                phase: LivePhase::Countdown,
                players: [2, 2],
                played: None,
                time_left: Some(Duration::from_secs(15 * 60 + 12)),
                // Full tickets, or no battery yet: nothing to show.
                standing: Some(LiveStanding {
                    points: [0, 0],
                    leader: None,
                    age: Duration::ZERO,
                }),
            },
            LiveMatch {
                map_id: RUDNYA_TDM,
                phase: LivePhase::Gathering,
                players: [5, 5],
                played: None,
                time_left: None,
                standing: None,
            },
        ]
    }

    fn counts() -> Counts {
        Counts {
            online: 12,
            registered: 3456,
            matches: 2,
            rooms: 3,
            in_rooms: 5,
            clans: 7,
            maintenance: None,
        }
    }

    fn board() -> Board {
        Board {
            top: vec![
                BoardRow {
                    name: "Sasha".into(),
                    clan_tag: "TAG".into(),
                    rating: 1420,
                    level: 31,
                },
                BoardRow {
                    name: "<b>x</b>".into(),
                    clan_tag: String::new(),
                    rating: 1300,
                    level: 4,
                },
            ],
            battles: 1_234_567,
        }
    }

    // Render the actual server output for the browser comparison; no server,
    // credentials, or live player database is needed.
    #[test]
    #[ignore = "set LAUNCHER_VISUAL_DIR and run the browser comparison"]
    fn export_visual_fixtures() {
        let dir =
            PathBuf::from(std::env::var_os("LAUNCHER_VISUAL_DIR").expect("LAUNCHER_VISUAL_DIR"));
        std::fs::create_dir_all(&dir).unwrap();
        let full = Board {
            top: (0..10)
                .map(|i| BoardRow {
                    name: format!("ДужеДовгеІм'яГравця_ABCDEFGHIJKLMNOPQRSTUVWXYZ_{i}"),
                    clan_tag: "ДОВГИЙТЕГ".into(),
                    rating: 12345 - i,
                    level: 100,
                })
                .collect(),
            battles: 1_234_567,
        };
        // Every map there is, in each mode's name, with the widest numbers.
        let many: Vec<LiveMatch> = (0..12)
            .map(|i| LiveMatch {
                players: [10, 10],
                played: Some(Duration::from_secs(14 * 60)),
                ..playing(
                    maps::playable()
                        .filter(|map| map.level == i % 8 + 1)
                        .nth(usize::from(i % 3))
                        .unwrap()
                        .id,
                    [150, 149],
                    Some(0),
                )
            })
            .collect();
        for lang in [Lang::En, Lang::Uk] {
            for (state, board, live, maintenance, test) in [
                ("full", &full, live(), None, false),
                ("empty", &Board::default(), live(), None, false),
                ("maintenance", &full, live(), Some(25), true),
                ("down", &full, live(), Some(0), true),
                ("matches", &full, live(), None, true),
                ("matches-many", &full, many.clone(), None, false),
                ("matches-none", &full, Vec::new(), None, false),
            ] {
                let mut counts = counts();
                counts.maintenance = maintenance;
                let mut page = render(lang, board, &live, counts, test);
                // As after a click on the matches' label.
                if state.starts_with("matches") {
                    assert_eq!(page.matches("<div id=left>").count(), 1);
                    page = page.replace("<div id=left>", "<div id=left class=live>");
                }
                std::fs::write(
                    dir.join(format!("online-{}-{state}.html", lang.code())),
                    page,
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn the_page_lists_the_board_and_the_counts() {
        let page = render(Lang::En, &board(), &live(), counts(), false);
        assert!(page.contains("<td class=n>1</td><td class=p><i>[TAG]</i> Sasha</td><td class=l>31</td><td class=r>1420</td>"));
        // A name is text, never markup.
        assert!(page.contains("&lt;b&gt;x&lt;/b&gt;"));
        assert!(!page.contains("<b>x</b>"));
        assert!(page.contains("<b>12</b><span>online</span>"));
        assert!(page.contains("<b>3</b><span>rooms</span>"));
        assert!(page.contains("<b>5</b><span>in rooms</span>"));
        assert!(page.contains("<b>2</b><span>matches now</span>"));
        assert!(page.contains("<b>3&nbsp;456</b><span>players</span>"));
        assert!(page.contains("<b>1&nbsp;234&nbsp;567</b><span>battles fought</span>"));
        assert!(page.contains("<b>7</b><span>clans</span>"));
        assert!(page.contains("Not affiliated with or endorsed by Vostok Games"));
        assert!(!page.contains("TEST SERVER"));
        assert!(
            render(Lang::En, &board(), &live(), counts(), true).contains("<em>TEST SERVER</em>")
        );
    }

    #[test]
    fn the_matches_tab_says_how_each_match_stands_and_names_nobody() {
        let tab = matches(Lang::En, &live());
        assert!(tab.contains(
            "<tr><th class=m>Match</th><th class=s>Players</th><th class=a>Started</th>\
             <th class=e>Time left</th><th class=c>Score</th></tr>"
        ));
        // In play: the game's own names, minutes since play began, minutes
        // to the time limit rounded up, and the leader's points in bold.
        assert!(tab.contains(
            "<tr><td class=m title=\"Rudnya · Team Deathmatch\">Rudnya <i>&middot; Team \
             Deathmatch</i></td><td class=s>4 v 3</td><td class=a>6 min ago</td>\
             <td class=e>&le; 9 min</td><td class=c><b>88</b> : 78</td></tr>"
        ));
        // Before play there is no score to tell, whatever the world says.
        assert!(tab.contains(
            "Rudnya <i>&middot; Protective device</i></td><td class=s>2 v 2</td>\
             <td class=a>starting</td><td class=e>&le; 16 min</td><td class=c>&mdash;</td></tr>"
        ));
        assert!(tab.contains(
            "<td class=s>5 v 5</td><td class=a>waiting</td><td class=e>&mdash;</td>\
             <td class=c>&mdash;</td></tr>"
        ));
        assert_eq!(tab.matches("<tr>").count(), 4);

        let row = |each: LiveMatch| matches(Lang::En, &[each]);
        // The other team leads; a draw has no leader.
        assert!(row(playing(RUDNYA_TDM, [40, 71], Some(1))).contains("40 : <b>71</b>"));
        assert!(row(playing(RUDNYA_BATTERIES, [3, 3], None)).contains("<td class=c>3 : 3</td>"));
        // A score the match server could not renew is not shown.
        let mut stale = playing(RUDNYA_TDM, [88, 78], Some(0));
        stale.standing.as_mut().unwrap().age = SCORE_AGE + Duration::from_secs(1);
        assert!(row(stale.clone()).contains("<td class=c>&mdash;</td>"));
        stale.standing.as_mut().unwrap().age = SCORE_AGE;
        assert!(row(stale).contains("<b>88</b> : 78"));
        // The first minute of play, and the last one.
        let mut edge = playing(RUDNYA_TDM, [100, 100], None);
        edge.played = Some(Duration::from_secs(59));
        edge.time_left = Some(Duration::from_secs(1));
        let edge = row(edge);
        assert!(edge.contains("<td class=a>just now</td><td class=e>&le; 1 min</td>"));

        // A match that has its result is not listed; with none left the
        // tab says so.
        let mut over = playing(RUDNYA_TDM, [0, 12], Some(1));
        over.phase = LivePhase::Over;
        let none = row(over.clone());
        assert_eq!(none, "<p class=none>No match is being played now.</p>");
        assert_eq!(matches(Lang::En, &[]), none);
        let mut two = live();
        two.insert(0, over);
        assert_eq!(matches(Lang::En, &two), tab);

        // Ten lines: with more matches the last one counts the rest.
        let ten = vec![playing(RUDNYA_TDM, [9, 8], Some(0)); MOST_MATCHES];
        let tab = matches(Lang::En, &ten);
        assert_eq!(tab.matches("<tr>").count(), 1 + MOST_MATCHES);
        assert!(!tab.contains("more"));
        let tab = matches(Lang::En, &vec![playing(RUDNYA_TDM, [9, 8], Some(0)); 13]);
        assert_eq!(tab.matches("<tr>").count(), 1 + MOST_MATCHES);
        assert_eq!(tab.matches("<b>9</b> : 8").count(), MOST_MATCHES - 1);
        assert!(tab.ends_with("<tr><td class=m colspan=5><i>and 4 more</i></td></tr></table>"));

        let tab = matches(Lang::Uk, &live());
        assert!(tab.contains(
            "<td class=m title=\"Рудня · Командний бій\">Рудня <i>&middot; Командний бій</i></td>\
             <td class=s>4 на 3</td><td class=a>6 хв тому</td><td class=e>&le; 9 хв</td>"
        ));
        assert!(
            tab.contains("<td class=a>старт</td>") && tab.contains("<td class=a>очікування</td>")
        );
        // A name of the game's with quotes in it stays inside its attribute.
        let radar = maps::playable().find(|map| map.level == 3).unwrap().id;
        assert!(row_uk(radar).contains("title=\"РЛС &quot;Восток&quot; · "));
        // The names are the game data's: its localizations, by the keys of
        // the map's config.
        let rudnya_tdm = maps::map_mode(RUDNYA_TDM).unwrap();
        assert_eq!(map_name(Lang::En, rudnya_tdm), "Rudnya");
        assert_eq!(mode_name(Lang::En, rudnya_tdm), "Team Deathmatch");
        // A map the localizations do not name (a patch's that brings no
        // texts): its project's folder, and its mode by number.
        let twelve = maps::MapMode {
            level: 12,
            mode: 9,
            project: "level_12/day_4",
            map_name_key: "st_level_12_name",
            mode_name_key: "st_mode_09_name",
            ..*rudnya_tdm
        };
        assert_eq!(map_name(Lang::En, &twelve), "level_12");
        assert_eq!(map_name(Lang::Uk, &twelve), "level_12");
        assert_eq!(mode_name(Lang::Uk, &twelve), "режим 9");
        // Every playable map and mode has a name of the game's.
        for map in maps::playable() {
            for lang in [Lang::En, Lang::Uk] {
                let game = |key| localization::text(game_language(lang), key).map(str::to_owned);
                assert_eq!(Some(map_name(lang, map)), game(map.map_name_key));
                assert_eq!(Some(mode_name(lang, map)), game(map.mode_name_key));
            }
        }
    }

    fn row_uk(map_id: u8) -> String {
        matches(Lang::Uk, &[playing(map_id, [1, 0], Some(0))])
    }

    #[test]
    fn the_skin_embeds_only_the_original_images_within_ie8_limits() {
        let skin = style();
        let mut urls = Vec::new();
        let mut rest = skin;
        while let Some((_, tail)) = rest.split_once("url(\"") {
            let (url, tail) = tail.split_once("\")").unwrap();
            urls.push(url);
            rest = tail;
        }
        assert_eq!(urls.len(), 2);
        for (url, prefix, original) in [
            (
                urls[0],
                "data:image/jpeg;base64,",
                include_bytes!("../launcher-skin/back.jpg").as_slice(),
            ),
            (
                urls[1],
                "data:image/png;base64,",
                include_bytes!("../launcher-skin/shadow.png").as_slice(),
            ),
        ] {
            assert!(url.len() <= 32768, "IE8 silently discards longer data URLs");
            let decoded = openssl::base64::decode_block(url.strip_prefix(prefix).unwrap()).unwrap();
            assert_eq!(decoded, original);
        }
        let css = skin.to_ascii_lowercase();
        for banned in [
            "http:",
            "https:",
            "@import",
            "expression(",
            "behavior:",
            "javascript:",
        ] {
            assert!(!css.contains(banned), "{banned} in the skin");
        }
        assert_eq!(skin.matches("url(").count(), 2);
    }

    #[test]
    fn the_page_asks_nothing_of_another_host_and_runs_its_one_script_only() {
        let script = format!("<script type=\"text/javascript\">{SCRIPT}</script>");
        // The script asks this server alone, by a path, and makes no text
        // into code.
        for banned in ["http", "//", "href", "src", "eval", "write", "</"] {
            assert!(!SCRIPT.contains(banned), "{banned} in the script");
        }
        for lang in [Lang::En, Lang::Uk] {
            let page = render(lang, &board(), &live(), counts(), true);
            assert!(page.contains(style()));
            assert_eq!(page.matches(&script).count(), 1);
            // The only external destination is an explicit user-clicked link,
            // not an image, script, or automatic navigation.
            let discord = DISCORD_LINK.trim();
            assert_eq!(page.matches(discord).count(), 1);
            assert!(discord.contains("href=\"https://discord.gg/Pxa4prXfwu\""));
            assert!(discord.contains("target=\"_blank\""));
            let page = page
                .replace(style(), "")
                .replace(discord, "")
                .replace(&script, "")
                .to_ascii_lowercase();
            for banned in [
                "http://",
                "https://",
                "//",
                "<script",
                "<img",
                "<link",
                "<iframe",
                "url(",
                "@import",
                "javascript:",
                "href",
                "src=",
            ] {
                // The doctype and the X-UA-Compatible line name no address.
                assert!(!page.contains(banned), "{banned} in the launcher page");
            }
        }
    }

    #[test]
    fn a_click_shows_a_tab_and_no_fresh_answer_takes_it_back() {
        // The script is plain text of its own: the labels it listens to and
        // the class it sets are the page's and the skin's.
        assert!(SCRIPT.contains("left=document.getElementById(\"left\")"));
        assert!(SCRIPT.contains("label.onclick=function(){left.className=name;}"));
        assert!(SCRIPT.contains(&format!("tab(\"tab-{}\",\"\");", PLACES[0])));
        assert!(SCRIPT.contains(&format!("tab(\"tab-{0}\",\"{0}\");", PLACES[1])));
        // Nothing else of the script touches the class, and the only thing
        // it writes into the page is what a place holds.
        assert_eq!(SCRIPT.matches("className").count(), 1);
        assert_eq!(SCRIPT.matches("innerHTML").count(), 1);
        let skin = style();
        // The board shows until the left side is given the matches' class.
        assert!(skin.contains("#live,#left.live #board{display:none}"));
        assert!(skin.contains("#left.live #live{display:block}"));
        for lang in [Lang::En, Lang::Uk] {
            let page = render(lang, &board(), &live(), counts(), false);
            assert_eq!(page.matches("<div id=left>").count(), 1);
            for place in &PLACES[..2] {
                assert_eq!(page.matches(&format!("<span id=tab-{place}>")).count(), 1);
            }
            // What says which tab shows is outside every place, so filling
            // the places again leaves the chosen tab as it is.
            for part in places(lang, &board(), &live(), counts()) {
                assert!(!part.contains("id=") && !part.contains("<h1"), "{part}");
            }
            let before = page.split_once("<div id=board>").unwrap().0;
            assert!(before.contains("<div id=left><h1><span id=tab-board>"));
            assert!(before.ends_with("</h1>"));
        }
    }

    #[test]
    fn the_script_fills_the_places_the_fresh_answer_names() {
        // The script is plain text of its own: what it asks for, what it
        // splits by and where it puts the pieces are the server's.
        assert!(SCRIPT.contains(&format!("\"{FRESH}\"+document.documentElement.lang")));
        assert!(SCRIPT.contains(&format!("split(\"{BETWEEN}\")")));
        assert!(SCRIPT.contains("place=document.getElementById(parts[at]);"));
        assert!(SCRIPT.contains("setInterval(fresh,10000)"));
        for lang in [Lang::En, Lang::Uk] {
            // The page's own language is one the path of the answer reads.
            assert_eq!(language(&format!("{FRESH}{}", lang.code()), Lang::En), lang);
            assert_eq!(language(&format!("{FRESH}{}", lang.code()), Lang::Uk), lang);
            let mut soon = counts();
            soon.maintenance = Some(25);
            for (board, live, counts) in [
                (board(), live(), counts()),
                (Board::default(), Vec::new(), soon),
            ] {
                let page = render(lang, &board, &live, counts, false);
                assert!(page.contains(&format!("lang={}>", lang.code())));
                let parts = places(lang, &board, &live, counts);
                // Each place is on the page once, holding what the answer
                // gives for it, so the first answer changes nothing seen.
                for (place, part) in PLACES.iter().zip(&parts) {
                    assert_eq!(page.matches(&format!(" id={place}>")).count(), 1);
                    assert!(
                        page.contains(&format!(" id={place}>{part}</div>")),
                        "{place}"
                    );
                }
                // The answer: each place's name, then what it holds.
                let answer = named(&parts);
                let answer: Vec<&str> = answer.split(BETWEEN).collect();
                assert_eq!(answer.len(), 2 * PLACES.len());
                for (at, (place, part)) in PLACES.iter().zip(&parts).enumerate() {
                    assert_eq!(
                        (answer[2 * at], answer[2 * at + 1]),
                        (*place, part.as_str())
                    );
                }
            }
        }
        // A name cannot stand for the border between two places.
        let mut forged = board();
        forged.top[0].name = BETWEEN.into();
        forged.top[0].clan_tag = BETWEEN.into();
        let answer = named(&places(Lang::En, &forged, &live(), counts()));
        assert_eq!(answer.split(BETWEEN).count(), 2 * PLACES.len());
        // The page from before the tabs has the first, third and fourth.
        assert_eq!(
            BEFORE.map(|place| PLACES[place]),
            ["board", "now", "totals"]
        );
    }

    #[test]
    fn the_launchers_policy_lets_its_script_run_and_no_other() {
        let policy = policy();
        assert!(policy.starts_with(crate::POLICY));
        let hash = openssl::base64::encode_block(&openssl::sha::sha256(SCRIPT.as_bytes()));
        assert!(policy.contains(&format!("script-src 'sha256-{hash}';")));
        assert!(policy.contains("connect-src 'self'"));
        assert!(policy.contains("img-src data:;"));
        assert!(!policy.contains("unsafe-inline' 'sha256") && !policy.contains("unsafe-eval"));
        assert!(!crate::POLICY.contains("script-src"));
        assert!(serves("/launcher") && serves("/launcher/v2/survarium/") && serves(FRESH));
        assert!(serves(FRESH_BEFORE));
        assert!(!serves("/launchers") && !serves("/") && !serves("/api/stats"));
    }

    #[test]
    fn the_page_is_ukrainian_for_the_ukrainian_launcher() {
        let page = render(Lang::Uk, &board(), &live(), counts(), false);
        assert!(page.contains("Топ-10 гравців"));
        assert!(page.contains("<span>проведено боїв</span>"));
        assert!(page.contains("lang=uk"));
        assert!(page.contains("<span id=tab-live>Матчі зараз</span>"));
        let empty = render(Lang::Uk, &Board::default(), &[], counts(), false);
        assert!(empty.contains("Ще не відбулося жодного бою"));
        assert!(empty.contains("Зараз не грається жодного матчу."));
        assert!(!empty.contains("<th"));
    }

    #[test]
    fn a_planned_maintenance_is_said_on_the_page() {
        let mut soon = counts();
        soon.maintenance = Some(25);
        let page = render(Lang::En, &board(), &live(), soon, false);
        assert!(page.contains("<div class=\"cap warn\">Maintenance in 25 min</div>"));
        soon.maintenance = Some(0);
        assert!(
            render(Lang::En, &board(), &live(), soon, false)
                .contains("Maintenance: the server is down")
        );
        assert!(
            render(Lang::En, &board(), &live(), counts(), false)
                .contains("<div class=\"cap\">On the server now</div>")
        );
    }

    #[test]
    fn the_language_comes_from_the_launchers_path() {
        for path in [
            "/launcher/v2/index-ua.html",
            "/launcher/ua",
            "/launcher/v2/ua",
            "/launcher/v2/ukrainian",
        ] {
            assert_eq!(language(path, Lang::En), Lang::Uk, "{path}");
        }
        for path in [
            "/launcher/v2/index-en.html",
            "/launcher/ru",
            "/launcher/v2/de",
            "/launcher/pl",
            "/launcher/v2/english",
        ] {
            assert_eq!(language(path, Lang::Uk), Lang::En, "{path}");
        }
        // Nothing in the path (the launcher's own form,
        // `/launcher/v2/survarium/?lang=ua`): `?lang=` or the browser's
        // language, as on every other page.
        assert_eq!(language("/launcher/v2/survarium/", Lang::Uk), Lang::Uk);
        assert_eq!(language("/launcher/v2/survarium/", Lang::En), Lang::En);
        assert_eq!(Lang::pick("ua", "en-US"), Lang::Uk);
        assert_eq!(Lang::pick("en", "uk-UA"), Lang::En);
        assert_eq!(Lang::pick("ru", "uk-UA,uk;q=0.9"), Lang::Uk);
        assert_eq!(Lang::pick("de", "de-DE"), Lang::En);
        assert_eq!(language("/launcher/v2/", Lang::Uk), Lang::Uk);
        assert_eq!(language("/launcher", Lang::En), Lang::En);
    }

    #[test]
    fn numbers_are_grouped_by_thousands() {
        for (value, text) in [
            (0, "0"),
            (999, "999"),
            (1000, "1&nbsp;000"),
            (1_234_567, "1&nbsp;234&nbsp;567"),
        ] {
            assert_eq!(grouped(value), text);
        }
    }

    #[test]
    fn the_offered_archive_is_its_checksum_and_size() {
        let dir = std::env::temp_dir().join(format!("srp-launcher-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("files.zip");
        std::fs::write(&file, b"abc").unwrap();
        let answer = client_files_of(Some(&file));
        assert_eq!(answer.status, "200 OK");
        assert_eq!(
            answer.body,
            "skhid-client-files 1\n\
             sha256 ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\nsize 3\n"
        );
        // Another archive in its place is seen without a restart.
        std::fs::write(
            &file,
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
        )
        .unwrap();
        assert!(client_files_of(Some(&file)).body.contains(
            "sha256 248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1\nsize 56\n"
        ));
        assert_eq!(
            client_files_of(Some(&dir.join("gone.zip"))).status,
            "503 Service Unavailable"
        );
        assert_eq!(client_files_of(None).status, "404 Not Found");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_archive_with_the_launcher_is_known_by_the_launchers_name() {
        let dir = std::env::temp_dir().join(format!("srp-launcher-name-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("files.zip");
        // Without it; then with it at the start, across the border of two
        // pieces of the read, and at the end.
        std::fs::write(&file, vec![b'x'; 200_000]).unwrap();
        assert!(!offered(&file).unwrap().launcher);
        for before in [
            0,
            64 * 1024 - 7,
            64 * 1024 + 20,
            2 * 64 * 1024 + 30,
            199_000,
        ] {
            let mut bytes = vec![b'x'; before];
            bytes.extend_from_slice(LAUNCHER_NAME);
            if before != 199_000 {
                bytes.resize(200_000, b'x');
            }
            std::fs::write(&file, &bytes).unwrap();
            let (sha256, launcher, extractor) = read_through(&file).unwrap();
            assert!(launcher, "{before}");
            assert!(!extractor, "{before}");
            let whole: String = openssl::sha::sha256(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(sha256, whole, "{before}");
        }
        // The game exe's name is not the launcher's.
        std::fs::write(&file, b"game/binaries/x86/survarium_skhid.exe").unwrap();
        assert!(!read_through(&file).unwrap().1);
        // The extractor is known by its own name, alone or with the
        // launcher, also across the border of two pieces of the read.
        for before in [0, 64 * 1024 - 5, 64 * 1024 + 20, 199_000] {
            let mut bytes = vec![b'x'; before];
            bytes.extend_from_slice(EXTRACTOR_NAME);
            std::fs::write(&file, &bytes).unwrap();
            let (_, launcher, extractor) = read_through(&file).unwrap();
            assert!(extractor && !launcher, "{before}");
            bytes.extend_from_slice(LAUNCHER_NAME);
            std::fs::write(&file, &bytes).unwrap();
            let found = offered(&file).unwrap();
            assert!(found.extractor && found.launcher, "{before}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
