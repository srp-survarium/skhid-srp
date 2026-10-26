# Rust exe patches: the framework

How to write a patch of the 0.26g client exe in Rust, hook the game from it,
and prove a port of an asm or C patch. The code is `client-patches/rust/`
(the `patches` crate), `client-patches/engine_026g.py` and `hooks_026g.py`
(the generators), `client-patches/link_026g.py` (the arena link) and one
worked example, patch 34, the commit_suicide key.

The owner's shape (2026-10-25/26): one arena crate, not one per patch; three
layers `raw` -> `vostok` -> `patches`; the hook system modelled on NH3API /
patcher_x86 but applied when the build writes the exe; equivalence is
port-only tooling.

## 1. The three layers

A patch never touches a raw address or an `unsafe` block. Everything is in
one crate, `client-patches/rust/patches`, in three layers:

- **`raw`** (generated, unsafe, no logic): `extern` declarations for every
  game function and the address of every object field. engine_026g.py writes
  it (`engine_026g.rs`) from `engine_026g.tsv` (the functions) and
  `fields_026g.tsv` (the fields). A function Rust can call directly (stdcall,
  cdecl, thiscall) is declared in that convention at its VA; one the game
  takes in registers keeps its cdecl thunk (block `engine.code`). kernel32's
  imports are declarations over their IAT slots. Only `vostok` names `raw`.
- **`vostok`** (the engine, safe and typed): handles (`InputHandler`),
  enums and newtypes (`Action`, `ActionState`, `ActionMask`), and the hook
  system. It is the only place with `unsafe`, each block with its invariant.
  The runtime lives here too (the log, the panic handler, `memcpy`/`memset`,
  `c_u32`), in `vostok::rt`/`vostok::log`.
- **The patches** (`patches::<patch>`, one module each): safe logic over
  `vostok`, `#![forbid(unsafe_code)]`. The example is `suicide_key.rs`.

link_026g.py enforces the layering on every build (and the self-test
repeats it): in the crate's own sources no game address, no hand-made
binding (`GameFn`, `Import`, `transmute`, an `extern` block), and a patch
module forbids `unsafe` and never names `raw`. A new game function is a new
row of `engine_026g.tsv`, never a binding written by hand.

## 2. One crate, i686 everywhere

`rust/patches` is a `staticlib`, the arena's one block `patches.code` (with
its generated hook entries). One copy of the core, one `memcpy`/`memset`, no
per-patch page.

Two builds, both i686 (`i686-unknown-linux-gnu`), so no `#[cfg(target_arch)]`
appears anywhere:

- the game's: link_026g.py builds the crate with the dev-shell nightly rustc
  alone (no cargo, offline), `opt-level=s`, `panic=abort`, fat LTO, no
  allocator; its bytes are `arena_026g.lock.json`'s.
- the host's: `cargo test` and `cargo clippy --all-targets` in `rust/`, the
  logic and the views, built for i686 too (`rust/.cargo/config.toml` sets the
  target; the dev shell has the 32-bit std and linker). State a patch keeps
  lives in `static`s (`vostok::GameCell`, below); nothing allocates.

Logic shared beyond the client (with srp, the match server) is a library
crate of its own -- pure Rust, no game bindings, so it needs no `cfg` -- that
`patches` depends on; none exists yet.

## 3. The hook system

A hook is declared in a patch's `rust/patches/src/<patch>.toml` (TOML, read
by hooks_026g.py). The build generates the NASM entry
(`hook_entries_026g.asm`, block `patches.code`) and the `extern "C"` shim
(`hooks_026g.rs`, inside `vostok`), installs the site, and audits the
overlap. The kinds, modelled on NH3API's HiHook/LoHook applied at build
time:

| Kind | What | The handler gets |
|---|---|---|
| `call` | a game `call F` (and any pushes before it) | F's typed arguments and `Original<F>`; it decides whether and when to `orig.call(..)`, and returns F's result or a named `Exit` |
| `splice` | a function's start, through a trampoline | the same, the original a trampoline; `replace` when ours is the whole body |
| `pointer` | a function pointer in the image | the same; `Original` is the old pointer |
| `lohook` | a detour anywhere (>= 5 bytes) | `&mut Context`, returning a `Flow` |
| `vtable` | a run-time copy of a vtable with slots ours | each slot in its convention |
| `callback` | a function the game is handed | its typed arguments |
| `write` | a `byte`, `word`, `dword`, `float`, `bytes` or pointer, with its expected original | nothing |
| `code` | hand-written NASM with a byte limit, where nothing above fits | -- |

**`Context` (`lohook`)**: the registers (`reg`/`set_reg`, `set_reg8` for
`al`/`bl`/..), the flags (`set_flag` for a following `jcc`), xmm0..7
(`xmm_f32`/`set_xmm_f32`, f64), the x87 top (`set_st0`), the stack and typed
frame locals. **`Flow`**: `Continue` (apply the context's writes, then replay
the overwritten bytes), `Skip`, or `Exit(E)` -- a generated enum of the
hook's named exits, so a continuation is never return-address arithmetic in
Rust. An exit is a jump inside the host function or a `host_return { pop,
value }`.

Every `lohook`/`splice` entry saves and restores the full register, flag and
FPU/SSE state (`fxsave`/`fxrstor`); the result register the row declares is
written into the saved state (or `st0` loaded) on the way out. A `call`/
`pointer` entry wraps a call boundary, where the host already treats
`eax`/`ecx`/`edx`/xmm as clobbered, so it only aligns the stack, passes the
arguments and cleans up as the function did; it adds no `fxsave`.

**Chains**: several hooks on the same site declare an `order`; a partial
overlap fails the build. In a `call`/`splice` chain each handler's `Original`
is the next; in a `lohook` chain the first non-`Continue` flow ends it.
**Cross-patch**: a patch names another in `uses` and calls its `pub fn`s.
State shared between hooks is a `vostok::GameCell<T>`, reachable only inside
a hook (a `Thread` token), so patch code stays safe and single-threaded.

The TOML schema (a `call` row shown; the other kinds' keys are in
hooks_026g.py):

```toml
name = "suicide_key"       # = the module
patch = 34

[[call]]
at = 0x242196              # the site
original = "e8c5020000"    # the replaced `call rel32`; at + 5 + rel32 must be F's VA
function = "engine_action_present"   # F, a row of engine_026g.tsv (stdcall or cdecl)
handler = "on_spot"        # patches::suicide_key::on_spot
returns = "bool"           # mapped from F's result
original_name = "OrigActionPresent"  # the generated Original<F> type
what = "the input builder's last action query (spot)"
args = ["handler: InputHandler", "action: Action", "state: Out<u32>"]
```

Generated and tested (check_hooks_026g.py runs each kind's entry template
beside the original exe and checks it is transparent -- the arguments and
ECX delivered in order, the result passed back, the callee-saved registers
kept, the right bytes cleaned):
- `call` (the example): `RUST_CALL`;
- `pointer`, `vtable`, `callback` (the convention entries): `RUST_THISCALL`
  / `RUST_CDECL`, the handler's `Original` the function the slot or callback
  used to name;
- `splice`: the function's convention entry plus a trampoline (the relocated
  prologue, checked branch-free, then a jump past the start our jump
  replaced), which the handler's `Original` calls;
- `code`: a hand-written cave the site jumps to, linked into `patches.code`
  with a byte limit (the escape hatch for what no typed kind fits);
- `lohook`: a detour jumped to from a span (>= 5 bytes, branch-free so
  Continue can replay it). The entry saves the flags, the registers and the
  whole FPU/SSE state (`fxsave`), hands the handler a `Context` over them,
  and takes its `Flow`: Continue (replay the span, go on), Skip, or a named
  `Exit` (a generated `enum`, the entry jumps there). It reloads everything
  (`fxrstor`, `popad`/`popfd`), so what the handler wrote is what the game's
  code gets. check_hooks_026g.py checks Continue replays and is transparent,
  a Context register write applies, Skip does not replay, and Exit jumps to
  the named address; the generated shim compiles against the `Context`/`Flow`
  types.

`write` is the design above; a register-ABI `call` still needs its reverse
thunk. Only `call` ships today (the suicide-key example); the other kinds'
generators are proven by check_hooks_026g.py and wait for their first patch
to wire the install into patch_026g.py.

**fxsave on `lohook`/`splice`, not `call`/`pointer`/`callback` (the owner's
rule, kept).** A `lohook` and a `splice` are true detours: the game does not
expect any register or FPU/SSE state to change across them, so their entries
save and restore all of it. A `call`, `pointer`, `vtable` or `callback`
entry wraps a call boundary, where the host already treats EAX/ECX/EDX and
the XMM registers as clobbered and the callee-saved ones as kept; its entry
only aligns the stack, passes the arguments and cleans up as the function
did, so it adds no `fxsave`.

## 4. Writing a patch, by the example

Patch 34: the input builder never asks the handler for commit_suicide, so
the input's suicide bit, which the simulation still reads, is never set. The
patch wraps the builder's last action query (spot) and, before it, sets the
bit when the key went down.

1. **The `vostok` types** the patch needs, added to `vostok` once:
   `InputHandler` (a handle), `Action` (a newtype, so the game's unmodelled
   actions pass through), `ActionState` (an enum; an unknown value is a typed
   refusal, not a panic), `ActionMask::SUICIDE`. New game functions and
   fields are rows of `engine_026g.tsv` / `fields_026g.tsv`.
2. **The module**, `rust/patches/src/suicide_key.rs`, `#![forbid(unsafe_code)]`:

   ```rust
   pub fn on_spot(spot: &OrigActionPresent, handler: &mut InputHandler,
                  action: Action, state: &mut Out<u32>) -> bool {
       if handler.action_state(Action::COMMIT_SUICIDE) == Some(ActionState::Down) {
           handler.add_actions(ActionMask::SUICIDE);
       }
       spot.call(handler, action, state)
   }
   ```

   No `Regs`, no pointer, no address, no `cfg`: it reads the handler, sets a
   bit, and asks the game's own function for the spot the site wanted.
3. **The TOML** (section 3), declaring the `call` hook; `pub mod suicide_key`
   in `lib.rs`, `"suicide_key"` in `RUST_PATCHES` (patch_026g.py).
4. **Build**: `python engine_026g.py --write` (if the TSVs changed),
   `python hooks_026g.py --write`, `python link_026g.py --write` (the lock).
5. **Check** (section 5).

## 5. The build and the checks

- **Generators**: engine_026g.py (`--write`/`--check`) writes the `raw`
  layer; hooks_026g.py (`--write`/`--check`) writes the entries and shims
  from the TOML and lists the installs (with their owning patch) for
  patch_026g.py. link_026g.py links the arena and writes the lock; it builds
  the crate and runs the layer check first.
- **Overlap audit**: hooks_026g.py rejects two Rust hooks whose replaced
  spans overlap (each hook names its patch); the whole arena's audit, across
  the asm and C installs too, is selftest_026g.py's span check (it now
  includes the Rust hooks).
- **Sizes**: `python link_026g.py --sizes` prints each block's bytes and
  pages and, for `patches.code`, each patch's bytes from its symbols -- the
  budget a port reports before and after.
- **Tests**: `cargo test` and `cargo clippy --all-targets` in `rust/` (i686);
  the patch's own Unicorn check unchanged (`check_suicide_key_026g.py` runs
  the site to the reconvergence point against the original exe, which is the
  behavioural proof and handles the call-and-return shape); `selftest_026g.py`
  check 35 (the action table, the builder's 35 queries without 1Fh and
  without the bit, the consumer and its one caller, the site, the generated
  entry's shape, the blocks and the lock) plus the engine, hooks-staleness
  and layer checks.
- **Not run in a game** until the owner plays it.

## 6. Porting an asm/C patch: the equivalence gate

`client-patches/port/equiv_026g.py` is **port-only tooling** (the owner,
"equivalence is only needed while we do the port"); the `port/` folder is
deleted once the last port lands. It builds the exe with the old patch (asm
or C, at a git revision) and the exe with the new Rust and runs each hook of
`port/equiv/<patch>.py` from the same input states in both -- the patch's
check cases, edge values, seeded random states, captured states -- comparing
the exit, the calls into the game and the memory written. It suits a port
that keeps the original's control-flow shape (a tail jump to the same
reconvergence). The holistic `call` hook wraps the original with
call-and-return instead, so a ported patch's fidelity is proven by its own
Unicorn check against the original exe (`check_<patch>_026g.py`), which runs
to the reconvergence point; the gate stays for shape-preserving ports and as
the porting model.

The equivalence gate may skip an out-pointer argument and a slot the game
never reads again (declared in the spec, with the proof cited), and a new
Rust patch that is not a port needs no spec -- its Unicorn check and cargo
tests are enough.

## 7. What a crate costs

The example's `patches.code` is about 190 bytes: the generated entry
(`RUST_CALL`), the shim, `OrigActionPresent::call`, and the weak
`memcpy`/`memset` the arena keeps one of. `core::fmt` is never linked (the
log takes pieces). `link_026g.py --sizes` gives the per-patch bytes.
