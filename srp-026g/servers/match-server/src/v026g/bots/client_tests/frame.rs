//! What the runner sees of the match at one poll: the server's own view, parsed once from the bots' [`MatchView`].

use mapdata::Vec3;

use super::step::{Group, Side, Size};
use crate::v026g::bots::view::{MatchView, Place};
use crate::v026g::bots::Note;

/// A body in the match, as the world last showed it.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub place: Place,
    pub team: u8,
    pub alive: bool,
    pub feet: Vec3,
    /// [0, 1].
    pub health: f32,
    pub kills: u32,
    pub deaths: u32,
    pub crouched: bool,
}

/// A line of the server's log about a bot (a [`Note`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub place: Place,
    pub time: u32,
    pub text: String,
}

/// One poll.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The match clock, ms.
    pub clock: u32,
    /// Play has begun (not the start countdown).
    pub playing: bool,
    /// The human in the match; none until he has entered. The runner is for one human: the first.
    pub owner: Option<Body>,
    /// The server's bots that are in the match, by place.
    pub bots: Vec<Body>,
    /// What the bots said in this poll.
    pub heard: Vec<Heard>,
}

impl Frame {
    pub fn of_view(view: &MatchView, playing: bool, notes: &[Note]) -> Self {
        let body = |p: &crate::v026g::bots::view::PlaceView| Body {
            place: p.place,
            team: p.team,
            alive: p.alive,
            feet: p.feet,
            health: p.health,
            kills: p.kills,
            deaths: p.deaths,
            crouched: p.crouched(),
        };
        let owner = view.places.iter().find(|p| p.present && !p.bot).map(body);
        let bots = view.places.iter().filter(|p| p.present && p.bot).map(body).collect();
        let heard = notes
            .iter()
            .map(|n| Heard {
                place: n.place,
                time: n.time,
                text: n.text.clone(),
            })
            .collect();
        Self {
            clock: view.clock,
            playing,
            owner,
            bots,
            heard,
        }
    }

    /// The bots of `side`, by place.
    pub fn side(&self, side: Side) -> impl Iterator<Item = &Body> {
        let owner_team = self.owner.as_ref().map(|o| o.team);
        self.bots.iter().filter(move |b| match (side, owner_team) {
            (_, None) => false,
            (Side::Mates, Some(team)) => b.team == team,
            (Side::Enemies, Some(team)) => b.team != team,
        })
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
