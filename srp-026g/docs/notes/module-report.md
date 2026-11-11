# Module reports (client patch 125)

Each game tells the match server what is loaded into it besides Windows' own DLLs, which DLLs
were loaded since its first instruction, and which other programs' always-on-top windows
(overlays) are over it. The server writes it to a log; `release/moderate modules` reads the
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
  (`srp_validate::report`, kind 2; `memory-report.md` describes the channel). Never on a timer.
- **What is told** (`srp_validate::modules`), the rare kinds first so a long module list cannot
  crowd them out; up to 144 entries a report, more are only counted:
  - **over**: each other process's window that is visible, always on top (`WS_EX_TOPMOST`), above
    the game's window in the windows' order and over some of it: the exe's file name and where
    its folder is (as a module's, a user's name left out; `windows` for the Windows folder, `?`
    where Windows does not tell), the window's class, its extended styles and how much of
    the game's window it covers. Needs no right. Discord's,
    Steam's and a recorder's overlays, the taskbar and a volume pop-up are such windows too:
    read rarest first. The first 16 from the top. Where it cannot be told, one entry says why:
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
  its process, whether it is visible, its extended styles, its rectangle and its class
  (user32's own questions; no message is sent to it). The exe's path of each one found is asked
  with `QueryFullProcessImageNameW`. A window that is not always on top cannot
  stay above the game while it is played, so only those are told: what the player has open
  beside the game is not.
- **Memory is read with `ReadProcessMemory`**, which fails instead of faulting on a page made
  unreadable or freed meanwhile.
- **Windows' functions** are called through their own exports (dynamic rows of the engine
  library). A Windows without one (`LdrRegisterDllNotification` is Vista's) leaves that part off.

## The server and the tool

- The match server logs each part once per connection to `logs/modules.log` (16 files of 4 MB):
  `<name> (account <id>), match <id>, <moment>, part 1/2 of 12 entries: <entry> | <entry> ...`
  (`client_report.rs` gives each entry's form).
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
- Unseen so far: the report at a match's close; Windows; what overlays look like in the log.
