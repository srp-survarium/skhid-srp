//! The lobby menu's scene (patch 81, docs/notes/lobby-scenes.md): the lobby
//! menu asks for its scene once, at the game's start, in one query with its
//! movies (lobby_menu::query_resources 0x21E450), and takes the results in
//! on_render_scenes_ready (0x220B20). At `lobby_menu.scene_request` the query's
//! list is built and not yet asked: its fourth request (the project) can be
//! another lobby's project, or a lobby's config to see whether it is there.
//! At `lobby_menu.scenes_ready` the results are in and nothing has used them:
//! the lobby may ask the whole query again instead.
//!
//! After a match (lobby-scenes.md, "After a match"): while the lobby menu is
//! not the active scene (`lobby_menu.deactivate` to `lobby_menu.activate`)
//! the lobby asks its query again ([`HiddenLobby::ask`]) with only its
//! project in it ([`SceneQuery::ask_alone`]), and puts the project it gets in
//! the old one's place ([`ScenesReady::swap`]): the project halves of
//! lobby_menu::clear_resources (0x21D080) and on_render_scenes_ready
//! (0x220B20), in their order.

use core::ffi::CStr;
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::Context;
use super::hook::Thread;
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

/// What the lobby's query asks for in its project's place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneRequest {
    /// A lobby's project (`lobby_scene/<id>`): its cook needs the project's
    /// config; without one it reads a null config and the game crashes
    /// (project_cooker_simple::create_game_objects, 0x22C11E).
    Project(&'static CStr),
    /// A lobby's config (`resources/projects/lobby_scene/<id>/client_project`),
    /// as a binary config, with no user data: a missing file fails only this
    /// result.
    Config(&'static CStr),
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

    /// Puts `request` in the project's place. The name is a static, alive
    /// as long as the query reads it.
    pub fn ask(&mut self, request: SceneRequest) {
        let (name, class, keeps_data) = match request {
            SceneRequest::Project(name) => (name, ResourceClass::GameProject, true),
            SceneRequest::Config(name) => (name, ResourceClass::BinaryConfig, false),
        };
        // SAFETY: the frame's request of the project and its user data
        // pointer, written by query_resources before this site and read by
        // the query after it; a static C string outlives the query.
        unsafe {
            let slot = self.frame.scene();
            slot.set_name(name.as_ptr().cast_mut().cast());
            *slot.class() = class as u32;
            if !keeps_data {
                self.frame.scene_data().set_data(core::ptr::null_mut());
            }
        }
    }

    /// Puts `request` in the project's place and `filler` (a lobby's
    /// config, as a binary config, no user data) in every other: the
    /// lobby's query with nothing in it but its project, for a swap (no
    /// second render scene, sound scene or movie is made).
    pub fn ask_alone(&mut self, request: SceneRequest, filler: &'static CStr) {
        self.ask(request);
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
    _thread: &'t Thread,
}

impl<'t> ScenesReady<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<ScenesReady<'t>> {
        use raw::site::lobby_menu_scenes_ready as site;
        // SAFETY: the context is this site's: the lobby menu (ECX) and the
        // callback's argument, the query's results, alive for the call on
        // the game's thread.
        let (menu, results) = unsafe { (site::menu(ctx)?, site::frame(ctx)?.results()?) };
        Some(ScenesReady { menu, results, _thread: ctx.thread() })
    }

    /// Whether the project's place holds what `asked` asked for, usable: a
    /// config there, or a project with its camera track (which
    /// on_render_scenes_ready plays without a check).
    pub fn loaded(&self, asked: SceneRequest) -> bool {
        // SAFETY: the results are alive for the call; result 3's resource
        // is what was asked there (a binary config or a project), 0 when it
        // failed, and its kind is read only as what was asked.
        unsafe {
            match asked {
                SceneRequest::Config(_) => *self.results.scene() != 0,
                SceneRequest::Project(_) => self.results.project().is_some_and(|project| *project.track() != 0),
            }
        }
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
    /// of the lobby menu's, while the lobby is not drawn: the old project
    /// out as lobby_menu::clear_resources takes it out (0x21D080..0x21D100),
    /// the new one in as on_render_scenes_ready puts it in (0x220DA2..0x220EC3,
    /// 0x22193E..0x221975), every step the game's own function. Nothing is
    /// touched when the results hold no such project.
    pub fn swap(&self) {
        // SAFETY: the results and the lobby menu are alive for the callback
        // on the game's thread; result 3 is the project this patch asked
        // (a project's resource, its project 38h before it); the old project
        // is the lobby menu's (+114h, one reference of its own), with its
        // static collisions (+28h, +2Ch of them) in the menu's physics world.
        unsafe {
            let Some(arrived) = self.results.project() else { return };
            let new = arrived.project();
            let Some(track) = new.track() else { return };
            let (menu, slot) = (self.menu, self.menu.scene());
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

    /// Asks the lobby's query (its list built again, `lobby_menu.scene_request`
    /// passed), its results to come at `lobby_menu.scenes_ready`.
    pub fn ask(&self) {
        // SAFETY: as ScenesReady::ask_again: the lobby menu is alive (its
        // own on_deactivate runs).
        unsafe { raw::engine_lobby_query_resources(Some(self.menu)) }
    }
}

/// The quest faction the server just gave, at `lobby_client.faction` (the
/// 0x8E handler as it keeps the message's last byte, EAX, at the lobby
/// client's +1EA998h): the factions_dict id, 0 for none chosen yet.
pub fn arrived_faction(ctx: &Context<'_>) -> u8 {
    ctx.reg(super::context::Reg::Eax) as u8
}
