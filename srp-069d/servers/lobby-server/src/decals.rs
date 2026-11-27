//! Camouflage and decals on armour (45 01 attach, 45 02 remove, 45 04 editor save). See `docs/notes/decals.md`.
//!
//! State: `Profile::decals` (owner item uid -> entries, schema v13). The entries go out in the `n3` list of every item
//! record (`build::record_value`), and the worn ones feed the reward bonus (`boosters::worn_bonus`).
//!
//! Rules (client facts PROVEN, the rest CHOICE and said so):
//! * the target must be a torso or legs that accepts the kind (`camo_available` / `decal_available` of its dictionary row);
//! * at most 3 entries per item (client record cap); an entry is keyed by its slot byte: attaching to a used slot replaces
//!   the entry (the client does the same when it reads `83 45 01`);
//! * the slot follows the editor (PROVEN, lobby movie `DecorateControl`): camouflage only in slot 3, decals only in slots 0
//!   and 1, so a piece wears one camouflage and two decals; the camouflage has no placement editor (no 45 04 for slot 3);
//!   the 8 placement bytes are an opaque packed transform made by the native editor (0x27A660 -> 0x61A460), not an
//!   IEEE double: they are stored and echoed verbatim;
//! * one owner per decal item (the client keys an item-to-owner map by the decal item's uid): a decal item that sits on
//!   another piece is refused, the same item twice on one piece too (CHOICE: the client would show it twice);
//! * attaching and removing are free (no price in the data or the client: CHOICE).

use accounts::DecalRow;
use protocol::s2c::*;

use crate::dispatch::{update, Ctx};
use crate::peer::Peer;

/// Why an attach was refused (logged; the client gets no answer, its UI shows no change).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NoItem,
    NoDecal,
    DoesNotFit,
    /// camouflage outside slot 3 or a decal outside slots 0 / 1 (the editor never offers it)
    WrongSlot,
    OwnedElsewhere,
    Duplicate,
    Full,
}

/// Attach the decal item `decal` to `item` in `slot`. Returns the (possibly replaced) entry on success.
pub fn attach(gd: &game_data::GameData, p: &mut accounts::Profile, item: u32, decal: u32, slot: u8, x_bits: u64) -> Result<DecalRow, Refused> {
    let target = p.item(item).ok_or(Refused::NoItem)?.dict_id;
    let d = p.item(decal).ok_or(Refused::NoDecal)?.dict_id;
    if item == decal || !gd.decals.fits(target, d) {
        return Err(Refused::DoesNotFit);
    }
    if !gd.decals.slot_fits(d, slot) {
        return Err(Refused::WrongSlot);
    }
    if let Some(owner) = p.decal_owner(decal) {
        if owner != item {
            return Err(Refused::OwnedElsewhere);
        }
    }
    let list = p.decals.entry(item).or_default();
    if list.iter().any(|e| e.decal_uid == decal && e.slot != slot) {
        return Err(Refused::Duplicate);
    }
    let replaces = list.iter().any(|e| e.slot == slot);
    if !replaces && list.len() >= game_data::decals::MAX_PER_ITEM {
        return Err(Refused::Full);
    }
    list.retain(|e| e.slot != slot);
    let row = DecalRow { slot, decal_uid: decal, dict: d, x_bits };
    list.push(row);
    list.sort_by_key(|e| e.slot);
    Ok(row)
}

/// Remove the entry (`decal`, `slot`) of `item`; false when there is none.
pub fn detach(p: &mut accounts::Profile, item: u32, decal: u32, slot: u8) -> bool {
    let Some(list) = p.decals.get_mut(&item) else { return false };
    let before = list.len();
    list.retain(|e| !(e.decal_uid == decal && e.slot == slot));
    let gone = list.len() != before;
    if list.is_empty() {
        p.decals.remove(&item);
    }
    gone
}

/// Set the placement value of an entry (45 04); false when there is none.
pub fn set_placement(p: &mut accounts::Profile, item: u32, decal: u32, slot: u8, x_bits: u64) -> bool {
    match p.decals.get_mut(&item).and_then(|l| l.iter_mut().find(|e| e.decal_uid == decal && e.slot == slot)) {
        Some(e) => {
            e.x_bits = x_bits;
            true
        }
        None => false,
    }
}

/// 45 01.
pub fn relocate(cx: &mut Ctx, peer: &mut Peer, item: u32, decal: u32, slot: u8, x_bits: u64) {
    let gd = &cx.lobby.data;
    let rw = cx.lobby.cfg.achievement_rewards;
    let Some(res) = update(cx, peer, |p| {
        let row = attach(gd, p, item, decal, slot, x_bits)?;
        // achievement 403 ("Change appearance of an item")
        Ok::<_, Refused>((row, crate::achievements::event_in(gd, rw, p, crate::achievements::Event::Customised)))
    }) else {
        return;
    };
    match res {
        Ok((row, (changes, before))) => {
            log::info!("45 01: decal item {decal} (dict {}) on item {item} slot {slot}", row.dict);
            peer.reply(Reply::Inventory(Inventory45::Camo { item, id: decal, x: slot, value: f64::from_bits(x_bits) }));
            crate::achievements::push_changes(cx, peer, &changes, before);
        }
        Err(why) => log::info!("45 01 item {item} decal {decal} slot {slot}: refused ({why:?})"),
    }
}

/// 45 02.
pub fn drop(cx: &mut Ctx, peer: &mut Peer, item: u32, decal: u32, slot: u8) {
    let Some(gone) = update(cx, peer, |p| detach(p, item, decal, slot)) else { return };
    if gone {
        log::info!("45 02: decal item {decal} removed from item {item} slot {slot}");
        peer.reply(Reply::Inventory(Inventory45::CamoDrop { item, id: decal, slot }));
    } else {
        log::info!("45 02 item {item} decal {decal} slot {slot}: no such entry");
    }
}

/// 45 04: the editor stores the placement the client already applied. No answer (the client reads none).
pub fn save(cx: &mut Ctx, peer: &mut Peer, item: u32, decal: u32, x_bits: u64, slot: u8) {
    let Some(ok) = update(cx, peer, |p| set_placement(p, item, decal, slot, x_bits)) else { return };
    if !ok {
        log::info!("45 04 item {item} decal {decal} slot {slot}: no such entry");
    }
}

/// The dictionary ids of the decals and camouflages worn by the equipped armour (the entries of every item in a slot).
pub fn worn_dicts(p: &accounts::Profile) -> Vec<u16> {
    let mut out = vec![];
    for uid in p.slots.values() {
        if let Some(list) = p.decals.get(uid) {
            out.extend(list.iter().map(|d| d.dict));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use accounts::Profile;
    use game_data::GameData;

    fn setup() -> (GameData, Profile, u32, u32, u32) {
        let gd = game_data::testing::tables();
        let mut p = Profile::starter();
        let torso = p.items.iter().find(|i| gd.items.category(i.dict_id) == 3).unwrap().uid;
        let decal = p.add_item(718, 718);
        let camo = p.add_item(154, 154);
        (gd, p, torso, decal, camo)
    }

    #[test]
    fn attach_replace_and_limits() {
        game_data::require_tables!();
        let (gd, mut p, torso, decal, camo) = setup();
        assert!(attach(&gd, &mut p, torso, decal, 1, 7).is_ok());
        assert!(attach(&gd, &mut p, torso, camo, 3, 8).is_ok());
        // the editor's slots: camouflage only in 3, decals only in 0 and 1
        assert_eq!(attach(&gd, &mut p, torso, camo, 0, 8), Err(Refused::WrongSlot));
        let d2 = p.add_item(709, 709);
        assert_eq!(attach(&gd, &mut p, torso, d2, 3, 8), Err(Refused::WrongSlot));
        assert_eq!(attach(&gd, &mut p, torso, d2, 2, 8), Err(Refused::WrongSlot));
        // one item twice on a piece
        assert_eq!(attach(&gd, &mut p, torso, decal, 0, 9), Err(Refused::Duplicate));
        // the same slot replaces
        assert!(attach(&gd, &mut p, torso, d2, 1, 5).is_ok());
        assert_eq!(p.decals[&torso].len(), 2);
        let d3 = p.add_item(904, 904);
        assert!(attach(&gd, &mut p, torso, d3, 0, 1).is_ok());
        assert_eq!(p.decals[&torso].len(), 3, "two decals and one camouflage");
        // weapon and boots refuse
        let weapon = p.items.iter().find(|i| gd.items.is_weapon(i.dict_id)).unwrap().uid;
        let d4 = p.add_item(904, 904);
        assert_eq!(attach(&gd, &mut p, weapon, d4, 0, 1), Err(Refused::DoesNotFit));
    }

    #[test]
    fn one_owner_and_cleanup() {
        game_data::require_tables!();
        let (gd, mut p, torso, decal, _) = setup();
        let legs = p.add_item(gd.items.iter().find(|i| i.category == 5).unwrap().dict_id, 1);
        attach(&gd, &mut p, torso, decal, 0, 1).unwrap();
        assert_eq!(attach(&gd, &mut p, legs, decal, 0, 1), Err(Refused::OwnedElsewhere));
        assert_eq!(p.decal_owner(decal), Some(torso));
        assert!(set_placement(&mut p, torso, decal, 0, 99));
        assert_eq!(p.decals[&torso][0].x_bits, 99);
        // selling the decal item removes the entry
        p.remove_item(decal);
        assert!(p.decals.is_empty());
        assert!(!detach(&mut p, torso, decal, 0));
    }

    /// dict 184 `camo_fabric_grey` carries (128, 1) = `edge_rep_booster` +1 %: reputation bonus for faction 4 (border)
    /// only, the faction the match reputation goes to (the active one). The starter's scavenger kit carries (125, 3) =
    /// +2 % for faction 1 the same way (faction armour: group 125..128 = the item's own `faction` 1..4 in the data).
    #[test]
    fn faction_reputation_camouflage_counts_for_its_faction_only() {
        game_data::require_tables!();
        let (gd, mut p, torso, _, _) = setup();
        let rep = |p: &mut Profile, f: u8| {
            p.faction = f;
            crate::boosters::worn_bonus(&gd, p)[2]
        };
        let (kit1, kit4) = (rep(&mut p, 1), rep(&mut p, 4));
        assert!((kit1 - kit4 - 0.02).abs() < 1e-6, "scavenger kit {kit1} vs {kit4}");
        let camo = p.add_item(184, 184);
        attach(&gd, &mut p, torso, camo, 3, 0).unwrap();
        assert!((rep(&mut p, 4) - kit4 - 0.01).abs() < 1e-6, "the camouflage adds 1 % for faction 4");
        assert!((rep(&mut p, 1) - kit1).abs() < 1e-6, "and nothing for faction 1");
        p.faction = 4;
        // the result window's decal row shows it too
        let parts = crate::boosters::reward_percent_parts(&gd, &p, 0, &[]);
        assert_eq!(parts[crate::boosters::BONUS_DECALS][2], 1);
    }

    #[test]
    fn worn_bonus_follows_the_worn_piece() {
        game_data::require_tables!();
        let (gd, mut p, torso, decal, _) = setup();
        let none = crate::boosters::worn_bonus(&gd, &p);
        attach(&gd, &mut p, torso, decal, 0, 1).unwrap();
        // dict 718 = char_decal_5_29, default_modifications [(124, 6)]
        let gain = gd.progression.reward_bonus([(124, 6)]);
        assert!(gain[0] > 0.0, "(124, 6) is an experience bonus");
        let worn = crate::boosters::worn_bonus(&gd, &p);
        assert!((worn[0] - none[0] - gain[0]).abs() < 1e-9, "{worn:?} vs {none:?} + {gain:?}");
        assert_eq!(super::worn_dicts(&p), vec![718]);
        // taken off: the bonus is gone
        p.slots.retain(|_, u| *u != torso);
        assert!(super::worn_dicts(&p).is_empty());
        assert!(crate::boosters::worn_bonus(&gd, &p)[0] <= none[0]);
    }
}
