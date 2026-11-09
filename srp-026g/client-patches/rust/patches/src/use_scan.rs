//! Patch 23 and the use scan: base_player::detect_usable_object (0x2D84C0)
//! in Rust. The game's rule with patch 23's (a battery under the floor one
//! stands on: client_patch.md 3.23) is [`rule`]; this module runs it over
//! the game ([`vostok::use_scan`]) at the function's start, and for
//! weapon_pickup, which leaves Use to the game when the scan takes
//! something.
#![forbid(unsafe_code)]

pub mod rule;

use crate::vostok::use_scan::{Usable, UseHit, UseScanCall};
use crate::vostok::world::Player;
use crate::vostok::{Context, Flow, UseScanOnDetectExit};
use rule::{Found, Scene, User};

/// detect_usable_object's start: the scan, its answer given back, out by
/// the function's `ret 8`.
pub fn on_detect(ctx: &mut Context<'_>) -> Flow<UseScanOnDetectExit> {
    let Some(call) = UseScanCall::at(ctx) else { return Flow::Continue };
    let found = detect(&call.player()).map(|found| {
        let spot = found.spot();
        (spot.object.taken(), spot.at)
    });
    call.finish(found);
    Flow::Exit(UseScanOnDetectExit::Returned)
}

/// What a use scan by `player` finds now: the object Use takes, or the
/// first that refused it (for the prompt). None for a dead player.
pub fn detect<'t>(player: &Player<'t>) -> Option<Found<Usable<'t>>> {
    let view = player.use_view()?;
    let reach = player.use_distance();
    let user = User { eye: view.eye, feet: view.feet, reach };
    let mut hits = player.use_ray(rule::ray_length(reach))?;
    rule::scan(&mut Game { player }, &user, hits.iter())
}

/// The game, as the rule asks it.
struct Game<'p, 't> {
    player: &'p Player<'t>,
}

impl<'t> Scene for Game<'_, 't> {
    type Hit = UseHit<'t>;
    type Object = Usable<'t>;

    fn group(&mut self, hit: UseHit<'t>) -> u16 {
        hit.group()
    }

    fn point(&self, hit: UseHit<'t>) -> [f32; 3] {
        hit.point()
    }

    fn usable(&mut self, hit: UseHit<'t>) -> Option<Usable<'t>> {
        hit.usable()
    }

    fn battery_centre(&mut self, object: Usable<'t>) -> Option<[f32; 3]> {
        object.battery_centre()
    }

    fn can_use(&mut self, object: Usable<'t>) -> bool {
        object.can_use(self.player)
    }
}
