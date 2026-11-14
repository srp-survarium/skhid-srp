#!/usr/bin/env bash
# check.sh <main_menu.swf> [work dir]: the Settings' "Test microphone" button and "Microphone" list (exe
# patch 66, docs/notes/voice-chat.md, "Microphone test", "Microphone") in Ruffle, on a private CPU-only virtual display, the
# exe stood in for: MicTestHarness.as. Screenshots of the sound tab with the button in each state
# into the work dir (mic-test-idle.png, -recording.png, -playing.png, -no-microphone.png,
# -no-playback.png, -microphones.png: the list open). Exits 0 when the harness says pass. No game, no voice DLL; Ruffle is given no
# audio device (a null ALSA one, and no sound server).
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
menu=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# Every dictionary's texts: the button's five, the sound tab's stock ones, the Microphone list's two
# (MicTestHarness's rows).
texts=$("${PYTHON:-python3}" - "$here/../../client-patches" <<'PY'
import json, sys
sys.path.insert(0, sys.argv[1])
from localization_026g import CATALOG, LANGUAGES, read_translations
keys = ('st_srp_mic_test', 'st_srp_mic_test_recording', 'st_srp_mic_test_playing', 'st_srp_mic_test_no_microphone',
        'st_srp_mic_test_no_playback',
        'st_general_volume_option', 'st_ingame_volume_option', 'st_music_volume_option', 'st_voice_chat_volume_option',
        'st_use_microphone_option', 'st_microphone_sensitivity_option', 'st_ptt_button_option', 'st_options_volume',
        'st_options_voice', 'st_options_sound', 'st_srp_microphone', 'st_srp_microphone_default')
print(json.dumps([[read_translations(CATALOG / f'{name}.tsv')[k] for k in keys] for name, *_ in LANGUAGES],
                 ensure_ascii=False, separators=(',', ':')))
PY
)

harness_compile "$here/MicTestHarness.as" 1280,720
cp "$menu" "$work/main_menu.swf"
harness_display 1280x720 ffmpeg-full

# Ruffle plays nothing here, and is kept from the machine's audio devices all the same.
printf 'pcm.!default { type null }\nctl.!default { type null }\n' >"$work/asound.conf"
export ALSA_CONFIG_PATH=$work/asound.conf PULSE_SERVER=unix:$work/no-pulse PIPEWIRE_REMOTE=$work/no-pipewire
unset JACK_DEFAULT_SERVER

# The dummy external interface: the Settings' own calls of the exe (a tab's change) return as in the game.
harness_run MicTestHarness.log -p high --no-gui --dummy-external-interface --width 1280 --height 720 "-Ptexts=$texts" \
    MicTestHarness.swf
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size 1280x720 -i "$display" -frames:v 1 "$work/mic-test-$1.png"; }
for state in idle recording playing no-microphone no-playback microphones; do
    wait_for MicTestHarness.log "MICTEST SHOT $state\$\|MICTEST RESULT fail\|MICTEST END" 600 || true
    if grep -q "MICTEST SHOT $state\$" "$work/MicTestHarness.log"; then shot "$state"; fi
done
wait_for MicTestHarness.log 'MICTEST RESULT fail\|MICTEST END' 600 || true
harness_stop

harness_report 'MICTEST .*' MicTestHarness.log
! grep -q "RESULT fail" "$work/MicTestHarness.log" && grep -q "MICTEST RESULT pass" "$work/MicTestHarness.log"
