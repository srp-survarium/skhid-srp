# Survarium 0.26g launcher (`survarium_launcher.exe`): static reverse-engineering notes

> **Version scope:** client 0.26g (build 2777), `survarium_launcher.exe`. Static
> reverse engineering of 2026-10-13; nothing was run to write it. What was
> built from it, and what was run afterwards (the patches marked "not
> tested" below were then run on Windows 11): `../../client-patches/launcher/README.md`.
> The updater's side of the pipe: `launcher_updater.md` beside this page.
> The scripts named here are in `tools/re-kit/launcher/scripts`.

Target: `downloads\v026g\extracted\Survarium\game\binaries\x86\survarium_launcher.exe`,
4,740,728 bytes, SHA-256 `710956d2347864effb353064dd7beaee1dd2773c74756d3f6677df71431c5266`,
link time 2015-01-14, PDB path `d:\survarium.deploy\binaries\Win32\survarium_launcher.pdb` (no PDB available).

Method: static only (dumpbin listing + own scripts). Nothing was run. Every address below is a
0.26g virtual address (image base 0x10000). File offsets: `.text` = VA - 0x10C00, `.rdata` and
`.data` = VA - 0x11000.

Confidence labels used throughout:

- **PROVEN**: read in the 0.26g disassembly at the address given.
- **LIKELY**: inferred (from 0.21d, from strings, from Windows behaviour, or from the 0.26g updater's strings), not fully traced.
- **UNKNOWN**: could not be determined statically.

Helper material written during this work (all in `tools\re-kit\launcher\`):

| File | Purpose |
|---|---|
| `scripts\lpe.py` | tiny PE reader (sections, imports, VA/offset) |
| `scripts\strings_xref.py` | all ASCII/UTF-16 strings with VA, length, slack, code xrefs -> `v026g\strings_xref.txt` |
| `scripts\annotate.py` | dumpbin listing + strings + import names + names from `names_026g.txt` -> `v026g\survarium_launcher.annot.txt` |
| `scripts\names_026g.txt` | 0.26g function names matched by hand (about 110) |
| `scripts\show.py START END` | compact view of a VA range of the annotated listing |
| `scripts\peek.py VA [LEN]` | hex dump at a VA |
| `scripts\callers.py VA...` | direct call/jmp sites and absolute references |
| `scripts\rsrc.py [strings|list]` | resource tree, string tables per language |
| `scripts\sites.py` | prints VA / file offset / original bytes of every patch site and the string table used below |
| `scripts\relocs.py VA:LEN...` | tells whether a byte range is touched by a base relocation |

General patching caveats (PROVEN from headers):

- The exe has a `.reloc` section (5087 HIGHLOW fixups) and image base 0x10000. If Windows rebases it,
  every relocated dword gets the delta added. So: never overwrite a relocated dword with non-address
  bytes, and a new absolute address needs a fixup of its own (or reuse an existing fixed-up operand).
  `relocs.py` was run on every site recommended below; exceptions are marked.
- The exe carries an Authenticode certificate table (file 0x483E00, 0x1878 bytes). Any patch
  invalidates the signature. The launcher never checks it (see 1.7).
- Zero-filled slack usable as caves: `.text` tail VA 0x3AB39..0x3ABFF (file 0x29F39, 199 bytes),
  `.rdata` tail VA 0x43E64..0x43FFF (file 0x32E64, 412 bytes). Both lie past VirtualSize but inside
  SizeOfRawData of the last page (LIKELY mapped, the usual code-cave situation; not tested).

---

## 0. Object layout cheat sheet (PROVEN)

`launcher` object (lives on the stack of `application::execute` 0x11166, about 0x51F0 bytes):

| Offset | Meaning | Evidence |
|---|---|---|
| +0x0000, +0x0D20 | two in-place storages for `launcher_form` (0xD20 bytes each), used alternately | 0x14F8D..0x14FB7 |
| +0x1A40 | pointer to the storage of the current form | 0x141D1, 0x14F8D |
| +0x1A48 | `settings` (see section 11) | 0x141D9 |
| +0x1A64 | `settings.language` (0..4) | 0x150AC, 0x1718D |
| +0x1A68/+0x1A6C | `settings` language-changed callback / context | 0x14EC6 |
| +0x1A70, +0x1A90 | boost::function: download type changed, port changed | 0x14376..0x143B0 |
| +0x1AB0 | boost::asio io_service | 0x1420C |
| +0x1AC0 | `vostok::win_utils::pipe` (see section 4) | 0x14245.. |
| +0x2B70 | pipe state (pipe+0x10B0) | 0x1429D |
| +0x2B78 | SC_HANDLE of the update service or 0 | 0x142EF |
| +0x2B80..+0x2B9F | progress: 4 x u64 (current, total, speed, time left) | 0x15BF0, 0x16926 |
| +0x2BA0 | u8 from message 19 (seconds) | 0x15FBB |
| +0x2BA8 | wchar[260] drive/path from message 4; +0x2DB0 u64 bytes; +0x2DB8 u8 | 0x13F97 |
| +0x2DC0 | u8 pending prompt kind (4, 0x0F, 0x10, 0x11, 0x12, 0x16, 0x19) | 0x15C38.. |
| +0x2DC4 | `launcher_form*` current form (0 when not the temp copy) | 0x1445C |
| +0x2DC8 | `launcher_form*` new form being loaded after a language change | 0x14FC0 |
| +0x2DCC | list head for key-sequence handlers (never filled in 0.26g) | 0x16FE5 |
| +0x2DD0 | timer id | 0x14EDD |
| +0x2DD4 | **launcher state** | 0x145A8 |
| +0x2DD8 | **updater state** (last value from message 2) | 0x15BCD |
| +0x2DDC | single-instance mutex handle | 0x17060 |
| +0x2DE0 | flag "language changed, rebuild the form" | 0x150A3 |
| +0x2DE1 | flag "send download type at connect" (never set in 0.26g) | 0x15AE6 |
| +0x2DE2 | flag "restart the updater after it disconnects" | 0x16580 |
| +0x2DE4 | wchar[0x200] installed version text | 0x1435A |
| +0x2FE4 | wchar[0x1000] delegated command line (message 3) | 0x15C11 |
| +0x4FE4 | wchar[260] current file name (messages 6..9) | 0x15D7B |
| +0x51EC | last message type (set by messages 2, 3, 5, 6..9 only; not initialised) | 0x15BE5 |

---

## 1. Startup flow

### 1.1 Entry, `application::initialize` (0x11068), `application::execute` (0x11166) (PROVEN)

- WinMain body at 0x1E241 calls `application::initialize`, then `application::execute`; the exit code is the global 0x47050.
- `initialize`: stores support e-mail `launcher_crash_reports@survarium.com` (0x110A5), support URL
  `http://support.survarium.com` (0x110D3), calls `debug::set_support_server` (0x20360: URL
  `http://crasheat.survarium.com/process.php`, port 80), version `0.26g0` (0x11106), sets the BugTrap
  usage flags 0x45720 = 0x4571C = 0x45704 = 1 (0x11134), `bugtrap::initialize` (0x20610, section 12),
  build fingerprint (0x11817), `logging::initialize` (0x1DFFB).
- `execute`: `launcher::launcher` (0x141BE), `launcher::execute` (0x14DB1), destructor (0x14539).

### 1.2 `launcher::launcher` (0x141BE) (PROVEN)

1. `settings::reload` (0x1D4C4) and `settings::reload_language` (0x1D852) on this+0x1A48.
2. io_service, pipe object (state 1, buffer capacity 0x1000).
3. Service name `"%s Update Service"` (VA 0x3E760) with the product name, then
   `OpenSCManagerW(NULL, NULL, SC_MANAGER_CONNECT=1)` (0x142D4) and
   `OpenServiceW(scm, "Survarium Update Service", SERVICE_START=0x10)` (0x142E8). The handle (or 0) goes to this+0x2B78. No message if absent.
4. `OleInitialize(NULL)` (0x1434D), `on_language_changed` (0x150AC: `SetThreadUILanguage(LANGID)` on Vista+, else `SetThreadLocale`; LANGID table at 0x3E098 = 0x0409, 0x0419, 0x0422, 0x0407, 0x0415).
5. `get_local_version(this+0x2DE4, 0x200)` (0x14366), empty string on failure.
6. `GetModuleFileNameW`; **only if the path ends with `\temp\survarium_launcher.exe`**
   (`is_running_from_temp` 0x1415F, exact case-sensitive tail compare with the string at 0x3E908)
   the `launcher_form` is created (0x14457) and its four callbacks are bound:
   form+0xCA0 -> `on_form_input` 0x16FE3, form+0xCE0 -> `on_launcher_window_closing` 0x1679F,
   form+0xCC0 -> `on_play_button_clicked` 0x17160, form+0xD00 -> `on_new_form_ready` 0x167BF.
   Otherwise this+0x2DC4 = 0 (no window at all in the first process).
7. State = 1, `SetThreadExecutionState(ES_CONTINUOUS|ES_SYSTEM_REQUIRED|ES_AWAYMODE_REQUIRED = 0x80000041)` (0x1451B).

### 1.3 `launcher::execute` (0x14DB1) (PROVEN)

| Step | Code | Failure |
|---|---|---|
| log "Launcher started" | 0x14DE3 | |
| single instance: `create_instance_mutex` 0x17027 | 0x14E25 | MessageBoxW(title 0x81, text 0xA8, MB_ICONWARNING), return 6 |
| Windows Vista or later (`is_vista_or_greater` 0x12B88, dwMajorVersion >= 6) | 0x14E7B | MessageBoxW(text 0x85), return 5 |
| `run_as_copy` 0x1472A; true -> return 0 (process ends) | 0x14E89 | |
| if state == 8 -> return 4 | 0x14E99 | |
| `check_hardware` 0x145C5 (RAM, CPU, VRAM), non-zero -> return it | 0x14EA6 | see 6.4 |
| `show_launcher_window` 0x150FA if a form exists (centre on desktop, `ShowWindow(SW_SHOW)`) | 0x14EB3 | |
| language callback = 0x15097, `SetTimer(NULL, 0, 100 ms, NULL)` | 0x14EC6..0x14ED6 | |
| `start_updater` 0x15693 | 0x14EE3 | |
| message loop: `WM_TIMER` with our id -> `io_service.poll()` (0x18DE1); everything else Translate/Dispatch then `on_message_processed` 0x14F58 | 0x14EE8..0x14F37 | |

All pipe I/O is serviced only from that 100 ms timer, on the UI thread. While a modal MessageBox is
open the pipe is not serviced (LIKELY: the timer has no window and no TimerProc, the box's own loop just discards it).

Single instance (PROVEN, 0x17027): `CreateMutexW(NULL, FALSE, "<product>_or_its_installer_is_already_running")`
(format at 0x3E7B8, i.e. `Survarium_or_its_installer_is_already_running`), failure when
`GetLastError() == ERROR_ALREADY_EXISTS (0xB7)`. The mutex is closed (`close_instance_mutex` 0x1707C)
just before the temp copy is started (0x14BDD), before a delegated command line (0x16742), before
Play (0x171E5) and at the start of `restart_launcher` (0x162EC).

### 1.4 `run_as_copy` (0x1472A) (PROVEN)

**The launcher does not copy itself.** It asks `survarium_updater.exe` to do it, over the pipe.

1. `IsDebuggerPresent()` non-zero -> return false (0x14742..0x1474A).
2. `GetModuleFileNameW`; fails -> MessageBoxW("GetModuleFileName(NULL) failed", "Launcher"), state 8, return false.
3. Path already ends with `\temp\survarium_launcher.exe` -> return false (0x14780..0x14788). This is how the copy knows it is the copy.
4. State = 3, `refresh_status`; pipe callbacks: on_message 0x15B02, on_disconnected 0x1619B, on_connected 0x15A5D.
5. Updater path = directory of the launcher + `\survarium_updater.exe` (0x14847).
6. argv for the updater (5 entries, array at [ebp-4Ch]):

   | # | Value | Source |
   |---|---|---|
   | 0 | `<launcher dir>\survarium_updater.exe` | 0x14862 |
   | 1 | `copy` | 0x14865 (string 0x3E9D8) |
   | 2 | `\\.\pipe\vostok_updater_pipe-<product>` | 0x1486C..0x14883, format `%s-%s` |
   | 3 | user Documents folder | `SHGetKnownFolderPath(FOLDERID_Documents)` 0x1376C (GUID at 0x3E678) |
   | 4 | user name | `GetUserNameW` 0x148BB |

7. Up to **3 tries** (counter [ebp-8], 0x14A71..0x14A78). Each try:
   - updater state (this+0x2DD8) = 0; `pipe::accept(pipe name)` (0x14952) **before** the process is started;
   - `run_survarium_executable_as_administrator(5, argv, use_service=true)` (0x14961); false ->
     MessageBoxW("Can't run survarium_updater from service nor directly", "Launcher"), state 8, return false;
   - wait loop: `io_service.poll()`, stop when updater state == 1 (failure) or == 0x13 (done), else
     `Sleep(100)`, at most 30 rounds = about 3 s ("Updater timeout", 0x149E4..0x149EF);
   - not 0x13: log "Copying failed. Retrying...", `pipe::disconnect`, `Sleep(1000)`, next try
     (**a new updater process is started on every try**);
   - after the third failure: MessageBoxW("Error running survarium_updater", "Launcher"), state 8, return false
     (then `execute` returns 4: the process ends and no window was ever shown).
8. On updater state 0x13: `pipe::disconnect`, then the launcher directory is cut three more levels
   (0x14BB6..0x14BD9): `X\game\binaries\x86` -> `X`. Application = `X\temp\survarium_launcher.exe`.
9. Command line for the copy (0x14C13..0x14CA5): `"X\temp\survarium_launcher.exe"` followed by every
   original argument (argv[1..]) each preceded by one space and **not re-quoted**.
10. `CreateProcessW(app, cmdline, NULL, NULL, FALSE, 0, NULL, lpCurrentDirectory = X, &si, &pi)` (0x14CEA).
    Success -> handles closed, return true. Failure -> MessageBoxW("Can't run self copy"), state 8.

So the copy's working directory is the folder that contains `game` and `temp`, and it finds the game
files relative to its own path (`<own dir>\..\game\launcher\...`, section 8) or through the registry.

What the original updater copies in `copy` mode (LIKELY, from the 0.26g updater's strings and the call
sequence at updater VA 0x1D488..0x1D691, not fully traced): creates `\temp`, copies
`survarium_launcher.exe`, `survarium_updater.exe`, `bugtrap.dll` (each mandatory) and `apatch.exe`
(only if present). The launcher itself only requires that `X\temp\survarium_launcher.exe` exists
afterwards (PROVEN); the temp copy then needs `X\temp\survarium_updater.exe` (3.1) and loads
`BugTrap.dll` from its own directory if present (section 12).

### 1.5 Command-line options (PROVEN)

Only two are parsed, both by prefix match on whole arguments (wcsstr result == argument start):

- `-product_name=<name>` (`get_product_name` 0x1D3EF, string 0x3F0A4): default `Survarium` (0x3F98C).
  The product name feeds the registry path, the mutex name, the service name, the pipe name, the
  Documents sub-folder and the remote page URL.
- `-language=<english|russian|ukrainian|german|polish>` (`get_command_line_language` 0x1D567,
  string 0x3F9A0): anything else or absent = english. Only used when no valid `language` value exists
  in HKCU (section 10).

Both are forwarded to the temp copy because all arguments are forwarded.

### 1.6 Exit codes (PROVEN)

`execute` return: 0 normal, 1/2/3 user declined the RAM/CPU/VRAM warning, 4 updater failure during
`run_as_copy`, 5 OS too old, 6 already running. `PostQuitMessage` codes: 0 (window closed, Play
succeeded, delegated command started, updater aborted), 7 (`restart_launcher` could not restart),
8 (no game launch parameters), 9 (game could not be started).

### 1.7 Integrity checks

- **No signature or hash check of the launcher or of any other file** (PROVEN by absence: imports
  contain no crypt32/wintrust/Crypt* functions, see `survarium_launcher.headers.txt`; the only file
  comparison is `files_equal` 0x1625B in `restart_launcher`, a plain byte compare).
- The exe manifest is `asInvoker`. The original `survarium_updater.exe` manifest is
  `requireAdministrator`, `survarium_service.exe`, `survarium.exe`, `apatch.exe` are `asInvoker` (PROVEN, manifests read).

### 1.8 Patches for this section

| Goal | VA | File | Original | New | Notes |
|---|---|---|---|---|---|
| never use the temp copy (run in place) | 0x1474A | 0x3B4A | `0F 85 58 06 00 00` | `E9 59 06 00 00 90` | jmp to 0x14DA8 (return false); no relocation |
| create the window even when not in `\temp\` | 0x14442 | 0x3842 | `0F 84 C5 00 00 00` | `90 90 90 90 90 90` | needed together with the previous one |
| local pages when run in place | string 0x3F514 | 0x2E514 | `\..\game\launcher\index-` (24 chars) | `\..\..\launcher\index-` (22 chars + NUL) | UTF-16, room for 25 chars |
| skip hardware warnings | 0x145C5 | 0x39C5 | `81 EC 04 03 00 00` | `33 C0 C3` (rest unchanged) | `check_hardware` returns 0 |
| never use the Windows service | 0x15521 | 0x4921 | `74 20` | `EB 20` | always `CreateProcessW`/`runas` (see 3.3) |

Run-in-place consequences (PROVEN from the code paths): updater path becomes
`binaries\x86\survarium_updater.exe` (0x157A4..0x157EA), the temp-path argument becomes `GetTempPathW()`
(0x15902) instead of `X\temp\patch_temp`, and without the string patch the local pages would be looked up
in `binaries\game\launcher\`.

---

## 2. Registry

All access goes through `RegOpenKeyExW(hive, path, 0, sam, &h)` with **no `KEY_WOW64_*` flag**; the
launcher is a 32-bit process, so on 64-bit Windows HKLM paths resolve to
`HKLM\SOFTWARE\WOW6432Node\...` (LIKELY, standard redirection; HKCU\Software is not redirected).

Product key path: `Software\Vostok Games\<product>` (format 0x3E784, built at 0x14080 and 0x1D496),
normally `Software\Vostok Games\Survarium`.

`registry_key::get_string` (0x14027): `RegQueryValueExW`; succeeds only for type `REG_SZ` (1) with an
even byte count; forces a terminating NUL. `REG_EXPAND_SZ` is rejected.

### 2.1 Every access (PROVEN)

| # | Function (VA) | Hive | Key | Value | R/W, sam | Buffer (chars) | When missing |
|---|---|---|---|---|---|---|---|
| 1 | `get_product_guid` 0x140AE | HKLM (push at 0x140D7) | product key | `product GUID` | R, KEY_QUERY_VALUE | 256 | GUID = "" (silent) |
| 2 | `get_first_install_version` 0x15177 | HKLM (0x15194) | product key, then subkey `Installer` (0x151B8) | `first install version` | R | 64 | falls back to #3 |
| 3 | `get_local_version` 0x15213 | HKLM (0x15231) | product key | `file to execute` | R | 260 | version unknown (section 7) |
| 4 | `get_uninstall_string` 0x15434 | HKCU first (0x1544B), then HKLM via 0x13FF1 | `Software\Microsoft\Windows\CurrentVersion\Uninstall\{<GUID>}_is1` (format 0x3E828) | `UninstallString` | R | 260 | "" (silent); surrounding quotes are stripped |
| 5 | `get_install_location` 0x1E30D | HKCU first (0x1E321), then HKLM | same uninstall key | `InstallLocation` | R | 260 | "" (silent); trailing slashes removed |
| 6 | `start_updater` 0x15997 | HKLM (0x159A4) | product key | `update server` | R | 260 | key: MessageBoxW "Can't open survarium registry key"; value: "Can't retrieve updater server string from survarium registry key"; both -> state 8 |
| 7 | `restart_launcher` 0x162F4 | HKLM (0x16301) | product key | `launcher` | R | 260 | assertion "Can't restart launcher", `PostQuitMessage(7)` |
| 8 | `get_game_launch_parameters` 0x17092 | HKLM (0x170AD) | product key | `file to execute`, `arguments`, `working folder` | R | 260 / rest of 0x8000 / 260 | any missing -> assertion "Can't get game launch parameters", `PostQuitMessage(8)` |
| 9 | `settings::reload` 0x1D4C4 | **HKCU** (0x1D4E3) | product key | `port_number` (REG_SZ), `download_type` (REG_DWORD) | R | 6 / dword | defaults: torrent allowed, port "" |
| 10 | `settings::save` 0x1D71A | **HKCU** (0x1D72F), `RegCreateKeyExW`, KEY_SET_VALUE | product key | `download_type` (REG_DWORD 1 = torrent, 0 = direct), `port_number` (REG_SZ) | W | | silent |
| 11 | `settings::reload_language` 0x1D852 | **HKCU** (0x1D871) | product key | `language` (REG_DWORD 0..4) | R | | see section 10 |
| 12 | `settings::save_language` 0x1D8DA | **HKCU** (0x1D8E8), `RegCreateKeyExW`, KEY_SET_VALUE | product key | `language` | W | | silent |
| 13 | `check_cpu` 0x12D3C | HKLM (0x12D55) | `Hardware\Description\System\CentralProcessor` + subkeys (`RegEnumKeyExW`) | `~MHz` | R | | assertion "Can't retrieve processor info" |

Nothing is ever written to HKLM by the launcher.

### 2.2 What a player with no registry keys and no service sees (PROVEN path by path)

Unpatched launcher, original files only:

1. `execute` -> `run_as_copy` starts `binaries\x86\survarium_updater.exe copy ...` (UAC prompt, because the
   original updater requires elevation: `CreateProcessW` fails with 740 and `ShellExecuteW "runas"` is used).
   If the updater exe is missing or the prompt is refused: message box "Can't run survarium_updater from
   service nor directly", exit code 4, **no window**.
2. In the temp copy: constructor reads `file to execute` -> missing -> version text empty (no message).
3. Window is shown (0x14EBB) **before** `start_updater`.
4. `start_updater`: neither `Installer\first install version` nor `file to execute` -> MessageBoxW
   "Can't retrieve first install version or local version" (0x1577A), state 8. The window stays open with
   status text `Error` (string 0x88), empty progress bar, **Play disabled** (it is only enabled by updater
   state 14, see 5.4). No updater is started; the pipe keeps listening.
5. If Play were clicked anyway (it cannot be, the control has no callback while disabled, 0x1B699):
   missing `file to execute` / `arguments` / `working folder` would be a fatal assertion.

### 2.3 Minimal registry content for a working launcher (PROVEN requirements)

In the product key (HKLM 32-bit view, or HKCU after the hive patch below):

| Value | Type | Needed for | Content |
|---|---|---|---|
| `file to execute` | REG_SZ | version detection (unless `Installer\first install version` exists) and Play | path of the game exe; opened with `_wfopen(path, "rb")` as is (so absolute, or relative to the working directory `X`) and used as the quoted first token of the game command line |
| `arguments` | REG_SZ | Play | appended verbatim after `"<exe>" -localization=<lang> `; may be empty but must exist |
| `working folder` | REG_SZ | Play | `lpCurrentDirectory` of the game process |
| `update server` | REG_SZ | `start_updater` | passed to the updater as argv[12]; any text for a replacement updater |
| `launcher` | REG_SZ | only when the updater reports state 17 | path of the real launcher exe |

Optional: `product GUID` (then `InstallLocation`/`UninstallString` are looked up),
`Installer\first install version` (its presence switches the updater mode to `full_update`).

### 2.4 Patch: read the product key from HKCU instead of HKLM (no admin rights needed)

Each site is `68 02 00 00 80` (`push 80000002h`); change the `02` to `01`. No relocations on these bytes.

| Site (VA of the `68`) | File offset of the `02` | Used by |
|---|---|---|
| 0x140D7 | 0x34D8 | `product GUID` |
| 0x15194 | 0x4595 | `Installer\first install version` |
| 0x15231 | 0x4632 | `file to execute` (version) |
| 0x159A4 | 0x4DA5 | `update server` |
| 0x16301 | 0x5702 | `launcher` |
| 0x170AD | 0x64AE | Play parameters |
| 0x13FFF | 0x3400 | HKLM fallback of the uninstall key (optional) |

Leave 0x12D55 (CPU info) alone. After this patch the same HKCU key holds both the install values and
the user settings (`language`, `port_number`, `download_type`), which does not conflict (different names).

---

## 3. `start_updater` (0x15693)

### 3.1 Sequence (PROVEN)

1. State = 2, `refresh_status`; pipe callbacks bound (0x156B7..0x15718).
2. Pipe name `\\.\pipe\vostok_updater_pipe-<product>` (0x1571D..0x15734); `pipe::accept` (0x1574D):
   **the launcher creates the pipe and starts listening before the updater process exists.**
3. Version: `get_first_install_version` (0x15759); if that fails `get_local_version(buf, 64)` (0x1576F);
   both fail -> message box, state 8, return.
4. Updater path: own module path without the file name + `\` + `survarium_updater.exe`
   (0x157A4..0x157EA). For the temp copy this is `X\temp\survarium_updater.exe`.
5. argv, 14 entries (array at [ebp-6Ch], count 0x0E pushed at 0x15A21):

   | # | Value | Source (VA) |
   |---|---|---|
   | 0 | updater exe path | 0x157FC |
   | 1 | `full_update` if `Installer\first install version` was read, else `update` | 0x157F8..0x15812 (strings 0x3ED0C / 0x3ED24) |
   | 2 | pipe name `\\.\pipe\vostok_updater_pipe-Survarium` | 0x1581B |
   | 3 | version string: first install version, else local version (e.g. `0.26g0`) | 0x15824 |
   | 4 | `torrent` if settings.allow_torrent (default **true**) else `direct` | 0x1580B, 0x15827..0x15833 |
   | 5 | `InstallLocation` from the uninstall key, or "" | 0x15836..0x1584D |
   | 6 | common Start Menu folder (FOLDERID_CommonStartMenu, GUID at 0x3E658) | 0x15858 |
   | 7 | public Desktop folder (FOLDERID_PublicDesktop, 0x3E668) | 0x1586E |
   | 8 | user Documents folder (FOLDERID_Documents, 0x3E678) | 0x15884 |
   | 9 | `UninstallString` without quotes, or "" | 0x15899 |
   | 10 | user name (`GetUserNameW`) | 0x158B9 |
   | 11 | temp folder: temp copy -> `<own dir>\patch_temp` (own path minus the 22 chars `survarium_launcher.exe`, plus `patch_temp`, 0x15924..0x15969); otherwise `GetTempPathW()`; trailing backslashes removed (`PathRemoveBackslashW`) | 0x158F1..0x1598E |
   | 12 | registry `update server` | 0x15A09 |
   | 13 | settings port string (wchar[6], "" when unset) | 0x15A12 (this+0x1A4A) |

   (0.21d had 12 arguments; 0.26g has 14.)
6. `run_survarium_executable_as_administrator(14, argv, use_service=true)` (0x15A25); false -> message box
   "Can't run survarium_updater from service nor directly", state 8. True -> state 3.

### 3.2 `on_updater_connected` (0x15A5D) (PROVEN)

State = 4, `refresh_status`. The "send download type" branch is dead (flag this+0x2DE1 is never set).
**The launcher sends nothing at connect.**

### 3.3 `run_survarium_executable_as_administrator` (0x15511) (PROVEN)

1. If `use_service` and the service handle is non-zero: `StartServiceW(handle, argc, argv)` (0x15532);
   success -> return true. Otherwise fall through.
2. Command line: `"argv[0]"` then, for each further argument, ` "` + argument + `"` (strings 0x3EC7C,
   0x3EC88, 0x3EBB4). No escaping of embedded quotes or of a trailing backslash.
3. `CreateProcessW(NULL, cmdline, NULL, NULL, FALSE, 0, NULL, NULL, &si, &pi)` (0x15627): inherits the
   working directory, no handle inheritance.
4. If it fails with `GetLastError() == 740` (ERROR_ELEVATION_REQUIRED, 0x1563C):
   `ShellExecuteW(NULL, "runas", argv[0], <rest of the command line>, NULL, SW_HIDE)` (0x15653), success when > 32.
5. Any other failure (e.g. file not found) -> false.

For a replacement updater: give it an `asInvoker` manifest and step 3 starts it without UAC. If an old
"Survarium Update Service" is still installed on the machine, step 1 would start that old service
instead; the one-byte patch at 0x15521 (1.8) removes the service path.

### 3.4 Timeouts (PROVEN)

- `copy` phase: about 3 s per try, 3 tries, 1 s pause (1.4).
- Normal phase: none. The launcher stays in state 3 ("Checking game version...") for ever if nobody connects.
- Strings 0xAA "Waiting for updating software..." and 0xAB "Launching updating software..." are not referenced by 0.26g code.

---

## 4. The pipe and its framing (`vostok::win_utils::pipe`, `fixed_packet<4096>`)

### 4.1 Server side (PROVEN, `pipe::accept` 0x13B4D)

```
CreateNamedPipeW(name,
    dwOpenMode   = 0x40080003  // PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED
    dwPipeMode   = 0           // PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT
    nMaxInstances = 1, nOutBufferSize = 0x10000, nInBufferSize = 0x10000,
    nDefaultTimeOut = 0, lpSecurityAttributes = NULL)          // 0x13B66..0x13B7C
ConnectNamedPipe(handle, overlapped)                            // 0x13BF4
```

- Only runs when the pipe object is in state 1 (idle). Error from `CreateNamedPipeW` is stored in a local
  error code that the callers ignore (then nothing listens and nothing is reported).
- Pipe states (pipe+0x10B0): 1 idle, 2 waiting for a client, 3 connected, 0 failed (transient).
- The client (updater) opens the name it got in argv[2] with read+write access; byte stream.
- Default security descriptor: fine for the same user and for an elevated instance of the same user or
  LocalSystem (LIKELY, default DACL).

### 4.2 Framing (PROVEN, `async_receive` 0x13D32, `on_received` 0x13E9D, `send` 0x13CCA)

Both directions:

```
u16 little-endian  N        payload length in bytes (not counting these 2 bytes)
N bytes            payload  payload[0] = message type (u8), then the fields
```

- Receive: read exactly 2 bytes, then exactly N bytes into the fixed buffer (data+2), call the
  message callback, repeat. `N + 2` must be <= 0x1000 (N <= 4094), otherwise the fatal assertion
  "mutable buffer overflow" (0x13A0C). **Never send N = 0** (the handler would read a stale type byte).
- Send: the packet is built at buffer+2, the length `size - 2` is stored at buffer+0 (0x13CE7) and the
  whole thing is written synchronously (0x13CFC). A write error puts the pipe in state 0 and posts
  `on_disconnected`.
- Read error or EOF (client closed): optional error callback (none bound by the launcher), then
  `on_disconnected` (0x13E62): handle closed, state 1, launcher's `on_updater_disconnect`.

### 4.3 Primitive encoding inside a payload (PROVEN)

| Type | Encoding | Reader/writer |
|---|---|---|
| u8, u16, u64 | raw little-endian, no alignment | 0x13AF2 (write), inline reads |
| narrow string | u8 length L, then L bytes, no terminator | `read_string8` 0x13A7B |
| wide string | u16 character count C, then C UTF-16LE code units, no terminator | `read_string16` 0x13AB3 |

Neither reader checks the declared length against the packet or the destination. A replacement updater
must keep: wide string of message 3 <= 4095 chars (cannot exceed it honestly within N <= 4094), wide
string of message 4 <= 259 chars (destination is wchar[260] inside the launcher object), narrow strings
<= 255 by construction.

---

## 5. The protocol

### 5.1 Messages the launcher RECEIVES (`on_updater_message` 0x15B02) (PROVEN)

Dispatch: `jmp [0x1609E + (type-1)*4]` (0x15BB5) with **no range check**; the table has 25 entries
(file 0x549E). Type 0, type 24 (table entry is 0) and anything above 25 crash the launcher.

| Type | Payload after the type byte | Handler | Effect |
|---|---|---|---|
| 1 | u64 current, u64 total, u64 speed (bytes/s), u64 time left (s) | 0x15BF0 | stored at this+0x2B80.., `refresh_status` |
| 2 | u8 updater state | 0x15BBF | this+0x2DD8 = state, `on_updater_state_changed` (5.3), `refresh_status`, last-type = 2 |
| 3 | wide string | 0x15C0B | delegated command line stored at this+0x2FE4, last-type = 3, no redraw |
| 4 | wide string (drive letter or path), u64 available bytes, u8 | 0x15C27 | stored; pending prompt kind = 4 |
| 5 | narrow string | 0x15CDA | **`MessageBoxA(NULL, text, "Updater error:", MB_ICONERROR)`**, modal; last-type = 5; state unchanged |
| 6, 7, 8, 9 | narrow string (file name or command) | 0x15D0D | if longer than 48 chars it becomes `...` + its last 44 chars (`std::rotate` 0x18345 called at 0x15D55, then three dots); converted with `mbstowcs_s` into this+0x4FE4; last-type = type; `refresh_status` |
| 10..13 | (ignored) | 0x16093 | nothing |
| 14 | none | 0x15D94 | shows the pending prompt (5.2) and answers with 0x1C or 0x1D |
| 15, 16, 17, 18 | none | 0x15C44.. | pending prompt kind = 0x0F, 0x10, 0x11, 0x12 |
| 19 | u8 seconds | 0x15FAE | this+0x2BA0, `refresh_status` (text 0xB9 in updater state 23) |
| 20 | none | 0x15FC6 | port rejected: re-shows the settings window, settings "port ok" flag = 0, MessageBoxW text 0xB8 if the stored port string is empty else 0xB7 |
| 21 | none | 0x16067 | port accepted: flag = 1, `WM_CLOSE` posted to the settings window |
| 22 | none | 0x15C80 | pending prompt kind = 0x16 |
| 23 | none | 0x15C8C | MessageBoxW(title 0x81, text 0xBF "Package is corrupted", MB_ICONWARNING) |
| 24 | - | (null) | **crash** |
| 25 | none | 0x15C74 | pending prompt kind = 0x19 |

### 5.2 Prompt shown by message 14 (PROVEN, 0x15D94..0x15FA4, `ask_retry_cancel` 0x16102)

Title string 0x81 "Warning", `MessageBoxW(..., MB_RETRYCANCEL | MB_ICONWARNING = 0x35)`; Retry -> the
launcher sends 0x1C, anything else -> 0x1D; pending kind reset to 0.

| Pending kind (set by message) | Text |
|---|---|
| 4 (msg 4) | 0xAC "Not enough free disk space on drive %s (only %s is available)" with `X:` (one-character string, upper-cased) or `"path"`, and the size |
| 0x0F (msg 15) | 0xBB "Program can't open file" |
| 0x10 (msg 16) | 0xBC "Program can't close file" |
| 0x11 (msg 17) | 0xBD "Program can't move file" |
| 0x12 (msg 18) | 0xC1 "An error has occurred during running shell command" |
| 0x16 (msg 22) | 0xBE "An error has occurred during verifying hash" |
| 0x19 (msg 25) | 0xC0 "An error has occurred during installation" |
| 0 or other | no box, nothing sent |

### 5.3 Messages the launcher SENDS (PROVEN)

| Type | Payload | Sent by | When |
|---|---|---|---|
| 0x1A | u8: 1 = torrent not allowed (direct), 0 = torrent allowed | `send_switch_download_type_message` 0x17319 | settings saved with a changed "Allow torrent" while the pipe is connected (0x172FA) |
| 0x1B | none | `send_simple_message` 0x1749F | abort: window close (0x167A9) and the "files equal" path of `restart_launcher` (0x16562) |
| 0x1C | none | 0x1611D | Retry in the prompt |
| 0x1D | none | 0x16119 | Cancel in the prompt |
| 0x21 | u16 port (decimal parse of the settings port string) | `send_port_message` 0x173B1 | settings saved with a changed port while connected (0x17309) |

Wire examples: abort = `01 00 1B`; retry = `01 00 1C`; port 6881 = `03 00 21 E1 1A`. Nothing else is
sent, in particular nothing at connect.

Window close (`on_launcher_window_closing` 0x1679F): pipe connected -> send 0x1B; if the write
succeeded `PostQuitMessage(0)` **immediately** (the launcher does not wait for the updater); if the write
failed it returns without quitting; pipe not connected -> `PostQuitMessage(0)`. The 0.21d confirmation
box (string 0xB0) is not present in 0.26g.

### 5.4 State machine

Launcher state (this+0x2DD4), rendering in `refresh_status` (0x16811) (PROVEN):

| State | Set by | Status text | Progress bar | Play |
|---|---|---|---|---|
| 1 | constructor | (0x88 "Error" if ever drawn) | 0 | off |
| 2 | `start_updater` entry | 0x86 "Checking game version..." | 0 | off |
| 3 | updater started, `run_as_copy` | 0x86 | 0 | off |
| 4 | pipe connected | by updater state, below | | unchanged |
| 6 | updater state 13, 14 or 17 (`set_state` 0x145A5, also resets `SetThreadExecutionState(ES_CONTINUOUS)`) | 0x87 "The latest version of the game is installed" (status bar window hidden, single text) | full | on for state 14 only |
| 7 | updater state 22 | 0xB2 "Server maintenance..." | full | off |
| 8 | any failure, or disconnect with updater state != 15 | 0x88 "Error" | 0 | unchanged |

Updater state (this+0x2DD8) while the launcher is in state 4, and the side effects in
`on_updater_state_changed` (0x1612B) (PROVEN):

| Value | Text / behaviour |
|---|---|
| 0 | 0x86 "Checking game version...", bar 0 |
| 1, 3, 7 | 0x88 "Error", bar 0 (1 also means "failed" to `run_as_copy`) |
| 2 | no text |
| 4 | 0x86, bar 0, **clears the stored file name** |
| 5 | 0xAF "An error has occurred during checking current version, retrying..." |
| 6 | 0x89 "Download is starting..." |
| 8 | 0x8A "Checking downloaded files" + " N%" (N = current/total), bar = current/total |
| 9 | downloading: bar = current/total; line 1 = 0xD6 "Estimated time < T" with T = (total - current) / speed (only if speed > 0 and T <= 31,104,000 s); line 2 = 0x8D "S left" with S = total - current (only if total > 0); line 3 = 0xD5 "Speed V" (low 32 bits of speed) |
| 10 | 0xAD "An error has occurred during downloading, retrying..." |
| 11 | 0xB1 "Saving downloaded files..." |
| 12 | 0x8C "Download complete", bar full |
| 13 | launcher state 6, then `run_delegated_command_line` (0x166F1): `CommandLineToArgvW` on the text of message 3, start it with `CreateProcessW` (or `runas` on error 740, never the service), `PostQuitMessage(0)`; errors: "Invalid delegated command line" / "Can't run delegated command line", state 8 |
| 14 | launcher state 6 and **`set_play_button_active(true)`** (0x1616D..0x1617F) |
| 15 | "aborted": at disconnect the launcher quits, or restarts the updater if flag this+0x2DE2 is set |
| 16 | installing: text = 0xB3 "Running command %s" / 0xB4 "Extracting file %s" / 0xB5 "Verifying file %s" / 0xB6 "Patching file %s" for last-type 6 / 7 / 8 / 9, or 0xA9 "Updating" when no file name is stored; bar = current/total; if total > 0 and time left <= 31,104,000 s: `, ` + 0x8D "T left" |
| 17 | launcher state 6, then `restart_launcher` (0x162D8), below |
| 19 (0x13) | "copy done", only meaningful to `run_as_copy` |
| 20 | 0xAC disk space text (uses the data of message 4), clears the file name |
| 22 | launcher state 7 "Server maintenance...", `set_play_button_active(false)` |
| 23 | 0xB9 "Package corrupted! Restarting download in %u sec." with the u8 of message 19 |
| others | no text |

Hazard (PROVEN, 0x16D06..0x16D2C): in updater state 16 with a non-empty file name the string id is
taken from a 4-entry stack table indexed by `last-type - 6` without a range check. Messages 2, 3 and 5
overwrite last-type. So: enter state 16 from state 4 (which clears the file name), and whenever a
message 2, 3 or 5 was sent while a file name is set, send a message 6..9 before anything that redraws
(messages 1, 19, another 2; a language change also redraws).

`on_updater_disconnect` (0x1619B) (PROVEN): clears flag +0x2DE1; if updater state == 15: flag +0x2DE2 set
-> `start_updater` again and clear the flag, else `PostQuitMessage(0)`; any other updater state ->
launcher state 8 ("Error"). It never touches the Play button: **after state 14 a disconnect leaves Play
enabled but replaces the text with "Error"**. A replacement updater should therefore stay connected
until the pipe breaks (the launcher exits on Play and on window close).

`restart_launcher` (0x162D8) (PROVEN): close the mutex; read HKLM `launcher` (missing -> assertion "Can't
restart launcher", `PostQuitMessage(7)`); if `<own dir>\apatch.exe` exists and the file named by
`launcher` is byte-identical to the running exe: re-read the local version, set the "rebuild form"
flag, send 0x1B, set flag +0x2DE2 (so: the updater must answer by going to state 15 and closing the
pipe, then a new updater process is started) and return; otherwise
`CreateProcessW(NULL, "<launcher> -product_name=<product>", ...)` (path not quoted),
`WaitForInputIdle(5000)`, `PostQuitMessage(0)`; failure -> assertion "Can't start launcher".

### 5.5 Recipes for a replacement updater

Frames are written as hex bytes. `u64` = 8 bytes little-endian.

**copy mode** (argv[1] = `copy`): copy `survarium_launcher.exe` (required by the launcher),
`survarium_updater.exe`, `bugtrap.dll`, optionally `apatch.exe` from the updater's own directory to
`<three levels up>\temp\`, connect to argv[2], send `02 00 02 13`, close. On failure send
`02 00 02 01` (the launcher retries, 3 tries in total). Must happen within about 3 s of process start.

**(a) nothing to update, enable PLAY**: connect, send `02 00 02 0E`, keep the pipe open and keep
reading. Launcher: "The latest version of the game is installed", full bar, PLAY active.

**(b) downloading N of M bytes**: `02 00 02 09`, then repeatedly
`21 00 01 <u64 N> <u64 M> <u64 speed> <u64 0>`. The launcher computes the remaining size and the
time itself.

**(c) installing**: `02 00 02 04` (clears the file name), `02 00 02 10`, then per file
`<len> 07 <u8 L> <L bytes name>` (7 = extracting; 6 running command, 8 verifying, 9 patching;
frame length = L + 2) and progress frames `21 00 01 <u64 done> <u64 total> <u64 0> <u64 seconds left>`.

**(d) fatal error with custom text**: `<len> 05 <u8 L> <L bytes>` with L <= 255, frame length L + 2.
Shown in a modal ANSI message box titled "Updater error:" (text interpreted in the system ANSI code
page, so only ASCII is portable). Follow it with `02 00 02 01` (status "Error") or just close the pipe
(launcher state 8, also "Error"). There is no message that puts free text into the status line; the
closest is state 16 + message 6..9, which shows "Running command <text>" etc. with at most 48
characters (longer text is cut to its tail; bytes mapped by `mbstowcs_s` in the C locale, LIKELY 1:1 for ASCII only).
Fixed-text alternatives: message 23 ("Package is corrupted"), the Retry/Cancel prompts of 5.2.

**(e) launcher must restart**: `02 00 02 11`. Needs the `launcher` registry value (2.3).

**maintenance**: `02 00 02 16` ("Server maintenance...", PLAY off).

**expect from the launcher**: `01 00 1B` (abort: the launcher is already quitting, or wants state 15
`02 00 02 0F` followed by a close in the "files equal" case), `02 00 1A xx`, `03 00 21 lo hi`
(answer with `01 00 15` = accepted or `01 00 14` = rejected, otherwise the settings window stays
open), `01 00 1C` / `01 00 1D` after a message 14.

Never send: type 0, 24, anything above 25, or a zero-length frame.

### 5.6 String table (RT_STRING, English 0x0409; also present for 0x0419, 0x0422, 0x0407, 0x0415) (PROVEN, `rsrc.py`)

| Id | Text | Used |
|---|---|---|
| 0x81 | Warning | message box title |
| 0x82 / 0x83 / 0x84 | Your RAM / CPU / VRAM doesn't meet minimal system requirements of Survarium. Would you like to continue? | 0x145FC.. |
| 0x85 | Survarium can be launched only on Windows Vista or later version. | 0x14704 |
| 0x86 | Checking game version... | states 2, 3; updater 0, 4 |
| 0x87 | The latest version of the game is installed | state 6 |
| 0x88 | Error | state 8; updater 1, 3, 7 |
| 0x89 | Download is starting... | updater 6 |
| 0x8A | Checking downloaded files | updater 8 |
| 0x8B | Downloading | not referenced |
| 0x8C | Download complete | updater 12 |
| 0x8D | %s left | updater 9, 16 |
| 0x8E..0x90 | MB/s, KB/s, bytes/s | speed text |
| 0x91..0x95 | GB, MB, KB, byte, bytes | size text |
| 0x96..0xA7 | year(s), month(s), day(s), hour(s), minute(s), second(s) (3 forms each) | time text |
| 0xA8 | Survarium or its installer is already running.\nYou may close it and try again. | 0x14E54 |
| 0xA9 | Updating | updater 16 without file name |
| 0xAA, 0xAB | Waiting for updating software... / Launching updating software... | not referenced |
| 0xAC | Not enough free disk space on drive %s (only %s is available) | updater 20, prompt 4 |
| 0xAD | An error has occurred during downloading, retrying... | updater 10 |
| 0xAF | An error has occurred during checking current version, retrying... | updater 5 |
| 0xB0 | Update process termination may break the installed game.\nAre you sure you want to quit? | not referenced |
| 0xB1 | Saving downloaded files... | updater 11 |
| 0xB2 | Server maintenance... | state 7 |
| 0xB3..0xB6 | Running command %s / Extracting file %s / Verifying file %s / Patching file %s | updater 16 |
| 0xB7 / 0xB8 | The entered port number is busy / is invalid | message 20, settings |
| 0xB9 | Package corrupted! Restarting download in %u sec. | updater 23 |
| 0xBB..0xBD | Program can't open / close / move file | prompts |
| 0xBE | An error has occurred during verifying hash | prompt |
| 0xBF | Package is corrupted | message 23 |
| 0xC0 | An error has occurred during installation | prompt |
| 0xC1 | An error has occurred during running shell command | prompt |
| 0xC2 / 0xC3 / 0xC4 | PROJECT\nWEBSITE / CREATE\nACCOUNT / SHOP | button labels (0x1B8FF..) |
| 0xC5 / 0xC6 | PLAY / UPDATING... | Play button enabled / disabled image |
| 0xC7 / 0xC8 | SAVE / CANCEL | settings |
| 0xD2..0xD4 | Settings / Allow torrent / Port: | settings |
| 0xD5 / 0xD6 | Speed %s / Estimated time < %s | updater 9 |
| 0xD7..0xDA | Torrent download error / Direct download error. / Unable to determine size of the files. / Error writing to file. | not referenced by the launcher |

Hard-coded English texts (not localised): the "Launcher"-titled message boxes of sections 1..3 and
"Updater error:".

---

## 6. Play (`on_play_button_clicked` 0x17160)

### 6.1 How the command line is built (PROVEN)

**There is no game exe name inside the launcher.** Everything comes from the registry
(`get_game_launch_parameters` 0x17092, HKLM product key):

```
cmdline = swprintf(L"\"%s\" -localization=%s ", <"file to execute">, <language word>)   // format VA 0x3F1C0
cmdline += <"arguments">                                                                    // read straight after it
cwd     = <"working folder">
```

- Format string `"%s" -localization=%s ` : VA 0x3F1C0, file 0x2E1C0, UTF-16LE, 22 chars, 4 zero bytes
  after it (room for 23 chars), single xref 0x170F4 (relocated operand).
- Language word by `settings.language` (0x1718D..0x171C5): 0 `english` (0x3F1F0), 1 `russian` (0x3F200),
  2 `ukrainian` (0x3F210), 3 `german` (0x3F224), 4 `polish` (0x3F234), all UTF-16.
- Command-line buffer: 0x8000 wchar on the heap.

### 6.2 Launch (PROVEN)

`close_instance_mutex`, then
`CreateProcessW(NULL, cmdline, NULL, NULL, FALSE, 0, NULL, working folder, &si, &pi)` (0x17216).
No `ShellExecute`, no elevation retry (a game exe that needs elevation would fail here).
Success -> handles closed, `PostQuitMessage(0)`: **the launcher exits**, it does not hide or wait.
Failure -> assertion "Can't start the game" (launcher.cpp line 0x7FF), `PostQuitMessage(9)`.
(Skhid: error 4551, Smart App Control, gets a box of its own and the launcher stays open; patch 14 in
`../../client-patches/launcher/README.md`.)
Missing registry values -> assertion "Can't get game launch parameters", `PostQuitMessage(8)`.
(The assertion UI is the engine's "Fatal Error" box, LIKELY; flags at 0x4857B / 0x4857C suppress repeats.)

The button itself: `launcher_form::launch_survarium` 0x1B893 fires form+0xCC0. While disabled the
control has no callback and an arrow cursor (`set_play_button_active` 0x1B5E9: callback 0x1B893 written
at control+0x18 only when enabled).

### 6.3 `run_survarium_executable_as_administrator` / `run_delegated_command_line`

See 3.3 and updater state 13 in 5.4. They are not involved in Play.

### 6.4 Hardware checks (PROVEN, before the window is shown, `check_hardware` 0x145C5)

`check_ram` 0x12CB7 (`GlobalMemoryStatusEx`, total physical < 0x78000000 = 1920 MB fails),
`check_cpu` 0x12D3C (registry `~MHz` and core count; constants 2 and 0x7CB = 1995 at 0x12E69..0x12E6F, exact
rule not traced), `check_vram` 0x13672 (larger result of two probes, 0x13486 and 0x1353B; < 0x1C000000 = 448 MB fails; the
probes use DXGI / DirectDraw and WMI `Win32_VideoController` on `\\.\root\cimv2`, all local; which probe
uses which API was not traced). Each failure:
`MessageBoxW(MB_OKCANCEL | MB_ICONWARNING)`; Cancel ends the launcher with code 1, 2 or 3.

### 6.5 Patch ideas

- Easiest: provide the three registry values (HKCU after patch 2.4).
- To hard-code instead, the three `get_string` calls at 0x170E4, 0x1711A, 0x17144 would have to be
  replaced by code in a cave; not worked out here.

---

## 7. Version detection (PROVEN)

`get_local_version(out, size)` 0x15213:

1. HKLM product key, value `file to execute` (260 chars).
2. `_wfopen_s(path, L"rb")` (mode string at 0x3EC34).
3. The file is read in 0x80000-byte chunks (with a 0x40-byte overlap) and searched for the 32 bytes
   `survarium_version_header_mark!!\0` (VA 0x3EC3C).
4. The 15 bytes that follow the marker (marker + 0x20) are copied as a C string, converted to UTF-16
   (static buffer 0x487B0) and returned.

In the shipped `survarium.exe` the marker is at file offset 0xA5F594 and is followed by `0.26g0\0`
(PROVEN by reading the file; exactly one occurrence).

Uses: (a) constructor -> this+0x2DE4 -> passed to the form constructor and drawn on the window
(0x1B18C -> 0x1B8B0, LIKELY the small version label); (b) `start_updater` argv[3] when
`Installer\first install version` is absent; (c) re-read in `restart_launcher`.

`get_first_install_version` 0x15177: HKLM product key, subkey `Installer`, value
`first install version` (64 chars). Its mere presence selects `full_update`.

The launcher's own version strings: `0.26g0` (ASCII, VA 0x3DBE0, BugTrap app version) and
`Vostok Engine v0.26g0` (0x3FE88, BugTrap app name).

---

## 8. Embedded web page

### 8.1 Hosting (PROVEN)

`browser::create` 0x19F22: `OleCreate(CLSID_WebBrowser {8856F961-340A-11D0-A96B-00C04FD705A2} at 0x3B68C)`,
in-place activation, `IWebBrowser2` ({D30C1661-...} at 0x3B66C), event sink on `DWebBrowserEvents2`
({34A715A0-...} at 0x3B67C). This is the in-process Internet Explorer (MSHTML) control: **its HTTP
requests are made from inside the launcher process** by system components; the launcher has no HTTP
code of its own (imports: no wininet/winhttp/urlmon).

Each `launcher_form` creates two browser objects in the rectangle 0x45874 = (0, 128, 899, 463):

- A (form+0x2CC): navigates at once to the full path of
  `<own dir>\..\game\launcher\index-blank-<code>.html` (0x1B0DF `GetFullPathNameW`, 0x1B143), shown.
- B (form+0x2D0): navigates to the remote URL (0x1B155), hidden (`ShowWindow(.., 0)` at 0x1B17D).

### 8.2 Remote URL (PROVEN, 0x1AFA1..0x1B005)

```
"http://survarium.com/launcher/v2/" + lowercase(product name) + "/?lang=" + code
=> http://survarium.com/launcher/v2/survarium/?lang=en
```

- Base: VA 0x3F4C0, file 0x2E4C0, UTF-16LE, 33 chars, bytes
  `68 00 74 00 74 00 70 00 3A 00 2F 00 2F 00 73 00 75 00 72 00 76 00 61 00 72 00 69 00 75 00 6D 00 2E 00 63 00 6F 00 6D 00 2F 00 6C 00 61 00 75 00 6E 00 63 00 68 00 65 00 72 00 2F 00 76 00 32 00 2F 00`
  then exactly one NUL terminator (2 zero bytes): **no slack**, a longer base needs a cave and a new
  operand at 0x1AFA2 (relocated dword, keep it an image address).
- `/?lang=`: VA 0x3F504, 7 chars, no slack.
- Codes (`get_language_code` 0x1A84B): `en` 0x3F490, `ru` 0x3F498, `ua` 0x3F4A0, `de` 0x3F4A8, `pl` 0x3F4B0
  (UTF-16, 2 chars each, room for 3).
- Buffer 0x100 wchar.

### 8.3 Events (PROVEN, `browser_events::Invoke` 0x19B4E)

| DISPID | Meaning | Action |
|---|---|---|
| 259 DocumentComplete | page B loaded | `on_browser_loaded` 0x1B408: show B, delete A, B becomes the current browser, fire form+0xD00. Also installs the sink as a document event handler through `IHTMLDocument2` vtable slot 103 (LIKELY `put_ondragstart`; the handler 0x19A8C returns VARIANT_FALSE) |
| 271 NavigateError | any navigation error of B (DNS, connect, HTTP status) | `on_browser_error` 0x1B44F: A navigates to the local **`index-<code>.html`** (form+0x2D4 = `<own dir>\..\game\launcher\index-<code>.html`), B is destroyed via `WM_USER`, fire form+0xD00 |
| 273 NewWindow3 | link that opens a new window (`target=_blank`, `window.open`) | cancel, then `ShellExecuteW(NULL, "open", <url>, NULL, NULL, SW_SHOWNORMAL)` (0x19B71..0x19B90) |

So: blank local page first; remote page replaces it when it loads; the local "offline" page
`index-<code>.html` is used only on a navigation error. Links without a new-window target navigate
inside the control (no `BeforeNavigate2` handling). The shipped `index-<code>.html` files use
`target`-style external links to survarium.com, forum.survarium.com and YouTube and local `public/`
resources; their only non-local resource reference is a `<link rel="shortcut icon">` to
forum.survarium.com (LIKELY never fetched by an embedded control).

Security note: the remote page is plain HTTP on a domain outside the project's control, rendered by
MSHTML with scripting, and `NewWindow3` hands any URL it supplies to `ShellExecuteW`. Removing it is
advisable.

### 8.4 Patches

| Goal | VA | File | Original | New | Notes |
|---|---|---|---|---|---|
| never contact survarium.com: make browser B load the local `index-<code>.html` | 0x1B15D | 0xA55D | `8D 95 DC FB FF FF` (`lea edx,[ebp-424h]`) | `8D 93 D4 02 00 00` (`lea edx,[ebx+2D4h]`) | same length, no relocation; B then fires DocumentComplete and is shown. LIKELY works (the same string is what the error path navigates to); not tested |
| other remote base (<= 33 chars) | 0x3F4C0 | 0x2E4C0 | see 8.2 | new UTF-16 text + NUL | the product name and `/?lang=xx` are still appended |

The `NewWindow3` -> `ShellExecuteW` path (0x19B71..0x19B90) has no clean one-instruction patch: its
operands at 0x19B8B and 0x19B92 are relocated, and skipping the dispatch (`jne` at 0x19B6B, bytes
`0F 85 0A 01 00 00`) would skip the cancel as well, so IE would open a window of its own. With the
0x1B15D patch only the local pages can reach that path.

### 8.5 Other outbound traffic

- Buttons start `cmd.exe /c start <url>` (section 9): the request is made by the user's browser.
- BugTrap (section 12) is only handed a URL.
- WS2_32: only `WSAStartup`/`WSACleanup` (section 13).

---

## 9. Buttons, social links, other controls

### 9.1 How they are created (PROVEN, `launcher_form::launcher_form` 0x1A87C)

Not a table: a sequence of constructor calls. Images are loaded first with
`image::image(id, L"PNG"|L"JPG", kind)` (0x11F1D), then each button is a `transparent_control`
(0x12548) with: parent window, static rectangle, cursor (IDC_HAND 0x7F89), three bitmaps
(normal / hover / pressed), callback, callback context.

Rectangles are static in `.data`: `{left, top}` are initialised data, `{right, bottom}` are computed
by small dynamic initialisers as left + width, top + height.

| Control | Object (form+) | `call 0x12548` at | Rect VA (file) left, top | Size initialiser (width / height imm) | Images (resource ids) | Click handler | Parent |
|---|---|---|---|---|---|---|---|
| settings gear | 0x1DC | 0x1AC21 | 0x45814 (0x34814) 819, 12 | 0x3A63D / 0x3A64A (`83 C0 14` each: 20 x 20) | PNG 308, 309, 310 | 0x1B810 `show_settings` | top panel |
| minimize | 0x204 | 0x1AC5D | 0x45824 (0x34824) 841, 12 | 0x3A658 / 0x3A665 (20 x 20) | PNG 305, 306, 306 | 0x1B888 `CloseWindow` | top panel |
| close | 0x22C | 0x1AC99 | 0x45834 (0x34834) 863, 12 | 0x3A673 / 0x3A680 (20 x 20) | PNG 302, 303, 304 | 0x1B7FF `PostMessage(WM_CLOSE)` | top panel |
| SHOP | 0x254 | 0x1ACDB | 0x45844 (0x34844) 750, 47 | 0x3A68E (`05 85 00 00 00`) / 0x3A69D (`83 C0 40`): 133 x 64 | JPG 319, 320, 321 | 0x1BBEF | top panel |
| CREATE ACCOUNT | 0x27C | 0x1AD1D | 0x45854 (0x34854) 614, 47 | 0x3A6AB / 0x3A6BA (133 x 64) | JPG 322, 323, 324 | 0x1BC45 | top panel |
| PROJECT WEBSITE | 0x2A4 | 0x1AD5F | 0x45864 (0x34864) 478, 47 | 0x3A6C8 / 0x3A6D7 (133 x 64) | JPG 325, 326, 327 | 0x1BCAD | top panel |
| PLAY | 0x620 | 0x1ADDD | 0x45894 (0x34894) 694, 9 | 0x3A6F8 (`05 BD 00 00 00`) / 0x3A707 (`83 C0 41`): 189 x 65 | PNG 315, 316, 317; disabled 318 | 0x1B893 | bottom panel |
| YouTube | 0x648 | 0x1AE1F | 0x458A4 (0x348A4) 32, 28 | 0x3A715 / 0x3A722 (`83 C0 1B`: 27 x 27) | PNG 328, 329, 330 | 0x1BD03 | bottom panel |
| Facebook | 0x670 | 0x1AE61 | 0x458B4 (0x348B4) 68, 28 | 0x3A730 / 0x3A73D (27 x 27) | PNG 331, 332, 333 | 0x1BD16 | bottom panel |
| Twitter | 0x698 | 0x1AEA3 | 0x458C4 (0x348C4) 104, 28 | 0x3A74B / 0x3A758 (27 x 27) | PNG 334, 335, 336 | 0x1BD29 | bottom panel |
| VK | 0x6C0 | 0x1AEE5 | 0x458D4 (0x348D4) 140, 28 | 0x3A766 / 0x3A773 (27 x 27) | PNG 337, 338, 339 | 0x1BD3C | bottom panel |
| status bar | 0x4E8 | 0x1AD9B (`status_bar` 0x1BF61) | 0x458E4 (0x348E4) 200, 30 | 0x3A781 / 0x3A790 (478 x 10) | PNG 359 | | bottom panel |
| settings window | 0x6E8 | 0x1AEF7 (`settings_window` 0x1C49E) | own | | JPG 340, PNG 341..356 | section 11 | |
| language drop-down | 0xB48 | 0x1AF0C (`language_menu` 0x1942F) | 0x45914 (0x34914) 766, 12 | | PNG 311..314, 357, 358, 360..364 | section 10 | |

The button captions (strings 0xC2..0xC6) are painted into the button bitmaps by
`draw_button_texts` 0x1B8B0, so they move/disappear with the button.

### 9.2 What the click handlers do (PROVEN)

`open_url(url)` 0x1BB95: `wsprintfW(buf, L"cmd.exe /c start %s", url)` (format 0x3F564) and
`CreateProcessW(NULL, buf, NULL, NULL, FALSE, CREATE_NO_WINDOW | NORMAL_PRIORITY_CLASS = 0x08000020, ...)`.
The URL is passed to `cmd` unquoted (an `&` in a URL would be a command separator).

| Handler | URL composed |
|---|---|
| 0x1BBEF shop | `https://shop.survarium.com/` + code, e.g. `.../en` |
| 0x1BC45 create account | `https://forum.survarium.com/` + code + `/ucp.php?mode=register` |
| 0x1BCAD project website | `http://survarium.com/` + code |
| 0x1BD03 / 0x1BD16 / 0x1BD29 / 0x1BD3C | the YouTube / Facebook / Twitter / VK string as is |

### 9.3 All URL and related strings (PROVEN, `sites.py`)

"Room" = longest replacement in characters that still leaves a terminator (length + zero bytes after it, minus the terminator).

| VA | File | Enc | Len | Room | Xref (operand VA) | Text |
|---|---|---|---|---|---|---|
| 0x3F4C0 | 0x2E4C0 | UTF-16 | 33 | 33 | 0x1AFA2 | `http://survarium.com/launcher/v2/` |
| 0x3F504 | 0x2E504 | UTF-16 | 7 | 7 | 0x1AFE1 | `/?lang=` |
| 0x3F514 | 0x2E514 | UTF-16 | 24 | 25 | 0x1B04B | `\..\game\launcher\index-` |
| 0x3F548 | 0x2E548 | UTF-16 | 6 | 7 | 0x1B07B | `blank-` |
| 0x3F558 | 0x2E558 | UTF-16 | 5 | 5 | 0x1B0B2, 0x1B0C7 | `.html` |
| 0x3F564 | 0x2E564 | UTF-16 | 19 | 19 | 0x1BBA9 | `cmd.exe /c start %s` |
| 0x3F58C | 0x2E58C | UTF-16 | 27 | 27 | 0x1BBFC | `https://shop.survarium.com/` |
| 0x3F5C4 | 0x2E5C4 | UTF-16 | 28 | 29 | 0x1BC52 | `https://forum.survarium.com/` |
| 0x3F600 | 0x2E600 | UTF-16 | 22 | 23 | 0x1BC83 | `/ucp.php?mode=register` |
| 0x3F630 | 0x2E630 | UTF-16 | 21 | 23 | 0x1BCBA | `http://survarium.com/` |
| 0x3F660 | 0x2E660 | UTF-16 | 39 | 39 | 0x1BD0B | `http://www.youtube.com/user/vostokgames` |
| 0x3F6B0 | 0x2E6B0 | UTF-16 | 34 | 35 | 0x1BD1E | `https://www.facebook.com/Survarium` |
| 0x3F6F8 | 0x2E6F8 | UTF-16 | 31 | 31 | 0x1BD31 | `https://twitter.com/vostokgames` |
| 0x3F738 | 0x2E738 | UTF-16 | 23 | 23 | 0x1BD44 | `http://vk.com/survarium` |
| 0x3DB6C | 0x2CB6C | ASCII | 36 | 39 | 0x110A6 | `launcher_crash_reports@survarium.com` |
| 0x3DB94 | 0x2CB94 | ASCII | 28 | 31 | 0x110D4 | `http://support.survarium.com` |
| 0x3DBB4 | 0x2CBB4 | ASCII | 41 | 43 | 0x20367 | `http://crasheat.survarium.com/process.php` |

Related non-URL strings: pipe base `\\.\pipe\vostok_updater_pipe` 0x3DF98 (UTF-16, 28, room 29, xrefs
0x14873 and 0x15724); `survarium_updater.exe` 0x3DF6C (xref 0x157DE) and `\survarium_updater.exe`
0x3E9A8 (xref 0x14848); `\temp\survarium_launcher.exe` 0x3E908 (xrefs 0x14177, 0x141A0, 0x14BFF);
`Software\Vostok Games\%s` 0x3E784 (xrefs 0x1409D, 0x1D4B3).

All xref operands in this table are relocated dwords: to point one at a longer string placed in a
cave, write the cave's VA (for base 0x10000) into the operand and nothing else.

### 9.4 Simplest safe patches

(i) Button does nothing (still drawn): overwrite the first byte of its handler with `C3` (`ret`; the
handlers are plain cdecl functions that the control calls and cleans up after). First bytes: 0x1BBEF,
0x1BC45, 0x1BCAD = `55`; 0x1BD03, 0x1BD16, 0x1BD29, 0x1BD3C = `83`; file offsets = VA - 0x10C00
(0xAFEF, 0xB045, 0xB0AD, 0xB103, 0xB116, 0xB129, 0xB13C). Or patch `open_url` 0x1BB95 (file 0xAF95,
`55` -> `C3`) to silence all seven at once. No relocations at these bytes.

(i') Button not visible and not clickable: move it out of its parent panel by changing the static
`left` (4 bytes little-endian at the rect VA in the table above), e.g. `30 75 00 00` (30000). Data
only, no relocations. LIKELY effective (child windows outside the parent's client area are clipped);
not tested. Not creating the control at all is not recommended: the destructor and repaint code touch
every control object unconditionally (0x1B22D..0x1B2D2).

(ii) Other URL: overwrite the string in place within "Room"; remember the language code is appended
for shop / forum / website (and `/ucp.php?mode=register` for the forum). To drop a suffix, shorten the
suffix string instead (e.g. set the first character of 0x3F600 to NUL).

---

## 10. Language

### 10.1 Table (PROVEN)

| Index | Menu label (UTF-16 VA) | LANGID (0x3E098) | Page code | `-localization=` / `-language=` word |
|---|---|---|---|---|
| 0 | `EN` 0x3F360 | 0x0409 | `en` | `english` |
| 1 | `RU` 0x3F358 | 0x0419 | `ru` | `russian` |
| 2 | `UA` 0x3F350 | 0x0422 | `ua` | `ukrainian` |
| 3 | `DE` 0x3F348 | 0x0407 | `de` | `german` |
| 4 | `PL` 0x3F340 | 0x0415 | `pl` | `polish` |

The label/index pairs are written to 0x48280.. by the initialiser at 0x3A5D6. The launcher's own UI
languages are exactly these five string tables in `.rsrc` (selected with `SetThreadUILanguage`).
An `RCDATA` resource 500 exists per language (2 bytes: en 0, ru 1, uk 2, de 3, pl 4); no code
reference to it was found (UNKNOWN use).

### 10.2 Storage and start-up (PROVEN, `settings::reload_language` 0x1D852)

1. Default = command line `-language=` (english if absent).
2. `HKCU\Software\Vostok Games\<product>`, value `language` (REG_DWORD). Key present and value < 5 ->
   **the registry value wins over the command line**.
3. Key missing, value missing or >= 5 -> the default is taken and written back at once
   (`settings::set_language` 0x1D926 -> `save_language` 0x1D8DA, which creates the key with
   `RegCreateKeyExW`).

Persistence without the installer: yes, the value lives in HKCU and the key is created on demand.
If HKCU cannot be written the choice still applies for the running process but is lost at exit
(write errors are ignored).

### 10.3 On change (PROVEN)

`language_menu::language_selected` 0x19A20 -> `settings::set_language` -> saved to HKCU -> callback
`launcher::language_changed` 0x15097: `SetThreadUILanguage`, flag +0x2DE0. On the next message
(`on_message_processed` 0x14F58) a **second `launcher_form` is built** in the other storage slot
(new strings, new page URLs with the new code), Play active iff launcher state == 6; when its page
has loaded or failed (`on_new_form_ready` 0x167BF) the new window is shown at the old position and the
old form is destroyed. No process restart, no `-language=` re-launch. The updater is not informed.

---

## 11. Settings window (gear)

`settings` object (launcher+0x1A48) (PROVEN):

| Offset | Field |
|---|---|
| +0x00 / +0x01 | allow torrent: current / last saved (default 1) |
| +0x02 / +0x0E | port text wchar[6]: current / last saved |
| +0x1A | "port accepted" flag (1 at start; message 20 clears, message 21 sets) |
| +0x1C | language |
| +0x20 / +0x24 | language-changed callback / context |
| +0x28 | boost::function download type changed -> launcher 0x172FA |
| +0x48 | boost::function port changed -> launcher 0x17309 |

Window (`settings_window` 0x1C49E): title 0xD2 "Settings", checkbox 0xD3 "Allow torrent"
(`torrent_checkbox_click` 0x1CDCC toggles settings+0), 0xD4 "Port:" with an EDIT control and two
spin buttons (0x1CFDE, 0x1D02E), 0xC7 "SAVE" (0x1CBCC), 0xC8 "CANCEL" and a close icon (0x1CBA5:
`settings::reload`, `WM_CLOSE`). There is no install-path or other option in 0.26g.

SAVE (`save_settings` 0x1CC0C): with torrent allowed the port text is parsed; `0` or `65535` ->
MessageBoxW 0xB8 "invalid"; then `settings::save` (0x1D71A) writes `download_type` / `port_number` to
HKCU when changed and fires the callbacks, which send 0x1A / 0x21 **only if the pipe is connected**.
Torrent off -> window closes. Torrent on and port changed -> the window stays open until the updater
answers 21 (close) or 20 (re-show + message). Torrent on, port unchanged: closes if the accepted flag
is set, otherwise MessageBoxW 0xB7 "busy".

Storage: `HKCU\Software\Vostok Games\<product>`: `download_type` REG_DWORD (1 torrent, 0 direct),
`port_number` REG_SZ. The download type also selects argv[4] at the next updater start.

---

## 12. BugTrap / crash reporting

### 12.1 Loading (PROVEN, `bugtrap::load` 0x1FB30, `bugtrap::initialize` 0x20610)

- `LoadLibraryA("BugTrap.dll")` (string 0x3FD34; `BugTrapN.dll` 0x3FD24 would be used if flag
  0x45720 were not 1, it is set to 1 in `application::initialize`). Bare name: normal DLL search, so
  the exe's own directory first (for the temp copy that is `X\temp\`).
- **DLL missing: not fatal.** Flag 0x45720 = 0, an optional log line "cannot load bugtrap library",
  everything BugTrap-related is skipped (0x1FB60..0x1FB8E, 0x20627).
- Exports resolved with `GetProcAddress`, in this order (pointer slots 0x481CC..0x48204):

  `BT_CallCppFilter`, `BT_SetUserMessage`, `BT_InstallSehFilter`, `BT_SetDialogMessage`,
  `BT_SetAppName`, `BT_SetReportFormat`, `BT_SetFlags`, `BT_SetDumpType`, `BT_SetSupportEMail`,
  `BT_SetSupportURL`, `BT_SetSupportServer`, `BT_SetAppVersion`, `BT_SetActivityType`,
  `BT_AddLogFile`, `BT_SetReportFilePath`.

- **Any export missing: not fatal at start-up.** The flag is cleared, the later slots are zeroed and
  `bugtrap::setup` is skipped (0x1FC08.., 0x20627). So a replacement DLL must export all 15 names or
  the launcher silently runs without it.
- With all exports present `bugtrap::setup` (0x203E0) calls, unguarded: `BT_InstallSehFilter()`,
  `SetUnhandledExceptionFilter(0x210A0)`, `BT_SetDialogMessage`, `BT_SetAppName("Vostok Engine v0.26g0")`,
  `BT_SetSupportEMail`, `BT_SetSupportURL`, `BT_SetSupportServer(url, 80)`, `BT_SetAppVersion("0.26g0")`,
  `BT_SetReportFormat`, `BT_SetFlags`, `BT_SetDumpType`; then `BT_SetActivityType(1 or 2)` (0x20642).
  Later: `BT_AddLogFile` (0x2066C / 0x206B6), `BT_SetUserMessage` (0x20733), `BT_SetFlags(5)` in the
  crash path (0x2101A). Strings are passed as ANSI (flag == 1) in all of these.
  `BT_CallCppFilter` and `BT_SetReportFilePath` are resolved; no call site was looked for beyond the
  list above (UNKNOWN whether they are ever called).
- Crash path without BugTrap (LIKELY): `debug::unhandled_exception_filter` 0x210D0 still calls
  0x20F40, which writes a minidump through dbghelp (0x22C70) and then calls the `BT_SetFlags` slot
  without a null check (0x2101A), i.e. a second fault while already crashing.

### 12.2 Does the launcher upload anything itself?

**No (PROVEN by absence).** `http://crasheat.survarium.com/process.php` is only copied into a static
buffer and passed to `BT_SetSupportServer` (0x20360..0x203D2). The launcher has no HTTP or socket send
code. Whether anything is uploaded depends entirely on the `bugtrap.dll` in use. To be certain,
overwrite the first byte of the string at 0x3DBB4 (file 0x2CBB4) with 0.

Logs and dumps (LIKELY, partly traced: 0x11000, 0x1DE16, 0x11443): under the user's Documents folder,
`<Documents>\<product>\survarium_launcher\...`, file names built from user name, build and time stamp
with `.log` / `.dmp`.

---

## 13. WS2_32

Imports by ordinal only: 115 `WSAStartup` (called once at 0x18ECC, version 2.0, inside boost::asio's
`winsock_init`, io_service construction 0x18EA4) and 116 `WSACleanup` (0x18E9D). No `socket`,
`connect`, `send`, `recv`, `getaddrinfo` or any other Winsock import exists (PROVEN, import table).
The launcher's own code does no socket I/O; asio is used only for the named pipe (IOCP + stream handle).

---

## 14. Suggested minimal set-ups (summary of the above)

A. **No launcher patch**: replacement `survarium_updater.exe` (asInvoker manifest) implementing `copy`
   and the normal mode; registry values of 2.3 written once to `HKLM\SOFTWARE\WOW6432Node\Vostok Games\Survarium`
   (needs admin once). The remote page is still requested from survarium.com.

B. **No admin at all**: A + the seven one-byte hive patches of 2.4 (values go to HKCU) + the
   `0x1B15D` patch of 8.4 (local page only) + optionally `0x15521` (ignore a stale service) and URL edits of 9.4.

C. **No temp copy**: B + the three patches of 1.8 (`0x1474A`, `0x14442`, string `0x3F514`). The updater's
   `copy` mode is then never requested.

## 15. Open unknowns

- Nothing was executed: every "LIKELY" above, in particular the patch side effects (off-panel
  buttons, browser B on a local file, caves past VirtualSize, whether Windows rebases this image).
- Exact rule of `check_cpu` (constants 2 cores / 1995 MHz seen, combination not traced).
- Which `IHTMLDocument2` property slot 103 is (believed `ondragstart`).
- Use of `RCDATA` 500 and of the exports `BT_CallCppFilter` / `BT_SetReportFilePath`.
- Exact log/dump path format.
- The original updater's behaviour (what it sends in which order, what `copy` copies exactly, what the
  service does with `StartServiceW` arguments) was only skimmed from its strings; the launcher-side
  contract in sections 4 and 5 is what is proven. A separate note on the updater side,
  `notes\updater-026g.md`, was being written by another task at the same time and was not
  cross-checked against this one.
- What the installer wrote into `file to execute`, `arguments`, `working folder`, `update server`,
  `launcher` on a real installation (no registry export was available).
- Meaning the original updater gives to updater states 2, 3, 7, 18, 21 (the launcher shows "Error" or
  nothing for them) and to the u8 of message 4.
