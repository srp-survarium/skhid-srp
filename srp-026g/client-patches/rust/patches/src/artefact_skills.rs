//! Patch 109: the artefact tree's skills the exe computes (docs/notes/artefact-skills.md).
//! The data skills (Concentration, Forest Protection, Knowledge of Artifacts) are native
//! modifiers 7, 41 and 39; the others travel packed in modifier 40 (`artefact_power`), parsed
//! once here.
#![forbid(unsafe_code)]

use crate::vostok::{Context, Flow, NoExit, artefact_skills};

/// A hit's type (the hit +1Ch; 0.26g's seven, `blunt` gone): 4..7 are the anomalous ones
/// (table 0x92075C), against which armour is the card's isolation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    Ranged,
    Blunt,
    Slash,
    Explosive,
    Toxic,
    Radiation,
    Fire,
    Shock,
}

impl HitKind {
    pub fn parse(value: u32) -> Option<Self> {
        Some(match value {
            0 => Self::Ranged,
            1 => Self::Blunt,
            2 => Self::Slash,
            3 => Self::Explosive,
            4 => Self::Toxic,
            5 => Self::Radiation,
            6 => Self::Fire,
            7 => Self::Shock,
            _ => return None,
        })
    }

    fn anomalous(self) -> bool {
        matches!(self, Self::Toxic | Self::Radiation | Self::Fire | Self::Shock)
    }
}

/// The match's Keeper of the Artifacts draw, 1..=6, as srp rolls it once a match and writes it
/// into every kit of every player (modifier 40 bits 9..11), with the skill or not: every game
/// knows the kind at the map's load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Draw(u8);

impl Draw {
    pub const MOST: u8 = 6;

    pub fn parse(draw: u8) -> Option<Self> {
        (1..=Self::MOST).contains(&draw).then_some(Self(draw))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// Resistance's rank, 0..=5.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resistance(u8);

impl Resistance {
    pub const MOST: u8 = 5;
    /// Armour a rank against the anomalous types: +5 isolation points (the card x100).
    pub const ARMOUR: f32 = 0.05;
}

/// The packed skills of a kit's modifier 40.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Skills {
    resistance: Resistance,
    keeper: bool,
    draw: Option<Draw>,
    resonance: bool,
    spring: bool,
    onyx: bool,
    larkspur: bool,
    cascade: bool,
}

impl Skills {
    /// Bits: 0..2 Resistance, 3 Keeper, 4 Resonance, 5 Spring II, 6 Onyx II, 7 Larkspur II,
    /// 8 Resonance Cascade, 9..11 the match's Keeper draw 1..6 (0 none; with the skill or not).
    /// Anything else (a fraction, a negative, a rank past 5, a draw of 7, a bit past 11) is no
    /// skill at all.
    pub fn parse(value: f32) -> Option<Self> {
        if !(0.0..4096.0).contains(&value) || value != (value as u32) as f32 {
            return None;
        }
        let bits = value as u32;
        let resistance = (bits & 7) as u8;
        if resistance > Resistance::MOST {
            return None;
        }
        let bit = |n: u32| bits & (1 << n) != 0;
        let draw = match ((bits >> 9) & 7) as u8 {
            0 => None,
            draw => Some(Draw::parse(draw)?),
        };
        Some(Self {
            resistance: Resistance(resistance),
            keeper: bit(3),
            draw,
            resonance: bit(4),
            spring: bit(5),
            onyx: bit(6),
            larkspur: bit(7),
            cascade: bit(8),
        })
    }

    /// Keeper of the Artifacts taken.
    pub fn keeper(self) -> bool {
        self.keeper
    }

    /// The match's Keeper draw (srp writes it into every kit of the match), none in the lobby.
    pub fn draw(self) -> Option<Draw> {
        self.draw
    }

    /// Resistance: the part's armour `native` against a hit of `kind`, +0.05 a rank for the
    /// anomalous types, before the piercing's subtraction and the native clamp.
    pub fn armour(self, native: f32, kind: HitKind) -> f32 {
        if kind.anomalous() {
            native + f32::from(self.resistance.0) * Resistance::ARMOUR
        } else {
            native
        }
    }
}

/// Resistance at body_part_parameters::hit_by_type (0x32A083): XMM0 the part's armour.
pub fn on_isolation(ctx: &mut Context<'_>) -> Flow<NoExit> {
    artefact_skills::isolation(ctx);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_packed_ranks_only_exactly() {
        for bad in [-1.0, 0.5, 6.0, 7.0, 7.0 * 512.0, 8.0 + 7.0 * 512.0, 4096.0, f32::INFINITY, f32::NAN] {
            assert_eq!(Skills::parse(bad), None, "{bad}");
        }
        assert_eq!(Skills::parse(0.0), Some(Skills::default()));
        let full = Skills::parse(509.0).unwrap();
        assert_eq!(full.resistance, Resistance(5));
        assert!(full.keeper && full.draw.is_none());
        assert!(full.resonance && full.spring && full.onyx && full.larkspur && full.cascade);
        for draw in 1..=6u8 {
            let skills = Skills::parse(8.0 + f32::from(draw) * 512.0).unwrap();
            assert!(skills.keeper && skills.draw == Draw::parse(draw));
            // Every kit of the match has the draw, a player without the skill too.
            let skills = Skills::parse(f32::from(draw) * 512.0).unwrap();
            assert!(!skills.keeper && skills.draw.map(Draw::get) == Some(draw));
        }
        assert!(Skills::parse(509.0 + 6.0 * 512.0).is_some());
    }

    #[test]
    fn resistance_adds_isolation_to_the_anomalous_types_only() {
        for rank in 0..=5u8 {
            let skills = Skills::parse(f32::from(rank)).unwrap();
            for value in 0..8 {
                let kind = HitKind::parse(value).unwrap();
                let want = if value >= 4 { 0.3 + 0.05 * f32::from(rank) } else { 0.3 };
                assert!((skills.armour(0.3, kind) - want).abs() < 1e-6, "{rank} {kind:?}");
            }
        }
        assert_eq!(HitKind::parse(8), None);
    }
}
