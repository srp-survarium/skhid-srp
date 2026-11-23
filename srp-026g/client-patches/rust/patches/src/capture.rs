//! Patch 176: a capture run (`docs/notes/capture.md`). With
//! `SKHID_CAPTURE=<script>` in the game's environment the game puts its
//! camera at the script's poses and saves each one's picture, then ends;
//! without it nothing happens. Started with `-match_replay=` of a map's
//! replay with nobody in it (`srp-map-sur`), the game goes from its start
//! straight into the map: no server, sign-in, lobby or click
//! (`scripts/capture/capture.py` does all of it).
//!
//! The script, a line each (`#` a comment):
//!
//! ```text
//! folder Z:\path\to\pictures
//! wait 15000                       # ms from the world's first tick to pose 1
//! pose <name> <x> <y> <z> <fx> <fy> <fz> <fov> <near> <far> <settle ms>
//! eyes <name> <settle ms>          # the game's own camera
//! follow <place>                   # a replay's camera on that place's eyes
//! ```
//!
//! `eyes` and `follow` are diagnostics for a replay whose place enters with
//! a kit (`srp-map-sur --hold`): the picture is what its eyes see, first
//! person, as the game draws it.
//!
//! A pose is the eye's place, the direction it looks (any length, not
//! straight up or down), the vertical field of view in degrees and the
//! depth range; the aspect is the game's own. The camera is at pose 1 from
//! the world's first tick, so its surroundings stream in during the wait;
//! each pose is held its settle time, then the next frame's scene, before
//! the interface is drawn over it (F12's, patch 152), is saved as
//! `<folder>\<name>.png`. After the last the game exits with 0; a script it
//! cannot read, or a picture not saved, ends it with 2 or 1, the reason in
//! `srp_patches.log`.
//!
//! The camera is the one base_game_scene::apply_camera draws the scene from
//! (`camera.apply`): its director's transform and projection, written there
//! every frame. Nothing a match computes: the picture only.

#![forbid(unsafe_code)]

use core::f32::math::sqrt;
use core::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use crate::screenshot;
use crate::vostok::back_buffer::Output;
use crate::vostok::camera::Director;
use crate::vostok::clock;
use crate::vostok::files::{File, Path};
use crate::vostok::log::{self, Piece};
use crate::vostok::renderer;
use crate::vostok::system;
use crate::vostok::trig;
use crate::vostok::world::GameWorld;
use crate::vostok::{Context, Flow, Lock, NoExit, Thread};

/// The most poses a script has.
pub const MOST: usize = 64;
/// The longest pose name, and the longest folder.
const NAME: usize = 48;
const FOLDER: usize = 200;
/// The longest script.
const SCRIPT: usize = 16 * 1024;

/// One pose: a camera of its own, or (None) the game's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    name: [u8; NAME],
    name_len: usize,
    camera: Option<Camera>,
    settle_ms: u32,
}

/// A pose's camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    eye: [f32; 3],
    forward: [f32; 3],
    /// Vertical, degrees.
    fov: f32,
    /// 1 / tan(fov / 2).
    cotangent: f32,
    near: f32,
    far: f32,
}

impl Pose {
    const NONE: Pose = Pose { name: [0; NAME], name_len: 0, camera: None, settle_ms: 0 };

    fn name(&self) -> &[u8] {
        &self.name[..self.name_len]
    }
}

impl Camera {
    /// The camera's transform (rows right, up, forward, place) and the
    /// projection at `aspect` (width / height), D3D's left-handed one.
    fn matrices(&self, aspect: f32) -> ([f32; 16], [f32; 16]) {
        let [x, y, z] = self.forward;
        let n = sqrt(x * x + y * y + z * z);
        let (x, y, z) = (x / n, y / n, z / n);
        let h = sqrt(x * x + z * z);
        let (rx, rz) = (z / h, -x / h);
        let [px, py, pz] = self.eye;
        let view = [
            rx, 0.0, rz, 0.0, //
            y * rz, z * rx - x * rz, -y * rx, 0.0, //
            x, y, z, 0.0, //
            px, py, pz, 1.0,
        ];
        let cotangent = self.cotangent;
        let depth = self.far / (self.far - self.near);
        let mut projection = [0.0; 16];
        projection[0] = cotangent / aspect;
        projection[5] = cotangent;
        projection[10] = depth;
        projection[11] = 1.0;
        projection[14] = -self.near * depth;
        (view, projection)
    }
}

/// The script, read once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Script {
    folder: [u8; FOLDER],
    folder_len: usize,
    wait_ms: u32,
    poses: [Pose; MOST],
    count: usize,
    /// A replay's place whose eyes the camera follows.
    follow: Option<u8>,
}

/// Why a script is refused: what is wrong, and the line (from 1) it is
/// on, or none for the script as a whole (no folder, no pose).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused {
    pub line: Option<u32>,
    pub why: &'static str,
}

impl Script {
    const EMPTY: Script =
        Script { folder: [0; FOLDER], folder_len: 0, wait_ms: 0, poses: [Pose::NONE; MOST], count: 0, follow: None };

    fn poses(&self) -> &[Pose] {
        &self.poses[..self.count]
    }

    /// A picture's path: `<folder>\<name>.png` and a NUL.
    fn path(&self, pose: &Pose) -> [u8; FOLDER + NAME + 8] {
        let mut path = [0; FOLDER + NAME + 8];
        let parts: [&[u8]; 4] = [&self.folder[..self.folder_len], b"\\", pose.name(), b".png"];
        for (slot, &byte) in path.iter_mut().zip(parts.into_iter().flatten()) {
            *slot = byte;
        }
        path
    }

    /// Parse, don't validate: every line is a folder, a wait, a pose, a
    /// comment or empty, else the whole script is refused.
    pub fn parse(text: &[u8]) -> Result<Script, Refused> {
        let mut script = Script::EMPTY;
        for (index, line) in text.split(|&c| c == b'\n').enumerate() {
            let at = |why| Refused { line: Some(index as u32 + 1), why };
            let line = line.split(|&c| c == b'#').next().unwrap_or(&[]).trim_ascii();
            let mut words = line.split(|c| c.is_ascii_whitespace()).filter(|w| !w.is_empty());
            match words.next() {
                None => {}
                Some(b"folder") => {
                    let folder = line[b"folder".len()..].trim_ascii();
                    if folder.is_empty() || folder.len() > FOLDER || folder.iter().any(|c| !(b' '..=b'~').contains(c)) {
                        return Err(at("a folder of 1 to 200 printable ASCII bytes"));
                    }
                    script.folder[..folder.len()].copy_from_slice(folder);
                    script.folder_len = folder.len();
                }
                Some(b"wait") => {
                    script.wait_ms = words.next().and_then(whole).ok_or(at("wait <ms>"))?;
                    if words.next().is_some() {
                        return Err(at("wait <ms>"));
                    }
                }
                Some(b"follow") => {
                    let place = words.next().and_then(whole).filter(|&p| p < 20).ok_or(at("follow <place 0..19>"))?;
                    if words.next().is_some() {
                        return Err(at("follow <place 0..19>"));
                    }
                    script.follow = Some(place as u8);
                }
                Some(kind @ (b"pose" | b"eyes")) => {
                    if script.count == MOST {
                        return Err(at("more than 64 poses"));
                    }
                    let pose = match kind {
                        b"pose" => pose(&mut words).ok_or(at(
                            "pose <name> <x> <y> <z> <fx> <fy> <fz> <fov 1..170> <near> <far> <settle ms>",
                        ))?,
                        _ => eyes(&mut words).ok_or(at("eyes <name> <settle ms>"))?,
                    };
                    if script.poses().iter().any(|p| p.name() == pose.name()) {
                        return Err(at("a pose name twice"));
                    }
                    script.poses[script.count] = pose;
                    script.count += 1;
                }
                Some(_) => return Err(at("a line is folder, wait, pose, eyes or follow")),
            }
        }
        if script.folder_len == 0 {
            return Err(Refused { line: None, why: "no folder" });
        }
        if script.count == 0 {
            return Err(Refused { line: None, why: "no pose" });
        }
        Ok(script)
    }
}

fn named(name: &[u8], camera: Option<Camera>, settle_ms: u32) -> Option<Pose> {
    let allowed = |c: &u8| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.');
    if name.is_empty() || name.len() > NAME || name[0] == b'.' || !name.iter().all(allowed) {
        return None;
    }
    let mut pose = Pose { camera, settle_ms, ..Pose::NONE };
    pose.name[..name.len()].copy_from_slice(name);
    pose.name_len = name.len();
    Some(pose)
}

fn eyes<'a>(words: &mut impl Iterator<Item = &'a [u8]>) -> Option<Pose> {
    let name = words.next()?;
    let settle_ms = whole(words.next()?)?;
    if words.next().is_some() {
        return None;
    }
    named(name, None, settle_ms)
}

fn pose<'a>(words: &mut impl Iterator<Item = &'a [u8]>) -> Option<Pose> {
    let name = words.next()?;
    let mut values = [0.0f32; 9];
    for value in &mut values {
        *value = number(words.next()?)?;
    }
    let settle_ms = whole(words.next()?)?;
    if words.next().is_some() {
        return None;
    }
    let [x, y, z, fx, fy, fz, fov, near, far] = values;
    let level = sqrt(fx * fx + fz * fz);
    let length = sqrt(fx * fx + fy * fy + fz * fz);
    if level <= length * 1e-4 || !(1.0..=170.0).contains(&fov) || near <= 0.0 || far <= near {
        return None;
    }
    let half = fov * (core::f32::consts::PI / 360.0);
    let cotangent = trig::cos(half)? / trig::sin(half)?;
    named(name, Some(Camera { eye: [x, y, z], forward: [fx, fy, fz], fov, cotangent, near, far }), settle_ms)
}

/// A decimal: an optional sign, digits, an optional point and digits.
fn number(word: &[u8]) -> Option<f32> {
    let (sign, digits) = match word.first()? {
        b'-' => (-1.0, &word[1..]),
        b'+' => (1.0, &word[1..]),
        _ => (1.0, word),
    };
    let (whole_part, fraction) = match digits.iter().position(|&c| c == b'.') {
        Some(at) => (&digits[..at], &digits[at + 1..]),
        None => (digits, &[][..]),
    };
    if whole_part.is_empty() && fraction.is_empty() || whole_part.len() > 9 || fraction.len() > 9 {
        return None;
    }
    let mut value = 0.0f64;
    for &c in whole_part {
        value = value * 10.0 + f64::from(c.is_ascii_digit().then(|| c - b'0')?);
    }
    let mut scale = 0.1f64;
    for &c in fraction {
        value += f64::from(c.is_ascii_digit().then(|| c - b'0')?) * scale;
        scale *= 0.1;
    }
    Some(sign * value as f32)
}

/// Digits only, at most 9.
fn whole(word: &[u8]) -> Option<u32> {
    if word.is_empty() || word.len() > 9 || !word.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(word.iter().fold(0, |n, &c| n * 10 + u32::from(c - b'0')))
}

/// Where a run is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capture {
    /// The environment not looked at yet (the world's first tick does).
    Unread,
    /// No `SKHID_CAPTURE`: the game's own camera, nothing else, ever.
    Off,
    /// The world is in since `since` (ms); the camera at pose 0 while it
    /// streams in.
    Waiting { since: u32 },
    /// At `pose` since `since`, held its settle time.
    Settling { pose: usize, since: u32 },
    /// The next frame's scene is the picture of `pose`.
    Asked { pose: usize },
    /// The run is over: the game's thread ends the process with `code` at its
    /// next tick.
    Over { code: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The world's tick at `now` ms.
    Tick { now: u32 },
    /// The asked picture was saved, at `now` ms.
    Saved { now: u32 },
    /// It was not.
    NotSaved,
}

impl Capture {
    /// The run after `event`, and whether the process ends now (its code).
    pub fn step(self, script: &Script, event: Event) -> (Capture, Option<u32>) {
        let poses = script.poses();
        match (self, event) {
            (Capture::Waiting { since }, Event::Tick { now }) if now.wrapping_sub(since) >= script.wait_ms => {
                (Capture::Settling { pose: 0, since: now }, None)
            }
            (Capture::Settling { pose, since }, Event::Tick { now })
                if now.wrapping_sub(since) >= poses[pose].settle_ms =>
            {
                (Capture::Asked { pose }, None)
            }
            (Capture::Asked { pose }, Event::Saved { now }) if pose + 1 < poses.len() => {
                (Capture::Settling { pose: pose + 1, since: now }, None)
            }
            (Capture::Asked { .. }, Event::Saved { .. }) => (Capture::Over { code: 0 }, None),
            (Capture::Asked { .. }, Event::NotSaved) => (Capture::Over { code: 1 }, None),
            (Capture::Over { code }, Event::Tick { .. }) => (self, Some(code)),
            _ => (self, None),
        }
    }

    /// The pose the camera is at.
    fn pose(self) -> Option<usize> {
        match self {
            Capture::Waiting { .. } => Some(0),
            Capture::Settling { pose, .. } | Capture::Asked { pose } => Some(pose),
            Capture::Unread | Capture::Off | Capture::Over { .. } => None,
        }
    }
}

struct Run {
    capture: Capture,
    script: Script,
}

/// The game's thread ticks and places the camera, the drawing thread saves.
static RUN: Lock<Run> = Lock::new(Run { capture: Capture::Unread, script: Script::EMPTY });

/// Whether there is a run, for the hooks' first look: [`UNREAD`] until the
/// world's first tick, then [`OFF`] for good, or [`ON`] once [`RUN`] holds
/// it (written by the game's thread alone). Off, no hook takes the lock.
static STARTED: AtomicU8 = AtomicU8::new(UNREAD);
const UNREAD: u8 = 0;
const OFF: u8 = 1;
const ON: u8 = 2;

/// When the replay was last asked to follow the script's place (ms).
static FOLLOW_TRIED: AtomicU32 = AtomicU32::new(0);

/// The world's first tick: the run, from the environment and its script.
fn start(thread: &Thread, now: u32) -> (Capture, Script) {
    if !renderer::environment_set(thread, c"SKHID_CAPTURE") {
        return (Capture::Off, Script::EMPTY);
    }
    let mut room = [0u8; 1024];
    let len = renderer::get_environment(thread, c"SKHID_CAPTURE", &mut room);
    if len == 0 {
        log::line(&[Piece::Text("capture: SKHID_CAPTURE is longer than 1023 bytes")]);
        return (Capture::Over { code: 2 }, Script::EMPTY);
    }
    let mut text = [0u8; SCRIPT];
    let read = Path::new(&room[..=len]).and_then(File::open).and_then(|mut file| file.read(&mut text));
    let Some(read) = read.filter(|&n| (n as usize) < SCRIPT) else {
        log::line(&[Piece::Text("capture: SKHID_CAPTURE names no script the game can read (16 KiB at most)")]);
        return (Capture::Over { code: 2 }, Script::EMPTY);
    };
    match Script::parse(&text[..read as usize]) {
        Ok(script) => {
            log::line(&[Piece::Text("capture: "), Piece::Dec(script.count as u32), Piece::Text(" poses")]);
            (Capture::Waiting { since: now }, script)
        }
        Err(refused) => {
            match refused.line {
                Some(line) => log::line(&[
                    Piece::Text("capture: the script is refused, line "),
                    Piece::Dec(line),
                    Piece::Text(": "),
                    Piece::Text(refused.why),
                ]),
                None => log::line(&[Piece::Text("capture: the script is refused: "), Piece::Text(refused.why)]),
            }
            (Capture::Over { code: 2 }, Script::EMPTY)
        }
    }
}

/// `game_world.after_move`: every tick of a world.
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let now = clock::milliseconds(thread);
    match STARTED.load(Ordering::Acquire) {
        OFF => return Flow::Continue,
        UNREAD => {
            let (capture, script) = start(thread, now);
            let on = capture != Capture::Off;
            if on {
                RUN.with(thread, |run| (run.capture, run.script) = (capture, script));
            }
            STARTED.store(if on { ON } else { OFF }, Ordering::Release);
            if !on {
                return Flow::Continue;
            }
        }
        _ => {}
    }
    let (exit, follow) = RUN.with(thread, |run| {
        let (next, exit) = run.capture.step(&run.script, Event::Tick { now });
        run.capture = next;
        (exit, run.script.follow)
    });
    // a place enters some ticks after the start: asked once a second until followed
    if let Some(place) = follow
        && now.wrapping_sub(FOLLOW_TRIED.load(Ordering::Relaxed)) >= 1000
        && let Some(world) = GameWorld::at_after_move(ctx)
        && !world.follow_first_person(place)
    {
        FOLLOW_TRIED.store(now, Ordering::Relaxed);
    }
    if let Some(code) = exit {
        log::line(&[Piece::Text("capture: over, exit code "), Piece::Dec(code)]);
        system::exit(thread, code);
    }
    Flow::Continue
}

/// `camera.apply`: the scene is drawn from the pose.
pub fn on_camera(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let ctx = &*ctx;
    if STARTED.load(Ordering::Acquire) != ON {
        return Flow::Continue;
    }
    let Some(camera) = RUN.with(ctx.thread(), |run| run.capture.pose().and_then(|p| run.script.poses[p].camera)) else {
        return Flow::Continue;
    };
    let Some(mut director) = Director::at_apply(ctx) else { return Flow::Continue };
    let native = director.projection();
    let aspect = if native[0] > 0.0 && native[5] > 0.0 { native[5] / native[0] } else { 16.0 / 9.0 };
    let (view, projection) = camera.matrices(aspect);
    director.look(view, projection);
    Flow::Continue
}

/// `render.flash_ui`: the scene of the frame, before its interface.
pub fn on_scene(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let ctx = &*ctx;
    if STARTED.load(Ordering::Acquire) != ON {
        return Flow::Continue;
    }
    let thread = ctx.thread();
    let asked = RUN.with(thread, |run| match run.capture {
        Capture::Asked { pose } => Some((run.script.poses[pose], run.script.path(&run.script.poses[pose]))),
        _ => None,
    });
    let Some((pose, path)) = asked else { return Flow::Continue };
    let event = match save(ctx, &path) {
        Ok((width, height)) => {
            log::line(&[
                Piece::Text("capture: "),
                Piece::Text(core::str::from_utf8(pose.name()).unwrap_or("?")),
                Piece::Text(".png, "),
                Piece::Dec(width),
                Piece::Text(" x "),
                Piece::Dec(height),
            ]);
            Event::Saved { now: clock::milliseconds(thread) }
        }
        Err(why) => {
            log::line(&[
                Piece::Text("capture: "),
                Piece::Text(core::str::from_utf8(pose.name()).unwrap_or("?")),
                Piece::Text(" not saved: "),
                Piece::Text(why),
            ]);
            Event::NotSaved
        }
    };
    RUN.with(thread, |run| run.capture = run.capture.step(&run.script, event).0);
    Flow::Continue
}

fn save(ctx: &Context<'_>, path: &[u8]) -> Result<(u32, u32), &'static str> {
    let output = Output::before_interface(ctx).ok_or("the frame has no render output")?;
    let picture = output.read().map_err(|_| "the back buffer was not read")?;
    let path = Path::new(path).ok_or("no path")?;
    let mut file = File::create_always(path).ok_or("the file could not be made")?;
    screenshot::write_png(ctx.thread(), &picture, &mut file).map_err(screenshot::Refusal::text)?;
    Ok((picture.width(), picture.height()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &[u8] = b"# a run\nfolder Z:\\tmp\\out dir\r\nwait 15000\n\
        pose nw-high -72 26 66 0.6 -0.2 -0.5 73.7 0.1 1500 6000\n\
        pose s.2 1.5 -2.25 +3 0 0 1 60 .1 500 0 # last\n";

    #[test]
    fn a_script_parses() {
        let script = Script::parse(TEXT).unwrap();
        assert_eq!((script.count, script.wait_ms), (2, 15000));
        let first = script.poses[0];
        assert_eq!(first.name(), b"nw-high");
        assert_eq!((first.camera.unwrap().eye, first.camera.unwrap().fov, first.settle_ms), ([-72.0, 26.0, 66.0], 73.7, 6000));
        assert_eq!((script.poses[1].camera.unwrap().eye, script.poses[1].camera.unwrap().near), ([1.5, -2.25, 3.0], 0.1));
        let path = script.path(&script.poses[1]);
        assert!(path.starts_with(b"Z:\\tmp\\out dir\\s.2.png\0"));
    }

    #[test]
    fn a_bad_line_refuses_the_script() {
        let refused = |text: &[u8]| Script::parse(text).unwrap_err();
        let pose = b"folder x\npose a 0 0 0 1 0 0 60 0.1 100 0\n";
        assert!(Script::parse(pose).is_ok());
        assert_eq!(refused(b"folder x\npose a 0 0 0 0 1 0 60 0.1 100 0\n").line, Some(2), "straight up");
        assert_eq!(refused(b"folder x\npose a 0 0 0 1 0 0 180 0.1 100 0\n").line, Some(2), "the fov");
        assert_eq!(refused(b"folder x\npose a 0 0 0 1 0 0 60 1 1 0\n").line, Some(2), "far not past near");
        assert_eq!(refused(b"folder x\npose ../a 0 0 0 1 0 0 60 0.1 100 0\n").line, Some(2), "a name");
        assert_eq!(refused(b"folder x\npose a 0 0 0 1 0 0 60 0.1 100 0 7\n").line, Some(2), "a word more");
        assert_eq!(refused(b"folder x\npose a 0 0 0 1e3 0 0 60 0.1 100 0\n").line, Some(2), "no exponent");
        assert_eq!(refused(b"folder x\npose a 0 0 0 1 0 0 60 0.1 100 0\npose a 1 0 0 1 0 0 60 0.1 100 0\n").line, Some(3));
        assert_eq!(refused(b"folder x\nzoom 2\n").line, Some(2));
        assert_eq!(refused(b"pose a 0 0 0 1 0 0 60 0.1 100 0\n").why, "no folder");
        assert_eq!(refused(b"folder x\nwait 1\n").why, "no pose");
    }

    #[test]
    fn a_run_waits_settles_saves_each_pose_and_ends() {
        let script = Script::parse(TEXT).unwrap();
        let tick = |now| Event::Tick { now };
        let mut run = Capture::Waiting { since: 1000 };
        assert_eq!(run.pose(), Some(0), "the first pose streams in while it waits");
        assert_eq!(run.step(&script, tick(15999)), (run, None));
        run = run.step(&script, tick(16000)).0;
        assert_eq!(run, Capture::Settling { pose: 0, since: 16000 });
        assert_eq!(run.step(&script, Event::Saved { now: 1 }), (run, None), "nothing asked yet");
        run = run.step(&script, tick(22000)).0;
        assert_eq!(run, Capture::Asked { pose: 0 });
        assert_eq!(run.step(&script, tick(30000)), (run, None), "asked until the frame saves it");
        run = run.step(&script, Event::Saved { now: 22100 }).0;
        assert_eq!(run, Capture::Settling { pose: 1, since: 22100 });
        run = run.step(&script, tick(22100)).0;
        assert_eq!(run, Capture::Asked { pose: 1 }, "no settle time");
        run = run.step(&script, Event::Saved { now: 22200 }).0;
        assert_eq!(run, Capture::Over { code: 0 });
        assert_eq!(run.step(&script, tick(22250)), (run, Some(0)), "the game's thread ends it");
        assert_eq!(Capture::Asked { pose: 1 }.step(&script, Event::NotSaved).0, Capture::Over { code: 1 });
        assert_eq!(Capture::Off.step(&script, tick(5)), (Capture::Off, None));
    }

    #[test]
    fn the_view_looks_along_the_pose_with_its_fov() {
        let script = Script::parse(b"folder x\npose a 1 2 3 0 0 2 90 0.5 100.5 0\n").unwrap();
        let (view, projection) = script.poses[0].camera.unwrap().matrices(2.0);
        assert_eq!(&view[..12], &[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        assert_eq!(&view[12..], &[1.0, 2.0, 3.0, 1.0]);
        assert!((projection[5] - 1.0).abs() < 1e-6 && (projection[0] - 0.5).abs() < 1e-6);
        assert!((projection[10] - 1.005).abs() < 1e-6 && (projection[14] + 0.5025).abs() < 1e-6);
    }
}
