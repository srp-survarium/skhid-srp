//! The address of the server the updater asks: `https://host[:port][/path]`.
//!
//! The files it brings are programs, and the only thing that says they are
//! the server's own is the HTTPS connection. So plain `http://` is taken
//! only for this computer itself (a server under test), never for a host on
//! a network.

use alloc::string::String;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Url {
    pub secure: bool,
    pub host: String,
    pub port: u16,
    /// What every request path starts with: empty, or `/some/path` (no `/`
    /// at its end).
    pub prefix: String,
}

/// Reads a base address. `None` for anything else than the forms above.
pub fn parse(text: &str) -> Option<Url> {
    base(text, false)
}

/// `parse`; `any_http`: plain HTTP to any host too.
fn base(text: &str, any_http: bool) -> Option<Url> {
    let text = text.trim();
    let (secure, rest) = if let Some(rest) = text.strip_prefix("https://") {
        (true, rest)
    } else {
        (false, text.strip_prefix("http://")?)
    };
    let (authority, path) = match rest.find('/') {
        Some(at) => (&rest[..at], &rest[at..]),
        None => (rest, ""),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => {
            if port.is_empty() || port.len() > 5 || !port.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let port: u32 = port.parse().ok()?;
            if port == 0 || port > 65535 {
                return None;
            }
            (host, port as u16)
        }
        None => (authority, if secure { 443 } else { 80 }),
    };
    if host.is_empty()
        || host.len() > 253
        || host.starts_with(['.', '-'])
        || host.ends_with(['.', '-'])
        || !host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
    {
        return None;
    }
    if !secure && !any_http && !(host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1") {
        return None;
    }
    let prefix = path.trim_end_matches('/');
    if prefix.len() > 200
        || !prefix
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'/' | b'.' | b'_' | b'-' | b'~'))
    {
        return None;
    }
    Some(Url {
        secure,
        host: host.to_ascii_lowercase(),
        port,
        prefix: prefix.into(),
    })
}

/// The address of a patch file of `resources.db` (`patches`), or of the
/// game's archive (`patches::parse_game`): `http://` or
/// `https://`, any host, a bare IP address too, and a path. Plain HTTP is
/// taken here: the file is checked against the SHA-256 of the list, which
/// comes from the update server over its own address. No credentials, no
/// fragment, no space, control character, `"` or `\` (the list's strings
/// have no escapes, so it cannot carry those two anyway).
///
/// srp-game-data (srp-026g/crates/resources-db, `server::is_url`) takes
/// these URLs and no other, so that a URL this would refuse is refused when
/// the server's game data is built, not on the players' computers. Both are
/// tested with `tests/data/patch_urls.txt`: change the two and it together.
pub fn patch(text: &str) -> Option<Url> {
    if text.len() > 1024
        || text
            .bytes()
            .any(|c| !c.is_ascii_graphic() || matches!(c, b'\\' | b'#' | b'"'))
    {
        return None;
    }
    let scheme_end = text.find("://")? + 3;
    let at = scheme_end + text[scheme_end..].find('/')?;
    let path = &text[at..];
    if path.len() < 2 {
        return None;
    }
    let mut url = base(&text[..at], true)?;
    url.prefix = path.into();
    Some(url)
}

impl Url {
    /// The address as text: `http://` or `https://`, the host, the port
    /// when it is not the scheme's own, and the prefix (a patch's path and
    /// query).
    pub fn text(&self) -> String {
        let scheme = if self.secure { "https" } else { "http" };
        let default = if self.secure { 443 } else { 80 };
        if self.port == default {
            alloc::format!("{scheme}://{}{}", self.host, self.prefix)
        } else {
            alloc::format!("{scheme}://{}:{}{}", self.host, self.port, self.prefix)
        }
    }
}

/// The most redirects one download of a patch follows ([`redirect`]).
pub const MAX_REDIRECTS: u32 = 5;

/// Where a redirect from the patch address `from` ([`patch`]) leads: its
/// `Location` resolved against `from` (an absolute URL as it is; `//host/x`
/// with `from`'s scheme; `/x` and `?x` on `from`'s host; `x` beside
/// `from`'s path), and taken only if [`patch`] takes the result, as it
/// takes an address of the list: http or https, no credentials, no
/// fragment, and so on. From https to http is taken too: the file is
/// checked against the list's SHA-256 all the same. `None` for anything
/// else.
pub fn redirect(from: &Url, location: &str) -> Option<(String, Url)> {
    if location.is_empty() {
        return None;
    }
    let scheme = if from.secure { "https:" } else { "http:" };
    let origin = Url {
        prefix: String::new(),
        ..from.clone()
    }
    .text();
    let path = from.prefix.split('?').next().unwrap_or("");
    let text = if has_scheme(location) {
        location.into()
    } else if location.starts_with("//") {
        alloc::format!("{scheme}{location}")
    } else if location.starts_with('/') {
        alloc::format!("{origin}{location}")
    } else if location.starts_with('?') {
        alloc::format!("{origin}{path}{location}")
    } else {
        let folder = &path[..path.rfind('/').map_or(0, |at| at + 1)];
        alloc::format!("{origin}{folder}{location}")
    };
    let url = patch(&text)?;
    Some((text, url))
}

/// Whether `text` starts with a scheme (a letter, then letters, digits,
/// `+`, `-` or `.`, then `:`), as an absolute URL does.
fn has_scheme(text: &str) -> bool {
    match text.find(':') {
        Some(at) if at > 0 => {
            let scheme = &text.as_bytes()[..at];
            scheme[0].is_ascii_alphabetic()
                && scheme
                    .iter()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'))
        }
        _ => false,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn a_patch_may_be_on_any_host_over_http_or_https() {
        let u = patch("http://203.0.113.5/patch_14").unwrap();
        assert_eq!(
            (u.secure, u.host.as_str(), u.port, u.prefix.as_str()),
            (false, "203.0.113.5", 80, "/patch_14")
        );
        let u = patch("https://Files.Example.org:8443/db/patch_14?v=2").unwrap();
        assert_eq!(
            (u.secure, u.host.as_str(), u.port, u.prefix.as_str()),
            (true, "files.example.org", 8443, "/db/patch_14?v=2")
        );
        // What else is taken and what is not: `shared_patch_urls`.
    }

    /// The URLs of `tests/data/patch_urls.txt`, which srp-game-data's tests
    /// read too: each with whether it is taken.
    pub(crate) fn shared_patch_urls() -> Vec<(String, bool)> {
        let cases: Vec<(String, bool)> = include_str!("../tests/data/patch_urls.txt")
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                let (verdict, url) = line.split_once(' ').unwrap_or((line, ""));
                let taken = match verdict {
                    "ok" => true,
                    "bad" => false,
                    _ => panic!("patch_urls.txt: {line:?}"),
                };
                (expand(url), taken)
            })
            .collect();
        assert!(cases.iter().filter(|case| case.1).count() >= 10);
        assert!(cases.iter().filter(|case| !case.1).count() >= 40);
        cases
    }

    /// A URL of `patch_urls.txt` with its `{SP}`, ..., `{a*N}` written out.
    fn expand(text: &str) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(at) = rest.find('{') {
            out.push_str(&rest[..at]);
            let end = at + rest[at..].find('}').expect(text);
            match &rest[at + 1..end] {
                "SP" => out.push(' '),
                "TAB" => out.push('\t'),
                "CR" => out.push('\r'),
                "LF" => out.push('\n'),
                "NUL" => out.push('\0'),
                "DEL" => out.push('\x7f'),
                token => {
                    let n: usize = token
                        .strip_prefix("a*")
                        .and_then(|n| n.parse().ok())
                        .unwrap_or_else(|| panic!("patch_urls.txt: {text:?}"));
                    out.extend(core::iter::repeat_n('a', n));
                }
            }
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        out
    }

    #[test]
    fn the_patch_urls_srp_game_data_takes_too() {
        for (url, taken) in shared_patch_urls() {
            assert_eq!(patch(&url).is_some(), taken, "{url:?}");
        }
    }

    #[test]
    fn an_address_reads_back_as_its_text() {
        for text in [
            "http://203.0.113.5/patch_14",
            "https://files.example.org:8443/db/patch_14?v=2",
            "http://127.0.0.1:8080/x",
            "https://drive.usercontent.google.com/download?id=abc&export=download&confirm=t",
        ] {
            assert_eq!(patch(text).unwrap().text(), text);
        }
        // The scheme's own port is not written; the host is lowercase.
        assert_eq!(
            patch("https://Host:443/x").unwrap().text(),
            "https://host/x"
        );
        assert_eq!(patch("http://host:80/x").unwrap().text(), "http://host/x");
    }

    #[test]
    fn a_redirect_is_resolved_against_the_address_and_checked() {
        let from = patch("https://files.example.org:8443/db/patch_14?v=2").unwrap();
        let to = |location: &str| redirect(&from, location).map(|(text, _)| text);
        for (location, expected) in [
            ("https://other.example/x", "https://other.example/x"),
            // Down to plain HTTP: the list's SHA-256 decides.
            (
                "http://203.0.113.5:8090/p/patch_14",
                "http://203.0.113.5:8090/p/patch_14",
            ),
            ("//cdn.example/f?id=1", "https://cdn.example/f?id=1"),
            (
                "/elsewhere/patch_14",
                "https://files.example.org:8443/elsewhere/patch_14",
            ),
            ("?v=3", "https://files.example.org:8443/db/patch_14?v=3"),
            ("patch_15", "https://files.example.org:8443/db/patch_15"),
            (
                "sub/patch_15?x=1",
                "https://files.example.org:8443/db/sub/patch_15?x=1",
            ),
            (
                "https://drive.usercontent.google.com/download?id=abc&export=download&confirm=t",
                "https://drive.usercontent.google.com/download?id=abc&export=download&confirm=t",
            ),
        ] {
            assert_eq!(to(location).as_deref(), Some(expected), "{location}");
        }
        let (_, url) = redirect(&from, "http://203.0.113.5:8090/p/patch_14").unwrap();
        assert_eq!(
            (url.secure, url.host.as_str(), url.port, url.prefix.as_str()),
            (false, "203.0.113.5", 8090, "/p/patch_14")
        );
        // What a list may not name, a redirect may not lead to.
        for location in [
            "",
            "ftp://203.0.113.5/patch_14",
            "file:///C:/Windows/patch_14",
            "javascript:alert(1)",
            "data:text/plain,x",
            "HTTP://host/patch_14",
            "https://user:pw@host/patch_14",
            "https://host/patch_14#x",
            "https://host",
            "https://host/",
            "//",
            "https://[::1]/x",
            "/a b",
            "/a\\b",
            "/a\"b",
        ] {
            assert_eq!(to(location), None, "{location:?}");
        }
        assert_eq!(to(&alloc::format!("/{}", "a".repeat(1100))), None);
        // From an address without a query or a folder.
        let from = patch("http://h/p").unwrap();
        assert_eq!(redirect(&from, "q").unwrap().0, "http://h/q");
        assert_eq!(redirect(&from, "?x").unwrap().0, "http://h/p?x");
    }

    #[test]
    fn the_forms_that_are_taken() {
        let url = parse("https://skhid.link").unwrap();
        assert_eq!(
            (url.secure, url.host.as_str(), url.port, url.prefix.as_str()),
            (true, "skhid.link", 443, "")
        );
        let url = parse(" https://Test.Skhid.Link:8443/srp/ \r\n").unwrap();
        assert_eq!(
            (url.host.as_str(), url.port, url.prefix.as_str()),
            ("test.skhid.link", 8443, "/srp")
        );
        let url = parse("http://127.0.0.1:8088").unwrap();
        assert_eq!(
            (url.secure, url.host.as_str(), url.port),
            (false, "127.0.0.1", 8088)
        );
        assert_eq!(parse("http://localhost").unwrap().port, 80);
    }

    #[test]
    fn everything_else_is_refused() {
        for bad in [
            "",
            "skhid.link",
            "ftp://skhid.link",
            "https://",
            "https:///x",
            "https://a b",
            "https://user@skhid.link",
            "https://skhid.link:0",
            "https://skhid.link:65536",
            "https://skhid.link:",
            "https://skhid.link:80a",
            "https://-a.b",
            "https://a.b.",
            "https://skhid.link/?x=1",
            "https://skhid.link/a b",
            "https://skhid.link/#x",
            // Plain HTTP to a host on a network: the files would be anybody's.
            "http://skhid.link",
            "http://192.168.1.10:8088",
            "http://127.0.0.2",
            "http://localhost.evil.example",
        ] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
    }
}
