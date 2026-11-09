# Animation ids the registry does not have (the rev47 crash at 0x1F18C8)

What is known of the crash, and the diagnostic (patch 95) that names the missing clip the next
time it happens. The cause (2026-11-05, "The cause" below): a picked-up gun's copy that is not
the recipe's gun, the bug `fix/pickup-state-length` (#1246) fixes; no separate registry bug.

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
  animations, 0x338ED6, 0x339176, 0x2ADDBD), reached from two places of the world's load:
  `player::register_animations` 0x2D8100, the player's inventory slots 7 and 10 (inventory
  +128h, +134h; vtable +20h of each, `weapon_core::register_animations` 0x2DF720 for a gun),
  and each match rule's vtable +28h (0x2CBE19; the batteries' rule 0x313FD0 registers each
  battery item through its vtable +20h).
- 0x2D8100 runs at the world's load for every kit player of every place (0x2CBF9B), then
  `actualize` 0x2EF280 (0x2CBFD7) sorts the vector (0x2EF490, then 0x2EF530) and numbers
  it, once (+10050h). A late join's install registers the newcomer's kit players and appends
  them without renumbering (the client's `late_join/animations.rs`; `lj_native_animations` of late_join_install_026g.c before its Rust port).
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

## What was ruled out (2026-11-05)

- **A cooked copy with its own resources** (a clip loaded again under a new pointer, so the
  pointer key misses): no. The cooks share animations by name (`weapon_pickup.md`), the
  registry's entries hold a reference to each, and in the full dump the four live cooked
  copies (0x7CA50C98, 0x7CB560D8, 0x7C88E830, 0x7C8975A8, each named by its cook's
  `srp_patches.log` line) register only pairs the map has (334 to 342 each), as the kit guns do.
- **The user's side** (the picker's animations paired with another gun family's): the player's
  clips while holding a gun come from the gun's own `user_animations` object
  (`[weapon+278h]+48h`, made from the gun's config, 0x30AAE1), registered with the gun: one
  object for every rifle-family gun of the dump (0x7C15C290).
- **Lexemes outside the registered tables**: every lexeme maker (0x31EBD0, 0x31EAB0,
  0x31E960 and their 31 callers: the weapon states, the shotgun and grenade substates, the
  melee state, the battery states 0x344F60/0x345070, the user animations 0x3300E0) takes its
  pair from the same arrays the registrators register (for example the two-clip reload
  0x30FE90: +144h + 8 x (empty + 2 x crouch), registered by 0x24B820 as four pairs from +144h).
  Indices are computed on the spot from the gun's state (ammo, posture). A state's current
  clip, a byte at +134h relative to +130h (0x30FEE4), is saved and loaded raw
  (`weapon_core_animation_end_aware_state::serialize` 0x30DC90, `deserialize` 0x30DBB0) but
  only compared with an ending clip (0x30DB7D, 0x310692, 0x310CF0), never looked up.

So a missing pair needs a clip pointer that is in no registered table: a resource the load
never registered, held by an object the load never saw. Not found statically; the names the
diagnostic logs will say which.

## The cause (2026-11-05)

- **The live crash** (the owner's 15:42 match): place 17's pick at 601 750 ms installed copy
  0x7CD3F7C8, which the owner's `srp_patches.log` shows cooked in the match before (line
  86 178, "the gun of place 2 kit 1 slot 7" of that match) and handed out again by recipe
  alone. That match's gun need not be in this one; when no kit of this match carries it, its
  clips were never registered here (the registry is emptied at each `~game_world`) and every
  lookup of them is 0FFFFh. #1246's "a copy is its match's" frees the copies of the match
  before; a copy of this match is a kit gun of this match, registered at its load.
- **The replay repro** (School 0x71A7A62A, key frame 789 800, #1246's build, full dump
  `SRP-crash-20261105-043142-405`): `convert_to_object_matrices` 0x3B336C reads past a clip's
  bone table. The clip is id 278, `ots14/item/stand/3rd_view/idle`, in the element of place
  6's tree (player +77ACh, 0B0h each, node at +0A8h, its target +30h) whose target is the
  picked VHS-2 copy 0x7CB0D110: an OTs-14 clip on a VHS-2 skeleton. Place 6 has no OTs-14 (its
  guns: the 9A-91, its own SVD parked, the VHS-2). The registry's numbering is the same on
  every client: each kit player's new pairs appended in roster order (the 0x2CBF9B call goes to
  the late join's `register_animations`, which appends and numbers; actualize then finds it
  done), checked on the dump's 545 entries kit player by kit player, and the other held guns'
  elements decode right at high ids (place 10's PKP, 472; place 15's SV-98, 456). So the
  recording's writer, a rev47 game, had an OTs-14 under that pick: a copy holding another gun
  (#1246's "every client installs the recipe's gun"). The reader then puts the writer's tree
  on its own, right, gun. Replays recorded before #1246's simulation change are refused.

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
