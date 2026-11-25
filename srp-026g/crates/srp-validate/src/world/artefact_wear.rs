//! Successful activations of one combat artefact since its anomaly spawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArtefactWear(u16);

impl ArtefactWear {
    pub const FRESH: Self = Self(0);
    /// Reserved low state 7. Followed by the little-endian count and native state.
    pub const EXTENDED: u8 = 0xff;

    pub fn new(uses: u16) -> Self { Self(uses) }
    pub fn uses(self) -> u16 { self.0 }

    pub fn after_use(self) -> Self { Self(self.0.saturating_add(1)) }

    /// The activation in progress uses the power from before it was paid for.
    pub fn firing(self) -> Self { Self(self.0.saturating_sub(1)) }

    pub fn power(self) -> f32 {
        let mut exponent = self.0;
        let mut power = 1.0_f64;
        let mut factor = 0.8_f64;
        while exponent != 0 {
            if exponent & 1 != 0 { power *= factor; }
            exponent >>= 1;
            factor *= factor;
        }
        // Preserve a positive multiplier even below the game's float precision.
        (power as f32).max(f32::from_bits(1))
    }

    /// Existing worlds encode a small count directly above the three state bits.
    pub fn inline(self, state: u8) -> Option<u8> {
        (self.0 < 32).then_some(((self.0 as u8) << 3) | state)
    }

    pub fn from_inline(byte: u8) -> (u8, Self) { (byte & 7, Self(u16::from(byte >> 3))) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continues_across_five_uses_and_the_inline_wire_limit() {
        let mut wear = ArtefactWear::FRESH;
        for uses in 0..512 {
            let next = wear.after_use();
            assert_eq!(next.firing(), wear);
            let expected = (0.8_f64.powi(uses) as f32).max(f32::from_bits(1));
            assert_eq!(wear.power(), expected);
            assert_eq!(wear.inline(4).map(ArtefactWear::from_inline), (uses < 32).then_some((4, wear)));
            wear = next;
        }
        assert!(ArtefactWear::new(u16::MAX).after_use().power() > 0.0);
    }
}
