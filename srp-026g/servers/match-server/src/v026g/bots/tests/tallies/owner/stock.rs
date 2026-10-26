//! The owner counts of the stand-in's weapons ([`super::super::standin_weapon`]): what the world's own
//! magazines and ammo did, against what the bots believe (`dry_s`, `deaths_dry`, the reload ticks).

use super::*;
use crate::v026g::bots::tests::standin_weapon;

impl OwnerTally {
    /// The world's counts: reloads begun, round by round loops the fire or aim key cut, the ticks a living body
    /// was out of rounds and of ammo, and the ticks the fire key was down on a weapon that could not fire (all
    /// of them, and those empty or reloading).
    pub(super) fn stock_part(&self, game: &Game) -> Part {
        let c = standin_weapon::counts_of(game.bodies.iter().map(|b| &b.stock));
        let bot_min = self.recs.iter().enumerate().filter(|(p, _)| self.is_bot[*p]).map(|(_, r)| r.iter().filter(|r| r.alive).count()).sum::<usize>() as f64 * f64::from(TICK_S) / 60.0;
        let secs = |ticks: u32| (10.0 * f64::from(ticks) * f64::from(TICK_S)).round() / 10.0;
        let json = serde_json::json!({
            "world_reloads": c.reloads,
            "world_cut_loops": c.cut_loops,
            "world_dry_s": secs(c.dry_ticks),
            "world_held_no_shot_s": secs(c.held_no_shot),
            "world_held_empty_or_reloading_s": secs(c.held_empty_or_reloading),
        });
        let text = format!(
            "owner-test world weapons: reloads {} (loops cut by fire or aim {}), out of rounds and ammo {:.1} s, fire held on a weapon that could not fire {:.1} s (empty or reloading {:.1} s), in {:.0} bot minutes",
            c.reloads,
            c.cut_loops,
            secs(c.dry_ticks),
            secs(c.held_no_shot),
            secs(c.held_empty_or_reloading),
            bot_min,
        );
        Part::new(json, text)
    }
}
