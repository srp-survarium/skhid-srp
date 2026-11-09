//! No tutorial (the owner's of 2026-10-19): it took a new player through
//! Play into a match, and with rooms nobody is sent into a match they did
//! not choose. The server tells every client that every step is done, so
//! the client runs none of it; a new account starts with more instead
//! ([`signup_grant`]). `docs/notes/lobby.md` §2.21.
//!
//! The client runs the tutorial itself from the state the server sends.
//!
//! `9D` is `bool show, u32 a, u32 b`. With `show` the client's tutorial
//! leaves its disabled state by `a` (0x283190, PROVEN):
//!
//! | `a` | state |
//! |---|---|
//! | bit 0 clear | `tutorial_state_choose_nickname` |
//! | bit 0 set, `b` == 0 | `tutorial_state_click_inventory` |
//! | bit 0 set, `b` != 0 | `tutorial_state_equip_drop` |
//! | bit 1 or bit 5 set (`a & 0x22`) | `default_state`: no tutorial |
//!
//! The other bits are the steps the client reported with `4B 01 <step>`:
//! **bit `n` is step `n`** (PROVEN for 2 and 3: the survival notes' reward
//! is shown while `a >> 3 & 1` (notes 0, "prepare to battle", step 3) or
//! `a >> 2 & 1` (notes 1, "items", step 2) is clear, 0x2833F1; for 4: the
//! match's own tutorial is skipped when `a & 0x10`, 0x27AE46; for 1:
//! `a >> 1 & 1` opens `state_choose_        .find(|row| row.id == faction)`, 0x28E9AD). Bit 0 is a
//! nickname chosen and bit 5 the tutorial skipped (`4B 02`), LIKELY.
//!
//! So srp sends [`ALL_DONE`]: `a & 0x22` puts the lobby in its
//! `default_state`, bit 4 skips the match's own tutorial, bits 2 and 3 hide
//! the notes' rewards. `9D` with `show` off stores the same flags and runs
//! no tutorial (0x27136E): the sign-in's `9D` has `show` on, so the client
//! leaves its disabled state for `default_state`, as after a finished
//! tutorial; the ranking's recounts send it off. A client that sends a
//! step anyway (`4B`, `46 0A`) is not answered.
//!
//! **What the steps paid**, the game's own
//! `gameplay/awards/unique_awards.options` ([`survarium::v026g::awards`]):
//! `tutorial_completed` 1000 silver and 250 experience, the two notes
//! 25 gold each, the match's tutorial 50 gold. **A new account starts**
//! with what the game data's table says
//! ([`survarium::v026g::new_account`]): its
//! level and skills, money, premium, the rounds of every ammo
//! ([`starter_ammo`]), its reputation and quest faction, what it owns and
//! what its three kits wear (the owner's of 2026-10-19, 2026-10-24 and
//! 2026-11-06, `docs/notes/no-tutorial.md`). It is the account's state
//! when the client first hears of it: no window, no notice.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::item_params::{self, shop_unit};
use survarium::v026g::loadouts::POCKET_SLOTS;
use survarium::v026g::lobby::requests::{INVENTORY_SLOT, Relocation};
use survarium::v026g::lobby::{self, FULL_CONDITION, ServerMessage};
use survarium::v026g::modifications::Modifications;
use survarium::v026g::new_account::Preset;
pub use survarium::v026g::new_account::shared as new_account;

use super::account::{Account, KITS, level_start};

/// The bits of the tutorial's state (`a` of `9D`).
pub mod flag {
    pub const NICKNAME: u32 = 1 << 0;
    pub const COMPLETED: u32 = 1 << 1;
    pub const ITEM_NOTES: u32 = 1 << 2;
    pub const BATTLE_NOTES: u32 = 1 << 3;
    pub const GAME_WORLD: u32 = 1 << 4;
    pub const SKIPPED: u32 = 1 << 5;
}

/// Every step done: the client runs no tutorial (see the module).
pub const ALL_DONE: u32 = flag::NICKNAME
    | flag::COMPLETED
    | flag::ITEM_NOTES
    | flag::BATTLE_NOTES
    | flag::GAME_WORLD
    | flag::SKIPPED;

/// Where the ammo lives: every item of the dictionary under it is ammo.
pub const AMMO_CONFIGS: &str = "gameplay/items/weapons/ammo/";

/// Every ammo a player can own: the dictionary's stacks under
/// [`AMMO_CONFIGS`], ported weapons' too, by dict id.
pub fn starter_ammo(dictionary: &Dictionary) -> Vec<u16> {
    dictionary
        .items
        .iter()
        .filter(|item| item.is_stack && item.cfg_name.starts_with(AMMO_CONFIGS))
        .map(|item| item.dict_id)
        .collect()
}

/// The ammo bags of a kit whose worn items have `modifications` (their
/// defaults and their own): the player's (`default.player`
/// `ammo_bags_count`, 3) and what the modifications add
/// (`parameters/ammo_bags_modifier`, 1050 and 1051), the lobby's
/// `PlayerInfo.ammoBags` for an account of no skills.
pub fn ammo_bags(modifications: impl IntoIterator<Item = u32>) -> u32 {
    let table = Modifications::shared();
    let added: f32 = modifications
        .into_iter()
        .filter_map(|id| table.modification(id))
        .flat_map(|row| &row.modifiers)
        .filter(|modifier| modifier.path == "parameters/ammo_bags_modifier")
        .map(|modifier| modifier.value)
        .sum();
    (item_params::player().ammo_bags_count as f32 + added).max(0.0) as u32
}

/// The most rounds of `ammo` an ammo slot takes with `bags` ammo bags: what
/// the ammo window fills it to, `int(bags × clips a bag) × clip_size`
/// (`AmmoShareElement`, clips a bag `1 / required_bags`, `ItemFactory`).
pub fn full_rounds(ammo: u16, bags: u32) -> u32 {
    let required = item_params::item_params(ammo).map_or(1, |params| params.required_bags.max(1));
    bags / required * shop_unit(ammo)
}

/// [`ammo_bags`] of kit `kit` (0 on) of the grant's `preset`: its worn
/// items' default modifications and their own
/// ([`survarium::v026g::new_account::NewAccount::modifications`]).
pub fn starter_ammo_bags(dictionary: &Dictionary, preset: &Preset, kit: usize) -> u32 {
    let grant = new_account();
    let worn: Vec<&str> = preset.kit(kit).map(|(_, name)| name).collect();
    let defaults = worn
        .iter()
        .filter_map(|name| dictionary.item_named(name))
        .flat_map(|item| {
            item_params::item_params(item.dict_id)
                .map_or(&[][..], |params| params.default_modifications)
                .iter()
                .copied()
        });
    let own = grant
        .modifications
        .iter()
        .filter(|(name, _)| worn.contains(&name.as_str()))
        .map(|&(_, id)| id);
    ammo_bags(defaults.chain(own))
}

/// The ammo bags of ammo slot `slot` of kit `kit` (0 on) of `preset`: the
/// kit's ([`starter_ammo_bags`]) shared by its ammo slots (the stacks it
/// wears out of the pockets), one for each after the first, the first the
/// rest.
pub fn starter_slot_bags(dictionary: &Dictionary, preset: &Preset, kit: usize, slot: u8) -> u32 {
    let ammo: Vec<u8> = preset
        .kit(kit)
        .filter(|(at, name)| {
            !POCKET_SLOTS.contains(at)
                && dictionary
                    .item_named(name)
                    .is_some_and(|item| item.is_stack)
        })
        .map(|(at, _)| at)
        .collect();
    let first = ammo.iter().copied().min();
    let bags = starter_ammo_bags(dictionary, preset, kit);
    if first == Some(slot) {
        bags.saturating_sub(ammo.len() as u32 - 1)
    } else {
        1
    }
}

/// The experience an account starts with: one point short of the level
/// after the table's (the owner's of 2026-11-06: the last point of level
/// 15, so its first match brings it to 16).
pub fn start_experience() -> u32 {
    level_start(new_account().level.saturating_add(1)) - 1
}

/// A new account's start, before its client first hears of it
/// ([`new_account`]): its level and skills, its money, the rounds of every
/// ammo, the faction's reputation level and its quests (the tutorial ended
/// in the faction's choice; the owner's of 2026-10-19: never neutral), what
/// it owns and what its three kits wear ([`wear_starter_kit`]), and its
/// premium from `now` (Unix seconds).
pub fn signup_grant(account: &mut Account, now: u64) {
    signup_grant_with(account, true, now);
}

/// [`signup_grant`], its level and skills, what it owns, the kits and the
/// premium given only with `starter_kit` (off only in the lobby's tests,
/// `GameAccounts::set_starter_kit`).
pub fn signup_grant_with(account: &mut Account, starter_kit: bool, now: u64) {
    let grant = new_account();
    let faction = grant.faction.id();
    let dictionary = Dictionary::shared();
    // The reputation first: the quests are given by its level.
    let points = dictionary
        .factions
        .iter()
        .find(|row| row.id == faction)
        .and_then(|row| row.levels.get(usize::from(grant.reputation_level)))
        .expect("the faction's level is there (new_account::check_data)")
        .value;
    account.add_reputation(faction, points.saturating_sub(account.reputation(faction)));
    // A faction whose quests the lobby shows (`QuestFaction`).
    super::quests::choose_faction(account, faction).expect("every quest faction is open");
    account.generic_money = grant.silver;
    if starter_kit {
        super::premium::add_premium(account, u64::from(grant.premium_days) * 86_400, now);
        // The levels below it pay nothing. Before the kits: they carry the
        // skills' modifiers.
        account.exp_total = start_experience();
        account.skills = grant.skills.clone();
    }
    for dict_id in starter_ammo(dictionary) {
        let owned = account
            .inventory
            .iter()
            .find(|item| item.dict_id == dict_id)
            .map_or(0, |item| item.condition_or_stack);
        if owned < grant.rounds {
            account.add_to_stack(dict_id, grant.rounds - owned);
        }
    }
    if starter_kit {
        wear_starter_kit(account, dictionary);
    }
}

/// What the grant owns (`NewAccount::own`), then its presets' kits worn by
/// the lobby's own rule of a kit put on (`rules::fill_kit`): the saved
/// presets' kits each kept as that preset ([`presets::snapshot`]), then
/// preset 1's, the kits. Each item worn is bought once and every kit and
/// preset that wears it names that one, as a player's own kits share the
/// inventory; a stack worn is the one owned: an ammo slot filled to the
/// most its share of the kit's bags takes ([`starter_slot_bags`],
/// [`full_rounds`]), a pocket with what one of the preset's `pocket_units` holds of it. Every item
/// given, owned or worn, has the modifications of its own the grant names
/// for it (`NewAccount::modifications`) before a kit wears it (they may
/// open a pocket). An item the game data lacks is left out: srp starts
/// only on data with all of them (`known::check_code`); the test data
/// predates the Keeper of the Sun, the Novice's backpack and more.
pub fn wear_starter_kit(account: &mut Account, dictionary: &Dictionary) {
    let grant = new_account();
    // The items given, owned and bought for the kits, `(dict id, item id)`.
    let mut given: Vec<(u16, u32)> = Vec::new();
    for (name, units) in &grant.own {
        let Some(item) = dictionary.item_named(name) else {
            continue;
        };
        if item.is_stack {
            let owned = account
                .inventory
                .iter()
                .find(|owned| owned.dict_id == item.dict_id)
                .map_or(0, |owned| owned.condition_or_stack);
            account.add_to_stack(item.dict_id, units.saturating_sub(owned));
        } else {
            for _ in 0..*units {
                given.push((item.dict_id, account.add_item(item.dict_id, FULL_CONDITION)));
            }
        }
    }
    // The items bought for the kits, once each, `(dict id, item id)`.
    let mut bought: Vec<(u16, u32)> = Vec::new();
    for name in grant
        .presets()
        .flat_map(|preset| (0..KITS).flat_map(move |kit| preset.kit(kit)))
        .map(|(_, name)| name)
    {
        let Some(item) = dictionary.item_named(name) else {
            log::warn!("starter kit: the game data has no {name}; left out");
            continue;
        };
        if !item.is_stack && bought.iter().all(|(id, _)| *id != item.dict_id) {
            bought.push((item.dict_id, account.add_item(item.dict_id, FULL_CONDITION)));
        }
    }
    for (name, modification) in &grant.modifications {
        let Some(dict_id) = dictionary.item_named(name).map(|item| item.dict_id) else {
            continue;
        };
        for &(_, item_id) in given.iter().chain(&bought).filter(|(id, _)| *id == dict_id) {
            if let Some(item) = account.item_mut(item_id) {
                item.modifications.push((*modification, false));
            }
        }
    }
    let wear = |account: &mut Account, number: usize, preset: &Preset| {
        for kit in 0..KITS {
            let mut moves: Vec<Relocation> = Vec::new();
            for (slot, name) in preset.kit(kit) {
                let Some(item) = dictionary.item_named(name) else {
                    continue;
                };
                let (item_id, count) = if !item.is_stack {
                    let (_, item_id) = bought
                        .iter()
                        .find(|(id, _)| *id == item.dict_id)
                        .expect("bought above");
                    (*item_id, 1)
                } else if POCKET_SLOTS.contains(&slot) {
                    (
                        account.add_to_stack(item.dict_id, 0),
                        preset.pocket_units_of(item.dict_id),
                    )
                } else {
                    let bags = starter_slot_bags(dictionary, preset, kit, slot);
                    let rounds = full_rounds(item.dict_id, bags);
                    (account.add_to_stack(item.dict_id, 0), rounds)
                };
                moves.push(Relocation {
                    item_id,
                    dict_id: item.dict_id,
                    source_slot: INVENTORY_SLOT,
                    target_slot: slot,
                    count,
                    count_in_inventory: 0,
                });
            }
            if let Err(error) = super::rules::fill_kit(account, kit, &moves) {
                log::error!(
                    "starter kit: preset {number} kit {} could not wear it ({error})",
                    kit + 1
                );
            }
        }
    };
    for (index, preset) in grant.saved.iter().enumerate() {
        wear(account, index + 2, preset);
        account.presets[index + 1] = super::presets::snapshot(account);
    }
    wear(account, 1, &grant.active);
    account.sync_kits();
}

/// `9D`: every step done (see the module). `show` on at sign-in, off for
/// the ranking's recounts.
pub fn message(show: bool) -> ServerMessage {
    lobby::tutorial(show, ALL_DONE, 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::loadouts::{KitLayout, Loadout, SlotItem};
    use survarium::v026g::lobby::client_model::{Client, Decoded};
    use survarium::v026g::lobby::tutorial_model::State;

    #[test]
    fn the_client_runs_no_tutorial() {
        let message = message(true);
        assert_eq!(message.id_byte(), 0x9D);
        assert_eq!(message.body(), [1, 0x3F, 0, 0, 0, 1, 0, 0, 0]);
        let mut client = Client::signed_in();
        assert!(matches!(
            client.receive(&message).unwrap(),
            Decoded::Tutorial {
                show: true,
                a: 0x3F,
                b: 1
            }
        ));
        assert_eq!(client.tutorial().state(), State::Default);
        assert!(!client.tutorial().is_modal());
        assert_eq!(super::message(false).body()[0], 0);
    }

    /// A fresh account on the test data, made as `GameAccounts::sign_up`
    /// makes it: everything, but the Keeper of the Sun and the Novice's backpack of
    /// patches the test data predates (srp refuses such data,
    /// `known::check_code`). The whole kit by its dict ids on released
    /// data: `tests/starter_kit.rs`.
    #[test]
    fn a_new_account_starts_with_the_starter_kit() {
        use super::super::account::KIT_SLOTS;
        use survarium::v026g::item_params::item_params;

        let grant = new_account();
        const NOW: u64 = 1_800_000_000;
        let mut account = Account::with_loadout(
            "Alice",
            &survarium::v026g::loadouts::Loadout { items: Vec::new() },
        );
        signup_grant(&mut account, NOW);
        assert_eq!(account.generic_money, grant.silver);
        // 30 days of premium from the making, 720 hours as the lobby is
        // told.
        assert_eq!(account.premium_until, NOW + 30 * 86_400);
        assert!(account.is_premium(NOW) && account.is_premium(NOW + 30 * 86_400 - 1));
        assert!(!account.is_premium(NOW + 30 * 86_400));
        assert_eq!(account.premium_hours(NOW), Some(720));
        assert_eq!(account.level(), grant.level);
        assert_eq!(account.exp_total, level_start(16) - 1);
        assert_eq!(account.skills, grant.skills);
        assert_eq!(account.free_skill_points(), 0);
        assert_eq!(account.quest_faction, grant.faction.id());

        // The Scavengers at 3000 points: level 5, their highest.
        let dictionary = Dictionary::shared();
        assert_eq!(account.reputation(grant.faction.id()), 3000);
        assert_eq!(account.reputation_level(grant.faction.id()), 5);
        let scavengers = dictionary.factions.iter().find(|f| f.id == 1).unwrap();
        assert_eq!(scavengers.levels.len(), 6);
        for faction in [2, 3, 4] {
            assert_eq!(account.reputation(faction), 0);
        }

        // Exactly the table's rounds of every ammo, one stack each.
        let ammo = starter_ammo(dictionary);
        for name in [
            "ammo_7.62x25",
            "ammo_7.62x54r_old",
            "ammo_7.62x39",
            "ammo_9x18_makarov",
        ] {
            let dict_id = dictionary.item_named(name).unwrap().dict_id;
            assert!(ammo.contains(&dict_id), "{name}");
        }
        let stacks = |dict_id: u16| -> Vec<u32> {
            account
                .inventory
                .iter()
                .filter(|item| item.dict_id == dict_id)
                .map(|item| item.condition_or_stack)
                .collect()
        };
        for &dict_id in &ammo {
            assert_eq!(item_params(dict_id).unwrap().kind, "ammo");
            assert_eq!(stacks(dict_id), [grant.rounds], "{dict_id}");
        }
        // The modifications of its own a given item has: those it comes
        // with (items.tsv `start`), then the grant's.
        let own_modifications = |name: &str| -> Vec<(u32, bool)> {
            let dict_id = dictionary.item_named(name).unwrap().dict_id;
            let mut own = survarium::v026g::golden::starting_modifications(dict_id);
            own.extend(
                grant
                    .modifications
                    .iter()
                    .filter(|(piece, _)| piece == name)
                    .map(|&(_, modification)| (modification, false)),
            );
            own
        };
        // What it owns: the free weapons and the AKs, unworn, the stash
        // maps, and the stacks the pockets take theirs from (the inventory
        // shows them whole). The test data predates the AKMSU and the Glock 17.
        for (name, units) in &grant.own {
            let Some(item) = dictionary.item_named(name) else {
                assert!(["akmsu", "glock_17"].contains(&name.as_str()), "{name}");
                continue;
            };
            let owned = stacks(item.dict_id);
            if item.is_stack {
                assert_eq!(owned, [*units], "{name}");
            } else {
                assert_eq!(owned, vec![FULL_CONDITION; *units as usize], "{name}");
                let worn = account
                    .inventory
                    .iter()
                    .filter(|it| it.dict_id == item.dict_id);
                for it in worn {
                    assert!(
                        account
                            .kits
                            .iter()
                            .all(|kit| kit.slot_of(it.item_id).is_none())
                    );
                    assert_eq!(it.modifications, own_modifications(name), "{name}");
                }
            }
        }

        // Preset 1's ammo slots full: the player's three bags, three clips
        // of one bag each (no bag modification on its gear); preset 2's
        // five (the Keeper vest's and pants' 1050), all for its one weapon.
        let [active, keeper] = [&grant.active, &grant.saved[0]];
        for kit in 0..KITS {
            assert_eq!(starter_ammo_bags(dictionary, active, kit), 3);
            assert_eq!(starter_slot_bags(dictionary, active, kit, 8), 3);
        }
        // Preset 2 on the test data: kits 1 and 2 without the Keeper of the
        // Sun, kit 3 wearing nothing: the player's three.
        let bags: Vec<u32> = (0..KITS)
            .map(|kit| starter_ammo_bags(dictionary, keeper, kit))
            .collect();
        assert_eq!(bags, [3, 3, 3]);
        assert_eq!(starter_slot_bags(dictionary, keeper, 0, 8), 3);
        // A kit of two weapons shares its bags: a TT-33 with its ammo in
        // weapon slot 2 of kit 1 takes one, the AK-12's ammo the rest.
        let mut two = keeper.clone();
        two.kits[0].extend([(10, "tt_33".into()), (11, "ammo_7.62x25".into())]);
        assert_eq!(starter_slot_bags(dictionary, &two, 0, 8), 2);
        assert_eq!(starter_slot_bags(dictionary, &two, 0, 11), 1);
        let rounds =
            |ammo: &str, bags| full_rounds(dictionary.item_named(ammo).unwrap().dict_id, bags);
        assert_eq!(rounds("ammo_7.62x51", 3), 30);
        assert_eq!(rounds("ammo_12mm_buck", 3), 45);
        assert_eq!(rounds("ammo_6.5x39", 3), 90);
        assert_eq!(rounds("ammo_7.62x25", 1), 50);
        // A bag modification counts: 1051 adds two.
        assert_eq!(ammo_bags([1030, 1051]), 5);
        // Each preset's kits: preset 1 the kits themselves, preset 2 kept
        // (`Account::presets`), as the grant says.
        assert_eq!(account.active_preset, 1);
        let kept = [
            super::super::presets::snapshot(&account).unwrap(),
            account.presets[1].unwrap(),
        ];
        assert!(account.presets.iter().skip(2).all(Option::is_none));
        let mut missing = Vec::new();
        for (number, (preset, kept)) in (1..).zip(grant.presets().zip(kept)) {
            for (index, kit) in kept.kits.iter().enumerate() {
                // The test data lacks some of the gear (below): a kit's
                // pockets are what the gear it has opens.
                let gear = Loadout {
                    items: preset
                        .kit(index)
                        .filter_map(|(slot, name)| {
                            let item = dictionary.item_named(name)?;
                            let dict_id = item.dict_id;
                            (!item.is_stack).then_some(SlotItem {
                                slot,
                                dict_id,
                                count: 1,
                            })
                        })
                        .collect(),
                };
                let own: Vec<(u32, bool)> = gear
                    .items
                    .iter()
                    .map(|item| &dictionary.item(item.dict_id).unwrap().name)
                    .flat_map(|name| own_modifications(name))
                    .collect();
                let layout = KitLayout::of_loadout(&gear, &own);
                let mut dropped = 0;
                for (slot, name) in preset.kit(index) {
                    let held = kit[usize::from(slot)];
                    let Some(item) = dictionary.item_named(name) else {
                        assert_eq!(held, None, "{name}");
                        missing.push(name);
                        continue;
                    };
                    let count = match (item.is_stack, POCKET_SLOTS.contains(&slot)) {
                        (true, true) => {
                            let mut alone = [None; KIT_SLOTS];
                            alone[usize::from(slot)] =
                                Some((item.dict_id, preset.pocket_units_of(item.dict_id)));
                            let Some((_, units)) = layout.kept(&alone)[usize::from(slot)] else {
                                assert_eq!(held, None, "{name}: a pocket the gear does not open");
                                dropped += 1;
                                continue;
                            };
                            units
                        }
                        (true, false) => {
                            rounds(name, starter_slot_bags(dictionary, preset, index, slot))
                        }
                        (false, _) => 1,
                    };
                    let held = held
                        .unwrap_or_else(|| panic!("preset {number} kit {index}: {name} not worn"));
                    let owned = account.item(held.item_id).unwrap();
                    assert_eq!((owned.dict_id, held.count), (item.dict_id, count), "{name}");
                    if !item.is_stack {
                        // One item of a kind: every kit and preset wears
                        // the same one.
                        let of_kind = account
                            .inventory
                            .iter()
                            .filter(|o| o.dict_id == item.dict_id);
                        assert_eq!(of_kind.count(), 1, "{name}");
                        assert_eq!(owned.condition_or_stack, FULL_CONDITION, "{name}");
                        assert_eq!(owned.modifications, own_modifications(name), "{name}");
                    }
                }
                // Nothing else.
                let worn = kit.iter().flatten().count();
                let named = preset
                    .kit(index)
                    .filter(|(_, name)| dictionary.item_named(name).is_some())
                    .count();
                assert_eq!(worn, named - dropped, "preset {number} kit {index}");
            }
        }
        // The test data predates the knit cap and the Novice's backpack
        // (preset 1), the Keeper of the Sun and the Saiga-12 and its buckshot
        // (preset 2).
        missing.sort_unstable();
        missing.dedup();
        assert_eq!(
            missing,
            [
                "ammo_12mm_buck2",
                "edge_legs_5_special",
                "edge_torso_5_special",
                "novice_026_helmet",
                "saiga_12",
                "stk2_novice_026_back"
            ]
        );
        // Preset 1 plays: one weapon, in weapon slot 1; weapon slot 2
        // empty; three pockets.
        let mut levels = Vec::new();
        for (index, kit) in account.kits.iter().enumerate() {
            let worn: Vec<u8> = (0..KIT_SLOTS as u8)
                .filter(|&slot| kit.slots[usize::from(slot)].is_some())
                .collect();
            // Without the Novice's backpack (missing here) two pockets of the
            // three: the server keeps a kit to the gear it wears.
            assert_eq!(worn, [1, 2, 4, 5, 6, 7, 8, 13, 14], "kit {index}");
            assert_eq!(
                super::super::rules::check_play(&account, kit.id, 0),
                Ok(()),
                "kit {index}"
            );
            let weight = account.kit_weight(kit);
            assert!(weight <= 30.0, "kit {index}: {weight}");
            assert!(account.doubled_pockets(kit).is_empty());
            levels.push(super::super::matchmaking::kit_level(&account, kit));
        }
        assert_eq!(levels, [7, 5, 4]);
        assert_eq!(account.active_kit, account.kits[0].id);

        // The two modifications of every Keeper piece: the game's own rows,
        // armour +8 and weight -15 %, the strongest of their kinds.
        let table = Modifications::shared();
        let armour = table.modification(1004).unwrap();
        assert_eq!(armour.name, "st_modify_armor");
        assert_eq!(armour.lobby_info, [("armor".to_owned(), 0.08)]);
        let weight = table.modification(1064).unwrap();
        assert_eq!(weight.name, "st_modify_weight");
        assert_eq!(weight.lobby_info, [("weight".to_owned(), -0.15)]);
    }
}
