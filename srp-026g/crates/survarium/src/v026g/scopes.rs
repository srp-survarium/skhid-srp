//! Weapon scopes a player buys once, owns and puts on any number of
//! weapons (`docs/notes/weapon_scopes.md`, `docs/notes/attachments.md`).
//!
//! **A scope is a module of the weapon** ([`super::weapon_modules`]), as a
//! muzzle device and a grip are. Which weapon takes which scope, and how it
//! stands on it, is the modules table's (`gameplay/weapon_modules.srpm`):
//! there is no pair kept anywhere else, and every fit question is asked of
//! [`Modules`]. A scope is view-only (no shooting, dispersion or hit code
//! reads it), so nothing here changes what a match computes.
//!
//! **How the client shows a scope.** An item's upgrade record
//! (`{u32 id, u32 dict_id, u64, u8 slot}`) makes the client build the item
//! from a modified config: for each record it applies the
//! `parameters.modification` of the item whose dict id is in the record
//! (0x2F15A2). A scope's record holds the scope item's **own** dict id, in
//! the weapon kit's slot [`SCOPE_SLOT`], and the scope item's modification
//! adds `data/srp_scope` = that dict id to the weapon's config. The client
//! (patch 161) takes it as the weapon's scope item (zoom, the two models)
//! and asks the modules table where the optic stands and where the eye is.
//! One owned scope item goes on any number of weapons, each its own record.
//! Without the record the weapon is built from its plain config: a weapon's
//! own default optic (`addons.rifle_scope_dict_id`: 0.26g's Leupold 69,
//! PSO-1 304, 1P69 362) comes back.
//!
//! **What is sold**: a scope item some gun takes, as any item of the
//! modules ([`Modules::is_module`], `super::prices`). Game data without the
//! table has no such gun: no scope is sold and none goes on, whatever the
//! server's code.
//!
//! **Before the table** (releases up to revision 68) a record held a
//! *variant*, a category-22 entry made for one weapon. Those dict ids are
//! retired; an account's load rewrites its records to the scope's own
//! (`session`'s `items_rules::scope_records_to_own_ids`), and [`released`]
//! reads the old pairs for the check that none was lost.

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};
use super::weapon_modules::{Kind, Modules, TABLE_PATH};

pub mod released;

/// The category of scopes (`categories_dict`): the three of 0.26g, the
/// shop's scope items and the retired variants. It has no profile slot.
pub const SCOPE_CATEGORY: u8 = 22;
/// The weapons' categories: rifles (8), sniper rifles (13), shotguns (14),
/// submachine guns (15), machine guns (16) and pistols (17).
pub const WEAPON_CATEGORIES: [u8; 6] = [8, 13, 14, 15, 16, 17];
/// The upgrade slot of a scope on a weapon: the armour camouflage's slot 3
/// ([`super::modifications::CAMO_SLOT`]), free on a weapon, whose camouflage
/// goes into 2 ([`super::modifications::WEAPON_CAMO_SLOT`]). The client sends
/// the slot of its window's drop (`relocate_upgrade_item`, 0x29A8B0) and
/// checks none.
pub const SCOPE_SLOT: u8 = 3;

/// Every weapon of `dictionary` with each scope `modules` fits to it, as
/// `(weapon, scope)` by weapon and scope: the weapons are the dictionary's
/// (a gun of the table that is no item is nobody's), the scopes the
/// table's.
pub fn pairs(dictionary: &Dictionary, modules: &Modules) -> Vec<(u16, u16)> {
    let mut pairs: Vec<(u16, u16)> = dictionary
        .items
        .iter()
        .filter(|item| WEAPON_CATEGORIES.contains(&item.category))
        .flat_map(|weapon| {
            modules
                .fits(weapon.dict_id)
                .into_iter()
                .filter(|&(_, kind)| kind == Kind::Scope)
                .map(move |(scope, _)| (weapon.dict_id, scope))
        })
        .collect();
    pairs.sort_unstable();
    pairs
}

/// The start's check of `data`: its modules table parses, and what it fits
/// as a scope is a scope item ([`check_pairs`]).
pub fn check(data: &GameData, dictionary: &Dictionary) -> Result<(), GameDataError> {
    let modules = Modules::of(data).map_err(|why| {
        GameDataError(format!("{}: {TABLE_PATH} does not parse: {why:?}", data.origin))
    })?;
    check_pairs(dictionary, &modules).map_err(|why| GameDataError(format!("{}: {why}", data.origin)))
}

/// Every scope `modules` fits to a weapon of `dictionary` is a scope item
/// (category [`SCOPE_CATEGORY`]), so that what goes into a weapon's
/// [`SCOPE_SLOT`] is always one. A table that names anything else would let
/// that item be put on as a scope: srp refuses to start on it.
pub fn check_pairs(dictionary: &Dictionary, modules: &Modules) -> Result<(), String> {
    for (weapon, scope) in pairs(dictionary, modules) {
        let category = dictionary.item(scope).map(|item| item.category);
        if category != Some(SCOPE_CATEGORY) {
            return Err(format!(
                "{TABLE_PATH} fits {scope} to weapon {weapon} as a scope, and {scope} is {}",
                match category {
                    Some(category) => format!("an item of category {category}"),
                    None => "no item of the dictionary".to_owned(),
                }
            ));
        }
    }
    Ok(())
}

/// The start's log line: how many pairs the players' game data has.
pub fn log_shared() {
    let pairs = pairs(Dictionary::shared(), Modules::shared());
    let mut weapons: Vec<u16> = pairs.iter().map(|&(weapon, _)| weapon).collect();
    weapons.dedup();
    let mut scopes: Vec<u16> = pairs.iter().map(|&(_, scope)| scope).collect();
    scopes.sort_unstable();
    scopes.dedup();
    log::info!(
        "weapon scopes: {} pair(s) of {} scope item(s) on {} weapon(s){}",
        pairs.len(),
        scopes.len(),
        weapons.len(),
        if pairs.is_empty() {
            " (the modules table fits no scope: none is sold or put on)"
        } else {
            ""
        }
    );
}

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;

#[cfg(test)]
mod tests;
