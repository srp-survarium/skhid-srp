# Held artefact transfer

## Death, switching and collection UI corrections (2026-10-29)

The live client dispatches `survarium::player::on_player_death` at 0x1F28E0.
It inlines remove_alive/inventory teardown and never calls the base handler
at 0x2D77C0. Patch 78 now hooks both entries before ownership is cleared.
The native integration test executes each installed entry, saves/loads the
dropped resource and takes it into another player's container.

A different quick slot queues the next owned artefact while the old item
lowers for 500 ms, then raises the new one for 600 ms. The weapon stays hidden
between them. There is no offer during either motion. Re-selecting the queued
slot cancels the switch; losing the queued item lowers the old one normally;
losing the old item lowers an empty hand before raising the queued item.
A late tick can cross both boundaries. The version-2, 32-byte canonical carry
snapshot contains the current and queued tokens, so rollback reproduces the
same transition. All clients in a test must be updated together.

Held meshes use their resource's bottom-centre as the attachment origin,
with a 12 cm palm-normal clearance and 1.2 cm bob. The largest held dimension
is capped at 14 cm (scale at most 0.65). This clears Onyx's lower edge from the
fingertips without moving the damage bones. Offline previews execute both the
native pose and model-bound attachment code on the real stock meshes; they
no longer duplicate the attachment formula in Python.

The hint uses the existing `SrvContextInfo` collection widget, including its
font, placement and instant-action frame. Its owner retains the native text,
progress and visibility while an artefact hint is shown. Native hide/show or
progress updates cannot erase an active take hint, and leaving the target
restores native context. Missing/full messages still use the recipient's
containers. The Ruffle harness checks the actual exported widget, not a
replacement TextField.

### Alignment with dropped weapon pickup

Inspected the in-progress `feat/dropped-weapon-pickup` worktree (no PR was
listed for that branch at inspection). Its `weapon-pickup` crate owns typed
player-life/drop identities and canonical snapshots; `vostok/pickup_input`
keeps local rendered targets separate from deterministic input. Both death
entries are covered there too. Artefact ground ownership follows the same
separation: the pure `ground::Drop` contains release time/position/floor,
while `vostok/carry_ground` resolves a stable world-registry ID and performs
native inventory removal/take. Renderer handles and matrices are not part
of artefact simulation snapshots.

Integration seams for shared code, without copying unfinished weapon logic:

- A common Use arbiter should choose one typed target (weapon drop or
  artefact ID) and consume the whole press. Compose the shared death/input
  hooks once; two independently installed detours at these addresses conflict.
- The weapon branch's bounded snapshot codec and input/journal target relay
  can serve a shared target enum. Keep native weapon templates/reserves and
  artefact state/cooldown extensions in their owning adapters.
- Weapon targeting currently intersects published drawn boxes and sends a
  stable life/drop ID. Artefacts currently target their deterministic falling
  position, reach cone and static LOS from existing input; they do not need
  a second target packet today. If exact mesh targeting is adopted, reuse the
  weapon `Segment`/`WeaponBox` geometry and frame publication owner, while
  carrying only stable IDs through the simulation and replay stream.

## Movement, recipient prompts and death drops (2026-10-29)

The carry modifier is either Alt key plus the artefact quick key. Shift
continues to mean sprint; a plain quick key retains native activation.

Held models are now submitted from the same `on_bones` pass that poses the
right hand. The native artefact callback keeps that presentation without
reapplying a cached matrix. Restoring the same carry snapshot preserves its
presentation identity. This removes the previous callback-order/frame lag
and restore-driven removal/re-addition during movement.

The HUD chooses its message using the receiving player's inventory:
`{USE}: Take artefact`, `No artefact container`, or `No free artefact
container`. All three keys are registered as HUD-name/localization-key pairs
in `localization/flash_movies_localizations.options/hud_strings`, as well
as translated into all six language tables. Previously only the language
tables contained the key, so the HUD could not resolve it.

Death drops the currently carried passive artefact, including a raise or
lower in progress. Stowed inventory artefacts keep their native behaviour.
Hooks at the client override (0x1F28E0) and base handler (0x2D77C0) run
before native inventory death handling. A retained reference bridges native remove,
which unlinks passive effects and clears ownership. The world/anomaly
registry continues owning the exact resource; cooldown and ID are retained.

Native state 6 tags a dropped item (native states are 0..5). The anomaly's
spawn selection at 0x31B815 accepts only state 0, so it cannot reuse this
item. Its release point, static-collision floor and release time form an
immutable 20-byte extension, inserted into `artefact_base::serialize` at
0x313735 and read at 0x3137EC, before the native active-start time in both
streams. Falling position is an analytical vertical gravity step clamped
to that floor, independent of frame rate and reproducible after rollback.
The core world registry is +18940h/+18944h, item world +248h, completed
simulation clock +18FC0h (0x2D1E84). All are catalogued in fields_026g.tsv.

Ground presentation uses the original full-size model, including Larkspur's
stem. A dropped item is targeted within 2.2 m, the same gaze cone and static
line-of-sight rule as teammate transfer, without a team restriction. Use
calls native take only after confirming a free container; the consumed press
cannot fall through to a second pickup. Native take re-enables passive
behaviour. The dropped-state cache is keyed by live world, item and registry
ID; every interaction resolves the resource again from the native registry.

Validation: 19 pure carry/drop tests; 19 native Rust tests; strict clippy on
the changed crates; carry, Blackout and ragdoll Unicorn checks; 10 content
editor tests; the built HUD in headless Ruffle; full manifest build (2,580
files). Native checks include the real artefact save/load stream around the
extension, both native on_removed/on_taken, missing/full containers, enemy
ground pickup, Larkspur returning to the complete ground model, real HUD
argument keys, and callback-before-bones with moving/restored player state.
These checks do not replace the owner's in-game visual test.

## Animation resumption (2026-10-28)

The owner's current scope: the weapon goes away and a hand comes up with
the artefact, similar to S.T.A.L.K.E.R. 2; both the carrier and teammates
see it. Taking the artefact uses a button, without a taking animation.

`scripts/content-edit/artefact-carry-pose-1.json`, listed in
`release/content.tsv`, authors three new full-pose clips on the stock
70-channel player rig:

| Resource under `animations/single/artefact_carry/` | Motion |
| --- | --- |
| `raise` | Open right palm rises from waist to chest level in 18 frames (0.6 s at native 30 fps); left arm stays lowered. |
| `hold` | The same final pose, 36 frames, suitable for looping. |
| `lower` | Exact return to the starting pose in 15 frames (0.5 s). |

Each has `.b-spline` and `.bones_names` resources. `make_pose_clip` in
`content_edit.py` is their reproducible source builder: hash-pinned stock
battery idle plus authored local arm/wrist XYZ rotations, constant
translations/scales and other bones. Clamped cubic controls `(a,a,b,b)`
give smoothstep motion and zero velocity at each end. All three clip seams
match exactly. No stock clip is replaced. These are full poses for the
hand layer, **not additive offsets and not a replacement for locomotion**.
The stock grenade idle is type 1 (additive), while battery idle and these
clips are type 0; directly substituting the grenade clip would be wrong.

The authored right wrist moves from model-space (0.28, 0.83, -0.18) to
(0.20, 1.28, 0.24) metres. Upper/lower arm lengths remain the stock rig's.
Right fingers open during the raise; local hand +Y points upwards and -X
runs towards the fingers. The artefact centre is at local (-0.07, 0.11, 0)
with a 12 mm sinusoidal hover over four seconds. Its model stays upright
in player space. Weapon, parented to Root, follows the right-hand delta
while the weapon lowers/restores. Both views still need visual playtesting.

`scripts/artefact-carry/preview.py` evaluates the generated native cubic
tracks, checks all three seams and draws front/side rig projections. Run
it with `--clip` and `--names` pointing to the stock battery
`key_objects/player/nonadditive/stand/3rd_view/on_site/hands_only/idle`
resources, `--skeleton` to stock `animations/skeletons/scavengers_01.skeleton`,
and `--out preview.svg`. The green marker shows the proposed attachment,
not the actual mesh. This checks joint motion, not skinning, first-person
camera placement or rendered multiplayer behavior.

The resumed native preparation owner (`vostok/carry_animation.rs`) now
compiles, is bounded by the hook's `Thread` lifetime, recognizes the two
verified weapon vtables before reading weapon fields and owns independent
portable selectors. Its three Rust tests cover allocation failure, native
destruction order/reference balance and refusing non-weapon interfaces.
The constructor RNG seed and both destructor ABIs are now in the re-kit;
the four generated engine thunks pass the original-executable ABI gate.
The generator's image-field check now uses the exact executable's virtual
sections: the allocator at `0x1F1D9C4` lives in `.data`'s zero-filled tail,
beyond the old arbitrary 16 MiB bound.

`check_portable_carry.py` additionally executes the real resource binder
with eight managed hit-animation duration fixtures. Two independent cores
retain the immutable source twice each, and destroying them in either
order returns every reference/pin to its original count. This extends the
original graph-ownership and idle snapshot checks; it does not claim real
clip playback or bound-player callback teardown.

## Runtime integration (patch 78)

The feature is installed by the normal executable build. Hold either Alt key and press the artefact's quick-slot key to offer
it. The weapon lowers for 200 ms, leaves the scene, and the open right hand rises
with the actual artefact over 600 ms. Both views use the same native player
skeleton. A teammate looking at the carrier within 2.2 m, with an unobstructed
level ray and a free container, presses Use to take it. There is no recipient
pickup animation. The donor lowers the empty hand and restores the weapon.
The same chord again, or a weapon action, cancels the offer smoothly. Normal
quick-slot presses keep their activation behavior; a chord remains consumed
until the quick key is released, even if Alt was released first.

`artefact_carry.rs` owns the runtime integration. `carry.rs` is its total,
wrapping-time state machine. Unlike the earlier active-adapter proposal,
the existing weapon remains the native held/serialized object. Its actions
are filtered during carry; its render models have one visibility owner.
Before native `weapon::on_hide` runs, that owner returns the exact
removed model set to the live weapon callback, which immediately removes
it normally. This avoids a duplicate native remove assertion on death or
a held-item change; no cached weapon address is dereferenced.
No artefact pointer is put into an active-item slot. Shared render/collision arm and finger
rotations come from `compile_pose.py`, using the same authored clip source;
the layer preserves locomotion and native camera bones. The root-parented
Weapon bone follows the right-hand delta while lowering/restoring.

The same arm layer runs at 0x2DA769, before the native damage model
consumes the skeleton matrices. It reads the core skeleton at player +408h,
so collision poses do not depend on a preceding render frame. The visual
hover moves only the artefact, never damage bones. Matrix passes are sparse:
`bone+18h & 1` selects render bones, `& 2` selects damage bones. Unselected
matrices are not read or written. Requiring every matrix to be rigid used
to reject the whole collision layer when unused finger/Weapon matrices were
uncomputed. Native regression now poisons those entries and compares all
computed damage bones with the rendered pose.

The raised right wrist targets `(0.10, -0.14, 0.34)` metres relative to the
simulation eye. A two-bone solve keeps original limb lengths and limits reach
to 96 percent extension. The same eye and solver run in both bone passes;
no first-person-only arm displacement can detach bullet collision. Authored
curves still own lowering, raising and relaxed finger curls. The artefact
inherits the cupped palm's orientation, including camera pitch, and floats
above it. Native static-model bounds anchor its mesh at the bottom and cap its largest
held dimension at 14 cm, with a maximum scale of 0.65. This affects neither
ground geometry nor damage shapes.

Selecting a different passive artefact during weapon lowering replaces the
pending selection before the hand appears. During hand raising or offering,
it lowers the current item before raising the new one with a fresh generation. Repeating the selected artefact's chord cancels it; an
empty/invalid quick slot leaves the current offer alone.

artefact-carry-flower-1.json creates a held-only Larkspur model, cropping
triangles below Y=0.425 m from the hash-pinned stock mesh, compacting vertices
and recomputing bounds. Materials and packed vertex attributes are preserved.
The native cook queries a fourth resource for Lifebone only. carry_flower
retains it until that exact item's destructor, removes it on lower/take/death,
and releases its reference even if the scene is already gone. Native ground
rendering keeps the original model and stem. Query layout: result count +38h,
resource 0 +12Ch, stride 2E0h; resource 3 +9CCh. Hook sites are 25E91Ah,
260278h and 25EFE0h; no relative call instruction is replayed by a lohook.

A render attachment contains comparison identities and a copied matrix,
not a retained native pointer. Each of the four artefact callbacks resolves
its live model and scene. Client-only cache state 5 means the hand model is
present; native artefact state 3 remains passive. Cancellation, transfer and
death remove that model through the same callback. Full recipient containers
are checked before the native remove/take transaction, which retains the
same resource and preserves cooldown and passive-modifier bookkeeping.

Input bits 29..31 encode a bounded quick-slot command. The input-builder
hook runs after Blackout filtering; the shared packet is never modified.
Native apply_input tracks command/Use edges before its early-out and filters
only the player's copied action mask before native quick-slot handling.
A consumed Use press stays consumed until release. Native tick advances the
carry machine; save/load append/read its canonical version-2 32-byte snapshot inside
the existing length-prefixed player state. Thus the server's player-block
walker remains compatible. The `next` simulation-change row gates older
replays when released. Both clients must use this build; it is not wire
compatible with unpatched player-state readers.

The existing Blackout and ragdoll handlers compose the shared sites explicitly;
there is one detour per address. The independent portable-selector owner from
the earlier investigation remains tested groundwork, not the runtime renderer.

Validation: 16 pure carry tests, 19 native Rust tests, strict native clippy,
engine ABI checks, the carry/Blackout/ragdoll Unicorn checks, nine targeted content
editor tests, and the real HUD prompt in headless Ruffle pass. The carry
integration check executes the linked machine, hand layer, snapshot reader,
all four model callbacks and original inventory/world lookup functions. It
covers full containers, team/LOS refusal, repeated Use, cooldown/reference
preservation, restoration, death cleanup and the actual native `on_hide`
callback after the player has already changed its held-item pointer. The content manifest build
reproduces all 2,579 files including the six clips, localized prompt and held flower model.

The isolated test installation is `/tmp/artefact-carry-local`, with this
branch's server and final content. No live installation or release was
changed. The owner approved testing and both clients (CamoBob/CamoAlice) and this
branch's server are now running at 127.0.0.1:2334 for that test. The local
exe uses --revision=36 to match the local content candidate; source REVISION
is unchanged. Actual first/third-person camera framing, mesh fit, networking
and interactive gameplay remain subject to the owner's playtest feedback.
The full executable self-test passed every build variant and all 126
installed sites; the Windows certificate/checksum API checks passed for
all ten produced binaries under headless Wine. Workspace clippy is clean.
Automated checks are not evidence of an in-game visual pass.

## Earlier investigation

Historical investigation below predates patch 78; the runtime integration
above supersedes its pending active-adapter roadmap.
The requested interaction is another character pressing the normal Use key;
the recipient must have a free container. Full inventories must refuse the
transfer without replacing an artefact. The game must not run a local-only
inventory mutation: input, ownership and offer state must survive rollback,
joining and replay on every client.

## Inventory ownership and quick slots

Disassembled from the original 0.26g executable, and exercised with
`tools/re-kit/v026g/check_inventory_transfer.py` under Unicorn:

| Function | Contract |
| --- | --- |
| `inventory::set_item` 0x2FC620 | ESI inventory, EDI slot, stack reference to item pointer, `ret 4`. Retains incoming item, replaces slot reference, calls item virtual +0x28 with inventory and slot, marks inventory dirty. |
| `inventory::remove_item` 0x2FC680 | ESI inventory, EAX slot, `ret`. Requires an occupied slot. Calls item virtual +0x34, then +0x28 with null owner and slot 23; clears and releases slot reference. |
| `inventory::take_artefact` 0x2FC850 | EAX inventory, stack reference to incoming pointer, `ret 4`. Finds the first free artefact slot. If none is free, **removes the first artefact, shifts the remaining artefacts, and inserts the new one last**. A zero-container inventory is not a supported caller. |
| Quick-slot lookup 0x2FE7F0 | Stack inventory and zero-based quick ordinal, `ret 8`, EAX encoded inventory slot or sentinel 23. Walks the regular item order, then the artefact containers. Empty artefact containers still consume an ordinal. |
| `base_player::process_quick_slots` 0x2D8BB0 | EAX player, stack previous input, current input, simulation time; `ret 12`. Input action masks are at input +4. Bits 14..19 address the six quick slots. Calls `inventory::action` on rising and falling edges. |

Inventory slots are 23 resource pointers at +0x10C. The pointer at +0x168 is
the beginning of a **separately allocated artefact array**, not a subrange
of those 23 pointers;
the artefact slot count is the byte +0x16C. Item owner and slot are +0x250 and
+0x254; the intrusive reference count is +0xD0. The inventory owner interface
is +0x174; inventory dirty is +0x184.

At 0x2FE892 the quick resolver encodes an external container as
`((array - inventory - 0x10C) arithmetic_shift_right 2) + ordinal`, using
32-bit wrapping subtraction. `take_artefact` uses the same representation.
Negative offsets wrap to large u32 values; they are never Rust in-bounds
array indices. The carry snapshot stores a container ordinal instead.
Native transfer resolves that ordinal anew and confines encoded offsets
to the engine boundary. Unicorn covers separate allocations both above
and below the inventory, not merely an artificial inline subrange.

Unicorn verifies: empty-container quick-slot mapping; insertion into the
first empty slot; full-container eviction and shifting; removal followed by
pickup while retaining the item across the gap. A further case runs the
original artefact on_taken/on_removed methods and Spring passive modifier
link/unlink against two distinct player modifier lists. The donor loses
the movement node, the recipient gains that exact node, and cooldown and
resource references survive. Holder getters and item-owner setters are
fixtures; this still does not prove world integration or presentation.

Artefact virtual +0x30 is 0x313470: enable passive, set state 3. Virtual +0x34
is 0x313490: if state 4, disable active; disable passive; set state 0. The
native transfer sequence therefore must retain the resource, remove from
the donor, then take into the already-validated empty recipient container.
Calling take first would overwrite item ownership before the donor's
passive unlink. Cloning the artefact is also incorrect: it remains in its
original anomaly's resource list and is serialized by that anomaly.

The observed pickup routine checks capacity, not an artefact-kind/container
compatibility table. Any additional restriction needs evidence from the
container setup/configuration code, rather than a hard-coded kind list.

## Why assigning the held pointer is unsafe

`inventory::action` 0x2FCAA0 checks item category +0x24C. Artefacts are category
1, which calls their activation method. Category 2 selects a held object
through a separate active-item interface at item minus 0x10. Artefact
constructors do not establish that interface.

The player's current and desired active objects are +0x128 and +0x12C.
`base_player::tick_active_object` 0x2D7650 calls the current object's virtual
+0x2C without a null guard at 0x2D767F. It negotiates readiness/deactivation,
updates animation objects, activates the target, and checks continued
usability. `process_quick_slots` also calls active virtual +0x70 and reads
active +0x0E. Neither a raw artefact pointer nor a null pointer is a valid
replacement for a living player's active object.

`set_new_active_item` 0x2D8210 only writes the desired pointer; it does not
construct an interface, register an object, or validate its lifecycle.

The current and desired objects are serialized as bytes from their +0x0C
IDs (0x2DB15x and 0x2DB196). Deserialization at 0x2DB626..0x2DB654 resolves
those IDs through world +0x18B4C. A patch-local pointer or per-player array
therefore cannot stand in for a registered native active object. Its
registration, reconstruction and teardown must be explicit.

The registry's concrete layout and construction are now traced:

- World +0x18B4C/+0x18B50/+0x18B54 are begin/end/capacity-end pointers.
  The constructor 0x2CFFF0 initializes their storage at world +0x18B58
  with 0x3FC bytes: **255 active-object pointers**, leaving ID 255 for none.
- Append 0x2D4540 takes ESI vector header and a stack pointer to the pointer
  being appended, `ret 4`. It checks capacity through the engine assertion
  path but does not reliably refuse the write after an ignored assertion.
  A typed registration owner must reject a full table itself.
- Weapon registration 0x2E30F0 takes ECX inventory subobject and stack world,
  `ret 4`. It appends ECX minus 0x10, stores world at active +4 and item
  +0x248, and stores `(end-begin)/4 - 1` as the active object's byte ID +0x0C.
  The Unicorn test now executes this registration and verifies two IDs,
  pointers and world references. Do not call it on an empty-hands object:
  its inventory-specific +0x248 write is outside that smaller allocation.
- Gather-rule setup 0x313F50 registers batteries through the same append
  at 0x313F8B and writes their world/IDs. World teardown 0x2D0330 resets
  end to begin. The registry does not own an intrusive resource reference.

There is a legitimate native `empty_hands` implementation, vtable
0xA0BA6C, constructed by the resource path at 0x259245. Its constructor
0x299830 takes EDI destination, stack animation-pointer array and count,
`ret 8`; the object is 0x168 bytes before external animation-array storage.
It sets ID 255 and is not registered by the constructor. Its set-user
0x299900 records the player and copies the player's transform. It has real
selected-animation and bone-computation implementations (0x299930 and
0x299B00), but selected-animation indexes the supplied animation array;
constructing it with a null/empty array is not a valid shortcut. It does
not by itself provide an artefact model or a holding pose. Reusing its
implementation requires tracing and loading suitable animation resources
for a match, supplying registered identity and snapshot state, and adding
the artefact's model attachment. It is a better starting point to
investigate than modifying battery objective objects, but is not yet a
proven complete carry adapter.

Further tracing found its existing per-player owner: `player::initialize`
0x1F2540 uses player +0xF47C as the empty-hands fallback when there is no
desired active item (0x1F25AD). The player resource cook requests
`player/empty_hands` as resource class **0x7C** at 0x202980..0x2029A0, only
when its creation object's byte +0xB1 is set. The stock default player
config defines that path as `gameplay/empty_hands/empty_hands`. That config
contains only `user_animations[0] =
resources/animations/single/weapons/ak/player/stand/3rd_view/on_site/idle_lobby`.
Enabling it in a match requires checking both the request and response
branches; changing the shared stock config would also change the lobby.

Empty-hands vtable +0x58/+0x5C point to 0x2AFAC0/0x0F7760: both reach
**INT3 stubs**, rather than real serializer implementations. Registration
alone is therefore insufficient. Its animation selector 0x299930 selects
an idle clip using its local RNG; it does not select the portable core's
movement/stance animations. A carry adapter must supply real snapshot
support and locomotion/hand animation composition.

The portable core constructor 0x2FEBC0 creates animation-selection and
animation-object machinery. The battery has its own take/carry/put states,
hand expressions and serialized carried-item representation. Reusing a
battery pointer as an artefact risks objective-rule ownership and battery
serialization; that reuse has not been established as safe.

## Existing rendering and animation resources

The artefact does already own its own render-model instance. The client
wrapper is more than its active-effect hint. `artefact_cook` 0x25E800 asks
for `object/model` as resource class **0x0F**. The three stock artefact
configs provide models under `item/artefacts/`; Spring is
`item/artefacts/artefact_spring`.

The wrapper callback has the same relative tail fields for each artefact:
callback +0x0C is its client game world, +0x10 is cached presentation state,
+0x14 is the model holder (its +0x108 is the render resource reference),
+0x18 is the spawn particle instance. Callback offsets from the artefact
base are Spring +0x398, Lifebone +0x498, Onyx +0xF20, Rattle +0x340.
The corresponding update callbacks start at 0x25EBB0, 0x25F100, 0x25F640,
0x25FB80 (the Rattle body was traced in detail).

Rattle's callback 0x25FB80 compares native state +0x280 with cached state.
Leaving spawned states 1/2 removes the existing model (0x25FC7C or
0x25FCCD); entering those states adds it using the container transform
(0x25FDB0 and the following state-2 branch). While spawning, it updates
the scaled transform through `scene_renderer::update_model` 0x22280.
The ordinary inventory state 3 does not draw the model. Thus a hand
attachment can reuse the existing model, but must coordinate add/update/
remove with the native callback under one presentation owner. Reusing the
model does not require stealing an objective's battery renderer.

Stock `gameplay/victory_items/default.options` names existing player-hand
show/idle/hide clips under
`resources/animations/single/weapons/key_objects/player/nonadditive/stand/3rd_view/on_site/hands_only/`.
They are present in resources.db, along with `default.user_animations` for
portable-object movement. The idle-lobby clip alone would not provide
appropriate walking/crouching carry behavior. The native item clips use
the electrobox skeleton and cannot simply be applied to a static artefact
model without an attachment transform.

## Integration requirements still to resolve

1. Establish a dedicated carry adapter's active-object ABI, world
   registration, lifetime and snapshot reconstruction. Obtain the artefact
   model and first-/third-person hand attachment through existing resource
   and animation systems. The native inventory artefact wrapper alone is
   not an active item.
2. Encode Alt + quick slot in deterministic input without also activating
   the artefact or changing equipment presets. Chain `input_builder.end`
   with Blackout filtering; do not read keyboard state from simulation.
3. Persist an explicit offering state and the previous active selection.
   Cancellation, weapon selection, death, disconnect and rollback must
   remove the adapter and restore normal ownership/selection exactly once.
   Initially allow offering passive artefacts only; active effects need an
   explicit gameplay decision before crossing owners.
4. Use the engine's line-of-sight/range selection and normal Use interaction
   with a clear prompt. `detect_usable_object` 0x2D84C0 raycasts and returns
   a usable-object interface; `apply_new_input` 0x2D88C3..0x2D898F starts Use
   on a rising edge after calling its can-use virtual. No remote transfer
   should be possible through walls or by client-only targeting.
5. Serialize contested takes by simulation order: the first successful
   ownership transition consumes the offer; later attempts see no offer.
   Recheck donor identity, offer generation, item ownership and recipient
   free capacity at the single commit point. The transfer guard owns the
   temporary resource reference and must never expose a half-transfer.
6. Cover active/passive modifier transfer, full/no containers, cancellation,
   death, disconnect, competing takers, repeated Use, snapshot restore and
   replay. Add a `next` simulation-change row with the actual gameplay
   implementation. These research-only changes do not change a match.

HUD QuickSlots/QuickItemRenderer and the native quick-slot description are
deliberately outside the anticipated hook surface; the separate consumable
activity-ring work owns those areas.

## Implemented deterministic adapter core

`artefact-carry` has one state owner and one transition function:
Stowed -> Equipping -> Offering -> Unequipping -> Stowed. The artefact is
not available to another player while the weapon's hide is still running.
Only Offering exposes a token. Cancel/owner teardown removes that token;
successful transfer does so before a second request can run. A failed
transfer keeps the offer and both inventories intact. The native `Transfer`
implementation must perform the retained-reference transaction described
above, including range/ownership/capacity validation at its commit point.

The versioned nine-byte snapshot includes phase, generation, artefact ID,
container ordinal and previous active-object ID. Parsing is atomic: malformed
snapshots leave the old state unchanged. Native identity/owner are bound
at construction and cannot be supplied by snapshot bytes. **The core must
be serialized for every player even while Stowed**: it preserves the
generation counter between offers. Writing it only from the native
`m_carried_item` branch would omit that counter when stowed and would not
be a valid rollback implementation.

Ten unit tests pass, covering hide completion, cancellation during equip,
stale requests, competing takers, full/missing containers, ownership
changes, owner teardown, every snapshot phase, malformed input and replay
after rollback. `cargo clippy -p artefact-carry --all-targets -- -D warnings`
also passes. These are state/inventory-contract tests; they do not prove
native animation selection, in-game snapshots or hand rendering. The
crate is now used by the native inventory owner, but no carry hook is
installed and no existing match computation changes.

## Native inventory binding checkpoint

`vostok::inventory::Inventory` now owns the live hook-scoped inventory view.
Quick-slot resolution parses the external container address before reading
an artefact; offer state holds an ordinal, never an address. The transfer
method rejects stale ownership, active effects, dead owners, self-transfer
and absent/full containers before native mutation. A private RAII reference
keeps the same anomaly-owned resource alive across native removal and pickup.
Three 32-bit Rust tests exercise this boundary, alongside the original-code
Unicorn test. This remains unhooked: interaction reach/LOS, the registered
active adapter, animation loading and world snapshot integration are still
required before exposing Alt+number.

## Portable animation construction findings

The native battery factory 0x2F9F40 allocates 0x4D0 bytes; its independent
portable core occupies +0x2D0..+0x4CF and the battery points to it at +0x1AC.
The core constructor 0x2FEBC0 receives destination at stack+4 and an initial
selector value in EAX. Its meaning is still being traced; do not infer that
it is a resource pointer. The selector owns individually allocated state
nodes, so copying the 0x200-byte core or borrowing a weapon's mutable core
is invalid.

The subresource callback at 0x2FA462 passes that core in EAX and a reference
to the loaded user-animation resource in EDI to 0x2DF050. That helper retains
the resource at core+0x48 and binds the hit-animation selector at core+0x60.
`selected_animations` 0x3012A0 combines the battery state's hand expression
with 0x2FEC10's independently selected locomotion expression. The latter
takes EAX portable core, EDX hand-expression metadata, EDI result pair, and
stack+4 animation buffer, then returns with ret 4. It dispatches the active
selector node at core+0x20 through virtual +0x20. These findings establish
the composition point, not yet a safe construction/teardown binding.

A candidate implementation owns its own portable core and retains only
immutable animation resources; it never borrows the weapon's mutable
selector. Dedicated hand clips can be preloaded through the player cook's
resource query. The existing empty-hands query at 0x202974..0x2029A0 is
conditional on creation-data+0xB1; changing the shared lobby resource would
be inappropriate. Match-only resource loading and callback teardown must
be proven before enabling a registered active adapter.
