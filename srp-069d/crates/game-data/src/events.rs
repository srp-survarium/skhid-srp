//! The 0.69d event: three event currencies, their match drops, the worn-item bonus and the schedule input.
//! Notes: docs/notes/events.md.
//!
//! PROVEN data (`data/event_drops_069d.json`, `crates/data-extract`):
//! * the currencies are the items 951 `global_event_currency`, 1138 `global_event_currency2`, 1139 `global_event_currency3`
//!   (the lobby movie's `PlayerInfo` hard-codes the three ids; the exe reads the wallet from 8B, fields `event1..3`);
//! * `drop_after_combat` has 72 rows for them, all with `service_needed 23` (the 94 record type 23 = a running event, see
//!   the lobby `events` module), each `{game_mode_mask, dict_id, amount, place_in_match, premium_factor 0.25}` plus
//!   `win_needed` (winners) or `loss_needed` (losers) or neither (mask 8 = Guns Race, mode id 3: the rows do not look at
//!   the result);
//! * `event_exchange_action_items.options`: five winter hats give +1 of each currency per match when worn
//!   (`put_on_bonus_items`; the client text "Use winter hat to receive more symbols"), and `item_to_drop_after_win` 2201.
//!
//! LIKELY: `game_mode_mask` bit `n` = game mode id `n` (the client's `GameMode` constants: 0 gather, 1 team deathmatch,
//! 2 capture point, 3 guns race, ... 7 PvE, 8 domination, 9 artifact hunt; the rows' masks 503 and 8 split exactly on
//! mode 3), `place_in_match` = the player's place in his own team by score (0 = best; the ranked league uses the same
//! kind of index), the premium factor is applied like the spare-parts one, the worn bonus is added flat after it.
//! The schedule is NOT in any file: [`Schedule`] is an input and absent by default.

use serde::Deserialize;

/// Dict ids of the three event currencies, in 8B / wallet slot order.
pub const CURRENCIES: [u16; 3] = [951, 1138, 1139];

/// The `94` record type of a running event (`service_needed` of the drop rows; the lobby movie opens the exchange
/// window for the action with `service_dict_id` 23 when `FactionsBattleWindow.FACTIONS_BATTLE_ENABLED` is false).
pub const SERVICE: u8 = 23;

/// Wallet slot (0..=2) of an event currency dict id.
pub fn slot_of(dict: u16) -> Option<usize> {
    CURRENCIES.iter().position(|&d| d == dict)
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Row {
    pub mode_mask: u32,
    pub currency: u16,
    pub place: u8,
    pub amount: u32,
    pub win_needed: bool,
    pub loss_needed: bool,
    pub premium_factor: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BonusValue {
    pub currency: u16,
    pub amount: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BonusItem {
    pub dict_id: u16,
    pub bonus: Vec<BonusValue>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EventData {
    pub service_needed: u8,
    pub currencies: Vec<u16>,
    pub rows: Vec<Row>,
    pub put_on_bonus_items: Vec<BonusItem>,
    /// `item_to_drop_after_win` of the exchange options (no chance or count in any file: OPEN)
    pub item_to_drop_after_win: u16,
}

pub const FILE: &str = "event_drops_069d.json";

impl EventData {
    pub fn parse(json: &str) -> Result<EventData, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// The currencies a finished match pays at `place` (0 = best of the player's team; Guns Race: of the match):
    /// `[currency 1, 2, 3]`. Winners match the `win_needed` rows, losers the `loss_needed` rows, and a Guns Race row has
    /// neither flag. A premium account gets `round(amount * (1 + premium_factor))`. A place without a row (there are
    /// rows for places 0..=7) and a mode without rows pay 0.
    pub fn match_pay(&self, mode: u32, won: bool, place: u8, premium: bool) -> [u32; 3] {
        let mut out = [0u32; 3];
        if mode >= 32 {
            return out;
        }
        for r in &self.rows {
            if r.mode_mask & (1 << mode) == 0 || r.place != place {
                continue;
            }
            if (r.win_needed && !won) || (r.loss_needed && won) {
                continue;
            }
            let Some(slot) = slot_of(r.currency) else { continue };
            let factor = if premium { 1.0 + r.premium_factor } else { 1.0 };
            out[slot] += (f64::from(r.amount) * factor).round() as u32;
        }
        out
    }

    /// The flat bonus of the worn items (`put_on_bonus_items`): `[currency 1, 2, 3]` summed over the dict ids in `worn`.
    pub fn worn_bonus(&self, worn: impl IntoIterator<Item = u16>) -> [u32; 3] {
        let mut out = [0u32; 3];
        for d in worn {
            for b in self.put_on_bonus_items.iter().filter(|b| b.dict_id == d) {
                for v in &b.bonus {
                    if let Some(s) = slot_of(v.currency) {
                        out[s] += v.amount;
                    }
                }
            }
        }
        out
    }
}

/// The place in a team by score: how many teammates scored strictly more (0 = best; equal scores share the place).
pub fn place_by_score(score: u16, team_scores: impl IntoIterator<Item = u16>) -> u8 {
    team_scores.into_iter().filter(|&s| s > score).count().min(u8::MAX as usize) as u8
}

/// When the event runs. An input (no file has dates): absent = no event, no `94` record 23, no drops, no exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    /// unix seconds, inclusive
    pub start: i64,
    /// unix seconds, exclusive: matches pay event currency until here
    pub end: i64,
    /// unix seconds, exclusive, at least `end`: the exchange window (the `94` record) stays until here. The client's text
    /// for a finished event is "exchange the symbols for the unique weapons", so the window outlives the drops
    /// (LIKELY); default = `end`.
    pub exchange_end: i64,
    /// the window's header line (the record's `ui` `label`); empty = the lobby's default text
    pub label: String,
    /// the window's body text (`description`); empty = the lobby's default text
    pub description: String,
}

impl Schedule {
    /// The event runs: matches pay currency.
    pub fn active(&self, now: i64) -> bool {
        self.start <= now && now < self.end
    }

    /// The exchange window exists (the `94` record 23 is sent, `48 28` is served).
    pub fn window_open(&self, now: i64) -> bool {
        self.start <= now && now < self.exchange_end.max(self.end)
    }

    /// Seconds left of the event (0 outside it): the record's `actionTime`, the window's countdown.
    pub fn left(&self, now: i64) -> u32 {
        if self.active(now) {
            (self.end - now).clamp(0, i64::from(u32::MAX)) as u32
        } else {
            0
        }
    }

    /// `start:end[:exchange_end]` in unix seconds, e.g. `1700000000:1707000000`.
    pub fn parse(text: &str) -> Result<Schedule, String> {
        let f: Vec<&str> = text.trim().split(':').map(str::trim).collect();
        if !(2..=3).contains(&f.len()) {
            return Err(format!("{text:?}: need start:end[:exchange_end] (unix seconds)"));
        }
        let num = |s: &str, what: &str| s.parse::<i64>().map_err(|_| format!("event {what} {s:?}"));
        let (start, end) = (num(f[0], "start")?, num(f[1], "end")?);
        let exchange_end = if f.len() == 3 { num(f[2], "exchange end")? } else { end };
        if end <= start {
            return Err("the event must end after it starts".into());
        }
        if exchange_end < end {
            return Err("the exchange window must not close before the event ends".into());
        }
        Ok(Schedule { start, end, exchange_end, label: String::new(), description: String::new() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> EventData {
        EventData::parse(&crate::testing::read("event_drops_069d.json")).unwrap()
    }

    #[test]
    fn the_rows_are_the_drop_table() {
        crate::require_tables!();
        let d = data();
        assert_eq!(d.rows.len(), 72);
        assert_eq!(d.service_needed, SERVICE);
        assert_eq!(d.currencies, CURRENCIES);
        // 8 places x 3 currencies x (win, loss, guns race)
        assert_eq!(d.rows.iter().filter(|r| r.win_needed).count(), 24);
        assert_eq!(d.rows.iter().filter(|r| r.loss_needed).count(), 24);
        assert_eq!(d.rows.iter().filter(|r| r.mode_mask == 8).count(), 24);
        assert!(d.rows.iter().all(|r| r.premium_factor == 0.25));
        assert_eq!(d.item_to_drop_after_win, 2201);
    }

    #[test]
    fn a_winner_and_a_loser_of_team_deathmatch_are_paid_by_place() {
        crate::require_tables!();
        let d = data();
        // mode 1 = team deathmatch: mask 503 has bit 1
        assert_eq!(d.match_pay(1, true, 0, false), [13, 14, 15]);
        assert_eq!(d.match_pay(1, true, 7, false), [3, 4, 5]);
        assert_eq!(d.match_pay(1, false, 0, false), [10, 11, 12]);
        assert_eq!(d.match_pay(1, false, 7, false), [1, 2, 3]);
        // beyond the 8 listed places: nothing
        assert_eq!(d.match_pay(1, true, 8, false), [0, 0, 0]);
    }

    #[test]
    fn premium_adds_a_quarter_rounded() {
        crate::require_tables!();
        let d = data();
        // 13 * 1.25 = 16.25 -> 16, 14 * 1.25 = 17.5 -> 18, 15 * 1.25 = 18.75 -> 19
        assert_eq!(d.match_pay(1, true, 0, true), [16, 18, 19]);
        assert_eq!(d.match_pay(1, false, 7, true), [1, 3, 4]);
    }

    #[test]
    fn guns_race_ignores_the_result_and_other_modes_do_not_use_its_rows() {
        crate::require_tables!();
        let d = data();
        // mode 3: mask 8 rows only
        assert_eq!(d.match_pay(3, true, 0, false), [15, 16, 17]);
        assert_eq!(d.match_pay(3, false, 0, false), [15, 16, 17]);
        assert_eq!(d.match_pay(3, true, 7, false), [4, 5, 6]);
        // artifact hunt (mode 9) and anything above 31 are in no mask
        assert_eq!(d.match_pay(9, true, 0, false), [0, 0, 0]);
        assert_eq!(d.match_pay(40, true, 0, false), [0, 0, 0]);
    }

    #[test]
    fn worn_winter_hats_add_one_of_each() {
        crate::require_tables!();
        let d = data();
        assert_eq!(d.worn_bonus([2201]), [1, 1, 1]);
        assert_eq!(d.worn_bonus([1168, 107]), [1, 1, 1]);
        assert_eq!(d.worn_bonus([107, 3]), [0, 0, 0]);
        assert_eq!(d.put_on_bonus_items.iter().map(|b| b.dict_id).collect::<Vec<_>>(), [1168, 1910, 1964, 2054, 2201]);
    }

    #[test]
    fn place_counts_better_teammates() {
        assert_eq!(place_by_score(50, [80, 60, 50, 10]), 2);
        assert_eq!(place_by_score(80, [80, 60]), 0);
        assert_eq!(place_by_score(0, []), 0);
    }

    #[test]
    fn schedule_parses_and_counts_down() {
        let s = Schedule::parse("1000:2000").unwrap();
        assert!(!s.active(999) && s.active(1000) && s.active(1999) && !s.active(2000));
        assert_eq!((s.left(1500), s.left(2500), s.left(10)), (500, 0, 0));
        assert!(Schedule::parse("2000:1000").is_err());
        assert!(Schedule::parse("x:1").is_err());
        assert!(Schedule::parse("1").is_err());
        // the exchange window can outlive the event
        let s = Schedule::parse("1000:2000:3000").unwrap();
        assert!(s.window_open(2500) && !s.active(2500) && !s.window_open(3000));
        assert_eq!(s.left(2500), 0);
        assert!(Schedule::parse("1000:2000:1500").is_err());
        assert!(!Schedule::parse("1000:2000").unwrap().window_open(2000));
    }
}
