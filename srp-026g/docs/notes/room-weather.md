# The Room's weather

The owner (2026-11-02): "room should actually be secretely 3 rooms": day, rain and night; "night
by real time, rain sometimes like 15% of enters after match/login"; the night "starts at night for
players"; "it shouldn't happen when hopping and choosing different lobbies, so it is a surprise";
"if it is raining, it should rain for 3 more enters", then "roll again". Exe patch 81
(`client-patches/rust/patches/src/lobby_scene.rs`); the switching is lobby-scenes.md's.

- **Three projects, one lobby:** `lobby_scene/camp_069` (day, as before), `lobby_scene/camp_069_rain`,
  `lobby_scene/camp_069_night` (the camp pack's). Only the Room has a weather; the Basement, the
  Bunker and the Hangar are as they were.
- **Secret:** the lobby window's card stays "Room" (id 1), its picture as before; user.cfg's
  `srp_lobby`, srp's `3B` and `0xA1` know only the lobby. The weather is the client's alone:
  each client decides its own (squad mates may see different weather).
- **When it is decided: an entry to the lobby**, and nowhere else:
  - the game's start (the lobby menu's first query, before the sign-in);
  - each return from a match (the lobby shown again after it was hidden).
  A pick in the window, a squad joined or left, a commander's pick: no entry, nothing decided or
  rolled; the Room picked again shows the weather of the entry. Hopping Room, Bunker, Room shows
  the same Room.
- **Night:** the player's local time (GetLocalTime) from 21:00 up to 06:00 (`NIGHT`, one const).
  The night wins over the rain: three rooms, no rainy night.
- **The night faces the chandelier** (the owner, 2026-11-02): the night's room keeps the day's
  layout; its project's root `srp_lobby.facing = pi` (pack lobby-camp-069@19) turns the lobby's
  three characters about the origin by it (the squad's spots to (-1, 0, -0.5) and (1, 0, -0.5),
  every yaw pi + facing) and the camera's stock pose (yaw pi: the eye at (0, 1.2, 1.96), the
  chandelier's side). Any lobby project may set `facing` (radians); without it, 0: as before.
  - read from the project each time one is put in (the start's, a swap's), as patch 45 reads
    `srp_lobby.weapon`; the weapon's spot stays in room coordinates;
  - the characters: their own point and yaw (what a player made later takes:
    lobby_character::on_player_ready, a squad member joining) and, for a player already made,
    `base_player::set_transform` (0x2DAB70), at the swap behind the black or at the start;
  - the camera: at the new lobby's first frame (the start's, the first frame behind the black, or
    a return's first frame after a swap made during the match); a night to day change puts it
    back at yaw 0. Not while a weapon's upgrade window has the camera (no swap happens then).
- **Rain, a spell:** out of a spell an entry rolls the dice (xorshift64, seeded once by the
  performance counter): under 15 in 100 (`RAIN_PERCENT`) it rains and a spell starts; the next
  3 entries (`SPELL_ENTRIES`) rain too, rolling nothing; the entry after them rolls again (no dry
  entry kept between spells). About 4 entries in 10 rain over time.
- **The spell survives a restart:** user.cfg's `srp_room_rain <entries left>` (0 none, 1 to 3),
  written at once when an entry changes it.
- **A spell's entry at night** shows the night, and the spell counts it. A roll at night still
  rolls: a rain rolled then starts a spell that the morning's entries show.
- **The spell counts every entry**, whatever lobby is picked (an entry in the Bunker uses one).
- **The start** asks the decided Room's project directly (as the saved pick's is asked).
- **A return into another weather** (the Room shown): the black and the loading spinner are on
  from the return's first frame, the new Room is loaded behind the black, swapped in and faded
  in, as a pick's change (lobby-scenes.md); the old weather is never seen again. Into the same
  weather: nothing. A weapon's upgrade window open at the return: the change waits for it, as a
  pick's.
- **The night beginning (or ending) while sitting in the lobby:** nothing until the next entry
  (no change in sight, no flicker).
- **Missing projects:** the camp pack with the two variants must ship with (or before) this exe:
  a missing project crashes the project's cook (lobby-scenes.md, "At start").
- **Testing (a diagnostics build only, `--ragdoll-trace`; never a release's):** user.cfg's
  `srp_room_weather day|rain|night` forces every entry's weather (the spell untouched); the
  variable is registered in a diagnostics build alone, so a release's game ignores the line.
  Each entry's weather is a line of `srp_lobby_scene.txt` (`entry: rain`). The camera sweep's
  `return <weather>` plays a return's events (hidden, shown) into that weather without a match,
  and a pose `<hold> free` leaves the camera to the game.
- **Checks:** `lobby_scene.rs`'s tests (the night window, the spell, the dice's 15 %, no entry at
  a pick or a squad, a return into another weather behind the black, every sequence of events
  with entries of changing weather, the characters and the camera turned by the shown project's
  facing at every swap and first frame); `check_lobby_scene_026g.py` (the start at noon, at
  night, in a spell, a rain rolled, the variable saved; a return at night: the black at once, the
  night's project asked alone, put in with the characters turned and their players moved, the
  camera's yaw pi; facing absent: the exe's own stances and yaw 0; picks between entries read no
  clock).
- **In a game** (2026-11-02, headless, a diagnostics build forcing the night, pack
  lobby-camp-069@19 as a local test patch, the squad panels' dummies, the camera the game's own):
  the start asked camp_069_night, stood the three characters turned (they face the camera from
  the chandelier's side, lit by it) and put the camera at yaw pi; a sweep `return day` (the
  machine's return events, not a real match) blacked out at once, swapped the day in behind the
  black with the characters back at the exe's spots and the camera at yaw 0; `return night` turned
  them again. A real match's return was not played.
