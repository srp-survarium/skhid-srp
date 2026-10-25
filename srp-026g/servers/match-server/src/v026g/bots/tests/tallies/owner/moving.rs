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
        } else if let Some((aim, time)) = thrown_at(&note.text)
            && let Some(body) = game.bodies.get(note.place)
        {
            self.grenades.grenades += 1;
            let at = grenade_lands(&game.world.sight, Game::eye(body), aim);
            let inside = game
                .bodies
                .iter()
                .map(|b| b.alive && (Game::chest(b) - at).length() <= BLAST_M)
                .collect();
            self.grenades.blasts.push(Blast {
                thrower: note.place,
                at,
                time,
                inside,
            });
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
