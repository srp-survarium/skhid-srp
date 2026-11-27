//! The mode's parameters: a mode config's raw values parsed once into what
//! the rule plays with. Never serialized: every game reads the same config.

use crate::geometry::Position;
use crate::restart::Timing;
use crate::roster::Team;
use crate::shape::{Shape, ShapeKind, View};
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
    pub vi_spawn_time: u32,
    pub vi_spawn_variation: u32,
    pub vi_not_picked_ttl: u32,
    pub vi_dropped_ttl: u32,
    /// How far over the ground a dropped artefact lies, m (the item's
    /// `collision_shape.position` y: 0.13).
    pub drop_lift: f32,
    /// The radius of the sphere the use key takes the artefact in, m (the
    /// item's `usable_shape`: 0.3).
    pub usable_radius: f32,
}

/// A place the artefact appears at: `mode.artefact_hunt.spawners[]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawner {
    pub position: [f32; 3],
}

/// The most shapes a base's container has.
pub const MAX_SHAPES: usize = 8;

/// A shape of a base as the config has it (`shapes[]`: the form Research's
/// `capture_zones` use).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawShape {
    /// 0 sphere, 1 box, 2 cylinder.
    pub kind: u32,
    pub position: [f32; 3],
    pub axes: [[f32; 3]; 3],
    pub half: [f32; 3],
}

/// A team's base: `mode.artefact_hunt.bases[]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BaseDef<'a> {
    pub team: u32,
    pub position: [f32; 3],
    /// Where the use key delivers: 1 to [`MAX_SHAPES`].
    pub shapes: &'a [RawShape],
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
    ViSpawnTime,
    ViNotPickedTtl,
    ViDroppedTtl,
    DropLift,
    UsableRadius,
    /// None, more than [`MAX_SPAWNERS`], or a position that is no point of a map.
    Spawners,
    /// Not exactly two, one a team, at least a metre apart, each 1 to
    /// [`MAX_SHAPES`] shapes of a kind with finite numbers.
    Bases,
}

/// The parameters the rule plays with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub(crate) match_time: Ms,
    /// A delivery made when more than this of the match time has passed
    /// (less than `time_needed_to_continue_match` left) starts no new round
    /// of the regular time: `match_time` less `time_needed_to_continue_match`.
    pub(crate) rounds_until: Ms,
    pub(crate) sudden_death: Ms,
    /// The rounds' restart: `time_before_new_round`, `round_countdown_time`.
    pub(crate) restart: Timing,
    /// 1..=255.
    pub(crate) goal: u8,
    /// Not 0.
    pub(crate) respawn_wave: Ms,
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
    /// By team: its shapes, the first `base_shapes` of them.
    pub(crate) base_shapes: [[Shape; MAX_SHAPES]; 2],
    pub(crate) base_shape_count: [u8; 2],
    /// The artefact's usable sphere, m: above 0.
    pub(crate) usable_radius: f32,
}

/// How far the use key reaches along the view (0.26g's
/// `usable_objects_detection_distance`, 0xA710D8).
pub const REACH: f32 = 2.0;
/// How far before a base's shape the level may be met (its prop's own
/// collision lies at the shape's surface).
pub const BASE_MARGIN: f32 = 0.15;

fn base_shapes(base: &BaseDef) -> Result<([Shape; MAX_SHAPES], u8), ParamError> {
    if base.shapes.is_empty() || base.shapes.len() > MAX_SHAPES {
        return Err(ParamError::Bases);
    }
    let none = Shape::sphere([0.0; 3], 0.0).ok_or(ParamError::Bases)?;
    let mut shapes = [none; MAX_SHAPES];
    for (out, raw) in shapes.iter_mut().zip(base.shapes) {
        let kind = ShapeKind::from_raw(raw.kind).ok_or(ParamError::Bases)?;
        *out = Shape::new(kind, raw.position, raw.axes, raw.half).ok_or(ParamError::Bases)?;
    }
    Ok((shapes, base.shapes.len() as u8))
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
        let (shapes_0, count_0) = base_shapes(first)?;
        let (shapes_1, count_1) = base_shapes(second)?;
        if !(raw.usable_radius > 0.0 && raw.usable_radius <= 10.0) {
            return Err(ParamError::UsableRadius);
        }

        Ok(Params {
            match_time,
            rounds_until: match_time.minus(needed),
            sudden_death,
            restart: Timing::new(before_new_round, round_countdown),
            goal,
            respawn_wave,
            wait_least: spawn.minus(variation),
            wait_range: 2 * variation.get(),
            not_picked_ttl,
            dropped_ttl,
            drop_lift,
            spawner_count: spawners.len() as u8,
            spawners: places,
            bases,
            base_shapes: [shapes_0, shapes_1],
            base_shape_count: [count_0, count_1],
            usable_radius: raw.usable_radius,
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
        self.spawners
            .get(..usize::from(self.spawner_count))?
            .get(usize::from(index))
            .copied()
    }

    pub fn base(&self, team: Team) -> Position {
        self.bases[team.index()]
    }

    /// Whether the view takes the artefact lying at `at`: its usable sphere
    /// entered within the use key's reach and before the level, which the
    /// game's ray along the view met `level` along it (none: not within the
    /// reach).
    pub fn takes(&self, view: &View, at: Position, level: Option<f32>) -> bool {
        let reach = level.map_or(REACH, |hit| if hit < REACH { hit } else { REACH });
        Shape::sphere(at.metres(), self.usable_radius)
            .is_some_and(|sphere| sphere.reached(view, reach))
    }

    /// Whether the view puts the artefact into `team`'s base: one of its
    /// shapes entered within the reach, the level met no nearer than
    /// [`BASE_MARGIN`] before it.
    pub fn delivers(&self, view: &View, team: Team, level: Option<f32>) -> bool {
        let reach = level.map_or(REACH, |hit| {
            let past = hit + BASE_MARGIN;
            if past < REACH { past } else { REACH }
        });
        let count = usize::from(self.base_shape_count[team.index()]);
        self.base_shapes[team.index()][..count]
            .iter()
            .any(|shape| shape.reached(view, reach))
    }

    /// The longest any of the rule's deadlines can be after the match's
    /// start: the match, a round's restart (twice: sudden death's time is
    /// counted from past one), sudden death and an artefact's longest wait
    /// or life, and a minute.
    ///
    /// No match of these parameters ends later than this after its start
    /// (an `Over` is at most a delivery's restart and sudden death past the
    /// match time).
    pub fn horizon(&self) -> u32 {
        let wait = self.wait_least.get() + self.wait_range;
        let life = wait
            .max(self.not_picked_ttl.get())
            .max(self.dropped_ttl.get());
        let restart = self.restart.whole().get();
        self.match_time.get() + self.sudden_death.get() + 2 * restart + life + 60_000
    }
}
