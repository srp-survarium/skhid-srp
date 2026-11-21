//! The mode's parameters: a mode config's raw values parsed once into what
//! the rule plays with. Never serialized: every game reads the same config.

use crate::geometry::Position;
use crate::roster::Team;
use crate::time::Ms;

/// The most places an artefact can appear at on a map (the wire names one in
/// a byte; 0.46c2's maps have a handful).
pub const MAX_SPAWNERS: usize = 64;

/// `mode.parameters` as the config has them: whole seconds (0.46c2 keeps
/// each x 1000), `score` a count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawParams {
    pub match_time: u32,
    pub time_needed_to_continue_match: u32,
    pub draw_additional_time: u32,
    pub time_before_new_round: u32,
    pub round_countdown_time: u32,
    pub score: u32,
    pub respawn_time: u32,
    pub time_needed_to_respawn: u32,
    pub vi_spawn_time: u32,
    pub vi_spawn_variation: u32,
    pub vi_not_picked_ttl: u32,
    pub vi_dropped_ttl: u32,
    /// How far over the ground a dropped artefact lies, m (the item's
    /// `collision_shape.position` y: 0.13).
    pub drop_lift: f32,
}

/// 0.46c2's transition to a new round: the screen fades for a constant
/// second (0x740000), in no config.
pub const FADE: Ms = match Ms::new(1000) {
    Some(ms) => ms,
    None => Ms::ZERO,
};

/// A place the artefact appears at: `mode.artefact_hunt.spawners[]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawner {
    pub position: [f32; 3],
}

/// A team's base: `mode.artefact_hunt.bases[]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BaseDef {
    pub team: u32,
    pub position: [f32; 3],
}

/// Why a config cannot be played: the key at fault.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamError {
    MatchTime,
    TimeNeededToContinueMatch,
    DrawAdditionalTime,
    TimeBeforeNewRound,
    RoundCountdownTime,
    Score,
    RespawnTime,
    TimeNeededToRespawn,
    ViSpawnTime,
    ViNotPickedTtl,
    ViDroppedTtl,
    DropLift,
    /// None, more than [`MAX_SPAWNERS`], or a position that is no point of a map.
    Spawners,
    /// Not exactly two, one a team, at least a metre apart.
    Bases,
}

/// The parameters the rule plays with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    pub(crate) match_time: Ms,
    /// A delivery made when more than this of the match time has passed
    /// (less than `time_needed_to_continue_match` left) starts no new round
    /// of the regular time: `match_time` less `time_needed_to_continue_match`.
    pub(crate) rounds_until: Ms,
    pub(crate) sudden_death: Ms,
    pub(crate) before_new_round: Ms,
    pub(crate) round_countdown: Ms,
    /// 1..=255.
    pub(crate) goal: u8,
    /// Not 0.
    pub(crate) respawn_wave: Ms,
    pub(crate) respawn_least: Ms,
    /// The artefact's wait is `wait_least` plus a draw below `wait_range`.
    pub(crate) wait_least: Ms,
    pub(crate) wait_range: u32,
    pub(crate) not_picked_ttl: Ms,
    pub(crate) dropped_ttl: Ms,
    pub(crate) drop_lift: i32,
    /// 1..=MAX_SPAWNERS.
    pub(crate) spawner_count: u8,
    pub(crate) spawners: [Position; MAX_SPAWNERS],
    /// By team.
    pub(crate) bases: [Position; 2],
}

fn seconds(value: u32, most: u32, error: ParamError) -> Result<Ms, ParamError> {
    if value > most {
        return Err(error);
    }
    Ms::from_seconds(value).ok_or(error)
}

/// A day, an hour and ten minutes, in seconds.
const DAY: u32 = 86_400;
const HOUR: u32 = 3_600;
const TEN_MINUTES: u32 = 600;

impl Params {
    /// Parses a mode config's values; the key at fault otherwise.
    pub fn parse(
        raw: &RawParams,
        spawners: &[Spawner],
        bases: &[BaseDef],
    ) -> Result<Params, ParamError> {
        let match_time = seconds(raw.match_time, DAY, ParamError::MatchTime)?;
        if match_time == Ms::ZERO {
            return Err(ParamError::MatchTime);
        }
        let needed = seconds(
            raw.time_needed_to_continue_match,
            raw.match_time,
            ParamError::TimeNeededToContinueMatch,
        )?;
        let sudden_death = seconds(
            raw.draw_additional_time,
            DAY,
            ParamError::DrawAdditionalTime,
        )?;
        let before_new_round = seconds(
            raw.time_before_new_round,
            TEN_MINUTES,
            ParamError::TimeBeforeNewRound,
        )?;
        let round_countdown = seconds(
            raw.round_countdown_time,
            TEN_MINUTES,
            ParamError::RoundCountdownTime,
        )?;
        let goal = match u8::try_from(raw.score) {
            Ok(goal) if goal != 0 => goal,
            _ => return Err(ParamError::Score),
        };
        let respawn_wave = seconds(raw.respawn_time, HOUR, ParamError::RespawnTime)?;
        if respawn_wave == Ms::ZERO {
            return Err(ParamError::RespawnTime);
        }
        let respawn_least = seconds(
            raw.time_needed_to_respawn,
            HOUR,
            ParamError::TimeNeededToRespawn,
        )?;
        // 0.46c2 (0x73FCDA): v = min(spawn time, variation); the wait is
        // uniform in [spawn time - v, spawn time + v).
        let spawn = seconds(raw.vi_spawn_time, DAY / 2, ParamError::ViSpawnTime)?;
        let variation = Ms::from_seconds(raw.vi_spawn_variation.min(raw.vi_spawn_time))
            .ok_or(ParamError::ViSpawnTime)?;
        let not_picked_ttl = seconds(raw.vi_not_picked_ttl, DAY, ParamError::ViNotPickedTtl)?;
        let dropped_ttl = seconds(raw.vi_dropped_ttl, DAY, ParamError::ViDroppedTtl)?;
        if !(0.0..=2.0).contains(&raw.drop_lift) {
            return Err(ParamError::DropLift);
        }
        let drop_lift = (raw.drop_lift * 1000.0) as i32;

        if spawners.is_empty() || spawners.len() > MAX_SPAWNERS {
            return Err(ParamError::Spawners);
        }
        let mut places = [Position::from_mm([0; 3]).ok_or(ParamError::Spawners)?; MAX_SPAWNERS];
        for (place, spawner) in places.iter_mut().zip(spawners) {
            *place = Position::from_metres(spawner.position).ok_or(ParamError::Spawners)?;
        }

        let [a, b] = bases else {
            return Err(ParamError::Bases);
        };
        let (first, second) = match (Team::from_raw(a.team), Team::from_raw(b.team)) {
            (Some(Team::Zero), Some(Team::One)) => (a, b),
            (Some(Team::One), Some(Team::Zero)) => (b, a),
            _ => return Err(ParamError::Bases),
        };
        let bases = [
            Position::from_metres(first.position).ok_or(ParamError::Bases)?,
            Position::from_metres(second.position).ok_or(ParamError::Bases)?,
        ];
        // The way to a base is measured between the two: they are apart.
        if bases[0].distance(bases[1]) < 1000 {
            return Err(ParamError::Bases);
        }

        Ok(Params {
            match_time,
            rounds_until: match_time.minus(needed),
            sudden_death,
            before_new_round,
            round_countdown,
            goal,
            respawn_wave,
            respawn_least,
            wait_least: spawn.minus(variation),
            wait_range: 2 * variation.get(),
            not_picked_ttl,
            dropped_ttl,
            drop_lift,
            spawner_count: spawners.len() as u8,
            spawners: places,
            bases,
        })
    }

    pub fn goal(&self) -> u8 {
        self.goal
    }

    pub fn match_time(&self) -> Ms {
        self.match_time
    }

    pub fn sudden_death(&self) -> Ms {
        self.sudden_death
    }

    pub fn spawner_count(&self) -> u8 {
        self.spawner_count
    }

    /// Where spawner `index` puts the artefact; `None` for no spawner of the map.
    pub fn spawner(&self, index: u8) -> Option<Position> {
        (index < self.spawner_count).then(|| self.spawners[usize::from(index)])
    }

    pub fn base(&self, team: Team) -> Position {
        self.bases[team.index()]
    }

    /// The longest any of the rule's deadlines can be after the match's
    /// start: the match, a round's restart (twice: sudden death's time is
    /// counted from past one), sudden death and an artefact's longest wait
    /// or life, and a minute.
    pub(crate) fn horizon(&self) -> u32 {
        let wait = self.wait_least.get() + self.wait_range;
        let life = wait
            .max(self.not_picked_ttl.get())
            .max(self.dropped_ttl.get());
        let restart = self.before_new_round.get() + FADE.get() + self.round_countdown.get();
        self.match_time.get() + self.sudden_death.get() + 2 * restart + life + 60_000
    }
}
