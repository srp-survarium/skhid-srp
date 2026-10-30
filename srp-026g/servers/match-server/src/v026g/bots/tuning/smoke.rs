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
    /// S26/S15). Off: no reaction, and no cloud is learnt (no ray is cast for it). On by default (#801's bench, with the
    /// stand-in's clouds of #719).
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
    /// [smoke use] A bot whose route crosses a held lane ahead (K1, `threat.rs`) within `lane_ahead_m`, before it
    /// steps in and while it is not sprinting (a sprint is across before the cloud forms), smokes the lane: a cloud between the spot and the crossing (`smoke::smoke_between`), if the team
    /// sensed the spot's shooter there within 15 s (`smoke::FAIR_SENSED_MS`) and the kit has a smoke. Off: none. On by default
    /// (#801: 4.23 per 100 lives on map data format 5, live 2.97; no sight or fire through smoke).
    pub cross_lane: bool,
    /// The crossing is this near ahead on the route, metres: a smoke for the gap the bot is about to step into, not
    /// one it may reach later (20 m threw 8.9 per 100 lives, `h2h-radio-vs-off-20261029T1211`; live 2.97).
    pub lane_ahead_m: Metres,
    /// A lane is smoked only while its shooter is known to be there now: the team sensed him at the spot within this,
    /// ms (its own sight or hearing, or the kill feed of a death to him), and the spot has a clear line onto the
    /// crossing. Humans smoke a threat they know of (69 % of live clouds lie between the thrower and the nearest
    /// enemy); a lane only remembered is crossed fast (`threat.sprint`), not smoked. With 15 s and no line test the
    /// bench's bots threw 15.1 per 100 lives (`h2h-free-vs-radio-20261029T1047`; live 2.97).
    pub lane_sensed_ms: u32,
    /// A life smokes this many lanes at most (the coordinator's cap, 2026-10-29): a player carries a smoke or two and
    /// spends it on the crossing that matters. With no cap the bench's bots threw 7.84 per 100 lives on map data
    /// format 5 (`h2h-on-vs-off-20261029T2004`; live 2.97, the feel bar under 2x live).
    pub lanes_a_life: u32,
    /// A lane is smoked only where the route its shooter sees runs this far from the crossing on, metres: a shorter
    /// gap is sprinted (5 m/s) before his answer (the bench's median first sight to first shot, about 1.0 s,
    /// `h2h-on-vs-off-20261029T2103`). Players sprint short gaps and smoke the wide ones.
    pub lane_open_m: Metres,
    /// [smoke use] A bot outranged by a sniper it sees (play 11's matchup: its weapon short of him) smokes the line
    /// between them, as near him as the throw reaches. Off: none.
    pub anti_sniper: bool,
    /// The owner's rules for every smoke use (2026-10-30): "they shouldnt throw smoke if smoke is already there (they should
    /// plan near smoke if its nearby) and several bots at a time should not plan to smoke near same spot".
    /// - A plan counts every cloud live or forming, of either team, and the team's own still in the air, at full size
    ///   (`Clouds::planning`), where it only counted live clouds.
    /// - A region a cloud hides from a held lane's shooter is not covered while it stands: routes cross under it (the
    ///   detour weighs the lanes) and no smoke is planned for it.
    /// - A plan is the team's for `claim_ms`: no mate plans one within `claim_m` of its landing or against the same
    ///   shooter (`TeamKnowledge::smoke_claimed`).
    ///
    /// Off: #801's rules.
    pub spot_rules: bool,
    /// A mate's planned smoke keeps others off within this of its landing, metres: RDG-M's cloud radius.
    pub claim_m: Metres,
    /// For this long after it was planned, ms: the throw's turn, hold and launch (`grenade_give_up_ms` 1.5 s +
    /// `grenade_launch_ms` 1.1 s + the look after 0.3 s); thrown, the cloud in the air counts.
    pub claim_ms: u32,
}

impl SmokeTuning {
    pub(super) fn check(&self) -> Result<(), String> {
        check(self.lane_sensed_ms <= super::super::smoke::FAIR_SENSED_MS, "smoke.lane_sensed_ms")
    }
}

impl Default for SmokeTuning {
    fn default() -> Self {
        Self {
            sight: true,
            spray_ms: 750,
            thin_m: Positive::of(2.0),
            react_watch: true,
            see_m: Metres::of(80.0),
            watch_m: Metres::of(40.0),
            watch_ms: 1_500,
            watch_every_ms: 4_000,
            heard_m: Metres::of(15.0),
            heard_ms: 2_000,
            told_ms: 400,
            cross_lane: true,
            lane_ahead_m: Metres::of(10.0),
            lane_sensed_ms: 5_000,
            lanes_a_life: 1,
            lane_open_m: Metres::of(5.0),
            anti_sniper: false,
            spot_rules: true,
            claim_m: Metres::of(8.0),
            claim_ms: 3_000,
        }
    }
}
