//! The lobby menu's scene (patch 81, docs/notes/lobby-scenes.md): the lobby
//! menu asks for its scene once, at the game's start, in one query with its
//! movies (lobby_menu::query_resources 0x21E450), and takes the results in
//! on_render_scenes_ready (0x220B20). At `lobby_menu.scene_request` the query's
//! list is built and not yet asked: its fourth request (the project) can be
//! another lobby's project.
//! At `lobby_menu.scenes_ready` the results are in and nothing has used them:
//! the lobby may ask the whole query again instead.
//!
//! A change of lobby during a session: behind the lobby movie's black
//! ([`Shade`], `srp_scene_fade`) the lobby asks its query again
//! ([`DrawnLobby::ask`]) with only its project in it
//! ([`SceneQuery::ask_alone`]), and puts the project in the old one's place
//! ([`ScenesReady::swap_in`]): the project halves of
//! lobby_menu::clear_resources (0x21D080) and on_render_scenes_ready
//! (0x220B20), in their order. `0xA1`'s srp bytes are the lobby picks
//! ([`Picks`]); the exe sends the saved one with the game's own ranking
//! request ([`SquadId::send`]).

use core::ffi::CStr;
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::Context;
use super::hook::Thread;
use super::parse::Reader;
use super::ui::movie_of;
use crate::raw;

/// A resource class of the game's (its cook's id).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
enum ResourceClass {
    /// A binary config (the lobby's `default.player`, 1Fh).
    BinaryConfig = 0x1F,
    /// A game project (the lobby's `lobby_scene/day`, 40h: project_cooker_simple).
    GameProject = 0x40,
}

/// The lobby's query at `lobby_menu.scene_request`.
pub struct SceneQuery<'t> {
    frame: raw::LobbyQueryFrame,
    _thread: &'t Thread,
}

impl<'t> SceneQuery<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<SceneQuery<'t>> {
        // SAFETY: the context is this site's: query_resources' frame (EBP),
        // alive while it runs on the game's thread.
        let frame = unsafe { raw::site::lobby_menu_scene_request::frame(ctx)? };
        Some(SceneQuery { frame, _thread: ctx.thread() })
    }

    /// Puts the project `name` (`lobby_scene/<id>`) in the project's place,
    /// the lobby menu's variant still its user data. The name is a static,
    /// alive as long as the query reads it.
    pub fn ask(&mut self, name: &'static CStr) {
        // SAFETY: the frame's request of the project, written by
        // query_resources before this site and read by the query after it; a
        // static C string outlives the query.
        unsafe {
            let slot = self.frame.scene();
            slot.set_name(name.as_ptr().cast_mut().cast());
            *slot.class() = ResourceClass::GameProject as u32;
        }
    }

    /// Puts the project `name` in the project's place and `filler` (a
    /// lobby's config, as a binary config, no user data) in every other: the
    /// lobby's query with nothing in it but its project, for a swap (no
    /// second render scene, sound scene or movie is made).
    pub fn ask_alone(&mut self, name: &'static CStr, filler: &'static CStr) {
        self.ask(name);
        // SAFETY: the frame's eight requests and their user data pointers
        // (an array each from `requests` and `user_data`, QUERY_REQUESTS
        // long: the count query_resources passes, 0x21E5E7), written before
        // this site and read by the query after it; a static C string
        // outlives the query.
        unsafe {
            let (first, data, project) = (self.frame.requests(), self.frame.user_data(), self.frame.scene());
            for index in 0..QUERY_REQUESTS {
                let slot = first.at(index);
                if slot == project {
                    continue;
                }
                slot.set_name(filler.as_ptr().cast_mut().cast());
                *slot.class() = ResourceClass::BinaryConfig as u32;
                data.at(index).set_data(core::ptr::null_mut());
            }
        }
    }
}

/// How many requests the lobby's query has (`lea ecx,[esi+8]`, 0x21E5E7).
const QUERY_REQUESTS: usize = 8;

/// The lobby's query's results at `lobby_menu.scenes_ready`.
pub struct ScenesReady<'t> {
    menu: raw::LobbyMenu,
    results: raw::LobbySceneResults,
    thread: &'t Thread,
}

impl<'t> ScenesReady<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<ScenesReady<'t>> {
        use raw::site::lobby_menu_scenes_ready as site;
        // SAFETY: the context is this site's: the lobby menu (ECX) and the
        // callback's argument, the query's results, alive for the call on
        // the game's thread.
        let (menu, results) = unsafe { (site::menu(ctx)?, site::frame(ctx)?.results()?) };
        Some(ScenesReady { menu, results, thread: ctx.thread() })
    }

    /// Whether the project's place holds a project with its camera track
    /// (which on_render_scenes_ready plays without a check).
    pub fn loaded(&self) -> bool {
        // SAFETY: the results are alive for the call; result 3's resource is
        // the project asked there, 0 when it failed.
        unsafe { self.results.project().is_some_and(|project| *project.track() != 0) }
    }

    /// Asks the lobby's whole query again (its list rebuilt, and
    /// `lobby_menu.scene_request` passed again), for these results to be
    /// let go unused.
    pub fn ask_again(&self) {
        // SAFETY: the lobby menu is alive (its own callback runs); its query
        // only builds a request list and asks the resources, as its ctor did.
        unsafe { raw::engine_lobby_query_resources(Some(self.menu)) }
    }

    /// Puts the results' project (one with its track: `loaded`) in the place
    /// of the lobby menu's (behind the black, or while the lobby is not
    /// drawn). Nothing is touched when the results hold no such project.
    pub fn swap_in(&self) {
        // SAFETY: the results are alive for the callback; result 3 is the
        // project this patch asked (a project's resource, its project 38h
        // before it); the lobby menu is alive.
        unsafe {
            let Some(arrived) = self.results.project() else { return };
            put_in(self.menu, arrived.project());
        }
    }

    /// The black over the scene (the lobby movie's).
    pub fn shade(&self, shade: Shade) {
        // SAFETY: the lobby menu is alive for the callback.
        unsafe { shade_scene(self.menu, shade) }
    }

    pub fn reset_eye_adaptation(&self) {
        reset_eye_adaptation(self.thread);
    }
}

/// Puts `new` (a project with its track) in the place of the lobby menu's:
/// the old project out as lobby_menu::clear_resources takes it out
/// (0x21D080..0x21D100), the new one in as on_render_scenes_ready puts it in
/// (0x220DA2..0x220EC3, 0x22193E..0x221975), every step the game's own
/// function; the menu takes a reference of its own.
///
/// # Safety
/// `menu` and `new` are alive on the game's thread; the old project is the
/// lobby menu's (+114h, one reference of its own), with its static
/// collisions (+28h, +2Ch of them) in the menu's physics world.
unsafe fn put_in(menu: raw::LobbyMenu, new: raw::SimpleGameProject) {
    // SAFETY: the caller's.
    unsafe {
        let Some(track) = new.track() else { return };
        let slot = menu.scene();
        let (player, world) = (menu.track_player(), menu.physics_world());
        if let Some(old) = slot.project() {
            raw::engine_project_remove(Some(old));
            for collision in collisions(old) {
                raw::engine_collision_remove(Some(collision), world);
            }
            raw::engine_effects_stop(Some(player.effects()));
            *player.state() = 0;
            slot.set_project(None);
            release(old);
        }
        AtomicU32::from_ptr(new.count()).fetch_add(1, Ordering::SeqCst);
        slot.set_project(Some(new));
        raw::engine_project_on_inserted(Some(new));
        raw::engine_effects_stop(Some(player.effects()));
        *player.state() = 0;
        let mut made = raw::GameEffectSlotStorage::zeroed();
        let out = raw::engine_emitter_emit(Some(track), Some(made.handle()), 0.0, 1.0);
        raw::engine_effect_play(Some(player), out, 0, 1, 0, Some(menu.listener()));
        if let Some(effect) = made.handle().effect() {
            *effect.count() -= 1;
            if *effect.count() == 0 {
                raw::engine_emitter_destroy(effect.emitter(), Some(effect));
            }
        }
        raw::engine_project_insert(Some(new));
        for collision in collisions(new) {
            raw::engine_collision_insert(Some(collision), world);
        }
    }
}

/// The black over the lobby's 3D scene, as the lobby movie's
/// `srp_scene_fade` takes it (per mille of black, the loading spinner on it
/// or not): under every panel; its fades stepped by patch 81 a frame each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shade {
    /// No black.
    Gone,
    /// `per_mille` of the way to black.
    At { per_mille: u32, loading: bool },
    /// All black.
    Black { loading: bool },
}

impl Shade {
    /// The movie's arguments.
    fn arguments(self) -> [u32; 2] {
        match self {
            Shade::Gone => [0, 0],
            Shade::At { per_mille, loading } => [per_mille.min(1000), u32::from(loading)],
            Shade::Black { loading } => [1000, u32::from(loading)],
        }
    }
}

/// The lobby movie (lobby menu+690h's holder).
///
/// # Safety
/// `menu` is alive, on the game's thread.
unsafe fn lobby_movie<'t>(menu: raw::LobbyMenu) -> Option<super::ui::Movie<'t>> {
    // SAFETY: the caller's menu; its movie holder and the holder's movie live as long.
    unsafe { movie_of(menu.movie()) }
}

/// Asks the lobby movie for the black.
///
/// # Safety
/// `menu` is alive, on the game's thread.
unsafe fn shade_scene(menu: raw::LobbyMenu, shade: Shade) {
    // SAFETY: the caller's.
    if let Some(movie) = unsafe { lobby_movie(menu) } {
        movie.invoke_uints(c"root.srp_scene_fade", &shade.arguments());
    }
}

/// Resets the eye adaptation: the next frame takes its own luminance as the
/// last one's (stage_postprocess 0x1B14A6: the byte set, `t_previous_luminance`
/// the frame's own, then cleared), as the game's first frame does.
fn reset_eye_adaptation(_: &Thread) {
    // SAFETY: the image's byte (the render thread reads and clears it, a
    // byte written whole).
    unsafe { *raw::image::postprocess().reset_eye_adaptation() = 1 };
}

/// A project's static collisions (its +28h array, +2Ch of them).
///
/// # Safety
/// `project` is alive, its array as long as its count, while the iterator is.
unsafe fn collisions(project: raw::SimpleGameProject) -> impl Iterator<Item = raw::StaticCollision> {
    // SAFETY: the caller's.
    let (first, count) = unsafe { (project.collisions(), *project.collision_count() as usize) };
    // SAFETY: each index is under the array's count.
    first.into_iter().flat_map(move |first| (0..count).map(move |index| unsafe { first.at(index) }))
}

/// Lets go of the lobby menu's reference of a project (its count, lock
/// xadd; at 0 the game's release of its resource, as clear_resources does).
///
/// # Safety
/// `project` is alive and one of its references is the caller's.
unsafe fn release(project: raw::SimpleGameProject) {
    // SAFETY: the caller's.
    unsafe {
        if AtomicU32::from_ptr(project.count()).fetch_sub(1, Ordering::SeqCst) == 1 {
            raw::engine_release_resource(Some(project.resource()), project.count());
        }
    }
}

/// The lobby menu at `lobby_menu.deactivate`: no longer the active scene (a
/// match entered), its scene not drawn until `lobby_menu.activate`.
pub struct HiddenLobby<'t> {
    menu: raw::LobbyMenu,
    _thread: &'t Thread,
}

impl<'t> HiddenLobby<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<HiddenLobby<'t>> {
        // SAFETY: the context is this site's: on_deactivate's ECX, the lobby
        // menu, alive for the call on the game's thread.
        let menu = unsafe { raw::site::lobby_menu_deactivate::menu(ctx)? };
        Some(HiddenLobby { menu, _thread: ctx.thread() })
    }

    /// At `lobby_menu.activate`: the lobby menu about to be shown again.
    pub fn at_shown(ctx: &'t Context<'_>) -> Option<HiddenLobby<'t>> {
        // SAFETY: the context is this site's: on_activate's ECX, the lobby
        // menu, alive for the call on the game's thread.
        let menu = unsafe { raw::site::lobby_menu_activate::menu(ctx)? };
        Some(HiddenLobby { menu, _thread: ctx.thread() })
    }

    /// The black over the scene (the lobby movie's).
    pub fn shade(&self, shade: Shade) {
        // SAFETY: the lobby menu is alive (its own on_deactivate or
        // on_activate runs).
        unsafe { shade_scene(self.menu, shade) }
    }
}

/// The lobby menu as its camera ticks (`lobby_camera.view`): the lobby drawn.
pub struct DrawnLobby<'t> {
    menu: raw::LobbyMenu,
    thread: &'t Thread,
}

impl<'t> DrawnLobby<'t> {
    pub fn of(camera: &super::lobby_camera::LobbyCamera<'t>) -> Option<DrawnLobby<'t>> {
        Some(DrawnLobby { menu: camera.menu()?, thread: camera.thread() })
    }

    /// Asks the lobby's query (its list built again, `lobby_menu.scene_request`
    /// passed), its results to come at `lobby_menu.scenes_ready`.
    pub fn ask(&self) {
        // SAFETY: as ScenesReady::ask_again: the lobby menu is alive (its
        // camera ticks); its query only builds a request list and asks the
        // resources, as its ctor did.
        unsafe { raw::engine_lobby_query_resources(Some(self.menu)) }
    }

    /// The black over the scene (the lobby movie's).
    pub fn shade(&self, shade: Shade) {
        // SAFETY: the lobby menu is alive (its camera ticks).
        unsafe { shade_scene(self.menu, shade) }
    }

    /// Opens (or closes) the lobby window, as its button does (a diagnostics
    /// build's captures).
    pub fn toggle_window(&self) {
        // SAFETY: as shade.
        if let Some(movie) = unsafe { lobby_movie(self.menu) } {
            movie.invoke_uints(c"root.srp_lobby_window", &[]);
        }
    }

    /// Tells the lobby window what it shows: the pick and the lobby shown,
    /// as `root.srp_lobby_state`.
    pub fn tell(&self, state: [u32; 2]) {
        // SAFETY: as shade.
        if let Some(movie) = unsafe { lobby_movie(self.menu) } {
            movie.invoke_uints(c"root.srp_lobby_state", &state);
        }
    }

    pub fn reset_eye_adaptation(&self) {
        reset_eye_adaptation(self.thread);
    }
}

/// srp's two bytes of a `0xA1`: the receiver's lobby pick and its squad
/// commander's (lobby ids, 0 none).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Picks {
    pub own: u8,
    pub commander: u8,
}

/// srp's two bytes at a reader's cursor, when the message has them.
///
/// # Safety
/// `reader` is a live reader of the message, for the hook.
unsafe fn picks_at(reader: raw::BufferReader) -> Option<Picks> {
    // SAFETY: the caller's reader.
    let (begin, cursor, size) = unsafe { (reader.begin(), reader.cursor(), *reader.size()) };
    let (first, at) = (begin as usize, cursor as usize);
    let end = first.wrapping_add(size as usize);
    if cursor.is_null() || at < first || at > end || end < first {
        return None;
    }
    // SAFETY: begin..begin+size are the message's bytes, alive for the hook,
    // and cursor..end lies inside them.
    let mut rest = unsafe { Reader::of_memory(cursor, end - at) };
    Some(Picks { own: rest.u8()?, commander: rest.u8()? })
}

/// Sends srp a ranking request (`3B 00 <start> 1`) of one of its reserved
/// starts (the lobby_client's ranking request, 0x270070, as the movie's
/// request_players_range sends it).
///
/// # Safety
/// `client` is the lobby client, alive on the game's thread.
unsafe fn send_start(client: raw::LobbyClient, start: u32) {
    // SAFETY: the caller's; the request only builds and queues a message.
    unsafe { raw::engine_lobby_query_players_range(Some(client), start, 1) }
}

/// `0xA1`'s squad id, at `lobby_client.squad_id` (read_squad_info 0x274720
/// with the id kept, both paths: the lobby client in EBX, the reader in ESI,
/// past the id).
pub struct SquadId<'t> {
    client: raw::LobbyClient,
    reader: raw::BufferReader,
    _thread: &'t Thread,
}

impl<'t> SquadId<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<SquadId<'t>> {
        use raw::site::lobby_client_squad_id as site;
        // SAFETY: the context is this site's: the lobby client and the
        // message's reader, alive while the handler runs on the game's thread.
        let (client, reader) = unsafe { (site::client(ctx)?, site::reader(ctx)?) };
        Some(SquadId { client, reader, _thread: ctx.thread() })
    }

    /// The squad id the handler kept (+1EAAA4h): 0 no squad.
    pub fn squad_id(&self) -> u32 {
        // SAFETY: the client's field, written by the handler before the site.
        unsafe { *self.client.squad_id() }
    }

    /// For no squad (id 0), srp's two bytes after the id (a squad's come
    /// after its members, `SquadRead`); none when the message ends short.
    pub fn picks(&self) -> Option<Picks> {
        // SAFETY: the live reader.
        unsafe { picks_at(self.reader) }
    }

    /// Sends srp `3B` at `start` (one of its reserved starts).
    pub fn send(&self, start: u32) {
        // SAFETY: the lobby client of the handler.
        unsafe { send_start(self.client, start) }
    }
}

/// A squad `0xA1` just gave, at `lobby_client.squad_read` (read_squad_info
/// 0x274720 as it keeps the revision, a squad's path only: the lobby client
/// in EBX, the message's reader in ESI, its cursor past the revision).
pub struct SquadRead<'t> {
    client: raw::LobbyClient,
    reader: raw::BufferReader,
    _thread: &'t Thread,
}

impl<'t> SquadRead<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<SquadRead<'t>> {
        use raw::site::lobby_client_squad_read as site;
        // SAFETY: the context is this site's: the lobby client and the
        // message's reader, alive while the handler runs on the game's thread.
        let (client, reader) = unsafe { (site::client(ctx)?, site::reader(ctx)?) };
        Some(SquadRead { client, reader, _thread: ctx.thread() })
    }

    /// srp's two bytes after the squad, when the message has them.
    pub fn picks(&self) -> Option<Picks> {
        // SAFETY: the live reader.
        unsafe { picks_at(self.reader) }
    }

    /// Sends srp `3B` at `start` (one of its reserved starts).
    pub fn send(&self, start: u32) {
        // SAFETY: the lobby client of the handler.
        unsafe { send_start(self.client, start) }
    }
}
