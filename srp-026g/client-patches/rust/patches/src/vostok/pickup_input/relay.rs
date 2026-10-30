//! Validated ordinary input shared by targeted and untargeted native relays.
use super::{GameTime, ReceiveError, Refused};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelayInput {
    pub(super) time: GameTime,
    pub(super) rotation: u32,
    pub(super) actions: u32,
    pub(super) precise: bool,
}

impl RelayInput {
    pub fn new(
        time: GameTime,
        rotation: [u8; 4],
        actions: u32,
        precise: bool,
    ) -> Result<Self, ReceiveError> {
        if GameTime::from_raw(time.0).is_none() {
            return Err(ReceiveError::TimeOutsideClock);
        }
        if !weapon_pickup::input::rotation_is_sane(rotation) {
            return Err(ReceiveError::InvalidRotation);
        }
        Ok(Self {
            time,
            rotation: u32::from_le_bytes(rotation),
            actions,
            precise,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "handle native input rejection at the transport boundary"]
pub enum Placement {
    Placed(GameTime),
    Duplicate,
    Refused(Refused),
}
