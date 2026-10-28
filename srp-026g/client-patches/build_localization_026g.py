#!/usr/bin/env python3
"""Build a complete translated dictionary. No implicit English fallback.

Normally called by content_edit.py through release/content.tsv.
"""
from pathlib import Path

from check_localization_026g import compare
from localization_026g import encode_strings, read_strings, read_translations


def translate(source, table, keep_file, extra=(), latin=False):
    reference, encoding_errors = read_strings(bytes(source))
    translated = read_translations(table)
    keep = [line for line in Path(keep_file).read_text(encoding='utf-8').splitlines()
            if line and not line.startswith('#')]
    if len(keep) != len(set(keep)):
        raise ValueError('duplicate intentionally unchanged key')
    if set(keep) & translated.keys():
        raise ValueError('keys cannot be both translated and intentionally unchanged')
    unknown = set(keep) - reference.keys()
    if unknown:
        raise ValueError(f'unknown intentionally unchanged keys: {sorted(unknown)}')
    if set(encoding_errors) & set(keep):
        raise ValueError('cannot copy malformed source text')
    translated.update({key: reference[key] for key in keep})
    report = compare(reference, translated)
    problems = {k: report[k] for k in ('missing', 'empty', 'placeholders', 'markup') if report[k]}
    unknown = set(report['extra']) - set(extra)
    if unknown:
        problems['unknown'] = sorted(unknown)
    if set(extra) - translated.keys():
        problems['missing_extra'] = sorted(set(extra) - translated.keys())
    # Native localization formatting uses a 512-byte buffer, including NUL.
    too_long = [key for key, text in translated.items() if len(text.encode('utf-8')) >= 512]
    if too_long:
        problems['too_long'] = sorted(too_long)
    if latin:
        bad = [k for k, v in translated.items() if any('\u0400' <= c <= '\u04ff' for c in v)]
        if bad:
            problems['non_latin'] = sorted(bad)
    if problems:
        raise ValueError(f'incomplete/invalid translation: {problems}')
    encoded = encode_strings(translated)
    actual, errors = read_strings(encoded)
    if errors or actual != translated:
        raise ValueError('compiled translation did not read back exactly')
    return encoded
