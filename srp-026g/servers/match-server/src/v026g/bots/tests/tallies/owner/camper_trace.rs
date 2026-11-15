//! [camper trace] The stand-in camper's match tick by tick, for a reader outside the bench (`SRP_BENCH_CAMPER_TRACE=<dir>`):
//! one JSON line a tick with his body, what his motion is doing (a stay, a leave, a return) and its spot, and each living enemy's
//! body with the lines of sight between the two and when each one's own senses last placed the other; then each hit and kill he
//! is in. It counts nothing and prints only the file's name; unset, it keeps nothing. Its rays are its own (not the tallies').

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
    now: Option<(char, Vec3, bool)>,
    hits_seen: usize,
    kills_seen: usize,
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
                ])
            })
            .collect();
        let (mode, spot, cover) = match self.now {
            Some((mode, spot, cover)) => (Some(mode.to_string()), Some([r2(spot.x), r2(spot.y), r2(spot.z)]), cover),
            None => (None, None, false),
        };
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
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        use std::io::Write;
        if let Some((mut file, path)) = self.out.take() {
            file.flush().unwrap();
            eprintln!("camper trace: {} ticks to {path}", self.ticks);
        }
        Vec::new()
    }
}
