//! What a match is won by, asked as questions (`MODES-READY.md` rules 1-8).
//!
//! Cross-cutting bot code (the commander, the reflexes, the plays that are
//! not a mode's own) asks what the objective is, never which mode it is nor
//! whether a body `carrying` something:
//!
//! - **Where** it is: [`ObjectiveMap::has_homes`], [`ObjectiveMap::home_spots`].
//! - **Who** does the scoring act now: [`ObjectiveMap::is_actor`] (a body of
//!   ours; its hands are its own business and stay `carrying`),
//!   [`TeamObjectiveView::enemy_actors`] (an enemy: only what the team's
//!   knowledge holds, seen or told, never the tracker's truth).
//! - **When** and how hard: [`ObjectiveMap::clock`], [`ObjectiveMap::score_gap`],
//!   [`ObjectiveMap::tickets`], [`ObjectiveMap::enemy_spent`].
//! - A **number per mode** ([`ObjectiveMap::by_mode`], [`Mode::weight_key`]): the one
//!   match on the modes, so a mode added later breaks every such number at
//!   compile time instead of falling to a default.
//!
//! The mode comes from the match's map by [`Mode::parse`], once, at the
//! boundary: a mode the bots do not play (Research) is an error there, not
//! batteries.

use std::fmt;

use mapdata::{MapData, Vec3};
use survarium::v026g::maps::mode;

use super::know::TeamKnowledge;
use super::view::{MatchView, Place, PlaceView};

/// The modes the bots play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Batteries,
    TimedBatteries,
    TeamDeathmatch,
}

/// A mode id the bots do not play (Research, or none known).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unsupported(pub u8);

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the bots do not play mode {}", self.0)
    }
}

impl std::error::Error for Unsupported {}

impl Mode {
    /// Every mode the bots play, for the loops of tests and tallies.
    pub const ALL: [Mode; 3] = [Mode::Batteries, Mode::TimedBatteries, Mode::TeamDeathmatch];

    /// The mode of a map's `mode.mode` id.
    pub fn parse(id: u8) -> Result<Self, Unsupported> {
        match id {
            mode::BATTERIES => Ok(Self::Batteries),
            mode::TIMED_BATTERIES => Ok(Self::TimedBatteries),
            mode::TEAM_DEATHMATCH => Ok(Self::TeamDeathmatch),
            other => Err(Unsupported(other)),
        }
    }

    /// The id of the map's `mode.mode` (the routes table's key).
    pub fn id(self) -> u8 {
        match self {
            Self::Batteries => mode::BATTERIES,
            Self::TimedBatteries => mode::TIMED_BATTERIES,
            Self::TeamDeathmatch => mode::TEAM_DEATHMATCH,
        }
    }

    /// The key of a play's weight for this mode (`weight_tdm`,
    /// `weight_batteries`, `weight_timed`), for the plays that weigh the
    /// timed batteries apart from the plain ones (Cut, HoldExit, Crossfire,
    /// Flank).
    pub fn weight_key(self) -> &'static str {
        match self {
            Self::TeamDeathmatch => "weight_tdm",
            Self::Batteries => "weight_batteries",
            Self::TimedBatteries => "weight_timed",
        }
    }

    /// The questions about this mode's objective.
    pub fn objective(self) -> ObjectiveMap {
        ObjectiveMap { mode: self }
    }
}

/// What the HUD shows of the objective's clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectiveClock {
    /// The objective does not run on a clock.
    Never,
    /// It does (timed batteries), and the world has not said when the next
    /// comes.
    Awaited,
    /// The next battery appears at this match time, ms.
    Next { at_ms: u32 },
}

/// The mode's objective, world-true and fair to everyone: where it is and
/// what the HUD shows of it. It holds no knowledge of any team: an enemy's
/// part in it is asked of a [`TeamObjectiveView`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectiveMap {
    mode: Mode,
}

impl ObjectiveMap {
    /// The key of a play's weight for this mode ([`Mode::weight_key`]).
    pub fn weight_key(self) -> &'static str {
        self.mode.weight_key()
    }

    /// A number (or a pair of them) the modes set apart: the one place
    /// that names the modes for such a choice. Timed batteries are given
    /// apart from the plain ones, as the plays that weigh them so ask for.
    pub fn by_mode<T>(self, tdm: T, batteries: T, timed: T) -> T {
        match self.mode {
            Mode::TeamDeathmatch => tdm,
            Mode::Batteries => batteries,
            Mode::TimedBatteries => timed,
        }
    }

    /// Whether the bodies bring things to a home (a team's container): the
    /// battery modes. Without, the match is a fight (Team Deathmatch).
    pub fn has_homes(self) -> bool {
        self.mode != Mode::TeamDeathmatch
    }

    /// Where the objective lies on the map: the battery spawners and the
    /// home containers; none where there is no home. (Research's capture
    /// zones go here with that update.)
    pub fn home_spots(self, map: &MapData) -> Vec<Vec3> {
        if self.has_homes() {
            map.battery_spawners
                .iter()
                .map(|s| s.position)
                .chain(map.containers.iter().map(|c| c.position))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Whether a body of ours is doing the mode's scoring act now (rule
    /// 2): the carrier in the battery modes, nobody in Team Deathmatch. A
    /// role, not the hands: an actor is never pulled off, not a free
    /// mate, never calm to stop. What its hands cannot do (fire, sprint,
    /// use an item) is `carrying` on the body.
    pub fn is_actor(self, body: &PlaceView) -> bool {
        self.has_homes() && body.carrying.is_some()
    }

    /// The living players of `team` that are actors now.
    pub fn own_actors(self, view: &MatchView, team: u8) -> impl Iterator<Item = &PlaceView> {
        view.team(team).filter(move |body| self.is_actor(body))
    }

    /// Whether the objective's refills come on a clock, so a spot seen
    /// empty is worth another look after a time (timed batteries).
    pub fn refills_on_a_clock(self) -> bool {
        self.mode == Mode::TimedBatteries
    }

    /// What the HUD shows of the objective's clock.
    pub fn clock(self, view: &MatchView) -> ObjectiveClock {
        match (self.refills_on_a_clock(), view.next_battery) {
            (false, _) => ObjectiveClock::Never,
            (true, None) => ObjectiveClock::Awaited,
            (true, Some(at_ms)) => ObjectiveClock::Next { at_ms },
        }
    }

    /// The team's tickets, where the match counts them (Team Deathmatch).
    pub fn tickets(self, view: &MatchView, team: u8) -> Option<u32> {
        (self.mode == Mode::TeamDeathmatch).then(|| view.points[usize::from(team & 1)])
    }

    /// Our points minus theirs, for `team` (tickets count a tenth).
    pub fn score_gap(self, view: &MatchView, team: u8) -> f32 {
        let (ours, theirs) = (view.points[usize::from(team & 1)], view.points[usize::from(!team & 1)]);
        let gap = ours as f32 - theirs as f32;
        if self.mode == Mode::TeamDeathmatch {
            gap / 10.0
        } else {
            gap
        }
    }

    /// Whether the enemies of `team` cannot take part any more: no tickets
    /// to respawn by, so the match is the bodies still standing.
    pub fn enemy_spent(self, view: &MatchView, team: u8) -> bool {
        self.mode == Mode::TeamDeathmatch && !view.can_respawn[usize::from(!team & 1)]
    }

    /// One team's view of the objective ([`TeamObjectiveView`]).
    pub fn for_team<'a>(self, view: &'a MatchView, know: &'a TeamKnowledge, team: u8) -> TeamObjectiveView<'a> {
        TeamObjectiveView { map: self, view, know, team }
    }
}

/// A body doing the scoring act.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    /// Carrying battery `battery` (the battery modes).
    Hauler { place: Place, battery: usize },
}

/// Where a fact about an enemy actor came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Known<T> {
    /// A bot of the team saw it, at match time `at`.
    Seen { what: T, at: u32 },
    /// The HUD or a callout told it, at match time `at`.
    Told { what: T, at: u32 },
}

impl<T> Known<T> {
    pub fn what(&self) -> &T {
        match self {
            Self::Seen { what, .. } | Self::Told { what, .. } => what,
        }
    }
}

/// One team's view of the objective: [`ObjectiveMap`] with that team's
/// bodies from the match view and what that team knows. The only way a
/// decision asks about actors, so an enemy actor can only come from the
/// team's knowledge (rule 6): nothing here reads an enemy's `carrying` off
/// the view.
pub struct TeamObjectiveView<'a> {
    map: ObjectiveMap,
    view: &'a MatchView,
    know: &'a TeamKnowledge,
    team: u8,
}

impl TeamObjectiveView<'_> {
    pub fn map(&self) -> ObjectiveMap {
        self.map
    }

    /// The team's own bodies doing the scoring act: its own are known.
    pub fn own_actors(&self) -> impl Iterator<Item = &PlaceView> {
        self.map.own_actors(self.view, self.team)
    }

    /// The enemy actors the team knows of: haulers a bot saw carrying, and
    /// thieves the HUD told. They last as long as the knowledge does
    /// (`TeamKnowledge::forget_old`); a hauler never seen nor told is not
    /// here, whatever the server's truth.
    pub fn enemy_actors(&self) -> impl Iterator<Item = Known<Actor>> + '_ {
        let homes = self.map.has_homes();
        let seen = self.know.carriers_seen.iter().map(|s| Known::Seen {
            what: Actor::Hauler { place: s.who, battery: s.battery },
            at: s.time,
        });
        let told = self.know.thefts.iter().filter_map(|t| {
            Some(Known::Told { what: Actor::Hauler { place: t.thief?, battery: t.battery }, at: t.time })
        });
        seen.chain(told).filter(move |_| homes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::know::Theft;
    use crate::v026g::bots::plays::tests::Scenario;

    #[test]
    fn the_modes_the_bots_play_parse_and_research_does_not() {
        for id in [mode::BATTERIES, mode::TIMED_BATTERIES, mode::TEAM_DEATHMATCH] {
            assert_eq!(Mode::parse(id).map(Mode::id), Ok(id));
        }
        assert_eq!(Mode::parse(mode::RESEARCH), Err(Unsupported(5)));
        assert_eq!(Mode::parse(0), Err(Unsupported(0)));
        assert_eq!(Mode::parse(6), Err(Unsupported(6)));
        // `ALL` is every mode the parse gives.
        let parsed: Vec<Mode> = (0..=u8::MAX).filter_map(|id| Mode::parse(id).ok()).collect();
        assert_eq!(parsed, Mode::ALL);
    }

    #[test]
    fn only_the_battery_modes_have_homes_and_actors() {
        for mode in Mode::ALL {
            let mut s = Scenario::field(mode);
            s.bot(0, 0, Vec3::new(20.0, 0.0, 30.0)).carrying = Some(0);
            s.bot(1, 0, Vec3::new(22.0, 0.0, 30.0));
            let hauls = mode != Mode::TeamDeathmatch;
            let objective = mode.objective();
            assert_eq!(objective.has_homes(), hauls, "{mode:?}");
            assert_eq!(objective.is_actor(&s.view.places[0]), hauls, "{mode:?}");
            assert!(!objective.is_actor(&s.view.places[1]), "{mode:?}");
            assert_eq!(objective.own_actors(&s.view, 0).count(), usize::from(hauls), "{mode:?}");
            assert_eq!(objective.for_team(&s.view, &s.team, 0).own_actors().count(), usize::from(hauls), "{mode:?}");
        }
    }

    /// An enemy hauler the server carries but this team never saw nor was told of is not an actor
    /// in its view; once a bot saw it, it is (`Seen`), and it stops being one when the knowledge drops it.
    #[test]
    fn an_enemy_hauler_the_team_never_saw_is_not_an_actor() {
        let mut s = Scenario::field(Mode::Batteries);
        s.bot(0, 0, Vec3::new(20.0, 0.0, 30.0));
        s.human(1, 1, Vec3::new(60.0, 0.0, 30.0)).carrying = Some(3);
        let objective = Mode::Batteries.objective();
        // The enemy's `carrying` is in the view, as the tracker infers it; the team's view of the
        // objective does not read it.
        assert_eq!(s.view.places[1].carrying, Some(3));
        assert_eq!(objective.for_team(&s.view, &s.team, 0).enemy_actors().count(), 0);
        // The other team's own hauler is its own actor, and not ours.
        assert_eq!(objective.for_team(&s.view, &s.team, 1).own_actors().count(), 1);
        assert_eq!(objective.for_team(&s.view, &s.team, 0).own_actors().count(), 0);

        s.team.saw_carrying(1, 3, Vec3::new(60.0, 0.0, 30.0), 290_000);
        let actors: Vec<_> = objective.for_team(&s.view, &s.team, 0).enemy_actors().collect();
        assert_eq!(actors, [Known::Seen { what: Actor::Hauler { place: 1, battery: 3 }, at: 290_000 }]);

        // Told by the HUD's theft, a thief of ours is known too.
        s.team.thefts.push(Theft { battery: 5, at: Vec3::ZERO, time: 295_000, thief: Some(2) });
        let actors: Vec<_> = objective.for_team(&s.view, &s.team, 0).enemy_actors().collect();
        assert_eq!(actors.len(), 2);
        assert_eq!(actors[1], Known::Told { what: Actor::Hauler { place: 2, battery: 5 }, at: 295_000 });

        // The knowledge forgets: so does the view.
        s.team.forget_old(295_000 + 61_000);
        assert_eq!(objective.for_team(&s.view, &s.team, 0).enemy_actors().count(), 0);
    }

    #[test]
    fn team_deathmatch_has_no_enemy_actors() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.team.saw_carrying(1, 3, Vec3::ZERO, 290_000);
        assert_eq!(Mode::TeamDeathmatch.objective().for_team(&s.view, &s.team, 0).enemy_actors().count(), 0);
    }

    #[test]
    fn the_clock_is_the_timed_modes_alone() {
        let mut s = Scenario::field(Mode::Batteries);
        s.view.next_battery = Some(12_000);
        for (mode, want) in [
            (Mode::Batteries, ObjectiveClock::Never),
            (Mode::TeamDeathmatch, ObjectiveClock::Never),
            (Mode::TimedBatteries, ObjectiveClock::Next { at_ms: 12_000 }),
        ] {
            assert_eq!(mode.objective().clock(&s.view), want, "{mode:?}");
        }
        s.view.next_battery = None;
        assert_eq!(Mode::TimedBatteries.objective().clock(&s.view), ObjectiveClock::Awaited);
    }

    #[test]
    fn tickets_count_a_tenth_in_the_gap() {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.view.points = [30, 10];
        assert_eq!(Mode::TeamDeathmatch.objective().score_gap(&s.view, 0), 2.0);
        assert_eq!(Mode::TeamDeathmatch.objective().score_gap(&s.view, 1), -2.0);
        assert_eq!(Mode::Batteries.objective().score_gap(&s.view, 0), 20.0);
        assert_eq!(Mode::TeamDeathmatch.objective().tickets(&s.view, 1), Some(10));
        assert_eq!(Mode::Batteries.objective().tickets(&s.view, 1), None);
    }
}
