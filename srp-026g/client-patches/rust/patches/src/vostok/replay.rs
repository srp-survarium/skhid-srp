//! A `.sur` replay the game plays, at its world's frame end
//! (`game_world.after_move`): its clock and its length, the seek its replay
//! player's slider makes, the place the camera follows, the replay's controls
//! and the game's own exit (patch 143, docs/notes/replay-director.md).
//!
//! Only a world whose game plays a replay gives one ([`GameWorld::replay`]:
//! its network client's class and the game's replay reader), so none of this
//! reaches a live match.

use super::hook::Thread;
use super::types::{GameTime, Place};
use super::world::{GameWorld, NetworkClient, Playback, PlayerId, read};
use crate::raw;

/// The replay a world's game plays.
pub struct Replay<'t> {
    world: raw::GameWorld,
    game: raw::Game,
    client: raw::NetworkClient,
    reader: raw::ReplayMatchReader,
    thread: &'t Thread,
}

impl<'t> GameWorld<'t> {
    /// The replay this world's game plays: its network client is the replay's
    /// (its class) and the game has the replay's reader (only `-match_replay`
    /// makes one). None in a live match.
    pub fn replay(&self) -> Option<Replay<'t>> {
        if self.playback() != Playback::Replay {
            return None;
        }
        // SAFETY: the world, its game, the game's client and replay reader,
        // alive for the hook on its thread (the reader as long as the game).
        unsafe {
            let game = self.this.game()?;
            Some(Replay {
                world: self.this,
                game,
                client: game.network_client()?,
                reader: game.replay_reader()?,
                thread: self.thread(),
            })
        }
    }
}

impl Replay<'_> {
    /// The game's clock, the match's time the replay is at.
    pub fn clock(&self) -> GameTime {
        // SAFETY: the game, alive for the hook.
        GameTime(unsafe { read(self.game.clock()) })
    }

    /// The replay's first and last times (its body's first and last
    /// records): at the last, the game seeks to the first and pauses.
    pub fn span(&self) -> (GameTime, GameTime) {
        // SAFETY: the reader, alive as long as the game.
        unsafe { (GameTime(read(self.reader.start())), GameTime(read(self.reader.end()))) }
    }

    /// Seeks to `to` as the replay player's slider does (a user seek, at
    /// once): the key frame at or before it loaded, the replay read on to
    /// it, the clock set to it. Done at the frame's end, never from a key
    /// mid-frame (docs/notes/replay-keys.md).
    pub fn seek(&self, to: GameTime) {
        let (start, _) = self.span();
        // SAFETY: the game's own seek, on its thread, at its frame's end; the
        // target is ms from the replay's start; 0 is the slider's user seek.
        unsafe { raw::engine_replay_seek_to(Some(self.game), to.since(start), 0) }
    }

    /// The player the camera is on (the client's local player: in a replay
    /// the one followed), if any.
    pub fn followed(&self) -> Option<PlayerId> {
        let client = NetworkClient::of(self.client, self.thread);
        client.local_player().map(|player| player.id())
    }

    /// The player of a place in the match now, if it is in.
    pub fn player(&self, place: Place) -> Option<PlayerId> {
        let client = NetworkClient::of(self.client, self.thread);
        client.player(u32::from(place.get())).map(|player| player.id())
    }

    /// Whether two places of the roster are one player's: the same name (a
    /// player who left and came back has a place each time).
    pub fn same_player(&self, place: Place, other: Place) -> bool {
        let client = NetworkClient::of(self.client, self.thread);
        let name = client.place_name(place);
        place == other || (name[0] != 0 && client.place_name(other) == name)
    }

    /// Follows `place` in the first person, as the replay player's list does
    /// (`select_player(place, 0)`): nothing when the place is not in the
    /// match.
    pub fn follow_first_person(&self, place: Place) {
        // SAFETY: the replay client's own follow (base_local_network_client's,
        // the replay client's slot +7Ch), with its world and a place below 20.
        unsafe { raw::engine_follow_first_person(Some(self.client), Some(self.world), u32::from(place.get())) }
    }

    /// Hides the replay's controls (the replay player movie, F11's) and the
    /// hud's cursor; nothing when they are hidden.
    pub fn hide_controls(&self) {
        // SAFETY: the world's hud, alive as long as the world; each call does
        // nothing when its flag is already 0.
        unsafe {
            let ui = self.world.ui();
            raw::engine_show_replay_player(Some(ui), 0);
            raw::engine_show_cursor(Some(ui), 0);
        }
    }

    /// Quits the game as the replay's own close does
    /// (replay_network_client::close_current_match 0x2193D6: game::exit with
    /// the `quit` console command's text).
    pub fn quit(&self) {
        // SAFETY: the game's exit, on its thread, with a C string.
        unsafe { raw::engine_game_exit(Some(self.game), c"quit".as_ptr().cast()) }
    }
}
