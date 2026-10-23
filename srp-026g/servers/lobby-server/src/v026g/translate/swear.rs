//! What never reaches the translation backend (`docs/notes/chat-translation.md`,
//! "What never reaches the model"): swear words are masked and put back
//! after; a line that is mostly swearing, or holds a slur, a threat or
//! sexual violence, is not sent at all.
//!
//! - **The list** is srp's own, `servers/lobby-server/chat-swear.tsv`
//!   (built in; its header says the format and the sources): stems for the
//!   inflected Russian, Ukrainian and Polish mat and its transliteration,
//!   whole words (with LDNOOBW's, CC-BY 4.0), innocent words the stems
//!   would take, and what blocks a line.
//! - **The game's own lists** (the client's swear filter,
//!   `localization/<language>/swear_filter_data_a_c` of `resources.db`)
//!   are one more input when `SRP_CHAT_SWEAR_DIR` holds them (game data,
//!   copied there by hand, never into the repository): their swear entries
//!   of 4 letters or more as stems, their clean ones as allowed. They are
//!   not good enough alone (the owner's).
//! - **Matching:** a word is what lies between spaces; inside it, what is
//!   no letter, digit or leetspeak character is dropped ("х.у.й", "x-y-й");
//!   single letters spaced out ("х у й") are one word. A word is read twice:
//!   with every look-alike made Cyrillic and made Latin (a/а, e/е, o/о, p/р,
//!   c/с, x/х, y/у, k/к, m/м, t/т, 0 o, 3 з or e, 6 б, 4 ч or a, @ a, $ s,
//!   1 ! i), lowercase, ё as е; and once more with a letter typed three times
//!   or more taken once ("бляяяя"). It swears when one reading equals a
//!   `word` or begins with a `stem`, and none begins with an `allow`.

use std::collections::HashSet;
use std::path::Path;

use binary_config_parser::tree::ConfigValue;
use encoding_rs::WINDOWS_1251;

use super::Lang;

/// srp's list, built in.
const BUILT_IN: &str = include_str!("../../../chat-swear.tsv");
/// Over this share of a line's words swearing, the line is not sent.
const MOSTLY: f32 = 0.5;
/// The game's entries shorter than this are left out (they would take too
/// many innocent words at the start of a word).
const GAME_MIN_LETTERS: usize = 4;

/// A placeholder the model is told to keep: `__X1__`, `__X2__`...
pub fn placeholder(index: usize) -> String {
    format!("__X{index}__")
}

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

/// String values of a game config, as their bytes say (the reader takes
/// them for Windows-1251; the files are UTF-8, as the localizations).
fn utf8(read: &str) -> String {
    let (bytes, _, unmappable) = WINDOWS_1251.encode(read);
    match (unmappable, String::from_utf8(bytes.into_owned())) {
        (false, Ok(text)) => text,
        _ => read.to_owned(),
    }
}

/// What may go to the backend.
#[derive(Clone, Debug, PartialEq)]
pub enum Masked {
    /// This text, with the swear words as placeholders; the words, in the
    /// placeholders' order.
    Send { text: String, words: Vec<String> },
    /// Not sent: mostly swearing, or blocked.
    Skip,
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

/// The game's folder name of a language.
pub fn folder(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "english",
        Lang::Ru => "russian",
        Lang::Uk => "ukrainian",
        Lang::De => "german",
        Lang::Pl => "polish",
    }
}

fn starts_with_any(reading: &str, set: &HashSet<String>) -> bool {
    reading
        .char_indices()
        .map(|(at, c)| at + c.len_utf8())
        .any(|end| set.contains(&reading[..end]))
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

    /// Adds the game's own list, a `swear_filter_data_a_c` file.
    pub fn add_game_list(&mut self, bytes: &[u8]) -> Result<usize, String> {
        let root = ConfigValue::parse(bytes).map_err(|error| error.0)?;
        let entries = root
            .req("swear_filter_data")
            .and_then(|data| data.req("swear_dictionary"))
            .map_err(|error| error.0)?
            .children();
        let mut added = 0;
        for line in entries
            .into_iter()
            .filter_map(|value| value.as_str().map(utf8))
        {
            let Some((word, flag)) = line.trim().rsplit_once(' ') else {
                continue;
            };
            let reading = entry_reading(word.trim());
            if flag == "1" {
                self.allow.insert(reading);
            } else if reading.chars().count() >= GAME_MIN_LETTERS {
                added += usize::from(self.stems.insert(reading));
            }
        }
        Ok(added)
    }

    /// srp's list and, under `dir`, the game's five
    /// (`<language>/swear_filter_data_a_c`, as `srp-game-data get` writes
    /// `localization/`). A list that is there must read.
    pub fn with_game_lists(dir: &Path) -> Result<Self, String> {
        let mut filter = Self::built_in();
        for lang in Lang::ALL {
            let path = dir.join(folder(lang)).join("swear_filter_data_a_c");
            let bytes =
                std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            filter
                .add_game_list(&bytes)
                .map_err(|error| format!("{}: {error}", path.display()))?;
        }
        Ok(filter)
    }

    pub fn len(&self) -> usize {
        self.words.len() + self.stems.len() + self.block_stems.len() + self.block_phrases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether `word` is one the filter masks.
    pub fn swears(&self, word: &str) -> bool {
        let readings = readings(word);
        !readings
            .iter()
            .any(|reading| starts_with_any(reading, &self.allow))
            && readings.iter().any(|reading| {
                self.words.contains(reading) || starts_with_any(reading, &self.stems)
            })
    }

    fn blocks(&self, word: &str) -> bool {
        let readings = readings(word);
        !readings
            .iter()
            .any(|reading| starts_with_any(reading, &self.allow))
            && readings
                .iter()
                .any(|reading| starts_with_any(reading, &self.block_stems))
    }

    /// The words of `text`: the byte ranges of their typed characters,
    /// single letters spaced out joined into one.
    fn words(text: &str) -> Vec<(usize, usize)> {
        let mut words: Vec<(usize, usize)> = Vec::new();
        for token in text.split_whitespace() {
            let offset = token.as_ptr() as usize - text.as_ptr() as usize;
            // A word starts and ends on a letter, a digit, @ or $ (a `!` there is
            // punctuation).
            let edge = |c: char| c.is_alphanumeric() || matches!(c, '@' | '$');
            let first = token.char_indices().find(|(_, c)| edge(*c));
            let last = token.char_indices().rev().find(|(_, c)| edge(*c));
            if let (Some((from, _)), Some((to, c))) = (first, last) {
                words.push((offset + from, offset + to + c.len_utf8()));
            }
        }
        // "х у й": runs of single letters are one word.
        let mut joined: Vec<(usize, usize)> = Vec::new();
        let single = |(from, to): (usize, usize)| text[from..to].chars().count() == 1;
        for word in words {
            match joined.last_mut() {
                Some(last) if single(*last) && single(word) => last.1 = word.1,
                Some(last)
                    if single(word)
                        && text[last.0..last.1]
                            .chars()
                            .filter(|c| !c.is_whitespace())
                            .count()
                            > 1
                        && text[last.0..last.1].contains(' ') =>
                {
                    last.1 = word.1
                }
                _ => joined.push(word),
            }
        }
        joined
    }

    /// `text` ready for the backend, or [`Masked::Skip`].
    pub fn mask(&self, text: &str) -> Masked {
        let words = Self::words(text);
        let line: String = words
            .iter()
            .map(|&(from, to)| text[from..to].to_owned())
            .collect::<Vec<_>>()
            .join("");
        let whole = readings(&line);
        if words.iter().any(|&(from, to)| self.blocks(&text[from..to]))
            || self.block_phrases.iter().any(|phrase| {
                whole
                    .iter()
                    .any(|reading| reading.contains(phrase.as_str()))
            })
        {
            return Masked::Skip;
        }
        let swearing: Vec<(usize, usize)> = words
            .iter()
            .copied()
            .filter(|&(from, to)| self.swears(&text[from..to]))
            .collect();
        if !words.is_empty() && swearing.len() as f32 > words.len() as f32 * MOSTLY {
            return Masked::Skip;
        }
        let mut out = String::new();
        let mut kept = Vec::new();
        let mut last = 0;
        for (from, to) in swearing {
            out.push_str(&text[last..from]);
            kept.push(text[from..to].to_owned());
            out.push_str(&placeholder(kept.len()));
            last = to;
        }
        out.push_str(&text[last..]);
        Masked::Send {
            text: out,
            words: kept,
        }
    }
}

/// The words put back in a translation. A placeholder the backend dropped
/// is gone with its word; one it left twice gets the word twice.
pub fn unmask(translated: &str, words: &[String]) -> String {
    let mut out = translated.to_owned();
    // The highest first, so __X1__ never eats the start of __X12__.
    for (index, word) in words.iter().enumerate().rev() {
        out = out.replace(&placeholder(index + 1), word);
    }
    out
}

/// Whether an answer holds a placeholder that is not one of ours: then it
/// is no translation of the line.
pub fn has_stray_placeholder(translated: &str, words: &[String]) -> bool {
    let mut rest = translated;
    while let Some(at) = rest.find("__X") {
        let tail = &rest[at + 3..];
        let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
        let known = digits
            .parse::<usize>()
            .is_ok_and(|index| (1..=words.len()).contains(&index))
            && tail[digits.len()..].starts_with("__");
        if !known {
            return true;
        }
        rest = &tail[digits.len()..];
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn swears(text: &str) -> Vec<String> {
        match SwearFilter::built_in().mask(text) {
            Masked::Send { words, .. } => words,
            Masked::Skip => vec!["<skip>".into()],
        }
    }

    #[test]
    fn the_built_in_list_reads() {
        assert!(SwearFilter::built_in().len() > 300);
    }

    #[test]
    fn swearing_is_found_in_its_forms_spellings_and_disguises() {
        let cases = [
            ("ну ты и мудак, иди на базу", "мудак,"),
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
            assert_eq!(swears(text), [word.trim_end_matches(',')], "{text}");
        }
    }

    #[test]
    fn innocent_words_are_left_alone() {
        for text in [
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
            let found = swears(text);
            assert!(found.is_empty(), "{text}: {found:?}");
        }
    }

    #[test]
    fn a_line_mostly_swearing_or_blocked_is_not_sent() {
        let filter = SwearFilter::built_in();
        for text in [
            "сука блять",
            "fuck fuck ok",
            "just kys",
            "kill yourself noob",
            "убей себя",
            "ты пидор",
            "вбий себе",
            "zabij się",
        ] {
            assert_eq!(filter.mask(text), Masked::Skip, "{text}");
        }
        assert!(matches!(
            filter.mask("one fucking word in five"),
            Masked::Send { .. }
        ));
    }

    #[test]
    fn the_words_are_masked_and_go_back_after_the_translation() {
        let filter = SwearFilter::built_in();
        let Masked::Send { text, words } = filter.mask("иди на базу, мудак, там снайпер, сука")
        else {
            panic!("sent")
        };
        assert_eq!(text, "иди на базу, __X1__, там снайпер, __X2__");
        assert_eq!(words, ["мудак", "сука"]);
        let translated = "go to the base, __X1__, there's a sniper, __X2__";
        assert!(!has_stray_placeholder(translated, &words));
        assert_eq!(
            unmask(translated, &words),
            "go to the base, мудак, there's a sniper, сука"
        );
        assert!(has_stray_placeholder("go __X3__", &words));
        assert!(has_stray_placeholder("__Xoops", &words));
    }

    #[test]
    fn the_games_list_is_one_more_input() {
        let file = ConfigValue::Named(vec![(
            "swear_filter_data".to_owned(),
            ConfigValue::Named(vec![
                ("alphabet".to_owned(), ConfigValue::Indexed(vec![])),
                (
                    "swear_dictionary".to_owned(),
                    ConfigValue::Indexed(
                        ["badword 0", "bad 0", "badwordish 1"]
                            .iter()
                            .map(|entry| ConfigValue::String((*entry).to_owned()))
                            .collect(),
                    ),
                ),
                ("separators".to_owned(), ConfigValue::Indexed(vec![])),
            ]),
        )])
        .to_bytes()
        .unwrap();
        let mut filter = SwearFilter::built_in();
        // "bad" is too short to be a stem.
        assert_eq!(filter.add_game_list(&file), Ok(1));
        assert!(
            matches!(filter.mask("a badwords map here"), Masked::Send { words, .. } if words == ["badwords"])
        );
        assert!(
            matches!(filter.mask("a badwordish map, bad luck"), Masked::Send { words, .. } if words.is_empty())
        );
    }

    /// The game's lists: `SRP_CHAT_SWEAR_DIR=<folder> cargo test -p
    /// lobby-server -- --ignored the_games_lists_read`.
    #[test]
    #[ignore]
    fn the_games_lists_read() {
        let dir = std::env::var("SRP_CHAT_SWEAR_DIR").expect("SRP_CHAT_SWEAR_DIR");
        let built_in = SwearFilter::built_in().len();
        let filter = SwearFilter::with_game_lists(Path::new(&dir)).unwrap();
        eprintln!("built in {built_in}, with the game's {}", filter.len());
        let started = std::time::Instant::now();
        for _ in 0..100 {
            let _ =
                filter.mask("ну что за лаг опять, идите все на базу, там снайпер сидит на крыше");
        }
        eprintln!("a line: {:?}", started.elapsed() / 100);
        // What the game's lists add on ordinary chat: printed, for a person.
        for line in [
            "надо употреблять аптечки чаще, хороший психолог нужен",
            "мудрый ход, сучок на дереве, лайнер, мандарин",
            "the Scunthorpe assassin passed the class in the cockpit",
            "mój sukces, dobra gra, ktoś ma apteczkę",
            "gutes Spiel Leute, Scharfschütze auf dem Dach",
            "хлопці, тримаємо точку біля церкви",
        ] {
            eprintln!("{line}: {:?}", filter.mask(line));
        }
    }
}
