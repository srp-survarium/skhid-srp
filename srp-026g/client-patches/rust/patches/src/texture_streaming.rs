//! Patch 137: texture streaming's per-frame bookkeeping in Rust (client_patch.md 3.79), the
//! same records and the same answer as the game's, with less of the render thread's frame.
//!
//! - scene::update_streaming_data (0x155980): each frame it rewrites the record of every object
//!   drawn with a streamable texture from the object's proxy, through four virtual calls each
//!   (bound_sphere, last_render_time, texel_factor, and the surface's mark cleared), the
//!   texel factor with three x87 square roots. Here the proxy's class is read once from its
//!   vtable (vostok::streaming) and [`record`] is the three classes' methods, one square root
//!   for a surface.
//! - resource_manager::get_texture_video_memory_size (0x3BE280): each frame, for the renderer's
//!   statistics, the video memory of every registered texture whose name lacks "$user$",
//!   found with the CRT's byte-at-a-time strstr. Here the same sum, the names searched 16
//!   bytes at a time.
//!
//! Every value written is the native's, bit for bit, NaNs included: the x87's rules are
//! [`x87`]'s. check_texture_streaming_026g.py runs both in the original and the build.
#![forbid(unsafe_code)]

use crate::vostok::streaming::{self, Aabb, Needle, Proxy, Record, Surface};
use crate::vostok::{Context, Flow, TextureStreamingOnTextureMemoryExit, TextureStreamingOnUpdateExit};

/// update_streaming_data's start: the whole function.
pub fn on_update(ctx: &mut Context<'_>) -> Flow<TextureStreamingOnUpdateExit> {
    streaming::update_streaming_data(ctx, record);
    Flow::Exit(TextureStreamingOnUpdateExit::Returned)
}

/// The names get_texture_video_memory_size leaves out (the image's "$user$", 0xA25DA4): the
/// render targets and textures the game makes itself.
const USER: Needle = Needle::new(b"$user$");

/// get_texture_video_memory_size's start: the whole function.
pub fn on_texture_memory(ctx: &mut Context<'_>) -> Flow<TextureStreamingOnTextureMemoryExit> {
    let bytes = streaming::texture_registry(ctx)
        .filter(|texture| !texture.name_contains(&USER))
        .fold(0u32, |sum, texture| sum.wrapping_add(texture.memory()));
    streaming::texture_memory_result(ctx, bytes);
    Flow::Exit(TextureStreamingOnTextureMemoryExit::Returned)
}

/// A grass patch's bound sphere radius (the image's 3000.0 at 0xA2D8A0, read-only).
const GRASS_RADIUS: f32 = 3000.0;

/// What a record keeps of its proxy: its class's bound_sphere, last_render_time and
/// texel_factor as the record's f32 receive them (an x87 result through `fstp`).
pub fn record(proxy: Proxy) -> Record {
    match proxy {
        // bound_sphere 0x1C8CD0 (a copy), last_render_time 0x1C8B80 (fld), texel_factor 0x1C8B90.
        Proxy::RenderSurface(surface) => Record {
            sphere: surface.sphere,
            last_render_time: x87::load(surface.last_render_time),
            texel_factor: surface_texel_factor(&surface),
        },
        // bound_sphere 0x1C8D40, last_render_time 0x1C8D20 (fldz), texel_factor 0x1C8D30 (fld1).
        Proxy::GrassPatch => Record { sphere: [0.0, 0.0, 0.0, GRASS_RADIUS], last_render_time: 0.0, texel_factor: 1.0 },
        // bound_sphere 0x1C8E00, last_render_time 0x1C8D20 (fldz), texel_factor 0x1C8D80.
        Proxy::Decal(aabb) => Record { sphere: decal_sphere(&aabb), last_render_time: 0.0, texel_factor: decal_texel_factor(&aabb) },
    }
}

/// render_surface_texture_instance_proxy::texel_factor: the surface's factor times its
/// matrix's largest axis scale. The native takes the three rows' lengths (x87 fsqrt each, a
/// float's square root correctly rounded: sqrtss's) and the larger of the second and third,
/// then of the first and that; the square root is monotonic, so the same choice made on the
/// squares and one root of it is the same float, a NaN where the native's is. Each square is
/// summed in the native's SSE order.
fn surface_texel_factor(surface: &Surface) -> f32 {
    let [r0, r1, r2] = surface.rows;
    let a = (r0[1] * r0[1] + r0[2] * r0[2]) + r0[0] * r0[0];
    let b = (r1[0] * r1[0] + r1[1] * r1[1]) + r1[2] * r1[2];
    let c = (r2[1] * r2[1] + r2[2] * r2[2]) + r2[0] * r2[0];
    let larger = if b > c { b } else { c };
    let largest = if a > larger { a } else { larger };
    x87::mul(x87::load(surface.factor), core::f32::math::sqrt(largest))
}

/// decal_texture_instance_proxy::bound_sphere: the box's centre and the distance from it to
/// the box's min (SSE, the native's order; its x87 root a float's).
fn decal_sphere(aabb: &Aabb) -> [f32; 4] {
    let centre = [0, 1, 2].map(|i| (aabb.max[i] + aabb.min[i]) * 0.5);
    let d = [0, 1, 2].map(|i| centre[i] - aabb.min[i]);
    let radius = core::f32::math::sqrt((d[2] * d[2] + d[1] * d[1]) + d[0] * d[0]);
    [centre[0], centre[1], centre[2], radius]
}

/// decal_texture_instance_proxy::texel_factor: half the larger of the box's half x and half z
/// extents (the last half an x87 `fmul` by the image's 0.5, exact as SSE's).
fn decal_texel_factor(aabb: &Aabb) -> f32 {
    let x = (aabb.max[0] - aabb.min[0]) * 0.5;
    let z = (aabb.max[2] - aabb.min[2]) * 0.5;
    (if x > z { x } else { z }) * 0.5
}

/// The x87's answers where they differ from SSE's, for a float result stored with `fstp`
/// (the control word's precision 53 or 64 bits, the MSVC runtime's and Direct3D 11's: a
/// product of two floats is exact there, so its one rounding is the store's, mulss's).
mod x87 {
    const QUIET: u32 = 1 << 22;
    const SIGN: u32 = 1 << 31;
    const SIGNIFICAND: u32 = (1 << 23) - 1;

    /// A float through `fld` and `fstp`: the same, but a signalling NaN comes out quiet.
    pub fn load(x: f32) -> f32 {
        if x.is_nan() { f32::from_bits(x.to_bits() | QUIET) } else { x }
    }

    /// `fmul` of two floats: SSE's product, but of two NaNs the one with the larger
    /// significand, the positive one of two equal (Intel SDM vol. 1 table 4-7; SSE takes the
    /// first). Both are quiet here: `a` came through `fld`, `b` is a computed result.
    pub fn mul(a: f32, b: f32) -> f32 {
        if a.is_nan() && b.is_nan() {
            let (qa, qb) = (a.to_bits() | QUIET, b.to_bits() | QUIET);
            let pick = match (qa & SIGNIFICAND).cmp(&(qb & SIGNIFICAND)) {
                core::cmp::Ordering::Greater => qa,
                core::cmp::Ordering::Less => qb,
                core::cmp::Ordering::Equal => if qa & SIGN == 0 { qa } else { qb },
            };
            return f32::from_bits(pick);
        }
        a * b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(rows: [[f32; 3]; 3], factor: f32) -> Surface {
        Surface { sphere: [1.0, 2.0, 3.0, 4.0], last_render_time: 5.0, rows, factor }
    }

    #[test]
    fn a_surface_scaled() {
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert_eq!(surface_texel_factor(&surface(identity, 7.5)), 7.5);
        let stretched = [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.5]];
        assert_eq!(surface_texel_factor(&surface(stretched, 2.0)), 6.0);
        // the three roots' largest, as the native's three roots
        let rows = [[0.3, 0.4, 1.2], [1.1, 0.2, 0.7], [0.9, 0.9, 0.9]];
        let roots = rows.map(|r: [f32; 3]| ((r[1] * r[1] + r[2] * r[2]) + r[0] * r[0]).sqrt());
        assert_eq!(surface_texel_factor(&surface(rows, 1.0)), roots[0].max(roots[1]).max(roots[2]));
    }

    #[test]
    fn a_grass_patch_and_a_decal() {
        let grass = record(Proxy::GrassPatch);
        assert_eq!((grass.sphere, grass.last_render_time, grass.texel_factor), ([0.0, 0.0, 0.0, 3000.0], 0.0, 1.0));
        let decal = record(Proxy::Decal(Aabb { min: [1.0, 2.0, 3.0], max: [5.0, 4.0, 11.0] }));
        assert_eq!(decal.sphere[..3], [3.0, 3.0, 7.0]);
        assert_eq!(decal.sphere[3], (16.0f32 + 1.0 + 4.0).sqrt());
        assert_eq!(decal.texel_factor, 2.0);
    }

    #[test]
    fn the_x87s_nans() {
        let signalling = f32::from_bits(0x7F80_0001);
        assert_eq!(x87::load(signalling).to_bits(), 0x7FC0_0001);
        assert_eq!(x87::load(1.5).to_bits(), 1.5f32.to_bits());
        let (small, large) = (f32::from_bits(0xFFC0_0001), f32::from_bits(0x7FC0_0002));
        assert_eq!(x87::mul(small, large).to_bits(), 0x7FC0_0002);
        assert_eq!(x87::mul(large, small).to_bits(), 0x7FC0_0002);
        let (negative, positive) = (f32::from_bits(0xFFC0_0005), f32::from_bits(0x7FC0_0005));
        assert_eq!(x87::mul(negative, positive).to_bits(), 0x7FC0_0005);
        assert_eq!(x87::mul(2.0, 3.0), 6.0);
    }
}
