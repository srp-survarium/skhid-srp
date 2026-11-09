# The memory profiler (client patch 127)

The client is a 32-bit process. The engine reserves fixed arenas at its start
(`memory::allocate_region` 0xE1120, `platform::allocate_arenas` 0xE9580) and a
full arena ends the game (`out_of_memory_with_crash` 0xE4D70): "ai" live on
2026-10-17, "render" live on 2026-11-08 (fragmentation: its largest hole 1.59 MiB
of 37.7 MB free), "resources helper allocator" and "render" in the bake stress
test (PR #1556). What fills an arena was known only from one full dump walked by
hand. Patch 127 records it while the game runs, for the owner's own game.

## Switching it on

It is in every build, players' too, and dormant. The game's environment variable
`SKHID_MEMORY_PROFILE`, read once when the arenas are made, switches it on: `on`
or `1` (any case); anything else, or nothing, leaves it off. The same way as
`SKHID_CRASH_DUMP=full` (`client/bugtrap/README.md`, "Dump contents"), and the
two go together:

- **Linux, the owner's install**: `SKHID_MEMORY_PROFILE=on SKHID_CRASH_DUMP=full ./Skhid-local.sh launcher`
  (or `bash Skhid.sh launcher`). The launcher, the updater and Wine pass the
  environment on to the game.
- **Windows**: `set SKHID_MEMORY_PROFILE=on` in the `cmd` that then starts `Skhid.bat`.
- **Local clients** (`srp-026g/scripts/local-clients/run.py`): on for every
  client, next to `SKHID_CRASH_DUMP=full`; `SKHID_MEMORY_PROFILE=off` in run.py's
  own environment leaves it off.

On, `srp_patches.log` (beside the exe) says `memory profile on: snapshots to
srp_memory_profile.txt ...` with the bytes and place of its tables. Off, nothing
is written or logged.

## What it writes, and when

`srp_memory_profile.txt` in the game's user data folder (`Documents\Survarium`,
the game's `get_user_data_directory` 0x18BD0; under Wine
`<prefix>/drive_c/users/<user>/Documents/Survarium/`). The file only grows: each
start of the game appends a `run` line, then its snapshots. A snapshot is taken:

- when a match's world is in (`world_load.loaded`, every one: the first and a resync's);
- when a match closes on the game, its world still there (`match_recovery.close`);
- when a fixed arena's heap has no room, before `out_of_memory_with_crash` ends the
  game (`memory.out_of_memory`): the arena and the bytes asked;
- when the owner presses **Scroll Lock** (in the lobby or a match; the key still goes on to the game).

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
Each entry's first instructions jump to a gate (`memory_profile_026g.asm`, three
`code` hooks) that tests one byte, `SRP_MEMORY_PROFILE`:

- **0, off**: the replayed first instructions and a jump back. Nothing else.
- **1, on**: the gate calls the stock body itself and tells the Rust patch through
  two generated entries: `memory_profile.freeing` before a block is given back
  (mspace_free, mspace_realloc's old block) and `memory_profile.allocated` after one
  is made (mspace_malloc, mspace_realloc's new block). The entries save and restore
  the whole machine; what the game gets is the stock body's result and registers.

`memory.arenas_ready` (allocate_region after `memory::initialize`, 0xE12BF: every
arena is reserved and its heap made, only the main thread runs) reads the
variable, reserves the tables with `VirtualAlloc` (never an engine heap), records
where each arena lies, and only then sets the byte.

**Who asked.** The site of a block is the first return address out of the
allocator layers, from the ESP of the game's call. Where the call came from a
known wrapper, its frame is unwound by the offsets `fields_026g.tsv` gives
(`vostok::memory::game_call`): doug_lea_allocator::malloc_impl (0xE4F90, 665
callers) and realloc_impl (0xE5050) to their caller; ptmalloc3's malloc
(`pt3malloc` 0x106EF0) to mt_allocator::call_malloc (0xEB530) to its caller. A
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

- **Off** (every player): each call of the three entries runs 4 instructions more
  (a jump to the gate, the byte's test, its branch, a jump back: counted by
  `check_memory_profile_026g.py` over its calls, 1270 -> 1326 for 14 entries); the
  byte is in our arena's state, in the cache with the rest of it. No table, no file,
  no log line. The arena holds the code (patches.code about 9 KiB more: the gates,
  the entries, the profiler) and its state's static (about 3 KiB of patches.data).
- **On**: 16.8 MiB of address space; each allocation and free the gate's call of
  the stock body, an entry (the machine saved), the lock and a probe of the tables.

## The checks

- `check_memory_profile_026g.py <original exe>` (Unicorn): runs dlmalloc's own code
  on a heap made with the game's `create_mspace_with_base` over 1 MiB, its
  doug_lea_allocator registered as the only arena; the same allocations, frees and
  reallocations (direct and through malloc_impl) in the original and the patched
  exe. Unset: nothing reserved, written or logged, every result, register, flag
  and heap byte the original's, 4 instructions more an entry. On: the tables
  reserved, one line logged, every result, register and heap byte the original's.
  Then an allocation the heap cannot hold: out_of_memory_with_crash's entry writes
  the snapshot before the game's own body goes on, and the analyzer reads it: the
  heap's use as its walk finds it, its holes, each site's live bytes as the
  chunks say (malloc_impl's caller, a reallocation's caller; none of
  mspace_realloc's own), the bytes asked, the regions.
- `cargo test -p patches memory_profile` (the tables, the text, the setting) and
  `cargo test -p srp-validate` (the heap walk's free blocks).
