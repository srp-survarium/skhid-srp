//! What a match's polls and the bots' sight came to, kept for the match-end
//! log line (`Game::finish`): live numbers for the sight's two loads, the
//! ray budget and the server's poll gaps, and for what the sight does to
//! fights against players of every ping.
//!
//! Always on and allocation-free: a fixed histogram of the poll gaps, a few
//! counters.
//!
//! `MatchView::clock` is the server's clock. A player's ping changes only
//! how accurate the estimate of his body is (`view`, `body`), never the
//! sight's windows (`know::SIGHT_UNTESTED_MS`, `know::SIGHT_GRACE_MS`),
//! which run on that clock.

use super::Bots;
use super::reflex::Lapse;
use super::sense::{HeardCounts, RayBudget, STARVED_PLACES};
use super::view::PlaceView;

/// The ping bands of a target, ms: unknown or under 20, 20 to 60, 60 to 150,
/// 150 and over.
pub const PING_BANDS: usize = 4;
const PING_BAND_EDGES_MS: [u32; PING_BANDS - 1] = [20, 60, 150];
const PING_BAND_NAMES: [&str; PING_BANDS] = ["<20", "20-60", "60-150", "150+"];

/// How late a player's input reaches the server against the ping the match
/// server logs for him: about 0.45 of it (LIKELY; in the live recordings the
/// input arrives p50 27 ms after its tick where the logged ping is p50 58
/// ms: the one way upstream and the client's send lead). The bands' edges in
/// lateness are 9, 27 and 68 ms.
pub const LATENESS_PER_PING: f32 = 0.45;

/// The band of a body's ping, from how late his inputs come
/// (`PlaceView::late_ms`, [`LATENESS_PER_PING`]); a bot, known to its own
/// decided ticks, has none (band 0).
pub fn ping_band(body: &PlaceView) -> usize {
    if body.bot {
        return 0;
    }
    PING_BAND_EDGES_MS
        .iter()
        .take_while(|&&edge| body.late_ms as f32 >= edge as f32 * LATENESS_PER_PING)
        .count()
}

/// Width of a poll-gap bucket, ms, and their number; the last holds every
/// longer gap.
const GAP_BUCKET_MS: u32 = 5;
const GAP_BUCKETS: usize = 100;

/// A match's polls: the gaps between them, the budget's refusals.
#[derive(Clone, Debug)]
pub struct PollStats {
    last: Option<u32>,
    gaps: [u32; GAP_BUCKETS + 1],
    pub polls: u64,
    pub max_gap_ms: u32,
    /// Polls in which the ray budget refused a ray.
    pub binding: u64,
    /// Rays cast at enemies in view, outside the budget.
    pub exempt_rays: u64,
    pub starved_by_team: [u64; 2],
    pub starved_by_place: [u64; STARVED_PLACES],
    /// What the bots' hearing did, by the hearer's team ([`HeardCounts`]).
    pub heard_by_team: [HeardCounts; 2],
}

impl Default for PollStats {
    fn default() -> Self {
        Self {
            last: None,
            gaps: [0; GAP_BUCKETS + 1],
            polls: 0,
            max_gap_ms: 0,
            binding: 0,
            exempt_rays: 0,
            starved_by_team: [0; 2],
            starved_by_place: [0; STARVED_PLACES],
            heard_by_team: [HeardCounts::default(); 2],
        }
    }
}

impl PollStats {
    /// A poll at `clock` that ended with `rays`.
    pub fn record(&mut self, clock: u32, rays: &RayBudget) {
        if let Some(last) = self.last.filter(|&last| clock > last) {
            let gap = clock - last;
            self.max_gap_ms = self.max_gap_ms.max(gap);
            self.gaps[(gap / GAP_BUCKET_MS).min(GAP_BUCKETS as u32) as usize] += 1;
        }
        self.last = Some(clock);
        self.polls += 1;
        if rays.starved() > 0 {
            self.binding += 1;
        }
        self.exempt_rays += u64::from(rays.exempt);
        for (total, polled) in self.starved_by_team.iter_mut().zip(rays.starved_by_team) {
            *total += u64::from(polled);
        }
        for (total, polled) in self.starved_by_place.iter_mut().zip(rays.starved_by_place) {
            *total += u64::from(polled);
        }
        for (total, polled) in self.heard_by_team.iter_mut().zip(&rays.heard_by_team) {
            total.add(polled);
        }
    }

    /// The gap, ms, that 99 of 100 polls were within (the bucket's upper
    /// edge; the longest gap if it is beyond the buckets).
    pub fn p99_gap_ms(&self) -> u32 {
        let gaps: u64 = self.gaps.iter().map(|&n| u64::from(n)).sum();
        let mut within = 0;
        for (k, &n) in self.gaps.iter().enumerate() {
            within += u64::from(n);
            if within * 100 >= gaps * 99 {
                return if k == GAP_BUCKETS {
                    self.max_gap_ms
                } else {
                    (k as u32 + 1) * GAP_BUCKET_MS
                };
            }
        }
        self.max_gap_ms
    }
}

/// A bot's target sights that ended (`reflex::Lapse`), by the target's ping
/// band: the lapses, and those that cut a burst.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SightCounts {
    pub lapses: [u32; PING_BANDS],
    pub cuts: [u32; PING_BANDS],
}

impl SightCounts {
    pub fn add(&mut self, lapse: Lapse) {
        self.lapses[lapse.band] += 1;
        self.cuts[lapse.band] += u32::from(lapse.cut);
    }
}

impl Bots {
    /// The match's polls, for the bench and the log.
    pub fn polls(&self) -> &PollStats {
        &self.polls
    }

    /// The sight's counts of every bot, summed.
    pub fn sight_counts(&self) -> SightCounts {
        let mut sum = SightCounts::default();
        for bot in self.bots.iter().flatten() {
            for band in 0..PING_BANDS {
                sum.lapses[band] += bot.sight_counts.lapses[band];
                sum.cuts[band] += bot.sight_counts.cuts[band];
            }
        }
        sum
    }

    /// TEST ONLY: `place` as the view has it at `clock`, the tick of the
    /// last input taken for it, and its ping band.
    #[cfg(test)]
    pub(crate) fn player_view(
        &self,
        place: super::Place,
        clock: u32,
    ) -> Option<(PlaceView, Option<u32>, usize)> {
        let clock = super::view::Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        };
        let world = self.tactics.as_ref().map(|t| t.world.clone());
        let view = self.tracker.view(&clock, &self.tunings.match_wide().kin, world.as_deref());
        view.place(place).cloned().map(|body| {
            let band = ping_band(&body);
            (body, self.tracker.last_input(place), band)
        })
    }

    /// The match-end log line's text.
    pub fn poll_report(&self) -> String {
        let polls = &self.polls;
        let counts = self.sight_counts();
        let by_band = |counts: &[u32; PING_BANDS]| {
            PING_BAND_NAMES
                .iter()
                .zip(counts)
                .map(|(name, n)| format!("{name} ms {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "{} polls, gap longest {} ms, 99% within {} ms (the top of its 5 ms bucket); the ray \
             budget bound in {} polls, starved rays by team {:?}, {} rays at enemies in view \
             outside it; target sights lost by the target's ping: {}; of them bursts cut: {}",
            polls.polls,
            polls.max_gap_ms,
            polls.p99_gap_ms(),
            polls.binding,
            polls.starved_by_team,
            polls.exempt_rays,
            by_band(&counts.lapses),
            by_band(&counts.cuts),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gap_histogram_gives_the_longest_gap_and_the_p99() {
        let mut stats = PollStats::default();
        let rays = RayBudget::new(400);
        let mut clock = 0;
        for k in 0..1_000 {
            // 5 ms apart, five stalls of 230 ms among them.
            clock += if k % 200 == 199 { 230 } else { 5 };
            stats.record(clock, &rays);
        }
        assert_eq!(stats.polls, 1_000);
        assert_eq!(stats.max_gap_ms, 230);
        // 994 of 999 gaps are 5 ms: 99 % are within 10 ms (bucket 1's top).
        assert_eq!(stats.p99_gap_ms(), 10);
        // A gap beyond the buckets reports the longest.
        stats.record(clock + 1_000, &rays);
        assert_eq!(stats.max_gap_ms, 1_000);
    }

    #[test]
    fn a_poll_that_was_refused_a_ray_is_binding_and_counted_by_team() {
        let mut stats = PollStats::default();
        let mut rays = RayBudget::new(1);
        assert!(rays.take(3, 1));
        assert!(!rays.take(3, 1));
        assert!(!rays.take(12, 0));
        stats.record(100, &rays);
        stats.record(200, &RayBudget::new(400));
        assert_eq!(stats.binding, 1);
        assert_eq!(stats.starved_by_team, [1, 1]);
        assert_eq!(
            (stats.starved_by_place[3], stats.starved_by_place[12]),
            (1, 1)
        );
    }

    /// The band is read from the lateness of his inputs, at the edges of the
    /// logged ping times 0.45; a bot is under 20. (The lateness itself comes
    /// from the intake: `tests/sight.rs`.)
    #[test]
    fn a_players_ping_band_is_how_late_his_inputs_come() {
        let mut body = PlaceView {
            place: 1,
            team: 1,
            bot: false,
            present: true,
            alive: true,
            feet: mapdata::Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            velocity: [0.0, 0.0],
            keys: 0,
            health: 1.0,
            spawned: 0,
            kills: 0,
            deaths: 0,
            streak: 0,
            carrying: None,
            seen_at: 0,
            late_ms: 0,
            known_at: 0,
        };
        let mut band = |late: u32| {
            body.late_ms = late;
            ping_band(&body)
        };
        assert_eq!([band(0), band(8), band(9), band(26)], [0, 0, 1, 1]);
        assert_eq!([band(27), band(67), band(68), band(900)], [2, 2, 3, 3]);
        // The world's age is not his ping: a body whose last world is old.
        body.seen_at = 0;
        body.late_ms = 20;
        assert_eq!(ping_band(&body), 1);
        body.bot = true;
        body.late_ms = 500;
        assert_eq!(ping_band(&body), 0);
    }
}
