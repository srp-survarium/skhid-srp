//! Patch 155: room acoustics, as 0.69d has them (docs/notes/room-acoustics.md).
//! A map's sound environments (its client project's `srp_sound_environments`,
//! read once as it is inserted) are rooms of boxes; a point's room is the
//! first box surface straight above it. A composite's item tagged
//! `sound_environment <room>` plays only in that room: a placed sound in the
//! room of its source, a hud sound in the listener's. The listener's room
//! (the local player's feet while he is alive, else the camera) plays its
//! environment's bed, crossfaded at a room change.
//!
//! The owners: [`ROOMS`] (the session's room names, the map's zones and the
//! listener's place; the cook's, the sound's and the game's threads read
//! it), [`BEDS`] (the beds' machine, the game's thread), [`MIX`] (the beds'
//! scenes and volumes, which the sound thread steps). A map without zones
//! is all outdoors ("default"): every untagged and every `default` item
//! plays, as the game plays them today, and no bed.
//!
//! Presentation only: nothing a match computes.

#![forbid(unsafe_code)]

pub mod model;

use crate::vostok::config::{ConfigError, ConfigTable, ConfigValue};
use crate::vostok::log::{self, Piece::{Dec, Text}};
use crate::vostok::project::{self, ProjectKey};
use crate::vostok::resources::{self, Arrived, QueryResults, QueryTag, Serial};
use crate::vostok::room_sound::{Heard, ItemPlay, ItemRead, Listening};
use crate::vostok::sound::{BusScene, Emitter, SoundTick, SoundUser, TrackPath, Voice, Volume};
use crate::vostok::{Context, Flow, Lock, NoExit, RoomAcousticsOnItemPlayExit, Thread};
use model::{Bed, BedEvent, Beds, Engine, Environment, FadeMs, ItemRoom, Mix, Place, RoomNames, Toward, ZoneShape, Form, Zones,
            SLOTS};

/// The data's version (the edit list's `version`).
const VERSION: u32 = 1;

/// The map's zones, once a project with them is inserted.
enum Map {
    Unzoned,
    Zoned { project: ProjectKey, zones: Zones, listener: Place },
}

struct Rooms {
    names: RoomNames,
    map: Map,
    /// A tag the names had no number for was logged.
    full_logged: bool,
}

static ROOMS: Lock<Rooms> = Lock::new(Rooms { names: RoomNames::new(), map: Map::Unzoned, full_logged: false });

/// Why a map's data was refused.
#[derive(Debug)]
enum DataError {
    Config(ConfigError),
    Version,
    Value(&'static str),
    TooMany,
}

impl From<ConfigError> for DataError {
    fn from(value: ConfigError) -> Self {
        DataError::Config(value)
    }
}

impl DataError {
    fn why(&self) -> &'static str {
        match self {
            DataError::Config(ConfigError::Missing) => "a key missing",
            DataError::Config(_) => "a value of the wrong type",
            DataError::Version => "another version",
            DataError::Value(what) => what,
            DataError::TooMany => "more environments or meshes than kept",
        }
    }
}

fn floats<const N: usize>(table: &ConfigTable<'_>, keys: [&core::ffi::CStr; N]) -> Result<[f32; N], DataError> {
    let mut out = [0.0; N];
    for (value, key) in out.iter_mut().zip(keys) {
        *value = table.required(key)?.number()?;
    }
    Ok(out)
}

fn fade(table: &ConfigTable<'_>, key: &core::ffi::CStr) -> Result<FadeMs, DataError> {
    Ok(FadeMs::new(table.required(key)?.integer()?))
}

/// A project's `srp_sound_environments`, parsed once (None: it has none).
fn parse(root: ConfigValue<'_>, names: &mut RoomNames) -> Result<Option<Zones>, DataError> {
    let Some(spec) = root.table()?.get(c"srp_sound_environments")? else { return Ok(None) };
    let spec = spec.table()?;
    if spec.required(c"version")?.integer()? != VERSION {
        return Err(DataError::Version);
    }
    let mut zones = Zones::new();
    for environment in spec.required(c"environments")?.entries()?.iter() {
        let environment = environment?.table()?;
        let name = environment.required(c"room")?.text()?;
        let room = names.room(name.to_bytes()).ok_or(DataError::Value("a room name empty or past the table"))?;
        let bed = match environment.get(c"ambient_sound")? {
            None => None,
            Some(sound) => Some(Bed {
                sound: TrackPath::new(sound.text()?).ok_or(DataError::Value("a bed that is not a sound's path"))?,
                fade_in: fade(&environment, c"fade_in")?,
                fade_out: fade(&environment, c"fade_out")?,
            }),
        };
        let index = zones.add(Environment { room, bed }).map_err(|_| DataError::TooMany)?;
        for shape in environment.required(c"meshes")?.entries()?.iter() {
            let shape = shape?.table()?;
            let centre = floats(&shape, [c"x", c"y", c"z"])?;
            let [a, b, c, d, e, f, g, h, i] =
                floats(&shape, [c"r00", c"r01", c"r02", c"r10", c"r11", c"r12", c"r20", c"r21", c"r22"])?;
            let [hx, hy, hz] = floats(&shape, [c"hx", c"hy", c"hz"])?;
            // 0.69d's mesh types: 1 a box, 2 a cylinder, 0 a sphere
            let form = match shape.required(c"kind")?.integer()? {
                1 => Form::Box { half: [hx, hy, hz] },
                2 => Form::Cylinder { radius: hx, half_height: hy },
                0 => Form::Sphere { radius: hx },
                _ => return Err(DataError::Value("a mesh of no known kind")),
            };
            let shape = ZoneShape::parse(centre, [[a, b, c], [d, e, f], [g, h, i]], form)
                .ok_or(DataError::Value("a mesh that is not one"))?;
            zones.add_shape(index, shape).map_err(|_| DataError::TooMany)?;
        }
    }
    Ok(Some(zones))
}

/// project.insert: a map's zones, read once.
pub fn on_project_insert(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some((key, root)) = project::inserted(ctx) else { return Flow::Continue };
    let zoned = ROOMS.with(thread, |rooms| {
        let parsed = root.map_err(|_| DataError::Config(ConfigError::Missing)).and_then(|root| parse(root, &mut rooms.names));
        match parsed {
            Ok(Some(zones)) => {
                rooms.map = Map::Zoned { project: key, zones, listener: Place::Outdoors };
                true
            }
            Ok(None) => false,
            Err(error) => {
                log::line(&[Text("room acoustics: this map's sound environments are not used, its data has "),
                            Text(error.why())]);
                false
            }
        }
    });
    if zoned {
        beds(thread, BedEvent::Cut);
    }
    Flow::Continue
}

fn gone(thread: &Thread, key: Option<ProjectKey>) {
    let Some(key) = key else { return };
    let was = ROOMS.with(thread, |rooms| match rooms.map {
        Map::Zoned { project, .. } if project == key => {
            rooms.map = Map::Unzoned;
            true
        }
        _ => false,
    });
    if was {
        beds(thread, BedEvent::Cut);
    }
}

/// project.removed: the map's zones go with it, and its beds at once.
pub fn on_project_removed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    gone(ctx.thread(), project::removed(ctx));
    Flow::Continue
}

/// project.destroy: as removed.
pub fn on_project_destroy(ctx: &mut Context<'_>) -> Flow<NoExit> {
    gone(ctx.thread(), project::destroying(ctx));
    Flow::Continue
}

/// A composite's item read: its room kept with it.
pub fn on_item_read(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(item) = ItemRead::at(ctx) else { return Flow::Continue };
    let tag = item.tag().map(|t| t.to_bytes());
    let room = ROOMS.with(ctx.thread(), |rooms| {
        let room = rooms.names.item(tag);
        if room == ItemRoom::Unnumbered && !rooms.full_logged {
            rooms.full_logged = true;
            log::line(&[Text("room acoustics: more room names than "), Dec(model::ROOMS_MOST as u32),
                        Text("; an item of a room past them never plays")]);
        }
        room
    });
    item.set_room(room.byte());
    Flow::Continue
}

/// A composite's item about to play: on only in its room.
pub fn on_item_play(ctx: &mut Context<'_>) -> Flow<RoomAcousticsOnItemPlayExit> {
    let Some(item) = ItemPlay::at(ctx) else { return Flow::Continue };
    let room = ItemRoom::from_byte(item.room());
    if room == ItemRoom::Any {
        return Flow::Continue;
    }
    let plays = ROOMS.with(ctx.thread(), |rooms| {
        let outdoors = rooms.names.outdoors();
        let here = match &rooms.map {
            Map::Unzoned => outdoors,
            Map::Zoned { zones, listener, .. } => match item.heard() {
                Heard::AtListener => zones.room(*listener, outdoors),
                Heard::AtSource(point) => zones.room(zones.place(point), outdoors),
            },
        };
        room.plays_in(here)
    });
    if plays { Flow::Continue } else { Flow::Exit(RoomAcousticsOnItemPlayExit::Skip) }
}

/// Every frame of a game world: the listener's room, its bed.
pub fn on_listener(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    open_scenes(thread, SoundUser::at_game_world_listener(ctx));
    let Some(listening) = Listening::at(ctx) else { return Flow::Continue };
    // 0.69d: the controlled player's own room while he is alive, else the
    // listener's position (the camera: the death camera, a spectator)
    let point = listening.alive_player.unwrap_or(listening.listener);
    let moved = ROOMS.with(thread, |rooms| match &mut rooms.map {
        Map::Unzoned => None,
        Map::Zoned { zones, listener, .. } => {
            let place = zones.place(point);
            (place != *listener).then(|| {
                let left = zones.environment(*listener).and_then(|e| e.bed);
                *listener = place;
                (left, zones.environment(place).and_then(|e| e.bed))
            })
        }
    });
    if let Some((left, entered)) = moved {
        beds(thread, BedEvent::Moved { left, entered });
    }
    beds(thread, BedEvent::Frame);
    Flow::Continue
}

/// The beds' scenes and their volumes, which the sound thread steps.
struct Shared {
    scenes: [Option<BusScene>; SLOTS],
    mix: Mix,
}

static MIX: Lock<Shared> = Lock::new(Shared { scenes: [None; SLOTS], mix: Mix::new() });

/// The beds before and after their scenes are asked for.
enum Glue {
    /// Before a game world's first frame.
    Waiting,
    /// The scenes asked for (they arrive one by one, into free slots).
    Open { sound: SoundUser, beds: Beds<Emitter, Voice>, serial: Serial },
}

static BEDS: Lock<Glue> = Lock::new(Glue::Waiting);

/// A query the beds want, asked once their lock is let go (its callback
/// may come at once).
enum Query {
    Scenes,
    Bed(Serial, TrackPath),
}

struct Game<'a> {
    thread: &'a Thread,
    sound: SoundUser,
    serial: &'a mut Serial,
    queries: &'a mut [Option<Query>; SLOTS + 1],
}

impl Game<'_> {
    fn ask(&mut self, query: Query) {
        if let Some(slot) = self.queries.iter_mut().find(|q| q.is_none()) {
            *slot = Some(query);
        }
    }
}

impl Engine for Game<'_> {
    type Emitter = Emitter;
    type Voice = Voice;

    fn load(&mut self, sound: &TrackPath) -> Serial {
        let serial = *self.serial;
        *self.serial = serial.next();
        self.ask(Query::Bed(serial, *sound));
        serial
    }

    fn play(&mut self, slot: usize, emitter: &Emitter) -> Option<Voice> {
        let scene = MIX.with(self.thread, |shared| shared.scenes[slot])?;
        self.sound.play_looped(self.thread, scene, emitter)
    }

    fn fade(&mut self, slot: usize, toward: Toward, ms: FadeMs) {
        MIX.with(self.thread, |shared| shared.mix.fade(slot, toward, ms));
    }

    fn cut(&mut self, slot: usize) {
        MIX.with(self.thread, |shared| shared.mix.cut(slot));
    }

    fn silent(&self, slot: usize) -> bool {
        MIX.with(self.thread, |shared| shared.mix.silent(slot))
    }
}

fn ask(thread: &Thread, queries: [Option<Query>; SLOTS + 1]) {
    for query in queries.into_iter().flatten() {
        match query {
            Query::Scenes => {
                for _ in 0..SLOTS {
                    resources::query_bus(thread, crate::vostok::room_acoustics_on_query_done_entry());
                }
            }
            Query::Bed(serial, sound) => {
                resources::query_track(thread, sound.as_c_str(), serial,
                                       crate::vostok::room_acoustics_on_query_done_entry())
            }
        }
    }
}

/// The scenes asked for at the first frame with the game's sound world.
fn open_scenes(thread: &Thread, sound: Option<SoundUser>) {
    let mut queries = [const { None }; SLOTS + 1];
    BEDS.with(thread, |glue| {
        if let (Glue::Waiting, Some(sound)) = (&*glue, sound) {
            queries[0] = Some(Query::Scenes);
            *glue = Glue::Open { sound, beds: Beds::new(), serial: Serial::FIRST };
        }
    });
    ask(thread, queries);
}

fn beds(thread: &Thread, event: BedEvent<Emitter>) {
    let mut queries = [const { None }; SLOTS + 1];
    BEDS.with(thread, |glue| {
        if let Glue::Open { sound, beds, serial } = glue {
            let mut game = Game { thread, sound: *sound, serial, queries: &mut queries };
            beds.step(event, &mut game);
        }
    });
    ask(thread, queries);
}

/// A query's callback, on the game's thread: a scene or a bed's sound.
pub fn on_query_done(results: QueryResults<'_>, tag: QueryTag) {
    let thread = results.thread();
    match results.arrived(tag) {
        Arrived::Bus(Some(scene)) => {
            scene.listen(thread);
            MIX.with(thread, |shared| {
                if let Some(free) = shared.scenes.iter_mut().find(|s| s.is_none()) {
                    *free = Some(scene);
                }
            });
        }
        Arrived::Bus(None) => {
            log::line(&[Text("room acoustics: a bed's scene (a sound_scene of class 38) did not load")]);
        }
        Arrived::Track(serial, emitter) => beds(thread, BedEvent::Arrived(serial, emitter)),
        Arrived::Effects(_) => {}
    }
}

/// The sound thread's tick: each bed's scene kept active at its volume.
pub fn on_sound_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(tick) = SoundTick::at(ctx) {
        let scenes = MIX.with(ctx.thread(), |shared| {
            let volumes = shared.mix.tick(tick.elapsed());
            core::array::from_fn::<_, SLOTS, _>(|slot| shared.scenes[slot].map(|scene| (scene, volumes[slot])))
        });
        for (scene, volume) in scenes.into_iter().flatten() {
            tick.keep(scene, Volume::new(volume));
        }
    }
    Flow::Continue
}

#[cfg(test)]
mod tests;
