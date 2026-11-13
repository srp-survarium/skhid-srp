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
}
