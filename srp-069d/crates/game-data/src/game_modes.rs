//! Game-mode timing per level project (`data/game_modes_069d.json`, made by `crates/data-extract` from the
//! `mode.parameters` block of every
//! `gameplay/game_modes/<project>.options`). The match server ends an online match itself: a client closes it only on a
//! DF leave bit (0x2001B0B6E..0x2001B0B76), so it needs the numbers the clients' timelimit
//! rule runs on.

use std::collections::BTreeMap;

use serde::Deserialize;

pub const FILE: &str = "game_modes_069d.json";

/// The timelimit rule's parameters of one project (absent fields are None).
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
pub struct ModeTiming {
    /// seconds from the enter that reaches the wait threshold to "Match started"
    pub countdown_time: Option<f64>,
    /// seconds of play after "Match started" (0 = no time limit)
    pub match_time: Option<f64>,
    /// seconds left after a team drops to the leave threshold
    pub advantage_time: Option<f64>,
    /// seconds the match waits for players before it ends without a start
    pub wait_time: Option<f64>,
    /// share of the humans that must be in for the countdown
    pub wait_player_percent: Option<f64>,
    /// share of a team that may leave before the advantage cut
    pub leave_player_percent: Option<f64>,
}

#[derive(Debug, Default)]
pub struct GameModes {
    by_project: BTreeMap<String, ModeTiming>,
}

#[derive(Deserialize)]
struct Raw {
    game_modes: BTreeMap<String, ModeTiming>,
}

impl GameModes {
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        let raw: Raw = serde_json::from_str(json)?;
        Ok(GameModes { by_project: raw.game_modes })
    }

    /// The timing of a project (`ModeRow::project`, e.g. `level_01_day_tdm`).
    pub fn get(&self, project: &str) -> Option<&ModeTiming> {
        self.by_project.get(project)
    }

    pub fn len(&self) -> usize {
        self.by_project.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_project.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tdm_day_timing_is_the_options_file() {
        crate::require_tables!();
        let m = GameModes::parse(&crate::testing::read("game_modes_069d.json")).unwrap();
        assert!(m.len() > 300);
        let t = m.get("level_01_day_tdm").unwrap();
        assert_eq!((t.countdown_time, t.match_time, t.advantage_time, t.wait_time), (Some(20.0), Some(900.0), Some(30.0), Some(300.0)));
        assert_eq!((t.wait_player_percent, t.leave_player_percent), (Some(0.6), Some(0.51)));
    }
}
