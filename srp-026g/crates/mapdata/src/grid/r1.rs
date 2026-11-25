//! [body physics S5b] R1, the press point on the take-off's side (fix-builder `pocket-runin-20261123.md`; DESIGN §3.3a): every
//! stored press (a link's own and a sprint link's walking press) is kept only when its press point P is on floor the take-off walks
//! to without the link. A press whose P only the landing walks to (`p_across`, #2462's pattern: its run-up starts across the gap) is
//! dropped. A link left with no press is re-pressed from the take-off's own side when a client press lands from there (the take-off
//! check: along take-off -> landing, every kind and lead), and goes only when none does: teach, do not cut.
//!
//! The rules are the census's (`srp-human-ways-pp.rs --press-points`): P = take-off - press way x lead (plus the aim's side), located
//! within [`LOCATE_M`] at the take-off's level (+-[`LEVEL_M`]); walked to from the take-off by Walk links within the press's lead +
//! its run-in + 1 m; else, walked to from the landing within [`FROM_LANDING_M`]: `p_across`. A press whose P is on neither side
//! (`p_off`) or whose run-up start is not (`s_off`) is kept: those test the grid's floors, not the press (fix-builder).
//! A take-off press lands when every flight of it (each sprint variant) comes down without a skim and rests within [`LAND_FLAT_M`]
//! across and [`LAND_DY_M`] in height of the landing, nearer it than the take-off (the take-off check's verdict, 7 of 40 census
//! links, exe = port on 9 of 9).

use std::collections::BinaryHeap;

use super::gap::{Aim, Mask, OffLink, PressLead, Share, Takeoff};
use super::{WalkGrid, Wide};
use crate::controller::jump::{self, Kind, Kits, Neighbour};
use crate::controller::tick::Settings;
use crate::controller::Collision;
use crate::Vec3;

/// A press point is located within this of where it falls, m.
const LOCATE_M: f32 = 0.2;
/// … at the take-off's level within this, m.
const LEVEL_M: f32 = 0.35;
/// A press point the landing walks to within this is across, m.
const FROM_LANDING_M: f32 = 4.0;
/// The take-off check's rest: within this of the landing across, m …
const LAND_FLAT_M: f32 = 0.6;
/// … and this in height.
const LAND_DY_M: f32 = 0.25;
/// The take-off check's leads, m before the take-off along take-off -> landing.
const TAKE_OFF_LEADS: [f32; 17] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6];

/// What R1 did to a build's off links.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct R1Stats {
    /// Presses whose P is across, dropped.
    pub across: usize,
    /// Links whose own press went and whose walking press took its place.
    pub walk_kept: usize,
    /// Links re-pressed from the take-off's side.
    pub re_pressed: usize,
    /// Links that went: no press of theirs, and none from the take-off, lands.
    pub dropped: usize,
}

/// Where a press's P lies ([`WalkGrid::press_side`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// On the take-off's side.
    TakeOff,
    /// On floor only the landing walks to.
    Across,
    /// On neither (the grid's floors; kept).
    Off,
}

impl WalkGrid {
    /// R1 on the wide grid's off links (before the reach flood, so reach, pieces and pockets see its result).
    pub(super) fn take_off_side_presses(&self, wide: &mut Wide, world: &Collision, s: &Settings) -> R1Stats {
        let (first, to) = wide.walk_adjacency();
        let kits = Kits { body: jump::WALK_CAP_MS, fastest: self.body.fastest_walk_cap };
        let links = std::mem::take(&mut wide.off);
        let judged = super::controlled::parallel(&links, |l| self.r1_link(*l, &first, &to, world, s, kits));
        let mut stats = R1Stats::default();
        wide.off = judged
            .into_iter()
            .filter_map(|(kept, what)| {
                stats.across += what.across;
                stats.walk_kept += what.walk_kept;
                stats.re_pressed += what.re_pressed;
                stats.dropped += what.dropped;
                kept
            })
            .collect();
        stats
    }

    /// One link under R1: the link kept (as it was, with a press dropped, or re-pressed), or None, and what was done.
    fn r1_link(&self, l: OffLink, first: &[u32], to: &[u32], world: &Collision, s: &Settings, kits: Kits) -> (Option<OffLink>, R1Stats) {
        let mut what = R1Stats::default();
        let (a, b) = (self.position(l.from), self.position(l.to));
        let way = l.press_way(a, b);
        let main = press_run_in(l.press, s).map(|(lead, run_in)| self.press_side(l.from, l.to, way, lead, l.aim.side_m(), run_in, first, to));
        let walk = l.walk.map(|(lead, _)| self.press_side(l.from, l.to, way, lead.metres(), 0.0, run_in_m(s, false, 6), first, to));
        if walk == Some(Side::Across) {
            what.across += 1;
        }
        let walk_press = l.walk.filter(|_| walk != Some(Side::Across));
        if main != Some(Side::Across) {
            return (Some(OffLink { walk: walk_press, ..l }), what);
        }
        what.across += 1;
        if let Some((lead, share)) = walk_press {
            what.walk_kept += 1;
            return (Some(OffLink { press: Takeoff::Press(lead), share, walk: None, aim: Aim::STRAIGHT, ..l }), what);
        }
        match self.take_off_press(l, a, b, first, to, world, s, kits) {
            Some((press, share)) => {
                what.re_pressed += 1;
                (Some(OffLink { press, share, walk: None, aim: Aim::STRAIGHT, way: None, ..l }), what)
            }
            None => {
                what.dropped += 1;
                (None, what)
            }
        }
    }

    /// Where the press point of a press `lead` m before the take-off `from` along `way` (and `side` m to its right) lies, its run-in
    /// `run_in` m long ([`Side`]).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn press_side(&self, from: u32, to_node: u32, way: (f32, f32), lead: f32, side: f32, run_in: f32, first: &[u32], to: &[u32]) -> Side {
        let a = self.position(from);
        let right = (way.1, -way.0);
        let (px, pz) = (a.x - way.0 * lead + right.0 * side, a.z - way.1 * lead + right.1 * side);
        let Some(p) = self.locate(Vec3::new(px, a.y, pz), LOCATE_M).filter(|&n| (self.position(n).y - a.y).abs() <= LEVEL_M) else {
            return Side::Off;
        };
        if walk_reaches(self, first, to, from, p, lead + run_in + 1.0) {
            Side::TakeOff
        } else if walk_reaches(self, first, to, to_node, p, FROM_LANDING_M) {
            Side::Across
        } else {
            Side::Off
        }
    }

    /// The take-off check: the best press from the take-off's side along take-off -> landing that lands ([`R1Stats::re_pressed`]):
    /// every kind (the walk, the slow presses, the sprint) at [`TAKE_OFF_LEADS`] whose P is on the take-off's side, its own flight
    /// landing; of those, the best neighbourhood (the priced count, then the count), its share.
    #[allow(clippy::too_many_arguments)]
    fn take_off_press(&self, l: OffLink, a: Vec3, b: Vec3, first: &[u32], to: &[u32], world: &Collision, s: &Settings, kits: Kits) -> Option<(Takeoff, Share)> {
        let (dx, dz) = (b.x - a.x, b.z - a.z);
        let len = dx.hypot(dz).max(1e-6);
        let way = (dx / len, dz / len);
        let lands = |kind: Kind, lead: f32, n: Neighbour| {
            // [body physics S5a fix] Flown to the ground as every pass's flights ([`WalkGrid::fall_ground`]).
            jump::neighbour_flights_for(world, s, a, way, kind, lead, n, kits, self.fall_ground())
                .is_some_and(|flights| !flights.is_empty() && flights.iter().all(|f| rests_on_landing(f, a, b)))
        };
        let own = jump::neighbourhood()[0];
        let kinds = [Kind::Walk, Kind::Slow(0), Kind::Slow(1), Kind::Slow(2), Kind::Slow(3), Kind::Slow(4), Kind::Slow(5), Kind::Sprint];
        let mut best: Option<(u8, u8, Kind, f32, u16)> = None;
        for kind in kinds {
            for lead in TAKE_OFF_LEADS {
                // [body physics S5c] Only a press the grid's file holds: a walking press at lead 0 is none (its 0 cm
                // reads back as a walk-off, `Takeoff::from_centimetres`; the lowest is `PRESS_LEAD_LOWEST_M`). Rudnya batteries'
                // S5c grid had such a re-press, (-49.82, 26.87, 24.60) -> (-49.32, 27.81, 25.10), and its file was refused.
                if !press_of(kind, lead).is_valid() {
                    continue;
                }
                let Some((_, run_in)) = press_run_in(press_of(kind, lead), s) else { continue };
                if self.press_side(l.from, l.to, way, lead, 0.0, run_in, first, to) != Side::TakeOff || !lands(kind, lead, own) {
                    continue;
                }
                let mut bits = 1u16;
                for (i, n) in jump::neighbourhood().into_iter().enumerate().skip(1) {
                    bits |= u16::from(lands(kind, lead, n)) << i;
                }
                let mask = Mask::of_bits(bits).expect("a neighbour's bit");
                let rank = (mask.priced(), mask.count());
                if best.is_none_or(|(p, c, ..)| rank > (p, c)) {
                    best = Some((rank.0, rank.1, kind, lead, bits));
                }
            }
        }
        best.map(|(.., kind, lead, bits)| (press_of(kind, lead), Share::of_mask(Mask::of_bits(bits).expect("a neighbour's bit"))))
    }
}

/// The stored press of `kind` at `lead`.
fn press_of(kind: Kind, lead: f32) -> Takeoff {
    let lead = PressLead::from_metres(lead);
    match kind {
        Kind::Walk => Takeoff::Press(lead),
        Kind::Slow(steps) => Takeoff::Slow { lead, steps },
        Kind::Sprint => Takeoff::Sprint(lead),
        Kind::SprintHeld => Takeoff::SprintHeld(lead),
        Kind::Climb => Takeoff::Climb,
    }
}

/// A stored press's lead and its shortest run-in, m (the client's body's walk-up, played on a flat floor: a walk or sprint press's 6
/// moving steps, a slow press's own); None for a press with no run-up.
fn press_run_in(press: Takeoff, s: &Settings) -> Option<(f32, f32)> {
    match press {
        Takeoff::Press(lead) => Some((lead.metres(), run_in_m(s, false, 6))),
        Takeoff::Sprint(lead) | Takeoff::SprintHeld(lead) => Some((lead.metres(), run_in_m(s, true, 6))),
        Takeoff::Slow { lead, steps } => Some((lead.metres(), run_in_m(s, false, u32::from(steps)))),
        _ => None,
    }
}

/// The client's body's walk-up of `moving` steps from rest on a flat floor (sprinting or not), m ([`jump::walk_up_length`]).
fn run_in_m(s: &Settings, sprint: bool, moving: u32) -> f32 {
    use crate::motion::keys;
    let held = if sprint { keys::FORWARD | keys::SPRINT } else { keys::FORWARD };
    jump::walk_up_length(s, held, moving, jump::WALK_CAP_MS).unwrap_or(0.0)
}

/// Whether a flight comes down without a skim and rests within [`LAND_FLAT_M`] across and [`LAND_DY_M`] in height of the landing
/// `b`, nearer it than the take-off `a`.
fn rests_on_landing(steps: &[jump::FlightStep], a: Vec3, b: Vec3) -> bool {
    let Some((_, rest)) = jump::landing_and_rest(steps, |_| true) else { return false };
    let f = rest.feet;
    f.distance_xz(b) <= LAND_FLAT_M && (f.y - b.y).abs() <= LAND_DY_M && f.distance_xz(b) < f.distance_xz(a)
}

/// Whether the floor `goal` is walked to from `from` by Walk links within `budget` m ([`super::Wide::walk_adjacency`]'s lists).
fn walk_reaches(grid: &WalkGrid, first: &[u32], to: &[u32], from: u32, goal: u32, budget: f32) -> bool {
    #[derive(PartialEq)]
    struct Open(f32, u32);
    impl Eq for Open {}
    impl PartialOrd for Open {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Open {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            other.0.total_cmp(&self.0).then(other.1.cmp(&self.1))
        }
    }
    let mut best = std::collections::HashMap::from([(from, 0.0f32)]);
    let mut open = BinaryHeap::from([Open(0.0, from)]);
    while let Some(Open(g, n)) = open.pop() {
        if n == goal {
            return true;
        }
        if g > best[&n] + 1e-4 {
            continue;
        }
        let p = grid.position(n);
        for &m in &to[first[n as usize] as usize..first[n as usize + 1] as usize] {
            let gm = g + p.distance_xz(grid.position(m));
            if gm <= budget && best.get(&m).is_none_or(|&old| gm < old) {
                best.insert(m, gm);
                open.push(Open(gm, m));
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {

    /// [body physics S5c] Every press R1's take-off check may store is one the grid's file holds: lead 0 is left out for the
    /// walking press (stored, it would read back as a walk-off), and kept for the slow and the sprint ones.
    #[test]
    fn the_take_off_check_stores_only_presses_the_file_holds() {
        let kinds = [Kind::Walk, Kind::Slow(0), Kind::Slow(5), Kind::Sprint];
        let tried: Vec<Takeoff> =
            kinds.iter().flat_map(|&k| TAKE_OFF_LEADS.iter().map(move |&lead| press_of(k, lead))).filter(|p| p.is_valid()).collect();
        assert!(tried.iter().all(|p| p.is_valid()));
        assert!(!press_of(Kind::Walk, 0.0).is_valid(), "a walking press at 0 reads back as a walk-off");
        assert!(press_of(Kind::Sprint, 0.0).is_valid(), "a sprint press keeps its bit");
        assert!(press_of(Kind::Slow(0), 0.0).is_valid());
        assert!(press_of(Kind::Walk, 0.1).is_valid());
    }

    use super::*;
    use crate::grid::gap::GridWay;
    use crate::grid::{Body, LinkKind, OffMeshLinks};
    use crate::{Triangle, WalkerBehaviour};

    fn quad(tris: &mut Vec<Triangle>, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        let walker = WalkerBehaviour::BySlope;
        tris.push(Triangle { v: [a, b, c], walker });
        tris.push(Triangle { v: [a, c, d], walker });
    }

    /// A 1.2 m gap 2 m deep across x 10-11.2 (z 0-8): the take-off floor before it, the landing floor after.
    fn gap_world() -> Vec<Triangle> {
        gap_of(1.2)
    }

    /// A `width` m gap 2 m deep from x 10 (z 0-8).
    fn gap_of(width: f32) -> Vec<Triangle> {
        let mut t = Vec::new();
        let floor = |t: &mut Vec<Triangle>, x0: f32, x1: f32, y: f32| {
            quad(t, Vec3::new(x0, y, 0.0), Vec3::new(x0, y, 8.0), Vec3::new(x1, y, 8.0), Vec3::new(x1, y, 0.0));
        };
        floor(&mut t, 0.0, 10.0, 0.0);
        floor(&mut t, 10.0, 10.0 + width, -2.0);
        floor(&mut t, 10.0 + width, 20.0, 0.0);
        for x in [10.0, 10.0 + width] {
            quad(&mut t, Vec3::new(x, -2.0, 0.0), Vec3::new(x, 0.0, 0.0), Vec3::new(x, 0.0, 8.0), Vec3::new(x, -2.0, 8.0));
        }
        t
    }

    /// A built grid's Walk links as R1's lists (the wide grid's [`super::super::Wide::walk_adjacency`], in the grid's numbering).
    fn walk_lists(grid: &WalkGrid) -> (Vec<u32>, Vec<u32>) {
        let mut first = Vec::new();
        let mut to = Vec::new();
        for a in 0..grid.len() as u32 {
            first.push(to.len() as u32);
            to.extend(grid.links(a).filter(|&(_, k)| k == LinkKind::Walk).map(|(b, _)| b));
        }
        first.push(to.len() as u32);
        (first, to)
    }

    /// [body physics S5b] R1 on a gap link whose stored press point is across (its stored way reversed: P 1.6 m past the take-off,
    /// on the landing's floor, #2462's pattern): the press is across, and the link is re-pressed from the take-off's side, where a
    /// client press lands; the new press's P is on the take-off's side. The link's own stored press is on the take-off's side.
    #[test]
    fn a_press_across_the_gap_is_re_pressed_from_the_take_off() {
        let tris = gap_world();
        let body = Body { exe_collision: true, start_witness: true, ..Body::client() };
        let start = [Vec3::new(2.0, 0.0, 4.0)];
        let (grid, _) = WalkGrid::build_kept_with_links(&tris, [0.0, 0.0, 20.0, 8.0], 0.25, body, &start, &start, OffMeshLinks::Gaps);
        let (caster, s) = body.controller();
        let world = Collision::build(&tris).with_caster(caster);
        let kits = Kits { body: jump::WALK_CAP_MS, fastest: body.fastest_walk_cap };
        let (first, to) = walk_lists(&grid);
        let link = *grid
            .off_links()
            .iter()
            .find(|l| {
                let (a, b) = (grid.position(l.from), grid.position(l.to));
                l.kind == LinkKind::GapJump && a.x < 10.0 && b.x > 11.2 && (a.z - 4.0).abs() < 1.0 && press_run_in(l.press, &s).is_some()
            })
            .expect("a pressed gap link across");
        let (a, b) = (grid.position(link.from), grid.position(link.to));
        let (lead, run_in) = press_run_in(link.press, &s).unwrap();
        let own = grid.press_side(link.from, link.to, link.press_way(a, b), lead, link.aim.side_m(), run_in, &first, &to);
        assert_eq!(own, Side::TakeOff, "the stored press: {link:?}");
        let across = OffLink { press: Takeoff::Press(PressLead::from_metres(1.6)), way: Some(GridWay::of_dir(1)), aim: Aim::STRAIGHT, walk: None, ..link };
        let (_, run_in) = press_run_in(across.press, &s).unwrap();
        assert_eq!(grid.press_side(across.from, across.to, across.press_way(a, b), 1.6, 0.0, run_in, &first, &to), Side::Across);
        let (kept, what) = grid.r1_link(across, &first, &to, &world, &s, kits);
        assert_eq!(what, R1Stats { across: 1, walk_kept: 0, re_pressed: 1, dropped: 0 });
        let kept = kept.expect("re-pressed");
        assert!(kept.way.is_none() && kept.walk.is_none(), "{kept:?}");
        let (lead, run_in) = press_run_in(kept.press, &s).expect("a run-up press");
        assert_eq!(grid.press_side(kept.from, kept.to, kept.press_way(a, b), lead, 0.0, run_in, &first, &to), Side::TakeOff, "{kept:?}");
        assert!(kept.share.mask().is_some_and(|m| m.lands(0)), "{kept:?}");
    }

    /// [body physics S5b] A link over a 6 m gap whose press point is across: no client press from the take-off lands (a sprint
    /// press makes 3.5 m), so it goes.
    #[test]
    fn a_press_across_a_gap_no_take_off_press_makes_goes() {
        let tris = gap_of(6.0);
        let body = Body { exe_collision: true, start_witness: true, ..Body::client() };
        let start = [Vec3::new(2.0, 0.0, 4.0), Vec3::new(18.0, 0.0, 4.0)];
        let (grid, _) = WalkGrid::build_kept_with_links(&tris, [0.0, 0.0, 20.0, 8.0], 0.25, body, &start, &start, OffMeshLinks::Gaps);
        let (caster, s) = body.controller();
        let world = Collision::build(&tris).with_caster(caster);
        let kits = Kits { body: jump::WALK_CAP_MS, fastest: body.fastest_walk_cap };
        let (first, to) = walk_lists(&grid);
        let from = grid.locate(Vec3::new(9.8, 0.0, 4.0), 0.3).expect("the take-off floor");
        let landing = grid.locate(Vec3::new(16.2, 0.0, 4.0), 0.3).expect("the landing floor");
        let across = OffLink {
            from,
            to: landing,
            kind: LinkKind::GapJump,
            // P 6.5 m past the take-off: on the landing's floor (x 16.3).
            press: Takeoff::Press(PressLead::from_metres(6.5)),
            share: Share::landed(1),
            walk: None,
            aim: Aim::STRAIGHT,
            way: Some(GridWay::of_dir(1)),
        };
        let (kept, what) = grid.r1_link(across, &first, &to, &world, &s, kits);
        assert_eq!((kept, what), (None, R1Stats { across: 1, walk_kept: 0, re_pressed: 0, dropped: 1 }));
    }
}
