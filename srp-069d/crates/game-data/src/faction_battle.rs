//! Faction battles: the reward tiers (data) and the battle schedule (input, absent in every game file).
//!
//! Data: `data/faction_battle_069d.json` (`crates/data-extract`, `faction_battle_drop` of
//! `gameplay/drops/drops.options`): per faction 5 tiers `{reputation, dict_id, amount}` (500 spare parts at 10000,
//! a 3-day Premium voucher at 20000, then one item each at 40000 / 60000 / 80000). The client shows the same table
//! (`root.set_faction_battle_drop`, 0x2001BC87B) and marks a tier received when `2^tier - 1 < received_mask`
//! (`WinterEventData.setLineRewards`), so the received mask the server sends is `2^n - 1` for n tiers received.
//!
//! The schedule is NOT in any game file and the developer notes do not give it (the window only links to the
//! website timetable, `buttonTimeTable_onClick` -> `go_to_online_message`): [`Schedule`] is an input the operator
//! supplies; without one no battle runs and nothing is reachable (nothing is invented).

use serde::Deserialize;

/// Faction ids of the client: 1 Scavengers, 2 Black Market, 3 Renaissance Army, 4 Fringe.
pub const FACTIONS: [u8; 4] = [1, 2, 3, 4];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Tier {
    pub reputation: u32,
    pub dict_id: u16,
    pub amount: u32,
}

#[derive(Debug, Deserialize)]
struct Raw {
    item_to_drop_after_win: u16,
    factions: std::collections::HashMap<String, Vec<Tier>>,
}

#[derive(Debug)]
pub struct FactionBattleData {
    /// dictionary id dropped after a won battle match (`item_to_drop_after_win`; its rule is not in the data: OPEN)
    pub item_after_win: u16,
    tiers: [Vec<Tier>; 4],
}

impl FactionBattleData {
    pub fn parse(json: &str) -> Result<FactionBattleData, serde_json::Error> {
        let raw: Raw = serde_json::from_str(json)?;
        let mut tiers: [Vec<Tier>; 4] = Default::default();
        for (k, v) in raw.factions {
            if let Some(i) = k.parse::<usize>().ok().filter(|i| (1..=4).contains(i)) {
                tiers[i - 1] = v;
            }
        }
        Ok(FactionBattleData { item_after_win: raw.item_to_drop_after_win, tiers })
    }

    /// Tiers of faction `f` (1..=4), ascending by reputation.
    pub fn tiers(&self, f: u8) -> &[Tier] {
        match f {
            1..=4 => &self.tiers[f as usize - 1],
            _ => &[],
        }
    }

    /// Number of tiers a reputation total has reached.
    pub fn reached(&self, f: u8, reputation: u32) -> usize {
        self.tiers(f).iter().take_while(|t| t.reputation <= reputation).count()
    }
}

/// One battle: two factions fight between `start` and `end` (unix seconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Interval {
    pub start: i64,
    pub end: i64,
    pub factions: [u8; 2],
}

/// The operator's battle schedule, ascending and non-overlapping.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Schedule {
    pub intervals: Vec<Interval>,
}

impl Schedule {
    /// The battle running at `now`.
    pub fn current(&self, now: i64) -> Option<&Interval> {
        self.intervals.iter().find(|i| i.start <= now && now < i.end)
    }

    /// The next battle that has not started.
    pub fn next(&self, now: i64) -> Option<&Interval> {
        self.intervals.iter().filter(|i| i.start > now).min_by_key(|i| i.start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> FactionBattleData {
        FactionBattleData::parse(&crate::testing::read("faction_battle_069d.json")).unwrap()
    }

    #[test]
    fn every_faction_has_five_ascending_tiers() {
        crate::require_tables!();
        let d = data();
        for f in FACTIONS {
            let t = d.tiers(f);
            assert_eq!(t.len(), 5, "faction {f}");
            assert!(t.windows(2).all(|w| w[0].reputation < w[1].reputation));
            assert_eq!((t[0].reputation, t[0].dict_id, t[0].amount), (10_000, 925, 500));
        }
        assert_eq!(d.item_after_win, 2086);
        assert_eq!(d.reached(3, 39_999), 2);
        assert_eq!(d.reached(3, 80_000), 5);
        assert_eq!(d.reached(3, 0), 0);
    }

    #[test]
    fn schedule_finds_the_running_and_the_next_battle() {
        let s = Schedule { intervals: vec![Interval { start: 100, end: 200, factions: [1, 2] }, Interval { start: 300, end: 400, factions: [3, 4] }] };
        assert_eq!(s.current(150).unwrap().factions, [1, 2]);
        assert!(s.current(200).is_none());
        assert_eq!(s.next(150).unwrap().start, 300);
        assert!(s.next(300).is_none());
    }
}
