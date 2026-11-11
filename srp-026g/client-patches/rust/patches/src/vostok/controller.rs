//! A character controller at the end of its step (`controller.step_end`,
//! bullet_character_controller::player_step 0x7BDA0's epilogue 0x7C7D7, which
//! every path of the step joins). player_step copied the position (+0B0h) to
//! m_last_pos (+50h) at its start (0x7BE84) and wrote the position where the
//! step left it at its end (0x7C169..0x7C797); nothing between those writes
//! and the epilogue reads it. The position is the translation of the ghost
//! object's world transform (the object at +70h, its transform +80h:
//! set_transform 0x7F6D0 writes it through 0x70C20), and the step writes it
//! in place without updating the object's AABB, which the next AABB update
//! (set_transform's 0x9FF40, the world's) takes from it. Every game runs every
//! controller's step in its simulation, so what is decided here is decided
//! alike everywhere (patch 117, docs/notes/client_patch.md section 3.71).
//! Every field is fields_026g.tsv's.

use super::context::Context;
use crate::raw;

/// A controller position, Bullet's frame (z the game's -z), with every one
/// of its three floats finite. Kept as the floats' bits, so it is written
/// back exactly as it was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position([u32; 3]);

impl Position {
    /// The position of three floats' bits, or None when one of them is an
    /// infinity or a NaN (its exponent all ones).
    pub fn parse(bits: [u32; 3]) -> Option<Position> {
        bits.iter().all(|&b| f32::from_bits(b).is_finite()).then_some(Position(bits))
    }

    pub fn bits(self) -> [u32; 3] {
        self.0
    }
}

/// A controller whose step just ended.
pub struct SteppedController {
    end: Option<Position>,
    start: Option<Position>,
    controller: raw::BulletCharacterController,
}

impl SteppedController {
    pub fn at(ctx: &Context<'_>) -> Option<SteppedController> {
        use raw::site::controller_step_end as site;
        // SAFETY: the context is this site's: EDI is player_step's
        // controller (0x7BDAC), alive while its step runs, on the physics
        // tick's thread; the position and m_last_pos are three f32 each.
        unsafe {
            let controller = site::controller(ctx)?;
            let end = Position::parse(controller.position().cast::<[u32; 3]>().read_unaligned());
            let start = Position::parse(controller.last_position().cast::<[u32; 3]>().read_unaligned());
            Some(SteppedController { end, start, controller })
        }
    }

    /// Where the step left it, if finite.
    pub fn end(&self) -> Option<Position> {
        self.end
    }

    /// Where the step began (m_last_pos), if finite.
    pub fn start(&self) -> Option<Position> {
        self.start
    }

    /// Puts it at `position`, as the step's end.
    pub fn set_end(&mut self, position: Position) {
        // SAFETY: the controller's position, its step's own until the
        // epilogue returns.
        unsafe { self.controller.position().cast::<[u32; 3]>().write_unaligned(position.bits()) }
        self.end = Some(position);
    }
}

#[cfg(test)]
mod tests {
    use super::Position;

    #[test]
    fn finite_floats_are_a_position() {
        let bits = [(-12.7f32).to_bits(), 21.69f32.to_bits(), 0.0f32.to_bits()];
        assert_eq!(Position::parse(bits).map(Position::bits), Some(bits));
        assert!(Position::parse([(-0.0f32).to_bits(), f32::MIN.to_bits(), 1]).is_some());
    }

    #[test]
    fn an_infinity_or_a_nan_is_not() {
        // x87's indefinite (the sign set), the live world's quiet NaN, a
        // signalling NaN, the infinities.
        let signalling = f32::INFINITY.to_bits() | 1;
        for bad in [(-f32::NAN).to_bits(), f32::NAN.to_bits(), signalling, f32::INFINITY.to_bits(), f32::NEG_INFINITY.to_bits()] {
            for at in 0..3 {
                let mut bits = [0u32; 3];
                bits[at] = bad;
                assert_eq!(Position::parse(bits), None, "{bad} at {at}");
            }
        }
    }
}
