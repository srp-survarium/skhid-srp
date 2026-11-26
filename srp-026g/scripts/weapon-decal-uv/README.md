# Weapon decal coordinates

The AK-12's central body is called the receiver. Its render surface, `ak_12_s`, includes several pieces. Two faces can read the same pixels of the metal texture even when they are far apart on the model. Baking a projected decal into that texture paints both. A plain AK-12 with the Snufkin decal reproduces the problem. Skhid's optional weapon camouflage uses the authored material coordinates and its AOAC mask.

## Meshes and rendering

`weapon-decal-uv@2` covers all 102 whole and 102 bare models in `weapon-modules@3`. The event MP7 shares the suppressed MP7 model. Each render surface has its own 2048² bake. The generator lays out its triangles with xatlas, packed into exactly one 2048² atlas so that xatlas's 8 texels of padding and 4×4 block alignment are those of the baked texture. It tests the quantized triangles for positive-area overlap; each folded or collapsed face becomes a chart of its own, and xatlas packs those and the other charts again, whole and padded. (@1 squeezed the other charts to make room for such faces, and stretched every atlas to the square, which cost charts their padding: the SCAR-H's front sight had two charts in neighbouring blocks.) It preserves the order and positions of all triangle corners, their bone indices, the XYZ tangent frame and the authored texture coordinates.

The native one-weight vertex is 36 bytes. The new U is an unsigned 16-bit integer in bytes 13 and 14, and V is in bytes 15 and 23, low byte first. Bytes 19 and 27 contain the tag `SD`. The one-weight skinning programs use the first blend index and the XYZ components of the tangent frame. Other meshes retain their original encoding.

The unwrap vertex shader rasterizes tagged meshes in the new atlas. The composition pixel shader reads accumulated paint in that atlas and samples the original metal, mask and camouflage through the authored UVs. In a normal draw, the one-weight vertex shader passes both coordinates. After the engine binds the textures for the current surface, the client sets the four components of `srp_weapon_bake_channels` from the diffuse, normal, Fresnel and smoothness overrides. Each sampler uses its atlas as soon as that channel has a bake. Every patched material program carries the flags in the same float4 of its material buffer (`sequence_parameters`' place, which none of them uses), since they all share the renamed buffer `srp_dual_material_parameters`. This also handles a camouflage-only gun, where just diffuse and smoothness are baked. Untagged meshes use their usual UVs.

The content pack contains the meshes, their updated vertex counts and the shader permutations (about 54 MB). All source DDS textures stay in their existing packs. Ship the client and content together.

## Rebuild

Use [xatlas](https://github.com/jpcy/xatlas/tree/f700c7790aaa030e794b52ba7791a05c085faf0c) at commit `f700c7790aaa030e794b52ba7791a05c085faf0c`. From this directory, set `XATLAS` to its checkout, `PACKS` to the source content packs, `STOCK` to the stock 0.26g resources.db and `GAME_DATA` to the absolute path of srp-game-data.

```sh
c++ -std=c++17 -O2 -DXA_MULTITHREADED=0 -I"$XATLAS/source/xatlas" \
    unwrap.cpp "$XATLAS/source/xatlas/xatlas.cpp" -o /tmp/unwrap-weapon
python3 make_pack.py --stock "$STOCK" --shader-pack "$PACKS/london-v042-7" \
    --weapons "$PACKS/weapon-modules-3" --game-data "$GAME_DATA" \
    --unwrap /tmp/unwrap-weapon --version 2 --out /tmp/weapon-decal-uv-2
```

The generator guards each replacement with the hash of its upstream file. Run `tools/check weapon-decal-uv weapon-bake-coordinates` from the repository. The catalogue check follows every model path, checks the authored data at every corner, verifies exclusive interior coverage on the 2048² texel grid and that each chart's texels (every texel its triangles touch) lie in 4×4 blocks of their own with at least two empty blocks to the next chart's, and regenerates the shader edits from stock and the upstream shader pack. Across the 298 distinct meshes it checked 552,000 triangles and 764,076,879 interior samples, with zero shared texels, collapsed UV faces or charts closer than that. An atlas that keeps its padding fills less of the square than @1's stretched one did: about 8% fewer interior texels.

## Reproduce on the game GPU

Use separate local setups with the same saved loadout and empty streamed-texture caches. Apply the source pack and matching exe to the candidate using `tools/test-patch <setup> <pack> --exe decal_uv`. Equip AK-12 item 363. For the decal fixture, equip Snufkin (1896) in slot 0 at its default placement, with camouflage empty. For the camouflage fixture, equip City (190) in slot 2 with both decal slots empty. Enter the lobby and let the native textures finish in each run.

Use Python with NumPy and Pillow. The two mesh arguments are the corresponding `ak_12.skinned_model/render/ak_12_s/converted_model`. Both DDS arguments point to the source `textures/weapons/assault/ak_12/ak_12.dds`. Each cache is its setup's own `Documents/Survarium/streamed_textures`.

```sh
python3 check_bake.py --dual-uv --original "$OLD" "$DDS" "$OLD_DECAL_CACHE" \
    --fixed "$NEW" "$DDS" "$NEW_DECAL_CACHE"
python3 check_bake.py --city --original "$OLD" "$DDS" "$OLD_CITY_CACHE" \
    --fixed "$NEW" "$DDS" "$NEW_CITY_CACHE"
```

On the same 400,000 area-weighted AK-12 surface positions, the original decal produced 10,658 yellow samples within 7.5 cm of placement and 3,068 beyond it. The new layout produced 8,863 nearby and zero beyond. With City equipped, the median difference from the original camouflage at matching positions was 3/255 in diffuse and 2/255 in smoothness. Different bake texel footprints and block compression affect the sampled colours. With City and Snufkin equipped together, comparison to City alone found 2,132 distant decal-coloured samples before the fix and zero afterward, with 4,559 samples still painted near the placement. These measurements cover the local lobby and this AK-12 placement, with `weapon-decal-uv@1`; @2 changes only where the charts lie in the atlas.

A local draw probe also encoded the diffuse and normal override flags as colours on tagged triangles. The lobby drew the AK-12 in the diffuse-only probe colour with City and the diffuse-plus-normal colour with Snufkin. The native hook check exercises all 16 channel combinations and a reset to a plain surface. A weapon's bake still covers its first eight surfaces only (patch 44's limit, `docs/notes/baked-textures.md` for why bakes are counted): 21 of the 204 models have nine to thirteen, and their further surfaces stay unpainted, as before this change. A copy of the original player's cache also rebuilt the corrected AK-12 diffuse automatically. Its DDS matched the fresh cache byte for byte.
