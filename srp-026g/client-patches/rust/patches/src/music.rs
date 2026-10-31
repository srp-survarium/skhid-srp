//! Patch 88: the music (docs/notes/music.md, the owner's rules): the main
//! theme on the login screen, the lobby theme in the lobby, the map's theme
//! from its loading screen to 15 s past the match-start announcement; 2 s
//! fades; never two songs at once.
//!
//! One owner of "the current music": [`Music`], a state machine whose one
//! transition function ([`Music::step`]) takes every event and calls the
//! [`Engine`]. Its tracks play in a sound scene of their own, the bus, whose
//! volume the sound thread steps ([`Bus`]): a fade never waits on a frame.
//! Before the bus is open the asked track waits ([`Glue`]).
//!
//! The hooks (`music.toml`): `on_frame` (game::tick), `on_login`,
//! `on_lobby`, `on_loading` (the mode's `ui.loading_music`),
//! `on_announced` (the welcome message), `on_unload`
//! (game::clear_resources), `on_query_done` (a load's callback) on the
//! game's thread; `on_sound_tick` on the sound thread. A query is asked
//! after the music's lock is let go: its callback may come at once.
//!
//! Presentation only: nothing a match computes.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::resources::{self, Arrived, QueryResults, QueryTag, Serial};
use crate::vostok::settings::ConsoleFloat;
use crate::vostok::sound::{BusScene, Emitter, LoadingMusic, SoundTick, SoundUser, TrackPath, Voice, Volume};
use crate::vostok::log::{self, Piece::Text};
use crate::vostok::{Context, Flow, GameCell, Lock, NoExit, Thread, clock};

/// A fade's length, in and out (retail's 2000 ms).
const FADE_MS: f32 = 2000.0;
/// The map's theme plays on this long past the announcement (the owner's 15 s).
const HOLD_MS: u64 = 15_000;

/// The Settings' Music slider: the stock option names `s_music_volume`, a
/// variable stock 0.26g never registered. 0 to 100 as General's (the
/// slider snaps by 1, options_items 0x262DA6), full at first; saved in user.cfg.
static MUSIC_VOLUME: ConsoleFloat = ConsoleFloat::new(100.0, 0.0, 100.0);

/// The machine's clock, in ms (QueryPerformanceCounter's).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Millis(u64);

impl Millis {
    fn after(self, ms: u64) -> Millis {
        Millis(self.0.saturating_add(ms))
    }
}

/// A theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track {
    /// The login screen's.
    Main,
    /// The lobby's.
    Lobby,
    /// A map's (its mode's `ui.loading_music`).
    Map(TrackPath),
}

impl Track {
    fn path(&self) -> &CStr {
        match self {
            Track::Main => c"music/main_theme",
            Track::Lobby => c"music/lobby_theme",
            Track::Map(path) => path.as_c_str(),
        }
    }
}

/// A track asked for, and when it is to fall silent (a map's, 15 s past
/// the announcement; None until then, and for the menus' themes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Want {
    track: Track,
    until: Option<Millis>,
}

impl Want {
    fn new(track: Track) -> Want {
        Want { track, until: None }
    }

    /// The announcement: a map's theme is given its deadline (once).
    fn announced(self, now: Millis) -> Want {
        match (self.track, self.until) {
            (Track::Map(_), None) => Want { until: Some(now.after(HOLD_MS)), ..self },
            _ => self,
        }
    }

    fn over(&self, now: Millis) -> bool {
        self.until.is_some_and(|until| now >= until)
    }
}

/// What the machine is told.
pub enum Event<E> {
    /// A screen asks for a track (None: no music).
    Ask(Option<Want>),
    /// The match-start announcement.
    Announced,
    /// A frame went by.
    Frame,
    /// A track's load ended (None: it failed).
    Arrived(Serial, Option<E>),
    /// The game unloads its screens.
    Unload,
}

/// What the machine does.
pub trait Engine {
    type Emitter;
    type Voice;
    fn now(&self) -> Millis;
    /// Asks for a track's sound; it arrives as [`Event::Arrived`] with the serial.
    fn load(&mut self, track: &Track) -> Serial;
    /// The sound playing, looped, in the bus.
    fn play(&mut self, emitter: &Self::Emitter) -> Option<Self::Voice>;
    /// The bus's volume moves to `to` over a fade, from where it is.
    fn fade(&mut self, to: Volume);
    /// The bus silent at once (then on towards its target).
    fn cut(&mut self);
    /// The bus is silent, by the latest fade or cut.
    fn silent(&self) -> bool;
}

/// The track playing: its voice and its loaded sound (the voice first,
/// so it stops before its sound is let go).
pub struct Playing<V, E> {
    track: Track,
    _voice: V,
    _emitter: E,
}

/// The current music. At most one [`Playing`] exists, so at most one track
/// is ever heard: a new voice is made only from `Loading`, which holds none.
pub enum Music<E, V> {
    /// Nothing plays.
    Silent,
    /// A track's sound is being loaded; nothing plays.
    Loading { want: Want, serial: Serial },
    /// A track fades in or plays (the bus's target full).
    Sounding { now: Playing<V, E>, until: Option<Millis> },
    /// A track fades out (the bus's target silent); `next` starts when the
    /// bus is silent.
    FadingOut { now: Playing<V, E>, next: Option<Want> },
}

impl<E, V> Music<E, V> {
    /// The transition function: the next state, the engine told what to do.
    /// A track or sound left behind is dropped here (stopped, let go).
    pub fn step<G: Engine<Emitter = E, Voice = V>>(self, event: Event<E>, engine: &mut G) -> Music<E, V> {
        use Music::*;
        let now = engine.now();
        match (self, event) {
            (_, Event::Unload) => {
                engine.cut();
                engine.fade(Volume::SILENT);
                Silent
            }

            (Silent, Event::Ask(want)) => start(want, engine),
            (Loading { want, serial }, Event::Ask(asked)) => match asked {
                Some(asked) if asked.track == want.track => Loading { want, serial },
                asked => start(asked, engine),
            },
            (Sounding { now: playing, .. }, Event::Ask(asked)) => match asked {
                // the track already playing plays on (a new match's map: its own deadline)
                Some(asked) if asked.track == playing.track => Sounding { now: playing, until: asked.until },
                next => {
                    engine.fade(Volume::SILENT);
                    FadingOut { now: playing, next }
                }
            },
            (FadingOut { now: playing, next }, Event::Ask(asked)) => match asked {
                // the track fading out, asked again: back in from where it is
                Some(asked) if asked.track == playing.track => {
                    engine.fade(Volume::FULL);
                    Sounding { now: playing, until: asked.until }
                }
                // the same next track asked again: the fade goes on
                asked if asked.map(|a| a.track) == next.map(|n| n.track) => FadingOut { now: playing, next },
                // another one: the fading track stopped at once
                asked => {
                    drop(playing);
                    engine.cut();
                    start(asked, engine)
                }
            },

            (Loading { want, serial }, Event::Announced) => Loading { want: want.announced(now), serial },
            (Sounding { now: playing, until }, Event::Announced) => {
                let want = Want { track: playing.track, until }.announced(now);
                Sounding { now: playing, until: want.until }
            }
            (FadingOut { now: playing, next }, Event::Announced) => {
                FadingOut { now: playing, next: next.map(|n| n.announced(now)) }
            }
            (Silent, Event::Announced) => Silent,

            (Loading { want, serial }, Event::Arrived(arrived, sound)) if arrived == serial => match sound {
                // a track that is not there: silence
                None => Silent,
                // its time went by while it loaded
                Some(_) if want.over(now) => Silent,
                Some(emitter) => {
                    // the bus is silent (nothing played since it was cut or
                    // faded out): the voice starts there and fades in
                    engine.cut();
                    match engine.play(&emitter) {
                        Some(voice) => {
                            engine.fade(Volume::FULL);
                            Sounding { now: Playing { track: want.track, _voice: voice, _emitter: emitter }, until: want.until }
                        }
                        None => Silent,
                    }
                }
            },
            // a sound nobody waits for any more: let go of it here
            (state, Event::Arrived(..)) => state,

            (Loading { want, .. }, Event::Frame) if want.over(now) => Silent,
            (Sounding { now: playing, until: Some(until) }, Event::Frame) if now >= until => {
                engine.fade(Volume::SILENT);
                FadingOut { now: playing, next: None }
            }
            (FadingOut { now: playing, next }, Event::Frame) if engine.silent() => {
                drop(playing);
                start(next, engine)
            }
            (state, Event::Frame) => state,
        }
    }

    /// The voices held: 0 or 1.
    #[cfg(test)]
    fn voices(&self) -> usize {
        matches!(self, Music::Sounding { .. } | Music::FadingOut { .. }) as usize
    }
}

/// A track starts: its sound is asked for (None, or its time gone: silence).
fn start<E, V, G: Engine<Emitter = E, Voice = V>>(want: Option<Want>, engine: &mut G) -> Music<E, V> {
    match want {
        Some(want) if !want.over(engine.now()) => Music::Loading { serial: engine.load(&want.track), want },
        _ => Music::Silent,
    }
}

/// The bus: what the game's thread asks of its volume, and what the sound
/// thread made of it. The game's thread writes the command, the sound
/// thread steps the volume towards it and says which command it has seen.
pub struct Bus<S> {
    /// The bus's scene, once open (the sound thread keeps it active).
    scene: Option<S>,
    target: Volume,
    /// Cuts asked (each sets the volume to silent at once, once).
    cuts: u32,
    /// Commands given (fades and cuts), and the last the sound thread applied.
    commands: u32,
    seen_cuts: u32,
    seen: u32,
    /// The fade's volume, 0 to 1.
    volume: Volume,
    /// The Music slider's share, 0 to 1 (the game's thread copies it).
    level: Volume,
}

impl<S: Copy> Bus<S> {
    const fn new() -> Bus<S> {
        Bus {
            scene: None, target: Volume::SILENT, cuts: 0, commands: 0, seen_cuts: 0, seen: 0, volume: Volume::SILENT,
            level: Volume::FULL,
        }
    }

    fn fade(&mut self, to: Volume) {
        self.target = to;
        self.commands = self.commands.wrapping_add(1);
    }

    fn cut(&mut self) {
        self.cuts = self.cuts.wrapping_add(1);
        self.commands = self.commands.wrapping_add(1);
    }

    fn silent(&self) -> bool {
        self.seen == self.commands && self.volume == Volume::SILENT
    }

    /// The sound thread's tick, `elapsed` ms after the last: the volume to
    /// give the bus, the fade's times the Music slider's (None before it is
    /// open). The engine multiplies it by General.
    fn tick(&mut self, elapsed: u32) -> Option<(S, Volume)> {
        let scene = self.scene?;
        if self.seen_cuts != self.cuts {
            self.seen_cuts = self.cuts;
            self.volume = Volume::SILENT;
        }
        let step = elapsed as f32 / FADE_MS;
        let (now, to) = (self.volume.get(), self.target.get());
        self.volume = Volume::new(if now < to { (now + step).min(to) } else { (now - step).max(to) });
        self.seen = self.commands;
        Some((scene, Volume::new(self.volume.get() * self.level.get())))
    }
}

/// A query to ask once the music's lock is let go.
enum Query {
    Bus,
    Track(Serial, Track),
}

/// The game's engine for the machine, inside the music's lock.
struct Game<'a> {
    thread: &'a Thread,
    now: Millis,
    sound: SoundUser,
    bus: BusScene,
    serial: &'a mut Serial,
    query: &'a mut Option<Query>,
}

impl Engine for Game<'_> {
    type Emitter = Emitter;
    type Voice = Voice;

    fn now(&self) -> Millis {
        self.now
    }

    fn load(&mut self, track: &Track) -> Serial {
        *self.serial = self.serial.next();
        *self.query = Some(Query::Track(*self.serial, *track));
        *self.serial
    }

    fn play(&mut self, emitter: &Emitter) -> Option<Voice> {
        self.sound.play_looped(self.thread, self.bus, emitter)
    }

    fn fade(&mut self, to: Volume) {
        BUS.with(self.thread, |bus| bus.fade(to));
    }

    fn cut(&mut self) {
        BUS.with(self.thread, |bus| bus.cut());
    }

    fn silent(&self) -> bool {
        BUS.with(self.thread, |bus| bus.silent())
    }
}

/// The music before and after its bus is open.
pub enum Glue {
    /// Before the game's first frame: the asked track waits.
    Waiting { asked: Option<Want> },
    /// The bus asked for; the asked track waits.
    Opening { sound: SoundUser, asked: Option<Want>, attempt: Attempt },
    /// The bus did not load: asked again once, at the next screen's ask.
    Failed { sound: SoundUser, asked: Option<Want> },
    /// The bus did not load twice: no music this session (both logged).
    Off,
    /// The bus open: the machine runs.
    Open { sound: SoundUser, bus: BusScene, music: Music<Emitter, Voice>, serial: Serial },
}

/// Which query of the bus is out: the first, or the one retry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attempt {
    First,
    Retry,
}

/// What the glue is told.
enum Told {
    Frame(Option<SoundUser>),
    Ask(Option<Want>),
    Announced,
    Arrived(Arrived),
    Unload,
}

impl Glue {
    fn on(self, told: Told, thread: &Thread, now: Millis, query: &mut Option<Query>) -> Glue {
        match self {
            Glue::Waiting { asked } => match told {
                Told::Frame(Some(sound)) => {
                    *query = Some(Query::Bus);
                    Glue::Opening { sound, asked, attempt: Attempt::First }
                }
                told => Glue::Waiting { asked: wait(asked, told, now) },
            },
            Glue::Opening { sound, asked, attempt } => match told {
                Told::Arrived(Arrived::Bus(Some(bus))) => {
                    BUS.with(thread, |b| b.scene = Some(bus));
                    let mut serial = Serial::FIRST;
                    let mut game = Game { thread, now, sound, bus, serial: &mut serial, query };
                    let music = Music::Silent.step(Event::Ask(asked), &mut game);
                    Glue::Open { sound, bus, music, serial }
                }
                Told::Arrived(Arrived::Bus(None)) => {
                    log::line(&[Text("music: the bus (a sound_scene of class 38) did not load"), Text(match attempt {
                        Attempt::First => "; asked again at the next screen",
                        Attempt::Retry => " again; no music this session",
                    })]);
                    match attempt {
                        Attempt::First => Glue::Failed { sound, asked },
                        Attempt::Retry => Glue::Off,
                    }
                }
                told => Glue::Opening { sound, asked: wait(asked, told, now), attempt },
            },
            Glue::Failed { sound, asked } => match told {
                Told::Ask(want) => {
                    *query = Some(Query::Bus);
                    Glue::Opening { sound, asked: want, attempt: Attempt::Retry }
                }
                told => Glue::Failed { sound, asked: wait(asked, told, now) },
            },
            // a sound that arrives now is let go here
            Glue::Off => Glue::Off,
            Glue::Open { sound, bus, music, mut serial } => {
                let event = match told {
                    Told::Frame(_) => Event::Frame,
                    Told::Ask(want) => Event::Ask(want),
                    Told::Announced => Event::Announced,
                    Told::Arrived(Arrived::Track(arrived, emitter)) => Event::Arrived(arrived, emitter),
                    // the bus is open already: never asked twice (one query, or
                    // a retry after a failure), so none comes; one would keep
                    // its reference (a scene is never let go)
                    Told::Arrived(Arrived::Bus(_)) => Event::Frame,
                    Told::Unload => Event::Unload,
                };
                let mut game = Game { thread, now, sound, bus, serial: &mut serial, query };
                let music = music.step(event, &mut game);
                Glue::Open { sound, bus, music, serial }
            }
        }
    }
}

/// The asked track while the bus is not open.
fn wait(asked: Option<Want>, told: Told, now: Millis) -> Option<Want> {
    match told {
        Told::Ask(want) => want,
        Told::Announced => asked.map(|a| a.announced(now)),
        Told::Unload => None,
        Told::Frame(_) | Told::Arrived(_) => asked,
    }
}

static MUSIC: Lock<Glue> = Lock::new(Glue::Waiting { asked: None });
static BUS: Lock<Bus<BusScene>> = Lock::new(Bus::new());

/// The game thread's ms clock, from the machine's counter: its low word's
/// steps since the last call (well under a wrap: a call a frame), so no
/// 64-bit division is needed.
#[derive(Clone, Copy)]
enum Clock {
    Unstarted,
    Running { last: u32, per_ms: u32, carry: u32, now: Millis },
}

static CLOCK: GameCell<Clock> = GameCell::new(Clock::Unstarted);

fn now(thread: &Thread) -> Millis {
    let ticks = clock::counter(thread) as u32;
    let next = match CLOCK.get(thread) {
        Clock::Running { last, per_ms, carry, now } => {
            let steps = ticks.wrapping_sub(last).wrapping_add(carry);
            Clock::Running { last: ticks, per_ms, carry: steps % per_ms, now: now.after(u64::from(steps / per_ms)) }
        }
        Clock::Unstarted => match u32::try_from(clock::frequency(thread)).map(|f| f / 1000) {
            Ok(per_ms) if per_ms > 0 => Clock::Running { last: ticks, per_ms, carry: 0, now: Millis(0) },
            _ => Clock::Unstarted,
        },
    };
    CLOCK.set(thread, next);
    match next {
        Clock::Running { now, .. } => now,
        Clock::Unstarted => Millis(0),
    }
}

/// Tells the music, then asks the query it wants (the lock let go).
fn tell(thread: &Thread, told: Told) {
    let now = now(thread);
    let mut query = None;
    MUSIC.with(thread, |glue| {
        let state = core::mem::replace(glue, Glue::Waiting { asked: None });
        *glue = state.on(told, thread, now, &mut query);
    });
    match query {
        Some(Query::Bus) => resources::query_bus(thread, crate::vostok::music_on_query_done_entry()),
        Some(Query::Track(serial, track)) => {
            resources::query_track(thread, track.path(), serial, crate::vostok::music_on_query_done_entry())
        }
        None => {}
    }
}

/// game::on_configs_loaded, before user.cfg: the Music slider's variable.
pub fn on_configs(ctx: &mut Context<'_>) -> Flow<NoExit> {
    MUSIC_VOLUME.register(ctx.thread(), c"s_music_volume");
    Flow::Continue
}

/// game::tick (every frame): the slider's level to the bus, then the music.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let level = Volume::new(MUSIC_VOLUME.get(thread) / 100.0);
    BUS.with(thread, |bus| bus.level = level);
    tell(thread, Told::Frame(SoundUser::at_game_tick(ctx)));
    Flow::Continue
}

/// login_menu::on_activate: the main theme.
pub fn on_login(ctx: &mut Context<'_>) -> Flow<NoExit> {
    tell(ctx.thread(), Told::Ask(Some(Want::new(Track::Main))));
    Flow::Continue
}

/// lobby_menu::on_activate: the lobby theme.
pub fn on_lobby(ctx: &mut Context<'_>) -> Flow<NoExit> {
    tell(ctx.thread(), Told::Ask(Some(Want::new(Track::Lobby))));
    Flow::Continue
}

/// switch_to_level_loading: the mode's theme, or none.
pub fn on_loading(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let want = match LoadingMusic::at_level_loading(ctx) {
        LoadingMusic::Track(path) => Some(Want::new(Track::Map(path))),
        LoadingMusic::None => None,
    };
    tell(ctx.thread(), Told::Ask(want));
    Flow::Continue
}

/// A replay's options read (its chunk 1): a replay has no theme (it never
/// passes switch_to_level_loading, whose only caller is a live match's
/// process_static_match_info 0x27BEA0): what plays fades out.
pub fn on_replay(ctx: &mut Context<'_>) -> Flow<NoExit> {
    tell(ctx.thread(), Told::Ask(None));
    Flow::Continue
}

/// update_ui's welcome message: the match has started.
pub fn on_announced(ctx: &mut Context<'_>) -> Flow<NoExit> {
    tell(ctx.thread(), Told::Announced);
    Flow::Continue
}

/// game::clear_resources: the music stops.
pub fn on_unload(ctx: &mut Context<'_>) -> Flow<NoExit> {
    tell(ctx.thread(), Told::Unload);
    Flow::Continue
}

/// A load's callback, on the thread that asked (the game's).
pub fn on_query_done(results: QueryResults<'_>, tag: QueryTag) {
    tell(results.thread(), Told::Arrived(results.arrived(tag)));
}

/// The sound thread's tick: the bus kept active at its stepped volume.
pub fn on_sound_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(tick) = SoundTick::at(ctx)
        && let Some((scene, volume)) = BUS.with(ctx.thread(), |bus| bus.tick(tick.elapsed()))
    {
        tick.keep(scene, volume);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests;
