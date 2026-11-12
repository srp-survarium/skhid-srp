# Module reports (client patch 125)

Each game tells the match server what is loaded into it besides Windows' own DLLs, which DLLs
were loaded since its first instruction, which other programs' always-on-top windows
(overlays) are over it, and which other processes hold a handle to the game's own process with
access to its memory. The server writes it to a log; `release/moderate modules` reads the
log across all games.

## The rules

- **Only logged.** No automatic action, nothing in the db, nothing a player sees. Discord,
  Steam, RTSS/MSI Afterburner, OBS and DXVK put DLLs into the game and windows over it, so an
  entry is read against everyone else's (`release/moderate modules`).
- **Other processes: only whose window is over the game.** The game tells which processes
  have an always-on-top window over its own, by exe file name and folder, window class, styles
  and size. Such a process is opened only to ask Windows where its exe is, with the one right
  that can ask nothing else (`PROCESS_QUERY_LIMITED_INFORMATION`), and the handle is closed at
  once. Nothing else of another process is looked at: not its title, its other windows, what
  a window shows or its memory.
- **Privacy.** A module is told by its file name and where its folder is: below the game's
  folder, below the temporary one, or (any other) its path from the drive with a user's name
  left out (`C:\Users\*\Downloads\tools`; also Wine's `Z:\home\*`). At most 64 bytes of it.
  Modules in the Windows folder are not told.
- **When.** With a match connection's first world (`world_load.loaded`) and when the match
  closes on the game (`match_recovery.close`), once each, on the client-report channel
  (`srp_validate::report`, kind 2; `memory-report.md` describes the channel).
- **Each minute in between** (the owner, 2026-11-11: a window opened and closed again mid-match
  was never seen): a minute after the last look, a frame of the match (`game.tick`) looks at the
  windows over the game and the handle holders again. When it saw otherwise than the last look, a
  report of moment `minute <n>` (minutes since the first world, up to 254) tells those alone, so an
  overlay that stays the whole match is told once and an empty one says the windows are gone.
  The modules, loads and code are told only at the first world and the end.
- **Windows told as they happen, not only at the minute** (the owner's next step, 2026-11-12: the
  once-a-minute look leaves gaps). Besides the look, the watch asks Windows to tell it of the
  window events out of context (`SetWinEventHook`, `WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS`,
  for show / hide / destroy / reorder and foreground); the callbacks arrive on the game's main
  thread, which pumps messages. A window that comes over the game opens an episode, one that goes
  closes it, and the next look drains the closed episodes as `over` entries with the match seconds
  they were over for (`<since>-<until>s`), so a window over the game only between two looks is still
  told, with its times. A window still over at the look is told by the look (`<since>s+`, start not
  pinned). Event-driven: bounded room (16 episodes), counts not elapsed time.
- **What is told** (`srp_validate::modules`), the rare kinds first so a long module list cannot
  crowd them out; up to 144 entries a report, more are only counted:
  - **holds game memory** (handle holder): each other process that holds a handle to the game's own
    process object with memory access (`PROCESS_VM_READ`, and `VM_WRITE`/`VM_OPERATION` where
    granted): the exe's file name and folder (as a module's, a user's name left out), and the
    granted access. A recorder, an overlay or a debugger reads the game's memory this way;
  - **holds game memory unchecked**: a count, by holder exe, of the game's process-object handles
    the game could not check — the kernel zeroed the object address (Windows 11 24H2, a caller
    without the debug right) and the holder could not be opened to duplicate the handle, or it is
    elevated or protected. Never a silent gap;
  - **over**: each other process's window that is visible, always on top (`WS_EX_TOPMOST`), above
    the game's window in the windows' order and over some of it: the exe's file name and where
    its folder is (as a module's, a user's name left out; `windows` for the Windows folder, `?`
    where Windows does not tell), the window's class, its extended styles, how much of the game's
    window it covers, whether a screen capture leaves it out (`GetWindowDisplayAffinity`: shown,
    monitor-only, or hidden from capture), and the match seconds it was over for. Needs no right.
    Discord's, Steam's and a recorder's overlays, the taskbar and a volume pop-up are such windows
    too: read rarest first. The first 16 from the top. Where it cannot be told, one entry says why:
    the game has no window yet, or its window is not in the order;
  - **module**: each module of the loader's list: name, place, image size, the PE header's
    link time and checksum (a build of a DLL, not just its name);
  - **load**: each DLL the loader said it loaded after the game's start, with its place, size
    and PE marks as it was loaded, when, and whether the loader's list still has it. One that was
    loaded and then taken out of the list stays on record. The first 32;
  - **code**: each executable allocation that is no listed module (a DLL mapped by hand, JIT
    code, an overlay's trampolines): kind of memory, protection, sizes, a fingerprint of its
    first 256 executable bytes, the PE marks of a header at its start.
- **Times** are whole seconds since the game's start.

## How

- **The start.** The exe's TLS directory (0xA365A8) names a callback list of ours
  (`module_watch_callbacks`; the original's list was empty). Windows calls a TLS callback on the
  main thread before the exe's entry point, so before the C runtime, `bugtrap.dll` (which the exe
  loads itself, later) and anything loaded afterwards (and then on every new thread, which the
  watch does nothing with). Only the DLLs the exe imports run earlier; one of those replaced by a file
  in the game's folder shows in the module list by its folder.
- **The watch** (`vostok::module_watch`). At the first call it notes the time and the folders
  and registers ntdll's DLL notification (`LdrRegisterDllNotification`). The notification tells
  loads; the DLL's header is read then, mapped already. It comes with the loader's lock held,
  so it only keeps a record in fixed room.
- **Windows over the game.** The game's window is the one its swap chain last presented to
  (`render_output.present`), or the top-level window that one is in. From the top of the
  windows' order (`GetTopWindow`, `GetWindow` next) down to the game's own, each window is asked
  its process, whether it is visible, its extended styles, its rectangle, its class and whether a
  screen capture leaves it out (`GetWindowDisplayAffinity`, user32's own questions; no message is
  sent to it). The exe's path of each one found is asked with `QueryFullProcessImageNameW`. A
  window that is not always on top cannot stay above the game while it is played, so only those
  are told: what the player has open beside the game is not.
- **The window event watch** (`vostok::module_watch`). Besides the once-a-minute look, the watch
  registers `SetWinEventHook` out of context (`WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS`)
  for show / hide / destroy / reorder and foreground, from the game's main thread, which pumps
  messages — so the callbacks arrive there. Each event's window is tested as the look tests one
  (`over_now`); a window that comes over the game opens an episode (its description captured then),
  one that goes closes it (`EVENT_OBJECT_DESTROY`, a hide, or it is no longer over), and a look
  drains the closed episodes with the match seconds they were over for. The callback's address is
  our own entry (`module_watch_win_event`), handed up to Rust at the process's start the way the
  DLL notification's is, since the arena's symbols are not named from the safe layer.
- **Handle holders.** The system handle list is asked
  (`NtQuerySystemInformation(SystemExtendedHandleInformation)`, the buffer grown on
  `STATUS_INFO_LENGTH_MISMATCH`). The game's process-object type index and its own object address
  are learnt from a handle the game opens to itself; another process's entry of that type is the
  game's by its object address where the kernel gives one, else by duplicating the handle into the
  game (`OpenProcess(PROCESS_DUP_HANDLE)`, `DuplicateHandle`, `GetProcessId == own pid`, close).
  Windows 11 24H2 zeroes the object address for a caller without the debug right, and an elevated or
  protected holder cannot be opened, so those are told `unchecked` by holder exe, never dropped
  silently; the duplicate path is bounded (a budget of 512 a scan). The scan reads the system-wide
  handle table and may duplicate handles, so it is **not cheap and never runs on an ordinary
  frame** — only at a report (a match's load, its end, a minute), where the full module walk
  already runs. Its own parse is cheap (a test times it on a realistic handle count); the
  `NtQuerySystemInformation` call and the duplicate-and-compare on 24H2 are the OS-bound cost.
- **Memory is read with `ReadProcessMemory`**, which fails instead of faulting on a page made
  unreadable or freed meanwhile. The handle-scan buffer is the game's own `VirtualAlloc`'d
  memory, read within the length `NtQuerySystemInformation` filled.
- **Windows' functions** are called through their own exports (dynamic rows of the engine
  library). A Windows without one (`LdrRegisterDllNotification` is Vista's,
  `GetWindowDisplayAffinity` is 7's) leaves that part off.

## The server and the tool

- The match server logs each part once per connection to `logs/modules.log` (16 files of 4 MB):
  `<name> (account <id>), match <id>, <moment>, part 1/2 of 12 entries: <entry> | <entry> ...`
  (`client_report.rs` gives each entry's form), the moment `loaded`, `ended` or `minute <n>`.
- `release/moderate modules` lists each entry with the accounts that have it, rarest first, a
  module with the folders it was told in; `--account <name>` lists one account's entries with
  how many accounts share each.
## Checked, and not

- Checked: the codec, the folder and name rules, the address-space scan, the
  windows' overlap (`srp-validate`'s tests); the watch's records (the patch
  crate's tests); the server's line and that a report removes no one from a match (match server's tests); the
  watch as Windows calls it, in Unicorn (`check_module_report_026g.py`: the TLS directory, both
  callbacks' stack and registers from every stack skew, the registration once, what is asked and
  kept).
- Run in a game once (2026-11-10, one headless Wine client with DXVK, a bot match, a test build
  of this branch and its server): the game starts with the TLS callback and plays; the report
  at the world's load reached the server's log with the modules, the three DLLs loaded after
  the start (`dxgi.dll`, `BugTrap.dll`, `d3d11.dll`, each with its PE marks as loaded and still
  listed). Wine has one executable image no module
  lists (`code image rx 2000/c000`), not traced.
- Windows over the game, in the same kind of run with Wine's task manager set always on top
  over the game: with the game in a window the report has `over "taskmgr.exe" class "#32770"
  10108 404x453 from windows` (its exe is in Wine's Windows folder; a place outside it has
  only been seen in tests); with the game fullscreen (its window above the task manager, which the
  screen does not show either) and without the task manager, no over entry. On Windows it has
  not run: what the taskbar, Windows' own pop-ups and the usual overlays look like will show
  in the first live reports.
- Unseen so far: the report at a match's close; a minute's report in a game (only the state machine's tests
  and the server's line); Windows; what overlays look like in the log.
- The later look (client patch 125, the owner's step 2026-11-12): the handle holders, the
  window event watch and the display affinity. Checked: the shared format for the new entries
  (`holder`, `unchecked`, the `over` entry's affinity and its match seconds), the server's line
  and `release/moderate`'s parse and lines for them (`srp-validate` and match-server tests, a
  parse check of the exact log lines); the patch crate builds and lints clean (i686), the arena
  relinks (`arena_026g.lock.json`), and the watch as Windows calls the TLS and DLL callbacks
  still passes Unicorn (`check_module_report_026g.py`), now with the window event callback's
  address handed up beside the DLL notification's. **Not run on Windows or in a game**, as the
  rest of patch 125 has not been: the `NtQuerySystemInformation` scan, the duplicate-and-compare,
  24H2's zeroed object addresses, elevated holders, `SetWinEventHook`'s delivery on the message
  pump and `GetWindowDisplayAffinity`'s values are unseen live. The handle scan's syscall cost on
  a real machine and the event callback's behaviour under the game's pump are to be watched in the
  first live reports.
