The original trailer canopy was solid in the 0.21 rain reference and perforated
in the port. Read-only native and original draw observations matched the same
3,283 vertices, 7,800 indices, LOD0, alpha cutoff 0.460000008 and anisotropic8
sampler with zero LOD bias. Both GPUs bound the same full-resolution image.
Every smaller mip differed: the port bound regenerated native levels, whereas
the original bound its original levels. Original lower-level coverage at the
cutoff reached 100%; native coverage remained approximately 97–98%.

The established colour-sharing rule treated equal DDS headers, full-resolution
pixels, colour space and options as equivalent even when lower mips differed.
That rule does not establish visibility equivalence. The importer now classifies
active alpha-test diffuse, explicit transparency and original masked-blend base
bindings separately: sharing
requires identical complete DDS data and options. Opaque colour textures retain
the existing rule. No alpha cutoff, sampler, geometry or mip generation changes.

An audit of all 618 imported materials found 109 shared alpha-sensitive textures;
102 have equal full-resolution RGBA but different lower-level alpha. The source
pack generator `examples/alpha-textures-021-pack.rs` copies their fingerprinted
original DDS files and options into `ported/v021d/original_alpha_mips`. The
authored edit changes 114 active texture references across 113 materials. It
copies 33 shared materials privately and rebinds only imported model settings.
Existing original visibility and vertex-blend corrections remain intact.

`check_alpha_textures.py` independently compares every packed and composed file
with its pinned original, verifies original selector/reference values, all final
material leaves and model bindings, and checks that shared native texture and
material paths remain absent from the patch. Earlier material checks normalize
only these exact independently checked later texture-reference deltas.

The 13 original masked-blend materials also consume base-texture alpha. Their
four base DDS mipchains already equal the complete originals: three use exact
native copies, and `stucco_mask_03` uses an existing private original. All option
values match too; the private mask03 options differ only in serialization. The
registered checker verifies all four effective mounted resources and 13 material
bindings, independently of the 102 newly restored textures.

Source checks do not establish visual acceptance. The combined corrected client
still needs the affected canopy/foliage comparisons and final regional sweeps.
Frozen Vostok rain and Mamayev rain/evening references remain missing; no new
original launches for those variants have been authorized.
