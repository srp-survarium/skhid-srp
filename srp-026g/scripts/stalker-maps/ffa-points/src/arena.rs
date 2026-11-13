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

/// The reviewed raised islands inside the fence: a box around each flat
/// platform of the elevated walkways, with its floor's height range.
pub const ISLANDS: &str = include_str!("../../training-islands.tsv");

pub struct Island {
    pub name: &'static str,
    min: [f32; 3],
    max: [f32; 3],
}

impl Island {
    /// Whether a standing floor at (x, y, z) is this island's.
    pub fn holds(&self, x: f32, y: f32, z: f32) -> bool {
        [x, y, z]
            .iter()
            .zip(self.min.iter().zip(&self.max))
            .all(|(v, (lo, hi))| (lo..=hi).contains(&v))
    }
}

pub fn islands() -> &'static [Island] {
    static PARSED: std::sync::OnceLock<Vec<Island>> = std::sync::OnceLock::new();
    PARSED.get_or_init(|| {
        ISLANDS
            .lines()
            .skip(1)
            .map(|line| {
                let mut cells = line.split('\t');
                let name = cells.next().expect("island name");
                let v: Vec<f32> = cells.map(|c| c.parse().expect("island bound")).collect();
                let [x0, z0, x1, z1, y0, y1] = v[..] else {
                    panic!("island {name}: expected x_min z_min x_max z_max y_min y_max");
                };
                Island { name, min: [x0, y0, z0], max: [x1, y1, z1] }
            })
            .collect()
    })
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
    fn islands_are_raised_and_inside_the_fence() {
        assert_eq!(islands().len(), 5);
        for island in islands() {
            assert!(island.min.iter().zip(&island.max).all(|(lo, hi)| lo < hi), "{}", island.name);
            assert!(island.min[1] > 15.0, "{} is not raised", island.name);
            for x in [island.min[0], island.max[0]] {
                for z in [island.min[2], island.max[2]] {
                    assert!(fenced(x, z, 0.0), "{} reaches the fence", island.name);
                }
            }
        }
        // Point 8, the original south-east island spawn, is on its island.
        assert!(islands().iter().any(|i| i.holds(30.435, 21.321, -23.845)));
    }
}
