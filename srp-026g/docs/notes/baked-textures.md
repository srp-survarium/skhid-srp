# Baked camouflage and decal textures

Players reported short freezes in matches at a steady ~100 fps (2026-10-25, revision 33). Exe patch 70
fixes them; the RE is in `client_patch.md` section 3.57.

## The cause

- **Stock: every game baked everything again.** `game::game` (0x1E9455) and `game::exit` (0x1EC832)
  delete `Documents\Survarium\streamed_textures` (0x1E8DA0: SHFileOperationA, FO_DELETE). The baker's
  list of saved textures exists only in memory. So the first time a game sees an item with camouflage
  or decals, it bakes that item's textures on the spot (d, n, f and s for each surface: about
  5–20 ms each, plus the CRC and the save). In a match that happens when a player joins or enters with
  a kit nobody in this game has seen yet.
- **Stock: one texture baked once per player object.** A texture goes into the list only when its bake
  is done (process_compression 0x12EC40). Every query that arrives before then bakes the same texture
  again. A player has one player object for each kit (three since #148), and their kits often share
  items. Each burst therefore baked each texture 2–3 times: 15:06:54, 18 bakes for 6 files; the whole
  first match, 115 bakes for 62 files (the owner's log, 2026-10-25).
- **Everyone at once.** Every game builds a joining player's objects at the same tick, so every game
  bakes or loads the same textures in the same frame. The freeze is the same moment for everyone,
  including in the countdown when players come in (the owner's log: 15:27:23.62, one player's
  armour loaded three times and his four painted weapons, ~300 ms, 1.3 s before the start).
- **Revision 32 made it worse for new players** (#400, server only): a new account has three kits
  wearing the same seven items, so a newcomer who joins costs three times the same bakes. The delete
  and the per-object bakes are stock. Our weapon camouflage and decals (patches 44, 47) bake up to
  20 textures for each painted weapon (5 surfaces × d, n, f, s; with a camouflage and no decal,
  d and s only since 2026-11-11, 10) the first time it is seen. They are
  not new in revision 32.
- **Not the cause:** "cook of '' [class 107] failed" is the fresnel query of a surface that has no
  fresnel layer (and, since 2026-11-11, a painted weapon's normal query: the weapon keeps its own). "new id = N" is a texture loaded from the folder (a cache hit), not a bake.
  remove_unwanted_files deletes only past 1 GiB, or past 350/50 MB when the disk has less than 50 MB
  free (3 GiB and 50 MB since 2026-11-12, "The folder's size" below).

## The rules (patch 70)

- The folder is kept from game to game, and from release to release. It is deleted (the stock delete)
  only when its stamp `srp_baked_textures.txt` ("Skhid baked textures, format F, inputs H") names
  another bake format or other game data, or when it has no stamp:
  - **The format** (`BAKE_FORMAT`, 2 since 2026-11-11: a weapon's bake without the camouflage's
    weave, `weapon-camo.md`; 1 was revision 54's) is what the bake code makes. A change of that code that makes
    other bytes bumps it (every player bakes once more). The test `the_bake_code_is_this_format`
    fails at any change of the files it names (patch 121's bake, `vostok::baker`, the weapons'
    camouflage and decals) until its hash is set again, so each such change is weighed. An exe patch
    of the baker's own code needs the same.
  - **The game data** (`inputs`, 16 hex digits) is a fingerprint of what a bake reads that its file
    name does not say. The name says what differs from item to item (the model and surface, the
    camouflage, the decals' values); the rest is the same for every bake of one game data: the
    models, material instances, textures, shaders and the camouflage and decal items
    (`gameplay/items/upgrades`). `resources.db` never changes; each `resources.db.patch_<n>` is
    released once and never changed. So at the start every patch's file table is read (not its
    files) and the fingerprint folds, for each patch with a file in those folders, its number and
    those files (path, size and the table's CRC). A patch of Flash movies, item stats, maps or sounds
    leaves it as it was. Of the patches 24 to 54, 14 changed it (27, 28, 30, 31, 38-41, 43-45, 49,
    51, 54) and 17 releases would have kept the folder.
  - A table this walk does not follow (anything `resources-db` refuses, a hard link) is hashed whole,
    and a patch that cannot be read counts as one: either only deletes the folder too often. With no
    `resources.db` where the game mounts it from (`..\..\` of its working folder) nothing is known:
    deleted and not stamped.
  - Revision 54's stamp ("Skhid baked textures, revision 54", before formats) was format 1's and
    was kept while the format was 1; since format 2 it is no stamp, and its folder is deleted.
  - `srp_patches.log` gets a line at the start: kept or deleted, and the stamp.
- At the start, every whole DDS file in the folder goes back into the baker's list. A file cut short
  is deleted. So are the files past 3 GiB less 128 MiB, or past the list's room less 512 records
  (room for the game's own bakes before the trim lets any go). Until 2026-11-12: 512 files and 896 MB
  under the stock 1 GiB; until 2026-11-09 512 MB, which deleted half of a 20-player match's folder,
  0.9-1 GB in ~500 files, at every start: the next game baked ~400 textures again, fps-measure
  `r54-warm`, 398 bakes against the cold run's 831.
- **The folder's size: 3 GiB** (2026-11-12). Stock keeps 1 GiB, about one 20-player match's bakes, so
  a game with its folder kept still baked: the owner's game of 2026-11-12 (revision 56, 28 minutes,
  one lobby visit and three matches) baked 415 textures, loaded 300 from the folder and deleted 295
  files, some baked minutes before; players reported the microfreezes of those bakes. Now:
  - the baker's list is 4096 records of the patch's own pages, moved there empty at the game's
    start. Stock it is 1024 records inside the baker, about 2 GiB of files, and the game writes the
    1025th past them;
  - after each bake the oldest records with no load under way go, each with its file, while the
    list's files are over 3 GiB or the list is full (the game's remove_unwanted_files, whole in
    Rust: its own walk skipped every other record and, past half the list, read records no longer
    there). With less than 50 MB free on the folder's drive: down to 50 MB, the game's rule;
  - `srp_patches.log`, at most a line a second: "bake_cache: the oldest baked textures let go: N;
    the folder holds M MB in K files, at most 3072 MB and 4095 files".
  Disk space only: the start lists the files' names and sizes and reads each one's 128-byte header;
  the textures held in memory are as before (256, 256 MB).
- A kept file's record has CRC 0. Its first load is used and takes the CRC it read; after that, stock
  rules apply.
- While a texture's bake or load is under way, another query for the same texture (same file name)
  starts no bake and no load: it waits and gets the same texture when it is made. At most 16 queries
  wait on one; the 17th bakes as stock does.
- If the bake or load ends any other way (an error, a requery, the texture not where it was expected),
  the waiting queries are made again, as if they had just arrived.
- A bake or load still listed after 10 s (none is known that never ends) is taken over by the next query
  for its texture, along with its waiting queries.
- A texture loaded from the folder is held for the game: the next query for it (a respawn, a kit
  change, a rejoin) gets it at once, with no file read, no CRC and no new texture. The owner's log,
  15:40:43, 15:40:57 and 15:43:59: one player's armour was read from disk again at each respawn. At most
  256 textures and 256 MB of their files are held; the least recently used is let go first. A baked
  texture is held from its first load.
- The CRC-32 the game runs over every baked or loaded file (0x42050, a byte a step; also the texture
  streamer's check of an archive mip, texture_mip_cook::on_dds_body_ready 0x3DD4A0, when the mip's
  descriptor asks for it, +13Dh) is the same value computed faster. On the owner's game it was most
  of a bake burst's stall (16:37:47, 0.4 s), and still 20-35 ms of render-thread CPU at a time in the
  2026-11-09 profile with the sliced step. Where the processor has carry-less multiplies (PCLMULQDQ:
  Intel since 2010, AMD since 2011; CPUID asked once) 64 bytes a step are folded (Intel's 2009 method,
  the constants of crc32fast): 0.48 instructions a byte in Unicorn against 3.25 sliced and 11 stock;
  12.5 GB/s against 2.3 sliced and 0.5 stock on a Ryzen 5900X (i686 build). Without them, and below
  128 bytes, eight bytes a step through tables as before.
- Textures only: nothing a match computes changes. Games with and without the patch play together.

## Bakes off the GPU's clock (patch 121)

A player's report (2026-11-05): with `streamed_textures` emptied, every player joining a room or a
match makes a hitch while their items bake. A RAM disk does not help: the save was already
asynchronous, the cost is the GPU work the game waited for.

- **Stock, the game's thread waits for the GPU at every readback** (`client_patch.md` 3.57, "The
  bake's GPU round trips"). A texture goes GPU -> CPU -> GPU -> CPU -> GPU: the 2048x2048 target is
  read back (16 MB, blocking), uploaded again as the compressor's level 0, and each of its 10
  compressed levels is read back, blocking each time. Diffuse bake ~15 ms against ~3 ms for the normal
  and smoothness bakes of the same size, right after it: LIKELY the frames still queued, waited for.
- **The first version gave nothing** (measured 2026-11-08: 19 bots in recorded live players' gear and
  one headless game, live content, `streamed_textures` empty; ~800 bakes in the match load, 9-12 s).
  Frames over 100 ms: 7.7 s on main, 7.2 s with it; the longest ~1.3 s in both. It had 8 bakes on the
  GPU and 8 queued, and past them the stock bake: at least 474 of 768 bakes took the stock bake, which
  then waited behind the GPU work. It finished 2 done bakes a dispatch (~36 a second at 18 fps). The
  game also ran out of its 512 MB managed resources arena or its 8 MB resources helper arena now and
  then in that load ("not enough memory"), so what a burst holds matters.
- **The second version crashed in a burst** (the same load, 2026-11-09): it put off a bake with no place
  before its draw and made its query again once a place was free (finish_query's requery). Every
  requery made the query again (translate_query and its texture queries), and in the burst their
  allocations piled up: "a bake postponed: the GPU's places are taken; postponed now 1; 38 more
  postponed since the last line", and 62 bakes later "out of memory in arena 'resources helper
  allocator'" (8 MB, 93.5% used). Main got through the same load. That design is gone: nothing is put
  off, nothing is asked again.
- A bake is issued in the frame it is asked for, and nothing waits:
  - level 0 is a GPU copy of the target (the same R8G8B8A8_UNORM texels): no readback, no upload;
  - the levels are drawn, compressed and copied to stagings, the last copy flushed, none mapped;
  - process_compression ends with its query not finished; patch 70's waiting queries go on waiting;
  - no DDS memory is held while the GPU has the bake: the DDS is allocated (MT) when it is read back.
- At most 8 bakes are on the GPU (their stagings, up to 5.6 MB each). **With all 8 taken, the oldest is
  completed on the spot** when the next bake is issued: read back (waiting only if the GPU has not
  done it yet; it usually has, 8 bakes later) and its tail run, and the new bake takes its place. So a
  bake never waits for a place, never takes the stock bake for want of one, and is never asked for
  again; a burst is throttled by the oldest bakes completing. A line a second goes to `srp_patches.log`
  ("the GPU's 8 places taken: the oldest completed for a new bake (read back waiting / the GPU had done
  it, ms N)"), with the count of the others in between.
- The immediate context is one thread's: the places are the thread's that issued the first of them,
  until all are free. A bake of another thread meanwhile takes the stock bake (logged, a line a
  second), as main does; that thread's dispatches never touch the places' bakes.
- The resource system's "postponed" (result 2) is no "later" for this query: finish_query stores it
  and the query ends there, unanswered (0xF0F59, 0x109BD0, 0x103140); nothing asks again. Requery
  (4) asks again, from translate_query on, and its allocations are what ran out in a burst. An
  unfinished query held by the cook is what "postponed" means in the engine (0.10b: a cook that
  finishes later): that is what a bake on the GPU is.
- Each resources dispatch on the bakes' thread asks the oldest bake's last staging without waiting
  (map2D's no-wait byte, as the occlusion manager polls). Once the GPU has done it, the DDS is
  allocated, the levels are copied into it and the game's own tail completes the bake: the texture,
  the slot, patch 70's sharing, the save, the record, the query, the "camouflage/decal cook" line.
  The line's "bake" is [esp+40h] + argument 10 at 0x12F4E4 (ESP S-24h there): the ms process_
  compression ran until the bake was issued (resume writes it at +40h) and render_bake's. It holds no
  GPU wait: a bake 3 s on the GPU logs 3.5 ms in Unicorn (1 ms issue, 2.5 ms draw), as stock's sum.
  In the 2026-11-09 runs it read 0.3-1 s for fresh bakes and up to 65 s, so that time went by in the
  issue or in the draw; which one, and why, is not known. An issue (with its draw) of 250 ms or
  more is logged now, a line a second: "issued in N ms after a draw of M ms". Its "compress" is
  the ms of the readback.
- **A frame's budget, on the thread that ticks frames:** done bakes are completed while the frame's
  6 ms last (each makes its texture from its DDS, runs its CRC, saves it; a frame's first pass
  completes one). The time of a bake completed for its place counts in it too. The rest wait in their
  places for the next frames, oldest first; a done bake behind one the GPU has not done waits for it.
  A frame is engine_world::tick (once a frame, the main loop's); a dispatch with no tick for 100 ms
  (apc's wait loops) starts a new budget.
- **The bakes are not the frame thread's** (measured 2026-11-09, the third version: 19 bots, ~800
  bakes). No pass ever saw a bake for minutes: not one "the GPU had it 1 s" line, bakes completed
  only for a new bake's place or at the end of a burst's dispatch, and the last 8 of a burst stayed
  on the GPU until the next burst, up to ~65 s (gear without its camouflage meanwhile). The main
  thread dispatches three times a frame (engine_world::tick 0x13CE0, 0x13D0E, 0x13D1F through
  0xE5AB0) and the cooker thread at least every 300 ms (0x1BEA0, 0x103398), so the bakes' thread
  is neither: it dispatches only while its callbacks come. Which thread it is, is not read from the
  code yet (the baker's translate thread is the one its static constructor ran on, 0x12CD1B), and
  `srp_patches.log` now names it. So: a dispatch of a thread that does not tick frames (or any,
  before the first frame) completes all of its bakes before it ends, waiting for the GPU where it
  has not done them; that thread may not dispatch again for minutes. The pipelining within a burst
  stays (8 on the GPU, the oldest completed for a new one); only the last of a dispatch's bakes are
  waited for. Lines, one a second with the count between: "thread X ticks no frames (the frame
  thread Y): its N bakes completed at its dispatch's end, M read back waiting", and, on the frame
  thread, "the GPU's bakes are thread X's, not the frame thread Y's".
- A bake the GPU has had 1 s is read back waiting, and a line goes to `srp_patches.log`.
- The model instance a bake is for is held until the bake ends: a player who leaves meanwhile only
  delays its release. Patch 70's owner row of its path ends with it (at most 8 bakes later, or 1 s).
- The save takes the DDS the bake owns, MT memory, with no copy. The stock save copied the whole DDS
  into MT (a 5.6 MB copy ran MT out, 2026-10-28). The descriptor frees it when ~query_result destroys
  it (0xF1135 calls ~save_descriptor 0x10A050, which mt_frees its +0 when +8 is set), after the device
  manager wrote it; the device manager's queue is the load's reads'. With no MT memory for the DDS,
  the bake fails, as a bake that fails does (patch 70 makes its waiters again). With no memory for
  the descriptor, the texture is used and not saved (the stock save would write through 0).
- **The saves not written yet are bounded** (measured 2026-11-09, 712dc41e5 with -rh_allocator=32
  -render_allocator=320: 458 bakes in 11 s, 774 MB of DDS, then MT ("global multithreaded") at
  99.3%, an assertion and a crash; stock in the same load did 768 bakes, 1229 MB, and lived). A
  bake's per-save MT is the same as stock's, so the difference is how many saves wait at once. At most
  64 bakes' saves, 128 MB, are pending (`save.freed` 0x10A050 takes each off when its descriptor
  goes); past either, a bake's texture is made and used but not saved, its DDS freed at once, and a
  line goes out ("used, not saved: N saves not written yet hold M MB of MT"). It is baked again next
  game.
- The managed resources: a bake on the GPU holds its model instance and its levels' stagings (up to
  5.6 MB) until it completes; at most 8 bakes. Level 0 and the mips are let go at the issue, the bake
  targets by on_textures_loaded as stock, and the textures made are the game's. So the patch keeps no
  more than 8 bakes' stagings and models alive past stock (the 99.5% managed of 2026-11-09 is the
  load's; main ran out of it in the same load at 124-184 cooks without the raised arenas).
- A bake whose levels cannot be mapped (the device lost), whose level 0 finds no GPU memory, or whose
  compressor does not fit its DDS fails its query the same way, once (never tried again), and is
  logged.
- At game::exit (its start: every exit passes it) every bake on the GPU is completed, waiting; bakes
  after it are the stock ones.
- `srp_patches.log`: the game's threads all log; a line now goes out whole, one at a time, and is
  not cut before 512 bytes (in the 2026-11-08 run lines came out cut and mixed).
- Textures only: nothing a match computes changes. Games with and without the patch play together.

## Not done

- The held textures' cap (256 textures, 256 MB) is not lowered. In the bake-stress loads (20 players)
  a match loads 146-201 distinct textures from the folder, 290-410 MB: 256 MB already holds about 60 %
  of them, 64 MB 3-4 players' kits. A respawn not held reads its kit again (~40 ms for 12 textures on
  the owner's game, a third of it the CRC, now about a sixth of that). On native Windows a held D3D11
  texture is video memory, not the game's address space, so what a lower cap frees there is not
  known. To measure before changing it: the address space and the memory report (patch 126) after
  several matches at 256 and at 64 MB, and the respawn reloads ("new id" lines after the match's
  first minute) at each.

- A first load of a kept file is still on the main thread: a joiner's kit read from the folder took
  ~40 ms for 12 textures (15:11:27). Spreading them over frames, or moving them behind the loading
  screen, is a separate change.
- The stock bake (after game::exit, another thread's bake while the places are one thread's, or a bake
  whose frame patch 121 does not read) still waits for the GPU, and its save still copies the DDS.
- Patch 121 with the drain is not measured in a game: the owner compares the frame times (over 100
  ms, the longest), the "camouflage/decal cook" lines (none long after its burst) and the
  `bake_async` lines (which thread the bakes are, how many each dispatch waited for) with
  `streamed_textures` emptied, in the match load of 2026-11-08. The 6 ms budget and the 8 places are
  the knobs. If the bakes' thread turns out to have a regular point of its own, its pass can move
  there and wait less.
- Which thread bakes, and why it dispatches only while its callbacks come, is not read from the code.
- An issue's own cost (level 0's copy, the mips, the compress draws, and the oldest's completion when
  the places are full) is not budgeted a frame: a burst is throttled by the completions alone.
  Bounding it would mean holding the query back, which is the postponing that ran out of memory.
- "camouflage_decal_texture_type_baker_cook: gbuffer_render_stage not found!" (15:41:05, ×28): a bake
  asked for a surface whose material has no gbuffer stage yet (translate_query 0x12D5C5..0x12D64F: the
  material's stage 1 through 0x12C6A0). It fails at once, which costs nothing, and the model's bakes come
  again when it is asked again. Here that was 0.6 s later: 53 bakes, 1.3 s back to back, then a FULL
  RESYNC 5 s after. A stall that long can lose the 250 ms relays the game must ack, and the match server
  takes an unacked relay as a game out of step (match-server game.rs, its header), so the resync is
  LIKELY the stall's.
- The replay viewer (a second game sharing the folder) no longer deletes the running game's folder at
  its start. Both games trim the folder past 3 GiB (each by its own list).
