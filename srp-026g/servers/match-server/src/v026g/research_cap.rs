//! Limit repeated capture/neutralisation pay, with room for active fighting.
//! Kill-invader rewards are combat rewards and are never cut here.

use survarium::v026g::match_msgs::snapshot::{Statistics, event};
use survarium::v026g::match_msgs::static_info::RESEARCH_WEIGHTS;

use super::battery_cap::cut;

/// Base objective allowance, plus forty points per paid kill and twenty per
/// assist. Apply the repeated-kill limit first (see `reward_caps`).
pub(crate) const MOST_SCORE: u32 = 200;
const PER_KILL: u32 = 40;
const PER_ASSIST: u32 = 20;

/// A player's Research score cut: the place, what it scored and what it is
/// paid, whole points.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Cut {
    pub place: usize,
    pub scored: u32,
    pub paid: u32,
}

/// Limit captures and neutralisations to the combat-adjusted allowance.
pub(crate) fn cap(statistics: &mut Statistics) -> Vec<Cut> {
    let mut cuts = Vec::new();
    for (place, player) in statistics.players.iter_mut().enumerate() {
        // Both objectives count hundredths and have whole-point weights.
        // Keep the ratio integral: floating-point truncation can otherwise
        // drop an extra hundredth even when the scaled total is exact.
        let score: u64 = RESEARCH_WEIGHTS
            .iter()
            .filter(|&&(kind, _)| kind != event::KILL_INVADER)
            .map(|&(event, weight)| u64::from(player.total(event)) * weight as u64)
            .sum();
        let allowance = MOST_SCORE
            + PER_KILL * player.total(event::KILL)
            + PER_ASSIST * player.total(event::ASSIST);
        let most = u64::from(allowance) * 100;
        if score <= most {
            continue;
        }
        for (event, _) in RESEARCH_WEIGHTS {
            if event == event::KILL_INVADER {
                continue;
            }
            let counted = player.total(event);
            if counted > 0 {
                cut(
                    &mut player.events[event],
                    (u64::from(counted) * most / score) as u32,
                );
            }
        }
        cuts.push(Cut {
            place,
            scored: (score / 100) as u32,
            paid: allowance,
        });
    }
    cuts
}

#[cfg(test)]
mod tests;
