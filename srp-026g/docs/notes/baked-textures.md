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

## Not done

- A first load of a kept file, and every bake, is still on the main thread: a joiner's kit read from
  the folder took ~40 ms for 12 textures (15:11:27). Spreading them over frames, or moving them behind
  the loading screen, is a separate change; see the PR.
- "camouflage_decal_texture_type_baker_cook: gbuffer_render_stage not found!" (15:41:05, ×28): a bake
  asked for a surface whose material has no gbuffer stage yet (translate_query 0x12D5C5..0x12D64F: the
  material's stage 1 through 0x12C6A0). It fails at once, which costs nothing, and the model's bakes come
  again when it is asked again. Here that was 0.6 s later: 53 bakes, 1.3 s back to back, then a FULL
  RESYNC 5 s after. A stall that long can lose the 250 ms relays the game must ack, and the match server
  takes an unacked relay as a game out of step (match-server game.rs, its header), so the resync is
  LIKELY the stall's.
- The replay viewer (a second game sharing the folder) no longer deletes the running game's folder at
  its start. Both games trim the folder past 1 GiB as before.
