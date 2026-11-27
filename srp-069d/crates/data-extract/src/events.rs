//! Event currency drops, hero of the day, the event exchange, ranked leagues, progression.

use client_files::pyjson::{self, Style};
use client_files::{dict, list, Dict, Value};

use crate::drops::py_int;
use crate::items::ItemRow;
use crate::util::{get_or, json, need, round, Cx};
use crate::{data_err, Result};

const EVENT_RULES: &str = "gameplay/event_rules/event_exchange_action_items.options";

/// event_drops_069d.json: the `drop_after_combat` rows of the three event currencies (951, 1138, 1139; service 23) and
/// the worn-item bonuses (`put_on_bonus_items`) and win item of the event exchange rules.
pub fn event_drops(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "event_drops_069d.json";
    const CUR: [i64; 3] = [951, 1138, 1139];
    let src = cx.cfg("gameplay/drops/drops.options")?;
    let drops = need(T, &src, "drop_after_combat")?;
    let ex = cx.cfg(EVENT_RULES)?;
    let mut rows = Vec::new();
    for it in drops["items"].items() {
        let Some(did) = it.get("dict_id").and_then(Value::as_i64) else { continue };
        if !CUR.contains(&did) {
            continue;
        }
        let place = it["place_in_match"].items();
        if it["service_needed"].as_i64() != Some(23) || place.len() < 2 || place[0] != place[1] {
            return Err(data_err(T, format!("event currency row {}", pyjson::repr(it))));
        }
        rows.push(dict! {
            "mode_mask" => &it["game_mode_mask"],
            "currency" => &it["dict_id"],
            "place" => &place[0],
            "amount" => &it["amount"],
            "win_needed" => it["win_needed"].truthy(),
            "loss_needed" => it["loss_needed"].truthy(),
            "premium_factor" => round(&it["premium_factor"], 6),
        });
    }
    let bonus: Vec<Value> = ex["put_on_bonus_items"]
        .items()
        .iter()
        .map(|b| {
            let vals: Vec<Value> = b["bonus_values"].items().iter().map(|v| dict! { "currency" => &v["currency_id"], "amount" => &v["bonus_amount"] }).collect();
            dict! { "dict_id" => &b["dict_id"], "bonus" => vals }
        })
        .collect();
    let out = dict! {
        "service_needed" => 23,
        "currencies" => CUR.to_vec(),
        "rows" => rows,
        "put_on_bonus_items" => bonus,
        "item_to_drop_after_win" => need(T, &ex, "item_to_drop_after_win")?,
    };
    Ok(json(&out, &Style::indent(1), false))
}

/// hero_of_the_day_069d.json (competitive groups with the bonus fractions per place, resolved from `modifications` by
/// (group, grade); the periods) and event_exchange_069d.json (`exchange_items` as they are).
pub fn hero(cx: &Cx) -> Result<(Vec<u8>, Vec<u8>)> {
    const T: &str = "hero_of_the_day_069d.json";
    let src = cx.cfg("gameplay/event_rules/hero_of_the_day.options")?;
    let hero = need(T, &src, "parameters")?;
    let ex = cx.cfg(EVENT_RULES)?;
    let mods = cx.game()?["modifications"].items();
    let names = [("experience_bonus_modifier", "experience"), ("money_bonus_modifier", "silver"), ("reputation_bonus_modifier", "reputation")];
    let mut groups = Vec::new();
    for g in hero["competitive_groups"].items() {
        let mut bs: Vec<&Value> = g["bonuses"].items().iter().collect();
        bs.sort_by_key(|b| b["priority"].as_i64().unwrap_or(0));
        let mut bonuses = Vec::new();
        for b in bs {
            let mut row = Dict::new();
            row.set("place", &b["priority"]);
            row.set("experience", 0.0);
            row.set("silver", 0.0);
            row.set("reputation", 0.0);
            row.set("exp_group", 0);
            row.set("grade", 0);
            for pair in b["modifiers"].items() {
                let (grp, grade) = (&pair[0], &pair[1]);
                let m = mods
                    .iter()
                    .rev()
                    .find(|m| m["group"] == *grp && m["grade"] == *grade)
                    .ok_or_else(|| data_err(T, format!("no modification {}", pyjson::repr(pair))))?;
                for (k, v) in m["lobby_info"].entries() {
                    let n = names.iter().find(|(a, _)| *a == k).map(|(_, n)| *n).ok_or_else(|| data_err(T, format!("bonus modifier {k}")))?;
                    row.set(n, Value::Float(pyjson::f32_shortest(v.as_f64().unwrap_or(0.0))));
                    if k == "experience_bonus_modifier" {
                        row.set("exp_group", grp);
                        row.set("grade", grade);
                    }
                }
            }
            bonuses.push(Value::Dict(row));
        }
        groups.push(dict! {
            "min_level" => &g["min_level"],
            "max_level" => &g["max_level"],
            "max_player_level" => &g["max_player_level"],
            "bonuses" => bonuses,
        });
    }
    let doc = dict! {
        "competitive_groups" => groups,
        "minutes_recalc_period" => need(T, hero, "minutes_recalc_period")?,
        "minutes_operating_period" => need(T, hero, "minutes_operating_period")?,
    };
    let exchange = dict! { "exchange_items" => need("event_exchange_069d.json", &ex, "exchange_items")? };
    Ok((json(&doc, &Style::indent(1), true), json(&exchange, &Style::indent(1), true)))
}

/// Items of the league shop: league-themed weapons and armour (CHOICE rules below).
const LEAGUE_ITEM_MARKS: [&str; 2] = ["league", "legend"];
const LEAGUE_SKIP: [&str; 1] = ["league_currency"];
/// PROVEN price (official post 2020-06-03: the Saiga-12K "Legend" for 2250 league tokens).
const LEAGUE_PROVEN: [(&str, i64); 1] = [("saiga_12_spring_league_2020", 2250)];
/// CHOICE: every other league item costs 450 tokens per item level (the Saiga is level 5).
const TOKENS_PER_LEVEL: i64 = 450;

/// league_069d.json: `leagues_parameters` of `gameplay/event_rules/leagues.options`, `rating_match_bonus_percent` of
/// game.options, and the league shop.
pub fn league(cx: &Cx, items: &[ItemRow]) -> Result<Vec<u8>> {
    const T: &str = "league_069d.json";
    let lg = cx.cfg("gameplay/event_rules/leagues.options")?;
    let mut leagues = Vec::new();
    for p in need(T, &lg, "leagues_parameters")?.items() {
        leagues.push(dict! {
            "possible_level_down" => p["possible_level_down"].truthy(),
            "min_equip_level" => &p["min_equip_level"],
            "points_for_levelup" => &p["points_for_levelup"],
            "looser_team_points" => Value::List(p["looser_team_points"].items().iter().map(py_int).collect()),
            "winner_team_points" => Value::List(p["winner_team_points"].items().iter().map(py_int).collect()),
        });
    }
    let mut shop = Vec::new();
    for r in items {
        let name = r.cfg_name.as_str();
        if !matches!(r.section, "weapons" | "equipment") || LEAGUE_SKIP.contains(&name) {
            continue;
        }
        if !LEAGUE_ITEM_MARKS.iter().any(|m| name.contains(m)) || name.ends_with("_premium") {
            // the "Delta 2" premium variants stay gold items (CHOICE)
            continue;
        }
        let level: i64 = r.col("item_level").parse().map_err(|_| data_err(T, format!("{name}: item_level")))?;
        let (tokens, source) = match LEAGUE_PROVEN.iter().find(|(n, _)| *n == name) {
            Some((_, t)) => (*t, "PROVEN web 2020-06-03"),
            None => (TOKENS_PER_LEVEL * level, "CHOICE 450 x item_level"),
        };
        shop.push(dict! { "dict_id" => r.dict_id, "cfg_name" => name, "item_level" => level, "tokens" => tokens, "source" => source });
    }
    let doc = dict! {
        "leagues" => leagues,
        "rating_match_bonus_percent" => need(T, cx.game()?, "rating_match_bonus_percent")?,
        "shop" => shop,
    };
    Ok(json(&doc, &Style::indent(1), true))
}

/// progression_069d.json: the new-player kit, faction reputation levels, match event rewards (base + per mode),
/// tutorial awards and the reward / spare-parts / faction reputation bonus modifiers of `modifications`.
pub fn progression(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "progression_069d.json";
    let kit = cx.cfg("gameplay/items/items_for_new_player.options")?;
    let new_player: Vec<Value> = need(T, &kit, "items")?
        .items()
        .iter()
        .map(|r| {
            dict! {
                "dict_id" => &r["dict_id"],
                "buy_amount" => get_or(r, "buy_amount", 1),
                "wear_amount" => get_or(r, "wear_amount", 0),
                "slot" => get_or(r, "slot", Value::Null),
                "faction_id" => get_or(r, "faction_id", Value::Null),
                "profiles_to_wear" => get_or(r, "profiles_to_wear", Value::List(vec![])),
            }
        })
        .collect();
    let rep = cx.cfg("gameplay/reputation_leveling.options")?;
    let mut faction_levels = Dict::new();
    for (_, v) in need(T, &rep, "factions_dict")?.entries() {
        faction_levels.set(pyjson::str(&v["id"]), Value::List(v["levels"].items().iter().map(|l| l["value"].clone()).collect()));
    }
    const FIELDS: [&str; 5] = ["score", "experience", "money", "reputation", "reward_factor"];
    let pick = |x: &Value| Value::Dict(Dict::from_pairs(FIELDS.iter().filter_map(|k| x.get(k).map(|v| (*k, v.clone())))));
    let ev = cx.cfg("gameplay/awards/events.options")?;
    let mut events = Dict::new();
    for (name, e) in need(T, &ev, "events")?.entries() {
        let modes = Dict::from_pairs(e.entries().filter(|(_, x)| x.as_dict().is_some()).map(|(m, x)| (m, pick(x))));
        events.set(name, dict! { "base" => pick(e), "modes" => modes });
    }
    let ua = cx.cfg("gameplay/awards/unique_awards.options")?;
    let mut awards = Dict::new();
    for (name, rows) in need(T, &ua, "awards")?.entries() {
        let v: Vec<Value> = match rows {
            Value::List(l) => l.iter().map(|r| dict! { "amount" => &r["amount"], "param" => &r["param"], "type" => &r["type"] }).collect(),
            _ => vec![],
        };
        awards.set(name, v);
    }
    let mods = cx.game()?["modifications"].items();
    let names = ["experience_bonus_modifier", "money_bonus_modifier", "reputation_bonus_modifier"];
    let mut reward = Vec::new();
    for m in mods {
        let li = &m["lobby_info"];
        for (i, n) in names.iter().enumerate() {
            if let Some(v) = li.get(n) {
                reward.push(list![&m["group"], &m["grade"], i, round(v, 6)]);
            }
        }
    }
    let mut parts = Vec::new();
    for m in mods {
        if let Some(v) = m["lobby_info"].get("details_bonus_modifier") {
            parts.push(list![&m["group"], &m["grade"], round(v, 6)]);
        }
    }
    // faction reputation bonus (groups 125..128): per-faction lobby_info names; faction ids of reputation_leveling
    let faction_rep = [("scav_rep_booster", 1), ("bm_rep_booster", 2), ("ren_rep_booster", 3), ("edge_rep_booster", 4)];
    let mut frep = Vec::new();
    for m in mods {
        for (k, fid) in faction_rep {
            if let Some(v) = m["lobby_info"].get(k) {
                frep.push(list![&m["group"], &m["grade"], fid, round(v, 6)]);
            }
        }
    }
    let doc = dict! {
        "new_player" => new_player,
        "faction_levels" => faction_levels,
        "events" => events,
        "tutorial_awards" => awards,
        "reward_modifiers" => reward,
        "parts_modifiers" => parts,
        "faction_reputation_modifiers" => frep,
    };
    Ok(json(&doc, &Style::compact().utf8().sorted(), true))
}
