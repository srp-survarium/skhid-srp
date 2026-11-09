//! What never leaves srp (`docs/notes/chat-translation.md`, "What never
//! leaves srp"): a line with sexual content involving children is not sent
//! to the translation backend, nor kept in the chat sample. The one case
//! that must not depend on a provider; every other line is translated as it
//! is typed, swearing and trash talk included (the owner's).
//!
//! - **The list** is the server's, never a repository's: the file of
//!   `SRP_CHAT_BLOCKED` (on the data volume). Without it, translation and the
//!   chat sample stay off. Its rows: `lang` (en ru uk de pl, or `*` for
//!   any), a tab, the entry; `#` begins a comment. Explicit phrases only:
//!   words that are mere insults in game chat are not on it.
//! - **Matching:** a word is what lies between spaces; inside it, what is no
//!   letter, digit or leetspeak character is dropped; single letters spaced
//!   out are one word. A word is read with every look-alike made Cyrillic
//!   and made Latin (a/а, e/е, o/о, p/р, c/с, x/х, y/у, k/к, m/м, t/т, 0 o,
//!   3 з or e, 6 б, 4 ч or a, @ a, $ s, 1 ! i), lowercase, ё as е, and once
//!   more with a letter typed three times or more taken once. An entry
//!   matches words in a row from a word's start that begin with it, the
//!   spaces between them left out; never from inside a word.

use std::collections::HashSet;

use super::Lang;

/// The setting that names the list's file.
pub const VAR: &str = "SRP_CHAT_BLOCKED";

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

/// The readings of typed text: Cyrillic and Latin, and the same with long
/// runs squeezed.
fn readings(text: &str) -> Vec<String> {
    let lower: String = text
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

/// An entry as the list keeps it: in its own script's reading, spaces out.
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

/// The words of `text`, as typed: single letters spaced out ("c p") joined
/// into one.
fn words_of(text: &str) -> Vec<&str> {
    let mut words: Vec<(usize, usize)> = Vec::new();
    for token in text.split_whitespace() {
        let offset = token.as_ptr() as usize - text.as_ptr() as usize;
        // A word starts and ends on a letter, a digit, @ or $ (a `!` there
        // is punctuation).
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockList {
    /// The entries, spaces taken out.
    entries: HashSet<String>,
    /// The longest entry's length, in characters.
    longest: usize,
}

impl BlockList {
    /// The file of `SRP_CHAT_BLOCKED`; `Err` (why) when it is not set, not
    /// read, or empty: what needs it stays off.
    pub fn from_env() -> Result<Self, String> {
        let path = std::env::var(VAR)
            .ok()
            .map(|path| path.trim().to_owned())
            .filter(|path| !path.is_empty())
            .ok_or_else(|| {
                format!("{VAR} is not set (the server's list of what never leaves srp)")
            })?;
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{VAR} is `{path}`, not read: {error}"))?;
        let list = Self::parse(&text).map_err(|error| format!("{path}: {error}"))?;
        if list.is_empty() {
            return Err(format!("{path} has no entry"));
        }
        Ok(list)
    }

    /// Rows of `lang<TAB>entry`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut list = Self::default();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((lang, entry)) = line.split_once('\t') else {
                return Err(format!("line {}: a row is lang, entry", number + 1));
            };
            if lang != "*" && Lang::from_code(lang).is_none() {
                return Err(format!("line {}: no language `{lang}`", number + 1));
            }
            let reading = entry_reading(entry);
            if reading.is_empty() {
                return Err(format!("line {}: an empty entry", number + 1));
            }
            list.longest = list.longest.max(reading.chars().count());
            list.entries.insert(reading);
        }
        Ok(list)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether `text` holds an entry: words in a row, from a word's start,
    /// that begin with it.
    pub fn blocks(&self, text: &str) -> bool {
        let words = words_of(text);
        (0..words.len()).any(|start| {
            let mut joined = String::new();
            for word in &words[start..] {
                joined.push_str(word);
                let readings = readings(&joined);
                if readings
                    .iter()
                    .any(|reading| starts_with_any(reading, &self.entries))
                {
                    return true;
                }
                if readings
                    .iter()
                    .all(|reading| reading.chars().count() >= self.longest)
                {
                    return false;
                }
            }
            false
        })
    }
}

/// A list for the tests, of made-up phrases: the real one is never in the
/// repository.
#[cfg(test)]
pub const TEST_LIST: &str =
    "# made up\n*\tforbidden phrase\nru\tзапретная фраза\nde\tverbotenwort\n";

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> BlockList {
        BlockList::parse(TEST_LIST).unwrap()
    }

    #[test]
    fn a_list_reads_and_a_bad_row_is_refused() {
        assert_eq!(list().len(), 3);
        assert!(BlockList::parse("forbidden phrase\n").is_err());
        assert!(BlockList::parse("xx\tforbidden phrase\n").is_err());
    }

    #[test]
    fn its_phrases_are_found_with_their_spaces_disguises_or_without() {
        let list = list();
        for text in [
            "a forbidden phrase here",
            "FORBIDDEN PHRASE",
            "forbiddenphrase",
            "f0rbidden phr@se",
            "f o r b i d d e n phrase",
            "это запретнaя фраза",
            "verbotenworte",
            "verboten wort",
        ] {
            assert!(list.blocks(text), "{text}");
        }
    }

    #[test]
    fn everything_else_goes_and_never_from_inside_a_word() {
        let list = list();
        for text in [
            "бля, промазал",
            "сука блять нахуй",
            "kys noob",
            "forbidden",
            "a phrase",
            "unforbidden phrase",
            "no forbiddenness phrase",
        ] {
            assert!(!list.blocks(text), "{text}");
        }
    }
}
