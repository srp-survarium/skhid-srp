//! The account's side of the 0.26g faction quests: choosing a faction,
//! being given its quests, and being paid for them
//! (`survarium::v026g::quests` has the data and the goals).
//!
//! What 0.26g changed (`lobby_requests.md` §2.1, `lobby.md` §2.14):
//! - The player takes quests from **one faction it chose** (`4B 00
//!   <faction>`; the lobby's "join" button). The server tells it which
//!   (the last byte of 0x8E); 0 makes the client ask the player to choose.
//!   PROVEN.
//! - The quest swap is gone (no `accept_change_quest` in the exe), so a
//!   proposal has no use: every quest the account holds is active.
//!
//! How many quests the faction gives at once, and what leaving a faction
//! does to its quests, is not in the client. srp's rules [choice]:
//! - the chosen faction keeps [`QUESTS_AT_ONCE`] quests going, drawn from
//!   the quests it offers at the account's reputation level with it
//!   ([`Quests::band`]) that are not resting ([`Quest::cooldown_minutes`]);
//! - grenade-kill quests are offered only once a store sells RGD-5
//!   grenades to the account, using the shop's reputation requirements;
//! - artifact quests likewise require store access to gear whose default
//!   modifications add artifact containers;
//! - a quest that runs out (`ttl_minutes`) fails, and another is drawn;
//! - choosing another faction drops the old faction's quests unpaid.
//!
//! The notices are the client's own important-message texts
//! (`social.md` §1.4, type 1 and 4): `#+q` a new quest, `#!q` a failed
//! one, `#q!` a completed one, `#ru` a reputation level reached. Their
//! bracketed numbers are 0.21d's [LIKELY the same: the 0.26g handler
//! 0x226020 tests the same tags; its number parsing was not traced].

use survarium::v026g::drops::Rng;
use survarium::v026g::quests::{MatchStats, Quests, kind};
use survarium::v026g::{dictionary::Dictionary, item_params, modifications::Modifications, prices};

use super::account::{Account, QuestInstance};
use super::pay::QuestKindRates;
use super::rules::{ActionError, faction_unlocked};

/// The quests the chosen faction keeps going (PROVEN: patch note 0.22a,
/// "Number of tasks simultaneously given by a faction increased to 3",
/// unchanged until 0.27a).
pub const QUESTS_AT_ONCE: usize = 3;

/// The important-message types of the notices.
pub const QUEST_MESSAGE: u8 = 1;
pub const REPUTATION_LEVEL_MESSAGE: u8 = 4;

/// An important message for the account's client: `(type, text)`.
pub type Notice = (u8, String);

/// Unix seconds now.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn faction_of(instance: &QuestInstance) -> Option<u8> {
    Quests::shared()
        .quest(instance.quest_id)
        .map(|quest| quest.faction)
}

/// `4B 00`: take quests from `faction`. The old faction's quests are
/// dropped; [`refresh`] then gives the new one's. Choosing the faction it
/// already has changes nothing.
pub fn choose_faction(account: &mut Account, faction: u8) -> Result<(), ActionError> {
    if !faction_unlocked(account, faction) {
        return Err(ActionError::BadFaction);
    }
    if account.quest_faction != faction {
        account.quest_faction = faction;
        account
            .quests
            .retain(|instance| faction_of(instance) == Some(faction));
    }
    Ok(())
}

/// The damaging grenade's item name ([`grenades_available_in_store`]).
pub const GRENADE: &str = "rgd-5";

/// RGD-5 is the damaging grenade in 0.26g; unlocking RDG-M smoke does
/// not make grenade-kill quests available. Use the same seller rules as
/// purchases, independent of the chosen quest faction, balance or loadout.
fn grenades_available_in_store(account: &Account) -> bool {
    Dictionary::shared()
        .items
        .iter()
        .find(|item| item.name == GRENADE)
        .is_some_and(|item| {
            matches!(
                prices::record_for(item.dict_id, 0, |seller| account.reputation_level(seller)),
                Ok(Some(_))
            )
        })
}

/// Container capacity in 0.26g comes from gear's default modifications.
/// Check actual shop records: backpacks that only add pockets do not
/// qualify, and any seller may unlock the quest regardless of its faction.
fn artifact_containers_available_in_store(account: &Account) -> bool {
    let modifications = Modifications::shared();
    item_params::all().iter().any(|item| {
        let has_container = item
            .default_modifications
            .iter()
            .filter_map(|&id| modifications.modification(id))
            .flat_map(|modification| &modification.modifiers)
            .any(|modifier| {
                modifier.path == "parameters/artefact_slots_modifier"
                    && modifier.kind == 1
                    && modifier.value > 0.0
            });
        has_container
            && matches!(
                prices::record_for(item.dict_id, 0, |seller| account.reputation_level(seller)),
                Ok(Some(_))
            )
    })
}

/// Bring the account's quests up to date at `now`: a quest that ran out
/// fails, quests of a faction it no longer takes them from go, and the
/// chosen faction's quests are filled up to [`QUESTS_AT_ONCE`].
pub fn refresh(account: &mut Account, now: u64, rng: &mut Rng) -> Vec<Notice> {
    let quests = Quests::shared();
    let mut notices = Vec::new();
    account.quest_cooldowns.retain(|&(_, until)| until > now);
    let faction = account.quest_faction;
    account.quests.retain(|instance| {
        let known = quests.quest(instance.quest_id);
        if known.is_none_or(|quest| quest.faction != faction) {
            return false;
        }
        if instance.expires_at <= now {
            notices.push((
                QUEST_MESSAGE,
                format!("#!q[{}][{}]", instance.instance_id, instance.quest_id),
            ));
            return false;
        }
        true
    });
    if faction == 0 {
        return notices;
    }
    // TODO: Quest equipment prerequisites should be data-driven. Hardcoding
    // grenade and artifact requirements here is a stopgap; keep these checks
    // for now until prerequisites are represented in the quest data.
    let grenades_available = grenades_available_in_store(account);
    let artifact_containers_available = artifact_containers_available_in_store(account);
    loop {
        if account.quests.len() >= QUESTS_AT_ONCE {
            break;
        }
        let available: Vec<(u32, u32)> = quests
            .band(faction, |with| account.reputation_level(with))
            .filter(|quest| quest.kind != kind::GRENADE_ENEMIES || grenades_available)
            .filter(|quest| quest.kind != kind::FIND_ARTEFACTS || artifact_containers_available)
            .filter(|quest| {
                !account
                    .quest_cooldowns
                    .iter()
                    .any(|&(resting, _)| resting == quest.id)
                    && !account
                        .quests
                        .iter()
                        .any(|instance| instance.quest_id == quest.id)
            })
            .map(|quest| (quest.id, quest.ttl_minutes))
            .collect();
        if available.is_empty() {
            break;
        }
        let (quest_id, ttl) = available[rng.below(available.len() as u32) as usize];
        let instance_id = account.next_quest_instance;
        account.next_quest_instance += 1;
        account.quests.push(QuestInstance {
            instance_id,
            quest_id,
            games: 0,
            expires_at: now + u64::from(ttl) * 60,
        });
        notices.push((QUEST_MESSAGE, format!("#+q[{instance_id}][{quest_id}]")));
    }
    notices
}

/// What a finished match's quests paid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestPay {
    pub money: u32,
    /// `(faction, points)`.
    pub reputation: Vec<(u8, u32)>,
    pub notices: Vec<Notice>,
}

/// What the match a quest was completed in does to its award
/// (`progression_sources.md` §4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    /// The match level's `money_k` and `reputation_k` (`match_levels_dict`).
    /// That a quest's award depends on the level of its match is PROVEN
    /// (patch note 0.23h: "The quest reward now depends on the level of
    /// match, in which the quest was completed"); that these are the
    /// multipliers is LIKELY (they are the match's own, `pay`).
    pub money_k: f64,
    pub reputation_k: f64,
    /// srp's own rates of a quest's silver and reputation
    /// (`pay::quest_money_rate`, `pay::quest_reputation_rate`, as shares:
    /// 1 is the original's), on top of the match level's.
    pub money_share: f64,
    pub reputation_share: f64,
    /// srp's percent for each kind of quest (`pay::quest_kind_rates`), on
    /// top of both shares.
    pub kinds: QuestKindRates,
    /// The match is one of the day's first three victories: the quest's
    /// reputation is paid twice (PROVEN that such a match changes a quest's
    /// award: patch note 0.25a, "Fixed bug in calculation of awards for
    /// quests completed in "x2" matches"; LIKELY that it doubles what the
    /// match itself has doubled, experience and reputation; a 0.26g quest
    /// awards no experience, so only the reputation).
    pub doubled: bool,
}

impl Scale {
    /// The award as the data has it.
    pub const PLAIN: Self = Self {
        money_k: 1.0,
        reputation_k: 1.0,
        money_share: 1.0,
        reputation_share: 1.0,
        kinds: QuestKindRates::NONE,
        doubled: false,
    };

    /// The scale of a quest of `kind`: its percent of
    /// [`Self::kinds`] taken into both shares.
    pub fn of_kind(&self, kind: u32) -> Self {
        let share = f64::from(self.kinds.of(kind)) / 100.0;
        Self {
            money_share: self.money_share * share,
            reputation_share: self.reputation_share * share,
            kinds: QuestKindRates::NONE,
            ..*self
        }
    }

    /// A quest's `(money, reputation)` as this match pays them: each times
    /// its multiplier and its share and rounded down, as the match's own
    /// rows are ([`Self::kinds`] is not applied: [`Self::of_kind`]).
    pub fn of(&self, money: u32, reputation: u32) -> (u32, u32) {
        let scaled = |value: u32, k: f64| {
            (f64::from(value) * k + 1e-6)
                .floor()
                .clamp(0.0, f64::from(u32::MAX)) as u32
        };
        let reputation = scaled(reputation, self.reputation_k * self.reputation_share);
        (
            scaled(money, self.money_k * self.money_share),
            if self.doubled {
                reputation.saturating_mul(2)
            } else {
                reputation
            },
        )
    }
}

/// [`evaluate_scaled`] with the awards as the data has them.
pub fn evaluate(account: &mut Account, stats: &MatchStats, now: u64, rng: &mut Rng) -> QuestPay {
    evaluate_scaled(account, stats, &Scale::PLAIN, now, rng)
}

/// Check the account's quests against a finished match. A quest whose goal
/// the match met is paid (its money and reputation, as `scale` makes
/// them), rests for its cooldown, and is replaced. `win_x_games` counts
/// won battles towards its `max_games_count`.
pub fn evaluate_scaled(
    account: &mut Account,
    stats: &MatchStats,
    scale: &Scale,
    now: u64,
    rng: &mut Rng,
) -> QuestPay {
    let quests = Quests::shared();
    let mut pay = QuestPay {
        notices: refresh(account, now, rng),
        ..QuestPay::default()
    };
    let mut done = Vec::new();
    for instance in &mut account.quests {
        let Some(quest) = quests.quest(instance.quest_id) else {
            continue;
        };
        let met = if quest.kind == kind::WIN_GAMES {
            if stats.won {
                instance.games += 1;
            }
            instance.games >= quest.max_games
        } else {
            quest.met_by(stats)
        };
        if met {
            done.push((instance.instance_id, quest));
        }
    }
    for (instance_id, quest) in done {
        account
            .quests
            .retain(|instance| instance.instance_id != instance_id);
        account
            .quest_cooldowns
            .push((quest.id, now + u64::from(quest.cooldown_minutes) * 60));
        let scale = scale.of_kind(quest.kind);
        let (money, _) = scale.of(quest.money, 0);
        account.generic_money = account.generic_money.saturating_add(money);
        pay.money = pay.money.saturating_add(money);
        pay.notices
            .push((QUEST_MESSAGE, format!("#q![{instance_id}][{}]", quest.id)));
        for &(faction, amount) in &quest.reputation {
            let (_, amount) = scale.of(0, amount);
            for level in account.add_reputation(faction, amount) {
                pay.notices
                    .push((REPUTATION_LEVEL_MESSAGE, format!("#ru[{faction}:{level}]")));
            }
            match pay.reputation.iter_mut().find(|(id, _)| *id == faction) {
                Some(entry) => entry.1 += amount,
                None => pay.reputation.push((faction, amount)),
            }
        }
    }
    pay.notices.extend(refresh(account, now, rng));
    pay
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> Account {
        Account::new("Alice")
    }

    fn reputation_for(faction: u8, level: usize) -> u32 {
        Dictionary::shared()
            .factions
            .iter()
            .find(|entry| entry.id == faction)
            .unwrap()
            .levels[level]
            .value
    }

    /// Leave only the requested kind in the pool, making the result independent
    /// of random selection. Ordinary quest filling is covered below.
    fn quest_account(faction: u8, level: usize, kind: u32) -> Account {
        let mut account = account();
        account.add_reputation(faction, reputation_for(faction, level));
        choose_faction(&mut account, faction).unwrap();
        account.quest_cooldowns = Quests::shared()
            .quests
            .iter()
            .filter(|quest| quest.kind != kind)
            .map(|quest| (quest.id, 100_000))
            .collect();
        assert!(
            Quests::shared()
                .band(faction, |with| account.reputation_level(with))
                .any(|quest| quest.kind == kind)
        );
        account
    }

    fn grenade_quest_account(faction: u8, level: usize) -> Account {
        quest_account(faction, level, kind::GRENADE_ENEMIES)
    }

    #[test]
    fn artifact_quests_wait_until_container_gear_is_available_from_either_seller() {
        for seller in [1, 4] {
            let mut account = quest_account(4, 0, kind::FIND_ARTEFACTS);
            // Edge offers artifact quests at rank 0. The first container
            // gear unlocks at rank 1 with either Scavengers or Edge.
            account.add_reputation(seller, reputation_for(seller, 1) - 1);
            assert!(refresh(&mut account, 1_000, &mut Rng::new(1)).is_empty());
            assert!(account.quests.is_empty());
            account.add_reputation(seller, 1);
            account.generic_money = 0;
            // The store unlock is enough, regardless of money or loadout.
            assert!(!refresh(&mut account, 1_001, &mut Rng::new(1)).is_empty());
            assert!(!account.quests.is_empty());
            assert!(account.quests.iter().all(|instance| {
                let quest = Quests::shared().quest(instance.quest_id).unwrap();
                quest.kind == kind::FIND_ARTEFACTS && quest.faction == 4
            }));
        }
    }

    #[test]
    fn stores_without_artifact_containers_do_not_unlock_artifact_quests() {
        let mut account = quest_account(4, 0, kind::FIND_ARTEFACTS);
        account.add_reputation(2, reputation_for(2, 5));
        account.add_reputation(3, reputation_for(3, 10));
        // These stores offer backpacks and pockets, but no artifact slots.
        assert!(refresh(&mut account, 1_000, &mut Rng::new(1)).is_empty());
        assert!(account.quests.is_empty());
    }

    #[test]
    fn owned_container_gear_does_not_bypass_the_store_unlock_requirement() {
        let mut account = quest_account(4, 0, kind::FIND_ARTEFACTS);
        let container = Dictionary::shared().item_named("scav_back_1").unwrap();
        account.add_item(container.dict_id, 1);
        assert!(refresh(&mut account, 1_000, &mut Rng::new(1)).is_empty());
        assert!(account.quests.is_empty());
    }

    #[test]
    fn grenade_quests_wait_until_a_store_seller_is_unlocked() {
        for seller in [1, 2] {
            let mut account = grenade_quest_account(seller, 2);
            // Even one point short of rank 3, the RGD-5 is unavailable.
            account.add_reputation(
                seller,
                reputation_for(seller, 3) - 1 - account.reputation(seller),
            );
            assert!(refresh(&mut account, 1_000, &mut Rng::new(1)).is_empty());
            assert!(account.quests.is_empty());
            account.add_reputation(seller, 1);
            account.generic_money = 0;
            // Availability is enough: the player need not own or equip
            // grenades, or currently have the money to buy one.
            assert!(!refresh(&mut account, 1_001, &mut Rng::new(1)).is_empty());
            assert!(!account.quests.is_empty());
            assert!(account.quests.iter().all(|instance| {
                Quests::shared().quest(instance.quest_id).unwrap().kind == kind::GRENADE_ENEMIES
            }));
        }
    }

    #[test]
    fn grenade_quest_availability_uses_either_seller_not_the_quest_faction() {
        for quest_faction in [3, 4] {
            for seller in [1, 2] {
                let mut account = grenade_quest_account(quest_faction, 1);
                // Rank 1 here unlocks smoke grenades, not RGD-5.
                assert!(refresh(&mut account, 1_000, &mut Rng::new(1)).is_empty());
                account.add_reputation(seller, reputation_for(seller, 3));
                assert!(!refresh(&mut account, 1_001, &mut Rng::new(1)).is_empty());
                assert!(account.quests.iter().all(|instance| {
                    Quests::shared().quest(instance.quest_id).unwrap().faction == quest_faction
                }));
            }
        }
    }

    #[test]
    fn owned_grenades_do_not_bypass_the_store_unlock_requirement() {
        let mut account = grenade_quest_account(1, 2);
        let grenade = Dictionary::shared()
            .items
            .iter()
            .find(|item| item.name == "rgd-5")
            .unwrap()
            .dict_id;
        account.add_item(grenade, 10);
        assert!(refresh(&mut account, 1_000, &mut Rng::new(1)).is_empty());
        assert!(account.quests.is_empty());
    }

    #[test]
    fn no_faction_no_quests() {
        let mut account = account();
        assert!(refresh(&mut account, 1_000, &mut Rng::new(1)).is_empty());
        assert!(account.quests.is_empty());
        assert_eq!(
            choose_faction(&mut account, 0),
            Err(ActionError::BadFaction)
        );
        assert_eq!(
            choose_faction(&mut account, 5),
            Err(ActionError::BadFaction)
        );
    }

    #[test]
    fn the_chosen_faction_gives_its_quests() {
        let quests = Quests::shared();
        let mut account = account();
        choose_faction(&mut account, 2).unwrap();
        let notices = refresh(&mut account, 1_000, &mut Rng::new(7));
        assert_eq!(account.quests.len(), QUESTS_AT_ONCE);
        assert_eq!(notices.len(), QUESTS_AT_ONCE);
        let mut ids = Vec::new();
        for (instance, notice) in account.quests.iter().zip(&notices) {
            let quest = quests.quest(instance.quest_id).unwrap();
            // Its own faction's, of the level 0 band, each once.
            assert_eq!(quest.faction, 2);
            assert_eq!((quest.min_level, quest.max_level), (0, 0));
            assert!(!ids.contains(&quest.id));
            ids.push(quest.id);
            assert_eq!(
                instance.expires_at,
                1_000 + u64::from(quest.ttl_minutes) * 60
            );
            assert_eq!(
                *notice,
                (
                    QUEST_MESSAGE,
                    format!("#+q[{}][{}]", instance.instance_id, quest.id)
                )
            );
        }
        // Up to date: nothing more happens.
        assert!(refresh(&mut account, 1_001, &mut Rng::new(7)).is_empty());
        // The same faction again keeps them.
        let held = account.quests.clone();
        choose_faction(&mut account, 2).unwrap();
        assert_eq!(account.quests, held);
        // Another faction drops them; its own come.
        choose_faction(&mut account, 1).unwrap();
        assert!(account.quests.is_empty());
        refresh(&mut account, 1_002, &mut Rng::new(7));
        assert_eq!(account.quests.len(), QUESTS_AT_ONCE);
        assert!(
            account
                .quests
                .iter()
                .all(|instance| faction_of(instance) == Some(1))
        );
        // Instance ids are never reused.
        assert!(
            account
                .quests
                .iter()
                .all(|instance| instance.instance_id > QUESTS_AT_ONCE as u32)
        );
    }

    #[test]
    fn a_quest_that_runs_out_fails_and_is_replaced() {
        let mut account = account();
        choose_faction(&mut account, 1).unwrap();
        refresh(&mut account, 1_000, &mut Rng::new(3));
        let first = account.quests[0];
        account.quests[0].expires_at = 2_000;
        let notices = refresh(&mut account, 2_000, &mut Rng::new(3));
        assert_eq!(
            notices[0],
            (
                QUEST_MESSAGE,
                format!("#!q[{}][{}]", first.instance_id, first.quest_id)
            )
        );
        assert_eq!(account.quests.len(), QUESTS_AT_ONCE);
        assert!(
            account
                .quests
                .iter()
                .all(|instance| instance.instance_id != first.instance_id)
        );
    }

    #[test]
    fn a_won_match_pays_the_quests_it_meets() {
        let quests = Quests::shared();
        let mut account = account();
        choose_faction(&mut account, 1).unwrap();
        // Give it one known quest: Scavengers' first, "defend the
        // batteries" (won, base not robbed), 1 game.
        let quest = quests.quest(10101).unwrap();
        account.quests.push(QuestInstance {
            instance_id: 50,
            quest_id: quest.id,
            games: 0,
            expires_at: 10_000,
        });
        account.next_quest_instance = 51;
        let money = account.generic_money;
        // A lost match pays nothing.
        let lost = MatchStats::default();
        let pay = evaluate(&mut account, &lost, 1_000, &mut Rng::new(5));
        assert_eq!(pay.money, 0);
        assert!(
            account
                .quests
                .iter()
                .any(|instance| instance.instance_id == 50)
        );

        let won = MatchStats {
            won: true,
            battery_mode: true,
            ..MatchStats::default()
        };
        let pay = evaluate(&mut account, &won, 1_100, &mut Rng::new(5));
        assert!(pay.money >= quest.money);
        assert!(
            pay.notices
                .contains(&(QUEST_MESSAGE, "#q![50][10101]".to_owned()))
        );
        assert_eq!(account.generic_money, money + pay.money);
        // Paid in reputation with its own faction.
        let (faction, points) = quest.reputation[0];
        assert_eq!(faction, 1);
        assert!(account.reputation(1) >= points);
        assert!(pay.reputation.iter().any(|&(id, _)| id == 1));
        // It rests, and is not given again while it does.
        assert!(
            account
                .quest_cooldowns
                .contains(&(10101, 1_100 + u64::from(quest.cooldown_minutes) * 60))
        );
        assert!(
            account
                .quests
                .iter()
                .all(|instance| instance.quest_id != 10101)
        );
        assert_eq!(account.quests.len(), QUESTS_AT_ONCE);
    }

    #[test]
    fn the_match_scales_a_quests_award() {
        let quests = Quests::shared();
        let quest = quests.quest(10101).unwrap();
        let (faction, points) = quest.reputation[0];
        let won = MatchStats {
            won: true,
            battery_mode: true,
            ..MatchStats::default()
        };
        let paid = |scale: &Scale| {
            let mut account = account();
            choose_faction(&mut account, 1).unwrap();
            account.quests.push(QuestInstance {
                instance_id: 50,
                quest_id: quest.id,
                games: 0,
                expires_at: 10_000,
            });
            account.next_quest_instance = 51;
            let (money, reputation) = (account.generic_money, account.reputation(faction));
            let mut plain = account.clone();
            let base = evaluate(&mut plain, &won, 1_000, &mut Rng::new(5));
            let pay = evaluate_scaled(&mut account, &won, scale, 1_000, &mut Rng::new(5));
            assert_eq!(account.generic_money, money + pay.money);
            let got: u32 = pay
                .reputation
                .iter()
                .filter(|&&(id, _)| id == faction)
                .map(|&(_, points)| points)
                .sum();
            assert_eq!(account.reputation(faction), reputation + got);
            let base_got: u32 = base
                .reputation
                .iter()
                .filter(|&&(id, _)| id == faction)
                .map(|&(_, points)| points)
                .sum();
            ((base.money, base_got), (pay.money, got))
        };
        // The plain scale changes nothing.
        let (base, same) = paid(&Scale::PLAIN);
        assert_eq!(base, same);
        assert!(base.0 >= quest.money && base.1 >= points);
        // Match level 2: money twice, reputation 1.05 times, rounded down
        // a quest; then twice in one of the day's doubled matches.
        let level = survarium::v026g::dictionary::Dictionary::shared()
            .match_level(2)
            .unwrap();
        let scale = Scale {
            money_k: crate::v026g::pay::multiplier(level.money_k),
            reputation_k: crate::v026g::pay::multiplier(level.reputation_k),
            ..Scale::PLAIN
        };
        assert_eq!(scale.of(100, 100), (200, 105));
        assert_eq!(scale.of(quest.money, points).0, 2 * quest.money);
        let (base, scaled) = paid(&scale);
        assert_eq!(scaled.0, 2 * base.0);
        assert!(scaled.1 >= base.1 && scaled.1 <= base.1 * 105 / 100);
        let doubled = Scale {
            doubled: true,
            ..scale
        };
        assert_eq!(doubled.of(100, 100), (200, 210));
        let (_, twice) = paid(&doubled);
        assert_eq!(twice, (scaled.0, 2 * scaled.1));
        // srp's rate of reputation: 120 % of 105, and of every award of
        // the data at every match level a whole number comes out whole.
        let raised = Scale {
            reputation_share: 1.2,
            ..scale
        };
        assert_eq!(raised.of(100, 100), (200, 126));
        assert_eq!(
            Scale {
                doubled: true,
                ..raised
            }
            .of(100, 100),
            (200, 252)
        );
        assert_eq!(
            Scale {
                reputation_share: 1.2,
                ..Scale::PLAIN
            }
            .of(50, 50),
            (50, 60)
        );
        // srp's rate of a quest's silver, apart from its reputation's.
        let paid_more = Scale {
            money_share: 1.5,
            ..raised
        };
        assert_eq!(paid_more.of(100, 100), (300, 126));
        let (_, more) = paid(&paid_more);
        // 50 × 2 × 1.5 silver, 50 × 1.05 × 1.2 reputation.
        assert_eq!(more, (150, 63));
    }

    #[test]
    fn a_kind_of_quest_can_pay_more() {
        // Quest 10101 is a `defend_items`; 300 % for it, 200 % for another
        // kind.
        let quest = Quests::shared().quest(10101).unwrap();
        assert_eq!(quest.kind, kind::DEFEND_ITEMS);
        let kinds = QuestKindRates::parse("defend_items=300,kill_enemies=200").unwrap();
        let scale = Scale {
            money_share: 1.5,
            reputation_share: 1.2,
            kinds,
            ..Scale::PLAIN
        };
        // Without the kind: the two shares alone.
        assert_eq!(scale.of(100, 100), (150, 120));
        assert_eq!(scale.of_kind(kind::DEFEND_ITEMS).of(100, 100), (450, 360));
        assert_eq!(scale.of_kind(kind::KILL_ENEMIES).of(100, 100), (300, 240));
        assert_eq!(scale.of_kind(kind::WIN_GAMES).of(100, 100), (150, 120));
        // Taken once: a scale of a kind has no kinds left.
        let of_kind = scale.of_kind(kind::DEFEND_ITEMS);
        assert_eq!(of_kind.of_kind(kind::DEFEND_ITEMS), of_kind);

        let mut account = account();
        choose_faction(&mut account, 1).unwrap();
        account.quests.push(QuestInstance {
            instance_id: 50,
            quest_id: quest.id,
            games: 0,
            expires_at: 10_000,
        });
        account.next_quest_instance = 51;
        let won = MatchStats {
            won: true,
            battery_mode: true,
            ..MatchStats::default()
        };
        let pay = evaluate_scaled(&mut account, &won, &scale, 1_000, &mut Rng::new(5));
        // 50 silver and 50 reputation: 4.5 and 3.6 times.
        assert_eq!((quest.money, quest.reputation[0]), (50, (1, 50)));
        assert_eq!(pay.money, 225);
        assert_eq!(pay.reputation, [(1, 180)]);
    }

    #[test]
    fn reaching_a_reputation_level_is_announced() {
        let quests = Quests::shared();
        let mut account = account();
        choose_faction(&mut account, 1).unwrap();
        let quest = quests.quest(10101).unwrap();
        let (_, points) = quest.reputation[0];
        assert!(points > 0);
        // So short of level 1 (100 points with the Scavengers) that the
        // quest's award reaches it.
        account.add_reputation(1, 100 - points.min(100));
        assert_eq!(account.reputation_level(1), 0);
        account.quests.push(QuestInstance {
            instance_id: 50,
            quest_id: quest.id,
            games: 0,
            expires_at: 10_000,
        });
        account.next_quest_instance = 51;
        let won = MatchStats {
            won: true,
            battery_mode: true,
            ..MatchStats::default()
        };
        let pay = evaluate(&mut account, &won, 1_000, &mut Rng::new(5));
        assert!(
            pay.notices
                .contains(&(REPUTATION_LEVEL_MESSAGE, "#ru[1:1]".to_owned()))
        );
        assert!(account.reputation_level(1) >= 1);
    }
}
