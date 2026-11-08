# Decals and the Steam camouflage from 0.69d (pack decals-069d)

The owner's pick of 2026-10-22 from the 0.69d gallery
(`069d-vs-026-items/camo-decal-gallery.html`): 38 decals and the Steam camouflage
("no need for ILS"), version 1; and ten more decals picked 2026-11-08, version 2 (seven of 0.69d's
premium decals and three of its achievement awards). Made by `scripts/decals-069d/make_pack.py`
from `items.tsv` (the list, ids, kinds, icon cells), `texts.tsv` (names) and `donor.sha256`
(0.69d's files); the pack is `decals-069d@2` of `release/content.tsv` (version 1's 39 items as
they were, byte for byte, and the ten). srp needs no code: it reads each item's kind, price and
drop weight from the dictionary and the config; each item has its row in
`content-edit/items.tsv` (name, price, premium).

## Rules

- **Items.** Category 26 decals and one category 25 camouflage, each a 0.26g config made from a
  native one of its kind (only the modification id, texture, name key and icon cell differ):
  flags and holiday decals from `char_decal_4_41` (`prm_decl`, level 4, drop 0), Jarilo from
  `char_decal_3_4` (`drp_decl`, level 3, drop 160), the badges Musician, Boar hunter and Deer
  hunter (0.69d's `unl_decl` achievement awards) from 0.26g's badge `char_decal_1_1` (`unl_decl`,
  level 1, drop 0), Steam camo from `camo_new_year` (as camos-069).
- **Names kept from 0.69d**: config `char_decal_<r>_<c>`, texture `character/human/decal/<r>_<c>`,
  dictionary name `decals_<r>_<c>`, modification `200000 + 100 r + c`, name key
  `st_character_decals_<r>_<c>_name`; the description is 0.26g's generic one. None is taken in
  0.26g or the released patches (checked: dictionary, modification ids of every item config).
- **Ids**: 1824 to 1862, the next free after 1823 (1817, 1818 stay reserved for the unshipped
  masks); version 2's 1884 to 1893, the next free after 1883 on main and in the released patches
  (`content-edit/items.tsv`, a row per dictionary entry; 1877 stays the parked mountain rucksack's,
  1901 on are the scope fits). 0.69d's own ids (762 to 2195) clash with the scopes (2001 to 2468).
- **Textures**: 0.69d's DDS as they are (the formats of 0.26g's decals: DXT5 diffuse and normal
  map, DXT1 fmap and smap; 0.26g's fmaps are 512² or 16²), each with 0.26g's `.options` of the same role (0.69d's
  hold the same values in its newer format). Boar hunter and Deer hunter (1_10, 1_11) have a 128²
  normal map and 256² fmap and smap in 0.69d, kept so (every 0.26g normal map is 512²; the bake
  samples each layer on its own).
- **Where they go**: as any decal: two on a vest or trousers, one on a weapon (#273); Steam camo
  on any vest, trousers and weapon.
- **Texts**: 0.69d's in the six languages. Fixed: English "Clover оrnament" (a Cyrillic o),
  "Flag Of …" → "Flag of …", Polish "Flaga Nigeri" → "Flaga Nigerii".
- **Icons**: 0.69d's, scaled to the decal atlas's 82x82 cells, in cells empty in revision 27's
  movie (the atlas keeps its 3444x410 size): the flags in row 2, columns 9 to 42; the holiday
  decals [1, 9..11]; Jarilo [3, 22]. Version 2, in cells empty in the live movie (patch 51's): the
  seven premium decals [1, 12..18] beside the holiday decals, the three badges [3, 23..25] beside
  Jarilo. Musician, Boar hunter and Deer hunter are not square in 0.69d (82x77, 62x80, 66x80):
  centred on a transparent square first. Free after them: [1, 19..42], [3, 26..42], [4, 42].
  Steam camo: [2, 7] of both camouflage atlases (72x72 and
  140x140, its 0.69d icons). The big camouflage atlas is a JPEG; the porter now writes it back
  lossless: the lobby movie grows by about 2.5 MB (40.3 to 42.9 MB).

## The list and the prices (proposed; the owner's call)

Flags sell like 0.26g's 77 flags and coats of arms: premium, 300 gold, seller 0 (the gold list,
`shop_offers::premium_price_list`), never dropped. srp sells no decal for silver but the badges
(`prices::is_badge`: `unl_decl`, not premium; since 2026-11-07), so another silver decal can only
drop.

| id | item | 0.69d | kind | price | drop | icon |
|---|---|---|---|---|---|---|
| 1824 | Herbs | 2041 `char_decal_7_9` | holiday (Summer 2019) | 300 gold | 0 | [1, 9] |
| 1825 | Clover ornament | 2157 `char_decal_7_27` | holiday (St Patrick's Day 2021) | 300 gold | 0 | [1, 10] |
| 1826 | Horseshoe | 2158 `char_decal_7_28` | holiday (St Patrick's Day 2021) | 300 gold | 0 | [1, 11] |
| 1827 to 1839 | Flags: USA, Canada, Brazil, Argentina, Australia, Finland, New Zealand, Ireland, Scotland, England, South Korea, Japan, China | 896 to 964 `char_decal_4_42..54` | flag | 300 gold | 0 | [2, 9..21] |
| 1840 | Jarilo | 969 `char_decal_1_12` | drop (0.69d: an achievement award) | 1,000 silver (sells back 250) | 160 | [3, 22] |
| 1841 to 1846 | Flags: India, Indonesia, Malaysia, Philippines, Thailand, Vietnam | 1345 to 1350 `4_55..60` | flag | 300 gold | 0 | [2, 22..27] |
| 1847 to 1854 | Flags: Bolivia, Chile, Colombia, Ecuador, Paraguay, Peru, Uruguay, Venezuela | 1930 to 1937 `4_61..68` | flag | 300 gold | 0 | [2, 28..35] |
| 1855 to 1859 | Flags: Albania, Cuba, Slovenia, Angola, Nigeria | 2064 to 2068 `4_69..73` | flag | 300 gold | 0 | [2, 36..40] |
| 1860 | Flag of Wales | 2070 `char_decal_4_75` | flag | 300 gold | 0 | [2, 41] |
| 1861 | Flag of South Africa | 2069 `char_decal_4_74` | flag | 300 gold | 0 | [2, 42] |
| 1862 | Steam camo | 840 `camo_steam` | premium camouflage (0.69d: the Steam release's, free) | 300 gold | 0 | [2, 7] |
| 1884 | Matchsticks | 2042 `char_decal_7_10` | holiday (0.69d premium) | 300 gold | 0 | [1, 12] |
| 1885 | Heart | 2114 `char_decal_7_23` | holiday (0.69d premium) | 300 gold | 0 | [1, 13] |
| 1886 | Frightening Pumpkin | 2115 `char_decal_7_24` | holiday (0.69d premium) | 300 gold | 0 | [1, 14] |
| 1887 | Homing pigeon | 2162 `char_decal_7_30` | holiday (0.69d premium) | 300 gold | 0 | [1, 15] |
| 1888 | Victory | 2163 `char_decal_7_31` | holiday (0.69d premium) | 300 gold | 0 | [1, 16] |
| 1889 | Bloody Hand | 2193 `char_decal_7_35` | holiday (0.69d premium) | 300 gold | 0 | [1, 17] |
| 1890 | The Keeper | 2195 `char_decal_7_37` | holiday (0.69d premium) | 300 gold | 0 | [1, 18] |
| 1891 | Musician | 762 `char_decal_1_9` | badge (0.69d: an achievement award) | 50,000 silver, seller 0, no rank (sells back 12,500) | 0 | [3, 23] |
| 1892 | Boar hunter | 903 `char_decal_1_10` | badge (0.69d: an achievement award) | 50,000 silver, seller 0, no rank (sells back 12,500) | 0 | [3, 24] |
| 1893 | Deer hunter | 904 `char_decal_1_11` | badge (0.69d: an achievement award) | 50,000 silver, seller 0, no rank (sells back 12,500) | 0 | [3, 25] |

- 0.69d's prices: flags 50 gold, Herbs 10,000 silver, Clover ornament and Horseshoe 100,000
  silver, Jarilo 1,000 silver, Steam camo free; version 2's: Matchsticks 10,000 silver, the other
  six premium decals 100,000 silver (`prm_decl`), Musician, Boar hunter and Deer hunter 1,000
  silver (`unl_decl`, achievement awards).
- Version 2 (the owner, 2026-11-08): the seven `prm_decl` decals are holiday decals as Herbs,
  Clover ornament and Horseshoe; Musician, Boar hunter and Deer hunter are badges, sold for 50,000
  silver: `unl_decl` and not premium, so `prices::is_badge` sells them as gear for their
  `base_cost`, by seller 0 at no rank (no faction, no ShopRules row), and with drop weight 0
  nothing drops them (`drops::droppable`). "300 gold" is main's today; "Silver only" (#1523, open) turns every
  premium price into its gold price x 100 in silver (30,000) by its rows of `content-edit/items.tsv`.
- Event rewards in 0.69d: Herbs (Summer 2019), Clover ornament and Horseshoe (St Patrick's Day
  2021), Jarilo (an achievement, likely), Steam camo (a promotion). Here all are sold or dropped.
- Jarilo: the gallery's proposal was 1,000 silver from seller 0; selling a decal for silver needed
  an srp change then (2026-10-22), so it drops instead, as 0.26g's 21 drop decals (1,000 silver,
  weight 160).

## Open for the owner

1. The prices above (300 gold each; Jarilo dropped, or sold for silver with an srp change).
2. Steam camo is also in the unmerged `feat/camos-069` pack (with Christmas Raid 2016, id 840,
   no icons). Shipping both would add `camo_steam` twice (the composition stops); that pack
   should drop Steam camo or be closed.
3. The release PC's `vostok-port` (`private/release-tools`) is resource-porter
   `feat/decals-069d` 4beb92f since 2026-10-22 (it draws the decal atlas); that branch is not in
   resource-porter's `v0.10b` yet.

## Checked (offline, no game)

- `make_pack.py` twice gives the same pack; `vostok-port pack check` over the stock db and
  patches 12 to 27.
- `release/build-content 28` (with the porter of `feat/decals-069d`): `resources.db.patch_28` of
  420 files (the pack's 349 new files, the dictionary, the six localizations, the lobby movie, and
  main's other unreleased changes); the server game data built from it and checked; the icons
  read back from the movie in their cells.
- Renders of six decals on a vest (15 cm) and on the AK-74N (6 cm) with the porter's
  `scripts/render-decal-026-preview.py` (a box projection, not the game's bake).
- Version 2 (2026-11-08): `make_pack.py` twice gives the same pack (439 files, 50 icons); version
  1's files and icons in it byte for byte; `vostok-port pack check` over the stock db and the base
  rows' patches (12 to 24): ok. A quick `release/build-content 99` (a throwaway revision, over the
  list up to patch 53): 103 files differ from what players have, all of them this version's: its
  90 new files (10 item configs, 80 textures), the dictionary, the 11 localizations (the ten
  languages' catalog and `original`) and the lobby movie, where only the decal atlas changed and
  only in its ten new cells (the icons read back). The dictionary differs from the live one only
  in the rows 1884-1893 (new, as listed) and 221 Literator's price (50,000, the same PR's
  `items.tsv`, `badges-shop.md`); no id twice. The names in the five languages 0.69d lacks (Serbian, Czech, Hungarian, Finnish,
  Belarusian) are ours, in `client-patches/localization/`. The badges on that build's server game
  data: `drops::tests::the_069d_badges_are_sold_by_seller_0_and_never_dropped` (ignored; run with
  `SRP_GAME_DATA`): `prices::price_list` lists each once, 50,000 by seller 0 at level 0, and no
  drop table's pool at any level has them.
- Not seen in a game.
