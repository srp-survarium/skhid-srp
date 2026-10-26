//! The motion numbers. `Kinematics` (kin.rs) and `Feel` (motor.rs) keep their types and defaults; the tuning only
//! names them (`Tuning::kin`, `Tuning::feel`). `Feel`'s hop numbers and its lip look-ahead are parsed (`limits`);
//! its one range left to check is below.

use super::*;

impl Check for Feel {
    /// How far under a walked leg's floor a body is still on the route.
    fn check(&self) -> Result<(), String> {
        check(self.off_walk_below_m >= 0.6, "feel.off_walk_below_m")
    }
}
