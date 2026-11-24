//! Maps, modes, bots and kits: bot match rows, the tutorial, mode timing, the gun race and PvE.

use client_files::pe::Image;
use client_files::pyjson::{self, Style};
use client_files::{dict, list, Dict, Value};

use crate::util::{get_or, json, need, Cx};
use crate::{data_err, Result};

const MODES_DIR: &str = "gameplay/game_modes/";
const AI_PROFILES: &str = "gameplay/players/ai.profiles";
const PVE_MODE: &str = "gameplay/game_modes/pve_01_v1_maze1.options";

/// The client's modifier name table (modifier id -> name): 138 pointers at this address of the 0.69d exe.
const MOD_NAMES_VA: u64 = 0x2_0099_5E10;
const MOD_NAMES_COUNT: usize = 138;

/// The mode files: (project = file name without `.options`, decoded config), sorted by project.
fn mode_files(cx: &Cx) -> Result<Vec<(String, Value)>> {
    let mut out = Vec::new();
    for p in cx.src.game.paths() {
        let Some(rest) = p.strip_prefix(MODES_DIR) else { continue };
        let Some(name) = rest.strip_suffix(".options") else { continue };
        let project = name.rsplit('/').next().unwrap_or(name).to_string();
        out.push((project, cx.cfg(p)?));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Level projects the client ships: `projects/<level>/client_project` of every volume.
fn shipped(cx: &Cx) -> std::collections::HashSet<String> {
    cx.src
        .game
        .paths()
        .into_iter()
        .filter_map(|p| p.strip_prefix("projects/").and_then(|p| p.strip_suffix("/client_project")).map(str::to_string))
        .collect()
}

/// A matchmaking row `[map, mode, variation, weather, project, level path, min players, max players]`.
type Row = (i64, i64, i64, i64, String, String, i64, i64);

fn row_of(project: &str, mode: &Value) -> Option<Row> {
    let mm = &mode["match_making_params"];
    Some((
        mm["map_id"].as_i64()?,
        mode["mode"].as_i64()?,
        mm["map_variation_id"].as_i64()?,
        mm["map_weather_id"].as_i64()?,
        project.to_string(),
        mm["map_name"].as_str()?.to_string(),
        get_or(mm, "min_players", 0).as_i64()?,
        get_or(mm, "max_players", 0).as_i64()?,
    ))
}

fn row_value(r: &Row) -> Value {
    list![r.0, r.1, r.2, r.3, r.4.as_str(), r.5.as_str(), r.6, r.7]
}

/// bot_match_069d.json: the matchmaking rows whose level ships (the training modes 0 1 2 6 9 of the mode files, then
/// the Search and Destroy layouts the client's custom-match list offers), the AI player roster
/// (`ai_players.options`), the skill tree's modifiers per level, and the client's modifier names.
pub fn bot_match(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "bot_match_069d.json";
    let shipped = shipped(cx);
    let files = mode_files(cx)?;
    let mut rows: Vec<Row> = files
        .iter()
        .filter(|(_, m)| matches!(m["mode"]["mode"].as_i64(), Some(0 | 1 | 2 | 6 | 9)))
        .filter_map(|(p, m)| row_of(p, &m["mode"]))
        .collect();
    rows.sort();
    let mut out: Vec<Value> = rows.iter().filter(|r| shipped.contains(&r.5)).map(row_value).collect();
    // Search and Destroy (mode 5): the (map, variation) pairs of `custom_match_data` for mode 5, projects that ship (a
    // level named without a folder ships as `<name>/default`)
    let sp = cx.cfg("gameplay/match_making/singleplayer_match_maps.options")?;
    let mut offered = Vec::new();
    for top in sp["custom_match_data"]["maps"].items() {
        for gm in top["game_modes"].items().iter().filter(|gm| gm["id"].as_i64() == Some(5)) {
            for mm in gm["maps"].items() {
                for w in mm["weather"].items() {
                    offered.push((mm["id"].clone(), w["id"].clone()));
                }
            }
        }
    }
    let mut sad: Vec<Row> = Vec::new();
    for (project, m) in &files {
        let mode = &m["mode"];
        let mm = &mode["match_making_params"];
        if mode["mode"].as_i64() != Some(5) || !offered.contains(&(mm["map_id"].clone(), mm["map_variation_id"].clone())) {
            continue;
        }
        let level = mm["map_name"].as_str().ok_or_else(|| data_err(T, format!("{project}: map_name")))?;
        let folder = if level.contains('/') { level.to_string() } else { format!("{level}/default") };
        if !shipped.contains(&folder) {
            continue;
        }
        sad.push(row_of(project, mode).ok_or_else(|| data_err(T, format!("{project}: matchmaking row")))?);
    }
    sad.sort();
    out.extend(sad.iter().map(row_value));
    let ai = cx.cfg("gameplay/players/ai_players.options")?;
    let tree = cx.cfg("gameplay/skills/skills_tree.options")?;
    let mut skill_tree = Dict::new();
    for s in need(T, &tree, "skills_tree")?.items() {
        let levels: Vec<Value> = s["modifiers"].items().iter().map(|lvl| Value::List(lvl.entries().map(|(n, v)| list![n, v]).collect())).collect();
        skill_tree.set(pyjson::str(&s["id"]), levels);
    }
    let img = Image::parse(&cx.src.exe)?;
    let mod_names = img.string_table(MOD_NAMES_VA, MOD_NAMES_COUNT)?;
    let doc = dict! {
        "mode_rows" => out,
        "ai_players" => need(T, &ai, "players")?,
        "skill_tree" => skill_tree,
        "mod_names" => mod_names,
    };
    Ok(json(&doc, &Style::compact().utf8(), true))
}

/// tutorial_069d.json: the human's four kits `tutorial_player_0..3` and the tutorial script's bots (name, team, head,
/// kit, AI block), kits from `ai.profiles`.
pub fn tutorial(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "tutorial_069d.json";
    let tut = cx.cfg("gameplay/game_modes/tutorial.options")?;
    let kits_src = cx.cfg(AI_PROFILES)?;
    let kits = need(T, &kits_src, "profiles")?;
    let kit = |n: &str| kits.get(n).cloned().ok_or_else(|| data_err(T, format!("no kit {n}")));
    let human = (0..4).map(|i| kit(&format!("tutorial_player_{i}"))).collect::<Result<Vec<_>>>()?;
    let mut bots = Vec::new();
    for b in tut["mode"]["scripting"]["profiles"].items() {
        bots.push(dict! {
            "name" => &b["name"],
            "team" => &b["team"],
            "head" => &b["head"],
            "kit" => kit(b["profile"].as_str().unwrap_or(""))?,
            "ai" => &b["ai_params"],
        });
    }
    Ok(json(&dict! { "human" => human, "bots" => bots }, &Style::compact().utf8(), true))
}

/// game_modes_069d.json: the timing block (`mode.parameters`) of every mode file: countdown, match and advantage
/// time, wait time and player percentages (absent = null), floats rounded to 6 places.
pub fn game_modes(cx: &Cx) -> Result<Vec<u8>> {
    const KEYS: [&str; 6] = ["countdown_time", "match_time", "advantage_time", "wait_time", "wait_player_percent", "leave_player_percent"];
    let mut rows = Dict::new();
    for (project, d) in mode_files(cx)? {
        let p = &d["mode"]["parameters"];
        let p = if p.truthy() { p } else { &Value::Null };
        if !p.is_null() && p.as_dict().is_none() {
            continue;
        }
        let row = Dict::from_pairs(KEYS.iter().map(|k| {
            let v = match p.get(k) {
                Some(Value::Float(f)) => Value::Float(pyjson::round(*f, 6)),
                Some(v) => v.clone(),
                None => Value::Null,
            };
            (*k, v)
        }));
        rows.set(project, row);
    }
    Ok(json(&dict! { "game_modes" => rows }, &Style::indent(1).sorted(), true))
}

/// gun race kits: race level 7 is replaced by level 8 (see [`gunrace`]).
const SNOWBALL_LEVEL: usize = 7;
const ICICLE_LEVEL: usize = 8;

/// gunrace_069d.json: Arms Race (mode 3, `arena_01.options`): its matchmaking row, score, rule ids, script and the race
/// kits `[equipment level][race level][variants]` with their `ai.profiles` entries.
///
/// CHOICE: race level 7 carries the tier's icicle kits (level 8's list) instead of the snowball kits: the snowball's
/// grenade set (`snowball_grenade.options`) is not in the 0.69d client's resources and a record with a snowball kit
/// makes the exe fault while it builds the match. `level7_replaced` keeps the data's names and the reason.
pub fn gunrace(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "gunrace_069d.json";
    let src = cx.cfg("gameplay/game_modes/arena_01.options")?;
    let mode = need(T, &src, "mode")?;
    let kits_src = cx.cfg(AI_PROFILES)?;
    let kits = need(T, &kits_src, "profiles")?;
    let mm = &mode["match_making_params"];
    if mode["mode"].as_i64() != Some(3) || mm["map_id"].as_i64() != Some(13) {
        return Err(data_err(T, "arena_01 is not the gun race file"));
    }
    let name = mm["map_name"].as_str().ok_or_else(|| data_err(T, "map_name"))?;
    let level = if name.contains('/') { name.to_string() } else { format!("{name}/default") };
    let row = list![&mm["map_id"], &mode["mode"], &mm["map_variation_id"], &mm["map_weather_id"], name, level, &mm["min_players"], &mm["max_players"]];
    let mut tiers = Vec::new();
    let mut replaced = Vec::new();
    for tier in mode["scripting"]["player_profiles"].items() {
        let t = tier.items();
        let names_with = |g: &Value, s: &str| g.items().iter().all(|n| n.as_str().is_some_and(|n| n.contains(s)));
        if t.len() != 9 || !names_with(&t[SNOWBALL_LEVEL], "snowball") || !names_with(&t[ICICLE_LEVEL], "icicle") {
            return Err(data_err(T, "a tier is not nine race levels with snowball level 7 and icicle level 8"));
        }
        replaced.push(t[SNOWBALL_LEVEL].clone());
        let mut groups: Vec<&Value> = t.iter().collect();
        groups[SNOWBALL_LEVEL] = &t[ICICLE_LEVEL];
        let mut tv = Vec::new();
        for g in groups {
            let mut gv = Vec::new();
            for n in g.items() {
                let k = n.as_str().unwrap_or("");
                gv.push(dict! { "name" => n, "kit" => kits.get(k).cloned().ok_or_else(|| data_err(T, format!("no kit {k}")))? });
            }
            tv.push(Value::List(gv));
        }
        tiers.push(Value::List(tv));
    }
    if tiers.len() != 5 {
        return Err(data_err(T, format!("{} equipment tiers, expected 5", tiers.len())));
    }
    let res = dict! {
        "row" => row,
        "score" => &mode["parameters"]["score"],
        "rules" => need(T, mode, "rules")?,
        "script" => &mode["scripting"]["script"],
        "tiers" => tiers,
        "level7_replaced" => dict! {
            "by" => "icicle (level 8's kits)",
            "data" => replaced,
            "why" => "snowball grenade_set snowball_grenade.options is not shipped; the exe faults building the match",
        },
    };
    Ok(json(&res, &Style::compact().utf8().sorted(), true))
}

/// pve_069d.json: missions (`pve_missions.options`), the mission's mode file (the six `pve_01_v*_maze*` files differ
/// only in `map_name`), the bots per difficulty, prices, reroll info, tickets, chest items, module drops and a summary
/// of the mission script (`gameplay/scripting/pve.lua`, a binary config).
pub fn pve(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "pve_069d.json";
    let missions = cx.cfg("gameplay/pve_missions.options")?;
    let src = cx.cfg(PVE_MODE)?;
    let mode = need(T, &src, "mode")?;
    let script = cx.cfg("gameplay/scripting/pve.lua")?;
    let game = cx.game()?;
    let mut prof = Dict::new();
    for (d, groups) in mode["scripting"]["pve_profiles"].items().iter().enumerate() {
        let g = groups.items();
        if g.iter().any(|x| *x != g[0]) {
            return Err(data_err(T, "PvE equipment groups differ"));
        }
        let bots: Vec<Value> = g[0]
            .items()
            .iter()
            .map(|b| {
                dict! {
                    "name" => &b["name"],
                    "profile" => &b["profile"],
                    "kit" => &b["profile1"],
                    "head" => &b["head"],
                    "hit_chance" => &b["ai_params"]["hit_chance"],
                    "hit_damage_multiplier" => &b["ai_params"]["hit_damage_multiplier"],
                }
            })
            .collect();
        prof.set(d.to_string(), bots);
    }
    let amm = &game["ammo_grenades_etc"];
    let mut chests = Dict::new();
    for (k, v) in amm.entries() {
        if k.contains("pve_mission") {
            let p = &v["parameters"];
            chests.set(pyjson::str(&p["dict_id"]), dict! { "name" => k.trim_start_matches('/'), "drop_type" => get_or(p, "drop_type", Value::Null) });
        }
    }
    let mut vouchers = Dict::new();
    for k in ["pve_ticket_voucher", "pve_ticket_voucher_free"] {
        let p = &amm[k]["parameters"];
        vouchers.set(pyjson::str(need(T, p, "dict_id")?), dict! { "name" => k, "currency" => &p["currency"], "cost" => &p["cost"], "category_id" => &p["category_id"] });
    }
    let procs: Vec<Value> = script["actions_processors"]
        .items()
        .iter()
        .map(|a| {
            dict! {
                "id" => &a["id"],
                "event" => get_or(a, "triggered_by_event", Value::Null),
                "enabled" => &a["is_enabled"],
                "actions" => a["actions"].items().len().max(a["actions"].as_dict().map_or(0, |d| d.len())),
            }
        })
        .collect();
    let projects: Vec<Value> = [1, 2, 3].iter().flat_map(|v| [1, 2].map(|m| Value::from(format!("pve_01_v{v}_maze{m}")))).collect();
    let mdo = &game["modules_drop_options"];
    let count = |v: &Value| v.items().len().max(v.as_dict().map_or(0, |d| d.len()));
    let res = dict! {
        "missions" => need(T, &missions, "pve_missions")?,
        "chests_premium_bonus_coef" => need(T, &missions, "chests_premium_bonus_coef")?,
        "mode" => dict! {
            "mode" => &mode["mode"],
            "match_making_params" => &mode["match_making_params"],
            "parameters" => &mode["parameters"],
            "rules" => &mode["rules"],
            "scripting_script" => &mode["scripting"]["script"],
            "anomaly_maze" => &mode["anomaly_maze"],
            "projects" => projects,
        },
        "pve_profiles" => prof,
        "prices" => need(T, game, "pve_default_prices")?,
        "reward_reroll_info" => need(T, game, "pve_reward_reroll_info")?,
        "vouchers" => vouchers,
        "chest_items" => chests,
        "modules_drop" => dict! {
            "used_weapon_pve_dropping_modules_percent" => &mdo["used_weapon_pve_dropping_modules_percent"],
            "modules_mods_pair_pve_prob" => &mdo["modules_mods_pair_pve_prob"],
        },
        "script" => dict! {
            "engine_string_constants" => &script["engine_string_constants"],
            "required_game_effects" => &script["required_game_effects"],
            "processors" => procs,
            "lootbox_count" => count(&script["lootboxes"]),
            "bots_spawn_sawmill_forward" => count(&script["bots_spawn_sawmill_forward"]),
            "bots_spawn_sawmill_backward" => count(&script["bots_spawn_sawmill_backward"]),
        },
    };
    Ok(json(&res, &Style::indent(1).sorted(), true))
}

/// pve_bots_069d.json: the 17 script bots of the PvE mission per difficulty: record name, team, head, both kits (with
/// their `ai.profiles` slots), the kit names and the whole AI block.
pub fn pve_bots(cx: &Cx) -> Result<Vec<u8>> {
    const T: &str = "pve_bots_069d.json";
    let src = cx.cfg(PVE_MODE)?;
    let kits_src = cx.cfg(AI_PROFILES)?;
    let kits = need(T, &kits_src, "profiles")?;
    let kit = |n: &Value| kits.get(n.as_str().unwrap_or("")).cloned().ok_or_else(|| data_err(T, format!("no kit {}", pyjson::repr(n))));
    let mut res = Dict::new();
    for (d, groups) in src["mode"]["scripting"]["pve_profiles"].items().iter().enumerate() {
        let g = groups.items();
        if g.iter().any(|x| *x != g[0]) {
            return Err(data_err(T, "PvE equipment groups differ"));
        }
        let mut bots = Vec::new();
        for (i, b) in g[0].items().iter().enumerate() {
            if b["name"].as_str() != Some(&format!("pve_bot_{}", i + 1)) {
                return Err(data_err(T, format!("bot {i} is not pve_bot_{}", i + 1)));
            }
            bots.push(dict! {
                "name" => &b["name"],
                "team" => &b["team"],
                "head" => &b["head"],
                "profile" => kit(&b["profile"])?,
                "profile1" => kit(&b["profile1"])?,
                "kit_names" => list![&b["profile"], &b["profile1"]],
                "ai" => &b["ai_params"],
            });
        }
        if bots.len() != 17 {
            return Err(data_err(T, format!("{} bots, expected 17", bots.len())));
        }
        res.set(d.to_string(), bots);
    }
    Ok(json(&Value::Dict(res), &Style::compact().utf8().sorted(), true))
}
