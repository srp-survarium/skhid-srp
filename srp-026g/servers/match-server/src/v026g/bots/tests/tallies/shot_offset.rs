//! Where a shot's line passed its target, for the stand-in's hit volume ([`super::owner`]'s `shot_offsets`
//! topic counts them).
//!
//! The stand-in decides a hit with one test, `miss < 0.45` m from the target's chest point (`Game::shot`). The
//! game's own hit boxes are the body's, so the volume is what the stand-in's per-shot hit rate rests on. The
//! shot's offset from the chest is recorded here as a fact of the shot, in fixed bins, so that any other volume
//! (a smaller disc, the body's capsules) is a sum over the same shots offline, with no other bench run. Nothing
//! here decides anything: the offset is worked out from the line the shot already has, after the stand-in has
//! resolved the shot.
//!
//! The offset is the vector from the target's chest to the nearest point of the shot's line (the line after the
//! spread's deflection, the one that hits), split on the two axes across the line: `lateral` (horizontal, positive
//! to the shooter's right, the map frame being left-handed with Y up and Z forward) and `vertical` (positive up).
//! `miss` is its length, the number the stand-in compares with 0.45 m.

use mapdata::Vec3;

/// The stand-in's shots reach this far, metres (`Game::shot`'s `along > 100.0`).
pub(crate) const REACH_M: f32 = 100.0;
/// The radial histogram: this wide a bin, [`RADIAL_BINS`] of them (0 to 1 m), then one for all farther.
pub(crate) const RADIAL_M: f32 = 0.05;
pub(crate) const RADIAL_BINS: usize = 20;
/// The grid: this wide a cell, [`GRID`] of them on each axis, centred on the chest (-1.5 m to 1.5 m).
pub(crate) const CELL_M: f32 = 0.1;
pub(crate) const GRID: usize = 30;

/// The stand-in's hit volume, metres: a shot within this of the chest point hits (when it is the first body).
pub(crate) const DISC_M: f32 = 0.45;
/// The `line on body` test of the fight counts, metres.
pub(crate) const BODY_M: f32 = 0.25;

/// The shot line's offset from the chest, metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Offset {
    pub miss: f32,
    pub lateral: f32,
    pub vertical: f32,
}

impl Offset {
    /// The line from `eye` along `dir` (unit) against `chest`: none when the chest is not ahead of the eye or is
    /// past the stand-in's reach (the shot cannot hit it), else the offset.
    fn of(eye: Vec3, dir: Vec3, chest: Vec3) -> Option<Self> {
        let to = chest - eye;
        let along = to.dot(dir);
        if along <= 0.0 || along > REACH_M {
            return None;
        }
        let off = dir * along - to;
        let flat = dir.x.hypot(dir.z);
        let side = if flat > 1e-4 {
            Vec3::new(dir.z / flat, 0.0, -dir.x / flat)
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        let up = dir.cross(side);
        Some(Self {
            miss: off.length(),
            lateral: off.dot(side),
            vertical: off.dot(up),
        })
    }

    /// The radial histogram's bin of this offset.
    pub(crate) fn radial_bin(self) -> usize {
        ((self.miss / RADIAL_M) as usize).min(RADIAL_BINS)
    }

    /// The grid's cell (lateral, vertical) of this offset: none outside the grid.
    pub(crate) fn cell(self) -> Option<(usize, usize)> {
        let index = |x: f32| {
            let i = ((x + GRID as f32 * CELL_M / 2.0) / CELL_M).floor();
            (i >= 0.0 && i < GRID as f32).then_some(i as usize)
        };
        Some((index(self.lateral)?, index(self.vertical)?))
    }
}

/// The distance bands of the offsets, by the horizontal distance to the target (the one the fight counts use).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DistBand {
    To10,
    To20,
    To35,
    To50,
    To80,
    Beyond,
}

impl DistBand {
    pub(crate) const ALL: [Self; 6] = [Self::To10, Self::To20, Self::To35, Self::To50, Self::To80, Self::Beyond];

    pub(crate) fn of(distance: f32) -> Self {
        match distance {
            d if d <= 10.0 => Self::To10,
            d if d <= 20.0 => Self::To20,
            d if d <= 35.0 => Self::To35,
            d if d <= 50.0 => Self::To50,
            d if d <= 80.0 => Self::To80,
            _ => Self::Beyond,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::To10 => "0-10",
            Self::To20 => "10-20",
            Self::To35 => "20-35",
            Self::To50 => "35-50",
            Self::To80 => "50-80",
            Self::Beyond => "80+",
        }
    }
}

/// The weapon classes of `Gun::class`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ShotClass {
    Rifle,
    Smg,
    Mg,
    Shotgun,
    Marksman,
    Bolt,
    Pistol,
}

impl ShotClass {
    pub(crate) const ALL: [Self; 7] =
        [Self::Rifle, Self::Smg, Self::Mg, Self::Shotgun, Self::Marksman, Self::Bolt, Self::Pistol];

    /// A `Gun::class` name (which gives "rifle" for any category it has no row for).
    pub(crate) fn of(name: &str) -> Self {
        match name {
            "smg" => Self::Smg,
            "mg" => Self::Mg,
            "shotgun" => Self::Shotgun,
            "marksman" => Self::Marksman,
            "bolt" => Self::Bolt,
            "pistol" => Self::Pistol,
            _ => Self::Rifle,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Rifle => "rifle",
            Self::Smg => "smg",
            Self::Mg => "mg",
            Self::Shotgun => "shotgun",
            Self::Marksman => "marksman",
            Self::Bolt => "bolt",
            Self::Pistol => "pistol",
        }
    }
}

/// The target's stance: the chest point is at 1.30 m standing, 0.80 m crouched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Stance {
    Standing,
    Crouched,
}

impl Stance {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Standing => "stand",
            Self::Crouched => "crouch",
        }
    }
}

/// One shot at the shooter's target: where the line passed him (none: not ahead of the eye, or past the reach),
/// and what he and the weapon were.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ShotOffset {
    pub band: DistBand,
    pub class: ShotClass,
    pub stance: Stance,
    pub line: Option<Offset>,
}

impl ShotOffset {
    /// The shot from `eye` along `dir` (unit) at a target whose chest point is `chest`, `distance` metres away on
    /// the ground, of a weapon of class `class` named by [`ShotClass::of`].
    pub(crate) fn of(eye: Vec3, dir: Vec3, chest: Vec3, distance: f32, class: ShotClass, crouched: bool) -> Self {
        Self {
            band: DistBand::of(distance),
            class,
            stance: if crouched { Stance::Crouched } else { Stance::Standing },
            line: Offset::of(eye, dir, chest),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offset(miss: f32) -> Offset {
        Offset { miss, lateral: 0.0, vertical: 0.0 }
    }

    #[test]
    fn the_radial_bins_are_5_cm_wide_up_to_a_metre() {
        assert_eq!(offset(0.0).radial_bin(), 0);
        assert_eq!(offset(0.04).radial_bin(), 0);
        assert_eq!(offset(0.06).radial_bin(), 1);
        // The stand-in's disc (0.45 m) and the line-on-body test (0.25 m) are bin edges: the bins below them sum
        // to the shots inside.
        assert_eq!(offset(0.44).radial_bin(), 8);
        assert_eq!(offset(0.46).radial_bin(), 9);
        assert_eq!(offset(0.24).radial_bin(), 4);
        assert_eq!(offset(0.26).radial_bin(), 5);
        assert_eq!(offset(0.99).radial_bin(), RADIAL_BINS - 1);
        assert_eq!(offset(1.01).radial_bin(), RADIAL_BINS);
        assert_eq!(offset(250.0).radial_bin(), RADIAL_BINS);
    }

    #[test]
    fn the_grid_cells_are_10_cm_wide_from_minus_1_5_to_1_5() {
        let at = |lateral: f32, vertical: f32| Offset { miss: 0.0, lateral, vertical }.cell();
        assert_eq!(at(-1.45, 1.45), Some((0, 29)));
        assert_eq!(at(-0.02, 0.02), Some((14, 15)));
        assert_eq!(at(0.04, -0.04), Some((15, 14)));
        assert_eq!(at(-1.55, 0.0), None);
        assert_eq!(at(0.0, 1.55), None);
    }

    #[test]
    fn the_bands_and_classes_are_the_fight_counts() {
        assert_eq!(DistBand::of(10.0), DistBand::To10);
        assert_eq!(DistBand::of(10.01), DistBand::To20);
        assert_eq!(DistBand::of(35.0), DistBand::To35);
        assert_eq!(DistBand::of(80.0), DistBand::To80);
        assert_eq!(DistBand::of(80.5), DistBand::Beyond);
        for class in ShotClass::ALL {
            assert_eq!(ShotClass::of(class.label()), class);
        }
        assert_eq!(ShotClass::of("anything else"), ShotClass::Rifle);
    }

    #[test]
    fn a_line_beside_and_above_the_chest_is_split_on_the_axes_across_it() {
        // A shot along +Z at a chest 20 m ahead, 0.25 m to its right and 0.1 m below the line: the line is
        // 0.25 m to the left of the chest and 0.1 m above it.
        let eye = Vec3::new(0.0, 1.6, 0.0);
        let chest = Vec3::new(0.25, 1.5, 20.0);
        let off = Offset::of(eye, Vec3::new(0.0, 0.0, 1.0), chest).expect("ahead");
        assert!((off.lateral + 0.25).abs() < 1e-5, "{off:?}");
        assert!((off.vertical - 0.1).abs() < 1e-5, "{off:?}");
        assert!((off.miss - 0.25f32.hypot(0.1)).abs() < 1e-5, "{off:?}");
        // Along a tilted line the two axes are still across it: the split keeps the length.
        let dir = Vec3::new(0.3, 0.2, 1.0);
        let dir = dir * (1.0 / dir.length());
        let off = Offset::of(eye, dir, Vec3::new(2.0, 3.0, 30.0)).expect("ahead");
        assert!((off.miss - off.lateral.hypot(off.vertical)).abs() < 1e-4, "{off:?}");
    }

    #[test]
    fn a_chest_behind_the_eye_or_past_the_reach_has_no_line() {
        let eye = Vec3::new(0.0, 1.6, 0.0);
        let dir = Vec3::new(0.0, 0.0, 1.0);
        assert!(Offset::of(eye, dir, Vec3::new(0.0, 1.3, -5.0)).is_none());
        assert!(Offset::of(eye, dir, Vec3::new(0.0, 1.3, 100.5)).is_none());
        assert!(Offset::of(eye, dir, Vec3::new(0.0, 1.3, 99.0)).is_some());
    }
}
