//! Patch 198: picture-in-picture scopes (docs/notes/pip-scopes.md). With the
//! setting `srp_pip_scopes` (or `SKHID_PIP_SCOPES=on`) the eyes' magnified
//! optic is not swapped for its aimed model: the real gun, hands and optic
//! stay (`scope_draw.*`), the frame keeps uro_fov, and the optic's glass shows
//! the scene drawn a second time from the aim, magnified, with the aimed
//! model's reticle. Off: today's scopes exactly. Diagnostics:
//! `SKHID_PIP_LOG=on|picture`; `SKHID_PIP_PROBE=256|512|1024|time|disc`, the
//! second view without a scope (in the frame's corner, timed only, or as a
//! centred lens).
//!
//! The draws are queued on the game's thread (`game.draw_scene`); the
//! render thread knows the second view's draw by its scene view
//! (`render.begin`), gives it the frame's weather, leaves the first-person
//! surfaces, the interface and the Present out of it, and keeps the scene's
//! moved models for the frame's own draw (`render.unmove`). Display only:
//! nothing a match computes.

#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::vostok::log::{self, Piece};
use crate::vostok::renderer::get_environment;
use crate::hud_options;
use crate::pip_lens;
use crate::vostok::second_view::{self, Disc, Frame, GpuClock, Keep, Lens, LensPlace, MainView, Marks, ModelId, ReadError, Put, ScopeSite, ScopeWeapon, Scoped, SecondView, Stage, Stage22, UnknownModel, RING};
use crate::vostok::{
    trig, Context, Flow, Lock, NoExit, OrigRenderFlashUi, PipScopesOnPresentExit, PipScopesOnScopeBodyExit,
    PipScopesOnScopeCulledBodyExit, PipScopesOnScopeOpticExit, PipScopesOnScopeShowExit, PipScopesOnScopeSwapExit, PipScopesOnUnmoveExit, Reg, Thread,
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
/// The lens probe's optic: a full scope's zoom (the PSO-1's, the ACOG's).
const PROBE_FOV_FACTOR: f32 = 0.25;
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

/// The picture a large lens is drawn from: made the first time a lens is
/// wider on the screen than [`SCOPE_SIZE`] pixels, kept after.
#[derive(Clone, Copy)]
enum Large {
    Unwanted,
    Wanted,
    Made(SecondView),
    Failed,
}

static LARGE: Lock<Large> = Lock::new(Large::Unwanted);

/// The large picture's size, pixels square: the most a lens draws from.
const LARGE_SIZE: u32 = 1024;

/// The frame's back buffer's height, pixels (render thread, for the game's).
static FRAME_HEIGHT: Lock<u32> = Lock::new(1080);

/// The view the render thread draws now, of the second views (its picture the lens's).
static PICTURE: Lock<Option<SecondView>> = Lock::new(None);

/// SKHID_PIP_SCOPES set: the lens whatever the setting says.
static SCOPES_FORCED: AtomicBool = AtomicBool::new(false);

/// SKHID_PIP_LOG=on: what the lens does at each aim, in the log.
static LOG_ON: AtomicBool = AtomicBool::new(false);

/// The aim the game's thread and the render thread last logged (an aim is
/// its aimed model in the scene: logged once each).
static NOTED_AIM: Lock<Option<ModelId>> = Lock::new(None);
static SEEN_AIM: Lock<Option<(ModelId, u32)>> = Lock::new(None);

/// SKHID_PIP_LOG=picture: the log, and the second view's picture copied into the
/// frame's top right corner while the lens is drawn.
static PICTURE_ON: AtomicBool = AtomicBool::new(false);

/// The second view's passes with a lens, for one log line of its lists.
static SECOND_SEEN: AtomicU32 = AtomicU32::new(0);

/// A log line of the lens's, built in place.
struct Note {
    pieces: [Piece<'static>; 80],
    len: usize,
}

impl Note {
    fn new(text: &'static str) -> Note {
        let mut note = Note { pieces: [Piece::Text(""); 80], len: 0 };
        note.text(text);
        note
    }

    fn text(&mut self, text: &'static str) -> &mut Note {
        if let Some(slot) = self.pieces.get_mut(self.len) {
            *slot = Piece::Text(text);
            self.len += 1;
        }
        self
    }

    fn dec(&mut self, value: u32) -> &mut Note {
        if let Some(slot) = self.pieces.get_mut(self.len) {
            *slot = Piece::Dec(value);
            self.len += 1;
        }
        self
    }

    fn hex(&mut self, value: u32) -> &mut Note {
        if let Some(slot) = self.pieces.get_mut(self.len) {
            *slot = Piece::Hex(value);
            self.len += 1;
        }
        self
    }

    /// A float as signed thousandths.
    fn milli(&mut self, value: f32) -> &mut Note {
        let m = value * 1000.0;
        self.text(if m < 0.0 { " -" } else { " " });
        self.dec(if m.is_finite() { m.abs().min(4e9) as u32 } else { u32::MAX })
    }

    fn point(&mut self, p: [f32; 3]) -> &mut Note {
        self.text(" (").milli(p[0]).milli(p[1]).milli(p[2]).text(" )")
    }

    /// Into the log with SKHID_PIP_LOG=on.
    fn send(&self) {
        if LOG_ON.load(Ordering::Relaxed) {
            log::line(&self.pieces[..self.len]);
        }
    }
}

/// The lens is on: the setting (or the variable), and the view made for it.
fn scopes_active(thread: &Thread, probe: Probe) -> bool {
    matches!(probe, Probe::Made { used: Use::Scopes, .. })
        && (SCOPES_FORCED.load(Ordering::Relaxed) || hud_options::pip_scopes_on(thread))
}

/// The held magnified optic this frame (update_camera to the draw).
static SCOPED: Lock<Option<Scoped>> = Lock::new(None);

/// The class of the last aimed optic the lens could not read (logged once).
static UNREAD: Lock<Option<u32>> = Lock::new(None);

/// The optic's aimed model while the lens is drawn: the second view draws it
/// alone of the first-person models, the frame all but it.
static AIMED: Lock<Option<ModelId>> = Lock::new(None);

/// The second view was drawn for a lens this frame: the frame's draw shows it.
static LENS_DUE: AtomicBool = AtomicBool::new(false);

enum LensState {
    Unmade,
    Made(Lens),
    Failed,
}

static LENS: Lock<LensState> = Lock::new(LensState::Unmade);

/// The switches the second view's draw turns off, as it found them: put back
/// after it (at its Present, or the next draw's start should it end early).
#[derive(Clone, Copy)]
struct Kept {
    hiz: bool,
    ambient_occlusion: bool,
    reflections: bool,
    sun_shadows: bool,
    eye_reset: bool,
}

static KEPT: Lock<Option<Kept>> = Lock::new(None);

fn restore_switches(thread: &Thread) {
    if let Some(kept) = KEPT.with(thread, Option::take) {
        second_view::set_hiz(thread, kept.hiz);
        second_view::set_stage(thread, Stage::AmbientOcclusion, kept.ambient_occlusion);
        second_view::set_stage(thread, Stage::Reflections, kept.reflections);
        second_view::set_stage(thread, Stage::SunShadows, kept.sun_shadows);
        second_view::eye_reset(thread, Some(kept.eye_reset));
    }
}

fn turn_off_switches(thread: &Thread) {
    let kept = Kept {
        hiz: second_view::set_hiz(thread, false),
        ambient_occlusion: second_view::set_stage(thread, Stage::AmbientOcclusion, false),
        reflections: second_view::set_stage(thread, Stage::Reflections, false),
        // the frame's cascades of the last draw, from the same eye, cover the second view's narrower frustum
        sun_shadows: second_view::set_stage(thread, Stage::SunShadows, false),
        eye_reset: second_view::eye_reset(thread, Some(false)),
    };
    KEPT.with(thread, |slot| *slot = Some(kept));
}

/// The frame's view and projection, for the lens's place on its screen.
static FRAME_CAMERA: Lock<Option<([f32; 16], [f32; 16])>> = Lock::new(None);

/// The lens on the frame's screen this frame (clip space: centre, radii).
static LENS_DISC: Lock<Option<LensPlace>> = Lock::new(None);

/// The aimed model's lens surface the last frame found: the second view
/// leaves it out (its glass would cover the whole picture).
static LENS_SURFACE: Lock<Option<u32>> = Lock::new(None);

/// The radius of the lens's last place, of the half height, for the next
/// second view's field of view.
static LENS_SIZE: Lock<f32> = Lock::new(LENS_RADIUS);

/// The render thread's times.
struct Clock {
    gpu: Option<GpuClock>,
    frame: u32,
    second_started: Option<f64>,
    main_started: Option<f64>,
    /// The frame's second view was drawn (its GPU interval begun).
    second_this_frame: bool,
    last_present: Option<f64>,
    /// The frame's Present began (its end is renderer::render's end, `render.unmove`).
    present_started: Option<f64>,
    /// The game's thread's last scene draw.
    last_game: Option<f64>,
    sums: Sums,
    /// GPU reads that failed, logged up to 8.
    misses: u32,
}

#[derive(Clone, Copy, Default)]
struct Sums {
    frames: u32,
    /// The frame's Present, its call to its return: a wait for the display shows here.
    present: f64,
    /// The game's thread, from one scene's draw to the next.
    game: f64,
    game_frames: u32,
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
    present_started: None,
    last_game: None,
    misses: 0,
    sums: Sums { frames: 0, present: 0.0, game: 0.0, game_frames: 0, second_cpu: 0.0, main_cpu: 0.0, interval: 0.0, gpu_frames: 0, second_gpu: 0.0, main_gpu: 0.0 },
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
            let mut c = [0u8; 8];
            let lc = get_environment(thread, c"SKHID_PIP_LOG", &mut c);
            LOG_ON.store(matches!(&c[..lc], b"on" | b"picture"), Ordering::Relaxed);
            PICTURE_ON.store(&c[..lc] == b"picture", Ordering::Relaxed);
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

/// The vertical field of view the frame was last drawn with, degrees (game thread).
static FRAME_FOV: Lock<f32> = Lock::new(65.0);

fn set_frame_fov(thread: &Thread, projection: &[f32; 16]) {
    if let Some(half) = trig::atan2(1.0, projection[5]) {
        FRAME_FOV.with(thread, |fov| *fov = 2.0 * half * 180.0 / core::f32::consts::PI);
    }
}

/// The frame's vertical field of view this frame, degrees: uro_fov while the lens
/// is on (the scope's zoom is in the lens only), the game's zoomed one otherwise.
/// For other patches that scale with what the eyes see (the camera trail);
/// the camera's zoom (+94h) stays the scope's either way.
pub fn frame_field_of_view(thread: &Thread) -> f32 {
    FRAME_FOV.with(thread, |fov| *fov)
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
    if timed(probe) {
        let now = second_view::cpu_ms(thread);
        CLOCK.with(thread, |clock| {
            if let Some(last) = clock.last_game.replace(now) {
                clock.sums.game += now - last;
                clock.sums.game_frames += 1;
            }
        });
    }
    let scoped = SCOPED.with(thread, Option::take);
    // nothing made before the first aim through a magnified optic
    if matches!(probe, Probe::Off) && scoped.is_some() && hud_options::pip_scopes_on(thread) {
        probe = Probe::Wanted(Use::Scopes, SCOPE_SIZE);
        PROBE.with(thread, |kept| *kept = probe);
    }
    let scoped = scoped.filter(|_| scopes_active(thread, probe));
    // an optic the lens does not take is today's scope (lens_takes logs why)
    let scoped = scoped.filter(|scoped| matches!(scoped.aimed, Some(Ok(model)) if !refused(thread, model)));
    let Probe::Made { second, used, .. } = probe else { return Flow::Continue };
    let Some(frame) = Frame::at(ctx) else { return Flow::Continue };
    let (camera, projection) = (frame.camera(), frame.projection());
    let view = invert_affine(&camera);
    let main = frame.main_view();
    set_frame_fov(thread, &projection);
    let drawn = match used {
        Use::Probe => {
            frame.draw_second(&second, &view, &narrowed(&projection, ZOOM), &view, &projection);
            None
        }
        // as a full scope's lens (fov_factor 0.25) of the probe's fixed size: today's magnification
        Use::ProbeLens => {
            let uro_fov = second_view::field_of_view(thread);
            if let Some(lens) = through_lens(&projection, PROBE_FOV_FACTOR, uro_fov, LENS_RADIUS) {
                frame.draw_second(&second, &view, &lens, &view, &projection);
            }
            None
        }
        Use::Scopes => {
            let Some(scoped) = scoped else {
                AIMED.with(thread, |aimed| *aimed = None);
                return Flow::Continue;
            };
            let uro_fov = second_view::field_of_view(thread);
            let size = LENS_SIZE.with(thread, |size| *size);
            let (Some(main_projection), Some(lens)) =
                (unzoomed(&projection, uro_fov), through_lens(&projection, scoped.fov_factor, uro_fov, size))
            else {
                return Flow::Continue;
            };
            frame.set_projection(&main_projection);
            set_frame_fov(thread, &main_projection);
            FRAME_CAMERA.with(thread, |kept| *kept = Some((view, main_projection)));
            // the picture as wide as the lens on the screen: the large one past SCOPE_SIZE pixels
            let wide = size * FRAME_HEIGHT.with(thread, |h| *h) as f32 > SCOPE_SIZE as f32 * 1.1;
            let large = LARGE.with(thread, |large| match *large {
                Large::Unwanted if wide => {
                    *large = Large::Wanted;
                    None
                }
                Large::Made(view) if wide => Some(view),
                _ => None,
            });
            let picture = large.unwrap_or(second);
            let shown = scoped.scope_view && reticle_of(thread, scoped.weapon);
            if shown {
                // from the aim itself: the reticle's centre is where a shot goes
                frame.draw_second(&picture, &invert_affine(&scoped.eye), &lens, &view, &main_projection);
            }
            let aimed = scoped.aimed.and_then(Result::ok).filter(|_| shown);
            if NOTED_AIM.with(thread, |noted| core::mem::replace(noted, aimed)) != aimed
                && let Some(model) = aimed
            {
                let fov = trig::atan2(1.0, lens[5]).map_or(0.0, |half| 2.0 * half * 180.0 / core::f32::consts::PI);
                Note::new("pip: aim (thousandths): optic fov_factor")
                    .milli(scoped.fov_factor)
                    .text(", aimed model ")
                    .text(model.kind().name())
                    .text(" ")
                    .hex(model.address())
                    .text(", idle model ")
                    .hex(scoped.idle.unwrap_or(0))
                    .text(", uro_fov")
                    .milli(uro_fov)
                    .text(", last lens radius (half heights)")
                    .milli(size)
                    .text(", second view fov (degrees)")
                    .milli(fov)
                    .text(", its picture ")
                    .dec(SCOPE_SIZE)
                    .text(" px square")
                    .send();
            }
            aimed
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
    let large = LARGE.with(thread, |large| match *large {
        Large::Made(view) => Some(view),
        _ => None,
    });
    let drawn = match probe {
        Probe::Made { second, main, .. } => [Some(second), large]
            .into_iter()
            .flatten()
            .find(|view| second_view::drawn_view(ctx, view))
            .map(|view| (view, main)),
        _ => None,
    };
    if let Some((view, main)) = drawn {
        if let Some(main) = main {
            second_view::match_environment(thread, &view, main);
        }
        PICTURE.with(thread, |picture| *picture = Some(view));
        LENS_DUE.store(true, Ordering::Relaxed);
    }
    let second = drawn.is_some();
    SECOND_PASS.store(second, Ordering::Relaxed);
    restore_switches(thread);
    if second {
        turn_off_switches(thread);
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
    LARGE.with(thread, |large| {
        if let Large::Wanted = *large {
            *large = match second_view::make(thread, LARGE_SIZE) {
                Ok(view) => {
                    log::line(&[Piece::Text("pip: second view made, "), Piece::Dec(LARGE_SIZE), Piece::Text(" (a large lens)")]);
                    Large::Made(view)
                }
                Err(_) => {
                    log::line(&[Piece::Text("pip: no large second view: the lens draws from the 512 one")]);
                    Large::Failed
                }
            };
        }
    });
    if let Some(height) = second_view::frame_height(ctx) {
        FRAME_HEIGHT.with(thread, |kept| *kept = height);
    }
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
        Probe::Made { second, used: used @ (Use::Scopes | Use::ProbeLens), .. } if LENS_DUE.swap(false, Ordering::Relaxed) => {
            let second = PICTURE.with(thread, Option::take).unwrap_or(second);
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
                    // the eyepiece's own place this frame; the probe's, at the centre
                    let (centre, radius) = match LENS_DISC.with(thread, Option::take) {
                        Some(place) => (place.centre, place.radius),
                        None if used == Use::ProbeLens => ([0.0, 0.0], [LENS_RADIUS / aspect, LENS_RADIUS]),
                        None => return,
                    };
                    LENS_SIZE.with(thread, |size| *size = radius[1]);
                    lens.draw(ctx, &second, &Disc { centre, radius, rim: [RIM_SHARPNESS, RIM_DARKNESS] });
                    if PICTURE_ON.load(Ordering::Relaxed) {
                        second_view::composite(ctx, &second, MARGIN);
                    }
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
        restore_switches(thread);
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
    clock.present_started = Some(now);
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
        Piece::Text(", present "),
        Piece::Dec(us(sums.present, sums.frames)),
        Piece::Text(", game thread "),
        Piece::Dec(us(sums.game, sums.game_frames)),
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
pub fn on_unmove(ctx: &mut Context<'_>) -> Flow<PipScopesOnUnmoveExit> {
    let thread = ctx.thread();
    if !SECOND_PASS.load(Ordering::Relaxed) && timed(PROBE.with(thread, |probe| *probe)) {
        let now = second_view::cpu_ms(thread);
        CLOCK.with(thread, |clock| {
            if let Some(started) = clock.present_started.take() {
                clock.sums.present += now - started;
            }
        });
    }
    if SECOND_PASS.load(Ordering::Relaxed) { Flow::Exit(PipScopesOnUnmoveExit::Kept) } else { Flow::Continue }
}

/// stage_forward::execute_foreground: the second view draws no first-person
/// model but the lens's aimed model (the probe, none), the frame all but it.
pub fn on_foreground(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let aimed = AIMED.with(ctx.thread(), |aimed| *aimed);
    match (SECOND_PASS.load(Ordering::Relaxed), aimed) {
        (true, Some(aimed)) => {
            let except = LENS_SURFACE.with(ctx.thread(), |kept| *kept);
            let filtered = second_view::filter_foreground(ctx, Stage22::Forward, Keep::Only { model: aimed, except });
            if SECOND_SEEN.fetch_add(1, Ordering::Relaxed) == 30 {
                let mut note = Note::new("pip: second view's first-person lists (forward stage): forward");
                note.dec(filtered.foreground.0).text(" -> ").dec(filtered.foreground.1).text(", lit ").dec(filtered.lit.0).text(" -> ").dec(filtered.lit.1);
                note.text(" (").dec(filtered.lit_first_person).text(" first-person, of owners");
                for owner in filtered.owners.iter().filter(|&&o| o != 0) {
                    note.text(" ").hex(*owner);
                }
                note.text(")").send();
            }
        }
        (true, None) => second_view::drop_foreground(ctx),
        (false, Some(aimed)) => {
            let thread = ctx.thread();
            let camera = FRAME_CAMERA.with(thread, |camera| *camera);
            let disc = camera.and_then(|(view, projection)| second_view::lens_on_screen(aimed, &view, &projection));
            LENS_DISC.with(thread, |kept| *kept = disc);
            if let Some(place) = disc {
                LENS_SURFACE.with(thread, |kept| *kept = Some(place.surface));
            }
            // an aim's first frame and its thirtieth (its surfaces placed by then)
            let frame = SEEN_AIM.with(thread, |seen| {
                let frame = match *seen {
                    Some((model, frame)) if model == aimed => frame + 1,
                    _ => 0,
                };
                *seen = Some((aimed, frame));
                frame
            });
            if frame == 0 || frame == 30 {
                log_surfaces(ctx, aimed, camera, disc);
            }
            // its spheres are placed a frame or two after it enters the scene
            if disc.is_none() && frame >= 30 {
                refuse(thread, aimed);
            }
            let _ = second_view::filter_foreground(ctx, Stage22::Forward, Keep::AllBut(aimed));
        }
        (false, None) => SEEN_AIM.with(ctx.thread(), |seen| *seen = None),
    }
    Flow::Continue
}

/// The aimed model's surfaces and the lens's place, at an aim's first frame.
fn log_surfaces(ctx: &Context<'_>, aimed: ModelId, camera: Option<([f32; 16], [f32; 16])>, disc: Option<LensPlace>) {
    if !LOG_ON.load(Ordering::Relaxed) {
        return;
    }
    let Some((view, _)) = camera else {
        Note::new("pip: aim: no frame camera").send();
        return;
    };
    let (seen, count) = second_view::surfaces_seen(aimed, &view);
    Note::new("pip: aim: model ").hex(aimed.address()).text(" ").text(aimed.kind().name()).text(", surfaces ").dec(count as u32).send();
    for (index, surface) in seen.iter().take(count).enumerate() {
        let mut note = Note::new("pip:   surface ");
        note.dec(index as u32).text(" radius").milli(surface.radius).text(", centre").point(surface.stored);
        note.text(", in view").point(surface.in_view);
        if let Some((vertices, indices)) = surface.mesh {
            note.text(", mesh ").dec(vertices).text("/").dec(indices);
        }
        match surface.placed {
            Some(at) => note.text(", matrix at").point(at),
            None => note.text(", no matrix"),
        };
        note.send();
    }
    let (list, listed) = second_view::foreground_seen(ctx, aimed);
    Note::new("pip: aim: the frame's first-person list, ").dec(listed as u32).text(" shown").send();
    for entry in list.iter().take(listed) {
        let mut note = Note::new("pip:   entry ");
        note.hex(entry.address).text(if entry.aimed { " aimed" } else { " other" }).text(" sphere");
        note.point([entry.sphere[0], entry.sphere[1], entry.sphere[2]]).text(" r").milli(entry.sphere[3]);
        note.text(" surface ").hex(entry.surface).text(" owner ").hex(entry.owner);
        match entry.placed {
            Some(at) => note.text(", matrix at").point(at),
            None => note.text(", no matrix"),
        };
        note.send();
    }
    match disc {
        Some(place) => Note::new("pip: aim: lens at (clip)")
            .milli(place.centre[0])
            .milli(place.centre[1])
            .text(", radii")
            .milli(place.radius[0])
            .milli(place.radius[1])
            .text(", surface ")
            .hex(place.surface)
            .send(),
        None => Note::new("pip: aim: no glass (a mesh of 61 vertices, 180 indices) placed on the aimed model: no lens").send(),
    }
}

/// stage_gbuffer::execute before it copies its list: the same first-person
/// surfaces left out of the G-buffer (the guns', hands' and optics' opaque
/// ones are drawn there) as at the foreground stage.
pub fn on_gbuffer(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(aimed) = AIMED.with(thread, |aimed| *aimed) else {
        return Flow::Continue;
    };
    if SECOND_PASS.load(Ordering::Relaxed) {
        let except = LENS_SURFACE.with(thread, |kept| *kept);
        let filtered = second_view::filter_foreground(ctx, Stage22::Gbuffer, Keep::Only { model: aimed, except });
        if SECOND_SEEN.load(Ordering::Relaxed) == 30 {
            let mut note = Note::new("pip: second view's G-buffer list ");
            note.dec(filtered.lit.0).text(" -> ").dec(filtered.lit.1).text(" (").dec(filtered.lit_first_person).text(" first-person, of owners");
            for owner in filtered.owners.iter().filter(|&&o| o != 0) {
                note.text(" ").hex(*owner);
            }
            note.text(")").send();
        }
    } else {
        let _ = second_view::filter_foreground(ctx, Stage22::Gbuffer, Keep::AllBut(aimed));
    }
    Flow::Continue
}

fn lens_on(thread: &Thread) -> bool {
    scopes_active(thread, PROBE.with(thread, |probe| *probe))
}

/// The aimed model of the lens's weapon: in the scene while the lens is
/// drawn (put there by the lens, the game's swap skipped for that weapon),
/// for the second view's reticle; the frame leaves it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reticle {
    Out,
    In { weapon: u32 },
}

static RETICLE: Lock<Reticle> = Lock::new(Reticle::Out);

/// Aimed models the lens found no glass on: today's scope for them (logged once each).
static REFUSED: Lock<([u32; 16], usize)> = Lock::new(([0; 16], 0));

fn refused(thread: &Thread, model: ModelId) -> bool {
    REFUSED.with(thread, |(list, count)| list[..*count].contains(&model.address()))
}

fn refuse(thread: &Thread, model: ModelId) {
    let fresh = REFUSED.with(thread, |(list, count)| {
        if list[..*count].contains(&model.address()) {
            return false;
        }
        if *count < list.len() {
            list[*count] = model.address();
            *count += 1;
        }
        true
    });
    if fresh {
        log::line(&[
            Piece::Text("pip: no glass of the aimed model "),
            Piece::Hex(model.address()),
            Piece::Text(" (no surface of 61 vertices and 180 indices) placed in 30 frames: today's scope for that optic"),
        ]);
    }
}

/// The lens takes this weapon: the eyes' own, a magnified optic, the lens
/// on, and an aimed model of a class it reads whose glass it found. Every
/// other case is today's scope (an unread class said once in the log).
fn lens_takes(thread: &Thread, weapon: &ScopeWeapon<'_>) -> bool {
    if !weapon.magnified() || !lens_on(thread) || !weapon.first_person() {
        return false;
    }
    match weapon.aimed() {
        Some(Ok(model)) => !refused(thread, model),
        Some(Err(UnknownModel(vtable))) => {
            if UNREAD.with(thread, |kept| kept.replace(vtable)) != Some(vtable) {
                log::line(&[Piece::Text("pip: an aimed optic's model of class "), Piece::Hex(vtable), Piece::Text(" is not one the lens reads: today's scope for it")]);
            }
            false
        }
        None => false,
    }
}

/// The reticle model for `weapon` after this frame's draw: in while the
/// lens takes it and its scope's view is on (and the game's own swap is
/// undone: never both in the scene), out otherwise.
fn place_reticle(thread: &Thread, weapon: &ScopeWeapon<'_>, takes: bool) {
    let want = takes && weapon.scope_view() && !weapon.swapped();
    RETICLE.with(thread, |reticle| {
        *reticle = match (*reticle, want) {
            (Reticle::Out, true) if weapon.put_aimed(Put::In) => Reticle::In { weapon: weapon.address() },
            (Reticle::In { weapon: w }, false) if w == weapon.address() => {
                weapon.put_aimed(Put::Out);
                Reticle::Out
            }
            (kept, _) => kept,
        };
    });
}

fn reticle_of(thread: &Thread, weapon: u32) -> bool {
    RETICLE.with(thread, |reticle| *reticle == Reticle::In { weapon })
}

/// weapon::on_show, its scope's view on: the lens's weapon shows its idle optic.
pub fn on_scope_show(ctx: &mut Context<'_>) -> Flow<PipScopesOnScopeShowExit> {
    let Some(weapon) = ScopeWeapon::at(ctx, ScopeSite::Show) else { return Flow::Continue };
    if lens_takes(ctx.thread(), &weapon) { Flow::Exit(PipScopesOnScopeShowExit::Idle) } else { Flow::Continue }
}

/// weapon::draw's body update: the lens's weapon's body is updated (it never leaves the scene).
pub fn on_scope_body(ctx: &mut Context<'_>) -> Flow<PipScopesOnScopeBodyExit> {
    let Some(weapon) = ScopeWeapon::at(ctx, ScopeSite::Body) else { return Flow::Continue };
    if lens_takes(ctx.thread(), &weapon) { Flow::Exit(PipScopesOnScopeBodyExit::Update) } else { Flow::Continue }
}

pub fn on_scope_culled_body(ctx: &mut Context<'_>) -> Flow<PipScopesOnScopeCulledBodyExit> {
    let Some(weapon) = ScopeWeapon::at(ctx, ScopeSite::CulledBody) else { return Flow::Continue };
    if lens_takes(ctx.thread(), &weapon) { Flow::Exit(PipScopesOnScopeCulledBodyExit::Update) } else { Flow::Continue }
}

/// weapon::draw's swap, first person: the lens's weapon takes the other
/// branch (no swap; the game's own aimed model, if in, taken out there) and
/// the lens puts the aimed model in for its reticle.
pub fn on_scope_swap(ctx: &mut Context<'_>) -> Flow<PipScopesOnScopeSwapExit> {
    let thread = ctx.thread();
    let Some(weapon) = ScopeWeapon::at(ctx, ScopeSite::Swap) else { return Flow::Continue };
    let takes = lens_takes(thread, &weapon);
    place_reticle(thread, &weapon, takes);
    if takes { Flow::Exit(PipScopesOnScopeSwapExit::Unscoped) } else { Flow::Continue }
}

/// weapon::draw's optic update: the lens's weapon updates its idle optic
/// (the one in view) and the lens its aimed model (the reticle), both to the
/// weapon's matrix.
pub fn on_scope_optic(ctx: &mut Context<'_>) -> Flow<PipScopesOnScopeOpticExit> {
    let thread = ctx.thread();
    let Some(weapon) = ScopeWeapon::at(ctx, ScopeSite::Optic) else { return Flow::Continue };
    if !lens_takes(thread, &weapon) {
        return Flow::Continue;
    }
    if reticle_of(thread, weapon.address()) {
        weapon.put_aimed(Put::Moved);
    }
    Flow::Exit(PipScopesOnScopeOpticExit::Idle)
}

/// weapon::on_hide: the lens's aimed model leaves the scene with the weapon.
pub fn on_weapon_hidden(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(weapon) = ScopeWeapon::at(ctx, ScopeSite::Hidden) else { return Flow::Continue };
    place_reticle(thread, &weapon, false);
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

