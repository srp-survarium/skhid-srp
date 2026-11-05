//! Patch 108: a gun-stock hit to a hand kills when the attacker carries the
//! Anomalous Force Protein (the owner, 2026-11-05;
//! docs/notes/force-protein-hand-kill.md).
//!
//! - A melee hit is blunt (a weapon's melee/data) and has an attacker other
//!   than the victim (a fall is blunt too, and has none).
//! - A hand is a zone whose blunt hits go to a hand's pool (hit_params:
//!   the arms and the forearms, into right_hand or left_hand, and 5 % into
//!   pain); stock, such a hit only breaks the hand at 0.
//! - The carrier: the attacker's kit sums `stamina_boost_interval` above
//!   0, which the booster's parameter gives while it is in a pocket.
//! - Lethal: the hit deals [`LETHAL`] from here, through the zone's armour
//!   and the victim's drugs as any hit; its 5 % reaches pain, whose empty
//!   pool is the game's own death (affect 0), as a limb shot kills.

#![forbid(unsafe_code)]

use crate::vostok::drug::HitType;
use crate::vostok::melee::{DealingHit, ModifierIndex};
use crate::vostok::{Context, Flow, NoExit};

/// The zones of a hand (hit_params' arm and forearm zones; the hand pools
/// themselves take no hit, `can_be_assigned` 0).
const HAND_ZONES: [&[u8]; 4] = [b"right_arm", b"left_arm", b"right_forearm", b"left_forearm"];

/// Damage past every pool: each holds 1 at most, and pain takes 5 % of a
/// blunt arm hit, so 1000 empties it through any armour short of full.
pub const LETHAL: f32 = 1000.0;

/// hit_body_part once invulnerability is applied (`medical.hit`), before
/// patch 94's rules for the same hit.
pub fn on_hit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut hit) = DealingHit::at(ctx)
        && hit.kind() == Some(HitType::Blunt)
        && hit.zone().is_some_and(|zone| HAND_ZONES.contains(&zone.to_bytes()))
        && hit.attacker_modifier(ModifierIndex::MELEE_HAND_KILL).is_some_and(|sum| sum > 0.0)
    {
        hit.set_damage(LETHAL);
    }
    Flow::Continue
}
