# The memory profiler (client patch 127)

The client is a 32-bit process. The engine reserves fixed arenas at its start
(`memory::allocate_region` 0xE1120, `platform::allocate_arenas` 0xE9580) and a
full arena ends the game (`out_of_memory_with_crash` 0xE4D70): "ai" live on
2026-10-17, "render" live on 2026-11-08 (fragmentation: its largest hole 1.59 MiB
of 37.7 MB free), "resources helper allocator" and "render" in the bake stress
test (PR #1556). What fills an arena was known only from one full dump walked by
hand. Patch 127 records it while the game runs, for the owner's own game.

## Switching it on

A tool for the owner, and for a worker whose local test needs it; never for
players, who never set its variable. It is in every build and dormant: an off
game runs the game's own allocator byte for byte (below, "What it costs"). The
game's environment variable `SKHID_MEMORY_PROFILE`, read once when the arenas are
made, switches it on: `on` or `1` (any case); anything else, or nothing, leaves it
off. The same way as `SKHID_CRASH_DUMP=full` (`client/bugtrap/README.md`, "Dump
contents"), and the two go together:

- **The owner's own install** (Linux): `SKHID_MEMORY_PROFILE=on ./Skhid-local.sh launcher`
  (with `SKHID_CRASH_DUMP=full` for a full dump if it crashes). The launcher, the
  updater and Wine pass the environment on to the game.
- **Windows**: `set SKHID_MEMORY_PROFILE=on` in the `cmd` that then starts `Skhid.bat`.
- **A worker's local clients** (`srp-026g/scripts/local-clients/run.py`): off unless
  run.py itself is started with it (`SKHID_MEMORY_PROFILE=on python3 run.py ...`),
  as run.py's environment goes on to the clients; it sets nothing of its own.

## What it writes, and when

`srp_memory_profile.txt` in the game's user data folder (`Documents\Survarium`,
the game's `get_user_data_directory` 0x18BD0; under Wine
`<prefix>/drive_c/users/<user>/Documents/Survarium/`). The file only grows: each
start of the game appends a `run` line, then its snapshots. A snapshot is taken:

- when a match's world is in (`world_load.loaded`, every one: the first and a resync's);
- when a match closes on the game, its world still there (`match_recovery.close`);
- when a fixed arena's heap has no room, before `out_of_memory_with_crash` ends the
  game (`memory.out_of_memory`): the arena and the bytes asked;
- when **Scroll Lock** goes down (in the lobby or a match; the gates poll it, the key still goes on to the game).

Each snapshot has:

- the address space's free bytes and largest free region, and the engine's video budget;
- each arena of the engine's list (0xA808FC): its name, place and size, and its use
  as patch 126's reader finds it (a doug_lea heap walked chunk by chunk: used and
  largest free; the managed and the multithreaded arenas' counts), and a heap's free
  blocks by size (powers of two: how fragmented it is);
- each arena's tracked blocks: live bytes and blocks, the peak, all asked since the
  start, the blocks the table had no room for, frees of blocks not tracked;
- each call site of each arena: live bytes and blocks, its own peak, all it asked;
- the address space region by region (`VirtualQuery`: base, size, state, protection,
  type, reservation).

## Reading it

```sh
python3 srp-026g/scripts/memory-profile/analyze.py <prefix>/drive_c/users/<user>/Documents/Survarium/srp_memory_profile.txt
```

It reads the file's last run (`--run N`), prints a line per snapshot, then the last
snapshot (`--snapshot N` or `all`) in full: the address-space budget (fixed
arenas, the profiler's own tables, thread stacks, images, mapped files, other
private memory such as the driver's and Wine's heaps, and what is left free for
the video driver, with its largest regions), then each arena (`--arena render`
for one): its use, its holes, and its top call sites by live and by peak bytes
(`--top N`), each named from the re-kit (`tools/re-kit/v026g`): the nearest
function label at or below the return address, `?` for an unreviewed one, "far
past it" when the label is more than 16 KiB below (a function with no label;
`lookup.py` and the disassembly tell).

## How it works

Every block of the engine's heaps is made and given back by dlmalloc 2.8's three
entries: `mspace_malloc` (0x101330), `mspace_realloc` (0x1005A0) and `mspace_free`
(0x1015E0). Each doug_lea arena (render, ai, the helpers, ...) is one mspace at
its start; ptmalloc3's "global multithreaded" arena is several in its region.
ptmalloc3 gives a block mapped on its own (one too large for its arena's heap) back
to the region itself, not through mspace_free: in `pt3free` (0x106FC0, at
0x10700A) and its `free_starter` hook (0x106D90, at 0x106DAC). Each of these five
places gets a gate (`memory_profile_026g.asm`, five `code` hooks), and
`out_of_memory_with_crash`'s entry (0xE4D70) a lohook, all six **installed on
demand** (`install = "on_demand"`, hooks_026g.py): linked into the arena, but the
exe keeps the game's bytes at their sites.

`memory.arenas_ready` (allocate_region after `memory::initialize`, 0xE12BF: every
arena is reserved and its heap made) reads the variable. Off, it returns and that
is all. On, it reserves the tables with `VirtualAlloc` (never an engine heap),
records where each arena lies, and writes the six jumps
(`vostok::hook::install_on_demand`: each site's bytes checked to be the game's
first, the page made writable for the write and given its protection back, the
instruction cache flushed). It does that while no other thread runs the game's
code: allocate_region runs in engine_world's constructor on the main thread, and
the exe's threads are all made by threading::spawn (0xE3770) or the CRT's
_beginthreadex (0x422C69, for boost.asio, the renderer and Scaleform), none
reachable from the constructor before that call (the crash reporter's and Wine's
own threads never run the game's allocator). A site that is not the game's bytes
writes nothing and leaves the profile off, with a line in `srp_patches.log`.

A gate calls the stock body itself (its replayed first instructions and a jump
back) and tells the Rust patch through two generated entries:
`memory_profile.freeing` before a block is given back (mspace_free,
mspace_realloc's old block, ptmalloc3's mapped blocks) and
`memory_profile.allocated` after one is made (mspace_malloc, mspace_realloc's new
block). The entries save and restore the whole machine; what the game gets is the
stock body's result and registers.

**The snapshot key** is read by the gates themselves: an allocation looks at
Scroll Lock (user32's GetAsyncKeyState, resolved when the profile goes on) at most
every 200 ms, and a press takes a snapshot on that thread. No keyboard hook.

**Who asked.** The site of a block is the first return address out of the
allocator layers, from the ESP of the game's call. Where the call came from a
known wrapper, its frame is unwound by the offsets `fields_026g.tsv` gives
(`vostok::memory::game_call`), up to six levels:

| wrapper | its call of | return |
|---|---|---|
| doug_lea_allocator::malloc_impl 0xE4F90 (665 callers) | mspace_malloc | 0xE4FB0 |
| doug_lea_allocator::realloc_impl 0xE5050 | mspace_malloc, mspace_realloc | 0xE50E4, 0xE5100 |
| doug_lea_allocator::call_malloc 0xE5120, call_realloc 0xE5140 (the virtuals) | malloc_impl, realloc_impl | 0xE5131, 0xE5156 |
| pt3malloc 0x106EF0 | mspace_malloc | 0x106F70 |
| pt3realloc 0x107070 | mspace_realloc | 0x10710A |
| mt_allocator's malloc_impl 0x1C6A0 | pt3malloc | 0x1C6B9 |
| mt_allocator::call_malloc 0xEB530 | pt3malloc | 0xEB550 |
| mt_allocator's realloc 0xEB470 | pt3realloc (or pt3malloc by its tail jump) | 0xEB4F8 |
| mt_allocator::call_realloc 0xEB590 | 0xEB470 | 0xEB5A5 |

A wrapper not listed shows up as a site of its own; it is one row more. A
place that inlined malloc_impl (about 50 more callers of mspace_malloc) is its own site.
dlmalloc's own calls of mspace_malloc and mspace_free (mspace_realloc's move and
its cut tail, mspace_memalign's; return addresses 0xFF160..0x101B40) are left out:
the outer call is the one counted. A block's bytes are its chunk's (the head word
before it), what the arena spends on it.

**The tables** (one spin lock, `vostok::Lock`, over all of them):

| table | slots | holds | when full (7/8) |
|---|---|---|---|
| blocks | 2 Mi x 8 bytes (16 MiB) | each live block's site | the block is counted to its arena as untracked; its free later as unseen |
| sites | 16 Ki x 44 bytes | each (arena, return address): live, blocks, peak, asked | a new site's blocks go to its arena's `other` line |
| text | 64 KiB | a snapshot's text before each write | written and refilled |

About 16.8 MiB of the address space, which the report counts as the profiler's own.

**Threads.** The gates run on whatever thread allocates. A gate takes the lock
only after the game's body made a block and before it gives one back, so a block
is in the table only while it is live and no other thread can be handed it first.
Nothing done under the lock reaches an engine heap or the engine (kernel32's
`VirtualQuery`, `CreateFileA`, `WriteFile` only), so a gate never waits on itself;
a lock taken again on its own thread would be a bug, and panics. A snapshot holds
the lock while it writes: the game's other threads wait for it at their next
allocation.

**What it does not see**: the managed resources allocator (its own allocator, no
dlmalloc; its arena's count is in the report), the process heap and the stack
allocators; ptmalloc3's memalign (its blocks' frees are counted as unseen); a
reallocation that fails keeps its old block, which the profile forgets (a full
fixed arena ends the game anyway). The heap walk is patch 126's: a heap another
thread changes under it is told as unknown for that snapshot.

## What it costs

- **Off** (every player's game):
  - bytes changed in the exe for it: none at the six sites (the game's own bytes,
    checked by `check_memory_profile_026g.py`); the arena holds the gates, the
    entries and the profiler (patches.code about 9 KiB, patches.data about 3 KiB);
  - instructions added on a hot path: none (the check counts the same
    instructions as the original's over its allocations, frees and reallocations);
  - hooks that run: `memory.arenas_ready` once at the start (one read of the
    variable), and the profiler's handler in two lohook chains that already exist
    for other patches, at a world loaded and a match closing (a few times a match:
    one call that reads a byte and returns).
- **On**: 16.8 MiB of address space; each allocation and free the gate's call of
  the stock body, an entry (the machine saved), the lock and a probe of the
  tables; a GetTickCount each allocation and a GetAsyncKeyState every 200 ms.

## The checks

- `check_memory_profile_026g.py <original exe>` (Unicorn): runs dlmalloc's own code
  on a heap made with the game's `create_mspace_with_base` over 1 MiB, its
  doug_lea_allocator registered as the only arena; the same allocations, frees and
  reallocations (direct and through malloc_impl) in the original and the patched
  exe. The built exe keeps the game's bytes at the six on-demand sites. Unset:
  nothing reserved, written or logged, every result, register, flag and heap byte
  the original's, and as many instructions. On: the tables reserved, the six jumps
  written (the pages made writable and given their protection back, the cache
  flushed), one line logged, every result, register and heap byte the original's;
  Scroll Lock down at a gate's poll takes one snapshot. Then an allocation the heap cannot hold: out_of_memory_with_crash's entry writes
  the snapshot before the game's own body goes on, and the analyzer reads it: the
  heap's use as its walk finds it, its holes, each site's live bytes as the
  chunks say (malloc_impl's caller, a reallocation's caller; none of
  mspace_realloc's own), the bytes asked, the regions.
- `cargo test -p patches memory_profile` (the tables, the text, the setting) and
  `cargo test -p srp-validate` (the heap walk's free blocks).
