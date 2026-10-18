# SRP BugTrap (Rust, Win32)

A local crash reporter for the native 32-bit Survarium client. The DLL is written
in Rust, with small x86 assembly helpers for the compiler ABI. It implements all
82 exported BugTrap names with their original ordinals and calling conventions.
The installed 0.26g Skhid client has been tested under Wine Staging 11.8 / DXVK.

On a handled crash it writes
`SRP-crash-YYYYMMDD-HHMMSS-mmm-pPID-tTID-N.dmp` **beside the game EXE**, then
zips it with recent logs to `SRP-crash-…-N.zip` (see Zipped dumps), before callbacks or the
dialog. Closing the dialog keeps the file. Existing files are
never overwritten. If the EXE folder is unwritable, it tries the Windows temporary
folder; the dialog shows the filename and folder in separate labeled fields,
or a failure message. Long folders use middle ellipsis. It uses static text
without selection or scrollbars. The layout follows the original C++
`IDD_SIMPLE_DLG`: an explicit 8-point Tahoma font with smoothing, white message panel, thin divider,
and gray footer with Open Discord and Close buttons. The panel is taller to fit
the dump details; the original Mail and More controls are omitted. The original
green beetle icon (`vostok/sources/BugTrap/res/Bug.ico`) is embedded in the DLL
and used in both the message panel and the localized Survarium window title.
Both button labels are centered horizontally and vertically. Open Discord uses
the same measured character advances for painting, avoiding the caption offset
seen under Wine.

Open Discord opens [SRP Discord](https://discord.gg/Pxa4prXfwu) in the default
browser, only when clicked. The dialog tells the player to find the named file
in the displayed folder and send it to **#bugreports**. This applies to both a
ZIP and a raw dump kept after compression failed. If saving failed, it asks
them to describe the crash in that channel instead. A browser-launch failure
shows the invite address and manual instructions.

The whole dialog (title, status, file/folder labels, instructions, buttons and
errors) is localized in English, Russian, Ukrainian, German and Polish. It
automatically uses the Windows user's UI language, including regional variants;
unsupported languages fall back to English. The game's `-localization=` option
does not override this. The startup notice uses the same automatic selection.
The dialog and its buttons have room for the translated text.

Dialog screenshots from Wine (client area): [English](crash-dialog.png),
[Russian](crash-dialog-ru.png), [Polish](crash-dialog-pl.png),
[Ukrainian](crash-dialog-uk.png).

Nothing is uploaded automatically. Email/upload entry points fail with
`ERROR_NOT_SUPPORTED`, and legacy support destinations are discarded. The DLL
imports `kernel32.dll`, `user32.dll`, `gdi32.dll` and `shell32.dll` (for the browser
link and the Documents folder of the start-up notice); it loads `dbghelp.dll` by absolute path from beside the EXE or the Windows
system directory when capturing a dump.

## Start-up notice: the game cannot save its files

The game keeps its settings and its log in the folder `Survarium` of the
player's Documents folder, and has no word for a folder Windows does not let
it write to: before client patch revision 4 it stopped two seconds after its
start, and since then it runs on with settings that are not saved
(`srp-026g/docs/notes/client_patch.md` section 3.10).

When the game names itself (`BT_SetAppName`, once, early in its start, on its
main thread), the DLL makes that folder if it is missing, as the game does,
and makes a file `skhid-write-test-<process id>.tmp` in it that is gone when
its handle closes. If either is refused, one message box says so before the
game goes on, in the Windows user’s UI language (automatically;
English, Russian, Ukrainian, German, Polish; English for unsupported languages):

> **Survarium cannot save its files**
>
> Windows is blocking the game from saving files to this folder:
> *(the folder)*
>
> You can still play, but your settings (graphics, sound, controls) will not
> be kept for next time.
>
> To fix it:
> 1. Open the Start menu, type "Ransomware protection" and open it.
> 2. Click "Allow an app through Controlled folder access", then "Add an
>    allowed app", and choose: *(the game exe, with its folder)*
> 3. Start the game again.
>
> If this does not help, ask us in Discord: discord.gg/Pxa4prXfwu
>
> (Windows error *N*)

- The folder and the exe are the ones of that PC, asked of Windows at that
  moment; they are shown on the player's screen and go nowhere else.
- It is asked from inside the game because Windows' Controlled folder access
  judges each program by itself: only the game's own try says whether the
  game is let through. For the same reason the launcher, the updater and the
  service, which load this DLL from the same folder, are not asked (an exe
  whose file name holds `launcher`, `updater` or `service`): the box would
  name the wrong program and stay after the game was allowed.
- The box comes at every start for as long as the folder refuses the file,
  and never otherwise. Nothing is remembered: there is nowhere to write it.
- The names in quotation marks are Windows 11's in English. In the other four
  languages they are that window's labels as far as known when written; a
  reader with a Windows in that language should correct one that differs
  (`src/notice_text.rs`).
- Tried on Windows 11 with the 0.26g game (2026-10-13): a Documents folder
  that refuses new folders, and a folder `Survarium` that refuses new files
  (a deny entry in its access list): the box in each of the five languages,
  the game goes on after OK; an ordinary folder: no box. Not tried: Windows'
  Controlled folder access itself, Wine, the launcher's own start.

## Zipped dumps

A dump is about 160–190 MB (it holds the game's data segments); zipped, it is
2–4 MB, small enough for Discord.

- When: right after the dump is written, flushed and closed, on the report
  worker, before callbacks and the dialog; for crashes and for
  `BT_SaveSnapshot[Ex]` alike, and also for a stack overflow (no dialog).
- Files: `SRP-crash-…-N.zip` beside its `SRP-crash-…-N.dmp` (the same folder:
  the EXE's or the temporary one), holding the dump under its own name and recent game logs under `logs/`
  (DEFLATE; Unicode log names use the ZIP UTF-8 flag). It is written as
  `SRP-crash-…-N.zip.part`, never over an existing file, and renamed to `.zip`
  when complete.
- Recent logs: up to 32 newest matching files modified in the last 48 hours,
  from fixed locations (never from an arbitrary working directory or recursive
  scan): Windows Documents/Survarium and the game EXE's folder. For the standard
  `game/binaries/x86` installation layout, also `game/`, the installation root,
  and its `temp/` folder. Matching is case-insensitive: `Survarium.log`,
  `Survarium_*.log`, `Survarium.journal`, `Survarium_*.journal`,
  `updater_skhid.log`, `launcher_skhid.log`, and `launcher.log`. This covers the
  game's and original launcher's username-suffixed logs, game journals, and
  DXVK's `survarium_skhid_d3d11.log` / `survarium_skhid_dxgi.log`. Updater patch
  transaction journals, settings, screenshots, other extensions and unrelated
  files are excluded. Directories and reparse points are excluded when Windows
  reports them as such; Wine may expose host symlinks as ordinary files.
- Bounds and omissions: logs are captured **whole**, preserving startup
  diagnostics, the intervening session and crash context. There is no per-file
  raw-size truncation and no 16-MiB raw-total limit. Instead, up to 32 recent
  sources may contribute **16 MiB of compressed ZIP bytes**, including each
  log's local header, compressed data and central-directory record. Streaming
  writes enforce that bound before a block reaches `WriteFile`, rather than
  compressing an oversized source completely and checking afterward.
  The dump and a separately bounded 32-KiB collection summary are outside that
  log-byte allowance. Original logs are never changed or deleted.
- Work limit: at most 256 MiB of source bytes are read across all attempts,
  including reads from a source whose ZIP member is later rolled back.
  A source larger than the remaining input allowance is omitted before reading.
  A source exceeding the compressed, input or time budget is omitted whole;
  no head/tail fragments or retry are substituted. Failed output is removed,
  and later small files remain eligible when time and input budget permit.
  Each location has its own ZIP subfolder (`logs/documents/`, `logs/game-exe/`,
  etc.) so identical filenames cannot collide. `logs/collection.txt` records
  each included source's full size and ZIP contribution, omission reasons,
  processed-input totals, and per-folder missing/unavailable status, stale
  counts, scan errors and files omitted by the count limit. The summary is
  included even when no logs are available.
- Live files: shared reads capture the active log when its writer permits them.
  A size snapshot bounds concurrent appends; a shrinking or unreadable file is
  skipped atomically: its partial ZIP member is removed on any read failure.
  Missing or locked logs do not discard the dump ZIP. Collection gets up to five seconds of the
  archive's 60-second deadline. OS file calls are synchronous, so a blocked
  disk/network call can exceed that deadline. Buffered bytes not yet written
  by the game cannot be recovered; no game logging lock is acquired.
- Checked before the dump goes: the whole ZIP is read back; its records must
  be the ones written and each member must inflate to its recorded size and
  CRC-32, refusing what zlib refuses (such as a Huffman code with unused
  bit patterns). Then the `.dmp` is deleted and the dialog names the `.zip`.
- On any failure (no memory, disk full, a file of that name, a dump of 4 GiB
  or more, more than 60 seconds): the `.part` file is deleted, the `.dmp` is
  kept and the dialog names it as before. A crash or forced exit while zipping
  also keeps the `.dmp` (and may leave a `.part` file).
- Cost: a 166 MB dump took 1.3 s to zip and check under Wine (i686 build), to
  2.4–3.3 MB. The DEFLATE writer (`src/zip.rs`, no dependency) does greedy
  matching with 8 candidates per position, and dynamic Huffman blocks of 256 KiB
  of input, or stored ones where those would be larger. It uses 2 MB of fresh
  pages (`VirtualAlloc`), not the game's heap.
- A concurrent crash on another thread waits for the ZIP as it waits for the
  dump (but not for callbacks or the dialog).

## Install

Use the 0.26g player overlay, or build the DLL for manual installation. Keep a
backup of the game's original `bugtrap.dll` and replace it with the built DLL
under that filename. The DLL is linked without a PDB, so that every build of
the same source is the same file (see Build); a crash inside BugTrap itself is
read from its addresses.
An EXE rebuild is unnecessary. This build
supports the native ANSI Win32 interface; x64 and BugTrapN/managed UI are outside
its scope. Native Windows testing is still outstanding.

## Capture behavior

- Uses the original faulting thread ID, exception record and register context.
- Runs the writer on a fresh 1 MiB stack, including for stack overflow. The
  overflow path skips callbacks and UI afterward.
- Captures despite `MiniDumpNoDump`, disabled report flags or silent activity.
- Keeps the game's `BT_SetUserMessage` in the dump's comment stream.
- Serializes its own captures without acquiring a configuration or logging lock.
  Concurrent crashing threads wait until the primary dump is closed and zipped,
  then apply their exit mode without waiting for callbacks or the dialog. Recursive captures
  on the owner/writer thread and overlapping snapshot requests return busy. A crash
  during a manual snapshot waits for it, then captures a separate crash report
  with the faulting thread context and normal callbacks/dialog.
- Synthetic SEH/C++/native .NET entry points propagate an exception to an outer
  native handler in `BTEM_CONTINUESEARCH` mode, and return in `BTEM_EXECUTEHANDLER`
  mode. Propagation happens after the Rust capture frames have returned.
- When Wine DbgHelp emits an empty module stream, collects modules from a snapshot
  of the current process and fills the standard module records, paths, addresses,
  timestamps and CodeView/PDB identities. Existing nonempty streams are preserved.
  A supplemental-write failure keeps the primary dump and reports it as incomplete.

This is an in-process handler. Forced termination, fail-fast paths that bypass
exception filters, loader deadlocks, severe corruption, unavailable DbgHelp, or
unwritable/full storage can prevent capture. It cannot guarantee a dump for those
cases. See Microsoft's [MiniDumpWriteDump guidance](https://learn.microsoft.com/en-us/windows/win32/api/minidumpapiset/nf-minidumpapiset-minidumpwritedump)
and [module record format](https://learn.microsoft.com/en-us/windows/win32/api/minidumpapiset/ns-minidumpapiset-minidump_module).

## API scope

| Legacy interface | Behavior |
|---|---|
| SEH/C++ filters, install/uninstall, exit mode | Implemented; repeated installation restores our filter after CRT startup |
| `BT_NetFilter` / `BT_CallNetFilter` | Native context capture; no managed stack processing |
| App name/version, user message, intro messages, flags, dump type, report format/path | Setters/getters retained; local capture policy and SRP dialog remain fixed |
| Pre/post handlers | Invoked on the report worker after the dump closes; skipped on stack overflow |
| `BT_SaveSnapshot[Ex]` | Writes an automatically named local dump with the same folder policy, zipped as a crash dump is; the legacy filename argument is ignored |
| Email, uploads, registry export, version-resource reading, tracing/log APIs, import-table interception | Unsupported; operations with failure results return failure; applicable calls set `ERROR_NOT_SUPPORTED` |
| Legacy attached log/archive list | Empty; recent game logs are collected independently for the local ZIP |

There is no XML report generation, arbitrary caller-selected attachments, screenshot capture,
restart-after-crash, advanced BugTrap UI or mail integration. The zipped dump is
the report. Settings strings
are immutable process-lifetime copies (up to 32,767 bytes each), so getter pointers
stay valid after another setter; configuration calls allocate, capture does not
allocate through Rust (zipping takes its memory from `VirtualAlloc`).
Win32/DbgHelp may allocate internally. Callers must supply valid native pointers and coordinate configuration changes.

## Build

Use the same nightly Rust environment as the game workspaces;
`srp-026g/flake.nix` supplies it with `rust-src`. The build also needs a host C
linker for Cargo build scripts and `rust-lld` from that toolchain. No Windows
SDK or proprietary compiler is required for the DLL when cross-building on Linux.
The two locked dependencies are Microsoft's `windows-sys` and `windows-link`.

```sh
nix develop ./srp-026g
cd client/bugtrap
./build.sh
```

This uses `-Z build-std=core` and the `i686-pc-windows-msvc` target. Output:
`target/i686-pc-windows-msvc/release/BugTrap.dll`.

The build is reproducible: the same source and toolchain give the same DLL,
byte for byte, in any folder. `rust-lld` gets `/Brepro` (the PE stamp is a
hash of the content, not the time) and `/DEBUG:NONE` (no PDB: a PDB records
the linker's command line, which for a DLL names a temporary folder of a
random name, and the build folder, and the DLL's stamp and PDB identity would
follow them). `../check-reproducible.sh` builds BugTrap and the updater twice,
in two folders, and compares.
It has no Rust standard-library runtime or C runtime DLL dependency. Unsafe code
is confined to the native ABI, Win32 calls, fixed crash state and compiler helpers;
Rust cannot make an already-corrupted game's memory safe.

## Verify

The C++ programs in `tests/` are independent ABI/crash callers, not part of the
shipping DLL. From an x86 Visual Studio developer prompt with `lld-link` on PATH,
run `tests\build.cmd` to build them. VS2008 was used for validation.

On Linux with Wine and an X server (Xvfb works), use an isolated test prefix:

```sh
python3 tests/check_abi.py target/i686-pc-windows-msvc/release/BugTrap.dll
WINEPREFIX=/path/to/test-prefix python3 tests/check_dumps.py target/i686-pc-windows-msvc/release
rustc --edition 2024 --test src/arithmetic.rs -o target/arithmetic-test
./target/arithmetic-test
rustc --edition 2024 --test tests/text_tests.rs -o target/text-tests
./target/text-tests
rustc --edition 2024 -O --test src/zip.rs -o target/zip-test
./target/zip-test   # SRP_ZIP_SAMPLE=<a dump> zips that file instead
```

The text tests cover automatic Windows UI language selection, regional variants,
English fallback and the startup notice.
For the dialog itself, `tests/build.cmd` also builds `ui_probe.exe`. On Linux
it can be built with `i686-w64-mingw32-g++ -static -O2 tests/ui_probe.cpp
-o target/i686-pc-windows-msvc/release/ui_probe.exe -luser32 -lgdi32` (with the
MinGW runtime libraries on the linker's search path). Run under Wine/Xvfb:

```sh
WINEPREFIX=/path/to/test-prefix python3 tests/check_localization.py target/i686-pc-windows-msvc/release
```

This checks all five languages on real Win32 dialogs: ZIP, raw-dump fallback,
partial report, failed save, failed browser launch and failed dialog creation,
plus automatic locale selection with absent or conflicting game-language options.
It measures text against control bounds, checks translated labels
and buttons, and intercepts the browser call so no browser or network is used.
An optional second argument saves dialog screenshots as BMPs for inspection.

The ZIP tests round-trip empty, tiny, zero, repetitive, random and dump-like
inputs across block and read boundaries, check the CRC-32, the code-length
limits and that damaged or truncated streams are refused, that the check
refuses the incomplete codes zlib refuses and reads zlib's fixed codes, and have
Python's `zipfile` read a 40 MB ZIP made as the DLL makes it (when `python3` is
on PATH).

Every crash case reads its dump out of the `.zip` with `zipfile` (CRC checked,
one dump member named like the ZIP, not UTF-8 flagged; up to 32 whole log members and the collection summary) and checks that no `.dmp` or
`.part` file is left. The twenty crash cases verify exception codes and thread
IDs, module presence, the game error comment, callback ordering, stack overflow, dialog closure, repeated
crashes, Unicode paths, a different working directory, read-only EXE fallback,
and explicit failure when neither folder is writable. They also cover all three
synthetic entry points in continue-search/execute-handler modes, recursive worker
callbacks, and a second crash while a test-only DbgHelp shim holds the primary
writer behind a gate. The concurrency case verifies that the dump survives and
the second crash does not wait for the primary callback. A snapshot-overlap case
checks that a real crash saves its own exception stream and runs its callback
after the snapshot finishes. The zip-blocked case puts a folder in the place of
the temporary ZIP while the test DbgHelp holds the writer, and checks that the
`.dmp` is kept and valid. Run as a normal user so
permission tests are effective. The Discord case intercepts only the browser
API call and checks the exact invite, absence of automatic launches, and that
the dialog and dump survive the click; it does not contact Discord. The ABI check also rejects runtime/network import
changes and checks all 82 legacy ordinals.

Log collection has a separate integration test. Build `tests/logs_probe.cpp`
with MinGW (`i686-w64-mingw32-g++ -static -municode -O2 tests/logs_probe.cpp
-o target/i686-pc-windows-msvc/release/logs_probe.exe -ladvapi32`) or MSVC
(`cl tests\logs_probe.cpp advapi32.lib`). Then run:

```sh
WINEPREFIX=/path/to/disposable-prefix python3 tests/check_logs.py target/i686-pc-windows-msvc/release
```

This test redirects **that disposable prefix's Documents folder** to temporary
fixtures. It checks absent logs, Documents and game-folder coverage, age and
filename filtering, Unicode paths/names, journals, manifest outcomes, writer
sharing and exclusive locks, injected read/time failures with whole-source
rollback and input accounting, directory exclusion and unchanged originals.
Two compressible logs larger than 16 MiB each are recovered byte-for-byte.
An incompressible source exceeds the ZIP cap, while a `WriteFile` hook checks
that streaming writes never cross it; a later small source still fits.
A sparse source larger than 256 MiB is omitted before reading. The tests also
check the 32-file bound and count omitted candidates.
Python `zipfile` checks every member and its CRC.

For an **idle, disposable installed 0.26g client** with `survarium_skhid.exe`:

```sh
python3 tests/game_test.py /path/to/game/binaries/x86 \
  --wine /path/to/wine \
  --prefix /path/to/game-prefix \
  --output "$PWD/target/game-test-026g"
```

The harness temporarily replaces BugTrap and adds a D3D11 forwarding test DLL.
After the game window appears and 15 seconds elapse, it triggers a real null write
inside the client, checks the dialog, closes it and parses the dump inside the
persistent ZIP.
It validates the original exception/register context, fault module, game module
and that BugTrap is in the module list. It restores the original BugTrap and removes both test
DLLs in `finally`, checks the EXE and earlier dump hashes, and keeps evidence in
`--output`. Its server address is loopback. Do not use this against a running
play session. The test does not change host ptrace policy.

The existing player-overlay builder includes the built DLL. From the repository
root, create the player ZIP with:

```sh
python3 srp-026g/client-patches/make_client_026g.py /path/to/game \
  target/Survarium-0.26g-srp-files.zip --pack-zip --server=026g.skhid.link:1234
```

It contains the patched EXE, `bugtrap.dll`, launcher, login certificate and player
README. Symbols, sources, tests and crash dumps stay outside the player ZIP.
