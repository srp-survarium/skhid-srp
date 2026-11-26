//! Drop tables: module rarity, chests, post-match drops, the treasure hunt and the faction battle rewards.

use client_files::pyjson::{self, Style};
use client_files::{dict, list, Dict, Value};

use crate::util::{get_or, json, need, round, Cx};
use crate::{data_err, Result};

/// Python's `int(x)` of a number (floats truncate toward zero).
pub fn py_int(x: &Value) -> Value {
    match x {
        Value::Float(f) => Value::Int(f.trunc() as i64),
        Value::Bool(b) => Value::Int(*b as i64),
        x => x.clone(),
    }
}

/// module_drops_069d.json: `modules_drop_options` of game.options (the four 11 x 11 weight tables, the rarity table,
/// the rarity group, the used-weapon percentages and each module's first / second drop modifier).
pub fn module_drops(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "module_drops_069d.json";
    let m = need(T, cx.game()?, "modules_drop_options")?;
    let table = |name: &str| -> Result<Value> {
        let t = need(T, m, name)?.items();
        if t.len() != 11 || t.iter().any(|r| r.items().len() != 11) {
            return Err(data_err(T, format!("{name} is not 11 x 11")));
        }
        Ok(Value::List(t.iter().map(|r| Value::List(r.items().iter().map(|x| round(x, 6)).collect())).collect()))
    };
    let drop_mods: Vec<Value> = m["modules_additional_infos"]
        .items()
        .iter()
        .map(|r| list![&r["dict_id"], get_or(r, "first_drop_modifier", Value::Null), get_or(r, "second_drop_modifier", Value::Null)])
        .collect();
    let res = dict! {
        "tables" => dict! {
            "pair" => table("modules_mods_pair_prob")?,
            "second" => table("modules_mods_pair_second_rarity_prob")?,
            "pve" => table("modules_mods_pair_pve_prob")?,
            "fourth" => table("modules_mods_pair_fourth_rarity_prob")?,
        },
        "rarity" => Value::List(m["modules_rarity"].items().iter().map(|r| Value::List(r.items().iter().map(py_int).collect())).collect()),
        "rarity_group" => need(T, m, "module_rarity_modifier")?,
        "used_weapon_percent" => need(T, m, "used_weapon_dropping_modules_percent")?,
        "used_weapon_pve_percent" => need(T, m, "used_weapon_pve_dropping_modules_percent")?,
        "drop_mods" => drop_mods,
    };
    Ok(json(&res, &Style::compact(), true))
}

/// chests_069d.json: the chest items of the dictionary (category 31: name, extraction, drop type, count, fixed items)
/// and the `*_drop` tables of `gameplay/drops/chest_drops.options` (level row, groups with one amount range each).
pub fn chests(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "chests_069d.json";
    let cd = cx.cfg("gameplay/drops/chest_drops.options")?;
    let mut tables = Dict::new();
    for (name, t) in cd.entries() {
        if !name.ends_with("_drop") || t.as_dict().is_none() {
            continue;
        }
        let rows = need(T, t, "item_level_probabilities")?.items();
        if rows.iter().any(|r| *r != rows[0]) {
            return Err(data_err(T, format!("{name}: level rows differ")));
        }
        let mut groups = Vec::new();
        for grp in t["item_category_probabilities"].items() {
            let rar = grp["rarity"].items();
            if rar.iter().any(|r| *r != rar[0]) {
                return Err(data_err(T, format!("{name}: rarity rows differ")));
            }
            groups.push(dict! {
                "weight" => &grp["drop_weight"],
                "categories" => &grp["categories"],
                "ids" => get_or(grp, "items_dict_ids", Value::Null),
                "min" => &rar[0][0],
                "max" => &rar[0][1],
            });
        }
        tables.set(name, dict! { "levels" => &rows[0], "groups" => groups });
    }
    let mut chests = Dict::new();
    for (key, x) in cx.game()?["ammo_grenades_etc"].entries() {
        let p = &x["parameters"];
        if p["category_id"].as_i64() != Some(31) || p["category_id"].as_bool().is_some() {
            continue;
        }
        let mut items = Vec::new();
        for grp in x["items"].items() {
            for e in grp.items() {
                items.push(list![&e["dict_id"], &e["amount"]]);
            }
        }
        chests.set(
            pyjson::str(need(T, p, "dict_id")?),
            dict! {
                "name" => key,
                "extraction" => get_or(p, "extraction_type", Value::Null),
                "drop_type" => get_or(p, "drop_type", Value::Null),
                "items_count" => get_or(p, "items_count", Value::Null),
                "items" => items,
            },
        );
    }
    Ok(json(&dict! { "chests" => chests, "tables" => tables }, &Style::compact().utf8(), true))
}

/// drops_069d.json: the post-match drop rows of `drop_after_combat` (`gameplay/drops/drops.options`) the server uses:
/// spare parts (925) and silver (214) by match level, and the win items.
pub fn drops(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "drops_069d.json";
    let src = cx.cfg("gameplay/drops/drops.options")?;
    let d = need(T, &src, "drop_after_combat")?;
    let (mut details, mut money, mut win) = (Vec::new(), Vec::new(), Vec::new());
    let pick = |it: &Value, keys: &[&str]| Value::Dict(Dict::from_pairs(keys.iter().map(|k| (*k, it[*k].clone()))));
    for it in d["items"].items() {
        let did = it.get("dict_id").and_then(Value::as_i64);
        if did == Some(925) {
            details.push(pick(it, &["match_level", "min", "max", "premium_factor"]));
        } else if did == Some(214) {
            money.push(pick(it, &["match_level", "min", "max"]));
        } else if it.get("win_needed").is_some() && (it.get("collection").is_some() || did == Some(1872)) {
            let mut row = Dict::new();
            row.set("win_needed", &it["win_needed"]);
            row.set("ignore_loss_if_premium", get_or(it, "ignore_loss_if_premium", false));
            if it.get("collection").is_some() {
                let c = &it["collection"][0];
                row.set("dict_id", &c["dict_id"]);
                row.set("min", &c["min"]);
                row.set("max", &c["max"]);
                row.set("prob", round(&it["prob"], 6));
                row.set("match_level", &it["match_level"]);
            } else {
                row.set("dict_id", &it["dict_id"]);
                row.set("min", &it["amount"]);
                row.set("max", &it["amount"]);
                row.set("prob", 1.0);
            }
            win.push(Value::Dict(row));
        }
    }
    let out = dict! {
        "max_drop_items_count" => need(T, d, "max_drop_items_count")?,
        "details" => details,
        "money" => money,
        "win_items" => win,
    };
    Ok(json(&out, &Style::indent(1), false))
}

/// weekly_drop_069d.json (the treasure hunt pages of `weekly_drop_configs`, `rare_levels`) and
/// faction_battle_069d.json (`faction_battle_drop`: reward tiers per faction, the item dropped after a win).
pub fn hunt(cx: &Cx) -> Result<(Vec<u8>, Vec<u8>)> {
    const T: &str = "weekly_drop_069d.json";
    let d = cx.cfg("gameplay/drops/drops.options")?;
    let f32s = |x: &Value| Value::Float(pyjson::f32_shortest(x.as_f64().unwrap_or(0.0)));
    let mut configs = Dict::new();
    for (name, c) in need(T, &d, "weekly_drop_configs")?.entries() {
        let mut levels = Vec::new();
        for pages in c["drop_by_level"].items() {
            let mut row = Vec::new();
            for pg in pages.items() {
                let mut items = Vec::new();
                for it in pg["items"].items() {
                    let ids = match &it["dict_id"] {
                        Value::List(_) => it["dict_id"].clone(),
                        x => list![x],
                    };
                    items.push(dict! {
                        "ids" => ids,
                        "min" => &it["min"],
                        "max" => &it["max"],
                        "prob" => f32s(&it["prob"]),
                        "rare_level" => get_or(it, "rare_level", 0),
                        "item_quality" => get_or(it, "item_quality", 0),
                    });
                }
                row.push(dict! { "roulette_prob" => f32s(&pg["roulette_prob"]), "items" => items });
            }
            levels.push(Value::List(row));
        }
        configs.set(
            name,
            dict! {
                "max_free_drop_cards_in_period" => &c["max_free_drop_cards_in_period"],
                "max_free_drop_cards" => &c["max_free_drop_cards"],
                "can_change_drop_level" => &c["can_change_drop_level"],
                "drop_cards_to_open" => &c["drop_cards_to_open"],
                "premium_money_to_open" => &c["premium_money_to_open"],
                "levels" => levels,
            },
        );
    }
    let weekly = json(&dict! { "configs" => configs, "rare_levels" => need(T, &d, "rare_levels")? }, &Style::compact(), false);
    let fb = need("faction_battle_069d.json", &d, "faction_battle_drop")?;
    // faction ids of the client: 1 Scavengers, 2 Black Market, 3 Renaissance Army, 4 Fringe (key "edge")
    let mut fac = Dict::new();
    for (k, n) in [("scavengers", 1), ("blackmarket", 2), ("renaissance", 3), ("edge", 4)] {
        let rows: Vec<Value> =
            fb[k].items().iter().map(|r| dict! { "reputation" => &r["reputation"], "dict_id" => &r["dict_id"], "amount" => &r["amount"] }).collect();
        fac.set(n.to_string(), rows);
    }
    let faction = json(&dict! { "item_to_drop_after_win" => &fb["item_to_drop_after_win"], "factions" => fac }, &Style::indent(1), false);
    Ok((weekly, faction))
}
