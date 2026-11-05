# Animation ids the registry does not have (the rev47 crash at 0x1F18C8)

What is known of the crash, and the diagnostic (patch 95) that names the missing clip the next
time it happens. The root cause is **not proven**: no dump of the crash has the heap.

## The crash

- Every report: `player::mt_draw` 0x2476AD -> the held item's `compute_bones_matrices`
  (weapon_core 0x2E2F50, through the item's vtable +44h from 0x2DA6F7) -> 0x2FF643 ->
  `n_ary_tree::compute_bones_matrices` 0x39F77F -> the bone_matrices_computer constructor
  0x3B19A5 -> 0x395D70 -> 0x1F5520 -> 0x1F5650 -> 0x1F18B0, which reads
  `[registry+E044h][id]` with id 0FFFFh. The id is an animation interval's (+8 of a 20-byte
  interval, the player's animation tree): the held weapon is drawn from the player's tree.
- The game log has `animation <ERROR> value[0x0000ffff] - bits[10]` (n_ary_tree_serializer's
  bit writer 0x396180) for some seconds before: the tree held the 0FFFFh and was saved into
  the hashed world (a desync on top of the crash).

## The registry

- One global object, 0x2247D00 (game code writes the address as a constant: 0x2D7E6E,
  0x2D7F2F, 0x1F2680, 0x2DA6D4). A map at +E030h keyed by a pair of resource pointers, each
  entry `{first, second, id: u16}` with its tree node after it; the ids' vector at +E044h /
  +E048h; +10050h set once the entries are numbered.
- `animations_registry::register_animation` 0x2EF130 (registry, &first, &second) inserts a
  pair. Its callers are the animation registrators (the weapon states' 0x24B700..0x24B946,
  0x30D616.., 0x30F2C6.., 0x30FC06.., 0x3300BC, 0x3388DC/0x338911 for the weapon's user
  animations, 0x338ED6, 0x339176, 0x2ADDBD), reached only from `player::register_animations`
  0x2D8100: the player's inventory slots 7 and 10 (inventory +128h, +134h; vtable +20h of each,
  `weapon_core::register_animations` 0x2DF720 for a gun).
- 0x2D8100 runs at the world's load for every kit player of every place (0x2CBF9B), then
  `actualize` 0x2EF280 (0x2CBFD7) sorts the vector (0x2EF490, then 0x2EF530) and numbers
  it, once (+10050h). A late join's install registers the newcomer's kit players and appends
  them without renumbering (`lj_native_animations`, late_join_install_026g.c).
- `~game_world` 0x1F7100 empties it (vector end = begin, the map cleared by 0x2EF590, +10050h
  0). Nothing else removes an entry.
- The lookup 0x2D6590 (EAX the pair, the registry pushed, ret 4; only caller
  `base_player::animation_id_resolver` 0x2D7B90 at 0x2D7C26, a boost function the player's
  animation player holds at +E000h) returns the entry's id, or 0FFFFh for a pair not in the
  map. Nothing checks it.
- A resource's name: `resource_base::m_name_registry_entry` at +B0h, the entry's name at +8
  (0.10b `name_registry_entry { class_id, associated, name, ... }`).

## What the dumps show

- The 2026-11-03 full-memory dump (rev44 build, `local-clients-rev43-new-weapons`): 221
  entries, all named; 219 are one resource twice, two are 1st-view/3rd-view pairs (the Uzi's
  hands-only hide and show). Only the families of the guns the kits carried at the load are
  there (the AK rifle user animations, Mosin, SCAR-H, TOZ-34, Uzi, VHS-2, two TT-33 clips,
  the key objects). Every one of the 33 kit players' slot 7, slot 10, held and pending items,
  their registrators run under Unicorn on the dump's memory, asks only for pairs the map has
  (334 to 342 pairs a gun). So in that state no registered object was missing one.
- The owner's report of 2026-11-04 15:42 (rev47, match -521412091, 8 players at the start,
  places up to 17 by the end): the 0FFFFh errors begin at 15:42:14.864, about 602 s of match
  time (the match started at 15:32:36.272 with 23 700 ms on the clock). In
  `srp_patches.log`, place 17 (a late joiner) picked up the gun of place 2 kit 1 slot 7 at
  600 950 ms and the switch installed it at 601 750 ms; rollbacks then re-installed it from a
  second copy (0x7C971D48, then 0x7CD3F7C8). The timing makes the pickup a strong lead, not a
  proof: a picked gun's clips are the donor gun's, and the donor's kit player registered them.

## The diagnostic (patch 95)

`rust/patches/src/animation_id_log.rs`, one lohook `animations.id_missing` (0x2D65EE, the
lookup's 0FFFFh exit: `pop edi / pop esi / mov eax,0FFFFh`, replayed). Each distinct missing
pair, at most 8 a run, gives `srp_patches.log`:

```
srp: animation_id: no id for 0x<first> <its name>
srp: animation_id:   paired with 0x<second> <its name>        (a pair of two resources only)
srp: animation_id:   registry of <n> ids; asked from 0x<the resolver's return address>
srp: animation_id:   stack above: <six words above the resolver's arguments>
```

The game's behaviour is unchanged (the lookup still returns 0FFFFh): nothing a match computes changes,
no row in `simulation_changes.tsv`. With the name, the next live crash says which clip (and so
which weapon state or user animation) nobody registered, and the fix is to register it at the
point its object enters the match, the same tick on every client.

Checked: `check_animation_id_log_026g.py` (Unicorn) runs the lookup from its entry in the
original and the patched exe on a made-up registry. **Not run in a game.**
