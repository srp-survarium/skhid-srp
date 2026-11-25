//! Importer for the account state file (`state_069d.json`) of the earlier reference implementation:
//! `{items: [[uid, dict, u32]], slots: {slot: uid}, slot_rounds: {slot: n}, money: [silver, gold, parts],
//!   xp, fac_points: {faction: points}, mods: {uid: [[group, grade]]}, faction,
//!   attach: {weapon uid: {slot id: module uid}}, bound: {module uid: dict}, skills: {skill id: points}}`.
//! Missing keys take the defaults of `Profile::starter()`, a short `money` list is padded from them.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::Deserialize;

use crate::{AccountId, Error, Profile, Store};
use crate::profile::ItemRow;

#[derive(Debug, Deserialize)]
struct StateFile {
    #[serde(default)]
    items: Option<Vec<(u32, u16, u32)>>,
    #[serde(default)]
    slots: Option<HashMap<String, u32>>,
    #[serde(default)]
    slot_rounds: Option<HashMap<String, u32>>,
    #[serde(default)]
    money: Option<Vec<u32>>,
    #[serde(default)]
    xp: Option<u32>,
    #[serde(default)]
    fac_points: HashMap<String, u32>,
    #[serde(default)]
    mods: HashMap<String, Vec<(u16, u16)>>,
    #[serde(default)]
    faction: Option<u8>,
    #[serde(default)]
    attach: HashMap<String, BTreeMap<String, u32>>,
    #[serde(default)]
    bound: HashMap<String, u16>,
    /// {skill id: points}
    #[serde(default)]
    skills: HashMap<String, u8>,
}

/// What an import did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub items: usize,
    pub slots: usize,
    /// references to missing items that were dropped
    pub dropped: usize,
}

fn num_keys<V: Clone>(m: &HashMap<String, V>, what: &str) -> Result<BTreeMap<u32, V>, Error> {
    m.iter()
        .map(|(k, v)| k.parse::<u32>().map(|n| (n, v.clone())).map_err(|_| Error::Import(format!("bad {what} key {k:?}"))))
        .collect()
}

/// Parse a state file into a profile (dangling references pruned).
pub fn profile_from_state(json: &str) -> Result<(Profile, ImportReport), Error> {
    let st: StateFile = serde_json::from_str(json).map_err(|e| Error::Import(e.to_string()))?;
    let mut p = Profile::starter();
    if let Some(items) = st.items {
        p.items = items.into_iter().map(|(uid, dict_id, count)| ItemRow { uid, dict_id, count }).collect();
        let mut seen = std::collections::BTreeSet::new();
        if let Some(d) = p.items.iter().find(|i| !seen.insert(i.uid)) {
            return Err(Error::Import(format!("duplicate item uid {}", d.uid)));
        }
    }
    if let Some(s) = st.slots {
        p.slots = num_keys(&s, "slot")?
            .into_iter()
            .map(|(k, v)| u8::try_from(k).map(|k| (k, v)).map_err(|_| Error::Import(format!("slot {k} out of range"))))
            .collect::<Result<_, _>>()?;
    }
    if let Some(s) = st.slot_rounds {
        p.slot_rounds = num_keys(&s, "slot_rounds")?
            .into_iter()
            .map(|(k, v)| u8::try_from(k).map(|k| (k, v)).map_err(|_| Error::Import(format!("slot {k} out of range"))))
            .collect::<Result<_, _>>()?;
    }
    if let Some(m) = st.money {
        // the saved values first, the defaults for the missing tail
        let d = [p.silver, p.gold, p.parts];
        let mut v = m;
        v.truncate(3);
        let n = v.len();
        v.extend_from_slice(&d[n..]);
        (p.silver, p.gold, p.parts) = (v[0], v[1], v[2]);
    }
    if let Some(xp) = st.xp {
        p.xp = xp;
    }
    if let Some(f) = st.faction {
        p.set_faction(f).map_err(|_| Error::Import(format!("faction {f} out of range")))?;
    }
    for (k, v) in num_keys(&st.fac_points, "fac_points")? {
        p.fac_points.insert(u8::try_from(k).map_err(|_| Error::Import(format!("faction {k} out of range")))?, v);
    }
    p.mods = num_keys(&st.mods, "mods")?.into_iter().filter(|(_, v)| !v.is_empty()).collect();
    p.attach = num_keys(&st.attach, "attach")?.into_iter().filter(|(_, v)| !v.is_empty()).collect();
    p.bound = num_keys(&st.bound, "bound")?;
    for (k, v) in num_keys(&st.skills, "skills")? {
        p.skills.insert(u16::try_from(k).map_err(|_| Error::Import(format!("skill {k} out of range")))?, v);
    }
    // the uid counter: past every uid in use (max + 1)
    p.next_uid = p.items.iter().map(|i| i.uid + 1).max().unwrap_or(1);
    // the file numbers items 1, 2, 3 ...; the lobby needs uids that are no dictionary ids (UID_BASE)
    p.lift_uids();
    let dropped = p.prune_dangling();
    let report = ImportReport { items: p.items.len(), slots: p.slots.len(), dropped };
    Ok((p, report))
}

impl Store {
    /// Replace the profile of an existing account with the state file text `json`.
    pub fn import_state(&self, id: AccountId, json: &str) -> Result<ImportReport, Error> {
        let (p, report) = profile_from_state(json)?;
        self.save_profile(id, &p)?;
        Ok(report)
    }

    /// Same, reading the file.
    pub fn import_state_file(&self, id: AccountId, path: &Path) -> Result<ImportReport, Error> {
        let json = std::fs::read_to_string(path).map_err(|e| Error::Import(format!("{}: {e}", path.display())))?;
        self.import_state(id, &json)
    }

    /// Create an account whose profile is the state file text.
    pub fn create_account_from_state(&self, name: &str, password: &str, json: &str) -> Result<(AccountId, ImportReport), Error> {
        let (p, report) = profile_from_state(json)?;
        let id = self.create_account_with_profile(name, password, &p)?;
        Ok((id, report))
    }
}
