#!/usr/bin/env python3
"""Check every checked-in TSV and the Flash menus' text keys with no game assets needed.

python check_localization_026g.py
python check_localization_026g.py /path/to/dbgrab-output
An optional resource directory additionally checks the compiled dictionaries and UI mapping.
"""
import argparse
from collections import Counter
import json
from pathlib import Path
import re
import sys

from localization_026g import (CATALOG, CYRILLIC, EMPTY_KEYS, FLASH_MOVIES, FLASH_PREFIX, FOREIGN_LETTERS,
                              LANGUAGES, discover, encode_strings, read_config, read_strings,
                              read_translations, rust_table_issue)
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


def audit_catalog(root=CATALOG):
    root = Path(root)
    expected = {name for name, *_ in LANGUAGES}
    paths = {p.stem: p for p in sorted(root.glob('*.tsv'))}
    dictionaries = {name: read_translations(path) for name, path in paths.items()}
    if 'english' not in dictionaries:
        raise ValueError(f'{root}: missing English TSV')
    report = {'reference': 'english', 'missing_languages': sorted(expected - paths.keys()),
              'unexpected_languages': sorted(paths.keys() - expected), 'languages': {},
              'ui_mapping_found': False, 'ui_missing': {}}
    for language, strings in dictionaries.items():
        result = compare(dictionaries['english'], strings)
        result['empty'] = sorted(k for k, v in strings.items() if not v.strip() and k not in EMPTY_KEYS)
        # The game's localizer (0x1FEF90) makes {NAME} an action's key and drops any other
        # {...}: srp's own texts take %1$s placeholders.
        result['braces'] = sorted(k for k, v in strings.items()
                                  if k.startswith(FLASH_PREFIX) and re.search('[{}]', v))
        result['invalid_utf8'] = []  # Strict UTF-8 decoding happens in read_translations.
        result['key_order'] = [] if list(strings) == sorted(strings) else ['keys must be sorted']
        result['too_long'] = sorted(k for k, v in strings.items() if len(v.encode('utf-8')) >= 512)
        result['non_latin'] = sorted(k for k, v in strings.items()
            if language not in CYRILLIC and re.search('[\u0400-\u04ff]', v))
        # Old Latin dictionaries contain unfinished Russian descriptions; preserve
        # those existing texts. Added Latin languages must remain fully Latin.
        if language in ('english', 'german', 'polish'):
            result['non_latin'] = []
        # Russian or Ukrainian letters left in a translation from those dictionaries.
        foreign = FOREIGN_LETTERS.get(language)
        result['foreign_letters'] = sorted(k for k, v in strings.items()
                                           if foreign and re.search('[' + foreign + ']', v))
        if read_strings(encode_strings(strings)) != (strings, []):
            raise ValueError(f'{language}: compiled dictionary did not read back exactly')
        report['languages'][language] = result
    return report


def audit_content_sources(repo, keys):
    """Prevent new edit lists from becoming another dictionary source."""
    issues = []
    rows = [line.split('\t') for line in (repo / 'release/content.tsv').read_text().splitlines()
            if line and not line.startswith('#')]
    edits = [(row[1], json.loads((repo / 'srp-026g/scripts/content-edit' / row[1]).read_text()))
             for row in rows if row[0] == 'edit' and row[2] == '-' and row[1].endswith('.json')]
    compilers = [name for name, edit in edits if 'localization' in edit]
    if len(compilers) != 1 or rows[-1][:3] != ['edit', compilers[0], '-']:
        issues.append('content.tsv must end with exactly one localization catalog edit')

    def references(value):
        if isinstance(value, str) and re.fullmatch(r'st_[A-Za-z0-9_]+', value):
            yield value
        elif isinstance(value, list):
            for item in value:
                yield from references(item)
        elif isinstance(value, dict):
            for key, item in value.items():
                if key not in ('from', '_about'):
                    yield from references(item)

    for name, edit in edits:
        for key in sorted(set(references(edit)) - keys):
            issues.append(f'{name}: references missing dictionary key {key}')
        for entries in edit.values():
            if not isinstance(entries, list):
                continue
            for entry in entries:
                path = entry.get('file', entry.get('files', ''))
                if isinstance(path, str) and re.fullmatch(r'localization/[^/]+/localization', path):
                    issues.append(f'{name}: edit dictionary text in localization/*.tsv')
    return issues


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
        # srp's own texts: each mapped to itself for the movies that read them.
        own = {k for k in baseline if k.startswith(FLASH_PREFIX)}
        report['ui_unmapped'] = {f'{movie}_strings': sorted(
            k for k in own if k.startswith(prefix) and [k, k] not in mapping.get(f'{movie}_strings', []))
            for movie, prefix in FLASH_MOVIES.items()}
    return report


def failed(report):
    return bool(report['missing_languages'] or report.get('unexpected_languages') or report.get('source_issues')
                or report.get('rust_table') or any(
        result.get(field) for result in report['languages'].values()
        for field in ('missing', 'extra', 'empty', 'placeholders', 'markup', 'invalid_utf8',
                      'too_long', 'non_latin', 'foreign_letters', 'braces', 'key_order', 'catalog_mismatch'))
        or any(report['ui_missing'].values()) or any(report.get('ui_unmapped', {}).values())
        or report.get('flash', {}).get('issues'))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?', type=Path)
    parser.add_argument('--catalog', type=Path, default=CATALOG)
    parser.add_argument('--reference', default='english')
    parser.add_argument('--require', action='append', default=[], metavar='LANGUAGE')
    parser.add_argument('--json', action='store_true', help='machine-readable report including every key')
    parser.add_argument('--flash-source', type=Path,
                        help="Flash source tree whose text keys to check against the catalog "
                             "(default: this checkout's, for this checkout's catalog only)")
    args = parser.parse_args(argv)
    try:
        report = audit_catalog(args.catalog)
        report['rust_table'] = rust_table_issue()
        flash = args.flash_source
        if flash is None and args.catalog.resolve() == CATALOG:
            flash = Path(__file__).resolve().parents[1] / 'flash'
        report['flash'] = (audit_flash(flash, read_translations(args.catalog / 'english.tsv').keys())
                           if flash else {'checked': 0, 'issues': []})
        if args.catalog.resolve() == CATALOG:
            report['source_issues'] = audit_content_sources(Path(__file__).resolve().parents[2],
                                        read_translations(CATALOG / 'english.tsv').keys())
        if args.resources:
            report['resources'] = audit(args.resources, args.require or
                                        [name for name, *_ in LANGUAGES], args.reference)
            for language, path in discover(args.resources).items():
                table = args.catalog / f'{"russian" if language == "original" else language}.tsv'
                if table.exists():
                    expected = read_translations(table)
                    actual, _ = read_strings(path)
                    report['resources']['languages'][language]['catalog_mismatch'] = sorted(
                        k for k in expected.keys() | actual.keys() if expected.get(k) != actual.get(k))
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
            for field in ('missing', 'extra', 'empty', 'placeholders', 'markup', 'invalid_utf8',
                          'too_long', 'non_latin', 'foreign_letters', 'braces', 'key_order'):
                for key in result[field]:
                    print(f'  {field}: {key}')
            for key in report['ui_missing'].get(language, []):
                print(f'  UI mapping references missing key: {key}')
        for language in report['missing_languages']:
            print(f'MISSING LANGUAGE: {language}')
        for language in report['unexpected_languages']:
            print(f'UNREGISTERED LANGUAGE: {language}')
        for issue in report.get('source_issues', []):
            print(f'CONTENT: {issue}')
        if report['rust_table']:
            print(f"EXE TABLE: {report['rust_table']}")
        if 'resources' in report:
            print('Compiled resources: ' + ('FAIL' if failed(report['resources']) else 'PASS'))
            if failed(report['resources']):
                print(json.dumps(report['resources'], ensure_ascii=True, indent=2))
            if not report['resources']['ui_mapping_found']:
                print('UI mapping not supplied; UI-reference coverage was not checked.')
        print(f"Flash: {report['flash']['checked']} text keys of the movies checked")
        for issue in report['flash']['issues']:
            print(f'  {issue}')
        print('FAIL' if failed(report) or failed(report.get('resources', report)) else 'PASS')
    return int(failed(report) or failed(report.get('resources', report)))


if __name__ == '__main__':
    sys.exit(main())
