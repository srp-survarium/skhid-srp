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


# Resource directory and Flash constant, in the movie tables' column order.
# Keep the existing column numbers stable when adding languages.
LANGUAGES = (
    ('english', 'EN'), ('ukrainian', 'UK'), ('russian', 'RU'),
    ('german', 'DE'), ('polish', 'PL'), ('serbian', 'SR'),
    ('czech', 'CS'), ('hungarian', 'HU'), ('finnish', 'FI'),
)
CATALOG = Path(__file__).resolve().parent / 'localization'
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
