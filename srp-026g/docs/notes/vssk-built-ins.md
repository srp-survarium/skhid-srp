# The VSSK rifles' built-ins

The owner (2026-11-06): the SV-1367 Dusk was the VKS "Vychlop" recoloured, with three free bonuses
at the same price. "One should have aiming speed on Vychlop, other should have reload speed on Dusk."

- Vychlop (`vssk`, 365): one built-in, 2074: aiming time -10 % (`parameters/aim_time` x 0.9,
  0.4 s -> 0.36 s). This is the row Dusk had.
- Dusk (`vssk_camo`, 2250): one built-in, 2044: reload +15 % (`parameters/reload_time` x 0.85,
  4 s -> 3.4 s). It is the reload roll of the same tier as 2074: the strongest of its kind,
  drop weight 4.096.
- Dusk loses chambering -15 % (2054), aiming time -10 % (2074) and reliability -25 % (2094, wear
  only; wear is off).
- Both stay at 165 000. Every other stat of the two guns is the same.
- Dusk's base rarity is 0, as the Vychlop's (the owner, 2026-11-08: "SV Dusk is considered golden and
  cannot roll upgrades - it should be able to"). Stock it was 3, the golden copies' value: the
  lobby's rank is `min(3, base_rarity_level + modifications)`, so it showed rank 3 and offered
  no craft or roll, though srp allowed them.
- Owned guns: a built-in belongs to the item's config, not the saved item, so every owned gun
  changes with the content patch, with no migration. The roll pool leaves out a type an item
  holds, built-ins included. So a Dusk can now roll chambering and aiming time but no longer
  reload, and a Vychlop can no longer roll aiming time.
- An owned Vychlop that already rolled aiming time keeps that roll (9 of the 30 live, rev50 backup).
  The cook adds the two: 2074 and its own -10 % make 0.4 s x 0.8 = 0.32 s. A reroll of that roll
  offers other types only. None of the 4 live Dusks has a modification of its own.
- Data: the cells `aim_bonus` and `reload_bonus` of `weapon-stats.tsv`. Vychlop's config had no
  `default_modifications`, so `content_edit.py` makes the list when a bonus column needs it.
- A simulation change: the `next` row of `simulation_changes.tsv`.
