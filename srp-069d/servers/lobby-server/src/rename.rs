//! Rename voucher (`rename_account_voucher`, dict 152, `voucher_type` 0): 48 05 `change_player_nickname`.
//!
//! * Request `48 05 str nickname, u8 face, u32 voucher uid` (builder 0x200290BC0: the first Flash argument is the
//!   nickname, the next two go to the `u8` and the `u32`; the `u32` being the voucher's inventory uid is LIKELY: the
//!   answer below returns it and the reader uses it as an inventory row uid). PROVEN layout, LIKELY meaning.
//! * Answer `83 48 05 u8 ok [u32 uid, u8 face]` (reader 0x200192B68, PROVEN): ok = 0 ends the wait; else the client
//!   lowers the stack of the inventory row `uid` by one (removing the row at 0) and stores the face. The server pushes
//!   8F (the account name) afterwards.
//! * Rules: the voucher must be a rename voucher the account owns; the name follows the account rules (1..=32
//!   characters, no leading / trailing space, no control character: `accounts::Store::rename_account`) and must be
//!   free (case-insensitive, nicknames only: login names are private). 48 04 validation answers for real
//!   ([`validate`]) with the reason codes the exe reader knows ([`refusal_reason`]).
//! * The FIRST nickname is free: the onboarding nickname screen sends uid 0xFFFFFFFF while the account has none
//!   ([`change`] -> `first_nickname`, docs/notes/onboarding.md).
//! * Later renames without a voucher: the window's "pay with gold" option sends uid 0 (PROVEN, `MessageEnterName`) and
//!   shows the price 8D `costs[3]` (`set_change_name_cost`). The price is the rename voucher's own dictionary price
//!   ([`gold_price`], 250 gold; CHOICE: no client file carries the real value). Answer
//!   `83 48 05 01, u32 0, face` (uid 0 decrements nothing), then 8B and 8F.
//! * Only the NICKNAME changes (accounts column `nick`, NULL = the login name): sign-in keeps using the login name, every
//!   display (87, 8F, chat, friends, clans, squads, results) reads the nickname, as sign-in and nickname were separate
//!   in the original game.

use accounts::Currency;
use game_data::GameData;
use protocol::s2c::*;

use crate::dispatch::{update, Ctx};
use crate::peer::Peer;

/// `voucher_type` of the rename voucher (data `vouchers_069d.json`).
pub const RENAME_VOUCHER_TYPE: u32 = 0;

/// The voucher uid the client sends from the onboarding nickname screen (`48 05 04 "test" 06 ffffffff`): no
/// voucher, the first nickname of a new player.
pub const NO_VOUCHER: u32 = 0xFFFF_FFFF;

/// Dictionary id of `rename_account_voucher`.
pub const RENAME_VOUCHER: u16 = 152;

/// Gold price of a rename paid without a voucher (8D `costs[3]`): the rename voucher's dictionary `cost` (250 gold),
/// so the two ways cost the same (CHOICE). 0 when the data lacks the row: then the gold path is
/// refused.
pub fn gold_price(gd: &GameData) -> u32 {
    gd.vouchers.get(RENAME_VOUCHER).filter(|v| v.currency == 1).map_or(0, |v| v.cost)
}

/// How many free alternatives a refused 48 04 offers (CHOICE: the reply carries a list, its content is the server's).
const SUGGESTIONS: usize = 3;

/// 48 04 `request_nickname_validation` (the client asks about once a second while the nickname box is open):
/// `83 48 04 str name, u8 valid` (1 = the name can be taken), else `u8 reason, u32 n, n x str` free alternatives.
/// The reason byte selects the text the client shows ([`refusal_reason`], PROVEN from the exe reader of this message).
pub fn validate(cx: &mut Ctx, peer: &mut Peer, name: &str) {
    let Some(id) = peer.account else { return };
    let store = &cx.lobby.store;
    let verdict = store.check_nickname(id, name);
    let valid = u8::from(verdict.is_ok());
    let suggestions = if verdict.is_ok() {
        vec![]
    } else {
        // a well-formed base: the typed name cut to leave room for two digits (30 characters at most)
        let base: String = name.trim().chars().filter(|c| !c.is_control()).take(accounts::MAX_NICKNAME_CHARS - 2).collect();
        if base.is_empty() {
            vec![]
        } else {
            (1..100u32).map(|n| format!("{base}{n}")).filter(|s| store.check_nickname(id, s).is_ok()).take(SUGGESTIONS).collect()
        }
    };
    log::debug!("{}: 48 04 {name:?}: valid {valid} ({verdict:?}), {} suggestions", peer.addr, suggestions.len());
    peer.reply(Reply::Account(Account48::Nickname { name: name.to_string(), valid, reason: verdict.as_ref().err().map_or(0, refusal_reason), suggestions }));
}

/// Reason bytes of a refused `83 48 04` (PROVEN: the exe reader 0x2001920DA maps the byte to a localisation key,
/// native 0x2001C7650 hands the localised text to the movie's `handle_nickname_validation(valid, name, message, names)`,
/// which shows `message` as the warning under the name box). Any other byte shows an empty warning.
pub mod reason {
    /// `st_wrong_symbol_clan_string`: "The string contains non-printable character".
    pub const WRONG_SYMBOL: u8 = 14;
    /// `st_already_exist_clan_string`: "This name is already reserved.".
    pub const ALREADY_EXISTS: u8 = 15;
    /// `st_short_clan_string`: "The name is too short".
    pub const TOO_SHORT: u8 = 21;
    /// `st_long_clan_string`: "The name is too long".
    pub const TOO_LONG: u8 = 22;
    /// `st_space_symbol_clan_string`: "Invalid delimiters".
    pub const BAD_DELIMITERS: u8 = 23;
}

/// The reason byte for a refusal of [`accounts::Store::check_nickname`]: the account rules (`check_name`) map one to
/// one (empty, over 30 characters, leading / trailing space, control character), a used name is `ALREADY_EXISTS`.
/// An error that is not a name verdict (database) sends 0: an empty warning.
pub fn refusal_reason(e: &accounts::Error) -> u8 {
    use accounts::Error;
    match e {
        Error::NameTaken => reason::ALREADY_EXISTS,
        Error::InvalidName("empty") => reason::TOO_SHORT,
        Error::InvalidName("longer than 30 characters" | "longer than 32 characters") => reason::TOO_LONG,
        Error::InvalidName("leading or trailing space") => reason::BAD_DELIMITERS,
        Error::InvalidName("control character") => reason::WRONG_SYMBOL,
        _ => 0,
    }
}

/// 48 05 `change_player_nickname`: the free first nickname of the onboarding screen (no voucher, the account has
/// none yet), a rename paid with gold (uid 0, [`gold_price`]) or a rename with a rename voucher.
pub fn change(cx: &mut Ctx, peer: &mut Peer, name: &str, face: u8, voucher: u32) {
    let Some(id) = peer.account else { return };
    let first = !cx.lobby.store.has_nickname(id).unwrap_or(true);
    if first && (voucher == NO_VOUCHER || voucher == 0) {
        return first_nickname(cx, peer, name, face);
    }
    if voucher == 0 {
        return pay_with_gold(cx, peer, name, face);
    }
    let gd = &cx.lobby.data;
    let owns_voucher = cx
        .lobby
        .store
        .load_profile(id)
        .ok()
        .and_then(|p| p.item(voucher).copied())
        .is_some_and(|row| gd.vouchers.get(row.dict_id).is_some_and(|v| v.category == 27 && v.voucher_type == Some(RENAME_VOUCHER_TYPE)));
    let refuse = |peer: &mut Peer, why: &str| {
        log::info!("{}: 48 05 rename to {name:?} refused: {why}", peer.addr);
        peer.reply(Reply::Account(Account48::NicknameChanged { ok: 0, uid: 0, face: 0 }));
    };
    if !owns_voucher {
        return refuse(peer, "no rename voucher with that uid");
    }
    if let Err(e) = cx.lobby.store.rename_account(id, name) {
        return refuse(peer, &e.to_string());
    }
    let consumed = update(cx, peer, |p| {
        if p.item(voucher).is_some_and(|r| r.count > 1) {
            p.item_mut(voucher).unwrap().count -= 1;
        } else {
            p.remove_item(voucher);
        }
        // the voucher changes "nickname and appearance" (st_rename_account_voucher_descr)
        p.face = face;
    });
    log::info!("{}: 48 05 nickname of account {id} {:?} -> {name:?}, voucher {voucher} consumed {}", peer.addr, peer.name, consumed.is_some());
    peer.name = name.to_string();
    peer.reply(Reply::Account(Account48::NicknameChanged { ok: 1, uid: voucher, face }));
    peer.push(&S2c::Name(NameMsg { flag: 0, name: name.to_string() }));
}

/// A rename paid with gold: the price is taken first and given back when the name is refused, so a failed rename costs
/// nothing. Answer `83 48 05 01, u32 0, face` (no inventory row decremented), then 8B (gold) and 8F (name).
fn pay_with_gold(cx: &mut Ctx, peer: &mut Peer, name: &str, face: u8) {
    let Some(id) = peer.account else { return };
    let price = gold_price(&cx.lobby.data);
    let refuse = |peer: &mut Peer, why: &str| {
        log::info!("{}: 48 05 gold rename to {name:?} refused: {why}", peer.addr);
        peer.reply(Reply::Account(Account48::NicknameChanged { ok: 0, uid: 0, face: 0 }));
    };
    if price == 0 {
        return refuse(peer, "no gold price in the data");
    }
    match update(cx, peer, |p| p.spend(Currency::Gold, u64::from(price))) {
        Some(Ok(())) => {}
        Some(Err(e)) => return refuse(peer, &format!("{e:?}")),
        None => return refuse(peer, "profile not saved"),
    }
    if let Err(e) = cx.lobby.store.rename_account(id, name) {
        update(cx, peer, |p| p.credit(Currency::Gold, u64::from(price)));
        return refuse(peer, &e.to_string());
    }
    update(cx, peer, |p| p.face = face);
    log::info!("{}: 48 05 nickname of account {id} {:?} -> {name:?} for {price} gold", peer.addr, peer.name);
    peer.name = name.to_string();
    peer.reply(Reply::Account(Account48::NicknameChanged { ok: 1, uid: 0, face }));
    crate::shop::send_money(cx, peer);
    peer.push(&S2c::Name(NameMsg { flag: 0, name: name.to_string() }));
}

/// The onboarding nickname (9D bit 7 clear): free, once. The answer `83 48 05 01, u32 0, u8 face` stores the face on
/// the client and, with uid 0, decrements no inventory row (reader 0x200192C33 skips the search for uid 0).
/// The client then leaves the nickname step by its own `tutorial_nickname_ok_clicked` (next: choose faction).
fn first_nickname(cx: &mut Ctx, peer: &mut Peer, name: &str, face: u8) {
    let Some(id) = peer.account else { return };
    if let Err(e) = cx.lobby.store.rename_account(id, name) {
        log::info!("{}: 48 05 first nickname {name:?} refused: {e}", peer.addr);
        peer.reply(Reply::Account(Account48::NicknameChanged { ok: 0, uid: 0, face: 0 }));
        return;
    }
    update(cx, peer, |p| {
        p.onboarding |= accounts::Profile::NICKNAME_CHOSEN;
        p.face = face;
    });
    log::info!("{}: 48 05 account {id} chose its nickname {name:?} (face {face})", peer.addr);
    peer.name = name.to_string();
    peer.reply(Reply::Account(Account48::NicknameChanged { ok: 1, uid: 0, face }));
    peer.push(&S2c::Name(NameMsg { flag: 0, name: name.to_string() }));
}

/// 4A 02 `skip_tutorial` and a finished tutorial match: 9D bit 5 (tutorial complete), so the next sign-in goes
/// straight to the full lobby once the nickname and the faction are chosen.
pub fn tutorial_done(cx: &mut Ctx, peer: &mut Peer) {
    update(cx, peer, |p| p.onboarding |= accounts::Profile::TUTORIAL_DONE);
}
