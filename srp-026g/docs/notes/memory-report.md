# Memory reports (client patch 126)

The game is a 32-bit process. At its start the engine splits its 4 GiB of address space into fixed
arenas, and the video driver gets what is left. A full arena ends the game ("not enough memory for
arena [...]"), and so does a driver that has no room left. We have sized the arenas from a few crash
dumps. These reports show how close players' games really come to those limits. The RE behind them
is in `client_patch.md` section 3.76.

Revision 54's reports showed many players' free address space shrinking 30 to 80 MB a match
within a session (one from 374 to 33 MB free, its largest free region 5 MB), the arenas the same
size and about as full, and a restart giving it back; the owner's game under Wine stays at about
1 GB free. So from the next release a report also tells **where** the address space goes: what
every allocation outside the arenas is (a module, a file mapping, the driver's or a heap's private
memory), the largest of them named, the renderer the game draws with and the baked textures it
holds (`client_patch.md` section 3.79). Two reports of one game, a match apart, show what grew.

## The rules

- **What is reported.** A snapshot of the whole game, read once into typed values
  (`srp_validate::memory`):
  - the process: the free address space in all and its largest free region (`VirtualQuery` over
    the whole space), and the video budget the engine worked out at its start;
  - each arena the engine registered (its list at [0xA808FC], up to 32), in the engine's order:
    its name, its size and what is known of its use:
    - **heap** (every doug_lea arena: render, sound, network, survarium, ai, resources helper,
      resources unmanaged, ...): the bytes in use, the same figure as the engine's "used" in its
      out-of-memory dump, and the largest free block (top included). The largest free block is
      the biggest allocation that still fits, and it is what ran out in the render arena at
      88 % used;
    - **counted**: the managed resources (total less free, defragmented, so no free block to
      report) and the multithreaded region (the pages ptmalloc3 has taken from it);
    - **unknown**: an allocator kind we do not read (the process heap, the C runtime's), an
      arena that is not all readable, or a heap that changed while it was being walked.
  - **the census**: the address space every allocation takes, by `VirtualQuery`'s type: images
    (the exe and the DLLs), mapped views (file mappings, shared sections) and private memory
    (VirtualAlloc's: the arenas, the heaps, the drivers' own), each as its bytes reserved (taken
    from the address space, committed or not), of them committed, and its number of allocations
    (reservations, by `AllocationBase`). The arenas are in the private figures;
  - **the largest allocations outside the arenas**, the 20 largest by size (an allocation whose
    base is in a registered arena is the arena's, told above): its base, its type, its bytes
    reserved and committed, and the file's name for an image (`GetModuleFileNameA` of its base,
    else the section's file) or a mapping (kernel32's `K32GetMappedFileNameA`: none for a mapping
    of no file, shared memory or a driver's). The name is the path's last part, at most 32
    printable ASCII bytes;
  - **the client**: Windows or Wine (ntdll exports `wine_get_version`); the d3d11.dll the game's
    delayed import is bound to (none yet, or the folder it came from: a `dxvk` folder, the
    game's own DXVK of the Settings' Renderer; `system32` or `syswow64`, the system's, where
    `play_skhid.sh` installs DXVK in its Wine prefix; or another, a player's own beside the
    exe), and whether it has Direct3D's layered-device exports (`D3D11CoreRegisterLayers`):
    Windows' own d3d11 and Wine's have them, DXVK's 3.1.1 exports four functions, none of those;
    the baked textures: patch 70's cache's held textures and their files' bytes (at most 256 and
    256 MiB), and the baker's list of saved textures and their bytes (what the game baked or
    found in the folder, at most 1024).
  - No high-water mark. Nothing in the engine keeps one: a fixed-base mspace's max_footprint is
    its size from the start. The two snapshots below are the measurement.
  - Not told: the number of all the engine's textures alive (the render's resource manager is
    walked only under its lock), and what is inside the driver's allocations.
- **When.** On events, never on a timer:
  - **loaded**: when a match connection's first world is in (`world_load.loaded`, after patch
    97's receipt). By then the level, its camouflage/decal bakes and the players' textures are
    loaded, which is the load's peak. Later worlds on the same connection (resyncs) report
    nothing;
  - **ended**: when the match closes on the game (close_current_match's entry, any reason,
    before patch 97 forgets the connection), its world still there: the match's peak. Only for
    a connection that reported loaded, and only once (a second close reports nothing).
- **Where.** Each snapshot goes to `srp_patches.log` as one line for the process, one for each
  arena, one for the census, one for the client and one for each allocation told, so a crash
  report carries the last one. It is also sent to the match server on
  the generic client-report channel.
- **The channel** (`srp_validate::report`): one typed `Report` enum (memory now; the module
  report can become a second kind), one carrier, one send path and one owner on the server.
  - The carrier is 0xE0 with the impossible chunk prefix `ff ff ff ff ff ff 03 00`, the match
    id (its seed), the kind, the part and the number of parts. A memory report's entries are the
    process, the arenas, the census, the client and the allocations, 10 to a part: 4 parts with
    the engine's 16 arenas and 20 allocations (at most 6). Each part is plain and resent until
    acked, like the receipts;
  - the client sends all of a report's parts at once or none of them
    (`vostok::packet_recovery::Connection::tell`). Nothing waits and nothing is retried: a
    snapshot is of its moment, and the log line keeps it;
  - the server (`match-server` `client_report`) logs each part once per connection
    (`Carrier::key`: kind, moment, part), for its own match only, whatever the place's stage. A
    game given its farewell still has its end heard. The lines go to target `memory`, which is
    `logs/memory.log` (16 files of 4 MB, about 6 KB a player a match). A part's entries are
    split by ` | `: `process free <bytes> largest <bytes> video <bytes>`, `arena "<name>" size
    <bytes> used <bytes> largest <bytes>` (or `counted <bytes>`, or `unknown`), `census image
    <reserved>/<committed> in <allocations>, mapped ..., private ...`, `client <windows|wine>,
    d3d11 <direct3d|dxvk> (<dxvk folder|system folder|elsewhere>)` (or `d3d11 unbound`) `, baked
    kept <count> of <bytes>, listed <count> of <bytes>`, `allocation 0x<base> <image|mapped|private>
    "<file>" size <bytes> committed <bytes>` (no file, no name);
  - nothing else: no ban, nothing a player sees, nothing in the db. `release/moderate memory`
    reads the log's end and lists, tightest first, each arena's least room left and least
    largest free block, and the address space's least largest free region, each with who, when
    and at which moment (it reads the process and the arenas; the other entries are for reading
    two reports of one game side by side).
- **Who.** Every game of revision 126 or later in a match.
  - Server-driven bots have no game and send nothing. A test bot's own game (a headless client
    on a bot place) is logged like a player's.

## Edge cases

- **Late join**: the joiner's first world is its loaded snapshot; its match's end, its ended
  snapshot.
- **A match never played** (handshake, level loading, kicked before its first world): it gets
  no loaded snapshot, so its close reports nothing.
- **Resyncs and rollbacks**: no report. The report does not depend on the world.
- **A link already gone at the end** (timeout, lost connection, 0xFD kick): the ended snapshot
  is written to the log only (`not told` in its line). It is not sent, since only a live match
  transport (state 2) sends.
- **The end on 0xEA or on leaving by the menu**: the report is queued before the game's own
  close, while the link is still up. Whether the link flushes it before the close is LIKELY but
  not proven (the menu's own 0xE0 at the same place gets through). The log has the snapshot
  either way.
- **A pool without room** for all the parts plus late join's 16 entries: nothing is sent, and the
  log line says so.
- **A report lost or refused**: nothing retries it; 0xE0's resend covers a lost datagram while
  the link lives. A part the server cannot parse (another kind, a bad body) falls through to the
  chunk parser and is ignored like any malformed chunk. A part of another match is not logged.
- **An old server** ignores a carrier that is alone in its datagram: it reads it as a chunk
  whose offset (FFFFh) is past its total (`sent what is no message`). In a datagram with the
  multi bit set, though, it cannot measure the carrier. It reads the E0 as an empty leave and
  the carrier's first byte as message id FFh, so the whole datagram is refused unacked, together
  with whatever rode with it (DF hash reports, DB/DC). The carrier is resent until acked, so this
  repeats for the rest of the match. **The server must ship in the same release as patch 126, or
  earlier, never after.** A server with this change measures the carrier with the codec's own
  `srp_validate::report::carrier_len` (`udp-transport`'s `carrier_len` calls it).
- **Replays**: the carriers are datagrams like any other. A server replay of a recording logs
  them again under its own logger. They change nothing a match computes, so there is no
  `simulation_changes.tsv` row.
- **Old reports**: a revision 54 game sends the process and the arenas only; the server reads
  them as before. An older server reads a part with the new entries as no carrier (its kinds are
  unknown to it) and ignores it, whether alone or in a multi datagram (it measures a part by its
  count).
- **Names**: an image's name is its module's (`GetModuleFileNameA`), which takes the loader's
  lock for a moment; a game thread in a DLL's load holds it for as long. A mapping's is the
  section's file, read by `NtQueryVirtualMemory` (no lock). On a Windows without
  `K32GetMappedFileNameA` (before 7, never seen) mappings have no name. The bytes are named
  as the system sees them; a module unloaded between the walk and its name has none.
- **Threads**: the engine's arenas have no lock, and its own size queries walk a heap another
  thread may be changing (internal_mallinfo can loop for ever on a chunk being split). So the
  report calls none of them. It reads only words inside each arena (committed and readable by
  `VirtualQuery`, never freed while the game runs), walks at most 2^20 chunks, and reports a
  walk that meets something that is not a chunk as unknown. A walk that ends cleanly can still be
  slightly stale: the top and its size are read before the walk, and another thread may move the
  top or split a chunk while it runs. The figures are close, not exact.
  The client's figures are two words of the delayed import's slot and the baker's list (read
  without a lock: a bake thread may push a record meanwhile) and patch 70's tables, under their
  own lock (held for a few instructions by the resource threads).
- **Cost**: one `VirtualQuery` walk of the whole address space (some thousands of regions) and the
  heap walks, then at most 20 file names and one GetProcAddress, twice a match, on the game's
  thread, both at moments that are a scene switch anyway. Not measured in a game.
