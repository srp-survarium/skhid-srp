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
    /// A forming cloud hides from the client's 1.24 s after the blast (`smoke::CLIENT_FORMING_MS`, where a line
    /// through its middle is 75 % opaque: one puff's texture averages 0.291 alpha, PROVEN; the layering LIKELY),
    /// not the RDG-M shape's 0.7 s (a GUESS from the alpha keys alone, when the middle line is about 0.45 opaque):
    /// a bot sees through a forming cloud as long as a human's client lets him, never longer. Off: 0.7 s.
    /// On: by side, 136 pairs (`full-forming-20261031T0625`), wins +18.5 % (z +2.6), score +1.3 %, kills +1.5 %,
    /// deaths -1.4 %, delivered flat, crossings under a cloud 568 vs 709; the fairness counts 0 in both arms, judged
    /// with the client's forming.
    pub forming_client: bool,
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
    /// [smoke use] A bot outranged by a sniper (play 11's matchup: its weapon short of him), the team sensing him where he
    /// is within `lane_sensed_ms`, smokes the line from him to the first point of its route within `lane_ahead_m` that he
    /// sees, before it steps out: not from where he already sees it, not at a sprint, not a gap shorter than
    /// `lane_open_m`, not a line a cloud already covers, `snipers_a_life` at most. Off: none. Uncut (the line from him to
    /// the bot whenever outranged) it threw 18.3 per 100 lives (`h2h-radio-vs-off-20261029T1852`, live 2.97 for all uses).
    /// On by default: the owner, "in general isnt it the most impacting behaviour against snipers?", and on its rate
    /// "i think its ok" ("Unless its caused by bug": every throw classed, none onto a line he had not, after the eye spread).
    pub anti_sniper: bool,
    /// A life smokes this many snipers' lines at most; its own count beside `lanes_a_life` (a crossing a lane smoke
    /// already covers is not smoked again, the cloud test).
    pub snipers_a_life: u32,
    /// A sniper's crossing is one he sees from his sensed eye and from this far to either side of it, nearer and farther along
    /// the line to the bot, and crouched if he was seen standing, metres: where he is is known to a few decimetres and he can
    /// crouch, and a sliver open from one point is not his line. The sensed eye was 0.1-1.0 m from his eye in the game in the
    /// bench's 11 throws onto crossings he could not see.
    pub sniper_eye_spread_m: Metres,
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
    /// A bot standing to throw a smoke keeps its route (`LegsWhy::ThrowingSmoke`) and walks on along it once the smoke is
    /// away: the smoke was planned for that route's crossing. Off: standing left it no goal and dropped its route
    /// (`RouteEnd::NoGoal`, about 0.24 s into the throw), and the route drawn after the throw could miss the crossing
    /// (`notes-route-20261030T0911`: both "route changed" throws of `h2h-sniper-d-20261030T0850`).
    pub throw_keeps_route: bool,
    /// [A208] Smoke first, then the crossing (`WaitForCloud`): after a smoke's launch the thrower walks its route on to the
    /// last point short of the crossing its threat does not see, and holds there until the cloud hides the planned
    /// line. None: it walks on at the launch and reaches the crossing before its cloud hides (a trace of 283 smoked
    /// sniper crossings, `full-25a-xing-20261031T1801`: 69 of 86 with his line open went over before their cloud hid,
    /// 0.8-2 s early; the cloud pours at the fuse, 4 s from the press, and hides 1.24 s later). ON by default, `crossers`:
    /// by side (136 pairs, `full-wait-crossers-20261101T0214`) smoked crossings reached with the line open fell from 93.5 %
    /// to 51.0 % (z 9.7), score +2.1 % (z +2.0), no guard worse; `all_off.json` keeps it off.
    pub wait_for_cloud: Option<WaitForCloud>,
}

/// [A208] `SmokeTuning::wait_for_cloud`: who holds for a cloud, and what ends the hold before the cloud hides the line.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WaitForCloud {
    pub who: CloudWaiters,
    /// The hold ends this long after the cloud should hide, ms: past the press + the fuse + the forming (the team's
    /// rule, `smoke::Forming`) for its own smoke; past now + those for a mate's in the air. The landing and the
    /// rule's lag are the margin.
    pub margin_ms: u32,
    /// An enemy its own senses (seen, heard, told) put this near within `close_sensed_ms` ends the hold, metres:
    /// a fight is coming to it, and the crossing is not the danger then. Humans throw smokes at a nearest enemy
    /// of 32 m median, 14 m at P10 (`fx-smoke-data.md` section 5).
    pub close_m: Metres,
    pub close_sensed_ms: u32,
}

impl Default for WaitForCloud {
    fn default() -> Self {
        Self { who: CloudWaiters::Thrower, margin_ms: 500, close_m: Metres::of(15.0), close_sensed_ms: 2_000 }
    }
}

/// [A208] Who holds short of a crossing for a cloud.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudWaiters {
    /// The thrower, for its own smoke (every use: the lanes, the sniper, an objective actor's, a Breach thrower's).
    #[default]
    Thrower,
    /// The thrower, and every bot (a carrier too) whose next crossing of a held lane a team cloud in the air or
    /// forming will hide, until it hides.
    Crossers,
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
            forming_client: true,
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
            anti_sniper: true,
            snipers_a_life: 1,
            sniper_eye_spread_m: Metres::of(0.5),
            spot_rules: true,
            claim_m: Metres::of(8.0),
            claim_ms: 3_000,
            throw_keeps_route: true,
            wait_for_cloud: Some(WaitForCloud { who: CloudWaiters::Crossers, ..WaitForCloud::default() }),
        }
    }
}
