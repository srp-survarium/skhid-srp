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
}

// SAFETY: made and used on the render thread alone.
unsafe impl Send for Lens {}

/// Where the lens is drawn: its centre and radii in the frame's clip space,
/// and its rim's darkening (how sharp, how dark).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Disc {
    pub centre: [f32; 2],
    pub radius: [f32; 2],
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
            // 32 bytes, dynamic, a constant buffer, written by the CPU
            let desc = BufferDesc { bytes: 32, usage: 2, bind: 4, cpu_access: 1 << 16, misc: 0, stride: 0 };
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
            raw::engine_d3d_device_release(device.cast());
            Some(Lens { vertex: made(gv, v)?, pixel: made(gp, p)?, constants: made(gc, c)?, sampler: made(gs, s)? })
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
            let values = [disc.centre[0], disc.centre[1], disc.radius[0], disc.radius[1], disc.rim[0], disc.rim[1], 0.0, 0.0];
            core::ptr::copy_nonoverlapping(values.as_ptr(), mapped.data.cast::<f32>(), values.len());
            raw::engine_context_unmap(c, Some(self.constants.0), 0);

            let mut target = [None];
            let mut depth = None;
            raw::engine_context_om_get_render_targets(c, 1, target.as_mut_ptr(), &mut depth);
            let Some(target_view) = target[0] else {
                if let Some(d) = depth {
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
            raw::engine_context_om_set_blend_state(c, None, null(), u32::MAX);
            raw::engine_context_om_set_depth_stencil_state(c, None, 0);
            raw::engine_context_rs_set_state(c, None);
            raw::engine_context_rs_set_viewports(c, 1, (&raw const full).cast());
            raw::engine_context_draw(c, 4, 0);

            // the state as it was
            raw::engine_context_ps_set_shader_resources(c, 0, 1, [None].as_ptr().cast_mut());
            raw::engine_context_om_set_render_targets(c, 1, [Some(target_view)].as_ptr().cast_mut(), depth);
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
            for got in [Some(target_view), depth, layout, vs, ps, gs, vcb[0], pcb[0], srv[0], smp[0], blend, depth_state, raster].into_iter().flatten() {
                raw::engine_d3d_resource_release(Some(got));
            }
        }
        true
    }
}

/// A model of the scene as the render thread compares its surfaces: the
/// static model instance an optic's aimed model draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelId(raw::ModelInstance);

// SAFETY: read on the render thread while the weapon holds its optic (the
// aimed model is in the scene between add_aimed and remove_aimed).
unsafe impl Send for ModelId {}

/// The held weapon's optic at `camera.zoom` (update_camera, the game's
/// thread), when it is a magnified one (hide_weapon_on_aim).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scoped {
    /// The optic's fov_factor: 1 / its magnification of the 65 degrees.
    pub fov_factor: f32,
    /// Its aimed model is in the scene (the weapon's +181Dh).
    pub shown: bool,
    pub aimed: Option<ModelId>,
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
        let aimed = optics.aim_model().and_then(|m| m.render_instance().instance()).map(ModelId);
        let eye = read16(raw::site::camera_zoom::player(ctx)?.eye());
        Some(Scoped { fov_factor: *optics.fov_factor(), shown: *weapon.scope_in_scene() != 0, aimed, eye })
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

/// At `scope.*` (add/remove_aimed_rifle_scope_to_scene, ESI the weapon):
/// its address (an id) and whether it is the eyes' (is_first_person_view).
pub fn scope_weapon(ctx: &Context<'_>) -> Option<(u32, bool)> {
    // SAFETY: the context is one of these four sites', each holding the
    // weapon in ESI (sites_026g.tsv), alive while it runs on the game's thread.
    unsafe {
        let weapon = raw::site::scope_add_idle::weapon(ctx)?;
        Some((weapon.address(), raw::engine_weapon_first_person(Some(weapon)) != 0))
    }
}

/// Which of the frame's first-person models stay, at
/// `forward.execute_foreground`: only `model`'s surfaces, or all but them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keep {
    Only(ModelId),
    AllBut(ModelId),
}

/// The frame's foreground list kept as `keep` says (a static model's
/// surfaces are its records, +298h, 64h bytes each); a model that is not a
/// static instance keeps the list as it is.
pub fn filter_foreground(ctx: &Context<'_>, keep: Keep) {
    let (Keep::Only(ModelId(model)) | Keep::AllBut(ModelId(model))) = keep;
    // SAFETY: the context is this site's: ECX the forward stage on the
    // render thread, its scene view the frame's; the model alive (see
    // ModelId); the list's pointers rewritten within begin..end.
    unsafe {
        if *model.vtable() != raw::image::STATIC_MODEL_VTABLE {
            return;
        }
        let first = model.first_surface().address();
        let past = first + u32::from(*model.surface_count()) * raw::RenderSurfaceInstance::SIZE as u32;
        let Some(view) = raw::site::forward_execute_foreground::stage(ctx).and_then(|s| s.context()).and_then(|c| c.scene_view()) else {
            return;
        };
        let (begin, end) = (*view.foreground_first() as usize as *mut u32, *view.foreground_end() as usize as *mut u32);
        if begin.is_null() || end < begin {
            return;
        }
        let mut kept = begin;
        let mut at = begin;
        while at < end {
            let surface = at.read();
            let ours = (first..past).contains(&surface);
            if ours == matches!(keep, Keep::Only(_)) {
                kept.write(surface);
                kept = kept.add(1);
            }
            at = at.add(1);
        }
        *view.foreground_end() = kept as usize as u32;
    }
}

/// The frame's back buffer's width over its height, at `render.flash_ui`.
pub fn frame_aspect(ctx: &Context<'_>) -> Option<f32> {
    // SAFETY: as composite's.
    let window = unsafe { raw::site::render_flash_ui::frame(ctx).and_then(|f| f.window()) }?;
    let (_, desc) = back_buffer(window)?;
    (desc.height > 0).then(|| desc.width as f32 / desc.height as f32)
}

/// An output window of the game's, kept by the render thread from one frame
/// to the next (the frame's own: alive for the game).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameWindow(raw::RenderOutputWindow);

// SAFETY: named on the render thread alone.
unsafe impl Send for FrameWindow {}

/// The window the frame at `render.flash_ui` draws into.
pub fn frame_window(ctx: &Context<'_>) -> Option<FrameWindow> {
    // SAFETY: as composite's.
    unsafe { raw::site::render_flash_ui::frame(ctx).and_then(|f| f.window()) }.map(FrameWindow)
}

/// The second view given the frame's exposure: the frame window's last
/// adapted luminance copied into ours, which our draw's eye adaptation then
/// keeps (its time step 0, `render.frame_delta`). The render thread, at our
/// draw's start.
pub fn match_exposure(_: &Thread, second: &SecondView, frame: FrameWindow) -> bool {
    let Some(context) = immediate_context() else { return false };
    // SAFETY: both windows alive (ours for good, the frame's for the game),
    // their 1x1 luminance targets of one format, their resources the
    // textures'; CopyResource on the render thread's immediate context.
    unsafe {
        let (Some(to), Some(from)) = (second.window.adapted_luminance(), frame.0.adapted_luminance()) else { return false };
        let (Some(to), Some(from)) = (to.hw_resource(), from.hw_resource()) else { return false };
        raw::engine_context_copy_resource(Some(context), Some(to), Some(from));
    }
    true
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

/// The aimed model's lens on the frame's screen, at `forward.execute_foreground`
/// (the frame's draw, its first-person models' spheres this frame's, world
/// space): of its surfaces the camera's axis passes through, the smallest of
/// at least `least` metres (the eyepiece's glass or the reticle's plane,
/// not the body nor a dot), projected with the frame's `view` and
/// `projection`. Its centre and radii in clip space.
pub fn lens_on_screen(model: ModelId, view: &[f32; 16], projection: &[f32; 16], least: f32) -> Option<([f32; 2], [f32; 2])> {
    let ModelId(model) = model;
    // SAFETY: the model alive (see ModelId) on the render thread, its
    // records its own; each sphere four floats at the record's +10h.
    unsafe {
        if *model.vtable() != raw::image::STATIC_MODEL_VTABLE {
            return None;
        }
        let first = model.first_surface();
        let mut best: Option<(f32, [f32; 3])> = None;
        for index in 0..usize::from(*model.surface_count()) {
            let sphere = first.at(index).sphere();
            let (c, r) = ([sphere.read(), sphere.add(1).read(), sphere.add(2).read()], sphere.add(3).read());
            let v: [f32; 3] = core::array::from_fn(|k| c[0] * view[k] + c[1] * view[4 + k] + c[2] * view[8 + k] + view[12 + k]);
            let off_axis = v[0] * v[0] + v[1] * v[1];
            if v[2] <= 0.0 || r < least || off_axis > r * r || best.is_some_and(|(b, _)| b <= r) {
                continue;
            }
            best = Some((r, v));
        }
        let (r, v) = best?;
        let (x, y) = (v[0] * projection[0] + v[2] * projection[8], v[1] * projection[5] + v[2] * projection[9]);
        Some(([x / v[2], y / v[2]], [r * projection[0] / v[2], r * projection[5] / v[2]]))
    }
}
