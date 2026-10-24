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
//! and a starter kit worn in its first kit ([`STARTER_GEAR`],
//! [`STARTER_WEAPONS`], [`STARTER_MODIFICATIONS`]) (the owner's of
//! 2026-10-19 and 2026-10-24, `docs/notes/no-tutorial.md`). It is the
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

/// The armour a new account wears in its first kit, `(slot, item)`, each
/// the plain item of its English name: the Novice's knit cap (1821), the
/// Amber respirator (43), the Survivor's jacket (325, not the premium 345),
/// the Chemical protection pants (46), gloves (42) and boots (38).
pub const STARTER_GEAR: [(u8, &str); 6] = [
    (0, "novice_026_helmet"),
    (1, "scav_resp_1"),
    (2, "scav_torso_4"),
    (4, "scav_legs_2"),
    (5, "scav_gloves_2"),
    (6, "scav_boots_2"),
];

/// The weapons it wears, `(weapon slot, weapon, ammo slot, its ammo)`: the
/// SOK-94 (2215) with 7.62x39 (81) and the OTs-02 Kiparis (88, not the
/// premium 332) with 9x18 (70), each with its category's
/// `optimal_count` of rounds in whole clips ([`loaded_rounds`]).
pub const STARTER_WEAPONS: [(u8, &str, u8, &str); 2] = [
    (7, "soc_vepr", 8, "ammo_7.62x39"),
    (10, "kiparis", 11, "ammo_9x18_makarov"),
];

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
    STARTER_GEAR.iter().map(|&(_, name)| name).chain(
        STARTER_WEAPONS
            .iter()
            .flat_map(|&(_, weapon, _, ammo)| [weapon, ammo]),
    )
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
/// and the starter kit worn in its first kit ([`wear_starter_kit`]).
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

/// The starter kit bought new and worn in the first kit by the lobby's own
/// rule of a kit put on (`rules::fill_kit`): what the kit wore before (the
/// TT-33 and its clip) goes back to the inventory. An item the game data
/// lacks is left out: srp starts only on data with all of them
/// (`known::check_code`); the test data predates the SOK-94 and the knit
/// cap.
pub fn wear_starter_kit(account: &mut Account, dictionary: &Dictionary) {
    // A move of the item `name` (one bought new, or the stack owned) from
    // the inventory into `slot`.
    let wear = |account: &mut Account, slot: u8, name: &str| -> Option<Relocation> {
        let Some(item) = dictionary.item_named(name) else {
            log::warn!("starter kit: the game data has no {name}; left out");
            return None;
        };
        let item_id = if item.is_stack {
            account.add_to_stack(item.dict_id, 0)
        } else {
            account.add_item(item.dict_id, FULL_CONDITION)
        };
        Some(Relocation {
            item_id,
            dict_id: item.dict_id,
            source_slot: INVENTORY_SLOT,
            target_slot: slot,
            count: 1,
            count_in_inventory: 0,
        })
    };
    let mut moves = Vec::new();
    for (slot, name) in STARTER_GEAR {
        moves.extend(wear(account, slot, name));
    }
    for (slot, weapon, ammo_slot, ammo) in STARTER_WEAPONS {
        let Some(weapon) = wear(account, slot, weapon) else {
            continue;
        };
        moves.push(weapon);
        if let Some(mut ammo) = wear(account, ammo_slot, ammo) {
            ammo.count = loaded_rounds(dictionary, weapon.dict_id, ammo.dict_id);
            moves.push(ammo);
        }
    }
    for (name, modification) in STARTER_MODIFICATIONS {
        let Some(dict_id) = dictionary.item_named(name).map(|item| item.dict_id) else {
            continue;
        };
        let worn = moves
            .iter()
            .find(|relocation| relocation.dict_id == dict_id)
            .map(|relocation| relocation.item_id);
        if let Some(item) = worn.and_then(|item_id| account.item_mut(item_id)) {
            item.modifications.push((modification, false));
        }
    }
    if let Err(error) = super::rules::fill_kit(account, 0, &moves) {
        log::error!("starter kit: the first kit could not wear it ({error}); it keeps its own");
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

    /// A fresh account on the test data: everything, but the two items of
    /// patches the test data predates (the SOK-94 and the knit cap; srp
    /// refuses such data, `known::check_code`). The whole kit by its dict
    /// ids on released data: `tests/starter_kit.rs`.
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

        // The first kit wears the kit, the rest of its slots empty.
        let kiparis = dictionary.item_named("kiparis").unwrap().dict_id;
        let nine = dictionary.item_named("ammo_9x18_makarov").unwrap().dict_id;
        assert_eq!(loaded_rounds(dictionary, kiparis, nine), 100);
        let mut wanted: Vec<(u8, &str, u32)> = STARTER_GEAR
            .iter()
            .map(|&(slot, name)| (slot, name, 1))
            .collect();
        for (slot, weapon, ammo_slot, ammo) in STARTER_WEAPONS {
            wanted.push((slot, weapon, 1));
            if let (Some(weapon), Some(ammo)) =
                (dictionary.item_named(weapon), dictionary.item_named(ammo))
            {
                let rounds = loaded_rounds(dictionary, weapon.dict_id, ammo.dict_id);
                wanted.push((ammo_slot, ammo.name.as_str(), rounds));
            }
        }
        let kit = &account.kits[0];
        let mut missing = Vec::new();
        for &(slot, name, count) in &wanted {
            let held = kit.slots[usize::from(slot)];
            let Some(item) = dictionary.item_named(name) else {
                assert_eq!(held, None, "{name}");
                missing.push(name);
                continue;
            };
            let held = held.unwrap_or_else(|| panic!("{name} not worn"));
            let owned = account.item(held.item_id).unwrap();
            assert_eq!((owned.dict_id, held.count), (item.dict_id, count), "{name}");
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
        assert_eq!(missing, ["novice_026_helmet", "soc_vepr"]);
        let worn: Vec<u8> = (0..KIT_SLOTS as u8)
            .filter(|&slot| kit.slots[usize::from(slot)].is_some())
            .collect();
        assert_eq!(worn, [1, 2, 4, 5, 6, 10, 11]);
        assert_eq!(account.active_kit, kit.id);
        assert!(
            account.kits[1..]
                .iter()
                .all(|kit| kit.slots.iter().all(Option::is_none))
        );
        assert_eq!(super::super::rules::check_play(&account, kit.id, 0), Ok(()));
        assert!(account.kit_weight(kit) <= 30.0);
        // The starting weapons stay owned, in the inventory.
        for name in ["tt_33", "mosin_nagant", "ppsh"] {
            let dict_id = dictionary.item_named(name).unwrap().dict_id;
            let owned = account
                .inventory
                .iter()
                .find(|item| item.dict_id == dict_id)
                .unwrap();
            assert_eq!(kit.slot_of(owned.item_id), None, "{name}");
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
