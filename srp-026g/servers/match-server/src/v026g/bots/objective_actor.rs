//! The objective questions the cross-cutting decisions ask (MODES-READY rule 1): who is doing the scoring action now,
//! and who is acting against the team's objective. A decision asks these and never reads a battery, a mode or a
//! `carrying` field itself.
//!
//! The answers today, for the battery modes and Team Deathmatch:
//! - **objective actor**: an enemy that carries a battery. In Team Deathmatch nobody does. (Research, later: a body
//!   standing in a capture zone.)
//! - **acts against our objective**: an actor the team's theft notice names, a thief of its battery. (The notice is
//!   the HUD's.)
//!
//! R14 moves this into `bots/objective.rs` with the rest of the objective view.

use super::know::TeamKnowledge;
use super::view::PlaceView;

/// The theft notice holds this long, ms (`TeamKnowledge::forget_old` keeps a theft 60 s).
pub const THEFT_NOTICE_MS: u32 = 60_000;

/// Whether `body` is an objective actor now: doing the scoring action, whatever the mode's.
pub fn is_actor(body: &PlaceView) -> bool {
    body.carrying.is_some()
}

/// Whether `body`, an actor, acts against the objective of the team whose knowledge is `team` at `now`.
pub fn acts_against(body: &PlaceView, team: Option<&TeamKnowledge>, now: u32) -> bool {
    body.carrying.is_some_and(|battery| {
        team.is_some_and(|team| {
            team.thefts
                .iter()
                .any(|t| t.battery == battery && now < t.time.saturating_add(THEFT_NOTICE_MS))
        })
    })
}
