//! Deterministic state owned by one player's artefact-carry adapter.
//!
//! This crate contains no engine pointers and does not install input hooks.
//! A native adapter must drive `step` on the simulation thread, serialize
//! its snapshot with the world, and implement `Transfer` as one checked
//! inventory transaction. Presentation consumes the state; it cannot change
//! inventory ownership or make an offer available early.

#![no_std]
#![forbid(unsafe_code)]

pub mod carry;
pub mod ground;

/// Artefact identity in the anomaly registry, distinct from an active-item ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtefactId(u8);

impl ArtefactId {
    pub const fn parse(raw: u8) -> Option<Self> {
        if raw == u8::MAX {
            None
        } else {
            Some(Self(raw))
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Ordinal in the separately allocated artefact container array.
/// Never a native address-relative inventory slot or quick-use ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainerIndex(u8);

impl ContainerIndex {
    pub const fn parse(raw: u8) -> Option<Self> {
        if raw < u8::MAX { Some(Self(raw)) } else { None }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Roster place, distinct from either kind of world object ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place(u8);

impl Place {
    pub const fn parse(raw: u8) -> Option<Self> {
        if raw < 20 { Some(Self(raw)) } else { None }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Stable identity and original donor slot of a passive artefact.
/// The engine boundary must obtain these from the same owned inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Artefact {
    pub id: ArtefactId,
    pub slot: ContainerIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    NotOffering,
    StaleOffer,
    SamePlayer,
    NoFreeContainer,
    OwnershipChanged,
    NotInReach,
}
