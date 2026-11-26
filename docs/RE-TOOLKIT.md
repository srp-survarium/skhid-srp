# Reverse-engineering toolkit

> **Version scope:** the tools are general. Addresses in the examples are for
> **client 0.21d** (build 2010) unless marked otherwise. Other builds have
> other addresses. Builds after 0.21d have no PDB (see "Symbols by build"), so
> their addresses come from matching functions against 0.21d.

`tools/re-kit` holds the scripts that were used to read the game client and
the servers' logs. **Only the scripts are here.** What they read and what
they produce is not: the game's exe, PDB and `resources.db`, the full
disassembly, the function lists and the data dumps are parts of the game or
made from it (`docs/PUBLISHING.md`). This page says how to make each of them
from your own copy of the game.

Several scripts have the author's paths at their top (`E:/sur-res-pro/...`):
`common/scripts/exes.py` (where each build's exe is), `dis.py` (where
`ndisasm` is), `match_functions.py`, the `v0100b/e2e` clients (the login
certificate). Change them to your layout.

## What you need

| What | Where it comes from |
|---|---|
| The original game, 0.21d (build 2010) | archive.org, `survarium_full_021d.7z` (with `survarium.pdb`) |
| The original game, 0.26g (build 2777) | archive.org, `survarium_full_026g0.7z` (no PDB) |
| The 0.100b game | the 2013 client the SRP project targets |
| nasm and ndisasm | nasm.us (2.16.03 was used) |
| The `resources.db` unpacker | [srp-survarium/vostok-resources-db](https://github.com/srp-survarium/vostok-resources-db), checked out as `tools/vostok-resources-db` |

0.21d address math: image base 0x10000, `.text` at VA 0x11000 = file offset
0x400, so **file offset = VA − 0x10C00**. The patched 0.21d exe has its new
code in padding and a few patched sites, so its addresses are the same.

## Symbols by build

Checked 2026-10-12 by reading each archive.org archive's file table with
range requests (a few KB each, no full download). The scripts are
`common/scripts/list7z.py`, `listzip.py` and `listiso.py` (`python -I
list7z.py <url>`). A control run on the 0.21d archive found its PDBs.

| Build | archive.org file | PDB |
|---|---|---|
| 0.20e (1916), 0.20f (1923), 0.21d (2010) | `survarium_full_0NNx.7z` | **yes**: survarium, launcher, service, updater |
| 0.23h (2285) | `survarium_full_023h.7z` | no |
| 0.25c (Oct 2014) | `Survarium.iso` (Inno installer: launcher, updater and an `installer.sup` patch) | no |
| 0.26e (2727) | `survarium_full_026e0.7z` | no |
| 0.26g (2777) | `survarium_full_026g0.7z` | no (the exe names `D:\survarium.deploy\binaries\Win32\survarium.pdb`, not shipped) |
| 0.27d to 0.33a5, 0.44a5 and later | small Steam/web installers that download the game | no |
| 0.34a0 (3545) | `survarium_full_034a0.zip` | no |

The 0.10b–0.11c builds (2013) have PDBs in the srp-survarium/vostok-versions
`binary-evidence` release. **0.21d is the last build with symbols anywhere
public.**

## Finding code

| Tool | Use |
|---|---|
| `common/pdb-procs/` | Rust program (crate `pdb`) that writes the function list of a build that has a PDB: `cargo build --release`, then run it on `survarium.pdb` and keep the output (`address length name`, sorted; 41,797 lines for 0.21d). Grep a name to get its address, or bisect an address to get its function |
| `common/scripts/dis.py <build> VA LEN`, `dis.sh <build> VA LEN` | Disassemble LEN bytes at VA with ndisasm |
| `common/scripts/callers.py <build> VA ...` | Every direct `call VA` site (E8 scan). Indirect calls (vtables) are not found |
| `common/scripts/xref.py <build> TEXT`, `fn.py <build> ADDR\|NAME` | Where the code uses a string (every 4-byte reference to its address); the function an address is in, or the functions with a name (from the function list) |
| `common/scripts/symbolize.py <dumpbin.txt> ADDR...` | Map addresses to the function labels of a dumpbin listing |
| `common/scripts/pe.py`, `rva.py` | Print PE sections, convert addresses |
| `common/scripts/match_functions.py` | Matches the functions of a build without symbols (0.26g) against one with them (0.21d): exact bytes, shape, strings, call graph, vtables. Its docstring has the method and how sure each kind of match is; `srp-026g/docs/notes/client_patch.md` has the numbers |

Two quick recipes:
- **Find what reads a string:** find the string's VA in the exe, then search
  the exe for those 4 bytes little-endian (a `push imm32` or a pointer table),
  then bisect each hit in the function list.
- **A full listing with symbols** comes from Visual Studio's dumpbin
  (`dumpbin /DISASM:NOBYTES survarium.exe`, with the PDB beside the exe);
  for 0.21d it is 99 MB.

## Game data

| Tool | Use |
|---|---|
| `tools/vostok-resources-db` (not in this repository, see above) | Unpacks `resources.db` |
| `common/dbgrab/` | `dbgrab <resources.db> list [substring]` and `get <substring> <out>` (Rust; needs the unpacker checked out as `tools/vostok-resources-db`) |
| `scripts/binary-config-cli` in each server folder | `print-binary-config -i <file>` prints a binary config as a tree. **Its output is UTF-16**: pipe it through `common/scripts/u16.py <raw> <out.txt>` |
| `common/cfgdump/` | Another config printer, built on `srp-021d/crates/binary-config-parser` |
| `v021d/scripts/gen_item_params.py` | Made `srp-021d/crates/survarium/src/v021d/item_params.rs` from the dumped item configs |
| `srp-021d/resources/binary-config-examples/static_dictionary-021d.txt` | The item, perk, skill, booster and faction dictionary, as text |

The lobby and HUD of the game are Flash (`flash_movies/*` in `resources.db`);
JPEXS FFDec decompiles them.

## Logs, snapshots, crashes

| Tool | Use |
|---|---|
| `v021d/log-analysis/deep.py` | Per-resync analysis of a saved match-server log (0.21d server logs) |
| `v021d/log-analysis/snapdiff.py`, `pdump.py` | Split and diff 0.21d world snapshots (dumps from `SRP_SNAPSHOT_DIR`) |
| `v021d/log-analysis/loads.py`, `rejoins.py` | Loading times, rejoins |
| `common/scripts/mdmp.py` | Reads a client minidump (`Documents/survarium/*.dmp`) and symbolises the stacks. It needs a `syms.pkl` (address, name) list beside it, built from the function list of the build that crashed |
| `cargo run -p survarium --example snapdiff -- a.bin b.bin` in `srp-026g` | Prints a saved 0.26g world's player blocks and sections with their CRCs |

## Scripted clients for 0.100b

`v0100b/e2e/`: scripted UDP/TCP clients for the **0.100b** server (lifecycle,
combat, batteries, traps, chat, queue, presence, time sync, retransmit).
`e2e_dummy.py` signs in like the stock 0.100b client. They don't speak the
0.21d or 0.26g protocols; those servers have scripted clients in their own
test suites.

## The launcher and the updater (0.26g)

`tools/re-kit/launcher/scripts`: the scripts that read `survarium_launcher.exe`
and `survarium_updater.exe` of build 2777 (no PDB; the 0.21d ones have PDBs,
and their `dumpbin /DISASM` listings with symbol labels name the functions).
`lpe.py` (sections, imports), `strings_xref.py` (every string with its
references and the room after it), `annotate.py` and `show.py` (the listing
with strings, imports and the names of `names_026g.txt`), `sites.py` (every
patch site with its original bytes), `relocs.py` (whether a range holds a
relocation), `rsrc.py` (resources, the string tables by language); `upd_*.py`
do the same for the updater (message ids, states). The listings themselves
are made with `dumpbin -DISASM` and are not kept in the repository. What was
found: `srp-026g/docs/notes/launcher.md` and `launcher_updater.md`.
