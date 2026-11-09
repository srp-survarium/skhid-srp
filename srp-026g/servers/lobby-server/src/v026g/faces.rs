//! The face an account is drawn with (`docs/notes/face-choice.md`): the
//! lobby's side of its wire (`survarium::v026g::lobby::faces`, `lobby.md`
//! §2.16.6). The movie asks for the state and chooses a face; a face chosen
//! is sent in the account (`87`, every kit's revision bumped), so the exe
//! draws the lobby's character with it, and squad mates' lobbies draw it
//! too (`0xA1`'s revisions, then their `88`).

use session::LockExt;
use survarium::v026g::faces::{Face, Faces};
use survarium::v026g::lobby::faces::{self as wire, FaceDone, FaceRequest};

use super::core::{Core, Who};

impl Core<'_> {
    /// A faces request (`3B` at a faces start): answered with the state.
    pub(super) fn face_request(&mut self, who: &Who, request: FaceRequest) {
        let done = match request {
            FaceRequest::State => FaceDone::Stated,
            FaceRequest::Choose(face) => self.face_choose(who, face),
            FaceRequest::Unlisted => {
                log::info!(
                    "{}: choose a face the game data does not list: refused",
                    who.name
                );
                FaceDone::Refused
            }
        };
        let face = who.account.locked().face;
        self.send_made(who.addr, wire::face_state(done, face, Faces::shared()));
    }

    /// `face` the asker's, not while it is in a match or on its way to one
    /// (as a preset's choice: the match has its face already). Done: the
    /// client gets the account, and squad mates the kits' new revisions.
    fn face_choose(&mut self, who: &Who, face: Face) -> FaceDone {
        let free = self
            .player(who.addr)
            .is_some_and(|player| player.in_match.is_none() && player.connect.is_none());
        if !free {
            log::info!(
                "{}: choose face {}: refused in a match",
                who.name,
                face.index()
            );
            return FaceDone::Refused;
        }
        if who.account.locked().choose_face(face) {
            log::info!("{}: face {}", who.name, face.index());
            self.push_account(who);
            self.squads_dirty = true;
        }
        FaceDone::Chosen
    }
}
