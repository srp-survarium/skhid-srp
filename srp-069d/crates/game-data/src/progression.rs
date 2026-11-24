//! Progression data of 0.69d: the new-player kit, the faction level table and the event rewards of a match.
//!
//! Source `data/progression_069d.json` (made by `crates/data-extract`):
//! * `new_player` = `gameplay/items/items_for_new_player.options` (volume 38): rows `{dict_id, buy_amount, wear_amount,
//!   slot, faction_id, profiles_to_wear}`; a row with a `faction_id` is only for that faction;
//! * `faction_levels` = `gameplay/reputation_leveling.options`: points at which each faction level starts;
//! * `events` = `gameplay/awards/events.options` (volume 54): per match event (kill, assist, ...) the score, experience,
//!   silver ("money") and reputation it pays, with rows for single game modes that replace the base row;
//! * `tutorial_awards` = `gameplay/awards/unique_awards.options`.
//!
//! Match rewards (docs/notes/progression-audit.md section 4): the experience, silver and reputation of a match are the
//! SUM of the event rows of the player's events, times the match level factors (`match_levels_factors`), plus the
//! silver roll of `drop_after_combat`. Facts the server does not have (`MatchFacts` field `None`) contribute nothing.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::craft::Roll;
use crate::facts::{MatchFacts, KILL_HEADSHOT, KILL_MELEE};
use crate::{GameData, LevelFactor};

pub const FILE: &str = "progression_069d.json";

/// The normal length of a PvP match: `match_time` 900 s of the game-mode files (327 of them; PROVEN data). The 0.62b
/// short-match thresholds below are minutes of such a match.
pub const NORMAL_MATCH_SECS: u32 = 900;

/// Reward factor of a match that lasted `secs` (OFFICIAL 0.62, 2020-03-04: "The reward for prematurely ended match is
/// proportional to the duration of the match"; 0.62b: "less than 5 minutes: the reward is multiplied by 3, 5-10
/// minutes - multiplied by 2, 10-13 minutes - multiplied by 1.5"; still the rule in 0.69d). A match that ran its full
/// time pays x1. CHOICE: the proportion first, then the multiplier (how they combine is not published).
pub fn short_match_factor(secs: u32) -> f64 {
    if secs >= NORMAL_MATCH_SECS {
        return 1.0;
    }
    let part = f64::from(secs) / f64::from(NORMAL_MATCH_SECS);
    let mult = match secs {
        s if s < 5 * 60 => 3.0,
        s if s < 10 * 60 => 2.0,
        s if s < 13 * 60 => 1.5,
        _ => 1.0,
    };
    part * mult
}

/// One row of `items_for_new_player.options`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct KitRow {
    pub dict_id: u16,
    /// units bought into the inventory
    pub buy_amount: u32,
    /// units worn / loaded (rounds of an ammo stack); 0 = not worn
    pub wear_amount: u32,
    pub slot: Option<u8>,
    /// only for this faction (1..=4)
    pub faction_id: Option<u8>,
    /// the player profiles (0..=2) that wear it
    pub profiles_to_wear: Vec<u8>,
}

/// Reward of one event (all four are signed: a team kill costs score and experience).
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
pub struct EventRow {
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub experience: i32,
    #[serde(default)]
    pub money: i32,
    #[serde(default)]
    pub reputation: i32,
    /// the event value that counts as one full reward (100 for the objective events, 1 for the rest; LIKELY: the paid
    /// amount is `row * value / reward_factor`)
    #[serde(default = "one")]
    pub reward_factor: i32,
}

fn one() -> i32 {
    1
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventEntry {
    pub base: EventRow,
    /// per game mode name (`tdm`, `capt`, `gvi`, `sad`, `pb`, `slaughter`, `gunrace`)
    pub modes: BTreeMap<String, EventRow>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub struct Award {
    pub amount: u32,
    pub param: u32,
    #[serde(rename = "type")]
    pub kind: u32,
}

#[derive(Debug, Default, Deserialize)]
pub struct Progression {
    pub new_player: Vec<KitRow>,
    /// faction id (as text) -> thresholds, index = level (two zero entries: level 0 and 1)
    pub faction_levels: BTreeMap<String, Vec<u32>>,
    pub events: BTreeMap<String, EventEntry>,
    pub tutorial_awards: BTreeMap<String, Vec<Award>>,
    /// `(modification group, grade, component, fraction)`; component 0 experience, 1 silver, 2 reputation. Items carry
    /// them in `default_modifications` (decals, premium gear), workshop rolls as (group, grade) records.
    #[serde(default)]
    pub reward_modifiers: Vec<(u32, u32, u8, f64)>,
    /// `(group, grade, fraction)` of the spare-parts bonus `details_bonus_modifier` (group 147: +5 % .. +100 % of the
    /// match's spare-parts drop; 21 decals, 22 weapons and 8 decal sets carry it).
    #[serde(default)]
    pub parts_modifiers: Vec<(u32, u32, f64)>,
    /// `(group, grade, faction id, fraction)` of the faction reputation bonus (groups 125..128, lobby_info
    /// `scav_rep_booster` / `bm_rep_booster` / `ren_rep_booster` / `edge_rep_booster` = factions 1..4): a reputation
    /// bonus that counts only for reputation earned with that faction (12 camouflages / decals carry one).
    #[serde(default)]
    pub faction_reputation_modifiers: Vec<(u32, u32, u8, f64)>,
}

/// Experience, silver and reputation of one match (before premium and booster percentages).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reward {
    pub exp: u32,
    pub silver: u32,
    pub rep: u32,
}

/// Game mode id -> the key of the per-mode rows of `events.options`. `tdm` = mode 1 (PROVEN: `level_01_day_tdm.options`
/// `mode_name "tdm"`, `mode 1`), `capt` = mode 2 (LIKELY: capture the point). The other modes have no PROVEN key: the
/// base row is used.
pub fn mode_key(mode: u32) -> Option<&'static str> {
    match mode {
        1 => Some("tdm"),
        2 => Some("capt"),
        // the exe picks an events.options row's per-mode block by its mode table (0x2002E0741: `table[mode]`, table
        // 0x200987060 entry 3 = `gunrace`): PROVEN
        3 => Some("gunrace"),
        _ => None,
    }
}

impl Progression {
    pub fn parse(json: &str) -> Result<Progression, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// The kit rows a new account of `faction` gets: the rows without a faction and those of `faction`.
    pub fn kit_for(&self, faction: u8) -> impl Iterator<Item = &KitRow> {
        self.new_player.iter().filter(move |r| r.faction_id.is_none_or(|f| f == faction))
    }

    /// Row of an event for a game mode (the mode's own row when it has one).
    pub fn event_row(&self, name: &str, mode: u32) -> Option<EventRow> {
        let e = self.events.get(name)?;
        Some(mode_key(mode).and_then(|k| e.modes.get(k)).copied().unwrap_or(e.base))
    }

    /// Reward bonus (fractions `[experience, silver, reputation]`) of a set of (group, grade) modifications.
    pub fn reward_bonus(&self, mods: impl IntoIterator<Item = (u32, u32)>) -> [f64; 3] {
        let mut out = [0.0; 3];
        for (g, gr) in mods {
            for &(mg, mgr, c, v) in &self.reward_modifiers {
                if mg == g && mgr == gr {
                    out[c as usize] += v;
                }
            }
        }
        out
    }

    /// [`Self::reward_bonus`] for a player whose match reputation goes to `faction`: also the faction reputation pairs
    /// (groups 125..128) of that faction; the other factions' pairs give nothing.
    pub fn reward_bonus_for(&self, mods: impl IntoIterator<Item = (u32, u32)>, faction: u8) -> [f64; 3] {
        let mods: Vec<(u32, u32)> = mods.into_iter().collect();
        let mut out = self.reward_bonus(mods.iter().copied());
        for (g, gr) in mods {
            out[2] += self.faction_reputation_modifiers.iter().filter(|m| m.0 == g && m.1 == gr && m.2 == faction).map(|m| m.3).sum::<f64>();
        }
        out
    }

    /// Spare-parts bonus (fraction of the drop) of a set of (group, grade) modifications.
    pub fn parts_bonus(&self, mods: impl IntoIterator<Item = (u32, u32)>) -> f64 {
        let mut out = 0.0;
        for (g, gr) in mods {
            out += self.parts_modifiers.iter().filter(|&&(mg, mgr, _)| mg == g && mgr == gr).map(|m| m.2).sum::<f64>();
        }
        out
    }

    /// Thresholds of a faction (empty for an unknown faction).
    pub fn faction_thresholds(&self, faction: u8) -> &[u32] {
        self.faction_levels.get(&faction.to_string()).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// Count of each event the facts show. A fact that is `None` gives no count (the event is not guessed).
///
/// * `kill`: one per kill; `headshot_kill`, `melee_kill`, `grenade_kill`, `trap_kill`: one more per kill of that kind
///   (LIKELY: the extra rows are bonus events on top of `kill`, as the score table lists them separately);
/// * `assist`: `stat_events[1]` (the achievement 204 key); `death`: `deaths`.
///
/// With the world's whole event row (`stat_events`, world host protocol 3) every event counts under its
/// `events.options` name ([`crate::facts::event_row_name`]; PROVEN by the kill handler 0x20033DCF0 that the kill-kind rows
/// are extra events on top of `kill`), except `participating` / `won` / `played_first_game`: the world adds those only
/// at its match end, so the lobby's own `first_game` decides (and `participating` / `won` have no reward row).
pub fn event_counts(f: &MatchFacts) -> BTreeMap<&'static str, u32> {
    use crate::facts::ev;
    let mut c: BTreeMap<&'static str, u32> = BTreeMap::new();
    if let Some(rows) = &f.stat_events {
        for (&e, &n) in rows {
            if n == 0 || matches!(e, ev::PARTICIPATING | ev::WON | ev::PLAYED_FIRST_GAME) {
                continue;
            }
            if let Some(name) = crate::facts::event_row_name(e) {
                c.insert(name, n);
            }
        }
        if f.first_game && f.finished {
            c.insert("played_first_game", 1);
        }
        return c;
    }
    if let Some(kills) = &f.kills {
        c.insert("kill", kills.len() as u32);
        for k in kills {
            if k.has_flag(KILL_HEADSHOT) == Some(true) {
                *c.entry("headshot_kill").or_default() += 1;
            }
            if k.has_flag(KILL_MELEE) == Some(true) {
                *c.entry("melee_kill").or_default() += 1;
            }
            match k.inflictor_type {
                Some(1) => *c.entry("grenade_kill").or_default() += 1,
                Some(2) => *c.entry("trap_kill").or_default() += 1,
                _ => {}
            }
        }
    }
    if f.first_game && f.finished {
        c.insert("played_first_game", 1);
    }
    if let Some(a) = f.stat(1) {
        c.insert("assist", a);
    }
    if let Some(d) = f.deaths {
        c.insert("death", d);
    }
    c
}

/// The parts of a match [`Reward`] the `97` result window shows in its three reward rows:
/// one row per `events.options` `ui_category`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RewardParts {
    /// per category (0 kills, 1 help, 2 the rest): its share of the events' experience / silver / reputation; the three
    /// add up to the reward's event part
    pub rows: [Reward; 3],
    /// per category: the count of its paying events (the row's amount; the sample's kills row shows the kill count)
    pub amounts: [u32; 3],
    /// the `drop_after_combat` silver roll inside `Reward::silver` (no event row)
    pub drop_silver: u32,
}

/// Split `total` over buckets in proportion to the positive `weights` (largest remainder; equal remainders go to the
/// lower index), so the parts add up to `total` exactly. Non-positive weights get 0; when no weight is positive the
/// whole total goes to the last bucket.
pub fn apportion(total: u32, weights: &[i64]) -> Vec<u32> {
    let n = weights.len();
    let mut out = vec![0u32; n];
    if n == 0 || total == 0 {
        return out;
    }
    let pos: Vec<u128> = weights.iter().map(|w| (*w).max(0) as u128).collect();
    let sum: u128 = pos.iter().sum();
    if sum == 0 {
        out[n - 1] = total;
        return out;
    }
    let mut rem: Vec<(u128, usize)> = Vec::with_capacity(n);
    let mut given = 0u32;
    for (i, w) in pos.iter().enumerate() {
        let q = u128::from(total) * w;
        out[i] = (q / sum) as u32;
        given += out[i];
        rem.push((q % sum, i));
    }
    rem.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, i) in rem.into_iter().filter(|(_, i)| pos[*i] > 0).take((total - given) as usize) {
        out[i] += 1;
    }
    out
}

/// Match level factors row for a match level (LIKELY: the row index is the match level; the table has 6 rows for the
/// levels 0..=5 of `drop_after_combat`).
pub fn level_factor(factors: &[LevelFactor], match_level: u32) -> LevelFactor {
    factors
        .get(match_level as usize)
        .or_else(|| factors.last())
        .copied()
        .unwrap_or(LevelFactor { reputation_factor: 1.0, experience_factor: 1.0, silver_factor: 1.0 })
}

impl GameData {
    /// Experience, silver and reputation a finished match pays to the player of `f` (a leaver or an unfinished match
    /// pays nothing: official 2022-01-27).
    ///
    /// silver = sum(event money) * silver_factor(match level) + the `drop_after_combat` silver roll (100..200 at level 1
    /// ... 500..1000 at level 5, no factor: CHOICE, nothing says the factor applies to the drop).
    /// experience / reputation = sum(event rows) * experience_factor / reputation_factor.
    /// Without kills / assists / deaths in `f` only the silver roll is paid.
    pub fn match_reward(&self, f: &MatchFacts, match_level: u32, rng: &mut dyn Roll) -> Reward {
        self.match_reward_parts(f, match_level, rng).0
    }

    /// [`GameData::match_reward`] with its parts for the result window's rows ([`RewardParts`]): the event amounts per
    /// `ui_category` (the level factor applied to each category's share of the total) and the silver roll. The rows add up
    /// to the total exactly ([`apportion`]).
    pub fn match_reward_parts(&self, f: &MatchFacts, match_level: u32, rng: &mut dyn Roll) -> (Reward, RewardParts) {
        self.match_reward_parts_with(f, match_level, None, rng)
    }

    /// [`GameData::match_reward_parts`] with the events' silver factor given (`None` = `match_levels_factors` of the data);
    /// experience, reputation, the drop roll and everything else as the data says.
    pub fn match_reward_parts_with(&self, f: &MatchFacts, match_level: u32, silver_factor: Option<f64>, rng: &mut dyn Roll) -> (Reward, RewardParts) {
        if !f.finished {
            return Default::default();
        }
        let (mut exp, mut money, mut rep) = (0i64, 0i64, 0i64);
        // per category: [experience, money, reputation] before the level factor
        let mut raw = [[0i64; 3]; 3];
        let mut amounts = [0u32; 3];
        for (name, n) in event_counts(f) {
            if let Some(r) = self.progression.event_row(name, f.mode) {
                let (e, m, p) = (
                    r.experience as i64 * n as i64 / r.reward_factor as i64,
                    r.money as i64 * n as i64 / r.reward_factor as i64,
                    r.reputation as i64 * n as i64 / r.reward_factor as i64,
                );
                exp += e;
                money += m;
                rep += p;
                let c = usize::from(self.event_categories.of(name));
                raw[c][0] += e;
                raw[c][1] += m;
                raw[c][2] += p;
                if r.experience != 0 || r.money != 0 || r.reputation != 0 {
                    amounts[c] = amounts[c].saturating_add(n);
                }
            }
        }
        let mut lf = level_factor(&self.match_level_factors, match_level);
        if let Some(k) = silver_factor {
            lf.silver_factor = k;
        }
        // the gun race pays no level factor: the client's own reward computation takes 1.0 for all three in mode 3
        // (0x200332080, PROVEN for the badge awards; that the event reward followed it: LIKELY), and
        // the official notes (Guns Race 0.61b / 0.65d / 0.69b): "The list of rewards does not depend on your maximum
        // equipment tier". The server's silver factor setting does not apply either.
        if f.mode == crate::gunrace::MODE {
            lf = LevelFactor { reputation_factor: 1.0, experience_factor: 1.0, silver_factor: 1.0 };
        }
        let scale = |v: i64, k: f64| ((v.max(0) as f64) * k).round() as u32;
        let base = self.drops.base_silver(match_level, rng);
        let (e, m, p) = (scale(exp, lf.experience_factor), scale(money, lf.silver_factor), scale(rep, lf.reputation_factor));
        let col = |k: usize| [raw[0][k], raw[1][k], raw[2][k]];
        let (ex, si, re) = (apportion(e, &col(0)), apportion(m, &col(1)), apportion(p, &col(2)));
        let rows = [0, 1, 2].map(|c| Reward { exp: ex[c], silver: si[c], rep: re[c] });
        (Reward { exp: e, silver: m.saturating_add(base), rep: p }, RewardParts { rows, amounts, drop_silver: base })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::SplitMix;
    use crate::facts::KillFact;

    fn gd() -> GameData {
        crate::testing::tables()
    }

    #[test]
    fn short_matches_pay_by_duration_times_the_0_62b_multiplier() {
        assert_eq!(short_match_factor(900), 1.0);
        assert_eq!(short_match_factor(2000), 1.0);
        assert!((short_match_factor(240) - 240.0 / 900.0 * 3.0).abs() < 1e-9);
        assert!((short_match_factor(450) - 0.5 * 2.0).abs() < 1e-9);
        assert!((short_match_factor(720) - 0.8 * 1.5).abs() < 1e-9);
        assert!((short_match_factor(840) - 840.0 / 900.0).abs() < 1e-9);
        assert_eq!(short_match_factor(0), 0.0);
    }

    #[test]
    fn kit_is_the_data_kit() {
        crate::require_tables!();
        let g = gd();
        let all: Vec<_> = g.progression.kit_for(1).collect();
        // 12 faction-free rows + the 3 armour rows of faction 1 (data rows 0..=17 and the faction-1 armour)
        assert!(all.iter().any(|r| r.dict_id == 107 && r.profiles_to_wear == vec![1]));
        assert!(all.iter().any(|r| r.dict_id == 31 && r.faction_id == Some(1)));
        assert!(!all.iter().any(|r| r.dict_id == 29), "faction 2 armour is not for faction 1");
        assert_eq!(g.progression.new_player.len(), 30);
    }

    #[test]
    fn item_modifications_resolve_to_reward_bonuses() {
        crate::require_tables!();
        let g = gd();
        // (124, 6) = experience +2 % (decal char_decal_5_29), (139, 0) = reputation +10 %, (123, 2) = silver +50 %
        let b = g.progression.reward_bonus([(124, 6), (139, 0), (123, 2)]);
        assert!((b[0] - 0.02).abs() < 1e-9 && (b[1] - 0.5).abs() < 1e-9 && (b[2] - 0.1).abs() < 1e-9, "{b:?}");
        assert_eq!(g.progression.reward_bonus([(173, 1)]), [0.0; 3], "pocket groups are not reward bonuses");
        assert_eq!(g.progression.reward_bonus([(147, 1)]), [0.0; 3], "the spare-parts group is not a reward component");
        assert!((g.progression.parts_bonus([(147, 1), (147, 7), (124, 6)]) - 0.15).abs() < 1e-9);
        assert_eq!(g.progression.parts_bonus([(148, 0)]), 0.0, "group 148 has no carrier and no proven meaning");
    }

    #[test]
    fn faction_table_is_the_data() {
        crate::require_tables!();
        let g = gd();
        for f in 1..=4u8 {
            let t = g.progression.faction_thresholds(f);
            assert_eq!(t, &crate::levels::FACTION_THRESHOLDS[..], "faction {f}");
        }
    }

    #[test]
    fn tdm_kill_uses_the_mode_row() {
        crate::require_tables!();
        let g = gd();
        assert_eq!(g.progression.event_row("kill", 1).unwrap(), EventRow { score: 15, experience: 9, money: 45, reputation: 5, reward_factor: 1 });
        assert_eq!(g.progression.event_row("kill", 5).unwrap().money, 30, "other modes: base row");
    }

    #[test]
    fn reward_sums_events_and_scales_by_match_level() {
        crate::require_tables!();
        let g = gd();
        let mut f = MatchFacts::local(1, 1, true, true, 1);
        f.kills = Some(vec![KillFact::default(); 10]);
        f.deaths = Some(3);
        // the world's event row (protocol 3): 10 kills, 4 assists, 3 deaths; the kill list is not read any more
        f.stat_events = Some([(0u16, 10u32), (1, 4), (2, 3)].into_iter().collect());
        // level 1 row: silver x1, experience x1, reputation x4; tdm kill 9/45/5, assist 10/30/5
        let r = g.match_reward(&f, 1, &mut SplitMix(1));
        let base = r.silver - (10 * 45 + 4 * 30);
        assert!((100..=200).contains(&base), "base silver {base}");
        assert_eq!((r.exp, r.rep), (10 * 9 + 4 * 10, 4 * (10 * 5 + 4 * 5)));
        // level 3 (row 3): silver x3, experience x1.25, reputation x4
        let r3 = g.match_reward(&f, 3, &mut SplitMix(1));
        assert_eq!(r3.exp, ((10 * 9 + 4 * 10) as f64 * 1.25).round() as u32);
    }

    #[test]
    fn apportion_adds_up_and_follows_the_weights() {
        assert_eq!(apportion(10, &[1, 1, 0]), vec![5, 5, 0]);
        assert_eq!(apportion(10, &[1, 1, 1]), vec![4, 3, 3], "largest remainder, lower index first");
        assert_eq!(apportion(7, &[3, -2, 0, 1]), vec![5, 0, 0, 2], "non-positive weights get nothing");
        assert_eq!(apportion(5, &[0, 0]), vec![0, 5], "no positive weight: the last bucket");
        assert_eq!(apportion(0, &[1, 2]), vec![0, 0]);
        for t in [1u32, 99, 1001, 65_535] {
            assert_eq!(apportion(t, &[13, 7, 29, 1]).iter().sum::<u32>(), t);
        }
    }

    /// The result window's rows: the event parts per `ui_category` add up to the reward, the kills row counts the kill
    /// events (deaths pay nothing and are not counted), the silver roll is apart.
    #[test]
    fn the_reward_parts_split_the_events_by_category() {
        crate::require_tables!();
        let g = gd();
        let mut f = MatchFacts::local(1, 1, true, true, 1);
        // tdm: 5 kills, 2 assists, 3 deaths, 1 headshot (category 2)
        f.stat_events = Some([(0u16, 5u32), (1, 2), (2, 3), (15, 1)].into_iter().collect());
        let (r, p) = g.match_reward_parts(&f, 3, &mut SplitMix(9));
        assert_eq!(r, g.match_reward(&f, 3, &mut SplitMix(9)), "the same reward");
        assert_eq!(p.amounts, [5, 2, 1]);
        assert_eq!(p.rows.iter().map(|x| x.exp).sum::<u32>(), r.exp);
        assert_eq!(p.rows.iter().map(|x| x.rep).sum::<u32>(), r.rep);
        assert_eq!(p.rows.iter().map(|x| x.silver).sum::<u32>() + p.drop_silver, r.silver);
        assert!(p.drop_silver > 0);
        // kills 5 x 45 vs assists 2 x 30 vs headshot: the kills row has the largest share
        assert!(p.rows[0].silver > p.rows[1].silver && p.rows[1].silver > 0 && p.rows[2].silver > 0);
    }

    /// A given silver factor replaces only the events' silver factor of the level: experience, reputation and the drop
    /// roll stay the data's.
    #[test]
    fn a_given_silver_factor_scales_only_the_events_silver() {
        crate::require_tables!();
        let g = gd();
        let mut f = MatchFacts::local(1, 1, true, true, 1);
        f.stat_events = Some([(0u16, 4u32), (15, 2)].into_iter().collect()); // tdm: 4 kills (45), 2 headshots (15)
        let (d, dp) = g.match_reward_parts(&f, 5, &mut SplitMix(3));
        let (s, sp) = g.match_reward_parts_with(&f, 5, Some(14.0), &mut SplitMix(3));
        assert_eq!((s.exp, s.rep, sp.drop_silver), (d.exp, d.rep, dp.drop_silver));
        assert_eq!(d.silver - dp.drop_silver, 210 * 4, "the data's level-5 factor 4");
        assert_eq!(s.silver - sp.drop_silver, 210 * 14);
        assert_eq!(g.match_reward_parts_with(&f, 5, None, &mut SplitMix(3)), (d, dp));
    }

    #[test]
    fn the_worlds_event_row_pays_every_named_event() {
        crate::require_tables!();
        let g = gd();
        let mut f = MatchFacts::local(1, 2, true, true, 1);
        // ctp (mode 2): 3 kills, 1 headshot on top, 150 capture points (amount; reward_factor 100), team kill, and the
        // match-end rows the lobby replaces (participating / won / played_first_game)
        f.stat_events = Some([(0u16, 3u32), (15, 1), (29, 150), (4, 1), (11, 1), (12, 1), (27, 1), (42, 2)].into_iter().collect());
        let c = event_counts(&f);
        assert_eq!(
            c.into_iter().collect::<Vec<_>>(),
            vec![("capture_the_point", 150), ("headshot_kill", 1), ("kill", 3), ("match_state_event_pve_open_chest", 2), ("team_kill", 1)]
        );
        f.first_game = true;
        assert_eq!(event_counts(&f).get("played_first_game"), Some(&1), "the lobby's own first game");
        f.first_game = false;
        // level 0 factors (silver 1, experience 1, reputation 2): the sum of the rows of mode 2
        let r = g.match_reward(&f, 0, &mut SplitMix(3));
        let row = |n: &str| g.progression.event_row(n, 2).unwrap();
        let sum = |pick: fn(&EventRow) -> i32| -> i64 {
            [("kill", 3), ("headshot_kill", 1), ("capture_the_point", 150), ("team_kill", 1), ("match_state_event_pve_open_chest", 2)]
                .iter()
                .map(|(n, k)| {
                    let r = row(n);
                    i64::from(pick(&r)) * k / i64::from(r.reward_factor)
                })
                .sum()
        };
        assert_eq!(r.exp, sum(|r| r.experience).max(0) as u32);
        assert_eq!(r.rep, (sum(|r| r.reputation).max(0) as f64 * 2.0).round() as u32);
        assert!(r.silver >= sum(|r| r.money) as u32, "the event silver plus the drop roll");
    }

    #[test]
    fn leavers_and_unknown_facts_pay_what_the_data_says() {
        crate::require_tables!();
        let g = gd();
        let left = MatchFacts::local(1, 1, false, false, 1);
        assert_eq!(g.match_reward(&left, 1, &mut SplitMix(1)), Reward::default());
        // finished, but no kills / assists / deaths known: only the base silver roll
        let f = MatchFacts::local(1, 1, true, true, 1);
        let r = g.match_reward(&f, 1, &mut SplitMix(2));
        assert_eq!((r.exp, r.rep), (0, 0));
        assert!((100..=200).contains(&r.silver));
    }

    /// The gun race (mode 3) takes its own events.options block (`gunrace`: a kill pays 45 silver, 6 experience, 3
    /// reputation, score 0) and no level factor, whatever the level and the server's silver factor (client 0x200332080).
    #[test]
    fn the_gun_race_pays_its_own_rows_without_a_level_factor() {
        crate::require_tables!();
        let g = gd();
        let mut f = MatchFacts::local(13, crate::gunrace::MODE, false, true, 1);
        f.stat_events = Some([(0u16, 4u32)].into_iter().collect());
        for (level, k) in [(1, None), (5, Some(14.0))] {
            let (r, p) = g.match_reward_parts_with(&f, level, k, &mut SplitMix(3));
            assert_eq!((r.exp, r.rep, r.silver - p.drop_silver), (6 * 4, 3 * 4, 45 * 4), "level {level}");
        }
        assert_eq!(mode_key(3), Some("gunrace"));
    }
}
