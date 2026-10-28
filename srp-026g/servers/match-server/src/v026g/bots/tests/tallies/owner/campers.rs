use super::*;

/// The campers counts: counted as the match runs, kept for [`OwnerTally::finish`].
#[derive(Default)]
pub(super) struct CamperCounts {
    /// Enemy kills; the kill log read so far (the events take it).
    pub(super) kills: Vec<Kill>,
    pub(super) kills_seen: usize,
    pub(super) last_kill: Option<(u32, Place, Place)>,
    pub(super) spots: Vec<KillSpot>,
    pub(super) entries: Vec<Entry>,
    /// Trade takes: the trader, its team, when.
    pub(super) trades: Vec<(Place, u8, u32)>,
    /// Flank or Crossfire takes on a shooter holding his kill spot: all,
    /// and Flank's.
    pub(super) counter_runs: [u32; 2],
    /// Probe: each counter run's runner, holder, take time, the runner's
    /// height over the holder and the distance across at the take.
    pub(super) counter_detail: Vec<(Place, Place, u32, f32, f32)>,
    /// Shooters killed while holding their kill spot: who, and whether the
    /// killer was [`OFF_LANE_DEG`] or more off every victim's bearing.
    pub(super) held_kills: Vec<(Place, bool)>,
    /// Seconds a shooter's spot was held, summed over the holders (counted once a second), for the lane entries a 10 s.
    pub(super) held_s: u32,
    /// The stand-in camper's place, if the run has one: his holds are scored with [`CAMPER_HOLD_M`].
    pub(super) camper: Option<Place>,
}

/// Campers (`campers.md` Q2, Q6 "How a bench would see it"): a shooter's
/// kills within this of his run's first spot, m, each within this of the
/// last, ms; a held line reaches the kill's distance and this more, and
/// at least this, m; an entry is fatal when the shooter kills the bot
/// within this, ms, and in the killed lane within this of a victim's
/// bearing, degrees; runs and streaks of this many count.
pub(crate) const SPOT_HOLD_M: f32 = 8.0;

/// The stand-in camper's holds are scored within this of the first kill's place, m (an-anticamper §8.4: 16-19 % of a real
/// camper's hold time is past 8 m from his first kill, so 8 m would cut his holds in two).
pub(crate) const CAMPER_HOLD_M: f32 = 10.0;

pub(crate) const RUN_APART_MS: u32 = 120_000;

pub(crate) const REACH_PLUS_M: f32 = 15.0;

pub(crate) const REACH_LEAST_M: f32 = 40.0;

pub(crate) const FATAL_MS: u32 = 15_000;

pub(crate) const LANE_DEG: f32 = 15.0;

pub(crate) const CAMP_RUN: u32 = 3;

/// Trades into the killer (`tally.py`, `campers.md` K3): the partner's
/// death this long before the Trade take, or this long after it, ms; the
/// trader dies to (or kills) that killer within [`FATAL_MS`] of the take.
pub(crate) const TRADE_BEFORE_MS: u32 = 4_000;

pub(crate) const TRADE_AFTER_MS: u32 = 500;

/// Answers to a held spot (`campers.md` K2): a holder killed by a bot this
/// far off every victim's bearing from the spot, degrees.
pub(crate) const OFF_LANE_DEG: f32 = 45.0;

/// A kill of an enemy: when, by whom, of whom, the shooter's feet, the
/// victim's, and the victim's team.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Kill {
    pub(crate) t: u32,
    pub(crate) shooter: Place,
    pub(crate) victim: Place,
    pub(crate) spot: Vec3,
    pub(crate) at: Vec3,
    pub(crate) team: u8,
}

/// A shooter holding his kill spot (`zones.py`): from a kill until he
/// leaves the spot by [`SPOT_HOLD_M`] or [`RUN_APART_MS`] after his last
/// kill there. The victims' team, the line's reach, the victims'
/// bearings, and who of the victims' team is in his line now.
#[derive(Clone, Debug)]
pub(crate) struct KillSpot {
    pub(crate) shooter: Place,
    pub(crate) spot: Vec3,
    pub(crate) team: u8,
    pub(crate) last: u32,
    pub(crate) reach: f32,
    pub(crate) bearings: Vec<f32>,
    pub(crate) in_line: Vec<bool>,
}

/// A bot of the victims' team coming into a held line: when, whose,
/// which bot, in the killed lane.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    pub(crate) t: u32,
    pub(crate) shooter: Place,
    pub(crate) place: Place,
    pub(crate) lane: bool,
}

impl Topic for CamperCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// A Trade take (the trader, its team, when), and a Flank or Crossfire take on a shooter holding his
    /// kill spot against this bot's side (a counter run).
    fn note(&mut self, _run: &Run, note: &Note, kind: &NoteKind<'_>, game: &Game) {
        let NoteKind::Takes { play, target } = *kind else {
            return;
        };
        let team = game.bodies.get(note.place).map(|b| b.team);
        if play == "Trade"
            && let Some(team) = team
        {
            self.trades.push((note.place, team, note.time));
        }
        let on = target
            .strip_prefix("Place(")
            .and_then(|t| t.strip_suffix(')'))
            .and_then(|t| t.parse::<Place>().ok());
        if matches!(play, "Flank" | "Crossfire")
            && let (Some(who), Some(team)) = (on, team)
            && self.spots.iter().any(|h| h.shooter == who && h.team == team)
        {
            self.counter_runs[0] += 1;
            self.counter_runs[1] += u32::from(play == "Flank");
            if let (Some(me), Some(them)) = (game.bodies.get(note.place), game.bodies.get(who)) {
                self.counter_detail.push((
                    note.place,
                    who,
                    note.time,
                    me.feet.y - them.feet.y,
                    me.feet.distance_xz(them.feet),
                ));
            }
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, sight: &mapdata::RayTree, rays: &mut u64) {
        let tick = game.tick;
        self.kills_tick(run, game, sight, rays, tick);
    }

    fn parts(&mut self, run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("campers", self.part(run))]
    }

    /// The entries into held lines and the seconds held, for the outcomes' lane entries a 10 s.
    fn publish(&self) -> Vec<(&'static str, serde_json::Value)> {
        vec![
            ("camper_entries", serde_json::json!(self.entries.len())),
            ("camper_held_s", serde_json::json!(self.held_s)),
        ]
    }
}

impl CamperCounts {
    /// Probe: the counter runs by the runner's height over the holder
    /// (flat under 1.5 m, a floor 1.5-2.5 m, a storey over 2.5 m, above or
    /// below), whether the runner came within 10 m of the holder (same
    /// level) in the next 30 s and whether the holder died in them.
    fn counter_split(&self, run: &Run) -> String {
        let mut split: BTreeMap<String, u32> = BTreeMap::new();
        for &(runner, holder, time, dy, dist) in &self.counter_detail {
            let k0 = (time.saturating_sub(run.first_tick) / TICK_MS) as usize;
            let k1 = k0 + TPS * 30;
            let (Some(rr), Some(rh)) = (run.recs.get(runner), run.recs.get(holder)) else {
                continue;
            };
            let len = rr.len().min(rh.len());
            let reached = (k0..k1.min(len)).any(|k| {
                rr[k].alive && rh[k].alive && rr[k].feet.distance_xz(rh[k].feet) <= 10.0 && (rr[k].feet.y - rh[k].feet.y).abs() <= 1.5
            });
            let killed = (k0..k1.min(len)).any(|k| !rh[k].alive);
            let level = match dy.abs() {
                d if d < 1.5 => "flat",
                d if d < 2.5 => "floor",
                _ if dy > 0.0 => "storey-over",
                _ => "storey-under",
            };
            let range = if dist < 20.0 { "<20m" } else if dist < 40.0 { "20-40m" } else { "40m+" };
            *split
                .entry(format!("{level} {range} reached {} holder-died {}", u8::from(reached), u8::from(killed)))
                .or_default() += 1;
        }
        format!("owner-test counter runs split: {split:?}")
    }

    /// A kill: the shooter's held spot goes on (within [`SPOT_HOLD_M`] of
    /// it and [`RUN_APART_MS`] of his last kill there), or a new one starts
    /// with who of the victims' team is in his line now.
    pub(crate) fn kill_spot(&mut self, game: &Game, kill: &Kill, sight: &mapdata::RayTree, rays: &mut u64) {
        let bearing = yaw_of([kill.at.x - kill.spot.x, kill.at.z - kill.spot.z]);
        let reach = ((kill.at - kill.spot).length() + REACH_PLUS_M).max(REACH_LEAST_M);
        let hold = self.hold_m(kill.shooter);
        if let Some(held) = self.spots.iter_mut().find(|s| s.shooter == kill.shooter)
            && held.spot.distance_xz(kill.spot) <= hold
            && kill.t <= held.last + RUN_APART_MS
            && held.team == kill.team
        {
            held.last = kill.t;
            held.reach = held.reach.max(reach);
            held.bearings.push(bearing);
            return;
        }
        self.spots.retain(|s| s.shooter != kill.shooter);
        let me = &game.bodies[kill.shooter];
        let in_line = game
            .bodies
            .iter()
            .map(|b| b.alive && b.team == kill.team && in_sight(sight, me, b, reach, rays))
            .collect();
        self.spots.push(KillSpot {
            shooter: kill.shooter,
            spot: kill.spot,
            team: kill.team,
            last: kill.t,
            reach,
            bearings: vec![bearing],
            in_line,
        });
    }

    /// Once a second: the held spots still held, and the victims' team's
    /// bots coming into their lines.
    pub(crate) fn held_lines(&mut self, game: &Game, sight: &mapdata::RayTree, rays: &mut u64) {
        let tick = game.tick;
        let mut spots = std::mem::take(&mut self.spots);
        let camper = self.camper;
        let hold_of = move |shooter: Place| if camper == Some(shooter) { CAMPER_HOLD_M } else { SPOT_HOLD_M };
        spots.retain_mut(|held| {
            let me = &game.bodies[held.shooter];
            if !me.alive || me.feet.distance_xz(held.spot) > hold_of(held.shooter) || tick > held.last + RUN_APART_MS {
                return false;
            }
            for (place, b) in game.bodies.iter().enumerate() {
                if b.team != held.team {
                    continue;
                }
                let seen = b.alive && in_sight(sight, me, b, held.reach, rays);
                if seen && !held.in_line[place] {
                    let bearing = yaw_of([b.feet.x - held.spot.x, b.feet.z - held.spot.z]);
                    let lane = held
                        .bearings
                        .iter()
                        .any(|&w| wrap(w - bearing).abs().to_degrees() <= LANE_DEG);
                    self.entries.push(Entry {
                        t: tick,
                        shooter: held.shooter,
                        place,
                        lane,
                    });
                }
                held.in_line[place] = seen;
            }
            true
        });
        self.held_s += spots.len() as u32;
        self.spots = spots;
    }
}

#[test]
pub(crate) fn kills_from_one_spot_and_a_bot_walking_into_the_held_line_are_counted() {
    // Shooter 1 holds (30, 30) and kills bots 0, 2 and 3 there 2 s apart;
    // bot 4 comes out from behind the wall (x = 40, z < 25) into his line
    // in the killed lane and dies to him. Bot 3 (the near miss) kills bot
    // 5 twice: a run of two.
    let (mut game, mut tally) = made_up(&[0, 1, 0, 0, 0, 1]);
    play(&mut game, &mut tally, 200, |k, game| {
        put(game, 1, 30.0, 30.0, 0);
        put(game, 0, 20.0, 35.0, 0);
        put(game, 2, 21.0, 35.0, 0);
        put(game, 3, 22.0, 35.0, 0);
        put(game, 5, 60.0, 35.0, 0);
        if k >= 120 {
            put(game, 4, 20.0, 36.0, 0);
        } else {
            put(game, 4, 50.0, 10.0, 0);
        }
        for (at, shooter, victim) in [(20, 1, 0), (30, 3, 5), (60, 1, 2), (70, 3, 5), (100, 1, 3), (160, 1, 4)] {
            if k == at {
                game.kill_log.push((game.tick, shooter, victim));
            }
        }
        for (at, victim) in [(20, 0), (60, 2), (100, 3), (160, 4)] {
            if k >= at {
                game.bodies[victim].alive = false;
            }
        }
        vec![bot_now(); 6]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["camper_runs_3"], 1, "{json}");
    assert_eq!(json["camper_run_kills"], 4);
    assert_eq!(json["camper_run_longest"], 4);
    assert_eq!(json["deaths_in_row_3"], 1);
    assert_eq!(json["deaths_in_row_longest"], 4);
    assert_eq!(json["reentries"], 1);
    assert_eq!(json["reentries_fatal"], 1);
    assert_eq!(json["reentries_lane"], 1);
}

/// `ticks` ticks of `game` with every body at its spot (dead after its
/// death in `kills`, which go in the kill log at their tick), and the
/// notes given at theirs.
pub(crate) fn kills_and_notes(
    game: &mut Game,
    tally: &mut OwnerTally,
    ticks: usize,
    spots: &[(f32, f32)],
    kills: &[(usize, Place, Place)],
    notes: &[(usize, Place, &str)],
) {
    for k in 0..ticks {
        game.tick = (k as u32 + 1) * TICK_MS;
        for (place, &(x, z)) in spots.iter().enumerate() {
            put(game, place, x, z, 0);
        }
        for &(at, shooter, victim) in kills {
            if k == at {
                game.kill_log.push((game.tick, shooter, victim));
            }
            if k >= at {
                game.bodies[victim].alive = false;
            }
        }
        let nows = vec![bot_now(); spots.len()];
        tally.step_with(game, &nows);
        game.hit_log.clear();
        for &(at, place, text) in notes {
            if k == at {
                let note = Note {
                    place,
                    time: game.tick,
                    text: text.to_owned(),
                };
                tally.note(&note, game);
            }
        }
    }
}

#[test]
pub(crate) fn a_trade_into_the_partners_killer_and_a_trade_that_kills_him_are_counted() {
    // Bot 2 kills bot 0; bot 1 trades and dies to bot 2 in 15 s. Bot 5
    // kills bot 3; bot 4 trades and kills bot 5. Near miss: bot 4 trades
    // again with no partner's death in the 4 s before.
    let (mut game, mut tally) = made_up(&[0, 0, 1, 1, 1, 0]);
    let spots = [(10.0, 10.0), (11.0, 10.0), (20.0, 10.0), (40.0, 10.0), (41.0, 10.0), (30.0, 10.0)];
    let kills = [(20, 2, 0), (30, 5, 3), (80, 4, 5), (100, 2, 1)];
    let notes = [
        (39, 1, "takes Trade Place(0) (score 1.00: x)"),
        (39, 4, "takes Trade Place(3) (score 1.00: x)"),
        (179, 4, "takes Trade Place(3) (score 1.00: x)"),
    ];
    kills_and_notes(&mut game, &mut tally, 200, &spots, &kills, &notes);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["trades"], 3, "{json}");
    assert_eq!(json["trades_died_to_killer"], 1);
    assert_eq!(json["trade_kills"], 1);
}

#[test]
pub(crate) fn a_counter_run_on_a_holder_and_a_holder_killed_off_his_lane_are_counted() {
    // Shooter 1 holds (30, 30), killing bots 0 and 2 to the west; bot 3
    // takes Flank on him and kills him from the north. Shooter 5 holds
    // (60, 60), killing bot 4 to the west; bot 6 takes Crossfire on him
    // and kills him from that lane (the near miss). Near misses: Flank on
    // shooter 1 before he held, Crossfire on an exit.
    let (mut game, mut tally) = made_up(&[0, 1, 0, 0, 0, 1, 0]);
    tally.run.camper = Some(1);
    let spots = [(20.0, 35.0), (30.0, 30.0), (21.0, 35.0), (30.0, 45.0), (50.0, 60.0), (60.0, 60.0), (49.0, 61.0)];
    let kills = [(20, 1, 0), (40, 1, 2), (50, 5, 4), (80, 3, 1), (90, 6, 5)];
    let notes = [
        (10, 3, "takes Flank Place(1) (score 1.00: x)"),
        (60, 3, "takes Flank Place(1) (score 1.00: x)"),
        (60, 6, "takes Crossfire Place(5) (score 1.00: x)"),
        (60, 6, "takes Crossfire Exit(0) (score 1.00: x)"),
    ];
    kills_and_notes(&mut game, &mut tally, 120, &spots, &kills, &notes);
    let (_, json) = tally.finish(&game);
    assert_eq!(json["counter_runs"], 2, "{json}");
    assert_eq!(json["counter_runs_flank"], 1);
    assert_eq!(json["camper_kills"], 2);
    assert_eq!(json["camper_kills_off_lane"], 1);
    let camper = &json["stand_in_camper"];
    assert_eq!(camper["kills"], 2);
    assert_eq!(camper["deaths"], 1);
    assert_eq!(camper["killed_holding"], 1);
    assert_eq!(camper["killed_off_lane"], 1);
}

impl CamperCounts {
    /// How far from his first kill a shooter's hold goes on, m.
    fn hold_m(&self, shooter: Place) -> f32 {
        if self.camper == Some(shooter) { CAMPER_HOLD_M } else { SPOT_HOLD_M }
    }

    /// The kills this tick, the shooters' held spots, and (once a second) the held lines.
    fn kills_tick(&mut self, run: &Run, game: &Game, sight: &mapdata::RayTree, rays: &mut u64, tick: u32) {
        // Kills of enemies, and the lines they make held.
        if self.kills_seen > 0 && game.kill_log.get(self.kills_seen - 1) != self.last_kill.as_ref() {
            self.kills_seen = 0;
        }
        let new_kills = game.kill_log[self.kills_seen.min(game.kill_log.len())..].to_vec();
        self.kills_seen = game.kill_log.len();
        self.last_kill = game.kill_log.last().copied();
        for (t, shooter, victim) in new_kills {
            let (Some(s), Some(v)) = (game.bodies.get(shooter), game.bodies.get(victim)) else {
                continue;
            };
            if s.team == v.team {
                continue;
            }
            let kill = Kill {
                t,
                shooter,
                victim,
                spot: s.feet,
                at: v.feet,
                team: v.team,
            };
            // A shooter killed holding his spot by one of his victims'
            // side: from off his killed lanes or not.
            if let Some(held) = self.spots.iter().find(|h| {
                h.shooter == victim
                    && h.team == s.team
                    && v.feet.distance_xz(h.spot) <= self.hold_m(victim)
                    && t <= h.last + RUN_APART_MS
            }) {
                let bearing = yaw_of([s.feet.x - held.spot.x, s.feet.z - held.spot.z]);
                let off = held
                    .bearings
                    .iter()
                    .all(|&w| wrap(w - bearing).abs().to_degrees() >= OFF_LANE_DEG);
                self.held_kills.push((victim, off));
            }
            self.kills.push(kill);
            self.kill_spot(game, &kill, sight, rays);
        }
        if (((tick - run.first_tick) / TICK_MS) as usize).is_multiple_of(TPS) {
            self.held_lines(game, sight, rays);
        }
    }

    /// The campers counts at the end: their keys of the `owner` object and their lines of the owner-test text.
    fn part(&self, run: &Run) -> Part {
        // Campers: runs of kills from one spot, a team's deaths in a row to
        // one shooter from one spot, entries into held lines.
        // `only`: the runs of that shooter alone are counted.
        let in_runs = |groups: &BTreeMap<Place, Vec<Kill>>, only: Option<Place>| {
            let (mut runs, mut kills, mut longest) = (0u32, 0u32, 0u32);
            for list in groups.values() {
                let mut run: Option<(Place, Vec3, u32, u32)> = None;
                let mut close = |run: Option<(Place, Vec3, u32, u32)>| {
                    if let Some((shooter, .., len)) = run
                        && only.is_none_or(|o| o == shooter)
                    {
                        longest = longest.max(len);
                        if len >= CAMP_RUN {
                            runs += 1;
                            kills += len;
                        }
                    }
                };
                for k in list {
                    run = match run {
                        Some((shooter, spot, last, len))
                            if shooter == k.shooter
                                && k.spot.distance_xz(spot) <= self.hold_m(k.shooter)
                                && k.t <= last + RUN_APART_MS =>
                        {
                            Some((shooter, spot, k.t, len + 1))
                        }
                        other => {
                            close(other);
                            Some((k.shooter, k.spot, k.t, 1))
                        }
                    };
                }
                close(run);
            }
            (runs, kills, longest)
        };
        let mut by_shooter: BTreeMap<Place, Vec<Kill>> = BTreeMap::new();
        let mut by_team: BTreeMap<Place, Vec<Kill>> = BTreeMap::new();
        for k in &self.kills {
            by_shooter.entry(k.shooter).or_default().push(*k);
            by_team.entry(usize::from(k.team)).or_default().push(*k);
        }
        let (camp_runs, camp_run_kills, camp_longest) = in_runs(&by_shooter, None);
        let (in_row, _, in_row_longest) = in_runs(&by_team, None);
        let fatal = |e: &Entry| {
            self.kills
                .iter()
                .any(|k| k.shooter == e.shooter && k.victim == e.place && k.t >= e.t && k.t <= e.t + FATAL_MS)
        };
        let entries = self.entries.len() as u32;
        let entries_fatal = self.entries.iter().filter(|e| fatal(e)).count() as u32;
        let entries_lane = self.entries.iter().filter(|e| e.lane).count() as u32;
        let entries_lane_fatal = self.entries.iter().filter(|e| e.lane && fatal(e)).count() as u32;
        // Trades into the killer: of each Trade take, the partner's killer
        // (the last kill of a teammate from 4 s before to 0.5 s after);
        // the trader killed by him, or killing him, within 15 s.
        let mut trades = [self.trades.len() as u32, 0, 0];
        for &(place, team, t) in &self.trades {
            let killer = self
                .kills
                .iter()
                .rfind(|k| {
                    k.team == team && k.victim != place && k.t + TRADE_BEFORE_MS >= t && k.t <= t + TRADE_AFTER_MS
                })
                .map(|k| k.shooter);
            let Some(killer) = killer else {
                continue;
            };
            let within = |k: &Kill| k.t >= t && k.t <= t + FATAL_MS;
            trades[1] += u32::from(self.kills.iter().any(|k| within(k) && k.victim == place && k.shooter == killer));
            trades[2] += u32::from(self.kills.iter().any(|k| within(k) && k.shooter == place && k.victim == killer));
        }
        let held_killed = |who: Option<Place>| {
            let mine = self.held_kills.iter().filter(|(p, _)| who.is_none_or(|w| w == *p));
            [mine.clone().count() as u32, mine.filter(|(_, off)| *off).count() as u32]
        };
        let held_all = held_killed(None);
        // The stand-in camper's part of the counts (the rest is the bots').
        let camper = run.camper.map(|c| {
            let (runs, run_kills, longest) = in_runs(&by_shooter, Some(c));
            let (row, _, row_longest) = in_runs(&by_team, Some(c));
            let into: Vec<&Entry> = self.entries.iter().filter(|e| e.shooter == c).collect();
            let killed = held_killed(Some(c));
            serde_json::json!({
                "place": c,
                "kills": self.kills.iter().filter(|k| k.shooter == c).count(),
                "deaths": self.kills.iter().filter(|k| k.victim == c).count(),
                "runs_3": runs,
                "run_kills": run_kills,
                "run_longest": longest,
                "deaths_in_row_3": row,
                "deaths_in_row_longest": row_longest,
                "reentries": into.len(),
                "reentries_fatal": into.iter().filter(|e| fatal(e)).count(),
                "reentries_lane": into.iter().filter(|e| e.lane).count(),
                "killed_holding": killed[0],
                "killed_off_lane": killed[1],
            })
        });

        let json = serde_json::json!({
            "camper_runs_3": camp_runs,
            "camper_run_kills": camp_run_kills,
            "camper_run_longest": camp_longest,
            "deaths_in_row_3": in_row,
            "deaths_in_row_longest": in_row_longest,
            "reentries": entries,
            "reentries_fatal": entries_fatal,
            // A count rises with the volume of entries and kills; the share says whether walking into a held line got deadlier
            // (an-anticamper §5: 13.0 % -> 14.5 % where the count read +30 %).
            "reentries_fatal_share": Scale::rate(entries_fatal, entries),
            "reentries_lane": entries_lane,
            "reentries_lane_fatal": entries_lane_fatal,
            "trades": trades[0],
            "trades_died_to_killer": trades[1],
            "trade_kills": trades[2],
            "counter_runs": self.counter_runs[0],
            "counter_runs_flank": self.counter_runs[1],
            "camper_kills": held_all[0],
            "camper_kills_off_lane": held_all[1],
            "stand_in_camper": camper,
        });
        let counter_split = self.counter_split(run);
        let text = format!(
            "{counter_split}
  owner-test campers: runs of 3+ from one spot {} ({} kills, longest {}); a team's deaths in a row to one shooter's spot, 3+ {} (longest {}); entries into a held line {} fatal {}, in the killed lane {} fatal {}; Trade takes {} died to the partner's killer {} killed him {}; counter runs on a holder {} (Flank {}); holders killed {} from off their lanes {}; the stand-in camper {}",
            camp_runs,
            camp_run_kills,
            camp_longest,
            in_row,
            in_row_longest,
            entries,
            entries_fatal,
            entries_lane,
            entries_lane_fatal,
            trades[0],
            trades[1],
            trades[2],
            self.counter_runs[0],
            self.counter_runs[1],
            held_all[0],
            held_all[1],
            json["stand_in_camper"],
        );
        Part::new(json, text)
    }
}
