# Free for all: Pistols, everyone plays the same two pistol kits

The owner, 2026-11-27: "let's implement FFA pistols, it is just like ffa: roulette, but everyone
gets 2 slots instead of 3 and 2 pistols of the same level band, also kashtan is not a pistol";
"it should be 10% of FFA cards"; "take 5% from roulette and 5% from FFA normal"; "full mode requires
icon too, wiki updated".

A twist of the free-for-all cards (`ffa.md`, "Twists"), beside Racket, Saloon and Roulette. Code:
`survarium::v026g::pistols`, built on `roulette`'s pools and draw.

## In short

- The card draws a **level band** among those the room allows, as Roulette's (`ffa-roulette.md`,
  "The band": 0-5, 6-10, 11-15 by `match_making_level`, each band whose top is at most the room's
  max level, each as likely), then **one pair**: two kits of that band.
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
no golden or premium copy of another (`like`), with an ammunition the shop offers. The 11-15 band
has none of its own (no pistol is above level 10), so it deals the 6-10 band's, as a Roulette pistol
hand does; its armour is the 11-15 band's.

| band | pistols (live data, revision 74) |
| --- | --- |
| 0-5 | TT-33 (0), PM (2), Colt M1911 (3) |
| 6-10 | Fort 17, APS, CZ 75, MP-443 Grach, Beretta 93R, Mauser M.712 (6), Glock 17 (7), Colt Python (10) |
| 11-15 | the 6-10 band's eight |

## The Kashtan

The AEK-919K "Kashtan" (`aek_919k`) is a submachine gun in Skhid's data: its dictionary category is
15 (submachine guns), it is `double_handed`, the shop's filters (by category,
`ShopFilters.tsv`), the wiki's weapons table (`docs/data/weapons.tsv`) and Roulette's main-gun
classes all have it as one. The one thing that calls it a pistol is its config's
`parameters.type`, `wpn_pstl` (the PPSh-41 and the MP7 are `wpn_aslt` the same way): the game's
exe has no such string, and srp reads it only for the damage calculator's grouping ("Pistols &
revolvers") and the match server's bots' reflex (`bots::reflex::Kind::of_item`, after the sniper and
shotgun categories). It is left as it is: what the game does with `type` is not known, and the
grouping by category is right everywhere a player meets it. Pistols' pool is by category and hands,
so the Kashtan is never in it; the test `each_band_deals_its_own_pistols_never_the_kashtan` holds it,
and the one-hand rule keeps out any two-handed gun a later data change might file under pistols.

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

## Open questions for the owner

1. "2 slots instead of 3" read as two kits (the warm-up picker's slots), a pistol each; the other
   reading, three kits with a pistol in both weapon slots, is not what this does.
2. The 11-15 band deals the 6-10 pistols with 11-15 armour (Roulette's rule); the alternative is to
   leave 11-15 out of Pistols' draw.
3. The two pistols are always different (every band has three or more).
