//! Watching a replay in the running game, asked from srp's changed lobby
//! movie (History's Watch, the results screen, the Watch window): a replay
//! session (`session::v026g::replay_watch`, `docs/notes/replay-stream.md`).
//!
//! - The ask: `3D 00 <REPLAY_WATCH_BASE | place << 16 | seconds> <match>`
//!   (`victory_screen_request_player_data` with a player no account has),
//!   to watch the match in this game from that time (0: the first view
//!   with a player in it) with that place in view first (0xFF: anyone).
//!   Answered at once with one `R1` row
//!   (`lobby::replay_answer`).
//! - The polls: `3B 00 <REPLAY_START> <count>`, every second while the
//!   prompt waits, each answered with the asker's own session's `R1` row:
//!   `preparing` while the match server reads the recording, `watching`
//!   with the match time it starts at (ms) once its game is sent, or why
//!   not (`not_yours` under `SRP_REPLAY_WATCH=match`, `incompatible`,
//!   `gone`, `failed`, `busy`, `in_room`, `old_game`, `off`). An account
//!   that looked up too many matches with no recording gets `busy` for
//!   every match for a while (`replay_watch::MISSES_ALLOWED`, shared with
//!   Save): a match's id is all that keeps a private room's replay to those
//!   who were given it. `in_room`: the asker's room has a use for it now
//!   (its vote, a place in its match: `rooms`' `room_keeps`).
//! - The withdrawal: `3D 00 <REPLAY_WITHDRAW> <match>`, when the movie's
//!   prompt is closed or the movie gives up waiting. Not answered: the
//!   asker's session of that match is over unless its game was sent
//!   already (`Watches::withdraw`), so it is never sent later, and the
//!   asker may ask for another once `replay_watch::START_EVERY` has passed
//!   since its last ask (`busy` before: each ask has a recording converted).
//! - A replay refused for its recording's games (`incompatible`) is said by
//!   the movie, in the game's language, from the localization tables: the
//!   prompt's `st_srp_replay_incompatible`, the card's
//!   `st_srp_watch_reason_old`. The server only logs why.
//!
//! The game goes to the match server as a spectator does (`85` "found",
//! then `82` with its session) and is shown the replay there; its player
//! is in none of its room's matches until the session is over. Only the
//! asker's own answer is ever sent (a poll names no account).
//!
//! Save (the Watch window's save icon, the results screen's Save replay;
//! `research/local-replays-design.md` section 1): `3D 00 <SAVE_ASK>
//! <match>` asks for the match's file, `3B 00 <SAVE_START> <count>` polls,
//! each answered with one `R2` row (`lobby::save_answer`): `preparing`
//! (its place in the line) while the match server makes it, `ready` (its
//! size) with the hidden `9E` link just before the row, or why not as a
//! session says it. A row of its own: a save and a session may run at once.

use std::time::Instant;

use session::v026g::replay_watch::{WatchFrom, WatchRefusal, WatchRequest};
use session::v026g::replays::{Answer, Status};
use survarium::v026g::lobby;
use survarium::v026g::lobby::ReplayWatchAsk;

use super::core::{Core, Who};

impl Core<'_> {
    /// `3D 00 <REPLAY_WATCH_BASE | place << 16 | seconds> <match_id>`: the
    /// asker wants to watch that match's replay in this game. Its player
    /// must be free in the lobby (in no match, not queued), and its room, if
    /// it is in one, must have no use for it right now (its vote, a place in
    /// its match: `rooms`' `room_keeps`); a member of a room that waits for
    /// players, or one in a room's queue, may watch, and is called back when
    /// its room can play (`room_recalls`).
    pub(super) fn replay_watch(
        &mut self,
        who: &Who,
        match_id: u32,
        ask: ReplayWatchAsk,
        now: Instant,
    ) {
        let in_lobby = self.player(who.addr).is_some_and(|player| {
            player.in_match.is_none() && player.connect.is_none() && player.queued_since.is_none()
        });
        let room_keeps = self.room_keeps(who.account_id, now);
        let free = in_lobby && !room_keeps;
        // Past the limit of lookups of unknown matches: `busy`, before
        // anything says whether this match exists or is being played.
        let limited = self
            .replays
            .is_some_and(|replays| replays.watches().limited(who.account_id, now));
        // One thing at a time: with a replay session on, `start` says `busy`.
        let in_session = self
            .replays
            .is_some_and(|replays| replays.watches().holds(who.account_id));
        // A match still being played is watched as it is (`live`).
        let live = if free && !limited && !in_session {
            self.live_watch(who, match_id, ask.place, now)
        } else {
            None
        };
        let status = match (live, self.replays) {
            (Some(status), _) => status,
            (None, None) => Status::Off,
            (None, Some(_)) if !in_lobby || limited => Status::Busy,
            (None, Some(_)) if room_keeps => Status::InRoom,
            (None, Some(replays)) => replays.watches().start(
                WatchRequest {
                    account_id: who.account_id,
                    session_id: who.session_id,
                    match_id,
                    from: WatchFrom {
                        ms: u32::from(ask.seconds) * 1000,
                        place: ask.place,
                    },
                    revision: self.store.client_revision(who.session_id),
                },
                now,
            ),
        };
        log::info!(
            "{} asked to watch the replay of match {match_id:#x} ({ask:?}): {}",
            who.name,
            status.word()
        );
        let figure = u64::from(status == Status::Preparing) * 2;
        self.send_made(
            who.addr,
            lobby::replay_answer(status.word(), match_id, figure),
        );
    }

    /// `3D 00 <REPLAY_WITHDRAW> <match_id>`: the asker's movie no longer
    /// asks to watch that match's replay (its prompt closed, or it gave up
    /// waiting). Its session is over unless its game was sent already: it
    /// would otherwise load into the replay later, and hold the account
    /// (`busy`) meanwhile. Not answered.
    pub(super) fn replay_withdraw(&self, who: &Who, match_id: u32) {
        if let Some(replays) = self.replays {
            replays.watches().withdraw(who.account_id, match_id);
        }
    }

    /// The replay sessions' turn: those waiting too long end, and the games
    /// of those read go to the match server (a player no longer free in
    /// the lobby, or gone, gets `busy`).
    pub(super) fn replay_sessions(&mut self, now: Instant) {
        let Some(replays) = self.replays else {
            return;
        };
        let watches = replays.watches();
        watches.expire(now);
        for (request, _) in watches.to_send() {
            let addr = self
                .by_account
                .get(&request.account_id)
                .copied()
                .filter(|&addr| {
                    self.player(addr).is_some_and(|player| {
                        player.session_id == request.session_id
                            && player.in_match.is_none()
                            && player.connect.is_none()
                            && player.queued_since.is_none()
                    })
                });
            match addr.zip(self.host.replay_ticket(request.match_id)) {
                Some((addr, ticket)) => {
                    watches.sent(request.account_id, now);
                    self.send_to_replay(addr, &ticket);
                }
                None => watches.refuse(&request, WatchRefusal::Busy, now),
            }
        }
    }

    /// `3B 00 <REPLAY_START> <count>`: the asker's replay session's
    /// answer; `none` (match 0) without one, `off` with replays off.
    pub(super) fn replay_poll(&mut self, who: &Who) {
        let Some(answer) = self
            .replays
            .and_then(|replays| replays.watches().answer(who.account_id))
        else {
            let on = self
                .replays
                .is_some_and(|replays| replays.watches().settings().is_some());
            let status = if on { Status::None } else { Status::Off };
            self.send_made(who.addr, lobby::replay_answer(status.word(), 0, 0));
            return;
        };
        let row = lobby::replay_answer(
            answer.status.word(),
            answer.match_id,
            u64::from(answer.figure),
        );
        self.send_made(who.addr, row);
    }

    /// `3D 00 <SAVE_ASK> <match_id>`: the asker wants that match's replay
    /// saved to a file (`session::v026g::replays`, Save). Answered at once
    /// as a poll is.
    pub(super) fn save_ask(&mut self, who: &Who, match_id: u32, now: Instant) {
        if let Some(replays) = self.replays {
            // While the lobby keeps the match's report: whether the asker
            // played it. A courtesy, not the line that keeps anything
            // private (only made files are ever served).
            let played = self
                .results
                .find(match_id)
                .map(|report| report.player(who.account_id).is_some());
            let revision = self.store.client_revision(who.session_id);
            let status = replays.save_ask(who.account_id, match_id, revision, played, now);
            log::info!(
                "{} asked to save the replay of match {match_id:#x} (game revision {}): {}",
                who.name,
                revision.map_or_else(|| "unknown".to_owned(), |revision| revision.to_string()),
                status.word()
            );
        }
        self.save_poll(who, now);
    }

    /// `3B 00 <SAVE_START> <count>`: the asker's latest save answer, one
    /// `R2` row. A `ready` one has a `9E` just before it:
    /// `video_link(false, "replay <id> <token> <sha256> <size>")`, which the
    /// game keeps hidden and patch revision 14 hands to the updater when the
    /// movie calls `video_tutorial_link_clicked` (`client_patch.md` 3.27).
    /// The packet log never shows a `9E` (`core`), and a log line six hex
    /// digits of a token at most.
    pub(super) fn save_poll(&mut self, who: &Who, now: Instant) {
        let answer = match self.replays {
            Some(replays) => replays.save_answer(who.account_id, now),
            None => Answer {
                status: Status::Off,
                match_id: 0,
                place: 0,
                ready: None,
            },
        };
        if let Some(link) = answer.link() {
            self.send(who.addr, lobby::video_link(false, &link));
        }
        let row = lobby::save_answer(answer.status.word(), answer.match_id, answer.figure());
        self.send_made(who.addr, row);
    }
}
