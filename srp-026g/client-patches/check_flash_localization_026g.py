"""Check the custom Flash text tables alongside the native dictionaries."""
import json
from pathlib import Path
import re

from localization_026g import LANGUAGES

STR = r'"(?:[^"\\]|\\.)*"'
ROWS = re.compile(r'\[\s*' + STR + r'(?:\s*,\s*' + STR + r'){4,}\s*\]')
TOKENS = re.compile(r'\{[^{}]+\}|%[A-Z]')
# These are identifiers or search synonyms, not language columns.
IDENTIFIERS = {
    ('I', 'II', 'III', 'IV', 'V'),
    ('clan_team_commander', 'clan_team_soldier', 'clan_team_reserve', 'clan_team_invites', ''),
    ('friend_state_lobby', 'friend_state_game', 'friend_state_offline', 'friend_state_away', 'friend_invite'),
}


def audit_flash(root):
    root = Path(root)
    if not (root / 'lobby_menu/src/survarium/constants/Language.as').is_file():
        raise ValueError(f'{root}: missing Flash sources')
    issues, checked = [], 0

    constants = (root / 'lobby_menu/src/survarium/constants/Language.as').read_text(encoding='utf-8')
    actual = {name: int(index) for name, index in re.findall(
        r'public static const ([A-Z]+):int\s*=\s*(\d+);', constants)}
    expected = {code: index for index, (_, code) in enumerate(LANGUAGES)}
    if actual != expected:
        issues.append(f'Language.as: expected language indices {expected}, got {actual}')

    def check(row, location):
        nonlocal checked
        checked += 1
        if len(row) != len(LANGUAGES):
            issues.append(f'{location}: expected {len(LANGUAGES)} language columns, got {len(row)}')
            return
        # The shipped legacy translations have their own pre-existing defects.
        # Require every added language, including Serbian, to match English.
        for index in range(5, len(LANGUAGES)):
            language = LANGUAGES[index][0]
            en, translated = row[0], row[index]
            where = f'{location} ({language})'
            if isinstance(en, list):
                if not isinstance(translated, list) or len(en) != len(translated):
                    issues.append(f'{where}: replay substitution pieces differ')
                    continue
                en, translated = ''.join(en), ''.join(translated)
            if not isinstance(translated, str):
                issues.append(f'{where}: expected translated text')
                continue
            if en.strip() and not translated.strip():
                issues.append(f'{where}: empty translation')
            if re.search('[\u0400-\u04ff]', translated):
                issues.append(f'{where}: Cyrillic in Latin text')
            if sorted(TOKENS.findall(en)) != sorted(TOKENS.findall(translated)):
                issues.append(f'{where}: changed substitutions')

    for path in sorted(root.rglob('*.as')):
        source = path.read_text(encoding='utf-8')
        for match in ROWS.finditer(source):
            row = json.loads(match[0].replace("\\'", "'"))
            if path.name == 'GangShop.as' and re.search(r'disabled:Array\s*=\s*$', source[max(0, match.start()-30):match.start()]):
                continue  # five faction slots, not languages
            if tuple(row) in IDENTIFIERS or re.search(r'words\s*:\s*$', source[max(0, match.start()-20):match.start()]):
                continue
            # The established order starts with English, Ukrainian, Russian.
            # A numbered label can consist only of a substitution in those
            # languages (e.g. "Loadout {n}", "{n}", "{n}"). Include it too.
            # Other arrays are animation names, item identifiers, etc.
            if not (all(re.search('[\u0400-\u04ff]', s) for s in row[1:3])
                    or len(set(row)) == 1 or TOKENS.search(row[0])):
                continue
            check(row, f'{path.relative_to(root)}:{source.count(chr(10), 0, match.start()) + 1}')
        if path.name == 'NotificationListScrollContent.as':
            # Replay messages include arrays of fragments around interpolated values.
            start = source.index('protected function rpWords()')
            start = source.index('var words:Array = ', start) + len('var words:Array = ')
            rows, _ = json.JSONDecoder().raw_decode(source[start:])
            for index, row in enumerate(rows):
                check(row, f'replay message {index}')
    return {'checked': checked, 'issues': issues}
