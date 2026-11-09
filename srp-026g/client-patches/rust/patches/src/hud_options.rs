//! Patch 83: the Settings' "HUD:" group (docs/notes/hide-hit-markers-scores.md):
//! four settings of the match hud kept in user.cfg, "Hide hit markers", "Hide
//! player scores", "Names on map" and the crosshair list's "Hidden crosshair"; and the
//! Graphics tab's Renderer list and its Vulkan options
//! (docs/notes/dxvk-option.md), which the next start reads. The hooks are
//! `hud_options.toml`'s, each a `lohook` after another patch's at the same site:
//!
//! - on_configs_loaded before user.cfg (`configs.start`, after patch 79):
//!   the console variables `srp_hide_hit_markers`, `srp_hide_scores`,
//!   `srp_map_names`, `srp_hide_crosshair` and `srp_dxvk`, saved in user.cfg, off by default,
//!   and the Vulkan options' `srp_dxvk_frame_limit`, `srp_dxvk_low_latency`,
//!   `srp_dxvk_tear_free`, `srp_dxvk_dedicated_gpu` (on by default) and `srp_dxvk_overlay`;
//! - the Settings' call (`options.callback`, after patch 79): a setting's
//!   name answers the main menu with `root.srp_check_option(name, label,
//!   saved)`, and with "1" or "0" (a list: its index) saves that first
//!   (user.cfg at once); the Renderer and the Vulkan options answer what this
//!   process runs too, a list its choices' texts; `uro_fov` (the Gameplay
//!   tab's field of view, patch 22's variable) answers
//!   `root.srp_slider_option(name, label, degrees, least, most)`, setting
//!   whole degrees within the range first;
//!   `srp_hud_texts` answers the group's own texts (`root.srp_hud_texts`:
//!   the heading, "Hide quests", the crosshair list's three choices,
//!   "Static minimap");
//! - each frame's player icons update (`player_icons.update_start`, after
//!   patch 65): the hud movie is told the four settings
//!   (`root.srp_hud_options(hide hit markers, hide scores, hide crosshair,
//!   map names)`),
//!   once for each change of the movie or of a setting;
//! - the process's start (`memory.allocate_arenas`, the engine's first
//!   memory set-up, before the VRAM check's Direct3DCreate9 0xE9C3F and the
//!   render device; every Direct3D DLL is a delayed import, so none has run
//!   yet): the renderer is chosen ([`step`]). With Vulkan/DXVK picked in
//!   user.cfg, the five DLLs in `dxvk\`, no marker left and a Vulkan 1.4
//!   device, DXVK's DLLs are loaded from `dxvk\` by their full paths, after
//!   the marker `dxvk.pending` is written and `DXVK_CONFIG` set from the
//!   Vulkan options user.cfg has ([`VulkanOptions`]), and the game's delayed d3d9, dxgi
//!   and d3d11 bound to them (their module slots: Windows' dxgi.dll, which
//!   the Vulkan loader loads, may be loaded under the same name). A marker
//!   left is a start with DXVK that never showed its login screen: plain
//!   DirectX, and the setting goes off;
//! - every Present of the game's swap chain, right before it
//!   (`render_output.present`, on the window's own thread): with DXVK, a window
//!   DXGI refuses (`WindowStyles::dxgi_takes`: topmost with no frame style,
//!   as DXVK's fullscreen and the game's set_size leave it) gets WS_POPUP, so
//!   the Vulkan swap chain AMD's driver makes through DXGI on it is made
//!   (docs/notes/dxvk-option.md, "The fullscreen window");
//! - the login screen shown (`login_menu.activated`, after patch 88): the
//!   renderer works, the marker goes; after a failed DXVK start the setting
//!   is turned off there (user.cfg has run by then) and saved;
//! - a match's options read (`static_info.options_read`,
//!   `replay.options_read`, after patch 65): a new match, whose hud may be
//!   another at the last one's address, is told again.
//!
//! What the hud does with them is the hud's (HUD.as): no hit marker on the
//! crosshair; a Tab list of names only; no crosshair; the Tab map's names
//! shown unless Alt is held (patch 82 tells it Alt). Nothing a match
//! computes: the interface only.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::language::ConsoleString;
use crate::vostok::log::{Piece::*, line};
use crate::vostok::renderer::{DelayedImport, Gpu, OutputWindow, Vulkan, VulkanVersion, WindowStyles, best_version,
                               clear_environment, dxvk_pick, environment_set, get_environment, look,
                              bind_delayed_import, exists_beside_exe, is_loaded, load_beside_exe, set_environment,
                              with_user_cfg};
use crate::vostok::settings::{ConsoleBool, FieldOfView, OptionsCall, SliderSteps, create_file, delete_file, file_exists,
                              save_user_config};
use crate::vostok::{Context, Flow, GameCell, GameWorldUi, HudOptionsOnOptionsCallExit, Lock, NoExit, Thread};

/// A Settings checkbox kept in user.cfg as `<name> on|off`: the Settings ask
/// for it by the same name; its label is a dictionary key. Off until the
/// file says otherwise, or on for one made `with` true.
pub struct Setting {
    pub name: &'static CStr,
    pub label: &'static CStr,
    value: ConsoleBool,
}

impl Setting {
    const fn new(name: &'static CStr, label: &'static CStr) -> Setting {
        Setting::with(name, label, false)
    }

    const fn with(name: &'static CStr, label: &'static CStr, default: bool) -> Setting {
        Setting { name, label, value: ConsoleBool::with(default) }
    }

    fn on(&self, thread: &Thread) -> bool {
        self.value.get(thread)
    }

    /// Its value in user.cfg's text as the console would set it: its line's
    /// `on` or `off`, otherwise (no line, another word) its default.
    pub fn of_user_cfg(&self, text: &[u8]) -> bool {
        match user_cfg_word(text, self.name) {
            Some(b"on") => true,
            Some(b"off") => false,
            _ => self.value.initial(),
        }
    }
}

/// The value of the first line of user.cfg's text that sets `name`: its first
/// word (the game writes one line of each, `<name> <value>`).
fn user_cfg_word<'t>(text: &'t [u8], name: &CStr) -> Option<&'t [u8]> {
    let rest = text.split(|&b| b == b'\n').find_map(|line| {
        let rest = line.strip_prefix(name.to_bytes())?;
        matches!(rest.first(), Some(b' ' | b'\t')).then_some(rest)
    })?;
    rest.split(|&b| b <= b' ').find(|w| !w.is_empty())
}

pub static HIDE_HIT_MARKERS: Setting = Setting::new(c"srp_hide_hit_markers", c"st_srp_hide_hit_markers_option");
pub static HIDE_SCORES: Setting = Setting::new(c"srp_hide_scores", c"st_srp_hide_scores_option");
/// The Tab map's player names always shown, Alt hiding them (patch 82's Alt
/// reversed; docs/notes/tab-map-names.md).
pub static MAP_NAMES: Setting = Setting::new(c"srp_map_names", c"st_srp_map_names_option");
/// The crosshair list's third choice, "Hidden crosshair" (Dynamic and Static
/// are the stock static crosshair setting's two states).
pub static HIDE_CROSSHAIR: Setting = Setting::new(c"srp_hide_crosshair", c"st_srp_crosshair_hidden");

/// The Graphics tab's Renderer list, on Vulkan/DXVK: the next process start
/// reads it from user.cfg itself (before the console exists), and turns it
/// off after a failed DXVK start ([`step`]).
pub static DXVK: Setting = Setting::new(c"srp_dxvk", c"st_srp_renderer_option");

// The Vulkan options under it (docs/notes/dxvk-option.md, "Vulkan options"):
// the next start with DXVK gives them to it ([`VulkanOptions`]).
pub static LOW_LATENCY: Setting = Setting::new(c"srp_dxvk_low_latency", c"st_srp_vulkan_low_latency");
pub static TEAR_FREE: Setting = Setting::new(c"srp_dxvk_tear_free", c"st_srp_vulkan_tear_free");
/// On by default: a user.cfg without its line prefers the dedicated GPU.
pub static DEDICATED_GPU: Setting = Setting::with(c"srp_dxvk_dedicated_gpu", c"st_srp_vulkan_dedicated_gpu", true);
static FRAME_LIMIT_SAVED: ConsoleString<8> = ConsoleString::new();
static OVERLAY_SAVED: ConsoleString<16> = ConsoleString::new();

/// A list choice's text: a dictionary key, or the text itself (a number).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceText {
    Key(&'static CStr),
    Plain(&'static CStr),
}

const OFF: ChoiceText = ChoiceText::Key(c"st_srp_vulkan_off");

/// A list's choices: each one's word in user.cfg and its text. The first is
/// the default (no line, another word).
pub trait ListChoice: Copy + Eq + 'static {
    /// In the list's order.
    const ALL: &'static [Self];

    fn word(self) -> &'static CStr;

    fn text(self) -> ChoiceText;

    fn of_word(word: Option<&[u8]>) -> Self {
        Self::ALL.iter().copied().find(|c| Some(c.word().to_bytes()) == word).unwrap_or(Self::ALL[0])
    }

    /// Its place in the list.
    fn index(self) -> usize {
        Self::ALL.iter().position(|&c| c == self).unwrap_or(0)
    }

    /// The choice at the Settings' index ("0", "1", ...).
    fn of_index(text: &CStr) -> Option<Self> {
        match text.to_bytes() {
            [digit @ b'0'..=b'9'] => Self::ALL.get(usize::from(digit - b'0')).copied(),
            _ => None,
        }
    }
}

/// The frame rate limit (`dxvk.maxFrameRate`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameLimit {
    Off,
    Fps60,
    Fps120,
    Fps144,
    Fps165,
    Fps240,
}

impl ListChoice for FrameLimit {
    const ALL: &'static [FrameLimit] =
        &[FrameLimit::Off, FrameLimit::Fps60, FrameLimit::Fps120, FrameLimit::Fps144, FrameLimit::Fps165, FrameLimit::Fps240];

    fn word(self) -> &'static CStr {
        match self {
            FrameLimit::Off => c"0",
            FrameLimit::Fps60 => c"60",
            FrameLimit::Fps120 => c"120",
            FrameLimit::Fps144 => c"144",
            FrameLimit::Fps165 => c"165",
            FrameLimit::Fps240 => c"240",
        }
    }

    fn text(self) -> ChoiceText {
        match self {
            FrameLimit::Off => OFF,
            limit => ChoiceText::Plain(limit.word()),
        }
    }
}

/// The performance overlay (`dxvk.hud`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Off,
    Fps,
    FrameTimes,
    Full,
}

impl ListChoice for Overlay {
    const ALL: &'static [Overlay] = &[Overlay::Off, Overlay::Fps, Overlay::FrameTimes, Overlay::Full];

    fn word(self) -> &'static CStr {
        match self {
            Overlay::Off => c"off",
            Overlay::Fps => c"fps",
            Overlay::FrameTimes => c"frametimes",
            Overlay::Full => c"full",
        }
    }

    fn text(self) -> ChoiceText {
        match self {
            Overlay::Off => OFF,
            Overlay::Fps => ChoiceText::Key(c"st_srp_vulkan_overlay_fps"),
            Overlay::FrameTimes => ChoiceText::Key(c"st_srp_vulkan_overlay_frametimes"),
            Overlay::Full => ChoiceText::Key(c"st_srp_vulkan_overlay_full"),
        }
    }
}

/// The Vulkan options' two lists, kept in user.cfg as `<name> <word>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VulkanList {
    FrameLimit,
    Overlay,
}

impl VulkanList {
    pub const ALL: [VulkanList; 2] = [VulkanList::FrameLimit, VulkanList::Overlay];

    pub fn name(self) -> &'static CStr {
        match self {
            VulkanList::FrameLimit => c"srp_dxvk_frame_limit",
            VulkanList::Overlay => c"srp_dxvk_overlay",
        }
    }

    pub fn label(self) -> &'static CStr {
        match self {
            VulkanList::FrameLimit => c"st_srp_vulkan_frame_limit",
            VulkanList::Overlay => c"st_srp_vulkan_overlay",
        }
    }

    fn named(name: &CStr) -> Option<VulkanList> {
        VulkanList::ALL.into_iter().find(|list| list.name() == name)
    }

    fn register(self, thread: &Thread) {
        match self {
            VulkanList::FrameLimit => FRAME_LIMIT_SAVED.register(thread, self.name()),
            VulkanList::Overlay => OVERLAY_SAVED.register(thread, self.name()),
        }
    }

    /// The choice at the Settings' index saved; whether there is one.
    fn set(self, thread: &Thread, index: &CStr) -> bool {
        match self {
            VulkanList::FrameLimit => {
                FrameLimit::of_index(index).map(|c| FRAME_LIMIT_SAVED.set(thread, c.word().to_bytes())).is_some()
            }
            VulkanList::Overlay => Overlay::of_index(index).map(|c| OVERLAY_SAVED.set(thread, c.word().to_bytes())).is_some(),
        }
    }

    /// Its choices' texts, in the list's order (at most [`MAX_CHOICES`]).
    fn texts(self) -> impl Iterator<Item = ChoiceText> {
        let (limits, overlays): (&[FrameLimit], &[Overlay]) = match self {
            VulkanList::FrameLimit => (FrameLimit::ALL, &[]),
            VulkanList::Overlay => (&[], Overlay::ALL),
        };
        limits.iter().map(|c| c.text()).chain(overlays.iter().map(|c| c.text()))
    }
}

/// The longest list's choices.
const MAX_CHOICES: usize = 6;

/// The Gameplay tab's field of view slider: the engine's `uro_fov` (patch
/// 22), which the Settings name by it; its label the video options' own key
/// (their option 7 names no variable: the original's dead "fov").
pub const FIELD_OF_VIEW: &CStr = c"uro_fov";
const FIELD_OF_VIEW_LABEL: &CStr = c"st_fov_option";

/// The nearest whole degrees to the variable's value (user.cfg may hold a
/// fraction).
fn whole_degrees(degrees: f32) -> SliderSteps {
    SliderSteps::nearest(degrees, 1.0, SliderSteps::new(0), SliderSteps::new(u16::MAX))
}

/// The Vulkan options, read once: at the process's start from user.cfg's
/// text (a start with DXVK gives them to it), in the Settings from the
/// console's variables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VulkanOptions {
    pub frame_limit: FrameLimit,
    pub low_latency: bool,
    pub tear_free: bool,
    pub dedicated_gpu: bool,
    pub overlay: Overlay,
}

/// The Settings' digits: a checkbox's "1" or "0", a list's index.
const DIGITS: [&CStr; 10] = [c"0", c"1", c"2", c"3", c"4", c"5", c"6", c"7", c"8", c"9"];

impl VulkanOptions {
    pub fn of_user_cfg(text: &[u8]) -> VulkanOptions {
        VulkanOptions {
            frame_limit: FrameLimit::of_word(user_cfg_word(text, VulkanList::FrameLimit.name())),
            low_latency: LOW_LATENCY.of_user_cfg(text),
            tear_free: TEAR_FREE.of_user_cfg(text),
            dedicated_gpu: DEDICATED_GPU.of_user_cfg(text),
            overlay: Overlay::of_word(user_cfg_word(text, VulkanList::Overlay.name())),
        }
    }

    /// As the console's variables have them (the Settings' saved ones).
    fn saved(thread: &Thread) -> VulkanOptions {
        VulkanOptions {
            frame_limit: FrameLimit::of_word(Some(FRAME_LIMIT_SAVED.get(thread))),
            low_latency: LOW_LATENCY.on(thread),
            tear_free: TEAR_FREE.on(thread),
            dedicated_gpu: DEDICATED_GPU.on(thread),
            overlay: Overlay::of_word(Some(OVERLAY_SAVED.get(thread))),
        }
    }

    /// What this process runs: the options DXVK was given at its start, or,
    /// without DXVK, none but the saved ones (nothing to restart for but the
    /// Renderer itself).
    fn running(thread: &Thread) -> VulkanOptions {
        match renderer(thread) {
            Renderer::Dxvk(options) => options,
            _ => VulkanOptions::saved(thread),
        }
    }

    /// An option's value as the Settings show it, by its name: a checkbox's
    /// "1" or "0", a list's index; None for another name.
    fn shown(&self, name: &CStr) -> Option<&'static CStr> {
        let checked = |on: bool| Some(DIGITS[usize::from(on)]);
        match name {
            n if n == LOW_LATENCY.name => checked(self.low_latency),
            n if n == TEAR_FREE.name => checked(self.tear_free),
            n if n == DEDICATED_GPU.name => checked(self.dedicated_gpu),
            n => VulkanList::named(n).map(|list| DIGITS[self.index(list)]),
        }
    }

    /// A list's choice, as its index.
    fn index(&self, list: VulkanList) -> usize {
        match list {
            VulkanList::FrameLimit => self.frame_limit.index(),
            VulkanList::Overlay => self.overlay.index(),
        }
    }

    /// DXVK's options (`DXVK_CONFIG`: `key = value`, split by `;`); one off
    /// is DXVK's default, not named. Prefer the dedicated GPU is
    /// `dxvk.hideIntegratedGraphics`, which DXVK 3.1.1 applies only when it
    /// finds both a dedicated GPU and an integrated one (dxvk_instance.cpp):
    /// a PC with an integrated GPU alone keeps it.
    pub fn config(&self) -> DxvkConfig {
        let mut config = DxvkConfig { bytes: [0; DxvkConfig::ROOM], len: 0, left_out: 0 };
        if self.frame_limit != FrameLimit::Off {
            config.option("dxvk.maxFrameRate", self.frame_limit.word());
        }
        if self.low_latency {
            config.option("dxvk.latencySleep", c"True");
        }
        if self.tear_free {
            config.option("dxvk.tearFree", c"True");
        }
        if self.dedicated_gpu {
            config.option("dxvk.hideIntegratedGraphics", c"True");
        }
        let hud = match self.overlay {
            Overlay::Off => None,
            Overlay::Fps => Some(c"fps"),
            Overlay::FrameTimes => Some(c"fps,frametimes"),
            Overlay::Full => Some(c"fps,frametimes,gpuload,compiler,memory"),
        };
        if let Some(hud) = hud {
            config.option("dxvk.hud", hud);
        }
        config
    }
}

/// `DXVK_CONFIG`'s value, built in place, NUL-terminated (ASCII).
pub struct DxvkConfig {
    bytes: [u8; DxvkConfig::ROOM],
    len: usize,
    left_out: u32,
}

impl DxvkConfig {
    /// Below DXVK's MAX_PATH (260 with the NUL), past which it reads none;
    /// every option at once fits (a test).
    const ROOM: usize = 256;

    /// An option whole, or (no room for it with its NUL) none of it: then
    /// counted as left out.
    fn option(&mut self, key: &str, value: &CStr) {
        let separator: &[u8] = if self.len == 0 { b"" } else { b"; " };
        let parts = [separator, key.as_bytes(), b" = ", value.to_bytes()];
        let end = self.len + parts.iter().map(|part| part.len()).sum::<usize>();
        if end >= Self::ROOM {
            self.left_out += 1;
            return;
        }
        for part in parts {
            self.bytes[self.len..self.len + part.len()].copy_from_slice(part);
            self.len += part.len();
        }
    }

    /// The options that did not fit (none: every option at once fits, a test).
    pub fn left_out(&self) -> u32 {
        self.left_out
    }

    pub fn as_c_str(&self) -> &CStr {
        CStr::from_bytes_until_nul(&self.bytes).unwrap_or(c"")
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }

    /// What the driver of DXVK's GPU needs besides the player's options: on
    /// AMD's (Windows has only its own), no descriptor heap, as DXVK after
    /// 3.1.1 decides by itself (doitsujin/dxvk 076922bd2, its #5837: DXVK 3
    /// crashed on start on an RX 7900 XTX with it). Whether it was added.
    pub fn for_gpu(&mut self, gpu: Option<Gpu>) -> bool {
        if gpu.is_some_and(|gpu| gpu.vendor == VENDOR_AMD) {
            let before = self.left_out;
            self.option("dxvk.enableDescriptorHeap", c"False");
            return self.left_out == before;
        }
        false
    }
}

/// PCI vendors (`vendorID`).
const VENDOR_AMD: u32 = 0x1002;
const VENDOR_NVIDIA: u32 = 0x10DE;
const VENDOR_INTEL: u32 = 0x8086;

/// The Vulkan loader's driver manifests of a vendor's Windows drivers, by
/// their file names (`VK_LOADER_DRIVERS_SELECT`'s globs): AMD's
/// `amd-vulkan32.json` (and an older package's `amdvlk32.json`), NVIDIA's
/// `nv-vk32.json`, Intel's `igvk32.json`. A name not matched is caught by the
/// look that follows ([`limit_vulkan_drivers`]).
pub fn drivers_of(vendor: u32) -> Option<&'static CStr> {
    match vendor {
        VENDOR_AMD => Some(c"amd*"),
        VENDOR_NVIDIA => Some(c"nv*"),
        VENDOR_INTEL => Some(c"igvk*"),
        _ => None,
    }
}

/// `DXVK_FILTER_DEVICE_NAME` as the player set it (DXVK reads it; empty for
/// none), the name part DXVK's GPU must have.
struct DeviceFilter {
    room: [u8; 128],
    len: usize,
}

impl DeviceFilter {
    fn of_environment(thread: &Thread) -> DeviceFilter {
        let mut filter = DeviceFilter { room: [0; 128], len: 0 };
        filter.len = get_environment(thread, c"DXVK_FILTER_DEVICE_NAME", &mut filter.room);
        filter
    }

    fn bytes(&self) -> &[u8] {
        &self.room[..self.len]
    }
}

/// The vendors whose drivers are looked at one by one, in this order.
const VENDORS: [u32; 3] = [VENDOR_NVIDIA, VENDOR_AMD, VENDOR_INTEL];
/// `deviceType` of a CPU's device (lavapipe, SwiftShader).
const PHYSICAL_DEVICE_TYPE_CPU: u32 = 4;

/// What one vendor's look found: its drivers' glob and DXVK's pick of its
/// devices (None: none it would take).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VendorLook {
    pub vendor: u32,
    pub pick: Option<Gpu>,
}

/// DXVK's GPU over the vendors' looks, in their order: the first discrete
/// one, else the first integrated, else the first virtual, else the first
/// (DXVK sorts its adapters so, `dxvk_pick`). None when no look found one.
pub fn pick_across(looks: &[VendorLook]) -> Option<Gpu> {
    let picks = || looks.iter().filter_map(|look| look.pick);
    [2, 1, 3].iter().find_map(|&kind| picks().find(|gpu| gpu.kind == kind)).or_else(|| picks().next())
}

/// The GPU DXVK takes and the drivers asked for, from the start's look.
static DXVK_GPU: Lock<Option<Gpu>> = Lock::new(None);

/// The start's Vulkan look. The loader loads every driver it knows when an
/// instance is made, and AMD's stays loaded after (its own D3D12 driver too,
/// once it presents): on a PC of two vendors' GPUs both drivers took room in
/// the game's 4 GiB of address space, which DXVK ran out of (SRP-3: 237 MiB
/// on an AMD iGPU beside an RTX). So each vendor's drivers are looked at alone
/// (`VK_LOADER_DRIVERS_SELECT`, a glob of their manifests' names), NVIDIA's,
/// AMD's, Intel's, until one has a discrete GPU DXVK takes: the variable then
/// stays set to that vendor's, and DXVK's instance loads those alone. With no
/// discrete GPU, the first vendor's integrated one. When the loader takes no
/// such variable (an elevated process, where it ignores it) or no vendor's
/// drivers found a GPU, the variable is deleted and every driver looked at,
/// as before. A player's own `VK_LOADER_DRIVERS_SELECT` or `_DISABLE` is
/// kept. The best version found is the look's answer (Vulkan 1.4 or not).
fn look_for_dxvk(thread: &Thread) -> Vulkan {
    let filter = DeviceFilter::of_environment(thread);
    let everything = |thread: &Thread, why: &str| {
        let all = look(thread);
        let gpu = all.as_ref().ok().and_then(|gpus| dxvk_pick(gpus, VULKAN_NEEDED, filter.bytes()));
        DXVK_GPU.with(thread, |kept| *kept = gpu);
        line(&[Text("dxvk: Vulkan drivers: all ("), Text(why), Text(")")]);
        all.map_or_else(|failed| failed, |gpus| best_version(&gpus))
    };
    if environment_set(thread, c"VK_LOADER_DRIVERS_SELECT") || environment_set(thread, c"VK_LOADER_DRIVERS_DISABLE") {
        return everything(thread, "the player's own VK_LOADER_DRIVERS_SELECT or _DISABLE kept");
    }
    let mut looks = [VendorLook { vendor: 0, pick: None }; VENDORS.len()];
    let mut best: Option<VulkanVersion> = None;
    let mut done = 0;
    for (slot, vendor) in looks.iter_mut().zip(VENDORS) {
        let Some(drivers) = drivers_of(vendor) else { continue };
        if !set_environment(thread, c"VK_LOADER_DRIVERS_SELECT", drivers) {
            clear_environment(thread, c"VK_LOADER_DRIVERS_SELECT");
            return everything(thread, "VK_LOADER_DRIVERS_SELECT not set");
        }
        let gpus = match look(thread) {
            Ok(gpus) => gpus,
            // no loader: none for any vendor
            Err(Vulkan::NoLoader) => {
                clear_environment(thread, c"VK_LOADER_DRIVERS_SELECT");
                return Vulkan::NoLoader;
            }
            Err(_) => continue,
        };
        // another vendor's GPU (a CPU driver's device is none): the loader takes no such variable here
        if gpus.list.iter().flatten().any(|gpu| gpu.kind != PHYSICAL_DEVICE_TYPE_CPU && gpu.vendor != vendor) {
            clear_environment(thread, c"VK_LOADER_DRIVERS_SELECT");
            return everything(thread, "the loader ignores VK_LOADER_DRIVERS_SELECT here: an elevated game?");
        }
        if let Vulkan::Device(version) = best_version(&gpus) {
            best = best.max(Some(version));
        }
        *slot = VendorLook { vendor, pick: dxvk_pick(&gpus, VULKAN_NEEDED, filter.bytes()) };
        done += 1;
        if slot.pick.is_some_and(|gpu| gpu.kind == 2) {
            break;
        }
    }
    let Some(gpu) = pick_across(&looks[..done.max(1)]) else {
        clear_environment(thread, c"VK_LOADER_DRIVERS_SELECT");
        return everything(thread, "no vendor's drivers alone found a GPU DXVK would take");
    };
    let drivers = drivers_of(gpu.vendor).unwrap_or(c"*");
    set_environment(thread, c"VK_LOADER_DRIVERS_SELECT", drivers);
    DXVK_GPU.with(thread, |kept| *kept = Some(gpu));
    line(&[Text("dxvk: Vulkan drivers: \""), Text(drivers.to_str().unwrap_or("?")),
           Text("\" only (VK_LOADER_DRIVERS_SELECT), DXVK's GPU "), Text(gpu.name())]);
    best.map_or(Vulkan::NoDevice, Vulkan::Device)
}

/// The marker of a start with DXVK that has not shown its login screen yet,
/// in the game's working folder (`binaries\x86`).
pub const DXVK_MARKER: &CStr = c"dxvk.pending";
/// A DLL of DXVK's: its file and the game's delayed import it serves.
pub struct DxvkDll {
    pub file: &'static CStr,
    pub serves: Option<DelayedImport>,
}

/// DXVK's DLLs (`make_client_026g.py`'s `DXVK_DLLS`, `winetricks dxvk`'s set),
/// in their load order: each one's imports by name are of those before it
/// (d3d11 imports dxgi, d3d10core d3d11, d3d8 d3d9).
pub const DXVK_DLLS: [DxvkDll; 5] = [
    DxvkDll { file: c"dxvk\\dxgi.dll", serves: Some(DelayedImport::Dxgi) },
    DxvkDll { file: c"dxvk\\d3d11.dll", serves: Some(DelayedImport::D3d11) },
    DxvkDll { file: c"dxvk\\d3d10core.dll", serves: None },
    DxvkDll { file: c"dxvk\\d3d9.dll", serves: Some(DelayedImport::D3d9) },
    DxvkDll { file: c"dxvk\\d3d8.dll", serves: None },
];
/// What DXVK 3 needs of a device.
const VULKAN_NEEDED: VulkanVersion = VulkanVersion::new(1, 4, 0);

/// The Renderer list's choice as user.cfg keeps it: `srp_dxvk on` is
/// Vulkan, with the Vulkan options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    DirectX,
    Vulkan(VulkanOptions),
}

impl Choice {
    pub fn of_user_cfg(text: &[u8]) -> Choice {
        if DXVK.of_user_cfg(text) { Choice::Vulkan(VulkanOptions::of_user_cfg(text)) } else { Choice::DirectX }
    }
}

/// The renderer of this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Renderer {
    /// Before the process's start hook.
    Undecided,
    /// Vulkan picked: its marker written, the Vulkan look asked.
    Checking(VulkanOptions),
    /// Windows' own Direct3D.
    DirectX,
    /// DXVK's DLLs loaded with these options (all, or as many as loaded: the
    /// marker covers a crash); the login screen takes the marker.
    Dxvk(VulkanOptions),
    /// The last start with DXVK never showed its login screen: Windows' own,
    /// and the setting goes off at the login screen.
    TurnOff,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The process's start: user.cfg's choice, whether all of `dxvk\` is
    /// there, whether a marker is left.
    Started { choice: Choice, files: bool, marker: bool },
    /// What the Vulkan look found.
    Looked(Vulkan),
    /// The login screen shown.
    LoginShown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Nothing,
    /// Write the marker, then look at Vulkan.
    MarkAndLook,
    /// Give DXVK these options (`DXVK_CONFIG`), then load its DLLs in order
    /// (the marker stays).
    Load(VulkanOptions),
    /// Delete the marker.
    Unmark,
    /// The setting off, user.cfg saved, the marker deleted.
    TurnOff,
}

/// The one transition of the renderer's choice.
pub fn step(state: Renderer, event: Event) -> (Renderer, Action) {
    match (state, event) {
        (Renderer::Undecided, Event::Started { choice: Choice::Vulkan(_), marker: true, .. }) => {
            (Renderer::TurnOff, Action::Unmark)
        }
        (Renderer::Undecided, Event::Started { choice: Choice::Vulkan(options), files: true, marker: false }) => {
            (Renderer::Checking(options), Action::MarkAndLook)
        }
        (Renderer::Undecided, Event::Started { .. }) => (Renderer::DirectX, Action::Unmark),
        (Renderer::Checking(options), Event::Looked(Vulkan::Device(version))) if version >= VULKAN_NEEDED => {
            (Renderer::Dxvk(options), Action::Load(options))
        }
        (Renderer::Checking(_), Event::Looked(_)) => (Renderer::DirectX, Action::Unmark),
        (Renderer::TurnOff, Event::LoginShown) => (Renderer::DirectX, Action::TurnOff),
        (state, Event::LoginShown) => (state, Action::Unmark),
        (state, _) => (state, Action::Nothing),
    }
}

/// A lock, by way of defence: `on_present` reads it at each Present. The
/// report's stack shows Present on the game's main thread, the one that
/// pumps the window's messages and then ticks the engine (engine_world::run
/// 0x18B30), where the start and the login screen run too; no second thread
/// is shown.
static RENDERER: Lock<Renderer> = Lock::new(Renderer::Undecided);

/// The last window styles DXGI refused that `on_present` made a popup.
static REFUSED: Lock<Option<WindowStyles>> = Lock::new(None);

fn renderer(thread: &Thread) -> Renderer {
    RENDERER.with(thread, |state| *state)
}

/// `event` through [`step`], its action done; an action that finds something
/// (the Vulkan look) goes on with it.
fn drive(thread: &Thread, event: Event) {
    let mut event = Some(event);
    while let Some(now) = event.take() {
        let (next, action) = RENDERER.with(thread, |state| {
            let (next, action) = step(*state, now);
            *state = next;
            (next, action)
        });
        match action {
            Action::Nothing => {}
            Action::Unmark => delete_file(thread, DXVK_MARKER),
            Action::MarkAndLook => {
                // first: a driver that crashes in the look leaves it
                create_file(thread, DXVK_MARKER);
                event = Some(Event::Looked(look_for_dxvk(thread)));
            }
            Action::Load(options) => {
                // the GPU DXVK will take, as the start's look found it
                let gpu = DXVK_GPU.with(thread, |kept| *kept);
                // before DXVK's DLLs: DXVK reads it when the game makes its first device
                let mut config = options.config();
                let players = config.as_str().len();
                let for_gpu = config.for_gpu(gpu);
                if set_environment(thread, c"DXVK_CONFIG", config.as_c_str()) {
                    // the player's options on the line (it holds them all), the GPU's own on the next
                    let options = &config.as_str()[..players];
                    if config.left_out() == 0 {
                        line(&[Text("dxvk: DXVK_CONFIG \""), Text(options), Text("\"")]);
                    } else {
                        line(&[Text("dxvk: DXVK_CONFIG \""), Text(options), Text("\" ("), Dec(config.left_out()),
                               Text(" option(s) left out: no room)")]);
                    }
                    if for_gpu {
                        line(&[Text("dxvk: DXVK_CONFIG adds \"dxvk.enableDescriptorHeap = False\": DXVK's GPU is AMD's, ")
                               , Text(gpu.as_ref().map_or("?", Gpu::name))]);
                    }
                } else {
                    line(&[Text("dxvk: DXVK_CONFIG not set")]);
                }

                if is_loaded(thread, c"dxgi.dll") {
                    line(&[Text("dxvk: another dxgi.dll is loaded already")]);
                }
                match load_dxvk(thread) {
                    Ok(()) => line(&[Text("dxvk: DXVK loaded from dxvk\\, the game's d3d9, dxgi and d3d11 bound to it")]),
                    Err(Unloaded(i)) => line(&[Text("dxvk: loading DXVK stopped at DLL "), Dec(i as u32),
                                               Text(" of dxgi, d3d11, d3d10core, d3d9, d3d8 (from 0)")]),
                    Err(Called(i)) => line(&[Text("dxvk: the game had called DLL "), Dec(i as u32),
                                             Text(" of dxgi, d3d11, d3d10core, d3d9, d3d8 (from 0) already: not bound")]),
                }
            }
            Action::TurnOff => {
                DXVK.value.set(thread, false);
                save_user_config(thread);
                delete_file(thread, DXVK_MARKER);
                line(&[Text("dxvk: the last start with DXVK never showed its login screen: DirectX 11 again")]);
            }
        }
        if let (Renderer::Checking(_), Some(Event::Looked(found))) = (next, event)
            && !matches!(found, Vulkan::Device(version) if version >= VULKAN_NEEDED)
        {
            line(&[Text("dxvk: Vulkan/DXVK picked, but no Vulkan 1.4 device: DirectX 11")]);
        }
    }
}

/// Why DXVK's loads stopped: the DLL of that index did not load, or the
/// game's import it serves was bound already.
enum Stopped {
    Unloaded(usize),
    Called(usize),
}
use Stopped::*;

/// DXVK's DLLs loaded in order, each delayed import of the game bound to its
/// DLL as it loads.
fn load_dxvk(thread: &Thread) -> Result<(), Stopped> {
    for (i, dll) in DXVK_DLLS.iter().enumerate() {
        let module = load_beside_exe(thread, dll.file).ok_or(Unloaded(i))?;
        if let Some(import) = dll.serves
            && !bind_delayed_import(thread, import, module)
        {
            return Err(Called(i));
        }
    }
    Ok(())
}

/// Every setting of this patch, as the Settings call them.
pub static SETTINGS: [&Setting; 8] =
    [&HIDE_HIT_MARKERS, &HIDE_SCORES, &MAP_NAMES, &HIDE_CROSSHAIR, &DXVK, &LOW_LATENCY, &TEAR_FREE, &DEDICATED_GPU];

/// The Settings' "HUD:" group's own texts (dictionary keys), in the order
/// `root.srp_hud_texts` gives them: the heading, the stock quest progress
/// checkbox's label as the group shows it (inverted), the crosshair list's
/// three choices, the stock fixed minimap checkbox's label; then the
/// Graphics tab's Renderer list's two choices, its note (the Language
/// choice's "Applies after restart."), the Vulkan options' button and its
/// five options' hints (in the panel's order).
pub const TEXTS: [&CStr; 15] = [c"st_srp_options_hud", c"st_srp_hide_quests_option", c"st_srp_crosshair_dynamic",
                                c"st_srp_crosshair_static", c"st_srp_crosshair_hidden", c"st_srp_static_minimap_option",
                                c"st_srp_renderer_dx11", c"st_srp_renderer_dxvk", c"st_srp_language_restart",
                                c"st_srp_vulkan_options", c"st_srp_vulkan_frame_limit_hint", c"st_srp_vulkan_overlay_hint",
                                c"st_srp_vulkan_low_latency_hint", c"st_srp_vulkan_tear_free_hint",
                                c"st_srp_vulkan_dedicated_gpu_hint"];

/// The setting the Settings' call names.
pub fn setting(name: &CStr) -> Option<&'static Setting> {
    SETTINGS.iter().copied().find(|s| s.name == name)
}

/// The Settings' value of a checkbox: "1" checked, "0" not.
fn choice(text: &CStr) -> Option<bool> {
    match text.to_bytes() {
        b"1" => Some(true),
        b"0" => Some(false),
        _ => None,
    }
}

/// What the hud is told: the four settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudView {
    pub hit_markers_hidden: bool,
    pub scores_hidden: bool,
    pub crosshair_hidden: bool,
    pub map_names: bool,
}

impl HudView {
    fn now(thread: &Thread) -> HudView {
        HudView {
            hit_markers_hidden: HIDE_HIT_MARKERS.on(thread),
            scores_hidden: HIDE_SCORES.on(thread),
            crosshair_hidden: HIDE_CROSSHAIR.on(thread),
            map_names: MAP_NAMES.on(thread),
        }
    }
}

/// The hud movie last told and what it was told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Told {
    movie: usize,
    view: HudView,
}

/// None until a hud movie took the settings.
static TOLD: GameCell<Option<Told>> = GameCell::new(None);

pub fn on_configs(ctx: &mut Context<'_>) -> Flow<NoExit> {
    for setting in SETTINGS {
        setting.value.register(ctx.thread(), setting.name);
    }
    for list in VulkanList::ALL {
        list.register(ctx.thread());
    }
    Flow::Continue
}

/// The process's start (memory::platform::allocate_arenas 0xE9580, once):
/// the renderer chosen from user.cfg, `dxvk\` and the marker.
pub fn on_process_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let choice = with_user_cfg(thread, Choice::of_user_cfg).unwrap_or(Choice::DirectX);
    let files = DXVK_DLLS.iter().all(|dll| exists_beside_exe(thread, dll.file));
    let marker = file_exists(thread, DXVK_MARKER);
    drive(thread, Event::Started { choice, files, marker });
    Flow::Continue
}

/// The Settings' calls: a setting's name answers its label and saved state
/// (`root.srp_check_option`), saving a "1" or "0" (a list's index) first;
/// the Renderer and the Vulkan options answer the value this process runs
/// after it (for their note), a list its choices' texts after that.
/// `srp_hud_texts` answers the groups' texts (`root.srp_hud_texts`).
pub fn on_options_call(ctx: &mut Context<'_>) -> Flow<HudOptionsOnOptionsCallExit> {
    let Some(call) = OptionsCall::at(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let method = call.method();
    if method == c"srp_hud_texts" {
        let mut rooms = [[0; 0x200]; TEXTS.len()];
        let mut texts: [&CStr; TEXTS.len()] = [c""; TEXTS.len()];
        for ((text, room), key) in texts.iter_mut().zip(rooms.iter_mut()).zip(TEXTS) {
            *text = call.translate(key, room);
        }
        call.answer(c"root.srp_hud_texts", &texts);
        return Flow::Exit(HudOptionsOnOptionsCallExit::Done);
    }
    let mut label = [0; 0x200];
    if method == FIELD_OF_VIEW {
        let Some(now) = FieldOfView::now(thread) else { return Flow::Continue };
        // whole degrees: the slider's snap (the stock option's 1)
        if let Some(degrees) = call.string_argument(0).and_then(SliderSteps::parse)
            && FieldOfView::set(thread, f32::from(degrees.get()))
        {
            save_user_config(thread);
        }
        let saved = FieldOfView::now(thread).map_or(now.degrees, |then| then.degrees);
        let (mut value, mut least, mut most) = ([0; 8], [0; 8], [0; 8]);
        let values = [FIELD_OF_VIEW, call.translate(FIELD_OF_VIEW_LABEL, &mut label),
                      whole_degrees(saved).text(&mut value), whole_degrees(now.least).text(&mut least),
                      whole_degrees(now.most).text(&mut most)];
        call.answer(c"root.srp_slider_option", &values);
        return Flow::Exit(HudOptionsOnOptionsCallExit::Done);
    }
    if let Some(setting) = setting(method) {
        if let Some(on) = call.string_argument(0).and_then(choice) {
            setting.value.set(thread, on);
            save_user_config(thread);
        }
        let label = call.translate(setting.label, &mut label);
        let saved = DIGITS[usize::from(setting.on(thread))];
        // the renderer this process runs ("1": DXVK loaded at its start), or the option's value it runs
        let running = if core::ptr::eq(setting, &DXVK) {
            Some(DIGITS[usize::from(matches!(renderer(thread), Renderer::Dxvk(_)))])
        } else {
            VulkanOptions::running(thread).shown(setting.name)
        };
        match running {
            Some(running) => call.answer(c"root.srp_check_option", &[setting.name, label, saved, running]),
            None => call.answer(c"root.srp_check_option", &[setting.name, label, saved]),
        };
        return Flow::Exit(HudOptionsOnOptionsCallExit::Done);
    }
    let Some(list) = VulkanList::named(method) else { return Flow::Continue };
    if call.string_argument(0).is_some_and(|index| list.set(thread, index)) {
        save_user_config(thread);
    }
    let mut values: [&CStr; 4 + MAX_CHOICES] = [c""; 4 + MAX_CHOICES];
    let mut rooms = [[0; 0x200]; MAX_CHOICES];
    values[0] = list.name();
    values[1] = call.translate(list.label(), &mut label);
    values[2] = DIGITS[VulkanOptions::saved(thread).index(list)];
    values[3] = DIGITS[VulkanOptions::running(thread).index(list)];
    let mut count = 4;
    for (text, room) in list.texts().zip(rooms.iter_mut()) {
        values[count] = match text {
            ChoiceText::Key(key) => call.translate(key, room),
            ChoiceText::Plain(plain) => plain,
        };
        count += 1;
    }
    call.answer(c"root.srp_check_option", &values[..count]);
    Flow::Exit(HudOptionsOnOptionsCallExit::Done)
}

/// The login screen shown (login_menu::on_activate 0x21EAC8): the renderer
/// works, with DXVK or not; the marker goes, and after a failed DXVK start
/// the setting goes off (user.cfg has run by now).
pub fn on_login(ctx: &mut Context<'_>) -> Flow<NoExit> {
    drive(ctx.thread(), Event::LoginShown);
    Flow::Continue
}

/// res_render_output::present right before the game's IDXGISwapChain::Present
/// (0x3CF8FF), on the window's own thread: DXVK makes its Vulkan swap chain
/// inside a Present (its first, and again after a resize or a fullscreen change), and
/// AMD's Windows driver makes a DXGI swap chain on the game's window for it,
/// which DXGI refuses for a topmost window with no frame style: DXVK's
/// fullscreen takes WS_OVERLAPPEDWINDOW's styles away and makes it topmost
/// (a start in fullscreen, the 2026-11-08 report), the game's set_size sets
/// style 0 (Settings to fullscreen). So under DXVK such a window gets WS_POPUP
/// first, once for each refused state (a style Windows does not take is not
/// set and said again at every Present); any other window, and every window
/// without DXVK, is left as it is.
pub fn on_present(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if !matches!(renderer(thread), Renderer::Dxvk(_)) {
        return Flow::Continue;
    }
    let Some(window) = OutputWindow::at_present(ctx) else { return Flow::Continue };
    let before = window.styles(thread);
    let refused = before.for_dxgi();
    // a window DXGI takes ends a refused state: the same styles later are a new one
    let last = REFUSED.with(thread, |last| core::mem::replace(last, refused.map(|_| before)));
    if let Some(style) = refused
        && last != Some(before)
    {
        window.set_style(thread, style);
        let after = window.styles(thread);
        if after.dxgi_takes() {
            // the refused state ended here: the same styles again are a new one
            REFUSED.with(thread, |last| *last = None);
        }
        line(&[Text("dxvk: the window DXGI refuses (style "), Hex(before.style), Text(", extended "),
               Hex(before.extended), Text(") made a popup: style "), Hex(after.style),
               Text(if after.dxgi_takes() { ", DXGI takes it" } else { ", DXGI still refuses it" })]);
    }
    Flow::Continue
}

/// A new match (process_static_match_info 0x27B92D, a replay's chunk 1
/// 0x1F0971): its hud movie can be made at the last match's address, a new
/// root that was not told, so it is told again (as patch 65 tells its own).
fn new_match(thread: &Thread) -> Flow<NoExit> {
    TOLD.set(thread, None);
    Flow::Continue
}

pub fn on_static_info(ctx: &mut Context<'_>) -> Flow<NoExit> {
    new_match(ctx.thread())
}

pub fn on_replay_options(ctx: &mut Context<'_>) -> Flow<NoExit> {
    new_match(ctx.thread())
}

/// update_player_icons' start (0x23AA10), every frame of a match: the hud
/// told the settings when it is another movie or they changed.
pub fn on_update_icons(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(ui) = GameWorldUi::at_update_icons(ctx) else { return Flow::Continue };
    let Some(movie) = ui.hud_movie() else { return Flow::Continue };
    let thread = ctx.thread();
    let now = Told { movie: movie.id(), view: HudView::now(thread) };
    if TOLD.get(thread) != Some(now)
        && movie.invoke_bools(c"root.srp_hud_options",
                           [now.view.hit_markers_hidden, now.view.scores_hidden, now.view.crosshair_hidden,
                            now.view.map_names])
    {
        TOLD.set(thread, Some(now));
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_value_is_one_or_zero() {
        assert_eq!(choice(c"1"), Some(true));
        assert_eq!(choice(c"0"), Some(false));
        assert_eq!(choice(c"on"), None);
        assert_eq!(choice(c""), None);
    }

    #[test]
    fn a_setting_is_found_by_its_name_only() {
        assert!(core::ptr::eq(setting(c"srp_hide_hit_markers").unwrap(), &HIDE_HIT_MARKERS));
        assert!(core::ptr::eq(setting(c"srp_hide_scores").unwrap(), &HIDE_SCORES));
        assert!(core::ptr::eq(setting(c"srp_map_names").unwrap(), &MAP_NAMES));
        assert!(core::ptr::eq(setting(c"srp_dxvk").unwrap(), &DXVK));
        assert!(setting(c"srp_language").is_none());
        assert!(setting(c"srp_hide").is_none());
    }

    /// A user.cfg without the Vulkan options' lines.
    const DEFAULTS: VulkanOptions = VulkanOptions {
        frame_limit: FrameLimit::Off,
        low_latency: false,
        tear_free: false,
        dedicated_gpu: true,
        overlay: Overlay::Off,
    };
    const VULKAN: Choice = Choice::Vulkan(DEFAULTS);

    #[test]
    fn user_cfg_picks_vulkan_with_on_only() {
        assert_eq!(Choice::of_user_cfg(b"localization russian\r\nsrp_dxvk on\r\nr_fullscreen on\r\n"), VULKAN);
        assert_eq!(Choice::of_user_cfg(b"srp_dxvk\ton"), VULKAN);
        for text in [&b"srp_dxvk off\r\n"[..], b"srp_dxvk 1", b"srp_dxvk\r\n", b"srp_dxvk_old on\r\n# srp_dxvk on", b"",
                     b"srp_dxvk_low_latency on\r\nsrp_dxvk_frame_limit 144\r\n"] {
            assert_eq!(Choice::of_user_cfg(text), Choice::DirectX, "{text:?}");
        }
    }

    #[test]
    fn user_cfg_gives_the_vulkan_options_with_dxvk() {
        let text = b"srp_dxvk on\r\nsrp_dxvk_dedicated_gpu off\r\nsrp_dxvk_frame_limit 144\r\nsrp_dxvk_low_latency on\r\n\
                     srp_dxvk_overlay frametimes\r\nsrp_dxvk_tear_free on\r\n";
        let all = VulkanOptions {
            frame_limit: FrameLimit::Fps144,
            low_latency: true,
            tear_free: true,
            dedicated_gpu: false,
            overlay: Overlay::FrameTimes,
        };
        assert_eq!(Choice::of_user_cfg(text), Choice::Vulkan(all));
        // a word the lists do not have, or none, is the first choice; a checkbox's other word its default
        let odd = b"srp_dxvk on\r\nsrp_dxvk_frame_limit 100\r\nsrp_dxvk_overlay \r\nsrp_dxvk_dedicated_gpu 0\r\nsrp_dxvk_tear_free yes\r\n";
        assert_eq!(Choice::of_user_cfg(odd), VULKAN);
        for limit in FrameLimit::ALL {
            let text = [b"srp_dxvk on\r\nsrp_dxvk_frame_limit ", limit.word().to_bytes(), b"\r\n"].concat();
            assert_eq!(Choice::of_user_cfg(&text), Choice::Vulkan(VulkanOptions { frame_limit: *limit, ..DEFAULTS }));
        }
        for overlay in Overlay::ALL {
            let text = [b"srp_dxvk on\r\nsrp_dxvk_overlay ", overlay.word().to_bytes(), b"\r\n"].concat();
            assert_eq!(Choice::of_user_cfg(&text), Choice::Vulkan(VulkanOptions { overlay: *overlay, ..DEFAULTS }));
        }
    }

    #[test]
    fn the_defaults_prefer_the_dedicated_gpu_only() {
        assert_eq!(DEFAULTS.config().as_str(), "dxvk.hideIntegratedGraphics = True");
        let none = VulkanOptions { dedicated_gpu: false, ..DEFAULTS };
        assert_eq!(none.config().as_str(), "");
        assert_eq!(none.config().as_c_str(), c"");
    }

    #[test]
    fn every_option_on_is_one_dxvk_config_line_that_fits() {
        let all = VulkanOptions {
            frame_limit: FrameLimit::Fps240,
            low_latency: true,
            tear_free: true,
            dedicated_gpu: true,
            overlay: Overlay::Full,
        };
        let config = all.config();
        assert_eq!(config.as_str(), "dxvk.maxFrameRate = 240; dxvk.latencySleep = True; dxvk.tearFree = True; \
                                     dxvk.hideIntegratedGraphics = True; dxvk.hud = fps,frametimes,gpuload,compiler,memory");
        assert_eq!(config.as_c_str().to_bytes(), config.as_str().as_bytes());
        // below DXVK's MAX_PATH, and the log's line (its 200 bytes) holds it with its words
        assert!(config.as_str().len() < 259 && config.as_str().len() + "dxvk: DXVK_CONFIG \"\"".len() <= 200);
        let each = [(FrameLimit::Fps60, Overlay::Fps, "dxvk.maxFrameRate = 60; dxvk.hud = fps"),
                    (FrameLimit::Fps165, Overlay::FrameTimes, "dxvk.maxFrameRate = 165; dxvk.hud = fps,frametimes")];
        for (frame_limit, overlay, want) in each {
            let options = VulkanOptions { frame_limit, overlay, dedicated_gpu: false, ..DEFAULTS };
            assert_eq!(options.config().as_str(), want);
        }
    }

    #[test]
    fn an_amd_gpu_gets_no_descriptor_heap_and_every_option_still_fits() {
        let all = VulkanOptions {
            frame_limit: FrameLimit::Fps240,
            low_latency: true,
            tear_free: true,
            dedicated_gpu: true,
            overlay: Overlay::Full,
        };
        let gpu = |vendor| Some(Gpu { vendor, kind: 2, version: Some(VULKAN_NEEDED), name: [0; crate::vostok::renderer::GPU_NAME] });
        let mut config = all.config();
        let players = config.as_str().len();
        assert!(config.for_gpu(gpu(VENDOR_AMD)));
        assert_eq!(&config.as_str()[players..], "; dxvk.enableDescriptorHeap = False");
        assert_eq!(config.left_out(), 0);
        assert!(config.as_str().len() < 259, "below DXVK's MAX_PATH");
        for other in [Some(VENDOR_NVIDIA), Some(VENDOR_INTEL), None] {
            let mut config = all.config();
            assert!(!config.for_gpu(other.and_then(gpu)));
            assert_eq!(config.as_str().len(), players);
        }
        let mut none = VulkanOptions { dedicated_gpu: false, ..DEFAULTS }.config();
        assert!(none.for_gpu(gpu(VENDOR_AMD)));
        assert_eq!(none.as_str(), "dxvk.enableDescriptorHeap = False");
    }

    #[test]
    fn the_vendors_looks_give_dxvk_s_choice() {
        let gpu = |vendor, kind| Some(Gpu { vendor, kind, version: Some(VULKAN_NEEDED), name: [0; crate::vostok::renderer::GPU_NAME] });
        let look = |vendor, pick| VendorLook { vendor, pick };
        // an AMD iGPU beside an RTX: NVIDIA's look alone, a discrete GPU, the end
        let rtx = look(VENDOR_NVIDIA, gpu(VENDOR_NVIDIA, 2));
        assert_eq!(pick_across(&[rtx]).map(|g| g.vendor), Some(VENDOR_NVIDIA));
        // an Intel iGPU beside an RX: NVIDIA's found none, AMD's a discrete one
        let none = look(VENDOR_NVIDIA, None);
        let rx = look(VENDOR_AMD, gpu(VENDOR_AMD, 2));
        assert_eq!(pick_across(&[none, rx]).map(|g| g.vendor), Some(VENDOR_AMD));
        // integrated GPUs alone: the first vendor's
        let amd_igpu = look(VENDOR_AMD, gpu(VENDOR_AMD, 1));
        let intel_igpu = look(VENDOR_INTEL, gpu(VENDOR_INTEL, 1));
        assert_eq!(pick_across(&[none, amd_igpu, intel_igpu]).map(|g| g.vendor), Some(VENDOR_AMD));
        // a discrete GPU after an integrated one is still DXVK's
        assert_eq!(pick_across(&[amd_igpu, look(VENDOR_INTEL, gpu(VENDOR_INTEL, 2))]).map(|g| g.vendor), Some(VENDOR_INTEL));
        assert_eq!(pick_across(&[none]), None);
    }

    #[test]
    fn each_vendor_has_its_drivers_glob() {
        assert_eq!(drivers_of(VENDOR_AMD), Some(c"amd*"));
        assert_eq!(drivers_of(VENDOR_NVIDIA), Some(c"nv*"));
        assert_eq!(drivers_of(VENDOR_INTEL), Some(c"igvk*"));
        assert_eq!(drivers_of(0x5143), None, "Qualcomm's: not known, all drivers");
    }

    #[test]
    fn an_option_that_does_not_fit_is_left_out_whole() {
        let mut config = DxvkConfig { bytes: [0; DxvkConfig::ROOM], len: 0, left_out: 0 };
        for _ in 0..10 {
            config.option("dxvk.hud", c"fps,frametimes,gpuload,compiler,memory");
        }
        let text = config.as_str();
        // only whole options, each `key = value`, and the NUL in its room
        assert!(text.len() < DxvkConfig::ROOM && config.left_out() > 0);
        assert!(text.split("; ").all(|option| option == "dxvk.hud = fps,frametimes,gpuload,compiler,memory"));
        assert_eq!(text.split("; ").count() as u32 + config.left_out(), 10);
        assert_eq!(config.as_c_str().to_bytes(), text.as_bytes());
    }

    #[test]
    fn a_settings_default_is_its_console_variables() {
        assert!(DEDICATED_GPU.of_user_cfg(b"") && !LOW_LATENCY.of_user_cfg(b""));
        assert!(DEDICATED_GPU.value.initial() && !TEAR_FREE.value.initial());
    }

    #[test]
    fn the_settings_index_is_the_lists_order() {
        assert_eq!(FrameLimit::of_index(c"0"), Some(FrameLimit::Off));
        assert_eq!(FrameLimit::of_index(c"3"), Some(FrameLimit::Fps144));
        assert_eq!(FrameLimit::of_index(c"5"), Some(FrameLimit::Fps240));
        assert_eq!(FrameLimit::of_index(c"6"), None);
        assert_eq!(Overlay::of_index(c"3"), Some(Overlay::Full));
        assert_eq!(Overlay::of_index(c"4"), None);
        for text in [c"", c"-1", c"10", c"fps"] {
            assert_eq!(Overlay::of_index(text), None);
        }
        for list in VulkanList::ALL {
            assert!(list.texts().count() <= MAX_CHOICES && list.texts().next() == Some(OFF));
            assert_eq!(VulkanList::named(list.name()), Some(list));
        }
        assert_eq!(VulkanList::FrameLimit.texts().nth(3), Some(ChoiceText::Plain(c"144")));
        assert!(DEFAULTS.shown(c"srp_dxvk_dedicated_gpu") == Some(c"1") && DEFAULTS.shown(c"srp_dxvk_tear_free") == Some(c"0"));
        let options = VulkanOptions { frame_limit: FrameLimit::Fps165, overlay: Overlay::FrameTimes, ..DEFAULTS };
        assert!(options.shown(c"srp_dxvk_frame_limit") == Some(c"4") && options.shown(c"srp_dxvk_overlay") == Some(c"2"));
        assert!(options.shown(c"srp_dxvk").is_none() && options.shown(c"srp_hide_scores").is_none());
    }

    #[test]
    fn the_field_of_view_is_whole_degrees() {
        assert_eq!(SliderSteps::parse(c"50"), Some(SliderSteps::new(50)));
        assert_eq!(SliderSteps::parse(c"100"), Some(SliderSteps::new(100)));
        for text in [c"", c"-5", c"65.5", c"10000", c"fov"] {
            assert_eq!(SliderSteps::parse(text), None, "{text:?}");
        }
        assert_eq!(whole_degrees(74.5), SliderSteps::new(75));
        assert_eq!(whole_degrees(65.0), SliderSteps::new(65));
        let mut room = [0; 8];
        assert_eq!(SliderSteps::new(100).text(&mut room), c"100");
        assert_eq!(SliderSteps::new(7).text(&mut room), c"7");
        assert!(setting(FIELD_OF_VIEW).is_none() && VulkanList::named(FIELD_OF_VIEW).is_none());
    }

    const ENOUGH: Vulkan = Vulkan::Device(VulkanVersion::new(1, 4, 303));

    fn started(choice: Choice, files: bool, marker: bool) -> Event {
        Event::Started { choice, files, marker }
    }

    #[test]
    fn directx_picked_is_directx_and_a_stale_marker_goes() {
        for (files, marker) in [(false, false), (true, false), (true, true), (false, true)] {
            assert_eq!(step(Renderer::Undecided, started(Choice::DirectX, files, marker)), (Renderer::DirectX, Action::Unmark));
        }
        assert_eq!(step(Renderer::DirectX, Event::LoginShown), (Renderer::DirectX, Action::Unmark));
    }

    #[test]
    fn vulkan_picked_with_its_files_marks_looks_and_loads() {
        let options = VulkanOptions { frame_limit: FrameLimit::Fps144, ..DEFAULTS };
        let (state, action) = step(Renderer::Undecided, started(Choice::Vulkan(options), true, false));
        assert_eq!((state, action), (Renderer::Checking(options), Action::MarkAndLook));
        // DXVK given the options user.cfg had at the start, and running with them
        assert_eq!(step(state, Event::Looked(ENOUGH)), (Renderer::Dxvk(options), Action::Load(options)));
        // the login screen shown: the start worked, the marker goes, DXVK stays
        assert_eq!(step(Renderer::Dxvk(options), Event::LoginShown), (Renderer::Dxvk(options), Action::Unmark));
    }

    #[test]
    fn vulkan_picked_without_a_vulkan_1_4_device_is_directx() {
        for found in [Vulkan::Device(VulkanVersion::new(1, 3, 280)), Vulkan::NoLoader, Vulkan::NoInstance, Vulkan::NoDevice] {
            assert_eq!(step(Renderer::Checking(DEFAULTS), Event::Looked(found)), (Renderer::DirectX, Action::Unmark), "{found:?}");
        }
    }

    #[test]
    fn vulkan_picked_without_all_of_dxvk_is_directx() {
        assert_eq!(step(Renderer::Undecided, started(VULKAN, false, false)), (Renderer::DirectX, Action::Unmark));
    }

    #[test]
    fn a_marker_left_is_a_failed_start_turned_off_at_the_login_screen() {
        for files in [true, false] {
            assert_eq!(step(Renderer::Undecided, started(VULKAN, files, true)), (Renderer::TurnOff, Action::Unmark));
        }
        assert_eq!(step(Renderer::TurnOff, Event::LoginShown), (Renderer::DirectX, Action::TurnOff));
        // once: the next login screen of the process only takes a marker
        assert_eq!(step(Renderer::DirectX, Event::LoginShown), (Renderer::DirectX, Action::Unmark));
    }

    #[test]
    fn nothing_but_its_start_decides() {
        for state in [Renderer::Checking(DEFAULTS), Renderer::DirectX, Renderer::Dxvk(DEFAULTS), Renderer::TurnOff] {
            assert_eq!(step(state, started(VULKAN, true, false)), (state, Action::Nothing));
        }
        for state in [Renderer::Undecided, Renderer::DirectX, Renderer::Dxvk(DEFAULTS), Renderer::TurnOff] {
            assert_eq!(step(state, Event::Looked(ENOUGH)), (state, Action::Nothing));
        }
    }

    #[test]
    fn dxvk_loads_dxgi_first_and_d3d9_before_d3d8() {
        let at = |name: &str| DXVK_DLLS.iter().position(|dll| dll.file.to_bytes() == name.as_bytes()).unwrap();
        assert_eq!(at("dxvk\\dxgi.dll"), 0);
        assert!(at("dxvk\\dxgi.dll") < at("dxvk\\d3d11.dll") && at("dxvk\\d3d11.dll") < at("dxvk\\d3d10core.dll"));
        assert!(at("dxvk\\d3d9.dll") < at("dxvk\\d3d8.dll"));
    }

    #[test]
    fn each_delayed_import_of_the_game_is_bound_to_its_own_dll() {
        let serving = |import| DXVK_DLLS.iter().filter(|dll| dll.serves == Some(import)).map(|dll| dll.file.to_bytes()).collect::<Vec<_>>();
        assert_eq!(serving(DelayedImport::Dxgi), [b"dxvk\\dxgi.dll"]);
        assert_eq!(serving(DelayedImport::D3d11), [b"dxvk\\d3d11.dll"]);
        assert_eq!(serving(DelayedImport::D3d9), [b"dxvk\\d3d9.dll"]);
    }

    #[test]
    fn names_and_labels_are_distinct_and_fit() {
        for (i, a) in SETTINGS.iter().enumerate() {
            assert!(a.name.count_bytes() < 64 && a.label.to_bytes().starts_with(b"st_srp_"));
            for b in &SETTINGS[i + 1..] {
                assert!(a.name != b.name && a.label != b.label);
            }
        }
        for (i, key) in TEXTS.iter().enumerate() {
            assert!(key.to_bytes().starts_with(b"st_srp_") && !TEXTS[i + 1..].contains(key));
        }
        assert!(setting(c"srp_hud_texts").is_none());
        for list in VulkanList::ALL {
            assert!(setting(list.name()).is_none() && list.label().to_bytes().starts_with(b"st_srp_"));
            assert!(SETTINGS.iter().all(|s| s.label != list.label()));
        }
        // the lists' words fit their console variables (with the NUL)
        assert!(FrameLimit::ALL.iter().all(|c| c.word().count_bytes() < 8));
        assert!(Overlay::ALL.iter().all(|c| c.word().count_bytes() < 16));
    }
}
