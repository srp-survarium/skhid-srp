# Golden weapons

The owner's rules of 2026-10-21. Code: `survarium::v026g::golden`,
`session::v026g::items_rules` (`craft`, `give_golden_silver`), `Account::add_item`; the
data: `scripts/content-edit/golden-weapons-1.json`; the lobby: `LobbyMenu.fill_crafted_item`.

- **The rule:** "remove extra bonuses from all golden weapons and make them upgradable just
  like any other weapon but with the first golden bonus already set"; "remove modifiers except
  for silver from all premium weapons".
- **Golden weapons:** the 21 weapons whose first default bonus was +10 % silver (modification
  1230): the 20 `wpn_*_premium` / `*_premium_2` copies (Kiparis, Uzi ×2, Vityaz, VSS, SR-3M,
  Lancaster ×2, AK-74N, AK-74U, AK-12, AKMN ×2, SKS ×2, Vepr, Vepr-12, Saiga-12, VSK-94,
  Remington 700) and the Neostead. Not the MP7 special (gold, one bonus, no silver), the VSSK
  camo (not premium) or the premium armour.
- **What they keep:** the silver bonus only. Their other built-in bonuses (reload, reliability,
  accuracy and the like) go.
- **The silver bonus is a modification of the weapon's own, its first place**, not a built-in
  one: it counts for the rank (a new golden weapon is rank 1), and the weapon takes two more.
- **Upgradable as the plain weapon:** the same craft and reroll rules, places, chances and
  prices (same item level).
- **Rolls like any modification, the silver bonus too** (the owner: "modifications on premium
  weapons can be rolled too (even silver)"): a reroll may replace it, a failed craft may take
  it. Once gone it does not come back by itself (it never rolls: drop weight 0).
- **Craft with the plain weapon** (the owner: "you should be able to use simple AK-12's and
  non-premium weapons for crafting"): a golden weapon and its plain weapon count as one weapon;
  either is the other's donor, `*_premium` and `*_premium_2` too. The item upgraded stays what
  it is (the golden weapon keeps its look and silver bonus). The Neostead has no plain weapon.
- **Weapons owned before:** at the first start of the srp with this rule each golden weapon gets
  the silver bonus as its first modification, once (`g26_meta` `golden_silver_given`, written
  only after the changed accounts are; a failed write or a crash leaves it unmarked and the next
  start gives it to the golden weapons still without it). One that
  already had three of its own keeps them (logged; none can have: the lobby showed rank 3).
- **New ones** (shop, bundles, the admin) come with it.
- **Data:** the patch takes every default modification off the 21 configs and sets
  `base_rarity_level` 3 -> 0 (the lobby's rank is `min(3, base + modifications)`: 3 forbade
  every craft and a default is never offered for a reroll). srp refuses to start on data
  without it (`golden::check`).
- **Lobby:** the craft result showed the donor's entry (the exe names it); it shows the item
  upgraded.
- **Release:** a resources.db patch (the content edit; the movie), the server data built with
  it, the server, the `next` row of `simulation_changes.tsv` (the bonuses change matches). Not
  tested in the game yet.
