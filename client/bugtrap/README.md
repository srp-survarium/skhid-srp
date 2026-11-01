# SRP BugTrap (Rust, Win32)

A local crash reporter for the native 32-bit Survarium client. The DLL is written
in Rust, built as an ordinary Windows DLL: Rust's standard library and
Microsoft's C runtime, linked in (`+crt-static`, as `cl /MT`), and a version
resource (Skhid, "Skhid crash reporter", the release revision). It implements all
82 exported BugTrap names with their original ordinals and calling conventions.
The installed 0.26g Skhid client has been tested under Wine Staging 11.8 / DXVK.

On a handled crash it writes
`SRP-crash-YYYYMMDD-HHMMSS-mmm-pPID-tTID-N.dmp` **beside the game EXE**, then
zips it with recent logs to `SRP-crash-…-N.zip` (see Zipped dumps), before callbacks or the
dialog. Closing the dialog keeps the file. Existing files are
never overwritten. If the EXE folder is unwritable, it tries the Windows temporary
folder; the dialog shows the filename and folder in separate labeled fields,
or a failure message. Long folders use middle ellipsis. It uses static text
without scrollbars; only the crash details (see Crash details) can be
selected. The layout follows the original C++
`IDD_SIMPLE_DLG`: an explicit 8-point Tahoma font with smoothing, white message panel, thin divider,
and gray footer with Open Discord, Copy and Close buttons. The panel is taller to fit
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

Dialog screenshots from Wine (client area, the probe's synthetic crash):
[English](crash-dialog.png), [Russian](crash-dialog-ru.png),
[Polish](crash-dialog-pl.png), [Ukrainian](crash-dialog-uk.png); a capture
an antivirus refused, [English](crash-dialog-blocked.png) and
[Russian](crash-dialog-blocked-ru.png); a game folder with Cyrillic
letters, [Russian](crash-dialog-folder-ru.png).

## Crash details

Every crash also gets a short text report, made from the exception and
context records whether or not the dump is saved:

```
Skhid crash report, 2026-10-25 18:44:17
Exception: 0xC0000005 ACCESS_VIOLATION (write to 0x00000000)
Address:   report_probe.exe+0x1444 (0x00401444)
Thread:    284
Registers: EAX=00000000 EBX=009F7F48 ECX=00000000 EDX=00000000 ESI=00000000 EDI=7BEBD4B4
           EBP=0032FED0 ESP=0032FE6C EIP=00401444 EFL=00010246
Stack:     report_probe.exe+0x14D4
           msvcrt.dll+0x3F56D
           ...
Game:      report_probe.exe, patch revision unknown, sha256 6ac354ea1c2e
Reporter:  local build
Windows:   10.0.19045 (Wine 11.8)
Dump:      saved
Game log:  last 20 lines of the newest Survarium*.log in Documents\Survarium
...
```

- Exception: the code, its Windows name, and for an access violation
  whether it read, wrote or executed and the address; for an MSVC C++
  exception the thrown type (`.?AVbad_alloc@std@@`).
- Address and Stack: module+offset, the module's file name from Windows
  (`survarium_skhid.exe+0x2D1E20`); names are resolved on our side with
  `tools/re-kit/v026g/lookup.py`. The stack is a scan, not an unwind: up to
  8 distinct values on the faulting stack that point just past a `call` in
  a loaded module's code, the first usually the caller. Stale return
  addresses can appear.
- Game: the exe's name, the patch revision its sign-in sends (read from the
  loaded image where `client/updater/logic/src/revision.rs` reads the file),
  the first 12 hex digits of the exe file's SHA-256. Reporter: the
  `SKHID_REVISION` this DLL was built for. Windows: `RtlGetVersion`, and
  Wine's version under Wine.
- Paths (only when it applies): non-ASCII letters in the game folder or the
  temporary folder, and whether they are outside the ANSI code page. The
  folders themselves are not written.
- Dump: saved, or why not, with the Windows error.
- Game log: the last 20 lines of the newest `Survarium*.log` in
  Documents\Survarium (its last 8 KiB), from the first line that starts a
  record (a cut inside a sensitive record's continuation lines would not be
  caught), through the same credential filter as the ZIP's logs; the file
  name is not written (it holds the account name).
- Every read of the game's memory goes through `ReadProcessMemory`: a
  damaged pointer gives no line, not a second crash. The stack scan reads
  whole dwords only and stops at a short read. The report is made only after
  the dump is closed, so nothing in it can cost the dump.

Where it goes: `SRP-crash-…-N.txt` beside the dump (the same name), and into
the ZIP as `crash.txt`; with no dump, as a new `SRP-crash-…-N.txt` beside the
exe or else in the temporary folder, and the dialog names it in the dump's
place; when neither takes it, it is only shown.

The dialog shows the first two lines (code, name, module+offset) in a
selectable field. **Copy** puts everything above the game log on the
clipboard as a Discord code block (about 1 KB) and says "Copied"; without a
clipboard it selects the field for Ctrl+C.

The first line of the dialog tells why a dump is missing:

- the file could not be made or written (folder, permission, a full disk):
  "Could not save a crash dump beside the game or in the temporary folder.";
- the file was made, but `MiniDumpWriteDump` could not read the game
  (an HRESULT such as `0x8007001F` or `0x8007012B`, access denied on the
  process: an antivirus, EDR or Windows' exploit protection): "An antivirus
  or Windows security feature blocked the crash dump. The crash details below
  are still useful: copy them to #bugreports."

Windows errors of 0x10000 and above are shown in hex. A game folder with
non-ASCII letters adds a line: "The game is in a folder with non-English
letters (…). Move it to a simple path like C:\Games\Survarium and start it
from there." (the folder shortened in the middle to 60 characters).
All file and folder calls of this DLL are the Windows wide (W) calls.

Nothing is uploaded automatically. Email/upload entry points fail with
`ERROR_NOT_SUPPORTED`, and legacy support destinations are discarded. The DLL
imports `kernel32.dll`, `user32.dll`, `gdi32.dll`, `shell32.dll` (for the browser
link and the Documents folder of the start-up notice) and `ntdll.dll` (two
functions of the standard library's), no C runtime DLL; it loads `dbghelp.dll` by absolute path from beside the EXE or the Windows
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

## Dump contents

`src/dump.rs`, `DUMP_TYPE`:

- **Players' build:** `MiniDumpWithDataSegs | MiniDumpWithIndirectlyReferencedMemory`
  (0x41): every thread's stack and registers, the modules, their writable data
  sections, and the memory the stacks point to. Its memory callback also adds,
  for the crashing thread's EAX, EBX, ECX, EDX, ESI, EDI and EBP, the range
  `[value - 256, value + 2048)` cut to the committed readable region holding
  it: the object behind a faulting `mov eax, [esi+116h]`, which no stack slot
  need point to (at most 7 × 2.3 KiB). A range dbghelp cannot read is skipped,
  not a failed dump. Before 2026-11-01 it was DataSegs alone: no heap at all.
- **Local test build** (`./build.sh --features full-memory-dumps`):
  `MiniDumpWithFullMemory | FullMemoryInfo | HandleData | ThreadInfo |
  UnloadedModules` (0x1826): the whole process memory, every heap object.
  About the game's committed memory (1–3 GB); kept as the `.dmp` beside the
  exe, not zipped (zipping would hold the crash for many seconds, and the dump
  is read on that PC), the text report beside it. Crash details say
  `Reporter:  local build, full-memory dumps`. Never released: `build.rs`
  refuses the feature with `SKHID_REVISION`, which every release build sets
  (`release/build-zip` builds a fresh worktree with plain `build.sh`).
- **Wine's dbghelp** (Linux players and the local clients, which have no
  `dbghelp.dll` beside the exe) ignores DataSegs, IndirectlyReferencedMemory
  and memory callbacks: a players' dump written under Wine has stacks,
  registers and modules only (about 0.5–1 MB, 50 KB zipped). Full memory works
  under Wine (`tests/check_memory.py`). Microsoft's own dbghelp/dbgcore does
  not run under Wine 11.8 (`RtlAcquirePrivilege` is missing: the capture fails
  with 0x80070005), so the Windows sizes below are from players' reports, not
  measured here.
- Reading one: `python3Packages.minidump`, or WinDbg (`dd <address>`); a full
  dump answers for any heap object.

## Zipped dumps

A Windows dump is about 160–190 MB (it holds the game's data segments, mostly
zeros); zipped, it is 2–4 MB, small enough for Discord (10 MB). The memory the
stacks and registers point to is expected to add little (pages near stack
pointers; not yet measured on Windows: see Dump contents).

- When: right after the dump is written, flushed and closed, on the report
  worker, before callbacks and the dialog; for crashes and for
  `BT_SaveSnapshot[Ex]` alike, and also for a stack overflow (no dialog).
- Files: `SRP-crash-…-N.zip` beside its `SRP-crash-…-N.dmp` (the same folder:
  the EXE's or the temporary one), holding the dump under its own name, the text report as `crash.txt` (see Crash details) and recent game logs under `logs/`
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
- Legacy code pages: the game writes its log in its locale's code page. The
  engine's start-up (`core::preinitialize`, 0.26g 0xE3C10) sets it from
  `-localization`: original and russian cp1251, english cp1252, polish cp1250;
  any other value leaves the C locale (Latin-1 bytes). Its audio device names
  (`%S`) and other localized text are therefore not UTF-8 on Russian,
  Ukrainian or Polish Windows ("Динамики", "Głośniki"), and the whole game log
  used to be omitted. A line that is not UTF-8 but has no NUL or control bytes
  is now taken as text of that code page (from the game's `-localization` on
  its command line; without one the option's default, original: cp1251) and transcoded to
  UTF-8 (`MultiByteToWideChar` with `MB_ERR_INVALID_CHARS`, then
  `WideCharToMultiByte`) before the checks below. `collection.txt` names the
  code page and counts each source's transcoded lines
  (`transcoded_lines=2 from cp1251`). The crash details' log tail (crash.txt)
  still replaces such letters with `?`.
- Credential filtering: before compression, UTF-8/ASCII text is checked one
  complete line at a time (64 KiB maximum, also after transcoding), including
  labels split across disk reads. Records containing known password/passwd/pwd, account-digest,
  authorization/bearer/basic-auth, token, secret, API/access/session-key, cookie,
  credential, command-line or URL-userinfo markers are replaced with a fixed
  removal notice. Following unframed lines are removed until the next native
  `channel <level>\t[HH:MM:SS.mmm]\t` (also with a colon before the milliseconds, `[HH:MM:SS:mmm]`) or updater `YYYY-MM-DD HH:MM:SS ` record.
  This deliberately favors omission over retaining ambiguous continuations.
  Unclosed quotes/trailing backslashes in sensitive lines, oversized lines,
  text that is neither UTF-8 nor of the legacy code page, UTF-16/32,
  NUL/binary/control bytes omit the entire source. Diagnostics around removed
  records remain available; binary journals can therefore be omitted.
  This detects **known credential-bearing records**, not arbitrary unlabelled
  secrets, unusual obfuscation, or a multiline value that impersonates a valid
  new record. **Minidump memory is not sanitized** and may contain credentials;
  review the complete report before sharing it publicly.
- Bounds and omissions: accepted text is captured without size truncation,
  preserving startup diagnostics, the intervening session and crash context
  except records removed by the credential filter. There is no per-file
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
  etc.). Entries use anonymous `source-01.txt` IDs, never original filenames or
  full paths, so a filename containing a username or secret cannot echo into
  ZIP metadata or the summary. `logs/collection.txt` records each source's
  original size, sanitized size, removed-line count and ZIP contribution,
  omission reasons,
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

Use the dev shell of `srp-026g/flake.nix`: the nightly Rust of the game
workspaces with `rust-src`, `rust-lld`, `llvm-rc` (the version resource) and
Microsoft's C runtime and import libraries for 32-bit x86
(`SKHID_MSVC_LIB`: libcmt, libvcruntime, libucrt of the Visual Studio 2022
build tools 14.44.17.14 and the Windows SDK 10.0.26100, fetched by `xwin` at
those pinned versions, its output pinned by hash; the flake says more). The
build also needs a host C linker for Cargo build scripts. No Windows or
Visual Studio installation is needed.
The two locked dependencies are Microsoft's `windows-sys` and `windows-link`.

```sh
nix develop ./srp-026g
cd client/bugtrap
./build.sh                       # a local build: version 0.0.0.0, "local build"
SKHID_REVISION=27 ./build.sh     # as release/build-zip builds revision 27
./build.sh --features full-memory-dumps   # local tests only: whole-memory dumps
```

The target is `i686-win7-windows-msvc` (Windows 7 and later, as the game; the
standard library of `i686-win7-windows-msvc` asks for Windows 10), its standard
library built here with `-Z build-std=std,panic_abort`. Output:
`target/i686-win7-windows-msvc/release/BugTrap.dll`. `DllMain` is called by the
C runtime's `_DllMainCRTStartup`, which sets the runtime up first (no thread,
no library load); `DllMain` then does what it always did and makes the panic
hook end the game with `0xE0000001`, as the panic handler of the `no_std` DLL
did. The version resource (`res/BugTrap.rc`, `build.rs`,
`../windows_build.rs`): CompanyName and ProductName `Skhid`, FileDescription
`Skhid crash reporter`, FileVersion `<revision>.0.0.0`, ProductVersion
`revision <revision>` (a local build: `0.0.0.0`, `local build`),
LegalCopyright, InternalName `BugTrap`, OriginalFilename `BugTrap.dll`.

The build is reproducible: the same source, toolchain and `SKHID_REVISION` give
the same DLL, byte for byte, in any folder. `rust-lld` gets `/Brepro` (the PE
stamp is a hash of the content, not the time) and `/DEBUG:NONE` (no PDB: a PDB
records the linker's command line, which for a DLL names a temporary folder of
a random name, and the build folder, and the DLL's stamp and PDB identity would
follow them); Cargo's `trim-paths` keeps the build folder, the home folder and
the toolchain's store path out of the panic locations
(`.cargo/config.toml`). `../check-reproducible.sh` builds BugTrap and the
updater twice, in two folders, and compares.
Unsafe code is confined to the native ABI, Win32 calls and fixed crash state;
Rust cannot make an already-corrupted game's memory safe.

## Verify

The C++ programs in `tests/` are independent ABI/crash callers, not part of the
shipping DLL. From an x86 Visual Studio developer prompt with `lld-link` on PATH,
run `tests\build.cmd` to build them. VS2008 was used for validation.

On Linux with Wine and an X server (Xvfb works), use an isolated test prefix:

```sh
python3 tests/check_abi.py target/i686-win7-windows-msvc/release/BugTrap.dll
WINEPREFIX=/path/to/test-prefix python3 tests/check_dumps.py target/i686-win7-windows-msvc/release
rustc --edition 2024 --test tests/text_tests.rs -o target/text-tests
./target/text-tests
rustc --edition 2024 -O --test src/zip.rs -o target/zip-test
./target/zip-test   # SRP_ZIP_SAMPLE=<a dump> zips that file instead
```

What a dump holds (Dump contents) has its own check, no X server needed, run
once with a players' build and once with a `full-memory-dumps` build. Build the
probe and the test dbghelp with MinGW
(`nix shell nixpkgs#pkgsCross.mingw32.buildPackages.gcc`), into a folder with
that `BugTrap.dll`:

```sh
i686-w64-mingw32-gcc -O2 -DPROBE_NO_CRT -nostdlib -Wl,-e,_probe_start@0 \
  tests/memory_probe.c -lmsvcrt -lkernel32 -o <folder>/memory_probe.exe
i686-w64-mingw32-gcc -O2 -D__USE_MINGW_ANSI_STDIO=0 -shared -nostdlib -Wl,--kill-at \
  -Wl,-e,_DllMainCRTStartup@12 tests/callback_dbghelp.c -lkernel32 -lmsvcrt \
  -o <folder>/callback_dbghelp.dll
WINEPREFIX=/path/to/test-prefix python3 tests/check_memory.py <folder>
```

It checks the dump type BugTrap asks for, the register ranges its callback
gives (through the test dbghelp, which records them and forwards to Wine's),
and in Wine's dump: no unreferenced heap object in a players' dump, all three
probe objects in a full one, and which file is kept (ZIP or `.dmp`).

The text tests cover automatic Windows UI language selection, regional variants,
English fallback and the startup notice, and the crash details: exception
names and parameters, module+offset, hex error codes, the dump-failure message
by where the error came from, the stack scan's call test, the log tail and
the non-ASCII folder line.
For the dialog itself, `tests/build.cmd` also builds `ui_probe.exe`. On Linux
it can be built with `i686-w64-mingw32-g++ -static -O2 tests/ui_probe.cpp
-o target/i686-win7-windows-msvc/release/ui_probe.exe -luser32 -lgdi32` (with the
MinGW runtime libraries on the linker's search path). Run under Wine/Xvfb:

```sh
WINEPREFIX=/path/to/test-prefix python3 tests/check_localization.py target/i686-win7-windows-msvc/release
```

This checks all five languages on real Win32 dialogs: ZIP, raw-dump fallback,
partial report, failed save, a capture refused as an antivirus refuses it
(`MiniDumpWriteDump` failing with 0x8007001F), the game in a folder of
Cyrillic and Polish letters, failed browser launch and failed dialog creation,
plus automatic locale selection with absent or conflicting game-language options.
It measures text against control bounds, checks translated labels
and buttons, and intercepts the browser call so no browser or network is used.
In each, it presses Copy and reads the clipboard back (the code block, no
log lines), and checks the text report beside the dump, or in its place.
An optional second argument saves dialog screenshots as BMPs for inspection.

The ZIP tests round-trip empty, tiny, zero, repetitive, random and dump-like
inputs across block and read boundaries, check the CRC-32, the code-length
limits and that damaged or truncated streams are refused, that the check
refuses the incomplete codes zlib refuses and reads zlib's fixed codes, and have
Python's `zipfile` read a 40 MB ZIP made as the DLL makes it (when `python3` is
on PATH).

Every crash case reads its dump out of the `.zip` with `zipfile` (CRC checked,
one dump member named like the ZIP, not UTF-8 flagged; up to 32 filtered log members and the collection summary) and checks that no `.dmp` or
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
-o target/i686-win7-windows-msvc/release/logs_probe.exe -ladvapi32`) or MSVC
(`cl tests\logs_probe.cpp advapi32.lib`). Then run:

```sh
WINEPREFIX=/path/to/disposable-prefix python3 tests/check_logs.py target/i686-win7-windows-msvc/release
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
check the 32-file bound and count omitted candidates. Synthetic passwords,
account digests, bearer/tokens, command lines, URL credentials and continuation
values are absent from extracted log members, member names and the summary;
startup memory diagnostics remain. UTF-16/binary, oversized and ambiguous
multiline sources are omitted atomically; original files are unchanged.
cp1251, cp1250, cp1252 and Latin-1 logs (by `-localization=russian`,
`original`, `polish`, `english`, an unknown one and none (the default,
original) on the probe's command
line) come out as UTF-8 with their transcoded lines counted; a control byte
in a cp1251 line still omits the source, and a cp1251 credential record is
still removed with its continuation.
A short successful read followed by premature EOF must also roll back the
source even though filtered output has a different size. Host-only boundary,
encoding and read-failure tests run with:

```sh
rustc --edition 2024 --test tests/sanitize_tests.rs -o /tmp/sanitize-tests
/tmp/sanitize-tests
```
Python `zipfile` checks every member and its CRC.

The text report of a real, unhandled crash: build `tests/report_probe.cpp`
(`i686-w64-mingw32-g++ -static -municode -O2 tests/report_probe.cpp -o
target/i686-win7-windows-msvc/release/report_probe.exe -ladvapi32`), then

```sh
WINEPREFIX=/path/to/disposable-prefix python3 tests/check_report.py target/i686-win7-windows-msvc/release
```

It redirects that prefix's Documents to a folder with a game log, crashes
with a null write two calls deep, with an uncaught MSVC-style
`std::bad_alloc`, and with a null write 400 KB down the stack, ESP 4-aligned
or not (the scan once hung there), and checks the report beside the ZIP and in it: the
exception line, the address inside the faulting function as module+offset,
the caller on the stack scan, the other lines, and the log tail with a
password's record and its continuation removed and the records after it kept.

Run on 2026-10-21 (Linux, Wine, Xvfb), the build with the standard library
and the C runtime linked in, the C++ probes built with `clang-cl` against the
same Microsoft libraries: `check_abi.py` passes (the 82 ordinals; `_fltused`,
which the `no_std` DLL exported by mistake, is gone); `check_dumps.py` 17 of
its 20 cases pass, and the three that use the test DbgHelp (`concurrent`,
`snapshot-overlap`, `zip-blocked`) fail with the same probe exit codes (85,
72) with the DLL built from `main` before this change (a test-environment
problem of this Wine and these probes, not the DLL's);
`check_localization.py` and `check_logs.py` pass; the text (4), sanitize (5)
and ZIP (7) tests pass.

Run on 2026-10-25 for the crash details (Linux, Wine Staging 11.8, Xvfb; the
probes built with MinGW g++ 15.3): `check_abi.py`, `check_localization.py`
(47 dialogs), `check_report.py` (4 crashes) and `check_logs.py` pass; the
text (10) and sanitize (7) tests pass; `client/check-reproducible.sh` gives the same DLL
twice. `check_dumps.py` was not run: its probe uses MSVC `__try`, which
MinGW does not build.

Run on 2026-10-26 for the legacy code pages (Linux, Wine Staging 11.8; the
probe built with MinGW g++ 15.3): `check_abi.py` and `check_logs.py` pass;
the text (10) and sanitize (9) tests pass.

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
