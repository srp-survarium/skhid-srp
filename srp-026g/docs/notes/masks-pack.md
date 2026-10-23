# Ported masks: the Army balaclava and five 0.69d masks (one content patch)

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
- Not in this release (owner, 2026-10-21): 1817 Snow-20 Thermal vision and
  1818 Delta 2 mask. Their builders stay in resource-porter
  (`face_items_069d::NOT_SHIPPED`); the pack carries neither, nor their icons
  ([16, 6], [17, 6]) or texts.

## The items

| id | item | from | faction | level | armour / iso | kg | price | drop | icon |
|---|---|---|---|---|---|---|---|---|---|
| 1823 | Army balaclava | STK2 `bandit_01_head_02` | 3 Renaissance Army | **6** | 30 / 5 | 0.8 | 5,230 | 100 | [20, 6] |
| 1812 | Keffiyeh | 0.69d 2092 `edge_balaclava` | 4 Fringe Settlers | **5** | 30 / 5 | 1.0 | 4,460 | 110 | [10, 6] |
| 1813 | 3-Hole Balaclava | 0.69d 2094 `scavenger_balaclava` | 1 Scavengers | **8** | 30 / 5 | 1.0 | 7,840 | 80 | [11, 6] |
| 1814 | Balaclava | 0.69d 2093 `renesanse_balaclava` | 3 Renaissance Army | 3 | 18 / 6 | 0.3 | 1,300 | 130 | [12, 6] |
| 1815 | Buff | 0.69d 2095 `bm_balaclava` | 2 Black Market | 3 | 20 / 5 | 0.2 | 1,300 | 130 | [13, 6] |
| 1816 | Protective mask | 0.69d 948 `bm_mask_9` | 2 Black Market | **10** | 40 / 5 | 1.5 | 13,130 | 70 | [15, 6] |

- The levels in bold are the owner's (2026-10-21). Every other number is a
  proposal; balance is the owner's call.
- How the numbers were proposed: from the native mask of the item's level
  (0.26g has masks of levels 3, 5, 6, 8, 9 and 14 only, none of the sellers'
  own at these levels):
  - 1823 (level 6): the Fringe respirator `edge_resp_1` (10 / 50, 0.8 kg,
    5,230, drop 100), its weight, price and drop; a respirator's protection is
    no cloth's, so armour / isolation are the level 5 Corsair's (`bm_mask_1`,
    30 / 5). Before: 30 / 15, 1.0 kg, 10,000, drop 80 (the Bandit
    balaclava's).
  - 1812 (level 5): the Corsair, the one level 5 mask, whole (30 / 5, 1.0 kg,
    4,460, drop 110). Before: 15 / 10, 0.3 kg, 1,300, drop 130.
  - 1813 (level 8): the Smaragd respirator `edge_resp_3` (20 / 65, 1.0 kg,
    7,840, drop 80), its weight, price and drop; armour / isolation the
    Corsair's (30 / 5). Before: 12 / 12, 0.3 kg, 1,300, drop 130.
  - 1816 (level 10, none native): the level 9 Assault mask `renesanse_mask_2`
    whole (40 / 5, 1.5 kg, 13,130, drop 70). Before: 35 / 10, 0.8 kg,
    5,200, drop 110.
  - 1814, 1815: unchanged (level 3).
- Where they are sold follows srp's rule (`prices::seller_level`): Scavengers
  and Black Market at reputation = item level, at most their top rank (5);
  Renaissance and Fringe at item level - 5, at least 0. So: 1823 Renaissance
  rank 1; 1812 Fringe rank 0; 1813 Scavengers rank 5 (their top: level 8 is
  above it); 1814 Renaissance rank 0; 1815 Black Market rank 3; 1816 Black
  Market rank 5 (their top). Checked with srp on the built server data.
- Fringe and Renaissance gear starts at level 6, so a mask below it (1812 at 5,
  1814 at 3) is sold from their first rank (0): every player who has reached
  the faction can buy it.
- 1816 is just a mask (the owner, 2026-10-23, T28: "the hunter mask should
  just be a mask"; players asked to wear helmets with it). The pack made it
  take the helmet slot too (`additional_model_slots`: its goggles are rigid
  and bent when pulled under a helmet, so it was fitted over the bare head
  only); `scripts/content-edit/protective-mask-1.json` takes `helmet` out of
  that list. It is worn in the mask slot only (its category) and covers
  nothing: a helmet goes on with it, and a helmet with a built-in mask still
  takes it off. Kits that wear it need no repair: it was already in the mask
  slot, with the helmet slot empty.
  - Its model is the pack's, fitted over the bare head: with a helmet on,
    the hood shows through it. In the bind pose 11 of the 12 helmet
    surfaces worn with masks have part of its upper head (hood or goggles)
    outside them (resource-porter's shell measure; the deepest: Biker's
    helmet 30 mm, Master's hood 16 mm, Explorer's hat, Paratrooper's helmet
    and the OZK-18KB 9-10 mm, the rest 3-8 mm; only the Master's hood's alpha
    surface is clear). The cloth can be pulled in under the helmets as the
    other masks' is (a face-069d pack version with 1816 fitted with
    `helmets_worn_over_masks`), the goggles bending with it; an in-game look
    decides.
- 1823 and 1814 are both Renaissance balaclavas with one eye opening (STK2's
  and 0.69d's); the names keep them apart ("Army balaclava", "Balaclava").

## Open for the owner

1. The proposed armour / isolation, weights, prices and drop weights above
   (1823, 1812, 1813, 1816), and the levels of 1814 and 1815.
2. The item descriptions (0.69d has none; written new).
3. 1816 with helmets: as the pack fitted it (the hood through most helmets),
   or refitted under them (the goggles bent); the slot itself is decided (T28,
   a mask only).

## Fit checks (offline, resource-porter builders)

- Every mask: the fit, then the cloth's folds smoothed out (`mask_fit::unfold`),
  then the relief over the game's poses (19 lobby and match clips x 24 poses
  and the bind pose; through the head's vertices, edge and triangle middles),
  then the gate: at least 0.2 mm of cloth over the head in every pose. All six
  pass (least gaps 0.53 to 2.7 mm).
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
- In `release/content.tsv` the packs are two `pack` rows at the end of their
  kind: `army-balaclava@2` (pack.json sha256
  `a89701003319783aada1ce4df4bcca9a7a147179fe14829d423e094b512cde40`) and
  `face-069d@2` (`215d1e935f997efb638fcb10f0ea4b2ed49f5ff276ecc67eec36628c05113f11`).
  `release/build-content` composes them over the base and the movies built from
  source: the six icons go into the lobby movie it builds (cells [10..13, 6],
  [15, 6], [20, 6]; nothing else of the atlas changes).
- **Where the packs live.** The release PC keeps them in
  `private/content-packs/<id>-<version>/` of the main checkout
  (`SKHID_PACKS_DIR` in `private/release/release.env`), with resource-porter's
  `vostok-port` at `private/release-tools/vostok-port` (`SKHID_VOSTOK_PORT`: a
  wrapper giving the binary its zlib; built from resource-porter
  `feat/army-balaclava-stk2` 090a322). Another release machine copies the two
  folders there (the row's sha256 checks the pack.json) or rebuilds them from
  resource-porter: `examples/build-army-026` and `build-face-items-069d` over
  the stock db and the released patches, `scripts/render-mask-026-icons.py`,
  then `vostok-port pack from-bundle` (ids `army-balaclava` and `face-069d`,
  version 2). The donor sources: the STK2 characters export
  (`___surva_stk2_characters_port.7z`, the owner's download) with STK2's
  `bandit_01_attach` textures (`/mnt/toshiba/surv-builds/STK2.7z`), and the 0.69d
  files in `/mnt/hdd/Projects/face-items-069d/src069/files`.
