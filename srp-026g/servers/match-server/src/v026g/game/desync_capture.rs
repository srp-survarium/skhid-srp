//! The desync capture: the moment the games' hash reports for a tick
//! disagree, one game of each side is asked for its world, both worlds are
//! saved with what differs between them (`world_diff`), and one line says
//! it in the log (`docs/notes/desync_capture.md`).
//!
//! A game answers a request (0xF6) with its event-horizon world of the
//! moment it handles it (client_patch.md section 3.4), the oldest world it
//! keeps: the tick a report names has left the game by the time the report
//! is here. So the two requests are timed for one tick, the first both
//! games can still give (`aim_at`), as soon as the split is seen: about a
//! round trip after the tick the games first disagreed on.
//!
//! It only looks. Its requests go to games no other request waits on, it
//! is never a vote, a candidate or a world anyone starts from, and the
//! resync of a game out of step goes as it would without it.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::*;
use crate::v026g::world_diff::{self, WorldDiff};
use crate::v026g::world_validation::{self, Roster};

/// Captures in a match at most ...
const CAPTURES_PER_MATCH: u32 = 4;
/// ... and this long apart: a game that splits and comes back again and
/// again would take them all in its first minute.
const CAPTURE_GAP: Duration = Duration::from_secs(20);
/// A split's worlds are asked for within this of its first disagreeing
/// report, or not at all: a world asked for later is past the split by
/// more than its cause.
const ASK_REACH: Duration = Duration::from_millis(500);
/// The worlds asked for are written this long after the request at the
/// latest, whatever came (an upload takes well under a second; a full
/// packet pool, a world refused or a game gone sends none).
const CAPTURE_WAIT: Duration = Duration::from_secs(4);
/// ... and this long after it at the soonest: the reports of the split's tick
/// from the games of longer links come in meanwhile, and the sides it
/// names are all of them.
const SIDES_WAIT: Duration = Duration::from_secs(1);
/// The latest reports' lags kept for each game (`ReportLags`).
const LAGS_KEPT: usize = 20;
/// The folder of the captures under `SRP_SNAPSHOT_DIR`.
pub(crate) const CAPTURES_DIR: &str = "captures";

/// How far behind the match clock a game's hash reports arrive: the
/// match clock at each report less the tick it names, the latest
/// [`LAGS_KEPT`]. A game reports a tick as its event horizon passes it;
/// the least lag is the one held up least on its way, so the game's
/// upload offset (where its event horizon is against the match clock, a
/// way on) is about its round trip less that ([`Self::upload_offset`]).
#[derive(Default)]
pub(super) struct ReportLags(VecDeque<i64>);

impl ReportLags {
    pub(super) fn push(&mut self, clock: u32, tick: u32) {
        if self.0.len() == LAGS_KEPT {
            self.0.pop_front();
        }
        self.0.push_back(i64::from(clock) - i64::from(tick));
    }

    /// The middle of the game's upload offset as its reports put it, for
    /// a round trip of `round_trip` ms: a request that leaves at match
    /// clock `c` reaches it half a round trip later, and it uploads the
    /// tick its event horizon is at then, about `c` plus this.
    fn upload_offset(&self, round_trip: u32) -> Option<i64> {
        let least = self.0.iter().min()?;
        Some(i64::from(round_trip) - least)
    }
}

/// Where the games are in their agreement, as their reports show it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Episode {
    /// They agree; a disagreement at a tick after `after` starts a split.
    Agreeing { after: u32 },
    /// They disagree from `first` (the first tick whose reports were seen
    /// to differ), seen at `seen`; the worlds are still to be asked for.
    Unasked { first: u32, seen: Instant },
    /// They disagree from `first`, and the worlds were asked for or could
    /// not be: nothing more until they agree again.
    Handled { first: u32 },
}

/// What a disagreeing report means for the capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Disagreement {
    /// A split not asked for yet, first seen at `first`: ask now.
    Ask {
        first: u32,
    },
    /// The split's worlds could not be asked for in time.
    Missed {
        first: u32,
    },
    Nothing,
}

impl Episode {
    /// The reports for `tick` differ.
    fn disagree(&mut self, tick: u32, now: Instant) -> Disagreement {
        match *self {
            Episode::Agreeing { after } if tick > after => {
                *self = Episode::Unasked {
                    first: tick,
                    seen: now,
                };
                Disagreement::Ask { first: tick }
            }
            Episode::Unasked { first, seen } => {
                if now.saturating_duration_since(seen) < ASK_REACH {
                    Disagreement::Ask { first }
                } else {
                    *self = Episode::Handled { first };
                    Disagreement::Missed { first }
                }
            }
            Episode::Agreeing { .. } | Episode::Handled { .. } => Disagreement::Nothing,
        }
    }

    /// Every game in the simulation reported the same for `tick`.
    fn agree(&mut self, tick: u32) {
        match *self {
            Episode::Agreeing { after } => {
                *self = Episode::Agreeing {
                    after: after.max(tick),
                }
            }
            Episode::Unasked { first, .. } | Episode::Handled { first } if tick > first => {
                *self = Episode::Agreeing { after: tick }
            }
            Episode::Unasked { .. } | Episode::Handled { .. } => {}
        }
    }

    /// Its worlds were asked for, or will not be.
    fn handled(&mut self) {
        if let Episode::Unasked { first, .. } = *self {
            *self = Episode::Handled { first };
        }
    }
}

/// One side of a split: the games that reported its CRC.
#[derive(Clone, Debug)]
struct Group {
    crc: u32,
    places: Vec<usize>,
}

/// The game of a side asked for its world.
struct Side {
    index: usize,
    /// What it reported for the split's tick.
    crc: u32,
    round_trip: u32,
    world: Option<Snapshot>,
}

/// How the two worlds were asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asking {
    /// Both games answer a request for a tick (patch 211): each was asked for
    /// the split's first tick, which it gives from the worlds it hashed, or
    /// its event-horizon world when it no longer holds that tick.
    Tick(u32),
    /// For their event-horizon worlds, the requests timed so that both give
    /// the same tick (`aim_tick`): the tick, or `None` when no timing was
    /// known and both went at once.
    Aimed(Option<u32>),
}

impl Asking {
    fn word(self) -> &'static str {
        match self {
            Asking::Tick(_) => "tick",
            Asking::Aimed(_) => "aimed",
        }
    }

    /// The tick the worlds were asked for or timed for.
    fn tick(self) -> Option<u32> {
        match self {
            Asking::Tick(tick) => Some(tick),
            Asking::Aimed(tick) => tick,
        }
    }
}

/// A capture whose worlds are on their way.
struct Pending {
    number: u32,
    first: u32,
    /// The match clock when the split was seen.
    seen_at: u32,
    groups: Vec<Group>,
    sides: [Side; 2],
    asking: Asking,
    asked: Instant,
}

/// The match's captures: its episode, the one on its way, how many.
pub(super) struct Captures {
    episode: Episode,
    pending: Option<Pending>,
    taken: u32,
    last: Option<Instant>,
}

impl Default for Captures {
    fn default() -> Self {
        Self {
            episode: Episode::Agreeing { after: 0 },
            pending: None,
            taken: 0,
            last: None,
        }
    }
}

impl Game {
    /// A report for `time` came: a split starts or ends there.
    pub(super) fn watch_desync(&mut self, time: u32, now: Instant) {
        if self.installing() {
            // The games' worlds differ by the new place until each has it.
            return;
        }
        let Some(reports) = self.hashes.get(&time) else {
            return;
        };
        let mut crcs = reports.values();
        let first_crc = crcs.next().copied();
        let differ = crcs.any(|crc| Some(*crc) != first_crc);
        if !differ {
            if reports.len() >= self.live().max(2) {
                self.captures.episode.agree(time);
            }
            return;
        }
        match self.captures.episode.disagree(time, now) {
            Disagreement::Nothing => {}
            Disagreement::Missed { first } => log::info!(
                "Match {:#x}: the games split at {first} ms; no game of each side could be asked \
                 for its world in time (desync capture)",
                self.id
            ),
            Disagreement::Ask { first } => self.ask_capture(first, time, now),
        }
    }

    /// Ask a game of each of the two largest sides at `time` for its world,
    /// the best linked of each, timed for one tick.
    fn ask_capture(&mut self, first: u32, time: u32, now: Instant) {
        let limited = self.captures.taken >= CAPTURES_PER_MATCH
            || self.captures.pending.is_some()
            || self
                .captures
                .last
                .is_some_and(|at| now.saturating_duration_since(at) < CAPTURE_GAP);
        if limited {
            self.captures.episode.handled();
            return;
        }
        let Some(reports) = self.hashes.get(&time) else {
            return;
        };
        let groups = groups_of(reports);
        let askable = |index: usize| {
            !self.is_bot(index)
                && !self.waiting_entry(index)
                && self.peers[index].as_ref().is_some_and(|peer| {
                    peer.patched
                        && peer.stage == Stage::Live
                        && !peer.is_asked(now)
                        && peer.ask_at.is_none()
                })
        };
        let best = |group: &Group| {
            group
                .places
                .iter()
                .copied()
                .filter(|index| askable(*index))
                .min_by_key(|index| {
                    (
                        self.peers[*index]
                            .as_ref()
                            .map_or(u32::MAX, |peer| peer.round_trip),
                        *index,
                    )
                })
        };
        let chosen: Vec<(usize, u32)> = groups
            .iter()
            .filter_map(|group| Some((best(group)?, group.crc)))
            .take(2)
            .collect();
        let [(a, crc_a), (b, crc_b)] = chosen[..] else {
            // No game of a second side to ask yet: maybe at its next report.
            return;
        };
        let clock = self.clock(now);
        let exact = [a, b].iter().all(|index| {
            self.peers[*index]
                .as_ref()
                .is_some_and(|peer| peer.exact_worlds)
        });
        // Both answer a request for a tick: the split's first, at once (each
        // holds the worlds it hashed of the last 1.6 s). Else their horizon's,
        // timed for one tick.
        let (asking, times) = if exact {
            (Asking::Tick(self.split_start(first)), vec![clock, clock])
        } else {
            let offset = |index: usize| {
                let peer = self.peers[index].as_ref()?;
                match peer.upload_offset {
                    Some((low, high)) => Some((low + high) / 2),
                    None => peer.report_lags.upload_offset(peer.round_trip),
                }
            };
            let aimed = [offset(a), offset(b)]
                .into_iter()
                .collect::<Option<Vec<i64>>>()
                .and_then(|middles| aim_tick(clock, &middles, None));
            match aimed {
                Some((target, times)) => (Asking::Aimed(Some(target)), times),
                None => (Asking::Aimed(None), vec![clock, clock]),
            }
        };
        let wanted = match asking {
            Asking::Tick(tick) => UploadWanted::Tick(WireTime::tick(tick).tick),
            Asking::Aimed(_) => UploadWanted::Horizon,
        };
        let side = |index: usize, crc: u32| Side {
            index,
            crc,
            round_trip: self.peers[index].as_ref().map_or(0, |peer| peer.round_trip),
            world: None,
        };
        self.captures.taken += 1;
        let pending = Pending {
            number: self.captures.taken,
            first,
            seen_at: clock,
            groups,
            sides: [side(a, crc_a), side(b, crc_b)],
            asking,
            asked: now,
        };
        log::debug!(
            "Match {:#x}: desync capture {}: players {a} and {b} are asked for their worlds (at \
             {times:?}, {asking:?}, clock {clock})",
            self.id,
            pending.number
        );
        self.captures.pending = Some(pending);
        self.captures.last = Some(now);
        self.captures.episode.handled();
        for (index, at) in [a, b].into_iter().zip(times) {
            if let Some(peer) = self.peers[index].as_mut() {
                peer.ask_at = Some((at, Wanted::Look(WrongWorld::Capture(wanted))));
            }
        }
        self.send_requests(now);
    }

    /// A world asked for by the capture came.
    pub(super) fn capture_world(&mut self, index: usize, snapshot: Snapshot) {
        let Some(pending) = self.captures.pending.as_mut() else {
            return;
        };
        let Some(side) = pending.sides.iter_mut().find(|side| side.index == index) else {
            return;
        };
        side.world = Some(snapshot);
    }

    /// The capture is written once both worlds came and [`SIDES_WAIT`] is
    /// over, or once [`CAPTURE_WAIT`] is, whatever came.
    pub(super) fn check_capture(&mut self, now: Instant) {
        if self.captures.pending.as_ref().is_some_and(|pending| {
            let waited = now.saturating_duration_since(pending.asked);
            let came = pending.sides.iter().all(|side| side.world.is_some());
            waited >= CAPTURE_WAIT || (came && waited >= SIDES_WAIT)
        }) {
            self.finish_capture();
        }
    }

    /// Whether the capture waits for a world of the player at `index`.
    #[cfg(test)]
    pub fn capture_awaits(&self, index: usize) -> bool {
        self.captures.pending.as_ref().is_some_and(|pending| {
            pending
                .sides
                .iter()
                .any(|side| side.index == index && side.world.is_none())
        })
    }

    /// Whether a capture is still to be written.
    #[cfg(test)]
    pub fn capture_pending(&self) -> bool {
        self.captures.pending.is_some()
    }

    /// The captures asked for in this match.
    #[cfg(test)]
    pub fn captures_taken(&self) -> u32 {
        self.captures.taken
    }

    /// The first tick of the split as the reports now show it: the first
    /// of the run of disagreeing ticks that holds `first` (a report of an
    /// earlier tick may have come after the one that showed the split).
    fn split_start(&self, first: u32) -> u32 {
        let mut start = first;
        for (time, reports) in self.hashes.range(..first).rev() {
            let mut crcs = reports.values();
            let one = crcs.next();
            if reports.len() >= 2 && crcs.all(|crc| Some(crc) == one) {
                break;
            }
            if reports.len() >= 2 {
                start = *time;
            }
        }
        start
    }

    /// Write the capture with the worlds that came, its diff and its log line.
    pub(super) fn finish_capture(&mut self) {
        let Some(pending) = self.captures.pending.take() else {
            return;
        };
        let split = self.split_start(pending.first);
        // The sides as all reports of the tick say them now (more came since).
        let mut pending = pending;
        if let Some(reports) = self.hashes.get(&pending.first) {
            pending.groups = groups_of(reports);
        }
        let diff = match (&pending.sides[0].world, &pending.sides[1].world) {
            (Some(a), Some(b)) => {
                let roster = Roster::of(&self.formed);
                let time = a.time.max(b.time);
                Some(world_validation::with_contexts(
                    time,
                    self.snapshot_players(),
                    roster,
                    |contexts| world_diff::diff(&a.body, &b.body, contexts),
                ))
            }
            _ => None,
        };
        let folder = self.settings.snapshot_dir.as_ref().map(|dir| {
            dir.join(CAPTURES_DIR).join(format!(
                "match026g_{:x}_{split}_c{}_r{}",
                self.id,
                pending.number,
                run_stamp()
            ))
        });
        let summary = Summary::of(self.id, split, &pending, diff.as_ref());
        let written = folder.as_ref().map(|folder| {
            write_capture(folder, self.id, &pending, &summary)
                .map(|()| folder.display().to_string())
        });
        if let Some(dir) = &self.settings.snapshot_dir {
            prune(&dir.join(CAPTURES_DIR), self.settings.capture_max_bytes);
        }
        let groups: Vec<String> = pending
            .groups
            .iter()
            .map(|group| format!("{:08x}:{:?}", group.crc, group.places))
            .collect();
        let worlds: Vec<String> = pending
            .sides
            .iter()
            .map(|side| match &side.world {
                Some(world) => format!("player {} at {} ms", side.index, world.time),
                None => format!("player {} none", side.index),
            })
            .collect();
        let gap = summary
            .after_split_ms
            .map_or(String::new(), |gap| format!(", {gap} ms after the split"));
        let same = if summary.same_tick || diff.is_none() {
            ""
        } else {
            ", NOT the same tick"
        };
        let how = match pending.asking {
            Asking::Tick(tick) if summary.exact => format!(" (asked for {tick} ms: exact)"),
            Asking::Tick(tick) => {
                format!(" (asked for {tick} ms: NOT given, a horizon's world instead)")
            }
            Asking::Aimed(_) => " (their horizons' worlds, timed for one tick)".to_owned(),
        };
        let first = match &diff {
            Some(diff) => diff
                .first_field()
                .map_or("the worlds are equal".to_owned(), |first| {
                    format!("first difference: {first}")
                }),
            None => "no diff (a world did not come)".to_owned(),
        };
        let files = match written {
            Some(Ok(folder)) => format!("; {folder}"),
            Some(Err(error)) => format!("; not saved: {error}"),
            None => String::new(),
        };
        log::warn!(
            "Match {:#x}: desync capture {} at {split} ms (seen {} ms): groups {}; worlds {}{how}{gap}{same}; \
             {first}{files}",
            self.id,
            pending.number,
            pending.seen_at,
            groups.join(" "),
            worlds.join(", ")
        );
    }
}

/// The capture's `capture.json`.
#[derive(Serialize)]
struct Summary {
    #[serde(rename = "match")]
    match_id: String,
    capture: u32,
    /// The first tick the games are known to disagree on.
    split_tick: u32,
    /// The tick whose reports showed it first.
    first_seen_tick: u32,
    /// The match clock then.
    seen_at_ms: u32,
    groups: Vec<GroupSummary>,
    /// How the worlds were asked for: `tick` (both games answer a request
    /// for a tick, patch 211: each asked for `asked_tick`, the split's first)
    /// or `aimed` (their event-horizon worlds, the requests timed for
    /// `asked_tick`, `null` when no timing was known).
    asked: &'static str,
    asked_tick: Option<u32>,
    /// Both worlds are of the split's first tick.
    exact: bool,
    worlds: Vec<WorldSummary>,
    /// How far past `split_tick` the worlds are (the later one).
    after_split_ms: Option<u32>,
    same_tick: bool,
    /// `diff.differences[0]` and the first with a field, in a line.
    first_difference: Option<String>,
    diff: Option<WorldDiff>,
}

#[derive(Serialize)]
struct GroupSummary {
    crc: String,
    places: Vec<usize>,
}

#[derive(Serialize)]
struct WorldSummary {
    place: usize,
    /// The CRC its game reported for the split's tick.
    reported: String,
    round_trip_ms: u32,
    tick: Option<u32>,
    crc: Option<String>,
    file: Option<String>,
}

impl Summary {
    fn of(id: u32, split: u32, pending: &Pending, diff: Option<&WorldDiff>) -> Self {
        let ticks: Vec<Option<u32>> = pending
            .sides
            .iter()
            .map(|side| side.world.as_ref().map(|world| world.time))
            .collect();
        let same_tick = ticks[0].is_some() && ticks[0] == ticks[1];
        let exact = ticks
            .iter()
            .all(|tick| tick.is_some() && *tick == Some(split));
        Summary {
            match_id: format!("{id:#x}"),
            capture: pending.number,
            split_tick: split,
            first_seen_tick: pending.first,
            seen_at_ms: pending.seen_at,
            asked: pending.asking.word(),
            asked_tick: pending.asking.tick(),
            exact,
            groups: pending
                .groups
                .iter()
                .map(|group| GroupSummary {
                    crc: format!("{:08x}", group.crc),
                    places: group.places.clone(),
                })
                .collect(),
            worlds: pending
                .sides
                .iter()
                .map(|side| WorldSummary {
                    place: side.index,
                    reported: format!("{:08x}", side.crc),
                    round_trip_ms: side.round_trip,
                    tick: side.world.as_ref().map(|world| world.time),
                    crc: side
                        .world
                        .as_ref()
                        .map(|world| format!("{:08x}", snapshot::crc(&world.body))),
                    file: side
                        .world
                        .as_ref()
                        .map(|world| world_name(id, side.index, world)),
                })
                .collect(),
            after_split_ms: ticks
                .iter()
                .flatten()
                .max()
                .map(|tick| tick.saturating_sub(split)),
            same_tick,
            first_difference: diff
                .and_then(WorldDiff::first_field)
                .map(ToString::to_string),
            diff: diff.cloned(),
        }
    }
}

/// The sides of a tick's reports, the largest first (the lowest place first of
/// equal ones), each its places in order.
fn groups_of(reports: &HashMap<usize, u32>) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    let mut indices: Vec<(&usize, &u32)> = reports.iter().collect();
    indices.sort_unstable();
    for (index, crc) in indices {
        match groups.iter_mut().find(|group| group.crc == *crc) {
            Some(group) => group.places.push(*index),
            None => groups.push(Group {
                crc: *crc,
                places: vec![*index],
            }),
        }
    }
    groups.sort_by_key(|group| std::cmp::Reverse(group.places.len()));
    groups
}

/// A captured world's file name: a dump's (`world_check::dump_of_name`).
fn world_name(id: u32, index: usize, world: &Snapshot) -> String {
    format!(
        "match026g_{id:x}_{}_p{index}_capture_r{}.bin",
        world.time,
        run_stamp()
    )
}

fn write_capture(
    folder: &Path,
    id: u32,
    pending: &Pending,
    summary: &Summary,
) -> std::io::Result<()> {
    fs::create_dir_all(folder)?;
    for side in &pending.sides {
        if let Some(world) = &side.world {
            fs::write(folder.join(world_name(id, side.index, world)), &world.body)?;
        }
    }
    let json = serde_json::to_vec_pretty(summary).map_err(std::io::Error::other)?;
    fs::write(folder.join("capture.json"), json)
}

/// The oldest captures go until the rest fit `max_bytes`.
fn prune(dir: &Path, max_bytes: u64) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let size = |folder: &Path| -> u64 {
        fs::read_dir(folder).map_or(0, |files| {
            files
                .flatten()
                .filter_map(|file| file.metadata().ok())
                .map(|meta| meta.len())
                .sum()
        })
    };
    let mut kept: Vec<(std::time::SystemTime, PathBuf, u64)> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            let path = entry.path();
            let bytes = size(&path);
            Some((modified, path, bytes))
        })
        .collect();
    kept.sort();
    let mut total: u64 = kept.iter().map(|(_, _, bytes)| bytes).sum();
    for (_, path, bytes) in kept {
        if total <= max_bytes {
            break;
        }
        log::info!(
            "Desync captures: over {} MB together; {} deleted",
            max_bytes >> 20,
            path.display()
        );
        if fs::remove_dir_all(&path).is_ok() {
            total -= bytes;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_episode_asks_once_and_a_new_one_starts_only_after_the_games_agree() {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let mut episode = Episode::Agreeing { after: 0 };
        assert_eq!(
            episode.disagree(1_000, at(0)),
            Disagreement::Ask { first: 1_000 }
        );
        // No game of a side could be asked: it is tried again, in reach.
        assert_eq!(
            episode.disagree(1_050, at(100)),
            Disagreement::Ask { first: 1_000 }
        );
        episode.handled();
        assert_eq!(episode.disagree(1_100, at(150)), Disagreement::Nothing);
        // A report of a tick before the split that agrees ends nothing.
        episode.agree(950);
        assert_eq!(episode, Episode::Handled { first: 1_000 });
        episode.agree(5_000);
        assert_eq!(episode, Episode::Agreeing { after: 5_000 });
        // A late report of a tick of the old split starts nothing.
        assert_eq!(episode.disagree(4_000, at(5_000)), Disagreement::Nothing);
        assert_eq!(
            episode.disagree(5_050, at(5_000)),
            Disagreement::Ask { first: 5_050 }
        );
        // Not asked within reach: missed once, then nothing.
        assert_eq!(
            episode.disagree(5_100, at(5_600)),
            Disagreement::Missed { first: 5_050 }
        );
        assert_eq!(episode.disagree(5_150, at(5_650)), Disagreement::Nothing);
    }

    #[test]
    fn a_games_upload_offset_is_its_round_trip_less_its_least_report_lag() {
        let mut lags = ReportLags::default();
        assert_eq!(lags.upload_offset(100), None);
        lags.push(10_620, 10_000);
        lags.push(10_660, 10_050);
        lags.push(10_700, 10_100);
        assert_eq!(lags.upload_offset(100), Some(100 - 600));
        for tick in 0..LAGS_KEPT as u32 {
            lags.push(20_000 + tick * 50 + 700, 20_000 + tick * 50);
        }
        assert_eq!(lags.upload_offset(100), Some(-600));
    }

    #[test]
    fn the_oldest_captures_go_over_the_cap() {
        let dir = std::env::temp_dir().join(format!("srp-capture-prune-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for (name, age) in [("old", 30), ("mid", 20), ("new", 10)] {
            let folder = dir.join(name);
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join("a.bin"), [0u8; 100]).unwrap();
            let when = std::time::SystemTime::now() - Duration::from_secs(age);
            fs::File::open(&folder).unwrap().set_modified(when).unwrap();
        }
        prune(&dir, 250);
        let mut left: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(left, ["mid", "new"]);
    }
}
