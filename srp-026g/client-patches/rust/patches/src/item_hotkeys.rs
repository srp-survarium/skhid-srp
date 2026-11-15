//! Patch 149: a key per item type uses it (the rules: docs/notes/item-hotkeys.md;
//! client_patch.md section 3.82).
//!
//! Fourteen actions of the moved action table (item_hotkeys_026g.asm), one per
//! item type, unbound until a player binds them in Options, Controls. A key of
//! them is a press of the quick-use key of the quick slot that holds its item:
//! at the input builder's end, before patch 78's chord reads the mask (the
//! chain's order 0), each of them the builder would find (the test it makes
//! for every bit: action_present, whatever the state) sets the quick-use bit
//! of the first quick slot whose item is of its type, in the game's order of
//! the quick slots (the hud's). Nothing of that type: nothing. Only the input a
//! key makes changes: a game without the patch plays the bits it relays.

#![forbid(unsafe_code)]

use artefact_carry::carry::QuickSlot;

use crate::vostok::inventory::{DictId, ItemClass, QuickItem};
use crate::vostok::{Action, ActionMask, Context, Flow, InputHandler, NoExit};

/// An item type a key uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKey {
    Onyx,
    Spring,
    Lifebone,
    Bandages,
    Medikit,
    MilitaryMedkit,
    BlackMarketMedkit,
    ScienceMedkit,
    Antitoxin,
    Painkiller,
    Rgd5,
    RdgM,
    Trap,
    OxygenTank,
}

/// What a quick slot must hold for a key: an item of the dictionary, or an
/// item of a class whatever its id (the oxygen packs: a kit has one back).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Item(DictId),
    Class(ItemClass),
}

impl Target {
    fn holds(self, item: QuickItem) -> bool {
        match self {
            Target::Item(id) => item.id == id,
            Target::Class(class) => item.class == class,
        }
    }
}

impl ItemKey {
    pub const ALL: [ItemKey; 14] = [
        ItemKey::Onyx,
        ItemKey::Spring,
        ItemKey::Lifebone,
        ItemKey::Bandages,
        ItemKey::Medikit,
        ItemKey::MilitaryMedkit,
        ItemKey::BlackMarketMedkit,
        ItemKey::ScienceMedkit,
        ItemKey::Antitoxin,
        ItemKey::Painkiller,
        ItemKey::Rgd5,
        ItemKey::RdgM,
        ItemKey::Trap,
        ItemKey::OxygenTank,
    ];

    /// Its action: its row of `actions` (item_hotkeys_026g.asm) and of the
    /// options' `controls` (hide_ui_026g.asm), 4Ah..57h: bindings the patch
    /// adds to the key binder's (60h, not the game's 48h).
    pub const fn action(self) -> Action {
        Action(match self {
            ItemKey::Onyx => 0x4A,
            ItemKey::Spring => 0x4B,
            ItemKey::Lifebone => 0x4C,
            ItemKey::Bandages => 0x4D,
            ItemKey::Medikit => 0x4E,
            ItemKey::MilitaryMedkit => 0x4F,
            ItemKey::BlackMarketMedkit => 0x50,
            ItemKey::ScienceMedkit => 0x51,
            ItemKey::Antitoxin => 0x52,
            ItemKey::Painkiller => 0x53,
            ItemKey::Rgd5 => 0x54,
            ItemKey::RdgM => 0x55,
            ItemKey::Trap => 0x56,
            ItemKey::OxygenTank => 0x57,
        })
    }

    /// Its item (scripts/content-edit/items.tsv; the medicines'
    /// scripts/medicines/medicines.tsv), or the oxygen packs' class.
    pub const fn target(self) -> Target {
        Target::Item(DictId(match self {
            ItemKey::Onyx => 76,
            ItemKey::Spring => 74,
            ItemKey::Lifebone => 54,
            ItemKey::Bandages => 66,
            ItemKey::Medikit => 67,
            ItemKey::MilitaryMedkit => 390,
            ItemKey::BlackMarketMedkit => 1196,
            ItemKey::ScienceMedkit => 391,
            ItemKey::Antitoxin => 82,
            ItemKey::Painkiller => 65,
            ItemKey::Rgd5 => 5,
            ItemKey::RdgM => 219,
            ItemKey::Trap => 68,
            ItemKey::OxygenTank => return Target::Class(ItemClass::OxygenTank),
        }))
    }
}

/// The quick-use bits the keys of `pressed` make: each its item's first quick
/// slot's (`holds`: what a quick slot holds), none for an item no quick slot
/// of the six holds.
fn quick_bits(pressed: impl Iterator<Item = ItemKey>, holds: impl Fn(QuickSlot) -> Option<QuickItem>) -> ActionMask {
    let slots = || (0..6).filter_map(QuickSlot::parse);
    pressed
        .filter_map(|key| slots().find(|&slot| holds(slot).is_some_and(|item| key.target().holds(item))))
        .fold(ActionMask::NONE, |bits, slot| bits.union(ActionMask::quick_use(slot)))
}

/// The input builder's end (process_first_person_mode's epilogue, its early
/// exit too), before patch 78's chord.
pub fn on_input_end(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut handler) = InputHandler::at_builder_end(ctx) else { return Flow::Continue };
    let mut pressed = ItemKey::ALL.into_iter().filter(|key| handler.action_present(key.action())).peekable();
    if pressed.peek().is_none() {
        return Flow::Continue;
    }
    let Some(inventory) = handler.local_inventory() else { return Flow::Continue };
    let bits = quick_bits(pressed, |slot| inventory.quick_item(slot));
    handler.add_actions(bits);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    const OXYGEN: u16 = 9; // scav_oxygen_1

    fn kit(items: &[Option<u16>]) -> impl Fn(QuickSlot) -> Option<QuickItem> + '_ {
        |slot| {
            let id = items.get(usize::from(slot.get())).copied().flatten()?;
            let class = if id == OXYGEN { ItemClass::OxygenTank } else { ItemClass::Other };
            Some(QuickItem { id: DictId(id), class })
        }
    }

    fn bit(n: u32) -> u32 {
        1 << (14 + n)
    }

    #[test]
    fn a_key_is_its_items_quick_slot() {
        // bandages, an RGD-5, an oxygen pack (the back), Onyx, an empty container, Onyx again
        let items = [Some(66), Some(5), Some(OXYGEN), Some(76), None, Some(76)];
        assert_eq!(quick_bits([ItemKey::Bandages].into_iter(), kit(&items)).bits(), bit(0));
        assert_eq!(quick_bits([ItemKey::Rgd5].into_iter(), kit(&items)).bits(), bit(1));
        assert_eq!(quick_bits([ItemKey::OxygenTank].into_iter(), kit(&items)).bits(), bit(2));
        assert_eq!(quick_bits([ItemKey::Onyx].into_iter(), kit(&items)).bits(), bit(3), "the first Onyx's slot");
        assert_eq!(quick_bits([ItemKey::Onyx, ItemKey::Bandages].into_iter(), kit(&items)).bits(), bit(3) | bit(0));
    }

    #[test]
    fn nothing_of_its_type_is_nothing() {
        let items = [Some(66), Some(67)];
        assert_eq!(quick_bits([ItemKey::Spring, ItemKey::RdgM, ItemKey::OxygenTank].into_iter(), kit(&items)),
                   ActionMask::NONE);
        assert_eq!(quick_bits(core::iter::empty(), kit(&items)), ActionMask::NONE);
        // a seventh quick slot has no key
        let far = [None, None, None, None, None, None, Some(65)];
        assert_eq!(quick_bits([ItemKey::Painkiller].into_iter(), kit(&far)), ActionMask::NONE);
    }

    #[test]
    fn the_actions_and_items_are_distinct_and_free() {
        // past the game's ids (0..47h) and its two "none"s (48h the options', 49h the binder's),
        // within the patch's 60h bindings (patch_026g.py's ITEM_HOTKEYS_BINDINGS)
        for (i, a) in ItemKey::ALL.iter().enumerate() {
            assert!((0x4A..0x60).contains(&a.action().0), "{a:?}");
            for b in &ItemKey::ALL[i + 1..] {
                assert_ne!(a.action(), b.action());
                assert_ne!(a.target(), b.target());
            }
        }
    }
}
