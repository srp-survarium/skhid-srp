//! Patch 81: the lobby's scene follows the player's quest faction
//! (docs/notes/lobby-scenes.md): Renaissance the classic, the Black Market
//! the S.T.A.L.K.E.R. 2 bunker, the Scavengers the 0.69d camp, the Edge
//! the 0.46c2 hangar; no faction yet (no saved value) the Scavengers.
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
//!
//! A change of faction during a session shows after the next match: when
//! the lobby stops being the active scene (`lobby_menu.deactivate`) with
//! another faction's lobby wanted than the one shown, its project is asked
//! alone and, when it comes while the lobby is still hidden, put in the old
//! one's place; the first frame back is the new lobby. Results after the
//! return (`lobby_menu.activate`) or once the game closes
//! (`game.clear_resources`) are let go. One enum, [`Scene`], and one
//! transition function, [`Scene::step`].

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::language::{ConsoleString, save_user_config};
use crate::vostok::lobby::{HiddenLobby, SceneQuery, SceneRequest, ScenesReady, arrived_faction};
use crate::vostok::{Context, Flow, GameCell, LobbySceneOnScenesReadyExit, NoExit, Thread};

/// A quest faction (factions_dict; the server's `faction::` ids).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Faction {
    Scavengers,
    BlackMarket,
    Renaissance,
    Edge,
}

impl Faction {
    /// The faction of a factions_dict id; 0 (none chosen yet) and any
    /// other are the Scavengers (the owner, 2026-10-30: "no faction falls
    /// back to scavengers"; no account can switch to none).
    pub fn of_id(id: u8) -> Faction {
        match id {
            2 => Faction::BlackMarket,
            3 => Faction::Renaissance,
            4 => Faction::Edge,
            _ => Faction::Scavengers,
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

    /// The faction a saved value names; no value or any other text the
    /// Scavengers, as no faction.
    pub fn parse(saved: &[u8]) -> Faction {
        match saved {
            [digit @ b'0'..=b'9'] => Faction::of_id(digit - b'0'),
            _ => Faction::Scavengers,
        }
    }
}

/// A lobby scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lobby {
    /// The 0.26g lobby, `lobby_scene/day` (the stock game's).
    Classic,
    /// The S.T.A.L.K.E.R. 2 (2012) bunker.
    Stk2Bunker,
    /// The 0.46c2 hangar.
    Hangar046,
    /// The 0.69d tent camp.
    Camp069,
}

impl Lobby {
    pub const ALL: [Lobby; 4] = [Lobby::Classic, Lobby::Stk2Bunker, Lobby::Hangar046, Lobby::Camp069];

    /// Its project, as the lobby's query names it.
    pub fn project(self) -> &'static CStr {
        match self {
            Lobby::Classic => c"lobby_scene/day",
            Lobby::Stk2Bunker => c"lobby_scene/stk2_bunker",
            Lobby::Hangar046 => c"lobby_scene/hangar_046",
            Lobby::Camp069 => c"lobby_scene/camp_069",
        }
    }

    /// Its project's config, the file the project's cook reads first.
    pub fn config(self) -> &'static CStr {
        match self {
            Lobby::Classic => c"resources/projects/lobby_scene/day/client_project",
            Lobby::Stk2Bunker => c"resources/projects/lobby_scene/stk2_bunker/client_project",
            Lobby::Hangar046 => c"resources/projects/lobby_scene/hangar_046/client_project",
            Lobby::Camp069 => c"resources/projects/lobby_scene/camp_069/client_project",
        }
    }

    /// The lobby of a faction, one per faction (the owner, 2026-10-29: "different ones should be
    /// for different factions", "the current one is for army"; 2026-10-30: the Black Market the
    /// STK2 bunker, the Edge the hangar).
    pub fn of_faction(faction: Faction) -> Lobby {
        match faction {
            Faction::Renaissance => Lobby::Classic,
            Faction::BlackMarket => Lobby::Stk2Bunker,
            Faction::Scavengers => Lobby::Camp069,
            Faction::Edge => Lobby::Hangar046,
        }
    }
}

/// What the lobby's query asks for in its project's place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    /// A lobby's config alone: is its pack there?
    Config(Lobby),
    /// A lobby's project.
    Project(Lobby),
}

impl Ask {
    /// The first ask for a lobby: the classic's project at once (the stock
    /// game's), another lobby's config first (a missing project would crash
    /// its cook).
    pub fn first(lobby: Lobby) -> Ask {
        match lobby {
            Lobby::Classic => Ask::Project(Lobby::Classic),
            other => Ask::Config(other),
        }
    }

    pub fn lobby(self) -> Lobby {
        match self {
            Ask::Config(lobby) | Ask::Project(lobby) => lobby,
        }
    }

    pub fn request(self) -> SceneRequest {
        match self {
            Ask::Config(lobby) => SceneRequest::Config(lobby.config()),
            Ask::Project(lobby) => SceneRequest::Project(lobby.project()),
        }
    }
}

/// Whether the lobby's scene is on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// The lobby menu is the active scene (or may be: from the start until
    /// a match is entered).
    Drawn,
    /// A match (or any other scene) is: from on_deactivate to on_activate.
    Hidden,
    /// The game is closing (game::clear_resources): never drawn again, and
    /// the lobby menu not to be touched (its own clear_resources comes).
    Closed,
}

/// A swap's query in flight: the lobby it is for (the faction's when it
/// was asked) and what it asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fetch {
    pub wanted: Lobby,
    pub ask: Ask,
}

/// Where the lobby's scene is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// The lobby menu has not asked for it.
    Unasked,
    /// The start: the lobby menu's own query asks this (and the lobby menu
    /// takes the project).
    Starting(Ask),
    /// The lobby menu has the lobby `shown`; `fetch` a swap's query in
    /// flight.
    Loaded { shown: Lobby, view: View, fetch: Option<Fetch> },
}

/// What happens to the lobby's scene. `wanted` is the lobby of the faction
/// saved at that moment (`srp_lobby_faction`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The lobby menu builds its query (`lobby_menu.scene_request`).
    Query { wanted: Lobby },
    /// Its results are in (`lobby_menu.scenes_ready`); `loaded`: the
    /// project's place holds what was asked, usable (ScenesReady::loaded).
    Ready { loaded: bool, wanted: Lobby },
    /// The lobby is no longer drawn (`lobby_menu.deactivate`: a match).
    Hidden { wanted: Lobby },
    /// The lobby is shown again (`lobby_menu.activate`).
    Shown,
    /// The game closes (`game.clear_resources`: it deactivates the active
    /// scene, the lobby too, and clears the lobby menu).
    Closing,
}

/// What the site does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// The site's own code as it is: the query's own list, the results
    /// taken by the lobby menu, the menu's (de)activation.
    Original,
    /// The start's query: this in the project's place.
    Ask(SceneRequest),
    /// A swap's query: this in the project's place, the lobby's config
    /// (`filler`) in every other.
    AskAlone { request: SceneRequest, filler: Lobby },
    /// The results let go, the query asked again.
    AskAgain,
    /// The results let go.
    LetGo,
    /// The results' project put in the lobby menu's (the lobby not drawn).
    Swap,
    /// The lobby's query asked (a swap's first).
    Query,
}

impl Scene {
    /// What its query asked in the project's place, when it is this
    /// patch's.
    pub fn asked(self) -> Option<SceneRequest> {
        match self {
            Scene::Starting(ask) | Scene::Loaded { fetch: Some(Fetch { ask, .. }), .. } => Some(ask.request()),
            Scene::Unasked | Scene::Loaded { fetch: None, .. } => None,
        }
    }

    /// The machine: the next scene and what the site does, for every scene
    /// and every event. A swap's project is put in only while the lobby is
    /// hidden; results that come while it is drawn are let go, and the next
    /// match asks again.
    pub fn step(self, event: Event) -> (Scene, Action) {
        use Scene::{Loaded, Starting, Unasked};
        match (self, event) {
            (Unasked, Event::Query { wanted }) => (Starting(Ask::first(wanted)), Action::Ask(Ask::first(wanted).request())),
            (Starting(ask), Event::Query { .. }) => (self, Action::Ask(ask.request())),
            (Loaded { fetch: Some(Fetch { ask, .. }), .. }, Event::Query { .. }) => {
                (self, Action::AskAlone { request: ask.request(), filler: ask.lobby() })
            }
            (Loaded { fetch: None, .. }, Event::Query { .. }) => (self, Action::Original),

            (Unasked, Event::Ready { .. }) | (Loaded { fetch: None, .. }, Event::Ready { .. }) => (self, Action::Original),
            (Starting(Ask::Config(lobby)), Event::Ready { loaded, .. }) => {
                (Starting(Ask::Project(if loaded { lobby } else { Lobby::Classic })), Action::AskAgain)
            }
            (Starting(Ask::Project(lobby)), Event::Ready { loaded, .. }) if loaded || lobby == Lobby::Classic => {
                (Loaded { shown: lobby, view: View::Drawn, fetch: None }, Action::Original)
            }
            (Starting(Ask::Project(_)), Event::Ready { .. }) => (Starting(Ask::Project(Lobby::Classic)), Action::AskAgain),
            (Loaded { shown, view: view @ (View::Drawn | View::Closed), fetch: Some(_) }, Event::Ready { .. }) => {
                (Loaded { shown, view, fetch: None }, Action::LetGo)
            }
            (Loaded { shown, view: View::Hidden, fetch: Some(fetch) }, Event::Ready { loaded, wanted }) => {
                let hidden = |fetch| Loaded { shown, view: View::Hidden, fetch };
                let ask = |ask| (hidden(Some(Fetch { wanted, ask })), Action::AskAgain);
                let classic = || if shown == Lobby::Classic { (hidden(None), Action::LetGo) } else { ask(Ask::Project(Lobby::Classic)) };
                match fetch.ask {
                    _ if fetch.wanted != wanted && wanted == shown => (hidden(None), Action::LetGo),
                    _ if fetch.wanted != wanted => ask(Ask::first(wanted)),
                    Ask::Config(lobby) if loaded => ask(Ask::Project(lobby)),
                    Ask::Project(lobby) if loaded => (Loaded { shown: lobby, view: View::Hidden, fetch: None }, Action::Swap),
                    Ask::Project(Lobby::Classic) => (hidden(None), Action::LetGo),
                    Ask::Config(_) | Ask::Project(_) => classic(),
                }
            }

            (Loaded { view: View::Closed, .. }, Event::Hidden { .. } | Event::Shown | Event::Closing) => (self, Action::Original),
            (Loaded { shown, fetch, .. }, Event::Closing) => (Loaded { shown, view: View::Closed, fetch }, Action::Original),
            (Unasked | Starting(_), Event::Closing) => (self, Action::Original),
            (Loaded { shown, fetch: None, .. }, Event::Hidden { wanted }) if wanted != shown => {
                (Loaded { shown, view: View::Hidden, fetch: Some(Fetch { wanted, ask: Ask::first(wanted) }) }, Action::Query)
            }
            (Loaded { shown, fetch, .. }, Event::Hidden { .. }) => (Loaded { shown, view: View::Hidden, fetch }, Action::Original),
            (Unasked | Starting(_), Event::Hidden { .. }) => (self, Action::Original),

            (Loaded { shown, fetch, .. }, Event::Shown) => (Loaded { shown, view: View::Drawn, fetch }, Action::Original),
            (Unasked | Starting(_), Event::Shown) => (self, Action::Original),
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
        SAVED.set(thread, faction.saved());
        save_user_config(thread);
    }
    Flow::Continue
}

/// The lobby of the faction saved now.
fn wanted(thread: &Thread) -> Lobby {
    Lobby::of_faction(Faction::parse(SAVED.get(thread)))
}

/// One step of the machine, the scene kept (before the site acts: a query
/// it asks may come back at once).
fn step(thread: &Thread, event: Event) -> Action {
    let (scene, action) = SCENE.get(thread).step(event);
    SCENE.set(thread, scene);
    action
}

pub fn on_scene_request(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let action = step(thread, Event::Query { wanted: wanted(thread) });
    if let Some(mut query) = SceneQuery::at(ctx) {
        match action {
            Action::Ask(request) => query.ask(request),
            Action::AskAlone { request, filler } => query.ask_alone(request, filler.config()),
            // the query's own list; the other actions are not a query's
            Action::Original | Action::AskAgain | Action::LetGo | Action::Swap | Action::Query => {}
        }
    }
    Flow::Continue
}

pub fn on_scenes_ready(ctx: &mut Context<'_>) -> Flow<LobbySceneOnScenesReadyExit> {
    let Some(ready) = ScenesReady::at(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let loaded = SCENE.get(thread).asked().is_some_and(|asked| ready.loaded(asked));
    match step(thread, Event::Ready { loaded, wanted: wanted(thread) }) {
        Action::AskAgain => ready.ask_again(),
        Action::Swap => ready.swap(),
        Action::LetGo => {}
        // the lobby menu takes them; the other actions are not a result's
        Action::Original | Action::Ask(_) | Action::AskAlone { .. } | Action::Query => return Flow::Continue,
    }
    Flow::Exit(LobbySceneOnScenesReadyExit::Done)
}

/// A match entered: the lobby not drawn until it is shown again. When the
/// faction's lobby is not the one shown, its project is asked now.
pub fn on_deactivate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if step(thread, Event::Hidden { wanted: wanted(thread) }) == Action::Query
        && let Some(lobby) = HiddenLobby::at(ctx)
    {
        lobby.ask();
    }
    Flow::Continue
}

/// The lobby shown again: a swap's results that come from now on are let go.
pub fn on_activate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    step(ctx.thread(), Event::Shown);
    Flow::Continue
}

/// The game closes: nothing asked from now on (its on_deactivate of the
/// lobby comes next), a swap's results let go without the lobby menu.
pub fn on_game_closing(ctx: &mut Context<'_>) -> Flow<NoExit> {
    step(ctx.thread(), Event::Closing);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_faction_has_its_lobby() {
        assert_eq!(Lobby::of_faction(Faction::Renaissance), Lobby::Classic);
        assert_eq!(Lobby::of_faction(Faction::BlackMarket), Lobby::Stk2Bunker);
        assert_eq!(Lobby::of_faction(Faction::Scavengers), Lobby::Camp069);
        assert_eq!(Lobby::of_faction(Faction::Edge), Lobby::Hangar046);
        for lobby in Lobby::ALL {
            let project = lobby.project().to_str().unwrap();
            let id = project.strip_prefix("lobby_scene/").unwrap();
            assert_eq!(lobby.config().to_str().unwrap(), format!("resources/projects/lobby_scene/{id}/client_project"));
        }
    }

    #[test]
    fn a_faction_is_saved_as_its_id_and_read_back_and_none_is_the_scavengers() {
        for id in 0..=255u8 {
            let faction = Faction::of_id(id);
            if !(2..=4).contains(&id) {
                assert_eq!(faction, Faction::Scavengers, "{id}");
            }
            assert_eq!(Faction::parse(faction.saved()), faction);
        }
        assert_eq!(Faction::of_id(1).saved(), b"1");
        for other in [&b""[..], b"0", b"5", b"12", b" 1", b"a"] {
            assert_eq!(Faction::parse(other), Faction::Scavengers, "{other:?}");
        }
    }

    /// The start: the scene through its queries and results until the
    /// lobby menu takes it: the requests asked, in order, and the lobby
    /// shown.
    fn run(chosen: Lobby, mut loads: impl FnMut(SceneRequest) -> bool) -> (std::vec::Vec<SceneRequest>, Lobby) {
        let mut scene = Scene::Unasked;
        let mut asked = std::vec::Vec::new();
        loop {
            let (next, action) = scene.step(Event::Query { wanted: chosen });
            let Action::Ask(request) = action else { panic!("a start's query asks: {action:?}") };
            assert_eq!(next.asked(), Some(request));
            asked.push(request);
            assert!(asked.len() <= 3, "at most a probe, the lobby and the classic");
            let (next, action) = next.step(Event::Ready { loaded: loads(request), wanted: chosen });
            scene = next;
            match action {
                Action::AskAgain => continue,
                Action::Original => {
                    let Scene::Loaded { shown, view: View::Drawn, fetch: None } = scene else { panic!("{scene:?}") };
                    return (asked, shown);
                }
                other => panic!("a start's results: {other:?}"),
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
        let (asked, shown) = run(Lobby::Stk2Bunker, |request| request != SceneRequest::Project(Lobby::Stk2Bunker.project()));
        assert_eq!(asked, [
            SceneRequest::Config(Lobby::Stk2Bunker.config()),
            SceneRequest::Project(Lobby::Stk2Bunker.project()),
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
        let wanted = Lobby::Camp069;
        assert_eq!(Scene::Unasked.step(Event::Ready { loaded: false, wanted }), (Scene::Unasked, Action::Original));
        let shown = Scene::Loaded { shown: Lobby::Camp069, view: View::Drawn, fetch: None };
        assert_eq!(shown.step(Event::Ready { loaded: false, wanted }), (shown, Action::Original));
        assert_eq!(shown.step(Event::Query { wanted }), (shown, Action::Original));
        assert_eq!(Scene::Unasked.asked(), None);
    }

    /// A game after its start: the scene and each event's action, every
    /// query and result played as the game plays them (`loads` says whether
    /// what was asked is there).
    struct Game {
        scene: Scene,
        wanted: Lobby,
        loads: fn(SceneRequest) -> bool,
        /// A swap's query asked and its results not yet in.
        pending: Option<SceneRequest>,
        actions: std::vec::Vec<Action>,
    }

    impl Game {
        fn new(shown: Lobby, loads: fn(SceneRequest) -> bool) -> Game {
            Game { scene: Scene::Loaded { shown, view: View::Drawn, fetch: None }, wanted: shown, loads, pending: None, actions: std::vec![] }
        }

        fn event(&mut self, event: Event) -> Action {
            let (scene, action) = self.scene.step(event);
            if matches!(action, Action::Swap | Action::Query) {
                assert!(matches!(self.scene, Scene::Loaded { view: View::Hidden | View::Drawn, .. }), "{action:?} once closed");
            }
            if action == Action::Swap {
                assert!(matches!(self.scene, Scene::Loaded { view: View::Hidden, .. }), "a swap while drawn: {:?}", self.scene);
            }
            self.scene = scene;
            self.actions.push(action);
            action
        }

        /// The lobby's query built: what it asks in the project's place.
        fn query(&mut self) {
            match self.event(Event::Query { wanted: self.wanted }) {
                Action::AskAlone { request, filler } => {
                    assert_eq!(filler.config(), match request {
                        SceneRequest::Config(name) => name,
                        SceneRequest::Project(_) => filler.config(),
                    });
                    self.pending = Some(request);
                }
                other => panic!("a swap's query: {other:?}"),
            }
        }

        fn faction(&mut self, lobby: Lobby) {
            self.wanted = lobby;
        }

        fn match_entered(&mut self) {
            if self.event(Event::Hidden { wanted: self.wanted }) == Action::Query {
                self.query();
            }
        }

        fn back(&mut self) {
            self.event(Event::Shown);
        }

        /// The pending query's results come; each re-ask's too.
        fn results(&mut self) {
            while let Some(request) = self.pending.take() {
                let loaded = (self.loads)(request);
                if self.event(Event::Ready { loaded, wanted: self.wanted }) == Action::AskAgain {
                    self.query();
                }
            }
        }

        fn shown(&self) -> Lobby {
            let Scene::Loaded { shown, .. } = self.scene else { panic!("{:?}", self.scene) };
            shown
        }
    }

    #[test]
    fn a_faction_change_shows_after_the_next_match() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.faction(Lobby::Hangar046);
        assert_eq!(game.shown(), Lobby::Classic, "not while the lobby is drawn");
        game.match_entered();
        game.results();
        assert_eq!(game.shown(), Lobby::Hangar046);
        assert!(game.actions.contains(&Action::Swap));
        game.back();
        assert_eq!(game.scene, Scene::Loaded { shown: Lobby::Hangar046, view: View::Drawn, fetch: None });
        let before = game.actions.len();
        game.match_entered();
        game.back();
        assert_eq!(game.actions[before..], [Action::Original, Action::Original], "the same faction: nothing asked");
    }

    #[test]
    fn a_swap_asks_the_config_then_the_project_and_nothing_else() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.faction(Lobby::Camp069);
        game.match_entered();
        game.results();
        assert_eq!(game.actions, [
            Action::Query,
            Action::AskAlone { request: SceneRequest::Config(Lobby::Camp069.config()), filler: Lobby::Camp069 },
            Action::AskAgain,
            Action::AskAlone { request: SceneRequest::Project(Lobby::Camp069.project()), filler: Lobby::Camp069 },
            Action::Swap,
        ]);
    }

    #[test]
    fn back_to_the_classic_is_its_project_at_once() {
        let mut game = Game::new(Lobby::Stk2Bunker, |_| true);
        game.faction(Lobby::Classic);
        game.match_entered();
        game.results();
        assert_eq!(game.shown(), Lobby::Classic);
        assert_eq!(game.actions[1], Action::AskAlone { request: SceneRequest::Project(c"lobby_scene/day"), filler: Lobby::Classic });
    }

    #[test]
    fn a_missing_pack_is_the_classic_and_never_its_project() {
        let missing = |request| request != SceneRequest::Config(Lobby::Stk2Bunker.config());
        let mut game = Game::new(Lobby::Hangar046, missing);
        game.faction(Lobby::Stk2Bunker);
        game.match_entered();
        game.results();
        assert_eq!(game.shown(), Lobby::Classic);
        assert!(!game.actions.iter().any(|a| matches!(a, Action::AskAlone { request: SceneRequest::Project(name), .. }
            if *name == Lobby::Stk2Bunker.project())));
        // the next match asks its config again, finds it missing, keeps the classic
        game.back();
        game.match_entered();
        game.results();
        assert_eq!(game.shown(), Lobby::Classic);
        assert_eq!(*game.actions.last().unwrap(), Action::LetGo);
    }

    #[test]
    fn a_project_without_its_track_is_the_classic() {
        let broken = |request| request != SceneRequest::Project(Lobby::Camp069.project());
        let mut game = Game::new(Lobby::Hangar046, broken);
        game.faction(Lobby::Camp069);
        game.match_entered();
        game.results();
        assert_eq!(game.shown(), Lobby::Classic);
    }

    #[test]
    fn the_classic_failing_keeps_the_lobby_shown() {
        let mut game = Game::new(Lobby::Hangar046, |_| false);
        game.faction(Lobby::Classic);
        game.match_entered();
        game.results();
        assert_eq!(game.shown(), Lobby::Hangar046);
        assert_eq!(*game.actions.last().unwrap(), Action::LetGo);
    }

    #[test]
    fn results_after_the_return_are_let_go_and_asked_at_the_next_match() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.faction(Lobby::Hangar046);
        game.match_entered();
        game.back();
        game.results();
        assert_eq!(game.shown(), Lobby::Classic, "never swapped while drawn");
        assert_eq!(*game.actions.last().unwrap(), Action::LetGo);
        game.match_entered();
        game.results();
        assert_eq!(game.shown(), Lobby::Hangar046);
    }

    #[test]
    fn a_query_in_flight_at_the_next_match_is_taken_then() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.faction(Lobby::Hangar046);
        game.match_entered();
        game.back();
        game.match_entered();
        assert_eq!(*game.actions.last().unwrap(), Action::Original, "one query at a time");
        game.results();
        assert_eq!(game.shown(), Lobby::Hangar046);
    }

    #[test]
    fn a_faction_changed_while_its_query_is_out_asks_the_new_one() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.faction(Lobby::Hangar046);
        game.match_entered();
        game.back();
        game.faction(Lobby::Camp069);
        game.match_entered();
        game.results();
        assert_eq!(game.shown(), Lobby::Camp069);
        // and changed back to the one shown: let go
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.faction(Lobby::Hangar046);
        game.match_entered();
        game.back();
        game.faction(Lobby::Classic);
        game.match_entered();
        game.results();
        assert_eq!(game.shown(), Lobby::Classic);
        assert_eq!(*game.actions.last().unwrap(), Action::LetGo);
    }

    #[test]
    fn closing_asks_nothing_and_lets_a_swap_go() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.faction(Lobby::Hangar046);
        game.event(Event::Closing);
        game.match_entered();
        assert_eq!(game.actions, [Action::Original, Action::Original], "the exit's on_deactivate asks nothing");
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.faction(Lobby::Hangar046);
        game.match_entered();
        game.event(Event::Closing);
        game.results();
        assert_eq!(game.shown(), Lobby::Classic);
        assert_eq!(*game.actions.last().unwrap(), Action::LetGo, "results after the close: let go");
        assert!(matches!(game.scene, Scene::Loaded { view: View::Closed, fetch: None, .. }));
    }

    #[test]
    fn hidden_and_shown_before_the_scene_change_nothing() {
        for scene in [Scene::Unasked, Scene::Starting(Ask::Config(Lobby::Camp069))] {
            assert_eq!(scene.step(Event::Hidden { wanted: Lobby::Hangar046 }), (scene, Action::Original));
            assert_eq!(scene.step(Event::Shown), (scene, Action::Original));
            assert_eq!(scene.step(Event::Closing), (scene, Action::Original));
        }
    }

    /// Every sequence of events (up to a length) from a lobby shown: a swap
    /// only while hidden (Game::event), a query only while hidden, at most
    /// one swap query in flight, and whatever comes the lobby shown is one a
    /// faction asked or the classic.
    #[test]
    fn no_sequence_swaps_while_drawn() {
        const LOADS: [fn(SceneRequest) -> bool; 3] = [|_| true, |_| false, |r| matches!(r, SceneRequest::Project(_))];
        let steps = 6;
        for loads in LOADS {
            for code in 0..(7usize.pow(steps)) {
                let mut game = Game::new(Lobby::Classic, loads);
                let mut factions = std::vec![Lobby::Classic];
                let mut c = code;
                for _ in 0..steps {
                    match c % 7 {
                        0 => game.faction(Lobby::Hangar046),
                        1 => game.faction(Lobby::Classic),
                        2 => game.match_entered(),
                        3 => game.back(),
                        4 => game.results(),
                        5 => {
                            game.event(Event::Closing);
                        }
                        _ => game.faction(Lobby::Camp069),
                    }
                    factions.push(game.wanted);
                    c /= 7;
                }
                game.results();
                assert!(factions.contains(&game.shown()) || game.shown() == Lobby::Classic, "{code}: {:?}", game.scene);
                if let Scene::Loaded { view: View::Drawn, fetch: None, shown } = game.scene {
                    assert_eq!(game.scene.step(Event::Query { wanted: shown }).1, Action::Original);
                }
            }
        }
    }
}
