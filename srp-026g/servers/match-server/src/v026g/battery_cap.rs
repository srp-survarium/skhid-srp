//! A player's battery events in one match pay at most as many times as the
//! map has batteries, per kind (the owner's rule against farming them,
//! 2026-10-20, docs/notes/battery_credit.md). The server sees each player's
//! running totals only, not single events or which battery: so the totals
//! are cut.

use survarium::v026g::match_msgs::snapshot::{KITS, Statistics, event};

/// The battery events and what one battery is worth of each: a put-in, a
/// find and the quest events count 1 a battery; the delivery shares (a
/// delivery, a stolen one, a rescued one) are hundredths of a delivery.
const KINDS: [(usize, &str, u32); 7] = [
    (event::PUT_VICTORY_ITEM_IN_CONTAINER, "put in", 1),
    (event::VICTORY_ITEM_FOUND, "found", 1),
    (event::BRING_VICTORY_ITEM, "delivery share", 100),
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
fn cut(counts: &mut [u16; KITS], paid: u32) {
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

/// The events a Research game of revision 30 counted its captures,
/// neutralisations and kills of invaders in for a player of kit 1 or 2
/// (exe patch 53 took the statistics handler for the shared statistics:
/// its cell was 4 bytes on, `research_native_026g.c`): a capture's 100
/// hundredths in event 7 (a teammate cured, 2.5 a one and a help's pay),
/// a neutralisation's in 19 (a battery found, 2.5 a one). Both are one
/// byte on the wire, so what is left of them is the hundredths modulo 256
/// and the captures cannot be told from them: a Research match pays
/// neither. 19 has nothing else in Research (no batteries); 7's real cures
/// go unpaid with them (2.5 score and a help's 10 silver each).
const RESEARCH_MISPLACED: [(usize, &str); 2] = [
    (event::TEAMMATE_CURE, "teammate cure (a capture's share)"),
    (
        event::VICTORY_ITEM_FOUND,
        "battery found (a neutralisation's share)",
    ),
];

/// Zero [`RESEARCH_MISPLACED`] for every player of a Research match.
pub(crate) fn research_misplaced(statistics: &mut Statistics) -> Vec<Cut> {
    let mut cuts = Vec::new();
    for (place, player) in statistics.players.iter_mut().enumerate() {
        for (event, kind) in RESEARCH_MISPLACED {
            let counted = player.total(event);
            if counted > 0 {
                player.events[event] = [0; KITS];
                cuts.push(Cut {
                    place,
                    kind,
                    counted,
                    paid: 0,
                });
            }
        }
    }
    cuts
}

#[cfg(test)]
mod tests;
