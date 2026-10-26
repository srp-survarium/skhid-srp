# Decal mips: Cologne's flickering graffiti

The owner, 2026-10-25: "On Cologne there is a problem with art on walls (graffiti) with mipmaps:
they flicker."

## Spec

- A projected decal's texture takes its mip from the decal plane met by the pixel's eye ray, not
  from the screen differences of the point rebuilt from the depth target.
- Only `deferred_decal.ps`'s diffuse-only permutation (`_____10______00`, every graffiti material
  and the other plain decals); the rest of the shader byte for byte the stock one.
- Content only: the pack `decal-lod@1` (`scripts/decal-lod/make_pack.py`), no exe patch.
- Not tested in game yet: the content build is statically checked; the owner's test round decides.

## Where Cologne's graffiti come from

- Cologne is stock 0.26g (`level_08`, no port). Its 9 projects hold 131 decal objects, 128 of
  them graffiti (`decals/graffiti/{paint,sign,tag}_*`, textures `decal/graffitti/*`); no other
  stock map has graffiti. PROVEN (`client_objects[game_object_type=decal]`).
- No Skhid patch touches them: patch 17 (`cologne-probes`) carries `prerain_3`, patch 30
  (`research-mode`) `day_5`, `evening_5`, `prerain_5`, all with the same 131 decals; no patch
  carries a graffiti material or texture; patch 14 changes `shaders/masks` only for
  `forward_base.ps` (one define added), not for the decal shaders. PROVEN (the patches' file lists
  against the stock db).

## What it is not

- **The textures.** 27 DXT5 DDS, 512x512 / 512x256 / 256x256 / 256x128, every chain down to 4x4
  (8, 7 or 6 levels), as every other stock decal (`decal/newspaper` 8, `decal/moss_2` 9,
  `decal/dirt_wall` 9). Decoded, each level is the box filter of the one above it (mean alpha
  difference 2-12 of 255, the same as the newspaper's and the moss's). Options as the others'
  (`has_mip_levels` 1, `srgb` 1, DXT5; `use_existing_mips` 0, as 4,291 stock textures).
- **The materials.** `decals_default`, the template and values of the other decals.
- **Z-fighting.** A map decal has no geometry on the wall: `stage_decals_accumulate` draws its box
  and `deferred_decal.ps` projects onto whatever the G-buffer holds there.
- **The projection box.** 0.26g's box (decal_instance 0x14DBD0, D3DXMatrixOrthoLH 0xEEF40): the
  decal's full `decal_width` x `decal_height`, depth from 0.01 to `decal_far_distance` behind its
  position (the direction offset of 0.10b is multiplied by 0). The 128 graffiti's walls lie in
  their boxes (99 % of painted texels, by ray casts on the map's render models); 10 sit 0.6-1.7
  cm behind the box's front, 24 % of painted texels in its back half (the radial smear term);
  in the simulation below neither makes pixels come and go (one of the 10 at 45 m: about 1 % of
  its pixels differ from an exact depth, steadily).
- **Draw order.** The visible decals are `std::sort`ed by `draw_priority` (0x179650); every
  Cologne decal and every bullet mark (default constructor 0x14D080) has 0, and MSVC's sort leaves
  an all-equal list in its order: overlapping graffiti keep theirs.

## What it is

`deferred_decal.ps` (the stock blob, `vkd3d-compiler -x dxbc-tpf -b d3d-asm`):

```
sample_l r1, uv, t2 (the position target), s1     depth
mul r1.xyz, r1.x, eye_ray                          the point, view space
... into the world (cb1[11..13]), into the box (cb0[0..2]), discard outside
sample r0, tc, t0, s0                              mip from ddx/ddy(tc), per 2x2 quad
```

The position target is `DXGI_FORMAT_R16_FLOAT` (create_targets 0x162DA0: rt 9 `$user$position`,
format 0x36): linear depth with 11 bits, steps of 1.6 cm from 16 m, 3.1 cm from 32 m. On a wall
seen at an angle the rebuilt points lie in steps a few pixels wide, and `tc`'s screen differences
jump at every step edge: the mip comes out in bands, which move over the graffiti as the player
moves; every depth edge inside a box (planks, pipes, window frames) takes the smallest mip. Thin
high-contrast tags on walls show it; Cologne is the only map with them.

Simulated (the map's render models rasterized, the shader's math in numpy, the camera swaying by
millimetres; scratch tools, not in the repo): the LOD of a graffiti seen at 15 m and 30 degrees
is striped with an R16 depth and smooth with an R32 one; frame-to-frame change on graffiti pixels
0.0302 (R16) against 0.0256 (R32), 0.0212 against 0.0155 at 29 m, 0.0158 against 0.0136 at 30 m.

## The fix

`scripts/decal-lod/make_pack.py` gives the stock program the gradients and swaps `sample` for
`sample_d` (the derivation in its docstring). They are a smooth function of the screen position,
scaled by the depth, so the steps no longer show and depth edges no longer spike. The same
simulation: 0.0267, 0.0159, 0.0145 (most of the R16 excess gone). Run on a synthetic camera and
decal, the patched program's gradients equal the differences of `tc` between neighbouring pixels
to 0.05 %. The container's checksum is the DXBC MD5 variant, checked against the stock blob.

On a surface not parallel to the decal (a tilted plank) the mip is the decal plane's, a little
sharper than the surface's own; Cologne's graffiti are laid parallel to their walls.
