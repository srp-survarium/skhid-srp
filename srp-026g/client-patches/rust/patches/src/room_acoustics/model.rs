//! The rooms of patch 155 as 0.69d has them (docs/notes/room-acoustics.md):
//! room names, the zones of a map and the upward ray that finds a point's
//! zone, the composite items' room rule, and the beds' state machine. No
//! game memory or native handles.
#![forbid(unsafe_code)]

use core::num::NonZeroU8;

use crate::vostok::collection::BoundedVec;
use crate::vostok::resources::Serial;
use crate::vostok::sound::TrackPath;

/// A room name's longest: 0.69d keeps a tag in 32 bytes with its zero
/// (0x2006329EF, 0x20023B149), so a longer one is cut there.
pub const NAME_MOST: usize = 31;
/// The room names a session can hold: every tag of every composite and the
/// zones' names, once each.
pub const ROOMS_MOST: usize = 253;
/// 0.69d's room where no zone is: its controller's built-in first
/// environment (0x2001F84FD), with no bed.
pub const OUTDOORS: &[u8] = b"default";
/// The upward ray's length (0x20023AF50: (0, 100, 0)).
pub const RAY_UP: f32 = 100.0;
pub const ENVIRONMENTS_MOST: usize = 32;
pub const SHAPES_MOST: usize = 256;
/// The beds that can sound at once: the one fading in and those still
/// fading out of the rooms left.
pub const SLOTS: usize = 4;

/// A room by its name, a number of this session's [`RoomNames`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomTag(NonZeroU8);

impl RoomTag {
    /// Its number, 1 the outdoors'.
    pub fn number(self) -> u8 {
        self.0.get()
    }
}

/// The room an item of a composite plays in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemRoom {
    /// No `sound_environment`: in every room.
    Any,
    /// Only in that room.
    Room(RoomTag),
    /// A name the session had no number left for: never (logged once).
    Unnumbered,
}

const ANY_BYTE: u8 = 0;
const UNNUMBERED_BYTE: u8 = u8::MAX;

impl ItemRoom {
    /// The byte kept with the item.
    pub fn byte(self) -> u8 {
        match self {
            ItemRoom::Any => ANY_BYTE,
            ItemRoom::Room(tag) => tag.0.get(),
            ItemRoom::Unnumbered => UNNUMBERED_BYTE,
        }
    }

    /// The byte read back from the item (the inverse of [`ItemRoom::byte`]).
    pub fn from_byte(byte: u8) -> ItemRoom {
        match byte {
            ANY_BYTE => ItemRoom::Any,
            UNNUMBERED_BYTE => ItemRoom::Unnumbered,
            n => NonZeroU8::new(n).map_or(ItemRoom::Any, |n| ItemRoom::Room(RoomTag(n))),
        }
    }

    /// 0.69d's rule (composite_sound::play 0x200635F70): an item plays when
    /// it has no tag, or its tag is the room's name, byte for byte. There is
    /// no fallback: a room no item names plays the untagged items alone.
    pub fn plays_in(self, room: RoomTag) -> bool {
        match self {
            ItemRoom::Any => true,
            ItemRoom::Room(tag) => tag == room,
            ItemRoom::Unnumbered => false,
        }
    }
}

/// The session's room names (but the outdoors'), each a number for good: a composite's items
/// keep theirs from its load on, whichever map is played.
pub struct RoomNames {
    names: [[u8; NAME_MOST]; ROOMS_MOST],
    lengths: [u8; ROOMS_MOST],
    len: usize,
}

impl RoomNames {
    /// Empty (all zeros: kept in the arena's state); the outdoors' name is
    /// number 1 without a place in the table.
    pub const fn new() -> RoomNames {
        RoomNames { names: [[0; NAME_MOST]; ROOMS_MOST], lengths: [0; ROOMS_MOST], len: 0 }
    }

    /// The outdoors' room ("default").
    pub fn outdoors(&self) -> RoomTag {
        RoomTag(NonZeroU8::MIN)
    }

    /// The room of `name` (cut at 31 bytes, as 0.69d keeps it), numbered
    /// now if it is new; None for an empty name or when no number is left.
    pub fn room(&mut self, name: &[u8]) -> Option<RoomTag> {
        let name = &name[..name.len().min(NAME_MOST)];
        if name.is_empty() {
            return None;
        }
        if name == OUTDOORS {
            return Some(self.outdoors());
        }
        let found = (0..self.len).find(|&i| &self.names[i][..usize::from(self.lengths[i])] == name);
        let index = match found {
            Some(i) => i,
            None if self.len < ROOMS_MOST => {
                self.names[self.len][..name.len()].copy_from_slice(name);
                self.lengths[self.len] = name.len() as u8;
                self.len += 1;
                self.len - 1
            }
            None => return None,
        };
        // 1 is the outdoors', so the table's numbers start at 2
        NonZeroU8::new(index as u8 + 2).map(RoomTag)
    }

    /// A room's name (a valid UTF-8 one; else "?").
    pub fn name(&self, room: RoomTag) -> &str {
        let bytes = match usize::from(room.0.get()) {
            1 => OUTDOORS,
            n => self.names.get(n - 2).map_or(&b"?"[..], |name| &name[..usize::from(self.lengths[n - 2])]),
        };
        core::str::from_utf8(bytes).unwrap_or("?")
    }

    /// An item's room from its tag (None: no `sound_environment`).
    pub fn item(&mut self, tag: Option<&[u8]>) -> ItemRoom {
        match tag {
            None | Some([]) => ItemRoom::Any,
            Some(name) => self.room(name).map_or(ItemRoom::Unnumbered, ItemRoom::Room),
        }
    }
}

impl Default for RoomNames {
    fn default() -> Self {
        Self::new()
    }
}

/// A zone's mesh, as 0.69d's `sound_environment_geometry` primitives have
/// them (their `meshes[].type`): a box by its half sizes, a cylinder along
/// its own y by its radius and half height, a sphere by its radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Form {
    Box { half: [f32; 3] },
    Cylinder { radius: f32, half_height: f32 },
    Sphere { radius: f32 },
}

/// A placed mesh of a zone: its centre, the rows taking a world offset
/// into its own axes, and its form.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoneShape {
    centre: [f32; 3],
    rows: [[f32; 3]; 3],
    form: Form,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The t where a ray o + t d is inside a slab |x| <= half (d its component).
fn slab(o: f32, d: f32, half: f32) -> Option<(f32, f32)> {
    if d.abs() < 1e-6 {
        return (o.abs() <= half).then_some((f32::NEG_INFINITY, f32::INFINITY));
    }
    let (a, b) = ((-half - o) / d, (half - o) / d);
    Some((a.min(b), a.max(b)))
}

/// The t where o + t d is within `radius` of the origin, along the axes
/// given (d not 0 along them: a quadratic; else inside or never).
fn round(o: [f32; 2], d: [f32; 2], radius: f32) -> Option<(f32, f32)> {
    let a = d[0] * d[0] + d[1] * d[1];
    let c = o[0] * o[0] + o[1] * o[1] - radius * radius;
    if a < 1e-12 {
        return (c <= 0.0).then_some((f32::NEG_INFINITY, f32::INFINITY));
    }
    let b = o[0] * d[0] + o[1] * d[1];
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let root = core::f32::math::sqrt(disc);
    Some(((-b - root) / a, (-b + root) / a))
}

fn both(a: (f32, f32), b: (f32, f32)) -> Option<(f32, f32)> {
    let (near, far) = (a.0.max(b.0), a.1.min(b.1));
    (near <= far).then_some((near, far))
}

impl ZoneShape {
    /// Finite values, positive sizes and unit rows at right angles.
    pub fn parse(centre: [f32; 3], rows: [[f32; 3]; 3], form: Form) -> Option<ZoneShape> {
        let sizes: &[f32] = match &form {
            Form::Box { half } => half,
            Form::Cylinder { radius, half_height } => &[*radius, *half_height],
            Form::Sphere { radius } => core::slice::from_ref(radius),
        };
        let finite = centre.iter().chain(rows.iter().flatten()).chain(sizes).all(|v| v.is_finite());
        let unit = rows.iter().all(|&r| (dot(r, r) - 1.0).abs() < 1e-3);
        let square = dot(rows[0], rows[1]).abs() < 1e-3 && dot(rows[0], rows[2]).abs() < 1e-3
            && dot(rows[1], rows[2]).abs() < 1e-3;
        (finite && unit && square && sizes.iter().all(|&s| s > 0.0)).then_some(ZoneShape { centre, rows, form })
    }

    /// How far straight up from `point` the ray meets the mesh's surface:
    /// its faces are two-sided (0.69d's triangle test takes |det|,
    /// 0x20059FC32), so from inside it meets the face it leaves by (a box's
    /// top), from below the one it enters by; None past [`RAY_UP`] or when
    /// the ray misses it (a point above it, or beside it).
    pub fn up(&self, point: [f32; 3]) -> Option<f32> {
        let offset = [point[0] - self.centre[0], point[1] - self.centre[1], point[2] - self.centre[2]];
        let o = self.rows.map(|row| dot(row, offset));
        // the world's up in the mesh's axes
        let d = self.rows.map(|row| row[1]);
        let (near, far) = match self.form {
            Form::Box { half } => both(both(slab(o[0], d[0], half[0])?, slab(o[1], d[1], half[1])?)?,
                                       slab(o[2], d[2], half[2])?)?,
            Form::Cylinder { radius, half_height } => {
                both(round([o[0], o[2]], [d[0], d[2]], radius)?, slab(o[1], d[1], half_height)?)?
            }
            Form::Sphere { radius } => {
                let a = dot(d, d);
                let b = dot(o, d);
                let disc = b * b - a * (dot(o, o) - radius * radius);
                if disc < 0.0 {
                    return None;
                }
                let root = core::f32::math::sqrt(disc);
                ((-b - root) / a, (-b + root) / a)
            }
        };
        let t = if near >= 0.0 { near } else { far };
        (0.0..=RAY_UP).contains(&t).then_some(t)
    }
}

/// A sound environment's place in its map's list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvIndex(u8);

/// Where a point is: in no zone (0.69d's "default") or in an environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Outdoors,
    In(EnvIndex),
}

/// Milliseconds of a bed's fade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FadeMs(u32);

impl FadeMs {
    /// At least 1 ms (0 would be a cut, which the sound thread steps as 1).
    pub fn new(ms: u32) -> FadeMs {
        FadeMs(ms.max(1))
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

/// An environment's background sound and its fades.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bed {
    pub sound: TrackPath,
    pub fade_in: FadeMs,
    pub fade_out: FadeMs,
}

/// A sound environment: its room and its bed, if it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Environment {
    pub room: RoomTag,
    pub bed: Option<Bed>,
}

/// A map's zones: its environments and their boxes.
pub struct Zones {
    environments: BoundedVec<Environment, ENVIRONMENTS_MOST>,
    boxes: BoundedVec<(EnvIndex, ZoneShape), SHAPES_MOST>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooMany;

impl Zones {
    pub const fn new() -> Zones {
        Zones { environments: BoundedVec::new(), boxes: BoundedVec::new() }
    }

    pub fn add(&mut self, environment: Environment) -> Result<EnvIndex, TooMany> {
        let index = self.environments.push(environment).map_err(|_| TooMany)?;
        Ok(EnvIndex(index as u8))
    }

    pub fn add_shape(&mut self, environment: EnvIndex, shape: ZoneShape) -> Result<(), TooMany> {
        self.boxes.push((environment, shape)).map(|_| ()).map_err(|_| TooMany)
    }

    /// How many environments and meshes.
    pub fn counts(&self) -> (usize, usize) {
        (self.environments.len(), self.boxes.len())
    }

    /// 0.69d's lookup (0x20023AF50): a ray straight up from the point, the
    /// nearest box face it meets (two-sided) names the place; none within
    /// 100 m is outdoors. No priority and no smallest-zone rule: nested or
    /// overlapping zones resolve to the surface closest above. A tie goes to
    /// the later box, as 0.69d's `t <= best` keeps the last.
    pub fn place(&self, point: [f32; 3]) -> Place {
        let mut best: Option<(f32, EnvIndex)> = None;
        for &(environment, shape) in self.boxes.iter() {
            if let Some(t) = shape.up(point)
                && best.is_none_or(|(b, _)| t <= b)
            {
                best = Some((t, environment));
            }
        }
        best.map_or(Place::Outdoors, |(_, e)| Place::In(e))
    }

    pub fn environment(&self, place: Place) -> Option<&Environment> {
        match place {
            Place::Outdoors => None,
            Place::In(EnvIndex(i)) => self.environments.get(usize::from(i)),
        }
    }

    /// A place's room: its environment's, or the outdoors'.
    pub fn room(&self, place: Place, outdoors: RoomTag) -> RoomTag {
        self.environment(place).map_or(outdoors, |e| e.room)
    }
}

impl Default for Zones {
    fn default() -> Self {
        Self::new()
    }
}

/// Which way a bed's volume goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toward {
    Full,
    Silence,
}

/// What the beds' machine does in the game.
pub trait Engine {
    type Emitter;
    type Voice;
    /// Asks for a bed's sound; it arrives as [`BedEvent::Arrived`].
    fn load(&mut self, sound: &TrackPath) -> Serial;
    /// The sound playing, looped, in the slot's scene (silent until faded in).
    fn play(&mut self, slot: usize, emitter: &Self::Emitter) -> Option<Self::Voice>;
    /// The slot's volume moves toward `toward` over `ms` from silence to full.
    fn fade(&mut self, slot: usize, toward: Toward, ms: FadeMs);
    /// The slot silent at once.
    fn cut(&mut self, slot: usize);
    /// The slot is silent and stays so (faded out).
    fn silent(&self, slot: usize) -> bool;
}

/// A slot: free, its bed's sound on the way, or sounding (the voice first,
/// so it stops before its sound is let go).
pub enum Slot<E, V> {
    Free,
    Loading { sound: TrackPath, serial: Serial },
    Sounding { sound: TrackPath, _voice: V, _emitter: E },
}

impl<E, V> Slot<E, V> {
    fn sound(&self) -> Option<TrackPath> {
        match self {
            Slot::Free => None,
            Slot::Loading { sound, .. } | Slot::Sounding { sound, .. } => Some(*sound),
        }
    }
}

/// What the beds are told.
pub enum BedEvent<E> {
    /// The listener went from one environment's bed to another's (None: an
    /// environment with none, or outdoors).
    Moved { left: Option<Bed>, entered: Option<Bed> },
    /// A bed's sound arrived (None: its load failed).
    Arrived(Serial, Option<E>),
    /// A frame: a bed faded out is let go.
    Frame,
    /// The map's zones are gone: every bed stops at once.
    Cut,
}

/// The beds: at most [`SLOTS`] at once, each in a scene of its own so that
/// one fades in while the others fade out.
pub struct Beds<E, V> {
    slots: [Slot<E, V>; SLOTS],
    /// The slot each fades toward (the game thread's copy of the mix).
    toward: [Toward; SLOTS],
}

impl<E, V> Beds<E, V> {
    pub const fn new() -> Beds<E, V> {
        Beds { slots: [const { Slot::Free }; SLOTS], toward: [Toward::Silence; SLOTS] }
    }

    fn slot_of(&self, sound: TrackPath) -> Option<usize> {
        self.slots.iter().position(|s| s.sound() == Some(sound))
    }

    /// The one transition function (0.69d's leave/enter, 0x2002A0140 and
    /// 0x20029FF80, run in the same tick: a crossfade).
    pub fn step<G: Engine<Emitter = E, Voice = V>>(&mut self, event: BedEvent<E>, game: &mut G) {
        match event {
            BedEvent::Moved { left, entered } => {
                // leave: the old room's bed fades out over its own fade-out,
                // unless the new room has the same sound (handed over as it
                // plays: no restart, no fade)
                if let Some(old) = left
                    && entered.is_none_or(|new| new.sound != old.sound)
                    && let Some(slot) = self.slot_of(old.sound)
                {
                    self.toward[slot] = Toward::Silence;
                    game.fade(slot, Toward::Silence, old.fade_out);
                }
                // enter: the new room's bed fades in over its fade-in; a bed
                // still fading out of an earlier visit fades back in from
                // where it is
                if let Some(new) = entered {
                    let slot = match self.slot_of(new.sound) {
                        Some(slot) => slot,
                        None => {
                            let slot = self.take_slot(game);
                            let serial = game.load(&new.sound);
                            self.slots[slot] = Slot::Loading { sound: new.sound, serial };
                            slot
                        }
                    };
                    self.toward[slot] = Toward::Full;
                    game.fade(slot, Toward::Full, new.fade_in);
                }
            }
            BedEvent::Arrived(serial, emitter) => {
                let Some(slot) = self.slots.iter().position(|s| matches!(s, Slot::Loading { serial: s, .. } if *s == serial))
                else {
                    return; // nobody waits for it: let go here
                };
                let Slot::Loading { sound, .. } = self.slots[slot] else { return };
                // it plays from silence, at the fade already set; one the
                // listener left before it came is let go
                let playing = match (emitter, self.toward[slot]) {
                    (Some(emitter), Toward::Full) => game.play(slot, &emitter).map(|voice| (voice, emitter)),
                    _ => None,
                };
                self.slots[slot] = match playing {
                    Some((voice, emitter)) => Slot::Sounding { sound, _voice: voice, _emitter: emitter },
                    None => {
                        self.toward[slot] = Toward::Silence;
                        game.cut(slot);
                        Slot::Free
                    }
                };
            }
            BedEvent::Frame => {
                for slot in 0..SLOTS {
                    if matches!(self.slots[slot], Slot::Sounding { .. })
                        && self.toward[slot] == Toward::Silence
                        && game.silent(slot)
                    {
                        self.slots[slot] = Slot::Free;
                    }
                }
            }
            BedEvent::Cut => {
                for slot in 0..SLOTS {
                    self.slots[slot] = Slot::Free;
                    self.toward[slot] = Toward::Silence;
                    game.cut(slot);
                }
            }
        }
    }

    /// A free slot, else the one fading out (the quietest is not known
    /// here: the first fading out), cut; else the first.
    fn take_slot<G: Engine<Emitter = E, Voice = V>>(&mut self, game: &mut G) -> usize {
        let slot = self
            .slots
            .iter()
            .position(|s| matches!(s, Slot::Free))
            .or_else(|| self.toward.iter().position(|t| *t == Toward::Silence))
            .unwrap_or(0);
        self.slots[slot] = Slot::Free;
        game.cut(slot);
        slot
    }
}

impl<E, V> Default for Beds<E, V> {
    fn default() -> Self {
        Self::new()
    }
}

/// The beds' volumes, which the sound thread steps (a fade never waits
/// on a frame): linear, from where a slot is toward its target, at the
/// fade's pace (its ms from silence to full).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mix {
    volume: [f32; SLOTS],
    toward: [Toward; SLOTS],
    ms: [FadeMs; SLOTS],
}

impl Mix {
    pub const fn new() -> Mix {
        Mix { volume: [0.0; SLOTS], toward: [Toward::Silence; SLOTS], ms: [FadeMs(1); SLOTS] }
    }

    pub fn fade(&mut self, slot: usize, toward: Toward, ms: FadeMs) {
        self.toward[slot] = toward;
        self.ms[slot] = ms;
    }

    pub fn cut(&mut self, slot: usize) {
        self.volume[slot] = 0.0;
        self.toward[slot] = Toward::Silence;
    }

    pub fn silent(&self, slot: usize) -> bool {
        self.toward[slot] == Toward::Silence && self.volume[slot] <= 0.0
    }

    /// `elapsed` ms of the sound thread: each slot's volume after them.
    pub fn tick(&mut self, elapsed: u32) -> [f32; SLOTS] {
        for slot in 0..SLOTS {
            let step = elapsed as f32 / self.ms[slot].get() as f32;
            let v = self.volume[slot];
            self.volume[slot] = match self.toward[slot] {
                Toward::Full => (v + step).min(1.0),
                Toward::Silence => (v - step).max(0.0),
            };
        }
        self.volume
    }
}

impl Default for Mix {
    fn default() -> Self {
        Self::new()
    }
}
