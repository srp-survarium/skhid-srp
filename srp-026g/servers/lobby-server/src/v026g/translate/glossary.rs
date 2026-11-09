//! The chat glossary (`servers/lobby-server/chat-glossary.tsv`): how the
//! game's names and the players' slang are said in each of the five
//! languages, so that a translation says them the way that language's
//! players do.
//!
//! - **The file.** One row a term: `kind`, then the five languages (`en`,
//!   `ru`, `uk`, `de`, `pl`), then a note (the game's text key for its own
//!   names). A cell may list the ways players write it, `|` between them;
//!   the first is the one a translation uses. A way ending in `*` matches
//!   any word it begins (`мосин*`: "мосинка", "мосинку"). An empty cell:
//!   the language has no word of its own (the term is given as written).
//!   Lines starting with `#` are comments.
//! - **Seeded** with the game's own names (`srp-chat-glossary seed`); slang
//!   is added from what players really write (`srp-chat-glossary slang`
//!   over a chat sample), by hand.
//! - **Used** by the backend: the rows a line contains, with their words in
//!   the languages asked for, go with the line (`claude`). A machine
//!   translation model would swap them for placeholders before and back
//!   after instead (the spec); the cloud translators have glossaries of
//!   their own to load it into.

use std::sync::OnceLock;

use super::Lang;

/// The glossary built into srp.
const BUILT_IN: &str = include_str!("../../../chat-glossary.tsv");
/// The most rows sent with one line.
pub const MOST: usize = 12;

#[derive(Clone, Debug, PartialEq)]
struct Row {
    /// Per language (in [`Lang::ALL`]'s order): the ways it is written.
    cells: [Vec<String>; 5],
}

/// A term of the glossary found in a line.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// As the line has it.
    pub term: String,
    /// Its word in each language asked for that has one.
    pub into: Vec<(Lang, String)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Glossary {
    rows: Vec<Row>,
}

fn index(lang: Lang) -> usize {
    Lang::ALL
        .iter()
        .position(|other| *other == lang)
        .expect("one of the five")
}

/// Where `way` is in `words` (the line's words, lowercase): the words it
/// covers, as the line has them.
fn find(way: &str, words: &[(String, &str)]) -> Option<String> {
    let (stem, prefix) = match way.strip_suffix('*') {
        Some(stem) => (stem, true),
        None => (way, false),
    };
    let wanted: Vec<String> = stem
        .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '-'))
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    if wanted.is_empty() || wanted.len() > words.len() {
        return None;
    }
    let last = wanted.len() - 1;
    (0..=words.len() - wanted.len()).find_map(|start| {
        let fits = wanted.iter().enumerate().all(|(at, want)| {
            let word = &words[start + at].0;
            if prefix && at == last {
                word.starts_with(want.as_str())
            } else {
                word == want
            }
        });
        fits.then(|| {
            words[start..=start + last]
                .iter()
                .map(|(_, original)| *original)
                .collect::<Vec<_>>()
                .join(" ")
        })
    })
}

impl Glossary {
    pub const HEADER: &'static str = "# kind\ten\tru\tuk\tde\tpl\tnote";

    /// The glossary built into srp (its file, checked by the tests).
    pub fn shared() -> &'static Glossary {
        static SHARED: OnceLock<Glossary> = OnceLock::new();
        SHARED.get_or_init(|| {
            Glossary::parse(BUILT_IN).unwrap_or_else(|error| {
                log::error!("chat-glossary.tsv: {error}; translating without it");
                Glossary::default()
            })
        })
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut rows = Vec::new();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            if !(6..=7).contains(&fields.len()) {
                return Err(format!(
                    "line {}: {} fields; a row is kind, en, ru, uk, de, pl and a note",
                    number + 1,
                    fields.len()
                ));
            }
            let cells: [Vec<String>; 5] = std::array::from_fn(|at| {
                fields[1 + at]
                    .split('|')
                    .map(str::trim)
                    .filter(|way| !way.is_empty())
                    .map(str::to_owned)
                    .collect()
            });
            if cells.iter().all(Vec::is_empty) {
                return Err(format!("line {}: no term in any language", number + 1));
            }
            rows.push(Row { cells });
        }
        Ok(Self { rows })
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Whether `word` (lowercase) is one of the glossary's ways, or begins
    /// one written with `*`.
    pub fn knows(&self, word: &str) -> bool {
        self.rows
            .iter()
            .flat_map(|row| row.cells.iter().flatten())
            .any(|way| {
                let way = way.to_lowercase();
                match way.strip_suffix('*') {
                    Some(stem) => word.starts_with(stem),
                    None => way.split_whitespace().any(|part| part == word),
                }
            })
    }

    /// The terms of `text` (in `from`, or any language if not known) whose
    /// word differs in a language of `to`, at most [`MOST`], longest first.
    pub fn entries(&self, text: &str, from: Option<Lang>, to: &[Lang]) -> Vec<Entry> {
        let words: Vec<(String, &str)> = text
            .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '-'))
            .filter(|word| !word.is_empty())
            .map(|word| (word.to_lowercase(), word))
            .collect();
        let sources: Vec<Lang> = match from {
            Some(from) => vec![from],
            None => Lang::ALL.to_vec(),
        };
        let mut found: Vec<Entry> = Vec::new();
        for row in &self.rows {
            let term = sources.iter().find_map(|source| {
                row.cells[index(*source)]
                    .iter()
                    .find_map(|way| find(way, &words))
            });
            let Some(term) = term else { continue };
            let into: Vec<(Lang, String)> = to
                .iter()
                .filter_map(|lang| {
                    let word = row.cells[index(*lang)].first()?;
                    let word = word.trim_end_matches('*');
                    (!word.eq_ignore_ascii_case(&term)).then(|| (*lang, word.to_owned()))
                })
                .collect();
            if !into.is_empty() && !found.iter().any(|entry| entry.term == term) {
                found.push(Entry { term, into });
            }
        }
        found.sort_by_key(|entry| std::cmp::Reverse(entry.term.chars().count()));
        found.truncate(MOST);
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# kind\ten\tru\tuk\tde\tpl\tnote
item\tMosin carbine | mosin*\tКарабин Мосина | мосин*\tКарабін Мосіна | мосін*\tMosin-Karabiner | mosin*\tKarabinek Mosina | mosin*\tst_mosin_name
map\tChemical Plant\tХимзавод\tХімзавод\tChemiewerk\tZakłady chemiczne\tst_level_04_name
slang\tez\tизи\tізі\t\t\t
";

    #[test]
    fn the_built_in_glossary_reads() {
        let glossary = Glossary::parse(BUILT_IN).unwrap();
        assert!(!glossary.is_empty());
    }

    #[test]
    fn a_term_is_found_in_its_forms_and_given_in_each_language() {
        let glossary = Glossary::parse(SAMPLE).unwrap();
        let entries = glossary.entries(
            "у кого мосинку купить? я на Химзавод",
            Some(Lang::Ru),
            &[Lang::En, Lang::De],
        );
        assert_eq!(
            entries,
            vec![
                Entry {
                    term: "Химзавод".into(),
                    into: vec![
                        (Lang::En, "Chemical Plant".into()),
                        (Lang::De, "Chemiewerk".into())
                    ],
                },
                Entry {
                    term: "мосинку".into(),
                    into: vec![
                        (Lang::En, "Mosin carbine".into()),
                        (Lang::De, "Mosin-Karabiner".into())
                    ],
                },
            ]
        );
        // Several words, any case; a language with no word of its own is
        // left out.
        let entries = glossary.entries("meet at chemical plant, izi", None, &[Lang::Pl]);
        assert_eq!(entries[0].term, "chemical plant");
        assert_eq!(
            entries[0].into,
            vec![(Lang::Pl, "Zakłady chemiczne".into())]
        );
        assert!(
            glossary
                .entries("изи", Some(Lang::Ru), &[Lang::De])
                .is_empty()
        );
        assert_eq!(
            glossary.entries("изи", Some(Lang::Ru), &[Lang::En])[0].into,
            vec![(Lang::En, "ez".into())]
        );
    }

    #[test]
    fn a_bad_row_is_refused() {
        assert!(Glossary::parse("item\tone\ttwo").is_err());
        assert!(Glossary::parse("item\t\t\t\t\t\t").is_err());
    }

    #[test]
    fn the_slang_tool_knows_a_term() {
        let glossary = Glossary::parse(SAMPLE).unwrap();
        assert!(glossary.knows("мосинка"));
        assert!(glossary.knows("химзавод"));
        assert!(!glossary.knows("кемпер"));
    }
}
