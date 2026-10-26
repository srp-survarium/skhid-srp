use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use mapdata::{LinkKind, NodeId};

use super::*;

/// The moving counts: counted as the match runs, kept for [`OwnerTally::finish`].
#[derive(Default)]
pub(super) struct MoveCounts {
    pub(super) takes: Vec<Vec<u32>>,
    pub(super) stuck_notes: Vec<Vec<u32>>,
    pub(super) no_way: Vec<Vec<u32>>,
}

impl MoveCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            takes: vec![Vec::new(); n],
            stuck_notes: vec![Vec::new(); n],
            no_way: vec![Vec::new(); n],
        }
    }
}

/// Idle (`idle.py`): seconds under 0.3 m moved, walk keys under 30 % of
/// the ticks, calm; at least 5 of them in a row.
pub(crate) const IDLE_M: f32 = 0.3;

pub(crate) const IDLE_WALK: f32 = 0.3;

pub(crate) const IDLE_S: usize = 5;

/// Drops (`drops.py`): the feet down this far between two worlds, m.
pub(crate) const DROP_M: [f32; 2] = [2.5, 4.0];

/// Verticality (`grid-record.md` section V: `drops_make_pairs.py`, `drops_analyse.py`,
/// `height_share.py`): the bodies' samples 5 s apart, as the players' worlds are; a descent or a
/// climb of this much between two samples of a body alive throughout, m.
pub(crate) const VERT_M: f32 = 1.5;

pub(crate) const VERT_SAMPLE_TICKS: usize = 5 * TPS;

/// A "certain drop": no walk (Walk links only) from where the body was to where it is within what
/// it could walk between the samples: this fast, m/s, and this much more, m.
pub(crate) const VERT_WALK_MPS: f32 = 5.3;

pub(crate) const VERT_WALK_SLACK_M: f32 = 2.0;

/// The drops and climbs a kilometre of the players on a level.
type PlayersRates = Option<(f32, f32)>;

/// The players' values by level (live matches of 12+ places, 5 s worlds, 379 matches): the height
/// of their feet at the 75th percentile, m, and their certain drops and climbs of 1.5 m or more a
/// kilometre walked (none for Tarakanovsky Fort: no bot match was read). `high_time_pct` is the
/// share of the bots' time above that height; the players' own is 25 %.
pub(crate) const PLAYERS_VERT: [(&str, f32, PlayersRates); 8] = [
    ("level_01", 22.34, Some((2.01, 7.94))),
    ("level_02", 19.49, Some((3.53, 6.09))),
    ("level_03", 5.62, Some((1.22, 10.11))),
    ("level_04", 9.55, Some((4.23, 6.53))),
    ("level_05", 8.34, None),
    ("level_06", 36.80, Some((5.07, 17.78))),
    ("level_07", 21.12, Some((4.63, 3.83))),
    ("level_08", 35.03, Some((1.12, 11.67))),
];

/// Whether a body walks (Walk links only) from `from` to `to` within `budget` m of walking.
fn walks_within(grid: &WalkGrid, from: NodeId, to: NodeId, budget: f32) -> bool {
    let limit = (budget * 1000.0) as u32;
    let mut best: HashMap<NodeId, u32> = HashMap::from([(from, 0)]);
    let mut heap = BinaryHeap::from([(Reverse(0u32), from)]);
    while let Some((Reverse(d), a)) = heap.pop() {
        if a == to {
            return true;
        }
        if best.get(&a).is_some_and(|&b| d > b) {
            continue;
        }
        let at = grid.position(a);
        for (b, kind) in grid.links(a) {
            if kind != LinkKind::Walk {
                continue;
            }
            let step = ((grid.position(b) - at).length() * 1000.0) as u32;
            let nd = d + step;
            if nd <= limit && best.get(&b).is_none_or(|&known| nd < known) {
                best.insert(b, nd);
                heap.push((Reverse(nd), b));
            }
        }
    }
    false
}

/// U-turns (`uturns.py`): the moves 2.2 s before and after a take, each
/// at least 2 m, more than 120 degrees apart.
pub(crate) const UTURN_TICKS: usize = 44;

pub(crate) const UTURN_M: f32 = 2.0;

pub(crate) const UTURN_DEG: f32 = 120.0;

/// Close pairs (`teamwork.py`): teammates within 3 m for 5 s or more,
/// gaps under 2.5 s; one body (`overlap.py`): two bodies within 0.5 m
/// across and in height.
pub(crate) const CLOSE_M: f32 = 3.0;

pub(crate) const CLOSE_S: u32 = 5;

pub(crate) const CLOSE_GAP_S: u32 = 2;

pub(crate) const ONE_BODY_M: f32 = 0.5;

/// Two bodies on one spot are still when neither moves this far in the
/// second after, metres.
pub(crate) const ONE_BODY_STILL_M: f32 = 0.25;

/// The stuck detector (`navigation/stuck.py`, `navigation.md` §2d, final).
/// A designed hold's steps (and the Heal play); the steps with a goal to
/// walk to; the Follow-type steps, a goal only with the leader (the goal)
/// farther than [`LEADER_FAR_M`].
pub(crate) const HOLD_STEPS: [&str; 13] = [
    "Hold", "Holding", "Wait", "Pair", "Pair)", "Staying", "Looking", "Peek", "Trade", "Take",
    "PutIn", "LetGo", "NoSpot)",
];

pub(crate) const TRAVEL_STEPS: [&str; 14] = [
    "Going", "Route", "Via", "Fetch)", "Search)", "Search", "Bring", "Bring)", "Approach)",
    "Approach", "Chase", "Round", "Seek)", "Cut",
];

pub(crate) const FOLLOW_STEPS: [&str; 4] = ["Follow", "Follow)", "Behind", "Ahead"];

pub(crate) const LEADER_FAR_M: f32 = 20.0;

/// Its window kinds: the least seconds, the radius about the positions'
/// middle (m), the least share of ticks with a walk key, the share of aim
/// or fire under which it counts (none: any).
pub(crate) const STUCK_KINDS: [(&str, f32, f32, f32, Option<f32>); 3] = [
    ("push", 3.0, 0.6, 0.75, None),
    ("nohead", 10.0, 2.5, 0.70, Some(0.35)),
    ("loop", 20.0, 8.0, 0.80, Some(0.35)),
];

/// An episode's kind: the first of these it holds.
pub(crate) const STUCK_ORDER: [&str; 5] = ["push", "nohead", "loop", "stand", "watch"];

pub(crate) const PUSH_STILL_SPEED: f32 = 0.5;

pub(crate) const PUSH_OPPOSING_MAX: f32 = 0.25;

pub(crate) const PUSH_SPEED: f32 = 1.2;

pub(crate) const NOHEAD_HOLD_MAX: f32 = 0.5;

pub(crate) const STAND_S: f32 = 10.0;

pub(crate) const STAND_R: f32 = 0.6;

pub(crate) const STAND_WALK_MAX: f32 = 0.25;

pub(crate) const STAND_AIM_MAX: f32 = 0.5;

pub(crate) const STAND_TRAVEL: f32 = 0.7;

pub(crate) const LOOP_NET_M: f32 = 4.0;

pub(crate) const LOOP_PATH_M: f32 = 12.0;

pub(crate) const LOOP_RISE_M: f32 = 2.5;

/// The server's stuck watch firing: the body within this of where it was
/// then, m, with walk keys on this share between samples, for this long.
pub(crate) const WATCH_M: f32 = 1.0;

pub(crate) const WATCH_WALK: f32 = 0.5;

pub(crate) const WATCH_S: f32 = 3.0;

/// A fight dance (not stuck): aim or fire on this share, median speed
/// over this, m/s, opposing keys over this share.
pub(crate) const DANCE_AIM: f32 = 0.5;

pub(crate) const DANCE_MPS: f32 = 1.2;

pub(crate) const DANCE_OPPOSING: f32 = 0.25;

pub(crate) const MERGE_GAP_S: f32 = 2.0;

pub(crate) const SPAWN_GRACE_S: f32 = 3.0;

/// Causes: a teammate (or an enemy) this near, m; the feet up and down
/// this much, m.
pub(crate) const MATE_NEAR_M: f32 = 1.2;

pub(crate) const UP_DOWN_M: f32 = 0.8;

/// Spots with two or more cases: this across and this in height, m
/// (`stucktable.py`).
pub(crate) const SPOT_M: [f32; 2] = [4.0, 2.0];

/// A stuck episode: its start and end, s; its kinds (the first in
/// [`STUCK_ORDER`] first); its cause; where.
#[derive(Clone, Debug)]
pub(crate) struct Episode {
    /// The record index of its start.
    pub(crate) k0: usize,
    pub(crate) t0: f32,
    pub(crate) t1: f32,
    pub(crate) kinds: Vec<&'static str>,
    pub(crate) cause: &'static str,
    pub(crate) at: Vec3,
}

/// A bot's state by its play and step (§2d): in a designed hold; in a step
/// with a goal to walk to (a Follow-type one with its goal `goal_m` over
/// [`LEADER_FAR_M`] away).
pub(crate) fn status_of(play: &str, step: &str, goal_m: Option<f32>) -> (bool, bool) {
    let hold = play == "Heal" || HOLD_STEPS.contains(&step);
    let travel = !hold
        && (TRAVEL_STEPS.contains(&step)
            || (FOLLOW_STEPS.contains(&step) && goal_m.is_some_and(|m| m > LEADER_FAR_M)));
    (hold, travel)
}

impl OwnerTally {
    /// A bot's note: takes, stuck watches, plays ended for no way to the
    /// goal, grenades thrown.
    pub(crate) fn note(&mut self, note: &Note, game: &Game) {
        let at = |list: &mut Vec<Vec<u32>>| {
            if let Some(t) = list.get_mut(note.place) {
                t.push(note.time);
            }
        };
        if let Some(rest) = note.text.strip_prefix("takes ") {
            at(&mut self.moving.takes);
            let mut words = rest.split_whitespace();
            let (kind, target) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
            let team = game.bodies.get(note.place).map(|b| b.team);
            if kind == "Trade"
                && let Some(team) = team
            {
                self.campers.trades.push((note.place, team, note.time));
            }
            // A counter run: Flank or Crossfire on a shooter who holds his
            // kill spot against this bot's side.
            let on = target
                .strip_prefix("Place(")
                .and_then(|t| t.strip_suffix(')'))
                .and_then(|t| t.parse::<Place>().ok());
            if matches!(kind, "Flank" | "Crossfire")
                && let (Some(who), Some(team)) = (on, team)
                && self.campers.spots.iter().any(|h| h.shooter == who && h.team == team)
            {
                self.campers.counter_runs[0] += 1;
                self.campers.counter_runs[1] += u32::from(kind == "Flank");
            }
        } else if note.text.starts_with("is stuck at") {
            at(&mut self.moving.stuck_notes);
        } else if note.text.contains("no way to its goal") {
            at(&mut self.moving.no_way);
        } else if let Some((aim, time)) = thrown_at(&note.text) {
            let team = game.bodies.get(note.place).map_or(0, |b| b.team);
            self.grenade_thrown(note.place, team, aim, time);
        } else if let Some(why) = held_back(&note.text) {
            self.grenade_refused(why);
            // A throw given up in its hold (a mate walked in) is over: the shots after it are the bot's own.
            self.grenade_given_up(note.place);
        }
    }

    /// One bot's stuck episodes (`stuck.py`, `navigation.md` §2d) and its
    /// blips (stuck watches in no episode). From a sample a second of each
    /// life (a life breaks at a death, or where the body moves over 25 m
    /// in a second: a new spawn), the windows of each kind, merged when at
    /// most 2 s apart; each episode's cause. The bench has no start
    /// countdown, so nothing is left out at the start.
    pub(crate) fn stuck_episodes(&self, place: Place, teams: &[u8], presses: &[Press]) -> (Vec<Episode>, u32) {
        let recs = &self.recs[place];
        let secs: Vec<usize> = (0..recs.len() / TPS).map(|s| s * TPS).collect();
        // Lives: runs of living samples.
        let mut lives: Vec<Vec<usize>> = Vec::new();
        let mut cur: Vec<usize> = Vec::new();
        for &k in &secs {
            if !recs[k].alive {
                if !cur.is_empty() {
                    lives.push(std::mem::take(&mut cur));
                }
                continue;
            }
            if let Some(&last) = cur.last()
                && recs[last].feet.distance_xz(recs[k].feet) > 25.0
            {
                lives.push(std::mem::take(&mut cur));
            }
            cur.push(k);
        }
        if !cur.is_empty() {
            lives.push(cur);
        }
        let t_of = |k: usize| self.time_of(k) as f32 / 1000.0;
        // Ticks so far with a walk key, with aim or fire, and walking with
        // forward, back, left, right: a window's shares at once.
        let (walking, aiming) = (0, 1);
        let mut sums = vec![[0u32; 6]; recs.len() + 1];
        for (k, r) in recs.iter().enumerate() {
            let walk = r.keys & WALK_KEYS != 0;
            let mut now = sums[k];
            now[0] += u32::from(walk);
            now[1] += u32::from(r.keys & (key::AIM | key::FIRE) != 0);
            for (i, bit) in [key::FORWARD, key::BACK, key::LEFT, key::RIGHT].into_iter().enumerate() {
                now[2 + i] += u32::from(walk && r.keys & bit != 0);
            }
            sums[k + 1] = now;
        }
        let count = |k0: usize, k1: usize, i: usize| sums[k1 + 1][i] - sums[k0][i];
        let share = |k0: usize, k1: usize, i: usize| count(k0, k1, i) as f32 / (k1 - k0 + 1) as f32;
        let opposing = |k0: usize, k1: usize| {
            let n = count(k0, k1, walking);
            if n == 0 {
                0.0
            } else {
                let c = |i: usize| count(k0, k1, 2 + i);
                (c(0).min(c(1)) + c(2).min(c(3))) as f32 / n as f32
            }
        };
        let median_speed = |pts: &[usize]| {
            let mut sp: Vec<f32> = pts.iter().map(|&k| recs[k].speed).collect();
            sp.sort_by(f32::total_cmp);
            (sp[(sp.len() - 1) / 2] + sp[sp.len() / 2]) / 2.0
        };
        let stuck_line = |t0: f32, t1: f32| {
            self.moving.stuck_notes[place]
                .iter()
                .any(|&t| (t as f32) >= t0 * 1000.0 && (t as f32) <= t1 * 1000.0)
        };
        let radius = |pts: &[usize]| {
            let n = pts.len() as f32;
            let cx = pts.iter().map(|&k| recs[k].feet.x).sum::<f32>() / n;
            let cz = pts.iter().map(|&k| recs[k].feet.z).sum::<f32>() / n;
            pts.iter()
                .map(|&k| (recs[k].feet.x - cx).hypot(recs[k].feet.z - cz))
                .fold(0.0, f32::max)
        };
        // Windows: start, end (record indexes), kind.
        let mut marks: Vec<(usize, usize, &'static str)> = Vec::new();
        for life in &lives {
            let n = life.len();
            let t_spawn = t_of(life[0]);
            // The smallest window from `i` of `secs` s, extended while
            // `ok` holds.
            let window = |i: usize, secs: f32, ok: &dyn Fn(usize) -> bool| -> Option<usize> {
                let mut j = i;
                while j < n && t_of(life[j]) - t_of(life[i]) < secs {
                    j += 1;
                }
                if j >= n || j - i + 1 < 3 || !ok(j) {
                    return None;
                }
                while j + 1 < n && ok(j + 1) {
                    j += 1;
                }
                Some(j)
            };
            for i in 0..n {
                for &(name, dmin, rmax, wmin, amax) in &STUCK_KINDS {
                    let ok = |j: usize| -> bool {
                        let pts = &life[i..=j];
                        let (k0, k1) = (pts[0], pts[pts.len() - 1]);
                        if radius(pts) > rmax || share(k0, k1, walking) < wmin {
                            return false;
                        }
                        if amax.is_some_and(|amax| share(k0, k1, aiming) >= amax) {
                            return false;
                        }
                        let held = pts.iter().filter(|&&k| recs[k].hold).count() as f32 / pts.len() as f32;
                        match name {
                            "push" => {
                                let med = median_speed(pts);
                                med <= PUSH_STILL_SPEED
                                    || (opposing(k0, k1) <= PUSH_OPPOSING_MAX
                                        && (med <= PUSH_SPEED || stuck_line(t_of(k0), t_of(k1) + 1.5)))
                            }
                            "nohead" => held < NOHEAD_HOLD_MAX,
                            _ => {
                                let (a, b) = (recs[k0].feet, recs[k1].feet);
                                let path: f32 = pts.windows(2).map(|w| recs[w[0]].feet.distance_xz(recs[w[1]].feet)).sum();
                                held == 0.0
                                    && a.distance_xz(b) <= LOOP_NET_M
                                    && (b.y - a.y).abs() < LOOP_RISE_M
                                    && path >= LOOP_PATH_M
                            }
                        }
                    };
                    if let Some(j) = window(i, dmin, &ok) {
                        marks.push((life[i], life[j], name));
                    }
                }
                if t_of(life[i]) - t_spawn < SPAWN_GRACE_S {
                    continue;
                }
                let ok_stand = |j: usize| -> bool {
                    let pts = &life[i..=j];
                    let (k0, k1) = (pts[0], pts[pts.len() - 1]);
                    radius(pts) <= STAND_R
                        && share(k0, k1, walking) <= STAND_WALK_MAX
                        && share(k0, k1, aiming) < STAND_AIM_MAX
                        && pts.iter().filter(|&&k| recs[k].travel).count() as f32
                            >= STAND_TRAVEL * pts.len() as f32
                };
                if let Some(j) = window(i, STAND_S, &ok_stand) {
                    marks.push((life[i], life[j], "stand"));
                }
            }
            // The server's stuck watch: from each firing, both ways while
            // the body keeps near where it was then, walking, not holding.
            let (lt0, lt1) = (t_of(life[0]), t_of(life[n - 1]));
            for &fired in &self.moving.stuck_notes[place] {
                let tf = fired as f32 / 1000.0;
                if tf < lt0 || tf > lt1 {
                    continue;
                }
                let Some(m) = (0..n).find(|&m| t_of(life[m]) >= tf) else {
                    continue;
                };
                let p = recs[self.index(fired).min(recs.len() - 1)].feet;
                let fits = |m: usize| recs[life[m]].feet.distance_xz(p) <= WATCH_M && !recs[life[m]].hold;
                let walks = |a: usize, b: usize| share(life[a], life[b], walking) >= WATCH_WALK;
                if !fits(m) {
                    continue;
                }
                let (mut i, mut j) = (m, m);
                while j + 1 < n && fits(j + 1) && walks(j, j + 1) {
                    j += 1;
                }
                while i > 0 && fits(i - 1) && walks(i - 1, i) {
                    i -= 1;
                }
                let (k0, k1) = (life[i], life[j]);
                if t_of(k1) - t_of(k0) < WATCH_S || j - i + 1 < 3 {
                    continue;
                }
                let dance = share(k0, k1, aiming) >= DANCE_AIM
                    && median_speed(&life[i..=j]) > DANCE_MPS
                    && opposing(k0, k1) > DANCE_OPPOSING;
                if !dance {
                    marks.push((k0, k1, "watch"));
                }
            }
        }
        // Episodes: windows merged when at most 2 s apart.
        let order = |k: &str| STUCK_ORDER.iter().position(|o| *o == k).unwrap_or(STUCK_ORDER.len());
        marks.sort_by_key(|m| (m.0, m.1));
        let mut merged: Vec<(usize, usize, Vec<&'static str>)> = Vec::new();
        for (k0, k1, kind) in marks {
            match merged.last_mut() {
                Some(e) if t_of(k0) <= t_of(e.1) + MERGE_GAP_S => {
                    e.1 = e.1.max(k1);
                    if !e.2.contains(&kind) {
                        e.2.push(kind);
                    }
                }
                _ => merged.push((k0, k1, vec![kind])),
            }
        }
        let mut eps = Vec::new();
        for (k0, k1, mut kinds) in merged {
            kinds.sort_by_key(|k| order(k));
            let (t0, t1) = (t_of(k0), t_of(k1));
            let pts: Vec<usize> = (k0..=k1).step_by(TPS).collect();
            let within = |list: &[u32], a: f32, b: f32| {
                list.iter().filter(|&&t| (t as f32) >= a * 1000.0 && (t as f32) <= b * 1000.0).count()
            };
            // Samples with a body of one side (own or other) within reach.
            let near = |own: bool| {
                pts.iter()
                    .filter(|&&k| {
                        (0..self.recs.len()).any(|q| {
                            q != place
                                && (teams.get(q) == teams.get(place)) == own
                                && self.recs[q].get(k).is_some_and(|r| {
                                    r.alive && r.feet.distance_xz(recs[k].feet) <= MATE_NEAR_M
                                })
                        })
                    })
                    .count()
            };
            let (lo, hi) = pts.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &k| {
                (lo.min(recs[k].feet.y), hi.max(recs[k].feet.y))
            });
            let route_jump = presses
                .iter()
                .any(|p| p.place == place && p.by == "route" && (p.t as f32) >= (t0 - 2.0) * 1000.0 && (p.t as f32) <= t1 * 1000.0);
            let cause = if within(&self.moving.no_way[place], t0 - 1.0, t1 + 3.0) > 0 {
                "no_route"
            } else if 2 * near(true) >= pts.len() {
                "teammate"
            } else if 2 * near(false) >= pts.len() {
                "enemy"
            } else if route_jump {
                "route_jump"
            } else if hi - lo >= UP_DOWN_M {
                "ledge"
            } else if kinds.contains(&"loop") && within(&self.moving.takes[place], t0, t1) >= 2 {
                "play_churn"
            } else if kinds[0] == "stand" {
                "stand"
            } else if share(k0, k1, aiming) >= DANCE_AIM {
                "fight"
            } else if stuck_line(t0, t1 + 1.5) {
                "wall"
            } else {
                "wall_no_line"
            };
            let at = pts.iter().fold(Vec3::ZERO, |a, &k| a + recs[k].feet) * (1.0 / pts.len() as f32);
            eps.push(Episode {
                k0,
                t0,
                t1,
                kinds,
                cause,
                at,
            });
        }
        let blips = self.moving.stuck_notes[place]
            .iter()
            .filter(|&&ms| {
                let t = ms as f32 / 1000.0;
                let alive = recs.get(self.index(ms)).is_some_and(|r| r.alive);
                alive && !eps.iter().any(|e| t >= e.t0 - 1.0 && t <= e.t1 + 1.0)
            })
            .count() as u32;
        (eps, blips)
    }

    /// The verticality counts at the end: certain drops and climbs of 1.5 m or more a kilometre
    /// walked by the bots (the players' in the line of text), and the share of the bots' time
    /// above the players' 75th percentile height on the map.
    pub(super) fn vertical_part(&self, game: &Game) -> Part {
        let grid = &game.world.grid;
        let level = PLAYERS_VERT
            .iter()
            .find(|(name, ..)| game.world.map.project.starts_with(name));
        let (mut drops, mut climbs, mut km) = (0u32, 0u32, 0f64);
        let (mut high, mut alive) = (0u64, 0u64);
        let budget = VERT_WALK_MPS * VERT_SAMPLE_TICKS as f32 * TICK_S + VERT_WALK_SLACK_M;
        for recs in self.recs.iter().enumerate().filter(|&(p, _)| self.is_bot[p]).map(|(_, r)| r) {
            for r in recs.iter().filter(|r| r.alive) {
                alive += 1;
                high += u64::from(level.is_some_and(|&(_, p75, _)| r.feet.y >= p75));
            }
            let mut s = 0;
            while s + VERT_SAMPLE_TICKS < recs.len() {
                let (a, b) = (&recs[s], &recs[s + VERT_SAMPLE_TICKS]);
                s += VERT_SAMPLE_TICKS;
                if !recs[s - VERT_SAMPLE_TICKS..=s].iter().all(|r| r.alive) {
                    continue;
                }
                km += f64::from((b.feet.x - a.feet.x).hypot(b.feet.z - a.feet.z)) / 1000.0;
                let rise = b.feet.y - a.feet.y;
                if rise >= VERT_M {
                    climbs += 1;
                } else if rise <= -VERT_M
                    && let (Some(from), Some(to)) = (grid.locate(a.feet, 1.0), grid.locate(b.feet, 1.0))
                    && !walks_within(grid, from, to, budget)
                {
                    drops += 1;
                }
            }
        }
        let per_km = |n: u32| if km > 0.0 { (100.0 * f64::from(n) / km).round() / 100.0 } else { 0.0 };
        let high_pct = Scale::pct(high as f64, alive as f64);
        let json = serde_json::json!({
            "drops_certain": drops,
            "climbs_1_5m": climbs,
            "km_walked": (km * 100.0).round() / 100.0,
            "drops_per_km": per_km(drops),
            "climbs_per_km": per_km(climbs),
            "high_time_pct": high_pct,
        });
        let players = level
            .map(|&(name, p75, rates)| match rates {
                Some((d, c)) => format!("players on {name}: {d:.2} drops, {c:.2} climbs a km, p75 {p75:.1} m"),
                None => format!("players on {name}: p75 {p75:.1} m"),
            })
            .unwrap_or_else(|| "no players' values for this map".to_owned());
        let text = format!(
            "owner-test vertical: certain drops {drops} ({:.2} a km), climbs of 1.5 m+ {climbs} ({:.2} a km) over {km:.1} km walked; {high_pct:.1} % of the time above the players' p75 height ({players}; players 25 %)",
            per_km(drops),
            per_km(climbs),
        );
        Part::new(json, text)
    }

    /// The moving counts at the end: their keys of the `owner` object and their lines of the owner-test text.
    pub(super) fn moving_part(&self, game: &Game, presses: &[Press], scale: &Scale) -> Part {
        let n = self.recs.len();
        let tick_s = scale.tick_s;
        let per_min = |x: f64| scale.per_min(x);
        let pct = Scale::pct;
        // Calm walking and sprinting; idle; drops; U-turns.
        let (mut calm_walk, mut calm_sprint, mut calm_m) = (0u64, 0u64, 0f64);
        let mut idle: BTreeMap<String, f64> = BTreeMap::new();
        let (mut idle_s, mut drops) = (0u32, [0u32; 2]);
        let (mut uturns, mut takes_moving) = (0u32, 0u32);
        for (place, recs) in self.recs.iter().enumerate() {
            for r in recs.iter().filter(|r| r.alive && r.calm && !r.carrying) {
                if r.keys & WALK_KEYS != 0 {
                    calm_walk += 1;
                    calm_sprint += u64::from(r.keys & key::SPRINT != 0);
                    calm_m += f64::from(r.speed) * tick_s;
                }
            }
            // Seconds: idle ones, and the drops between them.
            let mut streak: Vec<u16> = Vec::new();
            let mut flush = |streak: &mut Vec<u16>, idle: &mut BTreeMap<String, f64>| {
                if streak.len() >= IDLE_S {
                    idle_s += streak.len() as u32;
                    for &p in streak.iter() {
                        *idle.entry(self.plays[usize::from(p)].clone()).or_default() += 1.0;
                    }
                }
                streak.clear();
            };
            for s in 0..recs.len() / TPS {
                let sec = &recs[s * TPS..(s + 1) * TPS];
                let whole = sec.iter().all(|r| r.alive);
                let moved: f32 = sec.iter().map(|r| r.speed * TICK_S).sum();
                let walk = sec.iter().filter(|r| r.keys & WALK_KEYS != 0).count() as f32 / TPS as f32;
                let is_idle = whole && moved < IDLE_M && walk < IDLE_WALK && sec.iter().all(|r| r.calm);
                if is_idle {
                    streak.push(sec[TPS - 1].play);
                } else {
                    flush(&mut streak, &mut idle);
                }
                if whole && s > 0 && recs[s * TPS - 1].alive {
                    let down = recs[s * TPS - 1].feet.y - sec[TPS - 1].feet.y;
                    for (c, m) in drops.iter_mut().zip(DROP_M) {
                        *c += u32::from(down >= m);
                    }
                }
            }
            flush(&mut streak, &mut idle);
            for &t in &self.moving.takes[place] {
                let i = self.index(t);
                if i < UTURN_TICKS || i + UTURN_TICKS >= recs.len() {
                    continue;
                }
                let (a, b, c) = (&recs[i - UTURN_TICKS], &recs[i], &recs[i + UTURN_TICKS]);
                if !(a.alive && b.alive && c.alive) || recs[i - UTURN_TICKS..=i + UTURN_TICKS].iter().any(|r| !r.alive) {
                    continue;
                }
                let (u, v) = (b.feet - a.feet, c.feet - b.feet);
                let (lu, lv) = (u.x.hypot(u.z), v.x.hypot(v.z));
                if lu < UTURN_M || lv < UTURN_M {
                    continue;
                }
                takes_moving += 1;
                let cos = ((u.x * v.x + u.z * v.z) / (lu * lv)).clamp(-1.0, 1.0);
                uturns += u32::from(cos.acos().to_degrees() > UTURN_DEG);
            }
        }

        // Close pairs (teammates, from the seconds) and one body (any two,
        // every tick).
        let (mut close_eps, mut close_s, mut one_eps, mut one_ticks) = (0u32, 0u32, 0u32, 0u32);
        // The same, where both bodies are still (under ONE_BODY_STILL_M in the
        // second after): two bots standing on one spot, not walking one path.
        let (mut still_eps, mut still_ticks) = (0u32, 0u32);
        // And of those, the pairs of plays that stand a bot beside a teammate
        // by their own spot choice, not by the batteries' stands (Collect), the
        // spawn (no play) or Retreat's: narrowed after the trace of S1's first
        // look (`push-record.md`).
        let (mut moved_eps, mut moved_ticks) = (0u32, 0u32);
        let counts = |play: u16, names: &[String]| {
            !matches!(names[usize::from(play)].as_str(), "Collect" | "Retreat" | "none")
        };
        for a in 0..n {
            for b in a + 1..n {
                let (ra, rb) = (&self.recs[a], &self.recs[b]);
                let len = ra.len().min(rb.len());
                let same = game.bodies[a].team == game.bodies[b].team;
                if same {
                    let mut ep: Option<(u32, u32)> = None;
                    let mut end = |ep: Option<(u32, u32)>| {
                        if let Some((s0, s1)) = ep
                            && s1 - s0 + 1 >= CLOSE_S
                        {
                            close_eps += 1;
                            close_s += s1 - s0 + 1;
                        }
                    };
                    for s in 0..(len / TPS) as u32 {
                        let k = s as usize * TPS + TPS - 1;
                        let near = ra[k].alive && rb[k].alive && ra[k].feet.distance_xz(rb[k].feet) <= CLOSE_M;
                        if !near {
                            continue;
                        }
                        ep = match ep {
                            Some((s0, s1)) if s - s1 <= CLOSE_GAP_S + 1 => Some((s0, s)),
                            other => {
                                end(other);
                                Some((s, s))
                            }
                        };
                    }
                    end(ep);
                }
                let mut last: Option<usize> = None;
                let mut last_still: Option<usize> = None;
                let mut last_moved: Option<usize> = None;
                for k in 0..len {
                    let (x, y) = (&ra[k], &rb[k]);
                    if x.alive
                        && y.alive
                        && x.feet.distance_xz(y.feet) <= ONE_BODY_M
                        && (x.feet.y - y.feet.y).abs() <= ONE_BODY_M
                    {
                        one_ticks += 1;
                        if last.is_none_or(|l| k > l + TPS / 2) {
                            one_eps += 1;
                        }
                        last = Some(k);
                        let later = (k + TPS).min(len - 1);
                        if x.feet.distance_xz(ra[later].feet) < ONE_BODY_STILL_M
                            && y.feet.distance_xz(rb[later].feet) < ONE_BODY_STILL_M
                        {
                            still_ticks += 1;
                            if last_still.is_none_or(|l| k > l + TPS / 2) {
                                still_eps += 1;
                            }
                            last_still = Some(k);
                            if counts(x.play, &self.plays) && counts(y.play, &self.plays) {
                                moved_ticks += 1;
                                if last_moved.is_none_or(|l| k > l + TPS / 2) {
                                    moved_eps += 1;
                                }
                                last_moved = Some(k);
                            }
                        }
                    }
                }
            }
        }

        // Stuck (bots only).
        let teams: Vec<u8> = game.bodies.iter().map(|b| b.team).collect();
        let mut stuck: BTreeMap<&str, (u32, f32)> = BTreeMap::new();
        let mut stuck_cause: BTreeMap<&str, (u32, f32)> = BTreeMap::new();
        let mut spots: BTreeMap<(i32, i32, i32), u32> = BTreeMap::new();
        let mut pre = PreCounts::default();
        let mut blips = 0u32;
        for place in (0..n).filter(|&p| self.is_bot[p]) {
            let (eps, b) = self.stuck_episodes(place, &teams, presses);
            blips += b;
            for e in eps {
                pre.add(self, e.cause, place, e.k0);
                for (map, name) in [(&mut stuck, e.kinds[0]), (&mut stuck_cause, e.cause)] {
                    let c = map.entry(name).or_default();
                    c.0 += 1;
                    c.1 += e.t1 - e.t0;
                }
                let cell = (
                    (e.at.x / SPOT_M[0]).floor() as i32,
                    (e.at.z / SPOT_M[0]).floor() as i32,
                    (e.at.y / SPOT_M[1]).floor() as i32,
                );
                *spots.entry(cell).or_default() += 1;
            }
        }
        let stuck_n: u32 = stuck.values().map(|e| e.0).sum();
        let stuck_s: f32 = stuck.values().map(|e| e.1).sum();
        let stuck_spots = spots.values().filter(|&&c| c >= 2).count();
        let round = |m: BTreeMap<&'static str, (u32, f32)>| -> BTreeMap<&'static str, (u32, f32)> {
            m.into_iter().map(|(k, (c, s))| (k, (c, s.round()))).collect()
        };

        let json = serde_json::json!({
            "stuck": stuck_n,
            "stuck_s": stuck_s.round(),
            "stuck_by": round(stuck),
            "stuck_by_cause": round(stuck_cause),
            "stuck_pre_5s": pre.by_json(),
            "stuck_play": pre.plays,
            "stuck_per_bot_min": per_min(stuck_n.into()),
            "stuck_s_per_bot_min": per_min(stuck_s.into()),
            "stuck_blips": blips,
            "stuck_blips_per_bot_min": per_min(blips.into()),
            "stuck_spots_2": stuck_spots,
            "calm_walk_s": (calm_walk as f64 * tick_s).round(),
            "calm_sprint_pct": pct(calm_sprint as f64, calm_walk as f64),
            "calm_mps": if calm_walk > 0 { (100.0 * calm_m / (calm_walk as f64 * tick_s)).round() / 100.0 } else { 0.0 },
            "idle_s": idle_s,
            "idle_by_play": idle,
            "drops_2_5m": drops[0] - drops[1],
            "drops_4m": drops[1],
            "uturns": uturns,
            "takes_moving": takes_moving,
            "uturns_per_bot_min": per_min(uturns.into()),
            "close_pairs": close_eps,
            "close_pair_s": close_s,
            "one_body": one_eps,
            "one_body_s": (f64::from(one_ticks) * tick_s).round(),
            "one_body_still": still_eps,
            "one_body_still_s": (f64::from(still_ticks) * tick_s).round(),
            "one_body_still_plays": moved_eps,
            "one_body_still_plays_s": (f64::from(moved_ticks) * tick_s).round(),
        });
        let text = format!(
            "owner-test moves: stuck {} ({:.0} s, {:.3} a bot-minute) by kind {} by cause {}, blips {}, spots of 2+ {}; calm sprint {:.1} % of {:.0} s walking, {:.2} m/s; idle {} s {}; drops 2.5-4 m {} >=4 m {}; U-turns {} of {} takes moving ({:.3} a bot-minute); close pairs {} ({} s); one body {} ({:.0} s); the 5 s before a stuck episode: {}",
            stuck_n,
            stuck_s,
            json["stuck_per_bot_min"].as_f64().unwrap_or(0.0),
            json["stuck_by"],
            json["stuck_by_cause"],
            blips,
            stuck_spots,
            json["calm_sprint_pct"].as_f64().unwrap_or(0.0),
            calm_walk as f64 * tick_s,
            json["calm_mps"].as_f64().unwrap_or(0.0),
            idle_s,
            json["idle_by_play"],
            drops[0] - drops[1],
            drops[1],
            uturns,
            takes_moving,
            json["uturns_per_bot_min"].as_f64().unwrap_or(0.0),
            close_eps,
            close_s,
            one_eps,
            f64::from(one_ticks) * tick_s,
            pre.text(),
        );
        Part::new(json, text)
    }
}

#[test]
pub(crate) fn calm_walking_counts_its_sprint_share_and_a_fight_does_not() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 40, |k, game| {
        let keys = if k < 20 { key::FORWARD | key::SPRINT } else { key::FORWARD };
        put(game, 0, 10.0, 10.0, keys);
        // Aiming: not calm.
        put(game, 1, 10.0, 20.0, key::FORWARD | key::AIM);
        game.bodies[0].step_m = 0.2;
        vec![bot_now(), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["calm_sprint_pct"], 50.0, "{json}");
    assert_eq!(json["calm_walk_s"], 2.0);
}

#[test]
pub(crate) fn five_calm_seconds_standing_are_idle_and_four_are_not() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 200, |k, game| {
        let s = k / 20;
        put(game, 0, 10.0, 10.0, if s < 6 { 0 } else { key::FORWARD });
        put(game, 1, 10.0, 20.0, if s < 4 { 0 } else { key::FORWARD });
        game.bodies[0].step_m = if s < 6 { 0.0 } else { 0.2 };
        game.bodies[1].step_m = if s < 4 { 0.0 } else { 0.2 };
        vec![bot_now(), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["idle_s"], 6, "{json}");
    assert_eq!(json["idle_by_play"]["Roam"], 6.0);
}

#[test]
pub(crate) fn a_drop_of_four_metres_in_a_second_is_counted() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 60, |k, game| {
        put(game, 0, 10.0, 10.0, 0);
        put(game, 1, 10.0, 20.0, 0);
        game.bodies[0].feet.y = if k < 20 { 10.0 } else { 4.5 };
        game.bodies[1].feet.y = if k < 20 { 10.0 } else { 8.0 };
        vec![bot_now(), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["drops_4m"], 1, "{json}");
    assert_eq!(json["drops_2_5m"], 0);
}

#[test]
pub(crate) fn a_take_that_turns_a_walking_bot_back_is_a_u_turn() {
    let (mut game, mut tally) = made_up(&[0, 0]);
    play(&mut game, &mut tally, 100, |k, game| {
        // Bot 0: east 0.1 m a tick, back west after tick 50; bot 1: east,
        // then north (90 degrees).
        let k = k as f32;
        let x0 = if k <= 50.0 { 5.0 + 0.1 * k } else { 10.0 - 0.1 * (k - 50.0) };
        put(game, 0, x0, 10.0, key::FORWARD);
        let (x1, z1) = if k <= 50.0 { (5.0 + 0.1 * k, 20.0) } else { (10.0, 20.0 + 0.1 * (k - 50.0)) };
        put(game, 1, x1, z1, key::FORWARD);
        vec![bot_now(), bot_now()]
    });
    let t = tally.first_tick + 50 * TICK_MS;
    tally.moving.takes[0].push(t);
    tally.moving.takes[1].push(t);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["takes_moving"], 2);
    assert_eq!(json["uturns"], 1, "{json}");
}

#[test]
pub(crate) fn teammates_close_five_seconds_and_bodies_in_one_another_are_counted() {
    let (mut game, mut tally) = made_up(&[0, 0, 0, 0]);
    play(&mut game, &mut tally, 160, |k, game| {
        let s = k / 20;
        put(game, 0, 10.0, 10.0, 0);
        // Within 1 m for 6 s, then 0.2 m (one body) for a second.
        let x1 = if s < 6 { 11.0 } else if s < 7 { 10.2 } else { 30.0 };
        put(game, 1, x1, 10.0, 0);
        // Within 1 m for 4 s, never closer than 0.6 m.
        put(game, 2, 10.0, 30.0, 0);
        let x3 = if s < 4 { 10.6 } else { 30.0 };
        put(game, 3, x3, 30.0, 0);
        vec![bot_now(); 4]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["close_pairs"], 1, "{json}");
    assert_eq!(json["close_pair_s"], 7);
    assert_eq!(json["one_body"], 1);
    assert_eq!(json["one_body_s"], 1.0);
}

#[test]
pub(crate) fn the_stuck_detector_finds_a_push_a_stand_and_a_watch_and_their_causes() {
    let (mut game, mut tally) = made_up(&[0, 0, 0, 0, 0]);
    play(&mut game, &mut tally, 400, |k, game| {
        let s = k / 20;
        // Bot 0 pushes forward without moving for 5 s, then walks off.
        if s < 5 {
            put(game, 0, 10.0, 10.0, key::FORWARD);
        } else {
            put(game, 0, 10.0 + 0.2 * (k - 100) as f32, 10.0, key::FORWARD);
            game.bodies[0].step_m = 0.2;
        }
        // Bot 1 (near miss) pushes 2 s.
        if s < 2 {
            put(game, 1, 10.0, 20.0, key::FORWARD);
        } else {
            put(game, 1, 10.0 + 0.2 * (k - 40) as f32, 20.0, key::FORWARD);
            game.bodies[1].step_m = 0.2;
        }
        // Bots 2 and 3 stand all along with no key, 2 in a step with a
        // goal, 3 (near miss) in a hold.
        put(game, 2, 10.0, 30.0, 0);
        put(game, 3, 20.0, 30.0, 0);
        // Bot 4 walks east, dances left and right on one spot for 6 s
        // (the stuck watch fires), walks on.
        let (x4, keys4) = match k {
            0..100 => (10.0 + 0.2 * k as f32, key::FORWARD),
            100..220 if k % 2 == 0 => (30.0, key::FORWARD | key::LEFT),
            100..220 => (30.3, key::FORWARD | key::RIGHT),
            _ => (30.0 + 0.2 * (k - 220) as f32, key::FORWARD),
        };
        put(game, 4, x4, 5.0, keys4);
        game.bodies[4].step_m = 0.1;
        let mut nows = vec![bot_now(); 5];
        nows[2].as_mut().unwrap().step = Some("Going".to_owned());
        nows[3].as_mut().unwrap().step = Some("Hold".to_owned());
        nows
    });
    let fired = tally.time_of(150);
    tally.moving.stuck_notes[4].push(fired);
    // Bot 1's watch fires in its 2 s push: a blip.
    let fired = tally.time_of(20);
    tally.moving.stuck_notes[1].push(fired);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["stuck_by"]["push"][0], 1, "{json}");
    assert_eq!(json["stuck_by"]["stand"][0], 1, "{json}");
    assert_eq!(json["stuck_by"]["watch"][0], 1, "{json}");
    assert_eq!(json["stuck"], 3);
    assert_eq!(json["stuck_by_cause"]["wall_no_line"][0], 1);
    assert_eq!(json["stuck_by_cause"]["stand"][0], 1);
    assert_eq!(json["stuck_by_cause"]["wall"][0], 1);
    assert_eq!(json["stuck_blips"], 1);
}

#[test]
pub(crate) fn a_stuck_bot_with_an_enemy_against_it_is_stuck_on_the_enemy() {
    // Bot 0 pushes for 5 s with enemy 2 at 1.0 m; bot 1 (near miss) pushes
    // as long with enemy 3 at 1.5 m.
    let (mut game, mut tally) = made_up(&[0, 0, 1, 1]);
    play(&mut game, &mut tally, 200, |k, game| {
        let s = k / 20;
        if s < 5 {
            put(game, 0, 10.0, 10.0, key::FORWARD);
            put(game, 1, 10.0, 30.0, key::FORWARD);
        } else {
            for (place, z) in [(0, 10.0), (1, 30.0)] {
                put(game, place, 10.0 + 0.2 * (k - 100) as f32, z, key::FORWARD);
                game.bodies[place].step_m = 0.2;
            }
        }
        put(game, 2, 11.0, 10.0, 0);
        put(game, 3, 11.5, 30.0, 0);
        vec![bot_now(); 4]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["stuck_by"]["push"][0], 2, "{json}");
    assert_eq!(json["stuck_by_cause"]["enemy"][0], 1, "{json}");
    assert_eq!(json["stuck_by_cause"]["wall_no_line"][0], 1, "{json}");
}

/// A 3 m plateau, walled on all four sides, in the soak field: no walk up or down.
fn plateau() -> Vec<mapdata::Triangle> {
    let walker = mapdata::WalkerBehaviour::BySlope;
    let quad = |a: Vec3, b: Vec3, c: Vec3, d: Vec3| {
        [
            mapdata::Triangle { v: [a, b, c], walker },
            mapdata::Triangle { v: [a, c, d], walker },
        ]
    };
    let v = Vec3::new;
    let mut tris = quad(v(10.0, 3.0, 10.0), v(20.0, 3.0, 10.0), v(20.0, 3.0, 20.0), v(10.0, 3.0, 20.0)).to_vec();
    for x in [10.0, 20.0] {
        tris.extend(quad(v(x, 0.0, 10.0), v(x, 0.0, 20.0), v(x, 3.0, 20.0), v(x, 3.0, 10.0)));
    }
    for z in [10.0, 20.0] {
        tris.extend(quad(v(10.0, 0.0, z), v(20.0, 0.0, z), v(20.0, 3.0, z), v(10.0, 3.0, z)));
    }
    tris
}

#[test]
pub(crate) fn a_drop_off_a_walled_plateau_and_a_climb_onto_it_are_counted_a_kilometre() {
    let world = super::super::soak_world_with(&plateau());
    let mut game = Game::new(world, &[0, 0], 1, None, None);
    let mut tally = OwnerTally::new(2);
    tally.is_bot = vec![true, true];
    play(&mut game, &mut tally, 2 * VERT_SAMPLE_TICKS + 1, |k, game| {
        let second = k / VERT_SAMPLE_TICKS;
        // Bot 0 on the plateau, then dropped to the field beside it, walking east.
        let (x0, y0) = if second == 0 { (15.0, 3.0) } else { (25.0 + 0.01 * k as f32, 0.0) };
        put(game, 0, x0, 15.0, 0);
        game.bodies[0].feet.y = y0;
        // Bot 1 on the field, then on the plateau (a climb with no walk: counted).
        let (x1, y1) = if second == 0 { (5.0, 0.0) } else { (15.0, 3.0) };
        put(game, 1, x1, 30.0, 0);
        game.bodies[1].feet.y = y1;
        vec![bot_now(), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["drops_certain"], 1, "{json}");
    assert_eq!(json["climbs_1_5m"], 1, "{json}");
    assert!(json["km_walked"].as_f64().unwrap() > 0.0, "{json}");
    assert!(json["drops_per_km"].as_f64().unwrap() > 0.0);
}

#[test]
pub(crate) fn a_walk_within_the_budget_is_a_walk_and_one_beyond_it_is_not() {
    let world = super::super::soak_world();
    let grid = &world.grid;
    let a = grid.locate(Vec3::new(10.0, 0.0, 10.0), 1.0).unwrap();
    let b = grid.locate(Vec3::new(14.0, 0.0, 10.0), 1.0).unwrap();
    assert!(walks_within(grid, a, b, 5.0));
    assert!(!walks_within(grid, a, b, 3.0));
}
