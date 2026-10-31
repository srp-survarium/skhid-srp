# Open issues: the 0.26g server

> **Version scope: the 0.26g server** (`srp-026g`, SRP-026G.md). **Live on
> 026g.skhid.link since 2026-10-12.** Sources: `srp-026g/WORK.md` (the
> 2026-10-12 entries, newest at the top).

### The list written before the first live session, and what live play settled

Items 26-1 to 26-13 were written before any real client had connected, each
as "unverified live". **Seen working with real clients on 2026-10-12**
(`srp-026g/WORK.md`): the TLS sign-in and registration (26-1's first part);
the lobby with the account, the inventory and the shop's prices (26-2,
26-3, the 0x8A part of 26-4); moving items, choosing a faction, the ammo
dialog (part of 26-5); Play, the queue, "match found", loading and both
clients connecting (26-6); the patched exe starting and uploading worlds
(26-7); matches staying in step, with resyncs (26-8); a match's end and the
results screen (26-10's first part); squads in the queue (part of 26-11).
What was found broken on the way and fixed is in `WORK.md`. **Everything
else in these items is still unverified**: the refusals' texts, 26-9,
26-12, 26-13, and the details each item names.

26-1. **Login over TLS** with the real client (OpenSSL 1.0.1h against srp's
      OpenSSL 3), the 11-byte answer, the status codes the client shows for
      a wrong password, a ban, closed registration, too many attempts.
26-2. **The lobby sign-in push**: whether its order is enough to reach the
      lobby screen; `0x80`'s second byte and what `32 00 <flag>` asks for
      are not known (CLIENT-FACTS-026G.md).
26-3. **`0x89`'s tail** (three empty lists, `00 00 00`): if the lobby breaks
      right after the inventory, look there first.
26-4. **Stored LZMA2 chunks** (0x8A prices, 0x95, 0xE4): whether the client
      decodes them.
26-5. **Shop, kits, skills, quests**: every action and every refusal's text
      on a real screen. The 0x9F repeat (every 5 s) that keeps Play on.
26-6. **Play with two clients**: the queue screen, "match found", `0x82`,
      the level loading, both connecting to the match server.
26-7. **The upload patch**: the client starting with an 8th PE section at
      all; the upload code running (it never has); the fallback is the
      `--cave=hash-mismatch` build.
26-8. **Match sync** (MATCH-SYNC-026G.md): the snapshot reader on a real
      world (the order of the rules inside it is LIKELY), the 0xDB clock
      field (LIKELY; `SRP_CLOCK_CORRECTION=off`), a first world that already
      has statistics (`SRP_CHECK_INITIAL_WORLD=off`), whether late inputs
      should be moved or discarded (`SRP_LATE_INPUTS`).
26-9. **Hash phases**: the 50 ms interval of the full patch was never tried
      live; `-hash_check_interval_in_ms=50` on the command line is LIKELY
      syntax. Watch for "on opposite hash phases" and "no agreed hash".
26-10. **The end of a match and the results**: the mode rule's end, score
       events 24 and 26, the results screen (0x97), the drop card (`46 03`:
       its two arguments are a guess, both orders are taken), the account
       after a second sign-in.
26-11. **Chat, friends, squads** on real screens; a squad queueing as one.
26-12. **The web page with a real account**: the admin's changes showing
       in an open lobby (the account pushed again outside a sign-in; the
       premium time through 0x90), the operator's text (0xA5: where the
       client shows it, whether both lines show), the ban's sign-out (0xAA)
       as the player sees it.
26-13. **An item given by the admin that the lobby cannot show**: every
       `items_dict` id is accepted; whether each kind is safe in an
       inventory was not checked item by item.

### Coded on 2026-10-12, not looked at with a real client yet

26-14. Item modifications, upgrades, reroll and craft; the premium shop,
       bonuses and the nickname change; clans; the match server follow-ups;
       the economy by the sources of the time; matchmaking by level; the
       return of a silent lobby client. All tested by scripted clients only.
       `srp-026g/WORK.md` has each one's entry.
26-14a. **Guesses in the match pay**: the activity row is a line through two
       won matches; what a lost match paid, and what made a kill the
       cheaper kind (20 / 2), are unknown
       (`srp-026g/docs/notes/progression_sources.md`). A results
       screen of a lost match would settle the first.
26-14b. **What the server does not do for a client message** is listed in
       `srp-026g/docs/notes/coverage.md` (comments about a
       player refused, the tutorial video link never shown, the login queue
       and EULA screens never sent).

### Server and deployment

26-15. **Deployed 2026-10-12** on a host of its own (`DEPLOY.md`).
       Open: no off-server copy of the database or the logs; no host rate
       limits; the host has one CPU, so an image built there while a match
       runs takes from the match (build only when none runs).
26-16. A restart of srp forgets the queue, squads, the operator's text and
       every match being played (a match ended by a restart gives no
       results). The results screens not shown yet and the play periods
       are kept since 2026-10-12 (26-30).
26-17. The abuse findings of item 10 apply to 0.26g as well (results from
       client snapshots, no host rate limits; the lobby is UDP now: links
       are capped at 4096, refused ones at 256).
26-18. No console in the srp terminal (the 0.100b-era `status` / `start` /
       `end`): the web admin is the only operator tool.
26-19. Nothing of 0.26g is committed (`srp-026g`, branch `v0.26g`). Commit
       only when the owner asks.

### Docs

26-20. The notes contradict each other in places (found while writing
       CLIENT-FACTS-026G.md; the notes were not changed): an out-of-range
       lobby id crashes (`lobby.md` H1) or is dropped by the transport
       (`transport.md` §4.1); 0xA7 "read and ignored" (`lobby.md`,
       `social.md`) or a jump to address 0 (`transport.md`): never send it;
       kit state 2; the 0x9E link length (245 or 251); chat client ids
       (0x4C..0x4F or ..0x50); `match_levels_dict` values (`data.md` against
       `modes_maps.md`); `match.md` §9 and §2.3 behind its own §14.
26-21. The 0.26g notes were written before the server went live; their
       status lines were corrected on 2026-10-12 (`README.md`, `DEPLOY.md`),
       but single notes may still say "not seen with a real client" of
       something live play has since shown. `srp-026g/WORK.md` has the last
       word.

### Client defects found live on 2026-10-12 (postponed by the owner)

26-24. **The previous-weapon bit can crash every client of a match.** An
       input with mask bit 26 from a player who has something other than
       its two weapons in its hands makes each client take a slot number
       from outside a two-entry table (it gets 47 of 23) and test the dword
       0x40 bytes past the end of that player's inventory object; anything
       but 0 there is called through as an item. PROVEN in the exe
       (`srp-026g/docs/notes/match.md` §20: 0x2D8D0A..0x2D8D1C,
       table 0x927DF0, inventory 0x188 bytes). 0.26i's notes name a fix for
       "crashes of all clients ... associated with sending "bad" input of
       one of the clients". Not known to be what killed match 0x5825bf7
       (all five clients gone within 4 s, 12:03 UTC): its inputs were not
       kept and no dump was seen.
       - The server cannot filter it: it does not know what a player holds,
         and the owner of the input has applied it already.
       - The exact fix is one instruction of the client (take slot 0x17,
         "none", when the item in hands is neither weapon), which is what
         an unpatched client does whenever the stray dword is 0, so patched
         and unpatched clients stay in step. **Not done: postponed by the
         owner on 2026-10-12.** It needs a new exe for every player.
       - Open: which items count as "in hands" besides the two weapons
         (LIKELY a grenade or a medkit in use); which of the two keys is
         bit 26 (LIKELY previous weapon); a dump from a player.
26-25. **A game that starts from a Team Deathmatch already in play has the
       tickets wrong in its statistics** (100, 100 and a draw; its tdm rule
       has the right ones), so it is out of step from its first tick and
       is resynced a few seconds later, which puts it right. 4 of 4 returns
       on 2026-10-12; PROVEN from the saved worlds of match 0x2490635b and
       since in the exe (match.md §21: the timelimit rule's "match started"
       event of a first load, 0x317FD0 → 0x316090 → 0x3161F0).
       - **Live since 2026-10-12 13:32 UTC:** a player back in such a match is started
         once more as soon as it has its first world (`game.rs::reload`),
         which a game in play takes as it is.
       - **Open: a player in the match for the first time when it is in
         play already** has the same wrong statistics and is still put
         right only by the resync, some seconds later (why: match.md §21).
       - The client's own fix would be for the tdm rule to write the
         statistics after it has read its tickets. Not looked into.
26-26. **Live since 2026-10-12 13:32 UTC:** the match server
       does not relay an input whose rotation is an infinity, a NaN or
       1024 radians or more (`PlayerInput::rotation_is_sane`; match.md
       §20).

### The update of 2026-10-12 (live on 026g.skhid.link since 13:31:58 UTC; none of it seen working by a player yet)

All of it is in `srp-026g/WORK.md`, "the next update". Workspace: 1004 tests
pass. None of it was seen by a client.

26-27. **A squad alone in the queue gets no match** (it got one with nobody
       against it). It waits for one more player who is not of it; not for
       a second squad (the owner's rule). matchmaking.md §3. **The live
       0.21d server had the same error**: fixed in `srp-021d` and deployed
       the same day (13:26:50 UTC; its `WORK.md`).
26-28. **A connecting client whose first datagram is lost is waited for**
       (it was told to disconnect after 300 ms). match.md §22.
26-29. **The server's log is kept across updates**: one file per UTC day in
       the data volume, 14 days (`SRP_LOG_DIR`, `SRP_LOG_KEEP_DAYS`).
       DEPLOY.md, "The server's log". Open: nothing copies it, or the
       database, off the host.
26-30. **The results screens not shown yet and the play periods outlive a
       restart** (`g26_results`). results.md §11. Still lost at a restart:
       the queue, the matches being played, squads.
26-31. **Open, the owner's decision: the clock corrections that undo each
       other.** Of the day's 271 corrections under 30 ms, 105 were followed
       within 6 s by one the other way and about as large: the server
       corrected a measuring error and then corrected it back. No harm to a
       player was tied to it. The proposal and its costs are in
       `srp-026g/WORK.md`, "the next update"; nothing was changed.

### From upstream's live-tested branch

26-22. **Upstream's `origin/feature/server-026g`** (repo `srp`; a dev match
       server with a fixed roster, no lobby or matchmaking, live-tested
       under Wine) disagrees with ours in places.
       `srp-026g/docs/notes/upstream_compare.md` is the
       cross-check; its "Recommended changes" table is the list. Open as of
       2026-10-12 (the match server ones are with the match follow-ups in
       progress): joined messages by default and a snapshot chunk inside a
       multi datagram; no replay of a client's own inputs after a resync;
       resync by 0xE8 alone; the 10 s / 30 s timeouts against unfocused
       clients that stall up to 58 s; 18 tail ack bytes; the Wine-only
       `qwave.dll` crash on the lobby link.
26-23. **The launch line.** `-mt_allocator=768` is needed to load a map
       (upstream, live under Wine; read in the exe: the default arena is
       376 MiB at 0xE0FF5, and `-minimum_resources_memory_size` is another
       arena, 0xE9966). It is in `make_client_026g.py`, the web page and
       the docs, and the shortcuts have it. Both keys together are what the
       players' start file uses; maps load with them (live, 2026-10-12).

## 0.26g, added 2026-10-12 (afternoon)

- **Done, live on 026g.skhid.link since 14:38:25 UTC:** an unarmed kit is no longer loaded in a match (it ended every game of three matches that day: match.md §23); a match whose world is lost is over after 20 s instead of holding its players (§24); the first accept of a squad invite is no longer lost (social.md, correction). `srp-026g/WORK.md`, the first entry of 2026-10-12.
- **The owner's to decide:**
  - Everybody of a match that died gets `Left` and rating −2, also the players who came back.
  - (Settled the same day: the squad invite fix is live on the 0.21d server too, `srp-021d/WORK.md`.)
  - The exact fix of the spawn crash is one test in the exe (`player::respawn`, +0xF47C null).
- **Open:** a client whose link is up and whose game stands still (a window that is not active) is still asked first for a world when its link is the best (`game.rs::sources`), which keeps a player back in the match waiting until that client is resynced or leaves. What `player`+0xF47C is meant to hold. How the match's kit choice draws a kit of state 2.

## 0.26g, added 2026-10-12 (later)

- **Done, staged on 026g.skhid.link (restart when no match runs):** a player who left its match by the menu is queued for another match instead of being sent back to the one it left (match.md §25; `srp-026g/WORK.md`). The same is in the `srp-021d` tree, not deployed.
- **The owner's to decide:** a match of two whose one player left for good goes on with the other alone to the time limit. Should it end at once, and who has won?

## 0.26g, added 2026-10-12 (evening)

- **Published 2026-10-13: the client patch for the tenth battery.** In modes 2 and 3 a team often cannot put its last battery in, and after the try nobody can use a container (match.md §26; 14 of the 19 live matches in which a team had all but one). The fix is two instructions of the client (client_patch.md §3.9, patch revision 3): built, self-tested, its handler run outside the game, and in the players' download since 2026-10-13 12:22 UTC. Not watched yet: a match played to a team's last battery with it. A game started through the launcher takes a new ZIP by itself; a game with a download from before the launcher keeps the old rule until its player takes the ZIP once more (on 2026-10-13 most accounts still signed in with revision 0), and old and new games part at a team's last battery. The login server logs the revision and does nothing else with it.

## 0.26g, added 2026-10-13: a Documents folder the game may not write to

- **In the players' download since 2026-10-13 15:36 UTC:** a game whose folder `Survarium`
  in Documents cannot be made, or cannot take a new file, stopped two
  seconds after its start in one of two ways (an access violation at
  0xF9A8F, an assertion about the `.journal` file at 0x40141). Client patch
  revision 4, eight bytes (client_patch.md §3.10): both crashes made again
  on a test PC, the built exe started in both situations. Not run by us: a
  sign-in, a match, saving the options into such a folder (the first
  sign-in with revision 4 on the players' server was logged three minutes
  after the release).
- **In the same download:** such a game is played
  with settings that are not saved, and a message box at its start says so
  and what to do, in the game's language (`client/bugtrap`, its README's
  "Start-up notice"). The check runs in the game's own process: Windows'
  "Controlled folder access" judges each program by itself, so a check by
  the launcher or the updater would say nothing of the game.
- **Open:** the box was tried with a folder that refuses writes by its
  access list, not with Controlled folder access itself; the Windows labels
  in its Russian, Ukrainian, German and Polish texts want a native check.
- **Not known:** what refuses the folder on the PC of the reports.

## 0.26g, added 2026-10-13: the launcher

- **Deployed, and in the players' download since 2026-10-13 09:50 UTC:** the game's launcher
  patched to run from an unpacked game and to ask this server only, with an
  updater of ours that installs the ZIP the page offers whenever it changes
  (`../client-patches/launcher/README.md`). Run on Windows 11 with a local
  server, the test server and the players' one; three later releases reached
  installed launchers that day (`../client-patches/README.md`, the rows of
  the downloads).
- **Done 2026-10-13:** the server part (`/api/client-files`, `/launcher/`)
  was deployed first, then the ZIP with the launcher was offered. The order
  matters: a launcher asking a server without the routes says "offers no
  client files now" at every start (PLAY still works).
- **Not run:** `bash play_skhid.sh launcher` under Wine; Windows 7, 8 and 10;
  a PC that still has the real game's installation and its update service.
- **Not done:** a signature on the ZIP with a key kept off the server (today
  the ZIP is trusted because HTTPS is); a line in the launcher for "the
  server could not be asked" (it says "The latest version of the game is
  installed" after the message box); the updater's messages in other
  languages than English; a count of kills for the page (the database keeps
  none).

## 0.26g, added 2026-10-14: two bytes of a battery's statistics

Both are the original game's own code, in modes 2 and 3: no patch made
them. `notes/match.md` §28 has the evidence and the choices,
`notes/snapshot_rules.md` §3.2 the code.

- **Open, the owner's to decide: a game that starts in a match being played
  does not know who took a battery out of a container.** The statistics keep
  that player in a byte no world carries, and event 26 ("stolen and brought
  home", what the quest `steal_box` counts) is counted only for it. Seen
  live once (match 0x69f8c0ba, a late join of revision 6): a battery taken
  out seconds before the newcomer's world was brought home half a minute
  later, the newcomer's game did not count it, its hash differed and it was
  resynced once. PROVEN from the two worlds of that tick and in the exe. A
  player back in a match and a game that loads after play began have the
  same gap (not seen); a game resynced in an ordinary match does not.
  - Nothing is broken beyond that one resync of that one game, and the
    result is right while the games of the start are the majority.
  - A fix is a client patch, so a new revision for everybody: save the byte
    (the count stays what it was; the worlds get ten bytes longer, so the
    new revision never plays a battery match with 7), or count the event
    without asking who took the battery out (two bytes, plays with 7, but
    another rule). Neither is written.
- **Open: a battery's finder outlives the match.** The reset between two
  matches clears neither that byte nor `first_finder`, and `first_finder`
  goes to every game with the first world. Seen in the same match: seven
  batteries nobody had touched had a finder from the start, LIKELY from the
  match the game that gave the first world had played two minutes before.
  From the code, not seen counted wrong: no "found" (event 19) for such a
  battery, and "found and brought alone" (event 24, the quest
  `bring_x_items`) for the wrong player. No game falls out of step by it.
  The fix is the same patch's (0xFF to both bytes at the reset) and plays
  with revision 7.

## 0.26g, added 2026-10-14: the statistics writer's event masks

- **Done 2026-10-14, in the code: `snapshot.rs` wrote a player's event
  masks otherwise than a game does.** A game does not start the mask again
  for each kit: the second kit's mask has the first's bits too, the third's
  has all three, and each such bit is followed by that kit's count, a zero
  when it has none (`notes/snapshot_rules.md` §3.1; PROVEN in the exe,
  0x33575D..0x3357CD, and seen in every record of the worlds of match
  0x69f8c0ba). `PlayerStatistics::write` made each kit's own mask; it keeps
  the mask across the kits now (a test with made-up records).
  - Nothing read from a real game was wrong: the reader takes either form
    and is as it was, so results, quests and the standing are as before.
  - Only the scripted test client (`match-server/src/v026g/scripted.rs`)
    and the tests write statistics: their worlds are now, for a player who
    has an event, the bytes a game writes. The statistics of the four saved
    worlds of match 0x69f8c0ba, read and written back, are the same bytes
    (a check run once, not a test: the worlds are not in the repository);
    in each of them four players have such zeros, which the old writer left
    out.
  - The change is in the server's code alone: no client patch, no
    revision, nothing for a player to install, and nothing a server in use
    does otherwise.

## 0.26g, added 2026-10-14: a newcomer's score on the scoreboard

- **Done 2026-10-14, in the code (patch revision 9), not in a download:
  the others' scoreboard (Tab) counted a newcomer's score again at every
  rollback.** Seen on the players' server (match 0x57b21c79): every one of
  five newcomers 2.7 to 4 times its score on the scoreboard of a player of
  the start, and one with no kill at 953. The game's statistics handler
  works a score out again after every state it loads only for the places
  of the roster of the start, which the late join does not raise; it adds
  each event as it comes for any place, so a rollback over a newcomer's
  kill added the kill once more. PROVEN in the code (`notes/client_patch.md`
  §3.16), the game's own code run with Unicorn before and after the patch.
  - Only the scoreboard during a match: the results screen, the rewards and
    the quests are the server's and were right.
  - Revision 9 keeps all twenty places in that count (six bytes). It plays
    with 7 and 8; a server need not ask for it. Not run in a game yet.

## 0.26g, added 2026-10-15: a Team Deathmatch that does not end when a team is out

- **Done 2026-10-15, in the code (patch revision 11), not in a download: a
  Team Deathmatch ran to the time limit though a team had no tickets and no
  player alive.** Seen on the players' server in two matches of twenty
  places (0x1f07c8fb, 0x7ce977cb): one dead player of the team that was out
  stayed in the rule's alive mask, so the server's end test (`tdm_finished`)
  never held. The rule's death handler skips every death once its completion
  time is set, and a rollback does not take the time back (the same as the
  tenth battery, `notes/match.md` section 26); revision 11 counts them
  (six bytes, `notes/client_patch.md` section 3.21, `notes/match.md` section
  28). Does not play with 10 or before: a server asks for 11 with the ZIP.
  Run outside the game for the handler (`client-patches/run_tdm_rule.ps1`),
  not in a game. Open: a server that reads a team as out from its blocks
  would end such a match for every revision (`notes/match.md` section 28,
  option 1, not built).

## 0.26g, added 2026-10-14: the team points during the countdown

- **Done 2026-10-14, in the same revision 9, not in a download: the team
  points under "Awaiting match start" were the last match's.** The game's
  hud skips their drawing while a match waits or counts down, though the
  rules fill the new match's numbers all along (the original game's code,
  so it was so before srp's patches too). Revision 9 draws them then as
  play does (one jump, five bytes; `notes/client_patch.md` §3.17). Not run
  in a game yet.

## 0.26g, added 2026-10-14: a newcomer out of step after a respawn

- **Done 2026-10-14, in the code (patch revision 10), not in a download: a
  game that loaded a world during a match went out of step when a player
  respawned into a kit he had used before that load.** Seen on the
  players' server (match 0x1ce45cbe): six short desyncs of its four
  newcomers, each at a respawn into such a kit. A weapon's random state
  (the spread of its shots) was 1 from its constructor and changed only by
  shots and by the load of a world, and a world holds the current kit's
  player alone: the newcomer had the other kits' weapons at 1, the games
  that had played them had them drawn from. The same happened in any game
  after a rollback over a respawn into another kit. PROVEN in the code and
  in the saved worlds (`notes/client_patch.md` §3.18).
  - Revision 10 gives every weapon of the new kit, at every spawn (a
    match's first too), a state worked out from its place, kit, slot and
    the spawn's time: the same in every game. The game's own spawn run with
    Unicorn on made-up objects; not run in a game yet.
  - **It changes what a match computes from a match's first spawn: games of
    10 and of 9 or before part there.** The release needs
    `SRP_MIN_PATCH_REVISION=10` together with the ZIP, as on the players'
    server. The
    server needs no change: its clock factor takes 8 to 0x7F as fast and the
    late join takes 5 and later. A late-join exe built by hand (`0x80`) is
    not refused and does not have it.
  - Not covered: the single-player respawn rule (the test mode
    `loopback_client` alone) spawns another way and keeps the old state.
  - The other cause of that match's desyncs (inputs made while a resync
    world loads) is the server's and is not touched here.

## 0.26g, added 2026-10-14: a newcomer that respawns a player in the wrong kit

- **Done 2026-10-14, in the same revision 10, not in a download: a game's
  first world of a match could make it respawn a player in another kit than
  the other games.** Reported by the analysis of the matches after
  0x1ce45cbe: of fifteen late joins five were out of step from their first
  tick and never let in; for two, a newcomer's world with one player's next
  kit changed gave its reported hash byte for byte (LIKELY a third). The
  first world gives each place an entry at its time in the lists of inputs
  and of kit choices: the kit choice was the kit in use, not the world's
  choice for the next spawn, and both went at the newest end even after
  newer entries relayed before the world. PROVEN in the code
  (`notes/client_patch.md` §3.19), the game's own code run with Unicorn
  before and after the patch.
  - Revision 10 puts the world's choice, and both entries in time order.
    A kit chosen before a match's first world is now kept by every game of
    10 (the original reset it to the kit in use in every game alike).
  - Not run in a game; the hash reproduction was not run with the new exe.
    Whether the two other never-admitted joins had this cause is open.

## 0.26g, added 2026-10-14: a weapon's state that a respawn kept

- **Done 2026-10-14, in the same revision 10, not in a download: a respawn
  into a kit used before a game's world could still part the games after
  the random state of revision 10.** Seen in the evening matches of
  2026-10-14 (0x47d5fa35: twelve games out of step at two respawns;
  0x3c67147b; 0x4301ed65, where the random states were equal): the saved
  worlds differ in the slot-10 weapon's `m_last_tick_time_in_ms`, -1 in the
  games that had loaded a world, the time of its last use in the others.
  A weapon also keeps the fire mode and the ammunition kind the player
  chose through deaths and spawns; a world holds the current kit alone. PROVEN in the code and in
  the saved worlds (`notes/client_patch.md` §3.20).
  - Revision 10 now gives every weapon of the new kit -1 as its last tick at
    every spawn, and at a spawn into another kit the constructor's fire
    mode and ammunition kind (and, as a safety, LIKELY no-ops, magazine,
    chamber, aim and recoil), before the initialize. For players exactly
    two things change, after a respawn into another kit (kept at a respawn
    into the same kit): the kit's fire mode and its ammunition kind are
    forgotten. Edge case: with kind 0's rounds used up and another kind
    left, the weapon spawns empty and switches only at an input with action
    bit 20h or 40h (which keys: NOT PROVEN). No ammunition is lost or added.
    The game's own spawn run with Unicorn; not run in a game.
  - NOT PROVEN edge case: rounds in a magazine of a weapon with no
    ammunition item would be dropped by the reset; no way to load such a
    magazine was found.
  - **Open: a player who leaves and comes back.** The place's profile (the
    storage of stacks, the conditions kept in it) and its kits' objects are
    in no world while he is away: the games that saw him leave keep them as
    they were, the games that loaded a world meanwhile build them anew
    (0x47d5fa35: seven games out of step at the return of two players; the
    majority was the newer games, so the older ones were resynced). Needs a
    decision: reset a returning place's profile to its static values in
    every game (he gets his storage back; the server's count of what he
    used would be short), or let worlds carry the profiles of the places
    that are away (a world format change).
  - Open, not seen apart in any saved world: the own timers of artefacts,
    medkits and oxygen tanks, a weapon's state machine and scope.
