//! Premium account (docs/notes/store.md): the status the client shows (push 94 record kind 0), premium vouchers
//! (45 03 `item_use`) and the match bonus.
//!
//! * Status: record kind 0 of push 94, `f38` = seconds of premium left (PROVEN: 0x2001C56B0, kind 0 stores the record's
//!   value at +8 and sets the "premium" flag; `LobbyMenu.set_premium_left` takes seconds).
//!   No record = no premium.
//! * Voucher: `premium_access_voucher_*` (`voucher_type` 3, `ttl_in_hours` of the dictionary) adds its hours to the
//!   expiry (from the running expiry, else from now) and is consumed. The 0.69d note "the 1 day voucher no longer
//!   activates automatically" is why the player uses it by hand.
//! * Premium window: the 95 premium-time elements ([`PremiumOffer`], [`shop_elem`]) bought with `46 02` ([`buy_offer`]);
//!   the 7-day actions-tab card is bought with `5C 01` ([`buy_card`]).
//! * Bonus: the client's own premium popup lists +50 % for details (spare parts), silver, reputation and
//!   experience (AS `PremiumPopup.configUI`, PROVEN text): [`MATCH_BONUS_PERCENT`]. The unlock of all three player
//!   profiles (official 0.63) has no server side here: the lobby keeps one profile per account.

use std::time::{SystemTime, UNIX_EPOCH};

use accounts::Profile;
use protocol::s2c::*;

use crate::dispatch::{update, Ctx};
use crate::peer::Peer;
use crate::shop::send_money;

/// Bonus of an active premium account on match silver, experience, reputation (AS `PremiumPopup`, PROVEN text).
pub const MATCH_BONUS_PERCENT: u32 = 50;

/// `B1` type of the client's own "premium bought" notice (0x2001C3D07 / sub 1 at 0x2001C3EA3, PROVEN): the client
/// formats `st_premium_bought_for_n_days` ("Days of Premium: %d! " in English, every language from the client's own
/// localisation) with the second bracketed number, shows a message notification and acks with `51`; it changes no state.
pub const KIND_PREMIUM_BOUGHT: u8 = 0x14;

/// The text of [`KIND_PREMIUM_BOUGHT`]: `[1][days]` (sub 1 = premium bought; `n` is printed unchanged, so it is DAYS,
/// PROVEN). Hours that are not whole days round to the nearest day, at least 1 (CHOICE; every offer is whole days).
pub fn bought_text(hours: u32) -> String {
    format!("[1][{}]", ((hours + 12) / 24).max(1))
}

pub fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Push 94 with the premium status of the profile (no records without premium: the smallest 94).
pub fn notices(gd: &game_data::GameData, p: &Profile, now: i64) -> Notices94 {
    let left = p.premium_left(now);
    let mut m = Notices94::default();
    if left > 0 {
        m.notices.push(Notice94 { f38: left, kind: 0, ..Default::default() });
    }
    // selected boosters (record type 0x23) and service boosters in stock (crate::boosters)
    m.notices.extend(crate::boosters::records(gd, p));
    // hero of the day status (kind 0x19)
    m.notices.extend(crate::hero::notice(gd, p, now));
    // the Winner's Contract (kind 0x16 + the 94 tail; contract.rs)
    if let Some((n, t)) = crate::contract::notice(p, now) {
        m.notices.push(n);
        m.tail = Some(t);
    }
    m
}

/// A premium-time offer of the premium window (one 95 element with `+9 = 0`, bought with `46 02`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PremiumOffer {
    /// the element id (`+0`): the client sorts the options by it and sends it back in `46 02`
    pub id: u32,
    /// `game.options` `shop_bundles` id (`+4`): the option's label and tooltip are that bundle's `ui_descr` texts. It MUST
    /// be a bundle id: the builder looks it up with `items_dictionary::bundle_item_by_id` (0x2002DFF60), which asserts on an
    /// unknown id (an item id such as the voucher 213 crashes the client, PROVEN)
    pub bundle: u32,
    /// gold price (`+0xC`, shown and charged)
    pub gold: u32,
    /// premium hours the purchase adds (the sub-record's `s5`)
    pub hours: u32,
}

/// The "saved" percentage's base rate: the 1-day voucher (dict 213, `premium_access_voucher_1d`), 50 gold for 24 h
/// (`data/vouchers_069d.json`, PROVEN data). CHOICE: it only drives the percentage the window shows.
pub const BASE_GOLD: u32 = 50;
pub const BASE_HOURS: u32 = 24;
/// `+0x488` money type of a premium element: 1 = gold (the window always charges gold, `Payment.MONEY_PREMIUM`; the
/// builder copies the byte into `money_type`, which `PremiumPopup` never reads).
const MONEY_GOLD: u8 = 1;

/// The default premium window (CHOICE): the 0.6x web-shop gold prices (OFFICIAL, Wayback 2021-05-07) on the four
/// `shop_bundles` rows whose texts name those durations: 1 "3 days", 2 "Week", 3 "Month", 4 "3 months". The actions tab
/// also sells 7 days as a card ([`crate::actions::default_offers`]; card texts are client texts, and only 1 / 2 / 3 /
/// 7-day premium texts exist).
pub fn default_offers() -> Vec<PremiumOffer> {
    vec![
        PremiumOffer { id: 1, bundle: 1, gold: 150, hours: 72 },
        PremiumOffer { id: 2, bundle: 2, gold: 300, hours: 168 },
        PremiumOffer { id: 3, bundle: 3, gold: 900, hours: 720 },
        PremiumOffer { id: 4, bundle: 4, gold: 2500, hours: 2160 },
    ]
}

/// The 95 element of a premium offer (reader 0x2001805A0, builder 0x200143F30, PROVEN): `+9 = 0` (premium time), one
/// sub-record `{s0 0, s1 base gold, s5 hours, s7 base hours, s11 1, s12 1}`. The window shows
/// `discount = trunc((full - price) / full * 100)` with `full = trunc(s5 / s7 * s1)` (sub-records with `s11 == 1`; none =
/// division by zero), so 3 days shows nothing (0 %), a week 14 %, a month 40 %, 3 months 44 %. `s12 == 1` with `s0 == 0`
/// keeps the line out of the bundle item list (LIKELY "a premium-time line"); `s5` = hours is also what the level-up premium
/// proposal 0x2001C1F90 compares with days x 24 to pick the offer it proposes (LIKELY).
pub fn shop_elem(o: &PremiumOffer) -> ShopElem {
    let mut vals = [0u32; 10];
    vals[1] = BASE_GOLD;
    vals[5] = o.hours;
    vals[7] = BASE_HOURS;
    ShopElem { f0: o.id, dict: o.bundle, f8: 0, f9: 0, fc: o.gold, f488: MONEY_GOLD, subs: vec![ShopSub { vals, a: 0, b: 1, c: 1 }] }
}

/// Take `gold` and add `hours` of premium in one transaction, then push 8B and the 94 with the new premium time. False
/// (nothing changed) without enough gold.
fn sell_premium(cx: &mut Ctx, peer: &mut Peer, gold: u32, hours: u32, what: &str) -> bool {
    let now = unix_now();
    let done = update(cx, peer, |p| {
        p.spend(accounts::Currency::Gold, u64::from(gold)).ok()?;
        p.extend_premium(now, i64::from(hours) * 3600);
        Some(p.premium_until)
    });
    match done {
        Some(Some(until)) => {
            log::info!("{}: {what}: {gold} gold, +{hours} h premium, premium until {until}, B1 0x14 {}", peer.addr, bought_text(hours));
            send_money(cx, peer);
            push_status(cx, peer);
            peer.push(&S2c::SystemMessage(SystemMessage {
                kind: KIND_PREMIUM_BOUGHT,
                text: bought_text(hours),
                message_id: crate::notices::next_message_id(),
                ..Default::default()
            }));
            true
        }
        _ => {
            log::info!("{}: {what}: refused (not enough gold for {gold})", peer.addr);
            false
        }
    }
}

/// `46 02 u32 offer id, u32 match id, f32 0.0` (`accept_premium` 0x24B890 from the lobby with match id 0, or
/// `accept_premium_victory_screen` 0x290E20 with the match id; 0x187E90, PROVEN): buy a premium-window offer. The client
/// reads no answer (`83 46` has no sub 2 case); a bought offer shows through 8B (gold) and 94 (premium time). An unknown
/// offer or too little gold changes nothing (logged). A bought offer also gets the client's own "premium bought" notice
/// ([`bought_text`]).
pub fn buy_offer(cx: &mut Ctx, peer: &mut Peer, offer: u32, match_id: u32) {
    let Some(o) = cx.lobby.cfg.premium_offers.iter().find(|o| o.id == offer).cloned() else {
        log::info!("{}: 46 02 premium offer {offer} (match {match_id}): no such offer", peer.addr);
        return;
    };
    sell_premium(cx, peer, o.gold, o.hours, &format!("46 02 premium offer {offer} (bundle {}, match {match_id})", o.bundle));
}

/// `5C 01` of a premium card of the actions tab: the same purchase for the card's gold and hours. Returns the `BC sub 1`
/// result.
pub fn buy_card(cx: &mut Ctx, peer: &mut Peer, card: u32, gold: u32, hours: u32) -> bool {
    sell_premium(cx, peer, gold, hours, &format!("5C buy {card} (premium card)"))
}

/// `amount` plus the premium bonus when the account has premium at `now`.
pub fn bonus(p: &Profile, now: i64, amount: u32) -> u32 {
    if p.premium_left(now) > 0 {
        amount + (amount as u64 * MATCH_BONUS_PERCENT as u64 / 100) as u32
    } else {
        amount
    }
}

/// What the premium account adds to a match's base reward `[experience, silver, reputation]`: [`MATCH_BONUS_PERCENT`] of
/// each, rounded down. The `97` premium row (score block 2) of a player without premium shows it (online.rs `pay`): the
/// lobby movie draws that row dimmed (`PREMIUM_COLOR_SECONDARY`) and builds "total premium" = "total" + the row; its
/// compact panel labels the premium bonus 50 % on every profile.
pub fn match_bonus(base: [u32; 3]) -> [u32; 3] {
    base.map(|v| (u64::from(v) * u64::from(MATCH_BONUS_PERCENT) / 100) as u32)
}

/// Re-send the status after a change.
pub fn push_status(cx: &mut Ctx, peer: &mut Peer) {
    let Some(id) = peer.account else { return };
    if let Some(m) = status_msg(cx, id) {
        peer.push(&m);
    }
}

/// The status push (94) of an account that may have no peer at hand.
pub fn status_msg(cx: &Ctx, account: accounts::AccountId) -> Option<S2c> {
    let p = cx.lobby.store.load_profile(account).ok()?;
    Some(S2c::Notices(crate::login_calendar::with_calendar(cx, &p, notices(&cx.lobby.data, &p, unix_now()))))
}

/// 45 03 `u32 item` for a premium voucher: consume one, extend the expiry. `83 45 03 <item>` (the client lowers the
/// stack; for the last one it removes the row, the same record the consumables use), push 94. Returns false when the
/// item is not a premium voucher of this profile (the caller tries the other item uses).
pub fn use_voucher(cx: &mut Ctx, peer: &mut Peer, item: u32) -> bool {
    let gd = &cx.lobby.data;
    let now = unix_now();
    let done = update(cx, peer, |p| {
        let row = *p.item(item)?;
        let hours = gd.vouchers.premium_hours(row.dict_id)?;
        if row.count > 1 {
            p.item_mut(item).unwrap().count -= 1;
        } else {
            p.remove_item(item);
        }
        p.extend_premium(now, hours as i64 * 3600);
        log::info!("45 03 premium voucher {item} (dict {}): +{hours} h, premium until {}", row.dict_id, p.premium_until);
        Some(())
    });
    if done != Some(Some(())) {
        return false;
    }
    peer.reply(Reply::Inventory(Inventory45::Used { item }));
    push_status(cx, peer);
    send_money(cx, peer);
    true
}
