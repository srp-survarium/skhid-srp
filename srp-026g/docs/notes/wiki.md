# Wiki (`/wiki`)

The 0.26g site's tab "Wiki" / "Вікі": a glossary of the game as Skhid plays it. Built by
`srp-026g/scripts/wiki/` (its README), served by `servers/web-server/src/wiki.rs`.

## Owner's rules

- 2026-10-20: "let's build our own wiki in this style" (the old survarium.fandom.com wiki); "it
  will be a part of our own website".
- "we should have it completely data driven (for items), for code/formulas explanations, we should
  just write it down".
- "style is our skhid-like": the site's look (the calculator's and the Maps tab's), its EN/UK
  switch, its short plain text. The fandom wiki gives only what to cover and how it is organised.
- "we only need structure for gameplay features that matter": weapons, ammo, equipment, scopes,
  supplies, modifications, skills, factions as sellers, maps, modes, mechanics. No lore, story,
  patch notes, events, community pages.
- "Essentially glossary for the game, we will then review it together": breadth first; every term a
  player meets gets an entry (the A to Z page).
- Must-haves: weapons, ammo, maps, how batteries work, modes, how healing works, anomalies and how
  they hurt, what antitoxin is for; and for every item what it can roll ("which bonuses can which
  items get when rolling, how big is their dispersion").
- Never stale: rebuilt by `release/build-site` at every release that offers other patches, as the
  Maps tab and the calculator; a web-server test fails a PR whose wiki is built from other patches
  or from other server rules.
- Player-facing text short: one line per rule; no intros, no sources on the pages.

## Generated (nothing typed by hand)

| What | From |
|---|---|
| Items: name, description, icon, level, card lines, stats, protection, effects | the item's config and the dictionary of the released game data; names and texts from the localizations; icons from the lobby atlases (the calculator's build) |
| Who sells an item, at which rank, for how much; what selling and a full repair give | srp: `prices::price_list`, `shop_offers::premium_price_list`, `prices::sell_price`, `prices::repair_cost` (through `session::v026g::wiki::rules`) |
| Which bonuses and defects an item can get, and how likely each step is | srp: `Modifications::candidates` / `defect_candidates` and `modifications::weight` (a step's chance: its weight over the weights of the item's candidates) |
| Which ammo a weapon fires | the dictionary's `items_compatibility` |
| Damage near and far, the falloff distances | the weapon and the ammo configs, by the calculator's formula (`engine.js` `bulletAt`) |
| Scopes and the weapons they fit | srp's scope pairs (`ScopeFits`) |
| Drops, bundles, premium time, exchange rate, skills reset, kits, room limits, a new account's gifts | srp's constants (`pay`, `shop_offers`, `rules`, `account`, `lobby::rooms`, `tutorial`, `loadouts`) |
| Maps, modes, weathers, respawns, batteries, chargers, tickets, timers | `gameplay/game_modes/` and the projects (the Maps tab's `game_maps.playable`) |
| Hazard zones | the projects' `damage_zone` objects with damage (`max_hit` > 0) |
| Factions and ranks, skills, modifications, healing, injuries, fall damage | `factions_dict`, `skills_tree`, `modifications.options`, `craft.options`, `reroll.options`, `human_hit_params`, `default.player` |

## Written down (text/, EN and UK)

Damage, healing, fall damage, spotting, batteries, rooms, kits, upgrades and rerolls, silver and
gold, lobby tips, the A to Z terms. Their numbers are facts of the data or srp (`{{name}}`), never
typed; a rule of code (the 10 s of a spot, the 90 s a lost place is kept) is written as the notes
give it (`spotting.md`, `rooms.md`, `kit-change-before-start.md`, `battery_pickup.md`,
`fall_damage.md`, `exchange.md`, `buy-modified.md`, `craft-menu.md`, `modes_maps.md`).

## Edge cases

- An item nobody can have on Skhid (not sold, not dropped, in no bundle, not a starter) gets no page.
- A description that is the developers' "ОПИСАНИЕ НЕ ГОТОВО" placeholder is left out.
- The artefacts' names come from their pickup hints, as on the Maps tab.
- Mode 3's name is the game's with ": chargers" (the rooms window's).
- Live data only: a change not released yet (e.g. the dynamic anomalies) is not on it.

## Serving

- `GET /wiki` and `/wiki/<page>`: the page in the request's language under the site's header
  (`no-store`); an unknown page is the site's 404. The calculator's Content-Security-Policy.
- `GET`/`HEAD /wiki/wiki.js`, `/wiki/icons/*.webp`: named with `?v=<hash>`, kept a year.
- The 0.21d site has no wiki.
