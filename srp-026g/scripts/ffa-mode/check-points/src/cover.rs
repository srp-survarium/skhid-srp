//! Where a Free for All spawn may stand (docs/notes/ffa.md, "Cover"): with
//! cover, and not in the play area's perimeter band. The owner, 2026-11-14:
//! "there should be a rule that spawns are inside buildings or close to two
//! walls or some cover", "and none at the perimeter"; calibrated on his 46
//! picks on Rudnya (`private/spawn-examples/rudnya-2026-11-14.tsv`).
//!
//! - **Cover**, any of three, measured on the bullet world (what stops shots
//!   and sight) round the spawn's feet:
//!   - a roof: collision straight over the feet within [`ROOF`];
//!   - a corner: two walls (collision at [`HIGH`], what hides a standing
//!     player) within [`CORNER_REACH`], their faces at least
//!     [`CORNER_ANGLE`] apart;
//!   - a side: an obstacle at [`LOW`] (what hides a crouching player) within
//!     [`SIDE_REACH`] over an arc of at least [`SIDE_ARC`] round the spawn.
//! - **Inside the play area**: deeper than [`BAND`] of the deepest point's
//!   depth in the walkable area ([`Area`], to_center's measure).
//!
//! [`Surroundings::measure`] reads the collision once into numbers;
//! [`Surroundings::cover`] and [`Area::band`] are the rule.
use mapdata::{RayTree, Vec3};

use crate::Map;

/// Rays round the spawn, from +z towards +x (the console's `where` yaw).
pub const DIRECTIONS: usize = 32;
/// How far the rays round the spawn look.
pub const REACH: f32 = 12.0;
/// How far up the roof ray looks, from the feet.
pub const ROOF_REACH: f32 = 30.0;

// The rule's numbers, calibrated on the owner's picks on Rudnya: 44 of the 46
// have cover (39 and 40, on a flat roof, nothing within 2.5 m), 44 are past
// the band (21 at 1 m from the outline, 25 at 6.2 m); of a sample of
// Rudnya's walkable floors (one in 4 m), 36 % have cover.

/// A roof: collision this high over the feet at most (a storey; the
/// picks' highest is 5.3 m).
pub const ROOF: f32 = 6.0;
/// Heights over the feet: what hides a crouching player (its capsule is
/// 1.1 m), and a standing one (1.8 m).
pub const LOW: f32 = 0.9;
pub const HIGH: f32 = 1.6;
/// A corner: two wall faces this near, this far apart in angle.
pub const CORNER_REACH: f32 = 2.5;
pub const CORNER_ANGLE: f32 = 60.0;
/// A side: low collision this near over this arc (degrees, about a fifth
/// of the way round).
pub const SIDE_REACH: f32 = 2.5;
pub const SIDE_ARC: f32 = 75.0;
/// The perimeter band: within this share of the deepest point's depth
/// (Rudnya's deepest is 66.5 m: the band is 6.6 m). to_center's band of
/// the move to the centre was 0.3: 16 of the 46 picks are in that.
pub const BAND: f32 = 0.1;

/// The direction of ray `k` on the ground plane.
pub fn direction(k: usize) -> Vec3 {
    let a = k as f32 * std::f32::consts::TAU / DIRECTIONS as f32;
    Vec3::new(a.sin(), 0.0, a.cos())
}

/// Its heading in degrees, from +z towards +x.
pub fn degrees(k: usize) -> f32 {
    k as f32 * 360.0 / DIRECTIONS as f32
}

/// The bullet world's rays (what stops shots and sight).
pub struct Sight(RayTree);

impl Sight {
    pub fn of(map: &Map) -> Self {
        Self(RayTree::build(&map.data.bullet_triangles()))
    }
}

/// A wall the rays met: its distance, the ray's heading, and its face's
/// direction in plan (degrees, 0..180), from the hits beside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wall {
    pub distance: f32,
    pub heading: f32,
    pub face: Option<f32>,
}

/// Which cover a spawn has, the strongest first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cover {
    /// Collision `height` over the feet.
    Roof { height: f32 },
    /// Two walls within CORNER_REACH, their faces `angle` degrees apart.
    Corner { first: Wall, second: Wall, angle: f32 },
    /// Low collision within SIDE_REACH over `arc` degrees round `heading`.
    Side { arc: f32, heading: f32 },
    /// None of them.
    Open,
}

impl Cover {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Roof { .. } => "roof",
            Self::Corner { .. } => "corner",
            Self::Side { .. } => "side",
            Self::Open => "none",
        }
    }

    pub fn holds(&self) -> bool {
        !matches!(self, Self::Open)
    }
}

/// What is round a spawn's feet: the roof ray and the two rings, each ray's
/// distance to collision (`None`: nothing within REACH).
#[derive(Clone, Debug, PartialEq)]
pub struct Surroundings {
    pub roof: Option<f32>,
    pub low: [Option<f32>; DIRECTIONS],
    pub high: [Option<f32>; DIRECTIONS],
}

impl Surroundings {
    pub fn measure(sight: &Sight, feet: Vec3) -> Self {
        let up = Vec3::new(0.0, 1.0, 0.0);
        let start = 0.2;
        let roof = sight
            .0
            .ray_cast(feet + up * start, up, ROOF_REACH - start)
            .map(|t| t + start);
        let ring = |height: f32| {
            std::array::from_fn(|k| sight.0.ray_cast(feet + up * height, direction(k), REACH))
        };
        Self {
            roof,
            low: ring(LOW),
            high: ring(HIGH),
        }
    }

    /// The walls of the high ring within `reach`, nearest first.
    pub fn walls(&self, reach: f32) -> Vec<Wall> {
        let hit = |k: usize| self.high[k % DIRECTIONS].filter(|&t| t <= reach);
        let at = |k: usize, t: f32| direction(k % DIRECTIONS) * t;
        let mut walls: Vec<Wall> = (0..DIRECTIONS)
            .filter_map(|k| {
                let t = hit(k)?;
                // The face through the hits on either side, when they are of
                // the same wall (close to this one).
                let (before, after) = (k + DIRECTIONS - 1, k + 1);
                let face = match (hit(before), hit(after)) {
                    (Some(a), Some(b)) => {
                        let (p, q) = (at(before, a), at(after, b));
                        let middle = at(k, t);
                        let span = (q - p).length();
                        let straight = (middle - (p + q) * 0.5).length();
                        (span > 0.0 && span < 1.5 * t.max(0.5) && straight < 0.25 * span)
                            .then(|| (q.x - p.x).atan2(q.z - p.z).to_degrees().rem_euclid(180.0))
                    }
                    _ => None,
                };
                Some(Wall {
                    distance: t,
                    heading: degrees(k),
                    face,
                })
            })
            .collect();
        walls.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        walls
    }

    /// Two walls within CORNER_REACH whose faces part the most, if any.
    pub fn corner(&self) -> Option<(Wall, Wall, f32)> {
        let walls = self.walls(CORNER_REACH);
        let mut best: Option<(Wall, Wall, f32)> = None;
        for (i, a) in walls.iter().enumerate() {
            for b in &walls[i + 1..] {
                let (Some(fa), Some(fb)) = (a.face, b.face) else { continue };
                let d = (fa - fb).rem_euclid(180.0);
                let angle = d.min(180.0 - d);
                if best.is_none_or(|(_, _, x)| angle > x) {
                    best = Some((*a, *b, angle));
                }
            }
        }
        best
    }

    /// The widest arc of the low ring hit within SIDE_REACH: its width and
    /// middle heading in degrees.
    pub fn side(&self) -> Option<(f32, f32)> {
        let hit = |k: usize| self.low[k % DIRECTIONS].is_some_and(|t| t <= SIDE_REACH);
        if (0..DIRECTIONS).all(hit) {
            return Some((360.0, 0.0));
        }
        let start = (0..DIRECTIONS).find(|&k| !hit(k))?;
        let (mut best, mut run, mut from) = (None::<(usize, usize)>, 0, 0);
        for i in 1..=DIRECTIONS {
            let k = start + i;
            if hit(k) {
                if run == 0 {
                    from = k;
                }
                run += 1;
                if best.is_none_or(|(n, _)| run > n) {
                    best = Some((run, from));
                }
            } else {
                run = 0;
            }
        }
        best.map(|(n, from)| {
            let step = 360.0 / DIRECTIONS as f32;
            (n as f32 * step, (degrees(from) + (n - 1) as f32 * step / 2.0).rem_euclid(360.0))
        })
    }

    /// The rule's cover: the first of a roof, a corner, a side.
    pub fn cover(&self) -> Cover {
        if let Some(height) = self.roof.filter(|&h| h <= ROOF) {
            return Cover::Roof { height };
        }
        if let Some((first, second, angle)) = self.corner().filter(|c| c.2 >= CORNER_ANGLE) {
            return Cover::Corner { first, second, angle };
        }
        match self.side() {
            Some((arc, heading)) if arc >= SIDE_ARC => Cover::Side { arc, heading },
            _ => Cover::Open,
        }
    }
}

/// The walkable area: a raster of depths (metres from the outline; `None`
/// outside), of the reachable standing floors at `Area::CELL` metres,
/// closed by one cell and with its holes filled (to_center's measure).
pub struct Area {
    pub x0: f32,
    pub z0: f32,
    pub width: usize,
    pub height: usize,
    pub depth: Vec<Option<f32>>,
    pub max: f32,
    pub centroid: (f32, f32),
}

impl Area {
    pub const CELL: f32 = 2.0;

    /// The area of `map`'s reachable standing floors inside `bounds`.
    pub fn of(map: &Map, bounds: Option<[f32; 4]>) -> Result<Self, String> {
        let positions: Vec<Vec3> = map
            .standing_floors(bounds)
            .iter()
            .map(|&n| map.grid.position(n))
            .collect();
        Self::new(&positions)
    }

    pub fn new(floors: &[Vec3]) -> Result<Self, String> {
        const CELL: f32 = Area::CELL;
        if floors.is_empty() {
            return Err("no reachable standing floor".into());
        }
        let pad = 3.0 * CELL;
        let lo = floors.iter().fold(floors[0], |a, p| a.min(*p));
        let hi = floors.iter().fold(floors[0], |a, p| a.max(*p));
        let (x0, z0) = (lo.x - pad, lo.z - pad);
        let width = ((hi.x + pad - x0) / CELL).ceil() as usize + 1;
        let height = ((hi.z + pad - z0) / CELL).ceil() as usize + 1;
        let mut floor = vec![false; width * height];
        for p in floors {
            let (i, j) = (((p.x - x0) / CELL) as usize, ((p.z - z0) / CELL) as usize);
            floor[j * width + i] = true;
        }
        let neighbours = |i: usize, j: usize| {
            (-1i32..=1).flat_map(move |dj| {
                (-1i32..=1).filter_map(move |di| {
                    let (a, b) = (i as i32 + di, j as i32 + dj);
                    (a >= 0 && b >= 0 && (a as usize) < width && (b as usize) < height)
                        .then_some((a as usize, b as usize))
                })
            })
        };
        // Close by one cell: grow, fill the holes (what the raster's border
        // cannot reach), shrink.
        let grown: Vec<bool> = (0..width * height)
            .map(|k| neighbours(k % width, k / width).any(|(a, b)| floor[b * width + a]))
            .collect();
        let mut outside = vec![false; width * height];
        let mut stack: Vec<(usize, usize)> = (0..width)
            .flat_map(|i| [(i, 0), (i, height - 1)])
            .chain((0..height).flat_map(|j| [(0, j), (width - 1, j)]))
            .collect();
        while let Some((i, j)) = stack.pop() {
            let k = j * width + i;
            if outside[k] || grown[k] {
                continue;
            }
            outside[k] = true;
            for (a, b) in neighbours(i, j) {
                if (a == i || b == j) && !outside[b * width + a] {
                    stack.push((a, b));
                }
            }
        }
        let inside: Vec<bool> = (0..width * height)
            .map(|k| !neighbours(k % width, k / width).any(|(a, b)| outside[b * width + a]))
            .collect();
        let edge: Vec<(f32, f32)> = (0..width * height)
            .filter(|&k| {
                !inside[k] && neighbours(k % width, k / width).any(|(a, b)| inside[b * width + a])
            })
            .map(|k| ((k % width) as f32, (k / width) as f32))
            .collect();
        let mut depth = vec![None; width * height];
        let (mut max, mut sx, mut sz, mut n) = (0f32, 0f32, 0f32, 0f32);
        for k in (0..width * height).filter(|&k| inside[k]) {
            let (i, j) = ((k % width) as f32, (k / width) as f32);
            let d = edge
                .iter()
                .map(|(a, b)| (a - i).hypot(b - j))
                .fold(f32::INFINITY, f32::min);
            let d = (d - 0.5) * CELL;
            depth[k] = Some(d);
            max = max.max(d);
            sx += x0 + (i + 0.5) * CELL;
            sz += z0 + (j + 0.5) * CELL;
            n += 1.0;
        }
        Ok(Self {
            x0,
            z0,
            width,
            height,
            depth,
            max,
            centroid: (sx / n, sz / n),
        })
    }

    /// The depth at `p`, 0 outside the area.
    pub fn at(&self, p: Vec3) -> f32 {
        let (i, j) = ((p.x - self.x0) / Self::CELL, (p.z - self.z0) / Self::CELL);
        if i < 0.0 || j < 0.0 || i as usize >= self.width || j as usize >= self.height {
            return 0.0;
        }
        self.depth[j as usize * self.width + i as usize].unwrap_or(0.0)
    }

    /// The perimeter band's inner edge: BAND of the deepest point's depth.
    pub fn band(&self) -> f32 {
        BAND * self.max
    }

    /// Whether `p` is inside the play area, past the perimeter band.
    pub fn inside(&self, p: Vec3) -> bool {
        self.at(p) > self.band()
    }
}

/// A spawn's place by the rule: its cover and how deep it stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub surroundings: Surroundings,
    pub cover: Cover,
    pub depth: f32,
    pub inside: bool,
}

impl Placement {
    pub fn of(sight: &Sight, area: &Area, feet: Vec3) -> Self {
        let surroundings = Surroundings::measure(sight, feet);
        Self {
            cover: surroundings.cover(),
            surroundings,
            depth: area.at(feet),
            inside: area.inside(feet),
        }
    }

    /// Both parts of the rule.
    pub fn holds(&self) -> bool {
        self.cover.holds() && self.inside
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(f: impl Fn(Vec3) -> Option<f32>) -> [Option<f32>; DIRECTIONS] {
        std::array::from_fn(|k| f(direction(k)))
    }

    /// The distance along `d` from the origin to the line x = `x` (or z).
    fn to_x(x: f32) -> impl Fn(Vec3) -> Option<f32> {
        move |d: Vec3| (d.x * x > 1e-6).then(|| x / d.x).filter(|&t| t <= REACH)
    }

    fn to_z(z: f32) -> impl Fn(Vec3) -> Option<f32> {
        move |d: Vec3| (d.z * z > 1e-6).then(|| z / d.z).filter(|&t| t <= REACH)
    }

    fn nearest(a: Option<f32>, b: Option<f32>) -> Option<f32> {
        match (a, b) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    #[test]
    fn open_ground_has_no_cover() {
        let s = Surroundings {
            roof: None,
            low: [None; DIRECTIONS],
            high: [None; DIRECTIONS],
        };
        assert_eq!(s.cover(), Cover::Open);
    }

    #[test]
    fn a_roof_overhead_is_cover() {
        let s = Surroundings {
            roof: Some(2.8),
            low: [None; DIRECTIONS],
            high: [None; DIRECTIONS],
        };
        assert_eq!(s.cover().name(), "roof");
    }

    #[test]
    fn one_straight_wall_is_a_side_and_two_at_right_angles_a_corner() {
        let wall = ring(to_x(1.0));
        let one = Surroundings {
            roof: None,
            low: wall,
            high: wall,
        };
        assert_eq!(one.cover().name(), "side");
        assert!(one.corner().is_none_or(|c| c.2 < 1.0));
        let (x, z) = (to_x(1.0), to_z(1.5));
        let two = ring(|d| nearest(x(d), z(d)));
        let corner = Surroundings {
            roof: None,
            low: two,
            high: two,
        };
        let Cover::Corner { angle, .. } = corner.cover() else {
            panic!("{:?}", corner.cover());
        };
        assert!((angle - 90.0).abs() < 1.0, "{angle}");
    }

    #[test]
    fn a_far_wall_is_no_cover() {
        let wall = ring(to_x(SIDE_REACH + 1.0));
        let s = Surroundings {
            roof: None,
            low: wall,
            high: wall,
        };
        assert_eq!(s.cover(), Cover::Open);
    }

    #[test]
    fn the_band_is_the_outer_share_of_the_area() {
        let floors: Vec<Vec3> = (0..50)
            .flat_map(|x| (0..50).map(move |z| Vec3::new(x as f32, 0.0, z as f32)))
            .collect();
        let area = Area::new(&floors).unwrap();
        assert!(area.inside(Vec3::new(25.0, 0.0, 25.0)));
        assert!(!area.inside(Vec3::new(0.0, 0.0, 25.0)));
    }
}
