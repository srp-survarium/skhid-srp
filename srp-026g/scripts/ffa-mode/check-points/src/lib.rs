//! Collision and standing-body checks shared by imported and authored FFA spawns.

use mapdata::{Body, MapData, Materials, RayTree, Resources, Vec3, WalkGrid};

pub mod cover;

/// The ground ray starts this far above the point.
const ABOVE: f32 = 1.5;
/// And looks this far below it.
const BELOW: f32 = 0.5;
/// A point this near its ground stays as it is.
const ON_GROUND: f32 = 0.05;
/// The standing body (`gameplay/players/default.player`, mapdata's
/// [`Body`]).
const RADIUS: f32 = 0.38;
const HEIGHT: f32 = 1.8;
/// Heights above the ground of the rings of rays.
const RINGS: [f32; 4] = [0.65, 1.0, 1.42, 1.65];
const DIRECTIONS: usize = 16;
/// How far from the point (on the ground plane) and from its ground (in
/// height) a floor of the walk grid may be.
const FLOOR_REACH: f32 = 0.6;
const FLOOR_HEIGHT: f32 = 0.35;

/// What the check makes of a point.
pub enum Verdict {
    /// On its ground: the donor's position as it is.
    Kept,
    /// Moved by `ground - y` onto its ground.
    Nudged {
        ground: f32,
    },
    Dropped(String),
}

/// A 0.26g project's collision: rays on the walker mesh and its walk grid.
pub struct Map {
    pub rays: RayTree,
    pub grid: WalkGrid,
    pub data: MapData,
}

impl Map {
    pub fn load(
        resources: &mut Resources,
        materials: &Materials,
        project: &str,
    ) -> Result<Self, String> {
        Self::load_in(resources, materials, project, None)
    }

    /// [`Map::load`] with the walk grid built over `bounds` (`[x0, z0, x1,
    /// z1]`) instead of the project's play area: the STALKER ports' levelmap
    /// is a dummy (scripts/stalker-maps/ffa-points).
    pub fn load_in(
        resources: &mut Resources,
        materials: &Materials,
        project: &str,
        bounds: Option<[f32; 4]>,
    ) -> Result<Self, String> {
        let map = MapData::load(resources, materials, project).map_err(|e| e.to_string())?;
        let walker = map.walker_triangles();
        let grid = WalkGrid::build_kept(
            &walker,
            bounds.unwrap_or_else(|| map.play_area(mapdata::AREA_MARGIN)),
            mapdata::CELL,
            Body::default(),
            &map.grid_seeds(),
        );
        Ok(Self {
            rays: RayTree::build(&walker),
            grid,
            data: map,
        })
    }

    pub fn check(&self, point: Vec3) -> Verdict {
        let top = point + Vec3::new(0.0, ABOVE, 0.0);
        let down = Vec3::new(0.0, -1.0, 0.0);
        let Some(t) = self.rays.ray_cast(top, down, ABOVE + BELOW) else {
            return Verdict::Dropped(match self.rays.ray_cast(point, down, 50.0) {
                Some(depth) => format!("the ground is {depth:.2} m below"),
                None => "no ground below".to_owned(),
            });
        };
        let ground = top.y - t;
        let rise = ground - point.y;
        if rise > BELOW {
            return Verdict::Dropped(format!("collision {rise:.2} m above the point"));
        }
        let feet = Vec3::new(point.x, ground, point.z);
        if let Some(why) = self.blocked(feet) {
            return Verdict::Dropped(why);
        }
        if !self.on_floor(feet) {
            return Verdict::Dropped(format!(
                "no floor reached from the map's spawns within {FLOOR_REACH} m"
            ));
        }
        if rise.abs() <= ON_GROUND {
            Verdict::Kept
        } else {
            Verdict::Nudged { ground }
        }
    }

    /// What a standing body at `feet` meets, if anything.
    pub fn blocked(&self, feet: Vec3) -> Option<String> {
        let up = Vec3::new(0.0, 1.0, 0.0);
        let base = feet + Vec3::new(0.0, 0.02, 0.0);
        if let Some(t) = self.rays.ray_cast(base, up, HEIGHT - 0.02) {
            return Some(format!("collision {:.2} m over the ground", t + 0.02));
        }
        for height in RINGS {
            let reach = ring_radius(height);
            let centre = feet + Vec3::new(0.0, height, 0.0);
            for k in 0..DIRECTIONS {
                let angle = k as f32 * std::f32::consts::TAU / DIRECTIONS as f32;
                let dir = Vec3::new(angle.cos(), 0.0, angle.sin());
                if let Some(t) = self.rays.ray_cast(centre, dir, reach) {
                    return Some(format!(
                        "collision {t:.2} m from the body's axis at {height} m"
                    ));
                }
            }
        }
        None
    }

    /// The walk grid's floors reached from the map's spawns where a body
    /// stands, inside `bounds` (`[x0, z0, x1, z1]`) when given.
    pub fn standing_floors(&self, bounds: Option<[f32; 4]>) -> Vec<u32> {
        let seeds: Vec<_> = self
            .data
            .spawns
            .iter()
            .filter_map(|s| self.grid.locate(s.position, 1.0))
            .collect();
        let reachable = self.grid.distances_from(&seeds);
        (0..self.grid.len() as u32)
            .filter(|&n| {
                let p = self.grid.position(n);
                reachable.reaches(n)
                    && self.grid.can_stand(n)
                    && bounds.is_none_or(|b| p.x >= b[0] && p.z >= b[1] && p.x <= b[2] && p.z <= b[3])
            })
            .collect()
    }

    /// Whether a standing body at `feet` (at 0.1, 0.9 and 1.7 m) comes within
    /// `margin` of a harmful zone's shape.
    pub fn harmful(&self, feet: Vec3, margin: f32) -> bool {
        self.harmful_zone(feet, margin).is_some()
    }

    /// The nearest harmful zone a standing body at `feet` comes within
    /// `margin` of: its name and distance.
    pub fn harmful_zone(&self, feet: Vec3, margin: f32) -> Option<(&str, f32)> {
        self.data
            .zones
            .iter()
            .filter(|z| z.damage.as_ref().is_some_and(|d| d.harmful()))
            .flat_map(|z| z.shapes.iter().map(move |s| (z.name.as_str(), s)))
            .flat_map(|(name, s)| {
                [0.1, 0.9, 1.7].map(|h| (name, s.distance(feet + Vec3::new(0.0, h, 0.0))))
            })
            .filter(|(_, d)| *d < margin)
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Whether a kept floor of the walk grid, where a body stands, is near.
    fn on_floor(&self, feet: Vec3) -> bool {
        let area = [
            feet.x - FLOOR_REACH,
            feet.z - FLOOR_REACH,
            feet.x + FLOOR_REACH,
            feet.z + FLOOR_REACH,
        ];
        self.grid.floors_in(area).any(|node| {
            let at = self.grid.position(node);
            at.distance_xz(feet) <= FLOOR_REACH
                && (at.y - feet.y).abs() <= FLOOR_HEIGHT
                && self.grid.can_stand(node)
        })
    }
}

/// The body's half width at `height` above its feet (the capsule's top
/// narrows).
fn ring_radius(height: f32) -> f32 {
    let over = height - (HEIGHT - RADIUS);
    if over <= 0.0 {
        RADIUS
    } else {
        (RADIUS * RADIUS - over * over).max(0.0).sqrt()
    }
}
