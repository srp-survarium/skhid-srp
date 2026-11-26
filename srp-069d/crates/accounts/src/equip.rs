//! Equipment profiles (schema v15): the three loadouts of an account, their names, enabled flags and which of them
//! the account owns, plus the weapon skins it unlocked. Rules and sources: docs/notes/profiles.md,
//! docs/notes/weapon-skins.md.
//!
//! The CURRENT profile's equipment lives in `Profile::slots` / `Profile::slot_rounds`, so every rule that works on
//! "the worn items" (equip, presets, pockets, the match loadout) follows the current profile without change. The
//! other profiles keep theirs in [`EquipProfile::slots`] / [`EquipProfile::rounds`] until a switch.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};

use crate::{AccountId, Error, Profile};

/// Profiles an account can have (client `ProfileSelector.max_profile_count` = 3, lobby movie).
pub const PROFILES: usize = 3;
/// Longest profile name the server stores (the 87 carries `str128`; CHOICE).
pub const MAX_NAME: usize = 32;

/// One equipment profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquipProfile {
    /// "" = the client's default name (`st_default_profile_nameN`)
    pub name: String,
    /// the checkbox of the profile tab (`48 01`); a disabled profile fails the client's play check
    pub enabled: bool,
    /// profile 0 always; 1 and 2 once bought (`48 03`)
    pub owned: bool,
    /// equipment while the profile is NOT the current one (slot -> item uid)
    pub slots: BTreeMap<u8, u32>,
    /// rounds / units per slot while not current
    pub rounds: BTreeMap<u8, u32>,
}

impl EquipProfile {
    fn new(owned: bool) -> Self {
        EquipProfile { name: String::new(), enabled: true, owned, slots: BTreeMap::new(), rounds: BTreeMap::new() }
    }
}

/// The profile table of an account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Equip {
    /// index (0..3) of the current profile
    pub current: u8,
    pub profiles: [EquipProfile; PROFILES],
}

impl Default for Equip {
    fn default() -> Self {
        Equip { current: 0, profiles: [EquipProfile::new(true), EquipProfile::new(false), EquipProfile::new(false)] }
    }
}

impl Profile {
    /// Make profile `idx` the current one: the worn items go back to the table, the target's come out. False when
    /// `idx` is out of range (the caller checks availability).
    pub fn switch_profile(&mut self, idx: u8) -> bool {
        let i = idx as usize;
        if i >= PROFILES {
            return false;
        }
        if idx == self.equip.current {
            return true;
        }
        let cur = self.equip.current as usize;
        self.equip.profiles[cur].slots = std::mem::take(&mut self.slots);
        self.equip.profiles[cur].rounds = std::mem::take(&mut self.slot_rounds);
        self.slots = std::mem::take(&mut self.equip.profiles[i].slots);
        self.slot_rounds = std::mem::take(&mut self.equip.profiles[i].rounds);
        self.equip.current = idx;
        true
    }

    /// Equipment of profile `idx` (the live slots for the current one).
    pub fn profile_slots_mut(&mut self, idx: u8) -> (&mut BTreeMap<u8, u32>, &mut BTreeMap<u8, u32>) {
        if idx == self.equip.current {
            (&mut self.slots, &mut self.slot_rounds)
        } else {
            let e = &mut self.equip.profiles[idx as usize];
            (&mut e.slots, &mut e.rounds)
        }
    }

    /// Equipment of profile `idx` (slot -> uid), read only.
    pub fn profile_slots(&self, idx: u8) -> &BTreeMap<u8, u32> {
        if idx == self.equip.current {
            &self.slots
        } else {
            &self.equip.profiles[idx as usize].slots
        }
    }

    /// Whether profile `idx` can be used now: owned, or free while premium runs (CHOICE, docs/notes/profiles.md).
    pub fn profile_available(&self, idx: u8, now: i64) -> bool {
        let i = idx as usize;
        i < PROFILES && (self.equip.profiles[i].owned || (idx > 0 && self.premium_left(now) > 0))
    }

    /// Make sure the current profile is usable (a premium profile whose premium ended falls back to profile 0).
    /// Returns whether it switched.
    pub fn fix_current_profile(&mut self, now: i64) -> bool {
        if self.profile_available(self.equip.current, now) {
            return false;
        }
        self.switch_profile(0)
    }

    /// Profile `idx` can be the play profile: available and enabled.
    pub fn profile_playable(&self, idx: u8, now: i64) -> bool {
        self.profile_available(idx, now) && self.equip.profiles[idx as usize].enabled
    }

    /// Drop an item from the stored equipment of the other profiles (called by `remove_item`).
    pub(crate) fn forget_in_profiles(&mut self, uid: u32) {
        for e in self.equip.profiles.iter_mut() {
            let gone: Vec<u8> = e.slots.iter().filter(|(_, u)| **u == uid).map(|(s, _)| *s).collect();
            for s in gone {
                e.slots.remove(&s);
                e.rounds.remove(&s);
            }
        }
    }
}

pub(crate) fn load(conn: &Connection, id: AccountId, p: &mut Profile) -> Result<(), Error> {
    let cur: Option<i64> = conn.query_row("SELECT current FROM equip_state WHERE account_id = ?1", [id], |r| r.get(0)).optional()?;
    let mut e = Equip::default();
    let mut st = conn.prepare("SELECT idx, name, enabled, owned FROM equip_profiles WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)))? {
        let (i, name, en, ow) = r?;
        if let Some(x) = usize::try_from(i).ok().and_then(|i| e.profiles.get_mut(i)) {
            x.name = name;
            x.enabled = en != 0;
            x.owned = ow != 0 || i == 0;
        }
    }
    let mut st = conn.prepare("SELECT idx, slot, uid, rounds FROM equip_profile_slots WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<i64>>(2)?, r.get::<_, Option<i64>>(3)?)))? {
        let (i, slot, uid, rounds) = r?;
        let (Some(x), Ok(slot)) = (usize::try_from(i).ok().and_then(|i| e.profiles.get_mut(i)), u8::try_from(slot)) else { continue };
        if let Some(u) = uid {
            x.slots.insert(slot, u32::try_from(u).map_err(|_| Error::Range("profile slot uid"))?);
        }
        if let Some(r) = rounds {
            x.rounds.insert(slot, u32::try_from(r).map_err(|_| Error::Range("profile slot rounds"))?);
        }
    }
    e.current = cur.and_then(|c| u8::try_from(c).ok()).filter(|c| (*c as usize) < PROFILES).unwrap_or(0);
    p.equip = e;
    let mut st = conn.prepare("SELECT skin_dict FROM skin_unlocks WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| r.get::<_, i64>(0))? {
        p.skins.insert(u16::try_from(r?).map_err(|_| Error::Range("skin dict id"))?);
    }
    let mut st = conn.prepare("SELECT uid, base_dict FROM item_skin_base WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
        let (uid, d) = r?;
        p.skin_base.insert(u32::try_from(uid).map_err(|_| Error::Range("skin uid"))?, u16::try_from(d).map_err(|_| Error::Range("skin base dict"))?);
    }
    Ok(())
}

pub(crate) fn save(tx: &rusqlite::Transaction<'_>, id: AccountId, p: &Profile) -> Result<(), Error> {
    let e = &p.equip;
    if *e != Equip::default() {
        tx.execute("INSERT INTO equip_state (account_id, current) VALUES (?1, ?2)", params![id, e.current])?;
        let mut st = tx.prepare("INSERT INTO equip_profiles (account_id, idx, name, enabled, owned) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        let mut sl = tx.prepare("INSERT INTO equip_profile_slots (account_id, idx, slot, uid, rounds) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        for (i, x) in e.profiles.iter().enumerate() {
            st.execute(params![id, i as i64, x.name, x.enabled, x.owned])?;
            if i as u8 == e.current {
                continue;
            }
            let all: std::collections::BTreeSet<u8> = x.slots.keys().chain(x.rounds.keys()).copied().collect();
            for s in all {
                sl.execute(params![id, i as i64, s, x.slots.get(&s), x.rounds.get(&s)])?;
            }
        }
    }
    let mut st = tx.prepare("INSERT INTO skin_unlocks (account_id, skin_dict) VALUES (?1, ?2)")?;
    for s in &p.skins {
        st.execute(params![id, s])?;
    }
    let mut st = tx.prepare("INSERT INTO item_skin_base (account_id, uid, base_dict) VALUES (?1, ?2, ?3)")?;
    for (uid, d) in p.skin_base.iter().filter(|(u, _)| p.item(**u).is_some()) {
        st.execute(params![id, uid, d])?;
    }
    Ok(())
}
