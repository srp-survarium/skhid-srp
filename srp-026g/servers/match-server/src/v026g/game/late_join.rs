//! Admission for the explicitly selected, experimental growing-roster dialect.
//! Preparation changes no simulation membership. Existing peers keep their
//! clocks, histories and normal input relay while this state machine waits.

use super::*;
use session::matchmaking_policy::bot_gives_way;
use survarium::v026g::match_msgs::late_join::{Prepare, Prepared, Proposal, Status};
use survarium::v026g::match_msgs::{MAX_PLAYERS, id};

const RETRY: Duration = Duration::from_secs(1);
const PREPARE_TIMEOUT: Duration = Duration::from_secs(60);
/// A newcomer whose reports have disagreed with the majority for this long
/// (by their ticks) and this many times, none agreeing, is wrong from its
/// world on: its world is asked for, and it is started once more.
const JOINER_WRONG_MS: u32 = 1_000;
const JOINER_WRONG_REPORTS: u32 = 3;
/// How long its world is waited for before that second start.
const JOINER_UPLOAD_WAIT: Duration = Duration::from_secs(2);
/// A newcomer's report the others have not reported its tick for within
/// this (match ms) is let go: no majority will come.
const JOINER_MAJORITY_WAIT_MS: u32 = 3_000;
/// The second start is not made while a roster change of another place is
/// due within this (match ms): the newcomer would get it from its replay
/// right at its tick.
const JOINER_ROSTER_CLEAR_MS: u32 = 500;
/// The admission's time is longer by this for the second start, once.
const JOINER_EXTRA_TIME: Duration = Duration::from_secs(30);
/// A newcomer whose game has not connected for this long (never since its
/// reservation, or not again after it left) is let go, and the rest of its
/// group is admitted without it.
const JOINER_ABSENT: Duration = Duration::from_secs(20);

pub(super) struct LoadingProfile {
    proposal: Proposal,
    sent: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AdmissionPhase {
    Preparing,
    CatchingUp { snapshot_after: u32 },
    Entered { snapshot_after: u32 },
    Failed,
    Rejected,
}

pub(super) struct Admission {
    since: Instant,
    phase: AdmissionPhase,
    catchup_hashes: VecDeque<(u32, u32)>,
    /// Its reports are judged up to this tick (`check_joiner_worlds`).
    judged_until: u32,
    /// Its reports that differed from the majority, in a row since the
    /// last that agreed: the first one's tick, how many, and the last one
    /// (tick, its CRC, the majority's).
    wrong_from: Option<u32>,
    wrong_reports: u32,
    wrong_last: Option<(u32, u32, u32)>,
    /// When its world was asked for over that, and whether it came.
    upload_asked: Option<Instant>,
    upload_received: bool,
    /// Starts past its first one (one at most).
    extra_syncs: u32,
    /// Added to its time ([`PREPARE_TIMEOUT`]) for that start.
    extra_time: Duration,
    /// Since when its game has not been connected (from the reservation
    /// until it connects; again while it is gone).
    absent_since: Option<Instant>,
    /// It failed by itself (its game never came, left, or did not catch up
    /// in time), not because a game could not install its place: the rest
    /// of its group goes on without it (`poll_roster`), and its place is
    /// still installed by every game, as one whose player left.
    failed_alone: bool,
    /// Caught up, it waits: without the rest of its group the sides would
    /// be too far apart (said once).
    waits_for_balance: bool,
}

impl Admission {
    fn new(now: Instant) -> Self {
        Self {
            since: now,
            phase: AdmissionPhase::Preparing,
            catchup_hashes: VecDeque::new(),
            judged_until: 0,
            wrong_from: None,
            wrong_reports: 0,
            wrong_last: None,
            upload_asked: None,
            upload_received: false,
            extra_syncs: 0,
            extra_time: Duration::ZERO,
            absent_since: Some(now),
            failed_alone: false,
            waits_for_balance: false,
        }
    }

    /// Its place is not installed any more: a game could not install it,
    /// and the places after it can then never be (a roster only grows at
    /// its end). A place whose player failed alone is installed on.
    fn install_stopped(&self) -> bool {
        self.failed() && !self.failed_alone
    }

    /// Its reports from here on start a new count.
    fn forget_wrong(&mut self) {
        self.wrong_from = None;
        self.wrong_reports = 0;
        self.wrong_last = None;
    }
}

impl Admission {
    pub(super) fn entered(&self) -> bool {
        matches!(self.phase, AdmissionPhase::Entered { .. })
    }

    pub(super) fn enter(&mut self) {
        let (AdmissionPhase::CatchingUp { snapshot_after }
        | AdmissionPhase::Entered { snapshot_after }) = self.phase
        else {
            unreachable!("entry requires successful preparation and catch-up");
        };
        self.phase = AdmissionPhase::Entered { snapshot_after };
        self.catchup_hashes.clear();
    }

    fn failed(&self) -> bool {
        matches!(
            self.phase,
            AdmissionPhase::Failed | AdmissionPhase::Rejected
        )
    }
}

impl Game {
    pub(crate) fn growing_roster(&self) -> bool {
        self.formed.growing_roster
    }

    pub(crate) fn roster_size(&self) -> usize {
        self.players()
    }

    pub(super) fn snapshot_players(&self) -> usize {
        if self.growing_roster() {
            MAX_PLAYERS
        } else {
            self.players()
        }
    }

    /// Registry updates only append immutable descriptors. Take the new Arc
    /// before routing a new connection and before each game-loop iteration.
    pub(crate) fn sync_roster(&mut self, formed: Arc<Formed>, now: Instant) {
        if !self.growing_roster() || formed.places.len() <= self.players() {
            return;
        }
        assert_eq!(formed.match_id, self.id);
        assert_eq!(&formed.places[..self.players()], &self.formed.places);
        for place in self.players()..formed.places.len() {
            self.peers.push(None);
            self.best_streaks.push(0);
            self.kit_changes.push(0);
            self.push_driven_place(formed.places[place].team);
            self.admissions.push(Some(Admission::new(now)));
        }
        let before = self.players();
        self.formed = formed;
        let note = serde_json::json!({ "note": "places", "places": self.places_note(before) });
        self.record_note(note, now);
    }

    pub(crate) fn admitting(&self) -> bool {
        self.growing_roster()
            && self.players() < MAX_PLAYERS
            && self.rule.state == rules::state::IN_PROCESS
            && self.mode_end_at.is_none()
            && self.ending.is_none()
            && self.finished_at.is_none()
            && self.live() > 0
            && self.admissions.iter().flatten().all(Admission::entered)
            && self
                .peers
                .iter()
                .flatten()
                .filter(|p| p.stage != Stage::Gone)
                .all(|p| p.resources_loaded && p.installed_roster == self.players())
    }

    pub(crate) fn admission_deadline(&self, now: Instant) -> Option<Instant> {
        if !self.admitting() {
            return None;
        }
        let remaining = Duration::from_millis(u64::from(
            self.rule.time_limit_at()?.saturating_sub(self.clock(now)),
        ));
        let open_for = remaining.checked_sub(session::matchmaking_policy::JOIN_CUTOFF)?;
        if open_for.is_zero() {
            return None;
        }
        now.checked_add(open_for)
    }

    /// Failed newcomers can queue elsewhere. Their descriptors remain
    /// reserved here so partially installed slots are never repurposed.
    pub(crate) fn take_rejected_joins(&mut self) -> Vec<(usize, u32)> {
        let mut rejected = Vec::new();
        for (index, admission) in self.admissions.iter_mut().enumerate() {
            if let Some(admission) = admission
                && admission.phase == AdmissionPhase::Failed
            {
                admission.phase = AdmissionPhase::Rejected;
                rejected.push((index, self.formed.places[index].account_id));
            }
        }
        rejected
    }

    /// Whether a game of the match has a place still to install. Until it
    /// has, its world and the world of a game that has the place differ by
    /// what the place's kits put into the saved world before its player is
    /// let in (a byte for each grenade and trap, patch revision 7; the
    /// games of 5 and 6 differed too), and neither is wrong. A place no
    /// game is asked to install any more (`Admission::install_stopped`) is
    /// not waited for.
    pub(super) fn installing(&self) -> bool {
        self.growing_roster()
            && self.peers.iter().flatten().any(|peer| {
                peer.stage != Stage::Gone
                    && peer.resources_loaded
                    && peer.installed_roster < self.players()
                    && !self.admissions[peer.installed_roster]
                        .as_ref()
                        .is_some_and(Admission::install_stopped)
            })
    }

    pub(super) fn join_ready(&self, index: usize) -> bool {
        let Some(admission) = &self.admissions[index] else {
            return true;
        };
        admission.entered()
            || (matches!(admission.phase, AdmissionPhase::CatchingUp { .. })
                && self.every_game_has(index))
    }

    /// Whether every game of the match has the place at `index`, or loads
    /// it with its map. A game that loads (its 0xE4 went, no 0xDA yet) with
    /// the place in its static info has it once it is loaded, as a newcomer
    /// has the places before its own; one still connecting gets every place
    /// in its static info. Neither holds the place up (live, 2026-10-16/17:
    /// a member of a group on its loading screen for a minute held up the
    /// rest of its group, and a player back in after a time-out the
    /// newcomers, until they were let go). A game that loads with a static
    /// info from before the place is asked to install it once loaded
    /// (`poll_roster`), and holds it up until then.
    fn every_game_has(&self, index: usize) -> bool {
        self.peers.iter().flatten().all(|p| {
            p.stage == Stage::Gone || p.stage == Stage::Connecting || p.installed_roster > index
        })
    }

    pub(super) fn after_preparation(&self, index: usize, time: u32) -> bool {
        self.admissions[index].as_ref().is_none_or(|a| {
            matches!(a.phase,
                AdmissionPhase::CatchingUp { snapshot_after }
                | AdmissionPhase::Entered { snapshot_after } if time >= snapshot_after)
        })
    }

    /// A place taken into the match being played whose player is still
    /// to enter, and has not failed to.
    pub(super) fn coming(&self, index: usize) -> bool {
        self.admissions
            .get(index)
            .and_then(Option::as_ref)
            .is_some_and(|admission| !admission.entered() && !admission.failed())
    }

    pub(super) fn waiting_entry(&self, index: usize) -> bool {
        self.admissions[index]
            .as_ref()
            .is_some_and(|a| !a.entered())
    }

    /// A snapshot's transport ACK alone does not prove application/catch-up.
    /// Keep this newcomer's reports separate from the existing world's votes
    /// until it demonstrates a recent matching simulation state.
    pub(super) fn catchup_hash(&mut self, index: usize, time: u32, crc: u32) {
        let Some(admission) = self.admissions[index].as_mut() else {
            return;
        };
        if !matches!(admission.phase, AdmissionPhase::CatchingUp { .. }) {
            return;
        }
        admission.catchup_hashes.push_back((time, crc));
        while admission.catchup_hashes.len() > 32 {
            admission.catchup_hashes.pop_front();
        }
    }

    pub(super) fn reset_catchup(&mut self, index: usize, new_connection: bool) {
        if let Some(admission) = &mut self.admissions[index]
            && !admission.entered()
            && !admission.failed()
        {
            admission.catchup_hashes.clear();
            admission.forget_wrong();
            if new_connection {
                admission.phase = AdmissionPhase::Preparing;
            }
        }
    }

    pub(super) fn accept_prepared(&mut self, index: usize, message_id: u8, payload: &[u8]) -> bool {
        if !self.growing_roster() || message_id != id::LEAVE {
            return false;
        }
        let Ok(ack) = Prepared::decode(payload) else {
            return false;
        };
        let Some(peer) = self.peers[index].as_mut() else {
            return true;
        };
        if peer.stage == Stage::Gone
            || !peer.resources_loaded
            || !peer
                .preparing
                .as_ref()
                .is_some_and(|p| p.proposal == ack.proposal)
        {
            return true;
        }
        if self.admissions[usize::from(ack.proposal.place)]
            .as_ref()
            .is_some_and(Admission::install_stopped)
        {
            return true;
        }
        match ack.status {
            Status::Installed => {
                log::debug!(
                    "Match {:#x}: player {index} installed place {} (proposal {})",
                    self.id,
                    ack.proposal.place,
                    ack.proposal.serial
                );
                peer.installed_roster = usize::from(ack.proposal.place) + 1;
                peer.preparing = None;
            }
            Status::Busy => {} // retry the same immutable proposal
            Status::Failed => {
                log::warn!(
                    "Match {:#x}: player {index} failed to prepare place {}",
                    self.id,
                    ack.proposal.place
                );
                if let Some(admission) = &mut self.admissions[usize::from(ack.proposal.place)] {
                    admission.phase = AdmissionPhase::Failed;
                }
            }
        }
        true
    }

    pub(super) fn poll_roster(&mut self, now: Instant) {
        if !self.growing_roster()
            || self.mode_end_at.is_some()
            || self.ending.is_some()
            || self.finished_at.is_some()
        {
            return;
        }
        let count = self.players();
        for peer in self.peers.iter_mut().flatten() {
            if peer.stage == Stage::Gone || !peer.resources_loaded || peer.installed_roster >= count
            {
                continue;
            }
            let place = peer.installed_roster;
            if self.admissions[place]
                .as_ref()
                .is_some_and(Admission::install_stopped)
            {
                continue;
            }
            if let Some(pending) = &peer.preparing {
                if now.saturating_duration_since(pending.sent) < RETRY {
                    continue;
                }
            } else {
                // A new connection receives a new serial, so a delayed ready
                // packet from its previous incarnation cannot pass the gate.
                let Some(serial) = self.next_proposal.checked_add(1) else {
                    continue;
                };
                self.next_proposal = serial;
                peer.preparing = Some(LoadingProfile {
                    proposal: Proposal {
                        match_id: super::super::registry::static_id(self.id),
                        serial,
                        place: place as u8,
                    },
                    sent: now,
                });
            }
            let pending = peer.preparing.as_mut().unwrap();
            let prepare = Prepare {
                proposal: pending.proposal,
                profile: self.formed.profiles[place].clone(),
            };
            let bytes = prepare
                .encode(Dictionary::shared())
                .expect("registry validated profile");
            if peer.transport.send(id::STATIC_INFO, &bytes).is_ok() {
                pending.sent = now;
            }
        }
        let clock = self.clock(now);
        // The group's latest deadline: a member started once more has
        // more time (`check_joiner_worlds`), and the members that caught
        // up wait for it that long, not their own (one pending group at a
        // time).
        let group_deadline = self
            .admissions
            .iter()
            .flatten()
            .filter(|a| !a.entered() && !a.failed())
            .map(|a| a.since + PREPARE_TIMEOUT + a.extra_time)
            .max();
        let mut caught_up = Vec::new();
        let mut overdue = Vec::new();
        for index in self.formed.initial_players..count {
            let ready = self.peers[index]
                .as_ref()
                .is_some_and(|p| p.resources_loaded && p.stage != Stage::Gone)
                && self.every_game_has(index);
            let connected = self.peers[index]
                .as_ref()
                .is_some_and(|p| p.stage != Stage::Gone);
            let caught_up_now = self.caught_up(index, ready, clock);
            let admission = self.admissions[index].as_mut().unwrap();
            if admission.entered() {
                continue;
            }
            if connected {
                admission.absent_since = None;
            } else {
                admission.absent_since.get_or_insert(now);
            }
            if !admission.failed()
                && admission
                    .absent_since
                    .is_some_and(|since| now.saturating_duration_since(since) >= JOINER_ABSENT)
            {
                log::warn!(
                    "Match {:#x}: new place {index}'s game has not been connected for {} s; it is \
                     let go, and the rest of its group goes on without it",
                    self.id,
                    JOINER_ABSENT.as_secs()
                );
                admission.phase = AdmissionPhase::Failed;
                admission.failed_alone = true;
            }
            let own_deadline = admission.since + PREPARE_TIMEOUT + admission.extra_time;
            let waits_for_group = caught_up_now || admission.waits_for_balance;
            let deadline = if waits_for_group {
                group_deadline.map_or(own_deadline, |group| group.max(own_deadline))
            } else {
                own_deadline
            };
            if !admission.failed() && now >= deadline && waits_for_group {
                // Its entry check comes first (`enter_group`).
                overdue.push(index);
            } else if !admission.failed() && now >= deadline {
                log::warn!(
                    "Match {:#x}: new place {index} timed out during preparation or catch-up; the \
                     rest of its group goes on without it",
                    self.id
                );
                admission.phase = AdmissionPhase::Failed;
                admission.failed_alone = true;
            }
            if admission.failed() {
                if let Some(peer) = self.peers[index].as_mut()
                    && peer.stage != Stage::Gone
                {
                    peer.transport.disconnect(now);
                    self.leave(index, now);
                }
                continue;
            }
            if ready && admission.phase == AdmissionPhase::Preparing {
                // Require a world captured after the barrier. Old cached
                // snapshots and the newcomer's initial upload cannot qualify.
                admission.phase = AdmissionPhase::CatchingUp {
                    snapshot_after: grid_up(clock),
                };
                log::info!(
                    "Match {:#x}: new place {index} prepared; requesting a world from {} ms or later",
                    self.id,
                    grid_up(clock)
                );
                self.request_upload(index, now, Duration::ZERO);
            }
            if self.caught_up(index, ready, clock) {
                caught_up.push(index);
            }
        }
        self.check_joiner_worlds(&caught_up, now);
        self.enter_group(caught_up, clock, now);
        // A member that caught up, or waits for the balance, and is past
        // its deadline had its entry check above first: when its group
        // shrank (its last slow member failed, by a timeout here or by its
        // game's own word), it may enter now rather than be let go.
        for index in overdue {
            let Some(admission) = self.admissions[index].as_mut() else {
                continue;
            };
            if admission.entered() || admission.failed() {
                continue;
            }
            log::warn!(
                "Match {:#x}: new place {index} timed out during preparation or catch-up; the \
                 rest of its group goes on without it",
                self.id
            );
            admission.phase = AdmissionPhase::Failed;
            admission.failed_alone = true;
            if let Some(peer) = self.peers[index].as_mut()
                && peer.stage != Stage::Gone
            {
                peer.transport.disconnect(now);
                self.leave(index, now);
            }
        }
    }

    /// The members of the pending group that caught up enter together, if
    /// the whole group (less those left out) did.
    fn enter_group(&mut self, caught_up: Vec<usize>, clock: u32, now: Instant) {
        // One reservation group is pending at a time. Its members enter
        // together, on one tick, even when their resources and snapshots
        // finish apart: the lobby placed them together, to even the sides
        // (`matchmaking_policy::place_late`), and a squad is one group. A
        // place no game could install fails them all: the places after it
        // can never be installed. A member that failed alone (its game
        // never came, left, or did not catch up) is left out, and the rest
        // enter without it if the sides are then no further apart than
        // one player, or than before (`entry_keeps_balance`).
        if self
            .admissions
            .iter()
            .flatten()
            .any(Admission::install_stopped)
        {
            for admission in self.admissions.iter_mut().flatten() {
                if !admission.entered() && !admission.failed() {
                    admission.phase = AdmissionPhase::Failed;
                }
            }
            return;
        }
        let waiting: Vec<Instant> = self
            .admissions
            .iter()
            .flatten()
            .filter(|a| !a.entered() && !a.failed())
            .map(|a| a.since)
            .collect();
        // A member of this group left out: it was reserved with them.
        let short = self
            .admissions
            .iter()
            .flatten()
            .any(|a| a.failed_alone && !a.entered() && waiting.contains(&a.since));
        if !waiting.is_empty() && caught_up.len() == waiting.len() {
            if short && let Err(sides) = self.entry_keeps_balance(&caught_up) {
                let admission = self.admissions[caught_up[0]].as_mut().unwrap();
                if !std::mem::replace(&mut admission.waits_for_balance, true) {
                    log::info!(
                        "Match {:#x}: new places {caught_up:?} caught up, but without the rest of \
                         their group the sides would be {} v {}; they wait",
                        self.id,
                        sides[0],
                        sides[1]
                    );
                }
                return;
            }
            let at = grid_up(clock + ROSTER_LEAD_MS);
            for &index in &caught_up {
                log::info!(
                    "Match {:#x}: new place {index} caught up; entry scheduled for {at} ms",
                    self.id
                );
                self.roster_change(None, index, true, at, now);
            }
            // The server's bots make room for them where it is needed
            // (`driven.rs`).
            self.give_way(&caught_up, at, now);
        }
    }

    /// Whether the newcomer at `index` (`ready`: every game installed its
    /// place) plays the others' world: live, with a recent report of a tick
    /// after its preparation that the majority agrees with.
    fn caught_up(&self, index: usize, ready: bool, clock: u32) -> bool {
        ready
            && self.peers[index]
                .as_ref()
                .is_some_and(|p| p.stage == Stage::Live)
            && self.admissions[index].as_ref().is_some_and(|admission| {
                admission.catchup_hashes.iter().any(|&(time, crc)| {
                    time <= clock
                        && clock - time <= FRESH_SNAPSHOT_MS
                        && self.after_preparation(index, time)
                        && self.majority(time) == Some(crc)
                })
            })
    }

    /// Whether `entering` may enter without the members of their group that
    /// were left out: the sides, counted by the players in the match, are
    /// then at most one player apart, or no further apart than now. The
    /// lobby placed the whole group to even the sides; a part of it may
    /// not (two placed one on each side, one of them gone). The sides it
    /// would make when not.
    fn entry_keeps_balance(&self, entering: &[usize]) -> Result<(), [i64; 2]> {
        let mut sides = [0usize; 2];
        for (index, place) in self.formed.places.iter().enumerate() {
            let in_match = !self.waiting_entry(index)
                && self.peers[index]
                    .as_ref()
                    .is_some_and(|peer| peer.stage != Stage::Gone);
            if in_match {
                sides[usize::from(place.team.min(1))] += 1;
            }
        }
        // The server's bots are players of their side too, as the lobby
        // counts them; one whose side needs the room makes it for an
        // entrant, as `give_way` lets it after the entry.
        let staying = self.bots_staying();
        let mut bots = if self.settings.bots_give_way {
            staying
        } else {
            [0, 0]
        };
        for side in 0..2 {
            sides[side] += staying[side];
        }
        let before = sides[0].abs_diff(sides[1]);
        for index in entering {
            let team = usize::from(self.formed.places[*index].team.min(1));
            if bot_gives_way(sides, bots, team) {
                bots[team] -= 1;
            } else {
                sides[team] += 1;
            }
        }
        let after = sides[0].abs_diff(sides[1]);
        if after <= 1 || after <= before {
            Ok(())
        } else {
            Err(sides.map(|side| side as i64))
        }
    }

    /// Newcomers whose world is wrong from their first report on: one in
    /// three of the late joins of 2026-10-14 was, and waited out its
    /// [`PREPARE_TIMEOUT`] without ever being let in. Its reports from
    /// after its preparation are judged against the majority of the
    /// others; once they have disagreed for [`JOINER_WRONG_MS`] (at least
    /// [`JOINER_WRONG_REPORTS`], none agreeing), its world is asked for and
    /// saved to look at (`joiner_uploaded`), and it is started once more,
    /// as `reload` starts one: a second 0xE5/0xE8 to a game catching up is
    /// safe and runs live. Once only, and with [`JOINER_EXTRA_TIME`] more
    /// to catch up in. `caught_up` are the newcomers about to enter.
    fn check_joiner_worlds(&mut self, caught_up: &[usize], now: Instant) {
        let clock = self.clock(now);
        let settle = self
            .settings
            .timing
            .hash_settle_ms
            .max(JOINER_MAJORITY_WAIT_MS);
        for index in self.formed.initial_players..self.players() {
            if caught_up.contains(&index) {
                continue;
            }
            let Some(admission) = self.admissions[index].as_ref() else {
                continue;
            };
            if !matches!(admission.phase, AdmissionPhase::CatchingUp { .. }) {
                continue;
            }
            // Its reports in tick order, each against the majority once
            // that is known, or let go once none will come.
            let mut reports: Vec<(u32, u32)> = admission
                .catchup_hashes
                .iter()
                .filter(|(time, _)| *time > admission.judged_until)
                .copied()
                .collect();
            reports.sort_unstable();
            let mut verdicts = Vec::new();
            for (time, crc) in reports {
                if !self.after_preparation(index, time) {
                    verdicts.push((time, None));
                    continue;
                }
                match self.majority(time) {
                    Some(majority) => verdicts.push((time, Some((crc, majority)))),
                    None if time + settle < clock => verdicts.push((time, None)),
                    None => break,
                }
            }
            let live = self.peers[index]
                .as_ref()
                .is_some_and(|peer| peer.stage == Stage::Live);
            let roster_due = self.events.iter().any(|(at, who, _)| {
                *who != index && *at > clock && *at <= clock + JOINER_ROSTER_CLEAR_MS
            });
            let id = self.id;
            let Some(admission) = self.admissions[index].as_mut() else {
                continue;
            };
            for (time, verdict) in verdicts {
                admission.judged_until = time;
                match verdict {
                    Some((crc, majority)) if crc == majority => admission.forget_wrong(),
                    Some((crc, majority)) => {
                        admission.wrong_from.get_or_insert(time);
                        admission.wrong_reports += 1;
                        admission.wrong_last = Some((time, crc, majority));
                    }
                    None => {}
                }
            }
            let wrong = admission.wrong_reports >= JOINER_WRONG_REPORTS
                && admission
                    .wrong_last
                    .zip(admission.wrong_from)
                    .is_some_and(|((last, ..), first)| last - first >= JOINER_WRONG_MS);
            if !live || !wrong || admission.extra_syncs > 0 {
                continue;
            }
            let Some(asked) = admission.upload_asked else {
                admission.upload_asked = Some(now);
                if let Some((time, crc, majority)) = admission.wrong_last {
                    log::warn!(
                        "Match {id:#x}: new place {index}'s reports disagree with the majority \
                         from {} ms on, {} of them (at {time} ms {crc:08x}, the majority \
                         {majority:08x}); its world is asked for, and it is started once more",
                        admission.wrong_from.unwrap_or(time),
                        admission.wrong_reports
                    );
                }
                self.ask_joiner_world(index, clock, now);
                continue;
            };
            let waited = now.saturating_duration_since(asked);
            if !(admission.upload_received || waited >= JOINER_UPLOAD_WAIT) || roster_due {
                continue;
            }
            // Its reports from here on are of the world it is started
            // from, which is one from now on, as its first was one from its
            // preparation on.
            admission.extra_syncs += 1;
            admission.extra_time = JOINER_EXTRA_TIME;
            admission.forget_wrong();
            if let AdmissionPhase::CatchingUp { snapshot_after } = &mut admission.phase {
                *snapshot_after = (*snapshot_after).max(grid_up(clock));
            }
            log::info!(
                "Match {id:#x}: new place {index} is started once more, from a world from {} ms on \
                 ({}; {} s more to catch up)",
                grid_up(clock),
                if admission.upload_received {
                    "its own world was saved".to_owned()
                } else {
                    format!("its world did not come in {} ms", waited.as_millis())
                },
                JOINER_EXTRA_TIME.as_secs()
            );
            self.start_sync(index, false, now);
        }
    }

    /// The world of the newcomer at `index`, for its tick of now, and the
    /// world of one of the others for about the same tick: both asked for
    /// at once (each uploads its event-horizon world of the moment it
    /// handles the request, client_patch.md section 3.4; the two ticks are
    /// the same one when the two clocks and links are alike, and a tick
    /// apart at most otherwise). The other's goes through the usual check
    /// and is saved by it.
    fn ask_joiner_world(&mut self, index: usize, clock: u32, now: Instant) {
        // Not one asked within the second (a patched client skips that).
        let source = self.sources(index).into_iter().find(|source| {
            self.peers[*source].as_ref().is_some_and(|peer| {
                peer.ask_at.is_none()
                    && peer
                        .asked
                        .is_none_or(|at| now.saturating_duration_since(at) >= RESYNC_REQUEST_GAP)
            })
        });
        for asked in std::iter::once(index).chain(source) {
            if let Some(peer) = self.peers[asked].as_mut() {
                peer.ask_at = Some(clock);
            }
        }
        self.send_requests(now);
    }

    /// The world the newcomer at `index` was asked for over its wrong
    /// reports (`check_joiner_worlds`) came: saved to look at, with what
    /// it and the majority say of its tick. It is no vote and no world to
    /// start anyone from.
    pub(super) fn joiner_uploaded(&mut self, index: usize, snapshot: &Snapshot) {
        let crc = snapshot::crc(&snapshot.body);
        let majority = self.majority(snapshot.time);
        log::warn!(
            "Match {:#x}: new place {index} gave its world at {} ms ({} bytes, CRC {crc:08x}; the \
             majority's for that tick {}); saved as \"joiner\"",
            self.id,
            snapshot.time,
            snapshot.body.len(),
            majority.map_or("unknown".to_owned(), |majority| format!("{majority:08x}"))
        );
        if let Some(admission) = self.admissions[index].as_mut() {
            admission.upload_received = true;
        }
        self.dump(snapshot, index, "joiner");
    }
}
