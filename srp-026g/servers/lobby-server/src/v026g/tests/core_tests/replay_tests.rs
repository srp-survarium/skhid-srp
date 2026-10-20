//! "Watch replay" from the History of srp's changed lobby movie: the ask
//! (`3D` with `REPLAY_ASK`) and the polls (`3B` with `REPLAY_START`),
//! answered with the asker's own `R1` row and, when ready, the link (`9E`)
//! just before it. Crafted requests get the asker's own answer and no
//! token they are not given.

use std::path::PathBuf;

use session::v026g::replays::{ReplaySettings, Replays, Sidecar};
use survarium::v026g::lobby::{REPLAY_ASK, REPLAY_START};

use super::*;

const MATCH: u32 = 0x1F07_C8FB;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "srp-lobby-replay-{name}-{}-{}",
        std::process::id(),
        Replays::now_unix()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Replays in `root` with a made replay of `MATCH` that `players` played.
/// Its games were of 12 and 14, given from 11 on whatever the last row of
/// `simulation_changes.tsv` is: the refusals of older recordings have their
/// own tests (`replays_of`).
fn replays(root: &std::path::Path, players: &[u32]) -> &'static Replays {
    let settings = ReplaySettings {
        replay_dir: root.join("replays"),
        record_dir: root.join("recordings"),
        modes_dir: root.join("modes"),
        max_recording_bytes: 1 << 20,
        max_replay_bytes: 1 << 20,
    }
    .checked()
    .unwrap();
    let replays: &'static Replays = Box::leak(Box::new(
        Replays::new(Some(settings), 6).with_oldest_revision(11),
    ));
    std::fs::write(replays.sur_path(MATCH).unwrap(), b"0123456789ab").unwrap();
    let sidecar = Sidecar {
        match_id: MATCH,
        size: 12,
        sha256: "cd".repeat(32),
        revisions: vec![Some(14), Some(12)],
        account_ids: players.to_vec(),
        made_at: 0,
    };
    std::fs::write(replays.json_path(MATCH).unwrap(), sidecar.to_json()).unwrap();
    replays
}

/// A player whose game signed in as `revision`.
fn player(world: &mut World, name: &str, revision: u8) -> usize {
    let (account, _) = signed_in(world.store, name);
    let session = world.store.create_session_with_revision(account, revision);
    world.connect_session((account, session))
}

/// The movie's poll: what came back, as `(9E text, R1 row)`.
fn poll(world: &mut World, index: usize) -> (Option<String>, String) {
    world.take(index);
    world.ask(index, request::ranking(REPLAY_START, 1));
    let decoded = world.take(index);
    let (link, rows) = match &decoded[..] {
        [Decoded::EloList(rows)] => (None, rows),
        [Decoded::VideoLink { show, url }, Decoded::EloList(rows)] => {
            assert!(!show, "the link is never shown in the lobby");
            (Some(url.clone()), rows)
        }
        other => panic!("not an R1 answer: {other:?}"),
    };
    let [(name, place, 0, 0)] = &rows[..] else {
        panic!("not one row: {rows:?}");
    };
    assert_eq!(*place, REPLAY_START);
    (link, name.clone())
}

#[test]
fn replays_off_answer_off_and_ask_nothing() {
    let mut world = World::new();
    let alice = player(&mut world, "Alice", 14);
    world.take(alice);
    world.ask(alice, request::player_details(REPLAY_ASK, MATCH));
    // The ask itself is not answered: the polls are.
    assert!(world.take(alice).is_empty());
    assert_eq!(poll(&mut world, alice), (None, "R1;off;00000000;0".into()));
    world.assert_clean();
}

#[test]
fn a_player_of_the_match_gets_the_link_before_a_ready_row() {
    let root = temp("ready");
    let mut world = World::new();
    let alice = player(&mut world, "Alice", 14);
    let bob = player(&mut world, "Bob", 14);
    let carol = player(&mut world, "Carol", 12);
    let dave = player(&mut world, "Dave", 14);
    let desk = replays(&root, &[world.ids[alice].0, world.ids[carol].0]);
    world.core.replays = Some(desk);

    // Nobody asked yet: `none`, for the match 0.
    assert_eq!(poll(&mut world, alice), (None, "R1;none;00000000;0".into()));
    world.ask(alice, request::player_details(REPLAY_ASK, MATCH));
    assert!(world.take(alice).is_empty());
    let (link, row) = poll(&mut world, alice);
    assert_eq!(row, "R1;ready;1f07c8fb;12");
    let link = link.expect("a 9E before the ready row");
    let parts: Vec<&str> = link.split(' ').collect();
    assert_eq!(parts.len(), 5, "{link}");
    assert_eq!(
        (parts[0], parts[1], parts[3], parts[4]),
        ("replay", "1f07c8fb", "cd".repeat(32).as_str(), "12")
    );
    let token = parts[2];
    assert_eq!(token.len(), 32);
    assert!(link.chars().count() <= 245);
    // The token downloads that match, and only from the web page.
    assert_eq!(
        desk.take_download(token, false, world.now)
            .map(|download| download.match_id),
        Ok(MATCH)
    );

    // Bob did not play it: `not_yours`, no link; his polls are his own.
    world.ask(bob, request::player_details(REPLAY_ASK, MATCH));
    assert_eq!(
        poll(&mut world, bob),
        (None, "R1;not_yours;1f07c8fb;0".into())
    );
    // Carol played it with revision 12 (the patches' mount, no
    // replay link): her game cannot start a replay.
    world.ask(carol, request::player_details(REPLAY_ASK, MATCH));
    assert_eq!(
        poll(&mut world, carol),
        (None, "R1;old_game;1f07c8fb;0".into())
    );
    // Dave never asked: his crafted poll shows nobody's answer.
    assert_eq!(poll(&mut world, dave), (None, "R1;none;00000000;0".into()));
    // Another match, of no recording: gone.
    world.advance(Duration::from_secs(3));
    world.ask(alice, request::player_details(REPLAY_ASK, 0x1234));
    assert_eq!(poll(&mut world, alice), (None, "R1;gone;00001234;0".into()));
    // Asked again at once for another: busy (one ask in two seconds).
    world.ask(alice, request::player_details(REPLAY_ASK, MATCH));
    assert_eq!(poll(&mut world, alice), (None, "R1;busy;1f07c8fb;0".into()));
    // An ordinary row of a results screen is still that (none kept here).
    world.ask(alice, request::player_details(2, MATCH));
    assert!(world.take(alice).is_empty());
    world.assert_clean();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_preparing_row_carries_the_place_in_the_conversion_line() {
    let root = temp("line");
    let mut world = World::new();
    let alice = player(&mut world, "Alice", 14);
    let bob = player(&mut world, "Bob", 14);
    let desk = replays(&root, &[]);
    world.core.replays = Some(desk);
    // Two kept recordings with no replay yet (MATCH has one).
    let record_dir = &desk.settings().unwrap().record_dir;
    for id in [0x0A, 0x0B] {
        std::fs::write(
            record_dir.join(format!("rec026g_20261015-120000_{id:08x}.srprec")),
            b"x",
        )
        .unwrap();
        desk.recording_kept(id);
    }
    world.ask(alice, request::player_details(REPLAY_ASK, 0x0A));
    assert_eq!(
        poll(&mut world, alice),
        (None, "R1;preparing;0000000a;1".into())
    );
    world.ask(bob, request::player_details(REPLAY_ASK, 0x0B));
    assert_eq!(
        poll(&mut world, bob),
        (None, "R1;preparing;0000000b;2".into())
    );
    // The first is taken up: still 1 for it, still 2 behind it.
    assert_eq!(desk.next_job(), Some(0x0A));
    assert_eq!(
        poll(&mut world, alice),
        (None, "R1;preparing;0000000a;1".into())
    );
    assert_eq!(
        poll(&mut world, bob),
        (None, "R1;preparing;0000000b;2".into())
    );
    // A `ready` row keeps the size there.
    world.advance(Duration::from_secs(3));
    world.ask(alice, request::player_details(REPLAY_ASK, MATCH));
    let (link, row) = poll(&mut world, alice);
    assert!(link.is_some());
    assert_eq!(row, "R1;ready;1f07c8fb;12");
    world.assert_clean();
    let _ = std::fs::remove_dir_all(&root);
}

/// `4F` of a game in `language` (the general room in slot 1, the
/// language byte last, as 0x26C320 sends it).
fn subscribe(language: u8) -> (u8, Vec<u8>) {
    let ids: [u32; 10] = [0, 7, u32::MAX, 0, 1, 0, 0, 0, 0, 0];
    let mut body: Vec<u8> = ids.iter().flat_map(|id| id.to_le_bytes()).collect();
    body.push(language);
    (0x4F, body)
}

/// Replays whose oldest given revision is `oldest`, with a made replay of
/// `MATCH` recorded by games of `revisions`, for every account.
fn replays_of(root: &std::path::Path, oldest: u8, revisions: Vec<Option<u8>>) -> &'static Replays {
    let settings = ReplaySettings {
        replay_dir: root.join("replays"),
        record_dir: root.join("recordings"),
        modes_dir: root.join("modes"),
        max_recording_bytes: 1 << 20,
        max_replay_bytes: 1 << 20,
    }
    .checked()
    .unwrap();
    let replays: &'static Replays = Box::leak(Box::new(
        Replays::new(Some(settings), 6).with_oldest_revision(oldest),
    ));
    std::fs::write(replays.sur_path(MATCH).unwrap(), b"0123456789ab").unwrap();
    let sidecar = Sidecar {
        match_id: MATCH,
        size: 12,
        sha256: "cd".repeat(32),
        revisions,
        account_ids: Vec::new(),
        made_at: 0,
    };
    std::fs::write(replays.json_path(MATCH).unwrap(), sidecar.to_json()).unwrap();
    replays
}

/// A poll: the `R1` row, and the message box's text if one came after it
/// (never a link).
fn poll_refused(world: &mut World, index: usize) -> (String, Option<String>) {
    world.take(index);
    world.ask(index, request::ranking(REPLAY_START, 1));
    match &world.take(index)[..] {
        [Decoded::EloList(rows)] => (rows[0].0.clone(), None),
        [Decoded::EloList(rows), Decoded::Denied { op, text }] => {
            assert_eq!(*op, 0x49, "the message box and nothing else");
            (rows[0].0.clone(), Some(text.clone()))
        }
        other => panic!("not a refused answer: {other:?}"),
    }
}

#[test]
fn a_replay_of_games_before_the_last_simulation_change_is_refused_and_said_why() {
    let root = temp("older");
    let mut world = World::new();
    let taras = player(&mut world, "Taras", 17);
    let emma = player(&mut world, "Emma", 17);
    // Revision 17 changed what a match computes; the match was of 16.
    world.core.replays = Some(replays_of(&root, 17, vec![Some(16), None, Some(16)]));
    world.ask(taras, subscribe(2));

    world.ask(taras, request::player_details(REPLAY_ASK, MATCH));
    let (row, said) = poll_refused(&mut world, taras);
    assert_eq!(row, "R1;incompatible;1f07c8fb;0");
    let said = said.expect("a message box with the first answer");
    assert!(said.contains("старою версією гри (ревізія 16)"), "{said}");
    assert!(
        said.ends_with("Дивитися його можна лише в тому випуску: 026g-rev16."),
        "{said}"
    );
    // The movie's next polls: the row alone.
    assert_eq!(
        poll_refused(&mut world, taras),
        ("R1;incompatible;1f07c8fb;0".into(), None)
    );
    // A game that said no language (or English): English.
    world.ask(emma, request::player_details(REPLAY_ASK, MATCH));
    let (_, said) = poll_refused(&mut world, emma);
    assert_eq!(
        said.as_deref(),
        Some(
            "This replay was recorded with an older version of the game (revision 16), which \
             plays matches differently. It can be watched only with that release: 026g-rev16."
        )
    );
    // Asked again later: said again.
    world.advance(Duration::from_secs(3));
    world.ask(taras, request::player_details(REPLAY_ASK, MATCH));
    assert!(poll_refused(&mut world, taras).1.is_some());
    world.assert_clean();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_replay_recorded_before_the_revisions_were_kept_is_refused() {
    let root = temp("unknown");
    let mut world = World::new();
    let ivan = player(&mut world, "Ivan", 16);
    world.core.replays = Some(replays_of(&root, 11, vec![None, None]));
    world.ask(ivan, subscribe(1));
    world.ask(ivan, request::player_details(REPLAY_ASK, MATCH));
    let (row, said) = poll_refused(&mut world, ivan);
    assert_eq!(row, "R1;incompatible;1f07c8fb;0");
    assert!(
        said.unwrap()
            .starts_with("Этот повтор записан старой версией игры")
    );
    world.assert_clean();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn every_refusal_fits_the_message_box_in_every_language() {
    use crate::v026g::replay::refusal_text;
    use session::v026g::replays::Refusal;
    let refusals = [
        Refusal::Unknown,
        // The longest: two three-digit revisions and a tag.
        Refusal::Older {
            from: 199,
            to: 254,
            oldest: 255,
        },
        Refusal::Older {
            from: 13,
            to: 14,
            oldest: 17,
        },
        Refusal::Newer { from: 199, to: 255 },
    ];
    let english: Vec<String> = refusals
        .iter()
        .map(|&refusal| refusal_text(refusal, Some(0)))
        .collect();
    for language in [None, Some(0), Some(1), Some(2), Some(3), Some(4), Some(9)] {
        for (n, &refusal) in refusals.iter().enumerate() {
            let text = refusal_text(refusal, language);
            assert!(
                text.len() <= 255,
                "{language:?} {refusal:?}: {} bytes",
                text.len()
            );
            if matches!(language, Some(1..=4)) {
                assert_ne!(text, english[n], "{language:?} {refusal:?}");
            }
            for part in ["199-254", "026g-rev254", "13-14", "199-255"] {
                assert_eq!(
                    text.contains(part),
                    english[n].contains(part),
                    "{language:?} {part}"
                );
            }
        }
    }
    // 14 has no tag: the release of that revision, unnamed.
    assert!(!refusal_text(refusals[2], None).contains("026g-rev"));
}
