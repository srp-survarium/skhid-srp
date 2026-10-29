//! Client-test steps of the threat package (P1 the threat answer #703, P2 the objectives' danger; `an-anticamper.md` section 6,
//! `an-objective.md` 6.4, `p1-client-tests.md` CT-P1-1 to CT-P1-4, CT-P2-1, `fx-zones-client-tests.md` Z5).
//!
//! The owner holds a spot and kills what comes; the steps read what team 1 does from the bots' notes (`job on threat spot`,
//! `takes Flank ...`, `threat spot N ...: cleared, seen empty`) and from the bodies (who is where, who fell). A bearing off his
//! victims by more than 60 degrees is a flank; his lanes are the 20-degree wedges round the bearings of his victims, out to 40 m
//! (the measure of `an-anticamper.md` 8.1). The runner places no bot, so the bots are the match's own (no scripting): the numbers
//! say how many came.
//!
//! CT-P1-5 (a camper outside a spawn exit on Tarakanovsky Fort) starts when the owner stands within 5 m of one of team 1's exits, which the frame
//! carries (`Frame::exits`, from `Tactics::exits`): the owner finds the way out of the enemy spawn and holds it.

use super::*;
use crate::v026g::bots::view::{BatteryState, Place};
use mapdata::Vec3;

const SCHOOL: u8 = 2;
const TARAKANOVSKY_FORT: u8 = 5;
const COLOGNE: u8 = 8;

/// School's way from the raiders' spawn to the container of the owner's team (`an-anticamper.md` 6.1).
const HELD_SPOT: Spot = Spot::at(47.0, 17.0, 116.0);

/// A victim of his is counted when he fell within this of him, m.
const VICTIM_M: f32 = 45.0;
/// A bot within this of him, off every lane by more than this angle, is a flank arrival.
const FLANK_M: f32 = 25.0;
const FLANK_DEG: f32 = 60.0;
/// His lanes: half the wedge (10 degrees each side) and how far out, m.
const LANE_HALF_DEG: f32 = 10.0;
const LANE_M: f32 = 40.0;
/// A wrong clear: the spot is cleared while he is still this near it, m (`an-anticamper.md` 6.3).
const SPOT_AREA_M: f32 = 10.0;
/// Quick second kills: a kill within this of the one before, ms.
const QUICK_MS: u32 = 3_000;
/// CT-P1-5: the owner holds within this of one of team 1's exits, and a death counts at the exit within this of it, m.
const EXIT_HOLD_M: f32 = 5.0;
const EXIT_DEATH_M: f32 = 30.0;

fn flat_bearing_deg(from: Vec3, to: Vec3) -> f32 {
    (to.z - from.z).atan2(to.x - from.x).to_degrees()
}

/// The angle between two bearings, degrees, 0 to 180.
fn angle_between_deg(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    if d > 180.0 { 360.0 - d } else { d }
}

fn near_held_spot(frame: &Frame, _since: Span) -> bool {
    frame.owner.as_ref().is_some_and(|o| o.body.alive && o.body.feet.distance_xz(HELD_SPOT.vec()) <= 8.0)
}

fn crouched_at_held_spot(frame: &Frame, since: Span) -> bool {
    near_held_spot(frame, since) && frame.owner.as_ref().is_some_and(|o| o.body.crouched && o.body.speed < 0.2)
}

fn two_deaths(_frame: &Frame, progress: &Progress) -> bool {
    progress.owner_deaths >= 2
}

fn two_kills(frame: &Frame, _since: Span) -> bool {
    frame.owner.as_ref().is_some_and(|o| o.body.alive && o.body.kills >= 2)
}

/// The enemy bots that fell between two frames, with where each stood at the frame before.
fn fallen(before: &[(Place, bool, Vec3)], frame: &Frame) -> Vec<Vec3> {
    frame
        .side(Side::Enemies)
        .filter(|b| !b.body.alive)
        .filter_map(|b| before.iter().find(|(p, alive, _)| *p == b.body.place && *alive).map(|(_, _, at)| *at))
        .collect()
}

fn enemy_positions(frame: &Frame) -> Vec<(Place, bool, Vec3)> {
    frame.side(Side::Enemies).map(|b| (b.body.place, b.body.alive, b.body.feet)).collect()
}

/// The shared part of the hold meters: his victims' bearings, the time of his 2nd kill since the try began, the moments of his kills.
#[derive(Debug, Default)]
struct Hold {
    before: Vec<(Place, bool, Vec3)>,
    /// Bearings from him to the bots that fell near him, degrees.
    victims: Vec<f32>,
    kills_at_start: Option<u32>,
    kill_times: Vec<At>,
    second_kill: Option<At>,
}

impl Hold {
    /// Reads a frame: new victims and new kills. Returns false when the owner is not in it.
    fn read(&mut self, frame: &Frame) -> bool {
        let Some(owner) = frame.owner.as_ref() else {
            return false;
        };
        let base = *self.kills_at_start.get_or_insert(owner.body.kills);
        while (self.kill_times.len() as u32) < owner.body.kills.saturating_sub(base) {
            self.kill_times.push(frame.clock);
        }
        if self.second_kill.is_none() && self.kill_times.len() >= 2 {
            self.second_kill = Some(self.kill_times[1]);
        }
        for at in fallen(&self.before, frame) {
            if at.distance_xz(owner.body.feet) <= VICTIM_M {
                self.victims.push(flat_bearing_deg(owner.body.feet, at));
            }
        }
        true
    }

    fn finish(&mut self, frame: &Frame) {
        self.before = enemy_positions(frame);
    }

    fn off_lanes(&self, bearing: f32, half_deg: f32) -> bool {
        !self.victims.is_empty() && self.victims.iter().all(|v| angle_between_deg(*v, bearing) > half_deg)
    }

    fn in_lane(&self, bearing: f32) -> bool {
        self.victims.iter().any(|v| angle_between_deg(*v, bearing) <= LANE_HALF_DEG)
    }

    /// Seconds from his 2nd kill to `at`, none before it or without one.
    fn since_second_kill(&self, at: At) -> Option<f64> {
        let second = self.second_kill?;
        (at.0 >= second.0).then(|| f64::from(at.0 - second.0) / 1_000.0)
    }
}

/// CT-P1-1 and CT-P1-2: does the held spot draw a flank? Names: `job_s` (from his 2nd kill to the first note `job on threat spot`),
/// `flank_take_s` (to the first `takes Flank ThreatSpot`), `flank_arrivals` (bots that came within 25 m of him more than 60 degrees
/// off every bearing of his victims, each counted once), `victims`.
#[derive(Debug, Default)]
struct FlankMeter {
    hold: Hold,
    job: Option<f64>,
    take: Option<f64>,
    arrived: Vec<Place>,
}

impl FlankMeter {
    fn make(_start: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for FlankMeter {
    fn sample(&mut self, frame: &Frame) {
        if !self.hold.read(frame) {
            return;
        }
        let owner = frame.owner.as_ref().map(|o| o.body.feet).unwrap_or_default();
        for heard in &frame.heard {
            if self.job.is_none() && heard.text.contains("job on threat spot") && heard.text.contains("Flanker") {
                self.job = self.hold.since_second_kill(heard.time);
            }
            if self.take.is_none() && heard.text.contains("takes Flank ThreatSpot") {
                self.take = self.hold.since_second_kill(heard.time);
            }
        }
        for bot in frame.side(Side::Enemies).filter(|b| b.body.alive) {
            if bot.body.feet.distance_xz(owner) <= FLANK_M
                && self.hold.off_lanes(flat_bearing_deg(owner, bot.body.feet), FLANK_DEG)
                && !self.arrived.contains(&bot.body.place)
            {
                self.arrived.push(bot.body.place);
            }
        }
        self.hold.finish(frame);
    }

    fn value(&self, n: usize) -> Option<f64> {
        self.hold.kills_at_start?;
        match n {
            0 => self.job,
            1 => self.take,
            2 => Some(self.arrived.len() as f64),
            3 => Some(self.hold.victims.len() as f64),
            _ => None,
        }
    }
}

fn flank_measure() -> Measure {
    Measure::Custom { names: &["job_s", "flank_take_s", "flank_arrivals", "victims"], make: FlankMeter::make }
}

/// CT-P1-3: do team 1's paths leave his lanes? Names: `lane_entries_per_10s` (bots coming into one of his lanes, 40 m out, after his 2nd
/// kill, per 10 s; the owner today 0.84-1.42, target 0.35-0.60), `quick_kill_share` (of his kills after the first, the ones within 3 s of
/// the one before; today 18 %, target near 4 %), `kills`, `recasts` (notes `cast again (a kill off its lanes)`) and `centre_moved`
/// (notes `its centre moved`).
#[derive(Debug, Default)]
struct LaneMeter {
    hold: Hold,
    in_lane: Vec<Place>,
    entries: u32,
    last: Option<At>,
    recasts: u32,
    moved: u32,
}

impl LaneMeter {
    fn make(_start: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for LaneMeter {
    fn sample(&mut self, frame: &Frame) {
        if !self.hold.read(frame) {
            return;
        }
        let owner = frame.owner.as_ref().map(|o| o.body.feet).unwrap_or_default();
        self.recasts += frame.heard.iter().filter(|h| h.text.contains("cast again (a kill off its lanes)")).count() as u32;
        self.moved += frame.heard.iter().filter(|h| h.text.contains("its centre moved")).count() as u32;
        let mut now = Vec::new();
        for bot in frame.side(Side::Enemies).filter(|b| b.body.alive) {
            let inside = bot.body.feet.distance_xz(owner) <= LANE_M && self.hold.in_lane(flat_bearing_deg(owner, bot.body.feet));
            if inside {
                now.push(bot.body.place);
                if self.hold.second_kill.is_some() && !self.in_lane.contains(&bot.body.place) {
                    self.entries += 1;
                }
            }
        }
        self.in_lane = now;
        self.last = Some(frame.clock);
        self.hold.finish(frame);
    }

    fn value(&self, n: usize) -> Option<f64> {
        self.hold.kills_at_start?;
        match n {
            0 => {
                let second = self.hold.second_kill?;
                let seconds = f64::from(self.last?.0.saturating_sub(second.0)) / 1_000.0;
                (seconds >= 10.0).then(|| f64::from(self.entries) / (seconds / 10.0))
            }
            1 => {
                let times = &self.hold.kill_times;
                (times.len() >= 2).then(|| {
                    let quick = times.windows(2).filter(|w| w[1].0.saturating_sub(w[0].0) <= QUICK_MS).count();
                    quick as f64 / (times.len() - 1) as f64
                })
            }
            2 => Some(self.hold.kill_times.len() as f64),
            3 => Some(f64::from(self.recasts)),
            4 => Some(f64::from(self.moved)),
            _ => None,
        }
    }
}

fn lane_measure() -> Measure {
    Measure::Custom {
        names: &["lane_entries_per_10s", "quick_kill_share", "kills", "recasts", "centre_moved"],
        make: LaneMeter::make,
    }
}

/// The spot of a note "threat spot N of shooter S at (x, y, z): ...", flat.
fn spot_of(text: &str) -> Option<(f32, f32)> {
    let inside = text.split("at (").nth(1)?.split(')').next()?;
    let mut parts = inside.split(',').map(|p| p.trim().parse::<f32>());
    let x = parts.next()?.ok()?;
    let _y = parts.next()?.ok()?;
    let z = parts.next()?.ok()?;
    Some((x, z))
}

/// CT-P1-4: is a threat spot cleared while he still sits in it? Names: `clears` (notes `cleared, seen empty` in the try),
/// `wrong_clears` (of them, with him within 10 m of the spot then), `first_clear_s` (from the start, s).
#[derive(Debug, Default)]
struct ClearMeter {
    first: Option<At>,
    clears: u32,
    wrong: u32,
    first_clear: Option<At>,
}

impl ClearMeter {
    fn make(_start: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for ClearMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        self.first.get_or_insert(frame.clock);
        for heard in frame.heard.iter().filter(|h| h.text.contains("cleared, seen empty")) {
            self.clears += 1;
            self.first_clear.get_or_insert(heard.time);
            if spot_of(&heard.text).is_some_and(|(x, z)| (owner.body.feet.x - x).hypot(owner.body.feet.z - z) <= SPOT_AREA_M) {
                self.wrong += 1;
            }
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        let first = self.first?;
        match n {
            0 => Some(f64::from(self.clears)),
            1 => Some(f64::from(self.wrong)),
            2 => Some(f64::from(self.first_clear?.0.saturating_sub(first.0)) / 1_000.0),
            _ => None,
        }
    }
}

fn clear_measure() -> Measure {
    Measure::Custom { names: &["clears", "wrong_clears", "first_clear_s"], make: ClearMeter::make }
}

/// A battery lies free within 25 m of the owner and he is at least 12 m from it (he holds his distance from the bait).
fn battery_to_hold(frame: &Frame, _since: Span) -> bool {
    let Some(owner) = frame.owner.as_ref().filter(|o| o.body.alive) else {
        return false;
    };
    frame.batteries.iter().any(|b| matches!(b, BatteryState::Free(at) if (12.0..=25.0).contains(&owner.body.feet.distance_xz(*at))))
}

/// CT-P2-1: bots at a lying battery in his line. Names: `bots_at_battery` (enemy bots that came within 8 m of it), `deaths_by_battery`
/// (enemy bots that fell within 25 m of it), `first_arrival_s` (from the start, s).
#[derive(Debug, Default)]
struct BaitMeter {
    battery: Option<Vec3>,
    first: Option<At>,
    before: Vec<(Place, bool, Vec3)>,
    arrived: Vec<Place>,
    deaths: u32,
    first_arrival: Option<At>,
}

impl BaitMeter {
    fn make(_start: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for BaitMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        self.first.get_or_insert(frame.clock);
        if self.battery.is_none() {
            self.battery = frame.batteries.iter().find_map(|b| match b {
                BatteryState::Free(at) if owner.body.feet.distance_xz(*at) <= 25.0 => Some(*at),
                _ => None,
            });
        }
        let Some(battery) = self.battery else {
            return;
        };
        self.deaths += fallen(&self.before, frame).into_iter().filter(|at| at.distance_xz(battery) <= 25.0).count() as u32;
        for bot in frame.side(Side::Enemies).filter(|b| b.body.alive && b.body.feet.distance_xz(battery) <= 8.0) {
            if !self.arrived.contains(&bot.body.place) {
                self.arrived.push(bot.body.place);
                self.first_arrival.get_or_insert(frame.clock);
            }
        }
        self.before = enemy_positions(frame);
    }

    fn value(&self, n: usize) -> Option<f64> {
        let first = self.first?;
        self.battery?;
        match n {
            0 => Some(self.arrived.len() as f64),
            1 => Some(f64::from(self.deaths)),
            2 => Some(f64::from(self.first_arrival?.0.saturating_sub(first.0)) / 1_000.0),
            _ => None,
        }
    }
}

fn bait_measure() -> Measure {
    Measure::Custom { names: &["bots_at_battery", "deaths_by_battery", "first_arrival_s"], make: BaitMeter::make }
}

/// The owner is alive within 5 m of one of team 1's exits.
fn near_enemy_exit(frame: &Frame, _since: Span) -> bool {
    frame.owner.as_ref().is_some_and(|o| o.body.alive && frame.exits[1].iter().any(|e| e.distance_xz(o.body.feet) <= EXIT_HOLD_M))
}

/// CT-P1-5: team 1's bots dying at the exit he holds. Names: `exit_deaths` (enemy bots that fell within 30 m of an exit of theirs while
/// he held it), `pair_deaths` (of them, the ones within 3 s of the one before: bots coming out one after another),
/// `deaths_per_10s_after_2nd` (the deaths after his 2nd kill per 10 s of the try after it; the bar is at most one), `flank_notes`
/// (notes `takes Flank ThreatSpot` in the try).
#[derive(Debug, Default)]
struct ExitMeter {
    hold: Hold,
    deaths: Vec<At>,
    after_second: u32,
    last: Option<At>,
    flank_notes: u32,
}

impl ExitMeter {
    fn make(_start: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for ExitMeter {
    fn sample(&mut self, frame: &Frame) {
        if !self.hold.read(frame) {
            return;
        }
        for at in fallen(&self.hold.before, frame) {
            if frame.exits[1].iter().any(|e| e.distance_xz(at) <= EXIT_DEATH_M) {
                self.deaths.push(frame.clock);
                if self.hold.second_kill.is_some_and(|second| frame.clock.0 > second.0) {
                    self.after_second += 1;
                }
            }
        }
        self.flank_notes += frame.heard.iter().filter(|h| h.text.contains("takes Flank ThreatSpot")).count() as u32;
        self.last = Some(frame.clock);
        self.hold.finish(frame);
    }

    fn value(&self, n: usize) -> Option<f64> {
        self.hold.kills_at_start?;
        match n {
            0 => Some(self.deaths.len() as f64),
            1 => Some(self.deaths.windows(2).filter(|w| w[1].0.saturating_sub(w[0].0) <= QUICK_MS).count() as f64),
            2 => {
                let second = self.hold.second_kill?;
                let seconds = f64::from(self.last?.0.saturating_sub(second.0)) / 1_000.0;
                (seconds > 0.0).then(|| f64::from(self.after_second) / (seconds / 10.0).max(1.0))
            }
            3 => Some(f64::from(self.flank_notes)),
            _ => None,
        }
    }
}

fn exit_measure() -> Measure {
    Measure::Custom { names: &["exit_deaths", "pair_deaths", "deaths_per_10s_after_2nd", "flank_notes"], make: ExitMeter::make }
}

fn picked_up(frame: &Frame, _progress: &Progress) -> bool {
    frame.batteries.iter().any(|b| matches!(b, BatteryState::Carried(Some(_))))
}

/// Z5: does the team go for a contested battery together? Names: `mate_takes` (notes of owner-team bots `takes Collect`, `takes Support`
/// or `takes Raid`), `first_take_s` (from the start), `picked_up_s` (the first frame a battery is carried; none: never), `scored_out`
/// (notes `scored out: enemies by it`).
#[derive(Debug, Default)]
struct ContestMeter {
    first: Option<At>,
    takes: u32,
    first_take: Option<At>,
    picked: Option<At>,
    scored_out: u32,
}

impl ContestMeter {
    fn make(_start: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for ContestMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        self.first.get_or_insert(frame.clock);
        for heard in &frame.heard {
            let mate = frame.bot(heard.place).is_some_and(|b| b.body.team == owner.body.team);
            if mate && ["takes Collect", "takes Support", "takes Raid"].iter().any(|t| heard.text.contains(t)) {
                self.takes += 1;
                self.first_take.get_or_insert(heard.time);
            }
            if heard.text.contains("scored out: enemies by it") {
                self.scored_out += 1;
            }
        }
        if self.picked.is_none() && frame.batteries.iter().any(|b| matches!(b, BatteryState::Carried(Some(_)))) {
            self.picked = Some(frame.clock);
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        let first = self.first?;
        let since = |at: Option<At>| at.map(|at| f64::from(at.0.saturating_sub(first.0)) / 1_000.0);
        match n {
            0 => Some(f64::from(self.takes)),
            1 => since(self.first_take),
            2 => since(self.picked),
            3 => Some(f64::from(self.scored_out)),
            _ => None,
        }
    }
}

fn contest_measure() -> Measure {
    Measure::Custom { names: &["mate_takes", "first_take_s", "picked_up_s", "scored_out"], make: ContestMeter::make }
}

pub fn steps() -> Vec<Step> {
    let school = || Group::new(SCHOOL, BATTERIES).of_size(10);
    vec![
        // CT-P1-1: an-anticamper 6.1, first half. The owner holds the raiders' way as he did on 2026-10-27.
        Step::new(
            "threat-flank-held",
            school(),
            "Hold the spot on the raiders' way to your container (marked); stand and move a little as usual and kill what comes.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are at the held spot", holds: near_held_spot })])
        .end(&[died_twice(), End::OwnerLeft { at: HELD_SPOT, r_m: 20.0 }])
        .measure(flank_measure())
        .measure(Measure::OwnerKills)
        .measure(Measure::OwnerDeaths)
        .measure(Measure::Notes { name: "jobs", contains: "job on threat spot" })
        .measure(Measure::Notes { name: "flank_takes", contains: "takes Flank ThreatSpot" })
        // K2: a flanker takes the play and one comes from off his lanes before he leaves or dies.
        .bar("flank_takes", Test::AtLeast(1.0))
        .bar("flank_arrivals", Test::AtLeast(1.0))
        .budget_s(180)
        .tries(1),
        // CT-P1-2: the second half, crouched and still (the gate of Flank, `takes Flank Place(0)`, is the other trigger).
        Step::new(
            "threat-flank-crouched",
            school(),
            "Same spot: crouch, aim down sights and keep still while the bots come.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are crouched and still at the spot", holds: crouched_at_held_spot })])
        .end(&[died_twice(), End::OwnerLeft { at: HELD_SPOT, r_m: 20.0 }])
        .measure(flank_measure())
        .measure(Measure::OwnerKills)
        .measure(Measure::OwnerDeaths)
        .measure(Measure::Notes { name: "flank_takes_gate", contains: "takes Flank Place(0)" })
        .measure(Measure::Notes { name: "flank_takes", contains: "takes Flank ThreatSpot" })
        .budget_s(120)
        .tries(1),
        // CT-P1-3: K1, the paths. Entries into his lanes per 10 s, quick second kills.
        Step::new(
            "threat-paths-lanes",
            school(),
            "Hold the same spot and keep killing; move up to 8 m around it between kills.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are at the held spot", holds: near_held_spot })])
        .end(&[died_twice(), End::OwnerLeft { at: HELD_SPOT, r_m: 25.0 }])
        .measure(lane_measure())
        .measure(Measure::OwnerKills)
        .measure(Measure::OwnerDeaths)
        .bar("lane_entries_per_10s", Test::AtMost(0.6))
        .budget_s(180)
        .tries(1),
        // CT-P1-4: RC5, F4: the spot is not cleared while he sits in it.
        Step::new(
            "threat-spot-clearing",
            school(),
            "After 2 kills from one spot, crouch behind cover for 10 s without firing, then stand and fire again.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you have 2 kills", holds: two_kills })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: HELD_SPOT, r_m: 40.0 }])
        .measure(clear_measure())
        .measure(Measure::OwnerKills)
        .bar("wrong_clears", Test::AtMost(0.0))
        .budget_s(90)
        .tries(2),
        // CT-P2-1: an-anticamper 6.2. Tarakanovsky Fort; a battery dropped by a carrier he killed.
        Step::new(
            "threat-bait-battery",
            Group::new(TARAKANOVSKY_FORT, BATTERIES).of_size(10),
            "Kill an enemy carrier so his battery drops, then hold 15-25 m from it with a clear line for 2 minutes.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "a battery lies 12-25 m from you", holds: battery_to_hold })])
        .end(&[End::OwnerDied])
        .measure(bait_measure())
        .measure(Measure::OwnerKills)
        .measure(Measure::Notes { name: "collect_takes", contains: "takes Collect" })
        .bar("deaths_by_battery", Test::AtMost(2.0))
        .budget_s(120)
        .arm_s(180)
        .tries(1),
        // CT-P1-5: a camper outside the enemy spawn exit (the owner's question). The step starts within 5 m of one of team 1's exits.
        Step::new(
            "threat-spawn-exit",
            Group::new(TARAKANOVSKY_FORT, BATTERIES).of_size(10),
            "Walk out to team 1's spawn exit (the way out of their base) and hold just outside it, facing in; kill what comes out.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are within 5 m of an enemy spawn exit", holds: near_enemy_exit })])
        .end(&[End::OwnerDied])
        .measure(exit_measure())
        .measure(Measure::OwnerKills)
        .measure(Measure::OwnerDeaths)
        // After his 2nd kill at most one more team-1 death per 10 s at that exit (the bots leave by another exit or wait out of his lanes).
        .bar("deaths_per_10s_after_2nd", Test::AtMost(1.0))
        .budget_s(180)
        .arm_s(180)
        .tries(1),
        // Z5 / an-objective 6.4: the contested battery in Cologne's middle (the match gives the bots; the runner places none).
        Step::new(
            "threat-contested-battery",
            Group::new(COLOGNE, BATTERIES).of_size(10),
            "Wait 20 s at your spawn, then go for the battery in the middle however you like.",
        )
        .start(&[Cond::After(Span::s(20))])
        .end(&[End::OwnerDied, End::Custom(EndWhen { reason: "picked_up", describe: "someone picks a battery up", holds: picked_up })])
        .measure(contest_measure())
        .measure(Measure::OwnerDeaths)
        .bar("mate_takes", Test::AtLeast(2.0))
        .bar("picked_up_s", Test::AtMost(60.0))
        .budget_s(180)
        .tries(1),
    ]
}

/// The end the holds share: he has died twice.
fn died_twice() -> End {
    End::Custom(EndWhen { reason: "died_twice", describe: "you died twice", holds: two_deaths })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::pools::Pools;
    use crate::v026g::bots::reflex::Gates;

    fn body(place: Place, team: u8, alive: bool, at: Vec3, kills: u32, crouched: bool) -> Body {
        Body { place, team, alive, feet: at, health: 1.0, kills, deaths: 0, crouched, keys: 0, yaw: 0.0, pitch: 0.0, speed: 0.0 }
    }

    fn bot(place: Place, alive: bool, at: Vec3) -> BotView {
        BotView {
            body: body(place, 1, alive, at, 0, false),
            mind: Mind { target: None, gates: Gates { at: 0, bits: 0, target: None }, legs: None },
            role: None,
        }
    }

    fn frame(clock: u32, owner_kills: u32, bots: Vec<BotView>, heard: Vec<Heard>, batteries: Vec<BatteryState>) -> Frame {
        Frame {
            clock: At(clock),
            playing: true,
            owner: Some(Owner { body: body(0, 0, true, Vec3::ZERO, owner_kills, false), pools: Some(Pools::FULL) }),
            bots,
            batteries,
            heard,
            sight: None,
            exits: Default::default(),
        }
    }

    fn say(clock: u32, place: Place, text: &str) -> Heard {
        Heard { place, time: At(clock), text: text.to_string() }
    }

    fn start() -> TryStart {
        TryStart { started: At(0), kills: 0, deaths: 0 }
    }

    /// Bearings and angles: east is 0, the angle between two is the short way round.
    #[test]
    fn bearings_and_angles_are_the_short_way_round() {
        assert_eq!(flat_bearing_deg(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0)), 0.0);
        assert!((flat_bearing_deg(Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0)) - 90.0).abs() < 1e-4);
        assert!((angle_between_deg(170.0, -170.0) - 20.0).abs() < 1e-4);
        assert!((angle_between_deg(0.0, 180.0) - 180.0).abs() < 1e-4);
    }

    /// His victims fall to the east; a bot from the north within 25 m is a flank, a bot in the east is not; the job and take notes are
    /// timed from his 2nd kill.
    #[test]
    fn the_flank_meter_counts_arrivals_off_the_lanes_and_times_the_notes() {
        let mut meter = FlankMeter::make(&start());
        let east = |alive| bot(10, alive, Vec3::new(20.0, 0.0, 0.0));
        meter.sample(&frame(0, 0, vec![east(true), bot(11, true, Vec3::new(60.0, 0.0, 60.0))], Vec::new(), Vec::new()));
        meter.sample(&frame(1_000, 1, vec![east(false), bot(11, true, Vec3::new(60.0, 0.0, 60.0))], Vec::new(), Vec::new()));
        let second = bot(12, true, Vec3::new(30.0, 0.0, 0.0));
        meter.sample(&frame(2_000, 1, vec![second.clone(), bot(11, true, Vec3::new(0.0, 0.0, 20.0))], Vec::new(), Vec::new()));
        // The second victim falls to the east; his 2nd kill at 3 s, a job note at 5 s, the take at 7 s.
        meter.sample(&frame(3_000, 2, vec![bot(12, false, second.body.feet), bot(11, true, Vec3::new(0.0, 0.0, 20.0))], Vec::new(), Vec::new()));
        meter.sample(&frame(
            5_000,
            2,
            vec![bot(11, true, Vec3::new(0.0, 0.0, 20.0))],
            vec![say(5_000, usize::MAX, "job on threat spot 1: Flanker is bot 11")],
            Vec::new(),
        ));
        meter.sample(&frame(7_000, 2, vec![bot(11, true, Vec3::new(0.0, 0.0, 20.0))], vec![say(7_000, 11, "takes Flank ThreatSpot(1)")], Vec::new()));
        let v = |n| meter.value(n);
        assert_eq!(v(0), Some(2.0), "job 2 s after the 2nd kill");
        assert_eq!(v(1), Some(4.0));
        assert_eq!(v(2), Some(1.0), "bot 11 from the north, once");
        assert_eq!(v(3), Some(2.0));
    }

    /// Lane entries count a bot coming into a wedge after the 2nd kill; kills within 3 s of the one before are the quick ones.
    #[test]
    fn the_lane_meter_counts_entries_after_the_second_kill_and_the_quick_kills() {
        let mut meter = LaneMeter::make(&start());
        let on_lane = |place| bot(place, true, Vec3::new(30.0, 0.0, 0.0));
        let off_lane = |place| bot(place, true, Vec3::new(0.0, 0.0, 30.0));
        meter.sample(&frame(0, 0, vec![bot(10, true, Vec3::new(20.0, 0.0, 0.0))], Vec::new(), Vec::new()));
        meter.sample(&frame(1_000, 1, vec![bot(10, false, Vec3::new(20.0, 0.0, 0.0))], Vec::new(), Vec::new()));
        meter.sample(&frame(2_000, 2, vec![bot(10, false, Vec3::new(20.0, 0.0, 0.0)), bot(11, true, Vec3::new(0.0, 0.0, 25.0))], Vec::new(), Vec::new()));
        // After the 2nd kill (at 2 s): bot 12 enters the lane at 6 s; bot 13 stays off it; 20 s in all.
        meter.sample(&frame(6_000, 2, vec![on_lane(12), off_lane(13)], Vec::new(), Vec::new()));
        meter.sample(&frame(12_000, 3, vec![on_lane(12), off_lane(13)], vec![say(12_000, 12, "its centre moved")], Vec::new()));
        meter.sample(&frame(22_000, 3, vec![on_lane(12), off_lane(13)], Vec::new(), Vec::new()));
        let v = |n| meter.value(n).unwrap();
        assert!((v(0) - 1.0 / 2.0).abs() < 1e-9, "one entry (12) in 20 s after the 2nd kill: {}", v(0));
        assert!((v(1) - 0.5).abs() < 1e-9, "kills at 1 s, 2 s (quick) and 12 s: one of two gaps");
        assert_eq!((v(2), v(3), v(4)), (3.0, 0.0, 1.0));
    }

    /// The spot of a note is read; a clear while he is within 10 m of it is wrong.
    #[test]
    fn the_clear_meter_calls_a_clear_beside_him_wrong() {
        assert_eq!(spot_of("threat spot 3 of shooter 0 at (47.0, 17.0, 116.0): cleared, seen empty"), Some((47.0, 116.0)));
        assert_eq!(spot_of("team 1: threat spots keep 2 regions"), None);
        let mut meter = ClearMeter::make(&start());
        meter.sample(&frame(0, 2, Vec::new(), Vec::new(), Vec::new()));
        meter.sample(&frame(4_000, 2, Vec::new(), vec![say(4_000, usize::MAX, "threat spot 1 of shooter 0 at (3.0, 0.0, 4.0): cleared, seen empty")], Vec::new()));
        meter.sample(&frame(6_000, 2, Vec::new(), vec![say(6_000, usize::MAX, "threat spot 2 of shooter 0 at (90.0, 0.0, 4.0): cleared, seen empty")], Vec::new()));
        let v = |n| meter.value(n).unwrap();
        assert_eq!((v(0), v(1), v(2)), (2.0, 1.0, 4.0), "the first spot is 5 m from him, the second 90 m");
    }

    /// The bait meter: it fixes the battery that lies within 25 m, counts the bots that come within 8 m of it once, and the ones that fall by it.
    #[test]
    fn the_bait_meter_counts_arrivals_and_deaths_by_the_battery() {
        let battery = Vec3::new(18.0, 0.0, 0.0);
        let lying = || vec![BatteryState::Free(battery)];
        assert!(battery_to_hold(&frame(0, 0, Vec::new(), Vec::new(), lying()), Span(0)));
        assert!(!battery_to_hold(&frame(0, 0, Vec::new(), Vec::new(), vec![BatteryState::Free(Vec3::new(5.0, 0.0, 0.0))]), Span(0)), "too near");
        assert!(!battery_to_hold(&frame(0, 0, Vec::new(), Vec::new(), vec![BatteryState::Carried(None)]), Span(0)));
        let mut meter = BaitMeter::make(&start());
        meter.sample(&frame(0, 0, vec![bot(10, true, Vec3::new(40.0, 0.0, 0.0))], Vec::new(), lying()));
        meter.sample(&frame(2_000, 0, vec![bot(10, true, Vec3::new(20.0, 0.0, 0.0))], Vec::new(), lying()));
        meter.sample(&frame(3_000, 0, vec![bot(10, true, Vec3::new(19.0, 0.0, 0.0))], Vec::new(), lying()));
        meter.sample(&frame(4_000, 0, vec![bot(10, false, Vec3::new(19.0, 0.0, 0.0))], Vec::new(), lying()));
        let v = |n| meter.value(n).unwrap();
        assert_eq!((v(0), v(1), v(2)), (1.0, 1.0, 2.0));
    }

    /// The exit meter counts the bots that fall at an exit of team 1's, the pairs within 3 s, and the rate after his 2nd kill.
    #[test]
    fn the_exit_meter_counts_deaths_at_the_exit_and_the_rate_after_the_second_kill() {
        let exit = Vec3::new(40.0, 0.0, 0.0);
        let mut meter = ExitMeter::make(&start());
        let f = |ms, kills, bots: Vec<BotView>| {
            let mut f = frame(ms, kills, bots, Vec::new(), Vec::new());
            f.exits[1] = vec![exit];
            f
        };
        let at_exit = |place, alive| bot(place, alive, exit + Vec3::new(5.0, 0.0, 0.0));
        meter.sample(&f(0, 0, vec![at_exit(10, true), at_exit(11, true), at_exit(12, true), bot(13, true, Vec3::new(0.0, 0.0, 90.0))]));
        meter.sample(&f(1_000, 1, vec![at_exit(10, false), at_exit(11, true), at_exit(12, true), bot(13, true, Vec3::new(0.0, 0.0, 90.0))]));
        meter.sample(&f(2_000, 2, vec![at_exit(10, false), at_exit(11, false), at_exit(12, true), bot(13, true, Vec3::new(0.0, 0.0, 90.0))]));
        // One more at the exit 8 s later, 20 s into the try; a bot far from the exit falls too (not counted).
        meter.sample(&f(10_000, 2, vec![at_exit(10, false), at_exit(11, false), at_exit(12, false), bot(13, false, Vec3::new(0.0, 0.0, 90.0))]));
        meter.sample(&f(22_000, 2, vec![at_exit(10, false), at_exit(11, false), at_exit(12, false), bot(13, false, Vec3::new(0.0, 0.0, 90.0))]));
        let v = |n| meter.value(n).unwrap();
        assert_eq!((v(0), v(1)), (3.0, 1.0), "three at the exit, one pair (1 s and 2 s)");
        assert!((v(2) - 0.5).abs() < 1e-9, "one after the 2nd kill, in 20 s: {}", v(2));
        assert!(near_enemy_exit(&{ let mut g = frame(0, 0, Vec::new(), Vec::new(), Vec::new()); g.exits[1] = vec![Vec3::new(3.0, 0.0, 0.0)]; g }, Span(0)));
        assert!(!near_enemy_exit(&frame(0, 0, Vec::new(), Vec::new(), Vec::new()), Span(0)), "no exits known");
    }

    /// The contest meter counts the mates' takes, the notes of the scored-out candidates and the first moment a battery is carried.
    #[test]
    fn the_contest_meter_counts_mate_takes_and_the_pickup() {
        let mate = |place| {
            let mut b = bot(place, true, Vec3::new(10.0, 0.0, 0.0));
            b.body.team = 0;
            b
        };
        let mut meter = ContestMeter::make(&start());
        meter.sample(&frame(0, 0, vec![mate(1), mate(2), bot(10, true, Vec3::new(30.0, 0.0, 0.0))], Vec::new(), vec![BatteryState::Free(Vec3::ZERO)]));
        meter.sample(&frame(
            3_000,
            0,
            vec![mate(1), mate(2), bot(10, true, Vec3::new(30.0, 0.0, 0.0))],
            vec![say(3_000, 1, "takes Collect Battery(3)"), say(3_000, 2, "takes Support"), say(3_000, 10, "takes Collect Battery(3)"), say(3_000, 1, "scored out: enemies by it")],
            vec![BatteryState::Free(Vec3::ZERO)],
        ));
        meter.sample(&frame(9_000, 0, vec![mate(1)], Vec::new(), vec![BatteryState::Carried(Some(1))]));
        let v = |n| meter.value(n).unwrap();
        assert_eq!((v(0), v(1), v(2), v(3)), (2.0, 3.0, 9.0, 1.0), "the enemy's own take is not a mate's");
        assert!(picked_up(&frame(0, 0, Vec::new(), Vec::new(), vec![BatteryState::Carried(Some(1))]), &Progress { since_start: Span::s(1), owner_kills: 0, owner_deaths: 0 }));
    }

    // ---- dry runs: the runner given a made-up match (dry.rs) ----

    fn named(id: &str) -> Step {
        steps().into_iter().find(|s| s.id == id).unwrap_or_else(|| panic!("no step {id}")).tries(1)
    }

    fn owner_at(at: Vec3, kills: u32, crouched: bool) -> Body {
        let mut owner = super::super::dry::body(0, 0, at);
        owner.kills = kills;
        owner.crouched = crouched;
        owner
    }

    fn dry_frame(ms: u32, owner: Body) -> Frame {
        super::super::dry::frame_at(ms, owner)
    }

    /// He holds the spot and kills two bots to the east (2 s and 3 s); a flanker from the north stands 20 m off at 6 s with a job note at
    /// 4 s and a take note at 5 s; at 40 s he walks away. PASS: a take, an arrival.
    #[test]
    fn dry_run_the_held_spot_passes_with_a_flank() {
        let (results, said) = super::super::dry::run(named("threat-flank-held"), 90, |ms| {
            let owner = owner_at(HELD_SPOT.vec(), u32::from(ms >= 2_000) + u32::from(ms >= 3_000), false);
            let owner = if ms >= 40_000 { Body { feet: HELD_SPOT.vec() + Vec3::new(30.0, 0.0, 0.0), ..owner } } else { owner };
            let mut f = dry_frame(ms, owner);
            let east = |alive| bot(10, alive, HELD_SPOT.vec() + Vec3::new(20.0, 0.0, 0.0));
            let east2 = |alive| bot(12, alive, HELD_SPOT.vec() + Vec3::new(25.0, 0.0, 0.0));
            let north = bot(11, ms >= 6_000, HELD_SPOT.vec() + Vec3::new(0.0, 0.0, 20.0));
            f.bots = vec![east(ms < 2_000), east2(ms < 3_000), north];
            if ms == 4_000 {
                f.heard.push(say(ms, usize::MAX, "job on threat spot 1: Flanker is bot 11"));
            }
            if ms == 5_000 {
                f.heard.push(say(ms, 11, "takes Flank ThreatSpot(1)"));
            }
            f
        });
        assert!(results.contains("threat-flank-held") && results.contains("PASS"), "{results}");
        assert!(results.contains("flank_arrivals=1") && results.contains("flank_takes=1"), "{results}");
        assert!(said.iter().any(|l| l.contains("Hold the spot")), "{said:?}");
    }

    /// Crouched and still at the spot starts the crouched step; it ends when he leaves and is an INFO row (no bar).
    #[test]
    fn dry_run_the_crouched_hold_starts_only_crouched() {
        let (results, _) = super::super::dry::run(named("threat-flank-crouched"), 60, |ms| {
            let crouched = (5_000..30_000).contains(&ms);
            let at = if ms >= 30_000 { HELD_SPOT.vec() + Vec3::new(40.0, 0.0, 0.0) } else { HELD_SPOT.vec() };
            dry_frame(ms, owner_at(at, 0, crouched))
        });
        assert!(results.contains("threat-flank-crouched") && results.contains("INFO"), "{results}");
    }

    /// Paths: nobody enters his lanes in 20 s after his 2nd kill: 0 entries a 10 s passes the bar.
    #[test]
    fn dry_run_the_paths_step_passes_with_no_lane_entry() {
        let (results, _) = super::super::dry::run(named("threat-paths-lanes"), 90, |ms| {
            let owner = owner_at(HELD_SPOT.vec(), u32::from(ms >= 2_000) + u32::from(ms >= 3_000), false);
            let owner = if ms >= 40_000 { Body { feet: HELD_SPOT.vec() + Vec3::new(30.0, 0.0, 0.0), ..owner } } else { owner };
            let mut f = dry_frame(ms, owner);
            f.bots = vec![bot(10, ms < 2_000, HELD_SPOT.vec() + Vec3::new(20.0, 0.0, 0.0)), bot(11, ms < 3_000, HELD_SPOT.vec() + Vec3::new(25.0, 0.0, 0.0))];
            f
        });
        assert!(results.contains("threat-paths-lanes") && results.contains("PASS") && results.contains("lane_entries_per_10s=0"), "{results}");
    }

    /// Spot clearing: a clear note for a spot 90 m off is no wrong clear; one beside him is, and fails the bar.
    #[test]
    fn dry_run_spot_clearing_fails_on_a_clear_beside_him() {
        let run = |text: &'static str| {
            super::super::dry::run(named("threat-spot-clearing"), 60, move |ms| {
                let owner = owner_at(HELD_SPOT.vec() + if ms >= 30_000 { Vec3::new(60.0, 0.0, 0.0) } else { Vec3::ZERO }, 2, false);
                let mut f = dry_frame(ms, owner);
                if ms == 10_000 {
                    f.heard.push(say(ms, usize::MAX, text));
                }
                f
            })
            .0
        };
        let far = run("threat spot 1 of shooter 0 at (200.0, 0.0, 200.0): cleared, seen empty");
        assert!(far.contains("PASS") && far.contains("clears=1") && far.contains("wrong_clears=0"), "{far}");
        let near = run("threat spot 1 of shooter 0 at (47.0, 17.0, 116.0): cleared, seen empty");
        assert!(near.contains("FAIL") && near.contains("wrong_clears=1"), "{near}");
    }

    /// The bait battery: it lies 18 m away from 4 s; one bot walks up to it at 20 s and falls at 30 s; the budget ends the try at 120 s.
    #[test]
    fn dry_run_the_bait_battery_counts_the_bots_at_it() {
        let battery = HELD_SPOT.vec() + Vec3::new(18.0, 0.0, 0.0);
        let (results, _) = super::super::dry::run(named("threat-bait-battery"), 150, |ms| {
            let mut f = dry_frame(ms, owner_at(HELD_SPOT.vec(), 1, false));
            if ms >= 4_000 {
                f.batteries = vec![BatteryState::Free(battery)];
            }
            let at = if ms >= 20_000 { battery } else { battery + Vec3::new(40.0, 0.0, 0.0) };
            f.bots = vec![bot(10, ms < 30_000, at)];
            f
        });
        assert!(results.contains("threat-bait-battery") && results.contains("PASS"), "{results}");
        assert!(results.contains("bots_at_battery=1") && results.contains("deaths_by_battery=1"), "{results}");
    }

    /// The exit hold: the step starts within 5 m of team 1's exit; two bots fall at it in the first minute; the owner dies at the end.
    #[test]
    fn dry_run_the_spawn_exit_hold_reads_the_deaths() {
        let exit = Vec3::new(60.0, 0.0, 0.0);
        let (results, _) = super::super::dry::run(named("threat-spawn-exit"), 120, |ms| {
            let mut owner = owner_at(exit + Vec3::new(3.0, 0.0, 0.0), u32::from(ms >= 5_000) + u32::from(ms >= 8_000), false);
            owner.alive = ms < 100_000;
            let mut f = dry_frame(ms, owner);
            f.exits[1] = vec![exit];
            f.bots = vec![bot(10, ms < 5_000, exit + Vec3::new(8.0, 0.0, 0.0)), bot(11, ms < 8_000, exit + Vec3::new(8.0, 0.0, 0.0))];
            f
        });
        assert!(results.contains("threat-spawn-exit") && results.contains("PASS") && results.contains("exit_deaths=2"), "{results}");
    }

    /// The contested battery: after 20 s two mates say they take it, someone has it at 45 s: PASS, ended by the pick-up.
    #[test]
    fn dry_run_the_contested_battery_passes_with_two_takes() {
        let (results, _) = super::super::dry::run(named("threat-contested-battery"), 120, |ms| {
            let mut f = dry_frame(ms, owner_at(Vec3::ZERO, 0, false));
            let mate = |place| {
                let mut b = bot(place, true, Vec3::new(10.0, 0.0, 0.0));
                b.body.team = 0;
                b
            };
            f.bots = vec![mate(1), mate(2), bot(10, true, Vec3::new(30.0, 0.0, 0.0))];
            f.batteries = vec![if ms >= 45_000 { BatteryState::Carried(Some(1)) } else { BatteryState::Free(Vec3::new(20.0, 0.0, 0.0)) }];
            if ms == 25_000 {
                f.heard.push(say(ms, 1, "takes Collect Battery(3)"));
                f.heard.push(say(ms, 2, "takes Support"));
            }
            f
        });
        assert!(results.contains("threat-contested-battery") && results.contains("PASS" ) && results.contains("picked_up"), "{results}");
        assert!(results.contains("mate_takes=2"), "{results}");
    }
}
