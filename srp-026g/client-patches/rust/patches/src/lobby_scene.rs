//! Patch 81: the lobby's scene follows the player's quest faction
//! (docs/notes/lobby-scenes.md): Renaissance the classic, the Black Market
//! the S.T.A.L.K.E.R. 2 blockpost, the Scavengers the 0.69d camp, the Edge
//! the 0.46c2 hangar; no faction yet the classic.
//!
//! The lobby menu asks for its scene once, at the game's start, before the
//! sign-in: the faction is not known yet (the network client is made in the
//! scene's own callback). So the faction the server gives (`0x8E`, at the
//! sign-in and after a change: `lobby_client.faction`) is kept in user.cfg
//! as the console variable `srp_lobby_faction`, registered before user.cfg
//! runs (`configs.start`, after patch 79's handler), and the next start shows
//! its lobby: the faction's lobby takes the classic's place in the scene's
//! query (`lobby_menu.scene_request`). A project whose config is missing
//! would crash the project's cook, so another lobby than the classic is
//! first asked as its config alone: when the results come
//! (`lobby_menu.scenes_ready`) the query is asked again, with the lobby's
//! project when its config is there, else with the classic's. A project
//! without its camera track is the classic too.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::language::{ConsoleString, save_user_config};
use crate::vostok::lobby::{SceneQuery, SceneRequest, ScenesReady, arrived_faction};
use crate::vostok::{Context, Flow, GameCell, LobbySceneOnScenesReadyExit, NoExit};

/// A quest faction (factions_dict; the server's `faction::` ids).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Faction {
    Scavengers,
    BlackMarket,
    Renaissance,
    Edge,
}

impl Faction {
    /// The faction of a factions_dict id; None for 0 (none chosen yet) and
    /// any other.
    pub fn of_id(id: u8) -> Option<Faction> {
        match id {
            1 => Some(Faction::Scavengers),
            2 => Some(Faction::BlackMarket),
            3 => Some(Faction::Renaissance),
            4 => Some(Faction::Edge),
            _ => None,
        }
    }

    /// Its id as user.cfg keeps it (one decimal digit).
    pub fn saved(self) -> &'static [u8] {
        match self {
            Faction::Scavengers => b"1",
            Faction::BlackMarket => b"2",
            Faction::Renaissance => b"3",
            Faction::Edge => b"4",
        }
    }

    /// The faction a saved value names; None for no value or any other text.
    pub fn parse(saved: &[u8]) -> Option<Faction> {
        match saved {
            [digit @ b'0'..=b'9'] => Faction::of_id(digit - b'0'),
            _ => None,
        }
    }
}

/// A lobby scene.
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

    /// The lobby of a faction (the owner, 2026-10-29: "different ones should be for different
    /// factions", "stalker2 one is for bandits", "the current one is for army", "the room is for
    /// stalker"); none chosen yet the classic.
    pub fn of_faction(faction: Option<Faction>) -> Lobby {
        match faction {
            Some(Faction::Renaissance) | None => Lobby::Classic,
            Some(Faction::BlackMarket) => Lobby::Stk2Blockpost,
            Some(Faction::Scavengers) => Lobby::Camp069,
            Some(Faction::Edge) => Lobby::Hangar046,
        }
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

/// The last quest faction the server gave, as user.cfg keeps it
/// (`srp_lobby_faction <id>`): the next start's lobby.
static SAVED: ConsoleString<4> = ConsoleString::new();

static SCENE: GameCell<Scene> = GameCell::new(Scene::Unasked);

pub fn on_configs_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    SAVED.register(ctx.thread(), c"srp_lobby_faction");
    Flow::Continue
}

/// A faction from the server: kept for the next start (user.cfg written at
/// once when it is another than the saved one).
pub fn on_faction(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let faction = Faction::of_id(arrived_faction(ctx));
    let saved = Faction::parse(SAVED.get(thread));
    if faction != saved {
        SAVED.set(thread, faction.map_or(&b""[..], Faction::saved));
        save_user_config(thread);
    }
    Flow::Continue
}

pub fn on_scene_request(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let scene = SCENE.get(thread).on_query(Lobby::of_faction(Faction::parse(SAVED.get(thread))));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_faction_has_its_lobby_and_none_the_classic() {
        assert_eq!(Lobby::of_faction(Some(Faction::Renaissance)), Lobby::Classic);
        assert_eq!(Lobby::of_faction(Some(Faction::BlackMarket)), Lobby::Stk2Blockpost);
        assert_eq!(Lobby::of_faction(Some(Faction::Scavengers)), Lobby::Camp069);
        assert_eq!(Lobby::of_faction(Some(Faction::Edge)), Lobby::Hangar046);
        assert_eq!(Lobby::of_faction(None), Lobby::Classic);
        for lobby in Lobby::ALL {
            let project = lobby.project().to_str().unwrap();
            let id = project.strip_prefix("lobby_scene/").unwrap();
            assert_eq!(lobby.config().to_str().unwrap(), format!("resources/projects/lobby_scene/{id}/client_project"));
        }
    }

    #[test]
    fn a_faction_is_saved_as_its_id_and_read_back() {
        for id in 0..=255u8 {
            let faction = Faction::of_id(id);
            assert_eq!(faction.is_some(), (1..=4).contains(&id), "{id}");
            if let Some(faction) = faction {
                assert_eq!(Faction::parse(faction.saved()), Some(faction));
            }
        }
        for other in [&b""[..], b"0", b"5", b"12", b" 1", b"a"] {
            assert_eq!(Faction::parse(other), None, "{other:?}");
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
}
