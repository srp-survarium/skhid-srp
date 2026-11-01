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
  charged), never lowered; its own guns as before. No minimum: the extra
  magazine is the pickup's alone. The game's rule (the lobby's
  process_default_relocate_item 0x288D60, read below): a kit has `bags` =
  `player.ammo_bags_count` of default.player (lobby+311Ch, 0x22112B) with
  the kit's modifier 16 (`ammo_bags`, apply_kit_parameter 0x2F7660,
  0x288E4B); a weapon put on gets one ammunition item it pairs with (the
  owned one of the lowest entry+384h, at least a clip owned), `(its bags
  div required_bags) x clip_size` rounds of it (capped by what is owned),
  into one slot: slot 7's weapon slot 8, slot 10's slot 9; both weapons
  with different ammunition split the bags, slot 7's `min(int(0.7 x bags
  + 0.5), its clips owned)` (0x28A0B2..0x28A0CE), slot 10's the rest; one
  weapon, or the same item for both, all of them in slot 8. The ammo
  window lets the player share the bags otherwise; the kit keeps each
  slot's rounds, and the crate (resupply_from_profile 0x2FDC10) fills each
  slot to them, the magazine and chamber of its weapon inside. So the
  bags of a weapon slot are those of its own gun's ammunition in the kit:
  `rounds x required_bags / clip_size` (rounded down) of each slot its
  own gun binds. The borrowed gun gets them in whole clips of its loaded
  ammunition, its magazine and chamber inside, in its loaded stack; its
  other stacks are not refilled. The owner's 83f41 test: the own slot-7
  gun's ammunition 70 (50 a clip, a bag) had 100 rounds in slot 8, 2 bags
  (int(0.7 x 3 + 0.5) = 2 of the default 3); the borrowed gun's
  ammunition 7 (30 a clip, a bag) in slots 8, 9 and 11, 13 rounds in it:
  2 bags are 60 rounds, slot 8 gets 47, slots 9 and 11 nothing.
- One rule per weapon slot (the owner, 2026-11-01): weapon slots 7 and 10
  each have a hidden twin, and a pickup acts on the slot of the gun in
  hands only. Holding its own gun: that gun and its ammunition go hidden,
  the picked gun takes the slot (a normal gun there: switching, the hud,
  weight and reload are the game's). Holding a picked gun: it is let go
  and the new one takes the slot; the own gun stays hidden. Both slots may
  hold picked guns. On the picker's death or respawn each slot gets its own
  gun and ammunition back and the picked guns go (not dropped again). A
  stack both own guns share (one calibre) stays with the gun that still
  binds it and is hidden with whichever own gun goes last; each slot's
  restore gives back what its own pickup hid. No donor is tracked: a drop
  is its body's, and a picked gun is the picker's.
- The carry rule, exactly (the owner, 2026-11-01): two pistols yes; two
  two-handed guns only with the terminator skill
  (`use_two_double_handed_weapons`); `disable_second_weapon` keeps guns to
  slot 7.
- The hud always says why a gun aimed at (in reach, in sight) cannot be
  taken (the owner, 2026-11-01), in the pickup prompt's place, Use doing
  nothing: a second two-handed gun "Can't carry a second rifle"
  (`st_weapon_cannot_carry`); `disable_second_weapon` "You can carry one
  gun only" (`st_weapon_one_gun_only`); no gun of its two in hands (a
  battery, an artefact, a grenade, a medkit, nothing) "Hold a gun to swap
  it" (`st_weapon_no_gun_in_hands`); a weapon switch under way "Wait for
  the weapon switch" (`st_weapon_switching`); taken already "Already
  taken" (`st_weapon_already_taken`; only while the gun is still drawn at
  its spot: once hidden, the empty spot shows nothing); anything else (a body that is not
  the drop's, no drop in its record) "Can't pick this up"
  (`st_weapon_cannot_pick_up`). A gun not made yet on this client is
  offered all the same (the pickup waits for it). Not aimed at, out of
  reach or out of sight: no prompt, as for the crate and the artefacts. A
  dead player has no prompt.
- The replacement rule (Codex's, approved): the gun in hands is replaced,
  never the other slot's; any gun for any gun; the carry rules after the
  skill modifiers hold: `disable_second_weapon` (modifier 64) keeps guns to
  slot 7 (a gun held in slot 10 is not replaced), and without
  `use_two_double_handed_weapons` (modifier 25) a double-handed gun is
  refused while the other slot holds one. Handedness is the item's
  `double_handed` (weapon+5E9h). Nothing in hands but a gun of its two
  slots: no pickup.
- Synced: only each player's record (its drop: open with its kit and slot,
  or taken; for each weapon slot the gun picked into it: the pickup's tick
  and the recipe, the profile kit slot it was cooked from, read only to
  cook it anew on a client without it), 12 bytes at the start of its
  state. The command rides in the input (patch 78's bit 29 with a pattern of
  two or more quick bits naming the place); the server drops an input whose
  pickup names no other place, and the client never sends one.
- My picks for stage 2a (the owner: "just don't overcomplicate the
  feature", smallest version everywhere):
  - the records are kept by place, not by kit player object: a state is a
    place's (its current kit's player); base_player::remove(1) of the
    place's player clears its record (its drop and its picks);
  - the magazine and chamber are those of the donor's gun in its slot when
    the donor's current player is of the drop's kit, else a full magazine;
    the reserve is one magazine; any other bound stack of the gun is empty;
  - the modifiers are read as the native code sums them: apply_modifier
    0x2D5AE0 of 0 with weight 1 on the current kit's container (both are
    additive), above 0 is the skill;
  - a refused pickup changes nothing and is logged; the client cannot
    install a gun its cook failed to make (logged loudly: that client
    then differs);
  - the crate's checks say "available" while the borrowed gun's loaded
    stack is below its refill;
  - a picked gun let go (its picker's death, respawn, destruction, another
    pickup into its slot) stays cooked 3 s (a rollback can take it back);
    a rollback undoing a pickup puts its gun back in its body's drop; an
    untaken drop's gun is freed 3 s after its body left the world.
- My picks for stage 2b (the same rule):
  - the prompt shows only when the pickup would be taken now (the drop open,
    its gun made on this client, the picker alive with a gun of its own in
    hands, the carry rules), as patch 78's take prompt; patch
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
    picked gun in its dead holder's hands (it vanishes);
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
  held a picked gun, which vanishes, or anything else), a pickup the
  picker's slot's pick and the donor's drop taken, base_player::remove(1)
  the whole record empty (the drop ends with the body, the picks with the
  life), the player's destruction the picks none. Its bytes: the version
  (2), the drop (tag, kit << 2, slot << 4), then for slot 7 and slot 10 a
  byte (0 none, else the recipe's place + 1, kit << 5, slot << 7) and the
  pickup's tick (u32). The server reads the native
  fields from `FIELDS_AT = 2 + Record::BYTES`.
- **The pickup**, at `carry.filter` (0x2D876F; artefact_carry's handler
  calls `weapon_pickup::on_filter` before patch 78 filters the actions):
  the command (`weapon_pickup::command` of +330h); the rule
  (`weapon_pickup::decide`: alive, a gun of its two slots held and pending
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
  then deactivates the picked gun itself and unloads the own loadout into
  the profile; at the player's destruction (`player.destroy`) the
  destructor sees the own gun.
- **A loaded record** (rollback, late join, resync): at `pickup.load`,
  before the native readers, each weapon slot is made what the record says:
  a picked gun the record does not name is restored and its gun put back
  in its body's drop (a rollback undoing the pickup); a pick the record
  names and the slot lacks is installed without the switch (its gun the
  one let go, else an untaken drop's of the same recipe, else cooked from
  the recipe's profile kit slot and waited for; set_user and
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
  inventory): the borrowed gun's loaded stack (its +538h) to
  `weapon_pickup::crate_refill`: the weapon slot's bags, those of each
  slot the parked own gun binds (its +5ACh, the picker's slot indices),
  the kit's allowance there (profile +98h + kit x 704h + 4 x slot +2, none
  for an item FFFFh or an allowance 0, as resupply_from_profile
  0x2FDD56..0x2FDD71) in bags of the own ammunition (the own stack there,
  parked where a borrowed one is), in whole clips of the borrowed
  ammunition, less the gun's magazine and chamber, never lowered, free;
  its other stacks left; each config's `parameters/clip_size` and
  `parameters/required_bags` (integers; the stack's dictionary entry
  +25Ch, its config +114h, the root +108h; the lobby's
  process_default_relocate_item 0x289EB2..0x289EF3 reads the same); the
  real inventory's weight marked, EAX the view (the native refill then
  refills the own parked stacks, charging the own storage). Logged: `place
  P crate: the weapon slot's own ammo: slot S, the kit's N of ammo O (clip
  C/B) = bags` for each, then `place P crate: slot S ammo A before ->
  after; the weapon slot's bags = total (clip C/B) less in-gun` for the
  loaded stack and `(not the gun's loaded stack: left)` for the others.
- **The cache**: a gun installed is lent (never freed by its body's end or
  its donor's destruction); kept, it is freed 3 s after its donor's
  remove(1) (the body left the world: `Origin::Death.gone`), or after its
  give-back when that came later (a holder's respawn long after the body
  went), so a rollback that takes it again finds it and cooks nothing.
- **For stage 2b**: `borrowing(thread, player)`; the death's body (player
  object and life) kept with the place's record (below).
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
  vectors) and its cooked gun's model box. Nothing of the body's own
  objects is read on the draw path (83b and 83c froze reading the body's
  model chain there about 0.2 s after a death): the box is read once, on
  the logic thread, when the cook is logged made (`after_move`), from the
  cooked gun, whose model is the same resource (weapon+1810h, set by
  load_weapon 0x243A9A at the weapon's cook, 0x24B0DC -> +108h the render
  reference -> +0 its skeleton instance -> +62B4h its resource -> +108h, 6
  f32, turned by Y(pi): [-maxx, miny, -maxz] .. [-minx, maxy, -minz];
  refused when not finite, inverted or a side over 4 m). Eight gun models
  of the data (A-545, AK-74U, AKMN, IZh-27, Mosin, TOZ-34, TOZ-122, Vepr)
  had the exporter's default box, +-1 m, so their target was a 2 m cube
  (83f41: "too big", offered off the gun); the content edit list
  `weapon-boxes-1.json` gives them their vertices' box (the skinned
  vertices are on the GPU only: the game has no other box of them). A new
  drop's box is logged once by the viewer: `place P: the gun box of place
  T at (x, y, z), half sizes (a, b, c) mm`, world, along its own axes.
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
  transform) and the cooked gun's model box (weapon+1810h -> +108h -> +0 ->
  +62B4h -> +108h, 6 f32, turned by Y(pi)); hide a taken one there as patch
  78 hides a carrier's gun.

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
  teardown). A gun lent to a holder is never freed; given back (its holder
  died or left) it is kept again, its 3 s counted from the later of the
  body's removal and the give-back. A holder's inventory keeps its own
  native reference.
- **The wait** (`wait_for`, for the pickup's tick and a loaded record): when the cook
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
  kit and slot; `need_cook(donor player, kit, slot)` finds a gun of that
  source nobody holds or cooks one (origin "a loaded world's") and waits the same
  way. The record's reader is inside `base_player::deserialize` of the 0xE8
  load (network_client::process_dynamic_match_info 0x27C0xx ->
  game_world_core::deserialize 0x2D0B70), a network handler outside any
  tick; the game itself dispatches from a network handler
  (process_static_match_info 0x27BEE7 loops on 0xE5AB0). No new
  reentrancy: the same callbacks as the game's own loop, with the late
  join's guarded as above. A loaded world's gun nobody holds is freed at
  the next frame, or at its donor's destruction.
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
  cargo test and clippy of `client-patches/rust`. No new site
  (player_cook.query removed).

What a cooked gun holds and shares (from the spike's reading of the cooks):

### What a cooked gun holds, and what it shares

From the cooks' constructors in the exe (each cook: +8 its class, +0Ch its
reuse policy; 1 is looked up by name in the resource registry,
`query_result::consider_with_name_registry` 0x108A00; 0 is made anew for each
query):

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

## 83b's and 83c's freeze (2026-10-31, fixed in 83f)

The test builds keep the name they had while this was patch 83 (main took 83 for the HUD options in
rev40; the pickup is now patch 85). 83b and 83c froze about 0.2 s after a death whose cook was in
flight: 83c's per-step trace (removed in 83f) ended each time at `ragdoll_gun: place N, drawn_gun
reading`, on the draw path (`carry.weapon`), which walked the dead body's own gun model chain
(weapon+1810h -> +108h -> +0 -> +62B4h -> +108h) every frame while the body went to ragdoll. 83f
reads the box once from the cooked gun on the logic thread (above) and takes only the site's
transform there. (The 704-byte allocator's out of memory 19.6 min later in 83b's log is the network
match client's packet pool, `match_client_impl` 0x385910, filling while the game was frozen: a
symptom.)

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
