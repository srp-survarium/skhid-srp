//! A spectator's game (patch 121, docs/notes/queue-spectate.md): a game in a
//! match whose 0xE4 named no place its own. process_static_match_info sets
//! each place's is_local (+45F0h) by the message's first byte (0x27BE5F); the
//! server sends 0xFF, so on_match_ready never sets the client's local place
//! (+1F1C44h stays 0xFF, 0x27AD89) and the game has no player of its own
//! ([`Role::Spectator`]). Where the game asks for that place's player it
//! gets an unchecked slot-table index (get_player 0x27A980); the sites here
//! are those it reaches in a match, each read once into its typed view:
//!
//! - `world_load.loaded` (0x27C116): after every world, the camera back to
//!   the local place;
//! - `keyboard.spectate` (0x25CDBD): the bindings next_player and
//!   prev_player, which the game takes only from a client that may spectate
//!   its teammates (+0Dh, which nothing sets in a live match's client);
//! - `chat.match_team` (0x208D77): a match line's team colour, by the local
//!   place's player;
//! - `hud.followed_alive` (0x23B7B9): the hud's change of the followed
//!   player's life (the kits and the cursor at a death);
//! - `player_left.local` (0x27C72C): an 0xEA, which the game takes as its
//!   own end only for its local place: a game with none returns at once.

use super::context::{Context, Reg};
use super::input::Action;
use super::world::{NetworkClient, Step};
use raw::site::static_info_options_read as static_info_site;
use crate::raw;

impl Action {
    /// cam_2 (the action table 0xA74C30, default F2): the replay's free view
    /// (on_keyboard_action 0x25CE4D -> vtable+84h, only for a client whose
    /// vtable+70h says so, a replay's).
    pub const FREE_VIEW: Action = Action(0x12);
    /// next_player (the action table 0xA74C80, default F6).
    pub const NEXT_PLAYER: Action = Action(0x17);
    /// prev_player (0xA74C94, default F5).
    pub const PREV_PLAYER: Action = Action(0x18);
}

/// The match's first facts at `static_info.options_read`: a game's role comes from them.
pub struct MatchStart<'t> {
    pub client: NetworkClient<'t>,
    /// The 0xE4's first byte.
    pub local_place: u8,
    /// The match's places.
    pub places: u8,
}

impl<'t> MatchStart<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<MatchStart<'t>> {
        // SAFETY: the context is this site's: EDI the network client (kept since 0x27B8DB), EBP
        // process_static_match_info's frame, ESI the options just read, alive with it.
        unsafe {
            let (client, frame, options) = (static_info_site::client(ctx)?, static_info_site::frame(ctx)?, static_info_site::options(ctx)?);
            Some(MatchStart {
                client: NetworkClient::of(client, ctx.thread()),
                local_place: frame.local_place().read(),
                places: options.places().read(),
            })
        }
    }
}

/// The client at `world_load.loaded`: a world was just loaded (or skipped).
pub fn world_camera_client<'t>(ctx: &'t Context<'_>) -> Option<NetworkClient<'t>> {
    // SAFETY: the context is this site's: EDI the network client
    // process_dynamic_match_info runs on, alive on the game's thread.
    let client = unsafe { raw::site::world_load_loaded::client(ctx) }?;
    Some(NetworkClient::of(client, ctx.thread()))
}

/// What a spectator's camera key asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraKey {
    /// Follow the next or the previous player (F6, F5).
    Follow(Step),
    /// Let go of the player followed and fly freely (F2).
    FlyFree,
}

/// A key's action at `keyboard.spectate`, with the game's client.
pub struct SpectateKey<'t> {
    pub client: NetworkClient<'t>,
    /// The camera key the action is; `None` for any other action.
    pub key: Option<CameraKey>,
}

impl<'t> SpectateKey<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<SpectateKey<'t>> {
        use raw::site::keyboard_spectate as site;
        // SAFETY: the context is this site's: EBX the game, EDI the action,
        // the game's client alive with it on the game's thread.
        let (game, action) = unsafe { (site::game(ctx)?, site::action(ctx)) };
        // SAFETY: as above.
        let client = unsafe { game.network_client() }?;
        let key = match Action(action) {
            Action::NEXT_PLAYER => Some(CameraKey::Follow(Step::Next)),
            Action::PREV_PLAYER => Some(CameraKey::Follow(Step::Previous)),
            Action::FREE_VIEW => Some(CameraKey::FlyFree),
            _ => None,
        };
        Some(SpectateKey { client: NetworkClient::of(client, ctx.thread()), key })
    }
}

/// A match line at `chat.match_team`, before add_message asks for the local
/// place's player.
pub struct MatchLine<'t> {
    pub client: NetworkClient<'t>,
    frame: raw::AddMessageFrame,
}

impl<'t> MatchLine<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<MatchLine<'t>> {
        use raw::site::chat_match_team as site;
        // SAFETY: the context is this site's: ESI the game's network client,
        // its ESP add_message's frame, alive while it runs.
        let (client, frame) = unsafe { (site::client(ctx)?, site::frame(ctx)?) };
        Some(MatchLine { client: NetworkClient::of(client, ctx.thread()), frame })
    }

    /// The line goes on as one of the reader's own team (the site's exit
    /// `plain`, 0x208DE2), with no player asked for: the local the release
    /// at 0x208E27 reads is 0, as for no player.
    pub fn as_plain(self) {
        // SAFETY: the frame's local, the game's to write before 0x208D86's
        // call would; 0 is its "no player".
        unsafe { self.frame.set_team_player(None) }
    }
}

/// A place that left (0xEA) at `player_left.local`, after leave_match.
pub struct PlayerLeft<'t> {
    pub client: NetworkClient<'t>,
    /// The place the 0xEA names; 0xFF a spectator's own end (the match
    /// server's farewell to a game with no place).
    pub place: u8,
}

impl<'t> PlayerLeft<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<PlayerLeft<'t>> {
        use raw::site::player_left_local as site;
        // SAFETY: the context is this site's: ESI the network client the
        // handler runs on, BL the place it read (0x27C6F9).
        let (client, place) = unsafe { (site::client(ctx)?, site::place(ctx) as u8) };
        Some(PlayerLeft { client: NetworkClient::of(client, ctx.thread()), place })
    }
}

/// The hud's read of the followed player's life at `hud.followed_alive`.
pub struct FollowedAlive<'c, 'm> {
    ctx: &'c mut Context<'m>,
}

impl<'c, 'm> FollowedAlive<'c, 'm> {
    pub fn at(ctx: &'c mut Context<'m>) -> FollowedAlive<'c, 'm> {
        FollowedAlive { ctx }
    }

    /// The game's client, from the hud: its game world's game's.
    pub fn client(&self) -> Option<NetworkClient<'_>> {
        // SAFETY: the context is this site's: ESI the hud, its game world,
        // the world's game and client, alive on the game's thread.
        let client = unsafe { raw::site::hud_followed_alive::ui(&*self.ctx)?.world()?.game()?.network_client() }?;
        Some(NetworkClient::of(client, self.ctx.thread()))
    }

    /// The hud takes the player for alive (AL 1, the overwritten read
    /// skipped): it shows no kits and no cursor at the player's death.
    pub fn as_alive(&mut self) {
        self.ctx.set_reg8(Reg::Eax, 1);
    }
}
