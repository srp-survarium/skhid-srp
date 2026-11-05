//! Patch 83: the Settings' "HUD:" group (docs/notes/hide-hit-markers-scores.md):
//! three settings of the match hud kept in user.cfg, "Hide hit markers", "Hide
//! player scores" and the crosshair list's "Hidden crosshair". The hooks are
//! `hud_options.toml`'s, each a `lohook` after another patch's at the same site:
//!
//! - on_configs_loaded before user.cfg (`configs.start`, after patch 79):
//!   the console variables `srp_hide_hit_markers`, `srp_hide_scores` and
//!   `srp_hide_crosshair`, saved in user.cfg, off by default;
//! - the Settings' call (`options.callback`, after patch 79): a setting's
//!   name answers the main menu with `root.srp_check_option(name, label,
//!   saved)`, and with "1" or "0" saves that first (user.cfg at once);
//!   `srp_hud_texts` answers the group's own texts (`root.srp_hud_texts`:
//!   the heading, "Hide quests", the crosshair list's three choices,
//!   "Static minimap");
//! - each frame's player icons update (`player_icons.update_start`, after
//!   patch 65): the hud movie is told the three settings
//!   (`root.srp_hud_options(hide hit markers, hide scores, hide crosshair)`),
//!   once for each change of the movie or of a setting;
//! - a match's options read (`static_info.options_read`,
//!   `replay.options_read`, after patch 65): a new match, whose hud may be
//!   another at the last one's address, is told again.
//!
//! What the hud does with them is the hud's (HUD.as): no hit marker on the
//! crosshair; a Tab list of names only; no crosshair. Nothing a match
//! computes: the interface only.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::settings::{ConsoleBool, OptionsCall, save_user_config};
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
/// The crosshair list's third choice, "Hidden crosshair" (Dynamic and Static
/// are the stock static crosshair setting's two states).
pub static HIDE_CROSSHAIR: Setting = Setting::new(c"srp_hide_crosshair", c"st_srp_crosshair_hidden");

/// Every setting of this patch, as the Settings call them.
pub static SETTINGS: [&Setting; 3] = [&HIDE_HIT_MARKERS, &HIDE_SCORES, &HIDE_CROSSHAIR];

/// The Settings' "HUD:" group's own texts (dictionary keys), in the order
/// `root.srp_hud_texts` gives them: the heading, the stock quest progress
/// checkbox's label as the group shows it (inverted), the crosshair list's
/// three choices, the stock fixed minimap checkbox's label.
pub const TEXTS: [&CStr; 6] = [c"st_srp_options_hud", c"st_srp_hide_quests_option", c"st_srp_crosshair_dynamic",
                               c"st_srp_crosshair_static", c"st_srp_crosshair_hidden", c"st_srp_static_minimap_option"];

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

/// What the hud is told: the three settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudView {
    pub hit_markers_hidden: bool,
    pub scores_hidden: bool,
    pub crosshair_hidden: bool,
}

impl HudView {
    fn now(thread: &Thread) -> HudView {
        HudView {
            hit_markers_hidden: HIDE_HIT_MARKERS.on(thread),
            scores_hidden: HIDE_SCORES.on(thread),
            crosshair_hidden: HIDE_CROSSHAIR.on(thread),
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
    call.answer(c"root.srp_check_option", &[setting.name, label, saved]);
    Flow::Exit(HudOptionsOnOptionsCallExit::Done)
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
                           [now.view.hit_markers_hidden, now.view.scores_hidden, now.view.crosshair_hidden])
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
        assert!(setting(c"srp_language").is_none());
        assert!(setting(c"srp_hide").is_none());
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
