use super::*;

/// A spell of a bot holding fire for a teammate in its line
/// ([`super::friends::Held`]), from its first tick to its last.
#[derive(Clone, Debug)]
pub(crate) struct Spell {
    pub(crate) t0: u32,
    pub(crate) who: Place,
    pub(crate) target: Option<Place>,
    pub(crate) ticks: u32,
    /// Ticks it ran, it was ready to fire but for him, and it and he
    /// moved across the line the same way and opposite ways (both over
    /// [`BlockTally::SIDE_MPS`]).
    pub(crate) run: u32,
    pub(crate) ready: u32,
    pub(crate) same: u32,
    pub(crate) against: u32,
    /// Ticks it held a side key, and he moved across the line (over
    /// [`BlockTally::SIDE_MPS`]).
    pub(crate) stepping: u32,
    pub(crate) his: u32,
    /// At its start: how far along the way to the target he stood, %
    /// (100 at the target).
    pub(crate) along: u32,
    /// At its start: its play, his play, and whether he had a target.
    pub(crate) plays: (String, String, bool),
    /// Its target killed meanwhile: by itself, by a teammate.
    pub(crate) target_killed: Option<bool>,
}

/// One ended spell: ms, running ms, ready ms, same-way ms, its end.
pub(crate) type SpellDone = (u32, u32, u32, u32, &'static str);

/// The real-map bench's blocked spells: every bot's, from the fire step's
/// [`super::friends::Held`], a game tick at a time; each spell as it ends
/// goes to the events (`"spell": [bot, blocker, ms, running ms, ready ms,
/// same-way ms, opposite-way ms, end, side-key ms, blocker-across ms,
/// along %, shooter's play, blocker's play, blocker aiming]`). And what
/// the blockers' own work came to meanwhile, so that a way made for the
/// shooter is seen to cost them nothing (`player_bots_behaviour.md` §2.6):
/// their shots and hits while they block, the time from a
/// new target to the first shot at it, their headway to their goal, and
/// the carriers' times from the take to the put-in (the events'
/// `"mission"` line at the run's end).
#[derive(Default)]
pub(crate) struct BlockTally {
    pub(crate) spells: Vec<Option<Spell>>,
    pub(crate) feet: Vec<Option<Vec3>>,
    pub(crate) kills_seen: usize,
    pub(crate) done: Vec<SpellDone>,
    /// Bot ticks alive.
    pub(crate) alive_ticks: u64,
    /// By place: the game's shots and hits so far; the target and since
    /// when it had it with no shot yet (and whether it blocked someone
    /// meanwhile); the goal's distance a tick ago; the carry's start.
    pub(crate) shots_hits: Vec<(u32, u32)>,
    pub(crate) aiming: Vec<Option<(Place, u32, bool)>>,
    pub(crate) goal_m: Vec<Option<f32>>,
    pub(crate) carry_from: Vec<Option<u32>>,
    /// [not blocking, blocking]: shots, hits; times from a new target to
    /// the first shot, ms; ticks with a goal, and the headway made, m.
    pub(crate) shots: [u32; 2],
    pub(crate) hits: [u32; 2],
    pub(crate) to_fire: [Vec<u32>; 2],
    pub(crate) goal_ticks: [u64; 2],
    pub(crate) headway_m: [f32; 2],
    /// Carries put in: from the take, ms.
    pub(crate) carries: Vec<u32>,
    /// Team hits: in all; the teammate check on the true bodies along the
    /// shot would have held it; on the shooter's reckoning of both. Each
    /// goes to the events (`"teamhit": [shooter, victim, held on the true
    /// bodies, held on the reckoned ones, the shooter's and the victim's
    /// reckoning off (m), the victim's world's age (ms), apart (m), off the
    /// shooter's facing (deg), the target's distance (m), shooter and
    /// victim crouched, the gun's category, held on the view's line before
    /// the spread, the cone (deg), the spread's turn (deg), aim held,
    /// moving]`). The stand-in game has no body collisions: most of its
    /// team hits are bodies standing in one another (under 0.3 m apart),
    /// which the game does not have.
    pub(crate) team_hits: [u32; 3],
    /// [measure] By place: the view's episode on a visible enemy.
    pub(crate) onview: Vec<Option<OnView>>,
    /// [measure] The on-view ticks with no shot by the fire gates.
    pub(crate) gates: fight_tally::GateTally,
    /// [measure] The bots' reloads.
    pub(crate) reloads: fight_tally::ReloadTally,
}

/// [measure] A bot free to fire with its view within 4 deg of a visible
/// enemy within 40 m (the Cologne report's episodes, per tick).
#[derive(Clone, Copy)]
pub(crate) struct OnView {
    pub(crate) enemy: Place,
    pub(crate) start: u32,
    pub(crate) last: u32,
    pub(crate) ticks: u32,
    pub(crate) fired: Option<u32>,
    pub(crate) held: u32,
    pub(crate) false_held: u32,
    pub(crate) new_held: u32,
    pub(crate) near_line: u32,
    pub(crate) distance: f32,
}

pub(crate) fn measure_place_view(game: &Game, place: Place) -> PlaceView {
    let body = &game.bodies[place];
    PlaceView {
        place,
        team: body.team,
        bot: true,
        present: true,
        alive: true,
        feet: body.feet,
        yaw: body.yaw,
        pitch: body.pitch,
        velocity: [0.0, 0.0],
        keys: body.keys,
        health: body.health,
        spawned: body.spawned,
        kills: 0,
        deaths: 0,
        streak: 0,
        carrying: None,
        seen_at: 0,
        late_ms: 0,
        known_at: 0,
    }
}

pub(crate) fn measure_view(game: &Game, places: Vec<PlaceView>) -> MatchView {
    MatchView {
        clock: game.tick,
        mode: super::view::Mode::TeamDeathmatch,
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
    }
}

impl BlockTally {
    /// [measure] With `SRP_BENCH_GATES` set, every fire gate is tested each
    /// tick (`Reflex::gate_tally`: `clear_shot`'s rays though an earlier
    /// gate failed; no shot changes), for the gate tally's "every" counts.
    pub(crate) fn gate_tally(bots: &mut Bots) {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *ON.get_or_init(|| bench_settings().flag("SRP_BENCH_GATES")) {
            for bot in bots.bots.iter_mut().flatten() {
                bot.reflex.gate_tally = true;
            }
        }
    }

    /// [measure] The view-on-enemy episodes, a tick at a time.
    pub(crate) fn onview_tick(&mut self, game: &Game, bots: &Bots, bot: &Bot, fired: bool, events: &mut BenchEvents) {
        let place = bot.place;
        self.onview.resize(game.bodies.len(), None);
        let me = &game.bodies[place];
        let eye = Game::eye(me);
        let [fx, fz] = forward(me.yaw);
        let pitch = me.recoil.view(me.pitch);
        let view_dir = Vec3::new(fx * pitch.cos(), pitch.sin(), fz * pitch.cos());
        let free = me.alive && me.carrying.is_none() && bot.intent.fire;
        let seen = if free {
            game.bodies
                .iter()
                .enumerate()
                .filter(|(e, b)| {
                    *e != place && b.alive && b.team != me.team && b.feet.distance_xz(me.feet) <= 40.0
                })
                .filter_map(|(e, b)| {
                    let to = Game::chest(b) - eye;
                    let off = (to.dot(view_dir) / to.length().max(0.01)).clamp(-1.0, 1.0).acos();
                    (off <= 4f32.to_radians()).then_some((e, off))
                })
                .filter(|&(e, _)| {
                    let b = &game.bodies[e];
                    super::aimpoint::shows(eye, b.feet, b.keys & key::CROUCH != 0, &game.world.sight, || true)
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(e, _)| e)
        } else {
            None
        };
        let now = game.tick;
        let mut ep = self.onview[place].take();
        if let Some(e) = ep
            && (!me.alive || (seen != Some(e.enemy) && now > e.last + 2 * TICK_MS))
        {
            Self::end_onview(place, e, events);
            ep = None;
        }
        if let Some(enemy) = seen {
            let e = ep.get_or_insert(OnView {
                enemy,
                start: now,
                last: now,
                ticks: 0,
                fired: None,
                held: 0,
                false_held: 0,
                new_held: 0,
                near_line: 0,
                distance: game.bodies[enemy].feet.distance_xz(me.feet),
            });
            if e.enemy == enemy {
                e.last = now;
            }
        }
        if let Some(e) = ep.as_mut() {
            e.ticks += 1;
            if !fired {
                self.gates.tick(bot.reflex.gates(), e.enemy, now);
                self.reloads.held(me.team, bot.reflex.gates(), bot.reflex.reload_note());
            }
            if fired && e.fired.is_none() {
                e.fired = Some(now - e.start);
            }
            let target = &game.bodies[e.enemy];
            let to = Game::chest(target) - eye;
            let distance = target.feet.distance_xz(me.feet);
            if let Some(held) = bot.reflex.held_for() {
                e.held += 1;
                let t = bots.tuning.aim.friends;
                let base = super::friends::FriendlyFireTuning {
                    touch_m: 0.0,
                    past_target_m: 2.0,
                    ..t
                };
                let dirs = [view_dir, to * (1.0 / to.length().max(0.01))];
                let view = measure_view(
                    game,
                    vec![measure_place_view(game, place), measure_place_view(game, held.who)],
                );
                let on = |t: &super::friends::FriendlyFireTuning| {
                    super::friends::blocking(
                        &measure_place_view(game, place),
                        &view,
                        &dirs,
                        distance,
                        t.spread(me.gun),
                        0.0,
                        t,
                    )
                    .is_some_and(|b| b.who == held.who)
                };
                let truly = on(&t);
                e.false_held += u32::from(!truly);
                e.new_held += u32::from(truly && !on(&base));
            }
            // A teammate within 1.2 m of the line in plan, between the eye
            // and the enemy (the report's measure).
            let (lx, lz) = (to.x, to.z);
            let len = lx.hypot(lz).max(0.01);
            let near = game.bodies.iter().enumerate().any(|(p, b)| {
                if p == place || !b.alive || b.team != me.team {
                    return false;
                }
                let (dx, dz) = (b.feet.x - eye.x, b.feet.z - eye.z);
                let s = (dx * lx + dz * lz) / len;
                s > 0.0 && s < len && ((dx * lz - dz * lx) / len).abs() <= 1.2
            });
            e.near_line += u32::from(near);
        }
        self.onview[place] = ep;
        self.reloads.tick(place, me.gun.class(), me.team, me.alive, seen.is_some(), fired, bot.reflex.reload_note(), now);
        self.reloads.looped(place, me.gun.class(), me.team, bot.reflex.loop_end());
    }

    pub(crate) fn end_onview(place: Place, e: OnView, events: &mut BenchEvents) {
        if e.last < e.start + 9 * TICK_MS {
            return;
        }
        events.line(serde_json::json!({
            "t": e.start,
            "onview": [place, e.enemy, e.last - e.start + TICK_MS, e.fired, e.ticks, e.held,
                e.false_held, e.new_held, e.near_line, e.distance],
        }));
    }

    /// Sideways speed counted as moving across the line, m/s.
    pub(crate) const SIDE_MPS: f32 = 0.5;

    /// After a game step.
    pub(crate) fn tick(&mut self, game: &Game, bots: &Bots, events: &mut BenchEvents) {
        self.team_hit_tick(game, bots, events);
        let n = game.bodies.len();
        self.spells.resize(n, None);
        self.shots_hits.resize(n, (0, 0));
        self.aiming.resize(n, None);
        self.goal_m.resize(n, None);
        self.carry_from.resize(n, None);
        let moved: Vec<Option<Vec3>> = (0..n)
            .map(|p| {
                let body = &game.bodies[p];
                let before = self.feet.get(p).copied().flatten();
                before.filter(|_| body.alive).map(|b| body.feet - b)
            })
            .collect();
        self.feet = game
            .bodies
            .iter()
            .map(|b| b.alive.then_some(b.feet))
            .collect();
        if game.kill_log.len() < self.kills_seen {
            self.kills_seen = 0;
        }
        for &(_, shooter, victim) in &game.kill_log[self.kills_seen..] {
            for (place, spell) in self.spells.iter_mut().enumerate() {
                if let Some(spell) = spell.as_mut()
                    && spell.target == Some(victim)
                {
                    spell.target_killed = Some(shooter == place);
                }
            }
        }
        self.kills_seen = game.kill_log.len();
        // The blockers now: teammates some bot holds fire for.
        let mut blocking = vec![false; n];
        for spell in self.spells.iter().flatten() {
            blocking[spell.who] = true;
        }
        for bot in bots.bots.iter().flatten() {
            let place = bot.place;
            let body = &game.bodies[place];
            let k = usize::from(blocking[place]);
            let (s, h) = game.shots_hits[place];
            let (s0, h0) = std::mem::replace(&mut self.shots_hits[place], (s, h));
            self.shots[k] += s - s0;
            self.hits[k] += h - h0;
            self.onview_tick(game, bots, bot, s > s0, events);
            // From a new target to the first shot at it.
            let target = bot.reflex.target().filter(|_| body.alive);
            self.aiming[place] = match (target, self.aiming[place]) {
                (Some(t), Some((was, since, blocked))) if t == was => {
                    let blocked = blocked || k == 1;
                    if s > s0 {
                        self.to_fire[usize::from(blocked)].push(game.tick - since);
                        None
                    } else {
                        Some((was, since, blocked))
                    }
                }
                // A new target with no shot this tick (a target taken while
                // firing is not timed).
                (Some(t), None) if s == s0 => Some((t, game.tick, k == 1)),
                (Some(t), Some(_)) if s == s0 => Some((t, game.tick, k == 1)),
                _ => None,
            };
            // Headway to the goal.
            let goal = match bot.intent.go {
                Go::To { at, within_m } if body.alive && body.feet.distance_xz(at) > within_m => {
                    Some(body.feet.distance_xz(at))
                }
                _ => None,
            };
            if let (Some(now), Some(before)) = (goal, self.goal_m[place]) {
                self.goal_ticks[k] += 1;
                self.headway_m[k] += (before - now).clamp(-1.0, 1.0);
            }
            self.goal_m[place] = goal;
            if body.alive {
                self.alive_ticks += 1;
            }
            self.spell_tick(game, bots, bot, &moved, events);
        }
        // Carries: from the take to the put-in.
        for (place, body) in game.bodies.iter().enumerate() {
            match (body.carrying.is_some(), self.carry_from[place]) {
                (true, None) => self.carry_from[place] = Some(game.tick),
                (false, Some(from)) => {
                    if game.put_in.contains(&place) {
                        self.carries.push(game.tick - from);
                    }
                    self.carry_from[place] = None;
                }
                _ => {}
            }
        }
    }

    /// Each team hit of the step: would the teammate check have held the
    /// shot on the true bodies (the shot's own eye and line), and on the
    /// bodies as the shooter reckoned them; how far the reckoning was off.
    pub(crate) fn team_hit_tick(&mut self, game: &Game, bots: &Bots, events: &mut BenchEvents) {
        let t = super::friends::FriendlyFireTuning::default();
        let place_view = |place: Place, feet: Vec3, yaw: f32, pitch: f32| {
            let body = &game.bodies[place];
            PlaceView {
                place,
                team: body.team,
                bot: true,
                present: true,
                alive: true,
                feet,
                yaw,
                pitch,
                velocity: [0.0, 0.0],
                keys: body.keys,
                health: body.health,
                spawned: body.spawned,
                kills: 0,
                deaths: 0,
                streak: 0,
                carrying: None,
                seen_at: 0,
                late_ms: 0,
                known_at: 0,
            }
        };
        let view = |places: Vec<PlaceView>| MatchView {
            clock: game.tick,
            mode: super::view::Mode::TeamDeathmatch,
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
        };
        for &(shooter, victim, _eye, dir, cone, view_dir) in &game.team_hit_shots {
            let (a, b) = (&game.bodies[shooter], &game.bodies[victim]);
            let distance = a
                .target
                .map_or(60.0, |target| game.bodies[target].feet.distance_xz(a.feet));
            let spread = t.spread(a.gun);
            let held_on = |me: PlaceView, him: PlaceView, dir: Vec3| {
                super::friends::blocking(&me, &view(vec![me.clone(), him]), &[dir], distance, spread, 0.0, &t)
                    .is_some_and(|blocked| blocked.who == victim)
            };
            let held = |me: PlaceView, him: PlaceView| held_on(me, him, dir);
            let on_view = held_on(
                place_view(shooter, a.feet, a.yaw, a.pitch),
                place_view(victim, b.feet, b.yaw, b.pitch),
                view_dir,
            );
            let deflected = view_dir.dot(dir).clamp(-1.0, 1.0).acos().to_degrees();
            let truly = held(
                place_view(shooter, a.feet, a.yaw, a.pitch),
                place_view(victim, b.feet, b.yaw, b.pitch),
            );
            let estimate = |place: Place| bots.tracker.body(place).and_then(|body| body.estimate()).copied();
            let (ea, eb) = (estimate(shooter), estimate(victim));
            let reckoned = match (ea, eb) {
                (Some(ea), Some(eb)) => held(
                    place_view(shooter, ea.feet, ea.yaw, ea.pitch),
                    place_view(victim, eb.feet, eb.yaw, eb.pitch),
                ),
                _ => false,
            };
            let off = |e: Option<super::body::Estimate>, body: &Body| e.map_or(-1.0, |e| e.feet.distance_xz(body.feet));
            let fix_age = bots
                .tracker
                .body(victim)
                .and_then(|body| body.fix())
                .map_or(0, |fix| game.tick.saturating_sub(fix.time));
            let to = b.feet - a.feet;
            let [fx, fz] = forward(a.yaw);
            let off_deg = (fx * to.z - fz * to.x).atan2(fx * to.x + fz * to.z).abs().to_degrees();
            self.team_hits[0] += 1;
            self.team_hits[1] += u32::from(truly);
            self.team_hits[2] += u32::from(reckoned);
            events.line(serde_json::json!({
                "t": game.tick,
                "teamhit": [shooter, victim, truly, reckoned, off(ea, a), off(eb, b), fix_age,
                    a.feet.distance_xz(b.feet), off_deg, distance,
                    a.keys & key::CROUCH != 0, b.keys & key::CROUCH != 0, a.gun.category,
                    on_view, cone, deflected, a.keys & key::AIM != 0, bench_spread::moving(a.keys)],
            }));
        }
    }

    pub(crate) fn spell_tick(
        &mut self,
        game: &Game,
        bots: &Bots,
        bot: &Bot,
        moved: &[Option<Vec3>],
        events: &mut BenchEvents,
    ) {
        let place = bot.place;
        let body = &game.bodies[place];
        let Some(held) = bot.reflex.held_for().filter(|_| body.alive) else {
            if let Some(spell) = self.spells[place].take() {
                let end = if !body.alive {
                    "died"
                } else {
                    match spell.target_killed {
                        Some(true) => "target killed by it",
                        Some(false) => "target killed by another",
                        None if bot.reflex.target() != spell.target => "target lost",
                        None => "cleared",
                    }
                };
                self.end(place, spell, end, events);
            }
            return;
        };
        let spell = self.spells[place].get_or_insert_with(|| Spell {
            t0: game.tick,
            who: held.who,
            target: bot.reflex.target(),
            ticks: 0,
            run: 0,
            ready: 0,
            same: 0,
            against: 0,
            stepping: 0,
            his: 0,
            along: bot.reflex.target().map_or(0, |t| {
                let to = game.bodies[t].feet.distance_xz(body.feet).max(0.1);
                (100.0 * game.bodies[held.who].feet.distance_xz(body.feet) / to) as u32
            }),
            plays: {
                let play = |p: Place| {
                    bots.bots[p]
                        .as_ref()
                        .and_then(|b| b.assignment.as_ref())
                        .map_or("none".to_owned(), |a| format!("{:?}", a.kind))
                };
                let aiming = bots.bots[held.who]
                    .as_ref()
                    .is_some_and(|b| b.reflex.target().is_some());
                (play(place), play(held.who), aiming)
            },
            target_killed: None,
        });
        spell.who = held.who;
        spell.ticks += 1;
        spell.run += u32::from(held.running);
        spell.ready += u32::from(held.ready);
        spell.stepping += u32::from(body.keys & (key::LEFT | key::RIGHT) != 0);
        let across = game.bodies[held.who].feet - body.feet;
        let side = Vec3::new(-across.z, 0.0, across.x);
        let len = side.length();
        if let (Some(a), Some(b)) = (moved[place], moved[held.who])
            && len > 0.01
        {
            let dt = TICK_MS as f32 / 1000.0;
            let (va, vb) = (a.dot(side) / len / dt, b.dot(side) / len / dt);
            spell.his += u32::from(vb.abs() > Self::SIDE_MPS);
            if va.abs() > Self::SIDE_MPS && vb.abs() > Self::SIDE_MPS {
                if va.signum() == vb.signum() {
                    spell.same += 1;
                } else {
                    spell.against += 1;
                }
            }
        }
    }

    pub(crate) fn end(&mut self, place: Place, spell: Spell, end: &'static str, events: &mut BenchEvents) {
        let ms = |ticks: u32| ticks * TICK_MS;
        events.line(serde_json::json!({
            "t": spell.t0,
            "spell": [place, spell.who, ms(spell.ticks), ms(spell.run), ms(spell.ready),
                ms(spell.same), ms(spell.against), end, ms(spell.stepping), ms(spell.his), spell.along,
                spell.plays.0, spell.plays.1, spell.plays.2],
        }));
        self.done.push((
            ms(spell.ticks),
            ms(spell.run),
            ms(spell.ready),
            ms(spell.same),
            end,
        ));
    }

    /// The spells under way at the end, and one line on all of them.
    pub(crate) fn finish(&mut self, events: &mut BenchEvents) -> String {
        for place in 0..self.onview.len() {
            if let Some(e) = self.onview[place].take() {
                Self::end_onview(place, e, events);
            }
        }
        for place in 0..self.spells.len() {
            if let Some(spell) = self.spells[place].take() {
                self.end(place, spell, "the run's end", events);
            }
        }
        events.line(serde_json::json!({
            "mission": {
                "shots": self.shots, "hits": self.hits,
                "to_fire_ms": [&self.to_fire[0], &self.to_fire[1]],
                "goal_ticks": self.goal_ticks, "headway_m": self.headway_m,
                "carries_ms": &self.carries,
                "team_hits": self.team_hits,
            }
        }));
        let mut ms: Vec<u32> = self.done.iter().map(|d| d.0).collect();
        ms.sort_unstable();
        let at = |q: f32| {
            ms.get(((ms.len() as f32 * q) as usize).min(ms.len().saturating_sub(1)))
                .copied()
                .unwrap_or(0)
        };
        let total: u64 = ms.iter().map(|&m| u64::from(m)).sum();
        let share = |f: fn(&SpellDone) -> u32| {
            let part: u64 = self.done.iter().map(|d| u64::from(f(d))).sum();
            100.0 * part as f32 / total.max(1) as f32
        };
        let mut ends: std::collections::BTreeMap<&str, u32> = Default::default();
        for d in &self.done {
            *ends.entry(d.4).or_default() += 1;
        }
        let bot_min = self.alive_ticks as f32 * TICK_MS as f32 / 60_000.0;
        format!(
            "blocked: {} spells, {:.1} s in all ({:.2} s a bot-minute alive), over 1 s {}; median {} ms, 90th {} ms, longest {} ms; running {:.0} %, ready to fire {:.0} %, moving the same way as the blocker {:.0} %; ends {ends:?}; blockers hit {} of {} shots",
            ms.len(),
            total as f32 / 1000.0,
            total as f32 / 1000.0 / bot_min.max(1e-3),
            ms.iter().filter(|&&m| m > 1000).count(),
            at(0.5),
            at(0.9),
            ms.last().copied().unwrap_or(0),
            share(|d| d.1),
            share(|d| d.2),
            share(|d| d.3),
            self.hits[1],
            self.shots[1],
        )
    }
}
