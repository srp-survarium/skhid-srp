# The Kosovo decals (pack kosovo-decals)

The owner, 2026-11-19: Kosovo's flag and coat of arms, from the official files, in 0.69's style.
Neither 0.26g nor 0.69d has them (0.69d's flags end at 4_75 Wales, its coats of arms at 5_42 Bosnia
and Herzegovina). Made by `scripts/kosovo-decals/make_pack.py` (the textures by `textures.py`); the
pack is `kosovo-decals@1` of `release/content.tsv`. srp needs no code change.

## The items

| id | item | config, texture | icon | price |
|---|---|---|---|---|
| 1894 | Flag of Kosovo | `char_decal_4_76`, `character/human/decal/4_76` | [4, 42] | 30,000 silver, premium, no drop |
| 1895 | Coat of arms of Kosovo | `char_decal_5_43`, `character/human/decal/5_43` | [3, 26] | 30,000 silver, premium, no drop |

- Sold as every flag and coat of arms (their `content-edit/items.tsv` rows); placed as any decal.
- Configs: 0.26g's flag `char_decal_4_41` (as decals-069d's flags), with the modification id
  (200476, 200543), texture, name key and icon cell changed.
- Names: the next free after 0.69d's (none in 0.26g, 0.69d or the released patches); ids the next
  free after 1893. Icons: [4, 42] beside 0.26g's 41 flags, [3, 26] after the badges (row 5, the
  coats of arms, is full).
- Texts: English Flag of Kosovo / Coat of arms of Kosovo, German Flagge des Kosovo / Kosovarisches
  Wappen, Russian (and `original`) Флаг Косово / Герб Косово, Polish Flaga Kosowa / Godło Kosowa,
  Ukrainian Прапор Косова / Герб Косова.

## The source

- The Government of Kosovo's files, linked from the Ministry of Culture's page of the state
  symbols: `flamuri_republikes_se_kosoves.pdf` and `stema_republikes_se_kosoves.pdf` of
  `kryeministri.rks-gov.net/wp-content/uploads/2022/03/` (CorelDRAW vectors of 2008-02-19; sha256 in
  make_pack.py). Colours CMYK 100/80/0/0 blue, 0/20/60/20 gold, white; the flag 2:3.
- Rendered at 300 dpi without antialiasing, each pixel one fill, boxed down to the texture.

## The style (0.69d's, from its own decals)

- **Flag: an embroidered patch**, as all 34 of 0.69d's own flags (4_42 to 4_75). Albania's patch
  (4_69) gives the frame, the merrow border, the field's weave, relief and gloss; its eagle is
  covered by the weave beside it (its detail copied, the patch's broad shading spread over it). The
  threads take 0.69d's colours of the same threads: blue as Chile's canton (4_62), white as Chile's
  star, gold as Vietnam's star (4_60), each dyed to the official colour (in linear light, the
  donor's shading of its official colour plus a haze, kept). The stars and the map are stitched
  across the field's weave, raised as Vietnam's star (its relief fitted), with a dark seam as
  around Chile's star. The design keeps its shapes: scaled by the field's height, centred (the
  patch is wider than 2:3).
- **Coat of arms: a worn painted shield**, as 0.69d's coats of arms 5_1 to 5_42 (0.26g's, which
  0.69d kept). Bosnia's shield (5_42) gives the wear, split per pixel into its paint's dirt and how
  far it is worn to the pale base; its gloss per paint, its chips (alpha), scratches and relief,
  without its own design's edges (stars, diagonal). Carried onto Kosovo's shield column by column,
  outline onto outline; the rim (7 pixels) made on Kosovo's outline with Bosnia's rim by depth. The
  border, stars and map raised as Bosnia's stars.
- Less worn than Bosnia's (the owner, 2026-11-19: "a bit too much tear", "remove that white part at
  the bottom", "and grey one too", "wear at top is fine"): dirt, scrapes, their gloss and relief at
  half strength, no chip under 40 pixels, and none of it in the lower part (faded out from 45 % to
  58 % of the shield's height: Bosnia's big scrape and the chips at its point). The rim stays.
- Formats as the donors': DXT5 diffuse and normal map (x in alpha, y in green), DXT1 specular, 512²,
  8 mips, the donor's header and sRGB byte; the fresnel map the donor's file; 0.26g's 4_41 `.options`.

## Checked (offline, no game)

- `make_pack.py` twice gives the same pack.
- `tools/check build-content`.
