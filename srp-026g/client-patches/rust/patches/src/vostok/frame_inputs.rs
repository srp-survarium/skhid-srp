//! What DLSS is given of a frame, the engine's side, for patch 83
//! (docs/notes/dlss-option.md, "The frame's inputs").
//!
//! - The jitter's seam ([`JitterSeam`]): renderer::render pushes the frame's
//!   jittered projection on the context's projection stack before the
//!   visibility stage and pops it after the stages (its own temporal AA's
//!   +-0.25 pixel, temporal_projection_matrix_modifier 0x1BE4A0). The
//!   first-person projection (foreground_begin 0x13E010) is made from the
//!   context's current one, and the sun's shadows put the projection before
//!   the jitter back (stage_shadow_direct::execute 0x172EA3, the renderer's
//!   +20h), so a projection set here is the whole frame's and nothing
//!   else's. The context's eye rays (what the deferred passes rebuild a
//!   position from depth with) are mirrored from one corner by the engine;
//!   [`EyeRays`] is where each corner gets its own.
//! - The game's own AA ([`no_aa`]), the object motion pass whatever its
//!   options say ([`MotionGate`]; its other gate and its target's clear are
//!   their sites' exits alone), and that target's format ([`MotionFormat`]).
//! - The frame's targets as the link takes them ([`Targets`], [`Target`]),
//!   and a target read back for a diagnostic ([`read_back`]).

use core::marker::PhantomData;
use core::ptr::{null, read, write, NonNull};

use super::baker::Texture;
use super::context::{Context, Reg};
use super::hook::Thread;
use super::render_size::Size;
use crate::raw;

/// The lit view mode (renderer::render's cmp [esp+0Ch],3 at 0x13D391).
const LIT: u32 = 3;
/// create_texture2d's usage of a staging texture (D3D11_USAGE_STAGING).
const STAGING: u32 = 3;

/// A matrix of the engine's (float4x4): 16 f32 by rows, for row vectors
/// (clip = view position * projection).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix(pub [f32; 16]);

impl Matrix {
    /// # Safety
    /// `first` is the first of 16 f32 alive for the read.
    unsafe fn read(first: *const f32) -> Matrix {
        // SAFETY: the caller's: 16 f32 there.
        Matrix(unsafe { read(first.cast::<[f32; 16]>()) })
    }
}

/// Which scene and scene view a frame renders: identities, never read
/// through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneId {
    scene: u32,
    view: u32,
}

impl SceneId {
    /// A scene by a number, for a test.
    #[cfg(test)]
    pub fn test(scene: u32) -> SceneId {
        SceneId { scene, view: 0 }
    }
}

/// `render.jittered`: renderer::render with the frame's projection set, at
/// the place the engine's own jitter is pushed.
pub struct JitterSeam {
    frame: raw::RenderFrame,
    context: raw::RendererContext,
}

impl JitterSeam {
    pub fn at(ctx: &Context<'_>) -> Option<JitterSeam> {
        // SAFETY: the context is this site's: its ESP is renderer::render's
        // frame, whose jitterer names the renderer's context (0x13D3CC).
        let frame = unsafe { raw::site::render_jittered::frame(ctx) }?;
        // SAFETY: as above.
        let context = unsafe { frame.jitterer().context() }?;
        Some(JitterSeam { frame, context })
    }

    /// Whether the frame is one the engine jitters for its own temporal AA:
    /// the lit view mode, the scene view's post-process on (0x13D391,
    /// 0x13D3A4). Any other frame has no post-process to take a jitter out.
    pub fn post_processed(&self) -> bool {
        // SAFETY: the site's frame and the context's scene view, alive while
        // render runs.
        unsafe {
            read(self.frame.view_mode()) == LIT
                && self.context.scene_view().is_some_and(|view| read(view.use_post_process()) != 0)
        }
    }

    /// The frame's view matrix.
    pub fn view(&self) -> Matrix {
        // SAFETY: the context's m_v, 16 f32.
        unsafe { Matrix::read(self.context.view()) }
    }

    /// The frame's projection before any jitter (push_jittering's answer).
    pub fn projection(&self) -> Matrix {
        // SAFETY: render's [esp+0B0h], 16 f32 push_jittering just filled.
        unsafe { Matrix::read(self.frame.unjittered_projection()) }
    }

    pub fn scene(&self) -> SceneId {
        // SAFETY: the context's scene and scene view, read as identities.
        unsafe {
            SceneId { scene: read(self.context.scene()), view: self.context.scene_view().map_or(0, |view| view.address()) }
        }
    }

    /// Whether the engine pushed its own jitter (the temporal AA's).
    pub fn engine_jittered(&self) -> bool {
        // SAFETY: the frame's jitterer, its m_jittered.
        unsafe { read(self.frame.jitterer().jittered()) != 0 }
    }

    /// The frame's targets.
    pub fn targets(&self) -> Targets {
        Targets { context: self.context, _local: PhantomData }
    }

    /// The frame is rendered with `projection` (the visibility stage, the
    /// stages, the first-person models; the sun's shadows keep the one
    /// before); render puts the projection before the jitter back after the
    /// stages (0x13D47B).
    pub fn project(self, projection: &Matrix) {
        // SAFETY: on the render thread, where render itself would push
        // (push_set_p 0x1392C0: the old projection on the context's stack,
        // the new one set, as push_jittering 0x1BE649..0x1BE661) or, its own
        // pushed, where set_p (0x14A6F0) replaces it. With need_modify set
        // render pops once (0x13D47B..0x13D495), as for its own.
        unsafe {
            let jitterer = self.frame.jitterer();
            if read(jitterer.jittered()) != 0 {
                raw::engine_context_set_p(projection.0.as_ptr(), Some(self.context));
            } else {
                raw::engine_context_push_set_p(Some(self.context), projection.0.as_ptr());
                write(jitterer.need_modify(), 1);
                write(jitterer.jittered(), 1);
            }
        }
    }
}

/// `context.eye_rays`: update_eye_rays about to return, the context's eye
/// rays written (all but the last word).
pub struct EyeRays {
    context: raw::RendererContext,
}

impl EyeRays {
    pub fn at(ctx: &Context<'_>) -> Option<EyeRays> {
        // SAFETY: the context is this site's: its EAX the renderer context.
        let context = unsafe { raw::site::context_eye_rays::context(ctx) }?;
        Some(EyeRays { context })
    }

    /// The inverse of the projection the rays are of.
    pub fn projection_inverted(&self) -> Matrix {
        // SAFETY: the context's m_p_inverted, 16 f32 set_p just wrote.
        unsafe { Matrix::read(self.context.projection_inverted()) }
    }

    /// The inverse of the context's view.
    pub fn view_inverted(&self) -> Matrix {
        // SAFETY: the context's m_v_inverted, 16 f32.
        unsafe { Matrix::read(self.context.view_inverted()) }
    }

    /// The context's eye rays are `rays`: the view's four, then the world's
    /// four. The caller skips the site's instruction (the last word's own
    /// store).
    pub fn set(self, rays: &[[f32; 3]; 8]) {
        // SAFETY: m_eye_rays, 24 f32 the render thread alone writes, here in
        // update_eye_rays itself.
        unsafe { write(self.context.eye_rays().cast::<[[f32; 3]; 8]>(), *rays) }
    }
}

/// At `postprocess.aa_method`: the post-process runs none of the game's AA
/// (r_antialiasing_method read as 0: no FXAA and its sharpen, no temporal
/// AA), whatever the option says.
pub fn no_aa(ctx: &mut Context<'_>) {
    ctx.set_reg(Reg::Eax, 0);
}

/// `postprocess.motion_gate`: the post-process about to skip the object
/// motion pass (no motion blur; the temporal AA's motion vectors off).
pub struct MotionGate {
    frame: raw::PostprocessFrame,
}

impl MotionGate {
    pub fn at(ctx: &Context<'_>) -> Option<MotionGate> {
        // SAFETY: the context is this site's: its ESP is execute's frame.
        let frame = unsafe { raw::site::postprocess_motion_gate::frame(ctx) }?;
        Some(MotionGate { frame })
    }

    /// The frame has no motion blur, as the instruction the site's exit
    /// skips writes (mov byte [esp+14h],0); the caller then takes the
    /// site's `accumulate` exit.
    pub fn without_blur(self) {
        // SAFETY: execute's [esp+14h], a byte of its frame.
        unsafe { write(self.frame.motion_blur(), 0) }
    }
}

/// DXGI_FORMAT_R16G16B16A16_FLOAT, a format create_targets makes targets of
/// (the bloom's, 0x163460..).
const RGBA16_FLOAT: u32 = 10;

/// `targets.motion_format`: create_targets about to make rt 18.
pub struct MotionFormat {
    frame: raw::NewRtFrame,
}

impl MotionFormat {
    pub fn at(ctx: &Context<'_>) -> Option<MotionFormat> {
        // SAFETY: the context is this site's: its ESP is new_rt's arguments.
        let frame = unsafe { raw::site::targets_motion_format::frame(ctx) }?;
        Some(MotionFormat { frame })
    }

    /// The target is made RGBA16_FLOAT: signed, where the stock HDR format
    /// (R11G11B10_FLOAT) keeps no negative motion.
    pub fn signed(self) {
        // SAFETY: the pushed format, the word new_rt takes it from.
        unsafe { write(self.frame.format(), RGBA16_FLOAT) }
    }
}

/// The targets of the frame's context, the render thread's.
#[derive(Clone, Copy)]
pub struct Targets {
    context: raw::RendererContext,
    _local: PhantomData<*const ()>,
}

/// A target of the frame (rt_index_to_name 0x162550).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetIndex {
    /// `$user$position`: the linear view depth, R16_FLOAT.
    LinearDepth = 9,
    /// `$user$object_motion_vectors`.
    ObjectMotion = 18,
    /// `$user$present`: the frame after the post-process, RGBA8.
    Present = 52,
}

impl Targets {
    /// A target as it is now: for the hook that asked (set_size remakes
    /// them).
    pub fn get(&self, _: &Thread, index: TargetIndex) -> Option<Target> {
        // SAFETY: the context's targets (get_rt 0x11C1D0: descriptor `index`
        // of [context+119Ch]), each a render_target and its texture, alive
        // on the render thread until set_size makes others.
        unsafe {
            let family = self.context.targets()?.at(index as usize);
            let target = family.target()?;
            let texture = family.texture()?;
            Some(Target { texture, size: Size { width: read(target.width()), height: read(target.height()) } })
        }
    }
}

/// A target of the frame: its texture and size.
#[derive(Clone, Copy)]
pub struct Target {
    texture: raw::ResTexture,
    size: Size,
}

impl Target {
    pub fn size(&self) -> Size {
        self.size
    }

    /// Its ID3D11Resource, what DXVK's interop names its VkImage by.
    pub(in crate::vostok) fn resource(&self) -> Option<raw::HwResource> {
        // SAFETY: a target the frame holds: its texture's +1ACh.
        unsafe { self.texture.hw_resource() }
    }

    /// The resource's address, for a log line.
    pub fn resource_address(&self) -> u32 {
        self.resource().map_or(0, |resource| resource.address())
    }
}

/// A format a target is read back in: what the target was made with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// DXGI_FORMAT_R16G16B16A16_FLOAT: four half floats.
    Rgba16Float,
    /// DXGI_FORMAT_R8G8B8A8_UNORM.
    Rgba8,
}

impl PixelFormat {
    fn dxgi(self) -> u32 {
        match self {
            PixelFormat::Rgba16Float => RGBA16_FLOAT,
            PixelFormat::Rgba8 => 28,
        }
    }

    /// A pixel's bytes.
    pub fn bytes(self) -> u32 {
        match self {
            PixelFormat::Rgba16Float => 8,
            PixelFormat::Rgba8 => 4,
        }
    }
}

/// A target's pixels, mapped for [`read_back`]'s closure.
pub struct Pixels<'a> {
    data: NonNull<u8>,
    pitch: u32,
    size: Size,
    format: PixelFormat,
    _mapped: PhantomData<&'a ()>,
}

impl Pixels<'_> {
    pub fn size(&self) -> Size {
        self.size
    }

    pub fn format(&self) -> PixelFormat {
        self.format
    }

    /// A row's pixels; none past the rows.
    pub fn row(&self, row: u32) -> Option<&[u8]> {
        let len = self.size.width * self.format.bytes();
        if row >= self.size.height || len > self.pitch {
            return None;
        }
        // SAFETY: a mapped staging of `height` rows holds `pitch` bytes a
        // row (D3D11_MAPPED_SUBRESOURCE), alive until the unmap after the
        // closure.
        Some(unsafe {
            core::slice::from_raw_parts(self.data.as_ptr().add(row as usize * self.pitch as usize), len as usize)
        })
    }
}

/// A diagnostic: `target`'s pixels read back and given to `f`. A staging
/// texture of the target's size is made, the target copied to it and mapped
/// (waiting for the GPU), then let go. None when the device made no staging
/// or mapped none (a `format` other than the target's own).
pub fn read_back<R>(_: &Thread, target: &Target, format: PixelFormat, f: impl FnOnce(&Pixels<'_>) -> R) -> Option<R> {
    let Size { width, height } = target.size;
    // SAFETY: the render device's and the resources' globals, pointers the
    // game sets once; on the render thread (the caller's Targets).
    let (context, manager) = unsafe {
        (raw::image::device_global().device()?.context()?, raw::image::render_resources().manager()?)
    };
    // SAFETY: create_texture2d (0x3BEA70), no name, no initial data, one
    // level, no bind: a staging, as the bake's read-back makes its own.
    let staging = Texture::of(unsafe {
        raw::engine_create_texture2d(width, Some(manager), null(), height, null(), format.dxgi(), STAGING, 1, 0)
    }?);
    staging.hold();
    let mut pitch = 0;
    // SAFETY: copy2D (0x3C0E00: CopySubresourceRegion of width x height) of
    // the target, alive while the hook runs, into ours; map2D (0x3D0F80) of
    // our staging, blocking; the Unmap of what it mapped.
    let out = unsafe {
        raw::engine_copy2d(width, height, Some(staging.raw()), Some(target.texture));
        let data = NonNull::new(raw::engine_map2d(Some(staging.raw()), &mut pitch, 0).cast::<u8>());
        let out = data.map(|data| f(&Pixels { data, pitch, size: target.size, format, _mapped: PhantomData }));
        if data.is_some() {
            raw::engine_context_unmap(Some(context), staging.raw().hw_resource(), 0);
        }
        out
    };
    staging.release();
    out
}
