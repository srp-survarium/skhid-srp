# Startup resource arenas on Wine

The reported dialog, “Not enough memory to run Vostok Engine v1.0”, is
shared by several budget checks and the final reservation-failure path.
On this workstation, the pre-change client reproduces it under Wine 9.0
in a `win32` prefix at **0xE9BA2**, after both allocation calls returned
false. Skipping that check would leave the resource arena addresses null.

The reference `../vostok/sources/vostok/core/sources/memory_win.cpp` is
0.100b, not the exact 0.26g binary. Both the reference and 0.26g have a
fragmentation-aware region enumerator; the engine already shrinks pools
to fit reported free regions. It does not retry smaller requests when
`VirtualAlloc` fails despite that enumeration. The outer fallback changes
only the page-file budget, which need not lower the request on a machine
with ample memory. The observed Wine 9 failure does not establish that
fragmentation alone, or the largest region being 1.6 GiB, explains every
player's failure.

## Executable change (patch 78, unreleased)

The generated Rust lohook at **0xE9B96** handles the second allocation's
result. On failure it reduces the combined resource request in 64 MiB
steps, keeping the managed/unmanaged proportions and Windows reservation
alignment, down to the parsed `-minimum_resources_memory_size` floor.
The existing full-build default is 768 MiB. Each attempt uses the original
allocator with `only_resources=true`; fixed arenas are never allocated
twice. A successful attempt consumes the retry owner, preventing a later
attempt from overwriting live reservations. Genuine exhaustion still
reaches the original error path.

The first allocation and successful native fallback are unchanged. An
explicit maximum stays an upper bound because retries only decrease the
request. The existing native allocator may itself shrink requested pools
to fit free regions; this patch does not change that behavior. The native
page-file warning after a successful fallback is also unchanged.

The safe policy and owner are `rust/patches/src/startup_memory.rs` and
`vostok/memory.rs`. Bindings, frame fields and hook entry are generated from
the usual tables; the existing `patches.code` arena block owns the code.
No revision, content or simulation compatibility number changes.

## Native evidence

Authority: stock 0.26g SHA-256
`ac33d9937015c2299bf62cc6b127cb308b48a3475b30fff2ddd65479ae3bf153`.

| Address | Meaning and evidence |
|---|---|
| `0xE9580` | Outer startup allocator; two calls at E9AAF/E9B91, fatal at E9BA2 |
| `0xE8DC0` | Resource budget calculation, including the command-line maximum |
| `0xE8C10` | cdecl `try_to_allocate_arenas(fixed, managed, unmanaged, only_resources)`, AL boolean, caller pops 16 bytes |
| `0xE8430` | Region enumeration and allocation; partial resource allocations freed on failure |
| `0xE8C59` | `only_resources` skips the contiguous-allocation attempt; E8C8D passes it to the region allocator |
| `0xE8C95..E8CA2` | Failed result returns without copying local resource descriptors to the caller |
| `0xE8CA3..E8D15` | Success copies both 16-byte region descriptors back, preserving managed/unmanaged identity |
| `0xE9B96` | Hook replaces `add esp,10h; test al,al`; arguments still on stack, floor at ESP+40h, granularity in EBP |

The generated lohook's saved ESP is eight bytes below the game ESP (dispatch
slot plus flags). The adapter accounts for that once at its boundary.
Function and ABI findings are also recorded in `tools/re-kit/v026g/`.

## Verification

`check_startup_memory_026g.py` executes the linked Rust hook and generated
NASM entry in Unicorn. It checks successful allocations bypass retries,
decreasing aligned sizes, the explicit floor, genuine exhaustion, invalid
boundary values, descriptor identity, stack cleanup, preserved registers
and SSE state, and the original fatal guard. The allocation result is
controlled at the engine-function boundary in this check.

`scripts/startup-memory/check.py` separately exercises actual native
allocation and Win32 APIs under Wine. It stops before video detection;
success is **startup allocation tested**, not rendering, gameplay, Windows
testing or release. Wine 9 here is Nixpkgs' Wine 9.0 build, not Ubuntu's
package. The complete runtime matrix and exact hashes are recorded in the
Rust client work log after the final run.
