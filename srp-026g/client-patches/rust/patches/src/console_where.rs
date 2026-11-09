//! Patch 133: the console command `where` says where the camera's character
//! is and where it looks (docs/notes/client_patch.md section 3.80).
//!
//! Every frame, update_camera after the view's update (`camera.viewed`)
//! keeps the camera's player's place and the view's transform, on the draw's
//! thread (mt_draw, maybe a task of its own: a [`Lock`]). The command, a
//! cc_delegate of ours registered at on_configs_loaded's start, prints the
//! last one when the console executes it, as a line of the game's log (the
//! console shows it, Survarium_<user>.log keeps it) and of srp_patches.log:
//!
//! ```text
//! where: pos -51.899 5.414 12.914 eye -51.900 7.114 12.914 dir 0.700 -0.150 -0.700 yaw 135.0 pitch -8.6
//! ```
//!
//! `pos` the player's place, `eye` the camera's (only when it is not `pos`
//! to the millimetre), `dir` the view's forward as a unit vector, `yaw` its
//! heading in degrees from +Z towards +X (0 to 360), `pitch` its angle above
//! the horizontal. A view older than a second says how old; none yet says
//! so. It shows only what the camera shows, so it ships on.

#![forbid(unsafe_code)]

use ::ragdoll::math::{atan2, sqrt};

use crate::vostok::camera::{self, CameraView};
use crate::vostok::console::{self, Command, ConsoleArguments, ConsoleCommand};
use crate::vostok::files::tick_count;
use crate::vostok::log::{self, Piece};
use crate::vostok::{Context, Flow, Lock, NoExit, OrigCcDelegateExecute, console_where_methods_vtable_handle};

const NAME: &core::ffi::CStr = c"where";

static COMMAND: Command = Command::new();

/// The last view the camera had.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Seen {
    Nothing,
    /// At GetTickCount's `tick`.
    View { view: CameraView, tick: u32 },
}

static SEEN: Lock<Seen> = Lock::new(Seen::Nothing);

/// A view older than this (ms) says how old it is.
const STALE_MS: u32 = 1000;

/// on_configs_loaded's start: the command registered (once).
pub fn on_configs(ctx: &mut Context<'_>) -> Flow<NoExit> {
    COMMAND.register(ctx.thread(), NAME, console_where_methods_vtable_handle());
    Flow::Continue
}

/// update_camera after the view's update: the view kept.
pub fn on_viewed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(view) = camera::viewed(ctx) {
        let tick = tick_count();
        SEEN.with(ctx.thread(), |seen| *seen = Seen::View { view, tick });
    }
    Flow::Continue
}

/// The console executes `where` (its arguments ignored).
pub fn on_execute(_: &OrigCcDelegateExecute, command: &mut ConsoleCommand<'_>, _: &ConsoleArguments<'_>) {
    let thread = command.thread();
    let seen = SEEN.with(thread, |seen| *seen);
    let mut line = Line::new();
    describe(seen, tick_count(), &mut line);
    console::game_line(thread, line.c_str());
    log::line(&[Piece::Text(line.text())]);
}

/// The line for `seen` at GetTickCount's `now`.
fn describe(seen: Seen, now: u32, line: &mut Line) {
    line.push("where: ");
    let (view, tick) = match seen {
        Seen::Nothing => {
            line.push("no camera yet");
            return;
        }
        Seen::View { view, tick } => (view, tick),
    };
    line.push("pos");
    line.point(view.body, 3);
    if !same_to_the_millimetre(view.body, view.eye) {
        line.push(" eye");
        line.point(view.eye, 3);
    }
    let [x, y, z] = view.forward;
    let length = sqrt(x * x + y * y + z * z);
    if length > 0.0 {
        let forward = [x / length, y / length, z / length];
        line.push(" dir");
        line.point(forward, 3);
        line.push(" yaw ");
        line.fixed(yaw_degrees(forward), 1);
        line.push(" pitch ");
        line.fixed(pitch_degrees(forward), 1);
    } else {
        line.push(" dir none");
    }
    let age = now.wrapping_sub(tick);
    if age > STALE_MS {
        line.push(" (");
        line.fixed(age as f32 / 1000.0, 1);
        line.push(" s ago)");
    }
}

fn same_to_the_millimetre(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.0005)
}

const DEGREES: f32 = 180.0 / core::f32::consts::PI;

/// The heading of a unit vector: degrees from +Z towards +X, 0 to 360.
fn yaw_degrees([x, _, z]: [f32; 3]) -> f32 {
    let yaw = atan2(x, z) * DEGREES;
    if yaw < 0.0 { yaw + 360.0 } else { yaw }
}

/// The angle of a unit vector above the horizontal, degrees.
fn pitch_degrees([x, y, z]: [f32; 3]) -> f32 {
    atan2(y, sqrt(x * x + z * z)) * DEGREES
}

/// The longest line (a longer one is cut), its zero after it.
const MOST: usize = 160;

/// A line of text, ASCII, with room for its zero.
struct Line {
    bytes: [u8; MOST + 1],
    len: usize,
}

impl Line {
    fn new() -> Line {
        Line { bytes: [0; MOST + 1], len: 0 }
    }

    fn byte(&mut self, byte: u8) {
        if self.len < MOST {
            self.bytes[self.len] = byte;
            self.len += 1;
        }
    }

    fn push(&mut self, text: &str) {
        text.bytes().for_each(|b| self.byte(b));
    }

    /// ` x y z`, each to `decimals`.
    fn point(&mut self, point: [f32; 3], decimals: u32) {
        for value in point {
            self.byte(b' ');
            self.fixed(value, decimals);
        }
    }

    /// `value` rounded to `decimals` (at most 3): `-51.899`; `nan`, `inf`,
    /// or `big` past a million.
    fn fixed(&mut self, value: f32, decimals: u32) {
        if value.is_nan() {
            return self.push("nan");
        }
        if value.is_infinite() {
            return self.push(if value > 0.0 { "inf" } else { "-inf" });
        }
        if value.abs() >= 1_000_000.0 {
            return self.push("big");
        }
        let scale = 10u32.pow(decimals.min(3));
        let scaled = f64::from(value.abs()) * f64::from(scale) + 0.5;
        let units = scaled as u32; // below 1e9: 1e6 x 1000
        if value < 0.0 && units != 0 {
            self.byte(b'-');
        }
        self.number(units / scale);
        if decimals > 0 {
            self.byte(b'.');
            let fraction = units % scale;
            let mut digit = scale / 10;
            while digit > 0 {
                self.byte(b'0' + (fraction / digit % 10) as u8);
                digit /= 10;
            }
        }
    }

    fn number(&mut self, value: u32) {
        let mut digits = [0u8; 10];
        let mut n = 0;
        let mut v = value;
        loop {
            digits[n] = b'0' + (v % 10) as u8;
            n += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        digits[..n].iter().rev().for_each(|&d| self.byte(d));
    }

    fn text(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("where: ?")
    }

    fn c_str(&self) -> &core::ffi::CStr {
        core::ffi::CStr::from_bytes_until_nul(&self.bytes[..=self.len]).unwrap_or(c"where: ?")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(seen: Seen, now: u32) -> String {
        let mut line = Line::new();
        describe(seen, now, &mut line);
        assert_eq!(line.c_str().to_str().unwrap(), line.text());
        line.text().to_string()
    }

    fn view(body: [f32; 3], eye: [f32; 3], forward: [f32; 3]) -> Seen {
        Seen::View { view: CameraView { body, eye, forward }, tick: 5000 }
    }

    #[test]
    fn nothing_seen_says_so() {
        assert_eq!(line(Seen::Nothing, 0), "where: no camera yet");
    }

    #[test]
    fn a_first_person_view() {
        let forward = [0.7, -0.15, -0.7];
        let text = line(view([-51.899, 5.414, 12.914], [-51.9, 7.114, 12.914], forward), 5400);
        let n = sqrt(0.7 * 0.7 * 2.0 + 0.15 * 0.15);
        let pitch = atan2(-0.15 / n, sqrt(2.0) * 0.7 / n) * DEGREES;
        assert_eq!(
            text,
            format!(
                "where: pos -51.899 5.414 12.914 eye -51.900 7.114 12.914 dir {:.3} {:.3} {:.3} yaw 135.0 pitch {:.1}",
                0.7 / n,
                -0.15 / n,
                -0.7 / n,
                pitch
            )
        );
    }

    #[test]
    fn the_eye_only_when_it_differs() {
        let text = line(view([1.0, 2.0, 3.0], [1.0002, 2.0, 3.0], [0.0, 0.0, 1.0]), 5000);
        assert_eq!(text, "where: pos 1.000 2.000 3.000 dir 0.000 0.000 1.000 yaw 0.0 pitch 0.0");
    }

    #[test]
    fn headings_round_the_compass() {
        for (forward, yaw, pitch) in [
            ([0.0, 0.0, 1.0], 0.0, 0.0),
            ([1.0, 0.0, 0.0], 90.0, 0.0),
            ([0.0, 0.0, -1.0], 180.0, 0.0),
            ([-1.0, 0.0, 0.0], 270.0, 0.0),
            ([0.0, 1.0, 0.0], 0.0, 90.0),
            ([0.0, -1.0, 0.0], 0.0, -90.0),
        ] {
            assert!((yaw_degrees(forward) - yaw).abs() < 1e-3, "{forward:?}");
            assert!((pitch_degrees(forward) - pitch).abs() < 1e-3, "{forward:?}");
        }
    }

    #[test]
    fn an_old_view_says_how_old() {
        let text = line(view([0.0; 3], [0.0; 3], [0.0, 0.0, 2.0]), 5000 + 12_340);
        assert_eq!(text, "where: pos 0.000 0.000 0.000 dir 0.000 0.000 1.000 yaw 0.0 pitch 0.0 (12.3 s ago)");
    }

    #[test]
    fn numbers_as_written() {
        let fixed = |value: f32, decimals| {
            let mut line = Line::new();
            line.fixed(value, decimals);
            line.text().to_string()
        };
        assert_eq!(fixed(-0.0004, 3), "0.000");
        assert_eq!(fixed(-0.0006, 3), "-0.001");
        assert_eq!(fixed(1234.5678, 3), "1234.568");
        assert_eq!(fixed(359.96, 1), "360.0");
        assert_eq!(fixed(f32::NAN, 3), "nan");
        assert_eq!(fixed(f32::NEG_INFINITY, 3), "-inf");
        assert_eq!(fixed(2e6, 3), "big");
        assert_eq!(fixed(7.0, 0), "7");
    }

    #[test]
    fn a_long_line_is_cut_with_its_zero() {
        let mut line = Line::new();
        line.push(&"x".repeat(500));
        assert_eq!(line.text().len(), MOST);
        assert_eq!(line.c_str().to_bytes().len(), MOST);
    }

    #[test]
    fn no_direction_says_so() {
        let text = line(view([0.0; 3], [0.0; 3], [0.0; 3]), 5000);
        assert_eq!(text, "where: pos 0.000 0.000 0.000 dir none");
    }
}
