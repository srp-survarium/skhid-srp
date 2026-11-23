//! The face an account is drawn with (`faces`, `docs/notes/face-choice.md`):
//! the movie's state and choice (`3B` / `91`), the account (`87`) that
//! carries it to the exe, a squad mate's (`88`), and the match profile.

use survarium::v026g::faces::{Face, Faces};
use survarium::v026g::lobby::faces::{FACE_CHOOSE_BASE, FACES_FIRST};

use super::*;

/// What a `3B` at `start` is answered with: the rows of its `91`, and
/// whether the account (`87`) came before it, with its face.
fn asked(world: &mut World, index: usize, start: u32) -> (Vec<String>, Option<Face>) {
    world.settle();
    world.take(index);
    answered_at_once(world, index, start)
}

/// [`asked`], of a lobby as it stands: nothing settled before the request.
fn answered_at_once(world: &mut World, index: usize, start: u32) -> (Vec<String>, Option<Face>) {
    world.ask(index, request::ranking(start, 1));
    let decoded = world.take(index);
    let list = decoded
        .iter()
        .position(|message| matches!(message, Decoded::EloList(_)))
        .expect("an answer");
    let Decoded::EloList(rows) = &decoded[list] else {
        unreachable!()
    };
    let account = decoded[..list].iter().find_map(|message| match message {
        Decoded::Account(account) => Some(account.face),
        _ => None,
    });
    (rows.iter().map(|row| row.0.clone()).collect(), account)
}

fn face(index: u8) -> Face {
    Faces::shared().face(u32::from(index)).unwrap()
}

#[test]
fn a_face_is_chosen_from_the_datas_and_sent_in_the_account() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let count = Faces::shared().count();
    assert_eq!(
        asked(&mut world, alice, FACES_FIRST),
        (vec![format!("F1;0;0;{count}")], None)
    );
    let revisions: Vec<u32> = world
        .account(alice)
        .kits
        .iter()
        .map(|kit| kit.revision)
        .collect();

    // Chosen: the account first, its face the exe draws by, every kit redrawn.
    let (rows, sent) = asked(&mut world, alice, FACE_CHOOSE_BASE + 2);
    assert_eq!(rows, [format!("F1;1;2;{count}")]);
    assert_eq!(sent, Some(face(2)));
    assert_eq!(world.account(alice).face, face(2));
    let now: Vec<u32> = world
        .account(alice)
        .kits
        .iter()
        .map(|kit| kit.revision)
        .collect();
    assert!(
        now.iter()
            .zip(&revisions)
            .all(|(now, before)| *now == before + 1),
        "{now:?}"
    );

    // The same again: chosen, nothing to send.
    assert_eq!(
        asked(&mut world, alice, FACE_CHOOSE_BASE + 2),
        (vec![format!("F1;1;2;{count}")], None)
    );
    // One the data does not list: refused, the face kept.
    let unlisted = FACE_CHOOSE_BASE + u32::from(count);
    assert_eq!(
        asked(&mut world, alice, unlisted),
        (vec![format!("F1;2;2;{count}")], None)
    );
    assert_eq!(world.account(alice).face, face(2));

    // A match draws the player with it.
    let account = world.account(alice).clone();
    assert_eq!(
        crate::v026g::play::match_profile(&account, 0, 0, session::v026g::quests::now()).face,
        face(2)
    );
}

#[test]
fn a_squad_mate_sees_the_face_chosen() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let bob_id = world.ids[bob].0;
    form_squad(&mut world, alice, bob);
    world.settle();
    world.take(alice);
    asked(&mut world, bob, FACE_CHOOSE_BASE + 1);
    world.settle();
    // Bob's kits moved: Alice's squad info says so, and her client asks.
    let kit = world
        .take(alice)
        .iter()
        .rev()
        .find_map(|message| match message {
            Decoded::SquadInfo { members, .. } => Some(members[1].kit_id),
            _ => None,
        })
        .expect("the squad again");
    world.ask(alice, request::squad_member(bob_id, kit));
    let decoded = world.take(alice);
    let [Decoded::SquadMemberAccount { face: shown, .. }] = &decoded[..] else {
        panic!("{decoded:?}");
    };
    assert_eq!(*shown, face(1));
}

/// What a choice of face 1 does to a player whose lobby state `set` made.
fn chosen_in(
    set: impl FnOnce(&mut crate::v026g::core::Player<'_>),
) -> (Vec<String>, Option<Face>, Face) {
    let mut world = World::new();
    let alice = world.connect("Alice");
    world.settle();
    world.take(alice);
    let addr = world.clients[alice].0;
    set(world.core.player_mut(addr).unwrap());
    let (rows, sent) = answered_at_once(&mut world, alice, FACE_CHOOSE_BASE + 1);
    let kept = world.account(alice).face;
    (rows, sent, kept)
}

#[test]
fn a_face_is_not_chosen_in_a_match_or_on_the_way_to_one() {
    let count = Faces::shared().count();
    let refused = (vec![format!("F1;2;0;{count}")], None, Face::DEFAULT);
    // In a match, and sent to one (its 0x82 not out yet): refused, no
    // account sent, the face kept.
    assert_eq!(chosen_in(|player| player.in_match = Some(1)), refused);
    let pending = survarium::v026g::lobby::faces::face_state(
        survarium::v026g::lobby::faces::FaceDone::Stated,
        Face::DEFAULT,
        Faces::shared(),
    )
    .unwrap();
    assert_eq!(
        chosen_in(|player| player.connect = Some((Some(u64::MAX), pending))),
        refused
    );
    // Neither: chosen.
    assert_eq!(
        chosen_in(|_| {}),
        (vec![format!("F1;1;1;{count}")], Some(face(1)), face(1))
    );

    // The same of a match the lobby made: refused from the moment the
    // player is sent to it, the state still told.
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.match_of(alice, bob);
    assert_eq!(
        asked(&mut world, alice, FACE_CHOOSE_BASE + 1),
        (vec![format!("F1;2;0;{count}")], None)
    );
    assert_eq!(world.account(alice).face, Face::DEFAULT);
    assert_eq!(
        asked(&mut world, alice, FACES_FIRST),
        (vec![format!("F1;0;0;{count}")], None)
    );
}
