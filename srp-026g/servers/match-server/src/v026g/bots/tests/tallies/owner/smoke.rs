//! The owner counts of smoke (`bots/smoke.rs`), by team: the smoke grenades thrown and the clouds they made (the
//! tracker's), what the clouds took from the bots' eyes (looks at an enemy lost to smoke, sights never begun for
//! it, the aim's target lost in one, ticks of fire sprayed at the last spot seen), and what went through them
//! (hits and kills whose line from the shooter's eye to the victim's chest ran through a cloud, as the clouds
//! hide: `Veil::hides` with the default `thin_m`), and the bodies' time in a cloud and deaths there.
//!
//! The eyes' counts are the bots' own (`smoke::tally`, this thread's, the change over the match); the rest is
//! read from the stand-in game and the tracker's clouds each tick. A smoke throw by a bot comes with the throw
//! uses (PR 2): until then the bench throws none and every count is 0 on both arms.

use super::*;
use crate::v026g::bots::smoke::{self, Cloud, Clouds, SmokeCounts as Thrown};
use crate::v026g::bots::tuning::SmokeTuning;

/// The smoke counts: counted as the match runs, kept for the end.
pub(super) struct SmokeCounts {
    /// The eyes' counts on this thread when the match began (`smoke::tally::counts`).
    eyes_start: [[u64; smoke::tally::EVENTS]; 2],
    /// The tracker's throws and clouds at the last tick, and its clouds then.
    thrown: Thrown,
    clouds: Vec<Cloud>,
    /// By the shooter's team: hits and kills through smoke.
    hits_through: [u32; 2],
    kills_through: [u32; 2],
    /// By the body's team: ticks alive inside a cloud (feet within its radius then), deaths there.
    in_smoke_ticks: [u64; 2],
    deaths_in_smoke: [u32; 2],
}

impl SmokeCounts {
    pub(super) fn new() -> Self {
        Self {
            eyes_start: smoke::tally::counts(),
            thrown: Thrown::default(),
            clouds: Vec::new(),
            hits_through: [0; 2],
            kills_through: [0; 2],
            in_smoke_ticks: [0; 2],
            deaths_in_smoke: [0; 2],
        }
    }

    /// The clouds as a bot's eyes with the default tuning read them at `tick`.
    fn hides(&self, tick: u32, a: Vec3, b: Vec3) -> bool {
        let clouds = Clouds::new(self.clouds.clone());
        clouds.veil(tick, &SmokeTuning::default()).hides(a, b)
    }

    /// Whether `feet` stand inside a cloud at `tick`.
    fn inside(&self, tick: u32, feet: Vec3) -> bool {
        let at = smoke::MatchMs(tick);
        self.clouds.iter().any(|c| c.holds(feet, at))
    }

    /// The tick's hits and kills through smoke, by the shooter's team.
    fn through(&mut self, game: &Game) {
        if self.clouds.is_empty() {
            return;
        }
        let tick = game.tick;
        let line = |shooter: Place, victim: Place| {
            let (s, v) = (&game.bodies[shooter], &game.bodies[victim]);
            (Game::eye(s), Game::chest(v), usize::from(s.team & 1))
        };
        for &(t, shooter, victim) in &game.hit_log {
            let (eye, chest, team) = line(shooter, victim);
            if t == tick && self.hides(tick, eye, chest) {
                self.hits_through[team] += 1;
            }
        }
        for &(t, shooter, victim) in &game.kill_log {
            let (eye, chest, team) = line(shooter, victim);
            if t == tick && shooter != victim && self.hides(tick, eye, chest) {
                self.kills_through[team] += 1;
            }
        }
    }

    fn part(&self) -> Part {
        let now = smoke::tally::counts();
        let eyes: [[u64; smoke::tally::EVENTS]; 2] =
            std::array::from_fn(|team| std::array::from_fn(|e| now[team][e] - self.eyes_start[team][e]));
        let by = |e: smoke::tally::Event| [eyes[0][e as usize], eyes[1][e as usize]];
        use smoke::tally::Event::{AimLost, NotSeen, SightLost, Spray};
        let secs = |ticks: u64| (10.0 * ticks as f64 * f64::from(TICK_S)).round() / 10.0;
        let in_smoke_s = [secs(self.in_smoke_ticks[0]), secs(self.in_smoke_ticks[1])];
        let json = serde_json::json!({
            "smoke_thrown": self.thrown.thrown,
            "smoke_clouds": self.thrown.clouds,
            "smoke_dropped": self.thrown.dropped,
            "smoke_sight_lost": by(SightLost),
            "smoke_not_seen": by(NotSeen),
            "smoke_aim_lost": by(AimLost),
            "smoke_spray_ticks": by(Spray),
            "smoke_hits_through": self.hits_through,
            "smoke_kills_through": self.kills_through,
            "smoke_in_s": in_smoke_s,
            "smoke_deaths_in": self.deaths_in_smoke,
        });
        let text = format!(
            "owner-test smoke (by team 0, 1): thrown {:?} clouds {:?} dropped {:?}; looks lost to smoke {:?}, sights not begun for it {:?}, aim's target lost in it {:?}, spray ticks {:?}; through smoke hits {:?} kills {:?}; time in smoke {:?} s, deaths there {:?}",
            self.thrown.thrown,
            self.thrown.clouds,
            self.thrown.dropped,
            by(SightLost),
            by(NotSeen),
            by(AimLost),
            by(Spray),
            self.hits_through,
            self.kills_through,
            in_smoke_s,
            self.deaths_in_smoke,
        );
        Part::new(json, text)
    }
}

impl Topic for SmokeCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        self.thrown = bots.tracker.smoke_counts();
        self.clouds = bots.tracker.clouds().to_vec();
    }

    fn begin(&mut self, _run: &Run, game: &mut Game) {
        self.through(game);
    }

    fn body(&mut self, _run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        if self.clouds.is_empty() {
            return;
        }
        let body = &at.game.bodies[at.place];
        let team = usize::from(body.team & 1);
        if at.rec.alive && self.inside(at.tick, at.rec.feet) {
            self.in_smoke_ticks[team] += 1;
        }
        if at.prev.alive && !at.rec.alive && self.inside(at.tick, at.prev.feet) {
            self.deaths_in_smoke[team] += 1;
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("smoke", self.part())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::smoke::{MatchMs, SmokeGrenade};

    /// A cloud between a shooter of team 0 and a victim of team 1 counts the hit and the kill through it, by the
    /// shooter's team; the victim stood in it and died there. One aside counts nothing.
    #[test]
    fn hits_and_kills_through_a_cloud_count_for_the_shooters_team() {
        let (mut game, _) = made_up(&[0, 1]);
        let mut counts = SmokeCounts::new();
        put(&mut game, 0, 10.0, 10.0, 0);
        put(&mut game, 1, 30.0, 10.0, 0);
        let cloud = |x: f32, z: f32| Cloud {
            by: 1,
            team: 1,
            centre: Vec3::new(x, 0.0, z),
            thrown: MatchMs(0),
            pours: MatchMs(0),
            shape: SmokeGrenade::RDG_M.cloud,
        };
        game.tick = 5_000;
        game.hit_log = vec![(5_000, 0, 1)];
        game.kill_log = vec![(5_000, 0, 1)];
        counts.clouds = vec![cloud(60.0, 60.0)];
        counts.through(&game);
        assert_eq!((counts.hits_through, counts.kills_through), ([0, 0], [0, 0]));
        counts.clouds = vec![cloud(28.0, 10.0)];
        counts.through(&game);
        assert_eq!((counts.hits_through, counts.kills_through), ([1, 0], [1, 0]));
        assert!(counts.inside(5_000, game.bodies[1].feet) && !counts.inside(5_000, game.bodies[0].feet));
        let part = counts.part();
        assert!(part.text.contains("through smoke hits [1, 0] kills [1, 0]"), "{}", part.text);
    }
}
