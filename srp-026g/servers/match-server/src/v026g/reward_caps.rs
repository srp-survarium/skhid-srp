//! Own the payment-only copy and the order of the anti-farming rules.

use survarium::v026g::{maps::mode, match_msgs::snapshot::Statistics};

use super::{kill_cap, research_cap};

pub(super) struct Payment {
    pub statistics: Statistics,
    pub kills: Vec<kill_cap::Cut>,
    pub objectives: Vec<research_cap::Cut>,
}

impl Payment {
    /// The original statistics remain available for score and quests. PvE
    /// keeps its existing reward policy. Unpaid feeding kills cannot buy
    /// extra objective allowance: both steps live here, in a fixed order.
    pub fn of(played: &Statistics, pairs: &kill_cap::Pairs, mode: u8, vs_bots: bool) -> Self {
        let mut payment = Self {
            statistics: played.clone(),
            kills: Vec::new(),
            objectives: Vec::new(),
        };
        if !vs_bots {
            payment.kills = kill_cap::cap(&mut payment.statistics, pairs);
            if mode == mode::RESEARCH {
                payment.objectives = research_cap::cap(&mut payment.statistics);
            }
        }
        payment
    }
}

#[cfg(test)]
mod tests;
