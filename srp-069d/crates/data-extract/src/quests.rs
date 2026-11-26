//! Achievements, badges, daily quests, the skill tree.

use client_files::pyjson::{self, Style};
use client_files::{dict, list, Dict, Value};

use crate::items::ItemRow;
use crate::util::{get_or, int, json, need, Cx};
use crate::{data_err, Result};

/// achievements_069d.json: `gameplay/quests/achievements.options` (id, category, type, apply type, preconditions,
/// states [condition, score, reward type, reward id, reward amount]) and the categories.
pub fn achievements(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "achievements_069d.json";
    let d = cx.cfg("gameplay/quests/achievements.options")?;
    let mut ach = Vec::new();
    for a in need(T, &d, "achievements")?.items() {
        let states: Vec<Value> = a["states"]
            .items()
            .iter()
            .map(|s| list![&s["condition"], &s["score"], &s["reward_type"], &s["reward_id"], &s["reward_num"]])
            .collect();
        ach.push(dict! {
            "id" => need(T, a, "id")?,
            "category" => need(T, a, "category")?,
            "type" => need(T, a, "type")?,
            "apply" => need(T, a, "apply_type")?,
            "pre" => need(T, a, "preconditions")?,
            "states" => states,
        });
    }
    let cats: Vec<Value> = d["categories"].items().iter().map(|c| list![&c["id"], &c["score"]]).collect();
    Ok(json(&dict! { "achievements" => ach, "categories" => cats }, &Style::compact().utf8(), true))
}

/// badges_069d.json: `gameplay/quests/badges.options`, the badge types in file order (index = type) with their states
/// `[condition, silver, experience, reputation]` (award type 2 = silver, 0 = experience, 1 = reputation).
pub fn badges(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "badges_069d.json";
    let d = cx.cfg("gameplay/quests/badges.options")?;
    let mut rows = Vec::new();
    for (i, b) in need(T, &d, "badges")?.items().iter().enumerate() {
        if int(T, b, "type")? != i as i64 {
            return Err(data_err(T, format!("badge {i} has type {}", pyjson::repr(&b["type"]))));
        }
        let mut states = Vec::new();
        for s in b["states"].items() {
            let mut am = [0i64; 3];
            for a in s["awards"].items() {
                let t = int(T, a, "type")?;
                if !(0..=2).contains(&t) || (t == 2 && get_or(a, "param", 0).as_i64() != Some(0)) {
                    return Err(data_err(T, format!("badge {i}: award {}", pyjson::repr(a))));
                }
                am[t as usize] += int(T, a, "amount")?;
            }
            states.push(list![&s["condition"], am[2], am[0], am[1]]);
        }
        rows.push(dict! { "type" => &b["type"], "states" => states });
    }
    Ok(json(&dict! { "badges" => rows }, &Style::compact().utf8(), true))
}

/// daily_quests_069d.json: `gameplay/quests/daily_quests.options` (quest pool, rewards by level, lifetime, rerolls)
/// and the drop table of each daily quest chest (`daily_quest_chest_N` -> `daily_quest_chest_N_drop` of
/// `gameplay/drops/chest_drops.options`, the base volume's copy).
pub fn daily_quests(cx: &Cx, items: &[ItemRow]) -> Result<Vec<u8>> {
    const T: &str = "daily_quests_069d.json";
    let dq = cx.cfg("gameplay/quests/daily_quests.options")?;
    let chests_src = cx.cfg_in(0, "gameplay/drops/chest_drops.options")?;
    let p = need(T, &dq, "parameters")?;
    let mut rewards = Vec::new();
    for r in p["rewards"].items() {
        let by_type = |t: i64| -> Result<Value> {
            r["quest_completed"]
                .items()
                .iter()
                .rev()
                .find(|q| q["type"].as_i64() == Some(t))
                .map(|q| q["amount"].clone())
                .ok_or_else(|| data_err(T, format!("reward row without type {t}")))
        };
        rewards.push(dict! {
            "min_level" => &r["min_level"],
            "max_level" => &r["max_level"],
            "all_completed" => &r["all_completed"],
            "exp" => by_type(0)?,
            "rep" => by_type(1)?,
            "silver" => by_type(2)?,
        });
    }
    let mut quests = Vec::new();
    for (i, q) in dq["quests"].items().iter().enumerate() {
        quests.push(dict! {
            "index" => i,
            "name" => need(T, q, "name")?,
            "type" => need(T, q, "type")?,
            "parameter" => need(T, q, "parameter")?,
            "value" => need(T, q, "value")?,
            "must_win" => need(T, q, "must_win_to_complete")?,
            "need_item_of_category" => get_or(q, "need_item_of_category", 255),
            "min_faction_level" => get_or(q, "min_faction_level", 0),
        });
    }
    // dict id -> chest name, ascending ids (a repeated id keeps the last row)
    let mut names: Vec<(i64, &str)> = Vec::new();
    for r in items.iter().filter(|r| r.cfg_name.starts_with("daily_quest_chest_")) {
        match names.iter_mut().find(|(d, _)| *d == r.dict_id) {
            Some(slot) => slot.1 = &r.cfg_name,
            None => names.push((r.dict_id, &r.cfg_name)),
        }
    }
    names.sort_by_key(|(d, _)| *d);
    let mut chests = Dict::new();
    for (item, name) in names {
        let drop = chests_src.get(&format!("{name}_drop")).ok_or_else(|| data_err(T, format!("no {name}_drop")))?;
        let mut rows = Vec::new();
        for e in drop["item_category_probabilities"].items() {
            let rar = e["rarity"].items();
            if rar.iter().any(|x| *x != rar[0]) {
                return Err(data_err(T, format!("rarity rows differ in {name}")));
            }
            rows.push(dict! { "weight" => &e["drop_weight"], "items" => &e["items_dict_ids"], "min" => &rar[0][0], "max" => &rar[0][1] });
        }
        chests.set(item.to_string(), rows);
    }
    let res = dict! {
        "lifetime_minutes" => need(T, p, "lifetime_in_minutes")?,
        "free_reroll_count" => need(T, p, "free_reroll_count")?,
        "reroll_premium_cost" => need(T, p, "reroll_premium_cost")?,
        "rewards" => rewards,
        "quests" => quests,
        "chests" => chests,
    };
    Ok(json(&res, &Style::compact().utf8(), true))
}

/// skills_069d.json: `gameplay/skills/skills_tree.options`, one row per skill {id, levels, branch_points} by id.
pub fn skills(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "skills_069d.json";
    let d = cx.cfg("gameplay/skills/skills_tree.options")?;
    let mut tree: Vec<&Value> = need(T, &d, "skills_tree")?.items().iter().collect();
    tree.sort_by_key(|s| s["id"].as_i64().unwrap_or(0));
    let rows: Vec<Value> = tree
        .into_iter()
        .map(|s| dict! { "id" => &s["id"], "levels" => &s["levels_amount"], "branch_points" => &s["preconditions"]["branch_points"] })
        .collect();
    Ok(json(&dict! { "skills" => rows }, &Style::compact(), true))
}
