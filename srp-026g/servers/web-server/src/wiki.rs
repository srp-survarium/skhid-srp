//! First wiki section: a weapons catalogue generated from the same released
//! dataset and images as the damage calculator. No separately maintained stats.

use crate::{ContentType, Lang, Response, damage, escape};

pub(crate) const PATH: &str = "/wiki";
pub(crate) const WEAPONS: &str = "/wiki/weapons";
const SCRIPT: &[u8] = include_bytes!("../wiki/weapons.js");
const STYLE: &str = include_str!("../wiki/style.css");

pub(crate) fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    (path == "/wiki/weapons.js").then_some(("text/javascript; charset=utf-8", SCRIPT))
}

pub(crate) fn page(path: &str, lang: Lang, header: &str) -> Response {
    let (title, body) = if path == WEAPONS {
        let body = match lang {
            Lang::En => include_str!("../wiki/weapons.en.html"),
            Lang::Uk => include_str!("../wiki/weapons.uk.html"),
        };
        (lang.tr("Weapons", "Зброя"), body.to_owned())
    } else {
        (
            lang.tr("Wiki", "Вікі"),
            format!(
                "<div class=wiki-heading><h1>{}</h1></div>\
                 <a class=wiki-link href=\"/wiki/weapons?lang={}\"><h2>{} →</h2><p>{}</p></a>",
                lang.tr("Wiki", "Вікі"),
                lang.code(),
                lang.tr("Weapons", "Зброя"),
                lang.tr("Browse weapons, compare base stats and explore their damage and handling.",
                    "Переглядайте зброю, порівнюйте базові характеристики, ушкодження та швидкість перезаряджання."),
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
            let page = page(WEAPONS, lang, "site header");
            assert!(page.body.contains("site header"));
            assert!(page.body.contains("id=\"wpn_ak_74m\""));
            assert!(page.body.contains("class=\"weapon-details\""));
            assert!(page.body.contains(&format!("lang=\"{}\"", lang.code())));
            assert!(page.body.contains("id=\"compare-a\""));
            assert!(page.body.contains("id=\"sim-vertical\""));
        }
    }
}
