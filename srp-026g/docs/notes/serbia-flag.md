# The Flag of Serbia decal (pack serbia-flag)

The stock decal "Flag Of Serbia" (`char_decal_4_34`, a premium flag) shows blue over white over red:
the flag of Yugoslavia (later Serbia and Montenegro). Serbia's flag is red over blue over white.
Players from Serbia took offence (2026-10-29). The coat of arms decal (`char_decal_5_36`) is
correct and stays as it is.

- **Texture** `character/human/decal/4_34.dds`: the stock one with its colours mapped (blue to red,
  white to blue, red to white, the grime kept), in its DXT5 blocks: every mip, the alpha and the
  normal, specular and fmap textures stay the stock ones, and so does the wear.
- **Lobby icon** `LobbyMenu_DecalsIcons` [4, 34]: the new texture at 82x82, drawn over the stock
  icon in its own cell (the pack names the stock icon's pixels; the porter refuses another). The
  item's config, name, price and id do not change; players who own it keep it.
- Made by `scripts/serbia-flag/make_pack.py` from the stock texture; the pack is `serbia-flag@1`
  of `release/content.tsv`. srp needs no code change.

## Checked (offline, no game)

- `vostok-port pack check` over the stock db and patches 12 to 37: ok; with a wrong
  `replaces_sha256` it fails on the cell.
- `make_pack.py` twice gives the same pack.
- The texture and the icon rendered and compared with the stock ones (same placement and wear,
  the stripes red, blue, white).
