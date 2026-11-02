# Skhid 0.26g — what changed

Changes for players of the Skhid server (026g.skhid.link), newest first.
All times are UTC. A time is when the change reached the players' server or
their download, taken from the server's own records.

Where a rule was changed more than once, it is written down once, at the
release that made it what it is now.

---

## 28 Oct, 18:37 — Game update 37: matchmaking, Research warnings and map data (update required)

<!-- Revision 37 (026g-rev37), released 2026-10-28 18:37 UTC. Shipped: #691, #692, #463, #694, #695, #696. -->

Update through the Skhid launcher before playing.

- **Matchmaking:** public 8v8 rooms now open on demand, with equipment
  brackets and improved queue transitions. Manually created rooms are
  private. Blackout joins the normal mode pool.
- **Research:** enemy capture and neutralisation attempts now trigger
  pulsing warnings.
- **Kit selection:** a short grace period when the kit screen opens
  prevents a key pressed just before death from accidentally changing
  your kit.
- **Rewards:** farming limits now account for combat activity. Rewards
  against bots are capped at equipment level 7, and eligible finishers
  receive a minimum silver payout.
- **Replays:** solo matches against bots can now be recorded and watched
  from History.
- **Map data:** removed 163.5 MiB of unused vegetation editor data across
  four supported maps and both variants. All 172,493 plant placements
  remain unchanged.


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

## 20 Oct, 20:38 — TODO: the owner's words (update required)

<!-- Revision 26 (026g-rev26), released 2026-10-20 20:38 UTC. To be worded for players; what it shipped: -->

- TODO: #228 Clippy clean, and clippy in the dev process (AGENTS.md)
- TODO: #229 Rooms: the create window remembers the last room made by hand
- TODO: #236 Patch 43: the player's own row in the Tab list is gold
- TODO: #232 Rankings: right-click a player to see their equipment
- TODO: #233 Rooms list: the selected room's players below State; right-click for their equipment
- TODO: #234 Rooms: a match starts at the room's minimum, or is called off
- TODO: #237 selftest: the relocation-under-site checks include one starting 3 bytes before
- TODO: #240 Admin page: take silver, gold and items from an account, with a preview
- TODO: #243 Drop cards: one per player of a match, own matches only; log sales and exchanges
- TODO: #242 Weapon balance 2: AK-12 50 / 35 (premium: silver only), Socom16 damage 60
- TODO: #239 Battery events pay per match at most as many times as the map has batteries (anti-farming)
- TODO: #245 Rooms with a PIN: their matches pay no rewards

## 21 Oct, 13:10 — TODO: the owner's words (update required)

<!-- Revision 27 (026g-rev27), released 2026-10-21 13:10 UTC. To be worded for players; what it shipped: -->

- TODO: #248 Flash movies as committed ActionScript source (baseline: all eight trees rebuild exactly)
- TODO: #253 Rooms: room chat and Join room from a friend's menu (source tree)
- TODO: #247 Fall damage: how speed counts
- TODO: #254 Rooms: who came by Play leaves its room after the match it played (without squads)
- TODO: #255 Rooms: a restarted game learns its room before the room plays with it (source tree)
- TODO: #256 Replay tests: fixtures of today's revision, not 11-15
- TODO: #257 Rooms: Switch room as the game's normal button, with why to switch
- TODO: #259 Client programs: ordinary Windows binaries (std, static MSVC CRT, version info, icon)
- TODO: #258 Updater: PLAY only for a game the server lets in; release puts the ZIP in before srp starts
- TODO: #260 Weapon balance 2, retuned: AK-12 piercing 40, Socom16 damage 70
- TODO: #264 HUD at 4K: hit marks and crosshair centred at any UI scale
- TODO: #265 Weapon balance 2: the A-545 takes 0.28a's nerf
- TODO: #266 Grenade bounces louder: impact sound settings (content-edit grenade-bounce-1)
- TODO: #261 Results screen: drop card slot greys once used or refused; messages show over the screen
- TODO: #262 Lobby source: names for every placeholder, named numbers in every movie
- TODO: #263 Golden weapons: only the silver bonus, as the weapon's own modification; upgradable as the plain weapon
- TODO: #267 release/build-content: the next content patch from a manifest of what should be live
- TODO: #268 Reroll: keep the old modification (the price stays paid)
- TODO: #269 Ported masks: the Army balaclava (1823) and five 0.69d masks, one content patch
- TODO: #270 Patch 46: on the death screen a drag turns the camera; the wheel does nothing
- TODO: #271 Death poses 1: remove the face-down death pose that looks into the arms
- TODO: #272 Crouched steps audible at 40 % of walking
- TODO: #273 Weapon camo, the workbench, decals and instant preview (#244) on main for revision 27
- TODO: #274 Grenade bounce retuned: only the distance preset changes (mid_range)

## 22 Oct, 14:12 — TODO: the owner's words (update required)

<!-- Revision 28 (026g-rev28), released 2026-10-22 14:12 UTC. To be worded for players; what it shipped: -->

- TODO: #276 Rooms: the results stay open when Play's member leaves the room after a match
- TODO: #277 One pocket an item type (server rule, kit migration, lobby drop targets, 4th-grenade fix)
- TODO: #278 Battery modes: respawn 10 s
- TODO: #279 UI at 4K: Esc menu, Settings, cursor, replay controls and lobby pop-ups take the 1080p scale
- TODO: #281 v0.11 maps: spawn protection 6 s
- TODO: #280 Patch 49: the game stops at its start without a patch its release requires
- TODO: #283 Lobby: item search (always open, typing starts it, levels and kinds) and lobby keys (R, Enter, Esc)
- TODO: #290 Golden weapons: the shop and bundles show the silver bonus again
- TODO: #292 Rerolls: no wait between silver ones; no Upgrade at three modifications
- TODO: #291 Lobby: best bonus in gold, and the bonus range in the reroll window
- TODO: #284 Release overlay test: expect required_patches (after #280)
- TODO: #287 Content: 38 decals and the Steam camouflage from 0.69d
- TODO: #293 Premium armour: its bonuses rerollable modifications of its own, upgradable as the plain piece
- TODO: #294 Crafts: a golden item is a donor for its plain one too
- TODO: #295 Lobby: compare with the item being upgraded; premium bonuses in gold
- TODO: #296 Patch 52: a key in the lobby no longer kills the game when the focus is on a dead object
- TODO: #298 Lobby: craft list and compare colours; sight card's kit weapons in gold; character menu in the shop
- TODO: #299 Shop: Attachment on a kit item no longer crashes; its window lists the inventory
- TODO: #300 Rooms: browse All rooms from a room (My room / All rooms tabs)
- TODO: #301 Lobby: no New Year tree, gifts or Christmas sound; two bulbs light the workbench
- TODO: #302 Lobby: the camera turns again after the upgrade window closes

## 22 Oct, 14:57 — TODO: the owner's words (update required)

<!-- Revision 29 (026g-rev29), released 2026-10-22 14:57 UTC. To be worded for players; what it shipped: -->

- TODO: #304 Pockets: equipping a medkit, grenade or trap works again (rev 28 regression)

## 23 Oct, 04:36 — TODO: the owner's words (update required)

<!-- Revision 30 (026g-rev30), released 2026-10-23 04:36 UTC. To be worded for players; what it shipped: -->

- TODO: #297 Research (mode 5) as 0.27d shipped it: exe patch 53, 25 maps, hud, lobby, srp (for the release after revision 28)
- TODO: #307 Generic tooling: .srpx arena, call-shape models, content value-type check
- TODO: #310 Research: review fixes (slot 7 argument order, scoreboard, draw at the goal) and slots driven through the game's caller
- TODO: #309 Any modification of an item may be rerolled
- TODO: #306 Ban texts: no prohibited-software header, a neutral fallback reason
- TODO: #312 Lobby: the Attachments window opens again after it was closed once (rev 28 regression)
- TODO: #313 Patch 48: an upgrade window closed at once leaves the kit's weapon in the hands
- TODO: #308 Artefacts buffed: Spring's aim time and no slow, Lifebone's full heal, one press for every artefact of a kind (patch 54)
- TODO: #317 Engine-function library: plain cdecl calls into LTO-ABI engine functions, checked against the re-kit rows
- TODO: #320 Exe patches: link the arena (.srpx) from C/NASM sources, no pinned addresses
- TODO: #314 Kit presets: five kit sets, the active one is the kits, a Character dropdown, and a preset offer on room join
- TODO: #321 Lobby: Play and the rooms' entries greyed until the reasons are known; a refused Play closes the rooms window
- TODO: #324 Shop shows only the bundles: reputations again after the price list
- TODO: #322 Reliability never rolls: upgrades, rerolls and drops leave it out
- TODO: #323 Updater: a file Windows Security blocks stops the update, never "up to date"
- TODO: #325 Artefact hints say the new numbers (Spring, Lifebone, Onyx)
- TODO: #326 Reroll window: remove the one-modification lock code left after #309
- TODO: #327 Upgrade: a failed craft no longer takes a modification away
- TODO: #328 Reroll window: the game's own Cancel in place of "Keep old"
- TODO: #329 Craft window: Buy from its slots; the TT-33 in the shop at 500
- TODO: #315 Patch 45: the camera flies to the bench at once
- TODO: #330 Research HUD: "Capturing unit..." on one line, not "Captu"

## 23 Oct, 13:10 — TODO: the owner's words (update required)

<!-- Revision 31 (026g-rev31), released 2026-10-23 13:10 UTC. To be worded for players; what it shipped: -->

- TODO: #334 Modification window: hovering the item shows its stats tip
- TODO: #332 Shop: a kit item's menu acts on that item (Modification refused on rev 30)
- TODO: #336 T16: Regeneration modification works, it is just small (findings + re-kit rows)
- TODO: #333 Research HUD: units under the timer from the first frame, below the respawn line
- TODO: #339 Crouched steps fire: the moving crouch clips get the step event channel (T27)
- TODO: #340 Larkspur (Lifebone): active cooldown 15 s -> 45 s
- TODO: #338 Lobby: Autorepair box gone, Autobuy on by default, larger loadout switcher, inventory loadout badges
- TODO: #337 Protective mask (1816) is just a mask: worn with a helmet (T28)
- TODO: #341 Master set: its camouflage net drawn as a normal alpha-tested material, not a glass-like forward one (T31)
- TODO: #225 Patch 42: dynamic anomalies as the release had them, on London and Cologne (exe, data, bots)
- TODO: #342 Match HUD: keys 1/2/3 pick a kit and Hide chat in match (patch 58, T29)
- TODO: #345 Take the dynamic anomalies (#225, patch 42) out of revision 31
- TODO: #349 Lobby: Esc closes the topmost modal only, from one modal stack
- TODO: #350 Esc follow-up to #349: box over results, invite Esc hides, craft search, loading, dropdown order
- TODO: #352 Research: count captures in the player's own kit cell
- TODO: #353 Research HUD: units back at rev 30's height

## 25 Oct, 12:14 — TODO: the owner's words (update required)

<!-- Revision 32 (026g-rev32), released 2026-10-25 12:14 UTC. To be worded for players; what it shipped: -->

- TODO: #356 Upgrade window: loadout badges on the item and donor slots
- TODO: #368 Patch 62: F11 hides the whole interface (match and lobby); no-UI battery mode spec
- TODO: #397 Bandages: hands and legs full, pain, body and head 35 %
- TODO: #400 New accounts start with a starter kit
- TODO: #408 Onyx nerf: -60 % for 3 s, +1 s for each other Onyx
- TODO: #414 Artefacts: the carrier sees his own glow in first person
- TODO: #423 Onyx: -75 % when active
- TODO: #422 Rooms: the create window opens with its saved settings, preloaded
- TODO: #424 Lobby card: stats over their scale show their real value
- TODO: #418 Kill feed: wall, ricochet and pain-shock icons after the weapon (patch 67)

## 25 Oct, 13:52 — TODO: the owner's words (update required)

<!-- Revision 33 (026g-rev33), released 2026-10-25 13:52 UTC. To be worded for players; what it shipped: -->

- TODO: #436 Kill feed: a suicide row with the pain icon no longer empties the feed

## 25 Oct, 18:43 — TODO: the owner's words (update required)

<!-- Revision 34 (026g-rev34), released 2026-10-25 18:43 UTC. To be worded for players; what it shipped: -->

- TODO: #443 SOCOM 16 nerf: damage 70 -> 60, armour piercing 45 -> 40, equipment level 15
- TODO: #444 Patch 72: falls gain speed for the time they fell, not per step of a split tick
- TODO: #367 Research: capture timers and the units' markers on the screen
- TODO: #450 Reroll: every modification change costs the same
- TODO: #440 Patch 70: baked camouflage textures kept across games, made once, held for the game
- TODO: #453 Network help: clearer "no connection" errors in every language, VPN.txt, firewall script
- TODO: #456 SVD at equipment level 15
- TODO: #457 AGENTS.md: types and states (illegal states unrepresentable, parse don't validate)
- TODO: #458 Revert Research HUD markers (#367) for rev 34

## 26 Oct, 15:08 — Game update 35: Blackout, London spawns and ragdolls (update required)

<!-- Revision 35 (026g-rev35), released 2026-10-26 15:08 UTC. To be worded for players; what it shipped: -->

Close the game and start `Skhid.bat`. The launcher installs revision 35 and
its new game content. Older game versions can no longer sign in.
Recordings made before revision 35 can no longer be watched, because this
update changes what a match computes.

- **Blackout — a new Protective device mode.** Collect batteries with the
  usual rules, but with the normal interface hidden: no crosshair, minimap,
  health or ammunition display, score, kill feed or objective markers.
  Hold **Tab** to check ammunition, quick-use items, player markers, match
  time and chat history. Looking around still works while checking, but
  movement and other actions pause; release Tab, then press an action again
  to use it. When dead, Tab also shows your kits, and **1–3** still selects
  a kit. Chat appears while typing. Blackout awards **double match points,
  silver, experience and reputation**; PIN rooms still give no rewards.
  It is available in the normal room mode pool, and room creators can
  deselect it.
- **London:** removed the anomalies and moved both teams' spawn points
  inside their buildings, onto the first floor above ground level. Each
  side has ten spawn positions, used consistently in Protective device,
  Protective device with chargers, Team deathmatch and Research.
- **Ragdolls:** bodies now collapse in response to the killing hit, carry
  their running momentum and settle against the ground and stairs. Dropped
  weapons fall beside them, and further shots can move a body again. The
  death camera follows your body's head while still letting you look
  around. Bodies do not block bullets or battery pickup.
- **Private rooms:** the creator starts the match with **Start** and uses
  the **« / »** buttons to arrange both teams. You can start alone at
  **1 v 0**, practise with uneven teams such as **1 v 10**, or fill all
  twenty places, with up to ten players per side. The game keeps the
  chosen teams between matches and waits for the creator to start again.
- **Death effect:** the view gradually dims, loses colour and narrows
  around the edges during the respawn countdown. Its pace follows the
  time left, the scene remains visible, and the view clears on respawn.
  Kit selection and the rest of the interface stay readable above it.
- **Weapon icons:** weapons added from later versions now show their own
  kill-feed icons, including the HK416, P90, MP7, Saiga-12K and Pecheneg.
  The M4A1 also uses the correct M4 icon.
- **Minimap:** choosing Hidden now keeps it hidden after restarting the
  game or loading another match. Changing its position takes effect
  immediately.
- **Crash reports:** the crash window now shows useful details even when
  Windows cannot save a memory dump, with a **Copy** button for reporting
  them. Reports also retain game logs containing Cyrillic, Polish or
  other supported local characters instead of discarding those logs.

## 27 Oct, 21:18 — TODO: the owner's words (update required)

<!-- Revision 36 (026g-rev36), released 2026-10-27 21:18 UTC. To be worded for players; what it shipped: -->

- TODO: #556 Changelog: revision 35 for players
- TODO: #558 release: ship and check the bots' data files (maps, routes table) with the release
- TODO: #566 release: the bots' foliage file in the bot data list
- TODO: #578 Bots at every equipment level, in the top players' kits
- TODO: #569 Rooms: a private room has no max level
- TODO: #599 Firewall script: in Russian, and it asks for administrator rights itself
- TODO: #686 Fix Research markers and countdown layout using Rust
- TODO: #689 Medicines: accurate card values, activation sweep and effect countdown in the quick slots, gray while pending

## 29 Oct, 20:32 — TODO: the owner's words (update required)

<!-- Revision 38 (026g-rev38), released 2026-10-29 20:32 UTC. To be worded for players; what it shipped: -->

- TODO: #698 Changelog: revision 37 for players, including rewards and solo replays
- TODO: #727 Add weapons wiki, recoil comparison and detailed game tooltips
- TODO: #739 Shop: buy an item by dropping it on its slot
- TODO: #721 Game languages: ten dictionaries as TSVs, Belarusian, Czech/Hungarian/Finnish/Serbian, Language in Settings
- TODO: #741 Rust patches: typed raw handles, layout only in raw
- TODO: #745 Weapon rolls: recoil and aim time take effect; chamber time on the card
- TODO: #716 Flag of Serbia decal: red over blue over white
- TODO: #746 Site: the wiki takes the calculator and the maps, and gets a modifiers page
- TODO: #750 Equipment through 0.29: 84 pieces, balanced against native gear
- TODO: #752 Weapons wiki: shots at their own moments, views hold at their last shot, no Next shot
- TODO: #699 Detach selected helmets on fatal headshots
- TODO: #754 Equipment: its texts in all ten languages (release 38 content build)
- TODO: #753 Weapons wiki: sideways kick at a degree a unit; smooth camera shake
- TODO: #757 Wiki: Equipment, every armour piece with its protection, weight and bonuses
- TODO: #755 Notes: what the 0.26g armour parameters do
- TODO: #756 Headwear: the 0.29 helmets fly off on fatal headshots; the knit cap doesn't
- TODO: #758 Wiki modifiers: Can roll leaves out what an item comes with
- TODO: #760 Exoskeleton boots and gloves without their black areas (pack exo-gear@1)
- TODO: #763 Exo boots' lobby icons at the stock boots' angle (exo-gear@2)
- TODO: #762 Release: per-release item tables (equipment, weapons) in the record
- TODO: #761 Wiki equipment: flying off only in the card's details
- TODO: #759 Built-in bonuses at the second-strongest strength; exoskeleton suits buffed
- TODO: #751 Carry and share artefacts with animated hand poses and death drops
- TODO: #765 Match: an anomaly artefact pickup scores 15 (5 with full containers)
- TODO: #770 Equipment 0.29: no donor placeholder descriptions
- TODO: #771 Equipment 0.29: German placeholder descriptions too
- TODO: #773 Weapon card: chambering time shows only the new value
- TODO: #774 Exoskeleton gloves keep their stock icons (exo-gear@3)
- TODO: #775 Equipment 0.29: no shader 0.26g lacks (Watchman gloves crash)
- TODO: #777 Artefact carry: H held, then a quick key, raises the artefact
- TODO: #779 Artefact carry: H's up comes from the key handler (stuck carry key)

## 30 Oct, 17:44 — TODO: the owner's words (update required)

<!-- Revision 39 (026g-rev39), released 2026-10-30 17:44 UTC. To be worded for players; what it shipped: -->

- TODO: #804 caddy26: plain TCP only (net.mptcp.enabled=0)
- TODO: #787 Launcher: the language is the game's, every language, kept in user.cfg
- TODO: #783 Artefact carry: H's key states as the engine defines them
- TODO: #790 Artefact carry: no gun falls from a carrier's death
- TODO: #792 Late join: a loaded world puts its alive players into it (patch 82)
- TODO: #806 Equipment from 0.31a2 and 0.46c2: 35 of the owner's 38 picks (pack equipment-046@4)
- TODO: #825 Backpacks from 0.69: the Medic's bag and the Soldier's backpack (pack equipment-069@1), into main
- TODO: #793 Rooms: the chosen room's card keeps one layout through the vote
- TODO: #786 Release: build and stage before the go, then the maintenance and a quick swap
- TODO: #789 Lobby: the kit's movement speed beside the weight
- TODO: #828 Release: close four gaps found in releases 36-38
- TODO: #831 Minimap: a late joiner's allies get the ally's icon
- TODO: #832 Revert #792: the world load already inserts its players (patch 82 never acts)
- TODO: #829 Hud: the Double cylinder's quick-slot icon
- TODO: #834 Wear the exoskeleton boots under full-length trousers
- TODO: #781 Artefact carry: the held hand breathes and flexes its fingers
- TODO: #835 Ragdoll: no headshot launch (exp_neg is 0 from 20)
- TODO: #764 Add breakable map lamps with the Rust patch framework
- TODO: #838 Wiki icons: read the shop filters from FiltersWindow's list functions

## 31 Oct, 19:04 — TODO: the owner's words (update required)

<!-- Revision 40 (026g-rev40), released 2026-10-31 19:04 UTC. To be worded for players; what it shipped: -->

- TODO: #842 Shop filters: one table the lobby build and the wiki read
- TODO: #843 Lamps: orient each shatter burst in a frame the game can build
- TODO: #841 Rooms: drop the vote sentence from the room's bar
- TODO: #845 Lobby: a right-click menu's Buy buys the item it was opened on
- TODO: #852 Play that would queue offers the other bracket's room as Join does
- TODO: #857 Tab map: names over the player icons while Alt is held (patch 82)
- TODO: #860 Tab list: Ctrl shows a cursor, and a row's menu sends a friend request (patch 86)
- TODO: #876 Rooms: members by Play stay after the match; the results stay up under room changes
- TODO: #863 Settings: Hide hit markers, Hide player scores (exe patch 83)
- TODO: #862 Lobby: transient windows (History closes when anything else opens; PvP/bots list over Rooms)
- TODO: #879 Settings HUD group: "Hidden minimap" as "Hidden crosshair"
- TODO: #878 Mute chat: two state-dependent entries in the lobby's player menu and the Tab list's row menu (patch 86)
- TODO: #880 World frame check: the match server and the game refuse a world no game could load (patch 87, crate world-frame)
- TODO: #895 Short scope slot label and stock scope names that fit the upgrade window
- TODO: #884 Squads of two: Play and Join take the mate along, marked in the rooms
- TODO: #881 Music: the menus' and maps' themes, 15 s past the start (patch 88, pack music@1)
- TODO: #851 STK2 outfits: the bandit's, the Novice's and the loner's pieces (packs stk2-bandit, stk2-novice, stk2-loner)
- TODO: #883 Patch 84: a bullet that hits a player stops the healing drug in use
- TODO: #901 Blackout: show teammates' markers all match, not only on Tab
- TODO: #882 Medicines: three coloured medkits, all five made from one table
- TODO: #805 Rudnya (v0.69): 0.69d's Rudnya as map 204 and its battery and Research modes (216-218)
- TODO: #909 Launcher: read the installer's values without opening any key
- TODO: #778 Lobby scene picked in a lobby window, the squad commander's in a squad, shown at once behind a fade (patch 81); patch 45 round 7
- TODO: #776 Content: the 0.46c2 hangar and 0.69d tent-camp lobbies (lobby-hangar-046@7, lobby-camp-069@9)
- TODO: #802 STK2 bomb shelter lobby (lobby-stk2-bunker@4): relit warm, the whole shelter; the Bar dropped
- TODO: #915 Lobby selection review: keep the lobby under the upgrade window, drop content-rev40
- TODO: #899 Start Skhid without the updater; split the Defender exclusion from the network setup
- TODO: #913 Lobby button: drawn at its box's size (it vanished in the game)
- TODO: #919 Rudnya (v0.69): ammo only at the bases
- TODO: #916 Lobby packs: textures 0.26g already has drawn from its own files (hangar @9, camp @17)
- TODO: #920 Drops: no camouflage or decal the player already has

## 1 Nov, 22:38 — TODO: the owner's words (update required)

<!-- Revision 41 (026g-rev41), released 2026-11-01 22:38 UTC. To be worded for players; what it shipped: -->

- TODO: #921 Remove dead Rust code
- TODO: #922 Remove stale docs, unused 0.100b tools and unused example files
- TODO: #923 Remove the dead tutorial code from the lobby and hud movies
- TODO: #924 Small audit fixes: Blackout in Matches, one on/off switch, one mask length, RUST_PATCHES
- TODO: #926 Remove old notes and one-off audits
- TODO: #946 Rudnya (v0.69): each base ammo crate refills its own team only
- TODO: #991 Notes: the 0.26g renderer as a light bake needs it
- TODO: #999 Login: ask for the nickname, not the e-mail
- TODO: #983 Shop: a check mark on owned items, and a Hide owned box
- TODO: #985 Rooms: rejoin the room's running match through the room after a relaunch
- TODO: #982 Queue of a full room offers a loadout as Join does
- TODO: #1000 Batteries: points for a battery's new progress toward its container (patch 92)
- TODO: #984 Lobby: buying an item owned asks first; only the inventory's right-click Buy skips it
- TODO: #1002 Player vs Bots: drops roll at match level 7 at most, drop cards too
- TODO: #893 Weapon kit, step A: the kit transport and a second decal on weapons (patch 68, Rust)
- TODO: #996 Patch 91: a weapon shown while aimed keeps its body
- TODO: #1004 Upgrade window: a double click puts a camouflage, decal or scope in its slot
- TODO: #1003 Shop: a consumable's buy starts at 30
- TODO: #1009 Download mirrors: the game's archive and the patches from many addresses, resumed, logged
- TODO: #990 build-content: files an earlier version of a pack left stay, listed
- TODO: #1013 Setup scripts: English as well as Russian; the Defender setup puts back deleted files
- TODO: #1017 release/test.sh: Ctrl-C tests signal a release.sh that can be interrupted
- TODO: #1018 Crash dumps: referenced memory live, full memory for local tests
- TODO: #939 Balance the sides by K/D at the match's equipment level
- TODO: #1023 Battery points: progress paid at the delivery; found 5, delivery share 5
- TODO: #1024 Patch 85: pick up a dead player's gun
- TODO: #951 Patch 19's 0.29a3 weapons: melee k_damage 1.5, as every native weapon's
- TODO: #1001 Content: 0.31a2's lower screen shake (shake-031-1)
- TODO: #1011 Content: HK416 and M4A1 kick nearer the AK-12 (recoil-hk416-m4a1-1)
- TODO: #1031 Revert #1011 (HK416/M4A1 recoil) for revision 41

## 2 Nov, 03:58 — TODO: the owner's words (update required)

<!-- Revision 42 (026g-rev42), released 2026-11-02 03:58 UTC. To be worded for players; what it shipped: -->

- TODO: #1026 Admin chat mute: a muted account's lines reach nobody, it is told why
- TODO: #1037 Put #1011 (HK416/M4A1 recoil) back after revision 41
- TODO: #1032 Weapon stats as a table (weapon-stats.tsv); cards written from the configs
- TODO: #1041 Readers of content.tsv edit rows skip stats tables
- TODO: #1044 Admin page: the level field's label says 0 to 100
- TODO: #1028 Scaleform: the render thread's capture walk never reaches a queue being destroyed (patch 93)
- TODO: #1034 Weapon reload rolls: checked, they work as the card shows (notes + re-kit)
- TODO: #1046 Shotguns: Saiga-12 and Vepr-12 armour piercing 0.1 -> 0.3
- TODO: #1043 Dead duplicate keys out of the item configs (dead-keys-1)
- TODO: #1048 A-545: damage 50 and 900 rounds a minute again
- TODO: #1047 Stats tables: list rows, several files a row, entry selectors, key/value tables
- TODO: #1033 Lobby: hold Alt to see a weapon's stats with kit and skills
- TODO: #988 Room lobby: camp @18, no hard-edged ambient boxes at the ceiling
- TODO: #1052 Site: serve every file of damage/ (Rudnya's render was not served)
- TODO: #1045 Patch 85: fix the rev41 late-join desync, simplify the pickup's ammo
- TODO: #1049 Fit the rooms' Leave on Play in every language
- TODO: #1050 re-kit: one name for the weapon states' get_weapon_lexeme
- TODO: #1054 Home page: trimmed download steps, then a short FAQ
- TODO: #1063 Check that a resync converges: world load+save round trip
- TODO: #1040 Shop: smooth Hide owned tick, Hide owned beside Comparing items, no sort (always by level)
- TODO: #1061 Skhid-collect-logs.cmd: logs and the game folder's state in one ZIP to send
- TODO: #1056 Bug-report Discord invite: discord.gg/QUmJg3gBhg
- TODO: #1065 Ragdolls: rest on the bullet collision too (dropped guns no longer sink on Rudnya)
- TODO: #1055 Ammo and throwables as tables (ammo.tsv, throwables.tsv)
- TODO: #1066 Maps: the game's minimap by default, the render on the player's pick
- TODO: #1067 Self-test ragdoll: the arena bounds the Rust statics; replayed calls allowed
- TODO: #1062 Updater: a box per problem of the site's list, its title what failed
- TODO: #1069 Home page: quote the launcher's new box titles
- TODO: #1058 Artefacts as a table (artefacts.tsv); artefacts-1 goes
- TODO: #1068 re-kit 0.26g: the weapon of a kill, a hit and a shot (research)
- TODO: #1059 Character and battery item as tables (character.tsv, battery.tsv)
- TODO: #1051 SR-3M 850 rounds a minute and +5 % movement; Vector armour piercing 0.3
- TODO: #1030 Renderer study: 0.26g frame reference, light shader sources and PCSS prototype
- TODO: #1070 MSBS: full damage to 35 m, 60 % from 70 m (was 20 / 50)
- TODO: #1073 Saiga-12 and Vepr-12: slugs take three, buckshot two only with every pellet
- TODO: #1076 ASh-12: 425 rounds a minute (was 400)
- TODO: #1075 MP7: armour piercing 0.3 -> 0.35
- TODO: #1077 Weapon pickup: a picked gun drops at death, and a drop lasts until that player's next death
