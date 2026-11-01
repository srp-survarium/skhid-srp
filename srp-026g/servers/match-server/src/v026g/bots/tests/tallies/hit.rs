use super::*;

/// A bot hit by an enemy, followed for [`HitTally::FOLLOW_MS`].
#[derive(Clone, Debug)]
pub(crate) struct HitFollow {
    pub(crate) t0: u32,
    pub(crate) shooter: Place,
    /// It knew of him at the hit: its target, or a track of him sensed
    /// within [`HitTally::SENSED_MS`].
    pub(crate) sensed: bool,
    /// Its view off the way to him at the hit, degrees, and his distance.
    pub(crate) off_deg: f32,
    pub(crate) distance: f32,
    /// Ms from the hit: its view within [`HitTally::FACED_DEG`] of him,
    /// him its target, its first hit on him.
    pub(crate) faced: Option<u32>,
    pub(crate) targeted: Option<u32>,
    pub(crate) hit_back: Option<u32>,
    pub(crate) died: bool,
    pub(crate) killed_him: bool,
    /// [A227] Ms from the hit to its look on a hit marker's direction (`aim.combat.hit_bearing`), and how far that direction
    /// was off the way to him then, degrees.
    pub(crate) marked: Option<(u32, f32)>,
}

/// The real-map bench's hits taken (audit issue 8, `hit_bearing`): each
/// hit on a living bot that it had no follow of already starts one, and
/// what it did in the next [`HitTally::FOLLOW_MS`]: turned to the
/// shooter, took him as its target, hit him back, killed him, died. Split
/// by whether the bot sensed him at the hit. Each follow goes to the
/// events as a `"hit"` line: [bot, shooter, sensed, off degrees,
/// distance, faced ms, targeted ms, hit back ms, died, killed him].
#[derive(Default)]
pub(crate) struct HitTally {
    pub(crate) follows: Vec<Option<HitFollow>>,
    /// [not sensed, sensed]: follows; faced within 1 s; targeted within
    /// 1.5 s; hit back; died; killed him.
    pub(crate) counts: [[u32; 6]; 2],
}

impl HitTally {
    pub(crate) const FOLLOW_MS: u32 = 3_000;
    pub(crate) const SENSED_MS: u32 = 1_500;
    pub(crate) const FACED_DEG: f32 = 20.0;

    /// How far a view at `yaw` from `from` is off the way to `to`, degrees.
    pub(crate) fn off(yaw: f32, from: Vec3, to: Vec3) -> f32 {
        use super::kin::{wrap, yaw_of};
        wrap(yaw - yaw_of([to.x - from.x, to.z - from.z])).abs().to_degrees()
    }

    /// After a game step: takes the game's hits.
    pub(crate) fn tick(&mut self, game: &mut Game, bots: &Bots, events: &mut BenchEvents) {
        let tick = game.tick;
        let n = game.bodies.len();
        self.follows.resize(n, None);
        let hits = std::mem::take(&mut game.hit_log);
        for &(at, shooter, victim) in &hits {
            // Its hit back (and the kill, a hit that left him dead).
            if let Some(follow) = self.follows[shooter].as_mut()
                && follow.shooter == victim
            {
                follow.hit_back.get_or_insert(at - follow.t0);
                let him = &game.bodies[victim];
                if !him.alive && him.dead_at == at {
                    follow.killed_him = true;
                }
            }
            let body = &game.bodies[victim];
            if !body.alive && body.dead_at == at {
                Self::mates(game, bots, at, shooter, victim, events);
            }
            let Some(bot) = bots.bots[victim].as_ref() else {
                continue;
            };
            if !body.alive || self.follows[victim].is_some() {
                continue;
            }
            let sensed = bot.reflex.target() == Some(shooter)
                || bot
                    .memory
                    .tracks()
                    .iter()
                    .any(|t| t.who == shooter && t.time + Self::SENSED_MS >= at);
            let to = game.bodies[shooter].feet;
            self.follows[victim] = Some(HitFollow {
                t0: at,
                shooter,
                sensed,
                off_deg: Self::off(body.yaw, body.feet, to),
                distance: body.feet.distance_xz(to),
                faced: None,
                targeted: None,
                hit_back: None,
                died: false,
                killed_him: false,
                marked: None,
            });
        }
        for place in 0..n {
            let Some(follow) = self.follows[place].as_mut() else {
                continue;
            };
            let body = &game.bodies[place];
            let since = tick.saturating_sub(follow.t0);
            if body.alive {
                let to = game.bodies[follow.shooter].feet;
                if follow.faced.is_none()
                    && Self::off(body.yaw, body.feet, to) <= Self::FACED_DEG
                {
                    follow.faced = Some(since);
                }
                if follow.targeted.is_none()
                    && bots.bots[place]
                        .as_ref()
                        .is_some_and(|b| b.reflex.target() == Some(follow.shooter))
                {
                    follow.targeted = Some(since);
                }
                if follow.marked.is_none()
                    && let Some(toward) = bots.bots[place].as_ref().and_then(|b| b.reflex.marker(tick))
                {
                    use super::kin::{wrap, yaw_of};
                    let truth = yaw_of([to.x - body.feet.x, to.z - body.feet.z]);
                    follow.marked = Some((since, wrap(toward - truth).abs().to_degrees()));
                }
            } else {
                follow.died = true;
            }
            if !body.alive || since >= Self::FOLLOW_MS || follow.killed_him {
                let follow = self.follows[place].take().unwrap();
                self.end(place, follow, events);
            }
        }
    }

    /// At a kill: the victim's living teammates within 30 m, as a
    /// `"mates"` line: [victim, killer, [[mate, metres from the victim,
    /// metres from the killer, a line from its eye to his chest, it knew of
    /// him (a track within [`Self::SENSED_MS`]), him its target], ...]].
    pub(crate) fn mates(
        game: &Game,
        bots: &Bots,
        at: u32,
        killer: Place,
        victim: Place,
        events: &mut BenchEvents,
    ) {
        if events.0.is_none() {
            return;
        }
        let dead = &game.bodies[victim];
        let him = &game.bodies[killer];
        let mates: Vec<serde_json::Value> = bots
            .bots
            .iter()
            .flatten()
            .filter(|b| b.place != victim && b.team == dead.team)
            .filter(|b| game.bodies[b.place].alive)
            .filter(|b| game.bodies[b.place].feet.distance_xz(dead.feet) <= 30.0)
            .map(|b| {
                let body = &game.bodies[b.place];
                let line = game
                    .world
                    .sight
                    .visible(Game::eye(body), Game::chest(him));
                let knew = b
                    .memory
                    .tracks()
                    .iter()
                    .any(|t| t.who == killer && t.time + Self::SENSED_MS >= at);
                serde_json::json!([
                    b.place,
                    (body.feet.distance_xz(dead.feet) * 10.0).round() / 10.0,
                    (body.feet.distance_xz(him.feet) * 10.0).round() / 10.0,
                    line,
                    knew,
                    b.reflex.target() == Some(killer),
                ])
            })
            .collect();
        events.line(serde_json::json!({"t": at, "mates": [victim, killer, mates]}));
    }

    pub(crate) fn end(&mut self, place: Place, f: HitFollow, events: &mut BenchEvents) {
        events.line(serde_json::json!({
            "t": f.t0,
            "hit": [place, f.shooter, f.sensed, f.off_deg.round(), (f.distance * 10.0).round() / 10.0,
                f.faced, f.targeted, f.hit_back, f.died, f.killed_him,
                f.marked.map(|(ms, off)| [f64::from(ms), f64::from(off.round())])],
        }));
        let row = &mut self.counts[usize::from(f.sensed)];
        row[0] += 1;
        row[1] += u32::from(f.faced.is_some_and(|ms| ms <= 1_000));
        row[2] += u32::from(f.targeted.is_some_and(|ms| ms <= 1_500));
        row[3] += u32::from(f.hit_back.is_some());
        row[4] += u32::from(f.died);
        row[5] += u32::from(f.killed_him);
    }

    /// The follows under way at the end, and one line on all of them.
    pub(crate) fn finish(&mut self, events: &mut BenchEvents) -> String {
        for place in 0..self.follows.len() {
            if let Some(follow) = self.follows[place].take() {
                self.end(place, follow, events);
            }
        }
        let line = |row: &[u32; 6]| {
            let pc = |k: usize| 100.0 * row[k] as f32 / row[0].max(1) as f32;
            format!(
                "{} hits; faced him within 1 s {:.0} %, took him as target within 1.5 s {:.0} %, hit him back {:.0} %, died {:.0} %, killed him {:.0} %",
                row[0],
                pc(1),
                pc(2),
                pc(3),
                pc(4),
                pc(5)
            )
        };
        format!(
            "hits taken: shooter not sensed: {}; sensed: {}",
            line(&self.counts[0]),
            line(&self.counts[1])
        )
    }
}
