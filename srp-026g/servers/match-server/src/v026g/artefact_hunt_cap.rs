//! Artifact Hunt's deliveries pay within what one match can give and an
//! allowance that grows with fighting (`artefact_hunt.md`, "srp"; the owner's
//! to change). Only the payment copy is cut: score, quests and rating keep
//! the counters as played.

use survarium::v026g::match_msgs::snapshot::{HUNDREDTHS, PlayerStatistics, Statistics, event};
use survarium::v026g::match_msgs::static_info::ARTEFACT_HUNT_WEIGHTS;

use super::battery_cap::cut;
use super::research_cap::{Cut, PER_ASSIST, PER_KILL};

/// What a player's put and bring pay at most with no kill and no assist: two
/// deliveries carried alone (a put 10 and the whole way 50 each).
pub(crate) const MOST_SCORE: u32 = 120;

/// A bring hundredth's score, in hundredths of a point (its weight, 50).
const BRING_HUNDREDTH: u64 = 50;

/// What one delivery of his team counts at most of each, per event: one put;
/// the shares of a delivery add up to 100 hundredths.
const PER_DELIVERY: [(usize, u32); 2] = [
    (event::PUT_VICTORY_ITEM_IN_CONTAINER, 1),
    (event::BRING_VICTORY_ITEM, 100),
];

/// Each player's put and bring: first to what `goal` deliveries of his team
/// count (no team delivers more in a match), then their score to
/// [`MOST_SCORE`] + 40 a paid kill + 20 an assist, both cut in proportion.
pub(crate) fn cap(statistics: &mut Statistics, goal: u8) -> Vec<Cut> {
    let mut cuts = Vec::new();
    for (place, player) in statistics.players.iter_mut().enumerate() {
        // Whole points in hundredths: a hundredths event's count, a whole
        // one's times 100 (the weights are whole).
        let score = |player: &PlayerStatistics| -> u64 {
            ARTEFACT_HUNT_WEIGHTS
                .iter()
                .map(|&(event, weight)| {
                    let unit = if HUNDREDTHS.contains(&event) { 1 } else { 100 };
                    u64::from(player.total(event)) * weight as u64 * unit
                })
                .sum()
        };
        let scored = score(player);
        for (event, per_delivery) in PER_DELIVERY {
            let most = u32::from(goal) * per_delivery;
            if player.total(event) > most {
                cut(&mut player.events[event], most);
            }
        }
        let bounded = score(player);
        let allowance =
            MOST_SCORE + PER_KILL * player.total(event::KILL) + PER_ASSIST * player.total(event::ASSIST);
        let most = u64::from(allowance) * 100;
        if bounded > most {
            let before = player.events[event::BRING_VICTORY_ITEM];
            for (event, _) in ARTEFACT_HUNT_WEIGHTS {
                let counted = player.total(event);
                if counted > 0 {
                    cut(&mut player.events[event], (u64::from(counted) * most / bounded) as u32);
                }
            }
            // The puts' rounding (10 points a put) back as bring hundredths
            // (half a point each), within what was brought.
            let mut rest = most.saturating_sub(score(player)) / BRING_HUNDREDTH;
            for (count, was) in player.events[event::BRING_VICTORY_ITEM].iter_mut().zip(before) {
                let back = rest.min(u64::from(was.saturating_sub(*count)));
                *count += back as u16;
                rest -= back;
            }
        }
        if score(player) < scored {
            cuts.push(Cut {
                place,
                scored: (scored / 100) as u32,
                paid: (score(player) / 100) as u32,
            });
        }
    }
    cuts
}

#[cfg(test)]
mod tests;
