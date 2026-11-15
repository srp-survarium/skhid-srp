# A key per item type (0.26g)

The owner's ask (2026-11-15): "let's have hotkeys for using specific artefact type or specific
medkit in the game"; then (the same day) traps, grenades ("two keys for different grenades") and
oxygen tanks too; then "Next artifact" and "Next medkit" (no "Next grenade": G already throws
the next).

## The keys

- **Fourteen actions, one per live item type** a quick slot can hold:
  - artefacts: Onyx, Spring, Lifebone (Larkspur);
  - medicines: Pack of bandages, Medikit, Military, Black Market, Science medkits, Antitoxin,
    Anesthetic (painkiller);
  - grenades: RGD-5, smoke grenade RDG-M;
  - the trap;
  - the oxygen tank;
  - and two group keys: **Next artifact**, **Next medkit** (below).
- **No default keys.** Options, Controls, the game group: one row each ("Use Onyx", "Throw
  RGD-5 grenade", "Use oxygen tank", ...; `st_use_<item>_action` in every
  `client-patches/localization/*.tsv`). Bound and saved as any key (`bind use_onyx kF5` in
  `user.cfg`).
- Action names: `use_onyx`, `use_spring`, `use_lifebone`, `use_bandages`, `use_medkit`,
  `use_military_medkit`, `use_blackmarket_medkit`, `use_science_medkit`, `use_antitoxin`,
  `use_painkiller`, `use_rgd5`, `use_rdgm`, `use_trap`, `use_oxygen_tank`, `use_next_artefact`,
  `use_next_medkit`.

## What a key does

- **A key is a press of the quick-use key of the quick slot that holds its item** (keys 3..8).
  The same input bit, under the same condition: while the game would set the quick-use bit
  for that slot's key (its event down, held or up this tick, or held from before).
- **The slot:** the first of the six quick slots, in the hud's order, whose item is that type
  (the pockets, then the oxygen tank on the back, then the artefact containers).
- **Nothing of that type in the six quick slots: nothing happens.** An item past the sixth
  quick slot has no quick key, so no item key either.
- **Artefacts:** as the quick key: every ready copy of the kind fires (`artefacts.md`). With H
  held it is patch 78's chord, as with the quick key (`held_artefact_transfer.md`).
- **Grenades:** the press throws that grenade, as its pocket's quick key does: the game gives
  that pocket's grenade set the press and the release, as G does the first grenade pocket's.
  G is unchanged.
- **The oxygen tank:** any oxygen pack (its class, whatever the item); a kit has one back.
- **Next artifact / Next medkit** (the owner's pick, 2026-11-15): the first item of the group, in
  quick-slot order, that a press of its quick key would use now. No cycling, no state.
  - the artefacts: any artefact in a container, ready: passive (not active), not cooling, and one
    of the 4 of its kind that count (patch 109: a copy past them never fires). A cooling Onyx, or
    a fifth Onyx, is skipped for a ready Spring.
  - the medkits: the bandages and the four medkits (not the antitoxin, not the painkiller), ready:
    not in use (its activity, the use delay included) and not used up.
  - none ready in the six quick slots: nothing happens.
- **Next grenade is G:** the game's throw_grenade already throws the first grenade pocket that is
  no empty stack (first_grenade_set_slot 0x2FE8B0), the same rule; its Controls label is now
  "Next grenade" in every language (`st_throw_grenade_action`; the action, its key G and what it
  does unchanged).
- **Two keys at once:** both quick bits.
- **A held key follows its type:** it is looked up again each tick, as the bits are made.

## Client only

- Only the input a key makes changes: bits the game already has (the quick-use bits 14..19).
  No new input bit, no simulation change (no `simulation_changes.tsv` row), no server, bot or
  movie change. Others, replays and games without the patch play the relayed bits. The same
  reasoning as patch 34 (`client_patch.md` 3.32).

## How (exe patch 149; `client_patch.md` 3.82)

- **The action table moved** into the arena (`item_hotkeys_026g.asm`): the game's 60 rows and
  the fourteen. The game's table ends where the key names begin, so it cannot grow in place.
- **The key binder's bindings grown**, 48h to 60h: the binder's own allocation made bigger, its
  array grown in place and its game pointer (+360h) moved after it, every reader repointed.
  Ours are ids 4Ah..59h; 48h and 49h stay the game's "none" values.
- **The input:** at the input builder's end, before patch 78's chord (`item_hotkeys.rs`). Ready is
  read as the game's own action reads it: an artefact's state and cooldown, a medkit's active
  flag and count.
- **The Controls rows:** patch 62's `controls` table (`hide_ui_026g.asm`), 16 more rows.

## Picks the owner may change

- One key for any oxygen tank (not a key per tank type): a kit has one back slot.
- Grenade labels say "Throw" ("Throw RGD-5 grenade"), the others "Use".
- The first quick slot of a type, when two hold it (only artefacts can: one pocket a type).
- Room for 6 more actions (ids 5Ah..5Fh) before the bindings must grow again.
- Not covered: boosters (carried, not used), items not in the shop or starting kits.
