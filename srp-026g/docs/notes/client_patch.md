# Porting the srp client patch from 0.21d to 0.26g

Version scope: client 0.26g (build 2777, "0.26g0", linked 2015-01-15, no PDB) vs client 0.21d (build 2010, April 2014, PDB). Every address is a VA, image base 0x10000 in both; `.text` VA 0x11000 = file 0x400 in both.

Tags: **[V]** read in the exe/disassembly; **[I]** inferred. Patch-site status: **PROVEN** (0.26g VA + original bytes + the 0.21d site it corresponds to), **LIKELY**, **GUESS**.

Summary: the mutex patch ports 1:1 (PROVEN). One patch is 0.26g's own: two instructions of the battery rule (§3.9, written 2026-10-12, patch revision 3). The snapshot upload is still needed (the world is still frozen until the 0xE8 snapshot, and the client never sends its state); it ports with new addresses. Only 0x5D bytes of .text padding exist, so the upload code goes into a new PE section `.srp` (default) or over the dead `on_hash_mismatch` (fallback). The patcher, the assembly and a self-test are in `srp-026g/client-patches/` (reviewed against the disassembly 2026-10-12, §3.1–§3.5; not run live yet). The mode 3 battery patch is not needed: 0.26g serializes the next-battery time.

## Function map (`tools/re-kit/v026g/functions_matched.txt`)

- Made by `tools/re-kit/common/scripts/match_functions.py` (stdlib only, ~1 min): `python match_functions.py [--ref 021d --target 026g] [--stats] [--holdout 0.3 --holdout-from exact|tokens]`.
- Format: `<0.26g VA> <length> <0.21d name(s)>  # <confidence>; <method>: <details>; 021d <0.21d VA> len <0.21d length>`. `#` lines at the top are the header (format, counts, method legend). Hex without `0x`, like `v021d/functions.txt`.
- Hit rate: **22 478 of 35 070** distinct 0.21d functions (64 %); 16 956 **high**, 5 522 **medium**; weaker candidates are not written. 41 263 function starts found in 0.26g.
  - By method: exact bytes 9 807, exact shape 678, tokens 7 315, propagated 1 870, vtable 1 436, address order 1 372.
  - By area: Scaleform ≈ 93 %, STLport/boost/Bullet/OpenSSL high; **game code much lower** (survarium:: ≈ 30 %, network ≈ 18 %). 0.26g game code was recompiled with different settings: frame-pointer omission (`mov ebx,[esp+8]` instead of `push ebp; mov ebp,esp`), 16-byte function alignment with `int3` padding (0.21d is packed), different inlining. Library code (prebuilt) is byte-identical apart from relocations.
- Method (details in the script docstring): x86 length decoder → per-function normalized bytes, shape (prefix/opcode/modrm sequence), tokens (strings incl. `__FUNCSIG__` log strings, imports, RTTI vtables/type descriptors, later matched callees and globals). Passes: unique exact bytes, unique exact shape, reciprocal-best weighted tokens, propagation through matched pairs (instruction-aligned calls/pointers; call-sequence gaps need 2 agreeing callers), RTTI vtable slots, address-order gaps. Every non-exact candidate needs a structural check: shape similarity, or opcode-histogram cosine + instruction-count ratio (tolerant of the recompilation).
- Confidence:
  - **high**: exact bytes (≥ 6 insns) / exact shape (≥ 30 insns), or a strong signal plus structure ≥ 0.8 shape or (histogram ≥ 0.9 and size within ×0.75–1.33). Hold-out test (2 775 exact matches, 30 %, hidden from the hash passes): 2 680 recovered by the other methods, **1 wrong** (a high token match).
  - **medium**: one signal + a looser structural check. Hold-out on 222 hidden "≥ 2 unique strings" pairs (blocked from the token pass): 107 recovered by propagation/vtable/order, **0 wrong**. One wrong medium match was found by hand and fixed by tightening propagation (`game_world_core::clear` was given to 0.26g `new_game_state_history_item` 0x2D09C0); expect a few % of medium rows to be wrong, mostly where inlining differs.
- Function starts in 0.26g are heuristic (call targets, data pointers, int3-padding ends, tail jumps). A "length" runs to the next found start and can include an undiscovered neighbour.

## Exes used

- 0.21d original: `E:/sur-res-pro/downloads/v021d/extracted/game/binaries/x86/survarium.exe` (sha256 def82eea...). **Note:** `E:/ssrp/Survarium-0.21d-srp/binaries/x86/survarium.exe`, which `common/scripts/exes.py` lists as "original", is the srp-patched copy (same sha as `survarium_srp.exe`: mutex `EB 22`, cave filled, new tick). [V]
- 0.26g: `E:/sur-res-pro/downloads/v026g/extracted/Survarium/game/binaries/x86/survarium.exe` (12 894 840 bytes). Not modified.

## 1. Single-instance mutex — PROVEN

| | 0.21d | 0.26g |
|---|---|---|
| code | inline in `WinMain` 0x12644 | own function 0x124D0 (returns bool), same strings |
| mutex name | `survarium_or_its_installer_is_already_running` 0x7EB7B4 | same string at 0x9DAECC, pushed at 0x124D6 [V] |
| check | `cmp eax,0B7h` (ERROR_ALREADY_EXISTS) then `jne 0x1269E` (= OK) at **0x1267A** `75 22` | `cmp eax,0B7h` then `jz 0x12504` (= fail path) at **0x124F9** `74 09` [V] |
| patch | `75 22` -> `EB 22` | `74 09` -> `90 90` (fall through to `mov al,1; ret`) |

The fail path is the same message box (`%s or its installer is already running...`). Still needed: yes, if several clients must run on one PC.

## 2. Is the snapshot upload (option B) still needed? — yes

- **The world still freezes until the first snapshot.** [V] `game_world_core` ctor (0.26g, around 0x2D00B0–0x2D0136) sets, exactly like 0.21d 0x1CA5F8–0x1CA609:
  - +0x18FDD = server-mode argument (0.21d +0xC80D);
  - +0x18FDF = frozen = !(server_mode || replay) (0.21d +0xC80E), written at 0x2D0130;
  - `game_world_core::move` 0x2D2E40 returns at once while +0x18FDF is set (0x2D2E4A; 0.21d 0x1CCCB4);
  - only `game_world_core::deserialize` clears it (0x2D0FE7 `mov byte [edi+18FDFh],0`; 0.21d 0x1CAD89). It is reached from the 0xE8 handler (0x27BFF0), the 0.26g dynamic-info message (0.21d 0x53).
- **The client still never sends its state.** [V] `game_state_history_item::serialize` 0x324BE0 has three callers: `hash()` 0x325150 (CRC only), `game_world_core::on_hash_mismatch` 0x325A60 (debug compare of a server state, only with `-debug_hash_mismatch`) and the new `game_world_core::dump_game_state` 0x3285D0 (writes a file; string `survarium::game_world_core::dump_game_state`). None of them builds a packet.
- The hash report is still there: `try_send_all` 0x27CBF0 sends **0xDF** = `{u8 (event-horizon item time / 50), u32 crc}` (0.21d 0x46 had a u32 time), using `is_there_event_horizon_item` 0x2D1070, `event_horizon_history_item` 0x2D0B30 and `hash()` 0x325150 with the players-mask history at core+0x12280. [V] The CRC table generator 0x1C210 uses polynomial 0x04C11DB7, reflected, init/final 0xFFFFFFFF (boost crc_32, as 0.21d). [V]
- So a non-simulating srp still needs a patched client to obtain the first and the resync snapshots. The 0.26g snapshot message also changed (below), so srp's 0.21d forwarding needs work too.

### What changed around it [V unless tagged]

- Message ids moved. Server→client dispatcher `on_match_packet_received` **0x27A300** (FUNCSIG string 0xA08908; 0.21d 0x1A1520), jump table 0x27A738, ids 0xE3–0xF6 (20; 0.21d 0x50–0x59). Client→server ids 0xD8–0xE1 (10; 0.21d 0x40–0x47). Message-info tables: C→S 0x279A60 (table 0x279AE0), S→C 0x279B10 (table 0x279BF0); entries are `{u32, u16, u8 flags}` (0.21d `{u16, u8, u8}`); meaning of the fields not re-derived [I].

| 0.21d | 0.26g | evidence |
|---|---|---|
| C→S 0x40 connect | 0xD8 | `match_client::connect` 0x27A1B0 pushes 0xD8 |
| C→S 0x41 port probe | 0xD9 | 0xE3 handler 0x27B830 passes 0xD9 to `set_connection_ports` (vtbl+0xC) |
| C→S 0x42 ready | 0xDA | end of `network_client::on_match_ready`, 0x27AEAE |
| C→S 0x43 set-time reply | 0xDB | 0xE5 handler 0x27B570 (0x27B5E7); also sent by `try_send_all` 0x27CE7A after 0xE6, with three u16 |
| C→S 0x44 clock | 0xE1? | `try_send_all` every 1000 ms (0x27CE3E), empty [I] |
| C→S 0x45 input, new ones | 0xDC / 0xDD / 0xDE | not traced; 0xDE is built in 0x27EB90 [I] |
| C→S 0x46 hash | 0xDF | `try_send_all` 0x27CDC9, after `hash()` |
| C→S 0x47 leave (u32) | 0xE0 (**empty**) | `close_current_match` 0x27A790, send at 0x27A833 |
| S→C 0x50 ports | 0xE3 | state-0 case, 0x27B830 |
| S→C 0x51 static info | 0xE4 | state 1→2 case, 0x27B880: u8 flag, then a payload that goes through a decompressor (`[nc+0x4C]` vtbl+4) into a 0x578A0 buffer |
| S→C 0x52 set time | 0xE5 | 0x27B570: u8 sequence (window ±128 against `[nc+0x1F1C68]`), u16, u32 time → `game::set_time_in_ms`, reply 0xDB |
| — | 0xE6 | 0x27B770: two u16 timing values, sets `[nc+0x1F1C69]` (then 0xDB from `try_send_all`) |
| S→C 0x55 clock offset | 0xE7 | 0x27B7F0 → `floating_timer::set_offset` |
| S→C 0x53 dynamic info (snapshot) | **0xE8** | 0x27BFF0: u8 sequence (same window), **u8 compressed**; if non-zero the rest is decompressed into 0x8000 bytes first, if zero it is read as is; then `game_world_core::deserialize` 0x2D0B70 (string "--- THE VERY FIRST SYNCHRONIZATION ---"), whose item reader takes the u32 item time first |
| S→C 0x56 entered | 0xE9 | 0x27C5F0 → `enter_match` 0x2D1470 |
| S→C 0x57 left | 0xEA | 0x27C690 → `leave_match` 0x2D18A0 |
| S→C 0x54, 0x58, new ones | 0xEB–0xF5 | not traced (EB/EC, ED/EE, EF/F0, F1/F2 share handlers) |
| S→C 0x59 hash mismatch | **0xF6** | case 0x27A4F4; parsed only when `g_debug_hash_mismatches` (byte 0x1F1D32C) is set, else a log hint and skip |

- `network_client` grew: core pointer, match client and state moved from +0x7A00 / +0x7A08 / +0x7A1C to +0x1F1C40 / +0x1F1C48 / +0x1F1C64. `game_world_core` is now **embedded** in `pvp_match_core` at +0x1F8 (0.21d: a pointer at pvp+0x138).
- Packets: the `udp_match_packet` ctor 0x278D00 sets the write buffer at pkt+0x298 to `{allocator 0, data pkt+0x99, capacity 0x1FC, size 3}` and the writer at **pkt+0x2A8** (0.21d: pkt+0x284 `{0, pkt+0x7B, 0x208, 0x0C}`, writer +0x294). The datagram header looks 3 bytes long now [I: from the initial size]. Writing past the capacity still reallocates through a null allocator (`mutable_buffer::resize` 0x7AB30 → 0x7AB70).

## 3. Snapshot-upload port table

Status: **PROVEN** = 0.26g VA + original bytes [V] + the 0.21d counterpart; **LIKELY** = right place, one convention or meaning inferred.

### Patch sites

| what | 0.21d | 0.26g | original bytes (0.26g) → patch | status |
|---|---|---|---|---|
| mutex check | 0x1267A `75 22` | **0x124F9** `jz 0x12504` | `74 09` → `90 90` | PROVEN |
| default hash interval (full builds only, §3.6) | none (not in the 0.21d patch) | **0x27CC4F** `mov dword [esp+18h],64h` in `network_client::try_send_all` 0x27CBF0 | `C7 44 24 18 64 00 00 00` → `C7 44 24 18 32 00 00 00` (one byte, 0x27CC53) | PROVEN (the bytes and what reads them); the effect is not tried live |
| first-sync hook (before the ready send; ebx = network_client) | 0x1A27A8 `lea esi,[ebx+7A08h]` in `on_match_ready` 0x1A25CC | **0x27AEA1** `mov ebx,[ebx+1F1C48h]` in `on_match_ready` **0x27AA20** (then `mov esi,[ebx]` … `push 0DAh; call [edx+18h]`); returns to 0x27AEA7 | `8B 9B 48 1C 1F 00` → `E9 rel32 90` | PROVEN |
| request hook (0x59 / 0xF6 without the debug flag) | 0x1A16B1 `mov edi,[g_log_filter_tree]`, edi = nc | **0x27A505** `cmp byte [1F1D32Ch],0` (the `g_debug_hash_mismatches` test, one instruction earlier than in 0.21d so the debug branch becomes unreachable); esi = nc, ebx = reader, edi = 0 | `80 3D 2C D3 F1 01 00` → `E9 rel32 90 90` | PROVEN |
| request done (skip payload, return) | 0x1A1721 | **0x27A5AB** `mov edx,[ebx+8]; add edx,[ebx]; mov [ebx+4],edx; jmp 27A6FCh` | `8B 53 08 03 13 89 53 04 E9 44 01 00 00` (unchanged) | PROVEN |
| code cave | 501 zero bytes after .text at 0x6A1E0B (0x185 upload + 0x70 batteries) | **only 0x5D zero bytes** after .text (0x86F5A3–0x86F5FF; .rdata's raw data starts right after): too small. Cave = **`game_world_core::on_hash_mismatch` 0x325A60**, 0x47B bytes of code + 5 int3 (FUNCSIG "void __thiscall survarium::game_world_core::on_hash_mismatch(class vostok::network_core::buffer_reader)"). Its only reference is the `call` at 0x27A632 inside the debug branch (no pointer to it, no other branch into it: full-.text sweep) | sha256 of 0x325A60..0x325EDF = `f757a414…040c`; 47 base relocations inside, disabled by the patcher | PROVEN (dead once the request hook is in) |

The default cave is now a new section, which keeps `on_hash_mismatch` intact; the overwrite in the table above is the `--cave=hash-mismatch` fallback (§3.3).

### Functions and data the injected code uses

| role | 0.21d | 0.26g | convention (0.26g) | evidence | status |
|---|---|---|---|---|---|
| item serializer | `game_state_history_item::serialize` 0x1FDE3E (eax = item, push mask, push &writer, ret 8) | **0x324BE0** | **stdcall(item, writer*, mask), ret 0Ch** | called from `hash()` at 0x3251D7 with the same scratch writer; ends `ret 0Ch`; writes the u32 mask, then `player_history_item::serialize` … | PROVEN |
| `hash()` | 0x1FE15A | 0x325150 (eax = item, push &masks, ret 4) | | same 0x8000 alloca, writer layout, mask walk, CRC; cache at item+0xC0BB4 / +0xC0BB8 (0.21d +0xB993C / +0xB9940) | PROVEN |
| buffer write | `buffer_writer::w(writer*, src, n)` 0x155EBE, ret 0Ch | **0x7AC10** | **push src; push writer*; edi = n; ret 8** | `resize` 0x7AB30 (eax = mutable_buffer), then `memcpy` 0x424580; the only small function with that call pair | PROVEN |
| stack probe | `__alloca_probe_16` 0x2B3F00 | **0x424CA0** | eax = size | CRT code; used by `hash()` at 0x325182 | PROVEN |
| has event-horizon item | `pvp_match_core::is_there_event_horizon_item` 0x1DCBE6 (eax = pvp) | **0x2D1070**, now on the core (eax = **core**) | → al | same tests (head, server-mode flag, time window); `try_send_all` calls it at 0x27CD7A before 0xDF | PROVEN |
| event-horizon item | `game_world_core::event_horizon_history_item` 0x1CAB13 (eax = core) | **0x2D0B30** (eax = core) | → eax | server mode → newest; else walks back from the newest while time > horizon | PROVEN |
| initial item builder | `game_world_core::serialize` 0x1CA765 (called at 0x1A26CF) | 0x2D04F0 (esi = core), called at 0x27AD15 in `on_match_ready` | | new item 0x2D09C0, world serialize 0x2D2570, append at the tail | PROVEN |
| match client | `[nc+0x7A08]`, vtbl +0x18 `new_packet(id)`, +0x1C `enqueue(pkt)` | **`[nc+0x1F1C48]`**, vtbl +0x18 = 0x279FE0 (→ 0x37A0E0), +0x1C = 0x27A2A0 | thiscall, one stack arg | `match_client@survarium` vtable 0xA08864; same calls in `on_match_ready` / `try_send_all` | PROVEN |
| core | `[[nc+0x7A00]+0x138]` | **`[nc+0x1F1C40]` + 0x1F8** | | `try_send_all` 0x27CC76 / 0x27CD66, `on_match_ready` 0x27AD0D, 0xE8 handler 0x27C0C9 | PROVEN |
| network state | `[nc+0x7A1C]` == 2 | **`[nc+0x1F1C64]`** == 2 | | every state test in the dispatcher | PROVEN |
| newest history item | `[core+0x2678]` | **`[core+0x2FC0]`** (oldest +0x2FBC, count +0x2FB8) | | 0x2D04F0 appends there; 0x2D0B30 returns it in server mode | PROVEN |
| players-mask history | core+0x5ED0, head +8, node {+4 next, +8 mask, +0xC time} | **core+0x12280**, same node layout | | `hash()` mask walk; `try_send_all` 0x27CD8A | PROVEN |
| item time | item+0xB9938 | **item+0xC0BB0** | | `hash()`; item deserialize 0x324DA0 | PROVEN |
| packet writer | pkt+0x294 | **pkt+0x2A8** | | ctor 0x278D00; every 0.26g send (`lea esi,[ebx+2A8h]`) | PROVEN |
| carrier message | C→S 0x47 (leave, u32), reliable, unordered | **C→S 0xE0** (leave, **empty**), info `{0xFFFFFFFF, 500, 0x9F}` like 0xD8 / 0xDA: reliable (resent every 500 ms until acked), plain, fixed length 0 | | 0x27A833; table 0x279AE0; send path §3.2 | PROVEN (a non-empty 0xE0 leaves the client intact, alone in its datagram) |
| chunk size | 480 (12 + 1 + 8 + 480 ≤ 508) | 480 (3 + 1 + 8 + 480 = 492 ≤ 0x1FC; 16 bytes left for tail ack bytes) | | packet capacity 0x1FC, initial size 3, id written by `construct_packet` 0x3780B1, no x byte for flags 0x9F (0x3780E5) | PROVEN (§3.2) |
| packet pool | — | `[[nc+0x1F1C48]+0x578B0]`: +0x30 packets in use, +0x34 capacity = **256** | | `new_packet` 0x37A0E0, allocator 0x279D80, pool built at 0x378A77 with 0x2C000 bytes / 0x2C0 | PROVEN |
| packet flags | — | pkt+0x98 = 0x60 from the ctor: bit 5 single message, bit 6 may be joined | | 0x278D98, 0x3FCA86, 0x3FCAD5 | PROVEN |

### 3.1 Review of the injected code (2026-10-12) — PROVEN unless tagged

`snapshot_upload_026g.asm` was read instruction by instruction against the original 0.26g exe (`dis.py 026g`). The numbered list at the end of the .asm file repeats this per line.

| what the code relies on | read at | result |
|---|---|---|
| `serialize` is stdcall(item, writer*, mask) | 0x324BE0 `push ebx; mov ebx,[esp+0Ch]` (writer), `push ebp; mov ebp,[esp+0Ch]` (item), mask read at 0x324CB1, `ret 0Ch`; `hash()` pushes mask, &writer, item (0x3251D1) | right; saves ebx, ebp, esi, edi |
| scratch writer layout | `hash()` 0x32518A..0x3251B0 | `mutable_buffer {0, esp, 0x8000, 0}`, writer `{+0 0, +8 0, +0xC 0, +0x10 0xFF, +0x14 &buffer}`: same |
| players mask for the item | `hash()` 0x325197..0x3251CE: head `[masks+8]`, node +0xC time, +4 next, +8 mask | same walk; `hash()` reads address 8 when the list is empty, the patch passes mask 0 |
| `buffer_writer::w` | 0x7AC10: writer = `[esp+8]` after one push (last pushed), src = `[esp+10h]`, n = edi, `ret 8` | right; edi is only read; saves ebx, esi |
| `_alloca_probe_16` | 0x424CA0 (tail-jumps to `_chkstk` 0x429F20) | only eax and esp change; `hash()` takes the buffer from esp the same way |
| `is_there_event_horizon_item` / `event_horizon_history_item` | 0x2D1070 / 0x2D0B30, called with eax = `[nc+1F1C40h]+1F8h` at 0x27CD66..0x27CD85 | eax in, al / eax out; esi and edi survive |
| `new_packet` / `enqueue` | vtbl+0x18 = 0x279FE0 → 0x37A0E0 (`ret 4`, packet in eax); vtbl+0x1C = 0x27A2A0 (`ret 4`) | thiscall with one stack argument; esi, edi survive |
| packet writer | ctor 0x278D9F..0x278DD7: buffer at pkt+0x298 `{0, pkt+0x99, 0x1FC, 3}`, writer pkt+0x2A8 with +0x14 → the buffer | right |
| ready hook 0x27AEA1 | 0x27AE9A `mov byte [ebx+1F1C68h],0FFh` just before: ebx = network_client. After it the function reloads esi, edx, eax and pops edi; no flags are used | `pushad`/`popad` around the upload, then the replaced `mov ebx,[ebx+1F1C48h]`, then `jmp 27AEA7h`. The 6 bytes hold no relocation |
| request hook 0x27A505 | 0x27A300: ebx = reader (`[ebp+0Ch]`), esi = network_client (`mov esi,ecx`). Jump table 0x27A738 entry 19 = 0x27A4F4, which skips the payload when state ≠ 2 and **falls into 0x27A505 in every state** | the cave tests state == 2 itself; it ends at 0x27A5AB (reader := end, then the common exit 0x27A6FC, which uses ebx and ebp only). The 7 bytes hold one base relocation (0x27A507): the patcher disables it |
| thread | both sites are followed by the client's own `new_packet`/`enqueue` on the same object (0x27AEAE; the 0xE5 handler sends 0xDB at 0x27B5E7 from the same dispatcher) | the upload runs where the client itself sends |
| nothing else enters the replaced bytes | linear disassembly of both functions, the jump table, and a brute-force scan of `.text` for rel8/rel32 branches and pointers into 0x27AEA2..0x27AEA6 and 0x27A506..0x27A50B (`selftest_026g.py`) | none |

Fixed or added in the review:
1. **Never joined.** Each chunk packet gets `and byte [pkt+98h],0BFh` (§3.2).
2. **Pool guard.** The upload is skipped unless `chunks + packets in use + 16 ≤ pool capacity` (§3.2).
3. **Rebase safe.** The code has no absolute operands; the patcher disables the base relocation under the request hook (the draft only did so inside the overwritten function). The image has no ASLR flag (DLL characteristics 0x8000, file characteristics 0x122), so a rebase happens only if 0x10000 is taken.
4. The 0xF6 request must carry **one payload byte** (§3.4); the draft said "any payload".

Limits that stay: a world over 0x8000 bytes asserts in `mutable_buffer::resize` (0x7AB70, null allocator), exactly as the client's own `hash()` does every 100 ms, so the patch adds no new failure there.

### 3.2 The carrier: C→S 0xE0 with a payload — PROVEN from the send path

Every C→S id is fixed length (transport.md §4), so no id carries a length; 0xE0 (leave, length 0) is kept because a real leave is told apart by being empty, and it is reliable (lifetime ∞, resend 500 ms). What the client does with a 0xE0 packet that has a payload:

- **Build.** `construct_packet` 0x378080 appends the id byte and, only for flags 0x40/0x20, a placeholder x byte (0x3780E5). 0xE0 has flags 0x9F: the packet is `3 header bytes, E0`, then whatever the caller writes. Nothing checks the size against the id's fixed length on the way out.
- **Enqueue.** `enqueue_impl` 0x3FCE30 touches the x byte only for ordered/unique ids, then links the same packet object into the connection's queue. No copy, no split (the splitter 0x3F9D00 is only in `enqueue(id, reader, size)` 0x3FD060, which the patch does not use).
- **Size.** `send_queued_packets` asserts only when the packet is over 0x1FC bytes (0x3FC310). A chunk packet is 3 + 1 + 8 + 480 = **492**.
- **Tail ack bytes.** `free = 0x1FC − size` (0x3FCA27); the client appends `min(wanted, free)` ack bytes (0x3FCA35..0x3FCA4B; wanted = (depth − 7 + 6)/7 ≤ 17, 0x3FC27E..0x3FC2AA). With 16 bytes free a chunk datagram can carry all but the 17th; when some did not fit, one extra keep-alive datagram with the full set follows the flush (0x3FCBCD..0x3FCBF4). The send completion handler removes the ack bytes from the packet again (0x3FAFE9: resize by −n), so a resend starts from 492 bytes.
- **Single datagram.** With no joined follower (`[pkt+4] == 0`, 0x3FB970) the packet's own buffer is sent: header filled in place by 0x3FB390, F = 0 because bit 5 of pkt+0x98 is set (0x3FB41A). **So the server receives `seq rseq ack0 E0 <payload> [tail ack bytes]`, single format.**
- **Joining.** Only with `-join_net_messages` (0x3FCA57..0x3FCA7D), and only when the first packet (0x3FCA86) **and** the candidate (0x3FCAD5) have bit 6 of pkt+0x98. The ctor sets 0x60 (0x278D98). The patch clears bit 6 on its packets, so a chunk is never the head of a joined datagram and never joined into one, whatever the option says. Bit 6 is tested nowhere else in `.text` (only these two `test byte [reg+98h],40h` exist); the low nibble of the byte is an in-flight counter (0x3FB2A4, 0x3FAFBE) and bit 4 "queued" (0x3FCEC4), both untouched.
- **Burst size.** A snapshot is at most 0x8000 bytes = **69 chunks** (0.21d worlds were 11–14 KB = 23–30). Three limits apply, all satisfied by one upload:
  - send window: the client sends while its next seq is 1..127 ahead of the newest acked (0x3FC2C3..0x3FC2FB); the rest of the queue is put back (0x3FCB81) and goes out on a later pass, nothing is lost;
  - unacked limit: ≥ 256 packets on the unacked list disconnects (0x3FC146). Only reliable packets count (0x3FB620);
  - **packet pool (new finding):** game-side packets come from a 256-packet pool (0x378A77: 0x2C000 bytes / 0x2C0) and stay out of it until acked or expired. `new_packet` 0x37A0E0 does not check for an empty pool: 0x279E8D asserts and returns 0, and `construct_packet` then writes to a null packet. So a burst that empties the pool **crashes** the client. The patch reads the pool's use count (`[pool+30h]`, compared with `[pool+34h]` by the client itself at 0x279D87) and skips the upload unless `chunks + in use + 16 ≤ capacity`. Since every unacked packet is a pool packet, this also keeps the unacked list under 256.
  - Pacing is therefore not needed for one upload. The server must not ask again before the previous upload is complete or about a second old (§3.4), or the guard will skip the new one.
- **Alternative carriers considered:** none is better. All ten C→S ids are fixed length and plain; 0xD8/0xD9/0xDA/0xDE are also reliable but have live meanings with non-zero lengths (a chunk could equal a real message's length), and the send-once ids would lose chunks. 0xE0's real form is the only empty reliable one.

### 3.3 Cave placement — section by default

**`--cave=section` (default).** A new section holds the upload code; `on_hash_mismatch` stays intact.

| header item | original | patched |
|---|---|---|
| NumberOfSections | 7 | 8 |
| section table | ends at 0x328; 216 zero bytes to SizeOfHeaders 0x400 | entry 8 at 0x328: `.srp`, VirtualSize = code size (0x1A4), RVA **0xA415000** (VA **0xA425000**), raw 0xC4AA00 size 0x200, characteristics 0x60000020 (code, execute, read) |
| SizeOfImage | 0xA415000 | 0xA416000 |
| SizeOfCode | 0x85E600 | 0x85E800 |
| security directory (file offset, size) | 0xC4AA00, 0x1878 | **0xC4AC00**, 0x1878 |
| CheckSum | 0xC5A7B2 | recomputed (imagehlp agrees) |
| file size | 12 894 840 | + 0x200 |

- **The Authenticode blob.** The attribute certificate table is found by the **file offset** in the security data directory and is the last thing in the file (0xC4AA00 + 0x1878 = file size), right after `.reloc`'s raw data. The new section's raw data must follow `.reloc` (sections are contiguous in the file), so the blob moves 0x200 bytes towards the end and the directory's offset follows it; it stays 8-byte aligned and last, as the PE format asks. Its bytes are unchanged.
- **Why it must stay readable.** At start `vostok::crypt::verify_sign` 0x13310 calls `CryptQueryObject(CERT_QUERY_OBJECT_FILE, own path, CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED 0x400, CERT_QUERY_FORMAT_FLAG_BINARY 2, …, &store, &msg, …)` (0x13336..0x1335B), then looks up "survarium.com" in that store and uses the certificate's key on the resources.db file table (§0; the default build skips that check since revision 12, §0.1, but still opens the store and finds the certificate first: verify_sign returns 1, a failure, at 0x1342F when `CryptQueryObject` fails). Dropping the blob would stop every start.
- **A changed file does not break it.** `CryptQueryObject` only extracts and decodes the PKCS#7 blob; it does not compare the Authenticode hash with the file (that is `WinVerifyTrust`; the exe has no `WinVerifyTrust`/wintrust or imagehlp checksum import, its only use of the signature is this `CryptQueryObject`). PROVEN two ways: the cert-only build already runs (§0), and `selftest_026g.py` makes the client's exact call on each built file: the store opens and the survarium.com certificate (1545 bytes, sha1 4b330026…eea6) is byte-identical to the original's, for the section, hash-mismatch and cert-only builds.
- **Relocations.** `.reloc` is untouched except the one entry under the request hook (0x27A507 → type 0). The new code needs none.
- **Risk, LIKELY harmless:** whether anything in the client minds an 8th section (a packer or anti-tamper check of its own image). None was seen: no code reads its own section table apart from the CRT/TLS startup (not searched exhaustively). If the section build misbehaves live, use the fallback.

**`--cave=hash-mismatch` (fallback).** As the draft: the code overwrites `game_world_core::on_hash_mismatch` 0x325A60 (0x47B bytes; unreachable once the request hook replaces the debug test), the rest becomes int3, the 47 relocations inside it and the one under the hook are disabled. No header change except the checksum. Loses the `-debug_hash_mismatch` compare.

### 3.4 Wire format for the server

**C→S 0xE0.** Flags 0x9F: plain, reliable, no x byte, fixed length 0 in the client's table.

| datagram | bytes after the id | meaning |
|---|---|---|
| single (F = 0) | none | a real leave (`close_current_match` 0x27A828) |
| single (F = 0) | `u32 item_time, u16 offset, u16 total, data[n]` with `n = min(480, total − offset)`, so 9..488 bytes | a snapshot chunk from a patched client |
| multi (F = 1) | none (take length 0, as the table says) | a real leave; a chunk never appears here (its packet cannot be joined, §3.2) |

- In a single datagram the message is everything between the id and the tail ack bytes (transport.md §3.1), so the server knows the length: 0 = leave, otherwise a chunk. Accept a chunk only if `offset % 480 == 0`, `offset < total ≤ 0x8000` and `length == 8 + min(480, total − offset)`; ack the datagram in any case.
- A chunk is resent every 500 ms until acked, and 0xE0 is plain (no transport dedup): drop duplicates by `(item_time, offset)`. Chunks can arrive in any order (the client sorts its send queue, transport.md §2.5, and UDP reorders): place each by its `offset`.
- When `total` bytes are present, the body is exactly what `game_state_history_item::serialize(mask)` wrote: `u32 active_clients_mask`, the player blocks, then the six u16-prefixed blobs (match.md §3). Its boost CRC-32 is what 0xDF reports for that item. The 0xE8 to send is `u8 generation, u8 0, u32 item_time, body`.
- Two uploads of one client never interleave within one `item_time`; a new request can produce a new `item_time` while old chunks are still being resent, so key the reassembly by `item_time`.

**S→C 0xF6 = "upload your event-horizon state".** 0xF6 is unique and variable length: `F6 <unique id> <payload>`, and the length getter rejects an empty payload (0x2799C1: `ok = remaining > 0`; a failed parse drops the whole datagram, transport.md §2.3). **Send one payload byte, `00`.** The hook ignores the payload. Send it only to patched clients (a stock client skips it and logs a hint).

The request is silently not answered when:
- the client is not in state 2 or has no match core yet;
- `is_there_event_horizon_item` is false: no history, or the event-horizon item came from a snapshot (item+0xC0BB9), or it is younger than the horizon (0x2D1070). So right after a 0xE8 the client has nothing to upload until its simulation passes the snapshot's time by the horizon (`max_ping_time_in_ms` + 250 ms);
- the pool guard skips it (§3.2).

Server rules: one request per client at a time; wait for the upload to complete or about 1 s before asking again; keep acking during the burst (up to 69 datagrams, then resends after 500 ms).

Code (added 2026-10-12): `crates/survarium/src/v026g/match_msgs/upload.rs`
(`Chunk::parse` with exactly these checks, `Reassembler` keyed by
`item_time`, `split` = what the patch sends); the request is
`ServerMessage::UploadRequest`. The last chunk of a full 0x8000-byte world
starts at 0x7F80 and has 128 data bytes.

**First sync.** In `on_match_ready`, after `game_world_core::serialize` 0x2D04F0 built the initial item and before 0xDA, the client uploads `[core+2FC0h]` unasked. The chunks are enqueued before 0xDA; do not rely on their order on the wire.

### 3.5 Build and self-test (`srp-026g/client-patches/`)

- `snapshot_upload_026g.asm` → `nasm -f bin -DCAVE=0xA425000 … -o snapshot_upload_026g.section.bin` and `-DCAVE=0x325A60 … -o snapshot_upload_026g.hash-mismatch.bin` (433 bytes each = 420 cave + 6 + 7 hook bytes; both checked in).
- `patch_026g.py <survarium.exe> <out.exe> [--cave=section|hash-mismatch] [--cert-only] [--bin=<file>]`, or `<survarium.exe> --check …` (everything in memory, nothing written). It checks the exe sha256 (`ac33d993…f153`), each site's original bytes, the free section table entry and the file layout (section mode) or the function's sha256 (hash-mismatch mode), and refuses a .bin assembled for the other cave. `--cert-only` output is byte-identical to the `survarium_srp.exe` installed in step 0.
- (Later patches change these counts and hashes: §3.7, §3.8, §3.9 and §6 each name their own. With all of them, revision 3, the self-test counts 61 bytes in 17 ranges for the section build and 1220 in 60 for hash-mismatch; with §3.10, revision 4, 67 in 19 and 1227 in 62. The cert-only output is no longer the installed `survarium_srp.exe`, which is a full build since the morning of 2026-10-12.)
- `selftest_026g.py` builds the three variants into `srp-026g/target-a4/scratch/` and checks each: PE re-parse (sections aligned and contiguous, SizeOfImage, data directories, certificate table last and identical); every byte outside the intended ranges equals the original (section build: 40 changed bytes in 12 ranges below the old end of file, plus the 0x200 new bytes; hash-mismatch: 1199 bytes in 55 ranges; the counts include the checksum's bytes that happen to differ); no active relocation under rewritten bytes and the rest of `.reloc` untouched; ndisasm of the cave and hooks (every direct branch inside the cave or to one of the 7 external addresses, the two indirect calls are `[eax+18h]` / `[eax+1Ch]`, both entries start with `pushad`); no branch or pointer into the hooks anywhere in `.text`; imagehlp's checksum equals the header's; the client's own `CryptQueryObject` call still yields the survarium.com certificate; the hash interval site (§3.6) is `mov dword [esp+18h],32h` in the full builds and the original in `--cert-only`. **All checks pass (2026-10-12, again with §3.6).** Nothing was run or installed; the builds in the scratch folder are for inspection only.
- Not tested: the code itself running (needs a 0.26g match server to reach `on_match_ready`).

### 3.6 Default hash interval 100 → 50 ms (added 2026-10-12) — PROVEN bytes, not run live

- **Why.** A stock client sends its world hash (0xDF) when `last_sent + interval <= now` on the 50 ms grid, with interval 100: every second tick, on a phase of its own that is drawn again at each 0xE5 (match.md §14). Two clients on opposite phases never send a hash for the same tick, and the match server trusts a world only when two clients named its tick: worlds are compared only at the tick of an upload that happens to fall on the other client's ticks (about one upload in two; the others are not trusted), a client out of step is found late, so the end by the mode's rule and the results come late or not at all. With 50 every client reports every tick, so there is no phase.
- **Where.** `network_client::try_send_all` 0x27CBF0, the one-time block guarded by bit 0 of 0xA2B3940: `lea ecx,[esp+14h]` / `push ecx` / `mov esi,0AA19F8h` (the option object of `-hash_check_interval_in_ms`) / **0x27CC4F `mov dword [esp+18h],64h`** / `call 0x141B0` / `mov edx,[esp+14h]` / `mov [0A2B393Ch],edx`. After the push `[esp+18h]` is the variable whose address was pushed: the instruction stores the default, and 0x141B0 writes over it only when the key was given (it returns at 0x141CA without touching `[ecx]` when 0xDDF20 finds no value). [V]
- **Patch.** `C7 44 24 18 64 00 00 00` → `C7 44 24 18 32 00 00 00`: the imm32's low byte at VA 0x27CC53 (file offset 0x26C053), `64` → `32`. Full builds only (`--cave=section` and `--cave=hash-mismatch`); `--cert-only` is unchanged and still byte-identical to the installed `survarium_srp.exe`.
- **Checked** (the self-test repeats each): no base relocation covers the immediate (the nearest are at 0x27CC40 and 0x27CC4B, the two absolute operands before it); no branch of the function lands inside the instruction; the static 0xA2B393C has one writer (0x27CC60) and two readers, both the same `last_sent + interval` gate (0x27CD2E and 0x27CD58); the option object 0xAA19F8 is named only at 0x27CC4A and at its registration 0x853F50, so nothing else holds or reads this default. [V]
- **Without the patch:** start every client with `-hash_check_interval_in_ms=50` (LIKELY syntax, as `-server=`; not tried live). The key still wins over the patched default.
- The match server logs "… opposite hash phases …" when it sees the starved state (match.md §13).

### 3.7 Minimum video-memory startup check

The full patch includes the one-byte bypass already used by `play_skhid.sh`.
Verified against the original build 2777 executable (sha256 checked by the
patcher) and its disassembly:

- VA **0x106411**, file offset **0xF5811**: `73 0F` (`jae 0x106422`) becomes
  `EB 0F` (`jmp 0x106422`). The preceding `cmp ebx,0x14000000` compares the
  detected video-memory size against **320 MiB**. The skipped path pushes
  320 and a warning string, then calls 0x113580. [V]
- The target at 0x106422 returns the original detected size from EDI:EBX
  in EDX:EAX. The zero/unknown-size path at 0x1063F1 still returns
  0x14000000. Neither the measurement nor the fallback is changed. [V]
- This bypasses the startup rejection for a low reported VRAM size; it does
  not increase memory or replace the allocator launch options.
- **Who is affected** (read in the exe, 2026-10-12, not run): for a size of 320 MiB or more, and for a size that cannot be detected (0, answered as 320 MiB at 0x1063F7), the original `jae` already went to 0x106422, so the patched exe does there exactly what the original does. Only a detected size under 320 MiB takes another way: the original stops in 0x113580 (it formats the text and never returns), the patched one returns the size. The function's one caller (0xE9C3F) stores the size (0x1B3A1F0, 0x1B3A1F8) and then has a check of its own, 0xE9D26..0xE9D47: under the minimum it holds at `[esp+28h]`, which is 320 MiB again (0xE990A), it stops with "Not enough video memory to run Vostok Engine v1.0". So by the code a card under 320 MiB is still stopped, with that other text, unless that second check does not see it; this was not tried. The size goes to the memory statistics, a log line and one initializer (0x15583 → 0x13C40) and to nothing of a match's world.
- Applied to `section` and `hash-mismatch` builds, before the existing PE
  checksum update. `--cert-only` stays unchanged. The self-test checks the
  branch target, unchanged return paths, exact changed byte, relocation
  coverage, and rejection of unexpected instruction bytes.

### 3.8 Embedded default login server (optional)

`patch_026g.py --server=host:port` embeds a login address in either full
build. `make_client_026g.py` supplies its existing `--server` argument to
this patch and omits `-server=` from the generated full-build launch line.
Without an embedded address, the old behavior is unchanged. Cert-only
builds retain the old behavior too.

- Original path: `game::on_base_resources_created` reads the `server`
  option at 0x1EAE5F, then checks `replay` at 0x1EB0A4 if it is absent.
  Only when both are absent does `jz` at 0x1EB0AB reach the fatal error at
  **0x1EB25B** (`push 0x9FA990; call 0x113580`). [V]
- Replace those ten error bytes with a relative jump and five NOPs. The
  hook enters `default_server_026g.asm`, appended after the unchanged
  420-byte snapshot code in either cave. Its assembled stub is 51 bytes;
  the patcher appends the validated ASCII address and NUL. [V]
- The stub initializes the parsed `server` key at **0xA9C550**: copy to
  its existing 512-byte buffer, update the end pointer at +4 excluding
  NUL, and set the type at +0x224 to **4 (`type_string`)**. It jumps back
  to **0x1EAE2D** to rebuild the temporary string (begin, end and buffer
  limit), then the original `is_set_as_string` call at 0x1EAE5F fills it
  and selects the normal startup path. ECX, EDX,
  ESI and EDI are saved/restored; EBX and EBP are untouched. [V]
- Both the string and key are located relative to call/pop; the return
  jump is also relative. No injected absolute pointer needs relocation.
  The original hook's push operand relocation is neutralized by the
  existing range logic.
- Input investigation: filling only the temporary stack string and jumping to
  0x1EAE90 left the parsed key `type_unset`. That draft displayed the
  login screen but did not accept typing in manual tests; supplying an
  explicit `-server=` restored input. Initialize the key and re-enter
  the original option read instead. This is an observed correlation; the
  downstream cause of the typing failure has not been established. No
  direct keyboard use of this key was found. In the controlled A/B retest
  on 2026-10-12, both the original fallback and the corrected fallback
  accepted typing with the same shell launcher and Wine prefix. Only the
  fallback payload and PE checksum differed. The absent-key explanation
  is therefore unsupported; the original input failure remains unreproduced.
- Re-entry regression: 0x1EAE47 skipped the `lea edx,[ebp-0x288]` that
  sets the temporary string's buffer limit, then stored the stale EDX
  at +8. An execution harness using the original startup instructions
  reproduced the wrong limit; returning to 0x1EAE2D fixes it. This is a
  separate proven defect, not an established cause of the input failure.
- Explicit server values (including malformed ones) and replay selection
  remain untouched. Only the login address is embedded: the TLS login
  reply supplies the lobby address, and the lobby supplies the match
  address (`login_chat.md` sections 2 and 4.1).
- Hostnames are at most 63 ASCII bytes (the login host field is 64 bytes
  including NUL), and ports are 1..65535. The self-test checks both caves,
  a normal address and the maximum length, and the unmodified variants.
  Byte/disassembly checks cover all seven variants. A native 32-bit
  execution harness verifies both caves, normal/max-length addresses,
  rebasing, buffer bounds, key type and register/stack preservation.
  Sixteen further cases execute the original startup and key-read code
  for both caves and address lengths, covering fallback, explicit server,
  replay, and both options together, including the stack buffer limit.
  Under Wine Staging 11.8 / DXVK 3.1.1, the section build was launched
  through the existing shell launcher with `-server=` removed; the user
  confirmed the final revision-2 build accepts input. Lobby handoff and match
  entry were not separately verified.

### 3.9 The battery rule: two instructions become nops (written 2026-10-12, patch revision 3) — PROVEN bytes, the handler run outside the game, not run live

- **Why.** match.md §26: the rule's completion (the time at +0x15C, the "containers can be used" byte at +0x160) is not serialized, a rollback of the completing tick leaves it set, and the tenth battery cannot be put in again; the containers are closed for the rest of the match.
- **Where.** `gather_victory_items_rule_core`'s event handler 0x313A50 (mode 3's rule shares it):
  - 0x313A62 `0F 85 D3 00 00 00` (`jnz 313B3Bh`, after `cmp dword [ecx+15Ch],-1`) → six `90`: an event after a completion is counted.
  - 0x313B34 `C6 81 60 01 00 00 00` (`mov byte [ecx+160h],0`, between `mov [ecx+15Ch],edx` and `ret 0Ch`) → seven `90`: the containers stay usable.
- **What is left:** the handler counts every put and take, sets the completion time when a team has all and takes it back when not: 0.21d's handler 0x1FB8A9, with 0.26g's 3 s.
- **Checked** (`selftest_026g.py` check 13): the instructions around each site, no base relocation on either, no branch into either, `mov byte [reg+160h],0` nowhere else in the rule's code.
- **Run outside the game** (2026-10-13, `client-patches/run_battery_rule.ps1`, Windows 11): the handler's 238 bytes (0x313A50..0x313B3D; no call, no absolute address) copied out of an exe and called in a 32-bit process on a made-up rule object (ten batteries, `+0x15C` = -1, `+0x160` = 1, a world with no lobby menu). Revision 2 (the players' exe): nine puts 9:0; the tenth 10:0, time t + 3000, byte 0; the count set back to 9 as a rollback does; the same put again: **not counted**, 9:0, byte 0; a take and a put: not counted. Revision 3: the tenth 10:0, time set, byte 1; after the count is set back the same put gives 10:0 again; a take gives 9:0 and time -1; a put 10:0 and a new time; a count set back with no replay leaves the time set until the next event, which takes it back. This is the handler alone: `can_use`, the rollback itself and the server's reading of the world were not run.
- Full builds only; `--cert-only` keeps the handler.
- **Builds** (revision 3, with everything of §3.7, §3.8 and §6): section sha256 `eedc5b5c9aa6111c999267eef8c6c27688d012aa203a7c6f6ccdbbc93262cf0d` (12 895 352 bytes), hash-mismatch `59eb5dfb1d1edb78e62d40001de04c47a10d6e3d4c22051f44be023112f49746`; with `--server=skhid.link:1234`, as the players' download is made, the section build is `ac947abd212d05f3172035967076517f057019b9e1499be06c28b622baf36d7b`. That one differs from the exe of the players' download of 2026-10-13 (revision 2, `30e928dd…4883`) in these 13 bytes (file 0x302E62, 0x302F34), the revision byte (file 0x36FC66, `02` → `03`) and the PE checksum, and in nothing else (compared byte by byte). The self-test counts 61 bytes in 17 ranges for the section build and 1220 in 60 for hash-mismatch.
- **Published:** in the players' download since 2026-10-13 12:22 UTC (a game started through the launcher takes it by itself, a game with an older download keeps the old rule until its player takes the ZIP; old and new games part at the tenth battery, match.md §26). **Not done:** a match of two or more players watched to a team's last battery.

### 3.10 A Documents folder the game may not write to: two start-up crashes (found and written 2026-10-13, patch revision 4) — PROVEN bytes, both crashes made again and the built exe started; not run past the sign-in screen

- **The reports.** Two BugTrap reports of one player, half an hour apart, each about two seconds after the game's start (the second before the renderer's DLLs were loaded), both with the user folder on another drive than the system's:
  - a breakpoint at 0x40141 with "assertion_failed / fatal error / failed opening file:`<Documents>\Survarium\Survarium_<Windows user>.journal` with open flags:create_always+read_write", `windows_hdd_file_system.cpp` line 97;
  - an access violation at 0xF9A8F (`esi` = 0, `edx` = 0xF9A80; the stack: 0x218980, 0x10E2DC).
- **The user folder.** 0x18BD0: `SHGetFolderPathA(NULL, CSIDL_PERSONAL, …)` (the call at 0x18C2A; a delay-loaded import), lower-cased, plus `\` and the application's name. No option and no setting names another folder. The engine mounts it as `user_data` at its start (0x14D3C…0x14DED; `user_data/user.cfg`, `user_data/crashes`, `user_data/streamed_textures`), and makes the folder itself when it is missing (a file of that name in its place is deleted first).
- **The second report: a mount without a root.** `vfs::mounter::finish` 0x469F0 ends every mount: the caller's callback (0x46ACC), then, when the result has a mount and it is not a lazy one, `root = [mount+34h]`, `ecx = root ? [root+40h] : 0`, and `file_system.on_mounted(ecx)` (0x46B31), which is `resources_manager::on_mounted` 0xF9A80 (bound in the manager's constructor, 0xF9288). That one reads `[node+30h]` at 0xF9A8F without looking. When the physical path is not there the mount fails ("mount physical path doesnt exist"), the result still has its mount object, the object has no root, and the node is null. The game's own message for it, "Cannot mount user_data folder. Please reinstall the application and try again." (0x148B0), is shown only for a result with no mount object at all, so it is never reached this way.
- **The first report: the journal.** The game's start (0x1F6940…0x1F69CB) decides the journal's use: 2 (replay) with `-replay_journal`, else 1 (record) when the option object `-record_journal` (0xA9D628) reads as set, else 0. `journaling`'s start 0xEB310 makes no journal for 0; for 1 it opens `<user folder>\Survarium_<user>.journal` with create_always and read_write through `journaling::open_file` 0x103EE0, whose open asks for an assertion on failure. The option is set by default: its initializer 0x851CC2 is `push 0A9D628h` / `mov eax,9DB51Ch` (the text "1") / `call 0DDFF0h`, which stores the number 1; 0x145C0 then reads "set". The option's description in the exe is "disable journal recording".
  - Every use of the journal object (42 places that name its flag 0xA8394C) tests the flag first, apart from the places that set or clear it.
  - The file is opened with `FILE_SHARE_READ | FILE_SHARE_WRITE` (0x400C4), so a second game on the same PC is not what refuses it; two games on one PC run.
- **The patch.**
  - 0x46B0B `33 C9 EB 03` (`xor ecx,ecx` / `jmp short 46B12h`) → `EB 29 90 90` (`jmp short 46B36h`, the function's four pops and `ret 0Ch`, where its other early exits go): a mount without a root is not announced. The path of a mount with a root is untouched. No relocation, no branch into the four bytes.
  - 0x851CC8 `1C B5 9D 00` → `0C BF 9D 00`: the default becomes the exe's own text "0" at 0x9DBF0C, the one three other options are given by the same three instructions (0x84CF92, 0x84E4F2, 0x853B02) and `bool_from_string` compares with. The base relocation at 0x851CC8 stays active (the tool does not count the site as rewritten code). `-record_journal=1` records as before.
- **Made again** (2026-10-13, Windows 11, a revision 1 exe, the second download's, started under the Windows debug API with `USERPROFILE` naming a throwaway folder, so that its Documents folder is that one's): a journal file with the read-only attribute, or a folder `Survarium` that refuses new files (a deny entry for "add file" and "add subfolder"), gives the first report's assertion, text and address, with the report's `ecx`, `esi` and `edi`; a Documents folder that refuses new folders gives the second report's access violation with the report's registers, and the stack shows `mounter::finish` (0x46B36), the physical path mounter (0x4FB90) and `query_mount_impl` (0x4D68E) above the two frames of the report. With the first patch alone the second situation runs into the first one's assertion: the two are one cause.
- **The built exe started** (same day, 28 s each, to the sign-in screen): Documents refuses new folders: no crash, one "mount physical path doesnt exist" line in the game's log; the folder refuses new files: no crash; an ordinary folder: no journal file, the log file as before; the same with `-record_journal=1`: the journal file is made.
- **Checked** (`selftest_026g.py` check 14): the instructions of `mounter::finish` from the read of the root to its end, before and after; no relocation on and no branch into the four bytes; the receiver's two instructions; the two texts; the initializer before and after; the three other initializers; the option's name and description; the relocation active in both; the option named nowhere but where it is registered and at 0x1F69B2; `journaling`'s test of the use.
- **Builds** (revision 4): section sha256 `3bd35a2078ab9288c1e30773cb4ade74f719fee15b2b5bc6ee39ca2f237d39fd` (12 895 352 bytes), hash-mismatch `74ae60e877a53fd82870694c8abe601ea3ebbbe177b367f88e8db86fc738a173`; 67 bytes in 19 ranges and 1227 in 62 differ from the original. With `--server=skhid.link:1234`, as the players' download is made, the section build is `ae3ddf43f38eecb9e5080b10aa6b8397ce5dbed0f133b381461c82cb17329f25`; it differs from the revision 3 exe made the same way (`ac947abd…6d7b`) in the four bytes at file 0x35F0B, two bytes of the address at file 0x8410C8, the revision byte (file 0x36FC66, `03` → `04`) and the PE checksum, and in nothing else (compared byte by byte).
- **What stays as it is:** a game in such a folder is played with settings that are not saved (the save, `console_commands::save` 0xEBE60 through 0xF7020, returns false when the file cannot be opened; read, not run), no log file and no local copy of a crash report in the user folder. The crash reporter of the download tells the player at the game's start (`client/bugtrap/README.md`, "Start-up notice": a message box in the game's language, from inside the game's process). What refuses the folder on a player's PC is not known from the reports (Windows' "Controlled folder access" for a program it does not know, or file rights on a moved Documents folder, would).
- **Published:** in the players' download since 2026-10-13 15:36 UTC, with the crash reporter that tells the player; the first sign-in with revision 4 on the players' server was logged three minutes later. **Not done by us:** a sign-in and a match with the built exe; the options saved into a folder that refuses the file.
- Full builds only; `--cert-only` keeps both sites.

### 3.11 The late join: a player enters a match being played (written by sushi-shi 2026-10-12/13 as an exe of its own, made patch revision 5 on 2026-10-13) — the code's own checks pass on the revision 5 exe; run in a game only as the hand-built exe, not as revision 5

What it is for, what the game does and what was run: `client-patches/README.md`, "The late join". Here: where it is in the exe, and what the release changed.

- **The sites** (`patch_026g.py`, `LATE_JOIN_PATCHES` and `LATE_JOIN_HOOKS`; each is checked against the original's bytes first). Rewritten in place, eight: `base_match_client::set_players_count` in both implementations (0x279610, 0x27A2E0: 17 bytes that worked out the mask size from the count become `mov eax,3` and nops), the kd stats rule's count (0x3237B4, `mov dl,[ecx+577B5h]` → `mov dl,14h`), the statistics handler's count (0x26D8F3, `mov al,[edx+577B5h]` → `mov al,14h`, since revision 9: §3.16), and the four loop ends of `shared_statistics::serialize` and `deserialize` (0x2F5ACF, 0x2F5AF4, 0x2F5BE6, 0x2F5C03: the compare with the live count at `+31E0h` → `cmp bl,14h`). Calls or jumps into the new code, eight, and the ready hook the upload patch has already: a player's animations (0x2CBF9B), a slot's ids (0x2D3880), the histories' constructor loop (0x2D3906), 0xD8's debug byte (0x27A23D), the 0xE4 case of `on_match_packet_received` (0x27A378), the player's deleting destructor (0x1F2013), `on_match_disconnected` (0x27B517), the `network_client` destructor (0x219E60), and `on_match_ready` (0x27AEA1, whose jump goes to the late join's `ready_hook`, which goes on to the upload code). None of the 16 holds an address: no base relocation is under one.
- **The code** is the second page of `.srp` and what follows: 4096 bytes of `late_join_026g.asm`, whose entries are at fixed offsets (0, 96, 128, 1664, 1856, 2048, 2336, 2432, 3072), then the C code (3513 bytes as built). The first page of the section is what it is without the late join: the 420 bytes of upload code, then the default-server stub and the address when one is given. The state (a lock, the list of profiles being loaded, the game's functions it calls, the match it belongs to) is one zeroed, writable page of its own, `.srpd`, on the page after the code: RVA 0xA418000 with the code as built, no bytes in the file. So the exe has nine sections, `.srp` is 0x2DB9 bytes (0x2E00 in the file) and the certificate table moves to file 0xC4D800.
- **What the release changed against the hand-built exe.** That exe had the code right behind the upload code, at offset 420 of the section, which is where the default-server stub goes: it could not have an address inside. The code now starts at the second page, whatever the first holds, and the exe has an address like any other (`--server=`). The code itself is the same sources; the C part is built by gcc 13.2.0 (Debian) here, another compiler than the author's, so the bytes differ. `late_join_026g.section.bin` is checked in with a `.json` that says where the C functions are and what built it; the release tools read it and need no compiler. `patch_026g.py --no-late-join` gives the revision 4 exe byte for byte (`3bd35a20…39fd`; `ae3ddf43…9f25` with `--server=skhid.link:1234`).
- **Builds** (revision 6, with §3.12 and §3.13; revision 7's are in §3.14): 12 906 616 bytes, sha256 `c85026bb2224fdfad3f3219545e08615349c039a5e80c823abbc4f9083747bcc`; with `--server=skhid.link:1234` `82265418234cf12ad2cd410567608d84bc150cfb152e99fd79e34880f93551ba`. `late_join_026g.section.bin`: 7609 bytes, sha256 `346df54b03816accdb3c64ed6d4687d795ca1c299771716836723e366208afaf`; built twice, the same bytes; NASM 2.16.03 on Windows gives the 4096 bytes of assembly that 2.16.01 gave.
- **Checked** (`selftest_026g.py` check 15, 2026-10-13: 670 checks in all pass; 673 with §3.12's, 2026-10-14, and the counts of the next sentence are then 135 bytes in 27 ranges): the build with the late join against the build without it, for no server, a server and the longest server name: 133 bytes differ before `.srp`, all in the 26 intended ranges (the 15 sites, four bytes of the ready hook, the revision byte, the header fields and the two section entries); each rewritten site disassembles to its instruction and nops, each hook to a call or jump to its entry and nops; the ready hook's code starts with `pushad` and jumps on to the upload code; the relocation table is the build's without it; `.srpd` is where the code looks for it; Windows computes the same checksum and reads the certificate. The C tests pass under AddressSanitizer and UBSan (with `setarch -R`: the sanitizer of this compiler loops at start otherwise). `check_late_join_026g.py`, with its addresses moved to the new place, passes on the revision 5 exe (2026-10-14, Unicorn 2.1.4): the patched machine code run on made-up objects, as the README says. **Run with the revision 5 exe** (2026-10-13, the test server): a match of two, in step for eight minutes with one resync, and the prediction line each minute; a join into it was tried and not completed, for a reason of the test and not of the code (§3.12). **Not run:** a join that completes.
- **Not in `--cave=hash-mismatch`:** that build has no section to put the code in, and no writable page. It stays the fallback of revision 4.


### 3.12 A game that goes on without the focus (written 2026-10-14, patch revision 5) — PROVEN bytes, run in the lobby, not run in a match

- **What the stock game does** (PROVEN, `transport.md` §6.1): `engine_world::logic_tick` 0x1A030, once the window was shown (engine_world+0x313), asks `is_application_active` (`mov edx,[ebx+8]` / `mov eax,[edx+30h]` / `lea ecx,[ebx+8]` / `call eax` at 0x1A0E1..0x1A0EB), then `test al,al` / `jz 1A113h` (0x1A0EC, 0x1A0EE). Active: `game::tick` (the game's vtable +8, 0x1A0F0..0x1A104). Not active: 0x1A113, the game's vtable +0xC, `on_waiting_for_render`, which sends what is queued and ticks the lobby link, and nothing of the game moves. Seen live 2026-10-13 with three games on one PC: only the one in front played; the two of a match never named the same tick in their hash reports (49 uploads vouched by player 0 alone, 7 by player 1 alone), and a third that was taken in got its world after 28 s and was given up 60 s after its place was reserved (`new place 2 timed out during preparation or catch-up`). The owner saw the same on the screen: the timer of a window without the focus stands still and jumps on when it has it again.
- **Why it matters for the late join:** `game/late_join.rs` lets a newcomer in when every game of the match has its resources loaded and its roster installed (`resources_loaded && installed_roster > index`), and a newcomer's own catch-up hash must be one the majority names for the same tick within 1.5 s. A game that stands still answers neither. One player at another window holds up every join.
- **The patch** (`RUN_UNFOCUSED`): `74 23` at 0x1A0EE becomes `90 90`. The call and the test before it are kept and checked as the original's. No relocation is under it. In the builds of revision 5 only; the build without the late join keeps the jump (the self-test checks all three).
- **The other gate is untouched:** before the active check `logic_tick` reads engine_world+0x30E (copied to +0x30F) and goes to the same not-active branch when it is zero (0x1A0CE..0x1A0D6). What sets it was not read; with a minimised window the patched game went on (below), so it was not zero there.
- **Measured** (2026-10-14, one PC, one game in the test server's lobby, windowed 1280x720, the focus taken away by a script for 15 s, then the window minimised for 15 s; `client-patches/README.md` patch 12 has the table): without the patch the game's log says `eating time: 18.444` and `15.550` after the two and it used a whole core while it stood still; with it no `eating time` line comes, it used 46 % of a core not in front (42 % in front) and 146 % minimised. Windows' cursor is shown and free on the whole desktop while the game is not in front, with and without the patch (`GetClipCursor` = the desktop; over the game too).
- **Not run:** any match with it; exclusive fullscreen with alt-tab (the settings file is in Documents, which this PC's folder protection lets only the game write: it has to be set in the game); the mouse in a match.
- **A crash found the same night, not of this patch:** a game whose join the server gave up (it had its world and was in sync, never entered) died when the server closed its link: access violation at 0x15EE8D writing 0xD1, right after `network_client::close_current_match [disconnected_by_initiator]` and `remove_model: model is not inserted now`. Not looked into yet. A player whose join times out would get it.
- **The test-session build does not work now:** `late_join_026g.py --test-session N` (the exe that passes the login by for `late_join_native`) dies at its direct connection: access violation at 0x110D9 writing 0x3BFDC4 (an address in `.text`), called from `direct_match` (the code at +2560) through 0x21C0AF, 0x27A1E6 and 0x37A132, three times out of three, 2026-10-14. Not looked into.

### 3.13 Three faults of a match that takes players in (found and written 2026-10-13/14, patch revision 6) — PROVEN bytes, each run with Unicorn, not run in a game

The first two were seen on the players' server in the first hour after revision 5 was released (three late joins by 2026-10-14 00:07 UTC: the first and the third completed, the second killed one game of its match and the newcomer's game stopped answering twice); the third on the test server the hour before.

- **The hud's lists of players.** PROVEN: 0x234880 (called once, from 0x1F83D0 at 0x1F855D with the hud = game_world+0x1FC and the options = the network client's vtable+0x34, `[nc+1F1C48h]+0Ch`; 0x1F83D0 is called from the match-ready path 0x27AB1F and from 0x2190BC) makes `root.list_set_players` on the hud's movie +8 (rows `id`, `team`, `name`, `ping`, `rank`, `artifacts`, `squad_state`, `squad_id`), `root.set_players_list` on the movie +0x14 when there is one (rows `id`, `team`, `name`) and `root.set_enemy_icon_ids` on the movie +0x10; the three loops end at the count `options+577B5h`, which `lj_native_install` raises. A row's numbers are refreshed by 0x236D30 (`root.list_update_player`), called for every place below the count each tick the list is shown (0x23B42C..0x23B475), so a place with no row was refreshed into nothing. Seen live: the two players of a match did not have the newcomer in their scoreboard.
- **The fix:** `install_context` (the late-join code) calls `rebuild_lists(hud, options)` after a successful install, with the two as 0x1F8542..0x1F855D pass them. It checks that the hud is the one this match's start made (`[hud+1Ch]` = the options, both movies there), builds 0x234880's frame (`sub esp,5D4h`, the four registers, `ebp` = the hud, `ebx` = `[esp+98h]` = hud+8, zero in the two values the function's end releases besides the arrays, `[esp+0DCh]` and `[esp+160h]`) and jumps to 0x234E71. `lists_hook` at 0x23572B (`mov ecx,[esp+98h]`, right after the third invocation) compares the frame's return address with `install_context`'s: that run goes to 0x2363CA, the function's own release of the two arrays and its `ret 8`; the game's own run does the seven bytes and goes on at 0x235732.
- **A box that is no box.** Seen live (the dump of 2026-10-13 23:49:30 UTC, 0.3 s after the server reserved a place for a newcomer in a match whose play had begun 0.4 s before that): access violation at 0x399A8 in `loose_oct_tree::update_bounds` 0x396F0 (`mov eax,[esi+0Ch]` / `mov edx,[eax+20h]`: the free node list is empty), returning to `insert_impl` (0x39DFF), `insert` (0x39E76), the tree's `move` (0x3A2B1, its erase-and-insert branch) and `scene::update_model` 0x14FED0 (0x14FEFC: `models_tree->move(model+118h, model->vtable[4Ch]())`, the tree at scene+0x9B7664): a model that was in the scene, moved. PROVEN in the dump: the box in `insert_impl`'s frame is `FFFFFFFF 7FFFFFFF FFFFFFFF` for the centre and for the half extents, `xmm7` is a not-a-number and `xmm3`, the tree's doubled size, is infinity. PROVEN in the code (and in the engine's 0.100b source, `vostok/collision/sources/loose_oct_tree.cpp`): `out_of_bounds` 0x39C90 is true for a not-a-number; `update_bounds` works out the size to reach as `extents * pow(2, power)` with `power` the floor of a logarithm of a not-a-number taken as unsigned (`edi` 0xFFFFFFFE in the dump), which is infinity, and its loop `for (radius = extents*2; radius <= new_radius; radius *= 2)` takes a node for a new root each turn and never ends. 0.26g's trees are fixed pools with nothing behind them (the constructor 0x393B0 takes the memory and the count and strings the free list through node+0x20; the scene makes four of 0x400 nodes of 0x28 bytes at 0x14F919, 0x14F94A, 0x14F97B, 0x14F9AC), so the free list ends: **a bigger pool ends the same way.** The game's other threads went on (the server resynced that game 5 s later and heard it for 12 s). LIKELY: a model of the newcomer with no place yet; which one was not found (the heap is not in the dump), and the other game of the match lived. The newcomer's own game loaded the world twice (in step at 23:49:35.6 and 23:50:18.7) and neither time got as far as agreeing on a tick with the others, which took the two joins that completed about a second; the first time the match server last heard it at 23:49:40.8, 50 s before it gave the join up (23:50:30.0), so that was not the fault of a join given up; the second time the lobby last heard it at about the giving up. Its player reports the same tree out of nodes while loading: LIKELY the same fault, their dump not seen here. `box_hook` at 0x39E52 (in `insert` 0x39E30, after the object's vtable+0x20 gave the box, before it is copied to object+4): a box with an exponent of all ones in any of its six floats becomes (-1,-1,-1)..(1,1,1). `insert` is the tree's vtable+0 (0x9DE254), the one way a new box gets in: `move` calls it in both its branches (0x3A690, 0x3A2AF), and the two other callers of `insert_impl` (0x3A67B, 0x3A75B) put back objects with the boxes they were stored with. The exe has one copy of this code for all its trees (the scene's four, the collision world's). A finite box is not touched, so nothing a match computes changes.
- **A model that is not in the scene.** Seen on the test server (2026-10-13 22:42 UTC, the dump of a game that had loaded a match, was never let in and was sent back after 60 s: `close_current_match`, then this). PROVEN: `scene::remove_model` 0x14FF60 (the scene, the model's pointer by value) finds the model in the list at scene+0x9B358C (0x14FF88, `cmp edi,esi` / `jnz 15006Ch`); not found, it logs (0x150012, "remove_model: model is not inserted now") and falls into the same code: 0x155D00, the model's vtable+0x40, the tree's erase (0x15019A), and `erase(it, it+1)` with `it` = the end (0x1501B4), which copies what lies after the list over its end and adds to a reference count through it (0x15EE8D, the crash). The second list in the same function is erased only when found (0x1501D9), and the function's end (0x1501F2) lets go of the pointer it was given (`lock xadd [model+0D0h]`, 0xE0210 at zero). `add_model` 0x14FE60 puts a model into the list and into the tree together and nothing but the two takes one out, and the tree's erase (0x3A600 reads the object's node) did not fault before the crash: LIKELY the model is in another scene's list and tree. `REMOVE_MODEL`: the six bytes at 0x14FF9B (`cmp [1F1D60Ch],ebx`) become `jmp 1501F2h` and a nop; the base relocation at 0x14FF9D is disabled. So this scene is not touched and the pointer is let go of; a scene that holds the model keeps it and lets go of it in its own time. (The first writing of this jumped to 0x150215, past the letting go: the model would have been kept for ever.)
- **Revision 6.** `REVISION` is 6; `late_join::spoken_by` takes 5 and later, so 5 and 6 are one queue and play together. A server asks for 6 (`SRP_MIN_PATCH_REVISION=6`): a game of 5 in a match can be killed when a player joins.
- **Checked:** `selftest_026g.py` 682 checks (the two hooks as calls into the code, the jump in `remove_model` and the function's end it goes to, the one relocation entry, 158 or 159 bytes before `.srp` in 31 ranges; no branch of the original into the replaced bytes). `check_revision6_026g.py` (Unicorn 2.1.4): see `client-patches/README.md` patch 13. `late_join_026g.section.bin`: 7609 bytes still (the new routines are in the assembly's free bytes), built twice with the same bytes.
- **The Flash side** (PROVEN in the decompiled ActionScript of `flash_movies/hud.swf`, `player_icons.swf`, `replay_player.swf`): `HUD.list_set_players` is `PlayerList.addPlayersList`, which sets `_players` to the new array, marks the player's own row by the name `set_local_name` gave, and makes both teams' data providers anew; `set_players_list` (the movie +0x14, the replay player's bar) makes a new data provider; `HUDIconLayer.set_enemy_icon_ids` sets the array. So a second call replaces. A row made anew has kills and deaths 0 until `list_update_player` (`PlayerList.updatePlayer`: by id, when kills, deaths, ping, artifacts or rank differ), which 0x23B421 sends for every place below the count each tick `[hud+0BAh]` is set (`root.show_player_list`, 0x2384C0). The ping the game sends is the constant 0x42 and it sends no score. A player's marker (`root.create_player_icon`, 0x23BD60) is made by `player_ready` 0x1F29D0, which the late-join install calls; the other three callers of 0x23BD60 are in 0x234880 after the lists, for what is not a player, and are not run again.
- **What a row means.** The lists are made again when the place is prepared, before the newcomer is let in; a newcomer whose join fails keeps its row for the rest of the match, as a player of the first roster who never loaded does. (Since patch 39, §3.36, a place has a row only while it is in the match.) The server's results screen (0x97) has a row for every place too, by the place's number (`id` = place + 1), with the numbers of the last trusted snapshot: seen live for both completed joins (the newcomer's score, pay and screen), and a place that never entered is a row of zeros.
- **Not run:** a game. Not known: what moves a model to not-a-numbers; whether a model so taken in is drawn right afterwards; what stopped the newcomer's game of 2026-10-13 23:49 (its dump); which scene holds a model that `remove_model` does not find; that the options the hud was started with (the client's vtable+0x34, `[nc+1F1C48h]+0Ch`) are the object whose count the install raises (`[pvp+124h]`): the game reads the count through both (0x234EC0, 0x23B435) and the late join checks the match's seed through both, but no run has shown one pointer equal to the other; were they two, the lists would be made again without the newcomer and nothing else would differ.

### 3.14 A newcomer's grenades and traps: the order of the saved objects (found and written 2026-10-14, patch revision 7) — PROVEN bytes, run with Unicorn, not run in a game

Seen on the players' server in the first hour of revision 6: a player whose three kits each had two grenades was taken into matches being played four times in three matches and never let in (the server gave each join up after 60 s); the six players taken in fine had none. With revision 5 the games already in such a match died (§3.13's second fault).

- **The list.** PROVEN: the world core has one list of what it saves as "game objects": core+0x18914 the first node, +0x18918 the last; a node is a vtable, +4 the node before, +8 the next. `core::serialize` 0x2D2570 walks it (0x2D2699: `node->vtable[0](stream, second stream)`, into the state's buffer at item+0xBBF30, the third length-prefixed part of a world's body, `game_objects_serialized_state`), `core::deserialize` 0x2D2770 walks it again (0x2D2B74: `vtable[1]`, then 0x2D2C98: `vtable[2]`). Each object reads what it wrote, with no length and no id: the order of the list is the layout of the bytes. Nothing else names the list but those that add to it and the game project's removal at the world's end (found by the displacements 0x18910, 0x18914, 0x18918 in the whole code).
- **Who adds to it.** Four functions: `grenade_core`'s bind 0x303820 (node grenade+0x10, vtable 0xA13FE8, or 0xA046D0 in the client's class; two ids taken), called for each grenade of a stack by the grenade item's bind 0x309EC0; the trap item's bind 0x3067A0 (node trap+0x74, vtable 0xA14298 or 0xA04658; one id), for each trap of a stack; the game project's `add_to_world` 0x285750 (four kinds of the map's objects; 0x318800 is the server project's); nothing else. The two binds are an inventory item's vtable+0x70, which the inventory's bind 0x2FC7D0 calls for its 23 slots, called by the kit's bind 0x2D75B0, called for a slot's three kits by 0x321820, whose one caller is the core's start 0x2D3820 (0x2D3880, the hooked call): slots first, then (0x2D3B29) the project. The other item classes (21 vtables with that slot were found: weapons 0x2E30F0, medkit 0x33D360, oxygen tank 0x33E3D0, artefacts, the plain item 0x2FFFD0) take ids or go into lists of their own and are saved by their player's inventory (`medkit::serialize` 0x33DBD0 starts with `inventory_item::serialize`), inside the player's block.
- **The fault.** The late join binds a newcomer's slot with the same 0x321820 when the project's objects are in the list: its grenades came last in the games of the match and first in the newcomer's own. PROVEN in kept worlds: match 0x1f03ab47's game objects are 7 bytes until place 2 was prepared and 13 after, six zero bytes at the end; the first worlds of four other matches begin with a zero byte for each grenade of their first players, before the map's objects (in one of them three of those bytes are `04`, spent, 15 minutes later). `grenade_core::deserialize` 0x303F80 reads one byte (1: in the physics world, 2: thrown, 4: spent), eight more when thrown and 88 more (a transform, two velocities) when in the physics world, and on those bits puts the grenade into the tick list (vtable+0x10) and the physics world (vtable+0x18): a byte of another object read as `FF` is a grenade thrown to a place of what follows it. The trap's 0x3053C0 reads a state byte and more when it is not 0.
- **The tick lists are not it.** The seven lists at core+0x18840 (count, first, last; a node is `this`, a function, +8 before, +0xC next, +0x10 an id, +0x14 the list's number) are walked each tick in the order of the nodes' ids (0x2D1D20 copies a list, sorts by +0x10 and calls), and a place's ids are of its own range since revision 5: when a node was added does not matter there. A grenade's node goes into list 6 when it is thrown (0x303B90) and out when it is spent.
- **The fix, the order** (`late_join_026g.asm`, `bind_slot_reserved`, the code the hooked call and the late join both bind a slot through): the first and the last of the list are kept and zeroed, the game binds the slot, and when the slot left nodes the kept ones follow its last (`last+8` = the kept first, the kept first's `+4` = that last, the list's last = the kept last); a slot that left none gets the list back as it was. The list is then the slots' objects from the newest slot to the first, each slot's in the order the game bound them, and the map's.
- **The fix, the states** (`late_join_install_026g.c`): before `bind->core` the list's first node is kept; after it the nodes up to that one are counted (`added`), and `shift_saved_objects` walks the states (core+0x2FBC the oldest, item+0xC0BAC the next; 11 of them, 128 with `-debug_hash_mismatch`): the buffer at item+0xBDF30 (`+4` the bytes, `+8` the room, `+0xC` the size; 0x3246D0 makes it with the bytes at item+0xBBF30, room 0x2000 and no allocator, so it never grows) gets its bytes moved up by `added` and zeroes in front. PROVEN that an untouched grenade and trap save as one zero byte: `grenade_core::serialize` 0x303DA0 writes `(spent ? 4 : 0) | (in the physics world ? 1 : 0) | (thrown ? 2 : 0)` and nothing more when both of the last are off, and the constructor 0x303640 starts with all three off; `booby_trap_core::serialize` 0x3052E0 writes the state's byte and nothing more for 0, which the constructor 0x3043C0 sets. A state whose hash was worked out already keeps it (item+0xC0BB4, +0xC0BB8): it was reported.
- **Why the states matter.** 0.26g rolls back: an input for a past tick has `move` load the state before it (0x2D2FF0, 0x2D357A) and play the ticks again. A state saved before the install, loaded after it, has fewer bytes than the list has objects. With the nodes at the list's end (revisions 5 and 6) the newcomer's objects read what lies behind the bytes; with them in front every object would read out of step. With the bytes put in, a load of such a state leaves the newcomer's objects untouched, which is what they were.
- **While the games install.** Each game installs when it has loaded the newcomer's kits, so for a moment one game's world has the bytes and another's has not, and their hashes differ though neither is wrong (so it was in 5 and 6 for such a newcomer). The match server resynced a game whose reports were the minority's for a second (`divergence_confirm_ms`): with a world of the other kind. Now `Game::check_hashes` judges no report of a tick up to the moment the last game of the match has the place (`Game::installing`: a game that has loaded and has a place to install whose admission did not fail), and forgets what stood against a game before. A place whose admission failed stays as each game has it, as before.
- **Revision 7.** `REVISION` is 7. Two slots' grenades are in the other order than in 5 and 6, so the games part when one is not as it began: a server asks for 7 (`SRP_MIN_PATCH_REVISION=7`), put up with the ZIP.
- **Builds:** 12 907 128 bytes (512 more than revision 6: `.srp` is 0x2E4E bytes, 0x3000 in the file), sha256 `a83aca6b1c281c0fa5563319308f28caa80e90787284ee37ab31cc6f3539f95c`; with `--server=skhid.link:1234` `8b79f306f1c99c6ddcda64420a124a56fa768a559b2da0c0fc28fa186423a11e`. `late_join_026g.section.bin`: 7758 bytes, sha256 `4b97a9a5a39fbc85a89c65b86f743882c6670ac71ef49699459186614315ff51` (gcc 13.2.0, GNU ld 2.41.90, NASM 2.16.01; built twice, the same bytes). `--no-late-join` gives the revision 4 exe as before (`3bd35a20…39fd`, `ae3ddf43…9f25`).
- **Checked:** `check_late_join_026g.py` (Unicorn 2.1.4): `saved_objects` runs 0x321820, 0x2D75B0, 0x2FC7D0, 0x309EC0, 0x303820 and 0x3067A0 through the hooked call on made-up kits (slots at a start, three objects of the map appended as 0x285864 does, slots after them, a slot with none, a list of the map's alone) and reads the list back with every link; `native_preparation` runs the whole install for newcomers with (2, 0), (0, 1), (6, 3) and (1, 1) grenades and traps into a list of two and four states (7 bytes, none, 0x1FFE, 3), and checks the list, the ids, every state's bytes and size and the bytes behind them. Seven copies with one thing broken (the nodes appended as before, the link back missing, the last not restored, the states not touched, the bytes behind instead of in front, no look at the room, the oldest state only) fail it. `untouched_objects` runs the constructors (`grenade_core` 0x303640; the trap's 0x3043C0 as far as 0x3044E3; the two base constructors 0xF70F0 and 0xF17E0 are a `ret`), then each node's `serialize` and `deserialize`: one zero byte written, one byte read, the object the same byte for byte, nothing called but the grenade's look for its owner; with the grenade thrown or the trap armed it fails. `selftest_026g.py`: 682 checks. Server: 1,214 tests, two new, which fail without the change.
- **Not run:** a game. **Not known:** whether a join while a grenade is in the air agrees (the newcomer's game puts the bodies into the physics world in the list's order, the others had them in the order they were thrown; so it is for any world loaded then, also without the late join); what a player saw who, taken in late, "was not hurt by grenades" (the splash damage 0x339630 asks the physics world for the bodies in reach; nothing was found that leaves a late player out).

### 3.15 The game's clock is brought up at 30 % (found and written 2026-10-14, patch revision 8) — PROVEN bytes, not run in a game

Seen on the players' server on 2026-10-14 (match 0x69f8c0ba): after every resync a player at a round trip of 196 ms had 6 to 9 s in which the server dropped nearly every input, a player at 300 ms 13 to 15 s, a player at 22 ms about one. Twelve resyncs of the first in four minutes.

- **Where the time goes.** A client that starts from a snapshot has its clock put at the snapshot + 350 ms (`game.rs::sync_plan`, `SYNC_LEAD_MS`; since 2026-10-14 only a client that enters, `sync_start`: a client resynced while it plays is put at the match clock, match.md §30); the snapshot is its uploader's event-horizon item (500 ms old there) and ages by the round trip of the 0xE5 that the client must answer first (`begin_timing`). Measured: the clock is 250 to 264 ms behind at 22 ms, 462 to 484 at 196 ms, 539 to 566 at 300 ms. The server then sends the lag as 0xE7, the game makes it up at 5 %, and an input more than `HORIZON_MS` (250) late is discarded: until the lag is under 250 ms less the way there, every input.
- **The 5 % is a setting.** PROVEN: `floating_timer::set_offset` 0x1EE9E0 (the only caller is the 0xE7 handler 0x27B7F0, with the timer `game`+0x52D8 and the i16 as it came, no clamp) first rebases the timer (0x146B60: the time read by 0x147110 becomes the base, a correction that still runs keeps what is left of it for the same factor), then stores `[timer+0x28]` times +1 or -1 (the offset's sign; the floats 0xA70884 and 0xA2D778) at +0x24 and puts the end `offset / (factor * 1000)` seconds from now (+0x10). 0x147110 reads `base + elapsed * [+0x20] + min(elapsed, end - base) * [+0x24]`: the clock runs at 1 +- the factor until the end. `[timer+0x28]` is written once, by the `game` constructor (0x1E9848 `movss xmm0,[0A799C8h]` ... 0x1E986D `movss [esi+28h],xmm0`). 0xA799C8 is in .data and is the value of a console variable: the object at 0xA9D0D0 (registered by 0x851330 into the list at 0x1F1D66C; name at +0x10 `time_floating_factor`; +0x40 the value's address, +0x44 the least 0.001 from 0x91A8F0, +0x48 the most 0.5 from 0x927A44; the class of vtable 0x9E7C6C, whose text setter 0xECB70 stores a value in range and the least for one out of it). The address 0xA799C8 is in the code twice, the registration and the constructor. `max_ping_time_in_ms` (0xAA1DF8, an integer one, vtable 0x9E7CCC) is kept in `user.cfg`; `time_floating_factor` is not (a player's file read 2026-10-14: 122 lines, that one among them, this one not).
- **A new correction replaces the one that runs**: `set_offset` computes the end from now for the new offset alone. So the server sends the whole lag, and sends again only when the first is done.
- **The patch**: `CLOCK_FACTOR_PATCH` in `patch_026g.py`: the four bytes at 0xA799C8 (file 0xA67DC8), 0.05 to 0.3. In the builds with the late join. No relocation is under them (checked at build and in the self-test).
- **What depends on the factor**: the timer's speed and nothing else. The simulation plays ticks of 50 ms up to the clock's time (`move` 0x2D2E40), so a faster clock plays them sooner; no rule reads the timer's rate. Games of 0.05 and of 0.3 play one match.
- **The server** (`match_msgs::clock`, `game.rs`): `Peer::clock_factor` from the patch revision of the session that connects (`server.rs::revision_of`: the store's `client_revision`; a game back in its place after an update is asked anew). `correct_clock` waits `offset / factor` and a second before it measures again. Taken for the original, a fast game is right in its own 1.3 s and the server only waits longer; taken for fast, an original gets the lag that is left in place of the correction that runs, which changes nothing (tests of both).
- **Revision 8.** `REVISION` is 8. A server need not ask for it.
- **Builds**: 12 907 128 bytes, sha256 `9673dc5a9053b0901dbcb426cb05b665c2e204d812ed94a05cd148ed45094756` (no server), `5701a96dd56920300d9a493c8160823f4c2dcf20b441c68d460cca2a56386aea` (`--server=skhid.link:1234`). Against the revision 7 exe of the same server: the revision byte, the four bytes and two bytes of the PE checksum, nothing else. `--no-late-join` still gives the revision 4 exe byte for byte.
- **Checked:** `selftest_026g.py` 700 checks (check 16); `late_join_026g.py --check-bin` and the two Unicorn checks as before (their code did not change). Server: 1,223 tests, eight of them new; five copies of the server with one thing of the change broken each all fail one of them.
- **Not run: a game.** Not known: how 130 % and 70 % look and sound for a second or two; whether a frame may play any number of ticks (seven after a sync's load already).

### 3.16 A newcomer's score on the scoreboard (found and written 2026-10-14, patch revision 9) — PROVEN in the code, the game's own code run with Unicorn, not run in a game

Seen on the players' server on 2026-10-14 (match 0x57b21c79: five at the start, five taken in later, places 5 to 9). The scoreboard of a player of the start (Tab, in the match) had every newcomer's score 2.7 to 4 times what the results screen and the server had (1088 for 327, 467 for 116, 149 for 43, 80 for 30, 79 for 23), and the five players of the start, and every kill and death, right. Six minutes in, a newcomer with no kill and no death had 953. The worlds of that match, the scoring player's own uploads among them, hold the right statistics (that newcomer: nothing that scores): the numbers were only in the game's memory. The results screen, the rewards and the quests are the server's, from the statistics, and were right.

- **The scoreboard's score** is the hud row's `rank` (0.21d `game_world_ui::set_player_stats_info` 0x181CD6 names it so; 0.26g `list_update_player` 0x236D30): the hud's cell at +0xC2 + 8 × place, written by `game_world_ui::on_player_score_changed` 0x237050 alone (0.21d 0x181DE4, +0xB6), which 0x234880 (the hud's `initialize_match`) gives the network client as the score-changed callback. Kills and deaths in the same row come from `kd_stats_rule::fill_match_hud_state` (0.21d 0x1FD7C1) every tick, for its count, which the late join already makes 20 (0x3237B4).
- **`base_game_statistics_handler`** (the network client's, +0x1EE8D8; vtable 0xA0755C; 0.21d names, 0.26g the same code at other offsets): `m_scores[20]`, u16 at +0x31F0, which `serialize` (0x26DB40, slot +0x30) does not write: the world holds only the shared statistics at +8. `m_players_count`, u8 at +0x3240. The callback, a `boost::function2` at +0x3218.
  - `start_match` (0x26D8D0, slot +0x3C; called by `on_match_ready` at 0x27AD07, after the hud's `initialize_match` at 0x27AB1F) copies `options+0x577B5`, the roster of the start, into +0x3240, tells the callback `m_scores[i]` for every `i` below it, and copies the roster into the shared statistics' live count (+0x31E8) apart.
  - `on_event` (0x26DB80, slot +0x44; the shared statistics' callback at shared+0x31A0 is the thunk 0x26DEA0, `jmp [vtable+44h]`, called by every event site after it adds to the record, e.g. 0x2F4DE1): `m_scores[place] += floor(value × weight × k + 0.5)`, no bound, and the callback when it changed.
  - `deserialize` (0x26DB60, slot +0x34) = `shared_statistics::deserialize` (0x2F5BE0) + `recalculate_scores` (0x26DC30): for `place < m_players_count` the score from the statistics, stored and told when it differs.
  - `clear` (0x26D880, from `on_match_disconnected` 0x27B4C9) zeroes `m_scores` and the count at every match's end.
- **The rollback.** `game_world_core::move` (0x2D2E40) loads a kept state (0x2D2770, at 0x2D2FF0 and 0x2D357A) whenever an input for a past tick comes; 0x2D2770 calls the handler's slot +0x34 (0x2D2B0F), and the ticks after it are played again, with their events. For a place of the start the load works the score out again, and the events add to that. **For a newcomer's place** (`>= m_players_count`: `lj_native_install` raises `options+0x577B5` and the live count, not +0x3240) the score was never worked out again, so every rollback over one of its events added it once more: two to four times in a match of ten, where nearly every input comes for a past tick.
- **The 953.** `start_match` tells the hud a 0 only for the places below the roster, and the hud is not made anew between matches, so a newcomer's cell showed what a player had had in that place in an earlier match until its first score (LIKELY: the last match of that player's game had a newcomer in the same place, whose 255 would have been shown so inflated).
- **The patch**: `STATISTICS_COUNT` in `patch_026g.py`: `mov al,[edx+577B5h]` at 0x26D8F3 (file 0x25CCF3) → `mov al,14h` + four nops, in the builds with the late join. The loaded value goes into +0x3240 alone (`push edi`, `mov [esi+3240h],al`, then `eax` is loaded anew). With 20, `recalculate_scores` works every place out again at every state loaded, and `start_match` tells the hud a 0 for every place at a match's start. Places with no player have zero statistics (the patched serialize writes all twenty records, and the worlds of real matches have zeros there): a score of 0, never told again. The live count stays the roster's. Readers of +0x3240 in .text: `start_match`'s store and loop, `recalculate_scores`' loop, and four stores of a zeroed register (the constructor 0x26D750, `clear`, the two `finish_match` 0x26D9C0 and 0x278CB0); nothing else. Nothing of the world changes: games of 7, 8 and 9 compute the same match and hash alike. The newcomer's own game was not affected for itself: its static info has every place up to it (`registry.rs`, the join rebuilds it), so its roster count covers it; it was for the places taken in after it.
- **Revision 9.** `REVISION` is 9. It plays with 7 and 8; a server need not ask for it (`late_join::spoken_by`, the clock's factor from 8 on).
- **Builds** (with §3.17): 12 907 128 bytes, sha256 `03202d5ee70b9523560f8e9b362b80939f9f7cf05a6f699bc08e9945700820b7` (no server), `f30c3c666bf87ed6eecaea613523f7259306b7881d3157dde1ed4d80149211a1` (`--server=skhid.link:1234`). Against the revision 8 exe of the same server (built again from `main`: 5701a96d…6aea, the released one): the revision byte, the six bytes, one byte of §3.17's jump and the PE checksum, nothing else. `--no-late-join` still gives the revision 4 exe byte for byte. `late_join_026g.section.bin` is unchanged.
- **Checked:** `selftest_026g.py` 733 checks with §3.17's (check 17: the six bytes; `start_match`'s use of the count and the live count's copy beside it; `deserialize` and `recalculate_scores`; the vtable's slots; every reader of +0x3240; no branch or address into the six bytes). `check_late_join_026g.py` runs the game's own `start_match`, `on_event` (through the thunk and slot +0x44) and `deserialize` (the reader intercepted, a kept state put back) on a made-up handler for rosters of 1, 5, 8 and 19 with a newcomer, and 0, 1 and 3 rollbacks over a tick in which a starting place and the newcomer score: the original gives the newcomer 13, 26 and 52 for 13 and leaves every later cell at the earlier match's value, the build gives 13 each time and a 0 in every cell; given the original for the build it fails. `check_revision6_026g.py`, the C tests and `late_join_026g.py --check-bin` as before.
- **Not run: a game.** Not seen: a scoreboard of revision 9 with a newcomer on it.

### 3.17 The team points during the countdown (found and written 2026-10-14, patch revision 9) — PROVEN in the code, the game's own code run with Unicorn, not run in a game

Seen on the players' server on 2026-10-14: under "Awaiting match start" the bars of the team points showed 63 and 53, the end of that player's match before (0xd881ba4, points [53, 63]); the new match's 150 and 150 came when play began. A player wrote that it was so before srp's patches too: it is the original game's code (0.21d's `update_ui` has the same branch).

- **`game_world_ui::update_ui`** (0x23B200; 0.21d 0x183502), when the hash of the hud's state (+0x24 to +0x2D8) changed: with the countdown's flag (+0x15C, the timelimit rule's, while the match waits or counts down) it shows the pregame (0x236AD0 with 1, 0x236B50 with the text) and jumps to the players' rows (0x23B328 → 0x23B421). Otherwise it shows the start's tip once (0x2375C0), hides the pregame (0x236AD0 with 0), shows the match time (0x236A00) and goes on at 0x23B383: it takes the current player (counted up, and down at 0x23B405), draws the team points (0x2364C0 → `root.set_game_progress`, the state's +0x90/+0x92 and the player's team), in Team Deathmatch the last players (0x236610 → `root.set_last_players`, +0x94/+0x95) and the respawn time (0x236C30 → `root.set_respawn_time`, +0x08).
- **The state has the match's own numbers during the countdown**: the rules fill it every tick, the countdown too: the tdm rule (0x316430) its tickets and alive counts, the gather rule (0x3140C0) its points, the respawn rule (0x316C20) the time to the local player's respawn or 0. Only the drawing waited for play.
- **The patch**: `PREGAME_PROGRESS` in `patch_026g.py`: the countdown's `jmp 0x23b421` (E9 F4 00 00 00) → `jmp 0x23b383` (E9 56 00 00 00), in the builds with the late join. The countdown is shown as before; then the three drawings play makes, without the match time and the tip.
- **What the three draw then.** The team points: the start's (150 and 150 in Team Deathmatch, 0 and 0 with batteries). The last players: the hud movie hides them while a team has tickets (`HUD.as` `set_game_progress`: `team1LastPlayers.visible = team1 == 0`); the counts the tdm rule has before its start event (the event recounts them, 0x3160D8 → 0x3161F0) are never seen. The respawn time: no player waits for one during the countdown (the saved worlds of match 0x57b21c79: the respawn rule's times are empty from 50 ms to the start at 21 750 ms), so 0; `set_local_respawn_time` keeps the last value it drew (0xACFC30), draws nothing for the same one and an empty text for 0. So nothing but the bars changes, and they show what the first refresh of play would.
- **The jump.** Both paths reach 0x23B383 with the stack as deep (0x236AD0 `ret`, 0x236B50 and 0x236A00 `ret 4`, 0x2375C0 `ret 0x10`) and with what the code from there reads alike (`edi` the hud, `ebp` the state's hash stored after the rows; `esi` and `ebx` set anew). The rel32 holds no address; no base relocation is under it, and no branch or 4-byte value aims inside it.
- **Checked:** `selftest_026g.py` check 18 (the five bytes; both paths of the original around them; the calls from the target to the rows and nothing leaving elsewhere; the calls' `ret`; the three movie functions; nothing aiming inside). `check_late_join_026g.py` runs the game's own instructions from the flag's test (0x23B307) to the rows (0x23B421) on a made-up hud, the functions taken at their entry, for modes 4, 3 and 2, with and without the tip shown: the countdown of the original calls the pregame's two alone, the build's the same and then the three with the very arguments play gives them; play is the same in both; the stack, the hash, the hud and the player's count end as they began; given the original for the build it fails.
- **Not run: a game.** Not seen: a countdown of revision 9.

### 3.18 The weapons' random state at a spawn (found and written 2026-10-14, patch revision 10) — PROVEN in the code, the game's own spawn run with Unicorn, not run in a game

Seen on the players' server on 2026-10-14 (match 0x1ce45cbe, seven players at the start and four taken in later): the newcomers went out of step 18 times, the starters never. Six of those were short, each at a moment another player respawned into a kit he had used before the newcomer's world was made; in the saved worlds the newcomer's world then differed from the others' in four bytes only, the random state of that kit's weapon. (The other twelve are the server's: inputs made while a resync world loads, not touched here.)

- **The state.** PROVEN: `weapon_core` (vtables 0xA11094 / item 0xA1114C; the client's `weapon` 0xA04174 / 0xA0422C, made over it by 0x243640) keeps a boost `minstd_rand` at +0x5BC: the constructor 0x2DF0D0 stores 1 (0x2DF429); the spread of a shot draws from it (`instant_fire` 0x2E23A0 → 0x2E1D50 and, per pellet, 0x2E2120; the normal distribution 0x2E4850 is made anew on the stack for each draw and keeps nothing; `uniform_01` 0x2AB6E0 and the uniform 0x2E49F0 do `x * 48271 mod 2^31-1`, `mov ecx,0BC8Fh` and `__aullrem` by 0x7FFFFFFF); `weapon_core::serialize` 0x2E3130 writes it after the item's header (f32 condition, u8 place, u8 kit, u8 slot), and `deserialize` 0x2E3490 reads it back (0x2E34C1). A register plus 0x5BC is in `.text` five times, those five (0x2DF429, 0x2E1DCC, 0x2E2195, 0x2E315D, 0x2E34C1; the other eight `BC 05 00 00` are on the stack or no operand), and the weapon's +0x5AC, which the item pointer plus 0x5AC would be, is the weapon's ammunition slots wherever it is used (read by hand: 0x245042, 0x2DF411, 0x2DF5D0, 0x2E0A8B, 0x2E0EB0, 0x2E266C, 0x2E26C0). So nothing sets it at a respawn.
- **What a world holds.** PROVEN: a place has a player object for each of its three kits (place entry at core+0x18458 + place × 0x30: three pointers, +0xC the current kit, +0xD the target); `game_world_core_client::serialize` 0x3218E0 writes the current and the target and then the current kit's player alone. So a game that loads a world during a match (a newcomer, a game that is resynced) has every other kit's weapons as they were in it before: at 1 for a newcomer. The same in any game for a rollback (0.26g loads a kept state whenever an input for a past tick comes): a kit that was not current in the state loaded keeps what the first run of the ticks after it left.
- **The spawn.** PROVEN: every respawn rule has the spawn 0x316DA0 at vtable slot +0x5C (0xA16014, 0xA16084, the client's 0xA06D1C, single player 0xA06D8C; the value is in the file at those four slots only, and no branch aims at it). The rules tick through the node the base constructor makes (0x316700: `mov dword [esi+120h],317D30h`; 0x317D30 = `jmp [eax+50h]`), from the world's tick 0x2D1E20 (the tick's time) called by `move` 0x2D2E40 for the first run and every replay. The player respawn rule's slot +0x50 (0x316820) spawns each place whose respawn time (rule+0x138, in the world: 0x316C90 / 0x316D50) is the tick's time, calling slot +0x5C with (place, spawn point, that time). A player who enters a match gets its respawn time from the core's entered handler (0x2D2010: each rule's slot +0x20, 0x316BC0 = `rule[place] := time`), so a match's first spawn is one of these too. All modes of the game's list use rule 91 (`player_respawn_rule`); rule 92 (`single_player_respawn_rule`, whose slot +0x50 0x267980 spawns through 0x2679D0 and not through the spawn) only the test mode `loopback_client`.
- **Inside the spawn** (PROVEN): 0x316DE3 makes the target the current kit; on a change of kit the rule's slot +0x60 (the client's view, 0x2678E0) and the statistics' slot +0x2C are told; 0x316E50 takes the new kit's player into `esi`; 0x316E70 calls its initialize (vtable+0x1C: `player` 0x1F2540 → `base_player` 0x2DAC00: transform, physics, animations, effects, damage model, then the inventory (player+0x10C) set up from the profile by 0x2FCB70, which writes the weapons' condition and a flag (+0x5F4) and makes no object, and a weapon taken in hand by `inventory::action` 0x2FCAA0) with the time in `ebp` (`mov ebp,[esp+20h]`, 0x316DC8, the third argument); on a protection time the damage model's timer; then at 0x316EA0 `mov edx,[esi]` / `mov eax,[edx+20h]` and the call that puts the player into the world (vtable+0x20, 0x2DA850: the tick lists, `+0x33F` = 1). `esi` and `ebp` are written nowhere between 0x316E50 and that call; after 0x316EA0 nothing reads a flag. A weapon can fire only from its fire state (0x30EBF0, the one caller of `instant_fire`) in the tick of a player in the world, so nothing draws from the new kit's weapons from the moment the kit is current to that call.
- **The weapons of a player.** PROVEN: the inventory's 23 slots are pointers at +0x10C (0x2FCB70's last loop: `lea esi,[edi+10Ch]`, `mov ebp,17h`); a slot holds the item part of its object, which for a weapon is weapon+0x10; the weapon slots are 7 and 10 (the table at 0x925F98 that 0x2FCB70 sets weapons up by; `inventory::action` with 7 or 10 in 0x2DAC00; two weapons a kit in the late join's ids). An item is a weapon exactly when its vtable+0x70 (the bind every inventory item has, which the inventory calls for its slots, 0x2FC7D0) is `weapon_core`'s bind 0x2E30F0: the value is in the file only at 0xA0429C and 0xA111BC, +0x70 of the item vtables of `weapon` and `weapon_core` (their RTTI names, both at offset 0x10 of their objects). The player's place and kit are its bytes +0x114 and +0x115, the two the item header of the world carries (`inventory_item::serialize` 0x2FFDF0 through item+0x250 → +0x174 → the holder).
- **The patch** (`WEAPON_RANDOM_HOOK` in `patch_026g.py`; `weapon_random_hook` at offset 3712 of `late_join_026g.asm`, 118 bytes in its free part, after `box_hook`): the five bytes at 0x316EA0 (file 0x3062A0), `8B 16 8B 42 20`, become `call 0xA426E80`. The code: `pushad`; the address of 0x2E30F0 taken relative to the code (call/pop: no absolute operand, no relocation); for each of the 23 slots of `[esi+10Ch]`, an item whose `[[item]+70h]` is that address gets at item+0x5AC (weapon+0x5BC) `h = ((place << 16 | kit << 8 | slot) * 0x85EBCA6B) ^ (time * 0x9E3779B1)` (mod 2^32), `state = (h ^ h >> 15) mod (2^31 - 2) + 1`, in 1..2^31-2 (0 would stay 0, 2^31-1 would become 0); `popad`; the two instructions; `ret`. It writes nothing else. `patch_026g.weapon_random_state` is the same formula. Only in the builds with the late join; `late_join_026g.section.bin` differs from revision 9's in those 118 bytes only (the C code after the assembly the same, byte for byte; built with the same gcc 13.2.0, GNU ld 2.41.90 and NASM 2.16.01; `--check-bin` current).
- **Why every game computes the same.** The place, the kit and the slot are the player's and its inventory's, the same objects in every game; the time is the place's respawn time, which is in the world, on the 50 ms grid, and the same in a replay. After any spawn the new kit's weapons have that state in every game, whatever they had before; a kit's weapons can be drawn from only while it is current, and a kit becomes current only by a spawn (0x316DE3), by its slot being bound (0x32182D, at the start and at a join, before its first spawn, weapons fresh from the constructor in every game) or by a world loaded (with its weapons' states from the world). So a game that loaded a world mid-match agrees from the next spawn of each place on, and a rollback over a respawn into another kit gives the same state again.
- **What else reads the state** (PROVEN): the world's hash (the current kit's weapons, now the same in every game); the match server's reading of a player's block skips the four bytes (`match_msgs/players.rs`, `MAGAZINE_AT`) and does not depend on them; the late-join code (`late_join_*.c`, `.asm`) never names +0x5BC (its newcomer's weapons come from the game's constructor in every game and are set at the newcomer's first spawn).
- **Revision 10.** `REVISION` is 10. A game of 10 and one of 9 or before part at a match's first spawn (10 puts a state that is not 1 into every weapon of every spawned kit), so **a server asks for 10** (`SRP_MIN_PATCH_REVISION=10` with the check on). The match server needs nothing: `match_msgs::clock::factor` takes 8 to 0x7F as fast (10 included), `late_join::spoken_by` takes 5 and later. A late-join exe built by hand (`0x80`) is let in by the check and has not the patch.
- **Builds** (with this section alone; with §3.19 too, the hashes of revision 10 are there): 12 907 128 bytes, sha256 `e02183a89db5e0eb3166a3f88a9cb9e229f4897dd653023e3e78cf07e91ff97f` (no server), `af1fa71d0e32835fba3e08e17c5e33dc2327c9e3507c931de03cf6fef1e48390` (`--server=skhid.link:1234`). Against the revision 9 exe of the same server (built from `main` 3b034dc: `03202d5e…20b7`, `f30c3c66…11a1`): the revision byte (file 0x36FC66), the five bytes (file 0x3062A0), 118 bytes of `.srp` (file 0xC4C880, the hook's code) and two bytes of the PE checksum, nothing else (compared byte by byte). `--no-late-join` still gives the revision 4 exe.
- **Checked:** `selftest_026g.py` 760 checks (check 19: the five bytes; the spawn's instructions around them and the registers; the rules' vtables and tick; the initialize and the inventory's slots; the constructor, the bind's two vtables and their RTTI names; every [register+5BCh] of `.text`; the hook's code: what it tests, the one store, the two instructions, no branch out; nothing aims inside the five bytes). `check_late_join_026g.py` (Unicorn 2.1.4) runs the game's own spawn 0x316DA0 on a made-up rule, world and place with three kits (weapons of both classes in slots 7 and 10, an ammunition item and a grenade item), its calls intercepted, for four places, kit changes, times and protection times, as two games (the new kit's weapons drawn from, and at 1): the build gives both games the states `weapon_random_state` names, after the initialize and before the player is put into the world, and changes nothing else (the other kits' weapons, the other items, the registers, the stack, the calls and their arguments); the game's own `uniform_01` then draws the same numbers in both; the original leaves them apart; the original given as the build, and copies of the build with slot 10 left out, the time left out or every item taken for a weapon, fail it. `check_revision6_026g.py`, the C tests (AddressSanitizer, UBSan), the packaging tests and `late_join_026g.py --check-bin` pass.
- **Not run: a game.** Not known: a match of revision 10 watched through respawns with a newcomer. Not covered: the single-player respawn rule (the test mode only). Other state of a kit that is not current may be stale in the same way after a world's load (nothing of it was found to differ in the saved worlds of match 0x1ce45cbe; not looked for further).

### 3.19 A first world's kit choices and inputs, in time order (found and written 2026-10-14, patch revision 10) — PROVEN in the code, the game's own code run with Unicorn, not run in a game

Reported by the analysis of the six matches after 0x1ce45cbe (players' server, 2026-10-14): five of fifteen late joins were wrong from their first tick and never let in; for two of them a newcomer's world with one player's next kit changed reproduced the newcomer's reported hash byte for byte (a CRC search over single bytes), LIKELY a third. That reproduction was not run again here; the cause below is read in the code and run with Unicorn.

- **The first world.** PROVEN: `game_world_core::deserialize` 0x2D0B70 reads the world (0x2D2770, which puts the world's next kit of each place, the chosen one, into place+0xD at 0x2D2A37 and handles the kit in use apart), and when the world is still frozen (+0x18FDF: a game's first world of a match: every game's at a match's start, a newcomer's, a game's that comes back; a resync's world is not one) clears the flag and, for each place of the world's mask, calls 0x2D4AB0 and 0x2D4B50 (at 0x2D1050 and 0x2D1055, the only calls of the two) with edi -> {core, the world's time, place}.
- **The two entries.** PROVEN: 0x2D4AB0 gives the place's input list (core+0x2FCC, 0x480 a place; a node: +0 the next newer, +4 the next older, +8/+0xC the value, +0x20 its time; the list: +0 count, +4 the oldest, +8 the newest, +0x10 the pool) an entry of the current player's input (+0x32C, +0x330) at the world's time, 0x2D4B50 the kit-choice list (core+0x89D8) an entry of **the kit in use** (`mov bl,[edx+115h]`). Both walk from the newest end to the first node at or before the time: one of the same time gets the value; otherwise a new node (0x1FCB60, the pool's allocator, which gives the oldest node back first when the pool is full) is linked **at the newest end**, whatever newer nodes there are. Every other writer keeps the lists in time order: the network's inserts (0x1FB41B for inputs, 0x1FBF00 for kit choices) walk from the oldest to the first node not older and put the new one before it through 0x2D4850 (the list's insert before a node, at the end for none); the game's own new entries (0x1FBDF1) come at the newest end (LIKELY always the newest; not needed here).
- **Who reads the kit choices.** PROVEN: each place's tick 0x321790 (its node in the world's first tick list, function set at 0x2D4D0B) walks from the newest end to the first entry at or before the tick's time and makes its value the place's next kit (place+0xD) when that kit is there (`< 3` and the kit's player not null), otherwise keeps it. So the first world's entry, put at the newest end with the kit in use, is the one every tick from the world's time on finds: a kit the world had chosen (a choice made before the world), and a choice relayed between the server's 0xE5 and the world for a later time, are both hidden by the kit in use; the newcomer respawned that player in another kit than the other games did, and was out of step from the respawn on (the world itself had the right next kit). Run with Unicorn: the original's block on such lists leaves `[(t-200, 2), (t+100, 1), (t, kit in use)]`, and every tick from t on sets the kit in use, where a game that played the match has 2 and then 1.
- **The patch** (`FIRST_LOAD_INPUT_HOOK`, `FIRST_LOAD_KIT_HOOK` in `patch_026g.py`; `first_load_input` at offset 3840 and `first_load_kit` at 3920 of `late_join_026g.asm`, 178 bytes with `entry_at_time`): the two calls (`E8 5B 3A 00 00` at 0x2D1050, file 0x2C0450, and `E8 F6 3A 00 00` at 0x2D1055, file 0x2C0455) go to the new code; no other byte of the original changes. The input entry is the same (the current player's input); the kit choice is place+0xD, the world's next kit. `entry_at_time` walks from the newest end: an entry of the same time is given the value, as before; otherwise a node from the game's own allocator, with the time, goes before the oldest entry that is newer, through the game's own 0x2D4850 (the place is looked for after the allocation, which may have taken the oldest node away; the network's insert re-checks for the same reason). With no newer entry the node goes at the newest end, as the game's own put it: the lists are then the original's but for the kit choice's value (run with Unicorn: the same lists when the next kit is the kit in use).
- **Why every game agrees, starters too.** The world is the same for every game that loads it, so is each place's next kit; the entries relayed by the network are the same in every game, and with every writer keeping the lists in time order a list ends the same whatever came first, the world or a relayed entry (in the original the order of the two decided where the first world's entry went). The tick works the next kit out of the lists: for a tick at or after the world's time the newest entry at or before it is the first world's entry (the world's next kit, which is what that tick leaves in the games that played: they had worked it out of the same entries up to that time, and the world was saved after its tick) or a relayed one, as in the other games. A starter's first world at a match's start is no other: before it no game of the match had played, all load the same world. **A change for games that load the same first world**: a kit chosen before a match's first world was reset to the kit in use by the original and is kept by the patch, in every game of revision 10 alike.
- **Inputs.** The value is unchanged; only the place of the node: an input relayed before the first world for a later time is no longer hidden by it for readers that look from the newest end (`discard_input` 0x27D2D0 and the network's own insert walk from the oldest; 0x2DC4B0, a reader of the next input, walks from the oldest and skips the misplaced node anyway). LIKELY no game was put out of step by the inputs' order alone (not looked for in the matches); the patch makes the list what every other writer keeps.
- **Revision 10**, with §3.18; nothing on the server.
- **Builds** (revision 10, §3.18 and this; with §3.20 too, the hashes of revision 10 are there): 12 907 128 bytes, sha256 `6689d3549a13f63a38d60a031df5aa4a4903ad811f291d4a35425ef8400f9923` (no server), `1656e51d96a6eb6c1f7da61bfa513f37db620f410c4eaf8a4b93bbcf49404efe` (`--server=skhid.link:1234`). Against the revision 9 exe of the same server: the revision byte, the PE checksum, §3.18's five bytes, seven bytes of the two calls' displacements, and 296 bytes of `.srp` (file 0xC4C880..0xC4C9BF: both sections' code), nothing else. `late_join_026g.section.bin`: 7758 bytes, sha256 `23063ded089b2e2f04699a0df34f5221992254f3fe0def23ecfb022c6b3fac34`, the C part byte for byte revision 9's. `--no-late-join` still gives the revision 4 exe.
- **Checked:** `selftest_026g.py` 787 checks (check 20: the two calls and that they were the only ones of 0x2D4AB0 and 0x2D4B50; deserialize's block of the first world and the read of the next kit; the two originals' kit in use and their own append; the tick's search from the newest end and the network's inserts in time order; the new code's reads, calls and stores; nothing aims inside the two calls). `check_late_join_026g.py` runs the block 0x2D0FF2..0x2D105E (the game's own instructions with the two calls, the allocator 0x1FCB60 with 0x2D48B0 and 0x5A540 on made-up pools, the insert 0x2D4850), then the places' own tick 0x321790 for eight ticks, for four places: a choice before the world and one relayed for later, empty lists, an entry of the world's very time, every entry newer, and the pool full (its oldest node given back): in the build every list is in time order with the world's entry at its time and the next kit after each tick is what a game that played has; in the original the case of the newcomer hides the later choice; the original and copies of the build with the kit in use read, with the node always at the newest end, or with an entry of the same time not found, fail it.
- **Not run: a game**, nor the CRC reproduction of the joins with the patched exe. Not known: whether the two other wrong joins of those matches had this cause.

### 3.20 A weapon's state that a respawn kept (found and written 2026-10-14, patch revision 10) — PROVEN in the code and in saved worlds, the game's own spawn run with Unicorn, not run in a game

Reported by the analysis of the evening matches of 2026-10-14 (players' server): §3.18 resets the random state alone, and most of the stale-kit desyncs of match 0x47d5fa35 (and some of 0x3c67147b and 0x4301ed65) were not of it. Every saved world of those matches that was rejected or unchecked was compared byte for byte with the checked one of the same tick (of the twenty matches of the evening, 0x47d5fa35 and 0x3c67147b have such pairs of this kind; for the others the hashes the games reported were compared with checked worlds changed by hand).

- **What differed** (PROVEN, bytes of saved worlds, mapped to fields by the serializers' own field names, which the exe passes to its writers):
  - a weapon's random state (`weapon_core`+5BCh, §3.18): every stale-kit pair;
  - `m_last_tick_time_in_ms` (`weapon_core`+5A4h, the u32 after the random state; writer `weapon_core::serialize` 0x2E3130, name at 0xA10F5C) of the kit's weapon in slot 10: -1 in the games that had loaded a world, the time it was last in hand in the others: 0x47d5fa35 place 4 kit 0 (-1 / 58700, worlds 564750..573450, six games out of step at 570400) and place 6 kit 1 (-1 / 364174, worlds 764800 and 769800, six at 771550); 0x3c67147b place 1 kit 0 (-1 / 73150, world 588400); 0x4301ed65 place 5 kit 0 (-1 / 81450 in the majority's worlds 739800 and 744800; its random states were 1 in every game, so §3.18 alone could not help; LIKELY, no rejected world of that tick);
  - a player who left and came back (0x47d5fa35 places 2 and 7, back in at 521250 and 525400; seven games out of step from 531900): 20 and 8 bytes of the current kit's player: `player_profile::serialize_dynamic` (0x2F7CC0) `amount_in_storage` of two and of one stack item (662 / 884 and 9 / 13; 53 / 76), the `m_condition` of three items of the kit (one the slot-7 weapon; 0.889 / 1.0 and the like), the slot-7 random state and the slot-10 last tick time (-1 / 312300). The profile is one per place, serialized with the current kit's player: a game that loaded a world while the player was away built that place's profile and objects anew from the static info (full storage, conditions 1.0), the games that had seen the player kept them as he left them. Not touched by this section (see below).
  - Nothing else of a player's block differed in any of these pairs; the other rejected worlds of the evening differ in many fields and in their animations (other causes).
- **The weapon's fields** (PROVEN in the code): `weapon_core::serialize` writes, after the item's header (`m_amount` or `m_condition`, place, kit, slot): the random state +5BCh, `m_last_tick_time_in_ms` +5A4h, `m_load_ammo_on_next_activate` +5F4h, `m_ammo_in_magazine` +5E5h, `m_fire_queue_type` +5E6h, `m_current_ammo_id` +5B1h, `m_is_round_chambered` +5F1h (with a chamber), `m_ammo_switch_time_in_ms` +5A8h and `m_next_ammo_id` +5B2h (two or more kinds of ammunition), then its state machine (+468h, 0x331FF0), +3F8h (0x3312F0), `m_aimed` +5E8h, the recoil (+5FCh, 0x32FEE0), the scope when aimed (+288h, 0x31DA30), +5ECh as a byte, `m_need_to_auto_reload` +5F5h, +320h (0x38CC90) and the object at +278h (its vtable+14h). What sets them again at a respawn:
  - the initialize (`player` 0x1F2540 → `base_player` 0x2DAC00) keeps the spawn's time at player+324h, sets the inventory up from the profile (0x2FCB70: each weapon's condition from the profile and `m_load_ammo_on_next_activate` = 1; the stacks' amounts), and activates the item it puts in hand (player+128h, vtable+14h; `weapon_core`'s 0x2E2A30): aim 0, recoil 0.0 and +60Ch -1, +5ECh 6, +5F5h 0, the ammunition item from +5B1h, the ammunition loaded (0x2E27C0: the magazine when empty, a round into the chamber), +5A4h = the spawn's time, +5A8h -1, +5B2h = +5B1h;
  - the removal before a spawn: the respawn tick takes the place's current player out of the world when it is in it (0x316987..0x3169A0, vtable+24h with 1: 0x2DA790), and so does a player's departure (0x316BE0): the weapon in hand is put away (0x2DA7ED) and the inventory goes back to the profile (0x2DA840 → 0x2FD2D0): each weapon's magazine and chambered round back into its ammunition stack, both then 0 (0x2FD370 → 0x2E18E0: 0x2E1936, 0x2E1941), the stacks back into the storage (0x2FD4B4), the conditions back into the profile. (The death handler of the slots, 0x2FCA50 → `weapon_core`'s item vtable+20h 0x2E3390, wears the condition and writes item+5E5h, which is weapon+5F5h, `m_need_to_auto_reload`, not the magazine.)
  - only the weapon in hand is ticked (vtable+28h, 0x2E09E0: `dt = time - [+5A4h]`, then `[+5A4h] = time`); the saved worlds show it: a kit's slot-10 weapon keeps the time of its last use while its owner plays with the slot-7 one.
  So a weapon not taken in hand at a spawn keeps +5A4h, and every weapon keeps the fire mode +5E6h (written by the constructor, the player's input 0x2E0CA0 and the deserialize alone) and the ammunition kind +5B1h (constructor, the tick at the end of a change of kind, the selection 0x2E0DE0 → 0x2E0E87 when the kind in use has no rounds, the kit's list 0x2E41D0, deserialize): what the kit's last life left in the games that played it, the constructor's in a game that loaded a world since. Both are in the world's bytes: the hash parts the games at the respawn. The magazine and the chamber are 0 after the removal above; the aim, the recoil, +5ECh, +5F5h and +5A8h of a weapon put away are LIKELY what the putting away (0x2DA7ED) leaves, not read further.
- **Who reads +5A4h** (PROVEN: every [register+5A4h] of `.text`, eleven): the constructor, the tick and the activation, the serialize and the deserialize, the aim on and off (0x2E1B50, 0x2E1BF0: the weapon in hand), the change of ammunition kind (0x2E2650, the player's input on the weapon in hand), the client's draw of the weapon (0x244E10, the client weapon's vtable+48h), and 0x26D028 (another class's +5A4h, read by hand). So a weapon not in hand is not affected by its value until it is activated, and the activation sets it first.
- **The patch** (`KIT_STATE_HOOK` in `patch_026g.py`; `kit_state_hook` at offset 3168 of `late_join_026g.asm`, 129 bytes in its free part, after `register_animations`): in the respawn rule's spawn (0x316DA0, §3.18) the five bytes at 0x316E57 (file 0x306257), `8B 16 8B 52 1C` (`mov edx,[esi]` / `mov edx,[edx+1Ch]`, the load of the initialize's call), become `call 0xA426C60`. There esi is the new kit's player, the spawn's own [esp+10h] the kit of the player that was current before and [esp+18h] the new one's (0x316E02, 0x316E06; the game's own test of a change of kit at 0x316E0A; both paths reach 0x316E53, which reads the spawn point at [esp+1Ch], with the same esp). For each item of the inventory's 23 slots whose vtable+70h is `weapon_core`'s bind 0x2E30F0 (as §3.18): +5A4h = -1 at every spawn; and at a spawn into another kit than the one before, the constructor's values (0x2DF0D0) of +5A8h (-1), +5B1h, +5B2h, +5E5h, +5E6h, +5E8h, +5F1h, +5F5h (0), +5ECh (6), +5FCh..+608h (0.0), +60Ch (-1) (`KIT_STATE_FIELDS`). Then the two instructions. Nothing else is written. The initialize then activates the weapon it puts in hand as it does a new one. `late_join_026g.section.bin` differs from §3.19's in those 129 bytes only (the C part byte for byte the same; `--check-bin` current).
- **Why every game ends alike.** The kit before and the new kit are the place's (in every world); at a change of kit the fields above are the constructor's in every game, whatever the kit's last life left; at a respawn into the same kit that kit's player was the current one, so it was in every world a game loaded and in every state a rollback loaded, and every game has it alike; +5A4h is -1 in every game at every spawn and the weapon in hand gets the spawn's time from its activation. A rollback over a spawn does the same again.
- **What changes for a player** (compared with the original), exactly two things, both only after a respawn **into another kit** (a respawn into the same kit keeps them; the original kept them in the games that had played that kit, a game that had loaded a world since had the first ones anyway):
  - the kit's fire mode is forgotten: its weapons are back at mode 0, the first of each weapon's own list from its config (0x30AC90..0x30ACEF; a shot reads the list at the mode, 0x30E946..0x30E94C; a single-shot-only or burst-only weapon has one entry). PROVEN;
  - the kit's ammunition kind is forgotten: id 0, the first of the weapon's list of this kit's ammunition slots that hold matching ammunition (built when the kit is made, 0x2C75B0 → 0x2E41D0 at 0x2C799A: always a kind the kit carries). PROVEN. Edge case: when the kit's rounds of kind 0 are used up (the place's storage of it is empty) and another kind still has rounds, the original kept the kind the player had switched to and loaded it at the spawn; now the weapon spawns on kind 0, empty, and goes to a kind with rounds (0x2E0DE0 → 0x2E0E87, the first kind with rounds) only at an input of the player with bit 20h or 40h of its actions (+32Ch+4 of the player). PROVEN that the switch waits for that input; which keys set those two bits is NOT PROVEN (LIKELY fire or reload).
  The magazine, the aim, the recoil, +5ECh, +5F5h, +5A8h and the chamber are reset too, as a safety: LIKELY no-ops (the removal before a spawn already leaves the magazine and the chamber 0; the rest LIKELY by the putting away). No round is lost or added and the server's count of ammunition does not change: the reset writes no stack, no storage and no profile. +5A4h of a weapon not in hand has no effect (above). The random state: §3.18.
- **Not covered** (open):
  - NOT PROVEN edge case: rounds in a magazine of a weapon with no ammunition item (+538h null). The unloading 0x2E18E0 returns at once for such a weapon and leaves the magazine; the reset at a change of kit would then drop those rounds. No way was found for a magazine to be loaded without an ammunition item (0x2E17D0 and 0x2E27C0 load from it).
  - **A player who left and came back** (above): the place's profile (the storage of stacks and the conditions kept in it) differs between the games that saw him leave and the games that loaded a world while he was away, and no world carries it. Fixing it needs a decision: every game resets the profile of a place that comes back to its static values (a returning player gets his storage back; the server's count of what he used would be short), or the world carries the profiles of the places that are away (a world format change for the server's parsers). This revision makes the rest of such a return agree only at his next spawn into another kit; the current kit's fire mode and ammunition kind of a returning player can still differ.
  - The other items' own state is not reset: artefacts (`artefact_base::serialize` 0x313670: `m_state`, `m_time_left_to_cool`, `m_container_id`, `m_time_left_to_spawn`, `m_last_activation_time_ms`; spring, lifebone, onyx: `m_time_left_to_deactivate`; onyx `m_damage_to_absorb`), medkits (0x33DBD0: `m_active`, `m_activity_time_ms`, `m_delay_ms`, `m_applied_influence`), oxygen tanks (0x33E790: `m_active`, `m_amount_ms`). Whether the initialize or the death sets them again was not read; none of them differed in the saved worlds of the evening (LIKELY rare, not proven absent). A weapon's state machine (+468h), +3F8h, +320h, the object at +278h and the scope (+288h, saved only when aimed) are not reset either; no saved world showed them apart.
  - The single-player respawn rule (the test mode `loopback_client`), as §3.18.
  - Two newcomers of 0x4301ed65 (from the world of 548400) reported the same wrong hash from 556000: no fresh-object value of a weapon (random state, last tick) of any place, alone or in pairs, nor any u32 of their own two blocks set to 0, 1, -1 or 1.0, gives it (CRC search on the world of 559300): not this cause, not found. 0x6968f246 at 234600: a game that came back had, for another player who came back, the full storage and conditions of 2.0 and 1.992 (one more than the others' 1.0 and 0.992): a back-in of the kind above with something more; not found.
- **Revision 10**, with §3.18 and §3.19; nothing on the server.
- **Builds** (revision 10, §3.18 to §3.20): 12 907 128 bytes, sha256 `c43bfd8a1f67369c394470848aa7610a19405122c306414992a47784c2def85e` (no server), `c2188027dfce70a4591d13912fea5560b4492de5a39bf394cb28d8d31ac2746b` (`--server=skhid.link:1234`). Against §3.19's build of the same server (`1656e51d…4efe`): the five bytes (file 0x306257), 129 bytes of `.srp` (file 0xC4C660, the hook) and two bytes of the PE checksum, nothing else (compared byte by byte). `late_join_026g.section.bin`: 7758 bytes, sha256 `8bc3be3bef1378f1be064a302ac333638c4e2099b9a4db265c399910b722928f`. `--no-late-join` still gives the revision 4 exe.
- **Checked:** `selftest_026g.py` 814 checks (check 21: the five bytes; the spawn's kit bytes, the test of a change, the two paths' esp; the constructor's values of the fields; the initialize's time and activation and what the activation sets; every [register+5A4h] of `.text`; the writers of the fire mode; the hook's code: what it reads and tests, its stores exactly `KIT_STATE_FIELDS` and +5A4h, no other write, the two instructions, no branch out; nothing aims inside the five bytes). `check_late_join_026g.py` (Unicorn 2.1.4) runs the game's own spawn 0x316DA0 with both hooks on made-up objects (three kits, weapons of both classes in slots 7 and 10) for five spawns (changes of kit and respawns into the same kit, with and without protection), as two games, one whose weapons are as a kit's last life leaves them and one with a new weapon's values (at a respawn into the same kit, both as the world has them): in the build, at the initialize, the new kit's weapons are alike in both games, every field as above, and nothing else of any weapon changes but the random state (the other kits' weapons not at all); the original leaves them apart; the original and five broken copies (slot 10 left out, the change read from the wrong place, the fire mode left out, every spawn taken for a change, the last tick left out) fail it. `check_revision6_026g.py`, the C tests (AddressSanitizer, UBSan), the packaging tests (44 on Linux, 19 on Windows) and `late_join_026g.py --check-bin` pass.
- **Not run: a game.**

### 3.21 Team Deathmatch's rule counts a death after its own end (found and written 2026-10-15, patch revision 11) — PROVEN: the handler run outside the game, the live signature made again; not run in a game

- **Why.** match.md §28: in two live matches a team ran out of tickets and its players died one by one, and one dead player of that team stayed in the rule's alive mask for the rest of the match; the server saw a player alive and the match ran to the time limit (930 s and 925 s). The same mechanism as §3.9: the death handler (`tdm_rule_core` event 1, 0x316090) returns at once when the rule's completion time (+0x12C) is not -1; the end test 0x316470 sets it (now + 3000) when a team has no tickets and no player alive; the constructor (0x31601D) is the only other writer and the time is not serialized. A rollback over the death that completed the rule loads the tickets and the masks and not the time, and the replay of that death is skipped: the bit is back and stays, and so do the tickets.
- **Where.** 0x3160F1 `0F 85 E6 00 00 00` (`jnz 3161DDh`, after `cmp dword [esi+12Ch],-1`) → six `90`. The jump of the other event (0x3160E4, "not event 1") stays.
- **What is left:** the handler counts every death: the ticket, the alive mask, the respawn flag, the recount and the end test, which sets the time again, later, at every death after the first end (the end test also writes it into the field +32Ch of every player in the alive mask, 0x316500, and 0 into +330h; the respawn rule writes the spawn time plus the spawn point's +20h there, 0x316E90, when that is not 0; the player's constructor sets -1 and 0, 0x2E5303, and its load reads both from the world, 0x2E54D3: a serialized field, LIKELY the protection after a spawn, so the end protects who is still alive until the completion time; what reads it was not traced). Nothing reads the time but the rule (`rule_completed` 0x316520, the end test and the guard; the world's save copies `rule_completed` into a history byte nothing reads, modes_maps.md §11); the match's end is the server's, which sets it 3 s after the first snapshot with a team out.
- **What changes in the 3 s after a real end** (checked 2026-10-15 for the merge): a death there now costs its ticket, clears its bit and runs the end test again, which moves the completion time and the survivors' +32Ch to that death + 3 s. A ticket never goes below 0 (the handler takes one only from a team that has one, 0x31612C). The one case where the result can change: both teams at 0 tickets, one team out, and the other team's last alive players also die in those 3 s: the original's server counted that team the winner (by alive players, 0x316301), revision 11 and the server count a draw (`winner_tdm`). With the survivors protected by +32Ch (LIKELY) such a death should be rare; not seen in the data. Left as it was in 10 and before: a completion time from a world a rollback threw away in which a team was out, but is not out in the replay, stays set (no serialization takes it back); in 10 that client then skips every later death, in 11 it counts them and only writes that stale time into the survivors' +32Ch at each death (a world difference at each such death, which a resync puts right, not a stuck match). Such a rollback was not seen.
- **Checked** (`selftest_026g.py` check 22): the six bytes are the original's in the build without the late join and nops in the default build; the instructions around them; the handler's only jumps to its return are the two named; no relocation under the six bytes and no branch into them; the rule's code touches +12Ch at seven places (the constructor, the guard, the end test's four, `rule_completed`).
- **Run outside the game** (2026-10-15, `client-patches/run_tdm_rule.ps1`, 32-bit PowerShell on Windows 11): the code of 0x316000..0x317000 copied to the same addresses of a process and the handler called on a made-up rule (two teams of two, 5 and 1 tickets), the flag setter 0x316630 replaced by `ret 8`. A death of team 1's last ticket, a second death of the same player (out of the alive mask), the death of its last player: tickets 5:0, alive 0011, time set. A rollback loads the tickets and the mask (1011) and not the time. The same death again: **original** (and the build without the late join): alive 1011, time unchanged, and a later death of team 0 is not counted (tickets 5:0 as before): the live signature of 0x1f07c8fb and 0x7ce977cb. **Revision 11**: alive 0011 again, time set again, and the later death counted (tickets 4:0). This is the handler with its recount and end test, not the game: the rollback itself, the respawn rule and a server reading the world were not run.
- **Revision 11.** `REVISION` is 11. A game of 11 and one of 10 or before part at the first death after a team is out (the older one does not count it): a server asks for 11 (`SRP_MIN_PATCH_REVISION=11`), put in the same change as the ZIP. Before that match.md §28's fix 1 (a server that reads the team as out from its blocks) works with every revision; both together are harmless, the check then never fires. A build without the late join (`--no-late-join`, `--cave=hash-mismatch`) keeps the original handler and says 4.
- **Builds** (revision 11 before §3.22, §3.18 to §3.21): 12 907 128 bytes, sha256 `985ca3410f024a9e391a8f6fd5750295d6c0add8185fc244245c15371400b75a` (no server), `b728a80c62a8da6b0dca46de55236d1e7c4b010190f9c0fddf884e0c998d6ece` (`--server=skhid.link:1234`). Against revision 10 made the same way (`c43bfd8a…ef85e`) the exe differs in the six bytes (file 0x3054F1), the revision byte (file 0x36FC66, `0A` → `0B`) and the PE checksum, and in nothing else (compared byte by byte).
- **Not run: a game.** Not known: whether a rollback crosses the deciding death in every match (the live data say it did in both that ran a team out), and what the field +32Ch does in the 3 s after an end (not traced beyond its writers).

### 3.22 A field-of-view setting (found and written 2026-10-15, patch revision 11) — PROVEN in the code, the projection run with Unicorn, run in a game on an earlier revision

Asked for by players: 0.26g has no field-of-view setting, the camera is fixed at 65 degrees (vertical).

- **The option the game already has.** PROVEN: the video options' constructor (0x262929) makes option 7, a float slider (vtable 0xA06728) of snap 1, labelled `st_fov_option` (the label key at 0x9FD11C, offset +214h of the options; "FOV", "Sichtfeld", "Поле зрения", "Поле зору" in the five localizations), and names its console variable by `mov eax,offset "fov"` at 0x262930 ("fov" at 0xA0659C, pointed to from there alone). The command lookup (0xEB7C0) finds no "fov", so the option is dead: no range and no value. `Settings.as` of `main_menu.swf` maps index 7 of the video tab to `_slider_fov` and settingsTabs_15 declares `_slider_fov`, but no frame places it.
- **uro_fov.** PROVEN: a float console variable (object 0xAA6510, vtable 0x9E7C6C; name "uro_fov" at 0xA281D0 through +10h; value 0xA6FA28, 65.0 in .data; least +44h, most +48h), registered at 0x85ADF0 with the range 0 (`xorps xmm0,xmm0`) to the float 100 at 0xA710D0, saved in `user.cfg`. Its value is pointed to by the registration alone (0x85AE2A): nothing read it.
- **The projection.** PROVEN: 0x242900 is slot 0 of five vtables, by RTTI those of `player_input_handler` (0xA0406C), `game_camera` (0xA04084), `free_fly_camera` (0xA047F0), `demo_camera` (0xA0482C) and `lobby_camera` (0xA09ED8), and pointed to from nowhere else. It pushes far (this+8Ch), near (+88h) and the viewport's aspect, loads the zoom (+90h; 1, a scope's factor while aimed) times 1.134464 (the float at 0xA2D9EC, 65 degrees in radians, read there alone) into xmm0, calls the matrix builder 0xEEE30 (which takes the tangent of half the angle) and returns `ret 8` with the matrix in eax.
- **The float setter.** PROVEN: the setter of every float console variable (0xECB70, vtable 0x9E7C6C, 63 variables) stores a value only when it reads as a float within the least and the most; otherwise it prints its error (0xEC530) and stores the least. So `sensitivity 20` gives 0.01, `s_general_volume 150` gives 0, and a wrong `uro_fov` gives its least. The ranges of all 63 were read by running their registrations with Unicorn. The menu's sliders send values within their range only.
- **The zoom** (PROVEN, 0x1F3480, at every frame): camera (player+F470h)+94h, the zoom at +90h of its camera part, = `1 + (target − 1) × p`, p = the weapon's aim progress (+5FCh, 0..1), target = the fitted scope's `fov_factor` (scope+114h, read from the scope's config at 0x258845) or else the weapon's +5B4h, which the weapon's cook sets to 1 / `aim_zoom_factor` (0x30AB61..0x30AB94); 1 when nothing is aimed. The same function sets the near plane from scope+118h or weapon+5B8h. The configs: `fov_factor` 0.25 in all three scopes (PSO-1, Giperon, Leupold), `aim_zoom_factor` 1.0 (13 weapons), 1.1 (25) or 1.2 (12). 0.10b had the same design: `weapon::instant_aim_start` set the player's target zoom to the scope's `fov_factor` or the weapon's own, `instant_aim_end` to 1, slid linearly over the aim time, and its projection took `cam_fov` (a console variable over `default_vertical_fov`, 60 to 70) × π/180 × the zoom; 0.26g made the angle a constant.
- **The patch** (`FOV_PATCHES` and `FOV_PROJECTION_HOOK` in `patch_026g.py`, only in the builds with the late join): `mov eax,0xA281D0` at 0x262930 (the relocation at 0x262931 stays: an address for an address); `mov dword [0xAA6554],50.0; nop` for `xorps; movss [0xAA6554],xmm0` at 0x85AE06 (xmm0 is loaded with the most right after; the only branch in, `je 0x85ae06`, lands at its start), the relocation at 0x85AE0D disabled; at 0 the builder would take tan 0. The projection's first instruction (`fld dword [ecx+8Ch]`, 6 bytes at 0x242900, file 0x231D00) becomes `jmp 0xA426240` + nop, to `fov_projection` (offset 576 of `late_join_026g.asm`, 126 bytes in its free part after `query_players`), which pushes far, near and the aspect as the original, computes the angle `65·z + (uro_fov − 65) · max(0, (z − 0.25) / 0.75)` degrees in x87 (`fcomi`/`fcmovb` for the max; the constants 0.25, 4/3, 65 and π/180 after its code), puts it in radians into xmm0, calls 0xEEE30 and returns `ret 8` with the matrix in eax. The rest of the original's body stays, never reached, with the constant's relocation.
- **Why that formula:** z = 1 gives uro_fov; a full scope (z = 0.25) gives 65 × 0.25 = 16.25 degrees, the original's, whatever the setting; iron sights (z = 0.909 or 0.833) nearly uro_fov (at 100: 89.9 and 88.9 degrees for 90.9 and 83.3); between, the angle falls monotonically as the zoom goes in (its slope in z is 65 + (uro_fov − 65)/0.75 ≥ 45); through a scope it is `65·z + (uro_fov − 65)(1 − p)`, the setting's part fading with the aim. At 65 every zoom is the original's. A zoom below 0.25 (none in the configs) would get the original's angle.
- **Revision 11**: a camera is not in what a match computes and nothing else read the constant; two games of 11 at different FOVs played one match (below). Revision 11 as a whole does not play with 10 (§3.21), and a server asks for 11.
- **What follows from it for a player:** the lobby, spectator and replay cameras follow the setting.
- **Builds** (revision 11 with §3.21): 12 907 128 bytes, sha256 `3943fe6ebc2c3dc3788d3533401766fc7e79c856d235d520ce8e93ed54de1146` (no server), `8aea6cb41e3df47c6568ea51f593fc0bd76410285c3367cb13aa5db4aa0e3d43` (`--server=skhid.link:1234`). Against §3.21's build of the same server: the six bytes of the hook (file 0x231D00), three bytes of the option's address (file 0x251D31), eleven bytes of the least (file 0x84A206), two bytes of the PE checksum, one byte of the disabled relocation entry and 126 bytes of `.srp` (file 0xC4BC40, `fov_projection`), nothing else (compared byte by byte). `late_join_026g.section.bin`: 7758 bytes, sha256 `8857b5c2b74b5831d7fd833906054d7490e4479927177e850ab253e0ff13c310`, §3.20's but for those 126 bytes (the C part byte for byte the same; the assembly assembled with NASM 3.02, which gives §3.20's 4096 bytes from §3.20's source). `--no-late-join` still gives the revision 4 exe.
- **Checked:** `selftest_026g.py` check 23 (the two sites in place and the hook; "fov" and "uro_fov" and what points to them; `fov_projection`'s read of uro_fov, blend, constants, call, XMM registers and return; the original's body after the hook, the constant's only reader; the five vtables by RTTI; the least and the most; nothing aims inside the sites in place), the hook with the other late-join hooks (check 15) and the relocation check (the table differs from the build without the late join in the two disabled entries, `scene::remove_model`'s and the least's). On Linux with NASM 3.02 the self-test fails the same 70 disassembly-text checks before and after (NASM 2.16.01 spells some instructions otherwise; Windows' checks 7 and 8 and check 21 skipped there). `check_fov_026g.py` (Unicorn) runs the original and the revision 11 projection, 0xEEE30 included, at 50, 65, 74.5, 90 and 100 degrees, zoom 1, 1/1.1, 1/1.2, 0.625, 0.325 and 0.25, three viewports: the matrices are the formula's (the original with the matching constant, a few ulp), a full scope is the original's at every setting, and 65 is the original's at every zoom. Run beside it: every general register and XMM1..7 on return are the original's, the x87 stack balanced, at the precision controls 0x37F and 0x07F. `check_late_join_026g.py` and `check_revision6_026g.py` pass.
- **Run in a game** (2026-10-15, a local server and `srp-bot`, an earlier form of the patch, the zoom multiplied by uro_fov, on revision 2, with `fov_slider_026g.py`'s slider): the lobby and a match, the view following the slider from 50 to 98, the value saved to `user.cfg`. **Run with this revision's exe** (2026-10-15, a local server built from this source, Wine): two games in one match, one at 100 with an SVD and a PSO-1, one at 50 with an SKS; the scope and the iron sights as the formula says, each game started with the setting saved in its `user.cfg`. **Not run:** Windows; the Giperon and the Leupold (the PSO-1's `fov_factor`); a game of 10 with one of 11.

### 3.23 A battery under the floor (found and written 2026-10-15, patch revision 11) — PROVEN in the code, the scan run with Unicorn, a test build run in a game

A battery dropped by a dying carrier can end up under the floor, out of reach for the rest of the match.

- **The scan.** PROVEN: `base_player::detect_usable_object` (0x2D84C0; called at a use's start 0x2D8900, every tick of a use 0x2D86AD, for the prompt 0x246FCD) casts one ray from the eye (player+47Ch) along the view (+46Ch), `usable_objects_detection_distance` long (2.0 at 0xA710D8), through the collision world's `ray_query` (vtable+4Ch: every hit, sorted nearest first; groups 0x114, mask 0x8A). It ORs each hit's collision group (`get_collision_group`, vtable slot 0; the 0.10b enum: walkable 2, hittable 8, usable 80h); a usable object is the result if its `can_use` (vtable+30h) says yes, and at any other hit it stops once walkable and hittable were both seen (0x2D861C `and al,0Ah; cmp al,0Ah; jz`).
- **The use.** PROVEN: at the start the hit's point is stored (player+148h); every tick 0x300830 (vtable+38h) checks the player is alive and the eye within the native distance of that point.
- **A battery.** PROVEN: `victory_item_core::can_use` (0x300710) is at +30h of two vtables only, the usable-object vtables of `victory_item_core` (0xA13E8C) and `victory_item` (0xA0437C). Their +44h, `get_transform` (`usable_object`'s is 0x2F8440), is the battery's own (0x246ED0: `sub ecx,14h; jmp 0x300A20`): the carrier's transform, the container's, or its own.
- **The patch** (`PICKUP_*` in `patch_026g.py`, `pickup_below_026g.asm` at `.srp`+800h, only in the builds with the late join): the frame grows by 8 for a flag (0x2D84C9); `call reach` at 0x2D84E9 doubles the ray (relocation 0x2D84EB disabled); `solid` at 0x2D861C sets the flag and goes on where the original stops; `candidate` at 0x2D85BE takes a usable object beyond the native distance or the flag only if it is a battery whose centre (vtable+44h) is below the feet (player+43Ch) and at most 1 m from them across, and makes the feet its point; the original otherwise. The 4 m ray from the eye reaches about 2.3 m below the feet, through any number of floors (kept so on purpose).
- **Revision 11.** A use is in what a match computes: a game with it takes a battery an older game leaves. Patch 21 already makes a server ask for 11.
- **Builds** (revision 11 with §3.21 and §3.22): sha256 `bf289387d1dedf6bf1fd2d8bd41d3d190d1e8bdeae54fa0d6b3e7e882b9842c6` (no server), `6b752ed0a23c03f4eb82111c955fe4bed61f7c087c40f6d78de4c686ecf0c46a` (`--server=skhid.link:1234`). Against §3.22's build: the frame byte, the three hooks, one relocation entry, the PE checksum and the code at `.srp`+800h (compared byte by byte).
- **Checked:** `selftest_026g.py` check 24 and the late join's byte and relocation checks; `check_pickup_below_026g.py` (31 scans of made-up hits, the original and the build).
- **Run in a game** (2026-10-15, two games on Keln, a test build with the same rule that called 0x2F8440 directly instead of vtable+44h): a battery stuck under the floor was picked up. **Not run:** this build; Windows. Not known: which hits the real collision world returns for a battery inside geometry.

### 3.24 Replay fixes: the camera after a kit change, a full sound queue, sounds after a seek (found and written 2026-10-15, in patch revision 11) — PROVEN in the code; all three run in a game (the seek also with Unicorn)

Both came up while watching `.sur` files made from match recordings (`replay_sur.md`). None changes what a match computes, so none has a revision number of its own (owner, 2026-10-15): they are in 11. A build without the late join keeps the original's bytes for both.

- **The camera follows its player into another kit** (`REPLAY_REATTACH_THUNK`, `REPLAY_REATTACH_SLOT`). replay_network_client's "is this place mine" (vtable 0x9FF344, slot +0x54 at 0x9FF398) is 0x54E10, `xor al,al` / `ret 4`. The client respawn rule's kit change (0x2678E0) asks it and re-attaches the camera to the place's new kit object when it says yes, as the live client's 0x27B550 does for its own place. The replay never did: it kept the old kit's object as the one watched, and the new kit's weapon got a HUD of 0 + 0x2E4 in `weapon::draw` (0x24502B), an access violation at the watched player's first respawn into another kit, in first or third person. The slot now points at a thunk in the seven int3 after is_player_current 0x242A40 (`mov eax,ecx` / `jmp 242A40h`), so "mine" is "the place the camera is on". The slot keeps its base relocation. The slot's only other caller, 0x1F2990, ends in a `ret 4` that does nothing. **Run**: a replay of a 10-player match followed one player in first person past that player's kit change at 748.5 s, where the exe without it died.
- **A weapon with a full sound queue drops the new sound** (`SOUND_QUEUE_HOOK`, `SOUND_QUEUE_CAVE_SITE`). Each weapon has a queue of delayed sounds, `delayed_callbacks_queue<64>` at weapon+0x9FC (begin, end, capacity end; 0x38 bytes an item). Only the weapon's destructor empties it. Each sound id keeps one item for good, plus a chain of at most 2 or 4 waiting ones. `add` (0x2454A0) drains what is due by the game clock, then pushes without looking; `push_back` (0x245AD0) asserts at 64 (`int3` at 0x245B32). A seek in a replay moves the game clock and leaves the queue as it was. A replay died that way about 50 s after a seek, in the simulation tick (an animation's sound event, `weapon_sound_effect::on_sound_event` 0x2981A0). What filled the queue in those 50 s is not proven: the dump holds no heap.
  - Where: at 0x2454C1, after the drain, `8B 06 8B 4E 04` (`mov eax,[esi]` / `mov ecx,[esi+4]`, esi = the queue) becomes `jmp 835A20h`. At 0x835A20, in the 42 int3 between a `ret` (0x835A15) and the next function, 20 bytes: `mov ecx,[esi+4]` / `cmp ecx,[esi+8]` / `jae` to `jmp 24569Ah`, else `mov eax,[esi]` / `jmp 2454C6h`.
  - Why 0x24569A is safe: it is the game's own exit for a sound past its chain's cap, in the same frame. Nothing between 0x2454C6 and its branch at 0x2454F1 pushes, pops or moves esp. It calls the sound's functor with flag 0, which the sound player 0x297E20 takes as "do not play", and returns with `ret 18h`. The queue is not touched, so no chain pointer goes stale.
  - Effect: while a weapon's queue is full, its new delayed sounds are dropped until it drains, where the game used to die. This is in live games too, where a full queue also killed the game.
  - It also changes live games, but only where a full queue used to kill the game. The owner accepted this (2026-10-15).
- **A replay's seek empties the weapons' sound queues** (`SEEK_CLEAR_HOOK`, `seek_clear`, `replay_seek_026g.asm` at `.srp`+A00h after the pickup code). The seek (0x1EE2E0, esi = game, edi = the target) loads a key frame, sets the clock, re-simulates to the target and sets the clock to the target. The queued sounds kept the old timeline's times, so a sound waited behind them, late by the seek's distance (reloads were heard late), and the re-simulation's sounds all came due at once.
  - Where: for a user seek only (`test bl,bl` / `jnz`: a key frame reached in play takes the jump), `lea eax,[edi-3E8h]` at 0x1EE3A3 becomes a jump to `seek_clear`. It walks every place's three kit players and their 23 inventory slots. For every client weapon (item vtable 0xA0422C, written only by its constructor and destructor), it does what `~weapon` 0x243790 does: destroys the functors (0x245C50, which calls only a functor's manager with "destroy" and plays nothing), sets the queue's end to its begin, and empties the fx history (weapon+828h, the duplicate check). Then it runs the lea and goes back.
  - Replay only: the seek starts from the replay reader `[game+53C8h]`, which only `-match_replay` sets. Its only callers are the replay's seek functor 0x1EF8C0 and the replay movie's slider 0x233E2F. A live game never runs it.
  - Live clock corrections (0x27B5D7) are left alone: clearing there would drop real pending sounds, such as the rest of a reload.
  - Not covered: a weapon in no kit's inventory slot (none found, LIKELY none exist).
  - Run with Unicorn (`check_late_join_026g.py`, `seek_clear`): the seek's end ran on made-up places, kits and weapons. On a user seek every client weapon is emptied, its managed functor is destroyed once and nothing is invoked; items of another class, a key frame in play and the original leave everything as it was.
- **Checked** (`selftest_026g.py`, `test_replay_reattach` and `test_sound_queue`):
  - the bytes of all three sites in the original, in the build without the late join and in this one;
  - the instructions around them;
  - that no rel8/rel32 branch or 4-byte value aims inside the hook or into either padding;
  - no base relocation under the new bytes;
  - push_back's full test uses the same two fields as the cave;
  - `test_seek_clear`: the seek's site and steps, its two callers, the vtable's two writers, the destructor's clear, and the cave's start and end.

### 3.25 The sign-in reports a stale shell_32.dll in the temporary folder (written 2026-10-15 as patch revision 13, never released; in revision 12 since 2026-10-16) — PROVEN bytes, run with Unicorn, not run in a game

One known cheat, "Survarium 0.26e ESP NAT" (`surv_26e_esp_hack_v21`,
`cmtme.exe`, 2014), is an external wallhack: it reads the game's memory from
its own process and draws on a window of its own over the game's. It touches
the game once — it writes a DLL to `%TEMP%\shell_32.dll` and loads it into
the game, where the DLL unloads `bugtrap.dll` and then itself (`ofstream`
`out|binary`, 10 240 bytes, sha256 `a7f7593f…b817`, carved from the exe's
`.data` at 0x40A020). As shipped it does not start against 0.26g: it looks
for the 0.26e window title and its memory offsets are 0.26e's, both in
`cmtme.exe`. A port would change those. Players can patch any client check
out, so this is not proof; it is a first count, to learn whether the cheat
is here at all before deciding whether to worry about it. Only the one file
is checked, because the cheat leaves it behind and never removes it, so it
also catches a PC where the cheat ran in an earlier session.

- **The site** (`STALE_DLL`, full builds only, `stale_dll_026g.asm` at
  `.srp`+E00h): `sign_in_on_connected` finishes the 12-byte version field
  with `mov [edi+8],eax` / `mov [edi+0Ch],dl` at 0x380889 (bytes 8..11 are 0,
  eax being 0; byte 12 is the EULA flag). These two become `call` the cave
  and a `nop`. The cave saves every register (`pushad`), asks Windows whether
  `%TEMP%\shell_32.dll` exists (`GetTempPathA` through `GetProcAddress`,
  then `CreateFileA` with `OPEN_EXISTING` and no access, then `CloseHandle`;
  `kernel32.dll` and the two imports `CreateFileA`/`CloseHandle` are the
  exe's own), restores them (`popad`), and does the two writes it replaced
  with the flags in byte 8: `0x80` the check ran, `0x80 | 0x01` it ran and
  the file exists. Every register but eax is as the game had it; the image
  may be rebased (call/pop-relative addressing, like the other caves).
- **The field**: the text still ends at byte 6, so a server that reads only
  the text (`parse_sign_in`) is unaffected, and the patch revision in byte 7
  (§6) is unchanged. Byte 8 was 0 in every earlier exe.
- **The server** (`login-server/src/lib.rs` `parse_sign_in`, `v026g.rs`):
  reads byte 8 as `cheat_flags`, next to the patch revision, and logs it in
  the sign-in line; `0x01` also logs a warning line with the account and
  address. Nothing else is done with it (the owner: only count at first). A
  shadowban, if wanted, should be delayed, so a caught player cannot tell
  which sign-in flagged them.
- **Nothing a match computes changes**: revisions 11 and 12 play together.
  The two writes do what the originals did; the stack and every kept register
  are restored; the field's length is unchanged.
- **Run with Unicorn** (`check_stale_dll_026g.py`): the hook and its code run
  on the built exe, loaded as it is and again 1 MiB higher (a rebased image),
  with the Win32 functions stubbed. Byte 8 is `0x80` on a clean machine and
  with the file elsewhere or `GetTempPathA` missing, `0x81` with
  `%TEMP%\shell_32.dll`, including a 258-letter temporary folder; a folder
  too long for the name gives `0x80`. Every register but eax is kept, the
  stack is restored, and every handle opened is closed.
- **Checked** (`selftest_026g.py` check 26, `test_stale_dll`): the two
  instructions become the call and a nop (the build without the late join
  keeps them); no relocation under them and no branch into them; the cave is
  `stale_dll_026g.section.bin`, sits after the seek code and before the
  late join without overlap, saves and restores every register, calls only
  itself, the four imports and `GetTempPathA`, branches only inside itself,
  and names only `kernel32.dll`, `GetTempPathA` and `shell_32.dll`. Check 11
  works out byte 8 as the check's.
- **What it is not**: proof, and not a defence. A ported cheat that skips the
  `%TEMP%` DLL, or a player who deletes the file, leaves nothing to count.

### 3.26 The transparent effects scroll their textures as in 0.29 (written 2026-10-16, patch revision 13, README patch 28) — PROVEN in the code, the game's compile run with Unicorn, not run in a game

London's sand and fire (0.29a3 content) are forward_default materials with
`use_uv_scrolling`; 0.26g's forward effect ignores it. The research behind
it: `scratch-london/engine-work-research.md` sections 1 to 3 (not in this
repository); everything below was read again in 0.26g's code for the patch.

- **What 0.29 does** [V, 0.29a3 decoded with `unbcj.py`, compile 0x7F2600]:
  unless `use_sequence` is on it reads `use_uv_scrolling` into bit 6 of the
  shader configuration's byte 0Ch (0x7F286C..0x7F28B1); after the uv
  distortion's constants, with that bit set, it sends
  `uv_scrolling_parameters0 = (0, 0, transparency_map_scroll_u_direction,
  transparency_map_scroll_v_direction)` and `uv_scrolling_parameters1 =
  (emissive_map_scroll_u_direction, emissive_map_scroll_v_direction, 0, 0)`,
  a value the material lacks being 0 (0x7F2CD0..0x7F2E34). The key names end
  in `_direction`; 0.26g has the two emissive ones (its g_stage reads them)
  and not the two transparency ones, which the patch carries.
- **What 0.26g has** [V]: g_stage's compile sets the same bit from the same
  switch (0x1DE4F1..0x1DE533; its configuration at [esp+18h]) and sends the
  same two constants with 0x1A8900 (0x1E0815, `ret 8`, ecx = &float4, then
  the compiler and the name). `fill_shader_configuration_macros` 0x3C9B80
  makes `CONFIG_USE_UV_SCROLLING` of byte 0Ch bit 6 (0x3C9F51, the stack
  followed from the function's start: its three macro helpers are cdecl).
  forward_default's compile (0x1D8F40) keeps its configuration at [esp+50h]
  (zeroed at 0x1D8F90..0x1D8FA3, passed with `lea edx,[esp+58h]` after two
  pushes at 0x1D9100); byte 0Ch is written nowhere else in it; `use_sequence`
  is byte 4 bit 4 (0x1D9075).
- **The keys and the masks** [V in data]: the shader key has a slot per macro
  that `shaders/masks` lists for the shader, `_` otherwise. The retail
  masks list `CONFIG_USE_UV_SCROLLING` for `gbuffer_pass` and not for
  `forward_base` or `vertex_base_z_shifted`, and all 495 retail
  `forward_base.ps` keys have `_` in that slot (`scratch-london/engine-research`
  `masks26.txt`, `sh26.txt`). So with the retail data the bit picks the same
  files. A binding the shader's constant table has no entry for is skipped
  (`shader_constant_table::apply_bindings` 0x3D7F40: no match, next).
- **The sites** (`FORWARD_SCROLL_HOOKS`, full builds only,
  `forward_scroll_026g.asm` at `.srp`+B00h, 540 bytes: after the seek code,
  which ends at +A97h, and before the shell_32.dll check at +E00h):
  - 0x1D90DC `8A 54 24 56 8B 75 10` (`mov dl,[esp+56h]` / `mov esi,[ebp+10h]`)
    becomes `jmp 0A425B00h` and two nops. Both ends of the angle
    attenuation's read meet there (0x1D90D8, 0x1D90DA; al the switch); esi
    and edi are written again before they are read; the flags are not read
    before `and eax,ecx` (0x1D90FD). The entry: `use_sequence` on, nothing;
    else pushad, `use_uv_scrolling` through the game's key check 0xF6C00
    (eax the node, esi the name) and lookup 0xF6B10 (eax the node, edi the
    name; it assumes the key, so the check comes first), a non-zero value
    sets the bit; popad, the two instructions, back to 0x1D90E3.
  - 0x1D94CB `8A 4C 24 57 80 E1 03` (`mov cl,[esp+57h]` / `and cl,3`)
    becomes `jmp 0A425B40h` and two nops. The uv distortion's block ends
    there from both sides (0x1D931E's `jz` and 0x1D94C6, esi 2 on both);
    the flags are written (`cmp cl,2`) before they are read. The entry: with
    the bit, pushad, the four numbers (a float when the value's type word
    +14h is 2, else an integer made a float, as 0x1D9249 reads them; 0 for
    a missing key), two `set_constant<float4>` with [ebp+8], compile's
    first argument, as the compiler; popad, the two instructions, back to
    0x1D94D2. xmm0 is lost; no xmm register is read after 0x1D94CB before it
    is written again.
  - 0x14990F `89 7C 24 2C 8D B3 3C 12 00 00` (`mov [esp+2Ch],edi` / `lea
    esi,[ebx+123Ch]`) in `renderer_context::renderer_context` (0x148570)
    becomes `jmp 0A425C00h` and five nops: right after the last of its 19
    `register_constant_binding` calls (20 in the exe), `scene_time`'s
    (0x1498A6..0x14990A). The entry makes the binding as the game does:
    {&value, 4, name, 0, 4} on the stack, the name the shared string of
    "scene_slomo" (0xE0B50, `ret 4`) held while
    `register_constant_binding` (0x3BCCC0, eax the binding, the resource
    manager [226047Ch] pushed, `ret 4`) runs, then let go (0xE0AE0 at a
    count of 0). The value is a 1.0 in `.srp` (read and execute): the
    binding copies the address and the size into the backend's constant
    host (0x3BCD02..0x3BCD09), which reads it; nothing in 0.26g names
    `scene_slomo`, so nothing writes it [V for the exe's strings; the read
    at every frame is I]. The handle the constructor keeps for its own
    bindings (+5520h, +5524h) is not kept for this one. The stack keeps the
    game's alignment at the call (20h of binding space after pushad).
  - No site holds an address (no base relocation under them), nothing aims
    inside one but its first byte, the build without the late join keeps
    them. The .bin ends with the address it was assembled for.
- **Nothing a match computes**: shaders and their constants only. Revisions
  12 and 13 play together; a server need not ask for 13 (one that offers the scrolling shaders
  in a patch does: its minimum revision is at least that patch's number).
- **Run with Unicorn** (`check_forward_scroll_026g.py`, the image as is and
  rebased by 1 MiB with its relocations): the original's and the build's
  compile from 0x1D8F40 to its `ret 0Ch` on 11 made-up materials (the
  config's two helpers, compile_begin, set_texture, set_constant<float>,
  <float2>, <float4>, the colour conversion, color_write_enable, end_pass
  and end_technique being stubs that record what they are given): the
  build's calls from the exe are the original's with the original's values,
  compile_begin's configuration differing only in bit 6 of byte 0Ch and only
  when the switch is true and `use_sequence` is not; its own calls are the
  two constants with the expected values, right after the uv distortion's;
  every register after both hooks and at the return is the original's; no
  lookup of a missing key. The third hook against the game's own
  `scene_time` binding (0x1498A6..0x14990F) with the same stubs and three
  made-up name counts: the same calls, the name `scene_slomo`, the value
  1.0 in `.srp`, the same alignment at the call, every register and
  [esp+2Ch] after the hook as the original's. With `--rev12=` the build is
  compared byte by byte with revision 12 (README, Revision 13).
- **Checked** (`selftest_026g.py` check 28, `test_forward_scroll`): the
  sites, the original's instructions around them, the configuration's
  accesses in compile, g_stage's code, the fill function's bit, the
  constructor's bindings, no relocation under and no branch into a site,
  the code's only calls (0xF6C00, 0xF6B10, 0x1A8900, 0xE0B50, 0xE0AE0,
  0x3BCCC0) and jumps back, the exe's texts it reads and its own data.
- **Not proven**: a game was not run; no scrolling shader exists in any
  archive yet (London plan, phase 2). That an unbound constant host holds 0
  (why `scene_slomo` is bound at all) is not proven; the binding makes it
  1.0 either way. That the GPU draws 0.29's shaders in 0.26g correctly is
  the London plan's phase 0, not this patch.
### 3.27 "Watch replay": the lobby's link starts the updater (written 2026-10-15, patch revision 14, README patch 29; first written as revision 12 at `.srp`+B00h, then as revision 13 at +DA0h; moved to +220h on 2026-10-16, when revision 12's patches' mount, revision 13's scrolling and the shell_32.dll check had taken the rest of the first page) — PROVEN in the code, the handler run with Unicorn; not run in a game

The lobby's History asks the server for a replay; the server sends `9E` with the link `replay <id 8 hex> <token 32 hex> <sha256 64 hex> <size>` (shown: no), and the movie's **Download and watch** calls `ExternalInterface.call("video_tutorial_link_clicked")` (contract: `private/lobby-movie/live/CONTRACT-R1-REPLAY.md`).

- **The original handler** (PROVEN, read 2026-10-15): the name `video_tutorial_link_clicked` is entry 106 of the callback names at 0x9C56C8 (0x9C5870); the handler 0x28CDA0 is in the file once, as the delegate's `mov eax,28CDA0h` at 0x283053, and no call or jump goes to it. Thiscall, one argument, `ret 4`, al = 1. It takes the lobby client from its owner (`[[esi+9Ch]+53ACh]`, vtable+3Ch, in eax at 0x28CDB6), then `ShellExecuteA(NULL, "open", client+1E6144h, NULL, NULL, SW_SHOWNORMAL)` through the delay-loaded SHELL32 import at 0xAEFE74, then `enable_element("root.statusPanel.videoTutorial", 1)` (0x22A660). Any text the server sent was opened as a file, a program or a URL.
- **The buffer** (PROVEN): lobby client +1E6144h. Its five uses in the file: emptied at 0x26F34F (`mov [ebp+1E6144h],bl`); filled by the 9E reader (0x2713A0: a show byte to +1EE611h, a u16 count of UTF-16 units into a 0x1F8-byte stack buffer, `WideCharToMultiByte(CP_UTF8, 0, ..., client+1E6144h, 2 × count, ...)` at 0x27143E, then a zero at the end, 0x271444; a failed conversion leaves an empty string); tested for empty at 0x2768A1; opened by the handler. It always ends with a zero.
- **The patch** (`REPLAY_LINK_HOOK`, `replay_link_026g.asm` at `.srp`+220h = 0xA425220, 472 bytes, ending at +3F8h, in the builds with the late join only; after the upload code, the default server's stub and its address (+1A4h, or +21Dh at the most with the longest address) and before the patches' mount at +400h (section 0.2), nothing else in between): `6A 01 6A 00 6A 00` at 0x28CDB6 becomes `E9 65 84 19 0A 90` (`jmp 0xA425220` / nop). No base relocation is under the six bytes and nothing aims inside them. `replay_link` (pushad; its own address by `call/pop`, nothing absolute) reads the buffer byte by byte against its pattern: the literal `replay `, a run of 8 lower-case hex digits, ` `, 32, ` `, 64, ` ` (a pattern byte 80h + n is a run of n hex digits), then 1..10 decimal digits and the zero; a zero matches no literal and no class, so it never reads past the string. Anything else: `popad / mov al,1 / pop esi / ret 4` and nothing more. A match: `GetModuleFileNameW(NULL, buf, 520)` (IAT 0x870074; a path of 520 units or more, none, or one without a backslash does nothing), the folder with its backslash as the working folder, `<folder>updater_skhid.exe` as the program (a fixed name in the code; the link is never a program), the command line `updater_skhid.exe <link>` (the updater reads no first argument: `updater_logic::replay::command`), `CreateProcessW` (not imported: `GetProcAddress(GetModuleHandleA("kernel32.dll"), "CreateProcessW")`, IAT 0x87003C / 0x870084) with no inherited handle, no flags and this environment, then `CloseHandle` of both handles (0x870040). The frame is 0xE84 bytes, under one page. ShellExecute is no longer called, so no file association, no shell and no zone prompt is involved; `enable_element` is not called.
- **Why nothing else changes:** the handler is reached only through the delegate; the code before 0x28CDB6 is the original's; the registers the callee must keep are restored by `popad` and the original's own `pop esi`; the direction flag is clear on return (`cld`). The rest of the original's body stays in the file, never reached (its relocations untouched).
- **Revision 14.** Nothing a match computes: 11 to 14 play together. A server sends a replay link only to a game that says 14 or more (`session::v026g::replays::MIN_REVISION`; an older game would hand it to ShellExecute: a file name that does not exist, so nothing opens; not tried). Revisions 12 (no signature check, the patches' mount, the shell_32.dll report) and 13 (the forward effect's scrolling) have no replay link.
- **Builds:** without a server sha256 `df9e66463bd8cb4bc4fa2536d87bc39fd7bfa8b17a0ebd184cb18392264a60ae`, with `--server=skhid.link:1234` `0ad0132a767dc87c571065b1c9cbe7071f152ae5fc26cc1b5f0fbdc629ebcf49` (12 907 128 bytes each). Against revision 13 made the same way by the tool of `main` decacfa (`7929b863…d842`, `94c3b0fc…16ae9`) they differ in the PE checksum (file 0x170, 2 bytes), the hook (file 0x27C1B6, 6 bytes), the revision byte (file 0x36FC66, 0D to 0E) and the 472 bytes of code at file 0xC4AC20, and in nothing else (481 bytes; compared byte by byte). (Before the move, as revision 13 at +DA0h on the never-released signed revision 12: `4aee61b7…45f3`, `ab5dd99e…aebe`; at `.srp`+B00h on revision 11: `80fe17f7…e7fa`, `2de2f1bc…9af2`; that code was 597 bytes, its pattern one byte per character.) `--no-late-join` still gives the revision 4 exe byte for byte (`3bd35a20…39fd`).
- **Checked:** `selftest_026g.py` check 29 (the hook in all three builds, the handler's instructions, the import and the strings, the name table entry and the one reference, every use of the buffer, no branch into and no relocation under the hook, `replay_link`'s IAT slots by the import table, its only exit, its pattern and fixed name) and check 15's layout of the first page (the code at [+220h, +3F8h), also with the longest default server, whose first page ends at +21Dh); 1114 checks in all pass. `check_replay_link_026g.py` (Unicorn 2.1.4) runs the handler from 0x28CDA0 on a made-up owner and lobby client, the Windows calls as stubs: the original opens every text with ShellExecuteA and enables the element; the build starts `<folder>\updater_skhid.exe` with `updater_skhid.exe <link>` and the folder for 16 good links (sizes 1 to 10 digits, all hex digits, folders with spaces, Cyrillic letters, a UNC path, the longest path that fits) and starts, opens or enables nothing for 83 other texts (other prefixes and case, every field one short, one long, upper-case, with `"`, `&`, `|`, `\`, `/`, `:`, `.`, `%`, `^`, a non-ASCII letter or a zero; extra and missing spaces, tabs, line ends; a signed, hex or 11-digit size; programs, paths, UNC, URLs; a 4000-byte text; an empty buffer) nor when Windows says no (a cut path, a folder too long for the name, no folder, no path, no kernel32, a failed start: nothing is closed that was not opened); the registers, the stack and `ret 4` as the original's. **Not run:** a game; Windows' own `CreateProcessW` from the game.

#### The game's `-match_replay=` (read 2026-10-15) — PROVEN unless tagged

- The command line is the ANSI one: `GetCommandLineA` at 0x12504 → 0xDE300, which drops the program's name (a quoted name up to its closing quote, else up to the first space or tab) and copies the rest into a 4 KiB buffer (0xA7DDDC, bounded by 0x12780).
- The keys are read once by 0xDF020: spaces and tabs part them; a key starts with `-` (or `--`) and ends at a space, a tab or `=`; after `=`, a value that starts with `"` runs to the next `"` that has no `\` just before it (0xDF1E3..0xDF20F), the quotes not kept; any other value runs to the next space or tab. **So `-match_replay="D:\My Games\...\srp_x.sur"` is one value with its spaces.** The value is copied with no bound into a 0x200-byte stack buffer (0xDF25A) and by `is_set_as_string` (0xDDEB0) into the 0x200 bytes at `[ebp-278h]` of `on_base_resources_created` (0x1EAE64): a value of 512 bytes or more overruns the stack (the original's own fault; the updater never gives one).
- The replay's name is then cut to 259 bytes (0x1F02E4..0x1F02EC: `0x12780` into a 0x104-byte buffer), and a name that starts with `\\` or has `:` before any `\` (0x41050) is a path used as given, `.sur` not added (0x1F0307..0x1F0311).
- The engine's file layer opens with `CreateFileA` (0x400C8; LIKELY the replay reader's path to it, 0x1F05BA → 0x1F01B0, not followed to the end). Either way a character the ANSI code page does not hold is lost in `GetCommandLineA`.
- **What the updater gives:** the full path in quotes when the ANSI code page holds every character of it and it is at most 259 bytes; else its short (8.3) form when that does; else it tells the player (`client/updater/README.md`, "Watch replay"). A path cannot hold `"`, and it ends in `.sur`, never in `\`.

### 3.28 One upgrade on several items: taking it off names the item (written 2026-10-17, README patch 30; the revision is the release's) — PROVEN in the code, the lobby code run with Unicorn; not run in a game

The owner's rule (2026-10-16): one camouflage or decal of the stash on any number of items at once, later weapon attachments too (`items.md` §6 has the client's model, which holds that as it is). Two things of the lobby named no item:

- **`46 02`** (PROVEN): Flash calls `drop_upgrade_item(<the item the upgrade window shows>, <upgrade>)` (the window's drop and cancel; `on_item_sold` with the camouflage's `is_attached_to`). The callback is the delegate `{lobby ui, 0x29A9D0}` (`mov eax,29A9D0h` at 0x2818FA, its one reference; the name `drop_upgrade_item` at 0x9C56E8). The handler reads the arguments' values at `[[ebp+8]+8]` +8 (the item, esi) and +20h (the upgrade, edi), logs `ui: detach_upgrade[%u] from [%u]`, takes the kit (`[[ecx+6A0h]+4620h]`) and the lobby client (owner vtable+3Ch), and calls 0x2708C0 (its only caller), which writes `46 02 <kit> <upgrade>` (new_packet 0x37A0E0, mutable_buffer::resize 0x7AB30, enqueue 0x379640). After the call it only pops what it pushed and returns `ret 4`.
- **`83 46 02`** and **`83 46 01`'s detach_first** (PROVEN): both call `process_detach_upgrade` 0x275220 (`ret 8`: the lobby client, the upgrade; its only callers 0x27558B and 0x275760, and its address is in the file nowhere else). It sets the upgrade's `is_attached_to` to 0 (`update_inventory_item` 0x287CC0), then finds **the first item of the inventory holding it** (`find_if` 0x277CF0 over lobby client+1EA978h, items 0B4h apart; its only call, 0x27534D, cdecl), and takes the record off that item (0x2775B0, 0x277520).

**The patch** (`SHARED_UPGRADES_HOOKS`, `shared_upgrades_026g.asm` at `.srp`+700h = 0xA425700, 181 bytes, entries at +0, +60h, +80h, +90h; in the builds with the late join only; after the patches' mount, which ends at +6C3h, and before the pickup code at +800h):

| site | original | patched | code |
|---|---|---|---|
| 0x29AAD5 | `push edi / push ecx / call 2708C0h` | `call 0xA425700` + 2 nops | `drop_upgrade`: reads the item as the handler does, writes `46 02 <kit> <upgrade> <item>` with the game's three packet functions (one resize of 13 bytes), keeps ebx, esi, edi, ebp |
| 0x275756 | `mov ecx,[eax] / push ecx / add eax,4 / push ebp / mov [esi+4],eax / call 275220h` | `jmp 0xA425760` + 10 nops | `detach_answer`: reads `<upgrade> <item>`, calls process_detach_upgrade with the item as a third argument, takes it off the stack, jumps to the case's end 0x275765 |
| 0x275589 | `push edi / push ebp / call 275220h` | `call 0xA425780` + 2 nops | `detach_first`: the same with the answer's host (ebx) |
| 0x27534D | `call 277CF0h` | `call 0xA425790` | `find_host`: the item whose id is the third argument (`[esp+58h]` there: above the call's return address, the three cdecl arguments, four registers and the function's 2Ch, its return address, the lobby client and the upgrade), or the end; with 0, the original find_if |

No base relocation is under a site, nothing aims inside one, and the code holds no address (relative calls and jumps only), so a rebase changes nothing. Then `process_detach_upgrade` takes the record off that item if it holds it, as before; every other item keeps its record. The upgrade's `is_attached_to` is still set to 0 first; the full inventory the lobby sends after each answer makes it again from all the records (`fill_inventory_contents`), so the stash shows a cosmetic that is still worn elsewhere as worn.

- **Wire:** `46 02 <kit> <upgrade> <host>` and `83 46 02 <upgrade> <host>`, the host appended. The lobby reads only that form (`requests.rs`): a game without the patch is refused at sign-in by the release's `SRP_MIN_PATCH_REVISION`, and its short `46 02` is an unreadable request (`st_cannot_relocate_item`).
- **Nothing a match computes:** the lobby only. A game with it and one without play together.
- **Builds:** without a server sha256 `0e1438823aff1da32916798467af910cfff0f7345e7b6103497e551f20f85087`, with `--server=skhid.link:1234` `019f2c678fa5e69e9fea60fa73ecbf2c8f7fcf9803b3dd7e2a44474b6da711de` (12 907 128 bytes each, the revision byte REVISION's). Against the builds of `main` 01eb96a (`9ef299c0…7aa1`, `b937e86f…3ad0`) they differ in the PE checksum (file 0x170, 2 bytes), the four sites (files 0x26474E, 0x264989, 0x264B56, 0x289ED5: 4, 7, 15 and 7 bytes) and the code at file 0xC4B100 (171 bytes that are not int3), and in nothing else (197 bytes; compared byte by byte).
- **Checked:** `selftest_026g.py` check 31 (each site in the original, the build without the late join and this one; the callers of the replaced calls; the handler's reads of the arguments and its epilogue; the code's only calls and jumps; no relocation, nothing aiming inside) and check 15's layout of the first page. `check_shared_upgrades_026g.py` (Unicorn 2.1.4) runs the original's and the build's drop handler from 0x29A9D0 and `process_succeeded_inventory_action` from 0x275520 (with the game's own searches and vector code; the packet functions, `update_inventory_item` and the log filter stubs) on inventories where one camouflage is on two vests and a decal on two: the original sends `46 02 <kit> <upgrade>`, takes it off the first vest for `83 46 02` and for `detach_first`; the build sends the item too, takes it off the vest named and no other (the item 0: the original's search; an item that does not hold it or no item: nothing), reads the message to its end, and `detach_first` moves a decal between the slots of one vest while the other keeps it. **Not run:** a game.

### 3.29 A kit chosen before the match starts is played at once (written 2026-10-17, README patch 31, the next release's revision; the rules: `kit-change-before-start.md`) — PROVEN in the code, the places' and the respawn rule's tick, the spawn, the view and the hud run with Unicorn; 3417c05 run in a game

The owner's wish: a kit may be changed while the match loads, not only after a death; the player then plays the new kit at once, where it stands, with no death and no penalty, and only before the match starts.

- **How a kit is chosen today** (PROVEN). The in-match hud shows the kits (`root.show_profiles_selector`, called only by 0x2383B0 at 0x238451) when the local player's `m_is_alive` (+33Ch) turns false and hides them when it turns true (`game_world_ui` 0x23B740, the transition at 0x23B7B3..0x23B7F7; 0x2383B0 also gives the hud the cursor, 0x23A930). The movie's `profile_changed` (a tab clicked) calls 0x233F06 → 0x1F8070, which queues a profile input {place, kit} at the game's time (0x1FB730, sent as 0xDE, relayed as 0xF3 to the others) whatever the player's state, then hides the kits. Each tick the place's tick 0x321790 makes the newest kit choice at or before it (the list `[core+89D8h]` + place×480h, kept in time order, §3.19) the place's next kit (place+0Dh, if under 3 and that kit has a player); the respawn rule's spawn (0x316DA0) makes the next kit current (0x316DE3). So a choice is played at the next spawn, which comes only after a death (0x316AA0) or the first entry (0x316BC0).
- **What the tick does** (0x316820, slot +50h of the respawn rules' vtables 0xA16014, 0xA16084, 0xA06D1C; `[esp+8]` the tick's time, `ret 0Ch`): per place the current player (0x2D0AB0); a place whose respawn time (rule+138h) is now goes to its team's list, a player in the world (+33Fh) to the list of the living; slot +54h (0x3171D0) gives points; each spawned place's player, when in the world, is taken out (vtable+24h with 1, 0x2DA790: an alive one through 0x2D99C0, which clears `m_is_alive` and counts nothing; the weapon put away; magazines and slots back into the store, 0x2FD2D0) and spawned (slot +5Ch with the place, the point and the time). The spawn reads only +8 (the position), +14h (the yaw, which 0x2DAB70 stores at +490h; the pitch passed is 0.0) and +20h (protection ms: then the damage model's `m_invulnerability_time_in_ms` +32Ch = time + it, found through the player's +108h vtable+8, 0x1F1790 → player+2E0h) of its point; on a change of kit it tells the client rule's view (+60h, 0x2678E0) and the statistics handler (vtable+2Ch, the client's empty `ret 10h`, 0x2DED60). No rule event: no death, no ticket.
- **The state and the statistics** (PROVEN). The time rule writes its state to core+18FE4h at each change (0x317EFC waiting, 0x318291 countdown, 0x31816B in process) and in deserialize (0x318025): 1 waiting, 2 countdown, 3 in process. Nothing in the simulation but the batteries (0x3148B3) reads it: before the start a player could move and fire as in play, but the client's input is in its warmup mode then (below). A place's record of the shared statistics is at handler+8 + place×1F8h (0x2F5AE3): `events[28][3]` u16 (0x335750), then `received_damage[20][3]` f32 (0x3353C0), 198h bytes in all. The feet are player+43Ch (the matrix +40Ch, which deserialize 0x2DB8B9 and the tick 0x2D9461 write and the controller is set from).
- **The pre-start camera and input** (PROVEN; the bug of the first build). The client's time rule ticks through 0x267AA0 (its node, 0x267A75): `logic_tick` (0x318100), then, for a tick first played (`time > [core+18FC4h]`, 0x267AD2) and a state other than the one the network client keeps for the camera (+1F1C60h, 0x267AE6), it keeps the new state (0x267AFB) and calls 0x27B1A0: the local player bound, then the player camera with the first person input (`switch_to_player_camera(1)`, 0x1F8200: the input handler's mode 0) in play (state 3), else the warmup's (`switch_to_warmup_camera`, 0x1F8190: mode 3, `process_warmup_mode` 0x242200, no movement) (0x27B1C8..0x27B1E5). The first world sets it the same way (0x27C147). The view of a spawn into another kit (0x2678E0) binds the new kit's player as the local one and then always calls `switch_to_player_camera(1)` (0x26794D): in the first build's game test (2026-10-17, two games, map 78) the kit changed and the player was then in first person and free to move during the countdown. A death and a respawn into another kit before the start did the same in the original.
- **The patch** (`KIT_CHANGE_HOOKS` and `KIT_PICK` in `patch_026g.py`; `kit_change_026g.asm` at `.srp`+EC0h, 319 bytes):
  - the tick's `push ebp` / `mov ebp,esp` / `sub esp,34h` (0x316820) become `jmp kit_change` and a nop. With the state 1 or 2, each place of the match (up to the end of the core's places, +1845Ch, as `active_player_ptr` 0x2D0AB0 checks: their buffer holds 20 and the entries past a match's places are as memory had them, an earlier match's dangling players among them) whose next kit is another (under 3, with a player), whose player is alive and in the world (a respawn time is set only for a dead player and an entering one is not in the world: none is due to spawn) and whose record is all zero is respawned: out of the world (vtable+24h, 1), then the spawn (rule vtable+5Ch) with a point on the stack: +8 the feet, +14h the yaw, +20h what is left of its invulnerability (+32Ch less now; when it is -1 or not after now, -1 less now, which the spawn adds to now: -1, not invulnerable, and +330h 0. A protection of 0 makes the spawn write neither and leaves the new kit's damage model with what an earlier life of that kit gave it, which a game that loaded a world since does not have: the world holds the kit in use's player alone, so the worlds' hashes would part). Then the three instructions and back to 0x316826;
  - `push 1` / `call 0x1F8200` at 0x26794D become `call kit_view` and two nops: the player camera when the network client's +1F1C60h is 3, else `switch_to_warmup_camera` (edx = the game world), as 0x27B1C8 decides;
  - `call 0x23A1A0` / `push 0` at 0x23B7DA (the transition to alive) become `jmp ui_alive` and two nops: the call, then 0x2383B0 with 1 when the network client's +1F1C60h (`[[[hud+18h]+9Ch]+53ACh]`, the client 0x23B74F reads) is not 3, else 0, then 0x23B7FC;
  - `lea eax,[ebp+0D0h]` at 0x23B7FC (every update) becomes `call ui_start` and a nop: a live local player gets 0x2383B0 with 1 while that state is not 3 and with 0 when it is (0x2383B0 returns at once when the kits are as asked);
  - `push 0` / `mov eax,ebx` / `call 0x2383B0` at 0x233F28 (the `profile_changed` callback, after it queued the pick through 0x1F8070) become nine nops (`KIT_PICK`). The original hid the kits after every pick and the hud's cursor with them (0x2383B0 → 0x23A930 with 0); with the second build the switch's own change of the local player then showed them again (0x2383B0 → 0x23A930 with 1, which puts the cursor at the centre of the screen: the only place the hud sets it, besides a key at 0x25CF08): the second game test (d8acf39), "clicking on a new set resets cursor". Now the kits stay shown and the cursor where it is, for another pick, before the start and on the death screen alike; they are hidden when the player becomes alive in play, as before (0x23B7DA).
  - No site holds an address; nothing branches into one (the checks read every rel32 and rel8 of `.text` and every relocated pointer).
- **The camera stood still before the start** (PROVEN; the owner, 2026-10-18: "once we added the ability to switch kits at start up, it is no longer possible to move the camera around"; then, of the first fix, which let every move turn the camera: "it should rotate when the left button is pressed, just like in the lobby", "and scrolling for putting the camera further, just like in the lobby"). The warmup camera turns by the mouse: the player's input handler (game world+4D58h, vtable 0xA04048, `input_priority` 10) adds each move up in its `on_mouse_move` (0x240200: +338h, the wheel into +348h), and its end of the frame (0x241170) plays mode 3 as `process_warmup_mode` (0x242200): the distance +354h plus the wheel of the frame (0x242219, stored with no limit; 1.0 from the constructor, 0x23FDE7), the yaw +34Ch and the pitch +350h plus the turn (0x24225E, no limit either); the player's input gets no turn (it is built in mode 0 alone, 0x241250). The input world gives a move to its handlers in their priority's order, lowest first (`add_handler` 0x3B7D60 inserts by vtable+10h with a lower bound, 0x3B8010), and stops at the first that answers 1 (0x3B9F10..0x3B9F32; the move is the mouse's +4, +8, +0Ch, read again for each handler; the mouse is the world's +28h, 0x3B7FB0, the buttons held its +10h, the left one bit 0, 0x3B9610). The game world's own handler (game world+1F8h, vtable 0x9FC588, priority 7: before the player's) moves the hud's cursor in its `on_mouse_move` (0x25D0E0: the system's cursor, [handler+4B70h] vtable+0, then the hud's pointer, game world vtable+30h = 0x25D160, which keeps it at game world+8Ch, +90h) and answers whether the cursor is shown (hud+2DCh, the hud being game world+1FCh). The kits show and hide the cursor with them (0x2383B0 → `show_cursor` 0x23A930; the kits at hud+2DFh), so the kits patch 31 shows before the start gave every move to the cursor and the warmup camera none.
  - **The lobby's character view** (`lobby_camera`, vtable 0xA09EB4, priority 10, before the lobby menu's 100, 0x21E9A0; `on_mouse_move` 0x283D30): the wheel moves its distance by 0.001 a unit (0.12 a notch, nearer when it turns forward); while a button of mask 52h is held (the mouse's buttons 2, 5 and 7: the right one, not the left, which is the menu's) a move turns the camera and is not the menu's, so the cursor stays; the first move after the release puts the system's cursor back where the drag began (0x29A680). Its tick (0x283EF0) keeps the distance within 1.5 (0xA7AFA4) and 4.6 (0xA2DB0C) and the pitch within ±60° (0xA2DB08, 0x9C56C0).
  - **The patch.** `WARMUP_LOOK_HOOK`: the first four instructions of 0x25D0E0 (ten bytes) jump to `warmup_look` (`warmup_look_026g.asm` at `.srp`+D40h, 184 bytes, after the weapon preview's draw, before the shell_32.dll check; its state, 12 bytes, at the late join's state page +E20h, after the weapon preview's, found from its own address). While the kits are shown (hud+2DFh; the cursor with them), the player's input handler exists (game world+4D58h, 0 while no local player is bound, 0x242CEE) and is in mode 3 (set by `switch_to_warmup_camera` 0x1F8190 alone; the warmup camera is the pre-start's, 0x27B1C8, and kit_view's), at every move:
    - a button of 52h held: a drag at once; the left button held: the moves since it went down add up (|x| + |y|), and past 8 it is a drag; before that the move is the cursor's, so a click stays the selector's. A drag begins by keeping the hud's pointer (game world+8Ch, +90h) and moving the pointer to (-128, -128) (vtable+30h): a press on a kit is let go outside it (the release is handled before the move, 0x3B9E50, at the pointer's place) and picks nothing. In a drag every move answers 0: it goes on to the player's handler, which turns the camera (and moves it by the wheel), and the cursor stays;
    - no button held: a drag ends (`drag_end`, `warmup_limits_026g.asm` at `.srp`+A98h, after the seek code) with the pointer and the system's cursor back where it began (game world vtable+3Ch = 0x25D130, as the lobby puts its cursor back); a move of the wheel then goes on to the player's handler with the mouse's x and y of the frame made 0 (the wheel alone: nearer or further, no turn, the lobby's step); any other move is the original's (the cursor's, 1).
    Any other time the ten bytes run as they were (then 0x25D0EA).
  - `WARMUP_KEYS_HOOK`, a button held from before the start (PROVEN; the owner, 2026-10-19: "players will shoot immediately once match starts"). `vostok::input::platform::mouse::process` (0x3B9D90; the buttons held now at the mouse's +10h, the frame before at +20h) tells the handlers (vtable+8: the input world, the code, the kind; until one answers 1) of each button gone down (kind 0, then 2), let go (1) and still held (2, at every frame); the codes are 151h + the button's bit (0x3B9CA0), the left one 151h. The game world's `on_mouse_key_action` (0x25CF60, priority 7) takes them (1) while the cursor is shown (handler+2E0h): the kits before the start, the death screen. The player's `on_mouse_key_action` (0x23FF50) keeps each kind as the action's state (handler+98h, 0x242580) and `action_present` (0x242460) answers for an action with any kind queued; the play step (0x24128A..0x2421B0) asks it of 35 actions and of only three for a press, so a drag still held when the kits went at the start came to the player's handler as kind 2 at the next frame and fired the weapon, and as kind 1 at its release and fired once more (aimed, with the right button; a pick held through a respawn the same). The owner, 2026-10-19, on the first writing, which gave the release on: "when mouse is released one shot still happens". The first five instructions of 0x25CF60 (ten bytes, no absolute address under them) jump to `warmup_keys` (`warmup_keys_026g.asm` at `.srp`+9A8h, 88 bytes, after the suicide key, up to the seek code; the buttons marked in a dword at the late join's state page +E30h, after the warmup look's): a mouse button's kind 0 while the cursor is shown marks it, its kind 1 unmarks it; while the cursor is not shown a marked button's kind 2 is taken there (`inc eax; ret 0Ch`: al is the button + 1, not 0, and the input world tests `al` only; the player's handler not told), its kind 1 unmarks it and is taken the same, and its kind 0 (a release missed) unmarks it and runs on; anything else runs the ten bytes and goes on at 0x25CF6A. The player's handler only misses the held states and the release of a press it never had: nothing a match computes changes.
  - `WARMUP_LIMITS_HOOK`: `movss [esi+350h],xmm0` at 0x24225E (eight bytes) becomes `call warmup_limits` and three nops: the pitch stored within ±60°, the distance within 1.5 and 4.6, as integers (a positive float's bits order as signed integers, any negative one is below, a not-a-number is past the limit); eax and edx are written after it before they are read. The warmup camera so starts at 1.5 (the stock 1.0 is nearer than the lobby's nearest).
  - The death screen (the player's handler in mode 0 or 2), a replay's cursor without the kits, or no local player: the original's. Nothing a match computes: the warmup mode gives the player's input no turn.
- **The order of a tick** (PROVEN). The world's tick (0x2D1E20) goes through seven tick lists from core+18840h in steps of 10h (0x2D1E3F..0x2D1E66), each sorted by its nodes' keys (0x2D1D20); a key is the core's counter (`[core+18FB0h]`) at registration. The places' nodes (place+10h, the tick 0x321790) go into list 0 when a place enters (0x2D28F6..0x2D2900), keyed at the slot's bind (0x321877); the respawn rule's node (rule+11Ch) into list 0, keyed when the rule is given the core (0x316A3A..0x316A5A); the time rule's (rule+110h) into list 1 (0x318070..0x31807A). The core binds its places (the loop at 0x2D3880, its counter 0 from 0x2D385A) before it gives the rules the core (0x2D3B2B..0x2D3B47) in one function: in the original exe the places' keys come first. In the builds with the late join (every build this patch is in) the bind is the late join's `bind_slot_reserved`, which binds each place with a counter of its own from (place+1) << 16 and gives the core's back after: the places' keys are (place+1) << 16 + n, the rules' come from the core's counter, from 0. So in each tick of the build: the respawn rule (this patch's pass, then the tick's spawns), then the places, which take the tick's kit choices, then the time rule. Hence:
  - a choice for a tick before play is the place's next kit from that tick on and switches at the next tick;
  - a choice for the last tick before play switches in the tick whose time rule begins play, before the change: the view gives the warmup (the camera's state is still the countdown's), then the time rule's change of that tick, first played, gives play's camera like everyone's;
  - a choice for the tick that begins play, or a later one, is the next life's kit (state 3 when the respawn rule runs next);
  - on the death screen a respawn at a tick takes the last choice for a tick before it; a choice for the respawn's own tick is the life after's;
  - every game keys its places and rules alike (the late join keys each place by its number, whenever it is bound), so every game has this order;
  - a game that gets a choice late plays the ticks again from a world before it (a rollback) and switches at the tick the choice is for, as every other game; a choice the server moved (0xF4) or discarded (0xF5) is moved or dropped in its owner's game too (`game.rs::profile_input`), so every game has the same list;
  - the camera follows the state of the newest tick first played: a rollback that plays a switch again after play began leaves the player camera (`kit_view` reads +1F1C60h, which only a first play changes), and one that plays the start again does not change the camera (0x267AD2). No order of the two can leave a player in the warmup in play.
- **The hud and the start.** The selector is shown or hidden by the same state as the camera, for a live player at every update: shown while it is not 3 (after a pick too), hidden at the next update after it is 3. A pick in the last frames is for the game's current tick: before the start tick it switches (and the start then releases the player), from it on it is the next life's kit.
- **Late joiners.** A newcomer comes into a match in play (the server takes players in only then): its slot is bound with its first kit (the lowest the match loaded) current and next (0x32182D), it spawns in it, the state is 3 at its first spawn, the hud does not show the kits and the pass does nothing; a choice of its is for its next life. The same on `main`.
- **Why every game does the same.** All the pass reads is in the world or follows from it at that tick: the state, the places' kits, the alive and in-world flags, the statistics, the transform and the damage model. Patches 18 and 20 give the new kit's weapons their spawn state. The camera and the hud are no part of a world.
- **Decisions.** The cutoff is the match's start (state 3; the countdown is 20 s in every mode, after the wait for four fifths of the roster), the same for every place. Not after a shot, a hit, a kill or any event, and not after damage taken from a player, so a switch never heals or refills a player who fought; several switches are possible while nothing has happened, each by a pick in the selector, which stays open. Damage from the world (a fall, an anomaly) is not in the record [accepted: before the start only]. The look up or down is level after a switch.
- **The server** (`match-server`): `TimelimitRule::before_play` (a tick before the start's) and `Game::note_kit_choice` count each kit choice relayed for a tick before play (a player's 0xDE, a bot's) into `PlayerResult::kit_changes`; the rewards take each as a life more for what a match is taken to have used of a player's stacks (`autobuy::used`, `respawns`). The server's bots pick a kit only once play has begun.
- **Replays** recorded before this patch play differently in a game with it where a live player chose a kit before the start: the change is the `next` row of `session/src/v026g/simulation_changes.tsv`, so a game of the release that ships it is not given them (`session::v026g::replays`).
- **Checked:** `selftest_026g.py` check 32 (the four sites, the tick, the spawn, the view, the order of a tick and the camera's state byte for byte, the code's calls and exits; the mouse move's, the mouse key's and the warmup mode's sites, the two handlers' vtables and priorities, the input world's order and its mouse, the mouse's kinds and codes and the player's kept states, the lobby camera's buttons, step and limits, the warmup mode's stores and the hud's flags); `check_kit_change_026g.py` (Unicorn): the rules' cases, the ticks around the start with the places' own tick in the build's order, the view before play, in play and after a rollback over the start, the hud, the pick, a death screen's respawn (a pick, a re-pick, a pick for the respawn's tick), a late joiner, a match of six places with an earlier match's entries past them, a new kit whose damage model an earlier life left protected and fading, mouse moves through the input world's loop and the game world's handler (moves, a click with a little movement, a left drag, a right drag, the wheel, the wheel in a drag, the death screen, a replay's cursor, no local player, no cursor), the mouse's buttons through the input world's loop and the game world's handler (a left or right drag held into the start, a pick held through a respawn, a click let go before the start, a press in play: the original tells the player's handler of a held button at the frame after the start, the build only of its release) and the warmup mode's step with its limits; the original and nine broken builds fail it. **Run in a game** (2026-10-18, the owner): the first warmup look (every move turned the camera): "not very good, it should rotate when the left button is pressed, just like in the lobby". **Not run in a game: the drag, the wheel and the limits.** **Run in a game** (2026-10-17, the owner, two games): 3417c05, the build before the last two checks: a switch before the start in place with the warmup's camera and the cursor kept, re-picks, re-picks on the death screen, the start's release. **Not run in a game: this build**, which adds the end of the places and the protection when none is left: a switch of a place of the match plays as in 3417c05 (with no protection left the new kit is not invulnerable in either; in the tick its protection ends it now is not). The first build in a game (packets of match 0x8d9935): the player sent 0xDE for kits 1 and 0 at 2.95 s and 4.5 s of a countdown that began at 0.4 s; its world at 4.55 s has kit 0 current and next, the last of its two choices.

### 3.30 A weapon's upgrade window shows the weapon (written 2026-10-17, README patch 32, the next release's revision) — PROVEN in the code, the handlers, `setup_profile`'s entry, the camera's clamps and the decal pose run with Unicorn; the first build run in a game

The owner's decision (2026-10-17): the upgrade window of a weapon (the scope slot, "Оптика") shows the weapon in the lobby's 3D view instead of the character's chest.

**What the stock lobby does** (read in the exe; the class names from the 0.21d matches in `tools/re-kit/v026g/inferred.tsv`):

- **One 3D subject.** The lobby menu has three `survarium::lobby_character` objects (ctor 0x284730, 0xD558 bytes): its own at +694h and two squad panels at +698h/+69Ch (`social.md`, the squad window). A character keeps three copies of a profile (0x4634 bytes each at +18h; the current one's index at +D2B4h), the kit it shows at +D2B5h, its player at +D2B8h, the lobby menu at +D2BCh. `setup_profile` 0x284B10 (eax = a profile, then the character and the kit on the stack, `ret 8`) writes the kit to +D2B5h, returns at once when that kit's id and revision are those shown and nothing is pending, and otherwise copies the profile into the next copy and queries the player's resources; `on_player_ready` 0x284F20 makes the new player and, when another call came meanwhile, calls `setup_profile` again (0x2850C5). Its callers are the lobby's refresh (0x222A42: the account's profile, lobby menu+6A0h = lobby client+1E6344h, and its chosen kit, profile+4632h), the squad panels (0x223029) and that re-call: nothing in the stock game shows an item on its own. Of the 112 movie callbacks (names at 0x9C56C8) none does.
- **What the player holds.** A kit is 23 slots `{u16 item index, u16 amount}` at kit+40h (`lobby.md` §2.5); slot 7 is weapon 1, slot 10 weapon 2 (the server's `WEAPON_SLOTS`), slot 4 the legs armour. The lobby reads the same "slot 7, or slot 10 when 7 is empty" for the squad window's player type (0x22248A, of the first kit). That the lobby's player holds that weapon is LIKELY (the player is the match's own class, spawned with the kit; not traced to the weapon's attach).
- **The window.** `start_item_upgrade` is the delegate 0x29A870 (callback 5, registered at 0x281843 in the order of the names, as `drop_upgrade_item`'s at 0x2818FA): esi = the lobby ui's character (+694h), edi = the item, then 0x265DE0, which sets the upgrade mode (+D515h = 1; set nowhere else; the ctor's 0x265B40 clears it) and calls the camera's setter 0x2845C0 on the camera (lobby menu+10Ch; ctor 0x283BC0, tick 0x283EF0) with the look-at height in xmm0 (1.3, or 0.8 when the item is the shown kit's slot 4), the side angle 0.37 on the stack and the distance 1.5 in xmm1; the setter keeps the distance it replaces at 0x1F1D9E4 and sets the speeds that move the camera there. Then the input focus, the scene's decal texture and the decal markers (0x265F80, slot 3 skipped). `exit_item_upgrade` (0x29A890 -> 0x265F10) clears the mode and calls the setter with 1.2, 0 and the kept distance. The camera's tick clamps its distance to 1.5..4.6 at three loads of the float at 0xA7AFA4 (1.5, shared with other code): the wheel (0x284081, before a `ja` on the flags of the comparison before it), the step towards the target (0x28428C) and the last clamp (0x284316); the camera's other functions do not read it.
- **The decal pose** (found after the first build's game run, 2026-10-17: camera in, the weapon's kit shown, but a T-pose and no weapon). The character's update (0x2849F0, every frame) reads its upgrade mode (+D515h, read nowhere else, 0x2849F8) and gives it to its player at +F49Ch (0x284A33; the player's ctor clears it at 0x1F1BFB; nothing else writes it). `player::mt_draw` (0x2474A0) then puts the skeleton's bone matrices in their bind pose (0x2476F8: `[[player+F290h]+108h]` vtable+38h with the matrices and their count: the T-pose) and draws the held item (+128h) with a transform 10 m down, (0, −10, 0) (0x24778F); `player::insert` (0x1F30E2) takes the transform at +F4A0h instead of +40Ch. That is the decal editor's presentation: the decal markers are placed on the base character's collision model (+D504h, `base_character.model/collision`, made at 0x265741 and put into the collision scene +D2C0h by the start), in its bind pose.

**The patch** (`WEAPON_PREVIEW_HOOKS`, `weapon_preview_026g.asm` at `.srp`+2E50h = 0xA427E50, 431 bytes, after the late join's code (it ends at +2E4Eh) and before `.srpd` (+3000h), which stays where the late join was built for; its state at `.srpd`+E00h = 0xA428E00, after the patches' mount's (+800h..+D07h): the mode, the window's item, the least's choice):

| site | original | patched | code |
|---|---|---|---|
| 0x265E73 | `call 2845C0h` | `call 0xA427E50` | `start_cam`: the first kit (below the profile's count) whose slot 7, or slot 10 with an empty 7, indexes an item of the table (below its count) with the window's id: the mode 1, `setup_profile(profile, character, chosen kit)` (the hook gives it that kit), the camera's height 1.05 and distance 0.85, the least 0.6. None: the mode 0; if a weapon was shown before (a window opened over another), `setup_profile` with the chosen kit; the original's values. xmm0, xmm1, xmm3 (read after the call) and every register kept |
| 0x265F44 | `call 2845C0h` | `call 0xA427EBD` | `exit_cam`: after a weapon, the mode 0 and `setup_profile` with the chosen kit; the original's values, the kept distance at least 1.5 while the least is 0.6 |
| 0x284B10 | `push ebp / mov ebp,esp / and esp,-8` | `jmp 0xA427EFD` + nop | `setup_hook`: in the mode 1 and for a character whose +D515h is set, the kit argument becomes the weapon's kit; then the three instructions and a jump to 0x284B16. Every register kept |
| 0x284081, 0x28428C | `movss xmm1,[0A7AFA4h]` | `call 0xA427F36` + 3 nops | `min_step`: xmm1 = 1.5, or 0.6; no flag changed |
| 0x284316 | `movss xmm0,[0A7AFA4h]` | `call 0xA427F4C` + 3 nops | `min_final`: as `min_step` into xmm0, and once the mode is 0 and the clamped distance (xmm1) is 1.49 or more, the least is 1.5 again |
| 0x284A33 | `mov [ecx+0F49Ch],al` | `call 0xA425D20` + nop | `player_mode` (`weapon_draw_026g.asm` at `.srp`+D20h, 32 bytes, after the forward effect's scrolling and before the shell_32.dll check): the decal pose 0 while the mode is 1, the window's mode otherwise; every register kept, and what follows reads no flag before setting it |

- **Relocations:** the three loads held the address 0xA7AFA4; their base relocations (0x284085, 0x284290, 0x28431A) are disabled. No other site holds an address, nothing aims inside one. The code holds no absolute address: it finds its state with `call`/`pop` and `lea`, so a rebased image works (run so).
- **What it leaves alone:** the kit the lobby plays with (profile+4632h) and everything sent to the server; the decal markers (0x265F80 runs as before; a weapon's scope is slot 3, which it skips); the squad panels (their +D515h is never set); armour windows and weapons no kit holds (byte for byte the original's calls and values, PROVEN by the run below). A match is not touched: the lobby character and camera are not in the world.
- **A weapon no kit holds** (written 2026-10-21, the owner: "the weapon should move to character hands", "it shouldn't replace what the character has", weapons only, not armour; PROVEN with Unicorn, `check_weapon_stash_026g.py`; not yet run in a game): a weapon only in the stash, or a weapon 2 behind a weapon 1, is shown in the hands of the kit chosen in the lobby, from a scratch copy of the account's profile; the account's profile is never written. The two `call find_kit` (start_cam, setup_hook) call `find_kit_or_show` (`weapon_stash_026g.asm` at `.srpw`+360h = 0xA42A360, after the camouflage's code, 408 bytes; `-DFIND` of `weapon_preview_026g.asm`): find_kit's answer when a kit holds the item; otherwise, for the account's profile only (its inventory vector follows it: profile+4634h {begin, end, allocator = [0x1F1D9C4], storage end}; a character's own copy and a squad mate's temporary one have none), the item is found in the inventory (`find_item_by_id` 0x277AA0), must be a weapon (`item_by_id` 0x1F0090 on `[[profile−C84h]+53A4h]`, the dictionary, and its `item_category`, u8 +228h, 8 or 13..17, the movie's `isWeaponCategory`), and then: a scratch profile (allocated once from the game's allocator, 4634h bytes, built by `player_profile`'s ctor 0x2F73F0; its pointer at patch 32's state +0Ch) is made the account's by its copy-assign 0x284D10 (a deep copy: an item record's two buffer_vectors point into the record itself), the weapon's record is the table's own or the inventory's appended after the last (0x270390, the record copy-assign; the table holds 69: a full one shows nothing new), the chosen kit's weapon 1 is that record, and the chosen kit's revision is `0x80000000 | n` (state +10h), a new one per scratch: `setup_profile`'s early return compares the kit's id and revision with the character's copy (0x284B1D..0x284B83), so the character is rebuilt from the scratch, and from the account's profile again at the close. setup_hook then gives `setup_profile` the scratch and the chosen kit; start_cam the weapon's camera. `setup_profile` keeps no pointer to the profile it is given (it deep-copies it into the character's own copies; on_player_ready's re-call passes the character's copy). A refresh while the window is open (the weapon's camouflage or scope attached) makes a new scratch with the weapon's new record. What the player holds depends on the slot's item index alone (inventory_cook::translate_query 0x2C7250, inventory::setup_from_profile 0x2FCB70: neither reads the amount, the count or the revision). The lobby movie now asks `start_item_upgrade` for every item again.
- **Armour the chosen kit does not wear** (written 2026-10-21, the owner: the appearance window of body armour "is still tposing, if it is not equipped"; chosen: "wear it, display only"; PROVEN with Unicorn, not yet run in a game): shown worn, in the decal pose the window gives the character, from the same scratch: its first armour slot (0..6) that `items_dictionary::can_move_item_to_slot` 0x2CA910 (stdcall dictionary, item_category, slot; al; `ret 0Ch`) allows for its category (1..7), in the chosen kit. Patch 32's start (0x265E73), exit (0x265F44) and `setup_profile` (0x284B10) sites now go to `weapon_stash_026g.asm`'s entries (+0, +5, +0Ah: `armour_start`, `armour_exit`, `armour_setup`), which go on to patch 32's start_cam, exit_cam and setup_hook (`-DPREVIEW`): armour_start, for armour the account's chosen kit does not wear (any slot), sets the state's ARMOUR (+14h) and calls `setup_profile` with the chosen kit, which armour_setup, for the character with its window open, gives the scratch; armour_exit clears it and calls `setup_profile` again (the account's profile, rebuilt: the scratch's revision differs). The camera and the decal pose are the original's. Patch 32's find_kit_or_show moved to +0Fh (`-DFIND=0xA42A36F`).
- **Limits:** the two camera values are first guesses; no game has been run with them.
- **Since patch 45** (section 3.42, 2026-10-21): start_cam's and exit_cam's jumps to the setter go to `weapon_table_026g.asm`'s camera (`-DCAMERA`), which in the mode 1 flies the camera to the workbench and has mt_draw draw the held weapon there; the two values above then give way to the bench's. Without `-DCAMERA` the code assembles to this section's bytes.
- **Since patch 48** (section 3.45, 2026-10-21): the two `call FIND` go to `upgrade_preview_026g.asm`'s find (`-DFIND=0xA42C00F`), which calls the stash's find_kit_or_show and, for a weapon an upgrade was dropped on in this window, gives the scratch with the change applied.
- **Builds:** without a server sha256 `74c0f7f4cc761c8895b3e2ad9752c65bdaa2e2bf78921ec46396fb18fac8dbc9`, with `--server=skhid.link:1234` `a5af932beb634b60b585e22a5a8e765d2bf194d9795af293cfc172ecc630c379` (12 907 128 bytes each, the revision byte REVISION's). Against the builds of `main` 5fba57f (`fdc03b41…f35f`, `786f42aa…7e68`) they differ in the PE checksum (file 0x170, 2 bytes), `.srp`'s virtual size (file 0x330, 2 bytes: 0x2E4E -> 0x2FFF), the seven sites (files 0x255273, 0x255344, 0x273481, 0x27368C, 0x273716, 0x273F10, 0x273E33: 4, 4, 8, 8, 8, 6 and 6 bytes), three relocation entries (3 bytes), the draw at file 0xC4B720 (32 bytes) and the code at file 0xC4D850 (431 bytes), and in nothing else (514 bytes; compared byte by byte). The first build, without the draw, was `c4d557f9…0fd3` / `3217cb81…d8ef`.
- **Checked:** `selftest_026g.py` check 33 and the late join's layout and relocation checks. `check_weapon_preview_026g.py` (Unicorn 2.1.4) runs the original's and the build's handlers 0x29A870 and 0x29A890 with the camera's own setter (the decals, the scene's two calls, the input focus and `setup_profile`'s body stubbed), `setup_profile`'s entry, and the camera tick's blocks 0x284020..0x2840C5, 0x284222..0x2842C6 and 0x28430E..0x28433B, as is and rebased: armour, legs armour (0.8), a stash weapon, a weapon 2 behind a weapon 1, an index past the table, a kit past the count and item 0 leave the camera, the character, the globals and the stubs' calls the original's; the AK (chosen kit), the SVD (kit 2) and a lone pistol (kit 1) are shown with their kit and the build's camera, and the close shows the chosen kit; a vest's window over the SVD's brings the chosen kit back and keeps no distance under 1.5; the hook changes the kit only in the mode 1 for a character with its window open and keeps every register and the frame; the three blocks give the original's distances, registers and flags in 500 cases each with no weapon's window, and the least is 0.6 from one until the camera reaches 1.49; the decal pose's write gives the window's mode as the original with no weapon shown and 0 with one, keeping every register. **Run in a game:** the first build (2026-10-17, round 4c data): the camera moved in and the weapon's kit was shown, but in the decal pose (a T-pose, no weapon), which this build's draw takes away. **Not run:** this build.

### 3.31 The order of the archive reads is a strict weak ordering (written 2026-10-17, README patch 33, the next release's revision) — PROVEN in the code, the predicate and the game's sort run with Unicorn; not run in a game

**What crashed.** A lobby with the scope round 4d data (2026-10-17 14:10, dump `SRP-crash-20261017-141046-017-p44536-t46012-0.dmp`; the scope UI session's analysis): an access violation at 0x10BCCE reading `[0+0xA4]` in the predicate, called with the 0 that lies just before a 58-slot array on the stack. The data are valid; the batch read the kit weapons' new sounds from `resources.db.patch_14` beside stock ones from `resources.db`.

**The stock code** (names from 0.21d, which has the same predicate, `hdd_manager_sorting_predicate::operator()` 0xCD023, and sort; PROVEN by the listings):

- `hdd_manager::grab_sorted_queries` (0x10BE80, slot 2 of the hdd_manager vtable): the pending `query_result`s (linked by +270h, taken from +21A8h) go into an array of `alloca` (0x424CA0) and are sorted by std::sort: the introsort loop 0x10BF70 (called there, 0x10BF13, and by itself, 0x10C025), then the final insertion sort 0x10C060 (called there alone, 0x10BF1E): a guarded insertion of the first 16 (0x10C120) and an unguarded one of the rest (0x10C280: back while the predicate says the value is before the previous one, with no check of the start). The partition (0x10C0B0) has two unguarded loops too. The fifteen calls of the predicate are all in these helpers (0x10BF70..0x10C3ED).
- The predicate (0x10BCC0; eax = A, [esp+4] = B, `ret 4`, al = A before B):
  1. a query whose iterator node (+A4h) is 0 before one whose node is set; two such by address;
  2. if +158h (the parent: `query_result_for_cook`'s argument, 0x104527) is the same: +2B8h (`query_result`'s argument, 0x10460C; LIKELY the priority) descending, when they differ;
  3. if both file nodes (+A8h, else +A4h) have bit 4 of +30h (in an archive, `vfs_iterator::is_archive` in 0.21d) and both iterators' physical paths (0x4BC10, `get_physical_path`) are equal by `strcmp_s` (0x3B5A0): the file offsets (0x47160, `get_file_offs`), A's below B's;
  4. else the parent, or the query itself when it has none: A's below B's.
- Not a strict weak ordering: two reads of one archive are ordered by offset, a read of another archive by step 4, so A < C by offset with C < B < A by address is a cycle; the priority of step 2 between queries of one parent can also cross the offsets of others. With a cycle the unguarded loops step past the array. With `resources.db` alone there was one archive; patch 27 mounts every `resources.db.patch_<n>` over it.

**The patch** (`READ_ORDER_*`, `read_order_026g.asm`, assembled at 0x10BCC0, 0x1A0 bytes = the body to the next function 0x10BE60, int3 after the code; in the builds with the late join only): the body is replaced in place, with the same entry and convention, by a lexicographic order:

| key | as |
|---|---|
| the iterator node set | the original's 1 (nulls first, by address) |
| +2B8h descending | the original's 2 |
| the file node in an archive | archive reads first |
| archive reads: the physical path (`strcmp_s`), then the file offset | the original's 3 |
| others: +158h, or the query | the original's 4 |

It uses the same three helpers as the original (`get_physical_path` into two buffers of its 0x250-byte frame, `strcmp_s`, `get_file_offs`), keeps ebx, esi, edi and ebp, returns al. The body held no address (no base relocation), nothing outside it aims inside it, so the in-place replacement touches nothing else. A batch of one archive (or of files in none) in which queries of different priorities share their parent is answered exactly as before; reads of one archive stay in file order within a priority.

- **Nothing a match computes:** the order in which files are read. A game with it and one without play together.
- **Builds:** without a server sha256 `88899b9143e07f6f1478ad41aeac3d48d188ec4f9a00524946224ca434e55a8c`, with `--server=skhid.link:1234` `274a912a8cb3d93cf5bf5d27f1f114163ca97b74a31c654b191882ba4d534bd2`. Against the builds of `main` 8511083 (`1d7be23b…1227`, `69866b5a…e9f4`) they differ in the PE checksum (file 0x170, 2 bytes) and the predicate (file 0xFB0C0, 395 bytes), and in nothing else (397 bytes; compared byte by byte).
- **Checked:** `selftest_026g.py` check 34. `check_read_order_026g.py` (Unicorn 2.1.4) on made-up queries (archive nodes with the flags 4 | 2000h, their offset beside them, their mount's path at +28h), as is and rebased: on 120 random sets of 3 to 12 queries over two and three archives, loose files and null iterators the build's predicate is irreflexive, asymmetric, transitive and its incomparability transitive, the original's not on most; the game's sort (0x10BF70 and 0x10C060 as grab_sorted_queries calls them, a 0 before the array) with the build's predicate compares only the array's queries and leaves 400 random batches of 17 to 58 sorted, while the original's leaves the array on some, and on a fixed batch of 26 reads of two archives (no parents, one priority) it calls the predicate with the 0 before the array, as in the dump; on 160 sets of one archive, or of loose files, whose queries of different priorities share their parent, the two predicates answer alike for every pair. **Not run:** a game.

### 3.32 The commit_suicide key kills one's own player (written 2026-10-17, README patch 34, the next release's revision) — PROVEN in the code, the builder's last call run with Unicorn; not run in a game

The owner's wish: a player can kill their own character in a match, to respawn and pick another kit quickly (the local weapon and scope tests too).

- **What `commit_suicide` is** (PROVEN). A game action, not a console command or a message: the action table 0xA74AF0..0xA74FB4 (20-byte entries `{name, action, kind, toggle, default key}`) has `commit_suicide` at 0xA74D20, action 1Fh, kind 5, default key `kK`; no other action has K by default. A key bound to an action reaches `player_input_handler::on_keyboard_action` (0x23FE00), which asks `key_binder::get_binded_action` (0x207D50) and pushes `{action, state}` into the handler's event buffer (+98h: the write and read indexes, then 65 pairs; push_back 0x242580); key down is state 0 (0x23FEA4), a repeat 1, up 2. So K puts action 1Fh in the buffer in the original.
- **Why nothing happened** (PROVEN). `player_input_handler::process_first_person_mode` (0x241250) builds the input's action mask (handler+33Ch, the `actions_mask` the relay carries) by asking `action_present` (0x242460: `(handler, action, &state)`, `ret 0Ch`; true for an action in the buffer, with its state, or held, state 2) for 35 actions, one bit each (the table in `drive.rs::key`). Action 1Fh is not asked, and bit 24 (1000000h) is the one bit of 0..28 it never sets.
- **What bit 24 does** (PROVEN). `base_player::process_quick_slots` (0x2D8BB0, called only from the input apply 0x2D86E0 at 0x2D8777, which every game runs for every player's input) begins with `test dword [edi+4],1000000h` (0x2D8BC1): with the bit it builds the bone name "brain" (0xA1067C), pushes two floats 1.0 and calls the player's hit receiver (+118h, vtable+20h = `base_player::hit` 0x2D9AE0, which calls `hit_impl` 0x2D9B90), the attacker the player itself (+110h); then it stops if the player is dead (+33Dh). `hit_impl` returns at once for a dead player. `on_player_killed` (0x2F4EB0) counts a kill whose killer is the victim (0x2F4F38) as a death (event 2) and a suicide (event 3), no kill; Team Deathmatch charges it one ticket (0x31614E, `rules.rs::tdm_death_costs_ticket`); the score weighs a suicide 0.
- **The patch** (`SUICIDE_KEY_HOOK` in `patch_026g.py`; the arena's block `suicide_key.code`: `suicide_key_026g.asm`'s `RUST_HOOK` entry and the Rust crate `rust/suicide_key`, the Rust framework's example, `rust_patches.md`; 40 bytes of NASM at `.srp`+980h before it): the builder's last call, `call 0x242460` for spot (0x242196; ebx the handler, action 22h and &state, the builder's [esp+44h], pushed), becomes `call suicide`. The code asks `action_present` for 1Fh with the same handler and state, sets bit 24 when it is there with state 0 (the key went down: one input, as spot), and jumps to 0x242460 with the original arguments and return address. With spot in the buffer that call writes the state again; without it (al = 0) the builder reads no state (0x24219D `jz 0x2421B0`). `action_present` only looks (`find` in the buffer and the held vector), so asking twice changes nothing.
- **Why every game does the same.** The bit is an ordinary input bit: the owner's game applies the input at once and the server relays it unchecked (0xDC/0xDD → 0xEB), as the fire or grenade bit; every game, of any revision, applies it through 0x2D86E0 → 0x2D8BC1 at the input's tick, from what the world holds. A game without the patch cannot send the bit but plays it from the others, so no revision is needed for the match to agree. The server must not take the bit out of a relayed input: the owner has applied it already (as §20 of `match.md` says for any bit).
- **Replays and recordings.** What a match computes does not change (the simulation is the original's; only the input a key makes does): no row in `simulation_changes.tsv`. The server's bots never send bit 24 (`drive.rs::key::NEVER`).
- **Rules.** A suicide is a death in the statistics and in Team Deathmatch a ticket, and no one's kill, in the game's own counting; the respawn time is a death's. No server guard: the server cannot drop the bit without parting the worlds, and the game already charges it.
- **Checked:** `selftest_026g.py` check 35 (the action table's entry, the builder's 35 actions without 1Fh and without bit 24, the consumer at 0x2D8BC1 and its one caller, the site, the code's calls and exits, no relocation under the site and nothing aiming inside it); `check_suicide_key_026g.py` (Unicorn): the game's own push_back and `action_present` with made-up buffers: K down sets bit 24 alone of the mask, K repeated, released or held does not, spot's answer and state are the original's, the buffer is untouched, the registers and the stack are as the call leaves them; the original never sets the bit. **Not run in a game**: that the 1.0 hit on "brain" always kills (a helmet could take some of it), the respawn and the kit pick after it, the options screen's listing of the key.

### 3.33 A baked camouflage texture missing from the baker's list is baked again (written 2026-10-17, README patch 35, the next release's revision) — PROVEN in the code and in the dump, the list run with Unicorn; not run in a game

A player's game (revision 16: the dump's PE checksum 0x00C5BE7F is that of the revision 16 build with `--server=skhid.link:1234`, sha256 `b937e86f…3ad0`; the builds of 30/31 have others) stopped while it loaded Rudnya: `int3` at 0x131C72 after the fatal assertion "Saved baked texture user_data/streamed_textures/edge_researcher_03_legs_s3d.dds not found" (`camouflage_decal_texture_type_baker_cook.cpp`, line 1482), main thread, from `resources_manager::dispatch_callbacks` → `on_query_finished` → `on_baked_texture_loaded`. The baker is not in the 0.10b source; the names below are the target's own diagnostics (`target-diagnostic-inferred.json`), the rest read from the code.

- **The list** (PROVEN). The baker cook is a static at 0xA264E80 (the dump: `ebx`); `buffer_vector<baked_texture_data>` at +230h (begin, end, capacity end; 1024 records of 124h inline): the path as a `buffer_string` (+0 begin, +4 end, +8 capacity end, +0Ch the text, 104h), +114h the CRC of the file's bytes (0x3C0F70 over the loaded resource; process_compression's CRC of what it baked), +118h size, +11Ch age (the counter 0x2260920, ++ at each record and each good load), +120h "a load is under way". +4923Ch the bytes of all records.
- **Its uses** (PROVEN). translate_query (0x12D1D0) makes `user_data/streamed_textures/<name>.dds` and calls 0x131ED0 (the first record of that path: marks it loading, true); then it queries the file with on_baked_texture_loaded (0x12DB40) as the callback; else it bakes (0x12DEE0 → on_textures_loaded 0x12E510 → process_compression 0x12EC40). process_compression gives the query the bytes to save (`save_generated_resource`: the resources manager writes them later on its device thread and only then hot-mounts the file, 0.10b `resources_manager_helper.cpp` / `resources_device_manager.cpp`), **push_backs a record at once** (0x132640, +4923Ch += size) and calls remove_unwanted_files (0x131F90). on_baked_texture_loaded, for a query that read the file, calls check_loaded_baked_texture (0x131B40, `ret 0Ch`: the baker, the resource, the path): the first record of the path; its CRC equal: age renewed, the loading mark cleared, true (the texture is made of the file); another CRC: the record is erased (0x132E40, the size taken off, "erase %s, id = %u"), false; **no record: the fatal assertion** (`cmp byte [226091Dh],0` / `jnz 131C73h`, then 0x113450 and `int3`). On false on_baked_texture_loaded logs "Texture %s is corrupted!" and finishes with `result_requery` (4; 0.10b `cook_base::result_enum`): the query is made again, finds no record and bakes. A query whose load failed finishes with `result_error` and keeps its loading mark.
- **remove_unwanted_files** (PROVEN, not this crash). It removes the oldest records not loading, and their files, while the records' bytes are over 1 GiB, or over 350 MB / 50 MB when the disk has less than 50 MB free. The dump's records hold 504 MB (304 records), so it removed none at its last bake (with less than 50 MB free it would have gone down to 50 MB). Its loop keeps the vector's end of before the first erase and steps over the record after each erased one: a latent fault when it removes many, not patched.
- **The dump** (PROVEN). No record of `edge_researcher_03_legs_s3` (d, n or s); 163 paths in 304 records, 141 of them a second to fifth record of a path (identical CRC and size: the bakes are the same bytes; two characters wearing one item, both looked up before either was baked). Behind the end, six stale copies of the last record: the list was six records longer once, as six erases leave it (check_loaded's, most likely: the 1 GiB limit was not reached). The game's last log line, at the same millisecond (13:52:02.023): "Texture user_data/streamed_textures/edge_researcher_03_legs_s3s.dds is corrupted!". So the s3 textures were each recorded twice and all loads of them read bytes of another CRC: two erases each, and the third load of s3d found none. 18 records are marked loading (the match's other kits).
- **Why the bytes differed** (NOT PROVEN: the dump holds neither the heap with the log nor the files). The read is of `user_data/streamed_textures/<name>.dds` as the resources manager has it: before this game's own save has been written and mounted, the file of an earlier game. Such a file can be other bytes when another game wrote it: Skhid's "Watch replay" starts a second `survarium_skhid.exe` (`-match_replay`, the single-instance check is patch 2) with the same Documents folder, which bakes and writes the same names. The player had watched two replays before.

**The patch** (`BAKED_TEXTURE_MISSING`, the builds with the late join): 0x131C1E `75 53` (jnz) → `EB 53` (jmp): with no record the function takes its own path for the assertion's "ignore" flag (0x131C73: the resource let go, `xor al,al`, `ret 0Ch`), the path every "ignore" of that assertion took. on_baked_texture_loaded then answers as for another CRC: "Texture %s is corrupted!", `result_requery`, a new bake. Nothing is erased (there is no record); the cmp before it stays and is unused; no relocation under the byte, no branch into it.

- **Nothing a match computes:** textures only.
- **Builds:** without a server sha256 `fb90f2b837dcd9998be940f353b478f45a69b112c8bae05dfc41a6c0d5cd3f63`, with `--server=skhid.link:1234` `0910f74aa46d888b1a3876a385948321b876c4dfe8f191abe4aaeb526a4f0652` (the revision byte REVISION's, 17; with patch 33). Against the builds of `main` d51efa1 without it, the default build differs in file 0x12101E (75h → EBh) and the PE checksum, and in nothing else; `--no-late-join` gives the revision 4 exe as before.
- **Checked:** `selftest_026g.py` check 36. `check_baked_texture_026g.py` (Unicorn 2.1.4) runs 0x131ED0, 0x132640 and 0x131B40 (with the game's erase and record copy; the CRC, the log filter, the assertion and IsDebuggerPresent stubs): two bakes of one path and three loads of other bytes stop the original at the assertion on the third load with the list as in the dump; the build makes all three again and bakes the path; a good load and one load of other bytes are the original's in both. **Not run:** a game.

### 3.34 The memory arena "ai" is 4 MiB (written 2026-10-17, README patch 36, the next release's revision) — PROVEN in the code and in three live dumps; not run in a game

A player's game stopped on 2026-10-17 at 14:24 UTC in a 20-player match: the fatal "not enough memory for arena [ai]" (`memory_doug_lea_allocator.cpp`, line 53; out_of_memory_with_crash 0xE4D70, `int3` at 0xE4EA5) on the cooker thread, from `weapon_core_cook::query_weapon_states` → the weapon's state machine setup (0x2DF780) → `ai::fsm::append_transition` (0x38C970). Two analyses of the original exe (sha256 `ac33d993…f153`) and three live crash dumps were read.

- **The arena's size** (PROVEN). A static initializer (0x1E8B50 .. `ret` at 0x1E8D9B) makes the doug_lea arena 0xA967D8 (vtable 0x9E68DC stored at 0x1E8C60; the global pointer to it is 0x1F1DA44) and registers it with 0xE21C0 (`vostok::memory::register_allocator`, an inferred name) in the arenas' list at [0xA808FC]: on its stack {allocator, ?, size as a u64, base, name}, the size `mov dword [esp+18h],40000h` at 0x1E8CCB (bytes C7 44 24 18 00 00 04 00; the imm32 at 0x1E8CCF, RVA 0x1D8CCF, file 0x1D80CF): **256 KiB**, the name `mov dword [esp+24h],9FA4E0h` ("ai", named in the code there alone). No option or setting is read in the initializer, so no command line or config can change it; no base relocation is under the immediate.
- **Its user** (PROVEN). Only `ai::fsm` allocates from the arena: `append_transition` (0x38C970, 0x28 bytes a transition), `pop_state`, `generalized_action`. Every weapon of every player object sets up its state machine there (0x2DF780).
- **Why Skhid fills it** (INFERRED from the dumps and the kit change): a match has one player object for each usable kit, three a player since #148, so a 20-player match has some 60 player objects' weapon state machines in the arena; the crash came when one more was set up. How much one machine takes was not measured; 256 KiB was enough for the matches before #148.
- **The room for it** (PROVEN): the other arenas reserve about 1835 MiB together and the exe is large-address-aware; 3.75 MiB more is negligible to the address space.

**The patch** (`AI_ARENA`, the builds with the late join): 0x1E8CCB `C7 44 24 18 00 00 04 00` → `C7 44 24 18 00 00 40 00`, the size 400000h (4 MiB, sixteen times the original): one byte, file 0x1D80D1, 04h → 40h. The rest of the initializer is the original's.

- **Nothing a match computes:** the arena's size only; a game with it and one without play together.
- **Builds:** without a server sha256 `bbcf9a8506336d4d436cb0f7f6b5bfa8bbeee2f58ca00bece022746b7748f6de`, with `--server=skhid.link:1234` `eb6e25277348bb3fc0fd9fdc338126e04f386692ae154dfe1b02763a16420363` (the revision byte REVISION's, 17). Against the builds of `main` 838cce1 without it (`fb90f2b8…3f63`, `0910f74a…0652`), they differ in file 0x1D80D1 (04h → 40h) and the PE checksum, and in nothing else (compared byte by byte; also with `--revision=18`); `--no-late-join`, `--cave=hash-mismatch` and `--cert-only` give the exes of before.
- **Checked:** `selftest_026g.py` check 37. **Not run:** a game.

### 3.35 Spotting: no cooldown, a battery's body spots it (written 2026-10-19, README patch 38, the next release's revision; the rules: `spotting.md`) — PROVEN in the code, the input apply run with Unicorn; not run in a game

The owner's report: "battery marking sometimes works, sometimes doesn't"; the decision: no spot cooldown at all, and a hit on a battery's body counts.

- **The spot input** (PROVEN). The input builder sets bit 28 of the action mask when action 22h (spot) is in the event buffer with state 0, the key going down (0x24219F `cmp dword [esp+44h],0`, 0x2421A6 `or dword [ebx+33Ch],10000000h`); a repeat (1) or a held key (2) sets nothing. One press is one input with the bit. `base_player::apply_new_input` (0x2D86E0, thiscall `(tick, &input)`, `ret 8`) returns at once for an input whose actions and mouse moves are those it applied last (0x2D86FB..0x2D8741).
- **The spot** (PROVEN). With bit 28 and the spotter's `m_time_left_to_try_spot_in_ms` (+34Ch, the name of base_player::serialize's field at 0x2DB34D) at 0: the wait is set to 3000 (0x2D87C1 `mov dword [edi+34Ch],0BB8h`) before the ray is cast, so a miss waits too; the ray: the collision world (+350h) slot 13h (`(eye +47Ch, direction +46Ch, 100.0 [0xA710D0], &hits, 0FFFFh, 80Ah)`, `ret 18h`). The hits (28h bytes each, +1Ch the hit object) are walked in order with their groups or-ed into [esp+10h]: a group with 800h (spottable) asks the object's owner (+0Ch) for a player (slot 2): its spottable interface (+11Ch, vtable 0xA10B64: `set_spotted` 0x2DC0B0 puts 10000 into +348h, `can_be_spotted_by` 0x2DC0C0 says yes for another team) is asked and spotted (0x2D8862..0x2D888D); no player: at 0x2D8A0F the owner's slot 1, a geometry whose first spottable (0x2DC390 with 0x2DD760) is asked and spotted the same way (0x2D8A50..0x2D8A65). Not spotted: 800h out of the mask (0x2D8A8F), the next hit. A hit without 800h ends the walk when the mask has both 2h (walkable) and 8h (hittable) (0x2D8A9D). The end 0x2D888F frees the hits.
- **The wait** (PROVEN). The player's tick counts it down by the tick's time, to 0 at the least (0x2DAB44..0x2DAB69); a spawn sets it to 0 (0x2DAE6B); base_player::serialize saves it (0x2DB34D → 0x1F48A0) and the read puts it back (0x2DB95C). It is part of every game's world.
- **The battery** (PROVEN). `victory_item_core` (RTTI) has the spottable interface at +54h (vtable 0xA13EE0: `set_spotted` 0x301C40 ors the spotter's team bit, 1 or 2 by [spotter+6B8h]+45ECh, into +1A4h and calls the match's listener ([world+18FA8h], slot 3, `(place, kit, 4, item)`); `can_be_spotted_by` 0x301CA0 is `mov al,1`) and at +58h the owner of its rigid body (0x301CC0 puts item+58h at body+0Ch; vtable 0xA13EEC, stored by the two constructors alone, 0x30011D and 0x3004EA; slots 1 and 2 are both 0x300040 `xor eax,eax; ret`). The body's group is 0A18h with filter 418h (0x301CF5): 800h | 200h | 10h | 8h, spottable and hittable. Its use box `!victory_item_collision!` (group 880h) has an owner whose slot 1 is a geometry with the battery's +54h. So a ray that meets the body first finds no player and no geometry, spots nothing, leaves 8h in the mask, and a floor (2h) behind it ends the walk before the use box.
- **What listens to a spot of a battery** (PROVEN). [world+18FA8h] is set only by the statistics handler's start_match (0x26D8E7; base_game_statistics_handler and the class after it, slot 3 0x26DAA0 → 0x2F5570 → 0x334AC0). For event 4: when the battery's record has no spotter (+FCh = FFh) it awards the spotter (0x2E55B0, type 13h, at 0x334B32) and records them; then the battery's progress marks (0x334810: each team's farthest point along the line between the bases, the gain added to the (place, kit) of the event). A battery lying still gains nothing; nothing else is done with the event.
- **The patch** (`SPOT_COOLDOWN`, `SPOT_HOOK`, `spot_026g.asm` at `.srp`+7C0h, 50 bytes, after the shared upgrades' code, before the pickup code; the builds with the late join): 0x2D87C1 `mov dword [edi+34Ch],0BB8h` → `...,0` (two bytes, file 0x2C7BC7): the wait is 0 at every tick, in every game and snapshot, and every press casts the ray. 0x2D8A0F `mov ecx,[esi]; mov ecx,[ecx+0Ch]` → `jmp body`: the code does those two, and for an owner with the vtable 0xA13EEC takes the interface at owner−4 (item+54h), asks `can_be_spotted_by` and calls `set_spotted` through its vtable with the spotter, as 0x2D8A50..0x2D8A65 do for the use box, and ends the walk at 0x2D888F (not spotted: 0x2D8A8F); any other owner goes on at 0x2D8A14 with ecx the owner. esi is free on both ways out and kept by the calls. One absolute value (the vtable's): a rebased image would not match it and plays the original there. No relocation under either site, nothing aims inside them.
- **Pressing again.** A battery one's team already sees: its bit stays, the event is raised again (as the stock game after its 3 s; the award only for the first spotter). An enemy: 10 s again. No guard against a spot again was added: it awards nothing and marks nothing new.
- **Revisions.** Every game runs every player's spot, and the wait is in the world: a game with the patch and one without part at the first spot. `simulation_changes.tsv` has its `next` row; a server asks for the release's revision.
- **Builds:** without a server sha256 `af70c0edde39a2e63ddc1da0ff9ebdd74741ef2bafc163ba88add1b901b06242`, with `--server=skhid.link:1234` `7d887d1a3ee794550a1263f4308cb0dbf5f6f9bd8a0c4ff630485d98b0de2b36` (the revision byte REVISION's, 23). Against the builds of `main` 7cb1e45 (revision 23's `bccb7d7b…a9b2`, `d4154dbe…186e`) they differ in file 0x2C7BC7..0x2C7BC8, 0x2C7E0F..0x2C7E13, 0xC4B1C0..0xC4B1F1 and the PE checksum, and in nothing else (59 bytes; compared byte by byte).
- **Checked:** `selftest_026g.py` check 38; `check_spot_026g.py` (Unicorn 2.1.4) runs 0x2D86E0 to 0x2D8991 and the countdown with the game's own 0x2DC0B0, 0x2DC0C0, 0x301C40, 0x301CA0 and 0x300040 on made-up hits, the original and the build. **Not run:** a game; which hits the real collision world returns for a battery, and in what order.

### 3.36 The Tab list shows the places in the match, none whose player left (written 2026-10-19, README patch 39, the next release's revision; the rules: `tab-list-left-places.md`) — PROVEN in the code, the rows and the lists made again run with Unicorn; not run in a game

Seen live since the rooms (#203): a player who left a running match (Leave match, 0xE0) and came back into it as a new place had two rows in the others' scoreboard, the left one greyed. The owner: "once they leave their previous entry should be removed".

- **How a leave reaches the other games** (PROVEN, the server's code and the exe): `Game::leave` (match-server `game.rs`; for the leaver's 0xE0, a closed link, `SRP_MATCH_CLIENT_TIMEOUT_SECONDS` of silence, 10 s by default, or a new connection for the place) sends every other live game 0xEA `{tick, place}` for a tick 300 ms ahead (`roster_change`, `ROSTER_LEAD_MS`; a game in a resync gets it with its replay). The client's 0xEA handler (`network_client::process_player_left_match` 0x27C690) gives it to `game_world_core::leave_match` 0x2D18A0, and at that tick `game_world_core::on_player_left_match` 0x2D2040 calls slot +24h of every rule of the core (the vector at core+18824h..+18828h, 0x2D2119..0x2D2148); `kd_stats_rule::on_player_left_match` 0x323A00 clears the place's `online` (rule+114h + 8 × place; vtable 0xA177D4, whose +20h, on_player_entered_match 0x3239F0, sets it on 0xE9). It is in the world (snapshot_rules.md §2.7), so a late joiner or a resynced game has it too, and every game has it at the same tick. `kd_stats_rule::fill_match_hud_state` 0x323980 (+44h) copies kills, deaths and `online` of every place into the hud's state at +98h + 8 × place, the state being hud+24h..+2D8h (`update_ui` hashes it, 0x23B2D2): `online` of place i is hud+0C0h + 8i. A lost link and a leave look the same to the other games: one 0xEA, after 10 s of silence or at once; a lost game that comes back to its place is entered again (0xE9) and `online` is set again.
- **What the stock game did with it** (PROVEN): `game_world_ui::initialize_match` 0x234880 (once at the match's start, again after a late join's install, §3.13) puts a row for every place below the count into the scoreboard's array by SetElement (the value interface's vtable+34h, index = the place, 0x23537A..0x23538A), and nothing ever takes one out. `update_ui` 0x23B200 (the name LIKELY: 0.10b's `game_world_ui::update_ui` loops over the places while `m_players_list_visible` and calls `set_player_online_status`, as this does; 0.10b's `initialize_match` skipped places of no team and put the rest in at a running index; the label is new in `tools/re-kit/v026g/inferred.tsv`), while the list is shown (hud+0BAh) and the state changed, calls `set_player_online_status` 0x23A7F0 (`root.set_online_player(name, online)`) and `set_player_stats_info` 0x236D30 (`root.list_update_player`) for every place below the count (0x23B42C..0x23B475). In the movie (`hud.swf`, `PlayerList.set_online_player`, `PlayerListRenderer.updateData`) a row whose `online` is false gets the name colour 0x999999: a left place stayed as a grey row, and a place not yet entered too. `set_online_player` finds the row by its name, so with two places of one name the old place's call set the new place's row (the first of that name) and the new place's call set it back.
- **The patch** (`TAB_LIST_HOOKS`, `TAB_LIST_LOOP`, the builds with the late join; its code in `late_join_026g.asm`, with no C change, so `late_join_026g.section.bin` differs in its first 4096 bytes alone, assembled by NASM 2.16.03, the C part kept):
  - `tab_row` (late-join page +2480): 0x23537A's 16 bytes (`lea eax,[esp+58h]` / `push eax` / `mov eax,[esp+138h]` / `push ebx` / `push eax` / `call edx`) become a call of it and nops. It appends the row (PushBack, vtable+3Ch, the call the enemies' ids already use at 0x234F2D; the interface's order is Scaleform's `Value::ObjectInterface`: +34h SetElement, +38h VisitElements, +3Ch PushBack) when the place's hud byte is not 0, so the array is dense and in place order; and it sets the state dword (`.srpd`+0E40h, `TAB_LIST_STATE_OFFSET`) to -1 at every row: a list was made.
  - `tab_shown` (+3600): 0x23B421's `cmp byte [edi+0BAh],0` becomes a call of it and two nops; the `jz 0x23b477` after it stays and takes the flags it leaves. While the list is shown it makes the mask of the places in the match (the hud's bytes, for the count the loop after it reads, at most 20) and, when the mask is not the dword, calls `rebuild_lists` (§3.13) with the hud and the hud's own options (hud+1Ch) and stores the mask after it. `lists_hook` (+3456) ends a run of the lists whose return address is install_context's or tab_shown's.
  - The loop's body 0x23B447..0x23B476 is rewritten in place (48 bytes, the same calls): a place whose hud byte is 0 is skipped, no `set_online_player` and no `list_update_player` for it; list_update_player gets the place by `push ebx` (it reads the low byte alone, `movzx esi,byte [esp+44h]`, the byte the original stored at [esp+14h] and pushed as a dword).
  - No site holds an address, no base relocation is under them, and the code finds the state page from its own address.
- **What changes for a player** (the spec has the list): a left place's row goes; a place not yet in (loading at the start, a newcomer being prepared) has no row until it enters; a lost player is listed until the server's timeout, then not, and again if it comes back to its place. The second list (the replay bar, `root.set_players_list`), the enemies' ids and the results screen keep every place. Nothing a match computes: games with and without the patch play together, and no server rule asks for it.
- **Not done: a lost player greyed for the rooms' ~90 s.** The games cannot tell a lost link from a leave: both are one 0xEA, and nothing a game has says that the server keeps the seat. It would take a UI-only message from the server (a mask of the places away with a seat kept, sent at each change and with every sync), a hook in `on_match_packet_received` for it (the dispatcher skips an unknown id, 0x27A5AB, so stock games ignore it), the mask in `.srpd`, and `tab_row`, `tab_shown` and the loop listing those places too (the stock `set_online_player` greys them). Not built.
- **Builds:** without a server sha256 `c7ce9fdf778c67b1c417bbb86ef79ddbb6cc17e3112836ab76bb60ae6fe6e3ff`, with `--server=skhid.link:1234` `d5820608283c2f520f4af87f50e3ffb407bfe366bdeaf16f7072fb0c506d4d6d` (the tool's REVISION, 23). Against the build of `main` 7cb1e45, 67 bytes more differ before `.srp` (942 in 92 ranges, against 875 in 89); `.srp` differs in the late join's assembly.
- **Checked:** `selftest_026g.py` check 39 (all 1496 checks pass, NASM 2.16.03); `check_tab_list_026g.py` (Unicorn 2.1.4) runs 0x23B421..0x23B477 with the game's own 0x234880 list code on made-up huds, the build and the original; `check_revision6_026g.py` (the lists made again, now with every place in the match) and `check_late_join_026g.py` (the late-join code from the sources) pass. **Not run:** a game.

### 3.37 A model's LOD and screen factor at a wider field of view are the original's (written 2026-10-19, README patch 40, the next release's revision; the note: `fov_lod.md`) — PROVEN in the code and the data, the projection, select_lod and the screen factor run with Unicorn; not run in a game

The report: "with a high FOV some models disappear" (patch 22's `uro_fov`, 50 to 100).

- **The frustums** (PROVEN). `camera_director::apply` (0x243440) takes the camera's slot 0 (0x242900, `fov_projection` in the build) and gives it to `scene_renderer::set_projection_matrix` (0x21B50) and the render world (0x118300, scene+42Ch); `renderer_context::set_scene_view` (0x149A90) makes p (+4F58h) and vp (+5098h) of it (`set_p` 0x14A6F0). `stage_visibility::frustum_culling` (0x1B74D0), the portal sectors (0x1C5F20), the grass, the particles and the HiZ occlusion (0x1D7AA0 copies v, p and vp) all use that vp: nothing is cut at the screen's edges. The 65-degree constant 0xA2D9EC has one reader, 0x24293A.
- **The LOD** (PROVEN; the 0.10b source is `render_model_static.cpp`, the same code). `select_lod` (0x16EEC0, `(instance, vp)`, eax the eye, `ret 8`): no LOD1, no choice; type 0 compares the distance with param0..2 (default 20/40/240, 0xA7A808); type 1 transforms the box's eight corners by vp and compares half the larger NDC extent (`mulss xmm0,[0x927A44]`, 0.5, at 0x16F2A4) with param2, param1, param0 (default 0.4/0.2/0.001): under param2 it returns 3, and `get_surfaces` (0x16F390) gives no surfaces for 3 (0x16F46A). In retail `resources.db`: 952 of 1532 static models have LOD1, 281 of type 1, 20 with a param2 of 0.01 to 0.2 (README patch 40 names them). London (patch 14): 110 of type 1 with param0 0.2 to 0.9, whose LOD1 meshes drop whole surfaces: the case the owner saw (2026-10-20), the LOD0 -> LOD1 switch, fixed by the select_lod correction alone (`fov_lod.md`). LIKELY: LOD 3 usually draws LOD 2 or 1 after all (`get_surfaces` 0x16F3E6 falls back while a LOD's count is 0; LOD 3's count is never written by the cooker, in memory dlmalloc does not zero).
- **The screen factor** (PROVEN). `calculate_screen_factor` (0x158D80, esi the surface, edi the eye, `[esp+4]` p; called only by `scene::calculate_screen_factors` from `stage_visibility::execute_stage` 0x1B6B99 with p) projects the box through a look-at and p and stores the share of the screen it covers (at most 1) times 100 (`fmul dword [0xA710D0]` at 0x1590AF) at +58h (x 6 for some materials). Read by the shader LOD (0x158D20, inlined at 0x177449: 0.75/0.5/0.25), the sun's casters (0x176340 from 0x1752B8, per-cascade thresholds), spot shadows (0x190E49), rain occluders (under 10, 0x17C7ED) and LPV.
- **Stock** (PROVEN). The stock angle is 65 x z, z at most 1 (0x1F3480): the thresholds never met a wider view. At 65 the build's matrices are the original's.
- **The patch** (`FOV_LOD_HOOKS`, `FOV_LOD_STATE_OFFSET`; the builds with the late join): `fov_projection` keeps k = max(1, tan(A/2) / tan(65 z/2)) at the state page's +E50h (after the Tab list's dword at +E40h, §3.36; two `fptan`, the clamp `fcmovbe`, the store, before the matrix builder's call; the angle it passes is bit for bit patch 22's). 0x16F2A4 (8 bytes) and 0x1590AF (6) become `call` + nops to `fov_lod_scale` (late join +6E0h) and `fov_screen_scale` (+700h): the original instruction, then by k when k is not 0, `ret`. Their relocations (0x16F2A8, 0x1590B1) are disabled.
- **Revisions.** Rendering only: no match computation reads a LOD or a screen factor. No `simulation_changes.tsv` row; games with and without it play together.
- **Builds:** without a server sha256 `19ac89832023349fc20688c8b9b3635364298c5056da54af310c68af0f189ceb`, with `--server=skhid.link:1234` `4e9babea7d2b4e3c9ac08bc656a69805b5354d4833e071b0aa96498424c86548` (the revision byte REVISION's, 24). Against the builds of `main` bee9dc3 (`13084277…566c`, `0384d1ec…063c`) they differ in file 0x1484AF..0x1484B4 and 0x15E6A4..0x15E6AB (the two sites), 0xB71E47 and 0xB728AD (the two relocation entries), 0xC4BC60..0xC4BCED (fov_projection) and 0xC4C0E0..0xC4C115 (the two scales) and the PE checksum, and in nothing else (compared byte by byte).
- **Checked:** `selftest_026g.py` check 40 (and checks 15 and 23); `check_fov_lod_026g.py` (Unicorn) runs the projection (k at 50 to 100 and five zooms), select_lod on a Kamaz-sized box (out at 113.7 m at 65 in the original, at every setting in the build; 87.4 m at 80 and 62.5 m at 100 with k zeroed) and the screen factor (equal to the original's at 20, 60 and 150 m). **Not run:** a game.

### 3.40 The player's own row in the Tab list is gold (written 2026-10-20, README patch 43, the next release's revision; the rule: `tab-list-left-places.md`) — PROVEN in the code and the movie, on_activate's end run with Unicorn; not run in a game

The owner: "when pressing tab in game, your own character should be gold", "just like when you kill or get killed in a kill list top right".

- **The colour** (PROVEN, `flash_movies/hud.swf` of `resources.db.patch_22`, the last patch with the movie, decompiled with FFDec 25.1.3). `survarium.constants.Colors`: `PLAYER = YELLOW = 15852869` (0xF1E545), `PLAYER_ST = "#F1E545"`; the lobby movie's `Colors` is the same. The kill list: `CombatLogItem._colorCode = [OTHER_PLAYER, ENEMY, PLAYER]`, indexed by `who_team` / `victim_team`; `game_world_ui::on_player_killed` (0x237C70) sends 2 for the player's own place (`cmp [esp+13h],cl` / `mov esi,2`, 0x237EAA and 0x237FE2), else 1 for the other team and 0 for one's own (`[+45ECh]`). The Tab list: `PlayerListRenderer.updateData` sets the name's colour `Colors.PLAYER` when the row's `isLocal` is set (in both branches, with and without a squad icon) and puts a white 15 % background behind it; `PlayerList.addPlayersList` and `set_local_name` set `isLocal` on the row whose `name` is the name `HUD.set_local_player` gave (and select it). The same colour as the kill list's, from the same constant.
- **Who gives the name** (PROVEN). `root.set_local_player` (0x9FE534) has one caller, `chat_handler::set_local_player_name` (0x209AC0), which invokes it on the chat handler's movie. Its callers: the lobby client's 0x8F (`lobby_client::on_message_received` 0x274A00, table 0x275138; the account's name, lobby client +1E55D0h), sent at the sign-in (lobby.md section 2.15), when the chat handler's movie is the lobby's (LIKELY), and `game_world::on_activate` (0x1F9340) after `chat_handler::set_mode` (0x209430) has given the chat handler the game world's movie (world+204h, `root.chat`), the hud. on_activate does it when the game's network client's vtable 0x9FFB14 +20h says so (0x372420, `mov al,1`, one body that identical-code folding shares with other functions; what the slot means is not established), with the lobby client (+3Ch, 0x219960, network client +1E0h): "[%s] %s" (0x9FB140) of its clan (+1E5650h, at most 14 characters) and name, or the name (0x1F53D0), as `initialize_match` makes a row's name from the place's clan (+48h) and name (+8) (0x23506F..0x2350D8). Both come from the server's one account (`wire::account_body`, `play::match_profile`: name and `clan_tag`).
- **Why no row was gold** (PROVEN). The call at 0x1F9498 is under `mov eax,[0A8394Ch]` / `test eax,eax` / `jz 1F949Dh` / `mov ecx,[0A83948h]` / `cmp dword [ecx],1` / `jnz 1F949Dh`: the journal's flag, set by journaling's start (0xEB310, `xchg` at 0xEB37C) for a use other than 0, and its use (1 record, 2 replay; §3.10). The stock default recorded, so the hud had the name; patch 14 (revision 4, 2026-10-13) made the default "0": since then `PlayerList._playerName` is "" in every match and no row is `isLocal`.
- **The patch** (`TAB_SELF`, every full build: with and without the late join and over on_hash_mismatch, as patch 14's default): 0x1F9478 `85 C0` → `EB 0D`, `jmp short 1F9487h`. The two addresses (relocations at 0x1F9474 and 0x1F947E) are not under it, nothing aims at 0x1F9479. The movie is not changed.
- **What changes:** in a match the hud's PlayerList and chat get the player's name at on_activate, as with the journal on: the own row's name gold (0xF1E545) on a lighter background and selected, in both teams' lists, also after the lists are made again (patch 39, late joins: `addPlayersList` marks it by the kept name). The match chat's `GameChat` styles lines whose name is that name as the player's own (which name its lines carry was not checked). Rendering only; no simulation change.
- **Builds:** without a server sha256 `2ac4a26b2dbc286f41f95fcea2a24e8827d802bf448778b19c7d6b280f4380fa`, with `--server=skhid.link:1234` `6c6dfe49fb3e74f9824ca2991454752205469b76f983ef7458c7fd5d7c7fd659` (the revision byte REVISION's, 25). Against the build of `main` b97133d (`12006dd9…a8b4`, revision 25's) the first differs in file 0x1E8878..0x1E8879 and the PE checksum, and in nothing else (compared byte by byte).
- **Checked:** `selftest_026g.py` check 43 (all 1584 checks pass, NASM 2.16.03); `check_tab_self_026g.py` (Unicorn 2.1.4) runs 0x1F9395..0x1F949D with the game's network client (+20h, lobby_client), lengths, memcpy and 0x1F53D0 (stubs: _snprintf, set_mode, set_local_player_name): journal off, the original gives no name and the build "[SRP] sheep" / "sheep"; recording, both "[SRP] sheep"; played back, the build alone; +20h saying no, neither calls; the builds with and without the late join and over on_hash_mismatch give the same; the stack balanced. **Not run:** a game.

### 3.41 A weapon wears the camouflage its item holds (written 2026-10-20, README patch 44, the next release's revision; the rules: `weapon-camo.md`) — PROVEN in the code, the site and the code run with Unicorn; run in a game (the lobby), not in a match

The owner's request: the armour camouflages on the regular weapons, painted where the premium weapons are.

- **Why weapons stay plain** (PROVEN). Only three places make a camouflage_decal_cook_parameters (`type_to_int<P*>` 0x119590 has three producers): the profile's visual for the character's seven parts (0x204560: `parameters/camo_available` and `data/camouflage` of the part's merged config, then 0x129830 asks for a class-6Ch bake per flagged part), the lobby's decal editor (0x2641A0, one part of a model on screen, callback 0x2648E0) and skeleton_model_instance_cook's bake (0x1268C0), dead: the variant type it tests is never set. weapon_cook (0x24A920 → 0x24ACF0) queries `object/model` as class 10h with no user data and asks for no bake. The weapon's merged config (0x30A880 → 0x2F14C0) holds its upgrades as for armour.
- **What a bake needs** (PROVEN). The baker (0x12D1D0) takes surface `[P+794h]` of `get_all_surfaces([P+790h])` (0x159700) and its gbuffer material's textures; the camouflage layer is P+520h/+5ACh/+6C4h (diffuse, `_nmap`, `_smap`; fresnel never), its scale P+798h (4.0, [0x9E4EB4]); the `<diffuse>_aoac` layer must load or the type is refused, silently (0x12E8E9). The baked files are `user_data/streamed_textures/<P+7A4h><d|n|f|s>.dds`. P+7A0h is 1 in the editor only.
- **A camouflage's data** (PROVEN). The camouflage item's modification is a type-3 modifier at path `data` (`camouflage`, `<texture>`); 0x2F1430 finds the path in the host config or drops the modifier: a weapon config needs a `data` table (the data patch adds one).
- **The patch** (`WEAPON_CAMO_HOOK`, `weapon_camo_026g.asm` in `.srpw` at 0xA42A000, 739 bytes since patch 47, which looks up and fills what is painted: section 3.44): at 0x24B0D2 (the model loaded, `[esp+20h]`, the merged config `[esp+3Ch]`) `call weapon_ready`, which asks for the editor's bake for each surface (at most 8) with the weapon's camouflage, `data/camo_scale` or 4.0, the name `srpw1_<camouflage>_<object/model>_<surface>`, the game's allocator and no parent query; `bake_done` (through boost's bind invoker 0x127700, manager 0x9ED148) releases the render instance (0x11540) and frees P (0x1539D0), as 0x2648E0. Details: README patch 44.
- **Checked:** selftest check 44; `check_weapon_camo_026g.py`. In a game (2026-10-20, local): an AK-12 and an Uzi with the City camouflage in slot 2 baked their three textures and showed them in the lobby. **Not seen:** a match, a replay; the cost of many bakes at a match's start.

### 3.42 A weapon's upgrade window shows the weapon in the air above the workbench (written 2026-10-21, README patch 45, the next release's revision) — PROVEN in the code and the scene's data, the handlers, the camera's tick and mt_draw's two sites run with Unicorn; not run in a game

The owner's request (2026-10-21): "instead of focusing on players the camera would flow to the working table and the 3d model of the weapon would have been there", "that would allow you to rotate it etc", "with weapon in the air". Patch 32 (section 3.30) still decides what is shown and has the character hold it; this draws that held weapon on its own above the workbench and flies the camera there.

**The lobby's scene** (read 2026-10-21 from `resources.db`; PROVEN unless tagged):

- The lobby menu loads the project `lobby_scene/day`: its resources query (0x21E450, the list at 0x21E594..0x21E5C5 given to query_resources 0xE5B60 at 0x21E5ED) asks `sound_scene`, `lobby_scene/day` (class 40h), cursor.swf, lobby_menu.swf, `default.player` and `static_game_parameters.options`; `projects/lobby_scene/day/client_project` and `.lua` are in the stock db, in no patch. Its static models are `level/lobby/{base,door,electric1,pipes,celling,earth}` at the origin and about 90 props. There is no table object of its own: the **workbench** is the surface `bench_gi_s` of `models/level/lobby/door.model` (mesh `|door|vis|bench|benchShape`, texture `blackout/lobby/bench_gi`), read from its `converted_model` (u32 3, u32, u32 vertex count, 28-byte vertices with the position first, then u32 4, u32 bytes, u32 index count, u16 indices): its top at y 0.90..0.91, x 3.75..4.88, z 1.70..4.60 (a long bench along the right wall, x ≈ 5.1), legs at both ends; on it a drill press (`electric1.model` surface `drill_s`, x 3.82..4.64, z 1.89..2.40, up to y 1.94) and a vice and tools (z 3.5..4.6, up to y 1.19); the weapon lockers (`door.model` `wp_locker_s1`) at z −1.5..1.23 beside it. A bayonet `weapons/6x5` lies at (3.04, 0.73, 1.56) in front of it. Between the drill press and the vice (z 2.5..3.5) the bench's top holds only flat things (y ≤ 0.95).
- The lobby's own character (lobby menu+694h) is made at (0, 0, 0), yaw π (0x21CCD2 → 0x284730: position from eax, yaw +14h = π); the squad panels' at (1, 0, 0.5) and (−1, 0, 0.5) (0x21CD33, 0x21CD94).

**The lobby camera** (lobby_camera, lobby menu+10Ch; PROVEN, emulated):

- Fields, from the camera: +98h side angle target, +9Ch height target, the **point it looks at** +A0h/+A4h/+A8h (x, the current height, z), +ACh current side angle, +B8h yaw, +BCh pitch (clamped ±1.047 by the tick, 0x2840C5), +C0h distance target, +C4h current distance, +C8h the wheel's move, +CCh and +D0h the drag's yaw and pitch of the frame, +E0h..+E8h the speeds the setter sets (|new − current| · 1.111 per second). The ctor 0x283BC0 (eax = camera, ecx = the lobby menu (+88h, +B4h), [esp+4] = the collision scene (+B0h), `ret 4`) writes +A0h and +A8h (0); nothing else writes them (every `[reg+0A0h]`/`[reg+0A8h]` write of 0x260000..0x2A0000: the tick's are of ebx = camera+4, the height and the side angle). So the stock camera always looks at (0, height, 0): the character.
- The tick 0x283EF0 (vtable 0xA09ED8 slot 4, ecx = camera+4): dt = the lobby menu's time ([+9Ch]+584Ch, ms) minus the last (0x2260AEC), at most 0.1 s, kept at [esp+10h]; height and side angle stepped to their targets (stopping within 0.001); 0x284020 adds +CCh to the yaw and +D0h to the pitch; the wheel moves the distance target (1.5..4.6); the camera's world matrix (+48h, rows right, up, forward, eye; the game's matrices are row-vector, v · M; rotY 0x15AE50 and rotX 0x1BE3A0 with esi = the out matrix and the angle on the stack, the affine product 0xEE700 out = edx · ecx) has the eye = point − distance · forward with forward = (−sin yaw · cos pitch, sin pitch, cos yaw · cos pitch) (emulated: yaw 0.5 → eye (0.94, 1.2, −1.72) from the point (0, 1.2, 0) at 1.96), shortened by a collision ray from the point (0x2844D0, the collision scene's vtable+48h); the side angle turns only the view (emulated: 0.37 turns the view by −0.37 about y, the subject left of the screen's middle). Then 0x21DF00 (eax = the lobby menu) projects the character's feet and head to the screen for the movie's `root.set_player_region` (the drop region over the character, `dragRegion`, which the upgrade window disables).
- The drag (on_mouse_move 0x283D30, vtable 0xA09EB4 slot 3, priority 10): while a button of 52h is held, −dx·(π/180)·0.1·[0xA74AEC] to +CCh and −dy·… to +D0h; the wheel −0.001 a unit to +C8h unless the lobby menu's vtable+24h answers true. 0x283EB0 (slot 5) zeroes the three at the input's end of the frame. The character's input handlers (vtable 0xA09F6C) act only in the decal editing mode (+D514h), so they leave the drag to the camera in an upgrade window. **The camera can look at any world point**: write +A0h and +A8h (and the height by the setter); the yaw and pitch set the side it is seen from.

**A lone weapon at a transform** (PROVEN): the stock lobby never shows an item on its own (section 3.30), and a new render instance would need the scene's add_model (0x21CD0), a weapon_cook query and its release. It is not needed: the character's player draws its held item every frame in player::mt_draw (0x2474A0; called by lobby_character's update 0x2849F0 at 0x284A7E, return 0x284A83, and by the match's 0x1F7DCE alone) with a transform it computes: at 0x2477C8 eax = the transform, then the item's vtable+48h — for a weapon `weapon::draw` 0x244E10 ([ebp+8] the transform, kept at +9BCh; its two locators by calculate_locator 0x243BC0 at +620h/+660h; scene_renderer::update_model 0x22280 for the model and the scope, +1808h, at the locator; update_skeleton 0x22490 with the bone matrices). The transform is the hand's (0x247763: the "weapon" bone of the skeleton times the player's +40Ch, [ebp−0A8h]) or, in the decal pose (+F49Ch), translate(0, −10, 0) built by 0x573C0 in [ebp−0E8h] (that buffer used nowhere else in mt_draw). So any transform there draws the weapon there, scope and camouflage with it; the weapon's model is placed nowhere else (update_model's callers for a weapon: weapon::draw and weapon::on_culled_player_draw 0x245250 alone). One catch: a player not seen last frame (+F4E2h, [model+110h] below the renderer's frame) and not the world's own ([ebp−1]) with +12Ch set takes the culled path (0x24757C → 0x247811: on_culled_player_draw puts the weapon at the player's root); the camera at the bench does not see the character, so the patch sends its player to the draw path.

**The movie's calls** (PROVEN, section 3.30 and `LobbyMenu.as`): the upgrade window's open calls `start_item_upgrade(itemId)` (ExternalInterface, callback 5 → 0x29A870 → 0x265DE0 → the setter at 0x265E73, patch 32's start_cam) and its close `exit_item_upgrade` (callback 6 → 0x29A890 → 0x265F10 → 0x265F44, exit_cam); patch 32's refresh after an attach (a scope, a camouflage) rebuilds the character's player, whose held weapon is drawn by the same mt_draw, so the bench shows the new weapon without a call of its own.

**The patch** (`WEAPON_TABLE_HOOKS`, `weapon_table_026g.asm` at `.srpw`+800h = 0xA42A800, 812 bytes, after the stash code (+360h, 408 bytes; room left for it to grow); its state, 40 bytes, at `.srpd`+F00h = 0xA428F00: the table, the flight, the targets x, z, yaw, pitch, the camera's yaw and pitch kept, the weapon's spin and tilt). It begins with four `jmp near`, its entries (a fifth, `left`, in round 2 below):

| site | original | patched | code |
|---|---|---|---|
| patch 32's start_cam and exit_cam ends (`.srp`+2EB8h, +2EF8h) | `jmp 2845C0h` | `jmp 0xA42A800` (`-DCAMERA`) | `camera`: patch 32's mode 1: the table on, the targets (BENCH x and z, −π/2 the nearest turn to the camera's yaw, pitch −0.25), the camera's yaw and pitch kept unless already flying, the spin and tilt 0, xmm0 1.30 and xmm1 1.2; mode 0 with the table on: off, the targets the origin and the kept yaw and pitch; then the setter |
| 0x284020 | `movss xmm0,[ebx+0C8h]` | `call 0xA42A805` + 3 nops | `tick`: on: the drag's +CCh/+D0h of the frame to the spin and the tilt (±1.2), taken from the camera; flying: x, z, yaw, pitch 5·dt of the way, set to the targets once all are within 0.001, the flight over when off; then the load. Every register but xmm0 kept |
| 0x247576 | `cmp byte [ebp-1],0` / `jne 247592h` | `call 0xA42A80A` + nop | `cull`: the table's player to 0x247592 (the draw path); else the original's two ways |
| 0x24778F | `cmp byte [ebx+0F49Ch],0` / `je 2477C2h` | `call 0xA42A80F` + 4 nops | `matrix`: the table's player: eax = [ebp−0E8h] = rotY(spin) · rotZ(tilt) with (0, 0, −0.18) of the model at BENCH, on at 0x2477C8; else 0x247798 (the decal pose) or 0x2477C2 (the hand) as the original. Every register kept but eax |

- **The table's player:** the table on, mt_draw's return address 0x284A83 (the lobby character's update), its character (the update's ebx, [ebp−0F0h] after mt_draw's `push ebp / mov ebp,esp / sub esp,0ECh / push ebx`) holding it at +D2B8h and its upgrade window open (+D515h). The squad panels' characters and every match player fail it.
- **BENCH** (4.25, 1.30, 3.05): 0.39 m above the bench's top, in the clear part between the drill press and the vice, 0.85 m from the wall; a rifle's 0.95 m along z fits z 2.58..3.52 and its spin keeps within x 3.75..4.75. The camera at the distance 1.2 and pitch −0.25 then stands at (3.09, 1.60, 3.05), in the room in front of the bench (emulated, `check_weapon_table_026g.py`), looking along +x (yaw −π/2) with the stock side angle 0.37. **GUESS:** that this is a good view, that nothing of the bench's stands in front of the eye, that the room's lights light the weapon there, and the drag's direction (the stock camera's sign turns the weapon).
- **CENTRE_Z** (−0.18; **wrong, corrected in round 2 below**: the box is rotY(π) of the mesh, and the pose moves some): the weapons' models have their grip at the origin and the muzzle along −z (the render `export_properties` bounding boxes: AK-12 z −0.673..0.265, SVD −0.937..0.284, PM −0.124..0.034); −0.18 is a rifle's middle (a pistol turns about a point 0.13 m in front of its own). **GUESS** that the weapon's model is level with an identity transform (the boxes are thin in x and y).
- **What it leaves alone:** patch 32's choice and builds, the decal pose (patch 32 keeps it 0), the least distance (0.6 while patch 32's window is open), the wheel, armour windows (the original's camera as before), the squad panels, a match (the lobby character is not drawn there; the table's player test fails for a match's players). With patch 45's code missing or the table off, every site is the original's code and patch 32 is as it was (its two jumps go to the setter's own call when `-DCAMERA` is not given).
- **Failure:** while the held weapon is not there yet (the player being made, `setup_profile` → on_player_ready), nothing is at the bench and the character's hands are empty; the camera has flown anyway. A window left open into a match stays the table's until the movie's next exit_item_upgrade, as patch 32's mode does.
- **Relocations:** none under the three sites; nothing aims inside them (the jumps to 0x284020 from 0x283FB3 and 0x284004, to 0x24778F from 0x24778B, land on their first bytes). The code finds its state with call/pop and the return address it looks for from it; it calls nothing but the setter's jump.
- **Checked:** `selftest_026g.py` check 45 and check 33 (patch 32's jumps); `check_weapon_table_026g.py` (Unicorn 2.1.4, as is and rebased): 90 ticks with no weapon's window are the original's byte for byte; the AK's window: the point within 1 cm of the bench after about 1 s and exactly there after (the height within the camera's own 0.001), the eye at (3.09, 1.60, 3.05) 1.2 m from it; a drag turns the spin and the tilt, not the camera; the close back exactly at the origin and the yaw and pitch kept in about 1.6 s, then 60 ticks with a drag as the original's; windows over windows; the nearest turn for six yaws; mt_draw's two sites in 64 cases (the table's player drawn when not seen and given an orthonormal transform with the weapon's middle at the bench; every other case the original's way, every register and the x87 stack kept). `check_weapon_preview_026g.py` passes with patch 45's camera values. **Not run in a game.**
- **Builds** (this branch, 2026-10-21): without a server sha256 `860644dc46d32557034112fa6d6858fd23265b55fbfe6bdca5847add752a7f97`, with `--server=skhid.link:1234` `c37e73ae528b4d33b250daa88a3ddbbd5c67b045f47947b4d758c17eb7391ab3` (12 910 200 bytes each).

**Round 2** (2026-10-21, after the owner's game run of e93fb6f0, merged into feat/weapon-camo: the camera flew to the bench and the weapon was there; the owner: "lmb should also rotate above the crafting table", "the weapon should be centered", "now it is in the end", "sorry, I meant z axis", "the weapon should spawn first and then camera move to it (or happen at the same time)", "the lamp under the table should give more light", and of the left button: "it should only move weapon, when the cursor is currently not moving an item"). `weapon_table_026g.asm` is 1 488 bytes, its state 48h bytes (+28h READY, +2Ch HOLD, +30h the time held, +34h the left button's state, +38h the weapon's middle), and a fifth entry:

| site | original | patched | code |
|---|---|---|---|
| 0x283D81 | `test byte [eax+0Ch],52h` / `setne al` | `call 0xA42A814` + 2 nops | `left`: a button of 52h as before; at the bench (the table on, the camera there) the left button (01h) too when the lobby menu's vtable+24h (is_mouse_over_ui 0x225220, the movie's `root.get_mouse_over`: the top-most display object under the cursor is not the drag region and not the stage) answers false at its press's first move, until it is let go; true (a control, an item being dragged from the inventory) blocks it until it is let go |

- **Why the pivot was at the end** (PROVEN in the data and the code, emulated). A weapon's mesh (its `converted_model` vertices) is not in the frame of its render model's box (`render/export_properties` `bounding_box`, kept at render_model+108h by load_properties 0x1582C0): the box is rotY(π) of the mesh, the engine turns it back (skeleton_render_model_instance::update_render_matrices 0x15A590 copies the model's box to the instance's +62B8h and transforms it by rotY(π), 0x15A77F). The AK-12's mesh lies along +z (z −0.265..0.673), its box along −z (−0.673..0.265): the first round's CENTRE_Z −0.18, read from the box, was 0.38 m from the middle, by the stock's end. And a weapon is drawn posed: the bones mt_draw gives weapon::draw ([ebp−0Ch], [ebp−18h] of them; bone matrices in model space, rows, as update_render_matrices skins with the model's inverse binds +140h × each bone, 0x15A7DC..0x15A7EC) are its 3rd-view idle; bone 1 (`offset`, the body's, every weapon of the data) moves the PM's and the APS's body 20 cm along +z.
- **The middle at the bench** (PROVEN in the code; the arithmetic emulated on the AK-12's and the PM's data). The held item (player+128h), when a weapon (vtable 0xA04174, its ctor's alone), has its skeleton model at +1810h (load_weapon 0x243A9A), its render instance at +108h (skeleton_render_model_instance, vtable 0x9F0F24), the model at +62B4h: the middle = (−cx, cy, −cz) of the model's box centre × bone 1's inverse bind × bone 1's pose of the frame; row 3 of the transform = BENCH − middle × the rotation, so the weapon turns about it. A box of ±1 (the exporter's default in the model's `render/export_properties`: 14 weapon models of the data, the A-545, AK-74U, AKMN, IZh-27, both Mosins, TOZ-122, TOZ-34 and Vepr with their camo copies), anything missing or not a weapon: (0, 0, 0.2) of the model. The middles along the length (z, the posed mesh's box, from the data): AK-12 0.204, SVD 0.326, PKP 0.256, Mosin 0.221 (box ±1: 0.2 used, 2 cm off), Remington 870 0.242, KRISS 0.161, Uzi 0.085, PM 0.246, APS 0.265, MP7 −0.050, PP-2000 −0.076, P90 −0.028, ASh-12 −0.005, Magnum −0.009; a constant would have been up to 0.28 m off.
- **The weapon first, then the camera** (PROVEN in the code, emulated). A lobby character makes its player while +D2C4h holds the player's query (setup_profile 0x284CB7; −1 none: the ctor, on_player_ready 0x284F86) or +D2C8h is set (a setup_profile waiting for it, 0x284BB2). Until then the table's player's held item stays in its hand (the old player's weapon, the chosen kit's); after, it is drawn at the bench and READY is set. The window's start sets HOLD: the tick then holds the camera (the three steps' speeds +E0h, +E4h, +E8h 0, the flight not stepped) until READY, at most 2 s; then the speeds the setter gives (|target − now| · 1.111 a second) and the flight. A weapon its kit already shows needs no new player: READY at the next draw.
- **The light** (data, `../../scripts/content-edit/lobby-scene-2.json`, which replaced lobby-bench-light-1 on 2026-10-22): a point light of the bulb above the workbench (`point_light(7)` at (4.81, 3.02, 2.91), 1.81 m from BENCH, behind it seen from the camera: the ceiling lamp's `point_light` copied, its warm colour, intensity 3, range 5 m) and a second bulb made as the bench one (`level/lobby/bench_bulb`, the bench bulb's own socket and see-through glass cut out of `electric1` by the content pack lobby-bulb@1, tilted 50.7 degrees on a drop wire, a `wire_set`, from the `electric1` wire over the artefact table poster) with `point_light(8)` at its tip, (3.40, 3.17, 3.88), intensity 1.15, range 4 m, attenuation_power 2 (the poster behind it half as lit, BENCH as with 3, 5 m, 7.7), 2.22 m from BENCH on the camera's side; raising the ceiling lamp itself, the PR's first version, left the bench dark. The first version (lobby-bench-light-1, revision 27) raised the ambient fill light under the workbench's end (`client_objects[49]`, ambient_light(3)) intensity 1 → 2.5, radius 3.1 → 3.6 m; the owner saw it as a fake white light. In the content patch, not the exe.
- **Checked:** `check_weapon_table_026g.py` 28 checks (as is and rebased): the hold and the speeds given back, the 2 s limit, the character making its player (the held item in the hand, READY 0), the middle of the AK-12 and the PM at the bench and the fallbacks (a default box, not a weapon, no item), the left button (52h as the original; the left button not, but at the bench with its press off the movie; blocked when on it until let go; is_mouse_over_ui asked once a press); the round 1 cases. Selftest check 45 adds the site and the facts above (on_mouse_move reads only al after it; the weapon's, the instance's and the character's fields). **Not run in a game.**
- **Builds** (round 2): without a server sha256 `56ade5122f6438550e35222109597951104c2e6818aca474c1a8a3d263b668c6`, with `--server=skhid.link:1234` `3867f3ff8b7bdcf8f66ff2cb341d9549bc29f0934acd8c2ca642622e4775b84f` (12 910 712 bytes each).

**Round 3** (2026-10-21, after the owner's game run of round 2: "vss vintorez is not centered around the gun", "i mean rotating"; "when adding camo or scope, is it possible to not have the time where previous model disappears, empty screen is shown, and a new one appears"; and patch 47's need of the bench transform). `weapon_table_026g.asm` is 1 798 bytes (to `.srpw`+F06h), its state 90h bytes.

- **Why the VSS turned off its middle** (PROVEN in the data, emulated offline). Its render box (`render/export_properties`, x −0.296..0.021) holds a second magazine (bone 8 `magazin2`, 491 vertices) parked 28 cm beside the gun in the bind pose; the round-2 middle took the box's centre, 13 cm to the side, so the gun turned about a point beside it. The SR-3M and the Val share it (their `magazin3`), the SKS parks its stripper clip 35 cm beside (16.5 cm off), the vz. 58, the MP-153 and the Winchester others. And 14 models have the exporter's default box (±1) and fell back to (0, 0, 0.2): the AK-74U, the Vepr, the TOZ-34, the IZh-27, the AKMN, the A-545, the TOZ-122 (4 to 12 cm off); the Magnum's box holds its spare rounds; the RPD's and the Mosin's bodies are off the axis.
- **The fix.** The box's centre is taken with its x 0 (parts are parked beside the gun, not along it), then through bone 1 as before. The weapons this does not put within 1 cm of their middle are in a table MIDDLES (26 entries of 8 bytes: the items_dict id, the posed gun's middle in the model's space in mm, the parts parked beside it left out), keyed by the items_dict id of the record the character's player was built from (round 3b below; round 3 read weapon+25Ch, which crashed). The survey (every weapon of `items_dict`, 83, its config's `object/model`, all its surfaces' vertices posed by the 3rd-view idle, parts more than 10 cm beside bone 1's body left out) before and after:

| weapon (items_dict id) | model | round 2 | now | how |
|---|---|---|---|---|
| sks (109) | `weapons/sks` | 16.5 cm | 0.7 cm | box |
| sks_premium (2246) | `weapons/sks_camo/sks_camo` | 16.5 cm | 0.7 cm | box |
| sks_premium_2 (2247) | `weapons/sks_camo/sks_camo_st` | 16.5 cm | 0.7 cm | box |
| vss (308) | `weapons/vss/vss` | 13.0 cm | 0.8 cm | box |
| vss_premium (335) | `weapons/vss/vss` | 13.0 cm | 0.8 cm | box |
| sr3m (309) | `weapons/sr3m/sr3m` | 13.0 cm | 0.8 cm | box |
| sr3m_premium (336) | `weapons/sr3m/sr3m` | 13.0 cm | 0.8 cm | box |
| val (307) | `weapons/val/val` | 13.0 cm | 0.8 cm | box |
| vepr (216) | `weapons/vepr` | 12.2 cm | 0.0 cm | table |
| vepr_premium (2244) | `weapons/vepr_camo/vepr_camo` | 12.2 cm | 0.0 cm | table |
| ak_74u (13) | `weapons/ak74u` | 11.8 cm | 0.1 cm | table |
| ak_74u_premium (2243) | `weapons/aks74u_camo/ak74u_camo` | 11.8 cm | 0.1 cm | table |
| magnum (56) | `weapons/magnum/magnum` | 9.5 cm | 0.0 cm | table |
| vz58 (2213) | `weapons/vz58/vz58` | 9.0 cm | 0.6 cm | box |
| toz_34 (16) | `weapons/toz34/toz_34` | 8.4 cm | 0.1 cm | table |
| izh_27_lancaster (108) | `weapons/izh_27_lancaster/izh_27_lancaster_` | 8.2 cm | 0.0 cm | table |
| izh_27_lancaster_premium (337) | `weapons/izh_27_lancaster/izh_27_lancaster_` | 8.2 cm | 0.0 cm | table |
| izh_27_lancaster_premium_2 (2251) | `weapons/izh_27_lancaster_camo/izh_27_lancaster_camo_st` | 8.2 cm | 0.0 cm | table |
| winchester_1894 (90) | `weapons/winchester_1894` | 8.0 cm | 0.1 cm | table |
| mp_153 (110) | `weapons/mp153/mp153` | 6.5 cm | 0.1 cm | table |
| akmn (80) | `weapons/akmn/akmn` | 4.6 cm | 0.0 cm | table |
| akmn_premium (2241) | `weapons/akmn/akmn_camo` | 4.6 cm | 0.0 cm | table |
| akmn_premium_2 (2242) | `weapons/akmn/akmn_camo_st` | 4.6 cm | 0.0 cm | table |
| a_545 (220) | `weapons/a_545/a_545` | 4.3 cm | 0.1 cm | table |
| toz_122 (12) | `weapons/toz_122` | 4.1 cm | 0.1 cm | table |
| fort_17 (64) | `weapons/fort_17/fort_17` | 3.9 cm | 0.2 cm | box |
| pm (89) | `weapons/pm/pm` | 3.8 cm | 0.7 cm | box |
| aps (126) | `weapons/aps/aps` | 3.7 cm | 0.2 cm | box |

  Every other weapon was within 3 cm before and is within 1 cm now (the largest 1.0 cm); none is off by more than 1.0 cm now. A scope's model (the `_n_<scope>` variants) shares its weapon's id and so its middle.
- **No empty bench on a refresh.** An attach (a scope or a camouflage) makes the lobby build a new player (setup_profile → the query, +D2C4h, until on_player_ready). Round 2 drew the held item in the hand meanwhile, so the bench was empty until the new player came. Now, once the bench has shown a weapon in this window (READY), the current player's weapon stays at the bench through a rebuild, and the new player's is drawn there from its first frame; a weapon's window over a weapon's keeps READY too (no camera hold). **Not reachable:** keeping the old weapon until the new one's camouflage bake has landed: on_player_ready (0x284F20) puts the new player in at once and lets the old one go, so the old weapon cannot be drawn after it; the new one shows plain until its bake lands (the first time a camouflage is seen on that weapon: later ones come from the baked files' cache, faster).
- **The interface** (`WEAPON_TABLE_STAMP_OFFSET`, `WEAPON_TABLE_MATRIX_OFFSET` in patch_026g.py): every time the held weapon is drawn at the bench, the transform it is drawn with (a float4x4, rows, v · M as the game's, row 3 the translation, 64 bytes) is kept at the state's +50h (`.srpd`+F50h) and the u32 at +48h (`.srpd`+F48h) grows by 1. Valid while the state's +0 (TABLE) and +28h (READY) are 1.
- **Checked:** `check_weapon_table_026g.py` 32 checks (as is and rebased): the VSS's and the AK-74U's middles (its table) from the data at the bench within 1 cm of the gun's, the AK-12's and the PM's, the fallbacks; a rebuild after the bench was shown keeps the weapon at the bench, before it the hand; a window over a weapon's keeps READY; the transform at +50h and the stamp. The selftest as before. **Not run in a game.**
- **Builds** (round 3): without a server sha256 `901a166ea43ff19dbf17682444c14afd7b782933c9e93f3f03dcea601ee8b71d`, with `--server=skhid.link:1234` `a9f93e91d94e877db1ba3f4e8b5e13506e0635b72c65562248fcd92a225fd21f` (12911224 bytes each).

**Round 3b** (2026-10-21, the owner's crash: Optics on the VSS, ACCESS_VIOLATION at 11:18:24.004 just after the camouflage bake; the minidump `crash-vss-20261021/crashes/Survarium-#2777_261021-111824.dmp`: eip 0xA429BE3, `movzx eax,word [eax]` with eax = 2, the read at address 2). Round 3 took the held weapon's items_dict entry from weapon+25Ch (an upgrade item's dictionary pointer in weapon::draw's loop, 0x24505F); for the held weapon itself it is not a pointer (2). Now the table's key comes from inside the lobby character alone, which is_table has checked (mt_draw's caller, its +D2B8h the player): its current profile copy (+18h + 4634h × +D2B4h, the index below 3), the kit it shows (+D2B5h, below 3 and the copy's kit count +57h), that kit's slot 7 or, empty, slot 10 (patch 32's find_kit; patch 32 puts a stash weapon in the chosen kit's slot 7 of the scratch profile it is built from), the record's index below 69 and the copy's item count (+1564h), and the record's +4 (u16, the items_dict id, as the inventory's records: weapon_stash_026g.asm). Any index out of range: the box's way (the round-2 method with x 0). No pointer the code reads is unchecked: the weapon by its vtable, its model chain by the render instance's vtable and non-zero pointers (round 2's, run in the game), the rest inside the character. During a rebuild the copy may already be the next player's (the old weapon drawn with the new record's middle for those frames: the same weapon after an attach). `check_weapon_table_026g.py` 38 checks: every case with weapon+25Ch = 2 (the crash's value), the VSS, the AK-74U by its record, a copy index, a kit and a record out of range (the box's way). 1 894 bytes (to `.srpw`+F66h). Builds: without a server sha256 `69a77f17875f92559bc6f4dbed3434d5f1ec89d6d4fc3dd3667686e6c5c2b3f5`, with `--server=skhid.link:1234` `8729de50bd14c85738d8701f4b2c10939d2b279046e3e84238d6bfb4482bbf98` (12 911 224 bytes each). **Not run in a game.**

**Round 3c** (2026-10-21, the owner on e71eb9f7: "vepr is not centered it is a bit to the right"). The bench shows a weapon side on (its length along the screen), so "to the right" is along its length. The Vepr (and its premium) has the exporter's default box, so its middle came from the table, which held the middle **posed by the 3rd-view idle**. The Vepr also has an `item/stand/3rd_view/preview` animation (as the SKS, PPSh and Uzi), in which its bone 1 (`offset`) is at the origin, not 7 cm along as in the idle: the gun drawn in that pose had its middle 6.9 cm along (and 1.4 cm sideways, 2.1 cm down) from the table's. Which animation the lobby's held weapon plays is not read in the exe (GUESS: the preview where there is one; the owner's 7 cm matches it). Now the table holds each gun's middle in its **bind space** (the idle's middle through the inverse of bone 1's skinning in the idle), and the code takes it through bone 1's inverse bind and pose of the frame, as the box's: the middle is right in whatever pose the weapon plays. The same 26 entries (the values change where bone 1 moves in the idle: the Vepr, Magnum, RPD, Winchester, 9A-91). State A0h bytes (+90h the bind-space middle), 1 926 bytes (to `.srpw`+F86h). The 3D survey (all 83 weapons, each in its idle and, where it has one, its preview; error = the middle drawn minus the gun's posed middle; x sideways, y up, z along):

| weapon (id) | pose | round 3b x / y / z (cm) | now x / y / z (cm) |
|---|---|---|---|
| vepr (216) | preview | 1.4 / -2.1 / 6.9 | 0.0 / 0.0 / 0.0 |
| vepr_premium (2244) | preview | 1.4 / -2.1 / 6.9 | 0.0 / 0.0 / 0.0 |
- worst x (sideways): round 3b 1.4 cm, now 1.0 cm
- worst y (vertical): round 3b 2.1 cm, now 0.2 cm
- worst z (length): round 3b 6.9 cm, now 0.1 cm

  The Vepr-12 (another model, the box's way) is within 0.9 cm in its idle (it has no preview). `check_weapon_table_026g.py` 42 checks (the Vepr in both poses from the data). Builds: without a server sha256 `91c233ddcffccc9b548ffd54e40e1943bf601f873b1c7dcdea8942d6dc77f801`, with `--server=skhid.link:1234` `5fd38b4895bb10583f9ee752830b0fe75bfcc6a1cb646d08f34d0cbe4543be47` (12 911 224 bytes each). **Not run in a game.**

**Round 4** (2026-10-23, the owner's report on a local game of main with PR #312: "i can press attachments and then quickly esc, what happens is that the weapon I was trying to attach too, is now in my hands, even though in inventory I have a different weapon"; the screenshot: the chosen kit's weapon slot shows a Mosin carbine (its weapon 2, no weapon 1), the character holds a blue-camouflaged long rifle, the window's). PROVEN in the code, emulated:

- **The Esc path is the close's.** The movie's Esc (`close_active_window`) calls the open popup's `UpgradeWindow.closePopup`, which dispatches CLOSE (or, with the decal editor open, only leaves the editor: the window stays open); `closeModifierWindow` calls `exit_item_upgrade` whenever `_upgradeShown`, which `openModifierWindow` sets for every window it tells the game of. The screenshot agrees: the camera is back at the character (patch 45's table off), so the close ran. READY is not read by the close.
- **Why the window's weapon stayed.** `setup_profile` (0x284B10) returns early (0x284B83 → 0x284CF8) when the kit asked has the id (copy+76Ch) and revision (+4614h) of the **shown** copy (+D2B4h), the last asked id (+4) is that id and no call waits (+D2C8h); it does not look at a player being made (+D2C4h). The shown copy changes only when the player arrives (on_player_ready 0x284FAC). Patch 32's scratch (a weapon no kit holds, in the chosen kit's slot 7; armour the chosen kit does not wear) keeps the chosen kit's **id** with a revision of its own (`0x80000000 | n`). So: the window's start asks for the scratch (a new revision: a query, the copy after the shown one, +4 = the chosen kit's id); a close before that player arrives asks for the account's chosen kit, which is the shown copy's id and revision with +4 equal: the early return, nothing waits; the scratch's player arrives, is shown, and nothing makes the account's again (on_player_ready re-calls setup_profile only when +D2C8h is set, 0x285099). A weapon in another kit was not hit (+4 is that kit's id, so the close waited, as the original does for another kit). In the stock game a kit id comes back with an older revision while a newer one is being made only by such a scratch.
- **On live too.** Revisions 28 and 29 (tags `026g-rev28`, `026g-rev29`) hold the same patch 32, stash, 45 and 48 code as main (no change to those files since): the same race.
- **The fix** (patch 48's code, which already handles setup_profile while a player is being made, `again`): `early`, a seventh entry (+1Eh), at 0x284B7C `cmp byte [ebx+0D2C8h],0` (7 bytes, the early return's last test; `call 0xA42C01E` + 2 nops; only the `jz` after it reads the flags): ZF as the original's for a squad mate's character; for the lobby's own (lobby menu+694h) also only while no player is being made (+D2C4h −1). The close then goes on: `again` (the player being made is the scratch's revision, not the asked one) lets the original wait (the profile into the copy after the one being made, +D2C8h 1); the scratch's player arrives, on_player_ready's own re-call (0x2850C5) queries the account's, and the character holds the chosen kit's weapon (the scratch's player shows for the time one player takes to make). A call for what is being made still returns at `again`; once nothing is being made the early return is the original's. No base relocation under the site, nothing aims inside it (selftest check 48). Smallest: the game has no cancel of a query that shows something else after it (+D2C9h, set by 0x284AA0 and by the lobby menu letting its three characters' players go at 0x21D100.., comes with the shown player let go, and on_player_ready then shows nothing).
- **Checked:** `check_upgrade_preview_026g.py` (Unicorn, the game's own setup_profile, profile copy, the movie's start and exit handlers 0x29A870/0x29A890 with the camera's setter and patch 45's camera, and on_player_ready 0x284F20 with its players' virtual calls stubbed): a rifle only in the stash with the chosen kit's weapon 2 the SVD (the owner's case): start, close before the player arrives, the window's player arrives, the account's is queried by on_player_ready, then arrives: the character holds the SVD; with 0x284B7C the original's bytes the same run ends with the stash rifle in the hands (the report); the same for armour the chosen kit does not wear; `early` itself for both characters, waiting or not, a player being made or not (the flag, every register). The other weapon checks pass; the self-test passes.
- **Build** (local test, `--revision=30`): `survarium_skhid.esc-r30.exe` sha256 `c74062480cac6c06b9f8509ce24354b1d265a7c08a4342a890c95e0938451e5f` (12 934 776 bytes); against main's build with the same revision it differs in the PE checksum, `.srpw`'s virtual size, the site (7 bytes) and patch 48's code (+1Eh on). **Not run in a game.**

**Round 5** (2026-10-23, after the owner's game run of round 4 (#313: "it works"): the camera waited about 200 ms at a window's start, for the window's player, before it flew to the bench; asked whether the flight should start at once and the weapon appear above the bench when its player is ready (round 2's "or happen at the same time"), the owner: "yes, let's"). PROVEN in the code, emulated:

- **No wait.** The start no longer sets HOLD; the tick's hold block (the steps' speeds 0 until READY or 2 s, then the setter's speeds computed again) is gone, so the camera flies from the first tick (round 5b below: with eased speeds). `left` no longer tests HOLD: with the table on, a left-button drag off the movie turns the weapon, as before at the bench. State +2Ch and +30h (HOLD, the time held) are free (round 5b uses them); the interface (+48h, +50h) is where it was.
- **Nothing in the wrong place.** Before, the table's player's held item stayed in the hand while the character made the window's player (the camera at the character, holding). Now the camera leaves at once, so the held item of the player being replaced is drawn with the decal pose's transform (`matrix` returns to 0x247794: translate(0, -10, 0), 10 m under the floor, as the stock decal editor puts the weapon away) until the new player is made; its first draw then is at the bench (READY). A refresh after the bench showed a weapon keeps the current one there, as round 3. A weapon its kit already shows needs no new player: at the bench from the first draw.
- **What stays:** the close and the flight back, the drag and the left-button rules, the windows over windows, patch 48's preview and round 4's quick close (`check_upgrade_preview_026g.py`'s quick-Esc cases run patch 45's camera through the movie's handlers and pass).
- **Checked:** `check_weapon_table_026g.py` 40 checks (as is and rebased): at the start the targets are the bench's and the steps' speeds the setter's, and the point, yaw, pitch, height and distance move at the first tick with READY 0; the point at the bench within 1 cm; while a player is being made before READY the held item takes the decal pose's way (registers kept), and the first draw after it is made puts it at the bench (READY 1); the left button with the table on. `check_weapon_decals_026g.py` (a ray before READY misses; the matrix entry now returns to the decal pose's way) and the other weapon checks pass; the self-test passes. 1 742 bytes (to `.srpw`+ECEh).
- **Build** (local test, on #313, `--revision=30`): `survarium_skhid.camnow-r30.exe` sha256 `24735c9a873bd6ea0674ba71d71cd0b27ea2853a01823890a281b699de137a35` (12 934 776 bytes); against round 4's `esc-r30` it differs in the PE checksum and patch 45's code alone. **Not run in a game.**

**Round 5b** (2026-10-23, the owner on round 5's build: "the camera jumps a bit at the end of the transition", and "i guess it is transition from camera move to the scene"). PROVEN, emulated tick by tick (`check_weapon_table_026g.py`'s Table: the window's start through the movie's handler, the camera's whole tick 0x283EF0, READY at the 12th tick, the eye and view from the matrix the tick gives the menu):

- **The cause.** The flight had two motions. The point, yaw and pitch (this patch's) moved 5·dt of the way left each tick (an ease-out), and were snapped to their targets once all were within 0.001. The side angle, height and distance moved by the camera's own steps (0x283F62 height, 0x28412D side, 0x284222 distance) at the setter's constant speeds (|change|·1.111 a second, 0x2845C0), which reach their targets in 0.9 s and stop dead. Per 16 ms tick, the eye's and the view's move (|Δeye| + 1.2·|Δforward|) went 0.027 → 0.011 → 0.006 at 0.88 s (the distance's 0.84 m/s and the side angle's 23°/s stopping in one tick, the point still gliding). Then at 1.6 s it went 0.18 mm → 2.0 mm, the snap of the point, yaw and pitch, after creeping. No camera field changes at READY (nothing reads it there, nothing calls the setter again: its only callers are 0x265E73 and 0x265F44), and no dt is computed from the start. The hold had hidden none of this: it was there before too, just later.
- **The fix** (`weapon_table_026g.asm`, 1 994 bytes, to `.srpw`+FCAh):
  - **All seven ease out together.** While a flight eases (`camera` sets EASE +2Ch and its time +30h), the tick sets the camera's three steps' speeds to 5·|target − now| each tick, so they ease out with the point, yaw and pitch. EASE ends once those steps are within 0.001 (where they stop, 0x283FA0, 0x284253) and the point, yaw and pitch are there, or after 4 s.
  - **The point, yaw and pitch land exactly, with no snap.** Each step is 5·dt of the way plus 0.01·dt (FLOOR, m or rad a second). The step that would pass the target lands on it, so the last moves are 0.15 mm and the "there" test is within 1e-6.
  - **The bench and the wheel are unchanged.** At the bench (EASE over) the camera's steps keep their speeds (the wheel sets its own, 0x28409E). The close flies back the same way and ends exactly at the origin's point and the kept yaw and pitch.
- **Residual:** the camera's own steps stop within 0.001 of their targets (the side angle 0.369 of 0.37, the distance 1.201 of 1.2), as the stock camera's do. The first tick is the flight's fastest (0.45 there, 0.74 back, an ease-out).
- **Checked:** `check_weapon_table_026g.py` 42 checks (as is and rebased):
  - the new flight case: there (READY mid-flight) and back, every tick's move is at most 1.01× the tick before (+0.2 mm) and at least 3/4 of it while above 2 mm; the last move is under 0.5 mm, and the easing is over at the bench;
  - the start's speeds are 5× the way left;
  - the bench within the steps' 0.001;
  - run against round 5's bytes, the same measure fails at 0.88 s (two ticks) and at 1.6 s (the snap).

  The other weapon checks and the self-test pass.
- **Build** (local test, `--revision=30`): `survarium_skhid.camnow-r30.exe` sha256 `5d4e83b863cbf48c2b671869ec77d859719afba30bcbeb8100bd477cdbf5de44` (12 934 776 bytes; round 5's was `24735c9a…7a35`). **Not run in a game.**

**Round 5c** (2026-10-23, the owner on round 5b: "still the same problem", "same jump", "it is camera transition jump", "happens for every weapon"). PROVEN in the game's own log, then emulated.

- **The debug build.** A local debug exe (branch `debug/camlog`, not for release) logged the camera each frame for 3 s after a window's start or close (`srp camlog` lines). The owner's run (01:35:10):
  - frames 1–25 are the smooth flight, the distance's current value coming down 1.96 → 1.34 toward 1.2;
  - the window's player is made between frames 25 and 26 ("made in 334 ms"; the player pointer changes);
  - on frame 26 the current distance is **0.60**, the floor patch 32 gives the tick's last clamp, and the eye moves **0.80 m** in one frame: the jump;
  - from frame 38 the distance creeps back, but only to **1.0924** of 1.2, and stays there.
  - On the close (01:35:20) the second frame is cut to 0.60 again, and the cut stops once the camera has left the bench.
- **The cause.** The tick casts a collision ray from its point back toward the eye (0x284309 → 0x2844D0, through the lobby's collision scene at menu+3104h). At a hit it overwrites the current distance (+C4h) with the hit's, and 0x284316 clamps that to the least (0.6). The Unicorn checks stub that ray, so they never saw it.
- **What the ray hits.** Not identified by its object; what is known:
  - it appears in the ray's way the frame the window's player is made and goes when that player is replaced;
  - its hit points while the camera rests lie about 0.1 m in front of the bench's eye (the eye at (3.19, 1.57, 3.05) where (3.09, 1.60, 3.05) is meant);
  - so it is something the window's player brings near the bench, not the room;
  - no frame before that player had a hit.
- **Why only now.** With round 2's hold the camera flew only after READY, so it came onto the bench with that object already there. The cut grew smoothly as it approached, and nobody noticed a 1.09 instead of 1.2. Flying at once (round 5), the object appears mid-flight, already within reach of the ray: the cut is one frame.
- **The fix** (`weapon_table_026g.asm`, 1 870 bytes, to `.srpw`+F4Eh; a sixth entry `ray`, +19h):
  - **No ray while the camera is the table's.** 0x284309's `call 2844D0h` is `call ray`, which returns at once (`ret 4`, as the ray) while FLY is set: flying there, at the bench, flying back. The bench's view is made with nothing between its eye and the bench. Every other time it jumps to the game's ray. Declared CALLEE in `INSTALLS`.
  - **The steps are moved by the flight itself.** While a flight eases, the flight moves the camera's own side angle, height and distance the way it moves the point, yaw and pitch: 5·dt of the way plus 0.01·dt, landing exactly. The three steps' speeds are 0 meanwhile, as round 2's hold had them. So all seven end together and the bench's distance is exactly 1.2 (round 5b's per-tick speeds left them within the steps' 0.001).
  - **Simpler code.** One routine (`glide`) for both tables. No snap threshold, no time cap.
- **Checked:**
  - **`check_weapon_table_026g.py`: 44 checks (as is and rebased).**
    - New case: the ray stub reports a hit at 0.6 from READY on. No ray is cast while the camera is the table's, the flight is as smooth, and it rests at exactly 1.2, 1.3 and 0.37 with the eye 1.2 m from the bench.
    - Once the stock camera is back, its ray is cast and cuts the distance as the original's.
    - Round 5b's bytes under the same hit: an eye move of 0.77 m in one frame (the owner's log: 0.80 m) and a rest at 0.6.
    - The flight case: speeds 0 meanwhile.
  - The self-test, check 45, has the new site.
  - The other weapon checks pass.
- **Build** (local test, `--revision=30`): `survarium_skhid.camnow-r30.exe` sha256 `ef9b3a2dd438c5677766f77338c5f3bb4396089e4a31e5a0b59a0fd9603b99a0` (12 934 776 bytes). **Not run in a game.**

**Round 5d** (2026-10-23, the owner on 5c: "yes, it is smooth", then "the weapon is a bit too far, and is a bit off center to the right", "people go into that menu to interact with a weapon, so it should be front and center (also it seems we are centered from left to inventory, while there is still appearance tab)", "one problem is with fov", "i was on high fov (100) and that's why it was far too", "maybe we should not have fov affect weapon preview in lobby under bench?", "just like we did with scopes"). PROVEN in the code and the movie's source, emulated:

- **The measured point.** The owner's screenshot (5c, uro_fov 100, 1280 x 720): the PPSh's middle at x ≈ 520, about 230 px long. The camera then looked at BENCH from 1.2 m with the stock side angle 0.37, which turns the view (rotY of the matrix): BENCH sits 20.5 degrees off the view's axis, to the left. At a vertical 100 degrees (patch 22: the lobby camera follows uro_fov, `fov_projection` on lobby_camera's vtable slot 0, its zoom at camera+94h) that projects to x = 640 − 640 tan 20.5° / (tan 50° · 16/9) = 527, and a 0.91 m model to 233 px. The maths agree with the picture. The appearance window's left edge is at 692 and the inventory at 820, so "to the right" is relative to the part left of the appearance window (centre ≈ 346), not the screen.
- **The field of view, as a scope's.** A full scope's view is the original's whatever uro_fov because patch 22's angle `65 z + (uro_fov − 65) max(0, (z − 1/4) / (3/4))` drops the setting's term there. The bench uses the same projection and the same zoom field: `camera` sets TARGET_ZOOM (state +4Ch) to the z with that angle = 65, z = (uro_fov + 130) / (4 uro_fov − 65): 1.333 at 50, 1 at 65, 0.687 at 100. The flight glides camera+94h there with the rest (the field of view eases over the flight), and the close glides it back to 1, landing exactly: the player's field of view again. No second projection path. At uro_fov 65 nothing changes.
- **Centre and size, once, at the original's 65 degrees.**
  - **The free part's centre.** The lobby movie lays itself out on 1920 x 1080, scaled by the width below an aspect of 1.85 and by the height above (`LobbyMenu.onStageResize`). The inventory is right-aligned (`layoutData` −112, 580 wide), and the upgrade window's panel is 190 left of it (`UpgradeWindow` `container.x = inventory_list.x − x − 190`). So the free part ends 882 units from the right edge, and its middle is fx = 1/2 − 441 / (1080 a') of the width (a' the aspect from 1.85 on, else 16:9): 0.270 at 16:9 and 16:10, 0.329 at 21:9.
  - **Centring.** The side angle is now 0 (`camera` passes 0 to the setter). The camera looks at a point k = (1 − 2 fx) d T along its right of BENCH (TARGET_Z = 3.05 − k), T = tan(65/2) · a. The weapon's middle (drawn at BENCH) lands at fx across and at the middle in height.
  - **Distance.** An end at r = L/2 from the middle stays 2% in from the left edge at any spin while d ≥ L sqrt(1/T² + 0.96²) / (4 (fx − 0.02)), and that is d, at least 0.3 m. L is the window's weapon's render box along its length (`middle` measures it once; a default box counts as 1 m). Until the weapon is drawn the view is a 1 m weapon's; the frame it is, `tick` frames it for its own length and the flight eases there (EASE again).
  - **Patch 32's NEAR_MIN** (its least distance while a weapon's window is open) is 0.3, was 0.6, so a pistol can come that near.
- **What it gives** (check_weapon_table_026g.py, the same at uro_fov 50, 65 and 100), of the free part's width at rest:

  | screen | AK-12 (0.94 m) | SVD (1.22 m) | PM (0.16 m) |
  |---|---|---|---|
  | 16:9 | 63% at 1.22 m | 63% at 1.59 m | 43% at 0.30 m |
  | 16:10 | 66% at 1.29 m | 66% at 1.67 m | 48% at 0.30 m |
  | 21:9 | 52% at 0.90 m | 52% at 1.17 m | 26% at 0.30 m |
  | 32:9 | 30% at 0.90 m | 30% at 1.17 m | 15% at 0.30 m |

  The rifles are below the asked 70% because their ends must stay on the screen at any spin: a turned rifle's near end swings left toward the screen's edge (the middle sits at 0.27 of the width).
- **The review's two fixes** (5d's review, 2026-10-23):
  - **A window over a weapon's window kept the first weapon's distance.** `camera` reset LENGTH but the bench kept the old weapon (READY) through the ~0.3 s rebuild. `middle` measured that old weapon, and the new one was never measured: a pistol over the SVD stayed at 1.59 m, and a rifle over a pistol at 0.3 m with its ends off the screen. Now `middle` measures only when the character makes no player (+D2C4h −1, +D2C8h 0). Patch 32's start calls setup_profile at once, which sets them when a new player is to be made. So the old weapon drawn through the rebuild is not measured, and the window's weapon is measured on its first frame.
  - **Ultra-wide screens put the tilted ends off the top and bottom.** frame_view took only the width's distance, so at 32:9 (3840 x 1080, 5120 x 1440), with the tilt ±1.2 over a full spin, the AK-12's and the SVD's ends went from −0.24 to 1.09 of the height. Now d is also at least TILT_ROOM · L = 0.962 L: the ends stay 2% in from the top and the bottom at any spin and tilt at the original's 65 degrees (the pitch's −0.25 included, emulated). That binds beyond 16:10. At 21:9 the tilted ends had come within 0.8% of the edge, so 21:9 moves a little further too (0.88 → 0.90 m for the AK-12); 16:9 and 16:10 are unchanged.
- **Room.** The code grew to 2 230 bytes (2 266 with the review's fixes) and moved from `.srpw`+800h to +6B0h (the stash ends at +6A5h), so patch 32's two `jmp CAMERA` are reassembled for 0xA42A6B0.
- **Checked:**
  - **`check_weapon_table_026g.py` 48 checks (as is and rebased).** For 1280 x 720, 1920 x 1200, 3440 x 1440, 3840 x 1080 and 5120 x 1440 (viewport set), uro_fov 50, 65 and 100, and the AK-12, an SVD box and the PM:
    - the zoom gives 65 degrees;
    - the middle projects to fx and the height's middle within 0.0002;
    - the distance is the formula's for the measured length;
    - over a full spin at tilts 0 and ±1.2 the ends stay within the 2% margin across and up and down (and in front of the near plane);
    - the results do not depend on uro_fov;
    - after the close the zoom is exactly 1.
  - **A pistol's window over the SVD's**, the SVD drawn at the bench through the rebuild: the length is not measured then, and the pistol is framed for itself (1.59 m, then 0.30 m).
  - The other cases now have their targets from frame() (the side angle 0, every value landing exactly).
  - `check_weapon_preview_026g.py` (the side angle 0, the distance the 1 m weapon's, NEAR_MIN 0.3).
  - The self-test and the other weapon checks pass.
- **Build** (local test, `--revision=30`): `survarium_skhid.camnow-r30.exe` sha256 `4eb7c0dfc13a0fb27212863282755974fd1f99137f12b08355247110a0fb935b` (12 934 776 bytes; with the review's fixes). **Not run in a game.**

**Round 5e** (2026-10-23, the owner on 4c45c7a7 at 2560 x 1440: "a bit too much to the left", and "when scrolling the wheel to move the camera closer, it is not focused on the center of the weapon"). PROVEN in the code and the movie's layout, emulated:

- **Where it sat.** In the owner's screenshot the Mosin spans 0.115..0.445 of the width (its middle 0.28; the model's middle, drawn at BENCH, at 5d's 0.270). The Squad strip ends at 0.046 and the Attachments window begins at 0.54, so the part a player sees free is 0.046..0.54 and 0.27 read as left-heavy.
- **The new place.** The weapon's middle goes FRAME_SHIFT = 365 lobby-movie units left of the screen's middle: fx = 1/2 − 365 / (1080 a'). That is 0.310 at 16:9 and 16:10, 0.359 at 21:9 and 0.405 at 32:9. The free part's own middle, between the Squad strip's right edge (88 units, left-aligned) and the window's edge, is 397 units left; the 32 units more to the right are the owner's "a little more to the right". FRAME_SHIFT is the one constant to retune (`weapon_table_026g.asm`, `frame_shift`).
- **Clear of the window too.** frame_view takes the largest of three distances:
  - **the screen's left edge:** d ≥ L sqrt(1/T² + (1 − 2m)²) / (4 (1/2 − s − m));
  - **the upgrade window's left edge:** d ≥ L sqrt(1/T² + (1 − 2m − 2p)²) / (4 (1/2 − p − m + s)), with p = 882 / (1080 a');
  - **the height's:** 0.962 L, as in 5d.

  With the middle at 0.31 the left edge binds at 16:9 and 16:10, the height beyond.
- **The wheel zooms into the weapon.** 5d looked at a point beside the bench, so the stock wheel (the distance along the view, the point kept) slid the weapon across the screen.
  - **Looking at the weapon.** The camera now looks at BENCH, the weapon's middle, and its side angle turns the view so that the middle is at fx: σ = atan(t cos P) + asin(t sin²P / (cos P sqrt(1 + t² cos²P))), with t = 2 s T and P = −0.25, the view's pitch. The asin's argument is below 0.03 and is taken as the angle (5e-6 rad).
  - **Why the middle stays put.** The view's direction no longer depends on the distance, so the middle stays where it is on the screen at any distance. The distance becomes d sqrt(1 + t²): the middle is on the eye's line, not beside it.
  - **Limits.** The wheel goes from NEAR_MIN (0.3) to the larger of 1.25 times the framed distance and the framed distance plus 0.5 m (a pistol framed at 0.3 m goes out to 0.8 m). `ray` clamps the target while the table is on, after the tick's wheel and step, and the distance itself only once the flight's easing is over. LENGTH holds the weapon's length negated until it is framed, then that farthest distance.
  - **The close.** It flies back from wherever the wheel left the camera.
- **The vertical.** The side angle turns the pitched view about the vertical, so the middle sits about 2% of the height below the middle (accepted; the owner found the height right).
- **Room.** frame_view, the camera and the tick read their constants from a register (ebx or ebp, FC), and MIDDLES packs its x and y into bytes (6 bytes a weapon, all within 4 cm of the axis). The code is 2 378 bytes, ending at `.srpw`+FFAh of the page's 1000h.
- **Checked:**
  - **`check_weapon_table_026g.py` 50 checks (as is and rebased).** The bench's view at uro_fov 50/65/100 and 16:9, 16:10, 21:9, 32:9, for the AK-12, an SVD box and the PM:
    - the middle within 0.0001 of fx;
    - the distance and side angle frame()'s;
    - over a full spin at tilts 0 and ±1.2 the ends 2% clear of the screen's left edge, the window's left edge, the top and the bottom.
  - **The wheel at the bench:** in to 0.30 m, out to 1.25 of the framed distance and in again; the middle's projection moves at most 3e-7. The close then glides back to the stock camera, the zoom 1 and the yaw and pitch kept.
  - The other cases as before (with the side angle frame()'s). `check_weapon_preview_026g.py` (the side angle at the start), the other weapon checks and the self-test pass.
- **At rest**, of the part between the Squad strip and the window:

  | screen | AK-12 | SVD | PM |
  |---|---|---|---|
  | 16:9 | 89% at 1.15 m | 89% at 1.50 m | 56% at 0.30 m |
  | 16:10 | 92% at 1.19 m | 92% at 1.55 m | 60% at 0.30 m |
  | 21:9 | 62% at 0.98 m | 62% at 1.28 m | 33% at 0.30 m |
  | 32:9 | 35% at 0.98 m | 35% at 1.28 m | 19% at 0.30 m |

  The spin margin (2% of the width) sets these; a larger FRAME_MARGIN gives a smaller resting weapon.
- **The review's fix** (5e's review, 2026-10-23). `ray` clamped the current distance as well as its target to the wheel's farthest from the tick the weapon was framed, while the flight's glide was still bringing the distance in. A pistol framed on a settled view went 1.22 → 0.375 m in one frame (85 cm), a pistol over a rifle 1.27 → 0.375, a weapon drawn early in the flight 1.75 → 0.375. Now `ray` clamps the distance itself only once EASE is 0; while the flight eases, the glide brings it there. The farthest is now max(1.25 d, d + 0.5 m), set when framed (a pistol at 0.3 m could only reach 0.375 m). Checked with weapons drawn, not READY poked (`check_weapon_table_026g.py` 52 checks): a pistol and a rifle drawn at the flight's third frame, a pistol drawn after it settled, a pistol's window over a rifle's, and the wheel scrolled out before the framing. Every frame's distance step is within the glide's own (5 dt of the way left plus 0.01 dt) or the wheel's: 7.4 cm at most for the settled pistol, where 5e's bytes snap 85 cm. 2 382 bytes, to +FFEh.
- **Build** (local test, `--revision=30`): `survarium_skhid.camnow-r30.exe` sha256 `bbec605718fd1ffec45a2bf47132d581428dcda10de2d5021b46a1b55d935364`. **Not run in a game.**

**Round 6** (2026-10-29, the lobby scenes: players choose the scene behind the lobby menu, patch 81 and `lobby-scenes.md`; each scene has its own table). PROVEN in the code, emulated:

- **The spot is the scene's data.** BENCH (4.25, 1.30, 3.05) and VIEW_YAW (−π/2) were constants of the code, right for the classic lobby alone. Now `camera`, when patch 32's mode 1 turns the table on, reads them from the lobby's project: the lobby menu (the camera's +88h), its project (+114h, set by on_render_scenes_ready at 0x220E01), its config `[[project+17Ch]+108h]`, as the engine reads `camera/position` at 0x221975..0x2219A1; there `srp_lobby` (a named table), its `weapon` (a float3: its value's +0 points to the three floats) and `weapon_view` (a float, or an int; radians). Each key is asked with has_key (0xF6C00) before child (0xF6B10), which has no end check; each value's type (+14h) is checked. The classic lobby's values come from the content edit `lobby-scenes-1.json` (the old constants).
- **No spot, no table.** No project, no `srp_lobby`, no `weapon` or `weapon_view`, or a value of another type: the window is treated as an armour's. The table stays off, the stock setter takes patch 32's values, and the weapon stays in the character's hands. Nothing is read out of bounds and nothing is shown at a made-up place.
- **The turn.** A scene's table may stand along any wall. The bench's transform is the classic's turned about the vertical by R = weapon_view + π/2: rows rotY(spin) · rotZ(tilt) · rotY(R), with rotY in the spin's convention (rows (cos, 0, sin), (0, 1, 0), (−sin, 0, cos)). The camera then looks along (−sin view, 0, cos view), and the weapon lies across that view, level, tilting about its own length. R is 0 in the classic (−π/2 and π/2 are both float32 1.5707964, so their sum is exactly 0), and then the product is skipped: the classic's transform is bit for bit what round 5e drew. The camera's targets are the spot's x and z, its height (the setter's xmm0) is the spot's y, and its yaw is the nearest turn of weapon_view.
- **The state:** 0xBC bytes (A0h before). +A0h SPOT (x, y, z, 1), +B0h VIEW, +B4h/+B8h cos R and sin R. They are written only when all the keys are found, before TABLE is set, so they are valid whenever TABLE is 1. Patch 47's state (the left button's press) moves from +FA0h to +FC0h of the page.
- **Room.** The code is 2 779 bytes (2 382 before: `spot`, `find`, `turn`, the three keys and π/2), past the end of `.srpw`'s first page. Patch 47's code moves from `.srpw`+1000h to +1200h (0xA42B200; patch 44's `-DDECALS` with it); it still ends before the upgrade preview's page (+2000h). Patch 45's own address and entries are unchanged (patch 32's `-DCAMERA` is the same).
- **Checked:** `check_weapon_table_026g.py` 56 checks (28 as is and 28 rebased). Its lobby menu now has a project and a real binary config, laid out in emulated memory as the engine keeps one (24-byte records, offsets as pointers), holding the classic's `srp_lobby`; check_weapon_preview's lobby has the same, and its camera's +88h is its menu. Every earlier case passes unchanged with it. New:
  - **another lobby's spot:** four spots seen at 0, π, 3 (an int) and 0.7. The camera's point lands exactly on the spot, its yaw is the nearest turn of the view, and the eye is at the bench's distance along the view's forward. The weapon's middle is drawn at the spot with rows rotY(spin) rotZ(tilt) rotY(R), orthonormal; unspun, its length is level and across the view.
  - **no spot:** eight refusals (no project, no srp_lobby, srp_lobby not a table, no weapon, no weapon_view, a float or a float2 weapon, a text weapon_view). In each there is no flight (the point stays at the origin, the yaw and zoom as they were), the held item is drawn with the hand's transform, and the state's spot is untouched.
  - `check_weapon_decals_026g.py`, `check_weapon_preview_026g.py` (40), `check_upgrade_preview_026g.py`, `check_weapon_camo_026g.py` and the self-test's checks 44, 45 and 47 pass. Check 45 adds the engine's read of the project's config and the two config functions as the code's only calls outside itself.
- **Not run in a game.**

**Round 7** (2026-10-30, the lobby audit: the owner, of the new lobbies, "camera movement for weapon upgrade table"; `lobby-scenes.md`, "The lobby camera"). The flight is the Rust patch `weapon_flight` (`rust/patches/src/weapon_flight.rs`, one lohook, `lobby_camera.view` 0x2840F2); patch 45's NASM keeps the weapon at the bench. PROVEN in the code, emulated with the game's tick:

- **What was wrong.** The flight (rounds 5..6) glided the point, the yaw, the pitch and the distance 5·dt of the way left a tick, with no collision ray (round 5c): an ease-out from full speed that landed after about 1.5 s. Back, it glided the distance to the wheel's *target* (up to 4.6 m), out through the walls of a small room, and the first stock tick's ray snapped it back: up to 2.6 m in the classic (through the floor, a pitch of 0.6), 3.1 m in the camp and the bunker (the static audit, 72 starting poses a lobby). There, the eye's path crossed a wall or a prop in 5..27 of them.
- **The flight now.** One enum, `Flight` (stock, to the bench, at the bench, home, landed), one transition function `Flight::step` over two events (a request, a tick):
  - the eye moves on a straight line from where it is to where it goes, the yaw (its nearest turn), the pitch, the distance, the side angle and patch 22's zoom with it, by one smoothstep over 0.45 s (`DURATION`): no cut, no start or stop at full speed; the point is where that view looks from that eye. Where the line comes within 0.9 m (across the floor) of a character's spot (the own at the origin, the squad's at (+-1, 0, 0.5)) the eye rises smoothly over the head, to 2.25 m (`lift`, none at either end): the camp's spot is behind the character and the first game capture of this round flew straight through it. Both ends' eyes are in the room; a straight line between them crosses nothing where nothing stands between them (the audit: from the poses whose eye is clear, the line back crossed a surface in 4..21 of 144 starts where the glide's path did in 4..67);
  - back, it goes to where the stock camera will be: the origin's point and the yaw and pitch kept, the distance its wheel's target cut by the camera's own ray there (`engine_lobby_camera_ray`, 0x2844D0, cast once at the request) and clamped to 1.5..4.6 as the tick's last clamp does; it lands there, holds one tick more with no ray (`Landed`: patch 32's min_final sees the distance at the stock's least and gives 1.5 back), then gives the camera its steps again (speeds 1, 1, 3): the stock ray then cuts it where the flight put it, no snap;
  - at the bench the distance follows its target (the weapon's framing, the wheel) by patch 45's glide (5·dt of the way and 0.01·dt).
- **The interface** (patch 45's state, `fields_026g.tsv` `skhid::weapon_table` at its STATE, `.srpd`+F00h, now an `::image` row: the image range takes `.srpd`): the NASM writes TABLE, the targets (TARGET_X/Z/YAW/PITCH/ZOOM) and REQUEST (+2Ch, until round 6 EASE: +1 at each `camera` request, the table on, again or off); the flight writes FLIGHT (+4, until round 6 FLY: 0 the stock camera, 1 moving, 2 resting), which the NASM's `camera` (the yaw and pitch kept only from the stock camera), `tick` (the drag and the framing only with the table on) and `ray` (no ray while not 0; the wheel's farthest on the target) read. The NASM's glide, its tables and constants are gone (2 515 bytes, 2 779 before); its `ray` no longer clamps the distance itself (the flight's glide follows the clamped target: the review of 5e's snap cannot come back).
- **Checked:** `cargo test` (`weapon_flight/tests.rs`: the straight line, eased at both ends and exactly there; back exactly at the stock pose, a tick held, then released; a request mid-flight from where the camera is; the bench's distance by the glide, a framing mid-flight without a jump; the home goal's ray and clamp; every sequence of seven events of four kinds finite, `Leave` only from the stock camera); `check_weapon_table_026g.py` 29 checks, as is only (the flight is the arena's, linked at its addresses: the exe is never rebased): the round 6 cases with the new timing (the bench within 1 cm after 0.43 s, exactly at 0.45; back in 30 ticks of 16 ms), the eased moves there and back, the ray's hit at 0.6 (no ray but the flight's one at the close), the wheel and the framing glides, and new: **the close in a room** (the wheel at 4.6, a wall 2.2 m behind the point: the flight back lands 2.2 m from the point, and no tick after the landing moves the eye). Selftest check 45 (round 7's code to its data) and the new `test_weapon_flight` (the site's bytes, the frame's time at ESP+10h, the interface row at patch 45's STATE).
- **The captures' camera sweep** (`camera_sweep.rs`, the site's chain after the flight): a diagnostics build (`--ragdoll-trace`) with `srp_camera_sweep.txt` in the user data folder puts the camera at a script's poses (`hold yaw pitch distance` a line), each start a line of `srp_camera_sweep.csv`; never a release's (the switch off in every build, as patch 76's). A line `empty` (2026-10-31, the lobby window's pictures) has the lobby's three characters (the menu's +694h, +698h, +69Ch) drop their players with the game's own release_player 0x284AA0 (ESI the character, a byte at ESP+4, `ret 4`: the player's vtable+24h(1) and +28h, the reference released, +D2B8h 0; the byte 1 sets +D2C9h, so on_player_ready lets a player being made go too, 0x284F90), so nobody is drawn; the camera and the light are the scene's as ever.
- **Not run in a game** with input (the drag and the window's open by the movie): the flight's game captures are the sweep's poses.

**Round 8** (2026-10-31, the owner's test of #778: "in room camera does a jump when going to a table, most likely because it hits a character"). Round 7's eye rose over a character in a bump, `(2.25 - y) * near^2 * min(1, 6 s (1 - s))` with `near` from the eye's distance to the character across the floor: where the line passes the character (the Room's, 0.2 m off), that bump went up to 0.8 m and back within about a tenth of a second at the flight's fastest, the eye's vertical speed changing by 0.3 m a frame (at 60 frames a second): the jump. Not the ray (patch 45 casts none while the camera flies) nor the 1.5 m clamp (the flight sets the distance). Now `weapon_flight::arc_height`: one arc over the whole flight, `height * 4 s (1 - s)`, its height the least that still takes the eye over every character the line passes (the bump's rise at the line's nearest point, divided by the arc's share there, at least a quarter), capped so the arc's top stays under 2.5 m: 0.03 m a frame at most. PROVEN in the code; `weapon_flight` tests: the four lobbies' spots, twelve starts round the orbit, there and back, every tick's change of velocity within the smoothstep's own and 3.5 cm, over the heads on the way. A diagnostics build (`--ragdoll-trace`) writes every pose the flight gives the camera to `srp_weapon_flight.csv` (the tick, the eye, the point, yaw, pitch, distance, the state). Game captures: lobby-scenes.md.


### 3.43 On the death screen a drag turns the camera; the wheel does nothing (written 2026-10-21, README patch 46, the next release's revision; the rules: `death-camera.md`) — PROVEN in the code, the mouse moves and the view's update run with Unicorn; the first build run in a game

The owner: "when player dies they can still move camera around, with holding a mouse button? Scroll shouldn't work though".

- **The camera after a death** (PROVEN). `player::on_player_death` (0x1F28E0: remove_alive 0x2D99C0, +33Eh/+33Dh, the effects, the actions queue) sets no mode of the player's input handler. The handler's mode (+358h) is written by its constructor (0x23FDBD) and the game world's switches alone: `switch_to_warmup_camera` 0x1F8190 (3), `switch_to_player_camera` 0x1F8200 (0 for 1, 2 for 0), `switch_to_free_fly_camera` 0x1F8270 (1) and 0x1F8360 (4, `switch_to_spectator_camera`, called by the network clients' `spectate_next_player` 0x242D50 and `spectate_previous_player` 0x242F00, vtable+64h/+68h). The calls with 0 (third person) are base_local_network_client's and replay_network_client's (vtables 0xA074BC, 0x9FF344, +80h..+8Ch: 0x26DEB0, 0x26DF70, 0x26E010, 0x26E0D0). So a death in play leaves mode 0. `player::update_camera` (0x1F3480, from `player::mt_draw` 0x247847, ebx = the player) gives modes 0 and 4 the eyes' transform (player+44Ch) and the others the body's (+40Ch) raised by 1.4 × its up; then `update_inverted_view` (0x2422B0: the handler and the transform on the stack, `ret 8`) copies the transform to the view (+48h) in modes 0 and 4, and otherwise takes its third-person step (0x2422DE): the transform × a rotation about the world's up by the yaw (+34Ch; 0x15AE50, the product 0xEE700), × a rotation about the new right (0xEF370) by the pitch (+350h) (both shown by `check_death_camera_026g.py`'s geometry runs), the place moved by the distance (+354h) along −forward + 0.2 right; it reads nothing else of the handler and writes the view and +35Ch alone. The death camera is the corpse's eyes, first person.
- **The input after a death** (PROVEN). The hud shows the kits and the cursor when the local player turns dead (0x2383B0, `show_cursor` 0x23A930) until the respawn (8 s in every mode's options, `static_info.md`). The game world's input handler (priority 7) answers 1 for every mouse move while the cursor is shown (0x25D0E0, §3.29), so the player's handler (priority 10, 0x240200) gets no move and no wheel; in mode 0 it would add the wheel to +348h, which only the third-person and warmup steps use (0x242200). `game_world::tick` builds the local player's input from the handler (+338h, +33Ch) and sends it only while the player is alive (`cmp byte [eax+33Ch],0` / `jz` at 0x1F7883): a dead player sends no input.
- **The patch** (`DEATH_CAMERA_*` and `DEATH_VIEW_HOOK` in `patch_026g.py`; `death_camera_026g.asm`, 860 bytes, the section `.srpc` at 0xA429000, the page after the late join's state page; its turn, a yaw and a pitch, at the state page +E60h, after the FOV LOD scale's). The `.srp` page has no room left (61 and 60 bytes free between its pieces): a section of its own, as #225 and #244 add theirs (`add_code_section`, the same helper).
  - `WARMUP_LOOK_HOOK`'s jump goes to `death_look` (`.srpc`+0) instead of warmup_look. With the kits shown (game world+4DBh, hud+2DFh), the handler (game world+4D58h) in mode 0 and the local player (game world+9Ch → the game, +53ACh → the network client, +4) dead (+33Ch = 0; every pointer tested for null), a mouse button held is a drag as warmup_look's, with warmup_look's own state (+E20h: the moves of the left button, the pointer at the start) and its end (`drag_end`, warmup_limits+0): the lobby's buttons (52h) at once, the left one past 8 units; the pointer off the screen (game world vtable+30h, −128, −128) and back at the end (vtable+3Ch). A drag's move: the window's size (game+5348h, vtable+1Ch, w and h, as the lobby camera's 0x283E0C), the yaw −= x × s, the pitch −= y × s × h / w × 0.95492965 (×−1 with the option mouse_invertion, 0x1F1D9D3), s = the option sensitivity (0xA74AEC) × 0.1 × π/180, the player's look's (0x24026C..0x240353); the yaw within ±70°, the pitch within 60° up and 40° down (`minss`/`maxss` with the limits: a not-a-number becomes the limit); answers 1. A move without a button (the wheel's too) ends a drag and goes on to warmup_look, which gives it to the original (mode 0): the cursor's. Every other move goes on to warmup_look as before (`jmp` 0xA425D40).
  - update_camera's `call 0x2422B0` at 0x1F364B (five bytes, no address under it, nothing aims inside) becomes `call death_view` (`.srpc`+200h). While the player (ebx) is dead, the handler in mode 0, the kits shown (handler+334h → the game world, +4DBh) and the turn not 0 (−0.0 counts as 0): it keeps the handler's +34Ch, +350h, +354h on the stack, writes the turn's yaw and pitch and a distance 0, runs the update's own start (`sub esp,0D4h` … `push edi`) on the identity (`.srpc`'s own) and jumps to its third-person step (0x2422DE), which writes the turn L to the view (+48h), then puts the three back; then each of L's three rows becomes x E.i + y E.j + z E.k (`shufps`/`mulps`/`addps`, E the eyes' transform) and the place E's. A live player: the turn is cleared, then the original update; any other case the original update.
- **The turn follows the head's pose** (the owner after the first game test, 2026-10-21: "sometimes you can see inside of models"; "still orbit around eyes but take a pose into consideration"). The first build ran the step on the eyes' transform itself: the yaw about the world's up, which with a rolled or lying head swung the look into the corpse. E (player+44Ch) is base_player's character head transform, every frame the head bone's matrix times the body's (0.10b `animation::calculated_head_matrix`, called by the active object, `weapon_core.cpp`, `empty_hands.cpp`; in 0.26g detect_usable_object's ray starts at its place +47Ch along its forward +46Ch, §3.23). L × E turns the look about the head's own up, then its turned right: in the head's frame the look is (−sin yaw cos pitch, sin pitch, cos yaw cos pitch) (`check_death_camera_026g.py`; the game's step turns a positive yaw left and a positive pitch up), so the limits hold a cone in front of the face whatever the pose, with no table of poses: 70° to either side, 60° up, 40° down (the chest's front is about 70° below the eyes' forward; to be tuned in the game). The head is not drawn: mt_draw (0x24781E..0x247841) gives the local player's `set_head_visibility` (0x1F37A0) 0 in modes 0 and 4 every frame, dead too (its models 0, 5, 6, 7 get 2, not 3), so the eyes do not see the face from inside and the eye point is not moved. An orbit was considered (the lobby camera's wall test, `lobby_camera::process_collision` 0x2844D0 with `bullet_physics_world::ray_test` 0x75E20, the physics world at player+350h) and dropped by the owner.
- **Nothing a match computes:** the handler's input is untouched (a drag's moves never reach it; a dead player's input is not sent anyway), the view is the camera's alone; the warmup fields are the warmup camera's, put back in the same call, and in mode 0 nothing else reads them (process_warmup_mode 0x242200 runs in mode 3 alone, or in mode 2 with Alt held). No row in `simulation_changes.tsv`; replays unchanged.
- **Builds:** without a server sha256 `d824139f7a9a07f99dcd4a6a9f36b05d3cb70ba112883d2d05661dd1b791cbe4`, with `--server=skhid.link:1234` `9bf3c262260a32eb9d7f6e46de2b330bf87f03fea9533592ca64ad58f2059c6b`, with `--revision=27` `f50a062bf26ea919103d2e0b3e5ab6fbe61f18ab15e3955eab89eb44dd7309e7` (12 908 152 bytes, the revision byte REVISION's). Against the build of `main` ecb9916 (`f02c1534…1d2c`) the first differs in the headers (the section count, SizeOfCode, SizeOfImage, the checksum, the certificate's offset, the tenth section entry), the two sites (file 0x1E2A4C..0x1E2A4F, 0x24C4E1..0x24C4E2) and the new section's 1 024 bytes before the certificate, which moved behind them (compared byte by byte).
- **Checked:** `selftest_026g.py` check 46 and check 15's layout (`.srpc` after `.srpd`, then `.srpw`: eleven sections), check 32 (the jump to death_look); `check_death_camera_026g.py` (Unicorn): 122 runs, eight broken builds fail. **Run in a game** (the first build, world-up turn, 2026-10-21): the drag worked, the look reached into the corpse at some poses. **Not run:** this build (the head-relative cone, its limits).

### 3.44 A weapon wears a decal, placed on it above the workbench (written 2026-10-21, README patch 47, the next release's revision; the rules: `weapon-camo.md`) — PROVEN in the code and the data, the bake, the editor's five sites and the start's index run with Unicorn (with patch 45's own transform); not run in a game

The owner's request (2026-10-21): "implement and allow put two decals on weapons" (before: "it should be nice if I could put a decal on the weapon too"); then the server gives a weapon one decal slot (0): the client keeps three upgrade records an item, and a scoped weapon holds its scope and camouflage besides (the owner, 2026-10-21: "let's remove one decal instead for now"); the code bakes up to two, as written (`weapon-camo.md`).

**How armour's decals work** (read 2026-10-21; PROVEN unless tagged):

- **The record and the config.** A decal is an upgrade record in slot 0 or 1 (items.md §2) whose u64 is the editor's value. The decal item's modification (`gameplay/items/upgrades/decals/char_decal_*.options`: modification 200101, type 3, path `data/decals`, value `[-1, {texture, "<low_param>", "<high_param>"}]`) appends an entry to the host config's `data/decals` (armour configs have an empty `data/decals`); the config merge puts the record's u64 halves, as integers, where the two texts were (0x3584E0: 0x35854C..0x3585D5, type 1, count 4); the merged config's name holds a CRC of the item's three records' u64s (0x2F14C0, `_%d`).
- **The u64** (pack 0x263B60, unpack 0x263D00; their round trip run with Unicorn): bits 0-9 yaw ((a + pi/2) * 162.97), 10-19 pitch ((a + pi) * 162.97), 20-26 roll (* 20.37), 27-30 size (* 32; unpacked / 32, clamped 0.05..0.5), 31-41 x ((x + 1.5) * 682.67), 42-52 y (* 1024), 53-63 z as x. The float4x4 (rows, v * M): right * size, up * size, forward * 1.5, the point - 0.75 * forward: a projector box 0.75 m either side of the point, up the world's up for a level forward; u64 0 unpacks as 0x76961E1C98074123 (a place on the chest).
- **The bake.** camouflage_decal_cook_parameters (P, ctor 0x119760): two decal layers of 270h at P+0 and P+270h, the camouflage's at P+4E0h (each a float4x4, then buffer_strings 8Ch apart: the diffuse +40h, `_nmap` +CCh, `_fmap` +158h, `_smap` +1E4h); P+750h is the model's transform for the bake: rotY(pi) for each of the profile's visual's parts (0x2045C6..0x20460D, the matches' bake), rotY(the lobby character's yaw, pi) in the editor (0x2646F3). So a decal's place is in the model's space turned by pi about y: the vertices as the `converted_model` files hold them with x and z negated (the `export_properties` bounding boxes are the vertices' mirrored in x and z: AK-12 vertices z -0.265..0.673, export -0.673..0.265). The baker's occlusion (0x1307B0) renders a 256x256 depth map along each decal's matrix (an orthographic view the size of its axes) with the model drawn at P+750h (0x12F5B0), and sets `decal_transform` and `local_to_occlusion` (baker+49274h, +49278h, 0x130C3C): a decal paints the first surface its projector sees. The textures pass (0x12E510) needs the `_aoac` only with a camouflage (0x12E8E9); decals alone bake on any surface. A match's character bakes its decals from its merged config (0x204560 fills the layers, 0x2050E5 unpacks), on every game.
- **The editor.** The movie's start_decal_edit (UpgradeWindow's "change" on a decal slot) -> 0x29A830 -> start 0x2636B0 (eax = the host): the edit on (+D514h), the host +D520h, the decal +D524h, its record's u64 +D548h (and +D550h for the cancel), its index among the host's records not in slot 3 (+D52Ch: its entry of `data/decals`) and its record's index (+D52Dh), the size from the decal's `default_size` (+D518h, clamped 0.05..0.4), the roll 0 (+D51Ch); then 0x264CF0 queries the host's merged config (callback 0x264EA0: +D528h, then 0x264F80 queries `data/decals[+D52Ch]`'s four textures for the preview, with no bounds check, and 0x2641A0 bakes the host without the edited decal). Each frame (the character's update, 0x284A8E) 0x265980 picks (0x264960: the mouse's ray 0x26D130 against the character's collision model, mask 20h; the point +D530h, the normal +D53Ch), packs (0x264A10, the direction the normal reversed), unpacks and shows the box as a scene decal (0x265860, id 999; taken away on a miss). The wheel with a modifier key (0x265CB0) changes the size by 0.05 (0.05..0.4) or the roll by 0.1 (wrapped). The click (key 151h, 0x265C20): 0x2641A0(1, 0, 0) picks again, keeps the u64 (+D548h) and bakes; its callback (0x2648E0) accepts (0x263950: the record's u64 in the inventory's copy, `46 05`, the markers) and calls the movie's `root.exit_decal_edit`. The cancel (cancel_decal_edit -> 0x29A850 -> 0x263A70): the record's old u64 back and baked again.
- **The server answers no `46 05`**, and the client writes only the inventory's copy (profile+4634h), not the kits' item table (profile+1568h) the lobby's character is built from: an armour shows its new place by the editor's own bake.

**What a weapon lacked**: `decal_available` and `data/decals` in its config (the data patch); a bake of decals (patch 44 baked a camouflage only); a place to pick (weapons have no collision model: a weapon's `.skinned_model` holds its render surfaces, bind pose and skeleton only), and the editor's bake goes to the character's model (its player's +F290h, surface +D52Eh); its index (+D52Ch) counts a weapon's camouflage record (slot 2) as a decal, which would read past `data/decals`.

**The patch** (`weapon_decals_026g.asm` at `.srpw`+1000h = 0xA42B000, 3145 bytes, the section's second page (+1200h = 0xA42B200 since patch 45's round 6, 2026-10-29); its state, 12 bytes, at `.srpd`+FA0h = 0xA428FA0 (+FC0h since then), after patch 45's: the left button's press; it reads patch 32's mode and item and patch 45's table, READY and published transform). It begins with seven `jmp near`:

| site | original | patched | code |
|---|---|---|---|
| patch 44's bake_weapon | its camouflage lookup | `call 0xA42B000` | `lookup`: `data/camouflage` (non-empty text, else "") and `data/decals` (an indexed or named table, at most 2): eax 0 when neither |
| patch 44's bake_surface | its camouflage strings | `call 0xA42B005` (after the name) | `fill`: the camouflage's layer; for each of the first two entries {texture, low, high} with a non-empty texture, the layer P + i*270h: its four names and the float4x4 of its u64 (`weapon_matrix`: 0x263D00, the point 1.0 lower, the two sides halved), and `_<texture>_<high><low>` (hex) after the name; a u64 0 or a value no integer: the decal's default |
| 0x265999 | `call 0x264960` | `call 0xA42B00A` | `pick`: for the weapon above the bench (below), the mouse's ray (0x26D130) in the weapon model's space (the inverse, by cofactors, of `weapon_transform`: bone 1's skinning of the frame times the transform patch 45 drew the weapon with; none until patch 45 has: a miss); the ray's point nearest the model's z axis, if in front of the eye, within 0.25 m of the axis and at z -0.6..1.4: the point (y + 1.0) and the normal (the ray reversed), al 1, `ret 14h`; else al 0 and nothing written. Otherwise the original |
| 0x2659B2 | `call 0x263D00` | `call 0xA42B00F` | `decode`: for the weapon, the preview's box is `weapon_matrix` times `weapon_transform`; otherwise the original |
| 0x265C20 | `push ebp / mov ebp,esp / and esp,-8` | `jmp 0xA42B01E` + nop | `click`: the character's mouse key handler (input vtable 0xA09F6C slot 2; ecx = the character, the code, the kind; `ret 0Ch`), which bakes the edit's place on the left button's release (151h, kind 1; 0 is the press, 2 held). In a weapon's edit: the press keeps the cursor (lobby menu +8Ch, +90h); the release goes on to the original (the place) only within 5 pixels of it, else al 0 and nothing placed (the drag turned the weapon: patch 45's left button); a release with no press seen (the movie's "change") places nothing. Otherwise the original |
| 0x2641A0 | `push ebp / mov ebp,esp / and esp,-8` | `jmp 0xA42B014` + nop | `bake`: for the weapon, the click (1, 0) picks, packs (0x264A10) into +D548h, accepts (0x263950 with the host and the decal) and calls `root.exit_decal_edit` (0x511600), al 1; a miss al 0 (the edit stays on); the start's and the cancel's calls nothing, al 1; `ret 10h`. Otherwise the three instructions and on at 0x2641A6 |
| 0x263938 | `call 0x264CF0` | `call 0xA42B019` | `start`: for the weapon, +D52Ch = the records of slots 0 and 1 before the edited one (the decals' order in `data/decals`), or, for a record of another slot, the edit off and no query; then 0x264CF0 |

- **The weapon's transform** (`weapon_transform`, PROVEN in the code, emulated): patch 45 publishes the transform it draws the held weapon with at its state's +50h (rows, v * M; a stamp at +48h), valid while +0 (TABLE) and +28h (READY) are 1. The drawn weapon is skinned: a vertex goes through invbind_i * pose_i of its bone, then that transform (update_render_matrices 0x15A590: 0xEE700 multiplies the model's inverse bind +140h by the bone of the frame, 0x58B70 transposes it into the render instance's +229Ch, 0x15A7EC..0x15A810). The decal is baked on the bind pose's vertices, so the vertices' space is reached through bone 1 (`offset`, the body's in every weapon of the data; the PM's idle moves it 20 cm): out = S1 * W, S1 read back from +229Ch + 40h (transposed) of the held weapon's render instance (player +128h, a `weapon` (vtable 0xA04174), +1810h, +108h, vtable 0x9F0F24), the identity when any of them is not there. Inverted for the pick by cofactors (no rigidity assumed).
- **The weapon above the bench:** patch 45's table on, patch 32's weapon shown, the character's upgrade window open (+D515h) and the edit's host (+D520h) the window's item (patch 32's state +4). Armour, the squad panels, a weapon window without patch 45's table: every site is the original's code.
- **A weapon's decal space** [srp]: the weapon's vertices' own space (P+750h the identity, as patch 44 leaves it; muzzle towards +z, grip at the origin), y + 1.0 in the u64 (its range 0..2; a weapon's -1..1), the size halved (0.025..0.25 m; the editor's start size 0.15 gives 0.0625). A decal never placed (u64 0): the first on the left of the receiver ((0, 0.05, 0.12) looked at from -x: the side the bench shows), the second on the right, both upright (0x8A30CE0020080000, 0x8A30CE0020080200, packed by 0x264A10 with Unicorn).
- **The pick is no hit test** [srp]: the point is on the ray where it passes the weapon's long axis; the decal's projector (0.75 m either side along the ray) and its occlusion put it on the first surface the ray meets. A click in the air within 0.25 m of the axis places a decal that paints whatever its box reaches (maybe nothing).
- **The new place on screen:** the weapon's bake is not done by the click; the server sends the account after a weapon's `46 05` (`lobby-server` items.rs), its kits' revisions changed: the lobby's refresh rebuilds the character (patch 32's kit or scratch profile), weapon_cook runs and patch 44 bakes the decals with the new u64. Until then the weapon shows its old decals (the accept takes the preview's scene decal, id 999, away: 0x263950); a game whose server does not send the account shows the new place at its next refresh.
- **A decal dragged from one slot to the other** (found 2026-10-21 in the owner's game, a Mosin, `weapon-camo-27/runtime/packets`: "move the same decal into another slot ... it doesn't appear any longer"; PROVEN in the packets and the movie, not in the exe): the movie sent `46 01` (the new slot) and `46 02` (the slot it left) in one datagram, in that order (CLIK's DragManager: the target's handleDropEvent, then the source's handleDragEndEvent, whose dropSlot gives UpgradeWindow.onItemUpdated a drop_upgrade_item); the server moved the decal and then took it off the weapon; every later bake had no decal. Neither patch 47 nor patch 48 is in this: the same happens to armour's decals. Fixed in the movie (onItemUpdated sends no drop for a cosmetic another slot of the window holds) and the server (a move to another slot of the host keeps the record's u64, its place, and answers it in `83 46 01`; the client's record takes the value from the answer). `check_weapon_decals_026g.py` adds the start's index for a decal moved to slot 1 (its record last); `items_tests.rs` the sequence.
- **A click places, a drag turns** (the owner's left-button drag of patch 45 round 2): see `click`; the press and the release are the input world's mouse kinds (0x3B9D90, section 3.29's warmup keys). Armour's edit is the original's (its release places, as before).
- **Relocations:** none under the five sites; nothing aims inside them (the bake's three callers 0x263B46, 0x264F63, 0x265C4C call its first byte; the key handler is reached through its vtable alone). The code holds absolute addresses (the states, its constants, the game's strings), as patch 44's (the exe is never rebased).
- **GUESS:** that the body of every weapon is bone 1's alone (a part on another bone, a magazine, takes a click by the body's pose: off by what its own pose moves it). That the scene's decal (the preview) draws on a weapon. That the bake's occlusion and accumulation use P+750h alike (the identity here).
- **Limits:** a surface without `_aoac` of a weapon with a camouflage takes neither (the baker refuses the type); without a camouflage decals go on any surface.
- **Checked:** `selftest_026g.py` check 47 (the five sites in the original and the build without the late join, nothing aiming inside them, no relocation; the preview's, the start's and the key handler's code around them, the handler's vtable; the bake's three callers; the code's calls and jumps; the two pages of `.srpw`), check 44 (patch 44's calls include lookup and fill) and both `.section.bin` current; `check_weapon_decals_026g.py` (Unicorn, 202 checks; patch 45's own `matrix` run to publish the transform, a held weapon whose bone 1 is turned and moved 25 cm; the cases in its header); `check_weapon_camo_026g.py` (a camouflage alone as before) and patch 45's, 32's and the stash's checks on the merged branch (feat/weapon-camo e71eb9f7). **Not run in a game.**

### 3.45 An upgrade dropped on a weapon in its window is shown at once (written 2026-10-21, README patch 48, the next release's revision) — PROVEN in the code, the six sites run with Unicorn with the game's own setup_profile, profile copy and config lookups; not run in a game

The owner (2026-10-21): "can equipments in general be more responsive? I want the scope to be equipped and see the results a few frames after? Or camo?", "the server can be in the background".

**Where the time went** (measured offline: the local test's packet logs and the game's own log, `private/local-clients/weapon-camo-27/runtime/`; the static reads below):

| step | time | evidence |
|---|---|---|
| the request waits for the lobby client's send tick | 0..505 ms, ~250 ms on average | every client packet of the nine lobby sessions logged on 2026-10-21 comes ~500-510 ms after the one before (some 9 400 packets: their intervals are 0.5 s or 1 s steps, and immediate acks); a request carrying a message leaves at the next tick: the drop's `46 0E` at 08:51:37.974 came 508 ms after the client's previous packet (37.466) |
| the server | 2-3 ms (local; a network's round trip live) | `46 0E` in at .974, `83 20 46 01`, the inventory 0x89 (9 parts) and the account 0x87 out at .976-.978 |
| the character's player made again | not logged by the game | setup_profile 0x284B10 → query of `default.player` (class 54h) → on_player_ready 0x284F20; patch 48 now writes "srp: lobby character's player made in <n> ms" |
| a camouflage's bake (the first time a weapon wears a camouflage) | ~40 ms of work, ~150 ms from request to the last texture | the game's lines "camouflage/decal cook [ bake 15.4 ms, compress 4.2 ms, 2048x2048, diffuse ]", normal 3.0 + 4.4, smoothness 2.8 + 3.7, a second surface's three at 1024x1024 ~1.4 + 1.7 each (11:18:23.914..23.978, after the request at .831); later the files are read from `streamed_textures` |
| "cook of '' [quid n][class 107] failed" | none | class 107 = 6Bh is `camouflage_decal_texture_type_baker_cook`: the class-6Ch baker (`camouflage_decal_baker_cook::translate_query` 0x134020) asks four of them per surface, one per type d, n, f, s (the names at 0x9EE7A4, the class at 0x1342E8); one fails per baked surface, at once, before the surface's three lines and without a retry: LIKELY f (fresnel, which no bake fills, section 3.41) |

So of what the game logs, the dominant wait is the client's own send tick (a quarter to a half second), then the round trip; the bake is not. The player's rebuild was not measurable from the logs (no line); the new log line gives it at the next game run.

**The patch** (`UPGRADE_PREVIEW_HOOKS`, `upgrade_preview_026g.asm` at `.srpw`+2000h = 0xA42C000, 1576 bytes, the section's third page; its state, A0h bytes, at `.srpd`+D60h = 0xA428D60, after the patches' mount's and before patch 32's):

| site | original | patched | code |
|---|---|---|---|
| 0x29A9B2 (relocate_upgrade_item's handler 0x29A8B0) | `mov edx,[esi+4620h]` | `call 0xA42C000` + nop | `relocate`: edi the host, ebx the upgrade, cl the slot, esi the account's profile, the handler's [esp+10h] the lobby menu; then the original's load. Before `46 01` is written (0x2707C0) |
| 0x29AACF (drop_upgrade_item's handler 0x29A9D0) | `mov ecx,[esi+4620h]` | `call 0xA42C005` + nop | `drop`: edi the upgrade, the host [[ebp+8]+8]+8 (as patch 30 reads it); eax kept for patch 30's `46 02` |
| 0x270313 (the 0x87 reader 0x270260, called from 0x274B3F alone) | `call 0x270320` | `call 0xA42C00A` | `account`: the oldest queued change leaves the queue; then 0x270320 |
| patch 32's two `call FIND` | (stash +0Fh) | `call 0xA42C00F` | `find`: the stash's find_kit_or_show, then the queue applied (below) |
| 0x284B89 (setup_profile past its early return) | `cmp dword [ebx+0D2C4h],-1` | `call 0xA42C014` + 2 nops | `again` (below); the original's comparison for its `je` |
| 0x284F7F (on_player_ready) | `cmp byte [ebx+0D2C9h],0` | `call 0xA42C019` + 2 nops | `made`: the log line, then the original's comparison (read by the `jnz` after a `mov`) |
| 0x284B7C (setup_profile's early return, added 2026-10-23) | `cmp byte [ebx+0D2C8h],0` | `call 0xA42C01E` + 2 nops | `early`: the flag for `jz 284CF8h`: the original's, but for the lobby's own character the early return only while no player is being made (section 3.42, round 4) |

- **The preview.** For the window's weapon (patch 32's mode 1, its item the host, the menu's own character +D515h): the change is queued (five at most: {upgrade id, the record's dict id, slot 0..3 or FFh "take off"}), a line "srp: upgrade preview: item <host>, upgrade <id>, slot <n>" is logged (the game's logger 0x111ED0 on channel "game", info, as 0x27B326 calls it) and setup_profile(the account's profile, the character, the chosen kit) is called. setup_hook gives it the weapon's kit and `find` the profile: for the item a change was queued on and the account's profile given (its inventory behind it), the stash's scratch (allocated once, as the stash's make_scratch, if a kit weapon never needed it), the account's profile copied into it unless the stash just did, the queue applied to the weapon's record in order as `items_rules::attach_upgrade_in` / `detach_upgrade` do (a put of the same id, slot and dict id leaves it with its value; else the id and whatever is in the slot come off and {id, dict id, 0, slot} is added when the record has room; a take-off takes the id off), and the kit's revision: the last given (state +08h) while the kit and the record's upgrades (17 bytes each: id, dict id, value, slot) are those it was given for, else the next `0x80000000 | n` of patch 32's serial. Once a change was queued on the item, the character is built so (the queue empty or not) until another item's window queues one.
- **A scope's record** holds the variant made for that weapon (srp's `ScopeFits`, `weapon_scopes.md`): the dictionary's entries (3A0h each from [dictionary+130h], their count +134h, by dict id) are read in order; the first of category 22 whose config ([entry+114h]+108h, a binary config: +0 the children, +16h their count, 18h each, sorted by the crc32 of their names at +10h; `value_exists` 0x0F6C00 and `operator[]` 0x0F6B10) has `srp_scope_fit` with `scope` naming the scope item and `weapons` naming the weapon (a string: the entry's name at +4; an integer: its dict id) is the record's dict id. None: nothing is queued (the server refuses such a scope).
- **The server.** Each account the server sends (0x87; srp sends 0x89 and 0x87 after every `46 01` and `46 02`, a refusal too: `items.rs` `item_changed`) answers the oldest change: it leaves the queue, the account's record has it or not. Confirmed: the weapon's upgrades are those shown, the revision is the same, setup_profile returns at its own early return (or at `again` while the preview's player is still being made): nothing is made again. Refused: the upgrades differ, a new revision, the character is made again from the account's. An account sent for anything else in between takes a change out of the queue early: the next refresh shows the account's until the answer comes (a flicker at worst).
- **`again`.** Stock setup_profile, when a player is being made (+D2C4h not -1), copies the profile into the copy after the one being made and sets +D2C8h, and on_player_ready then makes that one too (0x2850C5), whatever it is: the account confirming a preview while its player is being made made the same player twice. For the lobby's own character, `again` returns from setup_profile (to 0x284CF8, +D2C8h cleared, [ebx+4] written as the original's next instruction does) when the copy being made (the shown one's next, +D2B4h + 1) has the asked kit's id (copy+754h + kit · 704h) and revision (copy+45FCh + 4 · kit) and that kit is the one its query started with. With no player being made it keeps the lobby's time ([menu+9Ch]+584Ch, ms) and the kit at the query's start, for `made`.
- **What it leaves alone:** what is sent to the server (the handlers send as before, after the preview); the account's profile and inventory (never written: the check compares them byte for byte); armour windows (patch 32's mode 0), the squad panels (`again` and `made` look at the menu's own character only), a match.
- **Relocations:** none under the five sites; nothing aims inside them (selftest). The code holds the absolute addresses of its state, patch 32's and its strings (the exe is never rebased).
- **Checked:** `selftest_026g.py` check 48; `check_upgrade_preview_026g.py` (Unicorn 2.1.4) on a made-up account (three kits, the item table with records whose upgrade vectors are the profile ctor's, the inventory, the dictionary with variant configs built as the game's binary configs): a camouflage on a kit's weapon and a take-off made at once from the scratch, the account untouched, the handlers' registers and stack the original's; the confirming account while that player is made makes nothing (`again`), and after it the early return; a refusal makes the account's again (waiting for the one being made); a replace in slot 2, a full record; scopes by name (the first variant of two) and by dict id, a weapon only in the stash, no variant; not the window's item, mode 0, no window, an unknown upgrade; a squad mate's character and the 0x87 site with nothing queued as the original; `made`'s line (340 ms on the made-up clock), its flags and registers. The other weapon checks (`check_weapon_stash`, `_preview`, `_table`, `_camo`, `_decals`) pass. **Not run in a game.**
- **Left:** the player's rebuild itself (setup_profile → the player's query → on_player_ready) still runs for every preview; if the new log line shows it long, the next step is to rebuild only the weapon (weapon_cook for the held item) when only its upgrades changed. A camouflage seen for the first time on a weapon still bakes (~40 ms of work, ~150 ms to land); lowering the bake's 2048² would not gain much. Armour windows have no preview.
- **Builds** (this branch, on feat/weapon-camo e1aa4878): without a server sha256 `0e463efb518b457290ba3d84c408992a82fe205918ad83c0d1ccbf556a07229e`, with `--server=skhid.link:1234` `74122ffcf45a2fd156469af299e5dc35744f5970f50c77c42b338d7cbe5e4d80` (12 917 368 bytes each).

**Round 2** (2026-10-21, after the owner's game run of feat/weapon-camo 10e3fe00: the decal on a Mosin moved and edited; `private/local-clients/weapon-camo-27/decal-move-1234/`):

- **No srp line in the log** (PROVEN in the code; the sites, the jumps and `.srpw`+2000h were in the game's exe). The game's lines go through a function: every caller builds `{0x9DAF4C | 1, 0, [0x1F1D614]}` (a boost::function: its manager, then the sink [0x1F1D614]; empty when that is 0) and passes it first to 0x111ED0 (relocate_upgrade_item's own "ui: attach_upgrade", 0x29A8E9..0x29A958); 0x111C90 keeps it in the line's record and the line is written through it. `log` passed an empty one: nothing was written. It now builds the game's (trivially copied: nothing to destroy after, as the callers' `test al,1` skip). The check's log stub now takes a line only with that function. 1596 bytes.
- **The decal moved to the other slot and gone** (PROVEN in the packet log; not patch 48's). The movie's move is `drop_upgrade_item(host, decal)` then `relocate_upgrade_item(host, decal, 1)` in one frame (the game's log: "ui: detach_upgrade[201] from [212]" then "ui: attach_upgrade", 12:33:21.060). Both left in one datagram (10:33:21.146Z), **`46 01` (unique id 22h) before `46 02` (21h)**: the client's send queue is sorted by size and popped from the back (transport.md §2.5), so a tick's messages leave largest first. The server took them in datagram order: the attach (the decal moved to slot 1, `detach_first`), then the detach (the decal off the Mosin); the same at 10:32:23 (0Dh before 0Ch). Patch 48 queued them in the call order (off, then into slot 1) and showed the decal in slot 1 until the two accounts came without it. Fixed in the server's transport (`udp-transport` `in_send_order`): a client datagram's unique messages are taken in unique-id order, the call order.
- **Decals and patch 48:** a decal dropped on the window's weapon is previewed as any upgrade ({decal, its dict id, 0, slot 0 or 1}; a put of the same decal into the slot it holds keeps its place). A place (`46 05`) is not previewed: the editor writes the value into the client's copy itself; the account the server sends after it (a weapon's) answers no queued change of the window only if none is queued (otherwise one leaves the queue early: a flicker at worst).
- **Builds** (round 2, on feat/weapon-camo 10e3fe00): without a server sha256 `1c2651913f62b72a24f9b0d6b4a563d7825194df04bafaf590293349a74d4539`, with `--server=skhid.link:1234` `4ab70c800845ae9b5fcb1f6cb1e48324cb4069be6a15f103315be3f11d1685d8`.

### 3.46 The game stops at its start without a patch its release requires (written 2026-10-21, README patch 49, the next release's revision; the rules: `required-patches.md`) — PROVEN in the code, the mount run with Unicorn; not run in a game

**Why** (the crash analysis of 2026-10-21): players whose updater could not reach the patches server started the revision 27 exe without the launcher and without patch_26/27. Patch 27 (section 0.2) skips a missing number, so the game ran on older content than the server: a profile with an item id its data did not have (crash A, a buffer_vector assert at the match's start), and other animation ids than the other players' in the world snapshots (crash B).

**The patch** (`mount_patches_026g.asm`, 759 bytes at `.srp`+400h, from 707; no new site): the probe's missing case. Before, `cmp eax,2` / `je .probe` / `cmp eax,3` / `je .probe` / `push eax` (the fatal's). Now:

```
0A425508  cmp eax,0x2
0A42550B  jz 0xa425512                 ; not there
0A42550D  cmp eax,0x3
0A425510  jnz 0xa425522                ; there, unreadable: the fatal
0A425512  mov edx,[esp]                ; the delta
0A425515  mov ecx,[esp+0x8]            ; n
0A425519  bt [edx+0xa4256d3],ecx       ; needed: bit n of byte n/8
0A425520  jnc 0xa4254c4                ; not required: the next n
0A425522  push eax                     ; the error: 2 or 3, or another
          ... the "Cannot open resources.db.patch_%u (error %u)" fatal (0x113580), as before
```

`needed` (0xA4256D3 as built, 32 bytes) is right after the "Cannot open" text and before the state's address that ends the .bin; a register `bt` on memory takes the bit string from that byte (n 1..255, so bytes 0..31). The probe runs n from 1 up before anything is queued, so the first required patch missing stops the game before any mount, with the error CreateFileA gave (2; 3 when a folder of the path is missing). Nothing else of patch 27 changes: a required patch that is there is mounted as any; one that is there and not required too; a missing one not required is skipped.

**The build:** the checked-in .bin's bitmap is zeros (`patch_026g.py` checks it, and the text before it), so a build without `--required-patches=` requires nothing. With it, `build()` writes `required_bitmap(numbers)` into its copy of the .bin; only those bytes and the PE checksum differ (selftest check 49). `required_patches_of(exe)` and `release/lib.sh`'s `exe_required_patches` read it back by finding the text (once in the exe) and the 32 bytes after it. A release: `release/build-zip --patches <list>` passes the numbers of every row of the offered-patch list (release.sh gives it its list after `upload-patch` added the new patch), checks both exes; `release.sh` and `release/deploy` refuse a ZIP whose exe requires other patches than the server data offers.

**Builds** (this branch, on main 7f010cc1, no server): the default build `8f75d355eda63a17d7afc83dd99ecff7d368e07801771c1f5e72f7d62c84c27b`; with `--required-patches=12,14,17,19,20,21,22,23,24,25,26,27` (revision 27's list) `33c7cba9bb1e1558b97ef43be5296f45d963174f7597f21c6f5a6e76ac3c0cc6` (12 918 392 bytes each).

### 3.49 `stage.focus` reads null for a focused object without its ActionScript object (written 2026-10-22, README patch 52, the next release's revision) — PROVEN in the code and Scaleform's source, the getter run with Unicorn; not run in a game

**Why** (the owner's local game, 2026-10-22 04:44, `Survarium-#2777_261022-044410.dmp`, r28 of main): `ACCESS_VIOLATION` reading 0x14 at 0x77200B, in the lobby, about a second after an upgrade. The minidump holds the threads' stacks only (no modules, no heap); the crash report's "frames" are not a stack walk. From the crashed thread's stack:

```
0x767FF258  0x775125   the Stage focus getter's thunk (0x7750F0), after `call [0xAFF180]`
0x767FF270  0x8F1E40   its ThunkInfo ("focus" at 0xA09EA8)
...         0x78E271 .. 0x7C7E12   the AS3 VM (LobbyMenu.has_active_window)
0x767FF698  0xA07404   "root.has_active_window"
0x767FF8B8  0x511644   the movie's invoke (0x511600)
            0x26CBFE   lobby_menu's key handler 0x26CB80 (0x26CBF9 `push 0xA07404` / `call 0x511600`)
```

`LobbyMenu.has_active_window` reads `stage.focus is DropdownMenu`; the exe invokes it on every key in the lobby.

**The getter** (0x771FA0 = Scaleform's `Instances::fl_display::Stage::focusGet`, the Scaleform GFx 4 source outside the repo (`scaleform-gfx/Src/GFx/AS3/Obj/Display/AS3_Obj_Display_Stage.cpp`); the static initializer 0x865D10 puts it in 0xAFF180): `[this+14h]` +40h +1A8h +8 = MovieImpl; `lea ecx,[MovieImpl + 3B64h + 40h * byte [MovieImpl+3F54h]]` = `FocusGroups[FocusGroupIndexes[0]].LastFocused`, a weak pointer locked by 0x514370; `[obj+41h]` the AvmInteractiveObj interface's index, its vtable +4 minus 1Ch the AvmDisplayObj; `GetAS3Obj()` = `[avm+8]` (pAS3RawPtr) or `[avm+4]` (pAS3CollectiblePtr, bit 0 cleared, the collector's mark); `[as3+14h]` the traits, `+3Ch` the type: 12h or 17h and up an InteractiveObject (0x77201B, the result via 0x790E40), else null (0x772031). The source tests `focused` but not the object (`IsInteractiveObject(ido->GetTraitsType())`); `FocusManager.getFocus` tests both.

**When the object is gone:** `~DisplayObject` (the AS3 side) calls `AvmDisplayObj::ReleaseAS3Obj` (both pointers 0); a display object then lives on while anything holds it (during a collection its release is delayed to the collection's end). AS3's `removeChild` (`AvmDisplayObjContainer::RemoveChildAt`) does not touch the focus; only `InteractiveObject::OnEventUnload` calls `MovieImpl::ResetFocusForChar`. Which object it was at 04:44 is not in a stack-only dump (esi 0x0D4386A0).

**The patch** (`STAGE_FOCUS`, every full build: with and without the late join and over on_hash_mismatch), 46 bytes at 0x771FED:

```
00771FED  test eax,eax              ; the interface call gave none
00771FEF  jz 0x772012
00771FF1  mov ecx,[eax-0x14]        ; raw pointer (+8 of eax-1Ch)
00771FF4  test ecx,ecx
00771FF6  jnz 0x771ffb
00771FF8  mov ecx,[eax-0x18]        ; collectible pointer (+4)
00771FFB  and ecx,byte -0x2         ; bit 0 cleared, as the old `test al,1` / `dec eax`
00771FFE  jz 0x772012               ; no ActionScript object
00772000  mov eax,ecx
00772002  mov ecx,[eax+0x14]
00772005  mov ecx,[ecx+0x3c]
00772008  cmp ecx,byte +0x12
0077200B  jz 0x77201b
0077200D  cmp ecx,byte +0x17
00772010  jnl 0x77201b
00772012  jmp short 0x772031        ; null
00772014  7 x nop
```

The old bytes' jumps all landed inside them; nothing else aims inside; no base relocation under them. ecx only; eax at 0x77201B the object as before.

**Checked:** `selftest_026g.py` check 52; `check_stage_focus_026g.py` (Unicorn 2.1.4): the original faults at 0x77200B with both pointers 0 (or the collectible 1) and at 0x771FF8 for an interface call that gives none; the build gives null; every other case the same in both.

### 3.50 Research, mode 5 (written 2026-10-22, README patch 53, the next release's revision; the rules: `research_mode.md`) — PROVEN in the code, the hooks, the cook, the rule and the hud run with Unicorn with the game's own register_cook, rule constructor and config look-ups; not run in a game

- **Rule cooks** (PROVEN): each is a static object `{+0 vtable, +8 class, +10h -1, +14h
  GetCurrentThreadId(), +18h 8, +20h the game (client cooks)}`, made by its function (tdm's
  0x2F41E0) and registered by `register_cook` 0x102D10 (`edi` = the cook): a vector at
  [0xA86E00]..[0xA86E04] grown to 0x206 places on the first call, `cooks[class] = cook`. The
  game's constructor makes the client ones and swaps their vtables (0x1E99C0..0x1E9B4C; tdm's
  client vtable 0x9FD8AC; ebx = the game); the last registration is at 0x1E9B4C (`mov eax,ebx /
  call 0x1EC170`). Classes in use found at the registrations: 0x0D..0x82, 0x12C..0x141, 0x200,
  0x202, 0x203 (a few set elsewhere); 0x1E0 is in no registration and no query.
- **A cook's create** (PROVEN, slot 9, thiscall, `ret 4`): client tdm 0x2063F0: `esi =
  [0x1F1D9C4]`, `ecx = 0xACCC60`, 0x420789 (`lea eax,[ecx+8]`), `push eax`, `eax = size`, 0xE4F90
  (allocate), the core constructor with the query (`[q]` the match options, `[q+0Ch]` the mode
  config's root), +130h the game; slot 10 the size. Slot 7 (0x2F4290) is the shared translate:
  it calls 9 with `[query+108h]` and 10.
- **The cook's RTTI** (PROVEN): tdm_rule_cook's locator 0xA47134 (type descriptor 0xACCC38,
  hierarchy 0xA47148: tdm_rule_cook, tdm_rule_core_cook, translate_query_cook, cook_base,
  boost's noncopyable at +20h, attributes 1). No code of `.text` names any cook's type descriptor
  (0xACCC38, tdm_rule_core_cook's 0xACCC0C, translate_query_cook's 0xAB8E58, cook_base's 0xAB8D2C)
  or the locator: nothing casts a cook or asks its type, so a borrowed locator would only misname
  it. The cook has its own all the same (`rn_cook_locator`): `.?AVresearch_rule_cook@srp@@`
  (type_info's vftable 0x886A78, the first word of every type descriptor), its hierarchy shaped
  as tdm_rule_core_cook's (locator 0xA524C0, hierarchy 0xA4718C: attributes 1, four bases) with
  that hierarchy's base class descriptors of translate_query_cook 0xA38D74, cook_base 0xA38D24 and
  noncopyable 0xA40010. The descriptor's second word (type_info's name cache) is written by
  type_info::name: `.srpx` is writable.
- **Rule vtables** (PROVEN, tdm 0xA15DA4, timelimit 0xA06E2C, kd 0xA177D4): 0 delete (0x266710:
  the base vtables back, 0xF1850, free), 1..6 the resource's, 7 `on_event(type, data, time)`
  `ret 0Ch` (a kill is type 1, `data = {killer, victim}`; 0 the match started; the world's
  dispatcher 0x2D1EA0 pushes the time `[core+18FC0h]`, then its second argument, then its first,
  and calls `[vtable+1Ch]`; the kill site 0x2D9EEE..0x2D9EF7 gives it `&{killer, victim}` and 1;
  tdm 0x316090 and kd 0x3237E0 read the data at `[esp+8]`), 8 / 9
  `entered` / `left(place, time)` `ret 8`, 10 register_animations, 11 `set_game_world_core(core)`,
  12 the blackboard (core+188B0h, 0x2D58F0), 13 `on_match_ready(time)`, 14 finished, 15
  `serialize(writer)` `ret 4`, 16 `deserialize(reader, ?)` `ret 8` (the snapshot's rules loop in
  0x2D2770, 0x2D2B42..0x2D2B5D: `push [esp+84h]` (its third argument), `push` the reader,
  `call [vtable+40h]`, no clean-up; tdm 0x3163D0, timelimit 0x317FD0 and kd 0x323910 each `ret 8`
  and none reads the second), 17 `fill_match_hud_state(hud, ?)` `ret 8`
  (tdm: +90h, +92h the tickets, +94h, +95h the alive counts; kd 0x323980 writes from +98h on,
  timelimit's 0x15A4B0 nothing; the hud state's constructor 0x278D00 sets +90h 0FFFFh, +92h 0,
  +94h and +95h 0FFh, and only mode 4's last players (0x23B3DE) read the alive counts: Research
  writes the points and leaves the counts, as modes 2 and 3 do), 18 `rule_completed(time)` `ret 4`
  (al), 19 the priority (0x8FE10: 3); then at +108h the link resolver's one slot (0x316540, `ret
  1Ch`). Base constructor 0x313900 (eax = this, push the type: 0 respawn, 1 gather, 2 timelimit, 3
  kd, 4 tdm; at +10Ch); the code compares the type with 0 and 1 only.
- **Ticks** (PROVEN): timelimit's set_game_world_core 0x318050 puts a delegate `{object,
  function, previous, next, order, enabled}` (rule+110h) at the end of the world's list at
  core+18850h (count, head, tail; order from the counter at [core+18FB0h]); atomic_tick 0x2D1E20
  runs the seven lists (core+18840h.., 10h each; 0x2D1D20 sorts by +10h) as `function(ecx =
  object, dt, time, step)` `ret 0Ch`, dt the time since the last (50 ms).
- **The world** (PROVEN): places at [core+18458h], 30h each, `[place + 4 * kit]` the player of
  kit `place[0Ch]` (tdm 0x3161F0); a player's team `[[player+6B8h]+45ECh]`, alive +33Ch, in the
  tick lists +33Fh, `m_transform` +40Ch (its translation +43Ch, read by the player's own code
  0x2D9148 and restored by its deserialize 0x2DB8B9: the feet, below), pitch +48Ch; the time rule's state copied at
  core+18FE4h (3 in play; written by its tick and deserialize); the game's own place
  core+18FDCh (client tdm 0x267852); the statistics `[core+18FA8h]`: a player's record 1F8h,
  `u16 events[28][3]` at +0Ch (the kill handler 0x2F4EB0), `m_team_points` +31D4h, the winner
  +31E4h (tdm 0x316257).
- **A player's transform is his feet** (PROVEN, 2026-10-22). A living player's tick takes his
  transform from his character controller (`[[player+494h]]`, 0x2D942A): `get_transform`
  0x7F5C0 copies the controller's matrix (+80h; its row 3 the capsule's centre) and moves its
  translation down by `heights[(stance + 2) % 3] + heights[stance]` (+1D0h float[3], the stance
  +1F0h) times up (0xA7DC00: 0, 1, 0), half the capsule's height; 0x74620 turns that into the map's
  frame and `rep movs` puts it at +40Ch (0x2D9461). The server's worlds keep the controller's
  centre, 0.9 m over the feet standing (player_bots.md §4), and `default.player` gives the capsule:
  0.76 m wide, 1.8 m standing, 1.1 m crouched. The first build took +43Ch for the capsule's centre
  and tested ±0.9 m round it: a player's feet up to 0.9 m over a unit's top counted, so at School
  the floor above units A and C (20.25-20.5 m, the boxes' tops 20.04 m) took them (the owner's
  test, 2026-10-22). 0.27d's units are physics triggers: `capturable_zone_core`'s enter 0x324350
  and leave 0x324420 get the colliding objects (each's owner, slot 1, the player) from the zone's
  trigger shapes (the collision geometries' meshes, half extents their scale, 0x7A1F0) overlapping
  the player's collision: no point, no height of its own. Now the capsule stands on the feet,
  2 x half its height tall (read from the controller as get_transform does; the config's
  `capture_half_height` without one), `capture_reach` round (0.38 m, half the width), and it is
  inside when the nearest it comes to a shape is at most its radius (the capsule's segment against
  the shape, a ternary search: the distance along a segment to a convex shape is convex); Bullet's
  collision margin is not counted.
- **Saving** (PROVEN): a writer's buffer at [writer+14h] (data +4, size +0Ch), grown by 0x7AB30
  (eax = buffer, push the size; `ret 4`); a reader's cursor at [reader+4] (tdm 0x3163D0). A rule
  writes field by field (tdm 0x316340) through `buffer_writer::w<unsigned char/short/int>` 0x1F3DE0
  / 0x2F5DA0 / 0x1F48A0: eax = the value's address, push the writer, the source file, the line, the
  function, the field's name; `ret 14h`. Each appends the value; with [0x1F1D32C] and [0x1F1D32D]
  set (the hash mismatch dump) it first records `{next, type name, name, function, file, line,
  size, writer+10h}` (20h bytes from 0x80450) at the end of the writer's list (+0 count, +8 head,
  +0Ch tail). Research's serialize does the same, 88 fields (`m_points[0]` .. `C.marks[19]`): the
  bytes `rr_write` gives (snapshot_rules.md 2.8) and each named in the dump; the first build
  appended `rr_write`'s bytes in one piece, which wrote the same bytes and nothing in the dump.
- **Config** (PROVEN): 0xF6B10 (eax = a table, edi = the key) gives the child (CRC look-up, then
  the names); 0xF6C00 (esi = the key) says whether there is one; a value is 24 bytes, its data a
  pointer for tables, strings and vectors, +14h the type, +16h the count (the respawn point's
  reader 0x2F4580).
- **The hud** (PROVEN): game_world_ui::update_ui (0x23B300..) shows the match's first message by
  mode at 0x23B336 (2, 3, 4: `st_start_match_welcome_message`, `_2`, `_3`; 0x2375C0, ecx = the
  key, `(ui, 21h, 14h, 6000)`, `ret 10h`: localised by 0x1FF0F0 into root.set_parameterized_
  message), then each frame the teams' points (0x2364C0 → `root.set_game_progress`) and, for mode
  4 only, `set_last_players` (0x23B3DE: `cmp dword [edi+20h],4`). The movie: [[[ui+8]+108h]+4],
  `Invoke` 0x511600 (thiscall: name, result, args, count; `ret 10h`), a GFx value 18h bytes (+4 the
  type: 4 uint, 5 number; +8 the value). The world from the ui: [[ui+18h]+4D4Ch]+1F8h, its rules
  at +18824h (the minimap's 0x23AA21). `root.set_game_type` gets `mode.mode` (0x234D66).
- **The first local test's crash** (PROVEN, 2026-10-22, the owner's two clients on map 184, both
  at their first synchronization: `PRIV_INSTRUCTION` at 0x9FB814): the first build's slot 16 was
  `deserialize(reader)` `ret 4`. The rules loop above left esp 4 bytes low; 0x2D2770's epilogue
  (`pop edi/esi/ebp/ebx; add esp,68h; ret 0Ch`) then popped each register one slot off and returned
  through the dword below its return address, a stale argument of 0x2D0B70's log call at 0x2D0F3D
  (`push 0x9FB814`, the "net_sync" channel's name), whose first byte is `outsb`. The dump agrees:
  esp 0x767F7754 is 0x2D0B70's frame minus 4, `[esp]` = edi = 8 (the loop's pushed dword), ebp
  clobbered, the last real return address 0x27C0DE (`network_client::process_dynamic_match_info`).
  Slot 16 takes the dword and pops it now; selftest check 53 compares every slot's `ret` with
  tdm's, and `check_research_026g.py` runs the rule through the game's own loops.
- **The review of #297** (2026-10-22, after the merge; `fix/research-review`):
  - **Slot 7's arguments** (PROVEN, CRITICAL): `rn_on_event` took `(type, time, data)`; the game
    passes `(type, data, time)` (above). It read the time as the kill's bytes: before 65,536 ms of
    game time a fault at the first death on every client, after it random killer and victim
    places (up to 254 into the 20 places) and no kill invader. Now `(type, data, time)`, and a
    kill's places out of 0..19 are refused. `check_research_026g.py` runs slot 7 through the
    dispatcher 0x2D1EA0 with a kill and checks what it counted; with the first build's order
    put back by a shim, the case fails. Every slot of ours, the tick and the cook are reached
    the same way (client_patch.md §0.4, "through the game's own caller").
  - **The scoreboard** (PROVEN): the game's event sites (the kill handler 0x2F4EB0, 0x2F4DE1)
    add to the player's cell and then, when the statistics have an event listener (+31A0h, a
    boost::function, not 0 once set), call it through 0x2F5D40 (eax = +31A0h; place, event,
    amount; `ret 0Ch`); the listener is `base_game_statistics_handler::on_event` 0x26DB80 (vtable
    0xA0755C slot +44h, §3.16), ecx the statistics object itself (`[core+18FA8h]`), which adds the
    event's weight times the amount (x 0.01 for the hundredths events 6, 17, 18) to the player's
    score (`m_scores`, +31F0h[place]). Research's counts did not call it: the Tab list's score grew at the
    next world loaded only. `count()` calls it after each add, with what it added.
  - **The leader at the goal**: both shown 100 (99.5 rounds up) was a draw. `rr_leader` takes
    the millionths once a team reached the goal (the rule's end is set), and so does srp
    (`rules::research_winner`).
  - **A late joiner's first world**: the time rule's deserialize (0x31803E) sends event 0
    before Research's deserialize runs, so slot 7's `publish()` wrote the fresh rule's 0 / 0
    and a draw into the statistics' tail; `rn_deserialize` publishes the loaded state at its
    end.
  - **The hud's cache** is of a rule by its address; a new match's rule allocated where the
    last one was found its owners there and could say "Unit A lost" at its first frame. The
    cook's create forgets it.

### 3.51 The artefacts: Spring's aim time and no slow from bullets, Lifebone's heal, one press for every artefact of a kind (written 2026-10-22, README patch 54, the next release's revision; the rules: `artefacts.md`) — PROVEN in the code, the loader, the links, the getter, Lifebone's active and the action run with Unicorn; not run in a game

**A Spring's modifiers.** `artefact<artefact_spring_core>` is built in place by the artefact cook (vtable 0x9FC720: size 0x25E770, construct 0x25E660, both a switch on `data/type`: 3 Lifebone 0x4B8 / 0x25EF80, 6 Spring 0x3B8 / 0x25EA10, 7 Onyx 0xF40 / 0x25F4D0, 8 Rattle 0x360 / 0x25FA10); the base cook builds bare cores (sizes 0x2EEA30: 0x498, 0x398, 0xF20, 0x340; construct 0x2EE930, Spring 0x311840). Both Spring constructors call the loader 0x33ACF0 with esi = the object +340h and the item's `data`; it writes, from fixed keys (0xA1C1FC `passive`, 0xA1C230 `active`):

```
+340h {0, 0, 4,    passive/movement_speed_mod}       +370h {0, 0, 3,   active/stamina_regen_mod}
+350h {0, 0, 5,    passive/max_carried_weight_mod}   +380h {0, 0, 13h, active/stamina_spend_mod}
+360h {0, 0, 4,    active/movement_speed_mod}        +390h trunc(active/duration x 1000) ms
```

A node is {prev, next, index, value}, linked at the tail of the holder's list holder+6B8h + kit (+115h) x 704h + index x 14h + 204h ({count, head, tail, ?, sorted +10h}): the passive's at 0x311AD0 (pickup, deserialize), unlinked at 0x311D10; the active's at 0x311BC0 (enable_active 0x3118D0, deserialize 0x3119E0 for state 4), unlinked at 0x311DA0 (disable_active 0x311970, from the active's end and death). The getter 0x2D5AE0 sums a list (`B x max(0, 1 + k S)` for index 49, aim_time, as for 4); a walk's speed (0x2DBB50) with its flag set (LIKELY aiming: the only reader of index 20, movement_speed_when_aiming) takes min(M4, M[4,20]) on that base, so a Spring counts there too.

**The patch.** 0x33AE13 `mov dword [esi+48h],13h` -> `jmp loader`: it does that, then (pushad, xmm0 kept) `passive/aim_time_mod` into +350h as index 31h (the max_carried_weight node, 0 in retail, no longer read) and `active/aim_time_mod` into a sixth node at +3B8h, index 31h, and goes on at 0x33AE1A. With the stopping power node at +3C8h (below) the object grows to 3D8h (0x25E7A8 `mov eax,3B8h`, 0x2EEA74 `mov eax,398h`); the wrapped Spring's own fields end at +3B7h (0x25EA4F..0x25EA83). 0x311BC0's first ten bytes (`push ebx / push esi / mov esi,eax / mov eax,[esi+250h]`) -> `jmp link_active`: the sixth node linked as 0x311BC0 links the others, then those ten bytes, back at 0x311BCA. 0x311DA0's `push ebx / mov ebx,[esp+8]` -> `jmp unlink_active`: 0x2D5D00 on it, the two instructions, back at 0x311DA5. The node is in its list exactly while the three active ones are (0x2D5D00 must not be given a node that is not in the list: it takes a missing prev as the list's head).

**A Lifebone's heal.** enable_active 0x312250 (thiscall, the time): `remove_affects` entries (+360h..+364h, stride 20h) each through 0x2E58B0 (eax = the damage model's pools, [the holder's slot 2]+108h; ecx = &name; strcmp with pool+80h) and 0x32A6A0; then the duration (M39) into +494h. Its argument slot [esp+14h] is scratch after 0x3122E2. 0x3122AA `mov ecx,[edi+250h]` (reached with four pushes, from the loop or around it) -> `jmp lifebone_active`: the pools named `pain`, `body` and `head` (its table `heals`), each with H < Hmax given increase_health 0x32A420 (esi = the pool; the time, Hmax - H; `ret 8`; it clamps, calls 0x32A9C0, and 0x32B920 when the pool is then full), as the medkit at 0x33D9C3; the instruction; back at 0x3122B0.

**The slow from bullets.** base_player::hit_impl (0x2D9B90) queues (time, stopping_power x k_power) in m_recent_hits (player+2E8h) once per bullet; the stagger 0x2DBCD0 (eax = the player, the time; `ret 4`; from the tick at 0x2D8FE5) sums those younger than [+320h] (0.5 s) into xmm0 and its local [esp+14h], both ways (the loop's end, or none at 0x2DBCF9) reaching 0x2DBD89 `movss xmm2,[esi+31Ch]`; then -min(local, [+31Ch] 0.5) goes into the player's own node +2F4h (index [+2FCh] = 4, value +300h). 0x2DBD89 -> `jmp stagger`: the getter on list 42 with (the sum, 1.0), the result into xmm0 and the local, xmm1 0, the instruction; back at 0x2DBD91. List 42 is artefact_synergy: no getter call reads it (re-kit damage notes) and no retail config sets it, so without a Spring the factor is 1. The Spring's seventh node, +3C8h, index 2Ah, `active/stopping_power_mod`, is filled by the loader hook and linked and unlinked with the aim node (the object 3D8h bytes).

**One press.** artefact_base::action 0x313300 (thiscall, pressed, time; `ret 8`): state 3, pressed, no cooldown (+28Ch) -> vtable +80h enable_active, the effect (0x313350), +2A0h = the time, state 4 at 0x313339. inventory::action (0x2FCAA0) calls an item of kind 1 (+24Ch) through vtable +2Ch with (pressed, time); the artefact slots are inventory+10Ch + 4 x 13..18; [artefact+250h] is the inventory. The entry's ten bytes (`push esi / mov esi,ecx / cmp dword [esi+280h],3`) -> `jmp fire_kind`: a key-down activates the pressed one, then each other slot's item with the same vtable and kind 1, each only in state 3 with no cooldown and as action does (vtable +80h with the time, 0x313350 (it, the time), +2A0h, state 4), and returns `ret 8`; nothing is pressed through action, so nothing recurses, and a press of a cooling or active one still fires the ready ones of its kind. A key-up runs the ten bytes and goes on at 0x31330A (the jne on their flags).

**Onyx stacking (2026-10-25).** An Onyx (`artefact_onyx_core`, 0xF20 bytes, its wrapper 0xF40, vtable 0xA157BC / 0xA06084) is built by 0x3126E0: its loader 0x33B780 (cdecl: the config at +C7Ch, `data`) fills +EE8h hit_type, +EECh..+EF8h the passive's damage_add, damage_mul, armor_piercing_add, armor_piercing_mul, +EFCh..+F08h the active's, +F0Ch absorb_total, +F10h active/duration ms, and the protected parts at +C90h (1Ch each); then one damage protector (70h bytes, at +340h) per part, each bound to the hit callback 0x312BF0 with the Onyx (0x312777). enable_passive_effects 0x312810 (vtable +7Ch, at pickup) links each protector into its pool's list (pool+BCh, next at +68h), disable_passive_effects 0x312950 (+84h, at death and drop) unlinks them: they are in from pickup to death in both states. enable_active_effects 0x3128C0 (+80h): +F14h (time left, ms) = trunc(+F10h x M39 artefact_duration, set by nothing in 0.26g), +F18h = absorb_total; tick_impl 0x3127E0 counts +F14h down; active_effect_ended 0x312A50 is 1 when +F14h is 0 or +F18h is 0.0 (nothing writes +F18h during the active: absorb_total is a cap per hit, 5 in retail); disable_active_effects 0x312A30 zeroes both. The base's tick 0x313200 (+74h) ends a state-4 artefact through 0x313530 (cooldown_sec x 1000 x M38 into +28Ch, state 3). The callback (thiscall: the hit, &damage, &piercing; `ret 0Ch`): the hit's type (+1Ch) must be +EE8h; state 3: D = (D + add) x mul, P likewise (passive); else D -= min(+F18h, D - (D + add) x mul) and P = (P + add) x mul (active). So n active Onyx gave D x 0.1^n, each 3 s. The press (fire_kind) now counts the ready ones it fires first (`ready`, `kin`: state 3, no cooldown; same vtable and kind 1 in the slots) and, when the kind's enable_active (vtable +80h) is 0x3128C0, adds (n - 1) x 1000 ms (ONYX_EXTRA_MS) to each fired one's +F14h after its activation: all n end on the same tick and cool down together. 0x312C03 `cmp dword [ecx+280h],3` (seven bytes, after the hit type test; the next instruction `mov eax,[esp+8]` at 0x312C0A, then `jne 0x312C47`) -> `jmp onyx_hit` (JUMP): state 3 goes back at once (ZF set, the passive's way); otherwise the slots 13..18 are walked and the first item that is the Onyx itself, or of its vtable in state 4, decides: itself -> the compare again (ZF clear) and back at 0x312C0A (the active's way); another -> `ret 0Ch` with damage and piercing unchanged. An Onyx in no slot goes on as before. eax and edx are free there (eax is loaded at 0x312C0A, edx, the hit, is not read after 0x312BFA); esi and ebx are pushed and popped.

**Onyx's pop-up.** The wrapper artefact<artefact_onyx_core> has a second vtable at +F20h (0xA06118); its slot 0, 0x25F640 (ecx = ebx = the Onyx +F20h), compares the state (+280h, ebx-0CA0h) with the one it saw last (+F30h, ebx+10h) and acts on a change; for state 4 (0x25F9A5) with the holder the local player (0x242A40 on the holder's +114h) it calls set_parameterized_message 0x2375C0 (ecx = the activation hint's text, +2F0h, a localization key that 0x1FF0F0 looks up; then the game +1FCh, 21h, 14h, BB8h; `ret 10h`), which invokes `root.set_parameterized_message` (0xA0360C) of the hud: once for each Onyx, the same fixed text (no number from the config), 3 s on screen. Spring (0x25EF63), Lifebone (0x25F4B3) and Rattle (0x25FF30) have the same code with their offsets. 0x25F9DD `mov ecx,[ebx-0C30h] / push 0BB8h` (eleven bytes; 0x25F9DB's `jz` goes to 0x25F9F8) -> `jmp onyx_popup` (JUMP): the slots 13..18 are walked; the first that is the Onyx itself, or of its vtable in state 4 with the same activation time (+2A0h), decides: itself -> ecx = the hint, `push dword [ebx-0Ch]` (+F14h, the active's time left in ms) and back at 0x25F9E8 (`push 14h`); another -> 0x25F9F8, past the call, where the function notes the state as it does after it. An Onyx in no slot shows its own. esi (the game) is pushed and kept; eax, ecx, edx and edi are free there (edi is popped at the end). The hud: set_parameterized_message replaces `%s` in the text with ceil(time / 1000) before it compares and shows it. The token is `%s`, not `{s}`: the game's localizer (0x1FEF90, every text set_parameterized_message shows) makes `{NAME}` the key of the action NAME (`{USE}`) and any other name nothing, and drops a `}` after a character: 0x2375C0 gets the text through 0x1FF0F0 (the localization lookup), which ends in 0x1FEF90 (eax = the text, ecx = the output, the game pushed; `ret 4`): `{{` is `{`, `{USE}` (0x9FC848) the bound key's name (0x207BF0, then the lookup again), any other `{...}` nothing. The first build used `{s}` and the owner's game showed "for  s." (2026-10-25).

**Checked:** `selftest_026g.py` check 54 (with the Onyx callback's bytes, its constructor's binding at 0x312777 and enable_active in both vtables' +80h); `check_artefacts_026g.py` (Unicorn): Onyx's own enable_active, countdown, end test and hit callback for 1, 2 and 3 Onyx fired by one press (3, 4 and 5 s; damage x0.4 and piercing x0.4 once; the original 3 s each and 0.4^n), a cooling Onyx beside a ready one, two Onyx active from two presses, and the end of the client update from 0x25F9DD (the hint once a press for 3000, 4000 and 5000 ms; the original once for each Onyx for 3000 ms); the loader with the game's lookup (0xF6B10) on a binary config of the release's numbers (and the retail ones in the original), links and unlinks with the getter for one and two Springs, Lifebone's enable_active with made-up pools, the action over a made-up inventory, and through the game's own callers: a slot's key through inventory::action 0x2FCAA0 (edi = the inventory; slot, pressed, time; its vtable +2Ch call), the Springs' own enable_active 0x3118D0 linking and disable_active 0x311970 unlinking their nodes; the build's loader on the edited retail `artefact_spring.options` (content_edit.py output).

**Where and how it is installed** (the merge with main's arena and call shapes, 2026-10-23). The code is the arena's block `artefacts.code`, a code page (0xA431000 as the link places it today, §0.3; the hooks jump to its six global entries by name), not `.srpw`+2700h where it was first written: it still fit there and nothing of main used that page, but new code goes on the arena. Its six sites are in `INSTALLS` (§0.4): 0x311BC0, 0x311DA0 and 0x313300 ENTRY, their models the functions at the sites (`ret` 0, 4 and 8, as `abi.tsv`; ours goes on in the function, or for a key-down returns `ret 8` itself), and 0x33AE13, 0x3122AA and 0x2DBD89 JUMP (from inside the game's code, back at the depth they left). It hands the game no pointer (no callback, no vtable). Each hook's arguments, read again against the game's code: the loader's ebx = `data`, eax and edi set by 0x33AE0B..0x33AE10 for the next lookup (kept by pushad), xmm0 the third node's value (kept); the link's eax = the Spring (0x311944, 0x311A4D); the unlink's Spring pushed (0x311973), [esp+24h] after pushad; Lifebone's time [esp+14h] with the function's four pushes on both ways to 0x3122AA (`push ebp` popped at 0x3122A9), [esp+38h] after pushad and the amount; the pool lookup 0x2E58B0's eax = the pools' holder, ecx = the address of the name's pointer, nothing on the stack (as 0x312291); increase_health (esi = the pool; time, amount; `ret 8`) pushed as the medkit pushes them (0x33D9A3..0x33D9C3); action's (pressed, time) as inventory::action pushes them (0x2FCB2D..0x2FCB31), enable_active's (time) and the effect 0x313350's (the artefact, the time) as action pushes them (0x313329, 0x31332C..0x31332E); the getter's (B, k) as enable_active 0x3118D0 passes them (B the value, k 1.0; ecx and edi kept, `ret 8`); the stagger's sum in xmm0 and its argument slot [esp+14h] on both ways to 0x2DBD89 (0x2DBCF9 and the loop's end, the memmove's three pushes taken back at 0x2DBD65). No argument count or order was wrong.

### 3.53 The keys 1, 2 and 3 pick a kit; a match chat option (written 2026-10-23, README patch 58, the next release's revision; the rules: `match-hud-options.md`) — PROVEN in the code, the four entries run with Unicorn with the game's own cvar code and input tick; not run in a game

A player's asks (board T29): number keys for the kit choice before a respawn (misclicks), and an option to turn the in-game chat off.

- **The kit picker** (PROVEN). The kits before a respawn and before the start are the hud movie's `_profilePanel` (`HUDProfileSelector`, its `SrvProfileSelectorHUD`, a `ProfileSelector` whose `tabPage` is a CLIK ButtonBar of three tabs, `SrvPlayerProfileTabHUD`), shown by `root.show_profiles_selector` (0x2383B0, with `show_cursor` 0x23A930: the hud's +2DCh). A click changes `tabPage.selectedIndex`; its INDEX_CHANGE runs `onProfileSelected`, which calls `ExternalInterface.call("profile_changed", index)`: the exe's 0x233F06 -> 0x1F8070 queues the kit input (§3.29; patch 31 keeps the kits shown after it). A tab is greyed (`enabled` false, `setup_player_profiles`: a kit that is not complete) and does nothing; a click on the chosen tab changes nothing (`set selectedIndex` returns on the same index). No new message: a key does what a click does.
- **Where the keys go** (PROVEN). `vostok::input` hands a key to the input handlers by priority; the game world's (`input_priority` 0x1F63D0: 7) comes before the player's (10). The game world's `on_keyboard_action` (0x25CD00, slot 0 of the vtable 0x9FC588 at game_world+1F8h; thiscall, the input world, the DirectInput key, the kind; `ret 0Ch`; re-kit abi rows) asks the input world whether Tab is held (0x2384C0, `root.show_player_list`), returns 0 for a held Tab, then at 0x25CD41 `cmp dword [ebp+10h],1 / jnz near 0x25CF0D` (kind 1, a key down; 0x25CF0D `xor al,al` and the epilogue; the kinds are the keyboard's: 1 down, 2 up, 3 held) and handles chat (0x3F, Enter, numpad Enter), L, Esc and its actions 11h..19h by bind; anything else returns 0 and goes on to the player's handler (`on_keyboard_action` 0x23FE00, kinds 1/2/3 pushed as states 0/1/2). The hud movie gets keys only from the chat handler (0x20899D, `0x1FDD70`, while the chat is typed in): `HUD.as` sees no 1, 2 or 3 on the kit screen.
- **The patch, keys** (`MATCH_KEYS_HOOKS[0]`, JUMP at 0x25CD41, ten bytes -> `jmp match_keys_keyboard` + five nops): the kind's test again; for a key down with DIK 2..4 while the handler's +2E0h (the hud's cursor) is set and the movie is there, `engine_movie_invoke([[[edi+0Ch]+108h]+4], "root.select_kit_key", 0, {0, 4 uint, key-2}, 1)` (as the hud's own calls, 0x2384C0), all registers kept (pushad), then `jmp 0x25CD4B`; any other kind `jmp 0x25CF0D`. The key is not taken. `HUD.select_kit_key` returns unless `_profilePanel.visible` and the chat is not being typed in (a flag `focus_chat` sets and clears, or `stage.focus` on its input; not the input's `visible`, which `SrvGameChat`'s timeline starts true until the chat is first closed); `ProfileSelector.pickByKey` sets `tabPage.selectedIndex` for a kit that is enabled and not the chosen one.
- **The input tick** (PROVEN, 2026-10-25). `game::tick` (0x1EC890) runs the input world's tick (0x3B7BD0, slot 0 of the input world 0xAA5430, `[game+5358h]`, at 0x1ECB67) before the game world's update (`[game+539Ch]` vtable+8 at 0x1ECBE1), whose hud turns to the kits when the local player dies. The tick (thiscall, the time; `ret 4`; it runs while the world is active, +34h) polls each device (the keyboard is +24h: its execute, vtable+20h 0x3B8D20, `GetDeviceData` into its buffer +408h, 14h a DIDEVICEOBJECTDATA, the count +404h, and the state of each key +4 + 4 x DIK, 80h down), then calls every handler's slot +14h with the input world, then each device's process (the keyboard's, vtable+24h 0x3B9AF0: each polled event in order, kind 1 for 80h down and 2 for up, then kind 3 for every key down, every tick), then every handler's slot +18h. A key's kind 1 is an edge: a key held makes kind 3 only. The game world's handler has `ret 4` (0x75AF0) in both slots (0x9FC59C, 0x9FC5A0).
- **Why kit 3 came after a death** (PROVEN, the owner's report 2026-10-25: "I often press 3 mid-fight to heal with the medkit, I die, and I swap to my 3rd kit"). Not a held key (that is kind 3, which the hook let by), and no queue of the patch's own: the hook asked the cursor when the key was handed out, not when it went down. A tick hands out what its poll got, every key that went down since the poll before; the death turns the hud to the kits in the game world's update after that poll. So the first tick on the death screen handed out, as kind 1 with the cursor shown, the key 3 pressed in play in the frame of the death (after that frame's poll), and `select_kit_key(2)` picked kit 3 at once: `profile_changed` queues the kit input (0xDE) for the next send, and the server makes it the next life's kit at its tick (§3.29); only another pick undoes it. The keys were taken in every state with the cursor: the death screen from the first tick after the death through the respawn countdown (the death camera is that screen, patch 46), and the kits before the start (patch 31). Run on main's build (rev 34) with the game's tick: 3 held at the death, nothing; 3 pressed or tapped in the death's frame, kit 3.
- **The patch, the kits' first 300 ms** (`MATCH_KEYS_POLL_SLOT`, SLOT at 0x9FC59C, `ret 4` 0x75AF0 -> `match_keys_poll`): the slot runs after each poll, before its keys are handed out, and moves `kit_keys` (match_keys.state) by the cursor (+2E0h) and GetTickCount (its import 0x870090, as bake_cache's): hidden CLOSED (`kit_keys_since` cleared); shown, from CLOSED OPENING with the time now in `kit_keys_since` (the kits came after the last poll: a key of this one may have gone down before them); from OPENING OPEN once now - since (unsigned, over the wrap) is `KIT_KEYS_DELAY` (300 ms) or more, else OPENING; from OPEN OPEN. The owner's rule (2026-10-25, after the first version, which opened at the next poll): a player still pressing 3 to heal just after the death presses on the kits before seeing them, so the keys pick nothing for 300 ms after the kits appear. `match_keys_keyboard` picks only in OPEN (and with the cursor still shown). OPEN comes at a later poll than the one that saw the kits, so a key pressed in their frame is OPENING's; a key pressed in their first 300 ms (less a frame: a press is taken at the poll that hands it out) is OPENING's; held, either makes no kind 1 until let go and pressed again. The same holds for the kits before the start. Nothing else changes: the keys go on to the game as before (1/2/3 the quick slots in play). The pointer in .rdata is the vtable's, under a base relocation already (the exe is never rebased).
- **The option** (PROVEN). The gameplay options (`options_tab::options_tab` 0x262300, tab 0) make option 4 at 0x262454: `options_item_bool` (vtable 0xA06744) named "g_private_messages_in_game" (0xA064FC, in the exe there alone), labelled `st_private_messages_in_game_option`. `options_item_base::options_item_base` (0x260E90) looks its command up by name (0xEB7C0, the list 0x1F1D66C) and logs a warning when there is none: there is none, so the option has no value and nothing reads it; `main_menu.swf`'s timeline greys its checkbox (`_no_in_batle.enabled = false`, as the other social options, which are dead the same way: g_invite_from_friends, g_friends_signin_notification, g_messages_only_from_friends, g_hide_spam). The options are made by `game_options::on_resources_ready` (from `game_options::initialize` 0x1FFC10, called at 0x1EAC7C after the base resources).
- **The patch, the option's variable** (`MATCH_KEYS_HOOKS[1]`, CALLEE at 0x1F6845, `call 0x233810` -> `call match_keys_hud`): `game::game` (0x1E9070) makes the game world (0x1F67B0 at 0x1E90A3) before `query_base_resources` (0x1E9B4C) queries `startup.cfg` and `user.cfg`, which `on_configs_loaded` (0x1EA210) runs: `load_cc_script` (0x1EC380) twice, `user.cfg` with filter 1 and mask 2 (`console_commands::execute` 0xEB980 runs a line for a command whose +18h is the filter, or any for filter 2, and whose +14h has a bit of the mask; `startup.cfg`'s mask is 1, so a kind-2 variable such as `minimap_position` cannot be set from it). `match_keys_hud` calls the hud's constructor with the game world (eax) and the hud as the site did, then once (a byte of its own) `engine_cc_bool(object, 0xA064FC, &hide_chat, 1, 2)`: `cc_value<bool>::cc_value` (0x1DDB0, eax the 48h-byte object: +0Ch the list's next, +10h the name, +14h the kind 2, +18h 1, +1Ch 1, +1Dh saved 1, +40h the value's address; the new list head), then the vtable cc_bool 0x9E7C4C, as the hud's own `show_quests_progress` (0x2339C9..0x2339F2, which also gives the dtor to atexit; ours is never destroyed, the arena's memory lasting the process). The options' lookup then finds it; `user.cfg` keeps it (`g_private_messages_in_game on|off`; an older game ignores the line). The main menu enables the checkbox where its timeline makes it (`settingsTabs_15`'s `__setProp__no_in_batle`, run for every new instance) and in `Settings.create_content`; `client-patches/localization/*.tsv` labels it "Hide chat in match".
- **The patch, the lines** (`MATCH_KEYS_HOOKS[2]`, CALLEE at 0x26B916, `call 0x208AD0` -> `call match_keys_chat`): `process_incoming_text_message` (0x26B6F0, message 0xA2) gives every line it shows to `chat_handler::add_message` (0x208AD0; stdcall: the handler, the channel, the body, the name, the tag; `ret 14h`). `chat_handler::set_mode` (0x209430) sets the handler's movie (+14h) and +0Dh: 1 with the hud (the game world's `on_activate`, 0x1F93C5), 0 with the lobby's (0x21D34F); add_message gives channels 1 and 4 to the lobby's movie while +0Dh is set. `match_keys_chat`: option on, +0Dh set and channel 3 or 5..9 -> `ret 14h` (eax not kept: add_message's own eax is garbage, the caller's next call `add_to_recent_list` 0x209330 takes the name and edi); else `jmp 0x208AD0`. The server relays a line to everyone but its sender (`relay`, `**id != from`), and the sender's own line is shown by the local echo (`messaging_client` 0x26ADF4, another call): it stays.
- **What changes:** in a match, keys 1, 2 and 3 pick kit 1, 2 and 3 on the kit screen (the key also reaches the player's handler as before, which a dead player's game does not send); with "Hide chat in match" checked, no other player's line appears in the match chat. No simulation change.
- **Checked:** `selftest_026g.py` check 58 and the call shapes; `check_match_keys_026g.py` (Unicorn): the poll's ten cases (299 and 300 ms, over GetTickCount's wrap too), 216 key cases (each state of `kit_keys`), 64 input-tick runs of the game's own tick and keyboard process (keys 1 to 4, GetTickCount from 1000 and from before its wrap: pressed 100 and 250 ms after the kits appear, none; 350 ms after, a pick; pressed at 100 ms and held into OPEN, none until let go and pressed again; held into the kits; pressed in their frame; in play; a respawn and a death), the variable's four, 40 chat cases (README patch 58). **Not run:** a game: that `vostok::input` gives the game world's handler the keys on the death screen (the cursor's, LIKELY: the handler answers 0 and the mouse's handler 1 there), the movie's tab change in Scaleform, the checkbox in the menu and its value saved, the lines hidden.

### 3.54 One key hides the interface, in a match and in the lobby (written 2026-10-23, README patch 62, the next release's revision; the rules: `hide-ui-key.md`) — PROVEN in the code, the three entries run with Unicorn with the game's own show_ui and get_binded_action; not run in a game

A player's ask through the owner: a key that hides the whole interface, the lobby's too, for screenshots.

- **What 0.26g has** (PROVEN). No console command or variable hides the hud (no `hud_draw`, `g_hud`, `ui_hidden`, `r_hud` or `hide_hud` string), and no screenshot key (no action, command or string). It has the action `toggle_replay_ui` (19h, kinds 5, F11; the action table 0xA74AF0, 20-byte entries `{name, id, kinds, toggle, default key}`, entry 0xA74CA8), which shows the replay viewer's controls and cursor (0x25CED1..0x25CF08) and does nothing without the replay's movie. It has `game_world::show_ui(bool)` (0x1F95F0, vtable 0x9FC4F4 slot 8; the activation's show_ui(1) at 0x1F8476, the deactivation's show_ui(0) at 0x1F96D1; state +0A0h, 0 at the construction 0x23DAF1) over the hud movie (+204h) and the player icons' (+20Ch), and `lobby_menu::show_ui` (0x21D570) over the lobby movie (+690h) and +68Ch: render only (base_game_scene show_movie 0x23DC00 / hide_movie 0x23DC70).
- **Why no new action** (PROVEN). The key binder (the game's +5344h) is 48h bindings of 0Ch bytes indexed by the action id (`get_binded_action` 0x207D50: eax the binder, edi the kinds, esi the DIK key, out the toggle; `ret 4`; 49h for none); ids 0Fh, 10h, 14h and 2Bh..33h are free. But the action table ends where the key names begin (0xA74FA0, read by 15 sites; 0x4B0 bytes counted at others), and the options' controls list is its own 40 entries of 68h bytes at 0xA9DB28 (+0 id, +4 label, +8 group, +0Ch type, +10h a buffer pointer into the entry; counts 28h at 0x201159, 0x20124D, 0x20140B, 0x201A1E, 0x201D0D; refs 0x201151, 0x2013C0, 0x201249, 0x201A1A, 0x201D09), followed by other data. Adding a row to the action table means moving it; `toggle_replay_ui` already is the action "toggle the ui", bound and saved as any other.
- **The controls options' row** (PROVEN; the owner, 2026-10-24: "there is no in settings option to switch it"). The options' table is static data: 28h rows, each `{action, label key, group, type, buffer_string set (+10h: begin, end, buffer end; 32 bytes at +1Ch), buffer_string kept (+3Ch; 32 bytes at +48h)}`, 240 pointers into itself, no initializer. Its six readers, game_options': 0x201150 (`mov eax,table`, count at 0x201159), 0x201248 (`mov esi,table+4`, 0x20124D), 0x2013BF (`add eax,table`, `cmp bl,28h` at 0x20140B), 0x201A19 (`mov esi,table+0Ch`, 0x201A1E), 0x201D05 (`mov [esp+0Ch],table`, 0x201D0D), 0x201F2B (`mov esi,table+10h`, 0x201F34). The movie names an action only (`start_bind_key`, BindKeyRender.as); the key it gets is kept by 0x2013A0 in the row of that action (none: kept nowhere) and accept (0x201F20) issues `bind <id_to_action_name> <key>` / `unbind` only for rows whose +10h changed. The binder's `bind` command (0x207390, cc with bind_key 0x207DC0) saves every bound action of the binder by name (0x207450: `bind <action> <key>`), whatever the table. The patch: `controls` (hide_ui.data) is the 40 rows and a 41st `{19h, "st_hide_interface_action", 2, 1}`, built by NASM with each row's pointers its own; HIDE_UI_TABLE_REFS / HIDE_UI_TABLE_COUNTS repoint the six readers and make the counts 29h. A pointer into an arena data or state block is data, not an install (`arena_data_ranges`, from the lock).
- **The keys' groups** (PROVEN). The game world asks kind 1 (0x25CD5F); the lobby kind 2 (0x26CCCD); the options list groups 0 lobby, 1 chat, 2 game. Free of every default binding: B, H, J, U, Y, Z, 9, 0, F4, F7..F10, F12, Caps Lock (L is taken by the game world at 0x25CD89, DIK 26h). The lobby movie never gets F1..F12 (the DIK-to-Flash key map made at 0x1FDF20 has 71 keys, no F key, not 9 or Caps Lock).
- **The patch, the match** (`HIDE_UI_HOOKS[0]`, JUMP at 0x25CED1, ten bytes -> `jmp hide_ui_match` + five nops): the replay test again (to 0x25CEDB with esi the handler); else, the hud the handler's +4, `engine_movie_invoke([[hud+8]+108h]+4, "root.interface_hidden", &answer, 0, 0)` (a bool at the answer's +8) and `root.hide_interface(not that)` (one GFx bool, type 2) to it and to [[hud+10h]+108h]+4, the player icons'; then 0x25CF0D. Whatever the cursor: the kits before the start are hidden too (the owner, 2026-10-24). The first build used the game world's `show_ui` (0x1F95F0): its `hide_movie` (0x23DC70, the renderer's 0x1FBB0) takes the movie out of the scene, which stops advancing it, and when shown it played the kill feed and messages of the whole time at once (the owner's test of 64ce3ce7); the movies hide their own root and keep running.
- **A new match**: the movies are the last match's; `initialize_match` (0x234880) invokes the hud movie's `root.reset` at 0x234AE8 (ebp the hud). `HUD.reset` shows the hud, and the patch (`HIDE_UI_HOOKS[2]`, CALLEE at 0x234AE8, `call 0x511600` -> `call hide_ui_reset`: the invoke through the library, then `root.hide_interface(false)` to the hud and the icons, `ret 10h`) shows the player icons' too. (`set_enemy_icon_ids`, the icons' other per-match call, is also made by base_network_client at 0x242C86 when the camera's player changes: a respawn, which keeps the state.)
- **The kits and the death screen**: hidden with the rest, and kept so by a death and a respawn (the owner, 2026-10-24); the first builds showed them again from show_profiles_selector's show_cursor call (0x23845B), which is the original's again. Keys 1 to 3 still pick a kit (patch 58: `select_kit_key` reads `_profilePanel.visible`, the panel's own flag). The hud's screen effects stay: `effectContainer` (add_effect 0x23C100: the blood, its drips, the grenade's flash) is on the stage below the root, placed as the root is scaled (`placeEffects`), so hiding the root (`HUD.hide_interface`: `visible`) leaves it; the fps line the game put on the stage is in the root's `_stageLayer`; the hit direction marks, the bleeding/toxic/radiation icons (criticalInd) and the corner frames (hud_overlay) are interface and hide.
- **The patch, the lobby** (`HIDE_UI_HOOKS[1]`, CALLEE at 0x26CCD2, `call 0x207D50` -> `call hide_ui_lobby`): `engine_binded_action(binder, 2, key, the place)`; for 49h again with kind 1 and a place of its own; 19h with the movie [[ebx+690h]+108h]+4 -> `engine_movie_invoke(movie, "root.toggle_interface", 0, 0, 0)`, the return address 0x26CCAE (`lea ecx,[esp+28h] / call 0x2AFA70 / mov al,1 / ... ret 0Ch`, the path after 0x1FDD70, which pops its five arguments: the same esp), `ret 4`; else, with the movie, `root.interface_hidden` into a GFx value of ours (a bool at +8, as the function reads has_active_window's at 0x26CC06; nothing to release): true -> the return address 0x26CDD2 (the key to the movie through 0x1FDD70, al 0; past the window keys' actions and F6's Play, `cmp esi,40h` at 0x26CDA2); anything else returns the first answer.
- **The lobby movie** (`LobbyMenu.toggle_interface`): the interface is the root, hidden in one step (`visible`). What the game put on the stage is in the root: `makeStageLayer` (before `init`) makes `_stageLayer`, the root's last child at the stage's scale (1/scaleFactor; kept last by `addChild`, `addChildAt` and `setChildIndex`), holding `_layers` (the front, top (messages) and top2 containers, the wheels, the loading screen, the fades; the right-click menu, `SrvContextMenu.registerStage(stage, _layers)`; CLIK's popup canvas, `PopUpManager.layer`) and over it `_overlay` (the tooltips' container and CLIK's drag canvas, `DragManager.init(stage, _overlay)`): no polling, nothing of the interface on the stage beside the root. First an open right-click menu is closed (only when open: `SrvContextMenu.hide` with none ever opened threw on `removeChild(null)` and nothing was hidden, the owner's test of 64ce3ce7) and the item cards (on Scaleform's topmost level, which is drawn whatever the parents' visibility, to be above the windows). `enable_element` does it structurally now: the exe's lobby state (`lobby_menu_state`, vtable 0xA0CBF4, slot 1 0x2A3EA0, at every entry of the lobby) calls it with `root.statusPanel.btn_off` and `btn_settings` and true (0x2A3F22), a match's loading (0x220400) with false; with true it put the two buttons on the topmost level (above every modal: a message's dim, the results, the wheels) and, through its RENDER handler, the tooltips, the drag canvas and the tutorial's highlights, and the corner buttons stayed on the screen with the root hidden (the owner's test of 335343b6). Now with true it moves the two buttons into `_overlay`'s `_corner`, placed as the status panel is (`placeOverlay`, again after each of the panel's layouts: `StatusPanel.LAID_OUT`), and with false back into the panel at their index; any other element (the tutorial's) is ignored. `_overlay` holds, bottom to top, `_corner`, `_cards` (the item cards, placed as the root is: no longer on the topmost level, so the tooltips are above them), the drag canvas and the tooltips: above every modal, inside the root. The tutorial's private test functions (testDragTutor, testPlayTutor, testAmmoTutor, testAmmoTutor2, testFactionPreTutor, testFactionTutor, testCloseSlide, showSlideOnVs, testTutorComplete) were never called and are gone; the functions the exe still names (show_slide, close_slide, highlight_element, clean_highlight, highlight_inventory_items, set_sybsystem_active, show_tutorial_complete_message) stay. Two KEY_DOWN listeners on the stage, priority 100, capture and target (the target is the stage when nothing has the focus), show it and stop the key, and two KEY_UP listeners stop that key's release once (a message box hidden with the interface keeps the focus and acts on Enter's release: MessageAcceptBase, MessageBox, MessageEnterName; SquadList on Backspace's); a new press of that key or a match found (the exe passes no key then, 0x26CBCE) clears a release never seen; `interface_hidden` answers the exe; `has_active_window` says true while hidden (the exe's Esc then calls `close_active_window`, which shows it); `get_mouse_over` false while hidden; a match found (`set_mm_sync_state`), the loading and the results show it.
- **What changes:** F11 in a match hides the hud, crosshair, minimap and markers until F11 or the kits; F11 in the lobby hides the lobby's interface until a key. Rendering and input only, no simulation change.
- **Checked:** `selftest_026g.py` check 62 and the call shapes; `check_hide_ui_026g.py` (Unicorn): 16 match cases, 28 lobby cases, 5 of the options (README patch 62). **Not run:** a game: that show_ui's hide leaves nothing of the hud (the hit marks and blood effects are the hud movie's, LIKELY), that the player icons' movie is +20Ch (LIKELY: initialize_resources' third movie, the exe's names in that order), that a stage child of the lobby keeps its own `visible`, and that Scaleform's capture listeners run as Flash's.

### 3.55 The kill feed shows a kill through a wall, by a ricochet and by pain (written 2026-10-25, README patch 67, the next release's revision; the rules: `kill-feed-icons.md`) — PROVEN in the code, the three sites run with Unicorn with the game's own part lookup; tested in game by the owner, 2026-10-25 (before the explicit melee test)

The owner: "when a character is killed from pain, the character should get <weapon> <white painkiller> icon"; "when through a wall: <weapon> <wall shot> icon (available in last patches)"; then yes to retail's ricochet icon.

- **The kill event** (PROVEN). `base_player::hit_impl` (0x2D9B90) runs the kill path once the dying flag (+33Dh, set by `on_affect` 0x2DA480 for the death affect of an empty body, head, pain or infection pool) is up: `generate_killed_event` (0x2DA170; esi the victim; stdcall: the time, the killer's place, the hit's bone name, the hit's damage source, the bullet `[ebp-4]` of hit_impl, from `[hit+20h]`'s vtable 0, `ret 14h`; 0x2D9EFC..0x2D9F15). It takes a node of the world's events (`world+12740h`, 0x2D43B0: +0, +4, +8 zeroed), writes the victim (+0, place +114h), the killer (+1), the weapon's icon (+2: the bullet's damage parameters' +50h when not 0, else the damage source's +224h) and the cause icon (+4), type 0 (+18h), and queues it (0x2D0A60; eax the node, edx the world). The cause icon: 3 for no killer or the victim himself; else 1 when the bone is `brain` or `face` (0xA1067C, 0xA10714) and the damage is not the weapon's melee (`[bullet+40h]` the weapon's +53Ch); else 0. 2 (a drop) and 4 (a fist) have frames in the movie but are never sent.
- **The hud** (PROVEN). The world's events are played by frame (0x1FB660, the list from +12744h); the hud's handler (0x23B5F0; edi the node) gives type 0 to `game_world_ui::on_player_killed` (0x237C70: the hud, victim, killer, weapon icon, cause icon, each a byte widened; `ret 14h`), which makes the description object (action_id 1, or 2 when the killer is the victim; who_name, who_team, victim_name, victim_team, object_icon, extra_icon as a GFx int from the fifth argument's first byte alone, `movzx eax,byte [esp+104h]` 0x2380EA, mastery_icon 0) and invokes `root.add_log_message` (0x2381A5). `CombatLogItem` shows extra_icon's frame (value + 1) after the weapon: 2 headshot, 3 a red drop, 4 a skull, 5 a fist; one cause, no other icon.
- **The pools** (PROVEN). The damage model is the player's +2E0h; its parts a list at +108h (first node +110h; a node's next +0, name +80h, health +0A0h, max +9Ch), found by name with 0x2E58B0 (eax the list, ecx the address of the name's pointer; eax the part or 0; keeps ebx, esi, edi, ebp), as hit_impl finds `pain` (0x2D9C3D). Health is clamped at 0 (pain-limbs, re-kit damage notes), so an empty pool's bits are 0 and a live one's are above 0 as an int.
- **The bullet** (PROVEN). +84h counts its trajectory's segments past the first: 0x32EE20 (`inc dword [esi+84h]`, then the new segment's start) is called by try_pierce (0x32D960, at 0x32DE2C, after the hit object's hit and the bullet's slowing) and by try_reflect (0x32DE40, at 0x32E335); +88h counts the reflections (`inc byte [ebx+88h]` 0x32E33A, at most 2). Both are zero at construction (0x32BB1B) and serialized (0x32BFFC). The killing hit happens inside try_pierce before its own increment, so at the kill +84h counts the pierces and reflections before it (a wall, a prop or another player pierced), +88h the reflections.
- **0.69d0, the movie** (PROVEN, its hud.swf, resources.43.db of the 0.69d0 build; the base 0.69 volume's hud and every earlier build's, 0.27d to 0.46c2, have none of it). `CombatLogContainer` maps four flags of the description, `ricochet_icon`, `penetration_icon`, `hipfire_icon` and `blind_kill_icon` (1: shown), to `log/icon_<name>.png`; `CombatLogItemRenderer.makeLayout` places the weapon, then blind_kill, hipfire, penetration, ricochet, then extra_icon, then the victim. No pain icon.
- **0.69d0, the exe** (PROVEN statically: `game/binaries/x64/survarium.exe` of the 0.69d0 Survarium.zip, image base 0x200000000, x64, unpacked; no earlier exe we have names the flags). The bullet keeps the 0.26g counters as bytes: +68h its segments, one more in `change_trajectory` (0x20039AA10, `inc byte [rcx+68h]`), called by try_pierce (0x200399F00, at 0x20039A546, after an object's or a player's hit and for static geometry alike: no material or character check) and by try_reflect (0x20039A590, at 0x20039A9BC, then `inc byte [rsi+69h]`); +69h its reflections (at most 2, `cmp byte [rdi+69h],2` 0x200399E15); +68h capped at 30 (0x200399EE9). try_pierce copies them into the hit (0x20039A17F: hit +0ADh = +68h, +0AEh = +69h; +0ACh the melee flag) before its own increment. hit_impl's caller of generate_killed_event (0x200303B19) passes penetration = segments and no reflection, ricochet = segments and a reflection; generate_killed_event (0x2003046B0) writes the event's +9 (1 penetration, 2 ricochet, ricochet first: `cmovne` at 0x200304B68) and clears both for a melee hit or when there is no other killer (0x200304A80..0x200304AC3); the hud's handler (0x2001A97E6) passes +9 to on_player_killed (0x2001A4BD0), which sets `penetration_icon` for 1 and `ricochet_icon` for 2 (0x2001A4E53, 0x2001A4E80). So 0.69d0's marks are, for a shot and someone else the killer: ricochet when the bullet reflected before the killing hit; else penetration when it pierced anything (a body included). INFERRED only: that 0.69d0's try_pierce is reached for a player's body as 0.26g's is (its object path, 0x200399F69, is the same shape).
- **The patch** (`KILL_FEED_HOOKS`, `KILL_FEED_ARGUMENT`; kill_feed_026g.asm in the arena's kill_feed.code): at 0x2DA3D1 (`mov [eax+4],bl / mov edx,[esi+120h]`, INLINE) `call kill_feed_event` + four nops: +4 the cause icon, +5 the flags (0.69d0's rules, for ebp not 0, the cause icon not 3 and generate_killed_event's melee flag `[esp+14h]` 0 (0x2DA2D7; `[esp+20h]` in the hook): 4 for `byte [ebp+88h]` not 0, else 1 for `[ebp+84h]` not 0; 2: the pain part's health 0 or less as an int, the body's and the head's above 0), edx the world; at 0x23B6F0 `movzx edx,byte [edi+4]` -> `movzx edx,word [edi+4]` (the fifth argument carries the flags in its second byte, which on_player_killed never read); at 0x23814F (`mov edx,[esp+30h] / mov ecx,[esp+2Ch]`, INLINE, before mastery_icon) `call kill_feed_row` + three nops: SetMember (the description's interface vtable +14h, as the site's own calls) of `penetration_icon`, `ricochet_icon` and `pain_icon`, GFx ints 0 or 1, then the two moves. The hud movie's `CombatLogItem` shows `CombatLogWallIcon` (0.69d0's icon_penetration.png), `CombatLogRicochetIcon` (0.69d0's icon_ricochet.png) and `CombatLogPainIcon` (a white lightning bolt for pain shock, drawn by make_icons.py) after the weapon, before extra_icon, for a kill or a suicide row; the build adds the three bitmaps (`flash/hud/bitmaps/`, `tools/actionscript/flash_tree.py`).
- **What changes:** the kill feed's row only. The flags are read from the simulation's state (the bullet, the pools) in every game's own run; the node's +5 was zero and is read by the hud alone; nothing a match computes changes (no `simulation_changes` row).
- **Checked:** `selftest_026g.py` check 67; `check_kill_feed_026g.py` (Unicorn): the event in 26 cases (a bullet death, a pain death, one hit emptying pain and body or pain and head together (no pain icon: the owner, damage wins), a headshot through pain, pain all but empty, a toxic death, through a wall, a ricochet, pierces and a ricochet (the ricochet only, as 0.69d0), a ricochet then a pierce, a ricochet to pain, a ricochet with no killer (none), melee hits with a straight bullet, with segments and with a reflection (none) and by pain (pain only), through another player, a wall with no killer (no wall), a headshot through a wall, a wall and pain, no bullet, no damage model, the parts in another order, no head pool), the handler's argument and the row's members for each, against the original. **Tested in game by the owner, 2026-10-25** ("works nicely"); the explicit melee test (the event's melee flag) came after that test and is checked with Unicorn only.

### 3.56 A fall's speed counts the time it fell, not the steps it took (written 2026-10-25, README patch 72, the next release's revision; the rules: `fall_damage.md` "Split ticks") — PROVEN in the code, step_down's free fall run with Unicorn; not run in a game

The owner, 2026-10-25: "the fall damage problem is back… it seems to be something stacking up"; "you can now die from falling 1 meter".

- **The cause** (PROVEN). `game_world_core::tick_events` (0x2D21E0) ends each step of the world at the next 50 ms boundary or at the next input's time (+1BCh of its node), whichever is first. A precise input (a change of the fire bit at an off-grid time, sent as 0xDD; `match.md` §5.2) is such a time, in every game alike, so the tick it falls in is stepped in two parts, and each further one cuts it again. Each step ticks the physics (the updater 0x2D1D00 -> `bullet_physics_world::tick` 0x75550: `stepSimulation(ms x 0.001, 3, 1/60)`, whose `updateActions` (0x9B810) runs once with the whole dt). `bullet_character_controller::updateAction` (0x7B730) runs `player_step` (0x7BDA0) for dt = 0.05 (+2C704h, the constant the constructor writes at 0x7B4A1), or for any other dt when the body moved since its last step (+0B0h against m_last_pos +50h), as a falling body has. player_step moves the body dt / 0.05 of the step it computes (0x7C734), but step_down's free fall (0x7E6E6..0x7E75C) took gravity x 0.05 off the velocity at every call. A tick in k parts so gave a falling player k ticks of speed: 0.49 m/s more a part, and a landing (on_physical_controller_landing 0x2D8220, the velocity's vertical part) as from a fall k times higher. Deterministic: every game cuts the same ticks, so nothing desyncs.
- **Not the cause** (PROVEN). The history: the controller's velocity, flags and m_last_pos are in its serialize (0x7F7F0, `m_fall_and_slide_velocity`), inside base_player::serialize (0x2DAF60), which game_world_core::serialize (0x2D2570 -> 0x3218E0) writes into each history item; deserialize (0x7FB10) reads them back and base_player::deserialize then sets the body's transform (0x7F6D0). A frame's step past the newest item restores that item first (game_world_core::move 0x2D3568..0x2D357A), so a frame's partial step never stays. No real time: gravity's dt is the constant.
- **The patch** (`FALL_GRAVITY`; in place, two sites): the free fall takes xmm3 = min(dt, +2C704h), dt read from player_step's frame (step_down's saved ebp, +8: step_down's only caller is player_step, 0x7C4F6; both start `push ebp / mov ebp,esp`), for the three products by xmm3 that were the tick's. 0x7E6E6: `lea eax,[ebx+2C704h] / movss xmm0,[eax+4] / jmp 0x7E74C`; 0x7E720: the velocity's three updates through eax, then the dt load (`mov ecx,[ebp] / movss xmm3,[ecx+8] / minss xmm3,[eax]`) and `jmp 0x7E6F6`. 0x7E6F6..0x7E720 (the three products, with the up axis's three relocated addresses) are the original's. eax and ecx are written at 0x7E762 and 0x7E75C before any read. No base relocation under a site; no branch aims inside one.
- **What changes:** a tick stepped in parts adds one tick of gravity in all; a whole tick's dt is the constant itself (`50 x 0.001f` as a float), and minss gives the constant back, so whole ticks are the original's bit for bit. A simulation change (`simulation_changes.tsv` `next` row).
- **Checked:** `selftest_026g.py` check 72; `check_fall_gravity_026g.py` (Unicorn): step_down from its entry on a made-up controller (the sweep finds nothing), 40 whole ticks bit for bit the original's, ticks of 20 + 30, 5 x 10 and 7 + 43 ms one tick of gravity in all (the original: 2, 5 and 2), a dt over the tick one tick, the velocity's other parts and the saved registers kept.

### 3.57 The baked camouflage and decal textures are kept, and baked or loaded once while asked for again (written 2026-10-25, README patch 70, the next release's revision; the rules: `baked-textures.md`) — PROVEN in the code and in the owner's log; the sites run with Unicorn; not run in a game

The owner's report (2026-10-25, revision 33): short freezes in matches at ~100 fps. The owner's running game's log (`Survarium_sheep.log`, 14:55..15:11): bursts of "camouflage/decal cook [ bake …, compress … ]" in matches (14:56:30, 15:00:14, 15:01:17, 15:01:29, 15:01:42, 15:04:12, 15:05:42, 15:06:54, 15:07:12, 15:07:26), 6 to 51 bakes each, 0.06 to 0.4 s of them back to back. Each is a new file in `streamed_textures`, its mtime the burst's second. That log has no "remove file" line.

- **The folder is deleted at every start and exit** (PROVEN). `survarium::game::game` (0x1E9070) at 0x1E9351..0x1E9455 and `game::exit` (0x1EC730) at 0x1EC74D..0x1EC832 convert `user_data/streamed_textures` to its physical path and call 0x1E8DA0: SHFileOperationA (delay import 0xAEFE78) with wFunc 3 (FO_DELETE) and flags 414h (silent, no confirmation, no error UI). The baker's list lives only in memory (the static 0xA264E80, set up by 0x12CCE0: list 0xA2650B0 begin, end, capacity end 0xA2AE0BC, storage 0xA2650BC). So each game bakes every item it sees once. The log agrees: all 141 files are from this game (14:55 on), and the first bake's "GetDiskFreeSpaceExW error: 2" is the folder not being there yet (remove_unwanted_files 0x131F90, at 0x13207A).
- **Within a game the list works** (PROVEN). "%s, new id = %u" (0x131E53) is check_loaded_baked_texture's (0x131B40) good load, with the age renewed: a cache hit. The second match (15:10:48) loaded ~200 textures from the folder and baked none.
- **Duplicate bakes** (PROVEN). translate_query (0x12D1D0) looks the path up (0x131ED0) and bakes when there is no record. process_compression (0x12EC40) pushes the record (0x132640, at 0x12F3F2) only once its bake is done, so every query of the path before then bakes it too. The log: 15:06:54 baked 18 textures into 6 files, 15:00:14 12 into 6, the minute 15:01 51 into 21; the first match 115 into 62. A place has a player object for each of its three kits (section 3.34). #400 (revision 32, server only) gives a new account three kits wearing the same seven items: a newcomer's three objects bake the same textures three times.
- **The type queries** (PROVEN, the diagnostics' signatures). on_baked_texture_loaded(queries_result&, camouflage_decal_texture_type_cook_parameters*, render_surface_instance*, query_result_for_cook*) and process_compression(…, render_surface_instance* (stack arg 5), …, decal_texture_type (arg 9), …) put the texture into the surface's slot of its type (surface+4×type: 0x12C6D0..0x12C760, 0x12F0E0..0x12F168). They give the query a resource of 108h bytes (0x12DE30 on the allocator [1F1D960h], 0xE4F90) through 0xF0080 (edx the query; the pointer, 0x21E76C8, 108h; `ret 0Ch`) and finish it: finish_query_impl 0xF0D00 (stdcall: query, result, 1, 0; `ret 10h`; results 1 error, 2 postponed, 3 success, 4 requery, 5 out of memory). The "cook of '' [class 107] failed" lines are fresnel queries of surfaces with no `t_fresnel` (0x12DA01).
- **The patch** (`BAKE_CACHE_HOOKS`, `BAKE_CACHE_KEEP`; bake_cache_026g.asm and .c in the arena's bake_cache.code and .state):
  - 0x1E9455 `call 0x1E8DA0` (CALLEE) → bake_cache_start. bc_open reads the folder's `srp_baked_textures.txt`. If it says "Skhid baked textures, revision N" for this exe's revision byte (0x380866), each `*.dds` that is a whole DDS ("DDS ", header 124, DXT1 or DXT5, the file 128 + its linear size) goes into the list as a record (path `user_data/streamed_textures/<file>`, CRC 0, its size, an age; the list's bytes grow by it), at most 512 files and 512 MB; the rest are deleted. Otherwise: the stock delete, then bc_stamp makes the folder and writes the stamp.
  - 0x1EC832 (game::exit's delete): a 5-byte nop.
  - 0x131C90 `cmp eax,[edi+114h]` (INLINE) → bake_cache_adopt: a record of CRC 0 takes the CRC loaded, then the compare.
  - 0x12D4CD `call 0x131ED0` (CALLEE) → bake_cache_lookup: bc_lookup registers the first query of a path as its owner (with its surface and type). A later query of the same path and type, while the owner is listed, becomes a waiter: it takes a reference to its model instance (parameters+790h, count +0D0h) and leaves translate_query at 0x12D42D, unfinished, with no look-up and no bake. 0x12D42D is translate_query's own end after a refused query: the surfaces' list emptied, its own reference released, `ret 4`.
  - 0x12F18F and 0x12DDB4 `call 0x420789` (CALLEE) → bake_cache_baked / bake_cache_loaded, once the texture is in the owner's slot. bc_take takes the owner out of the table if the texture is in its surface's slot of its type. Then, for each waiter: its slot gets the texture (count +4 up, the old texture released through 0x3BEE00); its query gets a resource as on_baked_texture_loaded makes one (0x12DD53..0x12DE0E's sequence) and is finished with 3; its reference is released (0xE0210 at zero). Then `lea eax,[ecx+8]` as 0x420789.
  - 0xF0D00 (ENTRY) → bake_cache_finish: with owners listed, a finished query that is an owner is taken out. If it has waiters: the function, then each waiter's query made again (translate_query, its cook), then its reference released. Every other query: the function at once (`push ebp / mov ebp,esp / and esp,-8`, then 0xF0D06).
  - The tables: 64 owners, 192 waiters, 16 per owner, behind a spin lock (no game call is made while it is held). An owner still listed 10 s after its look-up (GetTickCount, import 0x870090) is taken over, waiters and all, by the next query of its path: a guard for an owner that never ends (none is known).
  - The CRC (the owner's live game, 16:37:47: a 0.4 s stall in a 9-texture bake burst, almost all in 0x42050's loop): 0x42050 (eax &the CRC, edx the bytes, [esp+4] the count; `ret 4`; 43 callers, among them process_compression at 0x12F340 over every baked DDS and the loaded resource's CRC 0x3C0F70 at 0x3C0FF0 over every file read) is a byte a step, `crc = T[(crc ^ b) & 0xFF] ^ (crc >> 8)`, T at 0x225FD68 made by 0x1C210 from 04C11DB7h reflected (EDB88320h); the callers do the start and the end. Its entry jumps to bake_cache_crc32 (ENTRY): the bytes to a dword one at a time, then eight a step through eight tables made once from EDB88320h (bc_crc_build), the rest one at a time; [eax] the new CRC, ecx it, edx past the bytes, the others kept, as the original. Unicorn: 24 buffers of 0 to 10 MB from starts 0 to 3 past a dword with random CRCs give the original's CRC and registers; 3.25 instructions a byte against 11. Not done: check_loaded could trust a record's CRC and skip the read's; it would lose the check against a file another game (the replay viewer) rewrote or a damaged one, for what is now a third of the cost.
  - The stamp after a delete (bc_stamp): every `.dds` left is deleted one by one, and with one left (held open by another game) the folder is not stamped, so the next start deletes it again.
  - Held textures: a load's texture shared (bake_cache_loaded, bc_take's check 2) is also held for the game: its count +4 one up, its path's record size counted, at most 256 textures and 256 MB, the least used let go (bc_release_texture: the count down, 0x3BEE00 at zero, as 0x12C6D0). bc_lookup answers a held path with the texture: bake_cache_lookup completes the query at once (the waiter's sequence) and leaves translate_query at 0x12D42D. Why: the owner's log (15:39:59 match) reads one player's `renesanse_heavy_com_tank_01_torso_s1{d,n,s}` from disk again at 15:40:43, 15:40:57 and 15:43:59, once per respawn: the player's model is built again, and the old instance's textures went with it.
- **Nothing a match computes:** textures only. No `simulation_changes` row.
- **Builds:** without a server sha256 `d1c5510502230dda0e0362b30c588a81fe5c3928849d0687ef26cca292f97f94`, with `--server=skhid.link:1234` `f0538e3b2bf059e2248858f5a9e8dddcd5157c85a5ee1f495a558c5cc50dc7db` (the revision byte REVISION's, 33); the test build `--revision=34 --server=127.0.0.1:2234` `7144092fee48fdff7f3011693d284dbaaa769a156c8872d64b851a4241cfb03f`.
- **Checked:** `selftest_026g.py` check 70 (the sites' instructions in the original; the hooks; no relocation under them; no branch into them; the link's code). `check_bake_cache_026g.py` (Unicorn 2.1.4) runs the sites from the game's code:
  - game::game: a stamped folder of this revision is kept and goes into the list; a file cut short is deleted; past 512 files the rest are deleted. Another revision, no stamp, or no folder: deleted and stamped. The original deletes every time.
  - The game's own check_loaded: a kept file's first load is used and takes the CRC; a second load of other bytes is erased.
  - translate_query from 0x12D4C5: the original bakes for both queries of a path. In the build the second and third wait (out of translate_query by its own end, registers back); another path, another type and a 17th query go on.
  - process_compression's and on_baked_texture_loaded's sites: the waiters' slots, references, resources and finishes.
  - finish_query: an owner's error makes its waiters again; a texture not in the slot shares nothing and makes the waiters again at the owner's end; an owner past 10 s is taken over; any other query goes straight through.
  **Not run:** a game. The checks use stubs for allocation, the resource, set_resource, the finish's body and Windows (a made-up folder).
### 3.58 Blackout: a battery match with no interface, the players' markers on Tab (written 2026-10-24, the Rust framework's 2026-10-26; README patch 65, a prototype; the rules: `blackout.md`) — PROVEN in the code, the options read, the markers' feed, Tab and the input run with Unicorn from the game's own code before each site; not run in a game

- **Bands** (parked, branch `feat/blackout-bands`, the owner 2026-10-26: the release goes without
  them): a team colour painted into each torso's sleeve, carried in each player's profile; the
  findings stay in `tools/re-kit/v026g` (profile_skin_visual_cook's records, a part's material).
- **The flag** (PROVEN). match_options' +57840h (40h bytes) is the "Mode" string, only emptied
  (0x3183E3) and logged (0x27BD08); patch 63 keeps its numbers at +57860h, patch 65 "SRBO" at
  +57870h. The options are read for 0xE4 (process_static_match_info, its call at 0x27B928: ESI the
  options, which the call keeps; the reader {begin, cursor, size} its local at EBP-2Ch) and for a
  replay's chunk 1 (0x1F096C: the options the function's first argument, [esp+40h]; the reader its
  local at esp+14h). After either call the reader's cursor is where the options end: srp's block
  is read from there (patch 63 reads it inside, at the function's end 0x1F0F30, the same bytes).
- **The markers** (PROVEN in the code). The player icons' movie (`player_icons.swf`) is loaded with
  each game world (game_world::load, 0x1F8F69) and fed every frame by update_player_icons (0x23AA10,
  from update_ui at 0x23B207: create_player_icon 0x23BE57, the names 0x23C00D, the props 0x23B1C0).
  Its ids: a player's is his index (player::initialize_player_icon 0x1F29D0, 0x1F29E8); the
  match's objects' from 25h (initialize_match 0x235771.., update_player_icons 0x23AA83: their
  own list, each visible by team). The first Blackout build hid the movie at initialize_match's
  reset with root.hide_interface and the owner still saw a teammate's name and marker: LIKELY the
  hud then still held a movie that did not take it (the game world's own comes with it). So the
  forced state is told from the feed: at update_player_icons' start (each frame) the movies the hud
  holds are told it at a change of movie or state, and again while the Invoke answers 0. Forced,
  the player icons hide their objects' icons (ids from 37) and keep the players' as in a plain match
  (the teammates' always, an enemy's never; the owner, 2026-10-31: before, only while Tab was held).
- **Tab** (PROVEN in the code). game_world::on_keyboard_action (0x25CD00) asks the keyboard for DIK_TAB
  (0x25CD22, push 0Fh) at every key and passes it to game_world_ui::show_player_list (0x2384C0, esi
  the hud, its state +0BAh); Tab is no action of the binder. process_first_person_mode (0x241250,
  ebx the handler) builds the input's actions mask at handler+33Ch from action_present (0x242460)
  per action (bits 0..28: forward 0Bh 1, back 0Ch 2, rstrafe 0Eh 8, lstrafe 0Dh 4, jump 4 10h, fire
  26h 20h, reload 27h 40h, aim 28h/29h 80h, crouch 5/6 100h, sprint 7/8 200h, fire mode 9 400h, ammo
  type 0Ah 800h, wpn_1/2 1Dh/1Eh 1000h/2000h, quick use 39h..3Eh 4000h..80000h, breath 2Ah 100000h,
  use 35h 200000h, drop 36h 400000h, wheel 24h/25h 2000000h/4000000h, grenade 20h 800000h, melee 21h
  8000000h, spot 22h 10000000h) and returns through 0x2421B0 (its early exit at 0x241284 too).
  After show_player_list, `test bl,bl / jnz 0x25CF0D` (0x25CD39) sends every key to the function's
  end while Tab is down: patch 58's kit keys (0x25CD41) never get one. In Blackout the hook clears
  bl after the call's byte is stored (0x25CD28: [esp+14h], loaded at 0x25CD2C), so 1 to 3 reach the
  kits Tab shows and show_player_list still gets Tab.
- **The kits** (PROVEN in the code). game_world_ui::update_local_player_state (0x23B740) shows
  them at a death and hides them at a respawn (show_profiles_selector 0x2383B0 at 0x23B7F7: the
  kits, root.select_player_profile and the cursor); a pick is the movie's `profile_changed`
  (scaleform_callback 0x233F06: game_world::select_profile 0x1F8070, then
  show_profiles_selector(false), which does nothing while the game's +53C8h is set, 0x2383D8).
  The mouse reaches the hud movie while the cursor is shown (game_world::on_mouse_key_action
  0x25CF60: 0x2AF610 with the handler's +0Ch movie), Tab or not.
- **The patch:** README patch 65: the Rust module `patches::blackout` (rust/patches/src/
  blackout.rs and blackout.toml; `rust_patches.md`), five `lohook`s at sites of its own
  (`sites_026g.tsv`; their bytes read from the original exe at every build): `static_info.options_read`
  0x27B92D, `replay.options_read` 0x1F0971, `player_icons.update_start` 0x23AA10,
  `keyboard.tab_state` 0x25CD2C, `input_builder.end` 0x2421B0. Its state is the crate's
  (`patches.state`). No other patch is touched: patch 63's 0x1F0F30, Research's 0x23B207 and patch
  62's sites stay theirs alone. Movies: the hud's and the player icons' `force_interface_hidden`
  (forced, `hide_interface` does nothing: F11 and a match's reset leave it). No content of its own.
- **Checked:** selftest check 65; `check_blackout_026g.py` (Unicorn, from the game's code before each
  site to past it, the original beside); `cargo test` (rust/); `scripts/blackout-kits/check.sh`
  (Ruffle). **Not run in a game.**

### 3.59 The lobby's scene is the player's pick (written 2026-10-29, the pick 2026-10-30, README patch 81; the rules: `lobby-scenes.md`) — PROVEN in the code, the nine sites run with Unicorn from the game's own code before each, the swap against the original's halves; the start run in a game (headless, a local-clients prefix); a change shown at once captured in a game (2026-10-30, the diagnostics' forced picks and squads)

The owner (2026-10-30): "a lobby selection screen": a button opens four cards (since 2026-10-31 the player panel's, in the premium button's place, lobby-scenes.md) (the Room `camp_069`, the default; the Basement, the classic `day`; the Bunker `stk2_bunker`; the Hangar `hangar_046`); a click shows that lobby behind a fade; in a squad every member is shown its commander's pick. The four lobbies ship with the game ("lobby files can't be missing"): a project that fails to load is a bug, logged, with no fallback state.

- **How the scene loads** (PROVEN). The lobby menu is made once, at the game's start (`game::create_lobby_menu` 0x1EC0F0, from `create_and_assign_network_client` in on_base_resources_created), and its ctor (0x21C980) calls `lobby_menu::query_resources` (0x21E450, eax the menu, one caller 0x21CDBD). That builds eight requests at [ebp-80h] ({game_scene, 64h}, {game_scene_view, 65h}, {sound_scene, 26h}, {lobby_scene/day, 40h} at [ebp-68h]/[ebp-64h] written at 0x21E5A2/0x21E5A9, cursor.swf, lobby_menu.swf, default.player and static_game_parameters.options), their user data at [ebp-40h] (the project's, [ebp-34h], the variant of the menu as its base_game_scene), and queries them (0xE5B60 at 0x21E5ED, count 8) with `on_render_scenes_ready` (0x220B20, thiscall, `ret 4` at 0x221E77) as the callback. That reads result i's resource at +12Ch + 2E0h·i without a check: the project (+9CCh) less 38h kept at lobby menu+114h (0x220E01), inserted (vtable+0Ch; 0x285540 later), its track (project+1E4h) given to the track player at menu+118h (0x220E80, unchecked), its config's `camera` read (0x221975). Nothing else asks for the scene again: `lobby_menu::clear_resources` (0x21D080, vtable+28h: 0x2855B0 the project's remove, the static collisions at project+28h, the track player, the release) runs only from `game::clear_resources`, at exit. A match does not unload the lobby.
- **The sign-in comes after the scene** (PROVEN). on_render_scenes_ready makes the network client itself (game::create_network_client 0x1EBFC0 at 0x221C2F), so the sign-in and its first `0xA1` come after the scene's results are taken: the start shows the saved pick.
- **A missing project crashes** (seen 2026-10-29, a local game with `lobby_scene/dax` asked): `project_cooker_simple::translate_query` (0x22B790) queries `%sprojects/%s/client_project` (and `default/client_project`) and its callback reads the config's root (+108h) of a null config: an access violation at 0x22C11E (create_game_objects 0x22C010). A missing *config* asked as a binary config (class 1Fh) fails that result alone.
- **The patch** (Rust `lobby_scene`, `rust/patches/src/lobby_scene.rs`; its state the crate's):
  - `configs.start` (0x1EA210, the chain after patch 79's and patch 83's handlers: a shared lohook site, rust_patches.md): the console variable `srp_lobby`, a saved string of 4 bytes, registered once before user.cfg runs; read once into `Lobby` (no value or another: the Room), written (user.cfg, 0x1E9030) when the pick changes.
  - `lobby_menu.scene_request` (0x21E5CC, `movq [eax+8],xmm0`, the list whole and not yet asked): the fourth request is the saved pick's project (`lobby_scene/<id>`, 40h, the lobby menu's variant still its user data). A change's query has its lobby's project alone (the other seven requests its config, `resources/projects/lobby_scene/<id>/client_project`, 1Fh, user data 0).
  - `lobby_menu.scenes_ready` (0x220B20, the callback's start): the start's results go on into the original's code (a project without its track, +1E4h of the project, +1ACh of its resource, logged in srp_patches.log first). A change's project is put in the old one's place there (`vostok::lobby_menu::put_in`: clear_resources' project half 0x21D080..0x21D100 and on_render_scenes_ready's 0x220DA2..0x220EC3 and 0x22193E..0x221975, each step the game's function: remove_static 0x2855B0, static_collision's remove/insert 0x2F8970/0x2F8850 with the physics world +3104h, the track player's stop 0x2DE7F0 and play 0x2DE090, the release 0xE0210, simple_game_project's vtable+0Ch 0x2F8720, insert_static 0x285540, and the track emitter's emit (vtable+20h) and the effect's destroy (its emitter's vtable+1Ch) through the library's virtual rows, 0.5), the eye adaptation reset (the byte 0xAC914C: compute_per_pixel_eye_adaptated_luminance 0x1B1400 takes the frame's own luminance as the last one's while it is set, 0x1B14A6, and clears it, 0x1B14F2; 1 in the image, nothing else of the game sets it), and the callback returns (ret 4) (`engine_lobby_query_resources` asks again for the lobby wanted when it changed meanwhile); a change's project without its track: logged, nothing put in, the black stepped out.
  - `lobby_camera.view` (0x2840F2, the chain after patch 45's flight and the diagnostics' sweep): a frame of the lobby drawn. With another lobby wanted than the one shown the black is stepped in (`root.srp_scene_fade(per mille, loading)` on the lobby movie [[menu+690h]+108h]+4, fill_squad_info's 0x2277E1; by min(dt, 1/30 s) / 0.25 s a frame), then whole with the spinner the query is asked (`engine_lobby_query_resources`) for its project; after the swap, the camera at its ctor's pose (0x283C24..0x283C78: yaw, pitch, side 0, distance 1.96; not while patch 45's camera is at the bench), six frames, the adaptation reset again and the black stepped out (0.4 s). The lobby window is told `root.srp_lobby_state(pick, shown)` when one changes.
  - `lobby_client.squad_id` (0x2747C3, read_squad_info with the squad id kept, both paths: `mov byte [ebx+1EAAACh],0`, ESI the reader past the id) and `lobby_client.squad_read` (0x2748BC, a squad's revision kept: `mov [ebx+1EAAA0h],ecx`, ESI the reader past it): srp's two bytes (for no squad after the id, for a squad after its revision), required (a message without them is logged as srp's bug): the own pick saved (0, srp has none at each sign-in: the saved one sent, `3B` at 2 095 000 000 + id through the lobby client's 0x270070, EAX the client, ret 8, as the movie's request_players_range 0x28C960), the commander's pick `Squad::Led(lobby)`, else `Squad::None`. (0x2748C2, where both paths meet, has a base relocation under it: cmp dword [1F1D60Ch],0.)
  - `lobby_menu.deactivate` (0x21D4A0, on_deactivate's start, the lobby menu's vtable+4 0x9FFE74: the active scene switched away, a match) and `lobby_menu.activate` (0x21D2E0, on_activate's, vtable+0): the lobby hidden and shown. Hidden during a change: the black gone; a project that comes while hidden is put in at once; shown again while it loads: the black and the spinner again.
  - `game.clear_resources` (0x1ED030, the game's vtable 0x9FAC8C+10h, span 10: push ebx / push esi / mov esi,ecx / mov ecx,[esi+535Ch]): the game closing. It deactivates the active scene (0x1ED0B0, [+539Ch]'s vtable+4), the lobby menu too; from it on nothing is asked and results are let go without the menu being touched.
  - The pick and the squad are enums parsed at the boundary; the scene's state one enum (`Scene`: unasked; starting with what it asks; loaded with the lobby shown, its view and its change: settled, broken, darkening, loading, holding, brightening) with one transition function (`Scene::step`, six events: the query, its results, a tick, hidden, shown, closing).
- **The lobby window** (`flash/lobby_menu/src/survarium/lobbypick/LobbyWindow.as`, the movie's ActionScript; the button in `StatusPanel`): `request_players_range(2 095 000 000 + id, 1)` on a click; srp (`lobby_pick.rs`, `Player::lobby_pick`) gives the pick back in `0xA1`.
- **Checked:** selftest check 81 (the nine sites; the stock code the swap redoes, read from the original; the squad reader's paths; the eye adaptation's byte; the movie's holder; the 3B sender); `check_lobby_scene_026g.py` (Unicorn: the variable and a user.cfg line, the start's query for no pick, the Basement, an unknown value, each lobby, a project without its track (logged, taken), each query's other requests and user data the original's; the window told; a pick's black stepped a frame each, whole with the spinner and the project asked alone, the swap's 14 calls and the lobby menu's +114h/+118h, both counts and the effect's count equal to the original's halves run on the same made-up objects, the eye adaptation's byte, the camera's pose, six frames then the black stepped out; a stall's frame stepping the black by a 30th of a second; a project failing behind the black (logged, nothing put in, faded back, not asked again); `0xA1`'s picks: 0 sending the saved pick, the own pick saved, the commander's asked; a match entered during the fade and behind the black, the return while it loads; the close); `check_language_026g.py` (patch 79 in the chain); `scripts/lobby-fade/check.sh` (Ruffle: the movie's black under a panel, the spinner, the window); `cargo test` (rust/; the lobby-server's `a_squad_info_carries_the_lobby_picks`).

## 4. Mode 3 batteries: not needed in 0.26g

- 0.26g splits the rule: `match_time_vi_spawn_rule` (vtable 0xA06C04) wraps `match_time_vi_spawn_rule_core` (vtable 0xA15CAC). The battery tick is no longer a vtable slot: the core's ctor 0x3147A0 stores a delegate `{this, 0x3148A0}` at +0x1C4 / +0x1C8 (0x3147F1). [V]
- **Tick 0x3148A0** (0.21d `match_time_vi_spawn_rule::tick` 0x1FC388; thiscall, 3 args, `ret 0Ch`): the same LCG (`imul 8088405h; inc`, 0x314929), the same re-roll test `(variation − start + t) mod spawn_time == 0`, the same `t ≥ next` placement. Fields: items +0x110 / +0x114, spawners +0x120 (position at +0x18), LCG +0x140, start +0x168, **next time +0x16C**, A +0x170, spawn time +0x174, variation +0x178, started +0x17C (the tick itself now starts the rule once core+0x18FE4 == 3). A battery that is still inserted (+0x2C8) is removed (vtbl+0xA0) before it is inserted again (vtbl+0x9C). [V]
- **The 0.21d bug is fixed:** `serialize` 0x3149B0 writes, and `deserialize` 0x314B90 reads, +0x168, **+0x16C**, +0x174, +0x178 and +0x17C (0x314B22 / 0x314C82). A rollback restores the next-battery time, so the re-simulation places the battery again. [V] Batteries placed beyond the restored A are still not taken back by deserialize (it loops over the first A only, 0x314BE8–0x314C43), but the re-simulation up to the present re-places them (remove + insert) within the same `move()` [I]. No patch is drafted; a live mode 3 match should confirm.

## 5. Manually identified 0.26g functions (not all in the automatic map)

0x124D0 single-instance check · 0x27A300 `network_client::on_match_packet_received` · 0x27AA20 `network_client::on_match_ready` · 0x27CBF0 `network_client::try_send_all` · 0x27B830 / 0x27B880 / 0x27B570 / 0x27B770 / 0x27B7F0 / 0x27BFF0 / 0x27C5F0 / 0x27C690 = handlers of 0xE3 / E4 / E5 / E6 / E7 / E8 / E9 / EA · 0x27A1B0 `match_client::connect` · 0x279A60 / 0x279B10 sending / received message info · 0x278D00 `udp_match_packet` ctor · 0x37A0E0 `network::match_client::new_packet` · 0x324BE0 `game_state_history_item::serialize` · 0x324D80 `game_state_history_item::deserialize` · 0x325150 `game_state_history_item::hash` · 0x325A60 `game_world_core::on_hash_mismatch` · 0x3285D0 `game_world_core::dump_game_state` · 0x2D04F0 `game_world_core::serialize` · 0x2D09C0 `game_world_core::new_game_state_history_item` · 0x2D0B30 `event_horizon_history_item` · 0x2D1070 `is_there_event_horizon_item` · 0x2D2E40 `game_world_core::move` · 0x7AB30 `mutable_buffer::resize` · 0x7AC10 `buffer_writer::w(src, n)` · 0x424CA0 `_alloca_probe_16` · 0x1C210 CRC-32 table init · 0x3147A0 / 0x3148A0 / 0x3149B0 / 0x314B90 `match_time_vi_spawn_rule_core` ctor / tick / serialize / deserialize.

## 6. A patch revision in the sign-in (added 2026-10-12) — PROVEN bytes, run with a client

A server cannot tell one patched exe from another: nothing a client sends depends on a patch. What a client says of its build is the 12-byte version field of the first login message and nothing else (login_chat.md §4.1 and §7.8; no message has a hash of the exe).

- **PROVEN, the field** (`sign_in_on_connected`, 0x380847..0x38088C): eight zero bytes on the stack (`pxor` / `movq [esp+20h]`), `"0.26"` at +0, `"g0"` at +4, the string's own zero at +6 (`mov cl,[9DAE9Eh]` 0x380862, `mov [esp+26h],cl` 0x38086C) and a zero again at +6 (`mov [esp+26h],al` 0x38087B, `eax` being 0); then the eight bytes and four more zero bytes (`mov [edi+8],eax`) go into the message. Bytes 7..11 of the field are always 0.
- **PROVEN, the server:** `parse_sign_in` takes the field up to its first zero, so what stands after it is not read by any server built from this repository so far.
- **The patch** (`REVISION_SITE`, full builds only; `--cert-only` keeps the code): the six bytes at 0x380862, `8A 0D 9E AE 9D 00` (`mov cl,[9DAE9Eh]`), become `C6 44 24 27 NN 90` (`mov byte [esp+27h],NN` / `nop`): byte 7 of the field is the revision NN. Byte 6 still gets its zero at 0x38087B (0x38086C then writes the `'0'` left in `cl` there first, which is overwritten). The base relocation at 0x380864 (HIGHLOW; the three of this code are at 0x380849, 0x380850, 0x380864) is made a no-op as the hooks' are. The string, the title and replays are untouched. NN is `REVISION` of `patch_026g.py`. It is raised in the change that makes a full build something else for the game: **1** is the patch set of this change (the VRAM check of §3.7 and this byte beside what the download of 2026-10-12 has). 0 is every exe without this patch: the download of 2026-10-12 and anything built before.
- **Revision 2:** adds the optional embedded login server in §3.8. Full builds now advertise 2, whether or not an address is supplied at patch time; cert-only remains 0. The revision-1 build hashes and live test below describe the earlier published download.
- **Revision 3:** adds the battery rule patch of §3.9, the first of these patches that changes what a match computes: a game of revision 3 and an older one stay in step until a team's last battery goes in, and part there (match.md §26). Revisions 0, 1 and 2 play together as before.
- **Revision 4:** adds the two start-up sites of §3.10 (a Documents folder the game may not write to). Nothing a match computes changes: revisions 3 and 4 play together.
- **Revision 6:** adds the three fixes of §3.13 to revision 5; it plays with 5.
- **Revision 5:** adds the late join of §3.11 and the game that goes on without the focus of §3.12, in the default build. The first revision that plays with none before it: the server has a queue for the games from 5 on and one for the others (`matchmaking.md` §9). `REVISION` of `patch_026g.py` is 5; a build without the late join (`--no-late-join`, `--cave=hash-mismatch`) says `REVISION_WITHOUT_LATE_JOIN`, 4. The exe built by hand before the release said `0x80`, which is over 5 and so in the same queue.
- **Checked** (`selftest_026g.py` check 11): the field is worked out from the instructions that fill it (the bytes of the string are read from the image): `0.26g0` and six zero bytes in the original and in cert-only; `0.26g0`, a zero, the revision in byte 7 and four zero bytes in the full builds, the EULA flag following as before. Of the three relocations only the one at 0x380864 is disabled; no branch of the function lands inside the instruction.
- **Why nothing else changes** (read in the exe, 2026-10-12): the replaced instruction only loaded `cl`, which `mov ecx,[ebp+0Ch]` (0x380870) overwrites before any other use; neither it nor its replacement touches a flag. The eight stack bytes are read once, by the `movq` at 0x38087F, and nothing of the function reads them after that (to its `ret` at 0x380930 it builds the send callback above them and sends). The message's length does not change. The four references to the version string are this code, the title, the `%s (%d)` text and the replay check (login_chat.md §7.8); the patch touches none of the string's bytes. Nothing of it is in the world a match computes, so a patched and an unpatched client play one match as two unpatched ones do.
- **Builds:** section sha256 `3c7d7bf0195db000f0c6dc81e57fa15a50decec4fa0c0ea5494f361f91049c9d`, hash-mismatch `9f7f8b65203df7abf9720a174ad6782db42a5e42517430ec700a0c97f4707e91`; 48 bytes in 15 ranges and 1207 in 58 differ from the original. The section build differs from the exe of the first download of 2026-10-12 (`166d25bc…1ddf`) in the VRAM byte (file 0xF5811), these six bytes (file 0x36FC62), one relocation entry (file 0xB86623, `38` → `08`) and the PE checksum, and in nothing else (compared byte by byte).
- **The server** (2026-10-12): `parse_sign_in` reads byte 7 of a whole 12-byte field whose text ends before it (`patch_revision`; 0 otherwise), and the login server writes it in the sign-in's log line: `Client version: 0.26g0, patch revision 1 (…)`. Nothing else is done with it yet (the owner: "only log at first"). Tests: `a_patched_0_26g_client_says_its_patch_revision`, `a_client_that_says_a_patch_revision_signs_in_as_any_other`.
- **Run with a client** (2026-10-12, a local server and one game with the section build): the sign-in was logged as `Client version: 0.26g0, patch revision 1`, a client that sends the stock field (the test bot) as `patch revision 0`; the game went through the lobby and a match. The players' download of 026g.skhid.link has this exe since 2026-10-12 18:43 UTC (`Survarium-0.26g-srp-files.zip`, sha256 `16eb1f23c99d5d73c5228c168021c58adff5e0ebf3d543984f916b416cef9528`); the first download's exe says nothing, so 0.
- **What it gives:** the version text stays `0.26g0`, so a server that knows nothing of the byte takes the new exe as it takes the old, and a server that reads it sees 0 from the earlier unversioned exes.
- **What a server can do with it beyond the log:** show it on the admin page; refuse a sign-in under a set minimum with the answer 0x22, for which the client shows its own `st_login_menu_status_invalid_version`, "You're using an outdated version of the game." (PROVEN: 0x22 → 0x21ACE5 → `set_status(9)` 0x21AD8D → the text key 0xA0005C at 0x21F0F3; the server has sent 0x22 to two sign-ins of another build, 0.26e0, in the kept logs); or, in a match of both kinds whose worlds part, take the newer one's world.
- **The check** (2026-10-13 at Play; since 2026-10-15 at the sign-in; `lobby-server/src/v026g/patch_check.rs`, `login-server/src/v026g.rs` `RevisionGate`; `SRP_MIN_PATCH_REVISION`, unset or 0 for none, and a value that is not a number stops srp at start): the session keeps the revision of its sign-in. A game under `SRP_MIN_PATCH_REVISION` is answered 0x22 at its sign-in, the game's own "You're using an outdated version of the game." on its login screen, before the password, so it gets no session; and a lobby sign-in (`2E`) with a session it had before a restart of srp is refused like an ended session (`FD`), the session ended. Its game counts that a failed try and goes to its login screen after the fifth (about half a minute, only while its window is active: transport.md §6.1), where its sign-in is refused. It so never gets the lobby's sign-in, whose price list (0x8A) names every item sold: a game whose resources.db lacks an item crashes on it (lobby.md §2.6). Before 2026-10-15 the check let the game into the lobby and refused only its Play (`84 <op> 00 <text>`, `SRP_OUTDATED_TEXT`), or with `warn` told it in the general chat how to update (`SRP_UPDATE_HINT`); all three are gone, as the lobby itself is not safe for a game that lacks the server's content. A revision over the minimum is let in (the `0x80` of a late-join exe built by hand); a test bot is never checked. On the players' server on 2026-10-13, 12:30 to 16:43 UTC, 30 accounts signed in: 19 with revision 0, 1 with 1, 3 with 2, 2 with 3, 5 with 4. Not yet seen in a real game: the 0x22 answer to a patched game.
- **What it is not:** proof. The byte is what the exe says of itself; a changed exe can say anything.
- **Not taken:** changing the string itself (`0.26g1`): one byte, no code, but the title and the replay check change with it, and every server not yet updated refuses the new exe. A hash of the exe sent by the client: new code in the exe for no more than the byte gives.

## Open questions

1. Did 0.26g's original server still supply 0xE8 itself (lockstep with a server-mode core), as 0.21d's did? The client code says so (frozen world, no upload), but the new 0xE5 / 0xE6 sequence bytes and the 0xDB timing exchange are not understood yet.
2. Compression of 0xE8 / 0xE4: which codec sits at `[nc+0x4C]` (vtbl+4 = decompress)? srp can avoid it for 0xE8 with flag byte 0; check whether 0xE4 (static info) has the same flag.
3. C→S 0xDC / 0xDD / 0xDE and S→C 0xEB–0xF5 are not mapped (input, input relay, discard, …). 0xDF carries the item time as one byte (time / 50): how should srp key its CRC table by it?
4. Answered (transport.md §4, §3.2 here): 0xE0 is reliable and plain; 0xF6 is unique (not ordered), variable length, and **needs at least one payload byte**.
5. Live checks, as for 0.21d: identical initial uploads from two clients; a 30–69 packet burst of 0xE0 arriving complete (statically safe, §3.2); 0xF6 `00` reaching the hook in state 2; the client starting at all with an 8th section (§3.3; fall back to `--cave=hash-mismatch` if not).
6. Mode 3 in 0.26g: confirm live that batteries stay without a patch.
7. Function map: game-code coverage is only about 30 %. More could come from matching the recompiled structure with constants and struct offsets masked, or from 0.26g's own `__FUNCSIG__` / log strings (244 FUNCSIG strings name 0.26g functions directly, including ones 0.21d lacks).

## 0. Code-signing certificate check: required for every 0.26g client (found 2026-10-12, PROVEN live)

A stock 0.26g client stops at start with "Resources archive is corrupted.
Please reinstall the application and try again" (string 0x9DEF00, pushed at
0x432E1 in `vostok::vfs::archive_mounter::mount_archive_to_parent`).
- The mounter reads the resources.db file table plus a 0x100-byte signature
  (size from the function 0x12A00) and calls the verifier installed at start
  (boost::function at 0x21E5158 = 0x13310, `vostok::crypt::verify_sign`).
- 0x13310 takes the exe's own Authenticode store (`GetModuleFileNameW`,
  `CryptQueryObject`), finds the certificate with subject "survarium.com"
  (`CertFindCertificateInStore`, CERT_FIND_SUBJECT_STR), then calls 0x12A10.
- 0x12A10 builds the certificate's chain for **now** (`CertGetCertificateChain`,
  usage OID 1.3.6.1.5.5.7.3.3 code signing) and fails on
  CERT_TRUST_IS_NOT_TIME_VALID (returns 0xA), revoked (0xB) or a base-policy
  error (3). The certificate (C=UA, O=Vostok Games. Survarium Inc.,
  CN=survarium.com, issuer Certum Code Signing CA) was valid 2014-06-06 to
  **2015-06-06**, so the chain check always fails today.
- Patch: 0x12A10 `83 EC 2C` -> `31 C0 C3` (`xor eax,eax; ret`; cdecl, no
  relocations in those bytes). The file table signature was still verified
  with the certificate's key afterwards, until revision 12 (§0.1). Live:
  the patched client gets past the check and opens its window.
- Ruled out: a damaged unpack (resources.db CRC 0x4A01F332 equals the 7z
  record) and a half-applied update (the leftover
  `temp/patch_temp/resources.db.patch` names target size 6,285,533,906 =
  our resources.db).
- 0.21d has `verify_sign` too, but its clients start; not checked why.

### 0.1 What is signed, and why it is no longer checked (patch revision 12, 2026-10-15/16)

- **The bytes** [V, PROVEN with the retail archive]: the file table, the
  node buffer after the 24-byte header, `[24, 24 + u32 at 20)`; RSA PKCS#1
  v1.5 over its SHA-1, 2048 bits; the 256 signature bytes right after the
  table, least significant byte first (CryptoAPI's order), then zeros to the
  first file. Retail: table `[24, 10 344 304)`, signature `[10 344 304,
  10 344 560)`, first file at 10 344 588. The files' bytes are not signed.
- **The key**: verify_sign 0x13310 takes the survarium.com certificate from
  the exe's own Authenticode signature and calls verify_sign_impl 0x12CB0
  at 0x135D3 (eax = the certificate's context; the table, its size, the
  signature, its size). Of the context it reads the encoding type (+0) and
  CERT_INFO.SubjectPublicKeyInfo (+0Ch, +38h) alone, for
  CryptImportPublicKeyInfo, then SHA-1 and CryptVerifySignature; 0 is good,
  8 a bad signature ("Resources archive is corrupted"). [V]
- **The result** [V]: verify_sign keeps 0x12CB0's eax (`mov ebx,eax`
  0x135DC, `mov eax,ebx` 0x13601) and returns it; the mounter's
  `test eax,eax` / `jz` at 0x432DD mounts on 0 and pushes "Resources archive
  is corrupted" (0x432E1) on anything else. So 0 is success at this call.
- **The first revision 12** (README patch 24 as first written, never released): the call at
  0x135D3 went to 413 bytes at `.srp`+C00h, which called 0x12CB0 once with a
  context made on the stack whose key was Skhid's. Only a Skhid-signed table
  started the game.
- **Revision 12** (README patch 24; numbered 14 while it was written, owner
  2026-10-16: it is revision 12): the call at 0x135D3, `E8 D8 F6 FF FF`,
  is `31 C0 90 90 90` (`xor eax,eax` and three nops), the same skip as
  resource-porter's `tools/patch-026-map-resources.py`; the `add esp,10h`
  after it still takes the four arguments off, and the caller still frees
  the certificate. No signature is checked. Why (the owner, 2026-10-15): a
  check in the player's own exe is patched out by anyone who wants to, so it
  protected nothing, and the updater already checks every byte it writes by
  sha256. The 413 bytes at `.srp`+C00h are int3 padding again; the
  shell_32.dll check stays at +E00h. Not run in a game.

### 0.2 resources.db's patches: more archives over it (patch revision 12, README patch 27, 2026-10-16)

- **The design** (the owner, 2026-10-16): `resources.db` stays the retail
  file; content comes as `resources.db.patch_<n>`, each a small archive of
  the same format, mounted after it at the same virtual path, `resources`,
  in the order of n. A patch is numbered by the patch revision that brought
  it (owner, 2026-10-16), so numbers are missing where a revision had no
  content: n runs from 1 to 255 (the revision is a byte) and a missing one
  is skipped. A later archive's file replaces an earlier one's
  and its new files are added; nothing is taken away. (This replaces #92's
  `resources_skhid.db`, one archive the updater would have rebuilt.)
- **Why a later mount wins** [from the VFS labels of #95, not re-read
  here]: `hash_multiset::insert` 0x45900 puts a node in front of the nodes
  of the same path whose mount id is lower (predicate 0x453D0, unsigned
  `mount_id_of_node(node) < new id`), `find_no_lock` 0x452E0 returns the
  first; mount ids only grow (`archive_mounter` 0x42D30, `lock xadd
  [1F1D33Ch]` at 0x42E44); the file system's tasks run in the order they
  were added (`add_fs_task` 0xF8480). A folder over a folder merges, a file
  over a file replaces it.
- **The handles** [V]: `windows_hdd_file_system` (vtable 0x9DE6D4: the
  destructor 0x3FFC0, `open` 0x40000, `close` 0x40510) keeps one handle at
  [fs+4] (-1 for none). `open` first asks whether the path ends in `.db`
  (0x40590 with ".db" 0x9DE5A0, at 0x4008C): if so and [fs+4] holds a
  handle, that handle is the result, whatever the path; else `CreateFileA`
  (0x400C8), and a handle of a path ending in `.db` is put at [fs+4]
  (0x40160..0x40174). `close` does not close [fs+4] (`cmp [ecx+4],eax`,
  0x40514); the destructor does. So resources.db stays open, and a second
  archive whose name ended in `.db` would be read through resources.db's
  handle: a patch's name must not end in `.db`. A patch's file is opened
  and closed by each read. The archive mount holds no handle of its own:
  in the engine's source (`resources_device_manager_thread.cpp`,
  `device_manager::process_query`) each query opens the file, reads and
  closes it, and the mount (`vfs_mount_archive.cpp`) opens the file table
  only while it mounts; the 0x40000 cache is the one handle kept that was
  found in the exe. The 0.26g device manager (process_query 0x10B890, bound
  at 0x10B75C) was not followed to its open and close instruction by
  instruction. A patch is small and read once per file, so the cost is an
  open per file read of a patch.
- **The start-up mount** [V]: the start-up (0x14E8A) makes a
  `mount_by_config_helper` (0xE7D20) for "resources" with the callback
  `on_mounted_resources` 0x147E0 bound to engine_world+2D4h, where the
  mount is kept (m_resources_mount, the first argument), and +2C0h, the
  shaders' masks (m_shader_mask_config) (0x14E1F..0x14E50; `mov dword [esp+1Ch],147E0h` at 0x14E34 is the
  function's only reference in the file). `query_db` 0xE7D90 formats
  `"../../%s.db"` (`push 9E705Ch` at 0xE7DAD; 0x18190 formats with `/` made
  the string's separator, `\`) into a native path string on its stack
  (begin, end of the text, end of the buffer, 260 characters, the
  separator at +110h), and calls `query_mount_archive` 0xE7EF0 at 0xE7E3E,
  its only caller: esi the allocator, then cdecl the virtual path string
  ("resources"), the path string twice (the file table and the archive are
  the same file), the descriptor (`m_mount_id.c_str()`) and the callback
  (the helper's first member). It allocates a task (0x3E0 bytes) that
  copies all of it (0x105510; the helper is freed right after the call,
  0xE7E43..0xE7E6A) and adds it (0xF8480). The engine's source
  (`core/sources/resources_query_mount.cpp`) is the same: one
  `query_mount_archive(virtual_path, path, path, m_mount_id.c_str(),
  m_callback, m_allocator, NULL)`. The descriptor is copied into the mount
  root (`fixed_string<32>`, `vfs_mount_archive.cpp`) and only filters
  unmounts by descriptor: every patch gets "resources" too.
- **The callback** [V]: `on_mounted_resources(where it is kept, the next
  step, &mount)`, cdecl. A null mount calls the fatal error 0x113580
  (printf-like, never returns: 0x1134E0 and int3 after it) with "Cannot
  mount resources. Please reinstall the application and try again."
  (0x9DB7A0). Otherwise it takes a reference (`lock xadd [mount+1Ch],1`),
  stores the mount where it is kept, lets the one there go (0xDFE30 at
  zero) and starts the next step (0x176D0, 0xE5B60: the shaders' masks).
  `link_mount_ptr`, the engine's way to hold several mounts as one, is not
  in this build.
- **The patch** (`mount_patches_026g.asm`, 759 bytes at `.srp`+400h since patch 49, 707 before, after
  the upload code and the longest default server's stub and name, which end
  at +21Dh, and before the pickup code at +800h):
  - 0xE7E3E `E8 AD 00 00 00` (`call 0E7EF0h`) becomes `E8 45 D6 33 0A`,
    `call 0A425488h`, mount_all. It looks for
    `..\..\resources.db.patch_<n>` for every n from 1 to 255 (the same
    0x18190 with `"../../%s.db.patch_%u"` and the descriptor; `CreateFileA`
    with no access and OPEN_EXISTING, then `CloseHandle`, the imports at
    0x870048 and 0x870040) and writes down, in order, the n of each one
    that is there. A failed `CreateFileA` asks `GetLastError` (0x87005C):
    2 (file not found) and 3 (path not found) are a missing n, skipped
    unless the exe requires it (patch 49, section 3.46: then the same fatal
    as an unreadable file, with that error); any
    other error (a folder by that name and access denied give 5, a sharing
    violation 32) is the fatal "Cannot open resources.db.patch_<n> (error
    <e>). Please run the Skhid launcher again." before anything is queued.
    It writes the count of callbacks to come (1 + patches) to its state
    before it queues anything, then calls 0xE7EF0 with the game's five
    arguments, then once for each patch written down with its own path
    string in place of the two paths. All registers are kept (pushad /
    popad); nothing it needs survives a call in a register.
  - 0x147E0 `55 8B EC 83 E4 F8` (`push ebp` / `mov ebp,esp` / `and esp,-8`)
    becomes `E9 1B 0C 41 0A 90` (`jmp 0A425400h` / nop), on_mounted. When no
    start-up mount is pending it runs the six bytes and jumps to 0x147E6:
    the game's own. A null mount for the k-th callback: k = 0 the game's own
    (its fatal), else the fatal "Cannot mount resources.db.patch_<n>.
    Please run the Skhid launcher again." with the n of the k-th patch
    written down. Otherwise it takes a reference (`lock inc [mount+1Ch]`),
    keeps the mount at the next of the state's 256 slots and counts the
    callback down (`lock dec`); all but the last return (the caller takes
    the arguments and lets its own reference go). The last calls the six
    bytes and 0x147E6 as a function, with the address of slot 0
    (resources.db's mount) in place of its third argument: the game's code
    takes engine_world's reference, stores the mount at engine_world+2D4h,
    lets the one there go (none at start) and starts the next step; back
    in the patch, slot 0's reference is let go as the game lets a mount go
    (`lock xadd [mount+1Ch],-1`, 0xDFE30 at zero, which never happens:
    engine_world holds it).
  - The state: 0x800 of `.srpd` (0xA428800 with the late-join code as
    built; the late join's own state is its first 60 bytes), 0x507 bytes:
    the count of callbacks to come, the count held, 256 mounts and the 255
    bytes of the patches' numbers. The .bin ends with the
    address it was assembled for, and `patch_026g.py` refuses a .bin made
    for another one. Neither site holds an address (no base relocation
    under them) and nothing aims inside them but their first byte (a
    `call 3DB6F0h` at 0x3C6F0A has the bytes of 0x147E1, no relocation).
- **The references, and the quit** [V]: a mount's count is at +1Ch; one
  nobody holds is unmounted (0xDFE30, `intrusive_fs_task_unmount_base::
  destroy` in the 0.10b names: it queues an unmount task, `add_fs_task`'s
  list and event). The file system holds a mount while its callback runs
  and lets it go after. In the retail game resources.db ends held once, by
  engine_world+2D4h. With the patch: resources.db once (engine_world's;
  the state's reference is let go after the game takes its own), each
  patch once (the state's), with any number of patches. (The first writing
  kept the state's reference of resources.db as well: two.) Nothing lets
  them go at quit, and the retail game never lets resources.db go either:
  `engine_world`'s destructor (vtable 0x9DB60C slot 0, 0x13C10 → 0x13C20 →
  0x15C80, the engine being the static object 0xA7B6B8) lets
  engine_world+2C0h go (the shaders' masks, 0xE0210 at zero), logs any
  leaked resource of class 0xC (0x102E30, a log line, no assert), stops its
  threads and ends in `0x1135E0("", 0)` → 0x1134E0 → 0x1148D0:
  `TerminateProcess(GetCurrentProcess(), 0)`. No code of the exe lets
  engine_world+2D4h..2DCh go: their only writes are the constructor's
  zeros (0x138DC..0x138E8) and the callbacks', +2D4h is read nowhere, and
  none of 0xDFE30's nine callers (0x14822, 0x148EB, 0x1493B: the three
  callbacks' release of the one there before; 0xDFF09, 0xE7F7D, 0xF8178,
  0xF832C, 0x10549C, 0x1058CC) reaches them. (The 0.10b source's
  `engine_world::finalize_resources` sets the three to NULL; 0.26g has no
  such code.) So the patches stay mounted until the process ends, as
  resources.db does, and no unmount runs at quit with or without them.
- **The order of the callbacks**: FIFO, so the k-th callback is the k-th
  mount's, and the order is what the patch relies on: the fatal names the
  k-th patch, and slot 0, which the last callback hands to the game and
  whose reference it lets go, is resources.db only if resources.db's
  callback came first. If the order differed, engine_world would hold a
  patch and the state resources.db (each mount would still be held once,
  nothing unmounted), and a fatal could name the wrong patch. The evidence
  [the review of #105, not re-read here]: the file system's tasks are put
  in order on one list (`intrusive_list::push_back` in `add_fs_task`
  0xF8480), run one after another on the one resources thread, each mount
  synchronous (`query_mount_and_wait`), and the finished tasks' callbacks
  are dispatched in order on the main thread.
- **Checked**: `selftest_026g.py` check 27, and
  `check_mount_patches_026g.py` (Unicorn 2.1.4, the image as is and
  rebased by 1 MiB with its relocations): the two hooks, the code and the
  game's own on_mounted_resources from 0x147E6 to its next step (0x14827)
  run on made-up folders and callbacks (each mount held by its caller while
  the callback runs): the mounts queued, the reference counts at the end
  (resources.db 1, each patch 1, nothing unmounted), a folder, access
  denied and a sharing violation stopping before any mount, path not found
  skipped, the first, a middle and the last patch failing to mount; the
  formatting, the file checks, the mount, the fatal, the unmount and the
  next step being stubs. **In a game**: the first writing (the build
  numbered 14, sha256 `770f7c04…5b09`) mounted patches 14, 15 and 16 made
  by our writer in Wine on 2026-10-16, and they worked; its quit was not
  watched, and the build with this section's changes has not been run.
  Not known from the code alone: whether every small archive mounts as the
  retail one does (the mounter reads the 256 signature bytes after the
  file table, section 0.1).

### 0.3 `.srpx`, the arena of the later patches (generic tooling, 2026-10-22; linked from source since 2026-10-23) — PROVEN bytes: the self-test's arena and link checks pass, check_research_026g.py's and check_artefacts_026g.py's output is main's, check_engine_026g.py passes; not run in a game

The exe's section table has room for twelve entries: it begins at file 0x210
and the headers end at 0x400, where the first section's bytes begin, so
0x400 - 0x210 = 0x1F0 = 12 x 40 bytes. The game's seven sections and ours,
`.srp`, `.srpd`, `.srpc` and `.srpw`, take eleven. The twelfth and last is
`.srpx`, which Research (patch 53, #297) made first: code, initialized data
and writable (0xE0000060), on the page after `.srpw`'s three pages
(`ARENA_VA` = 0xA42D000). It is the arena every later patch's code, data
and state goes in, so no patch needs a PE section of its own again (the
table is full).

**Each patch is plain C or NASM; the tooling links it.** A patch that wants
room declares its blocks in `patch_026g.py`'s `ARENA`: an
`ArenaBlock(name, kind, align, sources)` each (`kind` "code", "data" or
"state"; `align` a power of two from 16, 0x1000 for a page of its own;
`sources` its `.c` and `.asm` files). `link_026g.py` builds the arena from
them at every build (the self-test's, the release's, every check script's):

1. each source once to a relocatable ELF32 object: `nasm -f elf32` for
   `.asm`, `gcc -m32 -c` with Research's flags (`-Os -ffreestanding
   -fno-pic -fno-builtin -fno-stack-protector -msse2 -mfpmath=sse
   -mincoming-stack-boundary=2 -fno-asynchronous-unwind-tables ...
   -Werror`, Nix hardening off) for `.c`;
2. a linker script made from `ARENA`: one output section per block, in
   `ARENA`'s order, each at the next multiple of its alignment, holding of
   each of its sources (in the order given) for a code block their `.text`
   then their `.rodata`, for a data block their `.data`, for a state block
   their `.bss` (zeros in the file, as every state was);
3. GNU ld links all the objects together at `ARENA_VA`; each block's VA and
   size are what the link gives it, and the file holds the whole arena (a
   code block's gap to the next block int3, the rest zeros). Nothing is
   reserved and no VA is written down: the exe is made whole for every
   release and never rebased, and nothing outside it holds an arena
   address.

A patch reaches another's code by name: ordinary ELF symbols (the engine
library's `engine_...`, 0.5; a C function the NASM part calls, `extern
rn_register`). The game's functions and data stay absolute addresses in the
sources. A hook site in the game takes its target from the link by symbol:
`RESEARCH_HOOKS` and `ARTEFACTS_HOOKS` are (site, original bytes, symbol,
opcode), and `arena_target` gives the symbol's VA (a global of a code block;
else the build stops). `INSTALLED_VTABLES` names the block whose code holds
each vtable, and `audit_vtables` reads every `*_vtable` symbol of the link,
locals too (0.4).

**Refused, loudly** (the build stops and says why): a missing tool (CC, LD,
NASM or NASM_DIR name others than `cc`, `ld` and `nasm`); NASM other than
2.16; a section with bytes no block of the source takes (a C file's `.data`
in a code block only); a relocation other than R_386_32 and R_386_PC32 (GOT,
PLT, 16- or 8-bit addresses: the code is not position-independent and the
exe is never rebased); a symbol no source defines or one defined twice (ld);
a block with nothing in it; an arena past `ARENA_LIMIT` (0x100000, a
deliberate cap against a runaway state array).

**NASM 2.16, not 3.** NASM 3.01 (nixpkgs' now) assembles `jmp near label`
to a label as a two-byte short jump (`EB`): the five-byte entry tables of
`research_026g.asm` and `artefacts_026g.asm` shrink and every entry after
the first moves. `link_026g.py` refuses any NASM but 2.16;
`nix develop ./srp-026g` has NASM 2.16.03 (built from its release tarball,
`flake.nix`), Python 3, the shell's gcc 15.2.0 and GNU ld 2.46; the lock was
written there. That gcc 15.2.0 also rebuilds main's last prebuilt Research
`.bin` byte for byte (its `.json` named gcc 15.3.0, which gives the same
bytes). `release/build-zip` runs `make_client_026g.py` and `patch_026g.py`
in that shell (`client_patches_python`, release/lib.sh), and
`release/test.sh` runs the same invocation for real with nothing but nix on
PATH.

**The lock, `arena_026g.lock.json`.** No prebuilt arena code is checked in;
the lock is: the linked arena's VA, size and SHA-256, every block's kind, VA
and size, every global symbol's VA (the map), and the tools that wrote it.
`python link_026g.py --write` writes it after a change of an arena source
(its diff in the PR shows what moved); `--check` compares. `arena()`, which
`build()` and every check script call, links and compares the bytes, blocks
and symbols with the lock, and stops when they differ: an unexpected change
(another compiler or NASM, a source edited without `--write`) cannot ship.
The self-test's made-up arenas are not compared (`build(...,
arena_blocks=...)`).

```
section table (0x210..0x400): 12 slots
  .text .rdata .data STLPORT_ .tls .rsrc .reloc   (the game's 7)
  .srp .srpd .srpc .srpw                           (ours, 8..11)
  .srpx                                            (the arena, 12)

.srpx (VA 0xA42D000), its first blocks (the lock records every block):
  research.code    code  0x1000  research_026g.asm + research_arena   -> 0xa42d000, 0x3874
  research.state   state 0x1000  research_arena STATE                 -> 0xa431000, 0x44
  artefacts.code   code  0x1000  artefacts_026g.asm                   -> 0xa432000, 0x440
  engine.code      code  0x1000  engine_026g.asm (0.5)                -> 0xa433000, 0x5dd
  match_keys.code  code  0x1000  match_keys_026g.asm (patch 58)       -> 0xa434000, 0x168
  match_keys.state state 0x1000  its option's variable, kit_keys      -> 0xa435000, 0x50
  (a new block: wherever its row goes; the link places it and every block after it)
```

**What changed with the link (2026-10-23).** Before it each block's bytes
were built for a VA written down by hand (`ARENA_PINNED`; NASM with
`-DCAVE`, Research's C linked by its own script at a fixed address with its
`.bin` and `.json` checked in, the engine library's entries at
`ENGINE_PINNED` VAs that Research's `.bin` called directly), so two PRs adding
a block collided and the second had to move and rebuild by hand. Now:
`ARENA_PINNED`, `check_arena`, `arena_layout`, `ARENA_SYMBOLS`,
`research_026g.py` and the checked-in `research_026g.section.bin` / `.json`,
`artefacts_026g.section.bin` and `engine_026g.section.bin` are gone;
Research's state is `rn_state` (its `.bss`) instead of `-DRN_STATE_VA`;
the asm entries are global symbols (the tables stay, so Research's and the
artefacts' bytes and their checks' output are unchanged); the reserved
`anomalies.code` page is dropped, so the library moved from 0xA433000 to
0xA432000. Research's C differs in one place: gcc unrolls a three-step copy
into the state that it looped over a constant address before (`rn_hud`);
check_research_026g.py prints exactly what it printed on main.

**Adding a block (a PR).** One `ARENA` row and its source file(s):

1. write the code as plain NASM (`bits 32`, `section .text`, `global` the
   labels a hook or another patch uses, `extern` what it uses of others,
   `%include "engine_026g.inc"` for the library) or C (`#include
   "engine_026g.h"`; state as ordinary zeroed globals);
2. add `ArenaBlock("<patch>.code", "code", <align>, ("<file>.asm",))` (and a
   `"<patch>.state"` block naming the same sources if it keeps state);
3. hook the game with rows of (site, original bytes, symbol, opcode), made in
   `build()` with `arena_target` as Research's and the artefacts' are; declare
   each in `INSTALLS` (and any vtable or callback in `INSTALLED_VTABLES` /
   `CALLBACKS`, 0.4), or `build()` refuses it;
4. `python link_026g.py --write`, commit the lock with the sources.

Two PRs adding blocks no longer collide on addresses; they meet only in the
lock (whichever merges second runs `--write` again on top of the first).

**The dynamic anomalies (#225), taken out again (2026-10-23).** #225's
block `anomalies.code` (`dynamic_anomalies_026g.asm`, 0x337 bytes, after
`engine.code`) was the first block added this way. The owner took patch 42
out of revision 31: its row, its two hooks and
INSTALLS rows and check 42 are gone, and `link_026g.py --write` moved
`match_keys.code` and `match_keys.state` down one page (0xA434000 and
0xA435000 before); no other block moved. The code stays on the branch
`feat/dynamic-anomalies`.

**The older fixed sections, later.** `.srp` (the upload code and the small
caves at fixed offsets: `SPOT_OFFSET`, `KIT_CHANGE_OFFSET`, ...), `.srpd`
(their state, at fixed offsets of one page), `.srpc` and `.srpw` (each
`.bin` assembled with `-DCAVE`/`-DSTATE`/`-DPREVIEW`/... naming other
patches' VAs) are out of this change. Moving one onto the link: its `.asm`
made relocatable (no `org`, `-D` addresses of other patches replaced by
`extern` symbols, its state a `.bss` in a state block), one `ARENA` row, its
hooks as symbol rows, its `*_bin()` / `*_OFFSET` / `*_VA` constants and the
self-test's "is current" and `-DCAVE` checks replaced by the link's symbols,
and its check script reading VAs from `arena()`. The late join's C
(`late_join_026g.py`, its checked-in `.bin`) would become sources of a block
the same way Research's did. Each move changes the exe's layout (the
sections shrink or go), so one patch at a time, each with its own Unicorn
check run before and after.

### 0.4 Call shapes: every installed function has a model (generic tooling, 2026-10-22) — PROVEN bytes, the self-test's shape checks pass

A function the game calls through a pointer our code hands it (a vtable slot,
a delegate, a callback) must pop off the stack exactly what the game's own
function in its place pops (its `ret imm16`), or the game's caller is left
with the stack off by the difference. Research's rule slot 16
(`rn_deserialize`) popped 4 bytes where the game's rules end `ret 8`, from a
wrong hand-written note, and the game died at the first sync (#297; fixed
there, and check 53 compared Research's own vtables with tdm's). This makes
that comparison the rule for every patch.

Every function a patch installs declares, in `patch_026g.py`, the game's own
function it stands in for (its **model**) or an explicit ABI:

- `INSTALLS` names each site where the game calls or enters our code, by the
  shape the site had in the original: **SLOT** (a pointer the game calls
  through: the model is the function the original's pointer named), **ENTRY**
  (a function's first bytes jump to ours, or its body is ours in place: the
  model is that function), **CALLEE** (`[push ...] call F` calls ours: the
  model is F), **INLINE** (a call of ours where the site was other
  instructions), **ABI** (pops what the declaration says, with the reason),
  **JUMP** (a jump into ours from the middle of the game's code).
- `CALLBACKS` names the functions our code hands the game at run time (no
  write of `build()` shows them), with the instruction that hands each over
  and its model.
- `INSTALLED_VTABLES` names each vtable our code builds and installs (a
  rule's, a cook's): the `.section.json` symbol of its locator, the model
  vtable (the game's own in the same role) and how many slots are ours.
  Today: Research's rule (tdm_rule_core, 0xA15DA4, 9 of 20 slots ours), its
  cook (tdm_rule_cook, 0x9FD8AC, 2 of 11) and its rule's link resolver (the
  game's own function only).

Each slot of ours and each callback also names a case of its patch's Unicorn
check that reaches it **through the game's own caller** (the last field of
`INSTALLED_VTABLES`' rows and `CALLBACKS`'): the `ret` comparison cannot see
the order of the arguments, and Research's slot 7 took `(type, time, data)`
where the game's dispatcher 0x2D1EA0 passes `(type, data, time)` (#297's
review, §3.50). The self-test fails a slot of ours or a callback without such
a case, or with a case its script does not have; a negative test shows both.
Research's cases: slot 7 through the dispatcher 0x2D1EA0 (with the first
build's order put back by a shim, the case fails), 8 and 9 through the
world's entered/left loops (0x2D2010, 0x2D2130), 11 and 12 through the
world's set-core and blackboard loops (0x2D3B40, 0x2D5906), 15 and 16 through
game_world_core's serialize and deserialize loops (0x2D2661, 0x2D2B42), 17
through the hud's loop (0x2D36D0), 18 through the completed loop (0x2D2730),
the cook's 9 and 10 through translate's calls (0x2F42B8..0x2F42CF), the tick
through the tick list's runner 0x2D1D20; the late join's query callback
through the game's boost invoker 0x202300, weapon_camo's bake through
0x127700.

The checks:

1. `build()` fails on any call (E8), jump (E9) or other 4 bytes naming our
   code (a pointer, a `push imm32`, a `mov imm32`), at any offset of any write
   into the game's sections (`installs_of`), that `INSTALLS` does not declare,
   or whose kind does not fit the site (`audit_installs`); and on any
   `*_vtable` symbol of the arena's link (`arena_symbols`, locals too) that
   `INSTALLED_VTABLES` does not declare (`audit_vtables`). A new installed
   function or vtable cannot ship undeclared.
2. The self-test (`test_call_shapes`) reads what each of ours pops from the
   built bytes, every `ret` reachable from its entry with its branches
   followed (a body the compiler split, a tail jump into the game:
   `function_pops`; an instruction cap reached is a failure, not a pass), and
   the model's the same way from the original exe; where
   `tools/re-kit/v026g/abi.tsv` has the model's row its `ret_cleanup` must say
   the same. It fails on a mismatch for:
   - every SLOT (model: the function the original's pointer named);
   - every ENTRY (model: the function at the site; a body in place too);
   - every CALLEE (model: the replaced call's target, less the bytes of the
     pushes replaced with it: `kit_view` 4 - 4 = 0, `detach_first` 8 - 8 = 0);
   - every ABI (its declared pops);
   - every CALLBACK;
   - every slot of ours of every INSTALLED_VTABLES vtable (check 53's
     comparison, generalised).
   Today that is 13 ENTRY (three of them the artefacts', patch 54), 18 CALLEE, 1 ABI and 1 SLOT site, 2 callbacks and
   Research's 11 own vtable slots. Over the variants it builds it also flags
   an `INSTALLS` row that none installs (a stale row).
   `test_call_shapes_negative` builds a made-up rule vtable whose slot 16
   pops 4 where tdm_rule_core's pops 8 and checks it is rejected (and the same
   slot at 8 passes), and refuses an undeclared vtable, an undeclared install
   and a `push imm32` of our code's address inside a write.

On main the installed pointers are the replay camera's "is this place mine"
slot (model `is_this_place_mine`, `ret 4`), the callbacks `query_completed`
(model `on_match_ready`, `ret 4`) and `bake_done` (model `on_bake_finished`,
`ret 0`), and Research's three vtables above (its rule slot 16 now `ret 8`).

**Not covered.** INLINE sites (a call of ours over instructions that were not
a call) and JUMP sites (a jump into ours from the middle of the game's code)
are declared but not `ret`-checked: their stack balance at the return point
stays covered by each patch's own Unicorn check and by the relocation and
no-branch-into-a-hook checks. Nor are the in-place rewrites of the game's code
(STAGE_FOCUS, the scoreboard's loop), which install nothing. Not yet either:
that every slot of ours is reached through the game's own call site with its
arguments in their order (the Research follow-up).

### 0.5 The engine-function library: calling the game's functions (generic tooling, 2026-10-23; by symbol since the arena's link, 0.3) — PROVEN bytes and thunks: check_engine_026g.py passes on every function and rejects the wrong rows; Research's checks pass on the migrated code; not run in a game

The game's compiler (LTO) moved arguments of many functions into arbitrary
registers (the config's child 0xF6B10: EAX, EDI; its has 0xF6C00: EAX, ESI;
the pool lookup 0x2E58B0: EAX, ECX), mixed registers and stack
(increase_health 0x32A420: ESI and two slots; the field writers 0x1F3DE0,
0x2F5DA0, 0x1F48A0: EAX and five slots), and made either side clean up
(0x1F3DE0 pops 20, the decal decoder 0x263D00 nothing). Each patch wrote its
own inline-asm stub per call, and Research crashed twice from exactly that
(a wrong argument count, a wrong order). The library makes each such
function an ordinary cdecl one; a patch calls that and never sees the ABI.

**Where the ABI comes from.** `tools/re-kit/v026g/abi_inputs.tsv` (each
argument's register or stack slot) and `abi.tsv` (the cleanup, the result),
every row of the VA together. `engine_026g.tsv` (in `client-patches/`)
lists the library's functions, one row each, in any order (each is a global
symbol the link resolves for every caller): the C name, the VA, where each C
parameter goes with a word of the re-kit's role there (`EAX:table
EDI:key`), the C prototype. (Until the arena was linked, 0.3, each function
was a `jmp near` entry at a VA pinned in `ENGINE_PINNED`, rows out of that
order refused, because Research's prebuilt `.bin` called the entries by
address. With calls by symbol a row's place moves nothing that is not
linked again, so the entries, the pin and the order check are gone.) `engine_026g.py` refuses a row
whose locations are not exactly the re-kit's for the VA, a parameter whose
word is not in a role of its location (and no two parameters may have words
that fit each other's locations, so a swap is always refused), stack slots
with a gap, a cleanup that is no number or not one value of at most the
slots' bytes, or a C return type that does not fit the rows' result (EAX; AL,
widened; EDX:EAX for 64 bits; XMM0, moved to ST0 for a float). An XMM or
x87 argument, varargs (log_message 0x111ED0, buffer_string::appendf
0xE45B0) and tail jumps stay hand-written.

**Virtual functions** (2026-10-29, patch 81's swap). A call through an
object's vtable whose class varies (the game effect emitter's emit,
`mov eax,[ecx] / call [eax+20h]`: nine emitter classes) has no VA. Its row
says `call` virtual and `va` `<class>+<slot>`
(`survarium::pure_game_effect_emitter+0x20`); its implementations are the
re-kit's `vtables.tsv` rows (vtable, slot, va, class, base, basis: the
function a class's vtable holds at a slot, and the class whose virtual it
is) of that slot with that class or base. Each implementation's re-kit rows
are checked against the row as a direct call's are, and must be the same
for all and thiscall (`this` ECX, the stack slots popped: an implementation
popping otherwise is refused). The thunk is a thunk's (cdecl in, the
callee-saved kept) with `mov eax,[ecx] / call [eax+slot]` in place of the
`call <VA>`. check_engine_026g.py: every vtables.tsv row is the exe's (the
vtable holds that function at that slot), each implementation's `ret`s pop
the cleanup, the thunk reaches a made-up vtable's slot with `this` in ECX
and each argument where the rows put it, and with an object of each listed
vtable, the exe's own, reaches that vtable's function. Rejected: the emit
row at slot +1Ch (destroy's, one stack argument), a class without
vtables.tsv rows, one emit popping 8 in abi.tsv, a vtables.tsv row naming
the sound emitter's emit in the hud emitter's slot. Not caught: a class of
the family that vtables.tsv does not list (no RTTI in the exe; the nine
were found by their shared slots and their cooks).

**What it generates.** `engine_026g.asm`, the arena's block `engine.code`: a
thunk per function, each the global symbol of its C name:

```
engine_x:   push ebp / ebx / esi / edi      ; cdecl's callee-saved: kept
            push dword [esp+..]             ; the stack slots, highest first
            mov  <reg>, [esp+..]            ; the register arguments
            call <the real VA>
            add  esp, <slots - cleanup>     ; what the real one leaves
            [movzx eax,al | fld the XMM0 float]
            pop  edi / esi / ebx / ebp
            ret                             ; cdecl: the caller pops its own
```

`engine_026g.h` declares each function in C (gcc emits `call rel32` with a
relocation the link fills); `engine_026g.inc` gives NASM an `extern` for
each. The comments name the real function and its shape.

**Why one block, not a copy in each patch.** The thunks are code shared by
every patch, so they are one arena block, `engine.code` (0xA432000 as the
link places it today), built once and checked once: the self-test runs the
ABI check on exactly the bytes every patch calls (the block as linked), and
a C patch only includes the header, an asm patch only `%include`s the
externs. A copy in each patch's own block would need the same thunks
checked per patch.

**The check** (`check_engine_026g.py`; the self-test's `test_engine`, which
fails without Unicorn): the original exe's sections and the library's bytes
in Unicorn. Per function:
- the exe's `ret`s (selftest's `function_pops`, the call-shape check's
  reader, 0.4) pop what abi.tsv says;
- the thunk, called as C calls it with a distinct marker per argument,
  reaches the real VA once, with each marker in the register or slot the rows
  give it and its return address in the library; there the return is played
  with the exe's own cleanup, EBX ESI EDI EBP clobbered and result markers in
  EAX, EDX and XMM0; back at the caller esp is the call's + 4, the four
  registers are the caller's and the result is where C takes it;
- the functions that run on made-up data run for real: the config's child
  and has on a mode config, the three field writers on a writer (the value's
  bytes appended), the statistics' listener (its invoker gets the functor,
  place, event and amount), the pool lookup on two pools, the allocation's
  name. A row with two arguments swapped makes a thunk the entry check agrees
  with but the real function does not.
Negative cases, each of which must be rejected: spec rows with two
parameters in each other's places (the config child's table and key,
malloc's allocator and size, the rule base's rule and type, the message's
second and third parameters, Invoke's name and result, free's allocator and
memory, profile assign's destination and source: the generator refuses
each), a cleanup "0x10"; libraries made from wrong re-kit rows
(0xF6B10's table and key registers exchanged, the spec following them: the
real lookup finds nothing; w<unsigned char> popping 16: the exe pops 20;
value_exists' key in EDI: the real one reads ESI); and the built library
checked against rows with malloc's allocator and size exchanged. The
self-test also fails when the three files are not what the rows give
(`--write` not run), and runs the same check on the library linked at
another VA (the arena with two blocks swapped, 0.3).

What is caught and what is not. Caught by the self-test: a spec parameter in
another's place (the generator), a wrong cleanup in
abi.tsv (the exe's `ret`s), a thunk that does not follow its rows (the entry
check), and a wrong re-kit location for the functions run for real (the
config's child and has, the three writers, the statistics' listener, the
pool lookup, the allocation's name). Not caught by it: a re-kit row whose
location is itself wrong, role and all, for any other function: the thunk
and the check both follow the row. For Research's other functions (malloc,
the rule base's constructor, show_parametrized_message, Invoke,
register_cook) check_research_026g.py runs the real ones or reads their
arguments where the game does; for the asm patches' functions, their own
Unicorn checks, once they call the library.

**The first users.** The library has 33 functions: the 11 Research calls
and the 22 the artefacts, weapon camouflage, weapon decals and upgrade
preview patches call directly (weapon_table calls none: its one game
transfer is the tail jump to set_target 0x2845C0, XMM arguments). Research's
C calls the library for all 11 (its inline-asm stubs and thiscall casts are
gone); its built code changed, its behaviour did not: check_research_026g.py
passes every case as before (the hooks, the cook and rule through the game's
own register_cook, constructor, config look-ups and field writers, the
slots driven through the game's own callers, the invoke and the messages
with their arguments where the game reads them). The rows this needed were
appended: show_parametrized_message 0x2375C0's three message parameters,
malloc_impl 0xE4F90's result, and the pool lookup 0x2E58B0, the artefact
effect 0x313350, the allocation's name 0x420789 and query_resources 0xE5B60
in full. The asm patches still call by hand; moving them is a follow-up
(README, the library's section).

### Medical consumable activity (patch 77)

`consumable_activity` is a Rust presentation hook before the native quick-slot
update (0x23B7A0). `vostok::QuickSlotActivity` reads the live local player's
mapped item. Only the medkit primary vtable permits reading medical state.
Inactive/dead, waiting for the native use delay, and active are distinct states;
waiting and active each carry their remaining milliseconds and skill-modified
duration. Waiting uses modifier 33 with weight 1, active uses modifier 34 with
the medicine's skill weight, matching native set_active.
No inventory, simulation timer, or snapshot is modified.

The HUD receives `set_quick_slot_activity(slot, phase, remaining, duration)`.
QuickSlots owns the combination of that snapshot with the native slot descriptor:
an exhausted stack retains its icon during its remaining delay/effect, and is
removed when the native effect ends. Waiting maps to the existing gray icon and
filled cooldown sector. Active displays only ceiling seconds in the native
countdown field; no custom outlined ring is added. Artefacts retain their
native countdown/cooldown behavior. No wall-clock timer runs in ActionScript.

Phase 3 (patch 84, `drug-interrupt.md`): a drug ended with time left
(set_active(0) leaves +2ACh/+2A8h; a spent one has both 0) that the hook saw
in use at its previous update. The hook keeps per quick slot only the last
seen state (idle, the drug in use by its address, or stopped with the local
player's simulation time +324h when first seen so) and sends 3 for 1000 ms of
that clock, then 0; never for a drug not seen in use (a previous life's, a
late join's), and in use again after a rollback that took the hit back.
QuickSlots keeps the exhausted stack for it and the renderer tints the slot's
own icon red (ColorTransform 1, 0.3, 0.3) where phase 1 dims it.

### Lobby weight explanation

`weight_ui_026g.c`/`.asm` live in the linked `weight_ui.code` and
`weight_ui.state` arena blocks. The jump at 0x22110F saves the machine state,
reads the current lobby's player stamina config, replays `mov eax,[esi+108h]`,
then continues at 0x221115. The call at 0x2222EB replaces only the lobby weight
GFx invocation: the original movie arguments supply current and modified maximum
weight; `engine_apply_modifier` (0x2D5AE0) on `profile + F4h + kit * 704h`
supplies modifiers 26, 31 and 32, matching the stamina calculation. The profile
equipment helper at 0x2F7660 cannot accept these indices: its unchecked item
switch crashes with equipped gear, now covered by the native regression check.
The callback gains movement/spending fractions and sprint state
(0 permitted by weight, 1 blocked by weight, 2 blocked by Heavyweight).
It also gives the kit's own movement-speed modifier (the raw float of index
4: the skills) and whether Bear Arms (modifier 62, additive) is on, for the
speed shown beside the weight (`data.md`, "The kit's movement speed").
And the carry limit's parts: the menu's base limit (+3118h), the kit's floats
of modifiers 5 (skills) and 26 (Packrat) (`data.md`, "The carry limit by
source"): ten arguments in all.
It retains the original two arguments if the config is unavailable or invalid.
The shim preserves registers, flags and floating-point state and returns with
the original `ret 16` convention. `check_weight_ui_026g.py` executes both hooks,
the original callback and kit evaluator; `data.md` records the formula evidence.
No gameplay calculation or release revision changes.

### Knowledge database link

The lobby's Knowledge database button keeps its confirmation dialog and native
`go_to_knowledge_base` callback (0x28CAB0), but opens
`https://026g.skhid.link/wiki`. `KNOWLEDGE_BASE` replaces the fixed URL prefix at
0xA0A7A8 and NOPs the two `strcat_s` calls at 0x28CBA3/0x28CBB7 that appended
the old support site's language and path. Their argument pushes and cdecl stack
cleanup remain paired. ShellExecuteA, `AL=1`, and `ret 4` stay unchanged. The wiki
uses browser language selection and its own language switch. No new code arena
is needed for these constant-data/in-place removals. `check_knowledge_base_026g.py`
executes the original and patched callback in every game language, capturing
the browser call and checking its URL, arguments, preserved ESI and stack.

### Weapon detail cards

`weapon-details-ui-1.json` copies each weapon's existing configuration values
into non-comparable UI properties 138–159; it changes no simulation parameters.
`WeaponDetails.as` reads those values for inventory and shop cards. The compact
card replaces the ambiguous distance row with the full-damage distance and the
minimum-damage distance/fraction, and adds base reload time. Holding Shift adds
draw/holster time, zoom, stamina damage, hip spread where specified, recoil,
spread/recovery and sway. Wear rates and reliability are deliberately excluded.
Weapon-local modifiers are included; profile gear and skills are excluded.

Recoil and spread are not labelled degrees: the native recoil loader at
0x331840 divides horizontal config values by 21.48 and vertical values by
63.735. The dispersion loader at 0x330EE0 copies growth and recovery rates
directly, as the breath loader at 0x2DEBC0 does for amplitudes and period.
The UI reports each weapon's own base recoil/spread/sway values, named in each
language's own recoil, spread and sway words (the stock dictionary's: Russian
отдача, разброс, раскачивание). Vertical (initial upward) recoil
shows `[min_top, max_top]`; horizontal (initial sideways) recoil shows
`[min_right - max_left, max_right - min_left]`, matching the engine's independent
right-minus-left draws. Variability is the standard deviation: upward range
width divided by `sqrt(12)`, sideways `sqrt((right_width² + left_width²) / 12)`.
These describe the initial shot impulse before accumulation and recovery.
Signed sideways ranges stay neutral; lower variability compares as better.
Upward ranges compare as better only when both endpoints are no higher and at
least one is lower, using displayed precision; crossing ranges stay neutral.
The values are relative configuration units, not degrees.
An early AK-74M = 100% benchmark was removed because it looked like a
weapon parameter and obscured the actual values.
Missing detail metadata retains the original distance row. Native card layout
owns resizing, and lobby positioning is refreshed when Shift changes its height.

Focused check: `scripts/weight-ui/check_ui.sh <lobby.swf> <output-dir>
scripts/weapon-details/WeaponDetailsHarness.as DETAILS` exercises native card
expansion/collapse, comparison colors, translated heights, base values and old
content fallback. In every language, compact and expanded, and beside the widest
values of the live weapons (MSBS: -0.58…0.58, 1.98…2.2), every label is one line
ending left of its value and every heading one line, with 5% of the width to
spare (Ruffle lays out the embedded Calibri close to, not exactly as, Scaleform);
each miss is printed as a `DETAILS OVERFLOW` line. A label that would not fit
wraps before its value rather than under it (`LabelDiffIndicator`), the row
taking the lines. The existing weight harness also checks the shop constructor.

Weapon detail rows use the complete `weapon_compare_mc_8` symbol through
`WeaponDetailsView`, including its `HintDiffIndicator`, embedded-font fields,
transforms and original row spacing from `ItemCompare`.
An initial dynamic `TextField` copied from the empty description rendered with
tiny black fallback text in Scaleform despite looking legible in Ruffle.
An intermediate fix still overrode the original gray/bold labels, gold numbers
and column geometry. `LabelDiffIndicator.setDetailText` now follows the original
formatter and only substitutes the read-only value string and hides bars (these
base fields have no comparison ranges). Native unit spacing is retained; long
notes wrap at the original columns. The harness compares font, size, boldness,
alignment, color, global text scale, columns and row spacing against the card's
original rows, as well as checking value and frame bounds in every language (the texts are
`st_srp_weapon_*` and `st_srp_unit_*` dictionary keys, localization/README.md).

Headings and the Shift hint are separate caption entries,
rendered with native embedded-font fields by `WeaponDetailCaption`; headings
have brighter, larger text on a near-black divider band, while the Shift hint is muted gray.
Only numeric entries use the stat-row formatter. Comparison cards reserve the
larger of each pair's wrapped label heights. Missing `from_the_hip_multiplier`
now displays 1x (including Kiparis), matching the loader's 0x330FE3 branch to the
1.0 constant at 0xA70884, rather than omitting the row and shifting every later
stat. The harness verifies matching row positions and caption contrast as well.
Compared weapon cards also reserve the larger natural header height, including
wrapped optics descriptions, so their original and added stats align. Returning
to a single card restores its natural header. The redundant "Weapon modifiers
only" note is omitted. Early comparison resize events wait for the three bonus
list containers as well as their data providers before laying out the card.

Each stat has a stable property id and an explicit comparison direction. Higher
damage/range and recovery rate are better; lower times, recoil, spread and sway
amplitudes are better. Zoom and sway period are neutral traits. Draw/holster,
damage-floor distance/amount and horizontal/vertical sway have separate rows so
one value cannot inherit another value's color. The native row formatter supplies
red/green; equal displayed values retain the original neutral color (comparison
scores use display precision, avoiding float-noise differences).
Holding Ctrl suppresses comparison colors and difference bars in both original
and added card rows, independently of Shift's expanded view. Releasing Ctrl
restores comparisons without changing values. The harness covers both key orders
and restores both original and added colors.

Audit of all 83 weapon configurations: none of these displayed parameters is
constant across every weapon. The minimum damage fraction is the closest: 60%
for 82 weapons, 80% for Winchester 1894. No varying stat was removed as a constant.

`WeaponDetails.read` applies only `item.modifiers` (already containing defaults),
never the profile, gloves or other equipped items. Property 24 scales both
falloff distances by `max(0, 1 + sum)`; property 114 scales weapon reload time by
`max(0, 1 - sum)`. The opposite reload sign is explicit in weapon modification
2040..2044 and defect 3040: the UI calls a reload improvement positive while its
config change to `parameters/reload_time` is negative. The native type-2 operator
at 0x358010 sums percentages, clamps `1 + sum` at zero, then multiplies the base.
Values are copied before modification and comparisons use the resulting values.

Existing data issue, deliberately not masked by UI: recoil modifications
2080..2084 and defect 3060 target `max_first_shoot_*` / `*_queue_shoot_*` fields,
absent from current weapons' recoil blocks. `find_modifier_target` 0x2F1430
returns null for missing paths; the cook skips them at 0x2F1ABF. Current recoil
loads `min_top_recoil`, `max_top_recoil` and the left/right ranges at 0x331840.
The card therefore does not invent a recoil reduction. Likewise a base-dispersion
upgrade (property 2) does not change shot spread, growth or recovery. The scope's
separate view FOV does not change the explicitly labelled base aim-zoom parameter.

### Artefact score (patch 80)

`artefact_score` (Rust, `vostok::ArtefactPickup`) is a lohook at 0x31C810 in
`artefact_container_core::use_execute`, after the collection time and before
`transfer_artefact` (`lea eax,[ebx+108h]`, 6 bytes; EBX the user, EDI the
container). It reads the user's artefact slots as `inventory::take_artefact`
0x2FC850 is about to search them ([inventory+168h] the first slot, byte
[+16Ch] how many; an empty slot holds 0): an empty one, Free; none, Full (the
game then replaces the first slot's artefact, 0x2FC897, as before). Free
counts event 16 for the user's place [player+114h] and kit [player+115h] in
the statistics of [[container+54h]+18FA8h] (+8 the shared ones, a place's
record 1F8h, events[28][3] u16): the cell + 1 with no bound, then
`notify_listener` 0x2F5D40 (place, 16, 1) when the listener (+31A0h) is set,
exactly as `on_artefact_taken` 0x2F55F0 counts event 5 after the transfer.
No statistics handler (0 before start_match and after finish_match), or a
place or kit that is no record's: nothing, as the game counts nothing then.
0.26g counts event 16 nowhere else (its finish_match 0x2F4CF0 counts 11, 12
and 27; no `inc` of a +60h..+64h cell in .text), so srp reuses it.

Simulation: the count is in the world's statistics, so every client of a match
must run it (a release's revision; the session refuses older replays). No
state is kept between hooks: a rollback re-runs the same pickup on the same
slots. `check_artefact_score_026g.py` runs the installed hook to the push after
the site on made-up objects (Free, Full, no slots, no listener, no handler,
places and kits out of range, the wrap), the game's own event 5 count on the
same statistics (the same indexing), and take_artefact's own search to its
two branches on the same layouts.

### Battery progress (patch 92)

`battery_progress` (Rust, `vostok::statistics`) pays a battery's carrier a point
(10 hundredths of event 6) for each 10 m the battery comes nearer the carrier's
team's container than it has been in its life (`battery_credit.md`). The game's
own measure (`victory_item_event_manager` 0x334810: the projection on the line
between the containers, `nearest_positions`, credited to `carried_distances` at
events only) resets a team's nearest whenever the other team touches the
battery, which two enemies passing it would farm, and the delivery shares
depend on that reset; so the patch keeps its own nearest, in the record's
unused +FEh/+FFh (a byte per team: 0 untouched in the life, else the steps + 1).

- `battery.carrier_tick` 0x2DAAD3 (`base_player::tick` after the held object's
  tick, `mov ecx,[esp+10h] / push edi`, 5 bytes): EBX the player. Its held
  item [+128h] must be one of the rule's items ([manager+0A38h]+110h..+114h,
  index i, record +20h + i x 100h); its team is the options' ([manager+0A3Ch]
  + 45FCh x place + 45ECh), its place and kit [+114h]/[+115h]. Both teams'
  bands (the 3D distance from the transform's position [+40Ch+30h] to
  [manager+0A20h] and [+0A2Ch], over 10 m, at most 254) become the nearer of
  the kept and the current; the steps the carrier's own team's band came
  nearer are counted in its cell and told to the listener as patch 80 counts.
  The first carry of a life sets both and pays nothing.
- `battery.match_start` 0x334999 (start_match once the containers' positions
  are kept, `mov eax,[esi+0A38h]`, 6 bytes; ESI the manager): the ten
  records untouched. `battery.delivered` 0x334CEA (on_event's put-in after the
  shares and the record's reset, `mov [ebx+0F8h],eax`, 6 bytes; EBX the
  record): the record untouched.
- `battery.save` 0x334F66 (serialize after a record's first_finder, 10 bytes;
  ESI the record, EDI the writer) writes the two bytes with w<u8> 0x1F3DE0;
  `battery.load` 0x3350D0 (deserialize after first_finder, 6 bytes; ESI the
  record, EDX the reader) reads them within the reader's size, untouched
  without them. The server's decoder (`VictoryItemStatus::progress`) reads them.

Simulation: the count and the bytes are in the world's statistics, so every
client of a match must run it. `check_battery_progress_026g.py` runs the five
hooks in the patched exe on made-up objects, and the game's own serialize and
deserialize of two records with the hooks inside.

### A bullet stops a drug (patch 84)

`drug_interrupt` (Rust, `vostok::drug`) is a lohook at 0x2D9CB1 in
`base_player::hit_impl` (0x2D9B90): `mov edi,esi / cmp dword [edi+40h],0` (6
bytes, replayed; the je at 0x2D9CB7 takes their flags), right after
`damage_model::hit_body_part` (0x2E5080) returned 1. It returns 1 when the
hit's damage, (1 - invulnerability) x hit+34h through the part's protectors
and armour (`body_part_parameters::hit_by_type` 0x329FF0), is above 0.0;
hit_impl leaves at once otherwise (no slow, no listeners, no death). EBX the
victim, ESI the `hit_info` (built by `hit_info::hit_info` 0x32B9F0: +1Ch the
hit type, +3Ch the time, +20h the damage source, +40h the weapon, +44h the
attacker's place). A dead, dying or fully invulnerable victim never gets
there (0x2D9BA3, 0x2D9BB0, 0x2D9BCB).

The hit type is `hit_type_enum`, the index of its name in 0x9257E0
(`survarium::hit_type` 0x304370, 0.21d 0x1E8DA3: ranged, blunt, slash,
explosive, toxic, radiation, fire, shock). A bullet's is its ammo data's +4Ch
(`ammo_data::load` 0x332440: `damage_type`, 0 when absent; try_pierce passes
`[[bullet+40h]+4Ch]` to `base_player::hit` 0x2D9A60): every 0.26g round has
none, so ranged; a weapon's melee data (weapon+53Ch) says blunt. The suicide
key's brain hit is ranged too (0x2D8C44). A bullet never hits its
`m_ignorable_object` (+50h, a player; 0x32D9DD).

For a ranged hit, each of the victim's 23 slots ([player+10Ch]+10Ch) holding
a drug (vtable 0xA1C79C, the game's medkit class, whatever folder its options
file is in) in use (+2B0h, set
from the press through the use time and the active time) whose
data/influences count (+288h) is not 0 gets `medkit::set_active(0, drug,
[hit+3Ch])` (0x33D390: AL, then medkit and time on the stack, `ret 8`; the
library's `engine_medkit_set_active`). That is the game's own end of a drug
(medkit::tick 0x33D7C1 for a dead holder, 0x33DA3C when the activity is
spent): it clears +2B0h; unlinks the drug_activity node +2B4h from the kit's
modifiers only when +2ACh is 0 and +2A8h is below the duration (the active
time began, 0x33D52B..0x33D583); tells the holder's listeners (0x2E5950)
and the match statistics handler ([world+18FA8h] vtable+20h, as at the press); zeroes the healed-so-far floats
(+284h); unlinks the tick node +268h from the world's list 6 (0x2D2DB0), so
no tick heals again; and unlinks its damage protectors (+294h, count +298h).
+2A8h and +2ACh stay as they were; nothing reads them while +2B0h is 0
(action 0x33D700 sets them again; patch 77 reads the flag first). The unit
was taken at the press (0x33D765). The painkiller (no influences, only a
ranged pain protection) is left running.

Simulation: hit_impl runs in every game for every hit, and the drug's state
is the game's own (serialized: `m_active`, `m_activity_time_ms`, `m_delay_ms`,
`m_applied_influence`), so a rollback, a late joiner and a replay agree; the
patch keeps no state. With 256 bullets or more, bullet_manager::tick
(0x321D10) runs bullets as tasks on several threads (0x3226F0), hits and all:
the stops take a `vostok::Lock`, since every drug shares the world's list 6,
and each reads +2B0h again under it (a second set_active(0) would unlink the
nodes again: the kit's drug_activity list emptied, list 6 with byte 0FFh).
`check_drug_interrupt_026g.py` runs the installed hook to the je on made-up
objects (set_active stubbed: each healing drug in use stopped in slot order,
in its use time and its active time; not a drug out of use, the painkiller,
another item, any other or unnamed hit type, no inventory; the replay's EDI
and ZF, every other register, the stack and the objects unchanged), and the
game's own set_active(0) in the original exe in the use time, the active time
and between them; and two bullet tasks on one holder, the second stopped by
the first between its look and the lock (patched exe, the game's set_active):
set_active once, both lists as the first stop left them.

The hit player's quick slot of the stopped drug shows red for a second of game
time (display only): patch 77's `consumable_activity` tells a stopped drug
from a spent one by the timers set_active(0) leaves, and sends phase 3
(section "Medical consumable activity (patch 77)").

### Names on the Tab map (patch 82)

`map_names` (Rust, `vostok::GameWorldKeys`) is a lohook at 0x25CD1C in
`game_world::on_keyboard_action` (0x25CD00), right after it took the input
world's keyboard (its vtable+24h, 0x79E40: `[input_world+24h]`, the platform
keyboard `create_devices` 0x3B82F0 made) and before it asks it for Tab
(`mov edx,[eax] / mov ecx,eax / mov eax,[edx]`, 6 bytes, then `push 0Fh`;
EAX the keyboard, EDI the game world's input handler, the hud at +4, as
`lea esi,[edi+4]` at 0x25CD30 gives show_player_list). The game world's
handler is given every key before the player's (priority 7 to 10, section
3.53), a held key every tick (the keyboard's process 0x3B9AF0, kind 3), so
each change of an Alt key comes with a key of its own, and while Tab is held
the hook runs every tick. It reads the keys' state only when the keyboard's
vtable is the platform keyboard's (0xA25478; its ctor 0x3B8BDC writes it):
`[keyboard + 4 + 4 x DIK]`, bit 80h, as `is_key_down` 0x3B8D00 reads it; left
Alt 38h (the game's names key, `render_name` 0x1F2DCD) or right Alt B8h
(`alt_is_held` 0x241200 asks both). The hud movie
(`[[[handler+4]+8]+108h]+4`) is told `root.show_map_names(bool)` when the
state or the movie changes, again at the next key while the Invoke answers 0.

The names: `update_minimap_players` (0x239420) gives each icon `player_id` =
the place of its loop (0..13h, 0x2395AC), and `initialize_match` (0x234880)
gives the Tab list's rows `id` = the same place (0x234FE4) and `name` (the
profile's name, `[%s] %s` with its clan tag, 0x235090). The movie keeps the
rows' names by place (`HUD.list_set_players` -> `Minimap.setPlayerNames`, the
Tab map's alone) and an icon shows its place's while the names are shown,
none for the local player (`is_local_player`). The Tab map is the Tab list's
(`SrvPlayerList.minimap`), which Blackout's forced Tab never shows, so no
name shows in Blackout.

Checked: `check_map_names_026g.py` (Unicorn) runs on_keyboard_action from its
start to past show_player_list (0x25CD39) in both builds on a made-up input
world (the game's own getter) and keyboard (the platform vtable, the game's
own is_key_down asked for Tab): the registers, stack, frame and Tab byte the
original's; the Invokes at the first key and each change (left, right, both,
state bits other than 80h), a refusing movie told again until it takes it,
another movie told at once, none without a movie or with another keyboard's
vtable. `tools/actionscript/tests/TabMapNamesHarness.as` (Ruffle) the movie's
part. Not run: a game.

### Tab list cursor and friend requests (patch 86)

Written 2026-10-30; the rules are in `tab-cursor-friends.md`. PROVEN in the code, and both sites run
with Unicorn from the game's code (`check_tab_cursor_026g.py`). Not run in a game.

- **The keys.** `game_world::on_keyboard_action` (0x25CD00, the game world's input handler in ECX,
  priority 7, before the player's handler at 10) gets every key. It asks the keyboard for Tab at
  every key and gives the answer to `show_player_list` (0x2384C0, hud +0BAh). While Tab is down it
  then returns AL 0 at 0x25CF0D, so the player's handler still gets every key: Ctrl (crouch, hold,
  DIK 1Dh/9Dh) crouches with Tab held. The patch's lohook (`tab_cursor.key`) is on the first nine
  bytes, with no branch: `push ebp`, `mov ebp,esp`, `and esp,-8`, `sub esp,0Ch`. The arguments
  are at the site's ESP+8 (the key) and +0Ch (its kind). Its exit `taken` is the function's own
  `ret 0Ch` (0x25CF15), with AL set to 1. The list after a key is the key's own up or down for
  Tab, and hud +0BAh for any other key: on_keyboard_action asks at every key, so +0BAh is the key
  before this one.
- **The cursor.** `game_world_ui::show_cursor` (0x23A930: EDI the hud, a byte on the stack,
  `ret 4`) writes hud +2DCh. The game world's `on_mouse_move` (0x25D0E0, `setnz al` of handler
  +2E0h = hud +2DCh) then takes every move, so the player's look gets none. `set_pointer`
  (0x25D160, gw +4D8h) moves the hud movies' pointer. `on_mouse_key_action` (0x25CF60) gives the
  buttons to the hud's movies (0x2AF610, a GFx mouse event: 151h is button 0, 152h is 1, the
  right) and takes them. The kits show and hide the same cursor (`show_profiles_selector`
  0x2383B0, hud +2DFh), so the patch does not hide it while the kits are shown.
- **The movie's calls.** `game_world_ui::scaleform_callback` (0x233D30: ECX the hud, then the
  movie, the method's name, the GFx arguments and their count; `ret 10h`) compares the name
  against the hud's callbacks. The lohook (`hud.callback`) is on its first six bytes: `push ebx`,
  `push esi`, `mov esi,[esp+10h]`. Its exit `done` is the `ret 10h` at 0x233D73, which leaves ESP
  as it was at the entry.
- **The friend request.** The lobby's `add_friend_name` (0x28C150) gets the network client's
  messaging client (vtable 0x9FFB14 +44h, 0x21BD80: network client +1E58A4h, which is the lobby
  client +1E0h, then +1E56C4h). It checks the lobby connection (`[lobby client +1E55C8h]`, its
  connection's state at +0B01F4h is 0 or 4), then calls
  `messaging_client::send_important_message` (0x26AFB0). That function takes the messaging client,
  the target and the type byte on the stack, takes the text in EDX, and pops 0Ch. The patch does
  the same. The game's network client is reached from the hud: +18h the world, then +9Ch the game,
  then +53ACh the network client.
- **The places.** The match's options are `[network client +1F1C48h]+0Ch` (0x219990). The places
  start there, 45FCh bytes each: initialize_match 0x234EEF. A place's name is at +8 (64 bytes)
  and its clan at +48h. The count is at +577B5h. A row's `id` is its place, and a row's name is
  `[clan] name`. The friend list is the messaging client's vector at +0Ch (its begin) and +10h
  (its end), 5Ch bytes per friend with the name at +8. A3 07's reader (0x26C540) fills it, and
  0x26C4FF reads it.

### World check (patch 87)

Written 2026-10-31; the rules are in `world_frame_check.md`. PROVEN in the code, and both sites run
with Unicorn from the game's code (`check_world_check_026g.py`). Not run in a game.

- **The loads.** `process_dynamic_match_info` (0x27BFF0: EDI the network client, ESI the packet's
  reader `{begin, cursor, size}`) stores the generation (0x27C040), reads the flag, and loads the
  world with `game_world_core::deserialize` (0x2D0B70; the core `[EDI+1F1C40h]+1F8h`, the reader,
  1, 8; `ret 10h`) at two calls.
- **Unpacked** (the flag not 0): 0x8000 bytes of the stack (0x27C05E), a mutable buffer at EBP-24h
  `{allocator 0, begin, capacity 8000h, size}` (0x27C06D..0x27C084), the codec of `[EDI+4Ch]`
  (vtable +4) into it, `mutable_buffer::resize` (0x7AB30) to its size, the packet's reader read to
  its end (0x27C0B7), the four arguments pushed, a reader at EBP-14h. The lohook
  `world_load.unpacked` is on the nine bytes 0x27C0D0..0x27C0D8 (`mov [ebp-14h],esi`,
  `mov [ebp-10h],esi`, `mov [ebp-0Ch],edx`), before the call at 0x27C0D9; it reads the world from
  the mutable buffer.
- **Raw:** the lohook `world_load.raw` is on the six bytes at 0x27C0FF (`mov ecx,[edi+1F1C40h]`,
  the target of the `je` at 0x27C053), before the pushes and the call at 0x27C111; the world is
  the packet's reader from its cursor to its end.
- **The exit `refused`** is the handler's epilogue 0x27C1E0 (`lea esp,[ebp-30h]` restores the
  stack, the pushed arguments and the 0x8000 bytes with it). It skips the post-load steps
  (0x27C116..0x27C1DF) and, on the unpacked path, the buffer's destructor (0x27C0DE..0x27C0FB),
  which frees nothing with a null allocator. The generation stays stored: the match link is
  closed.
- **The leave:** `close_current_match` 0x27A790 (thiscall, the reason, `ret 4`; engine row
  `engine_close_current_match`) with reason 2, as match.md section 25 has it.
