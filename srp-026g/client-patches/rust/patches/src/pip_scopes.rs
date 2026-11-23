//! Patch 198: a second view of the scene each frame, for picture-in-picture
//! scopes (docs/notes/pip-scopes.md). Dormant unless `SKHID_PIP_PROBE` is set
//! in the game's environment (read once, at the first scene drawn):
//!
//! - `256`, `512`, `1024` (or `on`, 512): the scene drawn a second time
//!   each frame, before the frame, from the frame's camera with a quarter of
//!   its field of view, into a square window of that size, and copied into
//!   the frame's top right corner; frame times logged;
//! - `time`: only the frame times logged.
//!
//! The draws are queued on the game's thread (`game.draw_scene`); the
//! render thread knows the second view's draw by its scene view
//! (`render.begin`), gives it the frame's weather, leaves the first-person
//! models, the interface and the Present out of it, and keeps the scene's
//! moved models for the frame's own draw (`render.unmove`). Display only:
//! nothing a match computes.

#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicBool, Ordering};

use crate::vostok::log::{self, Piece};
use crate::vostok::renderer::get_environment;
use crate::vostok::second_view::{self, Frame, GpuClock, MainView, Marks, SecondView, RING};
use crate::vostok::{Context, Flow, Lock, NoExit, OrigRenderFlashUi, PipScopesOnPresentExit, PipScopesOnUnmoveExit, Reg};

/// The second view's field of view: the frame's over this.
const ZOOM: f32 = 4.0;
/// The picture's distance from the frame's top right corner, pixels.
const MARGIN: u32 = 16;
/// Frames a line of times sums.
const LOGGED_FRAMES: u32 = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asked {
    Off,
    Timing,
    Second(u32),
}

fn asked(value: &[u8]) -> Asked {
    match value {
        b"on" => Asked::Second(512),
        b"256" => Asked::Second(256),
        b"512" => Asked::Second(512),
        b"1024" => Asked::Second(1024),
        b"time" => Asked::Timing,
        _ => Asked::Off,
    }
}

#[derive(Clone, Copy)]
enum Probe {
    Unread,
    Off,
    /// Frame times only.
    Timing,
    /// The render thread makes the view at its next frame.
    Wanted(u32),
    /// The view drawn each frame; the main view of the last frame queued.
    Made { second: SecondView, main: Option<MainView> },
    Failed,
}

static PROBE: Lock<Probe> = Lock::new(Probe::Unread);

/// Whether the render thread is in the second view's draw (renderer::render
/// is not reentrant: set at its start, read by its sites).
static SECOND_PASS: AtomicBool = AtomicBool::new(false);

/// The render thread's times.
struct Clock {
    gpu: Option<GpuClock>,
    frame: u32,
    second_started: Option<f64>,
    main_started: Option<f64>,
    /// The frame's second view was drawn (its GPU interval begun).
    second_this_frame: bool,
    last_present: Option<f64>,
    sums: Sums,
}

#[derive(Clone, Copy, Default)]
struct Sums {
    frames: u32,
    second_cpu: f64,
    main_cpu: f64,
    interval: f64,
    gpu_frames: u32,
    second_gpu: f64,
    main_gpu: f64,
}

static CLOCK: Lock<Clock> = Lock::new(Clock {
    gpu: None,
    frame: 0,
    second_started: None,
    main_started: None,
    second_this_frame: false,
    last_present: None,
    sums: Sums { frames: 0, second_cpu: 0.0, main_cpu: 0.0, interval: 0.0, gpu_frames: 0, second_gpu: 0.0, main_gpu: 0.0 },
});

fn timed(probe: Probe) -> bool {
    matches!(probe, Probe::Timing | Probe::Made { .. })
}

/// The inverse of an affine row-vector transform (rows right, up, forward,
/// place): the view matrix of a camera.
fn invert_affine(m: &[f32; 16]) -> [f32; 16] {
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

/// The frame's projection narrowed `zoom` times, square, without the frame's
/// jitter; its depth range kept.
fn narrowed(projection: &[f32; 16], zoom: f32) -> [f32; 16] {
    let mut p = *projection;
    let y = projection[5] * zoom;
    p[0] = y;
    p[5] = y;
    p[8] = 0.0;
    p[9] = 0.0;
    p
}

/// game::tick before it draws its scene: the second view's draw queued first.
pub fn on_draw(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let probe = PROBE.with(thread, |probe| {
        if let Probe::Unread = probe {
            let mut room = [0u8; 16];
            let len = get_environment(thread, c"SKHID_PIP_PROBE", &mut room);
            *probe = match asked(&room[..len]) {
                Asked::Off => Probe::Off,
                Asked::Timing => Probe::Timing,
                Asked::Second(size) => Probe::Wanted(size),
            };
            if !matches!(probe, Probe::Off) {
                log::line(&[Piece::Text("pip: on, "), Piece::Text(core::str::from_utf8(&room[..len]).unwrap_or("?"))]);
            }
        }
        *probe
    });
    let Probe::Made { second, .. } = probe else { return Flow::Continue };
    let Some(frame) = Frame::at(ctx) else { return Flow::Continue };
    let (camera, projection) = (frame.camera(), frame.projection());
    let view = invert_affine(&camera);
    frame.draw_second(&second, &view, &narrowed(&projection, ZOOM), &view, &projection);
    let main = frame.main_view();
    PROBE.with(thread, |probe| {
        if let Probe::Made { main: kept, .. } = probe {
            *kept = main;
        }
    });
    Flow::Continue
}

/// renderer::render's start: which view it draws.
pub fn on_render_begin(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let probe = PROBE.with(thread, |probe| *probe);
    let second = match probe {
        Probe::Made { second, main, .. } if second_view::drawn_view(ctx, &second) => {
            if let Some(main) = main {
                second_view::match_environment(thread, &second, main);
            }
            true
        }
        _ => false,
    };
    SECOND_PASS.store(second, Ordering::Relaxed);
    if timed(probe) {
        let now = second_view::cpu_ms(thread);
        CLOCK.with(thread, |clock| {
            let slot = clock.frame as usize % RING;
            if second {
                clock.second_this_frame = true;
                clock.second_started = Some(now);
                if let Some(gpu) = &mut clock.gpu {
                    gpu.begin(slot);
                    gpu.stamp(slot, 0);
                }
            } else {
                clock.main_started = Some(now);
                if let Some(gpu) = &mut clock.gpu {
                    if !clock.second_this_frame {
                        gpu.begin(slot);
                    }
                    gpu.stamp(slot, 2);
                }
            }
        });
    }
    Flow::Continue
}

/// The frame's call of render_flash_ui: none in the second view's draw (no
/// interface there, nor anyone's picture of it).
pub fn on_flash_ui_call(orig: &OrigRenderFlashUi, renderer: u32, window: u32) {
    if !SECOND_PASS.load(Ordering::Relaxed) {
        orig.call(renderer, window);
    }
}

/// The frame's scene on its back buffer, before the interface: the view made
/// when asked, its picture copied into the corner.
pub fn on_interface(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let probe = PROBE.with(thread, |probe| {
        if let Probe::Wanted(size) = *probe {
            *probe = match second_view::make(thread, size) {
                Ok(second) => {
                    log::line(&[Piece::Text("pip: second view made, "), Piece::Dec(size)]);
                    Probe::Made { second, main: None }
                }
                Err(why) => {
                    log::line(&[Piece::Text("pip: no second view: "), Piece::Text(match why {
                        second_view::MakeError::Window => "no window",
                        second_view::MakeError::Memory => "no memory",
                        second_view::MakeError::Constructed => "not constructed",
                    })]);
                    Probe::Failed
                }
            };
        }
        *probe
    });
    if timed(probe) {
        CLOCK.with(thread, |clock| {
            if clock.gpu.is_none() {
                clock.gpu = GpuClock::make(ctx);
                if clock.gpu.is_none() {
                    log::line(&[Piece::Text("pip: no GPU timestamps")]);
                }
            }
        });
    }
    if let Probe::Made { second, .. } = probe {
        second_view::composite(ctx, &second, MARGIN);
    }
    Flow::Continue
}

/// renderer::render before its Present: none for the second view (its
/// window is never shown), its answer S_OK; the times of either draw.
pub fn on_present(ctx: &mut Context<'_>) -> Flow<PipScopesOnPresentExit> {
    let thread = ctx.thread();
    let second = SECOND_PASS.load(Ordering::Relaxed);
    let probe = PROBE.with(thread, |probe| *probe);
    if timed(probe) {
        let now = second_view::cpu_ms(thread);
        CLOCK.with(thread, |clock| tick(clock, second, now));
    }
    if second {
        ctx.set_reg(Reg::Eax, 0);
        Flow::Exit(PipScopesOnPresentExit::Kept)
    } else {
        Flow::Continue
    }
}

fn tick(clock: &mut Clock, second: bool, now: f64) {
    let slot = clock.frame as usize % RING;
    if second {
        if let Some(started) = clock.second_started.take() {
            clock.sums.second_cpu += now - started;
        }
        if let Some(gpu) = &mut clock.gpu {
            gpu.stamp(slot, 1);
        }
        return;
    }
    if let Some(started) = clock.main_started.take() {
        clock.sums.main_cpu += now - started;
    }
    if let Some(last) = clock.last_present.replace(now) {
        clock.sums.interval += now - last;
        clock.sums.frames += 1;
    }
    if let Some(gpu) = &mut clock.gpu {
        gpu.stamp(slot, 3);
        gpu.end(slot);
        // the oldest frame in flight
        let oldest = (clock.frame as usize + 1) % RING;
        if clock.frame >= RING as u32 {
            if let Some(marks) = gpu.read(oldest) {
                add_gpu(&mut clock.sums, &marks);
            }
        }
    }
    clock.second_this_frame = false;
    clock.frame = clock.frame.wrapping_add(1);
    if clock.sums.frames >= LOGGED_FRAMES {
        report(&clock.sums);
        clock.sums = Sums::default();
    }
}

fn add_gpu(sums: &mut Sums, marks: &Marks) {
    if let (Some(begin), Some(end)) = (marks[2], marks[3]) {
        sums.main_gpu += end - begin;
        sums.gpu_frames += 1;
        if let (Some(b), Some(e)) = (marks[0], marks[1]) {
            sums.second_gpu += e - b;
        }
    }
}

fn us(total: f64, count: u32) -> u32 {
    if count == 0 { 0 } else { (total * 1000.0 / f64::from(count)) as u32 }
}

fn report(sums: &Sums) {
    log::line(&[
        Piece::Text("pip: frames "),
        Piece::Dec(sums.frames),
        Piece::Text(", us a frame: interval "),
        Piece::Dec(us(sums.interval, sums.frames)),
        Piece::Text(", main cpu "),
        Piece::Dec(us(sums.main_cpu, sums.frames)),
        Piece::Text(" gpu "),
        Piece::Dec(us(sums.main_gpu, sums.gpu_frames)),
        Piece::Text(", second cpu "),
        Piece::Dec(us(sums.second_cpu, sums.frames)),
        Piece::Text(" gpu "),
        Piece::Dec(us(sums.second_gpu, sums.gpu_frames)),
        Piece::Text(" ("),
        Piece::Dec(sums.gpu_frames),
        Piece::Text(" gpu frames)"),
    ]);
}

/// renderer::render's end: the scene's moved models are the frame's own
/// draw's to forget (its motion vectors and shadow caches), never ours.
pub fn on_unmove(_: &mut Context<'_>) -> Flow<PipScopesOnUnmoveExit> {
    if SECOND_PASS.load(Ordering::Relaxed) { Flow::Exit(PipScopesOnUnmoveExit::Kept) } else { Flow::Continue }
}

/// stage_forward::execute_foreground: no first-person models in the second view.
pub fn on_foreground(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if SECOND_PASS.load(Ordering::Relaxed) {
        second_view::drop_foreground(ctx);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_camera_and_its_view_undo_each_other() {
        // turned 90 degrees about up, at (1, 2, 3)
        let camera = [0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 1.0];
        let view = invert_affine(&camera);
        // the camera's place goes to the origin
        let p = [1.0f32, 2.0, 3.0];
        let x: [f32; 3] = core::array::from_fn(|c| p[0] * view[c] + p[1] * view[4 + c] + p[2] * view[8 + c] + view[12 + c]);
        assert!(x.iter().all(|v| v.abs() < 1e-6), "{x:?}");
    }

    #[test]
    fn the_narrowed_projection_is_square_and_unjittered() {
        let mut p = [0.0f32; 16];
        p[0] = 0.75;
        p[5] = 1.0;
        p[8] = 0.01;
        p[10] = 1.0;
        p[11] = 1.0;
        p[14] = -0.1;
        let n = narrowed(&p, 4.0);
        assert_eq!((n[0], n[5], n[8], n[10], n[14]), (4.0, 4.0, 0.0, 1.0, -0.1));
    }
}
