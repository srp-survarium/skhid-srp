//! `GET /patches` and `GET /game` (the 0.26g page only): the files the
//! players' updater downloads (`client/updater/README.md`, "Patches" and
//! "Mirrors"), as JSON:
//!
//! ```text
//! /patches  {"patch_14":{"sha256":"<64 hex>","urls":["http://203.0.113.5/patches/resources.db.patch_14",
//!                                                   "https://mirror.example/resources.db.patch_14"]},...}
//! /game     {"sha256":"<64 hex>","size":2906544103,"urls":["http://203.0.113.5:8090/survarium-026g.7z",...]}
//! ```
//!
//! Each file's URLs are the updater's to try in their order: the release's
//! (the patches server, then the mirrors of the release's list), then the
//! rows of `mirrors.tsv` ([`Downloads`]). The `/patches` form is that of
//! updater 0.3.0, which reads the one-`url` form of the updaters before it
//! too; those never read this one: a release that brings 0.3.0 has them
//! install it first (the overlay comes before the patch step), and it does
//! the patches. A patch has no `size` there: the updaters take an entry of
//! exactly `sha256` and `urls`. The game's archive is apart, at `/game`:
//! `Skhid-without-updater.bat` takes every member of `/patches` for a
//! patch.
//!
//! They are the files the server's game data names (`survarium::v026g::
//! game_data`, `Provenance`), the patches in the game's order, so the
//! players run the archives the server runs on; `{}` when it was made of
//! `resources.db` alone. They are public, as the download is: the updater
//! asks them with no token. A 404 of `/patches` would keep PLAY off for
//! everyone, so it is answered whenever the 0.26g page is, whatever
//! `mirrors.tsv` holds; `/game` is 404 when the data names no archive (the
//! updater then uses the addresses built into it).

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use crate::{ContentType, Response};

/// A file the updater is told to fetch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfferedPatch {
    /// `patch_<n>` (the game's file `resources.db.patch_<n>`), or the
    /// game's archive's file name.
    pub name: String,
    /// The sha256 of the file, lowercase hex.
    pub sha256: String,
    /// Its size in bytes (said at `/game` only).
    pub size: u64,
    /// Where the updater downloads it: 1 to 8 URLs, tried in this order.
    pub urls: Vec<String>,
}

impl OfferedPatch {
    /// Its name in `mirrors.tsv`: the file's own (`resources.db.patch_14`,
    /// `survarium-026g.7z`).
    fn file_name(&self) -> String {
        if self.name.starts_with("patch_") {
            format!("resources.db.{}", self.name)
        } else {
            self.name.clone()
        }
    }
}

/// The most URLs a file has: the players' updater takes up to this many.
const MAX_URLS: usize = resources_db::server::MAX_URLS;

/// What is offered: the release's files (those of the server's game data)
/// with their URLs, and after those the rows of `mirrors.tsv` of srp's data
/// folder, one `<file> <url>` a line (tab or spaces between; `#` starts a
/// comment), the file the name of a file of the release: `resources.db.
/// patch_<n>` or the game's archive's. The file is read again when its
/// time or size changed (looked at each request, no restart). A row of a
/// file the release has not, of a URL the updater would refuse, of one the
/// file has already, or past the file's 8th URL is logged and skipped: it
/// never breaks the list.
pub struct Downloads {
    patches: Vec<OfferedPatch>,
    game: Option<OfferedPatch>,
    mirrors: Option<PathBuf>,
    now: Mutex<Now>,
}

/// The lists with the mirrors of `mirrors.tsv` as it was when read last.
struct Now {
    /// Its time and size then; `None`: there was no such file.
    stamp: Option<(SystemTime, u64)>,
    patches: Vec<OfferedPatch>,
    game: Option<OfferedPatch>,
}

impl Downloads {
    /// The release's `patches` and `game`; `mirrors`, the path of
    /// `mirrors.tsv` (`None`: no more than the release's).
    pub fn new(
        patches: Vec<OfferedPatch>,
        game: Option<OfferedPatch>,
        mirrors: Option<PathBuf>,
    ) -> Self {
        let now = Now {
            stamp: None,
            patches: patches.clone(),
            game: game.clone(),
        };
        let downloads = Self {
            patches,
            game,
            mirrors,
            now: Mutex::new(now),
        };
        drop(downloads.current());
        downloads
    }

    /// The patches, each with its mirrors after its own URLs.
    pub fn patches(&self) -> Vec<OfferedPatch> {
        self.current().patches.clone()
    }

    /// The game's archive, with its mirrors after its own URLs.
    pub fn game(&self) -> Option<OfferedPatch> {
        self.current().game.clone()
    }

    /// The lists, `mirrors.tsv` read again when it changed.
    fn current(&self) -> std::sync::MutexGuard<'_, Now> {
        let mut now = crate::locked(&self.now);
        let Some(path) = &self.mirrors else {
            return now;
        };
        let stamp = std::fs::metadata(path).ok().map(|meta| {
            (
                meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                meta.len(),
            )
        });
        if stamp == now.stamp {
            return now;
        }
        let text = match (stamp, std::fs::read_to_string(path)) {
            (None, _) => String::new(),
            (Some(_), Ok(text)) => text,
            (Some(_), Err(error)) => {
                log::warn!(
                    "{}: {error}: the release's URLs alone are offered",
                    path.display()
                );
                String::new()
            }
        };
        let mut patches = self.patches.clone();
        let mut game = self.game.clone();
        let taken = add_mirrors(&text, &mut patches, game.as_mut(), &mut |line, why| {
            log::warn!("{}:{line}: skipped: {why}", path.display());
        });
        log::info!(
            "{}: {}: {taken} mirror(s) offered after the release's URLs",
            path.display(),
            if stamp.is_some() { "read" } else { "not there" }
        );
        *now = Now {
            stamp,
            patches,
            game,
        };
        now
    }
}

/// Adds the rows of `mirrors` (the text of `mirrors.tsv`) to the files'
/// URLs, after those they have; `skipped(line, why)` for each row that is
/// not taken. How many were.
fn add_mirrors(
    mirrors: &str,
    patches: &mut [OfferedPatch],
    mut game: Option<&mut OfferedPatch>,
    skipped: &mut dyn FnMut(usize, String),
) -> usize {
    let mut taken = 0;
    for (at, line) in mirrors.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let [file, url] = words[..] else {
            skipped(at + 1, format!("{line:?} is not <file> <url>"));
            continue;
        };
        let offered = patches
            .iter_mut()
            .chain(game.as_deref_mut())
            .find(|offered| offered.file_name() == file);
        let Some(offered) = offered else {
            skipped(at + 1, format!("{file} is no file of the release's"));
            continue;
        };
        if !resources_db::server::is_url(url) {
            skipped(
                at + 1,
                format!("{url:?} is not a URL the players' updater takes"),
            );
        } else if offered.urls.iter().any(|known| known == url) {
            skipped(at + 1, format!("{file} has {url} already"));
        } else if offered.urls.len() >= MAX_URLS {
            skipped(
                at + 1,
                format!("{file} has {MAX_URLS} URLs already, the most the updater takes"),
            );
        } else {
            offered.urls.push(url.to_owned());
            taken += 1;
        }
    }
    taken
}

/// `text` as a JSON string. srp-game-data only writes names, checksums
/// and URLs that need no escape (the updater takes none), but a string is
/// never written unescaped.
fn string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn urls(file: &OfferedPatch) -> String {
    let urls: Vec<String> = file.urls.iter().map(|url| string(url)).collect();
    urls.join(",")
}

/// The list, in the order given.
pub(crate) fn json(patches: &[OfferedPatch]) -> String {
    let entries: Vec<String> = patches
        .iter()
        .map(|patch| {
            format!(
                "{}:{{\"sha256\":{},\"urls\":[{}]}}",
                string(&patch.name),
                string(&patch.sha256),
                urls(patch)
            )
        })
        .collect();
    format!("{{{}}}", entries.join(","))
}

/// The game's archive.
pub(crate) fn game_json(game: &OfferedPatch) -> String {
    format!(
        "{{\"sha256\":{},\"size\":{},\"urls\":[{}]}}",
        string(&game.sha256),
        game.size,
        urls(game)
    )
}

pub(crate) fn response(downloads: &Downloads) -> Response {
    Response {
        status: "200 OK",
        content_type: ContentType::Json,
        body: json(&downloads.patches()),
    }
}

/// `GET /game`; `None` (a 404) when the data names no archive.
pub(crate) fn game_response(downloads: &Downloads) -> Option<Response> {
    downloads.game().map(|game| Response {
        status: "200 OK",
        content_type: ContentType::Json,
        body: game_json(&game),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered(name: &str, urls: &[&str]) -> OfferedPatch {
        OfferedPatch {
            name: name.into(),
            sha256: "a".repeat(64),
            size: 7,
            urls: urls.iter().map(|url| (*url).to_owned()).collect(),
        }
    }

    #[test]
    fn mirrors_follow_the_releases_urls_and_a_bad_row_is_skipped() {
        let mut patches = vec![
            offered("patch_12", &["http://p/12"]),
            offered("patch_14", &["http://p/14"]),
        ];
        let seven: Vec<String> = (1..=7).map(|n| format!("http://g{n}/g.7z")).collect();
        let seven: Vec<&str> = seven.iter().map(String::as_str).collect();
        let mut game = offered("survarium-026g.7z", &seven);
        let mut skipped = Vec::new();
        let text = "# mirrors\n\
                    resources.db.patch_14\thttps://m.example/p14?id=1&x=2\n\
                    \n\
                    survarium-026g.7z   https://archive.org/download/x/g.7z\n\
                    survarium-026g.7z https://one.too.many/g.7z\n\
                    resources.db.patch_14 http://p/14\n\
                    resources.db.patch_99 http://m/99\n\
                    patch_12 http://m/12\n\
                    resources.db.patch_12 ftp://m/12\n\
                    resources.db.patch_12 http://m/a\"b\n\
                    resources.db.patch_12\n\
                    resources.db.patch_12 http://m/12 extra\n\
                    resources.db.patch_12 http://m2/12\r\n";
        let taken = add_mirrors(text, &mut patches, Some(&mut game), &mut |line, why| {
            skipped.push(format!("{line}: {why}"))
        });
        assert_eq!(taken, 3);
        assert_eq!(patches[0].urls, ["http://p/12", "http://m2/12"]);
        assert_eq!(
            patches[1].urls,
            ["http://p/14", "https://m.example/p14?id=1&x=2"]
        );
        assert_eq!(game.urls.len(), 8);
        assert_eq!(game.urls[7], "https://archive.org/download/x/g.7z");
        assert_eq!(
            skipped,
            [
                "5: survarium-026g.7z has 8 URLs already, the most the updater takes",
                "6: resources.db.patch_14 has http://p/14 already",
                "7: resources.db.patch_99 is no file of the release's",
                "8: patch_12 is no file of the release's",
                "9: \"ftp://m/12\" is not a URL the players' updater takes",
                "10: \"http://m/a\\\"b\" is not a URL the players' updater takes",
                "11: \"resources.db.patch_12\" is not <file> <url>",
                "12: \"resources.db.patch_12 http://m/12 extra\" is not <file> <url>",
            ]
        );
    }

    #[test]
    fn mirrors_tsv_is_read_again_when_it_changes() {
        let dir = std::env::temp_dir().join(format!("web-mirrors-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mirrors.tsv");
        let downloads = Downloads::new(
            vec![offered("patch_14", &["http://p/14"])],
            Some(offered("survarium-026g.7z", &["http://p/g.7z"])),
            Some(path.clone()),
        );
        // No file: the release's URLs.
        assert_eq!(downloads.patches()[0].urls, ["http://p/14"]);
        std::fs::write(&path, "survarium-026g.7z https://m/g.7z\n").unwrap();
        assert_eq!(
            downloads.game().unwrap().urls,
            ["http://p/g.7z", "https://m/g.7z"]
        );
        assert_eq!(
            game_json(&downloads.game().unwrap()),
            format!(
                "{{\"sha256\":\"{}\",\"size\":7,\"urls\":[\"http://p/g.7z\",\"https://m/g.7z\"]}}",
                "a".repeat(64)
            )
        );
        // Another size: read again.
        std::fs::write(
            &path,
            "resources.db.patch_14 https://m/p14\nnonsense\nsurvarium-026g.7z https://m/g.7z\n",
        )
        .unwrap();
        assert_eq!(
            downloads.patches()[0].urls,
            ["http://p/14", "https://m/p14"]
        );
        assert_eq!(downloads.game().unwrap().urls.len(), 2);
        // Gone: the release's alone.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(downloads.patches()[0].urls, ["http://p/14"]);
        assert_eq!(downloads.game().unwrap().urls, ["http://p/g.7z"]);
        // A folder in its place cannot be read: the release's alone.
        std::fs::create_dir(&path).unwrap();
        assert_eq!(downloads.patches()[0].urls, ["http://p/14"]);
        let _ = std::fs::remove_dir_all(&dir);
        // None at all.
        let bare = Downloads::new(vec![], None, None);
        assert_eq!(json(&bare.patches()), "{}");
        assert!(game_response(&bare).is_none());
    }
}
