//! The per-release item tables: `equipment.tsv` and `weapons.tsv`, every
//! armour piece and weapon of the game data with what the wiki shows of it
//! (protection, weight, built-in bonuses, what it can roll, whether a
//! helmet flies off; damage, handling, recoil, spread), made by the same
//! code as the wiki's pages ([`crate::equipment`], [`crate::modifiers`]),
//! so they say what the server does. `srp-item-tables` writes them from a
//! server game data file; `release/build-content` and `release/release.sh`
//! run it, and the release's record commits them
//! (`srp-026g/docs/data/`), so each release's balance changes are a diff.
//!
//! Tab-separated, a header row, a row per item keyed by its `items_dict`
//! name; equipment by slot (the lobby's order), level and name, weapons by
//! class, level and name.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::game_data::GameDataError;
use survarium::v026g::item_params;
use survarium::v026g::weapon::{self, Weapon};

use crate::Lang;
use crate::equipment;
use crate::modifiers::{self, Group, number};

/// The tables' file names.
pub const EQUIPMENT: &str = "equipment.tsv";
pub const WEAPONS: &str = "weapons.tsv";

/// An item's English name, without the premium mark (the table's own
/// column says it).
fn name(dict_id: u16) -> String {
    let item = Dictionary::shared()
        .item(dict_id)
        .expect("an item of the dictionary");
    item_params::item_params(dict_id)
        .and_then(|params| params.name_key)
        .map_or_else(|| item.name.clone(), |key| modifiers::text(Lang::En, key))
}

/// A cell: no tabs or line breaks.
fn cell(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

fn yes(flag: bool) -> &'static str {
    if flag { "yes" } else { "" }
}

/// What an item comes with, as the lobby prints it.
fn built_in(dict_id: u16) -> String {
    modifiers::built_in(dict_id)
        .into_iter()
        .map(|(modification, property)| {
            format!(
                "{} {}",
                modifiers::property_name(Lang::En, property),
                modifiers::strength(Lang::En, property, modification.shown.value)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn table(header: &[&str], rows: Vec<Vec<String>>) -> String {
    let mut text = header.join("\t");
    text.push('\n');
    for row in rows {
        let row: Vec<String> = row.iter().map(|value| cell(value)).collect();
        text.push_str(&row.join("\t"));
        text.push('\n');
    }
    text
}

/// `equipment.tsv`.
pub fn equipment() -> String {
    let mut pieces = equipment::pieces();
    let dictionary = Dictionary::shared();
    // The lobby's slot order (as `pieces` has them), then level, then name.
    let slot_of = |group: Group| equipment::slot_word(group);
    let order: Vec<&str> = {
        let mut seen = Vec::new();
        for &(_, _, _, group) in &pieces {
            if !seen.contains(&slot_of(group)) {
                seen.push(slot_of(group));
            }
        }
        seen
    };
    pieces.sort_by(|a, b| {
        let rank = |group| order.iter().position(|&slot| slot == slot_of(group));
        (rank(a.3), a.1.item_level, &a.0.name).cmp(&(rank(b.3), b.1.item_level, &b.0.name))
    });
    let rows = pieces
        .into_iter()
        .map(|(item, params, armour, group)| {
            let protection =
                equipment::protection(Lang::En, armour).map_or_else(String::new, |table| {
                    table
                        .rows
                        .iter()
                        .map(|(parts, values)| {
                            let values: Vec<String> = table
                                .heads
                                .iter()
                                .zip(values)
                                .map(|(head, value)| {
                                    format!(
                                        "{} {}",
                                        head.to_lowercase(),
                                        value.map_or_else(|| "-".to_owned(), equipment::hundredths)
                                    )
                                })
                                .collect();
                            format!("{}: {}", parts.join("/"), values.join(", "))
                        })
                        .collect::<Vec<_>>()
                        .join("; ")
                });
            vec![
                item.name.clone(),
                name(item.dict_id),
                slot_of(group).to_owned(),
                params.item_level.to_string(),
                equipment::faction_name(Lang::En, params.faction),
                yes(dictionary
                    .item(item.dict_id)
                    .is_some_and(|row| row.is_premium))
                .to_owned(),
                protection,
                number(params.weight),
                built_in(item.dict_id),
                modifiers::can_roll(Lang::En, item.dict_id).join(", "),
                yes(equipment::flies_off(armour)).to_owned(),
            ]
        })
        .collect();
    table(
        &[
            "item",
            "name",
            "slot",
            "level",
            "faction",
            "premium",
            "protection",
            "weight_kg",
            "comes_with",
            "can_roll",
            "flies_off",
        ],
        rows,
    )
}

/// A range as `low–high`, or one number when they agree.
fn range((low, high): (f32, f32)) -> String {
    if low == high {
        number(low)
    } else {
        format!("{}–{}", number(low), number(high))
    }
}

/// A value the config may leave out: empty when it does.
fn maybe<T>(value: Option<T>, show: impl FnOnce(T) -> String) -> String {
    value.map_or_else(String::new, show)
}

/// `weapons.tsv`; an error when a weapon's config does not read.
pub fn weapons() -> Result<String, GameDataError> {
    let dictionary = Dictionary::shared();
    let all = weapon::derive(survarium::v026g::game_data::shared(), dictionary)?;
    let mut rows: Vec<(Group, u8, String, Weapon)> = Vec::new();
    for weapon in all {
        let (Some(item), Some(params)) = (
            dictionary.item(weapon.dict_id),
            item_params::item_params(weapon.dict_id),
        ) else {
            continue;
        };
        let Some(group) = Group::of(item.category) else {
            continue;
        };
        rows.push((group, params.item_level, item.name.clone(), weapon));
    }
    rows.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
    let percent = |value: Option<f32>| maybe(value, |value| number(value * 100.0));
    let plain = |value: Option<f32>| maybe(value, number);
    let rows = rows
        .into_iter()
        .map(|(group, level, dict_name, w)| {
            let recoil = &w.recoil;
            // The side kick a shot gives: right minus left.
            let side = recoil
                .right
                .zip(recoil.left)
                .map(|(right, left)| (right.0 - left.1, right.1 - left.0));
            vec![
                dict_name,
                name(w.dict_id),
                modifiers::text(Lang::En, group.name_key()),
                level.to_string(),
                yes(dictionary.item(w.dict_id).is_some_and(|row| row.is_premium)).to_owned(),
                percent(w.damage),
                percent(w.pierce),
                plain(w.max_damage_distance),
                plain(w.min_damage_distance),
                percent(w.min_damage_amount),
                plain(w.rounds_per_minute),
                maybe(w.magazine, |count| count.to_string()),
                plain(w.reload_time),
                plain(w.aim_time),
                plain(w.weight),
                maybe(recoil.top, range),
                maybe(side, range),
                plain(recoil.power),
                plain(w.spread.base),
                plain(w.spread.per_shot),
                plain(w.spread.max),
                plain(w.spread.recovery),
                built_in(w.dict_id),
                modifiers::can_roll(Lang::En, w.dict_id).join(", "),
            ]
        })
        .collect();
    Ok(table(
        &[
            "item",
            "name",
            "class",
            "level",
            "premium",
            "damage",
            "penetration",
            "full_damage_to_m",
            "min_damage_from_m",
            "min_damage_pct",
            "rpm",
            "magazine",
            "reload_s",
            "aim_s",
            "weight_kg",
            "kick_up",
            "kick_side",
            "kick_power",
            "spread_base",
            "spread_per_shot",
            "spread_max",
            "spread_recovery",
            "comes_with",
            "can_roll",
        ],
        rows,
    ))
}

/// Why the tables could not be written.
#[derive(Debug)]
pub enum Error {
    /// A config of the game data does not read.
    Data(GameDataError),
    /// A table could not be read or written.
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Data(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
        }
    }
}

/// The rows of a table's text by their key (the first column), and its
/// header.
fn parse(text: &str) -> (Vec<String>, BTreeMap<String, Vec<String>>) {
    let mut lines = text.lines();
    let header: Vec<String> = lines
        .next()
        .unwrap_or("")
        .split('\t')
        .map(str::to_owned)
        .collect();
    let rows = lines
        .filter(|line| !line.is_empty())
        .map(|line| {
            let row: Vec<String> = line.split('\t').map(str::to_owned).collect();
            (row[0].clone(), row)
        })
        .collect();
    (header, rows)
}

/// What changed from `old` to `new`, a line per change: an item's column
/// `old -> new`, an item new or gone, or the columns themselves.
pub fn diff(old: &str, new: &str) -> Vec<String> {
    let (old_head, old_rows) = parse(old);
    let (new_head, new_rows) = parse(new);
    let mut lines = Vec::new();
    if old.is_empty() {
        lines.push(format!("a new table: {} items", new_rows.len()));
        return lines;
    }
    if old_head != new_head {
        lines.push(format!(
            "columns: {} -> {}",
            old_head.join(" "),
            new_head.join(" ")
        ));
    }
    for (key, row) in &new_rows {
        match old_rows.get(key) {
            None => lines.push(format!("{key}: new")),
            Some(was) if was != row => {
                for (column, head) in new_head.iter().enumerate().skip(1) {
                    let before = old_head
                        .iter()
                        .position(|other| other == head)
                        .and_then(|at| was.get(at))
                        .map_or("", String::as_str);
                    let after = row.get(column).map_or("", String::as_str);
                    if before != after {
                        lines.push(format!(
                            "{key}: {head} {} -> {}",
                            shown(before),
                            shown(after)
                        ));
                    }
                }
            }
            Some(_) => {}
        }
    }
    for key in old_rows.keys().filter(|key| !new_rows.contains_key(*key)) {
        lines.push(format!("{key}: gone"));
    }
    lines
}

fn shown(value: &str) -> &str {
    if value.is_empty() { "(none)" } else { value }
}

/// Writes the tables of this process's game data into `dir` where they
/// differ from what is there, and says, a table at a time, `unchanged` or
/// what changed (a line per change). Nothing is written when a config does
/// not read.
pub fn update(dir: &Path) -> Result<Vec<String>, Error> {
    let tables = [
        (EQUIPMENT, equipment()),
        (WEAPONS, weapons().map_err(Error::Data)?),
    ];
    fs::create_dir_all(dir).map_err(Error::Io)?;
    let mut report = Vec::new();
    for (file, text) in tables {
        let path = dir.join(file);
        let was = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(Error::Io(error)),
        };
        if was == text {
            report.push(format!("{file}: unchanged"));
            continue;
        }
        let changes = diff(&was, &text);
        report.push(format!("{file}: {} change(s)", changes.len()));
        report.extend(changes.into_iter().map(|line| format!("  {line}")));
        fs::write(&path, text).map_err(Error::Io)?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_have_every_piece_and_weapon_once() {
        let equipment = equipment();
        let weapons = weapons().unwrap();
        let (_, rows) = parse(&equipment);
        assert_eq!(rows.len(), equipment::pieces().len());
        assert_eq!(rows.len(), equipment.lines().count() - 1, "keys are unique");
        let (head, rows) = parse(&weapons);
        let ak = &rows["ak_74m"];
        let column = |name: &str| ak[head.iter().position(|h| h == name).unwrap()].clone();
        assert_eq!(column("damage"), "50");
        assert_eq!(column("rpm"), "650");
        assert_eq!(column("magazine"), "30");
        assert_eq!(column("kick_up"), "1.8–2");
        assert!(column("can_roll").contains("Damage"));
        // The same text twice: no churn.
        assert_eq!(equipment, super::equipment());
    }

    #[test]
    fn a_diff_names_each_changed_column_and_new_and_gone_items() {
        let old = "item\tweight_kg\tcomes_with\na\t3\t\nb\t1\tx\n";
        let new = "item\tweight_kg\tcomes_with\na\t2.5\tMovement speed +4%\nc\t1\t\n";
        assert_eq!(
            diff(old, new),
            [
                "a: weight_kg 3 -> 2.5",
                "a: comes_with (none) -> Movement speed +4%",
                "c: new",
                "b: gone",
            ]
        );
        assert!(diff(new, new).is_empty());
        assert_eq!(diff("", new), ["a new table: 2 items"]);
    }

    #[test]
    fn update_writes_only_what_changed() {
        let dir = std::env::temp_dir().join(format!("srp-item-tables-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let first = update(&dir).unwrap();
        assert!(first.iter().any(|line| line.starts_with("equipment.tsv: ")));
        let second = update(&dir).unwrap();
        assert_eq!(
            second,
            ["equipment.tsv: unchanged", "weapons.tsv: unchanged"]
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
