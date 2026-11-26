//! Freeze Tag's deterministic rule and world format. A frozen body remains
//! installed in the native world, at its lethal hit, until a teammate thaws it
//! or, after [`AUTO_THAW_MS`], it thaws by itself. A round ends and the next
//! one starts by Artifact Hunt's restart (`artefact_hunt_rule::Restart`, the
//! one machine of both modes). This state is saved after the respawn rule.

use crate::reader::Reader;
use crate::{MAX_PLAYERS, Place, Places, Team};
use artefact_hunt_rule::{Lives, Ms, Restart, Spawning, Step, Time, Timing};
use core::num::NonZeroU32;

/// The restart's parts: Artifact Hunt's maps' own (3 s after the round, the
/// 1 s fade, the 5 s countdown).
pub const TIMING: Timing = Timing::STOCK;
pub const WINS: u8 = 5;
pub const PLAYERS: usize = MAX_PLAYERS;
pub const THAW_MS: u16 = 4000;
/// A frozen player thaws by himself this long after his freeze, a stop gap
/// (the owner, 2026-11-22: "2 min unthaw is good for now as a stop gap").
pub const AUTO_THAW_MS: NonZeroU32 = NonZeroU32::new(120_000).unwrap();
pub const RADIUS: f32 = 2.0;
/// A team's last one fighting is shown on the other team's maps this long after his team was down
/// to him (the owner, 2026-11-26: "the last person should only be seen after 15 seconds, with a
/// clear timer visible for everyone").
pub const REVEAL: Ms = Ms::short(15_000);

/// A team's last one fighting ([`FreezeTag::last_fighter`]): his place and the step his team was
/// down to him, which the state keeps (every game reads the same, a late joiner's and a
/// resynced one's too).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastFighter {
    pub place: Place,
    pub since: Time,
}
impl LastFighter {
    /// When he is shown on the other team's maps: [`REVEAL`] after his team was down to him.
    pub fn reveal_at(self) -> Time {
        self.since.after(REVEAL)
    }
    /// Whether he is shown at `now`.
    pub fn revealed(self, now: Time) -> bool {
        now.reached(self.reveal_at())
    }
}
/// A player record's bytes: tag, thaw progress (u16), auto-thaw time left (u32).
const RECORD: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Player {
    /// Not seen in this round yet: everyone at a round's start, who fights
    /// from his first spawn. A place that enters while a round is in play
    /// spawns lethal (the exe's spawn hook), so it is frozen at its first step.
    Waiting,
    Fighting(Team),
    Frozen {
        team: Team,
        /// A teammate's thaw.
        progress: Progress,
        /// The time left to the auto-thaw.
        thaws_in: NonZeroU32,
    },
}
impl Player {
    fn frozen(team: Team) -> Self {
        Self::Frozen {
            team,
            progress: Progress::ZERO,
            thaws_in: AUTO_THAW_MS,
        }
    }
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

/// A round's restart: Freeze Tag carries nothing from the fade to the round.
pub type RoundRestart = Restart<(), ()>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Playing,
    /// The round that ended so restarts. Nobody's state changes until the
    /// fade's end respawns everyone ([`Step::Started`]), when every player
    /// waits for his new spawn.
    Restart { ended: RoundEnd, restart: RoundRestart },
    /// `winner` won the match: the time after its last round ended at `at`.
    Finished { winner: Team, at: Time },
}

/// What a step asks of the native world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actions {
    /// A round's restart or the match's end: nothing of the rule's own.
    Wait,
    /// A played step: these places' native health and actions are restored
    /// (a teammate's thaw or the auto-thaw); `thawers` completed a thaw at it
    /// (every fighting teammate within reach of a body a teammate thawed),
    /// each credited as for a kill; an auto-thaw credits nobody.
    Play { thawed: Places, thawers: Places },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreezeTag {
    players: [Player; PLAYERS],
    phase: Phase,
    score: [u8; 2],
    /// The rounds started after the match's first (a round replayed after a
    /// double wipe too) and the kit switches in their countdowns.
    lives: Lives,
    /// Each team's (the first's first) step it was down to its last one fighting
    /// ([`FreezeTag::last_standing`]'s team), while it is: set at the step it gets there, kept
    /// while it stays there, none otherwise (a thaw, his freeze, the round's end and restart).
    last_since: [Option<Time>; 2],
}

/// The bytes before the player records: version, phase, scores, the ended
/// round, a time (u32), the round starts (u16), each place's kit switches (u16).
const HEAD: usize = 9 + 2 + 2 * PLAYERS;
/// A team's down-to-one step: a tag (0 none, 1 a time) and the time (u32, 0 with none).
const SINCE: usize = 5;
const _: () = assert!(RECORD == 1 + 2 + 4);

impl FreezeTag {
    pub const EMPTY: Self = Self {
        players: [Player::Waiting; PLAYERS],
        phase: Phase::Playing,
        score: [0; 2],
        lives: Lives::NONE,
        last_since: [None; 2],
    };
    /// [`HEAD`], then twenty player records (tag, thaw progress u16,
    /// auto-thaw time left u32), then each team's down-to-one step ([`SINCE`]).
    pub const LEN: usize = Self::LEN_V3 + 2 * SINCE;
    /// Version 3's: [`HEAD`] and the records.
    pub const LEN_V3: usize = HEAD + PLAYERS * RECORD;
    /// 4: each team's down-to-one step (the last one fighting revealed 15 s after it, the owner,
    /// 2026-11-26). 3 (Artifact Hunt's restart: the phase's deadline, the countdown's kit
    /// switches) is read, with no team down to one yet; 2 (revision 71: a 5 s pause, then the
    /// restart) is not.
    const VERSION: u8 = 4;
    const VERSION_3: u8 = 3;
    pub fn player(&self, place: Place) -> Player {
        // A place is below PLAYERS; the crate reaches no panic.
        self.players.get(place.index()).copied().unwrap_or(Player::Waiting)
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn winner(&self) -> Option<Team> {
        match self.phase {
            Phase::Finished { winner, .. } => Some(winner),
            _ => None,
        }
    }
    pub fn complete(&self) -> bool {
        self.winner().is_some()
    }
    /// The team with more round wins, none while even: the statistics' leader,
    /// and the match's winner when the time limit ends it (the owner,
    /// 2026-11-26: "when the timer ends, the team with the most points wins").
    /// A match won at [`WINS`] has its winner as leader.
    pub fn leader(&self) -> Option<Team> {
        let [first, second] = self.score;
        match first.cmp(&second) {
            core::cmp::Ordering::Greater => Some(Team::First),
            core::cmp::Ordering::Less => Some(Team::Second),
            core::cmp::Ordering::Equal => None,
        }
    }
    /// When the match ended: the end of the time after its last round.
    pub fn finished_at(&self) -> Option<Time> {
        match self.phase {
            Phase::Finished { at, .. } => Some(at),
            _ => None,
        }
    }
    fn restart(&self) -> Option<RoundRestart> {
        match self.phase {
            Phase::Restart { restart, .. } => Some(restart),
            _ => None,
        }
    }
    /// Whether the game plays the match start's countdown ([`Restart::round_countdown`]).
    pub fn round_countdown(&self) -> bool {
        self.restart().is_some_and(|restart| restart.round_countdown())
    }
    /// What the respawn rule does at `now`, before this step's rule step ([`Restart::spawning`]).
    pub fn spawning(&self, now: Time) -> Spawning {
        self.restart().map_or(Spawning::Native, |restart| restart.spawning(now))
    }
    /// The places whose native respawn is void: a freeze replaces the death,
    /// so only a round's start spawns a frozen or lethal body (its death
    /// queued a respawn, which the hud would count down: the owner,
    /// 2026-11-22, "respawn in 8 seconds"). An entering place's spawn is
    /// never voided (the patch's `RespawnTick::void`).
    pub fn voided(&self, bodies: &[Body; PLAYERS]) -> Places {
        Place::all()
            .zip(self.players.iter().zip(bodies))
            .filter(|(_, (player, body))| {
                matches!(player, Player::Frozen { .. })
                    || matches!(body, Body::Present { condition: Condition::Lethal, .. })
            })
            .fold(Places::EMPTY, |voided, (place, _)| voided.with(place))
    }
    /// Patch 31 switched `place` into its chosen kit where it stands.
    pub fn kit_switched(&mut self, place: Place) {
        if let Some(restart) = self.restart() {
            self.lives.kit_switched(&restart, place.index());
        }
    }
    pub fn lives(&self) -> &Lives {
        &self.lives
    }
    /// Round wins, the first team's first.
    pub fn points(&self) -> [u16; 2] {
        self.score.map(u16::from)
    }
    /// How many rounds started after the match's first: every one a new life
    /// of each player in the match then.
    pub fn round_starts(&self) -> u16 {
        self.lives.round_starts()
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
    /// Whether nothing of the places from `places` on is in the state.
    pub fn fits(&self, places: usize) -> bool {
        self.players
            .get(places..)
            .is_some_and(|rest| rest.iter().all(|p| *p == Player::Waiting))
            && self
                .lives
                .every_kit_switch()
                .get(places..)
                .is_some_and(|rest| rest.iter().all(|count| *count == 0))
    }
    /// Whether `place` is his team's last one fighting in a round in play,
    /// with two teammates of his frozen at least: the other team sees him on
    /// its map (the owner, 2026-11-22: "last member is always shown on the
    /// map"). Only in a team of three or more: never in 1 v 1, 2 v 1 or 2 v 2
    /// (the owner: "when 1vs1 we see each other on the map, bad", "2vs1
    /// should also not show", and the two-player side's last one hidden too:
    /// "yes").
    pub fn last_standing(&self, place: Place) -> bool {
        matches!(self.player(place), Player::Fighting(team) if self.down_to_one(team))
    }
    /// Whether `team` is down to its last one fighting, two teammates of his frozen at least, in
    /// a round in play. A place that left is frozen (it stays so), so a teammate leaving takes the
    /// team down as his freeze does.
    fn down_to_one(&self, team: Team) -> bool {
        self.phase == Phase::Playing && self.living(team) == 1 && self.frozen(team) >= 2
    }
    /// `team`'s last one fighting ([`Self::last_standing`]) and the step his team was down to
    /// him. A thaw (no longer last) ends it; the next time the team is down to one counts afresh.
    pub fn last_fighter(&self, team: Team) -> Option<LastFighter> {
        let since = self.last_since[team.index()]?;
        let place = Place::all().find(|place| self.player(*place) == Player::Fighting(team))?;
        Some(LastFighter { place, since })
    }
    /// Whether `place` is shown on the other team's maps at `now`: his team's last one fighting,
    /// [`REVEAL`] after his team was down to him (the owner, 2026-11-26: "the last person should
    /// only be seen after 15 seconds, with a clear timer visible for everyone").
    pub fn revealed(&self, place: Place, now: Time) -> bool {
        match self.player(place) {
            Player::Fighting(team) => self.last_fighter(team).is_some_and(|last| last.revealed(now)),
            Player::Waiting | Player::Frozen { .. } => false,
        }
    }
    /// Each team's down-to-one step after a step at `now`: kept while it stays down to one, `now`
    /// when it gets there (from two fighting or more, a leave too, or a frozen arrival that makes
    /// two frozen), none otherwise.
    fn keep_last_since(&mut self, now: Time) {
        for team in [Team::First, Team::Second] {
            let since = self.down_to_one(team).then(|| self.last_since[team.index()].unwrap_or(now));
            self.last_since[team.index()] = since;
        }
    }
    fn living(&self, team: Team) -> usize {
        self.players
            .iter()
            .filter(|p| **p == Player::Fighting(team))
            .count()
    }
    fn frozen(&self, team: Team) -> usize {
        self.players
            .iter()
            .filter(|p| matches!(p, Player::Frozen { team: side, .. } if *side == team))
            .count()
    }

    /// One simulation step at `now`, `elapsed` after the last. All rescues
    /// use the same pre-thaw population: a player revived this tick cannot
    /// revive another until the next. `visible[a][b]` is the world's
    /// obstruction test between the bodies.
    /// Each team's down-to-one step follows ([`Self::last_fighter`]).
    pub fn step(
        &mut self,
        now: Time,
        bodies: &[Body; PLAYERS],
        visible: &[[bool; PLAYERS]; PLAYERS],
        elapsed: u16,
    ) -> Actions {
        let actions = self.play(now, bodies, visible, elapsed);
        self.keep_last_since(now);
        actions
    }
    fn play(
        &mut self,
        now: Time,
        bodies: &[Body; PLAYERS],
        visible: &[[bool; PLAYERS]; PLAYERS],
        elapsed: u16,
    ) -> Actions {
        match self.phase {
            Phase::Finished { .. } => return Actions::Wait,
            Phase::Restart { ended, restart } => match restart.step(now, TIMING, &mut self.lives, |()| ()) {
                Step::Stays(restart) => {
                    self.phase = Phase::Restart { ended, restart };
                    return Actions::Wait;
                }
                Step::Decided { .. } => {
                    self.phase = match ended {
                        RoundEnd::Won(winner) if self.points_of(winner) == u16::from(WINS) => {
                            Phase::Finished { winner, at: now }
                        }
                        _ => Phase::Restart {
                            ended,
                            restart: Restart::fade(now, ()),
                        },
                    };
                    return Actions::Wait;
                }
                Step::Started(restart) => {
                    // Everyone respawns at this step: the new round's players.
                    self.players = [Player::Waiting; PLAYERS];
                    self.phase = Phase::Restart { ended, restart };
                    return Actions::Wait;
                }
                Step::Next(()) => self.phase = Phase::Playing,
            },
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
                    *player = Player::frozen(team);
                }
                (Player::Waiting, Body::Present { team, .. }) => *player = Player::Fighting(team),
                // Leaving cannot erase a freeze. Nor can reconnecting turn
                // a retained frozen place back into a fighting player: he
                // comes back at his team's spawn, frozen there until a
                // teammate or the auto-thaw thaws him (the release manager's
                // pick, 2026-11-22: rejoining is no way out of a freeze).
                (Player::Fighting(team), Body::Absent) => {
                    *player = Player::frozen(team);
                }
                _ => {}
            }
        }
        let before = self.players;
        let mut thawed = Places::EMPTY;
        let mut thawers = Places::EMPTY;
        for (place, ((slot, body), sight)) in Place::all().zip(self.players.iter_mut().zip(bodies).zip(visible)) {
            let Player::Frozen { team, progress, thaws_in } = *slot else {
                continue;
            };
            // The fighting teammates within reach, in clear sight.
            let helpers = match *body {
                Body::Present { position, .. } => Place::all().zip(before.iter().zip(bodies).zip(sight)).fold(
                    Places::EMPTY,
                    |helpers, (helper, ((player, body), clear))| match *body {
                        Body::Present { position: other, .. } if *clear && *player == Player::Fighting(team) => {
                            let ([ax, ay, az], [bx, by, bz]) = (position, other);
                            let [x, y, z] = [ax - bx, ay - by, az - bz];
                            if x * x + y * y + z * z <= RADIUS * RADIUS { helpers.with(helper) } else { helpers }
                        }
                        _ => helpers,
                    },
                ),
                Body::Absent => Places::EMPTY,
            };
            let progress = if helpers.count() > 0 {
                progress.0.saturating_add(elapsed)
            } else {
                0
            };
            // The auto-thaw's clock runs on a body in the world: a place that
            // left stays frozen.
            let thaws_in = match *body {
                Body::Present { .. } => NonZeroU32::new(thaws_in.get().saturating_sub(u32::from(elapsed))),
                Body::Absent => Some(thaws_in),
            };
            match (progress >= THAW_MS, thaws_in) {
                (true, _) => {
                    *slot = Player::Fighting(team);
                    thawed = thawed.with(place);
                    thawers = Places::new(thawers.bits() | helpers.bits()).unwrap_or(thawers);
                }
                (false, None) => {
                    *slot = Player::Fighting(team);
                    thawed = thawed.with(place);
                }
                (false, Some(thaws_in)) => {
                    *slot = Player::Frozen {
                        team,
                        progress: Progress(progress),
                        thaws_in,
                    };
                }
            }
        }
        if let Some(ended) = self.result() {
            if let RoundEnd::Won(team) = ended {
                let score = self.score_mut(team);
                *score = score.saturating_add(1);
            }
            self.phase = Phase::Restart {
                ended,
                restart: Restart::decided(now, TIMING),
            };
        }
        Actions::Play { thawed, thawers }
    }
    fn entered(&self, side: Team) -> bool {
        self.players.iter().any(|p| match *p {
            Player::Fighting(team) | Player::Frozen { team, .. } => team == side,
            Player::Waiting => false,
        })
    }
    /// A round ends once a team that entered has nobody fighting (a place
    /// that left stays frozen): the other team wins it if it has someone
    /// fighting, else it is a draw and is replayed, a double wipe as a team
    /// frozen with nobody on the other side (the owner, 2026-11-22: a solo
    /// player stayed frozen; a draw scores nothing, so nobody farms a point
    /// alone). Before anybody entered nothing ends.
    fn result(&self) -> Option<RoundEnd> {
        let wiped = |team| self.entered(team) && self.living(team) == 0;
        match (wiped(Team::First), wiped(Team::Second)) {
            (false, false) => None,
            (true, false) if self.living(Team::Second) > 0 => Some(RoundEnd::Won(Team::Second)),
            (false, true) if self.living(Team::First) > 0 => Some(RoundEnd::Won(Team::First)),
            _ => Some(RoundEnd::Draw),
        }
    }

    /// Whether the round starts fit the rounds: every round that ended (a win
    /// in the scores, or a draw, which scores nothing) was restarted but the
    /// one whose fade has not ended and the match's last; so the draws they
    /// leave are none fewer than zero, and one at least in a draw's restart.
    fn lives_fit(&self) -> bool {
        let [first, second] = self.score;
        let wins = u16::from(first).saturating_add(u16::from(second));
        let (pending, draw_now) = match self.phase {
            Phase::Playing => (0, false),
            Phase::Restart { ended, restart } => (
                u16::from(!matches!(restart, Restart::Countdown { .. })),
                ended == RoundEnd::Draw,
            ),
            Phase::Finished { .. } => (1, false),
        };
        let Some(ended) = self.lives.round_starts().checked_add(pending) else { return false };
        match ended.checked_sub(wins) {
            Some(draws) => !draw_now || draws > 0,
            None => false,
        }
    }

    pub fn bytes(&self) -> [u8; Self::LEN] {
        let team = |team| match team {
            Team::First => 1,
            Team::Second => 2,
        };
        let (phase, ended, time) = match self.phase {
            Phase::Playing => (0, 0, 0),
            Phase::Restart { ended, restart } => (
                match restart {
                    Restart::Decided { .. } => 1,
                    Restart::Fade { .. } => 2,
                    Restart::Countdown { .. } => 3,
                },
                match ended {
                    RoundEnd::Won(winner) => team(winner),
                    RoundEnd::Draw => 3,
                },
                restart.until().ms(),
            ),
            Phase::Finished { winner, at } => (4, team(winner), at.ms()),
        };
        // Each field into its own place of the layout `read` takes: a field of the wrong width
        // does not compile.
        let mut bytes = [0; Self::LEN];
        let [version, phase_at, zero, one, ended_at, t0, t1, t2, t3, s0, s1, rest @ ..] = &mut bytes;
        [*version, *phase_at, *ended_at] = [Self::VERSION, phase, ended];
        [*zero, *one] = self.score;
        [*t0, *t1, *t2, *t3] = time.to_le_bytes();
        [*s0, *s1] = self.lives.round_starts().to_le_bytes();
        let (kits, rest) = rest.split_at_mut(2 * PLAYERS);
        let (records, since) = rest.split_at_mut(PLAYERS * RECORD);
        for (out, count) in kits.as_chunks_mut::<2>().0.iter_mut().zip(self.lives.every_kit_switch()) {
            *out = count.to_le_bytes();
        }
        for (out, player) in records.as_chunks_mut::<RECORD>().0.iter_mut().zip(&self.players) {
            let (tag, progress, thaws_in) = match *player {
                // The tags `read` takes: 0 waiting, 1/2 fighting, 3/4 frozen, by team.
                Player::Waiting => (0, 0, 0),
                Player::Fighting(Team::First) => (1, 0, 0),
                Player::Fighting(Team::Second) => (2, 0, 0),
                Player::Frozen { team: Team::First, progress, thaws_in } => (3, progress.0, thaws_in.get()),
                Player::Frozen { team: Team::Second, progress, thaws_in } => (4, progress.0, thaws_in.get()),
            };
            let [p0, p1] = progress.to_le_bytes();
            let [h0, h1, h2, h3] = thaws_in.to_le_bytes();
            *out = [tag, p0, p1, h0, h1, h2, h3];
        }
        // The tags `read` takes: 0 none, 1 a time.
        for (out, since) in since.as_chunks_mut::<SINCE>().0.iter_mut().zip(self.last_since) {
            let (tag, [t0, t1, t2, t3]) = match since {
                None => (0, [0; 4]),
                Some(time) => (1, time.ms().to_le_bytes()),
            };
            *out = [tag, t0, t1, t2, t3];
        }
        bytes
    }
    /// The rule at the end of a world's respawn block (`middle`), version 3 or 4, and where it
    /// starts. Version 3's length is tried first: a version 4 rule's byte there is its round
    /// starts' high byte, never 3 in a match (768 rounds), while version 4's place in a version 3
    /// world is in the respawn points.
    pub fn read_back(middle: &[u8]) -> Option<(usize, Self)> {
        [Self::LEN_V3, Self::LEN].into_iter().find_map(|len| {
            let at = middle.len().checked_sub(len)?;
            Some((at, Self::read(middle.get(at..)?)?))
        })
    }
    /// The length of a rule of `version` (its first byte): 4's or 3's.
    pub fn len_of(version: u8) -> Option<usize> {
        match version {
            Self::VERSION => Some(Self::LEN),
            Self::VERSION_3 => Some(Self::LEN_V3),
            _ => None,
        }
    }
    /// A rule's bytes, version 4 or 3 (its first byte and its length).
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let mut input = Reader::new(bytes, 0);
        let v4 = match (input.u8().ok()?, bytes.len()) {
            (Self::VERSION, Self::LEN) => true,
            (Self::VERSION_3, Self::LEN_V3) => false,
            _ => return None,
        };
        let phase = input.u8().ok()?;
        let score = [input.u8().ok()?, input.u8().ok()?];
        let [zero, one] = score;
        if zero > WINS || one > WINS || (zero == WINS && one == WINS) {
            return None;
        }
        let ended = input.u8().ok()?;
        let time = Time::from_ms(input.u32().ok()?);
        let round_starts = input.u16().ok()?;
        let mut kit_switches = [0; PLAYERS];
        for count in &mut kit_switches {
            *count = input.u16().ok()?;
        }
        let winner = |team| match team {
            1 => Some(Team::First),
            2 => Some(Team::Second),
            _ => None,
        };
        let mut state = Self::EMPTY;
        state.score = score;
        state.lives = Lives::new(round_starts, kit_switches);
        state.phase = match (phase, ended, time.ms()) {
            (0, 0, 0) => Phase::Playing,
            (1..=3, 1..=3, _) => Phase::Restart {
                ended: winner(ended).map_or(RoundEnd::Draw, RoundEnd::Won),
                restart: match phase {
                    1 => Restart::Decided { until: time },
                    2 => Restart::Fade { until: time, next: () },
                    _ => Restart::Countdown { until: time, next: () },
                },
            },
            (4, _, _) => Phase::Finished {
                winner: winner(ended)?,
                at: time,
            },
            _ => return None,
        };
        // A team at the goal: its last round's 3 s, or the match over.
        let at_goal = |team| state.points_of(team) == u16::from(WINS);
        let goal_fits = match state.phase {
            Phase::Finished { winner, .. } => at_goal(winner),
            Phase::Restart {
                ended: RoundEnd::Won(team),
                restart: Restart::Decided { .. },
            } => !at_goal(team.other()),
            _ => !at_goal(Team::First) && !at_goal(Team::Second),
        };
        if !goal_fits {
            return None;
        }
        for player in state.players.iter_mut() {
            let tag = input.u8().ok()?;
            let progress = input.u16().ok()?;
            let thaws_in = input.u32().ok()?;
            let frozen = |team| {
                Some(Player::Frozen {
                    team,
                    progress: Progress(progress),
                    thaws_in: NonZeroU32::new(thaws_in)?,
                })
            };
            *player = match (tag, progress, thaws_in) {
                (0, 0, 0) => Player::Waiting,
                (1, 0, 0) => Player::Fighting(Team::First),
                (2, 0, 0) => Player::Fighting(Team::Second),
                (3, p, t) if p < THAW_MS && t <= AUTO_THAW_MS.get() => frozen(Team::First)?,
                (4, p, t) if p < THAW_MS && t <= AUTO_THAW_MS.get() => frozen(Team::Second)?,
                _ => return None,
            };
        }
        // Each team's down-to-one step: exactly while it is down to one. Version 3 has none: a
        // team down to one gets its step at the next.
        if v4 {
            for team in [Team::First, Team::Second] {
                let tag = input.u8().ok()?;
                let time = input.u32().ok()?;
                state.last_since[team.index()] = match (tag, time) {
                    (0, 0) => None,
                    (1, ms) => Some(Time::from_ms(ms)),
                    _ => return None,
                };
                if state.last_since[team.index()].is_some() != state.down_to_one(team) {
                    return None;
                }
            }
        }
        // Nobody's state changes from the round's end to the fade's end,
        // when everyone waits for his new spawn.
        let valid = match state.phase {
            Phase::Playing => state.result().is_none(),
            Phase::Restart {
                restart: Restart::Countdown { .. },
                ..
            } => state.players.iter().all(|p| *p == Player::Waiting),
            Phase::Restart {
                ended: RoundEnd::Won(team),
                ..
            }
            | Phase::Finished { winner: team, .. } => {
                state.result() == Some(RoundEnd::Won(team)) && state.points_of(team) > 0
            }
            Phase::Restart {
                ended: RoundEnd::Draw,
                ..
            } => state.result() == Some(RoundEnd::Draw),
        };
        (valid && state.lives_fit()).then_some(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn thawed(actions: Actions) -> u32 {
        match actions {
            Actions::Play { thawed, .. } => thawed.bits(),
            other => panic!("{other:?}"),
        }
    }
    fn place(n: u8) -> Place {
        Place::new(n).unwrap()
    }
    const VISIBLE: [[bool; PLAYERS]; PLAYERS] = [[true; PLAYERS]; PLAYERS];
    const BLIND: [[bool; PLAYERS]; PLAYERS] = [[false; PLAYERS]; PLAYERS];
    const DECIDED: u16 = 3000;
    const FADE: u16 = 1000;
    const COUNTDOWN: u16 = 5000;
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
    /// The rule and the world's time.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Game {
        s: FreezeTag,
        now: u32,
    }
    impl Game {
        fn new() -> Self {
            Self { s: FreezeTag::EMPTY, now: 100_000 }
        }
        fn step(&mut self, bodies: &[Body; PLAYERS], visible: &[[bool; PLAYERS]; PLAYERS], elapsed: u16) -> Actions {
            self.now += u32::from(elapsed);
            self.s.step(Time::from_ms(self.now), bodies, visible, elapsed)
        }
        /// Steps of 50 ms for `ms`, walls between everyone (no thaw).
        fn run_blind(&mut self, bodies: &[Body; PLAYERS], ms: u32) {
            for _ in 0..ms / 50 {
                self.step(bodies, &BLIND, 50);
            }
        }
        /// Steps of 50 ms for `ms`.
        fn run(&mut self, bodies: &[Body; PLAYERS], ms: u16) {
            for _ in 0..ms / 50 {
                self.step(bodies, &VISIBLE, 50);
            }
        }
        fn read_back(&self) -> FreezeTag {
            FreezeTag::read(&self.s.bytes()).unwrap()
        }
    }
    #[test]
    fn proximity_requires_the_continuous_thaw_time_and_restores_after_rollback() {
        let b = match_bodies();
        let mut g = Game::new();
        assert_eq!(thawed(g.step(&b, &VISIBLE, THAW_MS - 50)), 0);
        let saved = g;
        assert_eq!(thawed(g.step(&b, &VISIBLE, 50)), 1);
        assert_eq!(g.s.player(place(0)), Player::Fighting(Team::First));
        g = Game { s: saved.read_back(), ..saved };
        let mut far = b;
        far[1] = body(Team::First, RADIUS + 0.01, Condition::Fighting);
        assert_eq!(thawed(g.step(&far, &VISIBLE, 50)), 0);
        assert_eq!(thawed(g.step(&b, &VISIBLE, THAW_MS - 50)), 0);
        assert_eq!(thawed(g.step(&b, &VISIBLE, 50)), 1);
    }
    #[test]
    fn enemies_frozen_helpers_and_walls_do_not_thaw() {
        let mut b = match_bodies();
        b[3] = body(Team::First, 20.0, Condition::Fighting);
        let mut g = Game::new();
        assert_eq!(thawed(g.step(&b, &BLIND, THAW_MS)), 0);
        b[1] = body(Team::First, 1.0, Condition::Lethal);
        assert_eq!(thawed(g.step(&b, &VISIBLE, THAW_MS)), 0);
        assert!(!g.s.complete());
    }
    #[test]
    fn team_wipe_is_terminal_simultaneous_wipe_and_a_team_alone_draw() {
        let mut b = match_bodies();
        let mut g = Game::new();
        g.step(&b, &VISIBLE, 50);
        b[1] = body(Team::First, 3.0, Condition::Lethal);
        g.step(&b, &VISIBLE, 50);
        assert!(matches!(
            g.s.phase(),
            Phase::Restart {
                ended: RoundEnd::Won(Team::Second),
                restart: Restart::Decided { .. },
            }
        ));
        assert_eq!(g.s.points(), [0, 1]);
        assert_eq!(g.step(&match_bodies(), &VISIBLE, 3000), Actions::Wait);
        b[2] = body(Team::Second, 1.0, Condition::Lethal);
        let mut g = Game::new();
        g.step(&b, &VISIBLE, 50);
        assert!(matches!(g.s.phase(), Phase::Restart { ended: RoundEnd::Draw, .. }));
        // A team alone (1 v 0): its wipe is a draw, replayed, no point.
        b[2] = Body::Absent;
        let mut g = Game::new();
        g.step(&b, &VISIBLE, 50);
        assert!(matches!(g.s.phase(), Phase::Restart { ended: RoundEnd::Draw, .. }));
        assert_eq!(g.s.points(), [0, 0]);
        assert_eq!(g.read_back(), g.s);
        // Nobody at all: nothing ends.
        let mut g = Game::new();
        g.step(&[Body::Absent; PLAYERS], &VISIBLE, 50);
        assert_eq!(g.s.phase(), Phase::Playing);
    }
    /// An arrival in a round in play spawns frozen (lethal), on an empty team
    /// too: that team has entered with nobody fighting, so the other team wins
    /// the round (the owner, 2026-11-26: as intended).
    #[test]
    fn an_arrival_on_an_empty_team_is_frozen_and_loses_the_round() {
        let mut b = [Body::Absent; PLAYERS];
        b[0] = body(Team::First, 0.0, Condition::Fighting);
        let mut g = Game::new();
        g.step(&b, &VISIBLE, 50);
        assert_eq!(g.s.phase(), Phase::Playing, "1 v 0: nothing ends");
        b[1] = body(Team::Second, 10.0, Condition::Lethal);
        g.step(&b, &VISIBLE, 50);
        assert!(matches!(g.s.phase(), Phase::Restart { ended: RoundEnd::Won(Team::First), .. }));
        assert_eq!((g.s.points(), g.s.leader()), ([1, 0], Some(Team::First)));
    }
    /// The leader is by round wins, even none.
    #[test]
    fn the_leader_has_more_round_wins() {
        let mut g = Game::new();
        assert_eq!(g.s.leader(), None);
        let mut b = match_bodies();
        b[2] = body(Team::Second, 1.0, Condition::Lethal);
        g.step(&b, &VISIBLE, 50);
        assert_eq!(g.s.leader(), Some(Team::First));
        g.s.score = [2, 3];
        assert_eq!(g.s.leader(), Some(Team::Second));
        g.s.score = [3, 3];
        assert_eq!(g.s.leader(), None);
    }
    /// Every field non-zero and of its own value: `read(bytes())` gives the state back, and each
    /// field is at its place in the layout (since #2507 the writer had moved every field after
    /// the first a byte on per field before it, and only the round trips of whole states showed it).
    #[test]
    fn every_field_is_written_at_its_place() {
        let mut players = [Player::Waiting; PLAYERS];
        for (n, player) in players.iter_mut().enumerate() {
            let n16 = u16::try_from(n).unwrap();
            *player = match n {
                0..=9 => Player::Frozen {
                    team: Team::Second,
                    progress: Progress(100 + n16),
                    thaws_in: NonZeroU32::new(1000 + 7 * u32::from(n16)).unwrap(),
                },
                10..=17 => Player::Frozen {
                    team: Team::First,
                    progress: Progress(200 + n16),
                    thaws_in: NonZeroU32::new(5000 + 11 * u32::from(n16)).unwrap(),
                },
                18 => Player::Fighting(Team::First),
                _ => Player::Waiting,
            };
        }
        let kit_switches: [u16; PLAYERS] = core::array::from_fn(|n| 0x1001 + 0x0101 * u16::try_from(n).unwrap());
        let state = FreezeTag {
            players,
            phase: Phase::Restart {
                ended: RoundEnd::Won(Team::First),
                restart: Restart::Fade { until: Time::from_ms(0x0A0B_0C0D), next: () },
            },
            score: [3, 2],
            lives: Lives::new(0x0307, kit_switches),
            last_since: [None; 2],
        };
        let bytes = state.bytes();
        assert_eq!(FreezeTag::read(&bytes), Some(state));
        // The head: version (4), phase (the fade), the scores, the ended round (a win of the first
        // team), the fade's end, the round starts, each place's kit switches.
        assert_eq!(bytes[..11], [4, 2, 3, 2, 1, 0x0D, 0x0C, 0x0B, 0x0A, 0x07, 0x03]);
        assert_eq!(bytes[11..13], [0x01, 0x10]);
        assert_eq!(bytes[HEAD - 2..HEAD], kit_switches[PLAYERS - 1].to_le_bytes());
        // The records: tag, progress, thaws_in, from HEAD.
        let record = |n: usize| &bytes[HEAD + n * RECORD..HEAD + (n + 1) * RECORD];
        assert_eq!(record(0), [4, 100, 0, 0xE8, 0x03, 0, 0]);
        assert_eq!(record(17), [3, 217, 0, 0x43, 0x14, 0, 0]);
        assert_eq!(record(18), [1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(record(19), [0; RECORD]);
        assert_eq!(HEAD + PLAYERS * RECORD, FreezeTag::LEN_V3);
        // Each team's down-to-one step: none, none.
        assert_eq!(bytes[FreezeTag::LEN_V3..], [0; 2 * SINCE]);
        assert_eq!(FreezeTag::LEN_V3 + 2 * SINCE, FreezeTag::LEN);
    }

    #[test]
    fn wire_domains_and_roster_are_checked() {
        let mut g = Game::new();
        g.step(&match_bodies(), &VISIBLE, 1500);
        let s = g.s;
        assert_eq!(FreezeTag::read(&s.bytes()), Some(s));
        assert!(s.fits(3));
        assert!(!s.fits(2));
        let record = |n: usize| HEAD + n * RECORD;
        let mut bytes = s.bytes();
        bytes[record(0) + 1..record(0) + 3].copy_from_slice(&THAW_MS.to_le_bytes());
        assert!(FreezeTag::read(&bytes).is_none());
        bytes = s.bytes();
        bytes[record(0)] = 5;
        assert!(FreezeTag::read(&bytes).is_none());
        bytes = s.bytes();
        bytes[1] = 5;
        assert!(FreezeTag::read(&bytes).is_none());
        // Revision 71's version 2 is not read.
        bytes = s.bytes();
        bytes[0] = 2;
        assert!(FreezeTag::read(&bytes).is_none());
        // Playing has no time.
        bytes = s.bytes();
        bytes[5] = 1;
        assert!(FreezeTag::read(&bytes).is_none());
        // A kit switch past the match's places does not fit.
        bytes = s.bytes();
        bytes[11 + 2 * 3] = 1;
        assert!(FreezeTag::read(&bytes).is_some_and(|s| s.fits(4) && !s.fits(3)));
        // The auto-thaw time: 1 ms to AUTO_THAW_MS, a frozen record's only.
        let thaws_in = |bytes: &mut [u8; FreezeTag::LEN], at: usize, ms: u32| {
            bytes[at + 3..at + 7].copy_from_slice(&ms.to_le_bytes());
        };
        for (ms, ok) in [(1, true), (AUTO_THAW_MS.get(), true), (0, false), (AUTO_THAW_MS.get() + 1, false)] {
            let mut bytes = s.bytes();
            thaws_in(&mut bytes, record(0), ms);
            assert_eq!(FreezeTag::read(&bytes).is_some(), ok, "{ms}");
        }
        let mut fighting = s.bytes();
        thaws_in(&mut fighting, record(1), 1);
        assert!(FreezeTag::read(&fighting).is_none());
        assert!(FreezeTag::read(&bytes[..bytes.len() - 1]).is_none());
    }

    /// Fighting bodies of both teams, team 0 at x 0 and team 1 at x 10.
    fn fighting() -> [Body; PLAYERS] {
        let mut b = [Body::Absent; PLAYERS];
        b[0] = body(Team::First, 0.0, Condition::Fighting);
        b[1] = body(Team::Second, 10.0, Condition::Fighting);
        b
    }
    /// One round played to its end (`frozen` the places frozen at it) and,
    /// unless it ends the match, restarted and opened.
    fn round(g: &mut Game, frozen: &[usize]) {
        let mut b = fighting();
        g.step(&b, &VISIBLE, 50);
        for &i in frozen {
            if let Body::Present { condition, .. } = &mut b[i] {
                *condition = Condition::Lethal;
            }
        }
        g.step(&b, &VISIBLE, 50);
        assert_eq!(g.read_back(), g.s);
        g.run(&b, DECIDED + FADE);
        if g.s.complete() {
            return;
        }
        assert!(matches!(g.s.phase(), Phase::Restart { restart: Restart::Countdown { .. }, .. }));
        assert_eq!(g.read_back(), g.s);
        g.run(&fighting(), COUNTDOWN);
        assert_eq!(g.s.phase(), Phase::Playing);
    }

    #[test]
    fn a_round_ends_and_starts_as_artifact_hunts() {
        let mut b = fighting();
        let mut g = Game::new();
        g.step(&b, &VISIBLE, 50);
        b[1] = body(Team::Second, 10.0, Condition::Lethal);
        g.step(&b, &VISIBLE, 50);
        let decided = g.now;
        // 1. 3 s: play goes on, the frozen stay so, nobody respawns.
        assert_eq!(
            g.s.phase(),
            Phase::Restart {
                ended: RoundEnd::Won(Team::First),
                restart: Restart::Decided { until: Time::from_ms(decided + 3000) },
            }
        );
        assert!(!g.s.round_countdown());
        let frozen = g.s.player(place(1));
        while g.now < decided + 3000 - 50 {
            assert_eq!(g.s.spawning(Time::from_ms(g.now + 50)), Spawning::Held);
            assert_eq!(g.step(&b, &VISIBLE, 50), Actions::Wait);
        }
        assert_eq!(g.s.player(place(1)), frozen, "no thaw, no auto-thaw clock");
        // 3. the 1 s fade: held; at its end everyone respawns.
        g.step(&b, &VISIBLE, 50);
        assert!(matches!(g.s.phase(), Phase::Restart { restart: Restart::Fade { .. }, .. }));
        assert!(g.s.round_countdown());
        while g.now < decided + 4000 - 50 {
            assert_eq!(g.s.spawning(Time::from_ms(g.now + 50)), Spawning::Held);
            g.step(&b, &VISIBLE, 50);
        }
        assert_eq!(g.s.spawning(Time::from_ms(g.now + 50)), Spawning::Everyone);
        g.step(&b, &VISIBLE, 50);
        // 4. the 5 s countdown: a round start, every player waits for his spawn.
        assert!(matches!(g.s.phase(), Phase::Restart { restart: Restart::Countdown { .. }, .. }));
        assert_eq!((g.s.round_starts(), g.s.player(place(1))), (1, Player::Waiting));
        assert_eq!(g.s.spawning(Time::from_ms(g.now + 50)), Spawning::Native);
        g.s.kit_switched(place(0));
        assert_eq!(g.s.lives().kit_switches(0), 1);
        assert_eq!(g.read_back(), g.s);
        g.run(&fighting(), COUNTDOWN - 50);
        assert!(g.s.round_countdown());
        // 5. the next round.
        g.step(&fighting(), &VISIBLE, 50);
        assert_eq!(g.s.phase(), Phase::Playing);
        assert_eq!(g.s.player(place(1)), Player::Fighting(Team::Second));
        assert!(!g.s.round_countdown());
        // No kit switch counts in play.
        g.s.kit_switched(place(0));
        assert_eq!(g.s.lives().kit_switches(0), 1);
    }

    #[test]
    fn the_match_point_ends_the_match_after_the_3_s() {
        let mut g = Game::new();
        for _ in 0..WINS - 1 {
            round(&mut g, &[1]);
        }
        let mut b = fighting();
        g.step(&b, &VISIBLE, 50);
        b[1] = body(Team::Second, 10.0, Condition::Lethal);
        g.step(&b, &VISIBLE, 50);
        assert_eq!(g.s.points(), [u16::from(WINS), 0]);
        assert!(!g.s.complete());
        assert_eq!(g.read_back(), g.s);
        let decided = g.now;
        g.run(&b, DECIDED);
        assert_eq!(g.s.phase(), Phase::Finished { winner: Team::First, at: Time::from_ms(decided + 3000) });
        assert_eq!(g.read_back(), g.s);
        let over = g.s;
        assert_eq!(g.step(&b, &VISIBLE, 6000), Actions::Wait);
        assert_eq!(g.s, over);
        assert_eq!(g.s.spawning(Time::from_ms(g.now)), Spawning::Native);
        assert!(!g.s.round_countdown());
    }

    #[test]
    fn best_of_nine_finishes_at_five() {
        use Team::{First, Second};
        for winners in [[First; WINS as usize].as_slice(), &[First, Second, Second, First, Second, First, First, Second, Second]] {
            let mut g = Game::new();
            for &winner in winners {
                round(&mut g, &[1 - winner.index()]);
            }
            assert_eq!(g.s.winner(), Some(*winners.last().unwrap()));
            assert_eq!(g.s.round_starts(), winners.len() as u16 - 1);
        }
    }

    #[test]
    fn every_restart_counts_a_round_start_the_double_wipe_replay_too() {
        let mut g = Game::new();
        assert_eq!(g.s.round_starts(), 0);
        round(&mut g, &[1]);
        assert_eq!((g.s.points(), g.s.round_starts()), ([1, 0], 1));
        // A double wipe scores nothing; its replay is a round start.
        round(&mut g, &[0, 1]);
        assert_eq!((g.s.points(), g.s.round_starts()), ([1, 0], 2));
        round(&mut g, &[0]);
        round(&mut g, &[1]);
        assert_eq!((g.s.points(), g.s.round_starts()), ([2, 1], 4));
        round(&mut g, &[1]);
        round(&mut g, &[1]);
        assert_eq!((g.s.points(), g.s.round_starts()), ([4, 1], 6));
        // The match's last round is never restarted.
        round(&mut g, &[1]);
        assert_eq!(g.s.winner(), Some(Team::First));
        assert_eq!(g.s.round_starts(), 6);
        // A rollback restores the count with the rest.
        assert_eq!(g.read_back(), g.s);
    }

    #[test]
    fn the_round_starts_are_no_fewer_than_the_rounds_ended_before() {
        let starts = |s: &FreezeTag, n: u16| {
            let mut bytes = s.bytes();
            bytes[9..11].copy_from_slice(&n.to_le_bytes());
            FreezeTag::read(&bytes)
        };
        let mut g = Game::new();
        round(&mut g, &[1]);
        round(&mut g, &[1]);
        // Playing at 2:0: two starts at least; more are replayed draws.
        assert!(starts(&g.s, 1).is_none());
        assert!(starts(&g.s, 2).is_some() && starts(&g.s, 9).is_some());
        // After a win, before the fade's end: that win is not restarted yet.
        let mut b = fighting();
        g.step(&b, &VISIBLE, 50);
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        g.step(&b, &VISIBLE, 50);
        assert_eq!(g.s.points(), [2, 1]);
        assert!(starts(&g.s, 1).is_none() && starts(&g.s, 2).is_some());
        // In the countdown: this round's start is counted already.
        g.run(&b, DECIDED + FADE);
        assert!(matches!(g.s.phase(), Phase::Restart { restart: Restart::Countdown { .. }, .. }));
        assert!(starts(&g.s, 2).is_none() && starts(&g.s, 3).is_some());
        // After a double wipe at 1:0: the win restarted, and this draw to come.
        let mut g = Game::new();
        round(&mut g, &[1]);
        let mut b = fighting();
        g.step(&b, &VISIBLE, 50);
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        b[1] = body(Team::Second, 10.0, Condition::Lethal);
        g.step(&b, &VISIBLE, 50);
        assert!(matches!(g.s.phase(), Phase::Restart { ended: RoundEnd::Draw, .. }));
        assert!(starts(&g.s, 0).is_none() && starts(&g.s, 1).is_some());
        assert!(starts(&FreezeTag::EMPTY, u16::MAX).is_some());
    }

    #[test]
    fn the_frozen_and_the_lethal_respawn_only_with_the_round() {
        let mut bodies = match_bodies();
        let mut g = Game::new();
        assert_eq!(g.s.voided(&bodies), Places::EMPTY.with(place(0)));
        g.step(&bodies, &BLIND, 50);
        assert_eq!(g.s.voided(&bodies), Places::EMPTY.with(place(0)));
        // Left: still frozen, still void.
        bodies[0] = Body::Absent;
        assert_eq!(g.s.voided(&bodies), Places::EMPTY.with(place(0)));
        // A body killed in the 3 s after a round is held as the frozen are.
        let mut b = fighting();
        b[2] = body(Team::First, 5.0, Condition::Fighting);
        let mut g = Game::new();
        g.step(&b, &BLIND, 50);
        b[1] = body(Team::Second, 10.0, Condition::Lethal);
        g.step(&b, &BLIND, 50);
        b[2] = body(Team::First, 5.0, Condition::Lethal);
        assert_eq!(g.s.voided(&b), Places::EMPTY.with(place(1)).with(place(2)));
        assert_eq!(g.step(&b, &BLIND, 50), Actions::Wait);
        assert_eq!(g.s.player(place(2)), Player::Fighting(Team::First));
    }

    #[test]
    fn draws_replay_the_round_and_disconnects_cannot_clear_a_freeze() {
        let mut bodies = match_bodies();
        let mut g = Game::new();
        g.step(&bodies, &VISIBLE, 50);
        bodies[1] = Body::Absent;
        g.step(&bodies, &VISIBLE, 50);
        assert_eq!(g.s.points(), [0, 1]);
        for b in bodies.iter_mut().take(3) {
            *b = body(Team::First, 0.0, Condition::Lethal);
        }
        bodies[2] = body(Team::Second, 0.0, Condition::Lethal);
        let mut g = Game::new();
        g.step(&bodies, &VISIBLE, 50);
        assert_eq!(g.s.points(), [0, 0]);
        g.run(&bodies, DECIDED + FADE);
        assert_eq!(g.s.round_starts(), 1);
        assert_eq!(g.read_back(), g.s);
    }

    #[test]
    fn a_frozen_body_thaws_by_itself_after_two_minutes_crediting_nobody() {
        let b = match_bodies();
        let mut g = Game::new();
        let mut left = AUTO_THAW_MS.get();
        while left > 50 {
            let step = u16::try_from((left - 50).min(60_000)).unwrap();
            assert_eq!(g.step(&b, &BLIND, step), Actions::Play { thawed: Places::EMPTY, thawers: Places::EMPTY });
            left -= u32::from(step);
        }
        assert!(matches!(g.s.player(place(0)), Player::Frozen { thaws_in, .. } if thaws_in.get() == 50));
        assert_eq!(g.read_back(), g.s);
        let saved = g;
        let thawed = Places::EMPTY.with(place(0));
        assert_eq!(g.step(&b, &BLIND, 50), Actions::Play { thawed, thawers: Places::EMPTY });
        assert_eq!(g.s.player(place(0)), Player::Fighting(Team::First));
        // A teammate beside him then has not thawed him: no credit.
        let mut g = saved;
        let Actions::Play { thawers, .. } = g.step(&b, &VISIBLE, 50) else { panic!() };
        assert_eq!(thawers, Places::EMPTY);
        // A place that left keeps its freeze and its time.
        let mut g = saved;
        let mut gone = b;
        gone[0] = Body::Absent;
        g.step(&gone, &BLIND, 50);
        assert_eq!(g.s.player(place(0)), saved.s.player(place(0)));
    }

    #[test]
    fn a_team_of_three_shows_its_last_one_fighting_in_play_only() {
        // 3 v 1: the first team 0, 1 and 3; the second 2 alone.
        let mut b = [Body::Absent; PLAYERS];
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        b[1] = body(Team::First, 10.0, Condition::Fighting);
        b[2] = body(Team::Second, 20.0, Condition::Fighting);
        b[3] = body(Team::First, 30.0, Condition::Fighting);
        let mut g = Game::new();
        g.step(&b, &BLIND, 50);
        // One frozen, two fighting: nobody.
        assert!(!(0..4).any(|n| g.s.last_standing(place(n))));
        b[3] = body(Team::First, 30.0, Condition::Lethal);
        g.step(&b, &BLIND, 50);
        // Two frozen: their last one is shown; the lone one never.
        assert!(g.s.last_standing(place(1)));
        assert!(![0, 2, 3].into_iter().any(|n| g.s.last_standing(place(n))));
        // The round over: nobody.
        b[1] = body(Team::First, 10.0, Condition::Lethal);
        g.step(&b, &BLIND, 50);
        assert!(!(0..4).any(|n| g.s.last_standing(place(n))));
    }

    /// The first team 0, 1 and 3 fighting; the second 2 alone.
    fn three_v_one() -> [Body; PLAYERS] {
        let mut b = [Body::Absent; PLAYERS];
        b[0] = body(Team::First, 0.0, Condition::Fighting);
        b[1] = body(Team::First, 10.0, Condition::Fighting);
        b[2] = body(Team::Second, 20.0, Condition::Fighting);
        b[3] = body(Team::First, 30.0, Condition::Fighting);
        b
    }

    #[test]
    fn the_last_one_fighting_is_revealed_fifteen_seconds_after_his_team_is_down_to_him() {
        let mut b = three_v_one();
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        let mut g = Game::new();
        g.step(&b, &BLIND, 50);
        // One frozen, two fighting: nobody's count.
        assert_eq!(g.s.last_fighter(Team::First), None);
        g.run_blind(&b, 1000);
        // 3 frozen: 1 is the last one, from that step.
        b[3] = body(Team::First, 30.0, Condition::Lethal);
        g.step(&b, &BLIND, 50);
        let down = Time::from_ms(g.now);
        let last = LastFighter { place: place(1), since: down };
        assert_eq!(g.s.last_fighter(Team::First), Some(last));
        assert_eq!(g.read_back(), g.s, "the step kept in the world");
        assert_eq!(g.s.last_fighter(Team::Second), None);
        // Not before 15 s, from 15 s; the step kept while it stays so.
        g.run_blind(&b, 14_950);
        assert!(!g.s.revealed(place(1), Time::from_ms(g.now)));
        g.step(&b, &BLIND, 50);
        assert!(g.s.revealed(place(1), Time::from_ms(g.now)));
        assert_eq!(g.s.last_fighter(Team::First), Some(last));
        assert!(![0, 2, 3].into_iter().any(|n| g.s.revealed(place(n), Time::from_ms(g.now))));
        // He thaws 0 (2 m, 4 s): no longer last, no count.
        b[1] = body(Team::First, 1.0, Condition::Fighting);
        b[0] = body(Team::First, 0.0, Condition::Fighting);
        g.run(&b, THAW_MS);
        assert_eq!(g.s.player(place(0)), Player::Fighting(Team::First));
        assert_eq!(g.s.last_fighter(Team::First), None);
        assert!(!g.s.revealed(place(1), Time::from_ms(g.now)));
        // 0 frozen again: the count starts afresh.
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        g.step(&b, &BLIND, 50);
        assert_eq!(g.s.last_fighter(Team::First), Some(LastFighter { place: place(1), since: Time::from_ms(g.now) }));
        assert!(!g.s.revealed(place(1), Time::from_ms(g.now)));
        // He is frozen: the round over, nobody's count.
        b[1] = body(Team::First, 1.0, Condition::Lethal);
        g.step(&b, &BLIND, 50);
        assert!(matches!(g.s.phase(), Phase::Restart { .. }));
        assert_eq!(g.s.last_fighter(Team::First), None);
        assert_eq!(g.read_back(), g.s);
    }

    #[test]
    fn a_teammate_leaving_takes_the_team_down_to_one_and_a_frozen_arrival_counts_from_its_step() {
        // 0 frozen; 3 leaves: no longer fighting (his place stays frozen), so 1 is the last one
        // from the step he left.
        let mut b = three_v_one();
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        let mut g = Game::new();
        g.step(&b, &BLIND, 50);
        b[3] = Body::Absent;
        g.step(&b, &BLIND, 50);
        assert_eq!(g.s.last_fighter(Team::First), Some(LastFighter { place: place(1), since: Time::from_ms(g.now) }));
        // He comes back (frozen at his team's points): the count goes on from the leave.
        let left_at = Time::from_ms(g.now);
        g.run_blind(&b, 5000);
        b[3] = body(Team::First, 30.0, Condition::Lethal);
        g.step(&b, &BLIND, 50);
        assert_eq!(g.s.last_fighter(Team::First).map(|last| last.since), Some(left_at));
        // A team of two down to one (2 v 1 no count), then a late arrival frozen: two frozen now,
        // the count from that step, so its whole 15 s are seen.
        let mut b = [Body::Absent; PLAYERS];
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        b[1] = body(Team::First, 10.0, Condition::Fighting);
        b[2] = body(Team::Second, 20.0, Condition::Fighting);
        let mut g = Game::new();
        g.step(&b, &BLIND, 50);
        assert_eq!(g.s.last_fighter(Team::First), None);
        g.run_blind(&b, 20_000);
        b[3] = body(Team::First, 30.0, Condition::Lethal);
        g.step(&b, &BLIND, 50);
        assert_eq!(g.s.last_fighter(Team::First), Some(LastFighter { place: place(1), since: Time::from_ms(g.now) }));
        assert!(!g.s.revealed(place(1), Time::from_ms(g.now)));
    }

    #[test]
    fn a_version_3_rule_reads_with_no_count_which_starts_at_the_next_step() {
        let mut b = three_v_one();
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        b[3] = body(Team::First, 30.0, Condition::Lethal);
        let mut g = Game::new();
        g.step(&b, &BLIND, 50);
        assert!(g.s.last_fighter(Team::First).is_some());
        let mut v3 = g.s.bytes()[..FreezeTag::LEN_V3].to_vec();
        v3[0] = 3;
        let read = FreezeTag::read(&v3).unwrap();
        assert_eq!(read.last_fighter(Team::First), None);
        assert!(read.last_standing(place(1)));
        let mut g = Game { s: read, ..g };
        g.step(&b, &BLIND, 50);
        assert_eq!(g.s.last_fighter(Team::First), Some(LastFighter { place: place(1), since: Time::from_ms(g.now) }));
        // A version 3 rule's length with version 4, and the reverse, are no rule.
        let mut wrong = v3.clone();
        wrong[0] = 4;
        assert!(FreezeTag::read(&wrong).is_none());
        let mut wrong = g.s.bytes();
        wrong[0] = 3;
        assert!(FreezeTag::read(&wrong).is_none());
        // read_back finds each at the end of a respawn block.
        let mut middle = std::vec![7u8; 13];
        middle.extend_from_slice(&v3);
        assert_eq!(FreezeTag::read_back(&middle).map(|(at, _)| at), Some(13));
        let mut middle = std::vec![7u8; 13];
        middle.extend_from_slice(&g.s.bytes());
        assert_eq!(FreezeTag::read_back(&middle), Some((13, g.s)));
    }

    #[test]
    fn a_count_is_read_exactly_while_its_team_is_down_to_one() {
        let mut b = three_v_one();
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        b[3] = body(Team::First, 30.0, Condition::Lethal);
        let mut g = Game::new();
        g.step(&b, &BLIND, 50);
        let since = FreezeTag::LEN - 2 * SINCE;
        let bytes = g.s.bytes();
        assert_eq!(bytes[since], 1);
        assert_eq!(&bytes[since + 1..since + 5], &g.now.to_le_bytes());
        // None for the team down to one, or one for the other team, or a bad tag: no rule.
        for (at, value) in [(since, 0), (since + SINCE, 1), (since, 2)] {
            let mut wrong = bytes;
            wrong[at] = value;
            if value == 0 {
                wrong[since + 1..since + 5].fill(0);
            }
            assert!(FreezeTag::read(&wrong).is_none(), "{at} {value}");
        }
        // No time with no count.
        let mut wrong = bytes;
        wrong[since + SINCE + 1] = 1;
        assert!(FreezeTag::read(&wrong).is_none());
    }

    #[test]
    fn two_v_one_shows_nobody_whoever_is_frozen() {
        for frozen in [0, 1] {
            let mut b = [Body::Absent; PLAYERS];
            b[0] = body(Team::First, 0.0, Condition::Fighting);
            b[1] = body(Team::First, 10.0, Condition::Fighting);
            b[2] = body(Team::Second, 20.0, Condition::Fighting);
            b[frozen] = body(Team::First, 10.0 * frozen as f32, Condition::Lethal);
            let mut g = Game::new();
            g.step(&b, &BLIND, 50);
            assert!(matches!(g.s.phase(), Phase::Playing));
            assert!(!(0..3).any(|n| g.s.last_standing(place(n))), "{frozen}");
        }
    }

    #[test]
    fn two_v_two_shows_nobody() {
        let mut b = [Body::Absent; PLAYERS];
        b[0] = body(Team::First, 0.0, Condition::Lethal);
        b[1] = body(Team::First, 10.0, Condition::Fighting);
        b[2] = body(Team::Second, 20.0, Condition::Fighting);
        b[3] = body(Team::Second, 30.0, Condition::Lethal);
        let mut g = Game::new();
        g.step(&b, &BLIND, 50);
        assert!(matches!(g.s.phase(), Phase::Playing));
        assert!(!(0..4).any(|n| g.s.last_standing(place(n))));
    }

    #[test]
    fn a_team_of_one_is_never_last_standing() {
        let mut g = Game::new();
        g.step(&fighting(), &BLIND, 50);
        // 1 v 1, a round in play: nobody shown.
        assert!(matches!(g.s.phase(), Phase::Playing));
        assert!(!g.s.last_standing(place(0)) && !g.s.last_standing(place(1)));
    }
}
