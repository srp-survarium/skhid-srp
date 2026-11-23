//! Patch 198: a second view of the scene each frame, for picture-in-picture
//! scopes (docs/notes/pip-scopes.md). Dormant unless `SKHID_PIP_PROBE` is set
//! in the game's environment (read once, at the first scene drawn):
//!
//! - `256`, `512`, `1024` (or `on`, 512): the scene drawn a second time
//!   each frame, before the frame, from the frame's camera with a quarter of
//!   its field of view, into a square window of that size, and copied into
//!   the frame's top right corner; frame times logged;
//! - `time`: only the frame times logged;
//! - `disc`: 512, drawn at the centre as the lens is (its draw seen without a scope).
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
use crate::hud_options;
use crate::pip_lens;
use crate::vostok::second_view::{self, Disc, Frame, GpuClock, Keep, Lens, MainView, Marks, ModelId, ReadError, Scoped, SecondView, RING};
use crate::vostok::{
    trig, Context, Flow, Lock, NoExit, OrigRenderFlashUi, PipScopesOnAddHideExit, PipScopesOnAddIdleExit, PipScopesOnPresentExit,
    PipScopesOnRemoveIdleExit, PipScopesOnRemoveShowExit, PipScopesOnUnmoveExit, Reg, Thread,
};

/// The second view's field of view: the frame's over this.
const ZOOM: f32 = 4.0;
/// The picture's distance from the frame's top right corner, pixels.
const MARGIN: u32 = 16;
/// Frames a line of times sums.
const LOGGED_FRAMES: u32 = 240;
/// The lens's radius, of the frame's half height.
const LENS_RADIUS: f32 = 0.45;
/// The scope's picture, pixels square, with the setting.
const SCOPE_SIZE: u32 = 512;
/// The lens's rim: how sharp its darkening is, how dark it gets.
const RIM_SHARPNESS: f32 = 12.0;
const RIM_DARKNESS: f32 = 0.2;

/// What the second view is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Use {
    /// Every frame, from the frame's camera, its picture in the corner.
    Probe,
    /// The same, drawn as the lens is (`disc`): the lens's draw seen without a scope.
    ProbeLens,
    /// While a magnified optic is aimed: its lens.
    Scopes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asked {
    Off,
    Timing,
    Second(Use, u32),
}

fn size(value: &[u8]) -> Option<u32> {
    match value {
        b"on" | b"512" => Some(512),
        b"256" => Some(256),
        b"1024" => Some(1024),
        _ => None,
    }
}

fn asked(probe: &[u8], scopes: &[u8]) -> Asked {
    match (size(scopes), probe, size(probe)) {
        (Some(size), _, _) => Asked::Second(Use::Scopes, size),
        (None, _, Some(size)) => Asked::Second(Use::Probe, size),
        (None, b"disc", None) => Asked::Second(Use::ProbeLens, 512),
        (None, b"time", None) => Asked::Timing,
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
    Wanted(Use, u32),
    /// The view made; the main view of the last frame it was queued in.
    Made { second: SecondView, used: Use, main: Option<MainView> },
    Failed,
}

static PROBE: Lock<Probe> = Lock::new(Probe::Unread);

/// Whether the render thread is in the second view's draw (renderer::render
/// is not reentrant: set at its start, read by its sites).
static SECOND_PASS: AtomicBool = AtomicBool::new(false);

/// SKHID_PIP_SCOPES set: the lens whatever the setting says.
static SCOPES_FORCED: AtomicBool = AtomicBool::new(false);

/// The lens is on: the setting (or the variable), and the view made for it.
fn scopes_active(thread: &Thread, probe: Probe) -> bool {
    matches!(probe, Probe::Made { used: Use::Scopes, .. })
        && (SCOPES_FORCED.load(Ordering::Relaxed) || hud_options::pip_scopes_on(thread))
}

/// The held magnified optic this frame (update_camera to the draw).
static SCOPED: Lock<Option<Scoped>> = Lock::new(None);

/// The optic's aimed model while the lens is drawn: the second view draws it
/// alone of the first-person models, the frame all but it.
static AIMED: Lock<Option<ModelId>> = Lock::new(None);

/// The second view was drawn for a lens this frame: the frame's draw shows it.
static LENS_DUE: AtomicBool = AtomicBool::new(false);

/// The weapon (its address) whose aimed model was put in the scene the
/// lens's way: its body, hands and idle optic left in.
static LENS_WEAPON: Lock<Option<u32>> = Lock::new(None);

enum LensState {
    Unmade,
    Made(Lens),
    Failed,
}

static LENS: Lock<LensState> = Lock::new(LensState::Unmade);

/// The hi-Z switch as the second view's draw found it, put back after it.
static HIZ_KEPT: Lock<Option<bool>> = Lock::new(None);

fn restore_hiz(thread: &crate::vostok::Thread) {
    if let Some(was) = HIZ_KEPT.with(thread, Option::take) {
        second_view::set_hiz(thread, was);
    }
}

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
    /// GPU reads that failed, logged up to 8.
    misses: u32,
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
    misses: 0,
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
fn read_probe(thread: &Thread) -> Probe {
    PROBE.with(thread, |probe| {
        if let Probe::Unread = probe {
            let (mut a, mut b) = ([0u8; 16], [0u8; 16]);
            let la = get_environment(thread, c"SKHID_PIP_PROBE", &mut a);
            let lb = get_environment(thread, c"SKHID_PIP_SCOPES", &mut b);
            SCOPES_FORCED.store(size(&b[..lb]).is_some(), Ordering::Relaxed);
            *probe = match asked(&a[..la], &b[..lb]) {
                Asked::Off => Probe::Off,
                Asked::Timing => Probe::Timing,
                Asked::Second(used, size) => Probe::Wanted(used, size),
            };
            if !matches!(probe, Probe::Off) {
                log::line(&[
                    Piece::Text("pip: on, probe "),
                    Piece::Text(core::str::from_utf8(&a[..la]).unwrap_or("?")),
                    Piece::Text(", scopes "),
                    Piece::Text(core::str::from_utf8(&b[..lb]).unwrap_or("?")),
                ]);
            }
        }
        *probe
    })
}

/// The tangent of half of `degrees`.
fn half_tan(degrees: f32) -> Option<f32> {
    let half = degrees * core::f32::consts::PI / 360.0;
    let (s, c) = (trig::sin(half)?, trig::cos(half)?);
    (c > 1e-6).then(|| s / c)
}

/// Patch 22's field of view at zoom `z` (fov_projection): 65 z degrees plus
/// uro_fov's part, which fades out as z goes to a full scope's 0.25.
fn fov_at(z: f32, uro_fov: f32) -> f32 {
    65.0 * z + (uro_fov - 65.0) * ((z - 0.25) / 0.75).max(0.0)
}

/// The frame's projection as at zoom 1 (uro_fov), its aspect, depth range
/// and jitter kept.
fn unzoomed(projection: &[f32; 16], uro_fov: f32) -> Option<[f32; 16]> {
    let y = 1.0 / half_tan(uro_fov)?;
    let aspect = projection[0] / projection[5];
    let mut p = *projection;
    p[5] = y;
    p[0] = y * aspect;
    Some(p)
}

/// The scope's projection: the optic's full zoom, narrowed to the lens (its
/// radius `lens` of the frame's half height), square, unjittered.
fn through_lens(projection: &[f32; 16], fov_factor: f32, uro_fov: f32, lens: f32) -> Option<[f32; 16]> {
    let y = 1.0 / (lens * half_tan(fov_at(fov_factor, uro_fov))?);
    let mut p = *projection;
    p[0] = y;
    p[5] = y;
    p[8] = 0.0;
    p[9] = 0.0;
    Some(p)
}

/// player::update_camera's zoom: the held optic, when magnified, for this
/// frame's draw.
pub fn on_zoom(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let scoped = second_view::scoped(ctx);
    SCOPED.with(ctx.thread(), |kept| *kept = scoped);
    Flow::Continue
}

/// game::tick before it draws its scene: the second view's draw queued first.
pub fn on_draw(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let mut probe = read_probe(thread);
    if matches!(probe, Probe::Off) && hud_options::pip_scopes_on(thread) {
        probe = Probe::Wanted(Use::Scopes, SCOPE_SIZE);
        PROBE.with(thread, |kept| *kept = probe);
    }
    let scoped = SCOPED.with(thread, Option::take).filter(|_| scopes_active(thread, probe));
    let Probe::Made { second, used, .. } = probe else { return Flow::Continue };
    let Some(frame) = Frame::at(ctx) else { return Flow::Continue };
    let (camera, projection) = (frame.camera(), frame.projection());
    let view = invert_affine(&camera);
    let main = frame.main_view();
    let drawn = match used {
        Use::Probe | Use::ProbeLens => {
            frame.draw_second(&second, &view, &narrowed(&projection, ZOOM), &view, &projection);
            None
        }
        Use::Scopes => {
            let Some(scoped) = scoped else {
                AIMED.with(thread, |aimed| *aimed = None);
                return Flow::Continue;
            };
            let uro_fov = second_view::field_of_view(thread);
            let (Some(main_projection), Some(lens)) =
                (unzoomed(&projection, uro_fov), through_lens(&projection, scoped.fov_factor, uro_fov, LENS_RADIUS))
            else {
                return Flow::Continue;
            };
            frame.set_projection(&main_projection);
            if scoped.shown {
                frame.draw_second(&second, &view, &lens, &view, &main_projection);
            }
            scoped.aimed.filter(|_| scoped.shown)
        }
    };
    AIMED.with(thread, |aimed| *aimed = drawn);
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
            LENS_DUE.store(true, Ordering::Relaxed);
            true
        }
        _ => false,
    };
    SECOND_PASS.store(second, Ordering::Relaxed);
    restore_hiz(thread);
    if second {
        let was = second_view::set_hiz(thread, false);
        HIZ_KEPT.with(thread, |kept| *kept = Some(was));
    }
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
        if let Probe::Wanted(used, size) = *probe {
            *probe = match second_view::make(thread, size) {
                Ok(second) => {
                    log::line(&[Piece::Text("pip: second view made, "), Piece::Dec(size)]);
                    Probe::Made { second, used, main: None }
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
    match probe {
        Probe::Made { second, used: Use::Probe, .. } => {
            second_view::composite(ctx, &second, MARGIN);
        }
        Probe::Made { second, used: Use::Scopes | Use::ProbeLens, .. } if LENS_DUE.swap(false, Ordering::Relaxed) => {
            LENS.with(thread, |lens| {
                let lens = match lens {
                    LensState::Unmade => {
                        *lens = match Lens::make(ctx, &pip_lens::VERTEX, &pip_lens::PIXEL) {
                            Some(made) => LensState::Made(made),
                            None => {
                                log::line(&[Piece::Text("pip: no lens shaders")]);
                                LensState::Failed
                            }
                        };
                        lens
                    }
                    made => made,
                };
                if let LensState::Made(lens) = lens {
                    let aspect = second_view::frame_aspect(ctx).unwrap_or(16.0 / 9.0);
                    let disc = Disc { centre: [0.0, 0.0], radius: [LENS_RADIUS / aspect, LENS_RADIUS], rim: [RIM_SHARPNESS, RIM_DARKNESS] };
                    lens.draw(ctx, &second, &disc);
                }
            });
        }
        _ => {}
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
        restore_hiz(thread);
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
            match gpu.read(oldest) {
                Ok(marks) => add_gpu(&mut clock.sums, &marks),
                Err(why) if clock.misses < 8 => {
                    clock.misses += 1;
                    let (a, b, c) = match why {
                        ReadError::NoContext => (0, 0, 0),
                        ReadError::Disjoint { got, disjoint } => (1, got, disjoint),
                        ReadError::Mark { mark, got } => (2 + mark, got, 0),
                    };
                    log::line(&[Piece::Text("pip: gpu read "), Piece::Dec(a), Piece::Text(" "), Piece::Hex(b), Piece::Text(" "), Piece::Dec(c)]);
                }
                Err(_) => {}
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

/// stage_forward::execute_foreground: the second view draws no first-person
/// model but the lens's aimed model (the probe, none), the frame all but it.
pub fn on_foreground(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let aimed = AIMED.with(ctx.thread(), |aimed| *aimed);
    match (SECOND_PASS.load(Ordering::Relaxed), aimed) {
        (true, Some(aimed)) => second_view::filter_foreground(ctx, Keep::Only(aimed)),
        (true, None) => second_view::drop_foreground(ctx),
        (false, Some(aimed)) => second_view::filter_foreground(ctx, Keep::AllBut(aimed)),
        (false, None) => {}
    }
    Flow::Continue
}

fn lens_on(thread: &Thread) -> bool {
    scopes_active(thread, PROBE.with(thread, |probe| *probe))
}

/// add_aimed_rifle_scope_to_scene before it takes the idle optic out: with
/// the lens, the eyes' weapon keeps it.
pub fn on_add_idle(ctx: &mut Context<'_>) -> Flow<PipScopesOnAddIdleExit> {
    let thread = ctx.thread();
    let Some((weapon, eyes)) = second_view::scope_weapon(ctx) else { return Flow::Continue };
    let lens = eyes && lens_on(thread);
    LENS_WEAPON.with(thread, |kept| *kept = lens.then_some(weapon));
    if lens { Flow::Exit(PipScopesOnAddIdleExit::Aimed) } else { Flow::Continue }
}

/// add_aimed_rifle_scope_to_scene before it hides the body and the hands:
/// the lens's weapon keeps them.
pub fn on_add_hide(ctx: &mut Context<'_>) -> Flow<PipScopesOnAddHideExit> {
    if ours(ctx, false) { Flow::Exit(PipScopesOnAddHideExit::Shown) } else { Flow::Continue }
}

/// remove_aimed_rifle_scope_from_scene before it puts the idle optic back:
/// the lens's weapon never took it out.
pub fn on_remove_idle(ctx: &mut Context<'_>) -> Flow<PipScopesOnRemoveIdleExit> {
    if ours(ctx, false) { Flow::Exit(PipScopesOnRemoveIdleExit::Aimed) } else { Flow::Continue }
}

/// remove_aimed_rifle_scope_from_scene before it shows the body and the
/// hands again: the lens's weapon never hid them.
pub fn on_remove_show(ctx: &mut Context<'_>) -> Flow<PipScopesOnRemoveShowExit> {
    if ours(ctx, true) { Flow::Exit(PipScopesOnRemoveShowExit::Shown) } else { Flow::Continue }
}

/// Whether the weapon at this scope site put its aimed model in the lens's
/// way; `done` forgets it (its aimed model is out).
fn ours(ctx: &Context<'_>, done: bool) -> bool {
    let Some((weapon, _)) = second_view::scope_weapon(ctx) else { return false };
    LENS_WEAPON.with(ctx.thread(), |kept| {
        let ours = *kept == Some(weapon);
        if ours && done {
            *kept = None;
        }
        ours
    })
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
