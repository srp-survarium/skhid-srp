//! Shop: buy 46 00, sell 46 01, exchange 46 03.

use accounts::{Currency, ItemRow};
use game_data::shop::Wallet;
use protocol::s2c::*;

use crate::build;
use crate::dispatch::{update, Ctx};
use crate::peer::Peer;

pub fn currency(w: Wallet) -> Currency {
    match w {
        Wallet::Silver => Currency::Silver,
        Wallet::Gold => Currency::Gold,
        Wallet::Parts => Currency::Parts,
        Wallet::League => Currency::League,
    }
}

/// Text of a `46 00` refused for an item with no price, or a faction-free item above the best faction level
/// (localisation `st_cannot_buy_item`: "Cannot buy the item"; CHOICE of key).
pub const CANNOT_BUY_TEXT: &str = "st_cannot_buy_item";

/// Text of a `46 00` refused for too little money (`st_not_enough_money`, also the `84 48` profile refusal).
pub const NO_MONEY_TEXT: &str = "st_not_enough_money";

/// Text of a `46 00` refused because the faction level of a faction item (1..=4) is not reached:
/// `st_item_sell_precondition_faction_<f>` ("Reputation with <faction> required", present in all 11 languages; no
/// client code reads these keys: LIKELY the original server's refusal text).
pub fn faction_text(faction: u32) -> String {
    if (1..=4).contains(&faction) {
        format!("st_item_sell_precondition_faction_{faction}")
    } else {
        CANNOT_BUY_TEXT.into()
    }
}

/// 46 00: `u16 dict, u32 count, u8 slot, u32 profile, u8 gold`. Answer `83 46 00 <flag> <n> <records>` and 8B.
/// A refusal is `83 46 00 00 00` (it unlocks the shop window), 8B, then `84 46` with the reason's key: the client shows
/// it as a system message (display 0x200176BF8, text path).
///
/// A locked faction tile still opens the trade dialog: the client's force-buy path (`canForceBuyPrem`, gold lock icon)
/// offers the item for the 8A `c` gold price (`st_force_unlock_for_gold`) and sends `gold` = 1. With `c` = 0 the dialog
/// shows the silver price and sends `gold` = 0, so the buy is refused here by [`crate::progress::shop_allows`] (the
/// client's own rule). OPEN: the gold unlock price per item (no source gives one).
pub fn buy(cx: &mut Ctx, peer: &mut Peer, dict: u16, count: u32, gold: u8) {
    let gd = &cx.lobby.data;
    let outcome = update(cx, peer, |p| -> Result<protocol::items::ItemRecord, String> {
        let total = gd.shop.buy_total(&gd.items, dict, count);
        let mut wallet = currency(gd.shop.wallet(dict));
        // camouflage / decals that the faction shop table does not list are sold at the dictionary price (decals.rs)
        let (total, wallet) = match (total, gd.decals.dictionary_price(dict)) {
            (None, Some((cost, gold))) => {
                wallet = if gold { Currency::Gold } else { Currency::Silver };
                (Some(u64::from(cost)), wallet)
            }
            (t, _) => (t, wallet),
        };
        let Some(total) = total else {
            log::info!("46 00 dict {dict} x{count} gold {gold}: no price, refused");
            return Err(CANNOT_BUY_TEXT.into());
        };
        if !crate::progress::shop_allows(gd, p, dict) {
            let (faction, need) = gd.items.get(dict).map_or((0, 0), |it| (it.faction, it.item_level));
            let have = if (1..=4).contains(&faction) { game_data::levels::faction_level(p.points(faction as u8)) } else { 0 };
            log::info!(
                "46 00 dict {dict} x{count} gold {gold}: faction {faction} level {have} below item level {need}, refused"
            );
            return Err(faction_text(faction));
        }
        if p.spend(wallet, total).is_err() {
            log::info!("46 00 dict {dict} x{count} refused (price {total}, have {})", p.balance(wallet));
            return Err(NO_MONEY_TEXT.into());
        }
        let stack = gd.items.is_stackable(dict);
        let existing = if stack { p.items.iter().find(|i| i.dict_id == dict).map(|i| i.uid) } else { None };
        let (uid, shown) = match existing {
            // same uid: the client adds the amount to the stack it has
            Some(uid) => {
                p.item_mut(uid).unwrap().count += count;
                (uid, count)
            }
            None => {
                let value = if stack { count } else { gd.items.amount_field(dict, 100) };
                let uid = p.add_item(dict, value);
                // the client buys modules in the workshop (60 01); a module bought here would be green, never grey
                crate::chests::give_rarity(gd, p, uid, &mut crate::fresh_roll());
                (uid, value)
            }
        };
        let row: ItemRow = *p.item(uid).unwrap();
        log::info!("46 00 bought dict {dict} x{count} for {total}, uid {uid}");
        Ok(build::record_value(gd, p, &row, shown))
    });
    match outcome {
        // no profile (not signed in, or the store failed): still answer, the refusal unlocks the shop window
        None => {
            log::info!("{}: 46 00 dict {dict} x{count}: profile not available, refused", peer.addr);
            peer.reply(Reply::Shop(Shop46::Buy { flag: 0, items: vec![] }));
        }
        Some(Ok(record)) => {
            peer.reply(Reply::Shop(Shop46::Buy { flag: 0, items: vec![record] }));
            send_money(cx, peer);
        }
        Some(Err(text)) => {
            peer.reply(Reply::Shop(Shop46::Buy { flag: 0, items: vec![] }));
            send_money(cx, peer);
            log::info!("{}: 46 00 dict {dict}: refusal text {text}", peer.addr);
            peer.push(&S2c::Denied(Denied { request: 0x46, body: DeniedBody::Text(text) }));
        }
    }
}

/// 46 01: `u32 item, u32 amount`. Answer `83 46 01 <item> <amount>` (stacks lose the amount, other items are removed)
/// and 8B. An item the account does not own gets no answer.
pub fn sell(cx: &mut Ctx, peer: &mut Peer, item: u32, amount: u32) {
    let gd = &cx.lobby.data;
    let sold = update(cx, peer, |p| {
        let row = *p.item(item)?;
        let stack = gd.items.is_stackable(row.dict_id);
        // a weapon wearing a skin sells as the weapon it is, not as the skin variant
        let (amount, gain) = gd.shop.sell_gain(&gd.items, p.real_dict(&row), amount, row.count);
        // a sold decal leaves the piece it sat on: that piece's record is sent again below
        let decal_owner = p.decal_owner(item);
        // modules installed on a sold weapon go back into the inventory, free (0.63 dev notes: "when selling a
        // weapon its modules are removed automatically"); the client still shows them attached to the weapon
        let freed: Vec<u32> = p.attach.get(&item).map(|m| m.values().copied().collect()).unwrap_or_default();
        if stack && row.count > amount {
            // caps the rounds placed in every profile's ammo and pocket slots to what is left
            p.consume(item, amount);
        } else {
            p.remove_item(item);
        }
        p.credit(Currency::Silver, gain);
        log::info!("46 01 sold item {item} (dict {}) x{amount} for {gain}, {} modules freed", row.dict_id, freed.len());
        let mut freed: Vec<u32> = freed.into_iter().filter(|u| p.item(*u).is_some()).collect();
        freed.extend(decal_owner.filter(|o| p.item(*o).is_some()));
        Some((amount, build::records(gd, p, &freed)))
    });
    let Some(Some((amount, freed))) = sold else {
        log::info!("46 01: item {item} not owned");
        return;
    };
    peer.reply(Reply::Shop(Shop46::Sell { item, amount }));
    if !freed.is_empty() {
        peer.reply(Reply::Inventory(Inventory45::Records { x: 0, items: freed }));
    }
    send_money(cx, peer);
}

/// 46 03: `u32 gold`. No answer; silver += gold x 250, then 8B.
pub fn exchange(cx: &mut Ctx, peer: &mut Peer, gold: u32) {
    update(cx, peer, |p| {
        let (spent, gained) = game_data::craft::exchange(gold, p.gold);
        let _ = p.spend(Currency::Gold, spent as u64);
        p.credit(Currency::Silver, gained);
        log::info!("46 03 exchange {spent} gold -> {gained} silver");
    });
    send_money(cx, peer);
}

/// 8B with the account's current wallet.
pub fn send_money(cx: &mut Ctx, peer: &mut Peer) {
    let Some(id) = peer.account else { return };
    match cx.lobby.store.load_profile(id) {
        Ok(p) => peer.push(&S2c::Money(build::money_8b(&p))),
        Err(e) => log::error!("loading profile of {id}: {e}"),
    }
}
