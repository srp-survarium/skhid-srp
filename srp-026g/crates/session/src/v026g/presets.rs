//! Kit presets (`docs/notes/presets.md`, the owner's of 2026-10-23): five
//! sets of an account's three kits, one of them active, chosen in the
//! Character panel's dropdown, and offered when the kits' gear does not
//! suit a room.
//!
//! **The active preset is the kits.** It is kept nowhere else: whatever
//! changes a kit (a move, the ammo window, a purchase onto a slot, a sale,
//! a craft, a match's ammo, a repair at load) changes it, with nothing to
//! save and no path that could miss it. The other four are snapshots, kept
//! with the account ([`Account::presets`]; the active one's place there is
//! always empty).
//!
//! Choosing another preset ([`choose`]) keeps the kits as the snapshot of
//! the one left and puts the chosen one on through the moves' own rules
//! ([`rules::fill_kit`]): it never buys, never makes an item or money, and
//! never takes what the account does not own; an item gone since leaves its
//! slot empty. A preset names owned items as a kit does; an item may be in
//! several kits and several presets.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::loadouts::POCKET_SLOTS;
use survarium::v026g::lobby::requests::{INVENTORY_SLOT, Relocation};
use survarium::v026g::lobby::rooms::PRESET_COUNT;

use super::account::{Account, KIT_SLOTS, KITS};
use super::matchmaking;
use super::rules::{self, ActionError};

/// The presets an account has.
pub const PRESETS: usize = PRESET_COUNT as usize;
/// The preset a new account (and one of before presets) has active.
pub const FIRST_PRESET: u8 = 1;

/// What a slot of a preset held when it was left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresetSlot {
    pub item_id: u32,
    /// The item's dict id then: an item id never names another item, but a
    /// stack used up is found again by it.
    pub dict_id: u16,
    /// A stack's units the kit took; 1 for anything else.
    pub count: u32,
}

/// The three kits' slots, in the order of the account's kits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    pub kits: [[Option<PresetSlot>; KIT_SLOTS]; KITS],
}

impl Preset {
    /// How many slots it fills.
    pub fn slots(&self) -> usize {
        self.kits.iter().flatten().flatten().count()
    }
}

/// Why a preset request is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetError {
    /// Not 1 to [`PRESETS`].
    NoSuchPreset,
    /// The player is in a match or on its way to one: its kits stay as they
    /// are (the lobby's word).
    Busy,
    /// A kit rule refused (`rules::fill_kit`); nothing changed.
    Rule(ActionError),
}

impl std::fmt::Display for PresetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PresetError {}

fn index(number: u8) -> Result<usize, PresetError> {
    let index = usize::from(number)
        .checked_sub(1)
        .ok_or(PresetError::NoSuchPreset)?;
    (index < PRESETS)
        .then_some(index)
        .ok_or(PresetError::NoSuchPreset)
}

/// The three kits as they are now; none when they hold nothing.
pub fn snapshot(account: &Account) -> Option<Preset> {
    let mut preset = Preset {
        kits: [[None; KIT_SLOTS]; KITS],
    };
    for (kit, saved) in account.kits.iter().zip(preset.kits.iter_mut()) {
        for (held, slot) in kit.slots.iter().zip(saved.iter_mut()) {
            *slot = held.and_then(|held| {
                account.item(held.item_id).map(|item| PresetSlot {
                    item_id: held.item_id,
                    dict_id: item.dict_id,
                    count: held.count,
                })
            });
        }
    }
    (preset.slots() > 0).then_some(preset)
}

/// Preset `number` as it is now: the active one is the kits; none: empty.
pub fn preset(account: &Account, number: u8) -> Result<Option<Preset>, PresetError> {
    let index = index(number)?;
    Ok(if number == account.active_preset {
        snapshot(account)
    } else {
        account.presets[index]
    })
}

/// Make the account's presets whole: an active preset of 1 to [`PRESETS`]
/// (the first otherwise), and nothing kept in its place (the kits are it).
/// Whether anything changed.
pub fn settle(account: &mut Account) -> bool {
    let mut changed = false;
    if index(account.active_preset).is_err() {
        account.active_preset = FIRST_PRESET;
        changed = true;
    }
    let active = usize::from(account.active_preset - 1);
    if account.presets[active].take().is_some() {
        changed = true;
    }
    changed
}

/// What choosing a preset did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Applied {
    /// The preset's slots left empty: their items gone (sold, crafted away,
    /// used up), or a rule of the kit took them off.
    pub empty: u32,
}

/// The owned item a saved slot names now: the same item; for a stack used
/// up and bought again (a new item), the stack of its type owned now. None:
/// gone.
fn owned(account: &Account, saved: &PresetSlot) -> Option<u32> {
    let dictionary = Dictionary::shared();
    let stack = dictionary.is_stack(saved.dict_id);
    let left = |units: u32| !stack || units > 0;
    if let Some(item) = account.item(saved.item_id)
        && item.dict_id == saved.dict_id
        && left(item.condition_or_stack)
    {
        return Some(item.item_id);
    }
    if !stack {
        return None;
    }
    account
        .inventory
        .iter()
        .find(|item| item.dict_id == saved.dict_id && item.condition_or_stack > 0)
        .map(|item| item.item_id)
}

/// `preset` onto the three kits, all or nothing: each kit emptied and
/// filled from it by the kit rules ([`rules::fill_kit`]). An item gone
/// leaves its slot empty; a stack gets the units saved, at most those owned;
/// a type a pocket of the kit already holds stays out of a second one; an
/// item that no longer fits its slot (the game's data changed since) stays
/// out, as the repair at load takes it off a kit. Nothing is bought, made
/// or taken: the inventory and the money stay as they are. The kit played
/// is then one that can play ([`play_a_playable_kit`]).
fn put_on(account: &mut Account, preset: &Preset, now: u64) -> Result<Applied, PresetError> {
    let dictionary = Dictionary::shared();
    let mut changed = account.clone();
    let mut empty = 0;
    for (kit_index, saved) in preset.kits.iter().enumerate().take(changed.kits.len()) {
        let mut moves = Vec::new();
        let mut pocket_types: Vec<u16> = Vec::new();
        for (slot, saved) in saved.iter().enumerate() {
            let Some(saved) = saved else {
                continue;
            };
            let Some(item_id) = owned(&changed, saved) else {
                continue;
            };
            if !dictionary.fits(saved.dict_id, slot as u8) {
                continue;
            }
            if POCKET_SLOTS.contains(&(slot as u8)) {
                if pocket_types.contains(&saved.dict_id) {
                    continue;
                }
                pocket_types.push(saved.dict_id);
            }
            moves.push(Relocation {
                item_id,
                dict_id: saved.dict_id,
                source_slot: INVENTORY_SLOT,
                target_slot: slot as u8,
                count: saved.count.max(1),
                count_in_inventory: saved.count.max(1),
            });
        }
        rules::fill_kit(&mut changed, kit_index, &moves).map_err(PresetError::Rule)?;
        let filled = changed.kits[kit_index].slots.iter().flatten().count();
        let wanted = saved.iter().flatten().count();
        empty += wanted.saturating_sub(filled) as u32;
    }
    changed.sync_kits();
    play_a_playable_kit(&mut changed, now);
    *account = changed;
    Ok(Applied { empty })
}

/// The kit played can not play (no weapon, too little ammo) and another
/// can: that one is played, the one whose level is the account's gear level
/// (the first of two alike), so the rooms' gear check and the kit a match
/// takes agree. Whether the kit played changed.
fn play_a_playable_kit(account: &mut Account, now: u64) -> bool {
    if rules::check_play(account, account.active_kit, now).is_ok() {
        return false;
    }
    let gear = matchmaking::gear_level(account, now);
    let best = account
        .kits
        .iter()
        .filter(|kit| rules::check_play(account, kit.id, now).is_ok())
        .find(|kit| matchmaking::kit_level(account, kit) == gear)
        .map(|kit| kit.id);
    match best {
        Some(kit_id) => {
            account.active_kit = kit_id;
            true
        }
        None => false,
    }
}

/// Make preset `number` the active one, all or nothing. The kits as they
/// are stay with the preset left (its snapshot); the chosen one is put on
/// them ([`put_on`]). An empty preset chosen starts as the kits worn: they
/// stay as they are [choice, `presets.md`]. The active one chosen again
/// changes nothing.
pub fn choose(account: &mut Account, number: u8, now: u64) -> Result<Applied, PresetError> {
    let index = index(number)?;
    settle(account);
    if number == account.active_preset {
        return Ok(Applied { empty: 0 });
    }
    let left = usize::from(account.active_preset - 1);
    let mut changed = account.clone();
    let applied = match account.presets[index] {
        Some(chosen) => put_on(&mut changed, &chosen, now)?,
        None => Applied { empty: 0 },
    };
    changed.presets[left] = snapshot(account);
    changed.presets[index] = None;
    changed.active_preset = number;
    *account = changed;
    Ok(applied)
}

/// Preset `number`'s gear level, the measure of a room's max level
/// ([`matchmaking::gear_level`]): that of the kits it gives (the active
/// one's, the kits now; another's, on a copy of the account with it put
/// on), the highest of those that can play. None: empty, or none of its
/// kits can play.
pub fn level(account: &Account, number: u8, now: u64) -> Option<u8> {
    let preset = preset(account, number).ok()??;
    let mut copy;
    let kits = if number == account.active_preset {
        account
    } else {
        copy = account.clone();
        put_on(&mut copy, &preset, now).ok()?;
        &copy
    };
    kits.kits
        .iter()
        .any(|kit| rules::check_play(kits, kit.id, now).is_ok())
        .then(|| matchmaking::gear_level(kits, now))
}

/// Every preset's [`level`], preset 1 first.
pub fn levels(account: &Account, now: u64) -> [Option<u8>; PRESETS] {
    std::array::from_fn(|index| level(account, index as u8 + 1, now))
}

/// What a join offers (`presets.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    /// No popup.
    None,
    /// The room allows more than the kits: `preset` is higher and fits
    /// ("Enter anyway" or "Switch and enter").
    Higher { preset: u8, level: u8 },
    /// The gear is over the room's max level; `preset` fits ("Exit" or
    /// "Switch and enter").
    Fits { preset: u8, level: u8 },
}

/// The popup of a join of a room of `max_level` (0: any) with the kits'
/// gear at `gear` and the presets' `levels` ([`levels`]): the preset
/// offered is the one with the highest level the room allows (at most its
/// max level), the lower number of two alike (the owner's of 2026-10-23).
/// A room of any max level offers nothing.
pub fn offer(gear: u8, max_level: u8, levels: &[Option<u8>; PRESETS]) -> Offer {
    if max_level == 0 {
        return Offer::None;
    }
    let mut best: Option<(u8, u8)> = None;
    for (index, level) in levels.iter().enumerate() {
        let Some(level) = level.filter(|&level| level <= max_level) else {
            continue;
        };
        if best.is_none_or(|(_, kept)| level > kept) {
            best = Some((index as u8 + 1, level));
        }
    }
    match best {
        Some((preset, level)) if gear > max_level => Offer::Fits { preset, level },
        Some((preset, level)) if level > gear => Offer::Higher { preset, level },
        _ => Offer::None,
    }
}

#[cfg(test)]
mod tests;
