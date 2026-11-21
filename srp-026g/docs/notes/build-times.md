# Build times: from a commit to a local test

Measured 2026-11-21 on the owner's machine (Ryzen 9 5900X, 24 threads; busy with other
builds for part of it), revision 69, main at `5e605816b`. A local test builds three things:
the exe, the quick content and the debug server. `tools/test-build` builds them at the same
time (`AGENTS.md`, "Local test builds").

## The exe (`patch_026g.py`)

| build | before | after |
| --- | --- | --- |
| first build in a new worktree | 127 s | 5 s (35 s with nothing kept on the machine yet) |
| a build that changes no Rust source | 80 s | 4.5 s |
| a build that changes a Rust patch | 80 s | 24 s |
| nothing kept (`SRP_ARENA_CACHE=off`: the release, the self-test) | 80 s | 29 s |

Where the 80 s went: 50 s in `neutralize_relocs`, which read the whole base relocation table
(206,518 entries) at each of its 413 calls; 23 s in rustc (the `patches` crate with fat LTO and
one codegen unit: 18 s, of which 12 s is LLVM; its path crates 5 s); 4 s of Python, NASM, gcc
and ld. A new worktree also built `core` (12 s) and evaluated the flake.

- The table is read once and kept in step with what the function changes.
- What rustc made of each crate is kept in `~/.cache/skhid-arena` by what it was made from
  (`client-patches/README.md`, "A Rust crate as a source"), for every worktree. The release and
  the self-test build with the cache off.
- The same exe: revision 69 is `337d45db…` before and after, in a new worktree too; the four
  build modes (default, `--no-late-join`, `--cave=hash-mismatch`, `--cert-only`) and their
  `--installs` listings are byte for byte the base commit's; with three Rust inputs changed, the
  build from the cache and the build with the cache off give the same lock and the same exe.
- Left: the 18 s of the `patches` crate when a Rust patch changes. Incremental compilation and
  more codegen units would change its bytes (and the lock); `-Zthreads` was not tried for the
  same reason. NASM and gcc run for every source each build: 0.8 s in all.

## The dev shell

`nix develop --command true`: 8.4 s at a worktree's first use, 4.2 s after any tracked file
changed (the flake's source is the whole repository, so it is evaluated again), 2.0 to 2.6 s
otherwise. `tools/dev`: 0.15 to 0.2 s, from a profile kept under the hash of `flake.nix`,
`flake.lock` and the Nix files the flake imports; the environment is the same (compared
variable by variable; only `out` and the `-rpath` of `NIX_LDFLAGS` differ, with the folder the
command runs in). The release's scripts still call `nix develop`.

## The movies

`flash_tree.py build`: hud 8.9 s to 4.7 s, the lobby 13 s to 7 s, the same bytes. A PNG's
pixels were decoded and premultiplied a byte at a time in Python at every build (hud: 52
bitmaps, 4.7 s; the lobby: 139); they are kept by the PNG's bytes and `swf.py`'s own
(`~/.cache/skhid-checks/bitmaps`). The rest is Flex (2.7 s hud, 3.6 s lobby) and zlib.

## The pack makers

Measured, not changed: `artefact-hunt/make_pack.py` is 1.4 s of Python (23 s with its inputs
cold on the HDD, 72 s with the fit report of `--mapdata`), `v011e-vostok-anomalies` 1.2 s.
The makers run no heavy program (music: ffmpeg). A pack's change costs its minutes in
`release/build-content`, not in its maker.

## The content (`release/build-content`, quick)

One build of 2026-11-21 (Artifact Hunt, 84,402 files made): 1,632 s in `build_content.py`.

| step | s |
| --- | --- |
| the edit lists | 714 |
| the value types | 666 |
| the stats checks | 132 |
| the packs (`vostok-port pack patch`) | 84 |
| the server game data | 63 |
| the movies | 20 |
| srp-game-data built | 19 |

Not changed here (another change has the first two). What the others are:

- The packs: every pack of the manifest is composed in one `vostok-port` run whenever any pack
  or the lobby movie changes (the packs draw their icons into it).
- The server game data: `srp-game-data build` hashes the stock db and every offered patch, 18
  GB, one after another, each build.
- The stats checks are kept per category: one changed config checks its whole category again.
- The listing of every made file, unchanged ones too, is 120 MB of log and a 119 MB
  `files.tsv` a build.

## The server (`cargo build -p srp`): not measured

The measurements need builds of the whole workspace, and the machine had no room for them
that day. What is known: a fresh worktree compiles every workspace crate (about 280,000 lines;
sccache keeps the dependencies only) into a target of its own (two looked at: 11 and 18 GB); `cargo build -p
srp` links four binaries (`srp`, `srp-bot`, `srp-recording`, `srp-worlds`: 228, 171, 127 and 47
MB with full debug information) where a test runs one; one incremental build of a change in
`artefact-carry` (it and the ten crates after it) took 103 s.

Done without numbers, because they change no output: `tools/test-build` builds `--bin srp`,
and `--shared` builds in one test tree whose target stays warm.

To measure (from a checkout with a built `target/debug`; each line timed):

```sh
dev=tools/dev; cd srp-026g
$dev cargo build -p srp                                        # no-op
touch crates/session/src/lib.rs;  $dev cargo build -p srp             # four binaries
touch crates/session/src/lib.rs;  $dev cargo build -p srp --bin srp   # one
touch servers/srp/src/main.rs;    $dev cargo build -p srp --bin srp   # the link, about
# the linkers (nix build --inputs-from . nixpkgs#mold-wrapped, nixpkgs#llvmPackages.bintools)
touch servers/srp/src/main.rs; $dev cargo rustc -p srp --bin srp -- -C link-arg=-B<mold>/bin -C link-arg=-fuse-ld=mold
touch servers/srp/src/main.rs; $dev cargo rustc -p srp --bin srp -- -C link-arg=-B<bintools>/bin -C link-arg=-fuse-ld=lld
# the same series with less debug information (everything builds again once)
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
```

Then decide `[profile.dev] debug` (line tables keep a backtrace's files and lines and drop
the variables a debugger shows) and the linker (in the dev shell's environment, so only its
users get it). Neither changes what the server computes.
