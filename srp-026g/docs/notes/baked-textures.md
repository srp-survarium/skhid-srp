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
  20 textures for each painted weapon (5 surfaces × d, n, f, s) the first time it is seen. They are
  not new in revision 32.
- **Not the cause:** "cook of '' [class 107] failed" is the fresnel query of a surface that has no
  fresnel layer. "new id = N" is a texture loaded from the folder (a cache hit), not a bake.
  remove_unwanted_files deletes only past 1 GiB, or past 350/50 MB when the disk has less than 50 MB
  free.

## The rules (patch 70)

- The folder is kept from game to game. It is deleted (the stock delete) only when its stamp
  `srp_baked_textures.txt` names another exe revision, or when it has no stamp. Every release starts
  with an empty folder, so content that changes a camouflage cannot leave stale bakes.
- At the start, every whole DDS file in the folder goes back into the baker's list. A file cut short
  is deleted. So are the files past 512 or past 512 MB (half of the stock 1 GiB limit; stock trimming
  goes on as before).
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
- The CRC-32 the game runs over every baked or loaded file (0x42050, a byte a step) is the same value
  computed eight bytes a step: about a third of the instructions. On the owner's game it was most of a
  bake burst's stall (16:37:47, 0.4 s).
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
  The line's "bake" is the ms the game's thread spent until the bake was issued (the draw's with
  it); its "compress" the ms of the readback.
- **A frame's budget:** done bakes are completed while the frame's 6 ms last (each makes its texture
  from its DDS, runs its CRC, saves it; a frame's first pass completes one). The time of a bake
  completed for its place counts in it too. The rest wait in their places for the next frames,
  oldest first; a done bake behind one the GPU has not done waits for it. A frame is
  engine_world::tick (once a frame, the main loop's); a dispatch with no tick for 100 ms (apc's wait
  loops) starts a new budget.
- A bake the GPU has had 1 s is read back waiting, and a line goes to `srp_patches.log`.
- The model instance a bake is for is held until the bake ends: a player who leaves meanwhile only
  delays its release. Patch 70's owner row of its path ends with it (at most 8 bakes later, or 1 s).
- The save takes the DDS the bake owns, MT memory, with no copy: the save's descriptor frees it once
  written. The stock save copied the whole DDS into MT (a 5.6 MB copy ran MT out, 2026-10-28). With no
  MT memory for the DDS, the bake fails, as a bake that fails does (patch 70 makes its waiters again).
  With no memory for the descriptor, the texture is used and not saved (the stock save would write
  through 0).
- A bake whose levels cannot be mapped (the device lost), whose level 0 finds no GPU memory, or whose
  compressor does not fit its DDS fails its query the same way, once (never tried again), and is
  logged.
- At game::exit (its start: every exit passes it) every bake on the GPU is completed, waiting; bakes
  after it are the stock ones.
- `srp_patches.log`: the game's threads all log; a line now goes out whole, one at a time, and is
  not cut before 512 bytes (in the 2026-11-08 run lines came out cut and mixed).
- Textures only: nothing a match computes changes. Games with and without the patch play together.

## Not done

- A first load of a kept file is still on the main thread: a joiner's kit read from the folder took
  ~40 ms for 12 textures (15:11:27). Spreading them over frames, or moving them behind the loading
  screen, is a separate change.
- The stock bake (after game::exit, another thread's bake while the places are one thread's, or a bake
  whose frame patch 121 does not read) still waits for the GPU, and its save still copies the DDS.
- Patch 121 as reworked is not measured in a game: the owner compares the frame times (over 100 ms, the
  longest) and the "camouflage/decal cook" and `bake_async` lines with `streamed_textures` emptied, in
  the match load of 2026-11-08, before and after. The 6 ms budget and the 8 places are the knobs.
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
  its start. Both games trim the folder past 1 GiB as before.
