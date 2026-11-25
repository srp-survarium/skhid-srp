//! Successful activations of one combat artefact since its anomaly spawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ArtefactWear {
    #[default]
    Fresh,
    Once,
    Twice,
    ThreeUses,
    FourUses,
    Spent,
}

impl ArtefactWear {
    pub fn new(uses: u8) -> Option<Self> {
        Some(match uses {
            0 => Self::Fresh,
            1 => Self::Once,
            2 => Self::Twice,
            3 => Self::ThreeUses,
            4 => Self::FourUses,
            5 => Self::Spent,
            _ => return None,
        })
    }

    pub fn uses(self) -> u8 { self as u8 }

    pub fn after_use(self) -> Self {
        match self {
            Self::Fresh => Self::Once,
            Self::Once => Self::Twice,
            Self::Twice => Self::ThreeUses,
            Self::ThreeUses => Self::FourUses,
            Self::FourUses | Self::Spent => Self::Spent,
        }
    }

    /// The last activation uses the strength it had before paying for that use.
    /// Active snapshots from before wear have full strength.
    pub fn firing(self) -> Self {
        match self {
            Self::Fresh | Self::Once => Self::Fresh,
            Self::Twice => Self::Once,
            Self::ThreeUses => Self::Twice,
            Self::FourUses => Self::ThreeUses,
            Self::Spent => Self::FourUses,
        }
    }

    pub fn power(self) -> f32 {
        match self {
            Self::Fresh => 1.0,
            Self::Once => 0.8,
            Self::Twice => 0.64,
            Self::ThreeUses => 0.512,
            Self::FourUses => 0.4096,
            Self::Spent => 0.0,
        }
    }

    pub fn percent(self) -> u32 { (self.power() * 100.0 + 0.5) as u32 }

    pub fn unpack(byte: u8) -> Option<(u8, Self)> {
        Some((byte & 7, Self::new(byte >> 3)?))
    }

    pub fn pack(self, state: u8) -> u8 { self.uses() << 3 | state }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_activation_keeps_its_own_strength() {
        let mut wear = ArtefactWear::Fresh;
        for percent in [100, 80, 64, 51, 41] {
            assert_eq!(wear.percent(), percent);
            wear = wear.after_use();
            assert_eq!(wear.firing().percent(), percent);
            for state in 0..=6 {
                assert_eq!(ArtefactWear::unpack(wear.pack(state)), Some((state, wear)));
            }
        }
        assert_eq!(wear.after_use().percent(), 0);
        assert_eq!(ArtefactWear::unpack(6 << 3), None);
    }
}
