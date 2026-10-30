//! A diagnostics build's lobby camera sweep (the lobby audit's captures,
//! 2026-10-30): with no input, the lobby camera is put at the poses of a
//! script, each held a while, for static headless captures of every lobby
//! scene. Only in a diagnostics build (patch_026g.py `--ragdoll-trace`,
//! `vostok::trace::enabled`; never a release's) and only when the user data
//! folder has `srp_camera_sweep.txt`: one pose a line, `<hold s> <yaw deg>
//! <pitch deg> <distance m>` (`#` a comment), an orbit's pose: the distance
//! is the wheel's target and the distance at once, and the stock camera's
//! ray then cuts it as it always does; or `<hold s> <yaw deg> <pitch deg>
//! <distance m> <x> <y> <z> <side deg>`, a pose of patch 45's flight: the
//! camera's point there, its own steps held and no ray (patch 45's FLIGHT
//! word moving), as the flight puts it. Each pose's start is a line of
//! `srp_camera_sweep.csv` there (`step,yaw,pitch,distance`), which the
//! capture script waits for. After the last pose the camera stays there.
//! A line `squad` makes both squad panels' characters the account's own
//! (setup_profile with its profile and chosen kit) when the second pose
//! starts, for the full squad's view. A line `faction <id>` (the player's own
//! quest faction chosen), `led <id>` (a squad joined, its commander of that
//! faction) or `alone` (the squad left) changes patch 81's lobby when the
//! next pose starts, for captures of its change (lobby-scenes.md).

#![forbid(unsafe_code)]

use ragdoll::math::Vec3;

use crate::lobby_scene::{self, Faction, Squad};
use crate::vostok::lobby_camera::{CameraPose, FlightWord, LobbyCamera, Steps, WeaponTableState};
use crate::vostok::trace::{self, TraceFile};
use crate::vostok::{Context, Flow, GameCell, NoExit};

/// The most poses a script has.
pub const MOST: usize = 64;
const SCRIPT: &str = "srp_camera_sweep.txt";
const LOG: &str = "srp_camera_sweep.csv";

/// Where a pose looks from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Eye {
    /// The stock camera's orbit of its point, its ray cutting the distance.
    Orbit,
    /// A flight's pose: this point and side angle (radians), no ray.
    Flight { point: [f32; 3], side: f32 },
}

/// What changes patch 81's lobby as a pose starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    Nothing,
    /// The player's own quest faction chosen.
    Own(Faction),
    /// A squad joined or left.
    Squad(Squad),
}

/// One pose of the script.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub hold: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub eye: Eye,
    /// Applied as it starts.
    pub change: Change,
}

/// The script, parsed: its poses (`count` of them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Script {
    poses: [Pose; MOST],
    count: usize,
    squad: bool,
}

impl Script {
    const EMPTY: Script = Script {
        poses: [Pose { hold: 0.0, yaw: 0.0, pitch: 0.0, distance: 0.0, eye: Eye::Orbit, change: Change::Nothing }; MOST],
        count: 0,
        squad: false,
    };
}

/// The sweep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sweep {
    /// Not looked for yet (the first tick looks).
    Unread,
    /// Not this build, or no script: the camera is the game's.
    Off,
    /// At pose `step`, `held` s of its hold gone.
    /// (the script read is READ's)
    At { step: usize, held: f32 },
}

/// A number of a script's line: digits, an optional sign and point.
fn number(word: &[u8]) -> Option<f32> {
    let (sign, digits) = match word.first()? {
        b'-' => (-1.0, &word[1..]),
        b'+' => (1.0, &word[1..]),
        _ => (1.0, word),
    };
    let mut value = 0.0f32;
    let mut scale = 0.0f32;
    let mut any = false;
    for &c in digits {
        match c {
            b'0'..=b'9' => {
                any = true;
                if scale == 0.0 {
                    value = value * 10.0 + f32::from(c - b'0');
                } else {
                    value += f32::from(c - b'0') * scale;
                    scale *= 0.1;
                }
            }
            b'.' if scale == 0.0 => scale = 0.1,
            _ => return None,
        }
    }
    any.then_some(sign * value)
}

const DEGREE: f32 = core::f32::consts::PI / 180.0;

/// The script's poses; a line that is not four numbers (or a comment, or
/// empty) is passed over. None when no pose is in it.
pub fn parse(text: &[u8]) -> Option<Script> {
    let mut script = Script::EMPTY;
    let mut change = Change::Nothing;
    for line in text.split(|&c| c == b'\n') {
        if script.count == MOST {
            break;
        }
        let line = line.split(|&c| c == b'#').next().unwrap_or(&[]);
        if line.trim_ascii() == b"squad" {
            script.squad = true;
            continue;
        }
        if let Some(next) = change_of(line.trim_ascii()) {
            change = next;
            continue;
        }
        let mut words = line.split(|&c| c == b' ' || c == b'\t' || c == b'\r').filter(|w| !w.is_empty());
        let mut values = [0.0f32; 8];
        let mut n = 0;
        for word in words.by_ref() {
            if n == 8 {
                n = 9;
                break;
            }
            match number(word) {
                Some(v) => values[n] = v,
                None => {
                    n = 9;
                    break;
                }
            }
            n += 1;
        }
        let eye = match n {
            4 => Some(Eye::Orbit),
            8 => Some(Eye::Flight { point: [values[4], values[5], values[6]], side: values[7] * DEGREE }),
            _ => None,
        };
        if let Some(eye) = eye
            && values[0] >= 0.0
            && values[3] > 0.0
        {
            script.poses[script.count] = Pose {
                hold: values[0],
                yaw: values[1] * DEGREE,
                pitch: values[2] * DEGREE,
                distance: values[3],
                eye,
                change,
            };
            script.count += 1;
            change = Change::Nothing;
        }
    }
    (script.count > 0).then_some(script)
}

/// A line `faction <id>`, `led <id>` or `alone`.
fn change_of(line: &[u8]) -> Option<Change> {
    let id = |rest: &[u8]| match rest.trim_ascii() {
        [digit @ b'0'..=b'9'] => Some(Faction::of_id(digit - b'0')),
        _ => None,
    };
    if line == b"alone" {
        Some(Change::Squad(Squad::None))
    } else if let Some(rest) = line.strip_prefix(b"faction ") {
        id(rest).map(Change::Own)
    } else if let Some(rest) = line.strip_prefix(b"led ") {
        id(rest).map(|faction| Change::Squad(Squad::Led(faction)))
    } else {
        None
    }
}

impl Sweep {
    /// One tick of `dt` s: the next state, the pose to put the camera at,
    /// and whether it is a new step (the log's line).
    pub fn step(self, script: &Script, dt: f32) -> (Sweep, Option<(Pose, Option<usize>)>) {
        match self {
            Sweep::Unread | Sweep::Off => (self, None),
            Sweep::At { step, held } => {
                let pose = script.poses[step];
                let held = held + dt;
                if held >= pose.hold && step + 1 < script.count {
                    let next = Sweep::At { step: step + 1, held: 0.0 };
                    (next, Some((script.poses[step + 1], Some(step + 1))))
                } else {
                    (Sweep::At { step, held }, Some((pose, None)))
                }
            }
        }
    }
}

static SWEEP: GameCell<Sweep> = GameCell::new(Sweep::Unread);
static READ: GameCell<Script> = GameCell::new(Script::EMPTY);

/// `lobby_camera.view`, after patch 45's flight (its chain).
pub fn on_view(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let mut sweep = SWEEP.get(thread);
    let mut first = false;
    if sweep == Sweep::Unread {
        let mut text = [0u8; 4096];
        sweep = match trace::enabled().then(|| trace::read_input(thread, SCRIPT, &mut text)).flatten() {
            Some(n) => match parse(&text[..n]) {
                Some(script) => {
                    first = true;
                    READ.set(thread, script);
                    Sweep::At { step: 0, held: 0.0 }
                }
                None => Sweep::Off,
            },
            None => Sweep::Off,
        };
    }
    let Some((mut camera, dt)) = LobbyCamera::at(ctx) else {
        SWEEP.set(thread, sweep);
        return Flow::Continue;
    };
    let script = READ.get(thread);
    let (next, pose) = sweep.step(&script, if first { 0.0 } else { dt });
    if let Some((pose, new)) = pose {
        match pose.eye {
            Eye::Orbit => camera.look(pose.yaw, pose.pitch, pose.distance),
            Eye::Flight { point, side } => {
                let at = CameraPose {
                    point: Vec3::new(point[0], point[1], point[2]),
                    yaw: pose.yaw,
                    pitch: pose.pitch,
                    distance: pose.distance,
                    side,
                    zoom: 1.0,
                };
                camera.set(&at, Steps::Held);
                WeaponTableState::of(thread).set_flight(FlightWord::Moving);
            }
        }
        let logged = if first { Some(0) } else { new };
        if logged.is_some() {
            match pose.change {
                Change::Nothing => {}
                Change::Own(faction) => lobby_scene::force(thread, Some(faction), None),
                Change::Squad(squad) => lobby_scene::force(thread, None, Some(squad)),
            }
        }
        // the squad's characters at the second pose (the account's profile is there by then:
        // the first is the settling hold after the sign-in)
        if new == Some(1) && script.squad {
            camera.show_squad_dummies();
        }
        if let Some(step) = logged
            && let Some(file) = TraceFile::open(thread, LOG)
        {
            let mut line = [0u8; 64];
            let n = log_line(&mut line, step, &pose);
            file.write(thread, &line[..n]);
        }
    }
    SWEEP.set(thread, next);
    Flow::Continue
}

/// `v` in decimal at `out[n..]`; the new `n`.
fn put(out: &mut [u8; 64], n: usize, v: i32) -> usize {
    let mut n = n;
    if v < 0 {
        out[n] = b'-';
        n += 1;
    }
    let mut digits = [0u8; 10];
    let mut k = 0;
    let mut u = v.unsigned_abs();
    loop {
        digits[k] = b'0' + (u % 10) as u8;
        k += 1;
        u /= 10;
        if u == 0 {
            break;
        }
    }
    while k > 0 {
        k -= 1;
        out[n] = digits[k];
        n += 1;
    }
    n
}

/// `step,yaw,pitch,distance` (degrees, centimetres), a line.
fn log_line(out: &mut [u8; 64], step: usize, pose: &Pose) -> usize {
    let round = |x: f32| if x >= 0.0 { (x + 0.5) as i32 } else { (x - 0.5) as i32 };
    let mut n = put(out, 0, step as i32);
    for v in [round(pose.yaw / DEGREE), round(pose.pitch / DEGREE), round(pose.distance * 100.0)] {
        out[n] = b',';
        n = put(out, n + 1, v);
    }
    out[n] = b'\n';
    n + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_parses_and_steps() {
        let script = parse(b"# settle\n45 0 0 1.96\r\n6 30 0 1.96\nbad line here\n6 -30.5 -10 4.6\n").unwrap();
        assert_eq!(script.count, 3);
        assert_eq!(script.poses[2].distance, 4.6);
        assert!((script.poses[2].yaw + 30.5 * DEGREE).abs() < 1e-6 && (script.poses[2].pitch + 10.0 * DEGREE).abs() < 1e-6);
        let mut sweep = Sweep::At { step: 0, held: 0.0 };
        let mut starts = [0usize; 3];
        for tick in 0..6000 {
            let (next, pose) = sweep.step(&script, 0.016);
            if let Some((_, Some(step))) = pose {
                starts[step] = tick;
            }
            sweep = next;
        }
        assert_eq!(starts[1], 2812, "after its 45 s");
        assert!(matches!(sweep, Sweep::At { step: 2, .. }), "stays at the last");
        assert_eq!(parse(b"\n# nothing\n1 2 3\n"), None);
        let changes = parse(b"45 0 0 1.96\nfaction 2\n8 0 0 1.96\nled 4\n8 0 0 1.96\nalone\nled x\n8 0 0 1.96\n").unwrap();
        assert_eq!(changes.count, 4);
        assert_eq!(changes.poses[0].change, Change::Nothing);
        assert_eq!(changes.poses[1].change, Change::Own(Faction::BlackMarket));
        assert_eq!(changes.poses[2].change, Change::Squad(Squad::Led(Faction::Edge)));
        assert_eq!(changes.poses[3].change, Change::Squad(Squad::None), "a line that is no change passed over");
        let flight = parse(b"squad\n0.5 -90 -14.3 1.2 4.25 1.3 3.05 21\n").unwrap();
        assert!(flight.squad && flight.count == 1);
        assert_eq!(flight.poses[0].eye, Eye::Flight { point: [4.25, 1.3, 3.05], side: 21.0 * DEGREE });
        let mut line = [0u8; 64];
        let n = log_line(&mut line, 3, &script.poses[2]);
        assert_eq!(&line[..n], b"3,-31,-10,460\n");
    }
}
