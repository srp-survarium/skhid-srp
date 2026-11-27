//! The Winner's Contract (docs/notes/store.md "Store offers and pictures", docs/notes/requests-34-39-3a.md).
//!
//! OFFICIAL 0.6x web-shop item (listed 2019-07 .. 2022-05): "The Winner's Contract (3 days): 20,000 silver for the first win
//! each day! The contract lasts for three days, which brings you 60,000 silver in total!". The client has no way to buy a
//! contract itself (its contract window only starts matchmaking, PROVEN), so it is sold as an actions-tab card
//! ([`crate::actions::default_offers`], `5C 01` -> [`buy`]) for [`PRICE_GOLD`] gold.
//!
//! Wire (PROVEN): a `94` record of type 0x16 makes the client
//! show the contract button and window (0x2001C5BC7 -> 0x2001C5F00 `root.set_contract_data`); the data is the `94` tail
//! `u8 present, u16 C, C x u8 status, u32 A, u32 B, u32 len, blob`:
//! * C day statuses: the window draws one cell per day; `totalReward` = amount x (days with status 1) (0x1C60D6);
//! * A (+0xF0) = `minutesLeft` of the whole contract, B (+0xF4) = `timeLeft` until the next day's task, 0xFFFFFFFF =
//!   `challenge_available` (0x1C620C). Both MINUTES (`MainScene` time manager ticks every 60 000 ms; `LobbyMenu` asks
//!   `39 00` when `timeLeft` minus the elapsed minutes reaches 0);
//! * blob = a raw LZMA2 stream (at most 0x800 bytes decoded) of a binary config (PROVEN: decoder 0x2001C5F25, config read
//!   0x2000CC680; a plain config crashes the client at sign-in). Root keys read raw (0x1C6000..0x1C6045):
//!   `amount` (u32), `actionType` (u8: 1 = text `st_contract_complete_after_win_descr`), `dictId` (u16: 214 = silver,
//!   `getMoneyTypeByDictId`).
//!
//! Rules (CHOICE where marked): 3 days of 24 h counted from the purchase (CHOICE: day boundaries every 24 h from the
//! purchase, not calendar days); the FIRST WON online PvP match (random, ranked, clan battle; not a local match, a custom
//! training or PvE) of each day pays [`DAILY_SILVER`] silver once and marks that day's status 1; a day without a win stays 0
//! (the client sums only status-1 days; LIKELY that the real server also used 0 for a missed day). One contract at a time:
//! a purchase is refused while one runs. After the 3 days the record is no longer sent (the client hides the button when a
//! `94` has no 0x16 record, 0x1C5D21).
//! Daily notice: `B1` type 0x16, text `[amount][param]` (the Flash keys `amount` / `param`, reader 0x2001C4750; shown as a
//! REWARD with `st_received`; the client marks it read itself and opens the contract window). `param` 0 = silver (PROVEN,
//! LobbyMenu `add_important_message`; docs/notes/notices.md level-up reward).

use accounts::{AccountId, ContractState, Currency, Profile};
use protocol::binconfig::{self, Value};
use protocol::s2c::*;

use crate::dispatch::{update, update_account, Ctx};
use crate::peer::{Cross, Peer};
use crate::premium::unix_now;
use crate::shop::send_money;

/// Days of a contract (OFFICIAL "lasts for three days").
pub const DAYS: u32 = 3;
/// One contract day (CHOICE: 24 h from the purchase).
pub const DAY_SECS: i64 = 86_400;
/// Silver of the first win of a day (OFFICIAL "20,000 silver for the first win each day").
pub const DAILY_SILVER: u32 = 20_000;
/// Gold price of the card (CHOICE).
pub const PRICE_GOLD: u32 = 100;
/// `94` record type of the contract (0x2001C5BC7, PROVEN).
pub const RECORD_KIND: u8 = 0x16;
/// `B1` wire type of the daily reward notice (0x2001C3B28, PROVEN).
pub const NOTICE_KIND: u8 = 0x16;
/// Blob `actionType` 1 = "win" task text (`st_contract_complete_after_win_descr`, 0x1C6292, PROVEN).
pub const ACTION_WIN: i32 = 1;
/// Blob `dictId` of the reward: 214 = silver (the money dict; `ActionProfiteer.getMoneyTypeByDictId` -> MONEY_REGULAR).
pub const SILVER_DICT: i32 = 214;
/// B in the tail when today's task is open (`challenge_available`, 0x1C620C).
pub const CHALLENGE_AVAILABLE: u32 = u32::MAX;

/// The contract day (0-based) at `now`, or None when no contract runs.
pub fn day(c: &ContractState, now: i64) -> Option<u32> {
    if c.started <= 0 {
        return None;
    }
    let elapsed = (now - c.started).max(0);
    let d = elapsed / DAY_SECS;
    (d < i64::from(DAYS)).then_some(d as u32)
}

/// Whether a contract runs at `now`.
pub fn running(c: &ContractState, now: i64) -> bool {
    day(c, now).is_some()
}

/// Whole minutes until `secs` have passed, rounded UP (so the client's countdown never ends before the server's boundary).
fn minutes_up(secs: i64) -> u32 {
    (secs.max(0) as u64).div_ceil(60).min(u64::from(u32::MAX - 1)) as u32
}

/// The tail blob: `{amount, actionType, dictId}` (root keys read by 0x2001C5F00).
pub fn blob() -> Vec<u8> {
    binconfig::encode(&[
        ("amount".to_string(), Value::Int(DAILY_SILVER as i32)),
        ("actionType".to_string(), Value::Int(ACTION_WIN)),
        ("dictId".to_string(), Value::Int(SILVER_DICT)),
    ])
}

/// The `94` tail of a running contract: C = 3 day statuses (1 = the day's win was paid, else 0), A = minutes left of the
/// contract, B = [`CHALLENGE_AVAILABLE`] while today's win is still open, else the minutes until the next day starts
/// (on the last day: until the contract ends).
pub fn tail(c: &ContractState, now: i64) -> Option<Tail94> {
    let d = day(c, now)?;
    let end = c.started + i64::from(DAYS) * DAY_SECS;
    let bytes = (0..DAYS).map(|i| u8::from(c.won_mask & (1 << i) != 0)).collect();
    let won_today = c.won_mask & (1 << d) != 0;
    let b = if won_today { minutes_up(c.started + i64::from(d + 1) * DAY_SECS - now) } else { CHALLENGE_AVAILABLE };
    Some(Tail94 { bytes, a: minutes_up(end - now), b, text: protocol::lzma2::compress(&blob()) })
}

/// The `94` record and tail of the profile's running contract (None = no contract: nothing is sent).
pub fn notice(p: &Profile, now: i64) -> Option<(Notice94, Tail94)> {
    let t = tail(&p.contract, now)?;
    // only the type of a 0x16 record is read (0x1C5BC7..0x1C5C0B), but the client skips a record whose two buffers are
    // both empty (0x2001C5A50: then the contract is hidden). One byte in the second buffer passes that check without a
    // `ui` blob (data2 is not read).
    Some((Notice94 { f30: u32::from(RECORD_KIND), kind: RECORD_KIND, data2: vec![0], ..Default::default() }, t))
}

/// Why a purchase is refused.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    AlreadyRunning,
    NotEnoughGold,
}

/// Start a contract on the profile: refused while one runs or with less than `gold` gold; nothing changes then.
pub fn start(p: &mut Profile, gold: u32, now: i64) -> Result<(), Refused> {
    if running(&p.contract, now) {
        return Err(Refused::AlreadyRunning);
    }
    p.spend(Currency::Gold, u64::from(gold)).map_err(|_| Refused::NotEnoughGold)?;
    p.contract = ContractState { started: now, won_mask: 0 };
    Ok(())
}

/// `5C 01` of the contract card: take `gold`, start the contract, push 8B and the `94` with the contract. Returns the
/// `BC sub 1` result.
pub fn buy(cx: &mut Ctx, peer: &mut Peer, card: u32, gold: u32) -> bool {
    let now = unix_now();
    match update(cx, peer, |p| start(p, gold, now)) {
        Some(Ok(())) => {
            log::info!("{}: 5C buy {card}: Winner's Contract started for {gold} gold", peer.addr);
            send_money(cx, peer);
            crate::premium::push_status(cx, peer);
            true
        }
        Some(Err(why)) => {
            log::info!("{}: 5C buy {card}: Winner's Contract refused: {why:?}", peer.addr);
            false
        }
        None => false,
    }
}

/// Pay the day's first win on the profile: returns the paid day, or None (no contract, or the day was already paid).
pub fn pay_day(p: &mut Profile, now: i64) -> Option<u32> {
    let d = day(&p.contract, now)?;
    if p.contract.won_mask & (1 << d) != 0 {
        return None;
    }
    p.contract.won_mask |= 1 << d;
    p.credit(Currency::Silver, u64::from(DAILY_SILVER));
    Some(d)
}

/// The `B1` type 0x16 text: `[amount][param]`, param 0 = silver.
pub fn reward_text(amount: u32) -> String {
    format!("[{amount}][0]")
}

/// A won online PvP match of `account` (online.rs `pay`): the first win of a contract day pays [`DAILY_SILVER`] and sends
/// 8B, the `94` with the updated record (before the notice: the notice opens the contract window from the client's latest
/// contract data) and the `B1` 0x16 notice. A second win the same day, or no contract, does nothing.
pub fn on_won_match(cx: &mut Ctx, account: AccountId, now: i64) {
    // checked on a loaded copy first so a match without a payout writes nothing
    let Ok(p) = cx.lobby.store.load_profile(account) else { return };
    if day(&p.contract, now).is_none_or(|d| p.contract.won_mask & (1 << d) != 0) {
        return;
    }
    let Some(Some((d, money))) = update_account(cx, account, |p| pay_day(p, now).map(|d| (d, crate::build::money_8b(p)))) else { return };
    log::info!("account {account}: Winner's Contract day {} won: +{DAILY_SILVER} silver", d + 1);
    cx.fx.push(Cross { to: account, msg: S2c::Money(money) });
    if let Some(m) = crate::premium::status_msg(cx, account) {
        cx.fx.push(Cross { to: account, msg: m });
    }
    let notice = SystemMessage { kind: NOTICE_KIND, text: reward_text(DAILY_SILVER), message_id: crate::notices::next_message_id(), ..Default::default() };
    cx.fx.push(Cross { to: account, msg: S2c::SystemMessage(notice) });
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_800_000_000;

    fn c(started: i64, won_mask: u8) -> ContractState {
        ContractState { started, won_mask }
    }

    #[test]
    fn days_run_24_hours_each_from_the_purchase() {
        assert_eq!(day(&ContractState::default(), T0), None);
        assert_eq!(day(&c(T0, 0), T0), Some(0));
        assert_eq!(day(&c(T0, 0), T0 + DAY_SECS - 1), Some(0));
        assert_eq!(day(&c(T0, 0), T0 + DAY_SECS), Some(1));
        assert_eq!(day(&c(T0, 0), T0 + 3 * DAY_SECS - 1), Some(2));
        assert_eq!(day(&c(T0, 0), T0 + 3 * DAY_SECS), None);
    }

    #[test]
    fn the_tail_counts_minutes_and_opens_the_challenge_until_the_day_is_won() {
        let t = tail(&c(T0, 0), T0 + 30).unwrap();
        assert_eq!(t.bytes, vec![0, 0, 0]);
        assert_eq!(t.a, (3 * 24 * 60) as u32, "3 days minus 30 s, rounded up");
        assert_eq!(t.b, CHALLENGE_AVAILABLE);
        // day 1 won, 2 h into day 2: day 2 open
        let t = tail(&c(T0, 1), T0 + DAY_SECS + 7200).unwrap();
        assert_eq!((t.bytes.clone(), t.b), (vec![1, 0, 0], CHALLENGE_AVAILABLE));
        assert_eq!(t.a, (2 * 24 * 60 - 120) as u32);
        // day 2 won: B = minutes until day 3 starts
        let t = tail(&c(T0, 0b011), T0 + DAY_SECS + 7200).unwrap();
        assert_eq!((t.bytes.clone(), t.b), (vec![1, 1, 0], (22 * 60) as u32));
        // last day won: B = A (the contract ends at the next boundary)
        let t = tail(&c(T0, 0b101), T0 + 2 * DAY_SECS + 60).unwrap();
        assert_eq!(t.bytes, vec![1, 0, 1], "the missed day 2 stays 0");
        assert_eq!(t.b, t.a);
        assert!(tail(&c(T0, 0b111), T0 + 3 * DAY_SECS).is_none());
    }

    #[test]
    fn the_record_carries_a_buffer_so_the_client_reaches_the_contract() {
        let mut p = Profile::starter();
        assert!(notice(&p, T0).is_none());
        p.contract = c(T0, 0);
        let (n, t) = notice(&p, T0 + 60).unwrap();
        assert_eq!(n.kind, RECORD_KIND);
        assert!(!n.data1.is_empty() || !n.data2.is_empty(), "an empty record is skipped at 0x2001C5A5A");
        assert!(n.data1.is_empty(), "no ui blob to decode");
        assert!(!t.text.is_empty(), "set_contract_data needs the tail blob (0x2001C5BE3)");
        // the tail blob is a raw LZMA2 stream (stored chunk, control byte 1) of the config, at most 0x800 bytes decoded
        assert_eq!(t.text[0], 0x01, "a valid LZMA2 control byte; the plain config starts with 0x10 and crashed the client");
        let body = protocol::lzma2::decompress(&t.text).unwrap();
        assert!(body.len() <= 0x800);
        assert_eq!(body, blob());
        assert_eq!(blob()[0], 0x10, "the plain config is not LZMA2 (control bytes 0x03..0x7F are invalid)");
    }

    #[test]
    fn the_blob_has_the_three_keys_the_client_reads() {
        let mut r = binconfig::decode(&blob()).unwrap();
        r.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            r,
            vec![("actionType".into(), Value::Int(1)), ("amount".into(), Value::Int(20_000)), ("dictId".into(), Value::Int(214))]
        );
        assert_eq!(reward_text(20_000), "[20000][0]");
    }

    #[test]
    fn a_day_pays_once_and_a_running_contract_refuses_a_second() {
        let mut p = Profile::starter();
        p.gold = 150;
        let silver = p.silver;
        assert_eq!(start(&mut p, PRICE_GOLD, T0), Ok(()));
        assert_eq!((p.gold, p.contract), (50, c(T0, 0)));
        assert_eq!(start(&mut p, PRICE_GOLD, T0 + 10), Err(Refused::AlreadyRunning));
        assert_eq!(pay_day(&mut p, T0 + 100), Some(0));
        assert_eq!(pay_day(&mut p, T0 + 200), None);
        assert_eq!(p.silver, silver + DAILY_SILVER);
        assert_eq!(pay_day(&mut p, T0 + 2 * DAY_SECS + 5), Some(2));
        assert_eq!(p.contract.won_mask, 0b101);
        assert_eq!(pay_day(&mut p, T0 + 3 * DAY_SECS), None, "over");
        // after the end a new one may be bought; too little gold changes nothing
        assert_eq!(start(&mut p, PRICE_GOLD, T0 + 3 * DAY_SECS), Err(Refused::NotEnoughGold));
        assert_eq!(p.contract, c(T0, 0b101));
        p.gold = 100;
        assert_eq!(start(&mut p, PRICE_GOLD, T0 + 3 * DAY_SECS), Ok(()));
        assert_eq!((p.gold, p.contract), (0, c(T0 + 3 * DAY_SECS, 0)));
    }
}
