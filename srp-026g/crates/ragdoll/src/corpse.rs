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

use crate::camera::EyeFollow;
use crate::math::{Pose, Vec3};
use crate::ragdoll::{
    Cause, DriverBones, GunShape, Level, Motion, PartPoses, Ragdoll, Rig, Skeleton, Start, driver_bones,
};
use crate::skeleton::{BONE_COUNT, HEAD, PARTS};

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
    /// The ground the player stood on (its transform's place, up).
    pub stood_on: Option<(Vec3, Vec3)>,
}

/// A place's corpse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Corpse {
    Alive,
    /// Dead with its body in a slot of the pool (its stage the slot's).
    Body { death: DeathKey, slot: SlotId },
    /// Dead and drawn as the game draws it (no slot was free, or the pose
    /// could not be read).
    Canned { death: DeathKey },
}

/// A slot's body: its stage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
    Free,
    /// The game's clip drawn; its skeletons captured (the last, and the
    /// driver bones of the one before), for the ragdoll's start.
    Dying { clip_t: f32, captured: Captured },
    /// Moving: the ragdoll, and the clip's driver bones the last draw saw
    /// (the drive's target).
    Ragdoll { ragdoll: Ragdoll, clip: Option<DriverBones> },
    /// At rest: the pose, frozen. `since` orders the settled ones (the
    /// oldest gives its slot up first).
    Settled { rig: Rig, parts: PartPoses, gun: Option<Pose>, head_ground: Option<(Vec3, Vec3)>, since: u32 },
}

/// What a dying body's draws captured.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Captured {
    Nothing,
    /// One skeleton, `age` s ago.
    One { now: Skeleton, age: f32 },
    /// Two: the last (`age` s ago) and the driver bones of the one before,
    /// `dt` s earlier.
    Two { now: Skeleton, age: f32, before: DriverBones, dt: f32 },
}

impl Captured {
    fn take(self, skeleton: &Skeleton) -> Captured {
        match self {
            Captured::Nothing => Captured::One { now: *skeleton, age: 0.0 },
            Captured::One { now, age } | Captured::Two { now, age, .. } => {
                Captured::Two { now: *skeleton, age: 0.0, before: driver_bones(&now), dt: age }
            }
        }
    }

    fn older(self, dt: f32) -> Captured {
        match self {
            Captured::Nothing => Captured::Nothing,
            Captured::One { now, age } => Captured::One { now, age: age + dt },
            Captured::Two { now, age, before, dt: between } => Captured::Two { now, age: age + dt, before, dt: between },
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
            Slot::Settled { rig, parts, gun, .. } => {
                rig.draw(parts, *gun, skeleton);
                Drawn::Ragdoll
            }
        }
    }

    /// The camera's bone on the drawn head (the eye, "Item") and the ground
    /// the head rests on, while a ragdoll draws the body.
    fn eye(&self) -> Option<(Pose, Option<(Vec3, Vec3)>)> {
        let mut skeleton = [Pose::IDENTITY; BONE_COUNT];
        match self {
            Slot::Ragdoll { ragdoll, .. } => {
                let (parts, gun) = ragdoll.poses();
                ragdoll.rig().draw(&parts, gun, &mut skeleton);
                Some((ragdoll.rig().eye(skeleton[HEAD.index()]), ragdoll.head_ground()))
            }
            Slot::Settled { rig, parts, gun, head_ground, .. } => {
                rig.draw(parts, *gun, &mut skeleton);
                Some((rig.eye(skeleton[HEAD.index()]), *head_ground))
            }
            Slot::Free | Slot::Dying { .. } => None,
        }
    }
}

/// The local player's death camera: off, or following its place's head.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Eye {
    Off,
    Following { place: usize, death: DeathKey, follow: EyeFollow, eye: Pose },
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
}

impl Corpses {
    pub const fn new() -> Corpses {
        Corpses { places: [Corpse::Alive; MAX_PLACES], pool: [Slot::Free; POOL], eye: Eye::Off, settles: 0 }
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

    /// A slot of the corpses at `this`, as a pointer: the draws of several
    /// places run at once on several threads, each with its own slot, so no
    /// reference to the whole is made.
    ///
    /// # Safety
    /// `this` points to a live `Corpses`; the caller is the slot's only user
    /// while it holds the pointer.
    pub unsafe fn slot_raw(this: *mut Corpses, slot: SlotId) -> *mut Slot {
        unsafe { &raw mut (*this).pool[slot.index() % POOL] }
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
        let count = frame.places.len().min(MAX_PLACES);
        // first every corpse the world no longer shows (its slot freed),
        // so the new deaths find them
        for place in 0..MAX_PLACES {
            let seen = if place < count { frame.places[place] } else { Seen::Alive };
            let keep = match (self.places[place], seen) {
                (Corpse::Alive, _) => true,
                (Corpse::Body { death, .. } | Corpse::Canned { death }, Seen::Dead(d)) => death == d.key,
                (_, Seen::Alive) => false,
            };
            if !keep {
                self.release(place);
            }
        }
        for place in 0..count {
            if let (Corpse::Alive, Seen::Dead(death)) = (self.places[place], frame.places[place]) {
                self.places[place] = match self.take_slot() {
                    Some(slot) => {
                        self.pool[slot.index()] = Slot::Dying { clip_t: 0.0, captured: Captured::Nothing };
                        Corpse::Body { death: death.key, slot }
                    }
                    None => Corpse::Canned { death: death.key },
                };
            }
            if let (Corpse::Body { slot, .. }, Seen::Dead(death)) = (self.places[place], frame.places[place]) {
                self.advance(place, slot, death, frame, level);
            }
        }
        self.follow(frame);
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

    fn advance(&mut self, place: usize, slot: SlotId, death: Death, frame: &Frame<'_>, level: &mut impl Level) {
        let next = match self.pool[slot.index()] {
            Slot::Dying { captured: Captured::Two { now, age, before, dt }, .. } => {
                let start = Start {
                    now: &now,
                    before: &before,
                    dt,
                    up: frame.up,
                    cause: death.cause,
                    gun: death.gun,
                    stood_on: death.stood_on,
                };
                match Ragdoll::new(&start) {
                    Some(mut ragdoll) => {
                        // the time since that skeleton was drawn
                        let motion = ragdoll.step(age + frame.dt, None, level);
                        settle_or_move(ragdoll, motion, None, &mut self.settles)
                    }
                    None => {
                        self.places[place] = Corpse::Canned { death: death.key };
                        Slot::Free
                    }
                }
            }
            Slot::Dying { clip_t, captured } => Slot::Dying { clip_t: clip_t + frame.dt, captured: captured.older(frame.dt) },
            Slot::Ragdoll { mut ragdoll, clip } => {
                let motion = ragdoll.step(frame.dt, clip.as_ref(), level);
                settle_or_move(ragdoll, motion, clip, &mut self.settles)
            }
            settled @ (Slot::Settled { .. } | Slot::Free) => settled,
        };
        self.pool[slot.index()] = next;
    }

    /// The local player's camera: on its own ragdoll's head.
    fn follow(&mut self, frame: &Frame<'_>) {
        let target = frame.local.and_then(|place| match self.place(place) {
            Corpse::Body { death, slot } => self.pool[slot.index()].eye().map(|eye| (place, death, eye)),
            Corpse::Alive | Corpse::Canned { .. } => None,
        });
        self.eye = match (self.eye, target) {
            (_, None) => Eye::Off,
            (Eye::Following { place, death, mut follow, .. }, Some((p, d, (raw, ground)))) if place == p && death == d => {
                let eye = follow.follow(raw, frame.up, ground, frame.dt);
                Eye::Following { place, death, follow, eye }
            }
            (_, Some((place, death, (raw, ground)))) => {
                let mut follow = EyeFollow::new(raw, frame.up);
                let eye = follow.follow(raw, frame.up, ground, 0.0);
                Eye::Following { place, death, follow, eye }
            }
        };
    }
}

fn settle_or_move(ragdoll: Ragdoll, motion: Motion, clip: Option<DriverBones>, settles: &mut u32) -> Slot {
    match motion {
        Motion::Moving => Slot::Ragdoll { ragdoll, clip },
        Motion::Settled => {
            *settles = settles.wrapping_add(1);
            let (parts, gun) = ragdoll.poses();
            Slot::Settled { rig: *ragdoll.rig(), parts, gun, head_ground: ragdoll.head_ground(), since: *settles }
        }
    }
}

const _: () = assert!(POOL <= u8::MAX as usize && PARTS == 11);

#[cfg(test)]
mod tests;
