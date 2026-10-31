# Dropped weapon pickup

## Spec (the owner's rules, 2026-10-27..30; my picks marked)

- A player picks up the gun a dead player dropped: Use (the crate's and the
  artefact's key and prompt style, "Pick up <gun> (E)") while looking at that
  gun's bounding box on its own ragdoll, within the artefact's reach (2.2 m).
  Each client decides from where the gun lies on its own screen; nothing
  checks positions.
- A drop can be taken for 5 s of simulation time after the death, once.
  Whether the donor has respawned does not matter.
- The picked gun has the bullets left in its magazine (chamber included) and
  one full magazine in reserve.
- The picker's own ammo crate refills the borrowed gun's reserve to one
  magazine, for free (nobody's storage is charged); its own guns as before.
- On the picker's death the borrowed gun vanishes (not dropped again) and the
  next life has the original loadout.
- My picks: the picked gun replaces the gun in hands, only of the same kind
  (two-handed for two-handed, pistol for pistol), so a pair is never illegal;
  a player holds at most one borrowed gun; nothing in hands but a gun of its
  two: no pickup.
- Synced: only each player's record (its drop: open with its kit, slot and
  death time, or taken; and the gun it borrows), 12 bytes at the start of its
  state. The command rides in the input (patch 78's bit 29 with a pattern of
  two or more quick bits naming the place); the server drops an input whose
  pickup names no other place, and the client never sends one.
- Status (2026-10-31): the shared rules (`crates/weapon-pickup`), the
  server's check and the instance (a gun cooked at each death) are built;
  the install, the record, the prompt and the crate are not.

## How a pickup would be installed (read in the exe, not built)

- **Snapshot record.** A 12-byte record (`weapon_pickup::Record`) after the
  two kit indices of each player's state: written at 0x2DAF70
  (`base_player::serialize`, thiscall `ret 10h`; EBX the player, ESI the
  writer), read at 0x2DB4CF (`deserialize`, `ret 1Ch`; EBX the player, ESI the
  reader) before the held/pending IDs (0x2DB626, 0x2DB796) and the inventory
  (0x2DB7D8). The history capture (0x3218E0), the wire writer (0x324BE0), the
  hash (0x3251D7), rollback (0x2D2AA0) and the 0xE8 load all go through them.
  The server's `players.rs`/`motion.rs` parse `2 + 4 x items`: they shift by
  the record (`PlayerState::head_len`, `read`, `state_bytes`, `motion.rs`).
- **The switch.** No native call switches at once: `tick_active_object`
  0x2D7650 does it over ticks once the hide animation ends. Replay its steps
  through the objects' vtables at `carry.filter` (0x2D876F): old +1Ch(1)
  deactivate, `forget_active_object` 0x3919E0, held = pending = new, new
  +10h(player) set_user, +14h initialize, +18h(1) activate.
- **What a cooked gun lacks**: active+4 and +258h (the core), the
  registry ID +0Ch (give it the replaced gun's; core+18B4Ch is a fixed
  buffer_vector, `bind_to_game_core` 0x2E30F0 would append), and +5A0h, the
  bullet manager (set only by 0x2D7570 for roster weapons; `instant_fire`
  passes it to `emit_bullet` 0x322830: zero crashes the first shot). Clear
  +5F4h (load on initialize) to keep the magazine.
- **Ammo** binds by the config's (weapon, ammo) pairs to any of slots 8, 9,
  11, 12, 19..22 (0x2C7855..0x2C79E5), absolute indices at +5ACh into the
  owner inventory; use free slots or the encoded external offset
  (`check_weapon_pickup.py`).
- **Inventory set** 0x2FC620 adds a reference to the new item and releases
  the old: add one to the outgoing first.
- **Death**: `on_player_death` (0x2D77C0, client 0x1F28E0, thiscall, now at
  [esp+4]; called at 0x2DAA87 at that tick's player+324h) changes no slot;
  `base_player::remove` 0x2DA790(1) unloads into the profile: put the own gun
  back at its entry.
- **Crate**: `resupply_from_profile` 0x2FDC10 (stdcall inventory, profile,
  kit; called at 0x2F81C1 and by the client at 0x2954F8) and `check_resupply`
  0x2FDF50 (AL kit; 0x2F80AA, 0x2F80F2) read only slots +10Ch..+167h and
  write +184h: hand them a copy of the inventory with the own items in place
  of the borrowed ones, then set the borrowed stack (+260h) to the capacity
  (+5E4h). A slot the profile names must hold an item (no null check).
- **The prompt**: `artefact_carry::on_hud` (site `player_icons.update_start`)
  and `root.show_artefact_offer`; a key `st_weapon_pick_up` = `Pick up %1$s
  ({USE})` in the ten `localization/*.tsv` and an edit list for hud_strings;
  the gun's name from its dictionary entry (`ui_desc/text_descriptions/name`).
- **The gun on the ragdoll**: `carry.weapon` (0x2477C8; EAX its world
  transform) and the model box at weapon+1810h -> +108h -> +0 -> +62B4h ->
  +108h (6 f32, turned by Y(pi)); hide a taken one there as patch 78 hides a
  carrier's gun.

## The instance: one gun cooked at each death (patch 83, stage 1, built, not run in a game)

The owner's pick (2026-10-31): one cooked gun per death; no hold of the
simulation ("don't overcomplicate": the cook takes milliseconds, walking to
the body seconds), a plain wait in the rare case it is not in. The kit-load
spares of the spike are gone. `rust/patches/src/weapon_pickup.rs` (the
policy, the cache, the log) over `vostok/gun_cook.rs` (the cook).

- **The cook.** At the death (`carry.client_death` 0x1F28E0, on every client
  at the same tick; artefact_carry's handler calls `weapon_pickup::died`
  first; a client's players have only this death, 0x1F28E0 inlines the
  base's 0x2D77C0), when the held active object (+128h) is a client weapon
  (vtable 0xA04174) whose inventory item (+10h) is in slot 7 or 10: one
  query (`query_resources` 0xE5B60) of one request, the player cook's own
  {"inventory" 0x9FD55C, 41h}, its user data an inventory variant (vtable
  0x9FD808, type 0x205C60) and a recipe as the player cook fills it
  (0x20284C..0x2028A6): a deep copy of the player's profile (+6B8h) whose
  kit (+115h) keeps that weapon slot and the ammunition slots 8, 9, 11, 12,
  19..22 (the others FFFFh), the dictionary game+53A4h, the spatial tree
  match+130h (game_world+4D4Ch), the game world (+F474h), the seed
  options+577B0h, flag 0. The inventory cook makes the weapon and the
  ammunition its config binds; nothing of it is bound to the world, so
  nothing a match computes changes (no `simulation_changes` row).
- **Inside the tick, harmlessly.** `query_resources` (0.10b
  `resources_manager::query_resources_impl`) translates the inventory query
  at once (inventory_cook::translate_query, which reads only our profile copy
  and recipe and queries the weapon and ammunition with a parent) and may
  finish synchronously; it runs no other query's callback (that is
  `dispatch_callbacks`'). Our completion only records its result; no lock of
  ours is held across the query.
- **The completion.** A boost member callback made by the game
  (0x27D280, as late join's query_players makes its own): manager 0xA09374
  | 1, invoker 0x202300, which calls the member thiscall with ECX = the
  object (our ticket's number) and the queries_result pushed. The member is
  `gun_cook::cooked_entry`, a naked thiscall entry that aligns the stack
  (as RUST_THISCALL) and calls `cooked_shim`: one reference to the result's
  inventory kept (count 1 checked), the recipe freed. It runs where the game
  dispatches the querying thread's callbacks: between frames, or in a wait.
- **The cache, by donor life**: place + spawn time (+328h). A rollback that
  replays the death finds it and cooks nothing. Each entry owns a ticket of
  `gun_cook` (48): dropping it gives back the inventory and the profile copy;
  a cook still running is abandoned and its completion frees everything.
  Freed: a death's gun when the world core's time (+18FC0h, at the frame's
  `game_world.after_move`, ragdoll's handler calling
  `weapon_pickup::after_move`) is 5 s (`WINDOW_MS`) + 3 s (a rollback's
  reach) past the death; any gun of a donor player at its destruction
  (`player.destroy` 0x1F2100: it leaves; every player at the match's
  teardown); a gun the later stages give back (`give_back`: its holder died
  or left). A holder's inventory keeps its own native reference.
- **The wait** (`weapon_pickup::wait`, for the pickup's tick): when the cook
  is not in, the game's `dispatch_callbacks` (0xE5AB0) runs until it is,
  query_resources_and_wait's loop (0xE5C20); at most 2 s, then refused
  (logged). The callbacks it runs are those of every query the logic thread
  made: our other guns' (harmless), the UI's and sounds' (not the
  simulation's), and a late joiner's. **That one must not run in a tick**:
  its completion (late join's query_completed -> lj_complete -> install)
  binds the newcomer's slot, puts its objects in front of the saved states'
  game-objects bytes and renumbers the animation tables, "between simulation
  updates" (late_join_install_026g.c). It comes only after its players are
  made, each by a player cook's completion (0x202DD0, the next dispatch at
  the earliest). So `player_cook.ready` marks "a player was made" and the
  frame's `after_move` clears it (a frame's own dispatch lies between); a
  wait while it is marked does not dispatch and refuses (logged). A true
  deferral would need the late join's C completion to look at it: not done.
- **A loaded world** (a late join, a resync): the record names donor place,
  kit and slot; `weapon_pickup::need(donor player, kit, slot)` finds a gun of
  that source or cooks one (origin "a loaded world's") and waits the same
  way. The record's reader is inside `base_player::deserialize` of the 0xE8
  load (network_client::process_dynamic_match_info 0x27C0xx ->
  game_world_core::deserialize 0x2D0B70), a network handler outside any
  tick; the game itself dispatches from a network handler
  (process_static_match_info 0x27BEE7 loops on 0xE5AB0). No new
  reentrancy: the same callbacks as the game's own loop, with the late
  join's guarded as above. A loaded world's gun is freed by `give_back` or
  its donor's destruction.
- **The interface for the later stages** (`weapon_pickup.rs`):
  `instance(place, spawned) -> Cooking | Ready(inventory) | Failed |
  Missing`; `wait(place, spawned) -> Result<inventory, Refused>`;
  `need(donor, kit, slot) -> Result<inventory, Refused>`;
  `give_back(Source { place, kit, slot })`. A refusal is a refused pickup
  (logged), never a crash.
- **The log** (`srp_patches.log`, each line with the process id): `pickup:
  asked for the gun of place P kit K slot S (death at T ms): address space A
  MiB`; at the next frame `cook of ...: made, inventory 0x..., N ms after it
  was asked for (the death), address space A -> B MiB` or `FAILED`; a wait:
  `pickup: waited W ms for the gun of place P kit K slot S: made` or why not.
- **Checked:** `check_weapon_pickup_026g.py` (Unicorn) runs the death, the
  frame's step and the destruction of the built exe with the game's own
  profile code, match_options, boost binder and invoker: the query and its
  recipe and profile copy, a replayed death, no gun, the completion through
  the game's invoker, the window's end, an empty result, a donor destroyed
  while its gun cooks; every allocation freed once. `check_artefact_carry_026g.py`
  and `check_ragdoll_026g.py` (the composed handlers), `check_engine_026g.py`,
  cargo test and clippy of `client-patches/rust`. The wait is not run (no
  caller yet). No new site (player_cook.query removed).

What a cooked gun holds and shares (from the spike's reading of the cooks):

### What a cooked gun holds, and what it shares

From the cooks' constructors in the exe (each cook: +8 its class, +0Ch its
reuse policy; 1 is looked up by name in the resource registry,
`query_result::consider_with_name_registry` 0x108A00; 0 is made anew for each
query, `check_pickup_cooks.py` runs that gate):

- **Made per cooked gun:** the inventory (41h, inventory_cook 0x2C716A), each
  weapon (46h, weapon_core_cook 0x30A7FA), its state objects (12Ch..141h),
  each ammo stack (48h, items_cook 0x31F2DA), and the model *instances*: the
  pose and bones (skeleton_model_instance 10h, skeleton_render_model_instance
  15h, static 0Fh, 14h) and material_effects_instance (0Eh).
- **Shared by name:** the meshes (render_model_cook, ctor 0x1231C0 writes
  reuse 1), skeleton_combined_model (17h), materials (1Eh), skeletons and
  animations (2Ch..32h, 200h, 202h), configs (binary_config 1Fh, 205h;
  item_config 43h), sounds (27h, 28h), texture options (34h). A material is
  shared, so its textures load once.
- **Camouflage and decals:** the baker (camouflage_decal_baker_cook 6Ch) is
  asked per instance, but patch 70's bake cache answers a second query of the
  same baked texture with the texture already held (count +1, no bake, no
  load; `baked-textures.md`). So a cooked painted gun shares its baked
  textures.
- **Expected:** a cooked gun costs the per-instance objects (weapon, states, pose,
  ammo) and not the mesh or textures. The cook's log measures it.

## Codex's research (evidence; its code is removed)

The Unicorn checks named here are in `tools/re-kit/v026g/`.

## Proven native ammo behavior

`tools/re-kit/v026g/check_weapon_pickup.py <original survarium.exe>` runs
the original x86 routines under Unicorn, with synthetic weapons/inventories.
The routines below run unmodified; no ammo or resupply callback is stubbed.

There are two different bases. An inventory slot contains `inventory_item*`.
For a weapon this is **active_object + 0x10**. `weapon_core` functions generally
take the active-object base, but the item virtual death callback takes the item
subobject. Do not reuse offsets without adjusting that base.

Relative to the active-object base:

| Offset | Meaning | Evidence |
|---|---|---|
| +538 | retained ammunition stack pointer | load/unload below |
| +5AC | array of inventory slot indices for compatible ammo | initialize 2E2A99..2E2ABA; crate routines |
| +5B0 | count of compatible ammo slots | 2E41E5 |
| +5B1 | selected ammo ordinal | 2E2AA3; 2FDCFB |
| +5E4 | magazine capacity, u8 | load_magazine 2E1830 |
| +5E5 | loaded magazine rounds, u8 | load_magazine; serialize 2E31BD |
| +5E9 | double_handed, bool | config loader 30AB8D..30ABC0 |
| +5F0 | weapon uses a chamber | 2E2858; serialize 2E3209 |
| +5F1 | chambered round | unload_ammo; serialize 2E3226 |
| +5F4 | load on initialize | 2E2B71..2E2B81 |
| +5F5 | automatic reload flag | initialize 2E2A81; death below |

`load_magazine` (2E17D0, ESI = active object) moves
`min(capacity - loaded, stack)` rounds from the retained ammo item into the
magazine and marks its inventory weight dirty. `unload_ammo` (2E18E0,
EAX = active object) returns both the magazine and chamber to that stack.
The stack count is u16 at item+260; its inventory pointer is item+250.

The native harness also executes the original `double_handed` lookup branch
with absent/false/true config values, proving the field's base and offset.
The containing loader is named descriptively in the re-kit at 30AB40:
stack arguments retained config resource / weapon base, `ret 8`.

The initialize section 2E2A99..2E2B88 can bind a dedicated external ammo cell
using an encoded inventory-relative slot (the existing artefact scheme).
The harness runs both positive and wrapped-negative offsets and proves that
loaded rounds, chamber and reserve survive unchanged when +5F4 is clear,
including an empty magazine. This is a candidate way to isolate borrowed
ammo from the recipient's original calibre. It is **not yet installed**:
the allocation must have a native lifetime owner, reserve counts must enter
snapshots, weight must include it, and crates must refill it explicitly.
The normal crate cannot infer its supply allowance from an external slot.

Native item-slot ownership and weapon-user ownership are separate:
243590 sets item+250/+254 only. Client `weapon::set_user` 2445E0 calls
2E2990 to update the user and dependent pointers; it assumes a non-null
client player at its own +F4E3 read. The normal switch at 2D76D0..2D772F
deactivates the old held object, sets the new user, initializes, then activates
the new weapon. A transfer should follow that path with autoload disabled,
not independently overwrite just player+128 or item+250.
The original `weapon_core::deactivate` with argument one clears its user
at 2E2EE0 after clearing animation callbacks/state. The client wrapper 244860
also stops/releases its +80Ch effect object. Do not replace that native
deactivation with an isolated user-pointer store. The client initialize
wrapper 2449E0 calls core initialize and clears +181Eh.

The native creation path now has owned profile templates and verified
non-reusing inventory/weapon/ammo cooks. The normal inventory cook creates weapon
resources and binds compatible ammo (2C7250/2C75B0).
There is a synchronous resource-query wrapper at E5C20: it copies the query
parameters, wraps the callback, submits at 101F30, then dispatches resource
callbacks until completion. That observation does not establish that it is
safe to call from a simulation hook. Capturing the live context, choosing the
preparation point and a match-owned instance bank remain unimplemented.
Installation/restoration now has the owner above but is not wired into gameplay.
Do not substitute a byte copy of a live weapon:
it contains intrusive references, animation state and internal pointers.

## Weapon-slot eligibility

The data already defines weapon handedness: `ItemParams::double_handed`
(`crates/survarium/src/v026g/item_params.rs`, parsed from the item's config),
also consumed by the HUD/lobby `ItemFactory`. Skill modifier 25 is
`use_two_double_handed_weapons`; modifier 64 is `disable_second_weapon`
(`crates/survarium/src/v026g/skills.rs`). Read the effective modifiers rather
than assuming a particular learned skill node or the weapon's slot number.

Owner-approved replacement rule: replace the held weapon when the
resulting pair is legal. Without the dual-primary modifier, a rifle pickup
while holding a pistol beside another rifle requires switching to that rifle
first. With the modifier, either held rifle (or the held pistol) can be
replaced. `disable_second_weapon` must not be bypassed by pickup either.
The shared Rust rules implement this policy; the native pickup hook is still pending.

The death callback 2E3390 receives **ECX = item subobject**. Its store at
2E3407 to `[esi+5E5]` clears active-object+5F5, not the magazine. Loaded ammo
is still available to capture at death. The focused harness proves loaded
rounds and chamber survive the no-player branch; the player-present branch
has additional animation callbacks and still needs a lifecycle fixture.

## Crate path and the profile mismatch

Labels and ABIs are now in the re-kit. The descriptive names were assigned
from these bodies, not recovered from an original 0.26g PDB.

- `resupply_zone_core::can_use` (2F8080) compares zone+54 with the player's
  profile team at +45EC; team 2 permits both. It then calls check_resupply.
- `use_hint` (2F80C0) uses the same team check and maps availability to
  `st_use_resupply_zone`, `st_resupply_check_empty_inventory`, or
  `st_resupply_check_full_ammo`.
- `update_use` (2F8120) requires the selected zone and the player's Use bit
  (input mask+330, bit 21); after zone+50 milliseconds it calls vtable+48.
- `finish_use` (2F81A0) gets the player through its holder, then calls
  `inventory::resupply_from_profile` (2FDC10) with inventory, profile and kit.
- `inventory::check_resupply` (2FDF50) returns 0 available, 1 empty storage,
  2 full. Its args are AL kit, stack inventory/profile.

The native refill walks the **profile's slot descriptors**, not a weapon's
calibre. It counts the rounds already in each weapon's magazine and chamber
against the configured total carried allowance, adds reserve rounds up to
that allowance, and subtracts those rounds from the corresponding profile
item's storage. It does not fill the magazine directly.

The harness verifies ordinary and chambered weapons, exhausted storage,
repeated full-crate use, and a deliberately different profile item in the
ammo slot. That last case still fills the current ammo object while consuming
the unrelated profile item's storage. Therefore merely swapping the weapon
and its ammo stack is insufficient and would charge the wrong ammunition.
Borrowed weapon supplies need an explicit simulation-owned mapping/restock
path, with the native interaction's team and hold-duration behavior retained.

## Snapshot constraints

`inventory::serialize` 2FE360 and `deserialize` 2FE550 iterate the existing
slots without encoding arbitrary replacement weapon identities. The reader
2FE7B0..2FE7D4 invokes each **currently installed** item's deserializer. The
artifact slots have a separate identity mapping; weapon slots do not.

A pickup implementation must restore the weapon/ammo ownership mapping
**before** those readers run, and must keep an object alive independently of
the donor's respawn/profile setup. A raw pointer swap alone satisfies neither
condition. Any extension must also reach the server's snapshot parser and
replay format; an un-hashed local static cannot own this state.

The mapping must actually be restored earlier than the inventory call:
`base_player::deserialize` first reads held and pending active-object IDs
at 2DB626 and 2DB796, resolves them through core+18B4Ch, and writes player
+128h/+12Ch. Only afterward does 2DB7D8 call inventory::deserialize.
An ID byte of FFh means null. Restoring only at the inventory reader leaves
the already decoded held/pending pointers on the wrong weapon.

There are two native stages, so extending only the byte-stream reader is
insufficient. `game_world_core::serialize` (2D2570) captures live state into a
history record after `clear_buffers` at 2D257E. The record's wire writer is
`game_state_history_item::serialize` (324BE0), also called by the CRC routine
at 3251D7. The wire reader (324D80) clears its record at 324D90, then fills it.
Rollback applies a record through `game_world_core::deserialize` (2D2770),
without necessarily going through the wire reader again. Extra pickup state
must be owned by the history record across all these paths, included in its
wire bytes/hash, and cleared/released with the record; a current-world-only
side table cannot restore an older tick.

The record constructor is 324510 (ESI input, EAX result, ret 0), destructor
324910 (stack record, ret 4). Pool checkout calls construction at 2D3FEF;
pool return calls destruction at 2D4034. The in-place rebuild at 3252A7/3252AC
destroys and constructs the **same address**, then captures live state at
3252B3. Debug comparison also constructs/destroys a temporary record at
2D2446/2D254D. Therefore an address-keyed extension must release/reset on
native lifecycle events, not assume that an address identifies one snapshot.
`check_pickup_history.py` executes original construction, clear and destruction
through three same-address generations, verifies the self-relative section
buffers, player-buffer clearing, stack cleanup and allocation guards. Its
cases use inline buffers and empty animation references; heap-overflow and
nonempty reference teardown are not covered.

The fixture also demonstrates that `clear_buffers` preserves the cached CRC
flag at C0BB8 and value at C0BB4: calling native hash afterward returns the
cached value without serializing. Any extension mutation must invalidate
that cache explicitly. The constructor resets it. The hash writer's fixed
8000h-byte scratch buffer also needs a capacity audit before appending the
full extension (templates and displaced state can exceed the ammo payload).

The apply routine reconciles roster membership, then at 2D2A88 selects the
snapshot's kit in its roster slot and at 2D2AA0 calls that player's virtual
deserializer. Restoration must detach old borrowed mappings before native
roster/kit cleanup, then attach the decoded mapping before the player reader
resolves held/pending IDs. Native weapon decoding at 2E3513..2E3536 resolves
its selected ammo ordinal through weapon+5AC and inventory+10C, so the private
ammo cells must already be bound there too. These are inspected ordering
constraints, not installed hooks or proof of snapshot lifecycle completion.

The live installation sequence cannot be reused verbatim for this restore.
At 2DB62B the native reader saves the old held pointer, then resolves the new
one; when they differ, 2DB662..2DB67B deactivates the old object and calls the
new object's set-user. Prewriting held to the replacement would suppress
that transition. The snapshot attachment transaction above establishes
slot/registry/ammo bindings while preserving the old held pointer until the
native decoder performs its own transition.

`check_pickup_registry.py` executes the original weapon binder 2E30F0
(ECX = item subobject, stack core). It appends active-object pointers via
2D4540 to the core's fixed-capacity registry, writes core into active+4 and
active+258h, and writes the byte ID at active+Ch. The fixture then runs both
original ID-decoding slices, showing repeated original/replacement mapping
restoration with stable IDs and a negative control for restoring too late.
No transfer hook is installed by this fixture; the coordinated native
transaction above still needs match and snapshot integration.
Appending a new ID for every pickup is not a viable solution:
the native registry is capacity bounded and IDs are bytes.

