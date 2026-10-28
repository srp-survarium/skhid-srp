#!/usr/bin/env python3
"""Compile all game dictionaries from the same checked-in TSV catalog.

Called by content_edit.py as the final edit in release/content.tsv.
"""
from check_localization_026g import audit_catalog, failed
from localization_026g import LANGUAGES, encode_strings, read_strings, read_translations


def compile_catalog(directory, sources):
    report = audit_catalog(directory)
    if failed(report):
        problems = {language: {field: value for field, value in result.items()
                              if field not in ('entries', 'unchanged') and value}
                    for language, result in report['languages'].items()}
        raise ValueError(f'invalid localization catalog: {problems}; '
                         f'missing languages: {report["missing_languages"]}; '
                         f'unregistered languages: {report["unexpected_languages"]}')
    dictionaries = {language: read_translations(directory / f'{language}.tsv')
                    for language, _ in LANGUAGES}
    # `original` is the engine's internal Russian alias, not another translation.
    dictionaries['original'] = dictionaries['russian']
    for language, source in sources.items():
        previous, _ = read_strings(bytes(source))
        missing = previous.keys() - dictionaries[language].keys()
        if missing:
            raise ValueError(f'{language}: source keys missing from TSV: {sorted(missing)}')
    return {language: encode_strings(strings) for language, strings in dictionaries.items()}
