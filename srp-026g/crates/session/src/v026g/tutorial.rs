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
//! level and skills, money, the rounds of every ammo
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
/// it owns and what its three kits wear ([`wear_starter_kit`]).
pub fn signup_grant(account: &mut Account) {
    signup_grant_with(account, true);
}

/// [`signup_grant`], its level and skills, what it owns and the kits
/// given only with `starter_kit` (off only in the lobby's tests,
/// `GameAccounts::set_starter_kit`).
pub fn signup_grant_with(account: &mut Account, starter_kit: bool) {
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
}
