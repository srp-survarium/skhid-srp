//! A player's battery events in one match pay at most as many times as the
//! map has batteries (the owner's rule against farming them, 2026-10-20,
//! docs/notes/battery_credit.md). The server sees each player's running
//! totals only, not single events or which battery: so the totals are cut.

use survarium::v026g::match_msgs::snapshot::{Statistics, event};

/// The battery events and what one battery is worth of each: a put-in and
/// the quest events count 1 a delivery, the found / stolen / rescued shares
/// are hundredths of a delivery.
const KINDS: [(usize, &str, u32); 6] = [
    (event::PUT_VICTORY_ITEM_IN_CONTAINER, "put in", 1),
    (event::BRING_VICTORY_ITEM, "found", 100),
    (event::STEAL_VICTORY_ITEM, "stolen", 100),
    (event::RECOVER_VICTORY_ITEM, "rescued", 100),
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

/// Cut each player's battery events to `batteries` deliveries of each kind.
/// No batteries (Team Deathmatch): nothing is cut.
pub(crate) fn cap(statistics: &mut Statistics, batteries: usize) -> Vec<Cut> {
    let mut cuts = Vec::new();
    if batteries == 0 {
        return cuts;
    }
    for (place, player) in statistics.players.iter_mut().enumerate() {
        for (event, kind, per_battery) in KINDS {
            let counted = player.total(event);
            let paid = counted.min(batteries as u32 * per_battery);
            // Off the kits' counters, the last kit first.
            let mut over = counted - paid;
            for count in player.events[event].iter_mut().rev() {
                let take = over.min(u32::from(*count));
                *count -= take as u16;
                over -= take;
            }
            if paid < counted {
                cuts.push(Cut {
                    place,
                    kind,
                    counted,
                    paid,
                });
            }
        }
    }
    cuts
}

#[cfg(test)]
mod tests;
