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

use mapdata::{NodeId, Vec3, World};
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
/// The settle: still, facing the heading, at least this long, ms (a sprint try rests
/// [`REST_BEFORE_SPRINT_MS`]: the walk to the start was at a run).
const SETTLE_MS: u32 = 1_200;
/// A rest that misfits moves the aim of the walk by how far the reckoning stood off the body, if that is within
/// this, m; further off, the reckoning was lost and the aim stays the start.
const SHIFT_MAX_M: f32 = 1.0;
/// The settle is given up after this long, ms (the try is redone from Go).
const SETTLE_MAX_MS: u32 = 5_000;
/// The walk to the start is given up after this long, ms.
const GO_MAX_MS: u32 = 60_000;
/// A body this close to its start (m), at rest, has arrived: the real client's body coasts about 0.3-0.6 m after the
/// keys are released (blend), so it comes to rest that far past the point the walk ended at, and the stand-in's does not.
const ARRIVED_M: f32 = 0.6;
/// The radius the walk to a start asks the motor for ([`Intent::exact_arrive_m`]), m: the motor's own is 0.6 m (the J0 on
/// Chemical Plant struck five ledges at 0.30-0.35 m short of the start), 0.1 m overshot on the live client (rests 0.45-0.53 m
/// past the start, five of nine tries void in session 3): the keys are let go where the coast lands the body on the start.
const START_ARRIVE_M: f32 = 0.45;
/// The walk to a start goes at a walk for its last stretch (m): at a run the live body coasts 1.1-1.3 m past the point the keys are
/// let go (the sprint rows of session 3's calibration rested that far off), at a walk 0.4-0.6 m.
const GO_WALK_WITHIN_M: f32 = 6.0;
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
/// An upload that took longer than this (ms) makes the next request wait as long again.
const SLOW_UPLOAD_MS: u32 = 250;
/// A motor-driven try is given up after this long on its way to the top, ms.
const MOTOR_MAX_MS: u32 = 25_000;
/// A ledge within this of one whose start was not reached is given up with it, m.
const BAD_SITE_M: f32 = 1.0;
/// How far past the face, along its normal, a motor-driven try's top point is, m.
const TOP_BEYOND_M: f32 = 1.2;
/// The respawn delay of every mode's config (`respawn_time`), ms (`docs/notes/modes_maps.md`).
const RESPAWN_MS: f32 = 8_000.0;
/// The sprint speed of the budget's straight walk from a spawn, m/s (the `Kinematics` default), and the
/// slack on it (the route is longer than the line, and the walk is slower than a sprint).
const SPRINT_MPS: f32 = 5.0;
const BUDGET_SLACK: f32 = 1.3;
/// The suicide bit is sent this many ticks.
const SUICIDE_TICKS: u32 = 3;
/// A team kills itself at most this often in a match: a death costs the team a ticket (100, 150 on two
/// maps), and a team with none left no longer respawns.
const SUICIDE_CAP: u32 = 25;
/// A bot still alive this long after the suicide bit goes on walking, ms.
const SUICIDE_WAIT_MS: u32 = 1_500;
/// A rest position fits its start within this along the approach and across it (xz), m, this in height (y), and this facing (rad, 10 degrees); two worlds
/// of the rest agree within [`STILL_M`].
const REST_FIT_ALONG_M: f32 = 0.6;
const REST_FIT_ACROSS_M: f32 = 0.5;
const REST_FIT_Y_M: f32 = 0.3;
const REST_FIT_YAW: f32 = 0.1745;
const STILL_M: f32 = 0.05;
/// A bot with no try stays at least this far from the start, the run-up and the top of every try under
/// way, m (and moves off to a spot 0.5 m further).
const CLEAR_M: f32 = 5.0;
/// A try takes the ground this far past the face as well: the landing and the steps on after it, m.
const BEYOND_M: f32 = 4.0;
/// The keys a bot of the test may press: walking, sprinting, crouching and jumping.
const MOVING_KEYS: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::JUMP | key::CROUCH | key::SPRINT | key::SUICIDE;

/// One row of a world: what the owner's client says of one body at one tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The upload's own tick (the match clock).
    pub time: u32,
    pub place: Place,
    pub alive: bool,
    /// When the body was spawned (another value is another life).
    pub inserted: u32,
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
    /// How deep the flat top is past the face, m, if known (`sites.tsv`'s tenth column, from the walker mesh): a try
    /// whose jump travels past it ([`Site::kind`]) goes over the ledge instead of onto it.
    pub depth: Option<f32>,
    /// A gap site (`sites.tsv`'s trailing word `gap`): its top (the landing across a gap) is a floor the walk grid does not
    /// reach from the spawns until a link exists, so only the start is checked ([`ReachSets::refuse`]).
    pub gap: bool,
}

/// What a try on a site is: a jump onto a top, or over a thin obstacle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The top is deep enough for the jump to land on it.
    Climb,
    /// The jump travels past the top: it clears the obstacle and lands behind it at the deck's height.
    Over,
    /// A jump across a gap (the site is flagged `gap`).
    Gap,
    /// The site's depth is not known.
    Unknown,
}

/// How far a jump travels from the press, m: the walking jump 2.67 m, the sprint jump 4.4 m (the clips' roots,
/// `JUMP-FINDINGS.md`).
const WALK_TRAVEL_M: f32 = 2.67;
const SPRINT_TRAVEL_M: f32 = 4.4;

impl Site {
    /// Climb or over for a try that presses `edge` m before the face at `speed`: climb when the flat top is deep
    /// enough for the jump to land on it (`depth >= edge + travel`).
    pub fn kind(&self, edge: f32, speed: Speed) -> Kind {
        let travel = if speed.sprints() { SPRINT_TRAVEL_M } else { WALK_TRAVEL_M };
        if self.gap {
            return Kind::Gap;
        }
        match self.depth {
            Some(depth) if depth >= edge.max(0.0) + travel => Kind::Climb,
            Some(_) => Kind::Over,
            None => Kind::Unknown,
        }
    }

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
    // chem125a/b/c and vect090a: UNREACHABLE (no path from any spawn, found 2026-10-26; `gridtool ledges` now
    // prints `reach`): kept for the tests' geometry only.
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
    /// A standing jump: the press is the first key tick, with no move key (`edge` is where the body stands).
    Stand,
    /// A sprint jump from a full stop: forward and sprint from the first key tick, the press this many ms after it
    /// (`sprint_at_<ms>`): the sprint's prepare runs to the next footfall, so the take-off depends on the gait phase.
    SprintAt(u32),
    /// The ordinary drive at a walk (the motor presses the jumps).
    Motor,
    /// The ordinary drive at a run.
    MotorRun,
}

impl Speed {
    /// Whether the body sprints.
    fn sprints(self) -> bool {
        matches!(self, Self::Sprint | Self::SprintAt(_))
    }

    /// Whether the keys are the motor's, not the plan's.
    fn is_motor(self) -> bool {
        matches!(self, Self::Motor | Self::MotorRun)
    }
}

/// The keys held from a landing on ([`Mid::Land`]): what, if anything, cancels the carry of the landing clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Landing {
    /// Nothing held.
    Nothing,
    Back,
    Crouch,
    Left,
    Right,
    /// Forward alone.
    Forward,
    /// Forward and sprint, as the run-up.
    Sprint,
}

impl Landing {
    fn keys(self) -> u32 {
        match self {
            Self::Nothing => 0,
            Self::Back => key::BACK,
            Self::Crouch => key::CROUCH,
            Self::Left => key::LEFT,
            Self::Right => key::RIGHT,
            Self::Forward => key::FORWARD,
            Self::Sprint => key::FORWARD | key::SPRINT,
        }
    }
}

/// How long the keys of a [`Mid::Land`] try are played after they change, ms.
const LAND_KEYS_MS: u32 = 1_500;

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
    /// Every key released 0.25 s after the press (the air gets no control), then `keys` from `after_ms` after the press
    /// (about the landing: the clip ends 0.9-1.2 s after it): what carries the body after a sprint jump lands, and what
    /// stops it (`land_back_1100`).
    Land { keys: Landing, after_ms: u32 },
    /// Forward from `after_ms` after the press (a standing jump that steps on once it is in the air; 0: from the press).
    Step { after_ms: u32 },
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
    /// How many times the bot killed itself on the way to this try's start (one at most).
    pub suicides: u8,
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
            depth: None,
            gap: false,
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
        depth: None,
        gap: false,
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
        other if other.starts_with("step_") => {
            let after_ms = other["step_".len()..].parse::<u32>().map_err(|e| format!("mid {other:?}: {e}"))?;
            if after_ms > 1_000 {
                return Err(format!("mid {other:?}: forward 0-1000 ms after the press"));
            }
            Mid::Step { after_ms }
        }
        other if other.starts_with("land_") => {
            let (keys, ms) = other["land_".len()..].split_once('_').ok_or_else(|| format!("mid {other:?}"))?;
            let keys = match keys {
                "nothing" | "none" => Landing::Nothing,
                "back" => Landing::Back,
                "crouch" => Landing::Crouch,
                "left" => Landing::Left,
                "right" => Landing::Right,
                "forward" => Landing::Forward,
                "sprint" => Landing::Sprint,
                _ => return Err(format!("mid {other:?}: keys")),
            };
            let after_ms = ms.parse::<u32>().map_err(|e| format!("mid {other:?}: {e}"))?;
            if !(300..=1_500).contains(&after_ms) {
                return Err(format!("mid {other:?}: the keys change 300-1500 ms after the press"));
            }
            Mid::Land { keys, after_ms }
        }
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
                "stand" => Speed::Stand,
                other if other.starts_with("sprint_at_") => {
                    let ms = other["sprint_at_".len()..].parse::<u32>().map_err(|e| fail(format!("speed {other:?}: {e}")))?;
                    if !(100..=3_000).contains(&ms) {
                        return Err(fail(format!("speed {other:?}: the press 100-3000 ms after the sprint")));
                    }
                    Speed::SprintAt(ms)
                }
                "motor" => Speed::Motor,
                "motor_run" => Speed::MotorRun,
                other => return Err(fail(format!("speed {other:?}"))),
            },
            mid: parse_mid(f[8]).map_err(fail)?,
            repeat: f[9].parse().map_err(|e| fail(format!("repeat {:?}: {e}", f[9])))?,
            suicides: 0,
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
        let gap = f.last() == Some(&"gap");
        if !(8..=10 + usize::from(gap)).contains(&f.len()) {
            continue;
        }
        let v: Vec<f32> = f[1..].iter().filter_map(|s| s.parse().ok()).collect();
        if !(7..=9).contains(&v.len()) {
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
                depth: v.get(8).copied(),
                gap,
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
        let kind = usize::from(speed.sprints());
        let rows = &self.measured[kind];
        if let Some(i) = rows.iter().position(|r| r.0 >= ticks) {
            if rows[i].0 == ticks || i == 0 {
                return rows[i].1;
            }
            let (a, b) = (rows[i - 1], rows[i]);
            return a.1 + (b.1 - a.1) * (ticks - a.0) as f32 / (b.0 - a.0) as f32;
        }
        if let Some(&(t, d)) = rows.last() {
            let v = if speed.sprints() { 5.0 } else { 3.5 };
            return d + v * (ticks - t) as f32 * TICK_S;
        }
        let kin = super::kin::Kinematics::default();
        let v = if speed.sprints() { kin.sprint } else { kin.walk[0] };
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
    if def.speed == Speed::Stand {
        return 0;
    }
    let mut keys = key::FORWARD;
    if def.speed.sprints() {
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
    } else if matches!(def.speed, Speed::Stand | Speed::SprintAt(_)) {
        // The rest is the start (it fits within the settle's tolerance): the press is a fixed time from the first tick.
        (0.0, Some(()))
    } else {
        let run = to_face - def.edge / cos;
        if run < 0.4 {
            return None;
        }
        (run, Some(()))
    };
    let ticks = match def.speed {
        Speed::Stand => 0,
        Speed::SprintAt(ms) => (ms + TICK_MS / 2) / TICK_MS,
        _ => table.ticks_for(def.speed, run),
    };
    let press_at = first + ticks * TICK_MS;
    let end = match press {
        Some(()) => press_at + if let Mid::Land { after_ms, .. } = def.mid { after_ms + LAND_KEYS_MS } else { KEYS_AFTER_MS },
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
        if let Mid::Step { after_ms } = self.mid
            && time >= press + after_ms
        {
            keys |= key::FORWARD;
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
                Mid::CrouchJump | Mid::CrouchPre | Mid::Step { .. } => {}
                Mid::Land { keys: land, after_ms } => {
                    keys = if time >= press + after_ms { land.keys() } else { 0 };
                }
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

/// How long a walk to a start may take before the bot does better to die and respawn: the respawn
/// delay plus a sprint over the straight line from the nearest spawn of its team to the start, with
/// slack, ms.
fn budget_ms(spawn_to_start_m: f32) -> u32 {
    (RESPAWN_MS + spawn_to_start_m / SPRINT_MPS * BUDGET_SLACK * 1_000.0) as u32
}

/// How long the walk still to go takes at the budget's speed and slack, ms, from `distance_m` (straight): a
/// bot kills itself only when this alone is longer than the budget (dying and walking from a spawn).
fn remaining_ms(distance_m: f32) -> u32 {
    (distance_m / SPRINT_MPS * BUDGET_SLACK * 1_000.0) as u32
}

/// [`budget_ms`] for a team's bot and the start of `def`.
fn suicide_budget_ms(world: &World, team: u8, def: &TryDef) -> u32 {
    let start = start_of(def);
    let nearest = |own: bool| {
        world
            .map
            .spawns
            .iter()
            .filter(|s| !own || s.team == i32::from(team))
            .map(|s| s.position.distance_xz(start))
            .fold(f32::MAX, f32::min)
    };
    let own = nearest(true);
    budget_ms(if own < f32::MAX { own } else { nearest(false) })
}

#[derive(Clone, Debug)]
enum Phase {
    /// The bit of the suicide is sent; the next world should show the body dead. The walk had taken
    /// `walked_ms` of its `budget_ms`.
    Suicide { since: u32, walked_ms: u32, budget_ms: u32 },
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
    /// The team of the bot working it (enemies collide, teammates pass through each other: a ledge takes
    /// one team's tries at a time).
    team: u8,
    /// How far the reckoning stood off the body (x, z, m) at the last rest that did not fit: the walk to the start
    /// aims the reckoning there, so the body, not the reckoning, comes to the start. Zero until a rest misfit, and
    /// when the reckoning was lost (more than [`SHIFT_MAX_M`] off: the next ordinary world puts it right).
    shift: [f32; 2],
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
    /// Bots whose own planner found no way to a site from where they stand (they spawn in an area the other
    /// side's spawn areas shut off), by place and site: they take no try there.
    no_route: HashSet<(Place, (i32, i32))>,
    /// When a bot first waited short of an enemy's try on its way to a start (it waits [`YIELD_MAX_MS`] at most).
    yielding: HashMap<Place, u32>,
    /// Suicides so far, by team.
    suicides: [u32; 2],
    /// The reach of both teams' spawns, made at the first try file read.
    reach: Option<ReachSets>,
    /// Sites already checked: (the deck's x and z, in 0.1 m) and why refused.
    checked: HashMap<(i32, i32), Option<String>>,
    /// The team that works each ledge of the file (by its site): the ledges nearer a team's spawns are its own, half
    /// each, so the teams do not cross on the way (enemies collide). Empty with one ledge or a team without spawns.
    sides: HashMap<(i32, i32), u8>,
    /// Ledges whose start no bot reached twice: no try is taken on them (their base, before any lane).
    bad_sites: Vec<Vec3>,
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
    /// Every alive body of every world (place, x, y, z): who stood near a runner.
    bodies_log: Option<fs::File>,
    /// After a slow upload the next request waits this long at least, ms.
    backoff_ms: u32,
    worlds_log: Option<fs::File>,
    /// The last world request and whether it is answered; upload times of the burst.
    last_request: Option<u32>,
    outstanding: Option<u32>,
    uploads: Vec<(u32, u32, usize)>,
    sent: u32,
    /// The rows of the last asked world, until [`Self::take_fresh`] passes them to the bots' reckoning: an asked world
    /// goes no further in the game, so without this the reckoning of every bot is starved while the test asks (the
    /// J0 on Chemical Plant: rests 0.5-18 m off).
    fresh: Vec<Row>,
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
            bad_sites: Vec::new(),
            no_route: HashSet::new(),
            sides: HashMap::new(),
            yielding: HashMap::new(),
            suicides: [0; 2],
            reach: None,
            checked: HashMap::new(),
            idle_others: true,
            max_active: usize::MAX,
            done: HashMap::new(),
            started: HashMap::new(),
            known: HashMap::new(),
            table: RunTable::default(),
            rows: HashMap::new(),
            tries_log: None,
            bodies_log: None,
            backoff_ms: 0,
            worlds_log: None,
            last_request: None,
            outstanding: None,
            uploads: Vec::new(),
            sent: 0,
            fresh: Vec::new(),
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
    fn reload(&mut self, world: &World) {
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
                // A site whose start or top is not reached both ways from both teams' spawns is refused.
                let mut tries_ok = Vec::new();
                for def in tries {
                    let key = ((def.site.lower.x * 10.0).round() as i32, (def.site.lower.z * 10.0).round() as i32);
                    if !self.checked.contains_key(&key) {
                        let reach = self.reach.get_or_insert_with(|| ReachSets::build(world));
                        let why = reach.refuse(world, &def);
                        if let Some(why) = &why {
                            log::warn!("jump test: site {} refused: {why}", def.site.name);
                            let line = serde_json::json!({ "refused": def.site.name, "id": def.id, "why": why, "deck": def.site.lower.to_array() });
                            if self.tries_log.is_none() {
                                self.tries_log = self.open("tries");
                            }
                            Self::line(&mut self.tries_log, &line);
                        }
                        self.checked.insert(key, why);
                    }
                    if self.checked.get(&key).is_some_and(Option::is_none) {
                        tries_ok.push(def);
                    }
                }
                let tries = tries_ok;
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
                pool.retain(|d| self.bad_sites.iter().all(|bad| base_of(d).distance_xz(*bad) > BAD_SITE_M));
                self.pool = pool;
                let spawns: Vec<(Vec3, u8)> = world.map.spawns.iter().map(|s| (s.position, u8::from(s.team != 0))).collect();
                let sites: Vec<((i32, i32), Vec3)> = self.known.values().map(|d| (site_key(d), start_of(d))).collect();
                self.sides = split_sites(&spawns, sites);
                for (place, runner) in &mut self.runners {
                    runner.pending = by_bot.remove(place).unwrap_or_default();
                }
                for (place, pending) in by_bot {
                    self.runners.entry(place).or_default().pending = pending;
                }
                if self.tries_log.is_none() {
                    self.tries_log = self.open("tries");
                    self.worlds_log = self.open("worlds");
                    self.bodies_log = self.open("bodies");
                }
            }
        }
    }

    /// Whether a world was asked for within the last `ms` of the match clock `clock`: the game does not pass an
    /// asked world on to the bots' reckoning, so for these ms it gets none (the dry run models it).
    #[cfg(test)]
    pub(crate) fn asked_within(&self, clock: u32, ms: u32) -> bool {
        self.last_request.is_some_and(|at| clock < at + ms)
    }

    /// Whether the match must not end: some try is under way or still to come.
    pub fn holds_end(&self) -> bool {
        !self.pool.is_empty()
            || self
                .runners
                .values()
                .any(|r| r.active.is_some() || !r.pending.is_empty())
    }

    /// What the tries under way of the other bots take up.
    fn zones_except(&self, place: Place) -> Vec<Zone> {
        self.runners
            .iter()
            .filter(|(p, _)| **p != place)
            .filter_map(|(_, r)| r.active.as_ref())
            .map(|a| Zone::of(&a.def, a.team, matches!(a.phase, Phase::Settle { .. } | Phase::Run(_) | Phase::Record { .. } | Phase::Motor { .. })))
            .collect()
    }

    /// The bots with a try in a phase where another body near them spoils it, and their team.
    pub fn trying(&self) -> Vec<(Place, u8)> {
        self.runners
            .iter()
            .filter_map(|(&place, r)| {
                let a = r.active.as_ref()?;
                matches!(a.phase, Phase::Settle { .. } | Phase::Run(_) | Phase::Record { .. } | Phase::Motor { .. }).then_some((place, a.team))
            })
            .collect()
    }

    /// What a bot of the test is doing, for the dry run's reports.
    pub fn phase_name(&self, place: Place) -> &'static str {
        match self.runners.get(&place).and_then(|r| r.active.as_ref()).map(|a| &a.phase) {
            None if self.runners.contains_key(&place) => "free",
            None => "idle",
            Some(Phase::Rest { .. }) => "rest",
            Some(Phase::Go { .. }) => "go",
            Some(Phase::Settle { .. }) => "settle",
            Some(Phase::Run(_)) => "run",
            Some(Phase::Record { .. }) => "record",
            Some(Phase::Motor { .. }) => "motor",
            Some(Phase::Suicide { .. }) => "suicide",
        }
    }

    /// Tries under way.
    fn active_count(&self) -> usize {
        self.runners.values().filter(|r| r.active.is_some()).count()
    }

    /// The next pool try a free bot can take: the first with a free lane on its face (the lane is set
    /// and the site shifted along the face); a try whose faces are all taken waits.
    fn take_from_pool(&mut self, place: Place, team: u8) -> Option<TryDef> {
        let taken: Vec<Taken> = self
            .runners
            .values()
            .filter_map(|r| r.active.as_ref())
            .map(|a| (base_of(&a.def), a.def.lane, a.team, Zone::of(&a.def, a.team, true)))
            .collect();
        let (index, lane) = self
            .pool
            .iter()
            .enumerate()
            .filter(|(_, def)| !self.no_route.contains(&(place, site_key(def))))
            // A ledge on the other team's side is not this team's to work.
            .filter(|(_, def)| self.sides.get(&site_key(def)).is_none_or(|&side| side == team & 1))
            .find_map(|(i, def)| free_lane(def, &taken, team).map(|lane| (i, lane)))?;
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

    /// Only the keys of moving for a bot of the test (and for the others while they rest): whatever the
    /// ordinary drive decided while the bot walked to a start (its reflex aims and fires at anyone in
    /// sight), nothing is fired, aimed, thrown, used, dropped or switched.
    pub fn quiet(&self, place: Place, decision: &mut Decision) {
        if self.runners.contains_key(&place) || self.idle_others {
            decision.keys &= MOVING_KEYS;
        }
    }

    /// Whether this bot's place is in the test.
    pub fn owns(&self, place: Place) -> bool {
        self.runners.contains_key(&place)
    }

    /// The think of a bot in the mode: the intent (a walk to the start while going, else standing).
    /// Returns whether the bot is the test's.
    pub fn think(&mut self, bot: &mut Bot, view: &MatchView, world: &World) -> bool {
        self.reload(world);
        let now = view.clock;
        if !self.runners.contains_key(&bot.place) {
            if self.pool.is_empty() || self.active_count() >= self.max_active {
                if self.idle_others {
                    bot.intent = Intent::stay();
                    bot.intent.fire = false;
                    if let Some(me) = view.place(bot.place).filter(|p| p.alive) {
                        keep_clear(&self.zones_except(bot.place), world, bot, me.feet);
                    }
                }
                return self.idle_others;
            }
            self.runners.insert(bot.place, Runner::default());
        }
        let free = self.runners.get(&bot.place).is_some_and(|r| r.active.is_none() && r.pending.is_empty());
        let alive = view.place(bot.place).is_some_and(|p| p.alive);
        if free
            && alive
            && self.active_count() < self.max_active
            && let Some(def) = self.take_from_pool(bot.place, bot.team)
            && let Some(runner) = self.runners.get_mut(&bot.place)
        {
            runner.pending.push_back(def);
        }
        let zones = self.zones_except(bot.place);
        let Some(runner) = self.runners.get_mut(&bot.place) else {
            return false;
        };
        let Some(me) = view.place(bot.place).filter(|p| p.alive) else {
            // Dead: the try is lost with the life; it is redone from the start.
            if let Some(active) = runner.active.take() {
                if let Phase::Suicide { walked_ms, budget_ms, .. } = active.phase {
                    let line = serde_json::json!({
                        "id": active.def.id, "bot": bot.place, "team": active.team, "suicide": true, "elapsed_ms": walked_ms, "budget_ms": budget_ms,
                    });
                    Self::line(&mut self.tries_log, &line);
                }
                let def = TryDef { repeat: 1, ..active.def };
                if def.who == Who::Pool {
                    back_to_pool(&mut self.pool, def);
                } else {
                    runner.pending.push_front(def);
                }
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
            self.yielding.remove(&bot.place);
            let rest = if def.speed == Speed::MotorRun { Phase::Rest { until: now + REST_BEFORE_SPRINT_MS } } else { Phase::Go { since: now } };
            runner.active = Some(Active { def, n, phase: rest, redone: 0, began: now, presses: Vec::new(), reached: None, team: bot.team, shift: [0.0, 0.0] });
        }
        let Some(active) = runner.active.as_mut() else {
            keep_clear(&zones, world, bot, me.feet);
            return true;
        };
        let start = start_of(&active.def);
        match &mut active.phase {
            Phase::Suicide { since, walked_ms, budget_ms } => {
                if now > *since + SUICIDE_WAIT_MS {
                    // Still alive (a helmet took the hit): no second try, the bot walks.
                    let line = serde_json::json!({
                        "id": active.def.id, "bot": bot.place, "team": active.team, "suicide_failed": true, "elapsed_ms": *walked_ms, "budget_ms": *budget_ms,
                    });
                    Self::line(&mut self.tries_log, &line);
                    active.phase = Phase::Go { since: now };
                }
            }
            Phase::Rest { until } => {
                if now >= *until {
                    active.phase = Phase::Go { since: now };
                }
            }
            Phase::Go { since } => {
                if !yield_to(&zones, bot.team, me.feet) {
                    // Clear of enemy tries: the next encounter waits its own while.
                    self.yielding.remove(&bot.place);
                }
                let aim = Vec3::new(start.x + active.shift[0], start.y, start.z + active.shift[1]);
                if me.feet.distance_xz(aim) <= ARRIVED_M && me.velocity[0].hypot(me.velocity[1]) < 0.6 {
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
                } else if me.feet.distance_xz(start) > 3.0 && bot.no_way.is_some_and(|(goal, at)| goal.distance_xz(start) < 0.5 && now.saturating_sub(at) < 5_000) {
                    // The bot's own planner finds no way from here (it spawned in a part the other side's spawn
                    // areas cut off): the try goes to another bot, and this one takes none on that site.
                    let line = serde_json::json!({
                        "id": active.def.id, "n": active.n, "bot": bot.place, "team": active.team, "released": "no way from here",
                        "start": start.to_array(), "from": me.feet.to_array(),
                    });
                    Self::line(&mut self.tries_log, &line);
                    self.no_route.insert((bot.place, site_key(&active.def)));
                    let def = TryDef { repeat: 1, ..active.def.clone() };
                    if def.who == Who::Pool {
                        back_to_pool(&mut self.pool, def);
                    }
                    runner.active = None;
                } else if yield_to(&zones, bot.team, me.feet) && now.saturating_sub(*self.yielding.entry(bot.place).or_insert(now)) < YIELD_MAX_MS {
                    // An enemy's try is on the way: wait short of it (the walk's clock does not run meanwhile).
                    bot.intent = Intent::stay();
                    bot.intent.fire = false;
                    *since = (*since).max(now.saturating_sub(GO_MAX_MS - 5_000));
                    // Already on the ground of that try (it was not live yet when the bot came): walk off it.
                    let enemy: Vec<Zone> = zones.iter().filter(|z| z.live && z.team != bot.team).copied().collect();
                    if enemy.iter().any(|z| z.nearest(me.feet).1 < INSIDE_M) {
                        keep_clear(&enemy, world, bot, me.feet);
                    }
                } else if active.def.suicides == 0
                    && self.suicides[usize::from(bot.team & 1)] < SUICIDE_CAP
                    && now - *since > 8_000
                    && now - *since > suicide_budget_ms(world, bot.team, &active.def)
                    && remaining_ms(me.feet.distance_xz(start)) > suicide_budget_ms(world, bot.team, &active.def)
                {
                    // The walk takes longer than dying and walking from a spawn: once.
                    let (walked_ms, budget_ms) = (now - *since, suicide_budget_ms(world, bot.team, &active.def));
                    active.def.suicides += 1;
                    self.suicides[usize::from(bot.team & 1)] += 1;
                    active.phase = Phase::Suicide { since: now, walked_ms, budget_ms };
                } else if now > *since + GO_MAX_MS {
                    log::warn!("jump test: bot {} could not walk to the start of {}", bot.place, active.def.id);
                    active.redone += 1;
                    if active.redone >= 2 {
                        // Not reached twice: the try is void, and so is every try on that ledge.
                        let line = serde_json::json!({
                            "id": active.def.id, "n": active.n, "bot": bot.place, "team": active.team, "void": "start not reached",
                            "start": start.to_array(), "from": me.feet.to_array(),
                        });
                        Self::line(&mut self.tries_log, &line);
                        *self.done.entry(active.def.id.clone()).or_default() += 1;
                        let bad = base_of(&active.def);
                        forget_site(&mut self.pool, &mut self.bad_sites, bad);
                        runner.pending.retain(|d| base_of(d).distance_xz(bad) > BAD_SITE_M);
                        runner.active = None;
                    } else {
                        *since = now;
                    }
                } else {
                    bot.intent = Intent {
                        go: Go::To { at: aim, within_m: 0.0 },
                        pace: if active.def.speed == Speed::MotorRun || me.feet.distance_xz(aim) < GO_WALK_WITHIN_M { Pace::Walk } else { Pace::Run },
                        fire: false,
                        exact_arrive_m: Some(START_ARRIVE_M),
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
        // A bot with no try that walks off a try's ground (`keep_clear`) goes by the ordinary drive.
        let walking_off = matches!(bot.intent.go, Go::To { .. });
        let Some(runner) = self.runners.get_mut(&bot.place) else {
            return (self.idle_others && !walking_off).then(Decision::default);
        };
        let Some(active) = runner.active.as_mut() else {
            return (!walking_off).then(Decision::default);
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
                if elapsed >= 600 {
                    *asked = true;
                }
                // The rest position: the last two worlds of this place at rest, after the settle began, that
                // agree; they must be where the try starts and facing the heading.
                let start = start_of(&active.def);
                let rests: Vec<Row> = self
                    .rows
                    .get(&bot.place)
                    .map(|rows| rows.iter().rev().filter(|r| r.time >= *since + 600 && r.alive && r.speed < 0.15 && !r.jumping).take(2).cloned().collect())
                    .unwrap_or_default();
                if let [row, before] = &rests[..]
                    && row.feet.distance_xz(before.feet) <= STILL_M
                    && elapsed >= if active.def.speed.sprints() { REST_BEFORE_SPRINT_MS } else { SETTLE_MS }
                {
                    let first = (time / TICK_MS + LEAD_TICKS) * TICK_MS;
                    let plan = rest_fits(row.feet, row.yaw, start, heading, approach(&active.def), across_m(&active.def.site))
                        .then(|| plan_from(&active.def, row.feet, row.yaw, first, &self.table))
                        .flatten();
                    match plan {
                        Some(plan) => {
                            active.phase = Phase::Run(plan);
                        }
                        None => {
                            if active.redone < 3 {
                                active.redone += 1;
                                active.shift = redo_shift(active.shift, start, row.feet);
                                log::info!("jump test: {} from {:?}: the rest position does not fit; redone, aiming {:?} off", active.def.id, row.feet, active.shift);
                                active.phase = Phase::Go { since: now };
                            } else {
                                let line = serde_json::json!({
                                    "id": active.def.id, "n": active.n, "bot": bot.place, "team": active.team, "void": "start does not fit",
                                    "rest": row.feet.to_array(), "start": start.to_array(), "yaw": row.yaw, "heading": heading,
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
                    if active.redone >= 3 {
                        let line = serde_json::json!({
                            "id": active.def.id, "n": active.n, "bot": bot.place, "team": active.team, "void": "no rest",
                            "start": start.to_array(),
                        });
                        Self::line(&mut self.tries_log, &line);
                        *self.done.entry(active.def.id.clone()).or_default() += 1;
                        runner.active = None;
                        return Some(Decision::default());
                    }
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
                    let (n, began, redone, team) = (active.n, active.began, active.redone, active.team);
                    let motor = def.speed.is_motor().then(|| {
                        serde_json::json!({
                            "route_end": top_of(&def).to_array(), "reached": active.reached,
                            "presses": std::mem::take(&mut active.presses),
                        })
                    });
                    let place = bot.place;
                    let mut line = self.summary(place, &def, n, &plan, began, redone);
                    if let Some(obj) = line.as_object_mut() {
                        obj.insert("team".into(), team.into());
                        if let Some(motor) = motor {
                            obj.insert("motor".into(), motor);
                        }
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
        let gap = self.burst_ms.max(self.backoff_ms);
        if self.outstanding.is_some() || self.last_request.is_some_and(|at| clock < at + gap) {
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
        self.take_rows(rows_of(body, time), time, clock, bytes)
    }

    /// [`Self::upload`] for the rows of a world already read (the stand-in's dry run gives its own).
    pub fn take_rows(&mut self, rows: Vec<Row>, time: u32, clock: u32, bytes: usize) -> bool {
        let burst = self.outstanding.take().is_some();
        if let Some(sent) = self.last_request {
            self.uploads.push((sent, clock, bytes));
            let took = clock.saturating_sub(sent);
            self.backoff_ms = if took > SLOW_UPLOAD_MS { took } else { 0 };
        }
        if self.bodies_log.is_some() {
            let alive: Vec<serde_json::Value> = rows
                .iter()
                .filter(|r| r.alive)
                .map(|r| serde_json::json!([r.place, (r.feet.x * 100.0).round() / 100.0, (r.feet.y * 100.0).round() / 100.0, (r.feet.z * 100.0).round() / 100.0]))
                .collect();
            Self::line(&mut self.bodies_log, &serde_json::json!({ "t": time, "b": alive }));
        }
        self.fresh = rows.clone();
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

    /// The rows of the last asked world, once ([`Self::fresh`]).
    pub fn take_fresh(&mut self) -> Vec<Row> {
        std::mem::take(&mut self.fresh)
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

/// How far to aim the walk off the start after a rest that did not fit: the last aim moved by half the miss (`start` against the
/// body's `rest`), so the next walk ends where the coast puts the body on the start. It is measured on the body, so it
/// holds whatever the reckoning thought; kept within [`SHIFT_MAX_M`] of the start (further, the miss is not a coast: the
/// body is somewhere else, and the next ordinary world puts the reckoning right).
fn redo_shift(shift: [f32; 2], start: Vec3, rest: Vec3) -> [f32; 2] {
    // Half of the miss each time: the coast flips sign with the direction of the walk, a full step would swing the aim about.
    let next = [shift[0] + 0.5 * (start.x - rest.x), shift[1] + 0.5 * (start.z - rest.z)];
    let length = next[0].hypot(next[1]);
    if length <= SHIFT_MAX_M { next } else { [next[0] / length * SHIFT_MAX_M, next[1] / length * SHIFT_MAX_M] }
}

/// Which team works each ledge: `sites` (key, start) sorted by how much nearer team 0's spawns are than team 1's; the
/// nearer half is team 0's, the rest team 1's. None (everyone takes any) with fewer than two ledges or a team with no
/// spawn.
fn split_sites(spawns: &[(Vec3, u8)], sites: Vec<((i32, i32), Vec3)>) -> HashMap<(i32, i32), u8> {
    let nearest = |team: u8, at: Vec3| spawns.iter().filter(|s| s.1 == team).map(|s| s.0.distance_xz(at)).fold(f32::MAX, f32::min);
    if sites.len() < 2 || nearest(0, Vec3::ZERO) == f32::MAX || nearest(1, Vec3::ZERO) == f32::MAX {
        return HashMap::new();
    }
    let mut by_lean: Vec<((i32, i32), f32)> = sites.into_iter().map(|(key, at)| (key, nearest(0, at) - nearest(1, at))).collect();
    by_lean.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let own = by_lean.len() / 2;
    by_lean.into_iter().enumerate().map(|(i, (key, _))| (key, u8::from(i >= own))).collect()
}

/// Gives a ledge up: its tries leave the pool and no later one is taken.
fn forget_site(pool: &mut VecDeque<TryDef>, bad_sites: &mut Vec<Vec3>, bad: Vec3) {
    bad_sites.push(bad);
    pool.retain(|d| base_of(d).distance_xz(bad) > BAD_SITE_M);
}

/// How far across the approach a rest may be from the start (m): [`REST_FIT_ACROSS_M`], more on a long face (a rail: the half
/// width of the site less 0.3 m, at most 3 m), where the place along the face changes nothing.
fn across_m(site: &Site) -> f32 {
    (site.half - 0.3).clamp(REST_FIT_ACROSS_M, 3.0)
}

/// Whether a body at rest at `rest` facing `yaw` is where a try starts: within [`REST_FIT_ALONG_M`] of the start along the
/// approach `axis` (the plan is made from the actual rest, so the run-up is that much shorter or longer; the log carries
/// `rest_off`) and [`REST_FIT_ACROSS_M`] across it, [`REST_FIT_Y_M`] in height, and facing the heading within 10 degrees.
pub fn rest_fits(rest: Vec3, yaw: f32, start: Vec3, heading: f32, axis: [f32; 2], across_m: f32) -> bool {
    let (dx, dz) = (rest.x - start.x, rest.z - start.z);
    let along = dx * axis[0] + dz * axis[1];
    let across = -dx * axis[1] + dz * axis[0];
    along.abs() <= REST_FIT_ALONG_M
        && across.abs() <= across_m
        && (rest.y - start.y).abs() <= REST_FIT_Y_M
        && wrap(yaw - heading).abs() <= REST_FIT_YAW
}

/// What a try under way takes up: the start, the run-up line to the face and the top past it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Zone {
    start: Vec3,
    centre: [f32; 2],
    /// The far end of the ground past the face a try takes ([`BEYOND_M`] on).
    beyond: [f32; 2],
    /// The team of the bot working it, and whether it is in the part another body near spoils.
    team: u8,
    live: bool,
}

impl Zone {
    fn of(def: &TryDef, team: u8, live: bool) -> Self {
        let centre = def.site.centre();
        let t = def.site.toward;
        Self { start: start_of(def), centre, beyond: [centre[0] + t[0] * BEYOND_M, centre[1] + t[1] * BEYOND_M], team, live }
    }

    /// The least distance between this ground and another's (sampled along both lines).
    fn apart_from(&self, other: &Zone) -> f32 {
        let samples = |z: &Zone| {
            let (s, c, b) = ([z.start.x, z.start.z], z.centre, z.beyond);
            let mid = |a: [f32; 2], b: [f32; 2]| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            [s, mid(s, c), c, mid(c, b), b]
        };
        let near = |from: &Zone, to: &Zone| samples(from).iter().map(|p| to.nearest(Vec3::new(p[0], 0.0, p[1])).1).fold(f32::MAX, f32::min);
        near(self, other).min(near(other, self))
    }

    /// The nearest point of the zone to `p` on the ground plane, and its distance: the zone is the line
    /// from the start to the face and on over the top.
    fn nearest(&self, p: Vec3) -> ([f32; 2], f32) {
        let on = |a: [f32; 2], b: [f32; 2]| {
            let ab = [b[0] - a[0], b[1] - a[1]];
            let len2 = (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-6);
            let k = (((p.x - a[0]) * ab[0] + (p.z - a[1]) * ab[1]) / len2).clamp(0.0, 1.0);
            let q = [a[0] + ab[0] * k, a[1] + ab[1] * k];
            (q, (p.x - q[0]).hypot(p.z - q[1]))
        };
        let (a, b) = (on([self.start.x, self.start.z], self.centre), on(self.centre, self.beyond));
        if a.1 <= b.1 { a } else { b }
    }
}

/// A bot on its way to a start stops short of the ground of an enemy's try in its sensitive part: enemies
/// collide, and the run is lost when one comes near.
const YIELD_M: f32 = 4.0;
/// A bot waits this long at most for enemy tries on its way, ms, from the first wait.
const YIELD_MAX_MS: u32 = 60_000;
/// Within this of an enemy's live try a waiting bot walks off instead of standing.
const INSIDE_M: f32 = 3.0;

fn yield_to(zones: &[Zone], team: u8, feet: Vec3) -> bool {
    zones.iter().any(|z| z.live && z.team != team && z.nearest(feet).1 < YIELD_M)
}

/// A bot with no try that stands within [`CLEAR_M`] of a try under way walks off, straight away from it
/// where it can (to a floor of the grid at least [`CLEAR_M`] + 0.5 m from every zone), at a run.
fn keep_clear(zones: &[Zone], world: &World, bot: &mut Bot, feet: Vec3) {
    let Some((point, near)) = zones.iter().map(|z| z.nearest(feet)).min_by(|a, b| a.1.total_cmp(&b.1)) else {
        return;
    };
    if near >= CLEAR_M {
        return;
    }
    let away = {
        let (dx, dz) = (feet.x - point[0], feet.z - point[1]);
        let len = dx.hypot(dz);
        if len < 0.05 { [1.0, 0.0] } else { [dx / len, dz / len] }
    };
    for turn in [0.0f32, 0.785, -0.785, 1.571, -1.571, 2.356, -2.356, std::f32::consts::PI] {
        let (s, c) = turn.sin_cos();
        let dir = [away[0] * c - away[1] * s, away[0] * s + away[1] * c];
        for r in [7.0f32, 10.0, 14.0] {
            let want = Vec3::new(feet.x + dir[0] * r, feet.y, feet.z + dir[1] * r);
            let Some(node) = path::goal_floor(&world.grid, want) else {
                continue;
            };
            let at = world.grid.position(node);
            if at.distance_xz(want) < 1.5 && (at.y - feet.y).abs() < 3.0 && zones.iter().all(|z| z.nearest(at).1 >= CLEAR_M + 0.5) {
                bot.intent = Intent { go: Go::To { at, within_m: 0.5 }, pace: Pace::Run, fire: false, ..Intent::stay() };
                return;
            }
        }
    }
}

/// From each team's first spawn: the floors it reaches and the floors that reach it (any link of finite
/// cost), to check a site once.
struct ReachSets {
    forward: [Vec<bool>; 2],
    back: [Vec<bool>; 2],
}

impl ReachSets {
    fn build(world: &World) -> Self {
        let grid = &world.grid;
        let n = grid.len();
        let mut reverse: Vec<Vec<NodeId>> = vec![Vec::new(); n];
        for f in 0..n as NodeId {
            for (m, kind) in grid.links(f) {
                if grid.link_cost(f, m, kind).is_finite() {
                    reverse[m as usize].push(f);
                }
            }
        }
        let flood = |team: i32, back: bool| {
            let mut seen = vec![false; n];
            if let Some(start) = world.map.spawns.iter().find(|s| s.team == team).and_then(|s| grid.locate(s.position, 3.0)) {
                let mut stack = vec![start];
                seen[start as usize] = true;
                while let Some(f) = stack.pop() {
                    let next: Vec<NodeId> = if back {
                        reverse[f as usize].clone()
                    } else {
                        grid.links(f).filter(|&(m, kind)| grid.link_cost(f, m, kind).is_finite()).map(|(m, _)| m).collect()
                    };
                    for m in next {
                        if !seen[m as usize] {
                            seen[m as usize] = true;
                            stack.push(m);
                        }
                    }
                }
            }
            seen
        };
        Self { forward: [flood(0, false), flood(1, false)], back: [flood(0, true), flood(1, true)] }
    }

    /// Why a try's site is refused, or none: its start and top must be reached from both teams' spawns,
    /// and both must have a way back.
    fn refuse(&self, world: &World, def: &TryDef) -> Option<String> {
        let floor = |at: Vec3| path::goal_floor(&world.grid, at);
        // A gap's far side is not reached until a link joins it, and may be no floor at the point past the face: its
        // start is the only floor the bots walk to.
        let top = if def.site.gap { None } else { floor(top_of(def)) };
        let Some(start) = floor(start_of(def)) else {
            return Some("the start is on no floor".to_owned());
        };
        if !def.site.gap && top.is_none() {
            return Some("the top is on no floor".to_owned());
        }
        for team in 0..2usize {
            let checks = [
                ("start reached from", Some(self.forward[team][start as usize])),
                ("top reached from", top.map(|top| self.forward[team][top as usize])),
                ("start has a way back to", Some(self.back[team][start as usize])),
                ("top has a way back to", top.map(|top| self.back[team][top as usize])),
            ];
            if let Some((what, _)) = checks.iter().find(|c| c.1 == Some(false)) {
                return Some(format!("{what} the spawns of team {team}: no"));
            }
        }
        None
    }
}

/// How far a try's rest position is from its planned start: metres on the ground, metres in height, degrees
/// of facing (for the log).
fn rest_off(def: &TryDef, plan: &Plan) -> serde_json::Value {
    let start = start_of(def);
    let round = |v: f32| (v * 100.0).round() / 100.0;
    serde_json::json!([
        round(plan.rest.distance_xz(start)),
        round(plan.rest.y - start.y),
        round(wrap(plan.heading - heading_of(def)).to_degrees()),
    ])
}

/// A site by its deck (0.1 m), the key of the checks and of the bots' no-way lists.
fn site_key(def: &TryDef) -> (i32, i32) {
    let base = base_of(def);
    ((base.x * 10.0).round() as i32, (base.z * 10.0).round() as i32)
}

/// A pool try lost with a life goes back to the pool (as one more repeat of its row, on its own face and
/// lane again), not to its bot: the lanes and the team lock are the pool's.
fn back_to_pool(pool: &mut VecDeque<TryDef>, mut def: TryDef) {
    def.site.lower = base_of(&def);
    def.lane = 0.0;
    def.repeat = 1;
    if let Some(same) = pool.iter_mut().find(|d| d.id == def.id) {
        same.repeat += 1;
    } else {
        pool.push_front(def);
    }
}

/// The gap between two lanes of the same face, m.
const LANE_GAP_M: f32 = 2.0;

/// A try's site before its lane shift.
fn base_of(def: &TryDef) -> Vec3 {
    let perp = [-def.site.toward[1], def.site.toward[0]];
    Vec3::new(def.site.lower.x - perp[0] * def.lane, def.site.lower.y, def.site.lower.z - perp[1] * def.lane)
}

/// The nearest free lane for a try of `team` on its face, given the (face, lane, team) of the tries under
/// way; none while the other team has a try on the face (enemies collide: a ledge and its lanes take one
/// team at a time). Lanes: 0,
/// then 2.5 m either side, and so on, as far as the ledge is wide minus what the approach drifts
/// sideways; none when every lane of the face within reach is taken.
fn free_lane(def: &TryDef, taken: &[Taken], team: u8) -> Option<f32> {
    let drift = (def.runup + def.edge.max(0.0)) * def.angle.to_radians().sin().abs();
    let reach = def.site.half - 1.0 - drift;
    let base = base_of(def);
    if taken.iter().any(|(b, _, other, _)| *other != team && b.distance_xz(base) < 0.5) {
        return None;
    }
    let busy = |lane: f32| taken.iter().any(|(b, l, _, _)| b.distance_xz(base) < 0.5 && (l - lane).abs() < LANE_GAP_M);
    // The ground of this try at the lane against that of every enemy try: other sites, other faces, the same
    // wall from its other side.
    let enemy_near = |lane: f32| {
        let perp = [-def.site.toward[1], def.site.toward[0]];
        let mut shifted = def.clone();
        shifted.site.lower = Vec3::new(def.site.lower.x + perp[0] * lane, def.site.lower.y, def.site.lower.z + perp[1] * lane);
        let mine = Zone::of(&shifted, team, true);
        taken.iter().any(|(_, _, other, zone)| *other != team && mine.apart_from(zone) < LOCK_M)
    };
    (0..)
        .map(|k| {
            let step = 2.5 * ((k + 1) / 2) as f32;
            if k % 2 == 1 { step } else { 0.0 - step }
        })
        .take_while(|lane| lane.abs() <= reach.max(0.0))
        .find(|&lane| !busy(lane) && !enemy_near(lane))
}

/// The tries under way as the pool sees them: the base of the site, the lane, the team, the ground.
type Taken = (Vec3, f32, u8, Zone);

/// Another team's try this near (m), ground to ground, locks a ledge: enemies collide.
const LOCK_M: f32 = 4.0;

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
                inserted: motion.inserted_time_ms,
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
            inserted: motion.inserted_time_ms,
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
    // Worlds arrive out of order now and then (an older upload after a newer): by their own ticks.
    let mut sorted: Vec<&Row> = rows.to_vec();
    sorted.sort_by_key(|r| r.time);
    let rows = &sorted[..];
    let t = def.site.toward;
    let a = approach(def);
    let [cx, cz] = def.site.centre();
    let deck = def.site.lower.y;
    let top = deck + def.site.rise;
    let press = plan.press;
    let mut v = serde_json::json!({
        "id": def.id, "n": n, "bot": place, "site": def.site.name, "lane": def.lane,
        "top_depth": def.site.depth, "kind": format!("{:?}", def.site.kind(def.edge, def.speed)).to_lowercase(),
        "plan": {
            "edge": def.edge, "angle": def.angle, "face": format!("{:?}", def.face).to_lowercase(),
            "runup": def.runup, "speed": format!("{:?}", def.speed).to_lowercase(), "mid": format!("{:?}", def.mid),
            "run_planned": plan.run,
        },
        "rest": plan.rest.to_array(), "heading": plan.heading,
        "start": start_of(def).to_array(), "rest_off": rest_off(def, plan),
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
    // The way the body went from rest: to the press, else to the end of the keys; against the approach.
    let went_to = press.unwrap_or(plan.end.saturating_sub(600));
    if let Some(p) = feet_at(rows, went_to) {
        let (dx, dz) = (p.x - plan.rest.x, p.z - plan.rest.z);
        let length = dx.hypot(dz);
        if length > 0.5 {
            let cos = ((dx * a[0] + dz * a[1]) / length).clamp(-1.0, 1.0);
            obj.insert("dir_off".into(), serde_json::json!((cos.acos().to_degrees() * 10.0).round() / 10.0));
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
    let mut window: Vec<u32> = rows.iter().map(|r| r.time).filter(|&t| t + 300 >= from && t <= from + 2_000).collect();
    window.sort_unstable();
    let gap = window.windows(2).map(|w| w[1].saturating_sub(w[0])).max().unwrap_or(0);
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
        TryDef { id: "t".into(), who: Who::Bot(3), lane: 0.0, site: site(), edge, angle, face, runup, speed, mid, repeat: 1, suicides: 0 }
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

    fn t4(d: &TryDef, lane: f32, team: u8) -> Taken {
        (base_of(d), lane, team, Zone::of(d, team, true))
    }

    fn wide_def(angle: f32, runup: f32) -> TryDef {
        let site = named_site("chem125c", &HashMap::new()).expect("a site");
        TryDef { id: "p".into(), who: Who::Pool, lane: 0.0, site, edge: 1.0, angle, face: Face::Along, runup, speed: Speed::Sprint, mid: Mid::Hold, repeat: 3, suicides: 0 }
    }

    #[test]
    fn a_bot_of_the_test_presses_no_fire_aim_grenade_or_use_while_it_walks() {
        let mut jt = JumpTest::new(PathBuf::from("none"), PathBuf::from("none"), "t".into(), 120);
        jt.runners.entry(3).or_default();
        let noisy = key::FORWARD | key::SPRINT | key::FIRE | key::AIM | key::GRENADE | key::RELOAD | key::USE | key::DROP | key::SPOT | key::WEAPON_1 | key::QUICK[0];
        let mut decision = Decision { keys: noisy, turn: (0.1, 0.0) };
        jt.quiet(3, &mut decision);
        assert_eq!(decision.keys, key::FORWARD | key::SPRINT);
        assert_eq!(decision.turn, (0.1, 0.0));
        // A bot of the match with no try rests: the same.
        let mut other = Decision { keys: noisy, turn: (0.0, 0.0) };
        jt.quiet(9, &mut other);
        assert_eq!(other.keys, key::FORWARD | key::SPRINT);
        // With the others allowed to play they keep their keys.
        jt.idle_others = false;
        let mut free = Decision { keys: noisy, turn: (0.0, 0.0) };
        jt.quiet(9, &mut free);
        assert_eq!(free.keys, noisy);
        jt.quiet(3, &mut free);
        assert_eq!(free.keys, key::FORWARD | key::SPRINT);
    }

    #[test]
    fn a_bot_near_its_start_does_not_kill_itself_however_long_it_walked() {
        // The walk still to go (straight 10 m: 2.6 s at the slack sprint) is far under the budget of 8 s and more.
        assert!(remaining_ms(10.0) < budget_ms(0.0));
        // 80 m to go against a spawn 20 m from the start: the respawn and the walk from the spawn are quicker.
        assert!(remaining_ms(80.0) > budget_ms(20.0));
        assert!(remaining_ms(30.0) < budget_ms(20.0));
    }

    #[test]
    fn the_budget_is_the_respawn_and_a_slack_sprint_from_the_nearest_spawn() {
        assert_eq!(budget_ms(0.0), 8_000);
        // 50 m at 5 m/s is 10 s, with the slack 13 s.
        assert_eq!(budget_ms(50.0), 21_000);
    }

    #[test]
    fn the_quiet_keys_let_the_suicide_through() {
        let mut jt = JumpTest::new(PathBuf::from("none"), PathBuf::from("none"), "t".into(), 120);
        jt.runners.entry(3).or_default();
        let mut d = Decision { keys: key::SUICIDE | key::FIRE, turn: (0.0, 0.0) };
        jt.quiet(3, &mut d);
        assert_eq!(d.keys, key::SUICIDE);
    }

    #[test]
    fn a_ledge_given_up_takes_all_its_pool_tries_with_it() {
        let mut pool = VecDeque::from([wide_def(0.0, 5.5), TryDef { id: "q".into(), ..wide_def(30.0, 5.5) }]);
        pool.push_back(TryDef { site: named_site("chem060a", &HashMap::new()).expect("a site"), ..wide_def(0.0, 3.5) });
        let mut bad = Vec::new();
        let site = base_of(&pool[0]);
        forget_site(&mut pool, &mut bad, site);
        assert_eq!(pool.len(), 1);
        assert_eq!(pool[0].site.name, "chem060a");
        assert_eq!(bad.len(), 1);
    }

    #[test]
    fn a_rest_fits_within_six_tenths_along_the_approach_four_across_a_third_up_and_ten_degrees_of_the_heading() {
        let start = Vec3::new(-6.0, 1.72, 93.9);
        let axis = [1.0, 0.0];
        assert!(rest_fits(start, 0.0, start, 0.0, axis, 0.5));
        assert!(rest_fits(Vec3::new(-5.8, 1.9, 94.1), 0.1, start, 0.0, axis, 0.5));
        // The coast past the start (0.5 m along the approach) fits; 0.2 m more, 0.5 m sideways, another ledge's start 9 m off,
        // the top deck, facing 20 degrees off do not.
        assert!(rest_fits(Vec3::new(-5.5, 1.72, 93.9), 0.0, start, 0.0, axis, 0.5));
        assert!(!rest_fits(Vec3::new(-5.2, 1.72, 93.9), 0.0, start, 0.0, axis, 0.5));
        assert!(rest_fits(Vec3::new(-6.0, 1.72, 94.25), 0.0, start, 0.0, axis, 0.5));
        assert!(rest_fits(Vec3::new(-6.0, 1.72, 94.4), 0.0, start, 0.0, axis, 0.5));
        assert!(!rest_fits(Vec3::new(-6.0, 1.72, 94.6), 0.0, start, 0.0, axis, 0.5));
        // On a long face (a rail) the place along it changes nothing: a wider band across.
        assert!(rest_fits(Vec3::new(-6.0, 1.72, 95.9), 0.0, start, 0.0, axis, 2.0));
        assert!(!rest_fits(Vec3::new(3.0, 1.72, 93.9), 0.0, start, 0.0, axis, 0.5));
        assert!(!rest_fits(Vec3::new(-6.0, 2.32, 93.9), 0.0, start, 0.0, axis, 0.5));
        assert!(!rest_fits(start, 0.35, start, 0.0, axis, 0.5));
        // The axis turns with the approach: along z the same 0.5 m is along, 0.5 m in x is across.
        assert!(rest_fits(Vec3::new(-6.0, 1.72, 94.4), 0.0, start, 0.0, [0.0, 1.0], 0.5));
        assert!(!rest_fits(Vec3::new(-5.5, 1.72, 93.9), 0.0, start, 0.0, [0.0, 1.0], 0.5));
        // The heading wraps.
        assert!(rest_fits(start, 3.1, start, -3.1, axis, 0.5));
    }

    #[test]
    fn a_zone_is_the_start_the_run_up_and_the_top() {
        let d = def(0.8, 0.0, Face::Along, 3.5, Speed::Walk, Mid::Hold);
        let zone = Zone::of(&d, 0, true);
        let mid = Vec3::new((zone.start.x + zone.centre[0]) / 2.0, zone.start.y, (zone.start.z + zone.centre[1]) / 2.0);
        assert!(zone.nearest(zone.start).1 < 0.01);
        assert!(zone.nearest(mid).1 < 0.01);
        assert!(zone.nearest(Vec3::new(mid.x + 3.0, mid.y, mid.z)).1 > 2.9);
        // The top is in it as well; a point behind the start is measured from the start.
        assert!(zone.nearest(Vec3::new(zone.beyond[0], zone.start.y, zone.beyond[1] + 1.0)).1 < 1.01);
        // The ground past the face is in it up to BEYOND_M.
        assert!(zone.nearest(Vec3::new(zone.beyond[0], zone.start.y, zone.beyond[1] - 0.5)).1 < 0.51);
        let behind = Vec3::new(zone.start.x, zone.start.y, zone.start.z + 6.0);
        assert!(zone.nearest(behind).1 >= CLEAR_M);
    }

    #[test]
    fn a_bot_waits_short_of_an_enemy_try_in_its_sensitive_part_and_a_lost_try_goes_back_to_the_pool() {
        let d = def(0.8, 0.0, Face::Along, 3.5, Speed::Walk, Mid::Hold);
        let zone = Zone::of(&d, 1, true);
        let near = Vec3::new(zone.start.x + 2.0, zone.start.y, zone.start.z);
        let far = Vec3::new(zone.start.x + 4.0, zone.start.y, zone.start.z);
        assert!(yield_to(&[zone], 0, near));
        assert!(!yield_to(&[zone], 0, far));
        // A teammate's try, and an enemy's that is only walking to its start, are not waited for.
        assert!(!yield_to(&[Zone::of(&d, 0, true)], 0, near));
        assert!(!yield_to(&[Zone::of(&d, 1, false)], 0, near));
        // The lost try is one more repeat of its row, on its own face again.
        let mut pool = VecDeque::from([TryDef { repeat: 2, ..wide_def(0.0, 5.5) }]);
        let mut lost = wide_def(0.0, 5.5);
        lost.lane = 2.5;
        lost.site.lower = Vec3::new(lost.site.lower.x, lost.site.lower.y, lost.site.lower.z + 2.5);
        let base = base_of(&lost);
        back_to_pool(&mut pool, lost);
        assert_eq!(pool.len(), 1);
        assert_eq!(pool[0].repeat, 3);
        let other = TryDef { id: "other".into(), ..wide_def(0.0, 5.5) };
        back_to_pool(&mut pool, TryDef { lane: 2.5, ..other.clone() });
        assert_eq!(pool[0].id, "other");
        assert_eq!(pool[0].lane, 0.0);
        assert!(base_of(&pool[0]).distance_xz(base) < 3.0);
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
        assert_eq!(free_lane(&d, &[], 0), Some(0.0));
        assert_eq!(free_lane(&d, &[t4(&d, 0.0, 0)], 0), Some(2.5));
        assert_eq!(free_lane(&d, &[t4(&d, 0.0, 0), t4(&d, 2.5, 0)], 0), Some(-2.5));
        // The other team on the face locks it, whatever the lane.
        assert_eq!(free_lane(&d, &[t4(&d, 5.0, 1)], 0), None);
        assert_eq!(free_lane(&d, &[t4(&d, 5.0, 1)], 1), Some(0.0));
        // So does an enemy try on another face 1 m off (the two sides of one wall), not one 40 m off.
        let mut next_door = d.clone();
        next_door.site.lower = Vec3::new(d.site.lower.x, d.site.lower.y, d.site.lower.z + 1.0);
        assert!(base_of(&next_door).distance_xz(base) > 0.5);
        // Its lanes within 4 m of the enemy's ground are shut; one further along the wall is free.
        assert!(free_lane(&d, &[t4(&next_door, 0.0, 1)], 0).is_some_and(|lane| lane.abs() >= 5.0));
        assert_eq!(free_lane(&d, &[t4(&next_door, 0.0, 0)], 0), Some(0.0));
        // Another face is not in the way.
        let mut far = d.clone();
        far.site.lower = Vec3::new(d.site.lower.x + 40.0, d.site.lower.y, d.site.lower.z);
        assert_eq!(free_lane(&d, &[t4(&far, 0.0, 1)], 0), Some(0.0));
        // A 60 degree run-up drifts 4.8 m sideways: the lanes of a 12 m wall are one.
        let wall = TryDef { site: named_site("chem125a", &HashMap::new()).expect("a site"), ..wide_def(60.0, 5.5) };
        assert_eq!(free_lane(&wall, &[], 0), Some(0.0));
        assert_eq!(free_lane(&wall, &[t4(&wall, 0.0, 0)], 0), None);
        // A 1.2 m ledge has no lane but the middle.
        let narrow = TryDef { site: named_site("chem060a", &HashMap::new()).expect("a site"), ..wide_def(0.0, 3.5) };
        assert_eq!(free_lane(&narrow, &[t4(&narrow, 0.0, 0)], 0), None);
    }

    #[test]
    fn a_free_bot_takes_the_next_pool_try_on_a_free_lane_and_the_site_moves_along_the_face() {
        let mut jt = JumpTest::new(PathBuf::from("none"), PathBuf::from("none"), "t".into(), 120);
        jt.pool.push_back(wide_def(0.0, 5.5));
        let first = jt.take_from_pool(3, 0).expect("a try");
        assert_eq!(first.lane, 0.0);
        assert_eq!(jt.pool[0].repeat, 2);
        jt.runners.entry(3).or_default().active = Some(Active { def: first.clone(), n: 1, phase: Phase::Rest { until: 0 }, redone: 0, began: 0, presses: Vec::new(), reached: None, team: 0, shift: [0.0, 0.0] });
        let second = jt.take_from_pool(4, 0).expect("a try");
        // An enemy bot finds the ledge locked.
        assert!(jt.take_from_pool(7, 1).is_none());
        assert_eq!(second.lane, 2.5);
        // The shift is along the face: chem125c faces -x, so the lane moves z.
        assert!((second.site.lower.x - first.site.lower.x).abs() < 1e-4);
        assert!((second.site.lower.z - first.site.lower.z).abs() > 2.4);
        assert_eq!(base_of(&second), base_of(&first));
        assert!(jt.holds_end());
        // A bot that has no way to the site takes no try there.
        jt.pool.push_back(wide_def(0.0, 5.5));
        jt.runners.clear();
        jt.no_route.insert((6, site_key(&second)));
        assert!(jt.take_from_pool(6, 0).is_none());
        assert!(jt.take_from_pool(5, 0).is_some());
    }

    #[test]
    fn after_a_misfit_the_walk_is_aimed_off_the_start_by_half_the_miss_of_the_body() {
        let start = Vec3::new(10.0, 2.0, 20.0);
        // The body came to rest 0.5 m past the start (x) and 0.2 m to the side: aim 0.25 m short and 0.1 m back.
        let shift = redo_shift([0.0, 0.0], start, Vec3::new(10.5, 2.0, 20.2));
        assert!((shift[0] + 0.25).abs() < 1e-4 && (shift[1] + 0.1).abs() < 1e-4, "{shift:?}");
        // Still 0.1 m past the start (x) on the line: the shift adds up (an integrator of gain a half).
        let again = redo_shift(shift, start, Vec3::new(start.x + 0.1, 2.0, start.z));
        assert!((again[0] + 0.30).abs() < 1e-4 && (again[1] + 0.1).abs() < 1e-4, "{again:?}");
        // On the start: the shift stays.
        assert_eq!(redo_shift(again, start, start), again);
        // A rest 15 m off is no coast: the shift stays within the cap.
        let far = redo_shift([0.0, 0.0], start, Vec3::new(25.0, 2.0, 20.0));
        assert!((far[0].hypot(far[1]) - SHIFT_MAX_M).abs() < 1e-4, "{far:?}");
    }

    #[test]
    fn the_bodies_of_an_asked_world_go_once_to_the_reckoning() {
        let mut jt = JumpTest::new(PathBuf::from("none"), std::env::temp_dir(), "t".into(), 120);
        let rows = vec![row(1_000, 2.0, 5.0, false, 0), Row { place: 4, ..row(1_000, 2.0, 7.0, false, 0) }];
        jt.take_rows(rows.clone(), 1_000, 1_050, 100);
        assert_eq!(jt.take_fresh().len(), 2, "every body of the world, whoever works a try");
        assert!(jt.take_fresh().is_empty(), "once");
    }

    #[test]
    fn a_landing_try_releases_everything_in_the_air_and_holds_its_keys_from_the_landing() {
        assert_eq!(parse_mid("land_back_1100"), Ok(Mid::Land { keys: Landing::Back, after_ms: 1_100 }));
        assert_eq!(parse_mid("land_none_900"), Ok(Mid::Land { keys: Landing::Nothing, after_ms: 900 }));
        assert!(parse_mid("land_back").is_err() && parse_mid("land_up_900").is_err() && parse_mid("land_back_100").is_err());
        let d = def(1.0, 0.0, Face::Along, 5.5, Speed::Sprint, Mid::Land { keys: Landing::Back, after_ms: 1_100 });
        let plan = plan_from(&d, site_start(), 0.0, 1_000, &RunTable::default()).expect("fits");
        let press = plan.press.expect("a press");
        assert_eq!(plan.end, press + 1_100 + LAND_KEYS_MS);
        let before = plan.at(press + MID_AT_MS - TICK_MS);
        assert!(before.keys & key::FORWARD != 0, "the run goes on to the take-off");
        assert_eq!(plan.at(press + MID_AT_MS + TICK_MS).keys, 0, "all released in the air");
        assert_eq!(plan.at(press + 1_100 - TICK_MS).keys, 0);
        assert_eq!(plan.at(press + 1_100).keys, key::BACK);
        assert_eq!(plan.at(plan.end).keys, 0, "and nothing after the keys");
    }

    #[test]
    fn the_ledges_nearer_a_teams_spawns_are_that_teams_and_a_bot_takes_only_its_own() {
        let spawns = [(Vec3::new(0.0, 0.0, 0.0), 0), (Vec3::new(100.0, 0.0, 0.0), 1)];
        let site = |x: f32| ((x as i32 * 10, 0), Vec3::new(x, 0.0, 5.0));
        let sides = split_sites(&spawns, vec![site(10.0), site(90.0), site(40.0), site(70.0)]);
        assert_eq!(sides[&site(10.0).0], 0);
        assert_eq!(sides[&site(40.0).0], 0);
        assert_eq!(sides[&site(70.0).0], 1);
        assert_eq!(sides[&site(90.0).0], 1);
        // One ledge, or one team's spawns only: no split.
        assert!(split_sites(&spawns, vec![site(10.0)]).is_empty());
        assert!(split_sites(&spawns[..1], vec![site(10.0), site(90.0)]).is_empty());
        // The pool: team 0 gets the ledge on its side, team 1 the other, whatever the file's order.
        let mut jt = JumpTest::new(PathBuf::from("none"), PathBuf::from("none"), "t".into(), 120);
        let a = TryDef { id: "a".into(), ..wide_def(0.0, 5.5) };
        let mut b = TryDef { id: "b".into(), ..wide_def(0.0, 5.5) };
        b.site.lower = Vec3::new(b.site.lower.x + 80.0, b.site.lower.y, b.site.lower.z);
        jt.pool.extend([a.clone(), b.clone()]);
        jt.sides = HashMap::from([(site_key(&a), 0), (site_key(&b), 1)]);
        assert_eq!(jt.take_from_pool(1, 1).expect("a try").id, "b");
        assert_eq!(jt.take_from_pool(2, 0).expect("a try").id, "a");
    }

    #[test]
    fn a_site_with_a_depth_is_a_climb_when_the_jump_lands_on_the_top_and_over_when_it_clears_it() {
        let sites = parse_sites("c060_3 -38.38 1.96 30.12 0 -1 0.650 0.25 0.60 1.50  # a 1.5 m top
c060_x -1 2 3 0 1 0.6 0.25 0.6
s100_3 14.2 16.32 138.72 1 0 1.009 0.25 0.50 7.01
");
        assert_eq!(sites["c060_3"].depth, Some(1.5));
        assert_eq!(sites["c060_x"].depth, None, "the column is optional");
        // A 1.5 m top: the walking jump (2.67 m) from 0.3 m before the face lands past it; no walking edge is a climb.
        assert_eq!(sites["c060_3"].kind(0.3, Speed::Walk), Kind::Over);
        assert_eq!(sites["c060_3"].kind(1.3, Speed::Walk), Kind::Over);
        // A 7 m top takes the walking jump from 1.3 m (1.3 + 2.67) and the sprint jump from 2.5 m (2.5 + 4.4).
        assert_eq!(sites["s100_3"].kind(1.3, Speed::Walk), Kind::Climb);
        assert_eq!(sites["s100_3"].kind(2.5, Speed::Sprint), Kind::Climb);
        assert_eq!(sites["s100_3"].kind(3.0, Speed::Sprint), Kind::Over);
        assert_eq!(sites["c060_x"].kind(0.3, Speed::Walk), Kind::Unknown);
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
    fn a_sprint_from_a_stop_presses_at_the_asked_time_after_the_first_key() {
        let table = RunTable::default();
        let tries = parse_tries(
            "sp1 * rs3 0 0 along 0 sprint_at_850 none 2\nsp2 * rs3 0 0 along 0 sprint_at_800 none 2\n",
            &HashMap::from([("rs3".to_owned(), site())]),
        )
        .unwrap();
        assert_eq!((tries[0].speed, tries[1].speed), (Speed::SprintAt(850), Speed::SprintAt(800)));
        assert!(parse_tries("x * rs3 0 0 along 0 sprint_at_50 none 1\n", &HashMap::from([("rs3".to_owned(), site())])).is_err());
        for (d, ms) in tries.iter().zip([850u32, 800]) {
            assert_eq!(base_keys(d), key::FORWARD | key::SPRINT);
            let plan = plan_from(d, start_of(d), heading_of(d), 10_000, &table).unwrap();
            // A tick rounds to 50 ms: 850 is 17 ticks, 800 is 16.
            assert_eq!(plan.press, Some(10_000 + (ms + 25) / 50 * 50));
            assert_eq!(plan.at(10_000 + 50).keys, key::FORWARD | key::SPRINT);
        }
    }

    #[test]
    fn a_gap_site_is_flagged_by_its_trailing_word() {
        let sites = parse_sites(
            "rg1 55.88 8.58 66.88 -0.4472 0.8944 0.890 0.868 0.30 gap  # a gap\nrc 11.4 10.17 66 1 0 1.03 0.4 0.3\nd1 1 2 3 1 0 0.6 0.25 0.6 1.01 gap\n",
        );
        assert!(sites["rg1"].gap && sites["rg1"].depth.is_none());
        assert_eq!(sites["rg1"].kind(1.2, Speed::Walk), Kind::Gap);
        assert!(!sites["rc"].gap);
        assert!(sites["d1"].gap && sites["d1"].depth == Some(1.01));
    }

    #[test]
    fn a_standing_jump_presses_on_the_first_tick_with_no_move_key() {
        let table = RunTable::default();
        assert_eq!(parse_mid("step_200"), Ok(Mid::Step { after_ms: 200 }));
        assert!(parse_mid("step_2000").is_err());
        for (mid, forward_after) in [(Mid::Step { after_ms: 0 }, Some(0)), (Mid::Step { after_ms: 200 }, Some(200)), (Mid::Hold, None)] {
            let d = def(0.2, 0.0, Face::Along, 0.0, Speed::Stand, mid);
            assert_eq!(base_keys(&d), 0);
            let plan = plan_from(&d, start_of(&d), heading_of(&d), 10_000, &table).unwrap();
            assert_eq!(plan.press, Some(10_000), "{mid:?}");
            let keys = |after: u32| plan.at(10_000 + after).keys;
            assert_eq!(keys(0) & key::JUMP, key::JUMP, "{mid:?}");
            assert_eq!(keys(2 * TICK_MS) & key::JUMP, 0, "{mid:?}");
            for after in [0, 100, 150, 200, 300, 600] {
                let want = forward_after.is_some_and(|from| after >= from);
                assert_eq!(keys(after) & key::FORWARD != 0, want, "{mid:?} at {after}");
            }
        }
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
            inserted: 0,
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
        // Worlds in the order they arrived, the newest first: the same result, and a small gap (not an underflow).
        let mut shuffled: Vec<&Row> = up.iter().collect();
        shuffled.reverse();
        let v = summarize(&d, 1, 3, &plan, 0, 0, &shuffled, &[]);
        assert_eq!(v["outcome"], "landed", "{v}");
        assert!(v["world_gap_ms"].as_u64().unwrap() <= 1_200, "{v}");
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
        jt.runners.entry(3).or_default().active = Some(Active { def: d, n: 1, phase: Phase::Run(plan), redone: 0, began: 0, presses: Vec::new(), reached: None, team: 0, shift: [0.0, 0.0] });
        assert!(jt.wants_world(9_500));
        // One at a time: not again until answered or forgotten.
        assert!(!jt.wants_world(9_700));
        assert!(!jt.wants_world(10_000));
        assert!(jt.wants_world(10_200), "forgotten after the timeout");
        assert!(jt.holds_end());
    }
}
