# Premium armour

The owner's rule of 2026-10-22: give the premium armour the golden weapons' treatment
([golden_weapons.md](golden_weapons.md)), so that the Modification (reroll) button works on it. Code:
`survarium::v026g::golden` (`PREMIUM_ARMOUR`), `session::v026g::items_rules`
(`give_premium_armour_bonuses`), `Account::add_item`; the data:
`scripts/content-edit/premium-armour-1.json`; the lobby: `ItemDictId.PREMIUM_ARMOUR*`,
`ItemFactory` (`isGolden`, `plainOf`, `startingModifiers`, `BONUS_UI`).

- **The items:** every armour piece with a base rarity in the game's data, the jackets and trousers
  of the Bandit (8) and Survivor (14) bundles. Nothing else gold-sold is armour with premium bonuses:
  `scav_torso_4` and `scav_legs_4` are sold for gold one by one but are the plain pieces (base
  rarity 0, one default, already craftable).
- **Before:** three defaults each (the plain piece's one, then +5 % experience, then +5 %
  reputation with the faction) and base rarity 3: rank 3, no craft, and a default is never offered
  for a reroll.
- **No one bonus matches the golden rule.** The golden weapons kept their one money bonus and lost
  combat bonuses; this armour has two pay bonuses (experience, reputation) and no combat one beyond
  its plain piece's. So **[srp's choice] all bonuses stay** and players lose nothing:
  - the two premium bonuses become modifications of the piece's own, its first two places (rank 2);
  - the plain piece's default (a pocket, an ammo bag, 2 % less stamina spent) stays a config
    default, as on the plain piece (it is part of the piece, never rolls: drop weight 0).
- **Per item** (dict id, name, config under `gameplay/items/armour/`; rarity before -> after):

  | id | item | config | rarity | stays a default | own modifications (new) |
  |---|---|---|---|---|---|
  | 352 | Bandit jacket `bm_torso_4_premium` | `torso/blackmarket_torso_4_premium` | 3 -> 0 | 1030 +1 pocket | 1240 +5 % experience, 1260 +5 % Black Market reputation |
  | 354 | Bandit trousers `bm_legs_4_premium` | `legs/blackmarket_legs_4_premium` | 3 -> 0 | 1050 +1 ammo bag | 1240, 1260 |
  | 345 | Survivor jacket `scav_torso_4_premium` | `torso/scavenger_torso_4_premium` | 3 -> 0 | 1030 +1 pocket | 1240, 1250 +5 % Scavenger reputation |
  | 346 | Survivor trousers `scav_legs_4_premium` | `legs/scavenger_legs_4_premium` | 3 -> 0 | 1160 -2 % stamina spent | 1240, 1250 |

  Plain pieces (the donors): 351 `bm_torso_4`, 353 `bm_legs_4`, 325 `scav_torso_4`, 326
  `scav_legs_4`.
- **The other choice** (the owner may switch to it): keep only +5 % experience (1240, the bonus all
  four share, as the silver one all golden weapons share), the reputation bonus goes; rank 1, two
  craft places. It changes the patch (1250 / 1260 removed, not granted), `PREMIUM_ARMOUR`'s bonuses
  and the lobby's `PREMIUM_ARMOUR_BONUSES`; the one-time grant would then give 1240 only.
- **Upgradable as the plain piece:** same item level, so the same craft and reroll prices and
  chances; the premium bonuses reroll and a failed craft may take one, as any modification; gone,
  it does not come back (drop weight 0).
- **Craft donors:** a premium piece takes its plain piece or another of itself; a plain piece takes
  no premium one (`golden::may_donate`, `ItemFactory.isCraftDonor`). Not another faction's piece.
- **Gold background and gold name**, as the golden weapons (the lobby knows them by its list; the
  descriptor's rarity is 0 now).
- **Shop and bundles** show the two bonuses on a piece not owned (`ItemFactory.startingModifiers`;
  a test holds the icons and props to the data). Owned ones show the server's.
- **Owned before:** at the first start of the srp with this rule each premium piece gets the bonuses
  it lacks (none of that modification type held) as its first modifications, once (`g26_meta`
  `premium_armour_given`, written only after the changed accounts are; an account not written stops
  srp, and the next start gives them again to the pieces still without them). Each account changed is
  logged with its item ids; a piece with no place left keeps its own and is logged (none can have: the
  lobby showed rank 3). Golden weapons are not touched (their grant has its own mark).
- **New ones** (bundles, the admin) come with them.
- **Pay** is unchanged: `Modifications::bonuses` counts own modifications and defaults alike. Not a
  simulation change (no `simulation_changes.tsv` row): the bonuses only change what a match pays.
- **Data:** the patch removes 1240 and 1250 / 1260 from the four configs' `default_modifications`
  and sets `base_rarity_level` 3 -> 0. srp refuses to start on data without it (`golden::check`), and
  so does `release/build-server-db` (`srp-check-game-data`).
- **Release:** a resources.db patch (the content edit, the lobby movie), the server data built with
  it, the server. Not tested in the game yet.
- **Tech debt:** the same special cases as the golden weapons' ([tech-debt.md](tech-debt.md)).
