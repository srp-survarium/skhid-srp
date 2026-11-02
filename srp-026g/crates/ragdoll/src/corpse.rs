//! Each place's corpse as drawn: the state machine, keyed per death, and
//! the pool of ragdolls it draws from.
//!
//! ```text
//!                 seen dead(K)               two frames drawn          at rest 0.4 s, or 5 s
//!     Alive -------------------> Dying{K} -------------------> Ragdoll{K} --------------> Settled{K}
//!       ^                          |  (a slot of the pool)                                 |
//!       |  seen alive (respawn, a rollback or a loaded world that undid the death),       |
//!       +--- or dead(K2), K2 != K (then Dying{K2}) ---------------------------------------+
//! ```
//!
//! The step (`Corpses::step`) runs once a frame on the logic thread after the
//! simulation, from what the current world shows of each place: alive, or
//! dead with a [`DeathKey`] that the world restores on a rollback. So a
//! rollback that undoes a death drops the ragdoll at once, and a death
//! simulated again as the same death (the same key) keeps it.
//!
//! A place is small (its tag and key); the bodies are in a fixed pool of
//! [`POOL`] slots, each slot's own enum the stage of its body (the poses
//! captured, the ragdoll moving, the pose settled). With no slot free the
//! oldest settled corpse gives its slot up (it is drawn as the game draws it
//! again); with none settled a new death keeps the game's clip.
//!
//! A body that goes away while the place's player lives again (its respawn)
//! leaves its dropped gun behind: [`Lone`], the same gun body stepped alone
//! until it rests, then kept where it lies until the place's next death.

use crate::camera::EyeFollow;
use crate::math::{Pose, Vec3};
use crate::ragdoll::{
    Cause, DriverBones, GunShape, HeadwearShape, Knock, Level, LoneGun, Motion, Ragdoll, Skeleton,
    Start, StartReport, driver_bones,
};
use crate::shot::{Hit, RECENT, Shot, ShotHit};
use crate::skeleton::{BONE_COUNT, HEAD, PARTS, Part};

/// The places a match has at most (the ragdoll draws none beyond).
pub const MAX_PLACES: usize = 32;
/// The ragdolls kept at once.
pub const POOL: usize = 20;

/// One death of one place: a value the world restores on a rollback, and
/// that a later death of the same place does not repeat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeathKey(pub u32);

/// A slot of the pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotId(u8);

impl SlotId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// What the current world shows of a place this frame (an empty place is
/// `Alive`: nothing to draw of it).
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "a frame's places, on the stack, no allocation (the arena has none)"
)]
pub enum Seen {
    Alive,
    Dead(Death),
}

/// A death as the step needs it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Death {
    pub key: DeathKey,
    pub cause: Cause,
    /// The gun the player held, if it drops.
    pub gun: Option<GunShape>,
    pub headwear: Option<HeadwearShape>,
    /// The ground the player stood on (its transform's place, up).
    pub stood_on: Option<(Vec3, Vec3)>,
    /// The bullets that hit the player just before its death, the killing
    /// one last (`hit_count` of them).
    pub hits: [Hit; RECENT],
    pub hit_count: usize,
    /// What the player wore and carried, kg on each part.
    pub load: [f32; PARTS],
    /// How fast the player moved just before the death.
    pub velocity: Vec3,
}

impl Death {
    /// A death with no bullet known and nothing worn.
    pub fn new(
        key: DeathKey,
        cause: Cause,
        gun: Option<GunShape>,
        stood_on: Option<(Vec3, Vec3)>,
    ) -> Death {
        Death {
            key,
            cause,
            gun,
            headwear: None,
            stood_on,
            hits: [Hit {
                point: Vec3::ZERO,
                impulse: Vec3::ZERO,
            }; RECENT],
            hit_count: 0,
            load: [0.0; PARTS],
            velocity: Vec3::ZERO,
        }
    }
}

/// A place's corpse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Corpse {
    Alive,
    /// Dead with its body in a slot of the pool (its stage the slot's).
    Body {
        death: DeathKey,
        slot: SlotId,
    },
    /// Dead and drawn as the game draws it (no slot was free, or the pose
    /// could not be read).
    Canned {
        death: DeathKey,
    },
}

/// A slot's body: its stage.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "a fixed pool with no allocation (the arena has none): a stage holds its data inline"
)]
pub enum Slot {
    Free,
    /// The game's clip drawn; its skeletons captured (the last, and the
    /// driver bones of the one before), for the ragdoll's start.
    Dying {
        clip_t: f32,
        captured: Captured,
    },
    /// Moving: the ragdoll, and the clip's driver bones the last draw saw
    /// (the drive's target).
    Ragdoll {
        ragdoll: Ragdoll,
        clip: Option<DriverBones>,
    },
    /// At rest: the ragdoll frozen where it lay (a shot wakes it). `since`
    /// orders the settled ones (the oldest gives its slot up first).
    Settled {
        ragdoll: Ragdoll,
        since: u32,
    },
}

/// What a dying body's draws captured.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "inline in its slot, no allocation"
)]
pub enum Captured {
    Nothing,
    /// One skeleton, `age` s ago.
    One {
        now: Skeleton,
        age: f32,
    },
    /// Two: the last (`age` s ago) and the driver bones of the one before,
    /// `dt` s earlier.
    Two {
        now: Skeleton,
        age: f32,
        before: DriverBones,
        dt: f32,
    },
}

impl Captured {
    fn take(self, skeleton: &Skeleton) -> Captured {
        match self {
            Captured::Nothing => Captured::One {
                now: *skeleton,
                age: 0.0,
            },
            Captured::One { now, age } | Captured::Two { now, age, .. } => Captured::Two {
                now: *skeleton,
                age: 0.0,
                before: driver_bones(&now),
                dt: age,
            },
        }
    }

    /// `dt` s more since the last skeleton was captured.
    fn age_by(&mut self, dt: f32) {
        match self {
            Captured::Nothing => {}
            Captured::One { age, .. } | Captured::Two { age, .. } => *age += dt,
        }
    }
}

/// What a draw did to the game's skeleton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drawn {
    /// Left as the game made it.
    Game,
    /// Replaced by the ragdoll's.
    Ragdoll,
}

impl Slot {
    /// The draw of this slot's body (from `player::mt_draw`, after the
    /// game's skeleton is made, before it is drawn): `skeleton` is the
    /// game's, in world space. Touches nothing but this slot, so the places'
    /// draws may run at once on several threads.
    pub fn draw(&mut self, skeleton: &mut Skeleton) -> Drawn {
        match self {
            Slot::Free => Drawn::Game,
            Slot::Dying { captured, .. } => {
                *captured = captured.take(skeleton);
                Drawn::Game
            }
            Slot::Ragdoll { ragdoll, clip } => {
                *clip = Some(driver_bones(skeleton));
                let (parts, gun) = ragdoll.poses();
                ragdoll.rig().draw(&parts, gun, skeleton);
                Drawn::Ragdoll
            }
            Slot::Settled { ragdoll, .. } => {
                let (parts, gun) = ragdoll.poses();
                ragdoll.rig().draw(&parts, gun, skeleton);
                Drawn::Ragdoll
            }
        }
    }

    /// The draw of this slot's body when the game made no skeleton (mt_draw
    /// makes none for a player holding no item: a battery carrier's corpse
    /// once the simulation drops the battery): a ragdoll's own, every bone
    /// from the bodies; the clip the drive follows stays the last one seen.
    /// A dying body (its clip not captured yet) is left to the game.
    pub fn draw_alone(&self, skeleton: &mut Skeleton) -> Drawn {
        match self {
            Slot::Ragdoll { ragdoll, .. } | Slot::Settled { ragdoll, .. } => {
                let (parts, gun) = ragdoll.poses();
                ragdoll.rig().draw(&parts, gun, skeleton);
                Drawn::Ragdoll
            }
            Slot::Free | Slot::Dying { .. } => Drawn::Game,
        }
    }

    /// The camera's bone on the drawn head (the eye, "Item") and the ground
    /// the head rests on, while a ragdoll draws the body.
    fn eye(&self) -> Option<(Pose, Option<(Vec3, Vec3)>)> {
        let mut skeleton = [Pose::IDENTITY; BONE_COUNT];
        let ragdoll = self.body()?;
        let (parts, gun) = ragdoll.poses();
        ragdoll.rig().draw(&parts, gun, &mut skeleton);
        Some((
            ragdoll.rig().eye(skeleton[HEAD.index()]),
            ragdoll.head_ground(),
        ))
    }

    /// The ragdoll drawn, moving or settled.
    pub fn body(&self) -> Option<&Ragdoll> {
        match self {
            Slot::Ragdoll { ragdoll, .. } | Slot::Settled { ragdoll, .. } => Some(ragdoll),
            Slot::Free | Slot::Dying { .. } => None,
        }
    }
}

/// A place's dropped gun after its body went away (the player respawned,
/// the corpse released): none, still falling or sliding (stepped each
/// frame), or at rest. Any death seen at the place ends it (a new death, or
/// a rollback that brought the body back: its ragdoll starts again).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lone {
    None,
    Moving { death: DeathKey, gun: LoneGun },
    Resting { death: DeathKey, gun: LoneGun },
}

impl Lone {
    /// The gun a body leaves as it goes away: its ragdoll's, moving or
    /// settled; none from a dying body (its ragdoll not started) or a free slot.
    fn left_by(death: DeathKey, slot: &Slot) -> Lone {
        match slot {
            Slot::Ragdoll { ragdoll, .. } => ragdoll.lone_gun().map_or(Lone::None, |gun| Lone::Moving { death, gun }),
            Slot::Settled { ragdoll, .. } => ragdoll.lone_gun().map_or(Lone::None, |gun| Lone::Resting { death, gun }),
            Slot::Free | Slot::Dying { .. } => Lone::None,
        }
    }
}

/// The local player's death camera: off, or following its place's head.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Eye {
    Off,
    Following {
        place: usize,
        death: DeathKey,
        follow: EyeFollow,
        eye: Pose,
    },
}

/// A frame as the step sees it.
pub struct Frame<'a> {
    /// The frame's real time, s (capped by the ragdoll).
    pub dt: f32,
    /// The level's up (unit).
    pub up: Vec3,
    /// Each place, in place order (at most MAX_PLACES are drawn).
    pub places: &'a [Seen],
    /// The local player's place, if it has one.
    pub local: Option<usize>,
}

pub struct Corpses {
    places: [Corpse; MAX_PLACES],
    pool: [Slot; POOL],
    eye: Eye,
    /// Counts the settles (orders the settled slots).
    settles: u32,
    /// The ragdolls the last step started: their places and what they
    /// started with (the opt-in diagnostics read them).
    started: [Option<(u8, StartReport)>; 8],
    /// Each place's drawn ragdoll's box as the last step left it: the
    /// shots' broad phase (a shot is traced between two steps).
    bounds: [Option<(Vec3, Vec3)>; MAX_PLACES],
    /// Each place's gun left by its body (its player respawned).
    lone: [Lone; MAX_PLACES],
}

/// A shot through a corpse: the place, its death, where the shot met it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CorpseHit {
    pub place: usize,
    pub death: DeathKey,
    pub hit: ShotHit,
}

impl Corpses {
    pub const fn new() -> Corpses {
        Corpses {
            places: [Corpse::Alive; MAX_PLACES],
            pool: [Slot::Free; POOL],
            eye: Eye::Off,
            settles: 0,
            bounds: [None; MAX_PLACES],
            started: [None; 8],
            lone: [Lone::None; MAX_PLACES],
        }
    }

    /// The ragdolls the last step started (a place and what it started with).
    pub fn started(&self) -> impl Iterator<Item = (usize, StartReport)> + '_ {
        self.started.iter().flatten().map(|&(p, r)| (p as usize, r))
    }

    /// Whether any place has a ragdoll a shot could meet.
    pub fn any_bodies(&self) -> bool {
        self.bounds.iter().any(Option::is_some)
    }

    /// The nearest drawn ragdoll part (moving or settled) the shot meets:
    /// each place's box first, then the capsules of those it crosses. Reads
    /// only, so several threads may trace at once between two steps.
    pub fn trace(&self, shot: &Shot) -> Option<CorpseHit> {
        let mut best: Option<CorpseHit> = None;
        for place in 0..MAX_PLACES {
            let Some((lo, hi)) = self.bounds[place] else {
                continue;
            };
            if !shot.crosses_box(lo, hi) {
                continue;
            }
            let Corpse::Body { death, slot } = self.places[place] else {
                continue;
            };
            let Some(hit) = self.pool[slot.index()].body().and_then(|r| r.trace(shot)) else {
                continue;
            };
            if best.is_none_or(|b| hit.distance < b.hit.distance) {
                best = Some(CorpseHit { place, death, hit });
            }
        }
        best
    }

    /// A shot's push on a place's ragdoll part, if that place still has the
    /// same death's ragdoll drawn: a settled one wakes. What the push did,
    /// None if nothing was pushed.
    pub fn push(
        &mut self,
        place: usize,
        death: DeathKey,
        part: Part,
        point: Vec3,
        j: Vec3,
    ) -> Option<Knock> {
        let Some(Corpse::Body { death: d, slot }) = self.places.get(place).copied() else {
            return None;
        };
        if d != death {
            return None;
        }
        let current = &mut self.pool[slot.index()];
        match current {
            Slot::Ragdoll { ragdoll, .. } => ragdoll.push(part, point, j),
            Slot::Settled { ragdoll, .. } => {
                let mut ragdoll = *ragdoll;
                ragdoll.wake();
                let knock = ragdoll.push(part, point, j);
                *current = Slot::Ragdoll {
                    ragdoll,
                    clip: None,
                };
                knock
            }
            Slot::Free | Slot::Dying { .. } => None,
        }
    }

    /// The gun a place's body left as it went away (its player respawned):
    /// the death it dropped at and the `Weapon` bone's world pose, where the
    /// game would draw the held item; still moving or at rest.
    pub fn lone_gun(&self, place: usize) -> Option<(DeathKey, Pose)> {
        match self.lone.get(place)? {
            Lone::Moving { death, gun } | Lone::Resting { death, gun } => Some((*death, gun.weapon())),
            Lone::None => None,
        }
    }

    pub fn place(&self, place: usize) -> Corpse {
        self.places.get(place).copied().unwrap_or(Corpse::Alive)
    }

    pub fn slot(&self, slot: SlotId) -> &Slot {
        &self.pool[slot.index()]
    }

    /// The slot of a place's body, mutably (the draw's).
    pub fn slot_mut(&mut self, slot: SlotId) -> &mut Slot {
        &mut self.pool[slot.index()]
    }

    /// The local player's camera this frame, if it follows a ragdoll.
    pub fn eye(&self) -> Option<Pose> {
        match self.eye {
            Eye::Following { eye, .. } => Some(eye),
            Eye::Off => None,
        }
    }

    /// The step: every place's corpse from what the world shows of it, the
    /// moving ragdolls stepped, the local player's camera followed.
    pub fn step(&mut self, frame: &Frame<'_>, level: &mut impl Level) {
        self.started = [None; 8];
        let count = frame.places.len().min(MAX_PLACES);
        // first every corpse the world no longer shows (its slot freed),
        // so the new deaths find them
        for place in 0..MAX_PLACES {
            let seen = if place < count {
                frame.places[place]
            } else {
                Seen::Alive
            };
            let keep = match (self.places[place], seen) {
                (Corpse::Alive, _) => true,
                (Corpse::Body { death, .. } | Corpse::Canned { death }, Seen::Dead(d)) => {
                    death == d.key
                }
                (_, Seen::Alive) => false,
            };
            // the body goes away while its player lives again: its gun stays
            self.lone[place] = match (self.places[place], seen) {
                (_, Seen::Dead(_)) => Lone::None,
                (Corpse::Body { death, slot }, Seen::Alive) => Lone::left_by(death, &self.pool[slot.index()]),
                (Corpse::Alive | Corpse::Canned { .. }, Seen::Alive) => self.lone[place],
            };
            if !keep {
                self.release(place);
            }
        }
        for lone in self.lone.iter_mut() {
            if let Lone::Moving { death, mut gun } = *lone {
                *lone = match gun.step(frame.dt, frame.up, level) {
                    Motion::Moving => Lone::Moving { death, gun },
                    Motion::Settled => Lone::Resting { death, gun },
                };
            }
        }
        for place in 0..count {
            if let (Corpse::Alive, Seen::Dead(death)) = (self.places[place], frame.places[place]) {
                self.places[place] = match self.take_slot() {
                    Some(slot) => {
                        self.pool[slot.index()] = Slot::Dying {
                            clip_t: 0.0,
                            captured: Captured::Nothing,
                        };
                        Corpse::Body {
                            death: death.key,
                            slot,
                        }
                    }
                    None => Corpse::Canned { death: death.key },
                };
            }
            if let (Corpse::Body { slot, .. }, Seen::Dead(death)) =
                (self.places[place], frame.places[place])
            {
                self.advance(place, slot, death, frame, level);
            }
        }
        self.follow(frame);
        for place in 0..MAX_PLACES {
            self.bounds[place] = match self.places[place] {
                Corpse::Body { slot, .. } => self.pool[slot.index()].body().map(Ragdoll::bounds),
                Corpse::Alive | Corpse::Canned { .. } => None,
            };
        }
    }

    fn release(&mut self, place: usize) {
        if let Corpse::Body { slot, .. } = self.places[place] {
            self.pool[slot.index()] = Slot::Free;
        }
        self.places[place] = Corpse::Alive;
        if matches!(self.eye, Eye::Following { place: p, .. } if p == place) {
            self.eye = Eye::Off;
        }
    }

    /// A free slot; else the oldest settled one's, its place drawn as the
    /// game draws it from then on.
    fn take_slot(&mut self) -> Option<SlotId> {
        if let Some(i) = self.pool.iter().position(|s| matches!(s, Slot::Free)) {
            return Some(SlotId(i as u8));
        }
        let oldest = (0..MAX_PLACES)
            .filter_map(|place| match self.places[place] {
                Corpse::Body { death, slot } => match self.pool[slot.index()] {
                    Slot::Settled { since, .. } => Some((since, place, death, slot)),
                    _ => None,
                },
                _ => None,
            })
            .min_by_key(|&(since, ..)| since)?;
        let (_, place, death, slot) = oldest;
        self.places[place] = Corpse::Canned { death };
        if matches!(self.eye, Eye::Following { place: p, .. } if p == place) {
            self.eye = Eye::Off;
        }
        self.pool[slot.index()] = Slot::Free;
        Some(slot)
    }

    fn advance(
        &mut self,
        place: usize,
        slot: SlotId,
        death: Death,
        frame: &Frame<'_>,
        level: &mut impl Level,
    ) {
        // in place: a slot is a few kB, copied only when its stage changes
        let current = &mut self.pool[slot.index()];
        let next = match current {
            Slot::Dying {
                captured:
                    Captured::Two {
                        now,
                        age,
                        before,
                        dt,
                    },
                ..
            } => {
                let start = Start {
                    now,
                    before,
                    dt: *dt,
                    up: frame.up,
                    cause: death.cause,
                    gun: death.gun,
                    headwear: death.headwear,
                    stood_on: death.stood_on,
                    hits: &death.hits[..death.hit_count.min(RECENT)],
                    load: death.load,
                    velocity: death.velocity,
                };
                let since = *age + frame.dt;
                Some(match Ragdoll::new(&start) {
                    Some(mut ragdoll) => {
                        if let Some(free) = self.started.iter_mut().find(|s| s.is_none()) {
                            *free = Some((place as u8, ragdoll.report()));
                        }
                        // the time since that skeleton was drawn
                        match ragdoll.step(since, None, level) {
                            Motion::Moving => Slot::Ragdoll {
                                ragdoll,
                                clip: None,
                            },
                            Motion::Settled => settled(&ragdoll, &mut self.settles),
                        }
                    }
                    None => {
                        self.places[place] = Corpse::Canned { death: death.key };
                        Slot::Free
                    }
                })
            }
            Slot::Dying { clip_t, captured } => {
                *clip_t += frame.dt;
                captured.age_by(frame.dt);
                None
            }
            Slot::Ragdoll { ragdoll, clip } => match ragdoll.step(frame.dt, clip.as_ref(), level) {
                Motion::Moving => None,
                Motion::Settled => Some(settled(ragdoll, &mut self.settles)),
            },
            Slot::Settled { .. } | Slot::Free => None,
        };
        if let Some(next) = next {
            self.pool[slot.index()] = next;
        }
    }

    /// The local player's camera: on its own ragdoll's head.
    fn follow(&mut self, frame: &Frame<'_>) {
        let target = frame.local.and_then(|place| match self.place(place) {
            Corpse::Body { death, slot } => {
                self.pool[slot.index()].eye().map(|eye| (place, death, eye))
            }
            Corpse::Alive | Corpse::Canned { .. } => None,
        });
        self.eye = match (self.eye, target) {
            (_, None) => Eye::Off,
            (
                Eye::Following {
                    place,
                    death,
                    mut follow,
                    ..
                },
                Some((p, d, (raw, ground))),
            ) if place == p && death == d => {
                let eye = follow.follow(raw, frame.up, ground, frame.dt);
                Eye::Following {
                    place,
                    death,
                    follow,
                    eye,
                }
            }
            (_, Some((place, death, (raw, ground)))) => {
                let mut follow = EyeFollow::new(raw, frame.up);
                let eye = follow.follow(raw, frame.up, ground, 0.0);
                Eye::Following {
                    place,
                    death,
                    follow,
                    eye,
                }
            }
        };
    }
}

/// The settled slot of a ragdoll at rest: its pose frozen, numbered by the
/// settles' count.
fn settled(ragdoll: &Ragdoll, settles: &mut u32) -> Slot {
    *settles = settles.wrapping_add(1);
    Slot::Settled {
        ragdoll: *ragdoll,
        since: *settles,
    }
}

const _: () = assert!(POOL <= u8::MAX as usize && PARTS == 11);

#[cfg(test)]
mod tests;
