//! Shared pieces of the extractors: the decoded configs, output in the tables' layouts.

use std::cell::OnceCell;
use std::collections::HashMap;

use client_files::pyjson::{self, Style};
use client_files::{Encoding, Value};

use crate::{data_err, Result, Source};

/// `gameplay/game.options`: the item dictionary and most rule tables.
pub const GAME: &str = "gameplay/game.options";

/// The ten languages of the client's language files, in the order the tables list them.
pub const LANGS: [&str; 10] = ["russian", "english", "german", "polish", "ukrainian", "french", "spanish", "chinese", "portuguese", "turkish"];

pub struct Cx<'a> {
    pub src: &'a Source,
    game: OnceCell<Value>,
    langs: OnceCell<HashMap<&'static str, HashMap<String, String>>>,
}

impl<'a> Cx<'a> {
    pub fn new(src: &'a Source) -> Result<Cx<'a>> {
        Ok(Cx { src, game: OnceCell::new(), langs: OnceCell::new() })
    }

    /// The newest copy of a config, decoded.
    pub fn cfg(&self, path: &str) -> Result<Value> {
        let b = self.src.game.read(path)?;
        Ok(client_files::decode_options(&b, Encoding::Cp1251)?)
    }

    /// A config as volume `vol` stores it.
    pub fn cfg_in(&self, vol: u32, path: &str) -> Result<Value> {
        let b = self.src.game.read_in(vol, path)?;
        Ok(client_files::decode_options(&b, Encoding::Cp1251)?)
    }

    /// `gameplay/game.options` (decoded once).
    pub fn game(&self) -> Result<&Value> {
        if let Some(g) = self.game.get() {
            return Ok(g);
        }
        let g = self.cfg(GAME)?;
        Ok(self.game.get_or_init(|| g))
    }

    /// Every language file's `strings` (key -> text) of the ten languages, newest copies.
    pub fn langs(&self) -> Result<&HashMap<&'static str, HashMap<String, String>>> {
        if let Some(l) = self.langs.get() {
            return Ok(l);
        }
        let mut all = HashMap::new();
        for lang in LANGS {
            all.insert(lang, self.lang_in(None, lang)?);
        }
        Ok(self.langs.get_or_init(|| all))
    }

    /// One language file (`localization/<lang>/localization`, UTF-8 strings): key -> text. `vol` = a given volume, or
    /// the newest copy.
    pub fn lang_in(&self, vol: Option<u32>, lang: &str) -> Result<HashMap<String, String>> {
        let path = format!("localization/{lang}/localization");
        let b = match vol {
            Some(v) => self.src.game.read_in(v, &path)?,
            None => self.src.game.read(&path)?,
        };
        let v = client_files::decode_options(&b, Encoding::Utf8)?;
        let strings = v.get("strings").and_then(Value::as_dict).ok_or_else(|| data_err("language files", format!("{path} has no strings table")))?;
        Ok(strings.iter().map(|(k, t)| (k.to_string(), pyjson::str(t))).collect())
    }
}

/// JSON in a table's layout; `newline` = the file ends with a line break.
pub fn json(v: &Value, style: &Style, newline: bool) -> Vec<u8> {
    let mut s = pyjson::dumps(v, style);
    if newline {
        s.push('\n');
    }
    s.into_bytes()
}

/// A required member, or an error naming the table and the key.
pub fn need<'v>(table: &'static str, v: &'v Value, key: &str) -> Result<&'v Value> {
    v.get(key).ok_or_else(|| data_err(table, format!("missing `{key}`")))
}

/// A required integer member.
pub fn int(table: &'static str, v: &Value, key: &str) -> Result<i64> {
    need(table, v, key)?.as_i64().ok_or_else(|| data_err(table, format!("`{key}` is not an integer")))
}

/// `d.get(key, default)`.
pub fn get_or(v: &Value, key: &str, default: impl Into<Value>) -> Value {
    v.get(key).cloned().unwrap_or_else(|| default.into())
}

/// `round(float(x), n)` of a number member as a Value.
pub fn round(x: &Value, n: usize) -> Value {
    Value::Float(pyjson::round(x.as_f64().unwrap_or(0.0), n))
}
