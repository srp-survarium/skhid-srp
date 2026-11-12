# A replay that plays one moment by itself (patch 143)

For reviewing a moment of a match as a video: the game plays a `.sur` replay from one time to
another, from one player's first person, with nothing pressed, and quits at the end, so a
script can record its window (the render script is local, beside the recordings).

- **When:** the game starts with `-match_replay=<file>` and the environment variable

  ```text
  SKHID_REPLAY_DIRECTOR=<place>,<from ms>,<to ms>
  ```

  The place is the roster's (0 to 19, the recording's notes name them), the times the match's
  ms as the world, the recordings and the replay's clock count them (a `.sur` starts at 50).
  Not command-line keys: the engine refuses a key it has not registered ("Command line argument
  ... has not been registered", a message box before anything loads).
- **What:** at the replay's first frame the game seeks to `from` as the replay player's slider
  does, and from then on, every frame:
  - the place's player is followed in the first person (`select_player(place, 0)`), again
    whenever the camera is not on it: after a seek, a death and a respawn, or a leave and a
    return under another place of the same name (the roster's name);
  - the replay's controls (F11's replay player movie) and the hud's cursor are hidden; the
    followed player's hud stays.
  - At `to` the game quits, as the replay's own close does (`game::exit` with `quit`). It quits
    too when `from` is past the replay's end, and when the clock goes back before `from` (at its
    end the game sends a replay back to its start and pauses).
- **The log** (`srp_patches.log`, beside the exe), a line each: the variable read
  (`follow place P from F to T`, or `not understood`), `seeked to`, `watching from` (the clock
  reached `from`), each `follow place`, `quit at`. A capture times its cut by the `watching` and
  `quit` lines.
- **After a seek the clock steps back** about 1.5 s (55 530 after a seek to 57 000, seen) while the
  world was read to the target, so for that moment nothing moves; the window begins when the
  clock is at `from` again (`watching from`).
- **Live matches:** nothing. A game without the variable reads it once and stops; a live match's
  world has no replay (its network client is not the replay's, and only `-match_replay` makes the
  game's replay reader), so the hook goes no further.

## How

Exe patch 143 (Rust `replay_director`, `vostok::replay`): one lohook at `game_world::tick` after
the simulation's move (0x1F7C3C, the chain's order 4, after replay_keys'), where a seek is the
movie's own (`docs/notes/replay-keys.md`: never from a key mid-frame).

- The seek: `game::replay_seek_to` (0x1EE2E0; ESI the game, the target ms from the replay's start,
  0 a user seek), which the movie's `update_position` calls (0x233E2F). The replay's first and last
  times are its reader's (`[game+53C8h]+688h`, `+68Ch`), the clock the game's (`+5854h`).
- The follow: `base_local_network_client::follow_place_first_person` (0x26DF70, the replay client's
  slot +7Ch, what `select_player` with view 0 calls, 0x233EFC); the camera is on the client's
  local player (+4), which it sets; a place's player is `get_active_player`'s.
- The controls: `game_world_ui::show_replay_player` (0x23A9D0, +2DDh) and `show_cursor`
  (0x23A930, +2DCh) with 0, what F11 toggles in a replay (0x25CEF5, 0x25CF08).
- The quit: `game::exit` (0x1EC730, "quit"), as `replay_network_client::close_current_match`
  (0x2193D6) and the `quit` console command.

Nothing a match computes; no simulation change. Played 2026-11-12 under Wine on a headless Xorg
display (rev56 build with this patch, match 0x59bd5666, place 0, 57 000 to 122 000 ms). Windows
not run.
