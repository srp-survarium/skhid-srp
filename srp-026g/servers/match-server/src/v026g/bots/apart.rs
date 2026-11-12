//! A teammate inside the body (A175): the side to step to.
//!
//! There is no collision between teammates, so two bots that want the same ground walk in file a metre apart or stand in each other. With `spacing.apart` a bot with a living
//! teammate within `apart_m` steps aside, away from him; the drive tests the ground that side with the fight's own step test before it presses the key. Reynolds' separation
//! (*Steering Behaviors For Autonomous Characters*, GDC 1999) is the same rule as a steering force; here it is one strafe key, since the motor steers by keys.

use super::*;
use crate::v026g::drive::key;

/// A body that moves slower than this stands, m/s.
const STAND_MPS: f32 = 0.5;

/// Teammates this far apart in height are not in each other's way (a floor above).
const SAME_FLOOR_M: f32 = 2.0;

/// The step away from a teammate inside the body: the strafe key, and whether the teammate stands on the line (within 0.15 m across), so that either side would do and the
/// side chosen is the lower place's by convention, not by where he is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Away {
    pub key: u32,
    pub tie: bool,
}

impl Away {
    const NONE: Self = Self { key: 0, tie: false };

    /// The key of the other side.
    pub(super) fn other_key(self) -> u32 {
        if self.key == key::LEFT { key::RIGHT } else { key::LEFT }
    }
}

/// The strafe key that takes `me` away from the nearest living teammate within `within_m` (none: 0). A teammate straight ahead or behind (within 0.15 m of the line)
/// is stepped away from to the side the lower place number takes, so two bodies never step the same way into each other.
pub(super) fn key_away(me: &PlaceView, view: &MatchView, within_m: f32) -> Away {
    let mut nearest: Option<(f32, &PlaceView)> = None;
    let mut at = (0.0f32, 0.0f32);
    for mate in view.team(me.team) {
        if mate.place == me.place || (mate.feet.y - me.feet.y).abs() >= SAME_FLOOR_M {
            continue;
        }
        // His fix is for his own moment: led to the moment of mine, so a mate running by is where he will be, not where he was.
        let [mx, mz] = led(mate, me.known_at);
        let d = (mx - me.feet.x).hypot(mz - me.feet.z);
        if d < within_m && nearest.is_none_or(|(best, _)| d < best) {
            nearest = Some((d, mate));
            at = (mx, mz);
        }
    }
    let Some((_, mate)) = nearest else {
        return Away::NONE;
    };
    // The one that walks gives way to the one that stands (a camper at its post, a bot at a battery): a standing bot steps only when the other stands too, and then the higher place does.
    let still = |p: &PlaceView| p.velocity[0].hypot(p.velocity[1]) < STAND_MPS;
    if still(me) && (!still(mate) || me.place < mate.place) {
        return Away::NONE;
    }
    let [lx, lz] = kin::forward(me.yaw + std::f32::consts::FRAC_PI_2);
    let left = (at.0 - me.feet.x) * lx + (at.1 - me.feet.z) * lz;
    let tie = left.abs() <= 0.15;
    let key = if left > 0.15 {
        key::RIGHT
    } else if left < -0.15 || me.place < mate.place {
        key::LEFT
    } else {
        key::RIGHT
    };
    Away { key, tie }
}

/// A side of the bot's heading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    /// +1 for the left (a quarter turn from the heading), -1 for the right.
    fn sign(self) -> f32 {
        match self {
            Self::Left => 1.0,
            Self::Right => -1.0,
        }
    }
}

/// The line a bot walks (A175, `spacing.lane`): the route's own, or one beside it on a side until a time. The walk steers at the route's point shifted by [`Lane::offset`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lane {
    #[default]
    Centre,
    Over { side: Side, until: u32 },
}

impl Lane {
    /// The one transition: the lane after a tick at `now` (ms) in which a teammate in file was `in_file` (the side to move over to, none: nobody in file). From the centre a teammate
    /// in file puts the bot over on that side; a bot over keeps its side while one is in file (the time runs from the last), and goes back to the centre `hold_ms` after the last.
    pub(super) fn next(self, now: u32, in_file: Option<Side>, hold_ms: u32) -> Self {
        match (self, in_file) {
            (Self::Centre, Some(side)) => Self::Over { side, until: now.wrapping_add(hold_ms) },
            (Self::Over { side, .. }, Some(_)) => Self::Over { side, until: now.wrapping_add(hold_ms) },
            (Self::Over { until, .. }, None) if (now.wrapping_sub(until) as i32) >= 0 => Self::Centre,
            (lane, None) => lane,
        }
    }

    /// The signed offset of the steering point from the route's line, m (left positive); none in the centre.
    pub(super) fn offset(self, offset_m: f32) -> Option<f32> {
        match self {
            Self::Centre => None,
            Self::Over { side, .. } => Some(side.sign() * offset_m),
        }
    }
}

/// A bot walks the same way as a teammate when both go over this, m/s, and their directions are within 45 degrees.
const FILE_MPS: f32 = 1.0;

/// The side `me` moves over to when a living teammate walks his way in file: within `look_m` along his heading (ahead or behind) and `band_m` across it, on one floor. The side is
/// away from the teammate (he is on the left: the right); one on the line (within 0.15 m across) is gone round on the left by the lower place and on the right by the higher, so the two
/// never choose the same one. None: nobody in file, or `me` is not walking (a standing bot has no line).
pub(super) fn in_file(me: &PlaceView, view: &MatchView, look_m: f32, band_m: f32) -> Option<Side> {
    let speed = me.velocity[0].hypot(me.velocity[1]);
    if speed < FILE_MPS {
        return None;
    }
    let (ux, uz) = (me.velocity[0] / speed, me.velocity[1] / speed);
    let mut best: Option<(f32, f32, &PlaceView)> = None;
    for mate in view.team(me.team) {
        if mate.place == me.place || (mate.feet.y - me.feet.y).abs() >= SAME_FLOOR_M {
            continue;
        }
        let mate_speed = mate.velocity[0].hypot(mate.velocity[1]);
        if mate_speed < FILE_MPS || (mate.velocity[0] * ux + mate.velocity[1] * uz) / mate_speed < std::f32::consts::FRAC_PI_4.cos() {
            continue;
        }
        let [mx, mz] = led(mate, me.known_at);
        let (dx, dz) = (mx - me.feet.x, mz - me.feet.z);
        // The left of the heading is a quarter turn: (-uz, ux).
        let (along, left) = (dx * ux + dz * uz, -dx * uz + dz * ux);
        if along.abs() <= look_m && left.abs() < band_m && best.is_none_or(|(a, _, _)| along.abs() < a) {
            best = Some((along.abs(), left, mate));
        }
    }
    let (_, left, mate) = best?;
    Some(if left > 0.15 {
        Side::Right
    } else if left < -0.15 || me.place < mate.place {
        Side::Left
    } else {
        Side::Right
    })
}

/// What a lane did this tick, for the bench's counts by reason.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LaneUse {
    /// No lane (the centre), or the walk did not reach the steering point (a jump, an aimed walk).
    #[default]
    Idle,
    /// The steering point was moved over.
    Applied,
    /// The shifted point is not one the body walks to in a line (a wall, a corner).
    NoRoom,
    /// The shifted point is on another floor than the route's point, or near a lip or a fall: why.
    UnsafeFloor(Unsafe),
    /// The route has a gate, a jump or a drop within reach, or its end is near: the route's own line is kept.
    RouteBusy,
}

impl LaneUse {
    /// The bench's number of the use: 0 idle, 1 applied, 2 no room, 3 unsafe floor, 4 route busy.
    pub fn code(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Applied => 1,
            Self::NoRoom => 2,
            Self::UnsafeFloor(_) => 3,
            Self::RouteBusy => 4,
        }
    }
}

/// Why a lane's shifted point is not a floor to walk to (A212, the bench's counts): the body itself has no grid floor under it or stands near a lip or a fall, or the shifted point
/// has no floor, is near a lip or a fall, or lies on another floor than the route's point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsafe {
    BodyNoFloor,
    BodyAtEdge,
    NoFloor,
    AtEdge,
    OtherFloor,
}

/// Whether the bot behind should ease off: a living teammate within `within_m`, ahead of it (in the cone of its heading) and moving along it at over 1 m/s. A teammate standing
/// ahead is not waited for (a post held in a corridor would hold the bot for good).
pub(super) fn ease(me: &PlaceView, view: &MatchView, within_m: f32) -> bool {
    let [fx, fz] = kin::forward(me.yaw);
    view.team(me.team).any(|mate| {
        if mate.place == me.place || (mate.feet.y - me.feet.y).abs() >= SAME_FLOOR_M {
            return false;
        }
        let [mx, mz] = led(mate, me.known_at);
        let (dx, dz) = (mx - me.feet.x, mz - me.feet.z);
        let d = dx.hypot(dz);
        let ahead = dx * fx + dz * fz;
        d < within_m && ahead > 0.5 * d && mate.velocity[0] * fx + mate.velocity[1] * fz > EASE_MPS
    })
}

/// A teammate moves along the bot's heading at over this, m/s, to be waited for.
const EASE_MPS: f32 = 1.0;

/// What the queue (A212, `spacing.queue`) asks of a walking bot this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Give {
    /// No teammate just ahead.
    None,
    /// A teammate ahead within `gap_m`: the sprint is dropped.
    Slow,
    /// Forward is let go of too: a teammate ahead within `stop_m`, or (`abreast`) one beside the bot within it, walking its way with neither ahead of the other, and this bot the
    /// higher place (an order, so that the two never both wait).
    Stop { abreast: bool },
}

impl Give {
    fn rank(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Slow => 1,
            Self::Stop { .. } => 2,
        }
    }
}

/// A body walks for the queue at this or more, m/s: a standing bot gives way to nobody, and a teammate standing ahead is not waited for (a post held in a corridor would hold the
/// bot for good: the side step goes round him).
const QUEUE_MPS: f32 = 1.0;
/// Two bodies walk the same way when their directions are within 45 degrees.
const ALIKE_COS: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// The queue's ask of `me` (A212, Reynolds' queuing): the nearest need among its living teammates on its floor within `gap_m`, led to its moment. A teammate ahead (within
/// `cone_deg` of `me`'s way) that walks and is not coming at it is given way to ([`Give::Slow`], inside `stop_m` [`Give::Stop`]); when each has the other ahead (they cross), only
/// the higher place gives way. A teammate beside it within `stop_m`, walking its way, neither ahead, is waited for by the higher place unless `stepped` (the side step of
/// `spacing.apart` was taken this tick: the two part sideways).
pub(super) fn queue(me: &PlaceView, view: &MatchView, gap_m: f32, stop_m: f32, cone_deg: f32, stepped: bool) -> Give {
    let speed = me.velocity[0].hypot(me.velocity[1]);
    if speed < QUEUE_MPS {
        return Give::None;
    }
    let (ux, uz) = (me.velocity[0] / speed, me.velocity[1] / speed);
    let cone = cone_deg.to_radians().cos();
    let mut give = Give::None;
    for mate in view.team(me.team) {
        if mate.place == me.place || (mate.feet.y - me.feet.y).abs() >= SAME_FLOOR_M {
            continue;
        }
        let mate_speed = mate.velocity[0].hypot(mate.velocity[1]);
        if mate_speed < QUEUE_MPS {
            continue;
        }
        let [mx, mz] = led(mate, me.known_at);
        let (dx, dz) = (mx - me.feet.x, mz - me.feet.z);
        let d = dx.hypot(dz);
        if d >= gap_m {
            continue;
        }
        let (vx, vz) = (mate.velocity[0] / mate_speed, mate.velocity[1] / mate_speed);
        let mate_ahead = dx * ux + dz * uz > cone * d;
        let me_ahead = -dx * vx - dz * vz > cone * d;
        let along = vx * ux + vz * uz;
        let ask = if mate_ahead && along >= 0.0 && (!me_ahead || me.place > mate.place) {
            if d < stop_m { Give::Stop { abreast: false } } else { Give::Slow }
        } else if !mate_ahead && !me_ahead && !stepped && d < stop_m && along > ALIKE_COS && me.place > mate.place {
            Give::Stop { abreast: true }
        } else {
            Give::None
        };
        if ask.rank() > give.rank() {
            give = ask;
        }
    }
    give
}

/// What the queue did on a bot's tick, for the bench's counts by reason.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueueUse {
    /// Nothing asked.
    #[default]
    Idle,
    /// Slowed: the sprint was down and is dropped.
    SlowDropped,
    /// Asked to slow with no sprint down: nothing to drop.
    SlowNoSprint,
    /// Forward let go of behind a teammate within `stop_m`.
    Stop,
    /// Forward let go of beside a teammate (the abreast order).
    Yield,
    /// Asked, but on a jump of the route (lined up for it, or pressed): a sprint jump needs its run-up.
    RefusedJump,
    /// Asked, but the fight holds the keys (a hold, the reflex's strafe or fire, a use).
    RefusedHeld,
    /// Asked, but escaping (a grenade, a zone).
    RefusedEscape,
}

/// Where `mate` is at `moment` (ms), from his fix, his velocity and the time his fix is for; at most [`LEAD_MAX_S`] either way.
fn led(mate: &PlaceView, moment: u32) -> [f32; 2] {
    let dt = ((moment as f32 - mate.known_at as f32) / 1000.0).clamp(-LEAD_MAX_S, LEAD_MAX_S);
    [mate.feet.x + mate.velocity[0] * dt, mate.feet.z + mate.velocity[1] * dt]
}

/// The most a fix is led, s (a bot's decided tick is up to a few ticks past the clock; a human's fix is the clock's).
const LEAD_MAX_S: f32 = 0.3;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::view::Mode;

    fn view_of(places: Vec<PlaceView>) -> MatchView {
        MatchView {
            clock: 0,
            mode: Mode::TeamDeathmatch,
            time_left_s: 600.0,
            total_s: 900.0,
            points: [0, 0],
            places,
            batteries: Vec::new(),
            marks: Vec::new(),
            containers: Vec::new(),
            sounds: Vec::new(),
            deaths: Vec::new(),
            world_time: 0,
            can_respawn: [true, true],
            next_battery: None,
            grenades: Vec::new(),
            smoke: Default::default(),
        }
    }

    fn at(place: usize, team: u8, x: f32, z: f32) -> PlaceView {
        PlaceView {
            place,
            team,
            bot: true,
            present: true,
            alive: true,
            feet: Vec3::new(x, 0.0, z),
            yaw: 0.0,
            pitch: 0.0,
            velocity: [0.0, 0.0],
            keys: 0,
            health: 1.0,
            spawned: 0,
            kills: 0,
            deaths: 0,
            streak: 0,
            carrying: None,
            seen_at: 0,
            late_ms: 0,
            known_at: 0,
        }
    }

    #[test]
    fn a_bot_steps_away_from_the_teammate_inside_it_and_not_from_one_outside_it_or_an_enemy() {
        // Yaw 0 faces +z; the left of it is -x (kin::forward(yaw + 90 deg) = [-1, 0]).
        let mut me = at(0, 0, 0.0, 0.0);
        me.velocity = [0.0, 4.0];
        let on_left = view_of(vec![me.clone(), at(1, 0, -0.8, 0.0)]);
        assert_eq!(key_away(&me, &on_left, 1.1).key, key::RIGHT);
        let on_right = view_of(vec![me.clone(), at(1, 0, 0.8, 0.0)]);
        assert_eq!(key_away(&me, &on_right, 1.1).key, key::LEFT);
        let outside = view_of(vec![me.clone(), at(1, 0, 1.4, 0.0)]);
        assert_eq!(key_away(&me, &outside, 1.1).key, 0);
        let enemy = view_of(vec![me.clone(), at(1, 1, 0.5, 0.0)]);
        assert_eq!(key_away(&me, &enemy, 1.1).key, 0);
        let above = view_of(vec![me.clone(), PlaceView { feet: Vec3::new(0.3, 3.0, 0.0), ..at(1, 0, 0.3, 0.0) }]);
        assert_eq!(key_away(&me, &above, 1.1).key, 0);
    }

    fn walking(place: usize, x: f32, z: f32, speed: f32) -> PlaceView {
        let mut p = at(place, 0, x, z);
        p.velocity = [0.0, speed];
        p
    }

    #[test]
    fn a_bot_walking_in_file_goes_over_to_the_side_away_from_the_teammate_on_its_line() {
        let me = walking(0, 0.0, 0.0, 4.0);
        // Heading +z; the left of it is -x. A mate 3 m ahead and 0.4 m to the left: go right.
        let view = view_of(vec![me.clone(), walking(1, -0.4, 3.0, 4.0)]);
        assert_eq!(in_file(&me, &view, 8.0, 0.7), Some(Side::Right));
        let view = view_of(vec![me.clone(), walking(1, 0.4, -3.0, 4.0)]);
        assert_eq!(in_file(&me, &view, 8.0, 0.7), Some(Side::Left));
        // On the line: the lower place goes left, the higher right.
        let view = view_of(vec![me.clone(), walking(1, 0.0, 3.0, 4.0)]);
        assert_eq!(in_file(&me, &view, 8.0, 0.7), Some(Side::Left));
        let high = walking(2, 0.0, 0.0, 4.0);
        let view = view_of(vec![walking(1, 0.0, 3.0, 4.0), high.clone()]);
        assert_eq!(in_file(&high, &view, 8.0, 0.7), Some(Side::Right));
        // Too far along, too far across, walking another way, standing, an enemy, a floor above: nobody in file.
        for mate in [
            walking(1, 0.0, 9.0, 4.0),
            walking(1, 1.2, 3.0, 4.0),
            walking(1, 0.0, 3.0, -4.0),
            walking(1, 0.0, 3.0, 0.2),
            PlaceView { team: 1, ..walking(1, 0.0, 3.0, 4.0) },
            PlaceView { feet: Vec3::new(0.0, 3.0, 3.0), ..walking(1, 0.0, 3.0, 4.0) },
        ] {
            assert_eq!(in_file(&me, &view_of(vec![me.clone(), mate]), 8.0, 0.7), None);
        }
        // A bot that stands has no line.
        let still = walking(0, 0.0, 0.0, 0.2);
        assert_eq!(in_file(&still, &view_of(vec![still.clone(), walking(1, 0.0, 3.0, 4.0)]), 8.0, 0.7), None);
    }

    #[test]
    fn a_lane_is_taken_kept_while_the_teammate_is_in_file_and_given_up_after_the_hold() {
        let hold = 2_000;
        let lane = Lane::Centre.next(1_000, Some(Side::Left), hold);
        assert_eq!(lane, Lane::Over { side: Side::Left, until: 3_000 });
        // Still in file on the other side's account: the side is kept, the time runs on.
        let lane = lane.next(2_000, Some(Side::Right), hold);
        assert_eq!(lane, Lane::Over { side: Side::Left, until: 4_000 });
        // Nobody in file: kept until the hold is out, then the centre.
        assert_eq!(lane.next(3_900, None, hold), lane);
        assert_eq!(lane.next(4_000, None, hold), Lane::Centre);
        assert_eq!(Lane::Centre.next(5_000, None, hold), Lane::Centre);
        // The clock wraps.
        let late = Lane::Centre.next(u32::MAX - 500, Some(Side::Right), hold);
        assert_eq!(late.next(u32::MAX - 100, None, hold), late);
        assert_eq!(late.next(1_600, None, hold), Lane::Centre);
        assert_eq!(Lane::Over { side: Side::Left, until: 0 }.offset(0.9), Some(0.9));
        assert_eq!(Lane::Over { side: Side::Right, until: 0 }.offset(0.9), Some(-0.9));
        assert_eq!(Lane::Centre.offset(0.9), None);
    }

    #[test]
    fn a_bot_eases_off_behind_a_teammate_walking_ahead_of_it_and_not_behind_one_standing_or_one_behind() {
        let me = at(0, 0, 0.0, 0.0);
        let mut walking = at(1, 0, 0.0, 0.9);
        walking.velocity = [0.0, 4.0];
        assert!(ease(&me, &view_of(vec![me.clone(), walking.clone()]), 1.1));
        let standing = at(1, 0, 0.0, 0.9);
        assert!(!ease(&me, &view_of(vec![me.clone(), standing]), 1.1));
        let mut behind = at(1, 0, 0.0, -0.9);
        behind.velocity = [0.0, 4.0];
        assert!(!ease(&me, &view_of(vec![me.clone(), behind]), 1.1));
        let mut away = at(1, 0, 0.0, 0.9);
        away.velocity = [0.0, -4.0];
        assert!(!ease(&me, &view_of(vec![me.clone(), away]), 1.1));
    }

    #[test]
    fn of_two_bodies_standing_on_one_spot_the_higher_place_steps_and_the_one_that_walks_gives_way_to_the_one_that_stands() {
        let a = at(0, 0, 0.0, 0.0);
        let b = at(1, 0, 0.0, 0.0);
        let view = view_of(vec![a.clone(), b.clone()]);
        assert_eq!(key_away(&a, &view, 1.1).key, 0);
        assert_ne!(key_away(&b, &view, 1.1).key, 0);
        // A walker steps whatever his place; a stander does not step for a walker.
        let mut walker = at(0, 0, 0.0, 0.0);
        walker.velocity = [0.0, 4.0];
        let view = view_of(vec![walker.clone(), b.clone()]);
        assert_ne!(key_away(&walker, &view, 1.1).key, 0);
        assert_eq!(key_away(&b, &view, 1.1).key, 0);
    }

    /// The queue (A212): the bot behind a teammate walking its way drops its sprint inside 2 m and lets go of forward inside 1.1 m; the one ahead is asked nothing; a teammate
    /// standing ahead, one coming at the bot, one farther than 2 m, an enemy or a standing bot asks nothing; two walking side by side wait in place order unless they stepped apart;
    /// two crossing, each with the other ahead, only the higher place gives way.
    #[test]
    fn the_bot_behind_a_walking_teammate_gives_way_and_the_one_ahead_does_not() {
        // Heading +z for everyone (velocity [0, 4]).
        let behind = walking(0, 0.0, 0.0, 4.0);
        let ahead_far = walking(1, 0.2, 1.6, 4.0);
        let view = view_of(vec![behind.clone(), ahead_far.clone()]);
        assert_eq!(queue(&behind, &view, 2.0, 1.1, 60.0, false), Give::Slow);
        assert_eq!(queue(&ahead_far, &view, 2.0, 1.1, 60.0, false), Give::None);
        let ahead_near = walking(1, 0.2, 0.8, 4.0);
        let view = view_of(vec![behind.clone(), ahead_near]);
        assert_eq!(queue(&behind, &view, 2.0, 1.1, 60.0, false), Give::Stop { abreast: false });
        // Standing ahead, coming at it, out of reach, an enemy: nothing.
        let view = view_of(vec![behind.clone(), at(1, 0, 0.0, 0.8)]);
        assert_eq!(queue(&behind, &view, 2.0, 1.1, 60.0, false), Give::None);
        let view = view_of(vec![behind.clone(), walking(1, 0.0, 0.8, -4.0)]);
        assert_eq!(queue(&behind, &view, 2.0, 1.1, 60.0, false), Give::None);
        let view = view_of(vec![behind.clone(), walking(1, 0.0, 2.5, 4.0)]);
        assert_eq!(queue(&behind, &view, 2.0, 1.1, 60.0, false), Give::None);
        let view = view_of(vec![behind.clone(), PlaceView { team: 1, ..walking(1, 0.0, 0.8, 4.0) }]);
        assert_eq!(queue(&behind, &view, 2.0, 1.1, 60.0, false), Give::None);
        let standing = at(0, 0, 0.0, 0.0);
        let view = view_of(vec![standing.clone(), walking(1, 0.0, 0.8, 4.0)]);
        assert_eq!(queue(&standing, &view, 2.0, 1.1, 60.0, false), Give::None);
        // Abreast 0.8 m apart, both walking +z: the higher place waits, the lower walks on; a pair that stepped apart does not wait.
        let (low, high) = (walking(0, 0.0, 0.0, 4.0), walking(1, 0.8, 0.0, 4.0));
        let view = view_of(vec![low.clone(), high.clone()]);
        assert_eq!(queue(&low, &view, 2.0, 1.1, 60.0, false), Give::None);
        assert_eq!(queue(&high, &view, 2.0, 1.1, 60.0, false), Give::Stop { abreast: true });
        assert_eq!(queue(&high, &view, 2.0, 1.1, 60.0, true), Give::None);
        // Crossing: place 0 walks +z, place 1 walks +x from ahead and to its left, each with the other in its cone: only place 1 gives way.
        let a = walking(0, 0.0, 0.0, 4.0);
        let mut b = at(1, 0, -1.0, 1.2);
        b.velocity = [4.0, 0.0];
        let view = view_of(vec![a.clone(), b.clone()]);
        assert_eq!(queue(&a, &view, 2.0, 1.1, 60.0, false), Give::None);
        assert_ne!(queue(&b, &view, 2.0, 1.1, 60.0, false), Give::None);
    }
}
