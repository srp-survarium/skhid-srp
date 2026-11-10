//! A battery chosen by the initiating client's local use scan. Replicas
//! resolve its id and availability, never a second spatial scan.
use crate::{CHORD, QUICK, USE};

pub const COUNT: usize = 10;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Id(u8);
impl Id {
    pub fn new(value: usize) -> Option<Self> { (value < COUNT).then_some(Self(value as u8)) }
    pub fn index(self) -> usize { usize::from(self.0) }
}

// The next ten multi-quick-bit patterns after weapon commands. One bit
// remains an artefact command; the first twenty remain weapon pickups.
const PATTERNS: [u8; COUNT] = {
    let mut out = [0; COUNT];
    let (mut ordinal, mut value) = (0usize, 0u8);
    while ordinal < 30 {
        if value.count_ones() >= 2 {
            if ordinal >= 20 { out[ordinal - 20] = value; }
            ordinal += 1;
        }
        value += 1;
    }
    out
};

pub fn command(actions: u32) -> Option<Id> {
    if actions & CHORD == 0 { return None; }
    let quick = ((actions & QUICK) >> 14) as u8;
    PATTERNS.iter().position(|&p| p == quick).and_then(Id::new)
}

pub fn encode(actions: u32, target: Id) -> u32 {
    (actions & !(QUICK | USE)) | CHORD | (u32::from(PATTERNS[target.index()]) << 14)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_batteries_round_trip_without_becoming_weapon_or_artefact_commands() {
        for n in 0..COUNT {
            let id = Id::new(n).unwrap();
            let mask = encode(USE | 0x20, id);
            assert_eq!(command(mask), Some(id));
            assert_eq!(crate::command(mask), None);
            assert_eq!(mask & USE, 0);
            assert_ne!(mask & 0x20, 0);
            assert!((mask & QUICK).count_ones() >= 2);
        }
        for n in 0..crate::PLACES {
            assert_eq!(command(crate::encode(0, crate::Place::new(n).unwrap())), None);
        }
        for n in 0..6 { assert_eq!(command(CHORD | 1 << (14+n)), None); }
        assert_eq!(command(USE), None);
        assert_eq!(Id::new(COUNT), None);
    }
}
