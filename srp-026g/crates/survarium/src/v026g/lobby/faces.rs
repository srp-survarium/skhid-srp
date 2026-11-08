//! The face a player chooses (`face-choice.md`): srp's own wire, on the
//! ranking page's request (`3B`) at reserved starts and its reply (`91`),
//! as the presets are (`presets.rs`, `lobby.md` §2.16.6). Only srp's
//! changed lobby movie asks; a stock game never does.
//!
//! Every start is from [`FACES_FIRST`] to [`FACES_LAST`], above the lobby
//! pick and within an `int`. Every request is answered with the state
//! ([`face_state`]) at [`FACES_FIRST`]; a face chosen is sent in the
//! account (`87`) first, which the exe draws the lobby's character by.

use super::super::faces::{Face, Faces, MAX_FACES};
use super::push::{Ranked, elo_rows};
use super::{ServerMessage, WireError};

/// The first start: the asker's face and the faces' count.
pub const FACES_FIRST: u32 = 2_097_000_000;
/// `FACE_CHOOSE_BASE + face`, face 0 to [`MAX_FACES`] - 1: choose it.
pub const FACE_CHOOSE_BASE: u32 = FACES_FIRST + 10;
/// The last start.
pub const FACES_LAST: u32 = FACES_FIRST + 99;
/// The first field of the answer's row.
pub const FACES_VERSION: &str = "F1";

/// A faces request, by its start, the face parsed against the data's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceRequest {
    State,
    Choose(Face),
    /// A choice of a face the data does not list: refused.
    Unlisted,
}

/// The faces request a reserved start is, if it is one.
pub fn face_request(start: u32, faces: Faces) -> Option<FaceRequest> {
    match start {
        FACES_FIRST => Some(FaceRequest::State),
        _ if (FACE_CHOOSE_BASE..FACE_CHOOSE_BASE + u32::from(MAX_FACES)).contains(&start) => {
            Some(
                faces
                    .face(start - FACE_CHOOSE_BASE)
                    .map_or(FaceRequest::Unlisted, FaceRequest::Choose),
            )
        }
        _ => None,
    }
}

/// What a request did, said in the answer.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceDone {
    /// Only the state.
    Stated = 0,
    /// The face asked for is the account's now.
    Chosen = 1,
    /// Nothing changed: a face the data does not list.
    Refused = 2,
}

/// `91` at [`FACES_FIRST`], one row: `F1;<done>;<face>;<count>` (`done` a
/// [`FaceDone`], `face` the account's, `count` the faces of the data).
pub fn face_state(done: FaceDone, face: Face, faces: Faces) -> Result<ServerMessage, WireError> {
    let row = Ranked {
        name: state_name(done, face, faces),
        rating: 0,
        rating_before: 0,
    };
    elo_rows(&[(FACES_FIRST, &row)])
}

fn state_name(done: FaceDone, face: Face, faces: Faces) -> String {
    format!(
        "{FACES_VERSION};{};{};{}",
        done as u8,
        face.index(),
        faces.count()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_starts_are_above_the_lobby_pick_and_inside_an_int() {
        const { assert!(FACES_FIRST > super::super::lobby_pick::LOBBY_PICK_BASE + 99) };
        const { assert!(FACES_LAST < i32::MAX as u32) };
        const { assert!(FACE_CHOOSE_BASE + MAX_FACES as u32 - 1 <= FACES_LAST) };
    }

    #[test]
    fn a_start_is_a_request_by_the_datas_faces() {
        let faces = Faces::of_count(3).unwrap();
        assert_eq!(face_request(FACES_FIRST, faces), Some(FaceRequest::State));
        assert_eq!(
            face_request(FACE_CHOOSE_BASE + 2, faces),
            Some(FaceRequest::Choose(faces.face(2).unwrap()))
        );
        assert_eq!(face_request(FACE_CHOOSE_BASE + 3, faces), Some(FaceRequest::Unlisted));
        assert_eq!(face_request(FACE_CHOOSE_BASE + 89, faces), Some(FaceRequest::Unlisted));
        for start in [FACES_FIRST + 1, FACE_CHOOSE_BASE + 90, FACES_FIRST - 1] {
            assert_eq!(face_request(start, faces), None, "{start}");
        }
    }

    #[test]
    fn the_state_is_one_row() {
        let faces = Faces::of_count(11).unwrap();
        assert_eq!(state_name(FaceDone::Chosen, faces.face(4).unwrap(), faces), "F1;1;4;11");
        assert!(face_state(FaceDone::Stated, Face::DEFAULT, faces).is_ok());
    }
}
