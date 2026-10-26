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
    /// Traps the team's bots keep standing at once, counting each as
    /// standing for `stand_s` (the worlds will tell their state later).
    pub cap_team: u8,
    /// How long a trap placed is counted standing, seconds.
    pub stand_s: f32,
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
}

impl Default for TrapTuning {
    fn default() -> Self {
        Self {
            enabled: true,
            per_life: 1,
            cap_team: 4,
            stand_s: 240.0,
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
        }
    }
}

pub(super) fn validate(t: &TrapTuning) -> Result<(), String> {
    check(
        t.reach_m > 0.0 && t.reach_m <= 1.2 && t.lateral_m >= 0.0 && t.route_ahead_m >= 0.0,
        "traps.reach_m",
    )?;
    check(
        t.friend_m >= 0.0 && t.near_own_m >= 0.0 && t.jump_ahead_m >= 0.0,
        "traps.*_m",
    )?;
    check(
        t.presence_scale > 0.0 && t.w_friend >= 0.0,
        "traps.presence_scale",
    )?;
    check(
        (0.0..=1.0).contains(&t.chance) && (0.0..=1.0).contains(&t.min_health),
        "traps.chance",
    )?;
    check(
        t.tolerance_rad > 0.0 && t.look_ms > 0 && t.stand_s > 0.0,
        "traps.look",
    )?;
    Ok(())
}
