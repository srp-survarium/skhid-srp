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
//!   language ([`refusal_text`]): recorded before a game update, which no
//!   game plays yet; or recorded by a newer game, with its revisions, and
//!   to update.
//!
//! Only the asker's own answer is ever sent (a poll names no account), and
//! the token is never logged whole (its first six hex digits are; the
//! packet log redacts every `9E`, `core`).

use std::time::Instant;

use session::v026g::replays::{Answer, Known, Refusal, Status};
use survarium::v026g::lobby;
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
