//! [body physics S5d] The share-0 sprint links no client variant makes ([`super::Body::share0_drop`]).
//!
//! A sprint press lands only when each of the client's sprint variants does: the supporting leg (the selector's
//! `m_right_leg_is_supporting`, +50h, read at 0x3371F3) and the first air frame (the prepare's phase, 0x3375A0). The bot does not
//! choose either. A link whose press no neighbour backs is stored at share 0 (the aimed sweep's one exact press, [`super::rim`]),
//! and the scoring flight's verdict for it can rest on a knife edge: level_06 TDM's (54.60, 27.65, 100.12) -> (53.10, 29.06,
//! 98.38) was stored with one variant on its top, and all six of its variants flown in the exe (rig_real_v3, the builder's run-up
//! to the bit) fall 4.3 m to (51.1, 24.8, 98.0-98.4).
//!
//! This pass flies each share-0 sprint link's own stored press again, as the builder flies a press (its press way, its aim's side
//! and turn, the sprint run-up, every variant), on until the body is down ([`WalkGrid::fall_ground`]), and drops the link when no
//! variant comes to rest on its landing: within [`LAND_FLAT_M`] across and [`LAND_DY_M`] in height. Those bounds are looser than
//! the scoring flights' own top, so a link that lands in any variant by them stays (teach, do not cut). The census on the S5c
//! grids: 1,378 of 11,620 share-0 links land in no variant; the coverage gate checks no human crossing loses its way.

use super::gap::{OffLink, Share, Takeoff};
use super::{WalkGrid, Wide};
use crate::controller::jump::{self, Kind, Kits, Neighbour};
use crate::controller::tick::Settings;
use crate::controller::Collision;

/// A variant rests on the landing within this of it across, m …
pub const LAND_FLAT_M: f32 = 1.2;
/// … and this in height, m.
pub const LAND_DY_M: f32 = 0.35;

/// What the pass did ([`super::BuildStats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Share0Stats {
    /// Share-0 sprint links judged (a run-up that does not fit is kept unjudged) …
    pub flown: usize,
    /// … of them dropped: no variant rests on the landing.
    pub dropped: usize,
}

impl WalkGrid {
    /// The share-0 sprint links of `wide` flown again; the ones no variant rests on the landing of go (module documentation).
    pub(super) fn drop_unlanded_share0(&self, wide: &mut Wide, world: &Collision, s: &Settings) -> Share0Stats {
        let stats;
        (wide.off, stats) = self.share0_kept(std::mem::take(&mut wide.off), world, s);
        stats
    }

    /// `links` less the share-0 sprint links no variant rests on the landing of, in their order, and what was done.
    fn share0_kept(&self, links: Vec<OffLink>, world: &Collision, s: &Settings) -> (Vec<OffLink>, Share0Stats) {
        let kits = Kits { body: jump::WALK_CAP_MS, fastest: self.body.fastest_walk_cap };
        let lands = super::controlled::parallel(&links, |l| {
            let (kind, lead) = match l.press {
                Takeoff::Sprint(lead) => (Kind::Sprint, lead),
                Takeoff::SprintHeld(lead) => (Kind::SprintHeld, lead),
                _ => return None,
            };
            if l.share != Share::landed(0) {
                return None;
            }
            let (a, b) = (self.position(l.from), self.position(l.to));
            let n = Neighbour { lead: 0.0, side: l.aim.side_m(), turn: l.aim.turn_rad(), dy: 0.0, fastest: false };
            let flights = jump::neighbour_flights_for(world, s, a, l.press_way(a, b), kind, lead.metres(), n, kits, self.fall_ground());
            // A press whose run-up does not fit here is not judged (43 in the census): it stays.
            Some(flights.is_none_or(|flights| {
                flights.iter().any(|steps| {
                    jump::landing_and_rest(steps, |f| f.on_ground)
                        .is_some_and(|(_, rest)| rest.feet.distance_xz(b) <= LAND_FLAT_M && (rest.feet.y - b.y).abs() <= LAND_DY_M)
                })
            }))
        });
        let mut stats = Share0Stats::default();
        let kept = links
            .into_iter()
            .zip(lands)
            .filter(|(_, lands)| {
                stats.flown += usize::from(lands.is_some());
                let gone = *lands == Some(false);
                stats.dropped += usize::from(gone);
                !gone
            })
            .map(|(l, _)| l)
            .collect();
        (kept, stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Body, LinkKind, OffMeshLinks, STANDING_JUMP_M};
    use crate::{Triangle, Vec3, WalkerBehaviour};

    fn quad(tris: &mut Vec<Triangle>, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        tris.push(Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope });
        tris.push(Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope });
    }

    /// Ground at 0 for x < 5 and a block from x 5 to 12 whose top is 1.40 m high (the rim tests' `onto`): the triangles.
    fn block() -> Vec<Triangle> {
        let top = 1.40;
        let mut tris = Vec::new();
        quad(&mut tris, Vec3::new(-2.0, 0.0, 0.0), Vec3::new(-2.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 0.0));
        quad(&mut tris, Vec3::new(5.0, top, 0.0), Vec3::new(5.0, top, 6.0), Vec3::new(12.0, top, 6.0), Vec3::new(12.0, top, 0.0));
        quad(&mut tris, Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, top, 0.0), Vec3::new(5.0, top, 6.0), Vec3::new(5.0, 0.0, 6.0));
        tris
    }

    /// A sprint link stored at share 0 whose press lands on its top stays; the same press stored onto a floor 3 m along the top
    /// from where every variant comes to rest goes; a link not at share 0, or not a sprint, is not judged.
    #[test]
    fn a_share0_sprint_link_goes_only_where_no_variant_rests_on_its_landing() {
        let body = Body { rim_jumps: true, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() };
        let tris = block();
        let start = [Vec3::new(1.0, 0.0, 3.0)];
        let (grid, _) = WalkGrid::build_kept_with_links(&tris, [-2.0, 0.0, 12.0, 6.0], 0.25, body, &start, &start, OffMeshLinks::Gaps);
        let (caster, s) = body.controller();
        let world = Collision::build(&tris).with_caster(caster);
        let made = grid
            .off_links()
            .iter()
            .find(|l| l.kind == LinkKind::GapJump && matches!(l.press, Takeoff::Sprint(_) | Takeoff::SprintHeld(_)))
            .copied()
            .expect("a sprint rim link onto the 1.40 m top");
        let at0 = OffLink { share: Share::landed(0), ..made };
        let to = grid.position(made.to);
        let far = grid.locate(Vec3::new(to.x + 3.0, to.y, to.z), 0.3).expect("a floor 3 m on along the top");
        let past = OffLink { to: far, way: None, ..at0 };
        let walked = OffLink { press: Takeoff::Press(super::super::gap::PressLead::from_metres(0.6)), ..past };
        let full = OffLink { share: Share::FULL, ..past };
        let (kept, stats) = grid.share0_kept(vec![at0, past, walked, full], &world, &s);
        assert_eq!(kept, vec![at0, walked, full], "{stats:?}");
        assert_eq!(stats, Share0Stats { flown: 2, dropped: 1 });
    }
}
