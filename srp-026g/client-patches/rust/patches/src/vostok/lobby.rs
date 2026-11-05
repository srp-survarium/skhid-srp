//! The lobby as the match's network client keeps it: the account's name,
//! the friend list the lobby pushed (`A3 07`, social.md 1.3), the lobby
//! connection the messaging senders check, the friend request the lobby's
//! own menus send (`4D` type 0, messaging_client::send_important_message
//! 0x26AFB0), the chat mute the lobby's player menu sends (`49 05` kind 254,
//! lobby_client::comment_about_user 0x274470), and the match's places (the
//! Tab list's rows).

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
/// The friend and ignore lists hold at most this many (the lobby sends at
/// most 200).
const MAX_LISTED: usize = 1000;
/// srp's mark on an ignore list's id: a player shown as `<muted>`, not hidden
/// (the lobby's `SHOWN_MUTED`, docs/notes/chat-mute.md).
const SHOWN_MUTED: u32 = 1 << 31;
/// The lobby connection's states in which a messaging sender sends (0x28C1CF..0x28C1D6).
const SENDING: [u32; 2] = [0, 4];
/// The friend request's type in a 4D (important_message_types: 0).
const FRIEND_REQUEST: u32 = 0;

/// srp's chat mute in a `49 05` (comment_about_user's kind): the lobby's
/// `CHAT_MUTE_ACTION` (survarium::v026g::lobby), the lobby movie's
/// `ChatMute.ACTION`.
const CHAT_MUTE: u32 = 254;

/// How a player's chat lines are muted for the account: the lobby's
/// `ChatMuteMode`, read from the ignore list it pushed (`A3 08`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Muted {
    Hidden,
    ShownMuted,
}

impl Muted {
    /// An ignore list row's id: the mode is its mark.
    fn of_listed_id(id: u32) -> Muted {
        if id & SHOWN_MUTED != 0 { Muted::ShownMuted } else { Muted::Hidden }
    }

    /// The hud movie's code for a mute state: the menu's choice that sets it
    /// (`"1"`, `"2"`; 0 not muted, `MuteChoice::Unmute`'s).
    pub fn code(state: Option<Muted>) -> u32 {
        match state {
            None => 0,
            Some(Muted::Hidden) => 1,
            Some(Muted::ShownMuted) => 2,
        }
    }
}

/// A chat mute choice of a player's menu (docs/notes/chat-mute.md), as the
/// movies send it: `"1"` hide the player's lines, `"2"` show them as
/// `<muted>`, `"0"` unmute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MuteChoice {
    Hide,
    ShowMuted,
    Unmute,
}

impl MuteChoice {
    pub fn parse(text: &[u8]) -> Option<MuteChoice> {
        match text {
            b"1" => Some(MuteChoice::Hide),
            b"2" => Some(MuteChoice::ShowMuted),
            b"0" => Some(MuteChoice::Unmute),
            _ => None,
        }
    }

    /// The state the choice leaves.
    pub fn state(self) -> Option<Muted> {
        match self {
            MuteChoice::Hide => Some(Muted::Hidden),
            MuteChoice::ShowMuted => Some(Muted::ShownMuted),
            MuteChoice::Unmute => None,
        }
    }

    /// The `49 05`'s mode byte (the lobby's `ChatMuteMode`, 0 unmute).
    fn mode(self) -> u32 {
        match self {
            MuteChoice::Unmute => 0,
            MuteChoice::Hide => 1,
            MuteChoice::ShowMuted => 2,
        }
    }
}

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
        // SAFETY: the messaging client's friend vector, alive on the hook's
        // thread (only the lobby's messages, on this thread, change it).
        unsafe {
            let messaging = self.client.lobby().messaging();
            listed(messaging.friends_begin(), messaging.friends_end(), name).is_some()
        }
    }

    /// How `name`'s lines are muted, by the ignore list the lobby pushed
    /// (every muted player, the mode its id's mark), or `None`.
    pub fn muted(&self, name: &[u8]) -> Option<Muted> {
        // SAFETY: the messaging client's ignore vector, as the friend one.
        unsafe {
            let messaging = self.client.lobby().messaging();
            listed(messaging.ignored_begin(), messaging.ignored_end(), name)
                .map(|account| Muted::of_listed_id(*account.id()))
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

    /// Sends the lobby `choice` for `name`'s chat lines, as the lobby's player
    /// menu does (`49 05`, kind 254), when the lobby connection is up
    /// (comment_about_user checks none itself).
    pub fn send_chat_mute(&self, name: &CStr, choice: MuteChoice) -> Result<(), NotConnected> {
        if !self.connected() {
            return Err(NotConnected);
        }
        // SAFETY: the lobby client the lobby's own comment_about_user passes
        // (0x28CC57: the network client's vtable +3Ch, 0x219960, its +1E0h),
        // on the game's thread; the name a C string that outlives the call
        // (written into the message before it returns).
        unsafe {
            raw::engine_comment_about_user(Some(self.client.lobby()), name.as_ptr().cast(), CHAT_MUTE,
                                           choice.mode());
        }
        Ok(())
    }
}

/// The record of a list {begin, end} named `name` (ASCII case ignored).
///
/// # Safety
/// `begin..end` is one of the messaging client's vectors of 5Ch-byte
/// records, alive for the call, their names C strings in their 64 bytes.
unsafe fn listed(begin: Option<raw::ListedAccount>, end: Option<raw::ListedAccount>, name: &[u8])
                 -> Option<raw::ListedAccount> {
    let (Some(first), Some(end)) = (begin, end) else { return None };
    // SAFETY: the caller's vector.
    unsafe {
        let count = end.index_from(first).unwrap_or(0).min(MAX_LISTED);
        (0..count).map(|i| first.at(i)).find(|account| {
            c_string(account.name(), NAME_LIMIT).is_some_and(|listed| listed.to_bytes().eq_ignore_ascii_case(name))
        })
    }
}
