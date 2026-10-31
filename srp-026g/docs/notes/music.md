# Music

The menus' and the maps' themes, as retail 0.69d plays them, with the owner's 15 s rule. Exe patch
88 (Rust `music`) plays them; the pack `music@1` carries them; the edit list `music-1.json` names
each mode's theme.

## The rules

The owner's (2026-10-28; 2026-10-31 the 15 s; 2026-10-31 one song at a time, the mixing and the Music slider):

- **Login screen:** the main theme (`music/main_theme`), looping.
- **Lobby:** the lobby theme (`music/lobby_theme`), looping. Back from a match, it starts again.
- **Loading a match:** the map's theme (its mode's `ui.loading_music`, as retail), from the loading
  screen into the match. It stops 15 s after the match-start announcement (the welcome message as
  the countdown ends), with a 2 s fade-out. Retail fades it at the announcement itself; the 15 s
  are the owner's (2026-10-31). Waiting for players and the countdown do not count.
- **A mode with no `ui.loading_music`:** no map theme; whatever played fades out.
- **Fades:** a theme fades in over 2 s and fades out over 2 s (retail's 2000 ms, both ways).
- **One song at a time:** at most one track is audible at any moment, and never two at once.
  - A change of track fades the playing one out completely (or it is stopped), and only then
    does the next one start, with its fade-in. A crossfade never happens.
  - A track that is still fading out when another is asked for (a quick lobby -> match ->
    lobby, a reconnect) is stopped at once, and the new one starts with its fade-in.
  - The track already playing, asked for again (the lobby theme in the lobby), plays on.
  - The track fading out, asked for again before it is silent (lobby -> loading -> lobby within
    the fade), fades back in where it is: it never restarts.
  - Stock 0.26g plays no music: the lobby's own `background` sound
    (`scapes/lobby/lobby_background`) is a room-tone noise bed (spectral flatness 0.34, against
    0.02 to 0.03 for the themes), a scene object of the lobby that stays.
- **Late join, rejoin:** as a match: the map theme from the loading screen, 15 s after the
  announcement this game shows (a late joiner's at its first look at the match).
- **A loading that never reaches a match** (the match lost on the way): the map theme plays on
  until the next screen (the login screen's or the lobby's theme follows the rules above).
- **Replays:** no music. A replay never passes the loading of a live match
  (`switch_to_level_loading`'s only caller is `process_static_match_info` 0x27BEA0), so its
  options read (`replay.options_read`, its chunk 1) fades out whatever plays (the lobby's theme,
  when it is started from the lobby); the lobby's theme comes back with the lobby.
- **Quitting:** the music stops at once when the game unloads its screens.
- **The bus not made** (the engine refusing its sound scene): logged to `srp_patches.log`, asked
  again once at the next screen (the login screen's, the lobby's, a loading's); a second refusal
  is logged too, and the session has no music.
- **Nothing a match computes:** presentation only, no `simulation_changes.tsv` row.

## Mixing and volume

- **Its own bus.** The music plays in a sound scene of its own (0.26g has no music category):
  its XAudio2 submix voice is the music's alone, so the music's fades and level never touch the
  game's sounds, and the game's scene switches (login -> lobby -> match, each a 10 ms cut in
  0.26g) never cut the music. The game's sounds never duck it and it never ducks them.
- **The volume:** the Settings' Music slider times General (the owner, 2026-10-31: "why not tie it
  to the Music slider?"). Stock 0.26g's Music slider was dead: its option names `s_music_volume`
  (options_tab 0x262DA1), a console variable the game never registered, so it had no range and set
  nothing, and the movie greyed it out. Patch 88 registers `s_music_volume` at
  on_configs_loaded's start (before user.cfg runs; the options are made later, at
  on_base_resources_created): a cc_float of 0 to 100 as General's (the slider snaps by 1), 100 at
  first, saved in user.cfg; the stock slider and its Apply/Cancel read and write it, and the main
  menu's sound tab no longer greys it out. The bus gets fade x slider / 100 (the game's thread
  copies the slider to the bus every frame); the engine multiplies every scene by General. A
  slider at 0 is silence, not a stopped track: the track's rules go on.
- **In-game and Voice chat** (`s_ingame_volume`, `s_chat_volume`) stay dead and greyed out:
  registering a variable would give each slider a value, but nothing in 0.26g would read it (the
  game's sounds are one scene's, the voice chat its own), so each needs a consumer as well, not a
  one-line registration.
- **Its level.** Each track is set to -24 LUFS (integrated) by its `volume_k` (`.raw_options`):
  the level of the lobby's own room tone (-18.3 LUFS at its `volume_k` 0.5). The game's shots,
  footsteps and interface sounds stay above it (the match-start announcement is the hud's text);
  the retail tracks span -22.9 (level 6) to -10.1 LUFS (main) as recorded, which `volume_k`
  evens out. To retune: the pack's next version with other `volume_k`s.
- **No HDR.** The tracks' `use_hdr` is false (the room tone's is true). LIKELY (the 0.10b engine's
  HDR audio, the flag read by the single sound's cook at 0x61427): the loudness window that lowers
  quiet sounds under loud ones; off, the music neither masks a quiet sound nor is pumped by a
  loud one.
- **Not heard yet:** the levels and the fades are by the numbers; the owner's in-game listen
  decides them.
- **Stereo, not 3D:** the tracks are hud sounds (`pan_3d` false), heard the same wherever the
  camera is.

## The content

- **The audio** is retail's own: the 0.46c2 `sounds/single/music/<track>.high.ogg` and `.high.rms`
  (Vorbis, 44.1 kHz stereo, 112 kb/s nominal, the encoding of 0.26g's own long sounds), byte for
  byte. The pack adds each a `.raw_options` (the stock lobby room tone's, with the level above,
  `use_hdr` and `pan_3d` false and `can_be_displaced` false: the engine's voice limit never takes
  a music voice). `srp-026g/scripts/music/make_pack.py` makes the pack from the 0.46c2 archive;
  the earlier downloads (`~/Downloads/Музыка.zip`, `sources.tsv`) are the inventory it was chosen
  from (the FLACs re-encoded would be the same tracks, larger and not the game's encoding).
- **The tracks:** `main_theme`, `lobby_theme` and `level_01_loading` .. `level_09_loading`: 11
  tracks, 16.5 MB (16 526 528 bytes with their RMS and options). Retail 0.46c2's battle finale and
  win/lose cues are not used.
- **Which map, which theme:** `music-1.json` adds `ui.loading_music` to every mode file, as retail
  has it (`gameplay/game_modes/<mode>.options`, `mode.ui.loading_music = "music/level_0N_loading"`):
  levels 1 to 9 their own, the ported maps their original's (the v0.21d and v0.11e maps their
  0.26g levels, Research's its level's). A new map adds its files (Rudnya 0.69's and the STALKER
  maps' when they land); a mode without the key has no theme.

## How it works (patch 88)

- **The owner of the music** is one state machine (`music.rs`): no bus yet / silent / loading a
  track / sounding (fading in or full, with the 15 s deadline once announced) / fading out (with
  the track asked for next). One transition function takes each event (a screen asked for a
  track, the announcement, a load done, a frame, the unload) and calls the engine.
- **The slider's variable** is registered at `configs.start` (on_configs_loaded's start, after
  patch 79's and patch 83's) with the game's own cc_value constructor, as a cc_float.
- **Events** (game thread): `login_menu::on_activate`, `lobby_menu::on_activate`,
  `lobby_menu::switch_to_level_loading` (the mode's `ui` table: `loading_music`),
  `game_world_ui::update_ui`'s welcome message, a replay's options read, `game::tick` (every
  frame: the deadline, a fade done, the slider), `game::clear_resources` (the unload). A load is asked of the resource manager
  (`query_resources`, class 256 single_sound, as the lobby's own sounds), its result handed back
  by a callback with the request's tag; a result nobody waits for any more is let go at once.
- **The bus** is a `sound_scene` (class 38, as the lobby's, creation params 8 proxies, 16
  propagators, 2 receivers) the game never makes current. The sound thread's tick
  (`sound_world::tick`, after its orders) keeps it among the active scenes and steps its volume
  towards the target at 1/2000 a millisecond: the fades run on the sound thread, so a slow frame
  never steps them. Its volume reaches the submix by the engine's own fade path (the scene's
  fade-in state, which writes `volume x general` to the submix every sound tick).
- **A listener of its own:** a scene's sounds get voices only once its listener is set (its tick
  calls notify_listener 0x5C280, the only path to create_sound_voice, only when +2FAh is set,
  0x57C9C); the game sets its scenes' every frame. The bus is given one when it opens, at the
  origin (set_listener_properties 0x587D0): a hud sound is heard the same wherever it is. (The
  first test build had none: the tracks loaded and played with no voice, silent, 2026-10-31.)
- **No hitch:** a track is loaded asynchronously (the Vorbis file, 0.8 to 5.3 MB, streamed and
  decoded by the sound thread as it plays); one track is in memory at a time (and the one fading
  out). Nothing waits on the frame.

## Retail 0.69d (research, 2026-10-28)

Inspected the x64 executable from `Survarium/game/binaries/x64/survarium.exe` in
`/mnt/hdd/survarium-offload/0.69d0/Survarium.zip`, SHA-256
`1e1ae04dc981fc4f5734c56559fb0f2e0e74c787bcc53ebbfdc233f440d65448`.
[`../../../tools/re-kit/v069d/`](../../../tools/re-kit/v069d/) records its identity and the
functions below. Addresses are preferred-image VAs (base `0x200000000`). Static reading, not an
audible retail playtest.

| Situation | Retail behaviour | Evidence |
| --- | --- | --- |
| Initial menus | `music/main_theme`, track 0; looping, fade-in argument 0 | Resource request at `0x200112d59`; call at `0x200114327` |
| Lobby | `music/lobby_theme`, track 1; looping, 2000 ms fade-in | Resource request at `0x200112dd4`; `play_music(1, 2000, true)` at `0x200117f1f` |
| Map loading | Reads `mode/ui/loading_music`, falling back to `music/level_01_loading`; loads single-sound class `0x100` asynchronously into track 2 | `0x20011b7c0`-`0x20011b8ab`; callback `0x20014bb60` |
| Loaded map track | Replaces the old slot's emitter/proxy; starts looping with 2000 ms fade-in | `0x200117a50`, called by the loading callback with slot 2 |
| Match-start announcement | Requests no music (track `0xff`), fading every playing music slot out over 2000 ms | `0x2001a8514`-`0x2001a85e6`, the `st_start_match_welcome_message_*` selection and the call at `0x2001a85df` |
| Finale | Track 3, `music/battle_finale` | Resource request at `0x200112e25`; calls at `0x2001e791e` and `0x200258134` |
| Result | Loss/win tracks 4/5, `music/music_loose` / `music/music_win`, non-looping | Result call at `0x2001ac9cc` |

- `survarium::game::play_music(music_track, unsigned int, bool)` (`0x200117b60`, named by the
  diagnostic at `0x200956030`) visits the six slots but the requested one and fades each playing
  proxy out over 2000 ms; a track outside 0..5 only stops; a requested slot already playing is
  not restarted. The outgoing fade is always 2000 ms: the proxy's fade (vtable `0x2009d7ae0` slot
  4, `0x20062a8f0`, queued to `0x200623400`) takes milliseconds and converts them to samples.
  Retail lets the fade-out and the new track's fade-in overlap; this port does not (the rules).
- The HUD's stop request comes after its waiting/countdown branch (`+0x512` clear), once for the
  start announcement (`+0x765c`), with its two place bytes (`+0x75a9`, `+0x75aa`) equal. The
  nearby `6000` is the welcome message's display time, not a music timer: retail holds nothing.
- Retail's data agrees: 0.69d's `gameplay/game_modes/level_01_day_tdm.options` has
  `mode.ui.loading_music = "music/level_01_loading"`; the archives hold
  `sounds/single/music/<track>.high.ogg` and `.high.rms`.
