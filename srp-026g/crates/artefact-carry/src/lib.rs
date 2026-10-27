//! Deterministic state owned by one player's artefact-carry adapter.
//!
//! This crate contains no engine pointers and does not install input hooks.
//! A native adapter must drive `step` on the simulation thread, serialize
//! its snapshot with the world, and implement `Transfer` as one checked
//! inventory transaction. Presentation consumes the state; it cannot change
//! inventory ownership or make an offer available early.

#![no_std]
#![forbid(unsafe_code)]

/// A registered active-object ID. Native 255 means absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveObjectId(u8);

impl ActiveObjectId {
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

/// An interaction names a particular offer, not merely a player address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offer {
    pub artefact: Artefact,
    generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Stowed,
    /// Native weapon hide has not finished. The artefact cannot be taken.
    Equipping {
        offer: Offer,
        previous: ActiveObjectId,
    },
    Offering {
        offer: Offer,
        previous: ActiveObjectId,
    },
    /// The offer is gone; finish returning to the previous active item.
    Unequipping {
        previous: ActiveObjectId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The boundary has checked that this artefact is passive and owned.
    Equip {
        artefact: Artefact,
        previous: ActiveObjectId,
    },
    /// Confirmed by the native active-item animation/state transition.
    Equipped,
    Cancel,
    /// Confirmed after the native previous item is active again.
    Unequipped,
    Take {
        offer: Offer,
        recipient: Place,
    },
    /// Native death/disconnect teardown, never an inventory transfer.
    OwnerGone,
}

/// One native action to perform after the state transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    EquipAdapter,
    Present(Artefact),
    Restore(ActiveObjectId),
    RemovePresentation,
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

/// The native implementation owns capacity checking, the retained item
/// reference, passive unlink/relink, slot writes and dirty flags together.
/// On error it must leave both inventories unchanged. `Ok` transfers the
/// existing resource exactly once; it must never clone it or evict a slot.
pub trait Transfer {
    fn take(&mut self, donor: Place, recipient: Place, artefact: Artefact) -> Result<(), Refused>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub action: Action,
    pub refusal: Option<Refused>,
}

impl Outcome {
    fn action(action: Action) -> Self {
        Self {
            action,
            refusal: None,
        }
    }
    fn refused(why: Refused) -> Self {
        Self {
            action: Action::None,
            refusal: Some(why),
        }
    }
}

/// One owner for offer state. The immutable owner and registered adapter
/// ID are bound by native construction, not trusted from snapshot bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Adapter {
    owner: Place,
    id: ActiveObjectId,
    generation: u32,
    state: State,
}

impl Adapter {
    pub const fn registered(owner: Place, id: ActiveObjectId) -> Self {
        Self {
            owner,
            id,
            generation: 0,
            state: State::Stowed,
        }
    }

    pub const fn state(&self) -> State {
        self.state
    }

    pub const fn offer(&self) -> Option<Offer> {
        match self.state {
            State::Offering { offer, .. } => Some(offer),
            _ => None,
        }
    }

    /// A single mutation path for initialization, interaction and teardown.
    /// All otherwise inapplicable/repeated events are explicit no-ops.
    pub fn step(&mut self, event: Event, transfer: &mut impl Transfer) -> Outcome {
        let (state, action) = match (self.state, event) {
            (_, Event::OwnerGone) => (State::Stowed, Action::RemovePresentation),
            (State::Stowed, Event::Equip { artefact, previous }) if previous != self.id => {
                self.generation = self.generation.wrapping_add(1);
                let offer = Offer {
                    artefact,
                    generation: self.generation,
                };
                (State::Equipping { offer, previous }, Action::EquipAdapter)
            }
            (State::Equipping { offer, previous }, Event::Equipped) => (
                State::Offering { offer, previous },
                Action::Present(offer.artefact),
            ),
            (
                State::Equipping { previous, .. } | State::Offering { previous, .. },
                Event::Cancel,
            ) => (State::Unequipping { previous }, Action::Restore(previous)),
            (State::Unequipping { .. }, Event::Unequipped) => (State::Stowed, Action::None),
            (
                State::Offering { offer, previous },
                Event::Take {
                    offer: request,
                    recipient,
                },
            ) => {
                if request != offer {
                    return Outcome::refused(Refused::StaleOffer);
                }
                if recipient == self.owner {
                    return Outcome::refused(Refused::SamePlayer);
                }
                if let Err(error) = transfer.take(self.owner, recipient, offer.artefact) {
                    return Outcome::refused(error);
                }
                (State::Unequipping { previous }, Action::Restore(previous))
            }
            (_, Event::Take { .. }) => return Outcome::refused(Refused::NotOffering),
            (state, _) => (state, Action::None),
        };
        self.state = state;
        Outcome::action(action)
    }

    /// Version, generation, phase, and exactly the fields of that phase.
    /// Unused trailing bytes are canonical zeroes, never hidden state.
    pub fn snapshot(&self) -> [u8; 9] {
        let mut bytes = [0; 9];
        bytes[0] = 1;
        bytes[1..5].copy_from_slice(&self.generation.to_le_bytes());
        match self.state {
            State::Stowed => {}
            State::Equipping { offer, previous } | State::Offering { offer, previous } => {
                bytes[5] = if matches!(self.state, State::Equipping { .. }) {
                    1
                } else {
                    2
                };
                bytes[6] = offer.artefact.id.get();
                bytes[7] = offer.artefact.slot.get();
                bytes[8] = previous.get();
            }
            State::Unequipping { previous } => {
                bytes[5] = 3;
                bytes[8] = previous.get();
            }
        }
        bytes
    }

    /// Parse fully before mutating the adapter. A rejected snapshot cannot
    /// leave an old native presentation paired with half of a new state.
    /// The caller must reconcile native presentation after a successful
    /// restore, including a restore to Stowed.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), InvalidSnapshot> {
        let &[version, a, b, c, d, phase, item, slot, previous] = bytes else {
            return Err(InvalidSnapshot);
        };
        if version != 1 {
            return Err(InvalidSnapshot);
        }
        let generation = u32::from_le_bytes([a, b, c, d]);
        let state = match phase {
            0 if item == 0 && slot == 0 && previous == 0 => State::Stowed,
            1 | 2 => {
                let artefact = Artefact {
                    id: ArtefactId::parse(item).ok_or(InvalidSnapshot)?,
                    slot: ContainerIndex::parse(slot).ok_or(InvalidSnapshot)?,
                };
                let previous = ActiveObjectId::parse(previous).ok_or(InvalidSnapshot)?;
                if previous == self.id {
                    return Err(InvalidSnapshot);
                }
                let offer = Offer {
                    artefact,
                    generation,
                };
                if phase == 1 {
                    State::Equipping { offer, previous }
                } else {
                    State::Offering { offer, previous }
                }
            }
            3 if item == 0 && slot == 0 => {
                let previous = ActiveObjectId::parse(previous).ok_or(InvalidSnapshot)?;
                if previous == self.id {
                    return Err(InvalidSnapshot);
                }
                State::Unequipping { previous }
            }
            _ => return Err(InvalidSnapshot),
        };
        self.generation = generation;
        self.state = state;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidSnapshot;

#[cfg(test)]
mod tests;
