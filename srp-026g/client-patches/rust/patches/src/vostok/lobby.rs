//! The lobby menu's scene (patch 81, docs/notes/lobby-scenes.md): the lobby
//! menu asks for its scene once, at the game's start, in one query with its
//! movies (lobby_menu::query_resources 0x21E450), and takes the results in
//! on_render_scenes_ready (0x220B20). At `lobby_menu.scene_request` the query's
//! list is built and not yet asked: its fourth request (the project) can be
//! another lobby's project, or a lobby's config to see whether it is there.
//! At `lobby_menu.scenes_ready` the results are in and nothing has used them:
//! the lobby may ask the whole query again instead.

use core::ffi::CStr;

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
}

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
}
