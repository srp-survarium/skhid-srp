"""rows.py <client-patches dir> rows|texts: for KeysWindowHarness, as JSON.

rows: the Controls rows the exe gives the main menu (root.set_keybindings) in English: the `controls` table of
hide_ui_026g.asm in its order (action, label, group, type), each with its default key (item_hotkeys_026g.asm's
`actions`) by the dictionary's name.
texts: a list per dictionary: the window's texts (st_srp_key_bindings, the five kinds, the three groups'
headings), then each row's label in the rows' order."""
import json
import re
import sys
from pathlib import Path

patches = Path(sys.argv[1])
sys.path.insert(0, str(patches))
from localization_026g import CATALOG, LANGUAGES, read_translations  # noqa: E402

WINDOW = ('st_srp_key_bindings', 'st_srp_keys_movement', 'st_srp_keys_combat', 'st_srp_keys_items', 'st_srp_keys_talk',
          'st_srp_keys_interface', 'st_options_game', 'st_options_lobby', 'st_options_chat_keys')


def table():
    """The rows: (action id, label key, group, type, default key's name or None)."""
    defaults = {}
    for line in (patches / 'item_hotkeys_026g.asm').read_text().splitlines():
        m = re.match(r'\s*ACTION\s+\S+,\s*(0x[0-9A-Fa-f]+),[^;]*;\s*(.*)', line)
        if m:
            words = re.split(r'[\s,]+', m.group(2))
            defaults[int(m.group(1), 16)] = next((t for t in words if re.fullmatch(r'k[A-Z0-9]+|mouse\w+', t)), None)
    text = (patches / 'hide_ui_026g.asm').read_text()
    names = {m.group(1): int(m.group(2), 16) for m in re.finditer(r'%define (\w+) (0x[0-9A-Fa-f]+)', text)}
    rows = []
    for line in text[text.index('\ncontrols:') + 1:].splitlines()[1:]:
        m = re.match(r'\s*CONTROL\s+(\w+),\s*\w+,\s*(\d+),\s*(\d+)\s*;\s*(st_\w+)', line)
        if m:
            action = int(m.group(1), 16) if m.group(1).startswith('0x') else names[m.group(1)]
            rows.append((action, m.group(4), int(m.group(2)), int(m.group(3)), defaults.get(action)))
        elif re.match(r'\s*\w+:', line):
            break
    return rows


rows = table()
if sys.argv[2] == 'rows':
    words = read_translations(CATALOG / 'english.tsv')
    print(json.dumps([{'action_id': a, 'label': words[label], 'group_id': group, 'type': kind,
                       'key': words[key] if key else ''} for a, label, group, kind, key in rows],
                     ensure_ascii=False, separators=(',', ':')))
else:
    out = []
    for name, *_ in LANGUAGES:
        words = read_translations(CATALOG / f'{name}.tsv')
        out.append([words[k] for k in WINDOW] + [words[label] for _, label, *_ in rows])
    print(json.dumps(out, ensure_ascii=False, separators=(',', ':')))
