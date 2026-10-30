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

/// Whether two bodies move the same way (their yaws within 45 degrees; the walk keys are forward).
fn heading_alike(a: Option<&Rec>, b: Option<&Rec>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => crate::v026g::bots::kin::wrap(a.yaw - b.yaw).abs() < std::f32::consts::FRAC_PI_4,
        _ => false,
    }
}

/// Crowded: this many distinct enemies near in the window.
const CROWD: u32 = 3;

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
    /// The 1 m pair-seconds by what the step apart did on the pair's two bodies that tick (the worse of the two): nothing asked (no mate inside in the bot's picture), taken, refused (no clear
    /// ground that side), held off (the reflex strafes or fires, a use is held, a side key down).
    apart_state: [u64; 4],
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

/// What the topic keeps: the counts by team, and for each body the enemies near it (a bit by place) in each of the last
/// [`WINDOW_S`] seconds.
pub(super) struct SpacingCounts {
    by_team: [TeamSpacing; 2],
    window: Vec<[u64; WINDOW_S]>,
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
}

impl SpacingCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            by_team: Default::default(),
            window: vec![[0; WINDOW_S]; n],
            was_alive: vec![false; n],
            hits: vec![Vec::new(); n],
            last_pos: vec![None; n],
            still_s: vec![0; n],
            last_killer: vec![None; n],
            goal: vec![None; n],
            apart: vec![0; n],
            target: vec![crate::v026g::bots::plays::TargetRef::None; n],
            overlap_plays: Default::default(),
            born: vec![None; n],
            by_play: BTreeMap::new(),
        }
    }
}

impl Topic for SpacingCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if let Some(goal) = self.goal.get_mut(place) {
            *goal = bot.and_then(|b| b.route.as_ref().map(|(route, _)| route.end()));
        }
        if let Some(apart) = self.apart.get_mut(place) {
            *apart = bot.map_or(0, |b| b.bench_apart);
        }
        if let Some(target) = self.target.get_mut(place) {
            *target = bot.and_then(|b| b.assignment.as_ref().map(|a| a.target)).unwrap_or(crate::v026g::bots::plays::TargetRef::None);
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
                self.window[place] = [0; WINDOW_S];
                self.hits[place].clear();
            }
            if !self.was_alive[place] && body.alive {
                self.born[place] = (tick > 0).then_some(second);
            }
            self.was_alive[place] = body.alive;
            if sample && body.alive {
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
            for (other_place, other) in game.bodies.iter().enumerate() {
                if other_place != place && other.alive && other.team == body.team {
                    alive_mates += 1;
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
        }
        if sample {
            self.overlap(run, game);
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("spacing", self.part())]
    }
}

impl SpacingCounts {
    /// The overlap of the living teammates this second: see [`TeamSpacing::pairs`].
    fn overlap(&mut self, run: &Run, game: &Game) {
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
                if d >= OVERLAP_M || (a.feet.y - b.feet.y).abs() >= OVERLAP_DY_M {
                    continue;
                }
                let ts = &mut self.by_team[team];
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
                        (2, _) | (_, 2) => 2,
                        (3, _) | (_, 3) => 3,
                        (1, _) | (_, 1) => 1,
                        _ => 0,
                    }
                };
                ts.apart_state[worst(self.apart[i], self.apart[j])] += 1;
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
            serde_json::json!({
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
                "overlap_1m_apart_none_taken_refused_heldoff": c.apart_state,
                "overlap_1m_moving_pair_goal_under5_5_20_20up_none": c.in_file_goals,
                "killed_by_bullet_or_other": c.killed_by,
                "killed_by_shooters_1_2_3": c.shooters,
                "killed_by_killer_still_no_yes": c.killer_still,
                "killed_by_killer_m_10_30_up": c.killer_m,
                "by_life": c.by_life.iter().map(|[n, mates, alive]| serde_json::json!({ "samples": n, "mates_near": mates, "mates_alive": alive })).collect::<Vec<_>>(),
            })
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
            format!(
                "mates within 30 m {:.2}, enemies {:.2}, 3+ mates {:.0} % of {} body-seconds; deaths {} crowded {} ({:.0} %, mean {:.1} enemies near); by life age: {}",
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
        Part::new(json, format!("owner-test spacing: team 0 [{}]; team 1 [{}]", line(0), line(1)))
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
