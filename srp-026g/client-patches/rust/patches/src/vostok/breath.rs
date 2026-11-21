//! A hit victim's breath hold (patch 176, docs/notes/breath-hit.md). A gun's
//! breath_vibration_calculator (+288h) runs a machine of three states made
//! by initialize_logic (0x31D450): normal, holding and short breathing. It
//! ticks only while the gun is aimed (weapon_core::tick 0x2E0A23). Holding
//! goes back to normal when the key has been up for a second or the gun is
//! lowered (not_holding_breath 0x31D400, tried first), and to short
//! breathing once its reserve is spent (insufficient_breath 0x31D430):
//! holding's finalize refills the reserve and stops the hold's sound; short
//! breathing raises the penalty to 1, and no hold starts until normal has
//! brought it back to 0 (can_hold_breath 0x31D3B0).

use core::marker::PhantomData;

use super::drug::AppliedHit;
use super::hook::Thread;
use super::world::read;
use crate::raw;

/// A breath being held: the victim's held gun in the holding state.
pub struct HeldBreath<'h> {
    calculator: raw::BreathVibrationCalculator,
    thread: PhantomData<&'h Thread>,
}

impl AppliedHit<'_> {
    /// The victim's breath, if his held gun is holding it.
    pub fn held_breath(&self) -> Option<HeldBreath<'_>> {
        // SAFETY: the victim is alive for the hook; its held item is read as
        // a gun only once its vtable says it is one; a gun owns its
        // calculator, whose state is one of the three it made, never freed.
        unsafe {
            let held = self.victim().held_item()?;
            if ![raw::image::WEAPON_VTABLE, raw::image::WEAPON_CORE_VTABLE].contains(&read(held.vtable())) {
                return None;
            }
            let calculator = held.weapon().breath();
            let state = calculator.state()?;
            (read(state.vtable()) == raw::image::BREATH_HOLDING_VTABLE)
                .then_some(HeldBreath { calculator, thread: PhantomData })
        }
    }
}

impl HeldBreath<'_> {
    /// Spends the reserve, so the gun's next tick ends the hold as running
    /// out does. The game's own transition does the rest, alike in every
    /// game; two bullet tasks hitting one holder write the same 0.
    pub fn spend(self) {
        // SAFETY: the calculator of the victim's held gun, alive for the hook.
        unsafe { self.calculator.breath_holding_reserve().write_volatile(0.0) }
    }
}
