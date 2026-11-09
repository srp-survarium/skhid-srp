#!/usr/bin/env python3
"""Run patch 83 (rust/patches/src/hud_options.rs, its lohooks of
hud_options.toml: the Settings' "Hide hit markers" and "Hide player scores", kept in
user.cfg, and the Graphics tab's Renderer list, `srp_dxvk`, which the next start reads: DXVK loaded from dxvk\\) in the default build on made-up game objects, each from the game's
own code before its site to past it, the original beside it.

    python check_hud_options_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; check_language_026g.py's harness (check_hide_ui_026g.py's:
Scaleform's Invoke 0x511600 a stub that records each call; translate_text
0x1FF0F0 the key's English text; game::save_user_config 0x1E9030 counted):

- on_configs_loaded's start (0x1EA210 to 0x1EA216, the chain: patch 79's
  hook, then this one): the console variables `srp_hide_hit_markers`,
  `srp_hide_scores`, `srp_map_names`, `srp_hide_crosshair`, `srp_dxvk` and the Vulkan options'
  `srp_dxvk_low_latency`, `srp_dxvk_tear_free` and `srp_dxvk_dedicated_gpu` registered once, after patch 79's `localization`, with
  the game's own cc_value<bool>::cc_value (a cc_bool, saved, kind 2: user.cfg's
  lines set them), off (`srp_dxvk_dedicated_gpu` on); the game's own cc_bool::execute sets each from a
  user.cfg line's value; then the two lists `srp_dxvk_frame_limit` and `srp_dxvk_overlay`, saved
  cc_strings, empty; registers and stack as the original's, no file
  opened;
- the process's start (memory::platform::allocate_arenas 0xE9580 to past its
  first instruction, `mov eax,11A8h`), kernel32's and shell32's imports and
  the four Vulkan functions stubbed (`Run.kernel`): user.cfg read from
  SHGetFolderPathA(CSIDL_PERSONAL)\\survarium, the five dxvk\\ files looked
  for by full path, the marker; Vulkan/DXVK picked with all five, no marker
  and a 1.4 device: the marker written first, vulkan-1.dll loaded, an
  instance made and destroyed, the DLL let go, DXVK_CONFIG set from user.cfg's
  Vulkan options (SetEnvironmentVariableA; the defaults: the dedicated GPU
  alone; every one changed: each said) and logged, then dxgi, d3d11, d3d10core,
  d3d9, d3d8 loaded by full path, the game's delayed dxgi, d3d11 and d3d9
  bound to them (their module slots), also with Windows' dxgi.dll loaded
  already, said (one that fails stops the loads, nothing let go, the marker
  kept); a 1.3 device or no vulkan-1.dll, DirectX 11 picked,
  no user.cfg, a file missing, a marker left: nothing loaded, the marker
  deleted; never the engine's allocator; registers (EAX the probe's size) and
  stack as the original's, which calls none of these;
- each Present (res_render_output::present 0x3CF7F0 to its `ret 4`, the site 0x3CF8FF right before
  IDXGISwapChain::Present; its swap chain's GetFullscreenState and Present and user32's GetFocus,
  ShowWindow, GetWindowLongA and SetWindowLongA stubbed): after a start with DXVK, a window DXGI
  refuses (topmost, no frame style: DXVK's fullscreen of the game's window, the 2026-11-08 report's
  start; set_size's style 0) made a popup (SetWindowLongA, WS_POPUP added) before the Present, which
  sees a window DXGI takes, and said; the next Present reads only; a windowed window read only; after
  a DirectX 11 start no style read or set; registers and stack as the original's, which presents with
  the window as it is;
- the Renderer's answer (root.srp_check_option) says the renderer running:
  "1" in a process that loaded DXVK at its start, "0" otherwise; a Vulkan
  option's the value DXVK was given at the start (without DXVK the saved one);
  a list's index ("0", "1", ...) saves its choice's word, other values nothing,
  and its answer is (name, label, saved index, running index, the choices' texts); `uro_fov` (the
  Gameplay tab's field of view) sets whole degrees within the variable's range (50 to 100), saves
  user.cfg once, and answers root.srp_slider_option(name, label, degrees, least, most), the
  nearest whole degrees for a fraction; other values set nothing; the original answers nothing;
- the login screen (0x21EAC8, after patch 88's handler): after a failed DXVK
  start (a marker left) `srp_dxvk` off and user.cfg saved once, the marker
  deleted; after that, and after a start with DXVK, only the marker deleted;
- the Settings' call (game_options::scaleform_callback 0x1FF5D0 to its
  return, through patch 79's hook at its start): a setting's name with "1" or
  "0" sets that variable alone and writes user.cfg once, with another value
  or none sets nothing, and each answers root.srp_check_option(name, label,
  "1" or "0") and returns as the function does (ret 10h); srp_language still
  reaches patch 79; the original answers nothing;
- each frame's player icons update (update_ui's `push edi / call 0x23AA10`
  at 0x23B206 to update_player_icons' prologue's end 0x23AA16, through patch
  65's hook): the hud movie told root.srp_hud_options(hide hit markers, hide
  scores, hide crosshair, map names), after patch 65's root.force_interface_hidden, exactly at a change
  of the hud movie or of a setting, and again at the next frame while the
  movie refuses it (Invoke answering 0), and again after a match's or a
  replay's options read (a new match's hud can be made at the last one's
  address; process_static_match_info 0x27B92D, a replay's chunk 1 0x1F0971,
  after patch 65's hooks); nothing without a hud movie;
  registers and stack as the original's, which invokes nothing.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EBP, UC_X86_REG_EDI, UC_X86_REG_ESI, \
    UC_X86_REG_ESP

import patch_026g as base
import check_hide_ui_026g as hide
import check_blackout_026g as blackout
import check_language_026g as language

EXECUTE_BOOL, CC_BOOL_VTABLE = 0xECA30, 0x9E7C4C
SETTINGS = {"srp_hide_hit_markers": "Hide hit markers", "srp_hide_scores": "Hide player scores",
            "srp_map_names": "Names on map", "srp_hide_crosshair": "Hidden crosshair",
            "srp_dxvk": "Renderer", "srp_dxvk_low_latency": "Reduce input lag",
            "srp_dxvk_tear_free": "Tear-free with vsync off", "srp_dxvk_dedicated_gpu": "Prefer the dedicated GPU"}
# the checkboxes on until user.cfg says otherwise
ON_BY_DEFAULT = {"srp_dxvk_dedicated_gpu"}
# the Renderer and the Vulkan options answer the value this process runs after the saved one
RUNNING = {"srp_dxvk", "srp_dxvk_low_latency", "srp_dxvk_tear_free", "srp_dxvk_dedicated_gpu"}
# the Vulkan options' lists (cc_strings): label, user.cfg's words and the choices' texts, in the list's order
LISTS = {"srp_dxvk_frame_limit": ("Frame rate limit", ("0", "60", "120", "144", "165", "240"),
                                  ("Off", "60", "120", "144", "165", "240")),
         "srp_dxvk_overlay": ("Performance overlay", ("off", "fps", "frametimes", "full"),
                              ("Off", "FPS", "FPS+graph", "Full"))}
# the group's own texts, in root.srp_hud_texts' order
TEXTS = {"st_srp_options_hud": "HUD:", "st_srp_hide_quests_option": "Hide quests", "st_srp_crosshair_dynamic": "Dynamic crosshair",
         "st_srp_crosshair_static": "Static crosshair", "st_srp_crosshair_hidden": "Hidden crosshair",
         "st_srp_static_minimap_option": "Static minimap", "st_srp_renderer_dx11": "DirectX 11",
         "st_srp_renderer_dxvk": "Vulkan/DXVK", "st_srp_language_restart": "Applies after restart.",
         "st_srp_vulkan_options": "Vulkan options",
         "st_srp_vulkan_frame_limit_hint": "Caps how many frames per second the game draws.",
         "st_srp_vulkan_overlay_hint": "Shows the game's speed on screen.",
         "st_srp_vulkan_low_latency_hint": "Makes the game react sooner to your mouse.\n+ Aim reacts faster\n- May feel less smooth on some PCs",
         "st_srp_vulkan_tear_free_hint": "Keeps the picture whole when vertical sync is off.\n+ No torn lines when turning fast\n- A bit more input delay than plain vsync off",
         "st_srp_vulkan_dedicated_gpu_hint": "Picks the stronger graphics card on laptops.\n+ Better performance on laptops\n- Uses more battery"}
language.ENGLISH.update({key: text.encode() for key, text in TEXTS.items()})
language.ENGLISH.update({"st_srp_hide_hit_markers_option": b"Hide hit markers", "st_srp_hide_scores_option": b"Hide player scores",
                         "st_srp_map_names_option": b"Names on map",
                         "st_srp_renderer_option": b"Renderer", "st_srp_vulkan_low_latency": b"Reduce input lag",
                         "st_srp_vulkan_tear_free": b"Tear-free with vsync off",
                         "st_srp_vulkan_dedicated_gpu": b"Prefer the dedicated GPU",
                         "st_srp_vulkan_frame_limit": b"Frame rate limit", "st_srp_vulkan_overlay": b"Performance overlay",
                         "st_srp_vulkan_off": b"Off", "st_srp_vulkan_overlay_fps": b"FPS",
                         "st_srp_vulkan_overlay_frametimes": b"FPS+graph", "st_srp_vulkan_overlay_full": b"Full",
                         "st_fov_option": b"FOV"})
OTHER_HUD = 0x20076800                  # another hud movie (a new game world's)
# a match's options and the message they are read from (check_blackout_026g.py's OPTIONS and WORK
# lie in check_language_026g.py's DATA: these are their own)
MATCH_OPTIONS, MATCH_WORK = 0x31000000, 0x31100000


ARENAS, ARENAS_PAST = 0xE9580, 0xE9585                # allocate_arenas' start, past its `mov eax,11A8h`
LOGIN, LOGIN_PAST = 0x21EAC8, 0x21EACE                # login_menu::on_activate's site, past `mov ecx,[esi+9Ch]`
MALLOC = 0xE4F90                                      # the engine's allocator: never called at the process's start
DOCUMENTS = b"C:\\Users\\p\\Documents"
EXE_PATH = b"C:\\Game\\game\\binaries\\x86\\survarium_skhid.exe"
USER_CFG = "C:\\Users\\p\\Documents\\survarium\\user.cfg"
DXVK_DIR = "C:\\Game\\game\\binaries\\x86\\dxvk\\"
DXVK = ("dxgi.dll", "d3d11.dll", "d3d10core.dll", "d3d9.dll", "d3d8.dll")   # the load order
VULKAN, INSTANCE, DEVICES = 0x71000000, 0x73000000, 0x74000000
# kernel32's and shell32's import slots patch 83 calls at the process's start and the login screen
SLOTS = {"CreateFileA": (0x870048, 7), "ReadFile": (0x870064, 5), "WriteFile": (0x870108, 5), "CloseHandle": (0x870040, 1),
         "DeleteFileA": (0x87021C, 1), "LoadLibraryA": (0x870080, 1), "GetProcAddress": (0x870084, 2),
         "FreeLibrary": (0x870088, 1), "GetModuleFileNameA": (0x87014C, 3), "SHGetFolderPathA": (0xAEFE7C, 5),
         "GetModuleHandleA": (0x87003C, 1), "SetEnvironmentVariableA": (0x87029C, 2)}
# the game's delayed imports' module slots (__delayLoadHelper2 0xE48E0 loads nothing for one set), DXVK's DLL of each
MODULE_SLOTS = {"dxgi.dll": 0xAF30CC, "d3d11.dll": 0xAF30D0, "d3d9.dll": 0xAF1194}
CALLED = 0x6B000000     # a module the helper bound before patch 83 ran
VK = {"vkCreateInstance": 3, "vkDestroyInstance": 2, "vkEnumeratePhysicalDevices": 3, "vkGetPhysicalDeviceProperties": 2}


def vk_version(major, minor, patch):
    return (major << 22) | (minor << 12) | patch


# res_render_output::present (0x3CF7F0, the render output in EAX, the vsync byte at entry ESP+4, ret 4) and
# what it reaches: its swap chain's GetFullscreenState (vtable +2Ch) and Present (+20h), user32's GetFocus and
# ShowWindow, the renderer's options [0x2260498] (+105h a vsync byte), patch 83's GetWindowLongA / SetWindowLongA
PRESENT, PRESENT_RET = 0x3CF7F0, 0x3CF914
OUTPUT, SWAP_CHAIN, SWAP_VTABLE, RENDER_OPTIONS = 0x32000000, 0x32001000, 0x32002000, 0x32003000
RENDER_OPTIONS_SLOT = 0x2260498
WINDOW = 0x000A0B2C
USER32 = {"GetFocus": (0xAEFF48, 0), "ShowWindow": (0xAEFEF8, 2), "GetWindowLongA": (0xAEFE8C, 2),
          "SetWindowLongA": (0xAEFF44, 3)}
COM = {"Present": (0x20, 3), "GetFullscreenState": (0x2C, 3)}
GWL_STYLE, GWL_EXSTYLE = 0xFFFFFFF0, 0xFFFFFFEC
WS_POPUP, WS_VISIBLE, WS_CLIPSIBLINGS, WS_CAPTION, WS_SYSMENU = 0x80000000, 0x10000000, 0x04000000, 0xC00000, 0x80000
WS_EX_TOPMOST, WS_EX_WINDOWEDGE = 0x8, 0x100


def dxgi_takes(style, extended):
    """DXGI's rule for a swap chain's window (the debug layer's "The specified output window has a style
    that is incompatible with DXGI", AMD in doitsujin/dxvk#5258): one of WS_POPUP, WS_BORDER, WS_CAPTION,
    WS_SYSMENU, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME, or of WS_EX_CLIENTEDGE, WS_EX_CONTEXTHELP,
    WS_EX_DLGMODALFRAME, WS_EX_TOOLWINDOW, WS_EX_WINDOWEDGE, or not WS_EX_TOPMOST."""
    return bool(style & (WS_POPUP | 0x800000 | WS_CAPTION | WS_SYSMENU | 0x10000 | 0x20000 | 0x40000)
                or extended & (0x200 | 0x400 | 0x1 | 0x80 | WS_EX_WINDOWEDGE) or not extended & WS_EX_TOPMOST)


class Run(language.Run):
    def __init__(self, data):
        super().__init__(data)
        self.refused = False
        self.windows_world()
        self.emu.mem_map(MATCH_OPTIONS, 0x60000, UC_PROT_ALL)
        self.emu.mem_map(MATCH_WORK, 0x10000, UC_PROT_ALL)
        self.mallocs = 0
        self.emu.hook_add(UC_HOOK_CODE, self.malloc, begin=MALLOC, end=MALLOC)

    def malloc(self, *_):
        self.mallocs += 1

    def windows_world(self, user_cfg=None, present=DXVK, marker=False, vulkan=None, fails=None, windows_dxgi=False,
                      called=None):
        """What the stubs answer: user.cfg's text (None: no file), the dxvk\\ files there, a marker left,
        the best device's (apiVersion, type) (None: no vulkan-1.dll), a DLL whose load fails, whether Windows'
        dxgi.dll is loaded already (the Vulkan loader's), a delayed import the game called already (its slot set)."""
        self.user_cfg, self.present, self.marker, self.vulkan, self.fails = user_cfg, set(present), marker, vulkan, fails
        self.windows_dxgi, self.modules = windows_dxgi, {}
        for name, slot in MODULE_SLOTS.items():
            self.word(slot, CALLED if name == called else 0)
        self.calls_made, self.logged, self.read_at = [], b"", 0
        if getattr(self, "stubs", None):
            return
        self.stubs = {}
        at = hide.IMPORTS + 0x600
        for name, (slot, pops) in list(SLOTS.items()) + [(n, (None, p)) for n, p in VK.items()]:
            self.emu.mem_write(at, b"\xC2" + struct.pack("<H", 4 * pops))
            if slot is not None:
                self.word(slot, at)
            self.stubs[name] = at
            self.emu.hook_add(UC_HOOK_CODE, self.kernel, begin=at, end=at, user_data=name)
            at += 0x10

    def kernel(self, emu, address, size, name):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda i: self.read(sp + 4 + 4 * i)
        result = 1
        if name == "CreateFileA":
            path, disposition = self.text(arg(0)).decode(), arg(4)
            self.calls_made.append((name, path, disposition))
            if path == "srp_patches.log":
                result = 0x60
            elif path == USER_CFG:
                result = 0x50 if self.user_cfg is not None else 0xFFFFFFFF
            elif path == "dxvk.pending":
                result = 0x61 if self.marker or disposition == 2 else 0xFFFFFFFF
                self.marker |= disposition == 2
            elif path.startswith(DXVK_DIR):
                result = 0x62 if path[len(DXVK_DIR):] in self.present else 0xFFFFFFFF
            else:
                result = 0xFFFFFFFF
        elif name == "ReadFile":
            assert arg(0) == 0x50, hex(arg(0))
            chunk = self.user_cfg[self.read_at:self.read_at + arg(2)]
            self.read_at += len(chunk)
            emu.mem_write(arg(1), chunk)
            self.word(arg(3), len(chunk))
        elif name == "WriteFile":
            self.logged += bytes(emu.mem_read(arg(1), arg(2)))
            self.word(arg(3), arg(2))
        elif name == "DeleteFileA":
            path = self.text(arg(0)).decode()
            self.calls_made.append((name, path))
            if path == "dxvk.pending":
                self.marker = False
        elif name == "LoadLibraryA":
            path = self.text(arg(0)).decode()
            self.calls_made.append((name, path))
            if path == "vulkan-1.dll":
                result = VULKAN if self.vulkan is not None else 0
            else:
                result = 0 if path == DXVK_DIR + str(self.fails) else 0x72000000 + 0x10000 * len(self.calls_made)
                if result:
                    self.modules[path[len(DXVK_DIR):]] = result
        elif name == "GetModuleHandleA":
            path = self.text(arg(0)).decode()
            self.calls_made.append((name, path))
            result = 0x6A000000 if path == "dxgi.dll" and self.windows_dxgi else 0
        elif name == "GetProcAddress":
            function = self.text(arg(1)).decode()
            result = self.stubs[function] if arg(0) == VULKAN and function in VK else 0
        elif name == "FreeLibrary":
            self.calls_made.append((name, arg(0)))
        elif name == "SetEnvironmentVariableA":
            # the value whole (DXVK reads at most 259 characters)
            value = bytes(emu.mem_read(arg(1), 260))
            assert b"\0" in value, value
            self.calls_made.append((name, self.text(arg(0)).decode(), value.split(b"\0")[0].decode()))
        elif name == "GetModuleFileNameA":
            emu.mem_write(arg(1), EXE_PATH + b"\0")
            result = len(EXE_PATH)
        elif name == "SHGetFolderPathA":
            assert arg(1) == 5, arg(1)                 # CSIDL_PERSONAL
            emu.mem_write(arg(4), DOCUMENTS + b"\0")
            result = 0
        elif name == "vkCreateInstance":
            self.calls_made.append((name,))
            self.word(arg(2), INSTANCE)
            result = 0
        elif name == "vkEnumeratePhysicalDevices":
            assert arg(0) == INSTANCE
            self.word(arg(1), 2)
            self.word(arg(2), DEVICES)
            self.word(arg(2) + 4, DEVICES + 1)
            result = 0
        elif name == "vkGetPhysicalDeviceProperties":
            # the CPU's lavapipe first (1.4.354, never counted), then the case's device
            api, kind = (vk_version(1, 4, 354), 4) if arg(0) == DEVICES else self.vulkan
            self.word(arg(1), api)
            self.word(arg(1) + 16, kind)
        elif name == "vkDestroyInstance":
            self.calls_made.append((name, arg(0)))
        emu.reg_write(UC_X86_REG_EAX, result)

    def process_start(self, **world):
        """allocate_arenas' start to past its first instruction in `world` (windows_world): whether the
        registers and ESP are the original's, and the calls made."""
        self.windows_world(**world)
        sp = hide.STACK + 0x9000
        after, esp = self.run(ARENAS, (ARENAS_PAST,), sp, hide.GAME)
        assert self.stopped == ARENAS_PAST, self.stopped
        kept = esp == sp and after == {**hide.GAME, UC_X86_REG_EAX: 0x11A8}
        return kept, list(self.calls_made)

    def login(self):
        """login_menu::on_activate's site to past its span (patch 88's handler, then this one): whether the
        registers and ESP are the original's, and the calls made."""
        self.calls_made = []
        self.emu.mem_write(hide.WORLD, bytes(0x400))
        sp = hide.STACK + 0x9000
        regs = {**hide.GAME, UC_X86_REG_ESI: hide.WORLD}
        after, esp = self.run(LOGIN, (LOGIN_PAST,), sp, regs)
        assert self.stopped == LOGIN_PAST, self.stopped
        return esp == sp and after == {**regs, UC_X86_REG_ECX: 0}, list(self.calls_made)

    def new_match(self, replay):
        """A match's options read, as check_blackout_026g.py's static_info and replay run
        them (process_static_match_info from its pushes to past the site; a replay's chunk 1
        from its reader's making to its `ret 8`), a match without Blackout: whether it went
        on as the game's."""
        payload = blackout.empty_options()
        self.emu.mem_write(MATCH_OPTIONS, bytes(0x578A0))
        self.emu.mem_write(MATCH_WORK, bytes(0x10000))
        self.emu.mem_write(MATCH_WORK, payload)
        world = hide.WORLD
        self.emu.mem_write(world, bytes(0x10000))
        if replay:
            sp = hide.STACK + 0x8000
            self.emu.mem_write(sp, bytes(0x100))
            self.word(sp + 0x40, MATCH_OPTIONS)
            self.word(sp, 0x55555555)
            self.word(sp + 4, 0x66666666)
            regs, esp = self.run(blackout.REPLAY_START, (blackout.REPLAY_RET,), sp,
                                 {**hide.GAME, UC_X86_REG_EAX: MATCH_WORK, UC_X86_REG_ECX: len(payload),
                                  UC_X86_REG_EDI: world})
            return self.stopped == blackout.REPLAY_RET and esp == sp + 0x3C and regs[UC_X86_REG_EDI] == world
        self.word(world, world + 0x8000)                  # [edi]: a vtable
        self.word(world + 0x14, world + 0x1000)           # [edi+14h]+53A4h: the dictionary (none)
        ebp = hide.STACK + 0x9000
        for i, value in enumerate((MATCH_WORK, MATCH_WORK, len(payload))):
            self.word(ebp - 0x2C + 4 * i, value)
        sp = ebp - 0x100
        regs, esp = self.run(blackout.STATIC_START, (blackout.STATIC_PAST,), sp,
                             {**hide.GAME, UC_X86_REG_EDI: world, UC_X86_REG_ESI: MATCH_OPTIONS - 0xC,
                              UC_X86_REG_EBP: ebp})
        return self.stopped == blackout.STATIC_PAST and esp == sp and regs[UC_X86_REG_ESI] == MATCH_OPTIONS

    def presented(self, styles, vsync=0):
        """res_render_output::present from its start to its `ret 4`, its render output's window having `styles`
        (GWL_STYLE, GWL_EXSTYLE), focused, the swap chain fullscreen: the registers after and whether ESP is
        back, the window's styles after, and the calls in order (Present's with the window's styles as DXGI
        reads them then)."""
        if not getattr(self, "window_stubs", None):
            self.emu.mem_map(OUTPUT, 0x10000, UC_PROT_ALL)
            self.window_stubs = {}
            at = hide.IMPORTS + 0xA00
            for name, (_, pops) in list(USER32.items()) + list(COM.items()):
                self.emu.mem_write(at, b"\xC2" + struct.pack("<H", 4 * pops))
                self.emu.hook_add(UC_HOOK_CODE, self.window_call, begin=at, end=at, user_data=name)
                self.window_stubs[name] = at
                at += 0x10
        self.emu.mem_write(OUTPUT, bytes(0x4000))
        for name, (slot, _) in USER32.items():
            self.word(slot, self.window_stubs[name])
        for name, (offset, _) in COM.items():
            self.word(SWAP_VTABLE + offset, self.window_stubs[name])
        self.word(OUTPUT + 0xCC, SWAP_CHAIN)
        self.word(SWAP_CHAIN, SWAP_VTABLE)
        self.word(OUTPUT + 0xE4, WINDOW)
        self.emu.mem_write(OUTPUT + 0xED, b"\x00\x00\x01")    # fullscreen (m_windowed 0), +EFh its state applied
        self.word(RENDER_OPTIONS_SLOT, RENDER_OPTIONS)
        self.window, self.window_calls = list(styles), []
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        self.word(sp + 4, vsync)
        regs = {**hide.GAME, UC_X86_REG_EAX: OUTPUT}
        after, esp = self.run(PRESENT, (PRESENT_RET,), sp, regs)
        assert self.stopped == PRESENT_RET, self.stopped
        return (after, esp == sp), tuple(self.window), list(self.window_calls)

    def window_call(self, emu, address, size, name):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda i: self.read(sp + 4 + 4 * i)
        result = 0
        if name == "GetFocus":
            result = WINDOW
        elif name == "ShowWindow":
            self.window_calls.append((name, arg(0), arg(1)))
        elif name in ("GetWindowLongA", "SetWindowLongA"):
            assert arg(0) == WINDOW and arg(1) in (GWL_STYLE, GWL_EXSTYLE), (name, hex(arg(0)), hex(arg(1)))
            index = 0 if arg(1) == GWL_STYLE else 1
            result = self.window[index]
            if name == "SetWindowLongA" and not getattr(self, "styles_stick", False):
                self.window[index] = arg(2)
            self.window_calls.append((name, arg(1)) + ((arg(2),) if name == "SetWindowLongA" else ()))
        elif name == "GetFullscreenState":
            assert arg(0) == SWAP_CHAIN
            self.word(arg(1), 1)
            self.word(arg(2), 0)
        elif name == "Present":
            assert arg(0) == SWAP_CHAIN
            self.window_calls.append((name, arg(1), arg(2), tuple(self.window)))
        emu.reg_write(UC_X86_REG_EAX, result)

    # a hud movie that refuses root.srp_hud_options (no root yet): its Invoke answers 0
    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        refused = self.refused and address == hide.INVOKE and self.text(self.read(sp + 4)) == b"root.srp_hud_options"
        super().stub(emu, address, size, _)
        if refused:
            emu.reg_write(UC_X86_REG_EAX, 0)

    def frame(self, movie=hide.HUD_MOVIE):
        """update_ui's call of update_player_icons to past its prologue, the hud
        at hide.HUD with `movie` (0: none) and the icons' movie: the registers
        and the stack past the prologue as the game's (check_blackout_026g.py's
        icons_fed: called directly or through Research's wrapper, which
        forwards the same ui), and the Invokes."""
        self.movies(hide.HUD, movie != 0, True)
        self.word(hide.HUD_UI + 4, movie or hide.HUD_MOVIE)
        sp = hide.STACK + 0x8000
        given = {**hide.GAME, UC_X86_REG_EDI: hide.HUD}
        regs, esp = self.run(blackout.ICONS_SITE, (blackout.ICONS_PAST,), sp, given)
        frame = regs[UC_X86_REG_EBP]
        back = self.read(frame + 4)
        call = bytes(self.emu.mem_read(back - 5, 5))
        wrapped = call[0] == 0xE8 and back + struct.unpack("<i", call[1:])[0] == blackout.PLAYER_ICONS \
            and self.read(frame + 12) == blackout.ICONS_SITE + 6 and self.read(frame + 16) == hide.HUD
        kept = self.stopped == blackout.ICONS_PAST and self.read(frame) == hide.GAME[UC_X86_REG_EBP] \
            and (back == blackout.ICONS_SITE + 6 or wrapped) and self.read(frame + 8) == hide.HUD \
            and esp & 7 == 0 and esp <= frame and all(regs[r] == v for r, v in given.items() if r != UC_X86_REG_EBP)
        return kept, self.invokes()


def registered(r, head, before):
    return language.registered(r, head, before)


def console_bool(r, node):
    """The registered variable: (name, kind, saved byte, its value's address), its vtable checked."""
    assert r.read(node) == CC_BOOL_VTABLE, hex(r.read(node))
    return r.text(r.read(node + 0x10)).decode(), r.read(node + 0x14), r.read(node + 0x1C, 2)[1], r.read(node + 0x40)


def main():
    original = open(sys.argv[1], "rb").read()
    patched, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                            open(base.late_join_bin(), "rb").read())
    stock, ours = Run(original), Run(patched)
    expect = lambda ok, text: print("ok", text) if ok else sys.exit("FAIL " + text)

    # on_configs_loaded's start: patch 79's variable, then these, once
    head_before = ours.read(language.LIST_HEAD)
    a_regs, a_sp, a_head = stock.start()
    b_regs, b_sp, b_head = ours.start()
    # patch 81's `srp_lobby`, a later member of the site's chain, registers after these
    nodes = [node for node in registered(ours, b_head, head_before) if ours.text(ours.read(node + 0x10)) != b"srp_lobby"]
    names = [ours.text(ours.read(node + 0x10)).decode() for node in nodes]
    variables = {name: console_bool(ours, node) for name, node in zip(names, nodes) if name in SETTINGS}
    expect(a_regs == b_regs and a_sp == b_sp and a_head == head_before
           and names == ["srp_mouse_vertical", "srp_mouse_vertical_locked", "s_music_volume", "srp_dxvk_overlay",
                         "srp_dxvk_frame_limit", "srp_dxvk_dedicated_gpu", "srp_dxvk_tear_free", "srp_dxvk_low_latency",
                         "srp_dxvk", "srp_hide_crosshair", "srp_map_names", "srp_hide_scores", "srp_hide_hit_markers",
                         "localization"]
           and all((kind, saved) == (2, 1) and ours.read(value, 1) == (b"\x01" if name in ON_BY_DEFAULT else b"\0")
                   for name, (_, kind, saved, value) in variables.items()),
           "configs.start: registers and stack as the original's; the eight settings, "
           "saved bools of kind 2, off (Prefer the dedicated GPU on), registered after patch 79's `localization`, "
           "then the Vulkan options' two lists (the original registers nothing)")
    lists = {name: language.console_variable(ours, node) for name, node in zip(names, nodes) if name in LISTS}
    expect(all(size in (8, 16) and saved == 1 and ours.read(buffer, 1) == b"\0" for _, buffer, size, saved in lists.values()),
           f"configs.start: the two lists saved cc_strings, empty (the first choice) ({lists})")
    _, _, again = ours.start()
    again_nodes = [node for node in registered(ours, again, head_before) if ours.text(ours.read(node + 0x10)) != b"srp_lobby"]
    expect(again_nodes == nodes, "configs.start again: each registered once")
    expect(ours.calls_made == [], f"configs.start opens no file ({ours.calls_made})")

    value = {name: variables[name][3] for name in SETTINGS}
    for name, node in zip(names, nodes):
        if name not in SETTINGS:
            continue
        for line, want in ((b"on", b"\x01"), (b"off", b"\0")):
            ours.emu.mem_write(language.TEXT, line + b"\0")
            ours.call_at(EXECUTE_BOOL, (language.TEXT,), {UC_X86_REG_ECX: node})
            expect(ours.read(value[name], 1) == want, f"user.cfg's line `{name} {line.decode()}` sets it "
                   "(cc_bool::execute 0xECA30)")

    # the Settings' call
    state = lambda: {name: ours.read(value[name], 1) == b"\x01" for name in SETTINGS}
    for name, label in SETTINGS.items():
        others = [n for n in SETTINGS if n != name]
        for case, argument, on, saves in [
            ("\"1\"", "1", True, 1),
            ("\"0\"", "0", False, 1),
            ("\"1\" again", "1", True, 1),
            ("another value", "on", True, 0),
            ("no argument", None, True, 0),
        ]:
            before = state()
            invokes, count = ours.callback(name, argument)
            after = state()
            # the Renderer's answer says the renderer running too: DirectX here (no DXVK loaded at the start);
            # a Vulkan option's the value it runs, without DXVK the saved one
            running = ("0",) if name == "srp_dxvk" else ("1" if on else "0",) if name in RUNNING else ()
            answer = (name, label, "1" if on else "0") + running
            expect(invokes == [(language.MOVIE, "root.srp_check_option", answer)]
                   and count == saves and after[name] == on and all(after[n] == before[n] for n in others),
                   f"{name} with {case}: it is {int(on)} (the others unchanged), user.cfg written {saves} time(s), "
                   f"the movie answered with its name, label and {int(on)}")
            stock_invokes, stock_saves = stock.callback(name, argument)
            expect(stock_invokes == [] and stock_saves == 0, f"the original ignores {name} ({case})")
        ours.callback(name, "1" if name in ON_BY_DEFAULT else "0")
    # the Vulkan options' lists: an index saves its choice's word, the answer the saved index, the running one and
    # the choices' texts
    buffers = {name: lists[name][1] for name in LISTS}
    for name, (label, words, choices) in LISTS.items():
        other = next(n for n in LISTS if n != name)
        for case, argument, index, saves in [
            ("\"3\"", "3", 3, 1),
            ("its last", str(len(words) - 1), len(words) - 1, 1),
            ("past its last", str(len(words)), len(words) - 1, 0),
            ("another value", "fps", len(words) - 1, 0),
            ("no argument", None, len(words) - 1, 0),
            ("\"0\"", "0", 0, 1),
        ]:
            before_other = ours.read(buffers[other], 16)
            invokes, count = ours.callback(name, argument)
            saved = ours.text(buffers[name]).decode()
            answer = (name, label, str(index), str(index)) + choices
            expect(invokes == [(language.MOVIE, "root.srp_check_option", answer)] and count == saves
                   and saved == words[index] and ours.read(buffers[other], 16) == before_other,
                   f"{name} with {case}: user.cfg's word {words[index]!r} (the other list unchanged), user.cfg written "
                   f"{saves} time(s), the answer its index {index} saved and running (no DXVK) and the choices' texts"
                   + ("" if invokes == [(language.MOVIE, "root.srp_check_option", answer)] else f": {invokes}"))
            stock_invokes, stock_saves = stock.callback(name, argument)
            expect(stock_invokes == [] and stock_saves == 0, f"the original ignores {name} ({case})")
    before = state()
    invokes, count = ours.callback("srp_hud_texts")
    # (the harness reads a string's first 64 bytes: a hint's first line and some)
    expect(invokes == [(language.MOVIE, "root.srp_hud_texts", tuple(t[:64] for t in TEXTS.values()))] and count == 0
           and state() == before,
           "srp_hud_texts: the group's texts (heading, Hide quests, the crosshair list's three choices, Static minimap) "
           "and the Renderer list's two, its note, the Vulkan options' button and its five hints, "
           "nothing set or saved")
    stock_invokes, _ = stock.callback("srp_hud_texts")
    expect(stock_invokes == [], "the original ignores srp_hud_texts")
    invokes, count = ours.callback("srp_language")
    expect([m for _, m, _ in invokes] == ["root.srp_language"] and count == 0,
           "srp_language still reaches patch 79 (its hook before this one)")
    for r, which in ((stock, "the original"), (ours, "the build")):
        invokes, count = r.callback("srp_hide")
        expect(invokes == [] and count == 0, f"{which}: another method returns, nothing invoked or saved")

    # each frame: the hud told the settings at a change
    told = lambda movie, hits, scores, crosshair=0, names=0: [(movie, "root.srp_hud_options", (hits, scores, crosshair, names))]
    frames = [
        # (what, the Settings' calls made first (None: a new match's options read, "replay" a replay's),
        # the hud movie (0 none), it refuses, the hud's Invokes wanted)
        ("the first frame, both off", [], hide.HUD_MOVIE, False, told(hide.HUD_MOVIE, 0, 0)),
        ("the next frame", [], hide.HUD_MOVIE, False, []),
        ("Hide hit markers checked", [("srp_hide_hit_markers", "1")], hide.HUD_MOVIE, False,
         told(hide.HUD_MOVIE, 1, 0)),
        ("the next frame", [], hide.HUD_MOVIE, False, []),
        ("Hide player scores checked too", [("srp_hide_scores", "1")], hide.HUD_MOVIE, False, told(hide.HUD_MOVIE, 1, 1)),
        ("a Settings answer that changes nothing", [("srp_hide_scores", None)], hide.HUD_MOVIE, False, []),
        ("Hide hit markers unchecked", [("srp_hide_hit_markers", "0")], hide.HUD_MOVIE, False,
         told(hide.HUD_MOVIE, 0, 1)),
        ("another hud movie (a new game world's)", [], OTHER_HUD, False, told(OTHER_HUD, 0, 1)),
        ("its next frame", [], OTHER_HUD, False, []),
        ("no hud movie", [("srp_hide_scores", "0")], 0, False, []),
        ("Hidden crosshair chosen (another hud movie)", [("srp_hide_crosshair", "1")], OTHER_HUD, False,
         told(OTHER_HUD, 0, 0, 1)),
        ("Static crosshair chosen again", [("srp_hide_crosshair", "0")], OTHER_HUD, False, told(OTHER_HUD, 0, 0, 0)),
        ("Names on map checked", [("srp_map_names", "1")], OTHER_HUD, False, told(OTHER_HUD, 0, 0, 0, 1)),
        ("Names on map unchecked", [("srp_map_names", "0")], OTHER_HUD, False, told(OTHER_HUD, 0, 0, 0, 0)),
        ("the hud back, refusing (no root yet)", [], hide.HUD_MOVIE, True, told(hide.HUD_MOVIE, 0, 0)),
        ("the next frame, the movie taking it", [], hide.HUD_MOVIE, False, told(hide.HUD_MOVIE, 0, 0)),
        ("the frame after", [], hide.HUD_MOVIE, False, []),
        ("Hide hit markers checked", [("srp_hide_hit_markers", "1")], hide.HUD_MOVIE, False,
         told(hide.HUD_MOVIE, 1, 0)),
        ("a new match, its hud at the last one's address (a new root)", [None], hide.HUD_MOVIE, False,
         told(hide.HUD_MOVIE, 1, 0)),
        ("its next frame", [], hide.HUD_MOVIE, False, []),
        ("a replay, its hud at the same address", ["replay"], hide.HUD_MOVIE, False, told(hide.HUD_MOVIE, 1, 0)),
        ("its next frame", [], hide.HUD_MOVIE, False, []),
    ]
    for case, calls, movie, refuses, want in frames:
        for call in calls:
            if call is None or call == "replay":
                for r in (stock, ours):
                    expect(r.new_match(call == "replay"), f"{case}: the options read goes on as the game's")
                continue
            ours.callback(*call)
        ours.refused = refuses
        a_kept, a_invoked = stock.frame(movie)
        b_kept, b_invoked = ours.frame(movie)
        mine = [c for c in b_invoked if c[1] == "root.srp_hud_options"]
        order = [c[1] for c in b_invoked if c[0] == (movie or -1)]
        expect(a_kept and b_kept and a_invoked == [] and mine == want
               and (not mine or order.index("root.srp_hud_options") == len(order) - 1),
               f"{case}: " + (f"the hud told {want[0][2]}" if want else "nothing told")
               + ", after patch 65's Invokes; on into update_player_icons as the game's")
    renderer(original, patched, expect)
    fullscreen_window(original, patched, expect)
    field_of_view(original, patched, expect)
    print("all cases passed")


def renderer(original, patched, expect):
    """The process's start and the login screen: the renderer (docs/notes/dxvk-option.md)."""
    on, off = b"localization english\r\nsrp_dxvk on\r\nr_fullscreen on\r\n", b"srp_dxvk off\r\n"
    good, old = (vk_version(1, 4, 303), 2), (vk_version(1, 3, 280), 2)
    look = [("CreateFileA", "dxvk.pending", 2), ("LoadLibraryA", "vulkan-1.dll"), ("vkCreateInstance",),
            ("vkDestroyInstance", INSTANCE), ("FreeLibrary", VULKAN)]
    files = [("CreateFileA", DXVK_DIR + name, 3) for name in DXVK]
    opened = [("CreateFileA", USER_CFG, 3)]
    marker = [("CreateFileA", "dxvk.pending", 3)]
    unmark = [("DeleteFileA", "dxvk.pending")]
    loads = [("GetModuleHandleA", "dxgi.dll")] + [("LoadLibraryA", DXVK_DIR + name) for name in DXVK]
    log = [("CreateFileA", "srp_patches.log", 4)]
    # DXVK's options before its DLLs: user.cfg without their lines prefers the dedicated GPU alone
    config = [("SetEnvironmentVariableA", "DXVK_CONFIG", "dxvk.hideIntegratedGraphics = True")] + log
    every = on + (b"srp_dxvk_frame_limit 144\r\nsrp_dxvk_low_latency on\r\nsrp_dxvk_tear_free on\r\n"
                  b"srp_dxvk_dedicated_gpu off\r\nsrp_dxvk_overlay full\r\n")
    every_config = [("SetEnvironmentVariableA", "DXVK_CONFIG", "dxvk.maxFrameRate = 144; dxvk.latencySleep = True; "
                     "dxvk.tearFree = True; dxvk.hud = fps,frametimes,gpuload,compiler,memory")] + log
    cases = [
        # (what, the world, the calls wanted, the marker after, the log's words)
        ("Vulkan picked, dxvk\\ whole, a Vulkan 1.4 device: the marker first, the look (all let go), DXVK's five "
         "by their full paths in their order", dict(user_cfg=on, vulkan=good),
         opened + files + marker + look + config + loads + log, True, b"DXVK loaded"),
        ("Vulkan picked with every Vulkan option changed: DXVK_CONFIG says them, before the loads",
         dict(user_cfg=every, vulkan=good), opened + files + marker + look + every_config + loads + log, True,
         b'dxvk: DXVK_CONFIG "dxvk.maxFrameRate = 144; dxvk.latencySleep = True; dxvk.tearFree = True; '
         b'dxvk.hud = fps,frametimes,gpuload,compiler,memory"'),
        ("Vulkan picked, Windows' dxgi.dll loaded already (the Vulkan loader's): the same loads, the game still "
         "bound to DXVK's, and said", dict(user_cfg=on, vulkan=good, windows_dxgi=True),
         opened + files + marker + look + config + loads[:1] + log + loads[1:] + log, True,
         b"another dxgi.dll is loaded already"),
        ("Vulkan picked, the best device 1.3: no DLL loaded, the marker taken away", dict(user_cfg=on, vulkan=old),
         opened + files + marker + look + log + unmark, False, b"no Vulkan 1.4 device"),
        ("Vulkan picked, no vulkan-1.dll: the same", dict(user_cfg=on),
         opened + files + marker + look[:2] + log + unmark, False, b"no Vulkan 1.4 device"),
        ("DirectX 11 picked: no look, nothing loaded, no DXVK_CONFIG", dict(user_cfg=off + every[len(on):], vulkan=good),
         opened + files + marker + unmark, False, b""),
        ("no user.cfg: DirectX 11", dict(vulkan=good), opened + files + marker + unmark, False, b""),
        ("Vulkan picked, d3d10core.dll missing from dxvk\\: no look, nothing loaded",
         dict(user_cfg=on, vulkan=good, present=[n for n in DXVK if n != "d3d10core.dll"]),
         opened + files[:3] + marker + unmark, False, b""),
        ("Vulkan picked, a marker left: no look, nothing loaded, the marker taken away",
         dict(user_cfg=on, vulkan=good, marker=True), opened + files + marker + unmark, False, b""),
        ("Vulkan picked, d3d10core.dll does not load: the loads stop there, nothing let go, the marker kept",
         dict(user_cfg=on, vulkan=good, fails="d3d10core.dll"), opened + files + marker + look + config + loads[:4] + log,
         True,
         b"stopped at DLL 2"),
        ("Vulkan picked, the game's dxgi called already (its slot set): dxgi loaded, its slot kept, the loads stop "
         "there", dict(user_cfg=on, vulkan=good, called="dxgi.dll"),
         opened + files + marker + look + config + loads[:2] + log,
         True, b"had called DLL 0"),
    ]
    for what, world, wanted, marked, words in cases:
        r = Run(patched)
        kept, calls = r.process_start(**world)
        bound = {name: r.read(slot) for name, slot in MODULE_SLOTS.items()}
        loaded = {name: CALLED if name == world.get("called") else r.modules.get(name, 0) for name in MODULE_SLOTS}
        expect(kept and calls == wanted and r.marker == marked and words in r.logged and r.mallocs == 0
               and bound == loaded,
               f"the process's start, {what} (no engine allocation, registers and stack as the original's; the "
               "game's delayed dxgi, d3d11 and d3d9 bound to DXVK's as each loaded, the others' slots left 0)"
               + ("" if calls == wanted else f": {calls}") + ("" if bound == loaded else f": slots {bound}"))
    stock = Run(original)
    kept, calls = stock.process_start(user_cfg=on, vulkan=good)
    expect(kept and calls == [] and all(stock.read(slot) == 0 for slot in MODULE_SLOTS.values()),
           "the original's process start calls none of these and binds no delayed import")

    # a failed DXVK start: the setting off at the login screen (user.cfg has run by then), saved once
    r = Run(patched)
    before = r.read(language.LIST_HEAD)
    _, _, head = r.start()
    node = language.named(r, registered(r, head, before), "srp_dxvk")
    value = r.read(node + 0x40)
    r.emu.mem_write(language.TEXT, b"on\0")
    r.call_at(EXECUTE_BOOL, (language.TEXT,), {UC_X86_REG_ECX: node})
    r.process_start(user_cfg=on, vulkan=good, marker=True)
    saves = r.saves
    kept, calls = r.login()
    expect(kept and r.read(value, 1) == b"\0" and r.saves == saves + 1 and ("DeleteFileA", "dxvk.pending") in calls
           and b"DirectX 11 again" in r.logged,
           f"the login screen after a failed DXVK start: srp_dxvk off, user.cfg saved once, the marker deleted ({calls})")
    saves = r.saves
    kept, calls = r.login()
    expect(kept and r.saves == saves and calls == unmark, "the next login screen: only the marker's delete")
    r = Run(patched)
    r.start()
    r.process_start(user_cfg=every, vulkan=good)
    invokes, _ = r.callback("srp_dxvk")
    expect([v[-1] for _, m, v in invokes if m == "root.srp_check_option"] == ["1"],
           f"the Renderer's answer in a process that loaded DXVK: running \"1\" ({invokes})")
    kept, calls = r.login()
    expect(kept and calls == unmark and not r.marker and r.saves == 0,
           "the login screen after a start with DXVK: the marker deleted, nothing saved")
    # a Vulkan option changed in the Settings after a start with DXVK: saved now, running what DXVK was given
    for name, argument, want in [("srp_dxvk_frame_limit", "5", ("5", "3")), ("srp_dxvk_overlay", "1", ("1", "3")),
                                 ("srp_dxvk_low_latency", "0", ("0", "1")), ("srp_dxvk_dedicated_gpu", "1", ("1", "0")),
                                 ("srp_dxvk_overlay", "3", ("3", "3"))]:
        invokes, _ = r.callback(name, argument)
        answer = [v[2:4] for _, m, v in invokes if m == "root.srp_check_option"]
        expect(answer == [want], f"{name} set to {argument} in a process that runs DXVK: saved {want[0]}, "
               f"running {want[1]}, what user.cfg had at the start ({invokes})")


def shown(calls):
    """The calls with their numbers in hex."""
    hexed = lambda v: tuple(map(hexed, v)) if isinstance(v, tuple) else hex(v) if isinstance(v, int) else v
    return [hexed(call) for call in calls]


def fullscreen_window(original, patched, expect):
    """Each Present of the game's swap chain (res_render_output::present 0x3CF7F0 to its ret, the site
    0x3CF8FF right before IDXGISwapChain::Present): with DXVK, a window DXGI refuses gets WS_POPUP first
    (docs/notes/dxvk-option.md, "The fullscreen window")."""
    on, off = b"srp_dxvk on\r\nr_fullscreen on\r\n", b"srp_dxvk off\r\nr_fullscreen on\r\n"
    good = (vk_version(1, 4, 303), 2)
    # the game's window (CreateWindowExA WS_CAPTION | WS_SYSMENU at 0x186FF; Windows adds WS_CLIPSIBLINGS and
    # WS_EX_WINDOWEDGE, show WS_VISIBLE), then DXVK 3.1.1's fullscreen of it (enterFullscreenMode: no
    # WS_OVERLAPPEDWINDOW, no WS_EX_OVERLAPPEDWINDOW, HWND_TOPMOST): the 2026-11-08 report's start
    windowed = (WS_VISIBLE | WS_CLIPSIBLINGS | WS_CAPTION | WS_SYSMENU, WS_EX_WINDOWEDGE)
    dxvk_fullscreen = (WS_VISIBLE | WS_CLIPSIBLINGS, WS_EX_TOPMOST)
    # set_size to fullscreen: SetWindowLongA(GWL_STYLE, [+E0h] 0) 0x3CFA09, SetWindowPos(HWND_TOPMOST, SWP_SHOWWINDOW)
    set_size_fullscreen = (WS_VISIBLE, WS_EX_TOPMOST)
    reads = [("GetWindowLongA", GWL_STYLE), ("GetWindowLongA", GWL_EXSTYLE)]
    expect(not dxgi_takes(*dxvk_fullscreen) and not dxgi_takes(*set_size_fullscreen) and dxgi_takes(*windowed),
           "DXGI's rule: the window in DXVK's fullscreen and in set_size's refused, the windowed one taken")
    for what, cfg, styles, wanted in [
        ("DXVK, the start in fullscreen (DXVK's fullscreen of the game's window, the report's)", on,
         dxvk_fullscreen, WS_POPUP | WS_VISIBLE | WS_CLIPSIBLINGS),
        ("DXVK, Settings to fullscreen (set_size's style 0)", on, set_size_fullscreen, WS_POPUP | WS_VISIBLE),
        ("DXVK, windowed", on, windowed, None),
        ("DirectX 11 (srp_dxvk off), the same fullscreen window", off, dxvk_fullscreen, None),
    ]:
        stock, ours = Run(original), Run(patched)
        stock.process_start(user_cfg=cfg, vulkan=good)
        ours.process_start(user_cfg=cfg, vulkan=good)
        logged = len(ours.logged)
        (a_regs, a_back), a_after, a_calls = stock.presented(styles)
        (b_regs, b_back), b_after, b_calls = ours.presented(styles)
        present = [call for call in b_calls if call[0] == "Present"]
        expect(a_back and b_back and a_regs == b_regs and a_after == styles and not any(
                   call[0].endswith("WindowLongA") for call in a_calls),
               f"{what}: registers and stack as the original's, which reads and sets no window style")
        expect(a_calls == [("Present", 0, 0, styles)],
               f"{what}: the original presents with the window as it is ({shown(a_calls)})")
        if wanted is not None:
            expect(b_calls == reads + [("SetWindowLongA", GWL_STYLE, wanted)] + reads
                   + [("Present", 0, 0, (wanted, styles[1]))] and b_after == (wanted, styles[1])
                   and dxgi_takes(*b_after) and b"made a popup" in ours.logged[logged:]
                   and b"DXGI takes it" in ours.logged[logged:],
                   f"{what}: WS_POPUP set before the Present, which sees a window DXGI takes "
                   f"({shown(b_calls)}; {ours.logged[logged:]})")
            (c_regs, c_back), c_after, c_calls = ours.presented(b_after)
            expect(c_back and c_regs == a_regs and c_calls == reads + [("Present", 0, 0, b_after)],
                   f"{what}, the next Present: read only, nothing set ({shown(c_calls)})")
        elif cfg is on:
            expect(b_calls == reads + [("Present", 0, 0, styles)] and b_after == styles
                   and ours.logged[logged:] == b"",
                   f"{what}: read only, nothing set, nothing logged ({shown(b_calls)})")
        else:
            expect(b_calls == a_calls and b_after == styles and ours.logged[logged:] == b"",
                   f"{what}: no window style read or set, the Present as the original's ({shown(b_calls)})")
    # Settings to fullscreen, windowed, fullscreen again: the same refused styles a second time are a new state
    ours = Run(patched)
    ours.process_start(user_cfg=on, vulkan=good)
    popup = (WS_POPUP | WS_VISIBLE, WS_EX_TOPMOST)
    sets = []
    for styles in (set_size_fullscreen, windowed, set_size_fullscreen):
        _, after, calls = ours.presented(styles)
        sets.append([call for call in calls if call[0] == "SetWindowLongA"])
    expect(sets == [[("SetWindowLongA", GWL_STYLE, popup[0])], [], [("SetWindowLongA", GWL_STYLE, popup[0])]]
           and after == popup, f"fullscreen, windowed, fullscreen again: a popup each time ({sets})")
    # a style Windows does not take: set and said once, then each Present only reads
    ours = Run(patched)
    ours.process_start(user_cfg=on, vulkan=good)
    ours.styles_stick = True
    logged = len(ours.logged)
    runs = [ours.presented(dxvk_fullscreen) for _ in range(3)]
    sets = [[call for call in calls if call[0] == "SetWindowLongA"] for _, _, calls in runs]
    told = ours.logged[logged:]
    expect(sets == [[("SetWindowLongA", GWL_STYLE, WS_POPUP | WS_VISIBLE | WS_CLIPSIBLINGS)], [], []]
           and told.count(b"\n") == 1 and b"DXGI still refuses it" in told,
           f"a style that does not take: set once, said once ({sets}; {told})")
    # vsync on reaches Present as the original's
    stock, ours = Run(original), Run(patched)
    stock.process_start(user_cfg=on, vulkan=good)
    ours.process_start(user_cfg=on, vulkan=good)
    _, _, a_calls = stock.presented(dxvk_fullscreen, vsync=1)
    _, _, b_calls = ours.presented(dxvk_fullscreen, vsync=1)
    expect(a_calls[-1][:3] == b_calls[-1][:3] == ("Present", 1, 0), f"vsync on: Present(1, 0) as the original's")


# uro_fov's console variable (0xAA6510) as its static initializer 0x85ADF0 leaves it in a running game (the
# emulation runs no initializer): +40h its value 0xA6FA28 (65.0 in the image), +44h the least (patch 22's 50), +48h 100
URO_FOV, URO_FOV_VALUE = 0xAA6510, 0xA6FA28


def field_of_view(original, patched, expect):
    """The Gameplay tab's field of view: `uro_fov` in the Settings' call sets whole degrees within the range,
    saves user.cfg once, and answers root.srp_slider_option(name, label, degrees, least, most)."""
    r, stock = Run(patched), Run(original)
    for run in (r, stock):
        run.word(URO_FOV + 0x40, URO_FOV_VALUE)
        run.emu.mem_write(URO_FOV + 0x44, struct.pack("<ff", 50.0, 100.0))
    value = lambda: struct.unpack("<f", bytes(r.emu.mem_read(URO_FOV_VALUE, 4)))[0]
    expect(value() == 65.0, "uro_fov: 65 in the image")
    for case, argument, want, saves in [
        ("no value", None, 65.0, 0),
        ("\"70\"", "70", 70.0, 1),
        ("its most, \"100\"", "100", 100.0, 1),
        ("its least, \"50\"", "50", 50.0, 1),
        ("\"49\" (under the range)", "49", 50.0, 0),
        ("\"101\" (over it)", "101", 50.0, 0),
        ("\"65.5\" (not whole degrees)", "65.5", 50.0, 0),
        ("\"fov\"", "fov", 50.0, 0),
        ("\"80\"", "80", 80.0, 1),
    ]:
        invokes, count = r.callback("uro_fov", argument)
        answer = ("uro_fov", "FOV", str(int(want)), "50", "100")
        expect(invokes == [(language.MOVIE, "root.srp_slider_option", answer)] and count == saves and value() == want,
               f"uro_fov with {case}: {want:g} degrees, user.cfg written {saves} time(s), the answer its degrees and range"
               + ("" if invokes == [(language.MOVIE, "root.srp_slider_option", answer)] else f": {invokes}"))
        stock_invokes, stock_saves = stock.callback("uro_fov", argument)
        expect(stock_invokes == [] and stock_saves == 0, f"the original ignores uro_fov ({case})")
    r.emu.mem_write(URO_FOV_VALUE, struct.pack("<f", 74.5))
    invokes, _ = r.callback("uro_fov")
    expect([v[2] for _, m, v in invokes if m == "root.srp_slider_option"] == ["75"],
           f"uro_fov of 74.5 (user.cfg's): the answer the nearest whole degrees, 75 ({invokes})")


if __name__ == "__main__":
    main()
