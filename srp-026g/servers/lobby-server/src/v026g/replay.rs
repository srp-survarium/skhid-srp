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
//! - The replay plays in a second game; the first stays in the lobby, in
//!   its room. A player given a link is in no match its room starts, nor
//!   put into one late (`rooms::room_free`), until it acts in the lobby
//!   again (a room action or Play, not the movie's own polls) or
//!   [`WATCH_HOLD`] is over. Live, 2026-10-19: with the room's next match
//!   after the last, the lobby game was pulled into a match while its
//!   player watched. Nothing tells the server that the link was clicked
//!   or that the replay ended.
//!
//! Only the asker's own answer is ever sent (a poll names no account), and
//! the token is never logged whole (its first six hex digits are; the
//! packet log redacts every `9E`, `core`).

use std::time::{Duration, Instant};

use session::v026g::replays::{Answer, Known, Refusal, Status, release_tag};
use survarium::v026g::lobby;
use survarium::v026g::lobby::requests::id;

use super::core::{Core, Who};

/// How long a link keeps its player out of its room's matches when it
/// does nothing in the lobby: a long match's replay (15 minutes) and the
/// download and load before it.
pub(super) const WATCH_HOLD: Duration = Duration::from_secs(20 * 60);

impl Core<'_> {
    /// The player acts in the lobby: its room's matches take it again.
    pub(super) fn replay_back(&mut self, account_id: u32) {
        if self.replay_watchers.remove(&account_id).is_some() {
            log::info!("Account {account_id}: back in the lobby from its replay");
        }
    }

    /// The links whose [`WATCH_HOLD`] is over.
    pub(super) fn replay_holds_over(&mut self, now: Instant) {
        self.replay_watchers.retain(|account_id, until| {
            let held = *until > now;
            if !held {
                log::info!("Account {account_id}: its replay's hold is over");
            }
            held
        });
    }

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

    /// `3B 00 <REPLAY_START> <count>`: the asker's latest answer, with the
    /// link before it when the replay is ready.
    pub(super) fn replay_poll(&mut self, who: &Who, now: Instant) {
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
            if self
                .replay_watchers
                .insert(who.account_id, now + WATCH_HOLD)
                .is_none()
            {
                log::info!("{}: in no room's match while it may watch its replay", who.name);
            }
        }
        // The size when `ready`, the place in the conversion line while
        // `preparing`.
        let row = lobby::replay_answer(answer.status.word(), answer.match_id, answer.figure());
        self.send_made(who.addr, row);
        if let Some(refusal) = answer.refusal {
            let language = self.player(who.addr).and_then(|player| player.language);
            log::info!(
                "{} is told why the replay of match {:#x} is not given: {refusal:?}",
                who.name,
                answer.match_id
            );
            // 0x49: the message box and nothing else, whatever the client
            // is doing.
            self.deny(who.addr, id::PROFILE, &refusal_text(refusal, language));
        }
    }
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
