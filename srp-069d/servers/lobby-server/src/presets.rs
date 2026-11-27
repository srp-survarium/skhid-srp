//! Equipment presets: `58` requests (create, remove, rename, load, save) and the `BB` list (docs/notes/items.md).
//!
//! The client keeps the list from `BB` only (the table is zeroed on every `BB`), so every operation is answered with
//! a fresh `BB`; `BB` is also sent once after the lobby-load handshake (without it the preset UI stays disabled).
//! Layouts and op numbers: `protocol::c2s::Presets58`, `protocol::s2c::PresetsBB`.

use accounts::{Preset, PresetSlot, Profile};
use game_data::pockets::QUICK_SLOTS;
use protocol::c2s::Presets58;
use protocol::s2c::*;

use crate::build;
use crate::dispatch::{update, Ctx};
use crate::equip::AMMO_SLOTS;
use crate::peer::Peer;


/// Highest profile slot (0..=22).
const LAST_SLOT: u8 = 22;

/// The current equipment as preset slots: every occupied slot with its rounds / units.
pub fn snapshot(p: &Profile) -> Vec<PresetSlot> {
    p.slots
        .iter()
        .filter(|(_, uid)| p.item(**uid).is_some())
        .map(|(&slot, &uid)| PresetSlot { slot, uid, rounds: p.slot_rounds.get(&slot).copied() })
        .collect()
}

/// Equip a preset (choice: the preset names the slots it fills; slots it does not name keep what they hold).
/// A cell is skipped when its item is gone or when its slot is out of range. A non-stack item sitting in another slot moves (one item, one slot; ammo and pocket cells may repeat),
/// rounds are capped to the stack. A pocket cell is set whether its consumable is locked for the player or not: the
/// load only uses units the player owns (the rounds are capped to the stack, nothing is bought), and the client tests
/// `opened` only on the pocket popup's tiles (PROVEN, `crate::equip::apply`), not on a preset load.
/// Returns how many slots were set.
pub fn apply(p: &mut Profile, preset: &Preset) -> usize {
    let mut n = 0;
    for s in &preset.slots {
        if s.slot > LAST_SLOT {
            continue;
        }
        let quick = QUICK_SLOTS.contains(&s.slot);
        let Some(row) = p.item(s.uid).copied() else { continue };
        if !AMMO_SLOTS.contains(&s.slot) && !quick {
            let others: Vec<u8> = p
                .slots
                .iter()
                .filter(|(k, v)| **v == s.uid && **k != s.slot && !AMMO_SLOTS.contains(k) && !QUICK_SLOTS.contains(k))
                .map(|(k, _)| *k)
                .collect();
            for k in others {
                p.slots.remove(&k);
            }
        }
        p.slots.insert(s.slot, s.uid);
        match s.rounds {
            Some(r) if AMMO_SLOTS.contains(&s.slot) || quick => {
                p.slot_rounds.insert(s.slot, r.min(row.count));
            }
            _ => {
                p.slot_rounds.remove(&s.slot);
            }
        }
        n += 1;
    }
    n
}

/// The BB message for the account's stored presets.
pub fn list_message(presets: &[Preset]) -> S2c {
    S2c::Presets(PresetsBB {
        presets: presets.iter().map(|p| PresetEntry { id: p.id, flag: 0, name: p.name.clone() }).collect(),
    })
}

/// Send the account's presets (`BB`).
pub fn push_list(cx: &Ctx, peer: &mut Peer) {
    let Some(id) = peer.account else { return };
    match cx.lobby.store.presets(id) {
        Ok(list) => peer.push(&list_message(&list)),
        Err(e) => log::error!("{}: loading presets of account {id}: {e}", peer.addr),
    }
}

/// One `58` request. A request that names another profile, or a preset the account does not have, is ignored.
pub fn handle(cx: &mut Ctx, peer: &mut Peer, req: &Presets58) {
    let Some(id) = peer.account else { return };
    let store = cx.lobby.store.clone();
    let result: Result<(), accounts::Error> = (|| {
        let cur_id = build::wire_profile_id(store.load_profile(id)?.equip.current);
        match req {
            Presets58::Create { name, profile } if *profile == cur_id => {
                let p = store.load_profile(id)?;
                let premium = p.premium_left(crate::premium::unix_now()) > 0;
                let pid = store.create_preset(id, name, &snapshot(&p), premium)?;
                log::info!("{}: 58 create preset {pid} {name:?} ({} slots)", peer.addr, snapshot(&p).len());
            }
            Presets58::Save { profile, preset } if *profile == cur_id => {
                let p = store.load_profile(id)?;
                let ok = store.save_preset(id, *preset, &snapshot(&p))?;
                log::info!("{}: 58 save preset {preset}: {}", peer.addr, if ok { "saved" } else { "no such preset" });
            }
            Presets58::Remove { preset } => {
                let ok = store.remove_preset(id, *preset)?;
                log::info!("{}: 58 remove preset {preset}: {ok}", peer.addr);
            }
            Presets58::Rename { preset, name } => {
                let ok = store.rename_preset(id, *preset, name)?;
                log::info!("{}: 58 rename preset {preset} to {name:?}: {ok}", peer.addr);
            }
            Presets58::Load { profile, preset } if *profile == cur_id => {
                let Some(pr) = store.presets(id)?.into_iter().find(|p| p.id == *preset) else {
                    log::info!("{}: 58 load: no preset {preset}", peer.addr);
                    return Ok(());
                };
                let name = peer.name.clone();
                let gd = &cx.lobby.data;
                let out = update(cx, peer, |p| {
                    let n = apply(p, &pr);
                    p.bump_version();
                    (n, build::account_87(gd, p, &name))
                });
                if let Some((n, acct)) = out {
                    log::info!("{}: 58 load preset {preset}: {n} of {} slots set", peer.addr, pr.slots.len());
                    peer.push(&S2c::Account(acct));
                }
            }
            other => log::info!("{}: 58 {other:?} does not name the current profile {cur_id}: ignored", peer.addr),
        }
        Ok(())
    })();
    if let Err(e) = result {
        // a refused name or the preset limit: the list is sent anyway so the client's table stays right
        log::info!("{}: 58 refused: {e}", peer.addr);
    }
    push_list(cx, peer);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Loading a preset sets a pocket cell whose consumable is locked for the player (owned units only: the rounds are
    /// capped to the stack), as it sets open items and the other slots.
    #[test]
    fn a_locked_pocket_cell_of_a_preset_is_set_with_owned_units() {
        let mut p = Profile::starter();
        p.fac_points.clear();
        let medkit = p.add_item(760, 3); // Fringe Settlers level 2
        let mine = p.add_item(736, 1); // MON-50, Black Market level 4
        let scav = p.add_item(67, 2); // Scavengers medkit, level 1: open at 0 points
        let cell = |slot, uid, rounds| PresetSlot { slot, uid, rounds: Some(rounds) };
        let preset = Preset { id: 1, name: "x".into(), slots: vec![cell(13, medkit, 3), cell(14, mine, 1), cell(15, scav, 2)] };
        assert_eq!(apply(&mut p, &preset), 3);
        assert_eq!((p.slots.get(&13), p.slots.get(&14), p.slots.get(&15)), (Some(&medkit), Some(&mine), Some(&scav)));
        assert_eq!((p.slot_rounds.get(&13), p.slot_rounds.get(&14)), (Some(&3), Some(&1)));
        // more rounds than owned: capped to the stack, nothing bought
        let preset = Preset { id: 2, name: "y".into(), slots: vec![cell(16, mine, 4)] };
        assert_eq!(apply(&mut p, &preset), 1);
        assert_eq!((p.slot_rounds.get(&16), p.item(mine).unwrap().count), (Some(&1), 1));
    }
}
