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
//! `a >> 1 & 1` opens `state_choose_faction`, 0x28E9AD). Bit 0 is a
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
//! with 60 000 silver and 500 gold ([`SIGNUP_SILVER`], [`SIGNUP_GOLD`]),
//! the steps' experience, 1000 rounds of every ammo
//! ([`starter_ammo`]), the Scavengers at their highest reputation level
//! and a starter kit worn in each of its three kits ([`STARTER_GEAR`],
//! [`STARTER_WEAPONS`], [`STARTER_POCKETS`],
//! [`STARTER_MODIFICATIONS`]) (the owner's of 2026-10-19 and 2026-10-24,
//! `docs/notes/no-tutorial.md`). It is the
//! account's state when the client first hears of it: no window, no
//! notice.

use survarium::v026g::awards::{Awards, kind, name};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::item_params::shop_unit;
use survarium::v026g::lobby::requests::{INVENTORY_SLOT, Relocation};
use survarium::v026g::lobby::{self, FULL_CONDITION, ServerMessage};

use super::account::Account;

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

/// The awards a new account is given, the tutorial's steps'.
pub const AWARDS: [&str; 4] = [
    name::TUTORIAL_COMPLETED,
    name::ITEM_NOTES,
    name::BATTLE_NOTES,
    name::GAME_WORLD_TUTORIAL,
];

/// The faction a new account takes quests from: the Scavengers.
pub const SIGNUP_FACTION: u8 = survarium::prices::faction::SCAVENGERS;

/// What a new account's balance is: silver (10 000, and 50 000 more since
/// 2026-10-24) and gold.
pub const SIGNUP_SILVER: u32 = 60_000;
pub const SIGNUP_GOLD: u32 = 500;

/// The rounds a new account owns of every ammo ([`starter_ammo`]): the
/// client's own most of a free stack (`rules::MAX_FREE_UNITS`).
pub const SIGNUP_ROUNDS: u32 = 1000;

/// A new account's reputation level with [`SIGNUP_FACTION`]: the
/// Scavengers' highest (`factions_dict`: 3000 points), everything they sell
/// unlocked.
pub const SIGNUP_REPUTATION_LEVEL: u8 = 5;

/// Where the ammo lives: every item of the dictionary under it is ammo.
pub const AMMO_CONFIGS: &str = "gameplay/items/weapons/ammo/";

/// The gear each of a new account's three kits wears, `(slot, item)`, each
/// the plain item of its English name: the Novice's knit cap (1821), the
/// Amber respirator (43), the Survivor's jacket (325, not the premium 345),
/// the Oxygen tank (9, the back slot), the Chemical protection pants (46),
/// gloves (42) and boots (38).
pub const STARTER_GEAR: [(u8, &str); 7] = [
    (0, "novice_026_helmet"),
    (1, "scav_resp_1"),
    (2, "scav_torso_4"),
    (3, "scav_oxygen_1"),
    (4, "scav_legs_2"),
    (5, "scav_gloves_2"),
    (6, "scav_boots_2"),
];

/// The one weapon of kits 1, 2 and 3, `(weapon, its ammo)`, in weapon slot
/// 1 ([`WEAPON_SLOT`]; weapon slot 2 empty): the OTs-02 Kiparis (88, not
/// the premium 332) with 9x18 (70); the TOZ-122 (12) with 7.62x51 (51),
/// the game has no TOZ-11; the MP-153 Baikal (110) with 12x70 smallshot
/// (22). Each weapon's ammo is its plain round (not the PBM, M59 or slug),
/// loaded with its category's `optimal_count` of rounds in whole clips
/// ([`loaded_rounds`]).
pub const STARTER_WEAPONS: [(&str, &str); 3] = [
    ("kiparis", "ammo_9x18_makarov"),
    ("toz_122", "ammo_7.62x51"),
    ("mp_153", "ammo_12mm_buck"),
];

/// Weapon slot 1 and its first ammo slot.
pub const WEAPON_SLOT: (u8, u8) = (7, 8);

/// The pockets of every kit, `(slot, item, units)`: 2 medkits (67) and 2
/// antitoxins (82). A player has one pocket of one unit; the jacket's
/// default 1030 adds a pocket and the pants' 1170 a unit to each.
pub const STARTER_POCKETS: [(u8, &str, u32); 2] = [(13, "medkit", 2), (14, "antitoxin", 2)];

/// What a new account owns of the pockets' items, `(item, units)`: the kits
/// take theirs from these stacks.
pub const STARTER_SUPPLIES: [(&str, u32); 2] = [("medkit", 50), ("antitoxin", 50)];

/// The modifications of their own two pieces have, `(item, modification)`
/// (`modifications.options`, the strongest but one of their type):
/// - the respirator 1183, `st_modify_anom_filters`: +0.2 face armour
///   against toxins, radiation, fire and shock; the lobby shows
///   "Isolation: +20" (`props_list` 8, "20", added);
/// - the gloves 1133, `st_modify_artefact_search`: -0.2 artefact search
///   time; the lobby shows "Collection time: -20%" (`props_list` 108,
///   "-0.20", a percentage). Beside the gloves' own default 1130 (-5%).
pub const STARTER_MODIFICATIONS: [(&str, u32); 2] =
    [("scav_resp_1", 1183), ("scav_gloves_2", 1133)];

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

/// The rounds of `ammo` a kit loads for `weapon`: its category's
/// `optimal_count`, up to a whole clip (the ammo window's step).
pub fn loaded_rounds(dictionary: &Dictionary, weapon: u16, ammo: u16) -> u32 {
    let optimal = dictionary
        .item(weapon)
        .and_then(|item| dictionary.categories.iter().find(|c| c.id == item.category))
        .map_or(0, |category| category.optimal_count);
    let clip = shop_unit(ammo);
    optimal.div_ceil(clip) * clip
}

/// Every item name of the starter kit, for the start's check
/// (`known::named_items`).
pub fn starter_names() -> impl Iterator<Item = &'static str> {
    STARTER_GEAR
        .iter()
        .map(|&(_, name)| name)
        .chain(
            STARTER_WEAPONS
                .iter()
                .flat_map(|&(weapon, ammo)| [weapon, ammo]),
        )
        .chain(STARTER_POCKETS.iter().map(|&(_, name, _)| name))
}

/// The experience the tutorial's steps paid, by the game's file.
pub fn experience() -> u32 {
    AWARDS
        .iter()
        .flat_map(|award| Awards::shared().get(award))
        .filter(|row| row.kind == kind::EXPERIENCE)
        .map(|row| row.amount)
        .sum()
}

/// A new account's start, before its client first hears of it: its
/// balance ([`SIGNUP_SILVER`], [`SIGNUP_GOLD`]), the steps' experience,
/// [`SIGNUP_ROUNDS`] of every ammo, the Scavengers at
/// [`SIGNUP_REPUTATION_LEVEL`] and their quests (the tutorial ended in the
/// faction's choice; the owner's of 2026-10-19: Scavengers, never neutral),
/// and the starter kit worn in its three kits ([`wear_starter_kit`]).
pub fn signup_grant(account: &mut Account) {
    signup_grant_with(account, true);
}

/// [`signup_grant`], the kit worn only with `wear_kit` (off only in the
/// lobby's tests, `GameAccounts::set_starter_kit`).
pub fn signup_grant_with(account: &mut Account, wear_kit: bool) {
    let dictionary = Dictionary::shared();
    // The reputation first: the quests are given by its level.
    let points = dictionary
        .factions
        .iter()
        .find(|faction| faction.id == SIGNUP_FACTION)
        .and_then(|faction| faction.levels.get(usize::from(SIGNUP_REPUTATION_LEVEL)))
        .expect("the Scavengers have their levels")
        .value;
    account.add_reputation(
        SIGNUP_FACTION,
        points.saturating_sub(account.reputation(SIGNUP_FACTION)),
    );
    super::quests::choose_faction(account, SIGNUP_FACTION)
        .expect("every faction is open to a new account");
    account.generic_money = account.generic_money.max(SIGNUP_SILVER);
    account.premium_money = account.premium_money.max(SIGNUP_GOLD);
    // Short of a level: nothing more to pay.
    account.add_experience(experience());
    for dict_id in starter_ammo(dictionary) {
        let owned = account
            .inventory
            .iter()
            .find(|item| item.dict_id == dict_id)
            .map_or(0, |item| item.condition_or_stack);
        if owned < SIGNUP_ROUNDS {
            account.add_to_stack(dict_id, SIGNUP_ROUNDS - owned);
        }
    }
    if wear_kit {
        wear_starter_kit(account, dictionary);
    }
    account.extras.tutorial_flags = ALL_DONE;
    account.tutorial_done = true;
}

/// The starter kit worn in each of the three kits by the lobby's own rule
/// of a kit put on (`rules::fill_kit`): what the first kit wore before (the
/// TT-33 and its clip) goes back to the inventory. Each item is bought once
/// and the kits that wear it name that one, as a player's own kits share
/// the inventory; the pockets' items come from stacks of
/// [`STARTER_SUPPLIES`]. An item the game data lacks is left out: srp starts
/// only on data with all of them (`known::check_code`); the test data
/// predates the knit cap.
pub fn wear_starter_kit(account: &mut Account, dictionary: &Dictionary) {
    for (name, units) in STARTER_SUPPLIES {
        let Some(item) = dictionary.item_named(name) else {
            continue;
        };
        let owned = account
            .inventory
            .iter()
            .find(|owned| owned.dict_id == item.dict_id)
            .map_or(0, |owned| owned.condition_or_stack);
        account.add_to_stack(item.dict_id, units.saturating_sub(owned));
    }
    // The items bought for the kits, `(dict id, item id)`.
    let mut bought: Vec<(u16, u32)> = Vec::new();
    // A move of the item `name` (bought new the first time, the stack
    // owned for a stack) from the inventory into `slot`.
    let mut wear = |account: &mut Account, slot: u8, name: &str, count: u32| {
        let Some(item) = dictionary.item_named(name) else {
            log::warn!("starter kit: the game data has no {name}; left out");
            return None;
        };
        let item_id = if item.is_stack {
            account.add_to_stack(item.dict_id, 0)
        } else if let Some(&(_, item_id)) = bought.iter().find(|(id, _)| *id == item.dict_id) {
            item_id
        } else {
            let item_id = account.add_item(item.dict_id, FULL_CONDITION);
            bought.push((item.dict_id, item_id));
            item_id
        };
        Some(Relocation {
            item_id,
            dict_id: item.dict_id,
            source_slot: INVENTORY_SLOT,
            target_slot: slot,
            count,
            count_in_inventory: 0,
        })
    };
    for (kit, (weapon, ammo)) in STARTER_WEAPONS.into_iter().enumerate() {
        let mut moves: Vec<Relocation> = Vec::new();
        for (slot, name) in STARTER_GEAR {
            moves.extend(wear(account, slot, name, 1));
        }
        let (slot, ammo_slot) = WEAPON_SLOT;
        if let Some(weapon) = wear(account, slot, weapon, 1) {
            let rounds = dictionary.item_named(ammo).map_or(0, |ammo| {
                loaded_rounds(dictionary, weapon.dict_id, ammo.dict_id)
            });
            moves.push(weapon);
            moves.extend(wear(account, ammo_slot, ammo, rounds));
        }
        for (slot, name, units) in STARTER_POCKETS {
            moves.extend(wear(account, slot, name, units));
        }
        if let Err(error) = super::rules::fill_kit(account, kit, &moves) {
            log::error!("starter kit: kit {} could not wear it ({error})", kit + 1);
        }
    }
    for (name, modification) in STARTER_MODIFICATIONS {
        let Some(dict_id) = dictionary.item_named(name).map(|item| item.dict_id) else {
            continue;
        };
        let worn = bought.iter().find(|(id, _)| *id == dict_id);
        if let Some(item) = worn.and_then(|&(_, item_id)| account.item_mut(item_id)) {
            item.modifications.push((modification, false));
        }
    }
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

    /// A fresh account on the test data: everything, but the knit cap of a
    /// patch the test data predates (srp refuses such data,
    /// `known::check_code`). The whole kit by its dict ids on released
    /// data: `tests/starter_kit.rs`.
    #[test]
    fn a_new_account_starts_with_the_starter_kit() {
        use super::super::account::KIT_SLOTS;
        use survarium::v026g::item_params::item_params;
        use survarium::v026g::modifications::Modifications;

        assert_eq!(experience(), 250);
        let mut account = Account::new("Alice");
        signup_grant(&mut account);
        assert_eq!(
            (account.generic_money, account.premium_money),
            (60_000, 500)
        );
        assert_eq!(account.exp_total, 250);
        assert_eq!(account.level(), 0);
        assert_eq!(account.quest_faction, SIGNUP_FACTION);
        assert_eq!(account.extras.tutorial_flags, ALL_DONE);

        // The Scavengers at 3000 points: level 5, their highest.
        let dictionary = Dictionary::shared();
        assert_eq!(account.reputation(SIGNUP_FACTION), 3000);
        assert_eq!(account.reputation_level(SIGNUP_FACTION), 5);
        let scavengers = dictionary.factions.iter().find(|f| f.id == 1).unwrap();
        assert_eq!(scavengers.levels.len(), 6);
        for faction in [2, 3, 4] {
            assert_eq!(account.reputation(faction), 0);
        }

        // Exactly 1000 rounds of every ammo, one stack each.
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
        for &dict_id in &ammo {
            assert_eq!(item_params(dict_id).unwrap().kind, "ammo");
            let stacks: Vec<u32> = account
                .inventory
                .iter()
                .filter(|item| item.dict_id == dict_id)
                .map(|item| item.condition_or_stack)
                .collect();
            assert_eq!(stacks, [SIGNUP_ROUNDS], "{dict_id}");
        }

        // Each of the three kits wears the kit, the rest of its slots empty.
        let rounds = |weapon: &str, ammo: &str| {
            let id = |name: &str| dictionary.item_named(name).unwrap().dict_id;
            loaded_rounds(dictionary, id(weapon), id(ammo))
        };
        assert_eq!(rounds("kiparis", "ammo_9x18_makarov"), 100);
        assert_eq!(rounds("toz_122", "ammo_7.62x51"), 20);
        assert_eq!(rounds("mp_153", "ammo_12mm_buck"), 30);
        let mut missing = Vec::new();
        let mut levels = Vec::new();
        for (index, (weapon, ammo)) in STARTER_WEAPONS.into_iter().enumerate() {
            let mut wanted: Vec<(u8, &str, u32)> = STARTER_GEAR
                .iter()
                .map(|&(slot, name)| (slot, name, 1))
                .collect();
            let (slot, ammo_slot) = WEAPON_SLOT;
            wanted.push((slot, weapon, 1));
            wanted.push((ammo_slot, ammo, rounds(weapon, ammo)));
            wanted.extend(STARTER_POCKETS);
            let kit = &account.kits[index];
            for &(slot, name, count) in &wanted {
                let held = kit.slots[usize::from(slot)];
                let Some(item) = dictionary.item_named(name) else {
                    assert_eq!(held, None, "{name}");
                    missing.push(name);
                    continue;
                };
                let held = held.unwrap_or_else(|| panic!("kit {index}: {name} not worn"));
                let owned = account.item(held.item_id).unwrap();
                assert_eq!((owned.dict_id, held.count), (item.dict_id, count), "{name}");
                // One item of a kind: every kit wears the same one.
                let of_kind = account
                    .inventory
                    .iter()
                    .filter(|owned| owned.dict_id == item.dict_id);
                assert_eq!(of_kind.count(), 1, "{name}");
                if !item.is_stack {
                    assert_eq!(owned.condition_or_stack, FULL_CONDITION, "{name}");
                    let own: Vec<(u32, bool)> = STARTER_MODIFICATIONS
                        .iter()
                        .filter(|(piece, _)| *piece == name)
                        .map(|&(_, modification)| (modification, false))
                        .collect();
                    assert_eq!(owned.modifications, own, "{name}");
                }
            }
            let worn: Vec<u8> = (0..KIT_SLOTS as u8)
                .filter(|&slot| kit.slots[usize::from(slot)].is_some())
                .collect();
            // One weapon, in weapon slot 1; weapon slot 2 empty.
            assert_eq!(worn, [1, 2, 3, 4, 5, 6, 7, 8, 13, 14], "kit {index}");
            assert_eq!(super::super::rules::check_play(&account, kit.id, 0), Ok(()));
            let weight = account.kit_weight(kit);
            assert!(weight <= 30.0, "kit {index}: {weight}");
            assert!(account.doubled_pockets(kit).is_empty());
            levels.push(super::super::matchmaking::kit_level(&account, kit));
        }
        assert_eq!(missing, ["novice_026_helmet"].repeat(3));
        // Within the bots' max gear 5: the armour, the oxygen tank and the
        // pockets' drugs are 3, the MP-153 4.
        assert_eq!(levels, [3, 3, 4]);
        assert_eq!(account.active_kit, account.kits[0].id);
        // 50 medkits and 50 antitoxins owned: the kits take theirs from them.
        for (name, units) in STARTER_SUPPLIES {
            let dict_id = dictionary.item_named(name).unwrap().dict_id;
            let stacks: Vec<u32> = account
                .inventory
                .iter()
                .filter(|item| item.dict_id == dict_id)
                .map(|item| item.condition_or_stack)
                .collect();
            assert_eq!(stacks, [units], "{name}");
        }
        // The starting weapons stay owned, in the inventory.
        for name in ["tt_33", "mosin_nagant", "ppsh"] {
            let dict_id = dictionary.item_named(name).unwrap().dict_id;
            let owned = account
                .inventory
                .iter()
                .find(|item| item.dict_id == dict_id)
                .unwrap();
            let worn = account
                .kits
                .iter()
                .any(|kit| kit.slot_of(owned.item_id).is_some());
            assert!(!worn, "{name}");
        }

        // The two modifications: the game's own rows.
        let table = Modifications::shared();
        let isolation = table.modification(1183).unwrap();
        assert_eq!(isolation.name, "st_modify_anom_filters");
        assert_eq!(isolation.lobby_info, [("mask_ano_armor".to_owned(), 0.2)]);
        assert!(
            isolation
                .modifiers
                .iter()
                .all(|m| m.value == 0.2 && m.kind == 1)
        );
        let respirator = dictionary.item_named("scav_resp_1").unwrap().dict_id;
        assert!(table.fits(respirator, &[], 1183));
        let collection = table.modification(1133).unwrap();
        assert_eq!(collection.name, "st_modify_artefact_search");
        assert_eq!(
            collection.lobby_info,
            [("artefact_search_time_modifier".to_owned(), -0.2)]
        );
        // The gloves' own default of that type: 1130, -5 %.
        let gloves = dictionary.item_named("scav_gloves_2").unwrap().dict_id;
        assert_eq!(item_params(gloves).unwrap().default_modifications, [1130]);
    }
}
