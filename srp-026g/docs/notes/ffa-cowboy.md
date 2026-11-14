# Free for all: everyone has a Colt Python

The owner (2026-11-14): a version of Free for All (`ffa.md`) "where everyone has colt Python".
srp rewrites each player's match profile before the static info is sent
(`survarium::v026g::cowboy::arm`); every game cooks each life from those profiles, so every client,
a late joiner and a replay see the same kits.

## Rules

- **The gun:** the dictionary item named `magnum` (Colt Python), looked up by name. srp refuses to
  start on game data without it, its states, its fit for slot 7, its ammunition or its amount
  (`cowboy::check`).
- **One Python a kit, in slot 7.** Slot 10 is emptied: everyone has the same one gun, and a second
  Python would only save a reload.
- **Every kit**, loaded or not, and a kit with nothing in it too, gets the Python. A kit's state
  stays: a kit that was not loaded is still not loaded.
- **Ammunition:** the first ammunition of the dictionary the Python takes (`compatible`) that fits
  slot 8: `ammo_.357m`.
- **Rounds:** the dictionary's `optimal_count` of the Python's category (pistols, 17): 40, two clips
  of 20. All of them in slot 8; every other ammo slot (9, 11, 12, 19..22) is emptied.
- **Everything else stays:** armour, pockets (medicine, grenades, boosters, traps), artefacts, the
  kit's skills (modifiers), its name and id. The pockets stay packed.
- **Items:** the Python and its ammunition are one new item each in the profile's table, held by
  every kit: whole (`FULL_CONDITION`; the ammunition's stack is the 40 rounds), no modifications,
  no upgrades. Their ids are the lowest ones from 1 that the profile's items did not have. An item
  no kit holds any more (the old guns and ammunition) leaves the table, so a profile never grows
  past the 69 items a profile holds.
