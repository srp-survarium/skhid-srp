//! J0: the jump test for the real client (`grid-record.md` section J0). TEST-ONLY: compiled only with
//! the cargo feature `jump-test`, and then inert unless `SRP_JUMP_TEST=<try file>` names a try list.
//!
//! In the mode the listed bots do not play: no fighting, no batteries, no zone ways. Each bot works
//! through its tries in turn: walks to the try's start point by the ordinary route and motor (*Go*),
//! stands still facing its heading (*Settle*), with two worlds asked for so that its rest position is
//! exact, plans its keys from that position, holds them tick by tick and presses jump at the planned
//! tick (*Run*), then records worlds for a while (*Record*) and logs one line for the try.
//!
//! The truth is the owner's client: the server asks it for worlds (0xF6) every `burst_ms` ms around a
//! try; each upload carries its own tick, and its rows (position, velocity vector, jump type, keys,
//! yaw) are logged and, when a try ends, summarised. The bots' reckoning is not used for a result.
//!
//! The try file (tab or space separated; `#` starts a comment; re-read when a try starts, so tries
//! can be added, changed or struck while the match runs):
//!
//! ```text
//! id  bot  site  edge  angle  face  runup  speed  mid  repeat
//! ```
//!
//! - `site`: a name of [`SITES`], a line of `sites.tsv` beside the try file, or
//!   `x,y,z,tx,tz,rise,face` (the deck floor next to the face, the unit direction from the deck to the
//!   ledge, the ledge's rise, and the distance from that floor to the face along it);
//! - `edge` (m): the distance of the body's axis to the face at the press, along the face's normal; a
//!   negative edge is a run with no jump (calibration; walking into a face);
//! - `angle` (degrees, signed, positive to the body's left): the approach direction against the face's
//!   normal;
//! - `face`: `along` (the body faces the approach direction) or `normal` (it faces the face and the
//!   key diagonal F+L / F+R, with an angle of +-45, makes the approach);
//! - `runup` (m): the distance run (along the path) before the press; `speed`: `walk` or `sprint`;
//! - `mid`: the keys after the take-off: `hold`, `release_f`, `release_all`, `left`, `right`, `back`,
//!   `crouch`, `turn+<deg>` / `turn-<deg>`, `none` (= `hold`);
//! - `repeat`: how many times.
//!
//! The `bot` column is a place number, or `*` (or `any`): a pool try, taken by whichever bot of the match
//! is free. Every bot of the match works the pool at once; on a wide face the tries at one site take
//! lanes along it, at least 2 m apart and inside the ledge's width (`half` of the site: the 6th field of
//! [`SITES`], the 9th of a `sites.tsv` line `name x y z tx tz rise face half`) less what the angled
//! approach drifts sideways; a try with no free lane waits for the next one. Bots with no try rest (no
//! fire, no plays) unless `SRP_JUMP_TEST_IDLE_OTHERS=0`; `SRP_JUMP_TEST_MAX_ACTIVE=<n>` caps the tries
//! under way at once (the other bots rest). The log line of a try carries its `lane`.
//!
//! A bot that starts a try whose walk back to the start is longer than the respawn delay (8 s) plus
//! the walk from a spawn of its team (the mean over up to three spawns, both walks at the steady
//! speed along the grid route, a margin of 2 s) kills itself: the suicide bit (24) of the input mask,
//! which hits the player's own head (`client_patch.md` 3.32, PROVEN in the code, not run in a game).
//! The next world must show it dead; if it is alive 1.5 s later the log has `suicide_failed` and it
//! walks, without a second try. A suicide is logged with `route_back_s`, `respawn_s` and `saved_s`.
//! A team deathmatch charges a ticket; the test holds the end of the match.
//!
//! `speed` may also be `motor` or `motor_run` (the ordinary drive, walking or running): the bot is sent
//! from the start to the top of the ledge by the ordinary route and motor, nothing is scripted, and the
//! log line of the try carries a `motor` object: each press the motor made (`by`, the route press's
//! take-off, landing, try number, distance before the take-off and facing error, the decision's keys),
//! the route's end (the top point) and its age at the press, and whether the top was reached.
//! `edge`, `angle`, `face`, `runup` and `mid` only place the start then.
//!
//! `mid` also has `crouch_jump` (crouch held from the press) and `crouch_pre` (crouch held from four
//! ticks before the press): the crouch jump of `grid-record.md` audit item 2.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use mapdata::{Vec3, World};
use survarium::v026g::match_msgs::TICK_MS;
use survarium::v026g::match_msgs::motion::Motion;
use survarium::v026g::match_msgs::players;

use super::encode::Decision;
use super::kin::{TICK_S, forward, wrap, yaw_of};
use super::plays::{Go, Intent, Pace};
use super::view::{MatchView, Place};
use super::Bot;
use super::motor::RoutePress;
use super::path;
use crate::v026g::drive::key;

/// Worlds are asked for this often around a try, ms (`SRP_JUMP_TEST_BURST_MS`).
const BURST_MS: u32 = 120;
/// The settle: still, facing the heading, at least this long, ms.
const SETTLE_MS: u32 = 1_200;
/// The settle is given up after this long, ms (the try is redone from Go).
const SETTLE_MAX_MS: u32 = 5_000;
/// The walk to the start is given up after this long, ms.
const GO_MAX_MS: u32 = 90_000;
/// A body this close to its start (m) has arrived.
const ARRIVED_M: f32 = 0.6;
/// Worlds are recorded this long after the press, ms.
const RECORD_AFTER_MS: u32 = 3_000;
/// The keys are held until this long after the press, ms.
const KEYS_AFTER_MS: u32 = 1_500;
/// The mid-air keys begin this long after the press (about the take-off), ms.
const MID_AT_MS: u32 = 250;
/// Ticks of lead before a plan's first key tick.
const LEAD_TICKS: u32 = 4;
/// A body rests this long before a sprint try, ms (the stamina).
const REST_BEFORE_SPRINT_MS: u32 = 3_000;
/// Rows kept per place for the summaries.
const ROWS_KEPT: usize = 900;
/// A request not answered after this long is forgotten, ms.
const OUTSTANDING_MS: u32 = 600;
/// A motor-driven try is given up after this long on its way to the top, ms.
const MOTOR_MAX_MS: u32 = 25_000;
/// How far past the face, along its normal, a motor-driven try's top point is, m.
const TOP_BEYOND_M: f32 = 1.2;
/// The respawn delay of every mode's config (`respawn_time`), s (`docs/notes/modes_maps.md`).
const RESPAWN_S: f32 = 8.0;
/// A suicide has to save this much over the estimate to be worth it, s (the estimate is a walk at
/// the steady speed over the route's length: jumps and drops are not in it).
const SUICIDE_MARGIN_S: f32 = 2.0;
/// The suicide bit is sent this many ticks.
const SUICIDE_TICKS: u32 = 3;
/// A bot still alive this long after the suicide bit walks to the start instead, ms.
const SUICIDE_WAIT_MS: u32 = 1_500;
/// The steady walking speed of the estimates, m/s (the `Kinematics` default).
const WALK_MPS: f32 = 3.5;

/// One row of a world: what the owner's client says of one body at one tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The upload's own tick (the match clock).
    pub time: u32,
    pub place: Place,
    pub alive: bool,
    /// The feet, map frame.
    pub feet: Vec3,
    pub jumping: bool,
    pub jump_type: u8,
    pub speed: f32,
    /// `fall_and_slide_velocity`, map frame.
    pub velocity: [f32; 3],
    pub normal: [f32; 3],
    pub keys: u32,
    pub yaw: f32,
    pub pitch: f32,
    pub sprinting: bool,
    pub crouch: bool,
}

/// A site: a deck, a face and the ledge's rise.
#[derive(Clone, Debug, PartialEq)]
pub struct Site {
    pub name: String,
    /// The deck floor next to the face (x, y, z).
    pub lower: Vec3,
    /// The unit direction from the deck to the ledge, ground plane.
    pub toward: [f32; 2],
    /// The ledge's rise, m.
    pub rise: f32,
    /// From `lower` to the face along `toward`, m (half the gap to the lip until a sweep measures it).
    pub face: f32,
    /// Half the ledge's width along the face, m: the lanes of parallel tries stay inside it.
    pub half: f32,
}

impl Site {
    /// The point of the face the approach line aims at, ground plane.
    fn centre(&self) -> [f32; 2] {
        [
            self.lower.x + self.toward[0] * self.face,
            self.lower.z + self.toward[1] * self.face,
        ]
    }
}

/// A [`SITES`] row: name, deck floor, direction to the ledge, rise, face distance, half width.
type SiteRow = (&'static str, [f32; 3], [f32; 2], f32, f32, f32);

/// The sites of `grid-record.md` J0 section 2: name, deck floor, direction to the ledge, rise, face
/// distance (half the gap to the lip). `gridtool ledges` of the F1 branch's grid.
pub const SITES: &[SiteRow] = &[
    ("chem125a", [-78.9, 4.35, 47.1], [0.0, -1.0], 1.259, 0.25, 6.10),
    ("chem125b", [-79.9, 4.35, 70.4], [0.0, 1.0], 1.274, 0.25, 5.00),
    ("chem125c", [-84.9, 4.35, 58.9], [-1.0, 0.0], 1.259, 0.25, 11.75),
    ("chem060a", [-6.4, 1.72, 93.9], [1.0, 0.0], 0.632, 0.25, 0.60),
    ("chem060b", [30.6, 1.91, 127.1], [0.0, 1.0], 0.590, 0.15, 0.60),
    ("chem110", [21.4, 2.03, 128.4], [0.0, 1.0], 1.106, 0.25, 0.50),
    ("vect090a", [31.8, 24.46, 0.5], [-1.0, 0.0], 0.925, 0.25, 0.50),
    ("vect090b", [-12.2, 20.89, -27.3], [1.0, 0.0], 0.899, 0.25, 0.60),
    ("vect120", [-88.7, 20.94, -9.0], [-1.0, 0.0], 1.199, 0.25, 0.90),
    ("colo105", [-80.8, 35.16, 42.9], [0.0, -1.0], 1.063, 0.25, 0.75),
    ("colo100a", [39.2, 31.98, 53.1], [-1.0, 0.0], 1.007, 0.25, 0.50),
    ("colo100b", [9.2, 31.98, 52.6], [1.0, 0.0], 1.009, 0.25, 0.50),
    ("scho100a", [58.5, 15.57, 97.7], [-1.0, 0.0], 0.999, 0.25, 0.50),
    ("scho100b", [24.4, 16.00, 47.7], [1.0, 0.0], 1.017, 0.25, 1.00),
    ("scho110", [13.9, 16.04, 48.0], [0.0, 1.0], 1.092, 0.25, 0.50),
    ("scho125a", [26.2, 15.76, 47.2], [-1.0, 0.0], 1.239, 0.25, 0.75),
    ("scho125b", [3.0, 16.32, 77.0], [0.0, 1.0], 1.252, 0.25, 1.00),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Face {
    Along,
    Normal,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Speed {
    Walk,
    Sprint,
    /// The ordinary drive at a walk (the motor presses the jumps).
    Motor,
    /// The ordinary drive at a run.
    MotorRun,
}

impl Speed {
    /// Whether the keys are the motor's, not the plan's.
    fn is_motor(self) -> bool {
        matches!(self, Self::Motor | Self::MotorRun)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mid {
    Hold,
    ReleaseF,
    ReleaseAll,
    Left,
    Right,
    Back,
    Crouch,
    /// Crouch held from the press (a crouch jump).
    CrouchJump,
    /// Crouch held from four ticks before the press.
    CrouchPre,
    /// The yaw turned by this many degrees over 0.4 s from the take-off.
    Turn(f32),
}

/// Who works a try: a named bot, or whichever bot of the match is free (the pool).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Who {
    Bot(Place),
    Pool,
}

/// One line of the try file.
#[derive(Clone, Debug, PartialEq)]
pub struct TryDef {
    pub id: String,
    pub who: Who,
    /// The lateral shift of the site along its face given to a pool try, m (0 in the file).
    pub lane: f32,
    pub site: Site,
    pub edge: f32,
    pub angle: f32,
    pub face: Face,
    pub runup: f32,
    pub speed: Speed,
    pub mid: Mid,
    pub repeat: u32,
}

fn named_site(name: &str, extra: &HashMap<String, Site>) -> Option<Site> {
    if let Some(site) = extra.get(name) {
        return Some(site.clone());
    }
    SITES
        .iter()
        .find(|s| s.0 == name)
        .map(|&(name, lower, toward, rise, face, half)| Site {
            name: name.to_owned(),
            lower: Vec3::new(lower[0], lower[1], lower[2]),
            toward,
            rise,
            face,
            half,
        })
}

fn parse_site(text: &str, extra: &HashMap<String, Site>) -> Result<Site, String> {
    if let Some(site) = named_site(text, extra) {
        return Ok(site);
    }
    let v: Vec<f32> = text
        .split(',')
        .map(|s| s.trim().parse::<f32>().map_err(|e| format!("site {text:?}: {e}")))
        .collect::<Result<_, _>>()?;
    if v.len() != 7 {
        return Err(format!("site {text:?}: a name, or x,y,z,tx,tz,rise,face"));
    }
    let length = v[3].hypot(v[4]);
    if length < 0.5 {
        return Err(format!("site {text:?}: no direction"));
    }
    Ok(Site {
        name: "free".to_owned(),
        lower: Vec3::new(v[0], v[1], v[2]),
        toward: [v[3] / length, v[4] / length],
        rise: v[5],
        face: v[6],
        half: 0.0,
    })
}

fn parse_mid(text: &str) -> Result<Mid, String> {
    Ok(match text {
        "hold" | "none" => Mid::Hold,
        "release_f" => Mid::ReleaseF,
        "release_all" => Mid::ReleaseAll,
        "left" => Mid::Left,
        "right" => Mid::Right,
        "back" => Mid::Back,
        "crouch" => Mid::Crouch,
        "crouch_jump" => Mid::CrouchJump,
        "crouch_pre" => Mid::CrouchPre,
        other => match other.strip_prefix("turn") {
            Some(degrees) => Mid::Turn(degrees.parse::<f32>().map_err(|e| format!("mid {other:?}: {e}"))?),
            None => return Err(format!("mid {other:?}")),
        },
    })
}

/// The tries of a try file's text. A line that does not parse is an error naming it.
pub fn parse_tries(text: &str, extra: &HashMap<String, Site>) -> Result<Vec<TryDef>, String> {
    let mut tries = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        let fail = |why: String| format!("line {}: {why}", number + 1);
        if f.len() != 10 {
            return Err(fail(format!("{} fields, want 10: id bot site edge angle face runup speed mid repeat", f.len())));
        }
        let num = |i: usize, what: &str| f[i].parse::<f32>().map_err(|e| fail(format!("{what} {:?}: {e}", f[i])));
        tries.push(TryDef {
            id: f[0].to_owned(),
            who: match f[1] {
                "*" | "any" => Who::Pool,
                other => Who::Bot(other.parse().map_err(|e| fail(format!("bot {other:?}: {e}")))?),
            },
            lane: 0.0,
            site: parse_site(f[2], extra).map_err(fail)?,
            edge: num(3, "edge")?,
            angle: num(4, "angle")?,
            face: match f[5] {
                "along" => Face::Along,
                "normal" => Face::Normal,
                other => return Err(fail(format!("face {other:?}"))),
            },
            runup: num(6, "runup")?,
            speed: match f[7] {
                "walk" => Speed::Walk,
                "sprint" => Speed::Sprint,
                "motor" => Speed::Motor,
                "motor_run" => Speed::MotorRun,
                other => return Err(fail(format!("speed {other:?}"))),
            },
            mid: parse_mid(f[8]).map_err(fail)?,
            repeat: f[9].parse().map_err(|e| fail(format!("repeat {:?}: {e}", f[9])))?,
        });
    }
    Ok(tries)
}

/// The sites of a `sites.tsv`: `name x y z tx tz rise face` per line.
fn parse_sites(text: &str) -> HashMap<String, Site> {
    let mut sites = HashMap::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() != 8 && f.len() != 9 {
            continue;
        }
        let v: Vec<f32> = f[1..].iter().filter_map(|s| s.parse().ok()).collect();
        if v.len() != 7 && v.len() != 8 {
            continue;
        }
        let length = v[3].hypot(v[4]);
        if length < 0.5 {
            continue;
        }
        sites.insert(
            f[0].to_owned(),
            Site {
                name: f[0].to_owned(),
                lower: Vec3::new(v[0], v[1], v[2]),
                toward: [v[3] / length, v[4] / length],
                rise: v[5],
                face: v[6],
                half: v.get(7).copied().unwrap_or(0.0),
            },
        );
    }
    sites
}

/// The distance a body runs from rest in `ticks` ticks holding forward (`sprint`: with sprint): the
/// `Kinematics` blend (a linear ramp to the speed over `blend_ticks`), or the measured table
/// `kinematics.tsv` (`walk|sprint <ticks> <metres>`) of J0-0 when there is one.
#[derive(Clone, Debug, Default)]
pub struct RunTable {
    measured: [Vec<(u32, f32)>; 2],
}

impl RunTable {
    fn parse(text: &str) -> Self {
        let mut table = Self::default();
        for line in text.lines() {
            let f: Vec<&str> = line.split('#').next().unwrap_or("").split_whitespace().collect();
            if f.len() != 3 {
                continue;
            }
            let kind = match f[0] {
                "walk" => 0,
                "sprint" => 1,
                _ => continue,
            };
            if let (Ok(t), Ok(d)) = (f[1].parse::<u32>(), f[2].parse::<f32>()) {
                table.measured[kind].push((t, d));
            }
        }
        for rows in &mut table.measured {
            rows.sort_by_key(|r| r.0);
        }
        table
    }

    pub fn distance(&self, speed: Speed, ticks: u32) -> f32 {
        let kind = usize::from(speed == Speed::Sprint);
        let rows = &self.measured[kind];
        if let Some(i) = rows.iter().position(|r| r.0 >= ticks) {
            if rows[i].0 == ticks || i == 0 {
                return rows[i].1;
            }
            let (a, b) = (rows[i - 1], rows[i]);
            return a.1 + (b.1 - a.1) * (ticks - a.0) as f32 / (b.0 - a.0) as f32;
        }
        if let Some(&(t, d)) = rows.last() {
            let v = if speed == Speed::Sprint { 5.0 } else { 3.5 };
            return d + v * (ticks - t) as f32 * TICK_S;
        }
        let kin = super::kin::Kinematics::default();
        let v = if speed == Speed::Sprint { kin.sprint } else { kin.walk[0] };
        // The blend: a linear ramp over `blend_ticks`, then steady.
        let ramp = kin.blend_ticks.max(1);
        let mut d = 0.0;
        for t in 1..=ticks {
            d += v * (t.min(ramp) as f32 / ramp as f32) * TICK_S;
        }
        d
    }

    /// The tick count whose distance is nearest `metres`.
    pub fn ticks_for(&self, speed: Speed, metres: f32) -> u32 {
        let mut best = (0u32, f32::MAX);
        for t in 0..400 {
            let error = (self.distance(speed, t) - metres).abs();
            if error < best.1 {
                best = (t, error);
            }
        }
        best.0
    }
}

/// A try's keys over time, from the rest position.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The rest position and the facing it was planned from.
    pub rest: Vec3,
    pub heading: f32,
    /// The first key tick (match ms) and the press tick; none: no jump.
    pub first: u32,
    pub press: Option<u32>,
    /// The tick the keys end.
    pub end: u32,
    base: u32,
    mid: Mid,
    /// The planned distance run before the press, m.
    pub run: f32,
    pub edge_planned: f32,
}

/// The base keys of a try: forward, with sprint, and the diagonal of a `normal` face.
fn base_keys(def: &TryDef) -> u32 {
    let mut keys = key::FORWARD;
    if def.speed == Speed::Sprint {
        keys |= key::SPRINT;
    }
    if def.face == Face::Normal {
        keys |= if def.angle > 0.0 { key::LEFT } else { key::RIGHT };
    }
    keys
}

/// The unit direction of the approach: the direction to the ledge turned by the angle (positive: to
/// the body's left, `x cos - z sin, x sin + z cos`).
fn approach(def: &TryDef) -> [f32; 2] {
    let th = def.angle.to_radians();
    let [x, z] = def.site.toward;
    [x * th.cos() - z * th.sin(), x * th.sin() + z * th.cos()]
}

/// The heading (yaw) the body faces for a try.
fn heading_of(def: &TryDef) -> f32 {
    match def.face {
        Face::Along => yaw_of(approach(def)),
        Face::Normal => yaw_of(def.site.toward),
    }
}

/// The start point of a try: `runup` before the press on the approach line that meets the face's
/// centre, the press `edge` from the face.
pub fn start_of(def: &TryDef) -> Vec3 {
    let a = approach(def);
    let [cx, cz] = def.site.centre();
    let cos = (a[0] * def.site.toward[0] + a[1] * def.site.toward[1]).max(0.2);
    let back = def.edge.max(0.0) / cos + def.runup;
    Vec3::new(cx - a[0] * back, def.site.lower.y, cz - a[1] * back)
}

/// The top of a try's ledge, past the face: where a motor-driven try is sent.
fn top_of(def: &TryDef) -> Vec3 {
    let [cx, cz] = def.site.centre();
    let t = def.site.toward;
    Vec3::new(cx + t[0] * TOP_BEYOND_M, def.site.lower.y + def.site.rise, cz + t[1] * TOP_BEYOND_M)
}

/// The plan from a rest position at `heading`, the first key tick `first` (None: the geometry does
/// not fit: the body is not where the try needs it).
pub fn plan_from(def: &TryDef, rest: Vec3, heading: f32, first: u32, table: &RunTable) -> Option<Plan> {
    let a = approach(def);
    let t = def.site.toward;
    let [cx, cz] = def.site.centre();
    let cos = a[0] * t[0] + a[1] * t[1];
    if cos < 0.2 {
        return None;
    }
    // Along the approach, from the rest position to the face line.
    let to_face = ((cx - rest.x) * t[0] + (cz - rest.z) * t[1]) / cos;
    let (run, press) = if def.edge < 0.0 {
        (def.runup, None)
    } else {
        let run = to_face - def.edge / cos;
        if run < 0.4 {
            return None;
        }
        (run, Some(()))
    };
    let ticks = table.ticks_for(def.speed, run);
    let press_at = first + ticks * TICK_MS;
    let end = match press {
        Some(()) => press_at + KEYS_AFTER_MS,
        None => press_at + 600,
    };
    Some(Plan {
        rest,
        heading,
        first,
        press: press.map(|_| press_at),
        end,
        base: base_keys(def),
        mid: def.mid,
        run: table.distance(def.speed, ticks),
        edge_planned: def.edge,
    })
}

impl Plan {
    /// The keys and the yaw turn of the tick at `time`.
    pub fn at(&self, time: u32) -> Decision {
        if time < self.first || time >= self.end {
            return Decision::default();
        }
        let Some(press) = self.press else {
            return Decision { keys: if time < self.end - 600 { self.base } else { 0 }, turn: (0.0, 0.0) };
        };
        if time < press {
            let crouch = self.mid == Mid::CrouchPre && time + 4 * TICK_MS >= press;
            return Decision { keys: self.base | if crouch { key::CROUCH } else { 0 }, turn: (0.0, 0.0) };
        }
        // The press: jump held for two ticks.
        let mut keys = self.base;
        if matches!(self.mid, Mid::CrouchJump | Mid::CrouchPre) {
            keys |= key::CROUCH;
        }
        if time < press + 2 * TICK_MS {
            keys |= key::JUMP;
        }
        let mid_from = press + MID_AT_MS;
        let mut turn = 0.0;
        if time >= mid_from {
            match self.mid {
                Mid::Hold => {}
                Mid::ReleaseF => keys &= !(key::FORWARD | key::SPRINT),
                Mid::ReleaseAll => keys = 0,
                Mid::Left => keys |= key::LEFT,
                Mid::Right => keys |= key::RIGHT,
                Mid::Back => keys = (keys & !(key::FORWARD | key::SPRINT)) | key::BACK,
                Mid::Crouch => keys |= key::CROUCH,
                Mid::CrouchJump | Mid::CrouchPre => {}
                Mid::Turn(degrees) => {
                    // Eight ticks (0.4 s).
                    if time < mid_from + 8 * TICK_MS {
                        turn = degrees.to_radians() / 8.0;
                    }
                }
            }
        }
        Decision { keys, turn: (turn, 0.0) }
    }
}

/// The estimate that made a bot kill itself: the walk back to a try's start against the respawn
/// and the walk from a spawn.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Suicide {
    back_s: f32,
    respawn_s: f32,
}

impl Suicide {
    /// When the walk back (`back_s`) is longer than the respawn delay plus the walk from a spawn
    /// (`spawn_s`) by the margin.
    fn when(back_s: f32, spawn_s: f32) -> Option<Self> {
        let respawn_s = RESPAWN_S + spawn_s;
        (back_s > respawn_s + SUICIDE_MARGIN_S).then_some(Self { back_s, respawn_s })
    }

    fn json(self, def: &TryDef, place: Place, what: &str) -> serde_json::Value {
        serde_json::json!({
            "id": def.id, "bot": place, what: true, "route_back_s": self.back_s,
            "respawn_s": self.respawn_s, "saved_s": self.back_s - self.respawn_s,
        })
    }
}

/// The walk time over the grid between two points, s.
fn route_s(world: &World, from: Vec3, to: Vec3) -> Option<f32> {
    let a = path::goal_floor(&world.grid, from)?;
    let b = path::goal_floor(&world.grid, to)?;
    path::plan(&world.grid, a, b).map(|route| route.length() / WALK_MPS)
}

/// Whether a bot at `from` does better to die and respawn than to walk to the start of `def`: the
/// walk from up to three of its team's spawns stands for the respawn point (it is a random one).
fn suicide_estimate(world: &World, team: u8, from: Vec3, def: &TryDef) -> Option<Suicide> {
    let start = start_of(def);
    let back_s = route_s(world, from, start)?;
    let own: Vec<Vec3> = world.map.spawns.iter().filter(|s| s.team == i32::from(team)).map(|s| s.position).collect();
    let spawns: Vec<Vec3> = if own.is_empty() { world.map.spawns.iter().map(|s| s.position).collect() } else { own };
    let times: Vec<f32> = spawns.iter().step_by((spawns.len() / 3).max(1)).filter_map(|&at| route_s(world, at, start)).collect();
    if times.is_empty() {
        return None;
    }
    Suicide::when(back_s, times.iter().sum::<f32>() / times.len() as f32)
}

#[derive(Clone, Debug)]
enum Phase {
    /// The bit of the suicide is sent; the next world should show the body dead.
    Suicide { since: u32, est: Suicide },
    /// Walking to the start by the ordinary route.
    Go { since: u32 },
    /// Standing still facing the heading.
    Settle { since: u32, asked: bool },
    /// The keys.
    Run(Plan),
    /// The ordinary drive from the start to the top (`since`: when this began); the plan holds the
    /// first press seen.
    Motor { plan: Plan, since: u32 },
    /// Recording worlds, then the result.
    Record { plan: Plan, until: u32 },
    /// Resting before the next try (the stamina).
    Rest { until: u32 },
}

#[derive(Clone, Debug)]
struct Active {
    def: TryDef,
    /// Which repeat of the try this is (1-based).
    n: u32,
    phase: Phase,
    /// How many times the start was redone.
    redone: u8,
    /// What the bot did first at this try (the match clock), for the log.
    began: u32,
    /// Motor-driven tries: each press the motor made, and whether the top was reached.
    presses: Vec<serde_json::Value>,
    reached: Option<bool>,
}

#[derive(Default)]
struct Runner {
    active: Option<Active>,
    /// The tries to come, in file order, with the repeats done of each.
    pending: VecDeque<TryDef>,
}

/// The mode's state in a match.
pub struct JumpTest {
    path: PathBuf,
    dir: PathBuf,
    stamp: String,
    burst_ms: u32,
    modified: Option<std::time::SystemTime>,
    runners: HashMap<Place, Runner>,
    /// The tries of any bot, with repeats left, in file order.
    pool: VecDeque<TryDef>,
    /// Bots not in the test rest (no fire, no plays); `SRP_JUMP_TEST_IDLE_OTHERS=0` lets them play.
    idle_others: bool,
    /// At most this many tries under way at once (`SRP_JUMP_TEST_MAX_ACTIVE`).
    max_active: usize,
    /// Repeats done by try id.
    done: HashMap<String, u32>,
    /// Tries started by id (the `n` of the log; a try redone after a death counts again).
    started: HashMap<String, u32>,
    /// Tries of the file, by id, as last read.
    known: HashMap<String, TryDef>,
    table: RunTable,
    rows: HashMap<Place, VecDeque<Row>>,
    tries_log: Option<fs::File>,
    worlds_log: Option<fs::File>,
    /// The last world request and whether it is answered; upload times of the burst.
    last_request: Option<u32>,
    outstanding: Option<u32>,
    uploads: Vec<(u32, u32, usize)>,
    sent: u32,
    problem: Option<String>,
}

impl JumpTest {
    /// `SRP_JUMP_TEST=<try file>` names the list; unset or empty: no mode.
    pub fn from_env(match_id: u32) -> Option<Self> {
        let path = std::env::var_os("SRP_JUMP_TEST").filter(|p| !p.is_empty())?;
        let dir = std::env::var_os("SRP_JUMP_TEST_LOG")
            .filter(|p| !p.is_empty())
            .map_or_else(|| PathBuf::from("logs/jump-test"), PathBuf::from);
        let burst_ms = std::env::var("SRP_JUMP_TEST_BURST_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&v: &u32| v >= TICK_MS)
            .unwrap_or(BURST_MS);
        let mut test = Self::new(PathBuf::from(path), dir, format!("{match_id:08x}"), burst_ms);
        test.idle_others = !matches!(std::env::var("SRP_JUMP_TEST_IDLE_OTHERS").as_deref(), Ok("0" | "off" | "false"));
        test.max_active = std::env::var("SRP_JUMP_TEST_MAX_ACTIVE")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&v: &usize| v > 0)
            .unwrap_or(usize::MAX);
        Some(test)
    }

    pub fn new(path: PathBuf, dir: PathBuf, stamp: String, burst_ms: u32) -> Self {
        Self {
            path,
            dir,
            stamp,
            burst_ms,
            modified: None,
            runners: HashMap::new(),
            pool: VecDeque::new(),
            idle_others: true,
            max_active: usize::MAX,
            done: HashMap::new(),
            started: HashMap::new(),
            known: HashMap::new(),
            table: RunTable::default(),
            rows: HashMap::new(),
            tries_log: None,
            worlds_log: None,
            last_request: None,
            outstanding: None,
            uploads: Vec::new(),
            sent: 0,
            problem: None,
        }
    }

    fn open(&self, name: &str) -> Option<fs::File> {
        fs::create_dir_all(&self.dir).ok()?;
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join(format!("jump-test-{}-{name}.jsonl", self.stamp)))
            .ok()
    }

    fn line(file: &mut Option<fs::File>, value: &serde_json::Value) {
        if let Some(file) = file.as_mut() {
            let _ = writeln!(file, "{value}");
        }
    }

    /// The try file, when it changed: new tries are queued, changed or struck ones not yet started
    /// are replaced or dropped.
    fn reload(&mut self) {
        let Ok(meta) = fs::metadata(&self.path) else {
            return;
        };
        let modified = meta.modified().ok();
        if modified.is_some() && modified == self.modified {
            return;
        }
        self.modified = modified;
        let Ok(text) = fs::read_to_string(&self.path) else {
            return;
        };
        let beside = |name: &str| self.path.with_file_name(name);
        let extra = fs::read_to_string(beside("sites.tsv")).map(|t| parse_sites(&t)).unwrap_or_default();
        if let Ok(t) = fs::read_to_string(beside("kinematics.tsv")) {
            self.table = RunTable::parse(&t);
        }
        match parse_tries(&text, &extra) {
            Err(why) => {
                if self.problem.as_deref() != Some(&why) {
                    log::warn!("jump test: the try file {}: {why}", self.path.display());
                    self.problem = Some(why);
                }
            }
            Ok(tries) => {
                self.problem = None;
                self.known = tries.iter().map(|t| (t.id.clone(), t.clone())).collect();
                // Rebuild each runner's pending list: the file's tries of the bot with repeats left.
                let mut by_bot: HashMap<Place, VecDeque<TryDef>> = HashMap::new();
                let mut pool = VecDeque::new();
                for def in tries {
                    let done = self.done.get(&def.id).copied().unwrap_or(0);
                    let running = self
                        .runners
                        .values()
                        .filter_map(|r| r.active.as_ref())
                        .filter(|a| a.def.id == def.id)
                        .count() as u32;
                    let left = def.repeat.saturating_sub(done + running);
                    if left > 0 {
                        let def = TryDef { repeat: left, ..def };
                        match def.who {
                            Who::Bot(place) => by_bot.entry(place).or_default().push_back(def),
                            Who::Pool => pool.push_back(def),
                        }
                    }
                }
                self.pool = pool;
                for (place, runner) in &mut self.runners {
                    runner.pending = by_bot.remove(place).unwrap_or_default();
                }
                for (place, pending) in by_bot {
                    self.runners.entry(place).or_default().pending = pending;
                }
                if self.tries_log.is_none() {
                    self.tries_log = self.open("tries");
                    self.worlds_log = self.open("worlds");
                }
            }
        }
    }

    /// Whether the match must not end: some try is under way or still to come.
    pub fn holds_end(&self) -> bool {
        !self.pool.is_empty()
            || self
                .runners
                .values()
                .any(|r| r.active.is_some() || !r.pending.is_empty())
    }

    /// Tries under way.
    fn active_count(&self) -> usize {
        self.runners.values().filter(|r| r.active.is_some()).count()
    }

    /// The next pool try a free bot can take: the first with a free lane on its face (the lane is set
    /// and the site shifted along the face); a try whose faces are all taken waits.
    fn take_from_pool(&mut self) -> Option<TryDef> {
        let taken: Vec<(Vec3, f32)> = self
            .runners
            .values()
            .filter_map(|r| r.active.as_ref())
            .map(|a| (base_of(&a.def), a.def.lane))
            .collect();
        let (index, lane) = self
            .pool
            .iter()
            .enumerate()
            .find_map(|(i, def)| free_lane(def, &taken).map(|lane| (i, lane)))?;
        let mut def = self.pool[index].clone();
        self.pool[index].repeat -= 1;
        if self.pool[index].repeat == 0 {
            self.pool.remove(index);
        }
        let perp = [-def.site.toward[1], def.site.toward[0]];
        def.repeat = 1;
        def.lane = lane;
        def.site.lower = Vec3::new(def.site.lower.x + perp[0] * lane, def.site.lower.y, def.site.lower.z + perp[1] * lane);
        Some(def)
    }

    /// Whether this bot's place is in the test.
    pub fn owns(&self, place: Place) -> bool {
        self.runners.contains_key(&place)
    }

    /// The think of a bot in the mode: the intent (a walk to the start while going, else standing).
    /// Returns whether the bot is the test's.
    pub fn think(&mut self, bot: &mut Bot, view: &MatchView, world: &World) -> bool {
        self.reload();
        let now = view.clock;
        if !self.runners.contains_key(&bot.place) {
            if self.pool.is_empty() || self.active_count() >= self.max_active {
                if self.idle_others {
                    bot.intent = Intent::stay();
                    bot.intent.fire = false;
                }
                return self.idle_others;
            }
            self.runners.insert(bot.place, Runner::default());
        }
        let free = self.runners.get(&bot.place).is_some_and(|r| r.active.is_none() && r.pending.is_empty());
        if free
            && self.active_count() < self.max_active
            && let Some(def) = self.take_from_pool()
            && let Some(runner) = self.runners.get_mut(&bot.place)
        {
            runner.pending.push_back(def);
        }
        let Some(runner) = self.runners.get_mut(&bot.place) else {
            return false;
        };
        let Some(me) = view.place(bot.place).filter(|p| p.alive) else {
            // Dead: the try is lost with the life; it is redone from the start.
            if let Some(active) = runner.active.take() {
                if let Phase::Suicide { est, .. } = active.phase {
                    Self::line(&mut self.tries_log, &est.json(&active.def, bot.place, "suicide"));
                }
                let def = TryDef { repeat: 1, ..active.def };
                runner.pending.push_front(def);
            }
            bot.intent = Intent::stay();
            return true;
        };
        bot.intent = Intent::stay();
        bot.intent.fire = false;
        // A new try.
        if runner.active.is_none()
            && let Some(first) = runner.pending.front_mut()
        {
            let def = TryDef { repeat: 1, ..first.clone() };
            first.repeat -= 1;
            if first.repeat == 0 {
                runner.pending.pop_front();
            }
            let n = {
                let started = self.started.entry(def.id.clone()).or_default();
                *started += 1;
                *started
            };
            let sprints = matches!(def.speed, Speed::Sprint | Speed::MotorRun);
            let rest = match suicide_estimate(world, bot.team, me.feet, &def) {
                Some(est) => Phase::Suicide { since: now, est },
                None if sprints => Phase::Rest { until: now + REST_BEFORE_SPRINT_MS },
                None => Phase::Go { since: now },
            };
            runner.active = Some(Active { def, n, phase: rest, redone: 0, began: now, presses: Vec::new(), reached: None });
        }
        let Some(active) = runner.active.as_mut() else {
            return true;
        };
        let start = start_of(&active.def);
        match &mut active.phase {
            Phase::Suicide { since, est } => {
                if now > *since + SUICIDE_WAIT_MS {
                    // Still alive (a helmet took the hit): no second try, the bot walks.
                    Self::line(&mut self.tries_log, &est.json(&active.def, bot.place, "suicide_failed"));
                    active.phase = Phase::Go { since: now };
                }
            }
            Phase::Rest { until } => {
                if now >= *until {
                    active.phase = Phase::Go { since: now };
                }
            }
            Phase::Go { since } => {
                if me.feet.distance_xz(start) <= ARRIVED_M && me.velocity[0].hypot(me.velocity[1]) < 0.6 {
                    active.phase = if active.def.speed.is_motor() {
                        let plan = Plan {
                            rest: me.feet,
                            heading: me.yaw,
                            first: now,
                            press: None,
                            end: now,
                            base: 0,
                            mid: Mid::Hold,
                            run: 0.0,
                            edge_planned: active.def.edge,
                        };
                        Phase::Motor { plan, since: now }
                    } else {
                        Phase::Settle { since: now, asked: false }
                    };
                } else if now > *since + GO_MAX_MS {
                    log::warn!("jump test: bot {} could not walk to the start of {}", bot.place, active.def.id);
                    *since = now;
                } else {
                    bot.intent = Intent {
                        go: Go::To { at: start, within_m: 0.3 },
                        pace: Pace::Walk,
                        fire: false,
                        ..Intent::stay()
                    };
                }
            }
            Phase::Motor { plan, since } => {
                let top = top_of(&active.def);
                let reached = me.feet.distance_xz(top) <= 0.6 && (me.feet.y - top.y).abs() < 0.4;
                if reached || now > *since + MOTOR_MAX_MS {
                    let plan = Plan { end: now, ..plan.clone() };
                    let until = (plan.press.unwrap_or(now) + RECORD_AFTER_MS).max(now + 1_500);
                    active.reached = Some(reached);
                    active.phase = Phase::Record { plan, until };
                } else {
                    let pace = if active.def.speed == Speed::MotorRun { Pace::Run } else { Pace::Walk };
                    bot.intent = Intent { go: Go::To { at: top, within_m: 0.4 }, pace, fire: false, ..Intent::stay() };
                }
            }
            Phase::Settle { .. } | Phase::Run(_) | Phase::Record { .. } => {}
        }
        true
    }

    /// The scripted decision of a tick of a bot in Settle, Run or Record: None while it walks (the
    /// ordinary drive) or when the bot is not the test's.
    pub fn drive(&mut self, bot: &mut Bot, time: u32, view: &MatchView) -> Option<Decision> {
        let now = view.clock;
        let Some(runner) = self.runners.get_mut(&bot.place) else {
            return self.idle_others.then(Decision::default);
        };
        let Some(active) = runner.active.as_mut() else {
            return Some(Decision::default());
        };
        let me = view.place(bot.place).filter(|p| p.alive)?;
        match &mut active.phase {
            Phase::Rest { .. } => Some(Decision::default()),
            Phase::Suicide { since, .. } => Some(Decision {
                keys: if time >= *since && time < *since + SUICIDE_TICKS * TICK_MS { key::SUICIDE } else { 0 },
                turn: (0.0, 0.0),
            }),
            Phase::Go { .. } | Phase::Motor { .. } => None,
            Phase::Settle { since, asked } => {
                let heading = heading_of(&active.def);
                let turn = wrap(heading - me.yaw).clamp(-0.35, 0.35);
                let elapsed = time.saturating_sub(*since);
                // Worlds while still, from 0.4 s on.
                if elapsed >= 400 {
                    *asked = true;
                }
                // The rest position: the last world of this place at rest, after the settle began.
                let rest = self.rows.get(&bot.place).and_then(|rows| {
                    rows.iter()
                        .rev()
                        .find(|r| r.time >= *since + 600 && r.alive && r.speed < 0.15 && !r.jumping && wrap(r.yaw - heading).abs() < 0.04)
                        .cloned()
                });
                if let Some(row) = rest
                    && elapsed >= SETTLE_MS
                {
                    let first = (time / TICK_MS + LEAD_TICKS) * TICK_MS;
                    match plan_from(&active.def, row.feet, row.yaw, first, &self.table) {
                        Some(plan) => {
                            active.phase = Phase::Run(plan);
                        }
                        None => {
                            if active.redone < 3 {
                                active.redone += 1;
                                log::info!("jump test: {} from {:?}: the rest position does not fit; redone", active.def.id, row.feet);
                                active.phase = Phase::Go { since: now };
                            } else {
                                let line = serde_json::json!({
                                    "id": active.def.id, "n": active.n, "bot": bot.place, "void": "start does not fit",
                                    "rest": row.feet.to_array(),
                                });
                                Self::line(&mut self.tries_log, &line);
                                *self.done.entry(active.def.id.clone()).or_default() += 1;
                                runner.active = None;
                            }
                        }
                    }
                    return Some(Decision::default());
                }
                if elapsed > SETTLE_MAX_MS {
                    active.redone += 1;
                    active.phase = Phase::Go { since: now };
                }
                Some(Decision { keys: 0, turn: (turn, 0.0) })
            }
            Phase::Run(plan) => {
                let decision = plan.at(time);
                if time >= plan.end {
                    let plan = plan.clone();
                    let until = plan.press.unwrap_or(plan.end) + RECORD_AFTER_MS;
                    active.phase = Phase::Record { plan, until };
                }
                Some(decision)
            }
            Phase::Record { plan, until } => {
                if time >= *until {
                    let plan = plan.clone();
                    let def = active.def.clone();
                    let (n, began, redone) = (active.n, active.began, active.redone);
                    let motor = def.speed.is_motor().then(|| {
                        serde_json::json!({
                            "route_end": top_of(&def).to_array(), "reached": active.reached,
                            "presses": std::mem::take(&mut active.presses),
                        })
                    });
                    let place = bot.place;
                    let mut line = self.summary(place, &def, n, &plan, began, redone);
                    if let (Some(motor), Some(obj)) = (motor, line.as_object_mut()) {
                        obj.insert("motor".into(), motor);
                    }
                    Self::line(&mut self.tries_log, &line);
                    *self.done.entry(def.id.clone()).or_default() += 1;
                    if let Some(runner) = self.runners.get_mut(&place) {
                        runner.active = None;
                    }
                    return Some(Decision::default());
                }
                Some(plan.at(time))
            }
        }
    }

    /// A tick of a bot driven by the ordinary drive in a motor-driven try (`keys`: the tick's whole
    /// decision, jump included): the motor's presses are noted.
    pub fn motor_seen(&mut self, bot: &Bot, time: u32, keys: u32) {
        let Some(active) = self.runners.get_mut(&bot.place).and_then(|r| r.active.as_mut()) else {
            return;
        };
        let Phase::Motor { plan, since } = &mut active.phase else {
            return;
        };
        let Some(by) = bot.motor.jumped() else {
            return;
        };
        if keys & key::JUMP == 0 || active.presses.len() >= 24 {
            return;
        }
        plan.press.get_or_insert(time);
        let route = bot.motor.route_press().map(|r: RoutePress| {
            serde_json::json!({
                "take_off": r.take_off.to_array(), "landing": r.landing.to_array(), "try": r.tries,
                "before_m": r.before, "facing": r.facing,
            })
        });
        active.presses.push(serde_json::json!({
            "t": time, "by": format!("{by:?}").to_lowercase(), "press_keys": keys,
            "route_age_ms": time.saturating_sub(*since), "route": route,
        }));
    }

    /// Whether a world should be asked for at the match clock `clock`: during a settle (twice), a run
    /// and the record after it, every `burst_ms`, one at a time.
    pub fn wants_world(&mut self, clock: u32) -> bool {
        if let Some(sent) = self.outstanding
            && clock >= sent + OUTSTANDING_MS
        {
            self.outstanding = None;
        }
        if self.outstanding.is_some() || self.last_request.is_some_and(|at| clock < at + self.burst_ms) {
            return false;
        }
        let active = self.runners.values().filter_map(|r| r.active.as_ref()).any(|a| match &a.phase {
            Phase::Settle { asked, .. } => *asked,
            Phase::Motor { .. } => true,
            Phase::Run(plan) => clock + 1_000 >= plan.first,
            Phase::Record { until, .. } => clock <= *until + 200,
            _ => false,
        });
        if active {
            self.last_request = Some(clock);
            self.outstanding = Some(clock);
            self.sent += 1;
            Self::line(&mut self.worlds_log, &serde_json::json!({ "req": clock }));
        }
        active
    }

    /// An upload came (`time` its tick, `bytes` its size, at the match clock `clock`): log its rows.
    /// Returns whether it answered a burst request (then it is not passed on).
    pub fn upload(&mut self, time: u32, clock: u32, bytes: usize, body: &[u8]) -> bool {
        let rows = rows_of(body, time);
        let burst = self.outstanding.take().is_some();
        if let Some(sent) = self.last_request {
            self.uploads.push((sent, clock, bytes));
        }
        let wanted: HashSet<Place> = self.runners.keys().copied().collect();
        for row in rows.into_iter().filter(|r| wanted.contains(&r.place)) {
            Self::line(&mut self.worlds_log, &row_json(&row, clock));
            let rows = self.rows.entry(row.place).or_default();
            rows.push_back(row);
            while rows.len() > ROWS_KEPT {
                rows.pop_front();
            }
        }
        burst
    }

    /// The log line of a finished try, from the rows of its place.
    fn summary(&self, place: Place, def: &TryDef, n: u32, plan: &Plan, began: u32, redone: u8) -> serde_json::Value {
        let rows: Vec<&Row> = self
            .rows
            .get(&place)
            .map(|r| r.iter().filter(|r| r.time + 400 >= plan.first && r.alive).collect())
            .unwrap_or_default();
        summarize(def, n, place, plan, began, redone, &rows, &self.uploads)
    }
}

/// The gap between two lanes of the same face, m.
const LANE_GAP_M: f32 = 2.0;

/// A try's site before its lane shift.
fn base_of(def: &TryDef) -> Vec3 {
    let perp = [-def.site.toward[1], def.site.toward[0]];
    Vec3::new(def.site.lower.x - perp[0] * def.lane, def.site.lower.y, def.site.lower.z - perp[1] * def.lane)
}

/// The nearest free lane for a try on its face, given the (face, lane) of the tries under way: 0,
/// then 2.5 m either side, and so on, as far as the ledge is wide minus what the approach drifts
/// sideways; none when every lane of the face within reach is taken.
fn free_lane(def: &TryDef, taken: &[(Vec3, f32)]) -> Option<f32> {
    let drift = (def.runup + def.edge.max(0.0)) * def.angle.to_radians().sin().abs();
    let reach = def.site.half - 1.0 - drift;
    let base = base_of(def);
    let busy = |lane: f32| taken.iter().any(|(b, l)| b.distance_xz(base) < 0.5 && (l - lane).abs() < LANE_GAP_M);
    (0..)
        .map(|k| {
            let step = 2.5 * ((k + 1) / 2) as f32;
            if k % 2 == 1 { step } else { 0.0 - step }
        })
        .take_while(|lane| lane.abs() <= reach.max(0.0))
        .find(|&lane| !busy(lane))
}

/// The bodies of a world as rows.
pub fn rows_of(body: &[u8], time: u32) -> Vec<Row> {
    let Some((blocks, _)) = players::walk(body) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for block in blocks {
        let Some((_, motion)) = Motion::find(block.state, time) else {
            continue;
        };
        let Some(b) = motion.body.as_ref() else {
            out.push(Row {
                time,
                place: usize::from(block.place),
                alive: false,
                feet: Vec3::ZERO,
                jumping: false,
                jump_type: 0,
                speed: 0.0,
                velocity: [0.0; 3],
                normal: [0.0; 3],
                keys: motion.actions_mask,
                yaw: motion.rotation_y,
                pitch: motion.look_pitch,
                sprinting: false,
                crouch: false,
            });
            continue;
        };
        let feet = b.feet();
        out.push(Row {
            time,
            place: usize::from(block.place),
            alive: motion.alive,
            feet: Vec3::new(feet[0], feet[1], feet[2]),
            jumping: b.jumping,
            jump_type: motion.damage.as_ref().map_or(0, |d| d.jump_type),
            speed: motion.damage.as_ref().map_or(0.0, |d| d.linear_horizontal_speed),
            velocity: b.fall_and_slide_velocity,
            normal: b.supporting_surface_normal,
            keys: motion.actions_mask,
            yaw: motion.rotation_y,
            pitch: motion.look_pitch,
            sprinting: b.sprinting,
            crouch: b.capsule_crouch,
        });
    }
    out
}

fn row_json(r: &Row, clock: u32) -> serde_json::Value {
    serde_json::json!({
        "t": r.time, "got": clock, "place": r.place, "alive": r.alive, "feet": r.feet.to_array(),
        "jumping": r.jumping, "jump_type": r.jump_type, "speed": r.speed, "vel": r.velocity,
        "normal": r.normal, "keys": r.keys, "yaw": r.yaw, "pitch": r.pitch,
        "sprinting": r.sprinting, "crouch": r.crouch,
    })
}

/// The position at `time` interpolated between the rows round it.
fn feet_at(rows: &[&Row], time: u32) -> Option<Vec3> {
    let after = rows.iter().position(|r| r.time >= time)?;
    if rows[after].time == time || after == 0 {
        return Some(rows[after].feet);
    }
    let (a, b) = (rows[after - 1], rows[after]);
    let f = (time - a.time) as f32 / (b.time - a.time).max(1) as f32;
    Some(a.feet + (b.feet - a.feet) * f)
}

/// The result of a try from the rows of its world burst. See the log's fields in `grid-record.md` J0.
#[allow(clippy::too_many_arguments)]
pub fn summarize(
    def: &TryDef,
    n: u32,
    place: Place,
    plan: &Plan,
    began: u32,
    redone: u8,
    rows: &[&Row],
    uploads: &[(u32, u32, usize)],
) -> serde_json::Value {
    let t = def.site.toward;
    let a = approach(def);
    let [cx, cz] = def.site.centre();
    let deck = def.site.lower.y;
    let top = deck + def.site.rise;
    let press = plan.press;
    let mut v = serde_json::json!({
        "id": def.id, "n": n, "bot": place, "site": def.site.name, "lane": def.lane,
        "plan": {
            "edge": def.edge, "angle": def.angle, "face": format!("{:?}", def.face).to_lowercase(),
            "runup": def.runup, "speed": format!("{:?}", def.speed).to_lowercase(), "mid": format!("{:?}", def.mid),
            "run_planned": plan.run,
        },
        "rest": plan.rest.to_array(), "heading": plan.heading,
        "began": began, "first_key_ms": plan.first, "press_ms": press, "keys_end_ms": plan.end,
        "redone": redone, "rows": rows.len(),
    });
    let obj = v.as_object_mut().expect("an object");
    if rows.len() < 2 {
        obj.insert("outcome".into(), "no_worlds".into());
        return v;
    }
    // Rows from the press on.
    let from = press.unwrap_or(plan.first);
    let after: Vec<&&Row> = rows.iter().filter(|r| r.time >= from).collect();
    if let Some(press) = press {
        if let Some(p) = feet_at(rows, press) {
            let edge = (cx - p.x) * t[0] + (cz - p.z) * t[1];
            obj.insert("edge_press".into(), serde_json::json!((edge * 100.0).round() / 100.0));
        }
        // The velocity at the press, from the positions 100-200 ms either side.
        if let (Some(p0), Some(p1)) = (feet_at(rows, press.saturating_sub(100)), feet_at(rows, press + 100)) {
            let (dx, dz) = (p1.x - p0.x, p1.z - p0.z);
            let length = dx.hypot(dz);
            if length > 0.05 {
                let vel = length / 0.2;
                let cos = ((dx * t[0] + dz * t[1]) / length).clamp(-1.0, 1.0);
                let sign = (t[0] * dz - t[1] * dx).signum();
                obj.insert("speed_press".into(), serde_json::json!((vel * 100.0).round() / 100.0));
                obj.insert("angle_vel".into(), serde_json::json!((cos.acos().to_degrees() * sign * 10.0).round() / 10.0));
            }
        }
        if let Some(r) = rows.iter().min_by_key(|r| r.time.abs_diff(press)) {
            let f = forward(r.yaw);
            let cos = (f[0] * t[0] + f[1] * t[1]).clamp(-1.0, 1.0);
            let sign = (t[0] * f[1] - t[1] * f[0]).signum();
            obj.insert("angle_face".into(), serde_json::json!((cos.acos().to_degrees() * sign * 10.0).round() / 10.0));
        }
    }
    let types: Vec<u8> = after.iter().map(|r| r.jump_type).filter(|&j| j != 0).collect();
    obj.insert("jump_type".into(), serde_json::json!(types.first().copied().unwrap_or(0)));
    obj.insert("jump_types".into(), serde_json::json!(types));
    let air: Vec<&&&Row> = after.iter().filter(|r| r.jumping).collect();
    let (first_air, last_air) = (air.first().map(|r| r.time), air.last().map(|r| r.time));
    obj.insert("air_ms".into(), serde_json::json!(match (first_air, last_air) { (Some(a), Some(b)) => b - a, _ => 0 }));
    let apex = after.iter().map(|r| r.feet.y).fold(f32::MIN, f32::max);
    obj.insert("apex_over_deck".into(), serde_json::json!(((apex - deck) * 100.0).round() / 100.0));
    let last = after.last().map(|r| **r).unwrap_or_else(|| rows[rows.len() - 1]);
    obj.insert("land".into(), serde_json::json!(last.feet.to_array()));
    // Landed: the feet at the end at the ledge's height, beyond the face.
    let beyond = (last.feet.x - cx) * t[0] + (last.feet.z - cz) * t[1];
    let landed = (last.feet.y - top).abs() <= 0.2 && beyond > 0.1;
    let outcome = if press.is_none() {
        "no_jump"
    } else if landed {
        "landed"
    } else if apex >= top - 0.05 && last.feet.y < top - 0.3 {
        "back_down"
    } else if (last.feet.y - deck).abs() <= 0.3 && beyond > -0.7 && !air.is_empty() {
        "held_at_face"
    } else if apex < top - 0.1 {
        "fell_short"
    } else {
        "other"
    };
    obj.insert("landed".into(), landed.into());
    obj.insert("outcome".into(), outcome.into());
    obj.insert("beyond_face".into(), serde_json::json!((beyond * 100.0).round() / 100.0));
    // The displacement in the air, along and across the heading of the approach.
    if let (Some(first), Some(_)) = (air.first(), air.last()) {
        let (dx, dz) = (last.feet.x - first.feet.x, last.feet.z - first.feet.z);
        let along = dx * a[0] + dz * a[1];
        let across = dx * -a[1] + dz * a[0];
        obj.insert("air_dx".into(), serde_json::json!([(along * 100.0).round() / 100.0, (across * 100.0).round() / 100.0]));
    }
    // Quality: the largest gap between worlds from 300 ms before the press to 2 s after it, and the
    // uploads' times of the burst.
    let window: Vec<u32> = rows.iter().map(|r| r.time).filter(|&t| t + 300 >= from && t <= from + 2_000).collect();
    let gap = window.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0);
    obj.insert("world_gap_ms".into(), gap.into());
    let times: Vec<u32> = uploads.iter().filter(|u| u.1 >= plan.first && u.1 <= from + RECORD_AFTER_MS).map(|u| u.1.saturating_sub(u.0)).collect();
    let bytes: Vec<usize> = uploads.iter().filter(|u| u.1 >= plan.first && u.1 <= from + RECORD_AFTER_MS).map(|u| u.2).collect();
    obj.insert(
        "burst".into(),
        serde_json::json!({
            "worlds": times.len(),
            "upload_ms_mean": if times.is_empty() { 0 } else { times.iter().sum::<u32>() / times.len() as u32 },
            "upload_ms_max": times.iter().max().copied().unwrap_or(0),
            "bytes_mean": if bytes.is_empty() { 0 } else { bytes.iter().sum::<usize>() / bytes.len() },
        }),
    );
    v
}

/// The path of the try file beside which `sites.tsv` and `kinematics.tsv` are read.
pub fn beside(path: &Path, name: &str) -> PathBuf {
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> Site {
        named_site("chem125a", &HashMap::new()).expect("a site")
    }

    fn def(edge: f32, angle: f32, face: Face, runup: f32, speed: Speed, mid: Mid) -> TryDef {
        TryDef { id: "t".into(), who: Who::Bot(3), lane: 0.0, site: site(), edge, angle, face, runup, speed, mid, repeat: 1 }
    }

    #[test]
    fn a_try_file_parses_and_a_bad_line_is_named() {
        let text = "# a comment\nw1 3 chem060a 0.8 0 along 3.5 walk hold 10\ns1 4 -78.9,4.35,47.1,0,-1,1.26,0.25 2.5 -30 along 3.7 sprint release_f 5 # tail\n";
        let tries = parse_tries(text, &HashMap::new()).expect("parses");
        assert_eq!(tries.len(), 2);
        assert_eq!(tries[0].site.name, "chem060a");
        assert_eq!(tries[1].mid, Mid::ReleaseF);
        assert_eq!(tries[1].site.name, "free");
        assert_eq!(parse_mid("turn-30"), Ok(Mid::Turn(-30.0)));
        let bad = parse_tries("a 1 nowhere 1 0 along 3 walk hold 1\n", &HashMap::new());
        assert!(bad.unwrap_err().contains("line 1"));
        let short = parse_tries("a 1 chem060a\n", &HashMap::new());
        assert!(short.unwrap_err().contains("10"));
    }

    fn wide_def(angle: f32, runup: f32) -> TryDef {
        let site = named_site("chem125c", &HashMap::new()).expect("a site");
        TryDef { id: "p".into(), who: Who::Pool, lane: 0.0, site, edge: 1.0, angle, face: Face::Along, runup, speed: Speed::Sprint, mid: Mid::Hold, repeat: 3 }
    }

    #[test]
    fn a_bot_kills_itself_only_when_the_walk_back_beats_the_respawn_by_the_margin() {
        // 8 s of respawn and 5 s from a spawn: 13 s, and 2 s of margin.
        assert_eq!(Suicide::when(30.0, 5.0), Some(Suicide { back_s: 30.0, respawn_s: 13.0 }));
        assert_eq!(Suicide::when(15.1, 5.0), Some(Suicide { back_s: 15.1, respawn_s: 13.0 }));
        assert_eq!(Suicide::when(14.9, 5.0), None);
        assert_eq!(Suicide::when(9.0, 0.5), None);
        let line = Suicide { back_s: 30.0, respawn_s: 13.0 }.json(&def(0.8, 0.0, Face::Along, 3.5, Speed::Walk, Mid::Hold), 3, "suicide");
        assert_eq!(line["saved_s"], 17.0);
        assert_eq!(line["suicide"], true);
    }

    #[test]
    fn a_star_in_the_bot_column_is_a_pool_try() {
        let tries = parse_tries("p1 * chem125c 1.0 0 along 5.5 sprint hold 6
p2 any chem125c 1.0 0 along 5.5 sprint hold 6
p3 7 chem125c 1.0 0 along 5.5 sprint hold 6
", &HashMap::new()).expect("parses");
        assert_eq!(tries[0].who, Who::Pool);
        assert_eq!(tries[1].who, Who::Pool);
        assert_eq!(tries[2].who, Who::Bot(7));
    }

    #[test]
    fn lanes_on_a_wide_face_are_two_and_a_half_metres_apart_and_a_narrow_ledge_has_one() {
        let d = wide_def(0.0, 5.5);
        let base = base_of(&d);
        assert_eq!(free_lane(&d, &[]), Some(0.0));
        assert_eq!(free_lane(&d, &[(base, 0.0)]), Some(2.5));
        assert_eq!(free_lane(&d, &[(base, 0.0), (base, 2.5)]), Some(-2.5));
        // Another face is not in the way.
        let far = Vec3::new(base.x + 40.0, base.y, base.z);
        assert_eq!(free_lane(&d, &[(far, 0.0)]), Some(0.0));
        // A 60 degree run-up drifts 4.8 m sideways: the lanes of a 12 m wall are one.
        let wall = TryDef { site: named_site("chem125a", &HashMap::new()).expect("a site"), ..wide_def(60.0, 5.5) };
        assert_eq!(free_lane(&wall, &[]), Some(0.0));
        assert_eq!(free_lane(&wall, &[(base_of(&wall), 0.0)]), None);
        // A 1.2 m ledge has no lane but the middle.
        let narrow = TryDef { site: named_site("chem060a", &HashMap::new()).expect("a site"), ..wide_def(0.0, 3.5) };
        assert_eq!(free_lane(&narrow, &[(base_of(&narrow), 0.0)]), None);
    }

    #[test]
    fn a_free_bot_takes_the_next_pool_try_on_a_free_lane_and_the_site_moves_along_the_face() {
        let mut jt = JumpTest::new(PathBuf::from("none"), PathBuf::from("none"), "t".into(), 120);
        jt.pool.push_back(wide_def(0.0, 5.5));
        let first = jt.take_from_pool().expect("a try");
        assert_eq!(first.lane, 0.0);
        assert_eq!(jt.pool[0].repeat, 2);
        jt.runners.entry(3).or_default().active = Some(Active { def: first.clone(), n: 1, phase: Phase::Rest { until: 0 }, redone: 0, began: 0, presses: Vec::new(), reached: None });
        let second = jt.take_from_pool().expect("a try");
        assert_eq!(second.lane, 2.5);
        // The shift is along the face: chem125c faces -x, so the lane moves z.
        assert!((second.site.lower.x - first.site.lower.x).abs() < 1e-4);
        assert!((second.site.lower.z - first.site.lower.z).abs() > 2.4);
        assert_eq!(base_of(&second), base_of(&first));
        assert!(jt.holds_end());
    }

    #[test]
    fn the_motor_speeds_and_the_crouch_jumps_parse() {
        let text = "m1 3 chem060a 0.8 0 along 3.5 motor hold 3
m2 3 chem060a 0.8 0 along 3.5 motor_run crouch_jump 3
";
        let tries = parse_tries(text, &HashMap::new()).expect("parses");
        assert_eq!(tries[0].speed, Speed::Motor);
        assert!(tries[0].speed.is_motor() && tries[1].speed.is_motor());
        assert_eq!(tries[1].mid, Mid::CrouchJump);
        assert_eq!(parse_mid("crouch_pre"), Ok(Mid::CrouchPre));
        assert!(!Speed::Sprint.is_motor());
    }

    #[test]
    fn a_crouch_jump_holds_crouch_from_the_press_and_crouch_pre_from_four_ticks_before() {
        let table = RunTable::default();
        for (mid, early) in [(Mid::CrouchJump, false), (Mid::CrouchPre, true)] {
            let plan = plan_from(&def(1.3, 0.0, Face::Along, 3.5, Speed::Walk, mid), site_start(), 0.0, 1_000, &table).expect("fits");
            let press = plan.press.expect("a press");
            assert_eq!(plan.at(press - 5 * TICK_MS).keys & key::CROUCH, 0);
            assert_eq!(plan.at(press - 4 * TICK_MS).keys & key::CROUCH != 0, early);
            let at = plan.at(press);
            assert!(at.keys & key::JUMP != 0 && at.keys & key::CROUCH != 0);
        }
    }

    fn site_start() -> Vec3 {
        start_of(&def(1.3, 0.0, Face::Along, 3.5, Speed::Walk, Mid::Hold))
    }

    #[test]
    fn the_start_is_the_run_up_and_the_edge_before_the_face_on_the_approach_line() {
        // Straight at the face of chem125a: toward (0, -1), the face 0.25 from the deck floor at z 47.1.
        let d = def(0.8, 0.0, Face::Along, 3.5, Speed::Walk, Mid::Hold);
        let s = start_of(&d);
        assert!((s.x + 78.9).abs() < 1e-4);
        // The face is at z 46.85; the press 0.8 before it (z 47.65), the start 3.5 further (z 51.15).
        assert!((s.z - 51.15).abs() < 1e-3, "{s:?}");
        // At 60 degrees the path is longer by 1/cos and the line meets the face's centre.
        let d = def(0.8, 60.0, Face::Along, 3.5, Speed::Walk, Mid::Hold);
        let s = start_of(&d);
        let a = approach(&d);
        let back = 0.8 / 0.5 + 3.5;
        assert!((s.x - (-78.9 - a[0] * back)).abs() < 1e-3);
        assert!((s.z - (46.85 - a[1] * back)).abs() < 1e-3);
    }

    #[test]
    fn the_plan_presses_the_edge_from_the_face_whatever_the_rest_position_off_the_line() {
        let table = RunTable::default();
        let d = def(1.3, 0.0, Face::Along, 3.5, Speed::Walk, Mid::Hold);
        // At rest 0.4 m off the planned start, along and across.
        let rest = Vec3::new(-78.5, 4.35, 51.55);
        let heading = heading_of(&d);
        let plan = plan_from(&d, rest, heading, 10_000, &table).expect("fits");
        let ticks = (plan.press.unwrap() - plan.first) / TICK_MS;
        // The run covers the distance to (the face + 1.3), within a tick's travel (0.2 m).
        let want = (rest.z - 46.85) - 1.3;
        assert!((table.distance(Speed::Walk, ticks) - want).abs() < 0.2, "{ticks} ticks for {want}");
        // Too near the face for a run-up: no plan.
        assert!(plan_from(&d, Vec3::new(-78.9, 4.35, 47.5), heading, 10_000, &table).is_none());
    }

    #[test]
    fn the_keys_run_press_hold_and_do_what_the_mid_air_item_says() {
        let table = RunTable::default();
        let d = def(1.3, 0.0, Face::Along, 3.5, Speed::Sprint, Mid::ReleaseF);
        let plan = plan_from(&d, Vec3::new(-78.9, 4.35, 52.0), heading_of(&d), 10_000, &table).unwrap();
        let press = plan.press.unwrap();
        assert_eq!(plan.at(9_950).keys, 0);
        assert_eq!(plan.at(plan.first).keys, key::FORWARD | key::SPRINT);
        assert_eq!(plan.at(press).keys, key::FORWARD | key::SPRINT | key::JUMP);
        assert_eq!(plan.at(press + 2 * TICK_MS).keys, key::FORWARD | key::SPRINT);
        // Forward let go from the take-off on.
        assert_eq!(plan.at(press + MID_AT_MS).keys, 0);
        assert_eq!(plan.at(plan.end).keys, 0);
        // A diagonal approach by keys, and a turn in the air.
        let d = def(1.0, 45.0, Face::Normal, 3.0, Speed::Walk, Mid::Turn(30.0));
        assert_eq!(base_keys(&d), key::FORWARD | key::LEFT);
        let plan = plan_from(&d, start_of(&d), heading_of(&d), 10_000, &table).unwrap();
        let press = plan.press.unwrap();
        assert!(plan.at(press + MID_AT_MS).turn.0 > 0.0);
        assert_eq!(plan.at(press + MID_AT_MS + 8 * TICK_MS).turn.0, 0.0);
        // A run with no jump (calibration) holds forward then lets go.
        let d = def(-1.0, 0.0, Face::Along, 4.0, Speed::Walk, Mid::Hold);
        let plan = plan_from(&d, Vec3::new(-78.9, 4.35, 52.0), heading_of(&d), 10_000, &table).unwrap();
        assert!(plan.press.is_none());
        assert_eq!(plan.at(plan.first + TICK_MS).keys, key::FORWARD);
        assert_eq!(plan.at(plan.end - TICK_MS).keys, 0);
    }

    #[test]
    fn a_measured_table_replaces_the_blend_model() {
        let table = RunTable::parse("walk 0 0\nwalk 10 1.2\nwalk 20 3.4\nsprint 10 1.9\n");
        assert!((table.distance(Speed::Walk, 15) - 2.3).abs() < 1e-4);
        assert!((table.distance(Speed::Walk, 30) - (3.4 + 3.5 * 10.0 * 0.05)).abs() < 1e-3);
        assert_eq!(table.ticks_for(Speed::Walk, 3.4), 20);
        // Without a table: the ramp, then steady.
        let model = RunTable::default();
        assert!(model.distance(Speed::Walk, 6) < 3.5 * 6.0 * 0.05);
        assert!((model.distance(Speed::Walk, 40) - model.distance(Speed::Walk, 39) - 3.5 * 0.05).abs() < 1e-3);
    }

    fn row(time: u32, y: f32, z: f32, jumping: bool, jump_type: u8) -> Row {
        Row {
            time,
            place: 3,
            alive: true,
            feet: Vec3::new(-78.9, y, z),
            jumping,
            jump_type,
            speed: 3.5,
            velocity: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            keys: key::FORWARD,
            yaw: yaw_of([0.0, -1.0]),
            pitch: 0.0,
            sprinting: false,
            crouch: false,
        }
    }

    #[test]
    fn a_try_that_got_up_is_landed_and_one_that_came_back_is_not() {
        let table = RunTable::default();
        let d = def(0.8, 0.0, Face::Along, 3.5, Speed::Walk, Mid::Hold);
        let plan = plan_from(&d, Vec3::new(-78.9, 4.35, 51.15), heading_of(&d), 10_000, &table).unwrap();
        let press = plan.press.unwrap();
        // Up: the feet rise to the top (4.35 + 1.259) and stay beyond the face (z < 46.85).
        let up: Vec<Row> = vec![
            row(press - 100, 4.35, 48.5, false, 0),
            row(press + 100, 4.35, 47.7, true, 5),
            row(press + 500, 5.2, 46.9, true, 5),
            row(press + 900, 5.6, 46.3, true, 5),
            row(press + 2_000, 5.609, 45.9, false, 0),
        ];
        let refs: Vec<&Row> = up.iter().collect();
        let v = summarize(&d, 1, 3, &plan, 0, 0, &refs, &[(9_000, 9_040, 40_000)]);
        assert_eq!(v["outcome"], "landed", "{v}");
        assert_eq!(v["jump_type"], 5);
        assert!(v["landed"].as_bool().unwrap());
        assert_eq!(v["burst"]["worlds"], 0);
        // Back down: up to the apex and onto the deck again, short of the face.
        let down: Vec<Row> = vec![
            row(press - 100, 4.35, 48.5, false, 0),
            row(press + 100, 4.35, 47.7, true, 5),
            row(press + 500, 5.62, 46.9, true, 5),
            row(press + 2_000, 4.35, 47.3, false, 0),
        ];
        let refs: Vec<&Row> = down.iter().collect();
        let v = summarize(&d, 1, 3, &plan, 0, 0, &refs, &[]);
        assert_eq!(v["outcome"], "back_down", "{v}");
        // Short: never rose.
        let short: Vec<Row> = vec![row(press - 100, 4.35, 48.5, false, 0), row(press + 300, 4.9, 47.6, true, 5), row(press + 2_000, 4.35, 47.4, false, 0)];
        let refs: Vec<&Row> = short.iter().collect();
        let v = summarize(&d, 1, 3, &plan, 0, 0, &refs, &[]);
        assert_ne!(v["outcome"], "landed");
        assert!(v["edge_press"].as_f64().is_some());
    }

    #[test]
    fn a_world_burst_is_asked_one_at_a_time_during_a_try_only() {
        let mut jt = JumpTest::new(PathBuf::from("none"), std::env::temp_dir(), "t".into(), 120);
        assert!(!jt.wants_world(1_000), "no try, no request");
        let d = def(0.8, 0.0, Face::Along, 3.5, Speed::Walk, Mid::Hold);
        let plan = plan_from(&d, Vec3::new(-78.9, 4.35, 51.15), heading_of(&d), 10_000, &RunTable::default()).unwrap();
        jt.runners.entry(3).or_default().active = Some(Active { def: d, n: 1, phase: Phase::Run(plan), redone: 0, began: 0, presses: Vec::new(), reached: None });
        assert!(jt.wants_world(9_500));
        // One at a time: not again until answered or forgotten.
        assert!(!jt.wants_world(9_700));
        assert!(!jt.wants_world(10_000));
        assert!(jt.wants_world(10_200), "forgotten after the timeout");
        assert!(jt.holds_end());
    }
}
