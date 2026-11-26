//! Client texts the server sends in fields the client shows as they are (store cards, result window rows), and the
//! event categories of the result window.

use std::collections::HashMap;

use client_files::pyjson::Style;
use client_files::{dict, Dict, Value};

use crate::drops::py_int;
use crate::util::{json, need, Cx, LANGS};
use crate::{data_err, Result};

/// The keys of the store's action cards and tab (the client does not localize these fields): the tab name, the premium
/// card label and description, the contract card label and description.
const STORE_KEYS: [&str; 5] = [
    "st_special_offer",
    "st_premium_access_7d_voucher_league_desc",
    "st_gold_advert_1_hint",
    "st_contract_with_profiteer",
    "st_contract_complete_after_win_descr",
];

/// `{key: {language: text}}` of `keys` in the ten languages; a key missing in a language is an error.
fn texts_of(cx: &Cx, table: &'static str, keys: &[String]) -> Result<Dict> {
    let langs = cx.langs()?;
    let mut out = Dict::new();
    for k in keys {
        let mut per = Dict::new();
        for l in LANGS {
            let t = langs[l].get(k).ok_or_else(|| data_err(table, format!("{k} is missing in the {l} language file")))?;
            per.set(l, t.as_str());
        }
        out.set(k.as_str(), per);
    }
    Ok(out)
}

/// store_texts_069d.json
pub fn store_texts(cx: &Cx) -> Result<Vec<u8>> {
    let keys: Vec<String> = STORE_KEYS.iter().map(|s| s.to_string()).collect();
    let d = texts_of(cx, "store_texts_069d.json", &keys)?;
    Ok(json(&Value::Dict(d), &Style::indent(1).utf8().sorted(), true))
}

/// event_categories_069d.json: `ui_category` of every event of `gameplay/awards/events.options`.
pub fn event_categories(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "event_categories_069d.json";
    let ev = cx.cfg("gameplay/awards/events.options")?;
    let mut cats = Dict::new();
    for (name, e) in need(T, &ev, "events")?.entries() {
        let c = e.get("ui_category").ok_or_else(|| data_err(T, format!("event {name} has no ui_category")))?;
        cats.set(name, py_int(c));
    }
    Ok(json(&Value::Dict(cats), &Style::indent(1).utf8().sorted(), true))
}

fn name_keys(cx: &Cx) -> Result<(Vec<String>, Vec<String>)> {
    const T: &str = "bot_names_069d.json";
    let n = cx.cfg("gameplay/players/ai_players_names.options")?;
    let keys = |k: &str| -> Result<Vec<String>> {
        need(T, &n, k)?.items().iter().map(|x| x.as_str().map(str::to_string).ok_or_else(|| data_err(T, format!("{k}: not a text key")))).collect()
    };
    Ok((keys("players_1st_names")?, keys("players_2nd_names")?))
}

/// bot_names_069d.json: the client's bot name key lists (`ai_players_names.options`) and their texts per language.
pub fn bot_names(cx: &Cx) -> Result<Vec<u8>> {
    let (first, second) = name_keys(cx)?;
    let all: Vec<String> = first.iter().chain(second.iter()).cloned().collect();
    let texts = texts_of(cx, "bot_names_069d.json", &all)?;
    Ok(json(&dict! { "first" => first, "second" => second, "texts" => texts }, &Style::indent(1).utf8().sorted(), true))
}

/// The English language file the English bot name list is read from (the base volume's copy).
const BOT_NAMES_EN_VOLUME: u32 = 0;
/// Bot names per list: `st_bot_1st_name_1..=31`, `st_bot_2nd_name_1..=31`.
const BOT_NAMES_EN_COUNT: usize = 31;
/// CHOICE: names the list leaves empty although the language file has them (the first version of this table had no
/// text for `st_bot_2nd_name_25` "Scavenger"; kept so the bot names stay as before).
const BOT_NAMES_EN_EMPTY: [(&str, usize); 1] = [("2nd", 25)];

/// bot_names_en.json: the English texts of `st_bot_1st_name_N` / `st_bot_2nd_name_N` by N (null where the language
/// file has no such key).
pub fn bot_names_en(cx: &Cx) -> Result<Vec<u8>> {
    let en: HashMap<String, String> = cx.lang_in(Some(BOT_NAMES_EN_VOLUME), "english")?;
    let list = |kind: &str| -> Value {
        Value::List(
            (1..=BOT_NAMES_EN_COUNT)
                .map(|i| match en.get(&format!("st_bot_{kind}_name_{i}")) {
                    Some(s) if !BOT_NAMES_EN_EMPTY.contains(&(kind, i)) => Value::from(s.as_str()),
                    _ => Value::Null,
                })
                .collect(),
        )
    };
    Ok(json(&dict! { "first" => list("1st"), "second" => list("2nd") }, &Style::default(), false))
}
