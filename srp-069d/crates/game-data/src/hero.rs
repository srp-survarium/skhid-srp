//! Hero of the day: the data (`data/hero_of_the_day_069d.json`, `crates/data-extract`) and the ranking rule.
//! Notes: `docs/notes/hero-of-the-day.md`. PROVEN = read in the exe or the game data, CHOICE = server rule.
//!
//! Data (PROVEN, `gameplay/event_rules/hero_of_the_day.options`, volume 00; the client reads the same file at
//! 0x2001BB5F0): `competitive_groups` (`min_level`, `max_level`, `max_player_level`, per place the modifier
//! (group, grade) pairs of `game.options` `modifications` = +25 / +20 / +15 % to experience, silver and reputation for
//! place 1 / 2 / 3), `minutes_recalc_period` 30 and `minutes_operating_period` 1440.
//!
//! The ranking value of each nomination is a CHOICE (the client only shows the number): `score` = the best single
//! ranked match score of the window, `killstreak` = the longest kill streak of one ranked match, `winstreak` = the
//! longest run of consecutive ranked wins. The first to reach the value ranks higher on a tie.

use std::collections::BTreeMap;

use serde::Deserialize;

pub const FILE: &str = "hero_of_the_day_069d.json";

/// The three nominations, in the client's order (the `st_*_label` texts of the window, 0x2001CA550 loop).
pub const SCORE: u8 = 1;
pub const KILLSTREAK: u8 = 2;
pub const WINSTREAK: u8 = 3;
pub const NOMINATIONS: [u8; 3] = [SCORE, KILLSTREAK, WINSTREAK];
/// Places per nomination (the options list three `priority` rows, the window shows three).
pub const PLACES: usize = 3;

/// Bonus fractions of one place (0.25 = +25 %).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct PlaceBonus {
    pub place: u8,
    pub experience: f64,
    pub silver: f64,
    pub reputation: f64,
    /// the `modifications` group / grade of the experience modifier (141 / 4, 3, 2): what the 94 record names
    pub exp_group: u16,
    pub grade: u16,
}

impl PlaceBonus {
    /// The percent shown to the player (the A6 bonus byte, `set_hero_of_the_day_level` percent).
    pub fn percent(&self) -> u32 {
        (self.experience * 100.0).round() as u32
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Group {
    pub min_level: u8,
    pub max_level: u8,
    pub max_player_level: u8,
    pub bonuses: Vec<PlaceBonus>,
}

#[derive(Debug, Clone, Deserialize)]
struct Raw {
    competitive_groups: Vec<Group>,
    minutes_recalc_period: u32,
    minutes_operating_period: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeroData {
    pub groups: Vec<Group>,
    /// seconds between two recalculations
    pub recalc_secs: i64,
    /// seconds a title and its bonus last
    pub operating_secs: i64,
}

/// One finished ranked match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Played {
    pub account: i64,
    pub at: i64,
    pub group: u8,
    pub score: u32,
    pub killstreak: u32,
    pub won: bool,
}

/// One ranked place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub account: i64,
    pub value: u32,
}

impl HeroData {
    pub fn parse(json: &str) -> Result<HeroData, serde_json::Error> {
        let raw: Raw = serde_json::from_str(json)?;
        Ok(HeroData { groups: raw.competitive_groups, recalc_secs: i64::from(raw.minutes_recalc_period) * 60, operating_secs: i64::from(raw.minutes_operating_period) * 60 })
    }

    /// The competitive group of a match whose highest equipment level is `level`: the first group whose range holds it
    /// (the client's own selection, 0x2001CAF10: the loop falls back to group 0 when none matches, so an unknown level
    /// is group 0 too). A KNOWN level outside every range is None (CHOICE: no group, no ranking).
    pub fn group_of(&self, level: Option<u8>) -> Option<u8> {
        let Some(l) = level else { return (!self.groups.is_empty()).then_some(0) };
        self.groups.iter().position(|g| g.min_level <= l && l <= g.max_level).map(|i| i as u8)
    }

    /// Is a character of `player_level` admitted to the group (`st_hero_allowance_info`: "no higher than")?
    pub fn admits(&self, group: u8, player_level: u32) -> bool {
        self.groups.get(group as usize).is_some_and(|g| player_level <= u32::from(g.max_player_level))
    }

    pub fn bonus(&self, group: u8, place: u8) -> Option<&PlaceBonus> {
        self.groups.get(group as usize)?.bonuses.iter().find(|b| b.place == place)
    }

    /// Start of the recalculation slot `now` lies in.
    pub fn slot_of(&self, now: i64) -> i64 {
        now.div_euclid(self.recalc_secs) * self.recalc_secs
    }
}

/// The ranking value of every account in one (group, nomination) over `rows` (all of them in the window, oldest
/// first): returns the best three, best first. A value of 0 does not rank (the client hides score 0 entries).
pub fn rank(rows: &[Played], group: u8, nomination: u8) -> Vec<Place> {
    // account -> (value, the time it was reached)
    let mut best: BTreeMap<i64, (u32, i64)> = BTreeMap::new();
    let mut run: BTreeMap<i64, u32> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.group == group) {
        let value = match nomination {
            SCORE => r.score,
            KILLSTREAK => r.killstreak,
            _ => {
                let n = run.entry(r.account).or_insert(0);
                *n = if r.won { *n + 1 } else { 0 };
                *n
            }
        };
        let e = best.entry(r.account).or_insert((0, r.at));
        if value > e.0 {
            *e = (value, r.at);
        }
    }
    let mut v: Vec<(i64, u32, i64)> = best.into_iter().filter(|(_, (val, _))| *val > 0).map(|(a, (val, at))| (a, val, at)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0)));
    v.into_iter().take(PLACES).map(|(account, value, _)| Place { account, value }).collect()
}

pub const EXCHANGE_FILE: &str = "event_exchange_069d.json";

/// One row of `gameplay/event_rules/event_exchange_action_items.options` `exchange_items` (PROVEN data): the event
/// currency item `source` (dict 951 / 1138 / 1139) is paid `source_amount` times for `target_amount` of `target`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Exchange {
    #[serde(rename = "source_item_dict_id")]
    pub source: u16,
    #[serde(rename = "target_item_dict_id")]
    pub target: u16,
    pub source_amount: u32,
    pub target_amount: u32,
}

/// The event currency exchange table (`48 28 action_item_exchanged`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ExchangeData {
    pub exchange_items: Vec<Exchange>,
}

impl ExchangeData {
    pub fn parse(json: &str) -> Result<ExchangeData, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn find(&self, source: u16, target: u16) -> Option<&Exchange> {
        self.exchange_items.iter().find(|e| e.source == source && e.target == target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> HeroData {
        HeroData::parse(&crate::testing::read("hero_of_the_day_069d.json")).unwrap()
    }

    #[test]
    fn data_is_the_options_file() {
        crate::require_tables!();
        let d = data();
        assert_eq!(d.groups.len(), 1);
        assert_eq!((d.groups[0].min_level, d.groups[0].max_level, d.groups[0].max_player_level), (3, 5, 100));
        assert_eq!((d.recalc_secs, d.operating_secs), (1800, 86400));
        let p: Vec<u32> = d.groups[0].bonuses.iter().map(|b| b.percent()).collect();
        assert_eq!(p, [25, 20, 15]);
        let b = d.bonus(0, 2).unwrap();
        assert_eq!((b.experience, b.silver, b.reputation), (0.2, 0.2, 0.2));
        assert_eq!((b.exp_group, b.grade), (141, 3));
    }

    #[test]
    fn exchange_table_is_the_options_file() {
        crate::require_tables!();
        let e = ExchangeData::parse(&crate::testing::read("event_exchange_069d.json")).unwrap();
        assert_eq!(e.exchange_items.len(), 3);
        let r = e.find(1139, 2200).unwrap();
        assert_eq!((r.source_amount, r.target_amount), (2500, 1));
        assert!(e.find(1139, 2202).is_none());
    }

    #[test]
    fn groups_by_equipment_level() {
        crate::require_tables!();
        let d = data();
        assert_eq!(d.group_of(None), Some(0));
        assert_eq!(d.group_of(Some(3)), Some(0));
        assert_eq!(d.group_of(Some(5)), Some(0));
        assert_eq!(d.group_of(Some(2)), None);
        assert_eq!(d.group_of(Some(6)), None);
        assert!(d.admits(0, 100) && !d.admits(0, 101) && !d.admits(1, 1));
    }

    fn row(account: i64, at: i64, score: u32, ks: u32, won: bool) -> Played {
        Played { account, at, group: 0, score, killstreak: ks, won }
    }

    #[test]
    fn ranks_the_three_nominations() {
        let rows = [
            row(1, 10, 500, 3, true),
            row(2, 20, 900, 1, true),
            row(1, 30, 700, 5, true),
            row(3, 40, 700, 2, false),
            row(2, 50, 100, 0, false),
            row(4, 60, 0, 0, true),
            row(1, 70, 10, 1, true),
        ];
        let ids = |v: Vec<Place>| v.into_iter().map(|p| (p.account, p.value)).collect::<Vec<_>>();
        // best single score: 2 (900), 1 (700 reached first), 3 (700 later); 4 has 0 and does not rank
        assert_eq!(ids(rank(&rows, 0, SCORE)), [(2, 900), (1, 700), (3, 700)]);
        assert_eq!(ids(rank(&rows, 0, KILLSTREAK)), [(1, 5), (3, 2), (2, 1)]);
        // wins in a row: 1 has 3 (the 70 match is its third win), 2 has 1 then loses, 4 has 1, 3 never won
        assert_eq!(ids(rank(&rows, 0, WINSTREAK)), [(1, 3), (2, 1), (4, 1)]);
        // another group is a separate ranking
        assert!(rank(&rows, 1, SCORE).is_empty());
    }
}
