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

/// A section's link on the wiki's index: its address, name and one line.
fn section(lang: Lang, path: &str, name: &str, line: &str) -> String {
    format!(
        "<a class=wiki-link href=\"{path}?lang={}\"><h2>{} →</h2><p>{}</p></a>",
        lang.code(),
        escape(name),
        escape(line),
    )
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
                "<div class=wiki-heading><h1>{}</h1></div><div class=wiki-sections>{}{}{}{}{}{}{}{}{}{}</div>",
                lang.t("Wiki"),
                section(
                    lang,
                    WEAPONS,
                    &lang.t("Weapons"),
                    &lang.t("Browse weapons, compare base stats and explore their damage and handling."),
                ),
                section(
                    lang,
                    SIMULATORS,
                    &lang.t("Simulators"),
                    &lang.t("Recoil and spread of any weapon: aim, stance, skills, gear and injuries."),
                ),
                section(
                    lang,
                    EQUIPMENT,
                    &lang.t("Equipment"),
                    &lang.t("Every armour piece: protection, weight, built-in bonuses and what it can roll."),
                ),
                section(
                    lang,
                    artefacts::PATH,
                    &lang.t("Artefacts"),
                    &lang.t("Every artefact: what it gives carried and fired, how copies stack, where it is found."),
                ),
                section(
                    lang,
                    MODIFIERS,
                    &lang.t("Modifiers"),
                    &lang.t("Every modifier, its strengths and the items that can get it."),
                ),
                section(
                    lang,
                    skills::PATH,
                    &lang.t("Skills"),
                    &lang.t("Levels and the experience they take; every skill tree: thresholds, ranks and what each rank gives."),
                ),
                section(
                    lang,
                    quests::PATH,
                    &lang.t("Quests"),
                    &lang.t("Every faction quest: its goal at each reputation level and what it pays."),
                ),
                section(
                    lang,
                    damage::PATH,
                    &lang.t("Damage calculator"),
                    &lang.t("Shots to kill a target in any armour, at any distance."),
                ),
                section(
                    lang,
                    maps::PATH,
                    &lang.t("Maps"),
                    &lang.t("Spawns, batteries, chargers, containers and artefacts on every map."),
                ),
                section(
                    lang,
                    modes::PATH,
                    &lang.t("Modes"),
                    &lang.t("Every mode: its rules, numbers and maps."),
                ),
            ),
        )
    };
    framed(
        lang,
        &title,
        &lang.t("Skhid wiki"),
        header,
        &body,
    )
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
