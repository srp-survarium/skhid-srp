# Equipment profiles (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Status words: PROVEN = read from the exe / lobby movie / game data (address or file given); LIKELY = inferred; CHOICE = a
server rule no source states; OPEN = unknown, nothing invented. Nothing here was run against the real client.
VAs: exe base 0x200000000. AS = the lobby movie (`flash_movies/lobby_menu.swf`, class `survarium.player.ProfileSelector`,
`ProfileLevelRenderer`, `LobbyMenu`). Code: `crates/accounts/src/equip.rs` (schema v15), `servers/lobby-server/src/profiles.rs`,
`build.rs` (`account_87`), `equip.rs`, `presets.rs`, `progress.rs` (`new_player`).

## 1. What the client has (PROVEN)

* Three profiles at most: `ProfileSelector.max_profile_count = 3` (AS). The account table holds `+0x2F0` profiles, stride
  0x2A08, id at `+0x29F8`, state `+0x29FC`, enabled flag `+0x2A00` (87 reader 0x200125680).
* 87 profile record: `u8 state, u8 enabled, u32 id, str name, slot mask + (u16 item index, u16 rounds) pairs, ...`
  (the reader defaults the state to 2 before it reads the byte). After the profile list the 87 carries, per profile, a `u32`
  version (`table+0x300+4i`) and a `u8` (`table+0x33B+i`), then `u32 current profile id` (reader 0x200186430).
* State meaning (AS `ProfileSelector.setupProfiles`, C++ 0x2001BDD20): the Flash state is computed by the client: 2 = a slot the
  player does not have (shown as a tab to buy, with `tipText` = silver price), 1 = the client's play check failed (reasons
  `st_profile_check_failed_*`, among them `..._disabled`), 0 = fine. The wire state byte is only compared with 2.
* The price: static config `buy_preset_cost` of `game.options` = `[[200000, 500000]]`; the client reads it itself
  (`root.set_presets_cost`, 0x2001C02C4 sends it to Flash) and shows `_profilesCost[active - 1]`, `active` = number of profiles
  it has: the second profile costs 200 000, the third 500 000, in silver (AS `AskBuyProfile`: `Payment.MONEY_REGULAR`, text
  `premium_bonus_profile`). The server never sends the price.
* Requests (Flash passes the TAB INDEX; the C++ handlers turn it into the profile id at `+0x29F8`): `48 00 u32 id, str name`
  rename (0x200291000), `48 01 u32 id, u8 on` enable (0x200291150), `48 02 u32 id` select (0x200290F80), `48 03 u8 index` buy
  (AS `buy_preset(uint index)`, `index` = number of profiles the client has). Equip `45 00` and buy `46 00` carry the id of the
  profile in use; presets `58` name it too.
* Answers: `83 48 02 id` makes the profile current and rebuilds the character (0x200192CDD). `83 48` sub 0, 1 and 3 do nothing
  (the reader's jump table at 0x200192D64 starts at sub 2; subs 0 and 1 are out of range, sub 3 is the epilogue). So rename,
  enable and buy are answered with a fresh 87 (the 87 reader rebuilds the table and the tabs).
* The checkbox of a tab sends `48 01 index, !enabled`; the client only offers it when another profile is enabled
  (`isOtherProfilesEnabled`). A disabled profile fails the client's play check.
* Free-profile promotion: 94 records of type 1..7 (`useTime`, `service_dict_id`, `actionTime` seconds, 0x2001C56B0 ->
  `root.show_all_presets_service`) give a countdown; when it runs out the client locks every profile whose index has a non-zero
  entry in the table at 0x20096DE30 (`{0, 3, 3}`: profiles 1 and 2) unless its `table+0x33B+i` byte is non-zero (0x2001E9E90),
  and falls back to profile 0. So that byte = "bought for good".
* New-player kit: `items_for_new_player.options` rows carry `profiles_to_wear` 0..2: profile 0 AKMN + 200 x 7.62x39, profile 1
  Mosin + 30, profile 2 TOZ-66 + 80, the rest (TT-33, medkit, grenade, faction armour) for all three
  (`progression-audit.md` section 1).
* Official notes (0.6x era): premium unlocks all 3 profiles (0.63).

## 2. Server rules

| rule | status |
|---|---|
| Wire id of profile `i` = `i + 1`; the 87 always lists all three profiles; `current_profile` = id of the current one | CHOICE (the client takes any id) |
| Locked profile = state byte 2, empty slot table, name "Profile N" | CHOICE (the client builds a buy slot either way; name is the English `st_default_profile_nameN`) |
| Per-profile u8 of the 87 = 1 for a bought profile 1 / 2 (so a free-period expiry does not lock it), version = account version | LIKELY |
| Available = owned, or premium running (profiles 1, 2) | CHOICE from the official note (0.63) |
| A premium-only profile that is current when the premium ran out falls back to profile 0 at every match start (local Play, online queue / room / PvE start: `profiles::ensure_playable`), not only at sign-in, `45 00` and `48 02` | CHOICE |
| Using up part of a stack (`46 01` partial sale, `45` use) caps the rounds placed in the ammo and pocket slots of every profile that holds the stack, not only the current one (the new-player kit shares pistol ammo 1178 across the three profiles) | CHOICE |
| New accounts own all three profiles, each wears its kit | CHOICE: the data kits all three; the promotion period (94 types 1..7) has no known length, so no countdown is sent and the profiles are permanent. An account with no stored profile rows owns only profile 0 |
| Profile 0 is current at creation; the Mosin kit is profile index 1 (data profile 1) | CHOICE |
| `48 02`: switches the worn items (`Profile::switch_profile`), 87 when it changed, then `83 48 02 id`. Unknown / locked id: answered with the current id | PROVEN answer, CHOICE refusal |
| `48 00`: 1..32 characters after trimming, no control characters; locked profiles cannot be renamed; answered with an 87 | CHOICE (no limit in the client), answer LIKELY |
| `48 01`: the last enabled usable profile stays on; disabling the current one makes the first other enabled one current; 87 | CHOICE (mirrors the UI) |
| `48 03 index`: charges `buy_preset_cost[0][owned - 1]` silver, marks the profile owned, answers 87 + 8B. Not enough silver, owned or bad index: nothing, 87 | price PROVEN, refusal message OPEN (no `83 48 03` handler; the client might get a system message from the real server) |
| `45 00` edits the profile it names when that is another available profile (the client announces tab changes with `48 02`) | CHOICE |
| Presets (`58`) act on the current profile; a request naming another profile is ignored | CHOICE |
| The match loadout and the pockets use the CURRENT profile: `Profile::slots` / `slot_rounds` ARE the current profile's equipment; the others are stashed in `equip` until a switch | design (every worn-items rule follows the current profile) |
| 93 / 88 (player details, squad member) show the other player's CURRENT profile only | CHOICE |
| Which profile a match uses when several are enabled | OPEN: the queue / play requests carry no profile; the current one is used |
| What "enabled" does besides the client's play check | OPEN |
| Deleting an item (sale, consumption) removes it from every profile's stash | design |

## 3. Storage (schema v15, idempotent)

`equip_state (account_id, current)`, `equip_profiles (account_id, idx, name, enabled, owned)`,
`equip_profile_slots (account_id, idx, slot, uid, rounds)` (profiles that are not current; the current one is the existing
`slots` table), written only when the table differs from the default. `skin_unlocks` belongs to `weapon-skins.md`.

## 4. Checks in the game

* Three tabs at the top of the equipment screen, each wearing its kit (AKMN / Mosin / TOZ-66 for a new account); switching a
  tab changes the 3D character; the equipment edits stay with the tab.
* Right click on a tab: rename (name survives re-login); the checkbox disables / enables a profile.
* An account that owns profile 1 only shows two buy slots: 200 000 silver, then 500 000; after the buy the tab works and 8B
  shows the silver gone.
* A match starts with the equipment of the selected tab.
* Open: whether the locked tabs show a readable label ("Profile 2" highlighted), whether the checkbox exists in this build
  (`updateDisableButton` is empty in the movie, the per-tab checkbox is not).

## 5. More exe findings

* `enabled` (+0x2A00) readers: the lobby slot warning (disabled skips `profile_check`), the PvE start check (0x2001EB8AD: one enabled
  passing profile is enough; all disabled -> `st_profile_check_failed_disabled`), the equipment-level pickers (0x2001C752C/7588,
  0x2001BFA86/AE4: max level over enabled, usable, passing profiles). The options writer copies it into each profile record as 0 / 0xFF
  (0x2001CCFA9 from 0x2001CC8C1). The in-match switch (0x20023E34B) ignores it.
* "enabled" besides the Play check (PROVEN): the client's equipment tier is the maximum `item_level` over ENABLED
  profiles (UI+0x15F331); it drives the Hero of the Day group, the client-side ranked check ("Minimal tier is 3"), the league tier
  check, the "not enough money for maintenance" warning (`84` for request 0x2F) and the gun-race weapon icons.
* Which profile a match uses: no request carries one. In the match the respawn UI picks it (`respawn_checkbox_selected`;
  `CB` profile_input byte 2, 0x2001F13CD; 0x20F43FC6A is the cvar `set_player_autorespawn`, not the profile choice); the
  wheel / number keys cycle profiles whose state dword is 0.
* +0x330 writer 0x2002E7002 (PROVEN): the match builds one player object per profile whose state byte is 0 (enabled byte ignored),
  +0x330 = profile index; the first active profile is the lowest index that got an object. Slot = profile index: LIKELY (OPEN:
  who fills `entry[0..8]` at world+0x17C2A0). Our options send one profile, so the match starts with it; if all profiles are ever
  sent, put the current one at index 0. OPEN: whether the in-match profile choice (+0x10F5CC) reaches the server.
* `48 03 u8` (buy) sets no lock and no pending state; only the generic `84 48` text reacts (PROVEN). The client has no text
  of its own and checks the price first (`AskBuyProfile`); the server sends `84 x 48 01 st_not_enough_money` when the
  silver is short.
