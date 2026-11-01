# The pose marker (exe patch 89, local builds only)

The owner's ask (2026-11-01): press a key in a match and the place and direction are
remembered, so a map port can be checked against its donor from those spots.

## The rules

- Local builds only: patch_026g.py `--diagnostics` (the Rust patches' switch,
  `vostok::trace::enabled`, the same build as `srp_ragdoll.csv` and the lobby camera sweep).
  `release/build-zip` never passes it; in every other build the three hooks return at once and
  nothing is written. The self-test checks the default build has the switch off.
- The key is the **comma** (`,`, DIK 33h, the key right of M; `Б` on a Russian layout). M is
  bound: `set_next_ammo_type`, the input's next-ammo bit. No action has the comma by default
  (the action table, 0xA74AF0). The key is never taken: whatever the player bound to it still
  happens.
- Only in a match (the game world's keys, `tab_cursor.key`, after patch 86). One row a press: the
  key going down asks, the next frame's camera is written. Held or let go does nothing.
- What is written is what the screen shows: the transform update_camera gives the view
  (`camera.eye`, after patch 76's death camera), so a dead player's death camera too.
- The row goes to `srp_poses.tsv` in the user data folder (`Documents\survarium`, beside
  `srp_ragdoll.csv`), its header first when the file is new. The file is opened, appended and
  closed at each press. `srp_patches.log` (the game folder) gets a line naming the row. No
  on-screen message (the hud movie has no plain text call for it).
- Nothing a match computes; nothing sent anywhere.

## The row

Tab-separated, one a press:

```
name	time	map	project	mode	x	y	z	yaw	pitch	vfov
mark_20261101_090507_042	2026-11-01 09:05:07.042	204	level_01/day_tdm	4	-48.537	24.500	-103.080	-45.00	-2.00	65.00
```

- `name`: `mark_<date>_<time>_<ms>`, unique; the comparator names its shots by it (edit it to
  name a spot);
- `time`: the machine's local time;
- `map`: the match's maps_dict id (its options' +577B4h; `-` when there is none);
- `project`, `mode`: `match_making_params.map_name` and `mode.mode` of the level the lobby
  loaded last (switch_to_level_loading, `level_loading.map`), when it is the match's map;
  `-` otherwise (a replay of another map);
- `x y z`: the camera's place, world metres, y up;
- `yaw pitch`: degrees, forward = (sin yaw cos pitch, sin pitch, cos yaw cos pitch), the map-port
  comparator's convention (yaw 0 looks along +z, 90 along +x; pitch up positive);
- `vfov`: the camera's vertical angle unaimed, `uro_fov` (the Settings' FOV, 65 stock). An aimed
  weapon narrows the real view; mark poses unaimed.

## Using it

- Build the local exe (from `srp-026g/client-patches`, under `nix develop`):
  `python patch_026g.py <original survarium.exe> <game>/binaries/x86/survarium_skhid.diag.exe
  --server=127.0.0.1:<login port> --diagnostics`, and give the account that uses it this exe
  (`executable` in the local-clients config, the skhid-local-game skill). make_client_026g.py
  has no such flag on purpose: it makes the players' overlay.
- The map-port rig takes the file as its poses (`--poses .../srp_poses.tsv`, and every other
  poses argument of it): each row a pose with its map, role `marked`, hfov the row's vfov at the
  capture's 1280 x 720 (65 -> 97.1). Its `marks.py` writes the JSON form.

## Checks

`check_pose_marker_026g.py` (Unicorn: the three hooks from their sites in the default and the
diagnostics build, made-up objects, the game's own config lookups and match options),
`selftest_026g.py` check 89, `cargo test -p patches pose_marker` (the row, the yaw and pitch,
the state).
