//! A weapon's scope, muzzle device and grip as 0.69d's own modules in slots
//! (`docs/notes/attachments.md`). In 0.69d a gun is a base mesh with slots,
//! and every other part is a module in a slot, the gun's own brake, forend
//! and sights too; a module's rules are a small vocabulary. This crate is
//! that model's one definition, shared by three builds:
//!
//! - the porter, which writes the table ([`encode`], the `encode` feature)
//!   from 0.69d's data and runs [`resolve`] to build, check and render;
//! - srp's servers, which run [`resolve`] to accept or refuse an attach;
//! - the game itself, in the exe's arena (the patch `attachments`), which
//!   runs it once when a weapon is built and writes what it says.
//!
//! The parts:
//!
//! - [`SlotId`]: 0.69d's slot ids, a closed set (generated);
//! - [`Table`]: the table's bytes parsed once into typed views ([`Gun`],
//!   [`Module`], [`Attach`]); nothing after the parse checks them again;
//! - [`resolve`]: a gun and what a player fitted to it, to what to draw
//!   where, the eye, the fire point, the left hand and the shot effect, or a
//!   [`Refusal`];
//! - [`Placement`], [`Eye`], [`Hand`]: where things are, in the terms the
//!   client's locator records take.
//!
//! What a player fits is one module a [`Kind`]: the weapon kit's slots 3, 4,
//! 5 and 6 (`weapon-kit`), by dict id. Everything else a fit needs (a rail
//! bracket, a tactical forend in place of the gun's own) comes with it.
//!
//! `no_std`, no `alloc`, no `unsafe`, and no read can panic: a panic in the
//! game is a crash.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::arithmetic_side_effects
    )
)]

#[cfg(any(test, feature = "encode"))]
extern crate alloc;
#[cfg(test)]
extern crate std;

#[cfg(any(test, feature = "encode"))]
pub mod encode;
mod math;
mod pose;
mod resolve;
mod slot;
mod table;
#[cfg(test)]
mod tests;

use core::num::NonZeroU16;

pub use math::Affine;
pub use pose::{Eye, EyeAnchor, FINGERS, Hand, Placement, locator_frame};
pub use resolve::{Bare, Loadout, Mount, Mounts, Optic, Part, Refusal, SecondSight, Shape, Side, Wanted, iron_sights, mounts, resolve};
pub use slot::SlotId;
pub use table::{Attach, Gun, GunFault, HAND_LOCATORS, MAGIC, MAX_CHAIN, MAX_STOCK, Malformed, Module, ModuleFault, OLDEST, Table, VERSION};

/// What a player fits to a weapon: one module of each kind at most.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// The weapon kit's slot 3.
    Scope,
    /// Slot 4: a brake, a compensator, a flash hider, a choke.
    Muzzle,
    /// Slot 5.
    Grip,
    /// Slot 6: a collimator on a 45-degree offset rail at the gun's side
    /// (0.69d's NcStar MPR45 with a DeltaPoint Pro or an RSA-S in its
    /// `angled_collim_short_*` slot). Its eye is a second one beside the
    /// scope's ([`crate::Bare::second`]), which the player switches to.
    SecondSight,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Scope, Kind::Muzzle, Kind::Grip, Kind::SecondSight];

    /// Whether its attach must give an eye: an optic is one where it moves
    /// an eye.
    pub fn sees(self) -> bool {
        matches!(self, Kind::Scope | Kind::SecondSight)
    }
}

/// What a module is to a gun.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// A player fits it.
    Fitted(Kind),
    /// A fit brings it: a rail bracket, a clamp, a tactical forend, a barrel.
    Carrier,
    /// The gun's own part (0.69d's `default_modules`).
    Stock,
}

impl Role {
    /// The table's byte.
    pub fn byte(self) -> u8 {
        match self {
            Role::Fitted(Kind::Scope) => 0,
            Role::Fitted(Kind::Muzzle) => 1,
            Role::Fitted(Kind::Grip) => 2,
            Role::Carrier => 3,
            Role::Stock => 4,
            Role::Fitted(Kind::SecondSight) => 5,
        }
    }

    pub fn from_byte(byte: u8) -> Option<Role> {
        match byte {
            0 => Some(Role::Fitted(Kind::Scope)),
            1 => Some(Role::Fitted(Kind::Muzzle)),
            2 => Some(Role::Fitted(Kind::Grip)),
            3 => Some(Role::Carrier),
            4 => Some(Role::Stock),
            5 => Some(Role::Fitted(Kind::SecondSight)),
            _ => None,
        }
    }
}

/// A point of the gun a module moves to its own locator (0.69d's
/// `slots_to_override` of `aim` and `fire_point`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Point {
    /// The eye, the locator whole ([`AttachEye::Own`]).
    Eye,
    FirePoint,
    /// The eye, the locator's line at the gun's own eye plane
    /// ([`AttachEye::InGunPlane`]).
    EyeInGunPlane,
}

impl Point {
    pub fn byte(self) -> u8 {
        match self {
            Point::Eye => 0,
            Point::FirePoint => 1,
            Point::EyeInGunPlane => 2,
        }
    }

    pub fn from_byte(byte: u8) -> Option<Point> {
        match byte {
            0 => Some(Point::Eye),
            1 => Some(Point::FirePoint),
            2 => Some(Point::EyeInGunPlane),
            _ => None,
        }
    }
}

/// Where a module puts the eye: its own `aim` locator, and what of it the
/// eye takes (0.69d's `slots_to_override { id: aim }`, without and with
/// `plane_slot: body_aim`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AttachEye {
    /// The locator whole: a magnified scope brings its eye relief.
    Own(Affine),
    /// The locator's axes and its line of sight, at the plane of the gun's
    /// eye plane ([`Gun::eye_plane`]): a collimator, an iron sight and a rail
    /// mount give the line, and the gun keeps its eye relief.
    InGunPlane(Affine),
}

impl AttachEye {
    /// The module's `aim` locator, in its model.
    pub fn locator(&self) -> &Affine {
        match self {
            AttachEye::Own(locator) | AttachEye::InGunPlane(locator) => locator,
        }
    }
}

/// An item's dict id (the dictionary's, never 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DictId(NonZeroU16);

impl DictId {
    pub fn new(id: u16) -> Option<DictId> {
        NonZeroU16::new(id).map(DictId)
    }

    pub fn get(self) -> u16 {
        self.0.get()
    }
}

/// A bone of a gun's model. One a [`Gun`] gives is below its
/// [`Gun::bones`]; the game's own model may still have fewer, which the
/// client checks where it reads the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BoneIndex(pub u16);
