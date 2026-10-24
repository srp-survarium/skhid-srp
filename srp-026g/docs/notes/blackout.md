# Blackout (prototype)

The owner (2026-10-24): "make a game with all ui disabled and players having color bands to
distinguish teams… this is the new battery mode"; "take bands from stalker". Working name, not
final. A prototype to try locally: nothing here is decided until the owner says so.

## Rules (the prototype's defaults)

- **Which matches:** every battery match (modes 2 and 3) while the server runs with
  `SRP_BLACKOUT=on`; off by default. Battery rules unchanged (chargers spawn batteries, a team
  deposits them in its container).
- **No interface for the whole match:** exactly what F11 (patch 62) hides, from the match's first
  frame (the kits before the start too) to its end. Ammo, health, score, crosshair, minimap,
  markers, kill feed, chat, Tab list: all hidden.
- **Kept:** the screen effects (pain edges, blood drips, grenade flash), the 3D world, the Esc
  menu, the cursor of the kit screen.
- **F11 does nothing** in a Blackout match; keys 1 to 3 still pick a kit while hidden.
- **The end:** the hud stays hidden to the match's end (its winner/loser line too); the lobby and
  its results screen are as always.
- **Bands:** every player wears his team's armband on the upper arm: team 0 blue, team 1 green
  (absolute colours, the same for every viewer and in replays).
- **Not in the lobby:** the band is only on a match's characters.
- **Replays and late joiners:** they get the flag with the static info, so they see the match
  as it was (no interface, the bands); a replay's own controls still toggle with F11.
- **Not a simulation change:** the interface and the characters' looks only; no
  `simulation_changes.tsv` row.

## Bands

- **Source:** the multiplayer team bodies of Clear Sky and Call of Pripyat (byte-identical; taken
  from Call of Pripyat): `meshes/actors/stalker_mp/mp_team_{blue,green}_1.ogf`, textures
  `act_mp_team_{blue,green}_1.dds`. The band is a child mesh of each body, on both upper arms,
  self-lit in STALKER. Shadow of Chernobyl's MP has faction suits, no bands.
- **Content:** pack `blackout-bands` (resource-porter `feat/blackout-bands`,
  `examples/build-blackout-bands.rs`): two models on the native skeleton,
  `models/ported/blackout/band_blue` and `band_green`, surface `band`, the cloth cut from each
  texture around its emblem (64x256, flat normal map); no item, no dictionary row.
- **On the body:** a 32-segment ring on each upper arm, rigid on `LeftArm` / `RightArm`, 61 mm
  wide, its centre 208 mm below the shoulder (0.65-0.88 of the upper arm: lower than STALKER's
  spot, below the armpits and the combat suit's shoulder plates), the emblem outwards.
- **Fit:** one ring outside every sleeve of 70 torso models + 5 mm: about 70 cm around, so it
  stands 2-5 cm off most sleeves (least 5 mm, mean 19-49 mm); in some clips a sleeve pokes
  through its top edge (worst 28 mm, the AK lobby idle). Snug bands need sizes per torso.
- **Team colour:** two models, one per team; the exe picks the model by the player's team. Not
  self-lit (the plain cloth material).

## How

- **Server:** `SRP_BLACKOUT=on` puts tag 2 (one byte, 1) in srp's block after the static info's
  squads (`static_info.rs`), for battery maps only; a late joiner's static info keeps it.
- **Exe, patch 65** (`blackout_026g.asm` and `.c`; `client_patch.md` 3.58):
  - patch 63's reader of srp's block (0x1F0F30) keeps the flag in the options (+57870h);
  - patch 62's reset of a match's interface (0x234AE8) calls `hide_ui_force` with it: the one
    entry that forces the interface hidden and locks F11;
  - `profile_skin_visual_cook::on_configs_loaded` (0x2056F0) adds one part record, the team's
    band, to a match's player's character, and tells the visual's name apart by team.
- **Movies:** no change; patch 62's `hide_interface` hides each movie's root.
- **With the interface levels** (`feat/interface-level`, its own patch 64): the one forcing call,
  `hide_ui_force(bo_match())` in patch 62's reset, becomes `interface_force(0, 2)` (Disabled) or
  `interface_force(0, 0)` once both are merged.

## Open for the owner

- The name; which modes; relative colours (own team blue, enemy green) instead of absolute.
- The end of the match: show the hud's winner/loser line, or the interface, once it is over.
- Score: none in the match, or a short line on each battery delivered.
- The bands' look: loose one-size rings (now), a few sizes picked per torso, or self-lit as
  STALKER's (an emissive material).
- A game without patch 65 would show the interface and no bands: refuse it by revision before
  a release (`SRP_MIN_PATCH_REVISION`).
- Rooms: offered as its own mode, or a server switch only (today: the switch).
