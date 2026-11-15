//! [A389] Floors only where the client's capsule is held, and the way down from the last held floor as one drop, priced by the
//! client's fall damage ([`super::Body::held_floors`]).
//!
//! The rasteriser makes a floor from any piece of a walkable face in a cell, so a broken edge leaves small floors no body stands
//! on, joined to the ground's last floors by Walk links and leaving by a Drop measured from their own height. Chemical Plant
//! (68, 64), the broken ramp's end (PROVEN): 7 floors on a broken slab (9.56 down
//! to 8.04) behind the ramp's last floors, then Drops of 4.2-5.0 m to the ground at 3.83. In the exe's tick none of the 7 holds
//! a body (7 of 7 end on the ground); walked off at 1 m/s or more the body passes over them and falls 5.3-6.1 m from the
//! ramp's edge; bot 12 fell 6.5 m by the client's count and lost 0.334 of each leg (`fall_damage` 0x2D8220 at 6.5 m). Players
//! go down there too (100 of 126 player worlds at the ramp's end are followed by one 1.5 m+ lower).
//!
//! The rule:
//! 1. A floor at an edge (a side with no Walk link) is kept only where the client's walk tick holds a standing body
//!    ([`super::rest_floors::holds`]: come down onto its centre from 0.6 m over it, no keys, 2 s): once landed it never drops
//!    more than a step (`Body::step`, `cc_step_height`) off the ground (a body sliding down a steep walked slope stays on the
//!    ground: held). A floor a body falls off is our grid's only, no client's place.
//! 2. The way down stays, as one drop: from each kept floor that had a link onto a taken one, the client's body walked at the
//!    taken floor ([`super::drop_check::walk_drop`]) comes to stand on a floor lower down; the take-off gets an off-mesh drop
//!    walked off ([`LinkKind::GapDrop`], no key) to it, its height the true one (the take-off's to the landing's).
//! 3. A drop is priced by what the client's fall takes ([`fall_damage`], [`FALL_DAMAGE_M`] a pool's loss in the search's
//!    metres, the bots' zone search's own price): a drop the harmless height or less costs nothing more; deeper, the route takes
//!    it only when it pays.

use super::controlled::parallel;
use super::drop_check::{walk_drop, DropWalk};
use super::gap::{OffLink, Share, Takeoff};
use super::rest_floors::{NAME_HEIGHT_M, holds, rests_at};
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

/// [S6] The speed a body walked off a ledge lands at after a drop of `height` m, m/s: the client's whole ticks
/// (`step_down` 0x7E1B0, PROVEN, `docs/notes/fall_damage.md` "One tick is 0.05 s"; the split ticks fixed by patch 72): while
/// slower than `on_ground_max_vertical_speed` (2.0 m/s, `startup.cfg`) a tick goes down `additional_down_speed` (0.05 m),
/// else by its speed for the tick; each tick adds `gravity` x 0.05 s; the landing is the tick that reaches the floor, at the
/// speed it started with. A drop of 8 m lands at 12.74 m/s (the note's table). A jump off a ledge lands faster (its clip's
/// end speed, the note's "a jump off a ledge"): this is the walk-off's, the least a drop's fall is.
pub fn walk_off_landing_speed(height: f32) -> f32 {
    let s = Settings::LIVE;
    let (mut speed, mut fallen) = (0.0f32, 0.0f32);
    // At most fatal_height's ticks and some (a drop past it lands over the fatal speed).
    for _ in 0..200 {
        let down = if speed < s.on_ground_max_vertical_speed { s.additional_down_speed } else { speed * FALL_TICK_S };
        if fallen + down >= height {
            return speed;
        }
        fallen += down;
        speed += GRAVITY * FALL_TICK_S;
    }
    speed
}

/// [S6] The client's tick, s (`docs/notes/fall_damage.md`, PROVEN: the lockstep's 50 ms).
const FALL_TICK_S: f32 = 0.05;

/// [S6] The hit to each foot of a landing at `speed` m/s (`on_physical_controller_landing` 0x2D8220): over the fatal height's
/// speed `fatal_damage` x the scale, over the harmless height's linear in the speed up to it, else nothing (the speeds
/// `sqrt(2 g h)` of `harmless_height` and `fatal_height`, the loader 0x2D6600). `fall_scale`: `max(0, 1 + modifier 30)`
/// (`apply_modifier` 0x2D5AE0, called at 0x2D82C8 with `fatal_damage` +314h and index 30).
pub fn fall_hit(speed: f32, fall_scale: f32) -> f32 {
    let v = |h: f32| (2.0 * GRAVITY * h).sqrt();
    let (safe, fatal) = (v(HARMLESS_HEIGHT_M), v(FATAL_HEIGHT_M));
    let whole = FATAL_DAMAGE * fall_scale;
    if speed > fatal {
        whole
    } else if speed > safe {
        whole * (speed - safe) / (fatal - safe)
    } else {
        0.0
    }
}

/// [S6] The pain a walk-off drop of `height` m takes at the least ([`walk_off_landing_speed`], [`fall_hit`]): two foot hits,
/// each blunt, of which [`PAIN_PER_FOOT`] goes to pain (`human_hit_params` feet `bdb_coeff`, `docs/notes/hand-kill.md`; the
/// hits right foot then left foot through the player's hit 0x2D9AE0, type 1 at 0x2D8348, no source; no armour stops a blunt
/// hit, `docs/notes/armour-params.md`: no piece lists `blunt`). A pain pool emptied is death (`on_affect` 0x2DA480; `hurt >=
/// pain` the exe's own clamp, 0x32A25A..0x32A28B). The fall is counted from [`RIM_SLIDE_M`] under the ledge: the body slides
/// that far over the rim on ticks that still find ground before it falls, so a drop just past a tick's boundary lands a
/// tick's speed slower than the ledge's height alone gives (the exe review of #1969's run); counting from there never
/// overstates the pain.
pub fn fall_pain(height: f32, fall_scale: f32) -> f32 {
    2.0 * PAIN_PER_FOOT * fall_hit(walk_off_landing_speed((height - RIM_SLIDE_M).max(0.0)), fall_scale)
}

/// [S6] How far a body walked off a ledge slides down over its rim on ticks that still find ground, before its fall begins,
/// m (an exe run of a 2 m ledge: 0.05 m on 3 such ticks, no gravity yet; the exe review of #1969).
const RIM_SLIDE_M: f32 = 0.05;

/// What a drop of `height` m costs a route, m: the pain the fall takes (both feet, [`PAIN_PER_FOOT`] of each foot's hit) at
/// [`FALL_DAMAGE_M`] a pool's loss; 0 for a harmless one.
pub fn fall_cost_m(height: f32) -> f32 {
    2.0 * PAIN_PER_FOOT * fall_damage(height) * FALL_DAMAGE_M
}

/// [R2 F3] How far off a floor's centre the ring's starts are, m: the exe check's ring (Mamayev Kurgan R2, 2026-11-15).
const RING_M: f32 = 0.15;
/// The ring's starts.
const RING_WAYS: usize = 8;

/// What [`WalkGrid::drop_unheld`] did ([`super::BuildStats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeldStats {
    /// Edge floors tested …
    pub tested: usize,
    /// … and taken: no body stays on them.
    pub taken: usize,
    /// … kept by the ring alone: a body come down on the centre falls, one come down a little off it rests on the floor.
    pub by_ring: usize,
    /// The drops made from the last held floors (rule 2) …
    pub drops: usize,
    /// … of them deeper than the harmless height (priced by the fall's damage).
    pub hurting: usize,
    /// … and not made: their take-off is a floor no body gets to from the seeds or spawns (a roof's edge).
    pub unreached: usize,
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
        // Held at its centre ([`holds`]), else [R2 F3] where a body brought down a little off the centre rests on the floor: a
        // narrow stand on a rock's edge drops a body come down on its centre but keeps one that lands beside it (the exe's tick,
        // Mamayev Kurgan (-22.851, 32.311, 122.125): from 0.6 m over the centre it falls 1.95 m; from 0.6 m over a point 0.15 m
        // off it rests 0.14 m from the centre and 0.10 m over it, as a body from over the sliver beside does). The ring is the
        // exe check's: [`RING_M`] out, 8 ways; a rest counts for the floor when the floor names it (within the capsule's radius
        // across and [`NAME_HEIGHT_M`] in height) before any other floor does (a start toward a held neighbour lands on that
        // neighbour, which keeps nothing here).
        let held: Vec<(bool, bool)> = parallel(&edge, |&a| {
            let p = self.position(a as NodeId);
            let room = self.ceiling[a] - self.height[a] - self.body.stand_height;
            if holds(world, s, p, room, step) {
                return (true, false);
            }
            // A start that stays inside steep faces is kept in its pose by both ticks and reads as a rest; from 0.6 m over the
            // floor such a rest is refused by the name's height, but under a low roof the start is that close (the exe review of
            // #1914, Mamayev Kurgan F1/F2: 10 and 15 of 16 ring starts inside the rock). No ring there.
            if room < NAME_HEIGHT_M {
                return (false, false);
            }
            let ring = (0..RING_WAYS).any(|k| {
                let t = k as f32 * std::f32::consts::TAU / RING_WAYS as f32;
                let start = p + Vec3::new(RING_M * t.cos(), 0.0, RING_M * t.sin());
                rests_at(world, s, start, room).is_some_and(|rest| {
                    let named = self
                        .floors_near(rest, self.body.radius)
                        .filter(|&f| self.stand_gap(f, rest.y) <= NAME_HEIGHT_M)
                        .min_by(|&f, &g| self.position(f).distance_xz(rest).total_cmp(&self.position(g).distance_xz(rest)));
                    named == Some(a as NodeId)
                })
            });
            (ring, ring)
        });
        let mut alive = vec![true; n];
        for (&a, &(ok, _)) in edge.iter().zip(&held) {
            alive[a] = ok;
        }
        let mut stats = HeldStats {
            tested: edge.len(),
            taken: held.iter().filter(|h| !h.0).count(),
            by_ring: held.iter().filter(|h| h.1).count(),
            ..HeldStats::default()
        };
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
        self.held_taken = (0..n).filter(|&a| !alive[a]).map(|a| self.position(a as NodeId)).collect();
        self.compact(wide, &alive);
        (stats, drops)
    }

    /// The drops of [`Self::drop_unheld`] made off-mesh walk-offs in `wide.off` (kept sorted), where the take-off is a floor a
    /// body gets to from `seeds` or `spawns` over `wide`'s links and has no link to the landing yet. Returns the drops not made
    /// for an unreached take-off: a drop out of a region no body gets into would make the region lead in, and the build would
    /// keep it (School: 12,862 falls of 10 m or more off roofs, 976,594 roof floors kept).
    pub(super) fn add_unheld_drops(&self, wide: &mut Wide, drops: &[(Vec3, Vec3)], seeds: &[Vec3], spawns: &[Vec3]) -> usize {
        if drops.is_empty() {
            return 0;
        }
        let reached = self.reached_floors(wide, seeds, spawns);
        let mut unreached = 0;
        for &(from, to) in drops {
            let (Some(a), Some(b)) = (self.locate_within(from, 0.01, 0.01), self.locate_within(to, 0.01, 0.01)) else { continue };
            if !reached[a as usize] {
                unreached += 1;
                continue;
            }
            if wide.out(a as usize).any(|x| x == b) {
                continue;
            }
            wide.off.push(OffLink { from: a, to: b, kind: LinkKind::GapDrop, press: Takeoff::WalkOff, share: Share::UNFLOWN, walk: None, aim: super::gap::Aim::STRAIGHT });
        }
        wide.off.sort_by_key(|l| (l.from, l.to));
        wide.off.dedup_by_key(|l| (l.from, l.to));
        unreached
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [S6] The walk-off fall by the client's whole ticks lands as the note's tick table has it (`docs/notes/fall_damage.md`
    /// "Walking or sprinting off": 4 m 8.8 m/s 0; 6 m 10.8, 0.21; 8 m 12.7, 0.69; 10 m 14.2, 1.0). Counted from the rim's
    /// slide ([`RIM_SLIDE_M`]), a full pain pool (1.0) is emptied from 8.05 m (1.6 x 0.69) and not at 8.00 m (1.6 x 0.57), nor
    /// at 7 m; the fall-damage skill's x0.7 saves the 8.1 m drop.
    #[test]
    fn a_walk_off_fall_hurts_by_the_clients_ticks() {
        let near = |a: f32, b: f32| (a - b).abs() < 0.01;
        assert!(near(walk_off_landing_speed(4.0), 8.82) && fall_pain(4.0, 1.0) == 0.0);
        assert!(near(walk_off_landing_speed(6.0), 10.78) && near(fall_hit(walk_off_landing_speed(6.0), 1.0), 0.21));
        assert!(near(walk_off_landing_speed(8.0), 12.74) && near(fall_hit(walk_off_landing_speed(8.0), 1.0), 0.69));
        assert!(near(fall_hit(walk_off_landing_speed(10.0), 1.0), 1.0));
        assert!(fall_pain(8.1, 1.0) >= 1.0 && fall_pain(8.0, 1.0) < 1.0 && fall_pain(7.0, 1.0) < 1.0);
        assert!(fall_pain(8.1, 0.7) < 1.0, "{}", fall_pain(8.1, 0.7));
    }

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
