//! Patch 81: players choose the lobby's scene (docs/notes/lobby-scenes.md).
//!
//! user.cfg keeps the choice as the console variable `srp_lobby_scene` (a
//! lobby's id), which this patch registers before user.cfg runs
//! (`configs.start`, after patch 79's handler). The lobby menu asks for its scene once, at the
//! game's start, in one query with its movies; the chosen lobby's project
//! takes the classic's place there (`lobby_menu.scene_request`). A project
//! whose config is missing would crash the project's cook, so another lobby
//! than the classic is first asked as its config alone: when the results
//! come (`lobby_menu.scenes_ready`) the query is asked again, with the
//! lobby's project when its config is there, else with the classic's. A
//! project without its camera track is the classic too. The Settings' Lobby
//! selector (main_menu) calls `srp_lobby_scene` (`options.callback`, after patch 79's handler): with a
//! lobby's id it saves that choice for the next start; with or without, the
//! movie is answered with its texts, the saved and the shown lobby and every
//! lobby's id and name.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::language::{ConsoleString, OptionsCall, save_user_config};
use crate::vostok::lobby::{SceneQuery, SceneRequest, ScenesReady};
use crate::vostok::{Context, Flow, GameCell, LobbySceneOnOptionsCallExit, LobbySceneOnScenesReadyExit, NoExit};

/// A lobby scene, in the Settings' order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lobby {
    /// The 0.26g lobby, `lobby_scene/day` (the stock game's).
    Classic,
    /// The S.T.A.L.K.E.R. 2 (2012) blockpost hut.
    Stk2Blockpost,
    /// The 0.46c2 hangar.
    Hangar046,
    /// The 0.69d tent camp.
    Camp069,
}

impl Lobby {
    pub const ALL: [Lobby; 4] = [Lobby::Classic, Lobby::Stk2Blockpost, Lobby::Hangar046, Lobby::Camp069];

    /// Its id: the folder of its project, `projects/lobby_scene/<id>`, and
    /// the value user.cfg keeps.
    pub fn id(self) -> &'static CStr {
        match self {
            Lobby::Classic => c"day",
            Lobby::Stk2Blockpost => c"stk2_blockpost",
            Lobby::Hangar046 => c"hangar_046",
            Lobby::Camp069 => c"camp_069",
        }
    }

    /// Its project, as the lobby's query names it.
    pub fn project(self) -> &'static CStr {
        match self {
            Lobby::Classic => c"lobby_scene/day",
            Lobby::Stk2Blockpost => c"lobby_scene/stk2_blockpost",
            Lobby::Hangar046 => c"lobby_scene/hangar_046",
            Lobby::Camp069 => c"lobby_scene/camp_069",
        }
    }

    /// Its project's config, the file the project's cook reads first.
    pub fn config(self) -> &'static CStr {
        match self {
            Lobby::Classic => c"resources/projects/lobby_scene/day/client_project",
            Lobby::Stk2Blockpost => c"resources/projects/lobby_scene/stk2_blockpost/client_project",
            Lobby::Hangar046 => c"resources/projects/lobby_scene/hangar_046/client_project",
            Lobby::Camp069 => c"resources/projects/lobby_scene/camp_069/client_project",
        }
    }

    /// Its name's dictionary key.
    pub fn label(self) -> &'static CStr {
        match self {
            Lobby::Classic => c"st_srp_lobby_day",
            Lobby::Stk2Blockpost => c"st_srp_lobby_stk2_blockpost",
            Lobby::Hangar046 => c"st_srp_lobby_hangar_046",
            Lobby::Camp069 => c"st_srp_lobby_camp_069",
        }
    }

    /// The lobby of an id; None for any other text.
    pub fn parse(id: &[u8]) -> Option<Lobby> {
        Lobby::ALL.into_iter().find(|lobby| lobby.id().to_bytes() == id)
    }

    /// The lobby a saved value chooses: an unknown or empty one is the classic.
    pub fn chosen(saved: &[u8]) -> Lobby {
        Lobby::parse(saved).unwrap_or(Lobby::Classic)
    }
}

/// Where the lobby's scene is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// The lobby menu has not asked for it.
    Unasked,
    /// Asked as the lobby's config (is it there?).
    Probing(Lobby),
    /// Asked as the lobby's project.
    Asked(Lobby),
    /// The lobby menu has it.
    Shown(Lobby),
}

/// What the lobby's results lead to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// The lobby menu takes them (the original's code).
    Take,
    /// Asked again, these let go unused.
    AskAgain,
}

impl Scene {
    /// The query's request in the project's place; None leaves the
    /// original's.
    pub fn request(self) -> Option<SceneRequest> {
        match self {
            Scene::Unasked => None,
            Scene::Probing(lobby) => Some(SceneRequest::Config(lobby.config())),
            Scene::Asked(lobby) | Scene::Shown(lobby) => Some(SceneRequest::Project(lobby.project())),
        }
    }

    /// The lobby the player sees now, or will once the scene is there; None
    /// while that is not known yet.
    pub fn lobby(self) -> Option<Lobby> {
        match self {
            Scene::Asked(lobby) | Scene::Shown(lobby) => Some(lobby),
            Scene::Unasked | Scene::Probing(_) => None,
        }
    }

    /// The lobby menu builds its query: the first asks for the choice (the
    /// classic's project at once, another lobby's config first); one this
    /// patch asked again keeps what it set.
    pub fn on_query(self, chosen: Lobby) -> Scene {
        match self {
            Scene::Unasked if chosen == Lobby::Classic => Scene::Asked(Lobby::Classic),
            Scene::Unasked => Scene::Probing(chosen),
            other => other,
        }
    }

    /// The results are in; `loaded`: the project's place holds what was
    /// asked, usable (ScenesReady::loaded).
    pub fn on_ready(self, loaded: bool) -> (Scene, Next) {
        match self {
            Scene::Probing(lobby) if loaded => (Scene::Asked(lobby), Next::AskAgain),
            Scene::Probing(_) => (Scene::Asked(Lobby::Classic), Next::AskAgain),
            Scene::Asked(lobby) if loaded || lobby == Lobby::Classic => (Scene::Shown(lobby), Next::Take),
            Scene::Asked(_) => (Scene::Asked(Lobby::Classic), Next::AskAgain),
            other => (other, Next::Take),
        }
    }
}

/// The choice user.cfg keeps (`srp_lobby_scene <id>`).
static SAVED: ConsoleString<32> = ConsoleString::new();

static SCENE: GameCell<Scene> = GameCell::new(Scene::Unasked);

pub fn on_configs_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    SAVED.register(ctx.thread(), c"srp_lobby_scene");
    Flow::Continue
}

pub fn on_scene_request(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let scene = SCENE.get(thread).on_query(Lobby::chosen(SAVED.get(thread)));
    SCENE.set(thread, scene);
    if let (Some(request), Some(mut query)) = (scene.request(), SceneQuery::at(ctx)) {
        query.ask(request);
    }
    Flow::Continue
}

pub fn on_scenes_ready(ctx: &mut Context<'_>) -> Flow<LobbySceneOnScenesReadyExit> {
    let Some(ready) = ScenesReady::at(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let scene = SCENE.get(thread);
    let loaded = scene.request().is_some_and(|asked| ready.loaded(asked));
    let (scene, next) = scene.on_ready(loaded);
    SCENE.set(thread, scene);
    match next {
        Next::Take => Flow::Continue,
        Next::AskAgain => {
            ready.ask_again();
            Flow::Exit(LobbySceneOnScenesReadyExit::Done)
        }
    }
}

/// What the Settings' Lobby selector shows: its label, the note, the saved
/// choice, the lobby shown, then each lobby's id and name.
pub fn on_options_call(ctx: &mut Context<'_>) -> Flow<LobbySceneOnOptionsCallExit> {
    let Some(call) = OptionsCall::at(ctx) else { return Flow::Continue };
    if call.method() != c"srp_lobby_scene" {
        return Flow::Continue;
    }
    let thread = ctx.thread();
    if let Some(chosen) = call.string_argument(0).and_then(|id| Lobby::parse(id.to_bytes())) {
        SAVED.set(thread, chosen.id().to_bytes());
        save_user_config(thread);
    }
    let saved = Lobby::chosen(SAVED.get(thread));
    let shown = SCENE.get(thread).lobby().unwrap_or(saved);
    let mut texts = [[0u8; 0x200]; 2 + Lobby::ALL.len()];
    let [label, note, names @ ..] = &mut texts;
    let label = call.translate(c"st_srp_lobby_scene_option", label);
    let note = call.translate(c"st_srp_lobby_scene_restart", note);
    let mut values: [&CStr; 4 + 2 * Lobby::ALL.len()] = [c""; 4 + 2 * Lobby::ALL.len()];
    values[..4].copy_from_slice(&[label, note, saved.id(), shown.id()]);
    for ((pair, lobby), name) in values[4..].chunks_mut(2).zip(Lobby::ALL).zip(names.iter_mut()) {
        pair.copy_from_slice(&[lobby.id(), call.translate(lobby.label(), name)]);
    }
    call.answer(c"root.srp_lobby_scene", &values);
    Flow::Exit(LobbySceneOnOptionsCallExit::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_its_lobby_and_anything_else_the_classic() {
        for lobby in Lobby::ALL {
            assert_eq!(Lobby::parse(lobby.id().to_bytes()), Some(lobby));
            assert_eq!(Lobby::chosen(lobby.id().to_bytes()), lobby);
            let id = lobby.id().to_str().unwrap();
            assert_eq!(lobby.project().to_str().unwrap(), format!("lobby_scene/{id}"));
            assert_eq!(lobby.config().to_str().unwrap(), format!("resources/projects/lobby_scene/{id}/client_project"));
            assert_eq!(lobby.label().to_str().unwrap(), format!("st_srp_lobby_{id}"));
            assert!(lobby.id().count_bytes() < 32, "fits the console variable");
        }
        for other in [&b""[..], b"Day", b"stk2", b"day ", b"lobby_scene/day"] {
            assert_eq!(Lobby::parse(other), None);
            assert_eq!(Lobby::chosen(other), Lobby::Classic);
        }
    }

    /// The scene through its queries and results until the lobby has it:
    /// the requests asked, in order, and the lobby shown.
    fn run(chosen: Lobby, mut loads: impl FnMut(SceneRequest) -> bool) -> (std::vec::Vec<SceneRequest>, Lobby) {
        let mut scene = Scene::Unasked;
        let mut asked = std::vec::Vec::new();
        loop {
            scene = scene.on_query(chosen);
            let request = scene.request().expect("a query asks");
            asked.push(request);
            assert!(asked.len() <= 3, "at most a probe, the lobby and the classic");
            let (next, step) = scene.on_ready(loads(request));
            scene = next;
            if step == Next::Take {
                let Scene::Shown(lobby) = scene else { panic!("taken but not shown: {scene:?}") };
                return (asked, lobby);
            }
        }
    }

    #[test]
    fn the_classic_is_asked_at_once() {
        let (asked, shown) = run(Lobby::Classic, |_| true);
        assert_eq!(asked, [SceneRequest::Project(c"lobby_scene/day")]);
        assert_eq!(shown, Lobby::Classic);
    }

    #[test]
    fn another_lobby_is_probed_then_asked() {
        let (asked, shown) = run(Lobby::Hangar046, |_| true);
        assert_eq!(asked, [SceneRequest::Config(Lobby::Hangar046.config()), SceneRequest::Project(c"lobby_scene/hangar_046")]);
        assert_eq!(shown, Lobby::Hangar046);
    }

    #[test]
    fn a_missing_lobby_is_the_classic_its_project_never_asked() {
        let (asked, shown) = run(Lobby::Camp069, |request| !matches!(request, SceneRequest::Config(_)));
        assert_eq!(asked, [SceneRequest::Config(Lobby::Camp069.config()), SceneRequest::Project(c"lobby_scene/day")]);
        assert_eq!(shown, Lobby::Classic);
    }

    #[test]
    fn a_lobby_without_its_track_is_the_classic() {
        let (asked, shown) = run(Lobby::Stk2Blockpost, |request| request != SceneRequest::Project(Lobby::Stk2Blockpost.project()));
        assert_eq!(asked, [
            SceneRequest::Config(Lobby::Stk2Blockpost.config()),
            SceneRequest::Project(Lobby::Stk2Blockpost.project()),
            SceneRequest::Project(c"lobby_scene/day"),
        ]);
        assert_eq!(shown, Lobby::Classic);
    }

    #[test]
    fn the_classic_failing_is_the_stock_game() {
        let (asked, shown) = run(Lobby::Classic, |_| false);
        assert_eq!(asked, [SceneRequest::Project(c"lobby_scene/day")]);
        assert_eq!(shown, Lobby::Classic);
    }

    #[test]
    fn results_this_patch_did_not_ask_for_are_the_lobby_menus() {
        assert_eq!(Scene::Unasked.on_ready(false), (Scene::Unasked, Next::Take));
        assert_eq!(Scene::Shown(Lobby::Camp069).on_ready(false), (Scene::Shown(Lobby::Camp069), Next::Take));
        assert_eq!(Scene::Unasked.request(), None);
    }

    #[test]
    fn the_lobby_shown_is_known_once_its_project_is_asked() {
        assert_eq!(Scene::Unasked.lobby(), None);
        assert_eq!(Scene::Probing(Lobby::Hangar046).lobby(), None);
        assert_eq!(Scene::Asked(Lobby::Classic).lobby(), Some(Lobby::Classic));
        assert_eq!(Scene::Shown(Lobby::Camp069).lobby(), Some(Lobby::Camp069));
    }
}
