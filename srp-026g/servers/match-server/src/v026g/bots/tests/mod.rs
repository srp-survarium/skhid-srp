//! Soak runs (`player_bots_research.md` B8 point 5): whole matches of bots
//! against a stand-in for the game. Only a real client simulates a match,
//! so these runs stand in for it with a small game of their own:
//! - bodies move by the bots' inputs at their ticks with the movement
//!   model (`kin`), kept on the walk grid;
//! - a shot hits the first enemy whose chest lies within 0.45 m of the
//!   view's ray, with a clear line on the bullet mesh; what it takes off is
//!   the weapon's damage against the target's kit ([`standin_damage`]);
//! - the weapon each bot holds fires as the client fires it: an automatic
//!   one at its rate while the trigger is held, any other once a press;
//!   and kicks as the client kicks ([`super::recoil`]): the shots lift the
//!   view itself (the base pitch and the kick), a downward turn is taken
//!   off the kick first, the kick comes back by the weapon's controller;
//!   each shot's kick drawn in the config's range (the bots predict the
//!   middle);
//! - the dead respawn at their side's spawns after 8 s in the battery
//!   modes and 6.4 s in Team Deathmatch (the radio's -20 % on 10 s and
//!   8 s, [`RESPAWN_BATTERIES_MS`]);
//! - a use takes a battery, puts it in at one's own container, or steals
//!   from the other's, as the client's use does: only what the view's ray
//!   from the eye meets within 2 m (`base_player::detect_usable_object`):
//!   within 0.2 m of a battery's middle, 0.6 m of a use box's; the drop
//!   key held, or a new fire press, drops it, and the weapon is up again
//!   its `show_time` later (`Gun::show_ms`); a carrier cannot sprint;
//! - Team Deathmatch: each death costs the team a ticket; a team out of
//!   tickets no longer respawns, and loses when its last body dies;
//! - a world of every body, the statistics and the batteries comes every
//!   second, 300 ms behind the clock, as the match gives the bots.
//!
//! The stand-in is much simpler than the game (no spread, no side kick, no
//! stamina; a jump that follows the client's jump clips from the
//! keys held at its press, and a step no higher than the client's
//! ([`Flight`], [`honest_stand_in`]; `SRP_BENCH_STAND_IN=old`: the old
//! lift up any floor in reach, [`stand_in_jump`]); none with a broken leg, a
//! hit's stagger as one slow ([`STAND_IN_LEG_SHARE`], [`STAND_IN_STAGGER`]);
//! the rounds are not counted, the bots count their own): the runs check
//! that bots of two teams find each other, fight, die, come back, take
//! and bring batteries and end a match, with no input out of the rules;
//! not how well.
//! `SRP_BOT_NOTES=1` prints what each bot did.

use std::collections::VecDeque;
use std::sync::Arc;

use mapdata::{BatterySpawner, Container, NodeId, Vec3, World};
use survarium::v026g::match_msgs::TICK_MS;

use super::body::{self, Fix, jumped_to, slide, slide_capped, slide_supported};
use super::guns::{Arms, Gun};
use super::hazards::{Hazard, Hazards};
use super::kin::{Blend, Kinematics, TICK_S, forward, to_world};
use super::recoil::{Cadence, Recoil, kick_of};
use super::rng::Rng;
use super::trace::Trace;
use super::tuning::Tuning;
use super::view::{BatteryState, Clock, WorldFacts};
use super::*;
use crate::v026g::drive::{half_value, key};

mod arrival_height;
mod batteries;
mod bench;
mod bench_spread;
mod blind_walk;
mod camper;
mod door_walk;
mod driving;
mod eye;
mod false_walls;
mod fights;
mod fix_gap;
mod hump_deck;
#[cfg(feature = "jump-test")]
mod jump_pool;
mod kits;
mod live_zones;
mod neck;
mod no_way;
mod opening;
mod pace_gate;
mod pocket_runs;
mod settings;
mod sight;
mod soak;
mod soaks;
mod sorted;
mod sorted_use;
mod standin;
mod standin_aim;
mod standin_body;
mod standin_capsule;
mod standin_climb;
mod standin_damage;
mod standin_fields;
mod standin_grenade;
mod standin_heal;
mod standin_hitbox;
mod standin_jump;
mod standin_level;
mod standin_slide;
mod standin_smoke;
mod standin_stagger;
mod standin_stamina;
mod standin_trap_habit;
mod standin_traps;
mod standin_watch;
mod standin_weapon;
mod stuck_causes;
mod tallies;
mod team_kit;
mod team_tuning;
mod trap_react;
mod traps;
mod unclear_aim;
mod walkway;
mod weapon_class;
mod weapon_data;
mod work_by_team;
mod world_facts;
mod zone_runs;

use tallies::*;
use tallies::{fight_tally, owner};
use bench::*;
pub(crate) use camper::*;
mod camper_move;
pub(crate) use camper_move::{Arrival, BenchCamper, CamperAt, CamperStart, MoveOptions};
pub(crate) use standin_aim::human_tuning;
pub(crate) use team_kit::scope as team_kit_scope;
pub(crate) use standin_watch::{AT_REST_MPS, Watch, lane_look};
mod view_error;
pub(crate) use view_error::ViewError;
use settings::*;
pub(crate) use settings::EnemyLanes;
use soak::*;
use standin::*;
use standin_capsule::stand_in_corner;
use standin_hitbox::HitVolume;
use standin_jump::*;


/// A body at `x` on the line z = 0, alive or not, in a world at `time`.
fn standing(time: u32, x: f32, alive: bool) -> Fix {
    Fix {
        time,
        alive,
        inserted: 100,
        feet: alive.then(|| Vec3::new(x, 0.0, 0.0)),
        yaw: 0.0,
        pitch: 0.0,
        keys: 0,
        speed: Some(0.0),
    }
}

/// The bench (`bench::real_bot_cost`). It stays a test of this module so its path,
/// `v026g::bots::tests::real_bot_cost`, is the one every bench script runs.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn real_bot_cost() {
    bench::real_bot_cost();
}
