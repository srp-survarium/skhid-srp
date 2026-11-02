# Held artefact transfer (patch 78)

## Rules (the owner, 2026-10-29)

- **The key:** hold H and press an artefact's quick-use key: the weapon goes
  down and an open right palm comes up with that artefact over it; everyone
  sees the same pose. H is the game's own unused action `artefact` (23h), a
  row of Options, Controls ("Hold artifact"), rebound there like any other
  key. Alt only shows names again. A quick-use key without H uses the item as
  before; letting go of H before the quick key never uses it. H held, then
  the quick key, is the chord as the other order: the game gives the action
  only on H's down and up ticks, so the carry keeps H's state between them
  (`CarryKey`); the quick key never reaches the native quick use.
- **Taking:** Use takes a teammate's offered artefact into a free container
  (within 2.2 m, looked at, nothing in between), with no pickup animation.
  The hint "Take artifact (E)" shows only when Use would take; without a
  free container nothing is shown or taken and Use does what the game does
  there (a battery, a trap, a pickup).
- **Switching:** another artefact's key lowers the one in hand (500 ms), then
  raises the new one (600 ms); no offer during either. The same key again
  puts it away, or, while it goes down, brings it back up from where it is.
- **Putting away:** fire, reload, aim, a weapon key or the wheel, grenade,
  melee, or a quick-use key without H puts it away; each acts once the
  weapon is back if still held. An empty quick slot does nothing.
- **No sprint while carrying:** the sprint key does nothing while an artefact
  is in hand (it does not put it away); a carrier who was sprinting stops.
- **Hit while carrying:** the raised arm takes hits where it is drawn (the
  damage pose is the drawn one), as now.
- **Dropping:** only on death, including mid-raise or mid-lower; never by a
  key. It falls from the palm (pulled back from a wall in front) to the
  ground below, and stays there for the rest of the match: no return to its
  anomaly. Either team takes it with Use. With no ground below the palm it
  is not dropped: the game's own death handling keeps it.
- **No gun falls from a carrier** (the owner, 2026-10-29): a gun drops at
  death only from hands that held it. A carrier's hands held the artefact
  (its gun lowered, out of the scene), so only the artefact falls; the gun
  stays out of the scene on the corpse, its native draw skipped as while
  alive, until native on_hide or the next life takes it back. A battery
  carrier drops no gun either (as before).
- Stowed artefacts behave as in the game.

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

A container's artefact uses the same hand (the owner, 2026-11-02;
`artefacts.md`, "A pickup in the hand"): `Event::PickedUp` from the site
`artefact_container.taken` starts `Showing`, one phase with its own 2 s
timeline (weapon down, up, held, down, weapon back), snapshot tag 9. It
starts only from `Stowed`, takes no command and no cancel, is never an
offer and drops nothing at death; Tick's ownership check and `Gone` end it
as they end a carry.

The command rides in the existing input stream: bit 29 (unused by the game)
marks the chord, with exactly one quick-use bit (14..19) its slot. Bits 30
and 31 stay clear, since the relays (0x31847B, 0x27C509) and replays
(0x1EE2A1) strip bit 31 as the input's own flag; bit 29 with no or several
quick bits (two inputs OR-merged in one node, 0x2C5B60) is no command, and
its quick bits never reach the game. Input building composes with Blackout; apply_input observes edges
before the native early-out and filters only the player's copied action mask.
A consumed Use stays consumed until release. Teammate transfer checks same
team, life, nearest target within 2.2 m, gaze cone 0.9 and static line of sight.
Player bodies do not obstruct their own offered hand. Ground pickup uses the
same reach/cone/LOS rule without the team restriction. A teammate offer takes
priority over a ground item so one press cannot take both.

Each player appends a canonical version-2, 32-byte carry snapshot inside the
existing length-prefixed player state, after its last native field; the
server's `Motion::read` drops it before reading the tail from the end
(`snapshot_rules.md` 7.4). It includes both sides of a queued
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
static-collision floor and release time; every state-6 item writes one (all
0xFF, read back as none, if it had no release), so save and load agree. It extends `artefact_base::serialize`
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

The pose source, `scripts/artefact-carry/pose.json`, gives each arm and
finger bone's lowered and raised rotation over the pinned stock battery idle
rig (`make_pose_clip`). No clip is shipped in resources.db:
`scripts/artefact-carry/compile_pose.py` compiles it into the runtime
arm/finger curves (`artefact_carry/curves.rs`), blended by the raise. The pose layer
preserves locomotion and native camera bones. An eye-relative two-bone solve
targets the right wrist at `(0.10, -0.14, 0.34)` m, preserves limb lengths,
and limits reach to 96 percent extension. Authored curves provide relaxed
finger curl and lower/raise motion.

The held hand is never still (`Life` in `pose.rs`): the wrist target breathes
up to 7 mm up and down every 4.2 s and drifts a few millimetres on slower,
unrelated periods; the palm tips about 3 degrees a quarter breath behind;
the fingers flex up to 0.07 rad, index first, the pinky half a second
later. Walking adds nothing (the owner, 2026-10-30). The clock is the
simulation's, shifted by the place, so players do not breathe in step; the
render and damage passes read the same inputs and draw the same arm.

The same solver runs in the render and damage-bone passes. It respects each
bone's computed-layer mask, so uncomputed collision matrices cannot invalidate
the arm pose. The artefact follows the hand's orientation, including camera
pitch. Its resource bottom-centre is anchored 12 cm along the palm normal,
with a 1.2 cm, four-second bob, and turns up to 0.15 rad about that normal
every 6.7 s. The largest held dimension is capped at 14 cm
and scale at 0.65; damage shapes and ground geometry are not scaled.

The separate held-only Larkspur resource crops triangles below source
Y=0.425 m, compacts vertices and repairs bounds/counts while preserving
materials and packed vertex attributes. The Lifebone cook requests this
fourth resource; a sole owner retains it until the exact item's destructor.
Normal ground rendering keeps the original resource and stem.

The native weapon remains owned and serialized by the engine. A single
visibility owner temporarily removes its model/optics, remembers exactly
which models were removed (by weapon, so a respawn or a new kit never
forgets them), and restores those models before native on_hide, the weapon's
return, or a dead body's draw (its optics update their models, and the
renderer asserts on a model out of the scene). The feature does not register a fake weapon or
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
enemy ground pickup; Use left to the game without a free container; a death
with the weapon and optics hidden; commands through the relay's 31-bit read; switching models and snapshots across both phases;
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
