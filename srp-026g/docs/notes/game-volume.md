# The Game volume (patch 144)

The owner, 2026-11-14: "There should be setting to make sound of the game lower too (steps,
sounds, etc)", "which doesn't include voice chat", "or music".

## The rules

- **What it lowers:** every sound of the game's own: footsteps, shots, hits, explosions, effects,
  ambience, the lobby's room tone and the menus' interface sounds (play_ui_sound 0x1EDBF0 plays them in the
  current screen's scene).
- **What it leaves:** the music (patch 88's themes: General x Music only) and the voice chat
  (patch 66: its own slider only, played outside the engine).
- **The slider:** the stock Settings' Audio tab's **Game** slider, under General. 0 to 100, 100 at
  first (stock loudness), saved in `user.cfg` as `s_ingame_volume`; Apply and Cancel as every
  stock slider.
- **The mix:** a game sound plays at General x Game; the music at General x Music. Game 0 silences
  the game's sounds and changes nothing of the music.
- **When:** at once, from the next frame after Apply, in the menus, the lobby and a match alike.
- **Not a match change:** presentation only; replays and every other client are untouched.

## What stock 0.26g had

- The Game slider's option names `s_ingame_volume` (options_tab 0x262D55, option 1 of the sound
  tab), a console variable the game never registered: its only reference in the exe is that
  option. So it had no range and set nothing, and the main menu greyed the slider out (as it did
  Music's before patch 88). Registering it alone would give the slider a value nothing reads.
- **The engine's mix:** every sound plays in a sound scene, each scene one XAudio2 submix voice
  that all its voices are sent to (0x631E5). The submix's volume is the scene's fade volume
  (+2E4h) x its factor of General (+2E8h). The factor is General x 0.01, written by
  add_scene_to_active (0x59FFC) and by the sound tick's general pass (0x59ADF..0x59B4B, when
  `s_general_volume` changed; its value 0xA7AF90 has no other reader); process_fade (0x5C060)
  applies it while a scene fades. There is no per-category volume.

## How it works

- **Music stays put** because it has a scene of its own: patch 88's bus (class 38, never the
  game's current scene). The game's sounds are in the game's scenes (the login's, the lobby's, the
  match's).
- **The general pass is ours** (`sound_world.general`, a lohook at 0x59ADF, exit 0x59B4B past the
  engine's pass), every sound tick, after the scenes ticked: each active scene gets General x 0.01
  x its share, the bus 1 and every other scene Game / 100; a scene whose factor changed gets its
  submix's volume at once (SetVolume, as the engine's pass). One writer of the factor once a scene
  is active: add_scene_to_active's General-only factor is corrected in the same tick.
- At Game 100 the pass gives the same factors and the same submix calls as the engine's.
- **The variable** is registered at `configs.start` (before user.cfg runs), a cc_float as
  `s_music_volume`; the game's thread copies it to the sound thread every frame (`game.tick`).
- **The movie:** the main menu's sound tab no longer greys the slider out (`settingsTabs_15.as`).

## Not yet

- Not heard in a game: the owner's listen decides it.
