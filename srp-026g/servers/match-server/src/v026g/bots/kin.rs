//! How a player's body moves for the keys it holds: the bot's model of
//! the game's movement, used to keep track of every body between two
//! worlds ([`super::body`]) and to plan a bot's steps ([`super::motor`]).
//!
//! Read in the 0.26g client's code and checked tick by tick on a recorded
//! match (the private `KINEMATICS-CODE.md` of the bot research; summed up
//! in `docs/notes/player_bots.md` §4):
//! - **Movement is the animation's** (`set_physics_controller_walk_vector`
//!   0x2D9060): each tick the body moves at the movement class's desired
//!   speed × the body's factor `f = 1 + Σ movement_speed_modifier`
//!   (0x2D5AE0; weapons, mods, a hurt leg, the battery). PROVEN.
//! - **Desired speeds**, m/s (forward / side / back): walk 3.5 / 2.5 /
//!   3.0, aiming 3.0 / 2.0 / 2.5, crouched 2.5 / 2.0 / 2.0, crouched and
//!   aiming 2.0 / 1.5 / 1.5; sprint 5.0 (the config's 5.5 is the sprint
//!   animation's own speed). Forward diagonals go at the forward speed,
//!   back diagonals at the back speed; F+B and L+R cancel. PROVEN.
//! - **Sprint** needs SPRINT and FORWARD with none of BACK, FIRE, CROUCH
//!   (0x2F1220); aiming blocks it (LIKELY); strafing does not. PROVEN.
//! - **A change of speed** is a 0.3 s animation blend (0x921804): linear
//!   over 6 ticks from the present velocity to the new one, held in the
//!   body's frame, so a turn turns it at once. Blend PROVEN, the linear
//!   shape LIKELY (the best fit on the recording).
//! - **Ways:** forward at yaw `a` is `(-sin a, cos a)` in the map frame; a
//!   positive yaw turns left; LEFT goes to yaw + 90°. PROVEN.
//! - **Order:** a world of tick T has T's turn and keys; its position is
//!   from before T's move, which uses T's keys and the yaw after T's turn
//!   (`body.rs` keeps that order). PROVEN. The yaw is never wrapped by the
//!   game; the pitch is clamped at ±1. PROVEN.
//!
//! On the recording this model, with the body's level read from a world,
//! lands a median 0.33 m (p90 1.55 m) from the next world ~5 s on, walls
//! not counted. Every world puts the body back where the game has it.

use std::f32::consts::{PI, TAU};

use serde::{Deserialize, Serialize};

use super::pitch::PitchScale;
use crate::v026g::drive::key;

/// The game's ticks, seconds.
pub const TICK_S: f32 = 0.05;

/// The desired speeds of a movement class: forward, side, back (m/s).
pub type Speeds = [f32; 3];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Kinematics {
    pub walk: Speeds,
    pub aim: Speeds,
    pub crouch: Speeds,
    pub crouch_aim: Speeds,
    pub sprint: f32,
    /// Ticks of the blend to a new velocity.
    pub blend_ticks: u32,
    /// [A318] How a blend that a further key change interrupts goes on ([`Mixer`]).
    pub mixer: Mixer,
    /// [A318] The blend a world's fix starts the body's reckoning with ([`FixBlend`]).
    pub fix_blend: FixBlend,
    /// [A318] Along what a step the grid stops goes on ([`WallSlide`]).
    pub wall_slide: WallSlide,
    /// Which jump links the reckoned body (`body.rs`) takes up after a jump press ([`JumpSnap`]).
    pub jump_snap: JumpSnap,
    /// Where the reckoned body falls off a ledge ([`Edge`]).
    pub edge: Edge,
    /// [A318] How the reckoned body comes down a drop ([`DropFall`]).
    pub drop_fall: DropFall,
    /// [A269c] What a route adds to the cost of a walk onto a floor near an open side (with [`Edge::ClientWalksOff`], the grid's
    /// table of open sides), times the walk's own: the route keeps a body's width in from an edge the capsule walks off where the
    /// floor allows (`RouteFilter::weighing_open_sides`). 2 makes a cell's walk there cost 0.75 m, as a deep fall's edge does
    /// (`mapdata::grid::FALL_EDGE_PENALTY`). 0: none, as it was.
    pub open_edge_cost: f32,
    /// Where the reckoned body comes down off a ledge whose drop is an off-mesh walk-off ([`WalkOffLanding`]).
    pub walk_off_landing: WalkOffLanding,
    /// Where the stop forecast's margin falls off a ledge ([`MarginEdge`]).
    pub margin_edge: MarginEdge,
    /// Which floor a world's feet are put on ([`FixFloor`]).
    pub fix_floor: FixFloor,
    /// How a view's pitch unit relates to its elevation ([`PitchScale`]): the unit is 60.0 degrees up and 67.48 down
    /// (`pitch.rs`), not a radian (`radian`: the bots as they were, which aimed 3.5 degrees low at a target 15 to 30 degrees below the eye
    /// and shot 16 % there against 29 % with the map, `player_bots_weapons.md`). The world's pitch, the pitch turns the bots send and the geometry from the
    /// view's pitch (the eye, the throws, the rays) all follow it, so it is one setting for the whole match, not a team's.
    pub pitch_scale: PitchScale,
    /// [A304] How a ground step over a slope is reckoned ([`SlopeSpeed`]).
    pub slope_speed: SlopeSpeed,
    /// [A313] Which of the client's speed factor terms the reckoned body moves by ([`SpeedTerms`]).
    pub speed_terms: SpeedTerms,
    /// [A318] Whether the reckoned body's step is capped at the client's `max_speed` ([`SpeedCap`]).
    pub speed_cap: SpeedCap,
    /// [A318] Which worlds the body's speed level is read from ([`LevelRead`]).
    pub level_read: LevelRead,
    /// [A331] Whether the body's speed level is the one computed from its kit ([`LevelSource`]).
    pub level_source: LevelSource,
    /// [A331] Whether a battery carrier sprints ([`CarrierSprint`]).
    pub carrier_sprint: CarrierSprint,
    /// [A304] The flattest ramp face (degrees, the grid's ramp mark) [`SlopeSpeed::RampsGrade`] slows on: a floor whose mark is
    /// flatter counts as unmarked. 35 (the lab, 2026-11-05: Vostok Radar Station's and Chemical Plant's stairs carry 20-39 degree
    /// marks and the client climbs them at full speed; Cologne's slowing stairs are 45; 35 was better than 40 and 44). 20: every
    /// mark (`mapdata::grid::RAMP_MIN_DEG`).
    pub ramp_min_deg: f32,
    /// [A313] Whether the reckoned body walks a walk-off bridge (`mapdata::grid::controlled`'s off-mesh `GapJump` with no key, two
    /// cells on over a crack or a riser cell no floor covers, within a step) as it walks a Walk link: the step that lands in the
    /// cell between goes on to the bridge's far floor ([`crate::v026g::bots::body::Bridges::Walked`]). The client's capsule
    /// crosses the 0.25 m cell in one stride (the controller walks every such bridge the grid keeps); the reckoning moves cell by
    /// cell over walk links and stalled at the face of each one up (the pass's 324-pair bench: missed jumps 11 -> 29, at bridges
    /// up 0.45-0.75 m). ON with `grid.client_controller` (A313 bench4: no guard FAIL); OFF: as it was.
    pub walk_bridges: bool,
    /// [A318] Whether a jump pressed is reckoned as the client's ([`crate::v026g::bots::jump_arc`]): the jump clip's root rise and
    /// travel, the 1 m/s air control, the grid's floors for the capsule's walls and landings, the end at the clip's end. OFF: the
    /// snap onto a leap link the step crosses within [`crate::v026g::bots::body::JUMP_AIR_MS`] (`body::jumped_to`). Why: the
    /// drift lab's jump-key worlds (f1d92378, 605 = 0.451 points of the 4.03 %) are 59 % a floor apart after the jump (the model
    /// up a ledge the client missed, or the reverse) and the rest a distance in the air (the model walked on at the keys' speed).
    pub jump_arc: bool,
}

/// [A313] The terms of the client's movement speed factor `F = max(0, 1 + S4)` the reckoned body moves by
/// (`mapdata::controller::speed`, read from the 0.26g exe: skill 208, the weapon, the weight, each leg's wound, a hit's
/// stagger, a carried battery, the spring artefact). The body's level (`Kinematics::level_of`) holds what a world shows of the
/// rest; the terms here are the ones that change within a life faster than the level follows.
/// [A318] The cap on the reckoned body's flat step. The client's `set_physics_controller_walk_vector` 0x2D9060 caps the step it
/// hands the controller at `max_speed` 0x2DC010 (PROVEN, `mapdata::controller::speed::cap`): the sprint's 5.0 x F while the sprint
/// state is on (its `initialize` 0x336A10 sets the controller's +0x204, `finalize` 0x336A50 clears it, at once), else the walk's
/// 3.5 x F. The animation's blend toward the new keys goes on under it, so a released sprint moves at the walk cap on its own tick
/// while the blend still carries the sprint's speed (the client's recorded speeds: 3.62 m/s at the release tick, fx-smoke A325).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedCap {
    /// The blend's speed as it is (as it was).
    Off,
    /// Capped at the client's `max_speed` (`max(desired x F, minimal)`, the floor included) by the sprint state's keys
    /// ([`Kinematics::sprint_state`]) and the level; not in a jump (the cap is skipped while `+0x2C70C` is set, 0x2D915E, which
    /// `end_jump` 0x7F050 clears: LIKELY the jump's flag).
    #[default]
    Client,
}

impl SpeedCap {
    /// `local` (body frame) capped for a body moving by `keys` at `level`.
    pub fn capped(self, local: Local, keys: u32, level: f32) -> Local {
        use mapdata::controller::speed::{Gait, Heading, Stance, cap};
        if self == Self::Off {
            return local;
        }
        let gait = if Kinematics::sprint_state(keys) {
            Gait::Sprint
        } else {
            Gait::Move { stance: Stance::Stand, heading: Heading::Forward, aiming: false }
        };
        let cap = cap(gait, level);
        let len = local[0].hypot(local[1]);
        if len > cap { [local[0] * cap / len, local[1] * cap / len] } else { local }
    }
}

/// [A331] Where the reckoned body's speed level comes from. The client's factor is `F = max(0, 1 + S4)` (`apply_modifier`
/// 0x2D5AE0, enum 4; `max_speed` 0x2DC010), S4 summing the skills, the items' and modifications' `movement_speed_modifier`, the
/// carried weight's term (0x329B20, up to -0.2) and the weapon in hands (+-0.05); `survarium::v026g::bot_skills` computes it from a
/// match profile's kit (`profile_movement_candidates`), and fx-review's A332 check found it equal to the humans' measured level to
/// about 0.01 on 16 live matches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelSource {
    /// A settled world's speed level within 0.85..1.15 ([`Kinematics::level_of`]), else the profile's prior (as it was).
    Measured,
    /// The kit in use's computed F for each weapon it could have in hands ([`LEVEL_SNAP`]): a settled world's reading picks the
    /// nearest when within [`LEVEL_SNAP`] (the server has no reader of the weapon in hands), else stands (a battery's -0.05,
    /// stagger, Adrenalin: terms the kit does not give); before any reading, the primary's. A reading is taken within
    /// [`LEVEL_REACH`] of the kit's values instead of 0.85..1.15. A body with no profile: [`Self::Measured`].
    #[default]
    Computed,
}

/// [A331] Whether a battery carrier sprints. The owner: "battery cariers cant sprint"; `can_sprint` 0x2DBF40 and the jump
/// predicate 0x2FC230 refuse while the hands' item byte +0x388 != 0 (fx-review A332; that the take sets it for the battery was not
/// found: LIKELY by the exe). A carrier then walks at the run's 3.5 x F x 0.95 where a sprinter went 5.0 x F.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarrierSprint {
    /// As it was: a carrying bot may ask for a sprint, and a believed carrier's world keys sprint its reckoned body.
    Allowed,
    /// A carrying bot never asks for a sprint, and a believed carrier's world keys lose SPRINT (its body is run, its level read
    /// against the run).
    #[default]
    Refused,
}

/// [A331] How near a world's speed level must be to one of the kit's computed values to be taken as that value: the A332 check's
/// spread (93 of 103 player-kits within 0.02) with room for the loaded rounds spent.
pub const LEVEL_SNAP: f32 = 0.03;

/// [A331] How far outside the kit's computed values a world's speed level is still taken: the terms the kit does not give, a
/// battery's -0.05 (`victory_item_core::set_user` 0x300B00) and Adrenalin's +0.05 a rank (patch 94), with room.
pub const LEVEL_REACH: f32 = 0.1;

/// [A318] Which worlds the reckoned body's speed level ([`Kinematics::level_of`]) is read from. The client's ground speed is the
/// keys' desired speed times its movement factor once the animation's cross-fade toward them has settled (0.3 s, linear: fx-grid's
/// read of 0x3B0CD0 / 0x395360, PROVEN); in the fade it is between two keys' speeds. A world's speed is its body's move of the tick
/// before it (the world holds the body after that tick, `player_step` 0x7BDA0), so by that tick's keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelRead {
    /// Any world, by its own keys (as it was): a world in a fade gives a level off by the fade (a start's 0.85 passes).
    Any,
    /// A world whose last [`Kinematics::blend_ticks`] decided ticks before it held the same keys, read by those keys: the fade has
    /// settled and the speed is the keys' times the factor.
    #[default]
    Settled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedTerms {
    /// None: the level alone (as it was).
    #[default]
    None,
    /// Each leg's wound from the world's leg pools ([`super::motor::Hurt::legs_factor`]): -0.1 a leg under 20 % of its pool,
    /// -0.2 a leg at 0. ON and KEPT_RIGHT (the owner, 2026-11-05: correctness fixes go in; `None` moves a hurt body faster than
    /// the client does, so the all-off arm keeps it too). The drift lab (fx-bench, the server's body track fed each world's leg
    /// pools, 15 client-test recordings: drifting worlds 4.19 -> 4.14 %, every map better, the 911 worlds with a leg under 20 %
    /// 23.5 -> 18.9 %, median 0.45 -> 0.33 m). The battery and the stagger terms gave nothing there (4.1 % and 4.7 % against
    /// 3.6 %), the recorded speed worse.
    Legs,
}

/// [A304] How the reckoned body's ground step goes over a slope of the walk grid.
///
/// The client's capsule moves along its walker floor at the keys' speed, so over a ramp its ground speed is the speed times the
/// cosine of the grade along the walk: on Cologne's 45 degree stair ramps a sprint's 5.1 m/s is 3.6 m/s over the ground (fx-grid's
/// class of fx-bench's 73 slowed sprints, 2026-11-05: stairs and ramps, 51 %). On real stairs with treads the capsule steps up
/// (`cc_step_height`) at full speed. The reckoning moved at the keys' ground speed whatever the floor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlopeSpeed {
    /// The keys' ground speed on any floor (as it was).
    Planar,
    /// Every rising step is shortened to the cosine of its grade (the lab's first rule: real stairs too, which made Vostok Radar
    /// Station and Chemical Plant worse).
    Up,
    /// A rising step on a floor with a ramp mark at least [`Kinematics::ramp_min_deg`] steep (`grid.ramp_faces`,
    /// `WalkGrid::ramp_slope_deg`) is shortened to the cosine of its own grade: the rise along the walk, not the face's slope, as
    /// a walk across a ramp climbs little. Falling steps and unmarked floors keep their length (the client does not slow down a
    /// ramp, in the lab).
    #[default]
    RampsGrade,
}

impl SlopeSpeed {
    /// The share of a ground step of `run` metres that rises `rise` metres the body covers, on a floor whose ramp mark is
    /// `ramp_deg` (`None`: no mark, or one flatter than [`Kinematics::ramp_min_deg`]): 1 where the setting leaves the step as it
    /// is, else the grade's cosine.
    pub fn share(self, run: f32, rise: f32, ramp_deg: Option<f32>) -> f32 {
        let shortened = match self {
            Self::Planar => false,
            Self::Up => rise > 0.0,
            Self::RampsGrade => rise > 0.0 && ramp_deg.is_some(),
        };
        if shortened && run > 1e-4 { run / run.hypot(rise) } else { 1.0 }
    }
}

/// Which jump links the reckoned body takes up after a jump press, until a world shows where the body is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JumpSnap {
    /// Any jump link: the body is on its landing 0.8 s after the press whatever the rise, so a jump the clip
    /// cannot make is believed made until the next world.
    Any,
    /// Only a link whose rise the jump pressed could make: the plain jump's 1.09 m, or with forward and sprint held at
    /// the walk cap or more the sprint jump's 1.28 m (the client's choice by the keys and the speed at the press, PROVEN,
    /// the sprint-jump RE 1.1).
    #[default]
    ByKeys,
    /// None: the body stays on the take-off side until a world shows it on the landing floor (a landing counts only
    /// with walkable ground under the body near the target).
    Never,
}

/// Where the reckoned body (and the lip guard's forecast) falls off a ledge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    /// At the grid's cell boundary, 0.125 m past the last floor cell's centre (the lattice's picture, the bots' reckoning
    /// before the client edge).
    Lattice,
    /// As the client's capsule: held until [`super::body::SUPPORT_M`] past the centre of the floor it stands on, so a
    /// body that stays up at a ledge is not reckoned fallen (`body::slide_supported`, LIKELY, the live samples at lips).
    #[default]
    Client,
    /// [`Self::Client`], and over an open side of its floor (`WalkGrid::open_toward`: no link that way, no floor of its level
    /// beside, no wall) the body goes on as the client's capsule does: on while a floor of its level holds it within
    /// [`super::body::SUPPORT_M`], then straight down onto the floor under it (`body::walk_off_open`; `step_down` 0x7E1B0, PROVEN,
    /// the stand-in's `past_edge`). [`Self::Client`] holds it there as at a wall, since it moves a body past its floor only toward
    /// a drop link or along a lip: the reckoning stood while the body walked on, and the lip guard's stop forecast never fell, so
    /// the guard let keys walk the body off. A269: every entry into the Vector Laboratory middle pit (a 5.3 m edge, deeper than
    /// the harmless drop: no link) and into the Tarakanovsky Fort shut pocket at (-8, -20) (a walkway edge 1.7 m over it, no link)
    /// was over such an edge. The lip guard also watches the floors near an open side. Needs the grids' table of open sides
    /// ([`crate::v026g::bots::tuning::Tuning::needs_open_sides`]); a grid without it is [`Self::Client`]. Not the default: in
    /// A269's bench (160 pairs) falls off the route went -28 % (past open sides -46 %), but the guard changed the keys on 9.5 % of the
    /// ticks (0.5 % with [`Self::Client`]) in runs of 1-2 ticks, flipping 3 times a second, and stuck cases went +48 %, kills -13 %:
    /// it waits for a guard that holds its brake.
    ClientWalksOff,
}

/// [POCKETS (b)] Where the reckoned body ([`Edge::Client`]) comes down when it goes off a ledge whose drop is an off-mesh walk-off
/// (`GapDrop`, `gap::Takeoff::WalkOff`): a flight kept only where bodies at every one of `gap::WALK_OFF_SPEEDS` (3.0-5.1 m/s, PROVEN
/// J0 g2a) come down on its landing, which lies cells out from the edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkOffLanding {
    /// On the link's landing at any speed. A body that slips off slowly falls almost straight down, onto another floor: Vector
    /// Laboratory's 20.6 m lip, whose walk-off lands 2.5 m out on the 17.80 floor, put two fighting bots there at 0.03-0.12 m/s
    /// while their bodies fell onto the 16.22 ledge under the edge; believing they stood 2 m away, they walked off the ledge into
    /// the shut pit (`fx-smoke-pockets.md` §1, from their FALL lines in the bench's stand-in game: the walker mesh has a 0.97 m
    /// parapet along that edge and a plate over the ledge, so the client's body does neither, A247). Wrong by the client, so the
    /// all-off arm keeps the default (`KEPT_RIGHT`).
    Link,
    /// On the link's landing only at the slowest of `gap::WALK_OFF_SPEEDS` or more; slower, on the highest floor under the body
    /// (within its radius and half a cell, lower than a step) open above the edge (A247: no slab, plate or overhang between
    /// them, `WalkGrid::open_to`), else held at the edge until a world shows where it is. The default (A93's final sweep: the five A93 candidates on against the defaults, both teams, 576 pairs: stuck -5.2 %, kills +1.5 %, every guard level but A123's gap_failed_above, +45 % at paired z +1.8, in a few Vector Laboratory and Chemical Plant matches).
    #[default]
    BySpeed,
}

/// Where the stop forecast's margin (`body::stays_up`: the same stop from [`super::body::RECKONING_M`] further on along the body's
/// way, for a body not exactly where it is reckoned) falls off a ledge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarginEdge {
    /// At the grid's cell boundary ([`Edge::Lattice`]) whatever [`Kinematics::edge`] is: with [`Edge::Client`] the forecast moves
    /// the body up to [`super::body::SUPPORT_M`] past its floor's centre, past the cell's boundary, and the margin from there falls at
    /// once, whichever way the body goes. A body walking along a lip on its last floor is held by the full lip guard's sides
    /// (`Feel::lip_guard_full`: the same walk [`super::body::RECKONING_M`] to the side) every tick, and by its own margin once it is
    /// reckoned past the boundary: of the 3,705 ticks the guard held a walk in the bench's stuck episodes at lips, 2,031 (55 %) were
    /// held for a side only, the rest for the walk itself; with `edge = lattice` and no sides, 680, all for the walk (A205, 16 runs).
    Lattice,
    /// As the body itself ([`Kinematics::edge`]): the margin is the forecast's body a little further on, so it falls where that
    /// body would. The default (A93's final sweep: the five A93 candidates on against the defaults, both teams, 576 pairs: stuck -5.2 %, kills +1.5 %, every guard level but A123's gap_failed_above, +45 % at paired z +1.8, in a few Vector Laboratory and Chemical Plant matches).
    #[default]
    Body,
}

/// Which floor of the walk grid the reckoning puts a world's feet on (`body::BodyTrack::fixed`), the floor it moves the body on from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixFloor {
    /// The nearest floor from 1 m over the feet to 2.5 m under them (`WalkGrid::locate`): a world that shows the body in the air
    /// in front of a wall puts it on the floor over the wall's top when that is nearer than its own. School, the r1 stuck spot
    /// (A196, live): a jump against the facade, shown 0.68 m up, put the reckoned body on the top behind it, 0.59 m over the feet;
    /// with [`Edge::Client`] it walked on there for a second (2-3 m off) while the body fell back.
    Nearest,
    /// The nearest floor at most [`FIX_ABOVE_M`] over the feet, else the nearest: feet are on their floor or over it.
    Under,
    /// [`Self::Under`] while the body's own jump is in the air (its jump key held at a decided tick within
    /// [`super::body::JUMP_AIR_MS`] before the world's), else [`Self::Nearest`]. A world shows a body between floors in a jump
    /// (School r1: rising at the facade, its own floor under it) and in a fall (Tarakanovsky Fort (-54, 8, 12): walked off a
    /// ledge, 0.5 m under the top it left and over a lower top 0.6-0.8 m aside that it falls past); only the jump's floor is
    /// under it. Over every differing fix of the rev 38+ live recordings (14,097, A204), the mean error of the reckoning at the
    /// next world is 0.638 m with this rule, 1.380 with [`Self::Nearest`] and 0.944 with [`Self::Under`], and no map is worse
    /// than with [`Self::Nearest`] (Under alone was worse on four: Tarakanovsky Fort 1.644 against 0.691). The default: its by-side
    /// bench could not reach the case (47 such fixes in 256 matches: the stand-in's bodies stay on the grid's floors) and showed no
    /// cost (score +0.3 %, z +0.6, no guard flagged; #944).
    #[default]
    UnderInJump,
}

/// How far over a world's feet [`FixFloor::Under`] takes a floor, metres: a floor's height is its cell centre's, which on the
/// steepest walkable slope (55 degrees, `impassable_slope` 0x7DE10, PROVEN) lies up to 0.25 m over feet at the cell's edge (half
/// the diagonal of a 0.25 m cell, 0.18 m, times tan 55); a little over that.
pub const FIX_ABOVE_M: f32 = 0.3;

impl Default for Kinematics {
    fn default() -> Self {
        Self {
            walk: [3.5, 2.5, 3.0],
            aim: [3.0, 2.0, 2.5],
            crouch: [2.5, 2.0, 2.0],
            crouch_aim: [2.0, 1.5, 1.5],
            sprint: 5.0,
            blend_ticks: 6,
            mixer: Mixer::Layers,
            fix_blend: FixBlend::Replayed,
            wall_slide: WallSlide::Round,
            jump_snap: JumpSnap::ByKeys,
            edge: Edge::Client,
            drop_fall: DropFall::Client,
            open_edge_cost: 0.0,
            walk_off_landing: WalkOffLanding::BySpeed,
            margin_edge: MarginEdge::Body,
            fix_floor: FixFloor::UnderInJump,
            pitch_scale: PitchScale::Client,
            slope_speed: SlopeSpeed::RampsGrade,
            ramp_min_deg: 35.0,
            walk_bridges: true,
            jump_arc: false,
            speed_terms: SpeedTerms::Legs,
            speed_cap: SpeedCap::Client,
            level_read: LevelRead::Settled,
            level_source: LevelSource::Computed,
            carrier_sprint: CarrierSprint::Refused,
        }
    }
}

/// The ground direction a body faces at `yaw` (map frame, x and z).
pub fn forward(yaw: f32) -> [f32; 2] {
    let (s, c) = yaw.sin_cos();
    [-s, c]
}

/// The yaw that faces the ground direction `(x, z)`; the inverse of
/// [`forward`], in `(-π, π]`.
pub fn yaw_of(direction: [f32; 2]) -> f32 {
    (-direction[0]).atan2(direction[1])
}

/// `angle` brought into `(-π, π]`.
pub fn wrap(angle: f32) -> f32 {
    let a = (angle + PI).rem_euclid(TAU) - PI;
    if a <= -PI { a + TAU } else { a }
}

/// A velocity in the body's frame: (ahead, to the left), m/s.
pub type Local = [f32; 2];

/// The world velocity of `local` for a body at `yaw`.
pub fn to_world(local: Local, yaw: f32) -> [f32; 2] {
    let [fx, fz] = forward(yaw);
    // Left of facing: a quarter turn to the positive side.
    let [lx, lz] = [-fz, fx];
    [local[0] * fx + local[1] * lx, local[0] * fz + local[1] * lz]
}

/// [A318] How a blend goes on when the keys change again before it is over.
///
/// The client moves by its animations' root motion: each change of the movement keys brings in a new movement lexeme whose
/// weight a `linear_interpolator` takes from 0 to 1 over 0.3 s (KINEMATICS-CODE.md row 3, PROVEN for one change: the speed
/// k ticks after a single change of keys is the linear blend's at every k, 15 client-test recordings).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mixer {
    /// A change starts one new blend from the velocity of that tick, the blend in progress frozen there (as it was).
    Frozen,
    /// The client's lexeme stack: a change lays a new layer over the others with its own 0.3 s ramp; the layers under it keep
    /// their own ramps, each weighted by what the ramps above it leave; a layer at full weight hides all under it. ON and
    /// KEPT_RIGHT: against the client's speed at the worlds of 15 client-test recordings (bots' walk and aim keys), with 2, 3
    /// and 4+ changes in the last 0.3 s the share off by more than 0.5 m/s falls 31.7 / 34.7 / 41.9 % -> 8.4 / 9.8 / 12.3 %
    /// (median 0.27 -> 0.06 m/s); with none it is the same (6.4 %). Bots change their walk keys every ~7 ticks, so the
    /// blends are interrupted all the time (`scripts/kin_mixer_fit.py`, fx-zones).
    #[default]
    Layers,
}

/// [A318] Along what a step of the reckoned body the walk grid stops goes on (`body::slide_supported_by`).
///
/// The client's capsule slides along what it meets: the walk is projected on the wall (`get_wall_slide_vector`, the controller tick
/// ported in A313). The grid's cells see a wall as a row of cells, along an axis or as a 45 degree staircase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallSlide {
    /// The step's x part, else its z part (as it was): a wall along an axis.
    Axes,
    /// Those, then the step along each 45 degree diagonal at its projection (cos 45 of it): a wall the grid's cells draw as a 45
    /// degree staircase, which holds both axes.
    Diagonal,
    /// [A318] Those, and the step turned by 22.5 and 67.5 degrees either way at its projection: the capsule's slide along a
    /// corner or a thin post, whose contact normal is at any angle (level_01's post at (60.7, 33.2) and level_08's pillar corner
    /// at (68.55, 25.51) held the reckoning where the client slid round them). The default: the drift lab (15 client-test
    /// recordings) drifts 3.31 % of the worlds against 3.54 % with [`Self::Diagonal`], every map and recording better.
    #[default]
    Round,
}

/// [A318] How the reckoned body (`BodyTrack`, not the lip guard's stop forecast) comes down when its floor drops more than a step
/// under it (a drop link's landing, a walk-off under the edge, a world in the air over its floor).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropFall {
    /// On the floor below at once (as it was).
    Snap,
    /// As the client's capsule: its height falls from where it left under gravity (`step_down` 0x7E1B0, PROVEN: with no ground
    /// within the step under it, the velocity takes `g * min(dt, tick)` down each step and moves it by the speed it had, g = 9.8,
    /// `fall_params.gravity`) until it is on the floor below. Its floor and its way across are as with [`Self::Snap`].
    Gravity,
    /// [A318] [`Self::Gravity`] with the client's slow start (`step_down` 0x7E1B0, PROVEN by fx-gear-2's read and probe; port
    /// `mapdata::controller::tick`, `player_step` reads the speed at the tick's start, then `step_down`): while the speed down at
    /// the tick's start is under `on_ground_max_vertical_speed` (2.0 m/s, `startup.cfg`) the body goes down by
    /// `additional_down_speed` (0.05 m a tick, `startup.cfg`), else by the speed at the tick's start (`(v + a dt).y * tick`, `a`
    /// the slope's slide: none in the air); the speed takes `g * tick` after the move either way. From rest: five ticks of 0.05 m,
    /// then 2.45 m/s, 2.94 m/s, ...
    #[default]
    Client,
}

/// [A318] The blend a world's fix starts the reckoning with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixBlend {
    /// One layer: the way of the keys before the world at the world's speed (as it was). The client is mid-blend at most worlds
    /// (bots change their keys every ~7 ticks), so its mix and way are lost.
    Measured,
    /// The client's own: the keys of the bot's last ticks before the world played through the blend from the keys in force the
    /// blend's ticks before (an older layer is hidden by then, so the stack is exact), at the body's level.
    #[default]
    Replayed,
}

/// The most layers a blend holds: a change a tick at most, and a layer at full weight after the blend's ticks hides all under
/// it, so a 6-tick blend holds 7 at most.
const LAYERS: usize = 8;

/// One layer of a [`Blend`]: its velocity and the ticks since it was laid on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Layer {
    local: Local,
    step: u32,
}

/// The blend to a new velocity, in the body's frame: the layers, the first at full weight under the others (see [`Mixer`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blend {
    layers: [Layer; LAYERS],
    len: usize,
}

impl Default for Blend {
    fn default() -> Self {
        Self::at([0.0, 0.0])
    }
}

impl Blend {
    /// The velocity now: each layer from the top takes its ramp's share of what the layers above leave.
    pub fn now(&self, ticks: u32) -> Local {
        // Two layers: the blend from the first to the second, in the arithmetic the frozen blend always had (its numbers stay
        // bit for bit what they were).
        if self.len == 2 {
            let (from, to) = (self.layers[0].local, self.layers[1].local);
            let t = if ticks == 0 { 1.0 } else { (self.layers[1].step as f32 / ticks as f32).min(1.0) };
            return [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t];
        }
        let mut v = [0.0f32; 2];
        let mut rest = 1.0f32;
        for (i, layer) in self.layers[..self.len].iter().enumerate().rev() {
            let ramp = if i == 0 || ticks == 0 { 1.0 } else { (layer.step as f32 / ticks as f32).min(1.0) };
            let w = rest * ramp;
            v[0] += w * layer.local[0];
            v[1] += w * layer.local[1];
            rest -= w;
            if rest <= 0.0 {
                break;
            }
        }
        v
    }

    /// One tick on toward `target` with the [`Mixer::Frozen`] rule.
    pub fn tick(&mut self, target: Local, ticks: u32) -> Local {
        self.tick_mixed(target, ticks, Mixer::Frozen)
    }

    /// One tick on toward `target`: a new target lays a new layer on, by `mixer`'s rule.
    pub fn tick_mixed(&mut self, target: Local, ticks: u32, mixer: Mixer) -> Local {
        if target != self.layers[self.len - 1].local {
            match mixer {
                Mixer::Frozen => *self = Self::at(self.now(ticks)),
                Mixer::Layers if self.len == LAYERS => *self = Self::at(self.now(ticks)),
                Mixer::Layers => {}
            }
            self.layers[self.len] = Layer { local: target, step: 0 };
            self.len += 1;
        }
        for layer in &mut self.layers[..self.len] {
            layer.step = layer.step.saturating_add(1);
        }
        // A layer at full weight hides all the layers under it (the frozen blend keeps its two, as it always did).
        if mixer == Mixer::Frozen {
            return self.now(ticks);
        }
        if let Some(top) = (0..self.len).rev().find(|&i| ticks == 0 || self.layers[i].step >= ticks) {
            self.layers.copy_within(top..self.len, 0);
            self.len -= top;
            self.layers[self.len..].fill(Layer::default());
        }
        self.now(ticks)
    }

    /// Already at `local`, no blend going.
    pub fn at(local: Local) -> Self {
        let mut layers = [Layer::default(); LAYERS];
        layers[0] = Layer { local, step: u32::MAX };
        Self { layers, len: 1 }
    }
}

impl Kinematics {
    /// Whether the keys sprint (SPRINT and FORWARD, none of BACK, FIRE,
    /// CROUCH, AIM).
    pub fn sprints(keys: u32) -> bool {
        keys & key::SPRINT != 0
            && keys & key::FORWARD != 0
            && keys & (key::BACK | key::FIRE | key::CROUCH | key::AIM) == 0
    }

    /// [A318] Whether the keys hold the client's sprint state: [`Self::sprints`]. The start predicates (`sprint_predicate` 0x2FBC90,
    /// `is_trying_to_sprint` 0x2FC1F0) test SPRINT and FORWARD with none of `0x122` (BACK, FIRE, CROUCH); AIM ends the state through
    /// its own exits (PROVEN, static, fx-review A328 on the movement FSM built in 0x2FAB70): 0x2FBD50 (to stand_run) and 0x2FBCE0 (to
    /// stand_on_site) leave when `sprint_predicate` fails or AIM (keys +0x330 bit 0x80, 0x2FBD65 / 0x2FBCF5) is held with the hands'
    /// item byte +0x389 != 2 (which items are 2 is not read). The live game agrees: humans holding SPRINT + FORWARD + AIM move at a
    /// median 3.69 m/s (89 worlds), 5.59 without AIM. The state ends on the release tick (fx-smoke: the client's flag, 2,524 worlds).
    pub fn sprint_state(keys: u32) -> bool {
        Self::sprints(keys)
    }

    /// The velocity the keys ask for, in the body's frame, for a body at
    /// `level` × the desired speeds.
    pub fn target_local(&self, keys: u32, level: f32) -> Local {
        let held = |bit| f32::from(u8::from(keys & bit != 0));
        let ahead = held(key::FORWARD) - held(key::BACK);
        let left = held(key::LEFT) - held(key::RIGHT);
        if ahead == 0.0 && left == 0.0 {
            return [0.0, 0.0];
        }
        let class = match (keys & key::CROUCH != 0, keys & key::AIM != 0) {
            (false, false) => self.walk,
            (false, true) => self.aim,
            (true, false) => self.crouch,
            (true, true) => self.crouch_aim,
        };
        let speed = if Self::sprints(keys) {
            self.sprint
        } else if ahead > 0.0 {
            class[0]
        } else if ahead < 0.0 {
            class[2]
        } else {
            class[1]
        };
        let n = ahead.hypot(left);
        [ahead / n * speed * level, left / n * speed * level]
    }

    /// The world velocity the keys ask for at `yaw`.
    pub fn target_velocity(&self, keys: u32, yaw: f32, level: f32) -> [f32; 2] {
        to_world(self.target_local(keys, level), yaw)
    }

    /// The body's speed level from a world: its speed over the desired one
    /// of its keys, if it moved steadily under keys that have one.
    pub fn level_of(&self, keys: u32, speed: f32) -> Option<f32> {
        self.level_raw(keys, speed).filter(|level| (0.85..=1.15).contains(level))
    }

    /// [A331] A world's speed over the desired one of its keys, with no band: none for keys with no desired speed.
    pub fn level_raw(&self, keys: u32, speed: f32) -> Option<f32> {
        let [a, l] = self.target_local(keys, 1.0);
        let nominal = a.hypot(l);
        (nominal >= 0.5).then(|| speed / nominal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_and_direction_are_inverse_and_angles_wrap() {
        for yaw in [-3.0_f32, -1.0, 0.0, 0.5, 2.0, 3.1] {
            assert!((yaw_of(forward(yaw)) - yaw).abs() < 1e-5, "{yaw}");
        }
        assert!((wrap(3.0 * PI) - PI).abs() < 1e-5);
        assert!((wrap(-PI) - PI).abs() < 1e-5);
        assert!((wrap(0.25 - TAU) - 0.25).abs() < 1e-5);
        let (a, b) = (forward(0.4), forward(0.4 + 8.0 * PI));
        assert!((a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4);
    }

    #[test]
    fn keys_give_the_desired_speeds_of_the_code() {
        let k = Kinematics::default();
        let speed = |keys| {
            let [a, l] = k.target_local(keys, 1.0);
            a.hypot(l)
        };
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(close(speed(key::FORWARD), 3.5));
        assert!(close(speed(key::LEFT), 2.5));
        assert!(close(speed(key::BACK), 3.0));
        assert!(
            close(speed(key::FORWARD | key::LEFT), 3.5),
            "forward diagonals"
        );
        assert!(close(speed(key::BACK | key::RIGHT), 3.0), "back diagonals");
        assert!(close(speed(key::FORWARD | key::SPRINT), 5.0));
        assert!(
            close(speed(key::FORWARD | key::RIGHT | key::SPRINT), 5.0),
            "a diagonal sprints"
        );
        assert!(
            close(speed(key::FORWARD | key::SPRINT | key::FIRE), 3.5),
            "no sprint firing"
        );
        assert!(close(speed(key::FORWARD | key::CROUCH | key::SPRINT), 2.5));
        assert!(close(speed(key::FORWARD | key::AIM), 3.0));
        assert!(close(speed(key::BACK | key::CROUCH | key::AIM), 1.5));
        assert_eq!(speed(key::FORWARD | key::BACK), 0.0);
        assert_eq!(speed(key::LEFT | key::RIGHT), 0.0);
        // At yaw 0 forward is +z, LEFT is -x (a positive yaw turns left).
        let at = |keys| k.target_velocity(keys, 0.0, 1.0);
        assert!(close(at(key::FORWARD)[1], 3.5));
        assert!(close(at(key::LEFT)[0], -2.5));
        assert!(close(at(key::RIGHT)[0], 2.5));
        assert!(close(k.target_velocity(key::FORWARD, 0.0, 0.95)[1], 3.325));
    }

    #[test]
    fn a_step_up_a_marked_ramp_covers_its_grades_cosine_and_any_other_step_all_of_it() {
        let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
        let cos45 = std::f32::consts::FRAC_1_SQRT_2;
        // A 45 degree ramp with its mark: a step of 0.25 m that rises 0.25 m covers cos 45 of the run.
        let up = SlopeSpeed::RampsGrade.share(0.25, 0.25, Some(45.0));
        assert!(close(up, cos45), "{up}");
        // The sprint's ground speed there: 5.1 x 0.707 = 3.6 m/s, the client's on Cologne's stairs.
        assert!(close(5.1 * up, 3.606), "{}", 5.1 * up);
        // Across a ramp: the step's own grade, not the face's.
        assert!(close(SlopeSpeed::RampsGrade.share(0.25, 0.1, Some(45.0)), 0.25 / 0.25f32.hypot(0.1)));
        assert_eq!(SlopeSpeed::RampsGrade.share(0.25, 0.25, None), 1.0, "no mark (real stairs): full speed");
        assert_eq!(SlopeSpeed::RampsGrade.share(0.25, -0.25, Some(45.0)), 1.0, "down a ramp: all of it");
        assert_eq!(SlopeSpeed::RampsGrade.share(0.25, 0.0, Some(45.0)), 1.0, "a flat step on a marked floor");
        assert_eq!(SlopeSpeed::RampsGrade.share(0.0, 0.3, Some(45.0)), 1.0, "no run: nothing to shorten");
        assert!(close(SlopeSpeed::Up.share(0.25, 0.25, None), cos45), "up: any rising step, mark or none");
        assert_eq!(SlopeSpeed::Up.share(0.25, -0.25, None), 1.0);
        assert_eq!(SlopeSpeed::Planar.share(0.25, 0.25, Some(45.0)), 1.0, "planar: as it was");
        // The defaults: the ramp rule, from 35 degree marks.
        let kin = Kinematics::default();
        assert_eq!((kin.slope_speed, kin.ramp_min_deg), (SlopeSpeed::RampsGrade, 35.0));
    }

    #[test]
    fn a_new_velocity_is_blended_in_over_six_ticks() {
        let k = Kinematics::default();
        let mut blend = Blend::default();
        let target = k.target_local(key::FORWARD, 1.0);
        let speeds: Vec<f32> = (0..8)
            .map(|_| blend.tick(target, k.blend_ticks)[0])
            .collect();
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(
            close(speeds[0], 3.5 / 6.0) && close(speeds[5], 3.5) && close(speeds[7], 3.5),
            "{speeds:?}"
        );
        // Two ticks into a stop, a sprint: blended from where it was.
        blend.tick([0.0, 0.0], 6);
        blend.tick([0.0, 0.0], 6);
        let mid = blend.now(6)[0];
        assert!(close(mid, 3.5 * 4.0 / 6.0));
        let next = blend.tick([5.0, 0.0], 6)[0];
        assert!(close(next, mid + (5.0 - mid) / 6.0));
        assert!((k.level_of(key::FORWARD, 3.675).unwrap() - 1.05).abs() < 1e-4);
        assert_eq!(k.level_of(key::FORWARD, 1.0), None);
    }

    /// [A318] From rest, forward for two ticks, then left: with [`Mixer::Layers`] the forward layer keeps its own ramp under the
    /// left one (the client's lexeme stack), with [`Mixer::Frozen`] it is frozen at the change; both reach the left velocity
    /// when its ramp is full, and a single change is the same blend either way.
    #[test]
    fn an_interrupted_blend_keeps_the_layers_under_it_ramping() {
        let close = |a: Local, b: Local| (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4;
        let (fwd, left) = ([3.5, 0.0], [0.0, 2.5]);
        let run = |mixer: Mixer| {
            let mut blend = Blend::default();
            let mut out = Vec::new();
            for target in [fwd, fwd, left, left, left, left, left, left, left] {
                out.push(blend.tick_mixed(target, 6, mixer));
            }
            out
        };
        let layers = run(Mixer::Layers);
        let frozen = run(Mixer::Frozen);
        assert!(close(layers[1], [3.5 * 2.0 / 6.0, 0.0]) && close(frozen[1], layers[1]));
        assert!(close(layers[2], [3.5 * 5.0 / 12.0, 2.5 / 6.0]), "{:?}", layers[2]);
        assert!(close(layers[3], [3.5 * 16.0 / 36.0, 2.5 * 2.0 / 6.0]), "{:?}", layers[3]);
        assert!(close(frozen[2], [3.5 * 2.0 / 6.0 * 5.0 / 6.0, 2.5 / 6.0]), "{:?}", frozen[2]);
        assert!(close(layers[7], left) && close(frozen[7], left));
        assert!(close(layers[8], left));
        // `tick` is the frozen rule.
        let mut a = Blend::default();
        let mut b = Blend::default();
        for target in [fwd, fwd, left, left] {
            assert!(close(a.tick(target, 6), b.tick_mixed(target, 6, Mixer::Frozen)));
        }
        assert_eq!(Kinematics::default().mixer, Mixer::Layers);
    }
}
