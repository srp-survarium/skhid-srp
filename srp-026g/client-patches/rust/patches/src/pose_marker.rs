//! Patch 89: a local build's pose marker (docs/notes/pose-marker.md). In a
//! match, the comma key (the key right of M: M is `set_next_ammo_type`'s)
//! writes where the camera is and where it looks to `srp_poses.tsv` in the
//! user data folder, a row a press: the reference poses of a map port's
//! check against its donor (the comparator's poses: the place, the yaw and
//! pitch in degrees, forward = (sin yaw cos pitch, sin pitch, cos yaw cos
//! pitch)), with the map, its mode and project, the time and the camera's
//! vertical angle. Only in a diagnostics build (patch_026g.py
//! `--diagnostics`, `vostok::trace::enabled`); in every other build each hook
//! returns at once and the key is the game's as before. The key is never
//! taken from the game.
//!
//! The hooks are `pose_marker.toml`'s:
//! - the lobby loading a match's level (`level_loading.map`): its map id,
//!   its mode config's `mode` and `match_making_params.map_name` (the
//!   project) kept;
//! - the game world's keys (`tab_cursor.key`, after patch 86's): the comma
//!   key going down asks for a pose;
//! - update_camera's eye (`camera.eye`, after patch 76's death camera): the
//!   transform the view is given, once asked, written with the match's map
//!   (its options'), and the level kept when it is that map's. A line in
//!   srp_patches.log says what was written.
//!
//! Nothing a match computes: a file of the local machine.

#![forbid(unsafe_code)]

use ragdoll::math::{atan2, finite, sqrt};

use crate::vostok::clock::{self, LocalTime};
use crate::vostok::config::ConfigTable;
use crate::vostok::level::LevelLoading;
use crate::vostok::log::{self, Piece};
use crate::vostok::types::MapId;
use crate::vostok::{Context, Dik, Flow, GameWorldKey, KeyKind, Lock, NoExit, PoseMarkerOnKeyExit, camera, trace};

/// The key that marks a pose.
pub const KEY: Dik = Dik::COMMA;
/// The table, in the user data folder.
pub const FILE: &str = "srp_poses.tsv";
/// Its first line, written when the file is new.
pub const HEADER: &[u8] = b"name\ttime\tmap\tproject\tmode\tx\ty\tz\tyaw\tpitch\tvfov\n";

/// Whether a pose was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Marker {
    Idle,
    /// The key went down: the next eye is written.
    Asked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    /// The marker's key went down.
    Key,
    /// The view is given its eye.
    Eye,
}

impl Marker {
    /// The one transition: the next state, and whether this eye is written.
    fn step(self, event: Event) -> (Marker, bool) {
        match (self, event) {
            (_, Event::Key) => (Marker::Asked, false),
            (Marker::Asked, Event::Eye) => (Marker::Idle, true),
            (Marker::Idle, Event::Eye) => (Marker::Idle, false),
        }
    }
}

/// The longest project name (match_making_params.map_name: the game takes 31).
const PROJECT: usize = 31;

/// A project's name, as the mode config gives it ("level_01/day_tdm").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Project {
    bytes: [u8; PROJECT],
    len: u8,
}

impl Project {
    /// A name of printable ASCII without spaces (what the row can hold); None otherwise.
    fn new(text: &[u8]) -> Option<Project> {
        if text.is_empty() || text.len() > PROJECT || !text.iter().all(|c| c.is_ascii_graphic()) {
            return None;
        }
        let mut bytes = [0u8; PROJECT];
        bytes[..text.len()].copy_from_slice(text);
        Some(Project { bytes, len: text.len() as u8 })
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

/// The level the lobby loaded last: its map, and what its mode config says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Level {
    map: MapId,
    /// `mode.mode` (2, 3, 4, 5).
    mode: Option<u32>,
    project: Option<Project>,
}

impl Level {
    fn of(map: MapId, mode: Option<&ConfigTable<'_>>) -> Level {
        let number = mode.and_then(|t| t.get(c"mode").ok().flatten()).and_then(|v| v.integer().ok());
        let project = mode
            .and_then(|t| t.get(c"match_making_params").ok().flatten())
            .and_then(|v| v.table().ok())
            .and_then(|t| t.get(c"map_name").ok().flatten())
            .and_then(|v| v.text().ok())
            .and_then(|text| Project::new(text.to_bytes()));
        Level { map, mode: number, project }
    }
}

/// Where the camera is and where it looks, in the comparator's terms.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose {
    position: [f32; 3],
    /// Degrees about +y from +z toward +x.
    yaw: f32,
    /// Degrees up from the horizon.
    pitch: f32,
}

const DEGREES: f32 = 180.0 / core::f32::consts::PI;

impl Pose {
    /// A view's transform (rows right, up, forward, place): None when its
    /// forward or place is not a finite direction and point.
    fn of_view(view: &[f32; 16]) -> Option<Pose> {
        let (x, y, z) = (view[8], view[9], view[10]);
        let position = [view[12], view[13], view[14]];
        let flat = sqrt(x * x + z * z);
        if !(position.iter().chain(&[x, y, z]).all(|&v| finite(v)) && (flat > 0.0 || y != 0.0)) {
            return None;
        }
        Some(Pose { position, yaw: atan2(x, z) * DEGREES, pitch: atan2(y, flat) * DEGREES })
    }
}

/// One row being written.
struct Row {
    bytes: [u8; 256],
    len: usize,
}

impl Row {
    fn new() -> Row {
        Row { bytes: [0; 256], len: 0 }
    }

    fn text(&mut self, text: &[u8]) {
        for &c in text {
            if let Some(slot) = self.bytes.get_mut(self.len) {
                *slot = c;
                self.len += 1;
            }
        }
    }

    /// `value` in decimal, at least `width` digits.
    fn number(&mut self, value: u32, width: usize) {
        let mut digits = [0u8; 10];
        let (mut n, mut v) = (0, value);
        while n < 10 && (v != 0 || n < width.max(1)) {
            digits[n] = b'0' + (v % 10) as u8;
            v /= 10;
            n += 1;
        }
        digits[..n].reverse();
        self.text(&digits[..n]);
    }

    /// `value` with `places` decimals (1 to 3), rounded.
    fn fixed(&mut self, value: f32, places: u32) {
        let scale = 10u32.pow(places);
        let scaled = value * scale as f32;
        let rounded = (if scaled < 0.0 { scaled - 0.5 } else { scaled + 0.5 }) as i32;
        if rounded < 0 {
            self.text(b"-");
        }
        let units = rounded.unsigned_abs();
        self.number(units / scale, 1);
        self.text(b".");
        self.number(units % scale, places as usize);
    }

    fn tab(&mut self) {
        self.text(b"\t");
    }
}

/// The row of a pose: its name (`mark_<date>_<time>`, unique to the
/// millisecond), the time, the map (`-` when none), the level's project and
/// mode (`-` when the level loaded is not this map's), the place, the yaw
/// and pitch, the vertical angle.
fn row(out: &mut Row, time: &LocalTime, map: Option<MapId>, level: Option<&Level>, pose: &Pose, vfov: f32) {
    let level = level.filter(|level| Some(level.map) == map);
    out.text(b"mark_");
    out.number(u32::from(time.year), 4);
    out.number(u32::from(time.month), 2);
    out.number(u32::from(time.day), 2);
    out.text(b"_");
    out.number(u32::from(time.hour), 2);
    out.number(u32::from(time.minute), 2);
    out.number(u32::from(time.second), 2);
    out.text(b"_");
    out.number(u32::from(time.millisecond), 3);
    out.tab();
    out.number(u32::from(time.year), 4);
    out.text(b"-");
    out.number(u32::from(time.month), 2);
    out.text(b"-");
    out.number(u32::from(time.day), 2);
    out.text(b" ");
    out.number(u32::from(time.hour), 2);
    out.text(b":");
    out.number(u32::from(time.minute), 2);
    out.text(b":");
    out.number(u32::from(time.second), 2);
    out.text(b".");
    out.number(u32::from(time.millisecond), 3);
    out.tab();
    match map {
        Some(map) => out.number(map.get(), 1),
        None => out.text(b"-"),
    }
    out.tab();
    match level.and_then(|level| level.project) {
        Some(project) => out.text(project.as_bytes()),
        None => out.text(b"-"),
    }
    out.tab();
    match level.and_then(|level| level.mode) {
        Some(mode) => out.number(mode, 1),
        None => out.text(b"-"),
    }
    for v in pose.position {
        out.tab();
        out.fixed(v, 3);
    }
    out.tab();
    out.fixed(pose.yaw, 2);
    out.tab();
    out.fixed(pose.pitch, 2);
    out.tab();
    out.fixed(vfov, 2);
    out.text(b"\n");
}

static MARKER: Lock<Marker> = Lock::new(Marker::Idle);
static LEVEL: Lock<Option<Level>> = Lock::new(None);

/// switch_to_level_loading with the map's entry: the level kept.
pub fn on_level(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if !trace::enabled() {
        return Flow::Continue;
    }
    if let Some(loading) = LevelLoading::at(ctx) {
        let level = Level::of(loading.map(), loading.mode().as_ref());
        LEVEL.with(ctx.thread(), |kept| *kept = Some(level));
    }
    Flow::Continue
}

/// on_keyboard_action's start, after patch 86's: the key asks for a pose.
pub fn on_key(ctx: &mut Context<'_>) -> Flow<PoseMarkerOnKeyExit> {
    if !trace::enabled() {
        return Flow::Continue;
    }
    let Some(key) = GameWorldKey::at_start(ctx) else { return Flow::Continue };
    if key.key() == KEY && key.kind() == KeyKind::Down {
        MARKER.with(key.thread(), |marker| *marker = marker.step(Event::Key).0);
    }
    Flow::Continue
}

/// update_camera's eye, after patch 76's: the pose asked for written.
pub fn on_eye(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if !trace::enabled() {
        return Flow::Continue;
    }
    let thread = ctx.thread();
    let write = MARKER.with(thread, |marker| {
        let (next, write) = marker.step(Event::Eye);
        *marker = next;
        write
    });
    if !write {
        return Flow::Continue;
    }
    let Some(pose) = camera::view_transform(ctx).as_ref().and_then(Pose::of_view) else {
        log::line(&[Piece::Text("pose_marker: no camera transform, nothing written")]);
        return Flow::Continue;
    };
    let map = camera::eye_player(ctx).and_then(|(player, _)| player.match_map());
    let level = LEVEL.with(thread, |level| *level);
    let mut out = Row::new();
    row(&mut out, &clock::local_time(thread), map, level.as_ref(), &pose, camera::field_of_view(thread));
    let line = &out.bytes[..out.len];
    let name = line.split(|&c| c == b'\t').next().and_then(|name| core::str::from_utf8(name).ok()).unwrap_or("");
    if trace::append_row(thread, FILE, HEADER, line) {
        log::line(&[Piece::Text("pose_marker: "), Piece::Text(name), Piece::Text(" written to "), Piece::Text(FILE)]);
    } else {
        log::line(&[Piece::Text("pose_marker: "), Piece::Text(FILE), Piece::Text(" could not be written")]);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(row: &Row) -> &str {
        core::str::from_utf8(&row.bytes[..row.len]).unwrap()
    }

    const TIME: LocalTime =
        LocalTime { year: 2026, month: 11, day: 1, hour: 9, minute: 5, second: 7, millisecond: 42 };

    /// A view looking along (yaw, pitch) in degrees from `at`, rows right, up, forward, place.
    fn view(yaw: f32, pitch: f32, at: [f32; 3]) -> [f32; 16] {
        let (y, p) = (yaw.to_radians(), pitch.to_radians());
        let f = [y.sin() * p.cos(), p.sin(), y.cos() * p.cos()];
        let mut m = [0.0f32; 16];
        m[8..11].copy_from_slice(&f);
        m[12..15].copy_from_slice(&at);
        m[15] = 1.0;
        m
    }

    #[test]
    fn the_key_asks_and_the_next_eye_writes_once() {
        let (asked, write) = Marker::Idle.step(Event::Key);
        assert_eq!((asked, write), (Marker::Asked, false));
        assert_eq!(asked.step(Event::Key), (Marker::Asked, false), "pressed twice before a frame: one row");
        let (idle, write) = asked.step(Event::Eye);
        assert_eq!((idle, write), (Marker::Idle, true));
        assert_eq!(idle.step(Event::Eye), (Marker::Idle, false));
    }

    #[test]
    fn a_view_gives_the_comparators_yaw_and_pitch() {
        for (yaw, pitch) in [(0.0, 0.0), (90.0, 0.0), (-45.0, -2.0), (152.5, 30.0), (-170.0, -55.0)] {
            let pose = Pose::of_view(&view(yaw, pitch, [1.0, 2.0, 3.0])).unwrap();
            assert!((pose.yaw - yaw).abs() < 0.01 && (pose.pitch - pitch).abs() < 0.01, "{yaw} {pitch}: {pose:?}");
            assert_eq!(pose.position, [1.0, 2.0, 3.0]);
        }
        let straight_up = Pose::of_view(&view(0.0, 90.0, [0.0; 3])).unwrap();
        assert!((straight_up.pitch - 90.0).abs() < 0.01);
        assert_eq!(Pose::of_view(&[0.0; 16]), None, "no forward");
        let mut nan = view(0.0, 0.0, [0.0; 3]);
        nan[13] = f32::NAN;
        assert_eq!(Pose::of_view(&nan), None);
    }

    #[test]
    fn a_row_has_every_column_of_the_header() {
        let project = Project::new(b"level_01/day_tdm");
        let level = Level { map: MapId::new(204), mode: Some(4), project };
        let pose = Pose { position: [-48.5371, 24.5, -103.0804], yaw: -45.004, pitch: 0.004 };
        let mut out = Row::new();
        row(&mut out, &TIME, Some(MapId::new(204)), Some(&level), &pose, 65.0);
        assert_eq!(text(&out), "mark_20261101_090507_042\t2026-11-01 09:05:07.042\t204\tlevel_01/day_tdm\t4\t\
                               -48.537\t24.500\t-103.080\t-45.00\t0.00\t65.00\n");
        let header = core::str::from_utf8(HEADER).unwrap();
        assert_eq!(text(&out).split('\t').count(), header.split('\t').count());
        // another map's level, or none: the map alone
        let mut out = Row::new();
        row(&mut out, &TIME, Some(MapId::new(61)), Some(&level), &pose, 80.5);
        assert!(text(&out).contains("\t61\t-\t-\t") && text(&out).ends_with("\t80.50\n"));
        let mut out = Row::new();
        row(&mut out, &TIME, None, None, &pose, 65.0);
        assert!(text(&out).contains("\t-\t-\t-\t-48.537\t"));
    }

    #[test]
    fn numbers_round_and_keep_their_sign() {
        let cases: [(f32, u32, &str); 6] =
            [(0.0, 3, "0.000"), (-0.0004, 3, "0.000"), (-0.0006, 3, "-0.001"), (1.9999, 2, "2.00"),
             (-1234.5678, 3, "-1234.568"), (0.05, 1, "0.1")];
        for (value, places, want) in cases {
            let mut out = Row::new();
            out.fixed(value, places);
            assert_eq!(text(&out), want, "{value}");
        }
    }

    #[test]
    fn a_project_is_one_word_of_the_games_length() {
        assert_eq!(Project::new(b"level_01/day_tdm").map(|p| p.len), Some(16));
        assert_eq!(Project::new(b""), None);
        assert_eq!(Project::new(b"has space"), None);
        assert_eq!(Project::new(b"tab\there"), None);
        assert_eq!(Project::new(&[b'a'; 32]), None);
        assert!(Project::new(&[b'a'; 31]).is_some());
    }
}
