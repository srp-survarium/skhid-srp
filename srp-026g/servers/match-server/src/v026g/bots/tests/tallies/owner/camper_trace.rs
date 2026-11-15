//! [camper trace] The stand-in camper's match tick by tick, for a reader outside the bench (`SRP_BENCH_CAMPER_TRACE=<dir>`):
//! one JSON line a tick with his body, what his motion is doing (a stay, a leave, a return) and its spot, and each living enemy's
//! body with the lines of sight between the two and when each one's own senses last placed the other; then each hit and kill he
//! is in, and every kill of the match with whether its lethal shot landed on the head (`Game::head_kill_log`). It counts nothing
//! and prints only the file's name; unset, it keeps nothing. Its rays are its own (not the tallies').

use super::*;

/// The lines of sight are cast to enemies within this, m.
const REACH_M: f32 = 120.0;

#[derive(Default)]
pub(super) struct CamperTrace {
    /// The file, opened at the first tick with a camper when the setting is on.
    out: Option<(std::io::BufWriter<std::fs::File>, String)>,
    /// Tried to open it.
    tried: bool,
    /// `[place][who]`: the last tick the place's own senses placed `who` (`Track::own`), bots only.
    own: Vec<Vec<Option<u32>>>,
    /// What his motion is doing now (`BenchCamper::trace_now`).
    now: Option<crate::v026g::bots::tests::TraceNow>,
    hits_seen: usize,
    kills_seen: usize,
    all_kills_seen: usize,
    shots_seen: usize,
    parts_seen: usize,
    /// Each team's skill at the last tick (`signals::Skill`).
    team_skill: [f32; 2],
    ticks: u64,
}

impl CamperTrace {
    fn open(&mut self) {
        self.tried = true;
        let settings = crate::v026g::bots::tests::bench_settings();
        let Some(dir) = settings.var_os("SRP_BENCH_CAMPER_TRACE") else {
            return;
        };
        let maps = settings.var("SRP_BENCH_MAPS").unwrap_or_default().replace('/', "_");
        let seed = settings.var("SRP_BENCH_SEED").unwrap_or_default();
        let path = std::path::Path::new(&dir).join(format!("camper-trace-m{maps}-s{seed}.jsonl"));
        let file = std::fs::File::create(&path).unwrap_or_else(|e| panic!("camper trace {}: {e}", path.display()));
        self.out = Some((std::io::BufWriter::new(file), path.display().to_string()));
    }

    fn write(&mut self, value: serde_json::Value) {
        use std::io::Write;
        if let Some((file, _)) = self.out.as_mut() {
            writeln!(file, "{value}").unwrap();
        }
    }
}

fn r2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

fn r3(v: f32) -> f32 {
    (v * 1000.0).round() / 1000.0
}

impl Topic for CamperTrace {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if self.out.is_none() {
            return;
        }
        if self.own.len() <= place {
            self.own.resize(place + 1, Vec::new());
        }
        let own = &mut self.own[place];
        own.clear();
        if let Some(bot) = bot {
            for t in bot.memory.tracks() {
                if let Some((_, when)) = t.own {
                    if own.len() <= t.who {
                        own.resize(t.who + 1, None);
                    }
                    own[t.who] = Some(when);
                }
            }
        }
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        self.now = bots.bench_camper.as_ref().and_then(|c| c.trace_now());
        for (team, skill) in self.team_skill.iter_mut().enumerate() {
            if let Some(state) = bots.teams.get(team) {
                *skill = state.skill.skill;
            }
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, sight: &mapdata::RayTree, _rays: &mut u64) {
        let Some(camper) = run.camper else {
            return;
        };
        if !self.tried {
            self.open();
        }
        if self.out.is_none() {
            return;
        }
        let tick = game.tick;
        let own = |of: Place, who: Place| -> Option<u32> {
            self.own.get(of).and_then(|o| o.get(who)).copied().flatten().map(|when| tick.saturating_sub(when))
        };
        let me = &game.bodies[camper];
        let mut mine = 0u64;
        let enemies: Vec<serde_json::Value> = game
            .bodies
            .iter()
            .enumerate()
            .filter(|&(p, b)| p != camper && b.team != me.team && b.alive)
            .map(|(p, b)| {
                let near = me.alive && (b.feet - me.feet).length() <= REACH_M;
                let he_sees = near && in_sight(sight, b, me, REACH_M, &mut mine);
                let i_see = near && in_sight(sight, me, b, REACH_M, &mut mine);
                serde_json::json!([
                    p,
                    r2(b.feet.x),
                    r2(b.feet.y),
                    r2(b.feet.z),
                    r3(b.yaw),
                    r3(b.pitch),
                    u8::from(b.keys & key::FIRE != 0),
                    b.target,
                    r2(b.health),
                    u8::from(he_sees),
                    u8::from(i_see),
                    own(p, camper),
                    own(camper, p),
                    b.keys,
                ])
            })
            .collect();
        let (mode, spot, cover) = match self.now {
            Some(n) => (Some(n.mode.to_string()), Some([r2(n.spot.x), r2(n.spot.y), r2(n.spot.z)]), n.cover),
            None => (None, None, false),
        };
        let (hold, enemy_near, leaves_by_kill) = self.now.map_or((None, None, None), |n| (Some(n.hold.to_string()), Some(n.enemy_near), Some(n.leaves_by_kill)));
        let goal_m = self.now.and_then(|n| n.goal).map(|g| r2(g.distance_xz(me.feet)));
        let line = serde_json::json!({
            "t": tick,
            "c": [
                u8::from(me.alive),
                r2(me.feet.x),
                r2(me.feet.y),
                r2(me.feet.z),
                r3(me.yaw),
                r3(me.pitch),
                u8::from(me.keys & key::FIRE != 0),
                me.target,
                r2(me.health),
                mode,
                spot,
                u8::from(cover),
                me.keys,
                hold,
                enemy_near,
                leaves_by_kill,
                goal_m,
            ],
            "e": enemies,
        });
        self.write(line);
        self.ticks += 1;
        let hits: Vec<(u32, Place, Place)> =
            run.hits[self.hits_seen.min(run.hits.len())..].iter().copied().filter(|&(_, s, v)| s == camper || v == camper).collect();
        self.hits_seen = run.hits.len();
        for (t, s, v) in hits {
            self.write(serde_json::json!({"t": t, "hit": [s, v]}));
        }
        let kills: Vec<(u32, Place, Place)> =
            run.kills[self.kills_seen.min(run.kills.len())..].iter().copied().filter(|&(_, s, v)| s == camper || v == camper).collect();
        self.kills_seen = run.kills.len();
        for (t, s, v) in kills {
            self.write(serde_json::json!({"t": t, "kill": [s, v]}));
        }
        // Every round he fired or that was fired at him: shooter, target, flat distance, the gun's class, the shooter team's skill.
        let shots: Vec<crate::v026g::bots::tests::standin::ShotRecord> = game.shot_log[self.shots_seen.min(game.shot_log.len())..]
            .iter()
            .copied()
            .filter(|&(_, s, aimed, _)| s == camper || aimed.is_some_and(|(t, _)| t == camper))
            .collect();
        self.shots_seen = game.shot_log.len();
        for (t, s, aimed, class) in shots {
            let skill = game.bodies.get(s).map(|b| self.team_skill[usize::from(b.team).min(1)]);
            let (target, dist) = aimed.map_or((None, None), |(p, d)| (Some(p), Some(r2(d))));
            self.write(serde_json::json!({"t": t, "shot": [s, target, dist, class, skill.map(r3)]}));
        }
        // The box shapes' hits he took or made, with whether a bullet landed on the head.
        let parts: Vec<(u32, Place, Place, bool)> = game.hit_part_log[self.parts_seen.min(game.hit_part_log.len())..]
            .iter()
            .copied()
            .filter(|&(_, s, v, _)| s == camper || v == camper)
            .collect();
        self.parts_seen = game.hit_part_log.len();
        for (t, s, v, head) in parts {
            self.write(serde_json::json!({"t": t, "hit_part": [s, v, head]}));
        }
        let every: Vec<(u32, Place, Place)> = run.kills[self.all_kills_seen.min(run.kills.len())..].to_vec();
        self.all_kills_seen = run.kills.len();
        for (t, s, v) in every {
            let head = game.head_kill_log.iter().any(|&(ht, hs, hv)| ht == t && hs == s && hv == v);
            self.write(serde_json::json!({"t": t, "k": [s, v], "head": head}));
        }
    }

    fn parts(&mut self, run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        use std::io::Write;
        if self.out.is_some() {
            // Each shooter's rounds and rounds on an enemy by gun (`Game::shots_hits_by_gun`), with his team and whether he is the
            // camper: the live statistics' counters.
            let rows: Vec<serde_json::Value> = game
                .shots_hits_by_gun
                .iter()
                .map(|(&(place, item), &(shots, hits))| {
                    let team = game.bodies.get(place).map(|b| b.team);
                    serde_json::json!([place, team, run.camper == Some(place), item, shots, hits])
                })
                .collect();
            self.write(serde_json::json!({"shots_by_gun": rows}));
            // Every bot's rounds at an enemy target and the hits on that target (a hit of the shooter on it in the round's tick), by
            // the gun's class and the flat range band 0-10, 10-20, 20-40, 40+ m: the bots' side against every target, not only him.
            let mut hit_ticks: std::collections::BTreeMap<(u32, Place, Place), u32> = std::collections::BTreeMap::new();
            for &(t, s, v) in &run.hits {
                *hit_ticks.entry((t, s, v)).or_default() += 1;
            }
            let mut bands: std::collections::BTreeMap<(&'static str, usize), (u32, u32)> = std::collections::BTreeMap::new();
            for &(t, s, aimed, class) in &game.shot_log {
                let Some((target, dist)) = aimed else { continue };
                if run.camper == Some(s) || game.bodies.get(s).map(|b| b.team) == game.bodies.get(target).map(|b| b.team) {
                    continue;
                }
                let band = [10.0, 20.0, 40.0].iter().position(|&edge| dist < edge).unwrap_or(3);
                let cell = bands.entry((class, band)).or_default();
                cell.0 += 1;
                if let Some(left) = hit_ticks.get_mut(&(t, s, target)).filter(|n| **n > 0) {
                    *left -= 1;
                    cell.1 += 1;
                }
            }
            let rows: Vec<serde_json::Value> =
                bands.iter().map(|(&(class, band), &(rounds, hits))| serde_json::json!([class, band, rounds, hits])).collect();
            self.write(serde_json::json!({"bots_rounds_by_band": rows}));
        }
        if let Some((mut file, path)) = self.out.take() {
            file.flush().unwrap();
            eprintln!("camper trace: {} ticks to {path}", self.ticks);
        }
        Vec::new()
    }
}
