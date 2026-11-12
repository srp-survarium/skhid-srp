//! The 3D frame at another size than the window's, for patch 83's render
//! scale (docs/notes/dlss-option.md, "The render size").
//!
//! Stock, three things take the render output's size, not the targets':
//! the frame's base viewport (setup_render_output_window 0x13BAC0), the
//! screen image's viewport (it sets none) and the depth-stencil (the
//! output's own, the backend's m_base_zb and `t_depth_render_target`). Here
//! are the pieces that make a frame follow its targets instead: the size
//! set_size makes them at ([`TargetsSize`]), a depth-stencil of that size
//! ([`DepthTarget`]), the frame entered and left around the stages
//! ([`enter`], [`leave`]), the linear depth copied from ours
//! ([`copy_depth_from`]), and the targets remade ([`remake_targets`]). The
//! output's own depth-stencil is never touched: Scaleform's stencil on the
//! back buffer stays the window's size.
//!
//! The sites are written on demand ([`install`]): a game that never asks for
//! a scale runs the engine's own code.

use core::marker::PhantomData;
use core::ptr::{null, read, write};
use core::sync::atomic::{AtomicBool, Ordering};

use super::context::{Context, Reg};
use super::hook::Thread;
use crate::raw;

/// DXGI_FORMAT_R24G8_TYPELESS, the render output's own depth-stencil format
/// (update_depth_stencil_buffer 0x3D0083).
const R24G8_TYPELESS: u32 = 0x2C;
/// render_target::create's usage of a depth-stencil target (0x3D1F2A).
const DEPTH_STENCIL: u32 = 0;
/// DXGI_FORMAT_R8G8B8A8_UNORM, `$user$present`'s format (create_targets'
/// push 1Ch for rt 52, 0x16322F).
const R8G8B8A8_UNORM: u32 = 0x1C;
/// render_target::create's usage of a colour target, new_rt's (push 1,
/// 0x162A17).
const COLOUR: u32 = 1;

/// A size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    /// The whole of it as a viewport (D3D11_VIEWPORT: x, y, width, height,
    /// depth 0..1), as setup_render_output_window's.
    fn viewport(self) -> [f32; 6] {
        [0.0, 0.0, self.width as f32, self.height as f32, 0.0, 1.0]
    }
}

/// One reference of a render target of ours (the engine's render_target,
/// made as new_rt makes the frame's), let go when dropped. The render
/// thread's: made and dropped there.
struct Held {
    target: raw::RenderTarget,
    _local: PhantomData<*const ()>,
}

impl Held {
    /// A target of `size`, `format` (a DXGI_FORMAT) and `usage`
    /// (render_target::create's); None when the engine made none (no
    /// resource manager yet, the GPU's memory).
    fn make(size: Size, format: u32, usage: u32) -> Option<Held> {
        // SAFETY: the render resources' global, a pointer the game sets once.
        let manager = unsafe { raw::image::render_resources().manager() }?;
        // SAFETY: create_render_target (0x3BF930) as new_rt calls it
        // (0x162A0E..0x162A34): no name (it makes one), no texture given,
        // default memory, not pooled; on the thread that makes the frame's
        // targets.
        let target = unsafe {
            raw::engine_create_render_target(Some(manager), null(), size.width, size.height, format, usage, None, 0, 0, 0)
        }?;
        // SAFETY: the target the game just made: its count, plain adds as
        // new_rt's (inc [eax], 0x162A3F).
        unsafe {
            let count = target.references();
            write(count, read(count).wrapping_add(1));
        }
        Some(Held { target, _local: PhantomData })
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        // SAFETY: our reference let go as new_rt lets a target go
        // (add [eax],-1, then resource_manager::release 0x3BFBC0 at 0), on
        // the thread that made it (not Send).
        unsafe {
            let count = self.target.references();
            let left = read(count).wrapping_sub(1);
            write(count, left);
            if left == 0 {
                raw::engine_release_render_target(raw::image::render_resources().manager(), Some(self.target));
            }
        }
    }
}

/// A depth-stencil target of ours (the engine's render_target, usage
/// depth-stencil: an R24G8 texture, its D24S8 view and its res_texture, as
/// the render output's own), one reference held, let go when dropped. The
/// render thread's: made, bound and dropped there.
pub struct DepthTarget {
    held: Held,
    size: Size,
}

impl DepthTarget {
    /// A depth-stencil of `size`; None when the engine made none (no
    /// resource manager yet, the GPU's memory).
    pub fn make(_: &Thread, size: Size) -> Option<DepthTarget> {
        let made = DepthTarget { held: Held::make(size, R24G8_TYPELESS, DEPTH_STENCIL)?, size };
        // a target whose view or texture the device refused is dropped here
        (made.view() != 0 && made.texture().is_some()).then_some(made)
    }

    pub fn size(&self) -> Size {
        self.size
    }

    /// Its ID3D11DepthStencilView (0 none).
    fn view(&self) -> u32 {
        // SAFETY: a target we hold a reference of.
        unsafe { read(self.held.target.depth_stencil_view()) }
    }

    /// Its res_texture, what the engine samples it through.
    pub fn texture(&self) -> Option<DepthTexture> {
        // SAFETY: as view.
        unsafe { self.held.target.texture() }.map(DepthTexture)
    }
}

/// A colour target of ours, made as `$user$present` is (create_targets'
/// new_rt of rt 52, 0x16322F: R8G8B8A8_UNORM): what the screen image's quad
/// samples in place of it, at the window's size. One reference held, let go
/// when dropped; the render thread's.
pub struct ColourTarget {
    held: Held,
    size: Size,
}

impl ColourTarget {
    /// A target of `size`; None when the engine made none.
    pub fn make(_: &Thread, size: Size) -> Option<ColourTarget> {
        let made = ColourTarget { held: Held::make(size, R8G8B8A8_UNORM, COLOUR)?, size };
        made.texture().is_some().then_some(made)
    }

    pub fn size(&self) -> Size {
        self.size
    }

    /// Its res_texture, what the engine samples it through.
    pub fn texture(&self) -> Option<ColourTexture> {
        // SAFETY: a target we hold a reference of.
        unsafe { self.held.target.texture() }.map(ColourTexture)
    }
}

/// A colour target's texture, for the hook that read it (the target is its
/// owner's for at least that long).
#[derive(Clone, Copy)]
pub struct ColourTexture(raw::ResTexture);

impl ColourTexture {
    /// The frame's picture as the screen image is handed it
    /// (`$user$present`, or one of ours in its place).
    pub(in crate::vostok) fn of(texture: raw::ResTexture) -> ColourTexture {
        ColourTexture(texture)
    }

    pub(in crate::vostok) fn raw(&self) -> raw::ResTexture {
        self.0
    }

    /// Its ID3D11Resource, what DXVK's interop names its VkImage by.
    pub(in crate::vostok) fn resource(&self) -> Option<raw::HwResource> {
        // SAFETY: a texture of a target its owner holds for the hook.
        unsafe { self.0.hw_resource() }
    }
}

/// `render_window.targets_size`: render_output_window::set_size about to
/// make the frame's targets (create_targets' arguments pushed).
pub struct TargetsSize {
    frame: raw::SetSizeTargetsFrame,
}

impl TargetsSize {
    pub fn at(ctx: &Context<'_>) -> Option<TargetsSize> {
        // SAFETY: the context is this site's: its ESP is the arguments.
        let frame = unsafe { raw::site::render_window_targets_size::frame(ctx) }?;
        Some(TargetsSize { frame })
    }

    /// The size the targets are about to be made at: the window's.
    pub fn asked(&self) -> Size {
        // SAFETY: the site's stack words, create_targets' size.
        unsafe { Size { width: read(self.frame.width()), height: read(self.frame.height()) } }
    }

    /// The targets made at `size` instead.
    pub fn make_at(self, size: Size) {
        // SAFETY: as asked; create_targets takes the size from these words
        // alone (0x162DA0: its [esp+8], [esp+0Ch]).
        unsafe {
            write(self.frame.width(), size.width);
            write(self.frame.height(), size.height);
        }
    }
}

/// At `render.output_setup`: the frame's targets made again, by the engine's
/// own forced set_size of the window at the size and mode it has (what
/// end_render_options_changing does, 0x1181CF): `render_window.targets_size`
/// runs inside it. False when the frame has no window.
pub fn remake_targets(ctx: &Context<'_>) -> bool {
    // SAFETY: the context is this site's: its ESP is renderer::render's
    // frame, [esp+2Ch] the window it renders to, alive for the call.
    let Some(window) = (unsafe { raw::site::render_output_setup::frame(ctx).and_then(|frame| frame.window()) }) else {
        return false;
    };
    // SAFETY: the window's own size and mode, read and given back to its
    // set_size (0x119AB0) with the force on; on the thread that renders,
    // before anything of this frame is bound.
    unsafe {
        let (width, height) = (read(window.width()), read(window.height()));
        let fullscreen = u32::from(read(window.windowed()) == 0);
        raw::engine_render_window_set_size(width, Some(window), height, fullscreen, 1);
    }
    true
}

/// At `render.output_set`: the stages' base viewport is `depth`'s size (the
/// frame's targets') and their depth-stencil is `depth`, in place of the
/// render output's. [`leave`] ends it; the next frame's
/// setup_render_output_window ends it too.
pub fn enter(_: &Thread, depth: &DepthTarget) {
    // SAFETY: the render backend's global, a pointer the game sets once.
    let Some(backend) = (unsafe { raw::image::backend_global().backend() }) else { return };
    let viewport = depth.size().viewport();
    // SAFETY: set_viewports (0x11C080) with six floats of ours, copied by
    // it; the backend's base and current depth-stencil, plain words the
    // render thread alone writes, as reset_depth_stencil_target writes them
    // (0x13BAF8..0x13BB22). `depth` is held by the caller's state for as
    // long as it is the frame's.
    unsafe {
        raw::engine_set_viewports(viewport.as_ptr(), Some(backend));
        write(backend.base_depth_stencil(), depth.view());
        write(backend.depth_stencil(), depth.view());
        write(backend.depth_stencil_dirty(), 1);
    }
}

/// At `render.present_stage`: the viewport is the render output's whole
/// size again (the screen image's quad sets none) and the base
/// depth-stencil the output's own (Scaleform's, on the back buffer).
pub fn leave(_: &Thread) {
    // SAFETY: as enter; the output is the one the frame presents to.
    unsafe {
        let Some(backend) = raw::image::backend_global().backend() else { return };
        let Some(output) = backend.render_output() else { return };
        let viewport = Size { width: read(output.width()), height: read(output.height()) }.viewport();
        raw::engine_set_viewports(viewport.as_ptr(), Some(backend));
        write(backend.base_depth_stencil(), read(output.depth_stencil_view()));
    }
}

/// A depth target's texture, for [`copy_depth_from`] in the hook that read
/// it (the target is its owner's for at least that long).
#[derive(Clone, Copy)]
pub struct DepthTexture(raw::ResTexture);

impl DepthTexture {
    /// Its ID3D11Resource, what DXVK's interop names its VkImage by.
    pub(in crate::vostok) fn resource(&self) -> Option<raw::HwResource> {
        // SAFETY: a texture of a target its owner holds for the hook.
        unsafe { self.0.hw_resource() }
    }

    /// The resource's address, for a log line.
    pub fn resource_address(&self) -> u32 {
        self.resource().map_or(0, |resource| resource.address())
    }
}

/// At `gbuffer.depth_texture`: the linear depth (rt 9) is copied from
/// `texture` (as `t_depth_render_target`), not from the render output's.
pub fn copy_depth_from(ctx: &mut Context<'_>, texture: DepthTexture) {
    ctx.set_reg(Reg::Ecx, texture.0.address());
}

/// Why [`install`] wrote nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallRefused {
    /// The render device exists already: a frame may be running the sites.
    NotTheStart,
    /// A site's bytes or page refused.
    Site(super::InstallRefused),
}

static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Whether [`install`] wrote the sites.
pub fn installed() -> bool {
    INSTALLED.load(Ordering::Acquire)
}

/// Writes the render size's sites (`hud_options.toml`'s hooks installed on
/// demand), once. Only before the render device is made: the sites are
/// code of the window's set_size, renderer::render and the G-buffer stage,
/// which nothing runs without a device, so no thread is in them while they
/// are written. A refusal leaves the game's code as it was.
pub fn install(thread: &Thread) -> Result<(), InstallRefused> {
    if installed() {
        return Ok(());
    }
    // SAFETY: the render device's global, 0 until the device is made.
    if unsafe { raw::image::device_global().device() }.is_some() {
        return Err(InstallRefused::NotTheStart);
    }
    let sites = super::hud_options_on_demand();
    // SAFETY: checked above: no render device, so no frame, no window of it
    // and no stage runs: nothing executes the sites' spans.
    let unlocked = unsafe { super::hook::unlock_on_demand(thread, &sites) }.map_err(InstallRefused::Site)?;
    unlocked.write();
    INSTALLED.store(true, Ordering::Release);
    Ok(())
}
