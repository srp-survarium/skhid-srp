//! The word list's say on whether a line may go to the translation backend
//! (`docs/notes/chat-translation.md`, "What never reaches the model"): a
//! free first check before the local classifier (`moderate`). It says
//! **no** for a line that holds a slur, a threat or sexual violence (a
//! `block` entry), or whose words are more than half obscenity; every other
//! line goes on **as it is**, swearing and all (the owner's: swearing is
//! translated, not masked).
//!
//! - **The list** is srp's own, `servers/lobby-server/chat-swear.tsv`
//!   (built in; its header says the format and the sources): stems for the
//!   inflected Russian, Ukrainian and Polish mat and its transliteration,
//!   whole words (with LDNOOBW's, CC-BY 4.0), innocent words the stems
//!   would take, and what blocks a line. The game's own swear list is not
//!   used: it matches inside words ("сабля" for "бля", the owner's).
//! - **Matching:** a word is what lies between spaces; inside it, what is
//!   no letter, digit or leetspeak character is dropped ("х.у.й", "x-y-й");
//!   single letters spaced out ("х у й") are one word. A word is read twice:
//!   with every look-alike made Cyrillic and made Latin (a/а, e/е, o/о, p/р,
//!   c/с, x/х, y/у, k/к, m/м, t/т, 0 o, 3 з or e, 6 б, 4 ч or a, @ a, $ s,
//!   1 ! i), lowercase, ё as е; and once more with a letter typed three times
//!   or more taken once ("бляяяя"). It is obscene when one reading equals a
//!   `word` or **begins** with a `stem`, and none begins with an `allow`; a
//!   stem inside a word never counts ("сабля", "корабля", "учеба").

use std::collections::HashSet;

use super::Lang;

/// srp's list, built in.
const BUILT_IN: &str = include_str!("../../../chat-swear.tsv");
/// Over this share of a line's words obscene, the line is not sent.
const MOSTLY: f32 = 0.5;

/// One reading of a character: as Cyrillic and as Latin.
fn as_cyrillic(c: char) -> Option<char> {
    Some(match c {
        'a' | '@' => 'а',
        'e' => 'е',
        'ё' => 'е',
        'o' | '0' => 'о',
        'p' => 'р',
        'c' => 'с',
        'x' => 'х',
        'y' => 'у',
        'k' => 'к',
        'm' => 'м',
        't' => 'т',
        'n' => 'п',
        '3' => 'з',
        '6' => 'б',
        '4' => 'ч',
        '1' | '!' => 'и',
        c if c.is_alphabetic() => c,
        _ => return None,
    })
}

fn as_latin(c: char) -> Option<char> {
    Some(match c {
        'а' | '@' | '4' => 'a',
        'е' | 'ё' | '3' => 'e',
        'о' | '0' => 'o',
        'р' => 'p',
        'с' => 'c',
        'х' => 'x',
        'у' => 'y',
        'к' => 'k',
        'м' => 'm',
        'т' => 't',
        '$' | '5' => 's',
        '1' | '!' => 'i',
        '7' => 't',
        c if c.is_alphabetic() => c,
        _ => return None,
    })
}

/// Whether `c` is part of a word as typed (a letter, a digit, leetspeak).
fn in_word(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '@' | '$' | '!')
}

/// A letter typed three times or more, once.
fn squeeze_runs(word: &str) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut out = String::new();
    let mut squeezed = false;
    let mut at = 0;
    while at < chars.len() {
        let mut end = at + 1;
        while end < chars.len() && chars[end] == chars[at] {
            end += 1;
        }
        squeezed |= end - at >= 3;
        let keep = if end - at >= 3 { 1 } else { end - at };
        out.extend(std::iter::repeat_n(chars[at], keep));
        at = end;
    }
    squeezed.then_some(out)
}

/// The readings of a typed word: Cyrillic and Latin, and the same with
/// long runs squeezed.
fn readings(word: &str) -> Vec<String> {
    let lower: String = word
        .to_lowercase()
        .chars()
        .filter(|c| in_word(*c))
        .collect();
    let mut out: Vec<String> = Vec::new();
    for reading in [
        lower.chars().filter_map(as_cyrillic).collect::<String>(),
        lower.chars().filter_map(as_latin).collect::<String>(),
    ] {
        if let Some(squeezed) = squeeze_runs(&reading) {
            out.push(squeezed);
        }
        out.push(reading);
    }
    out.retain(|reading| !reading.is_empty());
    out.dedup();
    out
}

/// An entry as the list keeps it: in its own script's reading.
fn entry_reading(entry: &str) -> String {
    let lower = entry.to_lowercase();
    if lower.chars().any(|c| matches!(c, '\u{0400}'..='\u{04FF}')) {
        lower.chars().filter_map(as_cyrillic).collect()
    } else {
        lower.chars().filter_map(as_latin).collect()
    }
}

fn starts_with_any(reading: &str, set: &HashSet<String>) -> bool {
    reading
        .char_indices()
        .map(|(at, c)| at + c.len_utf8())
        .any(|end| set.contains(&reading[..end]))
}

/// The list's verdict on a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing against it: the classifier decides (or, with none, it goes).
    Pass,
    /// A slur, a threat or sexual violence.
    Blocked,
    /// More than half its words obscene.
    Obscene,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SwearFilter {
    words: HashSet<String>,
    stems: HashSet<String>,
    allow: HashSet<String>,
    block_stems: HashSet<String>,
    /// Phrases, spaces taken out.
    block_phrases: Vec<String>,
}

impl SwearFilter {
    /// srp's list (its file, checked by the tests).
    pub fn built_in() -> Self {
        Self::parse(BUILT_IN).expect("chat-swear.tsv reads (a test checks it)")
    }

    /// Rows of `lang<TAB>kind<TAB>entry`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut filter = Self::default();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            let [lang, kind, entry] = fields[..] else {
                return Err(format!("line {}: a row is lang, kind, entry", number + 1));
            };
            if lang != "*" && Lang::from_code(lang).is_none() {
                return Err(format!("line {}: no language `{lang}`", number + 1));
            }
            let reading = entry_reading(entry);
            if reading.is_empty() {
                return Err(format!("line {}: an empty entry", number + 1));
            }
            match kind {
                "word" => filter.words.insert(reading),
                "stem" => filter.stems.insert(reading),
                "allow" => filter.allow.insert(reading),
                "block" if entry.contains(' ') => {
                    filter.block_phrases.push(reading);
                    true
                }
                "block" => filter.block_stems.insert(reading),
                _ => return Err(format!("line {}: no kind `{kind}`", number + 1)),
            };
        }
        Ok(filter)
    }

    pub fn len(&self) -> usize {
        self.words.len() + self.stems.len() + self.block_stems.len() + self.block_phrases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn allowed(readings: &[String], allow: &HashSet<String>) -> bool {
        readings
            .iter()
            .any(|reading| starts_with_any(reading, allow))
    }

    /// Whether `word` is obscene by the list.
    pub fn swears(&self, word: &str) -> bool {
        let readings = readings(word);
        !Self::allowed(&readings, &self.allow)
            && readings.iter().any(|reading| {
                self.words.contains(reading) || starts_with_any(reading, &self.stems)
            })
    }

    fn blocks(&self, word: &str) -> bool {
        let readings = readings(word);
        !Self::allowed(&readings, &self.allow)
            && readings
                .iter()
                .any(|reading| starts_with_any(reading, &self.block_stems))
    }

    /// The words of `text`, as typed: single letters spaced out ("f u c k")
    /// joined into one.
    pub fn words_of(text: &str) -> Vec<&str> {
        let mut words: Vec<(usize, usize)> = Vec::new();
        for token in text.split_whitespace() {
            let offset = token.as_ptr() as usize - text.as_ptr() as usize;
            // A word starts and ends on a letter, a digit, @ or $ (a `!`
            // there is punctuation).
            let edge = |c: char| c.is_alphanumeric() || matches!(c, '@' | '$');
            let first = token.char_indices().find(|(_, c)| edge(*c));
            let last = token.char_indices().rev().find(|(_, c)| edge(*c));
            if let (Some((from, _)), Some((to, c))) = (first, last) {
                words.push((offset + from, offset + to + c.len_utf8()));
            }
        }
        let mut joined: Vec<(usize, usize)> = Vec::new();
        let single = |(from, to): (usize, usize)| text[from..to].chars().count() == 1;
        for word in words {
            match joined.last_mut() {
                Some(last) if single(*last) && single(word) => last.1 = word.1,
                Some(last) if single(word) && text[last.0..last.1].contains(' ') => last.1 = word.1,
                _ => joined.push(word),
            }
        }
        joined
            .into_iter()
            .map(|(from, to)| &text[from..to])
            .collect()
    }

    /// The obscene words of `text` (for the tests and the tools).
    pub fn obscene_words<'t>(&self, text: &'t str) -> Vec<&'t str> {
        Self::words_of(text)
            .into_iter()
            .filter(|word| self.swears(word))
            .collect()
    }

    /// The list's verdict on `text`.
    pub fn judge(&self, text: &str) -> Verdict {
        let words = Self::words_of(text);
        let whole = readings(&words.concat());
        if words.iter().any(|word| self.blocks(word))
            || self.block_phrases.iter().any(|phrase| {
                whole
                    .iter()
                    .any(|reading| reading.contains(phrase.as_str()))
            })
        {
            return Verdict::Blocked;
        }
        let obscene = words.iter().filter(|word| self.swears(word)).count();
        if !words.is_empty() && obscene as f32 > words.len() as f32 * MOSTLY {
            return Verdict::Obscene;
        }
        Verdict::Pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obscene(text: &str) -> Vec<String> {
        SwearFilter::built_in()
            .obscene_words(text)
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn the_built_in_list_reads() {
        assert!(SwearFilter::built_in().len() > 300);
    }

    #[test]
    fn obscenity_is_found_in_its_forms_spellings_and_disguises() {
        let cases = [
            ("ну ты и мудак, иди на базу", "мудак"),
            ("он опять заебал всех тут", "заебал"),
            ("охуеть какой лаг сегодня", "охуеть"),
            ("пиздец нам на этой карте", "пиздец"),
            ("ёбаный снайпер на крыше", "ёбаный"),
            ("x-y-й там сидит на точке", "x-y-й"),
            ("х.у.й там сидит на точке", "х.у.й"),
            ("бляяяя опять этот лаг", "бляяяя"),
            ("сука опять кемпер на точке", "сука"),
            ("cyka he is camping again", "cyka"),
            ("blyat he is camping again", "blyat"),
            ("pizdec we lost this one", "pizdec"),
            ("he went nahui from the point", "nahui"),
            ("kurwa znowu ten kamper", "kurwa"),
            ("ja pierdolę co za lag", "pierdolę"),
            ("zajebisty strzał stary dzięki", "zajebisty"),
            ("this f u c k i n g lag again", "f u c k i n g"),
            ("what a sh1t map this is", "sh1t"),
            ("so ein Scheiß Lag heute", "Scheiß"),
            ("п1зд@ец нам ребята", "п1зд@ец"),
        ];
        for (text, word) in cases {
            assert_eq!(obscene(text), [word], "{text}");
        }
    }

    #[test]
    fn a_bad_substring_inside_a_word_is_clean() {
        // The game's own filter masks these ("сабля" for "бля"); a stem
        // here only counts at a word's start, in every reading.
        for text in [
            "сабля",
            "корабля",
            "рубля",
            "грабли",
            "оглобля",
            "тебя",
            "небо",
            "учеба",
            "учёба",
            "хлеб",
            "гребля",
            "ребенок",
            "ребёнок",
            "c@бля",
            "kopaбля",
            "pу6ля",
            "надо употреблять аптечки чаще",
            "не надо оскорблять тиммейтов",
            "хороший психолог нужен этой команде",
            "бляха муха опять лаг",
            "mój sukces w tej rundzie",
            "Hancock and Dickens sit in the cockpit with a cocktail",
            "the Scunthorpe team assassin passed the class",
            "мандарин и мандат на руках",
            "хачапури после матча",
            "an escort mission on the map",
            "жидкость в контейнере",
            "go to the pedal station",
            "сучок на дереве у моста",
            "мудрый ход, хорошая игра",
            "лайнер на горизонте",
        ] {
            assert!(obscene(text).is_empty(), "{text}: {:?}", obscene(text));
            assert_eq!(SwearFilter::built_in().judge(text), Verdict::Pass, "{text}");
        }
    }

    #[test]
    fn swearing_and_trash_talk_pass_and_only_the_no_cases_do_not() {
        let filter = SwearFilter::built_in();
        // The owner's: these go to translation as they are.
        for text in [
            "бля, промазал",
            "ты бездарь, удали игру",
            "noob, uninstall the game",
            "ты рак, иди учись стрелять",
            "one fucking word in five",
            "ну ты и чёрт 😄",
        ] {
            assert_eq!(filter.judge(text), Verdict::Pass, "{text}");
        }
        for text in ["сука блять", "fuck fuck ok", "бля"] {
            assert_eq!(filter.judge(text), Verdict::Obscene, "{text}");
        }
        for text in [
            "just kys",
            "kill yourself noob",
            "убей себя",
            "ты пидор",
            "вбий себе",
            "zabij się",
        ] {
            assert_eq!(filter.judge(text), Verdict::Blocked, "{text}");
        }
    }
}
