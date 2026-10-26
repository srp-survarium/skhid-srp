//! The owner counts of the stand-in's weapons ([`super::super::standin_weapon`]): what the world's own
//! magazines and ammo did, against what the bots believe (`dry_s`, `deaths_dry`, the reload ticks).

use super::*;
use crate::v026g::bots::tests::standin_weapon;

/// The stand-in's weapons: it counts nothing as the match runs (the stock keeps its own counts), only
/// sums them at the end.
pub(super) struct StockCounts;

impl Topic for StockCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn parts(&mut self, run: &Run, game: &Game, _scale: &Scale) -> Vec<(&'static str, Part)> {
        vec![("stock", self.part(run, game))]
    }
}

impl StockCounts {
    /// The world's counts: reloads begun, round by round loops the fire or aim key cut, the ticks a living body
    /// was out of rounds and of ammo, and the ticks the fire key was down on a weapon that could not fire (all
    /// of them, and those empty or reloading).
    fn part(&self, run: &Run, game: &Game) -> Part {
        let c = standin_weapon::counts_of(game.bodies.iter().map(|b| &b.stock));
        let bot_min = run.bot_min();
        let secs = |ticks: u32| (10.0 * f64::from(ticks) * f64::from(TICK_S)).round() / 10.0;
        let json = serde_json::json!({
            "world_reloads": c.reloads,
            "world_box_uses": c.box_uses,
            "world_box_topped": c.box_topped,
            "world_box_cut": c.box_cut,
            "world_box_rounds": c.box_rounds,
            "world_cut_loops": c.cut_loops,
            "world_dry_s": secs(c.dry_ticks),
            "world_held_no_shot_s": secs(c.held_no_shot),
            "world_held_empty_or_reloading_s": secs(c.held_empty_or_reloading),
        });
        let text = format!(
            "owner-test world weapons: reloads {} (loops cut by fire or aim {}), out of rounds and ammo {:.1} s, fire held on a weapon that could not fire {:.1} s (empty or reloading {:.1} s), in {:.0} bot minutes; resupply box: uses {} (topped up {}, ended with nothing {}), rounds put in {}",
            c.reloads,
            c.cut_loops,
            secs(c.dry_ticks),
            secs(c.held_no_shot),
            secs(c.held_empty_or_reloading),
            bot_min,
            c.box_uses,
            c.box_topped,
            c.box_cut,
            c.box_rounds,
        );
        Part::new(json, text)
    }
}
