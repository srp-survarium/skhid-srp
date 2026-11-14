//! [A389] The gap links flown by the client's own jump code ([`super::Body::gap_flights`]).
//!
//! The clip pass ([`WalkGrid::gap_links`]) judges a gap's presses by the jump clip's root carried over the grid's cells
//! (`SHORT_JUMP_AHEAD`, `fly`): an approximation of the client's flight, which misses ways the client makes. Mamayev Kurgan's
//! gully (A369, `nav-builder-20261112/A369-root-cause.md`): from the ridge (52.8, 40.55, 135.0) to the plateau (56.2, 37.6,
//! 133.0) the clip pass made no link (the tip's route went round, 97.4 m for 6.4 m); on the game's shapes world the client's
//! walking jump lands on the plateau at all 11 leads, the sprint jump in 44 of 44 flights, and 56 players crossed there.
//!
//! With the switch every gap is flown as the rim jumps are (A385-FORMAT, `controller::jump`): each press a client makes (the
//! short jump at the body's walk cap, the slow presses, the sprint jump with every variant) at the gap leads, on the shapes
//! world, the keys let go at the landing; the link goes to the floor the most presses land and stay on, and carries the press
//! whose neighbourhood lands most and its share ([`WalkGrid::best_press`], the rim jumps' own scoring). No clip approximation
//! decides a link. The walk-offs (no press) are made as before ([`WalkGrid::walk_off_link`]); the thinning, the saving and the
//! way back are the clip pass's ([`WalkGrid::thin_gap_links`]).
//!
//! A gap: from a floor along a way with no Walk or Jump link (none, or a Drop), across cells with no floor within a step of
//! the take-off, to the first cell with a floor two or more cells on that is higher, by more than a step, than every floor
//! crossed (walking would go down and up again), no higher than a gap link reaches (above is the rim pass's) and no lower than
//! the harmless drop; within the clip pass's own reach ([`SPRINT_REACH_M`] past the take-off's cell).

use super::gap::{GapStats, OffLink, SPRINT_REACH_M};
use super::rim::{gap_leads, Made};
use super::*;
use crate::controller::jump::{self, FlightStep, Kits};
use crate::controller::tick::Settings;
use crate::controller::Collision;

/// A gap to fly: the take-off, the floor across it, the way, and the height a landing is over (a step over every floor
/// crossed: a press that comes down into the gap is no way across it, the Drop links' and walk-offs' way).
#[derive(Clone, Copy, Debug)]
struct Gap {
    a: u32,
    b: u32,
    d: usize,
    over: f32,
}

impl WalkGrid {
    /// The floor across the gap from `a` along `d` (module documentation) and the gap's bottom: the highest floor crossed, and at
    /// least the floor a body walking off the edge comes down on (the way's Drop link, the walk-off at the walk cap,
    /// [`WalkGrid::walk_off`]: the cells by the edge may have no floor, the erosion's under a face, and the gap's floor beyond
    /// them is no landing across it); -inf when there is neither. None when there is no gap.
    fn gap_across(&self, wide: &Wide, solids: &Solids, a: usize, d: usize) -> Option<(u32, f32)> {
        let h = self.height[a];
        let body = &self.body;
        let reach = body.max_jump.min(MARGINAL_JUMP_M);
        let (i, k) = self.cell_xz(self.cell_of[a] as usize);
        let (di, dk) = DIRS[d];
        let stride = self.cell * ((di * di + dk * dk) as f32).sqrt();
        let linked = |f: u32| wide.links[a].contains(&f);
        let drop = (wide.links[a][d] != NONE && wide.kinds[a][d] == LinkKind::Drop).then(|| self.height[wide.links[a][d] as usize]);
        let walked = self.walk_off(a, d, jump::WALK_CAP_MS, solids).map(|f| self.height[f as usize]);
        let mut crossed = [drop, walked].into_iter().flatten().filter(|&y| y < h - body.step).fold(f32::NEG_INFINITY, f32::max);
        let mut g = 1;
        while (g as f32 - 0.5) * stride <= SPRINT_REACH_M {
            let cell = self.cell_index(i + g * di, k + dk * g)?;
            let floors = self.start[cell]..self.start[cell + 1];
            if g == 1 {
                // A floor within a step in the next cell: no gap; one up within a gap link's reach is the Jump links' (a ledge).
                if floors.clone().any(|f| self.height[f as usize] - h > -body.step) {
                    return None;
                }
            } else {
                let target = floors.clone().filter(|&f| {
                    let y = self.height[f as usize];
                    y - h <= reach
                        && h - y <= body.max_drop
                        && y > crossed + body.step
                        && self.ceiling[f as usize] - y >= body.stand_height
                        && !linked(f)
                });
                if let Some(f) = target.min_by(|&p, &q| (self.height[p as usize] - h).abs().total_cmp(&(self.height[q as usize] - h).abs())) {
                    return Some((f, crossed));
                }
                // A floor within a step goes on (no gap), a higher one than the reach is a wall: either ends the search.
                if floors.clone().any(|f| {
                    let rise = self.height[f as usize] - h;
                    rise.abs() <= body.step || rise > reach
                }) {
                    return None;
                }
            }
            crossed = floors.map(|f| self.height[f as usize]).filter(|&y| y < h + body.step).fold(crossed, f32::max);
            g += 1;
        }
        None
    }

    /// The floor a flight's `steps` from the take-off `a` lands and stays on (A385-FORMAT §1): its landing
    /// ([`jump::landing_and_rest`], the first step out of the jump on the ground, `on_ground` 0x7EF90) over a floor
    /// ([`Self::floor_under`]) two or more cells on from `a`'s, in a gap link's reach of it (no higher than the reach, no lower
    /// than the harmless drop) and over `over` (out of the gap, [`Gap::over`]), with standing room, not one `a` has a link to
    /// (`linked`), and at rest over such a floor: the floor it rests on.
    fn gap_lands_on(&self, steps: &[FlightStep], a: u32, linked: &[NodeId], over: f32) -> Option<NodeId> {
        let h = self.height[a as usize];
        let reach = self.body.max_jump.min(MARGINAL_JUMP_M);
        let (i, k) = self.cell_xz(self.cell_of[a as usize] as usize);
        let onto = |feet: Vec3| {
            self.floor_under(feet).filter(|&g| {
                let y = self.height[g as usize];
                let (gi, gk) = self.cell_xz(self.cell_of[g as usize] as usize);
                (gi - i).abs().max((gk - k).abs()) >= 2
                    && y - h <= reach
                    && h - y <= self.body.max_drop
                    && y > over
                    && self.ceiling[g as usize] - y >= self.body.stand_height
                    && !linked.contains(&g)
            })
        };
        let (_, rest) = jump::landing_and_rest(steps, |f| onto(f.feet).is_some())?;
        onto(rest.feet)
    }

    /// [`super::Body::gap_flights`]: the gap links of the eroded grid flown by the client's jump on `world` (module
    /// documentation), with the walk-offs, kept and thinned as [`Self::gap_links`]'s, sorted by take-off then landing.
    pub(super) fn gap_links_flown(&self, wide: &Wide, solids: &Solids, world: &Collision, s: &Settings, stats: &mut GapStats) -> Vec<OffLink> {
        if self.body.max_jump <= 0.0 {
            return Vec::new();
        }
        let n = self.len();
        // The clip pass's links (its walk-offs and presses) as well: a link goes only where no client action makes it
        // and no player does. A flown press wins its group in the thinning; a clip press is kept, unflown,
        // where none is flown (a gap the candidates below miss, or no flown press lands).
        let clip = self.gap_found(wide, solids, stats);
        let mut found: Vec<(OffLink, usize)> = Vec::new();
        let mut gaps: Vec<Gap> = Vec::new();
        for a in 0..n {
            if self.ceiling[a] - self.height[a] < self.body.stand_height {
                continue;
            }
            for d in 0..DIRS.len() {
                let b = wide.links[a][d];
                if b != NONE && wide.kinds[a][d] != LinkKind::Drop {
                    continue;
                }
                match self.gap_across(wide, solids, a, d) {
                    Some((b, crossed)) => gaps.push(Gap { a: a as u32, b, d, over: crossed + self.body.step }),
                    None => stats.no_gap += 1,
                }
            }
        }
        // The gaps in groups by the thinning's key (a metre of edge, the way, the level, the far floor's level): flown in floor
        // order until one is made (the one the thinning keeps of a group's made links anyway), at most the group's two ends.
        let key = |g: &Gap| {
            let (p, q) = (self.position(g.a), self.position(g.b));
            (p.x.floor() as i32, p.z.floor() as i32, (2.0 * p.y).round() as i32, g.d, (2.0 * q.y).round() as i32)
        };
        gaps.sort_by_key(|g| (key(g), g.a));
        let mut groups: Vec<std::ops::Range<usize>> = Vec::new();
        for (i, g) in gaps.iter().enumerate() {
            match groups.last_mut() {
                Some(r) if key(&gaps[r.start]) == key(g) => r.end = i + 1,
                _ => groups.push(i..i + 1),
            }
        }
        stats.gaps = gaps.len();
        stats.groups = groups.len();
        let leads = gap_leads();
        let kits = Kits { body: jump::WALK_CAP_MS, fastest: self.body.fastest_walk_cap };
        let flown = super::controlled::parallel(&groups, |r| {
            let mut flights = 0;
            let mut tried = 0;
            // At most its first and its last take-off (in floor order): the thinning keeps one link a group, and a group's ends
            // span its metre of edge. A bound of the pass's cost (the flights are the build's dearest step), our choice.
            let group = &gaps[r.clone()];
            let ends: &[Gap] = if group.len() <= 2 { group } else { &[group[0], group[group.len() - 1]] };
            for g in ends {
                tried += 1;
                let (di, dk) = DIRS[g.d];
                let l = ((di * di + dk * dk) as f32).sqrt();
                // The floors `a` has a link to already (its sides and its off-mesh links): a landing there makes no link.
                let linked: Vec<NodeId> = wide.links[g.a as usize].iter().copied().filter(|&f| f != NONE).collect();
                let (made, f) =
                    self.best_press(world, s, g.a, g.b, (di as f32 / l, dk as f32 / l), &leads, |st| self.gap_lands_on(st, g.a, &linked, g.over), kits);
                flights += f;
                // Every press it makes (a jump's, and [S1] a sprint climb's onto its own floor), as the rim pass keeps them.
                if !made.is_empty() {
                    return (Some((*g, made)), tried, flights);
                }
            }
            (None, tried, flights)
        });
        for (made, tried, flights) in flown {
            stats.flown += tried;
            stats.flights += flights;
            let Some((g, made)) = made else {
                stats.no_landing += 1;
                continue;
            };
            for Made { to, press, share, walk } in made {
                let rise = self.height[to as usize] - self.height[g.a as usize];
                let kind = if rise > -self.body.step { LinkKind::GapJump } else { LinkKind::GapDrop };
                stats.made += 1;
                stats.sprint += usize::from(matches!(press, super::gap::Takeoff::Sprint(_)));
                stats.by_share[share.class()] += 1;
                found.push((OffLink { from: g.a, to, kind, press, share, walk }, g.d));
            }
        }
        found.extend(clip);
        found.sort_by_key(|&(l, d)| (l.from, d, l.to));
        let out = self.thin_gap_links(wide, found, stats);
        stats.clip_kept = out.iter().filter(|l| l.press != super::gap::Takeoff::WalkOff && l.share.flown().is_none()).count();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::gap::Takeoff;
    use crate::grid::{OffMeshLinks, STANDING_JUMP_M};
    use crate::WalkerBehaviour;

    const AREA: [f32; 4] = [0.0, 0.0, 14.0, 10.0];

    fn quad(x0: f32, x1: f32, y: f32) -> [Triangle; 2] {
        let (a, b, c, d) = (Vec3::new(x0, y, 0.0), Vec3::new(x1, y, 0.0), Vec3::new(x1, y, 10.0), Vec3::new(x0, y, 10.0));
        let walker = WalkerBehaviour::BySlope;
        [Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]
    }

    fn face_x(x: f32, y0: f32, y1: f32) -> [Triangle; 2] {
        let (a, b, c, d) = (Vec3::new(x, y0, 0.0), Vec3::new(x, y0, 10.0), Vec3::new(x, y1, 10.0), Vec3::new(x, y1, 0.0));
        let walker = WalkerBehaviour::BySlope;
        [Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]
    }

    /// Ground at `ground`, a block to x 6 with its top at `a`, a block from x `6 + gap` with its top at `b`.
    fn blocks(a: f32, b: f32, gap: f32, ground: f32) -> Vec<Triangle> {
        let mut t = quad(0.0, 14.0, ground).to_vec();
        t.extend(quad(0.0, 6.0, a));
        t.extend(face_x(6.0, ground, a));
        t.extend(quad(6.0 + gap, 14.0, b));
        t.extend(face_x(6.0 + gap, ground, b));
        t
    }

    fn grid(t: &[Triangle], flights: bool) -> WalkGrid {
        let body = Body { gap_flights: flights, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() };
        WalkGrid::build_with_links(t, AREA, 0.25, body, OffMeshLinks::Gaps)
    }

    /// The flown links across x 6 (take-off before it, landing past `past`).
    fn across(g: &WalkGrid, past: f32) -> Vec<OffLink> {
        g.off_links().iter().copied().filter(|l| g.position(l.from).x < 6.0 && g.position(l.to).x > past && l.press != Takeoff::WalkOff).collect()
    }

    /// A level gap of 1 m between blocks 2 m high: flown, the client's jump gets across, and each link carries its press and the
    /// share of its neighbourhood that lands (A385-FORMAT §2); the clip pass's links carry none.
    #[test]
    fn a_short_gap_is_jumped_with_a_flown_press_and_its_share() {
        let t = blocks(2.0, 2.0, 1.0, 0.0);
        let flown = across(&grid(&t, true), 7.0);
        assert!(!flown.is_empty(), "a flown link across");
        assert!(flown.iter().all(|l| l.kind == LinkKind::GapJump && l.share.flown().is_some_and(|n| n > 0)), "{flown:?}");
        let clip = across(&grid(&t, false), 7.0);
        assert!(clip.iter().all(|l| l.share.flown().is_none()), "{clip:?}");
    }

    /// A level gap of 6 m: no press of the client's lands across (the sprint jump from a 0.6-1.6 m lead comes down about 4 m on),
    /// no link.
    #[test]
    fn a_gap_no_press_crosses_has_no_flown_link() {
        let t = blocks(2.0, 2.0, 6.0, -3.0);
        assert!(across(&grid(&t, true), 11.0).is_empty());
    }

    /// [A369] A lower block 1.4 m down across a 2.5 m gully 3 m deep (Mamayev Kurgan's ridge -> plateau), a bridge at z 9-10 and
    /// stairs the way back: the client's jump comes down on the block; a link from the ridge to it, its landing more than a step
    /// down a GapDrop.
    #[test]
    fn a_lower_block_across_a_gully_is_jumped_to() {
        let mut t = blocks(2.0, 0.6, 2.5, -1.0);
        let walker = WalkerBehaviour::BySlope;
        let mut flat = |x0: f32, x1: f32, y: f32| {
            let (a, b, c, d) = (Vec3::new(x0, y, 9.0), Vec3::new(x1, y, 9.0), Vec3::new(x1, y, 10.0), Vec3::new(x0, y, 10.0));
            t.extend([Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]);
        };
        flat(6.0, 8.5, 2.0);
        flat(8.5, 9.0, 1.65);
        flat(9.0, 9.5, 1.3);
        flat(9.5, 10.0, 0.95);
        let flown: Vec<OffLink> = across(&grid(&t, true), 8.5).into_iter().filter(|l| l.kind == LinkKind::GapDrop).collect();
        assert!(!flown.is_empty(), "a flown link to the lower block");
        assert!(flown.iter().all(|l| l.share.flown().is_some()), "{flown:?}");
    }
}
