//! The site's own words in the languages other than English
//! (`../words/README.md`): the code writes a text in English
//! ([`Lang::t`]), and each language's file says it in that language. Every
//! file has every text (the tests name what one lacks); a text no file has
//! is shown in English.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;

use crate::Lang;

/// Each language's file, in [`Lang::ALL`]'s order. English has none: its
/// words are the code's.
const FILES: [&str; 12] = [
    "",
    include_str!("../words/uk.tsv"),
    include_str!("../words/ru.tsv"),
    include_str!("../words/de.tsv"),
    include_str!("../words/pl.tsv"),
    include_str!("../words/sr.tsv"),
    include_str!("../words/cs.tsv"),
    include_str!("../words/hu.tsv"),
    include_str!("../words/fi.tsv"),
    include_str!("../words/be.tsv"),
    include_str!("../words/hr.tsv"),
    include_str!("../words/pt.tsv"),
];

/// A row of a file: an English text in a context ("" for none) and what it
/// is in the file's language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pub english: String,
    pub context: String,
    pub text: String,
}

/// A text of a file as written there: `\n`, `\t` and `\\` read.
fn unescaped(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        out.push(match chars.next() {
            Some('n') => '\n',
            Some('t') => '\t',
            Some('\\') => '\\',
            other => return Err(format!("\\{} is no escape", other.map_or(String::new(), String::from))),
        });
    }
    Ok(out)
}

/// The rows of a file, or what is wrong with its line.
pub(crate) fn rows(file: &str) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for (n, line) in file.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let wrong = |why: &str| format!("line {}: {why}", n + 1);
        let mut fields = line.split('\t');
        let (Some(english), Some(text)) = (fields.next(), fields.next()) else {
            return Err(wrong("no tab between the English text and its translation"));
        };
        let context = fields.next().unwrap_or("");
        if fields.next().is_some() {
            return Err(wrong("more than three fields"));
        }
        rows.push(Row {
            english: unescaped(english).map_err(|why| wrong(&why))?,
            context: context.to_owned(),
            text: unescaped(text).map_err(|why| wrong(&why))?,
        });
    }
    Ok(rows)
}

/// A language's words: by the English text, each context's.
type Words = HashMap<String, Vec<(String, String)>>;

fn words(lang: Lang) -> &'static Words {
    static WORDS: [OnceLock<Words>; 12] = [const { OnceLock::new() }; 12];
    let index = Lang::ALL.iter().position(|each| *each == lang).expect("a language of ALL");
    WORDS[index].get_or_init(|| {
        let mut words = Words::new();
        // The tests read every file: one that does not read stops them.
        for row in rows(FILES[index]).unwrap_or_else(|why| panic!("words/{}.tsv: {why}", lang.code())) {
            words.entry(row.english).or_default().push((row.context, row.text));
        }
        words
    })
}

/// Whether `lang` has words of its own: English (the code's), or a file with
/// rows.
pub(crate) fn has(lang: Lang) -> bool {
    lang == Lang::En || !words(lang).is_empty()
}

/// `english` (in `context`, "" for none) in `lang`, if its file has it.
pub(crate) fn text(lang: Lang, context: &str, english: &str) -> Option<&'static str> {
    words(lang)
        .get(english)?
        .iter()
        .find(|(of, _)| of == context)
        .map(|(_, text)| text.as_str())
}

/// The names of a text's `{places}`, in its order.
#[cfg(test)]
fn places(text: &str) -> Vec<&str> {
    let mut places = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('}') else { break };
        let name = &rest[..close];
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            places.push(name);
            rest = &rest[close + 1..];
        }
    }
    places
}

/// `text` with each `{name}` of `values` filled, in one pass: what a value
/// holds is never read as a place.
pub(crate) fn filled(text: &str, values: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let value = after.find('}').and_then(|close| {
            let name = &after[..close];
            values
                .iter()
                .find(|(of, _)| *of == name)
                .map(|(_, value)| (close, value))
        });
        match value {
            Some((close, value)) => {
                out.push_str(&value.to_string());
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::*;

    /// The value of the Rust string literal that starts `source` (after its
    /// opening quote), and what follows it.
    fn literal(source: &str) -> Option<(String, &str)> {
        let mut out = String::new();
        let mut chars = source.char_indices();
        while let Some((at, c)) = chars.next() {
            match c {
                '"' => return Some((out, &source[at + 1..])),
                '\\' => match chars.next()?.1 {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    '0' => out.push('\0'),
                    'u' => {
                        let hex: String = chars
                            .by_ref()
                            .map(|(_, c)| c)
                            .skip(1)
                            .take_while(|c| *c != '}')
                            .collect();
                        out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    }
                    // A line continued: the break and the next line's indent go.
                    '\n' => {
                        let rest = &source[at + 2..];
                        let skipped = rest.len() - rest.trim_start().len();
                        for _ in 0..rest[..skipped].chars().count() {
                            chars.next();
                        }
                    }
                    other => out.push(other),
                },
                c => out.push(c),
            }
        }
        None
    }

    /// The string literal a call's argument list starts with (after
    /// whitespace), and what follows it.
    fn argument(source: &str) -> Option<(String, &str)> {
        literal(source.trim_start().strip_prefix('"')?)
    }

    /// Every text the code asks for: `(english, context)` of each
    /// `.t("..")`, `.tf("..", ..)` and `.tc("context", "..")` under `src/`.
    fn asked() -> BTreeSet<(String, String)> {
        fn files(dir: &Path, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    files(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("words.rs") {
                    out.push(std::fs::read_to_string(&path).unwrap());
                }
            }
        }
        let mut sources = Vec::new();
        files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut sources);
        let mut asked = BTreeSet::new();
        for source in &sources {
            for call in [".t(", ".tf("] {
                for (at, _) in source.match_indices(call) {
                    if let Some((english, _)) = argument(&source[at + call.len()..]) {
                        asked.insert((english, String::new()));
                    }
                }
            }
            for (at, _) in source.match_indices(".tc(") {
                let Some((context, rest)) = argument(&source[at + 4..]) else { continue };
                // `tc(context, english)` of a text passed on is no text of its own.
                let Some(rest) = rest.trim_start().strip_prefix(',') else { continue };
                let Some((english, _)) = argument(rest) else { continue };
                asked.insert((english, context));
            }
        }
        asked
    }

    /// A text as a file writes it: `\`, line breaks and tabs escaped.
    fn escaped(text: &str) -> String {
        text.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', "\\t")
    }

    /// Every language has every text the code asks for, and only those. What
    /// is wrong is named file by file; a missing text as the row to add, its
    /// translation left empty to be written.
    #[test]
    fn every_language_has_every_text() {
        let asked = asked();
        assert!(asked.len() > 300, "{} texts found in the code", asked.len());
        assert!(asked.contains(&("Home".to_owned(), String::new())));
        assert!(asked.contains(&("Ban".to_owned(), "button".to_owned())));
        assert!(FILES[0].is_empty(), "English's words are the code's");
        let mut faults = String::new();
        for (lang, file) in Lang::ALL.into_iter().zip(FILES).skip(1) {
            let name = format!("words/{}.tsv", lang.code());
            let rows = rows(file).unwrap_or_else(|why| panic!("{name}: {why}"));
            let mut wrong = Vec::new();
            let mut seen = BTreeSet::new();
            for row in &rows {
                let key = (row.english.clone(), row.context.clone());
                if !asked.contains(&key) {
                    wrong.push(format!("a row the code no longer asks for: {key:?}"));
                }
                if row.text.trim().is_empty() {
                    wrong.push(format!("not translated: {key:?}"));
                }
                let (mut english, mut text) = (places(&row.english), places(&row.text));
                english.sort_unstable();
                text.sort_unstable();
                if english != text {
                    wrong.push(format!("other places than its English: {key:?}"));
                }
                if !seen.insert(key.clone()) {
                    wrong.push(format!("twice: {key:?}"));
                }
            }
            let missing: Vec<_> = asked.difference(&seen).collect();
            if !missing.is_empty() {
                wrong.push(format!(
                    "lacks {} texts: add these rows where uk.tsv has them and write each translation after its tab:",
                    missing.len()
                ));
                for (english, context) in missing {
                    let context = if context.is_empty() { String::new() } else { format!("\t{context}") };
                    wrong.push(format!("{}\t{context}", escaped(english)));
                }
            }
            if !wrong.is_empty() {
                faults += &format!("\n{name}:\n{}\n", wrong.join("\n"));
            }
        }
        assert!(faults.is_empty(), "the site's words (words/README.md):{faults}");
    }

    #[test]
    fn a_text_is_its_languages_or_english() {
        assert_eq!(Lang::Uk.t("Home"), "Головна");
        assert_eq!(Lang::En.t("Home"), "Home");
        // A text the files lack (none has this one).
        assert_eq!(Lang::Fi.t("no such text"), "no such text");
        assert_eq!(Lang::Uk.t("no such text"), "no such text");
        // Two meanings of one English text.
        assert_ne!(Lang::Uk.t("Ban"), Lang::Uk.tc("button", "Ban"));
        assert_eq!(Lang::En.tc("button", "Ban"), "Ban");
    }

    #[test]
    fn places_are_filled_in_one_pass() {
        let name: &dyn Display = &"{b}";
        assert_eq!(filled("{a} and {b}, {a}", &[("a", name), ("b", &7)]), "{b} and 7, {b}");
        assert_eq!(filled("{c} {", &[("a", &1)]), "{c} {");
        assert_eq!(places("{0}: up to {limit} of {0} {} {x y}"), ["0", "limit", "0"]);
        assert_eq!(unescaped("a\\nb\\\\c\\t"), Ok("a\nb\\c\t".to_owned()));
        assert!(unescaped("a\\x").is_err());
        assert!(rows("one field").is_err());
        assert_eq!(rows("# note\n\nA\tB\tbutton").unwrap()[0].context, "button");
    }
}
