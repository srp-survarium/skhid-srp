#!/usr/bin/env bash
# check.sh <main_menu.swf> [work dir]: the Controls' action selector (SrpActionSelector's panel,
# survarium/menu/selector) in Ruffle, on a private CPU-only virtual display, at 1280x720 and 1920x1080:
# ActionSelectorHarness.as opens it over the Settings with the options' real rows (hide_ui_026g.asm's
# `controls`, every dictionary's labels; the keys made up) and the exe's texts (hud_options.rs's
# CONTROLS_TEXTS), picks, unbinds, cancels, searches, scrolls and checks every language's texts fit.
# Screenshots into the work dir: selector-<size>-en.png, -ru.png (editing Q, Jump highlighted: its
# warning), -search.png. The embedded font's glyphs are checked for every text first. Exits 0 when
# both harness runs say pass. No game, no exe.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
menu=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

data=$("${PYTHON:-python3}" - "$here/../../client-patches" "$menu" <<'PY'
import json, re, struct, sys, zlib
from pathlib import Path
patches = Path(sys.argv[1])
sys.path.insert(0, str(patches))
from localization_026g import CATALOG, LANGUAGES, read_translations

# The options' rows: action, label key, group, type (hide_ui_026g.asm's `controls`).
asm = (patches / 'hide_ui_026g.asm').read_text()
defines = dict(re.findall(r'^%define (\w+) (0x[0-9A-Fa-f]+)', asm, re.M))
table = []
for action, group, kind, label in re.findall(r'^\s+CONTROL (\w+), \w+, (\d), (\d)\s+; (st_\w+)', asm, re.M):
    table.append((int(defines.get(action, action), 16), label, int(group), int(kind)))
assert len(table) == 61, len(table)
# The texts' keys in root.srp_controls_texts' order.
rust = (patches / 'rust/patches/src/hud_options.rs').read_text()
block = re.search(r'pub const CONTROLS_TEXTS: \[&CStr; \d+\] = \[(.*?)\];', rust, re.S).group(1)
keys = re.findall(r'c"(\w+)"', block)
assert len(keys) == 16, keys
# Made-up keys (a default-like layout): Q is spot's, C crouch's toggle and the lobby's Character.
KEYS = {0x0B: 'W', 0x0C: 'S', 0x0D: 'A', 0x0E: 'D', 0x04: 'Space', 0x05: 'Left Ctrl', 0x06: 'C', 0x07: 'Left Shift',
        0x2A: 'Left Shift', 0x22: 'Q', 0x26: 'Mouse 1', 0x28: 'Mouse 2', 0x27: 'R', 0x1D: '1', 0x1E: '2', 0x20: 'G',
        0x21: 'V', 0x09: 'B', 0x0A: 'N', 0x24: 'Wheel Up', 0x25: 'Wheel Down', 0x35: 'F', 0x36: 'X',
        0x39: '3', 0x3A: '4', 0x3B: '5', 0x3C: '6', 0x3D: '7', 0x3E: '8', 0x3F: 'Y', 0x45: 'Enter', 0x46: 'Tab',
        0x47: 'T', 0x37: 'Z', 0x23: 'H', 0x40: 'C', 0x41: 'I', 0x42: 'P', 0x43: 'O', 0x44: 'K', 0x19: 'F9',
        0x1A: 'F11', 0x1B: 'F12', 0x4E: 'Mouse 4', 0x58: 'Mouse 5'}
languages = []
for name, *_ in LANGUAGES:
    words = read_translations(CATALOG / f'{name}.tsv')
    rows = [[action, words[label].strip(), group, kind, KEYS.get(action, '')] for action, label, group, kind in table]
    languages.append([name, [words[k] for k in keys], rows])

# Every text's characters in the menu's Calibri Bold (DefineFont3 29, its wide code table).
swf = Path(sys.argv[2]).read_bytes()
body = zlib.decompress(swf[8:]) if swf[:3] == b'CWS' else swf[8:]
pos = (5 + 4 * (body[0] >> 3) + 7) // 8 + 4
codes = None
while pos < len(body):
    word, = struct.unpack_from('<H', body, pos); pos += 2
    kind, size = word >> 6, word & 63
    if size == 63:
        size, = struct.unpack_from('<I', body, pos); pos += 4
    tag = body[pos:pos + size]; pos += size
    if kind == 75 and struct.unpack_from('<H', tag)[0] == 29:
        flags, length = tag[2], tag[4]
        glyphs, = struct.unpack_from('<H', tag, 5 + length)
        start = 7 + length
        wide = flags & 0x08
        table_at, = struct.unpack_from('<I' if wide else '<H', tag, start + glyphs * (4 if wide else 2))
        codes = set(struct.unpack_from(f'<{glyphs}H', tag, start + table_at))
assert codes, 'no Calibri Bold in the menu'
missing = set()
for name, texts, rows in languages:
    for text in texts + [r[1] for r in rows] + [r[4] for r in rows] + ['…—', 'ENTER Esc', 'ABCDEFGHIJKLMNOPQRSTUVWXYZ']:
        for ch in text.upper() + text:
            if ord(ch) not in codes and not ch.isspace():
                missing.add((name, ch))
for name, ch in sorted(missing):
    print(f'SELECTOR GLYPH missing {name} {ch!r} U+{ord(ch):04X}', file=sys.stderr)
print(json.dumps(languages, ensure_ascii=False, separators=(',', ':')))
sys.exit(1 if missing else 0)
PY
)

cp "$menu" "$work/main_menu.swf"
status=0
for size in 1280x720 1920x1080; do
    w=${size%x*} h=${size#*x}
    harness_compile "$here/ActionSelectorHarness.as" "$w,$h"
    mv "$work/ActionSelectorHarness.swf" "$work/ActionSelectorHarness-$size.swf"
    harness_display "$size" ffmpeg-full
    log=ActionSelectorHarness-$size.log
    harness_run "$log" -p high --no-gui --dummy-external-interface --width "$w" --height "$h" "-Pdata=$data" \
        "ActionSelectorHarness-$size.swf"
    for shot in en ru search; do
        wait_for "$log" "SELECTOR SHOT $shot\$\|SELECTOR RESULT\|SELECTOR END" 900 || true
        if grep -q "SELECTOR SHOT $shot\$" "$work/$log"; then
            ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "$size" -i "$display" -frames:v 1 \
                "$work/selector-$size-$shot.png"
        fi
    done
    wait_for "$log" 'SELECTOR RESULT' 1200 || true
    harness_stop
    harness_report 'SELECTOR .*' "$log"
    grep -q 'SELECTOR RESULT pass' "$work/$log" || status=1
done
exit $status
