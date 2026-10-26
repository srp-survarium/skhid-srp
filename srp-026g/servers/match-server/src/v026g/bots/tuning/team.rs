use super::*;

/// The team modifiers (`signals::Mods`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModTuning {
    /// Aggression from the score gap (behind is high) and the match's
    /// progress.
    pub aggression_gap: Consideration,
    pub aggression_time: Consideration,
    /// Risk from the share of the team alive against the enemy's.
    pub risk_alive: Consideration,
    /// Objective urgency from the time left.
    pub urgency_time: Consideration,
    /// How far a modifier moves toward its target each commander tick.
    pub smoothing: f32,
    pub min: f32,
    pub max: f32,
}

impl Default for ModTuning {
    fn default() -> Self {
        use super::super::curve::{Curve, Input};
        Self {
            aggression_gap: Consideration::new(
                Input::ScoreGap,
                -3.0,
                3.0,
                Curve::Logistic { k: 8.0, mid: 0.4 },
            )
            .inverted(),
            aggression_time: Consideration::new(
                Input::TimeLeft,
                0.0,
                900.0,
                Curve::Poly { k: 0.5 },
            )
            .inverted(),
            risk_alive: Consideration::new(Input::EnemiesDead, 0.0, 1.0, Curve::Linear),
            urgency_time: Consideration::new(Input::TimeLeft, 0.0, 600.0, Curve::Poly { k: 2.0 })
                .inverted(),
            smoothing: 0.2,
            min: 0.1,
            max: 1.0,
        }
    }
}

/// The team's threat spots (K1, `threat.rs`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThreatTuning {
    /// The team remembers where a shooter kills from and its routes keep
    /// out of his line (a soft preference); off, none of it runs.
    pub spots: bool,
    /// A bot walking to a goal in a held shooter's lane runs: it crosses
    /// fast when there is no good way round, and keeps its task.
    pub sprint: bool,
    /// A route through a lane gets a local detour round it where there is a
    /// short one (the search is capped, `search_floors`).
    pub detour: bool,
    /// Floors of a poll's search budget one search round the covered
    /// regions may take (the poll's is `ThinkTuning::search_floors_per_poll`).
    pub search_floors: u32,
}

impl Default for ThreatTuning {
    fn default() -> Self {
        Self { spots: true, sprint: true, detour: true, search_floors: 3_000 }
    }
}

/// [`ThinkTuning::search_floors_per_poll`] by default: a floor expanded
/// costs about 0.2 µs on a fast desktop core and 0.5 µs on the host's, so
/// a poll searches for at most about 2 ms here and 5 ms there. Measured on
/// six maps, ten seeds each (`docs/notes/player_bots_mapdata.md` §6): the
/// bots play as with no bound, and the worst poll fell from up to 81 ms to
/// 3-10 ms.
pub const SEARCH_FLOORS_PER_POLL: u32 = 10_000;

/// How often what runs (research B5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThinkTuning {
    /// A bot's think (senses, play step), ms: drawn per think.
    pub think_ms: (u32, u32),
    /// The commander's tick, ms.
    pub commander_ms: u32,
    /// Sight rays a poll may cast, all bots together.
    pub rays_per_poll: u32,
    /// Floors a poll's route searches may expand, all bots together, as
    /// Detour's crowd shares its path queue's iterations: a search that
    /// runs past them goes on at the next polls (`Bots::poll`).
    pub search_floors_per_poll: u32,
    /// A trace line at least this often per bot, ms.
    pub trace_every_ms: u32,
    /// A bot that gave up the tries to leave its pocket (`escape::Trapped::gave_up`)
    /// asks no search and takes no goal outside the pocket (`Ctx::reachable`,
    /// `drive`): it is the escape's, and `stuck::SelfKill`'s at the end. Off (the default
    /// until a train turns it on): it asks again for every goal the plays give it, every
    /// 0.5 s, as it did.
    pub shut_in_gate: bool,
}

impl Default for ThinkTuning {
    fn default() -> Self {
        Self {
            think_ms: (200, 400),
            commander_ms: 500,
            rays_per_poll: 400,
            search_floors_per_poll: SEARCH_FLOORS_PER_POLL,
            trace_every_ms: 5_000,
            shut_in_gate: false,
        }
    }
}

/// A personality (play 14): weights per play family and motor habits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Personality {
    pub name: String,
    /// Its share of the bots.
    pub share: f32,
    /// Weights, about 1 (`curve::Input::Personality` reads them).
    pub hold: f32,
    pub push: f32,
    pub sneak: f32,
    pub support: f32,
    /// How readily it sprints, `[0, 1]`.
    pub sprint: f32,
}

impl Default for Personality {
    fn default() -> Self {
        Self {
            name: "steady".into(),
            share: 1.0,
            hold: 1.0,
            push: 1.0,
            sneak: 1.0,
            support: 1.0,
            sprint: 0.5,
        }
    }
}

impl Personality {
    /// The mix of how humans play (saved worlds, per life): about 55 %
    /// steady, 30 % mostly sprinting, 10-15 % holding still; so mostly
    /// flankers and support, a third rushers, few campers.
    pub fn defaults() -> Vec<Self> {
        let p = |name: &str, share, hold, push, sneak, support, sprint| Self {
            name: name.into(),
            share,
            hold,
            push,
            sneak,
            support,
            sprint,
        };
        vec![
            p("flanker", 0.3, 0.8, 1.0, 1.6, 0.9, 0.5),
            p("support", 0.25, 0.9, 0.8, 0.7, 1.6, 0.4),
            p("rusher", 0.3, 0.5, 1.6, 0.9, 0.8, 0.9),
            p("camper", 0.15, 1.8, 0.6, 0.8, 1.0, 0.2),
        ]
    }
}

/// The think rates' ranges in order.
pub(super) fn validate_think(think: &ThinkTuning) -> Result<(), String> {
    check(
        think.think_ms.0 > 0 && think.think_ms.0 <= think.think_ms.1,
        "think.think_ms",
    )?;
    check(think.commander_ms >= 50, "think.commander_ms")?;
    Ok(())
}
