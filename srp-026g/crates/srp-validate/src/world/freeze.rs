//! Freeze Tag's deterministic rule and world format. A frozen body remains
//! installed in the native world, at its lethal hit, until a teammate thaws it.
//! This state is saved after the respawn rule; there is no wall-clock state.

use crate::{MAX_PLAYERS, Place, Places, Team};
use core::num::NonZeroU16;

pub const INTERMISSION: NonZeroU16 = NonZeroU16::new(5000).unwrap();
pub const WINS: u8 = 5;
pub const PLAYERS: usize = MAX_PLAYERS;
pub const THAW_MS: u16 = 3000;
pub const RADIUS: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Player {
    /// Never entered. A late arrival starts fighting on its first spawn.
    Waiting,
    Fighting(Team),
    Frozen {
        team: Team,
        progress: Progress,
    },
}

/// Completed progress is an action (thaw), never a retained frozen state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress(u16);
impl Progress {
    pub const ZERO: Self = Self(0);
    pub fn millis(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Body {
    Absent,
    Present {
        team: Team,
        position: [f32; 3],
        condition: Condition,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    Fighting,
    Lethal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundEnd {
    Won(Team),
    Draw,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Playing,
    Between {
        result: RoundEnd,
        remaining: NonZeroU16,
    },
    Restarting,
    Finished(Team),
}

/// What a step asks of the native world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actions {
    /// Between rounds, waiting for every body's new round spawn, or finished.
    Wait,
    /// A played step: these places' native health and actions are restored.
    Play { thawed: Places },
    /// Every place spawns again now through the native team spawn.
    Restart,
}

/// What the native respawn rule's schedule (twenty times, one a place: the
/// tick spawns a place whose time is the step's) is left to do after a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Schedule {
    /// In play: a frozen body's death respawn is void (the freeze replaced
    /// the death); every other place keeps its native time, so a late
    /// arrival's first spawn runs.
    CancelFrozen(Places),
    /// Between rounds and after the match: a place that falls due waits, as
    /// Artifact Hunt holds its round's end: a late arrival enters with the
    /// next round, never into the pause.
    HoldDue,
    /// The round's restart: every present place is due at this step
    /// ([`Actions::Restart`]), and a place without a free point retries
    /// natively 50 ms later.
    Native,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreezeTag {
    players: [Player; PLAYERS],
    phase: Phase,
    score: [u8; 2],
    /// The rounds started after the match's first: each restart spawns
    /// everyone with the kit's load, a life of every player then (a round
    /// replayed after a double wipe too).
    restarts: u16,
}

impl FreezeTag {
    pub const EMPTY: Self = Self {
        players: [Player::Waiting; PLAYERS],
        phase: Phase::Playing,
        score: [0; 2],
        restarts: 0,
    };
    /// Version, phase, scores, countdown (u16), round result, twenty player
    /// records, then the restarts (u16).
    pub const LEN: usize = 7 + PLAYERS * 3 + 2;
    pub fn player(&self, place: Place) -> Player {
        // A place is below PLAYERS; the crate reaches no panic.
        self.players.get(place.index()).copied().unwrap_or(Player::Waiting)
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn winner(&self) -> Option<Team> {
        match self.phase {
            Phase::Finished(team) => Some(team),
            _ => None,
        }
    }
    pub fn complete(&self) -> bool {
        self.winner().is_some()
    }
    /// Whether the next round is about to start: the pause and the restart,
    /// which the game plays as the match start's countdown (the warmup
    /// camera, the kits), as Artifact Hunt's fade and countdown.
    pub fn round_countdown(&self) -> bool {
        matches!(self.phase, Phase::Between { .. } | Phase::Restarting)
    }
    /// What the respawn rule's schedule is left to do in this phase.
    pub fn schedule(&self) -> Schedule {
        match self.phase {
            Phase::Playing => Schedule::CancelFrozen(
                Place::all()
                    .zip(&self.players)
                    .filter(|(_, player)| matches!(player, Player::Frozen { .. }))
                    .fold(Places::EMPTY, |frozen, (place, _)| frozen.with(place)),
            ),
            Phase::Between { .. } | Phase::Finished(_) => Schedule::HoldDue,
            Phase::Restarting => Schedule::Native,
        }
    }
    /// Round wins, the first team's first.
    pub fn points(&self) -> [u16; 2] {
        self.score.map(u16::from)
    }
    /// How many rounds started after the match's first ([`Actions::Restart`]):
    /// every one a new life of each player in the match then.
    pub fn round_starts(&self) -> u16 {
        self.restarts
    }
    pub fn points_of(&self, team: Team) -> u16 {
        let [first, second] = self.score;
        u16::from(match team {
            Team::First => first,
            Team::Second => second,
        })
    }
    fn score_mut(&mut self, team: Team) -> &mut u8 {
        let [first, second] = &mut self.score;
        match team {
            Team::First => first,
            Team::Second => second,
        }
    }
    pub fn fits(&self, places: usize) -> bool {
        self.players
            .get(places..)
            .is_some_and(|rest| rest.iter().all(|p| *p == Player::Waiting))
    }
    fn living(&self, team: Team) -> usize {
        self.players
            .iter()
            .filter(|p| **p == Player::Fighting(team))
            .count()
    }

    /// One simulation step. All rescues use the same pre-thaw population:
    /// a player revived this tick cannot revive another until the next.
    /// `visible[a][b]` is the world's obstruction test between the bodies.
    pub fn step(
        &mut self,
        bodies: &[Body; PLAYERS],
        visible: &[[bool; PLAYERS]; PLAYERS],
        elapsed: u16,
    ) -> Actions {
        match self.phase {
            Phase::Finished(_) => return Actions::Wait,
            Phase::Between { result, remaining } => {
                return match NonZeroU16::new(remaining.get().saturating_sub(elapsed)) {
                    Some(remaining) => {
                        self.phase = Phase::Between { result, remaining };
                        Actions::Wait
                    }
                    None => {
                        self.phase = Phase::Restarting;
                        self.restarts = self.restarts.saturating_add(1);
                        Actions::Restart
                    }
                };
            }
            Phase::Restarting => {
                // Native respawn may retry a crowded spawn point. Keep the
                // round closed until every present body has actually spawned
                // (the hold keeps each one lethal until its new spawn).
                if bodies.iter().any(|b| {
                    matches!(
                        b,
                        Body::Present {
                            condition: Condition::Lethal,
                            ..
                        }
                    )
                }) {
                    return Actions::Wait;
                }
                self.players = [Player::Waiting; PLAYERS];
                self.phase = Phase::Playing;
            }
            Phase::Playing => {}
        }
        for (player, body) in self.players.iter_mut().zip(bodies) {
            match (*player, *body) {
                (
                    Player::Waiting | Player::Fighting(_),
                    Body::Present {
                        team,
                        condition: Condition::Lethal,
                        ..
                    },
                ) => {
                    *player = Player::Frozen {
                        team,
                        progress: Progress::ZERO,
                    };
                }
                (Player::Waiting, Body::Present { team, .. }) => *player = Player::Fighting(team),
                // Leaving cannot erase a freeze. Nor can reconnecting turn
                // a retained frozen place back into a fighting player.
                (Player::Fighting(team), Body::Absent) => {
                    *player = Player::Frozen {
                        team,
                        progress: Progress::ZERO,
                    };
                }
                _ => {}
            }
        }
        let before = self.players;
        let mut thawed = Places::EMPTY;
        for (place, ((slot, body), sight)) in Place::all().zip(self.players.iter_mut().zip(bodies).zip(visible)) {
            let Player::Frozen { team, progress } = *slot else {
                continue;
            };
            let helped = match *body {
                Body::Present { position, .. } => before.iter().zip(bodies).zip(sight).any(
                    |((helper, body), clear)| match *body {
                        Body::Present { position: other, .. } if *clear && *helper == Player::Fighting(team) => {
                            let ([ax, ay, az], [bx, by, bz]) = (position, other);
                            let [x, y, z] = [ax - bx, ay - by, az - bz];
                            x * x + y * y + z * z <= RADIUS * RADIUS
                        }
                        _ => false,
                    },
                ),
                Body::Absent => false,
            };
            let progress = if helped {
                progress.0.saturating_add(elapsed)
            } else {
                0
            };
            if progress >= THAW_MS {
                *slot = Player::Fighting(team);
                thawed = thawed.with(place);
            } else {
                *slot = Player::Frozen {
                    team,
                    progress: Progress(progress),
                };
            }
        }
        if let Some(end) = self.result() {
            if let RoundEnd::Won(team) = end {
                let score = self.score_mut(team);
                *score = score.saturating_add(1);
            }
            self.phase = match end {
                RoundEnd::Won(team) if self.points_of(team) == u16::from(WINS) => Phase::Finished(team),
                _ => Phase::Between {
                    result: end,
                    remaining: INTERMISSION,
                },
            };
        }
        Actions::Play { thawed }
    }
    /// A round ends once both teams entered and one or both have nobody fighting.
    fn result(&self) -> Option<RoundEnd> {
        let entered = |side| {
            self.players.iter().any(|p| match *p {
                Player::Fighting(team) | Player::Frozen { team, .. } => team == side,
                Player::Waiting => false,
            })
        };
        if !entered(Team::First) || !entered(Team::Second) {
            return None;
        }
        match (self.living(Team::First), self.living(Team::Second)) {
            (0, 0) => Some(RoundEnd::Draw),
            (0, _) => Some(RoundEnd::Won(Team::Second)),
            (_, 0) => Some(RoundEnd::Won(Team::First)),
            _ => None,
        }
    }

    /// Whether the restarts fit the rounds: every round that ended (a win in
    /// the scores, or a draw, which scores nothing) was restarted but the one
    /// between rounds or the match's last; so the draws they leave are none
    /// fewer than zero, and one at least between rounds after a draw.
    fn restarts_fit(&self) -> bool {
        let [first, second] = self.score;
        let wins = u16::from(first).saturating_add(u16::from(second));
        let (pending, draw_now) = match self.phase {
            Phase::Playing => (0, false),
            Phase::Restarting => return self.restarts > 0 && self.restarts >= wins,
            Phase::Between { result, .. } => (1, result == RoundEnd::Draw),
            Phase::Finished(_) => (1, false),
        };
        let Some(ended) = self.restarts.checked_add(pending) else { return false };
        match ended.checked_sub(wins) {
            Some(draws) => !draw_now || draws > 0,
            None => false,
        }
    }

    pub fn bytes(&self) -> [u8; Self::LEN] {
        let mut bytes = [0; Self::LEN];
        let [version, phase, zero, one, low, high, result, slots @ .., restarts_low, restarts_high] = &mut bytes;
        [*restarts_low, *restarts_high] = self.restarts.to_le_bytes();
        *version = 1;
        [*zero, *one] = self.score;
        let team = |team| match team {
            Team::First => 1,
            Team::Second => 2,
        };
        *phase = match self.phase {
            Phase::Playing => 0,
            Phase::Between {
                result: end,
                remaining,
            } => {
                [*low, *high] = remaining.get().to_le_bytes();
                *result = match end {
                    RoundEnd::Won(winner) => team(winner),
                    RoundEnd::Draw => 3,
                };
                1
            }
            Phase::Restarting => 2,
            Phase::Finished(winner) => {
                *result = team(winner);
                3
            }
        };
        for (player, out) in self.players.iter().zip(slots.as_chunks_mut::<3>().0) {
            let (tag, progress) = match *player {
                Player::Waiting => (0, 0),
                Player::Fighting(side) => (team(side), 0),
                Player::Frozen { team: Team::First, progress } => (3, progress.0),
                Player::Frozen { team: Team::Second, progress } => (4, progress.0),
            };
            let [low, high] = progress.to_le_bytes();
            *out = [tag, low, high];
        }
        bytes
    }
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let bytes: &[u8; Self::LEN] = bytes.try_into().ok()?;
        let [version, phase, zero, one, low, high, result, slots @ .., restarts_low, restarts_high] = bytes;
        if *version != 1 {
            return None;
        }
        let mut state = Self::EMPTY;
        if *zero > WINS || *one > WINS || (*zero == WINS && *one == WINS) {
            return None;
        }
        state.score = [*zero, *one];
        let remaining = u16::from_le_bytes([*low, *high]);
        state.phase = match (*phase, remaining, *result) {
            (0, 0, 0) => Phase::Playing,
            (1, n, r @ 1..=3) if n <= INTERMISSION.get() => Phase::Between {
                remaining: NonZeroU16::new(n)?,
                result: match r {
                    1 => RoundEnd::Won(Team::First),
                    2 => RoundEnd::Won(Team::Second),
                    _ => RoundEnd::Draw,
                },
            },
            (2, 0, 0) => Phase::Restarting,
            (3, 0, 1) if *zero == WINS => Phase::Finished(Team::First),
            (3, 0, 2) if *one == WINS => Phase::Finished(Team::Second),
            _ => return None,
        };
        if !state.complete() && (*zero == WINS || *one == WINS) {
            return None;
        }
        for (player, &[tag, low, high]) in state.players.iter_mut().zip(slots.as_chunks::<3>().0) {
            let progress = u16::from_le_bytes([low, high]);
            *player = match (tag, progress) {
                (0, 0) => Player::Waiting,
                (1, 0) => Player::Fighting(Team::First),
                (2, 0) => Player::Fighting(Team::Second),
                (3, p) if p < THAW_MS => Player::Frozen {
                    team: Team::First,
                    progress: Progress(p),
                },
                (4, p) if p < THAW_MS => Player::Frozen {
                    team: Team::Second,
                    progress: Progress(p),
                },
                _ => return None,
            };
        }
        let valid = match state.phase {
            Phase::Playing => state.result().is_none(),
            Phase::Between {
                result: RoundEnd::Won(team),
                ..
            }
            | Phase::Finished(team) => state.result() == Some(RoundEnd::Won(team)) && state.points_of(team) > 0,
            Phase::Between {
                result: RoundEnd::Draw,
                ..
            } => state.result() == Some(RoundEnd::Draw),
            Phase::Restarting => state.result().is_some(),
        };
        state.restarts = u16::from_le_bytes([*restarts_low, *restarts_high]);
        (valid && state.restarts_fit()).then_some(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn thawed(actions: Actions) -> u32 {
        match actions {
            Actions::Play { thawed } => thawed.bits(),
            other => panic!("{other:?}"),
        }
    }
    fn place(n: u8) -> Place {
        Place::new(n).unwrap()
    }
    const VISIBLE: [[bool; PLAYERS]; PLAYERS] = [[true; PLAYERS]; PLAYERS];
    fn body(team: Team, x: f32, condition: Condition) -> Body {
        Body::Present {
            team,
            position: [x, 0.0, 0.0],
            condition,
        }
    }
    fn match_bodies() -> [Body; PLAYERS] {
        let mut b = [Body::Absent; PLAYERS];
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        b[1] = body(Team::First, RADIUS, Condition::Fighting);
        b[2] = body(Team::Second, 1.0, Condition::Fighting);
        b
    }
    #[test]
    fn proximity_requires_three_continuous_seconds_and_restores_after_rollback() {
        let b = match_bodies();
        let mut s = FreezeTag::EMPTY;
        assert_eq!(thawed(s.step(&b, &VISIBLE, 2950)), 0);
        let saved = s.bytes();
        assert_eq!(thawed(s.step(&b, &VISIBLE, 50)), 1);
        assert_eq!(s.player(place(0)), Player::Fighting(Team::First));
        s = FreezeTag::read(&saved).unwrap();
        let mut far = b;
        far[1] = body(Team::First, RADIUS + 0.01, Condition::Fighting);
        assert_eq!(thawed(s.step(&far, &VISIBLE, 50)), 0);
        assert_eq!(thawed(s.step(&b, &VISIBLE, 2950)), 0);
        assert_eq!(thawed(s.step(&b, &VISIBLE, 50)), 1);
    }
    #[test]
    fn enemies_frozen_helpers_and_walls_do_not_thaw() {
        let mut b = match_bodies();
        b[3] = body(Team::First, 20.0, Condition::Fighting);
        let mut s = FreezeTag::EMPTY;
        assert_eq!(thawed(s.step(&b, &[[false; PLAYERS]; PLAYERS], 3000)), 0);
        b[1] = body(Team::First, 1.0, Condition::Lethal);
        assert_eq!(thawed(s.step(&b, &VISIBLE, 3000)), 0);
        assert!(!s.complete());
    }
    #[test]
    fn team_wipe_is_terminal_simultaneous_wipe_draws_and_solo_waits() {
        let mut b = match_bodies();
        let mut s = FreezeTag::EMPTY;
        s.step(&b, &VISIBLE, 50);
        b[1] = body(Team::First, 3.0, Condition::Lethal);
        s.step(&b, &VISIBLE, 50);
        assert!(matches!(
            s.phase(),
            Phase::Between {
                result: RoundEnd::Won(Team::Second),
                ..
            }
        ));
        assert_eq!(s.points(), [0, 1]);
        assert_eq!(s.step(&match_bodies(), &VISIBLE, 3000), Actions::Wait);
        b[2] = body(Team::Second, 1.0, Condition::Lethal);
        let mut s = FreezeTag::EMPTY;
        s.step(&b, &VISIBLE, 50);
        assert!(matches!(
            s.phase(),
            Phase::Between {
                result: RoundEnd::Draw,
                ..
            }
        ));
        b[2] = Body::Absent;
        let mut s = FreezeTag::EMPTY;
        s.step(&b, &VISIBLE, 50);
        assert!(!s.complete());
    }
    #[test]
    fn wire_domains_and_roster_are_checked() {
        let mut s = FreezeTag::EMPTY;
        s.step(&match_bodies(), &VISIBLE, 1500);
        assert_eq!(FreezeTag::read(&s.bytes()), Some(s));
        assert!(s.fits(3));
        assert!(!s.fits(2));
        let mut bytes = s.bytes();
        bytes[8..10].copy_from_slice(&THAW_MS.to_le_bytes());
        assert!(FreezeTag::read(&bytes).is_none());
        bytes = s.bytes();
        bytes[7] = 5;
        assert!(FreezeTag::read(&bytes).is_none());
        bytes = s.bytes();
        bytes[1] = 4;
        assert!(FreezeTag::read(&bytes).is_none());
        assert!(FreezeTag::read(&bytes[..bytes.len() - 1]).is_none());
    }

    #[test]
    fn best_of_nine_restores_between_rounds_and_finishes_at_five() {
        use Team::{First, Second};
        for winners in [[First; WINS as usize].as_slice(), &[First, Second, Second, First, Second, First, First, Second, Second]] {
            let mut s = FreezeTag::EMPTY;
            for (round, &winner) in winners.iter().enumerate() {
                let mut bodies = [Body::Absent; PLAYERS];
                bodies[0] = body(Team::First, 0.0, Condition::Fighting);
                bodies[1] = body(Team::Second, 10.0, Condition::Fighting);
                s.step(&bodies, &VISIBLE, 50);
                bodies[1 - winner.index()] = body(
                    if winner == Team::First {
                        Team::Second
                    } else {
                        Team::First
                    },
                    10.0,
                    Condition::Lethal,
                );
                s.step(&bodies, &VISIBLE, 50);
                assert_eq!(s.points().iter().sum::<u16>(), (round + 1) as u16);
                assert_eq!(FreezeTag::read(&s.bytes()), Some(s));
                if round + 1 == winners.len() {
                    assert_eq!(s.winner(), Some(winner));
                    let saved = s;
                    assert_eq!(s.step(&bodies, &VISIBLE, 6000), Actions::Wait);
                    assert_eq!(s, saved);
                } else {
                    assert!(!s.complete());
                    assert_eq!(s.step(&bodies, &VISIBLE, 4950), Actions::Wait);
                    s = FreezeTag::read(&s.bytes()).unwrap();
                    assert_eq!(s.step(&bodies, &VISIBLE, 50), Actions::Restart);
                    assert_eq!(FreezeTag::read(&s.bytes()), Some(s));
                    // A delayed native spawn keeps the round closed; it
                    // cannot award another win for the same frozen body.
                    assert_eq!(s.step(&bodies, &VISIBLE, 50), Actions::Wait);
                    assert_eq!(s.phase(), Phase::Restarting);
                }
            }
        }
    }

    #[test]
    fn the_schedule_cancels_frozen_in_play_holds_between_and_runs_the_restart() {
        let mut bodies = match_bodies();
        let mut s = FreezeTag::EMPTY;
        assert_eq!(s.schedule(), Schedule::CancelFrozen(Places::EMPTY));
        s.step(&bodies, &[[false; PLAYERS]; PLAYERS], 50);
        assert_eq!(s.schedule(), Schedule::CancelFrozen(Places::EMPTY.with(place(0))));
        bodies[1] = body(Team::First, 3.0, Condition::Lethal);
        s.step(&bodies, &VISIBLE, 50);
        assert_eq!(s.schedule(), Schedule::HoldDue);
        assert_eq!(s.step(&bodies, &VISIBLE, INTERMISSION.get()), Actions::Restart);
        assert_eq!(s.schedule(), Schedule::Native);
        for b in bodies.iter_mut().take(3) {
            if let Body::Present { condition, .. } = b {
                *condition = Condition::Fighting;
            }
        }
        s.step(&bodies, &VISIBLE, 50);
        assert_eq!(s.phase(), Phase::Playing);
        assert_eq!(s.schedule(), Schedule::CancelFrozen(Places::EMPTY));
    }

    /// Fighting bodies of both teams, team 0 at x 0 and team 1 at x 10.
    fn fighting() -> [Body; PLAYERS] {
        let mut b = [Body::Absent; PLAYERS];
        b[0] = body(Team::First, 0.0, Condition::Fighting);
        b[1] = body(Team::Second, 10.0, Condition::Fighting);
        b
    }
    /// One round played to its end (`frozen` the places frozen at it) and,
    /// unless it ends the match, restarted and reopened.
    fn round(s: &mut FreezeTag, frozen: &[usize]) {
        let mut b = fighting();
        s.step(&b, &VISIBLE, 50);
        for &i in frozen {
            if let Body::Present { condition, .. } = &mut b[i] {
                *condition = Condition::Lethal;
            }
        }
        s.step(&b, &VISIBLE, 50);
        if s.complete() {
            return;
        }
        assert_eq!(FreezeTag::read(&s.bytes()), Some(*s));
        assert_eq!(s.step(&b, &VISIBLE, INTERMISSION.get()), Actions::Restart);
        assert_eq!(FreezeTag::read(&s.bytes()), Some(*s));
        s.step(&fighting(), &VISIBLE, 50);
        assert_eq!(s.phase(), Phase::Playing);
    }

    #[test]
    fn every_restart_counts_a_round_start_the_double_wipe_replay_too() {
        let mut s = FreezeTag::EMPTY;
        assert_eq!(s.round_starts(), 0);
        round(&mut s, &[1]);
        assert_eq!((s.points(), s.round_starts()), ([1, 0], 1));
        // A double wipe scores nothing; its replay is a round start.
        round(&mut s, &[0, 1]);
        assert_eq!((s.points(), s.round_starts()), ([1, 0], 2));
        round(&mut s, &[0]);
        round(&mut s, &[1]);
        assert_eq!((s.points(), s.round_starts()), ([2, 1], 4));
        round(&mut s, &[1]);
        round(&mut s, &[1]);
        assert_eq!((s.points(), s.round_starts()), ([4, 1], 6));
        // The match's last round is never restarted.
        round(&mut s, &[1]);
        assert_eq!(s.winner(), Some(Team::First));
        assert_eq!(s.round_starts(), 6);
        // A rollback restores the count with the rest.
        assert_eq!(FreezeTag::read(&s.bytes()), Some(s));
    }

    #[test]
    fn the_restarts_are_no_fewer_than_the_rounds_ended_before() {
        let restarts = |s: &FreezeTag, n: u16| {
            let mut bytes = s.bytes();
            bytes[FreezeTag::LEN - 2..].copy_from_slice(&n.to_le_bytes());
            FreezeTag::read(&bytes)
        };
        let mut s = FreezeTag::EMPTY;
        round(&mut s, &[1]);
        round(&mut s, &[1]);
        // Playing at 2:0: two restarts at least; more are replayed draws.
        assert!(restarts(&s, 1).is_none());
        assert!(restarts(&s, 2).is_some() && restarts(&s, 9).is_some());
        // Between rounds after a win: that win is not restarted yet.
        let mut b = fighting();
        s.step(&b, &VISIBLE, 50);
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        s.step(&b, &VISIBLE, 50);
        assert_eq!(s.points(), [2, 1]);
        assert!(restarts(&s, 1).is_none() && restarts(&s, 2).is_some());
        // Restarting: this round's restart is counted already.
        s.step(&b, &VISIBLE, INTERMISSION.get());
        assert_eq!(s.phase(), Phase::Restarting);
        assert!(restarts(&s, 2).is_none() && restarts(&s, 3).is_some());
        // Between rounds after a double wipe at 1:0: the win restarted, and
        // this draw to come.
        let mut s = FreezeTag::EMPTY;
        round(&mut s, &[1]);
        let mut b = fighting();
        s.step(&b, &VISIBLE, 50);
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        b[1] = body(Team::Second, 10.0, Condition::Lethal);
        s.step(&b, &VISIBLE, 50);
        assert!(matches!(s.phase(), Phase::Between { result: RoundEnd::Draw, .. }));
        assert!(restarts(&s, 0).is_none() && restarts(&s, 1).is_some());
        assert!(restarts(&FreezeTag::EMPTY, u16::MAX).is_some());
    }

    #[test]
    fn draws_replay_the_round_and_disconnects_cannot_clear_a_freeze() {
        let mut bodies = match_bodies();
        let mut s = FreezeTag::EMPTY;
        s.step(&bodies, &VISIBLE, 50);
        bodies[1] = Body::Absent;
        s.step(&bodies, &VISIBLE, 50);
        assert_eq!(s.points(), [0, 1]);
        for b in bodies.iter_mut().take(3) {
            *b = body(Team::First, 0.0, Condition::Lethal);
        }
        bodies[2] = body(Team::Second, 0.0, Condition::Lethal);
        s = FreezeTag::EMPTY;
        s.step(&bodies, &VISIBLE, 50);
        assert_eq!(s.points(), [0, 0]);
        assert_eq!(s.step(&bodies, &VISIBLE, INTERMISSION.get()), Actions::Restart);
        assert_eq!(FreezeTag::read(&s.bytes()), Some(s));
    }
}
