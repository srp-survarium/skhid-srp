//! A refused world is never relayed (the owner's of 2026-11-07: "the server
//! can block, we don't want to pass bad worlds around";
//! `docs/notes/packet-validation.md`, "A refused world is never relayed").
//! A refused upload means asking another game (`Game::sources` asks the one
//! with the fewest refused uploads first), and when no valid world can be
//! had, every game that waits for one gets a defined end, by counts, not by
//! time:
//!
//! - a late joiner whose every possible source gave a world the check
//!   refused, while it waited for it, fails its admission (the lobby is told,
//!   `take_rejected_joins`) and is removed;
//! - a player being resynced (or back in the match) likewise is removed;
//! - a match whose every initial world was refused, with no other game to
//!   give one, is not made (`MatchEnd::NotMade`);
//! - a game whose uploads asked for are refused [`REFUSED_IN_A_ROW`] times
//!   in a row is resynced from another game's world (removed when no other
//!   game's last upload passed), and removed if they are refused as often
//!   again.
//!
//! Every removal goes through [`Game::remove`]: the log line with its
//! [`Removal`], the game's link kicked (0xFD), its place left for the others.

use std::fmt;

use super::*;

/// A game's uploads asked for and refused in a row before it is resynced,
/// and as many again after that before it is removed.
pub(super) const REFUSED_IN_A_ROW: u8 = 3;

/// One round of requests for worlds of the present (0xF6 for a vote): the
/// moment it was asked for. The games asked in one round upload the same
/// present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Round(Instant);

impl Round {
    pub(super) fn at(now: Instant) -> Self {
        Self(now)
    }
}

/// Which of a game's uploads is judged for its standing: its initial world
/// (a game sends one as it loads, on every join), or one asked for as a
/// world of the present in a round (0xF6 for a vote: a world others may
/// start from). A world asked for only to look at (its own, out of step, or
/// a newcomer's) is none: nobody starts from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Judged {
    Initial,
    Asked(Round),
}

/// The world check's verdict on an upload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    Passed,
    Refused,
}

/// A refused upload asked for, not yet known to be its game's own fault
/// or the world every game has: its round and its tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Open {
    round: Round,
    tick: u32,
}

/// Whose a refused upload asked for is. Only its game's own fault counts
/// for its standing: a world every game has (rev50, 2026-11-06: a server
/// bot NaN in every game's simulation, 174 uploads over 185 s refused,
/// the games' hashes agreeing) is refused and relayed to nobody, and no
/// game is resynced or removed for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Blame {
    /// Another game's upload of its round passed, or its game's hash for
    /// its tick disagrees with the majority's.
    Own,
    /// Neither yet: decided by another upload of its round passing (its
    /// own), or by its tick's hashes being judged (`settle_open_refusals`).
    Open(Open),
}

/// How a game's uploads stand with the world check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UploadStanding {
    /// Its last judged upload passed (`round`: an asked one, in it), or it
    /// gave none yet.
    Passing { round: Option<Round> },
    /// Its initial world was refused. A game sends no other: it can no
    /// longer give the match its first world.
    InitialRefused,
    /// Its last judged upload was refused. Of those refused since its last
    /// that passed, `faults` were its own (fewer than [`REFUSED_IN_A_ROW`]);
    /// `open`, its last, is not decided yet.
    Refused { faults: u8, open: Option<Open> },
    /// Resynced for [`REFUSED_IN_A_ROW`] of its own; so many since.
    Resynced { faults: u8, open: Option<Open> },
}

impl Default for UploadStanding {
    fn default() -> Self {
        Self::Passing { round: None }
    }
}

/// What happens to a game's standing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Event {
    /// A judged upload of it passed.
    Passed(Judged),
    /// Its initial world was refused.
    InitialRefused,
    /// An upload of it asked for was refused.
    Refused(Blame),
    /// Its open refusal was found its own fault.
    OpenOwn,
    /// Its open refusal's tick was judged, nothing of it its own: the world
    /// every game has.
    OpenShared,
}

/// What becomes of the game for its upload's verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Then {
    Nothing,
    /// Resynced from another game's world (`Game::resync_refused`).
    Resync,
    Remove,
}

impl UploadStanding {
    /// The standing after an event, and what becomes of its game.
    pub(super) fn next(self, event: Event) -> (Self, Then) {
        use UploadStanding::{InitialRefused, Passing, Refused, Resynced};
        match (event, self) {
            (Event::Passed(Judged::Initial), _) => (Passing { round: None }, Then::Nothing),
            (Event::Passed(Judged::Asked(round)), _) => {
                (Passing { round: Some(round) }, Then::Nothing)
            }
            (Event::InitialRefused, _) => (InitialRefused, Then::Nothing),
            (Event::Refused(blame), Passing { .. } | InitialRefused) => Refused {
                faults: 0,
                open: None,
            }
            .refused(blame),
            (Event::Refused(blame), Refused { .. } | Resynced { .. }) => self.refused(blame),
            (
                Event::OpenOwn,
                Refused {
                    faults,
                    open: Some(_),
                },
            ) => Self::own_fault(faults),
            (
                Event::OpenOwn,
                Resynced {
                    faults,
                    open: Some(_),
                },
            ) => Self::own_fault_resynced(faults),
            (Event::OpenShared, Refused { faults, .. }) => {
                (Refused { faults, open: None }, Then::Nothing)
            }
            (Event::OpenShared, Resynced { faults, .. }) => {
                (Resynced { faults, open: None }, Then::Nothing)
            }
            (
                Event::OpenOwn | Event::OpenShared,
                Passing { .. }
                | InitialRefused
                | Refused { open: None, .. }
                | Resynced { open: None, .. },
            ) => (self, Then::Nothing),
        }
    }

    /// A refusal in `Refused` or `Resynced`: counted if its own, else kept
    /// open (an older open one is let go: its round is over).
    fn refused(self, blame: Blame) -> (Self, Then) {
        match (self, blame) {
            (Self::Refused { faults, .. }, Blame::Own) => Self::own_fault(faults),
            (Self::Resynced { faults, .. }, Blame::Own) => Self::own_fault_resynced(faults),
            (Self::Refused { faults, .. }, Blame::Open(open)) => (
                Self::Refused {
                    faults,
                    open: Some(open),
                },
                Then::Nothing,
            ),
            (Self::Resynced { faults, .. }, Blame::Open(open)) => (
                Self::Resynced {
                    faults,
                    open: Some(open),
                },
                Then::Nothing,
            ),
            (Self::Passing { .. } | Self::InitialRefused, _) => (self, Then::Nothing),
        }
    }

    fn own_fault(faults: u8) -> (Self, Then) {
        let faults = faults + 1;
        if faults >= REFUSED_IN_A_ROW {
            (
                Self::Resynced {
                    faults: 0,
                    open: None,
                },
                Then::Resync,
            )
        } else {
            (Self::Refused { faults, open: None }, Then::Nothing)
        }
    }

    fn own_fault_resynced(faults: u8) -> (Self, Then) {
        let faults = faults + 1;
        let then = if faults >= REFUSED_IN_A_ROW {
            Then::Remove
        } else {
            Then::Nothing
        };
        (Self::Resynced { faults, open: None }, then)
    }

    pub(super) fn passing(self) -> bool {
        matches!(self, Self::Passing { .. })
    }

    /// Its last judged upload passed, an asked one in `round`.
    fn passed_in(self, round: Round) -> bool {
        self == Self::Passing { round: Some(round) }
    }

    /// Its refusal not decided yet.
    fn open(self) -> Option<Open> {
        match self {
            Self::Refused { open, .. } | Self::Resynced { open, .. } => open,
            Self::Passing { .. } | Self::InitialRefused => None,
        }
    }
}

/// The places whose uploads the check refused while a game waited for a
/// world (`Stage::WaitingSnapshot`), less those whose upload passed since:
/// a bit a place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Refusers(u32);

impl Refusers {
    fn bit(place: usize) -> u32 {
        u32::try_from(place)
            .ok()
            .and_then(|place| 1u32.checked_shl(place))
            .unwrap_or(0)
    }

    fn with(self, place: usize) -> Self {
        Self(self.0 | Self::bit(place))
    }

    fn without(self, place: usize) -> Self {
        Self(self.0 & !Self::bit(place))
    }

    /// Every one of `places`, and there is one.
    fn all_of(self, places: &[usize]) -> bool {
        !places.is_empty()
            && places
                .iter()
                .all(|&place| Self::bit(place) != 0 && self.0 & Self::bit(place) != 0)
    }
}

/// What a human place is to the match's first world, before it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FirstWorld {
    /// Its game gave one, and the check refused it.
    Refused,
    /// Its game may still give one.
    MayGive,
    /// It cannot: given up, or its game not there (or loaded with none)
    /// past the server's bound for games that do not come.
    Cannot,
}

/// Why the match server takes a game out of its match ([`Game::remove`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Removal {
    /// A late joiner: every game it could take its first world from gave
    /// one the check refused while it waited.
    NoWorldToJoin,
    /// Being resynced, or back in the match: likewise.
    NoWorldToResync,
    /// Its uploads asked for were refused again and again, after a resync
    /// for that (or with no other game's world to resync it from).
    UploadsRefused,
    /// Invalid inputs in a stretch (`input_safety`).
    InvalidInputs,
    /// Restores in a row that never brought it to agree
    /// (`SRP_RESTORE_LIMIT`).
    RestoresUnagreed,
}

impl fmt::Display for Removal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoWorldToJoin => {
                "no world to join from: every game it could take one from gave a world the \
                 check refused"
            }
            Self::NoWorldToResync => {
                "no world to resync from: every game it could take one from gave a world the \
                 check refused"
            }
            Self::UploadsRefused => "its uploads were refused again and again",
            Self::InvalidInputs => "it repeatedly sent invalid inputs",
            Self::RestoresUnagreed => "restores did not bring it to agree",
        })
    }
}

impl Game {
    /// The one way a game is taken out of its match: said with its reason;
    /// a late joiner that has not entered fails its admission (the lobby
    /// hears it, `take_rejected_joins`); its link is kicked (0xFD), and the
    /// others see it leave. With exe patches 119 (a game that played) and
    /// 120 (one that never played: loading, or loaded before its first
    /// world) its game goes back to the lobby as from a lost connection; its
    /// seat is kept, as for one (Play sends it back), but a failed joiner's.
    pub(super) fn remove(&mut self, index: usize, why: Removal, now: Instant) {
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        if peer.stage == Stage::Gone {
            return;
        }
        log::warn!("Match {:#x}: player {index} removed: {why}", self.id);
        peer.transport.kick(now);
        self.fail_admission(index);
        self.record_note(
            serde_json::json!({ "note": "removed", "place": index, "why": why.to_string() }),
            now,
        );
        self.leave(index, now);
    }

    /// An upload of the game at `index` (for match ms `time`) was judged:
    /// its standing and what that brings, and the waiting games' refusers.
    pub(super) fn judge_upload(
        &mut self,
        index: usize,
        upload: Judged,
        verdict: Verdict,
        time: u32,
        now: Instant,
    ) {
        // Only a game that can take a world now, and this one, counts it
        // (a newcomer needs one past its preparation).
        let waits: Vec<bool> = (0..self.peers.len())
            .map(|other| {
                other != index && self.join_ready(other) && self.after_preparation(other, time)
            })
            .collect();
        for (peer, waits) in self.peers.iter_mut().zip(waits) {
            if let Some(peer) = peer
                && let Stage::WaitingSnapshot { refused, .. } = &mut peer.stage
            {
                *refused = match verdict {
                    Verdict::Passed => refused.without(index),
                    Verdict::Refused if waits => refused.with(index),
                    Verdict::Refused => *refused,
                };
            }
        }
        let Some(peer) = self.peers[index].as_ref() else {
            return;
        };
        let event = match (verdict, upload) {
            (Verdict::Passed, upload) => Event::Passed(upload),
            (Verdict::Refused, Judged::Initial) => Event::InitialRefused,
            (Verdict::Refused, Judged::Asked(round)) => {
                let another_passed = (0..self.peers.len()).any(|other| {
                    other != index
                        && self.peers[other]
                            .as_ref()
                            .is_some_and(|other| other.upload_standing.passed_in(round))
                });
                Event::Refused(if another_passed || self.hash_disagrees(index, time) {
                    Blame::Own
                } else {
                    Blame::Open(Open { round, tick: time })
                })
            }
        };
        let standing = peer.upload_standing;
        self.standing_event(index, standing, event, now);
        // An upload of the round passed: the others' refused in it were
        // their own.
        if let (Verdict::Passed, Judged::Asked(round)) = (verdict, upload) {
            for other in 0..self.peers.len() {
                if let Some(standing) = self.peers[other].as_ref().map(|peer| peer.upload_standing)
                    && standing.open().is_some_and(|open| open.round == round)
                {
                    self.standing_event(other, standing, Event::OpenOwn, now);
                }
            }
        }
    }

    /// The game at `index`'s standing `standing` meets `event`: its new
    /// standing, and what that brings.
    fn standing_event(
        &mut self,
        index: usize,
        standing: UploadStanding,
        event: Event,
        now: Instant,
    ) {
        let (standing, then) = standing.next(event);
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        peer.upload_standing = standing;
        match then {
            Then::Nothing => {}
            Then::Resync => self.resync_refused(index, now),
            Then::Remove => self.remove(index, Removal::UploadsRefused, now),
        }
    }

    /// The game at `index` reported a hash for the tick at `time` that the
    /// majority's is not: its simulation is out of step there.
    fn hash_disagrees(&self, index: usize, time: u32) -> bool {
        let reported = self
            .hashes
            .get(&time)
            .and_then(|reports| reports.get(&index).copied());
        reported.is_some_and(|crc| self.majority(time).is_some_and(|majority| majority != crc))
    }

    /// The open refusals whose ticks' hashes have been judged
    /// (`check_hashes`) are decided: its game's own fault if its hash for
    /// the tick disagreed with the majority's, else the world every game
    /// has, which counts for nobody.
    pub(super) fn settle_open_refusals(&mut self, now: Instant) {
        for index in 0..self.peers.len() {
            let Some(standing) = self.peers[index].as_ref().map(|peer| peer.upload_standing) else {
                continue;
            };
            let Some(open) = standing
                .open()
                .filter(|open| open.tick < self.hashes_checked_until)
            else {
                continue;
            };
            let event = if self.hash_disagrees(index, open.tick) {
                Event::OpenOwn
            } else {
                log::info!(
                    "Match {:#x}: player {index}'s upload at {} ms was refused, but no game's \
                     passed in its round and its hash agrees with the others': the world \
                     every game has; it counts against nobody",
                    self.id,
                    open.tick
                );
                Event::OpenShared
            };
            self.standing_event(index, standing, event, now);
        }
    }

    /// The game of place `index` is new (`attach`): what its place's last
    /// game uploaded no longer stands against it for the games that wait.
    pub(super) fn clear_refuser(&mut self, index: usize) {
        for peer in self.peers.iter_mut().flatten() {
            if let Stage::WaitingSnapshot { refused, .. } = &mut peer.stage {
                *refused = refused.without(index);
            }
        }
    }

    /// [`REFUSED_IN_A_ROW`] uploads of the game refused: it is started anew
    /// from another game's world, if another game whose last upload passed
    /// can give one; else it is removed.
    fn resync_refused(&mut self, index: usize, now: Instant) {
        let good = self.possible_sources(index).into_iter().any(|source| {
            self.peers[source]
                .as_ref()
                .is_some_and(|peer| peer.upload_standing.passing())
        });
        if !good {
            self.remove(index, Removal::UploadsRefused, now);
            return;
        }
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        log::warn!(
            "Match {:#x}: player {index}'s last {REFUSED_IN_A_ROW} uploads were refused; it is \
             resynced from another game's world",
            self.id
        );
        peer.last_resync = Some(now);
        peer.resyncs += 1;
        self.start_sync(index, false, now);
    }

    /// The game at `index` waits for a world, and every game it could take
    /// one from gave a world the check refused since it began to: none can
    /// be had.
    pub(super) fn no_world_for(&self, index: usize, refused: Refusers) -> Option<Removal> {
        if !refused.all_of(&self.possible_sources(index)) {
            return None;
        }
        Some(if self.coming(index) {
            Removal::NoWorldToJoin
        } else {
            Removal::NoWorldToResync
        })
    }

    /// What the human place at `index` is to the match's first world,
    /// before the match has one. A game of it that is not loading (none
    /// connected, gone, or loaded with no world yet: an honest game's
    /// 0xDA can come before its first chunk, a chunk can be lost) may
    /// still give one until the server's bound for games that do not come
    /// (`Timing::unstarted`, from the match's first connection) has passed.
    fn first_world_of(&self, index: usize, now: Instant) -> FirstWorld {
        if self.given_up.get(index).copied().unwrap_or(false) {
            return FirstWorld::Cannot;
        }
        let peer = self.peers[index].as_ref();
        if peer.is_some_and(|peer| peer.upload_standing == UploadStanding::InitialRefused) {
            return FirstWorld::Refused;
        }
        let loading = peer.is_some_and(|peer| peer.stage != Stage::Gone && !peer.resources_loaded);
        let waited = now.saturating_duration_since(self.created) >= self.settings.timing.unstarted;
        if loading || !waited {
            FirstWorld::MayGive
        } else {
            FirstWorld::Cannot
        }
    }

    /// The match has no world yet, and no game can give it its first one:
    /// an initial world was refused, and every other human place gave one
    /// refused too or cannot give one ([`Self::first_world_of`]). The
    /// server's own bots never give one.
    fn initial_worlds_refused(&self, now: Instant) -> bool {
        let places: Vec<FirstWorld> = (0..self.players())
            .filter(|&index| !self.is_bot(index))
            .map(|index| self.first_world_of(index, now))
            .collect();
        places.contains(&FirstWorld::Refused) && !places.contains(&FirstWorld::MayGive)
    }

    /// A match no game can give its first world is not made: its players
    /// are told the end (0xEA of their own place, `finish`), nothing of it
    /// counts, and a room votes and waits again.
    pub(super) fn check_initial_worlds(&mut self, now: Instant) {
        if self.snapshot.is_some()
            || self.finished_at.is_some()
            || !self.initial_worlds_refused(now)
        {
            return;
        }
        log::warn!(
            "Match {:#x}: every game that could give the match its first world gave one the \
             check refused; the match is not made",
            self.id
        );
        self.finish(MatchEnd::NotMade, now);
    }

    /// The match has its world, and every game it could take a new one
    /// from had its last judged upload refused: a joiner would get none
    /// (`Closed::NoWorld`) until one passes.
    pub(super) fn no_world_to_give(&self) -> bool {
        let sources = self.possible_sources(usize::MAX);
        self.snapshot.is_some()
            && !sources.is_empty()
            && sources.iter().all(|&source| {
                self.peers[source]
                    .as_ref()
                    .is_some_and(|peer| !peer.upload_standing.passing())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round() -> Round {
        Round::at(Instant::now())
    }

    fn open() -> Blame {
        Blame::Open(Open {
            round: round(),
            tick: 1_000,
        })
    }

    fn run(mut standing: UploadStanding, events: &[Event]) -> (UploadStanding, Vec<Then>) {
        let mut thens = Vec::new();
        for event in events {
            let (next, then) = standing.next(*event);
            standing = next;
            thens.push(then);
        }
        (standing, thens)
    }

    /// Its own refusals in a row: resynced at the third, removed at the
    /// third after that, whether each was known its own at once or found
    /// so later; one that passes clears the count; a refused initial world
    /// counts apart.
    #[test]
    fn own_refusals_in_a_row_resync_then_remove_their_game() {
        let own = Event::Refused(Blame::Own);
        let (_, thens) = run(
            UploadStanding::default(),
            &[own; 2 * REFUSED_IN_A_ROW as usize],
        );
        let mut expected = vec![Then::Nothing; 2 * usize::from(REFUSED_IN_A_ROW)];
        expected[usize::from(REFUSED_IN_A_ROW) - 1] = Then::Resync;
        expected[2 * usize::from(REFUSED_IN_A_ROW) - 1] = Then::Remove;
        assert_eq!(thens, expected);

        let found_own = [Event::Refused(open()), Event::OpenOwn];
        let events: Vec<Event> = found_own.repeat(usize::from(REFUSED_IN_A_ROW));
        let (standing, thens) = run(UploadStanding::default(), &events);
        assert_eq!(thens.last(), Some(&Then::Resync));
        assert_eq!(
            standing,
            UploadStanding::Resynced {
                faults: 0,
                open: None
            }
        );

        let (standing, _) = run(
            UploadStanding::Refused {
                faults: 2,
                open: None,
            },
            &[Event::Passed(Judged::Asked(round()))],
        );
        assert!(standing.passing());
        let (standing, then) = standing.next(Event::InitialRefused);
        assert_eq!(
            (standing, then),
            (UploadStanding::InitialRefused, Then::Nothing)
        );
        let (standing, then) = standing.next(Event::Refused(Blame::Own));
        assert_eq!(
            (standing, then),
            (
                UploadStanding::Refused {
                    faults: 1,
                    open: None
                },
                Then::Nothing
            )
        );
    }

    /// Refusals never found its own (the world every game has: no other
    /// game's passed in their round, its hashes agreeing) count for
    /// nothing, however many; yet its last judged upload is refused, so it
    /// is no source a joiner could take a world from.
    #[test]
    fn refusals_of_the_world_every_game_has_count_for_nobody() {
        let shared = [Event::Refused(open()), Event::OpenShared];
        let (standing, thens) = run(UploadStanding::default(), &shared.repeat(100));
        assert!(thens.iter().all(|then| *then == Then::Nothing));
        assert_eq!(
            standing,
            UploadStanding::Refused {
                faults: 0,
                open: None
            }
        );
        assert!(!standing.passing());
        // Decided once: a second verdict on it is nothing.
        let (after, then) = standing.next(Event::OpenOwn);
        assert_eq!((after, then), (standing, Then::Nothing));
    }

    #[test]
    fn refusers_name_every_source_or_none() {
        let refused = Refusers::default().with(3).with(5);
        assert!(refused.all_of(&[3, 5]));
        assert!(!refused.all_of(&[3, 5, 7]));
        assert!(!refused.all_of(&[]));
        assert!(!refused.without(5).all_of(&[3, 5]));
    }
}
