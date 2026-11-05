//! Patch 109: the artefact tree's skills the exe computes (docs/notes/artefact-skills.md).
//! The data skills (Concentration, Forest Protection, Knowledge of Artifacts) are native
//! modifiers 7, 41 and 39; the others travel packed in modifier 40 (`artefact_power`), parsed
//! once here.
#![forbid(unsafe_code)]

use crate::vostok::{ArtefactSkillsOnLandingExit, Context, Flow, NoExit, artefact_skills};

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

/// Keeper of the Artifacts' draw, 1..=6, as srp rolls it once a match (modifier 40 bits 9..11);
/// the kind is the draw over the kinds the map has.
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

/// Keeper of the Artifacts: not taken, taken (the lobby's kits), or taken with the match's
/// draw (the static info's kits, srp writes it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Keeper {
    #[default]
    No,
    Taken,
    Drawn(Draw),
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
    keeper: Keeper,
    resonance: bool,
    spring: bool,
    onyx: bool,
    larkspur: bool,
    cascade: bool,
}

impl Skills {
    /// Bits: 0..2 Resistance, 3 Keeper, 4 Resonance, 5 Spring II, 6 Onyx II, 7 Larkspur II,
    /// 8 Resonance Cascade, 9..11 Keeper's draw 1..6 (only with bit 3). Anything else (a
    /// fraction, a negative, a rank past 5, a draw of 7 or without the skill, a bit past 11) is
    /// no skill at all.
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
        let keeper = match (bit(3), ((bits >> 9) & 7) as u8) {
            (false, 0) => Keeper::No,
            (true, 0) => Keeper::Taken,
            (true, draw) => Keeper::Drawn(Draw::parse(draw)?),
            (false, _) => return None,
        };
        Some(Self {
            resistance: Resistance(resistance),
            keeper,
            resonance: bit(4),
            spring: bit(5),
            onyx: bit(6),
            larkspur: bit(7),
            cascade: bit(8),
        })
    }

    /// Keeper of the Artifacts: not taken, taken, or taken with the match's draw.
    pub fn keeper(self) -> Keeper {
        self.keeper
    }

    /// Onyx II: no fall damage while one of the player's Onyx is active.
    pub fn onyx(self) -> bool {
        self.onyx
    }

    /// Spring II (the owner, 2026-11-05): each carried Spring's reload speed (modifier 13),
    /// +5% while its passive is on and +10% more while its active is, as its other nodes:
    /// (passive, active) node values, 0 without the skill.
    pub fn spring_reload(self) -> (f32, f32) {
        if self.spring { (0.05, 0.10) } else { (0.0, 0.0) }
    }

    /// Larkspur II: a Larkspur's activation restores all energy.
    pub fn larkspur(self) -> bool {
        self.larkspur
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

/// Onyx II at the landing's start: the whole landing is its fall damage, skipped when the player
/// has the skill and an active Onyx.
pub fn on_landing(ctx: &mut Context<'_>) -> Flow<ArtefactSkillsOnLandingExit> {
    if artefact_skills::lands_unhurt(ctx) {
        Flow::Exit(ArtefactSkillsOnLandingExit::Done)
    } else {
        Flow::Continue
    }
}

/// Larkspur II at an artefact's actual activation (artefact_base::action 0x313333).
pub fn on_larkspur(ctx: &mut Context<'_>) -> Flow<NoExit> {
    artefact_skills::larkspur(ctx);
    Flow::Continue
}

/// Spring II, patch 54's entry at a Spring's passive link: its reload nodes' values.
pub fn on_spring_reload(ctx: &mut Context<'_>) -> Flow<NoExit> {
    artefact_skills::spring_reload(ctx);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_packed_ranks_only_exactly() {
        for bad in [-1.0, 0.5, 6.0, 7.0, 512.0, 1024.0, 8.0 + 7.0 * 512.0, 4096.0, f32::INFINITY, f32::NAN] {
            assert_eq!(Skills::parse(bad), None, "{bad}");
        }
        assert_eq!(Skills::parse(0.0), Some(Skills::default()));
        let full = Skills::parse(509.0).unwrap();
        assert_eq!(full.resistance, Resistance(5));
        assert_eq!(full.keeper, Keeper::Taken);
        assert!(full.resonance && full.spring && full.onyx && full.larkspur && full.cascade);
        for draw in 1..=6u8 {
            let skills = Skills::parse(8.0 + f32::from(draw) * 512.0).unwrap();
            assert_eq!(skills.keeper, Keeper::Drawn(Draw(draw)));
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
