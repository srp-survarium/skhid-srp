//! What the runner sees of the match at one poll: the server's own view, parsed once from the bots' [`MatchView`] and the
//! bots themselves ([`super::hook`]).

use mapdata::Vec3;

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
}

/// A line of the server's log about a bot (a [`crate::v026g::bots::Note`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub place: Place,
    pub time: At,
    pub text: String,
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
