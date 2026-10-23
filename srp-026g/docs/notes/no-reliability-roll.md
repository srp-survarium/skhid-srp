# No reliability rolls

The owner, 2026-10-23: the "Надежность" (Reliability) modification must never roll when a player
upgrades or rerolls an item; equipment wear was removed (#35), so it does nothing and wastes a
roll. The same day: the items it left with two types to roll get a third. Code:
`survarium::v026g::modifications` (`Modification::rolls`, `Modifications::candidates`),
`session::v026g::items_rules` (`craft`, `reroll`, `roll_modifications`); the edit list
`scripts/content-edit/third-mod-1.json`.

- **Which:** the modifications that change nothing but wear (`degrade_per_match`, `_per_use`,
  `_per_death`): 1200..1204 on gear (type 32) and 2090..2094 on weapons (type 10), name
  `st_modify_reliability`, the lobby's property 123 ("Reliability", "Надійність", "Надежность").
  Found by what they change, not by a list of ids; nothing else shows that property.
- **Never rolled:** not by an upgrade's success, not among a reroll's three, not on a drop. One
  pool for all of them (`candidates`), so a new roll path leaves it out too.
- **Owned ones stay:** an item that has reliability keeps it, loads and saves as it was; no
  migration. Rerolling it offers three others (never reliability), as for any modification.
- **A third type** (the owner, 2026-10-23) for the 20 live items reliability left with two:
  - the 15 masks and respirators (the stock nine, the Bandit balaclava 1810, the 0.69d masks
    1812-1816, the Army balaclava 1823): type 13 Armor (1000-1004: +2, +3, +4, +5, +8; on a mask
    only its face values exist, as on a helmet only its head's);
  - the 4 oxygen packs (`scavenger_oxygen_1`, `edge_back_3`, `ren_back_3`, `ren_back_5`): type 24
    Weight limit (1120-1124: +1..+5 kg) and type 28 Energy costs (1160-1164: −2..−10 %);
  - the "Pilgrim" container (`edge_back_6`, whose default Regeneration takes type 21): type 28
    Energy costs.
  - Each is a character of the item config's `parameters/available_modifications_mask`, which
    only the server reads. The client shows any modification the server sends, from its own
    `modifications.options`, so the bonus shows without the content patch (the edited configs
    ship in it all the same).
- **With it, every live item that rolls has three types or more** (checked on the game data
  `release/build-content` makes): an upgrade is never refused for "nothing to roll", and a full
  item's reliability always has another type to turn into. The other five backpacks had a third
  already (Regeneration, `backpacks-1.json`).
- **The guards stay:** an upgrade of an item with nothing left to roll is refused before anything
  is taken; a reroll with nothing to offer is refused, nothing paid. Neither happens on live data.
  The stock data the unit tests read (no edit lists) still has such items, and its tests keep
  covering them.
- **To check in a match before the release:** that a player modifier on a mask or oxygen item
  takes effect, e.g. the stamina drain with an Energy-costs oxygen pack. The match applies an
  item's modifications to its config by path, and these configs carry every player-modifier key
  at 0 (LIKELY; never tried on a mask or an oxygen pack).
- **Not changed:** what reliability does to wear where wear is on (`SRP_ITEM_WEAR=on`); the
  lobby's gold for its best value (an owned 25 % one is still the best of its kind); a reroll
  offer made and paid for before this change is kept as it was.
- **Release:** the server and a content patch (the edit list's 20 item configs).
