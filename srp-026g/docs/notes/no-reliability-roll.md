# No reliability rolls

The owner, 2026-10-23: the "Надежность" (Reliability) modification must never roll when a player
upgrades or rerolls an item; equipment wear was removed (#35), so it does nothing and wastes a
roll. Code: `survarium::v026g::modifications` (`Modification::rolls`, `Modifications::candidates`),
`session::v026g::items_rules` (`craft`, `reroll`, `roll_modifications`).

- **Which:** the modifications that change nothing but wear (`degrade_per_match`, `_per_use`,
  `_per_death`): 1200..1204 on gear (type 32) and 2090..2094 on weapons (type 10), name
  `st_modify_reliability`, the lobby's property 123 ("Reliability", "Надійність", "Надежность").
  Found by what they change, not by a list of ids; nothing else shows that property.
- **Never rolled:** not by an upgrade's success, not among a reroll's three, not on a drop. One
  pool for all of them (`candidates`), so a new roll path leaves it out too.
- **Owned ones stay:** an item that has reliability keeps it, loads and saves as it was; no
  migration. Rerolling it offers three others (never reliability), as for any modification.
- **Items of few types:** backpacks, masks and respirators, oxygen packs (19 items, the Bandit
  balaclava among them) had three types that roll, reliability one of them; two are left. They
  reach two modifications of their own by rolls; a drop of rarity 3 has two (its rank shows 2).
  No item is left with nothing to roll.
- **Nothing left to roll:** an upgrade of such an item holding its two types is refused before
  anything is taken (the lobby's "cannot upgrade" answer, `st_cannot_craft_item`); the donor
  and the money stay. The reliability of a full one (its two other types held) can not be
  rerolled: nothing to offer, refused, nothing paid.
- **Not changed:** what reliability does to wear where wear is on (`SRP_ITEM_WEAR=on`); the
  lobby's gold for its best value (an owned 25 % one is still the best of its kind); a reroll
  offer made and paid for before this change is kept as it was.
- **Release:** server only; no content or exe change.
