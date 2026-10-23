//! The chat translation's glossary tools (`docs/notes/chat-translation.md`).
//!
//! ```text
//! srp-chat-glossary seed <localization folder>
//! srp-chat-glossary slang <sample file>... [--dict <lang>=<word list>]... [--top <n>]
//! srp-chat-glossary judge < lines
//! srp-chat-glossary translate grok|claude < lines
//! srp-chat-glossary game-filter <localization folder> < lines
//! ```
//!
//! - `seed`: the glossary rows of the game's own names (items, artefacts
//!   among them, maps, factions) in its five languages, on standard output,
//!   in the glossary's format (`chat-glossary.tsv`). The names are those of
//!   the server's game data (`SRP_GAME_DATA`: its items' configs and maps);
//!   the texts are the five `localization/<language>/localization` files
//!   under `<localization folder>`, as `srp-game-data get` writes them from
//!   the players' game folder (the server data has only two languages).
//! - `slang`: slang candidates from a chat sample (`SRP_CHAT_SAMPLE_DIR`,
//!   `chat_sample`): per language of the writer's game, the words used most
//!   that are in neither the word list given for that language (one word a
//!   line, e.g. a hunspell `.dic`), nor the glossary, nor srp's swear list. For
//!   a person to read: the rows to add (to `chat-glossary.tsv`, or as swear
//!   words to `chat-swear.tsv`) are chosen by hand.
//! - `judge`: srp's word list's verdict on each line of standard input
//!   (`pass`, `blocked`, `obscene`), a tab, the line.
//! - `translate`: each line of standard input through the translation
//!   backend, as srp would send it (one call for the four other languages;
//!   the glossary's terms), to measure it: the milliseconds, `refused` or
//!   `ok`, the translations as JSON, a tab, the line. A line is the text, or
//!   the gate's test set's rows (`lang<TAB>...<TAB>text`: the first field is
//!   the language, the last the text). A line the word list refuses is not
//!   sent (`blocked`). Needs `XAI_API_KEY` (grok) or `ANTHROPIC_API_KEY`
//!   (claude); the model of `SRP_CHAT_TRANSLATE_MODEL` or the default. Run
//!   only lines the local gate let through: the rest must not reach the
//!   provider.
//! - `game-filter`: what the game's own client swear filter would mask in
//!   each line of standard input (the words, `|` between them; nothing for
//!   a clean line), a tab, the line. The five `swear_filter_data_a_c` files
//!   are read under `<localization folder>/<language>/`. Its rules are
//!   rebuilt from the data, not traced in the exe (`chat-translation.md`):
//!   a dictionary entry matches anywhere inside a word, each of its letters
//!   as typed or as one of the alphabet's look-alikes; a clean entry
//!   (flag 1) spares the whole word it equals. `--no-clean` drops the clean
//!   entries: the game masks "сабля" although its list has it clean.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lobby_server::v026g::translate::glossary::Glossary;
use lobby_server::v026g::translate::swear::{SwearFilter, Verdict};
use lobby_server::v026g::translate::{Lang, Line, Provider};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::game_data;
use survarium::v026g::localization::Localization;
use survarium::v026g::maps;

const USAGE: &str = "usage:
  srp-chat-glossary seed <localization folder>
  srp-chat-glossary slang <sample file>... [--dict <lang>=<word list>]... [--top <n>]
  srp-chat-glossary judge < lines
  srp-chat-glossary translate grok|claude < lines
  srp-chat-glossary game-filter <localization folder> [--no-clean] < lines";

/// The game's folder name of each language.
fn folder(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "english",
        Lang::Ru => "russian",
        Lang::Uk => "ukrainian",
        Lang::De => "german",
        Lang::Pl => "polish",
    }
}

/// The text keys named in a config: every `st_..._name` in its bytes.
fn name_keys(bytes: &[u8]) -> Vec<String> {
    let mut keys = Vec::new();
    let mut at = 0;
    while let Some(found) = bytes[at..].windows(3).position(|window| window == b"st_") {
        let start = at + found;
        let end = bytes[start..]
            .iter()
            .position(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_'))
            .map_or(bytes.len(), |length| start + length);
        let key = String::from_utf8_lossy(&bytes[start..end]).into_owned();
        if key.ends_with("_name") {
            keys.push(key);
        }
        at = end.max(start + 3);
    }
    keys
}

fn seed(localization: &Path) -> Result<(), String> {
    let mut texts = Vec::new();
    for lang in Lang::ALL {
        let path = localization
            .join("localization")
            .join(folder(lang))
            .join("localization");
        let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let parsed = Localization::parse(&bytes)
            .map_err(|error| format!("{}: {}", path.display(), error.0))?;
        texts.push((lang, parsed));
    }
    let data = game_data::shared();
    let dictionary = Dictionary::shared();
    // (kind, key), each once, in a stable order.
    let mut keys: BTreeSet<(&str, String)> = BTreeSet::new();
    for item in &dictionary.items {
        let kind = if item.cfg_name.contains("/artefacts/") {
            "artefact"
        } else {
            "item"
        };
        if let Some(bytes) = data.file(&item.cfg_name) {
            for key in name_keys(bytes) {
                keys.insert((kind, key));
            }
        }
    }
    for map in maps::all() {
        keys.insert(("map", map.map_name_key.to_owned()));
    }
    for faction in &dictionary.factions {
        keys.insert(("faction", format!("st_{}_faction", faction.name)));
    }
    for key in [
        "st_scavengers_faction",
        "st_border_faction",
        "st_black_market_faction",
        "st_renaissance_faction",
    ] {
        keys.insert(("faction", key.to_owned()));
    }
    println!("{}", Glossary::HEADER);
    let mut seen = BTreeSet::new();
    for (kind, key) in keys {
        let cells: Vec<String> = texts
            .iter()
            .map(|(_, texts)| {
                texts
                    .get(&key)
                    .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
                    .unwrap_or_default()
            })
            .collect();
        // A name every language writes the same says nothing to translate;
        // one the game lacks in English is not a name players see.
        let english = &cells[0];
        if english.is_empty()
            || cells.iter().all(|cell| cell == english)
            || cells
                .iter()
                .any(|cell| cell.contains('\t') || cell.contains('|'))
            || !seen.insert(cells.clone())
        {
            continue;
        }
        println!("{kind}\t{}\t{key}", cells.join("\t"));
    }
    Ok(())
}

fn slang(arguments: &[String]) -> Result<(), String> {
    let mut samples: Vec<PathBuf> = Vec::new();
    let mut dictionaries: HashMap<Lang, BTreeSet<String>> = HashMap::new();
    let mut top = 50;
    let mut rest = arguments.iter();
    while let Some(argument) = rest.next() {
        match argument.as_str() {
            "--dict" => {
                let value = rest.next().ok_or("--dict needs <lang>=<word list>")?;
                let (code, path) = value
                    .split_once('=')
                    .ok_or("--dict needs <lang>=<word list>")?;
                let lang = Lang::from_code(code).ok_or_else(|| format!("no language `{code}`"))?;
                let words =
                    std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
                // hunspell: `word/FLAGS`; a plain list: `word`.
                let set = dictionaries.entry(lang).or_default();
                for line in words.lines() {
                    if let Some(word) = line.split(['/', '\t', ' ']).next() {
                        set.insert(word.trim().to_lowercase());
                    }
                }
            }
            "--top" => {
                top = rest
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("--top needs a number")?;
            }
            path => samples.push(PathBuf::from(path)),
        }
    }
    if samples.is_empty() {
        return Err(USAGE.to_owned());
    }
    let glossary = Glossary::shared();
    let swear = SwearFilter::built_in();
    let mut counts: BTreeMap<Lang, HashMap<String, usize>> = BTreeMap::new();
    for path in &samples {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        for line in text.lines().filter(|line| !line.starts_with('#')) {
            // chat_sample's rows: time, writer's game language, guessed
            // language, channel, text.
            let fields: Vec<&str> = line.splitn(5, '\t').collect();
            let [_, game, _, _, body] = fields[..] else {
                continue;
            };
            let Some(lang) = Lang::from_code(game) else {
                continue;
            };
            let words = counts.entry(lang).or_default();
            for word in body
                .split(|c: char| !(c.is_alphabetic() || c == '\''))
                .map(|word| word.trim_matches('\'').to_lowercase())
                .filter(|word| word.chars().count() >= 2)
            {
                *words.entry(word).or_default() += 1;
            }
        }
    }
    for (lang, words) in counts {
        let known = dictionaries.get(&lang);
        let mut candidates: Vec<(usize, String)> = words
            .into_iter()
            .filter(|(word, _)| known.is_none_or(|known| !known.contains(word)))
            .filter(|(word, _)| !glossary.knows(word) && !swear.swears(word))
            .map(|(word, count)| (count, word))
            .collect();
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        println!(
            "# {} ({}): {}",
            lang.name(),
            lang.code(),
            if known.is_some() {
                "not in the word list"
            } else {
                "no word list given: every word"
            }
        );
        for (count, word) in candidates.into_iter().take(top) {
            println!("{count}\t{word}");
        }
    }
    Ok(())
}

fn translate(provider: &str) -> Result<(), String> {
    let provider = match provider {
        "grok" => Provider::Grok,
        "claude" => Provider::Claude,
        other => return Err(format!("no provider `{other}`: grok or claude")),
    };
    let model = std::env::var("SRP_CHAT_TRANSLATE_MODEL")
        .ok()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| provider.default_model().to_owned());
    let backend = provider.backend(&model, std::time::Duration::from_secs(10))?;
    let filter = SwearFilter::built_in();
    let glossary = Glossary::shared();
    for row in std::io::stdin().lines() {
        let row = row.map_err(|error| error.to_string())?;
        let fields: Vec<&str> = row.split('\t').collect();
        let text = fields.last().copied().unwrap_or_default();
        let from = (fields.len() > 1)
            .then(|| Lang::from_code(fields[0]))
            .flatten();
        if filter.judge(text) != Verdict::Pass {
            println!("-\tblocked\t-\t{row}");
            continue;
        }
        let to: Vec<Lang> = Lang::ALL
            .into_iter()
            .filter(|lang| Some(*lang) != from)
            .collect();
        let line = Line {
            text: text.to_owned(),
            from,
            glossary: glossary.entries(text, from, &to),
            to,
        };
        let started = std::time::Instant::now();
        let result = backend.translate(&line);
        let ms = started.elapsed().as_millis();
        match result {
            Ok(translated) if translated.refused => println!("{ms}\trefused\t-\t{row}"),
            Ok(translated) => {
                let lines: serde_json::Map<String, serde_json::Value> = translated
                    .lines
                    .into_iter()
                    .map(|(lang, text)| (lang.code().to_owned(), serde_json::Value::from(text)))
                    .collect();
                println!("{ms}\tok\t{}\t{row}", serde_json::Value::Object(lines));
            }
            Err(error) => println!("{ms}\terror\t{error}\t{row}"),
        }
    }
    Ok(())
}

fn judge() -> Result<(), String> {
    let filter = SwearFilter::built_in();
    for line in std::io::stdin().lines() {
        let line = line.map_err(|error| error.to_string())?;
        let verdict = match filter.judge(&line) {
            Verdict::Pass => "pass",
            Verdict::Blocked => "blocked",
            Verdict::Obscene => "obscene",
        };
        println!("{verdict}\t{line}");
    }
    Ok(())
}

/// One language's list of the game's client swear filter.
struct GameList {
    /// letter -> what is typed for it.
    alphabet: HashMap<char, Vec<Vec<char>>>,
    swears: Vec<Vec<char>>,
    clean: BTreeSet<String>,
    separators: BTreeSet<char>,
}

/// A string of a game config as its bytes say (read as Windows-1251; the
/// files are UTF-8).
fn utf8(read: &str) -> String {
    let (bytes, _, unmappable) = encoding_rs::WINDOWS_1251.encode(read);
    match (unmappable, String::from_utf8(bytes.into_owned())) {
        (false, Ok(text)) => text,
        _ => read.to_owned(),
    }
}

impl GameList {
    fn read(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let root = binary_config_parser::tree::ConfigValue::parse(&bytes)
            .map_err(|error| format!("{}: {}", path.display(), error.0))?;
        let data = root
            .req("swear_filter_data")
            .map_err(|error| format!("{}: {}", path.display(), error.0))?;
        let strings = |key: &str| -> Vec<String> {
            data.get(key)
                .map(|list| {
                    list.children()
                        .into_iter()
                        .filter_map(|value| value.as_str().map(utf8))
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut list = GameList {
            alphabet: HashMap::new(),
            swears: Vec::new(),
            clean: BTreeSet::new(),
            separators: strings("separators").concat().chars().collect(),
        };
        for line in strings("alphabet") {
            let mut parts = line.split(' ').filter(|part| !part.is_empty());
            if let Some(letter) = parts.next().and_then(|letter| letter.chars().next()) {
                list.alphabet
                    .entry(letter)
                    .or_default()
                    .extend(parts.map(|typed| typed.to_lowercase().chars().collect()));
            }
        }
        for line in strings("swear_dictionary") {
            if let Some((word, flag)) = line.trim().rsplit_once(' ') {
                let word = word.trim().to_lowercase();
                if flag == "1" {
                    list.clean.insert(word);
                } else if !word.is_empty() {
                    list.swears.push(word.chars().collect());
                }
            }
        }
        Ok(list)
    }

    /// Whether `entry` matches `word` from `at` on, letter by letter.
    fn matches_at(&self, word: &[char], at: usize, entry: &[char]) -> bool {
        let Some((&letter, rest)) = entry.split_first() else {
            return true;
        };
        if word.get(at) == Some(&letter) && self.matches_at(word, at + 1, rest) {
            return true;
        }
        self.alphabet.get(&letter).is_some_and(|typed| {
            typed.iter().any(|typed| {
                word.get(at..at + typed.len()) == Some(typed.as_slice())
                    && self.matches_at(word, at + typed.len(), rest)
            })
        })
    }

    fn masks(&self, word: &str) -> bool {
        let lower = word.to_lowercase();
        if self.clean.contains(&lower) {
            return false;
        }
        let chars: Vec<char> = lower.chars().collect();
        self.swears
            .iter()
            .any(|entry| (0..chars.len()).any(|at| self.matches_at(&chars, at, entry)))
    }
}

fn game_filter(folder: &Path, no_clean: bool) -> Result<(), String> {
    let mut lists = Lang::ALL
        .iter()
        .map(|lang| {
            GameList::read(
                &folder
                    .join(self::folder(*lang))
                    .join("swear_filter_data_a_c"),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if no_clean {
        // The game masks "сабля" although its list has it clean (the owner's):
        // its clean entries do not work as rebuilt here [the reason is not traced].
        for list in &mut lists {
            list.clean.clear();
        }
    }
    for line in std::io::stdin().lines() {
        let line = line.map_err(|error| error.to_string())?;
        // A word: what lies between the separators of every list that are
        // not typed for a letter, and white space.
        let typed: BTreeSet<char> = lists
            .iter()
            .flat_map(|list| list.alphabet.values().flatten().flatten().copied())
            .collect();
        let masked: Vec<&str> = line
            .split(|c: char| {
                c.is_whitespace()
                    || (!c.is_alphanumeric()
                        && !typed.contains(&c)
                        && lists.iter().any(|list| list.separators.contains(&c)))
            })
            .filter(|word| !word.is_empty())
            .filter(|word| lists.iter().any(|list| list.masks(word)))
            .collect();
        println!("{}\t{line}", masked.join("|"));
    }
    Ok(())
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.split_first() {
        Some((command, [folder])) if command == "seed" => seed(Path::new(folder)),
        Some((command, rest)) if command == "slang" => slang(rest),
        Some((command, [])) if command == "judge" => judge(),
        Some((command, [provider])) if command == "translate" => translate(provider),
        Some((command, [folder])) if command == "game-filter" => {
            game_filter(Path::new(folder), false)
        }
        Some((command, [folder, flag])) if command == "game-filter" && flag == "--no-clean" => {
            game_filter(Path::new(folder), true)
        }
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
