# Skhid 0.26g — what changed

Changes for players of the Skhid server (026g.skhid.link), newest first.
All times are UTC. A time is when the change reached the players' server or
their download, taken from the server's own records.

Where a rule was changed more than once, it is written down once, at the
release that made it what it is now.

---

## 18 Oct, 14:43 — Game update 21: weapon balance, magazine on the weapon card, crash reports as ZIPs (update required)

Start `Skhid.bat`: the launcher updates the game (`survarium_skhid.exe`
revision 21, a new `bugtrap.dll`) and downloads the new game content,
`resources.db.patch_21`, one file of 44 MB (43,910,935 bytes, sha256
`0db1c4bcb6f3257d56db8da93e9b918878de39d140ef6eb3bb2e75ce785a0de3`). A game
older than revision 21 can no longer sign in. Replays recorded by games older
than revision 21 can no longer be watched: this update changes what a match
computes, and the game says so when you try.

The values were chosen from the matches played since update 19 (about 10,000
kills by guns): each victim's real armour, the killer's ammunition and the
distance, through the game's own damage rule (damage × (1 − (armour − armour
penetration)), never below 0; both damage and penetration fall to 60 % between
a weapon's two damage distances). The weapon card shows damage and armour
penetration ×100.

- **Armour penetration of the weapons from 0.29, on this game's scale.** They
  came with this game's damage but 0.29's penetration, which is on another
  scale for each calibre (`bullet_pierce`):
  - 5.56 mm, too high: HK416 0.45 → 0.34; M4A1 and MSBS Radon 0.42 → 0.32;
    FN FNC 0.40 → 0.30, and its damage 0.45 → 0.50 so it plays where it played.
  - 4.6 mm and 5.7 mm, too high: MP7, MP7A1, MP7A1 "Legend" 0.55 → 0.30, and
    their damage 0.35 → 0.40; FN P90 0.55 → 0.30.
  - .45 ACP: KRISS Vector 0.45 → 0.25, and its damage 0.45 → 0.42 (its
    players got 1.22 times the kills with it that they got with their other
    weapons).
  - 7.62×54R: PKP "Pecheneg" 0.60 → 0.45.
  - 7.62×39, too low: vz. 58, Valmet, SOK-94 0.30 → 0.40, the AKM's.
  - 9×18, too low: PP-91 "Kedr", AEK-919K 0.10 → 0.20, the Kiparis's.

  Armour penetration also decides what a bullet goes through, so these
  weapons now go through walls as their calibre does.
- **Weapons that won too often** (how many kills a weapon gets against what
  the same players get with their other weapons):
  - SR-2M "Veresk": rate of fire 900 → 800 (`rounds_per_minute`; 1.33 times
    its players' other weapons, the most accurate automatic).
  - SR-3M "Vikhr" and its premium copy: rate of fire 900 → 800, damage
    0.55 → 0.45 (the fastest-killing automatic).
  - AS Val, 9A-91, VSS "Vintorez", VSK-94 and their premium copies: damage
    0.55 → 0.50 (the same 9×39 as the SR-3M).
  - PP-2000: rate of fire 750 → 680 (1.22 times).
  - SVD: magazine 10 → 5 rounds (`magazine_capacity`); damage and rate of fire
    as they were. Its description says 5 rounds in every language.
- **The beginner rifles,** which played weakest (0.54 and 0.66 times their
  players' other weapons): Mosin-Nagant damage 1.0 → 1.1 and armour penetration
  0.35 → 0.40; TOZ-122 damage 1.1 → 1.2.
- **Shop:** the Mosin-Nagant and the PPSh-41 (the starting kit's two weapons,
  which the game prices at 0) are sold at every faction from rank 0 for
  1,000 silver. New accounts still get both.
- **Stamina regeneration bonuses now work.** Every item config and the
  modifications named the bonus `stamina_regenation_speed_modifier`; the game
  reads only `stamina_regeneration_speed_modifier`, so the items' stamina
  regeneration bonuses, the five stamina regeneration modifications and the
  defect did nothing. Renamed in 216 item configs and the 6 modifications.
- **Red-dot and ring sights less bright.** The sights from 0.51 (Aimpoint
  CompM4, Cobra EKP-8, Docter, EOTech 552, Leupold DeltaPoint) glowed 3.3 times
  as bright as in 0.51: their three glowing-dot materials lost 0.51's
  `constant_transparency` 0.3 in the port and had 1. Back to 0.3. The 0.29
  stock sights are as they were.
- **Magazine on the weapon card.** Every weapon's card (83) has a line
  "Magazine" / "Магазин" / "Magazin" / "Magazynek" after the rate of fire,
  with the rounds the weapon's magazine holds, compared like the other lines.
  It is line 134 of the card lines (`gameplay/static_game_parameters.options`,
  `st_inv_magazine_size`), and the lobby movie's card has a ninth row for it
  (`lobby_menu.swf`: sprite 1538 `diff_9`, `ItemCompare`'s constructor).
- **Every changed value's line on the weapon card** shows the new value.
- **Crash reports:**
  - The crash reporter zips the dump itself (`SRP-crash-…-N.zip`, beside the
    game or in the Windows temporary folder; the `.dmp` is kept when zipping
    fails) and puts the newest game, launcher and updater logs of the last
    48 hours in it (`logs/`, with `logs/collection.txt` saying what was taken).
  - Lines of those logs that carry a password, the account digest, a token, a
    key, a cookie or a command line are taken out of the copies in the ZIP
    (the files on disk are not changed). The dump itself is not filtered.
  - The crash window is in the language of Windows (English, Ukrainian,
    Russian, Polish, German) and says where to send the file (#bugreports).

## 16 Oct, 08:27 — Game update 12: Duty exoskeleton, bandit balaclava (update required)

Start `Skhid.bat`: the launcher updates the game and then downloads the new
game content, one file of 86 MB. PLAY turns on once it is in place and
checked. An older game can no longer sign in.

- **New armour: Duty exoskeleton.** Level 15, at Renaissance from rank 10
  (Colonel), 300,000 silver. Body armour 0.87, head 0.72, isolation 0.70,
  weight 35 kg. Takes the usual exoskeleton modifications. It is not found in
  matches.
- **New mask: bandit balaclava.** Level 8, at the Black Market from rank 5,
  10,000 silver. Armour 30, isolation 15, weight 1 kg. It can also drop in
  matches.
- **Starting a match on a slow connection:** a player who loads late now gets
  the other players' entries together with the world. Before, that player
  could go out of step about 3 seconds into the match.
- **Crash reports** now say when an old `shell_32.dll` left in the Windows
  temporary folder caused the crash.
- **Game content now comes as separate downloads.** Later content (items,
  maps) comes the same way: the launcher fetches only what is new. Your
  `resources.db` is put back to the original.

## 15 Oct, 20:02 — Fewer and shorter desyncs, outdated games stopped at sign-in

- **An outdated game is stopped at sign-in** with the game's own "You're
  using an outdated version of the game." message, before it reaches the
  lobby. Start `Skhid.bat`: the launcher updates the game. Games older than
  update 11 are refused.
- **Fewer desyncs:** a player who comes back into a match they left no longer
  goes out of step right after, and a game whose fix did not take is put back
  again at once more reliably.
- **Shorter desyncs:** a player who goes out of step again a little while
  after a fix that worked is put back in about 5 seconds instead of up to 15.

## 15 Oct, 15:39 — Game update 11: FOV setting, exchange both ways, Team Deathmatch end, batteries under the floor (update required)

Start `Skhid.bat` and the launcher updates the game by itself. An older game
can no longer play.

- **Field of view:** a new "FOV:" slider under gamma in Settings → Graphics,
  from 50 to 100 (the game's view was fixed at 65). Iron sights stay close to
  your setting; a full scope looks exactly as before at any setting.
- **Team Deathmatch ends when a team is out.** Before, one dead player could
  stay counted as alive and the match ran on to its time limit.
- **A battery that fell under the floor** can now be picked up: stand right
  above it, look down and press use.
- **Exchange silver and gold both ways in the game:** the gold and silver
  boxes at the top of the lobby open an exchange window. Type how much to pay
  or receive and swap the direction with the arrow button, at 100 silver = 1
  gold. The exchange on the website still works too.
- **Fewer desyncs:** a game that went out of step and did not take the fix is
  put back again, and a move that reaches the server a little late is still
  passed on to the others while their games can take it.
- **Leaving a match:** when a match you left ends, you are no longer sent to
  the next match of the players still in it.
- **Launcher:** Enter presses PLAY.
- **"Matches" window:**
  - Each match shows its level.
  - When JOIN is off, it says why: you are searching, in a squad or in a match, the match has not started, it ends soon, the teams are already even, and so on.
- **Before a maintenance:** in the last 30 minutes, when Play is off, JOIN
  still works for a match that ends before the maintenance. You go onto the
  weaker side at once, even if that makes the sides uneven.
- **Website:** shorter steps on the home page, and a link to the project's
  Discord.

## 14 Oct, 21:26 — Fewer desyncs (update required)

Start `Skhid.bat` and the launcher updates the game by itself. An older game
can no longer play.

- **Back in step faster.** A player who went out of step is put back in one
  go; before, the game often fell out of step again right away, over and over
  for 15 seconds or more. This was the most common cause of lag in the
  evening matches of 14 Oct.
- **Fewer lost moves.** The server packs the other players' moves together
  and sends again what a weak connection lost, so big matches lag less.
- **Respawning into another kit** no longer puts a game out of step. That
  kit's weapons now start on their first fire mode and first ammunition type
  again; nothing you own changes.
- **Joining a running match** works more often: a joiner no longer respawns
  someone in the wrong kit, and one member of a group who is slow or leaves no
  longer keeps the others out.
- **Scoreboard:** a player who joined late now counts right on the Tab
  scoreboard, and the team points show during the countdown.

## 14 Oct, 05:36 — Match recordings

- Matches that had 6 or more players (late arrivals count) are now recorded
  on the server, so that lag and desync problems can be looked into afterwards.
  The recording holds the match's game traffic, not chat and not addresses,
  and stays on the server. A match that never reached 6 players is deleted
  when it ends.

## 14 Oct, 04:50 and 04:52 — "Matches" window with JOIN, login hint

These two come through the launcher: close the game, start `Skhid.bat`, and it
changes the game's lobby and login screens by itself (the original screens are
kept and can be put back).

- **New "Matches" window in the lobby** (a tab after the chat tabs): the matches
  being played right now, with map, mode, how many players each side has, the
  score (batteries or tickets) and how long it has been played. A match that has
  its result is not listed.
- **JOIN button** in that window, one click, no waiting. It is on for you in a
  match only when all of this is true:
  - the match takes players now: it is being played, has more than 2 minutes
    left, and has a free place on a side (a match takes one newcomer at a time,
    so for a few seconds after somebody joins, others see no JOIN on it);
  - you would be put into it at once and it would make the teams **more even by
    value** (gear, level and your hidden rating, yours counted), not just by
    head count: a match whose teams already count alike takes nobody, even at
    3 v 4;
  - you are in the lobby, not searching, not in a match and not in a squad, you
    were never in that match, and your game is up to date.
- When you press JOIN you are put into the match at once, on the side that
  makes the teams more even. The place is held for you for 10 seconds. If the
  match stops taking players before you are in (the teams became even, somebody
  took the place, it is nearly over), the game shows "Error changing readiness
  status" and you are not put in the search.
- **Login screen** no longer shows the original game's registration link; it
  has a short hint on how to get an account here.

## 14 Oct, 03:48 — Game update 8: lag

- After a start, a late join or a resync, a game's clock is behind the match.
  The game used to catch up very slowly, and the server throws away keys that
  come more than a quarter of a second late, so a far-away player's controls
  could do nothing for 6 to 15 seconds. The game now catches up about six
  times faster (a clock 465 ms behind is made up in about 1.6 s), and the
  server measures that lag more exactly.
- Update 8 plays together with update 7, so it is optional.

## 14 Oct, 02:26 — Game update 7: grenades for late arrivals, even teams

- **Players with grenades or traps in a kit can now join a match in progress.**
  Before, such a player was never let in (it failed four times out of four).
- **Teams are made even by value**: each player counts for the gear they can
  play with, their account level and a hidden rating built from how earlier
  matches went. Nobody sees it; it is not the rating in the ranking. With an
  odd number of players the stronger players make the side with one fewer.
  A match with fewer than 4 players moves nobody's hidden rating (such a match
  says little about how teams should be made).

## 14 Oct, 00:44 — Game update 6: three faults when someone joins a match

- A player who joins a running match now appears on the scoreboard and in the
  other lists of players (before, the lists were made once, for the players
  who were there at the start).
- Fixed two crashes of games that were given a newcomer.

## 13 Oct, 23:27 — Game update 5: join running matches, new economy

- **Matches no longer wait for a full lobby.** A match starts with 2 players,
  2 minutes after the second one pressed Play (20 players start at once). It
  takes new players while it is being played, until 2 minutes before its end:
  they go to the side that makes the teams more even. A match holds 20 players,
  10 a side; a place of a player who left is not given again. A late arrival
  is paid at the match's level, which stays what it was at the start.
- **Silver, experience and reputation**:
  - silver of a match is 150 % of the original game's at match levels 0 to 5,
    then 160, 170, 180, 190, 200, 210 and 220 % from level 12;
  - experience of a match is paid twice;
  - reputation of a match and of its quests is 120 %.
  The match's level is the mean of its players' gear levels (the first players
  put into it), so a match that began small is paid at a low level whoever
  joins later.
- **Ammo costs about half the original price** and **needs no reputation rank**
  (in the shop, in the kit's ammo window and in the auto-buy after a match).
  Grenades, traps and drugs keep their ranks.
- **Quest goals count what the in-match quest counter counts**: "Find and bring
  batteries" counts a battery you picked up first (or took from the enemy) and
  put into the container, if no teammate carried it in between; "Steal a battery" counts the one you took and put in; "Win a match
  without losing batteries" can be done only in the battery modes; a kills/
  deaths ratio goal is met at its number.
- **A game whose window is not in front keeps running.** Others no longer see
  you frozen when you alt-tab, and a join is not held up by a game that says
  nothing.
- **Players on bad connections**: the world of a match is now sent in smaller
  steps, aimed at the "free camera, no character" problem in which one player
  loaded the level and never entered. Not yet seen to be fixed.
- Launcher page: a second tab, **"Matches now"**, with the matches being played.

## 13 Oct, 18:38 — Economy, squads off

- **Weapons and armour no longer wear down** in matches (no new defects). All
  worn weapons and armour were mended once.
- **No play-time bonus** to the chance of a drop at the end of a match.
- **Squads are switched off**: an invite is refused with "Squads are switched
  off on this server" and everybody queues alone. Squads were in more than half
  of the matches and kept teams from being made even.
- Launcher page: the numbers (players online, queue, matches, maintenance) now
  refresh by themselves every 10 seconds.

## 13 Oct, 16:27 — Updater

- When the first installation of the game fails, the updater writes what the
  unpacker said into `updater_skhid.log`, so the reason can be found (a player
  had "exit 2" with no explanation).

## 13 Oct, 15:36 — Game update 4: Documents folder

- The game starts when Windows does not let it write to its folder in
  Documents (the cause of two start-up crashes). It then runs with settings
  that are not saved, and a message at the start says which folder and how to
  allow the game in Windows' "Controlled folder access".

## 13 Oct, 12:30 — The launcher installs the game

- A new player no longer needs the original game first: unpack our files into
  an empty folder, start `Skhid.bat`, and the launcher downloads Survarium 0.26g
  (2.9 GB, from Google Drive), checks it and installs it. The unpacker comes
  in our ZIP. Players who have the game already are not affected.
- **Game update 3 (download of 12:22):** the last battery could not be put into
  its container once a team had nine, which ended the match only on the time
  limit and stopped stealing. The containers stay usable now. (Since then a battery match
  has ended by its rule and not on the time limit: 13 Oct, 19:01.)

## 13 Oct, 10:30 — Launcher page

- The launcher's page has the game's original look again, and a link to Discord.

## 13 Oct, 09:48 (server) and 09:50 (download) — The launcher works again

- The game's own launcher starts again with our files: its page is ours (top 10,
  players online, queue, matches, accounts), it no longer asks any Vostok Games
  address, and it **updates your Skhid files by itself** from the download on
  the site each time it starts, including its own updater.
- A search for a match **restarts when your equipment level changes**.
- Quests that need grenades or artefacts are offered only once the store that
  sells them is unlocked.
- Fixed a crash after sign-in: a game that had not loaded any match was sent a
  results screen.

## 12 Oct, 22:18 — Search chat lines

- A line in the general chat when somebody starts or stops searching, and
  when a match starts, with whom. Everybody listening to the general chat sees
  it; the join line is shown once per 30 seconds per player.

## 12 Oct, 21:10 and 21:20 — New download

- The download opens the server `skhid.link` by itself (no address to type).
- Our own **crash report window** in the style of the original; "Submit bug"
  opens our Discord.
- The Linux (Wine) launcher is in the ZIP. The launchers and texts are English
  by default (21:20).

## 12 Oct, 19:16 — Leaving a match, camouflage

- **Leaving a match from the menu really leaves it.** Before, Play sent you
  straight back to the same match (a 1 v 1 reopened over and over). A game that
  crashed or lost its link still gets its place kept.
- Moving a camouflage between kits refreshes the kit.

## 12 Oct, 18:43 — New download

- The game says which version of our patches it has when it signs in, so the
  server knows who has updated.

## 12 Oct, 14:38 — Crash of every game in a match

- A match no longer crashes every game at one moment: a respawn with a kit that
  had no weapon (empty second kits and the premium kit of every premium
  account could be chosen in a match). Such kits can no longer be chosen.
- A match whose games have all stopped now ends once a player has waited about
  20 seconds for the world, instead of holding its players for 9 minutes (it
  had sent them back to the dead match each time they pressed Play).

## 12 Oct, 13:32 — Update

- The results screen you have not seen yet is kept across a server restart.
- A player who comes back into a Team Deathmatch in progress no longer gets
  wrong ticket counts.
- A game that is still connecting is waited for (3 seconds) instead of being
  told to disconnect when its first packet is lost.
- Impossible mouse turns sent by one game are no longer passed to the others
  (one bad input used to crash every game).

---

## Good to know

- The in-match quest window and the "Mission accomplished" card use the game's
  own numbers, so they do not show the 120 % reputation; the results screen,
  reputation bar and rank do.
- The kit's ammo window shows the original price for ammo you own; the shop
  price is the one the server uses.

## 17 Oct, 12:30 — TODO: the owner's words (update required)

<!-- Revision 17 (026g-rev17), released 2026-10-17 12:30 UTC. To be worded for players; what it shipped: -->

- TODO: #143 AGENTS.md: releases set the revision and tag; where the game's symbols are
- TODO: #141 release.sh: the go first, the maintenance at once, the build during the countdown; --now
- TODO: #142 0.26g: every map from the game data, nothing by level
- TODO: #144 Launcher without SHOP, site without the silver and gold exchange
- TODO: #145 release.sh: each release sets its revision (live + 1) and records and tags itself
- TODO: #148 The third kit for every account, premium or not
- TODO: #149 One cosmetic worn by many items (exe patch 30: taking it off names the item)
- TODO: #151 Replays of older games: refused, said why; a release raises the oldest replayable revision
- TODO: #150 Patch 31: a kit chosen before the match starts is played at once
- TODO: #152 Patch 31: the switch looks only at the match's places and always gives the new kit its protection

## 17 Oct, 15:38 — TODO: the owner's words (update required)

<!-- Revision 18 (026g-rev18), released 2026-10-17 15:38 UTC. To be worded for players; what it shipped: -->

- TODO: #157 Revision 17's SVD: 10 damage points less, checked on the candidate data
- TODO: #156 026g client patch 33: the archive reads' order is a strict weak ordering
- TODO: #160 026g client patch 35: a baked camouflage texture missing from the baker's list is baked again instead of stopping the game
- TODO: #161 patches.tsv: patch_17's Google Drive mirror
- TODO: #163 Shop: the factions' camouflages, the Vityaz and the Lancaster are sold
- TODO: #162 Client patch 36: the memory arena "ai" is 4 MiB instead of 256 KiB

## 17 Oct, 18:12 — TODO: the owner's words (update required)

<!-- Revision 19 (026g-rev19), released 2026-10-17 18:12 UTC. To be worded for players; what it shipped: -->

- TODO: #168 Release of weapon scopes and the 0.29a3 weapons: #153 + #155 + #159 on main

## 18 Oct, 02:10 — TODO: the owner's words (update required)

<!-- Revision 20 (026g-rev20), released 2026-10-18 02:10 UTC. To be worded for players; what it shipped: -->

- TODO: #171 Squads of two: SRP_SQUAD_SIZE, an invite into a full squad is refused
- TODO: #172 Revert "Squads of two" (#171)
- TODO: #175 0.26g matches: 8 a side by default, not 10

## 18 Oct, 21:55 — TODO: the owner's words (update required)

<!-- Revision 22 (026g-rev22), released 2026-10-18 21:55 UTC. To be worded for players; what it shipped: -->

- TODO: #184 Changelog: game update 21
- TODO: #185 Shop: stop selling the Russian and Belarusian flags and coats of arms, the USSR flag and the red star
- TODO: #187 Admin page: reset a forgotten password while srp runs
- TODO: #186 Site: damage calculator tab (/damage), English and Ukrainian
- TODO: #189 Password reset: close the sign-in race, write only the account, report errors

## 19 Oct, 10:52 — TODO: the owner's words (update required)

<!-- Revision 23 (026g-rev23), released 2026-10-19 10:52 UTC. To be worded for players; what it shipped: -->

- TODO: #191 Site in the calculator's look; calculator: healing table, play-area map, aligned bonuses
- TODO: #192 Damage calculator: armour picker by icon and level, item protection by its bonuses
- TODO: #193 Website: Maps tab (spawns, batteries, chargers, containers, artefacts)
- TODO: #194 Reroll offers three on items of few types; backpacks may roll health regeneration
- TODO: #188 Patch 31: before the start a drag turns the camera and the wheel moves it, as in the lobby
- TODO: #180 Match native game memory defaults to launcher budgets
- TODO: #178 Fix launcher crash when an install starts with stale status information
- TODO: #196 Shop: sell the STK2 Novice's knit cap to everyone
- TODO: #197 Site: Maps tab and calculator built with revision 23's patch

## 19 Oct, 18:03 — TODO: the owner's words (update required)

<!-- Revision 24 (026g-rev24), released 2026-10-19 18:03 UTC. To be worded for players; what it shipped: -->

- TODO: #204 Patch list: Google Drive mirrors of patches 19 to 23
- TODO: #202 No tutorial: new accounts start with 10k silver, 500 gold, the Scavengers and ammo
- TODO: #203 Rooms: every PvP match is a room's (the in-game Rooms window); the old queue goes
- TODO: #206 Patch 39: the Tab list shows the places in the match, none whose player left
- TODO: #207 Site: Maps tab and calculator built with revision 24's patch

## 20 Oct, 08:36 — TODO: the owner's words (update required)

<!-- Revision 25 (026g-rev25), released 2026-10-20 08:36 UTC. To be worded for players; what it shipped: -->

- TODO: #213 Lobby: every Get Gold opens the exchange, not the online store
- TODO: #214 Exchange: a typed amount over the balance becomes the most it pays for
- TODO: #215 Lobby: Buy on an owned item: modified ones too, no "already purchased" question
- TODO: #216 Lobby: craft window: right-click Buy and Sell; Continue and Exit after an upgrade
- TODO: #217 Lobby: Optics in a weapon's right-click menu (opens the scope editor)
- TODO: #201 Patch 38: spotting has no cooldown, and a battery's body spots it (simulation change)
- TODO: #200 Fall damage as in 0.27: fatal_damage 1.0 (simulation change)
- TODO: #205 Patch 40: a model's LOD and screen factor at a wider field of view are the original's
- TODO: #212 Rooms: the map vote runs until the match starts
- TODO: #218 Lobby movie: one driver builds every lobby change, in order
- TODO: #219 Rooms: the room's window under a match's results
- TODO: #210 Late joiners: fewer players first, then the side they played on, then the balance
- TODO: #211 Rooms: a member sees it is in a room (yellow tab, red Leave room)
- TODO: #222 Ukrainian: the Duty faction is Долг
- TODO: #209 Late joins: a failed joiner closes nothing, and no joiner waits for a game that is not playing
- TODO: #221 Battery pick-up 20 % faster (take_time 0.6 -> 0.5)
- TODO: #199 Release: rebuild the Maps tab and the calculator from the release's patches
- TODO: #224 re-kit 0.26g: weapon draw/holster, the modifier getter, the battery's states
