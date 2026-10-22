//! Kit presets (`docs/notes/presets.md`, the owner's of 2026-10-23): five
//! snapshots of an account's three kits, saved and put on from the
//! Character panel, and offered when the kits' gear does not suit a room.
//!
//! A preset names owned items as a kit does (by item id; for a stack, its
//! units). Kits share one inventory, so an item may be in several kits and
//! in several presets; saving reserves, copies or moves nothing. Putting a
//! preset on fills each kit through the moves' own rules
//! ([`rules::fill_kit`]): it never buys, never makes an item or money, and
//! never takes what the account does not own; an item gone since leaves its
//! slot empty.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::loadouts::POCKET_SLOTS;
use survarium::v026g::lobby::requests::{INVENTORY_SLOT, Relocation};
use survarium::v026g::lobby::rooms::PRESET_COUNT;

use super::account::{Account, KIT_SLOTS, KITS};
use super::matchmaking;
use super::rules::{self, ActionError};

/// The presets an account has.
pub const PRESETS: usize = PRESET_COUNT as usize;

/// What a slot of a preset held when it was saved.
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
    /// Nothing saved in it.
    Empty,
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

/// The three kits as they are now into preset `number` (1 to [`PRESETS`]),
/// replacing what it held. Kits with nothing in them leave it empty.
/// Whether it holds anything now.
pub fn save(account: &mut Account, number: u8) -> Result<bool, PresetError> {
    let index = index(number)?;
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
    let any = preset.slots() > 0;
    account.presets[index] = any.then_some(preset);
    Ok(any)
}

/// What putting a preset on did.
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

/// Preset `number` onto the three kits, all or nothing: each kit emptied
/// and filled from it by the kit rules ([`rules::fill_kit`]). An item gone
/// leaves its slot empty; a stack gets the units saved, at most those owned;
/// a type a pocket of the kit already holds stays out of a second one.
/// Nothing is bought, made or taken: the inventory and the money stay as
/// they are.
pub fn apply(account: &mut Account, number: u8) -> Result<Applied, PresetError> {
    let index = index(number)?;
    let preset = account.presets[index].ok_or(PresetError::Empty)?;
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
    *account = changed;
    Ok(Applied { empty })
}

/// Preset `number`'s gear level, the measure of a room's max level
/// ([`matchmaking::gear_level`]): that of the kits it would give now (on a
/// copy of the account), the highest of those that can play. None: empty,
/// or none of its kits can play.
pub fn level(account: &Account, number: u8, now: u64) -> Option<u8> {
    let mut copy = account.clone();
    apply(&mut copy, number).ok()?;
    copy.kits
        .iter()
        .any(|kit| rules::check_play(&copy, kit.id, now).is_ok())
        .then(|| matchmaking::gear_level(&copy, now))
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
