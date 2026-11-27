//! Match options of a local match (`83 5B <sub>` payload before packing): reader 0x2001250E0, player 0x200125460,
//! profile and items 0x200125680, AI block 0x200125B40. Every constant below was seen working in the real client.

use std::collections::BTreeMap;

use game_data::matches::{AiPlayer, DEFAULT_BOT_LEVEL, MAX_PLAYERS};
use game_data::GameData;
use protocol::Writer;

use crate::{Error, Roster, RosterBot};

/// LIKELY: u16 +0x1078, only used by the online transport; any value > 0.
pub const SEND_PERIOD_MS: u16 = 100;
/// PROVEN: u8 at options+0x1083, sent by the server, divisor of every time, must not be 0.
/// The value 50 is the server's choice (0.26g used it; LIKELY the real one).
pub const TICK_MS: u32 = 50;
/// `packed_inputs` option: 0 = plain 11-byte inputs (reader 0x2003276E0); 1 = range-coded bit stream (not needed).
pub const PACKED_INPUTS: u8 = 0;
/// Storage (options item record `+0xC`, u32) of every loaded ammo stack and pocket stack in training, the shooting range
/// and online matches. PROVEN: the match spawns each life with `min(slot count, +0xC)` taken out of `+0xC`
/// (0x20034AA99), the resupply box adds `min(missing, +0xC, slot count x percent / 100)` (0x20034B527; pockets
/// `min(missing, +0xC)`) and death puts the carried units back. A stack whose count is the slot count is therefore
/// empty after the first spawn: the box shows "Add ammunition (USE)" and adds nothing. The amount used in a match is
/// not limited in 0.6x (official patch notes 0.27a: "removed the limit on how much ammunition you can use in combat";
/// the 97 tail charges the used ammo and pockets after the match, `spent_for_autobuy_money`). CHOICE: 3000 units per
/// stack, far below the u32 that death adds back to. 0 = off.
pub const AMMO_STORAGE: u32 = 3000;
/// Profile slots whose stacks are loaded ammo (8, 9 and the two extra ammo slots).
const AMMO_SLOTS: [u8; 4] = [8, 9, 11, 12];
/// The slots whose stacks get [`AMMO_STORAGE`]: the ammo slots and the pockets (quick slots 13..20: grenades, medkits,
/// traps, the resupply crate), which the match loads and refills from the same `+0xC` (without the storage a
/// consumable is usable once per match).
fn storage_slots() -> impl Iterator<Item = u8> {
    AMMO_SLOTS.into_iter().chain(game_data::pockets::QUICK_SLOTS)
}

/// `items` with the storage of every uid in `uids` raised to [`AMMO_STORAGE`] (never lowered). The PvE nut record keeps
/// its storage 0: pve.lua's pickup fills it ("pick up the nuts" objective), the spawn must not.
fn with_storage(items: &[(u32, u16, u32)], uids: &[u32]) -> Vec<(u32, u16, u32)> {
    items
        .iter()
        .map(|&(uid, dict, c)| if AMMO_STORAGE != 0 && dict != PVE_NUT_DICT && uids.contains(&uid) { (uid, dict, c.max(AMMO_STORAGE)) } else { (uid, dict, c) })
        .collect()
}

/// An online human with [`AMMO_STORAGE`] on his ammo and pocket stacks (every profile's).
fn online_human(h: &HumanLoadout) -> HumanLoadout {
    HumanLoadout { items: with_storage(&h.items, &ammo_items(h)), ..h.clone() }
}

/// One equipment profile of a human in a match (the profile part of reader 0x200125680, PROVEN): `state` 0 = the
/// profile exists in the match (any other value = absent; 2 is the reader's
/// default and the lobby's "not owned"), `enabled` (not checked by the match code read), `id`, `name`, the slot table
/// (slot -> item uid, slot -> rounds / units). The in-match profile index is the position in the list: keys 1/2/3 pick
/// index 0/1/2, the wheel cycles over the usable ones, a switch applies at the next spawn.
#[derive(Debug, Clone, PartialEq)]
pub struct HumanProfile {
    pub state: u8,
    pub enabled: bool,
    pub id: u32,
    pub name: String,
    pub slots: BTreeMap<u8, u32>,
    pub rounds: BTreeMap<u8, u32>,
}

/// Most profiles one player may have in the options: the match entry has 9 profile slots and nothing checks the count.
pub const MAX_MATCH_PROFILES: usize = 9;

/// The local human as the lobby knows him (the loadout plus the saved state used for the item upgrades and
/// modules).
#[derive(Debug, Clone, PartialEq)]
pub struct HumanLoadout {
    pub name: String,
    pub clan: String,
    pub account_id: [u8; 8],
    /// (instance id, dict id, u32 at +0xC): quick-slot stacks already carry their slot count.
    pub items: Vec<(u32, u16, u32)>,
    /// profile slot -> instance id
    pub slots: BTreeMap<u8, u32>,
    /// ammo slot -> rounds loaded (the second u16 of the slot entry, any counted slot)
    pub slot_rounds: BTreeMap<u8, u32>,
    /// learned skill points (`skill_543` -> points)
    pub skills: BTreeMap<String, u32>,
    /// workshop upgrades per item (n1): instance id -> (group, rank) list; for a module uid its stored pairs, which
    /// also go into the n4 element of the weapon it sits on (with its type's override pairs, `put_n4`)
    pub upgrades: BTreeMap<u32, Vec<(u16, u16)>>,
    /// installed modules: weapon instance id -> (slot name, module instance id) in saved order
    pub attach: Vec<(u32, Vec<(String, u32)>)>,
    /// slots whose default module the player removed in the workshop: weapon instance id -> slot names (no default n4
    /// element there)
    pub removed_defaults: BTreeMap<u32, Vec<String>>,
    /// bound field (u16 after the owner) per item
    pub bound: BTreeMap<u32, u16>,
    /// Match-wide modifiers that are not skills (the enabled per-match boosters): modifier id -> value. They are
    /// ADDED to the skill sums in the profile's modifier block (same block, same reader 0x200125680).
    pub extra_modifiers: BTreeMap<u32, f64>,
    /// camouflage / decals per item (n3 of the item record, at most 3): instance id -> (decal item uid, decal dict id,
    /// the 8 placement bytes as stored, slot). The options reader decodes items with the lobby's own 89 item decoder
    /// (0x2001263D0) and the match's item cook applies each entry's decal modification (0x200335120, PROVEN carrier;
    /// that the match draws them is LIKELY).
    pub decals: BTreeMap<u32, Vec<(u32, u16, u64, u8)>>,
    /// Matches the player has finished (record +0x2D8; 0 = "first game" for the client: its `played_first_game` event
    /// and the newbie-protection formula read it, 0x20033DB84).
    pub games_played: u32,
    /// Face (record +0x2F6, the same byte as in 87; read in the match at 0x2001B896B).
    pub face: u8,
    /// Premium account (record +0x2EF: the end screen and the premium-players HUD, 0x2001414DB / 0x2001A5610).
    pub premium: bool,
    /// Every equipment profile in match order. Empty: one profile made of `slots` / `slot_rounds` (single-profile form).
    pub profiles: Vec<HumanProfile>,
}

impl HumanLoadout {
    /// The default test human: name "test", Mosin + AK-74M kit, single-profile form.
    pub fn default_loadout() -> Self {
        HumanLoadout {
            name: "test".into(),
            clan: String::new(),
            account_id: [0; 8],
            items: vec![(1, 107, 107), (2, 104, 200), (3, 31, 100)],
            slots: BTreeMap::from([(2, 3), (7, 1), (8, 2)]),
            slot_rounds: BTreeMap::from([(8, 100)]),
            skills: BTreeMap::new(),
            upgrades: BTreeMap::new(),
            attach: vec![],
            removed_defaults: BTreeMap::new(),
            bound: BTreeMap::new(),
            extra_modifiers: BTreeMap::new(),
            decals: BTreeMap::new(),
            games_played: 0,
            face: 0,
            premium: false,
            profiles: vec![],
        }
    }
}

/// What kind of local match to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainingParams {
    /// the three u32 of the lobby's `5B 01` (menu +0xEBBD8 / +0xEBBDC / +0xEBBE0)
    pub map: u32,
    pub mode: u32,
    pub weather: u32,
    /// total players incl. the human; None = the mode's max, else min players, else 10 (at most 20)
    pub n_players: Option<usize>,
    pub bot_level: u32,
    /// pass a fixed value for reproducible bytes, or a clock-based one from [`fresh_match_id`]
    pub match_id: u32,
}

impl TrainingParams {
    pub fn new(map: u32, mode: u32, weather: u32, match_id: u32) -> Self {
        TrainingParams { map, mode, weather, n_players: None, bot_level: DEFAULT_BOT_LEVEL, match_id }
    }
}

/// A clock-based match id: seconds since the epoch, 31 bits, never 0.
pub fn fresh_match_id() -> u32 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(1);
    ((t & 0x7FFF_FFFF) as u32).max(1)
}

fn put_str(w: &mut Writer, s: &str, limit: usize) {
    w.str_max(s, limit - 1);
}

fn put_f32(w: &mut Writer, v: f64) {
    w.f32(v as f32);
}

/// One item record of a bot: `u32 id, u32 owner, u16 0, u16 dict id, u32 count/base id, 3 empty lists`, then the n4
/// list: the kit's installed modules (`modules_to_equip`) and the weapon's default modules in the other slots (empty for
/// items that are not weapons). `owner` = the weapon a kit module is installed on, else 0.
fn bot_item(w: &mut Writer, modules: &game_data::modules::ModuleData, items: &[(u32, u16, u32)], attach: &[(u32, Vec<(String, u32)>)], iid: u32, did: u16, c: u32) -> Result<(), Error> {
    let owner = attach.iter().find(|(_, sl)| sl.iter().any(|(_, m)| *m == iid)).map(|(wp, _)| *wp).unwrap_or(0);
    w.u32(iid).u32(owner).u16(0).u16(did).u32(c).zeros(3);
    put_n4(w, modules, items, attach, &BTreeMap::new(), iid, did, &[])
}

/// The n4 list of an item: its installed modules (`attach`, dict ids from `items`), then the weapon's default modules in
/// the empty slots not in `removed`; written parents first (a mount before what sits in the slots it adds).
///
/// Each installed module's element carries its modifier pairs: `stored` (the module instance's own list, by uid) plus
/// its type's `override_modifiers` (`ModuleData::instance_mods`). The match's item cook applies exactly these pairs to
/// the weapon (`item_config_cook::translate_query` 0x200335120: element +0x30 list), so a
/// module's stats (an extended magazine's capacity, a premium module's recoil / reload pairs) reach the match only from
/// here. Default modules (uid 0xFFFFFFFF) carry none, like the client's own fill (0x20031F0A8).
fn put_n4(
    w: &mut Writer,
    modules: &game_data::modules::ModuleData,
    items: &[(u32, u16, u32)],
    attach: &[(u32, Vec<(String, u32)>)],
    stored: &BTreeMap<u32, Vec<(u16, u16)>>,
    iid: u32,
    did: u16,
    removed: &[String],
) -> Result<(), Error> {
    let empty: &[(String, u32)] = &[];
    let list = attach.iter().find(|(wp, _)| *wp == iid).map(|(_, l)| l.as_slice()).unwrap_or(empty);
    let mut ents: Vec<(&str, u16, u32)> = Vec::new();
    for (slot, muid) in list.iter().take(20) {
        let Some(md) = items.iter().find(|i| i.0 == *muid).map(|i| i.1) else { continue };
        // at most 31 bytes: the reader NULs [len] of a 32-byte field, which would hit the uid's low byte (0x2001267DE)
        if slot.len() > 31 {
            continue;
        }
        ents.push((slot.as_str(), md, *muid));
    }
    for (slot, md) in modules.default_modules(did) {
        if ents.len() >= game_data::modules::MAX_ATTACHED || slot.len() > 31 || ents.iter().any(|e| e.0 == slot) || removed.iter().any(|r| r == slot) {
            continue;
        }
        ents.push((slot.as_str(), *md, u32::MAX));
    }
    let keys: Vec<(&str, u16)> = ents.iter().map(|e| (e.0, e.1)).collect();
    let mut elems = Writer::new();
    for i in modules.record_order(did, &keys) {
        let (slot, md, uid) = ents[i];
        elems.u16(md).u32(uid).u32(0).u8(slot.len() as u8).bytes(slot.as_bytes());
        let pairs = if uid == u32::MAX { vec![] } else { modules.instance_mods(md, stored.get(&uid).map(Vec::as_slice).unwrap_or(&[])) };
        elems.u8(pairs.len() as u8);
        for (g, r) in pairs {
            elems.u16(g).u16(r);
        }
    }
    w.u8(ents.len() as u8);
    w.bytes(&elems.finish()?);
    Ok(())
}

/// The human's item with upgrades (n1) and installed modules (n4). The module dict ids come from the same item list.
fn human_item(w: &mut Writer, h: &HumanLoadout, modules: &game_data::modules::ModuleData, iid: u32, did: u16, c: u32) -> Result<(), Error> {
    let mods = h.upgrades.get(&iid).map(Vec::as_slice).unwrap_or(&[]);
    let owner = h.attach.iter().find(|(_, sl)| sl.iter().any(|(_, m)| *m == iid)).map(|(wp, _)| *wp).unwrap_or(0);
    let bound = h.bound.get(&iid).copied().unwrap_or(0);
    w.u32(iid).u32(owner).u16(bound).u16(did).u32(c);
    // at most 8 upgrades: more hits the reader's assert (0x20012643F)
    let mods = &mods[..mods.len().min(8)];
    w.u8(mods.len() as u8);
    for (g, r) in mods {
        w.u16(*g).u16(*r);
    }
    // n2 (meaning unknown) stays empty; n3 = the item's decals (same 15-byte entry as 89 / 83 45 01)
    w.u8(0);
    let decals = h.decals.get(&iid).map(Vec::as_slice).unwrap_or(&[]);
    let decals = &decals[..decals.len().min(3)];
    w.u8(decals.len() as u8);
    for &(uid, dict, bits, slot) in decals {
        w.u32(uid).u16(dict).u64(bits).u8(slot);
    }
    // installed modules, then the defaults of the empty slots the player did not clear
    let removed = h.removed_defaults.get(&iid).map(Vec::as_slice).unwrap_or(&[]);
    put_n4(w, modules, &h.items, &h.attach, &h.upgrades, iid, did, removed)
}

/// 18-byte bit mask (bit i of byte i>>3 = modifier i, 138 ids), then one f32 per set bit in id order
/// (reader 0x200125680, PROVEN).
fn put_mods(w: &mut Writer, mods: &BTreeMap<u32, f64>) {
    let mut mask = [0u8; 18];
    for &i in mods.keys() {
        if let Some(b) = mask.get_mut((i >> 3) as usize) {
            *b |= 1 << (i & 7);
        }
    }
    w.bytes(&mask);
    for v in mods.values() {
        put_f32(w, *v);
    }
}

/// Everything of a player record that depends on the player kind.
struct Pl<'a> {
    team: u8,
    name: &'a str,
    clan: &'a str,
    account_id: [u8; 8],
    ai_id: u32,
    items: &'a [(u32, u16, u32)],
    slots: &'a BTreeMap<u8, u32>,
    counts: &'a BTreeMap<u8, u32>,
    voice: &'a str,
    mods: &'a BTreeMap<u32, f64>,
    human: Option<&'a HumanLoadout>,
    ai: Option<(&'a AiPlayer, u8, u8)>,
    /// default modules of weapons (drawn only from n4 elements)
    modules: &'a game_data::modules::ModuleData,
    /// a bot's installed kit modules (weapon id -> (slot, module id)); a human's come from `human`
    kit_attach: &'a [(u32, Vec<(String, u32)>)],
    /// face (record +0x2F6): the human's own, a tutorial bot's `head`, else 0
    face: u8,
    /// PvE budget of a human (+0x2D0 access keys he may spend, +0x2D4 gold budget, +0x2F8 respawn price); zeros elsewhere
    pve_budget: (u32, u32, u16),
    /// A script bot's kits, one match profile each (index order); empty = the single `slots` / `counts` profile or the
    /// human's own profiles
    bot_profiles: &'a [BotProfile],
}

/// One kit profile of a script bot: slot -> item id, slot -> count, and the profile's AI sound scheme (the kit's `voices`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BotProfile {
    pub slots: BTreeMap<u8, u32>,
    pub counts: BTreeMap<u8, u32>,
    pub voice: String,
}

/// A script bot's record with several kits: the kits' items in one list (the second kit's instance ids follow the
/// first's, as [`tutorial_human`] does for the human's four kits), the installed modules, one profile per kit.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BotRecord {
    pub items: Vec<(u32, u16, u32)>,
    pub attach: Vec<(u32, Vec<(String, u32)>)>,
    pub profiles: Vec<BotProfile>,
}

/// The record of a PvE script bot (docs/notes/pve.md 10.2): `profile` as match profile 0, `profile1` as profile 1, both
/// present (state 0: the script spawns the army squad with profile 1 itself).
pub(crate) fn pve_bot_record(gd: &GameData, b: &game_data::pve::PveBot) -> BotRecord {
    let mut r = BotRecord { items: vec![], attach: vec![], profiles: vec![] };
    for prof in [&b.profile, &b.profile1] {
        let kit = gd.matches.kit(&gd.items, prof);
        let base = r.items.len() as u32;
        r.items.extend(kit.items.iter().map(|&(iid, did, c)| (iid + base, did, c)));
        r.attach.extend(kit.attach.iter().map(|(wp, l)| (wp + base, l.iter().map(|(s, m)| (s.clone(), m + base)).collect())));
        let voice = prof.voices.as_deref().filter(|v| !v.is_empty()).unwrap_or("silent").to_string();
        r.profiles.push(BotProfile { slots: kit.slots.iter().map(|(s, iid)| (*s, iid + base)).collect(), counts: kit.counts.clone(), voice });
    }
    r
}

/// The bots of a PvE difficulty with their records (all 17, else an error: the script binds every `pve_bot_N`).
pub(crate) fn pve_bots(gd: &GameData, difficulty: u8) -> Result<Vec<(&game_data::pve::PveBot, BotRecord)>, Error> {
    let list = gd.pve.bots(difficulty);
    if list.len() != game_data::pve::BOTS {
        return Err(Error::Invalid("no PvE bots for this difficulty"));
    }
    Ok(list.iter().map(|b| (b, pve_bot_record(gd, b))).collect())
}

/// One profile as the options carry it: (state, enabled, id, name, slot -> item id, slot -> count).
pub(crate) type MatchProfile<'a> = (u8, bool, u32, &'a str, &'a BTreeMap<u8, u32>, &'a BTreeMap<u8, u32>);

/// The profiles of a player record in options order: a human's own profiles (at most [`MAX_MATCH_PROFILES`]), else one
/// profile made of `slots` / `counts` (a bot, or a human in the single-profile form).
pub(crate) fn match_profiles<'a>(human: Option<&'a HumanLoadout>, slots: &'a BTreeMap<u8, u32>, counts: &'a BTreeMap<u8, u32>) -> Vec<MatchProfile<'a>> {
    match human {
        Some(h) if !h.profiles.is_empty() => {
            h.profiles.iter().take(MAX_MATCH_PROFILES).map(|q| (q.state, q.enabled, q.id, q.name.as_str(), &q.slots, &q.rounds)).collect()
        }
        _ => vec![(0u8, true, 1u32, "p", slots, counts)],
    }
}

/// Player record, reader 0x200125460.
fn player(w: &mut Writer, p: &Pl) -> Result<(), Error> {
    w.u8(p.team);
    put_str(w, p.name, 128);
    put_str(w, p.clan, 32);
    w.bytes(&p.account_id);
    // +0x2D8 games played (0 = first game), +0x2DC (not traced), +0x2E0 ai id; +0x2F5 (unused), +0x2F6 face, +0x2EF premium
    let (games, premium) = p.human.map_or((0, false), |h| (h.games_played, h.premium));
    w.u32(games).u32(0).u32(p.ai_id);
    w.u8(0).u8(p.face).u8(u8::from(premium));
    // +0x2D0 PvE access keys the player may spend in the match, +0x2D4 gold budget, +0x2F8 respawn price (PROVEN, the
    // paid respawn 0x2003CF5A6 and the PvE end screen 0x2001AC00F); none in a local match
    w.u32(p.pve_budget.0).u32(p.pve_budget.1).u16(p.pve_budget.2);
    // profile + items (reader 0x200125680); an item with dict 0 makes the reader stop after 12 bytes: never sent
    let items: Vec<(u32, u16, u32)> = p.items.iter().filter(|i| i.1 != 0).copied().collect();
    if items.len() > 0xFF {
        return Err(Error::Limit("items of a player"));
    }
    // the profiles (state, enabled, id, name, slots, rounds), then one item list shared by all of them
    let many: Vec<(MatchProfile, &str)> = if p.bot_profiles.is_empty() {
        match_profiles(p.human, p.slots, p.counts).into_iter().map(|m| (m, p.voice)).collect()
    } else {
        p.bot_profiles.iter().enumerate().map(|(i, b)| ((0u8, true, i as u32 + 1, "p", &b.slots, &b.counts), b.voice.as_str())).collect()
    };
    w.u8(many.len() as u8).u8(items.len() as u8);
    let empty = BTreeMap::new();
    for &((state, enabled, id, name, slots, counts), voice) in &many {
        // an absent profile carries no slot table (its items need not be in the list)
        let slots = if state == 0 { slots } else { &empty };
        let mut mask = 0u32;
        let mut ents = Writer::new();
        for (&sl, iid) in slots {
            let idx = items.iter().position(|i| i.0 == *iid).ok_or(Error::Invalid("slot names an unknown item"))?;
            mask |= 1u32.checked_shl(sl as u32).ok_or(Error::Invalid("slot"))?;
            ents.u16(idx as u16).u16(counts.get(&sl).copied().unwrap_or(0).min(0xFFFF) as u16);
        }
        w.u8(state).u8(u8::from(enabled)).u32(id);
        put_str(w, name, 128);
        w.u32(mask).bytes(&ents.finish()?);
        put_mods(w, p.mods);
        put_str(w, voice, 32); // AI sound scheme name (ai_sound_player::set_scheme, 0x2003686D0)
    }
    for &(iid, did, c) in &items {
        match p.human {
            Some(h) => human_item(w, h, p.modules, iid, did, c)?,
            None => bot_item(w, p.modules, &items, p.kit_attach, iid, did, c)?,
        }
    }
    if let Some((ai, first, second)) = p.ai {
        ai_block(w, ai);
        // u8 +0x2F2, u8 +0x2F3, u32 +0x2E4 (LIKELY match_making_level), f32 +0x2E8, u8 +0x2F4, u8 bool +0x2EE
        w.u8(first).u8(second).u32(ai.match_making_level);
        put_f32(w, ai.hit_damage_multiplier);
        w.u8(ai.hit_chance as u8).u8(ai.forgive_headshots as u8);
    }
    Ok(())
}

/// AI block, reader 0x200125B40. PROVEN: the two PID controllers, the noise block order, capsules, +0x94,
/// +0x98 think period. LIKELY: the order inside the first group, +0x88 aim distance, the meaning of most u32.
fn ai_block(w: &mut Writer, p: &AiPlayer) {
    let (v, h) = (&p.vision_sensor_parameters, &p.hearing_sensor_parameters);
    let g1 = [
        v.vertical_fov,
        v.aspect_ratio,
        v.near_plane_distance,
        v.far_plane_distance,
        v.increase_speed,
        v.decrease_speed,
        v.visibility_inertia,
        v.velocity_factor,
        h.crouch_sound_hearing_radius,
        h.walk_sound_hearing_radius,
        h.shot_sound_hearing_radius,
        h.sprint_sound_hearing_radius,
    ];
    let pid = |d: &game_data::matches::Pid| [d.mass, d.proportional_gain, d.integral_gain, d.derivational_gain, d.max_output];
    let (hn, pn) = (&p.heading_noise_parameters, &p.pitch_noise_parameters);
    let mut g2 = Vec::with_capacity(18);
    g2.extend(pid(&p.heading_pid_controller_parameters));
    g2.extend(pid(&p.pitch_pid_controller_parameters));
    g2.extend([
        hn.deviation,
        hn.max_value,
        pn.deviation,
        pn.max_value,
        p.aim_distance,
        p.enemy_detection_stand_capsule_radius,
        p.enemy_detection_crouch_capsule_radius,
        p.heading_value_too_large_for_fire_after,
    ]);
    // +0x98 think period (0x20036CD93, must be >= tick_ms), +0x9C .. +0xB8
    let ints = [
        p.tick_time_in_ms.max(2 * TICK_MS),
        p.fire_delay_after_enemy_change_in_ms,
        p.vision_reaction_delay_in_ms,
        p.sound_reaction_delay_in_ms,
        p.hit_reaction_delay_in_ms,
        p.shoot_after_aim_delay_in_ms,
        p.can_kill_series_before_shot_time_in_ms,
        p.enemy_death_reaction_delay_in_ms,
        p.last_sensed_enemy_cull_down_time_in_ms,
    ];
    for x in g1 {
        put_f32(w, x);
    }
    put_str(w, &p.sound_scheme, 32);
    for x in g2 {
        put_f32(w, x);
    }
    w.u32(p.fire_queue_distance_parameters.len() as u32);
    for x in ints {
        w.u32(x);
    }
    // entry = {u32 A, u32 B, u32 C, u32 D, f32 distance}: length base + spread, pause base + spread (LIKELY)
    for e in &p.fire_queue_distance_parameters {
        w.u32(e.length[0]).u32(e.length[1].wrapping_sub(e.length[0]));
        w.u32(e.interval[0]).u32(e.interval[1].wrapping_sub(e.interval[0]));
        put_f32(w, e.distance);
    }
}

/// Level of the tutorial (`gameplay/game_modes/tutorial.options` `level_name`; `projects/tutorial/default`).
const TUTORIAL_PROJECT: &str = "tutorial";
const TUTORIAL_MAP: &str = "tutorial/default";

/// Who plays besides the human.
#[derive(Debug, Clone, Copy)]
enum Bots {
    /// `count` AI players picked by level: alternating teams (Training with bots), or all on `team` (the shooting
    /// range's targets)
    Picked { count: usize, level: u32, team: Option<u8> },
    /// the tutorial script's bots, by name (`tutorial_069d.json`)
    Tutorial,
}

/// The tutorial's human: the account's name, face, skills and boosters with the four kits `tutorial_player_0..3` as
/// profiles 0..3, all present (state 0). The script spawns profile 3 (the start kit) and switches to 0 / 1 / 2 itself
/// without checking it exists (0x2003CDC2E: a missing profile crashes the client). Items are fresh instances (ids from
/// 1), kit modules installed; the account's own items, upgrades,
/// decals and workshop state are not used.
pub fn tutorial_human(gd: &GameData, h: &HumanLoadout) -> HumanLoadout {
    let mut out = HumanLoadout {
        items: vec![],
        slots: BTreeMap::new(),
        slot_rounds: BTreeMap::new(),
        upgrades: BTreeMap::new(),
        attach: vec![],
        removed_defaults: BTreeMap::new(),
        bound: BTreeMap::new(),
        decals: BTreeMap::new(),
        profiles: vec![],
        ..h.clone()
    };
    for (i, def) in gd.matches.tutorial().human.iter().enumerate() {
        let kit = gd.matches.kit(&gd.items, def);
        let base = out.items.len() as u32;
        out.items.extend(kit.items.iter().map(|&(iid, did, c)| (iid + base, did, c)));
        out.attach.extend(kit.attach.iter().map(|(wp, l)| (wp + base, l.iter().map(|(s, m)| (s.clone(), m + base)).collect())));
        let slots: BTreeMap<u8, u32> = kit.slots.iter().map(|(s, iid)| (*s, iid + base)).collect();
        out.slots = slots.clone();
        out.slot_rounds = kit.counts.clone();
        // the name is never shown by the tutorial (the script picks the kits); the kits have no display name
        out.profiles.push(HumanProfile { state: 0, enabled: true, id: i as u32 + 1, name: String::new(), slots, rounds: kit.counts });
    }
    out
}

/// Every item of an ammo slot (8, 9, 11, 12) or a pocket (13..17) of the loadout, over all profiles: the training storage.
fn ammo_items(h: &HumanLoadout) -> Vec<u32> {
    let mut v: Vec<u32> = storage_slots().filter_map(|s| h.slots.get(&s).copied()).collect();
    for p in &h.profiles {
        v.extend(storage_slots().filter_map(|s| p.slots.get(&s).copied()));
    }
    v
}

#[allow(clippy::too_many_arguments)]
fn build(
    gd: &GameData,
    project: &str,
    map_name: &str,
    bots: Bots,
    human: &HumanLoadout,
    match_id: u32,
    map: u32,
    mode: u32,
    ammo_storage: bool,
) -> Result<(Vec<u8>, Roster), Error> {
    let n_players = match bots {
        Bots::Picked { count, .. } => (count + 1).clamp(1, MAX_PLAYERS),
        Bots::Tutorial => gd.matches.tutorial().bots.len() + 1,
    };
    if n_players > MAX_PLAYERS {
        return Err(Error::Limit("players of a local match (1..=20)"));
    }
    let hm = if ammo_storage { online_human(human) } else { human.clone() };
    let mut w = Writer::new();
    w.u16(SEND_PERIOD_MS).u16(0).u8(TICK_MS as u8).u8(n_players as u8).u32(match_id);
    w.u8(PACKED_INPUTS).u8(1).u8(0).u8(0); // packed_inputs (0: one input = 11 bytes u16,u16,u56 BE), single_player, b3, +0x107D
    w.u8(1).u8(0); // match_level (+0x1081), +0x1082
    put_str(&mut w, project, 0x800);
    put_str(&mut w, map_name, 0x800);
    let mut hmods = gd.matches.skill_modifiers(&hm.skills);
    for (id, v) in &hm.extra_modifiers {
        *hmods.entry(*id).or_insert(0.0) += v;
    }
    player(
        &mut w,
        &Pl {
            modules: &gd.modules,
            team: 0,
            name: &hm.name,
            clan: &hm.clan,
            account_id: hm.account_id,
            ai_id: 0xFFFF_FFFF,
            items: &hm.items,
            slots: &hm.slots,
            counts: &hm.slot_rounds,
            voice: "",
            mods: &hmods,
            human: Some(&hm),
            ai: None,
            kit_attach: &[],
            face: hm.face,
            pve_budget: (0, 0, 0),
            bot_profiles: &[],
        },
    )?;
    let (picked, bot_level) = match bots {
        Bots::Picked { level, .. } => (n_players - 1, level),
        Bots::Tutorial => (0, DEFAULT_BOT_LEVEL),
    };
    if let Bots::Tutorial = bots {
        // the script's bots: record name = script name (bound by name, 0x2003C9464), ai_id != FFFFFFFF and distinct,
        // team and face (`head`) from the data, AI block from `ai_params`
        for (k, b) in gd.matches.tutorial().bots.iter().enumerate() {
            // the client indexes `ai_players.options` `players[ai_id]` with no bound check (character_sigma_params,
            // 0x20014C7CA): the ai id must stay below the list's length (36)
            if k >= gd.matches.ai_players().len() {
                return Err(Error::Limit("tutorial bots (ai id indexes ai_players)"));
            }
            let kit = gd.matches.kit(&gd.items, &b.kit);
            let (f, s) = game_data::MatchData::bot_name_indices(k);
            let id = (0xB07u64 << 32) | (k as u64 + 1);
            let voice = b.kit.voices.as_deref().filter(|v| !v.is_empty()).unwrap_or("silent");
            let mods = gd.matches.skill_modifiers(&b.ai.skills);
            player(
                &mut w,
                &Pl {
                    modules: &gd.modules,
                    team: b.team,
                    name: &b.name,
                    clan: "",
                    account_id: id.to_le_bytes(),
                    ai_id: k as u32,
                    items: &kit.items,
                    slots: &kit.slots,
                    counts: &kit.counts,
                    voice,
                    mods: &mods,
                    human: None,
                    ai: Some((&b.ai, f as u8, s as u8)),
                    kit_attach: &kit.attach,
                    face: b.head,
                    pve_budget: (0, 0, 0),
                    bot_profiles: &[],
                },
            )?;
        }
    }
    let fixed_team = match bots {
        Bots::Picked { team, .. } => team,
        Bots::Tutorial => None,
    };
    let mut bots = Vec::new();
    for (k, ai_idx) in gd.matches.pick_bots(picked, bot_level).into_iter().enumerate() {
        let ai = &gd.matches.ai_players()[ai_idx];
        let team = fixed_team.unwrap_or_else(|| game_data::MatchData::bot_team(k));
        let kit = gd.matches.kit(&gd.items, &ai.profiles[0]);
        let (f, s) = game_data::MatchData::bot_name_indices(k);
        let name = format!("Bot {}", k + 1);
        let id = (0xB07u64 << 32) | (k as u64 + 1);
        let voice = ai.profiles[0].voices.as_deref().filter(|v| !v.is_empty()).unwrap_or("silent");
        let mods = gd.matches.skill_modifiers(&ai.skills);
        player(
            &mut w,
            &Pl {
                modules: &gd.modules,
                team,
                name: &name,
                clan: "",
                account_id: id.to_le_bytes(),
                ai_id: ai_idx as u32,
                items: &kit.items,
                slots: &kit.slots,
                counts: &kit.counts,
                voice,
                mods: &mods,
                human: None,
                ai: Some((ai, f as u8, s as u8)),
                kit_attach: &kit.attach,
                face: 0,
                pve_budget: (0, 0, 0),
                bot_profiles: &[],
            },
        )?;
        bots.push(RosterBot { k: k as u32, team, ai: ai_idx as u32, first: f as u32, second: s as u32, name });
    }
    w.zeros(20 + 40 + 40); // the three trailing tables (LIKELY all zero)
    let roster = Roster { match_id, map_id: map, mode_id: mode, human_name: hm.name.clone(), bots };
    Ok((w.finish()?, roster))
}

/// Options of a Training-with-bots match (`83 5B 01`) and the roster for the results screen.
pub fn training_options(gd: &GameData, p: &TrainingParams, human: &HumanLoadout) -> Result<(Vec<u8>, Roster), Error> {
    let row = gd.matches.resolve_map(p.map, p.mode, p.weather).ok_or(Error::NoMode { map: p.map, mode: p.mode, variation: p.weather })?;
    let n = p.n_players.unwrap_or(match (row.max_players, row.min_players) {
        (0, 0) => 10,
        (0, m) => m as usize,
        (m, _) => m as usize,
    });
    let count = n.clamp(1, MAX_PLAYERS) - 1;
    build(gd, &row.project, &row.map_name, Bots::Picked { count, level: p.bot_level, team: None }, human, p.match_id, p.map, p.mode, true)
}

/// One human of an online match, in roster order (the roster index is the player index of the match).
#[derive(Debug, Clone, PartialEq)]
pub struct OnlinePlayer {
    /// team index 0 / 1 (`+0x2EC`)
    pub team: u8,
    pub human: HumanLoadout,
    /// hero-of-the-day title `(hero_place 1..=3, nomination 0..=2)`, None = no title: the third trailing table
    /// (in-match scoreboard icon `nomination * 3 + 4 - hero_place`, HUD `PlayerListRenderer`; reader 0x20019F980 reads
    /// options +0x4C + 2i / +0x4D + 2i, PROVEN; i = the player's record index, LIKELY)
    pub hero: Option<(u8, u8)>,
    /// PvE only: `(access keys, gold)` the player may spend on paid respawns in the match, written to his record
    /// `+0x2D0` / `+0x2D4` (PROVEN fields: the client's SPACE dialog 0x20023DE1A and the world's charge 0x2003CF5A6
    /// compare against them as spendable totals). `(0, 0)` = no paid respawn (the client shows
    /// its "can't" message and sends nothing). Ignored outside PvE.
    pub pve_budget: (u32, u32),
}

/// What the lobby decides for an online match; `match_id`, `tick_ms` and `send_period_ms` must be the values of the
/// cached world the match server will send (`WorldInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnlineParams {
    pub map: u32,
    pub mode: u32,
    pub weather: u32,
    pub match_id: u32,
    pub tick_ms: u8,
    pub send_period_ms: u16,
    /// The options match-type byte (+0x107D, the 4th flag byte; reader store 0x2001251C8): [`MATCH_TYPE_PLAIN`],
    /// [`MATCH_TYPE_RANKED`] or [`MATCH_TYPE_CLAN`].
    pub match_type: u8,
    /// Options +0x107A: the maximum remote delay in ms (PROVEN, 0x2002EFED6: the lag compensation cap, the history
    /// ring size min(X / tick + 1, 16), the high-ping HUD thresholds 0.6..1.05 x X). 0 would
    /// show the high-ping icon always and turn lag compensation off. See [`max_delay_ms`].
    pub max_delay_ms: u16,
}

/// The maximum remote delay of an online match: 500 ms (CHOICE: the original value is unknown; the client's own 500 ms
/// horizon constant 0x1F4 at 0x2001AF4FC), at most 15 ticks (the 16-entry history ring).
pub fn max_delay_ms(tick_ms: u8) -> u16 {
    500u16.min(15 * u16::from(tick_ms))
}

pub use protocol::s2c::online::{MATCH_TYPE_CLAN, MATCH_TYPE_PLAIN, MATCH_TYPE_RANKED};

/// The `match_options` blob of an ONLINE match: the body of the match server's DD (reader 0x2001250E0, the same blob
/// as `83 5B`; docs/notes/online.md section 1). Same record writers as the training options, different header and
/// roster: `n_players` humans (no AI block, `ai_id = FFFFFFFF`), `single_player = 0` (the seed is `match_id`, the end
/// event carries `match_id`), `packed_inputs = 0`, the ammo and pocket stacks with [`AMMO_STORAGE`] (as in
/// training: the box refills from it), the PvP bots' kits too; the PvE script bots keep the data's counts.
/// UNVERIFIED (never run): `match_level` (+0x1081, only logged) and the byte at +0x1082 are written as in the training
/// options. Trailing tables (reader 0x20012527B): 20 x u8 squad id and 20 x {u8 rank, u8 league top} stay
/// zero; 20 x {u8 hero_place, u8 nomination} carry the players' hero titles ([`OnlinePlayer::hero`]).
pub fn online_options(gd: &GameData, p: &OnlineParams, players: &[OnlinePlayer]) -> Result<Vec<u8>, Error> {
    if p.tick_ms == 0 || p.send_period_ms == 0 {
        return Err(Error::Invalid("tick_ms and send_period must not be 0"));
    }
    if players.is_empty() || players.len() > MAX_PLAYERS {
        return Err(Error::Limit("players of an online match (1..=20)"));
    }
    let row = gd.matches.resolve_map(p.map, p.mode, p.weather).ok_or(Error::NoMode { map: p.map, mode: p.mode, variation: p.weather })?;
    online_blob(gd, p, players, &row.project, &row.map_name, 1, 0, None, &[])
}

/// One AI player of an online PvP match: `ai_players.options` entry `ai` as the
/// record's ai id, its team, and the data's name indexes. The client shows `players_1st_names[first]` +
/// `players_2nd_names[second]` for a record with an ai id (0x20011B2D0, bounds-checked), not the record name.
#[derive(Debug, Clone)]
pub struct OnlineBot {
    pub team: u8,
    /// index into `ai_players.options` `players[]`: below its length (36; the client indexes it with no bound check,
    /// 0x20014C7CA) and each at most once a match (`limit_in_match` 1)
    pub ai: usize,
    pub first: u8,
    pub second: u8,
    /// a kit other than the AI player's own `profiles[0]` (offline tests only: a weapon outside the data kits); None =
    /// the data's kit
    pub kit: Option<game_data::matches::AiProfile>,
}

impl OnlineBot {
    /// The kit the record carries: the override, else the AI player's `profiles[0]`.
    pub fn profile<'a>(&'a self, gd: &'a GameData) -> Option<&'a game_data::matches::AiProfile> {
        self.kit.as_ref().or_else(|| gd.matches.ai_players().get(self.ai).and_then(|a| a.profiles.first()))
    }

    /// The name the record carries (the client shows the name indexes; logs and the relay use this): the data's two
    /// name lists, else "Bot k".
    pub fn name(&self, gd: &GameData, k: usize) -> String {
        gd.matches.bot_name(usize::from(self.first), usize::from(self.second), &format!("Bot {}", k + 1))
    }
}

/// The bots of an online PvP match: `need[t]` AI players for team t, picked by [`game_data::MatchData::pick_bots`] at
/// `level` (nearest `match_making_level` first; each AI player once). The picks go in turn to the team that still needs
/// the most (ties: the team with fewer bots so far, then team 1), so both teams get the same level mix; name indexes
/// [`game_data::MatchData::bot_name_indices`] of the bot number (distinct pairs).
pub fn pick_online_bots(gd: &GameData, need: [usize; 2], level: u32) -> Vec<OnlineBot> {
    let picks = gd.matches.pick_bots(need[0] + need[1], level);
    let mut left = need;
    let mut have = [0usize; 2];
    let mut out = Vec::with_capacity(picks.len());
    for (k, ai) in picks.into_iter().enumerate() {
        let team = if left[0] > left[1] || (left[0] == left[1] && have[0] < have[1]) { 0 } else { 1 };
        let team = if left[team] == 0 { 1 - team } else { team };
        left[team] = left[team].saturating_sub(1);
        have[team] += 1;
        let (f, s) = game_data::MatchData::bot_name_indices(k);
        out.push(OnlineBot { team: team as u8, ai, first: f as u8, second: s as u8, kit: None });
    }
    out
}

/// The `match_options` of an online PvP match with AI players: the humans in seat order, then `bots` as Training's
/// AI records (options.rs `build`, `Bots::Picked`: synthetic account id `0xB07 << 32 | k + 1`, the kit's sound scheme,
/// the AI player's skills, AI block, name indexes, match-making level), `n_players` = humans + bots. Header as
/// [`online_options`] (single_player 0: the world host ticks the AI).
pub fn online_options_bots(gd: &GameData, p: &OnlineParams, players: &[OnlinePlayer], bots: &[OnlineBot]) -> Result<Vec<u8>, Error> {
    if p.tick_ms == 0 || p.send_period_ms == 0 {
        return Err(Error::Invalid("tick_ms and send_period must not be 0"));
    }
    let row = gd.matches.resolve_map(p.map, p.mode, p.weather).ok_or(Error::NoMode { map: p.map, mode: p.mode, variation: p.weather })?;
    online_options_row(gd, p, row, players, bots)
}

/// [`online_options_bots`] on a given row of the data (map, mode, variation AND weather: the level project the clients
/// load), as the PvP queue picks it (lobby `queue::PvpQueue::pick`). `p.map` / `p.mode` must be the row's.
pub fn online_options_row(gd: &GameData, p: &OnlineParams, row: &game_data::ModeRow, players: &[OnlinePlayer], bots: &[OnlineBot]) -> Result<Vec<u8>, Error> {
    if p.tick_ms == 0 || p.send_period_ms == 0 {
        return Err(Error::Invalid("tick_ms and send_period must not be 0"));
    }
    if (p.map, p.mode) != (row.map, row.mode) {
        return Err(Error::Invalid("the params name another map / mode than the row"));
    }
    check_online_bots(gd, players.len(), bots)?;
    online_blob(gd, p, players, &row.project, &row.map_name, 1, 0, None, bots)
}

/// The rules of an online bot roster: at least one human, at most [`MAX_PLAYERS`] records, every ai id inside the AI
/// list and used once, teams 0 / 1, a kit for every bot.
pub(crate) fn check_online_bots(gd: &GameData, humans: usize, bots: &[OnlineBot]) -> Result<(), Error> {
    if humans == 0 || humans + bots.len() > MAX_PLAYERS {
        return Err(Error::Limit("players of an online match (1..=20, at least one human)"));
    }
    let n = gd.matches.ai_players().len();
    let mut seen = std::collections::BTreeSet::new();
    for b in bots {
        if b.ai >= n {
            return Err(Error::Invalid("bot ai id outside ai_players.options"));
        }
        if !seen.insert(b.ai) {
            return Err(Error::Invalid("an AI player twice in one match (limit_in_match 1)"));
        }
        if b.team > 1 {
            return Err(Error::Invalid("bot team"));
        }
        if b.profile(gd).is_none() {
            return Err(Error::Invalid("an AI player without a kit"));
        }
    }
    Ok(())
}

/// The modifier block of an online human's profiles: the learned skills' sums plus the per-match boosters (lobby
/// `boosters::match_modifier_ids`), added like the training human's.
pub(crate) fn online_mods(gd: &GameData, h: &HumanLoadout) -> BTreeMap<u32, f64> {
    let mut mods = gd.matches.skill_modifiers(&h.skills);
    for (id, v) in &h.extra_modifiers {
        *mods.entry(*id).or_insert(0.0) += v;
    }
    mods
}

/// The online blob on explicit level strings: `level` = +0x1081 (match level / PvE equipment level), `b1082` = +0x1082
/// (PvE difficulty, 0 otherwise). `pve` = the PvE difficulty whose 17 script bots follow the humans.
#[allow(clippy::too_many_arguments)]
pub(crate) fn online_blob(gd: &GameData, p: &OnlineParams, players: &[OnlinePlayer], project: &str, map_name: &str, level: u8, b1082: u8, pve: Option<u8>, pvp: &[OnlineBot]) -> Result<Vec<u8>, Error> {
    let bots = match pve {
        Some(d) => pve_bots(gd, d)?,
        None => vec![],
    };
    if players.is_empty() || players.len() + bots.len() + pvp.len() > MAX_PLAYERS {
        return Err(Error::Limit("players of an online match (1..=20)"));
    }
    // a human's PvE budget: his key / gold budget (`OnlinePlayer::pve_budget`: the caller gives one only when a world host
    // reports the spends) and the respawn price of the data
    let price = gd.pve.prices.respawn_price.min(u32::from(u16::MAX)) as u16;
    let budget = |pl: &OnlinePlayer| if pve.is_some() { (pl.pve_budget.0, pl.pve_budget.1, price) } else { (0, 0, 0) };
    let mut w = Writer::new();
    w.u16(p.send_period_ms).u16(p.max_delay_ms).u8(p.tick_ms).u8((players.len() + bots.len() + pvp.len()) as u8).u32(p.match_id);
    w.u8(PACKED_INPUTS).u8(0).u8(0).u8(p.match_type); // packed_inputs, single_player = 0, b3, match type (+0x107D)
    w.u8(level).u8(b1082); // match_level (+0x1081), +0x1082
    put_str(&mut w, project, 0x800);
    put_str(&mut w, map_name, 0x800);
    for pl in players {
        // the stacks the match loads and the box refills get the storage (AMMO_STORAGE): with the owned count the box
        // would refill nothing after the first spawn
        let h = &online_human(&pl.human);
        let mods = online_mods(gd, h);
        player(
            &mut w,
            &Pl {
                modules: &gd.modules,
                team: pl.team,
                name: &h.name,
                clan: &h.clan,
                account_id: h.account_id,
                ai_id: 0xFFFF_FFFF,
                items: &h.items,
                slots: &h.slots,
                counts: &h.slot_rounds,
                voice: "",
                mods: &mods,
                human: Some(h),
                ai: None,
                kit_attach: &[],
                face: h.face,
                pve_budget: budget(pl),
                bot_profiles: &[],
            },
        )?;
    }
    // the script's bots (Bots::Tutorial is the pattern): record name = script name, ai id k (< 36: it indexes
    // ai_players.options with no bound check, 0x20014C7CA), team / face / AI block from the data, two kit profiles
    for (k, (b, rec)) in bots.iter().enumerate() {
        let (f, s) = game_data::MatchData::bot_name_indices(k);
        let id = (0xB07u64 << 32) | (k as u64 + 1);
        let mods = gd.matches.skill_modifiers(&b.ai.skills);
        let empty = BTreeMap::new();
        player(
            &mut w,
            &Pl {
                modules: &gd.modules,
                team: b.team,
                name: &b.name,
                clan: "",
                account_id: id.to_le_bytes(),
                ai_id: k as u32,
                items: &rec.items,
                slots: &empty,
                counts: &empty,
                voice: "",
                mods: &mods,
                human: None,
                ai: Some((&b.ai, f as u8, s as u8)),
                kit_attach: &rec.attach,
                face: b.head,
                pve_budget: (0, 0, 0),
                bot_profiles: &rec.profiles,
            },
        )?;
    }
    // online PvP AI players: Training's record (`build`, `Bots::Picked`), the kit as the single profile
    for (k, b) in pvp.iter().enumerate() {
        let ai = gd.matches.ai_players().get(b.ai).ok_or(Error::Invalid("bot ai id outside ai_players.options"))?;
        let prof = b.profile(gd).ok_or(Error::Invalid("an AI player without a kit"))?;
        let kit = gd.matches.kit(&gd.items, prof);
        // the kit's ammo / pocket stacks carry the storage like the humans' (the data kit's count is one life's load:
        // without it the bots' loads would shrink life after life)
        let kit_items = with_storage(&kit.items, &storage_slots().filter_map(|s| kit.slots.get(&s).copied()).collect::<Vec<_>>());
        let name = b.name(gd, k);
        let id = (0xB07u64 << 32) | (k as u64 + 1);
        let voice = prof.voices.as_deref().filter(|v| !v.is_empty()).unwrap_or("silent");
        let mods = gd.matches.skill_modifiers(&ai.skills);
        player(
            &mut w,
            &Pl {
                modules: &gd.modules,
                team: b.team,
                name: &name,
                clan: "",
                account_id: id.to_le_bytes(),
                ai_id: b.ai as u32,
                items: &kit_items,
                slots: &kit.slots,
                counts: &kit.counts,
                voice,
                mods: &mods,
                human: None,
                ai: Some((ai, b.first, b.second)),
                kit_attach: &kit.attach,
                face: 0,
                pve_budget: (0, 0, 0),
                bot_profiles: &[],
            },
        )?;
    }
    w.zeros(20 + 40);
    for i in 0..MAX_PLAYERS {
        let (place, nomination) = players.get(i).and_then(|p| p.hero).unwrap_or((0, 0));
        w.u8(place).u8(nomination);
    }
    Ok(w.finish()?)
}

/// Options of the tutorial (`83 5B 02`): the human ([`tutorial_human`]: four kit
/// profiles) on team 0 as the first record, then the nine script bots, `n_players` 10, on `tutorial/default`. The kits'
/// ammo is the data's (no training storage). Pack with [`crate::pack_5b`] sub 2.
pub fn tutorial_options(gd: &GameData, human: &HumanLoadout, match_id: u32) -> Result<Vec<u8>, Error> {
    let h = tutorial_human(gd, human);
    Ok(build(gd, TUTORIAL_PROJECT, TUTORIAL_MAP, Bots::Tutorial, &h, match_id, 0, 1, false)?.0)
}

/// Project string of the shooting range: `mode.level_name` of `gameplay/game_modes/shooting_range.options` (PROVEN:
/// 0.69d data; the file is the `shooting_range` entry of the loader's project list, exe string 0x20095E058, and the
/// level files are `projects/shooting_range/test/client_project`).
pub const SHOOTING_RANGE_PROJECT: &str = "shooting_range";
/// `match_making_params.map_name` of the same file (PROVEN, 0.69d data).
pub const SHOOTING_RANGE_MAP: &str = "shooting_range/test";

/// AI players of the shooting range: the level has 8 respawn points with `team 1` and `use_for_ai true` and 8 team-1
/// patrol graphs (6 standing / crouching posts, 2 walking loops); the human's point is `team 0`, `use_for_ai false`
/// (PROVEN: `projects/shooting_range/test/client_project`). The count is in no client file (LIKELY: one bot per point).
pub const SHOOTING_RANGE_BOTS: usize = 8;
/// Their team: every bot point and graph of the range is team 1 (PROVEN, same level data).
pub const SHOOTING_RANGE_BOT_TEAM: u8 = 1;

/// Options of the shooting range (`83 5B 03`): the human on `shooting_range/test` and [`SHOOTING_RANGE_BOTS`] AI players,
/// all on team 1, as targets. They never attack: for a local match of kind 3 the client sets its bots-ignore-players
/// byte (0x2001337E2 / 0x200133817), so sight and sound skip the human, and bots of one team never pick each other
/// (PROVEN, enemy selection 0x20036AF10 / fire check 0x20036D450). OPEN: whether a bot the player wounds fires back (the
/// damage memory has no ignore check). Same options reader as every local match (PROVEN: the sub only goes to
/// `nc+0x7A9ADC`, and only the value 2 is special in the code read). Pack with [`crate::pack_5b`] sub 3.
pub fn shooting_range_options(gd: &GameData, human: &HumanLoadout, match_id: u32) -> Result<Vec<u8>, Error> {
    Ok(shooting_range_build(gd, human, match_id)?.0)
}

fn shooting_range_build(gd: &GameData, human: &HumanLoadout, match_id: u32) -> Result<(Vec<u8>, Roster), Error> {
    let bots = Bots::Picked { count: SHOOTING_RANGE_BOTS, level: DEFAULT_BOT_LEVEL, team: Some(SHOOTING_RANGE_BOT_TEAM) };
    build(gd, SHOOTING_RANGE_PROJECT, SHOOTING_RANGE_MAP, bots, human, match_id, 0, 1, true)
}

/// `+0x1081` of a PvE match and of its capture: the equipment level (pve.lua never reads it, docs/notes/pve.md 10.3). One
/// value for both, so the captured world and the match agree on every options byte but the roster (CHOICE: 1; the
/// original's value is OPEN).
pub const PVE_EQUIPMENT_LEVEL: u8 = 1;

/// The PvE nuts (`throwable_screw`): pve.lua's pickup (processor `tips_shoot_gaika`, action 48
/// `item=1355 amount=10000`) only refills a 1355 record already in the human's item list (give-item 0x2003CE960 searches
/// it and does nothing without it; 1355 is in no client constant), so the server must send one. PROVEN: F3
/// (`screw_use_1`, action 0x49 -> command bit 21, 0x20023FEF1) is quick slot 7 = profile slot 20 (0x20034C2DE); the
/// throw needs pocket count > 0 (0x20039DC30); pocket capacity = slot count x stack size 1.
pub const PVE_NUT_DICT: u16 = 1355;
pub const PVE_NUT_SLOT: u8 = 20;
/// Slot count of the nut pocket: at least 1 or the pickup fills nothing (capacity 0). CHOICE: 1, the smallest that works;
/// the original's value is OPEN. The record's storage is 0 ("pick up the nuts" objective; the pickup sets 10000).
pub const PVE_NUT_SLOT_COUNT: u32 = 1;

/// A PvE human's loadout as the mission sends it: the player's own kit plus the nut record (fresh uid, storage 0) in slot
/// 20 of every present (state 0) profile. [`pve_options`] and `pve_roster_spec` both use it, so the options and the
/// world's per-player objects (the nut adds `max(5, 2 x count)` grenade objects, start_objects.rs) agree.
pub fn pve_human(h: &HumanLoadout) -> HumanLoadout {
    let mut h = h.clone();
    if h.items.iter().any(|i| i.1 == PVE_NUT_DICT) {
        return h;
    }
    let uid = h.items.iter().map(|i| i.0).chain(h.decals.values().flatten().map(|d| d.0)).max().unwrap_or(0) + 1;
    h.items.push((uid, PVE_NUT_DICT, 0));
    if h.profiles.is_empty() {
        h.slots.insert(PVE_NUT_SLOT, uid);
        h.slot_rounds.insert(PVE_NUT_SLOT, PVE_NUT_SLOT_COUNT);
    } else {
        for q in h.profiles.iter_mut().filter(|q| q.state == 0) {
            q.slots.entry(PVE_NUT_SLOT).or_insert(uid);
            q.rounds.entry(PVE_NUT_SLOT).or_insert(PVE_NUT_SLOT_COUNT);
        }
    }
    h
}

/// Options of an online PvE mission (`DD`): the humans (team 0, `ai_id` -1, the respawn
/// price) in seat order, then the 17 script bots `pve_bot_1..17` of the difficulty (team 1, two kit profiles each, the
/// difficulty's AI block); `n_players` = humans + 17, `single_player` 0, `+0x1082` = the difficulty (0..2: also the maze
/// damage profile index, unchecked by the client), `+0x1081` = `level`, the project `pve_01_v{difficulty + 1}_maze{maze}`
/// and the map `pve_01/v{difficulty + 1}_maze{maze}` (the six mode files' `map_name`). `p.match_id` must be the seed of
/// the project's captured world. `p.map / mode / weather` are not used.
pub fn pve_options(gd: &GameData, p: &OnlineParams, humans: &[OnlinePlayer], difficulty: u8, maze: u8, level: u8) -> Result<Vec<u8>, Error> {
    if p.tick_ms == 0 || p.send_period_ms == 0 {
        return Err(Error::Invalid("tick_ms and send_period must not be 0"));
    }
    let project = gd.pve.project(difficulty, maze).ok_or(Error::Invalid("no PvE project for this difficulty / maze"))?;
    let map_name = format!("pve_01/v{}_maze{}", difficulty as u32 + 1, maze);
    let humans: Vec<OnlinePlayer> = humans.iter().map(|pl| OnlinePlayer { human: pve_human(&pl.human), ..pl.clone() }).collect();
    online_blob(gd, p, &humans, &project, &map_name, level, difficulty, Some(difficulty), &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A default the player removed in the workshop gets no n4 element; installed slots are skipped too.
    #[test]
    fn removed_defaults_get_no_n4_element() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let all = gd.modules.default_modules(1201).len();
        assert!(all > 2);
        // n4 count = the first byte put_n4 writes
        let n4 = |items: &[(u32, u16, u32)], attach: &[(u32, Vec<(String, u32)>)], removed: &[String]| {
            let mut w = Writer::new();
            put_n4(&mut w, &gd.modules, items, attach, &BTreeMap::new(), 10, 1201, removed).unwrap();
            w.finish().unwrap()[0] as usize
        };
        assert_eq!(n4(&[], &[], &[]), all);
        // an installed mag takes the default's place, a removed forend gets nothing
        let mag = gd.modules.default_modules(1201).iter().find(|(s, _)| s == "mag").map(|(_, d)| *d).unwrap();
        assert_eq!(n4(&[(11, mag, 1)], &[(10, vec![("mag".to_string(), 11)])], &["forend".to_string()]), all - 1);
    }

    /// An installed module's n4 element carries its pairs (the match's item cook applies them to the weapon,
    /// 0x2003356E0): a 40-round magazine (dict 1991, bought for gold: premium (311, 7) (407, 7), rarity (312, 4),
    /// capacity (520, 7) = 40) on its weapon; a module stored without pairs still carries its type's capacity pair; the
    /// defaults carry none. The weapon record decodes with the lobby's own item record layout.
    #[test]
    fn installed_module_elements_carry_their_modifier_pairs() {
        game_data::require_tables!();
        use protocol::items::ItemRecord;
        use protocol::Decode;
        let gd = game_data::testing::tables();
        let weapon = *gd.modules.possible_owners(1991).iter().find(|w| gd.modules.can_attach_to(1991, **w, "mag")).unwrap();
        let mut h = HumanLoadout::default_loadout();
        h.items = vec![(100002, weapon, u32::from(weapon)), (100049, 1991, 1991)];
        h.attach = vec![(100002, vec![("mag".to_string(), 100049)])];
        let record = |h: &HumanLoadout| {
            let mut w = Writer::new();
            human_item(&mut w, h, &gd.modules, 100002, weapon, u32::from(weapon)).unwrap();
            ItemRecord::from_bytes(&w.finish().unwrap()).unwrap()
        };
        h.upgrades = BTreeMap::from([(100049, gd.modules.bought_mods(1991).unwrap())]);
        let r = record(&h);
        let mag = r.modules.iter().find(|m| m.uid == 100049).unwrap();
        assert_eq!((mag.dict, mag.slot.as_str()), (1991, "mag"));
        assert_eq!(mag.mods, vec![(311, 7), (407, 7), (312, 4), (520, 7)]);
        assert!(r.modules.iter().filter(|m| m.uid == u32::MAX).all(|m| m.mods.is_empty()), "defaults carry no pairs");
        // stored without pairs (e.g. a seeded module or one from a chest): the capacity pair alone
        h.upgrades.clear();
        assert_eq!(record(&h).modules.iter().find(|m| m.uid == 100049).unwrap().mods, vec![(520, 7)]);
    }

    /// Training storage covers the pocket stacks too (a medkit in slot 13 of any profile), not other items.
    #[test]
    fn training_storage_covers_ammo_and_pockets() {
        let mut h = HumanLoadout::default_loadout();
        h.items.push((9, 65, 1));
        h.profiles = vec![HumanProfile { state: 0, enabled: true, id: 1, name: String::new(), slots: BTreeMap::from([(7, 1), (13, 9)]), rounds: BTreeMap::from([(13, 1)]) }];
        let v = ammo_items(&h);
        assert!(v.contains(&9), "pocket stack of a profile");
        assert!(v.contains(&2), "ammo slot 8");
        assert!(!v.contains(&1) && !v.contains(&3), "weapon and armour are no storage");
    }

    /// Every `+0xC` (storage) of the item records `{u32 uid, u32 owner 0, u16 bound 0, u16 dict, u32 +0xC}` in the blob.
    fn storages(blob: &[u8], uid: u32, dict: u16) -> Vec<u32> {
        let key = [&uid.to_le_bytes()[..], &[0; 6], &dict.to_le_bytes()].concat();
        blob.windows(16).filter(|w| w[..12] == key[..]).map(|w| u32::from_le_bytes(w[12..16].try_into().unwrap())).collect()
    }

    /// The resupply box refills online: the stacks of the ammo slots and pockets of every present profile carry
    /// AMMO_STORAGE as `+0xC` (the
    /// spawn takes one life's load out of it, the box refills from it), for humans in PvP / PvE / capture options and
    /// for the PvP bots' kits; weapons and armour keep their records; the PvE nut keeps storage 0.
    #[test]
    fn online_ammo_and_pocket_stacks_carry_the_box_storage() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let p = online_params(&gd);
        // a common kit shape: an ammo stack and a medkit stack whose counts are exactly one life's load, and a second
        // profile with its own ammo stack
        let mut h = HumanLoadout { name: "human".into(), ..HumanLoadout::default_loadout() };
        h.items = vec![(100001, 107, 107), (100014, 81, 90), (100028, 760, 3), (100013, 104, 30), (100003, 31, 100)];
        let prof = |slots: BTreeMap<u8, u32>, rounds: BTreeMap<u8, u32>| HumanProfile { state: 0, enabled: true, id: 1, name: "p".into(), slots, rounds };
        h.profiles = vec![
            prof(BTreeMap::from([(2, 100003), (7, 100001), (8, 100014), (13, 100028)]), BTreeMap::from([(8, 90), (13, 3)])),
            prof(BTreeMap::from([(2, 100003), (7, 100001), (8, 100013)]), BTreeMap::from([(8, 30)])),
        ];
        let pl = OnlinePlayer { team: 0, human: h.clone(), hero: None, pve_budget: (0, 0) };
        let bots = pick_online_bots(&gd, [5, 6], 2);
        let blobs = [
            ("pvp", online_options(&gd, &p, std::slice::from_ref(&pl)).unwrap()),
            ("pvp + bots", online_options_bots(&gd, &p, std::slice::from_ref(&pl), &bots).unwrap()),
            ("pve", pve_options(&gd, &p, std::slice::from_ref(&pl), 1, 1, PVE_EQUIPMENT_LEVEL).unwrap()),
        ];
        for (what, o) in &blobs {
            assert_eq!(storages(o, 100014, 81), vec![AMMO_STORAGE], "{what}: ammo of profile 0");
            assert_eq!(storages(o, 100013, 104), vec![AMMO_STORAGE], "{what}: ammo of profile 1");
            assert_eq!(storages(o, 100028, 760), vec![AMMO_STORAGE], "{what}: medkit pocket");
            assert_eq!(storages(o, 100001, 107), vec![107], "{what}: the weapon record is unchanged");
            assert_eq!(storages(o, 100003, 31), vec![100], "{what}: the armour record is unchanged");
        }
        // a stack above the storage is never lowered
        let mut big = pl.clone();
        big.human.items[1].2 = AMMO_STORAGE + 5;
        assert_eq!(storages(&online_options(&gd, &p, &[big]).unwrap(), 100014, 81), vec![AMMO_STORAGE + 5]);
        // the PvE nut (slot 20, a pocket) keeps storage 0: pve.lua's pickup fills it
        let nut = pve_human(&h).items.into_iter().find(|i| i.1 == PVE_NUT_DICT).unwrap();
        assert_eq!(storages(&blobs[2].1, nut.0, PVE_NUT_DICT), vec![0], "nut storage");
        // the PvP bots: every kit's ammo and pocket records carry the storage
        let o = &blobs[1].1;
        for b in &bots {
            let kit = gd.matches.kit(&gd.items, b.profile(&gd).unwrap());
            for s in storage_slots() {
                let Some(uid) = kit.slots.get(&s) else { continue };
                let dict = kit.items.iter().find(|i| i.0 == *uid).unwrap().1;
                let got = storages(o, *uid, dict);
                assert!(!got.is_empty() && got.iter().all(|&c| c >= AMMO_STORAGE), "bot ai {} slot {s} dict {dict}: {got:?}", b.ai);
            }
        }
    }

    /// A premium ammo box in profile slot 11 reaches the match options (the inventory cook
    /// applies the slot-11 item to weapon slot 7): its item record is written and profile 0's slot mask has bit 11 with
    /// the record's index and count 1.
    #[test]
    fn an_ammo_box_in_slot_11_reaches_the_options() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let p = online_params(&gd);
        let mut h = HumanLoadout { name: "human".into(), ..HumanLoadout::default_loadout() };
        h.items = vec![(100001, 107, 107), (100014, 81, 90), (100040, 1281, 4)];
        h.profiles = vec![HumanProfile {
            state: 0,
            enabled: true,
            id: 1,
            name: "p".into(),
            slots: BTreeMap::from([(7, 100001), (8, 100014), (11, 100040)]),
            rounds: BTreeMap::from([(8, 90), (11, 1)]),
        }];
        let pl = OnlinePlayer { team: 0, human: h, hero: None, pve_budget: (0, 0) };
        let o = online_options(&gd, &p, std::slice::from_ref(&pl)).unwrap();
        assert_eq!(storages(&o, 100040, 1281).len(), 1, "the box record is in the options");
        // profile 0's slot table: the mask with bits 7, 8, 11 followed by {u16 index, u16 count} per set bit
        let mask: u32 = (1 << 7) | (1 << 8) | (1 << 11);
        let at = o.windows(4).position(|w| u32::from_le_bytes(w.try_into().unwrap()) == mask).expect("profile 0 mask");
        let entries: Vec<(u16, u16)> = (0..3).map(|k| {
            let q = at + 4 + 4 * k;
            (u16::from_le_bytes([o[q], o[q + 1]]), u16::from_le_bytes([o[q + 2], o[q + 3]]))
        }).collect();
        assert_eq!(entries[2], (2, 1), "slot 11 -> item index 2 (the box), count 1");
    }

    /// The PvE human gets the nut record (fresh uid, storage 0) in slot 20 of every present profile, once; absent
    /// profiles stay without it; the single-profile form (no profiles) gets it in its own slot table.
    #[test]
    fn pve_humans_carry_the_nut_pocket() {
        let mut h = HumanLoadout::default_loadout();
        h.items = vec![(5, 107, 107), (9, 67, 1)];
        let prof = |state| HumanProfile { state, enabled: true, id: 1, name: String::new(), slots: BTreeMap::from([(7, 5)]), rounds: BTreeMap::new() };
        h.profiles = vec![prof(0), prof(2), prof(0)];
        let p = pve_human(&h);
        assert_eq!(p.items.last(), Some(&(10, PVE_NUT_DICT, 0)), "uid after the highest, storage 0");
        for (i, q) in p.profiles.iter().enumerate() {
            let want = if q.state == 0 { Some(&10) } else { None };
            assert_eq!(q.slots.get(&PVE_NUT_SLOT), want, "profile {i}");
            assert_eq!(q.rounds.get(&PVE_NUT_SLOT).is_some(), q.state == 0);
        }
        assert_eq!(pve_human(&p), p, "applied once");
        let mut s = HumanLoadout::default_loadout();
        s.profiles.clear();
        let s2 = pve_human(&s);
        let uid = s2.slots[&PVE_NUT_SLOT];
        assert!(s2.items.contains(&(uid, PVE_NUT_DICT, 0)));
        assert_eq!(s2.slot_rounds[&PVE_NUT_SLOT], PVE_NUT_SLOT_COUNT);
    }

    fn online_params(gd: &GameData) -> OnlineParams {
        let row = gd.matches.modes().iter().find(|r| r.project.ends_with("_tdm")).expect("a tdm row").clone();
        OnlineParams { map: row.map, mode: row.mode, weather: row.variation, match_id: 0x5EED_0001, tick_ms: 50, send_period_ms: 100, match_type: 1, max_delay_ms: 500 }
    }

    /// The picks fill each team's need with distinct AI players of the nearest level, both teams with the same level
    /// mix, distinct name pairs.
    #[test]
    fn online_bots_fill_both_teams_with_distinct_ai_players() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let bots = pick_online_bots(&gd, [5, 6], 3);
        assert_eq!(bots.len(), 11);
        assert_eq!((bots.iter().filter(|b| b.team == 0).count(), bots.iter().filter(|b| b.team == 1).count()), (5, 6));
        let ais: std::collections::BTreeSet<usize> = bots.iter().map(|b| b.ai).collect();
        assert_eq!(ais.len(), 11, "each AI player once");
        assert!(ais.iter().all(|&a| a < gd.matches.ai_players().len()));
        let lvl = |b: &OnlineBot| gd.matches.ai_players()[b.ai].match_making_level;
        assert!(bots.iter().all(|b| lvl(b).abs_diff(3) <= 1), "level 3 has 12 AI players: 11 stay within one level");
        let names: std::collections::BTreeSet<(u8, u8)> = bots.iter().map(|b| (b.first, b.second)).collect();
        assert_eq!(names.len(), 11);
        // a solo human at level 5: levels 5 (4 players) then 4
        let top = pick_online_bots(&gd, [5, 6], 5);
        assert!(top.iter().all(|b| lvl(b) >= 4));
        assert_eq!(top.iter().filter(|b| lvl(b) == 5).count(), 4);
    }

    /// PvP bot options: humans first, then the AI records with their team, ai id and synthetic account id; n_players counts
    /// both; an AI player twice, an ai id outside the list or no human are refused.
    #[test]
    fn online_bot_options_carry_the_ai_records_after_the_humans() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let p = online_params(&gd);
        let human = OnlinePlayer { team: 0, human: HumanLoadout { name: "solo".into(), ..HumanLoadout::default_loadout() }, hero: None, pve_budget: (0, 0) };
        let bots = pick_online_bots(&gd, [5, 6], 2);
        let o = online_options_bots(&gd, &p, std::slice::from_ref(&human), &bots).unwrap();
        assert_eq!(o[5], 12, "n_players = 1 + 11");
        assert_eq!(o[11], 0, "single_player 0");
        let at = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).position(|w| w == needle);
        for (k, b) in bots.iter().enumerate() {
            let name = b.name(&gd, k);
            let id = ((0xB07u64 << 32) | (k as u64 + 1)).to_le_bytes();
            let mut key = vec![name.len() as u8];
            key.extend_from_slice(name.as_bytes());
            key.push(0); // empty clan
            key.extend_from_slice(&id);
            let pos = at(&o, &key).unwrap_or_else(|| panic!("bot {k} record {name}"));
            assert_eq!(o[pos - 1], b.team, "team byte before the name");
            let ai = u32::from_le_bytes(o[pos + key.len() + 8..pos + key.len() + 12].try_into().unwrap());
            assert_eq!(ai as usize, b.ai, "ai id after games / +0x2DC");
        }
        let without = online_options(&gd, &p, std::slice::from_ref(&human)).unwrap();
        assert_eq!(without[5], 1);
        let mut twice = bots.clone();
        twice[1].ai = twice[0].ai;
        assert!(online_options_bots(&gd, &p, std::slice::from_ref(&human), &twice).is_err());
        let mut outside = bots.clone();
        outside[0].ai = gd.matches.ai_players().len();
        assert!(online_options_bots(&gd, &p, std::slice::from_ref(&human), &outside).is_err());
        assert!(online_options_bots(&gd, &p, &[], &bots).is_err(), "at least one human");
    }

    /// Header of the options: `u16 send_period, u16, u8 tick_ms, u8 n_players, u32 match_id, u8 packed_inputs, ...`.
    #[test]
    fn header_tick_and_plain_inputs() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let human = HumanLoadout::default_loadout();
        let o = tutorial_options(&gd, &human, 7).unwrap();
        assert_eq!(u16::from_le_bytes([o[0], o[1]]), SEND_PERIOD_MS);
        assert_eq!(o[4], 50, "tick_ms");
        assert_eq!(o[10], 0, "packed_inputs = 0");
    }

    /// The shooting range is the same blob layout on its own level: project `shooting_range`, map
    /// `shooting_range/test`, the human and eight AI players, all on team 1.
    #[test]
    fn shooting_range_is_one_human_on_its_own_level() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let human = HumanLoadout::default_loadout();
        let (o, roster) = shooting_range_build(&gd, &human, 7).unwrap();
        assert_eq!(o, shooting_range_options(&gd, &human, 7).unwrap());
        assert_eq!(o[5], 9, "the human and eight bots");
        assert_eq!(o[11], 1, "single_player");
        let at = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).position(|w| w == needle);
        assert!(at(&o, b"\x0eshooting_range\x13shooting_range/test").is_some(), "length-prefixed strings");
        // eight AI records, all on the level's bot team 1, distinct AI ids; the 100-byte zero trailer ends the blob
        assert_eq!(roster.bots.len(), 8);
        assert!(roster.bots.iter().all(|b| b.team == 1), "{:?}", roster.bots);
        let mut ids: Vec<u32> = roster.bots.iter().map(|b| b.ai).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 8);
        assert!(o.ends_with(&[0u8; 100]));
        assert_eq!(crate::pack_5b(3, &o)[0], 3);
    }

    /// The tutorial: ten players; the human first on team 0 with the four kit profiles,
    /// all present, the start kit `tutorial_player_3` at index 3; then the nine script bots on team 1 named as in the
    /// script, with distinct AI ids, their face (`head`), and the kits' modules installed (PPSh, Mosin).
    #[test]
    fn tutorial_has_four_kit_profiles_and_the_script_bots() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let human = HumanLoadout { face: 5, name: "rookie".into(), ..HumanLoadout::default_loadout() };
        let o = tutorial_options(&gd, &human, 7).unwrap();
        assert_eq!(o[5], 10, "n_players");
        assert_eq!(o[11], 1, "single_player");
        let at = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).position(|w| w == needle);
        assert!(at(&o, b"\x08tutorial\x10tutorial/default").is_some());
        // the human's profiles: four, ids 1..4, present, the kits' weapons in script order
        let h = tutorial_human(&gd, &human);
        assert_eq!(h.profiles.len(), 4);
        assert!(h.profiles.iter().all(|p| p.state == 0 && p.enabled));
        assert_eq!(h.profiles.iter().map(|p| p.id).collect::<Vec<_>>(), [1, 2, 3, 4]);
        let dict = |iid: u32| h.items.iter().find(|x| x.0 == iid).unwrap().1;
        let weapon = |i: usize| dict(h.profiles[i].slots[&7]);
        let by = |n: &str| gd.items.by_cfg_name(n).unwrap();
        assert_eq!([weapon(0), weapon(1), weapon(2), weapon(3)], [by("toz_66"), by("ppsh"), by("mosin_nagant"), by("tt_33")]);
        // every slot names an item of the shared list; the PPSh carries its mag and collimator, the Mosin its scope
        assert!(h.profiles.iter().all(|p| p.slots.values().all(|u| h.items.iter().any(|i| i.0 == *u))));
        let installed = |i: usize| {
            h.attach.iter().find(|(w, _)| *w == h.profiles[i].slots[&7]).map(|(_, l)| l.iter().map(|m| (m.0.clone(), dict(m.1))).collect::<Vec<_>>()).unwrap_or_default()
        };
        assert_eq!(installed(1), [("mag".to_string(), by("ppsh_762x25_30_round_mag")), ("scope_picatinnyrail".to_string(), by("eotech_552_collimator"))]);
        assert_eq!(installed(2), [("mosin_scope".to_string(), by("pu_35x22_scope"))]);
        assert!(installed(0).is_empty() && installed(3).is_empty());
        // counted slots carry the kit's amount_per_spawn (start kit: 200 pistol rounds, 1 medkit)
        assert_eq!((h.profiles[3].rounds[&8], h.profiles[3].rounds[&13]), (200, 1));
        // the account's name and face stay, its own items and workshop state do not
        assert_eq!((h.name.as_str(), h.face), ("rookie", 5));
        assert!(h.upgrades.is_empty() && h.bound.is_empty() && h.decals.is_empty());
        // the bots in script order after the human, each record named as the script binds it
        let mut last = at(&o, b"\x06rookie").unwrap();
        let bots = &gd.matches.tutorial().bots;
        assert_eq!(bots.len(), 9);
        for (k, b) in bots.iter().enumerate() {
            let mut key = vec![b.name.len() as u8];
            key.extend(b.name.as_bytes());
            let p = at(&o[last..], &key).unwrap_or_else(|| panic!("bot {k} {}", b.name)) + last;
            // u8 team, str name, str clan (empty), 8-byte id, u32 games, u32, u32 ai id, u8, u8 face
            assert_eq!(o[p - 1], 1, "team of {}", b.name);
            let r = p + key.len() + 1 + 8 + 8;
            assert_eq!(u32::from_le_bytes(o[r..r + 4].try_into().unwrap()), k as u32, "ai id of {}", b.name);
            assert_eq!(o[r + 5], b.head, "face of {}", b.name);
            last = p;
        }
        assert_eq!(crate::pack_5b(2, &o)[0], 2);
    }

    /// Online PvE options: the humans first (team 0, ai id -1, the data's respawn price,
    /// key and gold budgets 0), then pve_bot_1..17 on team 1 with ai ids 0..16, their faces, two present kit profiles each
    /// and the difficulty's hit values in the AI tail; n_players = humans + 17; a fourth human does not fit.
    #[test]
    fn pve_options_carry_the_humans_and_the_17_script_bots() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let p = OnlineParams { map: 15, mode: 7, weather: 1, match_id: 0x0BAD_F00D, tick_ms: 50, send_period_ms: 100, match_type: 0, max_delay_ms: max_delay_ms(50) };
        let mk = |n: &str| OnlinePlayer { team: 0, human: HumanLoadout { name: n.into(), ..HumanLoadout::default_loadout() }, hero: None, pve_budget: (0, 0) };
        let o = pve_options(&gd, &p, &[mk("alpha"), mk("bravo")], 1, 2, PVE_EQUIPMENT_LEVEL).unwrap();
        assert_eq!((o[5], o[11], o[14], o[15]), (19, 0, PVE_EQUIPMENT_LEVEL, 1), "n_players, single_player, +0x1081, +0x1082");
        assert_eq!(u32::from_le_bytes(o[6..10].try_into().unwrap()), 0x0BAD_F00D);
        let at = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).position(|w| w == needle);
        assert!(at(&o, b"pve_01_v2_maze2").is_some());
        // a human record: team, name, clan, id, games, u32, ai id, 3 bytes, then the keys / gold / respawn price
        let h = at(&o, b"\x05alpha").unwrap();
        let r = h + 6 + 1 + 8 + 8;
        assert_eq!(u32::from_le_bytes(o[r..r + 4].try_into().unwrap()), u32::MAX, "human ai id");
        let b = r + 4 + 3;
        assert_eq!(&o[b..b + 8], &[0u8; 8], "no key / gold budget");
        assert_eq!(u16::from_le_bytes([o[b + 8], o[b + 9]]), gd.pve.prices.respawn_price as u16);
        // with a budget (spendable keys after the entry fee, the gold balance): +0x2D0 keys,
        // +0x2D4 gold, +0x2F8 the price; the same record layout otherwise
        let rich = OnlinePlayer { pve_budget: (3, 120), ..mk("alpha") };
        let o2 = pve_options(&gd, &p, &[rich, mk("bravo")], 1, 2, PVE_EQUIPMENT_LEVEL).unwrap();
        assert_eq!(o2.len(), o.len());
        assert_eq!(u32::from_le_bytes(o2[b..b + 4].try_into().unwrap()), 3, "+0x2D0 access keys");
        assert_eq!(u32::from_le_bytes(o2[b + 4..b + 8].try_into().unwrap()), 120, "+0x2D4 gold");
        assert_eq!(u16::from_le_bytes([o2[b + 8], o2[b + 9]]), gd.pve.prices.respawn_price as u16);
        let b2 = at(&o2, b"\x05bravo").unwrap() + 6 + 1 + 8 + 8 + 4 + 3;
        assert_eq!(&o2[b2..b2 + 8], &[0u8; 8], "bravo has no budget");
        let mut last = at(&o, b"\x05bravo").unwrap();
        let bots = gd.pve.bots(1);
        for (k, bot) in bots.iter().enumerate() {
            let mut key = vec![bot.name.len() as u8];
            key.extend(bot.name.as_bytes());
            key.push(0); // empty clan
            let p = at(&o[last..], &key).unwrap_or_else(|| panic!("bot {k}")) + last;
            assert_eq!(o[p - 1], 1, "team of {}", bot.name);
            let r = p + key.len() + 8 + 8;
            assert_eq!(u32::from_le_bytes(o[r..r + 4].try_into().unwrap()), k as u32, "ai id of {}", bot.name);
            assert_eq!(o[r + 5], bot.head, "face of {}", bot.name);
            // two profiles (state 0, enabled, ids 1 / 2) after the 10 budget bytes
            assert_eq!(o[r + 4 + 3 + 10], 2, "profiles of {}", bot.name);
            last = p;
        }
        // the AI tail of the last bot: hit damage multiplier (f32), hit chance, forgive headshots
        let tail = [1.0f32.to_le_bytes().as_slice(), &[75, 1]].concat();
        assert!(at(&o[last..], &tail).is_some(), "medium hit values");
        let four: Vec<OnlinePlayer> = ["a", "b", "c", "d"].iter().map(|n| mk(n)).collect();
        assert!(pve_options(&gd, &p, &four, 1, 2, 1).is_err(), "4 + 17 > 20");
    }

    /// Booster modifiers go into the human's profile modifier block: bit mask (bit i of byte i>>3 = id i, 18 bytes)
    /// then one f32 per set bit in id order; everything else is byte for byte the options without boosters.
    #[test]
    fn booster_modifiers_fill_the_profile_block() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let human = HumanLoadout::default_loadout();
        let base = shooting_range_options(&gd, &human, 7).unwrap();
        let mut boosted = human.clone();
        boosted.extra_modifiers = BTreeMap::from([(68, 0.25), (122, 0.05), (54, -0.30)]);
        let o = shooting_range_options(&gd, &boosted, 7).unwrap();
        assert_eq!(o.len(), base.len() + 12);
        let d = (0..base.len()).find(|&i| base[i] != o[i]).unwrap();
        let start = d - 6; // first set bit: id 54 = byte 6
        assert!(base[start..start + 18].iter().all(|&b| b == 0), "empty mask without boosters");
        let mut mask = [0u8; 18];
        for id in [68usize, 122, 54] {
            mask[id >> 3] |= 1 << (id & 7);
        }
        assert_eq!(&o[start..start + 18], &mask);
        let mut floats = Vec::new();
        for v in [-0.30f32, 0.25, 0.05] {
            floats.extend(v.to_le_bytes());
        }
        assert_eq!(&o[start + 18..start + 30], &floats[..]);
        assert_eq!(&o[..start], &base[..start]);
        assert_eq!(&o[start + 30..], &base[start + 18..]);
        // an empty map gives the bytes without boosters
        let mut none = human.clone();
        none.extra_modifiers = BTreeMap::new();
        assert_eq!(shooting_range_options(&gd, &none, 7).unwrap(), base);
    }

    /// The booster modifiers of an online player go into its own modifier block, and only into its own.
    #[test]
    fn online_booster_modifiers_fill_only_their_players_block() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let row = gd.matches.modes().iter().find(|r| r.min_players > 0).unwrap().clone();
        let p = OnlineParams { map: row.map, mode: row.mode, weather: row.variation, match_id: 7, tick_ms: 50, send_period_ms: 100, match_type: 0, max_delay_ms: max_delay_ms(50) };
        let plain = |n: &str, team: u8| OnlinePlayer { team, human: HumanLoadout { name: n.into(), ..HumanLoadout::default_loadout() }, hero: None, pve_budget: (0, 0) };
        let base = online_options(&gd, &p, &[plain("alpha", 0), plain("beta", 1)]).unwrap();
        let mut boosted = plain("beta", 1);
        boosted.human.extra_modifiers = BTreeMap::from([(68, 0.25), (54, -0.30)]);
        let o = online_options(&gd, &p, &[plain("alpha", 0), boosted]).unwrap();
        // two ids set in the 18-byte mask, two f32
        assert_eq!(o.len(), base.len() + 8);
        let d = (0..base.len()).find(|&i| base[i] != o[i]).unwrap();
        // the first difference is the id-54 bit (byte 6 of the mask) of the second player's block: everything before is the
        // first player and the header, byte for byte
        let start = d - 6;
        assert!(base[start..start + 18].iter().all(|&b| b == 0));
        let mut mask = [0u8; 18];
        for id in [68usize, 54] {
            mask[id >> 3] |= 1 << (id & 7);
        }
        assert_eq!(&o[start..start + 18], &mask);
        assert_eq!(&o[start + 18..start + 26], &[(-0.30f32).to_le_bytes(), 0.25f32.to_le_bytes()].concat()[..]);
        assert_eq!(&o[start + 26..], &base[start + 18..]);
    }

    #[test]
    fn online_header_roster_and_seed() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let row = gd.matches.modes().iter().find(|r| r.min_players > 0).unwrap().clone();
        let mk = |n: &str, team: u8| OnlinePlayer { team, human: HumanLoadout { name: n.into(), ..HumanLoadout::default_loadout() }, hero: None, pve_budget: (0, 0) };
        let p = OnlineParams { map: row.map, mode: row.mode, weather: row.variation, match_id: 0xABCD_0123, tick_ms: 50, send_period_ms: 100, match_type: 0, max_delay_ms: max_delay_ms(50) };
        let o = online_options(&gd, &p, &[mk("alpha", 0), mk("beta", 1)]).unwrap();
        assert_eq!(u16::from_le_bytes([o[0], o[1]]), 100, "send period");
        assert_eq!(o[4], 50, "tick_ms");
        assert_eq!(o[5], 2, "two players");
        assert_eq!(u32::from_le_bytes([o[6], o[7], o[8], o[9]]), 0xABCD_0123, "match id = seed");
        assert_eq!(o[10], 0, "packed_inputs = 0");
        assert_eq!(o[11], 0, "single_player = 0 online");
        assert_eq!(o[13], MATCH_TYPE_PLAIN, "match type (+0x107D) of a plain match");
        // the 4th flag byte (+0x107D) carries the match type; nothing else changes
        let clan = online_options(&gd, &OnlineParams { match_type: MATCH_TYPE_CLAN, ..p }, &[mk("alpha", 0), mk("beta", 1)]).unwrap();
        assert_eq!(clan[13], 6);
        assert_eq!((&clan[..13], &clan[14..]), (&o[..13], &o[14..]));
        // the last table: {hero_place, nomination} of player i at 2i (scoreboard icon); nothing else changes
        let mut titled = mk("beta", 1);
        titled.hero = Some((1, 2));
        let h = online_options(&gd, &p, &[mk("alpha", 0), titled]).unwrap();
        assert_eq!(h.len(), o.len());
        let table = h.len() - 40;
        assert_eq!(&h[table..table + 4], &[0, 0, 1, 2]);
        assert_eq!((&h[..table + 2], &h[table + 4..]), (&o[..table + 2], &o[table + 4..]));
        // an item's decals go into its record (n3: u32 uid, u16 dict, 8 bytes, u8 slot): 15 more bytes, nothing else
        let mut decorated = mk("alpha", 0);
        let item = decorated.human.items[0].0;
        decorated.human.decals.insert(item, vec![(77, 718, 0x0102_0304_0506_0708, 1)]);
        let d = online_options(&gd, &p, &[decorated, mk("beta", 1)]).unwrap();
        assert_eq!(d.len(), o.len() + 15);
        let entry = [&77u32.to_le_bytes()[..], &718u16.to_le_bytes(), &0x0102_0304_0506_0708u64.to_le_bytes(), &[1]].concat();
        let at = d.windows(15).position(|w| w == entry.as_slice()).expect("the decal entry");
        assert_eq!(d[at - 1], 1, "n3 = 1");
        assert_eq!((&d[..at - 1], &d[at + 15..]), (&o[..at - 1], &o[at..]));
        // both names are in the blob, in roster order
        let at = |needle: &[u8]| o.windows(needle.len()).position(|w| w == needle);
        assert!(at(b"alpha").unwrap() < at(b"beta").unwrap());
        // zero ticks are refused (the client divides by tick_ms), so is an empty roster
        assert!(online_options(&gd, &OnlineParams { tick_ms: 0, ..p }, &[mk("a", 0)]).is_err());
        assert!(online_options(&gd, &p, &[]).is_err());
    }
}
