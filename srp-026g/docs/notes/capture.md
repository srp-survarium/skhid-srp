# Captures: straight into a map, pictures from poses, the game ends

The owner, 2026-11-21: "I am surprised for capturing we still don't have a way to skip lobby and
all and just go into capturing."

Before this every capture rig (map loading pictures, weapon shots, bake previews) started a
private srp, signed a client in, clicked through the login screen and the lobby into a match with
xdotool, imported resource-porter's camera hook DLL into a copied exe and pressed keys. Now one
command does it with no server and no input:

```sh
~/.cache/skhid-heavy/run --gpu nix shell nixpkgs#python3 -c python3 srp-026g/scripts/capture/capture.py \
  --map 39 --poses srp-026g/scripts/capture/fire-station.json --out <dir> \
  --game "$HOME/games/Survarium-0.26g-srp-files(2)/game" --exe <exe with patch 176> \
  [--content <build>/resources.db.patch_<n>] [--game-data <server-game-data.db>]
```

## How

1. **A replay of the map with nobody in it.** `srp-map-sur <maps_dict id> <out.sur>`
   (`match_server::v026g::sur_convert::of_map`) writes a `.sur` (`replay_sur.md`) of the map's
   options (one roster place, which never enters) and its mode config, from the game data, with
   no key frame and no event: the body is the start marker at 50 ms and the end record an hour
   later. The game builds the world fresh from those two chunks, as for any replay; with no key
   frame the next key frame's time is -1 (0x218E19) and the end is parsed from the body's start
   (0x1F0749), so nothing else is read.
2. **The game started with `-match_replay=` of it** goes from its start into the map: no server,
   no sign-in, no lobby (Fire Station, two poses: 38 s from the start to the exit on this machine).
3. **Exe patch 176 (`SKHID_CAPTURE=<script>`)** does the rest in the game, and nothing without
   the variable (read once, at the world's first tick): the camera at the first pose from the
   world's first tick (so it streams in), `wait` ms, then each pose held its settle time and the
   next frame's scene saved as `<folder>\<name>.png` before the interface is drawn over it (F12's
   picture, patch 152's encoder), then `ExitProcess(0)`. A script it cannot read ends the game
   with 2, a picture not saved with 1; the reason is in `srp_patches.<date>.log`.
4. **`capture.py`** does the rest outside the game: the replay, a game folder of its own (the
   given folder's `resources.db*` linked, never written; `--content` patches linked over them),
   a Wine prefix of its own (a copy of `private/local-clients/prefix-template`, its `user.cfg`
   given the window size and the poses file's `user_cfg`), an Xorg display of its own (the dummy
   driver at 60 Hz, openbox; never the desktop's, so no window on anyone's screen), silence
   (always: Wine's audio drivers off, `ALSOFT_DRIVERS=null`, no PulseAudio or PipeWire server to
   reach, `s_general_volume 0`; a run on 2026-11-21 opened no PipeWire stream), the script,
   the game, and everything stopped afterwards. It fails unless the game exited 0 with every
   picture made.

## The camera

base_game_scene::apply_camera (0x23DDD0, `camera.apply`) copies its camera_director's transform
(+4h, rows right, up, forward, place) and projection (+44h) into the scene for the frame's draw.
Patch 176 writes both at its start, every frame of every scene: the transform from the pose's
place and direction (no roll), the projection D3D's left-handed one with the pose's vertical field
of view, depth range and the game's own aspect (its projection's [1][1] / [0][0]), no jitter. The
same place and matrices as resource-porter's camera hook (`tools/render-compare/camera_hook.c`),
which earlier previews used.

## The poses file

A list of poses or `{"capture": {defaults}, "poses": [...]}`. A pose: `name`, `position`, and
`forward` or `yaw`/`pitch` (degrees; yaw 0 along +z, 90 along +x, pitch up positive; the
previews' convention), `hfov` (horizontal, default 90) or `fov` (vertical), `near` (0.1), `far`
(1500), `settle_ms` (4000). Defaults also: `wait_ms` (15000), `size` (`1920x1080`), `user_cfg`.
The console command `where` (patch 133) prints a place and look to copy from a game.

## Not covered

- Players, weapons or items in the picture: the world is the map's own (its objects, anomalies,
  lamps, weather), nobody in it. A weapon or kit shot needs a roster place that enters with that
  kit (an enter record and the kit in the options): the same file with one more event.
- The match's own clock runs (a free-for-all shows its waiting countdown in the hud, which the
  picture leaves out).
