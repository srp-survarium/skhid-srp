# Held artefact transfer (patch 78)

Alt + an artefact quick key lowers the weapon, then raises an open right
palm with the owned artefact hovering above it. Both the carrier and other
players see the same pose. A plain quick key retains native activation;
Shift remains sprint. Releasing Alt before the quick key cannot activate
the item accidentally.

Use takes a teammate's offered passive artefact into a free container,
without a recipient pickup animation. The existing collection widget shows
`{USE}: Take artefact`, `No artefact container`, or `No free artefact
container`, based on the receiving player's inventory. The three strings
live in every language of the unified TSV catalog; the hint edit list adds
only their HUD mappings.

Choosing a different artefact lowers the old one for 500 ms before raising
the next for 600 ms. The weapon stays hidden between them. There is no offer
during either motion. Repeating the selected chord or using a weapon action
cancels presentation; an invalid/empty quick slot leaves it alone. Death
drops the currently carried item, which either team can take from the ground.
Stowed artefacts retain their native behavior.

## Ownership and synchronization

`crates/artefact-carry/src/carry.rs` owns the deterministic presentation state,
input edge detection, action suppression and canonical snapshots. Phases
carry their valid token/timing data. `Switching` retains the old optional
token and the queued token: losing the old item lowers an empty hand; losing
the queued item cancels the switch and lowers normally. Absolute wrapping
timestamps make late and split ticks agree. The generation counter invalidates
old offers without replacing native inventory ownership.

`vostok/inventory.rs` owns the native transfer transaction. Artefact containers
are a separately allocated array (+168h, byte count +16Ch), not the fixed
23-slot inventory. Stable container ordinals become native address-relative
slots only at the boundary, including wrapped negative offsets. Capacity,
life and exact passive ownership are checked before mutation. A retained
resource reference bridges native remove/take, preserving cooldown, passive
modifier bookkeeping and the same world-registry ID. Full containers cannot
reach the engine's eviction path.

Input bits 29..31 encode a bounded quick-slot command in the existing input
stream. Input building composes with Blackout; apply_input observes edges
before the native early-out and filters only the player's copied action mask.
A consumed Use stays consumed until release. Teammate transfer checks same
team, life, nearest target within 2.2 m, gaze cone 0.9 and static line of sight.
Player bodies do not obstruct their own offered hand. Ground pickup uses the
same reach/cone/LOS rule without the team restriction. A teammate offer takes
priority over a ground item so one press cannot take both.

Each player appends a canonical version-2, 32-byte carry snapshot inside the
existing length-prefixed player state. It includes both sides of a queued
switch and the button-edge state even while stowed. Native pointers and
renderer state are excluded. The `next` simulation-change row gates older
replays at release; all clients in a match must use a compatible build.

## Death drops

Both death entries run the drop before inventory teardown:

- Client override `survarium::player::on_player_death`, 0x1F28E0.
- Core/base handler, 0x2D77C0.

The client override inlines teardown and never calls the base handler.
Hooking only the base was insufficient in the live game. The drop covers
raising and lowering as well as settled offers. During switching it drops
the currently displayed item, leaving the queued inventory item stowed.

The anomaly/world retains the exact resource after native inventory removal.
Native state 6 marks a dropped item; native states are 0..5, and the anomaly
respawns only state 0. An immutable 20-byte release record contains position,
static-collision floor and release time. It extends `artefact_base::serialize`
at 0x313735 and deserialize at 0x3137EC, before the native active-start time.
Falling is an analytical gravity step clamped to the floor, reproducible
across different tick sizes and rollback. Ground pickup calls native take,
restores passive behavior and clears the release record.

`ground::Drop` is the pure synchronized state. `vostok/carry_ground` resolves
IDs through the live core-world registry for every interaction; its cache
is keyed by world, item and registry ID. It never reads a cached item pointer.
Ground presentation uses the original full-size model, including Larkspur's
stem. Dropped objects do not add a new dynamic physics body: targeting uses
the deterministic release/fall position and native static collision.

## Pose and rendering

The content manifest builds raise/hold/lower clips from the pinned stock
battery idle rig via `make_pose_clip`. `scripts/artefact-carry/compile_pose.py`
emits the runtime arm/finger curves from the same source. The pose layer
preserves locomotion and native camera bones. An eye-relative two-bone solve
targets the right wrist at `(0.10, -0.14, 0.34)` m, preserves limb lengths,
and limits reach to 96 percent extension. Authored curves provide relaxed
finger curl and lower/raise motion.

The same solver runs in the render and damage-bone passes. It respects each
bone's computed-layer mask, so uncomputed collision matrices cannot invalidate
the arm pose. The artefact follows the hand's orientation, including camera
pitch. Its resource bottom-centre is anchored 12 cm along the palm normal,
with a 1.2 cm, four-second bob. The largest held dimension is capped at 14 cm
and scale at 0.65; damage shapes and ground geometry are not scaled.

The separate held-only Larkspur resource crops triangles below source
Y=0.425 m, compacts vertices and repairs bounds/counts while preserving
materials and packed vertex attributes. The Lifebone cook requests this
fourth resource; a sole owner retains it until the exact item's destructor.
Normal ground rendering keeps the original resource and stem.

The native weapon remains owned and serialized by the engine. A single
visibility owner temporarily removes its model/optics, remembers exactly
which models were removed, and restores those models before native on_hide
or return-to-weapon handling. The feature does not register a fake weapon or
assign the player's active-item pointer.

Held model transforms are submitted in the same `on_bones` pass as the hand.
Artefact callbacks keep that presentation without replaying a stale matrix;
restoring an unchanged carry state preserves the attachment identity. This
avoids the callback-order lag and remove/re-add flicker observed while moving.
The HUD likewise has one collection-widget owner: while a carry hint takes
priority, native context text/progress/visibility are retained and restored
when the target disappears. Chat/profile selection hides the hint.

## Engine boundary and future pickup sharing

All live handles use current main's generated typed `raw` layer: fields and
subobjects in `fields_026g.tsv`, entry arguments in `sites_026g.tsv`, calls in
`engine_026g.tsv`. The `vostok` layer owns lifetime and inventory/render
meaning; the patch and pure carry/drop crate forbid unsafe code. Only IDs
and copied values survive hooks. The unused alternative active-object and
portable-animation prototypes were removed when preparing the PR.

The in-progress `feat/dropped-weapon-pickup` worktree was inspected read-only.
It also covers both death handlers, separates item ownership from rendered
targets, and has bounded snapshots plus an input/journal target relay. The
features should share these integration points when combined:

- One Use arbiter selects a typed weapon-drop or artefact target and consumes
  the press. Shared death/input sites must compose into one installed hook.
- A bounded snapshot codec and target relay can serve a shared target enum;
  native weapon templates/reserves and artefact state/cooldown stay in their
  own adapters.
- Weapon targeting intersects published drawn boxes, then sends only a stable
  life/drop ID. Artefacts currently target deterministic falling positions,
  so no separate target packet is needed. Exact mesh targeting could reuse
  the weapon branch's segment/box geometry and frame-publication owner without
  putting renderer matrices into simulation state.

No unfinished weapon transport or cross-worktree dependency is included.

## Validation

`check_artefact_carry_026g.py` executes the linked hooks and original native
inventory routines in Unicorn, with renderer/physics results as fixtures.
It covers both death entries; actual artefact save/load around the extension;
removal/take with retained cooldown/references; missing/full containers and
enemy ground pickup; switching models and snapshots across both phases;
held-Use consumption; all four callback layouts; weapon on_hide ownership;
Larkspur ground restoration; and matching render/damage output through pitch.
The pure carry/drop tests cover cancellation, missing items, clock wrap and
late/split ticks. `check_inventory_transfer.py` additionally exercises the
original Spring passive modifier list unlink/relink.

The HUD harness loads the shipped movie in headless Ruffle and checks the
actual collection widget, localization, native show/hide/progress restoration
and chat/profile suppression. The full content manifest is built and compared
against published layers; its editor tests cover mesh cropping and clip seams.
Before pushing, run the full executable self-test (all variants), every
Unicorn check, workspace clippy, and the changed code's tests.

Offline mesh previews execute the actual pose and attachment hooks with stock
skinned meshes. They cover all three available artefacts, FOV 50/75/100,
pitch -40/0/+40 and teammate views. They are geometry studies, not engine
materials/camera captures; at low FOV, tall model tips can approach the top
edge at the bob crest. The owner tested the feature with three local clients
on v0.21 Vostok before requesting the PR.
