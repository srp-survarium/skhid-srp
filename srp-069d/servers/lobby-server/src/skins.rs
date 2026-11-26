//! Weapon skins: `45 11` change, `45 12` preview and the unlocked list of the workshop push B3. Rules, sources and open
//! points: docs/notes/weapon-skins.md. Data: `game_data::SkinData` (`skins_to_unlock`); storage: `Profile::skins`
//! (schema v15 `skin_unlocks`).
//!
//! A skin is a weapon dictionary item (a premium variant of a base weapon). Applying it changes the dict id of the
//! player's weapon record; `83 45 11` carries the new record (PROVEN reader 0x2001904CD: the client logs "changing item
//! %d skin from %d to %d" and replaces the record).

use accounts::{ItemRow, Profile};
use game_data::GameData;
use protocol::s2c::*;

use crate::build;
use crate::dispatch::{update, Ctx};
use crate::peer::Peer;

/// The base weapon of a weapon item: its `base_item_dict_id` (a skin or event variant points at the plain weapon), else
/// itself. None when the dict id is not a weapon.
pub fn base_weapon(gd: &GameData, dict: u16) -> Option<u16> {
    let it = gd.items.get(dict)?;
    (it.section == "weapons").then(|| it.base_item.and_then(|b| u16::try_from(b).ok()).unwrap_or(dict))
}

/// Unlock every skin the account owns an item of (CHOICE, docs/notes/weapon-skins.md: buying or receiving the variant
/// weapon unlocks its skin for good; selling it does not take the skin back). Returns whether something was added.
pub fn sync_unlocks(gd: &GameData, p: &mut Profile) -> bool {
    let owned: Vec<u16> = p.items.iter().map(|i| i.dict_id).filter(|d| gd.skins.is_skin(*d)).collect();
    let mut added = false;
    for d in owned {
        added |= p.skins.insert(d);
    }
    added
}

/// Skins a weapon of `dict` can wear that the account unlocked, plus the plain weapon itself (so the skin can be taken
/// off again). This is the `unlocked` list of B3.
pub fn unlocked_for(gd: &GameData, p: &Profile, dict: u16) -> Vec<u16> {
    let Some(base) = base_weapon(gd, dict) else { return vec![] };
    let mut v = vec![base];
    v.extend(gd.skins.skins_for(base).into_iter().filter(|s| p.skins.contains(s)));
    v
}

/// B3 list for the workshop of `dict` (syncs the unlocks first; the caller saves the profile).
pub fn open_list(gd: &GameData, p: &mut Profile, dict: u16) -> Vec<u16> {
    sync_unlocks(gd, p);
    unlocked_for(gd, p, dict)
}

/// Whether the weapon row may wear `skin` (`unlocked` = also require the account's unlock).
fn allowed(gd: &GameData, p: &Profile, row: &ItemRow, skin: u16, unlocked: bool) -> bool {
    let Some(base) = base_weapon(gd, row.dict_id) else { return false };
    if skin == base {
        return true;
    }
    gd.skins.targets(skin).contains(&base) && (!unlocked || p.skins.contains(&skin))
}

/// At login: unlock the skins of the weapons the account owns, and fall back to profile 0 when the current profile ended
/// to be available (premium over).
pub fn sync_on_login(cx: &Ctx, id: accounts::AccountId) {
    let gd = &cx.lobby.data;
    let now = crate::premium::unix_now();
    let res = cx.lobby.store.update_profile(id, |p| {
        let a = sync_unlocks(gd, p);
        let b = p.fix_current_profile(now);
        if b {
            p.bump_version();
        }
        Ok::<_, ()>(a || b)
    });
    if let Err(e) = res {
        log::error!("skin / profile sync of account {id} failed: {e:?}");
    }
}

/// The record of the weapon as the client would hold it with `skin` applied.
fn record_with(gd: &GameData, p: &Profile, row: &ItemRow, skin: u16) -> protocol::items::ItemRecord {
    let mut skinned = *row;
    skinned.dict_id = skin;
    // the skin's own default modules (skins have their own lists), so the record is built for the skin's dict
    let mut r = build::record(gd, p, &skinned);
    r.dict = skin;
    r
}

/// `45 11 u32 item, u16 skin`: put the skin on the weapon. Refusals (not a weapon, not owned, skin of another weapon,
/// not unlocked) are silent: the client has no refusal for this request and its UI only offers unlocked skins (OPEN).
/// Success: `83 45 11 <record>`, then a fresh 87 with a bumped version (the 3D character is rebuilt only then).
pub fn change(cx: &mut Ctx, peer: &mut Peer, item: u32, skin: u16) {
    let gd = cx.lobby.data.clone();
    let name = peer.name.clone();
    let out = update(cx, peer, |p| {
        sync_unlocks(&gd, p);
        let row = *p.item(item)?;
        if !allowed(&gd, p, &row, skin, true) {
            log::info!("45 11: weapon {item} (dict {}) cannot wear skin {skin}", row.dict_id);
            return None;
        }
        if row.dict_id != skin {
            // the weapon stays the item it was (bonuses, sell price): remember it while a skin is on
            let real = p.real_dict(&row);
            p.item_mut(item)?.dict_id = skin;
            if skin == real {
                p.skin_base.remove(&item);
            } else {
                p.skin_base.insert(item, real);
            }
            p.bump_version();
        }
        let row = *p.item(item)?;
        Some((build::record(&gd, p, &row), build::account_87(&gd, p, &name)))
    });
    let Some(Some((rec, acct))) = out else { return };
    log::info!("{}: 45 11 weapon {item} -> skin {skin}", peer.addr);
    peer.reply(Reply::Inventory(Inventory45::Skin { sub: 0x11, item: rec }));
    peer.push(&S2c::Account(acct));
}

/// `45 12 u32 item, u16 skin`: preview. Answered with `83 45 12 <record>` carrying the skin's dict id; nothing is stored
/// (the client shows it on the weapon being viewed, reader 0x2001906CD). Locked skins can be previewed: the selector
/// offers an observe button on them too (`SrvSkinObserveButton`).
pub fn preview(cx: &mut Ctx, peer: &mut Peer, item: u32, skin: u16) {
    let Some(id) = peer.account else { return };
    let gd = cx.lobby.data.clone();
    let p = match cx.lobby.store.load_profile(id) {
        Ok(p) => p,
        Err(e) => {
            log::error!("{}: loading profile of {id}: {e}", peer.addr);
            return;
        }
    };
    let Some(row) = p.item(item).copied() else { return };
    if !allowed(&gd, &p, &row, skin, false) {
        log::info!("{}: 45 12: weapon {item} cannot show skin {skin}", peer.addr);
        return;
    }
    peer.reply(Reply::Inventory(Inventory45::Skin { sub: 0x12, item: record_with(&gd, &p, &row, skin) }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skin_belongs_to_its_base_weapon() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        // 1240 sr2m_premium is a skin of 1225 sr2m
        assert_eq!(base_weapon(&gd, 1240), Some(1225));
        assert_eq!(base_weapon(&gd, 1225), Some(1225));
        assert_eq!(base_weapon(&gd, 925), None);
        let mut p = Profile::starter();
        assert_eq!(unlocked_for(&gd, &p, 1225), vec![1225]);
        p.skins.insert(1240);
        assert_eq!(unlocked_for(&gd, &p, 1225), vec![1225, 1240]);
        // a weapon wearing the skin lists the same set
        assert_eq!(unlocked_for(&gd, &p, 1240), vec![1225, 1240]);
    }

    #[test]
    fn owning_the_variant_unlocks_its_skin() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let mut p = Profile::starter();
        assert!(!sync_unlocks(&gd, &mut p));
        p.add_item(1240, gd.items.amount_field(1240, 1));
        assert!(sync_unlocks(&gd, &mut p));
        assert!(p.skins.contains(&1240));
        assert!(!sync_unlocks(&gd, &mut p));
    }
}
