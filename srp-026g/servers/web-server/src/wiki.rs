//! The wiki, `/wiki` and `/wiki/<page>` on the 0.26g site: every item, map,
//! mode and rule of the game as Skhid's players have it, built by
//! `srp-026g/scripts/wiki/build.py` into `../wiki/` from the game data of
//! the released patches and srp's own rules (its README says how). Each page
//! is written once a language; `../wiki/pages.rs` and `../wiki/files.rs`
//! list them and the files they load, written by the same build. Every file
//! is part of the binary.

use crate::{ContentType, Lang, Response};

/// The wiki's address: its main page.
pub(crate) const PATH: &str = "/wiki";

/// The pages' Content-Security-Policy: the calculator's (their own script,
/// images and fonts).
pub(crate) const POLICY: &str = crate::damage::POLICY;

macro_rules! pages {
    ($($slug:literal => $file:literal,)*) => {
        &[$((
            $slug,
            include_str!(concat!("../wiki/pages/", $file, ".en.html")),
            include_str!(concat!("../wiki/pages/", $file, ".uk.html")),
        ),)*]
    };
}

/// The pages: the part of the address after `/wiki/` (empty for the main
/// page), English, Ukrainian; each with `<!--SITE-HEADER-->` to fill.
const PAGES: &[(&str, &str, &str)] = include!("../wiki/pages.rs");

macro_rules! files {
    ($($path:literal => $content_type:literal,)*) => {
        &[$((
            concat!("/wiki/", $path),
            $content_type,
            include_bytes!(concat!("../wiki/", $path)) as &[u8],
        ),)*]
    };
}

/// The files the pages load: address, content type, bytes.
const FILES: &[(&str, &str, &[u8])] = include!("../wiki/files.rs");

/// The page's file at `path` (the target before any `?`): its content type
/// and bytes. Sent as the calculator's are ([`crate::damage::send`]).
pub(crate) fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    FILES
        .iter()
        .find(|(address, _, _)| *address == path)
        .map(|&(_, content_type, bytes)| (content_type, bytes))
}

/// Whether `path` is the wiki's: `/wiki` or `/wiki/...`.
pub(crate) fn serves(path: &str) -> bool {
    path == PATH || path.starts_with("/wiki/")
}

/// The page at `path` in `lang`, under `header` (the site's); `None` when
/// the wiki has no such page.
pub(crate) fn page(path: &str, lang: Lang, header: &str) -> Option<Response> {
    let slug = path.strip_prefix(PATH)?;
    let slug = slug.strip_prefix('/').unwrap_or(slug);
    let &(_, en, uk) = PAGES.iter().find(|(name, _, _)| *name == slug)?;
    let page = match lang {
        Lang::En => en,
        Lang::Uk => uk,
    };
    Some(Response {
        status: "200 OK",
        content_type: ContentType::Html,
        body: page.replacen("<!--SITE-HEADER-->", header, 1),
    })
}

/// Every page's address, for the tests.
#[cfg(test)]
pub(crate) fn addresses() -> impl Iterator<Item = String> {
    PAGES.iter().map(|(slug, _, _)| {
        if slug.is_empty() {
            PATH.to_owned()
        } else {
            format!("{PATH}/{slug}")
        }
    })
}
