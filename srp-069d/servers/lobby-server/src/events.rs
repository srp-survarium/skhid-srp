//! The event: the `94` record 23 that opens the exchange window, the three event currencies in the wallet (8B), what a
//! finished online match pays, and the `48 28` exchange. Research, evidence and open points: `docs/notes/events.md`.
//!
//! PROVEN (exe 0.69d + lobby movie): an event is a `94` record of type 23 (`service_dict_id` 23); it opens the
//! `ItemsExchangeEventWindow` (`new_year_event`). The window shows the countdown `actionTime` (the record's second u32,
//! seconds), the texts of the record's `ui` (first body, [`ui_blob`]), the exchange rows of the record's second body
//! ([`exchange_body`]) and the balances of the 8B wallet. The second body must not be empty (0x2001C63EE: an empty one
//! skips the exchange rows).
//!
//! The dates are NOT in any file: [`LobbyConfig::event`] is an input, absent by default (no record, no drops, no exchange).

use accounts::{Currency, Profile};
use game_data::chests::Reward;
use game_data::events::{self, Schedule};
use game_data::rewards::MatchDrops;
use protocol::binconfig::{self, Value};
use protocol::s2c::*;

use crate::dispatch::{update, Ctx};
use crate::peer::Peer;
use crate::premium::unix_now;
use crate::shop::send_money;

/// `94` record type of a running event (`service_needed` of the drop rows).
pub const KIND: u8 = events::SERVICE;

/// The window's default header: the client's own text `st_whats_new_051_ny_event` (English). CHOICE (the real server sent
/// its own, which no file has); replace through the schedule's `label` (srp: `header` of an `[[events]]` entry).
pub const DEFAULT_LABEL: &str = "st_merry_christmas";
/// The window's default text: the client's `st_global_event_currency2_descr`. CHOICE, replaceable.
pub const DEFAULT_DESCRIPTION: &str = "st_global_event_currency2_descr";

/// The wallet currency of slot `0..=2`.
pub fn currency(slot: usize) -> Currency {
    [Currency::Event1, Currency::Event2, Currency::Event3][slot]
}

/// The record's `ui` body: `[ {type: "TextComponent", objectData: {label, description}} ]` (`label` -> header,
/// `description` -> title; the text after `bottom=` goes to a small line below). LIKELY that the root may also be an
/// indexed table. Returned as the record carries it: a raw LZMA2 stream of at most 0x800 bytes (PROVEN: the 94 status
/// loop decodes it like the 8A, 0x2001C5A49, before the config read 0x2000CC680).
pub fn ui_blob(label: &str, description: &str) -> Vec<u8> {
    let build = |l: &str, d: &str| {
        binconfig::encode_value(&Value::List(vec![Value::Table(vec![
            ("type".to_string(), Value::Str("TextComponent".to_string())),
            (
                "objectData".to_string(),
                Value::Table(vec![("label".to_string(), Value::Str(l.to_string())), ("description".to_string(), Value::Str(d.to_string()))]),
            ),
        ])]))
    };
    let (l, d) = (if label.is_empty() { DEFAULT_LABEL } else { label }, if description.is_empty() { DEFAULT_DESCRIPTION } else { description });
    let b = protocol::lzma2::compress(&build(l, d));
    if b.len() <= 0x800 {
        b
    } else {
        protocol::lzma2::compress(&build(DEFAULT_LABEL, DEFAULT_DESCRIPTION))
    }
}

/// Exchange rows the client holds (0x2001B8EF0 fills entries of 16 bytes from +4 up to the next field at +0x64).
pub const MAX_EXCHANGE_ROWS: usize = 6;

/// The record's second body before LZMA2, as the type 0x17 reader 0x2001B8EF0 reads the DECODED bytes (PROVEN, status
/// builder 0x2001C6350): `u8 n` (<= 6), n x `{u16 source_dict_id, u32 source_amount, u16 target_dict_id, u32 target_amount}` (the
/// builder turns each into an `actionItemsData` entry with these keys), `u8 k` (<= 32) x `{u16, u8 m (<= 4), m x {u16, u32}}`
/// (stored at +0x64; no exe code reads it by offset: k = 0), `u8 only_exchange` (the action's `only_exchange`; set, it makes
/// `ActionsList.gameModeActionPresent(23)` false: no event game mode, which the server does not run). The rows are
/// `event_exchange_action_items.options` `exchange_items` (`game_data::hero::ExchangeData`), the table `48 28` checks.
pub fn exchange_body(rows: &[game_data::hero::Exchange]) -> Vec<u8> {
    let rows = &rows[..rows.len().min(MAX_EXCHANGE_ROWS)];
    let mut b = vec![rows.len() as u8];
    for r in rows {
        b.extend_from_slice(&r.source.to_le_bytes());
        b.extend_from_slice(&r.source_amount.to_le_bytes());
        b.extend_from_slice(&r.target.to_le_bytes());
        b.extend_from_slice(&r.target_amount.to_le_bytes());
    }
    b.push(0); // k: no second list
    b.push(1); // only_exchange
    b
}

/// The record of the exchange window, present while the window is open. Its second body is the raw LZMA2 stream of
/// [`exchange_body`] (types 8 / 0x17 / 0x1B / 0x2D / 0x2F are decoded before their reader, PROVEN; an empty stream would
/// make the reader run past the decoded bytes).
pub fn notice(gd: &game_data::GameData, s: &Schedule, now: i64) -> Option<Notice94> {
    s.window_open(now).then(|| Notice94 {
        f30: 1,
        f38: s.left(now),
        kind: KIND,
        data1: ui_blob(&s.label, &s.description),
        data2: protocol::lzma2::compress(&exchange_body(&gd.exchange.exchange_items)),
        ..Default::default()
    })
}

/// Add the event record to a `94` message (a no-op without a schedule or outside the window).
pub fn with_event(cx: &Ctx, mut m: Notices94) -> Notices94 {
    let now = unix_now();
    if let Some(n) = cx.lobby.cfg.event_at(now).and_then(|s| notice(&cx.lobby.data, s, now)) {
        m.notices.push(n);
    }
    m
}

/// The profile claim of an event's first-win item (one per event, keyed by its start).
pub fn first_win_claim(s: &Schedule) -> String {
    format!("event_first_win:{}", s.start)
}

/// Add what a finished match pays to `drops.event` (and the win item to `drops.items`): the account's worn winter hat bonus
/// is added flat to each currency. `place` = the player's place in his team by score (0 = best, [`events::place_by_score`]);
/// None = not observed: nothing is paid (a place is part of every row, nothing is invented). A premium account gets +25 %.
/// No schedule or no running event: nothing.
pub fn add_to_drops(cx: &Ctx, p: &Profile, mode: u32, won: bool, place: Option<u8>, now: i64, drops: &mut MatchDrops) {
    let Some(s) = cx.lobby.cfg.event_at(now).filter(|s| s.active(now)) else { return };
    let gd = &cx.lobby.data;
    let Some(place) = place else {
        log::info!("event: no place observed for this match, no event currency is paid");
        return;
    };
    let mut pay = gd.events.match_pay(mode, won, place, p.premium_left(now) > 0);
    let base = gd.events.match_pay(mode, won, place, false);
    if pay.iter().any(|&n| n > 0) {
        // the worn winter hats give +1 of each currency, only when the match paid something (they are a bonus of the drop)
        let worn = gd.events.worn_bonus(p.slots.values().filter_map(|u| p.item(*u)).map(|r| r.dict_id));
        for (a, b) in pay.iter_mut().zip(worn) {
            *a += b;
        }
    }
    drops.event = pay;
    // premium and hats: the bonus part the victory screen shows apart
    for s in 0..3 {
        drops.event_bonus[s] = pay[s].saturating_sub(base[s]);
    }
    // `item_to_drop_after_win`: one, on the account's first PvP win of the event (OFFICIAL: "You will receive Winter Hat
    // 2022 for the first win in a PvP-match during the event!"); the claim keeps it to one per event
    let claim = first_win_claim(s);
    if won && !p.claimed(&claim) {
        drops.once.push((claim, gd.events.item_to_drop_after_win, 1));
    }
}

/// Why an exchange was refused (logged; the two a player can cause also get a text, [`refusal_text`]).
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    NoEvent,
    NotInTable,
    NotACurrency,
    NotEnough { need: u32, have: u32 },
    AlreadyOwned,
    UnknownItem,
}

/// The client text for a refusal the player can cause, sent as `84 48 01 <key>` (the generic text channel, localised by
/// the client at 0x200176430, PROVEN): "Not enough money for exchange" / "You already have this item" (localisation
/// keys of the 0.69d table; which keys the original server sent is not recorded: CHOICE). The other refusals need a
/// hand-made request or a closed window and stay silent (the lock ack still goes out).
pub fn refusal_text(e: &Refused) -> Option<&'static str> {
    match e {
        Refused::NotEnough { .. } => Some("st_not_enought_items_for_exchange"),
        Refused::AlreadyOwned => Some("st_item_bought"),
        _ => None,
    }
}

/// Is `dict` a camouflage / decal item (one copy per account: the client marks an owned one "received")?
fn is_decal(gd: &game_data::GameData, dict: u16) -> bool {
    gd.items.get(dict).is_some_and(|i| i.kind == "prm_decl" || i.section == "camo_decal")
}

/// The exchange itself on a profile: check, charge the event currency, grant the target. Returns the item records.
pub fn exchange_on(gd: &game_data::GameData, p: &mut Profile, source: u16, target: u16) -> Result<Vec<protocol::items::ItemRecord>, Refused> {
    let row = gd.exchange.find(source, target).copied().ok_or(Refused::NotInTable)?;
    let slot = events::slot_of(source).ok_or(Refused::NotACurrency)?;
    if !gd.items.contains(target) {
        return Err(Refused::UnknownItem);
    }
    if is_decal(gd, target) && p.items.iter().any(|i| i.dict_id == target) {
        return Err(Refused::AlreadyOwned);
    }
    let have = p.event[slot];
    p.spend(currency(slot), u64::from(row.source_amount)).map_err(|_| Refused::NotEnough { need: row.source_amount, have })?;
    let (records, _) = crate::chests::grant(gd, p, &[Reward { dict: target, amount: row.target_amount }]);
    Ok(records)
}

/// `48 28 u16 source, u16 target` (`action_item_exchanged`): exchange `source_amount` of the event currency `source` for
/// `target_amount` of `target` (`event_exchange_action_items.options`). The client sets a request lock (0x2001EA503) that
/// only an `83 48` / `84 48` clears, so `83 48 03` always goes out; a success first sends the item (`83 46 00`, flag 1: the
/// client adds the record and shows the received-item popup, like the other grants) and the new wallet (8B).
pub fn exchange(cx: &mut Ctx, peer: &mut Peer, source: u16, target: u16) {
    let now = unix_now();
    let open = cx.lobby.cfg.event_at(now).is_some();
    let gd = &cx.lobby.data;
    let done = if open { update(cx, peer, |p| exchange_on(gd, p, source, target)) } else { Some(Err(Refused::NoEvent)) };
    match done {
        Some(Ok(records)) => {
            log::info!("{}: 48 28 exchanged event currency {source} for item {target}", peer.addr);
            peer.push(&S2c::Reply(Reply::Shop(Shop46::Buy { flag: 1, items: records })));
            send_money(cx, peer);
        }
        Some(Err(e)) => {
            log::info!("{}: 48 28 exchange {source} -> {target} refused: {e:?}", peer.addr);
            if let Some(key) = refusal_text(&e) {
                peer.push(&S2c::Denied(Denied { request: 0x48, body: DeniedBody::Text(key.into()) }));
            }
        }
        None => {}
    }
    peer.reply_raw(0x48, &[3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_exists_while_the_window_is_open() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let mut s = Schedule::parse("1000:2000:3000").unwrap();
        s.label = "L".into();
        assert!(notice(&gd, &s, 999).is_none());
        let n = notice(&gd, &s, 1500).unwrap();
        assert_eq!((n.kind, n.f38), (23, 500));
        // the second body: the exchange rows the window shows (0x17 reader 0x2001B8EF0 on the decoded bytes)
        let body = protocol::lzma2::decompress(&n.data2).unwrap();
        assert_eq!(body, exchange_body(&gd.exchange.exchange_items));
        assert_eq!(body.len(), 1 + 3 * 12 + 2);
        assert_eq!(&body[..13], &[3, 0x73, 0x04, 0xC4, 0x09, 0, 0, 0x98, 0x08, 1, 0, 0, 0], "1139 x 2500 -> 2200 x 1");
        assert_eq!(&body[37..], &[0, 1], "no second list, only the exchange");
        assert!(!n.data1.is_empty() && n.data1.len() <= 0x800);
        // the client decodes the first buffer as raw LZMA2, then reads the config
        let body = protocol::lzma2::decompress(&n.data1).unwrap();
        assert!(String::from_utf8_lossy(&body).contains("TextComponent") && n.data1[0] == 0x01, "stored LZMA2 chunk of the config");
        // after the event the window stays, the countdown is 0
        assert_eq!(notice(&gd, &s, 2500).unwrap().f38, 0);
        assert!(notice(&gd, &s, 3000).is_none());
    }

    #[test]
    fn a_long_text_falls_back_to_the_default() {
        let long = "x".repeat(3000);
        assert_eq!(ui_blob("L", &long), ui_blob("", ""));
        assert_ne!(ui_blob("L", "d"), ui_blob("", ""));
    }

    #[test]
    fn exchange_charges_and_grants() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let mut p = Profile::starter();
        // 1139 x 2500 -> 2200 (Barrett New Year 2022)
        assert_eq!(exchange_on(&gd, &mut p, 1139, 2200), Err(Refused::NotEnough { need: 2500, have: 0 }));
        p.credit(Currency::Event3, 2600);
        let recs = exchange_on(&gd, &mut p, 1139, 2200).unwrap();
        assert_eq!((p.event, recs.len()), ([0, 0, 100], 1));
        assert!(p.items.iter().any(|i| i.dict_id == 2200));
        // a second copy of a weapon is allowed (the client's button stays), the money check still applies
        assert!(matches!(exchange_on(&gd, &mut p, 1139, 2200), Err(Refused::NotEnough { .. })));
    }

    #[test]
    fn a_decal_is_given_once() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let mut p = Profile::starter();
        p.credit(Currency::Event1, 2000);
        exchange_on(&gd, &mut p, 951, 2202).unwrap();
        assert_eq!(p.event[0], 1250);
        assert_eq!(exchange_on(&gd, &mut p, 951, 2202), Err(Refused::AlreadyOwned));
        assert_eq!(p.event[0], 1250, "a refused exchange charges nothing");
    }

    #[test]
    fn rows_outside_the_table_are_refused() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let mut p = Profile::starter();
        p.credit(Currency::Event1, 5000);
        assert_eq!(exchange_on(&gd, &mut p, 951, 2200), Err(Refused::NotInTable));
        assert_eq!(exchange_on(&gd, &mut p, 214, 2202), Err(Refused::NotInTable));
        assert_eq!(p.event[0], 5000);
    }
}
