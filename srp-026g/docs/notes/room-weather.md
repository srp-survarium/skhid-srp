# The Room's weather

The owner (2026-11-02): "room should actually be secretely 3 rooms": day, rain and night; "night
by real time, rain sometimes like 15% of enters after match/login"; the night "starts at night for
players"; "it shouldn't happen when hopping and choosing different lobbies, so it is a surprise";
"if it is raining, it should rain for 3 more enters", then "roll again". 2026-11-27: "i would
add evening too"; and for the night "we shouldn't change pose in night, instead we should add
moonlight and moon somewhere there so it shines on the player" (the characters and the camera
stand as by day in every weather; the night's project turned nothing since). Exe patch 81
(`client-patches/rust/patches/src/lobby_scene.rs`); the switching is lobby-scenes.md's.

- **Four projects, one lobby:** `lobby_scene/camp_069` (day, as before), `lobby_scene/camp_069_rain`,
  `lobby_scene/camp_069_evening`, `lobby_scene/camp_069_night` (the camp pack's). Only the Room
  has a weather; the other lobbies are as they were.
- **Secret:** the lobby window's card stays "Room" (id 1), its picture as before; user.cfg's
  `srp_lobby`, srp's `3B` and `0xA1` know only the lobby. The weather is the client's alone:
  each client decides its own (squad mates may see different weather).
- **When it is decided: an entry to the lobby**, and nowhere else:
  - the game's start (the lobby menu's first query, before the sign-in);
  - each return from a match (the lobby shown again after it was hidden).
  A pick in the window, a squad joined or left, a commander's pick: no entry, nothing decided or
  rolled; the Room picked again shows the weather of the entry. Hopping Room, Bunker, Room shows
  the same Room.
- **The clock first:** the player's local time (`clock::local_day_minute`) gives the entry's
  `Hours`: the evening from 18:00 up to 21:00, the night from 21:00 up to 06:00, else the day
  (`DAY_FROM`, `EVENING_FROM`, `NIGHT_FROM`, the one place to move them). The evening and the
  night win over the rain: four rooms, no rainy evening or night. (The evening's hours and "rain
  by day alone" are the first build's picks, 2026-11-27, the owner's to change.)
- **Rain, a spell:** out of a spell an entry rolls the dice (xorshift64, seeded once by the
  performance counter): under 15 in 100 (`RAIN_PERCENT`) it rains and a spell starts; the next
  3 entries (`SPELL_ENTRIES`) rain too, rolling nothing; the entry after them rolls again (no dry
  entry kept between spells). About 4 entries in 10 rain over time.
- **The spell survives a restart:** user.cfg's `srp_room_rain <entries left>` (0 none, 1 to 3),
  written at once when an entry changes it.
- **A spell's entry in the evening or at night** shows the evening or the night, and the spell
  counts it. A roll then still rolls: a rain rolled starts a spell that the next day's entries
  show.
- **The spell counts every entry**, whatever lobby is picked (an entry in the Bunker uses one).
- **The start** asks the decided Room's project directly (as the saved pick's is asked).
- **A return into another weather** (the Room shown): the black and the loading spinner are on
  from the return's first frame, the new Room is loaded behind the black, swapped in and faded
  in, as a pick's change (lobby-scenes.md); the old weather is never seen again. Into the same
  weather: nothing. A weapon's upgrade window open at the return: the change waits for it, as a
  pick's.
- **The night beginning (or ending) while sitting in the lobby:** nothing until the next entry
  (no change in sight, no flicker).
- **Missing projects:** the camp pack with the three variants must ship with (or before) this
  exe: a missing project crashes the project's cook (lobby-scenes.md, "At start").
- **Looking at one weather:** the environment's `SKHID_ROOM_WEATHER=day|rain|evening|night`
  (read at the first entry; `run.py` passes its own environment to the clients) makes every
  entry that weather: no clock read, no roll, the spell as it was. Unset, as players have it, or
  any other text: nothing, one line of the log when it is on. In a diagnostics build
  (`--ragdoll-trace`) each entry's weather is a line of `srp_lobby_scene.txt` (`entry: rain`),
  and the camera sweep's line `return <weather>` plays a return's events into that weather
  without a match.
- **Checks:** `lobby_scene.rs`'s tests (the day's hours, the spell, the dice's 15 %, no entry at
  a pick or a squad, a return into another weather behind the black, every sequence of events
  with entries of changing weather); `check_lobby_scene_026g.py` (the start at noon, in the
  evening, at night, in a spell, a rain rolled, the variable saved; `SKHID_ROOM_WEATHER`; a
  return at night: the black at once, the night's project asked alone; picks between entries
  read no clock).
