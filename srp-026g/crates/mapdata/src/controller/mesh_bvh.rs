//! [L36] The order the game's mesh tree offers a mesh's triangles to a sweep: Bullet 2.79's quantized `btOptimizedBvh`, built
//! the exe's way (exactness `grid/rig/s7-20261114/L36-tie-order.md` §2; the reference port `bvh_port.py`, checked against the
//! exe's own build 0xA8A40 in Unicorn on 317 of 321 Vector Laboratory and 75 of 75 Chemical Plant meshes, the 4 that differed
//! explained by the variance loop's hoisted reciprocal, which this port takes too).
//!
//! - `create_btBvhTriangleMeshShape` 0x7A6E0 -> `buildOptimizedBvh` 0xA8A40 -> `btOptimizedBvh::build` 0xC06E0, quantized:
//!   `setQuantizationValues` 0xC1810 (the shape's local box +/- 1.0, 65533 / size), one leaf per triangle in buffer order
//!   (its scaled corners' box, widened by 0.001 each side on an axis thinner than 0.002, quantized: min `(u16)v & 0xFFFE`,
//!   max `(u16)(v + 1) | 1`), then `buildTree` 0xC1A60: `calcSplittingAxis` 0xC22F0 (the variance of the unquantized
//!   centres; its variance loop unquantizes by a hoisted reciprocal, 0xC24EE..0xC2580) and `sortAndCalcSplittingIndex`
//!   0xC1F60 (centres over the mean go first, swapped in place; the n/3 balance rule falls back to the middle).
//! - The walk (`walkStacklessQuantizedTreeAgainstRay` 0xC2FF0) visits the contiguous node array in order, so it offers the
//!   triangles whose leaf box meets the cast in the leaf order: the leaf array after `buildTree`'s in-place partitions. Only
//!   that order is needed here, not the internal nodes' boxes.
//!
//! Every operation is `f32` in the order the exe does it (SSE scalar: no x87 precision).

type V3 = [f32; 3];

/// `setQuantizationValues`' margin on the shape's local box, m.
const QUANT_MARGIN: f32 = 1.0;
/// The quantized range (`btQuantizedBvh`: 0xFFFD).
const QUANT_RANGE: f32 = 65533.0;
/// A leaf box thinner than this on an axis is widened by [`MIN_HALF`] each side (`MIN_AABB_DIMENSION`, 0.002).
const MIN_SIZE: f32 = 0.002;
const MIN_HALF: f32 = 0.001;

/// `cvttss2si`: truncation to an `i32`, NaN or out of range the integer indefinite (`i32::MIN`).
fn cvtt(v: f32) -> i32 {
    if v.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&v) { i32::MIN } else { v as i32 }
}

struct Quant {
    min: V3,
    quant: V3,
}

impl Quant {
    fn new(lo: V3, hi: V3) -> Self {
        let min = lo.map(|a| a - QUANT_MARGIN);
        let max = hi.map(|a| a + QUANT_MARGIN);
        Self { min, quant: [0, 1, 2].map(|k| QUANT_RANGE / (max[k] - min[k])) }
    }

    fn quantize(&self, p: V3, is_max: bool) -> [u16; 3] {
        [0, 1, 2].map(|k| {
            let v = (p[k] - self.min[k]) * self.quant[k];
            if is_max { ((cvtt(v + 1.0) as u32 & 0xFFFF) | 1) as u16 } else { (cvtt(v) as u32 & 0xFFFE) as u16 }
        })
    }

    /// A leaf's centre as the mean loop and the split unquantize (`q / quant + min`, 0xC238B, 0xC1FF6).
    fn centre(&self, lo: [u16; 3], hi: [u16; 3]) -> V3 {
        [0, 1, 2].map(|k| {
            let l = f32::from(lo[k]) / self.quant[k] + self.min[k];
            let h = f32::from(hi[k]) / self.quant[k] + self.min[k];
            (h + l) * 0.5
        })
    }

    /// A leaf's centre as the variance loop unquantizes (`min + q * (1 / quant)`, 0xC24EE..0xC2580).
    fn centre_by_reciprocal(&self, lo: [u16; 3], hi: [u16; 3]) -> V3 {
        [0, 1, 2].map(|k| {
            let r = 1.0 / self.quant[k];
            let l = self.min[k] + f32::from(lo[k]) * r;
            let h = self.min[k] + f32::from(hi[k]) * r;
            (h + l) * 0.5
        })
    }
}

/// A leaf: its triangle and its two centres.
#[derive(Clone, Copy)]
struct Leaf {
    triangle: u32,
    centre: V3,
    centre_by_reciprocal: V3,
}

fn mean(leaves: &[Leaf]) -> V3 {
    let mut sum = [0.0f32; 3];
    for l in leaves {
        sum = [0, 1, 2].map(|k| sum[k] + l.centre[k]);
    }
    let inv = 1.0 / leaves.len() as f32;
    sum.map(|s| s * inv)
}

/// `calcSplittingAxis` 0xC22F0.
fn split_axis(leaves: &[Leaf]) -> usize {
    let means = mean(leaves);
    let mut var = [0.0f32; 3];
    for l in leaves {
        let d = [0, 1, 2].map(|k| l.centre_by_reciprocal[k] - means[k]);
        var = [0, 1, 2].map(|k| var[k] + d[k] * d[k]);
    }
    let inv = 1.0 / (leaves.len() as f32 - 1.0);
    let var = var.map(|v| v * inv);
    if var[0] < var[1] {
        if var[1] < var[2] { 2 } else { 1 }
    } else if var[0] < var[2] {
        2
    } else {
        0
    }
}

/// `sortAndCalcSplittingIndex` 0xC1F60: partitions `leaves` in place, returns the split (an index into it).
fn split_index(leaves: &mut [Leaf], axis: usize) -> usize {
    let n = leaves.len();
    let split_value = mean(leaves)[axis];
    let mut split = 0;
    for i in 0..n {
        if leaves[i].centre[axis] > split_value {
            leaves.swap(i, split);
            split += 1;
        }
    }
    let third = n / 3;
    if split <= third || split >= n - 1 - third { n >> 1 } else { split }
}

/// The shape's local box (+10h, +20h): the scaled corners' extremes, the mesh's margin 0 (exactness: equal to the captured bits
/// on every mesh checked). None with no triangle or a corner that is not a finite number.
pub(super) fn local_box(corners: &[[V3; 3]]) -> Option<(V3, V3)> {
    if corners.is_empty() || corners.iter().flatten().flatten().any(|c| !c.is_finite()) {
        return None;
    }
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for v in corners.iter().flatten() {
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    Some((lo, hi))
}

/// The place of each triangle of a mesh in the order its tree offers them: `rank[t]` for the triangle `t` of the buffer
/// (`corners`: each triangle's corners scaled, in the mesh's own frame, in buffer order). None when a corner is not a finite
/// number (the port does not follow the exe's arithmetic on those).
pub(super) fn offering_ranks(corners: &[[V3; 3]]) -> Option<Vec<u32>> {
    let (lo, hi) = local_box(corners)?;
    let q = Quant::new(lo, hi);
    let mut leaves: Vec<Leaf> = corners
        .iter()
        .enumerate()
        .map(|(t, v)| {
            let mut lo = [0, 1, 2].map(|k| v[0][k].min(v[1][k]).min(v[2][k]));
            let mut hi = [0, 1, 2].map(|k| v[0][k].max(v[1][k]).max(v[2][k]));
            for k in 0..3 {
                if hi[k] - lo[k] < MIN_SIZE {
                    hi[k] += MIN_HALF;
                    lo[k] -= MIN_HALF;
                }
            }
            let (ql, qh) = (q.quantize(lo, false), q.quantize(hi, true));
            Leaf { triangle: t as u32, centre: q.centre(ql, qh), centre_by_reciprocal: q.centre_by_reciprocal(ql, qh) }
        })
        .collect();
    // `buildTree` 0xC1A60: each range split in place; the ranges are disjoint, so the order they are split in does not
    // change the leaf array it leaves.
    let mut ranges = vec![(0usize, leaves.len())];
    while let Some((start, end)) = ranges.pop() {
        if end - start < 2 {
            continue;
        }
        let axis = split_axis(&leaves[start..end]);
        let split = start + split_index(&mut leaves[start..end], axis);
        ranges.push((start, split));
        ranges.push((split, end));
    }
    let mut rank = vec![0u32; leaves.len()];
    for (place, leaf) in leaves.iter().enumerate() {
        rank[leaf.triangle as usize] = place as u32;
    }
    Some(rank)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip of triangles along x, in buffer order 0..n, all on one plane: every split is along x, the centres over the mean
    /// go first, so the offering order is not the buffer order.
    #[test]
    fn a_strip_is_offered_in_the_trees_order() {
        let tri = |x: f32| [[x, 0.0, 0.0], [x + 1.0, 0.0, 0.0], [x, 0.0, 1.0]];
        let corners: Vec<_> = (0..8).map(|i| tri(i as f32)).collect();
        let rank = offering_ranks(&corners).expect("finite");
        let mut order = vec![0u32; rank.len()];
        for (t, &r) in rank.iter().enumerate() {
            order[r as usize] = t as u32;
        }
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..8).collect::<Vec<_>>(), "a permutation");
        assert_ne!(order, (0..8).collect::<Vec<_>>(), "the tree's order is not the buffer's");
    }

    #[test]
    fn a_corner_that_is_no_number_gives_no_order() {
        assert!(offering_ranks(&[[[0.0, 0.0, 0.0], [f32::NAN, 0.0, 0.0], [0.0, 0.0, 1.0]]]).is_none());
    }
}
