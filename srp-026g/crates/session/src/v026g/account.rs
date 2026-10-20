//! A 0.26g account as the lobby serves it (`docs/notes/lobby.md`
//! §2.5..§2.14): one account with up to three kits that share its level,
//! experience, skills and autobuy, one inventory they all equip from,
//! money, reputations and the faction whose quests it takes.
//!
//! Against 0.21d (`survarium::v021d::account`): a kit ("profile" to the
//! client) holds only its slots; level, experience, skills and autobuy
//! moved to the account. A slot names an item of the inventory by its
//! instance id and how many of it the kit takes; the item itself (its
//! condition or count, modifications, upgrades) is kept once.

use survarium::v021d;
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::item_params;
use survarium::v026g::loadouts::{self, Loadout};
use survarium::v026g::lobby::{FULL_CONDITION, KIT_PREMIUM, KIT_READY, limits};
use survarium::v026g::quests::FACTIONS;

/// The kits an account has: the client's array holds three.
pub const KITS: usize = limits::KITS;
/// Slots of a kit.
pub const KIT_SLOTS: usize = limits::KIT_SLOTS;
/// A new 0.26g account's silver and gold (the owner's choice, 2026-10-12:
/// 2,000 silver and no gold; no source of the time gives the numbers).
pub const STARTING_MONEY: u32 = 2_000;
pub const STARTING_PREMIUM_MONEY: u32 = 0;
/// The highest level (PROVEN: the game's knowledge base, "the highest
/// possible level of a character is the hundredth", and the wiki's reward
/// table of February 2015, which ends at 100).
pub const MAX_LEVEL: u8 = 100;

/// One upgrade on an item (`survarium::upgrade`): a camouflage or a decal
/// (`super::items_rules`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Upgrade {
    /// The item id of the cosmetic; it stays in the inventory.
    pub id: u32,
    /// Its dict id.
    pub a: u32,
    /// What the decal editor set (`46 05`).
    pub value: u64,
    /// The slot: 0 and 1 decals, 3 the camouflage.
    pub param: u8,
}

/// What an item keeps of its modification changes ("reroll",
/// `super::items_rules`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reroll {
    /// The modification that was changed before, the only one that may be
    /// changed again; 0: none yet.
    pub modification: u32,
    /// The changes paid for so far.
    pub count: u32,
    /// A paid change that waits for its choice: the modification it
    /// replaces and the three offered.
    pub offer: Option<(u32, [u32; 3])>,
}

/// One item instance of the inventory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The instance id: non-zero, never reused.
    pub item_id: u32,
    pub dict_id: u16,
    /// A stack's count in units (rounds of ammo), or an item's condition
    /// × 10 000 ([`FULL_CONDITION`] is 100).
    pub condition_or_stack: u32,
    /// The Unix second its repair ends at, and the one from which a
    /// modification may be changed for silver again; 0: none. (The wire
    /// carries the seconds left, `wire::item_descr_at`.)
    pub repair_time: u32,
    pub reroll_time: u32,
    /// `(id, state)`: a modification (false) or a defect (true), each of
    /// `survarium::v026g::modifications`; at most 8.
    pub modifications: Vec<(u32, bool)>,
    /// At most 3.
    pub upgrades: Vec<Upgrade>,
    pub reroll: Reroll,
}

/// What a slot of a kit holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KitSlot {
    /// An item of the account's inventory.
    pub item_id: u32,
    /// For a stack the units the kit takes into a life (at most what is
    /// owned); 1 for anything else.
    pub count: u32,
}

/// One kit: the client's "profile".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kit {
    /// Non-zero and distinct among the account's kits.
    pub id: u32,
    /// Empty: the client shows its own name for the kit's place.
    pub name: String,
    /// Bumped on every change of what the kit holds or gives, so the client
    /// rebuilds the character it shows.
    pub revision: u32,
    pub slots: [Option<KitSlot>; KIT_SLOTS],
}

impl Kit {
    pub fn empty(id: u32) -> Self {
        Self {
            id,
            name: String::new(),
            revision: 1,
            slots: [None; KIT_SLOTS],
        }
    }

    /// The slot holding `item_id`, if any.
    pub fn slot_of(&self, item_id: u32) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.is_some_and(|slot| slot.item_id == item_id))
    }
}

/// A quest the account has. 0.26g has no proposals to swap to (the client's
/// `accept_change_quest` is gone): every quest held is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuestInstance {
    pub instance_id: u32,
    pub quest_id: u32,
    /// Games counted towards it.
    pub games: u32,
    /// Unix seconds.
    pub expires_at: u64,
}

/// Everything the 0.26g lobby keeps of an account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// The nickname other players see.
    pub name: String,
    /// The clan abbreviation; empty without a clan.
    pub clan_tag: String,
    pub generic_money: u32,
    pub premium_money: u32,
    /// When the premium account runs out, Unix seconds; 0 without.
    pub premium_until: u64,
    /// All the experience earned; the level follows from it ([`level_at`]).
    pub exp_total: u32,
    /// `(skill id, points)`, only skills with points.
    pub skills: Vec<(u16, u8)>,
    /// `(faction, points)` for the factions the lobby shows, 1..=4.
    pub reputations: Vec<(u8, u32)>,
    /// The faction whose quests it takes; 0 until it chooses one.
    pub quest_faction: u8,
    /// The ranking's rating, and what it was before the last match that
    /// moved it.
    pub rating: u16,
    pub rating_before: u16,
    /// What the sides of a match are made with, beside its gear and its
    /// level: not the ranking's rating, and shown to nobody
    /// ([`super::balance`]).
    pub balance: super::balance::Balance,
    pub kits: Vec<Kit>,
    /// The id of the kit it plays.
    pub active_kit: u32,
    pub autobuy: bool,
    /// Everything it owns, the equipped items included.
    pub inventory: Vec<Item>,
    /// The id the next new item instance gets.
    pub next_item_id: u32,
    pub quests: Vec<QuestInstance>,
    /// `(quest id, until)`: quests resting after being done, Unix seconds.
    pub quest_cooldowns: Vec<(u32, u64)>,
    pub next_quest_instance: u32,
    /// The last tutorial step the client reported (`4B 01`), and whether
    /// the tutorial is over (done or skipped, `4B 02`).
    pub tutorial_step: u8,
    pub tutorial_done: bool,
    /// The auto-repair box, the tutorial's steps and the daily bonus
    /// ([`super::extras`]).
    pub extras: super::extras::Extras,
    /// `(map, matches)`: how many matches the account was put in on each
    /// map (`maps::MapMode::level`), by map; the matchmaker draws the maps
    /// its players played less more often ([`super::matchmaking`]).
    pub map_plays: Vec<(u8, u32)>,
    /// The settings of the last room it made by hand, its create window's
    /// start ([`super::room_settings`]); none before its first.
    pub room_settings: Option<super::room_settings::RoomSettings>,
}

/// The level reached with `total` experience, where it starts and where
/// the next one does. 0.21d's table (the wiki's: 500 for the first level,
/// 250 more for each next), counted from level 0 as the wiki did: a 0.26g
/// account's level is the skill points it has unlocked, so a new one has
/// none. LIKELY (no table is in the client's data).
pub fn level_at(total: u32) -> (u8, u32, u32) {
    let (mut level, mut start) = (0_u8, 0_u32);
    let mut next = v021d::results::experience_for_level(2);
    while total >= next && level < MAX_LEVEL {
        level += 1;
        start = next;
        next = next.saturating_add(v021d::results::experience_for_level(level + 2));
    }
    (level, start, next)
}

/// A name cut to what the client's 64-byte buffers take.
pub fn cap_name(name: &str) -> String {
    survarium::v026g::lobby::cut(name, limits::NAME).to_owned()
}

impl Account {
    /// A new account: three kits, the first wearing the game's starter
    /// loadout ([`loadouts::starter`]: the TT-33 and a clip for it), and
    /// the rest of what every new player owned in the inventory
    /// ([`loadouts::starter_stash`]: the Mosin carbine, a clip for it and
    /// the PPSh-41).
    pub fn new(name: &str) -> Self {
        let mut account = Self::with_loadout(name, loadouts::starter());
        let dictionary = Dictionary::shared();
        for &(dict_id, units) in loadouts::starter_stash() {
            let stack = dictionary.item(dict_id).is_some_and(|item| item.is_stack);
            account.add_item(dict_id, if stack { units } else { FULL_CONDITION });
        }
        account
    }

    /// A new account whose first kit wears `loadout`.
    pub fn with_loadout(name: &str, loadout: &Loadout) -> Self {
        let dictionary = Dictionary::shared();
        let mut account = Self {
            name: cap_name(name),
            clan_tag: String::new(),
            generic_money: STARTING_MONEY,
            premium_money: STARTING_PREMIUM_MONEY,
            premium_until: 0,
            exp_total: 0,
            skills: Vec::new(),
            reputations: FACTIONS.iter().map(|&faction| (faction, 0)).collect(),
            quest_faction: 0,
            rating: super::pay::START_RATING,
            rating_before: super::pay::START_RATING,
            balance: Default::default(),
            kits: (1..=KITS as u32).map(Kit::empty).collect(),
            active_kit: 1,
            autobuy: false,
            inventory: Vec::new(),
            next_item_id: 1,
            quests: Vec::new(),
            quest_cooldowns: Vec::new(),
            next_quest_instance: 1,
            tutorial_step: 0,
            tutorial_done: true,
            extras: super::extras::Extras {
                tutorial_flags: super::tutorial::ALL_DONE,
                ..Default::default()
            },
            map_plays: Vec::new(),
            room_settings: None,
        };
        for worn in &loadout.items {
            let Some(entry) = dictionary.item(worn.dict_id) else {
                continue;
            };
            let slot = usize::from(worn.slot);
            if slot >= KIT_SLOTS || !dictionary.fits(worn.dict_id, worn.slot) {
                continue;
            }
            let (value, count) = if entry.is_stack {
                (worn.count, worn.count)
            } else {
                (FULL_CONDITION, 1)
            };
            let item_id = account.add_item(worn.dict_id, value);
            account.kits[0].slots[slot] = Some(KitSlot { item_id, count });
        }
        account
    }

    /// Puts units of a stack into the inventory: onto the stack owned, else
    /// as a new one. Returns its item id.
    pub fn add_to_stack(&mut self, dict_id: u16, units: u32) -> u32 {
        match self
            .inventory
            .iter_mut()
            .find(|item| item.dict_id == dict_id)
        {
            Some(owned) => {
                owned.condition_or_stack = owned.condition_or_stack.saturating_add(units);
                owned.item_id
            }
            None => self.add_item(dict_id, units),
        }
    }

    /// A new instance into the inventory; returns its id.
    pub fn add_item(&mut self, dict_id: u16, condition_or_stack: u32) -> u32 {
        let item_id = self.next_item_id;
        self.next_item_id += 1;
        self.inventory.push(Item {
            item_id,
            dict_id,
            condition_or_stack,
            repair_time: 0,
            reroll_time: 0,
            modifications: Vec::new(),
            upgrades: Vec::new(),
            reroll: Reroll::default(),
        });
        item_id
    }

    pub fn item(&self, item_id: u32) -> Option<&Item> {
        self.inventory.iter().find(|item| item.item_id == item_id)
    }

    pub fn item_mut(&mut self, item_id: u32) -> Option<&mut Item> {
        self.inventory
            .iter_mut()
            .find(|item| item.item_id == item_id)
    }

    pub fn kit(&self, kit_id: u32) -> Option<&Kit> {
        self.kits.iter().find(|kit| kit.id == kit_id)
    }

    pub fn kit_index(&self, kit_id: u32) -> Option<usize> {
        self.kits.iter().position(|kit| kit.id == kit_id)
    }

    /// The kit it plays; the first if the active id is stale.
    pub fn active(&self) -> Option<&Kit> {
        self.kit(self.active_kit).or_else(|| self.kits.first())
    }

    /// Whether its premium account holds at `now` (Unix seconds).
    pub fn is_premium(&self, now: u64) -> bool {
        self.premium_until > now
    }

    /// The whole hours of premium left, at least 1 while it holds.
    pub fn premium_hours(&self, now: u64) -> Option<u32> {
        self.is_premium(now).then(|| {
            let hours = (self.premium_until - now).div_ceil(3600);
            hours.min(u64::from(u32::MAX)) as u32
        })
    }

    /// Whether the kit at `index` may be used at `now`: every kit of the
    /// account, premium or not. The original game kept a kit for a premium
    /// account (the client's state 2, `st_premium_profile_hint`, PROVEN;
    /// that it was the third, LIKELY); srp gives all three to everyone (the
    /// owner's choice, 2026-10-17), so no kit is sent as [`KIT_PREMIUM`].
    pub fn kit_usable(&self, index: usize, _now: u64) -> bool {
        index < self.kits.len()
    }

    /// The active kit is none the account may use: back to the first kit.
    /// Call it before the account goes out (a sign-in, a Play). Returns
    /// whether the active kit changed.
    pub fn settle_active_kit(&mut self, now: u64) -> bool {
        let usable = self
            .kit_index(self.active_kit)
            .is_some_and(|index| self.kit_usable(index, now));
        match self.kits.first() {
            Some(first) if !usable => {
                self.active_kit = first.id;
                true
            }
            _ => false,
        }
    }

    /// The state byte the client gets for the kit at `index`.
    pub fn kit_state(&self, index: usize, now: u64) -> u8 {
        if self.kit_usable(index, now) {
            KIT_READY
        } else {
            KIT_PREMIUM
        }
    }

    /// `(level, where it starts, where the next starts)`.
    pub fn level_progress(&self) -> (u8, u32, u32) {
        level_at(self.exp_total)
    }

    pub fn level(&self) -> u8 {
        self.level_progress().0
    }

    /// Adds experience; returns the levels reached by it.
    pub fn add_experience(&mut self, amount: u32) -> Vec<u8> {
        let before = self.level();
        self.exp_total = self.exp_total.saturating_add(amount);
        (before + 1..=self.level()).collect()
    }

    /// The skill points not spent.
    pub fn free_skill_points(&self) -> u32 {
        let spent: u32 = self
            .skills
            .iter()
            .map(|&(_, points)| u32::from(points))
            .sum();
        u32::from(self.level()).saturating_sub(spent)
    }

    pub fn reputation(&self, faction: u8) -> u32 {
        self.reputations
            .iter()
            .find(|&&(id, _)| id == faction)
            .map_or(0, |&(_, points)| points)
    }

    /// Its reputation level with `faction` (`factions_dict`).
    pub fn reputation_level(&self, faction: u8) -> u8 {
        Dictionary::shared().reputation_level(faction, self.reputation(faction))
    }

    /// Adds reputation with a faction the lobby shows; returns the levels
    /// reached by it. The points stop at what the wire's u16 holds.
    pub fn add_reputation(&mut self, faction: u8, amount: u32) -> Vec<u8> {
        let before = self.reputation_level(faction);
        let Some(entry) = self.reputations.iter_mut().find(|(id, _)| *id == faction) else {
            return Vec::new();
        };
        entry.1 = entry.1.saturating_add(amount).min(u32::from(u16::MAX));
        (before + 1..=self.reputation_level(faction)).collect()
    }

    /// The slots of `kit` whose item is worn in a slot another worn item
    /// covers ([`item_params::covered_slots`]), and which the rule of the
    /// lobby (`rules::move_items`) never lets be filled. An item covering
    /// more slots is kept before one covering fewer (then by slot): it
    /// takes off what it covers, and an item that would cover a kept one is
    /// itself taken off.
    pub fn covered_items(&self, kit: &Kit) -> Vec<usize> {
        let covers = |slot: usize| -> &'static [u8] {
            kit.slots[slot]
                .and_then(|held| self.item(held.item_id))
                .map_or(&[], |item| item_params::covered_slots(item.dict_id))
        };
        let mut worn: Vec<usize> = (0..KIT_SLOTS)
            .filter(|&slot| kit.slots[slot].is_some())
            .collect();
        worn.sort_by_key(|&slot| (std::cmp::Reverse(covers(slot).len()), slot));
        let mut kept: Vec<usize> = Vec::new();
        let mut covered = [false; KIT_SLOTS];
        let mut off = Vec::new();
        for slot in worn {
            let over_kept = covers(slot).iter().any(|&c| kept.contains(&usize::from(c)));
            if covered[slot] || over_kept {
                off.push(slot);
                continue;
            }
            for &c in covers(slot) {
                if let Some(c) = covered.get_mut(usize::from(c)) {
                    *c = true;
                }
            }
            kept.push(slot);
        }
        off.sort_unstable();
        off
    }

    /// Takes off, to the inventory, every item [`Self::covered_items`]
    /// names, in every kit: a kit saved before the rule (a mask under a
    /// helmet that has one built in). How many it took off.
    pub fn uncover_kits(&mut self) -> usize {
        let mut taken = 0;
        for index in 0..self.kits.len() {
            let off = self.covered_items(&self.kits[index]);
            if off.is_empty() {
                continue;
            }
            let kit = &mut self.kits[index];
            for slot in &off {
                kit.slots[*slot] = None;
            }
            kit.revision = kit.revision.wrapping_add(1);
            taken += off.len();
        }
        taken
    }

    /// What a kit weighs, as the lobby counts it
    /// ([`item_params::carried_weight`]).
    pub fn kit_weight(&self, kit: &Kit) -> f32 {
        kit.slots
            .iter()
            .flatten()
            .filter_map(|slot| self.item(slot.item_id).map(|item| (item, slot)))
            .map(|(item, slot)| item_params::carried_weight(item.dict_id, slot.count))
            .sum()
    }

    /// Bring the kits up to the inventory after it changed: a slot whose
    /// item is gone is emptied, and a stack's slot takes at most what is
    /// owned (a stack down to nothing empties its slot). A kit whose slots
    /// changed gets a new revision. Returns whether anything changed.
    pub fn sync_kits(&mut self) -> bool {
        let dictionary = Dictionary::shared();
        // An upgrade whose cosmetic is gone (sold) is no upgrade, and the
        // kits wearing the item it was on show another one.
        let stripped = super::items_rules::drop_gone_upgrades(&mut self.inventory);
        let inventory = &self.inventory;
        let mut any = false;
        for kit in &mut self.kits {
            let mut changed = kit
                .slots
                .iter()
                .flatten()
                .any(|slot| stripped.contains(&slot.item_id));
            for slot in &mut kit.slots {
                let Some(held) = slot.as_mut() else {
                    continue;
                };
                let Some(owned) = inventory.iter().find(|item| item.item_id == held.item_id) else {
                    *slot = None;
                    changed = true;
                    continue;
                };
                if dictionary.is_stack(owned.dict_id) && held.count > owned.condition_or_stack {
                    if owned.condition_or_stack == 0 {
                        *slot = None;
                    } else {
                        held.count = owned.condition_or_stack;
                    }
                    changed = true;
                }
            }
            if changed {
                kit.revision = kit.revision.wrapping_add(1);
                any = true;
            }
        }
        any
    }

    /// Every kit gives other modifiers (the skills changed): new revisions.
    pub fn bump_kits(&mut self) {
        for kit in &mut self.kits {
            kit.revision = kit.revision.wrapping_add(1);
        }
    }

    /// Make a loaded account consistent, so that what the lobby sends from
    /// it can not crash a client: the next ids past every id in use, at
    /// most three kits with distinct non-zero ids, an active kit that
    /// exists, no duplicate item id, item lists within the client's
    /// limits, no slot holding an item that is gone or does not fit, and
    /// none worn in a slot another worn item covers
    /// ([`Self::uncover_kits`]). `true` if anything changed.
    ///
    /// It never deletes an item for a definition the game's data lacks:
    /// that is data made without a patch, and the load refuses it before
    /// this runs ([`super::known::Lacking`]).
    pub fn repair(&mut self) -> bool {
        let dictionary = Dictionary::shared();
        let before = self.clone();

        let mut seen = Vec::new();
        self.inventory.retain(|item| {
            let keep = item.item_id != 0 && !seen.contains(&item.item_id);
            seen.push(item.item_id);
            keep
        });
        for item in &mut self.inventory {
            // What the client's tables lack would crash it.
            super::items_rules::make_safe(item);
            item.modifications.truncate(limits::MODIFICATIONS);
            item.upgrades.truncate(limits::UPGRADES);
        }
        let next_item = self.inventory.iter().map(|item| item.item_id + 1).max();
        self.next_item_id = self.next_item_id.max(next_item.unwrap_or(1)).max(1);
        let next_quest = self.quests.iter().map(|quest| quest.instance_id + 1).max();
        self.next_quest_instance = self.next_quest_instance.max(next_quest.unwrap_or(1)).max(1);

        self.kits.truncate(KITS);
        let mut ids: Vec<u32> = Vec::new();
        for kit in &mut self.kits {
            if kit.id == 0 || ids.contains(&kit.id) {
                kit.id = (1..).find(|id| !ids.contains(id)).unwrap_or(1);
            }
            ids.push(kit.id);
            kit.name = cap_name(&kit.name);
        }
        while self.kits.len() < KITS {
            let id = (1..).find(|id| !ids.contains(id)).unwrap_or(1);
            ids.push(id);
            self.kits.push(Kit::empty(id));
        }
        if self.kit(self.active_kit).is_none() {
            self.active_kit = self.kits[0].id;
        }
        let inventory = &self.inventory;
        for kit in &mut self.kits {
            for (index, slot) in kit.slots.iter_mut().enumerate() {
                let fits = slot.is_some_and(|held| {
                    inventory
                        .iter()
                        .find(|item| item.item_id == held.item_id)
                        .is_some_and(|item| dictionary.fits(item.dict_id, index as u8))
                });
                if slot.is_some() && !fits {
                    *slot = None;
                }
            }
        }
        self.uncover_kits();
        self.sync_kits();
        self.name = cap_name(&self.name);
        self.clan_tag =
            survarium::v026g::lobby::cut(&self.clan_tag, limits::ACCOUNT_CLAN_TAG).to_owned();
        for faction in FACTIONS {
            if !self.reputations.iter().any(|&(id, _)| id == faction) {
                self.reputations.push((faction, 0));
            }
        }
        if !FACTIONS.contains(&self.quest_faction) {
            self.quest_faction = 0;
        }
        *self != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_account_wears_the_starter_in_its_first_kit() {
        let dictionary = Dictionary::shared();
        let account = Account::new("Alice");
        assert_eq!(account.kits.len(), 3);
        assert_eq!(
            account.kits.iter().map(|kit| kit.id).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(account.active_kit, 1);
        let pistol = dictionary.item_named("tt_33").unwrap().dict_id;
        let ammo = dictionary.item_named("ammo_7.62x25").unwrap().dict_id;
        let worn: Vec<(usize, u16, u32)> = account.kits[0]
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                slot.map(|slot| {
                    (
                        index,
                        account.item(slot.item_id).unwrap().dict_id,
                        slot.count,
                    )
                })
            })
            .collect();
        let rifle = dictionary.item_named("mosin_nagant").unwrap().dict_id;
        let rounds = dictionary.item_named("ammo_7.62x54r_old").unwrap().dict_id;
        let ppsh = dictionary.item_named("ppsh").unwrap().dict_id;
        assert_eq!(worn, [(7, pistol, 1), (8, ammo, 50)]);
        // The three free weapons and their ammo; only the pistol is worn.
        assert_eq!(
            account
                .inventory
                .iter()
                .map(|item| (item.item_id, item.dict_id, item.condition_or_stack))
                .collect::<Vec<_>>(),
            [
                (1, pistol, FULL_CONDITION),
                (2, ammo, 50),
                (3, rifle, FULL_CONDITION),
                (4, rounds, 10),
                (5, ppsh, FULL_CONDITION),
            ]
        );
        // Each ammo is its weapon's, and the PPSh-41 fires the pistol's.
        assert!(dictionary.compatible(pistol, ammo));
        assert!(dictionary.compatible(rifle, rounds));
        assert!(dictionary.compatible(ppsh, ammo));
        assert!(account.kits[1].slots.iter().all(Option::is_none));
        assert_eq!(account.next_item_id, 6);
        assert_eq!(
            (account.generic_money, account.premium_money),
            (STARTING_MONEY, STARTING_PREMIUM_MONEY)
        );
        assert_eq!((account.generic_money, account.premium_money), (2_000, 0));
        assert_eq!((account.rating, account.rating_before), (100, 100));
        assert_eq!(account.reputations, [(1, 0), (2, 0), (3, 0), (4, 0)]);
        // No faction chosen: the client asks.
        assert_eq!(account.quest_faction, 0);
        assert_eq!(account.level(), 0);
        // Nothing to repair in a new account.
        assert!(!account.clone().repair());
        // 0.8 kg of pistol and one 0.5 kg clip.
        assert!((account.kit_weight(&account.kits[0]) - 1.3).abs() < 1e-5);
    }

    #[test]
    fn the_games_test_loadouts_can_be_worn() {
        let loadout = loadouts::Loadouts::shared().usable().next().unwrap();
        let account = Account::with_loadout("Alice", loadout);
        assert_eq!(
            account.kits[0].slots.iter().flatten().count(),
            loadout.items.len()
        );
        assert!(account.kit_weight(&account.kits[0]) <= item_params::player().max_carried_weight);
    }

    #[test]
    fn levels_follow_the_experience_table() {
        assert_eq!(level_at(0), (0, 0, 500));
        assert_eq!(level_at(499), (0, 0, 500));
        assert_eq!(level_at(500), (1, 500, 1250));
        assert_eq!(level_at(1250), (2, 1250, 2250));
        assert_eq!(level_at(u32::MAX).0, MAX_LEVEL);
        // The hundredth level: 500 for the first and 250 more for each
        // next, a hundred times.
        assert_eq!(level_at(1_287_499).0, 99);
        assert_eq!(level_at(1_287_500).0, 100);
        let mut account = Account::new("Alice");
        assert_eq!(account.add_experience(1300), [1, 2]);
        assert_eq!(account.level_progress(), (2, 1250, 2250));
        assert_eq!(account.free_skill_points(), 2);
        account.skills.push((101, 2));
        assert_eq!(account.free_skill_points(), 0);
    }

    #[test]
    fn the_third_kit_needs_no_premium() {
        let mut account = Account::new("Alice");
        assert_eq!(account.premium_hours(100), None);
        assert_eq!(account.kit_state(0, 100), KIT_READY);
        assert_eq!(account.kit_state(1, 100), KIT_READY);
        assert_eq!(account.kit_state(2, 100), KIT_READY);
        account.premium_until = 100 + 2 * 3600 + 1;
        assert_eq!(account.kit_state(2, 100), KIT_READY);
        assert_eq!(account.premium_hours(100), Some(3));
        assert!(!account.is_premium(account.premium_until));
        // Playing the third kit when the premium runs out: it stays.
        account.active_kit = 3;
        assert!(!account.settle_active_kit(account.premium_until));
        assert_eq!(account.active_kit, 3);
        // An active kit that is none of the account's: the first kit.
        account.active_kit = 9;
        assert!(account.settle_active_kit(100));
        assert_eq!(account.active_kit, 1);
    }

    #[test]
    fn reputation_levels_come_from_the_dictionary() {
        let mut account = Account::new("Alice");
        // Scavengers: 100 for level 1, 420 for level 2.
        assert_eq!(account.add_reputation(1, 99), Vec::<u8>::new());
        assert_eq!(account.add_reputation(1, 400), [1, 2]);
        assert_eq!(account.reputation_level(1), 2);
        // Not a lobby faction.
        assert_eq!(account.add_reputation(9, 400), Vec::<u8>::new());
        // What the wire's u16 holds.
        account.add_reputation(3, u32::MAX);
        assert_eq!(account.reputation(3), 65535);
    }

    #[test]
    fn kits_follow_the_inventory() {
        let mut account = Account::new("Alice");
        let ammo = account.kits[0].slots[8].unwrap().item_id;
        let revision = account.kits[0].revision;
        assert!(!account.sync_kits());
        // The stack shrinks under what the slot takes.
        account.item_mut(ammo).unwrap().condition_or_stack = 20;
        assert!(account.sync_kits());
        assert_eq!(account.kits[0].slots[8].unwrap().count, 20);
        assert_eq!(account.kits[0].revision, revision + 1);
        // The other kits did not change.
        assert_eq!(account.kits[1].revision, 1);
        // The stack gone: its slot empties.
        account.inventory.retain(|item| item.item_id != ammo);
        assert!(account.sync_kits());
        assert_eq!(account.kits[0].slots[8], None);
        assert!(account.kits[0].slots[7].is_some());
    }

    #[test]
    fn a_loaded_account_is_made_safe_to_send() {
        let mut account = Account::new("Alice");
        // An item of no dictionary (kept: the load refuses data that lacks
        // it, `known`), a duplicate id, a stale slot, a 4th kit, two kits
        // with one id, a stale active kit and a low next id.
        account.inventory.push(Item {
            item_id: 9,
            dict_id: 9999,
            condition_or_stack: 1,
            repair_time: 0,
            reroll_time: 0,
            modifications: Vec::new(),
            upgrades: Vec::new(),
            reroll: Reroll::default(),
        });
        let copy = account.inventory[0].clone();
        account.inventory.push(copy);
        account.kits[1].slots[0] = Some(KitSlot {
            item_id: 77,
            count: 1,
        });
        // The pistol in a helmet's slot.
        account.kits[1].slots[1] = Some(KitSlot {
            item_id: 1,
            count: 1,
        });
        account.kits.push(Kit::empty(1));
        account.kits[2].id = 1;
        account.active_kit = 42;
        account.next_item_id = 1;
        // Twelve defects (the client's array holds 8), and one twice.
        account.inventory[0].modifications = [3000, 3010, 3020, 3030, 3040, 3050, 3060]
            .into_iter()
            .chain([4000, 4020, 4030, 4040, 4050, 3000])
            .map(|id| (id, true))
            .collect();
        // An id the client's tables lack, and a modification kept as a
        // defect.
        account.inventory[1].modifications = vec![(7, false), (2000, true)];
        account.quest_faction = 9;
        assert!(account.repair());
        assert_eq!(account.inventory.len(), 6);
        assert!(account.item(9).is_some_and(|item| item.dict_id == 9999));
        assert_eq!(account.inventory[0].modifications.len(), 8);
        assert!(account.inventory[1].modifications.is_empty());
        assert_eq!(account.kits.len(), 3);
        let ids: Vec<u32> = account.kits.iter().map(|kit| kit.id).collect();
        assert_eq!(ids, [1, 2, 3]);
        assert_eq!(account.active_kit, 1);
        assert_eq!(account.next_item_id, 10);
        assert!(account.kits[1].slots.iter().all(Option::is_none));
        assert_eq!(account.quest_faction, 0);
        // And it is stable.
        assert!(!account.repair());
    }
}
