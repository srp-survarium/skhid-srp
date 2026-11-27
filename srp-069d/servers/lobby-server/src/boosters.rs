//! Boosters (docs/notes/store.md section 4): the lobby panel's selection 45 0C / 0D / 0E, autobuy 48 29, the 94
//! records, the charge per match and the reward bonus of the experience / silver / reputation boosters.
//!
//! What the client proves:
//! * Every booster (dictionary category 23) is a PER-MATCH booster, the gold ones too: three silver slots (339..344,
//!   `use_variable_costs`) and three gold slots (8A kind 1: 853 / 857 / 861, "for one match" by their descriptions).
//!   Every selected booster without stock is a "Match costs" item (0x2001BF460, 0x2001BFBC0): 5 gold for 853. PROVEN.
//! * Clicking a selected slot unselects it (45 0D); 45 0E replaces, 45 0C fills an empty slot. The slots follow only the
//!   94 records (0x2001C50A0) and the client ignores `83 45 0C..0E` (0x2001901B0), so every request is answered with the
//!   94 records as they are now. PROVEN.
//! * The cost shown per booster (0x2001C5191.., 0x2001BF5B0..): none while the 89 inventory holds the item (stock first);
//!   `use_variable_costs` items `booster_use_costs[row][n]`, `n` = the variable-cost boosters before it that have no
//!   stock (0x2001BF701); the others their 8A price in their 8A currency (853: 5 gold). PROVEN.
//! * 94 record type 0x23 = a selected booster, `f40` = its dict id; at most three per slot group. PROVEN.
//! * Service boosters (PROVEN, 0x200181520 / 0x2001C5191): the 94 record type IS the item's `service_id` (0x18 =
//!   24 for `new_player_booster` 1088, the only one), `f40` = one of its `default_modifications` as `group << 16 | grade`
//!   (the client finds the item by category 27, `voucher_type` 4, service id and that pair), `f44` = the counter shown
//!   (matches left, LIKELY). CHOICE: a new account gets 1088 x `service_duration_matches` (3) in its
//!   inventory (`progress::new_player`); each ONLINE match uses one unit ([`use_services`], not local or PvE matches,
//!   like the per-match boosters) and carries its modifier (id 76, -0.15: anomaly hits reduced, the HUD's blessing info)
//!   in the player's options block ([`service_modifier_ids`]).
//! The charge happens when a match starts, with the costs the client shows (stock first; a booster the account cannot
//! pay is switched off). The selection stays for the next match (the client keeps showing it in the panel and in
//! "Match costs"). LIKELY: autobuy only stores the flag.
//! Effects: the experience / money / reputation modifiers (+50 %) of 853 / 857 / 861 add to the premium bonus of the
//! match they were paid for ([`reward_percent`]). The six other modifiers (weight, speed, regeneration, respawn,
//! aiming, artefact) are match-side: the match options' profile carries one modifier block per player (bit mask + floats,
//! reader 0x200125680), the boosters are added to it ([`match_modifier_ids`], `HumanLoadout::extra_modifiers`).

use accounts::{Currency, Profile};
use game_data::vouchers::MAX_MATCH_BOOSTERS;
use game_data::GameData;
use protocol::s2c::*;

use crate::dispatch::{update, Ctx};
use crate::peer::Peer;
use crate::premium::push_status;
use crate::shop::send_money;

/// 94 record type of a selected booster.
pub const KIND_BOOSTER: u8 = 0x23;

/// The reward modifiers of 853 / 857 / 861 (`[experience, silver, reputation]`): applied to the match's pay, not sent
/// to the match.
const REWARD_MODIFIERS: [&str; 3] = ["experience_bonus_modifier", "money_bonus_modifier", "reputation_bonus_modifier"];

/// The 94 records of the boosters the profile has selected (dict id order, the order the client prices them in), then
/// the service boosters in stock.
pub fn records(gd: &GameData, p: &Profile) -> Vec<Notice94> {
    let mut out: Vec<Notice94> = selected(gd, p).into_iter().map(|d| Notice94 { f40: d as u32, kind: KIND_BOOSTER, ..Default::default() }).collect();
    // service boosters in stock: type = service id, f40 = the (group, grade) pair the client matches, f44 = matches left
    for row in gd.vouchers.service_boosters() {
        let left: u32 = p.items.iter().filter(|i| i.dict_id == row.dict_id).map(|i| i.count).sum();
        let (Some(kind), Some(&(group, grade))) = (row.service_id.and_then(|s| u8::try_from(s).ok()), row.default_modifications.first()) else { continue };
        if left > 0 {
            out.push(Notice94 { f40: group << 16 | grade, f44: left, kind, ..Default::default() });
        }
    }
    out
}

/// The service boosters the profile has in stock (dict ids).
fn services_in_stock(gd: &GameData, p: &Profile) -> Vec<u16> {
    gd.vouchers.service_boosters().map(|r| r.dict_id).filter(|d| p.items.iter().any(|i| i.dict_id == *d && i.count > 0)).collect()
}

/// An online match starts: one unit of every service booster in stock is used. Returns the stock rows (uids) that went
/// down (the client lowers the stack: `83 45 03 <uid>`).
pub fn use_services(gd: &GameData, p: &mut Profile) -> Vec<u32> {
    services_in_stock(gd, p).into_iter().filter_map(|d| take_stock(p, d)).collect()
}

/// The match modifiers of the service boosters in stock, as client modifier ids (like [`match_modifier_ids`]): what an
/// online match adds to the player's options block (`new_player_damage_modifier` = id 76 for 1088).
pub fn service_modifier_ids(gd: &GameData, p: &Profile) -> std::collections::BTreeMap<u32, f64> {
    let names = gd.matches.modifier_names();
    let mut out = std::collections::BTreeMap::new();
    for d in services_in_stock(gd, p) {
        for m in gd.vouchers.get(d).map(|r| r.modifiers.as_slice()).unwrap_or(&[]) {
            if let Some(i) = names.iter().position(|n| *n == m.name) {
                *out.entry(i as u32).or_insert(0.0) += m.value;
            }
        }
    }
    out
}

/// Remove one unit of `dict` from the stock; the row's uid (the client lowers the stack on `83 45 03 <uid>`).
fn take_stock(p: &mut Profile, dict: u16) -> Option<u32> {
    let uid = p.items.iter().find(|i| i.dict_id == dict && i.count > 0)?.uid;
    if p.item(uid)?.count > 1 {
        p.item_mut(uid)?.count -= 1;
    } else {
        p.remove_item(uid);
    }
    Some(uid)
}

/// The boosters the profile has selected in the panel (dict id order).
pub fn selected(gd: &GameData, p: &Profile) -> Vec<u16> {
    p.boosters.iter().filter(|(d, b)| b.enabled && gd.vouchers.is_match_booster(**d)).map(|(d, _)| *d).collect()
}

/// Why a request was refused (logged; the client gets the unchanged 94 records).
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    NotABooster,
    AlreadyOn,
    TooMany,
    NotOn,
}

/// Select booster `dict` for the next matches (nothing is paid now: [`charge_profile`] charges it when a match starts).
/// At most three per slot group (silver / gold).
pub fn switch_on(gd: &GameData, p: &mut Profile, dict: u16) -> Result<(), Refused> {
    if !gd.vouchers.is_match_booster(dict) {
        return Err(Refused::NotABooster);
    }
    if p.boosters.get(&dict).is_some_and(|b| b.enabled) {
        return Err(Refused::AlreadyOn);
    }
    let gold = gd.vouchers.is_gold_booster(dict);
    if selected(gd, p).into_iter().filter(|d| gd.vouchers.is_gold_booster(*d) == gold).count() >= MAX_MATCH_BOOSTERS {
        return Err(Refused::TooMany);
    }
    p.boosters.entry(dict).or_default().enabled = true;
    Ok(())
}

/// Unselect booster `dict` (any booster: the client sends 45 0D for a selected gold slot as for a silver one).
pub fn switch_off(gd: &GameData, p: &mut Profile, dict: u16) -> Result<(), Refused> {
    if !gd.vouchers.is_match_booster(dict) {
        return Err(Refused::NotABooster);
    }
    match p.boosters.get_mut(&dict) {
        Some(b) if b.enabled => {
            b.enabled = false;
            Ok(())
        }
        _ => Err(Refused::NotOn),
    }
}

/// 45 0C: select.
pub fn enable(cx: &mut Ctx, peer: &mut Peer, dict: u16) {
    let gd = &cx.lobby.data;
    let r = update(cx, peer, |p| switch_on(gd, p, dict));
    after(cx, peer, "45 0C enable", dict, r);
}

/// 45 0D: unselect.
pub fn disable(cx: &mut Ctx, peer: &mut Peer, dict: u16) {
    let gd = &cx.lobby.data;
    let r = update(cx, peer, |p| switch_off(gd, p, dict));
    after(cx, peer, "45 0D disable", dict, r);
}

/// 45 0E: replace `old` by `new` (both must be valid, `old` on; nothing changes when the swap is refused).
pub fn replace(cx: &mut Ctx, peer: &mut Peer, old: u16, new: u16) {
    let gd = &cx.lobby.data;
    let r = update(cx, peer, |p| {
        let saved = p.boosters.clone();
        let out = switch_off(gd, p, old).and_then(|_| switch_on(gd, p, new));
        if out.is_err() {
            p.boosters = saved;
        }
        out
    });
    after(cx, peer, "45 0E replace", new, r);
}

/// 48 29: store the autobuy flag of a booster.
pub fn autobuy(cx: &mut Ctx, peer: &mut Peer, dict: u16, on: bool) {
    let gd = &cx.lobby.data;
    let r = update(cx, peer, |p| {
        if !gd.vouchers.is_match_booster(dict) {
            return Err(Refused::NotABooster);
        }
        p.boosters.entry(dict).or_default().autobuy = on;
        Ok(())
    });
    after(cx, peer, "48 29 autobuy", dict, r);
}

/// Answer a booster request with the 94 records as they are now, refused or not: the panel's slots follow only
/// `root.set_active_boosters`, which also unlocks the panel (`setActiveBoosters` -> `enableUI`).
fn after(cx: &mut Ctx, peer: &mut Peer, what: &str, dict: u16, r: Option<Result<(), Refused>>) {
    match r {
        Some(Ok(())) => log::info!("{}: {what} booster {dict} ok", peer.addr),
        Some(Err(e)) => log::info!("{}: {what} booster {dict} refused: {e:?}", peer.addr),
        None => return,
    }
    push_status(cx, peer);
}

/// Charge the selected boosters when a match of `level` starts ([`charge_profile`]), then show the used stock, the money
/// and the 94 records. Returns the dict ids that are active for the match.
pub fn charge_match(cx: &mut Ctx, peer: &mut Peer, level: u32) -> Vec<u16> {
    let gd = &cx.lobby.data;
    let Some(c) = update(cx, peer, |p| charge_profile(gd, p, level)) else { return vec![] };
    if !c.had {
        return c.active;
    }
    for item in c.used {
        peer.reply(Reply::Inventory(Inventory45::Used { item }));
    }
    send_money(cx, peer);
    push_status(cx, peer);
    log::info!("{}: match boosters charged: {:?}", peer.addr, c.active);
    c.active
}

/// What [`charge_profile`] did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Charge {
    /// the boosters that are active for the match
    pub active: Vec<u16>,
    /// stock rows that were used (the client lowers the stack: `83 45 03 <uid>`)
    pub used: Vec<u32>,
    /// the profile had a booster selected at all
    pub had: bool,
}

/// What booster `dict` costs when it has no stock, as the client prices it (0x2001BF5B0..0x2001BF7AF): a
/// `use_variable_costs` booster `booster_use_costs[level][nth]` silver (`nth` = variable-cost boosters paid before it), the
/// others their 8A price in their 8A currency (kind 1 = gold: 853 / 857 / 861 cost 5 gold).
fn price(gd: &GameData, dict: u16, level: u32, nth: usize) -> (Currency, u64) {
    if gd.vouchers.has_variable_cost(dict) {
        return (Currency::Silver, u64::from(gd.vouchers.match_cost(level, nth).unwrap_or(0)));
    }
    let (kind, cost) = match (gd.shop.price(dict), gd.vouchers.get(dict)) {
        (Some(p), _) => (u32::from(p.kind), p.a),
        (None, Some(r)) => (r.currency, r.cost),
        (None, None) => (0, 0),
    };
    (if kind == 1 { Currency::Gold } else { Currency::Silver }, u64::from(cost))
}

/// The charge of a match of `level` on a profile: per selected booster (dict id order), one unit of stock, else its
/// [`price`]; a booster that cannot be paid is switched off. Pure on the profile, so a copy shows what a match will
/// carry before the real charge is applied (an online match builds its options first and charges when the match server
/// accepted it).
pub fn charge_profile(gd: &GameData, p: &mut Profile, level: u32) -> Charge {
    let mut c = Charge::default();
    let on = selected(gd, p);
    c.had = !on.is_empty();
    let mut nth = 0;
    for dict in on {
        if let Some(uid) = take_stock(p, dict) {
            c.used.push(uid);
            c.active.push(dict);
            continue;
        }
        let (cur, cost) = price(gd, dict, level, nth);
        if p.spend(cur, cost).is_ok() {
            nth += usize::from(gd.vouchers.has_variable_cost(dict));
            c.active.push(dict);
        } else if let Some(b) = p.boosters.get_mut(&dict) {
            b.enabled = false;
        }
    }
    c
}

/// [`charge_match`] for an account that has no peer at hand (an online match starts for every member from the queue):
/// the charge is saved and the messages that show it (used stock, money, the 94 records) are returned for the account,
/// with what it took ([`crate::online::StartCharge`]: given back if nobody ever enters the match) and the boosters the
/// match carries (their reward bonus is paid with it).
pub fn charge_account(cx: &Ctx, account: accounts::AccountId, level: u32) -> (Vec<S2c>, crate::online::StartCharge, Vec<u16>) {
    let gd = &cx.lobby.data;
    // an online match: the selected boosters are charged and one unit of each service booster is used
    let Some((c, taken)) = crate::dispatch::update_account(cx, account, |p| {
        let before = p.clone();
        let mut c = charge_profile(gd, p, level);
        let services = use_services(gd, p);
        c.had |= !services.is_empty();
        c.used.extend(services);
        (c, crate::online::StartCharge::between(&before, p))
    }) else {
        return (vec![], Default::default(), vec![]);
    };
    if !c.had {
        return (vec![], taken, c.active);
    }
    let mut out: Vec<S2c> = c.used.iter().map(|&item| S2c::Reply(Reply::Inventory(Inventory45::Used { item }))).collect();
    if let Ok(p) = cx.lobby.store.load_profile(account) {
        out.push(S2c::Money(crate::build::money_8b(&p)));
        out.extend(crate::premium::status_msg(cx, account));
    }
    log::info!("account {account}: match boosters charged: {:?}", c.active);
    (out, taken, c.active)
}

/// Reward bonus (fractions `[experience, silver, reputation]`) of the worn items: the `default_modifications` of each
/// worn item and its workshop rolls whose (group, grade) is an experience / money / reputation bonus
/// (`game_data::progression::reward_modifiers`; premium weapons +25 % silver, premium armour +10 % experience, ...).
/// LIKELY additive with the premium and booster bonuses (official: bonuses are additive, Steam news 2019 double XP
/// weekend). Decals are not worn through the slots here (the decal editor is not implemented), so their bonus is OPEN.
/// The faction reputation pairs (groups 125..128 on 12 camouflages / decals) count only when the match reputation goes to
/// their faction, the active one (`Progression::reward_bonus_for`).
pub fn worn_bonus(gd: &GameData, p: &Profile) -> [f64; 3] {
    gd.progression.reward_bonus_for(worn_mods(gd, p), p.faction)
}

/// Spare-parts bonus in percent of the worn items (`details_bonus_modifier`, group 147: weapons and decals), added to
/// the spare-parts drop of a match (`Drops::roll_with_parts_bonus`).
pub fn parts_percent(gd: &GameData, p: &Profile) -> u32 {
    (gd.progression.parts_bonus(worn_mods(gd, p)) * 100.0).round() as u32
}

/// Every (group, grade) modification the worn items, their workshop rolls, their decals and the decal sets give.
fn worn_mods(gd: &GameData, p: &Profile) -> Vec<(u32, u32)> {
    let mut mods: Vec<(u32, u32)> = Vec::new();
    for uid in p.slots.values() {
        let Some(row) = p.item(*uid) else { continue };
        // a skin is cosmetic: the bonus is the one of the weapon underneath (docs/notes/weapon-skins.md)
        if let Some(par) = gd.craft.params(p.real_dict(row)) {
            mods.extend(par.default_modifications.iter().copied());
        }
        if let Some(rolls) = p.mods.get(uid) {
            mods.extend(rolls.iter().map(|&(g, gr)| (u32::from(g), u32::from(gr))));
        }
    }
    // camouflage and decals attached to the worn armour, plus the bonus tiers of decal sets (decals.rs)
    mods.extend(gd.decals.worn_mods(&crate::decals::worn_dicts(p)));
    mods
}

/// Bonus percentages `[experience, silver, reputation]` for the post-match rewards: the premium account's +50 % plus
/// the reward modifiers of the boosters the match carried (`boosters`: the ones its start charged; 853 / 857 / 861:
/// +50 % each), added per component (LIKELY additive).
pub fn reward_percent(gd: &GameData, p: &Profile, now: i64, boosters: &[u16]) -> [u32; 3] {
    let parts = reward_percent_parts(gd, p, now, boosters);
    [0, 1, 2].map(|i| parts.iter().map(|x| x[i]).sum())
}

/// The sources of [`reward_percent`], each `[experience, silver, reputation]` in percent, in the order of
/// [`BONUS_PREMIUM`] .. [`BONUS_BOOSTERS`]: the result window shows each in its own row.
pub fn reward_percent_parts(gd: &GameData, p: &Profile, now: i64, boosters: &[u16]) -> [[u32; 3]; 5] {
    let mut parts = [[0u32; 3]; 5];
    if p.premium_left(now) > 0 {
        parts[BONUS_PREMIUM] = [crate::premium::MATCH_BONUS_PERCENT; 3];
    }
    // the worn items' bonus (rounded once) and the decals' part of it
    let worn = worn_bonus(gd, p);
    let decals = gd.progression.reward_bonus_for(gd.decals.worn_mods(&crate::decals::worn_dicts(p)), p.faction);
    for i in 0..3 {
        let all = (worn[i] * 100.0).round() as u32;
        let d = ((decals[i] * 100.0).round() as u32).min(all);
        parts[BONUS_DECALS][i] = d;
        parts[BONUS_ITEMS][i] = all - d;
    }
    // hero of the day: +25 / +20 / +15 % for the 24 h of the status (added like the others)
    parts[BONUS_HERO] = crate::hero::bonus_percent(gd, p, now);
    for &d in boosters.iter().filter(|d| gd.vouchers.is_match_booster(**d)) {
        for (i, name) in REWARD_MODIFIERS.iter().enumerate() {
            parts[BONUS_BOOSTERS][i] += (gd.vouchers.modifier(d, name) * 100.0).round() as u32;
        }
    }
    parts
}

/// [`reward_percent_parts`] index: the premium account (97 score block 2 `premium`).
pub const BONUS_PREMIUM: usize = 0;
/// the worn items' reward modifications without decals (block 1 `premium_items`, "Equipment bonus")
pub const BONUS_ITEMS: usize = 1;
/// camouflage, decals and decal sets (block 8 `camo_and_decal_bonus`)
pub const BONUS_DECALS: usize = 2;
/// hero of the day (block 9 `hero`)
pub const BONUS_HERO: usize = 3;
/// the experience / silver / reputation boosters the match carried (block 3 `boosters`)
pub const BONUS_BOOSTERS: usize = 4;

/// The match-side modifiers of the selected boosters: (modifier name, value). The reward modifiers of 853 / 857 / 861
/// are not among them (they act on the pay, [`reward_percent`]).
pub fn match_modifiers(gd: &GameData, p: &Profile) -> Vec<(String, f64)> {
    selected(gd, p)
        .into_iter()
        .filter_map(|d| gd.vouchers.get(d))
        .flat_map(|r| r.modifiers.iter().filter(|m| !REWARD_MODIFIERS.contains(&m.name.as_str())).map(|m| (m.name.clone(), m.value)))
        .collect()
}

/// The same modifiers as the client's modifier ids (index in its name table 0x200995E10), summed per id: what goes
/// into the profile's modifier block of the match options (`HumanLoadout::extra_modifiers`).
pub fn match_modifier_ids(gd: &GameData, p: &Profile) -> std::collections::BTreeMap<u32, f64> {
    let names = gd.matches.modifier_names();
    let mut out = std::collections::BTreeMap::new();
    for (name, v) in match_modifiers(gd, p) {
        if let Some(i) = names.iter().position(|n| *n == name) {
            *out.entry(i as u32).or_insert(0.0) += v;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gd() -> GameData {
        game_data::testing::tables()
    }

    #[test]
    fn a_gold_booster_is_selected_free_charged_per_match_and_unselected_like_the_others() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        p.gold = 12;
        // selecting pays nothing (the client shows the 5 gold as a match cost), the 94 lists it, a second select is refused
        assert_eq!(switch_on(&gd, &mut p, 853), Ok(()));
        assert_eq!(p.gold, 12);
        assert_eq!(records(&gd, &p).iter().map(|n| (n.kind, n.f40)).collect::<Vec<_>>(), vec![(KIND_BOOSTER, 853)]);
        assert_eq!(switch_on(&gd, &mut p, 853), Err(Refused::AlreadyOn));
        // 45 0D on the selected gold slot unselects it (the client sends it for a gold slot as for a silver one)
        assert_eq!(switch_off(&gd, &mut p, 853), Ok(()));
        assert!(records(&gd, &p).is_empty() && selected(&gd, &p).is_empty());
        assert_eq!(switch_off(&gd, &mut p, 853), Err(Refused::NotOn));
        // each match start charges 5 gold per gold booster (its 8A price and kind); what cannot be paid is switched off
        switch_on(&gd, &mut p, 853).unwrap();
        switch_on(&gd, &mut p, 857).unwrap();
        let c = charge_profile(&gd, &mut p, 1);
        assert_eq!((c.active.clone(), c.used.clone(), p.gold), (vec![853, 857], vec![], 2));
        assert_eq!(selected(&gd, &p), vec![853, 857], "the selection stays for the next match");
        let c = charge_profile(&gd, &mut p, 1);
        assert!(c.active.is_empty() && c.had && p.gold == 2 && selected(&gd, &p).is_empty(), "2 gold pays neither");
        // the starter's worn scavenger kit gives +2 % reputation with the scavengers, the active faction (125, 3)
        let w = (worn_bonus(&gd, &p)[2] * 100.0).round() as u32;
        assert_eq!((p.faction, w), (1, 2));
        // the bonus goes with the match that carried the booster; no match-side modifier
        assert_eq!(reward_percent(&gd, &p, 2000, &[853]), [50, 0, w]);
        assert_eq!(reward_percent(&gd, &p, 2000, &[]), [0, 0, w]);
        p.extend_premium(2000, 100);
        assert_eq!(reward_percent(&gd, &p, 2000, &[853]), [100, 50, 50 + w]);
        assert_eq!(reward_percent(&gd, &p, 2000, &[853, 857, 861]), [100, 100, 100 + w]);
        switch_on(&gd, &mut p, 861).unwrap();
        assert!(match_modifiers(&gd, &p).is_empty(), "reward modifiers do not go into the match options");
    }

    #[test]
    fn stock_is_used_before_money() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        let uid = p.add_item(861, 2);
        p.gold = 0;
        assert_eq!(switch_on(&gd, &mut p, 861), Ok(()));
        assert_eq!(p.item(uid).unwrap().count, 2, "selecting takes nothing");
        let c = charge_profile(&gd, &mut p, 1);
        assert_eq!((c.active, c.used, p.gold, p.item(uid).unwrap().count), (vec![861], vec![uid], 0, 1));
    }

    #[test]
    fn charge_profile_prices_like_the_client_and_switches_off_what_it_cannot_pay() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        for d in [339, 340, 341, 853] {
            switch_on(&gd, &mut p, d).unwrap();
        }
        let uid = p.add_item(339, 1);
        p.silver = 600;
        p.gold = 5;
        let c = charge_profile(&gd, &mut p, 1);
        // 339 from the stock (free, no cost column used), 340 is the first variable booster without stock: column 0
        // (250); 341 takes column 1 (500) and 350 is left: switched off. 853 is 5 gold (no column).
        assert_eq!((c.active.clone(), c.used.clone(), c.had), (vec![339, 340, 853], vec![uid], true));
        assert_eq!((p.silver, p.gold), (600 - gd.vouchers.match_cost(1, 0).unwrap(), 0));
        assert!(!p.boosters[&341].enabled && p.boosters[&340].enabled && p.boosters[&853].enabled);
        // no booster on: nothing happens
        let mut q = Profile::starter();
        assert_eq!(charge_profile(&gd, &mut q, 1), Charge::default());
    }

    #[test]
    fn three_boosters_per_slot_group() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        for d in [339, 340, 341] {
            assert_eq!(switch_on(&gd, &mut p, d), Ok(()));
        }
        assert_eq!(switch_on(&gd, &mut p, 342), Err(Refused::TooMany));
        // the gold slots are a group of their own
        for d in [853, 857, 861] {
            assert_eq!(switch_on(&gd, &mut p, d), Ok(()));
        }
        assert_eq!(switch_on(&gd, &mut p, 1088), Err(Refused::NotABooster));
        assert_eq!(switch_off(&gd, &mut p, 213), Err(Refused::NotABooster));
        assert_eq!(selected(&gd, &p), vec![339, 340, 341, 853, 857, 861]);
        assert_eq!(switch_off(&gd, &mut p, 340), Ok(()));
        assert_eq!(switch_off(&gd, &mut p, 340), Err(Refused::NotOn));
        assert_eq!(match_modifiers(&gd, &p).len(), 2);
        // weight 68 and regeneration 0 (client ids)
        let ids: Vec<u32> = match_modifier_ids(&gd, &p).keys().copied().collect();
        assert_eq!(ids, vec![0, 68]);
        assert_eq!(records(&gd, &p).iter().map(|n| n.f40).collect::<Vec<_>>(), vec![339, 341, 853, 857, 861]);
    }

    #[test]
    fn the_newbie_blessing_shows_as_service_record_24_and_one_unit_goes_per_online_match() {
        game_data::require_tables!();
        use game_data::vouchers::NEW_PLAYER_BOOSTER;
        let gd = gd();
        let mut p = Profile::starter();
        assert!(service_modifier_ids(&gd, &p).is_empty() && use_services(&gd, &mut p).is_empty(), "none without stock");
        let uid = p.add_item(NEW_PLAYER_BOOSTER, 3);
        // 94 type 24 (the service id), f40 = group 140 grade 0 (the pair the client matches), f44 = 3 matches left
        let r: Vec<(u8, u32, u32)> = records(&gd, &p).iter().map(|n| (n.kind, n.f40, n.f44)).collect();
        assert_eq!(r, vec![(24, 0x008C_0000, 3)]);
        // the options carry modifier id 76 (new_player_damage_modifier) = -0.15
        assert_eq!(service_modifier_ids(&gd, &p).into_iter().collect::<Vec<_>>(), vec![(76, -0.15)]);
        assert!(!match_modifier_ids(&gd, &p).contains_key(&76), "not a per-match booster: local matches do not carry it");
        for left in [2, 1] {
            assert_eq!(use_services(&gd, &mut p), vec![uid]);
            assert_eq!(records(&gd, &p)[0].f44, left);
        }
        assert_eq!(use_services(&gd, &mut p), vec![uid]);
        assert!(p.item(uid).is_none() && records(&gd, &p).is_empty() && service_modifier_ids(&gd, &p).is_empty(), "used up");
    }
}
