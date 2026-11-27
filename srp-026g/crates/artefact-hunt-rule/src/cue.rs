//! What a viewer is told: the hud's messages and the announcer's lines.
//!
//! They are no part of a world. The game's hud compares the rule it showed
//! last with the rule of the frame ([`Cue::between`]), so a tick played
//! again says nothing twice and a game that joins late hears nothing old.

use crate::roster::{Place, Team};
use crate::rule::{Artefact, Outcome, Reason, Round, Rule, Spot, Stage};

/// Something that happened to the artefact or the match between two looks
/// at the rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Spawned,
    Taken {
        by: Place,
        team: Team,
    },
    Dropped {
        team: Team,
    },
    Delivered {
        team: Team,
    },
    /// Removed: nobody took it in its time.
    Destroyed,
    Ended {
        outcome: Outcome,
        reason: Reason,
    },
}

/// Who looks: the place the camera is on (none: a free camera) and its team.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewer {
    pub place: Option<Place>,
    pub team: Option<Team>,
}

/// The announcer's seven lines, in 0.46c2's order (its name table 0xB2E540).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    Spawned = 0,
    PickedUp = 1,
    PickedUpByEnemy = 2,
    Dropped = 3,
    DroppedByEnemy = 4,
    Brought = 5,
    BroughtByEnemy = 6,
}

/// The hud's texts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    /// "Artifact has spawned"
    Spawned,
    /// "You picked the artifact"
    YouPicked,
    /// "Your team have delivered the artifact"
    TeamDelivered,
    /// "The opponent team delivered the artifact"
    EnemyDelivered,
    /// "Artifact destroyed"
    Destroyed,
    Won(Reason),
    Lost(Reason),
    Draw,
}

/// The artefact at a look: none while one is waited for and outside play.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Seen {
    None,
    Lying(Spot),
    Carried(Place),
}

fn artefact(rule: &Rule) -> Seen {
    match rule.stage() {
        Stage::Play(play) => match play.round() {
            Round::Hunt {
                artefact: Artefact::Lying { spot, .. },
                ..
            } => Seen::Lying(*spot),
            Round::Hunt {
                artefact: Artefact::Carried { by, .. },
                ..
            } => Seen::Carried(*by),
            _ => Seen::None,
        },
        _ => Seen::None,
    }
}

impl Cue {
    /// What happened between the rule last shown and the rule now: the
    /// artefact's news, then the match's end. Two looks more than one
    /// transition apart tell the last that can be told.
    pub fn between(seen: &Rule, now: &Rule) -> [Option<Cue>; 2] {
        let was = artefact(seen);
        // A delivery: the carrier's team has a point more and he carries
        // no longer (a wiped team's point leaves the artefact with him).
        let delivered = match was {
            Seen::Carried(by) if artefact(now) != was => seen
                .sides()
                .team_of(by)
                .filter(|team| now.score()[team.index()] > seen.score()[team.index()])
                .map(|team| Cue::Delivered { team }),
            _ => None,
        };
        let news = delivered.or_else(|| match (was, artefact(now)) {
            (Seen::None, Seen::Lying(Spot::Spawner(_))) => Some(Cue::Spawned),
            (Seen::Lying(_), Seen::Carried(by)) => {
                now.sides().team_of(by).map(|team| Cue::Taken { by, team })
            }
            (Seen::Carried(old), Seen::Carried(by)) if old != by => {
                now.sides().team_of(by).map(|team| Cue::Taken { by, team })
            }
            (Seen::Carried(by), Seen::Lying(_)) => {
                seen.sides().team_of(by).map(|team| Cue::Dropped { team })
            }
            (Seen::Lying(_) | Seen::Carried(_), Seen::None) if now.result().is_none() => {
                Some(Cue::Destroyed)
            }
            _ => None,
        });
        let ended = match (seen.result(), now.result()) {
            (None, Some((outcome, over))) => Some(Cue::Ended {
                outcome,
                reason: over.reason,
            }),
            _ => None,
        };
        [news, ended]
    }

    /// The announcer's line for a viewer: none without a team (0.46c2
    /// 0x69F870), none for a removal or the end.
    pub fn line(self, viewer: Viewer) -> Option<Line> {
        let mine = |team: Team, own: Line, enemy: Line| {
            if viewer.team == Some(team) {
                own
            } else {
                enemy
            }
        };
        viewer.team?;
        match self {
            Cue::Spawned => Some(Line::Spawned),
            Cue::Taken { team, .. } => Some(mine(team, Line::PickedUp, Line::PickedUpByEnemy)),
            Cue::Dropped { team } => Some(mine(team, Line::Dropped, Line::DroppedByEnemy)),
            Cue::Delivered { team } => Some(mine(team, Line::Brought, Line::BroughtByEnemy)),
            Cue::Destroyed | Cue::Ended { .. } => None,
        }
    }

    /// The hud's text for a viewer: a take is told to the taker alone, a
    /// drop to nobody (0.46c2 0x6E16C0); a viewer without a team is told
    /// what is nobody's side.
    pub fn message(self, viewer: Viewer) -> Option<Message> {
        match self {
            Cue::Spawned => Some(Message::Spawned),
            Cue::Destroyed => Some(Message::Destroyed),
            Cue::Taken { by, .. } => (viewer.place == Some(by)).then_some(Message::YouPicked),
            Cue::Dropped { .. } => None,
            Cue::Delivered { team } => viewer.team.map(|own| {
                if own == team {
                    Message::TeamDelivered
                } else {
                    Message::EnemyDelivered
                }
            }),
            Cue::Ended {
                outcome: Outcome::Draw,
                ..
            } => Some(Message::Draw),
            Cue::Ended {
                outcome: Outcome::Winner(team),
                reason,
            } => viewer.team.map(|own| {
                if own == team {
                    Message::Won(reason)
                } else {
                    Message::Lost(reason)
                }
            }),
        }
    }
}
