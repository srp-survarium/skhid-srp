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

/// Which of a game's uploads is judged for its standing: its initial world
/// (a game sends one as it loads, on every join), or one asked for as a
/// world of the present (0xF6 for a vote: a world others may start from).
/// A world asked for only to look at (its own, out of step, or a newcomer's)
/// is none: nobody starts from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Judged {
    Initial,
    Asked,
}

/// The world check's verdict on an upload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    Passed,
    Refused,
}

/// How a game's uploads stand with the world check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum UploadStanding {
    /// Its last judged upload passed, or it gave none yet.
    #[default]
    Passing,
    /// Its initial world was refused. A game sends no other: it can no
    /// longer give the match its first world.
    InitialRefused,
    /// So many of its uploads asked for were refused since its last that
    /// passed, fewer than [`REFUSED_IN_A_ROW`].
    Refused { in_a_row: u8 },
    /// Resynced for [`REFUSED_IN_A_ROW`] of them; so many refused since.
    Resynced { in_a_row: u8 },
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
    /// The standing after a judged upload, and what becomes of its game.
    pub(super) fn judged(self, upload: Judged, verdict: Verdict) -> (Self, Then) {
        match (verdict, upload, self) {
            (Verdict::Passed, _, _) => (Self::Passing, Then::Nothing),
            (Verdict::Refused, Judged::Initial, _) => (Self::InitialRefused, Then::Nothing),
            (Verdict::Refused, Judged::Asked, Self::Passing | Self::InitialRefused) => {
                Self::refused_after(0)
            }
            (Verdict::Refused, Judged::Asked, Self::Refused { in_a_row }) => {
                Self::refused_after(in_a_row)
            }
            (Verdict::Refused, Judged::Asked, Self::Resynced { in_a_row }) => {
                let in_a_row = in_a_row + 1;
                let then = if in_a_row >= REFUSED_IN_A_ROW {
                    Then::Remove
                } else {
                    Then::Nothing
                };
                (Self::Resynced { in_a_row }, then)
            }
        }
    }

    fn refused_after(in_a_row: u8) -> (Self, Then) {
        let in_a_row = in_a_row + 1;
        if in_a_row >= REFUSED_IN_A_ROW {
            (Self::Resynced { in_a_row: 0 }, Then::Resync)
        } else {
            (Self::Refused { in_a_row }, Then::Nothing)
        }
    }

    pub(super) fn passing(self) -> bool {
        self == Self::Passing
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
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        let (standing, then) = peer.upload_standing.judged(upload, verdict);
        peer.upload_standing = standing;
        match then {
            Then::Nothing => {}
            Then::Resync => self.resync_refused(index, now),
            Then::Remove => self.remove(index, Removal::UploadsRefused, now),
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

    /// The match has no world yet, and no game can give it its first one:
    /// an initial world was refused, and every other human place's game
    /// either gave one refused too, has gone, was given up, or is a stock
    /// game (loaded with no upload). A place whose game has not connected
    /// may still come with one.
    fn initial_worlds_refused(&self) -> bool {
        let mut refused = false;
        for index in 0..self.players() {
            if self.is_bot(index) || self.given_up.get(index).copied().unwrap_or(false) {
                continue;
            }
            let Some(peer) = self.peers[index].as_ref() else {
                return false;
            };
            if peer.upload_standing == UploadStanding::InitialRefused {
                refused = true;
            } else if peer.stage != Stage::Gone && (peer.patched || !peer.resources_loaded) {
                return false;
            }
        }
        refused
    }

    /// A match no game can give its first world is not made: its players
    /// are told the end (0xEA of their own place, `finish`), nothing of it
    /// counts, and a room votes and waits again.
    pub(super) fn check_initial_worlds(&mut self, now: Instant) {
        if self.snapshot.is_some() || self.finished_at.is_some() || !self.initial_worlds_refused() {
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

    /// Refused in a row: resynced at the third, removed at the third after
    /// that; one that passes clears the count; a refused initial world
    /// counts apart.
    #[test]
    fn uploads_refused_in_a_row_resync_then_remove_their_game() {
        let mut standing = UploadStanding::default();
        let mut thens = Vec::new();
        for _ in 0..2 * REFUSED_IN_A_ROW {
            let (next, then) = standing.judged(Judged::Asked, Verdict::Refused);
            standing = next;
            thens.push(then);
        }
        let mut expected = vec![Then::Nothing; 2 * usize::from(REFUSED_IN_A_ROW)];
        expected[usize::from(REFUSED_IN_A_ROW) - 1] = Then::Resync;
        expected[2 * usize::from(REFUSED_IN_A_ROW) - 1] = Then::Remove;
        assert_eq!(thens, expected);

        let (standing, _) =
            UploadStanding::Refused { in_a_row: 2 }.judged(Judged::Asked, Verdict::Passed);
        assert_eq!(standing, UploadStanding::Passing);
        let (standing, then) = standing.judged(Judged::Initial, Verdict::Refused);
        assert_eq!(
            (standing, then),
            (UploadStanding::InitialRefused, Then::Nothing)
        );
        let (standing, then) = standing.judged(Judged::Asked, Verdict::Refused);
        assert_eq!(
            (standing, then),
            (UploadStanding::Refused { in_a_row: 1 }, Then::Nothing)
        );
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
