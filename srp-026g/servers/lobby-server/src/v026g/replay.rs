//! "Watch replay" from the History of srp's changed lobby movie
//! (`docs/notes/replay_watch.md`, `session::v026g::replays`).
//!
//! - The ask: `3D 00 <REPLAY_ASK> <match>` (`victory_screen_request_player_
//!   data` with a player no account has). Not answered at once: the answer
//!   is kept as the asker's latest, for its polls.
//! - The polls: `3B 00 <REPLAY_START> <count>`, every second while the
//!   dialog waits. Each is answered with the asker's own latest answer, one
//!   `91` row `R1;<status>;<match id 8 hex>;<n>` (`lobby::replay_answer`;
//!   `<n>` the size when `ready`, the place in the conversion line while
//!   `preparing`). A `ready` answer has a `9E` just before it:
//!   `video_link(false, "replay <id> <token> <sha256> <size>")`, which the
//!   game keeps hidden and patch revision 14 hands to the updater when the
//!   player presses "Download and watch" (`video_tutorial_link_clicked`).
//!
//! - A replay refused for its recording's games (`incompatible`: older
//!   than the last change of what a match computes, of no known revision,
//!   or newer than the asker's game) is never linked. The movie says "This
//!   replay was recorded by a game version that yours cannot play."; the
//!   first answer of each such ask also opens the game's message box
//!   (`84 49 00 <text>`, `social.md` §9.1) saying why in the game's
//!   language, with the revisions and the release to watch it with
//!   ([`refusal_text`]).
//!
//! - A replay session (`session::v026g::replay_watch`,
//!   `docs/notes/replay-stream.md`): `3D 00 <REPLAY_WATCH_BASE | place << 16
//!   | seconds> <match>` asks to watch the match in this game, from that
//!   time (0: the first view with a player in it) with that place in view
//!   first (0xFF: anyone). Answered at once with one `R1` row, then at each
//!   poll as an ask is: `preparing` while the match server reads the
//!   recording, `watching` with the match time it starts at (ms) once its
//!   game is sent, or why not (`not_yours`, `incompatible` with the box,
//!   `gone`, `failed`, `busy`, `old_game`, `off`). No `9E`, no download.
//!   The game goes to the match server as a spectator does (`85` "found",
//!   then `82` with its session) and is shown the replay there; its player
//!   is in none of its room's matches until the session is over.
//!
//! Only the asker's own answer is ever sent (a poll names no account), and
//! the token is never logged whole (its first six hex digits are; the
//! packet log redacts every `9E`, `core`).

use std::time::Instant;

use session::v026g::replay_watch::{WatchFrom, WatchRequest};
use session::v026g::replays::{Answer, Known, Refusal, Status, release_tag};
use survarium::v026g::lobby;
use survarium::v026g::lobby::ReplayWatchAsk;
use survarium::v026g::lobby::requests::id;

use super::core::{Core, Who};

impl Core<'_> {
    /// `3D 00 <REPLAY_ASK> <match_id>`: the asker wants that match's replay.
    pub(super) fn replay_ask(&mut self, who: &Who, match_id: u32, now: Instant) {
        let Some(replays) = self.replays else {
            log::info!(
                "{} asked for the replay of match {match_id:#x}: replays are off",
                who.name
            );
            return;
        };
        // While the lobby keeps the match's report: whether the asker played
        // it, and its places. A courtesy, not the line that keeps anything
        // private (only replays are ever served).
        let known = self.results.find(match_id).map(|report| Known {
            played: report.player(who.account_id).is_some(),
            places: report.players.len(),
        });
        let revision = self.store.client_revision(who.session_id);
        let status = replays.ask(who.account_id, match_id, revision, known, now);
        log::info!(
            "{} asked for the replay of match {match_id:#x} (game revision {}): {}",
            who.name,
            revision.map_or_else(|| "unknown".to_owned(), |revision| revision.to_string()),
            status.word()
        );
    }

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
        let live = match ask {
            ReplayWatchAsk::At { .. } if free => self.live_watch(who, match_id, now),
            _ => None,
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
                    from: match ask {
                        ReplayWatchAsk::At { seconds, place } => WatchFrom::At {
                            ms: u32::from(seconds) * 1000,
                            place,
                        },
                        ReplayWatchAsk::MyDeaths => WatchFrom::MyDeaths,
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

    /// `3B 00 <REPLAY_START> <count>`: the asker's latest answer, with the
    /// link before it when the replay is ready; a replay session's while
    /// it has one.
    pub(super) fn replay_poll(&mut self, who: &Who, now: Instant) {
        if let Some(answer) = self
            .replays
            .and_then(|replays| replays.watches().answer(who.account_id))
        {
            let row = lobby::replay_answer(
                answer.status.word(),
                answer.match_id,
                u64::from(answer.figure),
            );
            self.send_made(who.addr, row);
            if let Some(refusal) = answer.refusal {
                self.tell_refusal(who, answer.match_id, refusal);
            }
            return;
        }
        let answer = match self.replays {
            Some(replays) => replays.answer(who.account_id, now),
            None => Answer {
                status: Status::Off,
                match_id: 0,
                size: 0,
                place: 0,
                ready: None,
                refusal: None,
            },
        };
        if let Some(link) = answer.link() {
            self.send(who.addr, lobby::video_link(false, &link));
        }
        // The size when `ready`, the place in the conversion line while
        // `preparing`.
        let row = lobby::replay_answer(answer.status.word(), answer.match_id, answer.figure());
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
    /// `SRP_DEV_REPLAY_WATCH=<match hex>[,<seconds>[,<place>]]` or
    /// `<match hex>,deaths`: it asks to watch that replay, as the movie
    /// would.
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

/// `SRP_DEV_REPLAY_WATCH`: `<match hex>[,<seconds>[,<place>]]`, or
/// `<match hex>,deaths` (the asker's deaths).
fn dev_watch(value: &str) -> Option<(u32, ReplayWatchAsk)> {
    let mut parts = value.trim().split(',').map(str::trim);
    let match_id = u32::from_str_radix(parts.next()?.trim_start_matches("0x"), 16).ok()?;
    let ask = match parts.next() {
        Some("deaths") => ReplayWatchAsk::MyDeaths,
        seconds => ReplayWatchAsk::At {
            seconds: seconds.map_or(Some(0), |s| s.parse().ok())?,
            place: match parts.next() {
                Some(place) => Some(place.parse().ok()?),
                None => None,
            },
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
        Refusal::Older { from, to, .. } => {
            let revisions = span(from, to);
            // The release of the newest of them plays them all (a match's
            // games are of one span).
            match (language, release_tag(to)) {
                (1, Some(tag)) => format!("Этот повтор записан старой версией игры (ревизия {revisions}), в которой матчи идут иначе. Смотреть его можно только в том выпуске: {tag}."),
                (1, None) => format!("Этот повтор записан старой версией игры (ревизия {revisions}), в которой матчи идут иначе. Смотреть его можно только в выпуске той ревизии."),
                (2, Some(tag)) => format!("Цей повтор записано старою версією гри (ревізія {revisions}), у якій матчі йдуть інакше. Дивитися його можна лише в тому випуску: {tag}."),
                (2, None) => format!("Цей повтор записано старою версією гри (ревізія {revisions}), у якій матчі йдуть інакше. Дивитися його можна лише у випуску тієї ревізії."),
                (3, Some(tag)) => format!("Diese Wiederholung wurde mit einer älteren Spielversion (Revision {revisions}) aufgenommen, in der Matches anders ablaufen. Ansehen lässt sie sich nur mit diesem Release: {tag}."),
                (3, None) => format!("Diese Wiederholung wurde mit einer älteren Spielversion (Revision {revisions}) aufgenommen, in der Matches anders ablaufen. Ansehen lässt sie sich nur mit dem Release dieser Revision."),
                (4, Some(tag)) => format!("Ta powtórka została nagrana starszą wersją gry (rewizja {revisions}), w której mecze przebiegają inaczej. Obejrzysz ją tylko w tamtym wydaniu: {tag}."),
                (4, None) => format!("Ta powtórka została nagrana starszą wersją gry (rewizja {revisions}), w której mecze przebiegają inaczej. Obejrzysz ją tylko w wydaniu tej rewizji."),
                (_, Some(tag)) => format!("This replay was recorded with an older version of the game (revision {revisions}), which plays matches differently. It can be watched only with that release: {tag}."),
                (_, None) => format!("This replay was recorded with an older version of the game (revision {revisions}), which plays matches differently. It can be watched only with the release of that revision."),
            }
        }
        Refusal::Unknown => match language {
            1 => "Этот повтор записан старой версией игры (её номер тогда не сохранялся). Смотреть его можно только в том старом выпуске игры.".into(),
            2 => "Цей повтор записано старою версією гри (її номер тоді не зберігався). Дивитися його можна лише в тому старому випуску гри.".into(),
            3 => "Diese Wiederholung wurde mit einer älteren Spielversion aufgenommen (ihre Nummer wurde damals nicht gespeichert). Ansehen lässt sie sich nur mit diesem älteren Release.".into(),
            4 => "Ta powtórka została nagrana starszą wersją gry (jej numer nie był wtedy zapisywany). Obejrzysz ją tylko w tamtym starszym wydaniu gry.".into(),
            _ => "This replay was recorded with an older version of the game (its number was not kept then). It can be watched only with that older release of the game.".into(),
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
