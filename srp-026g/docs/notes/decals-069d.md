# Decals and the Steam camouflage from 0.69d (pack decals-069d)

The owner's pick of 2026-10-22 from the 0.69d gallery
(`069d-vs-026-items/camo-decal-gallery.html`): 38 decals and the Steam camouflage
("no need for ILS"). Made by `scripts/decals-069d/make_pack.py` from `items.tsv` (the list, ids,
kinds, icon cells), `texts.tsv` (names) and `donor.sha256` (0.69d's files); the pack is
`decals-069d@1` of `release/content.tsv`. srp needs no code: it reads each item's kind, price and
drop weight from the dictionary and the config.

## Rules

- **Items.** Category 26 decals and one category 25 camouflage, each a 0.26g config made from a
  native one of its kind (only the modification id, texture, name key and icon cell differ):
  flags and holiday decals from `char_decal_4_41` (`prm_decl`, level 4, drop 0), Jarilo from
  `char_decal_3_4` (`drp_decl`, level 3, drop 160), Steam camo from `camo_new_year` (as camos-069).
- **Names kept from 0.69d**: config `char_decal_<r>_<c>`, texture `character/human/decal/<r>_<c>`,
  dictionary name `decals_<r>_<c>`, modification `200000 + 100 r + c`, name key
  `st_character_decals_<r>_<c>_name`; the description is 0.26g's generic one. None is taken in
  0.26g or the released patches (checked: dictionary, modification ids of every item config).
- **Ids**: 1824 to 1862, the next free after 1823 (1817, 1818 stay reserved for the unshipped
  masks). 0.69d's own ids (896 to 2158) clash with the scopes (2001 to 2468).
- **Textures**: 0.69d's DDS as they are (the formats of 0.26g's decals: DXT5 diffuse and normal
  map, DXT1 fmap and smap; 0.26g's fmaps are 512² or 16²), each with 0.26g's `.options` of the same role (0.69d's
  hold the same values in its newer format).
- **Where they go**: as any decal: two on a vest or trousers, one on a weapon (#273); Steam camo
  on any vest, trousers and weapon.
- **Texts**: 0.69d's in the six languages. Fixed: English "Clover оrnament" (a Cyrillic o),
  "Flag Of …" → "Flag of …", Polish "Flaga Nigeri" → "Flaga Nigerii".
- **Icons**: 0.69d's, scaled to the decal atlas's 82x82 cells, in cells empty in revision 27's
  movie (the atlas keeps its 3444x410 size): the flags in row 2, columns 9 to 42; the holiday
  decals [1, 9..11]; Jarilo [3, 22]. Steam camo: [2, 7] of both camouflage atlases (72x72 and
  140x140, its 0.69d icons). The big camouflage atlas is a JPEG; the porter now writes it back
  lossless: the lobby movie grows by about 2.5 MB (40.3 to 42.9 MB).

## The list and the prices (proposed; the owner's call)

Flags sell like 0.26g's 77 flags and coats of arms: premium, 300 gold, seller 0 (the gold list,
`shop_offers::premium_price_list`), never dropped. srp never sells a decal for silver (`prices.rs`:
"decals ... not listed"), so a silver decal can only drop.

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

- 0.69d's prices: flags 50 gold, Herbs 10,000 silver, Clover ornament and Horseshoe 100,000
  silver, Jarilo 1,000 silver, Steam camo free.
- Event rewards in 0.69d: Herbs (Summer 2019), Clover ornament and Horseshoe (St Patrick's Day
  2021), Jarilo (an achievement, likely), Steam camo (a promotion). Here all are sold or dropped.
- Jarilo: the gallery's proposal was 1,000 silver from seller 0; selling a decal for silver needs
  an srp change, so it drops instead, as 0.26g's 21 drop decals (1,000 silver, weight 160).

## Open for the owner

1. The prices above (300 gold each; Jarilo dropped, or sold for silver with an srp change).
2. Steam camo is also in the unmerged `feat/camos-069` pack (with Christmas Raid 2016, id 840,
   no icons). Shipping both would add `camo_steam` twice (the composition stops); that pack
   should drop Steam camo or be closed.
3. The release PC's `vostok-port` (`private/release-tools`) is resource-porter 090a322, which
   draws only the equipment atlas: build-content stops on this pack ("82x82, expected 171x83")
   until it is rebuilt from resource-porter `feat/decals-069d`.

## Checked (offline, no game)

- `make_pack.py` twice gives the same pack; `vostok-port pack check` over the stock db and
  patches 12 to 27.
- `release/build-content 28` (with the porter of `feat/decals-069d`): `resources.db.patch_28` of
  420 files (the pack's 349 new files, the dictionary, the six localizations, the lobby movie, and
  main's other unreleased changes); the server game data built from it and checked; the icons
  read back from the movie in their cells.
- Renders of six decals on a vest (15 cm) and on the AK-74N (6 cm) with the porter's
  `scripts/render-decal-026-preview.py` (a box projection, not the game's bake).
- Not seen in a game.
