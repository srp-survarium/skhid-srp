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
//!   `gone`, `failed`, `busy`, `old_game`, `off`).
//! - A replay refused for its recording's games (`incompatible`: older
//!   than the last change of what a match computes, of no known revision,
//!   or newer than the asker's game): the first answer also opens the
//!   game's message box (`84 49 00 <text>`, `social.md` §9.1) saying why in
//!   the game's language ([`refusal_text`]): recorded before a game update,
//!   which no game plays yet; or recorded by a newer game, with its
//!   revisions, and to update.
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

use session::v026g::replay_watch::{WatchFrom, WatchRequest};
use session::v026g::replays::{Answer, Refusal, Status};
use survarium::v026g::lobby;
use survarium::v026g::lobby::ReplayWatchAsk;
use survarium::v026g::lobby::requests::id;

use super::core::{Core, Who};

impl Core<'_> {
    /// `3D 00 <REPLAY_WATCH_BASE | place << 16 | seconds> <match_id>`: the
    /// asker wants to watch that match's replay in this game. Its player
    /// must be free in the lobby (in no match, not queued).
    pub(super) fn replay_watch(
        &mut self,
        who: &Who,
        match_id: u32,
        ask: ReplayWatchAsk,
        now: Instant,
    ) {
        let free = self.player(who.addr).is_some_and(|player| {
            player.in_match.is_none() && player.connect.is_none() && player.queued_since.is_none()
        });
        // A match still being played is watched as it is (`live`).
        let live = if free {
            self.live_watch(who, match_id, ask.place, now)
        } else {
            None
        };
        let status = match (live, self.replays) {
            (Some(status), _) => status,
            (None, None) => Status::Off,
            (None, Some(_)) if !free => Status::Busy,
            (None, Some(replays)) => replays.watches().start(
                WatchRequest {
                    account_id: who.account_id,
                    session_id: who.session_id,
                    match_id,
                    from: WatchFrom {
                        ms: u32::from(ask.seconds) * 1000,
                        place: ask.place,
                    },
                    revision: self.store.client_revision(who.session_id).unwrap_or(0),
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

    /// The replay sessions' turn: those waiting too long end, and the games
    /// of those read go to the match server (a player no longer free in
    /// the lobby, or gone, gets `busy`). `SRP_DEV_REPLAY_WATCH`: the games
    /// due ask.
    pub(super) fn replay_sessions(&mut self, now: Instant) {
        if let Some(aid) = self.settings.dev_aid {
            let due: Vec<_> = self
                .dev_due
                .iter()
                .filter(|(_, at)| **at <= now)
                .map(|(addr, _)| *addr)
                .collect();
            for addr in due {
                self.dev_due.remove(&addr);
                let Some(who) = self.who(addr) else {
                    continue;
                };
                match aid {
                    DevAid::Watch { match_id, ask } => {
                        log::warn!(
                            "SRP_DEV_REPLAY_WATCH: {} asks for match {match_id:#x}",
                            who.name
                        );
                        self.replay_watch(&who, match_id, ask, now);
                    }
                    DevAid::PlaySolo => self.play_solo(&who, now),
                }
            }
        }
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
                None => watches.refuse(&request, Status::Busy, None, now),
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
        if let Some(refusal) = answer.refusal {
            self.tell_refusal(who, answer.match_id, refusal);
        }
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
                refusal: None,
            },
        };
        if let Some(link) = answer.link() {
            self.send(who.addr, lobby::video_link(false, &link));
        }
        let row = lobby::save_answer(answer.status.word(), answer.match_id, answer.figure());
        self.send_made(who.addr, row);
        if let Some(refusal) = answer.refusal {
            self.tell_refusal(who, answer.match_id, refusal);
        }
    }

    /// The game's message box says why a replay is not given.
    fn tell_refusal(&mut self, who: &Who, match_id: u32, refusal: Refusal) {
        let language = self.player(who.addr).and_then(|player| player.language);
        log::info!(
            "{} is told why the replay of match {match_id:#x} is not given: {refusal:?}",
            who.name
        );
        // 0x49: the message box and nothing else, whatever the client is
        // doing.
        self.deny(who.addr, id::PROFILE, &refusal_text(refusal, language));
    }
}

/// A dev aid, for tests that cannot click the lobby: done for each game
/// ten seconds after it signs in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DevAid {
    /// `SRP_DEV_REPLAY_WATCH=<match hex>[,<seconds>[,<place>]]`: it asks to
    /// watch that replay, as the movie would.
    Watch { match_id: u32, ask: ReplayWatchAsk },
    /// `SRP_DEV_PLAY_SOLO=1`: Play is pressed for it, and it is seated alone
    /// in a match of its own (played and recorded as any; it ends when the
    /// game leaves it or closes): a recording made with the game being
    /// tested (`play::play_solo`).
    PlaySolo,
}

impl DevAid {
    pub(super) fn from_env() -> Option<Self> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        if var("SRP_DEV_PLAY_SOLO").is_some() {
            return Some(Self::PlaySolo);
        }
        let (match_id, ask) = dev_watch(&var("SRP_DEV_REPLAY_WATCH")?)?;
        Some(Self::Watch { match_id, ask })
    }
}

/// `SRP_DEV_REPLAY_WATCH`: `<match hex>[,<seconds>[,<place>]]`.
fn dev_watch(value: &str) -> Option<(u32, ReplayWatchAsk)> {
    let mut parts = value.trim().split(',').map(str::trim);
    let match_id = u32::from_str_radix(parts.next()?.trim_start_matches("0x"), 16).ok()?;
    let ask = ReplayWatchAsk {
        seconds: parts.next().map_or(Some(0), |s| s.parse().ok())?,
        place: match parts.next() {
            Some(place) => Some(place.parse().ok()?),
            None => None,
        },
    };
    Some((match_id, ask))
}

/// `a` or `a-b`: the revisions of a recording's games.
fn span(from: u8, to: u8) -> String {
    if from == to {
        from.to_string()
    } else {
        format!("{from}-{to}")
    }
}

/// What the message box says of a refused replay, in the game's language
/// (`4F`'s byte: 0 English, 1 Russian, 2 Ukrainian, 3 German, 4 Polish;
/// English for any other, or none yet): at most 255 bytes (`84`'s string).
pub(super) fn refusal_text(refusal: Refusal, language: Option<u8>) -> String {
    let language = language.filter(|&n| n < 5).unwrap_or(0);
    match refusal {
        // No game a player has plays it (nothing installs an older one).
        // No date of the update: the server has none
        // (`simulation_changes.tsv` keeps revisions only).
        Refusal::Older { .. } | Refusal::Unknown => match language {
            1 => "Этот повтор записан до обновления игры; повторы из старых версий пока нельзя воспроизвести.".into(),
            2 => "Цей повтор записано до оновлення гри; повтори зі старих версій поки не можна відтворити.".into(),
            3 => "Diese Wiederholung wurde vor einem Spielupdate aufgenommen; Wiederholungen älterer Versionen lassen sich noch nicht abspielen.".into(),
            4 => "Ta powtórka została nagrana przed aktualizacją gry; powtórek ze starszych wersji nie można jeszcze odtworzyć.".into(),
            _ => "This replay was recorded before a game update; replays from older versions can't be played yet.".into(),
        },
        Refusal::Newer { from, to } => {
            let revisions = span(from, to);
            match language {
                1 => format!("Этот повтор записан более новой версией игры (ревизия {revisions}). Обновите игру: запустите её снова через лаунчер."),
                2 => format!("Цей повтор записано новішою версією гри (ревізія {revisions}). Оновіть гру: запустіть її знову через лаунчер."),
                3 => format!("Diese Wiederholung wurde mit einer neueren Spielversion (Revision {revisions}) aufgenommen. Aktualisiere das Spiel: Starte es erneut über den Launcher."),
                4 => format!("Ta powtórka została nagrana nowszą wersją gry (rewizja {revisions}). Zaktualizuj grę: uruchom ją ponownie przez launcher."),
                _ => format!("This replay was recorded with a newer version of the game (revision {revisions}). Update the game: start it again through the launcher."),
            }
        }
    }
}
