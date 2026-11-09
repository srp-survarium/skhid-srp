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
use survarium::v026g::golden;
use survarium::v026g::item_params;
use survarium::v026g::loadouts::{self, KitLayout, Loadout, POCKET_SLOTS};
use survarium::v026g::lobby::{FULL_CONDITION, limits};
use survarium::v026g::quests::FACTIONS;
use survarium::v026g::skills::{self, SkillsTree};

/// The kits an account has: the client's array holds three.
pub const KITS: usize = limits::KITS;
/// Slots of a kit.
pub const KIT_SLOTS: usize = limits::KIT_SLOTS;
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
    /// The modification changed last (the window marks it); 0: none yet.
    /// Not a lock: any of the item's own may be changed (the owner's rule
    /// of 2026-10-22, `docs/notes/reroll-keep-old.md`). How many changes
    /// were made is not kept: the price does not grow with them (the
    /// owner's rule of 2026-10-25).
    pub modification: u32,
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
    /// Silver, the one currency (gold was removed 2026-11-08: the balances
    /// were paid back in silver once, `persist::SqliteGameRepository::pay_back_once`).
    pub generic_money: u32,
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
    /// A rating of its wins and losses against what was expected: not the
    /// ranking's, shown to nobody, kept for the log and its refit
    /// ([`super::balance`]); the sides no longer count it.
    pub balance: super::balance::Balance,
    /// Its kills and deaths by the equipment level of the matches: what
    /// the sides of a match are made with ([`super::level_stats`]).
    pub level_stats: super::level_stats::LevelStats,
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
    /// Account-wide, persistent allowances for the first three introductions to each mode.
    pub mode_introductions: super::mode_intro::Allowances,
    /// The matches whose results screen it used a drop card on, sorted,
    /// of those whose reports are kept: one card a match
    /// ([`super::rewards::Results::use_drop_card`]). Kept with the account,
    /// so the mark is written with the card taken and the item given.
    pub drop_cards: Vec<u32>,
    /// Its five kit presets, preset 1 first; none: empty. The active
    /// one's place is always empty: the kits are it ([`super::presets`]).
    pub presets: [Option<super::presets::Preset>; super::presets::PRESETS],
    /// The preset the kits are, 1 to 5.
    pub active_preset: u8,
}

/// The pockets of a kit's `slots` ([`POCKET_SLOTS`]) holding an item type
/// (a dict id; `dict_of` gives an item id's) that an earlier pocket of it
/// holds: the rule is one pocket an item type, its units within that
/// pocket's size (`docs/notes/one-pocket-a-type.md`). A booster in a pocket
/// adds its bonus at every spawn, so a type in two pockets gave it twice.
/// Ammo slots are no pockets: the ammo window spreads one ammo over the
/// ammo bags by design.
pub fn doubled_pockets(
    slots: &[Option<KitSlot>; KIT_SLOTS],
    dict_of: impl Fn(u32) -> Option<u16>,
) -> Vec<usize> {
    let mut seen: Vec<u16> = Vec::new();
    let mut doubled = Vec::new();
    for pocket in POCKET_SLOTS.map(usize::from) {
        let Some(dict_id) = slots[pocket].and_then(|held| dict_of(held.item_id)) else {
            continue;
        };
        if seen.contains(&dict_id) {
            doubled.push(pocket);
        } else {
            seen.push(dict_id);
        }
    }
    doubled
}

/// [`KitLayout::kept`] put into a kit's or a preset's `slots`: `held` is
/// each slot's dict id and units (`None` for an empty one, or one whose
/// item is not known, which stays as it is), `layout` the kit's, `units` a
/// slot's units.
fn keep_into<S>(
    slots: &mut [Option<S>; KIT_SLOTS],
    held: &[Option<(u16, u32)>; KIT_SLOTS],
    layout: &KitLayout,
    units: fn(&mut S) -> &mut u32,
) {
    let kept = layout.kept(held);
    for ((slot, held), kept) in slots.iter_mut().zip(held).zip(kept) {
        if held.is_none() {
            continue;
        }
        match (slot.as_mut(), kept) {
            (Some(slot), Some((_, kept))) => *units(slot) = kept,
            _ => *slot = None,
        }
    }
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

/// The experience where `level` starts ([`level_at`]'s table).
pub fn level_start(level: u8) -> u32 {
    (0..level.min(MAX_LEVEL)).fold(0, |start, below| {
        start.saturating_add(v021d::results::experience_for_level(below + 2))
    })
}

/// A name cut to what the client's 64-byte buffers take.
pub fn cap_name(name: &str) -> String {
    survarium::v026g::lobby::cut(name, limits::NAME).to_owned()
}

impl Account {
    /// The stock account, the tests' (and [`super::GameAccounts::get_or_create`]'s):
    /// three kits, the first wearing the game's free pistol
    /// ([`loadouts::starter`]: the TT-33 and a clip for it), the Mosin
    /// carbine, a clip for it and the PPSh-41 owned
    /// ([`loadouts::starter_stash`]), no money. A player's account is made
    /// from nothing and what the game data's table gives
    /// ([`super::GameAccounts::sign_up`], `tutorial::NewAccount`).
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
            generic_money: 0,
            exp_total: 0,
            skills: Vec::new(),
            reputations: FACTIONS.iter().map(|&faction| (faction, 0)).collect(),
            quest_faction: 0,
            rating: super::pay::START_RATING,
            rating_before: super::pay::START_RATING,
            balance: Default::default(),
            level_stats: Default::default(),
            kits: (1..=KITS as u32).map(Kit::empty).collect(),
            active_kit: 1,
            // On for a new account (the owner's of 2026-10-23); the player
            // may turn it off.
            autobuy: true,
            inventory: Vec::new(),
            next_item_id: 1,
            quests: Vec::new(),
            quest_cooldowns: Vec::new(),
            next_quest_instance: 1,
            extras: Default::default(),
            map_plays: Vec::new(),
            room_settings: None,
            mode_introductions: Default::default(),
            drop_cards: Vec::new(),
            presets: [None; super::presets::PRESETS],
            active_preset: super::presets::FIRST_PRESET,
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
        let mut slots = account.kits[0].slots;
        account.keep_to_layout(&mut slots);
        account.kits[0].slots = slots;
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

    /// A new instance into the inventory; returns its id. A golden weapon
    /// comes with its silver bonus as its own first modification, a premium
    /// armour piece with its two premium bonuses
    /// ([`golden::starting_modifications`]), whatever made it (the shop, a
    /// bundle, the admin).
    pub fn add_item(&mut self, dict_id: u16, condition_or_stack: u32) -> u32 {
        let item_id = self.next_item_id;
        self.next_item_id += 1;
        self.inventory.push(Item {
            item_id,
            dict_id,
            condition_or_stack,
            modifications: golden::starting_modifications(dict_id),
            upgrades: Vec::new(),
            reroll: Reroll::default(),
        });
        item_id
    }

    pub fn item(&self, item_id: u32) -> Option<&Item> {
        self.inventory.iter().find(|item| item.item_id == item_id)
    }

    /// The dictionary ids of everything it owns, for a drop roll
    /// (`drops::roll`).
    pub fn owned_dict_ids(&self) -> Vec<u16> {
        self.inventory.iter().map(|item| item.dict_id).collect()
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

    /// Whether the kit at `index` may be used at `now`: every kit of the
    /// account. The original game kept a kit for a premium account (the
    /// client's state 2, `KIT_PREMIUM`, PROVEN; that it was the third,
    /// LIKELY); srp gives all three to everyone (the owner's choice,
    /// 2026-10-17) and has no premium account (2026-11-09), so every kit is
    /// sent as `KIT_READY`.
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

    /// The pockets of `kit` that break the rule of one pocket an item type
    /// ([`doubled_pockets`]).
    pub fn doubled_pockets(&self, kit: &Kit) -> Vec<usize> {
        doubled_pockets(&kit.slots, |item_id| {
            self.item(item_id).map(|item| item.dict_id)
        })
    }

    /// Makes every kit keep the rule of one pocket an item type
    /// (`docs/notes/one-pocket-a-type.md`): of the pockets holding one type,
    /// the first keeps it, with the most units any of them held, and the
    /// others are emptied. A slot only names units of the inventory's stack,
    /// so nothing leaves the inventory: what the emptied pockets named is in
    /// the storage. (The most, not the sum: every pocket of a kit has one
    /// size, which the server does not know, and each of them held what fit
    /// it.) A kit that changed gets a new revision. How many pockets it
    /// emptied.
    pub fn merge_pockets(&mut self) -> usize {
        let mut emptied = 0;
        for index in 0..self.kits.len() {
            let mut slots = self.kits[index].slots;
            // `(dict id, the first pocket holding it)`.
            let mut first: Vec<(u16, usize)> = Vec::new();
            let mut changed = false;
            for pocket in POCKET_SLOTS.map(usize::from) {
                let Some(held) = slots[pocket] else {
                    continue;
                };
                let Some(dict_id) = self.item(held.item_id).map(|item| item.dict_id) else {
                    continue;
                };
                match first.iter().find(|&&(id, _)| id == dict_id) {
                    Some(&(_, kept)) => {
                        if let Some(kept) = slots[kept].as_mut() {
                            kept.count = kept.count.max(held.count);
                        }
                        slots[pocket] = None;
                        changed = true;
                        emptied += 1;
                    }
                    None => first.push((dict_id, pocket)),
                }
            }
            if changed {
                let kit = &mut self.kits[index];
                kit.slots = slots;
                kit.revision = kit.revision.wrapping_add(1);
            }
        }
        emptied
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
    /// item is gone is emptied, a stack's slot takes at most what is owned
    /// (a stack down to nothing empties its slot), and each kit is held to
    /// its layout ([`Self::keep_to_layout`]). A kit whose slots changed gets
    /// a new revision. Returns whether anything changed.
    pub fn sync_kits(&mut self) -> bool {
        let dictionary = Dictionary::shared();
        // An upgrade whose cosmetic is gone (sold) is no upgrade, and the
        // kits wearing the item it was on show another one.
        let stripped = super::items_rules::drop_gone_upgrades(&mut self.inventory);
        let mut any = false;
        for index in 0..self.kits.len() {
            let mut slots = self.kits[index].slots;
            for slot in &mut slots {
                let Some(held) = slot.as_mut() else {
                    continue;
                };
                let Some(owned) = self.item(held.item_id) else {
                    *slot = None;
                    continue;
                };
                if dictionary.is_stack(owned.dict_id) {
                    held.count = held.count.min(owned.condition_or_stack);
                    if held.count == 0 {
                        *slot = None;
                    }
                }
            }
            self.keep_to_layout(&mut slots);
            let kit = &mut self.kits[index];
            let upgraded = kit
                .slots
                .iter()
                .flatten()
                .any(|slot| stripped.contains(&slot.item_id));
            if upgraded || slots != kit.slots {
                kit.slots = slots;
                kit.revision = kit.revision.wrapping_add(1);
                any = true;
            }
        }
        any
    }

    /// The saved skills after a change of `tree` ([`SkillsTree::settle`]):
    /// ranks past a skill's levels, a skill the tree dropped, the second of
    /// an exclusive pair and rows no longer reached are taken back, so their
    /// points are the account's to spend again. `true` if anything changed.
    pub fn settle_skills(&mut self, tree: &SkillsTree) -> bool {
        let settled = tree.settle(self.level(), &self.skills);
        if settled == self.skills {
            return false;
        }
        self.change_skills(settled, tree);
        true
    }

    /// The account's skills become `skills` (of `tree`). The kits carry the
    /// skills' modifiers, so each gets a new revision, and every kit and
    /// preset is held to its layout with them ([`KitLayout::kept`]): the
    /// pocket Deep Pockets or Supply Pocket opened, Additional Medicine's
    /// extra medicine, a second weapon beside Deep Pockets or Supply Pocket,
    /// a second double-handed one without Terminator. A slot names units of
    /// the inventory's stack, so nothing leaves the inventory.
    pub fn change_skills(&mut self, skills: Vec<(u16, u8)>, tree: &SkillsTree) {
        let modifiers = tree.modifiers(&skills);
        self.skills = skills;
        for index in 0..self.kits.len() {
            let held = self.kits[index].slots.map(|slot| {
                let held = slot?;
                Some((self.item(held.item_id)?.dict_id, held.count))
            });
            let worn = self.kits[index].slots.iter().flatten().map(|held| held.item_id);
            let layout = self.layout_of(worn, &modifiers);
            let kit = &mut self.kits[index];
            keep_into(&mut kit.slots, &held, &layout, |held| &mut held.count);
            kit.revision = kit.revision.wrapping_add(1);
        }
        let mut presets = self.presets;
        for kit in presets
            .iter_mut()
            .flatten()
            .flat_map(|preset| &mut preset.kits)
        {
            let held = kit.map(|slot| slot.map(|saved| (saved.dict_id, saved.count)));
            let worn = kit.iter().flatten().map(|saved| saved.item_id);
            let layout = self.layout_of(worn, &modifiers);
            keep_into(kit, &held, &layout, |saved| &mut saved.count);
        }
        self.presets = presets;
    }

    /// A kit's `slots` held to its layout: what its worn items and the
    /// account's skills open ([`KitLayout::kept`]). Every change of a kit
    /// ends in it ([`Self::sync_kits`], `rules::relocate`), so a request can
    /// not put back what the layout took off, nor fill more than it opens.
    pub fn keep_to_layout(&self, slots: &mut [Option<KitSlot>; KIT_SLOTS]) {
        let held = slots.map(|slot| {
            let held = slot?;
            Some((self.item(held.item_id)?.dict_id, held.count))
        });
        let modifiers = SkillsTree::shared().modifiers(&self.skills);
        let layout = self.layout_of(slots.iter().flatten().map(|held| held.item_id), &modifiers);
        keep_into(slots, &held, &layout, |held| &mut held.count);
    }

    /// The layout of a kit wearing the owned items of `item_ids`, with the
    /// skills' `modifiers`.
    fn layout_of(
        &self,
        item_ids: impl IntoIterator<Item = u32>,
        modifiers: &[f32; skills::MODIFIERS],
    ) -> KitLayout {
        KitLayout::new(&self.worn_modifications(item_ids), modifiers)
    }

    /// The modifications of the owned items of `item_ids`: their defaults
    /// and their own, defects too (an item's pockets count them all).
    fn worn_modifications(&self, item_ids: impl IntoIterator<Item = u32>) -> Vec<(u32, bool)> {
        item_ids
            .into_iter()
            .filter_map(|item_id| self.item(item_id))
            .flat_map(|item| {
                let own = item.modifications.iter().copied();
                loadouts::default_modifications(item.dict_id)
                    .into_iter()
                    .chain(own)
            })
            .collect()
    }

    /// Make a loaded account consistent, so that what the lobby sends from
    /// it can not crash a client: the next ids past every id in use, at
    /// most three kits with distinct non-zero ids, an active kit that
    /// exists, no duplicate item id, item lists within the client's
    /// limits, no slot holding an item that is gone or does not fit, none
    /// worn in a slot another worn item covers ([`Self::uncover_kits`]),
    /// no item type in two pockets ([`Self::merge_pockets`]), and skills
    /// the tree still allows ([`Self::settle_skills`], before the kits, which
    /// the skills shape). `true` if anything changed.
    ///
    /// It never deletes an item for a definition the game's data lacks:
    /// that is data made without a patch, and the load refuses it before
    /// this runs ([`super::known::Lacking`]).
    pub fn repair(&mut self) -> bool {
        let dictionary = Dictionary::shared();
        let before = self.clone();
        self.settle_skills(SkillsTree::shared());

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
            // A weapon's six logical slots go to the client packed into
            // three records (wire::item_descr); any other item's three.
            item.upgrades
                .truncate(super::items_rules::upgrade_capacity(item.dict_id));
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
        self.merge_pockets();
        self.sync_kits();
        super::presets::settle(self);
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
impl Account {
    /// Kit `kit` (an index) wearing a backpack and a vest whose
    /// modifications open all six pockets, of five units ([`KitLayout`]):
    /// the tests' room for whatever they put in the pockets.
    pub fn wear_roomy_gear(&mut self, kit: usize) {
        let dictionary = Dictionary::shared();
        // `ren_back_1` +2 pockets +2 units, `ren_torso_5` +1 and +1; 1031
        // +2 pockets, 1030 +1, 1171 +2 units.
        for (slot, name, modifications) in [
            (3, "ren_back_1", &[(1031, false), (1171, false)][..]),
            (2, "ren_torso_5", &[(1030, false)][..]),
        ] {
            let item_id = self.add_item(
                dictionary.item_named(name).unwrap().dict_id,
                FULL_CONDITION,
            );
            self.item_mut(item_id).unwrap().modifications = modifications.to_vec();
            self.kits[kit].slots[slot] = Some(KitSlot { item_id, count: 1 });
        }
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
        // No money: what a player starts with is the grant's (`tutorial`).
        assert_eq!(account.generic_money, 0);
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
        for level in [0, 1, 2, 15, 16, 99, 100] {
            assert_eq!(level_at(level_start(level)).1, level_start(level));
            assert_eq!(level_at(level_start(level)).0, level);
        }
        assert_eq!((level_start(15), level_start(16)), (33_750, 38_000));
        let mut account = Account::new("Alice");
        assert_eq!(account.add_experience(1300), [1, 2]);
        assert_eq!(account.level_progress(), (2, 1250, 2250));
        assert_eq!(account.free_skill_points(), 2);
        account.skills.push((101, 2));
        assert_eq!(account.free_skill_points(), 0);
    }

    #[test]
    fn every_kit_is_usable() {
        let mut account = Account::new("Alice");
        assert!((0..3).all(|index| account.kit_usable(index, 100)));
        // Playing the third kit: it stays.
        account.active_kit = 3;
        assert!(!account.settle_active_kit(100));
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

    /// The units each owned stack has, by item id, and what the pockets of
    /// every kit name of each, at most what is owned.
    fn units(account: &Account) -> Vec<(u32, u16, u32)> {
        for kit in &account.kits {
            for pocket in POCKET_SLOTS.map(usize::from) {
                if let Some(held) = kit.slots[pocket] {
                    let owned = account.item(held.item_id).unwrap().condition_or_stack;
                    assert!(held.count <= owned, "pocket {pocket}: {held:?} of {owned}");
                }
            }
        }
        account
            .inventory
            .iter()
            .map(|item| (item.item_id, item.dict_id, item.condition_or_stack))
            .collect()
    }

    fn in_pocket(account: &mut Account, kit: usize, pocket: usize, item_id: u32, count: u32) {
        account.kits[kit].slots[pocket] = Some(KitSlot { item_id, count });
    }

    /// Kits saved before the rule of one pocket an item type: a speed
    /// booster in three pockets (each adding its +5 % at a spawn), and
    /// grenades in two, the second holding more. The first pocket of a
    /// type keeps it with the most units any of them held; the others are
    /// emptied; the inventory is what it was (a slot only names units of
    /// the stack), so nothing is lost or made.
    #[test]
    fn a_type_in_two_pockets_is_merged_into_the_first() {
        let dictionary = Dictionary::shared();
        let speed = dictionary.item_named("speed_booster").unwrap().dict_id;
        let grenade = dictionary.item_named("rdg-m").unwrap().dict_id;
        let medkit = dictionary.item_named("medkit").unwrap().dict_id;
        let mut account = Account::new("Alice");
        account.wear_roomy_gear(0);
        account.wear_roomy_gear(1);
        let booster = account.add_item(speed, 3);
        let grenades = account.add_item(grenade, 3);
        let medkits = account.add_item(medkit, 5);
        in_pocket(&mut account, 0, 13, booster, 1);
        in_pocket(&mut account, 0, 14, grenades, 1);
        in_pocket(&mut account, 0, 15, booster, 1);
        in_pocket(&mut account, 0, 16, grenades, 2);
        in_pocket(&mut account, 0, 17, booster, 1);
        in_pocket(&mut account, 0, 18, medkits, 2);
        // The second kit keeps the rule: untouched.
        in_pocket(&mut account, 1, 13, booster, 1);
        in_pocket(&mut account, 1, 14, grenades, 2);
        let before = account.clone();
        assert_eq!(account.doubled_pockets(&account.kits[0]), [15, 16, 17]);
        assert!(account.doubled_pockets(&account.kits[1]).is_empty());

        assert_eq!(account.merge_pockets(), 3);
        let pockets: Vec<Option<KitSlot>> = account.kits[0].slots[13..=18].to_vec();
        assert_eq!(
            pockets,
            [
                Some(KitSlot {
                    item_id: booster,
                    count: 1
                }),
                Some(KitSlot {
                    item_id: grenades,
                    count: 2
                }),
                None,
                None,
                None,
                Some(KitSlot {
                    item_id: medkits,
                    count: 2
                }),
            ]
        );
        assert_eq!(account.kits[0].revision, before.kits[0].revision + 1);
        assert_eq!(account.kits[1], before.kits[1]);
        // The other slots, the inventory and the money are as they were.
        assert_eq!(account.kits[0].slots[..13], before.kits[0].slots[..13]);
        assert_eq!(account.inventory, before.inventory);
        assert_eq!(units(&account), units(&before));
        assert_eq!(account.generic_money, before.generic_money);
        // All six pockets are supported after duplicate types are merged.
        assert_eq!(account.merge_pockets(), 0);
        assert!(!account.repair());
    }

    /// Two stacks of one type (two instances of a dict id) count as one
    /// type; the first pocket keeps its own stack, at most what it owns.
    #[test]
    fn two_stacks_of_one_type_are_one_type() {
        let grenade = Dictionary::shared().item_named("rdg-m").unwrap().dict_id;
        let mut account = Account::new("Alice");
        account.wear_roomy_gear(2);
        let few = account.add_item(grenade, 1);
        let many = account.add_item(grenade, 4);
        in_pocket(&mut account, 2, 16, few, 1);
        in_pocket(&mut account, 2, 13, many, 2);
        let inventory = account.inventory.clone();
        assert_eq!(account.doubled_pockets(&account.kits[2]), [16]);
        assert!(account.repair());
        assert_eq!(
            account.kits[2].slots[13],
            Some(KitSlot {
                item_id: many,
                count: 2
            })
        );
        assert_eq!(account.kits[2].slots[16], None);
        assert_eq!(account.inventory, inventory);

        // The other way round: the kept stack owns less than the most held.
        in_pocket(&mut account, 2, 13, few, 1);
        in_pocket(&mut account, 2, 16, many, 2);
        assert!(account.repair());
        assert_eq!(
            account.kits[2].slots[13],
            Some(KitSlot {
                item_id: few,
                count: 1
            })
        );
        assert_eq!(account.kits[2].slots[16], None);
        assert_eq!(account.inventory, inventory);
        units(&account);
    }

    /// An RGD-5 takes two units of a pocket, so a pocket holds two of them
    /// at most (a pocket of 4 or 5 units; one in a smaller one): a kit saved
    /// with more keeps two, the rest stays in the storage.
    #[test]
    fn a_pocket_holds_two_rgd_5s_at_most() {
        let dictionary = Dictionary::shared();
        let grenade = dictionary.item_named("rgd-5").unwrap().dict_id;
        let smoke = dictionary.item_named("rdg-m").unwrap().dict_id;
        let mut account = Account::new("Alice");
        // Pockets of 4 units.
        for (slot, name) in [(3, "ren_back_1"), (2, "ren_torso_5")] {
            let dict_id = dictionary.item_named(name).unwrap().dict_id;
            let item_id = account.add_item(dict_id, FULL_CONDITION);
            in_pocket(&mut account, 0, slot, item_id, 1);
        }
        let grenades = account.add_item(grenade, 30);
        let smokes = account.add_item(smoke, 30);
        in_pocket(&mut account, 0, 13, grenades, 5);
        in_pocket(&mut account, 0, 14, smokes, 5);
        let inventory = account.inventory.clone();
        assert!(account.repair());
        let count = |slot: usize| account.kits[0].slots[slot].unwrap().count;
        // The smoke grenades: the pocket's four units.
        assert_eq!((count(13), count(14)), (2, 4));
        assert_eq!(account.inventory, inventory);
        assert!(!account.repair());
    }

    /// Ammo is no pocket item: one ammo in several ammo slots is the ammo
    /// window's own way.
    #[test]
    fn ammo_in_several_ammo_slots_is_left_alone() {
        let mut account = Account::new("Alice");
        let ammo = account.kits[0].slots[8].unwrap();
        account.kits[0].slots[9] = Some(KitSlot { count: 20, ..ammo });
        account.kits[0].slots[11] = Some(KitSlot { count: 10, ..ammo });
        assert!(account.doubled_pockets(&account.kits[0]).is_empty());
        assert_eq!(account.merge_pockets(), 0);
        assert!(!account.repair());
    }

    /// The medical branch of the changed tree (medical-skills.md).
    fn medical_tree() -> SkillsTree {
        use survarium::v026g::skills::Skill;
        type Row = (u16, u8, u32, u8, u8, &'static [u16]);
        #[rustfmt::skip]
        let rows: [Row; 13] = [
            (301, 3, 0, 2, 1, &[]), (302, 2, 0, 3, 1, &[]), (303, 2, 10, 2, 3, &[]),
            (311, 5, 3, 1, 2, &[]), (312, 3, 3, 4, 2, &[]), (313, 2, 3, 3, 2, &[]),
            (321, 3, 10, 4, 3, &[]), (322, 1, 20, 3, 5, &[331]), (323, 4, 15, 3, 4, &[333]),
            (331, 1, 20, 2, 5, &[322]), (332, 5, 15, 2, 4, &[]), (333, 4, 15, 4, 4, &[323]),
            (342, 2, 10, 1, 3, &[]),
        ];
        let skills = rows
            .iter()
            .map(
                |&(id, levels_amount, branch_points, column, row, antiskills)| Skill {
                    id,
                    levels_amount,
                    branch_points,
                    antiskills: antiskills.to_vec(),
                    column,
                    row,
                    levels: vec![Vec::new(); levels_amount.into()],
                    unknown_modifiers: Vec::new(),
                },
            )
            .collect();
        SkillsTree { skills }
    }

    /// Saved medical skills of the old tree (five-rank Fast Bandaging and
    /// Brothers in Arms, Medic, both tier-5 unlocks): the points taken back
    /// are the account's to spend again, and its kits are sent again.
    #[test]
    fn a_changed_tree_refunds_saved_skills() {
        let tree = medical_tree();
        let mut account = Account::new("Alice");
        account.add_experience(50_000_000);
        let level = u32::from(account.level());
        assert!(level >= 40, "{level}");
        let spent = |account: &Account| {
            account
                .skills
                .iter()
                .map(|&(_, n)| u32::from(n))
                .sum::<u32>()
        };
        account.skills = vec![
            (301, 5),
            (302, 5),
            (311, 5),
            (312, 5),
            (321, 5),
            (322, 1),
            (331, 1),
            (332, 5),
            (341, 1),
            (342, 5),
        ];
        let revisions: Vec<u32> = account.kits.iter().map(|kit| kit.revision).collect();
        assert_eq!(spent(&account), 38);
        assert!(account.settle_skills(&tree));
        assert_eq!(
            account.skills,
            [
                (301, 3),
                (302, 2),
                (311, 5),
                (312, 3),
                (321, 3),
                (331, 1),
                (332, 5),
                (342, 2)
            ]
        );
        assert_eq!(spent(&account), 24);
        assert_eq!(tree.validate(account.level(), &account.skills), Ok(()));
        assert!(
            account
                .kits
                .iter()
                .zip(&revisions)
                .all(|(kit, &before)| kit.revision != before)
        );
        // Settled once, the set stays as it is.
        assert!(!account.settle_skills(&tree));
    }

    /// Unflinching and Concentration exclude each other: a set holding both
    /// (none was ever saved; the pair is new) gives Unflinching's four
    /// points back, keeps Concentration, and sends the kits again.
    #[test]
    fn unflinching_beside_concentration_is_refunded() {
        let tree = medical_tree();
        let mut account = Account::new("Alice");
        account.add_experience(50_000_000);
        account.skills = vec![
            (301, 3),
            (302, 2),
            (311, 5),
            (312, 3),
            (313, 2),
            (303, 2),
            (321, 3),
            (342, 2),
            (323, 4),
            (333, 4),
        ];
        let revisions: Vec<u32> = account.kits.iter().map(|kit| kit.revision).collect();
        assert!(account.settle_skills(&tree));
        assert!(!account.skills.iter().any(|&(id, _)| id == 323));
        assert!(account.skills.contains(&(333, 4)));
        assert_eq!(account.skills.iter().map(|&(_, n)| u32::from(n)).sum::<u32>(), 26);
        assert_eq!(tree.validate(account.level(), &account.skills), Ok(()));
        assert!(
            account
                .kits
                .iter()
                .zip(&revisions)
                .all(|(kit, &before)| kit.revision != before)
        );
        assert!(!account.settle_skills(&tree));
    }

    /// A full build saved before Numb Legs moved from row 1 to row 3 (and
    /// Unflinching went from two ranks to four): the ids are the same and
    /// rows 1 and 2 hold Numb Legs' ten points, so nothing is taken back and
    /// the kits are not sent again.
    #[test]
    fn numb_legs_saved_in_row_one_stays_after_the_move() {
        let tree = medical_tree();
        let mut account = Account::new("Alice");
        account.add_experience(50_000_000);
        account.skills = vec![
            (303, 2),
            (301, 3),
            (302, 2),
            (311, 5),
            (313, 2),
            (312, 3),
            (321, 3),
            (342, 2),
            (323, 2),
            (332, 5),
            (331, 1),
        ];
        let saved = account.skills.clone();
        let revisions: Vec<u32> = account.kits.iter().map(|kit| kit.revision).collect();
        assert!(!account.settle_skills(&tree));
        assert_eq!(account.skills, saved);
        assert!(
            account
                .kits
                .iter()
                .zip(&revisions)
                .all(|(kit, &before)| kit.revision == before)
        );
    }
}
