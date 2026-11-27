//! Patch 81: the lobby's scene is the one the player picks in the lobby
//! window (docs/notes/lobby-scenes.md): the Room (the 0.69d camp, the
//! default), the Basement (the classic), the Bunker (S.T.A.L.K.E.R. 2) or
//! the Hangar (0.46c2). In a squad the lobby is its commander's pick. The
//! four lobbies ship with the game: a project that fails to load is a bug,
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
//! ([`wanted`]): patch 45 read the weapon's spot from the lobby
//! shown's project when its table turned on, so a project put in under it
//! would leave the weapon and the camera at the old lobby's spot in the new
//! scene. A pick, or a squad's, made meanwhile plays once the window is
//! closed and the camera home: a fade begun steps back out, a project that
//! comes behind the black is let go, and the change starts again then.
//!
//! The Room is secretly three rooms (docs/notes/room-weather.md): its day,
//! its rain and its night, a project each ([`Scenery`]). Each entry to the
//! lobby (the game's start, each return from a match) decides the Room's
//! [`Weather`] once, for this client alone: the night by the player's local
//! time ([`NIGHT`]), else the rain of a [`Spell`] (a roll of
//! [`RAIN_PERCENT`] starts one, rainy for [`SPELL_ENTRIES`] more entries,
//! kept in user.cfg as `srp_room_rain`), else the day. Only the machine's
//! entries decide it ([`Scene::step`]'s `enter`): a pick, a squad, a
//! night begun in the lobby change nothing of it. The lobby window, srp and
//! `0xA1` know the Room alone.

#![forbid(unsafe_code)]

use core::ffi::CStr;
use core::num::NonZeroU8;

use crate::vostok::clock::{self, DayMinute};
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
}

/// srp's start of a lobby pick (`3B`; its `lobby_pick::LOBBY_PICK_BASE`,
/// 2 095 000 000): a ranking start, not an address, so made of small
/// numbers for the layers' check of literals.
pub const PICK_BASE: u32 = 2095 * 1000 * 1000;

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

    /// Its value as user.cfg keeps it (one decimal digit).
    pub fn saved(self) -> &'static [u8] {
        match self {
            Lobby::Camp069 => b"1",
            Lobby::Classic => b"2",
            Lobby::Stk2Bunker => b"3",
            Lobby::Hangar046 => b"4",
        }
    }

    /// srp's `3B` start of its pick (the server's `lobby_pick::LOBBY_PICK_BASE`
    /// and its id).
    pub fn pick_start(self) -> u32 {
        PICK_BASE + u32::from(self.id())
    }

    /// What it shows in an entry's weather (the Room's alone has one).
    pub fn scenery(self, weather: Weather) -> Scenery {
        match self {
            Lobby::Camp069 => Scenery::Camp069(weather),
            Lobby::Classic => Scenery::Classic,
            Lobby::Stk2Bunker => Scenery::Stk2Bunker,
            Lobby::Hangar046 => Scenery::Hangar046,
        }
    }
}

/// The Room's weather in an entry to the lobby: one of its three projects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weather {
    /// `lobby_scene/camp_069`, the camp as the 0.69d has it.
    Day,
    /// `lobby_scene/camp_069_rain`.
    Rain,
    /// `lobby_scene/camp_069_night`.
    Night,
}

/// What the lobby shows: a lobby's project, the Room's in its weather.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenery {
    Camp069(Weather),
    Classic,
    Stk2Bunker,
    Hangar046,
}

impl Scenery {
    /// Every project a lobby shows.
    pub const ALL: [Scenery; 6] = [
        Scenery::Camp069(Weather::Day),
        Scenery::Camp069(Weather::Rain),
        Scenery::Camp069(Weather::Night),
        Scenery::Classic,
        Scenery::Stk2Bunker,
        Scenery::Hangar046,
    ];

    /// The lobby it is (the window's card: the Room whatever its weather).
    pub fn lobby(self) -> Lobby {
        match self {
            Scenery::Camp069(_) => Lobby::Camp069,
            Scenery::Classic => Lobby::Classic,
            Scenery::Stk2Bunker => Lobby::Stk2Bunker,
            Scenery::Hangar046 => Lobby::Hangar046,
        }
    }

    /// Its project, as the lobby's query names it.
    pub fn project(self) -> &'static CStr {
        match self {
            Scenery::Classic => c"lobby_scene/day",
            Scenery::Stk2Bunker => c"lobby_scene/stk2_bunker",
            Scenery::Hangar046 => c"lobby_scene/hangar_046",
            Scenery::Camp069(Weather::Day) => c"lobby_scene/camp_069",
            Scenery::Camp069(Weather::Rain) => c"lobby_scene/camp_069_rain",
            Scenery::Camp069(Weather::Night) => c"lobby_scene/camp_069_night",
        }
    }

    /// Its project's config: what a change's query asks in every other
    /// place (a binary config, so no second render scene, sound scene or
    /// movie is made).
    pub fn config(self) -> &'static CStr {
        match self {
            Scenery::Classic => c"resources/projects/lobby_scene/day/client_project",
            Scenery::Stk2Bunker => c"resources/projects/lobby_scene/stk2_bunker/client_project",
            Scenery::Hangar046 => c"resources/projects/lobby_scene/hangar_046/client_project",
            Scenery::Camp069(Weather::Day) => c"resources/projects/lobby_scene/camp_069/client_project",
            Scenery::Camp069(Weather::Rain) => c"resources/projects/lobby_scene/camp_069_rain/client_project",
            Scenery::Camp069(Weather::Night) => c"resources/projects/lobby_scene/camp_069_night/client_project",
        }
    }

    /// Its name in the log.
    pub fn name(self) -> &'static str {
        match self {
            Scenery::Camp069(Weather::Day) => "camp_069",
            Scenery::Camp069(Weather::Rain) => "camp_069_rain",
            Scenery::Camp069(Weather::Night) => "camp_069_night",
            Scenery::Classic => "day",
            Scenery::Stk2Bunker => "stk2_bunker",
            Scenery::Hangar046 => "hangar_046",
        }
    }
}

/// The night in the player's local time: from `from` up to (not at)
/// `until`, across midnight when `from` is the later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Night {
    pub from: DayMinute,
    pub until: DayMinute,
}

/// The Room's night: 21:00 to 06:00 local (the one place to move it).
pub const NIGHT: Night = Night { from: DayMinute::at(21, 0), until: DayMinute::at(6, 0) };

impl Night {
    /// Whether `time` is in it.
    pub fn holds(self, time: DayMinute) -> bool {
        if self.from <= self.until {
            self.from <= time && time < self.until
        } else {
            self.from <= time || time < self.until
        }
    }
}

/// The chance in a hundred that an entry out of a spell starts one.
pub const RAIN_PERCENT: u8 = 15;
/// The entries a spell rains after the one that started it (the owner,
/// 2026-11-02: "if it is raining, it should rain for 3 more enters").
pub const SPELL_ENTRIES: NonZeroU8 = NonZeroU8::new(3).unwrap();

/// A roll of the dice: 0 to 99.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Roll(u8);

impl Roll {
    /// The roll `percent` (cut to 99).
    pub fn percent(percent: u8) -> Roll {
        Roll(percent.min(99))
    }

    /// The roll of 64 random bits (their high half scaled to 0..100).
    fn of_bits(bits: u64) -> Roll {
        Roll((((bits >> 32) * 100) >> 32) as u8)
    }
}

/// The dice: Marsaglia's xorshift64 (its state never 0), seeded once by
/// the performance counter; a roll is its high half's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dice(u64);

impl Dice {
    /// The dice of a seed, stirred a few steps (a counter's seed differs
    /// from a start to the next in its low bits).
    pub fn seeded(seed: u64) -> Dice {
        let mut dice = Dice(seed.max(1));
        for _ in 0..8 {
            dice = dice.next();
        }
        dice
    }

    fn next(self) -> Dice {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        Dice(x)
    }

    /// The next roll and the dice after it.
    pub fn roll(self) -> (Roll, Dice) {
        let dice = self.next();
        (Roll::of_bits(dice.0), dice)
    }
}

/// Whether an entry rains (the night's shows the night all the same).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sky {
    Dry,
    Rain,
}

/// The rain over the entries, as user.cfg keeps it (`srp_room_rain`, the
/// entries it still rains: a restart in a spell keeps it raining).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spell {
    /// No rain: the next entry rolls.
    Clear,
    /// The next `left` entries rain, rolling nothing.
    Raining { left: NonZeroU8 },
}

impl Spell {
    /// The spell a saved value names: `1` to [`SPELL_ENTRIES`] its entries
    /// left, anything else none.
    pub fn parse(saved: &[u8]) -> Spell {
        let left = match saved {
            [digit @ b'0'..=b'9'] => NonZeroU8::new(digit - b'0').filter(|left| *left <= SPELL_ENTRIES),
            _ => None,
        };
        left.map_or(Spell::Clear, |left| Spell::Raining { left })
    }

    /// Its value as user.cfg keeps it (one decimal digit, 0 none).
    pub fn saved(self) -> &'static [u8] {
        const DIGITS: [&[u8]; 10] = [b"0", b"1", b"2", b"3", b"4", b"5", b"6", b"7", b"8", b"9"];
        match self {
            Spell::Clear => DIGITS[0],
            Spell::Raining { left } => DIGITS[usize::from(left.get()).min(9)],
        }
    }

    /// An entry: whether it rains, and the spell after it. Out of a spell
    /// the roll decides (a rain starts one); in one it rains and the spell
    /// counts the entry, the roll unused.
    pub fn enter(self, roll: Roll) -> (Sky, Spell) {
        match self {
            Spell::Clear if roll.0 < RAIN_PERCENT => (Sky::Rain, Spell::Raining { left: SPELL_ENTRIES }),
            Spell::Clear => (Sky::Dry, Spell::Clear),
            Spell::Raining { left } => {
                (Sky::Rain, NonZeroU8::new(left.get() - 1).map_or(Spell::Clear, |left| Spell::Raining { left }))
            }
        }
    }
}

/// An entry's weather: the night in [`NIGHT`] (a spell's entry all the
/// same: the spell counts it), else its sky; and the spell after it.
pub fn enter(spell: Spell, time: DayMinute, roll: Roll) -> (Weather, Spell) {
    let (sky, spell) = spell.enter(roll);
    let weather = match sky {
        _ if NIGHT.holds(time) => Weather::Night,
        Sky::Rain => Weather::Rain,
        Sky::Dry => Weather::Day,
    };
    (weather, spell)
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
    /// The scenery shown is the one wanted.
    Settled,
    /// `lobby`'s project failed to load (a bug, logged): not asked again
    /// until another scenery is wanted (else its fade would play every
    /// second).
    Broken { lobby: Scenery },
    /// The scene stepping to black, `black` of the way (0 to 1).
    Darkening { black: f32 },
    /// Behind the black, `to`'s project asked.
    Loading { to: Scenery },
    /// The new scenery in, drawn behind the black `frames` more frames.
    Holding { frames: u8 },
    /// The scene stepping from black, `black` of the way left; `broken` the
    /// scenery whose project failed behind it (then `Broken`).
    Brightening { black: f32, broken: Option<Scenery> },
}

/// Where the lobby's scene is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scene {
    /// The lobby menu has not asked for it.
    Unasked,
    /// The start: the lobby menu's own query asks `picked`'s project in the
    /// start's `weather`.
    Starting { picked: Lobby, weather: Weather },
    /// The lobby menu has the scenery `shown`; `weather` the Room's in this
    /// entry (what a Room wanted shows, the one shown's or not).
    Loaded { shown: Scenery, weather: Weather, view: View, change: Change },
}

/// What happens to the lobby's scene. `picked` is the lobby picked at that
/// moment ([`Squad::lobby`] of the saved pick and the squad), `camera`
/// patch 45's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// The lobby menu builds its query (`lobby_menu.scene_request`).
    Query { picked: Lobby },
    /// Its results are in (`lobby_menu.scenes_ready`); `loaded`: the
    /// project's place holds a project with its camera track
    /// (ScenesReady::loaded).
    Ready { loaded: bool, picked: Lobby, camera: TableCamera },
    /// A frame of the lobby drawn, `dt` s (`lobby_camera.view`).
    Tick { dt: f32, picked: Lobby, camera: TableCamera },
    /// The lobby is no longer drawn (`lobby_menu.deactivate`: a match).
    Hidden,
    /// The lobby is shown again (`lobby_menu.activate`).
    Shown { picked: Lobby, camera: TableCamera },
    /// The game closes (`game.clear_resources`: it deactivates the active
    /// scene, the lobby too, and clears the lobby menu).
    Closing,
}

/// What follows a project that failed to load (logged).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterBroken {
    /// The start's: the lobby menu takes the results as they are.
    Take,
    /// Behind the black: the black stepped out over the scenery shown.
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
    /// The start's query: this scenery's project in the project's place.
    Ask(Scenery),
    /// A change's query: this scenery's project in the project's place, its
    /// config in every other.
    AskAlone(Scenery),
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
    Broken { lobby: Scenery, then: AfterBroken },
}

/// The black's per mille of `black` (0 to 1).
fn per_mille(black: f32) -> u32 {
    (black.clamp(0.0, 1.0) * 1000.0) as u32
}

/// The scenery wanted at an event of a loaded scene: the lobby picked in
/// this entry's weather, but the one shown while patch 45's camera is the
/// table's (its spot is the shown scenery's project's).
pub fn wanted(shown: Scenery, weather: Weather, picked: Lobby, camera: TableCamera) -> Scenery {
    match camera {
        TableCamera::Table => shown,
        TableCamera::Stock => picked.scenery(weather),
    }
}

impl Scene {
    /// The machine: the next scene and what the site does, for every scene
    /// and every event. A project is put in only behind the black or while
    /// the lobby is not drawn; one change at a time, the last wanted
    /// scenery the next. `enter` is an entry's weather, called at the
    /// entries alone: the start's query and the return from a match (the
    /// lobby shown after it was hidden).
    pub fn step(self, event: Event, enter: impl FnOnce() -> Weather) -> (Scene, Action) {
        use Scene::{Loaded, Starting, Unasked};
        match (self, event) {
            (Unasked, Event::Query { picked }) => {
                let weather = enter();
                (Starting { picked, weather }, Action::Ask(picked.scenery(weather)))
            }
            (Starting { picked, weather }, Event::Query { .. }) => (self, Action::Ask(picked.scenery(weather))),
            (Loaded { change: Change::Loading { to }, .. }, Event::Query { .. }) => (self, Action::AskAlone(to)),
            (Loaded { .. }, Event::Query { .. }) => (self, Action::Original),

            (Unasked, Event::Ready { .. }) => (self, Action::Original),
            (Starting { picked, weather }, Event::Ready { loaded, .. }) => {
                let shown = picked.scenery(weather);
                let action =
                    if loaded { Action::Original } else { Action::Broken { lobby: shown, then: AfterBroken::Take } };
                (Loaded { shown, weather, view: View::Drawn, change: Change::Settled }, action)
            }
            (Loaded { shown, weather, view, change: Change::Loading { to } }, Event::Ready { loaded, picked, camera }) => {
                let wanted = wanted(shown, weather, picked, camera);
                let at = |change| Loaded { shown, weather, view, change };
                match view {
                    View::Closed => (at(Change::Settled), Action::LetGo),
                    _ if to != wanted && wanted == shown => match view {
                        View::Drawn => (at(Change::Brightening { black: 1.0, broken: None }), Action::Reset),
                        View::Hidden | View::Closed => (at(Change::Settled), Action::LetGo),
                    },
                    _ if to != wanted => (at(Change::Loading { to: wanted }), Action::AskAgain),
                    View::Drawn if loaded => (
                        Loaded { shown: to, weather, view, change: Change::Holding { frames: HOLD_FRAMES } },
                        Action::SwapIn,
                    ),
                    View::Hidden if loaded => {
                        (Loaded { shown: to, weather, view, change: Change::Settled }, Action::SwapAndClear)
                    }
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

            (Loaded { shown, weather, view: View::Drawn, change }, Event::Tick { dt, picked, camera }) => {
                let wanted = wanted(shown, weather, picked, camera);
                let at = |change| Loaded { shown, weather, view: View::Drawn, change };
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
            (Unasked | Starting { .. } | Loaded { .. }, Event::Tick { .. }) => (self, Action::Original),

            (Loaded { view: View::Closed, .. }, Event::Hidden | Event::Shown { .. } | Event::Closing) => (self, Action::Original),
            (Loaded { shown, weather, change, .. }, Event::Closing) => {
                let change = if let Change::Loading { .. } = change { change } else { Change::Settled };
                (Loaded { shown, weather, view: View::Closed, change }, Action::Original)
            }
            (Unasked | Starting { .. }, Event::Closing) => (self, Action::Original),
            (Loaded { shown, weather, change, .. }, Event::Hidden) => {
                let hidden = |change| Loaded { shown, weather, view: View::Hidden, change };
                match change {
                    Change::Darkening { .. } | Change::Holding { .. } | Change::Brightening { broken: None, .. } => {
                        (hidden(Change::Settled), Action::Clear)
                    }
                    Change::Brightening { broken: Some(lobby), .. } => (hidden(Change::Broken { lobby }), Action::Clear),
                    Change::Loading { .. } => (hidden(change), Action::Clear),
                    Change::Settled | Change::Broken { .. } => (hidden(change), Action::Original),
                }
            }
            (Unasked | Starting { .. }, Event::Hidden) => (self, Action::Original),

            // the return from a match: an entry, its weather decided; a
            // scenery other than the one shown wanted (the Room's new
            // weather) is asked behind the black from the return's first
            // frame, as a project loading is (the old one never seen again)
            (Loaded { shown, view: View::Hidden, change, .. }, Event::Shown { picked, camera }) => {
                let weather = enter();
                let wanted = wanted(shown, weather, picked, camera);
                let drawn = |change| Loaded { shown, weather, view: View::Drawn, change };
                let black = Action::Shade { per_mille: 1000, loading: true };
                match change {
                    Change::Loading { .. } => (drawn(change), black),
                    Change::Broken { lobby } if wanted == lobby => (drawn(change), Action::Original),
                    Change::Settled | Change::Broken { .. } if wanted != shown => {
                        (drawn(Change::Darkening { black: 1.0 }), black)
                    }
                    // hidden, a change is settled, broken or loading (Event::Hidden)
                    Change::Settled
                    | Change::Broken { .. }
                    | Change::Darkening { .. }
                    | Change::Holding { .. }
                    | Change::Brightening { .. } => (drawn(change), Action::Original),
                }
            }
            // the start's activation (the lobby drawn from its start): no entry
            (Loaded { view: View::Drawn, .. }, Event::Shown { .. }) => (self, Action::Original),
            (Unasked | Starting { .. }, Event::Shown { .. }) => (self, Action::Original),
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

/// The rain's spell as user.cfg keeps it (`srp_room_rain <entries left>`).
static RAIN: ConsoleString<4> = ConsoleString::new();

/// The dice, seeded at the first entry's roll.
static DICE: GameCell<Option<Dice>> = GameCell::new(None);

/// A diagnostics build's Room weather for every entry (`srp_room_weather
/// day`, `rain` or `night` in user.cfg; registered in a diagnostics build
/// alone, so a release's game never knows the line).
static FORCED: ConsoleString<8> = ConsoleString::new();

pub fn on_configs_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    SAVED.register(thread, c"srp_lobby");
    RAIN.register(thread, c"srp_room_rain");
    if trace::enabled() {
        FORCED.register(thread, c"srp_room_weather");
    }
    Flow::Continue
}

/// A diagnostics build's forced weather (none in a release's, or with no
/// value or another).
fn forced(thread: &Thread) -> Option<Weather> {
    if !trace::enabled() {
        return None;
    }
    match FORCED.get(thread) {
        b"day" => Some(Weather::Day),
        b"rain" => Some(Weather::Rain),
        b"night" => Some(Weather::Night),
        _ => None,
    }
}

/// An entry to the lobby ([`Scene::step`]'s `enter`): its weather, from
/// the local time, the saved spell and a roll; the spell after it kept
/// (user.cfg written at once when it changed).
fn entry(thread: &Thread) -> Weather {
    let weather = match forced(thread) {
        Some(weather) => weather,
        None => {
            let dice = DICE.get(thread).unwrap_or_else(|| Dice::seeded(clock::counter(thread)));
            let (roll, dice) = dice.roll();
            DICE.set(thread, Some(dice));
            let before = Spell::parse(RAIN.get(thread));
            let (weather, after) = enter(before, clock::local_time(thread), roll);
            if after != before {
                RAIN.set(thread, after.saved());
                save_user_config(thread);
            }
            weather
        }
    };
    let name: &[u8] = match weather {
        Weather::Day => b"entry: day",
        Weather::Rain => b"entry: rain",
        Weather::Night => b"entry: night",
    };
    diagnose(thread, name);
    weather
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

/// The lobby picked now ([`Squad::lobby`]).
fn picked(thread: &Thread) -> Lobby {
    SQUAD.get(thread).lobby(own(thread))
}

/// Patch 45's camera now.
fn table_camera(thread: &Thread) -> TableCamera {
    WeaponTableState::of(thread).camera()
}

/// One step of the machine, the scene kept (before the site acts: a query
/// it asks may come back at once).
fn step(thread: &Thread, event: Event) -> Action {
    let (scene, action) = SCENE.get(thread).step(event, || entry(thread));
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
    let action = step(thread, Event::Query { picked: picked(thread) });
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
    match step(thread, Event::Ready { loaded: ready.loaded(), picked: picked(thread), camera: table_camera(thread) }) {
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
    let action = step(thread, Event::Tick { dt, picked: picked(thread), camera: table_camera(thread) });
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
        let state = [u32::from(own(thread).id()), u32::from(shown.lobby().id())];
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

/// The lobby shown again: the black back over it while a project loads,
/// or at once when the return's weather wants another Room.
pub fn on_activate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Action::Shade { per_mille, loading } = step(thread, Event::Shown { picked: picked(thread), camera: table_camera(thread) })
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
        }
        assert_eq!(Lobby::ALL.map(Lobby::id), [1, 2, 3, 4]);
        for other in [&b""[..], b"0", b"5", b"12", b" 1", b"a"] {
            assert_eq!(Lobby::parse(other), Lobby::Camp069, "{other:?}");
        }
        assert_eq!(Lobby::of_id(0), None);
    }

    #[test]
    fn each_scenery_is_its_own_project_and_the_rooms_are_the_room() {
        for scenery in Scenery::ALL {
            let project = scenery.project().to_str().unwrap();
            let id = project.strip_prefix("lobby_scene/").unwrap();
            assert_eq!(id, scenery.name());
            assert_eq!(scenery.config().to_str().unwrap(), format!("resources/projects/lobby_scene/{id}/client_project"));
            assert_eq!(Scenery::ALL.iter().filter(|other| other.name() == scenery.name()).count(), 1);
        }
        let room = [Weather::Day, Weather::Rain, Weather::Night].map(|weather| Lobby::Camp069.scenery(weather).name());
        assert_eq!(room, ["camp_069", "camp_069_rain", "camp_069_night"]);
        for weather in [Weather::Day, Weather::Rain, Weather::Night] {
            for lobby in Lobby::ALL {
                assert_eq!(lobby.scenery(weather).lobby(), lobby, "the window's card: the Room whatever its weather");
            }
            assert_eq!(Lobby::Classic.scenery(weather), Scenery::Classic, "the weather is the Room's alone");
        }
    }

    #[test]
    fn the_night_is_21_to_6_local() {
        for (hour, minute, night) in
            [(21, 0, true), (23, 59, true), (0, 0, true), (5, 59, true), (6, 0, false), (12, 0, false), (20, 59, false)]
        {
            assert_eq!(NIGHT.holds(DayMinute::at(hour, minute)), night, "{hour}:{minute:02}");
        }
        // a window within a day
        let evening = Night { from: DayMinute::at(18, 0), until: DayMinute::at(20, 0) };
        assert!(evening.holds(DayMinute::at(19, 0)) && !evening.holds(DayMinute::at(20, 0)) && !evening.holds(DayMinute::at(3, 0)));
        assert_eq!(DayMinute::at(99, 99), DayMinute::at(23, 59), "the clock's fields cut to their last");
    }

    const DRY: Roll = Roll(50);
    const WET: Roll = Roll(0);
    const NOON: DayMinute = DayMinute::at(12, 0);
    const MIDNIGHT: DayMinute = DayMinute::at(0, 30);

    fn left(n: u8) -> Spell {
        Spell::Raining { left: NonZeroU8::new(n).unwrap() }
    }

    #[test]
    fn a_rain_rolled_rains_three_more_entries_then_rolls_again() {
        assert_eq!(Roll::percent(RAIN_PERCENT - 1), Roll(14));
        assert_eq!(enter(Spell::Clear, NOON, Roll::percent(RAIN_PERCENT - 1)), (Weather::Rain, left(3)));
        assert_eq!(enter(Spell::Clear, NOON, Roll::percent(RAIN_PERCENT)), (Weather::Day, Spell::Clear));
        // in a spell the roll is not used
        assert_eq!(enter(left(3), NOON, DRY), (Weather::Rain, left(2)));
        assert_eq!(enter(left(2), NOON, DRY), (Weather::Rain, left(1)));
        assert_eq!(enter(left(1), NOON, DRY), (Weather::Rain, Spell::Clear));
        // after it: a roll again (no dry entry kept)
        assert_eq!(enter(Spell::Clear, NOON, WET), (Weather::Rain, left(3)));
        // the night shows the night; a spell's entry counts all the same, and a rain rolled starts one
        assert_eq!(enter(left(2), MIDNIGHT, DRY), (Weather::Night, left(1)));
        assert_eq!(enter(Spell::Clear, MIDNIGHT, WET), (Weather::Night, left(3)));
        assert_eq!(enter(Spell::Clear, MIDNIGHT, DRY), (Weather::Night, Spell::Clear));
    }

    #[test]
    fn the_spell_is_saved_as_its_entries_left() {
        for spell in [Spell::Clear, left(1), left(2), left(3)] {
            assert_eq!(Spell::parse(spell.saved()), spell);
        }
        assert_eq!(left(3).saved(), b"3");
        assert_eq!(Spell::Clear.saved(), b"0");
        for other in [&b""[..], b"0", b"4", b"9", b"12", b" 1", b"a"] {
            assert_eq!(Spell::parse(other), Spell::Clear, "{other:?}");
        }
    }

    #[test]
    fn the_dice_rain_about_15_in_100() {
        let mut dice = Dice::seeded(0x1234_5678_9ABC);
        let mut wet = 0;
        let mut seen = [false; 100];
        for _ in 0..100_000 {
            let (roll, next) = dice.roll();
            dice = next;
            seen[usize::from(roll.0)] = true;
            if roll.0 < RAIN_PERCENT {
                wet += 1;
            }
        }
        assert!((14_000..16_000).contains(&wet), "{wet} in 100 000");
        assert!(seen.iter().all(|seen| *seen), "every roll 0 to 99 comes");
        // the first roll of a start: counters a tick apart (10 MHz, an hour up) rain as often
        let first = (0..10_000u64).filter(|tick| Dice::seeded(36_000_000_000 + tick).roll().0.0 < RAIN_PERCENT).count();
        assert!((1_300..1_700).contains(&first), "{first} first rolls in 10 000 starts");
        // a spell's share of entries: a roll starts four rainy entries
        let (mut spell, mut rainy, mut dice) = (Spell::Clear, 0, Dice::seeded(7));
        for _ in 0..100_000 {
            let (roll, next) = dice.roll();
            dice = next;
            let (weather, after) = enter(spell, NOON, roll);
            spell = after;
            rainy += usize::from(weather == Weather::Rain);
        }
        assert!((38_000..43_000).contains(&rainy), "{rainy} rainy entries in 100 000 (0.6 / 1.45)");
    }

    #[test]
    fn the_squad_commanders_pick_is_shown_in_a_squad() {
        assert_eq!(Squad::None.lobby(Lobby::Classic), Lobby::Classic);
        assert_eq!(Squad::Led(Lobby::Stk2Bunker).lobby(Lobby::Classic), Lobby::Stk2Bunker);
    }

    /// No entry expected.
    fn no_entry() -> Weather {
        panic!("an entry where there is none")
    }

    #[test]
    fn the_start_asks_the_saved_picks_project_in_its_weather() {
        let (scene, action) = Scene::Unasked.step(Event::Query { picked: Lobby::Hangar046 }, || Weather::Rain);
        assert_eq!(
            (scene, action),
            (Scene::Starting { picked: Lobby::Hangar046, weather: Weather::Rain }, Action::Ask(Scenery::Hangar046))
        );
        let ready = Event::Ready { loaded: true, picked: Lobby::Hangar046, camera: TableCamera::Stock };
        let (scene, action) = scene.step(ready, no_entry);
        assert_eq!(action, Action::Original);
        let loaded =
            Scene::Loaded { shown: Scenery::Hangar046, weather: Weather::Rain, view: View::Drawn, change: Change::Settled };
        assert_eq!(scene, loaded);
        assert_eq!(scene.step(Event::Query { picked: Lobby::Hangar046 }, no_entry), (scene, Action::Original));
        // the Room in the start's weather
        let (scene, action) = Scene::Unasked.step(Event::Query { picked: Lobby::Camp069 }, || Weather::Night);
        assert_eq!(action, Action::Ask(Scenery::Camp069(Weather::Night)));
        assert_eq!(scene.step(Event::Query { picked: Lobby::Classic }, no_entry).1, Action::Ask(Scenery::Camp069(Weather::Night)));
        // a broken project: logged, the lobby menu takes its results as they are
        let starting = Scene::Starting { picked: Lobby::Camp069, weather: Weather::Day };
        let (_, action) = starting.step(Event::Ready { loaded: false, picked: Lobby::Camp069, camera: TableCamera::Stock }, no_entry);
        assert_eq!(action, Action::Broken { lobby: Scenery::Camp069(Weather::Day), then: AfterBroken::Take });
    }

    const FRAME: f32 = 1.0 / 60.0;

    /// A game after its start: the scene and each event's action, every
    /// query and result played as the game plays them (`loads` says whether
    /// a scenery's project loads), and what the screen shows.
    struct Game {
        scene: Scene,
        /// The saved pick and the squad's lobby.
        picked: Lobby,
        /// Patch 45's camera.
        camera: TableCamera,
        loads: fn(Scenery) -> bool,
        /// The weather of each entry from now on, in turn (round).
        weathers: std::vec::Vec<Weather>,
        /// The entries so far.
        entries: usize,
        /// A query asked and its results not yet in, for this scenery.
        pending: Option<Scenery>,
        /// The scenery the lobby menu has.
        menu: Scenery,
        /// The black's per mille.
        black: u32,
        actions: std::vec::Vec<Action>,
    }

    impl Game {
        fn new(shown: Lobby, loads: fn(Scenery) -> bool) -> Game {
            Game::in_weather(shown, Weather::Day, loads)
        }

        fn in_weather(shown: Lobby, weather: Weather, loads: fn(Scenery) -> bool) -> Game {
            let shown_scenery = shown.scenery(weather);
            Game {
                scene: Scene::Loaded { shown: shown_scenery, weather, view: View::Drawn, change: Change::Settled },
                picked: shown,
                camera: TableCamera::Stock,
                loads,
                weathers: std::vec![weather],
                entries: 0,
                pending: None,
                menu: shown_scenery,
                black: 0,
                actions: std::vec![],
            }
        }

        fn wanted(&self) -> Scenery {
            let Scene::Loaded { shown, weather, .. } = self.scene else { panic!("{:?}", self.scene) };
            wanted(shown, weather, self.picked, self.camera)
        }

        fn view(&self) -> View {
            let Scene::Loaded { view, .. } = self.scene else { panic!("{:?}", self.scene) };
            view
        }

        fn event(&mut self, event: Event) -> Action {
            let before = self.scene;
            let next = self.weathers[self.entries % self.weathers.len()];
            let mut entered = 0;
            let (scene, action) = before.step(event, || {
                entered += 1;
                next
            });
            assert!(entered <= 1);
            self.entries += entered;
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
                assert_eq!(shown, self.menu, "the scene's scenery is the menu's");
            }
            action
        }

        /// The lobby's query built.
        fn query(&mut self) {
            match self.event(Event::Query { picked: self.picked }) {
                Action::AskAlone(scenery) => self.pending = Some(scenery),
                other => panic!("a change's query: {other:?}"),
            }
        }

        fn want(&mut self, lobby: Lobby) {
            self.picked = lobby;
        }

        fn tick(&mut self) {
            self.event(Event::Tick { dt: FRAME, picked: self.picked, camera: self.camera });
        }

        fn ticks(&mut self, n: usize) {
            for _ in 0..n {
                self.tick();
            }
        }

        fn shown_again(&mut self) -> Action {
            self.event(Event::Shown { picked: self.picked, camera: self.camera })
        }

        /// The pending query's results come; each re-ask's too.
        fn results(&mut self) {
            while let Some(scenery) = self.pending.take() {
                let loaded = (self.loads)(scenery);
                self.event(Event::Ready { loaded, picked: self.picked, camera: self.camera });
            }
        }

        fn shown(&self) -> Scenery {
            let Scene::Loaded { shown, .. } = self.scene else { panic!("{:?}", self.scene) };
            shown
        }

        /// Nothing more to do: no change out, no black, the wanted scenery
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
        assert!(game.actions.contains(&Action::ShadeAndQuery) && game.actions.contains(&Action::AskAlone(Scenery::Stk2Bunker)));
        assert!((15..=17).contains(&frames), "{frames} frames of fade (0.25 s)");
        assert_eq!(game.menu, Scenery::Classic);
        game.ticks(20);
        assert_eq!(game.black, 1000, "black while its project loads");
        game.results();
        assert_eq!(game.menu, Scenery::Stk2Bunker);
        assert_eq!(*game.actions.last().unwrap(), Action::SwapIn);
        game.ticks(usize::from(HOLD_FRAMES) - 1);
        assert_eq!(game.black, 1000, "the new lobby's first frames behind the black");
        game.tick();
        assert_eq!(*game.actions.last().unwrap(), Action::Reset);
        let frames = settle(&mut game, 60);
        assert!((22..=26).contains(&frames), "{frames} frames of fade in (0.4 s)");
        assert_eq!(game.shown(), Scenery::Stk2Bunker);
        assert_eq!(game.entries, 0, "a pick is no entry");
    }

    #[test]
    fn a_stall_does_not_skip_a_fade() {
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.tick();
        game.event(Event::Tick { dt: 3.0, picked: Lobby::Hangar046, camera: TableCamera::Stock });
        assert!(game.black > 0 && game.black < 200, "a 3 s frame steps the fade by a 30th of a second: {}", game.black);
        assert!(game.pending.is_none());
    }

    #[test]
    fn a_broken_project_is_logged_faded_back_and_not_asked_again() {
        let broken = |scenery: Scenery| scenery.lobby() != Lobby::Camp069;
        let mut game = Game::new(Lobby::Hangar046, broken);
        game.want(Lobby::Camp069);
        settle(&mut game, 200);
        assert_eq!(game.shown(), Scenery::Hangar046);
        assert!(game.actions.contains(&Action::Broken { lobby: Scenery::Camp069(Weather::Day), then: AfterBroken::FadeIn }));
        assert!(!game.actions.contains(&Action::SwapIn));
        let before = game.actions.len();
        game.ticks(30);
        assert!(game.actions[before..].iter().all(|a| *a == Action::Original), "not asked again");
        game.want(Lobby::Classic);
        settle(&mut game, 200);
        assert_eq!(game.shown(), Scenery::Classic, "another lobby wanted: asked");
    }

    #[test]
    fn the_last_lobby_wanted_wins() {
        // changed during the fade: the new one asked at the black
        let mut game = Game::new(Lobby::Classic, |_| true);
        game.want(Lobby::Hangar046);
        game.ticks(5);
        game.want(Lobby::Camp069);
        settle(&mut game, 300);
        assert_eq!(game.shown(), Scenery::Camp069(Weather::Day));
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
        assert_eq!(game.shown(), Scenery::Stk2Bunker);
        assert_eq!(game.actions.iter().filter(|a| **a == Action::SwapIn).count(), 1, "one swap: the last one wanted");
    }

    /// A pick while a weapon's upgrade window has patch 45's camera (its
    /// spot the scenery shown's): no project put in until the camera is the
    /// stock one again, then the change.
    #[test]
    fn a_change_waits_for_the_upgrade_windows_camera() {
        let swaps = |game: &Game| game.actions.iter().filter(|a| **a == Action::SwapIn).count();
        let then_the_change = |mut game: Game| {
            assert_eq!((game.menu, game.black, game.pending), (Scenery::Classic, 0, None), "{:?}", game.scene);
            assert_eq!(swaps(&game), 0);
            game.camera = TableCamera::Stock;
            settle(&mut game, 300);
            assert_eq!(game.shown(), Scenery::Stk2Bunker);
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
        assert_eq!(game.pending, Some(Scenery::Stk2Bunker));
        game.camera = TableCamera::Table;
        game.results();
        assert_eq!(*game.actions.last().unwrap(), Action::Reset);
        settle(&mut game, 300);
        then_the_change(game);
    }

    #[test]
    fn the_scenery_wanted_is_the_shown_one_only_under_the_tables_camera() {
        let shown = Scenery::Classic;
        assert_eq!(wanted(shown, Weather::Rain, Lobby::Hangar046, TableCamera::Table), Scenery::Classic);
        assert_eq!(wanted(shown, Weather::Rain, Lobby::Hangar046, TableCamera::Stock), Scenery::Hangar046);
        assert_eq!(wanted(shown, Weather::Rain, Lobby::Camp069, TableCamera::Stock), Scenery::Camp069(Weather::Rain));
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
            game.shown_again();
            settle(&mut game, 400);
            assert_eq!(game.shown(), Scenery::Hangar046, "{at}");
            assert_eq!(game.black, 0, "{at}");
        }
    }

    /// The owner (2026-11-02): "it shouldn't happen when hopping and
    /// choosing different lobbies, so it is a surprise".
    #[test]
    fn hopping_lobbies_rolls_nothing_and_the_room_keeps_its_weather() {
        let mut game = Game::in_weather(Lobby::Camp069, Weather::Rain, |_| true);
        game.weathers = std::vec![Weather::Night];
        for lobby in [Lobby::Stk2Bunker, Lobby::Camp069, Lobby::Hangar046, Lobby::Camp069] {
            game.want(lobby);
            settle(&mut game, 300);
            assert_eq!(game.shown(), lobby.scenery(Weather::Rain));
        }
        // a squad's lobby, a squad left: the same
        game.want(Lobby::Classic);
        settle(&mut game, 300);
        game.want(Lobby::Camp069);
        settle(&mut game, 300);
        assert_eq!(game.shown(), Scenery::Camp069(Weather::Rain));
        assert_eq!(game.entries, 0);
        // the start's activation of the lobby drawn: no entry either
        assert_eq!(game.shown_again(), Action::Original);
        assert_eq!(game.entries, 0);
    }

    /// A return from a match into another weather: the black at once (the
    /// old Room never seen again), the new Room loaded behind it.
    #[test]
    fn a_return_into_another_weather_loads_it_behind_the_black_from_the_first_frame() {
        let mut game = Game::in_weather(Lobby::Camp069, Weather::Day, |_| true);
        game.weathers = std::vec![Weather::Rain];
        game.event(Event::Hidden);
        assert_eq!(game.shown_again(), Action::Shade { per_mille: 1000, loading: true });
        assert_eq!(game.entries, 1);
        assert_eq!(game.black, 1000);
        game.tick();
        assert_eq!(game.pending, Some(Scenery::Camp069(Weather::Rain)));
        assert_eq!(game.black, 1000);
        game.results();
        assert_eq!(*game.actions.last().unwrap(), Action::SwapIn);
        settle(&mut game, 100);
        assert_eq!(game.shown(), Scenery::Camp069(Weather::Rain));
        let from_return = game.actions.iter().position(|a| *a == Action::Shade { per_mille: 1000, loading: true }).unwrap();
        let swap = game.actions.iter().position(|a| *a == Action::SwapIn).unwrap();
        assert!(
            game.actions[from_return..swap].iter().all(|a| !matches!(a, Action::Shade { per_mille, .. } if *per_mille < 1000)),
            "the black whole from the return to the swap"
        );
        // the same weather again: nothing
        let mut game = Game::in_weather(Lobby::Camp069, Weather::Night, |_| true);
        game.event(Event::Hidden);
        assert_eq!(game.shown_again(), Action::Original);
        assert!(settle(&mut game, 10) == 0 && game.entries == 1);
        // another lobby picked: the weather kept for the Room, nothing shown
        let mut game = Game::in_weather(Lobby::Classic, Weather::Day, |_| true);
        game.weathers = std::vec![Weather::Night];
        game.event(Event::Hidden);
        assert_eq!(game.shown_again(), Action::Original);
        game.want(Lobby::Camp069);
        settle(&mut game, 300);
        assert_eq!(game.shown(), Scenery::Camp069(Weather::Night));
        assert_eq!(game.entries, 1);
        // the upgrade window's camera at the return: the change waits for it
        let mut game = Game::in_weather(Lobby::Camp069, Weather::Day, |_| true);
        game.weathers = std::vec![Weather::Rain];
        game.camera = TableCamera::Table;
        game.event(Event::Hidden);
        assert_eq!(game.shown_again(), Action::Original);
        game.ticks(30);
        assert_eq!(game.shown(), Scenery::Camp069(Weather::Day));
        game.camera = TableCamera::Stock;
        settle(&mut game, 300);
        assert_eq!(game.shown(), Scenery::Camp069(Weather::Rain));
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
        game.shown_again();
        assert_eq!(game.actions[game.actions.len() - 3..], [Action::Original, Action::Original, Action::Original]);
        assert_eq!(game.entries, 0, "no entry once closed");
    }

    #[test]
    fn before_the_scene_nothing_changes() {
        let shown = Event::Shown { picked: Lobby::Hangar046, camera: TableCamera::Stock };
        let tick = Event::Tick { dt: FRAME, picked: Lobby::Hangar046, camera: TableCamera::Stock };
        for scene in [Scene::Unasked, Scene::Starting { picked: Lobby::Camp069, weather: Weather::Day }] {
            for event in [Event::Hidden, shown, Event::Closing, tick] {
                assert_eq!(scene.step(event, no_entry), (scene, Action::Original));
            }
        }
    }

    /// Every sequence of events (up to a length) from a lobby shown, patch
    /// 45's camera taken and given back among them, each return from a match
    /// an entry of another weather: a swap only behind the black or while
    /// not drawn and never under the table's camera, one query at a time
    /// (Game::event), no black left once settled, and the scenery shown one
    /// that was wanted (the Room's in an entry's weather).
    #[test]
    fn no_sequence_swaps_in_sight_or_leaves_the_black() {
        const LOADS: [fn(Scenery) -> bool; 2] = [|_| true, |scenery| scenery != Scenery::Hangar046];
        const KINDS: usize = 9;
        let steps = 6;
        for loads in LOADS {
            for code in 0..KINDS.pow(steps) {
                let mut game = Game::new(Lobby::Classic, loads);
                game.weathers = std::vec![Weather::Rain, Weather::Day, Weather::Night];
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
                            game.shown_again();
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
                    game.shown_again();
                }
                if game.view() == View::Drawn {
                    settle(&mut game, 600);
                    assert_eq!(game.black, 0, "{code}: {:?}", game.scene);
                    if let Scene::Loaded { shown: Scenery::Camp069(shown), weather, change: Change::Settled, .. } = game.scene
                        && game.camera == TableCamera::Stock
                    {
                        assert_eq!(shown, weather, "{code}: the Room in the entry's weather");
                    }
                }
                assert!(wanted.contains(&game.shown().lobby()), "{code}: {:?}", game.scene);
            }
        }
    }
}
