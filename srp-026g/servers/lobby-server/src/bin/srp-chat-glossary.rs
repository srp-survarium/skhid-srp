//! The chat translation's glossary tools (`docs/notes/chat-translation.md`).
//!
//! ```text
//! srp-chat-glossary seed <localization folder>
//! srp-chat-glossary slang <sample file>... [--dict <lang>=<word list>]... [--top <n>]
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
//!   line, e.g. a hunspell `.dic`) nor the glossary. For a person to read:
//!   the rows to add are chosen by hand.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lobby_server::v026g::translate::Lang;
use lobby_server::v026g::translate::glossary::Glossary;
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::game_data;
use survarium::v026g::localization::Localization;
use survarium::v026g::maps;

const USAGE: &str = "usage:
  srp-chat-glossary seed <localization folder>
  srp-chat-glossary slang <sample file>... [--dict <lang>=<word list>]... [--top <n>]";

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
            .filter(|(word, _)| !glossary.knows(word))
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

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.split_first() {
        Some((command, [folder])) if command == "seed" => seed(Path::new(folder)),
        Some((command, rest)) if command == "slang" => slang(rest),
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
