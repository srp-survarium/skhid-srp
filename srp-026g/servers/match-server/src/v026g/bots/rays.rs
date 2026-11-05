//! The bots' rays on the map's bullet mesh (`mapdata::RayTree`), through one function per kind of ray, so that the bench's work counter
//! (`bench_work`, test builds) counts every ray the bots cast and a new caller counts without a line of its own: the bots' code calls
//! these and not `RayTree`'s methods. In a live build each is the method itself.

use mapdata::{Followed, Pierce, RayHit, RayTree, Vec3};

#[inline(always)]
fn counted() {
    #[cfg(test)]
    super::bench_work::add(super::bench_work::Work::Rays, 1);
}

/// Whether `b` is seen from `a` ([`RayTree::visible`]).
#[inline(always)]
pub fn visible(sight: &RayTree, a: Vec3, b: Vec3) -> bool {
    counted();
    sight.visible(a, b)
}

/// The distance to the first surface along `dir` ([`RayTree::ray_cast`]).
#[inline(always)]
pub fn ray_cast(sight: &RayTree, origin: Vec3, dir: Vec3, max_t: f32) -> Option<f32> {
    counted();
    sight.ray_cast(origin, dir, max_t)
}

/// The first surface along `dir` with its normal ([`RayTree::ray_hit`]).
#[inline(always)]
pub fn ray_hit(sight: &RayTree, origin: Vec3, dir: Vec3, max_t: f32) -> Option<RayHit> {
    counted();
    sight.ray_hit(origin, dir, max_t)
}

/// The line from `a` to `b` followed through what a bullet pierces ([`RayTree::follow`]).
#[inline(always)]
pub fn follow(sight: &RayTree, a: Vec3, b: Vec3, pierce: Pierce, floor: f32) -> Followed {
    counted();
    sight.follow(a, b, pierce, floor)
}
