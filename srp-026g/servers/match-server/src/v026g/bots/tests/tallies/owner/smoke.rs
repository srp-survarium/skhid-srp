//! The owner counts of smoke (`bots/smoke.rs`), by team: the smoke grenades thrown and the clouds they made (the
//! tracker's), what the clouds took from the bots' eyes (looks at an enemy lost to smoke, sights never begun for
//! it, the aim's target lost in one, ticks of fire sprayed at the last spot seen), what went through them (hits
//! and kills whose line from the shooter's eye to the victim's chest ran through a cloud, as the clouds hide:
//! `Veil::hides` with the default tuning, a spray's apart), and the bodies' time in a cloud and deaths there.
//!
//! **Fairness** (BENCH-DECISIONS D): sightings and fire through smoke, read from outside the bots' sight code so
//! that a path that bypasses the veil shows. The definition (coordinator, 2026-10-29): a count is a sighting or a
//! shot where **every part a look tests is clearly behind smoke**:
//! - a sighting: a bot's `Seen` track refreshed this tick (its memory's), from its eye in the game to where that track
//!   has him (the place its view sensed);
//! - a shot: a shot at a target that is not a spray, from the shooter's eye to where its last `Seen` track had him;
//! - "every part": each part of `aimpoint::Part::SIGHT_ORDER`, standing and crouched, is hidden;
//! - "clearly": by [`FAIR_MARGIN_M`] (0.5 m) more smoke than the bots' 2 m rule, so the few tenths of a metre between
//!   the bot's view and the game's bodies at a cloud's edge do not count as a bypass.
//!
//! Both read 0 with `smoke.sight` on and count with it off: the loop test below reads 0 / 0 on and 1500 / 131 off.
//!
//! The eyes' counts are the bots' own (`smoke::tally`, this thread's, the change over the match); the rest is
//! read from the stand-in game, the bots' memories and the tracker's clouds each tick. A smoke throw by a bot comes
//! with the throw uses (PR 2): until then the bench throws none and every count is 0 on both arms.

use std::collections::BTreeMap;

use super::*;
use crate::v026g::bots::aimpoint::Part as BodyPart;
use crate::v026g::bots::know::Source;
use crate::v026g::bots::smoke::{self, Cloud, Clouds, SmokeCounts as Thrown};
use crate::v026g::bots::tuning::SmokeTuning;

/// A shot this near a spray's tick is the spray's, ms.
const SPRAY_NEAR_MS: u32 = 150;
/// The fairness counts take a line as through smoke past this much more smoke than the bots' rule, metres: the bots
/// look from and at their view's places, which lag the game's bodies the counts read by up to a few tenths of a metre.
const FAIR_MARGIN_M: f32 = 0.5;

/// A fairness case counted: the bot, the enemy, the tick, its eye, his feet as sensed.
#[cfg(test)]
type Case = (Place, Place, u32, Vec3, Vec3);

#[cfg(test)]
thread_local! {
    /// The fairness cases counted, for a test to look at.
    static SEEN_CASES: std::cell::RefCell<Vec<Case>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The smoke counts: counted as the match runs, kept for the end.
pub(super) struct SmokeCounts {
    /// The eyes' counts on this thread when the match began (`smoke::tally::counts`).
    eyes_start: [[u64; smoke::tally::EVENTS]; 2],
    throws_start: smoke::tally::Throws,
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
    /// By bot and enemy: where its last `Seen` track had him (what its shots are aimed from).
    sensed: BTreeMap<(Place, Place), Vec3>,
    fresh_seen: Vec<(Place, Place, Vec3)>,
}

impl SmokeCounts {
    pub(super) fn new() -> Self {
        Self {
            eyes_start: smoke::tally::counts(),
            throws_start: smoke::tally::throws(),
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
            sensed: BTreeMap::new(),
            fresh_seen: Vec::new(),
        }
    }

    /// The clouds as a bot's eyes with the default tuning read them at `tick`.
    fn hides(&self, tick: u32, a: Vec3, b: Vec3) -> bool {
        Clouds::new(self.clouds.clone()).covers(tick, &SmokeTuning::default(), a, b)
    }

    /// Whether the clouds hide every part a look tests of a body with its feet at `feet` (`aimpoint::Part::SIGHT_ORDER`,
    /// standing and crouched): a man at a cloud's edge with a part out of it is seen, as a look sees him.
    ///
    /// With [`FAIR_MARGIN_M`] more smoke than the bots' rule: the bot looks from where its view has it and at where
    /// its view has him, up to a few tenths of a metre from the game's bodies here, which at a cloud's edge moves a
    /// line across the 2 m.
    fn hides_body(&self, tick: u32, eye: Vec3, feet: Vec3) -> bool {
        let clouds = Clouds::new(self.clouds.clone());
        let default = SmokeTuning::default();
        let tuning = SmokeTuning {
            thin_m: crate::v026g::bots::tuning::Positive::of(default.thin_m.get() + FAIR_MARGIN_M),
            ..default
        };
        let veil = clouds.veil(tick, &tuning);
        [false, true].iter().all(|&crouched| {
            BodyPart::SIGHT_ORDER
                .iter()
                .all(|part| veil.hides(eye, part.point_of(feet, crouched, eye)))
        })
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
                let (eye, _, team) = Self::line(game, fact.shooter, target);
                // Where the shooter's eyes have him (its memory's sight), else the game's body.
                let feet = self.sensed.get(&(fact.shooter, target)).copied().unwrap_or(game.bodies[target].feet);
                if fact.tick == tick && self.hides_body(tick, eye, feet) && !Self::sprayed(fact.shooter, tick) {
                    self.fire_through[team] += 1;
                    #[cfg(test)]
                    SEEN_CASES.with(|c| c.borrow_mut().push((fact.shooter, target, tick, eye, feet)));
                }
            }
        }
    }

    /// The bot at `place`'s `Seen` tracks refreshed since the last tick: kept to check against the game's bodies.
    fn read_seen(&mut self, place: Place, tracks: &[crate::v026g::bots::know::Track]) {
        for track in tracks.iter().filter(|t| t.source == Source::Seen) {
            let last = self.seen_times.insert((place, track.who), track.time);
            self.sensed.insert((place, track.who), track.at);
            if last != Some(track.time) {
                self.fresh_seen.push((place, track.who, track.at));
            }
        }
    }

    /// The tick's fresh `Seen` sensings of the bot at `place` whose every part the clouds hide: from its eye to where
    /// it sensed him (the track's place, the view's: the game's body is later than what the bot saw).
    fn check_seen(&mut self, game: &Game, place: Place, tick: u32) {
        let mine: Vec<(Place, Vec3)> =
            self.fresh_seen.iter().filter(|(p, _, _)| *p == place).map(|&(_, who, at)| (who, at)).collect();
        self.fresh_seen.retain(|(p, _, _)| *p != place);
        let me = &game.bodies[place];
        for (who, at) in mine {
            let (eye, team) = (Game::eye(me), usize::from(me.team & 1));
            if game.bodies[who].alive && self.hides_body(tick, eye, at) {
                self.seen_through[team] += 1;
                #[cfg(test)]
                SEEN_CASES.with(|c| c.borrow_mut().push((place, who, tick, eye, at)));
            }
        }
    }

    fn part(&self) -> Part {
        let now = smoke::tally::counts();
        let eyes: [[u64; smoke::tally::EVENTS]; 2] =
            std::array::from_fn(|team| std::array::from_fn(|e| now[team][e] - self.eyes_start[team][e]));
        let by = |e: smoke::tally::Event| [eyes[0][e as usize], eyes[1][e as usize]];
        use smoke::tally::Event::{
            AimLost, EdgeWatch, LearntHeard, LearntSeen, LearntTold, NotSeen, Outranged, OutrangedNoPlan, SightLost, Spray, WatchBusy,
        };
        let secs = |ticks: u64| (10.0 * ticks as f64 * f64::from(TICK_S)).round() / 10.0;
        let in_smoke_s = [secs(self.in_smoke_ticks[0]), secs(self.in_smoke_ticks[1])];
        // The bots' smoke throws, by team and use: [begun, thrown, given up]; and the thrown by team and role.
        let now_throws = smoke::tally::throws();
        let roles = survarium::v026g::loadouts::BotRole::ALL;
        let uses = smoke::SmokeUse::ALL;
        let throw = |team: usize, role: usize, u: usize, step: usize| {
            now_throws[team][role][u][step] - self.throws_start[team][role][u][step]
        };
        let by_use: [[[u64; smoke::tally::THROW_STEPS]; smoke::SmokeUse::ALL.len()]; 2] = std::array::from_fn(|team| {
            std::array::from_fn(|u| std::array::from_fn(|step| (0..roles.len()).map(|r| throw(team, r, u, step)).sum()))
        });
        let thrown_by_role: [[u64; survarium::v026g::loadouts::BotRole::ALL.len()]; 2] =
            std::array::from_fn(|team| std::array::from_fn(|r| (0..uses.len()).map(|u| throw(team, r, u, 1)).sum()));
        let uses_json: serde_json::Map<String, serde_json::Value> = uses
            .iter()
            .map(|u| (format!("{u:?}"), serde_json::json!([by_use[0][u.index()], by_use[1][u.index()]])))
            .collect();
        let roles_json: serde_json::Map<String, serde_json::Value> = roles
            .iter()
            .enumerate()
            .map(|(r, role)| (role.name().to_owned(), serde_json::json!([thrown_by_role[0][r], thrown_by_role[1][r]])))
            .collect();
        let json = serde_json::json!({
            "smoke_bot_throws_by_use": uses_json,
            "smoke_bot_thrown_by_role": roles_json,
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
            "smoke_edge_watches": by(EdgeWatch),
            "smoke_watches_busy": by(WatchBusy),
            "smoke_learnt_seen": by(LearntSeen),
            "smoke_learnt_heard": by(LearntHeard),
            "smoke_learnt_told": by(LearntTold),
            "smoke_outranged_planned": by(Outranged),
            "smoke_outranged_no_plan": by(OutrangedNoPlan),
            "smoke_hits_through": self.hits_through,
            "smoke_spray_hits": self.spray_hits,
            "smoke_kills_through": self.kills_through,
            "smoke_in_s": in_smoke_s,
            "smoke_deaths_in": self.deaths_in_smoke,
            "smoke_fair_seen_through": self.seen_through,
            "smoke_fair_fire_through": self.fire_through,
        });
        let text = format!(
            "owner-test smoke (by team 0, 1): bots' smoke throws [begun, thrown, given up, died] by use {:?}, thrown by role {:?}; thrown {:?} clouds {:?} dropped {:?} (grenade keys with pockets unknown {}), placed by a world's grenade {:?} moved {:?} dm in all; looks lost to smoke {:?}, sights not begun for it {:?}, aim's target lost in it {:?}, spray ticks {:?}, smoke edges watched {:?} (busy {:?}), clouds learnt seen {:?} heard {:?} told {:?}, outranged by a seen sniper with a smoke: planned {:?} no plan {:?}; through smoke hits {:?} (spray {:?}) kills {:?}; time in smoke {:?} s, deaths there {:?}; FAIR (0 with smoke.sight on): seen through smoke {:?}, fire through smoke not a spray {:?}",
            uses.iter().map(|u| (format!("{u:?}"), [by_use[0][u.index()], by_use[1][u.index()]])).collect::<Vec<_>>(),
            roles.iter().enumerate().map(|(r, role)| (role.name(), [thrown_by_role[0][r], thrown_by_role[1][r]])).collect::<Vec<_>>(),
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
            by(EdgeWatch),
            by(WatchBusy),
            by(LearntSeen),
            by(LearntHeard),
            by(LearntTold),
            by(Outranged),
            by(OutrangedNoPlan),
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
            self.fresh_seen.retain(|(p, _, _)| *p != at.place);
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

    /// A shot at a target behind a cloud counts as fire through smoke unless the shooter sprays then.
    #[test]
    fn a_shot_through_a_cloud_counts_unless_it_is_a_spray() {
        use crate::v026g::bots::tests::tallies::fight_tally::ShotFact;
        let (mut game, _) = made_up(&[0, 1]);
        let mut counts = SmokeCounts::new();
        put(&mut game, 0, 10.0, 10.0, 0);
        put(&mut game, 1, 30.0, 10.0, 0);
        counts.clouds = vec![cloud(20.0, 10.0)];
        game.tick = 6_000;
        game.shot_facts = vec![ShotFact {
            shooter: 0,
            tick: 6_000,
            hit: None,
            target: Some((1, 20.0, 0.0)),
            moving: false,
            aimed: true,
            crouched: false,
            aim_state: Default::default(),
            cone: 0.0,
            offset: None,
            landed: Default::default(),
        }];
        counts.through(&game);
        assert_eq!(counts.fire_through, [1, 0]);
        smoke::tally::sprayed(0, 6_000);
        counts.through(&game);
        assert_eq!(counts.fire_through, [1, 0], "a spray's shot");
        counts.clouds = vec![cloud(20.0, 40.0)];
        smoke::tally::sprayed(0, 0);
        counts.through(&game);
        assert_eq!(counts.fire_through, [1, 0], "a clear line");
    }

    /// The fairness counts in a real loop (the stand-in game, the bots' poll, the owner tally): 2 against 2 for 60 s on
    /// the soak field, three clouds that never end across its open passage (x 36-44, z 24-41; the wall closes the rest).
    /// With `smoke.sight` off the bots see and fire through them, so the counts read above 0 (they count); on, 0.
    fn fairness_in_a_loop(sight: bool, seed: u64) -> ([u32; 2], [u32; 2]) {
        use std::sync::Arc;

        use crate::v026g::bots::smoke::{CloudShape, Placed};
        use crate::v026g::bots::trace::Trace;
        use crate::v026g::bots::tuning::Tuning;
        use crate::v026g::bots::view::Clock;
        use crate::v026g::drive::half_value;
        let teams = [0u8, 0, 1, 1];
        let world = crate::v026g::bots::tests::soak_world();
        let mut tuning = Tuning::default();
        tuning.smoke.sight = sight;
        let mut bots = Bots::new(
            0x5E0E + seed as u32,
            survarium::v026g::maps::mode::TEAM_DEATHMATCH,
            0,
            &teams,
            &[true; 4],
            &[None; 4],
            Arc::new(tuning),
            Trace::none(0),
        );
        bots.map_ready(world.clone());
        let mut game = Game::new(world, &teams, seed, Some(50), None);
        for place in 0..teams.len() {
            bots.entered(place, true, 0);
        }
        let shape = CloudShape {
            forming_ms: 0,
            full_ms: 600_000,
            gone_ms: 600_001,
            ..SmokeGrenade::RDG_M.cloud
        };
        for x in [36.0, 40.0, 44.0] {
            bots.tracker.add_cloud(Cloud {
                by: 0,
                team: 0,
                placed: Placed::Arc,
                centre: Vec3::new(x, 0.0, 32.5),
                thrown: MatchMs(0),
                pours: MatchMs(0),
                shape,
            });
        }
        let mut owner = OwnerTally::new(teams.len());
        let (mut clock, mut next_world) = (0, 1_000);
        while clock < 60_000 {
            clock += 25;
            while game.tick + TICK_MS <= clock {
                game.step();
                owner.tick(&mut game, &bots);
                game.shot_facts.clear();
                game.hit_log.clear();
            }
            if clock >= next_world {
                next_world += 1_000;
                if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300) {
                    for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                        bots.fixed(place, fix, Some(health));
                    }
                    bots.facts(facts);
                }
            }
            let (made, _) = bots.poll(Clock {
                clock,
                time_left_s: (60_000 - clock) as f32 / 1000.0,
                total_s: 60.0,
                playing: true,
            });
            for made in made {
                let r = made.input.rotation;
                let turn = (
                    half_value(u16::from_le_bytes([r[0], r[1]])),
                    half_value(u16::from_le_bytes([r[2], r[3]])),
                );
                let bot = bots.bots[made.place].as_ref();
                let target = bot.and_then(|bot| bot.reflex.target());
                let gun = bot.map_or(crate::v026g::bots::guns::Arms::fallback().primary.0, |bot| bot.reflex.held_gun());
                game.inputs[made.place].push_back((made.time, made.input.actions, turn, target, gun));
            }
        }
        let counts = owner.topic::<SmokeCounts>();
        SEEN_CASES.with(|c| {
            for case in c.borrow_mut().drain(..) {
                println!("seen through (sight {sight}): {case:?}");
            }
        });
        (counts.seen_through, counts.fire_through)
    }

    #[test]
    fn in_a_real_loop_the_fairness_counts_read_0_with_smoke_on_and_count_with_it_off() {
        let (mut off_seen, mut off_fire) = (0, 0);
        for seed in [1, 2] {
            let (seen, fire) = fairness_in_a_loop(false, seed);
            off_seen += seen[0] + seen[1];
            off_fire += fire[0] + fire[1];
            let (seen, fire) = fairness_in_a_loop(true, seed);
            assert_eq!((seen, fire), ([0, 0], [0, 0]), "seed {seed}: smoke on");
        }
        println!("smoke off: seen through {off_seen}, fire through {off_fire}");
        assert!(off_seen > 0 && off_fire > 0, "off: seen {off_seen} fire {off_fire}");
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
