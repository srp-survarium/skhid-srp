//! The skills, `/wiki/skills` on the 0.26g site (a section of the wiki,
//! [`crate::wiki`]): every branch of the skill tree as the lobby draws it,
//! with each skill's ranks and values, built by
//! `srp-026g/scripts/skills-tab/build.py` into `../skills/` from the game data
//! of the released patches. Every file is part of the binary; `../skills/files.rs`
//! lists the icons, written by the same build, at their `/skills/` addresses.

use crate::Lang;

/// The page's address.
pub(crate) const PATH: &str = "/wiki/skills";

macro_rules! files {
    ($($path:literal => $content_type:literal,)*) => {
        &[$((
            concat!("/skills/", $path),
            $content_type,
            include_bytes!(concat!("../skills/", $path)) as &[u8],
        ),)*]
    };
}

/// The files the page loads: address, content type, bytes.
const FILES: &[(&str, &str, &[u8])] = include!("../skills/files.rs");

/// The page's file at `path` (the target before any `?`): its content type
/// and bytes.
pub(crate) fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    FILES
        .iter()
        .find(|(address, _, _)| *address == path)
        .map(|&(_, content_type, bytes)| (content_type, bytes))
}

/// The page's body in `lang`, for the wiki's page.
pub(crate) fn body(lang: Lang) -> &'static str {
    match lang {
        Lang::En => include_str!("../skills/skills.en.html"),
        Lang::Uk => include_str!("../skills/skills.uk.html"),
    }
}
