//! Patch 81: the lobby's scene is the one the player picks in the lobby
//! window (docs/notes/lobby-scenes.md): the Room (the 0.69d camp, the
//! default), the Basement (the classic), the Bunker (S.T.A.L.K.E.R. 2), the
//! Hangar (0.46c2), the House or the Shooting Range (S.T.A.L.K.E.R. 2). In a
//! squad the lobby is its commander's pick. The six lobbies ship with the game: a project that fails to load is a bug,
//! logged, never a state of the game's.
//!
//! The pick is kept in user.cfg as the console variable `srp_lobby`
//! (registered before user.cfg runs, `configs.start`, after patches 79's and 83's
//! handlers), and the start shows it: its project takes the classic's place
//! in the scene's query (`lobby_menu.scene_request`).
//!
//! The lobby window sends a pick to srp itself (the movie's `3B` at
//! `2 095 000 000 + id`); srp sends it back in `0xA1` with the squad
//! commander's (two bytes after the squad, `lobby_client.squad_id` and
//! `lobby_client.squad_read`), 0 for none: at each sign-in srp has none,
//! and the exe sends it the saved pick. The lobby wanted is the
//! commander's pick in a squad, the player's own out of one.
//!
//! A change shows at once: the lobby's tick (`lobby_camera.view`) steps the
//! scene to black (the lobby movie's `srp_scene_fade`, under every panel,
//! the loading spinner on it), the lobby's project is asked alone and put
//! in the old one's place behind the black, the eye adaptation is reset and
//! the camera put at its stock pose, a few frames are drawn behind the
//! black, and the scene is stepped back from black. A match entered
//! (`lobby_menu.deactivate`) puts a project that comes in at once; results
//! once the game closes (`game.clear_resources`) are let go. One enum,
//! [`Scene`], and one transition function, [`Scene::step`].
//!
//! While patch 45's camera is the table's (a weapon's upgrade window open,
//! or its camera flying there or back) the lobby wanted is the one shown
//! ([`Scene::wanted`]): patch 45 read the weapon's spot from the lobby
//! shown's project when its table turned on, so a project put in under it
//! would leave the weapon and the camera at the old lobby's spot in the new
//! scene. A pick, or a squad's, made meanwhile plays once the window is
//! closed and the camera home: a fade begun steps back out, a project that
//! comes behind the black is let go, and the change starts again then.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::clock;
use crate::vostok::language::ConsoleString;
use crate::vostok::settings::save_user_config;
use crate::vostok::lobby_menu::{DrawnLobby, HiddenLobby, Picks, SceneQuery, ScenesReady, Shade, SquadId, SquadRead};
use crate::vostok::lobby_camera::{LobbyCamera, TableCamera, WeaponTableState};
use crate::vostok::log::{self, Piece::Text};
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
    /// The House: the upper floor of the S.T.A.L.K.E.R. 2 (2012) Bar.
    Stk2House,
    /// The Shooting Range: the S.T.A.L.K.E.R. 2 (2012) bar's shooting gallery.
    Stk2Bar,
}

/// srp's start of a lobby pick (`3B`; its `lobby_pick::LOBBY_PICK_BASE`,
/// 2 095 000 000): a ranking start, not an address, so made of small
/// numbers for the layers' check of literals.
pub const PICK_BASE: u32 = 2095 * 1000 * 1000;

impl Lobby {
    /// In the window's order (and the ids' 1 to 6).
    pub const ALL: [Lobby; 6] =
        [Lobby::Camp069, Lobby::Classic, Lobby::Stk2Bunker, Lobby::Hangar046, Lobby::Stk2House, Lobby::Stk2Bar];

    /// Its id: user.cfg's `srp_lobby`, the lobby window's card, srp's
    /// `3B` start (`2 095 000 000 + id`) and `0xA1`'s bytes (0 none).
    pub fn id(self) -> u8 {
        match self {
            Lobby::Camp069 => 1,
            Lobby::Classic => 2,
            Lobby::Stk2Bunker => 3,
            Lobby::Hangar046 => 4,
            Lobby::Stk2House => 5,
            Lobby::Stk2Bar => 6,
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

    /// Its value as user.cfg keeps it (one decimal digit).
    pub fn saved(self) -> &'static [u8] {
        match self {
            Lobby::Camp069 => b"1",
            Lobby::Classic => b"2",
            Lobby::Stk2Bunker => b"3",
            Lobby::Hangar046 => b"4",
            Lobby::Stk2House => b"5",
            Lobby::Stk2Bar => b"6",
        }
    }

    /// srp's `3B` start of its pick (the server's `lobby_pick::LOBBY_PICK_BASE`
    /// and its id).
    pub fn pick_start(self) -> u32 {
        PICK_BASE + u32::from(self.id())
    }

    /// Its project, as the lobby's query names it.
    pub fn project(self) -> &'static CStr {
        match self {
            Lobby::Classic => c"lobby_scene/day",
            Lobby::Stk2Bunker => c"lobby_scene/stk2_bunker",
            Lobby::Hangar046 => c"lobby_scene/hangar_046",
            Lobby::Camp069 => c"lobby_scene/camp_069",
            Lobby::Stk2House => c"lobby_scene/stk2_house",
            Lobby::Stk2Bar => c"lobby_scene/stk2_bar",
        }
    }

    /// Its project's config: what a change's query asks in every other
    /// place (a binary config, so no second render scene, sound scene or
    /// movie is made).
    pub fn config(self) -> &'static CStr {
        match self {
            Lobby::Classic => c"resources/projects/lobby_scene/day/client_project",
            Lobby::Stk2Bunker => c"resources/projects/lobby_scene/stk2_bunker/client_project",
            Lobby::Hangar046 => c"resources/projects/lobby_scene/hangar_046/client_project",
            Lobby::Camp069 => c"resources/projects/lobby_scene/camp_069/client_project",
            Lobby::Stk2House => c"resources/projects/lobby_scene/stk2_house/client_project",
            Lobby::Stk2Bar => c"resources/projects/lobby_scene/stk2_bar/client_project",
        }
    }

    /// Its name in the log.
    pub fn name(self) -> &'static str {
        match self {
            Lobby::Camp069 => "camp_069",
            Lobby::Classic => "day",
            Lobby::Stk2Bunker => "stk2_bunker",
            Lobby::Hangar046 => "hangar_046",
            Lobby::Stk2House => "stk2_house",
            Lobby::Stk2Bar => "stk2_bar",
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
    /// The lobby shown is the one wanted.
    Settled,
    /// `lobby`'s project failed to load (a bug, logged): not asked again
    /// until another lobby is wanted (else its fade would play every
    /// second).
    Broken { lobby: Lobby },
    /// The scene stepping to black, `black` of the way (0 to 1).
    Darkening { black: f32 },
    /// Behind the black, `to`'s project asked.
    Loading { to: Lobby },
    /// The new lobby in, drawn behind the black `frames` more frames.
    Holding { frames: u8 },
    /// The scene stepping from black, `black` of the way left; `broken` the
    /// lobby whose project failed behind it (then `Broken`).
    Brightening { black: f32, broken: Option<Lobby> },
}

/// Where the lobby's scene is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scene {
    /// The lobby menu has not asked for it.
    Unasked,
    /// The start: the lobby menu's own query asks this lobby's project.
    Starting(Lobby),
    /// The lobby menu has the lobby `shown`.
    Loaded { shown: Lobby, view: View, change: Change },
}

/// What happens to the lobby's scene. `wanted` is the lobby wanted at that
/// moment ([`Scene::wanted`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// The lobby menu builds its query (`lobby_menu.scene_request`).
    Query { wanted: Lobby },
    /// Its results are in (`lobby_menu.scenes_ready`); `loaded`: the
    /// project's place holds a project with its camera track
    /// (ScenesReady::loaded).
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

/// What follows a project that failed to load (logged).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterBroken {
    /// The start's: the lobby menu takes the results as they are.
    Take,
    /// Behind the black: the black stepped out over the lobby shown.
    FadeIn,
    /// While the lobby is not drawn: the results let go.
    LetGo,
}

/// What the site does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// The site's own code as it is: the query's own list, the results
    /// taken by the lobby menu, the menu's (de)activation.
    Original,
    /// The start's query: this lobby's project in the project's place.
    Ask(Lobby),
    /// A change's query: this lobby's project in the project's place, its
    /// config in every other.
    AskAlone(Lobby),
    /// The results let go, the query asked again.
    AskAgain,
    /// The results let go.
    LetGo,
    /// The black at `per_mille` of its way, the loading spinner on it or not.
    Shade { per_mille: u32, loading: bool },
    /// The black whole and the spinner on it, the lobby's query asked.
    ShadeAndQuery,
    /// The results' project put in the lobby menu's (behind the black), the
    /// eye adaptation reset.
    SwapIn,
    /// The results' project put in and the black gone at once (a match
    /// entered).
    SwapAndClear,
    /// The black whole, the spinner off, the eye adaptation reset again.
    Reset,
    /// The black gone at once.
    Clear,
    /// `lobby`'s project failed to load: logged, then `then`.
    Broken { lobby: Lobby, then: AfterBroken },
}

/// The black's per mille of `black` (0 to 1).
fn per_mille(black: f32) -> u32 {
    (black.clamp(0.0, 1.0) * 1000.0) as u32
}

impl Scene {
    /// The lobby wanted at an event: `picked` ([`Squad::lobby`] of the saved
    /// pick and the squad), but the lobby shown while patch 45's camera is
    /// the table's (its spot is the shown lobby's project's).
    pub fn wanted(self, picked: Lobby, camera: TableCamera) -> Lobby {
        match (self, camera) {
            (Scene::Loaded { shown, .. }, TableCamera::Table) => shown,
            _ => picked,
        }
    }

    /// The machine: the next scene and what the site does, for every scene
    /// and every event. A project is put in only behind the black or while
    /// the lobby is not drawn; one change at a time, the last wanted lobby
    /// the next.
    pub fn step(self, event: Event) -> (Scene, Action) {
        use Scene::{Loaded, Starting, Unasked};
        match (self, event) {
            (Unasked, Event::Query { wanted }) => (Starting(wanted), Action::Ask(wanted)),
            (Starting(lobby), Event::Query { .. }) => (self, Action::Ask(lobby)),
            (Loaded { change: Change::Loading { to }, .. }, Event::Query { .. }) => (self, Action::AskAlone(to)),
            (Loaded { .. }, Event::Query { .. }) => (self, Action::Original),

            (Unasked, Event::Ready { .. }) => (self, Action::Original),
            (Starting(lobby), Event::Ready { loaded, .. }) => {
                let action = if loaded { Action::Original } else { Action::Broken { lobby, then: AfterBroken::Take } };
                (Loaded { shown: lobby, view: View::Drawn, change: Change::Settled }, action)
            }
            (Loaded { shown, view, change: Change::Loading { to } }, Event::Ready { loaded, wanted }) => {
                let at = |change| Loaded { shown, view, change };
                match view {
                    View::Closed => (at(Change::Settled), Action::LetGo),
                    _ if to != wanted && wanted == shown => match view {
                        View::Drawn => (at(Change::Brightening { black: 1.0, broken: None }), Action::Reset),
                        View::Hidden | View::Closed => (at(Change::Settled), Action::LetGo),
                    },
                    _ if to != wanted => (at(Change::Loading { to: wanted }), Action::AskAgain),
                    View::Drawn if loaded => {
                        (Loaded { shown: to, view, change: Change::Holding { frames: HOLD_FRAMES } }, Action::SwapIn)
                    }
                    View::Hidden if loaded => (Loaded { shown: to, view, change: Change::Settled }, Action::SwapAndClear),
                    View::Drawn => (
                        at(Change::Brightening { black: 1.0, broken: Some(to) }),
                        Action::Broken { lobby: to, then: AfterBroken::FadeIn },
                    ),
                    View::Hidden => {
                        (at(Change::Broken { lobby: to }), Action::Broken { lobby: to, then: AfterBroken::LetGo })
                    }
                }
            }
            (Loaded { .. }, Event::Ready { .. }) => (self, Action::Original),

            (Loaded { shown, view: View::Drawn, change }, Event::Tick { dt, wanted }) => {
                let at = |change| Loaded { shown, view: View::Drawn, change };
                let step = dt.min(FADE_STEP);
                match change {
                    Change::Settled if wanted != shown => (at(Change::Darkening { black: 0.0 }), Action::Original),
                    Change::Broken { lobby } if wanted != shown && wanted != lobby => {
                        (at(Change::Darkening { black: 0.0 }), Action::Original)
                    }
                    Change::Broken { .. } if wanted == shown => (at(Change::Settled), Action::Original),
                    Change::Settled | Change::Broken { .. } | Change::Loading { .. } => (self, Action::Original),
                    Change::Darkening { black } if black + step / FADE_OUT < 1.0 => {
                        let black = black + step / FADE_OUT;
                        (at(Change::Darkening { black }), Action::Shade { per_mille: per_mille(black), loading: false })
                    }
                    Change::Darkening { .. } if wanted == shown => (at(Change::Brightening { black: 1.0, broken: None }), Action::Reset),
                    Change::Darkening { .. } => (at(Change::Loading { to: wanted }), Action::ShadeAndQuery),
                    Change::Holding { frames } if frames > 1 => (at(Change::Holding { frames: frames - 1 }), Action::Original),
                    Change::Holding { .. } => (at(Change::Brightening { black: 1.0, broken: None }), Action::Reset),
                    Change::Brightening { black, broken } if black - step / FADE_IN > 0.0 => {
                        let black = black - step / FADE_IN;
                        (at(Change::Brightening { black, broken }), Action::Shade { per_mille: per_mille(black), loading: false })
                    }
                    Change::Brightening { broken: None, .. } => (at(Change::Settled), Action::Clear),
                    Change::Brightening { broken: Some(lobby), .. } => (at(Change::Broken { lobby }), Action::Clear),
                }
            }
            (Unasked | Starting(_) | Loaded { .. }, Event::Tick { .. }) => (self, Action::Original),

            (Loaded { view: View::Closed, .. }, Event::Hidden | Event::Shown | Event::Closing) => (self, Action::Original),
            (Loaded { shown, change, .. }, Event::Closing) => {
                let change = if let Change::Loading { .. } = change { change } else { Change::Settled };
                (Loaded { shown, view: View::Closed, change }, Action::Original)
            }
            (Unasked | Starting(_), Event::Closing) => (self, Action::Original),
            (Loaded { shown, change, .. }, Event::Hidden) => {
                let hidden = |change| Loaded { shown, view: View::Hidden, change };
                match change {
                    Change::Darkening { .. } | Change::Holding { .. } | Change::Brightening { broken: None, .. } => {
                        (hidden(Change::Settled), Action::Clear)
                    }
                    Change::Brightening { broken: Some(lobby), .. } => (hidden(Change::Broken { lobby }), Action::Clear),
                    Change::Loading { .. } => (hidden(change), Action::Clear),
                    Change::Settled | Change::Broken { .. } => (hidden(change), Action::Original),
                }
            }
            (Unasked | Starting(_), Event::Hidden) => (self, Action::Original),

            (Loaded { shown, change: change @ Change::Loading { .. }, .. }, Event::Shown) => {
                (Loaded { shown, view: View::Drawn, change }, Action::Shade { per_mille: 1000, loading: true })
            }
            (Loaded { shown, change, .. }, Event::Shown) => (Loaded { shown, view: View::Drawn, change }, Action::Original),
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

/// What the lobby window was last told (the pick, the lobby shown), none
/// before the first.
static TOLD: GameCell<Option<[u32; 2]>> = GameCell::new(None);

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

/// srp's picks of a `0xA1`: its own pick kept (none: srp has none, the
/// sign-in's: the saved one sent), the commander's the squad's lobby.
fn take_picks(thread: &Thread, picks: Picks, send: impl FnOnce(Lobby)) {
    match Lobby::of_id(picks.own) {
        Some(lobby) => save(thread, lobby),
        None => send(own(thread)),
    }
    SQUAD.set(thread, Lobby::of_id(picks.commander).map_or(Squad::None, Squad::Led));
}

/// A `0xA1` without srp's two bytes (a bug of srp's): logged, nothing taken.
fn no_picks() {
    log::line(&[Text("lobby_scene: a squad info (0xA1) without srp's two lobby bytes")]);
}

/// `0xA1` with its squad id read: none (0) is out of a squad, its picks
/// after the id (a squad's go on to `on_squad`).
pub fn on_squad_id(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(read) = SquadId::at(ctx)
        && read.squad_id() == 0
    {
        match read.picks() {
            Some(picks) => take_picks(ctx.thread(), picks, |lobby| read.send(lobby.pick_start())),
            None => no_picks(),
        }
    }
    Flow::Continue
}

/// A squad from the server (`0xA1`, read to its end): its picks.
pub fn on_squad(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(read) = SquadRead::at(ctx) {
        match read.picks() {
            Some(picks) => take_picks(ctx.thread(), picks, |lobby| read.send(lobby.pick_start())),
            None => no_picks(),
        }
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

/// The lobby wanted now ([`Scene::wanted`]).
fn wanted(thread: &Thread) -> Lobby {
    SCENE.get(thread).wanted(SQUAD.get(thread).lobby(own(thread)), WeaponTableState::of(thread).camera())
}

/// One step of the machine, the scene kept (before the site acts: a query
/// it asks may come back at once).
fn step(thread: &Thread, event: Event) -> Action {
    let (scene, action) = SCENE.get(thread).step(event);
    SCENE.set(thread, scene);
    match action {
        Action::Original | Action::Shade { .. } => {}
        Action::Broken { lobby, .. } => {
            log::line(&[Text("lobby_scene: the project lobby_scene/"), Text(lobby.name()), Text(" failed to load")]);
            diagnose(thread, b"broken");
        }
        _ => diagnose(thread, name(action)),
    }
    action
}

/// An action's name in the diagnostics' log.
fn name(action: Action) -> &'static [u8] {
    match action {
        Action::Original => b"original",
        Action::Ask(_) => b"ask",
        Action::AskAlone(_) => b"ask alone",
        Action::AskAgain => b"ask again",
        Action::LetGo => b"let go",
        Action::Shade { .. } => b"shade",
        Action::ShadeAndQuery => b"black, query",
        Action::SwapIn => b"swap in",
        Action::SwapAndClear => b"swap in, clear",
        Action::Reset => b"reset, fade in",
        Action::Clear => b"clear",
        Action::Broken { .. } => b"broken",
    }
}

/// The diagnostics build's log of the scene's actions (srp_lobby_scene.txt,
/// one a line with the lobby's frame count and the clock's milliseconds;
/// never a release's).
fn diagnose(thread: &Thread, name: &[u8]) {
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
            Action::Ask(lobby) => query.ask(lobby.project()),
            Action::AskAlone(lobby) => query.ask_alone(lobby.project(), lobby.config()),
            // the query's own list; the other actions are not a query's
            Action::Original
            | Action::AskAgain
            | Action::LetGo
            | Action::Shade { .. }
            | Action::ShadeAndQuery
            | Action::SwapIn
            | Action::SwapAndClear
            | Action::Reset
            | Action::Clear
            | Action::Broken { .. } => {}
        }
    }
    Flow::Continue
}

pub fn on_scenes_ready(ctx: &mut Context<'_>) -> Flow<LobbySceneOnScenesReadyExit> {
    let Some(ready) = ScenesReady::at(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    match step(thread, Event::Ready { loaded: ready.loaded(), wanted: wanted(thread) }) {
        Action::AskAgain => ready.ask_again(),
        Action::SwapIn => {
            ready.swap_in();
            ready.reset_eye_adaptation();
            diagnose(thread, b"swapped");
        }
        Action::SwapAndClear => {
            ready.swap_in();
            ready.shade(Shade::Gone);
        }
        Action::Reset | Action::Broken { then: AfterBroken::FadeIn, .. } => {
            ready.reset_eye_adaptation();
            ready.shade(Shade::Black { loading: false });
        }
        Action::LetGo | Action::Broken { then: AfterBroken::LetGo, .. } => {}
        // the lobby menu takes them; the other actions are not a result's
        Action::Original
        | Action::Broken { then: AfterBroken::Take, .. }
        | Action::Ask(_)
        | Action::AskAlone(_)
        | Action::Shade { .. }
        | Action::ShadeAndQuery
        | Action::Clear => return Flow::Continue,
    }
    Flow::Exit(LobbySceneOnScenesReadyExit::Done)
}

/// A frame of the lobby drawn: a change's fades stepped and its query
/// asked, the lobby window told what it shows.
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
        | Action::AskAlone(_)
        | Action::AskAgain
        | Action::LetGo
        | Action::SwapIn
        | Action::SwapAndClear
        | Action::Broken { .. } => {}
    }
    if let Scene::Loaded { shown, .. } = SCENE.get(thread) {
        let state = [u32::from(own(thread).id()), u32::from(shown.id())];
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

/// The lobby shown again: the black back over it while a project loads.
pub fn on_activate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Action::Shade { per_mille, loading } = step(ctx.thread(), Event::Shown)
        && let Some(lobby) = HiddenLobby::at_shown(ctx)
    {
        lobby.shade(Shade::At { per_mille, loading });
    }
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
            assert_eq!(lobby.pick_start(), 2_095_000_000 + u32::from(lobby.id()));
            let project = lobby.project().to_str().unwrap();
            let id = project.strip_prefix("lobby_scene/").unwrap();
            assert_eq!(id, lobby.name());
            assert_eq!(lobby.config().to_str().unwrap(), format!("resources/projects/lobby_scene/{id}/client_project"));
        }
        assert_eq!(Lobby::ALL.map(Lobby::id), [1, 2, 3, 4, 5, 6]);
        for other in [&b""[..], b"0", b"7", b"12", b" 1", b"a"] {
            assert_eq!(Lobby::parse(other), Lobby::Camp069, "{other:?}");
        }
        assert_eq!(Lobby::of_id(0), None);
    }

    #[test]
    fn the_squad_commanders_pick_is_shown_in_a_squad() {
        assert_eq!(Squad::None.lobby(Lobby::Classic), Lobby::Classic);
        assert_eq!(Squad::Led(Lobby::Stk2Bunker).lobby(Lobby::Classic), Lobby::Stk2Bunker);
    }

    #[test]
    fn the_start_asks_the_saved_picks_project() {
        let (scene, action) = Scene::Unasked.step(Event::Query { wanted: Lobby::Hangar046 });
        assert_eq!((scene, action), (Scene::Starting(Lobby::Hangar046), Action::Ask(Lobby::Hangar046)));
        let (scene, action) = scene.step(Event::Ready { loaded: true, wanted: Lobby::Hangar046 });
        assert_eq!(action, Action::Original);
        assert_eq!(scene, Scene::Loaded { shown: Lobby::Hangar046, view: View::Drawn, change: Change::Settled });
        assert_eq!(scene.step(Event::Query { wanted: Lobby::Hangar046 }), (scene, Action::Original));
        // a broken project: logged, the lobby menu takes its results as they are
        let (_, action) = Scene::Starting(Lobby::Camp069).step(Event::Ready { loaded: false, wanted: Lobby::Camp069 });
        assert_eq!(action, Action::Broken { lobby: Lobby::Camp069, then: AfterBroken::Take });
    }

    const FRAME: f32 = 1.0 / 60.0;

    /// A game after its start: the scene and each event's action, every
    /// query and result played as the game plays them (`loads` says whether
    /// a lobby's project loads), and what the screen shows.
    struct Game {
        scene: Scene,
        /// The saved pick and the squad's lobby.
        picked: Lobby,
        /// Patch 45's camera.
        camera: TableCamera,
        loads: fn(Lobby) -> bool,
        /// A query asked and its results not yet in, for this lobby.
        pending: Option<Lobby>,
        /// The lobby the lobby menu has.
        menu: Lobby,
        /// The black's per mille.
        black: u32,
        actions: std::vec::Vec<Action>,
    }

    impl Game {
        fn new(shown: Lobby, loads: fn(Lobby) -> bool) -> Game {
            Game {
                scene: Scene::Loaded { shown, view: View::Drawn, change: Change::Settled },
                picked: shown,
                camera: TableCamera::Stock,
                loads,
                pending: None,
                menu: shown,
                black: 0,
                actions: std::vec![],
            }
        }

        fn wanted(&self) -> Lobby {
            self.scene.wanted(self.picked, self.camera)
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
            let to = || match before {
                Scene::Loaded { change: Change::Loading { to }, .. } => to,
                _ => panic!("{before:?}"),
            };
            match action {
                Action::Shade { per_mille, .. } => self.black = per_mille,
                Action::ShadeAndQuery | Action::Reset | Action::Broken { then: AfterBroken::FadeIn, .. } => {
                    self.black = 1000
                }
                Action::Clear => self.black = 0,
                Action::SwapIn => {
                    assert_eq!(self.black, 1000, "a swap behind the black");
                    assert_eq!(self.camera, TableCamera::Stock, "no swap under the table's camera");
                    self.menu = to();
                }
                Action::SwapAndClear => {
                    assert_ne!(self.view(), View::Drawn, "a swap at once only while not drawn");
                    assert_eq!(self.camera, TableCamera::Stock, "no swap under the table's camera");
                    self.menu = to();
                    self.black = 0;
                }
                Action::Original
                | Action::Ask(_)
                | Action::AskAlone(_)
                | Action::AskAgain
                | Action::LetGo
                | Action::Broken { .. } => {}
            }
            if matches!(action, Action::ShadeAndQuery | Action::AskAgain) {
                assert!(self.pending.is_none(), "one query at a time");
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
            match self.event(Event::Query { wanted: self.wanted() }) {
                Action::AskAlone(lobby) => self.pending = Some(lobby),
                other => panic!("a change's query: {other:?}"),
            }
        }

        fn want(&mut self, lobby: Lobby) {
            self.picked = lobby;
        }

        fn tick(&mut self) {
            self.event(Event::Tick { dt: FRAME, wanted: self.wanted() });
        }

        fn ticks(&mut self, n: usize) {
            for _ in 0..n {
                self.tick();
            }
        }

        /// The pending query's results come; each re-ask's too.
        fn results(&mut self) {
            while let Some(lobby) = self.pending.take() {
                let loaded = (self.loads)(lobby);
                self.event(Event::Ready { loaded, wanted: self.wanted() });
            }
        }

        fn shown(&self) -> Lobby {
            let Scene::Loaded { shown, .. } = self.scene else { panic!("{:?}", self.scene) };
            shown
        }

        /// Nothing more to do: no change out, no black, the wanted lobby
        /// shown unless it is broken.
        fn settled(&self) -> bool {
            let Scene::Loaded { change, shown, .. } = self.scene else { return false };
            let done = match change {
                Change::Settled => shown == self.wanted(),
                Change::Broken { lobby } => lobby == self.wanted(),
                _ => false,
            };
            done && self.black == 0 && self.pending.is_none()
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
    fn a_change_fades_loads_behind_the_black_swaps_and_fades_in() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Stk2Bunker);
        let mut frames = 0;
        while game.pending.is_none() {
            game.tick();
            frames += 1;
            assert!(game.black > 0 || frames == 1, "the fade to black from the first frame");
            assert!(frames <= 17);
        }
        assert_eq!(game.black, 1000);
        assert!(game.actions.contains(&Action::ShadeAndQuery) && game.actions.contains(&Action::AskAlone(Lobby::Stk2Bunker)));
        assert!((15..=17).contains(&frames), "{frames} frames of fade (0.25 s)");
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
        game.event(Event::Tick { dt: 3.0, wanted: Lobby::Hangar046 });
        assert!(game.black > 0 && game.black < 200, "a 3 s frame steps the fade by a 30th of a second: {}", game.black);
        assert!(game.pending.is_none());
    }

    #[test]
    fn a_broken_project_is_logged_faded_back_and_not_asked_again() {
        let broken = |lobby| lobby != Lobby::Camp069;
        let mut game = Game::new(Lobby::Hangar046, broken);
        game.want(Lobby::Camp069);
        settle(&mut game, 200);
        assert_eq!(game.shown(), Lobby::Hangar046);
        assert!(game.actions.contains(&Action::Broken { lobby: Lobby::Camp069, then: AfterBroken::FadeIn }));
        assert!(!game.actions.contains(&Action::SwapIn));
        let before = game.actions.len();
        game.ticks(30);
        assert!(game.actions[before..].iter().all(|a| *a == Action::Original), "not asked again");
        game.want(Lobby::Classic);
        settle(&mut game, 200);
        assert_eq!(game.shown(), Lobby::Classic, "another lobby wanted: asked");
    }

    #[test]
    fn the_last_lobby_wanted_wins() {
        // changed during the fade: the new one asked at the black
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.ticks(5);
        game.want(Lobby::Camp069);
        settle(&mut game, 300);
        assert_eq!(game.shown(), Lobby::Camp069);
        assert_eq!(game.actions.iter().filter(|a| **a == Action::SwapIn).count(), 1);
        // back to the one shown during the fade: faded back in, nothing asked
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.ticks(5);
        game.want(Lobby::Classic);
        settle(&mut game, 300);
        assert!(!game.actions.iter().any(|a| matches!(a, Action::AskAlone(_) | Action::SwapIn)));
        // while its project loads behind the black
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.ticks(20);
        game.want(Lobby::Stk2Bunker);
        settle(&mut game, 300);
        assert_eq!(game.shown(), Lobby::Stk2Bunker);
        assert_eq!(game.actions.iter().filter(|a| **a == Action::SwapIn).count(), 1, "one swap: the last one wanted");
    }

    /// A pick while a weapon's upgrade window has patch 45's camera (its
    /// spot the lobby shown's): no project put in until the camera is the
    /// stock one again, then the change.
    #[test]
    fn a_change_waits_for_the_upgrade_windows_camera() {
        let swaps = |game: &Game| game.actions.iter().filter(|a| **a == Action::SwapIn).count();
        let then_the_change = |mut game: Game| {
            assert_eq!((game.menu, game.black, game.pending), (Lobby::Classic, 0, None), "{:?}", game.scene);
            assert_eq!(swaps(&game), 0);
            game.camera = TableCamera::Stock;
            settle(&mut game, 300);
            assert_eq!(game.shown(), Lobby::Stk2Bunker);
            assert_eq!(swaps(&game), 1);
        };
        // picked with the window open: nothing at all while it is
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.camera = TableCamera::Table;
        game.want(Lobby::Stk2Bunker);
        game.ticks(60);
        assert!(game.actions.iter().all(|a| *a == Action::Original), "{:?}", game.actions);
        then_the_change(game);
        // opened during the fade to black: the black stepped back out, nothing asked
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Stk2Bunker);
        game.ticks(5);
        game.camera = TableCamera::Table;
        settle(&mut game, 300);
        assert!(!game.actions.iter().any(|a| matches!(a, Action::AskAlone(_))));
        then_the_change(game);
        // opened while its project loads behind the black: its results let go, the black stepped out
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Stk2Bunker);
        game.ticks(20);
        assert_eq!(game.pending, Some(Lobby::Stk2Bunker));
        game.camera = TableCamera::Table;
        game.results();
        assert_eq!(*game.actions.last().unwrap(), Action::Reset);
        settle(&mut game, 300);
        then_the_change(game);
    }

    #[test]
    fn the_lobby_wanted_is_the_shown_one_only_under_the_tables_camera() {
        let loaded = Scene::Loaded { shown: Lobby::Classic, view: View::Drawn, change: Change::Settled };
        assert_eq!(loaded.wanted(Lobby::Hangar046, TableCamera::Table), Lobby::Classic);
        assert_eq!(loaded.wanted(Lobby::Hangar046, TableCamera::Stock), Lobby::Hangar046);
        for scene in [Scene::Unasked, Scene::Starting(Lobby::Classic)] {
            assert_eq!(scene.wanted(Lobby::Hangar046, TableCamera::Table), Lobby::Hangar046, "{scene:?}");
        }
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
        game.ticks(20);
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
        for scene in [Scene::Unasked, Scene::Starting(Lobby::Camp069)] {
            assert_eq!(scene.step(Event::Hidden), (scene, Action::Original));
            assert_eq!(scene.step(Event::Shown), (scene, Action::Original));
            assert_eq!(scene.step(Event::Closing), (scene, Action::Original));
            assert_eq!(scene.step(Event::Tick { dt: FRAME, wanted: Lobby::Hangar046 }), (scene, Action::Original));
        }
    }

    /// Every sequence of events (up to a length) from a lobby shown, patch
    /// 45's camera taken and given back among them: a swap only behind the
    /// black or while not drawn and never under the table's camera, one query
    /// at a time (Game::event), no black left once settled, and the lobby
    /// shown one that was wanted.
    #[test]
    fn no_sequence_swaps_in_sight_or_leaves_the_black() {
        const LOADS: [fn(Lobby) -> bool; 2] = [|_| true, |lobby| lobby != Lobby::Hangar046];
        const KINDS: usize = 9;
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
                        7 => game.want(Lobby::Camp069),
                        _ => {
                            game.camera = match game.camera {
                                TableCamera::Stock => TableCamera::Table,
                                TableCamera::Table => TableCamera::Stock,
                            }
                        }
                    }
                    wanted.push(game.picked);
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
