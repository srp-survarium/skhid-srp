//! [body physics S5b] The stored run-ins (DESIGN §3.3a, in its derived form): after the flights, each stored press's run-up is played
//! again from the link as stored, and the moving steps it took are kept ([`WalkGrid::run_ins`], read by [`WalkGrid::run_in`]). The
//! run-up is the builder's own ([`jump::run_up`] / [`jump::run_up_slow`]: the client's body, `motion::ClientBody`), at the press's
//! own neighbour (its lead, its aim's side and turn, the grid body's walk cap, no height offset), so the run-in a reader derives from
//! the stored link is the one the link was flown with.

use super::gap::{OffLink, Takeoff};
use super::{WalkGrid, NO_RUN_IN};
use crate::controller::jump::{self, Neighbour};
use crate::controller::tick::Settings;
use crate::controller::Collision;

impl WalkGrid {
    /// The stored run-ins of every off link ([`Self::run_ins`]), played in `world` with the settings `s`.
    pub(super) fn record_run_ins(&mut self, world: &Collision, s: &Settings) {
        let cap = jump::WALK_CAP_MS;
        let links: Vec<(OffLink, crate::Vec3, crate::Vec3)> =
            self.off.iter().map(|l| (*l, self.position(l.from), self.position(l.to))).collect();
        self.run_ins = super::controlled::parallel(&links, |&(l, from, to)| {
            let way = l.press_way(from, to);
            let own = |side: f32, turn: f32| Neighbour { lead: 0.0, side, turn, dy: 0.0, fastest: false };
            let main = press_moving(world, s, from, way, l.press, own(l.aim.side_m(), l.aim.turn_rad()), cap);
            let walk = l.walk.and_then(|(lead, _)| press_moving(world, s, from, way, Takeoff::Press(lead), own(0.0, 0.0), cap));
            main.unwrap_or(NO_RUN_IN) | (walk.unwrap_or(NO_RUN_IN) << 8)
        });
    }
}

/// The moving steps of the run-up the press `press` from the take-off `take_off` along `way` (flat, unit) plays at the neighbour
/// `n` (its side and turn: the press's aim), at the walk cap `cap`: a walk press's ([`jump::run_up`] at the cap), a sprint press's
/// (at the sprint's speed, sprinting), a slow press's ([`jump::run_up_slow`], its own steps). None for a press with no run-up, or a
/// run-up that does not fit.
pub(super) fn press_moving(world: &Collision, s: &Settings, take_off: crate::Vec3, way: (f32, f32), press: Takeoff, n: Neighbour, cap: f32) -> Option<u16> {
    let flight = jump::Flight { cap, dy: 0.0 };
    let (c, sn) = (n.turn.cos(), n.turn.sin());
    let heading = (way.0 * c - way.1 * sn, way.0 * sn + way.1 * c);
    let run = match press {
        Takeoff::Press(lead) => jump::run_up(world, s, jump::press_point(take_off, way, lead.metres(), n), heading, cap, false),
        Takeoff::Sprint(lead) | Takeoff::SprintHeld(lead) => {
            jump::run_up(world, s, jump::press_point(take_off, way, lead.metres(), n), heading, flight.sprint_ms(), true)
        }
        Takeoff::Slow { lead, steps } => jump::run_up_slow(world, s, jump::press_point(take_off, way, lead.metres(), n), heading, cap, steps),
        _ => None,
    }?;
    u16::try_from(run.moving).ok().filter(|&m| m != NO_RUN_IN)
}
