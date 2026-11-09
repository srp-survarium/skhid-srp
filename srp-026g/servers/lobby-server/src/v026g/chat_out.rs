//! A player's chat line on its way out: to each reader in the reader's game
//! language when chat translation is on (`translate`,
//! `docs/notes/chat-translation.md`).
//!
//! - **Off** (no translator): every reader is sent the line at once, as
//!   before.
//! - **On:** the line's language is guessed (`translate::detect`); a reader
//!   of that language, of no known language (`4F` not seen), or of a line
//!   with no language ("gg", a number) is sent the original. The others
//!   wait for the translation into theirs, at most the translator's
//!   timeout, then get the original. A translation goes out as
//!   `[ru] <text>`: the reader sees it is one, and from what.
//! - **Each reader gets its lines in the order they were said.** A line
//!   that waits holds back the later lines to that reader (no more than the
//!   timeout), whatever their language.
//! - The server's own lines (sender id 0: "Alice joined the room") are
//!   never translated (the movie puts them in the player's language), but
//!   keep their place in that order.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Instant;

use session::v026g::rooms::ChatLine;
use survarium::v026g::lobby::{self, ServerMessage, limits, sender};

use super::core::Core;
use super::translate::{Lang, Lookup, detect};

/// A line said and not yet sent to every reader.
pub(super) struct Pending {
    said: ChatLine,
    channel: u8,
    /// Its language, as guessed.
    source: Option<Lang>,
    /// When the readers who wait for a translation get the original.
    deadline: Instant,
    /// Those not sent it yet.
    readers: Vec<Reader>,
}

struct Reader {
    addr: SocketAddr,
    wants: Wants,
}

/// What a reader is sent of a line.
#[derive(Clone, Copy)]
enum Wants {
    /// The line as it was said.
    Original,
    /// The line in this language (the original when it does not come).
    Into(Lang),
    /// The placeholder of a reader that mutes the sender (`chat_mute`):
    /// never the text, in any language.
    Muted,
}

/// The body a reader of a translation is sent, cut to what the client
/// reads (255 bytes) on a character.
fn translated_body(source: Option<Lang>, text: &str) -> String {
    let mut body = match source {
        Some(source) => format!("[{}] {text}", source.code()),
        None => text.to_owned(),
    };
    if body.len() > limits::TEXT {
        let mut end = limits::TEXT - "...".len();
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        body.truncate(end);
        body.push_str("...");
    }
    body
}

/// The `A2` of `said` with `body`.
fn message(said: &ChatLine, channel: u8, body: &str) -> ServerMessage {
    lobby::chat_text(
        sender::ACCOUNT,
        said.from,
        &said.name,
        &said.tag,
        channel,
        body,
    )
}

impl Core<'_> {
    /// A line (its sender's account, name and clan tag; account 0: the
    /// server's own) on `channel` to `readers` (their links and accounts; the
    /// sender's own client has shown it already), each as its mute of the
    /// sender says (`chat_mute`): none for one that hides the sender.
    pub(super) fn say(
        &mut self,
        said: ChatLine,
        channel: u8,
        readers: Vec<(SocketAddr, u32)>,
        now: Instant,
    ) {
        let (from, body) = (said.from, said.body.as_str());
        let language_of = |core: &Self, addr: SocketAddr| {
            core.player(addr)
                .and_then(|player| player.language)
                .and_then(Lang::from_game)
        };
        let game = self
            .by_account
            .get(&from)
            .and_then(|addr| language_of(self, *addr));
        let source = match from {
            0 => None,
            _ => detect::detect(body, game),
        };
        // The server's own lines are no player's chat.
        if from != 0
            && let Some(sample) = &mut self.chat_sample
        {
            sample.keep(game, source, channel, body);
        }
        let readers: Vec<Reader> = readers
            .into_iter()
            .filter_map(|(addr, account_id)| {
                let wants = match self.chat_mute(account_id, from) {
                    Some(lobby::ChatMuteMode::Hidden) => return None,
                    Some(lobby::ChatMuteMode::Placeholder) => Wants::Muted,
                    None => language_of(self, addr)
                        .filter(|lang| source.is_some_and(|source| source != *lang))
                        .map_or(Wants::Original, Wants::Into),
                };
                Some(Reader { addr, wants })
            })
            .collect();
        let Some(translator) = &self.translator else {
            for reader in readers {
                let body = match reader.wants {
                    Wants::Muted => lobby::MUTED_CHAT_TEXT,
                    Wants::Original | Wants::Into(_) => body,
                };
                self.send(reader.addr, message(&said, channel, body));
            }
            return;
        };
        let mut wanted: Vec<Lang> = readers
            .iter()
            .filter_map(|reader| match reader.wants {
                Wants::Into(lang) => Some(lang),
                Wants::Original | Wants::Muted => None,
            })
            .collect();
        wanted.sort();
        wanted.dedup();
        let deadline = now + translator.timeout;
        if let Some(translator) = &mut self.translator
            && !wanted.is_empty()
        {
            translator.request(&said.body, source, &wanted, said.from, now);
        }
        self.chat_queue.push_back(Pending {
            said,
            channel,
            source,
            deadline,
            readers,
        });
        self.chat_step(now);
    }

    /// The translations that came, and every line whose readers can have it
    /// now. On every pass of the lobby.
    pub(super) fn chat_step(&mut self, now: Instant) {
        let Some(translator) = &mut self.translator else {
            return;
        };
        translator.poll(now);
        if self.chat_queue.is_empty() {
            return;
        }
        // A reader with an earlier line still waiting gets no later one.
        let mut held: HashSet<SocketAddr> = HashSet::new();
        let mut out: Vec<(SocketAddr, ServerMessage)> = Vec::new();
        for line in &mut self.chat_queue {
            // A reader sent its line leaves it.
            line.readers.retain(|reader| {
                if held.contains(&reader.addr) {
                    return true;
                }
                let body = match reader.wants {
                    Wants::Original => Some(line.said.body.clone()),
                    Wants::Muted => Some(lobby::MUTED_CHAT_TEXT.to_owned()),
                    Wants::Into(lang) => match translator.lookup(&line.said.body, lang) {
                        Lookup::Text(text) => Some(translated_body(line.source, &text)),
                        Lookup::Original => Some(line.said.body.clone()),
                        Lookup::Waiting if now >= line.deadline => {
                            log::info!(
                                "A chat line of {} waited {} ms for its {} translation: sent as it is",
                                line.said.name,
                                translator.timeout.as_millis(),
                                lang.name()
                            );
                            Some(line.said.body.clone())
                        }
                        Lookup::Waiting => None,
                    },
                };
                match body {
                    Some(body) => {
                        out.push((reader.addr, message(&line.said, line.channel, &body)));
                        false
                    }
                    None => {
                        held.insert(reader.addr);
                        true
                    }
                }
            });
        }
        self.chat_queue.retain(|line| !line.readers.is_empty());
        for (addr, message) in out {
            self.send(addr, message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_translation_is_marked_and_fits_the_clients_buffer() {
        assert_eq!(
            translated_body(Some(Lang::Ru), "go to base"),
            "[ru] go to base"
        );
        let long = "щ".repeat(200);
        let body = translated_body(Some(Lang::Uk), &long);
        assert!(body.len() <= limits::TEXT);
        assert!(body.starts_with("[uk] щ") && body.ends_with("..."));
    }
}
