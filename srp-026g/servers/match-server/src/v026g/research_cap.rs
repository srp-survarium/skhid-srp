//! A player's Research score of units captured, units neutralised and
//! invaders killed pays at most [`MOST_SCORE`] in one match (the owner's
//! rule against trading units back and forth, 2026-10-27,
//! docs/notes/research_mode.md). The server sees each player's running
//! totals only: so the totals are cut, all three in the same proportion.

use survarium::v026g::match_msgs::snapshot::{Statistics, event};
use survarium::v026g::match_msgs::static_info::RESEARCH_WEIGHTS;

use super::battery_cap::cut;

/// About six whole captures. On live (the 20 Research matches before
/// revision 36) no player who fought scored more than about 190 of it; the
/// farmers scored 350 to 700.
pub(crate) const MOST_SCORE: u32 = 200;

/// A player's Research score cut: the place, what it scored and what it is
/// paid, whole points.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Cut {
    pub place: usize,
    pub scored: u32,
    pub paid: u32,
}

/// Cut each player's Research events to [`MOST_SCORE`] of score.
pub(crate) fn cap(statistics: &mut Statistics) -> Vec<Cut> {
    let mut cuts = Vec::new();
    for (place, player) in statistics.players.iter_mut().enumerate() {
        let score: f64 = RESEARCH_WEIGHTS
            .iter()
            .map(|&(event, weight)| {
                f64::from(player.total(event)) * unit(event) * f64::from(weight)
            })
            .sum();
        let most = f64::from(MOST_SCORE);
        if score <= most {
            continue;
        }
        let keep = most / score;
        for (event, _) in RESEARCH_WEIGHTS {
            let counted = player.total(event);
            if counted > 0 {
                cut(
                    &mut player.events[event],
                    (f64::from(counted) * keep) as u32,
                );
            }
        }
        cuts.push(Cut {
            place,
            scored: score as u32,
            paid: MOST_SCORE,
        });
    }
    cuts
}

/// A capture and a neutralisation count in hundredths, an invader by ones.
fn unit(event: usize) -> f64 {
    if event == event::RESEARCH_CAPTURE || event == event::RESEARCH_NEUTRALISE {
        0.01
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests;
