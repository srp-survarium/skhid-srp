//! The owner counts of the stuck watch (A240b, `unstick.window`), by team: its notes (and those the held keys alone told); of
//! them, the notes whose body walked [`WALKED_M`] or more over the watch's 1.5 s before it and ended [`NET_M`] or more from
//! where it began (`moved`: it went on), or came back within [`NET_M`] (`out_back`); the seconds a living bot with a route
//! held a walk key for [`PINNED_MS`] while its body stayed within [`PINNED_M`] of where it was then (`pinned_s`: stuck by the
//! body, whatever the watch said); and the seconds a living bot with no target stood on a stair's flight past the first
//! [`WAIT_MS`] of a stand (`flight_wait_s`, `route::StairFloors`).

use super::*;
use super::super::route::StairFloors;

/// The window before a note, ms (`stuck::STUCK_AFTER_MS`).
const WINDOW_MS: u32 = crate::v026g::bots::stuck::STUCK_AFTER_MS;
/// A walk of this much over the window is a walk (1 m/s), metres.
const WALKED_M: f32 = 1.5;
/// … that ends this near where it began came back, metres (the watch's own still, `stuck::STUCK_MOVED_M`).
const NET_M: f32 = crate::v026g::bots::stuck::STUCK_MOVED_M;
/// A walk key held this long with a route…
const PINNED_MS: u32 = 3_000;
/// … and the body within this of where it was then: pinned, metres.
const PINNED_M: f32 = 0.5;
/// A stand on a flight counts past this, ms (the route tally's wait).
const WAIT_MS: u32 = 2_000;
/// A body that moved under this in a tick stands, metres.
const STILL_M: f32 = 0.02;

/// What one body did lately.
#[derive(Clone, Default)]
struct Track {
    /// Its feet each tick over the last [`PINNED_MS`] (and a tick).
    ticks: std::collections::VecDeque<(u32, Vec3)>,
    /// The last tick it was dead, had no route or held no walk key.
    loose: u32,
    /// Since when it stands on a flight.
    on_flight: Option<u32>,
}

pub(super) struct StuckWatchCounts {
    tracks: Vec<Track>,
    stairs: Option<StairFloors>,
    /// By team: notes, by the keys alone, moved, out and back; pinned ticks; flight wait ticks.
    counts: [[u64; 6]; 2],
}

impl StuckWatchCounts {
    pub(super) fn new(n: usize) -> Self {
        Self { tracks: vec![Track::default(); n], stairs: None, counts: [[0; 6]; 2] }
    }
}

impl Topic for StuckWatchCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn note(&mut self, _run: &Run, note: &Note, kind: &NoteKind<'_>, game: &Game) {
        if !matches!(kind, NoteKind::Stuck) {
            return;
        }
        let (Some(body), Some(track)) = (game.bodies.get(note.place), self.tracks.get(note.place)) else {
            return;
        };
        let team = usize::from(body.team & 1);
        self.counts[team][0] += 1;
        self.counts[team][1] += u64::from(note.text.contains("(walk keys held"));
        // The note is of a world (its time), which comes to the bots later than its tick.
        let window: Vec<Vec3> = track.ticks.iter().filter(|t| t.0 + WINDOW_MS >= note.time && t.0 <= note.time).map(|t| t.1).collect();
        let walked: f32 = window.windows(2).map(|w| w[0].distance_xz(w[1])).sum();
        if let (Some(first), Some(last)) = (window.first(), window.last())
            && walked >= WALKED_M
        {
            self.counts[team][if first.distance_xz(*last) >= NET_M { 2 } else { 3 }] += 1;
        }
    }

    fn body(&mut self, _run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        let Some(track) = self.tracks.get_mut(at.place) else {
            return;
        };
        let (rec, team) = (at.rec, usize::from(at.game.bodies[at.place].team & 1));
        let stairs = self.stairs.get_or_insert_with(|| StairFloors::of(&at.game.world));
        let stands = rec.alive
            && at.game.bodies[at.place].target.is_none()
            && at.prev.feet.distance_xz(rec.feet) < STILL_M
            && stairs.under(&at.game.world.grid, rec.feet);
        track.on_flight = stands.then(|| track.on_flight.unwrap_or(at.tick));
        if track.on_flight.is_some_and(|since| at.tick >= since + WAIT_MS) {
            self.counts[team][5] += 1;
        }
        track.ticks.push_back((at.tick, rec.feet));
        while track.ticks.front().is_some_and(|t| t.0 + PINNED_MS + TICK_MS < at.tick) {
            track.ticks.pop_front();
        }
        if !rec.alive || !rec.has_route || rec.keys & crate::v026g::bots::stuck::HELD_KEYS == 0 {
            track.loose = at.tick;
            return;
        }
        let then = track.ticks.iter().find(|t| t.0 + PINNED_MS >= at.tick).map(|t| t.1);
        if at.tick >= track.loose + PINNED_MS && then.is_some_and(|then| then.distance_xz(rec.feet) < PINNED_M) {
            self.counts[team][4] += 1;
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let seconds = |ticks: u64| (ticks as f64 * f64::from(TICK_MS) / 1000.0).round();
        let team = |t: usize| {
            let c = self.counts[t];
            serde_json::json!({
                "notes": c[0],
                "notes_by_keys": c[1],
                "notes_moved": c[2],
                "notes_out_back": c[3],
                "pinned_s": seconds(c[4]),
                "flight_wait_s": seconds(c[5]),
            })
        };
        let json = serde_json::json!({ "stuck_watch": { "by_team": { "0": team(0), "1": team(1) } } });
        let line = |t: usize| {
            let c = self.counts[t];
            format!(
                "notes {} (by the keys {}), walked on {}, out and back {}, pinned {} s, standing on a flight {} s",
                c[0],
                c[1],
                c[2],
                c[3],
                seconds(c[4]),
                seconds(c[5])
            )
        };
        let text = format!("owner-test stuck watch: team 0: {}; team 1: {}", line(0), line(1));
        vec![("stuck_watch", Part::new(json, text))]
    }
}
