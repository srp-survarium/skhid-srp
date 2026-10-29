//! The falls probe (`SRP_BENCH_FALLS`): lines for the reloads and falls of the bots with their keys of the last
//! seconds, and at the end the reckoning error of the bots' own feet, their distance off the route and the spots
//! the lip guard worked at. Prints only; no key of the `routes:` line.

use super::*;

pub(super) struct Probe {
    /// The probe is on (`SRP_BENCH_FALLS`).
    fall_lines: bool,
    /// Each body's keys and feet of the last two seconds (40 ticks).
    ring: Vec<std::collections::VecDeque<(u32, Vec3)>>,
    /// The reload each body is in (its start and end, ms) as the probe last saw it.
    reload_seen: Vec<Option<(u32, u32)>>,
    /// The reckoning error of the bots' own feet (m), and of those on a lip's floors.
    est_err: Vec<f32>,
    est_err_lip: Vec<f32>,
    /// The body's distance off its route (m) while walking it: facing the way, aiming; and on lip floors.
    off: [Vec<f32>; 4],
    /// Where the lip guard changed or could not change the keys: (x, y, z) in 2 m cells, [changed, none] ticks.
    guard_spots: std::collections::BTreeMap<(i32, i32, i32), [u32; 2]>,
}

impl Probe {
    pub(super) fn new(bodies: usize) -> Self {
        Self {
            fall_lines: super::super::bench_settings().flag("SRP_BENCH_FALLS"),
            ring: vec![std::collections::VecDeque::new(); bodies],
            reload_seen: vec![None; bodies],
            est_err: Vec::new(),
            est_err_lip: Vec::new(),
            off: Default::default(),
            guard_spots: Default::default(),
        }
    }

    /// The reload probe: a `RELOAD` line when a bot starts one and a `RELDEATH` line when it dies in one, each
    /// with the lip floor it stood on and its keys and guard verdicts of the last second (`history`).
    fn reload_line(&mut self, place: usize, body: &Body, before: Option<Vec3>, bots: &Bots, time: u32) {
        let Some(bot) = bots.bots[place].as_ref() else {
            return;
        };
        let note = bot.reflex.reload_note().map(|r| (r.at, r.until));
        let on_lip = |feet: Vec3| {
            bots.tactics.as_ref().map_or("-".to_owned(), |t| {
                let grid = &t.world.grid;
                grid.locate(feet, 1.0).map_or("nofloor".to_owned(), |n| format!("{}{}", if grid.is_lip(n) { "lip" } else { "" }, if grid.near_lip(n) { "+near" } else { "" }))
            })
        };
        if body.alive && note.is_some() && note != self.reload_seen[place] {
            self.reload_seen[place] = note;
            let (at, until) = note.unwrap_or_default();
            eprintln!("RELOAD t={time} place={place} at={at} until={until} lip=[{}] guard=[{}]", on_lip(body.feet), self.history(place, 20));
        }
        if !body.alive && let Some(feet) = before {
            if let Some((at, until)) = self.reload_seen[place]
                && at <= time
                && time <= until + 1_000
            {
                eprintln!("RELDEATH t={time} place={place} at={at} until={until} lip=[{}] guard=[{}]", on_lip(feet), self.history(place, 40));
            }
            self.reload_seen[place] = None;
        }
    }

    /// The last `ticks` of a body's keys, runs joined: `FL*6 F*14` (`s`/`w`/`x` a side step taken, woven, refused;
    /// `.n` the lip guard's verdict).
    fn history(&self, place: usize, ticks: usize) -> String {
        use crate::v026g::drive::key;
        let name = |keys: u32| {
            let mut s = String::new();
            match (keys >> 29) & 3 {
                1 => s.push('s'),
                2 => s.push('w'),
                3 => s.push('x'),
                _ => {}
            }
            for (bit, c) in [(key::FORWARD, 'F'), (key::BACK, 'B'), (key::LEFT, 'L'), (key::RIGHT, 'R'), (key::JUMP, 'J'), (key::CROUCH, 'C'), (key::SPRINT, 'S')] {
                if keys & bit != 0 {
                    s.push(c);
                }
            }
            if s.is_empty() {
                s.push('-');
            }
            let guard = (keys >> 26) & 7;
            if guard != 0 {
                s += &format!(".{guard}");
            }
            s
        };
        let mut history: Vec<(String, u32)> = Vec::new();
        for (keys, _) in self.ring[place].iter().skip(self.ring[place].len().saturating_sub(ticks)) {
            let n = name(*keys);
            match history.last_mut() {
                Some((last, count)) if *last == n => *count += 1,
                _ => history.push((n, 1)),
            }
        }
        history.iter().map(|(n, c)| format!("{n}*{c}")).collect::<Vec<_>>().join(" ")
    }

    /// The falls probe: one line for a fall of a metre or more (planned or not): what the bot was doing,
    /// its keys of the last second, where it stood against the grid's lips, how far off its route.
    fn fall_line(&self, place: usize, body: &Body, bots: &Bots, from: Vec3, time: u32, planned: bool) {
        use crate::v026g::drive::key;
        let Some(bot) = bots.bots[place].as_ref() else {
            return;
        };
        let (play, step) = bot.running.as_ref().map_or(("none".to_owned(), String::new()), |r| (format!("{:?}", r.kind()), r.step_name()));
        let go = match bot.intent.go {
            crate::v026g::bots::plays::Go::To { at, within_m } => format!("To d={:.1} within={:.1}", at.distance_xz(from), within_m),
            crate::v026g::bots::plays::Go::Stay => "Stay".to_owned(),
        };
        let history = self.history(place, 20);
        let side = body.keys & (key::LEFT | key::RIGHT) != 0;
        let along = body.keys & (key::FORWARD | key::BACK) != 0;
        let stepped = bot.bench_stepped;
        let strafe = match (side, along) {
            (false, _) => "none",
            (true, false) => "pure",
            (true, true) => "woven",
        };
        // The step the body made over the last ticks, m a tick, and the heading of the last two.
        let moved = {
            let ring = &self.ring[place];
            match (ring.front(), ring.back()) {
                (Some((_, a)), Some((_, b))) if ring.len() > 1 => a.distance_xz(*b) / (ring.len() - 1) as f32,
                _ => 0.0,
            }
        };
        let real_v = {
            let ring = &self.ring[place];
            // The velocity over the two ticks before the last (the last holds the fall).
            if ring.len() >= 3 {
                let (a, b) = (ring[ring.len() - 3].1, ring[ring.len() - 2].1);
                format!("{:.2},{:.2}", (b.x - a.x) / 0.05, (b.z - a.z) / 0.05)
            } else {
                String::new()
            }
        };
        // The last five feet of the body and of the bot's reckoning, (x, y, z) each: where the model and the body part.
        let trail = |points: &mut dyn Iterator<Item = Vec3>| points.map(|p| format!("{:.2},{:.2},{:.2}", p.x, p.y, p.z)).collect::<Vec<_>>().join(" ");
        let real_trail = trail(&mut self.ring[place].iter().rev().take(5).rev().map(|r| r.1));
        let est_trail = trail(&mut bot.bench_est.iter().rev().take(5).rev().map(|r| r.1));
        let (est_v, est_at) = bot.bench_guard_at;
        // The next step of the body's last velocity played over the grid from the floor under `from`, with the bots' slide
        // and the stand-in's capped one: whether each goes down a ledge (the guard plays the first, the body the second).
        let models = bots.tactics.as_ref().map_or(String::new(), |tactics| {
            let grid = &tactics.world.grid;
            let ring = &self.ring[place];
            if ring.len() < 3 {
                return String::new();
            }
            let (a, b) = (ring[ring.len() - 3].1, ring[ring.len() - 2].1);
            let Some(node) = grid.locate(from, 1.5) else {
                return String::from("nofloor");
            };
            let to = Vec3::new(from.x + (b.x - a.x), from.y, from.z + (b.z - a.z));
            let (n1, f1, _) = crate::v026g::bots::body::slide(grid, node, from, to);
            let (n2, f2, _) = crate::v026g::bots::body::slide_capped(grid, node, from, to, grid.body().step);
            format!("{node}@{:.2}:slide={n1}@{:.2} capped={n2}@{:.2}", grid.position(node).y, f1.y, f2.y)
        });
        let (lip, lip_m) = bots.tactics.as_ref().map_or((String::from("-"), -1.0), |tactics| {
            let grid = &tactics.world.grid;
            let node = grid.locate(from, 1.0);
            let flags = node.map_or(String::from("nofloor"), |n| {
                format!("{}{}{}", if grid.is_lip(n) { "lip," } else { "" }, if grid.near_lip(n) { "near_lip," } else { "" }, if grid.near_fall(n) { "near_fall," } else { "" })
            });
            let mut best = f32::INFINITY;
            for n in 0..grid.len() as NodeId {
                let p = grid.position(n);
                if (p.y - from.y).abs() < 1.5 && p.distance_xz(from) < best && grid.is_lip(n) {
                    best = p.distance_xz(from);
                }
            }
            (flags, if best.is_finite() { best } else { -1.0 })
        });
        let route = bot.route.as_ref().map_or(String::from("none"), |(route, goal)| {
            let mut r = route.clone();
            let (walked, off) = r.progress(from);
            let ahead = route
                .points()
                .windows(2)
                .scan(0.0f32, |s, w| {
                    *s += w[0].at.distance_xz(w[1].at);
                    Some((*s, w[0].at.y - w[1].at.y))
                })
                .find(|(s, drop)| *s >= walked && *drop >= 1.0)
                .map_or(-1.0, |(s, _)| s - walked);
            format!("walked={:.1}/{:.1} off={:.2} drop_ahead={:.1} goal_d={:.1}", walked, route.length(), off, ahead, goal.distance_xz(from))
        });
        eprintln!(
            "{} t={} place={} drop={:.1} play={} step={} go={} legs={} why={:#x} unstick={} carrying={} strafe={} stepped={} est_v={:.2},{:.2} real_v={} est_dfeet={:.2} keys=[{}] speed={:.2} lip={} lip_m={:.2} route={} from={:.1},{:.1},{:.1} guard={} watch={:?} models=[{}] real_trail=[{}] est_trail=[{}]",
            if planned { "PLANNED" } else { "FALL" },
            time,
            place,
            from.y - body.feet.y,
            play,
            step,
            go,
            bot.bench_legs_why.map_or("-".to_owned(), |w| format!("{w:?}")),
            bot.bench_why,
            u8::from(bot.unstick.is_some()),
            u8::from(body.carrying.is_some()),
            strafe,
            stepped,
            est_v[0],
            est_v[1],
            real_v,
            est_at.distance_xz(from),
            history,
            moved,
            lip,
            lip_m,
            route,
            from.x,
            from.y,
            from.z,
            bot.bench_guard,
            bot.lip_watch,
            models,
            real_trail,
            est_trail
        );
    }
}

impl RouteTopic for Probe {
    fn tick(&mut self, _run: &RouteRun, at: &TickAt<'_>) {
        if !self.fall_lines {
            return;
        }
        let (place, body, bots, time, before) = (at.place, at.body, at.bots, at.time, at.before);
            if self.fall_lines {
                self.reload_line(place, body, before, bots, time);
            }
            if self.fall_lines {
                let ring = &mut self.ring[place];
                if !body.alive {
                    ring.clear();
                } else {
                    if ring.len() == 40 {
                        ring.pop_front();
                    }
                    if let Some(bot) = bots.bots[place].as_ref()
                        && let Some(&(_, est)) = bot.bench_est.iter().find(|(at, _)| *at == time)
                    {
                        let err = est.distance_xz(body.feet);
                        self.est_err.push(err);
                        if let Some(tactics) = bots.tactics.as_ref() {
                            let grid = &tactics.world.grid;
                            if grid.locate(body.feet, 1.0).is_some_and(|n| grid.near_lip(n) || grid.is_lip(n)) {
                                self.est_err_lip.push(err);
                            }
                        }
                    }
                    if let Some(bot) = bots.bots[place].as_ref()
                        && bot.bench_walk != 0
                        && let Some((route, _)) = bot.route.as_ref()
                    {
                        let mut r = route.clone();
                        let (_, off) = r.progress(body.feet);
                        if off.is_finite() {
                            let on_lip = bots.tactics.as_ref().is_some_and(|t| t.world.grid.locate(body.feet, 1.0).is_some_and(|n| t.world.grid.near_lip(n) || t.world.grid.is_lip(n)));
                            self.off[usize::from(bot.bench_walk) - 1 + if on_lip { 2 } else { 0 }].push(off);
                        }
                    }
                    if let Some(bot) = bots.bots[place].as_ref()
                        && matches!(bot.bench_guard, 2 | 3)
                    {
                        let cell = ((body.feet.x / 2.0).floor() as i32, (body.feet.y / 2.0).floor() as i32, (body.feet.z / 2.0).floor() as i32);
                        self.guard_spots.entry(cell).or_default()[usize::from(bot.bench_guard == 3)] += 1;
                    }
                    let (step, guard) = bots.bots[place].as_ref().map_or((0, 0), |bot| (u32::from(bot.bench_stepped), u32::from(bot.bench_guard)));
                    ring.push_back((body.keys | step << 29 | guard << 26, body.feet));
                }
            }
        if let Some(fall) = at.fall {
            self.fall_line(place, body, bots, fall.from, time, fall.planned);
        }
    }

    fn line(&mut self, _run: &RouteRun, _line: &mut Line) {
        if self.fall_lines {
            let pct = |v: &mut Vec<f32>, p: f32| {
                v.sort_by(f32::total_cmp);
                v.get(((v.len() as f32 - 1.0) * p) as usize).copied().unwrap_or(0.0)
            };
            let (mut all, mut lip) = (std::mem::take(&mut self.est_err), std::mem::take(&mut self.est_err_lip));
            for (name, v) in ["walk", "aiming", "walk_lip", "aiming_lip"].into_iter().zip(self.off.iter_mut()) {
                eprintln!("OFFROUTE {name} n={} p50={:.2} p90={:.2} p99={:.2}", v.len(), pct(v, 0.5), pct(v, 0.9), pct(v, 0.99));
            }
            let mut spots: Vec<_> = self.guard_spots.iter().collect();
            spots.sort_by_key(|(_, n)| std::cmp::Reverse(n[0] + n[1]));
            for ((x, y, z), n) in spots.into_iter().take(8) {
                eprintln!("GUARDSPOT x={} y={} z={} changed={} none={}", x * 2, y * 2, z * 2, n[0], n[1]);
            }
            eprintln!(
                "ESTERR n={} p50={:.2} p90={:.2} p99={:.2} lip_n={} lip_p50={:.2} lip_p90={:.2} lip_p99={:.2}",
                all.len(), pct(&mut all, 0.5), pct(&mut all, 0.9), pct(&mut all, 0.99), lip.len(), pct(&mut lip, 0.5), pct(&mut lip, 0.9), pct(&mut lip, 0.99)
            );
        }
    }
}
