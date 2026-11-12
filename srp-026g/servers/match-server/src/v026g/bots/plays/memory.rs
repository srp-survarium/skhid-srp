//! What the plays of a bot remember between runs (a bot's feature state, `bots::features`).

use super::collect::FailedStand;
use super::goals::Apart;
use super::{PlayKind, TargetRef};
use crate::v026g::bots::features::Feature;

/// Plays that failed at once at a target, what the bot remembers of giving way to a teammate on a spot, and
/// the stands its takes of batteries gave up at lately.
#[derive(Default)]
pub struct BotState {
    /// Plays that failed at once at a target, and until when they are not taken there again
    /// (`SET_ASIDE_MS`).
    pub set_aside: Vec<(PlayKind, TargetRef, u32)>,
    /// What it remembers of giving way to a teammate on a spot (`goals::keep_apart`).
    pub apart: Apart,
    /// Stands its takes of batteries gave up at lately: its next take stands elsewhere.
    pub failed_stands: Vec<FailedStand>,
}

impl Feature for BotState {}
