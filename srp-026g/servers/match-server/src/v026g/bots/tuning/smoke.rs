//! Smoke grenades (`bots/smoke.rs`): what a cloud hides from a bot's eyes. The cloud's own size and times are the
//! game's (`smoke::CloudShape`), not tuned here.

use super::*;

/// A cloud hides what lies behind it from a bot's eyes, as it does from a player's: a sight line that runs through
/// a live cloud for more than [`Self::thin_m`] sees nothing past it. Bullets go through as before.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SmokeTuning {
    /// Every sight of an enemy (noticing him, the aim's look at his parts) is blocked by the clouds in the match
    /// (`smoke::Veil`). Off: no cloud is read and bots see through smoke, as they did.
    pub sight: bool,
    /// After its target went into smoke, a bot that was firing keeps firing at the last point it saw of him this
    /// long, ms, as a player sprays a smoke he just lost a man in: continuing fire only, never a new burst at a
    /// man it did not see, and at where he was, not where he is (0: it stops at once). The owner's cap for fire
    /// through cover, 0.75 s.
    pub spray_ms: u32,
    /// The least length of a sight line inside the clouds that hides, metres (summed over the clouds it crosses): a
    /// line that only grazes an edge sees past it. Valve's Counter-Strike bot (`CBotManager::IsLineBlockedBySmoke`,
    /// the shipped bot's source, ReGameDLL_CS `game_shared/bot/bot_manager.cpp`) sums the smoked chords the same way
    /// and sees through up to 0.7 of its 115-unit smoke radius, 80 units, about 2.0 m: a smoke's density, not its
    /// size, so the metres carry over: GUESS for 0.26g's 5 m puffs, whose alpha per metre was not read (client test S1). Above
    /// nothing (0 would hide every line while any cloud lives).
    pub thin_m: Positive,
}

impl Default for SmokeTuning {
    fn default() -> Self {
        Self {
            sight: true,
            spray_ms: 750,
            thin_m: Positive::of(2.0),
        }
    }
}
