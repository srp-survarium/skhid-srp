//! A replay played to a spectator's game as a spectated match (path A,
//! docs/notes/replay-stream.md; patch 130). The match server streams the
//! recording to a game with no place of its own (patch 121) and runs its
//! playback; the game asks for a change with an 0xE0 and applies the
//! playback the server tells it in an 0xF6 (the messages: the
//! `replay-control` crate). Here, what the game is for that:
//!
//! - `match_packet.server_note` (0x27A4F4): the 0xF6's payload;
//! - the game's clock: game::pause / game::resume (0x1ED5C0 / 0x1ED650:
//!   the game's floating_timer frozen at +52F0h, the render timer by the
//!   paused flag 0x1F1DA3C, the sound's time scale 0) and the console
//!   variable time_scale (0xA70298), which game::tick makes the timer's
//!   speed at every frame (0x1EC96C..0x1EC9A8): the replay controls' own;
//! - an 0xE0 of ours to the match server (the match link's new_packet / enqueue,
//!   as close_current_match's leave and the snapshot upload send theirs);
//! - `close_match.reason` (0x27A7F3): every end of a match.

use core::marker::PhantomData;

use replay_control::{MatchMs, Request};

use super::context::Context;
use super::hook::Thread;
use super::world::{read, NetworkClient, Role};
use crate::raw;

/// The 0xF6 at `match_packet.server_note`, before the game's own case: the
/// network client and the message's payload.
pub struct ServerNoteSite<'t> {
    pub client: NetworkClient<'t>,
    pub payload: &'t [u8],
}

/// The longest 0xF6 payload read (a multipart message's whole, transport.md).
const PAYLOAD_LIMIT: usize = 1 << 16;

impl<'t> ServerNoteSite<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<ServerNoteSite<'t>> {
        use raw::site::match_packet_server_note as site;
        // SAFETY: the context is this site's: ESI the network client the
        // dispatch runs on, EBX the message's reader {begin, cursor, size}
        // over its bytes, alive while the handler runs on the game's thread.
        let (client, payload) = unsafe {
            let (client, reader) = (site::client(ctx)?, site::reader(ctx)?);
            let (begin, cursor, size) = (reader.begin(), reader.cursor(), read(reader.size()) as usize);
            let len = (begin as usize).checked_add(size)?.checked_sub(cursor as usize)?;
            if begin.is_null() || cursor < begin || len > PAYLOAD_LIMIT {
                return None;
            }
            (client, core::slice::from_raw_parts(cursor.cast_const(), len))
        };
        Some(ServerNoteSite { client: NetworkClient::of(client, ctx.thread()), payload })
    }
}

/// close_current_match at `close_match.reason`, past its guard: the game's
/// match ends whatever the reason (patch 119 gives reason 4, a kick, reason
/// 1's end).
pub struct MatchClose<'t> {
    pub client: NetworkClient<'t>,
}

/// Reason 2, disconnected by the game: an empty 0xE0 (the leave) to the
/// match server first (0x27A828), then as 1: `31 <order> FF` to the lobby and
/// the world ended, no results.
const LEAVE: u32 = 2;

impl<'t> MatchClose<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<MatchClose<'t>> {
        use raw::site::close_match_reason as site;
        // SAFETY: the context is this site's: EDI the network client, on the game's thread.
        let client = unsafe { site::client(ctx)? };
        Some(MatchClose { client: NetworkClient::of(client, ctx.thread()) })
    }
}

/// The game's clock as a replay's viewer runs it: paused, and its speed.
/// One owner: only this sets the game's paused flag and time_scale in a
/// match (a live match's game never pauses: toggle_pause refuses a network
/// client, 0x1ED59C).
pub struct GameClock<'t> {
    game: raw::Game,
    thread: PhantomData<&'t Thread>,
}

impl<'t> GameClock<'t> {
    /// The clock of a spectator's game (patch 121's role); none for a
    /// player's or a replay's own client.
    pub fn of(client: &NetworkClient<'t>) -> Option<GameClock<'t>> {
        if client.role()? != Role::Spectator {
            return None;
        }
        // SAFETY: the client's game, alive as long as the client, on its thread.
        let game = unsafe { client.raw().game() }?;
        Some(GameClock { game, thread: PhantomData })
    }

    /// Stands or runs the clock and the world with it (game::pause /
    /// game::resume, each only when the flag says it is not so already).
    pub fn set_paused(&self, paused: bool) {
        // SAFETY: the process's paused flag, read on the game's thread, its only writer.
        let now = unsafe { read(raw::image::game_pause().paused()) } != 0;
        if now == paused {
            return;
        }
        // SAFETY: the game's own pause and resume, on its thread, as its
        // console's pause and the replay controls' play button call them.
        unsafe {
            if paused {
                raw::engine_game_pause(Some(self.game));
            } else {
                raw::engine_game_resume(Some(self.game));
            }
        }
    }

    /// The clock's speed (time_scale; game::tick gives the timer it from the
    /// next frame on that it is not paused).
    pub fn set_speed(&self, factor: f32) {
        // SAFETY: the console variable's value, a f32 in .data, the game's
        // thread's (game::tick reads it there; the console writes it there).
        unsafe { raw::image::time_scale().value().write_volatile(factor) }
    }

    /// The clock as every other match has it: running, at 1.
    pub fn restore(&self) {
        self.set_paused(false);
        self.set_speed(1.0);
    }
}

/// The game -> match server side of a replay's viewer.
impl<'t> NetworkClient<'t> {
    /// The match's clock now: the world's completed simulation time (the
    /// core's +18FC0h), when the game has a world.
    pub fn clock(&self) -> Option<MatchMs> {
        // SAFETY: the client's pvp_match and its core, alive while the match is, on its thread.
        unsafe { Some(MatchMs(read(self.raw().pvp_match()?.core().time()))) }
    }

    /// Sends `request` to the match server: an 0xE0 alone in its datagram,
    /// as the snapshot upload's chunks are (client_patch.md 3.2). Not when
    /// the game is no spectator, its link not up (state 2), or the packet
    /// pool could run dry (an empty pool is a crash in new_packet): whether
    /// it went.
    pub fn send_request(&self, request: Request) -> bool {
        if self.role() != Some(Role::Spectator) {
            return false;
        }
        let bytes = request.encode();
        // SAFETY: the client's match link and its pool, alive with the
        // match on the game's thread; new_packet's packet is ours until
        // enqueue takes it; its writer holds 0x1FC bytes (8 here).
        unsafe {
            let this = self.raw();
            if read(this.link_state()) != LINK_UP {
                return false;
            }
            let Some(link) = this.options_holder() else { return false };
            let Some(pool) = link.packet_pool() else { return false };
            if read(pool.used()).saturating_add(1 + POOL_RESERVE) > read(pool.capacity()) {
                return false;
            }
            let Some(packet) = raw::engine_match_packet_begin(Some(link), LEAVE_MESSAGE) else { return false };
            let flags = packet.flags();
            flags.write_volatile(read(flags) & !MAY_JOIN);
            raw::engine_append_packet_bytes(Some(packet.writer()), bytes.as_ptr(), bytes.len() as u32);
            raw::engine_match_packet_send(Some(link), Some(packet));
        }
        true
    }

    /// A spectator's game leaves its match, as the game's own leave does:
    /// close_current_match(2), an empty 0xE0 to the match server, then back
    /// to the lobby (the world ended, no results). A player's is not touched.
    pub fn leave(&self) {
        if self.role() != Some(Role::Spectator) {
            return;
        }
        // SAFETY: the client's own close_current_match, on the game's thread
        // from the game world's keys (as its Esc menu's leave reaches it).
        unsafe { raw::engine_close_current_match(Some(self.raw()), LEAVE) }
    }
}

/// The link's state once the static info came (every S->C handler past 0xE4 tests it).
const LINK_UP: u32 = 2;
/// The 0xE0, the leave; with a payload srp's own (a snapshot chunk, a request).
const LEAVE_MESSAGE: u32 = 0xE0;
/// The packet's "may be joined" bit (+98h bit 6): cleared, so the request is
/// alone in its datagram whatever -join_net_messages says.
const MAY_JOIN: u8 = 0x40;
/// Packets left for the game's own inputs, hashes and acks (the upload's 16).
const POOL_RESERVE: u32 = 16;

/// The game world's load at `replay_movie.query` and its callback at
/// `replay_movie.take`, where the game asks for the replay controls' movie
/// only with a replay reader: whether this game is a spectator's (its 0xE4
/// made no place local, patch 121), whose world gets the movie too, kept
/// hidden until it is a replay's viewer. Both sites answer alike for one
/// game, so the query and its results stay in step.
pub fn spectator_load(game: Option<raw::Game>, thread: &Thread) -> bool {
    // SAFETY: the game of the site's register, alive while it loads its
    // world on the game's thread; its client likewise.
    let Some(client) = game.and_then(|game| unsafe { game.network_client() }) else { return false };
    let client = NetworkClient::of(client, thread);
    client.role().is_some() && client.places() > 0 && client.local_profile().is_none()
}

/// At `replay_movie.query`.
pub fn query_spectator(ctx: &Context<'_>) -> bool {
    // SAFETY: the context is this site's: ECX the game.
    spectator_load(unsafe { raw::site::replay_movie_query::game(ctx) }, ctx.thread())
}

/// At `replay_movie.take`.
pub fn take_spectator(ctx: &Context<'_>) -> bool {
    // SAFETY: the context is this site's: EAX the game.
    spectator_load(unsafe { raw::site::replay_movie_take::game(ctx) }, ctx.thread())
}

/// The replay controls (replay_player.swf) over a viewer's hud.
pub struct Overlay<'t> {
    ui: raw::GameWorldUi,
    movie: super::ui::Movie<'t>,
}

impl<'t> Overlay<'t> {
    /// The controls of the client's game world's hud, when it has them.
    pub fn of(client: &NetworkClient<'t>) -> Option<Overlay<'t>> {
        // SAFETY: the client's game, its game world and the hud inside it,
        // alive with the match on the game's thread; the holder the hud's.
        unsafe {
            let ui = client.raw().game()?.world().ui();
            let movie = super::ui::movie_of(ui.replay_movie())?;
            Some(Overlay { ui, movie })
        }
    }

    /// Shows or hides the controls (show_replay_controls: the movie into or
    /// out of the scene; nothing when it already is so).
    pub fn show(&self, shown: bool) {
        // SAFETY: the hud's own call, on its thread (the replay client's F11 makes it).
        unsafe { raw::engine_show_replay_controls(Some(self.ui), u32::from(shown)) }
    }

    /// The bar's position, ms from the replay's start (root.set_position).
    pub fn set_position(&self, ms: u32) {
        self.movie.invoke_uints(c"root.set_position", &[ms]);
    }

    /// The play button's state (root.set_play_state: true paused, as the
    /// replay client's tick gives it the paused flag, 0x218C1E).
    pub fn set_paused(&self, paused: bool) {
        self.movie.invoke_bool(c"root.set_play_state", paused);
    }

    /// The hud's kit chooser hidden (root.show_profiles_selector(false)): a
    /// viewer picks no kit, and the game shows it before the start and on a
    /// death screen, over the controls' bar.
    pub fn hide_kits(&self) {
        // SAFETY: the hud's own movie, alive with the hud on its thread.
        if let Some(hud) = unsafe { super::ui::movie_of(self.ui.hud_movie()) } {
            hud.invoke_bool(c"root.show_profiles_selector", false);
        }
    }

    /// The bar's length, ms (root.set_match_time).
    pub fn set_length(&self, ms: u32) {
        self.movie.invoke_uints(c"root.set_match_time", &[ms]);
    }

    /// The markers, as the movie's text helper takes them
    /// (root.set_markers_text: `time_ms:kind:label;...`).
    pub fn set_markers(&self, text: &core::ffi::CStr) {
        self.movie.invoke_text(c"root.set_markers_text", text);
    }
}

/// The hud at `hud.update` (game_world_ui::update_ui's start, every frame).
pub struct HudFrame {
    ui: raw::GameWorldUi,
}

impl HudFrame {
    pub fn at(ctx: &Context<'_>) -> Option<HudFrame> {
        // SAFETY: the context is this site's: EAX the hud, alive while it updates on the game's thread.
        Some(HudFrame { ui: unsafe { raw::site::hud_update::ui(ctx) }? })
    }

    /// Its game's network client.
    pub fn client<'t>(&self, thread: &'t Thread) -> Option<NetworkClient<'t>> {
        // SAFETY: the hud's game world, its game and the game's client, alive with the hud.
        let client = unsafe { self.ui.world()?.game()?.network_client()? };
        Some(NetworkClient::of(client, thread))
    }
}
