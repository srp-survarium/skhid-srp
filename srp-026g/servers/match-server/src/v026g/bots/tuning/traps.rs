//! The bear-trap habit's numbers (`bots/traps.rs`). Each is a GUESS marked so
//! unless it is the game's (`docs/notes/bear_traps.md`); the bench decides.

use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TrapTuning {
    /// Whether a bot that carries traps places them. A bot carries none
    /// until its kit does (`traps::Habit::carry`), so this off or the kit
    /// off gives today's bots bit for bit.
    pub enabled: bool,
    /// Traps a bot puts down in one life.
    pub per_life: u8,
    /// Traps the team's bots keep standing at once (promised or armed).
    pub cap_team: u8,
    /// How far ahead of the feet the floor point is, metres: the head ray
    /// is 2.0 m (PROVEN) and the view looks down at most 1.0 rad
    /// (`body::MAX_PITCH`), so at the eye's 1.6 m the floor is hit 1.0 m out.
    pub reach_m: f32,
    /// The least the trap lies off the bot's own route ahead, metres: the
    /// placer was the victim in 4.2 % of catches, 3 s after placing (PROVEN).
    pub lateral_m: f32,
    /// How far ahead the route is kept clear of it, metres.
    pub route_ahead_m: f32,
    /// A mate's place, or where it is in 2 and 4 s, this near the spot
    /// forbids it, metres.
    pub friend_m: f32,
    /// Another own trap this near forbids it, metres.
    pub near_own_m: f32,
    /// Open floor needed round the spot, outside its 0.9 m disc, within
    /// 2 m (floors of the grid): a trap never shuts a doorway.
    pub open_floors: u32,
    /// No enemy sensed this lately, seconds, and none known this near,
    /// metres.
    pub enemy_quiet_s: f32,
    pub enemy_near_m: f32,
    /// No hit taken this lately, seconds.
    pub hit_quiet_s: f32,
    /// Health at least.
    pub min_health: f32,
    /// A jump of the route this near ahead forbids it, metres.
    pub jump_ahead_m: f32,
    /// The spot's score at least (`traps::score`): enemy presence at it
    /// minus friend flow.
    pub min_score: f32,
    /// Weight of the friend term in the score.
    pub w_friend: f32,
    /// Enemy lane count at which the presence term is 63 %.
    pub presence_scale: f32,
    /// The chance a think that passes the gate takes the best spot.
    pub chance: f32,
    /// The view must be this near the spot's (radians) to press.
    pub tolerance_rad: f32,
    /// A look that has not found the spot after this long is given up, ms.
    pub look_ms: u32,
    /// Seconds before another try after a placement or a give-up.
    pub cooldown_s: f32,
    /// The response to a trap that is not the team's (`bots/trap_react.rs`).
    pub react: ReactTuning,
}

/// How a bot notices a trap of the other team or a human's (C18), and what is
/// done about it. **Off** (`on` false) the bots are as before, bit for bit;
/// a train turns it on by a tuning patch (`traps.react.on`). Every number is
/// a GUESS: the recordings show no measurable reaction of the players to an
/// armed trap, so the chance and the distances are tuned by the bench against
/// the players' numbers (about 10 % of traps end disarmed; a walk-through
/// share of passers of 28-35 %).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReactTuning {
    /// Whether a bot rolls for the traps it comes near, and counts its passes.
    pub on: bool,
    /// The chance of noticing a trap within `near_m` (once a roll).
    pub p0: f32,
    /// Within this the chance is `p0`, metres.
    pub near_m: f32,
    /// Beyond this a trap is not rolled for, metres; the chance falls
    /// linearly from `near_m` to it.
    pub far_m: f32,
    /// The view the trap must lie in, degrees wide (`sense::in_view`).
    pub cone_deg: f32,
    /// Seconds before a missed trap is rolled for again.
    pub roll_s: f32,
    /// The chance is divided by this while the bot is in a fight.
    pub fight_div: f32,
}

impl Default for ReactTuning {
    fn default() -> Self {
        Self {
            on: false,
            p0: 0.05,
            near_m: 3.0,
            far_m: 8.0,
            cone_deg: 140.0,
            roll_s: 20.0,
            fight_div: 3.0,
        }
    }
}

impl Default for TrapTuning {
    fn default() -> Self {
        Self {
            enabled: true,
            per_life: 1,
            cap_team: 4,
            reach_m: 1.0,
            lateral_m: 0.9,
            route_ahead_m: 12.0,
            friend_m: 2.5,
            near_own_m: 4.0,
            open_floors: 12,
            enemy_quiet_s: 4.0,
            enemy_near_m: 50.0,
            hit_quiet_s: 6.0,
            min_health: 0.5,
            jump_ahead_m: 6.0,
            min_score: 0.2,
            w_friend: 1.0,
            presence_scale: 3.0,
            chance: 0.35,
            tolerance_rad: 0.07,
            look_ms: 900,
            cooldown_s: 20.0,
            react: ReactTuning::default(),
        }
    }
}

impl Check for TrapTuning {
    fn check(&self) -> Result<(), String> {
        check(
            self.reach_m > 0.0 && self.reach_m <= 1.2 && self.lateral_m >= 0.0 && self.route_ahead_m >= 0.0,
            "traps.reach_m",
        )?;
        check(
            self.friend_m >= 0.0 && self.near_own_m >= 0.0 && self.jump_ahead_m >= 0.0,
            "traps.*_m",
        )?;
        check(
            self.presence_scale > 0.0 && self.w_friend >= 0.0,
            "traps.presence_scale",
        )?;
        check(
            (0.0..=1.0).contains(&self.chance) && (0.0..=1.0).contains(&self.min_health),
            "traps.chance",
        )?;
        check(self.tolerance_rad > 0.0 && self.look_ms > 0, "traps.look")?;
        let r = &self.react;
        check(
            (0.0..=1.0).contains(&r.p0)
                && r.near_m >= 0.0
                && r.far_m > r.near_m
                && r.cone_deg > 0.0
                && r.cone_deg <= 360.0
                && r.roll_s >= 0.0
                && r.fight_div >= 1.0,
            "traps.react",
        )?;
        Ok(())
    }
}
