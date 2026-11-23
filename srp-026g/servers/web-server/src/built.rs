//! The pages built at a release (`release/build-site`), in the reader's language. A built page
//! is one copy for every language: where it says something, its markup has a place `⟦n⟧`, and
//! its table (`<page>.texts.json`, made with it by `srp-026g/scripts/site-texts/texts.py`) says
//! what goes there: a text of the site's (its row in `../words/`), with its places filled or
//! counted, or the game's text in each language (the build read the game's dictionaries).
//! `⟦words⟧` is the words its script asks for (`T("…")`, `site-texts/words.js`), as JSON, and
//! `⟦lang⟧` the language's code.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::{Map, Value};

use crate::{Lang, words};

/// A built page: its markup, its table, and the script that reads `⟦words⟧`.
pub(crate) struct Built {
    markup: &'static str,
    table: &'static str,
    script: &'static Script,
    texts: OnceLock<Vec<Text>>,
}

/// A page's script, which says the site's words it asks for (`T("…")`, `site-texts/words.js`).
pub(crate) struct Script {
    source: &'static [u8],
    asked: OnceLock<Vec<Asked>>,
}

/// A text the site has in `words/`: its English and its context ("" for none).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Asked {
    pub(crate) english: String,
    pub(crate) context: String,
}

/// One text of a table.
#[derive(Debug, PartialEq)]
enum Text {
    /// The site's text, its `{places}` filled.
    Word {
        asked: Asked,
        places: Vec<(String, String)>,
    },
    /// The site's text for a count, its place `{n}`: `one` for one, `other` for more, each
    /// language's form of it by the count (contexts `one`, `few`, `many`).
    Counted {
        one: String,
        other: String,
        places: Vec<(String, String)>,
    },
    /// The game's text, in [`Lang::ALL`]'s order.
    Game(Vec<String>),
    /// Markup the build made of the game's texts, in [`Lang::ALL`]'s order.
    Html(Vec<String>),
}

/// The contexts of a counted text's forms.
#[cfg(test)]
const FORMS: [&str; 3] = ["one", "few", "many"];

/// An object's fields, each a text, by name; `None` if it is no object of texts.
fn texts_of(value: &Value) -> Option<BTreeMap<&str, &str>> {
    value
        .as_object()?
        .iter()
        .map(|(name, text)| Some((name.as_str(), text.as_str()?)))
        .collect()
}

impl Text {
    /// A table's row: `{"word", "context"?, "places"?}`, `{"one", "other", "places"}` (its
    /// places with an `n`), or `{"game"}` / `{"html"}`, a text in each language.
    fn of(row: &Value) -> Result<Self, String> {
        let wrong = |why: &str| format!("{row}: {why}");
        let object = row.as_object().ok_or_else(|| wrong("not an object"))?;
        let places: Vec<(String, String)> = match object.get("places") {
            Some(places) => texts_of(places).ok_or_else(|| wrong("places not texts"))?,
            None => BTreeMap::new(),
        }
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
        let mut keys: Vec<&str> = object
            .keys()
            .map(String::as_str)
            .filter(|key| *key != "places")
            .collect();
        keys.sort_unstable();
        let text = |name: &str| object.get(name).and_then(Value::as_str).map(str::to_owned);
        let in_every_language = |name: &str| -> Result<Vec<String>, String> {
            let by_code = texts_of(&object[name]).ok_or_else(|| wrong("not a text a language"))?;
            if by_code.len() != Lang::ALL.len() {
                return Err(wrong(&format!(
                    "{} languages, not {}",
                    by_code.len(),
                    Lang::ALL.len()
                )));
            }
            Lang::ALL
                .iter()
                .map(|lang| by_code.get(lang.code()).map(|text| (*text).to_owned()))
                .collect::<Option<_>>()
                .ok_or_else(|| wrong("a language missing"))
        };
        match (keys.as_slice(), text("word"), text("one"), text("other")) {
            (["word"] | ["context", "word"], Some(english), ..) => Ok(Self::Word {
                asked: Asked {
                    english,
                    context: text("context").unwrap_or_default(),
                },
                places,
            }),
            (["one", "other"], None, Some(one), Some(other))
                if places.iter().any(|(name, _)| name == "n") =>
            {
                Ok(Self::Counted { one, other, places })
            }
            (["game"], ..) => Ok(Self::Game(in_every_language("game")?)),
            (["html"], ..) => Ok(Self::Html(in_every_language("html")?)),
            _ => Err(wrong("no text of a table")),
        }
    }

    /// What the site's words file must have for it.
    #[cfg(test)]
    fn asked(&self) -> Vec<Asked> {
        match self {
            Self::Word { asked, .. } => vec![asked.clone()],
            Self::Counted { one, other, .. } => FORMS
                .iter()
                .map(|form| Asked {
                    english: if *form == "one" { one } else { other }.clone(),
                    context: (*form).to_owned(),
                })
                .collect(),
            Self::Game(_) | Self::Html(_) => Vec::new(),
        }
    }

    /// It in `lang`, and whether it is markup already.
    fn in_lang(&self, lang: Lang) -> (String, bool) {
        let index = Lang::ALL
            .iter()
            .position(|each| *each == lang)
            .expect("a language of ALL");
        let filled = |text: &str, places: &[(String, String)]| {
            let values: Vec<(&str, &dyn std::fmt::Display)> = places
                .iter()
                .map(|(name, value)| (name.as_str(), value as _))
                .collect();
            words::filled(text, &values)
        };
        match self {
            Self::Word { asked, places } => {
                let text =
                    words::text(lang, &asked.context, &asked.english).unwrap_or(&asked.english);
                (filled(text, places), false)
            }
            Self::Counted { one, other, places } => {
                let n = places.iter().find(|(name, _)| name == "n").map(|(_, n)| n);
                let form = counted(lang, n.and_then(|n| n.parse().ok()).expect("a count"));
                let english = if form == "one" { one } else { other };
                (
                    filled(words::text(lang, form, english).unwrap_or(english), places),
                    false,
                )
            }
            Self::Game(texts) => (texts[index].clone(), false),
            Self::Html(texts) => (texts[index].clone(), true),
        }
    }
}

/// The form of a count `n` in `lang`: `one`, `few` or `many` (a language with two forms has no
/// `few`: its row is its `many`).
fn counted(lang: Lang, n: u64) -> &'static str {
    let (ones, tens) = (n % 10, n % 100);
    let few = (2..=4).contains(&ones) && !(12..=14).contains(&tens);
    match lang {
        Lang::Uk | Lang::Ru | Lang::Be | Lang::Sr | Lang::Hr => {
            if ones == 1 && tens != 11 {
                "one"
            } else if few {
                "few"
            } else {
                "many"
            }
        }
        Lang::Pl if n == 1 => "one",
        Lang::Pl if few => "few",
        Lang::Cs if (2..=4).contains(&n) => "few",
        _ if n == 1 => "one",
        _ => "many",
    }
}

/// The texts of a table, or what is wrong with it.
fn table(json: &str) -> Result<Vec<Text>, String> {
    let rows: Vec<Value> = serde_json::from_str(json).map_err(|why| why.to_string())?;
    rows.iter().map(Text::of).collect()
}

/// What a table asks of the site's words (the words test's).
#[cfg(test)]
pub(crate) fn asked_by_table(json: &str) -> Result<Vec<Asked>, String> {
    Ok(table(json)?.iter().flat_map(Text::asked).collect())
}

/// The value of the JavaScript string literal that starts `source` (its quote first), and what
/// follows it; `None` if it is none (a template with `${…}` is none).
fn literal(source: &str) -> Option<(String, &str)> {
    let mut chars = source.char_indices();
    let (_, quote) = chars
        .next()
        .filter(|(_, c)| matches!(c, '"' | '\'' | '`'))?;
    let mut out = String::new();
    while let Some((at, c)) = chars.next() {
        match c {
            c if c == quote => return Some((out, &source[at + 1..])),
            '$' if quote == '`' && source[at..].starts_with("${") => return None,
            '\\' => match chars.next()?.1 {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'u' => {
                    let hex: String = (0..4)
                        .filter_map(|_| chars.next().map(|(_, c)| c))
                        .collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

/// The texts a script asks for: each `T("…")`, `Tf("…", …)` and `Tc("context", "…")`
/// (`site-texts/words.js`), or the place of one whose text is no literal.
pub(crate) fn asked_by_script(source: &str) -> Result<Vec<Asked>, String> {
    let mut asked = Vec::new();
    for (at, _) in source.match_indices('T') {
        let before = source[..at].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '.')) {
            continue;
        }
        let rest = &source[at + 1..];
        let (call, rest) = match () {
            () if rest.starts_with("c(") => ("Tc", &rest[2..]),
            () if rest.starts_with("f(") => ("Tf", &rest[2..]),
            () if rest.starts_with('(') => ("T", &rest[1..]),
            () => continue,
        };
        let wrong = || {
            let line = source[..at].matches('\n').count() + 1;
            format!("line {line}: {call}( of no literal text")
        };
        let (first, rest) = literal(rest.trim_start()).ok_or_else(wrong)?;
        if call == "Tc" {
            let rest = rest.trim_start().strip_prefix(',').ok_or_else(wrong)?;
            let (english, _) = literal(rest.trim_start()).ok_or_else(wrong)?;
            asked.push(Asked {
                english,
                context: first,
            });
        } else {
            asked.push(Asked {
                english: first,
                context: String::new(),
            });
        }
    }
    asked.sort();
    asked.dedup();
    Ok(asked)
}

/// `text` as HTML says it (as Python's `html.escape`).
fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            c => out.push(c),
        }
    }
    out
}

/// `text` inside a JSON string (as Python's `json.dumps`).
fn json_string(text: &str) -> String {
    let quoted = Value::String(text.to_owned()).to_string();
    quoted[1..quoted.len() - 1].to_owned()
}

impl Script {
    pub(crate) const fn new(source: &'static [u8]) -> Self {
        Self {
            source,
            asked: OnceLock::new(),
        }
    }

    pub(crate) fn source(&self) -> &'static [u8] {
        self.source
    }

    /// The words of the script in `lang`, by context, as JSON for its page's
    /// `<script type="application/json" id="page-words">`.
    pub(crate) fn words(&self, lang: Lang) -> String {
        let asked = self.asked.get_or_init(|| {
            asked_by_script(&String::from_utf8_lossy(self.source))
                .unwrap_or_else(|why| panic!("a built page's script: {why}"))
        });
        let mut by_context: BTreeMap<&str, Map<String, Value>> = BTreeMap::new();
        for asked in asked {
            let text = words::text(lang, &asked.context, &asked.english).unwrap_or(&asked.english);
            by_context
                .entry(&asked.context)
                .or_default()
                .insert(asked.english.clone(), Value::String(text.to_owned()));
        }
        let object: Map<String, Value> = by_context
            .into_iter()
            .map(|(context, words)| (context.to_owned(), Value::Object(words)))
            .collect();
        // Data, not markup: no `<` of a text ends the element.
        Value::Object(object).to_string().replace('<', "\\u003c")
    }
}

impl Built {
    pub(crate) const fn new(
        markup: &'static str,
        table: &'static str,
        script: &'static Script,
    ) -> Self {
        Self {
            markup,
            table,
            script,
            texts: OnceLock::new(),
        }
    }

    /// The page in `lang`.
    pub(crate) fn page(&self, lang: Lang) -> String {
        const OPEN: char = '⟦';
        const CLOSE: char = '⟧';
        let texts = self.texts.get_or_init(|| {
            table(self.table).unwrap_or_else(|why| panic!("a built page's texts: {why}"))
        });
        let mut out = String::with_capacity(self.markup.len() + self.markup.len() / 8);
        let mut rest = self.markup;
        while let Some(at) = rest.find(OPEN) {
            out.push_str(&rest[..at]);
            let after = &rest[at + OPEN.len_utf8()..];
            let end = after.find(CLOSE).expect("a built page's place is closed");
            let place = &after[..end];
            if place == "words" {
                out.push_str(&self.script.words(lang));
            } else if place == "lang" {
                out.push_str(lang.code());
            } else {
                let (number, json) = match place.strip_suffix('j') {
                    Some(number) => (number, true),
                    None => (place, false),
                };
                let text = number
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| texts.get(n))
                    .unwrap_or_else(|| panic!("a built page's place ⟦{place}⟧ has no text"));
                match (text.in_lang(lang), json) {
                    ((markup, true), false) => out.push_str(&markup),
                    ((text, false), false) => out.push_str(&escaped(&text)),
                    ((text, false), true) => out.push_str(&escaped(&json_string(&text))),
                    ((_, true), true) => panic!("a built page's markup ⟦{place}⟧ in JSON"),
                }
            }
            rest = &after[end + CLOSE.len_utf8()..];
        }
        out.push_str(rest);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = r#"[
        {"word": "Home"},
        {"word": "Ban", "context": "button"},
        {"one": "{n} point", "other": "{n} points", "places": {"n": "21"}},
        {"game": {"en": "Vostok \"Radar\" <1>", "uk": "РЛС \"Восток\"", "ru": "ru", "de": "de", "pl": "pl", "sr": "sr", "cs": "cs", "hu": "hu", "fi": "fi", "be": "be", "hr": "hr", "pt": "pt"}},
        {"html": {"en": "<b>A</b>", "uk": "<b>Б</b>", "ru": "", "de": "", "pl": "", "sr": "", "cs": "", "hu": "", "fi": "", "be": "", "hr": "", "pt": ""}}
    ]"#;

    #[test]
    fn a_page_is_filled_in_its_language_escaped_where_it_is() {
        static SCRIPT: Script =
            Script::new(b"const a = T(\"Home\"), b = Tc('button', 'Ban'); T(`Ho\\u006de`);");
        let page = Built::new(
            "<p title=\"⟦3⟧\">⟦0⟧ ⟦1⟧ ⟦2⟧</p><i data-x=\"{&quot;a&quot;:&quot;⟦3j⟧&quot;}\">⟦4⟧</i>\
             <script type=\"application/json\" id=\"page-words\">⟦words⟧</script>",
            TABLE,
            &SCRIPT,
        );
        assert_eq!(
            page.page(Lang::En),
            "<p title=\"Vostok &quot;Radar&quot; &lt;1&gt;\">Home Ban 21 points</p>\
             <i data-x=\"{&quot;a&quot;:&quot;Vostok \\&quot;Radar\\&quot; &lt;1&gt;&quot;}\"><b>A</b></i>\
             <script type=\"application/json\" id=\"page-words\">{\"\":{\"Home\":\"Home\"},\"button\":{\"Ban\":\"Ban\"}}</script>"
        );
        let uk = page.page(Lang::Uk);
        assert!(
            uk.starts_with("<p title=\"РЛС &quot;Восток&quot;\">Головна "),
            "{uk}"
        );
        assert!(
            uk.contains("<b>Б</b>") && uk.contains("\"Home\":\"Головна\""),
            "{uk}"
        );
    }

    #[test]
    fn a_count_takes_its_languages_form() {
        let forms = |lang| [1, 2, 5, 11, 21, 22, 25, 112, 102].map(|n| counted(lang, n));
        let (one, few, many) = ("one", "few", "many");
        assert_eq!(
            forms(Lang::En),
            [one, many, many, many, many, many, many, many, many]
        );
        assert_eq!(
            forms(Lang::Uk),
            [one, few, many, many, one, few, many, many, few]
        );
        assert_eq!(
            forms(Lang::Pl),
            [one, few, many, many, many, few, many, many, few]
        );
        assert_eq!(
            forms(Lang::Cs),
            [one, few, many, many, many, many, many, many, many]
        );
        assert_eq!(
            asked_by_table(TABLE).unwrap()[2..],
            [
                ("{n} point", "one"),
                ("{n} points", "few"),
                ("{n} points", "many")
            ]
            .map(|(english, context)| Asked {
                english: english.to_owned(),
                context: context.to_owned()
            })
        );
    }

    #[test]
    fn a_script_asks_for_literal_texts_only() {
        let asked =
            asked_by_script("T('a') + x.T(y) + Tf(\"b {n}\", {n}) + Tc(\"c\", 'd') + aT(e)")
                .unwrap();
        let pairs: Vec<_> = asked
            .iter()
            .map(|a| (a.english.as_str(), a.context.as_str()))
            .collect();
        assert_eq!(pairs, [("a", ""), ("b {n}", ""), ("d", "c")]);
        assert!(asked_by_script("T(name)").is_err());
        assert!(asked_by_script("T(`${n} shots`)").is_err());
        assert!(table(r#"[{"game": {"en": "a"}}]"#).is_err());
    }
}
