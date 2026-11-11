#!/usr/bin/env bash
# check.sh <main_menu.swf> <hud.swf> [work dir]: the Settings' "Hide hit markers" and "Hide player
# scores" (exe patch 83, docs/notes/hide-hit-markers-scores.md) in Ruffle, on a private CPU-only
# virtual display:
# - SettingsHarness.as: the HUD group (its heading, the two checkboxes) from the exe's answers, the
#   Gameplay tab's two columns (Friends, HUD; Crosshair, Language, Quest progress), a toggle of each
#   pending until Apply, Cancel, the tabs, every dictionary's labels (inside the panel, their glyphs)
#   and the field of view group under the HUD group (patch 83's uro_fov: the stock slider, its heading in the headings' look, Apply, Cancel);
#   and its crosshair and minimap position choices (inside their lists); the Graphics tab's
#   Renderer list (docs/notes/dxvk-option.md: under Monitor, the rows under it
#   moved, every dictionary's label in the column, Apply, Cancel) and its Vulkan options... button under it: disabled
#   while the list shows DirectX 11, its panel over the column (the five options, every dictionary's texts
#   inside it), closed by a press outside it or Esc, each option pending until Apply, Cancel, the note;
# - HudHarness.as: the crosshair's hit marker shown off and hidden on; the Tab list off as today,
#   on names only, each team by clan tag then name (clanless next, bots last; case-insensitive);
#   the Tab map's names with Alt, and "Names on map" reversing Alt.
# - SettingsHarness.as also: the controls tab's "Lock vertical sensitivity" and its slider (exe
#   patch 112, docs/notes/mouse-pitch.md): locked it follows the main slider x 0.5625, unlocked it is its own;
#   both sliders at their borders (0.01, 10), Apply and the exe's answers there.
# Screenshots into the work dir: settings.png (at rest), settings-checked.png (a toggle, Apply
# enabled), settings-crosshair-open.png, settings-minimap-open.png, settings-graphics.png (Vulkan picked),
# settings-vulkan.png (its options' panel open), settings-hint.png (an option's hint),
# settings-renderer-open.png (the Renderer list open); the Graphics shots draw checkbox labels centred on their
# box, as Scaleform does (Ruffle draws them at their field's top),
# settings-controls-off.png, settings-controls-on.png, list-scores.png, list-names.png. Exits 0 when both say
# pass. No game, no server.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
menu=$(readlink -f "$1")
hud=$(readlink -f "$2")
. "$here/../ruffle_harness.sh"
harness_work "${3:-}"

# Every dictionary's texts of the HUD group and the Language choice (SettingsHarness.check), as JSON.
texts=$("${PYTHON:-python3}" - "$here/../../client-patches" <<'PY'
import json, sys
sys.path.insert(0, sys.argv[1])
from localization_026g import CATALOG, LANGUAGES, read_translations
keys = ('st_srp_hide_hit_markers_option', 'st_srp_hide_scores_option', 'st_srp_options_hud', 'st_srp_language_option',
        'st_srp_language_restart', 'st_srp_hide_quests_option', 'st_srp_crosshair_dynamic', 'st_srp_crosshair_static',
        'st_srp_crosshair_hidden', 'st_srp_static_minimap_option', 'st_minimap_off', 'st_minimap_bottom', 'st_minimap_top',
        'st_srp_map_names_option', 'st_srp_renderer_option', 'st_srp_renderer_dx11', 'st_srp_renderer_dxvk',
        'st_srp_mouse_lock_option', 'st_srp_mouse_vertical_option', 'st_srp_vulkan_options',
        'st_srp_vulkan_frame_limit', 'st_srp_vulkan_off', 'st_srp_vulkan_low_latency', 'st_srp_vulkan_tear_free',
        'st_srp_vulkan_dedicated_gpu', 'st_srp_vulkan_overlay', 'st_srp_vulkan_overlay_fps',
        'st_srp_vulkan_overlay_frametimes', 'st_srp_vulkan_overlay_full', 'st_fov_option',
        'st_srp_vulkan_frame_limit_hint', 'st_srp_vulkan_overlay_hint', 'st_srp_vulkan_low_latency_hint',
        'st_srp_vulkan_tear_free_hint', 'st_srp_vulkan_dedicated_gpu_hint')
print(json.dumps([[read_translations(CATALOG / f'{name}.tsv')[k] for k in keys] for name, *_ in LANGUAGES],
                 ensure_ascii=False, separators=(',', ':')))
PY
)

for harness in SettingsHarness HudHarness; do
    harness_compile "$here/$harness.as" 1280,720
done
cp "$menu" "$work/main_menu.swf"
cp "$hud" "$work/hud.swf"
harness_display 1280x720 ffmpeg-full

# run <harness>: Ruffle on the harness in the background, its log <harness>.log
run() { harness_run "$1.log" -p high --no-gui --width 1280 --height 720 "-Ptexts=$texts" "$1.swf"; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size 1280x720 -i "$display" -frames:v 1 "$work/$1"; }

run SettingsHarness
wait_for SettingsHarness.log 'SETTINGS SHOT settings' && shot settings.png
wait_for SettingsHarness.log 'SETTINGS SHOT checked' && shot settings-checked.png
wait_for SettingsHarness.log 'SETTINGS SHOT open' && shot settings-crosshair-open.png
wait_for SettingsHarness.log 'SETTINGS SHOT minimap' && shot settings-minimap-open.png
wait_for SettingsHarness.log 'SETTINGS SHOT graphics' && shot settings-graphics.png
wait_for SettingsHarness.log 'SETTINGS SHOT vulkan' && shot settings-vulkan.png
wait_for SettingsHarness.log 'SETTINGS SHOT hint' && shot settings-hint.png
wait_for SettingsHarness.log 'SETTINGS SHOT renderer-open' && shot settings-renderer-open.png
wait_for SettingsHarness.log 'SETTINGS SHOT controls-off' && shot settings-controls-off.png
wait_for SettingsHarness.log 'SETTINGS SHOT controls-on' && shot settings-controls-on.png
kill $ruffle_pid 2>/dev/null || true
run HudHarness
wait_for HudHarness.log 'HUD SHOT scores' && shot list-scores.png
wait_for HudHarness.log 'HUD SHOT names' && shot list-names.png
harness_stop

harness_report 'SETTINGS .*\|HUD .*' SettingsHarness.log HudHarness.log
! grep -q "RESULT fail" "$work/SettingsHarness.log" "$work/HudHarness.log" \
    && grep -q "SETTINGS RESULT pass" "$work/SettingsHarness.log" && grep -q "SETTINGS controls pass" "$work/SettingsHarness.log" \
    && grep -q "SETTINGS borders pass" "$work/SettingsHarness.log" && grep -q "SETTINGS apply-cancel pass" "$work/SettingsHarness.log" \
    && grep -q "SETTINGS Graphics: Vulkan options pass" "$work/SettingsHarness.log" \
    && grep -q "HUD RESULT pass" "$work/HudHarness.log"
