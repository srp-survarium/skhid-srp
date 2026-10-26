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

/// Stepping out of a live grenade (`grenade_threat.rs`, `grenades-record/avoid-design.md`).
///
/// Every number but the switch is a GUESS and a sweep arm: the live matches show no human reaction to a
/// landed grenade at all (mates and enemies leave the radius as often as they do 20 s earlier or later,
/// `docs/notes/grenade_visibility.md`), so no delay can be copied from them; the defaults are the
/// published reaction times (Rabin, Game AI Pro 2 ch. 5: 0.16-0.22 s simple, 0.38 s recognition).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AvoidTuning {
    /// A bot steps out of the blast of every grenade it knows of (below). Off: no bot knows of one, the
    /// list is not read.
    pub grenade_avoid: bool,
    /// The thrower's teammates know the landing from the launch (the owner's rule, a justified cheat: a
    /// human team can call it, few do); they step out after this long, ms, drawn.
    pub told_ms: (u32, u32),
    /// The other team's bots know of a grenade once it lies on the ground and they hear or see it; they
    /// step out after this long, ms, drawn.
    pub notice_ms: (u32, u32),
    /// An enemy sees a grenade on the ground within this, metres, in its view cone, with a line to it.
    /// The client's draw distance for the 0.05 x 0.10 m model is not known.
    pub see_m: f32,
    /// The view cone of that, degrees (the whole angle).
    pub cone_deg: f32,
    /// An enemy within this of a landing hears the first bounce, metres (the live bounce sound is silent
    /// from 20 m; at 8 m it is 0.07 of full).
    pub hear_m: f32,
    /// A bot is out of the blast this far past its radius, metres.
    pub margin_m: f32,
    /// A bot whose walk would take it into the blast within this, ms, counts as in it.
    pub ahead_ms: u32,
    /// How often a bot that is not stepping looks again at the grenades, ms.
    pub look_ms: u32,
}

impl Default for AvoidTuning {
    fn default() -> Self {
        Self {
            grenade_avoid: false,
            told_ms: (200, 400),
            notice_ms: (400, 900),
            see_m: 30.0,
            cone_deg: 100.0,
            hear_m: 8.0,
            margin_m: 0.5,
            ahead_ms: 1_000,
            look_ms: 200,
        }
    }
}

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

/// The personalities and their share of the bots (play 14). In a file, a list of them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Personalities(pub Vec<Personality>);

impl Default for Personalities {
    fn default() -> Self {
        Self(Personality::defaults())
    }
}

impl std::ops::Deref for Personalities {
    type Target = Vec<Personality>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Personalities {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Check for Personalities {
    /// At least one, their shares summing to about 1.
    fn check(&self) -> Result<(), String> {
        let shares: f32 = self.0.iter().map(|p| p.share).sum();
        check(
            !self.0.is_empty() && (shares - 1.0).abs() < 0.05,
            "personalities: shares must sum to 1",
        )
    }
}

impl Check for AvoidTuning {
    /// The avoidance's ranges in order.
    fn check(&self) -> Result<(), String> {
        check(self.told_ms.0 <= self.told_ms.1, "avoid.told_ms")?;
        check(self.notice_ms.0 <= self.notice_ms.1, "avoid.notice_ms")?;
        check(self.see_m >= 0.0 && self.hear_m >= 0.0, "avoid.see_m, hear_m")?;
        check((0.0..=360.0).contains(&self.cone_deg), "avoid.cone_deg")?;
        check(self.margin_m >= 0.0, "avoid.margin_m")?;
        check(self.look_ms >= 20, "avoid.look_ms")?;
        Ok(())
    }
}

impl Check for ThinkTuning {
    /// The think rates' ranges in order.
    fn check(&self) -> Result<(), String> {
        check(
            self.think_ms.0 > 0 && self.think_ms.0 <= self.think_ms.1,
            "think.think_ms",
        )?;
        check(self.commander_ms >= 50, "think.commander_ms")?;
        Ok(())
    }
}
