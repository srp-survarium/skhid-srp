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
    pub(crate) escapes: [u64; 3],
    /// Each body on a pocket's floor at the last world; living bot-worlds
    /// on pockets' floors, deaths there; the pocket escapes' count at the
    /// start (`escape::counts`).
    pub(crate) pocket_in: Vec<bool>,
    pub(crate) pocket_worlds: u32,
    pub(crate) pocket_deaths: u32,
    pub(crate) pocket_escapes: [u64; 7],
    pub(crate) motor_counts: [u64; 6],
    pub(crate) route_ends: [(u64, u64); super::route_end::RouteEnd::ALL.len()],
    /// [CG-17] Each body's walk at the last tick to an exact spot
    /// (`Motor::exact_end`), and when it last landed a planned drop on one;
    /// planned drops on an exact walk (this tick's or the last), and of
    /// them those whose bot picked a battery up within 10 s.
    pub(crate) exact_was: Vec<bool>,
    pub(crate) exact_drop: Vec<Option<u32>>,
    pub(crate) falls_exact: (u32, u32),
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
            exact_was: vec![false; bodies],
            exact_drop: vec![None; bodies],
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
        let now = super::path::no_routes();
        let no_route: Vec<u64> = (0..5).map(|k| now[k] - self.no_route[k]).collect();
        let escapes = super::tactics::escapes::counts();
        let escaped: Vec<u64> = (0..3).map(|k| escapes[k] - self.escapes[k]).collect();
        let pockets = super::escape::counts::counts();
        let pocketed: Vec<u64> = (0..7)
            .map(|k| pockets[k] - self.pocket_escapes[k])
            .collect();
        let motor = super::motor::counts::counts();
        let moved: Vec<u64> = (0..6).map(|k| motor[k] - self.motor_counts[k]).collect();
        // Each ending of a route: how many, and their mean age in ms.
        let ends = super::route_end::counts::counts();
        let mut route_ends = String::new();
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
            "worlds {} pushes {} push_s {} idle {} idle_s {} idle_fight_s {} reversals {} stuck {} stuck_keys {} carried {} carry_ms {:.0} lost {} no_way {} searching {} starved {} searched {} ended {} falls_off {} falls_planned {} stuck_again {} falls_again {} zone_on_s {} zone_entries {} zone_again {} escapes_back {} escapes_on {} escapes_none {} pocket_s {} pocket_deaths {} pocket_tries {} pocket_worked {} pocket_failed {} pocket_gave_up {} pocket_handed {} pocket_failed_hurt {} pocket_gave_up_hurt {} falls_exact {} falls_exact_taken {} jumps_up_first {} jumps_up_later {} jumps_given_up {} ahead_cuts {} steps_clear {} steps_refused {}",
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
        ) + &route_ends
    }
}
