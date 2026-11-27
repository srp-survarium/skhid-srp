//! Equipment profiles: `48 00` rename, `48 01` enable, `48 02` select, `48 03` buy. Rules, sources and open points:
//! docs/notes/profiles.md. Storage: `accounts::equip` (schema v15).
//!
//! The wire id of profile `idx` is `idx + 1` ([`build::wire_profile_id`]); the client sends the id it read from the 87
//! (C++ `profile_rename` 0x200291000 / `enable_profile` 0x200291150 / `profile_changed` 0x200290F80 turn the tab index
//! of the Flash `ProfileSelector` into the id at `profile + 0x29F8`).

use accounts::equip::{MAX_NAME, PROFILES};
use accounts::{Currency, Profile};
use protocol::s2c::*;

use crate::build;
use crate::dispatch::{update, Ctx};
use crate::peer::Peer;
use crate::premium::unix_now;

/// Index of the profile a wire id names.
pub fn index_of(id: u32) -> Option<u8> {
    (1..=PROFILES as u32).contains(&id).then(|| (id - 1) as u8)
}

/// A name the rename dialog may send: 1..=[`MAX_NAME`] characters after trimming, no control characters (CHOICE: the
/// client has no stated limit; the 87 carries `str128`).
pub fn clean_name(raw: &str) -> Option<String> {
    let t = raw.trim();
    (!t.is_empty() && t.chars().count() <= MAX_NAME && !t.chars().any(|c| c.is_control())).then(|| t.to_string())
}

/// Price in silver of the next profile: `buy_preset_cost[0][owned - 1]` (the client shows `_profilesCost[active - 1]`
/// with `active` = profiles it has, `ProfileSelector.setupProfiles`).
pub fn next_price(gd: &game_data::GameData, owned: usize) -> Option<u32> {
    let row = gd.buy_preset_cost.first()?;
    row.get(owned.saturating_sub(1)).copied()
}

fn count_owned(p: &Profile) -> usize {
    p.equip.profiles.iter().filter(|e| e.owned).count()
}

fn push_87(cx: &Ctx, peer: &mut Peer, p: &Profile) {
    peer.push(&S2c::Account(build::account_87(&cx.lobby.data, p, &peer.name)));
}

/// The account's equipment profiles in the order a match's options carry them: entry p = options profile p (the world
/// host's `Used` bit p), as `(profile index, sent with its slot table)`. Tab order, except that the current profile moves
/// to the front when tab 0 is not playable: local matches always spawn profile index 0 and the online start is the lowest
/// usable index, and index 0 must exist or the spawn reads a null profile slot (0x2002F11F4). A profile the account can
/// play (bought / premium and enabled) or the current one is sent with its slots (state 0), any other without (state 2).
/// srp `matches::match_profiles` builds the options from this order.
pub fn options_order(p: &Profile, now: i64) -> Vec<(u8, bool)> {
    let mut order: Vec<u8> = (0..PROFILES as u8).collect();
    if !p.profile_playable(0, now) {
        let cur = p.equip.current;
        order.retain(|&i| i != cur);
        order.insert(0, cur);
    }
    order.into_iter().map(|i| (i, p.profile_playable(i, now) || i == p.equip.current)).collect()
}

/// The check every match start makes: a premium-only profile that is current when the premium ran out falls back to
/// profile 0 now, not at the next sign-in (the loadout of the match is the current profile). Returns whether it switched
/// (the profile version is bumped so the next 87 rebuilds the character).
pub fn ensure_playable(store: &accounts::Store, account: accounts::AccountId) -> bool {
    let now = unix_now();
    match store.update_profile(account, |p| {
        let switched = p.fix_current_profile(now);
        if switched {
            p.bump_version();
        }
        Ok::<_, ()>(switched)
    }) {
        Ok(s) => s,
        Err(e) => {
            log::error!("profile check of account {account} failed: {e:?}");
            false
        }
    }
}

/// `48 02 u32 id`: the client selected a profile tab (also 0.5 s after the lobby loads). The worn items change to that
/// profile's; the answer is `83 48 02 id` (PROVEN: the client makes it current and rebuilds the character) preceded by
/// a fresh 87 when the table changed. A profile that is not available, or an unknown id, is answered with the
/// current profile's id.
pub fn changed(cx: &mut Ctx, peer: &mut Peer, id: u32) {
    let now = unix_now();
    let want = index_of(id);
    let out = update(cx, peer, |p| {
        let mut switched = p.fix_current_profile(now);
        if let Some(i) = want.filter(|i| p.profile_available(*i, now)) {
            if i != p.equip.current {
                switched |= p.switch_profile(i);
            }
        } else {
            log::info!("48 02 profile id {id}: unknown or not available");
        }
        if switched {
            p.bump_version();
        }
        (switched, p.clone())
    });
    let Some((switched, p)) = out else { return };
    if switched {
        push_87(cx, peer, &p);
    }
    peer.reply(Reply::Account(Account48::ProfileSet { profile: build::wire_profile_id(p.equip.current) }));
}

/// `48 00 u32 id, str name`: rename. Answered with a fresh 87 (the client has no handler for `83 48 00`; the 87 rebuilds its
/// profile table, so the new name shows; LIKELY, no recording of the original server).
pub fn rename(cx: &mut Ctx, peer: &mut Peer, id: u32, name: &str) {
    let (Some(i), Some(name)) = (index_of(id), clean_name(name)) else {
        log::info!("{}: 48 00 rename of profile {id} to {name:?} refused", peer.addr);
        return;
    };
    let now = unix_now();
    let out = update(cx, peer, |p| {
        if !p.profile_available(i, now) {
            return None;
        }
        p.equip.profiles[i as usize].name = name;
        Some(p.clone())
    });
    if let Some(Some(p)) = out {
        push_87(cx, peer, &p);
    }
}

/// `48 01 u32 id, u8 enable`: the checkbox of a profile tab. The last enabled usable profile cannot be disabled (the client
/// only offers the box when another one is enabled: `ProfileSelector.isOtherProfilesEnabled`). Disabling the current
/// profile makes the first other enabled one current. Answered with a fresh 87.
pub fn enable(cx: &mut Ctx, peer: &mut Peer, id: u32, on: bool) {
    let Some(i) = index_of(id) else { return };
    let now = unix_now();
    let out = update(cx, peer, |p| {
        if !p.profile_available(i, now) {
            return None;
        }
        let others = (0..PROFILES as u8).any(|k| k != i && p.profile_playable(k, now));
        if !on && !others {
            log::info!("48 01: profile {id} is the last enabled one, stays on");
        } else {
            p.equip.profiles[i as usize].enabled = on;
            if !on && p.equip.current == i {
                if let Some(k) = (0..PROFILES as u8).find(|k| *k != i && p.profile_playable(*k, now)) {
                    p.switch_profile(k);
                }
            }
            p.bump_version();
        }
        Some(p.clone())
    });
    if let Some(Some(p)) = out {
        push_87(cx, peer, &p);
    }
}

/// `48 03 u8 index`: buy the profile slot at `index` (= number of profiles the client has) with silver. The price is
/// [`next_price`]; a profile that is already owned, an index out of range or too little silver changes nothing and the
/// answer is a fresh 87. Too little silver also gets `84 x 48 01 st_not_enough_money` ("Not enough money for the
/// transaction.", the generic text channel of 48, 0x200176E3A; the client checks the price itself first, `AskBuyProfile`,
/// and has no text of its own for this request). Success: 87, then 8B.
pub fn buy(cx: &mut Ctx, peer: &mut Peer, index: u8) {
    let gd = cx.lobby.data.clone();
    let out = update(cx, peer, |p| {
        let i = index as usize;
        if i == 0 || i >= PROFILES || p.equip.profiles[i].owned {
            return (false, false, p.clone());
        }
        let Some(price) = next_price(&gd, count_owned(p)) else { return (false, false, p.clone()) };
        if p.spend(Currency::Silver, price as u64).is_err() {
            log::info!("48 03: profile {index} costs {price} silver, balance {}", p.silver);
            return (false, true, p.clone());
        }
        p.equip.profiles[i].owned = true;
        p.bump_version();
        (true, false, p.clone())
    });
    let Some((bought, poor, p)) = out else { return };
    push_87(cx, peer, &p);
    if poor {
        peer.push(&S2c::Denied(Denied { request: 0x48, body: DeniedBody::Text("st_not_enough_money".into()) }));
    }
    if bought {
        log::info!("{}: 48 03 profile {index} bought", peer.addr);
        peer.push(&S2c::Money(build::money_8b(&p)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_map_to_indexes() {
        assert_eq!(index_of(1), Some(0));
        assert_eq!(index_of(3), Some(2));
        assert_eq!(index_of(0), None);
        assert_eq!(index_of(4), None);
    }

    #[test]
    fn names_are_checked() {
        assert_eq!(clean_name("  Sniper  ").as_deref(), Some("Sniper"));
        assert_eq!(clean_name("   "), None);
        assert_eq!(clean_name(&"x".repeat(MAX_NAME + 1)), None);
        assert_eq!(clean_name("a\nb"), None);
    }

    #[test]
    fn the_price_follows_the_profiles_owned() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        assert_eq!(next_price(&gd, 1), Some(200_000));
        assert_eq!(next_price(&gd, 2), Some(500_000));
    }
}
