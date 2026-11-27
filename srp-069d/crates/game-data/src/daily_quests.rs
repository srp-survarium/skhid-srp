//! Daily quests: `gameplay/quests/daily_quests.options` and the drop tables of the daily quest chests
//! (`gameplay/drops/chest_drops.options`), extracted by `crates/data-extract` into
//! `data/daily_quests_069d.json`.
//!
//! PROVEN (exe): the client loads exactly this file (0x2002DACB0 table, loader 0x2002DCB30..0x2002DD0A5); the old
//! `quests.options` is not loaded by the 0.69d exe at all. A quest's wire id (AB `dict_id`) is its index in `quests`
//! (client table `[dict + 0x2740] + idx * 0x130`). The reward row is chosen by the highest faction level of the
//! account, capped at 5 (AB reader 0x2001883A0..0x20018840C), as the row with `min_level <= level <= max_level`.
//! Reward amounts per completed quest: type 0 experience, 1 reputation (active faction), 2 silver (UI keys
//! `one_quest_exp_reward` / `rep` / `money`, 0x2001796xx).

use std::collections::BTreeSet;

use serde::Deserialize;

use crate::craft::Roll;
use crate::facts::{MatchFacts, KILL_HEADSHOT, KILL_MELEE};

pub const DAILY_QUESTS_FILE: &str = "daily_quests_069d.json";

#[derive(Debug, Clone, Deserialize)]
pub struct RewardRow {
    pub min_level: u32,
    pub max_level: u32,
    /// item dict id of the chest given for all quests (`daily_quest_chest_N`)
    pub all_completed: u16,
    pub exp: u32,
    pub rep: u32,
    pub silver: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuestDef {
    pub index: u16,
    pub name: String,
    /// file `type`: 0 games, 1 squad games, 3 faction games, 4 game mode, 5 kills with a weapon class, 6 match score,
    /// 7 event kills, 8 killstreak
    #[serde(rename = "type")]
    pub kind: u8,
    pub parameter: u32,
    /// goal (`value_to_reach`)
    pub value: u32,
    pub must_win: bool,
    /// 255 = none: the account must own an item of this category (LIKELY meaning of the client's +0xA byte)
    pub need_item_of_category: u8,
    pub min_faction_level: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChestEntry {
    pub weight: u32,
    pub items: Vec<u16>,
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Deserialize)]
struct Raw {
    lifetime_minutes: u32,
    free_reroll_count: u8,
    reroll_premium_cost: u32,
    rewards: Vec<RewardRow>,
    quests: Vec<QuestDef>,
    chests: std::collections::BTreeMap<String, Vec<ChestEntry>>,
}

/// What an account looks like to the assignment rules.
pub struct Eligibility<'a> {
    pub owned_categories: &'a BTreeSet<u16>,
    pub max_faction_level: u32,
}

#[derive(Debug)]
pub struct DailyQuests {
    pub lifetime_secs: u32,
    pub free_rerolls: u8,
    pub reroll_cost: u32,
    pub rewards: Vec<RewardRow>,
    pub quests: Vec<QuestDef>,
    chests: std::collections::BTreeMap<u16, Vec<ChestEntry>>,
}

/// How a finished match moves a quest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gain {
    /// progress += n
    Add(u32),
    /// progress = max(progress, n) (a best-in-one-match figure)
    Max(u32),
}

impl QuestDef {
    /// How a finished match moves this quest. None = a fact the quest needs is absent (progress must not change);
    /// `Some(Add(0))` = the facts are there and the match does not count.
    ///
    /// Types (file `type`): 0 games, 1 squad games, 3 games of faction `parameter`, 4 games of mode `parameter`
    /// (1 TDM, 2 CTP), 5 kills with a weapon of category `parameter`, 6 match score (best match), 7 event kills
    /// (`parameter` 0 any kill, 13 melee kill, 15 headshot kill, 5 artefact: by the quest names), 8 killstreak (best
    /// match). `must_win_to_complete` makes a lost match not count.
    pub fn gain(&self, f: &MatchFacts) -> Option<Gain> {
        let won_ok = !self.must_win || f.won;
        let one = |c: bool| Some(Gain::Add(u32::from(c && won_ok)));
        let times = |n: Option<u32>| n.map(|n| Gain::Add(if won_ok { n } else { 0 }));
        match self.kind {
            0 => one(true),
            1 => one(f.in_squad?),
            3 => one(u32::from(f.faction) == self.parameter),
            4 => one(f.mode == self.parameter),
            5 => times(f.count_kills(|k| k.weapon_category.map(|c| u32::from(c) == self.parameter))),
            6 => f.score.map(|s| Gain::Max(if won_ok { s } else { 0 })),
            // the parameter is the event id of the world's matrix (0 kill, 13 melee_kill, 15 headshot_kill, 5 artefact:
            // the quest names, PROVEN against the table 0x200986E10): with the world's row the count is that row
            7 if f.stat_events.is_some() => times(f.stat(self.parameter as u16)),
            7 => match self.parameter {
                0 => times(f.kills.as_ref().map(|k| k.len() as u32)),
                13 => times(f.count_kills(|k| k.has_flag(KILL_MELEE))),
                15 => times(f.count_kills(|k| k.has_flag(KILL_HEADSHOT))),
                5 => times(f.artefacts),
                _ => None,
            },
            8 => f.max_killstreak.map(|s| Gain::Max(if won_ok { s } else { 0 })),
            _ => None,
        }
    }
}

impl DailyQuests {
    pub fn parse(json: &str) -> Result<DailyQuests, serde_json::Error> {
        let r: Raw = serde_json::from_str(json)?;
        Ok(DailyQuests {
            lifetime_secs: r.lifetime_minutes * 60,
            free_rerolls: r.free_reroll_count,
            reroll_cost: r.reroll_premium_cost,
            rewards: r.rewards,
            quests: r.quests,
            chests: r.chests.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect(),
        })
    }

    pub fn quest(&self, index: u16) -> Option<&QuestDef> {
        self.quests.get(usize::from(index))
    }

    /// The reward row for a faction level (the client's rule; the first row when none matches).
    pub fn reward_row(&self, max_faction_level: u32) -> &RewardRow {
        let level = max_faction_level.min(5);
        self.rewards.iter().find(|r| r.min_level <= level && level <= r.max_level).unwrap_or(&self.rewards[0])
    }

    /// Quests an account may be given (data rules: needed item category, minimum faction level).
    pub fn eligible(&self, e: &Eligibility) -> Vec<u16> {
        self.quests
            .iter()
            .filter(|q| q.need_item_of_category == 255 || e.owned_categories.contains(&u16::from(q.need_item_of_category)))
            .filter(|q| u32::from(q.min_faction_level) <= e.max_faction_level)
            .map(|q| q.index)
            .collect()
    }

    /// One reward of a chest: weighted entry (`drop_weight`), then one of the entry's item ids at random, amount in
    /// the entry's range. Returns (item dict id, amount).
    pub fn draw_chest(&self, chest: u16, roll: &mut dyn Roll) -> Option<(u16, u32)> {
        let rows = self.chests.get(&chest)?;
        let total: u32 = rows.iter().map(|r| r.weight).sum();
        if total == 0 {
            return None;
        }
        let mut pick = roll.below(total);
        let row = rows.iter().find(|r| {
            if pick < r.weight {
                true
            } else {
                pick -= r.weight;
                false
            }
        })?;
        let item = *row.items.get(roll.below(row.items.len().max(1) as u32) as usize)?;
        let amount = row.min + roll.below(row.max.saturating_sub(row.min) + 1);
        Some((item, amount))
    }

    pub fn has_chest(&self, chest: u16) -> bool {
        self.chests.contains_key(&chest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::SplitMix;
    use crate::facts::{KillFact, KILL_HEADSHOT, KILL_MELEE};

    fn dq() -> DailyQuests {
        DailyQuests::parse(&crate::testing::read("daily_quests_069d.json")).unwrap()
    }

    #[test]
    fn file_matches_the_options_file() {
        crate::require_tables!();
        let d = dq();
        assert_eq!((d.lifetime_secs, d.free_rerolls, d.reroll_cost), (1080 * 60, 1, 30));
        assert_eq!(d.quests.len(), 29);
        assert_eq!(d.rewards.len(), 5);
        assert_eq!((d.rewards[4].exp, d.rewards[4].rep, d.rewards[4].silver, d.rewards[4].all_completed), (500, 500, 1500, 1233));
    }

    #[test]
    fn reward_row_follows_the_capped_faction_level() {
        crate::require_tables!();
        let d = dq();
        assert_eq!(d.reward_row(0).all_completed, 1157);
        assert_eq!(d.reward_row(1).all_completed, 1157);
        assert_eq!(d.reward_row(2).all_completed, 1158);
        assert_eq!(d.reward_row(4).all_completed, 1160);
        assert_eq!(d.reward_row(5).all_completed, 1233);
        assert_eq!(d.reward_row(15).all_completed, 1233);
    }

    fn by<'a>(d: &'a DailyQuests, n: &str) -> &'a QuestDef {
        d.quests.iter().find(|q| q.name == n).unwrap()
    }

    fn kill(cat: u16, flags: u8) -> KillFact {
        KillFact { weapon_category: Some(cat), kill_flags: Some(flags), ..KillFact::default() }
    }

    #[test]
    fn counting_rules_with_the_facts_of_the_local_path() {
        crate::require_tables!();
        let d = dq();
        let win = MatchFacts::local(4, 1, true, true, 2);
        let loss = MatchFacts::local(4, 2, false, true, 2);
        assert_eq!(by(&d, "st_daily_quest_win_games").gain(&win), Some(Gain::Add(1)));
        assert_eq!(by(&d, "st_daily_quest_win_games").gain(&loss), Some(Gain::Add(0)));
        assert_eq!(by(&d, "st_daily_quest_play_games").gain(&loss), Some(Gain::Add(1)));
        assert_eq!(by(&d, "st_daily_quest_win_games_faction_blackmarket").gain(&win), Some(Gain::Add(1)));
        assert_eq!(by(&d, "st_daily_quest_win_games_faction_edge").gain(&win), Some(Gain::Add(0)));
        assert_eq!(by(&d, "st_daily_quest_play_game_mode_capture_the_point").gain(&loss), Some(Gain::Add(1)));
        assert_eq!(by(&d, "st_daily_quest_win_game_mode_team_death_match").gain(&loss), Some(Gain::Add(0)));
        // facts the local path does not have: nothing moves
        for n in [
            "st_daily_quest_win_games_squad",
            "st_daily_quest_kill_with_weapon_class_sniper",
            "st_daily_quest_match_score",
            "st_daily_quest_killstreak_length",
            "st_daily_quest_event_kill",
            "st_daily_quest_event_melee_kill",
            "st_daily_quest_event_headshot_kill",
            "st_daily_quest_event_artefact",
        ] {
            assert_eq!(by(&d, n).gain(&win), None, "{n}");
        }
    }

    #[test]
    fn counting_rules_with_full_facts() {
        crate::require_tables!();
        let d = dq();
        let mut f = MatchFacts::local(4, 1, true, true, 1);
        f.in_squad = Some(true);
        f.kills = Some(vec![kill(13, KILL_HEADSHOT), kill(13, 0), kill(8, KILL_MELEE), kill(17, KILL_HEADSHOT | KILL_MELEE)]);
        f.score = Some(140);
        f.max_killstreak = Some(3);
        f.artefacts = Some(2);
        assert_eq!(by(&d, "st_daily_quest_win_games_squad").gain(&f), Some(Gain::Add(1)));
        assert_eq!(by(&d, "st_daily_quest_kill_with_weapon_class_sniper").gain(&f), Some(Gain::Add(2)));
        assert_eq!(by(&d, "st_daily_quest_kill_with_weapon_class_assault").gain(&f), Some(Gain::Add(1)));
        assert_eq!(by(&d, "st_daily_quest_kill_with_weapon_class_lmg").gain(&f), Some(Gain::Add(0)));
        assert_eq!(by(&d, "st_daily_quest_match_score").gain(&f), Some(Gain::Max(140)));
        assert_eq!(by(&d, "st_daily_quest_killstreak_length").gain(&f), Some(Gain::Max(3)));
        assert_eq!(by(&d, "st_daily_quest_event_kill").gain(&f), Some(Gain::Add(4)));
        assert_eq!(by(&d, "st_daily_quest_event_melee_kill").gain(&f), Some(Gain::Add(2)));
        assert_eq!(by(&d, "st_daily_quest_event_headshot_kill").gain(&f), Some(Gain::Add(2)));
        assert_eq!(by(&d, "st_daily_quest_event_artefact").gain(&f), Some(Gain::Add(2)));
        // no squad / observed zero kills are facts too
        f.in_squad = Some(false);
        f.kills = Some(vec![]);
        assert_eq!(by(&d, "st_daily_quest_play_games_squad").gain(&f), Some(Gain::Add(0)));
        assert_eq!(by(&d, "st_daily_quest_event_kill").gain(&f), Some(Gain::Add(0)));
        // a kill whose weapon is unknown is not counted as a weapon-class kill
        f.kills = Some(vec![KillFact::default()]);
        assert_eq!(by(&d, "st_daily_quest_kill_with_weapon_class_sniper").gain(&f), Some(Gain::Add(0)));
        assert_eq!(by(&d, "st_daily_quest_event_kill").gain(&f), Some(Gain::Add(1)));
    }

    #[test]
    fn event_quests_read_the_worlds_event_row() {
        crate::require_tables!();
        let d = dq();
        let mut f = MatchFacts::local(4, 1, false, true, 1);
        // the world's row (protocol 3): 7 kills, 2 melee, 3 headshots, 1 artefact; no kill list at all
        f.stat_events = Some([(0, 7), (13, 2), (15, 3), (5, 1)].into());
        assert_eq!(by(&d, "st_daily_quest_event_kill").gain(&f), Some(Gain::Add(7)));
        assert_eq!(by(&d, "st_daily_quest_event_melee_kill").gain(&f), Some(Gain::Add(2)));
        assert_eq!(by(&d, "st_daily_quest_event_headshot_kill").gain(&f), Some(Gain::Add(3)));
        assert_eq!(by(&d, "st_daily_quest_event_artefact").gain(&f), Some(Gain::Add(1)));
        // weapon-class quests still need the kill list
        assert_eq!(by(&d, "st_daily_quest_kill_with_weapon_class_sniper").gain(&f), None);
        // the row wins over a kill list
        f.kills = Some(vec![kill(13, 0)]);
        assert_eq!(by(&d, "st_daily_quest_event_kill").gain(&f), Some(Gain::Add(7)));
        f.stat_events = Some(Default::default());
        assert_eq!(by(&d, "st_daily_quest_event_headshot_kill").gain(&f), Some(Gain::Add(0)), "an empty row is a fact: none");
    }

    #[test]
    fn category_quests_need_an_owned_item() {
        crate::require_tables!();
        let d = dq();
        let none = BTreeSet::new();
        let e = Eligibility { owned_categories: &none, max_faction_level: 5 };
        assert_eq!(d.eligible(&e).len(), 29 - 7);
        let have: BTreeSet<u16> = [8].into();
        let e = Eligibility { owned_categories: &have, max_faction_level: 5 };
        assert_eq!(d.eligible(&e).len(), 29 - 6);
    }

    #[test]
    fn chest_draw_is_weighted_and_inside_the_table() {
        crate::require_tables!();
        let d = dq();
        let mut r = SplitMix(7);
        let mut seen = BTreeSet::new();
        for _ in 0..2000 {
            let (item, amount) = d.draw_chest(1233, &mut r).unwrap();
            assert!(amount >= 1);
            seen.insert(item);
        }
        assert!(seen.contains(&214) && seen.contains(&213) && seen.contains(&1164) && seen.contains(&925));
        assert!(d.draw_chest(1, &mut r).is_none());
        // money rows pay the fixed amount of the chest
        let silver: Vec<u32> = (0..3000).filter_map(|_| d.draw_chest(1233, &mut r)).filter(|x| x.0 == 214).map(|x| x.1).collect();
        assert!(silver.iter().all(|&a| a == 5000));
    }
}
