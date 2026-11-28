//! The wiki: its index, and a weapons catalogue and the simulators (recoil and
//! spread) generated from the same released dataset and images as the damage
//! calculator. No separately maintained stats.
//! The modifiers ([`crate::modifiers`]) and the equipment
//! ([`crate::equipment`]) come from the server's game data.
//! The calculator ([`crate::damage`]) and the maps ([`crate::maps`]) are its
//! sections too, each a page of its own look; the skills ([`crate::skills`])
//! are built like them, from the released game, into a page of the wiki's.
//! The quests ([`crate::quests`]) come from the server's game data and pay rules;
//! the modes ([`crate::modes`]) from its game data, its rules and the lobby's pools.

use crate::built::{Built, Script};
use crate::icons::{self, IconSet};
use crate::{
    ContentType, Lang, Response, artefacts, damage, equipment, escape, maps, modes, modifiers,
    quests, skills,
};

pub(crate) const PATH: &str = "/wiki";
pub(crate) const WEAPONS: &str = "/wiki/weapons";
pub(crate) const SIMULATORS: &str = "/wiki/simulators";
pub(crate) const MODIFIERS: &str = "/wiki/modifiers";
pub(crate) const EQUIPMENT: &str = "/wiki/equipment";
static SCRIPT: Script = Script::new(include_bytes!("../wiki/weapons.js"));
static SIMULATORS_SCRIPT: Script = Script::new(include_bytes!("../wiki/simulators.js"));
/// The weapons and the simulators, built in every language's places (`crate::built`).
static WEAPONS_PAGE: Built = Built::new(
    include_str!("../wiki/weapons.html"),
    include_str!("../wiki/weapons.texts.json"),
    &SCRIPT,
);
static SIMULATORS_PAGE: Built = Built::new(
    include_str!("../wiki/simulators.html"),
    include_str!("../wiki/simulators.texts.json"),
    &SIMULATORS_SCRIPT,
);
const STYLE: &str = include_str!("../wiki/style.css");

/// Where a page that moved into the wiki is now, for its old address.
pub(crate) fn moved(path: &str) -> Option<&'static str> {
    match path {
        "/damage" => Some(damage::PATH),
        "/maps" => Some(maps::PATH),
        _ => None,
    }
}

/// A section's picture on the wiki's index: the game's own, as the section
/// shows it.
enum Picture {
    /// A picture (its address with its version), whole: an item's, a
    /// skill's, a mode's icon, the factions' emblems.
    Image(Option<String>),
    /// A picture and its copy at twice the size (their addresses with their
    /// versions), for the browser to pick by the screen's density.
    Dense(Option<(String, String)>),
    /// Lobby glyphs (their addresses), in the accent colour.
    Glyphs(Vec<Option<String>>),
    /// Small pictures in their own colours (their addresses), two a row at
    /// about their files' size, so none is drawn larger than it is: the
    /// modes' loading-screen icons (64 px files of 61x55 bitmaps).
    Icons(Vec<Option<String>>),
    /// A drawing of the section's own (an SVG), in the page's colours.
    Drawing(Option<&'static str>),
}

impl Picture {
    /// The picture's markup; nothing for a file or drawing the build did not
    /// make (the tests require each).
    fn html(&self) -> String {
        match self {
            Self::Image(url) => url.as_deref().map_or_else(String::new, |url| {
                format!(
                    "<img src=\"{}\" alt=\"\" loading=lazy decoding=async>",
                    escape(url)
                )
            }),
            Self::Dense(urls) => urls.as_ref().map_or_else(String::new, |(url, double)| {
                format!(
                    "<img src=\"{}\" srcset=\"{} 1x, {} 2x\" alt=\"\" loading=lazy decoding=async>",
                    escape(url),
                    escape(url),
                    escape(double)
                )
            }),
            Self::Glyphs(urls) => {
                let glyphs: String = urls
                    .iter()
                    .flatten()
                    .map(|url| {
                        format!(
                            "<span class=glyph style=\"--icon:url('{}')\"></span>",
                            escape(url)
                        )
                    })
                    .collect();
                format!("<span class=wiki-glyphs aria-hidden=true>{glyphs}</span>")
            }
            Self::Icons(urls) => {
                let icons: String = urls
                    .iter()
                    .flatten()
                    .map(|url| {
                        format!(
                            "<img src=\"{}\" alt=\"\" loading=lazy decoding=async>",
                            escape(url)
                        )
                    })
                    .collect();
                format!("<span class=wiki-icons aria-hidden=true>{icons}</span>")
            }
            Self::Drawing(svg) => svg.unwrap_or_default().to_owned(),
        }
    }
}

/// The recoil and spread simulators' picture, in their plots' look: a burst
/// climbing inside its spread.
const BURST: &str = "<svg class=burst viewBox=\"0 0 100 100\" aria-hidden=true>\
    <circle class=\"sim-spread\" cx=\"50\" cy=\"38\" r=\"32\"/>\
    <path class=\"sim-curve\" d=\"M50 90C50 76 46 70 50 60S60 46 54 36 42 22 52 12\"/>\
    <circle class=\"sim-shot\" cx=\"50\" cy=\"90\" r=\"3.5\"/>\
    <circle class=\"sim-shot\" cx=\"48\" cy=\"73\" r=\"3.5\"/>\
    <circle class=\"sim-shot\" cx=\"50\" cy=\"60\" r=\"3.5\"/>\
    <circle class=\"sim-shot\" cx=\"54\" cy=\"36\" r=\"3.5\"/>\
    <circle class=\"sim-shot\" cx=\"52\" cy=\"12\" r=\"3.5\"/></svg>";

/// A section's tile on the wiki's index: its picture with its name on it, its
/// one line under them, a link to its address.
fn section(lang: Lang, path: &str, name: &str, line: &str, picture: &Picture) -> String {
    format!(
        "<a class=wiki-link href=\"{path}?lang={}\"><div class=wiki-pic>{}<h2>{}</h2></div><p>{}</p></a>",
        lang.code(),
        picture.html(),
        escape(name),
        escape(line),
    )
}

/// The wiki's sections in their index's order: what you play with (skills,
/// weapons, equipment and its modifiers), then where and how (modes, maps,
/// the artefacts found there, quests), then the tools. Each with its address,
/// name, line and picture.
fn sections(lang: Lang) -> [(&'static str, String, String, Picture); 10] {
    let item = |name: &str| Picture::Image(icons::url(IconSet::Item, name));
    let tile = |name: &str| {
        Picture::Dense(
            icons::url(IconSet::Tile, name).zip(icons::url(IconSet::Tile, &format!("{name}@2x"))),
        )
    };
    [
        (
            skills::PATH,
            lang.t("Skills"),
            lang.t("Levels and the experience they take; every skill tree: thresholds, ranks and what each rank gives."),
            Picture::Image(crate::versioned("/skills/icons/101.webp")),
        ),
        (
            WEAPONS,
            lang.t("Weapons"),
            lang.t("Browse weapons, compare base stats and explore their damage and handling."),
            item("ak_74m"),
        ),
        (
            EQUIPMENT,
            lang.t("Equipment"),
            lang.t("Every armour piece: protection, weight, built-in bonuses and what it can roll."),
            item("scav_resp_1"),
        ),
        (
            MODIFIERS,
            lang.t("Modifiers"),
            lang.t("Every modifier, its strengths and the items that can get it."),
            Picture::Glyphs(
                ["0", "16", "11", "7", "20", "33"]
                    .map(|icon| icons::url(IconSet::Modifier, icon))
                    .to_vec(),
            ),
        ),
        (
            modes::PATH,
            lang.t("Modes"),
            lang.t("Every mode: its rules, numbers and maps."),
            // Protective device, Team Deathmatch, Research, Artifact Hunt.
            Picture::Icons(
                ["1", "3", "2", "4"]
                    .map(|icon| icons::url(IconSet::Mode, icon))
                    .to_vec(),
            ),
        ),
        (
            maps::PATH,
            lang.t("Maps"),
            lang.t("Spawns, batteries, chargers, containers and artefacts on every map."),
            tile("maps"),
        ),
        (
            artefacts::PATH,
            lang.t("Artefacts"),
            lang.t("Every artefact: what it gives carried and fired, how copies stack, where it is found."),
            item("artefact_spring"),
        ),
        (
            quests::PATH,
            lang.t("Quests"),
            lang.t("Every faction quest: its goal at each reputation level and what it pays."),
            Picture::Image(icons::url(IconSet::Tile, "quests")),
        ),
        (
            damage::PATH,
            lang.t("Damage calculator"),
            lang.t("Shots to kill a target in any armour, at any distance."),
            Picture::Drawing(damage::doll()),
        ),
        (
            SIMULATORS,
            lang.t("Simulators"),
            lang.t("Recoil and spread of any weapon: aim, stance, skills, gear and injuries."),
            Picture::Drawing(Some(BURST)),
        ),
    ]
}

pub(crate) fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    const JS: &str = "text/javascript; charset=utf-8";
    match path {
        "/wiki/weapons.js" => Some((JS, SCRIPT.source())),
        "/wiki/simulators.js" => Some((JS, SIMULATORS_SCRIPT.source())),
        _ => None,
    }
    .or_else(|| modifiers::file(path))
    .or_else(|| equipment::file(path))
}

/// The wiki's page at `path` in `lang` under `header`; `query`: the
/// request's text after `?` (the modifiers page's chosen item);
/// `now`: what the skills page shows of the running server.
pub(crate) fn page(
    path: &str,
    query: &str,
    lang: Lang,
    header: &str,
    now: &skills::Now,
) -> Response {
    let (title, body) = if path == WEAPONS {
        (lang.t("Weapons"), WEAPONS_PAGE.page(lang))
    } else if path == SIMULATORS {
        (lang.t("Simulators"), SIMULATORS_PAGE.page(lang))
    } else if path == skills::PATH {
        (lang.t("Skills"), skills::body(lang, now))
    } else if path == quests::PATH {
        (lang.t("Quests"), quests::body(lang))
    } else if path == EQUIPMENT {
        (lang.t("Equipment"), equipment::body(lang))
    } else if path == artefacts::PATH {
        (lang.t("Artefacts"), artefacts::body(lang))
    } else if path == MODIFIERS {
        (
            lang.t("Modifiers"),
            modifiers::body(lang, modifiers::chosen(query)),
        )
    } else {
        (
            lang.t("Wiki"),
            format!(
                "<div class=wiki-heading><h1>{}</h1></div><div class=wiki-sections>{}</div>",
                lang.t("Wiki"),
                sections(lang)
                    .iter()
                    .map(|(path, name, line, picture)| section(lang, path, name, line, picture))
                    .collect::<String>(),
            ),
        )
    };
    framed(lang, &title, &lang.t("Skhid wiki"), header, &body)
}

/// A page in the wiki's look: `body` under `header`, titled `title · site`.
pub(crate) fn framed(lang: Lang, title: &str, site: &str, header: &str, body: &str) -> Response {
    Response {
        status: "200 OK",
        content_type: ContentType::Html,
        body: format!(
            "<!doctype html><html lang=\"{}\"><meta charset=utf-8>\
             <meta name=viewport content=\"width=device-width,initial-scale=1\">\
             <title>{} · {}</title><style>{}{}{STYLE}</style>{header}\
             <main class=wiki>{body}</main></html>",
            lang.code(),
            escape(title),
            escape(site),
            damage::font_faces(),
            crate::STYLE,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_weapon_stats_and_comparison() {
        for lang in Lang::ALL {
            let page = page(WEAPONS, "", lang, "site header", &skills::Now::default());
            assert!(page.body.contains("site header"));
            assert!(page.body.contains("id=\"wpn_ak_74m\""));
            assert!(page.body.contains("class=\"weapon-details\""));
            assert!(page.body.contains(&format!("lang=\"{}\"", lang.code())));
            assert!(page.body.contains("id=\"compare-a\""));
            assert!(page.body.contains(&format!("href=\"/wiki/simulators?lang={}", lang.code())));
            assert!(!page.body.contains('⟦'), "{lang:?}: a place left");
        }
    }

    #[test]
    fn every_language_has_the_simulators_and_name_only_files_they_serve() {
        for lang in Lang::ALL {
            let page = page(SIMULATORS, "", lang, "site header", &skills::Now::default());
            assert!(page.body.contains(&format!("lang=\"{}\"", lang.code())));
            assert!(page.body.contains("id=\"compare-a\""));
            assert!(page.body.contains("class=\"sim-vertical\""));
            assert!(page.body.contains("data-setup="));
            assert!(!page.body.contains('⟦'), "{lang:?}: a place left");
            let script = page.body.split_once("/wiki/simulators.js?v=").unwrap().1;
            assert!(script.starts_with(|c: char| c.is_ascii_hexdigit()));
            assert!(file("/wiki/simulators.js").is_some_and(|(_, bytes)| !bytes.is_empty()));
            let icons: Vec<&str> = page
                .body
                .match_indices("/skills/icons/")
                .map(|(at, _)| page.body[at..].split_once("?v=").unwrap().0)
                .collect();
            assert!(!icons.is_empty(), "the spread skills' icons");
            for icon in icons {
                assert!(skills::file(icon).is_some(), "{icon}");
            }
        }
    }

    #[test]
    fn the_skills_page_names_only_files_it_serves() {
        for lang in Lang::ALL {
            let page = page(
                skills::PATH,
                "",
                lang,
                "site header",
                &skills::Now::default(),
            );
            assert!(page.body.contains("site header"));
            assert!(page.body.contains(&format!("lang=\"{}\"", lang.code())));
            assert!(page.body.contains("class=\"skill-branch\""));
            assert!(page.body.contains("data-skill=\"101\""));
            assert!(!page.body.contains('⟦'), "{lang:?}: a place left");
            let files: Vec<&str> = page
                .body
                .match_indices("/skills/")
                .filter(|&(at, _)| page.body[..at].ends_with(['"', '\'']))
                .map(|(at, _)| page.body[at..].split_once("?v=").unwrap().0)
                .collect();
            assert!(files.iter().any(|file| file.starts_with("/skills/icons/")));
            assert!(files.contains(&"/skills/skills.js"));
            assert!(
                files
                    .iter()
                    .any(|file| file.starts_with("/skills/icons/branch-"))
            );
            for file in files {
                let (content_type, bytes) = skills::file(file).unwrap_or_else(|| panic!("{file}"));
                let kind = if file.ends_with(".js") {
                    "text/javascript; charset=utf-8"
                } else {
                    "image/webp"
                };
                assert_eq!(content_type, kind, "{file}");
                assert!(!bytes.is_empty(), "{file}");
            }
        }
    }
}
