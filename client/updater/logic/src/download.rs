//! A file of many addresses (`client/updater/README.md`, "Mirrors"): what
//! the updater needs to move on from one that fails and to say why, with
//! no Windows in it. Every address of a file serves the same bytes (its
//! size and SHA-256 are pinned), so a download that broke off goes on at
//! the next address from the bytes already on disk.

use alloc::format;
use alloc::string::String;

/// Slower than this, in bytes a second, over [`SLOW_WINDOW_MS`]: the
/// address is left for the next one.
pub const MIN_SPEED: u64 = 50 * 1024;
/// How long a download is measured before it is called too slow, and
/// again for every such stretch after it.
pub const SLOW_WINDOW_MS: u64 = 120_000;
/// The most of a web page read for its title.
pub const PAGE_HEAD: usize = 8 * 1024;

/// The speed of one answer's body, stretch by stretch of
/// [`SLOW_WINDOW_MS`].
pub struct Speed {
    since: u64,
    bytes: u64,
}

impl Speed {
    /// From now (`ticks`, milliseconds) and the bytes received so far.
    pub fn new(ticks: u64, bytes: u64) -> Self {
        Self {
            since: ticks,
            bytes,
        }
    }

    /// `Some(bytes a second)` once a whole stretch averaged less than
    /// [`MIN_SPEED`]; a stretch that did not starts the next one.
    pub fn slow(&mut self, ticks: u64, bytes: u64) -> Option<u64> {
        let elapsed = ticks.saturating_sub(self.since);
        if elapsed < SLOW_WINDOW_MS {
            return None;
        }
        let speed = bytes.saturating_sub(self.bytes).saturating_mul(1000) / elapsed;
        if speed < MIN_SPEED {
            return Some(speed);
        }
        *self = Self::new(ticks, bytes);
        None
    }
}

/// Where the body of a `206 Partial Content` answer starts, by its
/// `Content-Range: bytes <first>-<last>/<size or *>`.
pub fn range_start(content_range: &str) -> Option<u64> {
    let range = content_range.trim().strip_prefix("bytes ")?;
    let (first, rest) = range.split_once('-')?;
    let (last, _) = rest.split_once('/')?;
    let number = |text: &str| -> Option<u64> {
        (!text.is_empty() && text.len() <= 19 && text.bytes().all(|c| c.is_ascii_digit()))
            .then(|| text.parse().ok())
            .flatten()
    };
    let first = number(first)?;
    (number(last)? >= first).then_some(first)
}

/// The `<title>` of a web page's first bytes (any host's: a quota page, a
/// sign-in page, a virus-scan page), as one line of at most 100
/// characters; `None` when they have none.
pub fn title(page: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(page);
    let lower = text.to_ascii_lowercase();
    let open = lower.find("<title")?;
    let start = open + lower[open..].find('>')? + 1;
    let end = start + lower[start..].find("</title")?;
    let mut title = String::new();
    for word in text[start..end].split_whitespace() {
        if !title.is_empty() {
            title.push(' ');
        }
        title.push_str(word);
    }
    let title = title
        .replace("&amp;", "&")
        .replace("&quot;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace('"', "'");
    let mut shown: String = title
        .chars()
        .filter(|c| !c.is_control())
        .take(100)
        .collect();
    if shown.len() < title.len() {
        shown.push_str("...");
    }
    (!shown.is_empty()).then_some(shown)
}

/// A line of the log about one address of a file: `<what>  <verb>
/// <host>: <detail>`, the verb `try`, `failed` or `ok`, as
/// `Skhid-without-updater.bat` writes them too:
///
/// ```text
/// skhid-game  failed   drive.usercontent.google.com: web page "Google Drive - Quota exceeded", not the file
/// ```
pub fn line(what: &str, verb: &str, host: &str, detail: &str) -> String {
    format!("{what}  {verb:<8} {host}: {detail}")
}

/// What a player is told when no address gave the file: each address's
/// host and why it failed, in their order, `; ` between.
pub fn failures(failed: &[(String, String)]) -> String {
    let mut out = String::new();
    for (host, why) in failed {
        if !out.is_empty() {
            out.push_str("; ");
        }
        out.push_str(&format!("{host}: {why}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stretch_under_the_least_speed_is_too_slow() {
        let mut speed = Speed::new(1000, 0);
        // Not judged before a whole stretch.
        assert_eq!(speed.slow(1000 + SLOW_WINDOW_MS - 1, 0), None);
        // Fast enough: the next stretch starts.
        let fast = MIN_SPEED * SLOW_WINDOW_MS / 1000;
        assert_eq!(speed.slow(1000 + SLOW_WINDOW_MS, fast), None);
        assert_eq!(speed.slow(1000 + SLOW_WINDOW_MS + 60_000, fast), None);
        // Then a stretch of a tenth of it.
        assert_eq!(
            speed.slow(1000 + 2 * SLOW_WINDOW_MS, fast + fast / 10),
            Some(MIN_SPEED / 10)
        );
        assert_eq!(Speed::new(0, 0).slow(SLOW_WINDOW_MS, 0), Some(0));
    }

    #[test]
    fn a_partial_answer_says_where_it_starts() {
        assert_eq!(range_start("bytes 100-2906544102/2906544103"), Some(100));
        assert_eq!(range_start(" bytes 0-0/*"), Some(0));
        for bad in [
            "",
            "bytes",
            "bytes */100",
            "bytes 5-4/10",
            "bytes -5/10",
            "items 0-1/2",
            "bytes 1-x/2",
        ] {
            assert_eq!(range_start(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_web_page_is_named_by_its_title() {
        let quota = b"<!DOCTYPE html><html><head><meta charset=\"utf-8\"><TITLE>Google Drive -\n  Quota exceeded</TITLE></head><body>...";
        assert_eq!(
            title(quota).as_deref(),
            Some("Google Drive - Quota exceeded")
        );
        assert_eq!(
            title(b"<title lang=en>Sign in &amp; \"go\"</title>").as_deref(),
            Some("Sign in & 'go'")
        );
        assert_eq!(title(b"<html><body>no title</body></html>"), None);
        assert_eq!(title(b"<title>   </title>"), None);
        assert_eq!(title(b"<title>cut off"), None);
        let long = format!("<title>{}</title>", "x".repeat(300));
        assert_eq!(title(long.as_bytes()).unwrap().len(), 103);
    }

    #[test]
    fn the_lines_of_the_log_and_the_players_words() {
        assert_eq!(
            line(
                "skhid-game",
                "failed",
                "drive.usercontent.google.com",
                "web page \"Google Drive - Quota exceeded\", not the file"
            ),
            "skhid-game  failed   drive.usercontent.google.com: web page \"Google Drive - Quota exceeded\", not the file"
        );
        assert_eq!(
            failures(&[
                ("2.28.121.133".into(), "connect timeout".into()),
                ("archive.org".into(), "HTTP 503".into())
            ]),
            "2.28.121.133: connect timeout; archive.org: HTTP 503"
        );
    }
}
