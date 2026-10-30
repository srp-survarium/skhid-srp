//! Patch 81: the lobby's scene is the one the player picks in the lobby
//! window (docs/notes/lobby-scenes.md): the Room (the 0.69d camp, the
//! default), the Basement (the classic), the Bunker (S.T.A.L.K.E.R. 2) or
//! the Hangar (0.46c2). In a squad the lobby is its commander's pick.
//!
//! The pick is kept in user.cfg as the console variable `srp_lobby`
//! (registered before user.cfg runs, `configs.start`, after patch 79's
//! handler), and the start shows it: its lobby takes the classic's place in
//! the scene's query (`lobby_menu.scene_request`). A project whose config is
//! missing would crash the project's cook, so another lobby than the
//! classic is first asked as its config alone: when the results come
//! (`lobby_menu.scenes_ready`) the query is asked again, with the lobby's
//! project when its config is there, else with the classic's. A project
//! without its camera track is the classic too.
//!
//! The lobby window sends a pick to srp itself (the movie's `3B` at
//! `2 095 000 000 + id`); srp sends it back in `0xA1` with the squad
//! commander's (two bytes after the squad, `lobby_client.squad_id` and
//! `lobby_client.squad_read`); the exe sends the saved pick when srp has none
//! (the sign-in). The lobby wanted is the commander's pick in a squad, the
//! player's own out of one.
//!
//! A change shows at once: the lobby's tick (`lobby_camera.view`) asks the
//! lobby's config alone while the old lobby is drawn; when it is there the
//! scene fades to black (the lobby movie's `srp_scene_fade`, under every
//! panel, the loading spinner on it), its project is asked and put in the
//! old one's place behind the black, the eye adaptation is reset and the
//! camera put at its stock pose, a few frames are drawn behind the black,
//! and the scene fades in. A lobby not there keeps the one shown, with no
//! fade. A match entered (`lobby_menu.deactivate`) puts a project that comes
//! in at once; results once the game closes (`game.clear_resources`) are let
//! go. One enum, [`Scene`], and one transition function, [`Scene::step`].

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::language::{ConsoleString, save_user_config};
use crate::vostok::lobby::{
    DrawnLobby, HiddenLobby, Picks, SceneQuery, SceneRequest, ScenesReady, Shade, SquadId, SquadRead,
};
use crate::vostok::lobby_camera::LobbyCamera;
use crate::vostok::clock;
use crate::vostok::trace::{self, TraceFile};
use crate::vostok::{Context, Flow, GameCell, LobbySceneOnScenesReadyExit, NoExit, Thread};

/// A lobby scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lobby {
    /// The Room: the 0.69d camp's room, the default.
    Camp069,
    /// The Basement: the 0.26g lobby, `lobby_scene/day` (the stock game's).
    Classic,
    /// The Bunker: the S.T.A.L.K.E.R. 2 (2012) bomb shelter.
    Stk2Bunker,
    /// The Hangar: the 0.46c2 hangar.
    Hangar046,
}

impl Lobby {
    /// In the window's order (and the ids' 1 to 4).
    pub const ALL: [Lobby; 4] = [Lobby::Camp069, Lobby::Classic, Lobby::Stk2Bunker, Lobby::Hangar046];

    /// Its id: user.cfg's `srp_lobby`, the lobby window's card, srp's
    /// `3B` start (`2 095 000 000 + id`) and `0xA1`'s bytes (0 none).
    pub fn id(self) -> u8 {
        match self {
            Lobby::Camp069 => 1,
            Lobby::Classic => 2,
            Lobby::Stk2Bunker => 3,
            Lobby::Hangar046 => 4,
        }
    }

    /// The lobby of an id (none for 0 and any other).
    pub fn of_id(id: u8) -> Option<Lobby> {
        Lobby::ALL.into_iter().find(|lobby| lobby.id() == id)
    }

    /// The pick a saved value names; no value or any other text the Room.
    pub fn parse(saved: &[u8]) -> Lobby {
        match saved {
            [digit @ b'0'..=b'9'] => Lobby::of_id(digit - b'0').unwrap_or(Lobby::Camp069),
            _ => Lobby::Camp069,
        }
    }

    /// srp's `3B` start of its pick (the server's `lobby_pick::LOBBY_PICK_BASE`
    /// + its id).
    pub fn pick_start(self) -> u32 {
        PICK_BASE + u32::from(self.id())
    }

    /// Its value as user.cfg keeps it (one decimal digit).
    pub fn saved(self) -> &'static [u8] {
        match self {
            Lobby::Camp069 => b"1",
            Lobby::Classic => b"2",
            Lobby::Stk2Bunker => b"3",
            Lobby::Hangar046 => b"4",
        }
    }

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
}

/// srp's start of a lobby pick (`3B`; its `lobby_pick::LOBBY_PICK_BASE`,
/// 2 095 000 000): a ranking start, not an address, so made of small
/// numbers for the layers' check of literals.
pub const PICK_BASE: u32 = 2095 * 1000 * 1000;

/// Lobbies whose packs are there (a bit an id).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lobbies(u8);

impl Lobbies {
    pub const ALL: Lobbies = Lobbies(0b1111);

    pub fn of(found: impl Fn(Lobby) -> bool) -> Lobbies {
        Lobbies(Lobby::ALL.into_iter().filter(|&lobby| found(lobby)).fold(0, |bits, lobby| bits | 1 << (lobby.id() - 1)))
    }

    pub fn has(self, lobby: Lobby) -> bool {
        self.0 & 1 << (lobby.id() - 1) != 0
    }

    /// Without `lobby` (its pack found missing).
    pub fn without(self, lobby: Lobby) -> Lobbies {
        Lobbies(self.0 & !(1 << (lobby.id() - 1)))
    }

    /// As the lobby window takes it (bit `id - 1`).
    pub fn bits(self) -> u32 {
        u32::from(self.0)
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
/// `lobby_client.squad_id` and `lobby_client.squad_read`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Squad {
    /// In no squad, or its commander picked none.
    None,
    /// In one, its commander's pick this lobby.
    Led(Lobby),
}

impl Squad {
    /// The lobby shown: the commander's pick in a squad (the owner,
    /// 2026-10-30: "when someone joins your squad, they go into your
    /// lobby"), the player's own out of one.
    pub fn lobby(self, own: Lobby) -> Lobby {
        match self {
            Squad::None => own,
            Squad::Led(commander) => commander,
        }
    }
}

/// The fade to black, in seconds of the lobby's frames.
pub const FADE_OUT: f32 = 0.25;
/// The fade from black.
pub const FADE_IN: f32 = 0.4;
/// The most of a frame a fade steps by: after a stall the fade still plays
/// over its frames.
pub const FADE_STEP: f32 = 1.0 / 30.0;
/// The frames the new lobby is drawn behind the black before it fades in
/// (its first frames; the eye adaptation's reset taken).
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
    /// The lobby shown is the one wanted (or the wanted one is not there).
    Settled,
    /// The census's query out: which lobbies' packs are there.
    Census,
    /// `wanted`'s config asked alone, the old lobby drawn.
    Probing { wanted: Lobby },
    /// The scene fading to black, `to` next; `black` of the way (0 to 1).
    Darkening { to: Lobby, black: f32 },
    /// Behind the black, the query asks `ask` for `to`.
    Loading { to: Lobby, ask: Ask },
    /// The new lobby in, drawn behind the black `frames` more frames.
    Holding { frames: u8 },
    /// The scene fading in, `black` of the way left.
    Brightening { black: f32 },
}

/// Where the lobby's scene is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scene {
    /// The lobby menu has not asked for it.
    Unasked,
    /// The start: the lobby menu's own query asks this (and the lobby menu
    /// takes the project).
    Starting(Ask),
    /// The lobby menu has the lobby `shown`; `there` the lobbies whose packs
    /// are there (none: not looked for yet, the first frame's census).
    Loaded { shown: Lobby, view: View, change: Change, there: Option<Lobbies> },
}

/// What happens to the lobby's scene. `wanted` is the lobby wanted at that
/// moment ([`Squad::lobby`] of the saved pick and the squad).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// The lobby menu builds its query (`lobby_menu.scene_request`).
    Query { wanted: Lobby },
    /// Its results are in (`lobby_menu.scenes_ready`); `loaded`: the
    /// project's place holds what was asked, usable (ScenesReady::loaded);
    /// `found`: for a census, the lobbies whose configs came.
    Ready { loaded: bool, found: Lobbies, wanted: Lobby },
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
    /// The census's query: every lobby's config.
    AskCensus,
    /// The results let go, the query asked again.
    AskAgain,
    /// The results let go.
    LetGo,
    /// The lobby's query asked.
    Query,
    /// The black at `per_mille` of its way, the loading spinner on it or not.
    Shade { per_mille: u32, loading: bool },
    /// The black whole and the spinner on it, the lobby's query asked.
    ShadeAndQuery,
    /// The results' project put in the lobby menu's (behind the black, or
    /// the lobby not drawn), the eye adaptation reset and the camera at its
    /// stock pose.
    SwapIn,
    /// The black whole, the spinner off, the eye adaptation reset again.
    Reset,
    /// The black gone at once.
    Clear,
    /// The results' project put in and the black gone at once (a match
    /// entered).
    SwapAndClear,
}

/// The black's per mille of `black` (0 to 1).
fn per_mille(black: f32) -> u32 {
    (black.clamp(0.0, 1.0) * 1000.0) as u32
}

impl Scene {
    /// What its query asked in the project's place, when it is this
    /// patch's (none for a census: its own list).
    pub fn asked(self) -> Option<SceneRequest> {
        match self {
            Scene::Starting(ask) | Scene::Loaded { change: Change::Loading { ask, .. }, .. } => Some(ask.request()),
            Scene::Loaded { change: Change::Probing { wanted }, .. } => Some(Ask::Config(wanted).request()),
            Scene::Unasked | Scene::Loaded { .. } => None,
        }
    }

    /// The machine: the next scene and what the site does, for every scene
    /// and every event. A project is put in only behind the black or while
    /// the lobby is not drawn; one change at a time, the last wanted lobby
    /// the next.
    pub fn step(self, event: Event) -> (Scene, Action) {
        use Scene::{Loaded, Starting, Unasked};
        match (self, event) {
            (Unasked, Event::Query { wanted }) => (Starting(Ask::first(wanted)), Action::Ask(Ask::first(wanted).request())),
            (Starting(ask), Event::Query { .. }) => (self, Action::Ask(ask.request())),
            (Loaded { change: Change::Census, .. }, Event::Query { .. }) => (self, Action::AskCensus),
            (Loaded { change: Change::Probing { wanted }, .. }, Event::Query { .. }) => {
                (self, Action::AskAlone { request: Ask::Config(wanted).request(), filler: wanted })
            }
            (Loaded { change: Change::Loading { ask, .. }, .. }, Event::Query { .. }) => {
                (self, Action::AskAlone { request: ask.request(), filler: ask.lobby() })
            }
            (Loaded { .. }, Event::Query { .. }) => (self, Action::Original),

            (Unasked, Event::Ready { .. }) => (self, Action::Original),
            (Starting(Ask::Config(lobby)), Event::Ready { loaded, .. }) => {
                (Starting(Ask::Project(if loaded { lobby } else { Lobby::Classic })), Action::AskAgain)
            }
            (Starting(Ask::Project(lobby)), Event::Ready { loaded, .. }) if loaded || lobby == Lobby::Classic => {
                (Loaded { shown: lobby, view: View::Drawn, change: Change::Settled, there: None }, Action::Original)
            }
            (Starting(Ask::Project(_)), Event::Ready { .. }) => (Starting(Ask::Project(Lobby::Classic)), Action::AskAgain),
            (Loaded { shown, view, change, there }, Event::Ready { loaded, found, wanted }) => {
                let at = |change, there| Loaded { shown, view, change, there };
                match change {
                    _ if view == View::Closed => match change {
                        Change::Census | Change::Probing { .. } | Change::Loading { .. } => {
                            (at(Change::Settled, there), Action::LetGo)
                        }
                        _ => (self, Action::Original),
                    },
                    Change::Census => (at(Change::Settled, Some(found)), Action::LetGo),
                    Change::Probing { wanted: asked } if asked != wanted && wanted == shown => {
                        (at(Change::Settled, there), Action::LetGo)
                    }
                    Change::Probing { wanted: asked } if asked != wanted => {
                        (at(Change::Probing { wanted }, there), Action::AskAgain)
                    }
                    Change::Probing { wanted } if loaded => match view {
                        View::Drawn => (at(Change::Darkening { to: wanted, black: 0.0 }, there), Action::LetGo),
                        View::Hidden | View::Closed => {
                            (at(Change::Loading { to: wanted, ask: Ask::Project(wanted) }, there), Action::AskAgain)
                        }
                    },
                    Change::Probing { wanted } => (at(Change::Settled, there.map(|t| t.without(wanted))), Action::LetGo),
                    Change::Loading { to, .. } if to != wanted && wanted == shown => {
                        (at(Change::Brightening { black: 1.0 }, there), Action::Reset)
                    }
                    Change::Loading { to, .. } if to != wanted => {
                        (at(Change::Loading { to: wanted, ask: Ask::first(wanted) }, there), Action::AskAgain)
                    }
                    Change::Loading { to, ask: Ask::Config(_) } if loaded => {
                        (at(Change::Loading { to, ask: Ask::Project(to) }, there), Action::AskAgain)
                    }
                    Change::Loading { to, ask: Ask::Project(_) } if loaded => match view {
                        View::Drawn => {
                            (Loaded { shown: to, view, change: Change::Holding { frames: HOLD_FRAMES }, there }, Action::SwapIn)
                        }
                        View::Hidden | View::Closed => {
                            (Loaded { shown: to, view, change: Change::Settled, there }, Action::SwapAndClear)
                        }
                    },
                    Change::Loading { to, .. } => {
                        let there = there.map(|t| t.without(to));
                        match view {
                            View::Drawn => (at(Change::Brightening { black: 1.0 }, there), Action::Reset),
                            View::Hidden | View::Closed => (at(Change::Settled, there), Action::LetGo),
                        }
                    }
                    Change::Settled | Change::Darkening { .. } | Change::Holding { .. } | Change::Brightening { .. } => {
                        (self, Action::Original)
                    }
                }
            }

            (Loaded { shown, view: View::Drawn, change, there }, Event::Tick { dt, wanted }) => {
                let at = |change| Loaded { shown, view: View::Drawn, change, there };
                let step = dt.min(FADE_STEP);
                match change {
                    Change::Settled if there.is_none() => (at(Change::Census), Action::Query),
                    Change::Settled if wanted != shown && there.is_some_and(|t| t.has(wanted)) => {
                        (at(Change::Probing { wanted }), Action::Query)
                    }
                    Change::Settled | Change::Census | Change::Probing { .. } | Change::Loading { .. } => {
                        (self, Action::Original)
                    }
                    Change::Darkening { to, black } if black + step / FADE_OUT < 1.0 => {
                        let black = black + step / FADE_OUT;
                        (at(Change::Darkening { to, black }), Action::Shade { per_mille: per_mille(black), loading: false })
                    }
                    Change::Darkening { to, .. } => (at(Change::Loading { to, ask: Ask::Project(to) }), Action::ShadeAndQuery),
                    Change::Holding { frames } if frames > 1 => (at(Change::Holding { frames: frames - 1 }), Action::Original),
                    Change::Holding { .. } => (at(Change::Brightening { black: 1.0 }), Action::Reset),
                    Change::Brightening { black } if black - step / FADE_IN > 0.0 => {
                        let black = black - step / FADE_IN;
                        (at(Change::Brightening { black }), Action::Shade { per_mille: per_mille(black), loading: false })
                    }
                    Change::Brightening { .. } => (at(Change::Settled), Action::Clear),
                }
            }
            (Unasked | Starting(_) | Loaded { .. }, Event::Tick { .. }) => (self, Action::Original),

            (Loaded { view: View::Closed, .. }, Event::Hidden | Event::Shown | Event::Closing) => (self, Action::Original),
            (Loaded { shown, change, there, .. }, Event::Closing) => {
                let change = match change {
                    Change::Census | Change::Probing { .. } | Change::Loading { .. } => change,
                    _ => Change::Settled,
                };
                (Loaded { shown, view: View::Closed, change, there }, Action::Original)
            }
            (Unasked | Starting(_), Event::Closing) => (self, Action::Original),
            (Loaded { shown, change, there, .. }, Event::Hidden) => {
                let hidden = |change| Loaded { shown, view: View::Hidden, change, there };
                match change {
                    Change::Darkening { .. } | Change::Holding { .. } | Change::Brightening { .. } => {
                        (hidden(Change::Settled), Action::Clear)
                    }
                    Change::Settled | Change::Census | Change::Probing { .. } | Change::Loading { .. } => {
                        (hidden(change), Action::Original)
                    }
                }
            }
            (Unasked | Starting(_), Event::Hidden) => (self, Action::Original),

            (Loaded { shown, change, there, .. }, Event::Shown) => (Loaded { shown, view: View::Drawn, change, there }, Action::Original),
            (Unasked | Starting(_), Event::Shown) => (self, Action::Original),
        }
    }
}

/// The player's pick as user.cfg keeps it (`srp_lobby <id>`): its own
/// lobby, and the start's.
static SAVED: ConsoleString<4> = ConsoleString::new();

/// The squad `0xA1` last gave (none until it comes: a start's lobby is the
/// saved pick's).
static SQUAD: GameCell<Squad> = GameCell::new(Squad::None);

static SCENE: GameCell<Scene> = GameCell::new(Scene::Unasked);

/// What the lobby window was last told (the pick, the lobby shown, the
/// lobbies there), none before the first.
static TOLD: GameCell<Option<[u32; 3]>> = GameCell::new(None);

pub fn on_configs_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    SAVED.register(ctx.thread(), c"srp_lobby");
    Flow::Continue
}

/// The player's own pick.
fn own(thread: &Thread) -> Lobby {
    Lobby::parse(SAVED.get(thread))
}

/// Keeps the player's own pick (user.cfg written at once when it is
/// another than the saved one).
fn save(thread: &Thread, lobby: Lobby) {
    if own(thread) != lobby {
        SAVED.set(thread, lobby.saved());
        save_user_config(thread);
    }
}

/// srp's picks of a `0xA1`: the own one kept (srp has none: the saved one
/// sent, the sign-in's), the commander's the squad's lobby.
fn take_picks(thread: &Thread, picks: Picks, send: impl FnOnce(Lobby)) {
    match Lobby::of_id(picks.own) {
        Some(lobby) => save(thread, lobby),
        None => send(own(thread)),
    }
    SQUAD.set(thread, Lobby::of_id(picks.commander).map_or(Squad::None, Squad::Led));
}

/// `0xA1` with its squad id read: none (0) is out of a squad, its picks
/// after the id (a squad's go on to `on_squad`).
pub fn on_squad_id(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(read) = SquadId::at(ctx)
        && let Some(picks) = read.picks()
    {
        take_picks(ctx.thread(), picks, |lobby| read.send(lobby.pick_start()));
    }
    Flow::Continue
}

/// A squad from the server (`0xA1`, read to its end): its picks.
pub fn on_squad(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(read) = SquadRead::at(ctx) {
        let picks = read.picks().unwrap_or_default();
        take_picks(ctx.thread(), picks, |lobby| read.send(lobby.pick_start()));
    }
    Flow::Continue
}

/// A diagnostics build's change of the inputs (camera_sweep.rs): a pick, or
/// a squad joined or left.
pub fn force(thread: &Thread, own: Option<Lobby>, squad: Option<Squad>) {
    if let Some(lobby) = own {
        save(thread, lobby);
    }
    if let Some(squad) = squad {
        SQUAD.set(thread, squad);
    }
}

/// The lobby wanted now.
fn wanted(thread: &Thread) -> Lobby {
    SQUAD.get(thread).lobby(own(thread))
}

/// One step of the machine, the scene kept (before the site acts: a query
/// it asks may come back at once).
fn step(thread: &Thread, event: Event) -> Action {
    let (scene, action) = SCENE.get(thread).step(event);
    SCENE.set(thread, scene);
    if !matches!(action, Action::Original | Action::Shade { .. }) {
        diagnose(thread, action);
    }
    action
}

/// The diagnostics build's log of the scene's actions (srp_lobby_scene.txt,
/// one a line with the lobby's frame count and the clock's milliseconds;
/// never a release's).
fn diagnose(thread: &Thread, action: Action) {
    let name: &[u8] = match action {
        Action::Original => b"original",
        Action::Ask(_) => b"ask",
        Action::AskAlone { .. } => b"ask alone",
        Action::AskCensus => b"ask census",
        Action::AskAgain => b"ask again",
        Action::LetGo => b"let go",
        Action::Query => b"query",
        Action::Shade { .. } => b"shade",
        Action::ShadeAndQuery => b"black, query",
        Action::SwapIn => b"swap in",
        Action::Reset => b"reset, fade in",
        Action::Clear => b"clear",
        Action::SwapAndClear => b"swap in, clear",
    };
    log_line(thread, name);
}

fn log_line(thread: &Thread, name: &[u8]) {
    static LOG: GameCell<Option<TraceFile>> = GameCell::new(None);
    if !trace::enabled() {
        return;
    }
    let file = match LOG.get(thread) {
        Some(file) => file,
        None => {
            let Some(file) = TraceFile::open(thread, "srp_lobby_scene.txt") else { return };
            LOG.set(thread, Some(file));
            file
        }
    };
    let mut line = [0u8; 64];
    let mut n = decimal(&mut line, 0, FRAMES.get(thread));
    line[n] = b' ';
    n = decimal(&mut line, n + 1, clock::milliseconds(thread));
    line[n] = b' ';
    n += 1;
    let name = &name[..name.len().min(line.len() - n - 1)];
    line[n..n + name.len()].copy_from_slice(name);
    n += name.len();
    line[n] = b'\n';
    file.write(thread, &line[..n + 1]);
}

/// A line of the diagnostics' log after a step's work (its time).
fn note(thread: &Thread, what: &[u8]) {
    log_line(thread, what);
}

/// `v` in decimal at `out[n..]`; the new `n`.
fn decimal(out: &mut [u8; 64], n: usize, v: u32) -> usize {
    let mut digits = [0u8; 10];
    let (mut k, mut v) = (0, v);
    loop {
        digits[k] = b'0' + (v % 10) as u8;
        k += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let mut n = n;
    while k > 0 {
        k -= 1;
        out[n] = digits[k];
        n += 1;
    }
    n
}

/// The lobby's frames so far (the diagnostics' clock).
static FRAMES: GameCell<u32> = GameCell::new(0);

pub fn on_scene_request(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let action = step(thread, Event::Query { wanted: wanted(thread) });
    if let Some(mut query) = SceneQuery::at(ctx) {
        match action {
            Action::Ask(request) => query.ask(request),
            Action::AskAlone { request, filler } => query.ask_alone(request, filler.config()),
            Action::AskCensus => query.ask_configs(Lobby::ALL.map(Lobby::config), Lobby::Classic.config()),
            // the query's own list; the other actions are not a query's
            Action::Original
            | Action::AskAgain
            | Action::LetGo
            | Action::Query
            | Action::Shade { .. }
            | Action::ShadeAndQuery
            | Action::SwapIn
            | Action::Reset
            | Action::Clear
            | Action::SwapAndClear => {}
        }
    }
    Flow::Continue
}

pub fn on_scenes_ready(ctx: &mut Context<'_>) -> Flow<LobbySceneOnScenesReadyExit> {
    let Some(ready) = ScenesReady::at(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let scene = SCENE.get(thread);
    let loaded = scene.asked().is_some_and(|asked| ready.loaded(asked));
    let found = match scene {
        Scene::Loaded { change: Change::Census, .. } => Lobbies::of(|lobby| ready.config_at(lobby.id() as usize - 1)),
        _ => Lobbies::ALL,
    };
    match step(thread, Event::Ready { loaded, found, wanted: wanted(thread) }) {
        Action::AskAgain => ready.ask_again(),
        Action::SwapIn => {
            ready.swap_in();
            ready.reset_eye_adaptation();
            note(thread, b"swapped");
        }
        Action::SwapAndClear => {
            ready.swap_in();
            ready.shade(Shade::Gone);
        }
        Action::Reset => {
            ready.reset_eye_adaptation();
            ready.shade(Shade::Black { loading: false });
        }
        Action::LetGo => {}
        // the lobby menu takes them; the other actions are not a result's
        Action::Original
        | Action::Query
        | Action::Ask(_)
        | Action::AskAlone { .. }
        | Action::AskCensus
        | Action::Shade { .. }
        | Action::ShadeAndQuery
        | Action::Clear => return Flow::Continue,
    }
    Flow::Exit(LobbySceneOnScenesReadyExit::Done)
}

/// A frame of the lobby drawn: a change asked, its fades stepped, the
/// lobby window told what it shows.
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some((mut camera, dt)) = LobbyCamera::at(ctx) else { return Flow::Continue };
    FRAMES.update(thread, |n| n.wrapping_add(1));
    // the first frame of a lobby put in behind the black: its stock view
    if matches!(SCENE.get(thread), Scene::Loaded { change: Change::Holding { frames: HOLD_FRAMES }, .. }) {
        camera.to_stock_pose();
    }
    let action = step(thread, Event::Tick { dt, wanted: wanted(thread) });
    let Some(lobby) = DrawnLobby::of(&camera) else { return Flow::Continue };
    match action {
        Action::Query => lobby.ask(),
        Action::Shade { per_mille, loading } => lobby.shade(Shade::At { per_mille, loading }),
        Action::ShadeAndQuery => {
            lobby.shade(Shade::Black { loading: true });
            lobby.ask();
        }
        Action::Reset => {
            lobby.reset_eye_adaptation();
            lobby.shade(Shade::Black { loading: false });
        }
        Action::Clear => lobby.shade(Shade::Gone),
        // nothing to do this frame; the other actions are not a tick's
        Action::Original
        | Action::Ask(_)
        | Action::AskAlone { .. }
        | Action::AskCensus
        | Action::AskAgain
        | Action::LetGo
        | Action::SwapIn
        | Action::SwapAndClear => {}
    }
    if let Scene::Loaded { shown, there, .. } = SCENE.get(thread) {
        let there = there.unwrap_or(Lobbies::ALL);
        let state = [u32::from(own(thread).id()), u32::from(shown.id()), there.bits()];
        if TOLD.get(thread) != Some(state) {
            TOLD.set(thread, Some(state));
            lobby.tell(state);
        }
    }
    Flow::Continue
}

/// A match entered: a fade gone (a project that comes is put in at once).
pub fn on_deactivate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let action = step(ctx.thread(), Event::Hidden);
    if action == Action::Clear
        && let Some(lobby) = HiddenLobby::at(ctx)
    {
        lobby.shade(Shade::Gone);
    }
    Flow::Continue
}

/// The lobby shown again.
pub fn on_activate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    step(ctx.thread(), Event::Shown);
    Flow::Continue
}

/// The game closes: results from now on let go without the lobby menu.
pub fn on_game_closing(ctx: &mut Context<'_>) -> Flow<NoExit> {
    step(ctx.thread(), Event::Closing);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pick_is_saved_as_its_id_and_read_back_and_none_is_the_room() {
        for lobby in Lobby::ALL {
            assert_eq!(Lobby::parse(lobby.saved()), lobby);
            assert_eq!(Lobby::of_id(lobby.id()), Some(lobby));
            let project = lobby.project().to_str().unwrap();
            let id = project.strip_prefix("lobby_scene/").unwrap();
            assert_eq!(lobby.config().to_str().unwrap(), format!("resources/projects/lobby_scene/{id}/client_project"));
        }
        assert_eq!(Lobby::ALL.map(Lobby::id), [1, 2, 3, 4]);
        for other in [&b""[..], b"0", b"5", b"12", b" 1", b"a"] {
            assert_eq!(Lobby::parse(other), Lobby::Camp069, "{other:?}");
        }
        assert_eq!(Lobby::of_id(0), None);
    }

    #[test]
    fn the_squad_commanders_pick_is_shown_in_a_squad() {
        assert_eq!(Squad::None.lobby(Lobby::Classic), Lobby::Classic);
        assert_eq!(Squad::Led(Lobby::Stk2Bunker).lobby(Lobby::Classic), Lobby::Stk2Bunker);
        let there = Lobbies::of(|lobby| lobby != Lobby::Hangar046);
        assert!(there.has(Lobby::Camp069) && !there.has(Lobby::Hangar046));
        assert_eq!(there.bits(), 0b0111);
        assert!(!Lobbies::ALL.without(Lobby::Classic).has(Lobby::Classic));
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
            let (next, action) = next.step(Event::Ready { loaded: loads(request), found: Lobbies::ALL, wanted: chosen });
            scene = next;
            match action {
                Action::AskAgain => continue,
                Action::Original => {
                    let Scene::Loaded { shown, view: View::Drawn, change: Change::Settled, there: None } = scene else {
                        panic!("{scene:?}")
                    };
                    return (asked, shown);
                }
                other => panic!("a start's results: {other:?}"),
            }
        }
    }

    #[test]
    fn the_start_asks_the_saved_pick() {
        assert_eq!(run(Lobby::Classic, |_| true), (std::vec![SceneRequest::Project(c"lobby_scene/day")], Lobby::Classic));
        assert_eq!(
            run(Lobby::Hangar046, |_| true),
            (std::vec![SceneRequest::Config(Lobby::Hangar046.config()), SceneRequest::Project(c"lobby_scene/hangar_046")], Lobby::Hangar046)
        );
        let (asked, shown) = run(Lobby::Camp069, |request| !matches!(request, SceneRequest::Config(_)));
        assert_eq!(asked, [SceneRequest::Config(Lobby::Camp069.config()), SceneRequest::Project(c"lobby_scene/day")]);
        assert_eq!(shown, Lobby::Classic, "a missing pack: the classic, its project never asked");
        let (asked, shown) = run(Lobby::Stk2Bunker, |request| request != SceneRequest::Project(Lobby::Stk2Bunker.project()));
        assert_eq!(asked.len(), 3);
        assert_eq!(shown, Lobby::Classic, "a project without its track: the classic");
        assert_eq!(run(Lobby::Classic, |_| false).1, Lobby::Classic, "the classic failing is the stock game");
    }

    const FRAME: f32 = 1.0 / 60.0;

    /// A game after its start: the scene and each event's action, every
    /// query and result played as the game plays them (`loads` says whether
    /// what was asked is there), and what the screen shows.
    struct Game {
        scene: Scene,
        wanted: Lobby,
        loads: fn(SceneRequest) -> bool,
        /// A query asked and its results not yet in.
        pending: bool,
        /// The lobby the lobby menu has.
        menu: Lobby,
        /// The black's per mille.
        black: u32,
        actions: std::vec::Vec<Action>,
    }

    impl Game {
        /// The start done and the census in: `missing` not there.
        fn new(shown: Lobby, loads: fn(SceneRequest) -> bool) -> Game {
            let mut game = Game {
                scene: Scene::Loaded { shown, view: View::Drawn, change: Change::Settled, there: None },
                wanted: shown,
                loads,
                pending: false,
                menu: shown,
                black: 0,
                actions: std::vec![],
            };
            game.tick();
            assert_eq!(game.actions, [Action::Query, Action::AskCensus], "the first frame's census");
            game.results();
            game.actions.clear();
            game
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
            match action {
                Action::Shade { per_mille, .. } => self.black = per_mille,
                Action::ShadeAndQuery | Action::Reset => self.black = 1000,
                Action::Clear => self.black = 0,
                Action::SwapIn => {
                    assert!(self.black == 1000 || self.view() != View::Drawn, "a swap behind the black");
                    let Scene::Loaded { change: Change::Loading { to, .. }, .. } = before else { panic!("{before:?}") };
                    self.menu = to;
                }
                Action::SwapAndClear => {
                    assert_ne!(self.view(), View::Drawn, "a swap at once only while not drawn");
                    let Scene::Loaded { change: Change::Loading { to, .. }, .. } = before else { panic!("{before:?}") };
                    self.menu = to;
                    self.black = 0;
                }
                Action::Query => assert_eq!(self.view(), View::Drawn, "a query only while drawn"),
                Action::Original | Action::Ask(_) | Action::AskAlone { .. } | Action::AskCensus | Action::AskAgain | Action::LetGo => {}
            }
            if matches!(action, Action::Query | Action::ShadeAndQuery) {
                assert!(!self.pending, "one query at a time");
                self.query();
            }
            if action == Action::AskAgain {
                self.query();
            }
            if let Scene::Loaded { shown, view, .. } = self.scene
                && view != View::Closed
            {
                assert_eq!(shown, self.menu, "the scene's lobby is the menu's");
            }
            action
        }

        /// The lobby's query built.
        fn query(&mut self) {
            match self.event(Event::Query { wanted: self.wanted }) {
                Action::AskAlone { .. } | Action::AskCensus => self.pending = true,
                Action::Original => {}
                other => panic!("a change's query: {other:?}"),
            }
        }

        fn want(&mut self, lobby: Lobby) {
            self.wanted = lobby;
        }

        fn tick(&mut self) {
            self.event(Event::Tick { dt: FRAME, wanted: self.wanted });
        }

        fn ticks(&mut self, n: usize) {
            for _ in 0..n {
                self.tick();
            }
        }

        /// The pending query's results come; each re-ask's too.
        fn results(&mut self) {
            while self.pending {
                self.pending = false;
                let asked = self.scene.asked();
                let loaded = asked.is_some_and(self.loads);
                let found = Lobbies::of(|lobby| (self.loads)(SceneRequest::Config(lobby.config())));
                self.event(Event::Ready { loaded, found, wanted: self.wanted });
            }
        }

        fn shown(&self) -> Lobby {
            let Scene::Loaded { shown, .. } = self.scene else { panic!("{:?}", self.scene) };
            shown
        }

        /// Nothing more to do: no change out, no black, the wanted lobby shown
        /// unless it is not there.
        fn settled(&self) -> bool {
            let Scene::Loaded { change: Change::Settled, shown, there, .. } = self.scene else { return false };
            self.black == 0 && !self.pending && (shown == self.wanted || there.is_some_and(|t| !t.has(self.wanted)))
        }
    }

    /// Ticks (and results) until the change is done, at most `frames`.
    fn settle(game: &mut Game, frames: usize) -> usize {
        for frame in 0..frames {
            if game.settled() {
                return frame;
            }
            game.tick();
            game.results();
        }
        frames
    }

    #[test]
    fn a_change_probes_then_fades_loads_behind_the_black_swaps_and_fades_in() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Stk2Bunker);
        game.tick();
        assert_eq!(game.actions, [
            Action::Query,
            Action::AskAlone { request: SceneRequest::Config(Lobby::Stk2Bunker.config()), filler: Lobby::Stk2Bunker },
        ]);
        game.ticks(3);
        assert_eq!(game.black, 0, "the old lobby drawn while its config is probed");
        game.results();
        let mut frames = 0;
        while !game.pending {
            game.tick();
            frames += 1;
            assert!(game.black > 0 && frames <= 16, "the fade to black, then its project asked");
        }
        assert_eq!(game.black, 1000);
        assert!(game.actions.contains(&Action::ShadeAndQuery));
        assert!((14..=16).contains(&frames), "{frames} frames of fade (0.25 s)");
        assert_eq!(game.menu, Lobby::Classic);
        game.ticks(20);
        assert_eq!(game.black, 1000, "black while its project loads");
        game.results();
        assert_eq!(game.menu, Lobby::Stk2Bunker);
        assert_eq!(*game.actions.last().unwrap(), Action::SwapIn);
        game.ticks(usize::from(HOLD_FRAMES) - 1);
        assert_eq!(game.black, 1000, "the new lobby's first frames behind the black");
        game.tick();
        assert_eq!(*game.actions.last().unwrap(), Action::Reset);
        let frames = settle(&mut game, 60);
        assert!((22..=26).contains(&frames), "{frames} frames of fade in (0.4 s)");
        assert_eq!(game.shown(), Lobby::Stk2Bunker);
    }

    #[test]
    fn a_stall_does_not_skip_a_fade() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.tick();
        game.results();
        let before = game.actions.len();
        game.event(Event::Tick { dt: 3.0, wanted: Lobby::Hangar046 });
        assert!(game.black > 0 && game.black < 200, "a 3 s frame steps the fade by a 30th of a second: {}", game.black);
        assert!(!game.actions[before..].contains(&Action::ShadeAndQuery));
    }

    #[test]
    fn a_missing_pack_keeps_the_lobby_with_no_fade() {
        // known missing by the census: never asked
        let missing = |request| request != SceneRequest::Config(Lobby::Hangar046.config());
        let mut game = Game::new(Lobby::Camp069, missing);
        assert!(matches!(game.scene, Scene::Loaded { there: Some(t), .. } if !t.has(Lobby::Hangar046) && t.has(Lobby::Classic)));
        game.want(Lobby::Hangar046);
        game.ticks(30);
        assert!(game.actions.iter().all(|a| *a == Action::Original), "never asked");
        assert!(game.settled());
        // a project without its track: found out behind the black, faded back, not asked again
        let broken = |request| request != SceneRequest::Project(Lobby::Camp069.project());
        let mut game = Game::new(Lobby::Hangar046, broken);
        game.want(Lobby::Camp069);
        settle(&mut game, 200);
        assert_eq!(game.shown(), Lobby::Hangar046);
        let before = game.actions.len();
        game.ticks(30);
        assert!(game.actions[before..].iter().all(|a| *a == Action::Original), "not asked again");
    }

    #[test]
    fn the_last_lobby_wanted_wins() {
        // while its config is probed
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.tick();
        game.want(Lobby::Camp069);
        game.results();
        settle(&mut game, 300);
        assert_eq!(game.shown(), Lobby::Camp069);
        // back to the one shown while probed: let go, no fade
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.tick();
        game.want(Lobby::Classic);
        game.results();
        assert!(game.settled() && !game.actions.contains(&Action::ShadeAndQuery));
        // while its project loads behind the black
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.tick();
        game.results();
        game.ticks(20);
        game.want(Lobby::Stk2Bunker);
        settle(&mut game, 300);
        assert_eq!(game.shown(), Lobby::Stk2Bunker);
        assert_eq!(game.actions.iter().filter(|a| **a == Action::SwapIn).count(), 1, "one swap: the last one wanted");
    }

    #[test]
    fn a_match_entered_mid_change_leaves_no_black_and_the_return_shows_the_wanted_lobby() {
        for at in 0..70 {
            let mut game = Game::new(Lobby::Classic, |_| true);
            game.want(Lobby::Hangar046);
            for _ in 0..at {
                game.tick();
                if at % 3 == 0 {
                    game.results();
                }
            }
            game.event(Event::Hidden);
            game.results();
            assert_eq!(game.black, 0, "{at}: no black left");
            game.event(Event::Shown);
            settle(&mut game, 400);
            assert_eq!(game.shown(), Lobby::Hangar046, "{at}");
            assert_eq!(game.black, 0, "{at}");
        }
    }

    #[test]
    fn closing_lets_results_go() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.tick();
        game.event(Event::Closing);
        game.results();
        assert_eq!(*game.actions.last().unwrap(), Action::LetGo, "results after the close: let go");
        assert!(matches!(game.scene, Scene::Loaded { view: View::Closed, change: Change::Settled, .. }));
        game.tick();
        game.event(Event::Hidden);
        assert_eq!(game.actions[game.actions.len() - 2..], [Action::Original, Action::Original]);
    }

    #[test]
    fn before_the_scene_nothing_changes() {
        for scene in [Scene::Unasked, Scene::Starting(Ask::Config(Lobby::Camp069))] {
            assert_eq!(scene.step(Event::Hidden), (scene, Action::Original));
            assert_eq!(scene.step(Event::Shown), (scene, Action::Original));
            assert_eq!(scene.step(Event::Closing), (scene, Action::Original));
            assert_eq!(scene.step(Event::Tick { dt: FRAME, wanted: Lobby::Hangar046 }), (scene, Action::Original));
        }
    }

    /// Every sequence of events (up to a length) from a lobby shown: a swap
    /// only behind the black or while not drawn, a query only while drawn,
    /// one query at a time (Game::event), no black left once settled, and the
    /// lobby shown one that was wanted.
    #[test]
    fn no_sequence_swaps_in_sight_or_leaves_the_black() {
        const LOADS: [fn(SceneRequest) -> bool; 3] = [|_| true, |r| matches!(r, SceneRequest::Project(_)), |r| {
            r != SceneRequest::Config(Lobby::Hangar046.config())
        }];
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
                        2 => {
                            game.event(Event::Hidden);
                        }
                        3 => {
                            game.event(Event::Shown);
                        }
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
                    game.event(Event::Shown);
                }
                if game.view() == View::Drawn {
                    settle(&mut game, 600);
                    assert_eq!(game.black, 0, "{code}: {:?}", game.scene);
                }
                assert!(wanted.contains(&game.shown()), "{code}: {:?}", game.scene);
            }
        }
    }
}
