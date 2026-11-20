//! Training Camp's Free for All ground: inside its base's fence, with its
//! raised islands. The meadow and the crater round the base are scenery.

/// The fence round the base: its posts in order, read off the level's
/// collision, gateways post to post. At the south gateway the line runs
/// along the open west leaf, across the nose of the truck the level has
/// there and through the truck beside it. The walker wall stands just behind
/// the line (`make_training_arena.py`, which shares the file).
pub const FENCE: &str = include_str!("../../training-fence.tsv");

fn fence() -> &'static [[f32; 2]] {
    static POSTS: std::sync::OnceLock<Vec<[f32; 2]>> = std::sync::OnceLock::new();
    POSTS.get_or_init(|| {
        FENCE
            .lines()
            .skip(1)
            .map(|line| {
                let (x, z) = line.split_once('\t').expect("fence post");
                [x.parse().expect("post x"), z.parse().expect("post z")]
            })
            .collect()
    })
}

/// Inside the fence and at least `margin` from it.
pub fn fenced(x: f32, z: f32, margin: f32) -> bool {
    within(fence(), x, z, margin)
}

/// The reviewed raised islands inside the fence: a box around each floating
/// island's top, with its floor's height range.
pub const ISLANDS: &str = include_str!("../../training-islands.tsv");

/// The base's three towers: round floors, each a circle with a height
/// range. A tower's `stairs` are the walkway that winds round it, the
/// stairs inside it and their landings, below its roof; a `cabin` is one of
/// the two huts on a tower (the lower by its walkway, the upper at its roof).
pub const TOWERS: &str = include_str!("../../training-towers.tsv");

/// A raised floor a Free for All spawn may stand on, by its name in the
/// tables (the owner, 2026-11-14: "on islands or on ladders or in rubkas").
/// The towers' roofs, the catwalks and the platform over the hub are none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Raised {
    Island(&'static str),
    /// A tower's stairs: `training-towers.tsv`'s "ladders".
    Stairs(&'static str),
    Cabin(&'static str),
}

impl std::fmt::Display for Raised {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Island(name) => write!(f, "raised island {name}"),
            Self::Stairs(name) => write!(f, "stairs of tower {name}"),
            Self::Cabin(name) => write!(f, "cabin {name}"),
        }
    }
}

struct Island {
    name: &'static str,
    min: [f32; 3],
    max: [f32; 3],
}

struct Round {
    place: Raised,
    centre: [f32; 2],
    radius: f32,
    y: [f32; 2],
}

fn numbers<const N: usize>(what: &str, cells: std::str::Split<'static, char>) -> [f32; N] {
    let v: Vec<f32> = cells.map(|c| c.parse().unwrap_or_else(|_| panic!("{what}: {c}"))).collect();
    v.try_into().unwrap_or_else(|_| panic!("{what}: {N} numbers expected"))
}

fn islands() -> &'static [Island] {
    static PARSED: std::sync::OnceLock<Vec<Island>> = std::sync::OnceLock::new();
    PARSED.get_or_init(|| {
        ISLANDS
            .lines()
            .skip(1)
            .map(|line| {
                let mut cells = line.split('\t');
                let name = cells.next().expect("island name");
                let [x0, z0, x1, z1, y0, y1] = numbers(name, cells);
                Island { name, min: [x0, y0, z0], max: [x1, y1, z1] }
            })
            .collect()
    })
}

fn towers() -> &'static [Round] {
    static PARSED: std::sync::OnceLock<Vec<Round>> = std::sync::OnceLock::new();
    PARSED.get_or_init(|| {
        TOWERS
            .lines()
            .skip(1)
            .map(|line| {
                let mut cells = line.split('\t');
                let name = cells.next().expect("tower place");
                let place = match cells.next() {
                    Some("stairs") => Raised::Stairs(name),
                    Some("cabin") => Raised::Cabin(name),
                    kind => panic!("{name}: stairs or cabin expected, not {kind:?}"),
                };
                let [x, z, radius, y0, y1] = numbers(name, cells);
                Round { place, centre: [x, z], radius, y: [y0, y1] }
            })
            .collect()
    })
}

/// The raised floor a standing floor at (x, y, z) is part of, if it is one a
/// spawn may stand on: a cabin, else an island, else a tower's stairs.
pub fn raised_at(x: f32, y: f32, z: f32) -> Option<Raised> {
    let round = |cabin: bool| {
        towers().iter().find(|r| {
            matches!(r.place, Raised::Cabin(_)) == cabin
                && (r.y[0]..=r.y[1]).contains(&y)
                && (x - r.centre[0]).hypot(z - r.centre[1]) <= r.radius
        })
    };
    let island = || {
        islands().iter().find(|i| {
            [x, y, z].iter().zip(i.min.iter().zip(&i.max)).all(|(v, (lo, hi))| (lo..=hi).contains(&v))
        })
    };
    round(true)
        .map(|r| r.place)
        .or_else(|| island().map(|i| Raised::Island(i.name)))
        .or_else(|| round(false).map(|r| r.place))
}

/// Inside the outline and at least `margin` from its edges.
fn within(vertices: &[[f32; 2]], x: f32, z: f32, margin: f32) -> bool {
    let mut inside = false;
    for (a, b) in vertices.iter().zip(vertices.iter().cycle().skip(1)).take(vertices.len()) {
        let dx = b[0] - a[0];
        let dz = b[1] - a[1];
        let t = (((x - a[0]) * dx + (z - a[1]) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0);
        if (x - a[0] - t * dx).powi(2) + (z - a[1] - t * dz).powi(2) <= margin * margin {
            return false;
        }
        if (a[1] > z) != (b[1] > z) && x < a[0] + (z - a[1]) * dx / dz {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_base_is_fenced_and_the_crater_round_it_is_not() {
        assert!(fenced(20.0, 0.0, 2.0));
        // The crater's floor outside the fence, where points stood before, and the forest.
        for p in [[-24.0, 7.0], [9.2, 57.2], [24.2, -44.6], [60.2, -32.6], [20.0, 80.0]] {
            assert!(!fenced(p[0], p[1], 0.0), "{p:?}");
        }
        // The east side, which the crater's wall cut off, is the fence's.
        assert!(fenced(57.2, 6.2, 2.0) && !fenced(66.0, 6.2, 0.0));
        // A post is on the fence, and a place beside it is inside only without a margin.
        for &[x, z] in fence() {
            assert!(!fenced(x, z, 0.0));
        }
        assert!(fenced(-8.0, 0.0, 0.0) && !fenced(-8.0, 0.0, 2.0));
    }

    #[test]
    fn a_gateway_is_fence_between_its_posts() {
        // The north and north-east gateways' middles: the outline runs post
        // to post, so a step out of one is outside. The south one's runs
        // across its trucks' noses, inside the posts' line.
        for (inside, outside) in [
            ([9.2, 36.5], [9.2, 37.8]),
            ([47.2, 31.1], [48.2, 32.1]),
            ([27.5, -26.0], [27.0, -29.0]),
            ([22.5, -25.5], [22.5, -28.0]),
        ] {
            assert!(fenced(inside[0], inside[1], 0.0) && !fenced(outside[0], outside[1], 0.0));
        }
    }

    #[test]
    fn raised_floors_are_raised_and_inside_the_fence() {
        assert_eq!(islands().len(), 8);
        for island in islands() {
            assert!(island.min.iter().zip(&island.max).all(|(lo, hi)| lo < hi), "{}", island.name);
            assert!(island.min[1] > 15.0, "{} is not raised", island.name);
            for x in [island.min[0], island.max[0]] {
                for z in [island.min[2], island.max[2]] {
                    assert!(fenced(x, z, 0.0), "{} reaches the fence", island.name);
                }
            }
        }
        // Three towers, each with its stairs and two cabins.
        let count = |cabin: bool| towers().iter().filter(|r| matches!(r.place, Raised::Cabin(_)) == cabin).count();
        assert_eq!((count(false), count(true)), (3, 6));
        for round in towers() {
            assert!(round.y[0] >= 6.0 && round.y[0] < round.y[1], "{:?} is not raised", round.place);
            for (dx, dz) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                let (x, z) = (round.centre[0] + dx * round.radius, round.centre[1] + dz * round.radius);
                assert!(fenced(x, z, 0.0), "{:?} reaches the fence", round.place);
            }
        }
    }

    #[test]
    fn a_place_is_a_cabin_then_an_island_then_stairs() {
        // Point 8, the original south-east island spawn, and point 19 on the east tower's walkway.
        assert_eq!(raised_at(30.435, 21.321, -23.845), Some(Raised::Island("south_east")));
        assert_eq!(raised_at(38.685, 16.22, 6.155), Some(Raised::Stairs("east")));
        // The lower cabin of the north-west tower stands inside its stairs' circle.
        assert_eq!(raised_at(4.1, 15.55, 15.3), Some(Raised::Cabin("north_west_lower")));
        // The ground, a tower's roof, the catwalk from it and the platform over the hub are none.
        for p in [[20.0, -0.8, 0.0], [9.4, 28.5, 23.9], [15.0, 28.7, 13.0], [22.0, 29.0, 4.0]] {
            assert_eq!(raised_at(p[0], p[1], p[2]), None, "{p:?}");
        }
    }
}
