//! Private reserve counts in the weapon template's compatible-ammo order.
//! No allowance or grant is applied when restoring these values.

use crate::AmmoKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reserves {
    rounds: [u16; 8],
    count: u8,
}

impl Reserves {
    pub fn parse(rounds: &[u16]) -> Option<Self> {
        if rounds.is_empty() || rounds.len() > 8 {
            return None;
        }
        let mut out = Self {
            rounds: [0; 8],
            count: rounds.len() as u8,
        };
        out.rounds[..rounds.len()].copy_from_slice(rounds);
        Some(out)
    }

    pub fn counts(&self) -> &[u16] {
        &self.rounds[..usize::from(self.count)]
    }

    pub fn get(&self, kind: AmmoKind) -> Option<u16> {
        self.counts().get(usize::from(kind.get())).copied()
    }
}
