//! Weapons: slot ids (B3 workshop list), compatible ammo; weapon modules: links weapon <-> module,
//! seeding, slot checks (docs/notes/workshop.md).

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;

use crate::items::ItemDb;

/// Modules a fresh profile receives per owned weapon.
pub const SEED_PER_WEAPON: usize = 9;
/// Installed modules a weapon record carries at most (n4 list).
pub const MAX_ATTACHED: usize = 20;
/// Modifier pairs one n4 element carries at most (`u8 m <= 4`, reader 0x2001263D0).
pub const MAX_MODULE_MODS: usize = 4;
/// Longest slot id the record accepts.
pub const MAX_SLOT_ID: usize = 32;
/// Most slots a weapon's slot set holds (weapon slots + slots added by modules; the AKMN has 9 + 9).
pub const MAX_SLOTS: usize = 64;
/// The attach-params block a module uses when no `slots_to_attach_to` entry names another one.
pub const DEFAULT_BLOCK: &str = "attach_params";

/// The keys of one attach_params block the server needs (`attach_params`, `attach_params_2`, `attach_params_3`).
#[derive(Debug, Default, Clone, Deserialize)]
pub struct AttachBlock {
    /// `slots_to_add`: (slot id, accept_module); an absent accept_module is true (weapon cook default, 0x20038658F)
    #[serde(default)]
    pub add: Vec<(String, bool)>,
    /// `slots_to_disable` ids, any `clear_only`: attaching the module empties them (0x2002830B3..0x2002831BC)
    #[serde(default)]
    pub disable: Vec<String>,
    /// `slots_to_override` entries that set accept_module of an existing slot (0x2003869E0..0x200386CE9)
    #[serde(default, rename = "override")]
    pub override_accept: Vec<(String, bool)>,
}

/// `module_attach` row of game_tables_069d.json (crates/data-extract).
#[derive(Debug, Default, Clone, Deserialize)]
pub(crate) struct RawAttach {
    /// `slots_to_attach_to`: (slot id, attach_params block name)
    #[serde(default)]
    to: Vec<(String, String)>,
    /// weapon cfg name -> `exceptions[<weapon cfg>].slots_to_attach_to`
    #[serde(default)]
    ex: HashMap<String, Vec<(String, String)>>,
    #[serde(default)]
    blocks: HashMap<String, AttachBlock>,
}

#[derive(Debug, Default)]
struct ModuleAttach {
    to: Vec<(String, String)>,
    /// weapon dict id -> its exception list (replaces `to` on that weapon)
    ex: HashMap<u16, Vec<(String, String)>>,
    blocks: HashMap<String, AttachBlock>,
}

/// One slot of a weapon's current slot set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotInfo {
    pub id: String,
    /// accept_module after `slots_to_override`
    pub accept: bool,
    /// the slot whose module added this slot; None = a slot of the weapon itself
    pub parent: Option<String>,
}

/// The slots a weapon has right now: `weapons.<cfg>.slots` plus, recursively, the `slots_to_add` of every module in it
/// (installed or default), the way the client's weapon cook builds the workshop's slot list (0x2003862D0, 0x200286220).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SlotSet {
    pub slots: Vec<SlotInfo>,
}

impl SlotSet {
    pub fn get(&self, id: &str) -> Option<&SlotInfo> {
        self.slots.iter().find(|s| s.id == id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    /// The slot exists and takes modules.
    pub fn accepts(&self, id: &str) -> bool {
        self.get(id).is_some_and(|s| s.accept)
    }

    /// Does `target` exist only because of the module in `slot` (added by it, directly or through other added slots)?
    /// The client's check before it empties a `slots_to_disable` slot (0x200282C60).
    pub fn hangs_from(&self, target: &str, slot: &str) -> bool {
        let mut cur = self.get(target).and_then(|s| s.parent.as_deref());
        for _ in 0..self.slots.len() {
            let Some(p) = cur else { return false };
            if p == slot {
                return true;
            }
            cur = self.get(p).and_then(|s| s.parent.as_deref());
        }
        false
    }
}

#[derive(Debug, Default)]
pub struct ModuleData {
    /// weapon dict id -> slot ids in dictionary order
    slots: HashMap<u16, Vec<String>>,
    /// weapon dict id -> compatible ammo dict ids
    ammo: HashMap<u16, Vec<u16>>,
    /// weapon dict id -> linked module dict ids in file order (`module_to_item_links`, only known modules)
    links: HashMap<u16, Vec<u16>>,
    /// module dict id -> weapons it fits (the client's `module_possible_owners`)
    owners: BTreeMap<u16, Vec<u16>>,
    /// links of a weapon cut to the first SEED_PER_WEAPON entries before the module lookup
    seed: HashMap<u16, Vec<u16>>,
    /// weapon dict id -> its default modules (slot id, module dict id) in file order
    defaults: HashMap<u16, Vec<(String, u16)>>,
    /// weapon dict id -> its slot ids with accept_module true (absent weapon: every slot accepts)
    accepting: HashMap<u16, Vec<String>>,
    /// module dict id -> where it attaches and its attach_params blocks
    attach: HashMap<u16, ModuleAttach>,
    /// module dict id -> `detach_price_in_gold` (missing = 0)
    detach_price: HashMap<u16, u32>,
    /// module dict id -> (`premium_modifiers`, `override_modifiers`); every `modules_additional_infos` row
    fixed: HashMap<u16, (Vec<(u16, u16)>, Vec<(u16, u16)>)>,
}

impl ModuleData {
    pub(crate) fn build(
        items: &ItemDb,
        weapon_slots: &HashMap<String, Vec<Option<String>>>,
        compat: &HashMap<String, Vec<u32>>,
        links: &[(String, String)],
    ) -> ModuleData {
        let mut m = ModuleData::default();
        for (cfg, ids) in weapon_slots {
            if let Some(did) = items.by_cfg_name(cfg) {
                m.slots.insert(did, ids.iter().flatten().cloned().collect());
            }
        }
        for (cfg, ids) in compat {
            if let Some(did) = items.by_cfg_name(cfg) {
                m.ammo.insert(did, ids.iter().map(|&i| i as u16).collect());
            }
        }
        // module cfg name -> dict id, modules only (a later row of the same name wins)
        let mut by_cfg: HashMap<&str, u16> = HashMap::new();
        for it in items.iter() {
            if items.is_module(it.dict_id) {
                by_cfg.insert(it.cfg_name.as_str(), it.dict_id);
            }
        }
        let mut per_weapon: HashMap<&str, Vec<&str>> = HashMap::new();
        for (w, module) in links {
            per_weapon.entry(w.as_str()).or_default().push(module.as_str());
        }
        for (w, mods) in &per_weapon {
            let Some(wd) = items.by_cfg_name(w) else { continue };
            let all: Vec<u16> = mods.iter().filter_map(|c| by_cfg.get(c).copied()).collect();
            let seeded: Vec<u16> = mods.iter().take(SEED_PER_WEAPON).filter_map(|c| by_cfg.get(c).copied()).collect();
            for &md in &all {
                m.owners.entry(md).or_default().push(wd);
            }
            m.links.insert(wd, all);
            m.seed.insert(wd, seeded);
        }
        for v in m.owners.values_mut() {
            v.sort_unstable();
            v.dedup();
        }
        m
    }

    /// Default modules per weapon (`weapons.*.default_modules`): every dictionary row whose cfg name has a list (skins
    /// have their own lists), modules resolved to dictionary modules; an entry naming a module the dictionary lacks is
    /// dropped (the weapon cook looks the dict up with no NULL check, 0x200382D4B).
    pub(crate) fn set_defaults(&mut self, items: &ItemDb, by_weapon: &HashMap<String, Vec<(String, String)>>) {
        let mut by_cfg: HashMap<&str, u16> = HashMap::new();
        for it in items.iter() {
            if items.is_module(it.dict_id) {
                by_cfg.insert(it.cfg_name.as_str(), it.dict_id);
            }
        }
        for it in items.iter() {
            let Some(list) = by_weapon.get(&it.cfg_name) else { continue };
            let v: Vec<(String, u16)> = list
                .iter()
                .filter(|(slot, _)| slot.len() < MAX_SLOT_ID)
                .filter_map(|(slot, m)| by_cfg.get(m.as_str()).map(|&d| (slot.clone(), d)))
                .collect();
            if !v.is_empty() {
                self.defaults.insert(it.dict_id, v);
            }
        }
    }

    /// The weapon's default modules (slot id, module dict id). A weapon record must carry them as n4 elements with uid
    /// 0xFFFFFFFF (what the client's own `fill_default_modules` builds, 0x20031F0A8): the client never adds them to a
    /// server record, and the 3D weapon draws one model per n4 element (0x200382D40), so without them a weapon like the
    /// AKMN shows no forend, pistol grip, magazine or muzzle.
    pub fn default_modules(&self, weapon: u16) -> &[(String, u16)] {
        self.defaults.get(&weapon).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Attach data of the modules (`module_attach`), the weapon slots that accept modules and the detach prices. Every
    /// dictionary row of a module cfg gets its row; an exception keyed by a weapon cfg applies to every row of that cfg.
    pub(crate) fn set_attach(
        &mut self,
        items: &ItemDb,
        raw: &HashMap<String, RawAttach>,
        accepting: &HashMap<String, Vec<String>>,
        detach: &[(u16, u32)],
    ) {
        let mut rows_of: HashMap<&str, Vec<u16>> = HashMap::new();
        for it in items.iter() {
            rows_of.entry(it.cfg_name.as_str()).or_default().push(it.dict_id);
        }
        for it in items.iter() {
            if !items.is_module(it.dict_id) {
                continue;
            }
            let Some(r) = raw.get(&it.cfg_name) else { continue };
            let mut ex = HashMap::new();
            for (w, l) in &r.ex {
                for &wd in rows_of.get(w.as_str()).map(|v| v.as_slice()).unwrap_or(&[]) {
                    ex.insert(wd, l.clone());
                }
            }
            self.attach.insert(it.dict_id, ModuleAttach { to: r.to.clone(), ex, blocks: r.blocks.clone() });
        }
        for (cfg, ids) in accepting {
            if let Some(did) = items.by_cfg_name(cfg) {
                self.accepting.insert(did, ids.clone());
            }
        }
        self.detach_price = detach.iter().copied().collect();
    }

    /// `slots_to_attach_to` of a module on a weapon, (slot id, block name): `exceptions[<weapon cfg>]` first.
    fn attach_list(&self, module: u16, weapon: u16) -> &[(String, String)] {
        match self.attach.get(&module) {
            Some(a) => a.ex.get(&weapon).unwrap_or(&a.to).as_slice(),
            None => &[],
        }
    }

    /// Slot ids the module attaches to on this weapon (`exceptions[<weapon cfg>]` first).
    pub fn attach_slots(&self, module: u16, weapon: u16) -> impl Iterator<Item = &str> {
        self.attach_list(module, weapon).iter().map(|(s, _)| s.as_str())
    }

    /// The module may sit in `slot` of this weapon by its `slots_to_attach_to`.
    pub fn can_attach_to(&self, module: u16, weapon: u16, slot: &str) -> bool {
        self.attach_slots(module, weapon).any(|s| s == slot)
    }

    /// The attach_params block the module uses in `slot`: the one its `slots_to_attach_to` entry for the slot names,
    /// else `attach_params` (0x200332850..0x2003329ED). None when the module has no such block.
    pub fn block(&self, module: u16, weapon: u16, slot: &str) -> Option<&AttachBlock> {
        let a = self.attach.get(&module)?;
        let name = self.attach_list(module, weapon).iter().find(|(s, _)| s == slot).map(|(_, b)| b.as_str()).unwrap_or(DEFAULT_BLOCK);
        a.blocks.get(name)
    }

    /// The client's candidates for a weapon: its default modules and its `module_to_item_links` (0x2003B1EA0).
    pub fn is_candidate(&self, module: u16, weapon: u16) -> bool {
        self.default_modules(weapon).iter().any(|(_, m)| *m == module) || self.linked_modules(weapon).contains(&module)
    }

    /// The weapon's default module for a slot.
    pub fn default_in(&self, weapon: u16, slot: &str) -> Option<u16> {
        self.default_modules(weapon).iter().find(|(s, _)| s == slot).map(|(_, m)| *m)
    }

    /// `detach_price_in_gold` of a module (`modules_drop_options.modules_additional_infos`; 0 when it has no row): the
    /// price of CHANGE_MODULE_BOUND (60 01 action 7) and of a child DETACH that keeps the module.
    pub fn detach_price(&self, module: u16) -> u32 {
        self.detach_price.get(&module).copied().unwrap_or(0)
    }

    /// accept_module of a slot of the weapon itself (true when the weapon has no accept list).
    pub fn weapon_slot_accepts(&self, weapon: u16, slot: &str) -> bool {
        self.accepting.get(&weapon).is_none_or(|v| v.iter().any(|s| s == slot))
    }

    /// The weapon's current slot set. `occupant(slot)` = dict id of the module in that slot (installed, or the default
    /// still in its slot), None when empty. Breadth first from the weapon's own slots: every occupied slot adds the
    /// `slots_to_add` of its module's block for that slot; then the `slots_to_override` of the occupants set
    /// accept_module of existing slots (a missing target is skipped, as the client logs and skips it).
    pub fn slot_set(&self, weapon: u16, occupant: impl Fn(&str) -> Option<u16>) -> SlotSet {
        let mut set = SlotSet::default();
        for id in self.weapon_slot_ids(weapon) {
            set.slots.push(SlotInfo { id: id.clone(), accept: self.weapon_slot_accepts(weapon, id), parent: None });
        }
        let mut i = 0;
        while i < set.slots.len() {
            let id = set.slots[i].id.clone();
            if let Some(b) = occupant(&id).and_then(|m| self.block(m, weapon, &id)) {
                for (a, acc) in &b.add {
                    if set.slots.len() < MAX_SLOTS && !set.contains(a) {
                        set.slots.push(SlotInfo { id: a.clone(), accept: *acc, parent: Some(id.clone()) });
                    }
                }
            }
            i += 1;
        }
        for i in 0..set.slots.len() {
            let id = set.slots[i].id.clone();
            let Some(b) = occupant(&id).and_then(|m| self.block(m, weapon, &id)) else { continue };
            for (o, acc) in &b.override_accept {
                if let Some(s) = set.slots.iter_mut().find(|s| s.id == *o) {
                    s.accept = *acc;
                }
            }
        }
        set
    }

    /// The order to write a weapon's n4 elements in: a mount before the modules in the slots it adds (the slot set's
    /// breadth-first order; unknown slots last, stable otherwise). Every module's `slots_to_override` replaces a slot
    /// such as `aim` and the module applied last wins: a collimator written before its dovetail mount (both override
    /// `aim`) made the match aim through the mount. The client's own workshop appends modules as they are attached, so
    /// its records are parent-first too. `entries` = (slot, module dict) per element; returns indices into it.
    pub fn record_order(&self, weapon: u16, entries: &[(&str, u16)]) -> Vec<usize> {
        let set = self.slot_set(weapon, |s| entries.iter().find(|(x, _)| *x == s).map(|(_, d)| *d));
        let rank = |s: &str| set.slots.iter().position(|x| x.id == s).unwrap_or(usize::MAX);
        let mut idx: Vec<usize> = (0..entries.len()).collect();
        idx.sort_by_key(|&i| rank(entries[i].0));
        idx
    }

    /// Forget every default module: only for byte comparisons with the earlier reference implementation, which
    /// sent none.
    pub fn clear_default_modules(&mut self) {
        self.defaults.clear();
    }

    /// Forget every module type's fixed modifier pairs: only for byte comparisons with the earlier
    /// reference implementation, which wrote no pairs in the n4 elements.
    pub fn clear_fixed_mods(&mut self) {
        self.fixed.clear();
    }

    /// Slot ids of a weapon (B3 must list every one or the client dereferences NULL); empty for unknown items.
    pub fn weapon_slot_ids(&self, did: u16) -> &[String] {
        self.slots.get(&did).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Workshop attach check: any slot is accepted when the weapon has no slot list, else the id must be listed.
    pub fn slot_valid(&self, weapon: u16, slot: &str) -> bool {
        let v = self.weapon_slot_ids(weapon);
        v.is_empty() || v.iter().any(|s| s == slot)
    }

    /// A slot id fits the record (u8 length, <= 32 bytes).
    pub fn slot_id_fits(slot: &str) -> bool {
        slot.len() <= MAX_SLOT_ID
    }

    pub fn compatible_ammo(&self, did: u16) -> &[u16] {
        self.ammo.get(&did).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Modules linked to a weapon (all, file order).
    pub fn linked_modules(&self, weapon: u16) -> &[u16] {
        self.links.get(&weapon).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Modules a fresh profile gets for an owned weapon: the first nine links that name a known module.
    pub fn seed_modules(&self, weapon: u16) -> &[u16] {
        self.seed.get(&weapon).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Weapons a module fits.
    pub fn possible_owners(&self, module: u16) -> &[u16] {
        self.owners.get(&module).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Does the module fit the weapon by the link table?
    pub fn fits(&self, module: u16, weapon: u16) -> bool {
        self.possible_owners(module).binary_search(&weapon).is_ok()
    }

    /// The fixed modifier pairs of the module types (`module_mods`: dict id, `premium_modifiers`, `override_modifiers`
    /// of `modules_drop_options.modules_additional_infos`).
    pub(crate) fn set_fixed_mods(&mut self, rows: &[(u16, Vec<(u16, u16)>, Vec<(u16, u16)>)]) {
        self.fixed = rows.iter().map(|(d, p, o)| (*d, (p.clone(), o.clone()))).collect();
    }

    /// `override_modifiers` of a module type: the pairs every instance of it carries (the 40 extended magazines' capacity
    /// pair (520, 7) etc.). Empty for a module without a row.
    pub fn override_mods(&self, module: u16) -> &[(u16, u16)] {
        self.fixed.get(&module).map(|f| f.1.as_slice()).unwrap_or(&[])
    }

    /// The modifier list of a module bought in the workshop (60 01 action 3 or 4): `premium_modifiers`, the rarity pair
    /// (312, 4) and `override_modifiers`, in that order. This is the list the client gives the module's n4 element when
    /// it attaches a module bought in the same save (0x2002848B5 -> 0x2002DFD30, either buy type), until the server's
    /// records replace it. None for a module type without a row (0x2002DFD30 returns false and adds nothing).
    pub fn bought_mods(&self, module: u16) -> Option<Vec<(u16, u16)>> {
        let (premium, over) = self.fixed.get(&module)?;
        let mut out = premium.clone();
        out.push((crate::merge::RARITY_GROUP, crate::merge::RARITY_GOLD));
        out.extend(over.iter().copied());
        out.truncate(MAX_MODULE_MODS);
        Some(out)
    }

    /// The modifier pairs a module instance carries in its records (its own record's n1 list and the n4 element of the
    /// weapon it sits on, which the match's item cook applies to the weapon, 0x2003356E0): the stored pairs, then the
    /// type's `override_modifiers` whose group is not among them, at most [`MAX_MODULE_MODS`]. Only for module
    /// instances; a weapon's default modules (n4 uid 0xFFFFFFFF) carry no pairs, as in the client's own fill
    /// (0x20031F0A8).
    pub fn instance_mods(&self, module: u16, stored: &[(u16, u16)]) -> Vec<(u16, u16)> {
        let mut out: Vec<(u16, u16)> = stored.iter().copied().take(MAX_MODULE_MODS).collect();
        for &(g, r) in self.override_mods(module) {
            if out.len() < MAX_MODULE_MODS && !out.iter().any(|p| p.0 == g) {
                out.push((g, r));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::GameData;

    fn gd() -> GameData {
        crate::testing::tables()
    }

    /// dict 1991 `rpk_762x39_40_round_bakelite_mag` (premium (311, 7) (407, 7), override (520, 7) = 40 rounds): the
    /// module bought for gold.
    #[test]
    fn the_40_round_magazine_carries_its_capacity_pair() {
        crate::require_tables!();
        let gd = gd();
        assert_eq!(gd.modules.override_mods(1991), &[(520, 7)]);
        assert_eq!(gd.modules.bought_mods(1991), Some(vec![(311, 7), (407, 7), (312, 4), (520, 7)]));
        // stored without the pair (seeded, from a chest): the record still carries it; stored with it: no duplicate
        assert_eq!(gd.modules.instance_mods(1991, &[]), vec![(520, 7)]);
        assert_eq!(gd.modules.instance_mods(1991, &[(312, 1)]), vec![(312, 1), (520, 7)]);
        assert_eq!(gd.modules.instance_mods(1991, &gd.modules.bought_mods(1991).unwrap()), vec![(311, 7), (407, 7), (312, 4), (520, 7)]);
        // a module type without fixed pairs keeps its stored list; one without a row gets no bought list
        let plain = gd.items.iter().map(|i| i.dict_id).find(|d| gd.items.is_module(*d) && gd.modules.override_mods(*d).is_empty()).unwrap();
        assert_eq!(gd.modules.instance_mods(plain, &[(312, 2)]), vec![(312, 2)]);
        assert_eq!(gd.modules.bought_mods(1), None);
    }

    /// Every bought list fits the n4 element (at most 4 pairs) and every pair is a known modification.
    #[test]
    fn bought_lists_fit_the_record() {
        crate::require_tables!();
        let gd = gd();
        let mut n = 0;
        for it in gd.items.iter().filter(|i| gd.items.is_module(i.dict_id)) {
            if let Some(l) = gd.modules.bought_mods(it.dict_id) {
                assert!(l.len() <= super::MAX_MODULE_MODS, "{} {l:?}", it.dict_id);
                assert!(l.contains(&(312, 4)));
                n += 1;
            }
        }
        assert!(n >= 400, "{n} module rows");
    }
}
