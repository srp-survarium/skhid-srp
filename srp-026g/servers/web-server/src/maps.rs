//! The maps, `/wiki/maps` on the 0.26g site (a section of the wiki,
//! [`crate::wiki`]): every playable map with its respawn points, battery
//! spawns, chargers, containers and artefact spawns, built by
//! `srp-026g/scripts/maps-tab/build.py` into `../maps/` from the game data of
//! the released patches (its README says how). Every file is part of the
//! binary; `../maps/files.rs` lists them, written by the same build, at their
//! `/maps/` addresses. The top-down renders are the calculator's
//! (`/damage/maps/`).

use crate::{ContentType, Lang, Response};

/// The page's address.
pub(crate) const PATH: &str = "/wiki/maps";

/// The page's Content-Security-Policy: the calculator's (its own script,
/// images and fonts).
pub(crate) const POLICY: &str = crate::damage::POLICY;

/// The page, with `{{LANG}}` and `<!--SITE-HEADER-->` to fill.
const PAGE: &str = include_str!("../maps/maps.html");

macro_rules! files {
    ($($path:literal => $content_type:literal,)*) => {
        &[$((
            concat!("/maps/", $path),
            $content_type,
            include_bytes!(concat!("../maps/", $path)) as &[u8],
        ),)*]
    };
}

/// The files the page loads: address, content type, bytes.
const FILES: &[(&str, &str, &[u8])] = include!("../maps/files.rs");

/// The page's file at `path` (the target before any `?`): its content type
/// and bytes. Sent as the calculator's are ([`crate::damage::send`]).
pub(crate) fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    FILES
        .iter()
        .find(|(address, _, _)| *address == path)
        .map(|&(_, content_type, bytes)| (content_type, bytes))
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
