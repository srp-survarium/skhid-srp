//! How the bots walk, from the worlds (once a second): the living bot-worlds, the runs of pushing a walk key
//! without moving, the stretches standing still, and the moves that turn back.

use super::*;

/// A move this soon after a step out of a grenade's blast counts as part of it, ms: the walk back to the
/// play's goal after the blast is a turn the step caused.
const AVOID_TAIL_MS: u32 = 3_000;

const WALK: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT;

pub(super) struct Walking {
    /// Each body at the last world: feet, alive; and its runs so far
    /// (worlds holding a walk key without moving, worlds still, worlds of
    /// them in a fight); its last move of 1.5 m or more.
    last: Vec<Option<(Vec3, bool)>>,
    push_run: Vec<u32>,
    idle_run: Vec<(u32, u32)>,
    last_move: Vec<Option<(f32, f32)>>,
    /// Until when (ms) a body's moves count as after a step out of a grenade's blast: the step and
    /// [`AVOID_TAIL_MS`] after it.
    avoid_until: Vec<u32>,
    /// Runs of 5 worlds or more holding a walk key moving under 0.3 m a
    /// world, and their worlds.
    pushes: (u32, u32),
    /// Stretches of 5 s or more moving under 0.5 m a world, and their
    /// seconds: with no target, and in a fight.
    idle: (u32, u32),
    idle_fighting: u32,
    /// Moves of 1.5 m or more turning back over 135° from the one before,
    /// with no target.
    reversals: u32,
    /// ... of them while the bot was stepping out of a grenade's blast or within [`AVOID_TAIL_MS`] of it.
    reversals_avoid: u32,
    /// ... of them while the bot's legs were in the push after a grenade
    /// (`Reflex::after_grenade`), and while a throw was under way (its view
    /// held).
    reversals_grenade: (u32, u32),
    /// Living bot-worlds.
    worlds: u32,
}

impl Walking {
    pub(super) fn new(bodies: usize) -> Self {
        Self {
            last: vec![None; bodies],
            push_run: vec![0; bodies],
            idle_run: vec![(0, 0); bodies],
            last_move: vec![None; bodies],
            avoid_until: vec![0; bodies],
            pushes: (0, 0),
            idle: (0, 0),
            idle_fighting: 0,
            reversals: 0,
            reversals_avoid: 0,
            reversals_grenade: (0, 0),
            worlds: 0,
        }
    }

    fn end_push(&mut self, place: usize) {
        let run = std::mem::take(&mut self.push_run[place]);
        if run >= 5 {
            self.pushes.0 += 1;
            self.pushes.1 += run;
        }
    }

    fn end_idle(&mut self, place: usize) {
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

    fn end_runs(&mut self, place: usize) {
        self.end_push(place);
        self.end_idle(place);
    }
}

impl RouteTopic for Walking {
    fn world(&mut self, _run: &RouteRun, game: &Game, bots: &Bots) {
        for (place, body) in game.bodies.iter().enumerate() {
            if bots.bots[place].as_ref().is_some_and(|bot| {
                matches!(
                    bot.features.avoid.state,
                    crate::v026g::bots::grenade_threat::GrenadeThreat::Stepping { .. }
                )
            }) {
                self.avoid_until[place] = game.tick + AVOID_TAIL_MS;
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
                    let back = cos < (135f32).to_radians().cos();
                    self.reversals += u32::from(back);
                    self.reversals_avoid += u32::from(back && game.tick <= self.avoid_until[place]);
                    if let Some(bot) = bots.bots[place].as_ref() {
                        self.reversals_grenade.0 += u32::from(back && bot.reflex.after_grenade(game.tick));
                        self.reversals_grenade.1 += u32::from(back && bot.reflex.view_held());
                    }
                }
                self.last_move[place] = Some(step);
            } else if moved >= 1.5 {
                self.last_move[place] = None;
            }
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        for place in 0..self.last.len() {
            self.end_runs(place);
        }
        line.main.push(format!(
            "worlds {} pushes {} push_s {} idle {} idle_s {} idle_fight_s {} reversals {} reversals_avoid {}",
            self.worlds,
            self.pushes.0,
            self.pushes.1,
            self.idle.0,
            self.idle.1,
            self.idle_fighting,
            self.reversals,
            self.reversals_avoid
        ));
        line.tail.push(format!(
            "reversals_after_grenade {} reversals_throwing {}",
            self.reversals_grenade.0, self.reversals_grenade.1
        ));
    }
}
