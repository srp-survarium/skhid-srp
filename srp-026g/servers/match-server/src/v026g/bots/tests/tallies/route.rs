use super::*;

/// The bench's count of how the bots got about (`player_bots_stuck.md` §8),
/// as the audit's `wallpush.py` and `movement.py` count it from a match's
/// worlds: sampled once a world (1 s), and the carriers each tick.
#[derive(Default)]
pub(crate) struct RouteTally {
    /// Each body at the last world: feet, alive; and its runs so far
    /// (worlds holding a walk key without moving, worlds still, worlds of
    /// them in a fight); its last move of 1.5 m or more.
    pub(crate) last: Vec<Option<(Vec3, bool)>>,
    pub(crate) push_run: Vec<u32>,
    pub(crate) idle_run: Vec<(u32, u32)>,
    pub(crate) last_move: Vec<Option<(f32, f32)>>,
    /// Runs of 5 worlds or more holding a walk key moving under 0.3 m a
    /// world, and their worlds.
    pub(crate) pushes: (u32, u32),
    /// Stretches of 5 s or more moving under 0.5 m a world, and their
    /// seconds: with no target, and in a fight.
    pub(crate) idle: (u32, u32),
    pub(crate) idle_fighting: u32,
    /// Moves of 1.5 m or more turning back over 135° from the one before,
    /// with no target.
    pub(crate) reversals: u32,
    /// Living bot-worlds.
    pub(crate) worlds: u32,
    /// Each body's pickup time, and the carries delivered (ms each) and
    /// lost.
    pub(crate) carry_from: Vec<Option<(u32, usize)>>,
    pub(crate) carried: Vec<u32>,
    pub(crate) lost: u32,
    /// "is stuck" notes, and those of them by the walk keys alone.
    pub(crate) stuck: (u32, u32),
    pub(crate) no_route: [u64; 5],
    /// Each body's feet at the last tick, and its falls of a metre or more
    /// in a tick: off its route (no drop of it there), and taken by it.
    pub(crate) feet: Vec<Option<Vec3>>,
    pub(crate) falls: (u32, u32),
    /// Each body's team, where its bots got stuck or fell off their route
    /// (team, place, where), and the stucks and falls off at a spot a
    /// teammate had already (`stuck::TeamSpots`).
    pub(crate) teams: Vec<Option<u8>>,
    pub(crate) marks: Vec<(u8, usize, Vec3)>,
    pub(crate) repeats: (u32, u32),
    /// Each body's zone set at the last world (0: none) and the set it
    /// last left, when; living bot-worlds on a zone's floors while it is
    /// on; entries onto zones' floors, and of them back onto the set left
    /// within `stuck::ZONE_AGAIN_MS`; the escapes' count at the start
    /// (`tactics::escapes`).
    pub(crate) zone_in: Vec<u16>,
    pub(crate) zone_left: Vec<Option<(u16, u32)>>,
    pub(crate) zone_on: u32,
    pub(crate) zone_entries: (u32, u32),
    pub(crate) escapes: [u64; 5],
    /// Each body on a pocket's floor at the last world; living bot-worlds
    /// on pockets' floors, deaths there; the pocket escapes' count at the
    /// start (`escape::counts`).
    pub(crate) pocket_in: Vec<bool>,
    pub(crate) pocket_worlds: u32,
    pub(crate) pocket_deaths: u32,
    pub(crate) pocket_escapes: [u64; 7],
    pub(crate) motor_counts: [u64; 8],
    pub(crate) route_ends: [(u64, u64); super::route_end::RouteEnd::ALL.len()],
    /// The sights' count at the start (`foliage::through`).
    pub(crate) foliage: [u64; 3],
    /// [CG-17] Each body's walk at the last tick to an exact spot
    /// (`Motor::exact_end`), and when it last landed a planned drop on one;
    /// planned drops on an exact walk (this tick's or the last), and of
    /// them those whose bot picked a battery up within 10 s.
    pub(crate) exact_was: Vec<bool>,
    pub(crate) exact_drop: Vec<Option<u32>>,
    pub(crate) falls_exact: (u32, u32),
    /// The falls probe (`SRP_BENCH_FALLS`): each body's keys and feet of the last two seconds (40 ticks).
    pub(crate) ring: Vec<std::collections::VecDeque<(u32, Vec3)>>,
    pub(crate) fall_lines: bool,
    /// The reload each body is in (its start and end, ms) as the probe last saw it.
    pub(crate) reload_seen: Vec<Option<(u32, u32)>>,
    /// The reckoning error of the bots' own feet (m), and of those on a lip's floors.
    pub(crate) est_err: Vec<f32>,
    pub(crate) est_err_lip: Vec<f32>,
    /// The body's distance off its route (m) while walking it: facing the way, aiming; and on lip floors.
    pub(crate) off: [Vec<f32>; 4],
    /// Where the lip guard changed or could not change the keys: (x, y, z) in 2 m cells, [changed, none] ticks.
    pub(crate) guard_spots: std::collections::BTreeMap<(i32, i32, i32), [u32; 2]>,
}

impl RouteTally {
    pub(crate) fn start(bodies: usize) -> Self {
        RouteTally {
            last: vec![None; bodies],
            push_run: vec![0; bodies],
            idle_run: vec![(0, 0); bodies],
            last_move: vec![None; bodies],
            carry_from: vec![None; bodies],
            feet: vec![None; bodies],
            teams: vec![None; bodies],
            no_route: super::path::no_routes(),
            zone_in: vec![0; bodies],
            zone_left: vec![None; bodies],
            escapes: super::tactics::escapes::counts(),
            pocket_in: vec![false; bodies],
            pocket_escapes: super::escape::counts::counts(),
            motor_counts: super::motor::counts::counts(),
            route_ends: super::route_end::counts::counts(),
            foliage: super::foliage::through::counts(),
            exact_was: vec![false; bodies],
            exact_drop: vec![None; bodies],
            ring: vec![std::collections::VecDeque::new(); bodies],
            fall_lines: super::bench_settings().flag("SRP_BENCH_FALLS"),
            reload_seen: vec![None; bodies],
            est_err: Vec::new(),
            est_err_lip: Vec::new(),
            off: Default::default(),
            guard_spots: Default::default(),
            ..Default::default()
        }
    }

    /// A tick: the carries begun and ended, and the falls.
    pub(crate) fn tick(&mut self, game: &Game, bots: &Bots, time: u32) {
        for (place, body) in game.bodies.iter().enumerate() {
            if let Some(bot) = bots.bots[place].as_ref() {
                self.teams[place] = Some(bot.team);
            }
            let before = std::mem::replace(&mut self.feet[place], body.alive.then_some(body.feet));
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
            if let Some(from) = before
                && body.alive
                && from.y - body.feet.y >= 1.0
            {
                // Its route drops there: a corner over a metre under the
                // one before, within a metre of where it landed.
                let planned = bots.bots[place]
                    .as_ref()
                    .and_then(|bot| bot.route.as_ref())
                    .is_some_and(|(route, _)| {
                        route.points().windows(2).any(|w| {
                            w[0].at.y - w[1].at.y >= 1.0 && w[1].at.distance_xz(body.feet) <= 1.0
                        })
                    });
                if self.fall_lines {
                    self.fall_line(place, body, bots, from, time, planned);
                }
                if planned {
                    self.falls.1 += 1;
                    let exact = self.exact_was[place]
                        || bots.bots[place].as_ref().is_some_and(|bot| bot.motor.exact_end);
                    if exact {
                        self.falls_exact.0 += 1;
                        self.exact_drop[place] = Some(time);
                    }
                } else {
                    self.falls.0 += 1;
                    self.repeats.1 += u32::from(self.mark(place, from));
                }
            }
            self.exact_was[place] = bots.bots[place].as_ref().is_some_and(|bot| bot.motor.exact_end);
            match (self.carry_from[place], body.carrying) {
                (None, Some(battery)) => {
                    if let Some(at) = self.exact_drop[place].take()
                        && time <= at + 10_000
                    {
                        self.falls_exact.1 += 1;
                    }
                    self.carry_from[place] = Some((time, battery));
                }
                (Some((from, battery)), None) => {
                    self.carry_from[place] = None;
                    if matches!(game.batteries.get(battery), Some(Lying::Home(_))) {
                        self.carried.push(time - from);
                    } else {
                        self.lost += 1;
                    }
                }
                _ => {}
            }
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
            if ring.len() >= 2 {
                let (a, b) = (ring[ring.len() - 2].1, ring[ring.len() - 1].1);
                format!("{:.2},{:.2}", (b.x - a.x) / 0.05, (b.z - a.z) / 0.05)
            } else {
                String::new()
            }
        };
        let (est_v, est_at) = bot.bench_guard_at;
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
            "{} t={} place={} drop={:.1} play={} step={} go={} legs={} why={:#x} unstick={} carrying={} strafe={} stepped={} est_v={:.2},{:.2} real_v={} est_dfeet={:.2} keys=[{}] speed={:.2} lip={} lip_m={:.2} route={} from={:.1},{:.1},{:.1}",
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
            from.z
        );
    }

    /// A world: the runs of pushing and standing, the reversals, and the
    /// bots on zones' floors.
    pub(crate) fn world(&mut self, game: &Game, bots: &Bots) {
        const WALK: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT;
        for (place, body) in game.bodies.iter().enumerate() {
            if bots.bots[place].is_some() {
                self.zones(place, body, bots, game.tick);
            }
            let now = (body.feet, body.alive);
            let before = self.last[place].replace(now);
            let Some((from, true)) = before.filter(|_| body.alive) else {
                self.end_runs(place);
                self.last_move[place] = None;
                continue;
            };
            self.worlds += 1;
            let moved = from.distance_xz(body.feet);
            let fight = body.target.is_some();
            if body.keys & WALK != 0 && moved < 0.3 {
                self.push_run[place] += 1;
            } else {
                self.end_push(place);
            }
            if moved < 0.5 {
                let run = &mut self.idle_run[place];
                run.0 += 1;
                run.1 += u32::from(fight);
            } else {
                self.end_idle(place);
            }
            let step = (body.feet.x - from.x, body.feet.z - from.z);
            if moved >= 1.5 && !fight {
                if let Some(back) = self.last_move[place] {
                    let cos = (step.0 * back.0 + step.1 * back.1) / (moved * back.0.hypot(back.1));
                    self.reversals += u32::from(cos < (135f32).to_radians().cos());
                }
                self.last_move[place] = Some(step);
            } else if moved >= 1.5 {
                self.last_move[place] = None;
            }
        }
    }

    /// `place`'s bot's zone at `time`: on its floors while on, entered
    /// (again).
    pub(crate) fn zones(&mut self, place: usize, body: &Body, bots: &Bots, time: u32) {
        let Some(tactics) = bots.tactics.as_ref() else {
            return;
        };
        let hazards = &tactics.hazards;
        let node = body
            .alive
            .then(|| tactics.world.grid.locate(body.feet, 1.0))
            .flatten();
        let pocket = node.is_some_and(|node| tactics.world.grid.in_pocket(node));
        self.pocket_worlds += u32::from(pocket);
        let was_in = std::mem::replace(&mut self.pocket_in[place], pocket);
        self.pocket_deaths += u32::from(was_in && !body.alive);
        let set = node.map_or(0, |node| hazards.set_of(node));
        let on = node
            .and_then(|node| hazards.zone_of(node))
            .is_some_and(|zone| zone.activity.active(time));
        self.zone_on += u32::from(on);
        let was = std::mem::replace(&mut self.zone_in[place], set);
        if was == 0 && set != 0 {
            self.zone_entries.0 += 1;
            let again = self.zone_left[place].is_some_and(|(left, at)| {
                left == set && time.saturating_sub(at) <= super::stuck::ZONE_AGAIN_MS
            });
            self.zone_entries.1 += u32::from(again);
        } else if was != 0 && set == 0 && body.alive {
            self.zone_left[place] = Some((was, time));
        }
        if !body.alive {
            self.zone_left[place] = None;
        }
    }

    pub(crate) fn end_push(&mut self, place: usize) {
        let run = std::mem::take(&mut self.push_run[place]);
        if run >= 5 {
            self.pushes.0 += 1;
            self.pushes.1 += run;
        }
    }

    pub(crate) fn end_idle(&mut self, place: usize) {
        let (run, fighting) = std::mem::take(&mut self.idle_run[place]);
        if run >= 5 {
            if fighting * 2 >= run {
                self.idle_fighting += run;
            } else {
                self.idle.0 += 1;
                self.idle.1 += run;
            }
        }
    }

    pub(crate) fn end_runs(&mut self, place: usize) {
        self.end_push(place);
        self.end_idle(place);
    }

    pub(crate) fn note(&mut self, note: &Note) {
        if note.text.starts_with("is stuck at") {
            if let Some(feet) = self.feet.get(note.place).copied().flatten() {
                self.repeats.0 += u32::from(self.mark(note.place, feet));
            }
            self.stuck.0 += 1;
            self.stuck.1 += u32::from(note.text.contains("(walk keys held"));
        }
    }

    /// A stuck or a fall off its route of `place`'s bot at `at`: whether a
    /// teammate had one there already.
    pub(crate) fn mark(&mut self, place: usize, at: Vec3) -> bool {
        let Some(team) = self.teams.get(place).copied().flatten() else {
            return false;
        };
        let again = self
            .marks
            .iter()
            .any(|&(t, p, spot)| t == team && p != place && super::stuck::near(spot, at));
        self.marks.push((team, place, at));
        again
    }

    /// The line the bench prints (one `routes:` line, `key value` pairs).
    pub(crate) fn line(mut self) -> String {
        for place in 0..self.last.len() {
            self.end_runs(place);
        }
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
        let now = super::path::no_routes();
        let no_route: Vec<u64> = (0..5).map(|k| now[k] - self.no_route[k]).collect();
        let escapes = super::tactics::escapes::counts();
        let escaped: Vec<u64> = (0..5).map(|k| escapes[k] - self.escapes[k]).collect();
        let pockets = super::escape::counts::counts();
        let pocketed: Vec<u64> = (0..7)
            .map(|k| pockets[k] - self.pocket_escapes[k])
            .collect();
        let motor = super::motor::counts::counts();
        let moved: Vec<u64> = (0..8).map(|k| motor[k] - self.motor_counts[k]).collect();
        // Each ending of a route: how many, and their mean age in ms.
        let ends = super::route_end::counts::counts();
        // Sights begun, those through half a metre of bush or more, and the bush metres of all.
        let seen = super::foliage::through::counts();
        let foliage_line = format!(
            " foliage_sights {} foliage_sights_through {} foliage_mm {}",
            seen[0] - self.foliage[0],
            seen[1] - self.foliage[1],
            seen[2] - self.foliage[2]
        );
        // Ways out over a stronger zone than the start's, and drops onto a zone's floor.
        let mut route_ends = format!(" escapes_cross_zone {} escapes_drop_zone {}", escaped[3], escaped[4]);
        route_ends += &foliage_line;
        for (k, why) in super::route_end::RouteEnd::ALL.into_iter().enumerate() {
            let n = ends[k].0 - self.route_ends[k].0;
            let age = ends[k].1 - self.route_ends[k].1;
            route_ends += &format!(
                " route_end_{} {} route_end_{}_age_ms {}",
                why.name(),
                n,
                why.name(),
                age / n.max(1)
            );
        }
        let mean = |v: &[u32]| v.iter().map(|&x| f64::from(x)).sum::<f64>() / v.len().max(1) as f64;
        format!(
            "worlds {} pushes {} push_s {} idle {} idle_s {} idle_fight_s {} reversals {} stuck {} stuck_keys {} carried {} carry_ms {:.0} lost {} no_way {} searching {} starved {} searched {} ended {} falls_off {} falls_planned {} stuck_again {} falls_again {} zone_on_s {} zone_entries {} zone_again {} escapes_back {} escapes_on {} escapes_none {} pocket_s {} pocket_deaths {} pocket_tries {} pocket_worked {} pocket_failed {} pocket_gave_up {} pocket_handed {} pocket_failed_hurt {} pocket_gave_up_hurt {} falls_exact {} falls_exact_taken {} jumps_up_first {} jumps_up_later {} jumps_given_up {} ahead_cuts {} steps_clear {} steps_refused {} lip_guard_changed {} lip_guard_none {}",
            self.worlds,
            self.pushes.0,
            self.pushes.1,
            self.idle.0,
            self.idle.1,
            self.idle_fighting,
            self.reversals,
            self.stuck.0,
            self.stuck.1,
            self.carried.len(),
            mean(&self.carried),
            self.lost,
            no_route[0],
            no_route[1],
            no_route[2],
            no_route[3],
            no_route[4],
            self.falls.0,
            self.falls.1,
            self.repeats.0,
            self.repeats.1,
            self.zone_on,
            self.zone_entries.0,
            self.zone_entries.1,
            escaped[0],
            escaped[1],
            escaped[2],
            self.pocket_worlds,
            self.pocket_deaths,
            pocketed[0],
            pocketed[1],
            pocketed[2],
            pocketed[3],
            pocketed[4],
            pocketed[5],
            pocketed[6],
            self.falls_exact.0,
            self.falls_exact.1,
            moved[0],
            moved[1],
            moved[2],
            moved[3],
            moved[4],
            moved[5],
            moved[6],
            moved[7],
        ) + &route_ends
    }
}
