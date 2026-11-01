//! A player's battery events in one match pay at most as many times as the
//! map has batteries, per kind (the owner's rule against farming them,
//! 2026-10-20, docs/notes/battery_credit.md). The server sees each player's
//! running totals only, not single events or which battery: so the totals
//! are cut.

use survarium::v026g::match_msgs::snapshot::{KITS, Statistics, event};

/// The battery events and what one battery is worth of each: a put-in, a
/// find and the quest events count 1 a battery; the delivery shares (a
/// delivery, a stolen one, a rescued one) are hundredths of a delivery.
/// Event 6 also counts a carrier's progress (exe patch 92: 10 hundredths a
/// 10 m nearer its container than the battery has been): a battery is worth
/// its delivery share and 200 m of progress, more than any map's way home.
const KINDS: [(usize, &str, u32); 7] = [
    (event::PUT_VICTORY_ITEM_IN_CONTAINER, "put in", 1),
    (event::VICTORY_ITEM_FOUND, "found", 1),
    (event::BRING_VICTORY_ITEM, "share and progress", 300),
    (event::STEAL_VICTORY_ITEM, "stolen delivery share", 100),
    (event::RECOVER_VICTORY_ITEM, "rescued delivery share", 100),
    (event::SOLO_DELIVERY, "solo (quest)", 1),
    (event::STOLEN_AND_DELIVERED, "own steal (quest)", 1),
];

/// A total cut: the place, the kind, what it counted and what it is paid.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Cut {
    pub place: usize,
    pub kind: &'static str,
    pub counted: u32,
    pub paid: u32,
}

/// Cut each player's battery events to `batteries` batteries' worth of each
/// kind. No batteries (Team Deathmatch): nothing is cut.
pub(crate) fn cap(statistics: &mut Statistics, batteries: usize) -> Vec<Cut> {
    let mut cuts = Vec::new();
    if batteries == 0 {
        return cuts;
    }
    for (place, player) in statistics.players.iter_mut().enumerate() {
        for (event, kind, per_battery) in KINDS {
            let counted = player.total(event);
            let paid = counted.min(batteries as u32 * per_battery);
            if counted <= paid {
                continue;
            }
            cut(&mut player.events[event], paid);
            cuts.push(Cut {
                place,
                kind,
                counted,
                paid,
            });
        }
    }
    cuts
}

/// Cut one kind's counters of the kits to `paid` in all. Each kit that
/// counted keeps at least 1, so it is still a kit used in the match (its
/// wear, its part of the pay: `rewards`); the rest is shared in proportion
/// to what each kit counted above that 1.
pub(super) fn cut(counts: &mut [u16; KITS], paid: u32) {
    let was = *counts;
    let used = was.iter().filter(|&&count| count > 0).count() as u32;
    let above: u32 = was
        .iter()
        .map(|&count| u32::from(count.saturating_sub(1)))
        .sum();
    // Only a map of fewer batteries than kits could pay fewer than one each.
    let mut ones = paid.min(used);
    let share = paid - ones;
    let mut left = share;
    for count in counts.iter_mut().filter(|count| **count > 0) {
        let one = u32::from(ones > 0);
        ones -= one;
        let extra = (u64::from(*count - 1) * u64::from(share) / u64::from(above.max(1))) as u32;
        left -= extra;
        *count = (one + extra) as u16;
    }
    // The rounding's rest to the kits with room, the first first.
    for (count, was) in counts.iter_mut().zip(was) {
        let take = left.min(u32::from(was - *count));
        *count += take as u16;
        left -= take;
    }
}

#[cfg(test)]
mod tests;
