//! A second view of the frame's scene, drawn before the frame into an output
//! window of our own (patch 198, docs/notes/pip-scopes.md).
//!
//! The engine draws a scene view into an output window per draw_scene
//! command (game::renderer::draw_scene 0x1F7F0 -> renderer::render
//! 0x13D010), each window with its own render targets, depth and history
//! (render_output_window +118h, sized by its window), each view with its own
//! visible lists, exposure and camera: 0.10b's editor draws one scene into
//! two windows a frame. Ours is a hidden window whose swap chain is never
//! presented; its back buffer is the picture.
//!
//! - [`make`] (the render thread): the window, the output window and the
//!   scene view, each counted once by us and kept for the process;
//! - [`Frame`] (the game's thread, `game.draw_scene`): the frame's camera and
//!   the second view's draw, queued before the frame's own;
//! - the render thread's sites: which view a draw is ([`drawn_view`]), the
//!   frame's weather given to the second view ([`match_environment`]), the
//!   first-person models left out of it ([`drop_foreground`]), its picture
//!   copied into the frame's back buffer ([`composite`]), GPU timestamps
//!   ([`GpuClock`]).

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr::{null, null_mut, NonNull};

use super::context::Context;
use super::Thread;
use crate::raw;

/// IID_ID3D11Texture2D, {6F15AAF2-D208-4E89-9AB4-489535D34F9C}.
const TEXTURE2D_IID: [u8; 16] =
    [0xF2, 0xAA, 0x15, 0x6F, 0x08, 0xD2, 0x89, 0x4E, 0x9A, 0xB4, 0x48, 0x95, 0x35, 0xD3, 0x4F, 0x9C];

const WS_POPUP: u32 = 1 << 31;
const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const PAGE_READWRITE: u32 = 0x04;

/// The second view and its window: made once, never let go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecondView {
    view: raw::SceneView,
    window: raw::RenderOutputWindow,
}

// SAFETY: both are counted by us for the process; the game's thread only
// passes them to draw commands, the render thread draws them.
unsafe impl Send for SecondView {}

/// Why the second view was not made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MakeError {
    /// CreateWindowExA gave no window.
    Window,
    /// No pages for the objects.
    Memory,
    /// A constructor gave nothing back.
    Constructed,
}

/// Fresh zeroed pages of ours for one `T`, kept for the process.
fn room<T>() -> Option<NonNull<T>> {
    // SAFETY: the exe's VirtualAlloc import: committed pages of our own,
    // zeroed, aligned to 64 KiB, or null.
    let base = unsafe {
        (raw::engine_virtual_alloc)(null_mut(), size_of::<T>() as u32, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE)
    };
    NonNull::new(base.cast::<T>())
}

/// The second view drawn into a hidden `size` x `size` window. On the render
/// thread (the scene view's and output window's cooks run there; the
/// window's targets are the renderer's resources).
pub fn make(_: &Thread, size: u32) -> Result<SecondView, MakeError> {
    // SAFETY: user32's CreateWindowExA (the exe's delay-load slot): a popup
    // of the predefined STATIC class, never shown, no parent.
    let window = unsafe {
        (raw::engine_create_window_ex_a)(
            0,
            c"STATIC".as_ptr().cast(),
            c"".as_ptr().cast(),
            WS_POPUP,
            0,
            0,
            size,
            size,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
        )
    };
    if window.is_null() {
        return Err(MakeError::Window);
    }
    let mut configuration = raw::OutputWindowConfigurationStorage::zeroed();
    let (output_room, view_room) =
        (room::<raw::RenderOutputWindowStorage>().ok_or(MakeError::Memory)?, room::<raw::SceneViewStorage>().ok_or(MakeError::Memory)?);
    // SAFETY: a configuration of ours (the window, its own client size kept,
    // windowed, no flash renderer); each constructor on zeroed room of its
    // kind's size, kept for good, on the render thread as the cooks do;
    // then our one reference each, so no draw command's release reaches 0.
    unsafe {
        let c = configuration.handle();
        *c.window() = window as usize as u32;
        *c.windowed() = 1;
        let output = raw::engine_output_window_construct(output_room.as_ptr().cast(), Some(c)).ok_or(MakeError::Constructed)?;
        let view = raw::engine_scene_view_construct(view_room.as_ptr().cast()).ok_or(MakeError::Constructed)?;
        *output.references() = 1;
        *view.references() = 1;
        Ok(SecondView { view, window: output })
    }
}

/// The game's scene about to be drawn, at `game.draw_scene` (game::tick,
/// the game's thread): its camera, and the second view's draw.
pub struct Frame<'t> {
    game: raw::Game,
    scene: raw::GameScene,
    _thread: &'t Thread,
}

/// A scene view of the game's, as the render thread may name it while a
/// draw of it is queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MainView(raw::SceneView);

// SAFETY: compared and read only on the render thread while the game's
// scene holds it (and the frame's own draw command, queued after ours).
unsafe impl Send for MainView {}

fn read16(at: *mut f32) -> [f32; 16] {
    let mut out = [0.0; 16];
    for (i, slot) in out.iter_mut().enumerate() {
        // SAFETY: 16 floats at `at` (the callers' fields say so).
        *slot = unsafe { at.add(i).read() };
    }
    out
}

impl<'t> Frame<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<Frame<'t>> {
        // SAFETY: the context is this site's: EBX the game, on its thread;
        // its active scene is the one game::tick draws next.
        let game = unsafe { raw::site::game_draw_scene::game(ctx) }?;
        let scene = unsafe { game.active_scene() }?;
        Some(Frame { game, scene, _thread: ctx.thread() })
    }

    /// The game's logic sleeps 10 ms before each tick (its +4E38h, 0x1A06C).
    pub fn logic_sleeps(&self) -> bool {
        // SAFETY: a byte of the live game, on its thread.
        unsafe { *self.game.logic_sleeps() != 0 }
    }

    /// The camera's transform (rows right, up, forward, place) this frame.
    pub fn camera(&self) -> [f32; 16] {
        // SAFETY: the scene's 16 floats at +0Ch, apply_camera's this frame.
        read16(unsafe { self.scene.camera() })
    }

    /// The projection this frame (D3D's, row vectors).
    pub fn projection(&self) -> [f32; 16] {
        // SAFETY: the scene's 16 floats at +4Ch.
        read16(unsafe { self.scene.projection() })
    }

    /// The scene's view, for the render thread's draw of ours.
    pub fn main_view(&self) -> Option<MainView> {
        // SAFETY: the scene's view reference, alive with the scene.
        unsafe { self.scene.view_ref().view() }.map(MainView)
    }

    /// Queues the second view's draw of this scene from `view` (a view
    /// matrix) with `projection`, before the frame's own draw; then puts the
    /// scene renderer's matrices back to the frame's (`main_view`,
    /// `main_projection`), as apply_camera left them.
    pub fn draw_second(&self, second: &SecondView, view: &[f32; 16], projection: &[f32; 16], main_view: &[f32; 16], main_projection: &[f32; 16]) {
        // SAFETY: the game's renderer and scene renderer, alive for the run;
        // references of ours to our view and window (each counted by us,
        // the commands add their own); the scene's references and the game's
        // viewport, alive for the call (draw_scene copies them); all on the
        // game's thread, where game::tick makes these same calls.
        unsafe {
            let Some(renderer) = self.game.renderer() else { return };
            let Some(scene_renderer) = renderer.scene_renderer() else { return };
            let mut view_ref = raw::SceneViewRefStorage::zeroed();
            let view_ref = view_ref.handle();
            view_ref.set_view(Some(second.view));
            let mut window_ref = raw::OutputWindowRefStorage::zeroed();
            let window_ref = window_ref.handle();
            window_ref.set_window(Some(second.window));
            raw::engine_set_view_matrix(Some(scene_renderer), Some(view_ref), view.as_ptr());
            raw::engine_set_projection_matrix(Some(scene_renderer), Some(view_ref), projection.as_ptr());
            raw::engine_draw_scene(
                Some(self.scene.scene_ref()),
                Some(renderer),
                Some(view_ref),
                Some(window_ref),
                Some(self.game.viewport()),
                null(),
            );
            let own = self.scene.view_ref();
            raw::engine_set_view_matrix(Some(scene_renderer), Some(own), main_view.as_ptr());
            raw::engine_set_projection_matrix(Some(scene_renderer), Some(own), main_projection.as_ptr());
        }
    }
}

/// Which view the draw at `render.begin` (renderer::render's start, the
/// render thread) is: the second one or another.
pub fn drawn_view(ctx: &Context<'_>, second: &SecondView) -> bool {
    // SAFETY: the context is this site's: the frame's third argument the
    // scene-view reference the draw command holds, alive while it draws.
    unsafe { raw::site::render_begin::frame(ctx).and_then(|f| f.view_ref()).and_then(|r| r.view()) == Some(second.view) }
}

/// The second view given the frame's weather (the main view's environment,
/// its resources counted by the engine's own assignment). The render thread,
/// at the second view's draw: the main view's draw is queued after it.
pub fn match_environment(_: &Thread, second: &SecondView, main: MainView) {
    // SAFETY: both views alive (ours for good; the main one held by the
    // queued draw after ours); environment_properties::operator= on the
    // render thread, which every environment change runs on.
    unsafe { raw::engine_environment_assign(Some(second.view.environment()), Some(main.0.environment())) }
}

/// At `forward.execute_foreground`: the frame's first-person models left out (its
/// foreground list emptied: the visibility stage fills it again next frame).
pub fn drop_foreground(ctx: &Context<'_>) {
    // SAFETY: the context is this site's: ECX the stage executing on the
    // render thread, its context's scene view the frame's; the list's end
    // set to its begin (pointers it does not own).
    unsafe {
        let Some(view) = raw::site::forward_execute_foreground::stage(ctx).and_then(|s| s.context()).and_then(|c| c.scene_view()) else {
            return;
        };
        *view.foreground_end() = *view.foreground_first();
    }
}

/// A reference of ours to a Direct3D object, released when dropped.
struct Held(raw::HwResource);

impl Drop for Held {
    fn drop(&mut self) {
        // SAFETY: a reference GetBuffer or CreateQuery gave us, released once.
        unsafe { raw::engine_d3d_resource_release(Some(self.0)) };
    }
}

/// A D3D11_TEXTURE2D_DESC.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Desc {
    width: u32,
    height: u32,
    mip_levels: u32,
    array_size: u32,
    format: u32,
    samples: u32,
    quality: u32,
    usage: u32,
    bind: u32,
    cpu_access: u32,
    misc: u32,
}

fn back_buffer(output: raw::RenderOutputWindow) -> Option<(Held, Desc)> {
    // SAFETY: the output window's render output and its swap chain, alive
    // with the window; GetBuffer 0 as a texture (a reference added); its desc.
    unsafe {
        let chain = output.output()?.swap_chain()?;
        let mut buffer = None;
        let got = raw::engine_swap_chain_get_buffer(Some(chain), 0, TEXTURE2D_IID.as_ptr().cast(), &mut buffer);
        let buffer = buffer.filter(|_| got & 1 << 31 == 0).map(Held)?;
        let mut desc = Desc::default();
        raw::engine_texture2d_get_desc(Some(buffer.0), (&raw mut desc).cast());
        Some((buffer, desc))
    }
}

fn immediate_context() -> Option<raw::ImmediateContext> {
    // SAFETY: the render device's global, which the game sets once, and its
    // immediate context: the render thread's alone.
    unsafe { raw::image::device_global().device().and_then(|device| device.context()) }
}

/// At `render.flash_ui` (the frame's back buffer holds its scene, not yet
/// its interface): the second view's picture copied into its top right
/// corner, `margin` pixels in. Whether it was.
pub fn composite(ctx: &Context<'_>, second: &SecondView, margin: u32) -> bool {
    // SAFETY: the context is this site's: its window argument the output
    // window the frame draws into, alive for the frame.
    let Some(frame_window) = (unsafe { raw::site::render_flash_ui::frame(ctx).and_then(|f| f.window()) }) else {
        return false;
    };
    if frame_window == second.window {
        return false;
    }
    let (Some((target, to)), Some((source, from)), Some(context)) =
        (back_buffer(frame_window), back_buffer(second.window), immediate_context())
    else {
        return false;
    };
    if from.format != to.format || from.width + margin > to.width || from.height + margin > to.height {
        return false;
    }
    // SAFETY: the immediate context on its thread: a whole-texture copy
    // between two textures of one format, the region inside the target.
    unsafe {
        raw::engine_context_copy_subresource_region(
            Some(context),
            Some(target.0),
            0,
            to.width - from.width - margin,
            margin,
            0,
            Some(source.0),
            0,
            null(),
        )
    };
    true
}

/// D3D11_QUERY_TIMESTAMP and _TIMESTAMP_DISJOINT (D3D11_QUERY: event, occlusion, then these).
const TIMESTAMP: u32 = 2;
const TIMESTAMP_DISJOINT: u32 = 3;
/// D3D11_ASYNC_GETDATA_DONOTFLUSH.
const DO_NOT_FLUSH: u32 = 1;

/// GPU timestamps of a frame's marks, `RING` frames in flight.
pub struct GpuClock {
    frames: [GpuFrame; RING],
}

/// The frames a clock keeps in flight before it reads the oldest.
pub const RING: usize = 4;
/// The marks of a frame.
pub const MARKS: usize = 4;

struct GpuFrame {
    disjoint: Held,
    marks: [Held; MARKS],
    /// Which marks were stamped this frame.
    stamped: [bool; MARKS],
}

// SAFETY: made, stamped and read on the render thread alone.
unsafe impl Send for GpuClock {}

/// Why a frame's marks were not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    NoContext,
    /// The interval's GetData answer and its Disjoint word.
    Disjoint { got: u32, disjoint: u32 },
    Mark { mark: u32, got: u32 },
}

/// A frame's marks read back: each one's time from the first, in ms (None:
/// not stamped).
pub type Marks = [Option<f64>; MARKS];

fn create_query(device: *mut c_void, kind: u32) -> Option<Held> {
    let desc = [kind, 0u32];
    let mut query = None;
    // SAFETY: ID3D11Device::CreateQuery with a D3D11_QUERY_DESC of ours.
    let made = unsafe { raw::engine_device_create_query(device.cast(), desc.as_ptr().cast(), &mut query) };
    query.filter(|_| made & 1 << 31 == 0).map(Held)
}

impl GpuClock {
    /// On the render thread, with a frame's back buffer to find the device.
    pub fn make(ctx: &Context<'_>) -> Option<GpuClock> {
        // SAFETY: as composite's: the frame's output window at this site.
        let window = unsafe { raw::site::render_flash_ui::frame(ctx).and_then(|f| f.window()) }?;
        let (buffer, _) = back_buffer(window)?;
        let mut device = null_mut();
        // SAFETY: the back buffer's device (a reference added), let go below.
        unsafe { raw::engine_device_child_get_device(Some(buffer.0), &mut device) };
        if device.is_null() {
            return None;
        }
        let frame = || -> Option<GpuFrame> {
            Some(GpuFrame {
                disjoint: create_query(device.cast(), TIMESTAMP_DISJOINT)?,
                marks: [
                    create_query(device.cast(), TIMESTAMP)?,
                    create_query(device.cast(), TIMESTAMP)?,
                    create_query(device.cast(), TIMESTAMP)?,
                    create_query(device.cast(), TIMESTAMP)?,
                ],
                stamped: [false; MARKS],
            })
        };
        let frames = [frame(), frame(), frame(), frame()];
        // SAFETY: the device's reference GetDevice added.
        unsafe { raw::engine_d3d_device_release(device.cast()) };
        let [Some(a), Some(b), Some(c), Some(d)] = frames else { return None };
        Some(GpuClock { frames: [a, b, c, d] })
    }

    /// Frame `slot`'s disjoint interval begins.
    pub fn begin(&mut self, slot: usize) {
        let (Some(context), Some(frame)) = (immediate_context(), self.frames.get_mut(slot)) else { return };
        frame.stamped = [false; MARKS];
        // SAFETY: the immediate context on its thread, a query of ours.
        unsafe { raw::engine_context_begin(Some(context), Some(frame.disjoint.0)) };
    }

    /// Frame `slot`'s mark `mark` stamped now (in the GPU's order).
    pub fn stamp(&mut self, slot: usize, mark: usize) {
        let (Some(context), Some(frame)) = (immediate_context(), self.frames.get_mut(slot)) else { return };
        let Some(query) = frame.marks.get(mark) else { return };
        // SAFETY: as begin's.
        unsafe { raw::engine_context_end(Some(context), Some(query.0)) };
        frame.stamped[mark] = true;
    }

    /// Frame `slot`'s disjoint interval ends.
    pub fn end(&mut self, slot: usize) {
        let (Some(context), Some(frame)) = (immediate_context(), self.frames.get(slot)) else { return };
        // SAFETY: as begin's.
        unsafe { raw::engine_context_end(Some(context), Some(frame.disjoint.0)) };
    }

    /// Frame `slot`'s marks, once the GPU has them (None: not yet, or a
    /// disjoint interval).
    pub fn read(&mut self, slot: usize) -> Result<Marks, ReadError> {
        let context = immediate_context().ok_or(ReadError::NoContext)?;
        let frame = self.frames.get(slot).ok_or(ReadError::NoContext)?;
        // D3D11_QUERY_DATA_TIMESTAMP_DISJOINT: u64 frequency, BOOL disjoint (padded to 16).
        let mut disjoint = [0u32; 4];
        // SAFETY: GetData of a query of ours into room of its data's size.
        let got = unsafe {
            raw::engine_context_get_data(Some(context), Some(frame.disjoint.0), disjoint.as_mut_ptr().cast(), 16, DO_NOT_FLUSH)
        };
        if got != 0 || disjoint[2] != 0 {
            return Err(ReadError::Disjoint { got, disjoint: disjoint[2] });
        }
        let frequency = u64::from(disjoint[0]) | u64::from(disjoint[1]) << 32;
        if frequency == 0 {
            return Err(ReadError::Disjoint { got, disjoint: 0 });
        }
        let mut ticks = [None; MARKS];
        for (mark, query) in frame.marks.iter().enumerate() {
            if !frame.stamped[mark] {
                continue;
            }
            let mut tick = [0u32; 2];
            // SAFETY: as above, a u64.
            let got = unsafe {
                raw::engine_context_get_data(Some(context), Some(query.0), tick.as_mut_ptr().cast(), 8, DO_NOT_FLUSH)
            };
            if got != 0 {
                return Err(ReadError::Mark { mark: mark as u32, got });
            }
            ticks[mark] = Some(u64::from(tick[0]) | u64::from(tick[1]) << 32);
        }
        let first = ticks.iter().flatten().min().copied().ok_or(ReadError::NoContext)?;
        Ok(ticks.map(|t| t.map(|t| t.saturating_sub(first) as f64 * 1000.0 / frequency as f64)))
    }
}

/// The CPU's clock now, in ms (QueryPerformanceCounter).
pub fn cpu_ms(_: &Thread) -> f64 {
    let (mut count, mut frequency) = ([0u32; 2], [0u32; 2]);
    // SAFETY: kernel32's counter and its frequency, each a u64.
    unsafe {
        (raw::engine_query_performance_counter)(count.as_mut_ptr());
        (raw::engine_query_performance_frequency)(frequency.as_mut_ptr());
    }
    let count = u64::from(count[0]) | u64::from(count[1]) << 32;
    let frequency = (u64::from(frequency[0]) | u64::from(frequency[1]) << 32).max(1);
    count as f64 * 1000.0 / frequency as f64
}

/// The hi-Z occlusion's switch (render options +12Dh), set to `on`; what it
/// was. Its query and stored answers are the renderer's alone, made from one
/// view's depth: the second view's draw neither reads nor sends one, and
/// draws every model in its frustum. The render thread, around that draw.
pub fn set_hiz(_: &Thread, on: bool) -> bool {
    // SAFETY: the render options' global, set once by the renderer; the byte
    // the render thread reads at each draw's visibility stage.
    unsafe {
        let Some(options) = raw::image::options_global().options() else { return false };
        let switch = options.hiz_occlusion();
        let was = *switch != 0;
        *switch = u8::from(on);
        was
    }
}

/// D3D11_SAMPLER_DESC, linear and clamped.
#[repr(C)]
struct SamplerDesc {
    filter: u32,
    address: [u32; 3],
    bias: f32,
    anisotropy: u32,
    comparison: u32,
    border: [f32; 4],
    min_lod: f32,
    max_lod: f32,
}

/// A D3D11_MAPPED_SUBRESOURCE.
#[repr(C)]
struct MappedData {
    data: *mut c_void,
    row_pitch: u32,
    depth_pitch: u32,
}

/// D3D11_BUFFER_DESC.
#[repr(C)]
struct BufferDesc {
    bytes: u32,
    usage: u32,
    bind: u32,
    cpu_access: u32,
    misc: u32,
    stride: u32,
}

/// D3D11_VIEWPORT.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Viewport {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    min_depth: f32,
    max_depth: f32,
}

/// The lens's Direct3D objects: its two shaders, its constants and its
/// sampler, made once on the render thread.
pub struct Lens {
    vertex: Held,
    pixel: Held,
    constants: Held,
    sampler: Held,
    /// Source alpha over the frame: the disc's edge covers its pixels by its share.
    blend: Held,
}

// SAFETY: made and used on the render thread alone.
unsafe impl Send for Lens {}

/// Where the lens is drawn: the glass in the frame's view space (its centre
/// and two radii across its plane, metres), the frame's projection, the
/// picture's disc on the screen (clip space: centre, radii) and the rim's
/// darkening (how sharp, how dark).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Disc {
    pub centre: [f32; 3],
    pub across: [[f32; 3]; 2],
    pub projection: [f32; 16],
    pub picture: [f32; 4],
    pub rim: [f32; 2],
}


fn device_of(texture: &Held) -> Option<*mut c_void> {
    let mut device = null_mut();
    // SAFETY: a live texture's device, a reference added (the caller lets it go).
    unsafe { raw::engine_device_child_get_device(Some(texture.0), &mut device) };
    (!device.is_null()).then_some(device)
}

impl Lens {
    /// The lens's objects, `vertex` and `pixel` the shaders' DXBC; at
    /// `render.flash_ui`, whose frame's back buffer names the device.
    pub fn make(ctx: &Context<'_>, vertex: &[u8], pixel: &[u8]) -> Option<Lens> {
        // SAFETY: as composite's: the frame's output window at this site.
        let window = unsafe { raw::site::render_flash_ui::frame(ctx).and_then(|f| f.window()) }?;
        let (buffer, _) = back_buffer(window)?;
        let device = device_of(&buffer)?;
        let made = |got: u32, object: Option<raw::HwResource>| object.filter(|_| got & 1 << 31 == 0).map(Held);
        // SAFETY: the device's Create* with descs and code of ours, each
        // object into its own Option; then the device's reference let go.
        let lens = unsafe {
            let (mut v, mut p, mut c, mut s) = (None, None, None, None);
            let gv = raw::engine_device_create_vertex_shader(device.cast(), vertex.as_ptr().cast(), vertex.len() as u32, null_mut(), &mut v);
            let gp = raw::engine_device_create_pixel_shader(device.cast(), pixel.as_ptr().cast(), pixel.len() as u32, null_mut(), &mut p);
            // the shader's nine float4, dynamic, a constant buffer, written by the CPU
            let desc = BufferDesc { bytes: CONSTANTS as u32 * 4, usage: 2, bind: 4, cpu_access: 1 << 16, misc: 0, stride: 0 };
            let gc = raw::engine_device_create_buffer(device.cast(), (&raw const desc).cast(), null(), &mut c);
            // MIN_MAG_MIP_LINEAR, CLAMP, comparison NEVER, every mip
            let sampler = SamplerDesc {
                filter: 0x15,
                address: [3; 3],
                bias: 0.0,
                anisotropy: 1,
                comparison: 1,
                border: [0.0; 4],
                min_lod: 0.0,
                max_lod: f32::MAX,
            };
            let gs = raw::engine_device_create_sampler_state(device.cast(), (&raw const sampler).cast(), &mut s);
            // D3D11_BLEND_DESC (264 bytes): no alpha to coverage, target 0: SRC_ALPHA, INV_SRC_ALPHA, ADD;
            // alpha ONE, ZERO, ADD; red, green and blue written, the frame's alpha kept (the other seven unused)
            let mut blend_desc = [0u32; 66];
            blend_desc[2..10].copy_from_slice(&[1, 5, 6, 1, 2, 1, 1, 0x07]);
            let mut b = None;
            let gb = raw::engine_device_create_blend_state(device.cast(), blend_desc.as_ptr().cast(), &mut b);
            raw::engine_d3d_device_release(device.cast());
            Some(Lens { vertex: made(gv, v)?, pixel: made(gp, p)?, constants: made(gc, c)?, sampler: made(gs, s)?, blend: made(gb, b)? })
        };
        lens
    }

    /// At `render.flash_ui` (the frame's picture on its back buffer, before
    /// the interface): the second view's picture drawn as `disc` over the
    /// render target bound now. Every state it sets is put back as it was, so
    /// the engine's own record of the state stays true. Whether it drew.
    pub fn draw(&self, ctx: &Context<'_>, second: &SecondView, disc: &Disc) -> bool {
        // SAFETY: as composite's.
        let Some(frame_window) = (unsafe { raw::site::render_flash_ui::frame(ctx).and_then(|f| f.window()) }) else {
            return false;
        };
        let (Some((_, to)), Some(context)) = (back_buffer(frame_window), immediate_context()) else { return false };
        // SAFETY: our window's present texture and its shader view, alive
        // with the window (kept for good).
        let Some(picture) = (unsafe { second.window.present_texture().and_then(|t| t.view()) }) else { return false };
        let c = Some(context);
        // SAFETY: the immediate context on the render thread, its own
        // thread. Each Get* adds a reference to what it gives, let go after
        // the matching Set* put it back; our objects are alive in self.
        unsafe {
            let mut mapped = MappedData { data: null_mut(), row_pitch: 0, depth_pitch: 0 };
            // WRITE_DISCARD
            if raw::engine_context_map(c, Some(self.constants.0), 0, 4, 0, (&raw mut mapped).cast()) & 1 << 31 != 0 || mapped.data.is_null() {
                return false;
            }
            let values = constants(disc, [to.width as f32, to.height as f32]);
            core::ptr::copy_nonoverlapping(values.as_ptr(), mapped.data.cast::<f32>(), values.len());
            raw::engine_context_unmap(c, Some(self.constants.0), 0);

            let mut target = [None];
            let mut depth_view = None;
            raw::engine_context_om_get_render_targets(c, 1, target.as_mut_ptr(), &mut depth_view);
            let Some(target_view) = target[0] else {
                if let Some(d) = depth_view {
                    raw::engine_d3d_resource_release(Some(d));
                }
                return false;
            };
            let mut layout = None;
            raw::engine_context_ia_get_input_layout(c, &mut layout);
            let mut topology = 0;
            raw::engine_context_ia_get_primitive_topology(c, &mut topology);
            let (mut vs, mut ps, mut gs) = (None, None, None);
            let (mut nv, mut np, mut ng) = (0u32, 0u32, 0u32);
            raw::engine_context_vs_get_shader(c, &mut vs, null_mut(), &mut nv);
            raw::engine_context_ps_get_shader(c, &mut ps, null_mut(), &mut np);
            raw::engine_context_gs_get_shader(c, &mut gs, null_mut(), &mut ng);
            let (mut vcb, mut pcb, mut srv, mut smp) = ([None], [None], [None], [None]);
            raw::engine_context_vs_get_constant_buffers(c, 0, 1, vcb.as_mut_ptr());
            raw::engine_context_ps_get_constant_buffers(c, 0, 1, pcb.as_mut_ptr());
            raw::engine_context_ps_get_shader_resources(c, 0, 1, srv.as_mut_ptr());
            raw::engine_context_ps_get_samplers(c, 0, 1, smp.as_mut_ptr());
            let (mut blend, mut factor, mut mask) = (None, [0f32; 4], 0u32);
            raw::engine_context_om_get_blend_state(c, &mut blend, factor.as_mut_ptr(), &mut mask);
            let (mut depth_state, mut stencil) = (None, 0u32);
            raw::engine_context_om_get_depth_stencil_state(c, &mut depth_state, &mut stencil);
            let mut raster = None;
            raw::engine_context_rs_get_state(c, &mut raster);
            let mut viewports = [Viewport::default(); 16];
            let mut count = 16u32;
            raw::engine_context_rs_get_viewports(c, &mut count, viewports.as_mut_ptr().cast());

            let full = Viewport { x: 0.0, y: 0.0, width: to.width as f32, height: to.height as f32, min_depth: 0.0, max_depth: 1.0 };
            raw::engine_context_om_set_render_targets(c, 1, [Some(target_view)].as_ptr().cast_mut(), None);
            raw::engine_context_ia_set_input_layout(c, None);
            // TRIANGLESTRIP
            raw::engine_context_ia_set_primitive_topology(c, 5);
            raw::engine_context_vs_set_shader(c, Some(self.vertex.0), null(), 0);
            raw::engine_context_ps_set_shader(c, Some(self.pixel.0), null(), 0);
            raw::engine_context_gs_set_shader(c, None, null(), 0);
            raw::engine_context_vs_set_constant_buffers(c, 0, 1, [Some(self.constants.0)].as_ptr().cast_mut());
            raw::engine_context_ps_set_constant_buffers(c, 0, 1, [Some(self.constants.0)].as_ptr().cast_mut());
            raw::engine_context_ps_set_shader_resources(c, 0, 1, [Some(picture)].as_ptr().cast_mut());
            raw::engine_context_ps_set_samplers(c, 0, 1, [Some(self.sampler.0)].as_ptr().cast_mut());
            raw::engine_context_om_set_blend_state(c, Some(self.blend.0), null(), u32::MAX);
            raw::engine_context_om_set_depth_stencil_state(c, None, 0);
            raw::engine_context_rs_set_state(c, None);
            raw::engine_context_rs_set_viewports(c, 1, (&raw const full).cast());
            raw::engine_context_draw(c, 4, 0);

            // the state as it was
            raw::engine_context_ps_set_shader_resources(c, 0, 1, [None].as_ptr().cast_mut());
            raw::engine_context_om_set_render_targets(c, 1, [Some(target_view)].as_ptr().cast_mut(), depth_view);
            raw::engine_context_ia_set_input_layout(c, layout);
            raw::engine_context_ia_set_primitive_topology(c, topology);
            raw::engine_context_vs_set_shader(c, vs, null(), 0);
            raw::engine_context_ps_set_shader(c, ps, null(), 0);
            raw::engine_context_gs_set_shader(c, gs, null(), 0);
            raw::engine_context_vs_set_constant_buffers(c, 0, 1, vcb.as_ptr().cast_mut());
            raw::engine_context_ps_set_constant_buffers(c, 0, 1, pcb.as_ptr().cast_mut());
            raw::engine_context_ps_set_shader_resources(c, 0, 1, srv.as_ptr().cast_mut());
            raw::engine_context_ps_set_samplers(c, 0, 1, smp.as_ptr().cast_mut());
            raw::engine_context_om_set_blend_state(c, blend, factor.as_ptr(), mask);
            raw::engine_context_om_set_depth_stencil_state(c, depth_state, stencil);
            raw::engine_context_rs_set_state(c, raster);
            raw::engine_context_rs_set_viewports(c, count, viewports.as_ptr().cast());
            for got in [Some(target_view), depth_view, layout, vs, ps, gs, vcb[0], pcb[0], srv[0], smp[0], blend, depth_state, raster].into_iter().flatten() {
                raw::engine_d3d_resource_release(Some(got));
            }
        }
        true
    }
}

/// The lens shader's constants, float4 by float4 (pip_lens.hlsl's cbuffer).
const CONSTANTS: usize = 36;

fn constants(disc: &Disc, frame: [f32; 2]) -> [f32; CONSTANTS] {
    let mut c = [0.0; CONSTANTS];
    c[..3].copy_from_slice(&disc.centre);
    c[4..7].copy_from_slice(&disc.across[0]);
    c[8..11].copy_from_slice(&disc.across[1]);
    c[12..28].copy_from_slice(&disc.projection);
    c[28..32].copy_from_slice(&disc.picture);
    c[32..34].copy_from_slice(&disc.rim);
    c[34..36].copy_from_slice(&frame);
    c
}

/// A model of the scene as the render thread compares its surfaces: the
/// static model instance an optic's aimed model draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelId(raw::ModelInstance, ModelKind);

/// The model instance classes whose surfaces the lens reads, by their vtable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelKind {
    Static,
    Skeleton,
}

/// An aimed model the lens cannot read: its instance's vtable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownModel(pub u32);

impl ModelId {
    /// # Safety
    /// `model` is alive on this thread.
    unsafe fn parse(model: raw::ModelInstance) -> Result<ModelId, UnknownModel> {
        let vtable = unsafe { *model.vtable() };
        match vtable {
            raw::image::STATIC_MODEL_VTABLE => Ok(ModelId(model, ModelKind::Static)),
            raw::image::SKELETON_MODEL_VTABLE => Ok(ModelId(model, ModelKind::Skeleton)),
            other => Err(UnknownModel(other)),
        }
    }

    pub fn kind(self) -> ModelKind {
        self.1
    }

    /// The instance's address, for the log.
    pub fn address(self) -> u32 {
        self.0.address()
    }
}

impl ModelKind {
    pub fn name(self) -> &'static str {
        match self {
            ModelKind::Static => "static",
            ModelKind::Skeleton => "skeleton",
        }
    }
}

// SAFETY: read on the render thread while the weapon holds its optic (the
// aimed model is in the scene between add_aimed and remove_aimed).
unsafe impl Send for ModelId {}

/// The held weapon's optic at `camera.zoom` (update_camera, the game's
/// thread), when it is a magnified one (hide_weapon_on_aim).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scoped {
    /// The optic's fov_factor: 1 / its magnification of the 65 degrees.
    pub fov_factor: f32,
    /// The weapon (an address: whose aimed model the lens put in).
    pub weapon: u32,
    /// The scope's view is on (+181Eh).
    pub scope_view: bool,
    pub aimed: Option<Result<ModelId, UnknownModel>>,
    /// The idle model: the optic in view.
    pub idle: Option<Result<ModelId, UnknownModel>>,
    /// The eyes' transform (the player's +44Ch, bone Item: where a shot starts
    /// and goes), the second view's camera: the frame's view may be another.
    pub eye: [f32; 16],
}

pub fn scoped(ctx: &Context<'_>) -> Option<Scoped> {
    // SAFETY: the context is this site's: EAX the held weapon, alive for
    // update_camera on the game's thread; its optics and their models the
    // weapon's while it holds them.
    unsafe {
        let weapon = raw::site::camera_zoom::weapon(ctx)?;
        let optics = weapon.optics()?;
        if *optics.hide_weapon_on_aim() == 0 {
            return None;
        }
        let aimed = optics.aim_model().and_then(|m| m.render_instance().instance()).map(|m| ModelId::parse(m));
        let eye = read16(raw::site::camera_zoom::player(ctx)?.eye());
        let idle = optics.model().and_then(|m| m.render_instance().instance()).map(|m| ModelId::parse(m));
        Some(Scoped { fov_factor: *optics.fov_factor(), weapon: weapon.address(), scope_view: *weapon.scope_view() != 0, aimed, idle, eye })
    }
}

/// uro_fov, the vertical field of view of the eyes unzoomed, degrees.
pub fn field_of_view(_: &Thread) -> f32 {
    // SAFETY: the console variable's value pointer (its registration's, a
    // float in .data), read and written on the game's thread.
    unsafe { raw::image::uro_fov().value().as_ref().copied().unwrap_or(65.0) }
}

impl Frame<'_> {
    /// The scene's projection for this frame's draw set to `projection`
    /// (the scene's copy, which the interface projects with, and a set of
    /// the view's queued after apply_camera's).
    pub fn set_projection(&self, projection: &[f32; 16]) {
        // SAFETY: the scene's 16 floats at +4Ch, the game's thread's; the
        // renderers alive for the run; the scene's own view reference.
        unsafe {
            let at = self.scene.projection();
            for (i, value) in projection.iter().enumerate() {
                at.add(i).write(*value);
            }
            let Some(scene_renderer) = self.game.renderer().and_then(|r| r.scene_renderer()) else { return };
            raw::engine_set_projection_matrix(Some(scene_renderer), Some(self.scene.view_ref()), projection.as_ptr());
        }
    }
}

/// Which of the frame's first-person surfaces stay, at
/// `forward.execute_foreground`: only `model`'s (its lens surface, `except`,
/// left out too), or all but them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keep {
    Only { model: ModelId, except: Option<u32> },
    AllBut(ModelId),
}

/// The first-person surfaces of the drawn view kept as `keep` says: the
/// foreground list (+0C4B8h, forward's) and the first-person entries of the
/// forward-lit list (+44A0h, stage_lights' foreground pass, which draws the
/// guns, hands and optics); the world's entries of that list stay (drawn
/// before, in slot 16). A static model's surfaces are its records.
/// What a filter left: each list's entries before and after, the lit list's
/// first-person ones before, and up to 8 of their owners (for the log).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filtered {
    pub foreground: (u32, u32),
    pub lit: (u32, u32),
    pub lit_first_person: u32,
    pub owners: [u32; 8],
}

/// Where a filter runs: the forward stage's foreground (its two lists),
/// or the G-buffer stage before it copies its list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage22 {
    Forward,
    Gbuffer,
}

pub fn filter_foreground(ctx: &Context<'_>, at: Stage22, keep: Keep) -> Filtered {
    let (Keep::Only { model, .. } | Keep::AllBut(model)) = keep;
    // SAFETY: the context is this site's: ECX the forward stage on the
    // render thread, its scene view the drawn one; the model alive (see
    // ModelId); each list's entries live render_surface_instances (their
    // owner +3Ch a live model instance, its foreground byte +114h), the
    // lists' ends rewritten within begin..end (the visibility stage fills
    // them again next frame).
    unsafe {
        let mut filtered = Filtered::default();
        let Some((first, count)) = surfaces(model) else { return filtered };
        let first = first.address();
        let past = first + count as u32 * raw::RenderSurfaceInstance::SIZE as u32;
        let context = match at {
            Stage22::Forward => raw::site::forward_execute_foreground::stage(ctx).and_then(|s| s.context()),
            Stage22::Gbuffer => raw::site::gbuffer_models::stage(ctx).and_then(|s| s.context()),
        };
        let Some(view) = context.and_then(|c| c.scene_view()) else {
            return filtered;
        };
        let stays = |surface: u32| {
            let ours = (first..past).contains(&surface);
            match keep {
                Keep::Only { except, .. } => ours && except != Some(surface),
                Keep::AllBut(_) => !ours,
            }
        };
        let (first_word, end_word) = match at {
            Stage22::Forward => {
                filtered.foreground = compact(view.foreground_first(), view.foreground_end(), stays);
                (view.lit_first(), view.lit_end())
            }
            Stage22::Gbuffer => {
                // the other drawn list's first-person surfaces too (an optic's see-through glass is there)
                filtered.foreground = compact(view.forward_first(), view.forward_end(), |surface| !first_person(surface) || stays(surface));
                (view.opaque_first(), view.opaque_end())
            }
        };
        let mut owners = 0;
        let mut seen = 0;
        filtered.lit = compact(first_word, end_word, |surface| {
            if !first_person(surface) {
                return true;
            }
            seen += 1;
            let owner = (surface as usize as *const u8).add(0x3C).cast::<u32>().read();
            if owners < 8 && !filtered.owners[..owners].contains(&owner) {
                filtered.owners[owners] = owner;
                owners += 1;
            }
            stays(surface)
        });
        filtered.lit_first_person = seen;
        filtered
    }
}

/// Whether a render_surface_instance belongs to a first-person model (its
/// owner's foreground byte, as render_forward_lighting tests it, 0x192DC1).
///
/// # Safety
/// `surface` is a live render_surface_instance.
unsafe fn first_person(surface: u32) -> bool {
    unsafe {
        let owner = (surface as usize as *const u8).add(0x3C).cast::<*const u8>().read();
        !owner.is_null() && owner.add(0x114).read() != 0
    }
}

/// The vector `*first..*end` of u32 entries kept where `stays`, its end moved.
///
/// # Safety
/// The two words are a live vector's begin and end, the render thread's.
unsafe fn compact(first: *mut u32, end: *mut u32, mut stays: impl FnMut(u32) -> bool) -> (u32, u32) {
    unsafe {
        let (begin, past) = (*first as usize as *mut u32, *end as usize as *mut u32);
        if begin.is_null() || past < begin {
            return (0, 0);
        }
        let mut kept = begin;
        let mut at = begin;
        while at < past {
            let surface = at.read();
            if stays(surface) {
                kept.write(surface);
                kept = kept.add(1);
            }
            at = at.add(1);
        }
        *end = kept as usize as u32;
        (past.offset_from(begin) as u32, kept.offset_from(begin) as u32)
    }
}

/// The frame's back buffer's height, pixels, at `render.flash_ui`.
pub fn frame_height(ctx: &Context<'_>) -> Option<u32> {
    // SAFETY: as composite's.
    let window = unsafe { raw::site::render_flash_ui::frame(ctx).and_then(|f| f.window()) }?;
    back_buffer(window).map(|(_, desc)| desc.height).filter(|&h| h > 0)
}

/// The frame's back buffer's width over its height, at `render.flash_ui`.
pub fn frame_aspect(ctx: &Context<'_>) -> Option<f32> {
    // SAFETY: as composite's.
    let window = unsafe { raw::site::render_flash_ui::frame(ctx).and_then(|f| f.window()) }?;
    let (_, desc) = back_buffer(window)?;
    (desc.height > 0).then(|| desc.width as f32 / desc.height as f32)
}

/// r_gpu_sync, the wait for the GPU at the end of each draw (renderer::render
/// 0x13DDCC, after the Present), set to `on`; what it was. The render thread,
/// around the second view's draw: the frame's own draw keeps its wait, so a
/// frame still ends synchronised as today, and the second view's picture is
/// not waited for alone (a second wait a frame made the CPU and the GPU take
/// turns, and in the rig it was where the frame waited for the display).
pub fn set_gpu_sync(_: &Thread, on: bool) -> bool {
    // SAFETY: the console variable's byte in .data, read by the render thread
    // at each draw's end.
    unsafe {
        let flag = raw::image::GPU_SYNC as usize as *mut u8;
        let was = *flag != 0;
        *flag = u8::from(on);
        was
    }
}

/// The eye adaptation's reset flag (the next adaptation starts from the
/// frame's own measure), as it is; and set to `value`. One for every view.
pub fn eye_reset(_: &Thread, value: Option<bool>) -> bool {
    // SAFETY: stage_postprocess's byte in .data, the render thread's.
    unsafe {
        let flag = raw::image::postprocess().reset_eye_adaptation();
        let was = *flag != 0;
        if let Some(value) = value {
            *flag = u8::from(value);
        }
        was
    }
}

/// A stage of the frame the second view's draw can do without.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    AmbientOcclusion,
    Reflections,
    SunShadows,
}

/// The stage's enable byte (render options) set to `on`; what it was.
pub fn set_stage(_: &Thread, stage: Stage, on: bool) -> bool {
    // SAFETY: the render options' global; bytes the render thread reads at
    // each draw's stages.
    unsafe {
        let Some(options) = raw::image::options_global().options() else { return false };
        let byte = match stage {
            Stage::AmbientOcclusion => options.ambient_occlusion(),
            Stage::Reflections => options.reflections(),
            Stage::SunShadows => options.sun_shadows(),
        };
        let was = *byte != 0;
        *byte = u8::from(on);
        was
    }
}

/// A surface sphere larger than this is not an eyepiece (a record whose
/// sphere is not placed yet holds FLT_MAX).
const MOST_LENS: f32 = 0.25;

/// The aimed model's glass: every 0.69 optic's `scope_lense_1p_s` is a
/// fan of 60 triangles about its centre (the pack's meshes, checked for the
/// ten rifle scopes), its vertex and index counts these; the reticle's,
/// red dot's and body's are others.
pub const GLASS_MESH: (u32, u32) = (61, 180);

/// A record's render surface's vertex and index counts (a static one's).
///
/// # Safety
/// The record is a live render_surface_instance.
unsafe fn mesh_of(record: raw::RenderSurfaceInstance) -> Option<(u32, u32)> {
    unsafe { record.surface().map(|s| (*s.vertex_count(), *s.index_count())) }
}

/// The lens on the frame's screen: its centre and radii in clip space, and
/// the surface record it is (an address: the second view leaves it out).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LensPlace {
    pub centre: [f32; 2],
    pub radius: [f32; 2],
    pub surface: u32,
}

/// The aimed model's glass on the frame's screen, at `forward.execute_foreground`
/// (the frame's draw, the model's spheres placed this frame, world space):
/// its record of [`GLASS_MESH`], its bound sphere (a flat disc's: its
/// radius the disc's) projected with the frame's `view` and `projection`.
/// None before the renderer placed it, or for a model without one.
pub fn lens_on_screen(model: ModelId, view: &[f32; 16], projection: &[f32; 16]) -> Option<LensPlace> {
    // SAFETY: the model alive (see ModelId) on the render thread, its
    // records its own; each sphere four floats at the record's +10h.
    unsafe {
        let (first, count) = surfaces(model)?;
        let record = (0..count).map(|index| first.at(index)).find(|&record| mesh_of(record) == Some(GLASS_MESH))?;
        let sphere = record.sphere();
        let (c, r) = ([sphere.read(), sphere.add(1).read(), sphere.add(2).read()], sphere.add(3).read());
        let v: [f32; 3] = core::array::from_fn(|k| c[0] * view[k] + c[1] * view[4 + k] + c[2] * view[8 + k] + view[12 + k]);
        if v[2] <= 0.0 || !(0.001..=MOST_LENS).contains(&r) {
            return None;
        }
        let (x, y) = (v[0] * projection[0] + v[2] * projection[8], v[1] * projection[5] + v[2] * projection[9]);
        Some(LensPlace { centre: [x / v[2], y / v[2]], radius: [r * projection[0] / v[2], r * projection[5] / v[2]], surface: record.address() })
    }
}

/// One of an aimed model's surfaces as the lens's log shows it: its bound
/// sphere's radius and centre (as stored, and in the frame's view space),
/// and its world matrix's place.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceSeen {
    pub radius: f32,
    pub stored: [f32; 3],
    pub in_view: [f32; 3],
    pub placed: Option<[f32; 3]>,
    /// Its mesh's vertex and index counts.
    pub mesh: Option<(u32, u32)>,
    /// The sphere's centre in the model's own space (by its world matrix).
    pub local: Option<[f32; 3]>,
}

/// The most surfaces the log shows.
pub const SEEN: usize = 24;

/// The aimed model's surfaces as the lens reads them, at `forward.execute_foreground`.
pub fn surfaces_seen(model: ModelId, view: &[f32; 16]) -> ([SurfaceSeen; SEEN], usize) {
    let mut seen = [SurfaceSeen::default(); SEEN];
    // SAFETY: as lens_on_screen's; the matrix pointer the record's own, 16 floats or null.
    unsafe {
        let Some((first, count)) = surfaces(model) else { return (seen, 0) };
        for (index, slot) in seen.iter_mut().enumerate().take(count) {
            let record = first.at(index);
            let sphere = record.sphere();
            let c = [sphere.read(), sphere.add(1).read(), sphere.add(2).read()];
            let matrix = record.transform();
            *slot = SurfaceSeen {
                radius: sphere.add(3).read(),
                stored: c,
                in_view: core::array::from_fn(|k| c[0] * view[k] + c[1] * view[4 + k] + c[2] * view[8 + k] + view[12 + k]),
                placed: (!matrix.is_null()).then(|| [matrix.add(12).read(), matrix.add(13).read(), matrix.add(14).read()]),
                mesh: mesh_of(record),
                local: (!matrix.is_null()).then(|| transform_point(&invert_affine(&read16(matrix)), c)),
            };
        }
        (seen, count.min(SEEN))
    }
}

/// One entry of the frame's first-person list, as the lens's log shows it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ForegroundSeen {
    pub address: u32,
    /// Inside the aimed model's surface records.
    pub aimed: bool,
    pub sphere: [f32; 4],
    pub surface: u32,
    pub owner: u32,
    pub placed: Option<[f32; 3]>,
}

/// The frame's first-person list at `forward.execute_foreground`, up to [`SEEN`] entries.
pub fn foreground_seen(ctx: &Context<'_>, model: ModelId) -> ([ForegroundSeen; SEEN], usize) {
    let mut seen = [ForegroundSeen::default(); SEEN];
    // SAFETY: the context is this site's; the list's entries the surfaces the
    // renderer draws this frame, each a live render_surface_instance (its
    // sphere +10h, render surface +38h, matrix pointer +50h).
    unsafe {
        let (first, past) = surfaces(model).map_or((0, 0), |(first, count)| {
            (first.address(), first.address() + count as u32 * raw::RenderSurfaceInstance::SIZE as u32)
        });
        let Some(view) = raw::site::forward_execute_foreground::stage(ctx).and_then(|s| s.context()).and_then(|c| c.scene_view()) else {
            return (seen, 0);
        };
        let (begin, end) = (*view.foreground_first() as usize as *const u32, *view.foreground_end() as usize as *const u32);
        if begin.is_null() || end < begin {
            return (seen, 0);
        }
        let count = (end.offset_from(begin) as usize).min(SEEN);
        for (index, slot) in seen.iter_mut().enumerate().take(count) {
            let address = begin.add(index).read();
            let at = address as usize as *const u8;
            let sphere = at.add(0x10).cast::<f32>();
            let matrix = at.add(0x50).cast::<*const f32>().read();
            *slot = ForegroundSeen {
                address,
                aimed: (first..past).contains(&address),
                sphere: core::array::from_fn(|k| sphere.add(k).read()),
                surface: at.add(0x38).cast::<u32>().read(),
                owner: at.add(0x3C).cast::<u32>().read(),
                placed: (!matrix.is_null()).then(|| [matrix.add(12).read(), matrix.add(13).read(), matrix.add(14).read()]),
            };
        }
        (seen, count)
    }
}

/// A model instance's surface records and their count: a static instance's
/// inside it (+298h, count +294h), a skeleton instance's behind a pointer
/// (+62D4h, count +62D0h; none before it has them).
///
/// # Safety
/// The model is alive on this thread.
unsafe fn surfaces(model: ModelId) -> Option<(raw::RenderSurfaceInstance, usize)> {
    let ModelId(instance, kind) = model;
    unsafe {
        match kind {
            ModelKind::Static => Some((instance.first_surface()?, usize::from(*instance.surface_count()))),
            ModelKind::Skeleton => Some((instance.skeleton_surfaces()?, usize::from(*instance.skeleton_surface_count()))),
        }
    }
}

/// The eyes' weapon at one of its scope-draw sites (`scope_draw.*`,
/// `attachments.hidden`; the game's thread): its optic, its aimed model,
/// and that model in or out of the scene, which the lens owns while it
/// draws (the game's swap does not run for it).
pub struct ScopeWeapon<'t> {
    weapon: raw::Weapon,
    _thread: &'t Thread,
}

/// The scope-draw sites, each holding the weapon in a register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeSite {
    Show,
    Body,
    CulledBody,
    Swap,
    Optic,
    Hidden,
}

impl<'t> ScopeWeapon<'t> {
    pub fn at(ctx: &'t Context<'_>, site: ScopeSite) -> Option<ScopeWeapon<'t>> {
        // SAFETY: the context is that site's (sites_026g.tsv): the weapon it
        // shows, draws or hides, alive while it runs on the game's thread.
        let weapon = unsafe {
            match site {
                ScopeSite::Show => raw::site::scope_draw_show::weapon(ctx),
                ScopeSite::Body => raw::site::scope_draw_body::weapon(ctx),
                ScopeSite::CulledBody => raw::site::scope_draw_culled_body::weapon(ctx),
                ScopeSite::Swap => raw::site::scope_draw_swap::weapon(ctx),
                ScopeSite::Optic => raw::site::scope_draw_optic::weapon(ctx),
                ScopeSite::Hidden => raw::site::attachments_hidden::weapon(ctx),
            }
        }?;
        Some(ScopeWeapon { weapon, _thread: ctx.thread() })
    }

    pub fn address(&self) -> u32 {
        self.weapon.address()
    }

    /// The eyes' weapon (is_first_person_view).
    pub fn first_person(&self) -> bool {
        // SAFETY: the weapon alive (see at), its own test on the game's thread.
        unsafe { raw::engine_weapon_first_person(Some(self.weapon)) != 0 }
    }

    /// Its optic magnifies (hide_weapon_on_aim: the game swaps it).
    pub fn magnified(&self) -> bool {
        // SAFETY: the weapon's optics, alive with it.
        unsafe { self.weapon.optics().is_some_and(|o| *o.hide_weapon_on_aim() != 0) }
    }

    /// The scope's view is on (+181Eh: the aim past the optic's change point).
    pub fn scope_view(&self) -> bool {
        // SAFETY: a byte of the live weapon.
        unsafe { *self.weapon.scope_view() != 0 }
    }

    /// The game's own aimed model is in the scene (+181Dh: its swap ran).
    pub fn swapped(&self) -> bool {
        // SAFETY: a byte of the live weapon.
        unsafe { *self.weapon.scope_in_scene() != 0 }
    }

    /// Its optic's aimed model, when the lens reads its class.
    pub fn aimed(&self) -> Option<Result<ModelId, UnknownModel>> {
        // SAFETY: the weapon's optics and their aimed model, alive with it.
        unsafe { self.weapon.optics()?.aim_model()?.render_instance().instance().map(|m| ModelId::parse(m)) }
    }

    /// The aimed model put in (`Put::In`), moved with the gun (`Put::Moved`)
    /// or taken out (`Put::Out`) of the weapon's scene, at the weapon's
    /// matrix (+660h) as add_aimed_rifle_scope_to_scene puts it (0x244BA9).
    /// Whether the renderer took the call.
    pub fn put_aimed(&self, put: Put) -> bool {
        // SAFETY: the weapon's optics and aimed model, its world's scene and
        // the game's renderers, all alive while the weapon draws or hides;
        // the matrix 16 floats of the weapon's; add/update/remove_model on the
        // game's thread, as weapon::draw calls them.
        unsafe {
            let Some(aimed) = self.weapon.optics().and_then(|o| o.aim_model()) else { return false };
            let Some(world) = self.weapon.world() else { return false };
            let Some(renderer) = world.game().and_then(|g| g.renderer()).and_then(|r| r.scene_renderer()) else { return false };
            let matrix = self.weapon.transform();
            let (model, scene) = (Some(aimed.render_instance()), Some(world.scene()));
            match put {
                Put::In => raw::engine_add_model(Some(renderer), model, scene, matrix, matrix),
                Put::Moved => raw::engine_update_model(Some(renderer), model, scene, matrix, matrix),
                Put::Out => raw::engine_remove_model(Some(renderer), model, scene),
            }
            true
        }
    }
}

/// What [`ScopeWeapon::put_aimed`] does with the aimed model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Put {
    In,
    Moved,
    Out,
}

/// A model instance at `address` (an optic's idle model), when the lens reads its class.
///
/// # Safety
/// `address` is a live model instance on this thread.
pub unsafe fn model_at(model: raw::ModelInstance) -> Result<ModelId, UnknownModel> {
    unsafe { ModelId::parse(model) }
}

/// Diagnostics: the vectors of the drawn scene view (at `forward.execute_foreground`)
/// holding any of `model`'s records: each one's offset, its entries of them, all its entries.
pub fn lists_holding(ctx: &Context<'_>, model: ModelId) -> ([(u32, u32, u32); 12], usize) {
    let mut found = [(0, 0, 0); 12];
    let mut count = 0;
    // SAFETY: the stage's scene view, 1DE40h bytes; each candidate vector read
    // only after VirtualQuery says its whole range is committed and readable.
    unsafe {
        let Some((first, records)) = surfaces(model) else { return (found, 0) };
        let first = first.address();
        let past = first + records as u32 * raw::RenderSurfaceInstance::SIZE as u32;
        let Some(view) = raw::site::forward_execute_foreground::stage(ctx).and_then(|s| s.context()).and_then(|c| c.scene_view()) else {
            return (found, 0);
        };
        let base = view.address() as usize as *const u32;
        let words = raw::SceneView::SIZE / 4;
        for k in 0..words - 1 {
            let (b, e) = (base.add(k).read(), base.add(k + 1).read());
            if b == 0 || b % 4 != 0 || e < b || e - b > 1 << 16 || (e - b) % 4 != 0 || e == b {
                continue;
            }
            if !super::memory::all_readable(b as usize, e as usize) {
                continue;
            }
            let entries = (e - b) / 4;
            let ours = (0..entries).filter(|&i| (first..past).contains(&((b as usize as *const u32).add(i as usize).read()))).count() as u32;
            if ours > 0 && count < found.len() {
                found[count] = ((k * 4) as u32, ours, entries);
                count += 1;
            }
        }
    }
    (found, count)
}

/// Diagnostics: a model's record `index` and the first 32 words of its render surface.
pub fn record_words(model: ModelId, index: usize) -> Option<([u32; 25], [u32; 32])> {
    // SAFETY: the model alive (see ModelId), its records its own; the surface
    // read only when readable.
    unsafe {
        let (first, count) = surfaces(model)?;
        if index >= count {
            return None;
        }
        let record = first.at(index);
        let at = record.address() as usize as *const u32;
        let words: [u32; 25] = core::array::from_fn(|k| at.add(k).read());
        let surface = words[0x38 / 4] as usize;
        if surface == 0 || !super::memory::all_readable(surface, surface + 128) {
            return Some((words, [0; 32]));
        }
        let s = surface as *const u32;
        Some((words, core::array::from_fn(|k| s.add(k).read())))
    }
}

/// The inverse of an affine row-vector transform (rows right, up, forward,
/// place): a camera's view matrix, a model's world-to-model one.
pub fn invert_affine(m: &[f32; 16]) -> [f32; 16] {
    let a = |r: usize, c: usize| m[r * 4 + c];
    let det = a(0, 0) * (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1)) - a(0, 1) * (a(1, 0) * a(2, 2) - a(1, 2) * a(2, 0))
        + a(0, 2) * (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0));
    let inv_det = if det.abs() > 1e-12 { 1.0 / det } else { 1.0 };
    let mut r = [0.0f32; 16];
    r[0] = (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1)) * inv_det;
    r[1] = (a(0, 2) * a(2, 1) - a(0, 1) * a(2, 2)) * inv_det;
    r[2] = (a(0, 1) * a(1, 2) - a(0, 2) * a(1, 1)) * inv_det;
    r[4] = (a(1, 2) * a(2, 0) - a(1, 0) * a(2, 2)) * inv_det;
    r[5] = (a(0, 0) * a(2, 2) - a(0, 2) * a(2, 0)) * inv_det;
    r[6] = (a(0, 2) * a(1, 0) - a(0, 0) * a(1, 2)) * inv_det;
    r[8] = (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0)) * inv_det;
    r[9] = (a(0, 1) * a(2, 0) - a(0, 0) * a(2, 1)) * inv_det;
    r[10] = (a(0, 0) * a(1, 1) - a(0, 1) * a(1, 0)) * inv_det;
    for c in 0..3 {
        r[12 + c] = -(a(3, 0) * r[c] + a(3, 1) * r[4 + c] + a(3, 2) * r[8 + c]);
    }
    r[15] = 1.0;
    r
}

/// A point through a row-vector transform.
pub fn transform_point(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    core::array::from_fn(|c| p[0] * m[c] + p[1] * m[4 + c] + p[2] * m[8 + c] + m[12 + c])
}

/// The most vertices read back of a surface (a glass has some tens).
pub const MOST_VERTICES: usize = 512;

/// A static record's vertex positions in its model's space, read back from
/// the GPU (its vertex buffer copied into a staging buffer and mapped: a wait
/// for the GPU, so once per optic). The render thread. None for a surface of
/// more than [`MOST_VERTICES`] or one not read.
pub fn positions(_: &Thread, record: raw::RenderSurfaceInstance, out: &mut [[f32; 3]; MOST_VERTICES]) -> Option<usize> {
    let context = immediate_context()?;
    // SAFETY: a live record of a model in the scene; its render surface,
    // geometry and buffers the resource's own (renderer-026g.md 2.2); the
    // device's Create/Copy/Map on the render thread's immediate context, our
    // staging buffer released before return; reads within the mapped bytes.
    unsafe {
        let surface = record.surface()?;
        let count = *surface.vertex_count() as usize;
        if count == 0 || count > MOST_VERTICES {
            return None;
        }
        let geometry = surface.geometry()?;
        let stride = *geometry.stride() as usize;
        let vertices = geometry.vertices()?;
        let buffer = vertices.buffer()?;
        let offset = *vertices.offset() as usize;
        // a COM object: its vtable readable (a wrong field would be no object)
        let object = buffer.address() as usize;
        if !super::memory::all_readable(object, object + 4) {
            return None;
        }
        let vtable = (object as *const u32).read() as usize;
        if !super::memory::all_readable(vtable, vtable + 64) {
            return None;
        }
        let mut desc = BufferDesc { bytes: 0, usage: 0, bind: 0, cpu_access: 0, misc: 0, stride: 0 };
        raw::engine_buffer_get_desc(Some(buffer), (&raw mut desc).cast());
        let bytes = count * stride;
        if stride < 12 || (desc.bytes as usize) < offset + bytes {
            return None;
        }
        let mut device = null_mut();
        raw::engine_device_child_get_device(Some(buffer), &mut device);
        if device.is_null() {
            return None;
        }
        // STAGING, read by the CPU
        let staging_desc = BufferDesc { bytes: bytes as u32, usage: 3, bind: 0, cpu_access: 1 << 17, misc: 0, stride: 0 };
        let mut staging = None;
        let made = raw::engine_device_create_buffer(device.cast(), (&raw const staging_desc).cast(), null(), &mut staging);
        raw::engine_d3d_device_release(device.cast());
        let staging = staging.filter(|_| made & 1 << 31 == 0).map(Held)?;
        let area = Box3 { left: offset as u32, top: 0, front: 0, right: (offset + bytes) as u32, bottom: 1, back: 1 };
        raw::engine_context_copy_subresource_region(Some(context), Some(staging.0), 0, 0, 0, 0, Some(buffer), 0, (&raw const area).cast());
        let mut mapped = MappedData { data: null_mut(), row_pitch: 0, depth_pitch: 0 };
        // READ, waiting for the copy
        if raw::engine_context_map(Some(context), Some(staging.0), 0, 1, 0, (&raw mut mapped).cast()) & 1 << 31 != 0 || mapped.data.is_null() {
            return None;
        }
        let base = mapped.data.cast::<u8>();
        for (index, slot) in out.iter_mut().enumerate().take(count) {
            let at = base.add(index * stride).cast::<f32>();
            *slot = [at.read_unaligned(), at.add(1).read_unaligned(), at.add(2).read_unaligned()];
        }
        raw::engine_context_unmap(Some(context), Some(staging.0), 0);
        Some(count)
    }
}

/// A round glass of an optic in its model's space: centre, unit normal
/// (its side toward the eye), radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glass {
    pub centre: [f32; 3],
    pub normal: [f32; 3],
    pub radius: f32,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn length(a: [f32; 3]) -> f32 {
    core::f32::math::sqrt(dot(a, a))
}

fn scaled(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// A flat fan's glass (the aimed model's `scope_lense_1p_s`: a centre and
/// a rim): its centroid, the rim's farthest point, the fan's normal turned
/// toward `eye` (model space).
pub fn fan_glass(points: &[[f32; 3]], eye: [f32; 3]) -> Option<Glass> {
    let n = points.len() as f32;
    if points.len() < 8 {
        return None;
    }
    let centre = points.iter().fold([0.0; 3], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n, s[2] + p[2] / n]);
    let radius = points.iter().map(|&p| length(sub(p, centre))).fold(0.0, f32::max);
    let far = *points.iter().max_by(|a, b| length(sub(**a, centre)).total_cmp(&length(sub(**b, centre))))?;
    let normal = points
        .iter()
        .map(|&p| cross(sub(far, centre), sub(p, centre)))
        .max_by(|a, b| length(*a).total_cmp(&length(*b)))?;
    let size = length(normal);
    if size <= 1e-9 || radius <= 1e-4 {
        return None;
    }
    let normal = scaled(normal, if dot(normal, sub(eye, centre)) < 0.0 { -1.0 / size } else { 1.0 / size });
    Some(Glass { centre, normal, radius })
}

/// The flat rounds of `points` across `axis` (a line through `on`, unit
/// `along` toward the eye): each set of points in one plane across it
/// (within 0.5 mm), on it (its centre within 3 mm of the line), all of
/// them on one circle but a centre point. Up to four, each a [`Glass`].
pub fn rounds_across(points: &mut [[f32; 3]], on: [f32; 3], along: [f32; 3]) -> ([Glass; 4], usize) {
    const GAP: f32 = 0.001;
    const FLAT: f32 = 0.0005;
    let mut found = [Glass { centre: [0.0; 3], normal: along, radius: 0.0 }; 4];
    let mut count = 0;
    points.sort_unstable_by(|a, b| dot(sub(*a, on), along).total_cmp(&dot(sub(*b, on), along)));
    let mut start = 0;
    while start < points.len() {
        let mut end = start + 1;
        while end < points.len() && dot(sub(points[end], points[end - 1]), along) < GAP {
            end += 1;
        }
        let round = &points[start..end];
        start = end;
        let first = dot(sub(round[0], on), along);
        let last = dot(sub(round[round.len() - 1], on), along);
        if round.len() < 6 || last - first > FLAT {
            continue;
        }
        let n = round.len() as f32;
        let centre = round.iter().fold([0.0; 3], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n, s[2] + p[2] / n]);
        let off = sub(centre, on);
        let across = sub(off, scaled(along, dot(off, along)));
        let radius = round.iter().map(|&p| length(sub(p, centre))).fold(0.0, f32::max);
        let one_circle = round.iter().all(|&p| {
            let r = length(sub(p, centre));
            r < 0.3 * radius || r > 0.9 * radius
        });
        if length(across) > 0.003 || !(0.004..=0.04).contains(&radius) || !one_circle {
            continue;
        }
        if count < found.len() {
            found[count] = Glass { centre, normal: along, radius };
            count += 1;
        }
    }
    (found, count)
}

/// A model's surface records as the lens searches them: each one's record.
pub fn records(model: ModelId) -> ([Option<raw::RenderSurfaceInstance>; SEEN], usize) {
    let mut out = [None; SEEN];
    // SAFETY: as lens_on_screen's.
    unsafe {
        let Some((first, count)) = surfaces(model) else { return (out, 0) };
        for (index, slot) in out.iter_mut().enumerate().take(count) {
            *slot = Some(first.at(index));
        }
        (out, count.min(SEEN))
    }
}

/// The aimed model's glass record (its [`GLASS_MESH`]).
pub fn glass_record(model: ModelId) -> Option<raw::RenderSurfaceInstance> {
    // SAFETY: as lens_on_screen's.
    unsafe {
        let (first, count) = surfaces(model)?;
        (0..count).map(|index| first.at(index)).find(|&record| mesh_of(record) == Some(GLASS_MESH))
    }
}

/// A record's world matrix as the renderer draws it this frame (+50h).
pub fn world_matrix(record: raw::RenderSurfaceInstance) -> Option<[f32; 16]> {
    // SAFETY: a live record; its matrix pointer the model's own, 16 floats, or null.
    unsafe {
        let matrix = record.transform();
        (!matrix.is_null()).then(|| read16(matrix))
    }
}


/// The view the draw at `render.begin` draws (the frame's own, or another).
pub fn drawn_main(ctx: &Context<'_>) -> Option<MainView> {
    // SAFETY: as drawn_view's.
    unsafe { raw::site::render_begin::frame(ctx).and_then(|f| f.view_ref()).and_then(|r| r.view()).map(MainView) }
}

/// D3D11_BOX.
#[repr(C)]
struct Box3 {
    left: u32,
    top: u32,
    front: u32,
    right: u32,
    bottom: u32,
    back: u32,
}


/// Diagnostics: where in the drawn scene view (at `render.begin`) 16 floats
/// equal to `m`'s first 12 are (within 1e-4): up to four offsets.
pub fn find_matrix(ctx: &Context<'_>, m: &[f32; 16]) -> ([u32; 4], usize) {
    let mut found = [0; 4];
    let mut count = 0;
    // SAFETY: the drawn view, alive while it is drawn; read within its size.
    unsafe {
        let Some(MainView(view)) = drawn_main(ctx) else { return (found, 0) };
        let base = view.address() as usize as *const f32;
        let words = raw::SceneView::SIZE / 4;
        for k in 0..words - 16 {
            if (0..12).all(|i| (base.add(k + i).read() - m[i]).abs() < 1e-4) && m[..12].iter().any(|v| v.abs() > 1e-3) {
                if count < found.len() {
                    found[count] = (k * 4) as u32;
                    count += 1;
                }
            }
        }
    }
    (found, count)
}

/// The row-vector product `a` then `b`.
pub fn multiply(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    core::array::from_fn(|i| {
        let (r, c) = (i / 4, i % 4);
        (0..4).map(|k| a[r * 4 + k] * b[k * 4 + c]).sum()
    })
}

/// A direction through a row-vector transform.
pub fn transform_direction(m: &[f32; 16], d: [f32; 3]) -> [f32; 3] {
    core::array::from_fn(|c| d[0] * m[c] + d[1] * m[4 + c] + d[2] * m[8 + c])
}

/// Two unit vectors across `normal` (view space, the normal about along
/// the view): the first toward the screen's right, the second toward its top,
/// so a quad of them faces the viewer (the default rasterizer culls the back).
pub fn across(normal: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let u = sub([1.0, 0.0, 0.0], scaled(normal, normal[0]));
    let u = scaled(u, 1.0 / length(u).max(1e-9));
    let w = cross(normal, u);
    (u, if w[1] < 0.0 { scaled(w, -1.0) } else { w })
}

/// The length of a vector.
pub fn norm(a: [f32; 3]) -> f32 {
    length(a)
}

/// The view and projection matrices the draw at `render.begin` draws with
/// (its scene view's, set by the commands queued before it).
pub fn drawn_camera(ctx: &Context<'_>) -> Option<([f32; 16], [f32; 16])> {
    // SAFETY: the drawn view, alive while it is drawn; 16 floats each.
    unsafe {
        let MainView(view) = drawn_main(ctx)?;
        Some((read16(view.view_matrix()), read16(view.projection_matrix())))
    }
}

/// A record's bound sphere's radius (FLT_MAX or 0 before the renderer placed it).
pub fn sphere_radius(record: raw::RenderSurfaceInstance) -> Option<f32> {
    // SAFETY: a live record; its sphere four floats at +10h.
    unsafe { Some(record.sphere().add(3).read()) }
}
