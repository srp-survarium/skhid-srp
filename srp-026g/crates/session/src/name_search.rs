//! How well a typed text matches an account's name: the rules of the lobby movie's item search
//! (`InventoryList.wordScore`, `docs/notes/item-search.md`), for the search of players by name.
//!
//! The typed text and the name are compared as small Latin letters and digits; anything else
//! parts words, and so does a capital after a small letter (`SecondTester` is `second tester`).
//! Each typed word must match, best first:
//!
//! - at the name's start ([`START`]), at a word's start ([`WORD`]), inside a word ([`PART`]);
//! - from [`SPREAD_FROM`] letters on, its letters in order with others between, at most
//!   [`SPREAD_MOST`] times its length apart ([`SPREAD`] less the letters between, at least
//!   [`SPREAD_LEAST`]);
//! - from [`TYPO_FROM`] letters on, with one letter wrong, missing, extra or two swapped, and from
//!   [`TYPO_TWO_FROM`] on with two ([`TYPO`] less [`TYPO_EACH`] a typo).
//!
//! A text's score is its words' scores added; 0 when a word matches nothing.

const SPREAD_FROM: usize = 3;
const SPREAD_MOST: usize = 3;
const TYPO_FROM: usize = 4;
const TYPO_TWO_FROM: usize = 8;

const START: u32 = 400;
const WORD: u32 = 350;
const PART: u32 = 300;
const SPREAD: u32 = 250;
const SPREAD_LEAST: u32 = 200;
const TYPO: u32 = 150;
const TYPO_EACH: u32 = 25;

/// A text's letters (small Latin letters and digits) and where its words start in them.
fn form(text: &str) -> (Vec<u8>, Vec<usize>) {
    let (mut letters, mut starts) = (Vec::new(), Vec::new());
    let mut before: Option<u8> = None;
    for byte in text.bytes() {
        if !byte.is_ascii_alphanumeric() {
            before = None;
            continue;
        }
        let starts_word = match before {
            None => true,
            Some(before) => byte.is_ascii_uppercase() && before.is_ascii_lowercase(),
        };
        if starts_word {
            starts.push(letters.len());
        }
        letters.push(byte.to_ascii_lowercase());
        before = Some(byte);
    }
    (letters, starts)
}

fn find(letters: &[u8], word: &[u8], from: usize) -> Option<usize> {
    (from..=letters.len().checked_sub(word.len())?).find(|&at| &letters[at..at + word.len()] == word)
}

/// The shortest stretch of the letters that holds the word's letters in order.
fn spread_span(word: &[u8], letters: &[u8]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for from in (0..letters.len()).filter(|&at| letters[at] == word[0]) {
        let mut at = from;
        let mut whole = true;
        for &letter in &word[1..] {
            match letters[at + 1..].iter().position(|&l| l == letter) {
                Some(next) => at += 1 + next,
                None => {
                    whole = false;
                    break;
                }
            }
        }
        if !whole {
            // a later start finds the rest no sooner
            break;
        }
        best = Some(best.map_or(at - from + 1, |best| best.min(at - from + 1)));
    }
    best
}

/// The fewest typos (a letter wrong, missing, extra, or two swapped) that make the word a part of
/// the letters, when at most `most`.
fn typo_count(word: &[u8], letters: &[u8], most: usize) -> Option<usize> {
    let n = letters.len();
    // rows of the edit distance from the word's first i letters to a part of the letters ending
    // at j (a part may start anywhere: row 0 is all 0)
    let mut before = vec![0usize; n + 1];
    let mut last = vec![0usize; n + 1];
    let mut row = vec![0usize; n + 1];
    for i in 1..=word.len() {
        row[0] = i;
        let w = word[i - 1];
        for j in 1..=n {
            let t = letters[j - 1];
            let mut d = last[j - 1] + usize::from(w != t);
            d = d.min(last[j] + 1).min(row[j - 1] + 1);
            if i > 1 && j > 1 && w == letters[j - 2] && word[i - 2] == t {
                d = d.min(before[j - 2] + 1);
            }
            row[j] = d;
        }
        if row.iter().min().is_some_and(|&least| least > most) {
            return None;
        }
        std::mem::swap(&mut before, &mut last);
        std::mem::swap(&mut last, &mut row);
    }
    last.iter().copied().min().filter(|&typos| typos <= most)
}

/// One typed word against a name's letters; 0: no match.
fn word_score(word: &[u8], letters: &[u8], starts: &[usize]) -> u32 {
    if let Some(first) = find(letters, word, 0) {
        if first == 0 {
            return START;
        }
        let mut at = Some(first);
        while let Some(found) = at {
            if starts.contains(&found) {
                return WORD;
            }
            at = find(letters, word, found + 1);
        }
        return PART;
    }
    if word.len() >= SPREAD_FROM
        && let Some(span) = spread_span(word, letters)
        && span <= word.len() * SPREAD_MOST
    {
        return SPREAD_LEAST.max(SPREAD.saturating_sub((span - word.len()) as u32));
    }
    if word.len() >= TYPO_FROM
        && let Some(typos) = typo_count(word, letters, if word.len() >= TYPO_TWO_FROM { 2 } else { 1 })
    {
        return TYPO - typos as u32 * TYPO_EACH;
    }
    0
}

/// How well `typed` matches `name`; 0 when it does not (or names no letter).
pub fn score(typed: &str, name: &str) -> u32 {
    let (typed, typed_starts) = form(typed);
    let (letters, starts) = form(name);
    if typed.is_empty() || letters.is_empty() {
        return 0;
    }
    let mut total = 0;
    for (index, &start) in typed_starts.iter().enumerate() {
        let end = typed_starts.get(index + 1).copied().unwrap_or(typed.len());
        match word_score(&typed[start..end], &letters, &starts) {
            0 => return 0,
            word => total += word,
        }
    }
    // the whole name typed, whatever its capitals and marks, is the best match of all
    if typed == letters { total + START } else { total }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_found_by_its_start_a_words_start_and_a_part() {
        assert_eq!(score("sec", "SecondTester"), START);
        assert_eq!(score("test", "SecondTester"), WORD, "a capital after a small letter starts a word");
        assert_eq!(score("kova", "Olha_Kovalenko"), WORD);
        assert_eq!(score("cond", "SecondTester"), PART);
        assert_eq!(score("SECOND", "secondtester"), START, "any case");
    }

    #[test]
    fn the_whole_name_is_the_best_match() {
        assert!(score("secondtester", "SecondTester") > score("secondtester", "SecondTester2"));
        assert!(score("second tester", "SecondTester") > score("second", "SecondTester"));
        assert!(score("olha kovalenko", "Olha_Kovalenko") > score("olha", "Olha_Kovalenko"));
    }

    #[test]
    fn letters_in_order_and_typos_match_less_well() {
        let spread = score("sdt", "SecondTester");
        assert!((SPREAD_LEAST..=SPREAD).contains(&spread), "{spread}");
        assert_eq!(score("secnd", "SecondTester"), SPREAD - 1, "a letter left out is its letters in order");
        assert_eq!(score("sexond", "SecondTester"), TYPO - TYPO_EACH, "a wrong letter");
        assert_eq!(score("seocnd", "SecondTester"), TYPO - TYPO_EACH, "two swapped");
        assert_eq!(score("secondtezzer", "SecondTester"), TYPO - 2 * TYPO_EACH, "two typos in a long word");
        assert!(score("second", "SecondTester") > score("secnd", "SecondTester"));
        assert!(score("secnd", "SecondTester") > score("sexond", "SecondTester"));
    }

    #[test]
    fn what_is_too_short_or_too_far_matches_nothing() {
        assert_eq!(score("sx", "SecondTester"), 0, "two letters match only as a part");
        assert_eq!(score("sxc", "SecondTester"), 0, "three letters take no typo");
        assert_eq!(score("sexxnd", "SecondTester"), 0, "two typos in a short word");
        assert_eq!(score("third", "SecondTester"), 0);
        assert_eq!(score("second third", "SecondTester"), 0, "each word must match");
        assert_eq!(score("", "SecondTester"), 0);
        assert_eq!(score("__", "SecondTester"), 0);
        assert_eq!(score("a", ""), 0);
    }
}
