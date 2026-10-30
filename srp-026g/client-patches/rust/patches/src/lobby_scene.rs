//! Patch 81: the lobby's scene follows the player's quest faction
//! (docs/notes/lobby-scenes.md): Renaissance the classic, the Black Market
//! the S.T.A.L.K.E.R. 2 bunker, the Scavengers the 0.69d camp, the Edge
//! the 0.46c2 hangar; no faction yet (no saved value) the Scavengers. In a
//! squad the lobby is the commander's faction's (`0xA1`'s srp byte,
//! `lobby_client.squad_read`), out of one the player's own.
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
//! A change during a session (a faction chosen, a squad joined or left, its
//! commander's change) shows at once: the lobby's tick
//! (`lobby_camera.view`) asks the lobby's project alone while the old one is
//! drawn; when it is in, the scene fades to black (the lobby movie's
//! `srp_scene_fade`, under every panel), the project is put in the old one's
//! place, the eye adaptation is reset and the camera is put at its stock
//! pose, a few frames are drawn behind the black, and the scene fades in.
//! A lobby that is not there keeps the one shown, with no fade. A match
//! entered (`lobby_menu.deactivate`) puts a project that came in at once;
//! results once the game closes (`game.clear_resources`) are let go. One
//! enum, [`Scene`], and one transition function, [`Scene::step`].

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::language::{ConsoleString, save_user_config};
use crate::vostok::lobby::{
    DrawnLobby, Fade, HiddenLobby, SceneQuery, SceneRequest, ScenesReady, SquadId, SquadRead, arrived_faction,
    release_held,
};
use crate::vostok::lobby_camera::LobbyCamera;
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


/// The player's squad as `0xA1` gives it (read once, at
/// `lobby_client.squad_read`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Squad {
    /// In no squad (squad id 0).
    None,
    /// In one, led by a commander of this quest faction (srp's last byte).
    Led(Faction),
}

impl Squad {
    /// The squad of a squad id and srp's last byte (none when the message
    /// has no such byte: a squad of an srp before it, the player's own lobby).
    pub fn of(squad_id: u32, commander_faction: Option<u8>) -> Squad {
        match (squad_id, commander_faction) {
            (0, _) | (_, None) => Squad::None,
            (_, Some(id)) => Squad::Led(Faction::of_id(id)),
        }
    }

    /// The lobby shown: the commander's faction's in a squad (the owner,
    /// 2026-10-30: "when someone joins your squad, they go into your lobby"),
    /// the player's own out of one.
    pub fn lobby(self, own: Faction) -> Lobby {
        match self {
            Squad::None => Lobby::of_faction(own),
            Squad::Led(commander) => Lobby::of_faction(commander),
        }
    }
}

/// The fade to black, in seconds (the movie's tween is as long).
pub const FADE_OUT: f32 = 0.25;
/// The fade from black.
pub const FADE_IN: f32 = 0.4;
/// The frames the new lobby is drawn behind the black before it fades in
/// (its first frames, the eye adaptation's reset taken: lobby-scenes.md,
/// "A change shown at once").
pub const HOLD_FRAMES: u8 = 6;

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

/// Where a change of lobby is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    /// The lobby shown is the one wanted.
    Settled,
    /// `lobby` is wanted and not there (its pack or project missing): the
    /// one shown stays until another is wanted.
    Missing { lobby: Lobby },
    /// The lobby's query asks `ask` for `wanted`; the old lobby is drawn.
    Loading { wanted: Lobby, ask: Ask },
    /// `to`'s project is held (vostok::lobby's), the scene fading to black;
    /// `left` s of the fade to go.
    Fading { to: Lobby, left: f32 },
    /// The new lobby put in, drawn behind the black `frames` more frames.
    Holding { frames: u8 },
    /// The scene fading in, `left` s to go.
    Appearing { left: f32 },
}

/// Where the lobby's scene is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scene {
    /// The lobby menu has not asked for it.
    Unasked,
    /// The start: the lobby menu's own query asks this (and the lobby menu
    /// takes the project); `wanted` the lobby the start is for.
    Starting { wanted: Lobby, ask: Ask },
    /// The lobby menu has the lobby `shown`.
    Loaded { shown: Lobby, view: View, change: Change },
}

/// What happens to the lobby's scene. `wanted` is the lobby wanted at that
/// moment ([`Squad::lobby`] of the saved faction and the squad).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// The lobby menu builds its query (`lobby_menu.scene_request`).
    Query { wanted: Lobby },
    /// Its results are in (`lobby_menu.scenes_ready`); `loaded`: the
    /// project's place holds what was asked, usable (ScenesReady::loaded).
    Ready { loaded: bool, wanted: Lobby },
    /// A frame of the lobby drawn, `dt` s (`lobby_camera.view`).
    Tick { dt: f32, wanted: Lobby },
    /// The lobby is no longer drawn (`lobby_menu.deactivate`: a match).
    Hidden,
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
    /// A change's query: this in the project's place, the lobby's config
    /// (`filler`) in every other.
    AskAlone { request: SceneRequest, filler: Lobby },
    /// The results let go, the query asked again.
    AskAgain,
    /// The results let go.
    LetGo,
    /// The lobby's query asked (a change's first).
    Query,
    /// The results' project held, the results let go, the scene's fade to
    /// black begun.
    Hold,
    /// The results' project put in the lobby menu's at once (the lobby not
    /// drawn).
    SwapIn,
    /// The held project put in, the eye adaptation reset, the camera at its
    /// stock pose (behind the black).
    Swap,
    /// The eye adaptation reset again and the scene's fade from black begun.
    FadeIn,
    /// The held project put in and the fade gone at once (a match entered).
    SwapHidden,
    /// The fade gone at once (a match entered behind the black).
    ClearFade,
    /// The held project let go (the game closing).
    Release,
}

impl Scene {
    /// What its query asked in the project's place, when it is this
    /// patch's.
    pub fn asked(self) -> Option<SceneRequest> {
        match self {
            Scene::Starting { ask, .. } | Scene::Loaded { change: Change::Loading { ask, .. }, .. } => Some(ask.request()),
            Scene::Unasked | Scene::Loaded { .. } => None,
        }
    }

    /// The machine: the next scene and what the site does, for every scene
    /// and every event. A project is put in behind the black or while the
    /// lobby is not drawn; one change at a time, the last wanted lobby the
    /// next.
    pub fn step(self, event: Event) -> (Scene, Action) {
        use Scene::{Loaded, Starting, Unasked};
        match (self, event) {
            (Unasked, Event::Query { wanted }) => {
                let ask = Ask::first(wanted);
                (Starting { wanted, ask }, Action::Ask(ask.request()))
            }
            (Starting { ask, .. }, Event::Query { .. }) => (self, Action::Ask(ask.request())),
            (Loaded { change: Change::Loading { ask, .. }, .. }, Event::Query { .. }) => {
                (self, Action::AskAlone { request: ask.request(), filler: ask.lobby() })
            }
            (Loaded { .. }, Event::Query { .. }) => (self, Action::Original),

            (Unasked, Event::Ready { .. }) => (self, Action::Original),
            (Starting { wanted, ask: Ask::Config(lobby) }, Event::Ready { loaded, .. }) => {
                let ask = Ask::Project(if loaded { lobby } else { Lobby::Classic });
                (Starting { wanted, ask }, Action::AskAgain)
            }
            (Starting { wanted, ask: Ask::Project(lobby) }, Event::Ready { loaded, .. }) if loaded || lobby == Lobby::Classic => {
                let change = if lobby == wanted { Change::Settled } else { Change::Missing { lobby: wanted } };
                (Loaded { shown: lobby, view: View::Drawn, change }, Action::Original)
            }
            (Starting { wanted, ask: Ask::Project(_) }, Event::Ready { .. }) => {
                (Starting { wanted, ask: Ask::Project(Lobby::Classic) }, Action::AskAgain)
            }
            (Loaded { shown, view, change: Change::Loading { wanted: asked_for, ask } }, Event::Ready { loaded, wanted }) => {
                let at = |change| Loaded { shown, view, change };
                match ask {
                    _ if view == View::Closed => (at(Change::Settled), Action::LetGo),
                    _ if asked_for != wanted && wanted == shown => (at(Change::Settled), Action::LetGo),
                    _ if asked_for != wanted => (at(Change::Loading { wanted, ask: Ask::first(wanted) }), Action::AskAgain),
                    Ask::Config(lobby) if loaded => (at(Change::Loading { wanted, ask: Ask::Project(lobby) }), Action::AskAgain),
                    Ask::Project(lobby) if loaded => match view {
                        View::Drawn => (at(Change::Fading { to: lobby, left: FADE_OUT }), Action::Hold),
                        View::Hidden | View::Closed => (Loaded { shown: lobby, view, change: Change::Settled }, Action::SwapIn),
                    },
                    Ask::Config(_) | Ask::Project(_) => (at(Change::Missing { lobby: wanted }), Action::LetGo),
                }
            }
            (Loaded { .. }, Event::Ready { .. }) => (self, Action::Original),

            (Loaded { shown, view: View::Drawn, change }, Event::Tick { dt, wanted }) => {
                let at = |change| Loaded { shown, view: View::Drawn, change };
                let load = || (at(Change::Loading { wanted, ask: Ask::first(wanted) }), Action::Query);
                match change {
                    Change::Settled | Change::Missing { .. } if wanted == shown => (at(Change::Settled), Action::Original),
                    Change::Missing { lobby } if lobby == wanted => (self, Action::Original),
                    Change::Settled | Change::Missing { .. } => load(),
                    Change::Loading { .. } => (self, Action::Original),
                    Change::Fading { to, left } if left > dt => (at(Change::Fading { to, left: left - dt }), Action::Original),
                    Change::Fading { to, .. } => {
                        (Loaded { shown: to, view: View::Drawn, change: Change::Holding { frames: HOLD_FRAMES } }, Action::Swap)
                    }
                    Change::Holding { frames } if frames > 1 => (at(Change::Holding { frames: frames - 1 }), Action::Original),
                    Change::Holding { .. } => (at(Change::Appearing { left: FADE_IN }), Action::FadeIn),
                    Change::Appearing { left } if left > dt => (at(Change::Appearing { left: left - dt }), Action::Original),
                    Change::Appearing { .. } => (at(Change::Settled), Action::Original),
                }
            }
            (Unasked | Starting { .. } | Loaded { .. }, Event::Tick { .. }) => (self, Action::Original),

            (Loaded { view: View::Closed, .. }, Event::Hidden | Event::Shown | Event::Closing) => (self, Action::Original),
            (Loaded { shown, change, .. }, Event::Closing) => match change {
                Change::Fading { .. } => (Loaded { shown, view: View::Closed, change: Change::Settled }, Action::Release),
                Change::Loading { .. } => (Loaded { shown, view: View::Closed, change }, Action::Original),
                Change::Settled | Change::Missing { .. } | Change::Holding { .. } | Change::Appearing { .. } => {
                    (Loaded { shown, view: View::Closed, change: Change::Settled }, Action::Original)
                }
            },
            (Unasked | Starting { .. }, Event::Closing) => (self, Action::Original),
            (Loaded { shown, change, .. }, Event::Hidden) => {
                let hidden = |shown, change| Loaded { shown, view: View::Hidden, change };
                match change {
                    Change::Fading { to, .. } => (hidden(to, Change::Settled), Action::SwapHidden),
                    Change::Holding { .. } | Change::Appearing { .. } => (hidden(shown, Change::Settled), Action::ClearFade),
                    Change::Settled | Change::Missing { .. } | Change::Loading { .. } => (hidden(shown, change), Action::Original),
                }
            }
            (Unasked | Starting { .. }, Event::Hidden) => (self, Action::Original),

            (Loaded { shown, change, .. }, Event::Shown) => (Loaded { shown, view: View::Drawn, change }, Action::Original),
            (Unasked | Starting { .. }, Event::Shown) => (self, Action::Original),
        }
    }
}

/// The last quest faction the server gave, as user.cfg keeps it
/// (`srp_lobby_faction <id>`): the player's own lobby, and the next start's.
static SAVED: ConsoleString<4> = ConsoleString::new();

/// The squad `0xA1` last gave (none until it comes: a start's lobby is the
/// saved faction's).
static SQUAD: GameCell<Squad> = GameCell::new(Squad::None);

static SCENE: GameCell<Scene> = GameCell::new(Scene::Unasked);

pub fn on_configs_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    SAVED.register(ctx.thread(), c"srp_lobby_faction");
    Flow::Continue
}

/// Keeps the player's own faction for the next start (user.cfg written at
/// once when it is another than the saved one).
fn save(thread: &Thread, faction: Faction) {
    if Faction::parse(SAVED.get(thread)) != faction {
        SAVED.set(thread, faction.saved());
        save_user_config(thread);
    }
}

/// A faction from the server (`0x8E`): the player's own; its lobby shown
/// at the next tick when no squad's is.
pub fn on_faction(ctx: &mut Context<'_>) -> Flow<NoExit> {
    save(ctx.thread(), Faction::of_id(arrived_faction(ctx)));
    Flow::Continue
}

/// `0xA1` with its squad id read: none (0) is out of a squad, the player's
/// own lobby shown at the next tick (a squad's id goes on to `on_squad`).
pub fn on_squad_id(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if SquadId::at(ctx).is_some_and(|id| id.get() == 0) {
        SQUAD.set(ctx.thread(), Squad::None);
    }
    Flow::Continue
}

/// A squad from the server (`0xA1`, read to its end): its commander's lobby
/// shown at the next tick.
pub fn on_squad(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(read) = SquadRead::at(ctx) {
        SQUAD.set(ctx.thread(), Squad::of(read.squad_id(), read.commander_faction()));
    }
    Flow::Continue
}

/// A diagnostics build's change of the inputs (camera_sweep.rs): the
/// player's own faction chosen, or a squad joined or left.
pub fn force(thread: &Thread, own: Option<Faction>, squad: Option<Squad>) {
    if let Some(faction) = own {
        save(thread, faction);
    }
    if let Some(squad) = squad {
        SQUAD.set(thread, squad);
    }
}

/// The lobby wanted now.
fn wanted(thread: &Thread) -> Lobby {
    SQUAD.get(thread).lobby(Faction::parse(SAVED.get(thread)))
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
            Action::Original
            | Action::AskAgain
            | Action::LetGo
            | Action::Query
            | Action::Hold
            | Action::SwapIn
            | Action::Swap
            | Action::FadeIn
            | Action::SwapHidden
            | Action::ClearFade
            | Action::Release => {}
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
        Action::Hold => {
            ready.hold();
            ready.fade(Fade::Out { seconds: FADE_OUT });
        }
        Action::SwapIn => ready.swap_in(),
        Action::LetGo => {}
        // the lobby menu takes them; the other actions are not a result's
        Action::Original
        | Action::Ask(_)
        | Action::AskAlone { .. }
        | Action::Query
        | Action::Swap
        | Action::FadeIn
        | Action::SwapHidden
        | Action::ClearFade
        | Action::Release => return Flow::Continue,
    }
    Flow::Exit(LobbySceneOnScenesReadyExit::Done)
}

/// A frame of the lobby drawn: a change asked, or its fade, swap and fade
/// in stepped.
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some((mut camera, dt)) = LobbyCamera::at(ctx) else { return Flow::Continue };
    let action = step(thread, Event::Tick { dt, wanted: wanted(thread) });
    let Some(lobby) = DrawnLobby::of(&camera) else { return Flow::Continue };
    match action {
        Action::Query => lobby.ask(),
        Action::Swap => {
            lobby.swap_held();
            lobby.reset_eye_adaptation();
            camera.to_stock_pose();
        }
        Action::FadeIn => {
            lobby.reset_eye_adaptation();
            lobby.fade(Fade::In { seconds: FADE_IN });
        }
        // nothing to do this frame; the other actions are not a tick's
        Action::Original
        | Action::Ask(_)
        | Action::AskAlone { .. }
        | Action::AskAgain
        | Action::LetGo
        | Action::Hold
        | Action::SwapIn
        | Action::SwapHidden
        | Action::ClearFade
        | Action::Release => {}
    }
    Flow::Continue
}

/// A match entered: a held project put in at once, a fade gone.
pub fn on_deactivate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let action = step(ctx.thread(), Event::Hidden);
    let Some(lobby) = HiddenLobby::at(ctx) else { return Flow::Continue };
    match action {
        Action::SwapHidden => {
            lobby.swap_held();
            lobby.fade(Fade::Gone);
        }
        Action::ClearFade => lobby.fade(Fade::Gone),
        _ => {}
    }
    Flow::Continue
}

/// The lobby shown again.
pub fn on_activate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    step(ctx.thread(), Event::Shown);
    Flow::Continue
}

/// The game closes: a held project let go, results from now on let go
/// without the lobby menu.
pub fn on_game_closing(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if step(ctx.thread(), Event::Closing) == Action::Release {
        release_held(ctx.thread());
    }
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
                    let Scene::Loaded { shown, view: View::Drawn, .. } = scene else { panic!("{scene:?}") };
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
    fn a_start_on_a_missing_lobby_does_not_ask_it_again() {
        let mut scene = Scene::Unasked;
        let wanted = Lobby::Camp069;
        for loaded in [false, true] {
            scene = scene.step(Event::Query { wanted }).0;
            scene = scene.step(Event::Ready { loaded, wanted }).0;
        }
        assert_eq!(scene, Scene::Loaded { shown: Lobby::Classic, view: View::Drawn, change: Change::Missing { lobby: wanted } });
        assert_eq!(scene.step(Event::Tick { dt: 0.016, wanted }), (scene, Action::Original));
    }

    #[test]
    fn results_this_patch_did_not_ask_for_are_the_lobby_menus() {
        let wanted = Lobby::Camp069;
        assert_eq!(Scene::Unasked.step(Event::Ready { loaded: false, wanted }), (Scene::Unasked, Action::Original));
        let shown = Scene::Loaded { shown: Lobby::Camp069, view: View::Drawn, change: Change::Settled };
        assert_eq!(shown.step(Event::Ready { loaded: false, wanted }), (shown, Action::Original));
        assert_eq!(shown.step(Event::Query { wanted }), (shown, Action::Original));
        assert_eq!(Scene::Unasked.asked(), None);
    }

    #[test]
    fn the_squad_commanders_lobby_is_shown_in_a_squad() {
        assert_eq!(Squad::of(0, Some(2)), Squad::None);
        assert_eq!(Squad::of(7, None), Squad::None);
        assert_eq!(Squad::of(7, Some(4)), Squad::Led(Faction::Edge));
        assert_eq!(Squad::of(7, Some(0)), Squad::Led(Faction::Scavengers));
        assert_eq!(Squad::None.lobby(Faction::Renaissance), Lobby::Classic);
        assert_eq!(Squad::Led(Faction::BlackMarket).lobby(Faction::Renaissance), Lobby::Stk2Bunker);
        assert_eq!(Squad::Led(Faction::Renaissance).lobby(Faction::Edge), Lobby::Classic);
    }

    const FRAME: f32 = 1.0 / 60.0;

    /// A game after its start: the scene and each event's action, every
    /// query and result played as the game plays them (`loads` says whether
    /// what was asked is there), and what the screen shows.
    struct Game {
        scene: Scene,
        wanted: Lobby,
        loads: fn(SceneRequest) -> bool,
        /// A change's query asked and its results not yet in.
        pending: Option<SceneRequest>,
        /// A project held for a swap (vostok::lobby's HELD).
        held: Option<Lobby>,
        /// The lobby the lobby menu has.
        menu: Lobby,
        /// The movie's fade.
        black: bool,
        actions: std::vec::Vec<Action>,
    }

    impl Game {
        fn new(shown: Lobby, loads: fn(SceneRequest) -> bool) -> Game {
            Game {
                scene: Scene::Loaded { shown, view: View::Drawn, change: Change::Settled },
                wanted: shown,
                loads,
                pending: None,
                held: None,
                menu: shown,
                black: false,
                actions: std::vec![],
            }
        }

        fn view(&self) -> View {
            let Scene::Loaded { view, .. } = self.scene else { panic!("{:?}", self.scene) };
            view
        }

        fn event(&mut self, event: Event) -> Action {
            let before = self.scene;
            let (scene, action) = before.step(event);
            self.scene = scene;
            self.actions.push(action);
            let asked = |s: Scene| match s {
                Scene::Loaded { change: Change::Loading { ask, .. }, .. } => Some(ask.lobby()),
                _ => None,
            };
            match action {
                Action::Hold => {
                    assert_eq!(self.view(), View::Drawn);
                    assert!(self.held.is_none(), "one held at a time");
                    self.held = asked(before);
                    self.black = true;
                }
                Action::SwapIn => {
                    assert_ne!(self.view(), View::Drawn, "a swap at once only while not drawn");
                    self.menu = asked(before).unwrap();
                }
                Action::Swap => {
                    assert!(self.black, "a swap behind the black");
                    self.menu = self.held.take().expect("a project held");
                }
                Action::SwapHidden => {
                    self.menu = self.held.take().expect("a project held");
                    self.black = false;
                }
                Action::FadeIn | Action::ClearFade => self.black = false,
                Action::Release => self.held = None.or(self.held.take().and(None)),
                Action::Query => assert_eq!(self.view(), View::Drawn),
                Action::Original | Action::Ask(_) | Action::AskAlone { .. } | Action::AskAgain | Action::LetGo => {}
            }
            if let Scene::Loaded { shown, change, .. } = self.scene {
                if !matches!(change, Change::Fading { .. }) {
                    assert!(self.held.is_none() || matches!(self.scene, Scene::Loaded { view: View::Closed, .. }), "{:?}", self.scene);
                }
                if self.view() != View::Closed {
                    assert_eq!(shown, self.menu, "the scene's lobby is the menu's");
                }
            }
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
                other => panic!("a change's query: {other:?}"),
            }
        }

        fn want(&mut self, lobby: Lobby) {
            self.wanted = lobby;
        }

        fn tick(&mut self) {
            if self.event(Event::Tick { dt: FRAME, wanted: self.wanted }) == Action::Query {
                self.query();
            }
        }

        fn ticks(&mut self, n: usize) {
            for _ in 0..n {
                self.tick();
            }
        }

        fn match_entered(&mut self) {
            self.event(Event::Hidden);
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

        fn settled(&self) -> bool {
            matches!(self.scene, Scene::Loaded { change: Change::Settled, .. })
        }
    }

    /// Ticks until the change is done (at most `frames`).
    fn settle(game: &mut Game, frames: usize) -> usize {
        for frame in 0..frames {
            if game.settled() && !game.black && game.pending.is_none() && game.shown() == game.wanted {
                return frame;
            }
            game.tick();
            game.results();
        }
        frames
    }

    #[test]
    fn a_change_loads_behind_the_old_lobby_then_fades_swaps_and_fades_in() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Stk2Bunker);
        game.tick();
        assert_eq!(game.actions, [
            Action::Query,
            Action::AskAlone { request: SceneRequest::Config(Lobby::Stk2Bunker.config()), filler: Lobby::Stk2Bunker },
        ]);
        game.ticks(3);
        assert_eq!(game.shown(), Lobby::Classic, "the old lobby drawn while the new one loads");
        assert!(!game.black);
        game.results();
        assert!(game.black, "the fade begun");
        assert_eq!(game.shown(), Lobby::Classic);
        let out = (FADE_OUT / FRAME).round() as usize;
        let mut frames = 0;
        while game.menu == Lobby::Classic {
            game.tick();
            frames += 1;
            assert!(frames <= out + 1, "swapped once black");
        }
        assert!(frames >= out, "not swapped before the black: {frames} of {out}");
        assert_eq!(*game.actions.last().unwrap(), Action::Swap);
        game.ticks(usize::from(HOLD_FRAMES) - 1);
        assert!(game.black, "the new lobby's first frames behind the black");
        game.tick();
        assert_eq!(*game.actions.last().unwrap(), Action::FadeIn);
        assert!(!game.black);
        game.ticks((FADE_IN / FRAME).ceil() as usize);
        assert!(game.settled());
        let before = game.actions.len();
        game.ticks(10);
        assert!(game.actions[before..].iter().all(|a| *a == Action::Original), "nothing more asked");
    }

    #[test]
    fn a_missing_pack_keeps_the_lobby_with_no_fade_and_is_not_asked_again() {
        let missing = |request| request != SceneRequest::Config(Lobby::Hangar046.config());
        let mut game = Game::new(Lobby::Camp069, missing);
        game.want(Lobby::Hangar046);
        game.tick();
        game.results();
        assert!(!game.black);
        assert_eq!(game.shown(), Lobby::Camp069);
        let before = game.actions.len();
        game.ticks(30);
        assert!(game.actions[before..].iter().all(|a| *a == Action::Original), "never asked again for the same lobby");
        // another lobby wanted: asked; the one shown again: settled
        game.want(Lobby::Camp069);
        game.tick();
        assert!(game.settled());
    }

    #[test]
    fn a_project_without_its_track_keeps_the_lobby() {
        let broken = |request| request != SceneRequest::Project(Lobby::Camp069.project());
        let mut game = Game::new(Lobby::Hangar046, broken);
        game.want(Lobby::Camp069);
        game.tick();
        game.results();
        assert_eq!(game.shown(), Lobby::Hangar046);
        assert!(!game.black);
        assert!(matches!(game.scene, Scene::Loaded { change: Change::Missing { lobby: Lobby::Camp069 }, .. }));
    }

    #[test]
    fn the_last_lobby_wanted_wins() {
        // changed while its query is out: the new one asked
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.tick();
        game.want(Lobby::Camp069);
        game.results();
        assert!(settle(&mut game, 200) < 200);
        assert_eq!(game.shown(), Lobby::Camp069);
        assert!(!game.actions.iter().any(|a| matches!(a, Action::AskAlone { request: SceneRequest::Project(p), .. } if *p == Lobby::Hangar046.project())));
        // changed back to the one shown while out: let go, no fade
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.tick();
        game.want(Lobby::Classic);
        game.results();
        assert!(game.settled() && !game.black);
        assert_eq!(*game.actions.last().unwrap(), Action::LetGo);
        // changed during the fade: that change done, then the next
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.tick();
        game.results();
        game.want(Lobby::Stk2Bunker);
        settle(&mut game, 400);
        assert_eq!(game.shown(), Lobby::Stk2Bunker);
        assert_eq!(game.actions.iter().filter(|a| **a == Action::Swap).count(), 2);
    }

    #[test]
    fn a_match_entered_mid_change_puts_the_lobby_in_and_the_return_shows_it() {
        for at in 0..60 {
            let mut game = Game::new(Lobby::Classic, |_| true);
            game.want(Lobby::Hangar046);
            game.tick();
            for _ in 0..at {
                game.results();
                game.tick();
            }
            game.match_entered();
            game.results();
            assert!(!game.black, "{at}: no black left over the lobby");
            assert_eq!(game.shown(), Lobby::Hangar046, "{at}");
            game.back();
            settle(&mut game, 100);
            assert_eq!(game.shown(), Lobby::Hangar046, "{at}");
            assert!(!game.black, "{at}");
        }
    }

    #[test]
    fn closing_lets_a_held_project_and_late_results_go() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.tick();
        game.results();
        assert!(game.held.is_some());
        game.event(Event::Closing);
        assert_eq!(*game.actions.last().unwrap(), Action::Release);
        assert!(game.held.is_none());
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.tick();
        game.event(Event::Closing);
        game.results();
        assert_eq!(*game.actions.last().unwrap(), Action::LetGo, "results after the close: let go");
        assert!(matches!(game.scene, Scene::Loaded { view: View::Closed, change: Change::Settled, .. }));
        game.tick();
        game.match_entered();
        assert_eq!(game.actions[game.actions.len() - 2..], [Action::Original, Action::Original]);
    }

    #[test]
    fn hidden_and_shown_before_the_scene_change_nothing() {
        for scene in [Scene::Unasked, Scene::Starting { wanted: Lobby::Camp069, ask: Ask::Config(Lobby::Camp069) }] {
            assert_eq!(scene.step(Event::Hidden), (scene, Action::Original));
            assert_eq!(scene.step(Event::Shown), (scene, Action::Original));
            assert_eq!(scene.step(Event::Closing), (scene, Action::Original));
            assert_eq!(scene.step(Event::Tick { dt: FRAME, wanted: Lobby::Hangar046 }), (scene, Action::Original));
        }
    }

    /// Every sequence of events (up to a length) from a lobby shown: a swap
    /// only behind the black or while not drawn, a query only while drawn,
    /// one change at a time (Game::event), no black left once settled, and
    /// whatever comes the lobby shown is one that was wanted.
    #[test]
    fn no_sequence_swaps_in_sight_or_leaves_the_black() {
        const LOADS: [fn(SceneRequest) -> bool; 3] = [|_| true, |_| false, |r| matches!(r, SceneRequest::Project(_))];
        const KINDS: usize = 8;
        let steps = 6;
        for loads in LOADS {
            for code in 0..KINDS.pow(steps) {
                let mut game = Game::new(Lobby::Classic, loads);
                let mut wanted = std::vec![Lobby::Classic];
                let mut c = code;
                for _ in 0..steps {
                    match c % KINDS {
                        0 => game.want(Lobby::Hangar046),
                        1 => game.want(Lobby::Classic),
                        2 => game.match_entered(),
                        3 => game.back(),
                        4 => game.results(),
                        5 => {
                            game.event(Event::Closing);
                        }
                        6 => game.ticks(9),
                        _ => game.want(Lobby::Camp069),
                    }
                    wanted.push(game.wanted);
                    c /= KINDS;
                }
                game.results();
                if game.view() == View::Hidden {
                    game.back();
                }
                if game.view() == View::Drawn {
                    settle(&mut game, 400);
                    assert!(!game.black, "{code}: {:?}", game.scene);
                    assert!(game.held.is_none(), "{code}");
                }
                assert!(wanted.contains(&game.shown()), "{code}: {:?}", game.scene);
            }
        }
    }
}
