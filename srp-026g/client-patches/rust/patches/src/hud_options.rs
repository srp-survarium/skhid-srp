//! Patch 83: the Settings' "HUD:" group (docs/notes/hide-hit-markers-scores.md):
//! four settings of the match hud kept in user.cfg, "Hide hit markers", "Hide
//! player scores", "Names on map" and the crosshair list's "Hidden crosshair"; and the
//! Graphics tab's Renderer list (docs/notes/dxvk-option.md), which the next
//! start reads. The hooks are
//! `hud_options.toml`'s, each a `lohook` after another patch's at the same site:
//!
//! - on_configs_loaded before user.cfg (`configs.start`, after patch 79):
//!   the console variables `srp_hide_hit_markers`, `srp_hide_scores`,
//!   `srp_map_names`, `srp_hide_crosshair` and `srp_dxvk`, saved in user.cfg, off by default;
//! - the Settings' call (`options.callback`, after patch 79): a setting's
//!   name answers the main menu with `root.srp_check_option(name, label,
//!   saved)`, and with "1" or "0" saves that first (user.cfg at once);
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
//!   the marker `dxvk.pending` is written: a module of a name serves every
//!   later load of that plain name, so the game's delayed d3d9, dxgi and
//!   d3d11 are DXVK's. A marker left is a start with DXVK that never showed
//!   its login screen: plain DirectX, and the setting goes off;
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

use crate::vostok::log::{Piece::*, line};
use crate::vostok::renderer::{Vulkan, VulkanVersion, best_vulkan, exists_beside_exe, load_beside_exe, with_user_cfg};
use crate::vostok::settings::{ConsoleBool, OptionsCall, create_file, delete_file, file_exists, save_user_config};
use crate::vostok::{Context, Flow, GameCell, GameWorldUi, HudOptionsOnOptionsCallExit, NoExit, Thread};

/// A Settings checkbox kept in user.cfg as `<name> on|off`: the Settings ask
/// for it by the same name; its label is a dictionary key.
pub struct Setting {
    pub name: &'static CStr,
    pub label: &'static CStr,
    value: ConsoleBool,
}

impl Setting {
    const fn new(name: &'static CStr, label: &'static CStr) -> Setting {
        Setting { name, label, value: ConsoleBool::new() }
    }

    fn on(&self, thread: &Thread) -> bool {
        self.value.get(thread)
    }
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

/// The marker of a start with DXVK that has not shown its login screen yet,
/// in the game's working folder (`binaries\x86`).
pub const DXVK_MARKER: &CStr = c"dxvk.pending";
/// DXVK's DLLs (`make_client_026g.py`'s `DXVK_DLLS`, `winetricks dxvk`'s set),
/// in their load order: each one's imports by name are of those before it
/// (d3d11 imports dxgi, d3d10core d3d11, d3d8 d3d9).
pub const DXVK_DLLS: [&CStr; 5] =
    [c"dxvk\\dxgi.dll", c"dxvk\\d3d11.dll", c"dxvk\\d3d10core.dll", c"dxvk\\d3d9.dll", c"dxvk\\d3d8.dll"];
/// What DXVK 3 needs of a device.
const VULKAN_NEEDED: VulkanVersion = VulkanVersion::new(1, 4, 0);

/// The Renderer list's choice as user.cfg keeps it: `srp_dxvk on` is Vulkan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    DirectX,
    Vulkan,
}

impl Choice {
    /// The first `srp_dxvk` line's value (`on`, `off`; the game writes one).
    pub fn of_user_cfg(text: &[u8]) -> Choice {
        let value = text.split(|&b| b == b'\n').find_map(|line| {
            let rest = line.strip_prefix(b"srp_dxvk")?;
            matches!(rest.first(), Some(b' ' | b'\t')).then_some(rest)
        });
        let word = value.and_then(|rest| rest.split(|&b| b <= b' ').find(|w| !w.is_empty()));
        if word == Some(b"on") { Choice::Vulkan } else { Choice::DirectX }
    }
}

/// The renderer of this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Renderer {
    /// Before the process's start hook.
    Undecided,
    /// Vulkan picked: its marker written, the Vulkan look asked.
    Checking,
    /// Windows' own Direct3D.
    DirectX,
    /// DXVK's DLLs loaded (all, or as many as loaded: the marker covers a
    /// crash); the login screen takes the marker.
    Dxvk,
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
    /// Load DXVK's DLLs in order (the marker stays).
    Load,
    /// Delete the marker.
    Unmark,
    /// The setting off, user.cfg saved, the marker deleted.
    TurnOff,
}

/// The one transition of the renderer's choice.
pub fn step(state: Renderer, event: Event) -> (Renderer, Action) {
    match (state, event) {
        (Renderer::Undecided, Event::Started { choice: Choice::Vulkan, marker: true, .. }) => {
            (Renderer::TurnOff, Action::Unmark)
        }
        (Renderer::Undecided, Event::Started { choice: Choice::Vulkan, files: true, marker: false }) => {
            (Renderer::Checking, Action::MarkAndLook)
        }
        (Renderer::Undecided, Event::Started { .. }) => (Renderer::DirectX, Action::Unmark),
        (Renderer::Checking, Event::Looked(Vulkan::Device(version))) if version >= VULKAN_NEEDED => {
            (Renderer::Dxvk, Action::Load)
        }
        (Renderer::Checking, Event::Looked(_)) => (Renderer::DirectX, Action::Unmark),
        (Renderer::TurnOff, Event::LoginShown) => (Renderer::DirectX, Action::TurnOff),
        (state, Event::LoginShown) => (state, Action::Unmark),
        (state, _) => (state, Action::Nothing),
    }
}

static RENDERER: GameCell<Renderer> = GameCell::new(Renderer::Undecided);

/// `event` through [`step`], its action done; an action that finds something
/// (the Vulkan look) goes on with it.
fn drive(thread: &Thread, event: Event) {
    let mut event = Some(event);
    while let Some(now) = event.take() {
        let (next, action) = step(RENDERER.get(thread), now);
        RENDERER.set(thread, next);
        match action {
            Action::Nothing => {}
            Action::Unmark => delete_file(thread, DXVK_MARKER),
            Action::MarkAndLook => {
                // first: a driver that crashes in the look leaves it
                create_file(thread, DXVK_MARKER);
                event = Some(Event::Looked(best_vulkan(thread)));
            }
            Action::Load => match DXVK_DLLS.iter().position(|name| !load_beside_exe(thread, name)) {
                None => line(&[Text("dxvk: DXVK loaded from dxvk\\")]),
                Some(i) => line(&[Text("dxvk: loading DXVK stopped at DLL "), Dec(i as u32),
                                  Text(" of dxgi, d3d11, d3d10core, d3d9, d3d8 (from 0)")]),
            },
            Action::TurnOff => {
                DXVK.value.set(thread, false);
                save_user_config(thread);
                delete_file(thread, DXVK_MARKER);
                line(&[Text("dxvk: the last start with DXVK never showed its login screen: DirectX 11 again")]);
            }
        }
        if let (Renderer::Checking, Some(Event::Looked(found))) = (next, event)
            && !matches!(found, Vulkan::Device(version) if version >= VULKAN_NEEDED)
        {
            line(&[Text("dxvk: Vulkan/DXVK picked, but no Vulkan 1.4 device: DirectX 11")]);
        }
    }
}


/// Every setting of this patch, as the Settings call them.
pub static SETTINGS: [&Setting; 5] = [&HIDE_HIT_MARKERS, &HIDE_SCORES, &MAP_NAMES, &HIDE_CROSSHAIR, &DXVK];

/// The Settings' "HUD:" group's own texts (dictionary keys), in the order
/// `root.srp_hud_texts` gives them: the heading, the stock quest progress
/// checkbox's label as the group shows it (inverted), the crosshair list's
/// three choices, the stock fixed minimap checkbox's label; then the
/// Graphics tab's Renderer list's two choices and its note (the Language
/// choice's "Applies after restart.").
pub const TEXTS: [&CStr; 9] = [c"st_srp_options_hud", c"st_srp_hide_quests_option", c"st_srp_crosshair_dynamic",
                               c"st_srp_crosshair_static", c"st_srp_crosshair_hidden", c"st_srp_static_minimap_option",
                               c"st_srp_renderer_dx11", c"st_srp_renderer_dxvk", c"st_srp_language_restart"];

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
    Flow::Continue
}

/// The process's start (memory::platform::allocate_arenas 0xE9580, once):
/// the renderer chosen from user.cfg, `dxvk\` and the marker.
pub fn on_process_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let choice = with_user_cfg(thread, Choice::of_user_cfg).unwrap_or(Choice::DirectX);
    let files = DXVK_DLLS.iter().all(|name| exists_beside_exe(thread, name));
    let marker = file_exists(thread, DXVK_MARKER);
    drive(thread, Event::Started { choice, files, marker });
    Flow::Continue
}

/// The Settings' calls: a setting's name answers its label and saved state
/// (`root.srp_check_option`), saving a "1" or "0" first; `srp_hud_texts`
/// answers the group's texts (`root.srp_hud_texts`).
pub fn on_options_call(ctx: &mut Context<'_>) -> Flow<HudOptionsOnOptionsCallExit> {
    let Some(call) = OptionsCall::at(ctx) else { return Flow::Continue };
    if call.method() == c"srp_hud_texts" {
        let mut rooms = [[0; 0x200]; TEXTS.len()];
        let mut texts: [&CStr; TEXTS.len()] = [c""; TEXTS.len()];
        for ((text, room), key) in texts.iter_mut().zip(rooms.iter_mut()).zip(TEXTS) {
            *text = call.translate(key, room);
        }
        call.answer(c"root.srp_hud_texts", &texts);
        return Flow::Exit(HudOptionsOnOptionsCallExit::Done);
    }
    let Some(setting) = setting(call.method()) else { return Flow::Continue };
    let thread = ctx.thread();
    if let Some(on) = call.string_argument(0).and_then(choice) {
        setting.value.set(thread, on);
        save_user_config(thread);
    }
    let mut label = [0; 0x200];
    let label = call.translate(setting.label, &mut label);
    let saved = if setting.on(thread) { c"1" } else { c"0" };
    if core::ptr::eq(setting, &DXVK) {
        // and the renderer this process runs ("1": DXVK loaded at its start), for the list's note
        let running = if RENDERER.get(thread) == Renderer::Dxvk { c"1" } else { c"0" };
        call.answer(c"root.srp_check_option", &[setting.name, label, saved, running]);
    } else {
        call.answer(c"root.srp_check_option", &[setting.name, label, saved]);
    }
    Flow::Exit(HudOptionsOnOptionsCallExit::Done)
}

/// The login screen shown (login_menu::on_activate 0x21EAC8): the renderer
/// works, with DXVK or not; the marker goes, and after a failed DXVK start
/// the setting goes off (user.cfg has run by now).
pub fn on_login(ctx: &mut Context<'_>) -> Flow<NoExit> {
    drive(ctx.thread(), Event::LoginShown);
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

    #[test]
    fn user_cfg_picks_vulkan_with_on_only() {
        assert_eq!(Choice::of_user_cfg(b"localization russian\r\nsrp_dxvk on\r\nr_fullscreen on\r\n"), Choice::Vulkan);
        assert_eq!(Choice::of_user_cfg(b"srp_dxvk\ton"), Choice::Vulkan);
        for text in [&b"srp_dxvk off\r\n"[..], b"srp_dxvk 1", b"srp_dxvk\r\n", b"srp_dxvk_old on\r\n# srp_dxvk on", b""] {
            assert_eq!(Choice::of_user_cfg(text), Choice::DirectX, "{text:?}");
        }
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
        let (state, action) = step(Renderer::Undecided, started(Choice::Vulkan, true, false));
        assert_eq!((state, action), (Renderer::Checking, Action::MarkAndLook));
        assert_eq!(step(state, Event::Looked(ENOUGH)), (Renderer::Dxvk, Action::Load));
        // the login screen shown: the start worked, the marker goes, DXVK stays
        assert_eq!(step(Renderer::Dxvk, Event::LoginShown), (Renderer::Dxvk, Action::Unmark));
    }

    #[test]
    fn vulkan_picked_without_a_vulkan_1_4_device_is_directx() {
        for found in [Vulkan::Device(VulkanVersion::new(1, 3, 280)), Vulkan::NoLoader, Vulkan::NoInstance, Vulkan::NoDevice] {
            assert_eq!(step(Renderer::Checking, Event::Looked(found)), (Renderer::DirectX, Action::Unmark), "{found:?}");
        }
    }

    #[test]
    fn vulkan_picked_without_all_of_dxvk_is_directx() {
        assert_eq!(step(Renderer::Undecided, started(Choice::Vulkan, false, false)), (Renderer::DirectX, Action::Unmark));
    }

    #[test]
    fn a_marker_left_is_a_failed_start_turned_off_at_the_login_screen() {
        for files in [true, false] {
            assert_eq!(step(Renderer::Undecided, started(Choice::Vulkan, files, true)), (Renderer::TurnOff, Action::Unmark));
        }
        assert_eq!(step(Renderer::TurnOff, Event::LoginShown), (Renderer::DirectX, Action::TurnOff));
        // once: the next login screen of the process only takes a marker
        assert_eq!(step(Renderer::DirectX, Event::LoginShown), (Renderer::DirectX, Action::Unmark));
    }

    #[test]
    fn nothing_but_its_start_decides() {
        for state in [Renderer::Checking, Renderer::DirectX, Renderer::Dxvk, Renderer::TurnOff] {
            assert_eq!(step(state, started(Choice::Vulkan, true, false)), (state, Action::Nothing));
        }
        for state in [Renderer::Undecided, Renderer::DirectX, Renderer::Dxvk, Renderer::TurnOff] {
            assert_eq!(step(state, Event::Looked(ENOUGH)), (state, Action::Nothing));
        }
    }

    #[test]
    fn dxvk_loads_dxgi_first_and_d3d9_before_d3d8() {
        let at = |name: &str| DXVK_DLLS.iter().position(|n| n.to_bytes() == name.as_bytes()).unwrap();
        assert_eq!(at("dxvk\\dxgi.dll"), 0);
        assert!(at("dxvk\\dxgi.dll") < at("dxvk\\d3d11.dll") && at("dxvk\\d3d11.dll") < at("dxvk\\d3d10core.dll"));
        assert!(at("dxvk\\d3d9.dll") < at("dxvk\\d3d8.dll"));
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
    }
}
