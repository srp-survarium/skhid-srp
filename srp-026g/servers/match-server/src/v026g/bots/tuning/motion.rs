//! The motion numbers' checks. `Kinematics` (kin.rs) and `Feel` (motor.rs) keep their types and defaults; the
//! tuning only names them (`Tuning::kin`, `Tuning::feel`) and checks the ranges of `Feel`.

use super::*;

/// `Feel`'s ranges: the hop's rise in order, its period above nothing.
pub(super) fn validate_feel(feel: &Feel) -> Result<(), String> {
    check(
        feel.hop_rise_m.0 <= feel.hop_rise_m.1 && feel.hop_every_s > 0.0,
        "feel.hop_*",
    )?;
    check(feel.lip_look_ahead_m >= 0.0, "feel.lip_look_ahead_m")?;
    check(feel.off_walk_below_m >= 0.6, "feel.off_walk_below_m")?;
    Ok(())
}
