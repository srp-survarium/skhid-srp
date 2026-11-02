//! The damage calculator, `/wiki/calculator` on the 0.26g site (a section of
//! the wiki, [`crate::wiki`]): one page and the files it loads, built by
//! `srp-026g/scripts/damage-calc/build_page.py --site` into `../damage/` from
//! the game data of the latest content patch (the script's README says how).
//! Every file is part of the binary. The page names each with `?v=<its hash>`,
//! so they are sent to be kept for good. The files keep their `/damage/`
//! addresses.

use std::io::Write;
use std::net::{Shutdown, TcpStream};

use crate::{ContentType, Lang, Response};

/// The page's address.
pub(crate) const PATH: &str = "/wiki/calculator";

/// The page's Content-Security-Policy: the site's, with its own script,
/// images (its files and the icons inside the script) and fonts.
pub(crate) const POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; \
    script-src 'self'; img-src 'self' data:; font-src 'self'; form-action 'self'";

/// Where the fonts are, which every page of the site uses (both servers).
pub(crate) const FONTS: &str = "/damage/fonts/";

/// The page's `@font-face` rules (the fonts' addresses with their hashes),
/// for the site's other pages.
pub(crate) fn font_faces() -> &'static str {
    static FACES: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    FACES.get_or_init(|| {
        PAGE.lines()
            .filter(|line| line.starts_with("@font-face"))
            .collect()
    })
}

/// The page, with `{{LANG}}` and `<!--SITE-HEADER-->` to fill.
const PAGE: &str = include_str!("../damage/damage.html");

/// The files the page loads: address, content type, bytes. Every file of
/// `../damage/` but the page and its record (`../build.rs` lists them). The
/// renders are the Maps tab's too ([`crate::maps`]), which has every map's.
const FILES: &[(&str, &str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/damage_files.rs"));

/// The page's file at `path` (the target before any `?`): its content type
/// and bytes.
pub(crate) fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    FILES
        .iter()
        .find(|(address, _, _)| *address == path)
        .map(|&(_, content_type, bytes)| (content_type, bytes))
}

/// Sends one of the page's files, to be kept: its address changes with it.
pub(crate) fn send(stream: &mut TcpStream, head_only: bool, content_type: &str, bytes: &[u8]) {
    let head = format!(
        "HTTP/1.0 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: public, max-age=31536000, immutable\r\n\
         X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        bytes.len()
    );
    let _ = stream.write_all(head.as_bytes());
    if !head_only {
        let _ = stream.write_all(bytes);
    }
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Write);
}

/// The page in `lang`, under `header` (the site's).
pub(crate) fn page(lang: Lang, header: &str) -> Response {
    Response {
        status: "200 OK",
        content_type: ContentType::Html,
        body: PAGE
            .replacen("{{LANG}}", lang.code(), 1)
            .replacen("<!--SITE-HEADER-->", header, 1),
    }
}

/// The site's header as the calculator and the maps show it: the site
/// stylesheet's rules for it (`crate::STYLE`), under `.site`, so neither
/// page's rules reach the other's, in the page's own tokens (both pages
/// name them so). The calculator's own `.tag` is undone for the header's line.
pub(crate) const HEADER_STYLE: &str = ".site{font:15px/1.55 var(--font-body);color:var(--ink);\
margin:-20px calc(-1*max(16px,env(safe-area-inset-right,0px))) 22px calc(-1*max(16px,env(safe-area-inset-left,0px)))}\
.site header{display:flex;flex-wrap:wrap;align-items:center;gap:.5rem 1.4rem;padding:.75rem max(16px,env(safe-area-inset-left,0px));background:var(--bg);border-bottom:2px solid var(--ink)}\
.site .brand{font:700 1.5rem var(--font-display);letter-spacing:.24em;color:var(--accent);text-decoration:none}\
.site .tag{display:block;font:400 .72rem/1.55 var(--font-data);letter-spacing:0;text-transform:none;border:0;padding:0;border-radius:0;background:none;color:var(--ink-3);flex:1;min-width:12rem}\
.site nav{display:flex;flex-wrap:wrap;align-items:center;row-gap:.3rem}\
.site nav a{font:700 .85rem var(--font-display);text-transform:uppercase;letter-spacing:.1em;color:var(--ink-2);text-decoration:none;padding:0 .8rem;line-height:1.1}\
.site nav a+a{border-left:2px solid var(--ink-3)}.site nav a:first-child{padding-left:0}\
.site nav a:hover{color:var(--ink)}.site nav a[aria-current]{color:var(--accent)}\
.site .lang{font:.8rem var(--font-body);color:var(--ink-2);text-decoration:none;border:1px solid var(--rule);background:var(--bg);padding:.15rem .7rem;border-radius:999px}\
.site .lang:hover{border-color:var(--ink-3);color:var(--ink)}\
.site .badge{background:#d64531;color:#fff;font:700 .75rem system-ui,sans-serif;letter-spacing:.14em;padding:.2rem .55rem;border-radius:2px}\
.site .note.test{max-width:1240px;margin:1rem auto 0;font-size:1rem;color:var(--ink-2);background:var(--bg);border-left:3px solid #d64531;padding:.7rem .9rem;border-radius:2px}\
.site a{color:var(--accent)}\
.site .breadcrumb{box-sizing:content-box;max-width:1240px;margin:14px auto 0;padding:0 max(16px,env(safe-area-inset-right,0px)) 0 max(16px,env(safe-area-inset-left,0px));font-size:.8rem;color:var(--ink-3)}\
.site .breadcrumb a{color:var(--ink-2);text-decoration:none}";
