# Badges in the shop

The owner's rules (2026-11-07). Code: `prices::is_badge` and `sellers_of_in`
(`crates/survarium/src/v026g/prices.rs`); data: `scripts/content-edit/items.tsv` (prices),
`scripts/content-edit/shop.tsv` (sellers and ranks).

- **What:** the decals of type `unl_decl`, which stock 0.26g unlocked by rank or faction
  reputation. srp has no unlock, and no drop or shop gave them, so nobody could get one.
- **How:** bought for silver in the shop, like gear: once bought, the player owns the badge.
  Vanity items, so they are priced high.
- **Rank badges** (`decals_1_1`..`1_8`): sold by everyone (seller 0), no rank needed.
  1,000,000 silver each: "Alpha" Squad, Survivor, Explorer, Veteran, Master, Legend.
  Literator (`decals_1_1`): 50,000 (the owner, 2026-11-08). Flying Hussars (`decals_1_8`): 20,000.
  0.69d's achievement decals Musician, Boar hunter and Deer hunter (1891-1893, pack decals-069d@2,
  `decals-069d.md`) are badges too: 50,000 each, seller 0, no rank.
- **Faction badges** (`decals_2_1`..`2_8`): sold by their faction, at the rank they are named
  after (the faction's rank names in the localization):

  | badge | faction | rank | silver |
  |---|---|---|---|
  | Scavengers rookie | Scavengers | 1 Rookie | 50,000 |
  | Scavengers veteran | Scavengers | 5 Veteran | 100,000 |
  | Black Market Recruit | Black Market | 1 Recruit | 50,000 |
  | Black Market Elite | Black Market | 5 Elite | 100,000 |
  | The Renaissance Army Soldier | Renaissance | 1 Soldier | 50,000 |
  | The Renaissance Army Colonel | Renaissance | 10 Colonel | 100,000 |
  | "The Fringe Settlers" Hunter | Edge | 1 Hunter | 50,000 |
  | The Fringe Settlers Magister | Edge | 10 Magister | 100,000 |

- **Selling back:** a quarter of the price, as with any item (`sell_price`). That is the client's
  own rule, and it shows the same amount.
- **Not changed:** the other decals (drop and premium ones), and the six banned flags and
  emblems (`not_sold`).
- **Release:** a server release together with the content patch that carries the new prices
  (`items.tsv` changes `base_cost` in players' dictionary, which the lobby shows). Nothing
  changes in what a match computes.
