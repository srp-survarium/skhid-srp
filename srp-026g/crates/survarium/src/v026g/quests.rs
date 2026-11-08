//! The 0.26g faction quests, from the game's
//! `gameplay/quests/quests.options` (0.26g's `resources.db`;
//! `docs/notes/data.md` §5).
//!
//! Against 0.21d (`v021d::quests`): the file has only `quests` (369; no
//! `tasks`, `types` or `quest_givers`), a quest's entry has the same keys,
//! and **the `type` numbers moved** ([`kind`]): four goals are new and
//! `kill_enemy_x` is gone.
//!
//! This module is the data and the rules that need nothing but a match's
//! numbers: which quests a faction offers at a reputation level
//! ([`Quests::band`]) and whether a match meets a goal ([`Quest::met_by`]).
//! Giving, swapping, expiring and paying quests is the account's business
//! and comes with the 0.26g account model; 0.21d's rules for it
//! (`v021d::quests::{refresh, change, settle}`) carry over unchanged, since
//! the quest entries' keys, the awards and the preconditions did not change.
//!
//! The goals are checked as the 0.26g client's own counter of them counts
//! (PROVEN): `game_quest_progress_calculator::refresh_quest_ui_data`
//! 0x2A2FD0 (the function names itself in an assert) switches on the
//! quest's type and reads the player's statistics record, where event `e`
//! has its three kits' counters at `e × 6`. `set_player_quests` 0x2A35E0
//! makes the list it counts and sets each quest's target.
//!
//! | type | the client's counter | target |
//! |---|---|---|
//! | 6 `bring_x_items` | event 24, a battery found and carried alone ([`MatchStats::delivered`]); 0.21d counted event 20, every battery put into the container | `n` |
//! | 7 `steal_box` | event 26, an enemy's battery put into the container by the one who took it | `n` |
//! | 10 `find_artifacts` | event 5 | `n` |
//! | 11 `entrap_enemies` | event 21 | `n` |
//! | 12 `kill_enemies` | event 0 | `n` |
//! | 16 `grenade_enemies` | event 25 | `n` |
//! | 17 `meele_kill` | event 13 | `n` |
//! | 18 `headshots` | event 15 | `n` |
//! | 21 `killstreak` | the longest kill streak | `n` |
//! | 30 `win_x_games` | the games counted so far, and one more when the player's team is the winner | `max_games_count` |
//! | 31 `kd_ratio` | kills (event 0) / deaths (event 2); 0 with no kill; the kills themselves with no death | `y` |
//! | 32 `accuracy` | hits (event 23) × 100 / shots (event 22); 0 with no shot | `y` × 100 |
//! | 34 `win_score` | the batteries in the team's base | `n` |
//! | 35 `defend_items` | 1, and 0 once a player of the other team has event 17 | 1 |
//!
//! Types 6, 7, 34 and 35 are listed only in the two battery modes
//! (0x2A36B1, 0x2A3AD5, 0x2A3BE9 test the mode for 2 or 3): Team
//! Deathmatch has none of them.
//!
//! Not in that code, and read from the English objective texts [LIKELY]:
//! - which goals need the battle won ([`Quest::needs_win`]): the counter
//!   shows the number alone;
//! - that a counter at its target is the goal met: `kd_ratio` reads "the
//!   kill/death ratio of at least". `accuracy` reads "accuracy rate higher
//!   than" for one faction and "accuracy rate:" for the others; srp takes
//!   the target itself as met.

use binary_config_parser::tree::{ConfigError, ConfigValue};

/// Its path in the archive.
pub const PATH: &str = "gameplay/quests/quests.options";

/// The factions whose quests the lobby shows.
pub const FACTIONS: [u8; 4] = [1, 2, 3, 4];

/// `type` values the 0.26g data uses, each named by its quests'
/// `ui.name` (`st_<faction>_<goal>_name`).
pub mod kind {
    pub const BRING_ITEMS: u32 = 6;
    /// New in 0.26g.
    pub const STEAL_BOX: u32 = 7;
    pub const FIND_ARTEFACTS: u32 = 10;
    /// New in 0.26g.
    pub const ENTRAP_ENEMIES: u32 = 11;
    pub const KILL_ENEMIES: u32 = 12;
    /// New in 0.26g (0.21d's 16 was headshots).
    pub const GRENADE_ENEMIES: u32 = 16;
    /// New in 0.26g.
    pub const MELEE_KILL: u32 = 17;
    /// 0.21d: 16.
    pub const HEADSHOTS: u32 = 18;
    /// 0.21d: 19.
    pub const KILLSTREAK: u32 = 21;
    /// 0.21d: 28.
    pub const WIN_GAMES: u32 = 30;
    /// 0.21d: 29.
    pub const KD_RATIO: u32 = 31;
    /// 0.21d: 30.
    pub const ACCURACY: u32 = 32;
    /// 0.21d: 32.
    pub const WIN_SCORE: u32 = 34;
    /// 0.21d: 33.
    pub const DEFEND_ITEMS: u32 = 35;

    /// Each type and the goal in its quests' names.
    pub const NAMES: [(u32, &str); 14] = [
        (BRING_ITEMS, "bring_x_items"),
        (STEAL_BOX, "steal_box"),
        (FIND_ARTEFACTS, "find_artifacts"),
        (ENTRAP_ENEMIES, "entrap_enemies"),
        (KILL_ENEMIES, "kill_enemies"),
        (GRENADE_ENEMIES, "grenade_enemies"),
        (MELEE_KILL, "meele_kill"),
        (HEADSHOTS, "headshots"),
        (KILLSTREAK, "killstreak"),
        (WIN_GAMES, "win_x_games"),
        (KD_RATIO, "kd_ratio"),
        (ACCURACY, "accuracy"),
        (WIN_SCORE, "win_score"),
        (DEFEND_ITEMS, "defend_items"),
    ];
}

/// Precondition and award types (unchanged from 0.21d).
const COOLDOWN: i32 = 0;
const MINIMUM_REPUTATION: i32 = 1;
const MAXIMUM_REPUTATION: i32 = 2;
const AWARD_REPUTATION: i32 = 1;
const AWARD_MONEY: i32 = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct Quest {
    pub id: u32,
    pub faction: u8,
    pub kind: u32,
    pub n: u32,
    pub x: u32,
    pub y: f32,
    pub max_games: u32,
    /// 0, or the faction whose gear the player must wear (0 in every 0.26g
    /// quest).
    pub representative: u8,
    pub daily: bool,
    pub priority: u32,
    pub ttl_minutes: u32,
    pub cooldown_minutes: u32,
    /// The reputation levels, with `level_faction`, it is offered at.
    pub level_faction: u8,
    pub min_level: u8,
    pub max_level: u8,
    pub money: u32,
    /// `(faction, amount)`.
    pub reputation: Vec<(u8, u32)>,
    /// `ui.name`, a localization key.
    pub name: String,
    /// The goal as the lobby lists it (`ui.task_descriptions`' pair): its
    /// text's localization key, and the target the lobby prints after it
    /// (" 6", " 1.5", " 25%"), none for a goal without a number.
    pub objective: String,
    pub target: Option<String>,
}

/// What one match gave a player, as far as quests ask.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchStats {
    pub won: bool,
    pub kills: u16,
    pub deaths: u16,
    pub headshots: u16,
    pub max_killstreak: u16,
    pub grenade_kills: u16,
    pub melee_kills: u16,
    /// Enemies caught in the player's traps.
    pub trapped: u16,
    pub shots: u32,
    pub hits: u32,
    pub artefacts: u16,
    /// Batteries the player found and brought to the base: the game's
    /// event 24 (`match_msgs::snapshot::event::SOLO_DELIVERY`), which is
    /// what the client's own counter of `bring_x_items` shows. A battery
    /// put into the container counts when the player was the first to pick
    /// it up while it was new, or it was the enemy's, and no teammate
    /// carried it since it last left a container. So a carry finished for
    /// a teammate, a new battery somebody else dropped and the team's own
    /// battery brought back do not count.
    pub delivered: u16,
    /// Batteries of the enemy's the player took from its base and put into
    /// the container itself: the game's event 26
    /// (`match_msgs::snapshot::event::STOLEN_AND_DELIVERED`), counted by
    /// ones. Not event 17, the credit for a stolen battery, which is
    /// shared out by the distance each carrier carried it.
    pub stolen: u16,
    /// Batteries the player's team has at the end.
    pub team_batteries: u16,
    /// Whether the enemy took a battery from the team's base.
    pub base_robbed: bool,
    /// Whether the match was of a battery mode (2 or 3). The client lists
    /// the battery goals in no other mode, so "Win a match without losing
    /// batteries" is not met by a Team Deathmatch won.
    pub battery_mode: bool,
}

impl Quest {
    /// Whether the goal needs the battle won (the goals' texts: "Win
    /// matches", "Win a match with ...", "Win a match without losing
    /// batteries").
    pub fn needs_win(&self) -> bool {
        matches!(
            self.kind,
            kind::WIN_GAMES
                | kind::KD_RATIO
                | kind::ACCURACY
                | kind::WIN_SCORE
                | kind::DEFEND_ITEMS
        )
    }

    /// Whether one match's play meets the goal. A won battle counts for
    /// `win_x_games`, which needs `max_games` of them.
    pub fn met_by(&self, stats: &MatchStats) -> bool {
        if self.needs_win() && !stats.won {
            return false;
        }
        let n = self.n;
        match self.kind {
            kind::BRING_ITEMS => u32::from(stats.delivered) >= n,
            kind::STEAL_BOX => u32::from(stats.stolen) >= n,
            kind::FIND_ARTEFACTS => u32::from(stats.artefacts) >= n,
            kind::ENTRAP_ENEMIES => u32::from(stats.trapped) >= n,
            kind::KILL_ENEMIES => u32::from(stats.kills) >= n,
            kind::GRENADE_ENEMIES => u32::from(stats.grenade_kills) >= n,
            kind::MELEE_KILL => u32::from(stats.melee_kills) >= n,
            kind::HEADSHOTS => u32::from(stats.headshots) >= n,
            kind::KILLSTREAK => u32::from(stats.max_killstreak) >= n,
            kind::KD_RATIO => {
                // The client's counter (0x2A3294): nothing without a kill,
                // the kills themselves without a death.
                let ratio = if stats.kills == 0 {
                    0.0
                } else if stats.deaths == 0 {
                    f32::from(stats.kills)
                } else {
                    f32::from(stats.kills) / f32::from(stats.deaths)
                };
                ratio >= self.y
            }
            kind::ACCURACY => {
                stats.shots > 0 && stats.hits as f32 * 100.0 / stats.shots as f32 >= self.y * 100.0
            }
            kind::WIN_SCORE => u32::from(stats.team_batteries) >= n,
            kind::DEFEND_ITEMS => stats.battery_mode && !stats.base_robbed,
            kind::WIN_GAMES => true,
            _ => false,
        }
    }
}

fn int(value: &ConfigValue, key: &str) -> Result<i32, ConfigError> {
    value.i32_at(key)
}

fn parse_quest(quest: &ConfigValue) -> Result<Quest, ConfigError> {
    let params = quest.req("params")?;
    let ui = quest.req("ui")?;
    let (objective, target) = match ui.req("task_descriptions")?.children().as_slice() {
        [pair] => match pair.children().as_slice() {
            [key, target] => key.as_str().zip(target.as_str()),
            _ => None,
        },
        _ => None,
    }
    .ok_or_else(|| ConfigError("`ui.task_descriptions` is not one pair of strings".into()))?;
    let mut parsed = Quest {
        id: int(quest, "id")? as u32,
        faction: int(quest, "faction")? as u8,
        kind: int(quest, "type")? as u32,
        n: int(params, "n")?.max(0) as u32,
        x: int(params, "x")?.max(0) as u32,
        y: params.f32_at("y")?,
        max_games: int(params, "max_games_count")?.max(1) as u32,
        representative: int(quest, "faction_representative")? as u8,
        daily: quest
            .req("daily")?
            .as_bool()
            .ok_or_else(|| ConfigError("`daily` is not a boolean".into()))?,
        priority: int(quest, "priority")?.max(0) as u32,
        ttl_minutes: int(quest, "ttl_minutes")?.max(1) as u32,
        cooldown_minutes: 0,
        level_faction: 0,
        min_level: 0,
        max_level: u8::MAX,
        money: 0,
        reputation: Vec::new(),
        name: ui.str_at("name")?.to_owned(),
        objective: objective.to_owned(),
        target: Some(target.trim())
            .filter(|target| !target.is_empty())
            .map(str::to_owned),
    };
    parsed.level_faction = parsed.faction;
    for condition in quest.req("preconditions")?.children() {
        let n = int(condition, "n")?.max(0);
        match int(condition, "type")? {
            COOLDOWN => parsed.cooldown_minutes = n as u32,
            MINIMUM_REPUTATION => {
                parsed.min_level = n as u8;
                parsed.level_faction = int(condition, "x")? as u8;
            }
            MAXIMUM_REPUTATION => {
                parsed.max_level = n as u8;
                parsed.level_faction = int(condition, "x")? as u8;
            }
            _ => {}
        }
    }
    for award in quest.req("awards")?.children() {
        let amount = int(award, "amount")?.max(0) as u32;
        match int(award, "type")? {
            AWARD_MONEY => parsed.money += amount,
            AWARD_REPUTATION => parsed.reputation.push((int(award, "param")? as u8, amount)),
            _ => {}
        }
    }
    Ok(parsed)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quests {
    /// By id.
    pub quests: Vec<Quest>,
}

impl Quests {
    pub fn shared() -> &'static Self {
        static SHARED: std::sync::OnceLock<Quests> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| Self::parse(super::game_data::file(PATH)).expect("the quests parse"))
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, ConfigError> {
        let root = ConfigValue::parse(bytes)?;
        let mut quests = root
            .req("quests")?
            .children()
            .into_iter()
            .map(parse_quest)
            .collect::<Result<Vec<_>, _>>()?;
        quests.sort_by_key(|quest| quest.id);
        Ok(Self { quests })
    }

    pub fn quest(&self, id: u32) -> Option<&Quest> {
        self.quests
            .binary_search_by_key(&id, |quest| quest.id)
            .ok()
            .map(|index| &self.quests[index])
    }

    /// The quests `faction` offers a player whose reputation level with a
    /// faction is `level_with(faction)`.
    pub fn band<'a>(
        &'a self,
        faction: u8,
        level_with: impl Fn(u8) -> u8 + 'a,
    ) -> impl Iterator<Item = &'a Quest> {
        self.quests.iter().filter(move |quest| {
            quest.faction == faction
                && (quest.min_level..=quest.max_level).contains(&level_with(quest.level_faction))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_026g_quests() {
        let quests = Quests::shared();
        assert_eq!(quests.quests.len(), 369);
        assert_eq!(quests.quests[0].id, 10101);
        assert_eq!(quests.quests[368].id, 41113);
        let of = |faction: u8| {
            quests
                .quests
                .iter()
                .filter(|q| q.faction == faction)
                .count()
        };
        assert_eq!([of(1), of(2), of(3), of(4)], [63, 64, 121, 121]);
        for quest in &quests.quests {
            assert!(quest.daily && quest.priority == 10 && quest.ttl_minutes == 1440);
            assert_eq!(quest.representative, 0);
            assert!(quest.money > 0, "{}", quest.id);
            // Reputation with its own faction only.
            assert_eq!(quest.reputation.len(), 1, "{}", quest.id);
            assert_eq!(quest.reputation[0].0, quest.faction, "{}", quest.id);
            assert!(quest.cooldown_minutes > 0, "{}", quest.id);
        }

        // The first: Scavengers, defend the batteries, levels 0..=0.
        let quest = quests.quest(10101).unwrap();
        assert_eq!((quest.faction, quest.kind), (1, kind::DEFEND_ITEMS));
        assert_eq!(
            (quest.n, quest.max_games, quest.cooldown_minutes),
            (1, 1, 120)
        );
        assert_eq!(
            (quest.min_level, quest.max_level, quest.level_faction),
            (0, 0, 1)
        );
        assert_eq!(
            (quest.money, quest.reputation.as_slice()),
            (50, &[(1, 50)][..])
        );
        assert_eq!(quest.name, "st_scavengers_defend_items_name");
        assert_eq!(
            (quest.objective.as_str(), quest.target.as_deref()),
            ("st_scavengers_defend_items_obj", None)
        );
        // The lobby's target: an accuracy's in percent.
        assert_eq!(quests.quest(10113).unwrap().target.as_deref(), Some("15%"));
        assert!(quests.quest(1).is_none());
    }

    #[test]
    fn the_type_numbers_are_the_renumbered_ones() {
        let quests = Quests::shared();
        // Every quest's type is one of the fourteen, and its name says which.
        for quest in &quests.quests {
            let goal = kind::NAMES
                .iter()
                .find(|&&(kind, _)| kind == quest.kind)
                .unwrap_or_else(|| panic!("quest {} has type {}", quest.id, quest.kind))
                .1;
            assert!(
                quest.name.ends_with(&format!("_{goal}_name")),
                "{} is type {} but named {}",
                quest.id,
                quest.kind,
                quest.name
            );
        }
        // How many of each, quests.txt.
        let count = |kind: u32| quests.quests.iter().filter(|q| q.kind == kind).count();
        #[rustfmt::skip]
        let expected = [
            (6, 34), (7, 34), (10, 16), (11, 6), (12, 23), (16, 30), (17, 34), (18, 28),
            (21, 17), (30, 34), (31, 34), (32, 17), (34, 34), (35, 28),
        ];
        for (kind, quests_of) in expected {
            assert_eq!(count(kind), quests_of, "type {kind}");
        }
        // 0.21d's numbers that moved are not goals any more.
        for old in [15, 19, 28, 29, 33] {
            assert_eq!(count(old), 0, "type {old}");
        }
    }

    #[test]
    fn a_band_is_a_factions_quests_at_a_level() {
        let quests = Quests::shared();
        // A new player: level 0 with everyone.
        let band: Vec<&Quest> = quests.band(1, |_| 0).collect();
        assert!(!band.is_empty());
        assert!(band.iter().all(|q| q.faction == 1 && q.min_level == 0));
        assert!(band.iter().any(|q| q.id == 10101));
        // At level 3 the level 0 quests are gone.
        let band: Vec<&Quest> = quests
            .band(1, |faction| if faction == 1 { 3 } else { 0 })
            .collect();
        assert!(!band.is_empty());
        assert!(band.iter().all(|q| q.min_level <= 3 && q.max_level >= 3));
        assert!(band.iter().all(|q| q.id != 10101));
        // Every quest is offered at some level of its faction.
        for quest in &quests.quests {
            assert!(quest.min_level <= quest.max_level, "{}", quest.id);
            assert_eq!(quest.level_faction, quest.faction, "{}", quest.id);
        }
    }

    fn quest_of(kind: u32, n: u32, y: f32) -> Quest {
        let mut quest = Quests::shared()
            .quests
            .iter()
            .find(|quest| quest.kind == kind)
            .unwrap()
            .clone();
        quest.n = n;
        quest.y = y;
        quest
    }

    #[test]
    fn goals_are_checked_against_a_matchs_numbers() {
        let stats = MatchStats {
            won: true,
            kills: 6,
            deaths: 4,
            headshots: 2,
            max_killstreak: 3,
            grenade_kills: 1,
            melee_kills: 0,
            trapped: 2,
            shots: 200,
            hits: 50,
            artefacts: 1,
            delivered: 3,
            stolen: 1,
            team_batteries: 7,
            base_robbed: false,
            battery_mode: true,
        };
        let met = |kind, n, y| quest_of(kind, n, y).met_by(&stats);
        assert!(met(kind::BRING_ITEMS, 3, 0.0) && !met(kind::BRING_ITEMS, 4, 0.0));
        assert!(met(kind::STEAL_BOX, 1, 0.0) && !met(kind::STEAL_BOX, 2, 0.0));
        assert!(met(kind::FIND_ARTEFACTS, 1, 0.0) && !met(kind::FIND_ARTEFACTS, 2, 0.0));
        assert!(met(kind::ENTRAP_ENEMIES, 2, 0.0) && !met(kind::ENTRAP_ENEMIES, 3, 0.0));
        assert!(met(kind::KILL_ENEMIES, 6, 0.0) && !met(kind::KILL_ENEMIES, 7, 0.0));
        assert!(met(kind::GRENADE_ENEMIES, 1, 0.0) && !met(kind::GRENADE_ENEMIES, 2, 0.0));
        assert!(!met(kind::MELEE_KILL, 1, 0.0));
        assert!(met(kind::HEADSHOTS, 2, 0.0) && !met(kind::HEADSHOTS, 3, 0.0));
        assert!(met(kind::KILLSTREAK, 3, 0.0) && !met(kind::KILLSTREAK, 4, 0.0));
        // 6 / 4: "at least" 1.5 is met by 1.5 itself.
        assert!(met(kind::KD_RATIO, 0, 1.5) && !met(kind::KD_RATIO, 0, 1.6));
        assert!(met(kind::ACCURACY, 0, 0.25) && !met(kind::ACCURACY, 0, 0.3));
        assert!(met(kind::WIN_SCORE, 7, 0.0) && !met(kind::WIN_SCORE, 8, 0.0));
        assert!(met(kind::DEFEND_ITEMS, 1, 0.0));
        assert!(met(kind::WIN_GAMES, 0, 0.0));

        // The goals that need the win fail without it; the others do not.
        let lost = MatchStats {
            won: false,
            ..stats
        };
        for kind in [
            kind::WIN_GAMES,
            kind::KD_RATIO,
            kind::ACCURACY,
            kind::WIN_SCORE,
            kind::DEFEND_ITEMS,
        ] {
            assert!(quest_of(kind, 0, 0.0).needs_win());
            assert!(!quest_of(kind, 0, 0.0).met_by(&lost), "{kind}");
        }
        assert!(quest_of(kind::KILL_ENEMIES, 6, 0.0).met_by(&lost));
        assert!(quest_of(kind::STEAL_BOX, 1, 0.0).met_by(&lost));
        // No deaths: the kills are the ratio; no shots: no accuracy.
        let clean = MatchStats {
            deaths: 0,
            shots: 0,
            hits: 0,
            ..stats
        };
        assert!(quest_of(kind::KD_RATIO, 0, 6.0).met_by(&clean));
        assert!(!quest_of(kind::KD_RATIO, 0, 6.5).met_by(&clean));
        assert!(!quest_of(kind::ACCURACY, 0, 0.1).met_by(&clean));
        // No kills: no ratio, deaths or not.
        let harmless = MatchStats { kills: 0, ..clean };
        assert!(!quest_of(kind::KD_RATIO, 0, 1.0).met_by(&harmless));
        // Every K/D number of the data is met by the match that has
        // exactly it (11 / 10 for 1.1, and so on).
        for quest in &Quests::shared().quests {
            if quest.kind != kind::KD_RATIO {
                continue;
            }
            let tenths = (quest.y * 10.0).round() as u16;
            let exact = MatchStats {
                kills: tenths,
                deaths: 10,
                ..stats
            };
            assert!(quest.met_by(&exact), "{}", quest.id);
            let short = MatchStats {
                kills: tenths - 1,
                ..exact
            };
            assert!(!quest.met_by(&short), "{}", quest.id);
        }
        let robbed = MatchStats {
            base_robbed: true,
            ..stats
        };
        assert!(!quest_of(kind::DEFEND_ITEMS, 1, 0.0).met_by(&robbed));
        // A Team Deathmatch has no batteries to defend.
        let deathmatch = MatchStats {
            battery_mode: false,
            ..stats
        };
        assert!(!quest_of(kind::DEFEND_ITEMS, 1, 0.0).met_by(&deathmatch));
    }
}
