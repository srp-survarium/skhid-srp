# The Snufkin decals (pack snufkin-decals)

The owner, 2026-11-23: Snufkin "from the anime" (the 1990 Moomin series), two pictures cut out, as
decals, not sold. Made by `scripts/snufkin-decals/make_pack.py`; the pack is `snufkin-decals@1` of
`release/content.tsv`. srp needs no code change.

## The items

| id | item | config, texture | icon | price |
|---|---|---|---|---|
| 1896 | Snufkin | `char_decal_4_77`, `character/human/decal/4_77` | [1, 19] | none, not sold |
| 1897 | Snufkin sitting | `char_decal_4_78`, `character/human/decal/4_78` | [1, 20] | none, not sold |

- Not sold: `decals_4_77/not_sold` and `decals_4_78/not_sold` in `content-edit/shop.tsv` (neither a
  faction nor the premium list), and `base_cost` 0 (`content-edit/items.tsv`), so selling one back
  pays nothing. Never dropped (`prm_decl`, 4_41's). A player gets one by a give.
- Configs: 0.26g's flag `char_decal_4_41` (as the Kosovo and 0.69d flags), with the modification id
  (200477, 200478), texture, name key and icon cell changed. Names and ids the next free after the
  Kosovo flag (4_76, 1895); icons beside 0.69d's premium decals ([1, 12..18]).
- Texts: English Snufkin / Snufkin sitting; the character's name of each language's books (German
  Schnupferich, Russian and Belarusian Снусмумрик / Снусмумрык, Polish Włóczykij, Ukrainian
  Нюхмумрик, Czech Čenichal, Finnish Nuuskamuikkunen, Hungarian Kószász); Croatian, Serbian and
  Portuguese keep Snufkin.

## The pictures

- Two frames of the series (its only releases are SD: no better source exists), from the Moomin
  fandom wiki: `Snufkin17_0.jpg` (800x601, looking up, feather and scarf) and `Skärmbild (36).png`
  (1444x1080, sitting). Snufkin cut out of each with BiRefNet (rembg's `BiRefNet-general`, run
  with onnxruntime); the cutouts, the frames and the script are in `private/snufkin-decals/`
  (sha256 in make_pack.py). The frames end at his chest and knees: so do the decals.
- Printed on cloth as 0.26g's flags: the picture fitted in 512x512 with 16 pixels round it, its
  alpha's soft fringe narrowed, and the colour of every pixel at its edge and outside it the nearest
  solid colour of the picture (no background colour of the frame, and no white, filters or mips in);
  the normal map 4_41's bare weave (its strip above the flag, every other copy turned), the
  specular 4_41's cloth; its fresnel map and `.options` as they are.

## Checked (offline, no game)

- `make_pack.py` twice gives the same pack.
