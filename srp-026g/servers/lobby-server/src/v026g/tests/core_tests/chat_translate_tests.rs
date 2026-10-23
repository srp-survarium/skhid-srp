//! Chat translation (`chat_out`, `translate`): each reader gets a player's
//! line in its own game language, the original when that is the line's,
//! and nothing waits longer than the timeout or for a failed backend.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::v026g::translate::fake::{Broken, Echo, Stuck};
use crate::v026g::translate::{self, Backend, Translator};

use super::*;

/// `4F` of a game in `language` (0 English, 1 Russian, 2 Ukrainian, 3
/// German, 4 Polish), listening to the general channel.
fn subscribe(language: u8) -> (u8, Vec<u8>) {
    let ids: [u32; 10] = [0, 1, u32::MAX, 0, 1, 0, 0, 0, 0, 0];
    let mut body: Vec<u8> = ids.iter().flat_map(|id| id.to_le_bytes()).collect();
    body.push(language);
    (0x4F, body)
}

/// The general chat's bodies the client took since the last `take`.
fn heard(world: &mut World, index: usize) -> Vec<String> {
    world
        .take(index)
        .into_iter()
        .filter_map(|message| match message {
            Decoded::ChatText {
                channel: channel::GENERAL,
                body,
                ..
            } => Some(body),
            _ => None,
        })
        .collect()
}

/// A lobby whose translator calls `backend` (inline: at once), with the
/// players of `languages` (`None`: a game that never sent `4F`).
fn lobby(backend: Option<Arc<dyn Backend>>, languages: &[Option<u8>]) -> (World, Vec<usize>) {
    let mut world = World::new();
    world.core.translator =
        backend.map(|backend| Translator::inline(backend, translate::DEFAULT_TIMEOUT));
    let players: Vec<usize> = languages
        .iter()
        .enumerate()
        .map(|(at, language)| {
            let player = world.connect(&format!("Player{at}"));
            if let Some(language) = language {
                world.ask(player, subscribe(*language));
            }
            player
        })
        .collect();
    for player in &players {
        world.take(*player);
    }
    (world, players)
}

const RUSSIAN: u8 = 1;
const GERMAN: u8 = 3;
const ENGLISH: u8 = 0;
/// No language of the five (a stock client sends 0 to 4 only).
const UNKNOWN: u8 = 9;

#[test]
fn a_line_reaches_each_reader_in_its_own_language() {
    let echo = Arc::new(Echo::default());
    let (mut world, players) = lobby(
        Some(echo.clone()),
        &[
            Some(RUSSIAN),
            Some(RUSSIAN),
            Some(GERMAN),
            Some(ENGLISH),
            Some(UNKNOWN),
        ],
    );
    let [ivan, olga, hans, emma, quiet] = players[..] else {
        unreachable!()
    };
    let line = "го на базу, там снайпер";
    world.ask(ivan, request::chat(1, "", channel::GENERAL, line));
    assert_eq!(heard(&mut world, olga), [line]);
    assert_eq!(heard(&mut world, hans), [format!("[ru] de: {line}")]);
    assert_eq!(heard(&mut world, emma), [format!("[ru] en: {line}")]);
    // A language byte srp does not know: the original.
    assert_eq!(heard(&mut world, quiet), [line]);
    assert!(heard(&mut world, ivan).is_empty());
    // One call for both languages.
    assert_eq!(echo.calls.load(std::sync::atomic::Ordering::Relaxed), 1);
    // The same line again is not asked again.
    world.ask(olga, request::chat(1, "", channel::GENERAL, line));
    assert_eq!(heard(&mut world, hans), [format!("[ru] de: {line}")]);
    assert_eq!(echo.calls.load(std::sync::atomic::Ordering::Relaxed), 1);
    world.assert_clean();
}

#[test]
fn shorthand_is_never_translated_and_off_is_as_before() {
    let echo = Arc::new(Echo::default());
    let (mut world, players) = lobby(Some(echo.clone()), &[Some(RUSSIAN), Some(GERMAN)]);
    world.ask(players[0], request::chat(1, "", channel::GENERAL, "gg ez"));
    assert_eq!(heard(&mut world, players[1]), ["gg ez"]);
    assert_eq!(echo.calls.load(std::sync::atomic::Ordering::Relaxed), 0);

    let (mut world, players) = lobby(None, &[Some(RUSSIAN), Some(GERMAN)]);
    world.ask(
        players[0],
        request::chat(1, "", channel::GENERAL, "привет всем"),
    );
    assert_eq!(heard(&mut world, players[1]), ["привет всем"]);
}

#[test]
fn a_failing_backend_gives_the_original_at_once() {
    let (mut world, players) = lobby(Some(Arc::new(Broken)), &[Some(RUSSIAN), Some(GERMAN)]);
    world.ask(
        players[0],
        request::chat(1, "", channel::GENERAL, "привет всем"),
    );
    assert_eq!(heard(&mut world, players[1]), ["привет всем"]);
}

#[test]
fn a_slow_backend_holds_a_line_no_longer_than_the_timeout_and_keeps_the_order() {
    let (release, wait) = mpsc::channel();
    let stuck = Arc::new(Stuck {
        release: Mutex::new(wait),
    });
    let (mut world, players) = lobby(None, &[Some(RUSSIAN), Some(RUSSIAN), Some(GERMAN)]);
    // On worker threads, as srp runs it; this backend never answers.
    world.core.translator = Some(Translator::threaded(stuck, 1, translate::DEFAULT_TIMEOUT));
    let [ivan, olga, hans] = players[..] else {
        unreachable!()
    };
    world.ask(ivan, request::chat(1, "", channel::GENERAL, "привет всем"));
    world.ask(ivan, request::chat(1, "", channel::GENERAL, "gg"));
    // The Russian reader is not held up; the German one waits, and its
    // "gg" waits behind the line before it.
    assert_eq!(heard(&mut world, olga), ["привет всем", "gg"]);
    assert!(heard(&mut world, hans).is_empty());
    world.advance(translate::DEFAULT_TIMEOUT);
    assert_eq!(heard(&mut world, hans), ["привет всем", "gg"]);
    world.assert_clean();
    drop(release);
}

#[test]
fn a_rooms_lines_are_translated_but_not_its_own_notices() {
    let echo = Arc::new(Echo::default());
    let (mut world, players) = lobby(Some(echo.clone()), &[Some(GERMAN)]);
    let hans = players[0];
    let notice = session::v026g::rooms::ChatLine {
        from: 0,
        name: String::new(),
        tag: String::new(),
        body: "Ivan joined the room".into(),
    };
    let reader = world.clients[hans].0;
    world
        .core
        .say(notice, channel::ROOM, vec![reader], world.now);
    world.settle();
    let bodies: Vec<String> = world
        .take(hans)
        .into_iter()
        .filter_map(|message| match message {
            Decoded::ChatText { body, .. } => Some(body),
            _ => None,
        })
        .collect();
    assert_eq!(bodies, ["Ivan joined the room"]);
    assert_eq!(echo.calls.load(std::sync::atomic::Ordering::Relaxed), 0);
}
