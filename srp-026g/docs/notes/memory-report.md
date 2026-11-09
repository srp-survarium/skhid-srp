# Memory reports (client patch 126)

The game is a 32-bit process. At its start the engine splits its 4 GiB of address space into fixed
arenas, and the video driver gets what is left. A full arena ends the game ("not enough memory for
arena [...]"), and so does a driver that has no room left. We have sized the arenas from a few crash
dumps. These reports show how close players' games really come to those limits. The RE behind them
is in `client_patch.md` section 3.76.

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
  - No high-water mark. Nothing in the engine keeps one: a fixed-base mspace's max_footprint is
    its size from the start. The two snapshots below are the measurement.
- **When.** On events, never on a timer:
  - **loaded**: when a match connection's first world is in (`world_load.loaded`, after patch
    97's receipt). By then the level, its camouflage/decal bakes and the players' textures are
    loaded, which is the load's peak. Later worlds on the same connection (resyncs) report
    nothing;
  - **ended**: when the match closes on the game (close_current_match's entry, any reason,
    before patch 97 forgets the connection), its world still there: the match's peak. Only for
    a connection that reported loaded, and only once (a second close reports nothing).
- **Where.** Each snapshot goes to `srp_patches.log` as one line for the process and one for
  each arena, so a crash report carries the last one. It is also sent to the match server on
  the generic client-report channel.
- **The channel** (`srp_validate::report`): one typed `Report` enum (memory now; the module
  report can become a second kind), one carrier, one send path and one owner on the server.
  - The carrier is 0xE0 with the impossible chunk prefix `ff ff ff ff ff ff 03 00`, the match
    id (its seed), the kind, the part and the number of parts. A memory report takes 2 parts
    (10 entries each, at most 4). Each part is plain and resent until acked, like the receipts;
  - the client sends all of a report's parts at once or none of them
    (`vostok::packet_recovery::Connection::tell`). Nothing waits and nothing is retried: a
    snapshot is of its moment, and the log line keeps it;
  - the server (`match-server` `client_report`) logs each part once per connection
    (`Carrier::key`: kind, moment, part), for its own match only, whatever the place's stage. A
    game given its farewell still has its end heard. The lines go to target `memory`, which is
    `logs/memory.log` (16 files of 4 MB, about 2.5 KB a player a match);
  - nothing else: no ban, nothing a player sees, nothing in the db. `release/moderate memory`
    reads the log's end and lists, tightest first, each arena's least room left and least
    largest free block, and the address space's least largest free region, each with who, when
    and at which moment.
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
- **An old server** reads the carrier as a chunk whose offset (FFFFh) is past its total, and
  ignores it (`sent what is no message`). The empty 0xE0 is the leave, so play is not affected,
  and the release order does not matter.
- **Replays**: the carriers are datagrams like any other. A server replay of a recording logs
  them again under its own logger. They change nothing a match computes, so there is no
  `simulation_changes.tsv` row.
- **Threads**: the engine's arenas have no lock, and its own size queries walk a heap another
  thread may be changing (internal_mallinfo can loop for ever on a chunk being split). So the
  report calls none of them. It reads only words inside each arena (committed and readable by
  `VirtualQuery`, never freed while the game runs), walks at most 2^20 chunks, and reports a
  walk that meets something that is not a chunk as unknown.
- **Cost**: two `VirtualQuery` walks and the heap walks, twice a match, on the game's thread,
  both at moments that are a scene switch anyway. Not measured in a game.
