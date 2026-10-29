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
    /// React to a cloud the bot knows (it saw it, heard it pop, `smoke::KnownClouds`) and was not told is its team's:
    /// with nothing else holding its look, glance at the cloud's edge that faces it, where a man coming through steps
    /// out ("watch for enemies covering the smoke's far edge", "peek the smoke's edge", `bp-anticamper-human.md`
    /// S26/S15). Off: no reaction, and no cloud is learnt (no ray is cast for it). OFF until its bench (#719's clouds).
    pub react_watch: bool,
    /// A cloud is seen from this far at most, metres (its middle in the view and a clear line to it). GUESS: the
    /// 5 m puffs of a cloud are plain at 80 m.
    pub see_m: Metres,
    /// An enemy cloud's edge within this is watched, metres (the bots' fight ranges: a man stepping out at 40 m is
    /// in reach of every weapon but a shotgun).
    pub watch_m: Metres,
    /// A watch lasts this long, and comes again this often while the cloud lasts, ms (the glance of a callout).
    pub watch_ms: u32,
    pub watch_every_ms: u32,
    /// A cloud's pop is heard whatever way the bot faces within this, metres, while it begins to pour
    /// (`heard_ms`). GUESS: the smoke grenade's pop and hiss, no sound data read (client step S5b).
    pub heard_m: Metres,
    pub heard_ms: u32,
    /// A mate's smoke is known to the thrower's team this long after its pop, ms: a callout (the justified-cheat
    /// ledger's "own smoke told" row, as `avoid.told_ms` for a mate's frag; the thrower knows his own at once).
    /// Until then a mate's cloud seen or heard is of unknown side and watched as a possible enemy's.
    pub told_ms: u32,
}

impl Default for SmokeTuning {
    fn default() -> Self {
        Self {
            sight: true,
            spray_ms: 750,
            thin_m: Positive::of(2.0),
            react_watch: false,
            see_m: Metres::of(80.0),
            watch_m: Metres::of(40.0),
            watch_ms: 1_500,
            watch_every_ms: 4_000,
            heard_m: Metres::of(15.0),
            heard_ms: 2_000,
            told_ms: 400,
        }
    }
}
