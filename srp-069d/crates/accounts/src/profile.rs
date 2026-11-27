//! One player's profile.

use std::collections::{BTreeMap, BTreeSet};

use crate::Error;

/// Wallet of the 8B message that the rules touch (the other 8B fields are not kept).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Currency {
    Silver,
    Gold,
    /// spare parts (craft details)
    Parts,
    /// league tokens (8B `league`; ranked seasons pay them, the league shop takes them)
    League,
    /// event currency 1 / 2 / 3 (8B `event1..3`, item dict 951 / 1138 / 1139: docs/notes/events.md)
    Event1,
    Event2,
    Event3,
}

/// First item uid. Every item uid is >= this: the lobby movie treats the uid of an ammo or pocket stack as a
/// dictionary id first (`createBaseItem(uid)`), so a uid must never be a
/// valid dictionary id (the largest one in `items_069d.tsv` is 2204). Choice: 100001, as the real servers' big ids.
pub const UID_BASE: u32 = 100_001;

/// Module uid of an `attachments` row that marks a weapon slot whose default module the player removed
/// ([`Profile::removed_defaults`]): no item has uid 0 (uids start at [`UID_BASE`]), so no schema change is needed.
pub const DEFAULT_REMOVED: u32 = 0;

/// An inventory entry: instance id, dictionary id and the u32 of record +0xC (stack count, or the
/// original dict id of a non-stack item; see `game_data::items::ItemDb::amount_field`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemRow {
    pub uid: u32,
    pub dict_id: u16,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// inventory in list order (the 87 slot table indexes this order)
    pub items: Vec<ItemRow>,
    /// profile slot (0..=22) -> item uid
    pub slots: BTreeMap<u8, u32>,
    /// slot -> rounds loaded (ammo slots) or units placed (pockets)
    pub slot_rounds: BTreeMap<u8, u32>,
    pub silver: u32,
    pub gold: u32,
    pub parts: u32,
    /// account experience
    pub xp: u32,
    /// workshop modifiers per item uid: (group, grade) in record order
    pub mods: BTreeMap<u32, Vec<(u16, u16)>>,
    /// installed modules: weapon uid -> {slot id -> module uid}
    pub attach: BTreeMap<u32, BTreeMap<String, u32>>,
    /// default modules the player took off (workshop DETACH of uid 0xFFFFFFFF): weapon uid -> slot ids. The weapon's
    /// records leave these slots empty instead of adding the default; an ATTACH of the default (uid 0xFFFFFFFF) clears
    /// the mark. Saved as `attachments` rows with module uid [`DEFAULT_REMOVED`] (a slot holds a module or a mark).
    pub removed_defaults: BTreeMap<u32, BTreeSet<String>>,
    /// module uid -> dict id of the weapon it is bound to
    pub bound: BTreeMap<u32, u16>,
    /// active faction 1..=4
    pub faction: u8,
    /// reputation points per faction 1..=4
    pub fac_points: BTreeMap<u8, u32>,
    /// learned skills: skill id -> points (levels learned); points spent = their sum
    pub skills: BTreeMap<u16, u8>,
    /// achievement progress: achievement id -> progress
    pub ach: BTreeMap<u16, AchProgress>,
    /// daily quests (schema v7)
    pub daily: crate::quests::DailyState,
    /// treasure hunt (item search roulette, schema v8)
    pub hunt: crate::periodic::HuntState,
    /// faction battle progress per faction 1..=4 (schema v8)
    pub battle: BTreeMap<u8, crate::periodic::BattleRow>,
    /// daily login calendar (schema v8)
    pub calendar: crate::periodic::CalendarState,
    /// premium account expiry, unix seconds (0 = none); `Profile::premium_left(now)` gives the seconds still to run
    pub premium_until: i64,
    /// boosters by item dict id (schema v8)
    pub boosters: BTreeMap<u16, BoosterState>,
    /// ranked league state, league tokens and season history (schema v10)
    pub league: crate::league::LeagueState,
    /// event currencies 951 / 1138 / 1139 (schema v17, `event_wallet`)
    pub event: [u32; 3],
    /// living hero-of-the-day titles (schema v13, `hero_titles`; READ ONLY: filled by a load, never saved by it)
    pub hero: Vec<crate::hero::HeroTitle>,
    /// equipment profiles (schema v15): names, enabled / owned flags, the equipment of the profiles that are not current
    pub equip: crate::equip::Equip,
    /// weapon skins (dict ids of skin items) the account unlocked (schema v15, `skin_unlocks`)
    pub skins: BTreeSet<u16>,
    /// weapon rows that wear a skin: item uid -> the dict id of the weapon itself (schema v18, `item_skin_base`). The row's
    /// `dict_id` is the skin; bonuses and the sell price follow this real item (docs/notes/weapon-skins.md)
    pub skin_base: BTreeMap<u32, u16>,
    /// camouflage / decals attached to armour pieces: owner item uid -> entries (schema v13, `item_decals`)
    pub decals: BTreeMap<u32, Vec<DecalRow>>,
    /// one-time rewards the account already got (schema v13, `claims`): see the `CLAIM_*` names
    pub claims: BTreeSet<String>,
    /// leaver penalty state (schema v19, `leave_penalty`)
    pub penalty: LeavePenalty,
    /// the Winner's Contract (schema v20, `contracts`); the default = never bought
    pub contract: ContractState,
    /// the 87 profile version: bump it to make the client rebuild the 3D character
    pub version: u32,
    /// next free item uid (never reused)
    pub next_uid: u32,
    /// the client's tutorial-gate flags (S->C 9D `F`, schema v11): [`Profile::NICKNAME_CHOSEN`],
    /// [`Profile::FACTION_CHOSEN`], [`Profile::TUTORIAL_DONE`]; 0 = a new player (nickname screen first)
    pub onboarding: u32,
    /// character face (87 account body +0x2F6; chosen on the nickname screen or with the rename voucher)
    pub face: u8,
    /// finished online matches (schema v23, `profiles.matches`): match options record +0x2D8; below the client's
    /// `damage_protector` count (3) the player takes less toxic / radiation / shock damage (0x2003EA53D)
    pub matches: u32,
}

/// One-time claim name: the lobby tutorial's awards (`unique_awards.options` `tutorial_completed`) were paid.
pub const CLAIM_TUTORIAL: &str = "tutorial_completed";
/// One-time claim name: the tutorial match's awards (`game_world_tutorial_completed`) were paid.
pub const CLAIM_GAME_WORLD_TUTORIAL: &str = "game_world_tutorial_completed";
/// One-time claim name: the `played_first_game` event (+500 experience) was paid with a finished match.
pub const CLAIM_FIRST_GAME: &str = "played_first_game";

/// The leaver penalty of an account (docs/notes/online.md): leaves counted on UTC day `day` and the end of the queue block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LeavePenalty {
    /// days since the epoch (UTC) of the counted leaves
    pub day: i64,
    /// leaves counted on `day`
    pub count: u32,
    /// unix seconds: no matchmaking search before this
    pub blocked_until: i64,
}

/// The Winner's Contract of an account (the rules are the lobby's, `lobby-server/src/contract.rs`): when it was bought and
/// which of its days already paid their first win.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContractState {
    /// unix seconds of the purchase; 0 = never bought
    pub started: i64,
    /// bit `d` set = day `d` (0-based, 24 h each from `started`) paid its first win
    pub won_mask: u8,
}

/// One camouflage / decal entry of an item record (n3 of the client's item record, 0x2001263D0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecalRow {
    /// the entry's key in the record: `u8` of 45 01 / 02 / 04 (the client replaces an entry with the same value)
    pub slot: u8,
    /// uid of the decal / camo item in the inventory (record `id`)
    pub decal_uid: u32,
    /// dictionary id of that item (record `param`)
    pub dict: u16,
    /// the `f64` of 45 01 / 45 04 as raw bits (the client's placement value; the server never interprets it)
    pub x_bits: u64,
}

/// State of one booster of an account (docs/notes/store.md section 4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BoosterState {
    /// switched on by the player (45 0C); per-match boosters are charged for every match while it is on
    pub enabled: bool,
    /// unix seconds a time booster (24 h voucher) runs until; 0 = no time limit
    pub until: i64,
    /// 48 29 booster_autobuy
    pub autobuy: bool,
}

/// Progress of one achievement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AchProgress {
    /// the number the client shows and compares with the state conditions
    pub value: u32,
    /// the current streak (streak achievements only; `value` keeps the best)
    pub run: u32,
}

/// Result of `Profile::consume`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Consumed {
    /// units really taken (never more than the stack held)
    pub taken: u32,
    /// units left; 0 means the item was removed
    pub left: u32,
}

/// Slots whose `slot_rounds` count units or rounds of a stack: the ammo cells and the pockets.
const COUNTED_SLOTS: [u8; 12] = [8, 9, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20];

/// Not enough money for a charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("not enough {currency:?}: need {need}, have {have}")]
pub struct Insufficient {
    pub currency: Currency,
    pub need: u64,
    pub have: u32,
}

impl Profile {
    /// The fresh test profile: Mosin (uid 100001, dict 107), its ammo x200 (100002), scavenger torso
    /// (100003); torso, primary weapon and ammo slots; 1 000 000 silver, 10 000 gold, 10 000 spare
    /// parts; experience 2170 (level 5); faction 1; every faction at 200 000 points.
    pub fn starter() -> Profile {
        Profile {
            items: vec![
                ItemRow { uid: UID_BASE, dict_id: 107, count: 107 },
                ItemRow { uid: UID_BASE + 1, dict_id: 104, count: 200 },
                ItemRow { uid: UID_BASE + 2, dict_id: 31, count: 31 },
            ],
            slots: BTreeMap::from([(2, UID_BASE + 2), (7, UID_BASE), (8, UID_BASE + 1)]),
            slot_rounds: BTreeMap::from([(8, 100)]),
            silver: 1_000_000,
            gold: 10_000,
            parts: 10_000,
            xp: 2170,
            mods: BTreeMap::new(),
            attach: BTreeMap::new(),
            removed_defaults: BTreeMap::new(),
            bound: BTreeMap::new(),
            faction: 1,
            fac_points: (1..=4).map(|f| (f, 200_000)).collect(),
            skills: BTreeMap::new(),
            ach: BTreeMap::new(),
            daily: Default::default(),
            hunt: Default::default(),
            battle: Default::default(),
            calendar: Default::default(),
            premium_until: 0,
            boosters: BTreeMap::new(),
            league: Default::default(),
            event: [0; 3],
            hero: Vec::new(),
            equip: Default::default(),
            skins: BTreeSet::new(),
            skin_base: BTreeMap::new(),
            decals: BTreeMap::new(),
            claims: BTreeSet::new(),
            penalty: LeavePenalty::default(),
            contract: ContractState::default(),
            version: 0,
            next_uid: UID_BASE + 3,
            // the test starter is an account past onboarding (the flags of [`Profile::PAST_ONBOARDING`])
            onboarding: Profile::PAST_ONBOARDING,
            face: 0,
            matches: 0,
        }
    }

    /// 9D `F` bit 7: the nickname screen is done (selection 0x200173470: bit 7 clear -> nickname step, type 4).
    pub const NICKNAME_CHOSEN: u32 = 0x80;
    /// 9D `F` bit 6: the faction is chosen (bit 6 clear after the nickname -> choose-faction step, type 6).
    pub const FACTION_CHOSEN: u32 = 0x40;
    /// 9D `F` bit 5: tutorial complete (bit 1 or 5 -> straight to the full lobby; tested again at 0x2002CA209).
    pub const TUTORIAL_DONE: u32 = 0x20;
    /// The flags of an account past onboarding (0xE2: bits 7, 6, 5 and 1; the earlier reference implementation sent
    /// them to every account).
    pub const PAST_ONBOARDING: u32 = 0xE2;

    pub fn nickname_chosen(&self) -> bool {
        self.onboarding & Self::NICKNAME_CHOSEN != 0
    }

    /// Was the one-time reward `name` already given?
    pub fn claimed(&self, name: &str) -> bool {
        self.claims.contains(name)
    }

    /// Record the one-time reward `name`; false when it was already recorded.
    pub fn claim(&mut self, name: &str) -> bool {
        self.claims.insert(name.to_string())
    }

    /// The item that carries the decal item `decal_uid`, if any.
    pub fn decal_owner(&self, decal_uid: u32) -> Option<u32> {
        self.decals.iter().find(|(_, l)| l.iter().any(|d| d.decal_uid == decal_uid)).map(|(o, _)| *o)
    }

    pub fn item(&self, uid: u32) -> Option<&ItemRow> {
        self.items.iter().find(|i| i.uid == uid)
    }

    pub fn item_mut(&mut self, uid: u32) -> Option<&mut ItemRow> {
        self.items.iter_mut().find(|i| i.uid == uid)
    }

    /// Append an item with a fresh uid.
    pub fn add_item(&mut self, dict_id: u16, count: u32) -> u32 {
        let uid = self.next_uid.max(UID_BASE).max(self.items.iter().map(|i| i.uid + 1).max().unwrap_or(UID_BASE));
        self.next_uid = uid + 1;
        self.items.push(ItemRow { uid, dict_id, count });
        uid
    }

    /// The dict id of the item itself: a weapon wearing a skin shows the skin's dict id but stays the weapon it was.
    pub fn real_dict(&self, row: &ItemRow) -> u16 {
        self.skin_base.get(&row.uid).copied().unwrap_or(row.dict_id)
    }

    /// Remove an item and everything that points at it (slots, modifiers, installed modules, bindings).
    pub fn remove_item(&mut self, uid: u32) -> Option<ItemRow> {
        let pos = self.items.iter().position(|i| i.uid == uid)?;
        let row = self.items.remove(pos);
        let gone: Vec<u8> = self.slots.iter().filter(|(_, u)| **u == uid).map(|(s, _)| *s).collect();
        for s in gone {
            self.slots.remove(&s);
            self.slot_rounds.remove(&s);
        }
        self.forget_in_profiles(uid);
        self.mods.remove(&uid);
        self.decals.remove(&uid);
        for list in self.decals.values_mut() {
            list.retain(|d| d.decal_uid != uid);
        }
        self.decals.retain(|_, l| !l.is_empty());
        self.attach.remove(&uid);
        for sl in self.attach.values_mut() {
            sl.retain(|_, m| *m != uid);
        }
        self.attach.retain(|_, sl| !sl.is_empty());
        self.removed_defaults.remove(&uid);
        self.bound.remove(&uid);
        self.skin_base.remove(&uid);
        Some(row)
    }

    /// Shift every item uid (and every reference to it) up by `UID_BASE - 1` when any uid is below `UID_BASE`
    /// (a profile of the old numbering 1, 2, 3, ...). Same rule as the v2 -> v3 database migration; returns
    /// whether anything changed.
    pub fn lift_uids(&mut self) -> bool {
        if !self.items.iter().any(|i| i.uid < UID_BASE) && self.next_uid >= UID_BASE {
            return false;
        }
        const D: u32 = UID_BASE - 1;
        for i in &mut self.items {
            i.uid += D;
        }
        for u in self.slots.values_mut() {
            *u += D;
        }
        self.mods = std::mem::take(&mut self.mods).into_iter().map(|(u, v)| (u + D, v)).collect();
        self.attach = std::mem::take(&mut self.attach)
            .into_iter()
            .map(|(w, sl)| (w + D, sl.into_iter().map(|(k, m)| (k, m + D)).collect()))
            .collect();
        self.removed_defaults = std::mem::take(&mut self.removed_defaults).into_iter().map(|(w, s)| (w + D, s)).collect();
        self.bound = std::mem::take(&mut self.bound).into_iter().map(|(u, v)| (u + D, v)).collect();
        self.next_uid = self.next_uid.saturating_add(D).max(UID_BASE);
        true
    }

    /// Use up `amount` units of the stack `uid`, the way the client applies 90 / `83 45 03`: the count drops by
    /// `amount` (never below 0); at 0 the item is removed with everything that points at it (slots, modifiers,
    /// installed modules, bindings). While units are left, the rounds / units placed in ammo and pocket slots are
    /// capped to what the stack still holds. None when the item does not exist or `amount` is 0.
    pub fn consume(&mut self, uid: u32, amount: u32) -> Option<Consumed> {
        if amount == 0 {
            return None;
        }
        let row = self.item_mut(uid)?;
        let taken = amount.min(row.count);
        row.count -= taken;
        let left = row.count;
        if left == 0 {
            self.remove_item(uid);
            return Some(Consumed { taken, left });
        }
        for s in COUNTED_SLOTS {
            if self.slots.get(&s) == Some(&uid) {
                if let Some(r) = self.slot_rounds.get_mut(&s) {
                    *r = (*r).min(left);
                }
            }
        }
        // the other profiles keep their own rounds: a stack they share (the starter pistol ammo sits in all three) caps
        // them too
        for e in self.equip.profiles.iter_mut() {
            for s in COUNTED_SLOTS {
                if e.slots.get(&s) == Some(&uid) {
                    if let Some(r) = e.rounds.get_mut(&s) {
                        *r = (*r).min(left);
                    }
                }
            }
        }
        Some(Consumed { taken, left })
    }

    /// The player removed the weapon's default module from `slot` (see [`Profile::removed_defaults`]).
    pub fn default_removed(&self, weapon: u32, slot: &str) -> bool {
        self.removed_defaults.get(&weapon).is_some_and(|s| s.contains(slot))
    }

    /// Mark the default module of `slot` as removed: the weapon's records keep the slot empty.
    pub fn mark_default_removed(&mut self, weapon: u32, slot: &str) {
        self.removed_defaults.entry(weapon).or_default().insert(slot.to_string());
    }

    /// Clear the mark of `slot` (the default is back, or a module now sits there). Returns whether a mark was set.
    pub fn clear_default_removed(&mut self, weapon: u32, slot: &str) -> bool {
        let Some(s) = self.removed_defaults.get_mut(&weapon) else { return false };
        let had = s.remove(slot);
        if s.is_empty() {
            self.removed_defaults.remove(&weapon);
        }
        had
    }

    /// Weapon uid a module is installed on (record +4), None when free.
    pub fn owner_of(&self, module_uid: u32) -> Option<u32> {
        self.attach.iter().find(|(_, sl)| sl.values().any(|m| *m == module_uid)).map(|(w, _)| *w)
    }

    pub fn balance(&self, c: Currency) -> u32 {
        match c {
            Currency::Silver => self.silver,
            Currency::Gold => self.gold,
            Currency::Parts => self.parts,
            Currency::League => self.league.tokens,
            Currency::Event1 => self.event[0],
            Currency::Event2 => self.event[1],
            Currency::Event3 => self.event[2],
        }
    }

    fn wallet_mut(&mut self, c: Currency) -> &mut u32 {
        match c {
            Currency::Silver => &mut self.silver,
            Currency::Gold => &mut self.gold,
            Currency::Parts => &mut self.parts,
            Currency::League => &mut self.league.tokens,
            Currency::Event1 => &mut self.event[0],
            Currency::Event2 => &mut self.event[1],
            Currency::Event3 => &mut self.event[2],
        }
    }

    /// Charge `amount`; nothing changes when the balance is too small.
    pub fn spend(&mut self, c: Currency, amount: u64) -> Result<(), Insufficient> {
        let have = self.balance(c);
        if amount > have as u64 {
            return Err(Insufficient { currency: c, need: amount, have });
        }
        *self.wallet_mut(c) = have - amount as u32;
        Ok(())
    }

    /// Credit `amount` (saturates at u32::MAX, the wire width of 8B).
    pub fn credit(&mut self, c: Currency, amount: u64) {
        let w = self.wallet_mut(c);
        *w = (*w as u64 + amount).min(u32::MAX as u64) as u32;
    }

    /// Seconds of premium account left at unix time `now` (0 = no premium).
    pub fn premium_left(&self, now: i64) -> u32 {
        (self.premium_until - now).clamp(0, u32::MAX as i64) as u32
    }

    /// Extend the premium account by `seconds` (from the current expiry while it runs, else from `now`).
    pub fn extend_premium(&mut self, now: i64, seconds: i64) {
        self.premium_until = self.premium_until.max(now) + seconds;
    }

    /// Whether the time booster `dict` runs at `now`.
    pub fn booster_running(&self, dict: u16, now: i64) -> bool {
        self.boosters.get(&dict).is_some_and(|b| b.enabled && b.until > now)
    }

    /// Make the client rebuild the lobby 3D character (87 profile version).
    pub fn bump_version(&mut self) -> u32 {
        self.version = self.version.wrapping_add(1);
        self.version
    }

    pub fn add_xp(&mut self, xp: u32) {
        self.xp = self.xp.saturating_add(xp);
    }

    /// Points spent on skills.
    pub fn skill_points_spent(&self) -> u32 {
        self.skills.values().map(|&p| p as u32).sum()
    }

    /// Reputation points of a faction (0 when never set).
    pub fn points(&self, faction: u8) -> u32 {
        self.fac_points.get(&faction).copied().unwrap_or(0)
    }

    pub fn add_points(&mut self, faction: u8, points: u32) {
        let p = self.fac_points.entry(faction).or_insert(0);
        *p = p.saturating_add(points);
    }

    /// Set the active faction (1..=4).
    pub fn set_faction(&mut self, faction: u8) -> Result<(), Error> {
        if (1..=4).contains(&faction) {
            self.faction = faction;
            Ok(())
        } else {
            Err(Error::Invalid("faction must be 1..=4"))
        }
    }

    /// Drop references to items that do not exist (an old or hand-edited state); returns how many.
    pub fn prune_dangling(&mut self) -> usize {
        let have: std::collections::BTreeSet<u32> = self.items.iter().map(|i| i.uid).collect();
        let mut n = 0;
        let bad: Vec<u8> = self.slots.iter().filter(|(_, u)| !have.contains(u)).map(|(s, _)| *s).collect();
        for s in bad {
            self.slots.remove(&s);
            n += 1;
        }
        let before = self.slot_rounds.len();
        let slots = &self.slots;
        self.slot_rounds.retain(|s, _| slots.contains_key(s));
        n += before - self.slot_rounds.len();
        for (w, sl) in self.attach.iter_mut() {
            let w_ok = have.contains(w);
            let b = sl.len();
            sl.retain(|_, m| w_ok && have.contains(m));
            n += b - sl.len();
        }
        self.attach.retain(|_, sl| !sl.is_empty());
        let before = self.removed_defaults.len();
        self.removed_defaults.retain(|w, _| have.contains(w));
        n += before - self.removed_defaults.len();
        let before = self.mods.len() + self.bound.len();
        self.mods.retain(|u, _| have.contains(u));
        self.bound.retain(|u, _| have.contains(u));
        n += before - self.mods.len() - self.bound.len();
        n
    }
}
