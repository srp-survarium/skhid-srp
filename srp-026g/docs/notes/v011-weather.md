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

The current pack is `v011e-radar-weather@5`. Version 1 is the private native
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
kept. Both initial original capture batches are complete; native visual parity is
not yet established. The owner explicitly authorized concurrent isolated GPU
captures on 2026-11-12 ("no you have enough GPU"). The private queue retains
sequential render jobs, the 2.5 GiB free-VRAM check and dummy-Xorg isolation.
With the owner game and original capture running, 4,122 MiB remained free.
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
Live captures now confirm the quality-3 selection in both weather variants;
see the GPU round-trip evidence below.

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

## Original post replay: GPU round trip passed (2026-11-12)

The owner explicitly authorized concurrent isolated GPU captures ("no you have
enough GPU"). Jobs still run sequentially on dummy Xorg with at least 2.5 GiB
free VRAM. Both initial reference batches completed: 32 poses × two frames per
weather, 128 frames total. All actual positions, directions, FOVs and resolutions
match; see `/tmp/v011-weather-reference/original-camera-validation.json`.
Initial references are frozen and unchanged. Their frozen-manifest hashes:

- evening: `a674c3bb21645b503662a7b3af60146d6a7aa4204e811b2d4c58f1db37e79a65`;
- rain: `8bf442cb50ee0c7e34e53d8daefd094c944bdd7a51d868375168916ccecb41fd`.

Live diagnostics found two capture defects. The 0.11e initialization path never
armed buffer readbacks, so the first diagnostic batches contained only images.
It now enables the original-client observer without native stage hooks, and the
runner rejects missing post-stage evidence. The grading volume has three mips;
readback now records the full volume chain, preserving the existing mip-zero
format. Replay reads the actual `.post-output` filename. Additional diagnostic
captures went into new `rich/armed-*` and `rich/full-*` folders; earlier frozen
images were not replaced.

Full observer SHA256:
`43b89a5d33ff10e054b5d7409a6f8f73ab8e4ee04348dd4dae18b7b4d385e8bc`.
It builds with `-Wall -Wextra -Werror`. The CPU packet/comparator check passes,
including eight rejection cases and missing volume-mip coverage.

All eight original post draws (two poses × two frames × two weather variants)
use unchanged quality-3 PS `8696aa4b...` and copy-image VS `29abfee6...`.
Their full-mip packets replay on the GPU with **zero differing bytes** in every
3,686,400-byte 1280×720 R11G11B10 target. Results and input hashes:
`/tmp/v011-weather-reference/post-replays/results.json`, SHA256
`520206f6dc04ce5a3b0b1bc8dfa61b70b3e41d15c820cf791d97e614e79cd313`.
This proves the original post draw with original inputs; it does not establish
native scene-input adaptation, whole-renderer parity or moving-rain parity.

Original rain diagnostics confirm five shells in each captured frame, radii
5, 6.6000004, 8.4700003, 10.6480007 and 13.1769009, density 0.8 and speed 2.
All five share one scene time per frame, and time advances between frames.
`original-rain-layers.json` records the constants, geometry and camera evidence.
These are fixed-camera captures, not a walking test.

The native baseline uses the existing scratch content build 57. Its first
server startup correctly rejected minimum revision 56. Only the private capture
EXE's declared revision and PE checksum were aligned to scratch 57, with the
server and test bot matching; source REVISION and release state are unchanged.
`native-scratch-revision.json` records exact hashes and changed offsets.

The current queue is `/tmp/v011-weather-reference/complete_queue.py`, log
`complete-queue.log`: native baseline plus shader-state captures, image
comparison, then the original 128-position sweep. Native renderer integration
and walking-rain verification remain outstanding; nothing was released.

## Native baseline and pre-post deficit (2026-11-12)

Both native baseline batches completed: 32 matched poses × two frames for
both weather variants. These use the version-1 content baseline, not the
version-4 atlas/shader asset pack. Camera validation and comparison completed;
`/tmp/v011-weather-reference/baseline-report/{evening,rain}.json` contain all
64 per-weather results, with `*-worst.png` contact sheets. Mean display RGB
RMSE on the fixed comparison mask is 0.2033916655 for evening and 0.1366797850
for rain. These are baseline measurements, not acceptance thresholds.

The deficit is present before post-processing. Matching captured
`t_frame_color` inputs are both R11G11B10_FLOAT; the fixed comparison mask has
no nonfinite pixels. Mean linear luminance:

| Weather / view | Original | Native baseline | Native/original |
| --- | ---: | ---: | ---: |
| Evening / factory interior | 0.0164660024 | 0.0051916427 | 0.315295 |
| Evening / north yard | 0.0246467974 | 0.0090967289 | 0.369084 |
| Rain / factory interior | 0.0123134693 | 0.0036365242 | 0.295329 |

Receipts are `evening-post-frame-comparison.json`,
`evening-yard-post-frame-comparison.json` and `rain-post-frame-comparison.json`
in `/tmp/v011-weather-reference`. This localizes a significant difference
upstream of post; it does not identify one lighting producer as the sole cause.
Original composition consumes encoded R10G10B10A2 light accumulators, whereas
native composition consumes linear R11G11B10 accumulators and adds sun lighting
there. Their raw accumulator bytes cannot be compared as equivalent radiance.

Native evening grain is 0.06 at this resolution although the imported project
correctly retains the authored intensity 0.09. The stock environment loader
`0x34e1e0` reads `image_grain_intensity` at `0x34f36f` and stores it at
record+0x20c (`0x34f399`); record and config are entry stack+4/+8. The native
post stage at `0x1b24da..0x1b251c` multiplies authored intensity by a selected
integer rendering option and the stock float 1/1920. For selected value 1280,
this explains 0.09 becoming 0.06. The option's identity is not established by
this bounded check. `check_post_constants.py` passes 30 real-instruction grain
cases in addition to the existing 64 scene and 64 DOF cases. Loader and input
findings are recorded in the 0.26g re-kit.

The post replay preparer now supports explicit named texture substitutions
from another captured draw. It preserves the original shaders, geometry,
constants and all other textures, requires identical storage/view contracts,
hashes every input, and labels the result as an input sensitivity experiment.
CPU checks pass unchanged-input identity, changed-input propagation and 11
rejection cases. Eight native-frame/native-frame-plus-luminance experiments
(two views × two weather variants) are prepared at
`/tmp/v011-weather-reference/input-substitution`, queued after the original
sweep. They have **not yet executed on the GPU** at this checkpoint and do not
constitute a runtime adapter.

The current native stage captures lack vertex geometry/full-mip/rain-count
flags. The private runner now sets those for subsequent launches; the existing
stage buffers remain useful for the measurements above. Production renderer
integration, a final native sweep and moving-camera rain verification remain
outstanding. Nothing was released.

## Live original-post diagnostic bridge (2026-11-12, tests queued)

The porter now has a reusable original-post command list (`post_replay.h`) and
an opt-in native observer bridge (`original_post_live.c`). Both compile under
MinGW with `-Wall -Wextra -Werror`. This is diagnostic code outside the client
patch arena; no production runtime hooks or release change are included.

The bridge selects the exact observed native composition/post bytecodes,
retains current-frame depth, and validates texture dimensions, formats and mip
chains before using live frame/luminance/grading data. It packs native linear
scattering for the original shader, uses original grain/bokeh assets and
weather constants, updates grain phase, executes the unchanged original VS/PS,
and copies the private result into the native post target. Native drawing is
retained when an input contract fails. It runs only with the private capture
camera enabled. Lens flares are absent and native downstream presentation
remains; neither full appearance nor moving-rain parity is established.

The reusable command-list harness checks malformed packets, repeated draws and
restoration of viewport/rasterizer/CB/SRV/VS/RT/topology. Eight GPU comparisons
against the existing exact original post outputs are queued in
`run_input_substitutions.py`, after the remaining source sweep. Only if all
match does `run_live_original_post.py` launch two poses × two frames for each
weather in `/tmp/v011-weather-native-original-post`, using the unchanged
version-1 baseline content to isolate the shader change. No live-test pass is
claimed yet. Build inputs and DLL/EXE hashes are in
`/tmp/v011-weather-reference/live-post-build.json`.

All three evening reference sweep batches are now frozen (125 new locations,
plus three reused initial cameras). Rain sweep batch 1 is running at this
checkpoint. Initial references remain unchanged.

## Original gamma copy and pack 5 (2026-11-12)

Captured final-copy instructions establish another adapter requirement:
original `copy_image.ps` SHA256
`685c00b3451a1fb3291f40696e7b4ae34b0b18f6bad32ab62e634b5f8f9cb451`
computes `pow(abs(color), 1/(2.2*gamma))`; native `495af559...` computes
`1.055*pow(abs(color), 1/(2.4*gamma))-0.055`. Their UV inputs are registers 2
and 1 respectively. The observer calls this `.present-draw`, but the original
archive's separate `present.ps` (`0a747779...`) is a plain blit. It cannot
supply the observed gamma conversion. The captured post output equals the
captured gamma-copy input byte-for-byte in all eight inspected combinations
(two views × two weather variants × original/native); those views provide no
evidence of another intermediate image modification.

Pack `v011e-radar-weather@5` adds the unchanged original gamma-copy shader.
All 6,005 version-4 overlay files remain byte-identical; only
`shaders/sm_4_0/copy_image_v011w.ps/<67 underscores>` is added (6,006 total).
Pack JSON SHA256:
`e002c5352c079e6c00183ccab24d79af5edadf8bd93969ff70cd4f4992347a7a`.
The manifest row uses version 5. The stock-wrapper Unicorn check passes all
16 original shaders. The manifest content build passed at
`/tmp/v011-weather-content-gamma-v5`. This is a local QUICK-BUILD: mounted
players-file verification, site pages and item tables were not requested; it
is not release validation. Patch SHA256:
`79603a9fd8c419a0b768131e4ce137a41692f90e4fc147b309e4e8c2e686ad15`;
server-data SHA256:
`993e40504ef82d95040133820e41008aa7799870b69db06520570982238bb8d7`.

The diagnostic bridge now requires paired post/gamma-copy packets, validates
the current device and full viewport, and draws the original gamma copy only
after a successful original post in the same frame. It copies the live gamma
constant and current input, preserving native context state. Both programs
compile with warnings treated as errors. CPU packet checks pass presentation
preparation plus 13 rejection cases, including missing/different-frame quad
sources. The presentation packet explicitly derives its TL quad from the same
frame's original post capture. All eight original-post and eight original
gamma-copy command-list GPU comparisons passed byte-for-byte, including
repeated execution and native-context restoration. This establishes geometry
reuse for these eight captured gamma-copy draws. Result manifests:
`command-replays/results.json` SHA256
`423a9e41c81f21f2fd89c3028ca5def92e30e26125c1420e1f0a8dcfb2340787`,
`presentation-command-replays/results.json` SHA256
`b83216d5b833b45ed1df5de609fe5d6c831c2279e7c95c405f15f8dd5a78d52a`,
both under `/tmp/v011-weather-reference`.

The original skylight bytecode provides a useful next boundary: its pixel
shader reads only normal XY and performs the same sphere-map reconstruction,
then uses original skylight basis vectors. Its original vertex shader is
`9dab4eced49b2980e4e70843a6f42e9f59a84bf6e696121bc0c931e685a9f6f4`,
with POSITION float4/TEXCOORD float2 inputs, UV output register 1 and basis
outputs 2/3/4. Its sole constant is `skylight_parameters6` at b0 offset 0.
This shader was extracted and hash-checked against the 0.11e catalog in
`/tmp/v011-weather-source/skylight-vs-original.{dxbc,json}`. This is binding
analysis, not a skylight implementation or a visual-parity result.


## Completed GPU diagnostics and source sweep (2026-11-12)

The earlier queued checks are complete. The live original post/gamma bridge
executed 660 paired draws for evening and 480 for rain, with zero rejections.
Each capture has two views and two frames; camera position, direction, FOV and
resolution agree with the original and native baseline. Reports and contact
sheets are `/tmp/v011-weather-reference/live-post-comparison/{evening,rain}.{json,png}`.
The live test deliberately retains version-1 content to isolate the shader
change; it does not validate version-5 content in play.

Display RGB RMSE (baseline → bridge), first frame:

| Weather / view | Baseline | Original post + gamma |
| --- | ---: | ---: |
| Evening factory | 0.104025 | 0.099595 |
| Evening yard | 0.112383 | 0.111317 |
| Rain factory | 0.083476 | 0.076492 |
| Rain yard | 0.066650 | 0.064147 |

The modest improvement does not resolve the dark lighting. Rain factory mean
luma is 0.07751 versus original 0.12991; rain yard is 0.09692 versus 0.15071.
Screenshots were visually inspected. Lighting before post-processing remains
the next boundary; neither full renderer parity nor moving-rain parity is
established. The bridge remains a diagnostic, with CPU scattering conversion,
absent lens flares and no production scene lifecycle integration.

Eight controlled native-frame/native-frame-plus-luminance substitutions also
finished. Each pair produces identical output in these views, consistent with
the observed exposure clamp; this is not a general luminance-independence claim.
Metrics: `/tmp/v011-weather-reference/input-substitution/metrics.json`, SHA256
`5a1d4b621db33957f5243b45765bab6d0f4d3e499e2cf530488b98a4d8951c57`.

All six original sweep batches completed. Frozen file hashes and actual camera
metadata were verified: 500 new frames plus 12 reused initial frames, covering
128 locations per weather variant. Receipt:
`/tmp/v011-weather-reference/sweep/completion-receipt.json`. Initial originals
remain unchanged. A final native sweep is still required after renderer fixes.
Nothing was released.


## Exact original skylight replay (2026-11-12)

`replay_skylight.py` now prepares the unchanged original skylight VS/PS for the
reusable D3D11 command list. The first diagnostic reconstructed the clip quad
and omitted depth/stencil state. RGB on pixels changed by the captured draw
agreed within one 10-bit step, but alpha and about 1,000 untouched pixels per
view differed. This was not accepted as full equality.

Additional isolated original captures established the missing contract:
RGB-only write mask 7; depth disabled; stencil enabled with reference 0,
read mask 255, write mask 0, NOT_EQUAL on both faces and KEEP for every stencil
operation. The bound surface is R24G8_TYPELESS with a D24_UNORM_S8_UINT view.
The original quad uses POSITION float4/TEXCOORD float2, stride 24, six R16
indices and front-face culling. Its captured VS bytecode equals the catalog's
`9dab4eced49b2980e4e70843a6f42e9f59a84bf6e696121bc0c931e685a9f6f4`;
its VS constant equals the original PS's `skylight_parameters6` bytes.

The default preparer now requires the captured geometry, VS constant,
write mask and stencil state/surface. Reconstruction remains an explicitly
labelled experiment. Eight complete GPU replays (two views × two frames ×
two weather variants) match every target byte: zero differing bytes out of
3,686,400 per draw. Both existing post/gamma regression paths also remain
byte-identical after extending the shared command-list implementation.

Evidence under `/tmp/v011-weather-reference`:

- `skylight-state-replays/results.json`, SHA256
  `857a012bb1745fb03a5a591f5ee6775e2f01bff3ed1f364bced4dccfd7644ede`.
- `skylight-state-replays/restore.json`, SHA256
  `be5ed04dc18f8f26c665ec70a2d99705cfe871f917556ff1bb9e98318ca5768a`:
  additional GPU test verifies depth/stencil reference, blend factors/mask and
  the previously checked context state after two executions; output is exact.
- `skylight-state/completion.json`: frozen file hashes and actual camera
  metadata verified. Frozen manifests: evening
  `13c7a4ed39ff857dd1181b662c27d6f47a736f919842afe37feee0001d81e897`,
  rain `1114c17ffd9430c4dbdd941a689279d4013b03f5ebab79411852aed52a102e2b`.
  Earlier original references remain unchanged.

The DLL and command-list harness compile with `-Wall -Wextra -Werror`.
CPU preparation checks pass the valid captured draw and 14 rejection cases.
This verifies an original pass in isolation, not native skylight integration.
Next: provide its native depth/normal/AO inputs, preserve the original encoded
light accumulation and probe ordering, then compare the live result. The
post/gamma bridge remains the only live original-shader diagnostic added by
this weather work; production integration, final native sweep and moving-rain
parity are still outstanding. No release or weather PR was made.


## Original temporal SSAO and connected skylight (2026-11-12)

The captured 0.11e weather profile uses temporal SSAO. The older diagnostic
`legacy_ssao.c` runs accumulation/spatial upsample without temporal history;
it is not a complete producer for this profile. The actual original draws are:

| Stage | Pixel DXBC SHA256 | Targets |
| --- | --- | --- |
| Accumulation | `0420583bf092cbf6d2cda26fa446e58be777980f32c5bc7af40649317a562f85` | half-size R16G16_FLOAT |
| History mask | `c8e117ebc6186d7184631beab59ca9981be8362452f5aa845348d19fede383c6` | half-size R8G8_UNORM |
| Temporal resolve | `5a022edc69d5e2c3e9452dea90de45cd8c07fb45040b621daacb441ddf680326` | full-size R8G8_UNORM AO and R16_FLOAT depth |

Accumulation retains a full-size viewport over its half-size target. Its
original `post_process0.vs` is `3f081e9239ab97ad23d8f329e15703e398804b18e74a90c5c95de6062c1a7cb0`,
using a 36-byte POSITIONT/eye-ray/UV quad. Mask/resolve use the 24-byte clip
quad and `81ac78836039e3e79ee8e5fa150d9689ea18417b50b24453ba39325bc4c116af`.
Temporal resolve retains LESS_EQUAL depth testing without depth writes and
nonzero-stencil coverage. Both MRT outputs are required by the history loop.
The shader fixture now extracts 82 unique original programs across 26 families.

`replay_ssao_011e.py` prepares all three captured pass contracts;
`replay_post_command.exe` supports their geometry, viewport and second target.
All 24 independent GPU draws (three stages × two views × two frames × two
weather variants) match the original bytes, including temporal depth output.
The shared harness still passes the existing post, gamma and skylight cases.
CPU preparation checks pass three valid paths and 31 rejection cases. Strict
MinGW builds pass for the observer, command-list harness and new chain harness.

`replay_ssao_chain.exe` connects GPU-produced accumulation and history mask
to temporal resolve, then its GPU-produced AO to the unchanged skylight pass.
All eight chains match original AO, saved depth and skylight output byte for
byte. The executable runs each chain twice. Twenty-four same-frame captured
producer/consumer links were also independently checked byte-for-byte.
Previous-frame AO/depth remain captured inputs; these tests do not establish
history initialization, camera motion, native integration or scene teardown.

Evidence under `/tmp/v011-weather-reference`:

- `ssao-replays/results.json`, SHA256
  `cfc00eee2876d68de74d636dfb88d1bcad5b60e6712f32558b34770add970dac`.
- `ssao-replays/lineage.json`, SHA256
  `6073e0107d986e3fd86cf93d8d38703649653f91a452d2cd59541ffb510d6752`.
- `ssao-chain/results.json`, SHA256
  `4d3dab32fedddccbb1947ed9345f48eb0b86a6338ed90092ca84bccb3a9a712c`.
- `ssao-state/completion.json`: actual cameras and all frozen files verified.
  Frozen manifests: evening
  `c4151dca0aab9c7fe480a663addf6879a34e0a5390d42b3f05e89d65b55a4920`,
  rain `e1a44bf75cba4dcd758e0f51e0414df6082bfc29eb9aecd50d39430558815031`.
  Earlier originals remain unchanged.

Original build-884 PDB identifies `stage_ambient_occlusion::execute` at
0x00704940, 1717 bytes, and `execute_disabled` at 0x00704530, 228 bytes;
these identities were added to the 0.11e re-kit. The former's bounded original
instruction listing is `/tmp/v011-weather-source/ssao-execute-original.asm`,
SHA256 `0192dee57e9ea43c3a8ae7672cd00b455517e13b9a6654b062bc7b42ed3f34fa`.
The original source catalogs have identical `effect_ssao_filter4x4.cpp` MD5
`7ce10a018bfb83fb5e9316b846d8c43a` in 0.10b/0.11e, but different
`stage_ambient_occlusion.cpp` hashes: `0a7590133c06b33627eff8e6cfb0eec9`
versus `ba341fd40046ce631a055c31524b4542`. Do not infer the complete 0.11e
runtime from the reconstructed 0.10b implementation.

Next live integration needs one owner for current depth/normal/stencil,
previous AO/depth/view, and the four original passes. Verify native G-buffer
formats, stencil meaning and camera constant layout before feeding this owner;
keep geometry coverage and the encoded accumulator/probe ordering intact.
Initialization, camera discontinuities, resize and scene teardown remain
unverified. Production integration, final native sweep and dynamic rain remain
outstanding. The weather content remains version 5; no release or PR was made.

## Original ambient pipeline on native inputs (2026-11-12)

`original_ambient_live.c` now owns original accumulation, history mask, temporal
resolve and skylight passes, with previous AO/depth/view, in the render observer.
It runs once per frame at the first recognized native probe. All outputs remain
private: native probes, light composition and final presentation are unchanged.
This is diagnostic integration, not a completed weather renderer.

Native input captures established these boundary mappings:

- Depth is R16_FLOAT; the physical stencil surface is R24G8_TYPELESS with a
  D24_UNORM_S8_UINT view. The original nonzero-stencil coverage test is usable
  at this boundary. Native and original stencil numbers differ; their nonzero
  geometry coverage is not claimed pixel-identical.
- Native normal XY is R16G16_FLOAT, using the same sphere-map encoding as the
  original R10G10B10A2_UNORM XY. The unchanged skylight shader reads the native
  float2 texture directly. No repacking/precision loss is introduced.
- Native global constants are 416 bytes: the first 224 bytes contain the same
  projection/view transforms. Native screen dimensions at 224 map to original
  offset 288 in a 304-byte buffer. The current native projection/jitter is kept.
- Native probe CB0 is 208 bytes; eye-ray XYZ at 112 maps to original mask/sky
  offset 16 and temporal offset 20. Accumulation receives those live rays in
  its original 36-byte quad vertices. Preserve captured index offsets and base
  vertex: the evening packet uses base 0, the rainy packet base 7. An initial
  overly restrictive quad guard rejected rain before drawing; the corrected
  owner resolves the indexed quad within the captured allocation.

History is empty or valid with previous present, capture-camera ID and view.
Initialization, a present gap or a new capture-camera ID zeros previous AO/depth
and the history mask. The unchanged original temporal shader then takes its
current-AO-only branch. Subsequent frames connect GPU-produced accumulation and
mask to temporal resolve, connect its AO to skylight, and retain both outputs
needed by the next history step. Mismatched input/device/resolution contracts
stop and release the diagnostic instead of substituting arbitrary inputs.

Each reset and settled capture records five output targets, previous AO/depth,
current depth/normal/stencil, mapped constants and eye-ray geometry. Independent
`replay_live_ambient.py` packets retain the exact original shader identities and
captured geometry/state, with the captured native inputs and history substituted.
`replay_ssao_chain.exe --reset-history` additionally reproduces reset-mask clear;
it saves all five outputs. CPU checks reject stale eye rays, retained reset
history, missing/truncated inputs, overwrite attempts and bad packet extents.

The initial prototype completed 1,126 evening and 1,249 rainy frames, with five
resets per run and no rejected inputs. Eight snapshots independently replayed
all 24 AO/depth/skylight outputs byte-identically (`live-ambient-replay/results.json`,
SHA256 `91c276ba504b98cdd2260595a0cbb7b8a88a6bbb542588b436ebc61d7f0229f0`).
Review then found that accumulation still retained captured eye rays. Those are
now updated from the current native camera; the later validation below applies
to the corrected implementation. The failed intermediate rainy capture and all
earlier evidence were preserved.

These native runs deliberately use the earlier version-1 weather content
baseline to isolate renderer behavior, not the version-5 release candidate.
Original reference captures are reused unchanged. No weather release or PR was
made. Production ownership/hooks, resize and scene teardown, original probe and
encoded-light composition, moving-rain parity, local play and the final native
100+ camera sweep remain outstanding.

Corrected implementation validation (`live-ambient-v3-*` evidence):

- Native evening: **1,332 executed frames**, five resets, zero rejections.
  Native rain: **1,271 frames**, five resets, zero rejections. Both processes
  completed successfully and their isolated runtimes stopped.
- **20 snapshots**, ten reset and ten settled, independently replayed all
  **100 output targets byte-identically**. This includes accumulation, history
  mask, temporal AO/depth and skylight, with the rainy base-7 quad preserved.
  `/tmp/v011-weather-reference/live-ambient-v3-replay/results.json`, SHA256
  `aa90257587069ec4be9b0259808ac4b37fc7a7f1aafde4413089cf6904d1ae6b`.
- Original post, gamma and skylight command-list regressions remain exact,
  including their context-state restoration checks:
  `live-ambient-v3-replay/regressions.json`, SHA256
  `2ecd497f95e984b40b273966619d903b1dadab8b16189006a04344979d471bdc`.
  The original SSAO-to-skylight chain regression also remained byte-identical.
- Both weather fixtures pass reset/steady CPU preparation and 21 rejection
  cases each. Observer, chain and command-list programs build with
  `-Wall -Wextra -Werror`. No Rust or game-content sources changed here.
- `live-ambient-v3-completion.json` verifies build pins, all captured files and
  ten actual camera requests, SHA256
  `5134e5c3e9bf2ee58b81ac446c9acad842f3e355c74f1f1de4134829f8adf660`.
  Private output and native probe geometry have the same captured Present ID;
  control status is polled one to four frames later, retaining the same camera
  ID, position, direction and FOV. This distinction is recorded in the receipt.
- Buffer comparisons are in
  `/tmp/v011-weather-reference/live-ambient-v3-visuals/`. They compare AO and
  encoded skylight, not the final game image. Weapon coverage differs from the
  original login-scene capture; native floor/detail inputs also still differ.
  Exact replay establishes the shader/input pipeline, not full scene fidelity.

Next: integrate original probes and the encoded-light accumulator/composition
with these verified producers, preserving the original light ordering. Do not
publish this diagnostic as a final weather-lighting fix. Continuous camera
motion, intentional frame-gap recovery, resize and teardown remain untested;
these captures verify initialization and discrete camera-ID resets only.

Resource-porter checkpoint: `f9fea35` (`feat/v011-vostok-weather`). This game
worktree checkpoint changes documentation only; content version 5 and the
unreleased production integration status remain unchanged.

## Original probe draws and connected replay (2026-11-12)

The observer now captures all original probe draws when `--stages --probes` is
requested: full cubemap faces/mips, original sphere/box geometry, constants,
samplers, blend state, both lighting accumulators and pre/post depth-stencil.
The original probe family (kind 13) is normalized to the existing probe capture
path. Draw counts and first/last Present IDs detect truncation or mixed frames.

`replay_probe_011e.py` prepares strict `PRB011E1` packets with unchanged original
pixel and vertex bytecode. `post_replay.h` owns cube resources, two lighting
outputs and captured blend/stencil state. Probe command lists execute once:
repeating an alpha-blended draw would change its input. The packet checker tests
three supported paths (sphere front/back and box with an unbound index input),
including 42 rejection cases for malformed geometry, resources and state.

Additional source captures are frozen under
`/tmp/v011-weather-reference/probe-state/{evening,rain}/capture-radar/`.
They add probe diagnostics; earlier reference images remain unchanged. At
factory-interior and yard-north, evening has 18/8 draws and rain has 16/8.
All 50 draws independently reproduce **100 lighting MRTs byte-identically** and
all 50 stencil byte surfaces exactly. Source and replay depth remain unchanged
during each draw. Upload/readback of D24 rounds some values upward by one unit;
this was measured before execution. The full packed depth/stencil surface is
therefore not byte-identical, and the comparison reports that explicitly.

`replay_ssao_chain.exe ... OUTPUT --probes PACKET...` additionally connects
GPU-produced temporal AO to every probe and GPU-produced skylight/probe colour
outputs to each following draw. Four weather/view chains reproduce all six
colour/AO/history-depth outputs and the final stencil bytes exactly. Lineage
checks cover 100 colour links and 50 AO links; the initial specular targets are
zero. Twenty-one intervening stencil changes remain **captured volume-mask
inputs**, as does the probe-index map and previous temporal history. This is
not a complete replay of their producers or native probe integration.

Evidence under `/tmp/v011-weather-reference/`:

- `probe-replays-v3/results.json`: 50 successful independent draws.
- `probe-replays-v3/regressions.json`: original post, gamma, skylight and all
  three SSAO passes still match, including temporal depth and context restoration.
- `probe-chain/results.json`, `probe-chain/lineage.json`: four connected chains.
- `probe-cpu-checks.json`: three valid preparations and 42 rejected mutations.
- `probe-completion.json`: frozen capture/source-build hashes, actual camera
  requests and replay build pins. The capture-build source snapshot is retained
  because shared replay code changed after the reference DLL was built.

Observer, command replay and chain compile with `-Wall -Wextra -Werror`.
Earlier failed replay attempts are retained: first the D24 upload difference,
then an overly restrictive expectation that both probe passes enabled depth.
The second pass actually disables depth and tests/inverts stencil. Many draws
have no covered pixels; 14 draws change diffuse, 12 change specular and 25 change
stencil across these four views, so exact matches include nontrivial work.

Original PDB/code identifies `stage_ambient_lighting::make_probe_indices_map`
at VA `0x00625860` (1,080 bytes), `execute` at `0x00625ca0` (3,677 bytes), and
`sort_by_size_predicate::operator()` at `0x006257e0`. Verified PDB names were
added to the 0.11e re-kit without inventing ABI contracts. The original archive
contains `environment_probe_index.ps`; the reconstructed 0.100b ambient source
and native probe shader do not provide this input. Original probe slot 5 is an
R8_UNORM index map when bound, but intentionally unbound in 38/50 captured draws.
Do not substitute native AO or a zero texture for it. Captured cube residency is
6/7 mips while the original integer `num_mips` constant is 8/9; do not derive
that authored constant from the currently resident resource.

Next: capture/replay the original probe-index and volume-mask producers, then
map native material inputs and connect original probes to live ambient output.
Native `t_parameters` is not established as original `t_diffuse`. Production
ownership, full light composition, moving rain, local play and the final native
128-camera sweep per weather remain outstanding. No content/revision change,
weather PR, merge or release is claimed by this diagnostic checkpoint.

Resource-porter checkpoint: `630b5ff`. Completion receipt SHA256:
`48811c5400810be3e9625c4adda2b259fd1691b6e91b0b86fbb1dd09bf2b010b`.
All 8,194 frozen diagnostic files and four camera requests were verified;
requested/actual coordinates agree at float32 precision. Status polling is
one to four Presents later than the recorded probe draws.
