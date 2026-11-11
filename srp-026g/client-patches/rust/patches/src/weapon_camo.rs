//! Patch 44's part in the baker: a weapon's bake takes only the camouflage
//! textures the weapon's patch names (weapon_decals_026g.asm's fill: the
//! camouflage's colours and its `_smap`; docs/notes/weapon-camo.md).
//!
//! The game reads a camouflage layer's empty name as "the surface's own":
//! `<diffuse>_camo`, `_camo_nmap`, `_camo_smap` (0x12E124), a character
//! part's default camouflage. A weapon has none: beside some weapons'
//! textures files of those names are other things (the premium paints
//! ak_12_camo and rem700_camo_smap, sks_camo_nmap), which the bake would
//! tile over the weapon. So an empty name of a weapon's bake is no layer:
//!
//! - the camouflage's `_nmap` is never named (the weave of the cloth the
//!   camouflages were drawn for): the weapon keeps its own relief, and
//!   with no decal no `n` texture is baked at all;
//! - a weapon with decals and no camouflage gets its decals alone.

#![forbid(unsafe_code)]

use crate::vostok::baker::{Asker, Parameters};
use crate::vostok::{Context, Flow, WeaponCamoOnOwnCamouflageExit};

/// `bake.own_camouflage`: a camouflage layer's texture with no name.
pub fn on_own_camouflage(ctx: &mut Context<'_>) -> Flow<WeaponCamoOnOwnCamouflageExit> {
    match Parameters::at_own_camouflage(ctx).map(Parameters::asker) {
        Some(Asker::Weapon) => Flow::Exit(WeaponCamoOnOwnCamouflageExit::NoLayer),
        Some(Asker::Character) | None => Flow::Continue,
    }
}
