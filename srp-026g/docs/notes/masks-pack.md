# Ported masks: the Army balaclava and the seven 0.69d masks (one content patch)

The owner's rules for ported headwear and the items of the masks pack. The port
code is resource-porter `feat/army-balaclava-stk2` (`src/army_026.rs`,
`src/face_items_069d.rs`, `src/mask_fit.rs`); the packs are `vostok-port pack`
packs. srp needs no code for them: it reads each item's faction, level, slots
and drop weight from its config in the server game data.

## Owner's rules (headwear ports)

- Only the worn item, as a mask-slot item over the **native** head: never the
  donor's face or eye skin, no `remove_head_model` (2026-10-15, 2026-10-21).
- Fit (`mask_fit`): a rigid move of the eye holes onto the native eye bones
  (never move only the head-weighted part), a radial push-out with 4 mm
  clearance; the flap under the jaw tucked onto the native neck, the hem at the
  native neck bottom. Check against all 20 stock `torso_s1` models, and from
  behind too.
- The cloth over the skull goes under every helmet worn with a mask.
- New content ships as a db-growing client patch plus a server version, never
  the full db on the live host.
- Equipment work had been paused since 2026-10-17; the owner lifted it for
  these items on 2026-10-21 ("I want the next patch to drop a lot of them at
  once").

## The items

| id | item | from | faction | level | armour / iso | kg | price | drop | icon |
|---|---|---|---|---|---|---|---|---|---|
| 1823 | Army balaclava | STK2 `bandit_01_head_02` | 3 Renaissance Army | 8 | 30 / 15 | 1.0 | 10,000 | 80 | [20, 6] |
| 1812 | Keffiyeh | 0.69d 2092 `edge_balaclava` | 4 Fringe Settlers | 3 | 15 / 10 | 0.3 | 1,300 | 130 | [10, 6] |
| 1813 | 3-Hole Balaclava | 0.69d 2094 `scavenger_balaclava` | 1 Scavengers | 3 | 12 / 12 | 0.3 | 1,300 | 130 | [11, 6] |
| 1814 | Balaclava | 0.69d 2093 `renesanse_balaclava` | 3 Renaissance Army | 3 | 18 / 6 | 0.3 | 1,300 | 130 | [12, 6] |
| 1815 | Buff | 0.69d 2095 `bm_balaclava` | 2 Black Market | 3 | 20 / 5 | 0.2 | 1,300 | 130 | [13, 6] |
| 1816 | Protective mask | 0.69d 948 `bm_mask_9` | 2 Black Market | 5 | 35 / 10 | 0.8 | 5,200 | 110 | [15, 6] |
| 1817 | Snow-20 Thermal vision | 0.69d 2062 `snow_20_mask_7` | 3 Renaissance Army | 4 | 25 / 5 | 0.6 | 3,300 | 120 | [16, 6] |
| 1818 | Delta 2 mask | 0.69d 1913 `league_shadow_mask_premium` | none, premium | 5 | 35 / 5 | 0.6 | 500 gold | 0 | [17, 6] |

- Every number above is a proposal; balance is the owner's call.
- Where they are sold follows srp's rule (`prices::seller_level`): Scavengers
  and Black Market at reputation = item level (at most their top rank),
  Renaissance and Fringe at item level - 5 (at least 0). So: 1823 Renaissance
  rank 3; 1812 Fringe rank 0; 1813 Scavengers rank 3; 1814 Renaissance rank 0;
  1815 Black Market rank 3; 1816 Black Market rank 5; 1817 Renaissance rank 0.
  1818 is premium: seller 0, 500 gold, at no rank (`premium.md`, "Premium items
  one by one").
- 1816, 1817 and 1818 also take the helmet slot (`additional_model_slots`):
  their goggles, monocular and night vision mount are rigid and bent when
  pulled under a helmet (the Snow-20's hood was pulled in by 7 cm). Wearing
  one takes the helmet off (srp's covered-slot rule, as the stock helmets
  with a built-in mask take the mask off). srp reads it: covered slot 0.
- 1823 and 1814 are both Renaissance balaclavas with one eye opening (STK2's
  and 0.69d's); the names keep them apart ("Army balaclava", "Balaclava").

## Open for the owner

1. The level scale of the 0.69d masks (0.69d 1-5 vs 0.26g 1-15): kept, the
   protection scaled down to native masks of the same level.
2. Fringe and Renaissance sell only levels 6-15: their low-level masks sell at
   rank 0 under the rule.
3. The Snow-20's seller (proposed Renaissance); its "thermal vision" has no
   effect in 0.69d either (a mask with a lit diode).
4. The Delta 2: 500 gold premium, or silver with a faction.
5. The item descriptions (0.69d has none; written new).
6. 1816, 1817 and 1818 taking the helmet slot (the other way: worn with
   helmets, their rigid parts bent or through the helmet).
7. The drop weights (1823: 80, as the Renaissance level 8 mask; the Bandit
   balaclava has the Black Market mask's 110).

## Fit checks (offline, resource-porter builders)

- Every mask: the fit, then the cloth's folds smoothed out (`mask_fit::unfold`),
  then the relief over the game's poses (19 lobby and match clips x 24 poses
  and the bind pose; through the head's vertices, edge and triangle middles),
  then the gate: at least 0.2 mm of cloth over the head in every pose. All
  eight pass (least gaps 0.53 to 2.7 mm).
- The Army balaclava is fitted on the eye holes of the STK2 head it shares a
  rig with (head_01's two-hole balaclava, which is the Bandit balaclava's
  donor); its hem sits at the native neck bottom; against the 20 stock
  `torso_s1` models it shows over the collars about as the Bandit
  balaclava does (more at the throat of the leather vest, black_market_01),
  and nothing from behind.
- The Bandit balaclava's build is unchanged (its bundle byte for byte); the
  patch does not carry its files.

## Release

- One content patch over the released 12..26: the packs `army-balaclava` and
  `face-069d`, by `vostok-port pack patch` (new files, the dictionary rows, the
  six localizations' strings, the icons in the lobby movie's atlas).
- **The lobby movie.** The icons are pixels of the movie's equipment atlas
  (`LobbyMenu_EquipmentIcons`, a DefineBitsLossless2 tag). The movie built from
  source (`build_lobby_026g.py`) takes revision 24's graphics and replaces only
  the code, so it has none of these icons. A release with a rebuilt movie
  passes that movie to `pack patch` as the base
  (`--replace flash_movies/lobby_menu.swf=<built movie>`): the packs' atlas
  edit applies over it and refuses a cell that is taken. Without a rebuilt
  movie the base is the movie players have (patch 26's).
- The server version (`srp-game-data build`) is built with the patch; srp
  reads the new configs from it (`tables::check` passes). Every content
  release raises the revision.
- With release/build-content (the content manifest, in progress), the packs
  are two `pack` rows at the end of their kind:
  - `army-balaclava@1` `bc6266a33d24a351ca9679d5f1700083850909c07dfc092e929c480f92d99b7f`
  - `face-069d@1` `a2c2c0908525897939ed6e5fd4c69ca0a6807cbfe19520a496ad3aa14297907c`
