//! A spectator's game (patch 143, docs/notes/queue-spectate.md): a game in a
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
//!   player's life (the kits and the cursor at a death), and patch 31's
//!   ui_start after it (the kits and the cursor before play);
//! - `player_left.entry` (0x27C690): an 0xEA, process_player_left_match
//!   replaced whole (the game takes it as its own end only for its local
//!   place: a game with none returned at once).

use super::context::{Context, Reg};
use super::input::Action;
use super::types::Place;
use super::world::{read, NetworkClient, Step};
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

/// Who an 0xEA says left: the spectator's own end (the match server's farewell to a game with no
/// place, place 0xFF), or a place of the match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Left {
    Spectator,
    Place(Place),
}

impl Left {
    /// The 0xEA's byte, once at the boundary.
    fn of(byte: u8) -> Option<Left> {
        if byte == 0xFF { Some(Left::Spectator) } else { Place::new(u32::from(byte)).map(Left::Place) }
    }
}

/// An 0xEA, taken in by Rust at process_player_left_match's start (0x27C690, replaced whole): the
/// message read and its leave and statistics done, as the game's own code does them, up to what
/// the game does with the leave (the branch the spectator's game takes its own end from).
pub struct PlayerLeft<'t> {
    pub client: NetworkClient<'t>,
    /// The byte the 0xEA named (as the game read it).
    pub place: u8,
    /// Who it says left; `None` is a place this build does not know.
    pub left: Option<Left>,
}

/// An 0xEA with no match (no pvp_match, 0x27C6A2): the game read it to its end and told its stats.
pub enum PlayerLeftEvent<'t> {
    NoMatch,
    Left(PlayerLeft<'t>),
}

impl<'t> PlayerLeftEvent<'t> {
    /// The 0xEA, handled up to the leave: at `player_left.entry`, run once.
    pub fn at(ctx: &'t Context<'_>) -> Option<PlayerLeftEvent<'t>> {
        use raw::site::player_left_entry as site;
        // SAFETY: the context is this site's: ESI the network client the handler runs on, its ESP
        // process_player_left_match's frame (the reader its argument), alive while it runs on
        // the game's thread.
        let (client, reader) = unsafe { (site::client(ctx)?, site::frame(ctx)?.reader()?) };
        let game_client = NetworkClient::of(client, ctx.thread());
        // SAFETY: the client's match, alive for the hook.
        let Some(pvp) = (unsafe { client.pvp_match() }) else {
            // 0x27C6A4..0x27C6C0: the message read to its end, then its stats (5 for no match).
            // SAFETY: the reader {begin, cursor, size} the handler was given; the stats call is
            // the game's own, on its thread, its arguments the registers it takes.
            unsafe {
                reader.set_cursor(reader.begin().add(*reader.size() as usize));
                raw::engine_register_message_stats(5, Some(client), LEFT_MESSAGE);
            }
            return Some(PlayerLeftEvent::NoMatch);
        };
        // 0x27C6C5..0x27C727: `EA <tick byte> <place>`: leave_match at the tick the message names
        // (its low byte, widened by the wrap against the world's own tick) and the message's stats.
        // SAFETY: as above: the message's two bytes; the world core and the time are the game's
        // match's; leave_match and the stats are the game's own, the arguments as it passes them.
        let place = unsafe {
            let cursor = reader.cursor();
            let (tick, place) = (*cursor, *cursor.add(1));
            reader.set_cursor(cursor.add(2));
            let ticks = read(pvp.core().time()) / TICK_MS;
            let low = ticks as u8;
            let delta = raw::engine_tick_wrap_delta(&tick, &low) as i32;
            let requested = (ticks as i32 + delta).wrapping_mul(TICK_MS as i32) as u32;
            let result = raw::engine_leave_match(requested, Some(pvp.core()), u32::from(place));
            raw::engine_register_message_stats(result, Some(client), LEFT_MESSAGE);
            place
        };
        Some(PlayerLeftEvent::Left(PlayerLeft { client: game_client, place, left: Left::of(place) }))
    }
}

/// The 0xEA.
const LEFT_MESSAGE: u32 = 0xEA;
/// The simulation's step in ms (world time / this is a tick; 0x51EB851F's division, 0x27C6CA).
const TICK_MS: u32 = 50;

impl NetworkClient<'_> {
    /// A player's game, an 0xEA of `place` taken (its match's end): the game's own branch from
    /// 0x27C72C on. Its own leave: `31 <order> FF` to the lobby (unless a journal replays), then
    /// close_current_match(3), the results. The followed player's leave: follow its place again and
    /// step to the next (0x27C776..0x27C79D). Else nothing. A game with no local place returns.
    pub fn finish_player_left(&self, place: u8) {
        // SAFETY: the client's fields, on its thread; its local player, alive for the hook; the
        // journal's flag and state cells, plain words of the image, read on the game's thread.
        unsafe {
            let this = self.raw();
            let local = read(this.local_place());
            if local == NO_PLACE {
                return;
            }
            if local == place {
                let present = (raw::image::JOURNAL_PRESENT as usize as *const u32).read_volatile();
                let state = (raw::image::JOURNAL_POINTER as usize as *const *const u32).read_volatile();
                let replaying = present != 0 && state.read_volatile() == JOURNAL_REPLAYING;
                if !replaying {
                    raw::engine_discard_playing_order(Some(this.lobby()), 1);
                }
                raw::engine_close_current_match(Some(this), CLOSE_LEFT);
                return;
            }
            let Some(followed) = this.local_player() else { return };
            if read(followed.place()) != place {
                return;
            }
            let mut out = None;
            raw::engine_get_player(Some(this), &mut out, u32::from(local));
            raw::engine_follow_player(Some(this), out);
            raw::engine_spectate_next_player(Some(this));
        }
    }
}

/// The local place byte's "none".
const NO_PLACE: u8 = 0xFF;
/// journal_state: replaying.
const JOURNAL_REPLAYING: u32 = 2;
/// close_current_match's reason 3: the local player left, the results shown.
const CLOSE_LEFT: u32 = 3;

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

    /// The hud's panels left as they are: its exit `kept` (0x23B802) is past their change and past
    /// patch 31's `call ui_start` at 0x23B7FC (which, for a live followed player, would show the
    /// kits and the hud's cursor while the client's time state is not 3), with EAX the player's
    /// reference count, as the `lea eax,[ebp+0D0h]` there leaves it for the release that follows.
    /// `None` when the site has no player, which the game never gives it (0x23B75E).
    pub fn keep_panels(&mut self) -> Option<()> {
        // SAFETY: the context is this site's: EBP the followed player, a reference held.
        let references = unsafe { raw::site::hud_followed_alive::player(&*self.ctx)?.references() };
        self.ctx.set_reg(Reg::Eax, references as usize as u32);
        Some(())
    }
}

#[cfg(test)]
mod left_tests {
    use super::{Left, Place};

    #[test]
    fn the_byte_is_a_place_or_the_spectators_own_end() {
        assert_eq!(Left::of(0xFF), Some(Left::Spectator));
        assert_eq!(Left::of(0), Place::new(0).map(Left::Place));
        assert_eq!(Left::of(19), Place::new(19).map(Left::Place));
        assert_eq!(Left::of(20), None);
        assert_eq!(Left::of(0xFE), None);
    }
}
