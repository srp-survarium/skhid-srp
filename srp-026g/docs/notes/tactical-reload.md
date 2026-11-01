# Tactical reload

The owner (2026-11-01, "properly port all weapons"): "Do tactical reloading too." Exe patch 90
(Rust `tactical_reload`), content-edit `tactical-reload-1`, the porter's weapon core.

## The rules

- **A tactical reload** is a reload with rounds left in the magazine. It plays the weapon's
  tactical clip and lasts `parameters/tactical_reload_time`. A reload of an empty magazine plays
  the empty clip and lasts `parameters/reload_time`. As retail 0.69d0.
- **Which reloads:** the two-clip reload, state type 314 (0.26g's `pistol_weapon_core_reload_state`,
  0.31's and 0.69's 313): the pistols, the SVD, the SKS, the SOCOM 16, the Kedr, the Kiparis, every
  0.29/0.31/0.46/0.69 port with a `reload_tactic` clip. 28 live configs today.
- **Not:** the one-clip reload (303: the stock AKs, SMGs, the machine guns, the Magnum, the VSSK,
  the TOZ-122): one clip, nothing to choose. Retail's 303 ignores `tactical_reload_time` too
  (0x20038E440 reads `reload_time` only). Shell-by-shell (306: the pump and semi-auto shotguns,
  the bolt-actions): already only the missing rounds; 0.26g has no tactical start clip (0.69's
  `reload_finalround`), so they stay as they are. Double barrels (311): as retail, one time.
- **The clip** is the game's choice, unchanged: `ammo_in_magazine == 0` (get_weapon_lexeme 0x30FE90,
  retail's initialize 0x2003909A0). No round is kept in the chamber: retail has no +1 (its
  load_magazine 0x2006D6C50 fills to `magazine_capacity`; a magazine gun has no chamber state).
- **A weapon without `tactical_reload_time`** keeps the game's timing (both clips at the
  tactical clip's speed over `reload_time`). So does one whose value is no positive time.
- **Modifications:** the reload-time ones (2040-2044, defect 3040) scale both times, as 0.69d0's
  do (a second modifier on `tactical_reload_time`). The player's reload-speed modifier (13) applies
  to either clip as before.
- **The card** shows `reload_time`, now the empty reload's (as retail's card).
- **Deterministic:** both speeds come from the weapon's config when its states are cooked; the clip
  from the simulated magazine. Every client of a match times a reload alike. The match's
  computation changes (`simulation_changes.tsv`): older replays are refused.

## The data (content-edit tactical-reload-1)

Each live 314 weapon's `reload_time` is how long its tactical clip lasts in the game today. It
becomes `tactical_reload_time`, unchanged: the common reload feels the same. `reload_time` (the
empty one) is it times the same gun's 0.69d0 `reload_time / tactical_reload_time`, rounded to 0.01 s, or to a whole second where the config's `reload_time` is an integer (a key keeps its type: build-content's type check).
Before, the empty reload lasted `reload_time` x (empty clip / tactical clip), the clips' ratio.

| weapon | tactical (= live) | empty, now | empty, before | 0.69d0 rt / trt |
|---|---|---|---|---|
| PM | 1.6 | 2.4 | 3.08 | 2.4 / 1.6 |
| TT-33 | 1.6 | 2.5 | 3.08 | 2.5 / 1.6 |
| Fort-17 | 1.6 | 2.63 | 3.08 | 2.3 / 1.4 |
| APS | 1.3 | 2.11 | 2.50 | 2.6 / 1.6 |
| Kedr | 2.5 | 3.57 | 2.50 | 3.0 / 2.1 |
| Kiparis (+premium) | 2.5 | 3.2 | 2.50 | 3.2 / 2.5 |
| SVD | 3.1 | 3.62 | 4.34 | 3.5 / 3.0 |
| SKS (+2 premiums) | 5.0 | 3 | 3.22 | 3.7 / 5.7 |
| SOCOM 16 | 2.7 | 3.67 | 3.42 | 3.4 / 2.5 |
| HK416 | 3.0 | 4 | 3.38 | 3.0 / 2.2 |
| M4A1 | 3.2 | 4.27 | 3.60 | 3.2 / 2.4 |
| FN FNC | 3.0 | 4 | 4.61 | 3.0 / 2.2 |
| P90 | 4.0 | 5 | 4.53 | 4.5 / 3.5 |
| Kriss Vector | 3.0 | 4 | 3.56 | 3.5 / 2.8 |
| MP7 (+2 silenced) | 3.2 | 3.73 | 3.28 | 2.8 / 2.4 |
| MSBS | 3.0 | 4 | 3.26 | 3.0 / 2.2 |
| Saiga-12 (+premium) | 2.8 | 3.63 | 3.90 | 3.5 / 2.7 |
| Vepr-12 (+premium) | 2.8 | 4.14 | 3.90 | 3.4 / 2.3 |
| SOC Vepr | 3.1 | 4.18 | 4.32 | 3.1 / 2.3 |
| Valmet RK62 | 3.0 | 4 | 4.41 | 3.0 / 2.2 |
| vz. 58 | 3.0 | 4 | 4.22 | 3.0 / 2.2 |

New ports (the porter's weapon core, `resource-porter` `src/weapons_026/config.rs`): a 314 weapon
gets `tactical_reload_time` = its `reload_time` x the donor's `tactical_reload_time / reload_time`
(0.46c2 and 0.69d0 have both); a donor without it (0.29a3, 0.31a2: `tactical_reload_time` came in
between 0.31a2 and 0.46c2) the median of 0.69d0's two-clip guns' ratios, 0.75. A manifest's `set` may give
it outright.

## How (exe patch 90)

The game (0.26g) gives the state one speed. Its ctor (0x30FC50) sets `[this+138h]` =
computed_show_animation_time_scale (0x30D750) of the first hands clip (the tactical one) over
`params->reload_time`; get_weapon_lexeme (0x30FE90) picks the hands clip by
`2 x (ammo_in_magazine == 0) + 4 x crouch` and hands `[this+138h]` to apply_modifier 13; the item
clip follows the hands (weapon_and_hands_expression 0x30FE38, weight_driving). The reload ends at
the clip's end (`on_animation_end` -> `instant_reload`).

- `pistol_reload.constructed` (0x255276, the sound handler's ctor after the state's): with both
  times in the config, `[this+138h]` = the empty clip's length / `reload_time` and `[this+16Ch]` =
  the tactical clip's / `tactical_reload_time`, by the game's own 0x30D750; else +16Ch = 0.
  +16Ch is the class's tail padding (its handler's members start at +170h), written by nothing
  else.
- `pistol_reload.lexeme_time_scale` (0x30FEC9, after the speed is stored as apply_modifier's
  value): with a tactical speed and rounds in the magazine, that value is the tactical speed.

`check_tactical_reload_026g.py` runs the cook's ctor and get_weapon_lexeme in the original and the
patched exe.

## Retail 0.69d0 (x64, `private/music-research/retail-069d.exe`)

- weapon_core_cook::query_weapon_states (0x2003840E0): `reload_time`; `tactical_reload_time` (default `reload_time`),
  `reload_start_time`, `tactical_reload_start_time` (default `reload_start_time`),
  `reload_end_time`, `tactical_reload_end_time`, into weapon_state_creation_params +24h..+38h
  (0x2003847E2).
- pistol_weapon_core_reload_state (313; vtable 0x2009980F8): ctor 0x2003906D0 keeps
  `tactical_reload_time` at +2E4h; initialize 0x2003909A0 latches `[weapon+44BAh] == 0` (the
  magazine) at +2E0h and serializes it (0x200390A40); its lexeme 0x200390B60: tactical, the time
  +2E4h and clip +2D8h, else `reload_time` +26Ch and clip +2D0h; the time through the reload-speed
  modifier (0x20031E1E0, 0Bh), speed = clip length / 30 / time.
- weapon_core_reload_state (303) ctor 0x20038E440 and the double barrel's 0x2003900C0 read
  `reload_time` only.
- Shotgun substates (0x200395190: two times, +2B8h normal, +2B4h tactical): start
  0x20038FC00 (`reload_start_time`, `tactical_reload_start_time`), one round 0x20038FE50
  (`reload_time`, `tactical_reload_time`), finish (`reload_end_time`, `tactical_reload_end_time`);
  a pump's start clip pair is `[reload_start, reload_finalround]`.
- No +1: weapon_core_reload_state_base::initialize 0x20038EA70 puts a chambered round back into
  the magazine (0x2006D8E60) only with `chamber_a_round_on_reload`; load_magazine 0x2006D6C50
  loads `capacity - in_magazine`.
- 0.69d0's reload-time modifications (`modifications[20..25]` of its game options) have a second modifier,
  `parameters/tactical_reload_time`, of the first's value.
