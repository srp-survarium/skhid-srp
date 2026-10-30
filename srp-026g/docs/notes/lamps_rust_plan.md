# Breakable lamps: Rust port plan

Port PR #764's complete feature to the existing `raw -> vostok -> patches`
framework. Keep all authored fixtures, effects, snapshot bytes and cone
alignment. The reference C implementation is commit `a4f05203`; retain a
built reference executable and its behavioral traces before removing it.

## Ownership and boundaries

- `lamps` is safe Rust (`forbid(unsafe_code)`). Its domain values include
  `LampId`, bounded fixture count, material identity, finite world positions,
  bounded boxes and clipped bullet segments. Parse metadata and snapshots
  once; only typed values reach transitions and intersection logic.
- Lamp condition is `Intact | Broken`. A hit transition emits an effect only
  on the first break. Restoring history changes state without effects.
  Renderer membership is `Detached | Hidden | Shown`, with one total
  transition over insertion, removal and presentation events.
- One project registry owns both the lamp data and its presentation bindings.
  Its state is `Empty | Bound { project, physics, scene, data, bindings }`.
  Native handles never escape the engine boundary. Bullet workers access
  only simulation data under the framework lock. Renderer calls occur only
  through insertion/removal/frame sites on the game thread.
- Binding is transactional: parse all content, resolve unique visual/light
  bindings and verify capacities before changing any native vtable. Native
  removal and destructor fallback restore every original vtable before the
  project dies. Teardown and next-match initialization are explicit events.
- Typed `vostok` views cover binary configs, project visuals/lights, bullet
  effects and bounded history readers/writers. Fields and ABI facts live in
  the raw tables and re-kit, never in the patch. Reuse existing bullet and
  parsing APIs where their contracts fit.

## Framework integration

Declare named project lifecycle/history sites in `sites_026g.tsv`; use the
existing bullet/frame hook chains when possible. Generate detours, context
reads and native continuations. Complete runtime-vtable generation: copy the
native table including RTTI from its typed raw layout, replace declared
slots with generated convention entries, and expose the table through the
engine boundary. Keep its installation/audit coverage in the framework.
Remove `lamps_026g.c`, `lamps_026g.asm`, their arena blocks and manual hook
install declarations once the generated Rust implementation passes.

## Validation and delivery

1. Preserve the C reference executable and baseline traces; measure arena
   sizes. Add pure Rust tests for state transitions, invalid metadata,
   boundary geometry, history rejection without partial changes, and reset.
2. Run the existing native Unicorn scenarios on the Rust build, including
   all 92 map variants, renderer membership, occlusion, glass feedback,
   rollback/resimulation, quiet bullets, malformed history and teardown.
   Compare C and Rust observable renderer/effect/history traces during the
   port; do not compare private state layouts as though they were an ABI.
3. Exercise the new vtable generator with malformed declarations and real
   native dispatch, plus generation staleness and layering checks. Run Rust
   crate tests/clippy and focused patch checks while iterating.
4. Before updating PR #764: regenerate and commit the arena lock, run every
   full self-test variant and Unicorn check, workspace clippy and the full
   content manifest build. Update the PR description around the final Rust
   implementation. No revision bump or release.

Completion requires all C/manual lamp hooks removed, the framework's layer
checks passing, one owner for each state and native binding, and evidence
for initialization, transitions, rollback and teardown. The latest cone
height still needs the owner's visual confirmation; this port must not
claim that the screenshot adjustment was tested in-game.

## Implementation record

The port uses `lamps.rs` and `lamps/model.rs`, with engine access confined to
`vostok`. `ProjectRegistry` owns simulation data and native bindings together;
`ProjectPlan` resolves all bindings before installation. `Condition`,
`Membership` and `Registration` name the simulation, presentation and project
states. History is parsed before committing its cursor or changing state.
The generated shared hooks and runtime vtable copies replace every manual
lamp hook and both old C/NASM source files.

The stateful equivalence gate (run once, then removed with the C code) passed
1,692 comparisons, including 400 seeded shots and all 92 map variants. Native tests also verify atomic rejection of
bad metadata, a successful retry, unconfigured scenery, history failures,
removal, destructor fallback and new-match reset. The Rust suite passed 26
tests. All 46 client Unicorn checks and the launcher check passed; ragdoll's
entry assertions now recognize shared hook chains. The full content build
verified all 4,811 manifest files, and workspace clippy introduced no warnings.
The full client self-test passed every build variant, including the Windows
checksum/certificate checks through Wine and the complete installation audit.
The final staged sources also reproduce the arena lock without changes.

The initial Rust conversion grows the arena from 459,124 to 467,316 bytes
(+8 KiB) relative to the C reference. After integrating main's artefact carry
and pickup scoring, the combined arena is 512,096 bytes. Its lock records
SHA-256 `5b5260dfa9afcdb60f5cee1f64748e66701e7cf04a980197e7d4748634254cec`.
The shared Rust statics use 202,464 bytes, within a 256 KiB test budget; the
whole arena keeps its existing 1 MiB limit. Lamps use patch ID 81 because
pickup scoring now occupies 80. The shared reader layout has one definition,
and the carry flower's private retained-resource owner explicitly satisfies
the lock's `Send` requirement without making raw engine handles transferable.
Release revision 37 is unchanged; the content and simulation compatibility
entries remain part of PR #764.
