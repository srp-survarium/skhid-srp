# Golden weapons

The owner's rules of 2026-10-21. Code: `survarium::v026g::golden`,
`session::v026g::items_rules` (`craft`, `give_golden_silver`), `Account::add_item`; the
data: `scripts/content-edit/golden-weapons-1.json`; the lobby: `ItemDictId` (the lists), `ItemFactory`,
`InventoryList` (gold background, craft donors), `MessageCraftItem`, `LobbyMenu.fill_crafted_item`.

- **The rule:** "remove extra bonuses from all golden weapons and make them upgradable just
  like any other weapon but with the first golden bonus already set"; "remove modifiers except
  for silver from all premium weapons".
- **Golden weapons (22):** the 20 `wpn_*_premium` / `*_premium_2` copies (Kiparis, Uzi ×2,
  Vityaz, VSS, SR-3M, Lancaster ×2, AK-74N, AK-74U, AK-12, AKMN ×2, SKS ×2, Vepr, Vepr-12,
  Saiga-12, VSK-94, Remington 700) and the Neostead, whose first built-in bonus was +10 %
  silver (modification 1230); and the MP7 special (owner: it joins them; its one bonus, 2004,
  goes). Not the VSSK camo (not premium); the premium armour has the same treatment since
  2026-10-22 ([premium_armour.md](premium_armour.md)).
- **Plain weapon of each:** the same weapon for silver (`*_premium*` → its stem); the
  Neostead's is `neostead_2`, the MP7 special's the silenced MP7.
- **What they keep:** the silver bonus only. Their other built-in bonuses (reload, reliability,
  accuracy and the like) go.
- **The silver bonus is a modification of the weapon's own, its first place**, not a built-in
  one: it counts for the rank (a new golden weapon is rank 1), and the weapon takes two more.
- **Upgradable as the plain weapon:** the same craft and reroll rules, places, chances and
  prices (same item level).
- **Rolls like any modification, the silver bonus too** (owner: "modifications on premium
  weapons can be rolled too (even silver)"): a reroll may replace it. A failed craft no longer
  takes it (owner 2026-10-23: an upgrade never takes a modification away, `craft-menu.md`). Once
  gone it does not come back by itself (it never rolls: drop weight 0).
- **Craft donors** (owner: "you should be able to use simple AK-12's and non-premium weapons
  for crafting"):
  - golden weapon + its plain weapon as donor: yes, it stays golden;
  - golden + golden of the same plain weapon (`akmn_premium` + `akmn_premium_2`): yes;
  - plain weapon + a golden donor: yes (owner 2026-10-22: "if I press upgrade on lancaster, I
    should get inside premium lancasters too"); the golden one is used up, the weapon stays plain.
    The craft window lists every copy of the same plain weapon.
- **Gold background:** golden weapons keep it in the shop and the inventory (the lobby knows
  them by its list, not by the item's rarity, which is 0 now).
- **Gold name** (owner 2026-10-22): a golden weapon's name is gold (`Colors.GOLD`) in the shop,
  the inventory and the item card, whatever its rank. A shop entry keeps rank 0 although it shows
  the silver bonus: the shop's buy options are off for an item with a rank.
- **Weapons owned before:** at the first start of the srp with this rule each golden weapon gets
  the silver bonus as its first modification, once (`g26_meta` `golden_silver_given`, written
  only after the changed accounts are; an account not written stops srp, and the next start
  gives it to the golden weapons still without it). One that already had three of its own keeps
  them (logged; none can have: the lobby showed rank 3).
- **New ones** (shop, bundles, the admin) come with it.
- **Data:** the patch takes every default modification off the 22 configs and sets
  `base_rarity_level` 3 -> 0 (the lobby's rank is `min(3, base + modifications)`: 3 forbade
  every craft and a default is never offered for a reroll). srp refuses to start on data
  without it (`golden::check`), and so does `release/build-server-db` (`srp-check-game-data`).
- **Lobby:** the craft result showed the donor's entry (the exe names it); it shows the item
  upgraded.
- **Shop and bundles show the silver bonus** (owner: "gold sold weapons in shop no longer show that
  they have silver bonus"): a golden weapon not owned (a shop entry, a bundle's contents) shows
  1230 in its first place, as srp gives it on purchase (`ItemFactory.startingModifiers`; a test
  holds its icon and prop to the data). Owned ones show the server's modifications.
- **Release:** a resources.db patch (the content edit; the lobby movie), the server data built
  with it, the server, the `next` row of `simulation_changes.tsv` (the bonuses change matches).
  Not tested in the game yet. A rollback past it goes without `--keep-db` (the granted silver
  would add to an old default).
- **Tech debt** ([tech-debt.md](tech-debt.md)): the silver bonus is a granted modification, not a config
  default, so that it can be rerolled and takes a place. That needs special cases: srp grants it on
  creation (`golden::starting_modifications`, `give_golden_silver` for owned ones), the lobby fakes it on
  shop entries and bundle contents (`ItemFactory.startingModifiers`, `GangShop.setupShopItems`,
  `InventoryList.setupBundleData`), and `golden::check` guards the data. A config default would need none
  of these, but could not be rerolled and would add a 4th bonus.
