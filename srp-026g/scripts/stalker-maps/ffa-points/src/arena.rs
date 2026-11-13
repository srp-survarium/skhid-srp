//! Training Camp's crater rim, shared with the content collision generator.
//! The forest outside it is scenery, not part of the FFA arena.
pub const OUTLINE: &str = include_str!("../../training-arena.tsv");

pub fn vertices() -> &'static [[f32; 2]] {
    static VERTICES: std::sync::OnceLock<Vec<[f32; 2]>> = std::sync::OnceLock::new();
    VERTICES.get_or_init(|| OUTLINE.lines().skip(1).map(|line| {
        let (x, z) = line.split_once('\t').expect("arena vertex");
        [x.parse().expect("arena x"), z.parse().expect("arena z")]
    }).collect())
}

/// The reviewed raised islands inside the crater: a box around each flat
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

/// The point must be inside the rim and clear of its walker barrier.
pub fn contains(x: f32, z: f32, margin: f32) -> bool {
    let vertices = vertices();
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
    fn crater_is_playable_but_the_forest_and_rim_are_not() {
        assert!(contains(20.0, 0.0, 2.0));
        for p in [[-45.0, 0.0], [20.0, 80.0], [20.0, -80.0], [80.0, 0.0]] {
            assert!(!contains(p[0], p[1], 2.0));
        }
        for &[x, z] in vertices() {
            assert!(!contains(x, z, 2.0));
        }
    }

    #[test]
    fn islands_are_raised_and_inside_the_crater() {
        assert_eq!(islands().len(), 5);
        for island in islands() {
            assert!(island.min.iter().zip(&island.max).all(|(lo, hi)| lo < hi), "{}", island.name);
            assert!(island.min[1] > 15.0, "{} is not raised", island.name);
            for x in [island.min[0], island.max[0]] {
                for z in [island.min[2], island.max[2]] {
                    assert!(contains(x, z, 2.0), "{} reaches the rim", island.name);
                }
            }
        }
        // Point 8, the original south-east island spawn, is on its island.
        assert!(islands().iter().any(|i| i.holds(30.435, 21.321, -23.845)));
    }
}
