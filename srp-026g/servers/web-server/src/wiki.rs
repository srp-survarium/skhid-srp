//! The wiki: its index and a weapons catalogue generated from the same released
//! dataset and images as the damage calculator. No separately maintained stats.
//! The modifiers ([`crate::modifiers`]) come from the server's game data.
//! The calculator ([`crate::damage`]) and the maps ([`crate::maps`]) are its
//! sections too, each a page of its own look.

use crate::{ContentType, Lang, Response, damage, escape, maps, modifiers};

pub(crate) const PATH: &str = "/wiki";
pub(crate) const WEAPONS: &str = "/wiki/weapons";
pub(crate) const MODIFIERS: &str = "/wiki/modifiers";
const SCRIPT: &[u8] = include_bytes!("../wiki/weapons.js");
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
    (path == "/wiki/weapons.js")
        .then_some(("text/javascript; charset=utf-8", SCRIPT))
        .or_else(|| modifiers::file(path))
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
    } else if path == MODIFIERS {
        (
            lang.tr("Modifiers", "Модифікатори"),
            modifiers::body(lang, modifiers::chosen(query)),
        )
    } else {
        (
            lang.tr("Wiki", "Вікі"),
            format!(
                "<div class=wiki-heading><h1>{}</h1></div><div class=wiki-sections>{}{}{}{}</div>",
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
    fn both_languages_have_weapon_stats_and_comparison() {
        for lang in [Lang::En, Lang::Uk] {
            let page = page(WEAPONS, "", lang, "site header");
            assert!(page.body.contains("site header"));
            assert!(page.body.contains("id=\"wpn_ak_74m\""));
            assert!(page.body.contains("class=\"weapon-details\""));
            assert!(page.body.contains(&format!("lang=\"{}\"", lang.code())));
            assert!(page.body.contains("id=\"compare-a\""));
            assert!(page.body.contains("id=\"sim-vertical\""));
        }
    }
}
