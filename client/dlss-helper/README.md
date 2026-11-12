# dlss_helper.exe: NVIDIA DLSS for the 32-bit game

survarium.exe is 32-bit and NVIDIA ships DLSS for x64 only, so DLSS runs in this 64-bit helper.
The game (exe patch 83, with the Vulkan/DXVK renderer; `srp-026g/docs/notes/dlss-option.md`)
starts it and speaks [PROTOCOL.md](PROTOCOL.md): it shares each frame's colour, depth and motion
vectors as exported Vulkan images, and one timeline semaphore; the helper runs DLSS on its own
VkDevice on the same GPU.

## The files (`game\binaries\x86\dlss\`, made by `make_client_026g.py`)

| file | what | from |
|---|---|---|
| `dlss_helper.exe` | this program (x64) | `build.sh` |
| `nvngx_dlss.dll` | NVIDIA DLSS 310.9.16 | NVIDIA/DLSS v310.9.16 `lib/Windows_x86_64/rel`, sha256 `d00b5b0a…` |
| `wine\dxgi.dll` | DXVK 3.1.1's 64-bit DXGI, Wine only | the DXVK archive the game's `dxvk\` comes from |
| `wine\nvapi64.dll` | dxvk-nvapi 0.9.2, Wine only | its release archive, sha256 `60c28422…` |
| `LICENSES.txt` | the notices below | `LICENSES.txt` here |
| `NVIDIA-DLSS-SDK-LICENSE.txt` | NVIDIA's licence | NVIDIA/DLSS v310.9.16 `LICENSE.txt` |

About 67 MB on disk (nvngx_dlss.dll is 59 MB), about 31 MB more in the players' ZIP.
`dlss_helper.log` appears beside the exe (the temporary folder when that one cannot be written).

## Build

`nix develop ./srp-026g -c client/dlss-helper/build.sh` ->
`target/x86_64-pc-windows-msvc/release/dlss_helper.exe`. As the other Windows programs
(`../bugtrap`): Rust's standard library built here, rust-lld, Microsoft's static C runtime; the
flake supplies the 64-bit runtime (`SKHID_MSVC_LIB64`) and NVIDIA's NGX library
(`SKHID_NGX_LIB`: the DLSS SDK's `nvsdk_ngx_s.lib`, pinned).

- **NGX through NVIDIA's static library**, the SDK's supported way: it finds the driver's NGX
  core, checks it and talks to it. (The probe called the core's exports directly, with an ABI
  read out of the driver's DLL: not something to ship.) The static library is MSVC's, which is
  why the helper is an MSVC-target program.
- **A project id, not an application id:** `NVSDK_NGX_VULKAN_Init_with_ProjectID`, project id
  `c7bbddb5-39f3-4445-9b6b-e2659022068c` (a GUID made for Skhid; never change it), engine type
  CUSTOM, engine version `0.26g`: NVIDIA's way for a title that has no application id from
  them (`src/ngx.rs`).
- Vulkan through `ash` (types and function tables; `vulkan-1.dll` is loaded at run time).

## What it needs at run time

- **Windows:** an NVIDIA RTX GPU and its driver: `nvapi64.dll` (the driver's, loaded by name)
  and the NGX core in the driver store. Nothing to install.
- **Wine:** the helper notices Wine (`wine_get_version`) and, before NGX starts,
  - sets `DXVK_ENABLE_NVAPI=1` for itself and loads `wine\dxgi.dll` then `wine\nvapi64.dll` by
    their full paths (NGX finds the GPU through NvAPI, dxvk-nvapi finds it through DXVK's DXGI;
    without NvAPI NGX's start fails with 0xBAD00002). No DLL override and no environment from
    the game or the player: none of it is touched on Windows;
  - looks for NVIDIA's NGX core where NGX's loader looks under Wine: the folder named by
    `HKLM\SOFTWARE\NVIDIA Corporation\Global\NGXCore`, `FullPath` (the key Proton sets). When
    the key is missing or names a folder without `_nvngx.dll`, the helper sets it to the
    driver's own folder, found through Wine's `Z:` drive (`/usr/lib/nvidia/wine`,
    `/usr/lib64/nvidia/wine`, `/usr/lib/x86_64-linux-gnu/nvidia/wine`,
    `/run/opengl-driver/lib/nvidia/wine`): the core runs from there, so it is always the
    installed driver's. **So a Linux player's prefix needs nothing set up**, only the NVIDIA
    driver (470 or later) with its Wine files, a Wine with `VK_KHR_external_memory_win32` and
    `VK_KHR_external_semaphore_win32` in winevulkan (Wine 11.8 has them), and the `Z:` drive.
    A driver folder elsewhere (a Flatpak runtime, a prefix without `Z:`): the helper refuses
    with that reason; setting the key by hand to the folder that has `_nvngx.dll` fixes it.

## When it cannot run

The reason goes to the game in the reply's text (the game logs it in `srp_patches.log`) and into
`dlss_helper.log`; the helper then exits (code 3). Seen in tests (below) unless marked:

| case | the reply |
|---|---|
| not NVIDIA's GPU | `not an NVIDIA GPU: <name> (vendor 0x....)` |
| the game's GPU is not found | `no Vulkan device with the game's deviceUUID` |
| `nvngx_dlss.dll` missing | `nvngx_dlss.dll is not beside the helper (<folder>)` |
| Windows without NVIDIA's driver (not run) | `nvapi64.dll (NVIDIA's driver) cannot be loaded: not an NVIDIA driver` |
| Wine: `wine\` incomplete | `<file>: cannot be loaded` |
| Wine: no NGX core and no driver folder | `no NVIDIA NGX in this Wine prefix, and the NVIDIA driver's nvidia/wine/_nvngx.dll was not found (see LINUX.txt)` |
| NGX does not start (no NGX in the driver; not run) | `NGX did not start (0x... <NGX's name>)` and what it usually means |
| driver too old for DLSS (not run) | `the NVIDIA driver is too old for DLSS: it needs <major>.<minor> or later` |
| a GPU without DLSS, a GTX (not run) | `DLSS is not available on this GPU (0x... <NGX's name>)` |
| the images or the semaphore cannot be imported | in the Setup's reply: which one and Vulkan's error |

It ends by itself when the game says quit or the pipe closes (a game that exits or crashes),
also in the middle of a frame.

## Licences (what shipping these files asks of us)

`LICENSES.txt` is shipped in the folder with the attributions; NVIDIA's licence text beside it.

- **NVIDIA DLSS SDK** (`nvngx_dlss.dll`, and NVIDIA's NGX library inside the exe): the NVIDIA
  RTX SDKs License (v. March 14, 2024) with its RTX supplement, `LICENSE.txt` of NVIDIA/DLSS.
  - Allowed: distributing the DLL and the library in object form, **as part of an application**
    that has material functionality of its own; never on their own (§1c, §2a, §4b).
  - Our terms to players must protect NVIDIA's software at least as its licence does (§2c):
    `LICENSES.txt` says so and ships the licence.
  - No reverse engineering of the SDK, no removing its notices (§4a); the notice "This software
    contains source code provided by NVIDIA Corporation" goes with our declarations made after
    the SDK's headers (`src/ngx.rs`, `LICENSES.txt`) (§2b).
  - Only for systems with NVIDIA GPUs (supplement §1): the helper refuses any other GPU.
  - **For the owner to do:** notify NVIDIA before a commercial release
    (https://developer.nvidia.com/sw-notification: developer, SDK used, application, platform,
    date, link; supplement §4), and **attribute DLSS with NVIDIA's marks on splash screens, in
    an about box if there is one, and in the game's credits** (supplement §7.1b), each use of
    the marks shown to NVIDIA two weeks before for approval (§7.2c). Not done by this change:
    the notices here are text in a file.
  - NVIDIA may update DLSS over the air and may switch a faulty integration off (supplement
    §6); the SDK must not come under an open-source licence (§4e): the DLL and the library are
    downloaded at build time, never committed.
- **dxvk-nvapi** (`wine\nvapi64.dll`): MIT. The copyright notice and the licence text ship in
  `LICENSES.txt`.
- **DXVK** (`wine\dxgi.dll`, and the game's `dxvk\`): zlib/libpng. Unchanged binaries; the
  licence asks nothing of a binary distribution; its text ships in `LICENSES.txt` all the same.
- **ash** (Vulkan bindings in the exe): MIT or Apache-2.0; windows-sys: MIT or Apache-2.0.

## Tested

2026-11-12, Wine 11.8 staging (wow64), RTX 3060 Ti, driver 615.78.08, with the probe's 32-bit
D3D11 producer on DXVK 3.1.1 x32 (`private/dlss-probe`, not in the repo) and a prefix with no
DLL overrides and no NGX files:

- 1080p Quality end to end, 300 frames: DLSS 1.11 ms of GPU a frame, the round trip 1.30 ms,
  +82 MiB VRAM for the feature; the output 21.8 dB PSNR against a 16-sample reference
  (bilinear of the input: 18.0); the helper up in 0.8 s.
- A new size in the middle (command 2, Hello and Setup again): 36 ms, VRAM unchanged.
- The refusals marked seen above; the pipe closed after Hello: the helper gone in 50 ms.
- `tests/ipc_tests.rs` (the records and their parsing):
  `rustc --edition 2024 --test tests/ipc_tests.rs -o target/ipc-tests && target/ipc-tests`.

Not run: Windows; a driver without NGX or too old; a GPU without DLSS; the game itself.
