# Lobby idles

Why the lobby character "freezes in one pose" and what changes. Data: each weapon config's
`user_animations_in_place/preview`; the change ships as a resources.db patch
(`scripts/content-edit/lobby-idles-1.json`, `remove_item`).

- **Report (a player, via the owner, 2026-10-23):** in the lobby the character sometimes
  freezes in one pose; just before, it twitches slightly, as if an animation were about to
  start, then it stands still. An old bug, the original game has it too.
- **Rule:** the lobby character never stands rigid. Every clip it can pick moves; the
  calm idles (`idle_wpn_loby_1`, `_2`: breathing, small shifts) stay.

## How the lobby picks its idle (PROVEN in the code)

- The character's player holds the shown kit's weapon; the weapon's animations selector
  (weapon+278h) has a preview state, `player_logic_preview_state` (ctor 0x296AB0, vtable
  0xA0B4FC), made in `weapon::load_weapon` (0x243B6C) with the weapon's clips (+4710h) and
  their count (+1818h).
- The clips are the weapon config's own `user_animations_in_place/preview`, read by
  `weapon_cook::on_weapon_config_loaded` (0x24AA78) and `on_weapon_subresources_ready`
  (0x24AE29). The shared `common/rifles.user_animations` and `pistols_new.user_animations`
  `preview` lists are not read for it.
- `selected_animations` (0x296D00): a random index (0x296C20: `x * 8088405h + 1`, seeded from
  RDTSC; above 3 clips the last three indices picked are left out, one history at 0xA73CF4
  for every preview state), the clip kept at +38h, a lexeme of it blended in over 0.3 s
  (0x921804).
- `initialize` (0x296B30) subscribes `on_animation_interval_end` (0x296BF0) to the player's
  animation player (player+750h): when the clip playing ends it interrupts the tick, and
  `base_player::tick_animations` (0x2D8EE0) selects again. So the character plays one random
  clip after another, each blended in over 0.3 s.

## The cause (PROVEN in the data)

Every clip a preview list names, its length (30 frames a second) and how many of its 630
channels (70 bones, 9 channels) move:

| clip | length | channels that move |
|---|---|---|
| ak `idle_wpn_lobby_5`, `idle_wpn_loby_5` (two copies) | 11.2 s | 118 |
| ak `idle_wpn_loby` | 13.3 s | **0** |
| ak `idle_wpn_loby_1` | 15.0 s | 92 (calm) |
| ak `idle_wpn_loby_2` | 15.0 s | 89 (calm) |
| ak `idle_wpn_loby_3` | 15.0 s | **0** |
| ak `idle_wpn_loby_6` (and mp153, ppsh, rem870 copies) | 5.2 s | 121 |
| ak `idle_wpn_loby_7` | 5.7 s | 93 |
| ak `idle_wpn_loby_8` | 13.2 s | 121 |
| tt33 `idle_wpn_loby`, `_1`, `_2`, `_4` (pistols) | 9.2, 15, 15, 8.7 s | 124, 91, 87, 87 |
| pkp `..._pkp_pecheneg01`, `02` (patch 19) | 12.7, 13.2 s | 121, 89 |

- `idle_wpn_loby` and `idle_wpn_loby_3` are a still pose: every channel of every bone is a
  constant (5 knots, one value). Picking one, the character blends into that pose over 0.3 s
  (the twitch) and then holds it, without even breathing, for 13.3 or 15 s; 28 s when both
  come one after the other (the history leaves out only the last three picks).
- They are items 1 and 4 of the nine-clip list of 77 weapon configs: every rifle, shotgun
  and SMG (stock and the 0.29 ports alike) and the TOZ-66. About 2 picks in 9 are still, so a
  player watching the lobby sees it every minute or so. The pistols (5 configs) and the PKP
  have no still clip.
- Not a missing clip: every clip the weapons' lists name is in the archives (the shared
  `pistols_new` list names a tt33 `idle_wpn_loby_3` that no archive has, but nothing reads
  that list). A rebuild of the character or a lost interval event (GUESS, not seen) would
  need no still clip; the still clips alone give exactly the report.

## The change

`lobby-idles-1.json`: both still clips out of the preview list of the 77 configs that name
them (`remove_item`); 9 -> 7 clips, all moving. The clips stay in the archive. No exe change,
nothing in a match: the preview state's random is seeded from RDTSC, so no simulation reads
it.

## Test (not run yet)

The local game with the patch built from this list, one window: in the lobby, a kit with a
rifle (any AK) on the character, watch it for 3 minutes without touching anything; then with
a shotgun, an SMG and a 0.29 weapon (AEK-919K, MSBS). Expected: one clip after another, each
moving; no 13-15 s still pose after a twitch. Before the patch the same watch shows it
within about a minute.
