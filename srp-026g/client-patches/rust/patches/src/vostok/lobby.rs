//! The lobby as the match's network client keeps it: the account's name,
//! the friend list the lobby pushed (`A3 07`, social.md 1.3), the lobby
//! connection the messaging senders check, the friend request the lobby's
//! own menus send (`4D` type 0, messaging_client::send_important_message
//! 0x26AFB0), and the match's places (the Tab list's rows).

use core::ffi::CStr;
use core::marker::PhantomData;

use super::hook::Thread;
use super::types::Place;
use super::ui::c_string;
use crate::raw;

/// A place's or an account's name: 64 bytes with its zero (the places', the
/// friend list's, the 4D's target buffers).
const NAME_LIMIT: usize = 64;
/// The account's own name's room in the lobby client (+1E55D0h, the clan at +1E5650h).
const OWN_NAME_LIMIT: usize = 0x80;
/// The friend list holds at most this many (the lobby sends at most 200).
const MAX_FRIENDS: usize = 1000;
/// The lobby connection's states in which a messaging sender sends (0x28C1CF..0x28C1D6).
const SENDING: [u32; 2] = [0, 4];
/// The friend request's type in a 4D (important_message_types: 0).
const FRIEND_REQUEST: u32 = 0;

/// The lobby connection is down: a messaging message is not sent (as the
/// lobby's own senders refuse to).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotConnected;

/// The lobby of a match's network client.
pub struct Lobby<'t> {
    client: raw::NetworkClient,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Lobby<'t> {
    pub(super) fn of(client: raw::NetworkClient) -> Lobby<'t> {
        Lobby { client, thread: PhantomData }
    }

    /// The name of the player at `place`, when the match has that place.
    pub fn place_name(&self, place: Place) -> Option<&CStr> {
        // SAFETY: the network client's options and their places live as long
        // as the client, on the hook's thread; a place below the count is one
        // of the array, its name a C string in its 64 bytes.
        unsafe {
            let options = self.client.options_holder()?.options();
            if place.get() >= *options.places() {
                return None;
            }
            c_string(options.first_place().at(place.index()).name(), NAME_LIMIT)
        }
    }

    /// The account's own name.
    pub fn own_name(&self) -> Option<&CStr> {
        // SAFETY: the lobby client inside the network client, its name a C
        // string in its room.
        unsafe { c_string(self.client.lobby().name(), OWN_NAME_LIMIT) }
    }

    /// The lobby connection is up: a messaging sender would send now.
    fn connected(&self) -> bool {
        // SAFETY: the lobby client's connection slot and connection, read as
        // every messaging sender reads them first (0x28C1B6..0x28C1D6).
        unsafe {
            self.client.lobby().link().and_then(|link| link.connection())
                .is_some_and(|connection| SENDING.contains(&*connection.state()))
        }
    }

    /// `name` is on the friend list the lobby pushed (by the name, ignoring
    /// ASCII case, as the lobby finds an account).
    pub fn is_friend(&self, name: &[u8]) -> bool {
        // SAFETY: the messaging client's friend vector {begin, end}, its
        // elements 5Ch bytes each, their names C strings in their 64 bytes,
        // alive on the hook's thread (only the lobby's messages, on this
        // thread, change it).
        unsafe {
            let messaging = self.client.lobby().messaging();
            let (Some(first), Some(end)) = (messaging.friends_begin(), messaging.friends_end()) else {
                return false;
            };
            let count = end.index_from(first).unwrap_or(0).min(MAX_FRIENDS);
            (0..count).any(|i| {
                c_string(first.at(i).name(), NAME_LIMIT)
                    .is_some_and(|friend| friend.to_bytes().eq_ignore_ascii_case(name))
            })
        }
    }

    /// Sends the lobby a friend request to `name`, as the lobby's menus do
    /// (`4D`, type 0, no text), when the lobby connection is up.
    pub fn send_friend_request(&self, name: &CStr) -> Result<(), NotConnected> {
        if !self.connected() {
            return Err(NotConnected);
        }
        // SAFETY: the messaging client the lobby's add_friend_name passes
        // (0x28C1DF), on the game's thread; the name and the text C strings
        // that outlive the call (it copies both before it returns).
        unsafe {
            raw::engine_send_important(Some(self.client.lobby().messaging()), name.as_ptr().cast(),
                                       FRIEND_REQUEST, c"".as_ptr().cast());
        }
        Ok(())
    }
}
