# Rust exe patches: the framework

How to write a patch of the 0.26g exe in Rust, and how a port of an asm or C
patch proves itself. The owner (2026-10-25): "we need to port all our asm we
want to Rust", then "we first want the framework itself without much logic",
and "good testing that our Rust implementation is behaviourally identical to
asm/C". The framework is this note, `client-patches/rust/` (the workspace,
`patch-core`), `client-patches/rust_hook_026g.inc` (the hook ABI),
`client-patches/link_026g.py` (the build), `client-patches/equiv_026g.py`
(the gate) and one example patch, the commit_suicide key (patch 34). No game
logic is in the framework.

## 1. The decisions

| What | Decision | Why |
|---|---|---|
| Layout | One cargo workspace, `client-patches/rust/`: `patch-core` and **one crate per patch** | Each patch is its own ARENA block (one object each): its bytes, its size and a failure are its own; a change of one patch moves no other's code. `patch-core` is linked into each (fat LTO keeps only what a patch uses); its weak `memcpy`/`memset` and its panic handler are the only shared symbols. |
| Not in srp-026g's workspace | `rust/` is a workspace of its own | The server never links these; their host build (`cargo test`) leaves out what is x86-only. Logic the server shares (the sync core) stays a crate of srp-026g's workspace that a patch names by path. |
| The game's build | `link_026g.py` (moved from #452): rustc alone, offline, i686, `-C opt-level=s -C panic=abort -C codegen-units=1`, fat LTO, static relocation, `target-cpu=pentium4` | No cargo (`-Zbuild-std` needs crates.io); the bytes are the lock's, rebuilt by every build. |
| Allocator | **None** | A patch keeps its state in `static`s (the block's `.state`, zeros) and allocates what the game owns with the game's allocator (`engine::engine_malloc`). No bump arena: nothing needs one, and a heap of ours would be a second allocator the game cannot see. |
| Toolchain pin | The dev shell's nightly with `rust-src` (`flake.nix`, pinned by `flake.lock`'s rust-overlay); the lock's `built_with` names the rustc | An update of the pin changes the arena's bytes, and the build stops until the lock is written again on purpose (the same rule as NASM 2.16). |
| Hook ABI | Three NASM macros (`rust_hook_026g.inc`); the Rust side `extern "C"` | The game's frames are aligned to 4, rustc's i686 code assumes 16 (it keeps 16-byte locals with `movaps`: a crash on a misaligned frame). Only a macro enters Rust, and it aligns. |
| Engine calls | The engine library (`engine_026g.tsv` -> generated `engine_026g.rs`), `GameFn` for plain cdecl/stdcall/thiscall, `Import` for kernel32 | A function in registers gets a cdecl thunk proved against the exe (`check_engine_026g.py`); Rust never writes a register convention by hand. |
| Game memory | `Field<K, T>` offsets of an object kind `K`, `Obj<K>`, `Global<T>`, `Va` | An offset is named once as a constant; a field of another kind does not compile; a pointer field reads as `Option<Obj<_>>`. No offset arithmetic in patch code. |
| Types | `GameTime`, `Tick`, `Place`, `Va`; one enum a state; `parse::Reader` / `Parse` | AGENTS.md "Types and states": raw input parsed once at the boundary. |
| Panics | A log line (file and line) to `srp_patches.log`, then `ud2` | A panic is a bug, never a refusal: a hook refuses with a value and its entry takes the game's own path. There is no unwinding, and nothing goes on half done: the crash reporter gets our address, the log the line. A recovery (an SEH frame per entry) was looked at and left out: the exe's handlers and Wine make it a path nobody tests. |
| Log | `patch_core::log::line(&[Text(..), Hex(..), Dec(..)])` to `srp_patches.log` beside the exe | No `core::fmt` (kilobytes of arena); kernel32 through the exe's IAT. |
| Tests | Host `cargo test` (logic, views); the patch's Unicorn `check_*` on the built exe, unchanged; **the equivalence gate** for a port | |
| Size | `link_026g.py --sizes` per block, against `ARENA_LIMIT` (1 MiB) | A port reports its block's bytes before and after. |

## 2. Writing a patch

1. A crate `client-patches/rust/<patch>/`: `Cargo.toml` (`crate-type =
   ["staticlib"]`, `patch-core = { path = "../patch-core" }`, `[lints]
   workspace = true`), a member of `rust/Cargo.toml`. `src/lib.rs` starts
   `#![cfg_attr(target_arch = "x86", no_std)]` (no_std in the game, std on
   the host for its tests). The game's side (hooks, `stdcall`, `GameFn`
   constants of a convention) is `#[cfg(target_arch = "x86")]`; the logic is
   plain and tested on the host.
2. Its NASM entry, `client-patches/<patch>_026g.asm`, ten lines or so:

   ```nasm
   bits 32
   section .text
   %include "rust_hook_026g.inc"
   global suicide_key
   suicide_key:
       RUST_HOOK suicide_key_hook      ; registers saved, stack aligned, Rust called, registers back
       jmp 0x242460                    ; the replaced call, as the game made it
   ```

   - `RUST_HOOK target`: in the middle of the game's code (a replaced call, a
     jump to us): `target(regs: &mut Regs)` sees every register, the flags
     and the game's stack at the entry (`regs.stack(i)`), and what it writes
     to `Regs` is what the game's code after the entry gets (a result in EAX
     or AL, ZF for a `jz` after it). The entry then does the replaced
     instructions and jumps back.
   - `RUST_THISCALL name, target, n`: a vtable slot or delegate the game calls
     with ECX and `n` stack dwords (`ret 4n`): `extern "C" fn target(this,
     a, ..) -> R`.
   - `RUST_CDECL name, target, n`: a cdecl function the game calls.
3. Its ARENA row in `patch_026g.py`: `ArenaBlock("<patch>.code", "code",
   0x1000, ("<patch>_026g.asm", "rust/<patch>/Cargo.toml"))` (and a
   `"<patch>.state"` block naming the crate if it has `static`s); its hooks
   as (site, bytes, symbol, opcode) rows applied with the arena's; each
   declared in `INSTALLS` (client_patch.md 0.4); `python link_026g.py
   --write`.
4. Game memory through views, declared once:

   ```rust
   pub enum InputHandler {}
   pub const ACTIONS: Field<InputHandler, u32> = Field::at(0x33C);
   #[cfg(target_arch = "x86")]
   pub const ACTION_STATE: GameFn<unsafe extern "stdcall" fn(u32, u32, *mut u32) -> u8> =
       GameFn::at(Va(0x242460));
   // ...
   let Some(handler) = Obj::<InputHandler>::at(Va(regs.ebx)) else { return };
   handler.set(ACTIONS, handler.get(ACTIONS) | SUICIDE);
   ```

   A function the game takes in registers is a row of `engine_026g.tsv` (and
   of the re-kit), called as `patch_core::engine::engine_<name>(..)`.
5. State: `#[repr(C)]` structs mirroring what the game (or an older patch's
   layout) holds, its states enums; a byte read from the game is parsed with
   a `match` (or `Option<Team>`-style niches proven by a `const` assert),
   never transmuted.
6. A port of C that converts floats to unsigned keeps the C's results with
   `patch_core::c_u32` (gcc -m32: x87 `fistp` to 64 bits); float arithmetic
   is SSE single or double precision in both (the C patches' `-msse2
   -mfpmath=sse -ffp-contract=off`), in the C's order. A float path whose
   results cannot be matched stays asm or C.

## 3. Proving a port: the equivalence gate

**A port is accepted only with the gate green**: zero differences and every
basic block of the old code covered.

```
python equiv_026g.py <patch> <original survarium.exe> [--old REV] [--cases N] [--states DIR]
```

The spec `client-patches/equiv/<patch>.py` says:
- `HOOKS`: where the game enters the patch (`Hook(name, site, live,
  externals)`: the run starts at the game's own instruction at `site`, ends
  at the first leaving of the patch's code that is not a call; `live` the
  registers compared there);
- `EXTERNALS`: every game function the patch calls (`External(name,
  inputs, pops, model, real)`: its inputs are compared; a stub (`model`) or
  the game's own code (`real=True`));
- `ranges(P, linked)` and `entries(P, linked)`: where the patch's code is in
  a build (the old one's `.srp` offset or the new one's arena block);
- `cases(seed, count)`: (a) the patch's `check_*` cases, (c) edge values
  (0, all ones, boundaries; for floats NaN, infinities, denormals and the
  x87/SSE cases), (b) thousands of seeded random states in the hook's input
  domain. `--states DIR` adds (d) captured states (JSON: hook, regs, stack,
  memory), for example from `private/local-clients/runtime/`; never from
  `private/live-worlds-*` without asking.

It builds the old exe from `--old` (the commit before the port by default:
`git archive` of client-patches, crates and the re-kit, the sysroot shared)
and the new one from the working tree, and runs every case in both: the same
registers, stack and memory, and memory the case does not give made from the
case's seed and the address (lazily mapped pages, alike in both). It
compares the exit (its address, the live registers, ESP), every call into
the game in order with its inputs, and the final bytes of every place either
run wrote outside its private stack (an arena block's bytes by block and
offset, so the layouts of old and new compare). Then the old code's coverage:
its basic blocks by recursive descent (ndisasm) against Unicorn's block hook.

The example (patch 34, the NASM it replaced against the Rust): 3000 cases (5
x 8 edge masks, check_suicide_key's ten cases with the game's own
action_state on made-up event buffers, the rest random: stubbed answers and
random buffers), **0 differences, 5/5 basic blocks**. A broken port (a
repeated key also kills) shows as 15 differing cases in 300, each printed
with the byte that differs.

## 4. The other checks

- The patch's own Unicorn check (`check_<patch>_026g.py`) runs **unchanged**
  on the built exe: it enters through the game's site, so where the code is
  does not matter. Only plumbing that names a layout (a `.srp` offset, a
  symbol) is adapted, and the commit says what and why.
- `selftest_026g.py`: the site, the block's bytes (the link's), the entry's
  instructions, where the code goes outside.
- `cargo test` and `cargo clippy --all-targets` in `client-patches/rust`.
- `python link_026g.py --sizes`: the block's bytes before and after.
- **Not run in a game** until the owner plays it.

## 5. What a crate costs

The example's block is 135 bytes: the entry (26, then 2 of padding), the hook (63) and the
weak `memcpy`/`memset` (44, kept by every crate that links `patch-core`:
weak symbols are exported). A panic adds its location (file name and line)
to `.rodata`; `core::fmt` is never linked (the log takes pieces).

## 6. What moved from #452

From `feat/sync-core` (#452), unchanged: `link_026g.py`'s Rust support
(crate sources, the sysroot, the build) and `engine_026g.py`'s Rust output
(`engine_026g.rs`). #452 keeps the sync core: `crates/sync-core`,
`sync_core_arena`, `sync_core_026g.asm`, its ARENA rows, its engine row and
its checks. On its rebase onto this framework: `sync_core_arena` becomes
`rust/sync_core` (or keeps its folder) and drops its own panic handler for
`patch-core`'s, includes the engine declarations through `patch_core::engine`,
and its entry can be `RUST_THISCALL`-style or stay as it is (it already
aligns).
