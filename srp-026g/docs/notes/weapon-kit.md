# The weapon kit: six upgrades on a weapon in three records

**Version scope:** client 0.26g (build 2777), srp's lobby and match servers, client patch 68
(`../../client-patches/weapon_kit_026g.asm`, `weapon_kit_026g.c`). Tags: PROVEN (read at the
address or run with Unicorn), [srp] for srp's own choice.

## The ask

The owner (2026-10-25): weapons should hold two decals, a camouflage, a scope, a grip ("hand") and
a muzzle device ("fire podavitel'"), "at least 6 slots there for weapons only". Stock 0.26g gave
weapons no upgrade slots at all; srp's camouflage (patch 44), decal (patch 47) and scope (#153) use
the armour's upgrade records.

## Why three records

- The client keeps at most three upgrade records an item: the item record (0xB4 bytes, built at
  0x2F7C10) holds them inline at +0x6C..+0xB4, its buffer_vector at +0x60; the reader
  (inventory_item_descr 0x1F1350 → 0x1F13D0, the lobby's and a match profile's alike) sizes it
  at 0x1F147F, and more than three is the fatal assert at 0x1F1655 / 0x1F16C4. PROVEN.
- The item config builder (0x2F14C0, six callers) keeps the records' values in a stack array of
  three (0x2F157F..0x2F161F) for its cache key's checksum. PROVEN.
- Growing the record is out of reach (its size is a stride at about a hundred sites and the
  profile's item table is inline, profile+0x1568).

## The slots and the record [srp]

A weapon's upgrades are kept by the server in **logical slots**, one upgrade a slot:

| slot | what | on the wire |
|---|---|---|
| 0, 1 | a decal (its value its place) | a record of its own, as now |
| 2 | the camouflage | named by the kit, or the kit's own |
| 3 | the scope (a variant, `weapon_scopes.md`) | the kit's own |
| 4 | a muzzle device (flash hider, brake) | named by the kit, or the kit's own |
| 5 | a grip | named by the kit, or the kit's own |

The account stores them so (no rewrite: slots 0, 2 and 3 are those weapons had before). They go
to the client packed (`survarium::v026g::weapon_kit::pack`, `session::v026g::wire::upgrades_descr`):
the decals of slots 0 and 1 in that order, then one **kit** record in slot 3. The kit is the first
of scope, camouflage, muzzle, grip the weapon wears (its item id and record dict id, as that one's
own record would be); its value:

```text
bits  0..15  the camouflage's dict id (0: none, or it is the kit's own)
bits 16..31  the muzzle device's dict id
bits 32..47  the grip's dict id
bits 48..55  the kit's own logical slot (2, 3, 4 or 5)
bits 56..63  0xA7, the kit's mark
```

So a weapon is sent at most three records whatever it wears. Armour is sent as before.

## The client (patch 68)

- **The config.** item_config_cook::translate_query (0x2F1750) builds an item's config from the
  records its query points at: 0x2F2060 copies the query's {dict id, modifications*, upgrades*,
  dictionary} to [ebp+38h..44h], and the function reads the upgrades' buffer_vector at [ebp+40h]
  twice only, its size at 0x2F1799 and each record's dict id and value in its loop
  0x2F18D0..0x2F195A (each record: its item's `parameters.modification`, the value as the
  modification's `<low_param>`/`<high_param>`, 0x3584E0). PROVEN. Patch 68 jumps from its `call
  0x2F2060` (0x2F1771): when a record is a kit, a list on translate_query's own stack (below its
  frame, as its allocas; its epilogue takes esp from ebp, 0x2F2041) holds the records and one
  `{0, dict id, 0, slot}` for each dict id the kit names, and [ebp+40h] points at it. At most six.
  A dict id is left out when the dictionary lacks it, when the list holds it already (its
  modification would add a second `camouflage`: the config's duplicate-name assert), or when a
  record that is no kit is in its slot. Without a kit nothing changes (the original's stack too);
  nothing is shared between two cooks.
- **Every copy carries the kit.** The item record is never written: the lobby's inventory and
  kits, a match's static info (its profiles, a late join's), a replay's recorded static info all
  carry the three records as they are. The config builder's cache key checksums the kit's value,
  so each combination is its own config.
- **The lobby movie.** The upgrades to Flash (0x287E30, its loop 0x2881E0) give each record's object `id` and
  `param`; patch 68 (0x2882E3) gives a kit's object `kit_lo` and `kit_hi` (its value's halves).
  `UpgradeWindow.kitUpgrades` makes the window's slots of them: the kit's own item in its own slot,
  each dict id it names as the inventory's item of that id (`InventoryList.getInventoryItemOf`).
- **What a cosmetic is attached to.** setup_inventory_data (0x288520) maps each record's item id
  to its host ({id, host} pairs pushed at 0x2885C9, an alloca of 50h bytes an item) and gives each
  item its `is_attached_to` from them. Patch 68 pushes, after a kit's own pair, one for each dict
  id the kit names: the inventory's item of that id with the lowest item id (as the movie's
  `getInventoryItemOf`). A scoped weapon's camouflage shows worn on it.
- **The preview (patch 48).** Its `apply` calls `weapon_kit_preview_apply` through a dword the
  build writes: the queued change on the logical slots, packed again as the server packs. A muzzle
  device's or grip's record holds its own dict id (patch 48 looks for a scope's variant only
  outside 2601..2699).
- **The window** (the lobby movie, `UpgradeWindow`): the Attachments page has the scope's, the
  muzzle device's and the grip's slots (`weapon-camo.md`, the weapon's window: two pages). Muzzle
  devices are dict ids 2601..2649, grips 2650..2699 (`ItemDictId`, `devices.tsv`): category 22 as
  the scopes, told apart by the range.
- **The client never writes a weapon's record itself**: srp answers a weapon's `46 01` and `46 02`
  with the inventory and the account only (no `83 46 01` / `02`, whose own copy of the record would
  be a fourth, or take the kit off for its own).

## The muzzle device (slot 4)

[srp, the owner's rules of 2026-10-25] One slot takes any muzzle device: flash hiders and brakes
from 0.46c2 (no silencers). A device is an item of category 22 (the lobby's attachments filter)
whose `parameters/type` is `srp_muzl` (the server's, `modifications::MUZZLE_KIND`: slot 4 of a
weapon with a `data` table) and whose modification adds `srp_muzzle_model` = its model to the
weapon config's `data` (type 3, as a camouflage adds `camouflage`). The pack
`weapon-attachments` (`scripts/weapon-attachments`: 0.46c2's model, material and textures, the
configs written again in 0.26g's layout) holds them.

### The devices (pack weapon-attachments@3, `scripts/weapon-attachments`)

Every 0.46c2 muzzle model but its silencers: 20 items, dict ids 2601..2620 (`devices.tsv`).

| items | 0.46c2 models | stat | shot effect |
|---|---|---|---|
| Flash hider 1..9 (2601, 2603..2610) | fire_arrestor_01..09 | none | the model's 0.46c2 effect: pp_arrest1, pp_arrest2_3, arrestor4, arrestor5_6_7_8, m4a1 |
| Muzzle brake 1..7, 10 (2602, 2611..2617) | dtk_01..07, fire_arrestor_10 | recoil -0.05 (horizontal, 502/0) or -0.1 (vertical, 502/1: dtk_02, dtk_05) | the weapon's own |
| Compensator 9..11 (2618..2620) | dtk_09..11 | full-damage distance x1.05 (0.46c2's 501/4, effective distance +5 %) | the weapon's own |

- **The stat of a model** is the one most of its 0.46c2 weapons give it (0.46c2 set it per weapon
  and kind: a horizontal, vertical or universal brake). `max_damage_distance` is 0.26g's
  full-damage distance, 0.46c2's `effective_distance`.
- **Not ported:** dtk_08 and dtk_12, the shotguns' "universal" brakes: their only stats are 0.46c2's
  buckshot pattern scales (506, 514: vertical +10 %, horizontal -30 %), which 0.26g's shotguns
  do not have. The silencers (the owner's rule).
- **Which weapons** (`fit.tsv`, `survey_046.py`): a device fits each 0.26g weapon to which 0.46c2
  gave its model in any muzzle kind (a premium copy as its plain weapon; 0.46c2's `a545` is
  0.26g's `a_545`): 111 pairs, as `items_compatibility` rows of the dictionary
  (content-edit `weapon-muzzle-fit-1`, `make_edit_list.py`). The server puts a device only on a
  weapon of such a row (`modifications::upgrade_slots_in`); the lobby movie takes the rows into
  its pairs (`UpgradeWindow.addCompatibility`): only a device that fits lights its slot, the
  others are greyed, a device's card lists its weapons, a weapon's hover lights its devices, and
  a weapon no device fits has no muzzle slot. No device fits the VSS, Val, VSSK (their
  suppressors), the Mosin, PPSh, TT-33, TOZ-34, TOZ-66, Lancaster, Winchester, NeoStead.
- **The shop:** seller 0 at no rank, at the item's `base_cost` (`prices::sellers_of_in`): flash
  hiders 6,000, brakes 9,000 (-0.05) or 12,000 (-0.1), compensators 8,000 [srp's guess, the owner's
  to set]. Sold back for a quarter, as everything.
- **Icons:** a shop scope's cell for now (`devices.tsv` icon row and column).

### Where a device sits on each weapon (`offsets.tsv`)

0.26g's weapon models keep their stock muzzle devices in the mesh (0.46c2 made them separate
parts, `ak_74m_muzzle_brake.model`, and took them off for an attachment). barrel_point is the bare
barrel's end: on 23 weapons the mesh reaches past it. `data/srp_muzzle_offset` (content-edit
`weapon-muzzle-fit-1`) starts the device at the mesh's front there: the stock brake stays and the
device is in front of it. Measured from each model's bounding box (barrel_point's z against the
box's front), not seen in a game:

- **Offsets set:** AK-74M, AK-74N (and premium) 2.9 cm; AK-12 (and premium) 2.5; ASh-12 5.7;
  FN FNC 3.5; Fort-17 1.7; HK416 2.1; KRISS Vector 4.1; M4A1 -1.7 (the mesh ends short of
  barrel_point); MP7 1.4, MP7 with silencer 1.3; MSBS 2.6; Pecheneg 0.5; PP-2000 3.2; SR-3M (and
  premium) 2.6; SVD 7.4; Valmet 4.6; Vityaz (and premium) 3.2; vz.58 0.8.
- **Will look wrong (a device on a stock brake or flash hider):** the AK-74M/N, AK-12, ASh-12,
  SVD, Valmet, Vityaz, KRISS, FNC, HK416, PP-2000, SR-3M; on the VSK-94 the device sits on its
  integral suppressor.
- **Not measured (no bounding box in the model):** A-545, AK-74U (and premium), AKMN (and premiums),
  TOZ-122, Vepr (and premium): offset 0. **To check:** AEK-919K, Magnum and RPK, whose boxes reach
  far past barrel_point (a stock, a bipod?): offset 0.

How the client draws it (patch 68, PROVEN in the code, run with Unicorn; not run in a game):

- **Loading.** The weapon cook (0x24A920) asks for a weapon's sub-resources in one query: the
  scope's config when it names one (class 4Dh), the model (10h), each user animation (31h), the
  bullet shells' and shot effects' particles (3Dh). With `data/srp_muzzle_model` it asks for
  that model too (class 0Fh, a static model's instance, as the scope cook asks for its models),
  last; the requests' alloca has one more place (0x24AA04). The query's callback (0x24ACF0)
  finds it last among the results and keeps it.
- **The weapon's own part.** The weapon (weapon_cook 0x24A780: 4710h bytes and a pointer for each
  animation, shell and shot effect) is 50h bytes more; after its last array: the device's model,
  its offset, whether it is in the scene, its matrix. Zeroed when the weapon is made, the model
  released when the weapon is (delete_resource 0x24B190).
- **Drawing.** Wherever the game adds, removes or moves the weapon's model (on_show, on_hide,
  the aimed scope's add and remove that hide the weapon, draw), the device's model goes with it,
  at barrel_point's matrix of the frame (+0x620) moved `data/srp_muzzle_offset` metres forward
  along the barrel (default 0).
- **The weapons' own muzzles.** 0.26g's weapon models have their stock muzzle brakes in the mesh;
  a device starts at barrel_point (the muzzle's end, where the shot's flash is) and runs forward,
  `data/srp_muzzle_offset` metres on (`offsets.tsv`, below).
- **The shot's flash.** Each device's modification also adds `data/srp_muzzle_fire_mm`, the
  device's `fire_point` locator in millimetres (its model's: 42 for flash hider 04, 48 for brake
  04; an int, as every type-3 value of 0.26g's is an int or a string).
  After the device's update in draw, barrel_point's point (+0x620) moves `offset + fire` forward:
  update_pfx_transform (0x244570, right after) and the shot effects (0x244120) take the flash
  from there, at the device's front as in 0.46c2. barrel_point is computed again at each draw
  (0x244E68), so it never moves twice; a weapon hidden by its aimed scope keeps the stock point.

## A muzzle device's stats and effect [srp, the owner's rules of 2026-10-25]

A device's own modifiers, after the model's (`devices.tsv`, `make_pack.py`; modifier types from
the modification's apply 0x358560: 0 sets the node's value, 1 adds, 2 multiplies, 3 adds a key):

| kind | 0.46c2 | 0.26g modifier |
|---|---|---|
| brake (horizontal) | modification 502 grade 0: recoil -0.05 | `parameters/recoil_modifier` +(-0.05), type 1 |
| brake (vertical) | 502 grade 1: recoil -0.1 | `parameters/recoil_modifier` +(-0.1), type 1 |
| compensator | 501 grade 4: effective distance +0.05 | `parameters/max_damage_distance` x(1 + 0.05), type 2 |
| flash hider | no stats; `effects_override.particles.shoot` | `particles/shoot` = the effect, type 0 |

- `recoil_modifier` is the stock weapon modifications' own (1150 "player recoil" -0.02, type 1
  on `parameters/recoil_modifier`): the kick of a shot is times `max(0, 1 + recoil_modifier)`
  (0x331C50, `bots/recoil.rs`). 2602 is a horizontal brake (dtk_04 is 0.46c2's model of 20
  weapons' horizontal brake).
- A flash hider's effect is 0.46c2's for that model (fire_arrestor_04: `arrestor4`), ported with
  its materials (made again on 0.26g's `fx/weapons/ak_side`) and textures. 0.26g's particles lack
  0.46c2's TargetColor action (construct_action 0x364F10 has no case for type index 14 and
  leaves it out): each of arrestor4's seven gives one constant colour (white, alpha 0.23 to
  0.24), so it becomes an InitColor property of that colour (`widen_config.rs --particle`).
- A match computes a shot's kick from the weapon's config, so the brake changes the simulation
  (`simulation_changes.tsv`, its `next` row). Every client builds the weapon from the same
  records and pack: no server rule.

## What it changes

- Two decals on a weapon (`WEAPON_DECAL_SLOTS` 0 and 1, the window's `WEAPON_DECALS` 2): visual
  only. Patch 47 bakes both.
- The camouflage on a scoped weapon now travels in the kit: visual only.
- Slot 4: 9 flash hiders (their shot effect), 8 brakes (less kick), 3 compensators (a longer
  full-damage distance). Slot 5 has no item yet (a later step: grips).
- A camouflage, decals and a scope change nothing a match computes; the brake's recoil does: the
  `next` row of `simulation_changes.tsv`.

## Old clients

A client without patch 68 shows a kit's own (the scope, or the camouflage when there is none) and
the decals, not what the kit names (a scoped weapon's camouflage): never a crash, never a fourth
record. The release that ships the server change requires patch 68 (`SRP_MIN_PATCH_REVISION` the
release's revision; the exe and server ship together).

## Not done

- A player holding two items of one camouflage sees the lower item id as the one worn (the kit
  names the dict id, not the item).
- arrestor4's TargetColor actions are InitColor of their colour, set at a particle's birth; how
  0.46c2's action reaches it over a particle's life is not known (to be seen in a game).
- Not run in a game.
