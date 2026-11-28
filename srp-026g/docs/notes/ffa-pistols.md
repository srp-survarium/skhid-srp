# Free for all: Pistols, everyone plays the same two pistol kits

The owner, 2026-11-27: "let's implement FFA pistols, it is just like ffa: roulette, but everyone
gets 2 slots instead of 3 and 2 pistols of the same level band, also kashtan is not a pistol";
"it should be 10% of FFA cards"; "take 5% from roulette and 5% from FFA normal"; "full mode requires
icon too, wiki updated".

A twist of the free-for-all cards (`ffa.md`, "Twists"), beside Racket, Saloon and Roulette. Code:
`survarium::v026g::pistols`, built on `roulette`'s pools and draw.

## In short

- The card draws a **level band** among those the room allows, Roulette's 0-5 and 6-10 alone
  (`ffa-roulette.md`, "The band": by `match_making_level`, each band whose top is at most the room's
  max level, each as likely; `pistols::Band`), then **one pair**: two kits of that band, the
  pistols, the armour and the pockets all of it. A level-15 room plays 0-5 or 6-10, half each,
  never 11-15 (the owner, 2026-11-27: "as always 11-15 rooms just get 0-5 or 6-10 equips"); a
  level-10 room the same; a level-5 room 0-5.
- **Everyone plays that same pair.** Kit 1 and kit 2 are the pair's; kit 3 is not loaded
  (`KIT_NOT_LOADED`, empty): the game makes no player object for it, so nobody spawns with it and a
  pick of it changes nothing (`match.md` §23), as an account's unplayable kit today. That is the
  owner's "2 slots instead of 3": a kit is a slot of the warm-up's kit picker.
- **Each kit:** one pistol in weapon 1's slot (weapon 2's empty, as Roulette's one gun a kit), its
  highest ammo for every ammo bag, a full set of armour of the band, medkit, grenade and four more
  pockets; every pistol and armour piece with random bonuses as a dropped item, every pistol random
  attachments that fit it. All of it Roulette's draw (`roulette::BandPool::kit`, `dress`), the same
  code, in the same order as a Roulette pistol hand's kits.
- **The two pistols differ** while the band has two (every band has three or more): the pair is
  drawn without putting back, as a Roulette pistol hand's three are. A band of one pistol would deal
  it to both kits; srp refuses to start on data with a band of none (`pistols::check`).
- **Nothing of the players' is used or charged**, their skills still apply, the pay is a
  free-for-all's with no items' bonuses: as Roulette (`Armed::keeps_own_items`, `autobuy::used`,
  `rewards`).
- **Share:** 10 % of the free-for-all cards: plain 30 -> 25, Roulette 30 -> 25, Racket 30 and the
  Saloons 5 and 5 as they were (the owner's "take 5% from roulette and 5% from FFA normal"). On a
  map played with a twist only (Military Warehouses) 13.3 %. In a room under level 5 (no band)
  Pistols is left out of the draw, as Roulette.
- **Name:** "FFA: Pistols", the band not shown (as Roulette's). Lobby mode number **18**
  (`twist::LobbyMode::Pistols`): 17 is taken by the unmerged Instagib twist (branch
  `feat/ffa-instagib`), so neither has to move when both land.

## The pistols by band

A pistol is a gun of the pistols' dictionary category (17) held in one hand (no `double_handed`),
no golden or premium copy of another (`like`), with an ammunition the shop offers. No pistol is
above level 10: Pistols has no 11-15 band (Roulette's pistol hands of 11-15 deal the 6-10 pistols
with 11-15 armour; Pistols never does).

| band | pistols (live data, revision 74) |
| --- | --- |
| 0-5 | TT-33 (0), PM (2), Colt M1911 (3) |
| 6-10 | Fort 17, APS, CZ 75, MP-443 Grach, Beretta 93R, Mauser M.712 (6), Glock 17 (7), Colt Python (10) |

## The Kashtan

The AEK-919K "Kashtan" (`aek_919k`) is a submachine gun in Skhid's data: its dictionary category is
15 (submachine guns), it is `double_handed`, the shop's filters (by category, `ShopFilters.tsv`),
the wiki's weapons table (`docs/data/weapons.tsv`) and Roulette's main-gun classes all have it as
one. Its config's `parameters.type` was a pistol's, `wpn_pstl`: the site's damage calculator and the
wiki's simulators group the guns by it, so the site showed it under "Pistols & revolvers" (the owner,
2026-11-27: "for some reason on website it is a pistol"). Content edit `kashtan-type-1.json` makes it
`wpn_smg`, as every other gun of category 15 is but the PPSh-41 and the MP7s (`wpn_aslt`). A site
build with it (`release/build-site --with` a patch of the edited config) changes the calculator's
data in that one value: the Kashtan under "Submachine guns". The wiki's weapons table groups by
category and does not change.

- Who reads `type`: the site (above); srp's match server, `bots::reflex::Kind::of_item` for a
  place's weapon 1 (`game::driven::weapon_of`): a player holding the Kashtan is an SMG to the bots
  now, not a pistol (acceptable: it is the gun it is); nothing else of srp. The 0.26g exe has no
  `wpn_*` string, so what the game itself does with `type` is not known (it could hash it): the
  change is in the `next` row of `simulation_changes.tsv`, to be safe.
- Pistols' pool is by category and hands, so the Kashtan was never in it either way; the test
  `each_band_deals_its_own_pistols_never_the_kashtan` holds that, `test_kashtan_type.py` the edit.

## Determinism, late joiners, replays, the world format

As Roulette's: drawn once in the lobby's deal with the card's rolls (`twist::draw`), held by the
card (`twist::Armed::Pistols(Pair)`), armed by the match server into every profile it gives, a late
joiner's too; replays hold the profiles. The world format does not change: the items are the game's,
in slots they take, and a profile with a kit not loaded is one the game already gets from every
account with an unplayable kit. The `next` row of `simulation_changes.tsv` names it (the card's
shares changed, and a match of it arms the kits otherwise).

## Lobby, HUD and site

- **The card:** "FFA: Pistols" (`st_srp_mode_pistols`, the game's own word of its shop filter
  `st_filter_pistol` in each language: Pistols, Пистолеты, Пістолети, Пісталеты, Pistolen,
  Pistolety, Pistole, Pisztolyok, Pistoolit, Pištolji, Pistolas; Croatian "Svatko za sebe:" as its
  other twists), its tag gunmetal blue (`RoomsLook.MODE_COLOUR` 18).
- **The loading screen's icon:** `SrvGameModeIcons` frame 11, `GameModeIconPistols`: two TT-33s
  crossed, the game's own lobby icon of the TT-33 in Racket's steel blue-grey with its dark outline
  (made from `wiki/icons/items/tt_33.webp`: mirrored and turned 35 degrees each way, crossed at the
  grips, recoloured to Racket's palette, a 1.5 px navy outline, scaled to 61 x 56).
- **The wiki's Modes page:** a plaque among the versions (`#pistols`) with its icon, name, share,
  kits and each band's pistols; the plain Free for all got its plaque too, first (`#plain`: its
  icon, name, share and "Everyone plays his own kits"), where it had a bare line before.
- **The stats:** one more version of Free for all on both boards (its own lobby number in the
  history); its name in every language is the game's (`plays.rs` test).

## The owner's answers (2026-11-27)

1. Two kits, a pistol each, kit 3 empty: right.
2. No 11-15 band: "as always 11-15 rooms just get 0-5 or 6-10 equips", the whole pair of the drawn
   band.
3. The two pistols always different: keep.
4. The Kashtan's `type`: fix it ("for some reason on website it is a pistol"): `kashtan-type-1.json`.
