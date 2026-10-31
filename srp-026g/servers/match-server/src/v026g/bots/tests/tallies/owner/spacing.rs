//! The owner counts of how the bots spread (A151): the teammates within 30 m of each living body, once a second, and the
//! enemies within 30 m in the 10 s before each death (a death is "crowded" with 3 or more distinct enemies, the live
//! census's definition: 29 % of the live players' deaths at 18-20 players, 45 % for a top; 70 % of the bots'). By the dying
//! or sampled body's team. The live reading is 1.65 teammates and 0.65 enemies within 30 m of a living player.
//!
//! What it cannot say: the bench's matches are 180 s and 4v4 or 10v10, not 18-20 players, so the levels differ from the live
//! ones; the counters are for the arms of one bench against each other.

use super::*;

/// Within this of a body (ground distance), m: a teammate or an enemy is near (the live census's radius).
const NEAR_M: f32 = 30.0;

/// The seconds before a death whose enemies count.
const WINDOW_S: usize = 10;

/// The shooters counted in the last this long before a death, ms; a death with a hit in the last [`KILL_HIT_MS`] is a bullet's.
const SHOOTERS_MS: u32 = 3_000;
const KILL_HIT_MS: u32 = 1_000;
/// A body that moved less than this since a second ago is standing still, m.
const STILL_M: f32 = 1.5;

/// Two teammates overlap within this (ground distance, m) and less than [`OVERLAP_DY_M`] apart in height; half of it is a body inside the other. Standing
/// still is under [`STILL_MPS`], moving over [`MOVING_MPS`]; two routes end at the same goal within [`SAME_GOAL_M`].
const OVERLAP_M: f32 = 1.0;
const OVERLAP_HALF_M: f32 = 0.5;
const OVERLAP_DY_M: f32 = 2.0;
const SAME_GOAL_M: f32 = 2.0;
const STILL_MPS: f32 = 0.3;
const MOVING_MPS: f32 = 1.0;

/// The ground direction a body's walk keys take it, a unit vector (x, z), none with no walk key down or two that cancel: forward along the view, left a quarter turn from it.
/// The live worlds' keys are read the same way (`A175/human_file.py`), so a body that looks aside as it walks is not read as walking where it looks.
fn walk_direction(rec: &Rec) -> Option<(f32, f32)> {
    use crate::v026g::drive::key;
    let axis = |plus: u32, minus: u32| f32::from(i8::from(rec.keys & plus != 0) - i8::from(rec.keys & minus != 0));
    let (f, l) = (axis(key::FORWARD, key::BACK), axis(key::LEFT, key::RIGHT));
    let [fx, fz] = crate::v026g::bots::kin::forward(rec.yaw);
    let [lx, lz] = crate::v026g::bots::kin::forward(rec.yaw + std::f32::consts::FRAC_PI_2);
    let (x, z) = (f * fx + l * lx, f * fz + l * lz);
    let len = x.hypot(z);
    (len > 0.1).then(|| (x / len, z / len))
}

/// Whether two bodies move the same way (their yaws within 45 degrees; the walk keys are forward).
fn heading_alike(a: Option<&Rec>, b: Option<&Rec>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => crate::v026g::bots::kin::wrap(a.yaw - b.yaw).abs() < std::f32::consts::FRAC_PI_4,
        _ => false,
    }
}

/// The distance to the nearest living teammate, binned (upper edges, m; the last bin is farther): the live players' median is 22.4 m (p10 5.9 m,
/// 379 matches of 12+ places, `grid-record.md`), the bots' 11.4 m (p10 2.8 m).
const NEAREST_EDGES_M: [f32; 6] = [1.0, 3.0, 6.0, 12.0, 22.0, 40.0];

/// The way a body walks: the walk's progress (0 at its own spawns, 1 at the enemy's, `Tactics::progress_at`) a second on, per living bot with a known
/// side. A second that moved the progress by at least this much is forward (or back), the rest stood. GUESS: 1 % of the way a second.
const ADVANCE_EPS: f32 = 0.01;
/// The progress levels sampled, in fifths of the way.
const LEVEL_BINS: usize = 5;

/// Crowded: this many distinct enemies near in the window.
const CROWD: u32 = 3;

/// A pack: this many distinct enemies within [`PACK_M`] of a body in the window before its death, or of a living body (A195: attackers come as packs of 5 or more within 60 m).
const PACK: u32 = 5;
const PACK_M: f32 = 60.0;

/// The living mates a body has, one stratum each up to this many (the live table of `A175/human_normalised.py` reads the same strata: a team of 10 and one of 4 have different
/// distances, so bots and humans are compared at the same number of mates).
const MATES_STRATA: usize = 9;

/// A pair moving the same way is in file at a gap (m along the heading, upper edges; the last bin is farther up to [`FILE_MAX_M`]) and a lateral offset (m, upper edges).
const GAP_EDGES_M: [f32; 4] = [1.0, 2.0, 4.0, 8.0];
const LATERAL_EDGES_M: [f32; 3] = [0.5, 1.0, 2.0];
const FILE_MAX_M: f32 = 16.0;

/// The age of the younger of two bodies (s since each came alive, the opening counting from the match's start), upper edges of the buckets; the last is older.
const AGE_EDGES_S: [usize; 4] = [10, 20, 40, 80];

/// The cell of the map (m) that the 1 m pair-seconds are gathered in, for the hot cells of the per-match line.
const CELL_M: f32 = 6.0;

/// A212: the pairs whose younger body is under this old (s) are the young ones, where the 1 m overlap gathers (A175: 73 % of it, 6-9x the live rate).
const YOUNG_S: usize = 20;
/// The distance of a young pair's midpoint from the nearest spawn point of its team, upper edges (m; the last bin is farther): the spawn points of a team lie 0.5-3 m apart.
const SPAWN_EDGES_M: [f32; 4] = [3.0, 6.0, 13.0, 25.0];
/// The walkable width across the pair's heading, upper edges (m; the last bin is open ground): the grid's floors stepped out [`WIDTH_STEP_M`] at a time to each side until one is
/// missing or on another floor ([`WIDTH_DY_M`]), up to [`WIDTH_SIDE_M`] a side. The grid keeps floors 0.3175 m from a wall, so a door of width w reads about w - 0.6.
const WIDTH_EDGES_M: [f32; 4] = [1.5, 2.5, 4.0, 7.5];
const WIDTH_STEP_M: f32 = 0.25;
const WIDTH_SIDE_M: f32 = 4.0;
const WIDTH_DY_M: f32 = 0.8;
/// Two lives began together when within this of each other (s), from adjacent spawn points when those were within [`ADJACENT_SPAWN_M`] (m).
const SAME_BIRTH_S: usize = 2;
const ADJACENT_SPAWN_M: f32 = 4.0;

/// The young pairs (A212): the 1 m pair-seconds whose younger body is under [`YOUNG_S`], read for where and why they overlap.
#[derive(Clone, Copy, Default)]
struct Young {
    /// By cause, as [`TeamSpacing::cause`].
    cause: [u64; 6],
    /// Both lives from the match's opening, one of each, both respawns.
    kind: [u64; 3],
    /// By the distance of the pair's midpoint from the nearest spawn point of its team ([`SPAWN_EDGES_M`]): [young pairs on one floor, within 1 m].
    spawn: [[u64; 2]; SPAWN_EDGES_M.len() + 1],
    /// By the walkable width across the pair's heading ([`WIDTH_EDGES_M`], then open ground, then no heading known); `width_all` the same for every 1 m pair-second, young or not.
    width: [u64; WIDTH_EDGES_M.len() + 2],
    width_all: [u64; WIDTH_EDGES_M.len() + 2],
    /// The shape of the pair: in file (both moving alike, farther apart along the heading than across), abreast (alike, farther across), crossing (both moving, not alike), standing (one or both still).
    shape: [u64; 4],
    /// Where the two lives began: together (within [`SAME_BIRTH_S`]) on adjacent points (within [`ADJACENT_SPAWN_M`]), together on points farther apart, at different times.
    origin: [u64; 3],
    /// By what the step apart did, as [`TeamSpacing::apart_state`].
    apart: [u64; 7],
    /// Who could give way: one body has the other ahead (within [`AHEAD_RAD`] of its walk) and not the reverse, each has the other ahead (head-on), neither, a body with no walk.
    ahead: [u64; 4],
    /// The rates' denominators: the young pairs on one floor by kind (as `kind`), and the young pairs within [`NEAR_WIDTH_M`] by width (as `width`).
    kind_pairs: [u64; 3],
    width_near: [u64; WIDTH_EDGES_M.len() + 2],
}

/// A teammate is ahead of a walking body within this of its walk's direction, radians.
const AHEAD_RAD: f32 = std::f32::consts::FRAC_PI_3;
/// The young pairs this close (m) are the width's denominator: where the bodies walk near each other.
const NEAR_WIDTH_M: f32 = 4.0;
/// The opening's first moves (A212): a body's first tick 0.5 m from where its life began, as the live table reads it (`A175/live_opening.py`); the spread after the team's earliest
/// by upper edges (s; the last bin is later). Live players (10 recordings, 31 bodies): median 0.25 s, p75 0.75, p90 2.4.
const FIRST_MOVE_M: f32 = 0.5;
const SPREAD_EDGES_S: [f32; 4] = [0.25, 0.75, 1.5, 3.0];

/// The walkable width across the unit heading `(ux, uz)` at `p`, m: see [`WIDTH_EDGES_M`].
fn free_width(grid: &mapdata::WalkGrid, p: Vec3, (ux, uz): (f32, f32)) -> f32 {
    let side = |sign: f32| {
        let mut reach = 0.0;
        while reach < WIDTH_SIDE_M {
            let s = reach + WIDTH_STEP_M;
            let q = Vec3::new(p.x - uz * s * sign, p.y, p.z + ux * s * sign);
            match grid.locate(q, 0.5 * WIDTH_STEP_M) {
                Some(node) if (grid.position(node).y - p.y).abs() < WIDTH_DY_M => reach = s,
                _ => break,
            }
        }
        reach
    };
    side(1.0) + side(-1.0)
}

/// One stratum of [`TeamSpacing::by_mates`]: the body-seconds with that many living mates, the mates within 30 m, the pairs (a body's living mates, summed), those within 1 m, the
/// bodies with a mate within 1 m, and the nearest mate by [`NEAREST_EDGES_M`].
#[derive(Clone, Copy, Default)]
struct Stratum {
    samples: u64,
    mates_near: u64,
    pairs: u64,
    pairs_1: u64,
    body_1: u64,
    nearest: [u64; NEAREST_EDGES_M.len() + 1],
}

/// The bin of `value` among upper `edges` (the last bin is farther).
fn bin_of(value: f32, edges: &[f32]) -> usize {
    edges.iter().position(|&edge| value < edge).unwrap_or(edges.len())
}

/// The life-age buckets of the samples (s since the body came alive, respawns only): 0-10, 10-20, 20-30, 30 and more. The 10-20 s bucket
/// is the respawn clump's line (fx-cover's clump split: bots 0.51 mates near per available mate there, live 0.36).
const LIFE_BUCKET_S: usize = 10;
const LIFE_BUCKETS: usize = 4;

#[derive(Clone, Copy, Default)]
struct TeamSpacing {
    /// The living bodies sampled (a body-second), the teammates and enemies near them, and those with three or more mates near.
    samples: u64,
    mates: u64,
    enemies: u64,
    clumped: u64,
    deaths: u64,
    crowded_deaths: u64,
    /// The distinct enemies near in the window before a death, summed over the deaths.
    crowd_enemies: u64,
    /// What killed a body, for the other deaths (0) and the crowded ones (1): [bullet, not a bullet (a blast, a zone, a fall)];
    /// the distinct shooters that hit it in the last 3 s: 1, 2, 3 or more; the killer standing still for 2 s or more: [no, yes];
    /// the killer's distance: under 10 m, 10-30, 30 and more.
    /// The overlap itself (A175, the owner 2026-10-30: "too close to each other or maybe even inside each other"): each second, the pairs of living teammates
    /// (`pairs`) and those within 1 m and within 0.5 m of each other (ground distance, less than 2 m apart in height), the body-seconds with a mate within 1 m, and the
    /// 1 m pair-seconds by cause: [the same target (the commander gave both the same battery, place, container or spot), the same goal (both routes end within 2 m of each other), a follow play (Escort, Buddy, PairUp, CoverHuman), both standing
    /// still, in file (both moving the same way), other].
    pairs: u64,
    overlap_1: u64,
    overlap_05: u64,
    body_overlap_1: u64,
    cause: [u64; 6],
    /// The distance to the nearest living teammate of each body-second that has one (B6 / A114: the live players' row of the gap table): the
    /// samples, the metres summed, and the samples by [`NEAREST_EDGES_M`].
    nearest_n: u64,
    nearest_m: f64,
    nearest_bins: [u64; NEAREST_EDGES_M.len() + 1],
    /// How far forward the bots walk (B6 P1b: the forward share of the gap table): per living bot-second with a known side and the second
    /// before in the same life, the progress moved ([`Advance`]), and the progress level sampled.
    advance: Advance,
    /// The 1 m pair-seconds by what the step apart did on the pair's two bodies that tick (the worse of the two): nothing asked (no mate inside in the bot's picture), taken, refused (no clear
    /// ground that side), held off (the reflex strafes or fires, a use is held, a side key down).
    apart_state: [u64; 7],
    /// The living bot-seconds by lane (`spacing.lane`): in the centre, and over with what the lane did that tick ([`apart::LaneUse`]: idle, applied, no room, unsafe floor, route busy).
    lane_seconds: [u64; 6],
    /// The unsafe-floor seconds of `lane_seconds` by why ([`apart::Unsafe`]: the body with no floor, the body at an edge, the shifted point with no floor, at an edge, on another floor).
    lane_unsafe: [u64; 5],
    /// The young 1 m pair-seconds (A212).
    young: Young,
    /// Spacing per number of living mates (1 to [`MATES_STRATA`], the last 9 and more).
    by_mates: [Stratum; MATES_STRATA],
    /// The living pairs and those within 1 m by the age of the younger body ([`AGE_EDGES_S`]): [pairs, within 1 m].
    by_age: [[u64; 2]; AGE_EDGES_S.len() + 1],
    /// The pairs moving the same way (both over [`MOVING_MPS`], yaws within 45 degrees, on one floor) within [`FILE_MAX_M`], by the gap along the heading
    /// ([`GAP_EDGES_M`], and farther) and the lateral offset ([`LATERAL_EDGES_M`], and more).
    file: [[u64; LATERAL_EDGES_M.len() + 1]; GAP_EDGES_M.len() + 1],
    /// Every living pair on one floor within 16 m by the ground distance (under 1, 2, 4, 8 and 16 m): the pair-seconds.
    near_pairs: [u64; GAP_EDGES_M.len() + 1],
    /// Packs (A195): the living body-seconds with [`PACK`] or more enemies within [`PACK_M`], the samples, and the deaths with a pack in the window or at the last sample before.
    pack_samples: [u64; 2],
    pack_deaths: [u64; 3],
    /// The 1 m pair-seconds of two moving bodies by the distance of the nearer's route end from the other's feet (under 5 m, 5-20, 20 and more, no route): where the pair is bound.
    in_file_goals: [u64; 4],
    killed_by: [[u64; 2]; 2],
    shooters: [[u64; 3]; 2],
    killer_still: [[u64; 2]; 2],
    killer_m: [[u64; 3]; 2],
    /// By life-age bucket ([`LIFE_BUCKET_S`]): the samples, the teammates near, and the teammates alive (the available
    /// mates, the body itself left out).
    by_life: [[u64; 3]; LIFE_BUCKETS],
}

/// The walk's progress of the living bots, a second to the next: the seconds counted, the progress moved summed (signed), the seconds forward and
/// back ([`ADVANCE_EPS`]), and the level the bodies were at (the sum and by fifths of the way).
#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Advance {
    seconds: u64,
    moved: f64,
    forward: u64,
    back: u64,
    level_sum: f64,
    levels: [u64; LEVEL_BINS],
}

impl Advance {
    /// One body-second at progress `now`, `before` a second ago (none: first sample of its life).
    fn add(&mut self, before: Option<f32>, now: f32) {
        self.level_sum += f64::from(now);
        self.levels[((now * LEVEL_BINS as f32) as usize).min(LEVEL_BINS - 1)] += 1;
        let Some(before) = before else { return };
        let moved = now - before;
        self.seconds += 1;
        self.moved += f64::from(moved);
        self.forward += u64::from(moved >= ADVANCE_EPS);
        self.back += u64::from(moved <= -ADVANCE_EPS);
    }
}

/// The nearest-teammate bin of a distance `d` (m).
fn nearest_bin(d: f32) -> usize {
    NEAREST_EDGES_M.iter().position(|&edge| d < edge).unwrap_or(NEAREST_EDGES_M.len())
}

/// What the topic keeps: the counts by team, and for each body the enemies near it (a bit by place) in each of the last
/// [`WINDOW_S`] seconds.
pub(super) struct SpacingCounts {
    by_team: [TeamSpacing; 2],
    window: Vec<[u64; WINDOW_S]>,
    /// The same for the enemies within [`PACK_M`] (a bit by place), and the count at each body's last sample.
    window_pack: Vec<[u64; WINDOW_S]>,
    last_pack: Vec<u32>,
    /// The 1 m pair-seconds by cell of the map ([`CELL_M`]): where the bodies overlap.
    cells: BTreeMap<(i32, i32), u64>,
    was_alive: Vec<bool>,
    /// The hits each body took in the last 3 s (tick, shooter), the position a second ago of each body and the seconds it has stood within
    /// [`STILL_M`] of it.
    hits: Vec<Vec<(u32, Place)>>,
    last_pos: Vec<Option<Vec3>>,
    still_s: Vec<u32>,
    /// The bodies' last hit's killer (the shooter and the tick) for each body.
    last_killer: Vec<Option<(Place, u32)>>,
    /// Where each bot's route ends (its goal), as the bot reads it.
    goal: Vec<Option<Vec3>>,
    /// The step apart each bot took (or not) on its last tick.
    apart: Vec<u8>,
    /// Each bot's lane now: 0 the centre, 1 + the lane's use when over; and why its shifted point was unsafe, when it was.
    lane: Vec<u8>,
    lane_unsafe: Vec<Option<crate::v026g::bots::apart::Unsafe>>,
    /// Where each body's life began (its feet at its first sample alive).
    spawned_at: Vec<Option<Vec3>>,
    /// The tick of each body's first move ([`FIRST_MOVE_M`]) in its life from the match's opening, and its team.
    first_move: Vec<Option<usize>>,
    team_of: Vec<u8>,
    /// What the commander gave each bot to do (its target).
    target: Vec<crate::v026g::bots::plays::TargetRef>,
    /// The 1 m pair-seconds by the plays of the two bodies (sorted names), by team.
    overlap_plays: [BTreeMap<String, u64>; 2],
    /// The second each body came alive, for a respawn (its life's age); none for its life from the match's start (the
    /// opening is not a respawn, and is left out of the life-age buckets).
    born: Vec<Option<usize>>,
    /// By the play the body runs (a bot's assignment; "human" for the stand-in): the living bodies sampled and the
    /// teammates near them, and how many had 3 or more.
    by_play: BTreeMap<String, [u64; 3]>,
    /// Each bot's map side (the walk's progress is of a side), and its progress now and at the sample before; none for a body that is not a living bot
    /// with a known side (a stand-in human is left out).
    side: Vec<Option<i32>>,
    progress: Vec<Option<f32>>,
    progress_before: Vec<Option<f32>>,
}

impl SpacingCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            by_team: Default::default(),
            window: vec![[0; WINDOW_S]; n],
            window_pack: vec![[0; WINDOW_S]; n],
            last_pack: vec![0; n],
            cells: BTreeMap::new(),
            was_alive: vec![false; n],
            hits: vec![Vec::new(); n],
            last_pos: vec![None; n],
            still_s: vec![0; n],
            last_killer: vec![None; n],
            goal: vec![None; n],
            apart: vec![0; n],
            lane: vec![0; n],
            lane_unsafe: vec![None; n],
            spawned_at: vec![None; n],
            first_move: vec![None; n],
            team_of: vec![0; n],
            target: vec![crate::v026g::bots::plays::TargetRef::None; n],
            overlap_plays: Default::default(),
            born: vec![None; n],
            by_play: BTreeMap::new(),
            side: vec![None; n],
            progress: vec![None; n],
            progress_before: vec![None; n],
        }
    }
}

impl Topic for SpacingCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if let Some(side) = self.side.get_mut(place) {
            *side = bot.and_then(|b| b.side);
        }
        if let Some(goal) = self.goal.get_mut(place) {
            *goal = bot.and_then(|b| b.route.as_ref().map(|(route, _)| route.end()));
        }
        if let Some(apart) = self.apart.get_mut(place) {
            *apart = bot.map_or(0, |b| b.bench_apart);
        }
        if let Some(lane) = self.lane.get_mut(place) {
            *lane = bot.map_or(0, |b| match b.lane {
                crate::v026g::bots::apart::Lane::Centre => 0,
                crate::v026g::bots::apart::Lane::Over { .. } => 1 + b.bench_lane.code(),
            });
        }
        if let Some(why) = self.lane_unsafe.get_mut(place) {
            *why = bot.and_then(|b| match (b.lane, b.bench_lane) {
                (crate::v026g::bots::apart::Lane::Over { .. }, crate::v026g::bots::apart::LaneUse::UnsafeFloor(why)) => Some(why),
                _ => None,
            });
        }
        if let Some(target) = self.target.get_mut(place) {
            *target = bot.and_then(|b| b.assignment.as_ref().map(|a| a.target)).unwrap_or(crate::v026g::bots::plays::TargetRef::None);
        }
    }

    fn bots(&mut self, bots: &Bots, game: &Game) {
        let Some(tactics) = bots.tactics() else { return };
        for (place, body) in game.bodies.iter().enumerate().take(self.progress.len()) {
            self.progress[place] = self.side[place].filter(|_| body.alive).map(|side| tactics.progress_at(body.feet, side));
        }
    }

    fn begin(&mut self, _run: &Run, game: &mut Game) {
        let tick = game.tick;
        for &(_, shooter, target) in &game.hit_log {
            if let Some(hits) = self.hits.get_mut(target) {
                hits.push((tick, shooter));
                self.last_killer[target] = Some((shooter, tick));
            }
        }
        for hits in &mut self.hits {
            hits.retain(|&(at, _)| tick.saturating_sub(at) <= SHOOTERS_MS);
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        let tick = (game.tick.saturating_sub(run.first_tick) / TICK_MS) as usize;
        let (sample, second) = (tick.is_multiple_of(TPS), tick / TPS);
        for place in 0..game.bodies.len().min(self.window.len()) {
            let body = &game.bodies[place];
            let team = usize::from(body.team & 1);
            if self.was_alive[place] && !body.alive {
                let ts = &mut self.by_team[team];
                let near: u32 = self.window[place].iter().fold(0u64, |all, mask| all | mask).count_ones();
                ts.deaths += 1;
                ts.crowded_deaths += u64::from(near >= CROWD);
                ts.crowd_enemies += u64::from(near);
                let crowded = usize::from(near >= CROWD);
                let bullet = self.last_killer[place].filter(|&(_, at)| game.tick.saturating_sub(at) <= KILL_HIT_MS);
                ts.killed_by[crowded][usize::from(bullet.is_none())] += 1;
                let mut shooters: Vec<Place> = self.hits[place].iter().map(|&(_, s)| s).collect();
                shooters.sort_unstable();
                shooters.dedup();
                if bullet.is_some() {
                    ts.shooters[crowded][shooters.len().clamp(1, 3) - 1] += 1;
                }
                if let Some((killer, _)) = bullet {
                    ts.killer_still[crowded][usize::from(self.still_s.get(killer).copied().unwrap_or(0) >= 2)] += 1;
                    let d = game.bodies[killer].feet.distance_xz(body.feet);
                    ts.killer_m[crowded][if d < 10.0 { 0 } else if d < 30.0 { 1 } else { 2 }] += 1;
                }
                let pack: u32 = self.window_pack[place].iter().fold(0u64, |all, mask| all | mask).count_ones();
                ts.pack_deaths[0] += u64::from(pack >= PACK);
                ts.pack_deaths[1] += u64::from(self.last_pack[place] >= PACK);
                ts.pack_deaths[2] += 1;
                self.window[place] = [0; WINDOW_S];
                self.window_pack[place] = [0; WINDOW_S];
                self.hits[place].clear();
            }
            if !self.was_alive[place] && body.alive {
                self.born[place] = (tick > 0).then_some(second);
                self.spawned_at[place] = Some(body.feet);
            }
            self.team_of[place] = body.team;
            if body.alive
                && self.born[place].is_none()
                && self.first_move[place].is_none()
                && self.spawned_at[place].is_some_and(|from| from.distance_xz(body.feet) > FIRST_MOVE_M)
            {
                self.first_move[place] = Some(tick);
            }
            self.was_alive[place] = body.alive;
            if !body.alive {
                self.progress_before[place] = None;
            }
            if sample && body.alive {
                if let Some(now) = self.progress[place] {
                    self.by_team[team].advance.add(self.progress_before[place], now);
                }
                self.progress_before[place] = self.progress[place];
                self.still_s[place] = match self.last_pos[place] {
                    Some(before) if before.distance_xz(body.feet) < STILL_M => self.still_s[place] + 1,
                    _ => 0,
                };
                self.last_pos[place] = Some(body.feet);
            }
            if !(sample && body.alive) {
                continue;
            }
            let (mut mates, mut enemy_mask, mut alive_mates) = (0u64, 0u64, 0u64);
            let (mut pack_mask, mut within_1) = (0u64, 0u64);
            let mut nearest = f32::INFINITY;
            for (other_place, other) in game.bodies.iter().enumerate() {
                if other_place != place && other.alive && other.team == body.team {
                    alive_mates += 1;
                    nearest = nearest.min(other.feet.distance_xz(body.feet));
                    within_1 += u64::from(other.feet.distance_xz(body.feet) < OVERLAP_M && (other.feet.y - body.feet.y).abs() < OVERLAP_DY_M);
                }
                if other_place != place && other.alive && other.team != body.team && other_place < 64 && other.feet.distance_xz(body.feet) <= PACK_M {
                    pack_mask |= 1u64 << other_place;
                }
                if other_place == place || !other.alive || other.feet.distance_xz(body.feet) > NEAR_M {
                    continue;
                }
                if other.team == body.team {
                    mates += 1;
                } else if other_place < 64 {
                    enemy_mask |= 1u64 << other_place;
                }
            }
            let ts = &mut self.by_team[team];
            ts.samples += 1;
            ts.mates += mates;
            ts.enemies += u64::from(enemy_mask.count_ones());
            ts.clumped += u64::from(mates >= 3);
            if self.side[place].is_some() {
                ts.lane_seconds[usize::from(self.lane[place]).min(5)] += 1;
                if let Some(why) = self.lane_unsafe[place] {
                    ts.lane_unsafe[why as usize] += 1;
                }
            }
            ts.pack_samples[0] += 1;
            ts.pack_samples[1] += u64::from(pack_mask.count_ones() >= PACK);
            if alive_mates > 0 {
                let st = &mut ts.by_mates[(alive_mates as usize).min(MATES_STRATA) - 1];
                st.samples += 1;
                st.mates_near += mates;
                st.pairs += alive_mates;
                st.pairs_1 += within_1;
                st.body_1 += u64::from(within_1 > 0);
                st.nearest[nearest_bin(nearest)] += 1;
            }
            if nearest.is_finite() {
                ts.nearest_n += 1;
                ts.nearest_m += f64::from(nearest);
                ts.nearest_bins[nearest_bin(nearest)] += 1;
            }
            if let Some(born) = self.born[place] {
                let life = &mut ts.by_life[((second - born.min(second)) / LIFE_BUCKET_S).min(LIFE_BUCKETS - 1)];
                life[0] += 1;
                life[1] += mates;
                life[2] += alive_mates;
            }
            if let Some(rec) = run.recs[place].last() {
                let play = run.plays.get(usize::from(rec.play)).map_or("none", String::as_str);
                let entry = self.by_play.entry(play.to_owned()).or_default();
                entry[0] += 1;
                entry[1] += mates;
                entry[2] += u64::from(mates >= 3);
            }
            self.window[place][second % WINDOW_S] = enemy_mask;
            self.window_pack[place][second % WINDOW_S] = pack_mask;
            self.last_pack[place] = pack_mask.count_ones();
        }
        if sample {
            self.overlap(run, game, second);
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("spacing", self.part())]
    }
}

impl SpacingCounts {
    /// The overlap of the living teammates this second: see [`TeamSpacing::pairs`].
    fn overlap(&mut self, run: &Run, game: &Game, second: usize) {
        let n = game.bodies.len().min(self.goal.len());
        let mut near = vec![false; n];
        for i in 0..n {
            let (a, ra) = (&game.bodies[i], run.recs[i].last());
            if !a.alive {
                continue;
            }
            for j in (i + 1)..n {
                let (b, rb) = (&game.bodies[j], run.recs[j].last());
                if !b.alive || b.team != a.team {
                    continue;
                }
                let team = usize::from(a.team & 1);
                self.by_team[team].pairs += 1;
                let d = a.feet.distance_xz(b.feet);
                let same_floor = (a.feet.y - b.feet.y).abs() < OVERLAP_DY_M;
                let age = |p: usize| second.saturating_sub(self.born[p].unwrap_or(0));
                let age_bin = AGE_EDGES_S.iter().position(|&edge| age(i).min(age(j)) < edge).unwrap_or(AGE_EDGES_S.len());
                self.by_team[team].by_age[age_bin][0] += 1;
                let young = age(i).min(age(j)) < YOUNG_S;
                // The young pair's midpoint from the nearest spawn point of its team: the bin, for the near pairs on one floor (the rate's denominator) and the overlap.
                let spawn_bin = (young && same_floor && d < FILE_MAX_M).then(|| {
                    let mid = Vec3::new(0.5 * (a.feet.x + b.feet.x), 0.5 * (a.feet.y + b.feet.y), 0.5 * (a.feet.z + b.feet.z));
                    let nearest = game.world.map.spawns.iter().filter(|s| s.team == i32::from(a.team)).map(|s| s.position.distance_xz(mid)).fold(f32::INFINITY, f32::min);
                    bin_of(nearest, &SPAWN_EDGES_M)
                });
                if let Some(bin) = spawn_bin {
                    self.by_team[team].young.spawn[bin][0] += 1;
                }
                // The pair's heading: their walk keys' (the first that has one), else the way to a goal; the walkable width across it at their midpoint, for every 1 m pair and for the
                // young pairs within NEAR_WIDTH_M (the width's denominator).
                let (wa, wb) = (ra.and_then(walk_direction), rb.and_then(walk_direction));
                let width_bin = (same_floor && (d < OVERLAP_M || (young && d < NEAR_WIDTH_M))).then(|| {
                    let to_goal = |goal: Option<Vec3>, from: Vec3| {
                        goal.and_then(|g| {
                            let (dx, dz) = (g.x - from.x, g.z - from.z);
                            let len = dx.hypot(dz);
                            (len > 0.5).then(|| (dx / len, dz / len))
                        })
                    };
                    let heading = wa.or(wb).or_else(|| to_goal(self.goal[i], a.feet)).or_else(|| to_goal(self.goal[j], b.feet));
                    heading.map_or(WIDTH_EDGES_M.len() + 1, |h| {
                        let mid = Vec3::new(0.5 * (a.feet.x + b.feet.x), a.feet.y.max(b.feet.y), 0.5 * (a.feet.z + b.feet.z));
                        bin_of(free_width(&game.world.grid, mid, h), &WIDTH_EDGES_M)
                    })
                });
                if young && same_floor {
                    let y = &mut self.by_team[team].young;
                    y.kind_pairs[usize::from(self.born[i].is_some()) + usize::from(self.born[j].is_some())] += 1;
                    if let Some(bin) = width_bin.filter(|_| d < NEAR_WIDTH_M) {
                        y.width_near[bin] += 1;
                    }
                }
                if same_floor && d < FILE_MAX_M {
                    let ts = &mut self.by_team[team];
                    ts.near_pairs[bin_of(d, &GAP_EDGES_M)] += 1;
                    if let (Some(ra), Some(rb)) = (ra, rb)
                        && ra.speed >= MOVING_MPS
                        && rb.speed >= MOVING_MPS
                        && let (Some((fx, fz)), Some((gx, gz))) = (walk_direction(ra), walk_direction(rb))
                        && fx * gx + fz * gz > std::f32::consts::FRAC_PI_4.cos()
                    {
                        let (hx, hz) = (fx + gx, fz + gz);
                        let h = hx.hypot(hz).max(1e-6);
                        let (ux, uz) = (hx / h, hz / h);
                        let (dx, dz) = (b.feet.x - a.feet.x, b.feet.z - a.feet.z);
                        let (along, lateral) = ((dx * ux + dz * uz).abs(), (dx * -uz + dz * ux).abs());
                        ts.file[bin_of(along, &GAP_EDGES_M)][bin_of(lateral, &LATERAL_EDGES_M)] += 1;
                    }
                }
                if d >= OVERLAP_M || !same_floor {
                    continue;
                }
                let key = ((a.feet.x / CELL_M).floor() as i32, (a.feet.z / CELL_M).floor() as i32);
                *self.cells.entry(key).or_default() += 1;
                let ts = &mut self.by_team[team];
                ts.by_age[age_bin][1] += 1;
                ts.overlap_1 += 1;
                ts.overlap_05 += u64::from(d < OVERLAP_HALF_M);
                near[i] = true;
                near[j] = true;
                let name = |r: Option<&Rec>| r.and_then(|r| run.plays.get(usize::from(r.play))).cloned().unwrap_or_default();
                let (pa, pb) = (name(ra), name(rb));
                let follow = |p: &str| ["Escort", "Buddy", "PairUp", "CoverHuman"].iter().any(|f| p.contains(f));
                let same_target = self.target[i] == self.target[j] && self.target[i] != crate::v026g::bots::plays::TargetRef::None;
                let same_goal = matches!((self.goal[i], self.goal[j]), (Some(x), Some(y)) if x.distance_xz(y) < SAME_GOAL_M);
                let (sa, sb) = (ra.map_or(0.0, |r| r.speed), rb.map_or(0.0, |r| r.speed));
                let cause = if same_target {
                    0
                } else if same_goal {
                    1
                } else if follow(&pa) || follow(&pb) {
                    2
                } else if sa < STILL_MPS && sb < STILL_MPS {
                    3
                } else if sa >= MOVING_MPS && sb >= MOVING_MPS && heading_alike(ra, rb) {
                    4
                } else {
                    5
                };
                ts.cause[cause] += 1;
                let worst = |x: u8, y: u8| -> usize {
                    match (x, y) {
                        (4, _) | (_, 4) => 4,
                        (5, _) | (_, 5) => 5,
                        (2, _) | (_, 2) => 2,
                        (3, _) | (_, 3) => 3,
                        (1, _) | (_, 1) => 1,
                        _ => 0,
                    }
                };
                let apart_state = worst(self.apart[i], self.apart[j]);
                ts.apart_state[apart_state] += 1;
                let width_bin = width_bin.unwrap_or(WIDTH_EDGES_M.len() + 1);
                ts.young.width_all[width_bin] += 1;
                if young {
                    let y = &mut ts.young;
                    y.cause[cause] += 1;
                    y.kind[usize::from(self.born[i].is_some()) + usize::from(self.born[j].is_some())] += 1;
                    if let Some(bin) = spawn_bin {
                        y.spawn[bin][1] += 1;
                    }
                    y.width[width_bin] += 1;
                    y.shape[match (wa, wb) {
                        _ if sa < MOVING_MPS || sb < MOVING_MPS => 3,
                        (Some((fx, fz)), Some((gx, gz))) if fx * gx + fz * gz > std::f32::consts::FRAC_PI_4.cos() => {
                            let (dx, dz) = (b.feet.x - a.feet.x, b.feet.z - a.feet.z);
                            let (along, across) = ((dx * fx + dz * fz).abs(), (dx * -fz + dz * fx).abs());
                            usize::from(across > along)
                        }
                        _ => 2,
                    }] += 1;
                    let together = self.born[i].unwrap_or(0).abs_diff(self.born[j].unwrap_or(0)) <= SAME_BIRTH_S;
                    let adjacent = matches!((self.spawned_at[i], self.spawned_at[j]), (Some(p), Some(q)) if p.distance_xz(q) < ADJACENT_SPAWN_M);
                    y.origin[if !together { 2 } else if adjacent { 0 } else { 1 }] += 1;
                    y.apart[apart_state] += 1;
                    let ahead_of = |walk: Option<(f32, f32)>, from: Vec3, to: Vec3| {
                        walk.map(|(fx, fz)| {
                            let (dx, dz) = (to.x - from.x, to.z - from.z);
                            (dx * fx + dz * fz) > dx.hypot(dz) * AHEAD_RAD.cos()
                        })
                    };
                    y.ahead[match (ahead_of(wa, a.feet, b.feet), ahead_of(wb, b.feet, a.feet)) {
                        (Some(true), Some(false)) | (Some(false), Some(true)) => 0,
                        (Some(true), Some(true)) => 1,
                        (Some(false), Some(false)) => 2,
                        _ => 3,
                    }] += 1;
                }
                if sa >= MOVING_MPS && sb >= MOVING_MPS {
                    let bound = [(self.goal[i], b.feet), (self.goal[j], a.feet)]
                        .into_iter()
                        .filter_map(|(goal, other)| goal.map(|g| g.distance_xz(other)))
                        .fold(f32::INFINITY, f32::min);
                    ts.in_file_goals[if !bound.is_finite() { 3 } else if bound < 5.0 { 0 } else if bound < 20.0 { 1 } else { 2 }] += 1;
                }
                let (lo, hi) = if pa <= pb { (pa, pb) } else { (pb, pa) };
                *self.overlap_plays[team].entry(format!("{lo} + {hi}")).or_default() += 1;
            }
        }
        for (i, inside) in near.into_iter().enumerate() {
            self.by_team[usize::from(game.bodies[i].team & 1)].body_overlap_1 += u64::from(inside);
        }
    }

    fn part(&self) -> Part {
        let team = |t: usize| {
            let c = &self.by_team[t];
            // The opening's first moves of the team's bodies: the spread after the earliest, and the first moves' ticks summed with their count.
            let firsts: Vec<usize> = (0..self.first_move.len()).filter(|&p| usize::from(self.team_of[p] & 1) == t).filter_map(|p| self.first_move[p]).collect();
            let mut spread = [0u64; SPREAD_EDGES_S.len() + 1];
            if let Some(&earliest) = firsts.iter().min() {
                for &first in &firsts {
                    spread[bin_of((first - earliest) as f32 * TICK_MS as f32 / 1000.0, &SPREAD_EDGES_S)] += 1;
                }
            }
            // The A212 keys in an object of their own (one `json!` of every key passes the macro's recursion limit), merged into the team's.
            let young = serde_json::json!({
                "opening_first_move_spread_s_lt025_075_15_3_up": spread,
                "opening_first_move_ticks_sum_and_n": [firsts.iter().sum::<usize>(), firsts.len()],
                "young_1m_ahead_one_both_neither_nowalk": c.young.ahead,
                "young_pairs_by_kind_opening_mixed_respawn": c.young.kind_pairs,
                "young_pairs_within_4m_by_width_m_lt15_25_4_75_open_noheading": c.young.width_near,
                "lane_unsafe_body_nofloor_body_edge_nofloor_edge_otherfloor": c.lane_unsafe,
                "young_1m_cause_target_goal_follow_still_file_other": c.young.cause,
                "young_1m_kind_opening_mixed_respawn": c.young.kind,
                "young_near_pairs_and_1m_by_spawn_m_lt3_6_13_25_up": c.young.spawn,
                "young_1m_by_width_m_lt15_25_4_75_open_noheading": c.young.width,
                "all_1m_by_width_m_lt15_25_4_75_open_noheading": c.young.width_all,
                "young_1m_shape_file_abreast_crossing_standing": c.young.shape,
                "young_1m_origin_together_adjacent_together_apart_different": c.young.origin,
                "young_1m_apart_none_taken_refused_heldoff_eased_otherclear_tied": c.young.apart,
            });
            let mut json = serde_json::json!({
                "samples": c.samples,
                "mates_near": c.mates,
                "enemies_near": c.enemies,
                "clumped_samples": c.clumped,
                "deaths": c.deaths,
                "crowded_deaths": c.crowded_deaths,
                "crowd_enemies": c.crowd_enemies,
                "pairs": c.pairs,
                "overlap_1m_pair_s": c.overlap_1,
                "overlap_05m_pair_s": c.overlap_05,
                "overlap_1m_body_s": c.body_overlap_1,
                "overlap_1m_cause_target_goal_follow_still_file_other": c.cause,
                "nearest_mate_samples": c.nearest_n,
                "nearest_mate_m_sum": c.nearest_m,
                "nearest_mate_bins_lt1_3_6_12_22_40_up": c.nearest_bins,
                "advance_seconds": c.advance.seconds,
                "advance_moved_sum": c.advance.moved,
                "advance_forward_seconds": c.advance.forward,
                "advance_back_seconds": c.advance.back,
                "progress_level_sum": c.advance.level_sum,
                "progress_levels_fifths": c.advance.levels,
                "overlap_1m_apart_none_taken_refused_heldoff": c.apart_state,
                "lane_seconds_centre_over_idle_applied_noroom_unsafe_routebusy": c.lane_seconds,
                "overlap_1m_moving_pair_goal_under5_5_20_20up_none": c.in_file_goals,
                "killed_by_bullet_or_other": c.killed_by,
                "killed_by_shooters_1_2_3": c.shooters,
                "killed_by_killer_still_no_yes": c.killer_still,
                "killed_by_killer_m_10_30_up": c.killer_m,
                "by_mates": c.by_mates.iter().map(|st| serde_json::json!({ "samples": st.samples, "mates_near": st.mates_near, "pairs": st.pairs, "pairs_within_1m": st.pairs_1, "bodies_within_1m": st.body_1, "nearest_bins_lt1_3_6_12_22_40_up": st.nearest })).collect::<Vec<_>>(),
                "pairs_and_within_1m_by_younger_age_lt10_20_40_80_up": c.by_age,
                "moving_alike_pairs_by_gap_lt1_2_4_8_16_and_lateral_lt05_1_2_up": c.file,
                "pairs_by_distance_lt1_2_4_8_16": c.near_pairs,
                "pack_samples_all_and_5plus_within_60m": c.pack_samples,
                "pack_deaths_window_last_all": c.pack_deaths,
                "by_life": c.by_life.iter().map(|[n, mates, alive]| serde_json::json!({ "samples": n, "mates_near": mates, "mates_alive": alive })).collect::<Vec<_>>(),
            });
            if let (Some(all), serde_json::Value::Object(young)) = (json.as_object_mut(), young) {
                all.extend(young);
            }
            json
        };
        let by_play: serde_json::Map<String, serde_json::Value> = self
            .by_play
            .iter()
            .map(|(play, [n, mates, clumped])| {
                (play.clone(), serde_json::json!({ "samples": n, "mates_near": mates, "clumped_samples": clumped }))
            })
            .collect();
        let overlap_plays: serde_json::Value = serde_json::json!([&self.overlap_plays[0], &self.overlap_plays[1]]);
        let json = serde_json::json!({ "spacing": { "by_team": { "0": team(0), "1": team(1) }, "by_play": by_play, "overlap_plays": overlap_plays } });
        let mean = |sum: u64, n: u64| if n == 0 { 0.0 } else { sum as f64 / n as f64 };
        let mean_f = |sum: f64, n: u64| if n == 0 { 0.0 } else { sum / n as f64 };
        let line = |t: usize| {
            let c = &self.by_team[t];
            let ages: Vec<String> = c
                .by_life
                .iter()
                .enumerate()
                .map(|(i, [n, mates, alive])| {
                    let from = i * LIFE_BUCKET_S;
                    let to = if i + 1 == LIFE_BUCKETS { "+".to_string() } else { format!("-{}", from + LIFE_BUCKET_S) };
                    format!("{from}{to} s {:.2} ({:.2} an available mate)", mean(*mates, *n), mean(*mates, *alive))
                })
                .collect();
            let share = |bins: &[u64], from: usize, to: usize| 100.0 * mean(bins[from..to].iter().sum(), c.nearest_n);
            let a = &c.advance;
            format!(
                "nearest mate mean {:.1} m, under 6 m {:.0} %, beyond 40 m {:.0} %; progress {:+.3} a second ({:.0} % forward, {:.0} % back of {} bot-seconds, mean level {:.2}); mates within 30 m {:.2}, enemies {:.2}, 3+ mates {:.0} % of {} body-seconds; deaths {} crowded {} ({:.0} %, mean {:.1} enemies near); by life age: {}",
                mean_f(c.nearest_m, c.nearest_n),
                share(&c.nearest_bins, 0, 3),
                share(&c.nearest_bins, NEAREST_EDGES_M.len(), NEAREST_EDGES_M.len() + 1),
                mean_f(a.moved, a.seconds),
                100.0 * mean(a.forward, a.seconds),
                100.0 * mean(a.back, a.seconds),
                a.seconds,
                mean_f(a.level_sum, a.levels.iter().sum()),
                mean(c.mates, c.samples),
                mean(c.enemies, c.samples),
                100.0 * mean(c.clumped, c.samples),
                c.samples,
                c.deaths,
                c.crowded_deaths,
                100.0 * mean(c.crowded_deaths, c.deaths),
                mean(c.crowd_enemies, c.deaths),
                ages.join(", "),
            )
        };
        let mut cells: Vec<((i32, i32), u64)> = self.cells.iter().map(|(&cell, &n)| (cell, n)).collect();
        cells.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let total: u64 = self.cells.values().sum();
        let hot: Vec<String> = cells
            .iter()
            .take(5)
            .map(|&((x, z), n)| format!("({:.0},{:.0}) {n}", (x as f32 + 0.5) * CELL_M, (z as f32 + 0.5) * CELL_M))
            .collect();
        Part::new(json, format!("owner-test spacing: team 0 [{}]; team 1 [{}]; hot cells of {total} 1 m pair-seconds: {}", line(0), line(1), hot.join(", ")))
    }
}

/// Places 0 and 1 (team 0) stand 10 m apart and four enemies 20-26 m from them: each team-0 body is sampled with one mate
/// and four enemies near, place 0 dies after 3 s with the four near it, so its death is crowded; a team-1 body that dies with
/// no one near it is not.
#[test]
pub(crate) fn the_spacing_topic_counts_mates_and_enemies_near_and_the_crowded_deaths() {
    let (mut game, mut tally) = made_up(&[0, 0, 1, 1, 1, 1, 1]);
    play(&mut game, &mut tally, 80, |k, game| {
        for (place, x) in [0.0f32, 10.0, 20.0, 22.0, 24.0, 26.0].into_iter().enumerate() {
            put(game, place, x, 5.0, 0);
        }
        // Team 1's place 6 stands alone, far away, and dies after 3 s.
        put(game, 6, 75.0, 35.0, 0);
        if k >= 30 {
            game.bodies[0].alive = false;
            game.bodies[6].alive = false;
        }
        vec![bot_now(); 7]
    });
    let (_, json) = tally.finish(&game);
    let team0 = &json["spacing"]["by_team"]["0"];
    let team1 = &json["spacing"]["by_team"]["1"];
    assert!(team0["samples"].as_u64().unwrap() >= 3, "{json}");
    assert_eq!(team0["deaths"], 1, "{json}");
    assert_eq!(team0["crowded_deaths"], 1, "four enemies near before the death: {json}");
    assert_eq!(team0["crowd_enemies"], 4);
    assert_eq!(team1["deaths"], 1);
    assert_eq!(team1["crowded_deaths"], 0, "alone when it died");
    // Each team-0 body was sampled with the other within 30 m, until place 0 died.
    assert!(team0["mates_near"].as_u64().unwrap() >= 2);
    // Their lives from the match's start are no respawns: no life-age samples.
    assert_eq!(team0["by_life"][0]["samples"], 0, "{json}");
}

/// A body that comes alive again is sampled by its life's age: place 1 (team 0) dies at 1.5 s and is back at 2.5 s beside its
/// mate, so its samples from then on are in the 0-10 s bucket, each with one mate near of one alive.
#[test]
pub(crate) fn a_respawned_body_is_sampled_by_its_lifes_age() {
    let (mut game, mut tally) = made_up(&[0, 0, 1]);
    play(&mut game, &mut tally, 160, |k, game| {
        put(game, 0, 0.0, 5.0, 0);
        put(game, 1, 10.0, 5.0, 0);
        put(game, 2, 80.0, 40.0, 0);
        game.bodies[1].alive = !(30..50).contains(&k);
        vec![bot_now(); 3]
    });
    let (_, json) = tally.finish(&game);
    let young = &json["spacing"]["by_team"]["0"]["by_life"][0];
    assert!(young["samples"].as_u64().unwrap() >= 5, "{json}");
    assert_eq!(young["mates_near"], young["mates_alive"], "one mate near of one alive: {json}");
    assert_eq!(json["spacing"]["by_team"]["0"]["by_life"][1]["samples"], 0, "{json}");
}

/// The nearest teammate of each sampled body: places 0 and 1 (team 0) stand 10 m apart and place 2 (team 0) is 30 m from place 1, so each second
/// has two samples in the 6-12 m bin and one in the 22-40 m bin; a lone body (team 1) has no mate and no sample.
#[test]
pub(crate) fn the_spacing_topic_counts_the_distance_to_the_nearest_mate() {
    let (mut game, mut tally) = made_up(&[0, 0, 0, 1]);
    play(&mut game, &mut tally, 80, |_, game| {
        for (place, x) in [0.0f32, 10.0, 40.0].into_iter().enumerate() {
            put(game, place, x, 5.0, 0);
        }
        put(game, 3, 70.0, 40.0, 0);
        vec![bot_now(); 4]
    });
    let (_, json) = tally.finish(&game);
    let team0 = &json["spacing"]["by_team"]["0"];
    let bins = &team0["nearest_mate_bins_lt1_3_6_12_22_40_up"];
    let n = team0["nearest_mate_samples"].as_u64().unwrap();
    assert!(n >= 6, "{json}");
    // Places 0 and 1 are 10 m from each other (6-12 m, bin 3); place 2 is 30 m from place 1 (22-40 m, bin 5).
    assert_eq!(bins[3].as_u64().unwrap() + bins[5].as_u64().unwrap(), n, "{json}");
    assert_eq!(bins[3].as_u64().unwrap(), 2 * bins[5].as_u64().unwrap(), "{json}");
    assert_eq!(json["spacing"]["by_team"]["1"]["nearest_mate_samples"], 0, "no mate alive: {json}");
    let mean = team0["nearest_mate_m_sum"].as_f64().unwrap() / n as f64;
    assert!((mean - (10.0 + 10.0 + 30.0) / 3.0).abs() < 0.01, "{mean}");
}

/// The nearest-mate bins: under 1, 3, 6, 12, 22 and 40 m, then farther.
#[test]
pub(crate) fn the_nearest_mate_bins_are_by_the_live_yardstick_edges() {
    let bins: Vec<usize> = [0.5, 1.0, 2.9, 5.9, 11.9, 21.9, 39.9, 40.0, 90.0].into_iter().map(nearest_bin).collect();
    assert_eq!(bins, [0, 1, 1, 2, 3, 4, 5, 6, 6]);
}

/// The advance: a second's move of the progress is forward or back by [`ADVANCE_EPS`]; the first sample of a life has no move but has its level.
#[test]
pub(crate) fn the_advance_counts_the_progress_a_second_on_and_the_level() {
    let mut a = Advance::default();
    a.add(None, 0.10);
    a.add(Some(0.10), 0.13);
    a.add(Some(0.13), 0.125);
    a.add(Some(0.125), 0.10);
    a.add(Some(0.10), 1.0);
    assert_eq!((a.seconds, a.forward, a.back), (4, 2, 1), "{a:?}");
    assert!((a.moved - 0.90).abs() < 1e-6, "{a:?}");
    assert_eq!(a.levels, [4, 0, 0, 0, 1], "{a:?}");
    assert!((a.level_sum - (0.10 + 0.13 + 0.125 + 0.10 + 1.0)).abs() < 1e-6);
}

/// The young pairs (A212): place 1 (team 0) dies at 0.5 s and comes back at 1.5 s, then walks with place 0 0.6 m beside it from the spawn point at (5, 10) along x: every 1 m
/// pair-second is young, of one opening life and one respawn, and each of its splits (cause, kind, width, shape, origin, step apart) counts it once; the near ones are within 3 m of
/// the spawn point; the open field is open ground across the heading.
#[test]
pub(crate) fn the_young_overlap_is_split_by_kind_spawn_distance_width_and_origin() {
    let (mut game, mut tally) = made_up(&[0, 0, 1]);
    play(&mut game, &mut tally, 120, |k, game| {
        let x = 5.5 + 0.08 * k.saturating_sub(30) as f32;
        put(game, 0, x, 10.0, key::FORWARD);
        // Place 1 stands far off until it dies, so no pair-second is of two opening lives.
        put(game, 1, if k < 10 { 30.0 } else { x }, 10.6, key::FORWARD);
        put(game, 2, 75.0, 30.0, 0);
        game.bodies[1].alive = !(10..30).contains(&k);
        vec![bot_now(); 3]
    });
    let (_, json) = tally.finish(&game);
    let team0 = &json["spacing"]["by_team"]["0"];
    let sum = |key: &str| team0[key].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).sum::<u64>();
    let young = sum("young_1m_cause_target_goal_follow_still_file_other");
    assert!(young >= 3, "{json}");
    assert_eq!(team0["young_1m_kind_opening_mixed_respawn"][1].as_u64().unwrap(), young, "one opening life and one respawn: {json}");
    // The pairs' denominator holds every young pair-second of one opening and one respawned life (all on one floor), the overlap's among them.
    assert!(team0["young_pairs_by_kind_opening_mixed_respawn"][1].as_u64().unwrap() >= young, "{json}");
    // Only place 0's life is of the opening (place 1's is a respawn, place 2 never moves): one first move, the team's earliest.
    assert_eq!(team0["opening_first_move_ticks_sum_and_n"][1], 1, "{json}");
    assert_eq!(team0["opening_first_move_spread_s_lt025_075_15_3_up"][0], 1, "{json}");
    assert_eq!(json["spacing"]["by_team"]["1"]["opening_first_move_ticks_sum_and_n"][1], 0, "{json}");
    for key in ["young_1m_by_width_m_lt15_25_4_75_open_noheading", "young_1m_shape_file_abreast_crossing_standing", "young_1m_origin_together_adjacent_together_apart_different", "young_1m_apart_none_taken_refused_heldoff_eased_otherclear_tied", "young_1m_ahead_one_both_neither_nowalk"] {
        assert_eq!(sum(key), young, "{key}: {json}");
    }
    assert_eq!(sum("all_1m_by_width_m_lt15_25_4_75_open_noheading"), team0["overlap_1m_pair_s"].as_u64().unwrap(), "{json}");
    let spawn = &team0["young_near_pairs_and_1m_by_spawn_m_lt3_6_13_25_up"];
    assert!(spawn[0][1].as_u64().unwrap() > 0, "near the spawn point: {json}");
    let within: u64 = spawn.as_array().unwrap().iter().map(|bin| bin[1].as_u64().unwrap()).sum();
    assert_eq!(within, young, "{json}");
    // Born 1 s apart on points 0.6 m apart: together, adjacent.
    assert_eq!(team0["young_1m_origin_together_adjacent_together_apart_different"][0].as_u64().unwrap(), young, "{json}");
    // The soak field is 40 m deep: open ground across a walk along x, unless no heading is known.
    let width = &team0["young_1m_by_width_m_lt15_25_4_75_open_noheading"];
    assert_eq!(width[4].as_u64().unwrap() + width[5].as_u64().unwrap(), young, "{json}");
}
