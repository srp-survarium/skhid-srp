//! Equipment presets: a named copy of which item sits in which profile slot (`58` requests, `BB` push).

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::{AccountId, Error, Store};

/// Most presets per account without premium. PROVEN: the client's `BB` handler passes 10 and 100 to the Flash
/// `set_presets` (0x200195E4A / 0x200195E59) = `maxPresetsNoPremium` / `maxPresetsWithPremium`, and
/// `PresetSelector.updateCurrMaxPresetsCount(PlayerInfo.premiumState > 0)` picks one (lobby movie).
pub const MAX_PRESETS: usize = 10;

/// Most presets per account while the premium account runs (same evidence). Presets made during premium stay after
/// it ends; the client then only refuses new ones (`newPreset.enabled = count < max`).
pub const MAX_PRESETS_PREMIUM: usize = 100;

/// Longest preset name in bytes (the `BB` entry carries a `str(128)`).
pub const MAX_NAME: usize = 127;

/// One occupied slot of a preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetSlot {
    pub slot: u8,
    pub uid: u32,
    /// rounds loaded (ammo slots) or units placed (pockets), as in `Profile::slot_rounds`
    pub rounds: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    pub id: u32,
    pub name: String,
    pub slots: Vec<PresetSlot>,
}

fn check_name(name: &str) -> Result<String, Error> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Invalid("preset name is empty"));
    }
    if name.len() > MAX_NAME {
        return Err(Error::Invalid("preset name is too long"));
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(Error::Invalid("preset name has a control character"));
    }
    Ok(name.to_string())
}

fn write_slots(tx: &rusqlite::Transaction<'_>, account: AccountId, id: u32, slots: &[PresetSlot]) -> Result<(), Error> {
    tx.execute("DELETE FROM preset_slots WHERE account_id = ?1 AND preset_id = ?2", params![account, id])?;
    let mut st = tx.prepare("INSERT OR REPLACE INTO preset_slots (account_id, preset_id, slot, uid, rounds) VALUES (?1, ?2, ?3, ?4, ?5)")?;
    for s in slots {
        st.execute(params![account, id, s.slot, s.uid, s.rounds])?;
    }
    Ok(())
}

impl Store {
    /// The account's presets in list order.
    pub fn presets(&self, account: AccountId) -> Result<Vec<Preset>, Error> {
        let conn = self.conn();
        let mut out: Vec<Preset> = Vec::new();
        let mut st = conn.prepare("SELECT id, name FROM presets WHERE account_id = ?1 ORDER BY pos")?;
        for r in st.query_map([account], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)))? {
            let (id, name) = r?;
            out.push(Preset { id, name, slots: Vec::new() });
        }
        let mut st = conn.prepare("SELECT preset_id, slot, uid, rounds FROM preset_slots WHERE account_id = ?1 ORDER BY preset_id, slot")?;
        let rows = st.query_map([account], |r| {
            Ok((r.get::<_, u32>(0)?, PresetSlot { slot: r.get(1)?, uid: r.get(2)?, rounds: r.get(3)? }))
        })?;
        for r in rows {
            let (pid, s) = r?;
            if let Some(p) = out.iter_mut().find(|p| p.id == pid) {
                p.slots.push(s);
            }
        }
        Ok(out)
    }

    /// Add a preset (name, occupied slots); returns its id. Fails at `MAX_PRESETS`, or `MAX_PRESETS_PREMIUM` while
    /// `premium` runs.
    pub fn create_preset(&self, account: AccountId, name: &str, slots: &[PresetSlot], premium: bool) -> Result<u32, Error> {
        let max = if premium { MAX_PRESETS_PREMIUM } else { MAX_PRESETS };
        let name = check_name(name)?;
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (n, max_id, max_pos): (i64, i64, i64) = tx.query_row(
            "SELECT COUNT(*), COALESCE(MAX(id), 0), COALESCE(MAX(pos), -1) FROM presets WHERE account_id = ?1",
            [account],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if n as usize >= max {
            return Err(Error::Invalid("too many presets"));
        }
        let id = u32::try_from(max_id + 1).map_err(|_| Error::Range("preset id"))?;
        tx.execute("INSERT INTO presets (account_id, id, pos, name) VALUES (?1, ?2, ?3, ?4)", params![account, id, max_pos + 1, name])?;
        write_slots(&tx, account, id, slots)?;
        tx.commit()?;
        Ok(id)
    }

    /// Replace the slots of a preset (the client's "save"); false when the preset does not exist.
    pub fn save_preset(&self, account: AccountId, id: u32, slots: &[PresetSlot]) -> Result<bool, Error> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let there: Option<i64> = tx.query_row("SELECT 1 FROM presets WHERE account_id = ?1 AND id = ?2", params![account, id], |r| r.get(0)).optional()?;
        if there.is_none() {
            return Ok(false);
        }
        write_slots(&tx, account, id, slots)?;
        tx.commit()?;
        Ok(true)
    }

    pub fn rename_preset(&self, account: AccountId, id: u32, name: &str) -> Result<bool, Error> {
        let name = check_name(name)?;
        let n = self.conn().execute("UPDATE presets SET name = ?3 WHERE account_id = ?1 AND id = ?2", params![account, id, name])?;
        Ok(n > 0)
    }

    pub fn remove_preset(&self, account: AccountId, id: u32) -> Result<bool, Error> {
        let n = self.conn().execute("DELETE FROM presets WHERE account_id = ?1 AND id = ?2", params![account, id])?;
        Ok(n > 0)
    }
}
