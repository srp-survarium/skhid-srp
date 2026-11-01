//! The wiki: its index and a weapons catalogue generated from the same released
//! dataset and images as the damage calculator. No separately maintained stats.
//! The modifiers ([`crate::modifiers`]) and the equipment
//! ([`crate::equipment`]) come from the server's game data.
//! The calculator ([`crate::damage`]) and the maps ([`crate::maps`]) are its
//! sections too, each a page of its own look. The stats (`/wiki/stats`) are a
//! frame its script fills from the match history's JSON (`/api/history/...`,
//! docs/notes/match_history.md) when it is viewed.

use std::sync::OnceLock;

use crate::{ContentType, Lang, Response, damage, equipment, escape, maps, modifiers};

pub(crate) const PATH: &str = "/wiki";
pub(crate) const WEAPONS: &str = "/wiki/weapons";
pub(crate) const MODIFIERS: &str = "/wiki/modifiers";
pub(crate) const EQUIPMENT: &str = "/wiki/equipment";
pub(crate) const STATS: &str = "/wiki/stats";
/// The stats page's: the wiki's ([`damage::POLICY`]), and its script may ask
/// the site for the history's JSON.
pub(crate) const STATS_POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; \
    script-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; form-action 'self'";
const SCRIPT: &[u8] = include_bytes!("../wiki/weapons.js");
const STATS_SCRIPT: &[u8] = include_bytes!("../wiki/history.js");
const STATS_SCRIPT_PATH: &str = "/wiki/history.js";
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

/// The stats page's script's address with its version, so it can be kept for good.
fn stats_script_url() -> &'static str {
    static URL: OnceLock<String> = OnceLock::new();
    URL.get_or_init(|| {
        let digest: String = openssl::sha::sha256(STATS_SCRIPT)[..6]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("{STATS_SCRIPT_PATH}?v={digest}")
    })
}

/// The stats page's frame. Its script fills it, as the address says: a
/// player (`?player=<name>`), a match (`?match=<number>`), a clan
/// (`?clan=<tag>`), else the leaderboards.
fn stats_body(lang: Lang) -> String {
    format!(
        "<p class=breadcrumb><a href=\"{PATH}?lang={code}\">{wiki}</a> / \
         <a href=\"{STATS}?lang={code}\">{stats}</a><span id=stats-crumb></span></p>\
         <div class=wiki-heading><div><p class=eyebrow id=stats-eyebrow>{eyebrow}</p>\
         <h1 id=stats-title>{stats}</h1></div></div>\
         <div id=stats><noscript><p class=wiki-intro>{noscript}</p></noscript></div>\
         <script src=\"{}\"></script>",
        stats_script_url(),
        code = lang.code(),
        wiki = lang.tr("Wiki", "Вікі"),
        stats = lang.tr("Stats", "Статистика"),
        eyebrow = lang.tr("Public matches", "Публічні матчі"),
        noscript = lang.tr("The stats need JavaScript.", "Статистиці потрібен JavaScript."),
    )
}

pub(crate) fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    (path == "/wiki/weapons.js")
        .then_some(("text/javascript; charset=utf-8", SCRIPT))
        .or_else(|| {
            (path == STATS_SCRIPT_PATH).then_some(("text/javascript; charset=utf-8", STATS_SCRIPT))
        })
        .or_else(|| modifiers::file(path))
        .or_else(|| equipment::file(path))
}

/// The wiki's page at `path` in `lang` under `header`; `query`: the
/// request's text after `?` (the modifiers page's chosen item).
pub(crate) fn page(path: &str, query: &str, lang: Lang, header: &str) -> Response {
    let (title, body) = if path == WEAPONS {
        let body = match lang {
            Lang::En => include_str!("../wiki/weapons.en.html"),
            Lang::Uk => include_str!("../wiki/weapons.uk.html"),
        };
        (lang.tr("Weapons", "Зброя"), body.to_owned())
    } else if path == STATS {
        (lang.tr("Stats", "Статистика"), stats_body(lang))
    } else if path == EQUIPMENT {
        (lang.tr("Equipment", "Спорядження"), equipment::body(lang))
    } else if path == MODIFIERS {
        (
            lang.tr("Modifiers", "Модифікатори"),
            modifiers::body(lang, modifiers::chosen(query)),
        )
    } else {
        (
            lang.tr("Wiki", "Вікі"),
            format!(
                "<div class=wiki-heading><h1>{}</h1></div><div class=wiki-sections>{}{}{}{}{}{}</div>",
                lang.tr("Wiki", "Вікі"),
                section(
                    lang,
                    WEAPONS,
                    &lang.tr("Weapons", "Зброя"),
                    &lang.tr("Browse weapons, compare base stats and explore their damage and handling.",
                        "Переглядайте зброю, порівнюйте базові характеристики, ушкодження та швидкість перезаряджання."),
                ),
                section(
                    lang,
                    EQUIPMENT,
                    &lang.tr("Equipment", "Спорядження"),
                    &lang.tr("Every armour piece: protection, weight, built-in bonuses and what it can roll.",
                        "Усе спорядження: захист, вага, вбудовані властивості й модифікатори."),
                ),
                section(
                    lang,
                    MODIFIERS,
                    &lang.tr("Modifiers", "Модифікатори"),
                    &lang.tr("Every modifier, its strengths and the items that can get it.",
                        "Усі модифікатори, їхня сила та предмети, на які вони випадають."),
                ),
                section(
                    lang,
                    damage::PATH,
                    &lang.tr("Damage calculator", "Калькулятор шкоди"),
                    &lang.tr("Shots to kill a target in any armour, at any distance.",
                        "Скільки влучань треба, щоб убити ціль у будь-якій броні на будь-якій відстані."),
                ),
                section(
                    lang,
                    maps::PATH,
                    &lang.tr("Maps", "Мапи"),
                    &lang.tr("Spawns, batteries, chargers, containers and artefacts on every map.",
                        "Точки появи, батареї, зарядні пристрої, контейнери й артефакти на кожній мапі."),
                ),
                section(
                    lang,
                    STATS,
                    &lang.tr("Stats", "Статистика"),
                    &lang.tr("Players, matches, clans and leaderboards.",
                        "Гравці, матчі, клани й таблиці лідерів."),
                ),
            ),
        )
    };
    Response {
        status: "200 OK",
        content_type: ContentType::Html,
        body: format!(
            "<!doctype html><html lang=\"{}\"><meta charset=utf-8>\
             <meta name=viewport content=\"width=device-width,initial-scale=1\">\
             <title>{} · {}</title><style>{}{}{STYLE}</style>{header}\
             <main class=wiki>{body}</main></html>",
            lang.code(),
            escape(&title),
            lang.tr("Skhid wiki", "Вікі Сходу"),
            damage::font_faces(),
            crate::STYLE,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_uses_the_current_calculator_dataset() {
        let digest: String = openssl::sha::sha256(include_bytes!("../damage/calc.js"))
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            digest,
            include_str!("../wiki/source.sha256").trim(),
            "rebuild scripts/wiki/build.py after changing the calculator data"
        );
    }

    #[test]
    fn the_stats_page_is_a_frame_for_its_script() {
        for lang in [Lang::En, Lang::Uk] {
            let stats = page(STATS, "player=Sheep", lang, "site header").body;
            assert!(stats.contains("site header"));
            assert!(stats.contains("<div id=stats>"));
            assert!(stats.contains(stats_script_url()));
        }
        assert_eq!(file(STATS_SCRIPT_PATH).map(|(_, bytes)| bytes), Some(STATS_SCRIPT));
        assert!(STATS_POLICY.contains("connect-src 'self'"));
    }

    #[test]
    fn both_languages_have_weapon_stats_and_comparison() {
        for lang in [Lang::En, Lang::Uk] {
            let page = page(WEAPONS, "", lang, "site header");
            assert!(page.body.contains("site header"));
            assert!(page.body.contains("id=\"wpn_ak_74m\""));
            assert!(page.body.contains("class=\"weapon-details\""));
            assert!(page.body.contains(&format!("lang=\"{}\"", lang.code())));
            assert!(page.body.contains("id=\"compare-a\""));
            assert!(page.body.contains("class=\"sim-vertical\""));
            assert!(page.body.contains("id=\"recoil-template\""));
        }
    }
}
