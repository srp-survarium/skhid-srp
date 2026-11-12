//! Patch 83: what DLSS is given of a frame, while the 3D frame is rendered
//! at a render scale (docs/notes/dlss-option.md, "The frame's inputs").
//!
//! - The jitter: a Halton(2,3) offset of the frame's picture, under a
//!   render pixel, in the projection of the whole frame (the engine's own
//!   jitter seam, `render.jittered`).
//! - None of the game's own AA (FXAA and the sharpen that comes with it, the
//!   temporal AA) while it runs.
//! - The engine's object motion pass every frame, whatever its options say,
//!   into a signed target.
//! - What the link reads of the frame ([`frame`]): the jitter and the
//!   camera, this frame's and the one's before, the targets, and whether
//!   DLSS's history is thrown away.
//!
//! One owner ([`OWNER`]) holds which of these are asked for ([`Pieces`])
//! and the frames as they were rendered. The link asks with [`ask`]; until
//! it exists the diagnostics switch `SKHID_DLSS_INPUTS` does, beside
//! `SKHID_DLSS_RENDER_SCALE` (the hooks are the render scale's sites,
//! written only when a scale is asked for).

#![forbid(unsafe_code)]

use super::render_size;
use crate::vostok::files::{File, Path};
use crate::vostok::frame_inputs::{self, EyeRays, JitterSeam, Matrix, MotionFormat, MotionGate, PixelFormat, Pixels, SceneId, Target,
                                  TargetIndex, Targets};
use crate::vostok::log::{Piece::{self, *}, line};
use crate::vostok::render_size::{DepthTexture, Size};
use crate::vostok::renderer::get_environment;
use crate::vostok::{Context, Flow, HudOptionsOnMotionClearExit, HudOptionsOnMotionGateExit, HudOptionsOnMotionQualityExit, Lock,
                    NoExit, Thread, ThreadBound};

/// Which of the frame's inputs are asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pieces(u8);

impl Pieces {
    pub const NONE: Pieces = Pieces(0);
    /// The projection's jitter.
    pub const JITTER: Pieces = Pieces(1);
    /// None of the game's own AA.
    pub const NO_AA: Pieces = Pieces(2);
    /// The object motion pass every frame.
    pub const MOTION: Pieces = Pieces(4);
    /// The object motion's target signed.
    pub const SIGNED_MOTION: Pieces = Pieces(8);
    /// What DLSS needs.
    pub const ALL: Pieces = Pieces(15);

    pub fn has(self, piece: Pieces) -> bool {
        self.0 & piece.0 != 0
    }

    fn with(self, piece: Pieces) -> Pieces {
        Pieces(self.0 | piece.0)
    }
}

/// Where a frame's picture is moved from its unjittered place, in render
/// pixels: `x` to the right, `y` down, each within -0.5..0.5. (The sample
/// point of a pixel is then that much to the left and up in the scene.)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jitter {
    x: f32,
    y: f32,
}

impl Jitter {
    pub const NONE: Jitter = Jitter { x: 0.0, y: 0.0 };

    /// The `phase`th offset of the Halton(2,3) sequence.
    fn of_phase(phase: u32) -> Jitter {
        Jitter { x: halton(phase + 1, 2) - 0.5, y: halton(phase + 1, 3) - 0.5 }
    }

    pub fn x(self) -> f32 {
        self.x
    }

    pub fn y(self) -> f32 {
        self.y
    }

    /// `projection` with the picture of a `render`-sized frame moved by this
    /// jitter: the clip position's x and y get that share of its w (the
    /// engine's own jitter writes the same two elements, 0x1BE60F and
    /// 0x1BE643; clip y is up).
    fn of_projection(self, projection: &Matrix, render: Size) -> Matrix {
        let (right, up) = (2.0 * self.x / render.width as f32, -2.0 * self.y / render.height as f32);
        let mut moved = *projection;
        for row in moved.0.chunks_exact_mut(4) {
            row[0] += right * row[3];
            row[1] += up * row[3];
        }
        moved
    }
}

/// The `index`th of the Halton sequence of `base`, in 0..1 (index from 1).
fn halton(mut index: u32, base: u32) -> f32 {
    let (mut step, mut value) = (1.0f32, 0.0f32);
    while index > 0 {
        step /= base as f32;
        value += step * (index % base) as f32;
        index /= base;
    }
    value
}

/// A frame's camera: its view and its projection before any jitter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub view: Matrix,
    pub projection: Matrix,
}

impl Camera {
    /// Further than this between two frames, metres, is no motion of a body.
    const CUT_DISTANCE: f32 = 3.0;
    /// The cosine of the turn between two frames that is no turn of a head
    /// (60 degrees).
    const CUT_TURN: f32 = 0.5;

    /// Where the camera is in the world: the view is a rotation and a move
    /// (rows 0..2 and row 3), so its place is the move turned back.
    fn place(&self) -> [f32; 3] {
        let m = &self.view.0;
        [0, 1, 2].map(|axis| -(m[12] * m[axis * 4] + m[13] * m[axis * 4 + 1] + m[14] * m[axis * 4 + 2]))
    }

    /// Where it looks in the world: the view's z of each world axis.
    fn forward(&self) -> [f32; 3] {
        [self.view.0[2], self.view.0[6], self.view.0[10]]
    }

    /// Whether this camera is not the one `before` after a frame's motion:
    /// a respawn, the death camera's start, another player watched, a
    /// replay's jump.
    fn cut_from(&self, before: &Camera) -> bool {
        let (here, there) = (self.place(), before.place());
        let moved: f32 = (0..3).map(|axis| (here[axis] - there[axis]) * (here[axis] - there[axis])).sum();
        let (now, then) = (self.forward(), before.forward());
        let turn: f32 = (0..3).map(|axis| now[axis] * then[axis]).sum();
        moved > Camera::CUT_DISTANCE * Camera::CUT_DISTANCE || turn < Camera::CUT_TURN
    }
}

/// Why DLSS's history is thrown away at a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reset {
    /// The first frame of ours: nothing is before it.
    First,
    /// The frame's targets were made since the last frame (the window's
    /// size or mode, the render scale).
    Targets,
    /// Another scene or scene view than the last frame's (a map loaded,
    /// the lobby and a match).
    Scene,
    /// The camera is not where a frame's motion leaves it.
    Cut,
}

impl Reset {
    fn name(self) -> &'static str {
        match self {
            Reset::First => "first",
            Reset::Targets => "targets",
            Reset::Scene => "scene",
            Reset::Cut => "cut",
        }
    }
}

/// A frame of ours as it was rendered.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Shown {
    jitter: Jitter,
    camera: Camera,
    scene: SceneId,
    render: Size,
}

/// The frames of ours in a row.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Run {
    /// How many came before `now`.
    count: u32,
    now: Shown,
    /// The frame before `now` (`now` itself at the first).
    before: Shown,
    /// Why `now` has no history.
    reset: Option<Reset>,
    /// The targets were made since `now` was rendered.
    remade: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Frames {
    /// The last frame rendered was not ours (native, no post-process,
    /// nothing asked).
    Idle,
    Running(Run),
}

/// How the jitter goes on from frame to frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pace {
    /// No jitter.
    Still,
    /// The sequence, each offset for `hold` frames (1 for DLSS; more is a
    /// diagnostic, to see each offset's picture).
    Sequence { hold: u32 },
}

/// What happens to the frames.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Event {
    /// A frame of ours is about to be rendered, its jitter one of `phases`.
    Frame { camera: Camera, scene: SceneId, render: Size, phases: u32 },
    /// A frame that is not ours is about to be rendered.
    Other,
    /// The frame's targets were made.
    TargetsMade,
}

/// The frames after `event`.
fn step(frames: Frames, event: Event, pace: Pace) -> Frames {
    let jitter = |count: u32, phases: u32| match pace {
        Pace::Still => Jitter::NONE,
        Pace::Sequence { hold } => Jitter::of_phase(count / hold.max(1) % phases.max(1)),
    };
    match (frames, event) {
        (_, Event::Other) => Frames::Idle,
        (Frames::Idle, Event::TargetsMade) => Frames::Idle,
        (Frames::Running(run), Event::TargetsMade) => Frames::Running(Run { remade: true, ..run }),
        (Frames::Idle, Event::Frame { camera, scene, render, phases }) => {
            let now = Shown { jitter: jitter(0, phases), camera, scene, render };
            Frames::Running(Run { count: 0, now, before: now, reset: Some(Reset::First), remade: false })
        }
        (Frames::Running(run), Event::Frame { camera, scene, render, phases }) => {
            let count = run.count.wrapping_add(1);
            let reset = if run.remade || render != run.now.render {
                Some(Reset::Targets)
            } else if scene != run.now.scene {
                Some(Reset::Scene)
            } else if camera.cut_from(&run.now.camera) {
                Some(Reset::Cut)
            } else {
                None
            };
            let now = Shown { jitter: jitter(count, phases), camera, scene, render };
            Frames::Running(Run { count, now, before: run.now, reset, remade: false })
        }
    }
}

struct Owner {
    wanted: Pieces,
    pace: Pace,
    /// The diagnostics: every this many frames the frame is looked at
    /// ([`look`]).
    look_every: Option<u32>,
    frames: Frames,
    /// The targets of the context the last frame of ours was rendered with.
    targets: Option<ThreadBound<Targets>>,
}

impl Owner {
    /// Whether `piece` runs in the frame being rendered.
    fn runs(&self, piece: Pieces) -> bool {
        self.wanted.has(piece) && matches!(self.frames, Frames::Running(_))
    }
}

static OWNER: Lock<Owner> =
    Lock::new(Owner { wanted: Pieces::NONE, pace: Pace::Still, look_every: None, frames: Frames::Idle, targets: None });

/// The link's call, at the process's start beside [`render_size::ask`]:
/// `pieces` run in every frame rendered at a render scale.
pub fn ask(thread: &Thread, pieces: Pieces) {
    OWNER.with(thread, |owner| {
        owner.wanted = pieces;
        owner.pace = if pieces.has(Pieces::JITTER) { Pace::Sequence { hold: 1 } } else { Pace::Still };
    });
}

/// What DLSS is given of the frame being rendered: valid from the frame's
/// `render.jittered` to the next one's, on the render thread.
pub struct FrameInputs {
    /// The size the frame is rendered at.
    pub render: Size,
    /// This frame's jitter, and the jitter of the frame before.
    pub jitter: Jitter,
    pub jitter_before: Jitter,
    /// This frame's camera, and the camera of the frame before: neither
    /// holds a jitter.
    pub camera: Camera,
    pub camera_before: Camera,
    /// Why DLSS's history is thrown away at this frame.
    pub reset: Option<Reset>,
    /// `$user$present`: the frame after the post-process.
    pub colour: Target,
    /// The frame's depth-stencil (the render scale's own).
    pub depth: DepthTexture,
    /// `$user$position`: the linear view depth.
    pub linear_depth: Target,
    /// `$user$object_motion_vectors`, and whether it was made signed.
    pub motion: Target,
    pub motion_signed: bool,
}

/// The link's read; none while no frame of ours is being rendered.
pub fn frame(thread: &Thread) -> Option<FrameInputs> {
    let depth = render_size::depth_texture(thread)?;
    OWNER.with(thread, |owner| {
        let Frames::Running(run) = owner.frames else { return None };
        let targets = owner.targets.as_ref()?.get(thread)?;
        Some(FrameInputs {
            render: run.now.render,
            jitter: run.now.jitter,
            jitter_before: run.before.jitter,
            camera: run.now.camera,
            camera_before: run.before.camera,
            reset: run.reset,
            colour: targets.get(thread, TargetIndex::Present)?,
            depth,
            linear_depth: targets.get(thread, TargetIndex::LinearDepth)?,
            motion: targets.get(thread, TargetIndex::ObjectMotion)?,
            motion_signed: owner.wanted.has(Pieces::SIGNED_MOTION),
        })
    })
}

/// The diagnostics switch's text: the pieces by name (`jitter`, `aa`,
/// `motion`, `signed`, or `all`), and `hold<frames>` (each jitter offset
/// that many frames) and `look<frames>` (the frame looked at that often),
/// between commas.
fn switch_of_text(text: &[u8]) -> Option<(Pieces, Pace, Option<u32>)> {
    let number = |digits: &[u8]| {
        (!digits.is_empty() && digits.len() <= 6 && digits.iter().all(u8::is_ascii_digit))
            .then(|| digits.iter().fold(0u32, |n, digit| n * 10 + u32::from(digit - b'0')))
            .filter(|&n| n > 0)
    };
    let (mut pieces, mut hold, mut look) = (Pieces::NONE, 1, None);
    for word in text.split(|&byte| byte == b',') {
        match word {
            b"all" => pieces = Pieces::ALL,
            b"jitter" => pieces = pieces.with(Pieces::JITTER),
            b"aa" => pieces = pieces.with(Pieces::NO_AA),
            b"motion" => pieces = pieces.with(Pieces::MOTION),
            b"signed" => pieces = pieces.with(Pieces::SIGNED_MOTION),
            _ => match (word.strip_prefix(b"hold"), word.strip_prefix(b"look")) {
                (Some(digits), _) => hold = number(digits)?,
                (_, Some(digits)) => look = Some(number(digits)?),
                _ => return None,
            },
        }
    }
    let pace = if pieces.has(Pieces::JITTER) { Pace::Sequence { hold } } else { Pace::Still };
    (pieces != Pieces::NONE).then_some((pieces, pace, look))
}

/// The process's start, after the render scale's: the diagnostics switch
/// `SKHID_DLSS_INPUTS` (`all`, or `jitter,aa,motion,signed`; `hold120` and
/// `look60` beside them), for a game with `SKHID_DLSS_RENDER_SCALE` and no
/// helper. Unset, nothing is asked.
pub fn start(thread: &Thread) {
    let mut room = [0u8; 64];
    let len = get_environment(thread, c"SKHID_DLSS_INPUTS", &mut room);
    if len == 0 {
        return;
    }
    match switch_of_text(&room[..len]) {
        Some((pieces, pace, look_every)) => {
            ask(thread, pieces);
            OWNER.with(thread, |owner| {
                owner.pace = pace;
                owner.look_every = look_every;
            });
            let name = |piece: Pieces, name: &'static str| Text(if pieces.has(piece) { name } else { "" });
            line(&[Text("frame inputs: SKHID_DLSS_INPUTS:"), name(Pieces::JITTER, " jitter"), name(Pieces::NO_AA, " aa"),
                   name(Pieces::MOTION, " motion"), name(Pieces::SIGNED_MOTION, " signed"),
                   Text(if render_size::installed() { "" } else { ": no render scale asked (SKHID_DLSS_RENDER_SCALE), so nothing runs" })]);
        }
        None => line(&[Text("frame inputs: SKHID_DLSS_INPUTS is not all or jitter,aa,motion,signed (hold<frames>, look<frames>): nothing asked")]),
    }
}

/// `render.jittered`: a frame is about to be rendered. A frame of ours (a
/// render scale, the post-process on, something asked) goes on the run and
/// takes its jitter in its projection; the engine's own jitter, which
/// nothing resolves with its AA off, is taken out.
pub fn on_jittered(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(seam) = JitterSeam::at(ctx) else { return Flow::Continue };
    let scaled = render_size::scaled(thread).filter(|_| seam.post_processed());
    let unjittered = seam.projection();
    let projection = OWNER.with(thread, |owner| {
        let event = match scaled {
            Some((ratio, render)) if owner.wanted != Pieces::NONE => {
                Event::Frame { camera: Camera { view: seam.view(), projection: unjittered }, scene: seam.scene(), render,
                               phases: ratio.jitter_phases() }
            }
            _ => Event::Other,
        };
        owner.frames = step(owner.frames, event, owner.pace);
        let Frames::Running(run) = owner.frames else { return None };
        owner.targets = Some(ThreadBound::new(thread, seam.targets()));
        if owner.wanted.has(Pieces::JITTER) {
            Some(run.now.jitter.of_projection(&unjittered, run.now.render))
        } else {
            (owner.wanted.has(Pieces::NO_AA) && seam.engine_jittered()).then_some(unjittered)
        }
    });
    if let Some(projection) = projection {
        seam.project(&projection);
    }
    Flow::Continue
}

/// The rays from the eye through the frame's four corners at the far plane
/// (clip x,y (-1,1), (-1,-1), (1,1), (1,-1), the engine's order), in the
/// view and then in the world: each corner unprojected by the projection's
/// inverse and turned by the view's. The engine unprojects the first and
/// mirrors it, which a jittered projection is not.
fn eye_rays(projection_inverted: &Matrix, view_inverted: &Matrix) -> [[f32; 3]; 8] {
    let (p, v) = (&projection_inverted.0, &view_inverted.0);
    let mut rays = [[0.0; 3]; 8];
    for (corner, [x, y]) in [[-1.0f32, 1.0f32], [-1.0, -1.0], [1.0, 1.0], [1.0, -1.0]].into_iter().enumerate() {
        let at = |column: usize| x * p[column] + y * p[4 + column] + p[8 + column] + p[12 + column];
        let ray = [at(0) / at(3), at(1) / at(3), at(2) / at(3)];
        rays[corner] = ray;
        rays[4 + corner] = [0, 1, 2].map(|axis| ray[0] * v[axis] + ray[1] * v[4 + axis] + ray[2] * v[8 + axis]);
    }
    rays
}

/// `context.eye_rays`: in a jittered frame of ours each corner's eye ray is
/// its own, so a position rebuilt from depth is where the jittered frame
/// drew it.
pub fn on_eye_rays(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if !OWNER.with(ctx.thread(), |owner| owner.runs(Pieces::JITTER)) {
        return Flow::Continue;
    }
    match EyeRays::at(ctx) {
        Some(rays) => {
            let exact = eye_rays(&rays.projection_inverted(), &rays.view_inverted());
            rays.set(&exact);
            Flow::Skip
        }
        None => Flow::Continue,
    }
}

/// The render scale made the frame's targets ([`render_size::on_targets_size`]).
pub fn targets_made(thread: &Thread) {
    OWNER.with(thread, |owner| owner.frames = step(owner.frames, Event::TargetsMade, owner.pace));
}

/// `postprocess.aa_method`: none of the game's AA in a frame of ours.
pub fn on_aa_method(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if OWNER.with(ctx.thread(), |owner| owner.runs(Pieces::NO_AA)) {
        frame_inputs::no_aa(ctx);
    }
    Flow::Continue
}

/// `postprocess.motion_gate`: a frame of ours runs the object motion pass
/// though neither the motion blur nor the temporal AA asks for it.
pub fn on_motion_gate(ctx: &mut Context<'_>) -> Flow<HudOptionsOnMotionGateExit> {
    if !OWNER.with(ctx.thread(), |owner| owner.runs(Pieces::MOTION)) {
        return Flow::Continue;
    }
    match MotionGate::at(ctx) {
        Some(gate) => {
            gate.without_blur();
            Flow::Exit(HudOptionsOnMotionGateExit::Accumulate)
        }
        None => Flow::Continue,
    }
}

/// `postprocess.motion_quality`: a frame of ours runs the object motion
/// pass at any post-process quality.
pub fn on_motion_quality(ctx: &mut Context<'_>) -> Flow<HudOptionsOnMotionQualityExit> {
    if OWNER.with(ctx.thread(), |owner| owner.runs(Pieces::MOTION)) {
        Flow::Exit(HudOptionsOnMotionQualityExit::Accumulate)
    } else {
        Flow::Continue
    }
}

/// `postprocess.motion_clear`: a frame of ours clears the object motion's
/// target before the pass, as the engine does when its own options run it.
pub fn on_motion_clear(ctx: &mut Context<'_>) -> Flow<HudOptionsOnMotionClearExit> {
    if OWNER.with(ctx.thread(), |owner| owner.runs(Pieces::MOTION)) {
        Flow::Exit(HudOptionsOnMotionClearExit::Clear)
    } else {
        Flow::Continue
    }
}

/// `targets.motion_format`: targets made at a render scale have a signed
/// object motion target.
pub fn on_motion_format(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if render_size::scaled(thread).is_some()
        && OWNER.with(thread, |owner| owner.wanted.has(Pieces::SIGNED_MOTION))
        && let Some(format) = MotionFormat::at(ctx)
    {
        format.signed();
    }
    Flow::Continue
}

/// The stages of a frame are done ([`render_size::on_present_stage`]): the
/// diagnostics' look at it, when one is due.
pub fn frame_done(thread: &Thread) {
    let due = OWNER.with(thread, |owner| match (owner.frames, owner.look_every) {
        (Frames::Running(run), Some(every)) if run.count % every == every / 2 => Some((run, owner.wanted)),
        _ => None,
    });
    if let Some((run, wanted)) = due
        && let Some(inputs) = frame(thread)
    {
        look(thread, &run, wanted, &inputs);
    }
}

/// A value's thousandths, signed, for a log line.
fn thousandths(value: f32) -> [Piece<'static>; 2] {
    [Text(if value < 0.0 { "-" } else { "+" }), Dec((value.abs() * 1000.0 + 0.5) as u32)]
}

/// The diagnostics' look at a frame: a line of its jitter and reset, the
/// object motion read back (how much of it is negative), and the frame and
/// its motion written beside the game as `dlss-inputs-<frame>-present.bin`
/// and `-motion.bin` (a 16-byte header: `SKRT`, the width, the height, a
/// pixel's bytes; then the rows).
fn look(thread: &Thread, run: &Run, wanted: Pieces, inputs: &FrameInputs) {
    let [xs, x] = thousandths(inputs.jitter.x());
    let [ys, y] = thousandths(inputs.jitter.y());
    line(&[Text("frame inputs: frame "), Dec(run.count), Text(" "), Dec(inputs.render.width), Text("x"), Dec(inputs.render.height),
           Text(", jitter x "), xs, x, Text(" y "), ys, y, Text(" /1000 px, reset "),
           Text(inputs.reset.map_or("none", Reset::name)), Text(", present "), Hex(inputs.colour.resource_address()),
           Text(" motion "), Hex(inputs.motion.resource_address()), Text(" linear depth "),
           Hex(inputs.linear_depth.resource_address())]);
    let written = frame_inputs::read_back(thread, &inputs.colour, PixelFormat::Rgba8, |pixels| write(run.count, b"present", pixels));
    if written.is_none() {
        line(&[Text("frame inputs: the frame was not read back")]);
    }
    if !wanted.has(Pieces::SIGNED_MOTION) {
        return;
    }
    let motion = frame_inputs::read_back(thread, &inputs.motion, PixelFormat::Rgba16Float, |pixels| {
        write(run.count, b"motion", pixels);
        MotionSeen::of(pixels)
    });
    match motion {
        Some(seen) => {
            let [lxs, lx] = thousandths(seen.least[0]);
            let [mxs, mx] = thousandths(seen.most[0]);
            let [lys, ly] = thousandths(seen.least[1]);
            let [mys, my] = thousandths(seen.most[1]);
            line(&[Text("frame inputs: object motion: "), Dec(seen.covered), Text(" pixels covered, x "), lxs, lx, Text(" to "),
                   mxs, mx, Text(", y "), lys, ly, Text(" to "), mys, my, Text(" /1000 px; negative x in "), Dec(seen.negative[0]),
                   Text(", negative y in "), Dec(seen.negative[1])]);
        }
        None => line(&[Text("frame inputs: the object motion was not read back")]),
    }
}

/// The file `dlss-inputs-<frame>-<what>.bin` with `pixels`.
fn write(frame: u32, what: &[u8], pixels: &Pixels<'_>) {
    let mut name = [0u8; 48];
    let mut len = 0;
    let mut push = |bytes: &[u8]| {
        for &byte in bytes {
            if len + 1 < name.len() {
                name[len] = byte;
                len += 1;
            }
        }
    };
    push(b"dlss-inputs-");
    push(&[6, 5, 4, 3, 2, 1].map(|place| b'0' + (frame / 10u32.pow(place - 1) % 10) as u8));
    push(b"-");
    push(what);
    push(b".bin");
    let Some(mut file) = Path::new(&name).and_then(File::create_always) else { return };
    let Size { width, height } = pixels.size();
    file.write(b"SKRT");
    for word in [width, height, pixels.format().bytes()] {
        file.write(&word.to_le_bytes());
    }
    for row in (0..height).filter_map(|row| pixels.row(row)) {
        file.write(row);
    }
}

/// A half float's value (IEEE 754 binary16).
fn half(bits: u16) -> f32 {
    let (sign, exponent, fraction) = (u32::from(bits & 0x8000) << 16, u32::from(bits >> 10 & 0x1F), u32::from(bits & 0x3FF));
    match exponent {
        0 => {
            let small = fraction as f32 / 16_777_216.0;
            if sign == 0 { small } else { -small }
        }
        31 => f32::from_bits(sign | 0xFF << 23 | fraction << 13),
        _ => f32::from_bits(sign | (exponent + 112) << 23 | fraction << 13),
    }
}

/// What a read-back object motion target holds: the pixels a model covered
/// (z above a half), the least and most of their motion in render pixels,
/// and in how many it is negative.
#[derive(Clone, Copy, Debug, PartialEq)]
struct MotionSeen {
    covered: u32,
    least: [f32; 2],
    most: [f32; 2],
    negative: [u32; 2],
}

impl MotionSeen {
    fn of(pixels: &Pixels<'_>) -> MotionSeen {
        let Size { width, height } = pixels.size();
        let mut seen = MotionSeen { covered: 0, least: [0.0; 2], most: [0.0; 2], negative: [0; 2] };
        for row in (0..height).filter_map(|row| pixels.row(row)) {
            for pixel in row.chunks_exact(8) {
                let value = |channel: usize| half(u16::from_le_bytes([pixel[channel * 2], pixel[channel * 2 + 1]]));
                seen.take([value(0) * width as f32, value(1) * height as f32], value(2));
            }
        }
        seen
    }

    /// A pixel: its motion in render pixels and its cover.
    fn take(&mut self, motion: [f32; 2], cover: f32) {
        if cover <= 0.5 {
            return;
        }
        self.covered += 1;
        for (axis, &moved) in motion.iter().enumerate() {
            self.least[axis] = self.least[axis].min(moved);
            self.most[axis] = self.most[axis].max(moved);
            self.negative[axis] += u32::from(moved < 0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::render_size::Ratio;
    use super::*;

    const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

    /// The engine's perspective for row vectors: w is the view z.
    fn perspective() -> Matrix {
        let (near, far) = (0.1f32, 1000.0f32);
        let mut m = [0.0; 16];
        (m[0], m[5], m[10], m[11], m[14]) = (1.3, 2.3, far / (far - near), 1.0, -near * far / (far - near));
        Matrix(m)
    }

    /// A camera at `place` looking along the world's z.
    fn camera_at(place: [f32; 3]) -> Camera {
        let mut view = IDENTITY;
        (view[12], view[13], view[14]) = (-place[0], -place[1], -place[2]);
        Camera { view: Matrix(view), projection: perspective() }
    }

    /// Where a view position lands in a `render`-sized frame, pixels (y down).
    fn pixel(projection: &Matrix, at: [f32; 3], render: Size) -> [f32; 2] {
        let m = &projection.0;
        let clip = |column: usize| at[0] * m[column] + at[1] * m[4 + column] + at[2] * m[8 + column] + m[12 + column];
        [(clip(0) / clip(3) + 1.0) * 0.5 * render.width as f32, (1.0 - clip(1) / clip(3)) * 0.5 * render.height as f32]
    }

    #[test]
    fn the_jitter_is_the_halton_sequence_under_a_pixel() {
        assert_eq!(Jitter::of_phase(0), Jitter { x: 0.0, y: 1.0 / 3.0 - 0.5 });
        assert_eq!(Jitter::of_phase(1), Jitter { x: -0.25, y: 2.0 / 3.0 - 0.5 });
        assert_eq!(Jitter::of_phase(2), Jitter { x: 0.25, y: 1.0 / 9.0 - 0.5 });
        for phase in 0..64 {
            let jitter = Jitter::of_phase(phase);
            assert!((-0.5..0.5).contains(&jitter.x) && (-0.5..0.5).contains(&jitter.y), "{jitter:?}");
        }
        // DLSS's count at Quality: 8 * (output / render)^2
        assert_eq!(Ratio::new(0.667).unwrap().jitter_phases(), 18);
        assert_eq!(Ratio::new(0.5).unwrap().jitter_phases(), 32);
    }

    #[test]
    fn a_jittered_projection_moves_the_picture_by_the_jitter() {
        let render = Size { width: 1280, height: 720 };
        let jitter = Jitter { x: 0.25, y: -0.375 };
        let moved = jitter.of_projection(&perspective(), render);
        for at in [[0.0, 0.0, 1.0], [3.0, -2.0, 7.5], [-40.0, 11.0, 300.0]] {
            let (before, after) = (pixel(&perspective(), at, render), pixel(&moved, at, render));
            assert!((after[0] - before[0] - 0.25).abs() < 1e-3 && (after[1] - before[1] + 0.375).abs() < 1e-3,
                    "{at:?}: {before:?} to {after:?}");
        }
        // depth is the projection's own
        assert_eq!((moved.0[10], moved.0[14], moved.0[11]), (perspective().0[10], perspective().0[14], 1.0));
        assert_eq!(Jitter::NONE.of_projection(&perspective(), render), perspective());
    }

    /// The inverse of [`perspective`] with a jitter's two elements `e20`, `e21`.
    fn perspective_inverted(e20: f32, e21: f32) -> Matrix {
        let p = perspective().0;
        let mut m = [0.0; 16];
        (m[0], m[5], m[11], m[14], m[15]) = (1.0 / p[0], 1.0 / p[5], 1.0 / p[14], 1.0, -p[10] / p[14]);
        (m[12], m[13]) = (-e20 / p[0], -e21 / p[5]);
        Matrix(m)
    }

    #[test]
    fn each_corner_has_its_own_eye_ray() {
        // the engine's own for a projection that is the same either side: the first corner's, mirrored
        let rays = eye_rays(&perspective_inverted(0.0, 0.0), &Matrix(IDENTITY));
        let [x, y, z] = rays[0];
        assert!((z - 1000.0).abs() < 2.0 && x < 0.0 && y > 0.0, "{:?}", rays[0]);
        assert_eq!([rays[1], rays[2], rays[3]], [[x, -y, z], [-x, y, z], [-x, -y, z]]);
        assert_eq!(rays[4..], rays[..4]);
        // a jittered one: each ray lands on its corner, which the mirrored rays do not
        let render = Size { width: 1280, height: 720 };
        let jittered = Jitter { x: 0.5, y: -0.25 }.of_projection(&perspective(), render);
        let (e20, e21) = (jittered.0[8], jittered.0[9]);
        let rays = eye_rays(&perspective_inverted(e20, e21), &Matrix(IDENTITY));
        for (ray, corner) in rays[..4].iter().zip([[0.0, 0.0], [0.0, 720.0], [1280.0, 0.0], [1280.0, 720.0]]) {
            let at = pixel(&jittered, *ray, render);
            assert!((at[0] - corner[0]).abs() < 1e-2 && (at[1] - corner[1]).abs() < 1e-2, "{ray:?} at {at:?}");
        }
        let mirrored = pixel(&jittered, [-rays[0][0], -rays[0][1], rays[0][2]], render);
        assert!((mirrored[0] - 1281.0).abs() < 1e-2 && (mirrored[1] - 719.5).abs() < 1e-2, "{mirrored:?}");
        // the world's rays are the view's turned: a view looking along the world's x
        let mut turned = IDENTITY;
        (turned[0], turned[2], turned[8], turned[10]) = (0.0, -1.0, 1.0, 0.0);
        let world = eye_rays(&perspective_inverted(0.0, 0.0), &Matrix(turned));
        assert_eq!(world[4], [world[0][2], world[0][1], -world[0][0]]);
    }

    #[test]
    fn the_frames_follow_the_camera() {
        let render = Size { width: 1280, height: 720 };
        let scene = SceneId::test(1);
        let frame = |camera: Camera, scene: SceneId| Event::Frame { camera, scene, render, phases: 18 };
        let pace = Pace::Sequence { hold: 1 };
        let run = |frames: Frames| match frames {
            Frames::Running(run) => run,
            Frames::Idle => panic!("idle"),
        };
        // the first frame has nothing before it
        let first = run(step(Frames::Idle, frame(camera_at([0.0, 0.0, 0.0]), scene), pace));
        assert_eq!((first.count, first.reset, first.now.jitter), (0, Some(Reset::First), Jitter::of_phase(0)));
        assert_eq!(first.before, first.now);
        // a step of a body: the next offset, the frame before kept
        let second = run(step(Frames::Running(first), frame(camera_at([0.0, 0.0, 0.2]), scene), pace));
        assert_eq!((second.count, second.reset, second.now.jitter), (1, None, Jitter::of_phase(1)));
        assert_eq!(second.before, first.now);
        // a respawn: the camera elsewhere
        let cut = run(step(Frames::Running(second), frame(camera_at([40.0, 0.0, 0.2]), scene), pace));
        assert_eq!(cut.reset, Some(Reset::Cut));
        // a look the other way
        let mut back = camera_at([40.0, 0.0, 0.2]);
        (back.view.0[0], back.view.0[10]) = (-1.0, -1.0);
        assert_eq!(run(step(Frames::Running(cut), frame(back, scene), pace)).reset, Some(Reset::Cut));
        // another scene; the targets made between two frames; another size
        assert_eq!(run(step(Frames::Running(cut), frame(camera_at([40.0, 0.0, 0.2]), SceneId::test(2)), pace)).reset,
                   Some(Reset::Scene));
        let remade = step(Frames::Running(cut), Event::TargetsMade, pace);
        assert_eq!(run(step(remade, frame(camera_at([40.0, 0.0, 0.2]), scene), pace)).reset, Some(Reset::Targets));
        let smaller = Event::Frame { camera: camera_at([40.0, 0.0, 0.2]), scene, render: Size { width: 640, height: 360 }, phases: 18 };
        assert_eq!(run(step(Frames::Running(cut), smaller, pace)).reset, Some(Reset::Targets));
        // a frame that is not ours ends the run: the next one is a first
        assert_eq!(step(Frames::Running(cut), Event::Other, pace), Frames::Idle);
        assert_eq!(step(Frames::Idle, Event::TargetsMade, pace), Frames::Idle);
        // the sequence comes round, an offset held is held
        let mut frames = Frames::Idle;
        for count in 0..40u32 {
            frames = step(frames, frame(camera_at([0.0, 0.0, 0.0]), scene), pace);
            assert_eq!(run(frames).now.jitter, Jitter::of_phase(count % 18));
        }
        let held = Pace::Sequence { hold: 3 };
        let mut frames = Frames::Idle;
        for count in 0..12u32 {
            frames = step(frames, frame(camera_at([0.0, 0.0, 0.0]), scene), held);
            assert_eq!(run(frames).now.jitter, Jitter::of_phase(count / 3));
        }
        assert_eq!(run(step(Frames::Idle, frame(camera_at([0.0, 0.0, 0.0]), scene), Pace::Still)).now.jitter, Jitter::NONE);
    }

    #[test]
    fn the_switch_names_its_pieces() {
        assert_eq!(switch_of_text(b"all"), Some((Pieces::ALL, Pace::Sequence { hold: 1 }, None)));
        assert_eq!(switch_of_text(b"jitter,hold120,look60"),
                   Some((Pieces::JITTER, Pace::Sequence { hold: 120 }, Some(60))));
        assert_eq!(switch_of_text(b"aa,motion,signed"),
                   Some((Pieces::NO_AA.with(Pieces::MOTION).with(Pieces::SIGNED_MOTION), Pace::Still, None)));
        for refused in [&b""[..], b"hold3", b"jitter,", b"jitter,hold", b"jitter,hold0", b"jitter,look1234567", b"taa", b"all;aa"] {
            assert_eq!(switch_of_text(refused), None, "{:?}", core::str::from_utf8(refused));
        }
    }

    #[test]
    fn a_read_back_motion_keeps_its_sign() {
        assert_eq!([half(0x0000), half(0x3C00), half(0xBC00), half(0x3800), half(0xC000)], [0.0, 1.0, -1.0, 0.5, -2.0]);
        assert_eq!(half(0x0001), 1.0 / 16_777_216.0);
        assert_eq!(half(0x8200), -512.0 / 16_777_216.0);
        assert!(half(0x7C00).is_infinite() && half(0x7E00).is_nan());
        let mut seen = MotionSeen { covered: 0, least: [0.0; 2], most: [0.0; 2], negative: [0; 2] };
        seen.take([9.0, 9.0], 0.0);
        seen.take([-1.5, 0.25], 1.0);
        seen.take([2.0, -0.5], 1.0);
        assert_eq!(seen, MotionSeen { covered: 2, least: [-1.5, -0.5], most: [2.0, 0.25], negative: [1, 1] });
    }
}
