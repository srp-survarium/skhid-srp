#!/usr/bin/env python3
"""Read/write the game's string dictionaries without the legacy CP1251 decoder.

Accepts extracted resource trees, dbgrab's flat filenames, and UTF-8 JSON.
Game resources are inputs supplied by the operator, never repository fixtures.
"""
import json
import re
from pathlib import Path
import struct
import zlib


# Resource directory, the language's own name (the Settings' Language list,
# exe patch 79) and its Windows primary language id (a LANGID's low ten bits:
# a new player starts in the language of his Windows user locale, patch 79),
# in the list's order. After a new row, `python3 localization_026g.py --write`
# regenerates RUST_TABLE.
LANGUAGES = (
    ('english', 'English', 0x09), ('ukrainian', 'Українська', 0x22), ('russian', 'Русский', 0x19),
    ('german', 'Deutsch', 0x07), ('polish', 'Polski', 0x15),
    # 0x1A is Serbian, Croatian and Bosnian, every script: the Latin Serbian is the nearest we have.
    ('serbian', 'Srpski', 0x1A), ('czech', 'Čeština', 0x05), ('hungarian', 'Magyar', 0x0E),
    ('finnish', 'Suomi', 0x0B), ('belarusian', 'Беларуская', 0x23),
)
# Languages written in Cyrillic; every other dictionary is Latin.
CYRILLIC = frozenset({'ukrainian', 'russian', 'belarusian'})
# Letters of the neighbouring Cyrillic alphabets that standard Belarusian does not use.
FOREIGN_LETTERS = {'belarusian': 'иИщЩъЪїЇєЄґҐ'}
CATALOG = Path(__file__).resolve().parent / 'localization'
# srp's own texts (keys st_srp_...) reach these movies under their own keys: the
# content compile maps each such key to itself in the movie's table of
# localization/flash_movies_localizations.options (<movie>_strings), which the exe
# hands the movie through set_localization_data. The movies call
# LocalizationData.getName("st_srp_...") / LocalizationData.format(...).
FLASH_PREFIX = 'st_srp_'
FLASH_MOVIES = ('lobby_menu', 'login_menu')


def flash_rows(keys):
    """The UI mapping rows of srp's own texts: each key as its own UI key."""
    return [[key, key] for key in sorted(keys) if key.startswith(FLASH_PREFIX)]
# These unused directional controls intentionally have no visible label.
EMPTY_KEYS = frozenset('st_turn_' + direction + '_action'
                       for direction in ('down', 'left', 'up', 'right'))


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate key: {key}")
        result[key] = value
    return result


def read_config(path):
    if isinstance(path, bytes):
        data, path = path, '<binary config>'
    else:
        path = Path(path)
        if path.suffix == '.json':
            return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=unique_object), []
        data = path.read_bytes()
    encoding_errors = []

    def string(offset, label):
        if not 0 <= offset < len(data):
            raise ValueError(f'{path}: string offset outside file: {offset}')
        end = data.find(b'\0', offset)
        if end < 0:
            raise ValueError(f'{path}: unterminated string at {offset}')
        raw = data[offset:end]
        try:
            return raw.decode('utf-8')
        except UnicodeDecodeError:
            # Keep the defect visible. Do not silently reinterpret UTF-8 as CP1251.
            encoding_errors.append(label)
            return raw.decode('utf-8', errors='surrogateescape')

    def value(offset, ancestors=(), label='root'):
        if offset in ancestors or len(ancestors) >= 64:
            raise ValueError(f'{path}: cyclic or excessively nested config')
        if offset < 0 or offset + 24 > len(data):
            raise ValueError(f'{path}: truncated value at {offset}')
        pointer, _, _, kind, count = struct.unpack_from('<QQIHH', data, offset)
        if kind == 5:
            text = string(pointer, label)
            if len(text.encode('utf-8', errors='surrogateescape')) + 1 != count:
                raise ValueError(f'{path}: wrong string byte count for {label}')
            return text
        if kind not in (3, 4):
            raise ValueError(f'{path}: unsupported localization value type {kind}')
        if pointer + 24 * count > len(data):
            raise ValueError(f'{path}: table outside file')
        children = []
        for index in range(count):
            child = pointer + 24 * index
            name_offset = struct.unpack_from('<Q', data, child + 8)[0]
            key = string(name_offset, label + '.key') if kind == 3 else index
            item = value(child, (*ancestors, offset), str(key))
            children.append((key, item))
        return unique_object(children) if kind == 3 else [v for _, v in children]

    return value(0), encoding_errors


def read_strings(path):
    root, errors = read_config(path)
    strings = root.get('strings') if isinstance(root, dict) else None
    if not isinstance(strings, dict) or not all(
            isinstance(k, str) and isinstance(v, str) for k, v in strings.items()):
        raise ValueError(f'{path}: expected an object containing a strings dictionary')
    return strings, errors


def read_translations(path):
    """Read a two-column UTF-8 TSV; escapes keep each key on one physical line."""
    pairs = []
    lines = Path(path).read_text(encoding='utf-8').splitlines()
    if not lines or lines[0] != 'key\ttext':
        raise ValueError(f'{path}: expected key<TAB>text header')
    for number, line in enumerate(lines[1:], 2):
        if line.count('\t') != 1:
            raise ValueError(f'{path}:{number}: expected key, TAB, translation')
        key, text = line.split('\t')
        if not re.fullmatch(r'[A-Za-z0-9_]+', key):
            raise ValueError(f'{path}:{number}: invalid full dictionary key: {key!r}')
        def unescape(match):
            escapes = {'\\': '\\', 'n': '\n', 'r': '\r', 't': '\t'}
            if match[1] not in escapes:
                raise ValueError(f'{path}:{number}: invalid escape: {match[0]!r}')
            return escapes[match[1]]
        text = re.sub(r'\\(.)|\\$', unescape, text)
        if '\0' in text:
            raise ValueError(f'{path}:{number}: NUL in text')
        pairs.append((key, text))
    return unique_object(pairs)


def encode_strings(strings):
    """Serialize a named strings table, sorted by the engine's CRC32 key order."""
    if not all(isinstance(k, str) and isinstance(v, str) and '\0' not in k + v
               for k, v in strings.items()):
        raise ValueError('dictionary keys and values must be NUL-free strings')
    entries = sorted(strings.items(), key=lambda pair: zlib.crc32(pair[0].encode('utf-8')))
    if len(entries) > 65535:
        raise ValueError('too many dictionary entries')
    data = bytearray(24 * (2 + len(entries)))

    def append(text):
        raw = text.encode('utf-8') + b'\0'
        if len(raw) > 65535:
            raise ValueError('string too long for binary config')
        offset = len(data)
        data.extend(raw)
        return offset, len(raw)

    root_name, _ = append('strings')
    struct.pack_into('<QQIHH', data, 0, 24, 0, 0, 3, 1)
    struct.pack_into('<QQIHH', data, 24, 48, root_name, zlib.crc32(b'strings'), 3, len(entries))
    for i, (key, text) in enumerate(entries):
        name, _ = append(key)
        pointer, count = append(text)
        struct.pack_into('<QQIHH', data, 48 + 24 * i, pointer, name,
                         zlib.crc32(key.encode('utf-8')), 5, count)
    return bytes(data)


def discover(root):
    """Find dictionaries in either a resource tree or dbgrab output directory."""
    root = Path(root)
    found = {}
    for path in sorted(root.rglob('*')):
        if not path.is_file():
            continue
        parts = path.name.removesuffix('.json').split('__')
        if len(parts) == 3 and parts[0] == 'localization' and parts[2] == 'localization':
            language = parts[1]
        elif path.name in ('localization', 'localization.json') and path.parent.parent.name == 'localization':
            language = path.parent.name
        else:
            continue
        if language in found:
            raise ValueError(f'ambiguous dictionaries for {language}: {found[language]}, {path}')
        found[language] = path
    return found


# The registry as the exe's Language patch (79, rust/patches/src/language.rs)
# reads it: generated, never edited. `--write` writes it, `--check` (and
# check_localization_026g.py) says whether it is current.
RUST_TABLE = Path(__file__).resolve().parent / 'languages_026g.rs'


def rust_table():
    """The Rust source of RUST_TABLE: every registered language's resource name,
    own name and Windows primary language id, in the registry's order."""
    for name, native, windows in LANGUAGES:
        if not re.fullmatch(r'[a-z]{1,15}', name) or not native or '"' in native or '\\' in native:
            raise ValueError(f'{name}: a resource name is 1..15 lower-case letters (the engine keeps '
                             f'it in 16 bytes) and an own name has no quote or backslash')
        if not 0 < windows < 0x400:
            raise ValueError(f"{name}: a primary language id is 1..3FFh (a LANGID's low ten bits)")
    primaries = [windows for *_, windows in LANGUAGES]
    if len(set(primaries)) != len(primaries):
        raise ValueError('two languages have one Windows primary language id')
    rows = ''.join(f'    Language {{ name: c"{name}", native: c"{native}", windows: PrimaryLanguage({windows:#04x}) }},\n'
                   for name, native, windows in LANGUAGES)
    return ('// Generated by localization_026g.py from its LANGUAGES: do not edit.\n'
            '// The game languages, by resource name (-localization=, resources/localization/<name>),\n'
            '// their own name, as the Settings list shows them, and the Windows primary language\n'
            '// whose players start in them.\n'
            f'pub const LANGUAGES: [Language; {len(LANGUAGES)}] = [\n{rows}];\n')


def rust_table_issue():
    """None when RUST_TABLE is current, else what is wrong."""
    try:
        current = RUST_TABLE.read_text(encoding='utf-8')
    except OSError:
        current = None
    if current != rust_table():
        return f'{RUST_TABLE.name} is not the registry: run python3 localization_026g.py --write'
    return None


if __name__ == '__main__':
    import sys
    if sys.argv[1:] == ['--write']:
        RUST_TABLE.write_text(rust_table(), encoding='utf-8')
        print(f'wrote {RUST_TABLE}')
    elif sys.argv[1:] == ['--check']:
        issue = rust_table_issue()
        print(issue or f'{RUST_TABLE.name}: current')
        sys.exit(1 if issue else 0)
    else:
        sys.exit('usage: localization_026g.py --write | --check')
