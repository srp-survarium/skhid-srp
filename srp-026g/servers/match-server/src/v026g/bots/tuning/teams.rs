//! The tuning each team plays by, and the only way to reach it.
//!
//! A match has one tuning, and the bench's one-sided runs (`SRP_BENCH_ONE_SIDE`, head to head) give team 1 another. A decision
//! of a bot or a team that read team 0's tuning would then mix the rules of the two sides without a sound, so the per-team
//! tunings are fields of this type alone: a decision reaches its team's with [`TeamTunings::of`], and what exists once for
//! both teams (the map's tactics, the bodies' reckoning, the process-wide view settings) is read with
//! [`TeamTunings::match_wide`], whose name says it is nobody's decision.

use std::sync::Arc;

use super::Tuning;

/// The tunings of the two teams of a match.
pub struct TeamTunings {
    /// Team 0's, and team 1's too unless a one-sided run gives it another.
    team0: Arc<Tuning>,
    /// The real-map bench's one-sided runs: team 1 plays by this tuning instead (`real_bot_cost`, `SRP_BENCH_ONE_SIDE`).
    #[cfg(test)]
    team1: Option<Arc<Tuning>>,
}

impl TeamTunings {
    /// Both teams play by `tuning`.
    pub fn new(tuning: Arc<Tuning>) -> Self {
        Self {
            team0: tuning,
            #[cfg(test)]
            team1: None,
        }
    }

    /// The tuning team `team` plays by: every decision of one of its bots or of its commander reads this.
    #[cfg_attr(not(test), allow(unused_variables))]
    pub fn of(&self, team: u8) -> &Arc<Tuning> {
        #[cfg(test)]
        if let Some(tuning) = self.team1.as_ref().filter(|_| team == 1) {
            return tuning;
        }
        &self.team0
    }

    /// The tuning of what exists once for both teams: the map's tactics, the bodies' reckoning (`kin`), the ray budget of a
    /// poll, the board of live zones and the view settings that are process-wide statics. These cannot differ by team (a
    /// one-sided run has team 0's for them), so no decision of a bot or a team reads this: that is [`Self::of`].
    pub fn match_wide(&self) -> &Tuning {
        &self.team0
    }

    /// The bench's one-sided runs: team 1 plays by `tuning`.
    #[cfg(test)]
    pub fn give_team1(&mut self, tuning: Arc<Tuning>) {
        self.team1 = Some(tuning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The match-wide tuning is team 0's, team 1 plays by team 0's until a one-sided run gives it another.
    #[test]
    fn team_1_has_its_own_tuning_only_when_given() {
        let mut tunings = TeamTunings::new(Arc::new(Tuning::default()));
        assert!(Arc::ptr_eq(tunings.of(0), tunings.of(1)));
        let mut other = Tuning::default();
        other.think.stamina = !other.think.stamina;
        tunings.give_team1(Arc::new(other));
        assert_ne!(tunings.of(0).think.stamina, tunings.of(1).think.stamina);
        assert_eq!(tunings.match_wide().think.stamina, tunings.of(0).think.stamina);
    }
}
