//! Own the payment-only copy and the order of the anti-farming rules.

use survarium::v026g::{maps::MapMode, maps::RuleMode, match_msgs::snapshot::Statistics};

use super::{artefact_hunt_cap, kill_cap, research_cap};

/// The objective rule a match's pay is held to, by its mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Objectives {
    /// No objective allowance: the batteries have their own cap
    /// (`battery_cap`), Team Deathmatch and a free-for-all none.
    None,
    /// Captures and neutralisations (`research_cap`).
    Research,
    /// Puts and brings, within `goal` deliveries a team (`artefact_hunt_cap`).
    ArtefactHunt { goal: u8 },
}

impl Objectives {
    pub fn of(map: &MapMode) -> Self {
        match (map.mode.rules(), map.artefact_hunt) {
            (RuleMode::Research, _) => Self::Research,
            (RuleMode::ArtefactHunt, Some(params)) => Self::ArtefactHunt { goal: params.goal() },
            _ => Self::None,
        }
    }
}

pub(super) struct Payment {
    pub statistics: Statistics,
    pub kills: Vec<kill_cap::Cut>,
    pub objectives: Vec<research_cap::Cut>,
}

impl Payment {
    /// The original statistics remain available for score and quests. PvE
    /// skips these event caps (settlement caps its reward tier). Unpaid feeding kills cannot buy
    /// extra objective allowance: both steps live here, in a fixed order.
    pub fn of(
        played: &Statistics,
        pairs: &kill_cap::Pairs,
        objectives: Objectives,
        vs_bots: bool,
    ) -> Self {
        let mut payment = Self {
            statistics: played.clone(),
            kills: Vec::new(),
            objectives: Vec::new(),
        };
        if !vs_bots {
            payment.kills = kill_cap::cap(&mut payment.statistics, pairs);
            payment.objectives = match objectives {
                Objectives::None => Vec::new(),
                Objectives::Research => research_cap::cap(&mut payment.statistics),
                Objectives::ArtefactHunt { goal } => {
                    artefact_hunt_cap::cap(&mut payment.statistics, goal)
                }
            };
        }
        payment
    }
}

#[cfg(test)]
mod tests;
