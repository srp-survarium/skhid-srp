//! The motion numbers. `Kinematics` (kin.rs) and `Feel` (motor.rs) keep their types and defaults; the tuning only
//! names them (`Tuning::kin`, `Tuning::feel`). `Feel`'s hop numbers and its lip look-ahead are parsed (`limits`);
//! its ranges left to check are below.

use super::*;

impl Check for Feel {
    /// How far under a walked leg's floor a body is still on the route; the sprint by the turns' angles are a half turn
    /// at most.
    fn check(&self) -> Result<(), String> {
        check(self.off_walk_below_m >= 0.6, "feel.off_walk_below_m")?;
        check(
            self.sprint_turns.is_none_or(|t| t.sharp_deg.get() <= 180.0 && t.keep_facing_deg.get() <= 180.0),
            "feel.sprint_turns.*_deg",
        )
    }
}
