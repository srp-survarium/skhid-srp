# The VSSK rifles' built-ins

The owner (2026-11-06): the SV-1367 Dusk was the VKS "Vychlop" recoloured, with three free bonuses
at the same price. "One should have aiming speed on Vychlop, other should have reload speed on Dusk."
Then (2026-11-08): the Vychlop gets weight -15 % instead of its aiming speed.

- Vychlop (`vssk`, 365): one built-in, 2064: weight -15 % (`parameters/weight` x 0.85,
  7 kg -> 5.95 kg). It is the weight roll of the same tier as 2044: the strongest of its kind,
  drop weight 4.096. From rev51 to rev53 it was 2074, aiming time -10 % (0.4 s -> 0.36 s).
- Dusk (`vssk_camo`, 2250): one built-in, 2044: reload +15 % (`parameters/reload_time` x 0.85,
  4 s -> 3.4 s). It is the reload roll of the same tier: the strongest of its kind,
  drop weight 4.096.
- Dusk lost chambering -15 % (2054), aiming time -10 % (2074) and reliability -25 % (2094, wear
  only; wear is off) in rev51.
- Both stay at 165 000. Rarity is not touched (Dusk 3). Every other stat of the two guns is the same.
- Owned guns: a built-in belongs to the item's config, not the saved item, so every owned gun
  changes with the content patch, with no migration. The roll pool leaves out a type an item
  holds, built-ins included. So a Dusk can roll chambering and aiming time but not reload, and a
  Vychlop can roll aiming time again but no longer weight.
- An owned Vychlop that already rolled weight keeps that roll; the cook adds the two (2064 and a
  -10 % roll make 7 kg x 0.75). A reroll of that roll offers other types only. A Vychlop's own
  aiming-time roll no longer stacks with a built-in. None of the 4 live Dusks has a modification
  of its own (rev50 backup).
- Data: the cells `weight_bonus` (decade 206, the column since this change) and `reload_bonus` of
  `weapon-stats.tsv`. Vychlop's config had no `default_modifications`, so `content_edit.py` makes
  the list when a bonus column needs it.
- The item tables (`docs/data/weapons.tsv`: comes_with, can_roll) are made at release.
- A simulation change: the `next` row of `simulation_changes.tsv`.
