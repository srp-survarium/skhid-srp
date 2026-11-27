//! data-extract: the server's 0.69d game tables, built from the operator's own game install.
//!
//! The server needs a few dozen tables of the client's configs (item dictionary, quests, drops, modes, kits, ...).
//! None of them is shipped: [`extract`] reads the game folder's `resources.NN.db` volumes (and a string table of
//! `survarium.exe`) with [`client_files`] and writes the tables the `game-data` crate loads. Our own figures that are not
//! in the client (shop prices from the fan wiki, CHOICE prices and rules) live in this crate's `ours/` files and in the
//! extractor code, and are merged in here.
//!
//! The output is deterministic: the same install gives the same bytes (Python's JSON layout, see
//! `client_files::pyjson`).

mod drops;
mod events;
mod items;
mod matches;
mod quests;
mod shop;
mod texts;
mod util;

use std::path::{Path, PathBuf};

use client_files::GameFiles;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Files(#[from] client_files::Error),
    #[error("{table}: {why}")]
    Data { table: &'static str, why: String },
    #[error("writing {path}: {source}")]
    Write { path: String, source: std::io::Error },
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn data_err(table: &'static str, why: impl Into<String>) -> Error {
    Error::Data { table, why: why.into() }
}

/// What the extraction reads: the game's volumes and the client executable.
pub struct Source {
    pub game: GameFiles,
    pub exe: Vec<u8>,
}

impl Source {
    /// `resources` = the folder holding `resources.NN.db`, `exe` = `survarium.exe`.
    pub fn open(resources: &Path, exe: &Path) -> Result<Source> {
        let game = GameFiles::open(resources)?;
        let exe = std::fs::read(exe).map_err(|source| client_files::Error::Io { path: exe.display().to_string(), source })?;
        Ok(Source { game, exe })
    }
}

/// One output file.
#[derive(Debug, Clone)]
pub struct Table {
    pub name: &'static str,
    pub bytes: Vec<u8>,
}

/// The file names [`extract`] writes, in order.
pub const FILES: &[&str] = &[
    "items_069d.tsv",
    "shop_069d.tsv",
    "game_tables_069d.json",
    "bot_match_069d.json",
    "bot_names_en.json",
    "tutorial_069d.json",
    "achievements_069d.json",
    "badges_069d.json",
    "daily_quests_069d.json",
    "skills_069d.json",
    "skins_069d.json",
    "weapon_hands_069d.json",
    "decals_069d.json",
    "module_drops_069d.json",
    "chests_069d.json",
    "drops_069d.json",
    "weekly_drop_069d.json",
    "faction_battle_069d.json",
    "event_drops_069d.json",
    "hero_of_the_day_069d.json",
    "event_exchange_069d.json",
    "league_069d.json",
    "progression_069d.json",
    "vouchers_069d.json",
    "booster_costs_069d.json",
    "game_modes_069d.json",
    "gunrace_069d.json",
    "pve_069d.json",
    "pve_bots_069d.json",
    "store_texts_069d.json",
    "event_categories_069d.json",
    "bot_names_069d.json",
];

/// Build every table; the first failure stops the run.
pub fn extract(src: &Source) -> Result<Vec<Table>> {
    extract_each(src).into_iter().map(|(name, r)| r.map(|bytes| Table { name, bytes })).collect()
}

/// Build every table, each on its own: one table's failure does not hide the others (`(file name, result)` in
/// [`FILES`] order).
pub fn extract_each(src: &Source) -> Vec<(&'static str, Result<Vec<u8>>)> {
    let cx = match util::Cx::new(src) {
        Ok(cx) => cx,
        Err(e) => return vec![("game files", Err(e))],
    };
    let items = items::rows(&cx);
    let it = |f: &dyn Fn(&[items::ItemRow]) -> Result<Vec<u8>>| match &items {
        Ok(rows) => f(rows),
        Err(e) => Err(data_err("items", format!("the item dictionary failed: {e}"))),
    };
    let pair = |r: Result<(Vec<u8>, Vec<u8>)>| match r {
        Ok((a, b)) => (Ok(a), Ok(b)),
        Err(e) => {
            let s = e.to_string();
            (Err(e), Err(data_err("pair", s)))
        }
    };
    let (weekly, faction) = pair(drops::hunt(&cx));
    let (hero, exchange) = pair(events::hero(&cx));
    let (vouchers, costs) = pair(items::vouchers(&cx));
    vec![
        ("items_069d.tsv", it(&|r| Ok(items::tsv(r)))),
        ("shop_069d.tsv", it(&|r| items::shop(&cx, r))),
        ("game_tables_069d.json", items::game_tables(&cx)),
        ("bot_match_069d.json", matches::bot_match(&cx)),
        ("bot_names_en.json", texts::bot_names_en(&cx)),
        ("tutorial_069d.json", matches::tutorial(&cx)),
        ("achievements_069d.json", quests::achievements(&cx)),
        ("badges_069d.json", quests::badges(&cx)),
        ("daily_quests_069d.json", it(&|r| quests::daily_quests(&cx, r))),
        ("skills_069d.json", quests::skills(&cx)),
        ("skins_069d.json", items::skins(&cx)),
        ("weapon_hands_069d.json", items::weapon_hands(&cx)),
        ("decals_069d.json", items::decals(&cx)),
        ("module_drops_069d.json", drops::module_drops(&cx)),
        ("chests_069d.json", drops::chests(&cx)),
        ("drops_069d.json", drops::drops(&cx)),
        ("weekly_drop_069d.json", weekly),
        ("faction_battle_069d.json", faction),
        ("event_drops_069d.json", events::event_drops(&cx)),
        ("hero_of_the_day_069d.json", hero),
        ("event_exchange_069d.json", exchange),
        ("league_069d.json", it(&|r| events::league(&cx, r))),
        ("progression_069d.json", events::progression(&cx)),
        ("vouchers_069d.json", vouchers),
        ("booster_costs_069d.json", costs),
        ("game_modes_069d.json", matches::game_modes(&cx)),
        ("gunrace_069d.json", matches::gunrace(&cx)),
        ("pve_069d.json", matches::pve(&cx)),
        ("pve_bots_069d.json", matches::pve_bots(&cx)),
        ("store_texts_069d.json", texts::store_texts(&cx)),
        ("event_categories_069d.json", texts::event_categories(&cx)),
        ("bot_names_069d.json", texts::bot_names(&cx)),
    ]
}

/// Write the tables into `dir` (created if missing). Each file is written to a temporary name and renamed, so a
/// failed run never leaves a half-written table.
pub fn write(dir: &Path, tables: &[Table]) -> Result<Vec<PathBuf>> {
    let err = |p: &Path, source| Error::Write { path: p.display().to_string(), source };
    std::fs::create_dir_all(dir).map_err(|e| err(dir, e))?;
    let mut written = Vec::new();
    for t in tables {
        let p = dir.join(t.name);
        let tmp = dir.join(format!(".{}.tmp", t.name));
        std::fs::write(&tmp, &t.bytes).map_err(|e| err(&tmp, e))?;
        std::fs::rename(&tmp, &p).map_err(|e| err(&p, e))?;
        written.push(p);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use client_files::options::{encode, Encoding};
    use client_files::{dict, list, volume};

    /// A game folder with one volume holding a two-item game.options and an exe file (made up, no game data).
    fn synthetic(dir: &std::path::Path) -> super::Source {
        let game = dict! {
            "item_categories" => list![dict! { "category_id" => 8, "is_stackable" => false }, dict! { "category_id" => 9, "is_stackable" => true }],
            "weapons" => dict! {
                "rifle_a" => dict! {
                    "parameters" => dict! { "dict_id" => 7, "category_id" => 8, "type" => "wpn_x", "item_level" => 2, "weight" => 3.9, "rarity" => list![0, 1] },
                    "ui_desc" => dict! { "name" => "st_rifle_a", "icon" => list![1, 2] },
                },
            },
            "equipment" => dict! {},
            "ammo_grenades_etc" => dict! {
                "ammo_b" => dict! { "parameters" => dict! { "dict_id" => 3, "category_id" => 9, "cost" => 0, "rarity" => list![list![1, 2]] }, "data" => dict! { "type" => 9 } },
                "no_id" => dict! { "parameters" => dict! { "category_id" => 9 } },
            },
            "camo_decal" => dict! {},
            "weapon_modules" => dict! {},
        };
        let vol = volume::build(&[("gameplay/game.options", &encode(&game, Encoding::Cp1251))], &[]);
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("resources.00.db"), vol).unwrap();
        std::fs::write(dir.join("survarium.exe"), b"MZ").unwrap();
        super::Source::open(dir, &dir.join("survarium.exe")).unwrap()
    }

    #[test]
    fn the_item_table_of_a_synthetic_install() {
        let dir = std::env::temp_dir().join(format!("data-extract-items-{}", std::process::id()));
        let src = synthetic(&dir);
        let each = super::extract_each(&src);
        assert_eq!(each.iter().map(|(n, _)| *n).collect::<Vec<_>>(), super::FILES);
        let items = String::from_utf8(each[0].1.as_ref().unwrap().clone()).unwrap();
        let rows: Vec<&str> = items.lines().collect();
        assert_eq!(rows.len(), 3, "{items}");
        assert!(rows[0].starts_with("dict_id\tcfg_name\tsection\t"));
        // sorted by dict id; floats as Python writes them; nested rarity as a compact list; category stackability
        assert_eq!(rows[1], "3\tammo_b\tammo_grenades_etc\t9\t\t\t\t\t0\t\t\t\t\t\t[[1,2]]\t\t\t\tTrue\t9\t");
        assert_eq!(rows[2], "7\trifle_a\tweapons\t8\twpn_x\t2\t\t\t\t\t\t3.9000000953674316\t\t\t0,1\tst_rifle_a\t1,2\t\tFalse\t\t");
        // a table whose config is not in the volumes fails on its own and names the file
        let ach = each.iter().find(|(n, _)| *n == "achievements_069d.json").unwrap();
        assert!(ach.1.as_ref().unwrap_err().to_string().contains("gameplay/quests/achievements.options"));
        assert!(super::extract(&src).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tables_are_written_whole() {
        let dir = std::env::temp_dir().join(format!("data-extract-write-{}", std::process::id()));
        let t = [super::Table { name: "a.json", bytes: b"{}\n".to_vec() }];
        let w = super::write(&dir, &t).unwrap();
        assert_eq!(std::fs::read(&w[0]).unwrap(), b"{}\n");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no temporary file left");
        std::fs::remove_dir_all(&dir).ok();
    }
}
