//! A replay played to a spectator's game as a spectated match (path A,
//! docs/notes/replay-stream.md; patch 144). The match server streams the
//! recording to a game with no place of its own (patch 143) and runs its
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

use core::ffi::CStr;
use core::marker::PhantomData;

use replay_control::{MatchMs, Request};

use super::context::Context;
use super::hook::Thread;
use super::types::Place;
use super::world::{read, NetworkClient};
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

/// `hud.setup_profiles`: the kits initialize_match has just built for the hud.
pub struct KitFeed {
    place: raw::MatchPlace,
}

impl KitFeed {
    pub fn at(ctx: &Context<'_>) -> Option<KitFeed> {
        // SAFETY: the context is this site's: ESP initialize_match's frame, whose place it set
        // before its loop over that place's kits, alive on the game's thread.
        Some(KitFeed { place: unsafe { raw::site::hud_setup_profiles::frame(ctx)?.place() }? })
    }

    /// The place is the game's own, by the byte initialize_match chose it with (0x234949).
    pub fn is_the_games_own(&self) -> bool {
        // SAFETY: a place of the match's options, alive with the match on the game's thread.
        unsafe { read(self.place.is_local()) != 0 }
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
    /// The clock of a spectator's game (patch 143's role); none for a
    /// player's or a replay's own client.
    pub fn of(client: &NetworkClient<'t>) -> Option<GameClock<'t>> {
        if !client.is_spectator() {
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
        if !self.is_spectator() {
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
        if !self.is_spectator() {
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
/// only with a replay reader: whether this game is a network game, whose
/// world gets the movie too, kept hidden until it is a replay's viewer. The
/// game queries its movies at its first world only (game_world::load skips
/// them once its +4 is set, 0x1F8D5E), so a game that played before it
/// watches has them from its first match. Both sites answer alike for one
/// game, so the query and its results stay in step.
pub fn network_load(game: Option<raw::Game>, thread: &Thread) -> bool {
    // SAFETY: the game of the site's register, alive while it loads its
    // world on the game's thread; its client likewise.
    let Some(client) = game.and_then(|game| unsafe { game.network_client() }) else { return false };
    NetworkClient::of(client, thread).is_match_client()
}

/// At `replay_movie.query`.
pub fn query_network(ctx: &Context<'_>) -> bool {
    // SAFETY: the context is this site's: ECX the game.
    network_load(unsafe { raw::site::replay_movie_query::game(ctx) }, ctx.thread())
}

/// At `replay_movie.take`.
pub fn take_network(ctx: &Context<'_>) -> bool {
    // SAFETY: the context is this site's: EAX the game.
    network_load(unsafe { raw::site::replay_movie_take::game(ctx) }, ctx.thread())
}

/// The network client of the game whose hud's mouse is at
/// `hud_mouse.replay_button` or `hud_mouse.replay_pointer` (its hud has
/// the replay controls); none for the game's own replay client.
fn mouse_client<'t>(world: Option<raw::GameWorld>, thread: &'t Thread) -> Option<NetworkClient<'t>> {
    // SAFETY: the game world of the site's register, its game and client,
    // alive while the input world dispatches the mouse on the game's thread.
    let client = unsafe { world?.game()?.network_client() }?;
    let client = NetworkClient::of(client, thread);
    client.is_match_client().then_some(client)
}

/// At `hud_mouse.replay_button`.
pub fn button_client<'t>(ctx: &'t Context<'_>) -> Option<NetworkClient<'t>> {
    // SAFETY: the context is this site's: ESI the game world's input
    // handler, the hud inside it, the hud's game world.
    let world = unsafe { raw::site::hud_mouse_replay_button::input(ctx).and_then(|input| input.ui().world()) };
    mouse_client(world, ctx.thread())
}

/// At `hud_mouse.replay_pointer`.
pub fn pointer_client<'t>(ctx: &'t Context<'_>) -> Option<NetworkClient<'t>> {
    // SAFETY: the context is this site's: EDI the game world.
    mouse_client(unsafe { raw::site::hud_mouse_replay_pointer::world(ctx) }, ctx.thread())
}

/// `ui`'s quick slots hidden (root.show_slots(false)): a spectator uses no slot, and the game
/// shows them for the player followed (0x239C8D).
fn hide_player_panels(ui: raw::GameWorldUi) {
    // SAFETY: the hud's own movie, alive with the hud on its thread.
    if let Some(hud) = unsafe { super::ui::movie_of(ui.hud_movie()) } {
        hud.invoke_bool(c"root.show_slots", false);
    }
}

/// The call back's words, a dictionary key: its text has `%1$s` where the seconds go.
const CALLED_BACK: &CStr = c"st_srp_rooms_called_back_in";

/// A spectator's read-only kit chooser: the hud movie's own (`HUD.as`, srp_spectator_*), filled
/// with the kits the server's notes allow, which the game alone has the contents of.
pub struct KitChooser<'t> {
    ui: raw::GameWorldUi,
    movie: super::ui::Movie<'t>,
}

/// The kit chooser's place and whether it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChooserMode {
    Hidden,
    /// In the game's own place (a live spectator's).
    Live,
    /// Above a replay viewer's controls.
    Replay,
}

impl ChooserMode {
    /// root.srp_spectator_kits_show's argument.
    fn value(self) -> u32 {
        match self {
            ChooserMode::Hidden => 0,
            ChooserMode::Live => 1,
            ChooserMode::Replay => 2,
        }
    }
}

impl<'t> KitChooser<'t> {
    pub fn of(client: &NetworkClient<'t>) -> Option<KitChooser<'t>> {
        // SAFETY: as Overlay::of.
        unsafe {
            let ui = client.raw().game()?.world().ui();
            Some(KitChooser { ui, movie: super::ui::movie_of(ui.hud_movie())? })
        }
    }

    /// `kit` as initialize_match names it for the hud (0x235A7B): its own name, or the running
    /// language's default for its number when it has none, kept in `room`.
    pub fn named<'k>(&self, mut kit: KitData<'k>, room: &'k mut [u8; 0x200]) -> KitData<'k> {
        if let (true, Some(key)) = (kit.name.is_empty(), KIT_DEFAULT_NAMES.get(usize::from(kit.number))) {
            kit.name = super::ui::GameWorldUi::of(self.ui).translate(key, room).to_bytes();
        }
        kit
    }

    /// The kits' text (root.srp_spectator_kits).
    pub fn set_kits(&self, text: &CStr) {
        self.movie.invoke_text(c"root.srp_spectator_kits", text);
    }

    /// The kit the followed player has now (root.srp_spectator_current_kit).
    pub fn set_current(&self, kit: u8) {
        self.movie.invoke_uints(c"root.srp_spectator_current_kit", &[u32::from(kit)]);
    }

    /// The watcher's room calls it back, the game let go in `seconds`: the hud says so, in the
    /// running language, and counts them down (root.srp_called_back_words, root.srp_called_back).
    pub fn called_back(&self, seconds: u8) {
        let mut room = [0; 0x200];
        let words = super::ui::GameWorldUi::of(self.ui).translate(CALLED_BACK, &mut room);
        self.movie.invoke_text(c"root.srp_called_back_words", words);
        self.movie.invoke_uints(c"root.srp_called_back", &[u32::from(seconds)]);
    }

    /// The chooser's place and whether it shows (root.srp_spectator_kits_show).
    pub fn show(&self, mode: ChooserMode) {
        self.movie.invoke_uints(c"root.srp_spectator_kits_show", &[mode.value()]);
    }

    /// The pointer, for the hover list (root.srp_spectator_pointer).
    pub fn pointer(&self, x: u32, y: u32) {
        self.movie.invoke_uints(c"root.srp_spectator_pointer", &[x, y]);
    }

    /// The game's own show_cursor kept from the kits. The chooser has no cursor of its own.
    pub fn ui(&self) -> raw::GameWorldUi {
        self.ui
    }
}

/// One kit of a place, as the game keeps it in the match's options.
pub struct KitData<'a> {
    pub number: u8,
    pub state: u32,
    pub level: u16,
    pub name: &'a [u8],
    /// Its held items: (dictionary id, slot, count).
    pub items: [Option<(u16, u8, u16)>; KIT_SLOTS],
}

/// A kit's slots (`survarium::dictionary::PROFILE_SLOTS`).
pub const KIT_SLOTS: usize = 23;

/// A kit name's room, its zero included.
const KIT_NAME_LIMIT: usize = 64;

/// A kit's name when it has none of its own, by its number (the exe's table, 0xA7482C).
const KIT_DEFAULT_NAMES: [&CStr; 3] = [c"st_default_profile_name1", c"st_default_profile_name2", c"st_default_profile_name3"];

/// `ui`'s cursor as asked (show_cursor 0x23A930, when the hud's +2DCh says otherwise): the
/// kits show it before the start, and a spectator has none to pick. While the Tab list is up,
/// patch 86's cursor is left as it is.
fn keep_cursor(ui: raw::GameWorldUi, shown: bool) {
    // SAFETY: the hud's own bytes and its own call, on its thread.
    unsafe {
        if *ui.list_shown() == 0 && (*ui.cursor_shown() != 0) != shown {
            raw::engine_show_cursor(Some(ui), u32::from(shown));
        }
    }
}

impl<'t> NetworkClient<'t> {
    /// Kit `number` of `place`, when the options have it (read from the game's own copy).
    pub fn kit_data(&self, place: Place, number: u8) -> Option<KitData<'t>> {
        // SAFETY: the client's options and their places live with the client on its thread; the
        // indices are bounded by the counts the options give.
        unsafe {
            let options = self.raw().options_holder()?.options();
            if u32::from(place.get()) >= u32::from(*options.places()) {
                return None;
            }
            let record = options.first_place().at(place.index());
            if number >= *record.kit_count() {
                return None;
            }
            let kit = record.kit().at(usize::from(number));
            let name = super::ui::c_string(kit.name(), KIT_NAME_LIMIT).map_or(&b""[..], CStr::to_bytes);
            let config = self.raw().game()?.kit_level_config();
            let level = (raw::engine_kit_level(u32::from(number), Some(record), config) & 0xFFFF) as u16;
            let mut items = [None; KIT_SLOTS];
            let count = *record.item_count();
            for (slot, room) in items.iter_mut().enumerate() {
                let entry = kit.slots().at(slot);
                let item = read(entry.item());
                if item == NO_ITEM || item >= u16::from(count) {
                    continue;
                }
                *room = Some((read(record.item().at(usize::from(item)).dict_id()), slot as u8, read(entry.count())));
            }
            Some(KitData { number, state: read(kit.state()), level, name, items })
        }
    }

    /// The kit `place` has now (its client record's current player object).
    pub fn current_kit(&self, place: Place) -> Option<u8> {
        // SAFETY: the match's table of its places' records, a place below the options' count.
        unsafe { Some(read(self.raw().pvp_match()?.clients()?.at(place.index()).current_kit())) }
    }
}

const NO_ITEM: u16 = 0xFFFF;

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

    /// The hud's kit chooser and quick slots hidden: a viewer's.
    pub fn hide_player_panels(&self) {
        hide_player_panels(self.ui);
    }

    /// The hud's cursor as the viewer wants it.
    pub fn keep_cursor(&self, shown: bool) {
        keep_cursor(self.ui, shown);
    }

    /// The controls and the cursor's movie let go of every button they still hold. Both get a viewer's
    /// presses, and neither its release once the viewer has ended: the mouse goes back to the lobby's
    /// movie, and these two outlive the match.
    pub fn release_held_buttons(&self) {
        self.movie.release_held_buttons();
        // SAFETY: the hud's own slot; its movie, alive with the hud on its thread.
        if let Some(cursor) = unsafe { super::ui::movie_of(self.ui.cursor_movie().holder()) } {
            cursor.release_held_buttons();
        }
    }

    /// The speed stepper's value, as a percent (root.set_replay_speed: 25 to
    /// 800, the movie answers nothing).
    pub fn set_speed(&self, percent: u16) {
        self.movie.invoke_uints(c"root.set_replay_speed", &[u32::from(percent)]);
    }

    /// The bar's length, ms (root.set_match_time).
    pub fn set_length(&self, ms: u32) {
        self.movie.invoke_uints(c"root.set_match_time", &[ms]);
    }

    /// The markers, as the movie's text helper takes them
    /// (root.set_markers_text: `time_ms:kind:whose:actor:victim;...`).
    pub fn set_markers(&self, text: &CStr) {
        self.movie.invoke_text(c"root.set_markers_text", text);
    }

    /// The markers' names by place (root.set_marker_names: `place:name;...`),
    /// told before their markers.
    pub fn set_marker_names(&self, text: &CStr) {
        self.movie.invoke_text(c"root.set_marker_names", text);
    }

    /// Which movie this is (a new game world's is another).
    pub fn id(&self) -> usize {
        self.movie.id()
    }

    /// The controls made a viewer's (root.start_viewer: the Back button, the
    /// Moments switch and the key hint, shown its first 10 s), with the
    /// dictionary texts of `keys` in the running language; whether the movie
    /// took it (its root is made).
    pub fn start_viewer<const N: usize>(&self, keys: &[&CStr; N]) -> bool {
        let ui = super::ui::GameWorldUi::of(self.ui);
        let mut rooms = [[0u8; 0x200]; N];
        let mut texts = [c""; N];
        for ((room, key), text) in rooms.iter_mut().zip(keys).zip(texts.iter_mut()) {
            *text = ui.translate(key, room);
        }
        self.movie.invoke_strings(c"root.start_viewer", &texts)
    }

    /// The Moments switch's choice (root.set_moments), its Mine shown only to a viewer who played.
    pub fn set_moments(&self, choice: Moments, mine: bool) {
        self.movie.invoke_uints(c"root.set_moments", &[choice.value(), u32::from(mine)]);
    }

    /// The hud's cursor shown: the key hint with it (root.set_cursor_shown).
    pub fn set_cursor_shown(&self, shown: bool) {
        self.movie.invoke_bool(c"root.set_cursor_shown", shown);
    }
}

/// Which of a viewer's events the Moments switch lights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moments {
    All,
    /// The viewer's own: it played in the match.
    Mine,
    /// The player in view's.
    Followed,
}

impl Moments {
    /// The switch's index of the choice, as the movie has it (root.set_moments, moments_clicked).
    fn value(self) -> u32 {
        match self {
            Moments::All => 0,
            Moments::Mine => 1,
            Moments::Followed => 2,
        }
    }

    /// A choice the movie clicked (moments_clicked).
    pub fn of_value(index: u32) -> Option<Moments> {
        Some(match index {
            0 => Moments::All,
            1 => Moments::Mine,
            2 => Moments::Followed,
            _ => return None,
        })
    }
}

/// The pointer at `hud_mouse.replay_pointer`, in the screen's pixels.
pub fn pointer_at(ctx: &Context<'_>) -> Option<(u32, u32)> {
    // SAFETY: the context is this site's: EBX and EBP the pointer's coordinates' cells, set_pointer's arguments.
    unsafe {
        let (x, y) = (raw::site::hud_mouse_replay_pointer::x(ctx)?, raw::site::hud_mouse_replay_pointer::y(ctx)?);
        Some((u32::try_from(read(x.value())).ok()?, u32::try_from(read(y.value())).ok()?))
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

    /// The hud's kit chooser and quick slots hidden, for a spectator of a live match, who may
    /// not have the replay controls' movie.
    pub fn hide_player_panels(&self) {
        hide_player_panels(self.ui);
    }

    /// Its game's network client.
    pub fn client<'t>(&self, thread: &'t Thread) -> Option<NetworkClient<'t>> {
        // SAFETY: the hud's game world, its game and the game's client, alive with the hud.
        let client = unsafe { self.ui.world()?.game()?.network_client()? };
        Some(NetworkClient::of(client, thread))
    }
}

/// A world the 0xE8 handler is about to load (game_world_core::deserialize
/// 0x2D0B70), its arguments pushed: the mode it loads with.
pub struct CoreLoad {
    args: *mut u32,
}

impl CoreLoad {
    /// At `world_load.unpacked`.
    pub fn at_unpacked(ctx: &Context<'_>) -> Option<CoreLoad> {
        // SAFETY: the context is this site's: at ESP the load's arguments
        // (the core, the reader, the mode, 8), pushed at 0x27C0B5..0x27C0CF.
        Some(CoreLoad { args: unsafe { raw::site::world_load_unpacked::args(ctx)?.mode() } })
    }

    /// At `world_load.raw_args`.
    pub fn at_raw(ctx: &Context<'_>) -> Option<CoreLoad> {
        // SAFETY: the context is this site's: at ESP the reader, the mode, 8
        // (pushed at 0x27C105..0x27C109).
        Some(CoreLoad { args: unsafe { raw::site::world_load_raw_args::args(ctx)?.mode() } })
    }

    /// Loads it as the replay's own seek does (mode 0, 0x1EE377): the core's
    /// pending masks, timed entries and inputs are cleared first. A network
    /// world (mode 1) keeps them, and those of a later timeline make the
    /// load read a null entry (0x2D0DA4) when the world is older.
    pub fn as_seek(self) {
        // SAFETY: the pushed argument, the game's own stack slot, read by
        // the call that follows the site.
        unsafe { self.args.write(SEEK_MODE) }
    }
}

/// game_world_core::deserialize's mode of a user seek (0x1EE359..0x1EE36A).
const SEEK_MODE: u32 = 0;

/// The match's players' names, by place (the options' profiles, as the Tab
/// list shows them), for the controls' markers.
pub struct Names<'t> {
    lobby: super::lobby::Lobby<'t>,
}

impl<'t> Names<'t> {
    pub fn of(client: &NetworkClient<'t>) -> Names<'t> {
        Names { lobby: super::lobby::Lobby::of(client.raw()) }
    }

    /// The name of the player at `place`, when the match has one.
    pub fn of_place(&self, place: Place) -> Option<&[u8]> {
        let name = self.lobby.place_name(place)?;
        Some(name.to_bytes()).filter(|name| !name.is_empty())
    }

    /// The game's own account's name (the lobby's).
    pub fn own(&self) -> Option<&[u8]> {
        self.lobby.own_name().map(CStr::to_bytes)
    }
}
