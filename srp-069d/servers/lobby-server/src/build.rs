//! Builders of the S->C messages that describe an account: 87, 89, 8A, 8B, 8C, 8D, 8E and the item record. Pure
//! functions of the profile and the game tables.

use std::collections::BTreeMap;

use accounts::{ItemRow, Profile};
use game_data::pockets::QUICK_SLOTS;
use game_data::GameData;
use protocol::items::{AccountBody, ItemRecord, LevelFields, ModuleElem, Profile as WireProfile};
use protocol::s2c::{
    Account87, FactionRec, Factions8E, Inventory89, Level8C, Money, PriceRecord, Prices8A, ServicePrices8D,
};

/// 89 item limit the client shows.
pub const MAX_ITEMS: u32 = 150;
/// Unlocked seller level sent in every 8E record (LIKELY meaning; the real client accepts it).
pub const FACTION_UNLOCKED: u32 = 10;
/// Premium exchange rate (silver per gold) of 8D: 0 divides by zero in the exchange window.
pub const EXCHANGE_RATE: u32 = game_data::craft::EXCHANGE_RATE;

/// Item record of an inventory row: installed modules of a weapon (n4), modifiers, a module's owner weapon
/// and binding.
pub fn record(gd: &GameData, p: &Profile, row: &ItemRow) -> ItemRecord {
    record_value(gd, p, row, row.count)
}

/// Same with another value at +0xC (the buy reply of a stack carries the bought amount, not the total).
pub fn record_value(gd: &GameData, p: &Profile, row: &ItemRow, value: u32) -> ItemRecord {
    let module = gd.items.is_module(row.dict_id);
    let mut modules = Vec::new();
    if !module {
        if let Some(att) = p.attach.get(&row.uid) {
            for (slot, muid) in att.iter().take(20) {
                let Some(m) = p.item(*muid) else { continue };
                if slot.len() > 32 {
                    continue;
                }
                // module_modifiers / module_rarity of the installed module (n4 element, at most 4 pairs): the module
                // record's own list, which the client copies into the element on attach (0x2002848D8)
                let mods = gd.modules.instance_mods(m.dict_id, p.mods.get(muid).map(Vec::as_slice).unwrap_or(&[]));
                modules.push(ModuleElem { dict: m.dict_id, uid: *muid, f24: 0, slot: slot.clone(), mods });
            }
        }
        // the defaults of the slots with nothing installed (uid 0xFFFFFFFF, like the client's fill_default_modules),
        // except the ones the player removed in the workshop (60 01 DETACH of uid 0xFFFFFFFF)
        for (slot, md) in gd.modules.default_modules(row.dict_id) {
            if modules.len() >= game_data::modules::MAX_ATTACHED || modules.iter().any(|e| &e.slot == slot) || p.default_removed(row.uid, slot) {
                continue;
            }
            modules.push(ModuleElem { dict: *md, uid: u32::MAX, f24: 0, slot: slot.clone(), mods: vec![] });
        }
        // parents first: a mount before what sits in the slots it adds (the last `aim` override wins in the match)
        let keys: Vec<(&str, u16)> = modules.iter().map(|e| (e.slot.as_str(), e.dict)).collect();
        let order = gd.modules.record_order(row.dict_id, &keys);
        modules = order.into_iter().map(|i| modules[i].clone()).collect();
    }
    ItemRecord {
        uid: row.uid,
        attached_to: if module { p.owner_of(row.uid).unwrap_or(0) } else { 0 },
        bound: if module { p.bound.get(&row.uid).copied().unwrap_or(0) } else { 0 },
        dict: row.dict_id,
        value,
        // a module's list includes its type's override pairs (`ModuleData::instance_mods`)
        mods: if module {
            gd.modules.instance_mods(row.dict_id, p.mods.get(&row.uid).map(Vec::as_slice).unwrap_or(&[]))
        } else {
            p.mods.get(&row.uid).map(|m| m.iter().take(8).copied().collect()).unwrap_or_default()
        },
        n2: vec![],
        decals: p
            .decals
            .get(&row.uid)
            .map(|l| l.iter().take(game_data::decals::MAX_PER_ITEM).map(|d| protocol::items::Decal { id: d.decal_uid, param: d.dict, x: f64::from_bits(d.x_bits), y: d.slot }).collect())
            .unwrap_or_default(),
        modules,
    }
}

pub fn records(gd: &GameData, p: &Profile, uids: &[u32]) -> Vec<ItemRecord> {
    p.items.iter().filter(|i| uids.contains(&i.uid)).map(|i| record(gd, p, i)).collect()
}

pub fn inventory_89(gd: &GameData, p: &Profile) -> Inventory89 {
    Inventory89 { max_items: MAX_ITEMS, items: p.items.iter().map(|i| record(gd, p, i)).collect() }
}

/// Unlocked skill points: character level - 1 plus one per Prestige rank reached in the four factions
/// (`game_data::levels::prestige_skill_points`).
pub fn skill_points(gd: &GameData, p: &Profile) -> u32 {
    let l = gd.levels.level_fields(p.xp);
    u32::from(l.skill_points) + game_data::levels::prestige_skill_points((1..=4).map(|f| p.points(f)))
}

pub fn level_fields(gd: &GameData, p: &Profile) -> LevelFields {
    let l = gd.levels.level_fields(p.xp);
    LevelFields { level: l.level, points: skill_points(gd, p).min(255) as u8, exp: l.xp, prev: l.prev, next: l.next }
}

/// Default tab name of a profile (the client's English `st_default_profile_nameN` = "Profile N"; CHOICE: sent literally).
pub fn default_profile_name(idx: u8) -> String {
    format!("Profile {}", idx + 1)
}

/// The wire id of equipment profile `idx` (CHOICE: index + 1; the client takes any u32, 87 reader 0x200125680).
pub fn wire_profile_id(idx: u8) -> u32 {
    idx as u32 + 1
}

/// Equipment profile of the 87: slot table (item index in the 87 list, rounds / units), name, flags. A profile the account
/// cannot use (not bought, no premium) is sent with state byte 2: the client shows it as a slot to buy
/// (`ProfileSelector.setupProfiles`, docs/notes/profiles.md).
fn wire_profile(p: &Profile, idx: u8, now: i64) -> WireProfile {
    let available = p.profile_available(idx, now);
    let mut slots = BTreeMap::new();
    if available {
        let rounds_of = if idx == p.equip.current { &p.slot_rounds } else { &p.equip.profiles[idx as usize].rounds };
        for (&sl, &uid) in p.profile_slots(idx) {
            let Some(pos) = p.items.iter().position(|i| i.uid == uid) else { continue };
            let it = &p.items[pos];
            // second u16: rounds loaded into the ammo slots (the pre-match checker sums them) / units of a pocket
            let rounds = if matches!(sl, 8 | 9 | 11 | 12) {
                rounds_of.get(&sl).copied().unwrap_or(it.count)
            } else if QUICK_SLOTS.contains(&sl) {
                rounds_of.get(&sl).copied().unwrap_or(1).max(1)
            } else {
                0
            };
            slots.insert(sl, (pos as u16, rounds.min(0xFFFF) as u16));
        }
    }
    let e = &p.equip.profiles[idx as usize];
    WireProfile {
        kind: if available { 0 } else { 2 },
        flag: e.enabled as u8,
        id: wire_profile_id(idx),
        name: if e.name.is_empty() { default_profile_name(idx) } else { e.name.clone() },
        slots,
        mods: BTreeMap::new(),
        voice: String::new(),
    }
}

/// The profile version sent in 87 / 88 / AE: FNV-1a of the stored counter and the encoded 87 body (nickname, profiles,
/// worn item records, face). The client builds the lobby character from its disk cache (`profile.data`, 0x2001BD87F)
/// and rebuilds it from a later 87 / 88 only when the profile id or this version differs (0x200277EED..0x200277F06);
/// a bare counter repeats across sessions and accounts and would keep a stale cached build on screen. Never 0 and never
/// 0xFFFFFFFF (the client's local preview copies use that).
pub fn wire_version(gd: &GameData, p: &Profile, name: &str) -> u32 {
    use protocol::Encode;
    let bytes = account_body_all(gd, p, name).to_bytes().unwrap_or_default();
    let mut h: u32 = 0x811C_9DC5;
    for b in p.version.to_le_bytes().iter().chain(bytes.iter()) {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    match h {
        0 => 1,
        u32::MAX => u32::MAX - 1,
        v => v,
    }
}

/// The 87 body with all three profiles (shared by [`account_87`] and [`wire_version`]).
fn account_body_all(gd: &GameData, p: &Profile, name: &str) -> AccountBody {
    let now = crate::premium::unix_now();
    let profiles: Vec<WireProfile> = (0..accounts::equip::PROFILES as u8).map(|i| wire_profile(p, i, now)).collect();
    AccountBody {
        name: name.to_string(),
        profiles,
        items: body_items(p).into_iter().map(|i| record(gd, p, i)).collect(),
        // face (+0x2F6), chosen on the nickname screen or with the rename voucher
        b2f6: p.face,
        ..AccountBody::default()
    }
}

/// 87: account body with the account's three equipment profiles, the item list, the per-profile (version, bought) pairs
/// and the level fields. `current_profile` is the id of the current profile.
pub fn account_87(gd: &GameData, p: &Profile, name: &str) -> Account87 {
    // per profile: the version (a rebuild trigger of the current one: the content version, [`wire_version`]) and the byte
    // at table+0x33B: 1 = bought for good, so the client's free-profile expiry does not lock it (0x2001E9EC5)
    let version = wire_version(gd, p, name);
    let profile_state = (0..accounts::equip::PROFILES).map(|i| (version, (i > 0 && p.equip.profiles[i].owned) as u8)).collect();
    let body = account_body_all(gd, p, name);
    Account87 { body, profile_state, current_profile: wire_profile_id(p.equip.current), level: level_fields(gd, p) }
}

/// The most item rows an account body (87 / 88 / 93) may carry: the client stores them in a fixed area with no bounds
/// check, built for 9 profiles + 189 items (0x2001256E3).
pub const BODY_ITEM_CAP: usize = 189;

/// The item rows of an account body: all of them, or when the inventory is larger than [`BODY_ITEM_CAP`], every item
/// worn in one of the profiles first, then the rest in inventory order up to the cap (the 89 inventory still lists all).
fn body_items(p: &Profile) -> Vec<&accounts::ItemRow> {
    if p.items.len() <= BODY_ITEM_CAP {
        return p.items.iter().collect();
    }
    let worn: std::collections::BTreeSet<u32> =
        (0..accounts::equip::PROFILES as u8).flat_map(|i| p.profile_slots(i).values().copied().collect::<Vec<_>>()).collect();
    log::warn!("account body: {} items, only {BODY_ITEM_CAP} fit the client's area (worn first)", p.items.len());
    let (mut out, rest): (Vec<_>, Vec<_>) = p.items.iter().partition(|i| worn.contains(&i.uid));
    out.extend(rest);
    out.truncate(BODY_ITEM_CAP);
    out
}

/// The body other players see (93 player details, 88 member profile): the account with its CURRENT profile only.
pub fn account_body_current(gd: &GameData, p: &Profile, name: &str) -> AccountBody {
    let mut body = account_87(gd, p, name).body;
    let cur = p.equip.current as usize;
    let one = body.profiles.swap_remove(cur);
    body.profiles = vec![one];
    body
}

/// The body of a `93` (the result window's equipment view of a clicked player): the account's three profiles with the
/// match options' rule (srp `matches::match_profiles`): a profile the player can play (bought or premium, and enabled) or
/// the current one has state 0 and its slot table, every other one state 2 and no slots. The client shows the state-0
/// profiles only (0x2001431C0, state at `+0x29FC`) and pads the tabs to three with locked ones. The ids stay the 87's:
/// the 93 handler reads the body into a temporary copy (0x20018B5BE), not into the receiver's own profile table.
pub fn account_body_93(gd: &GameData, p: &Profile, name: &str) -> AccountBody {
    let now = crate::premium::unix_now();
    let mut body = account_body_all(gd, p, name);
    for (i, wp) in body.profiles.iter_mut().enumerate() {
        let i = i as u8;
        if !(p.profile_playable(i, now) || i == p.equip.current) {
            wp.kind = 2;
            wp.slots.clear();
        }
    }
    body
}

/// The `93` body of a server bot of a PvP match (the result window lists the bots with ids 1..; the 93 handler checks
/// no id, 0x20018B550): its kit as the match options carry it (`MatchData::kit` of the AI player's `profiles[0]`, the
/// same call as the options writer), one profile, state 0. The window pads the other two tabs with locked ones.
pub fn bot_body_93(gd: &GameData, ai: usize, name: &str) -> Option<AccountBody> {
    let prof = gd.matches.ai_players().get(ai)?.profiles.first()?;
    let kit = gd.matches.kit(&gd.items, prof);
    let mut p = Profile::starter();
    p.items = kit.items.iter().map(|&(uid, dict_id, count)| accounts::ItemRow { uid, dict_id, count }).collect();
    p.slots = kit.slots.clone();
    p.slot_rounds = kit.counts.clone();
    p.attach = kit.attach.iter().map(|(w, l)| (*w, l.iter().cloned().collect())).collect();
    p.mods.clear();
    p.decals.clear();
    p.bound.clear();
    p.removed_defaults.clear();
    p.equip = Default::default();
    let mut body = account_body_all(gd, &p, name);
    body.profiles.truncate(1);
    let first = body.profiles.first_mut()?;
    first.kind = 0;
    first.flag = 1;
    Some(body)
}

pub fn prices_8a(gd: &GameData) -> Prices8A {
    Prices8A {
        idx: 0,
        records: gd
            .shop
            .iter()
            .map(|(dict, pr)| PriceRecord {
                a: pr.a,
                b: pr.b,
                dict,
                c: pr.c.min(0xFFFF) as u16,
                d: pr.kind,
                e: pr.for_sale as u8,
            })
            .collect(),
    }
}

/// 8B: silver, gold, event 1, spare parts, event 2, event 3, league.
pub fn money_8b(p: &Profile) -> Money {
    Money { silver: p.silver, gold: p.gold, event1: p.event[0], spare_parts: p.parts, event2: p.event[1], event3: p.event[2], league: p.league.tokens }
}

/// 8C: level fields and the learned skills (`u16 id, u8 points`, id order).
pub fn level_8c(gd: &GameData, p: &Profile) -> Level8C {
    Level8C { level: level_fields(gd, p), skills: p.skills.iter().map(|(&i, &pts)| (i, pts)).collect() }
}

/// 8D: costs 2..5 zero, sell multipliers 1.0, exchange rate 250; costs 0 and 1 = skill reset price per point
/// (silver, gold: `root.set_reroll_cost`, the client multiplies them by the points of the reset).
pub fn service_prices_8d(gd: &GameData) -> ServicePrices8D {
    let mut costs = [0; 6];
    costs[0] = gd.skills.prices.reset_silver_per_point;
    costs[1] = gd.skills.prices.reset_gold_per_point;
    // slot 3 = `set_change_name_cost`: the gold rename of the rename window (rename::gold_price)
    costs[3] = crate::rename::gold_price(gd);
    // slot 4 = `set_clan_creation_cost` (8D); what 48 0D charges
    costs[4] = crate::clans::CREATION_COST_SILVER;
    ServicePrices8D { costs, sell_mult: [1.0; 4], exchange_rate: EXCHANGE_RATE, extra: None }
}

/// 8E: the four factions with their points, the active one. The active byte is 0 until the player joined a faction
/// (as in 9D): a non-zero value pre-selects that card on the faction pick, which then shows no Join button (+0x79868C,
/// reader 0x20018AD38, pick 0x2001C1C7D).
///
/// The records are indexed by faction id: record 0 is a faction-0 record with 0 points, records 1..4 are the four
/// factions. PROVEN: the client keeps the 8E records as a plain array (reader 0x20018AC70: `u8 n`, n x 12 bytes) and the
/// `91` handler (0x2002470B8) and the daily-quest `AB` reader (0x200188398) take record `f` as faction `f`; the readers
/// that look a faction up by its field (0x2001C1A82, 0x20018ADE0, 0x20011B699) skip record 0.
pub fn factions_8e(p: &Profile) -> Factions8E {
    let chosen = p.onboarding & Profile::FACTION_CHOSEN != 0;
    Factions8E {
        factions: std::iter::once(FactionRec { faction: 0, unlocked: 0, points: 0 })
            .chain((1u8..=4).map(|f| FactionRec { faction: f as u32, unlocked: FACTION_UNLOCKED, points: p.points(f) }))
            .collect(),
        active: if chosen { p.faction } else { 0 },
        battle: None,
    }
}
