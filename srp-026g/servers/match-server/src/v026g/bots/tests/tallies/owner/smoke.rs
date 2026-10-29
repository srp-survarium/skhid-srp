//! The owner counts of smoke (`bots/smoke.rs`), by team: the smoke grenades thrown and the clouds they made (the
//! tracker's), what the clouds took from the bots' eyes (looks at an enemy lost to smoke, sights never begun for
//! it, the aim's target lost in one, ticks of fire sprayed at the last spot seen), what went through them (hits
//! and kills whose line from the shooter's eye to the victim's chest ran through a cloud, as the clouds hide:
//! `Veil::hides` with the default tuning, a spray's apart), and the bodies' time in a cloud and deaths there.
//!
//! **Fairness** (BENCH-DECISIONS D): sightings and fire through smoke, read from outside the bots' sight code so
//! that a later bypass shows: a bot's `Seen` sensing of an enemy (its memory's track refreshed this tick) whose
//! line from its eye to his chest the default veil hides, and a shot at a target through smoke that is not a
//! spray. Both must read 0 with `smoke.sight` on; with it off they count.
//!
//! The eyes' counts are the bots' own (`smoke::tally`, this thread's, the change over the match); the rest is
//! read from the stand-in game, the bots' memories and the tracker's clouds each tick. A smoke throw by a bot comes
//! with the throw uses (PR 2): until then the bench throws none and every count is 0 on both arms.

use std::collections::BTreeMap;

use super::*;
use crate::v026g::bots::know::Source;
use crate::v026g::bots::smoke::{self, Cloud, Clouds, SmokeCounts as Thrown};
use crate::v026g::bots::tuning::SmokeTuning;

/// A shot this near a spray's tick is the spray's, ms.
const SPRAY_NEAR_MS: u32 = 150;

/// The smoke counts: counted as the match runs, kept for the end.
pub(super) struct SmokeCounts {
    /// The eyes' counts on this thread when the match began (`smoke::tally::counts`).
    eyes_start: [[u64; smoke::tally::EVENTS]; 2],
    /// The tracker's throws and clouds at the last tick, and its clouds then; grenade keys of humans whose pockets
    /// are not known (a real cloud the model cannot make).
    thrown: Thrown,
    clouds: Vec<Cloud>,
    unknown_pockets: u32,
    /// By the shooter's team: hits and kills through smoke, a spray's, and others.
    hits_through: [u32; 2],
    spray_hits: [u32; 2],
    kills_through: [u32; 2],
    /// By the body's team: ticks alive inside a cloud (feet within its radius then), deaths there.
    in_smoke_ticks: [u64; 2],
    deaths_in_smoke: [u32; 2],
    /// Fairness, by the bot's team: `Seen` sensings through smoke, and shots at a target through smoke not a spray.
    seen_through: [u32; 2],
    fire_through: [u32; 2],
    /// By bot and enemy: the last time a `Seen` track was refreshed; the tick's new sensings to check.
    seen_times: BTreeMap<(Place, Place), u32>,
    fresh_seen: Vec<(Place, Place)>,
}

impl SmokeCounts {
    pub(super) fn new() -> Self {
        Self {
            eyes_start: smoke::tally::counts(),
            thrown: Thrown::default(),
            clouds: Vec::new(),
            unknown_pockets: 0,
            hits_through: [0; 2],
            spray_hits: [0; 2],
            kills_through: [0; 2],
            in_smoke_ticks: [0; 2],
            deaths_in_smoke: [0; 2],
            seen_through: [0; 2],
            fire_through: [0; 2],
            seen_times: BTreeMap::new(),
            fresh_seen: Vec::new(),
        }
    }

    /// The clouds as a bot's eyes with the default tuning read them at `tick`.
    fn hides(&self, tick: u32, a: Vec3, b: Vec3) -> bool {
        Clouds::new(self.clouds.clone()).covers(tick, &SmokeTuning::default(), a, b)
    }

    /// Whether `feet` stand inside a cloud at `tick`.
    fn inside(&self, tick: u32, feet: Vec3) -> bool {
        let at = smoke::MatchMs(tick);
        self.clouds.iter().any(|c| c.holds(feet, at))
    }

    /// The line from `shooter`'s eye to `victim`'s chest, and the shooter's team.
    fn line(game: &Game, shooter: Place, victim: Place) -> (Vec3, Vec3, usize) {
        let (s, v) = (&game.bodies[shooter], &game.bodies[victim]);
        (Game::eye(s), Game::chest(v), usize::from(s.team & 1))
    }

    /// Whether `shooter` sprayed at `tick` (`smoke::tally::sprayed`).
    fn sprayed(shooter: Place, tick: u32) -> bool {
        smoke::tally::last_spray(shooter).is_some_and(|t| t.abs_diff(tick) <= SPRAY_NEAR_MS)
    }

    /// The tick's hits, kills and shots through smoke, by the shooter's team.
    fn through(&mut self, game: &Game) {
        if self.clouds.is_empty() {
            return;
        }
        let tick = game.tick;
        for &(t, shooter, victim) in &game.hit_log {
            let (eye, chest, team) = Self::line(game, shooter, victim);
            if t == tick && self.hides(tick, eye, chest) {
                if Self::sprayed(shooter, tick) {
                    self.spray_hits[team] += 1;
                } else {
                    self.hits_through[team] += 1;
                }
            }
        }
        for &(t, shooter, victim) in &game.kill_log {
            let (eye, chest, team) = Self::line(game, shooter, victim);
            if t == tick && shooter != victim && self.hides(tick, eye, chest) {
                self.kills_through[team] += 1;
            }
        }
        for fact in &game.shot_facts {
            if let Some((target, _, _)) = fact.target {
                let (eye, chest, team) = Self::line(game, fact.shooter, target);
                if fact.tick == tick && self.hides(tick, eye, chest) && !Self::sprayed(fact.shooter, tick) {
                    self.fire_through[team] += 1;
                }
            }
        }
    }

    /// The bot at `place`'s `Seen` tracks refreshed since the last tick: kept to check against the game's bodies.
    fn read_seen(&mut self, place: Place, tracks: &[crate::v026g::bots::know::Track]) {
        for track in tracks.iter().filter(|t| t.source == Source::Seen) {
            let last = self.seen_times.insert((place, track.who), track.time);
            if last != Some(track.time) {
                self.fresh_seen.push((place, track.who));
            }
        }
    }

    /// The tick's fresh `Seen` sensings of the bot at `place` whose line the clouds hide.
    fn check_seen(&mut self, game: &Game, place: Place, tick: u32) {
        let mine: Vec<Place> = self.fresh_seen.iter().filter(|(p, _)| *p == place).map(|&(_, who)| who).collect();
        self.fresh_seen.retain(|(p, _)| *p != place);
        for who in mine {
            let (eye, chest, team) = Self::line(game, place, who);
            if game.bodies[who].alive && self.hides(tick, eye, chest) {
                self.seen_through[team] += 1;
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
            "smoke_unknown_pockets": self.unknown_pockets,
            "smoke_bodies_seen": self.thrown.bodies_seen,
            "smoke_moved_dm": self.thrown.moved_dm,
            "smoke_sight_lost": by(SightLost),
            "smoke_not_seen": by(NotSeen),
            "smoke_aim_lost": by(AimLost),
            "smoke_spray_ticks": by(Spray),
            "smoke_hits_through": self.hits_through,
            "smoke_spray_hits": self.spray_hits,
            "smoke_kills_through": self.kills_through,
            "smoke_in_s": in_smoke_s,
            "smoke_deaths_in": self.deaths_in_smoke,
            "smoke_fair_seen_through": self.seen_through,
            "smoke_fair_fire_through": self.fire_through,
        });
        let text = format!(
            "owner-test smoke (by team 0, 1): thrown {:?} clouds {:?} dropped {:?} (grenade keys with pockets unknown {}), placed by a world's grenade {:?} moved {:?} dm in all; looks lost to smoke {:?}, sights not begun for it {:?}, aim's target lost in it {:?}, spray ticks {:?}; through smoke hits {:?} (spray {:?}) kills {:?}; time in smoke {:?} s, deaths there {:?}; FAIR (0 with smoke.sight on): seen through smoke {:?}, fire through smoke not a spray {:?}",
            self.thrown.thrown,
            self.thrown.clouds,
            self.thrown.dropped,
            self.unknown_pockets,
            self.thrown.bodies_seen,
            self.thrown.moved_dm,
            by(SightLost),
            by(NotSeen),
            by(AimLost),
            by(Spray),
            self.hits_through,
            self.spray_hits,
            self.kills_through,
            in_smoke_s,
            self.deaths_in_smoke,
            self.seen_through,
            self.fire_through,
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
        self.unknown_pockets = bots.tracker.throw_counts().no_pockets;
        self.clouds = bots.tracker.clouds().to_vec();
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if let Some(bot) = bot {
            self.read_seen(place, bot.memory.tracks());
        }
    }

    fn begin(&mut self, _run: &Run, game: &mut Game) {
        self.through(game);
    }

    fn body(&mut self, _run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        if self.clouds.is_empty() {
            self.fresh_seen.retain(|(p, _)| *p != at.place);
            return;
        }
        self.check_seen(at.game, at.place, at.tick);
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
    use crate::v026g::bots::know::Track;
    use crate::v026g::bots::smoke::{MatchMs, SmokeGrenade};

    fn cloud(x: f32, z: f32) -> Cloud {
        Cloud {
            by: 1,
            team: 1,
            placed: crate::v026g::bots::smoke::Placed::Arc,
            centre: Vec3::new(x, 0.0, z),
            thrown: MatchMs(0),
            pours: MatchMs(0),
            shape: SmokeGrenade::RDG_M.cloud,
        }
    }

    /// A cloud between a shooter of team 0 and a victim of team 1 counts the hit and the kill through it, by the
    /// shooter's team, a spray's hit apart; the victim stood in it. One aside counts nothing.
    #[test]
    fn hits_and_kills_through_a_cloud_count_for_the_shooters_team() {
        let (mut game, _) = made_up(&[0, 1]);
        let mut counts = SmokeCounts::new();
        put(&mut game, 0, 10.0, 10.0, 0);
        put(&mut game, 1, 30.0, 10.0, 0);
        game.tick = 5_000;
        game.hit_log = vec![(5_000, 0, 1)];
        game.kill_log = vec![(5_000, 0, 1)];
        counts.clouds = vec![cloud(60.0, 60.0)];
        counts.through(&game);
        assert_eq!((counts.hits_through, counts.kills_through), ([0, 0], [0, 0]));
        counts.clouds = vec![cloud(28.0, 10.0)];
        counts.through(&game);
        assert_eq!((counts.hits_through, counts.kills_through), ([1, 0], [1, 0]));
        // The same hit while the shooter sprays is a spray's hit.
        smoke::tally::sprayed(0, 4_950);
        game.kill_log.clear();
        counts.through(&game);
        assert_eq!((counts.hits_through, counts.spray_hits), ([1, 0], [1, 0]));
        assert!(counts.inside(5_000, game.bodies[1].feet) && !counts.inside(5_000, game.bodies[0].feet));
        let part = counts.part();
        assert!(part.text.contains("through smoke hits [1, 0] (spray [1, 0]) kills [1, 0]"), "{}", part.text);
    }

    /// The fairness count: a `Seen` sensing through a cloud counts once (a refreshed track), not again while the
    /// track stands, and not when the line is clear.
    #[test]
    fn a_seen_sensing_through_a_cloud_is_counted_once() {
        let (mut game, _) = made_up(&[0, 1]);
        let mut counts = SmokeCounts::new();
        put(&mut game, 0, 10.0, 10.0, 0);
        put(&mut game, 1, 30.0, 10.0, 0);
        let track = |time| Track {
            who: 1,
            at: game.bodies[1].feet,
            error_m: 0.0,
            time,
            source: Source::Seen,
            sight_since: Some(1_000),
            notice_ms: 300,
            own: None,
            missed: None,
            held_until: 0,
        };
        counts.clouds = vec![cloud(20.0, 10.0)];
        counts.read_seen(0, &[track(5_000)]);
        counts.check_seen(&game, 0, 5_000);
        assert_eq!(counts.seen_through, [1, 0]);
        counts.read_seen(0, &[track(5_000)]);
        counts.check_seen(&game, 0, 5_100);
        assert_eq!(counts.seen_through, [1, 0], "the same sensing");
        counts.clouds = vec![cloud(20.0, 40.0)];
        counts.read_seen(0, &[track(5_200)]);
        counts.check_seen(&game, 0, 5_200);
        assert_eq!(counts.seen_through, [1, 0], "a clear line");
    }
}
