//! Patch 108: a gun-stock hit kills, wherever it lands (the owner,
//! 2026-11-05; docs/notes/hand-kill.md).
//!
//! - A melee hit is blunt (a weapon's melee/data) and has an attacker other
//!   than the victim (a fall is blunt too, and its attacker is the victim).
//! - Every zone sends a blunt hit on to pain (hit_params: the arms and the
//!   hips 5 %, the feet 80 %, the torso and the head all of it, with body or
//!   head); stock, a hit to an arm, a hip or a shin only takes a little pain.
//! - Lethal: the hit deals [`LETHAL`] from here, through the zone's armour
//!   and the victim's drugs and artefacts as any hit; its share reaches pain,
//!   whose empty pool is the game's own death (affect 0), as a limb shot
//!   kills.

#![forbid(unsafe_code)]

use crate::vostok::drug::HitType;
use crate::vostok::melee::DealingHit;
use crate::vostok::{Context, Flow, NoExit};

/// Damage past every pool: each holds 1 at most, and pain takes 5 % of a
/// blunt hit at the least (an arm, a hip), so 1000 empties it through any
/// armour short of full (0.26g's has none for blunt) and an active Onyx's
/// 75 %.
pub const LETHAL: f32 = 1000.0;

/// hit_body_part once invulnerability is applied (`medical.hit`), before
/// patch 94's rules for the same hit.
pub fn on_hit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut hit) = DealingHit::at(ctx)
        && hit.kind() == Some(HitType::Blunt)
        && hit.by_another()
    {
        hit.set_damage(LETHAL);
    }
    Flow::Continue
}
