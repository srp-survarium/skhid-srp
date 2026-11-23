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

/// D3D11_QUERY_TIMESTAMP and _TIMESTAMP_DISJOINT.
const TIMESTAMP: u32 = 0;
const TIMESTAMP_DISJOINT: u32 = 1;
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
    pub fn read(&mut self, slot: usize) -> Option<Marks> {
        let context = immediate_context()?;
        let frame = self.frames.get(slot)?;
        // D3D11_QUERY_DATA_TIMESTAMP_DISJOINT: u64 frequency, BOOL disjoint (padded to 16).
        let mut disjoint = [0u32; 4];
        // SAFETY: GetData of a query of ours into room of its data's size.
        let got = unsafe {
            raw::engine_context_get_data(Some(context), Some(frame.disjoint.0), disjoint.as_mut_ptr().cast(), 16, DO_NOT_FLUSH)
        };
        if got != 0 || disjoint[2] != 0 {
            return None;
        }
        let frequency = u64::from(disjoint[0]) | u64::from(disjoint[1]) << 32;
        if frequency == 0 {
            return None;
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
                return None;
            }
            ticks[mark] = Some(u64::from(tick[0]) | u64::from(tick[1]) << 32);
        }
        let first = ticks.iter().flatten().min().copied()?;
        Some(ticks.map(|t| t.map(|t| t.saturating_sub(first) as f64 * 1000.0 / frequency as f64)))
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
