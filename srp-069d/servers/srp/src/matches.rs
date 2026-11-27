//! `LocalMatches` over `crates/local-match`: Training-with-bots and tutorial options built from the account's profile,
//! results (97) and rewards of a finished match. The roster of a running match is kept per account name between
//! `training` / `tutorial` and `finished` (one local match per account at a time).

use std::collections::HashMap;
use std::sync::Mutex;

use accounts::public_id;
use lobby_server::{LocalMatches, MatchCtx, MatchOutcome};
use local_match::{HumanLoadout, HumanProgress, Roster, TrainingParams};

#[derive(Default)]
pub struct Matches {
    rosters: Mutex<HashMap<String, Roster>>,
    /// Fixed match id (tests); None = the clock.
    pub fixed_id: Option<u32>,
}

impl Matches {
    /// Matches with a fixed match id (reproducible bytes).
    pub fn with_fixed_id(id: u32) -> Matches {
        Matches { fixed_id: Some(id), ..Default::default() }
    }

    fn match_id(&self) -> u32 {
        self.fixed_id.unwrap_or_else(local_match::fresh_match_id)
    }
}

/// The account's equipment profiles for a match, in lobby tab order: a profile the account can play
/// (bought / premium and enabled) has state 0, the others state 2 with no slot table. Local matches always spawn
/// profile index 0 and the online start is the lowest usable index, so when tab 0 is not playable the current profile
/// is moved to the front (index 0 must exist or the spawn reads a null profile slot, 0x2002F11F4). The order and the
/// states are `lobby_server::profiles::options_order` (the lobby maps the world host's `Used` bits with it).
pub fn match_profiles(p: &accounts::Profile) -> Vec<local_match::HumanProfile> {
    let now = lobby_server::premium::unix_now();
    lobby_server::profiles::options_order(p, now)
        .into_iter()
        .map(|(i, playable)| {
            let rounds = if i == p.equip.current { p.slot_rounds.clone() } else { p.equip.profiles[i as usize].rounds.clone() };
            let e = &p.equip.profiles[i as usize];
            local_match::HumanProfile {
                state: if playable { 0 } else { 2 },
                enabled: e.enabled,
                id: lobby_server::build::wire_profile_id(i),
                name: if e.name.is_empty() { format!("Profile {}", i + 1) } else { e.name.clone() },
                slots: if playable { p.profile_slots(i).clone() } else { Default::default() },
                rounds: if playable { rounds } else { Default::default() },
            }
        })
        .collect()
}

/// The human of a match as the account's profile has him: items, slots, loaded rounds, workshop upgrades, installed
/// modules and bindings, learned skills (`skill_<id>` -> points, turned into the modifier mask by the options builder).
pub fn human_loadout(ctx: &MatchCtx) -> HumanLoadout {
    let p = ctx.profile;
    HumanLoadout {
        name: ctx.account_name.to_string(),
        clan: String::new(),
        account_id: public_id(ctx.account_id).to_le_bytes(),
        items: p.items.iter().map(|i| (i.uid, i.dict_id, i.count)).collect(),
        slots: p.slots.clone(),
        slot_rounds: p.slot_rounds.iter().map(|(s, r)| (*s, *r)).collect(),
        skills: game_data::Skills::match_keys(&p.skills),
        upgrades: p.mods.clone(),
        attach: p.attach.iter().map(|(w, sl)| (*w, sl.iter().map(|(n, m)| (n.clone(), *m)).collect())).collect(),
        removed_defaults: p.removed_defaults.iter().map(|(w, s)| (*w, s.iter().cloned().collect())).collect(),
        bound: p.bound.clone(),
        extra_modifiers: lobby_server::boosters::match_modifier_ids(ctx.data, p),
        // the stored decals of every item (89 carries the same rows); the match draws them from the options (LIKELY)
        decals: p
            .decals
            .iter()
            .map(|(uid, l)| (*uid, l.iter().map(|d| (d.decal_uid, d.dict, d.x_bits, d.slot)).collect()))
            .collect(),
        // record +0x2D8 = finished matches (PROVEN, 0x2003EA53D: below `damage_protector` 3 the player takes 15 / 10 / 5 %
        // less toxic, radiation and shock damage)
        games_played: p.matches,
        face: p.face,
        premium: p.premium_left(lobby_server::premium::unix_now()) > 0,
        profiles: match_profiles(p),
    }
}

/// The kit of the shooting range: the account's own loadout with the weapon to test in the weapon slot of its kind
/// (CHOICE; the client names only the weapon, the exe sends no slot): pistols (`wpn_pstl`) replace slot 10, every
/// other weapon slot 7, ammo slots 9 / 8. An owned weapon keeps its saved upgrades and modules; a weapon the account
/// does not own (the shop's test button) is a temporary item with the modules of the request record. If the ammo in the
/// slot does not fit the weapon, the first ammo type of the game data's compatibility list replaces it. Nothing is
/// saved: the range kit never touches the profile.
pub fn range_loadout(ctx: &MatchCtx, weapon: &protocol::items::ItemRecord) -> HumanLoadout {
    let mut h = human_loadout(ctx);
    // the range kit is one profile (the weapon to test in its slot)
    h.profiles.clear();
    let gd = ctx.data;
    let dict = weapon.dict;
    if !gd.items.is_weapon(dict) {
        return h;
    }
    let pistol = gd.items.get(dict).is_some_and(|i| i.kind == "wpn_pstl");
    let (slot, ammo_slot) = if pistol { (10u8, 9u8) } else { (7, 8) };
    let mut next_uid = h.items.iter().map(|i| i.0 + 1).max().unwrap_or(accounts::UID_BASE).max(accounts::UID_BASE);
    let mut temp_uid = |h: &mut HumanLoadout, dict: u16, count: u32| {
        let uid = next_uid;
        next_uid += 1;
        h.items.push((uid, dict, count));
        uid
    };
    let owned = h.items.iter().find(|i| i.0 == weapon.uid && i.1 == dict).or_else(|| h.items.iter().find(|i| i.1 == dict)).map(|i| i.0);
    let uid = match owned {
        Some(uid) => uid,
        None => {
            let uid = temp_uid(&mut h, dict, gd.items.amount_field(dict, 1));
            if !weapon.mods.is_empty() {
                h.upgrades.insert(uid, weapon.mods.clone());
            }
            let mut installed = vec![];
            for m in weapon.modules.iter().filter(|m| gd.items.is_module(m.dict) && gd.modules.slot_valid(dict, &m.slot)) {
                let muid = match h.items.iter().find(|i| i.0 == m.uid && i.1 == m.dict) {
                    Some(i) => i.0,
                    None => temp_uid(&mut h, m.dict, gd.items.amount_field(m.dict, 1)),
                };
                if !m.mods.is_empty() {
                    h.upgrades.insert(muid, m.mods.clone());
                }
                installed.push((m.slot.clone(), muid));
            }
            if !installed.is_empty() {
                h.attach.push((uid, installed));
            }
            uid
        }
    };
    h.slots.insert(slot, uid);
    let compat = gd.modules.compatible_ammo(dict);
    let loaded_fits = h.slots.get(&ammo_slot).and_then(|u| h.items.iter().find(|i| i.0 == *u)).is_some_and(|i| compat.is_empty() || compat.contains(&i.1));
    if !loaded_fits {
        if let Some(&ammo) = compat.first() {
            let auid = match h.items.iter().find(|i| i.1 == ammo) {
                Some(i) => i.0,
                None => temp_uid(&mut h, ammo, 0),
            };
            h.slots.insert(ammo_slot, auid);
            // a full magazine: the data's clip size (CHOICE; the match refills from its ammo storage)
            h.slot_rounds.insert(ammo_slot, gd.shop.clip(ammo));
        }
    }
    h
}

impl LocalMatches for Matches {
    fn shooting_range(&self, ctx: &MatchCtx, weapon: &protocol::items::ItemRecord, _profile: u8) -> Vec<u8> {
        let human = range_loadout(ctx, weapon);
        self.rosters.lock().unwrap().remove(ctx.account_name);
        match local_match::shooting_range_options(ctx.data, &human, self.match_id()) {
            Ok(opts) => opts,
            Err(e) => {
                log::warn!("{}: shooting range options: {e}; minimal blob", ctx.account_name);
                protocol::s2c::PlayBlob::minimal_blob()
            }
        }
    }

    fn training(&self, ctx: &MatchCtx, map: u32, mode: u32, weather: u32) -> Vec<u8> {
        let human = human_loadout(ctx);
        let params = TrainingParams::new(map, mode, weather, self.match_id());
        match local_match::training_options(ctx.data, &params, &human) {
            Ok((opts, roster)) => {
                self.rosters.lock().unwrap().insert(ctx.account_name.to_string(), roster);
                opts
            }
            Err(e) => {
                log::warn!("{}: training options for map {map} mode {mode}: {e}; minimal blob", ctx.account_name);
                self.rosters.lock().unwrap().remove(ctx.account_name);
                protocol::s2c::PlayBlob::minimal_blob()
            }
        }
    }

    fn tutorial(&self, ctx: &MatchCtx) -> Vec<u8> {
        let human = human_loadout(ctx);
        let id = self.match_id();
        match local_match::tutorial_options(ctx.data, &human, id) {
            Ok(opts) => {
                let roster = Roster { match_id: id, map_id: 0, mode_id: 1, human_name: ctx.account_name.to_string(), bots: vec![] };
                self.rosters.lock().unwrap().insert(ctx.account_name.to_string(), roster);
                opts
            }
            Err(e) => {
                log::warn!("{}: tutorial options: {e}; minimal blob", ctx.account_name);
                self.rosters.lock().unwrap().remove(ctx.account_name);
                protocol::s2c::PlayBlob::minimal_blob()
            }
        }
    }

    fn finished(&self, ctx: &MatchCtx, _map: u32, _mode: u32, won: bool, finished: bool, events: &[u16]) -> MatchOutcome {
        let roster = self.rosters.lock().unwrap().remove(ctx.account_name);
        let Some(roster) = roster else {
            return lobby_server::local::outcome_from_data(ctx, _map, _mode, won, finished);
        };
        let p = ctx.profile;
        let progress = HumanProgress { xp: p.xp, faction_points: p.points(p.faction), faction: p.faction, first_game: !p.claimed(accounts::CLAIM_FIRST_GAME) };
        // an active premium account adds its bonus to the paid and the shown amounts
        // (the premium account's +50 % and the boosters the match carried: charged at its start, still the selected ones)
        let carried = lobby_server::boosters::selected(ctx.data, p);
        let bonus = lobby_server::boosters::reward_percent(ctx.data, p, lobby_server::premium::unix_now(), &carried);
        match local_match::results_with_bonuses(ctx.data, &roster, events, &progress, roster.match_id, bonus) {
            Ok((bytes, r)) => MatchOutcome { results97: finished.then_some(bytes), exp: r.exp, silver: r.silver, reputation: r.rep, first_game: progress.first_game && finished && game_data::rewards::LOCAL_MATCH_PAYS },
            Err(e) => {
                log::warn!("{}: results: {e}", ctx.account_name);
                lobby_server::local::outcome_from_data(ctx, _map, _mode, won, finished)
            }
        }
    }
}
