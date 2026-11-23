# A shot into a corpse (exe patch 196)

The owner, 2026-11-23: "0.5s after ragdoll began, one shot anywhere in the body cuts the game
sounds and blackens the camera, also add from 0.21 death sound there too"; the sound "by everyone
in the environment"; "at respawn all sounds are back, radio is heard/usable even when dead".
Code: `client-patches/rust/patches/src/corpse_shot_dark.rs`; the shot and the step are patch 76's
(`ragdoll.rs`), the black view the hud movie's (`death-eyes.md`), the cut patch 144's
(`game-volume.md`), the sound the pack `death-sound-021d@1`. Not part of PR #1618.

## When and who

- **When:** from 0.5 s after a dead player's ragdoll began (its own clock, as the shooter's game
  draws it) until that player lives again.
- **The trigger:** one bullet of a player that meets that corpse anywhere on its body, as the
  shooter's game draws it ("positions are different for everyone": the shooter's game decides).
  A corpse drawn as the game draws it (no ragdoll free) cannot be hit.
- **One shot** (my reading): the first such shot of a death counts; later shots of the same corpse
  do nothing more, in every game.
- **A teammate's shot** counts as an enemy's (my reading: "one shot", no side named).
- **Bots:** a bot's corpse shot by a player: the sound for everyone, no screen to darken. A bot's
  shot never counts (no game of its own decides it).
- **Freeze Tag:** a frozen player is no ragdoll: nothing.

## The message

- The shooter's game tells it in its next input with no quick-use key and no command of its own:
  patch 78's marker (bit 29) with one of 20 quick-bit patterns, the victim's place
  (`weapon_pickup::corpse_shot`: patterns 20..39 of those with two or more bits; 0..19 are patch
  85's pickups, 63 patch 180's throw-back). The server relays it as any input and refuses a victim
  that is no place of the match (`weapon_pickup::sane`, as for a pickup).
- **Which death:** the one the victim is dead of at that input's tick, read where every game applies
  it (`carry.filter`). A victim alive there (the death undone) gets nothing. A repeat, or the same
  input applied again by a rollback, does nothing: one sound a game a death.

## The effect

- **The sound, everyone:** each game plays 0.21's death sound once at the skull of its own drawn
  corpse of that death, a point sound of the world (heard as far as its presets carry: the fall to
  5 m, the scream to 9 m), by the shooter, the dead player, bystanders and spectators alike. A game
  that draws no ragdoll of it plays nothing.
- **The dead player's own game:** the screen black at once (the hud movie's black view, under the
  hud: the respawn countdown and the kill feed stay); the game's sounds cut once 0.21's sound has
  played (3 s, its longest take), so he hears it (my reading: it plays before the cut).
- **The cut** is patch 144's Game slider's share set to 0 (every scene but the music's): the world's
  sounds and the menus' interface sounds. The volume sliders' own values are untouched.
- **The radio** (the voice chat, patch 66) plays outside the engine and XAudio2 altogether, on its
  own winmm waveOut stream (`client-voice/src/windows.rs`), and its microphone is untouched: heard
  and usable while dark (the owner's rule).
- **Music** (my reading: "game sounds" are what the Game slider governs): patch 88's music is not
  cut.
- **No one else's** view or sound changes; nothing a match computes depends on it.

## The end

- **All back at respawn** (the owner's rule): the dark is a function of the step's view of the local
  player, so it ends at the first step that shows him alive (a respawn, a round or match restart, a
  rollback that undoes the death) or dead of another death, and when the game unloads the match
  (`game.clear_resources`: a leave to the lobby). Nothing is set that must be put back but the
  slider's share, which the next frame gives again.

## Others

- **Late joiners:** a game that joined after the shot's input never plays it (my reading); a dead
  player who rejoins is not dark again.
- **Replays and live watch** (my reading): the input is in the stream, so a replay or a watcher
  plays the sound at its corpse; no one's screen darkens (no local player).
- **A modified game** can tell a shot into any corpse: a sound and a dark screen until that
  player's respawn, and nothing else.

## Release

- The exe (patch 196), the hud movie (`root.set_dead_black`), the pack `death-sound-021d@1` (its
  row in `release/content.tsv`), and the server (its refusal of a victim of no place; an older
  server relays one).
- **No `simulation_changes.tsv` row:** patch 78's filter takes the marker and the quick bits out of
  every input before the game's handling, as for a pickup, and nothing is written to the world;
  replays of older games carry no such pattern.

## Not run

- Nothing has run in a game: the shot's age, the input, the sound and its reach, the black view and
  the cut are read, not seen. Checked: the transition's test (`cargo test -p patches corpse_shot_dark`).
