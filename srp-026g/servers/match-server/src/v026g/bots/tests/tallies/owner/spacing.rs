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
    /// The second each body came alive, for a respawn (its life's age); none for its life from the match's start (the
    /// opening is not a respawn, and is left out of the life-age buckets).
    born: Vec<Option<usize>>,
    /// By the play the body runs (a bot's assignment; "human" for the stand-in): the living bodies sampled and the
    /// teammates near them, and how many had 3 or more.
    by_play: BTreeMap<String, [u64; 3]>,
}

impl SpacingCounts {
    pub(super) fn new(n: usize) -> Self {
        Self { by_team: Default::default(), window: vec![[0; WINDOW_S]; n], was_alive: vec![false; n], born: vec![None; n], by_play: BTreeMap::new() }
    }
}

impl Topic for SpacingCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
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
                self.window[place] = [0; WINDOW_S];
            }
            if !self.was_alive[place] && body.alive {
                self.born[place] = (tick > 0).then_some(second);
            }
            self.was_alive[place] = body.alive;
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
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("spacing", self.part())]
    }
}

impl SpacingCounts {
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
        let json = serde_json::json!({ "spacing": { "by_team": { "0": team(0), "1": team(1) }, "by_play": by_play } });
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
