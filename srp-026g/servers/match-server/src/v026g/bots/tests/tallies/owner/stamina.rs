//! The owner counts of the stand-in's stamina ([`super::super::standin_stamina`]): how often the bar locked a
//! body out of sprinting, how long it was held out, and the jumps it refused.

use super::*;
use crate::v026g::bots::tests::standin_stamina;

impl OwnerTally {
    /// The world's counts: times the low flag was set, the seconds a living body had it, the seconds of those
    /// its sprint keys were held (a lockout lasts as long as the key does), the sprint keys held by a body that could
    /// not sprint (low or overburdened), and the jump presses refused for the bar.
    pub(super) fn stamina_part(&self, game: &Game) -> Part {
        let c = standin_stamina::counts_of(game.bodies.iter().map(|b| &b.stamina));
        let secs = |ticks: u32| (10.0 * f64::from(ticks) * f64::from(TICK_S)).round() / 10.0;
        let json = serde_json::json!({
            "stamina_lockouts": c.lockouts,
            "stamina_low_s": secs(c.low_ticks),
            "stamina_lockout_held_s": secs(c.sprint_keys_low),
            "stamina_sprint_refused_s": secs(c.sprint_refused),
            "stamina_jump_low": c.jump_low,
        });
        let text = format!(
            "owner-test world stamina: lockouts {} ({:.1} s low, sprint keys held through {:.1} s), sprint keys held by a body that could not sprint {:.1} s, jumps refused for the bar {}",
            c.lockouts,
            secs(c.low_ticks),
            secs(c.sprint_keys_low),
            secs(c.sprint_refused),
            c.jump_low,
        );
        Part::new(json, text)
    }
}
