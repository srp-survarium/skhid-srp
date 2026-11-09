//! A match closed before its game played (patch 120, client_patch.md section
//! 3.74): what the game holds of the match at the sites that open and close
//! it, read once into types.
//!
//! - `unplayed.close_guard`: close_current_match (0x27A790) at its guard,
//!   which returns for every reason while the time rule's state the client
//!   keeps (+1F1C60h) is 0, from the connection until the first tick played
//!   after the first world, and again after a close. What the game holds of
//!   a match there ([`Closing`]): its handshake (no level asked for), its
//!   level loading (the world's +4E20h), or its level loaded with the match
//!   core set (+1F1C40h, on_match_ready's).
//! - `unplayed.load_end`: on_project_loaded's abort test, where the lobby
//!   client's status 0 (the lobby) unloads the level and never calls
//!   on_match_ready.
//! - `unplayed.ready_end`: on_match_ready's epilogue, the level loaded and
//!   shown, where a close that waited for the load is made.

use super::context::Context;
use super::hook::Thread;
use super::world::read;
use crate::raw;

/// close_current_match's reason (its jump table 0x27A958): 0 a timeout,
/// 1 the connection lost, 2 by the initiator, 3 the 0xEA of the own place,
/// 4 the server's 0xFD, 5 an expired ordered message. Any other value is the
/// game's to refuse (`ja` past 5 returns), so it passes through unread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseReason(u32);

impl CloseReason {
    /// A timeout (0x3FC0BC): the world ended, nothing told the lobby
    /// (0x27A941). The close of a load the lobby already took back.
    pub const TIMEOUT: CloseReason = CloseReason(0);
    /// The connection lost: `31` to the lobby and the world ended.
    pub const CONNECTION_LOST: CloseReason = CloseReason(1);
}

/// What a match its game never played holds (the time rule's state 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unplayed {
    /// The match link's handshake: no level asked for yet (before 0xE4's
    /// game_world::load), the lobby scene shown.
    Connecting,
    /// Its level loading: game_world::load (0x1F8CB4) to set_game_project
    /// (0x1F862F), the loading screen on the lobby scene.
    Loading,
    /// Its level loaded, waiting for its first world: the match core set
    /// (on_match_ready), the game world shown.
    Loaded,
}

/// What close_current_match finds at its guard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Closing {
    /// The time rule's state not 0: the game's own close, past the guard.
    Played,
    /// The state 0: a match never played, or one closed already.
    Unplayed(Unplayed),
}

/// close_current_match at its guard (`unplayed.close_guard`).
pub struct MatchClose<'t> {
    client: raw::NetworkClient,
    reason: CloseReason,
    closing: Closing,
    thread: &'t Thread,
}

impl<'t> MatchClose<'t> {
    pub fn at_guard(ctx: &'t Context<'_>) -> Option<MatchClose<'t>> {
        use raw::site::unplayed_close_guard as site;
        // SAFETY: the context is this site's: EDI the network client (this),
        // EBP close_current_match's frame, both alive while it runs on the
        // game's thread.
        let (client, frame) = unsafe { (site::client(ctx)?, site::frame(ctx)?) };
        // SAFETY: the frame's argument and the client's fields, read once.
        let (reason, closing) = unsafe { (CloseReason(read(frame.reason())), closing(client)) };
        Some(MatchClose { client, reason, closing, thread: ctx.thread() })
    }

    pub fn reason(&self) -> CloseReason {
        self.reason
    }

    pub fn closing(&self) -> Closing {
        self.closing
    }

    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// `31 <order> FF` to the lobby, what close_current_match sends for a
    /// lost connection (0x27A930..0x27A93C): the game is back from its
    /// match. srp's lobby answers with the lobby's state (0x85 00).
    pub fn tell_lobby_back(&self) {
        // SAFETY: on the game's thread inside close_current_match; the lobby
        // client is the network client's own (+1E0h, lobby_client 0x219960).
        unsafe { raw::engine_discard_playing_order(Some(self.client.lobby()), 1) }
    }
}

/// The client's fields that say what it holds of its match.
///
/// # Safety
/// `client` is alive, on the game's thread.
unsafe fn closing(client: raw::NetworkClient) -> Closing {
    unsafe {
        if read(client.play_state()) != 0 {
            return Closing::Played;
        }
        if client.pvp_match().is_some() {
            return Closing::Unplayed(Unplayed::Loaded);
        }
        let loading = client.game().is_some_and(|game| read(game.world().loading()) != 0);
        Closing::Unplayed(if loading { Unplayed::Loading } else { Unplayed::Connecting })
    }
}

/// A level's load done, at on_project_loaded's abort test
/// (`unplayed.load_end`).
pub struct LoadEnd<'t> {
    in_lobby: bool,
    thread: &'t Thread,
}

impl<'t> LoadEnd<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<LoadEnd<'t>> {
        // SAFETY: the context is this site's: EAX the lobby client (the
        // network client's vtable +3Ch), alive on the game's thread.
        let lobby = unsafe { raw::site::unplayed_load_end::lobby(ctx)? };
        // SAFETY: its field, read once.
        let in_lobby = unsafe { read(lobby.status()) } == 0;
        Some(LoadEnd { in_lobby, thread: ctx.thread() })
    }

    /// The lobby says the client is in the lobby (status 0): the game's
    /// own test would unload the level and return.
    pub fn in_lobby(&self) -> bool {
        self.in_lobby
    }

    pub fn thread(&self) -> &'t Thread {
        self.thread
    }
}

/// on_match_ready's end (`unplayed.ready_end`): the level loaded.
pub struct ReadyMatch<'t> {
    client: raw::NetworkClient,
    thread: &'t Thread,
}

impl<'t> ReadyMatch<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<ReadyMatch<'t>> {
        use raw::site::unplayed_ready_end as site;
        // SAFETY: the context is this site's: on_match_ready's frame by ESP,
        // its network client stored at 0x27AA39, alive on the game's thread.
        let client = unsafe { site::frame(ctx)?.client() }?;
        Some(ReadyMatch { client, thread: ctx.thread() })
    }

    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// close_current_match(reason) on the client: the game's own close of
    /// its match, for the reason the close came with.
    pub fn close(&self, reason: CloseReason) {
        // SAFETY: on the game's thread, at on_match_ready's end, whose
        // epilogue reads no field of the client; close_current_match is the
        // game's own (on_match_disconnected calls it the same way, 0x21BE9A),
        // and on_project_loaded's own abort unloads the world inside this
        // callback (0x1F8AFC).
        unsafe { raw::engine_close_current_match(Some(self.client), reason.0) }
    }
}
