//! What the runner sees of the match at one poll: the server's own view, parsed once from the bots' [`MatchView`] and the
//! bots themselves ([`super::hook`]).

use mapdata::Vec3;
use survarium::v026g::loadouts::BotRole;

use super::step::{Group, Side, Size};
use super::time::At;
use crate::v026g::bots::pools::Pools;
use crate::v026g::bots::reflex::{Gates, LegsWhy};
use crate::v026g::bots::view::{BatteryState, Place};

/// A body in the match, as the world last showed it.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub place: Place,
    pub team: u8,
    pub alive: bool,
    pub feet: Vec3,
    /// The body pool, [0, 1].
    pub health: f32,
    pub kills: u32,
    pub deaths: u32,
    pub crouched: bool,
    /// The keys it last pressed (as the world and its inputs show them), its view and its speed over the ground, m/s.
    pub keys: u32,
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
}

/// The human of the match.
#[derive(Clone, Debug, PartialEq)]
pub struct Owner {
    pub body: Body,
    /// His health pools as the last world showed them (body, head, legs, hands, pain, infection); none until one has.
    pub pools: Option<Pools>,
}

/// What a bot's reflex is doing: its target, the gates that held its fire at its last tick, and why it took its legs, if it did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mind {
    pub target: Option<Place>,
    pub gates: Gates,
    pub legs: Option<LegsWhy>,
}

/// A server bot in the match.
#[derive(Clone, Debug, PartialEq)]
pub struct BotView {
    pub body: Body,
    pub mind: Mind,
    /// The role the server gave the bot (its gear's); none for a bot of no role.
    pub role: Option<BotRole>,
}

/// A line of the server's log about a bot (a [`crate::v026g::bots::Note`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub place: Place,
    pub time: At,
    pub text: String,
}

/// The map's line of sight, for a step to ask whether a point sees another (the bots' own test, a clear line on the bullet mesh).
#[derive(Clone)]
pub struct Sight(pub std::sync::Arc<mapdata::RayTree>);

impl std::fmt::Debug for Sight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sight")
    }
}

impl PartialEq for Sight {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

/// One poll.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub clock: At,
    /// Play has begun (not the start countdown).
    pub playing: bool,
    /// The human in the match; none until he has entered. The runner is for one human: the first.
    pub owner: Option<Owner>,
    /// The server's bots that are in the match, by place.
    pub bots: Vec<BotView>,
    /// Where each battery is.
    pub batteries: Vec<BatteryState>,
    /// What the bots said in this poll.
    pub heard: Vec<Heard>,
    /// The map's line of sight, once the map is built.
    pub sight: Option<Sight>,
    /// The exits of each team's base (`Tactics::exits`: points 30-60 m out on the ways from its spawns toward the other side's), by
    /// team; empty until the map is built.
    pub exits: [Vec<Vec3>; 2],
}

impl Frame {
    /// The bots of `side`, by place.
    pub fn side(&self, side: Side) -> impl Iterator<Item = &BotView> {
        let owner_team = self.owner.as_ref().map(|o| o.body.team);
        self.bots.iter().filter(move |b| match (side, owner_team) {
            (_, None) => false,
            (Side::Mates, Some(team)) => b.body.team == team,
            (Side::Enemies, Some(team)) => b.body.team != team,
        })
    }

    /// Whether nothing lies between `from` and `to` on the bullet mesh; none until the map is built.
    pub fn clear_line(&self, from: Vec3, to: Vec3) -> Option<bool> {
        self.sight.as_ref().map(|sight| sight.0.visible(from, to))
    }

    pub fn bot(&self, place: Place) -> Option<&BotView> {
        self.bots.iter().find(|b| b.body.place == place)
    }
}

/// The group a match belongs to: its map, mode and the places of its larger team.
pub fn group_of(level: u8, mode: u8, teams: &[u8]) -> Group {
    let count = |team: u8| teams.iter().filter(|&&t| t == team).count();
    let per_team = count(0).max(count(1)).min(usize::from(u8::MAX)) as u8;
    Group {
        level,
        mode,
        size: Size::PerTeam(per_team),
    }
}
