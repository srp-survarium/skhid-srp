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

## 0. The design principle

Each thing is written once, in the layer that owns it, and everything else is
generated and checked:

- **Hand-written is only what was found out or decided.** A fact about the
  exe -- a function's VA and convention, its argument locations, a field's
  offset, a named site's VA and kind -- is written once, in a raw table
  (`engine_026g.tsv`, `fields_026g.tsv`, `sites_026g.tsv`), with its evidence
  in the re-kit (`tools/re-kit/v026g`). A patch's decisions are names only:
  which site, which handler, the chain order. What a value *means* -- the
  Vostok types (`Action`, `Out<u32>`), what is safe -- is written once in
  `vostok`.
- **Everything that follows is generated at build time:** the entries, the
  shims, the `Original` types, the expected bytes, the trampolines, the
  install writes, the overlap audit, the sizes.
- **Everything that can drift is checked against the original exe at every
  build:** a site's bytes, a call really going to the named function, a
  prologue's shape. A mismatch fails the build. Nothing is looked up at run
  time -- there is one exe and it is never rebased, so no pattern scanning.

Two consequences:

- **(a) The hook kind follows from the site.** A call-instruction site is a
  `call` hook; a function start is a `splice`; and so on, from
  `sites_026g.tsv`. A patch names a kind only where there is a real choice at
  that place (wrapping a `call` vs splicing its callee), never to repeat what
  the site already says.
- **(b) The raw tables do not duplicate the re-kit.** `engine_026g.tsv`
  selects which functions are exposed and gives their Rust types; the VA and
  convention come from the re-kit rows (`functions.tsv` / `abi.tsv`), checked,
  so no fact is stored twice. The same holds for a site: its VA and kind come
  from, or are checked against, the exe and the re-kit.

So a patch's whole declaration is, for the example:

```toml
# rust/patches/src/suicide_key.toml
name = "suicide_key"
patch = 34

[[hook]]
site = "input_builder.spot_query"   # sites_026g.tsv: VA 0x242196, kind call, callee engine_action_present
handler = "on_spot"                 # patches::suicide_key::on_spot
```

No address, no bytes, no signature, no `Original` name -- all derived from
the site and its callee (section 3), and checked against the exe.

## 1. The three layers

A patch never touches a raw address or an `unsafe` block. Everything is in
one crate, `client-patches/rust/patches`, in three layers:

- **`raw`** (generated, unsafe, no logic): the only layer that knows the
  game's memory. engine_026g.py writes it (`raw_026g.rs`) from
  `engine_026g.tsv` (the functions), `fields_026g.tsv` (the kinds and their
  fields) and `sites_026g.tsv`'s `holds` (what a site's registers hold):
  - a handle per kind, a `#[repr(transparent)]` pointer that is never 0
    (`Player`, `MovieHolder`, `GfxMovie`, ...), with an unsafe accessor
    per field of its own. A field's `type` says what it is: a plain value
    (`u8` .. `f32`: its place), `ptr<kind>` (read: `Option<Kind>`),
    `ptr<u8>`/`ptr<f32>` (plain data: a C string, matrices), `obj<kind>`
    (one inside it, at the offset) or `size` (the kind's size: `at(i)`
    and `index_from` over an array of them, and `<Kind>Storage`, room of
    ours for one);
  - every game function, its prototype's `struct <kind> *` that kind's
    handle (`Option`, as C may pass or return 0). A function Rust can call
    directly (stdcall, cdecl, thiscall) is declared in that convention at its
    VA; one the game takes in registers keeps its cdecl thunk (block
    `engine.code`). kernel32's imports are declarations over their IAT slots;
  - `raw::site::<site>::<name>(machine)`: the handle a site's register (or
    its frame, by ESP or EBP) holds, as the site's `holds` declares it.

  A handle is made there alone: from a typed field, a function's typed
  result or a site's typed register; there is no constructor from a number.
  Only `vostok` names `raw`. (`engine_026g.rs` is the same functions with
  C's types, for research_arena.)
- **`vostok`** (the engine, safe and typed): what the handles mean -- its
  own handles borrowing the hook's thread (`InputHandler`, `Player`,
  `Movie`), enums and newtypes (`Action`, `ActionState`, `ActionMask`),
  and the hook system. It is the only place with `unsafe`: calls of
  `raw`'s accessors, each saying why the object is alive on that thread,
  never an address or an offset.
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
`patches` depends on. The first is patch 76's solver, `srp-026g/crates/ragdoll`
(a path dependency; tested with `cargo test -p ragdoll` in srp-026g).

## 3. The hook system

A hook is declared in a patch's `rust/patches/src/<patch>.toml` as a
`[[hook]]` naming a `site` (a row of `sites_026g.tsv`) and a `handler` (and,
for a `code` hook, the author's `source`/`symbol`/`limit`). The kind, the VA,
the callee and the typed signature come from the site and its callee row;
hooks_026g.py generates the NASM entry (`hook_entries_026g.asm`, block
`patches.code`), the `extern "C"` shim and `Original` (`hooks_026g.rs`, inside
`vostok`), and the install, and audits the overlap. The kinds, modelled on
NH3API's HiHook/LoHook applied at build time:

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
| `entry` | a Rust handler our own NASM calls (cdecl; a site row with VA `-`, its arguments the frame at ESP in `holds`), installed nowhere: the build writes its address where our code calls it | `&mut Context`, returning `Flow::Continue` |

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

**`entry`**: the way from our own NASM into Rust (patch 48's preview apply calls patch 68's). Its generated entry saves the machine and aligns the stack as a lohook's does, so the Context's site ESP is the caller's (the return address, then the arguments), and returns with `ret`; the caller pops its arguments. Never a hand-written `#[no_mangle]` export called from NASM: rustc's i686 code keeps 16-byte locals with movaps and assumes 16 at every call, and the game's stack (our NASM's too) is aligned to 4 only, a #GP on a real CPU that Unicorn does not raise (#893's review). `check_hooks_026g.py` runs the entry from each of the four skews.

**Lohook chains**: handlers sharing an exact named site declare distinct
integer `order`s. A single entry saves/restores the machine; handlers observe
earlier Context writes and the first non-`Continue` ends the chain. A partial
overlap, mismatched site/span/exits, absent order or duplicate order fails the
build. Lamps and ragdolls share their frame/ray sites this way. `call`/`splice`
chains remain unsupported.
**Cross-patch**: a patch names another in `uses` and calls its `pub fn`s.
State shared between hooks is a `vostok::GameCell<T>`, reachable only inside
a hook (a `Thread` token), so patch code stays safe and single-threaded.
State the game reaches from several of its threads (mt_draw's tasks, the
bullet manager's) is a `vostok::Lock<T>` instead: a spin lock, its value
`&mut` inside `with`, a lock taken again on its own thread a panic. A
static that is not all zeros goes to `patches.data`, a zero one to
`patches.state` (ARENA).

The declaration is names only; the facts are in the tables:

```toml
# rust/patches/src/suicide_key.toml   -- the whole thing
name = "suicide_key"
patch = 34

[[hook]]
site = "input_builder.spot_query"
handler = "on_spot"
```

```
# sites_026g.tsv   -- site, va, kind, callee, span, exits, what
input_builder.spot_query   0x242196   call   engine_action_present   -   -   the input builder's last action query (spot)

# engine_026g.tsv   -- the callee's row carries the typed signature
engine_action_present   0x242460   ...   stdcall   handler: InputHandler, action: Action, state: Out<u32> -> bool
```

From these the generator derives the replaced bytes (a `call` to the callee,
checked against the exe), the shim's typed arguments and result, and the
`Original` type's name (`OrigActionPresent`, from the function). A `code`
hook's `[[hook]]` also carries the author's `source`, `symbol` and `limit`
(its own NASM); a `lohook`'s exits are the site's. The Vostok types
(`InputHandler`, `Action`, `Out<u32>`, the `bool` from `AL`) are `vostok`'s,
named once in the callee's `vostok` column so every patch hooking it gets the
same typed signature.

Direct hooks of the same function share one generated `Original` type. A
splice has its own `Orig<Function>For<Patch>_<Handler>` type because its
`call` must enter that hook's trampoline. Replayable spans are decoded into
complete instructions with ndisasm; relative branches and truncated
instructions are refused, while opcode-looking bytes in operands are safe.

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
thunk. `call`, `lohook`, runtime `vtable` copies and runtime `callback`s ship.
Other pointer installs remain refused. The convention entries themselves are
exercised by `check_hooks_026g.py`.

A **runtime callback** is a `callback` site whose callee is a game function of
the convention the game calls it in (`resources.query_done`: 0x14730, the cdecl
`f(queries_result&, value)` a boost function of 0x176D0 calls). No site is
rewritten: the generator writes the entry (`RUST_CDECL`/`RUST_THISCALL`), a
marker type for the convention (no `Original`: the callee only names it, so the
handler gets the typed arguments alone, each a binding, never a fallible one),
and `<patch>_<handler>_entry() -> CallbackEntry<Marker>`, which vostok hands the
game (`vostok::resources::query_track`). Patch 88's query callback is the first.

A runtime vtable declaration names its source in `fields_026g.tsv`:

```toml
[[vtable]]
name = "visual"
source = "visual_methods"
check = "check_lamps_026g.py"
```

The source is an image `obj<methods>` row. Its kind supplies the size and RTTI
field at -4. Individual `[[hook]]` rows name vtable sites whose callees carry
the native and Vostok signatures; their `proof` labels identify native dispatch
checks. Generation copies RTTI and untouched slots from the original exe,
checks every replaced slot's native callee, and returns an opaque
`NativeVtable<T>` handle. Only the project registry installs it on resolved
objects. The same declarations generate `INSTALLED_VTABLES` audit rows.
A missing, ambiguous, unproved or incorrectly typed copy fails the build. A span
hook's bytes come from the original exe: patch_026g.py's `rust_hooks()`
reads them from the exe it builds from (hooks_026g.py and the checks take
`SRP_026G_EXE`), and INSTALLS declares every lohook a JUMP from its site row.
Which kind a site's register or frame holds is said once, in the site row's
`holds` (`player=EBX:survarium::player frame=EBP:survarium::mt_draw_frame`;
ESP is the site's own), from which `raw::site` is generated; vostok's view
of the site (`vostok::draw`, `vostok::bullet`, ...) says what it means. A
frame is a kind of `fields_026g.tsv` whose rows are its locals (a negative
offset by EBP); a data VA the code compares with (a vtable) is a row of the
kind `<...>::image`, generated as a constant (an object there, `obj<kind>`,
as its handle). A slot whose kind is not known stays a plain `u32`.

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

1. **The facts, once in the tables.** The site is a row of `sites_026g.tsv`
   (`input_builder.spot_query`, VA and kind and callee); the callee is a row
   of `engine_026g.tsv` with a `vostok` typed-signature column; any new field
   is a row of `fields_026g.tsv`. The **`vostok` types** the signature names
   are added to `vostok` once: `InputHandler` (a handle), `Action` (a newtype,
   so the game's unmodelled actions pass through), `ActionState` (an enum; an
   unknown value is a typed refusal, not a panic), `ActionMask::SUICIDE`.
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
3. **The declaration** (names only): `rust/patches/src/suicide_key.toml` with
   `[[hook]] site = "input_builder.spot_query"`, `handler = "on_spot"`;
   `pub mod suicide_key` in `lib.rs`.
4. **Build**: `python engine_026g.py --write` (if the TSVs changed),
   `python hooks_026g.py --write`, `python link_026g.py --write` (the lock).
5. **Check** (section 5).

## 5. The build and the checks

- **Generators**: engine_026g.py (`--write`/`--check`) writes the `raw`
  layer (`raw_026g.rs`); hooks_026g.py (`--write`/`--check`) writes the entries and shims
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
- **The alignment probe** (`align_probe_026g.py`): Unicorn does not fault on a misaligned aligned-SSE operand. A check that imports it and calls `install()` has every Unicorn machine check each aligned-SSE memory operand our code runs (`.srp` to the arena's end) and reports the misaligned ones; `python align_probe_026g.py check_<patch>_026g.py <exe>` runs any check under it unchanged. `check_weapon_kit_026g.py` and `check_upgrade_preview_026g.py` have it built in.
- **Not run in a game** until the owner plays it.

The generator regressions can also run alone in the dev shell, from
`client-patches`: `python -m unittest test_hooks_026g test_runtime_hooks_026g port.test_equiv_026g`.
They check forward/backward/wrapping calls, span decoding, multiple generated
Originals compiled and run on i686, and both equivalence-build subprocesses.
The self-test runs them too; `check_hooks_026g.py` also checks incoming flags
and Context flag edits through the generated lohook entries.

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
