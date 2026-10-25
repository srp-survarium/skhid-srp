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
    /// Shooters killed while holding their kill spot: who, and whether the
    /// killer was [`OFF_LANE_DEG`] or more off every victim's bearing.
    pub(super) held_kills: Vec<(Place, bool)>,
}

/// Campers (`campers.md` Q2, Q6 "How a bench would see it"): a shooter's
/// kills within this of his run's first spot, m, each within this of the
/// last, ms; a held line reaches the kill's distance and this more, and
/// at least this, m; an entry is fatal when the shooter kills the bot
/// within this, ms, and in the killed lane within this of a victim's
/// bearing, degrees; runs and streaks of this many count.
pub(crate) const SPOT_HOLD_M: f32 = 8.0;

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

impl OwnerTally {
    /// A kill: the shooter's held spot goes on (within [`SPOT_HOLD_M`] of
    /// it and [`RUN_APART_MS`] of his last kill there), or a new one starts
    /// with who of the victims' team is in his line now.
    pub(crate) fn kill_spot(&mut self, game: &Game, kill: &Kill, sight: &mapdata::RayTree, rays: &mut u64) {
        let bearing = yaw_of([kill.at.x - kill.spot.x, kill.at.z - kill.spot.z]);
        let reach = ((kill.at - kill.spot).length() + REACH_PLUS_M).max(REACH_LEAST_M);
        if let Some(held) = self.campers.spots.iter_mut().find(|s| s.shooter == kill.shooter)
            && held.spot.distance_xz(kill.spot) <= SPOT_HOLD_M
            && kill.t <= held.last + RUN_APART_MS
            && held.team == kill.team
        {
            held.last = kill.t;
            held.reach = held.reach.max(reach);
            held.bearings.push(bearing);
            return;
        }
        self.campers.spots.retain(|s| s.shooter != kill.shooter);
        let me = &game.bodies[kill.shooter];
        let in_line = game
            .bodies
            .iter()
            .map(|b| b.alive && b.team == kill.team && in_sight(sight, me, b, reach, rays))
            .collect();
        self.campers.spots.push(KillSpot {
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
        let mut spots = std::mem::take(&mut self.campers.spots);
        spots.retain_mut(|held| {
            let me = &game.bodies[held.shooter];
            if !me.alive || me.feet.distance_xz(held.spot) > SPOT_HOLD_M || tick > held.last + RUN_APART_MS {
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
                    self.campers.entries.push(Entry {
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
        self.campers.spots = spots;
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
        tally.step(game, &nows);
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
    tally.camper = Some(1);
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
