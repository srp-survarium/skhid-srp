# The Flag of Serbia decal (pack serbia-flag)

The stock decal "Flag Of Serbia" (`char_decal_4_34`, a premium flag) shows blue over white over red:
the flag of Yugoslavia (later Serbia and Montenegro). Serbia's flag is red over blue over white.
Players from Serbia took offence (2026-10-29). 0.69d had already fixed it: its `4_34` is Serbia's
state flag (red, blue, white, the coat of arms) on the same worn frame. The pack takes 0.69d's.
The coat of arms decal (`char_decal_5_36`) is correct and stays as it is.

- **Textures** `character/human/decal/4_34` diffuse, `_nmap`, `_smap`: 0.69d's files as they are
  (resources.04.db; the formats of 0.26g's). The fmap is the same file in both. 0.26g's `.options`
  stay (0.69d's carry the same values in its newer format).
- **Lobby icon** `LobbyMenu_DecalsIcons` [4, 34]: 0.69d's (resources.43.db, 128x128) scaled to 82x82
  as decals-069d's icons, drawn over the stock icon in its own cell (the pack names the stock icon's
  pixels; the porter refuses another). The item's config, name, price and id do not change; players
  who own it keep it.
- Made by `scripts/serbia-flag/make_pack.py` from 0.69d's files; the pack is `serbia-flag@1` of
  `release/content.tsv`. srp needs no code change.

## Checked (offline, no game)

- `vostok-port pack check` over the stock db and patches 12 to 37: ok; with a wrong
  `replaces_sha256` it fails on the cell.
- `make_pack.py` twice gives the same pack.
- `release/build-content 38`: only the lobby movie and the three textures differ from what players
  have; the icon read back from the built movie.
