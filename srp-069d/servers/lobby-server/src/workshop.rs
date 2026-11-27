//! Workshop: 60 00 -> B3, weapon modules 60 01, upgrades 45 07 / 45 08, reroll 45 05 / 45 06, module upgrade
//! 45 10 (refused), push 9B (sent after the lobby-load handshake, see `login::handshake`).
//! Rules: item-aware pools and costs of `game_data::craft`; modules per docs/notes/workshop.md.

use accounts::{Currency, Profile};
use game_data::craft::{CraftData, CRAFT_CHANCE};
use game_data::modules::SlotSet;
use game_data::GameData;
use protocol::c2s::{Inventory45 as Req45, ModuleAction};
use protocol::s2c::*;

use crate::achievements;
use crate::build;
use crate::dispatch::{update, Ctx};
use crate::peer::{Peer, RerollOffer};
use crate::shop::send_money;

/// 60 00 `u16 dict`: the workshop waits for push B3 listing every slot id of the weapon (an unlisted slot makes the
/// client dereference NULL). Weapons the table does not know get an empty list.
pub fn open(cx: &mut Ctx, peer: &mut Peer, dict: u16) {
    let slots: Vec<(String, u8)> = cx.lobby.data.modules.weapon_slot_ids(dict).iter().map(|s| (s.clone(), 0)).collect();
    log::info!("{}: 60 00 workshop dict {dict}: slots {}", peer.addr, slots.len());
    // the last list of the push is the unlocked weapon skins (docs/notes/weapon-skins.md, 0x200289C08)
    let unlocked = match peer.account {
        Some(id) => match cx.lobby.store.update_profile(id, |p| Ok::<_, ()>(crate::skins::open_list(&cx.lobby.data, p, dict))) {
            Ok(l) => l,
            Err(e) => {
                log::error!("{}: skin list of account {id}: {e:?}", peer.addr);
                vec![]
            }
        },
        None => vec![],
    };
    peer.push(&S2c::Workshop(Workshop { dict, kills: 0, matches: 0, slots, unlocked }));
}

// ---- modules (60 01) ----
// Rules: docs/notes/workshop.md section 5 (exe process_attach_module 0x200287480 and what it sends).

/// uid of a weapon's default module in 60 01 actions and n4 elements (the client's fill_default_modules, 0x20031F0A8).
const DEFAULT_UID: u32 = u32::MAX;

/// Result of a module save: items to send as records.
struct ModuleOutcome {
    changed: Vec<u32>,
    /// (uid, dict id) of the modules a DETACH with f24 = 1 destroyed: sent as records with count 0 (removal)
    destroyed: Vec<(u32, u16)>,
    installed: usize,
}

/// The weapon of one save.
#[derive(Clone, Copy)]
struct Bench<'a> {
    gd: &'a GameData,
    weapon: u32,
    /// dict id of the weapon row (a skinned weapon's skin dict: slots, defaults and exceptions are per cfg)
    wdict: u16,
}

impl Bench<'_> {
    /// Dict id of the module in a slot: the installed one, else the weapon's default unless the player removed it.
    fn occupant(&self, p: &Profile, slot: &str) -> Option<u16> {
        match p.attach.get(&self.weapon).and_then(|s| s.get(slot)) {
            Some(u) => p.item(*u).map(|i| i.dict_id),
            None if p.default_removed(self.weapon, slot) => None,
            None => self.gd.modules.default_in(self.wdict, slot),
        }
    }

    /// The weapon's slots right now (weapon slots + slots added by the modules in it, installed or default).
    fn slots(&self, p: &Profile) -> SlotSet {
        self.gd.modules.slot_set(self.wdict, |s| self.occupant(p, s))
    }

    /// Empty a slot (the client's DETACH, 0x200283610): an installed module goes to the inventory; a slot with a default
    /// module is marked "default removed" (the records then leave it empty). What hangs from the slot: `drop_orphans`.
    fn empty_slot(&self, p: &mut Profile, slot: &str, changed: &mut Vec<u32>) {
        if let Some(u) = p.attach.get_mut(&self.weapon).and_then(|s| s.remove(slot)) {
            changed.push(u);
            log::info!("60 01 module {u} off weapon {} slot {slot} -> inventory", self.weapon);
        }
        if self.gd.modules.default_in(self.wdict, slot).is_some() {
            p.mark_default_removed(self.weapon, slot);
        }
    }

    /// Modules whose slot no longer exists (the mount that added it left) go to the inventory, until nothing changes
    /// (the client detaches them depth first, 0x200283FA0).
    fn drop_orphans(&self, p: &mut Profile, changed: &mut Vec<u32>) {
        loop {
            let set = self.slots(p);
            let gone: Vec<String> =
                p.attach.get(&self.weapon).map(|s| s.keys().filter(|k| !set.contains(k)).cloned().collect()).unwrap_or_default();
            if gone.is_empty() {
                return;
            }
            for s in gone {
                if let Some(u) = p.attach.get_mut(&self.weapon).and_then(|m| m.remove(&s)) {
                    changed.push(u);
                    log::info!("60 01 module {u} lost its slot {s} (its mount left) -> inventory");
                }
            }
        }
    }
}

/// The module an action names: one bought earlier in this save (the buy's uid is the client's running counter, reused
/// by the following BIND and ATTACH, 0x200282B36), else an owned module with that uid and dict.
fn resolve(gd: &GameData, p: &Profile, bought: &[(u32, u32)], f20: u32, f28: u16) -> Option<u32> {
    if let Some((_, u)) = bought.iter().find(|(k, _)| *k == f20) {
        return (p.item(*u).map(|i| i.dict_id) == Some(f28)).then_some(*u);
    }
    p.item(f20).filter(|i| i.dict_id == f28 && gd.items.is_module(f28)).map(|i| i.uid)
}

/// Weapons, armour and modules held: the client's "inventory full" count against 89 `max_items` (0x2002875A2).
fn held_items(gd: &GameData, p: &Profile) -> u32 {
    p.items.iter().filter(|i| gd.items.is_weapon(i.dict_id) || gd.items.is_armour(i.dict_id) || gd.items.is_module(i.dict_id)).count() as u32
}

/// Actions 3 / 4: buy for gold at 8A `c` / for spare parts at 8A `a` (0x200282B00). The price is the table's: no
/// record, a price of 0 or a record that is not for sale refuses; the client's cost field is never trusted. A
/// spare-parts buy with a full inventory is refused like the client does ("Attach module failed - inventory full",
/// 0x200287B03); a gold buy is not.
///
/// The new module's modifiers: a gold buy (3) gets the client's own list (`ModuleData::bought_mods`: premium pairs,
/// rarity gold = the 0.53 "Special - gold" quality, override pairs; the 0.51 notes "modules with average
/// characteristics are available for purchase"). A spare-parts buy (4) is the workshop "Create" of the 0.60 notes:
/// "Rarity of the module will be determined randomly", rolled by `create` from the general drop table
/// (`ModuleDrops::roll`, `Source::Match`; docs/notes/workshop.md section 7). The client shows its own gold list for
/// either buy until the save's `83 45 0F` record replaces it.
fn buy(gd: &GameData, p: &mut Profile, a: &ModuleAction, bought: &mut Vec<(u32, u32)>, changed: &mut Vec<u32>, create: &mut dyn FnMut(u16) -> Option<Vec<(u16, u16)>>) -> Result<(), String> {
    let gold = a.kind == 3;
    let cur = if gold { Currency::Gold } else { Currency::Parts };
    let f28 = a.module_dict;
    let price = match gd.shop.price(f28) {
        Some(pr) if pr.for_sale => if gold { pr.c } else { pr.a },
        _ => 0,
    };
    if price == 0 || !gd.items.is_module(f28) {
        return Err(format!("dict {f28} has no price"));
    }
    if !gold && held_items(gd, p) >= crate::build::MAX_ITEMS {
        return Err(format!("inventory full ({} items)", held_items(gd, p)));
    }
    p.spend(cur, price as u64).map_err(|_| format!("price {price}, have {}", p.balance(cur)))?;
    let uid = p.add_item(f28, gd.items.amount_field(f28, 1));
    // the module's modifier list: gold = the one the client itself gives a module bought in the save (premium, rarity
    // gold, override; 0x2002DFD30); spare parts = "Create", a random roll; the records also add the type's override
    // pairs to any module
    let mods = if gold { gd.modules.bought_mods(f28) } else { create(f28) };
    if let Some(mods) = mods.clone().filter(|m| !m.is_empty()) {
        p.mods.insert(uid, mods);
    }
    // a type neither list covers (no such row in 0.69d data) is green, never grey
    crate::chests::give_rarity(gd, p, uid, &mut crate::fresh_roll());
    bought.push((a.module_uid, uid));
    changed.push(uid);
    log::info!("60 01 buy module dict {f28} for {price} {}: uid {uid}, modifiers {mods:?}", if gold { "gold" } else { "spare parts" });
    Ok(())
}

/// Action 0 ATTACH `(uid, dict, slot)` (client 0x200283040): checks, then the old module of the slot and the modules in
/// the new module's `slots_to_disable` slots go to the inventory (except the mounts the target slot hangs from).
fn attach(b: Bench, p: &mut Profile, a: &ModuleAction, real: u16, bought: &[(u32, u32)], changed: &mut Vec<u32>) -> Result<(), String> {
    let gd = b.gd;
    let (f20, f28) = (a.module_uid, a.module_dict);
    let slot = a.slot.as_deref().unwrap_or("");
    if !b.slots(p).accepts(slot) {
        return Err(format!("slot {slot:?} is not a slot of the weapon now or takes no module"));
    }
    if f20 == DEFAULT_UID {
        // the weapon's default back in its slot: whatever is installed there goes to the inventory, the mark goes
        if gd.modules.default_in(b.wdict, slot) != Some(f28) {
            return Err(format!("dict {f28} is not the default of slot {slot}"));
        }
        if let Some(u) = p.attach.get_mut(&b.weapon).and_then(|s| s.remove(slot)) {
            changed.push(u);
        }
        p.clear_default_removed(b.weapon, slot);
        b.drop_orphans(p, changed);
        log::info!("60 01 default module dict {f28} back in weapon {} slot {slot}", b.weapon);
        return Ok(());
    }
    let uid = resolve(gd, p, bought, f20, f28).ok_or_else(|| "module not owned".to_string())?;
    if let Some(w) = p.owner_of(uid)
        && w != b.weapon
    {
        return Err(format!("module {uid} sits on weapon {w}"));
    }
    // 0 = bound nowhere (as in `bind`)
    if let Some(&d) = p.bound.get(&uid)
        && d != 0
        && d != b.wdict
    {
        return Err(format!("module {uid} is bound to dict {d} (action 7 first)"));
    }
    if !gd.modules.can_attach_to(f28, b.wdict, slot) {
        return Err(format!("slot {slot} is not in the module's slots_to_attach_to"));
    }
    if !gd.modules.is_candidate(f28, b.wdict) && !gd.modules.is_candidate(f28, real) {
        return Err(format!("dict {f28} is no default or linked module of the weapon"));
    }
    let here = p.attach.get(&b.weapon).and_then(|m| m.iter().find(|(_, u)| **u == uid).map(|(s, _)| s.clone()));
    if here.as_deref() == Some(slot) {
        return Ok(());
    }
    // on this weapon in another slot: out of there first
    if let Some(old) = here {
        b.empty_slot(p, &old, changed);
        b.drop_orphans(p, changed);
    }
    // occupied slot: the old module to the inventory (a default: marked removed)
    b.empty_slot(p, slot, changed);
    // slots_to_disable (any clear_only) of the module's block for this slot: emptied unless the target slot hangs from
    // them (0x200282C60); the client sent these DETACH already, repeated for safety
    if let Some(blk) = gd.modules.block(f28, b.wdict, slot) {
        let set = b.slots(p);
        for d in &blk.disable {
            if d != slot && set.contains(d) && !set.hangs_from(slot, d) && b.occupant(p, d).is_some() {
                b.empty_slot(p, d, changed);
            }
        }
    }
    b.drop_orphans(p, changed);
    if !b.slots(p).accepts(slot) {
        return Err(format!("slot {slot} disappeared"));
    }
    p.attach.entry(b.weapon).or_default().insert(slot.to_string(), uid);
    p.clear_default_removed(b.weapon, slot);
    changed.push(uid);
    log::info!("60 01 attach module {uid} (dict {f28}) -> weapon {} slot {slot}", b.weapon);
    Ok(())
}

/// Action 1 DETACH `(uid, dict)` (no slot on the wire): the module and what hangs from the slots it added go to the
/// inventory (0x200283610, 0x200283FA0); f24 = 1 destroys the module (record count 0), f24 = 0 keeps it. uid 0xFFFFFFFF =
/// the weapon's default of that dict: its slot stays empty until an ATTACH refills it.
fn detach(b: Bench, p: &mut Profile, a: &ModuleAction, bought: &[(u32, u32)], changed: &mut Vec<u32>, destroyed: &mut Vec<(u32, u16)>) -> Result<(), String> {
    let (f20, f28) = (a.module_uid, a.module_dict);
    let slot = if f20 == DEFAULT_UID {
        let set = b.slots(p);
        b.gd.modules
            .default_modules(b.wdict)
            .iter()
            .find(|(s, m)| *m == f28 && set.contains(s) && b.occupant(p, s) == Some(f28) && !p.attach.get(&b.weapon).is_some_and(|x| x.contains_key(s)))
            .map(|(s, _)| s.clone())
            .ok_or_else(|| format!("default dict {f28} is not in place"))?
    } else {
        let uid = bought.iter().find(|(k, _)| *k == f20).map(|(_, u)| *u).unwrap_or(f20);
        p.attach
            .get(&b.weapon)
            .and_then(|m| m.iter().find(|(_, u)| **u == uid).map(|(s, _)| s.clone()))
            .ok_or_else(|| format!("module {uid} is not on this weapon"))?
    };
    // +0x2A of a DETACH that keeps the module = its detach_price_in_gold (child detaches, dialog toggle); whether 0.69d
    // charged it is OPEN (the dialog shows 0 and the totals ignore it): not charged
    if a.f24 == 0 && a.cost != 0 {
        log::info!("60 01 detach keeps module {f20} (dict {f28}): client cost {} gold not charged (OPEN)", a.cost);
    }
    let uid = p.attach.get(&b.weapon).and_then(|m| m.get(&slot)).copied();
    b.empty_slot(p, &slot, changed);
    b.drop_orphans(p, changed);
    if a.f24 == 1
        && let Some(u) = uid
        && let Some(r) = p.remove_item(u)
    {
        destroyed.push((r.uid, r.dict_id));
        changed.retain(|c| *c != u);
        log::info!("60 01 module {u} (dict {}) destroyed on detach", r.dict_id);
    }
    log::info!("60 01 detach dict {f28} from weapon {} slot {slot}", b.weapon);
    Ok(())
}

/// Actions 6 BIND and 7 CHANGE_MODULE_BOUND: both bind the module to THIS weapon (client 0x200284F00..0x200284FA0).
/// 6 is free and for a module bound nowhere (or here); its byte f2C is the action id of the buy that created the module
/// (0x200282E90), not a flag. 7 moves a module bound to another weapon for its `detach_price_in_gold` from the game data
/// (the client's cost field is not trusted; refused when gold is short); its f24 is the old binding (logged only).
fn bind(b: Bench, p: &mut Profile, a: &ModuleAction, bought: &[(u32, u32)], changed: &mut Vec<u32>) -> Result<(), String> {
    let (f20, f28) = (a.module_uid, a.module_dict);
    let uid = resolve(b.gd, p, bought, f20, f28).ok_or_else(|| "module not owned".to_string())?;
    let old = p.bound.get(&uid).copied().unwrap_or(0);
    if a.kind == 6 {
        if old != 0 && old != b.wdict {
            return Err(format!("module {uid} is bound to dict {old} (needs action 7)"));
        }
    } else if old != 0 && old != b.wdict {
        let price = b.gd.modules.detach_price(f28);
        p.spend(Currency::Gold, price as u64).map_err(|_| format!("rebind price {price} gold, have {}", p.gold))?;
        log::info!("60 01 module {uid} rebound from dict {old} (client f24 {}) for {price} gold", a.f24);
    } else {
        log::info!("60 01 action 7 on module {uid} bound to {old}: nothing to move, no charge");
    }
    p.bound.insert(uid, b.wdict);
    changed.push(uid);
    log::info!("60 01 bind module {uid} (dict {f28}) -> weapon dict {} (action {})", b.wdict, a.kind);
    Ok(())
}

/// Apply the actions of one workshop save to the weapon, in order, the state updated after every action. A refused
/// action is skipped (logged; `60 01` has no refusal reply: the client re-reads the records), the others still count.
fn apply_modules(gd: &GameData, p: &mut Profile, weapon: u32, actions: &[ModuleAction], create: &mut dyn FnMut(u16) -> Option<Vec<(u16, u16)>>) -> Option<ModuleOutcome> {
    let row = *p.item(weapon)?;
    let real = p.real_dict(&row);
    let b = Bench { gd, weapon, wdict: row.dict_id };
    let mut changed: Vec<u32> = vec![weapon];
    let mut destroyed = vec![];
    let mut bought: Vec<(u32, u32)> = vec![];
    for a in actions.iter().take(64) {
        let r = match a.kind {
            3 | 4 => buy(gd, p, a, &mut bought, &mut changed, create),
            0 => attach(b, p, a, real, &bought, &mut changed),
            1 => detach(b, p, a, &bought, &mut changed, &mut destroyed),
            6 | 7 => bind(b, p, a, &bought, &mut changed),
            k => {
                log::info!("60 01 action type {k} (slot {:?}, cost {}) ignored", a.slot, a.cost);
                Ok(())
            }
        };
        if let Err(why) = r {
            log::info!("60 01 action {} module {}/{} slot {:?} refused: {why}", a.kind, a.module_uid, a.module_dict, a.slot);
        }
    }
    if p.attach.get(&weapon).is_some_and(|s| s.is_empty()) {
        p.attach.remove(&weapon);
    }
    let installed = p.attach.get(&weapon).map(|s| s.len()).unwrap_or(0);
    Some(ModuleOutcome { changed, destroyed, installed })
}

/// 60 01 (workshop Save): apply, then `83 45 0F` (weapon + changed modules), 8B, `83 60 01 <weapon>` (closes the
/// loading overlay) and a new 87 with a bumped version (the lobby character is rebuilt only then).
pub fn modules(cx: &mut Ctx, peer: &mut Peer, weapon: u32, actions: &[ModuleAction]) {
    let gd = &cx.lobby.data;
    let lobby = &cx.lobby;
    let name = peer.name.clone();
    let out = update(cx, peer, |p| {
        let mut create = |dict: u16| lobby.roll(|r| gd.module_drops.roll(dict, game_data::module_drops::Source::Match, r));
        let o = apply_modules(gd, p, weapon, actions, &mut create)?;
        p.bump_version();
        log::info!("60 01 done: {} actions, {} installed on weapon {weapon}", actions.len(), o.installed);
        let mut records = build::records(gd, p, &o.changed);
        // destroyed modules: record count 0 = removal (0x1910FD)
        records.extend(o.destroyed.iter().map(|&(uid, dict)| protocol::items::ItemRecord { uid, dict, ..Default::default() }));
        Some((records, build::account_87(gd, p, &name)))
    });
    let Some(Some((records, acct))) = out else {
        log::info!("60 01: weapon {weapon} not owned");
        return;
    };
    peer.reply(Reply::Inventory(Inventory45::Records { x: 0, items: records }));
    send_money(cx, peer);
    peer.reply(Reply::WorkshopDone { uid: weapon });
    peer.push(&S2c::Account(acct));
}

// ---- craft / reroll (45 xx) ----

fn wallet(gold: bool) -> Currency {
    if gold {
        Currency::Gold
    } else {
        Currency::Parts
    }
}

/// 45 07 `u32 item, u8 pay` (0xFF gold, 0 spare parts). `83 45 07 <result> <item>` and for result 1 the offer (the pick
/// comes as 45 08), then 8B. Result 2 = refused (rank 3, no cost row, not enough money, no candidate): nothing charged;
/// result 0 = the roll failed (charged, item unchanged).
pub fn craft(cx: &mut Ctx, peer: &mut Peer, item: u32, pay: u8) {
    let gold = pay == 0xFF;
    let gd = &cx.lobby.data;
    let lobby = &cx.lobby;
    let res = update(cx, peer, |p| {
        let Some(row) = p.item(item).copied() else { return (2u8, vec![], 0) };
        let mods = p.mods.get(&item).cloned().unwrap_or_default();
        let quote = gd.craft.craft_quote(row.dict_id, mods.len(), gold);
        let cur = wallet(gold);
        if !quote.allowed || p.balance(cur) < quote.price {
            return (2u8, vec![], quote.price);
        }
        let offer = lobby.roll(|r| gd.craft.choices(row.dict_id, &mods, None, r));
        if offer.is_empty() {
            return (2u8, vec![], quote.price);
        }
        let _ = p.spend(cur, quote.price as u64);
        let chance = if gold { CRAFT_CHANCE.1 } else { CRAFT_CHANCE.0 };
        let ok = lobby.roll(|r| CraftData::roll_success(chance, r));
        (ok as u8, offer, quote.price)
    });
    let Some((result, offer, price)) = res else { return };
    log::info!("45 07 craft item {item} (gold {gold}, price {price}): result {result} offered {offer:?}");
    let offer = if result == 1 {
        peer.craft_offer.insert(item, offer.clone());
        offer
    } else {
        vec![]
    };
    peer.reply(Reply::Inventory(Inventory45::Craft { result, item, offer }));
    send_money(cx, peer);
}

/// 45 08 `u32 item, u32 index` into the sorted offer. `83 45 08 <item> (0,0) <new>`: the client appends the pair.
/// Without a stored offer (or for an item the profile no longer has) the answer is `83 45 07 result 2`: the client's
/// `fillCraftedItem(-2)` shows "cannot_craft_item" and returns the window to its default state (PROVEN reader
/// 0x200190E64..0x200190E9A, AS `CraftItem.fillCraftedItem`); an `83 45 08` would always change the item's list.
pub fn craft_pick(cx: &mut Ctx, peer: &mut Peer, item: u32, index: u32) {
    let Some(offer) = peer.craft_offer.remove(&item) else {
        log::info!("45 08: no offer for item {item}, refused (83 45 07 result 2)");
        peer.reply(Reply::Inventory(Inventory45::Craft { result: 2, item, offer: vec![] }));
        return;
    };
    let new = offer[(index as usize).min(offer.len() - 1)];
    let gd = &cx.lobby.data;
    let rw = cx.lobby.cfg.achievement_rewards;
    let res = update(cx, peer, |p| {
        p.item(item)?;
        let m = p.mods.entry(item).or_default();
        if m.len() >= 8 {
            return Some((vec![], p.items.len()));
        }
        m.push(new);
        // achievement 420: the upgrade took the item to its new quality (rank)
        let quality = achievements::quality(gd, p, item).unwrap_or(0);
        Some(achievements::event_in(gd, rw, p, achievements::Event::Upgraded { quality }))
    });
    let Some(Some((changes, before))) = res else {
        log::info!("45 08: item {item} not in the profile, refused (83 45 07 result 2)");
        peer.reply(Reply::Inventory(Inventory45::Craft { result: 2, item, offer: vec![] }));
        return;
    };
    log::info!("45 08 item {item} gets modifier {new:?}");
    peer.reply(Reply::Inventory(Inventory45::CraftDone { item, old: (0, 0), new }));
    achievements::push_changes(cx, peer, &changes, before);
}

/// 45 05 `u32 item, u16 group, u16 grade, u8 pay`: reroll list for one modifier. `83 45 05 <item> 3 rows`, 8B. Not
/// enough money (or an unknown item or modifier) gives the old pair three times and stores no offer, so the pick keeps
/// the modifier and charges nothing.
pub fn reroll(cx: &mut Ctx, peer: &mut Peer, item: u32, old: (u16, u16), pay: u8) {
    let gold = pay == 0xFF;
    let gd = &cx.lobby.data;
    let lobby = &cx.lobby;
    let res = update(cx, peer, |p| {
        let row = p.item(item).copied()?;
        if !p.mods.get(&item).is_some_and(|m| m.contains(&old)) {
            return None;
        }
        let price = gd.craft.reroll_price(row.dict_id, gold);
        if p.spend(wallet(gold), price as u64).is_err() {
            return None;
        }
        let mods = p.mods.get(&item).cloned().unwrap_or_default();
        Some(lobby.roll(|r| gd.craft.reroll_choices(row.dict_id, &mods, old, r)))
    });
    let offer = match res.flatten() {
        Some(offer) => {
            peer.reroll_offer.insert(item, RerollOffer { old, rows: offer.clone(), gold });
            log::info!("45 05 reroll item {item} modifier {old:?} (gold {gold}): offered {offer:?}");
            offer
        }
        None => {
            peer.reroll_offer.remove(&item);
            log::info!("45 05 reroll item {item} modifier {old:?} (gold {gold}): refused, old pair shown");
            vec![old; 3]
        }
    };
    peer.reply(Reply::Inventory(Inventory45::RerollOptions { item, options: [offer[0], offer[1], offer[2]] }));
    send_money(cx, peer);
}

/// The client's list rows of a reroll answer: the wire rows in order, minus rows of the module-rarity group 312, which
/// the list builder skips (PROVEN 0x2002477E0, loop 0x200247840..0x20024789A: no sorting, group `dict+0x3CB8` skipped).
fn shown_rows(rows: &[(u16, u16)]) -> Vec<(u16, u16)> {
    rows.iter().copied().filter(|r| r.0 != game_data::merge::RARITY_GROUP).collect()
}

/// What a 45 06 pick does: the new pair (None = keep the old one) and the gold a "premium" row costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RerollChoice {
    pub new: Option<(u16, u16)>,
    pub premium: u32,
}

/// Map the clicked row of a 45 06 to a pair. `RerollItem.setRerollOptions` marks row 0 "choose" and the other rows
/// "premium" (price `premium_cost`) when the reroll was paid with spare parts, and every row "choose" when it was paid
/// with gold (`rerollType == 1`); the list keeps the order of the `83 45 05` rows (PROVEN AS `RerollItem`
/// setRerollOptions / updateReroll / doReroll; `show_extended_reroll_variants`, the only other caller, is never called
/// by the exe). A row outside the list or a different old pair keeps the modifier.
pub fn reroll_choice(offer: &RerollOffer, old: (u16, u16), row: u32, premium_price: u32) -> RerollChoice {
    let keep = RerollChoice { new: None, premium: 0 };
    if offer.old != old {
        return keep;
    }
    let rows = shown_rows(&offer.rows);
    let Some(new) = rows.get(row as usize).copied() else { return keep };
    let premium = if !offer.gold && row >= 1 { premium_price } else { 0 };
    RerollChoice { new: Some(new), premium }
}

/// 45 06 `u32 item, u16 old group, u16 old grade, u32 row, u32 gold` (see [`reroll_choice`]). The answer
/// `83 45 06 <item> <old> <new>` replaces old by new on the client (old by old = kept, the window still unlocks:
/// reader 0x200190A9D only overwrites, then refreshes the item and the reroll data); 8B follows when gold was charged.
/// The gold of a premium row is our own price (`CraftData::reroll_premium_price`), never the client's number; without
/// enough gold the modifier is kept and nothing is charged.
pub fn reroll_pick(cx: &mut Ctx, peer: &mut Peer, item: u32, old: (u16, u16), row: u32, client_gold: u32) {
    let offered = peer.reroll_offer.remove(&item);
    let gd = &cx.lobby.data;
    let rw = cx.lobby.cfg.achievement_rewards;
    let res = update(cx, peer, |p| {
        let dict = p.item(item)?.dict_id;
        let choice = match &offered {
            Some(o) => reroll_choice(o, old, row, gd.craft.reroll_premium_price(dict)),
            None => RerollChoice { new: None, premium: 0 },
        };
        let Some(new) = choice.new else { return Some((old, 0, None, "kept")) };
        let i = p.mods.get(&item)?.iter().position(|x| *x == old)?;
        if choice.premium > 0 && p.spend(Currency::Gold, choice.premium as u64).is_err() {
            return Some((old, 0, None, "not enough gold, kept"));
        }
        p.mods.get_mut(&item)?[i] = new;
        if new == old {
            return Some((old, choice.premium, None, "same pair"));
        }
        // achievement 404: a modifier of an item of this quality was changed
        let quality = achievements::quality(gd, p, item).unwrap_or(0);
        let ach = achievements::event_in(gd, rw, p, achievements::Event::Rerolled { quality });
        Some((new, choice.premium, Some(ach), "changed"))
    });
    let (chosen, charged, ach, what) = res.flatten().unwrap_or((old, 0, None, "no such modifier, kept"));
    let stored = offered.is_some();
    log::info!(
        "45 06 item {item} modifier {old:?} row {row} (client gold {client_gold}, offer stored {stored}): {what} -> {chosen:?}, {charged} gold charged"
    );
    peer.reply(Reply::Inventory(Inventory45::RerollDone { item, old, new: chosen }));
    if charged > 0 {
        send_money(cx, peer);
    }
    if let Some((changes, before)) = ach {
        achievements::push_changes(cx, peer, &changes, before);
    }
}


/// What a module merge changed (see [`module_upgrade`]).
struct MergeOutcome {
    /// 0 = the roll failed, 1 = success
    result: u8,
    /// the module's modifiers after the merge
    mods: Vec<(u16, u16)>,
    /// (uid, dict id) of the donors that were consumed (to be removed from the client)
    removed: Vec<(u32, u16)>,
    /// records to refresh: the module and every weapon a consumed donor or the module sat on
    changed: Vec<u32>,
    installed: bool,
}

/// Check and apply one merge to the profile; None = refused (nothing charged, nothing changed).
/// Rules of `game_data::merge` (client rules from the AS panel; chance = [`merge::success_chance`], fan-wiki values).
///
/// A successful merge sets the next rarity and draws the module's drop modifier grades again from the table of that
/// rarity (`redraw` = `ModuleDrops::merged`: blue `second`, orange `pve`, legendary `fourth` = grades 10 / 10; the
/// client's panel shows exactly those bounds as `next_min_value` / `next_max_value` before the merge,
/// docs/notes/workshop.md section 7). When the rarity has no cells, only the rarity pair changes.
fn apply_merge(
    gd: &GameData,
    p: &mut Profile,
    items: [u32; 4],
    gold: bool,
    success: &mut dyn FnMut(f32) -> bool,
    redraw: &mut dyn FnMut(u16, &[(u16, u16)], u16) -> Option<Vec<(u16, u16)>>,
) -> Option<MergeOutcome> {
    use game_data::merge;
    let module = items[0];
    let donors = &items[1..];
    // four different uids, all modules of this profile
    for (i, u) in items.iter().enumerate() {
        if *u == 0 || items[..i].contains(u) || !p.item(*u).is_some_and(|r| gd.items.is_module(r.dict_id)) {
            return None;
        }
    }
    let mods = p.mods.get(&module).cloned().unwrap_or_default();
    let r = merge::rarity(&mods);
    if !merge::can_merge(r) {
        return None;
    }
    if !donors.iter().all(|d| merge::donor_fits(r, merge::rarity(p.mods.get(d).map(|m| m.as_slice()).unwrap_or(&[])))) {
        return None;
    }
    let price = if gold { merge::gold_price(&gd.craft, p.item(module)?.dict_id, r)? } else { 0 };
    p.spend(Currency::Gold, price as u64).ok()?;
    let ok = success(merge::success_chance(gold));
    let mut changed = vec![module];
    changed.extend(p.owner_of(module));
    let mut installed = p.owner_of(module).is_some();
    let mut removed = vec![];
    for d in donors {
        if let Some(w) = p.owner_of(*d) {
            changed.push(w);
            installed = true;
        }
        if let Some(row) = p.remove_item(*d) {
            removed.push((row.uid, row.dict_id));
        }
    }
    let dict = p.item(module)?.dict_id;
    let new = if ok {
        let to = merge::next_rarity(r);
        redraw(dict, &mods, to).unwrap_or_else(|| merge::with_rarity(&mods, to))
    } else {
        mods
    };
    p.mods.insert(module, new.clone());
    changed.sort_unstable();
    changed.dedup();
    Some(MergeOutcome { result: ok as u8, mods: new, removed, changed, installed })
}

/// 45 10 `u32 x4, u8 pay` (module, 3 donors; pay 1 = gold, 0 = the free button): `83 45 10 <result> <module> <list>`
/// FIRST (the client reads the module's old record while building the result view), then `83 45 0F` with the module,
/// the weapons that lost an installed module, and the donors with count 0 (removal), then 8B. Result 2 = refused.
pub fn module_upgrade(cx: &mut Ctx, peer: &mut Peer, items: [u32; 4], pay: u8) {
    let gold = pay != 0;
    let gd = &cx.lobby.data;
    let lobby = &cx.lobby;
    let name = peer.name.clone();
    let out = update(cx, peer, |p| {
        let o = apply_merge(gd, p, items, gold, &mut |chance| lobby.roll(|r| CraftData::roll_success(chance, r)), &mut |dict, mods, to| {
            lobby.roll(|r| gd.module_drops.merged(dict, mods, to, r))
        })?;
        if o.installed {
            p.bump_version();
        }
        let records = build::records(gd, p, &o.changed);
        let acct = o.installed.then(|| build::account_87(gd, p, &name));
        Some((o, records, acct))
    });
    let Some(Some((o, mut records, acct))) = out else {
        log::info!("45 10 module upgrade {items:?}: refused");
        peer.reply(Reply::Inventory(Inventory45::ModuleUpgrade { result: 2, uid: items[0], mods: vec![] }));
        return;
    };
    log::info!("45 10 module {} merged with {:?} (gold {gold}): result {}, modifiers {:?}", items[0], &items[1..], o.result, o.mods);
    // the list of the module's own record (stored pairs + its type's override pairs), as the records below carry it
    let mods = records.iter().find(|r| r.uid == items[0]).map(|r| r.mods.clone()).unwrap_or_else(|| o.mods.iter().take(4).copied().collect());
    peer.reply(Reply::Inventory(Inventory45::ModuleUpgrade { result: o.result, uid: items[0], mods }));
    for (uid, dict) in o.removed {
        records.push(protocol::items::ItemRecord { uid, dict, ..Default::default() });
    }
    peer.reply(Reply::Inventory(Inventory45::Records { x: 0, items: records }));
    send_money(cx, peer);
    if let Some(a) = acct {
        peer.push(&S2c::Account(a));
    }
}

/// Dispatch of the 45 xx subs this module owns; false when the sub belongs to someone else.
pub fn inventory(cx: &mut Ctx, peer: &mut Peer, r: &Req45) -> bool {
    match r {
        Req45::Craft { item, pay } => craft(cx, peer, *item, *pay),
        Req45::CraftPick { item, index } => craft_pick(cx, peer, *item, *index),
        Req45::Reroll { item, group, grade, pay } => reroll(cx, peer, *item, (*group, *grade), *pay),
        Req45::RerollPick { item, old_group, old_grade, row, gold } => {
            reroll_pick(cx, peer, *item, (*old_group, *old_grade), *row, *gold)
        }
        Req45::ModuleUpgrade { items, pay } => module_upgrade(cx, peer, *items, *pay),
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The clicked row indexes the rows in wire order without the group-312 rows; rows after the first are premium only
    /// for a spare-parts reroll; a wrong old pair or a row past the list keeps the modifier.
    #[test]
    fn a_reroll_row_maps_to_the_shown_list() {
        let old = (300, 4);
        let parts = RerollOffer { old, rows: vec![(330, 2), (312, 3), (410, 5)], gold: false };
        assert_eq!(reroll_choice(&parts, old, 0, 7), RerollChoice { new: Some((330, 2)), premium: 0 });
        assert_eq!(reroll_choice(&parts, old, 1, 7), RerollChoice { new: Some((410, 5)), premium: 7 });
        assert_eq!(reroll_choice(&parts, old, 2, 7), RerollChoice { new: None, premium: 0 });
        assert_eq!(reroll_choice(&parts, (301, 4), 0, 7), RerollChoice { new: None, premium: 0 });
        let gold = RerollOffer { old, rows: vec![(330, 2), (331, 1), (410, 5)], gold: true };
        assert_eq!(reroll_choice(&gold, old, 2, 7), RerollChoice { new: Some((410, 5)), premium: 0 });
    }
}
