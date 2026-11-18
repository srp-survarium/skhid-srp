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
//!
//! It also names a weapon's baked files (`weapon_camo.bake_name`, once patch
//! 44 has filled a surface's parameters): [`BakedName`], a fixed 22
//! characters made of all the bake is made of. The description patch 44 used
//! to write (camouflage, model, surface, each decal's texture and place) made
//! a path longer than Windows takes under a long Documents folder: the file
//! was not made and the game stopped at its file-open assertion. The name is
//! the texture's identity everywhere (the saved list, the bake under way,
//! the textures held: `bake_cache`), so it is made only of what is baked: a
//! layer with no texture gives it nothing of its own (its matrix in the
//! parameters is whatever the allocation held before).

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::bake_cache::Fnv;
use crate::vostok::baker::{Asker, BakeInputs, Parameters, WeaponBake};
use crate::vostok::{Context, Flow, NoExit, WeaponCamoOnOwnCamouflageExit};

/// How every weapon's baked files' name begins: the baker's [`Asker`] tells
/// a weapon's bake by it. Bumped when the same inputs bake into something
/// else (the masks), so the files baked before are not taken.
const PREFIX: &[u8; 6] = b"srpw1_";

/// `bake.own_camouflage`: a camouflage layer's texture with no name.
pub fn on_own_camouflage(ctx: &mut Context<'_>) -> Flow<WeaponCamoOnOwnCamouflageExit> {
    match Parameters::at_own_camouflage(ctx).map(Parameters::asker) {
        Some(Asker::Weapon) => Flow::Exit(WeaponCamoOnOwnCamouflageExit::NoLayer),
        Some(Asker::Character) | None => Flow::Continue,
    }
}

/// `weapon_camo.bake_name`: patch 44 is about to ask for one surface's bake.
pub fn on_bake_name(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(bake) = WeaponBake::at(ctx) {
        bake.name(BakedName::of(&bake.inputs).as_c_str());
    }
    Flow::Continue
}

/// A weapon's baked files' name (before the type's letter): [`PREFIX`] and
/// the FNV-1a 64, in 16 hex digits, of all the bake is made of: the model,
/// the surface, the scale, and each layer: a word 0 for one with no texture,
/// else a word 1, its matrix and its texture names (each name with its NUL,
/// so no two lists of names read alike).
pub struct BakedName([u8; 23]);

impl BakedName {
    pub fn of(bake: &BakeInputs<'_>) -> BakedName {
        let mut hash =
            Fnv::new().bytes(PREFIX).bytes(bake.model.to_bytes_with_nul()).word(bake.surface).word(bake.scale.to_bits());
        for layer in &bake.layers {
            let Some(layer) = layer else {
                hash = hash.word(0);
                continue;
            };
            hash = hash.word(1);
            for value in layer.matrix {
                hash = hash.word(value.to_bits());
            }
            for texture in layer.textures {
                hash = hash.bytes(texture.to_bytes_with_nul());
            }
        }
        let hash = hash.get();
        let mut name = [0; 23];
        name[..PREFIX.len()].copy_from_slice(PREFIX);
        for (i, digit) in name[PREFIX.len()..22].iter_mut().enumerate() {
            *digit = b"0123456789abcdef"[(hash >> (60 - 4 * i)) as usize & 0xF];
        }
        BakedName(name)
    }

    pub fn as_c_str(&self) -> &CStr {
        CStr::from_bytes_with_nul(&self.0).expect("a prefix, hex digits and one NUL")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::baker::BakeLayer;

    const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

    /// A weapon with a camouflage and no decal.
    fn bake<'a>(model: &'a CStr, camouflage: [&'a CStr; 4]) -> BakeInputs<'a> {
        BakeInputs {
            model,
            surface: 0,
            scale: 4.0,
            layers: [None, None, Some(BakeLayer { matrix: IDENTITY, textures: camouflage })],
        }
    }

    fn decal(texture: &'static CStr, at: f32) -> Option<BakeLayer<'static>> {
        let mut matrix = IDENTITY;
        matrix[12] = at;
        Some(BakeLayer { matrix, textures: [texture, c"", c"", c""] })
    }

    fn name(bake: &BakeInputs<'_>) -> String {
        BakedName::of(bake).as_c_str().to_str().unwrap().to_owned()
    }

    #[test]
    fn a_name_is_the_prefix_and_16_hex_digits_whatever_the_inputs() {
        let long = c"character/human/camo/black_market/brown_leather_03_and_a_good_deal_more_than_that";
        let name = name(&bake(c"weapons/pkp_pecheneg/pkp_pecheneg_n_cobra_ekp_8", [long, c"", c"", long]));
        assert_eq!(name.len(), 22);
        assert!(name.starts_with("srpw1_"));
        assert!(name[6..].bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    }

    #[test]
    fn every_input_makes_another_name() {
        let camo = c"character/human/base_camo/multicam_512";
        let smap = c"character/human/base_camo/multicam_512_smap";
        let base = bake(c"weapons/uzi", [camo, c"", c"", smap]);
        let mut seen = vec![name(&base)];
        let mut other = |change: &dyn Fn(&mut BakeInputs<'static>)| {
            let mut bake = bake(c"weapons/uzi", [camo, c"", c"", smap]);
            change(&mut bake);
            let name = name(&bake);
            assert!(!seen.contains(&name), "{name} again");
            seen.push(name);
        };
        other(&|b| b.model = c"weapons/uzi2");
        other(&|b| b.surface = 1);
        other(&|b| b.scale = 2.5);
        other(&|b| b.layers[2].as_mut().unwrap().matrix[12] = 0.5);
        other(&|b| b.layers[0] = decal(c"character/human/decal_3", 0.0));
        other(&|b| b.layers[0] = decal(c"character/human/decal_3", 0.25));
        other(&|b| b.layers[0] = decal(c"character/human/decal_4", 0.0));
        other(&|b| b.layers[1] = decal(c"character/human/decal_3", 0.0));
        other(&|b| b.layers[2].as_mut().unwrap().textures = [c"", c"", c"", smap]);
        // the same names, split otherwise between the fields
        other(&|b| b.layers[2].as_mut().unwrap().textures = [c"", camo, c"", smap]);
        // a decal alone, and the same texture as the camouflage's layer
        other(&|b| b.layers = [decal(camo, 0.0), None, None]);
        other(&|b| b.layers = [None, None, decal(camo, 0.0)]);
        assert_eq!(name(&bake(c"weapons/uzi", [camo, c"", c"", smap])), seen[0], "the same inputs, the same name");
    }
}
