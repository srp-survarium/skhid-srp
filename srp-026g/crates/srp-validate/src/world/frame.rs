//! Semantic views over the shipped world-frame parser. Framing has one owner.
use crate::{Part, Reason, Refusal};
pub use world_frame::{PLAYER_STATE_CAP, SectionBytes as Section};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block<'a> {
    pub place: u8,
    pub at: usize,
    pub state: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame<'a> {
    body: &'a [u8],
    native: world_frame::Frame<'a>,
}

impl<'a> Frame<'a> {
    /// The same parsed frame used by weapon-pickup preparation on the client.
    pub fn native(&self) -> &world_frame::Frame<'a> {
        &self.native
    }

    pub fn blocks(&self) -> impl Iterator<Item = Block<'a>> + '_ {
        self.native.states().map(|(slot, state)| Block {
            place: slot.index(),
            at: (state.as_ptr() as usize).saturating_sub(self.body.as_ptr() as usize),
            state,
        })
    }

    pub fn has_block(&self, place: u8) -> bool {
        self.native
            .slots()
            .mask()
            .checked_shr(u32::from(place))
            .unwrap_or(0)
            & 1
            != 0
    }

    pub fn sections(&self) -> [Section<'a>; 6] {
        world_frame::Section::ALL.map(|section| self.native.section(section))
    }

    pub fn bullets(&self) -> Section<'a> {
        self.native.section(world_frame::Section::Bullets)
    }
    pub fn statistics(&self) -> Section<'a> {
        self.native.section(world_frame::Section::Statistics)
    }
    pub fn objects(&self) -> Section<'a> {
        self.native.section(world_frame::Section::Objects)
    }
    pub fn physics(&self) -> Section<'a> {
        self.native.section(world_frame::Section::Physics)
    }
    pub fn rules(&self) -> Section<'a> {
        self.native.section(world_frame::Section::Rules)
    }
    pub fn effects(&self) -> Section<'a> {
        self.native.section(world_frame::Section::Effects)
    }
}

pub fn frame(body: &[u8], places: usize) -> Result<Frame<'_>, Refusal> {
    world_frame::Frame::parse(body, world_frame::Places::of(places))
        .map(|native| Frame { body, native })
        .map_err(|error| {
            use world_frame::Refusal as Error;
            let part = match error.part() {
                world_frame::Part::Player(slot) => Part::Player(slot.index()),
                world_frame::Part::Section(section) => match section {
                    world_frame::Section::Bullets => Part::Bullets,
                    world_frame::Section::Statistics => Part::Statistics,
                    world_frame::Section::Objects => Part::Objects,
                    world_frame::Section::Physics => Part::Physics,
                    world_frame::Section::Rules => Part::Rules,
                    world_frame::Section::Effects => Part::Effects,
                },
                _ => Part::Frame,
            };
            let reason = match error {
                Error::SlotPastPlaces { bit, places } => Reason::PlaceOutside {
                    place: usize::from(bit),
                    places: usize::from(places.count()),
                },
                Error::StateOverCap { len, .. } => Reason::StateOverCap { len },
                Error::OverBuffer { len } => Reason::BodyOverBuffer { len },
                Error::Trailing { .. } => Reason::NoSections,
                _ => Reason::NoWalk,
            };
            Refusal::new(part, error.at(), reason)
        })
}
