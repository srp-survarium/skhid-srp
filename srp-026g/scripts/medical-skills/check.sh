#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: medical skills in isolated CPU Ruffle.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/MedicalSkillsHarness.as" 1280,720
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
# The tree as the edit list makes it, and the PNGs the movie embeds (pixel comparison).
cp "$here/../content-edit/medical-skills-1.json" "$work/"
mkdir -p "$work/bitmaps"
cp "$here"/../../flash/lobby_menu/bitmaps/MedicalSkill069*.png "$work/bitmaps/"
harness_display 1280x720
harness_run MedicalSkillsHarness.log -p low --width 1280 --height 720 -Pmovie=lobby_menu.swf MedicalSkillsHarness.swf
wait_for MedicalSkillsHarness.log 'MEDICALSKILLS RESULT' || true
harness_stop
harness_report 'MEDICALSKILLS .*' MedicalSkillsHarness.log
grep -q 'MEDICALSKILLS RESULT pass' "$work/MedicalSkillsHarness.log"
