# Dropped weapon pickup

## Spec (the owner's rules, 2026-10-27..31; my picks marked)

- A player picks up the gun a dead player dropped: Use (the crate's and the
  artefact's key and prompt style, "Pick up <gun> (E)") while looking at that
  gun's bounding box on its own ragdoll, within the artefact's reach (2.2 m).
  Each client decides from where the gun lies on its own screen; nothing
  checks positions.
- A drop can be taken once, until the gun disappears (the owner, 2026-10-31):
  the dead player's body and the gun on it leave the world when the place's
  player is taken out of it, base_player::remove(1) in the simulation (the
  respawn tick 0x3169A0, a departure, a snapshot's kit change), the same
  tick on every client. That ends the drop. Typically the respawn, 8 s after
  the death by the modes' options (less with `respawn_booster`); in a mode
  without respawns, the match's end or the player leaving.
- The picked gun has the bullets left in its magazine (chamber included) and
  one full magazine in reserve.
- "Your ammo is replaced" (the owner, 2026-10-31): the borrowed gun's
  ammunition goes into the slots of the gun it replaces (the picker's own
  stacks there are kept aside and come back with the own gun); more
  stacks than those slots: free ammunition slots, then the gun binds the
  ones placed. A pickup is never refused for room.
- The picker's own ammo crate refills the borrowed gun by the game's own
  refill rule (the owner, 2026-11-01), for free (nobody's storage is
  charged), never lowered; its own guns as before. The game's rule: the
  crate fills each ammunition slot to the kit's count for it, the
  magazine and chamber of the gun that uses it inside; the lobby made that
  count whole clips of the ammo bags the slot was given (`bags div
  required_bags x clip_size` of the ammunition's config). A borrowed stack
  in a slot gets the same bags in its own ammunition: the bags the kit's
  count of the own ammunition there takes (`ceil(count x required_bags /
  clip_size)`), in whole clips of the borrowed one. A PKP (7N13: 100 a
  clip, 2 bags a clip) for an AK whose slot held 90 rounds of 7N10 (30 a
  clip, 1 bag; 3 bags): one belt, 100, so 60 in reserve with 40 in the
  belt; 4 bags two belts; 1 bag none. A slot the kit names no ammunition
  in (a free slot it was put in) is not refilled, as the game's.
- On the picker's death the borrowed gun vanishes (not dropped again) and the
  next life has the original loadout.
- The replacement rule (Codex's, approved): the gun in hands is replaced,
  never the other slot's; any gun for any gun; the carry rules after the
  skill modifiers hold: `disable_second_weapon` (modifier 64) keeps guns to
  slot 7 (a gun held in slot 10 is not replaced), and without
  `use_two_double_handed_weapons` (modifier 25) a double-handed gun is
  refused while the other slot holds one. Handedness is the item's
  `double_handed` (weapon+5E9h). My picks: a player holds at most one
  borrowed gun; nothing in hands but a gun of its two: no pickup.
- Synced: only each player's record (its drop: open with its kit and slot,
  or taken; and the gun it borrows), 12 bytes at the start of its
  state. The command rides in the input (patch 78's bit 29 with a pattern of
  two or more quick bits naming the place); the server drops an input whose
  pickup names no other place, and the client never sends one.
- My picks for stage 2a (the owner: "just don't overcomplicate the
  feature", smallest version everywhere):
  - the records are kept by place, not by kit player object: a state is a
    place's (its current kit's player); base_player::remove(1) of the
    place's player clears its record (its drop and its borrow);
  - the magazine and chamber are those of the donor's gun in its slot when
    the donor's current player is of the drop's kit, else a full magazine;
    the reserve is one magazine; any other bound stack of the gun is empty;
  - the modifiers are read as the native code sums them: apply_modifier
    0x2D5AE0 of 0 with weight 1 on the current kit's container (both are
    additive), above 0 is the skill;
  - a refused pickup changes nothing and is logged; the client cannot
    install a gun its cook failed to make (logged loudly: that client
    then differs);
  - the crate's checks say "available" while a borrowed stack is below its
    refill;
  - a gun given back (its holder's death, respawn, destruction, a rollback)
    stays cooked until 3 s after its body left the world (a rollback can
    take it again); a loaded world's gun is freed at the next frame.
- My picks for stage 2b (the same rule):
  - the prompt shows only when the pickup would be taken now (the drop open,
    its gun made on this client, the picker alive with a gun of its own in
    hands, no borrow, the carry rules), as patch 78's take prompt; patch
    78's own targets (a teammate's offer, a ground artefact) come first; the
    nearest gun wins;
  - the target is the gun's model box as the renderer has it, padded by
    5 cm, along the eye's 2.2 m segment; the level's ray (patch 78's) must
    be clear up to it;
  - a client that did not see the death (a late joiner) shows no prompt
    for that drop (it has no body for it and no cooked gun); it still plays
    a pickup another client sends;
  - Use's press sent as a pickup stays the pickup's until Use is let go (the
    crate or a battery under the body is not used by the same press);
  - the gun of a drop taken is hidden on its donor's ragdoll, and so is a
    borrowed gun on its dead holder's (it vanishes);
  - a gun whose config has no name shows the text without one.
- Status (2026-10-31): the shared rules (`crates/weapon-pickup`), the
  server's check and record shift, the instance (a gun cooked at each
  death), the record, the pickup, its restore and the crate (stage 2a), the
  ragdoll's gun, the prompt and the input builder's encoding (stage 2b) are
  built and checked under Unicorn; not run in a game yet.

## How a pickup is installed (patch 85, stage 2a, built, not run in a game)

`rust/patches/src/weapon_pickup.rs` (the records, the decision, the log)
over `vostok/pickup.rs` (the native moves) and `vostok/gun_cook.rs` (the
cooked gun).

- **The record** (`weapon_pickup::Record`, 12 bytes) after the two kit
  indices of each player state: `pickup.save` 0x2DAF70 writes it with the
  native byte writer (0x1F3DE0), `pickup.load` 0x2DB4CF reads it (the
  reader's begin/cursor/size); a record that does not decode (bad bytes, a
  short state) is logged and read as empty. Kept by place of the match
  (its world core): the death sets the place's drop (open with kit and
  slot when it held one of its own two guns; none when it
  held a borrowed gun, which vanishes, or anything else), a pickup the
  picker's borrow and the donor's drop taken, base_player::remove(1) the
  whole record empty (the drop ends with the body, the borrow with the
  life), the player's destruction the borrow none. The server reads the native
  fields from `FIELDS_AT = 2 + Record::BYTES`.
- **The pickup**, at `carry.filter` (0x2D876F; artefact_carry's handler
  calls `weapon_pickup::on_filter` before patch 78 filters the actions):
  the command (`weapon_pickup::command` of +330h); the rule
  (`weapon_pickup::decide`: alive, no borrow, its own gun held and pending
  in slot 7 or 10, another place's open drop, the carry rule: the other
  slot's gun's +5E9h and the kit's modifiers 64 and 25 through
  apply_modifier 0x2D5AE0) first without the gun's handedness; the gun from
  the cache (a cook of that place, kit and slot nobody holds, whatever its
  death's time: a rollback replays a death at another time), or (a death
  this client did not see) cooked
  from the donor's profile, waited for if not in; the rule again with the
  cooked gun's +5E9h; then the install:
  - every check before the first write: the own gun a client weapon in its
    slot, held and pending; the cooked slot a client weapon; its binding
    (+5ACh, +5B0h at most 8, +5B1h below it) and each placed stack there;
  - the gun bound to the world: +4 and +258h the core (player+120h), +0Ch
    the own gun's registry id, +5A0h the own gun's bullet manager, +5F4h 0;
    core+18B4Ch begin[id] = the gun (bind_to_game_core 0x2E30F0 never
    called: it appends);
  - each stack moved, the selected one first, into a slot of the own gun's
    binding that the other slot's gun does not bind, the own stack there
    parked in the cooked slot it came from (two set_items, one reference
    more on each across), else into a free ammunition slot (remove_item
    0x2FC680, set_item 0x2FC620); its +248h the core; the gun's +5ACh
    indices rewritten to the slots, +5B0h the stacks placed, +5B1h 0 (the
    cooked binding saved for the restore);
  - the guns swapped by set_item (the borrowed one into the picker's slot,
    the own one into the cooked slot; both held by one reference more
    across), each owner set by the item's own set_owner (vtable+28h);
  - the switch replayed as tick_active_object 0x2D76D0..0x2D772F does it:
    the own gun's deactivate(1) (client 0x244860), forget_active_object
    0x3919E0 (EAX player+750h, EDX the own gun), held +128h = pending +12Ch
    = the gun, set_user 0x2445E0, initialize 0x2449E0 (binds +538h), the
    magazine (+5E5h, the chamber +5F1h) and the reserve (the retained
    stack's +260h = the capacity +5E4h), activate(1) 0x244610.
- **The restore** (`vostok::pickup::restore`): everything back as cooked
  (the gun's saved +4, +0Ch, +258h, +5A0h, its stacks back in the cooked
  inventory and the own parked stacks back in their slots, +5ACh, +5B0h,
  +5B1h as cooked, the own gun in its slot and its registry entry); the
  held object is left as it is. At base_player::remove(1) (`pickup.remove`
  0x2DA790; flag 0, a snapshot's same-kit removal, is left alone) remove
  then deactivates the borrowed gun itself and unloads the own loadout into
  the profile; at the player's destruction (`player.destroy`) the
  destructor sees the own gun.
- **A loaded record** (rollback, late join, resync): at `pickup.load`,
  before the native readers, the borrow is made what the record says: an
  installed borrow the record does not name is restored; a borrow the
  record names and nobody installed is installed without the switch (its
  gun found or cooked from the donor's profile and waited for; set_user and
  initialize only, held left: 0x2DB62B deactivates the old held object and
  set_users the new one when they differ, and the inventory reader then
  finds the slots as the snapshot has them).
- **The crate**: `pickup.can_use` 0x2F809C and `pickup.use_hint` 0x2F80E4
  (before check_resupply's `mov ecx,[eax+10Ch]`): a borrowed stack below its
  refill exits to "available" (0x2F80B3, 0x2F8116); else ECX is a 188h
  view of the inventory with the own gun in its slot and the own parked
  stacks where the borrowed ones are (a free slot's empty), the loadout the
  profile names (what check_weapon_pickup.py proves check_resupply and
  resupply_from_profile read). `pickup.resupply` 0x2F81B8 and
  `pickup.client_resupply` 0x2954EF (before resupply_from_profile's
  inventory): each borrowed stack to `weapon_pickup::crate_refill`: the
  slot's allowance (profile +98h + kit x 704h + 4 x slot +2, none for an
  item FFFFh or an allowance 0, as resupply_from_profile
  0x2FDD56..0x2FDD71) in bags of the own ammunition parked from it, those
  bags in whole clips of the borrowed ammunition, less the gun's magazine
  and chamber for the selected stack, never lowered, free; each config's
  `parameters/clip_size` and `parameters/required_bags` (integers; the
  stack's dictionary entry +25Ch, its config +114h, the root +108h; the
  lobby's process_default_relocate_item 0x289EB2..0x289EF3 reads the same);
  the real inventory's weight marked, EAX the view (the native refill then
  refills the own parked stacks, charging the own storage). Each stack
  refilled is logged: `place P crate: slot S ammo A before -> after; the
  kit's N of ammo O (clip C/B) = bags = total (clip C/B) less in-gun`, C
  rounds a clip and B bags a clip.
- **The cache**: a gun installed is lent (never freed by its body's end or
  its donor's destruction); kept, it is freed 3 s after its donor's
  remove(1) (the body left the world: `Origin::Death.gone`), so a rollback
  that takes it again finds it.
- **For stage 2b**: `borrowing(thread, player)`, `instance(thread, place,
  died)`; the death's body (player object and life) kept with the place's
  record (below).
- **Checked**: `check_weapon_pickup_026g.py` (Unicorn, the built exe, the
  game's own set_item, remove_item and set_owner; the switch's calls and
  the byte writer recorded): the records written at a death and read back;
  a rifle for a held pistol taken; a second double-handed gun refused, then
  taken with modifier 25; a gun held in slot 10 refused with modifier 64
  (the game's apply_modifier); the picker's own place, a second command, a
  command after the donor's remove(1) refused with nothing changed, one 60 s
  after the death with the body there taken; the pickup's slots (the
  borrowed stack in the own gun's slot 8, the own stack parked), owners,
  references, bindings, switch calls, magazine and reserve; the crate's
  view, its "available" exits and its refill with real configs (90 rounds
  of 7N10 in the slot, 3 bags: a PKP's 7N13 100 - 18 = 82; 4 bags 182; 1
  bag none; the same ammunition 72; no storage charged, never lowered, the
  line logged) at both finish_use sites; a loaded record without
  and with the borrow; remove(0) and remove(1) (the own ammunition back);
  a record that does not decode; a lent gun kept past its body's end and
  freed 3 s after once given back; no branch into the new spans.
  `check_artefact_carry_026g.py` (the composed filter), cargo test and
  clippy of `client-patches/rust`, `weapon-pickup`, `survarium`,
  `match-server`, `session`.
- **Risks, not covered**: the native weapon calls (deactivate, set_user,
  initialize, activate, forget) are recorded, not run; a cooked gun's
  inventory item header names the donor's place and kit in snapshots (the
  inventory reader was not traced for it); a client whose cook failed, or
  whose wait was refused (a late joiner whose players were made this
  frame), refuses a pickup the others accept and then differs.

## How the local client offers and sends a pickup (patch 85, stage 2b, built, not run in a game)

`rust/patches/src/weapon_pickup.rs` (`ragdoll_gun`, `aim`, `on_input_end`)
over `vostok/pickup.rs` (`drawn_gun`, `gun_name`) and `vostok/ui.rs`
(`GameWorldUi::translate`, `places`). Nothing a match computes.

- **The ragdoll's gun** (`carry.weapon` 0x2477C8, mt_draw before the held
  item's draw; artefact_carry's on_weapon calls `ragdoll_gun` for a dead
  player): a borrowed gun on its dead holder, and the gun of a taken drop on
  the body whose death this client saw last at that place (player object and
  spawn time +328h, kept with the place's record, not synced), are hidden
  as patch 78 hides a carrier's (out of the scene, the native draw skipped;
  native on_hide gets them back). For an open drop the gun's pose is kept by
  place (a Lock: mt_draw may run in tasks): the site's transform (EAX, row
  vectors) and the box of its model (weapon+1810h -> +108h the render
  reference -> +0 a skeleton_render_model_instance -> +62B4h its resource ->
  +108h, 6 f32, turned by Y(pi): [-maxx, miny, -maxz] .. [-minx, maxy,
  -minz]; refused when not finite, inverted or a side over 4 m).
- **The prompt** (artefact_carry's on_hud at `player_icons.update_start`,
  after patch 78's own targets): for each kept pose, the eye's (player+44Ch:
  position m[12..14], forward m[8..10]) 2.2 m segment into the box padded by
  5 cm (the box's frame from the transform's rows, taken as orthogonal);
  the nearest whose drop a pickup would take now (the body still the
  drop's, the drop open, its cook made here, `decide` with the carry rules
  yes) and with patch 78's ray (group
  0x24, mask 2) clear up to it, and a place the server allows
  (`weapon_pickup::allowed` with match_options +577B5h). The hud is told
  `root.show_artefact_offer("st_weapon_pick_up", name)`: the name key from
  the cooked gun's dictionary entry (+114h its config holder, +108h the root,
  ui_desc/text_descriptions/name, each table's key asked first:
  engine_cfg_child reads past the end for a missing one), translated by the
  game's text translator (0x1FF0F0); `Pick up %1$s ({USE})`, `{USE}` the
  bound key (the localizer's), `%1$s` the name (the movie's
  `LocalizationData.format`). The target is remembered for Use.
  Each kept pose's verdict is logged once per drop body and change of
  reason (never per frame): `pickup: place P sees the gun of place T:
  offered`, or `out of reach (2.2 m)` (the box's centre farther than the
  reach and its padded half diagonal), `in reach, not aimed at its box`,
  `not offered, no line of sight`, `already taken`, `no drop in its
  record`, `the body drawn is not its drop's`, `its gun not made on this
  client`, or the carry rule's refusal with `held <slot> <its gun's
  handedness>, other <the other slot's gun's>, gun <the drop's>, rule <the
  kit's>`.
- **The pickup's log**: after `took`, `place P pickup: the donor's magazine
  M, chamber C` (or a full magazine, the donor's gun not there) and a line
  for each borrowed stack placed: `slot S ammo A x N (cooked slot F); own
  ammo O x K parked` (or `no own stack there`).
- **The command** (artefact_carry's on_input_end at `input_builder.end`,
  after patch 78's chord step): on Use's down edge while a gun is the
  prompt's and the input has no carry command, the mask becomes
  `weapon_pickup::encode(actions, target)` when `weapon_pickup::sane`
  passes it (the server's check: another place of the match); the press
  then stays the pickup's (Use cleared) until Use is let go. Logged:
  `pickup: place P sends a pickup of the gun of place T`.
- **The text**: `st_weapon_pick_up` in the ten `localization/*.tsv`;
  `scripts/content-edit/weapon-pickup-hint-1.json` maps it into
  `hud_strings`; the hud movie's `show_artefact_offer(key, value = "")`
  shows `LocalizationData.format(key, value)`. An older hud movie (one
  argument) refuses the two-argument call: the prompt is then not shown,
  the pickup by Use still works.
- **Checked**: `check_weapon_pickup_026g.py` (Unicorn, the built exe): the
  donor's gun drawn on its ragdoll is offered to the picker looking at it
  with its translated name; Use's press becomes the pickup command, held it
  stays off the game; the command played takes it, its gun is then out of
  the scene and the prompt gone; no prompt looking away, beyond 2.2 m,
  behind a wall, a second double-handed gun, modifier 64 with the gun in
  slot 10 or a place the match lacks; offered 60 s after the death, gone
  with the body's remove(1); the 5 cm padding. Unit tests of the segment test and the press.
  `check_artefact_carry_026g.py`, `check_ragdoll_026g.py`,
  `check_blackout_026g.py` (the composed handlers),
  `check_localization_026g.py`, the carry Ruffle harness
  (`scripts/artefact-carry/check.sh`) on the rebuilt hud movie.

## The facts the install was built from (read in the exe)

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

## The instance: one gun cooked at each death (patch 85, stage 1, built, not run in a game)

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
  `weapon_pickup::after_move`) is 3 s (a rollback's reach) past its donor's
  base_player::remove(1) (`pickup.remove`: the body and its gun left the
  world, typically the respawn 8 s after the death); any gun of a donor player at its destruction
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
  `instance(place, died) -> Cooking | Ready(inventory) | Failed |
  Missing` (by the death's time, which the record carries; stage 2a);
  `need(donor, kit, slot) -> Result<inventory, Refused>`;
  `give_back(Source { place, kit, slot })` (a gun nobody holds). The wait
  is inside the pickup and the record's reader (stage 2a). A refusal is a
  refused pickup (logged), never a crash.
- **The log** (`srp_patches.log`, each line with the process id): `pickup:
  asked for the gun of place P kit K slot S (death at T ms): address space A
  MiB`; at the next frame `cook of ...: made, inventory 0x..., N ms after it
  was asked for (the death), address space A -> B MiB` or `FAILED`; a wait:
  `pickup: waited W ms for the gun of place P kit K slot S: made` or why not.
- **Checked:** `check_weapon_pickup_026g.py` (Unicorn) runs the death, the
  frame's step and the destruction of the built exe with the game's own
  profile code, match_options, boost binder and invoker: the query and its
  recipe and profile copy, a replayed death, no gun, the completion through
  the game's invoker, the body's end (remove(1)) and 3 s, an empty result, a donor destroyed
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

## 83b's freeze and 83c's trace (2026-10-31)

The test builds keep the name they had while this was patch 83 (main took 83 for the HUD options in
rev40; the pickup is now patch 85): 83b, 83c, 83d. The first game test of 83b (one client, Player vs Bots, rev40 data) froze. What the logs show:

- the death of place 10 at 53700 ms (about 05:41:03.75Z): `asked for the gun of place 10 kit 0
  slot 7`, the last line of srp_patches.log (06:41:04.76 local); no `cook of ... made`, no panic
  line (a panic logs before its `ud2`);
- the client's last message to the server 05:41:07.25Z (the match packet log); the game log's
  5 s line stops at 06:41:06.9; 60 s later the server ends the match (no world uploaded);
- 19.6 min later the game log's `single_size_buffer_allocator<704> - out of memory!` asserts: that
  pool is the network match client's packets, not items (`allocate` 0x279E10, `malloc_impl`
  0x279D80; built by `match_client_impl::match_client_impl` 0x385910 at this+0B0000h over the
  object's own 0xB0000-byte arena, ctor 0x279C40: 1024 packets). The network thread kept receiving
  and nothing took the packets: a symptom of the frozen game, not its cause.

So the game stopped about 3.5 s after the cook was asked, about when the spike's cooks completed
(1.9 to 2.4 s).

Static review of the cook's path (threads, locks, waits):

- The death hook (0x1F28E0 from base_player::tick) runs on the logic thread
  (`engine_world::logic` 0x1A390 -> logic_tick 0x1A030). The query is that thread's; its
  completion runs in that thread's `logic_dispatch_callbacks` 0x19FF0 (resources::dispatch_callbacks
  0xE5AB0 -> resources_manager::dispatch_callbacks 0x102370 -> the boost invoker 0x202300 ->
  `cooked_entry`), at the start of the next logic tick: the same thread, never inside a tick.
- `start` holds no lock across `query_resources` (the ticket is Waiting before the query; a
  completion inside the call finds it), and `died` is called holding none. The completion takes
  TABLE alone; the cache's readers take CACHE then TABLE (`log_finished`); nothing takes TABLE then
  CACHE. No closure of patch 85 calls the game while it holds a lock. No lock is re-taken on its
  own thread (that panics, and logs).
- The wait (`wait_for`, 2 s) runs only for a pickup command: none was sent.
- What can still stop the game silently: the logic thread waits for the renderer (logic frame
  > renderer frame + 1: a loop of dispatches and Sleep(1), 0x1A2B0..0x1A35F), so a render thread
  that stops stops the logic thread too. On the render side, patch 85 runs `ragdoll_gun` (carry.weapon,
  every frame of a dead body: RECORDS, HOLDERS, SEEN, and `drawn_gun`'s raw pointer chain), and
  patch 78 holds HIDDEN across the native visibility change (on_weapon, on_weapon_hide).
  No deadlock of ours is found; the trace says where it stops.

83c changes nothing but the log (srp_patches.log, each line written and the file closed at once,
so a frozen game loses nothing written). Every line of patch 85 is `pid P tid T pickup: ...`
(GetCurrentThreadId). Lines:

- always: the death hook (entry, `done`); the issue (the profile copy, recipe, `query_resources`
  called / returned); the completion (entered, result count, the inventory, TABLE taken and
  released, freeing, left); a cook given back (Gun::drop), the native inventory release before
  and after; `restoring` before a restore; each free of the cache (`freeing the cook of`,
  `freed`); the locks of the death, the issue, the completion and the stores (RECORDS, CACHE,
  HOLDERS, TABLE: `trace: lock X in f: taking` / `released`).
- while a budget lasts (6000 traced uses, armed at each ask and each completion: about 6 s of
  frames): after_move, the cache's per-frame locks, `cooked`, `dispatch_callbacks` in a wait,
  ragdoll_gun (drawn_gun before/after, SEEN), a dead body's HIDDEN in on_weapon, the prompt's
  steps (offer, handedness, ray_test, gun_name).
- never: the record's per-tick writers (pickup.save/load, every player, every tick).

A frozen game's backtrace (winedbg) shows arena addresses (the `.srpx` arena at 0xA42D000). The
block is the one of `arena_026g.lock.json` whose `va`/`size` hold it; the lock keeps only global
symbols, so for the function link the arena again (the same sources give the same bytes, which
`--check` proves) and take the nearest symbol below, locals too:

    cd srp-026g/client-patches && nix develop .. --command python -c "import sys, link_026g as l, \
    patch_026g as b; x = l.link(b.ARENA, b.ARENA_VA, b.ARENA_LIMIT); a = int(sys.argv[1], 16); \
    v, n, h, _ = max(s for s in ((v, n, h, g) for n, v, h, g in x.every) if s[0] <= a); \
    print(hex(v), n, h, '+%#x' % (a - v))" 0xA457123

(SRP_026G_EXE set; the names are Rust's v0 mangling: `rustfilt` or `c++filt` reads them.) The
exe's own addresses: `tools/re-kit/v026g/lookup.py`.

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

