# Original Vostok 0.11e evening and rain

Work in progress: native-shader comparison content, not released or validated
for visual or moving-camera rain parity. The resource-porter manifest is
`ports/survarium-011e-radar-weather-to-026g.json`, with independent `v011w`
assets and map IDs 249–252 (evening PD/TDM, rain PD/TDM).

The original weather projects differ from the later daytime layout. Their
0.11a server fallback is hash pinned and strictly checked against the primary
0.11e clients, with one explicitly reviewed zone-mesh reconciliation.
The maps have no authored resupply/artefact objects corresponding to the day
map's table overrides; their map rows therefore leave those overrides empty.
Adding four map rows also advances Blackout's strict append counts by four.
Each new project gets the existing Keeper pool, and the new maps' match
settings are recorded in `simulation_changes.tsv` as `next`.

The current pack is `v011e-radar-weather@4`. Version 1 is the private native
baseline; version 2 selects existing 2048 mips for the three vehicle atlases,
byte-identical to the merged Radar streaming fix. It keeps the original cloud
texture at 4096x1024. Version 3 adds seven complete, unchanged original shader
files under `_v011w` family names (post, fullscreen VS, present, rain VS/PS
and rain-resolve VS/PS), plus the original grain/rain DDS and options. The
5,984 version-2 files are byte-identical; only these 11 files are added.
The shaders are available for the renderer adapter, not yet selected at runtime.
All four private quick content builds pass, including patch
readback and server game data. These are not full release builds. The
content-edit suite passes 66 tests, and localization validation passes.

Version 3 pack.json SHA256:
`70b472e72c71e9dca9543062e274f17c13a34f457e266698d16ddd3863f03759`.
Its private content build is `/tmp/v011-weather-content-original-shaders-v3`:
6,011 patch files, 917,675,520 bytes, patch SHA256
`5ca341c84b29e2896d8d4c5a76cbee265bca5aba546ef9bea772994b939668f0`;
server game data SHA256
`3061f8cfd53c27bc3432337dc74c29d97dbb46361fae0eaf42ff5e58bf7daae3`.
Version 4 corrects the post-program selection: all nine distinct original
post programs are now imported, with explicit variant family suffixes. The
unsuffixed post family is the authored quality-3 bokeh/image/grain program,
not the quality-2 program staged in version 3. The original 128x128 BC1
`fx/bokeh_image` DDS/options are also included unchanged. There are 6,005
bundle files: all 5,984 version-2 files are unchanged, plus 15 shaders and six
texture/options files. No renderer activation is included yet.

Version 4 pack.json SHA256:
`f8873d65239db4fd5eaec99375a6fc22812fd5e5dbb7a24f54f7cbdfe0815c54`.
Private quick build `/tmp/v011-weather-content-post-variants-v4` passes
patch/readback validation. Patch size 917,779,787 bytes, SHA256
`a4bc7008e8bcb92355bb42e05ccc5ec328ab5c0bd7f8dddc3b750e0616754ff0`;
server game data SHA256
`fb2c92508db037ec339ede043e58da58f4dc7c326c8fb5f9c6e5c05d8b488188`.
The stock loader check passes all 15 shader wrappers. The importer tests
pass. Strict all-targets Clippy reproduces the same 66 existing diagnostics;
none were added by the variant support.

Scratch patch number 57 is not a released or reserved revision.

Private comparison work is under `/tmp/v011-weather-reference`: original
evening/rain recipes, a 128-position collision-checked sweep, read-only stage
observer, and the guarded capture queue. The initial native baseline uses
`/tmp/v011-weather-content-baseline-v9`; the atlas correction uses
`/tmp/v011-weather-content-atlas-v2`. Original references must be frozen and
kept. Neither original-weather capture nor native visual parity is established
while the owner-game guard is waiting.
The further original sweep is queued after those jobs: three batches per
weather (48/48/29 new poses); three exactly matching cameras reuse the initial
references. Its plan is `/tmp/v011-weather-reference/sweep/plan.json` and log
`/tmp/v011-weather-reference/sweep.log`. No original frame is overwritten.

## Evening fog numeric type

Original evening `volume_fog:environment_and_lights/volume_fog` has
`density_offset` float 5.2. Rain has integer 5; the native corpus happens to
contain only integers at this path. Preserve 5.2 with two exact new-file/path
exceptions in `release/build_content.py` (evening modes 2 and 4).

Verified against stock executable SHA256
`ac33d9937015c2299bf62cc6b127cb308b48a3475b30fff2ddd65479ae3bf153`:

- `survarium::object_volume_fog::load` starts at VA `0x2911c0`, takes the
  object in ECX and config at stack+4, and returns with `ret 0xc`.
- At `0x291362`, ESI points to `density_offset` at `0xa0ad84`.
- `0x29137b` compares the value's type word at +0x14 with 2. Float takes
  `movss` at `0x291382`; integer takes `cvtsi2ss` at `0x291388`.
- `0x29138c` stores the resulting float at object+0x178. The insertion path
  reads this float at `0x291531` and passes it in the fog parameter structure
  to `scene_renderer::update_volume_fog` at `0x29155b`.

The reviewed symbol and observed ABI inputs are in `tools/re-kit/v026g`.
This verifies type compatibility, not the fog shader's appearance.

## Rain stage boundary

The same stock executable verifies the existing rain-stage ABI rows:
`stage_rain::stage_rain` (`0x1ad830`) takes its receiver in ESI, stores EAX
(renderer context) at receiver+4 at `0x1ad83f`, and ECX (renderer) at +8 at
`0x1ad842`. `stage_rain::execute` (`0x1adb00`) takes the receiver in ECX,
preserves it in EBP at `0x1adb10`, and reads its context at `0x1adb52`.
Those four ABI inputs are now observed. No hook or executable change has
been made from these findings.

## Original post shader binding boundary

The unchanged 0.11e post pixel shader (SHA256
`3eefb58778b7932d90b8f4af638dcc077ab6b59a9076bfa45e3e44de55f976af`)
uses both filmic parameter vectors. Merely importing the original material
and replacing the native shader cannot supply them:

- Original `scene_shader_constants::set`, PDB VA `0x603ec0`, reads the first
  vector from post parameters +0x23c/+0x240/+0x244/+0x248 and writes host
  receiver+0x14 (`0x6040d6..0x604177`). The second reads
  +0x24c/+0x250/+0x254/+0x44 and writes host+0x18
  (`0x60417d..0x604214`). The final component is dark desaturation.
- Native constructor `0x1af7c0` still registers these two names at +0x14 and
  +0x18. Native setter `0x1afb10` never uploads either. The 0.21d PDB-labelled
  constructor/setter (`0x1258b4`/`0x125b1f`) confirm the matching class and
  the later setter's omission.
- The native setter takes the receiver in ESI, height lights in ECX, fade
  colour in EAX, and five stack arguments: mid tones, shadows, gamma scalar,
  grain float4 reference, environment reference. It returns with `ret 20`.
  Height-lights and fade fourth components are fixed to 1. Environment
  +0x1f8 supplies blueshift and +0x230 supplies bloom scale. Grain is still
  uploaded unchanged, through receiver+0x28.
- Native backend upload `0x132a90` takes backend in EDI, host in EAX and
  data reference at entry ESP+4 (`ret 4`). It checks the host's update marker
  against the backend singleton and uses reflected slot/size information.
  This is the folded scalar/vector `set_ps_constant` family (0.21d
  `0xda127`); the caller must supply the active backend and bound shader.

`tools/re-kit/v026g/check_post_constants.py <stock-exe>` executes the native
constructor and setter in Unicorn with intercepted string/host/backend
operations. It passes 64 cases with distinct vector components, confirms all
11 host offsets, eight uploads and the missing filmic pair (also the unused
eye-ray host). It checks stack cleanup and rejects a different executable
hash. This establishes the CPU contract only. Texture producers, colour
space, original presentation, scene ownership and actual GPU output remain
to be validated. No production runtime hook has been added yet.

## Original shader archive compatibility

Stock `shader_binary_source_cook::converted_shader_loaded` (`0x3dc950`)
accepts a continuation payload at entry ESP+4 and query result at ESP+8,
returning with `ret 8`. The original and native resource wrappers have the
same layout: u32 source-record count, then count times 264 bytes, then DXBC.
At `0x3dca69..0x3dca92`, the native loader computes that offset and stores
the bytecode pointer at resource+0x120 and remaining size at +0x12c.
The continuation's resource is +0xc. The two ABI rows are now observed.

`tools/re-kit/v026g/check_shader_wrapper.py <stock-exe> <bundle>` executes
these stock instructions for all seven imported files. Each resulting pointer,
size and DXBC byte sequence matches the hash-pinned source. This tests the
wrapper arithmetic after pinning, not the resource manager or GPU creation.
The porter additionally checks complete-file and DXBC hashes, chunk extents,
shader-model/stage consistency, safe names, and collisions with native shader
families. Original textures include their original options and all mip bytes.

The native `copy_image.vs` takes POSITION/TEXCOORD, while the original takes
POSITIONT/COLOR/TEXCOORD. The original's UV output is register 2; native's is
register 1. Reusing the original VS therefore needs the original fullscreen
geometry/input layout. Replacing only a PS or supplying the native quad to the
original VS has not been established as compatible.

## Original post quality and bokeh selection

Both authored weather materials enable `use_bokeh_dof`, `use_bokeh_image`
and `use_image_grain`. The earlier 5,528-byte shader (`3eefb587...`) only
covers quality 2 with grain, or quality 3 with bokeh disabled. At quality 3,
the weather settings select the 6,744-byte program SHA256
`8696aa4bd4c7ca1091957cd27659b1658e0a461d1a7c8014f2ba82a19fcae105`.

Original stage execute `0x607160` selects its effect at
`0x608445..0x60847a`: option singleton `0x8c20d70`, quality +0xc0;
post parameters in EDI, bokeh DOF +0x21, bokeh image +0x2c, grain +0x2d;
stage in EBX, effect array +0x80. Quality zero disables all three flags;
otherwise bokeh image requires DOF. `check_post_selection.py` in
`tools/re-kit/v011e` executes these original instructions for all 32 input
combinations and resolves the selected original catalog permutation. It
passes, with authored weather quality 0/1/2/3 hashes respectively
`4afab015...`, `88a0cc79...`, `3eefb587...`, `8696aa4b...`.
This is CPU evidence; the queued stage captures still must confirm the live
selection and loaded parameters.

The quality-3 weather program's resources are position t0, grain t1, frame
t2, luminance t3, scattering t4, bokeh image t5, lens flares t6, grading 3D
t7. It does not sample `t_blurred_frame_color`; quality-1/2 programs do.
Its active constant-buffer values additionally include `bokeh_dof_parameters`
at +176; grain and filmic offsets remain +256/+272/+288. Consequently, the
quality-2 texture-slot description above must not be used for quality 3.

Native DOF constructor/setter `0x1af590`/`0x1af6d0` were checked against
0.21d `0x1256f9`/`0x1257e3` and executed in the extended
`check_post_constants.py`: 64 additional cases pass all four host offsets,
upload values and `ret 24`. The native setter retains bokeh radius/density
and uploads them unchanged; it hardcodes focus power to 1.0. Receiver ESI,
highlights reference EAX, stack scalars distance/region/near/far/radius/density.
The constructor uses EDI. These names and observed ABIs are in re-kit.
Whether the actual native post-stage caller supplies the authored weather
values remains a capture/integration check; constructor/setter tests do not
prove their runtime inputs or shader binding order.

## Original post replay evidence (pending GPU execution)

The read-only stage observer now records the original post VS/constants,
indexed TL geometry, rasterizer/output state, full sampler descriptors and
additional grain/bokeh texture mips. The existing screenshot-only batch keeps
its frozen DLL. The two still-pending rich diagnostic clients were checked
idle, their previous DLLs preserved as `camera_hook.before-post-observer.dll`,
and their recipes repinned to the new observer. Both recipe preflights pass.

Observer SHA256: `09bf1286b532997a6d9d330cd7b8cea21811c819c9555617de1d11df71a4a9bf`.
Replay executable SHA256: `4ea096eab63c2e95bc920f4c4d74af3f26d6ab6aeb8fbc7aef9f45597f19eba0`.
Both compile with `-Wall -Wextra -Werror`.

Resource-porter `tools/render-compare/replay_post.py` prepares a verified
packet from a `.post-draw` capture; `replay_post.exe` runs the unchanged
original VS/PS, geometry, constants, samplers and textures outside the game.
The comparator reports byte equality and component error against the frozen
original draw. It refuses missing mips, constants, altered programs, invalid
geometry/viewport/format sizes and modified replay packets. Seven such
rejection checks pass using real bytecode and synthetic data.
No GPU replay has run yet, and no original weather frames exist yet. The
three capture guards remain live; another owner/test game instance currently
matches the guard. Always check the full process pattern, not only a previously
observed game PID, because a replacement game may already be running.

Next: once the rich captures arrive, prepare each captured original post draw
and replay it under the guarded headless workflow. A byte-matching original
round trip establishes the draw adapter before binding native scene inputs.
The current content still uses native runtime shaders; no renderer hook or
moving-rain parity is implied by this preparatory tooling.
