//! Which of the game's five languages a chat line is in, from its letters
//! and its commonest words: no model, no dependency, microseconds a line.
//!
//! It only has to decide who gets the original at once (a reader of the
//! line's own language) and whom to ask a translation for. A wrong guess
//! costs little: the backend says what the line really is in, and a reader
//! of that language gets the original (`Translated::source`).
//!
//! - **No language** for a line with no word of two letters or more, or
//!   whose words are all gamer shorthand every player reads ("gg", "ez",
//!   "lol", "afk", "+1"): it goes out as it is, to everybody.
//! - **Cyrillic:** Ukrainian by its own letters (і ї є ґ), Russian by its
//!   own (ы э ъ ё), then by common words; a tie is the sender's game
//!   language when that is Russian or Ukrainian, else Russian.
//! - **Latin:** Polish by its letters (ą ć ę ł ń ś ź ż), German by its own
//!   (ä ö ü ß), then by common words. A line with no sign is the sender's
//!   game language when that is English, German or Polish; else (a Russian
//!   game typing Latin: transliteration, or English) no language.

use super::Lang;

/// Shorthand every player reads in any language: never translated.
const SHORTHAND: &[&str] = &[
    "gg", "ggwp", "wp", "gl", "hf", "glhf", "ez", "ezz", "lol", "lmao", "rofl", "xd", "xdd", "kek",
    "afk", "brb", "np", "ty", "thx", "omg", "wtf", "ok", "okay", "kk", "k", "re", "gj", "nt", "nc",
    "noob", "nub", "imba", "op", "pls", "plz", "ppl", "irl", "rip", "ff", "gogo", "go", "ha",
    "haha", "hah", "hehe", "xaxa", "mb", "idk", "sry", "lag", "лол", "кек", "гг", "ок", "изи",
    "го", "ха", "хаха", "ххх", "ап", "нуб", "рип", "афк", "лаг",
];

const RUSSIAN: &[&str] = &[
    "что",
    "как",
    "это",
    "нет",
    "где",
    "кто",
    "сейчас",
    "надо",
    "нужно",
    "очень",
    "только",
    "уже",
    "спасибо",
    "привет",
    "его",
    "она",
    "они",
    "мой",
    "твой",
    "хорошо",
    "давай",
    "иди",
    "сюда",
    "туда",
    "здесь",
    "почему",
    "когда",
    "мне",
    "меня",
    "тебя",
    "есть",
    "будет",
    "был",
    "была",
    "всем",
    "ребята",
    "пацаны",
    "стреляй",
    "игра",
    "опять",
    "или",
    "если",
    "чтобы",
    "можно",
    "нельзя",
    "сколько",
];

const UKRAINIAN: &[&str] = &[
    "що",
    "як",
    "це",
    "ні",
    "немає",
    "нема",
    "дякую",
    "хлопці",
    "мене",
    "ласка",
    "гра",
    "гарна",
    "добре",
    "зараз",
    "треба",
    "чому",
    "коли",
    "де",
    "він",
    "вона",
    "вони",
    "мій",
    "твій",
    "наш",
    "ще",
    "вже",
    "тільки",
    "дуже",
    "привіт",
    "бо",
    "й",
    "або",
    "якщо",
    "тримаємо",
    "біля",
    "набої",
    "всім",
];

const ENGLISH: &[&str] = &[
    "the", "is", "you", "and", "what", "need", "at", "on", "im", "i'm", "this", "that", "anyone",
    "want", "to", "up", "my", "me", "it", "are", "we", "they", "come", "help", "where", "who",
    "how", "why", "near", "have", "has", "low", "with", "for", "of", "in", "team", "enemy", "can",
    "yes", "please", "thanks", "guys", "here", "there", "i", "a", "an", "be", "do", "dont",
    "don't", "not", "just", "get", "got", "lets", "let's", "one", "sniper", "camper", "roof",
    "base", "medic", "ammo", "game", "good", "nice", "again",
];

const GERMAN: &[&str] = &[
    "und", "ich", "nicht", "ist", "das", "der", "die", "du", "wer", "hat", "noch", "bin", "auf",
    "dem", "den", "mit", "ein", "eine", "einen", "ja", "nein", "gut", "gutes", "spiel", "leute",
    "danke", "bitte", "was", "wie", "wo", "mir", "mich", "sind", "habe", "hab", "kein", "keine",
    "schon", "auch", "aber", "oder", "jetzt", "hier", "da", "passt", "munition", "dach", "leer",
    "los", "komm", "warum",
];

const POLISH: &[&str] = &[
    "nie",
    "jest",
    "się",
    "sie",
    "jak",
    "co",
    "ktoś",
    "ktos",
    "kto",
    "mam",
    "ma",
    "dobra",
    "dobry",
    "gra",
    "dzięki",
    "dzieki",
    "wszystkim",
    "jestem",
    "tak",
    "czy",
    "ale",
    "już",
    "juz",
    "mnie",
    "ja",
    "gdzie",
    "tu",
    "coś",
    "cos",
    "siema",
    "elo",
    "idę",
    "ide",
    "chodź",
    "chodz",
    "pomocy",
    "na",
    "koło",
    "kolo",
    "dachu",
    "mostu",
    "kampi",
    "umieram",
    "apteczkę",
    "apteczke",
    "teraz",
    "tylko",
    "bardzo",
    "proszę",
    "prosze",
    "żeby",
    "zeby",
];

fn is_cyrillic(c: char) -> bool {
    matches!(c, '\u{0400}'..='\u{04FF}')
}

fn count_in(words: &[String], list: &[&str]) -> usize {
    words
        .iter()
        .filter(|word| list.contains(&word.as_str()))
        .count()
}

/// The line's language, if it has one worth translating. `hint` is the
/// sender's game language (`4F`).
pub fn detect(text: &str, hint: Option<Lang>) -> Option<Lang> {
    let words: Vec<String> = text
        .split(|c: char| !(c.is_alphabetic() || c == '\''))
        .map(|word| word.trim_matches('\'').to_lowercase())
        .filter(|word| word.chars().count() >= 1 && word.chars().any(char::is_alphabetic))
        .collect();
    if words.iter().all(|word| {
        word.chars().count() < 2 || SHORTHAND.contains(&word.as_str()) || is_laugh(word)
    }) {
        return None;
    }
    let (mut cyrillic, mut latin) = (0, 0);
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        if is_cyrillic(c) {
            cyrillic += 1;
        } else if c.is_ascii_alphabetic() || matches!(c, '\u{00C0}'..='\u{024F}') {
            latin += 1;
        }
    }
    let lower = text.to_lowercase();
    let letters = |set: &str| lower.chars().filter(|c| set.contains(*c)).count();
    if cyrillic > latin {
        let ukrainian = letters("іїєґ") * 2 + count_in(&words, UKRAINIAN);
        let russian = letters("ыэъё") * 2 + count_in(&words, RUSSIAN);
        return Some(if ukrainian > russian {
            Lang::Uk
        } else if russian > ukrainian {
            Lang::Ru
        } else {
            match hint {
                Some(Lang::Uk) => Lang::Uk,
                _ => Lang::Ru,
            }
        });
    }
    if latin == 0 {
        // Another script (Greek, CJK, ...): none of the five.
        return None;
    }
    let scores = [
        (Lang::Pl, letters("ąćęłńśźż") * 2 + count_in(&words, POLISH)),
        (Lang::De, letters("äöüß") * 2 + count_in(&words, GERMAN)),
        (Lang::En, count_in(&words, ENGLISH)),
    ];
    let best = scores.iter().map(|(_, score)| *score).max().unwrap_or(0);
    let leaders: Vec<Lang> = scores
        .iter()
        .filter(|(_, score)| best > 0 && *score == best)
        .map(|(lang, _)| *lang)
        .collect();
    match (leaders.as_slice(), hint) {
        ([one], _) => Some(*one),
        // A tie: the sender's own language if it is one of them.
        (tied, Some(hint)) if tied.contains(&hint) => Some(hint),
        (tied, _) if !tied.is_empty() => Some(tied[0]),
        // No sign at all.
        (_, Some(hint @ (Lang::En | Lang::De | Lang::Pl))) => Some(hint),
        _ => None,
    }
}

/// "hahaha", "хахаха", "ахах", "xaxa": laughter in any language.
fn is_laugh(word: &str) -> bool {
    word.chars().count() >= 2 && word.chars().all(|c| "haхаxаеe".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_language_by_its_letters_and_words() {
        let cases = [
            ("го на базу, там снайпер сидит", Lang::Ru),
            ("нуб тиммейты опять слились", Lang::Ru),
            ("кто с аптечкой? я на нуле", Lang::Ru),
            ("хлопці, тримаємо точку біля церкви", Lang::Uk),
            ("в кого є набої до мосінки?", Lang::Uk),
            ("гарна гра, всім дякую", Lang::Uk),
            ("need medic at B, im low hp", Lang::En),
            ("camper on the roof near the bridge lol", Lang::En),
            ("anyone want to squad up?", Lang::En),
            ("Wer hat noch Munition? Bin leer", Lang::De),
            ("Scharfschütze auf dem Dach, passt auf", Lang::De),
            ("gutes Spiel Leute", Lang::De),
            ("ktoś ma apteczkę? umieram", Lang::Pl),
            ("kampi na dachu koło mostu", Lang::Pl),
            ("dobra gra, dzięki wszystkim", Lang::Pl),
        ];
        for (text, lang) in cases {
            assert_eq!(detect(text, None), Some(lang), "{text}");
        }
    }

    #[test]
    fn shorthand_and_laughter_are_no_language() {
        for text in [
            "gg",
            "gg ez",
            "GG WP",
            "lol",
            "xD",
            "+1",
            "хахаха",
            "изи",
            "ок",
            "))",
            "1 2 3",
        ] {
            assert_eq!(detect(text, Some(Lang::Ru)), None, "{text}");
        }
    }

    #[test]
    fn the_senders_game_language_breaks_a_tie() {
        // Cyrillic with no sign of either: Ukrainian only from a Ukrainian game.
        assert_eq!(detect("мосінка", None), Some(Lang::Uk));
        assert_eq!(detect("танк слева", Some(Lang::Uk)), Some(Lang::Uk));
        assert_eq!(detect("танк слева", Some(Lang::En)), Some(Lang::Ru));
        // Latin with no sign: the sender's language if it writes Latin.
        assert_eq!(
            detect("Bratwurst Kartoffel", Some(Lang::De)),
            Some(Lang::De)
        );
        // A Russian game typing transliteration: left alone.
        assert_eq!(detect("privet rebyata", Some(Lang::Ru)), None);
    }
}
