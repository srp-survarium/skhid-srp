//! The daily login reward calendar: the `94` service record (type 0x1B) and the collect request `5E`.
//! Research: docs/notes/quests.md section 5, docs/notes/progression.md section 6.
//!
//! PROVEN (exe): the calendar is a record of `94` (`u32 id` = the action's `callback_value`, type byte 0x1B, body
//! `u16 n, n x {u32 amount, u16 reward dict, u8 award_quality}, u8 has_state, {u64 received_mask, u8 current_day, u8, u8}`
//! as a raw LZMA2 stream in the record's second buffer; readers 0x200189F90, LZMA2 decode 0x2001C64DE, 0x2001B9040 with
//! at most 64 days); the window shows `today_reward_received = bit(mask, current_day) ||
//! current_day >= n`; `5E u32 id` is `daily_reward_collect` (0x2001E9760). The client strings: "Daily Rewards",
//! "On 7th day you will get", "seven days streak" window (type 0x2D = 7 `simple_daily_reward`).
//!
//! The reward table is NOT in any game file (searched: every `gameplay/**/*.options` of the newest volume copies,
//! `localization`, the lobby movie's AS3; only the window texts exist) and the developer notes do not give it:
//! [`LobbyConfig::login_calendar`] is an input; without one no record is sent and `5E` does nothing. NOT invented.
//!
//! CHOICE (nothing states it): one collection per UTC day; the cycle runs from day 0 to day n-1 on consecutive days
//! ("streak"); a missed day starts again at day 0, and the day after the last day starts a new cycle.

use accounts::{CalendarState, Profile};
use game_data::chests::Reward;
use protocol::s2c::quests::{BonusBody, BonusDay, BonusState};
use protocol::Encode;
use protocol::s2c::*;

use crate::dispatch::{update, Ctx};
use crate::peer::Peer;
use crate::premium::unix_now;

/// The first `u32` of the record (`callback_value` of the client's action object). `5E` does not have to repeat it
/// ([`collect_request`]).
pub const RECORD_ID: u32 = 1;
/// Service type of the calendar record.
pub const KIND: u8 = 0x1B;

/// UTC day number (days since the epoch) of a unix time.
pub fn day_of(now: i64) -> i64 {
    now.div_euclid(86_400)
}

/// What the account's calendar shows on `today`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct View {
    /// day of the cycle shown as today's
    pub index: u8,
    pub received_today: bool,
    pub mask: u64,
}

pub fn view(n: usize, c: &CalendarState, today: i64) -> View {
    let fresh = View { index: 0, received_today: false, mask: 0 };
    if n == 0 || c.last_day == 0 {
        return fresh;
    }
    if c.last_day == today {
        return View { index: c.day, received_today: true, mask: c.received_mask };
    }
    if c.last_day == today - 1 {
        let next = c.day as usize + 1;
        if next < n {
            return View { index: next as u8, received_today: false, mask: c.received_mask };
        }
    }
    fresh
}

/// The body of the record.
pub fn body(days: &[BonusDay], c: &CalendarState, today: i64) -> BonusBody {
    let v = view(days.len(), c, today);
    // the client holds at most 64 days (protocol::s2c::MAX_BONUS_DAYS); a longer configured table is cut there
    let days = &days[..days.len().min(protocol::s2c::quests::MAX_BONUS_DAYS)];
    BonusBody { days: days.to_vec(), state: Some(BonusState { received_mask: v.mask, current_day: v.index }) }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    NoCalendar,
    AlreadyReceived,
}

/// Collect today's reward: marks it and returns the reward.
pub fn collect(days: &[BonusDay], c: &mut CalendarState, today: i64) -> Result<BonusDay, Refused> {
    if days.is_empty() {
        return Err(Refused::NoCalendar);
    }
    let v = view(days.len(), c, today);
    if v.received_today {
        return Err(Refused::AlreadyReceived);
    }
    *c = CalendarState { last_day: today, day: v.index, received_mask: v.mask | (1u64 << v.index.min(63)) };
    Ok(days[v.index as usize])
}

/// Add the calendar record (and the event record, events.rs) to a `94` message; each is a no-op without its setting. Every
/// place that sends a `94` goes through here, so the service records of the offers list stay together.
///
/// PROVEN (exe + lobby movie): each `94` replaces the client's whole record list (reader 0x200189F90 empties the vector
/// at 0x200189FDC), and `LobbyMenu.set_actions_data` opens the `daily_bonus` dialog for every `94` whose 0x1B action has
/// `today_reward_received` false while no `MessageDailyBonus` is open. So the record belongs in every `94`; the dialog
/// pops again only until today's reward is collected (`collect_request` pushes the received state).
pub fn with_calendar(cx: &Ctx, p: &Profile, m: Notices94) -> Notices94 {
    let mut m = crate::events::with_event(cx, m);
    let Some(days) = &cx.lobby.cfg.login_calendar else { return m };
    if days.is_empty() {
        return m;
    }
    // the second buffer is a raw LZMA2 stream of the body, at most 0x800 bytes decoded (PROVEN: status builder
    // 0x2001C6350, decode 0x2001C64DE, 0x1B reader 0x2001B9040); a plain body makes the client assert at sign-in. The
    // record holds at most 0x400 bytes.
    let data2 = protocol::lzma2::compress(&body(days, &p.calendar, day_of(unix_now())).to_bytes().unwrap_or_default());
    if data2.len() <= 0x400 {
        m.notices.push(Notice94 { f30: RECORD_ID, kind: KIND, data2, ..Default::default() });
    }
    m
}

/// `5E u32 id`: collect today's reward.
///
/// The id selects nothing and is not checked: the real client sent 2373604912 for record id 1. PROVEN (exe + lobby
/// movie): `5E` has one sender, `daily_reward_collect` (0x2001E9760), for the one 0x1B action object (0x2001C6D9D); an
/// account has at most one 0x1B record. OPEN: the static path gives `callback_value` = the record's first u32
/// (0x2001C6DC2), which does not explain 2373604912.
pub fn collect_request(cx: &mut Ctx, peer: &mut Peer, id: u32) {
    let Some(days) = cx.lobby.cfg.login_calendar.clone() else {
        log::info!("{}: 5E daily reward {id}: no calendar is configured", peer.addr);
        return;
    };
    if id != RECORD_ID {
        log::info!("{}: 5E daily reward: client id {id} (record id {RECORD_ID}), collecting the account's calendar", peer.addr);
    }
    let gd = &cx.lobby.data;
    let today = day_of(unix_now());
    let res = update(cx, peer, |p| {
        let d = collect(&days, &mut p.calendar, today)?;
        let granted = crate::chests::grant(gd, p, &[Reward { dict: d.reward, amount: d.amount }]);
        Ok::<_, Refused>((d, granted))
    });
    // `update` wraps the closure's value; a refused collection must not save the half-changed calendar
    match res {
        Some(Ok((d, (records, granted)))) => {
            log::info!("{}: 5E daily reward collected: dict {} x{}", peer.addr, d.reward, d.amount);
            let Some(acct) = peer.account else { return };
            if let Ok(p) = cx.lobby.store.load_profile(acct) {
                // an item day: the records of the granted stack / items with the received-item popup (`83 46 00` flag 1:
                // the client adds the records and shows the popup, as for the event exchange, a daily key or the
                // drops after a match); currency records (uid 0) travel as the 8B wallet instead
                let items: Vec<_> = records.into_iter().filter(|r| r.uid != 0).collect();
                if !items.is_empty() {
                    peer.push(&S2c::Reply(Reply::Shop(Shop46::Buy { flag: 1, items })));
                }
                peer.push(&S2c::Money(crate::build::money_8b(&p)));
                if granted.level {
                    peer.push(&S2c::Level(crate::build::level_8c(gd, &p)));
                }
                if granted.reputation {
                    peer.push(&S2c::Factions(crate::build::factions_8e(&p)));
                }
                peer.push(&S2c::Notices(with_calendar(cx, &p, crate::premium::notices(&cx.lobby.data, &p, unix_now()))));
            }
        }
        Some(Err(why)) => log::info!("{}: 5E daily reward refused: {why:?}", peer.addr),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::Decode;

    fn days(n: usize) -> Vec<BonusDay> {
        (0..n).map(|i| BonusDay { amount: 10 * (i as u32 + 1), reward: 925, quality: 0 }).collect()
    }

    #[test]
    fn the_cycle_follows_consecutive_days() {
        let d = days(3);
        let mut c = CalendarState::default();
        assert_eq!(view(3, &c, 100), View { index: 0, received_today: false, mask: 0 });
        assert_eq!(collect(&d, &mut c, 100).unwrap().amount, 10);
        assert_eq!(collect(&d, &mut c, 100).unwrap_err(), Refused::AlreadyReceived);
        assert_eq!(view(3, &c, 100), View { index: 0, received_today: true, mask: 1 });
        // next day: day 1
        assert_eq!(view(3, &c, 101), View { index: 1, received_today: false, mask: 1 });
        assert_eq!(collect(&d, &mut c, 101).unwrap().amount, 20);
        assert_eq!(collect(&d, &mut c, 102).unwrap().amount, 30);
        assert_eq!(c.received_mask, 0b111);
        // the day after the last: a new cycle, mask cleared
        assert_eq!(view(3, &c, 103), View { index: 0, received_today: false, mask: 0 });
        assert_eq!(collect(&d, &mut c, 103).unwrap().amount, 10);
        assert_eq!(c.received_mask, 1);
    }

    #[test]
    fn a_missed_day_starts_again() {
        let d = days(7);
        let mut c = CalendarState::default();
        collect(&d, &mut c, 50).unwrap();
        collect(&d, &mut c, 51).unwrap();
        assert_eq!(c.day, 1);
        // day 52 skipped
        assert_eq!(view(7, &c, 53), View { index: 0, received_today: false, mask: 0 });
        assert_eq!(collect(&d, &mut c, 53).unwrap().amount, 10);
        assert_eq!((c.day, c.received_mask), (0, 1));
        assert_eq!(collect(&[], &mut c, 54).unwrap_err(), Refused::NoCalendar);
    }

    #[test]
    fn the_body_is_what_the_client_reads() {
        let d = days(2);
        let mut c = CalendarState::default();
        let b = body(&d, &c, 9);
        assert_eq!(b.state, Some(BonusState { received_mask: 0, current_day: 0 }));
        collect(&d, &mut c, 9).unwrap();
        let b = body(&d, &c, 9);
        assert_eq!(b.state, Some(BonusState { received_mask: 1, current_day: 0 }));
        assert_eq!(BonusBody::from_bytes(&b.to_bytes().unwrap()).unwrap(), b);
        assert_eq!(day_of(86_400 * 3 + 5), 3);
    }
}
