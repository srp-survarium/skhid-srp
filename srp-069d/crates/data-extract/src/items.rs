//! Tables of the item dictionary (`gameplay/game.options`): items, shop, rule tables, skins, hands, decals, vouchers.

use client_files::pyjson::{self, Style};
use client_files::{dict, Dict, Value};

use crate::util::{get_or, json, need, Cx};
use crate::{data_err, Result};

const SECTIONS: [&str; 5] = ["weapons", "equipment", "ammo_grenades_etc", "camo_decal", "weapon_modules"];

/// Columns of items_069d.tsv.
const COLS: [&str; 21] = [
    "dict_id", "cfg_name", "section", "category_id", "type", "item_level", "faction", "match_making_level", "cost", "currency",
    "base_item_dict_id", "weight", "drop_weight", "sale_confirmation_level", "rarity", "ui_name", "icon", "faction_affinity",
    "is_stackable", "data_type", "stack_size",
];

/// One dictionary item: its `parameters` plus the derived columns.
pub struct ItemRow {
    pub dict_id: i64,
    pub cfg_name: String,
    pub section: &'static str,
    /// the row's fields (`parameters` with the derived columns set)
    pub r: Dict,
}

impl ItemRow {
    /// A column as the table writes it.
    pub fn col(&self, c: &str) -> String {
        self.r.get(c).map(pyjson::str).unwrap_or_default()
    }
}

fn as_list(v: Option<&Value>) -> &[Value] {
    v.and_then(Value::as_list).unwrap_or(&[])
}

/// Every item with a `parameters.dict_id`, sorted by dict id (stable: equal ids keep section order).
pub fn rows(cx: &Cx) -> Result<Vec<ItemRow>> {
    let g = cx.game()?;
    let cats: Vec<(&Value, &Value)> = g["item_categories"].items().iter().map(|c| (&c["category_id"], c)).collect();
    let cat = |id: &Value| cats.iter().rev().find(|(k, _)| *k == id).map(|(_, c)| *c);
    let mut rows = Vec::new();
    for sec in SECTIONS {
        for (k, v) in g[sec].entries() {
            let Some(p) = v.get("parameters").and_then(Value::as_dict) else { continue };
            let Some(did) = p.get("dict_id") else { continue };
            let ui = &v["ui_desc"];
            let mut r = p.clone();
            r.set("cfg_name", k);
            r.set("section", sec);
            r.set("ui_name", get_or(ui, "name", ""));
            let icon = match ui.get("icon") {
                Some(Value::List(l)) => Value::from(l.iter().map(pyjson::str).collect::<Vec<_>>().join(",")),
                Some(x) => x.clone(),
                None => Value::from(""),
            };
            r.set("icon", icon);
            let rar = p.get("rarity");
            let rl = as_list(rar);
            let rarity = if rl.iter().all(|x| !matches!(x, Value::List(_))) {
                rl.iter().map(pyjson::str).collect::<Vec<_>>().join(",")
            } else {
                pyjson::str(rar.unwrap_or(&Value::Null)).replace(' ', "")
            };
            r.set("rarity", rarity);
            r.set("is_stackable", p.get("category_id").and_then(cat).map(|c| get_or(c, "is_stackable", "")).unwrap_or_else(|| "".into()));
            let data_type = match v.get("data") {
                Some(Value::Dict(d)) => d.get("type").cloned().unwrap_or_else(|| "".into()),
                _ => "".into(),
            };
            r.set("data_type", data_type);
            r.set("stack_size", p.get("stack_size").cloned().unwrap_or_else(|| "".into()));
            let dict_id = did.as_i64().ok_or_else(|| data_err("items", format!("{k}: dict_id is not an integer")))?;
            rows.push(ItemRow { dict_id, cfg_name: k.to_string(), section: sec, r });
        }
    }
    rows.sort_by_key(|r| r.dict_id);
    Ok(rows)
}

/// items_069d.tsv
pub fn tsv(rows: &[ItemRow]) -> Vec<u8> {
    let mut s = COLS.join("\t");
    s.push('\n');
    for r in rows {
        s.push_str(&COLS.iter().map(|c| r.col(c)).collect::<Vec<_>>().join("\t"));
        s.push('\n');
    }
    s.into_bytes()
}

/// shop_069d.tsv: our price rows (`ours/shop_prices_069d.tsv`) joined with the dictionary's columns.
pub fn shop(cx: &Cx, rows: &[ItemRow]) -> Result<Vec<u8>> {
    crate::shop::build(cx, rows)
}

/// `[x, ...]` of a list member, or `[]` (an empty table decodes as `{}`).
fn list_or_empty(v: &Value) -> Value {
    match v {
        Value::List(_) => v.clone(),
        _ => Value::List(vec![]),
    }
}

fn attach_to(lst: Option<&Value>) -> Value {
    Value::List(
        as_list(lst)
            .iter()
            .map(|e| match e {
                Value::Str(s) => client_files::list![s.as_str(), "attach_params"],
                e => client_files::list![&e["slot"], get_or(e, "attach_params", "attach_params")],
            })
            .collect(),
    )
}

fn attach_block(b: &Value) -> Dict {
    let mut e = Dict::new();
    let b = if b.as_dict().is_some() { b } else { &Value::Null };
    let add: Vec<Value> = as_list(b.get("slots_to_add")).iter().map(|s| client_files::list![&s["id"], get_or(s, "accept_module", true)]).collect();
    let dis: Vec<Value> = as_list(b.get("slots_to_disable")).iter().map(|s| s["id"].clone()).collect();
    let ovr: Vec<Value> = as_list(b.get("slots_to_override"))
        .iter()
        .filter(|s| s.get("accept_module").is_some())
        .map(|s| client_files::list![&s["id"], &s["accept_module"]])
        .collect();
    if !add.is_empty() {
        e.set("add", add);
    }
    if !dis.is_empty() {
        e.set("disable", dis);
    }
    if !ovr.is_empty() {
        e.set("override", ovr);
    }
    e
}

/// game_tables_069d.json: the rule tables of game.options the server reads (see crates/game-data/README.md).
pub fn game_tables(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "game_tables_069d.json";
    let g = cx.game()?;
    // per item parameters by dict id (a later section's copy replaces an earlier one in place)
    let mut params: Vec<(i64, &Value)> = Vec::new();
    for sec in ["weapons", "equipment", "weapon_modules", "ammo_grenades_etc"] {
        for (_, x) in g[sec].entries() {
            let pr = &x["parameters"];
            if let Some(did) = pr.get("dict_id").and_then(Value::as_i64) {
                match params.iter_mut().find(|(d, _)| *d == did) {
                    Some(slot) => slot.1 = pr,
                    None => params.push((did, pr)),
                }
            }
        }
    }
    let mut items = Dict::new();
    for (did, pr) in params {
        let mut e = Dict::new();
        if let Some(x) = pr.get("base_rarity_level") {
            e.set("base_rarity_level", x);
        }
        if let Some(m) = pr.get("available_modifications_mask") {
            let idx: Vec<Value> = m.items().iter().enumerate().filter(|(_, v)| v.truthy()).map(|(i, _)| Value::from(i)).collect();
            e.set("mask", idx);
        }
        for k in ["details_to_craft", "premium_to_craft", "details_to_reroll", "premium_to_reroll", "pocket_contains"] {
            if let Some(x) = pr.get(k) {
                e.set(k, x);
            }
        }
        if let Some(dm) = pr.get("default_modifications") {
            e.set("default_modifications", list_or_empty(dm));
        }
        if !e.is_empty() {
            items.set(did.to_string(), e);
        }
    }
    let mut groups: Vec<(String, Dict)> = Vec::new();
    for m in g["modifications"].items() {
        if get_or(m, "drop_weight", 0).as_f64().unwrap_or(0.0) > 0.0 {
            let key = pyjson::str(&m["group"]);
            let i = match groups.iter().position(|(k, _)| *k == key) {
                Some(i) => i,
                None => {
                    let mut d = Dict::new();
                    d.set("type", &m["type"]);
                    d.set("grades", Value::List(vec![]));
                    groups.push((key, d));
                    groups.len() - 1
                }
            };
            if let Some(Value::List(l)) = groups[i].1.get_mut("grades") {
                l.push(client_files::list![&m["grade"], &m["drop_weight"]]);
            }
        }
    }
    let weapons = &g["weapons"];
    let weapon_slots = Dict::from_pairs(weapons.entries().map(|(k, v)| (k, Value::List(v["slots"].items().iter().map(|s| s["id"].clone()).collect()))));
    let compat = Dict::from_pairs(weapons.entries().map(|(k, v)| (k, get_or(v, "compatible_ammo", Value::List(vec![])))));
    let defaults_of = |w: &Value| -> Vec<Value> {
        let dm: Vec<&Value> = match w.get("default_modules") {
            Some(Value::Dict(d)) => d.values().collect(),
            Some(v) if v.truthy() => v.items().iter().collect(),
            _ => vec![],
        };
        dm.into_iter().map(|e| client_files::list![&e["slot"], &e["module"]]).collect()
    };
    let weapon_default_modules = Dict::from_pairs(weapons.entries().filter_map(|(k, v)| {
        let d = defaults_of(v);
        (!d.is_empty()).then(|| (k, Value::List(d)))
    }));
    let mut module_attach = Dict::new();
    for (k, m) in g["weapon_modules"].entries() {
        let mut e = Dict::new();
        e.set("to", attach_to(m.get("slots_to_attach_to")));
        let mut exs = Dict::new();
        if let Some(Value::Dict(ex)) = m.get("exceptions") {
            for (w, x) in ex.iter() {
                if let Some(s) = x.as_dict().and_then(|d| d.get("slots_to_attach_to")) {
                    exs.set(w, attach_to(Some(s)));
                }
            }
        }
        if !exs.is_empty() {
            e.set("ex", exs);
        }
        let mut blocks = Dict::new();
        for (n, b) in m.entries() {
            if n.starts_with("attach_params") {
                let blk = attach_block(b);
                if !blk.is_empty() {
                    blocks.set(n, blk);
                }
            }
        }
        if !blocks.is_empty() {
            e.set("blocks", blocks);
        }
        module_attach.set(k, e);
    }
    let accepting = Dict::from_pairs(weapons.entries().map(|(k, v)| {
        let ids: Vec<Value> =
            as_list(v.get("slots")).iter().filter(|s| s["id"].truthy() && get_or(s, "accept_module", true).truthy()).map(|s| s["id"].clone()).collect();
        (k, Value::List(ids))
    }));
    let mao = g["modules_drop_options"]["modules_additional_infos"].items();
    let detach: Vec<Value> = mao.iter().filter(|r| r["detach_price_in_gold"].truthy()).map(|r| client_files::list![&r["dict_id"], &r["detach_price_in_gold"]]).collect();
    let module_mods: Vec<Value> = mao
        .iter()
        .map(|r| client_files::list![&r["dict_id"], list_or_empty(&r["premium_modifiers"]), list_or_empty(&r["override_modifiers"])])
        .collect();
    let col = |table: &str, f: &dyn Fn(&Value) -> Value| -> Value { Value::List(g[table].items().iter().map(f).collect()) };
    let tables = dict! {
        "items" => items,
        "mod_groups" => Dict::from_pairs(groups),
        "weapon_slots" => weapon_slots,
        "compatible_ammo" => compat,
        "module_links" => col("module_to_item_links", &|l| client_files::list![&l["item"], &l["module"]]),
        "xp_table" => col("character_leveling", &|c| c["experience_points"].clone()),
        "level_rewards" => col("character_leveling", &|c| client_files::list![&c["reward_amount"], &c["reward_currency"]]),
        "item_categories" => col("item_categories", &|c| client_files::list![&c["category_id"], &c["is_stackable"], &c["minimum_count"]]),
        "slot_categories" => need(T, g, "profile_slots_restrictions")?,
        "exchange_currency_rates" => need(T, g, "exchange_currency_rates")?,
        "buy_specific_cost" => need(T, g, "buy_specific_cost")?,
        "buy_preset_cost" => need(T, g, "buy_preset_cost")?,
        "match_levels_factors" => need(T, g, "match_levels_factors")?,
        "weapon_default_modules" => weapon_default_modules,
        "module_attach" => module_attach,
        "weapon_slots_accepting" => accepting,
        "module_detach_price" => detach,
        "module_mods" => module_mods,
    };
    Ok(json(&tables, &Style::compact().utf8(), true))
}

/// skins_069d.json: skin item -> the weapons whose skin it unlocks (`skins_to_unlock`).
pub fn skins(cx: &Cx) -> Result<Vec<u8>> {
    let g = cx.game()?;
    let mut skins: Vec<(i64, Value)> = Vec::new();
    for (_, w) in g["weapons"].entries() {
        let p = &w["parameters"];
        let s = &p["skins_to_unlock"];
        if s.truthy() {
            let did = p["dict_id"].as_i64().ok_or_else(|| data_err("skins_069d.json", "dict_id"))?;
            let v = s.clone();
            match skins.iter_mut().find(|(d, _)| *d == did) {
                Some(slot) => slot.1 = v,
                None => skins.push((did, v)),
            }
        }
    }
    skins.sort_by_key(|(d, _)| *d);
    let d = Dict::from_pairs(skins.into_iter().map(|(k, v)| (k.to_string(), v)));
    Ok(json(&Value::Dict(d), &Style::compact(), true))
}

/// weapon_hands_069d.json: the weapons with `double_handed` false.
pub fn weapon_hands(cx: &Cx) -> Result<Vec<u8>> {
    let g = cx.game()?;
    let mut one: Vec<i64> = g["weapons"]
        .entries()
        .filter(|(_, w)| w["parameters"].get("dict_id").is_some() && w["parameters"].get("double_handed") == Some(&Value::Bool(false)))
        .filter_map(|(_, w)| w["parameters"]["dict_id"].as_i64())
        .collect();
    one.sort();
    Ok(json(&dict! { "one_handed" => one }, &Style::compact(), true))
}

/// decals_069d.json: camouflage / decal items, decal sets, the armour that takes them.
pub fn decals(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "decals_069d.json";
    let g = cx.game()?;
    let mut items = Dict::new();
    for (name, v) in g["camo_decal"].entries() {
        let p = need(T, v, "parameters")?;
        let mods: Vec<Value> = p["default_modifications"].items().iter().map(|ab| client_files::list![&ab[0], &ab[1]]).collect();
        items.set(
            pyjson::str(need(T, p, "dict_id")?),
            dict! {
                "name" => name,
                "category" => need(T, p, "category_id")?,
                "type" => need(T, p, "type")?,
                "cost" => need(T, p, "cost")?,
                "currency" => need(T, p, "currency")?,
                "camo_decal_type" => need(T, p, "camo_decal_type")?,
                "mods" => mods,
            },
        );
    }
    let mut sets = Dict::new();
    for (name, s) in g["camo_decal_sets"].entries() {
        let tiers: Vec<Value> = s["bonus_tiers"].items().iter().map(|t| dict! { "needed" => &t["items_needed"], "mods" => &t["modifiers"] }).collect();
        sets.set(name, dict! { "items" => need(T, s, "items")?, "tiers" => tiers });
    }
    let mut armour = Dict::new();
    for (_, v) in g["equipment"].entries() {
        let p = &v["parameters"];
        if p["camo_available"].truthy() || p["decal_available"].truthy() {
            armour.set(pyjson::str(&p["dict_id"]), dict! { "camo" => p["camo_available"].truthy(), "decal" => p["decal_available"].truthy() });
        }
    }
    Ok(json(&dict! { "items" => items, "sets" => sets, "armour" => armour }, &Style::compact().utf8().sorted(), true))
}

/// vouchers_069d.json (vouchers and boosters, categories 23 and 27, with their resolved modifiers) and
/// booster_costs_069d.json (`booster_parameters.options`).
pub fn vouchers(cx: &Cx) -> Result<(Vec<u8>, Vec<u8>)> {
    const T: &str = "vouchers_069d.json";
    let g = cx.game()?;
    let mut amm: Vec<(&str, &Value)> = g["ammo_grenades_etc"].entries().collect();
    amm.sort_by_key(|(_, v)| v["parameters"]["dict_id"].as_i64().unwrap_or(0));
    let mods = g["modifications"].items();
    let find_mod = |grp: &Value, grade: &Value| mods.iter().rev().find(|m| m["group"] == *grp && m["grade"] == *grade);
    let mut out = Vec::new();
    for (name, v) in amm {
        let p = &v["parameters"];
        let cat = p["category_id"].as_i64();
        if cat != Some(23) && cat != Some(27) {
            continue;
        }
        let mut r = Dict::new();
        r.set("dict_id", &p["dict_id"]);
        r.set("cfg_name", name);
        r.set("category", &p["category_id"]);
        r.set("type", get_or(p, "type", ""));
        for k in ["voucher_type", "ttl_in_hours", "service_id", "service_duration_matches", "default_modifications"] {
            if let Some(x) = p.get(k) {
                r.set(k, x);
            }
        }
        r.set("cost", need(T, p, "cost")?);
        r.set("currency", need(T, p, "currency")?);
        r.set("use_variable_costs", p["use_variable_costs"].truthy());
        let mut mm = Vec::new();
        for gg in p["default_modifications"].items() {
            let m = find_mod(&gg[0], &gg[1]).ok_or_else(|| data_err(T, format!("{name}: no modification {}", pyjson::repr(gg))))?;
            for e in m["modifiers"].items() {
                let path = e["path"].items();
                mm.push(dict! {
                    "name" => path.last().cloned().unwrap_or(Value::Null),
                    "value" => Value::Float(pyjson::round(e["value"].as_f64().unwrap_or(0.0), 4)),
                    "type" => &e["type"],
                });
            }
        }
        if !mm.is_empty() {
            r.set("modifiers", mm);
        }
        out.push(pyjson::dumps(&Value::Dict(r), &Style::indent(0)));
    }
    let vouchers = format!("[\n{}\n]\n", out.join(",\n")).into_bytes();
    let b = cx.cfg("gameplay/items/boosters/booster_parameters.options")?;
    let src = b.get("parameters").unwrap_or(&b);
    let costs = dict! {
        "booster_use_costs" => need("booster_costs_069d.json", src, "booster_use_costs")?,
        "equipment_use_costs" => need("booster_costs_069d.json", src, "equipment_use_costs")?,
    };
    Ok((vouchers, json(&costs, &Style::indent(0), false)))
}
