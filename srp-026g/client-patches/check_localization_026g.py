#!/usr/bin/env python3
"""Audit extracted 0.26g dictionaries. Nonzero exit means incomplete/invalid data.

python check_localization_026g.py /path/to/dbgrab-output
python check_localization_026g.py /path/to/resources --require serbian --json
"""
import argparse
from collections import Counter
import json
from pathlib import Path
import re
import sys

from localization_026g import discover, read_config, read_strings
from check_flash_localization_026g import audit_flash


# Game substitutions and printf arguments must survive translation unchanged.
# A space after % is prose ("35% of health"), not a game format flag.
TOKENS = re.compile(r'\{[^{}]+\}|%%|%(?:\d+\$)?[-+#0]*(?:\d+|\*)?(?:\.(?:\d+|\*))?(?:ll|[hlL])?[diuoxXfFeEgGaAcsp]')
TAGS = re.compile(r'</?[A-Za-z_][^<>]*>')


def substitutions(text):
    tokens = TOKENS.findall(text)
    # Named substitutions may move. Positional printf arguments may not.
    ordered = [t for t in tokens if t.startswith('%') and t != '%%' and '$' not in t]
    return Counter(tokens), ordered


def compare(reference, translated):
    common = reference.keys() & translated.keys()
    return {
        'entries': len(translated),
        'missing': sorted(reference.keys() - translated.keys()),
        'extra': sorted(translated.keys() - reference.keys()),
        'empty': sorted(k for k in common if reference[k].strip() and not translated[k].strip()),
        'placeholders': sorted(k for k in common
                               if substitutions(reference[k]) != substitutions(translated[k])),
        'markup': sorted(k for k in common
                         if Counter(TAGS.findall(reference[k])) != Counter(TAGS.findall(translated[k]))),
        'unchanged': sorted(k for k in common if reference[k] and reference[k] == translated[k]),
    }


def audit(root, required=(), reference='english'):
    paths = discover(root)
    if reference not in paths:
        raise ValueError(f'missing reference dictionary: {reference}')
    dictionaries = {}
    encodings = {}
    for language, path in paths.items():
        dictionaries[language], encodings[language] = read_strings(path)
    baseline = dictionaries[reference]
    report = {'reference': reference, 'missing_languages': sorted(set(required) - paths.keys()),
              'languages': {}, 'ui_mapping_found': False, 'ui_missing': {}}
    for language, strings in dictionaries.items():
        result = compare(baseline, strings)
        result['invalid_utf8'] = encodings[language]
        report['languages'][language] = result
    mappings = [p for p in Path(root).rglob('*') if p.name in (
        'flash_movies_localizations.options', 'flash_movies_localizations.options.json',
        'localization__flash_movies_localizations.options', 'localization__flash_movies_localizations.options.json')]
    if len(mappings) > 1:
        raise ValueError('ambiguous UI mapping files')
    if mappings:
        mapping, errors = read_config(mappings[0])
        if errors:
            raise ValueError('UI mapping contains invalid UTF-8')
        keys = set()
        for rows in mapping.values():
            for row in rows:
                if not isinstance(row, list) or len(row) != 2 or not all(isinstance(v, str) for v in row):
                    raise ValueError('malformed UI mapping row')
                keys.add(row[1])
        report['ui_mapping_found'] = True
        report['ui_missing'] = {lang: sorted(keys - strings.keys()) for lang, strings in dictionaries.items()}
    return report


def failed(report):
    return bool(report['missing_languages'] or any(
        result[field] for result in report['languages'].values()
        for field in ('missing', 'empty', 'placeholders', 'markup', 'invalid_utf8'))
        or any(report['ui_missing'].values()) or report.get('flash', {}).get('issues'))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', type=Path)
    parser.add_argument('--reference', default='english')
    parser.add_argument('--require', action='append', default=[], metavar='LANGUAGE')
    parser.add_argument('--json', action='store_true', help='machine-readable report including every key')
    parser.add_argument('--flash-source', type=Path, default=Path(__file__).resolve().parents[1] / 'flash',
                        help='Flash source tree to check (default: this checkout)')
    args = parser.parse_args(argv)
    try:
        report = audit(args.resources, args.require, args.reference)
        report['flash'] = audit_flash(args.flash_source)
    except (OSError, ValueError) as error:
        print(f'ERROR: {error}', file=sys.stderr)
        return 2
    if args.json:
        print(json.dumps(report, ensure_ascii=True, indent=2))
    else:
        for language, result in report['languages'].items():
            print(f"{language}: {result['entries']} entries; " + '; '.join(
                f'{len(result[field])} {field}' for field in
                ('missing', 'extra', 'empty', 'placeholders', 'markup', 'invalid_utf8', 'unchanged')))
            for field in ('missing', 'extra', 'empty', 'placeholders', 'markup', 'invalid_utf8'):
                for key in result[field]:
                    print(f'  {field}: {key}')
            for key in report['ui_missing'].get(language, []):
                print(f'  UI mapping references missing key: {key}')
        for language in report['missing_languages']:
            print(f'MISSING LANGUAGE: {language}')
        if not report['ui_mapping_found']:
            print('UI mapping not supplied; UI-reference coverage was not checked.')
        print(f"Flash: {report['flash']['checked']} language rows checked")
        for issue in report['flash']['issues']:
            print(f'  {issue}')
        print('FAIL' if failed(report) else 'PASS')
    return int(failed(report))


if __name__ == '__main__':
    sys.exit(main())
