# Items workbook (`/items.xlsx`)

The owner (2026-10-25): "the table on the website exported with every single characteristic
(including all there is for recoil) for every single item. Completely data driven"; an Excel
workbook of all items, "nothing left unturned, so people could balance the game". Code:
`servers/web-server/src/items_xlsx.rs`.

## Owner's rules

- Every item of the dictionary (`items_dict`), sold or not: a row each, with how it is had.
- Every value of the item's config, by its exact path, so a balancer can find and change it;
  nothing picked by hand. Recoil: everything the config has (`recoil/*`, `dispersion/*`,
  `breath_vibration/*`, `parameters/recoil_modifier`, `recoil_animations`).
- The lobby card's numbers (`ui_desc/props_list`), named as the lobby names them; the shop
  (price, seller, rank, currency), bundles, drops, starting kit.
- Values from other files: the ammo a weapon fires, the scopes that fit, the modifications and
  defects it can roll, its default modifications applied.
- The shared tables as sheets: modifications, modification types (their ranges), defects,
  defect slots, craft and reroll, shop, bundles, compatibility, hit params, body parts,
  factions and ranks, the lobby's properties, match levels, the player (movement, stamina,
  dispersion), effects, drops, skills; every other `gameplay/` file value by value.
- "Hard-coded": what is in the exe or the server's code, not in the data, with its source.
- Numbers are numbers; a header row, frozen panes and filters; a "Columns" sheet.

## How it stays right

- Made by the server from the game data it runs on (`SRP_GAME_DATA`: the stock `resources.db`
  and exactly the patches `/patches` offers), once a process, at start. A release restarts the
  server with its data: the workbook can never be older than the players' game. No file is
  built or committed; no `built-from.tsv` (the Maps tab and the calculator need one).
- Names in English and Ukrainian (the site's languages, the texts the server's data has).
- Tests (`items_xlsx::tests`): a row per dictionary item, each on one item sheet; every weapon
  has every key of its `recoil` table with its value; numbers stay numbers; the file opens as a
  workbook with those sheets.

## Serving

- `GET`/`HEAD /items.xlsx` (the 0.26g site only), `Content-Disposition` file name
  `skhid-items-r<revision>.xlsx`. The header's "Items (.xlsx)" link on every page.
- By hand: `cargo run -p web-server --example items-xlsx -- <server-game-data.db> <out.xlsx>`.
