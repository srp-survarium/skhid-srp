//! The after-match results screen (`../srp-021d/docs/notes/results_spec.md`)
//! and what a match earns.
//!
//! The screen is `36 13`: `match_stats` (winner, team scores, the four "best"
//! titles, a brief row per player) and the recipient's own
//! `player_result_full`. Clicking a player asks `21 0E` for that player's
//! full result, answered with `36 0E`.
//!
//! The 0.21d data has no experience values
//! (`../srp-021d/docs/notes/progression.md`). What a match
//! earns:
//! - experience: a tenth of the score, so a kill (score weight 100) is 10 as
//!   in 0.21d (the 0.22a changelog cut it "from 10 to 3"), and batteries count
//!   through their score weights; plus 10 for staying to the end and 20 for a
//!   win [both a choice];
//! - all that times the experience rate, `SRP_EXPERIENCE_RATE` (default 3,
//!   so the 20 levels take a few hundred matches, not a thousand);
//! - money: 1.5 a point of experience before the rate, so a kill is 15 as
//!   0.22a cut it to (0.21d paid 50, too much against the shop's prices);
//! - reputation: as much as the experience, for the player's gear faction
//!   (`get_profile_faction_affinity`); none when the gear has no faction;
//! - drops: one item for each notch of the "found" bar the score reaches
//!   ([`super::drops`]);
//! - a level takes 500 experience more than the one before took... see
//!   [`experience_for_level`]; each gives a skill point and its reward.

use super::account::{Account, MAX_NAME_LEN, Profile};
use super::drops::{DropTable, Rng, roll};
use super::item_params::item_params;
use super::lobby::{CLIENT_STATUS, Writer};

/// `query_info_types` 0x13 and 0x0E.
pub const MATCH_STATS: u8 = 0x13;
pub const PLAYER_MATCH_STATS: u8 = 0x0E;
/// The client holds 20 rows.
const MAX_ROWS: usize = 20;
/// The client keeps up to four drops.
pub const MAX_DROPS: usize = 4;
/// `game_team_id` when nobody won.
pub const NO_WINNER: u8 = 3;

/// Experience is the score divided by this.
pub const SCORE_PER_EXPERIENCE: u32 = 10;
pub const EXPERIENCE_FOR_FINISHING: u32 = 10;
pub const EXPERIENCE_FOR_WINNING: u32 = 20;
/// Money a point of experience, in tenths: 15, a kill 15 (0.22a).
pub const MONEY_PER_TEN_EXPERIENCE: u32 = 15;
/// The experience rate when `SRP_EXPERIENCE_RATE` is unset [a choice].
pub const DEFAULT_EXPERIENCE_RATE: u32 = 3;

/// `SRP_EXPERIENCE_RATE`: what a match's experience is multiplied by, 1 to
/// 100. Read once.
pub fn experience_rate() -> u32 {
    static RATE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *RATE.get_or_init(|| match std::env::var("SRP_EXPERIENCE_RATE") {
        Ok(value) => match value.trim().parse::<u32>() {
            Ok(rate) if (1..=100).contains(&rate) => rate,
            _ => {
                log::error!(
                    "SRP_EXPERIENCE_RATE is `{value}`; it must be 1 to 100. Using {DEFAULT_EXPERIENCE_RATE}"
                );
                DEFAULT_EXPERIENCE_RATE
            }
        },
        Err(_) => DEFAULT_EXPERIENCE_RATE,
    })
}
/// The client counts gear faction affinity over 7 factions (0..=6).
const FACTIONS: usize = 7;
/// The skill tree has 20 levels.
pub const MAX_LEVEL: u8 = 20;

/// What one player did in a match: `match_stats_events_dict_enum` counters
/// and the match's facts about it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerStats {
    /// The id the screen knows the player by: its roster index + 1.
    pub id: u32,
    pub name: String,
    /// 0 or 1.
    pub team: u8,
    pub level: u8,
    /// The scoreboard's score (the client's weighted events).
    pub score: u16,
    pub kills: u16,
    pub deaths: u16,
    pub assists: u16,
    pub healed: u16,
    pub saved: u16,
    pub headshots: u16,
    pub silent_kills: u16,
    pub melee_kills: u16,
    /// ×100: credit for a battery is split by who carried it.
    pub stolen_x100: u16,
    pub found: u16,
    pub defended: u16,
    pub artefacts: u16,
    pub max_killstreak: u16,
    pub shots: u16,
    pub hits: u16,
    pub weapons: [u16; 2],
    /// It was there when the match ended.
    pub finished: bool,
    /// Its team won.
    pub won: bool,
    /// Batteries it put in its team's container.
    pub delivered: u16,
    /// Its kills of players wearing each faction's gear (the statistics'
    /// faction counts).
    pub faction_kills: [u8; 7],
    /// Batteries in its team's container at the end.
    pub team_batteries: u8,
    /// The other team stole a battery from its base.
    pub base_robbed: bool,
}

impl PlayerStats {
    /// Whole batteries stolen.
    pub fn steals(&self) -> u32 {
        u32::from(self.stolen_x100) / 100
    }

    pub fn accuracy(&self) -> u8 {
        (u32::from(self.hits) * 100)
            .checked_div(u32::from(self.shots))
            .unwrap_or(0)
            .min(100) as u8
    }
}

/// What a match gave one player.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reward {
    pub experience: u32,
    pub money: u32,
    /// The gear faction credited (0: none, no box on the screen).
    pub reputation_faction: u8,
    pub reputation: u32,
    /// The reputation levels the match reached.
    pub reputation_levels: Vec<u8>,
    /// The level after the match.
    pub level: u8,
    /// `{dict_id, condition_or_stack}`.
    pub drops: Vec<(u16, u32)>,
    pub drop_progress: u32,
    /// The rating before and after the match (`rating`).
    pub rating_before: u16,
    pub rating_after: u16,
}

/// The experience (at `rate`) and money a match earns.
pub fn earned_at(stats: &PlayerStats, rate: u32) -> (u32, u32) {
    let (experience, money) = earned(stats);
    (experience * rate, money)
}

/// The experience and money a match earns, before the experience rate.
pub fn earned(stats: &PlayerStats) -> (u32, u32) {
    let mut experience = u32::from(stats.score) / SCORE_PER_EXPERIENCE;
    if stats.finished {
        experience += EXPERIENCE_FOR_FINISHING;
    }
    if stats.won {
        experience += EXPERIENCE_FOR_WINNING;
    }
    (experience, experience * MONEY_PER_TEN_EXPERIENCE / 10)
}

/// The faction most of a profile's gear belongs to, by the items' faction
/// affinity (`get_profile_faction_affinity` 0x1DEC3A: a tie goes to the
/// lower faction), and that faction's share of the affinity. 0 when no
/// item has any.
pub fn gear_faction(profile: &Profile) -> (u8, f32) {
    let mut affinity = [0_u32; FACTIONS];
    for item in profile.slots.iter().flatten() {
        if let Some(params) = item_params(item.dict_id)
            && usize::from(params.faction) < FACTIONS
        {
            affinity[usize::from(params.faction)] += params.faction_affinity;
        }
    }
    let total: u32 = affinity.iter().sum();
    if total == 0 {
        return (0, 0.0);
    }
    let (faction, most) = affinity
        .iter()
        .enumerate()
        .fold((0, 0), |best, (faction, &value)| {
            if value > best.1 {
                (faction, value)
            } else {
                best
            }
        });
    (faction as u8, most as f32 / total as f32)
}

/// The reputation level `points` reach with `faction`, from the levels'
/// thresholds (`factions_dict`).
pub fn reputation_level(faction: u8, points: u32) -> u8 {
    crate::dictionary::Dictionary::shared()
        .factions
        .iter()
        .find(|entry| entry.id == faction)
        .map_or(0, |entry| {
            entry
                .levels
                .iter()
                .skip(1)
                .take_while(|level| points >= level.value)
                .count() as u8
        })
}

/// Add reputation with `faction`; returns the levels newly reached.
pub fn add_reputation(account: &mut Account, faction: u8, amount: u32) -> Vec<u8> {
    let Some(entry) = account
        .reputations
        .iter_mut()
        .find(|(id, _)| *id == faction)
    else {
        return Vec::new();
    };
    let before = reputation_level(faction, entry.1.into());
    entry.1 = u32::from(entry.1)
        .saturating_add(amount)
        .min(u16::MAX.into()) as u16;
    let after = reputation_level(faction, entry.1.into());
    (before + 1..=after).collect()
}

/// The experience `level` takes over the one before it: 500 for level 2,
/// 250 more for each next (the wiki's table from level 0, shifted by one as
/// 0.21d profiles start at level 1).
pub fn experience_for_level(level: u8) -> u32 {
    500 + 250 * u32::from(level.saturating_sub(2))
}

/// The level reached with `total` experience, where it starts and where the
/// next one does.
pub fn level_at(total: u32) -> (u8, u32, u32) {
    let (mut level, mut start) = (1, 0_u32);
    let mut next = experience_for_level(2);
    while total >= next && level < MAX_LEVEL {
        level += 1;
        start = next;
        next += experience_for_level(level + 1);
    }
    (level, start, next)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelReward {
    Money(u32),
    Premium(u32),
}

/// The wiki's reward for reaching `level` (counted as the wiki did, from
/// 0): gold at its level 1 and every fifth, money otherwise.
pub fn level_reward(level: u8) -> LevelReward {
    let wiki_level = u32::from(level) - 1;
    if wiki_level == 1 || wiki_level.is_multiple_of(5) {
        LevelReward::Premium(100)
    } else {
        LevelReward::Money((500 + 250 * ((wiki_level - 1) / 5)).min(5_000))
    }
}

/// Give `account` what a match earned with `profile_id`: the experience to
/// that profile (levels, each with its reward), the money, the reputation
/// and the drops.
pub fn apply(account: &mut Account, profile_id: u32, stats: &PlayerStats) -> Reward {
    apply_with(account, profile_id, stats, &mut Rng::from_time())
}

/// Put dropped items into the inventory: a stack onto the one the account
/// has, anything else as a new item.
fn add_drops(account: &mut Account, drops: &[(u16, u16)]) {
    let dictionary = crate::dictionary::Dictionary::shared();
    for &(dict_id, count) in drops {
        let is_stack = dictionary.item(dict_id).is_some_and(|item| item.is_stack);
        let owned = account
            .inventory
            .iter_mut()
            .find(|item| is_stack && item.dict_id == dict_id);
        match owned {
            Some(owned) => {
                owned.condition_or_stack = owned.condition_or_stack.saturating_add(count);
                owned.amount = owned.condition_or_stack.into();
            }
            None => {
                account.add_item(dict_id, count, is_stack);
            }
        }
    }
    // The slots holding a stack carry its new size.
    account.sync_stacks();
}

/// [`apply`] with a given random source.
pub fn apply_with(
    account: &mut Account,
    profile_id: u32,
    stats: &PlayerStats,
    rng: &mut Rng,
) -> Reward {
    apply_at(account, profile_id, stats, rng, super::quests::now())
}

/// [`apply_with`] at `now` (Unix seconds), which decides premium: half again
/// of the experience, money and reputation, and of the drop progress.
pub fn apply_at(
    account: &mut Account,
    profile_id: u32,
    stats: &PlayerStats,
    rng: &mut Rng,
    now: u64,
) -> Reward {
    let premium = super::premium::is_active(account, now);
    let (mut experience, mut money) = earned_at(stats, experience_rate());
    if premium {
        experience = super::premium::with_bonus(experience);
        money = super::premium::with_bonus(money);
    }
    let mut reward = Reward {
        experience,
        money,
        level: stats.level,
        ..Reward::default()
    };
    if let Some(profile) = account.profile(profile_id) {
        let (faction, _) = gear_faction(profile);
        if faction != 0 && account.reputations.iter().any(|(id, _)| *id == faction) {
            let reputation = super::loyalty::with_bonus(account, faction, experience);
            reward.reputation_faction = faction;
            reward.reputation = reputation;
            reward.reputation_levels = add_reputation(account, faction, reputation);
        }
    }
    if let Some(profile) = account.profile(profile_id) {
        let dictionary = crate::dictionary::Dictionary::shared();
        let wear_level = profile
            .slots
            .iter()
            .flatten()
            .filter_map(|item| dictionary.item(item.dict_id))
            .map(|item| item.level)
            .max()
            .unwrap_or(0);
        let table = DropTable::shared();
        reward.drop_progress = if premium {
            table.premium_progress(stats.score)
        } else {
            table.progress(stats.score)
        };
        let drops = roll(table, dictionary, reward.drop_progress, wear_level, rng);
        add_drops(account, &drops);
        reward.drops = drops
            .into_iter()
            .map(|(dict_id, count)| (dict_id, count.into()))
            .collect();
    }
    let mut levels = Vec::new();
    if let Some(profile) = account
        .profiles
        .iter_mut()
        .find(|profile| profile.id == profile_id)
    {
        profile.exp_total = profile.exp_total.saturating_add(experience);
        let (level, start, next) = level_at(profile.exp_total);
        let level = level.max(profile.level);
        levels.extend(profile.level + 1..=level);
        profile.level = level;
        profile.exp_level_start = start;
        profile.exp_next_level = next;
        reward.level = level;
    }
    account.generic_money = account.generic_money.saturating_add(money);
    for level in levels {
        match level_reward(level) {
            LevelReward::Money(money) => {
                account.generic_money = account.generic_money.saturating_add(money)
            }
            LevelReward::Premium(gold) => {
                account.premium_money = account.premium_money.saturating_add(gold)
            }
        }
    }
    reward
}

/// The best player of the four titles, by `key`, if anyone scored.
fn best(players: &[PlayerStats], key: impl Fn(&PlayerStats) -> u32) -> Option<&PlayerStats> {
    players
        .iter()
        .filter(|player| key(player) > 0)
        .max_by_key(|player| key(player))
}

/// `match_stats`: the winner, the team scores (batteries in base), the four
/// titles and the rows.
fn write_match_stats(
    writer: &mut Writer,
    match_id: u32,
    winner: u8,
    scores: [u8; 2],
    players: &[PlayerStats],
) {
    writer.u32(match_id).u8(winner).u8(scores[0]).u8(scores[1]);
    let title = |writer: &mut Writer, player: Option<&PlayerStats>| {
        writer.str(player.map_or("", |player| &player.name), MAX_NAME_LEN);
    };
    let player = best(players, |player| u32::from(player.score));
    title(writer, player);
    writer.u16(player.map_or(0, |player| player.score));
    let shooter = best(players, |player| {
        u32::from(player.accuracy()) * 0x10000 + u32::from(player.shots)
    });
    title(writer, shooter);
    writer
        .u8(shooter.map_or(0, PlayerStats::accuracy))
        .u16(shooter.map_or(0, |player| player.shots));
    let support = best(players, |player| {
        u32::from(player.saved) + u32::from(player.healed)
    });
    title(writer, support);
    writer
        .u16(support.map_or(0, |player| player.saved))
        .u16(support.map_or(0, |player| player.healed));
    let explorer = best(players, |player| {
        u32::from(player.artefacts) + u32::from(player.found)
    });
    title(writer, explorer);
    writer
        .u16(explorer.map_or(0, |player| player.artefacts))
        .u16(explorer.map_or(0, |player| player.found));
    let rows = &players[..players.len().min(MAX_ROWS)];
    writer.u8(rows.len() as u8);
    for player in rows {
        writer
            .str(&player.name, MAX_NAME_LEN)
            .u32(player.id)
            .u16(player.score)
            .u16(player.kills)
            .u16(player.deaths)
            .u8(player.team)
            .u16(u16::from(player.level));
    }
}

/// `player_result_full`.
fn write_full(writer: &mut Writer, stats: &PlayerStats, reward: &Reward) {
    writer
        .u32(stats.id)
        .u16(stats.assists)
        .u16(stats.healed)
        .u16(stats.saved)
        .u16(stats.headshots)
        .u16(stats.silent_kills)
        .u16(stats.melee_kills)
        .u16(stats.found)
        .u16(stats.defended)
        .u16(stats.artefacts)
        .u8(reward.level)
        .u16(stats.stolen_x100)
        .u16(reward.experience.min(u16::MAX.into()) as u16)
        .u16(reward.money.min(u16::MAX.into()) as u16)
        .u16(reward.rating_before)
        .u16(reward.rating_after)
        // Faction 0 hides the reputation box.
        .u8(reward.reputation_faction)
        .u16(reward.reputation.min(u16::MAX.into()) as u16)
        .u16(stats.max_killstreak)
        .u8(stats.accuracy())
        .u16(stats.weapons[0])
        .u16(stats.weapons[1])
        // A list the client never reads.
        .u8(0);
    let drops = &reward.drops[..reward.drops.len().min(MAX_DROPS)];
    writer.u8(drops.len() as u8);
    for &(dict_id, count) in drops {
        writer.u32(dict_id.into()).u32(count);
    }
    writer.u32(reward.drop_progress);
}

/// `36 13`: the screen for `recipient`.
pub fn match_stats(
    match_id: u32,
    winner: u8,
    scores: [u8; 2],
    players: &[PlayerStats],
    recipient: &PlayerStats,
    reward: &Reward,
) -> Vec<u8> {
    let mut writer = Writer::new(CLIENT_STATUS);
    writer.u8(MATCH_STATS);
    write_match_stats(&mut writer, match_id, winner, scores, players);
    write_full(&mut writer, recipient, reward);
    writer.done()
}

/// `36 0E`: one player's full result, for the screen's detail panel.
pub fn player_match_stats(stats: &PlayerStats, reward: &Reward) -> Vec<u8> {
    let mut writer = Writer::new(CLIENT_STATUS);
    writer.u8(PLAYER_MATCH_STATS);
    write_full(&mut writer, stats, reward);
    writer.done()
}

/// The stats sender's chat line that makes the client ask for the screen.
pub fn stats_line(match_id: u32) -> String {
    format!("#mstats [{match_id}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alice() -> PlayerStats {
        PlayerStats {
            id: 1,
            name: "alice".into(),
            team: 0,
            level: 1,
            score: 120,
            kills: 3,
            deaths: 1,
            stolen_x100: 200,
            shots: 40,
            hits: 10,
            finished: true,
            ..PlayerStats::default()
        }
    }

    #[test]
    fn a_dropped_stack_reaches_the_slots_holding_it() {
        let dictionary = crate::dictionary::Dictionary::shared();
        let mut account = Account::new("Player", dictionary);
        let ammo = account.profiles[0].slots[8].clone().unwrap();
        add_drops(&mut account, &[(ammo.dict_id, 20)]);
        let slot = account.profiles[0].slots[8].as_ref().unwrap();
        assert_eq!(slot.condition_or_stack, ammo.condition_or_stack);
        assert_eq!(slot.amount, u32::from(ammo.condition_or_stack) + 20);
    }

    #[test]
    fn experience_is_a_tenth_of_the_score_and_the_base() {
        // 12 from the score, 10 for finishing.
        assert_eq!(earned(&alice()), (22, 33));
        let winner = PlayerStats {
            won: true,
            ..alice()
        };
        assert_eq!(earned(&winner), (42, 63));
        let quitter = PlayerStats {
            finished: false,
            ..alice()
        };
        assert_eq!(earned(&quitter), (12, 18));
        // The live match: 3 batteries found and delivered, a win.
        let batteries = PlayerStats {
            score: 375,
            won: true,
            ..alice()
        };
        assert_eq!(earned(&batteries), (67, 100));
        // The rate multiplies the experience only.
        assert_eq!(earned_at(&batteries, 3), (201, 100));
    }

    #[test]
    fn the_gear_faction_has_the_most_affinity() {
        let account = Account::with_test_loadout("alice", crate::dictionary::Dictionary::shared());
        let mut profile = account.profiles[0].clone();
        // The starting loadout is mostly Renaissance (3).
        assert_eq!(gear_faction(&profile).0, 3);
        for slot in profile.slots.iter_mut() {
            *slot = None;
        }
        assert_eq!(gear_faction(&profile), (0, 0.0));
        // The Renaissance torso (30) alone.
        profile.slots[2] = account.profiles[0].slots[2].clone();
        assert_eq!(gear_faction(&profile), (3, 1.0));
        // A new account's TT-33 belongs to no faction.
        let new = Account::new("bob", crate::dictionary::Dictionary::shared());
        assert_eq!(gear_faction(&new.profiles[0]).0, 0);
    }

    #[test]
    fn reputation_levels_follow_the_faction_thresholds() {
        assert_eq!(reputation_level(1, 0), 0);
        assert_eq!(reputation_level(1, 99), 0);
        assert_eq!(reputation_level(1, 100), 1);
        assert_eq!(reputation_level(1, 3000), 5);
        assert_eq!(reputation_level(3, 1499), 0);
        assert_eq!(reputation_level(3, 35_250), 10);
        let mut account =
            Account::with_test_loadout("alice", crate::dictionary::Dictionary::shared());
        assert_eq!(add_reputation(&mut account, 1, 450), [1, 2]);
        assert_eq!(add_reputation(&mut account, 1, 10), [] as [u8; 0]);
        assert_eq!(account.reputations[0], (1, 460));
    }

    #[test]
    fn a_match_levels_the_profile_it_was_played_with() {
        let mut account =
            Account::with_test_loadout("alice", crate::dictionary::Dictionary::shared());
        let money = account.generic_money;
        let premium = account.premium_money;
        let stats = PlayerStats {
            score: 5000,
            won: true,
            ..alice()
        };
        // (500 + 10 + 20) × the rate: level 2 or more, and its 100 gold.
        let rate = experience_rate();
        let items = account.inventory.len();
        let reward = apply_with(&mut account, 1, &stats, &mut Rng::new(3));
        assert_eq!(reward.experience, 530 * rate);
        let profile = &account.profiles[0];
        assert_eq!(profile.exp_total, 530 * rate);
        assert_eq!(profile.level, level_at(530 * rate).0);
        assert!(profile.level >= 2);
        // 1.5 money a point of experience before the rate.
        assert!(account.generic_money >= money + 795);
        assert_eq!(account.premium_money, premium + 100);
        // The same as reputation, for the Renaissance gear.
        assert_eq!(
            (reward.reputation_faction, reward.reputation),
            (3, 530 * rate)
        );
        assert_eq!(account.reputations[2], (3, (530 * rate) as u16));
        // A full bar: three drops, in the inventory.
        assert_eq!(reward.drop_progress, 10_000);
        assert_eq!(reward.drops.len(), 3);
        for &(dict_id, _) in &reward.drops {
            assert!(account.inventory.iter().any(|item| item.dict_id == dict_id));
        }
        assert!(account.inventory.len() >= items);
    }

    #[test]
    fn premium_gives_half_again() {
        let stats = PlayerStats {
            score: 400,
            won: true,
            ..alice()
        };
        let now = 1_000_000;
        let mut plain =
            Account::with_test_loadout("alice", crate::dictionary::Dictionary::shared());
        let mut premium = plain.clone();
        premium.premium_until = now + 1;
        let money = plain.generic_money;
        let without = apply_at(&mut plain, 1, &stats, &mut Rng::new(3), now);
        let with = apply_at(&mut premium, 1, &stats, &mut Rng::new(3), now);
        // (40 + 10 + 20) × the rate, and 1.5 money a point before the rate.
        let rate = experience_rate();
        assert_eq!(without.experience, 70 * rate);
        assert_eq!(with.experience, 105 * rate);
        assert_eq!((without.money, with.money), (105, 157));
        assert_eq!(with.reputation, 105 * rate);
        assert_eq!(premium.profiles[0].exp_total, 105 * rate);
        assert_eq!(
            premium.generic_money - money,
            plain.generic_money - money + 52
        );
        assert_eq!((without.drop_progress, with.drop_progress), (4000, 6000));
        // Run out: no bonus.
        let mut expired =
            Account::with_test_loadout("alice", crate::dictionary::Dictionary::shared());
        expired.premium_until = now;
        let reward = apply_at(&mut expired, 1, &stats, &mut Rng::new(3), now);
        assert_eq!(reward.experience, 70 * rate);

        // Loyalty to the gear faction adds to the reputation only.
        let mut loyal =
            Account::with_test_loadout("alice", crate::dictionary::Dictionary::shared());
        let faction = without.reputation_faction;
        assert_ne!(faction, 0);
        loyal.loyalty[usize::from(faction) - 1] = 20;
        let reward = apply_at(&mut loyal, 1, &stats, &mut Rng::new(3), now);
        assert_eq!(reward.experience, 70 * rate);
        assert_eq!(reward.reputation, 84 * rate);
    }

    #[test]
    fn levels_take_250_more_each() {
        assert_eq!(level_at(0), (1, 0, 500));
        assert_eq!(level_at(499), (1, 0, 500));
        assert_eq!(level_at(500), (2, 500, 1250));
        assert_eq!(level_at(1250), (3, 1250, 2250));
        assert_eq!(level_at(u32::MAX).0, MAX_LEVEL);
        assert_eq!(level_reward(2), LevelReward::Premium(100));
        assert_eq!(level_reward(3), LevelReward::Money(500));
        assert_eq!(level_reward(6), LevelReward::Premium(100));
        assert_eq!(level_reward(7), LevelReward::Money(750));
    }

    #[test]
    fn the_screen_is_the_match_then_the_recipients_result() {
        let bob = PlayerStats {
            id: 2,
            name: "bob".into(),
            team: 1,
            ..PlayerStats::default()
        };
        let reward = Reward {
            experience: 100,
            money: 1100,
            level: 1,
            reputation_faction: 3,
            reputation: 0x0102,
            drops: vec![(22, 20)],
            drop_progress: 1100,
            ..Reward::default()
        };
        let players = [alice(), bob];
        let screen = match_stats(0x123, 0, [2, 0], &players, &players[0], &reward);
        let mut expected = vec![0x36, 0x13, 0x23, 1, 0, 0, 0, 2, 0];
        // Best player alice (120), best shooter alice (25 %, 40 shots); no
        // support or explorer.
        expected.extend([5, b'a', b'l', b'i', b'c', b'e', 120, 0]);
        expected.extend([5, b'a', b'l', b'i', b'c', b'e', 25, 40, 0]);
        expected.extend([0, 0, 0, 0, 0]);
        expected.extend([0, 0, 0, 0, 0]);
        expected.push(2);
        expected.extend([
            5, b'a', b'l', b'i', b'c', b'e', 1, 0, 0, 0, 120, 0, 3, 0, 1, 0, 0, 1, 0,
        ]);
        expected.extend([3, b'b', b'o', b'b', 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0]);
        assert_eq!(screen[..expected.len()], expected);
        let full = &screen[expected.len()..];
        assert_eq!(full[..4], [1, 0, 0, 0]);
        // Level, stolen ×100, experience, money, rating before and after,
        // the reputation faction and amount.
        assert_eq!(
            full[22..38],
            [1, 200, 0, 100, 0, 0x4C, 0x04, 0, 0, 0, 0, 3, 2, 1, 0, 0]
        );
        // One drop, then the progress.
        let tail = &full[full.len() - 13..];
        assert_eq!(tail, [1, 22, 0, 0, 0, 20, 0, 0, 0, 0x4C, 0x04, 0, 0]);
        assert_eq!(stats_line(0x123), "#mstats [291]");
    }
}
