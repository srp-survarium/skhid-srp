# Golden weapons: there are none

The owner's rule of 2026-11-14: "we have this debt with premium", "those all should be normal
weapons", "and follow standard rules"; "all code related to gold/premium weapons goes". It
replaces the rules of 2026-10-21/22 (only the silver bonus, as the weapon's own modification; any
copy of a plain weapon a craft donor), which are gone with their code.

- **The 22 golden weapons**: the `*_premium` / `*_premium_2` copies (Kiparis, Uzi ×2, Vityaz, VSS,
  SR-3M, Lancaster ×2, AK-74N, AK-74U, AK-12, AKMN ×2, SKS ×2, Vepr, Vepr-12, Saiga-12, VSK-94,
  Remington 700), the Neostead (plain: `neostead_2`) and the MP7 special (plain: the silenced
  MP7).
- **Owned ones are their plain weapon**: once, at the first start of the release, srp changes each
  one's dict id to its plain weapon's, in the items and the presets' slots
  (`persist::SqliteGameRepository::make_golden_weapons_plain_once`, table `persist::GOLDEN_WEAPONS`,
  `g26_meta` mark `golden_weapons_made_plain`, a copy of srp.db first, a log line per account). The
  item id, modifications, upgrades and kit slots stay. Dry run:
  `python3 scripts/golden-weapons/preview.py <a copy of srp.db>`.
- **A silver bonus (1230) a weapon holds** is a modification like any other: it pays (+10 % silver),
  takes a place, counts for the rank and rerolls; it never rolls (drop weight 0), so once rerolled
  it is gone.
- **Standard rules**: a weapon that was golden upgrades, rerolls and combines as its plain weapon,
  with any copy of it (two SR-3Ms that were golden are two plain SR-3Ms). No gold background or gold
  name, no silver bonus shown on a shop entry.
- **None is made**: the bundles hold the plain weapon (no bonus; their descriptions lose
  "improved"), no shop sells a golden copy (`shop.tsv` `not_sold`), the admin's give refuses one and
  names the plain weapon. `items.tsv` gives them no `like` and no `start`.
- **Unchanged**: the premium armour ([premium_armour.md](premium_armour.md)); the 22 rows of
  `weapon-stats.tsv` and their configs (nobody owns one; old replays read them).
- **Their stats are the plain weapon's**: five copies differed a little in `weapon-stats.tsv` (the
  copy's / the plain weapon's: Kiparis zoom 1.2 / 1.1, Uzi recoil 0.5 / 0.6 and top 0.8-1 / 1-1.2,
  AK-74N aim time 0.35 / 0.3, Vepr-12 weight 3.5 / 3.2 and spread 0.8 / 1, Saiga-12 weight 3.8 /
  3.6); a weapon made plain plays as its plain weapon.
- **Not a simulation change**: pay counts the same modifications, and a replay recorded before plays
  its golden copies with their own unchanged configs.
