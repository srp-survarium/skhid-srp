//! The owner counts of the pin from cover (Breach part 1, `threat.pin_from_cover`, `plays/crossfire.rs`): the
//! peek-spot searches of a job's pinner with no pin spot out of the lanes, those that found one and the pins started
//! from one (`crossfire::pin_tally`, read as the difference from the topic's start: a match runs on one thread); and
//! from the bots' states each tick: the seconds at a peek spot, the peeks (a stand from hidden), and the kills made and
//! deaths taken at a peek spot, up or hidden; and the walks to a peek spot: arrived or dropped, their seconds, and the
//! distance to the spot at the start.

use super::*;
use crate::v026g::bots::plays::Running;
use crate::v026g::bots::plays::crossfire::{self, Peek, pin_tally};

pub(super) struct PinCoverCounts {
    from: [u64; 6],
    /// The walks recorded before this topic started (`pin_tally::walks`).
    walks_from: usize,
    /// Each place walking to a peek spot: the ticks so far.
    going: BTreeMap<Place, u64>,
    /// Walks that reached the spot and walks dropped on the way: count and ticks.
    walks_arrived: (u64, u64),
    walks_dropped: (u64, u64),
    /// Each place's peek phase at its last tick (none: not at a peek spot).
    at: BTreeMap<Place, Peek>,
    ticks: u64,
    peeks: u64,
    /// Kills made and deaths taken at a peek spot, up and hidden.
    kills: [u64; 2],
    deaths: [u64; 2],
    kills_seen: usize,
    last_kill: Option<(u32, Place, Place)>,
}

impl Default for PinCoverCounts {
    fn default() -> Self {
        Self {
            from: pin_tally::counted(),
            walks_from: pin_tally::walks().len(),
            at: BTreeMap::new(),
            going: BTreeMap::new(),
            walks_arrived: (0, 0),
            walks_dropped: (0, 0),
            ticks: 0,
            peeks: 0,
            kills: [0; 2],
            deaths: [0; 2],
            kills_seen: 0,
            last_kill: None,
        }
    }
}

/// 0 for up, 1 for hidden.
fn side(p: Peek) -> usize {
    match p {
        Peek::Up { .. } => 0,
        Peek::Hidden { .. } => 1,
    }
}

impl Topic for PinCoverCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        let running = bot.and_then(|b| b.running.as_ref());
        let now = match running {
            Some(Running::Crossfire(crossfire::State::Peek { phase, .. })) => Some(*phase),
            _ => None,
        };
        if matches!(running, Some(Running::Crossfire(crossfire::State::PeekGoing { .. }))) {
            *self.going.entry(place).or_insert(0) += 1;
        } else if let Some(ticks) = self.going.remove(&place) {
            let walks = if now.is_some() { &mut self.walks_arrived } else { &mut self.walks_dropped };
            walks.0 += 1;
            walks.1 += ticks;
        }
        match now {
            Some(phase) => {
                self.ticks += 1;
                if matches!(phase, Peek::Up { .. }) && !matches!(self.at.get(&place), Some(Peek::Up { .. })) {
                    self.peeks += 1;
                }
                self.at.insert(place, phase);
            }
            None => {
                self.at.remove(&place);
            }
        }
    }

    fn end(&mut self, _run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        // The kill log may be taken between ticks (the bench's events): read what is new.
        if self.kills_seen > 0 && game.kill_log.get(self.kills_seen - 1) != self.last_kill.as_ref() {
            self.kills_seen = 0;
        }
        let new = game.kill_log[self.kills_seen.min(game.kill_log.len())..].to_vec();
        self.kills_seen = game.kill_log.len();
        self.last_kill = game.kill_log.last().copied();
        for (_, shooter, victim) in new {
            if let Some(&p) = self.at.get(&shooter) {
                self.kills[side(p)] += 1;
            }
            if let Some(&p) = self.at.get(&victim) {
                self.deaths[side(p)] += 1;
            }
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("pin_cover", self.part())]
    }
}

impl PinCoverCounts {
    fn part(&self) -> Part {
        let now = pin_tally::counted();
        let [searched, found, started, arrived, start_dm, past_reach] = [0, 1, 2, 3, 4, 5].map(|i| now[i] - self.from[i]);
        let start_m = if started > 0 { start_dm as f64 / 10.0 / started as f64 } else { 0.0 };
        let walk_s = |(n, ticks): (u64, u64)| if n > 0 { ticks as f64 * f64::from(TICK_MS) / 1000.0 / n as f64 } else { 0.0 };
        let (arrived_s, dropped_s) = (walk_s(self.walks_arrived), walk_s(self.walks_dropped));
        let need = self.walk_need();
        let secs = self.ticks as f64 * f64::from(TICK_MS) / 1000.0;
        let per_min = if secs > 0.0 { self.peeks as f64 * 60.0 / secs } else { 0.0 };
        let json = serde_json::json!({
            "pin_cover": {
                "searched": searched,
                "found": found,
                "started": started,
                "arrived": arrived,
                "past_reach": past_reach,
                "start_m": (start_m * 10.0).round() / 10.0,
                "walks_arrived_dropped": [self.walks_arrived.0, self.walks_dropped.0],
                "walk_s_arrived_dropped": [(arrived_s * 10.0).round() / 10.0, (dropped_s * 10.0).round() / 10.0],
                "walks_need": need,
                "peek_s": (secs * 10.0).round() / 10.0,
                "peeks": self.peeks,
                "kills_up_hidden": self.kills,
                "deaths_up_hidden": self.deaths,
            },
        });
        Part::new(
            json,
            format!(
                "owner-test pin from cover: walk need: {}; {searched} peek-spot searches, {found} found one, {started} pins from cover ({start_m:.1} m away), {arrived} arrived, {past_reach} polls past the walk cap; walks {} arrived ({arrived_s:.1} s) {} dropped ({dropped_s:.1} s); {secs:.0} s at a peek spot, {} peeks ({per_min:.1} a minute); kills up {} hidden {}, deaths up {} hidden {}",
                need.text(),
                self.walks_arrived.0,
                self.walks_dropped.0,
                self.peeks, self.kills[0], self.kills[1], self.deaths[0], self.deaths[1]
            ),
        )
    }
}

/// The walks to a peek spot against the time they had: each walk's speed from its own progress, the time it needed
/// for the whole way at that speed, and the time it had (to its last step: its arrival, or its job's end).
#[derive(serde::Serialize, Default)]
struct WalkNeed {
    walks: usize,
    /// Dropped walks that moved, and of those the ones whose whole way needed more than the time they had.
    dropped_moving: usize,
    dropped_short_of_time: usize,
    /// Medians: the job's age at the start, the distance at the start, the time the whole way needed.
    job_age_s: f32,
    start_m: f32,
    need_s: f32,
    /// Walks whose whole way needed more than the median job (6.9 s, the 2026-10-30 mechanism read).
    need_over_job: usize,
}

impl WalkNeed {
    fn text(&self) -> String {
        format!(
            "{} walks, need median {:.1} s ({} over the 6.9 s job), {:.1} m, job {:.1} s old; dropped moving {} of which {} short of time",
            self.walks, self.need_s, self.need_over_job, self.start_m, self.job_age_s, self.dropped_moving, self.dropped_short_of_time
        )
    }
}

fn median(mut v: Vec<f32>) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f32::total_cmp);
    (v[v.len() / 2] * 10.0).round() / 10.0
}

impl PinCoverCounts {
    fn walk_need(&self) -> WalkNeed {
        let walks = pin_tally::walks();
        let walks = &walks[self.walks_from.min(walks.len())..];
        let mut out = WalkNeed { walks: walks.len(), ..WalkNeed::default() };
        let mut need = Vec::new();
        for w in walks {
            let had_s = w.t_last.saturating_sub(w.start) as f32 / 1000.0;
            let moved = w.d0 - w.d_last;
            let need_s = (had_s > 0.0 && moved > 0.5).then(|| w.d0 / (moved / had_s));
            if let Some(n) = need_s {
                need.push(n);
                if n > 6.9 {
                    out.need_over_job += 1;
                }
            }
            if !w.arrived && moved > 0.5 {
                out.dropped_moving += 1;
                if need_s.is_some_and(|n| n > had_s) {
                    out.dropped_short_of_time += 1;
                }
            }
        }
        out.job_age_s = median(walks.iter().map(|w| w.job_age_ms as f32 / 1000.0).collect());
        out.start_m = median(walks.iter().map(|w| w.d0).collect());
        out.need_s = median(need);
        out
    }
}
