//! Row-vector affine 4x4s, as the game keeps a transform: rows 0 to 2 its
//! axes, row 3 its place, the last column 0, 0, 0, 1; `v * M`.

#![forbid(unsafe_code)]

use ragdoll::math::Vec3;

pub const IDENTITY: [f32; 16] = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];

/// `first` then `second` (the game's float4x4::multiply_affine 0xEE700).
pub fn product(first: &[f32; 16], second: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0; 16];
    for row in 0..4 {
        for column in 0..3 {
            let turned: f32 = (0..3).map(|k| first[row * 4 + k] * second[k * 4 + column]).sum();
            out[row * 4 + column] = if row == 3 { turned + second[12 + column] } else { turned };
        }
    }
    out[15] = 1.0;
    out
}

/// What undoes a transform; none for one that flattens space.
pub fn inverse(m: &[f32; 16]) -> Option<[f32; 16]> {
    let (x, y, z, place) = (row(m, 0), row(m, 1), row(m, 2), row(m, 3));
    let volume = x.dot(y.cross(z));
    if !volume.is_finite() || volume.abs() < 1e-12 {
        return None;
    }
    // the columns of the 3x3's inverse
    let (a, b, c) = (y.cross(z) * (1.0 / volume), z.cross(x) * (1.0 / volume), x.cross(y) * (1.0 / volume));
    let mut out = [0.0; 16];
    for (at, column) in [a, b, c].into_iter().enumerate() {
        (out[at], out[4 + at], out[8 + at]) = (column.x, column.y, column.z);
        out[12 + at] = -place.dot(column);
    }
    out[15] = 1.0;
    Some(out)
}

/// A transform's row (0 to 2 its axes, 3 its place).
pub fn row(m: &[f32; 16], at: usize) -> Vec3 {
    Vec3::new(m[at * 4], m[at * 4 + 1], m[at * 4 + 2])
}

/// `m` with its row `at` the given one.
pub fn with_row(mut m: [f32; 16], at: usize, v: Vec3) -> [f32; 16] {
    m[at * 4..at * 4 + 3].copy_from_slice(&[v.x, v.y, v.z]);
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32, z: f32) -> [f32; 16] {
        with_row(IDENTITY, 3, Vec3::new(x, y, z))
    }

    /// A quarter turn about y (x to -z), then a move.
    fn turned(x: f32, y: f32, z: f32) -> [f32; 16] {
        [0., 0., -1., 0., 0., 1., 0., 0., 1., 0., 0., 0., x, y, z, 1.]
    }

    fn near(a: &[f32; 16], b: &[f32; 16]) -> bool {
        a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5)
    }

    #[test]
    fn the_product_applies_the_first_first() {
        let p = product(&at(1.0, 0.0, 0.0), &turned(0.0, 0.0, 5.0));
        assert_eq!(&p[12..15], &[0.0, 0.0, 4.0]);
    }

    #[test]
    fn the_inverse_undoes() {
        let tilted = [1., 0., 0., 0., 0., 0.8, 0.6, 0., 0., -0.6, 0.8, 0., 0.1, 0.2, 0.3, 1.];
        for m in [at(1.0, -2.0, 3.0), turned(0.5, 0.25, 2.0), tilted] {
            assert!(near(&product(&m, &inverse(&m).unwrap()), &IDENTITY), "{m:?}");
            assert!(near(&product(&inverse(&m).unwrap(), &m), &IDENTITY), "{m:?}");
        }
        assert_eq!(inverse(&with_row(IDENTITY, 2, Vec3::new(0.0, 0.0, 0.0))), None);
    }
}
