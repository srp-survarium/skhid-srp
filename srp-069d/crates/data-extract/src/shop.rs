//! shop_069d.tsv: our shop prices joined with the item dictionary.
//!
//! The prices, ranks, sold flags, sources and notes are ours (fan wiki price tables of 2022 with source notes, and
//! CHOICE prices): `ours/shop_prices_069d.tsv`. The dictionary columns (config name, English name, faction, level,
//! category, type, section, clip size, cost, currency) come from the game files; a row whose `dict_cost_note` is 1 gets
//! the note `dict cost X currency Y` appended from the dictionary.

use client_files::pyjson;
use client_files::Value;

use crate::items::ItemRow;
use crate::util::Cx;
use crate::{data_err, Result};

const OURS: &str = include_str!("../ours/shop_prices_069d.tsv");
const T: &str = "shop_069d.tsv";

const COLS: [&str; 21] = [
    "dict_id", "cfg_name", "english_name", "price", "currency", "faction", "rank_required", "sold", "source", "note", "pack_price",
    "rounds_per_pack", "dict_clip_size", "wiki_name", "wiki_level", "dict_level", "dict_category", "dict_type", "section", "dict_cost",
    "dict_currency",
];

/// `p.get(k, '')` as text.
fn col(r: &ItemRow, k: &str) -> String {
    r.r.get(k).map(pyjson::str).unwrap_or_default()
}

pub fn build(cx: &Cx, rows: &[ItemRow]) -> Result<Vec<u8>> {
    let game = cx.game()?;
    let english = cx.lang_in(None, "english")?;
    let mut lines = OURS.lines().map(|l| l.trim_end_matches('\r'));
    let head: Vec<&str> = lines.next().ok_or_else(|| data_err(T, "empty price table"))?.split('\t').collect();
    let at = |name: &str| head.iter().position(|h| *h == name).ok_or_else(|| data_err(T, format!("price table has no {name} column")));
    let ix: Vec<usize> = ["dict_id", "price", "currency", "rank_required", "sold", "source", "note", "pack_price", "rounds_per_pack", "wiki_name", "wiki_level", "dict_cost_note"]
        .iter()
        .map(|c| at(c))
        .collect::<Result<_>>()?;
    let mut out = COLS.join("\t");
    out.push('\n');
    for l in lines {
        if l.is_empty() {
            continue;
        }
        let f: Vec<&str> = l.split('\t').collect();
        let g = |i: usize| f.get(ix[i]).copied().unwrap_or("");
        let did: i64 = g(0).parse().map_err(|_| data_err(T, format!("bad dict id {:?}", g(0))))?;
        // the dictionary row of this id (a later section's copy wins, as the item list is built)
        let it = rows.iter().rev().find(|r| r.dict_id == did).ok_or_else(|| data_err(T, format!("dict id {did} is not in the item dictionary")))?;
        let entry = &game[it.section][it.cfg_name.as_str()];
        let ui = &entry["ui_desc"];
        let key = match &ui["text_descriptions"] {
            Value::Dict(td) => td.get("name").map(pyjson::str).unwrap_or_default(),
            _ => ui.get("name").map(pyjson::str).unwrap_or_default(),
        };
        let name = if key.is_empty() { String::new() } else { english.get(&key).cloned().unwrap_or_default() };
        let mut note = g(6).to_string();
        if g(11) == "1" {
            let suffix = format!("dict cost {} currency {}", col(it, "cost"), col(it, "currency"));
            note = if note.is_empty() { suffix } else { format!("{note}; {suffix}") };
        }
        let clip = if it.section == "ammo_grenades_etc" { col(it, "clip_size") } else { String::new() };
        let cells: [String; 21] = [
            did.to_string(),
            it.cfg_name.clone(),
            name,
            g(1).into(),
            g(2).into(),
            col(it, "faction"),
            g(3).into(),
            g(4).into(),
            g(5).into(),
            note,
            g(7).into(),
            g(8).into(),
            clip,
            g(9).into(),
            g(10).into(),
            col(it, "item_level"),
            col(it, "category_id"),
            col(it, "type"),
            it.section.to_string(),
            col(it, "cost"),
            col(it, "currency"),
        ];
        out.push_str(&cells.join("\t"));
        out.push('\n');
    }
    Ok(out.into_bytes())
}
