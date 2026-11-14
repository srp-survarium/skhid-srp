//! [A389] Floors only where the client's capsule is held, and the way down from the last held floor as one drop, priced by the
//! client's fall damage ([`super::Body::held_floors`]; the owner's rule, 2026-11-14: "Ok keep").
//!
//! The rasteriser makes a floor from any piece of a walkable face in a cell, so a broken edge leaves small floors no body stands
//! on, joined to the ground's last floors by Walk links and leaving by a Drop measured from their own height. Chemical Plant
//! (68, 64), the broken ramp's end (owner-5 `cp-two-falls-rootcause.md` item 1, PROVEN): 7 floors on a broken slab (9.56 down
//! to 8.04) behind the ramp's last floors, then Drops of 4.2-5.0 m to the ground at 3.83. In the exe's tick none of the 7 holds
//! a body (7 of 7 end on the ground); walked off at 1 m/s or more the body passes over them and falls 5.3-6.1 m from the
//! ramp's edge; bot 12 fell 6.5 m by the client's count and lost 0.334 of each leg (`fall_damage` 0x2D8220 at 6.5 m). Players
//! go down there too (100 of 126 player worlds at the ramp's end are followed by one 1.5 m+ lower).
//!
//! The rule:
//! 1. A floor at an edge (a side with no Walk link) is kept only where the client's walk tick holds a standing body: set on its
//!    centre ([`super::rest_floors::rests_at`]: out of the walls, 1 cm/s down, no keys, until still) it rests on the ground no more than
//!    a step (`Body::step`, `cc_step_height`) under the floor. A floor no body stays on is our grid's only, no client's place.
//! 2. The way down stays, as one drop: from each kept floor that had a link onto a taken one, the client's body walked at the
//!    taken floor ([`super::drop_check::walk_drop`]) comes to stand on a floor lower down; the take-off gets an off-mesh drop
//!    walked off ([`LinkKind::GapDrop`], no key) to it, its height the true one (the take-off's to the landing's).
//! 3. A drop is priced by what the client's fall takes ([`fall_damage`], [`FALL_DAMAGE_M`] a pool's loss in the search's
//!    metres, the bots' zone search's own price): a drop the harmless height or less costs nothing more; deeper, the route takes
//!    it only when it pays.

use super::controlled::parallel;
use super::drop_check::{walk_drop, DropWalk};
use super::gap::{OffLink, Share, Takeoff};
use super::rest_floors::rests_at;
use super::*;
use crate::controller::tick::Settings;
use crate::controller::Collision;

/// `fall_params.harmless_height`, m: a fall no higher hurts nothing (`docs/notes/fall_damage.md`, PROVEN; the bots' drops'
/// own limit, `Body::max_drop`).
pub const HARMLESS_HEIGHT_M: f32 = 5.0;
/// `fall_params.fatal_height`, m: a fall this high or more deals `fatal_damage` whole (`docs/notes/fall_damage.md`, PROVEN).
pub const FATAL_HEIGHT_M: f32 = 10.0;
/// `fall_params.fatal_damage` (the content patch's 1.0, `scripts/content-edit/character.tsv`; `docs/notes/fall_damage.md`).
pub const FATAL_DAMAGE: f32 = 1.0;
/// `fall_params.gravity`, m/s^2 (`docs/notes/fall_damage.md`, PROVEN).
const GRAVITY: f32 = 9.8;
/// The share of a foot's blunt hit that goes to pain (`docs/notes/fall_damage.md`: foot blunt -> leg x 1, pain x 0.8; PROVEN on
/// bot 12's fall: 0.5348 pain = 2 x 0.3342 x 0.8).
const PAIN_PER_FOOT: f32 = 0.8;
/// A pool's loss in the route search's metres: the bots' zone search's price of damage (`bots::zone_search::DAMAGE_M`, "any
/// damage outweighs any detour the grid has"), so a hurting drop is taken only where no harmless way is near.
pub const FALL_DAMAGE_M: f32 = 1_000.0;

/// The client's fall damage to each foot for a fall of `height` m (`fall_damage` 0x2D8220, `docs/notes/fall_damage.md`): the
/// landing speed v = sqrt(2 g h); nothing up to the harmless height's speed, `fatal_damage` at the fatal height's, linear in v
/// between.
pub fn fall_damage(height: f32) -> f32 {
    let v = |h: f32| (2.0 * GRAVITY * h.max(0.0)).sqrt();
    let (safe, fatal) = (v(HARMLESS_HEIGHT_M), v(FATAL_HEIGHT_M));
    (FATAL_DAMAGE * (v(height) - safe) / (fatal - safe)).clamp(0.0, FATAL_DAMAGE)
}

/// What a drop of `height` m costs a route, m: the pain the fall takes (both feet, [`PAIN_PER_FOOT`] of each foot's hit) at
/// [`FALL_DAMAGE_M`] a pool's loss; 0 for a harmless one.
pub fn fall_cost_m(height: f32) -> f32 {
    2.0 * PAIN_PER_FOOT * fall_damage(height) * FALL_DAMAGE_M
}

/// What [`WalkGrid::drop_unheld`] did ([`super::BuildStats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeldStats {
    /// Edge floors tested …
    pub tested: usize,
    /// … and taken: no body stays on them.
    pub taken: usize,
    /// The drops made from the last held floors (rule 2) …
    pub drops: usize,
    /// … of them deeper than the harmless height (priced by the fall's damage).
    pub hurting: usize,
}

impl WalkGrid {
    /// [`super::Body::held_floors`] (module documentation): takes the edge floors no body stays on and returns the drops from
    /// the last held floors as (take-off, landing) positions, to be made off-mesh links once the floors are renumbered.
    pub(super) fn drop_unheld(&mut self, wide: &mut Wide, world: &Collision, s: &Settings) -> (HeldStats, Vec<(Vec3, Vec3)>) {
        let n = self.len();
        let step = self.body.step;
        let edge: Vec<usize> = (0..n)
            .filter(|&a| (0..DIRS.len()).any(|d| wide.links[a][d] == NONE || wide.kinds[a][d] != LinkKind::Walk))
            .collect();
        let held = parallel(&edge, |&a| {
            let p = self.position(a as NodeId);
            rests_at(world, s, p).is_some_and(|rest| rest.y >= p.y - step)
        });
        let mut alive = vec![true; n];
        for (&a, ok) in edge.iter().zip(&held) {
            alive[a] = *ok;
        }
        let mut stats = HeldStats { tested: edge.len(), taken: held.iter().filter(|h| !**h).count(), ..HeldStats::default() };
        if stats.taken == 0 {
            return (stats, Vec::new());
        }
        // Rule 2: from each kept floor with a link onto a taken one, where the client's body walked at it stands.
        let ways: Vec<(usize, usize)> = (0..n)
            .filter(|&a| alive[a])
            .flat_map(|a| (0..DIRS.len()).map(move |d| (a, d)))
            .filter(|&(a, d)| wide.links[a][d] != NONE && !alive[wide.links[a][d] as usize])
            .collect();
        let start = self.body.walk_start();
        let lands = parallel(&ways, |&(a, d)| {
            let (from, to) = (self.position(a as NodeId), self.position(wide.links[a][d]));
            match walk_drop(world, s, start, from, to) {
                DropWalk::LandsAt(feet) => self
                    .locate_within(feet, 0.5, 0.2)
                    .filter(|&b| alive[b as usize] && self.height[b as usize] < from.y - step && (self.height[b as usize] - feet.y).abs() <= 0.2)
                    .map(|b| self.position(b)),
                _ => None,
            }
        });
        let mut drops: Vec<(Vec3, Vec3)> = Vec::new();
        for (&(a, _), to) in ways.iter().zip(lands) {
            if let Some(to) = to {
                let from = self.position(a as NodeId);
                if !drops.iter().any(|&(f, t)| f == from && t == to) {
                    drops.push((from, to));
                }
            }
        }
        stats.drops = drops.len();
        stats.hurting = drops.iter().filter(|(f, t)| f.y - t.y > HARMLESS_HEIGHT_M).count();
        self.compact(wide, &alive);
        (stats, drops)
    }

    /// The drops of [`Self::drop_unheld`] made off-mesh walk-offs in `wide.off` (kept sorted), where the take-off has no link to
    /// the landing yet.
    pub(super) fn add_unheld_drops(&self, wide: &mut Wide, drops: &[(Vec3, Vec3)]) {
        for &(from, to) in drops {
            let (Some(a), Some(b)) = (self.locate_within(from, 0.01, 0.01), self.locate_within(to, 0.01, 0.01)) else { continue };
            if wide.out(a as usize).any(|x| x == b) {
                continue;
            }
            wide.off.push(OffLink { from: a, to: b, kind: LinkKind::GapDrop, press: Takeoff::WalkOff, share: Share::UNFLOWN, walk: None });
        }
        wide.off.sort_by_key(|l| (l.from, l.to));
        wide.off.dedup_by_key(|l| (l.from, l.to));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fall rule's table (`docs/notes/fall_damage.md`, fatal_damage 1.0): nothing to 5 m, 0.23 at 6 m, 0.44 at 7 m, 1.0 from
    /// 10 m; bot 12's 6.48 m (v 11.27) 0.334.
    #[test]
    fn a_fall_hurts_as_the_clients_rule_says() {
        assert_eq!(fall_damage(4.9), 0.0);
        assert_eq!(fall_damage(5.0), 0.0);
        assert!((fall_damage(6.0) - 0.23).abs() < 0.01, "{}", fall_damage(6.0));
        assert!((fall_damage(7.0) - 0.44).abs() < 0.01, "{}", fall_damage(7.0));
        assert!((fall_damage(6.48) - 0.334).abs() < 0.003, "{}", fall_damage(6.48));
        assert_eq!(fall_damage(12.0), 1.0);
        assert_eq!(fall_cost_m(5.0), 0.0);
    }

    use crate::grid::{OffMeshLinks, STANDING_JUMP_M};
    use crate::{Triangle, WalkerBehaviour};

    fn quad(x0: f32, x1: f32, y: f32) -> [Triangle; 2] {
        let (a, b, c, d) = (Vec3::new(x0, y, 0.0), Vec3::new(x1, y, 0.0), Vec3::new(x1, y, 4.0), Vec3::new(x0, y, 4.0));
        let walker = WalkerBehaviour::BySlope;
        [Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]
    }

    fn face_x(x: f32, y0: f32, y1: f32) -> [Triangle; 2] {
        let (a, b, c, d) = (Vec3::new(x, y0, 0.0), Vec3::new(x, y0, 4.0), Vec3::new(x, y1, 4.0), Vec3::new(x, y1, 0.0));
        let walker = WalkerBehaviour::BySlope;
        [Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]
    }

    /// Ground at 0 (x 0-10), a deck 6 m high to x 4.99, and a strip 0.05 m wide 0.4 m under the deck's edge (x 5-5.05): the
    /// grid walks onto the strip, and the client's body set on it is held by the deck's rim (it rests at 5.975 over the strip's
    /// cell): a floor a body stays on, kept, and no drop is made for it.
    #[test]
    fn a_floor_the_body_is_held_on_stays() {
        let mut t = quad(0.0, 10.0, 0.0).to_vec();
        t.extend(quad(0.0, 4.99, 6.0));
        t.extend(face_x(4.99, 0.0, 6.0));
        t.extend(quad(5.0, 5.05, 5.6));
        let build = |on: bool| {
            let body = Body { held_floors: on, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() };
            WalkGrid::build_with_links(&t, [0.0, 0.0, 10.0, 4.0], 0.25, body, OffMeshLinks::Gaps)
        };
        let strip = |g: &WalkGrid| g.floors_in([5.0, 0.5, 5.25, 3.5]).filter(|&f| (g.position(f).y - 5.6).abs() < 0.1).count();
        let (off, on) = (build(false), build(true));
        assert!(strip(&off) > 0, "the rasteriser makes the strip a floor");
        assert_eq!(strip(&on), strip(&off), "held, kept");
        assert!(!on.off_links().iter().any(|l| l.press == Takeoff::WalkOff && on.position(l.to).y.abs() < 0.1 && on.position(l.from).y > 5.0));
    }
}
