//! Rarity and stat modifiers of a new module instance (docs/notes/workshop.md section 7).
//!
//! Data: `data/module_drops_069d.json` (`crates/data-extract`, `modules_drop_options` of
//! game.options). The client parses the same tables (0x2002DB560..0x2002DBEDB) but rolls nothing: it only shows
//! them (below). Facts:
//!
//! * a module type has up to two drop modifier groups, `first_drop_modifier` (206 of 422 rows) and
//!   `second_drop_modifier` (156); grades 0..10 of the `modifications` table, 10 the best (e.g. group 403 aim time
//!   penalty 0.10 at grade 0, 0 at grade 10) (PROVEN data);
//! * the five 11 x 11 tables are indexed `[first grade][second grade]` (PROVEN 0x2001BD0C0: the bounds of the first
//!   group are the row bounds, of the second the column bounds); `modules_rarity` gives the table rarity value of a
//!   grade pair (-1 = none), the four weight tables the roll weights;
//! * table rarity value v = module rarity enum (`ModuleRarityEnums`, the group-312 grade of the record) green (1),
//!   blue (2), orange (3) for v = 0, 1, 2 and legendary (5) for v = 3 ([`value_rarity`]). PROVEN by three sources: the
//!   client's `fill_module_next_grade_max_values` (0x2001BCF60, called when the upgrade panel opens and after the
//!   `83 45 10` reply) maps the module's 312 grade 0 / 1 -> v 1, 2 -> 2, 3 / 4 -> 3, 5 -> none (jump table
//!   0x2001BD0A0) and sends the grade bounds of v as `next_min_value` / `next_max_value` (the stats after the next
//!   merge: green -> blue = v 1, blue -> orange = v 2, orange / gold -> legendary = v 3); the 0.53 notes (2018-08-03)
//!   name the qualities "Factory-made - green (default) ... Mastercraft - purple ... the maximum positive and minimum
//!   (or nonexistent) negative modifiers", and v 3 is the single cell (10, 10) = both best grades; legendary is drawn
//!   purple (`ModuleTooltip`, 0x8759AC). Grey (0) and gold (4) have no cells: gold is the bought "Special" module
//!   (`ModuleData::bought_mods`), grey a module without a rarity pair;
//! * the weight tables hold one rarity each except the general one: `pair` (`modules_mods_pair_prob`) gives
//!   v 0 / 1 / 2, `second` only v 1, `pve` only v 2, `fourth` only v 3 (the cell (10, 10)) (PROVEN data);
//! * a module with one drop modifier: the client takes the bounds of rarity v from the column the table's last
//!   column holding v (`cmax`, column 9 for v 0..2, 10 for v 3) and the rows of that column holding v
//!   (0x2002DBDA3..0x2002DBE81, read by 0x2001BD159 when the record has 2 pairs). [`ModuleDrops::roll`] draws such a
//!   module from the cells `(i, cmax(v))` only, so its grade always lies inside the bounds the client shows (LIKELY:
//!   the real roll is not in the client).
//!
//! Sources and their table (rules of [`Source`]):
//!
//! | source | table | evidence |
//! |---|---|---|
//! | PvP win chest `random_module_chest` | `pair` (green 71 %, blue 24 %, orange 4.7 % for two modifiers) | 0.51 notes "random modules for each PvP win. Higher quality modules are more rare"; the general table |
//! | workshop "Create" for spare parts (60 01 action 4) | `pair` | 0.60 notes "Rarity of the module will be determined randomly" (LIKELY: the general table) |
//! | `rare_modules_chest*` (faction battle top rewards) | `second` (blue) | LIKELY: "rare" = the middle of "common, rare, legendary" (0.51 preview) and the 0.51b "High quality modules (rare)" of the treasure hunt |
//! | treasure hunt module entries | `second` (blue) | LIKELY: the page entries of the modules carry `rare_level` 1 = table value 1; 0.51b "High quality modules (rare) can now be found ... in the Treasure Hunt" |
//! | PvE (team mission) drops | `pve` (orange) | table name; 0.51b "High quality modules (rare) ... in Team Missions' stashes". No PvE module drop path exists yet |
//! | merge to blue / orange / legendary | `second` / `pve` / `fourth` | the panel shows the next rarity's bounds before the merge (above); 0.53 notes "obtain rarer modules with improved characteristics"; the table holding only that rarity |
//! | merge grey -> green | `pair`, cells of v 0 only | CHOICE (no table holds only v 0; a grey module exists only without a rarity pair) |

use std::collections::HashMap;

use serde::Deserialize;

use crate::craft::Roll;
use crate::merge::{RARITY_GOLD, RARITY_GROUP, RARITY_LEGENDARY};

/// Side of the weight tables.
pub const GRADES: usize = 11;

type Table = [[f64; GRADES]; GRADES];

#[derive(Debug, Deserialize)]
struct RawTables {
    pair: Table,
    second: Table,
    pve: Table,
    fourth: Table,
}

#[derive(Debug, Deserialize)]
struct Raw {
    tables: RawTables,
    rarity: [[i8; GRADES]; GRADES],
    rarity_group: u16,
    used_weapon_percent: u32,
    used_weapon_pve_percent: u32,
    drop_mods: Vec<(u16, Option<u16>, Option<u16>)>,
}

/// Which weight table a new module is rolled from (module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `modules_mods_pair_prob`: PvP win chest, workshop "Create"
    Match,
    /// `modules_mods_pair_second_rarity_prob` (blue only): rare-module chests, treasure hunt
    Rare,
    /// `modules_mods_pair_pve_prob` (orange only): PvE drops
    Pve,
    /// `modules_mods_pair_fourth_rarity_prob` (legendary only, grades 10 / 10)
    Mastercraft,
}

/// Rarity enum of a table rarity value (module docs): 0 green, 1 blue, 2 orange, 3 legendary.
pub fn value_rarity(v: i8) -> Option<u16> {
    match v {
        0..=2 => Some(v as u16 + 1),
        3 => Some(RARITY_LEGENDARY),
        _ => None,
    }
}

/// Table rarity value of a rarity enum (inverse of [`value_rarity`]; grey and gold have none).
pub fn rarity_value(r: u16) -> Option<i8> {
    match r {
        1..=3 => Some(r as i8 - 1),
        RARITY_LEGENDARY => Some(3),
        _ => None,
    }
}

#[derive(Debug)]
pub struct ModuleDrops {
    pair: Table,
    second: Table,
    pve: Table,
    fourth: Table,
    rarity: [[i8; GRADES]; GRADES],
    /// table value -> the last column holding it (the client's single-modifier column)
    cmax: [usize; 4],
    rarity_group: u16,
    /// `used_weapon_dropping_modules_percent` / `used_weapon_pve_dropping_modules_percent` (which module drops; not
    /// applied, see docs/notes/workshop.md section 7)
    pub used_weapon_percent: u32,
    pub used_weapon_pve_percent: u32,
    /// module dict id -> (first, second drop modifier group)
    drop_mods: HashMap<u16, (Option<u16>, Option<u16>)>,
}

impl ModuleDrops {
    pub fn parse(json: &str) -> Result<ModuleDrops, serde_json::Error> {
        let raw: Raw = serde_json::from_str(json)?;
        let mut cmax = [0usize; 4];
        for (v, c) in cmax.iter_mut().enumerate() {
            *c = (0..GRADES).filter(|&j| (0..GRADES).any(|i| raw.rarity[i][j] == v as i8)).max().unwrap_or(0);
        }
        Ok(ModuleDrops {
            pair: raw.tables.pair,
            second: raw.tables.second,
            pve: raw.tables.pve,
            fourth: raw.tables.fourth,
            rarity: raw.rarity,
            cmax,
            rarity_group: raw.rarity_group,
            used_weapon_percent: raw.used_weapon_percent,
            used_weapon_pve_percent: raw.used_weapon_pve_percent,
            drop_mods: raw.drop_mods.into_iter().map(|(d, a, b)| (d, (a, b))).collect(),
        })
    }

    /// The modifier group that stores the rarity (`module_rarity_modifier`, 312).
    pub fn rarity_group(&self) -> u16 {
        self.rarity_group
    }

    /// (first, second) drop modifier group of a module type; (None, None) for a module without a row.
    pub fn drop_groups(&self, module: u16) -> (Option<u16>, Option<u16>) {
        self.drop_mods.get(&module).copied().unwrap_or((None, None))
    }

    /// Table rarity value of a grade pair (-1 = no module has it).
    pub fn cell_value(&self, first: usize, second: usize) -> i8 {
        self.rarity[first][second]
    }

    fn table(&self, s: Source) -> &Table {
        match s {
            Source::Match => &self.pair,
            Source::Rare => &self.second,
            Source::Pve => &self.pve,
            Source::Mastercraft => &self.fourth,
        }
    }

    /// The cells a module of type `module` can be drawn from in table `s` (only cells of table value `only` when
    /// given), with their weights: weight > 0 and a rarity; a module with one drop modifier only from its rarity's
    /// client column (module docs).
    pub fn cells(&self, module: u16, s: Source, only: Option<i8>) -> Vec<(usize, usize, f64)> {
        let t = self.table(s);
        let single = matches!(self.drop_groups(module), (Some(_), None));
        let mut out = vec![];
        for (i, row) in t.iter().enumerate() {
            for (j, &w) in row.iter().enumerate() {
                let v = self.rarity[i][j];
                if w <= 0.0 || v < 0 || only.is_some_and(|o| o != v) {
                    continue;
                }
                if single && j != self.cmax[v as usize] {
                    continue;
                }
                out.push((i, j, w));
            }
        }
        out
    }

    fn draw(&self, module: u16, s: Source, only: Option<i8>, r: &mut dyn Roll) -> Option<(usize, usize, i8)> {
        let cells = self.cells(module, s, only);
        let total: f64 = cells.iter().map(|c| c.2).sum();
        if cells.is_empty() || total <= 0.0 {
            return None;
        }
        let mut x = r.unit() * total;
        let mut pick = cells[cells.len() - 1];
        for c in &cells {
            if x < c.2 {
                pick = *c;
                break;
            }
            x -= c.2;
        }
        Some((pick.0, pick.1, self.rarity[pick.0][pick.1]))
    }

    /// The pairs of a drawn cell: rarity, then the drop modifiers the module type has.
    fn pairs(&self, module: u16, i: usize, j: usize, v: i8) -> Vec<(u16, u16)> {
        let (a, b) = self.drop_groups(module);
        let mut out = vec![(self.rarity_group, value_rarity(v).unwrap_or(0))];
        if let Some(g) = a {
            out.push((g, i as u16));
        }
        if let Some(g) = b {
            out.push((g, j as u16));
        }
        out
    }

    /// The stored modifier list of a new instance of `module` from source `s`: `(312, rarity)`, `(first group,
    /// row)`, `(second group, column)` (the type's `override_modifiers` are added by `ModuleData::instance_mods`).
    /// A module without drop modifiers gets the rarity pair only. None when the table has no cell for it.
    pub fn roll(&self, module: u16, s: Source, r: &mut dyn Roll) -> Option<Vec<(u16, u16)>> {
        let (i, j, v) = self.draw(module, s, None, r)?;
        Some(self.pairs(module, i, j, v))
    }

    /// The modifier list after a successful merge to rarity `to`: the drop grades drawn again from the table that holds
    /// only that rarity (blue `second`, orange `pve`, legendary `fourth`; green: the `pair` cells of green, CHOICE),
    /// the rarity pair set to `to`, every other stored pair kept (module docs). A module without drop modifiers only
    /// gets the new rarity pair. None when `to` has no cells (grey, gold).
    pub fn merged(&self, module: u16, mods: &[(u16, u16)], to: u16, r: &mut dyn Roll) -> Option<Vec<(u16, u16)>> {
        let v = rarity_value(to)?;
        let s = match to {
            2 => Source::Rare,
            3 => Source::Pve,
            RARITY_LEGENDARY => Source::Mastercraft,
            _ => Source::Match,
        };
        let (i, j, v) = self.draw(module, s, Some(v), r)?;
        let mut out = self.pairs(module, i, j, v);
        let (a, b) = self.drop_groups(module);
        for &(g, x) in mods {
            if g != RARITY_GROUP && Some(g) != a && Some(g) != b {
                out.push((g, x));
            }
        }
        Some(out)
    }

    /// The stored modifier list of a module that no drop table names a source for (the new-player kit, the module seed of
    /// an old profile, a module granted by a reward list): rarity green "Factory-made" `(312, 1)` plus the drop grades
    /// drawn from the green cells of the general table `pair` (exactly [`ModuleDrops::merged`] to 1 of a module with no
    /// pairs; a one-modifier module from column `cmax(0)` = 9). LIKELY: the 0.69d client has no state for an
    /// owned module without a rarity (no name, filter or merge price; "Factory-made - green (default)", 0.53 notes), so
    /// every module the server creates gets one; random green grades rather than fixed ones (no 0.69d source fixes them).
    /// A module type the table has no green cell for (none in 0.69d data) gets the rarity pair alone.
    pub fn green(&self, module: u16, r: &mut dyn Roll) -> Vec<(u16, u16)> {
        self.merged(module, &[], 1, r).unwrap_or_else(|| vec![(self.rarity_group, 1)])
    }

    /// The grade bounds the client shows for a module of table value `v` (`(first lo, first hi, second lo, second hi)`
    /// from the box of all cells holding `v`, 0x2002DBCB0..0x2002DBDCF; one-modifier modules: rows of column `cmax(v)`,
    /// 0x2002DBDA3..), for tests.
    pub fn client_bounds(&self, v: i8, single: bool) -> Option<(usize, usize, usize, usize)> {
        let cells: Vec<(usize, usize)> = (0..GRADES)
            .flat_map(|i| (0..GRADES).map(move |j| (i, j)))
            .filter(|&(i, j)| self.rarity[i][j] == v && (!single || j == self.cmax[v as usize]))
            .collect();
        let lo_i = cells.iter().map(|c| c.0).min()?;
        let hi_i = cells.iter().map(|c| c.0).max()?;
        let lo_j = cells.iter().map(|c| c.1).min()?;
        let hi_j = cells.iter().map(|c| c.1).max()?;
        Some((lo_i, hi_i, lo_j, hi_j))
    }
}

/// The weight table of a chest's modules (module docs): the rare-module chests `rare_modules_chest*` draw from the
/// blue table, every other chest (the PvP win `random_module_chest`) from the general one.
pub fn chest_source(chest_name: &str) -> Source {
    if chest_name.trim_start_matches('/').starts_with("rare_modules_chest") { Source::Rare } else { Source::Match }
}

/// Is `r` a rarity a merge can produce (green .. legendary, not gold)?
pub fn mergeable_target(r: u16) -> bool {
    r != RARITY_GOLD && rarity_value(r).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::SplitMix;
    use crate::GameData;

    fn gd() -> GameData {
        crate::testing::tables()
    }

    /// dict 2002 has both drop modifiers (325 reload time, 411 aim time); find a one-modifier module too.
    fn two_and_one(gd: &GameData) -> (u16, u16) {
        let d = &gd.module_drops;
        assert_eq!(d.drop_groups(2002), (Some(325), Some(411)));
        let one = gd.items.iter().map(|i| i.dict_id).find(|m| matches!(d.drop_groups(*m), (Some(_), None))).unwrap();
        (2002, one)
    }

    #[test]
    fn value_and_enum_map() {
        crate::require_tables!();
        assert_eq!((0..4).map(value_rarity).collect::<Vec<_>>(), vec![Some(1), Some(2), Some(3), Some(5)]);
        assert_eq!(value_rarity(-1), None);
        assert_eq!([0, 1, 2, 3, 4, 5].map(rarity_value), [None, Some(0), Some(1), Some(2), None, Some(3)]);
        let gd = gd();
        assert_eq!(gd.module_drops.rarity_group(), 312);
        assert_eq!(gd.module_drops.cmax, [9, 9, 9, 10]);
        assert_eq!(gd.module_drops.cell_value(10, 10), 3);
    }

    /// The general table rolled 40 000 times with a seeded generator: the share of each rarity is the table's (weights
    /// summed per rarity value), every pair is a real (group, grade) and every grade lies in the client's bounds.
    #[test]
    fn match_roll_follows_the_table() {
        crate::require_tables!();
        let gd = gd();
        let d = &gd.module_drops;
        let (two, one) = two_and_one(&gd);
        for (m, single, want) in [(two, false, [70.94, 24.37, 4.69]), (one, true, [42.25, 33.80, 23.94])] {
            let mut r = SplitMix(7);
            let mut n = [0u32; 6];
            const N: u32 = 40_000;
            for _ in 0..N {
                let l = d.roll(m, Source::Match, &mut r).unwrap();
                assert_eq!(l[0].0, 312);
                let rar = l[0].1;
                n[rar as usize] += 1;
                let v = rarity_value(rar).unwrap();
                let (lo_i, hi_i, lo_j, hi_j) = d.client_bounds(v, single).unwrap();
                let i = l[1].1 as usize;
                assert!((lo_i..=hi_i).contains(&i), "{m} {l:?}");
                if !single {
                    let j = l[2].1 as usize;
                    assert!((lo_j..=hi_j).contains(&j), "{m} {l:?}");
                    assert_eq!(d.cell_value(i, j), v);
                    assert_eq!(l.len(), 3);
                } else {
                    assert_eq!(l.len(), 2);
                }
            }
            assert_eq!(n[0] + n[4] + n[5], 0, "general drops are green / blue / orange only");
            for (k, w) in want.iter().enumerate() {
                let got = n[k + 1] as f64 * 100.0 / N as f64;
                assert!((got - w).abs() < 1.0, "module {m} rarity {} got {got:.2} % want {w} %", k + 1);
            }
        }
    }

    #[test]
    fn single_rarity_tables() {
        crate::require_tables!();
        let gd = gd();
        let d = &gd.module_drops;
        let (two, one) = two_and_one(&gd);
        let mut r = SplitMix(11);
        for m in [two, one] {
            for (s, want) in [(Source::Rare, 2), (Source::Pve, 3), (Source::Mastercraft, 5)] {
                for _ in 0..200 {
                    assert_eq!(d.roll(m, s, &mut r).unwrap()[0], (312, want));
                }
            }
            // legendary = both best grades
            let l = d.roll(m, Source::Mastercraft, &mut r).unwrap();
            assert!(l[1..].iter().all(|p| p.1 == 10), "{l:?}");
        }
    }

    #[test]
    fn merge_redraws_the_drop_grades_for_the_new_rarity() {
        crate::require_tables!();
        let gd = gd();
        let d = &gd.module_drops;
        let mut r = SplitMix(3);
        // a gold (bought) module: premium (325, 7) (411, 7), override kept elsewhere
        let bought = vec![(325, 7), (411, 7), (312, 4), (520, 7)];
        let l = d.merged(2002, &bought, 5, &mut r).unwrap();
        assert_eq!(l, vec![(312, 5), (325, 10), (411, 10), (520, 7)]);
        for to in [1u16, 2, 3] {
            for _ in 0..100 {
                let l = d.merged(2002, &[(312, to - 1), (325, 0), (411, 0)], to, &mut r).unwrap();
                assert_eq!(l[0], (312, to));
                assert_eq!(d.cell_value(l[1].1 as usize, l[2].1 as usize), rarity_value(to).unwrap());
            }
        }
        assert_eq!(d.merged(2002, &[], 4, &mut r), None);
        assert!(!mergeable_target(4) && mergeable_target(5) && !mergeable_target(0));
        // a module type without drop modifiers keeps its pairs and gets the rarity
        let plain = gd.items.iter().map(|i| i.dict_id).find(|m| gd.items.is_module(*m) && d.drop_groups(*m) == (None, None)).unwrap();
        assert_eq!(d.merged(plain, &[(312, 1)], 2, &mut r), Some(vec![(312, 2)]));
    }

    /// `green`: always `(312, 1)` first, the drop grades inside the client's green bounds (one-modifier modules from
    /// column 9), for every module type of the data.
    #[test]
    fn green_rolls_stay_inside_the_green_bounds() {
        crate::require_tables!();
        let gd = gd();
        let d = &gd.module_drops;
        let mut r = SplitMix(13);
        for it in gd.items.iter().filter(|i| gd.items.is_module(i.dict_id)) {
            let m = it.dict_id;
            let (a, b) = d.drop_groups(m);
            for _ in 0..20 {
                let l = d.green(m, &mut r);
                assert_eq!(l[0], (312, 1), "{m} {l:?}");
                assert_eq!(l.len(), 1 + a.is_some() as usize + b.is_some() as usize, "{m} {l:?}");
                if let Some(g) = a {
                    let (lo_i, hi_i, lo_j, hi_j) = d.client_bounds(0, b.is_none()).unwrap();
                    assert_eq!(l[1].0, g);
                    assert!((lo_i..=hi_i).contains(&(l[1].1 as usize)), "{m} {l:?}");
                    if let Some(g2) = b {
                        assert_eq!(l[2].0, g2);
                        assert!((lo_j..=hi_j).contains(&(l[2].1 as usize)), "{m} {l:?}");
                        assert_eq!(d.cell_value(l[1].1 as usize, l[2].1 as usize), 0, "{m} {l:?}");
                    } else {
                        assert_eq!(d.cell_value(l[1].1 as usize, 9), 0, "{m} {l:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn chest_sources() {
        crate::require_tables!();
        assert_eq!(chest_source("random_module_chest"), Source::Match);
        assert_eq!(chest_source("rare_modules_chest_3"), Source::Rare);
        assert_eq!(chest_source("/rare_modules_chest"), Source::Rare);
        let gd = gd();
        for c in [1957u16, 2033, 2034, 2035, 2036] {
            assert_eq!(chest_source(gd.chests.name(c).unwrap()), Source::Rare, "{c}");
        }
        assert_eq!(chest_source(gd.chests.name(1872).unwrap()), Source::Match);
    }

    /// Every module a chest or the hunt can give has a drop modifier, and every roll fits the record (rarity + 2 drop
    /// pairs + the override pairs <= 4).
    #[test]
    fn rolled_lists_fit_the_record() {
        crate::require_tables!();
        let gd = gd();
        let mut r = SplitMix(5);
        for it in gd.items.iter().filter(|i| gd.items.is_module(i.dict_id)) {
            let l = gd.module_drops.roll(it.dict_id, Source::Match, &mut r).unwrap();
            let full = gd.modules.instance_mods(it.dict_id, &l);
            assert!(l.len() <= crate::modules::MAX_MODULE_MODS && full.len() >= l.len().min(4), "{} {l:?}", it.dict_id);
            assert_eq!(&full[..l.len()], &l[..], "{} {full:?}", it.dict_id);
        }
    }
}
