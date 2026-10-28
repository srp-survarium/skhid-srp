"""Localization checks use synthetic dictionaries, never shipped game assets."""
import contextlib
import io
import json
import re
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

import check_localization_026g as checker
from localization_026g import LANGUAGES, TRANSLATED_LANGUAGES, discover, encode_strings, read_strings, read_translations
from build_localization_026g import translate
from check_flash_localization_026g import audit_flash


class LocalizationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def dictionary(self, language, strings):
        path = self.root / 'localization' / language / 'localization'
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(encode_strings(strings))
        return path

    def test_serbian_roundtrip_preserves_every_latin_letter_and_tokens(self):
        strings = {'st_test': 'ČĆŠŽĐ čćšžđ: {USE} %d <white>Pokreni</white>',
                   'st_blank': '', 'st_multiline': 'Prvi red\nDrugi red'}
        path = self.dictionary('serbian', strings)
        self.assertEqual(read_strings(path), (strings, []))

    def test_missing_languages_and_keys_fail_even_when_counts_match(self):
        self.dictionary('english', {'a': 'One', 'b': 'Two'})
        self.dictionary('serbian', {'a': 'Jedan', 'c': 'Tri'})
        report = checker.audit(self.root, ['serbian', 'polish'])
        self.assertEqual(report['missing_languages'], ['polish'])
        self.assertEqual(report['languages']['serbian']['missing'], ['b'])
        self.assertEqual(report['languages']['serbian']['extra'], ['c'])
        self.assertTrue(checker.failed(report))

    def test_named_substitutions_can_move_but_printf_arguments_cannot(self):
        report = checker.compare({'a': '{USE} %d %s', 'b': 'Text', 'c': ''},
                                 {'a': '%d {USE} %s', 'b': ' ', 'c': ''})
        self.assertEqual(report['placeholders'], [])
        self.assertEqual(report['empty'], ['b'])
        self.assertEqual(checker.compare({'a': '{USE} %d'}, {'a': '{USE}'})['placeholders'], ['a'])
        self.assertEqual(checker.compare({'a': '%d %s'}, {'a': '%s %d'})['placeholders'], ['a'])
        self.assertEqual(checker.compare({'a': '{USE} {USE}'}, {'a': '{USE}'})['placeholders'], ['a'])
        self.assertEqual(checker.compare({'a': '35% of health'}, {'a': '35% zdravlja'})['placeholders'], [])
        self.assertEqual(checker.compare({'a': '%%s'}, {'a': '%%d'})['placeholders'], [])

    def test_markup_changes_fail(self):
        self.assertEqual(checker.compare({'a': '<green>Level</green>'},
                                        {'a': '<green>Nivo<green/>'})['markup'], ['a'])

    def test_compiler_requires_explicit_translation_or_keep_and_preserves_utf8(self):
        table, keep = self.root / 'sr.tsv', self.root / 'keep.txt'
        table.write_text('message\tČekaj {USE} %d\\nJoš\n', encoding='utf-8')
        keep.write_text('st_model\n', encoding='utf-8')
        source = encode_strings({'st_message': 'Wait {USE} %d\nMore', 'st_model': 'AKM'})
        result, errors = read_strings(translate(source, table, keep, latin=True))
        self.assertFalse(errors)
        self.assertEqual(result, {'st_message': 'Čekaj {USE} %d\nJoš', 'st_model': 'AKM'})
        keep.write_text('')
        with self.assertRaisesRegex(ValueError, 'missing'):
            translate(source, table, keep)
        table.write_text('message\tČekaj {USE} %d\nmodel\tAKM\nextra\tVišak\n')
        with self.assertRaisesRegex(ValueError, 'unknown'):
            translate(source, table, keep)
        self.assertIn('st_extra', read_strings(translate(source, table, keep, ['st_extra']))[0])

    def test_compiler_rejects_duplicate_keys_cyrillic_and_oversized_text(self):
        table, keep = self.root / 'sr.tsv', self.root / 'keep.txt'
        keep.write_text('')
        source = encode_strings({'st_a': 'Hello'})
        for content, error in [('a\tZdravo\na\tZdravo\n', 'duplicate'),
                               ('a\tЗдраво\n', 'non_latin'), ('a\t' + 'č' * 256, 'too_long')]:
            table.write_text(content, encoding='utf-8')
            with self.assertRaisesRegex(ValueError, error):
                translate(source, table, keep, latin=True)

    def test_ui_mapping_detects_keys_missing_even_from_reference(self):
        self.dictionary('english', {'a': 'One'})
        self.dictionary('serbian', {'a': 'Jedan'})
        (self.root / 'flash_movies_localizations.options.json').write_text(
            json.dumps({'hud_strings': [['caption', 'unlisted_key']]}))
        report = checker.audit(self.root)
        self.assertEqual(report['ui_missing']['serbian'], ['unlisted_key'])
        self.assertTrue(checker.failed(report))

    def test_invalid_utf8_is_reported_without_silently_reencoding(self):
        path = self.dictionary('english', {'a': 'valid'})
        data = path.read_bytes().replace(b'valid\0', b'\xe0alid\0')
        path.write_bytes(data)
        self.assertEqual(read_strings(path)[1], ['a'])
        self.assertTrue(checker.failed(checker.audit(self.root)))

    def test_duplicates_and_malformed_tables_are_errors(self):
        path = self.root / 'duplicate.json'
        path.write_text('{"strings":{"a":"one","a":"two"}}')
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            read_strings(path)
        path = self.dictionary('english', {'a': 'text'})
        data = bytearray(path.read_bytes())
        struct.pack_into('<Q', data, 0, len(data) + 24)
        path.write_bytes(data)
        with self.assertRaisesRegex(ValueError, 'outside'):
            read_strings(path)

    def test_flat_and_tree_inputs_cannot_silently_shadow_each_other(self):
        path = self.dictionary('english', {'a': 'text'})
        (self.root / 'localization__english__localization').write_bytes(path.read_bytes())
        with self.assertRaisesRegex(ValueError, 'ambiguous'):
            discover(self.root)

    def test_cli_exit_codes(self):
        self.dictionary('english', {'a': 'text'})
        self.dictionary('serbian', {'a': 'tekst'})
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(checker.main([str(self.root), '--require', 'serbian']), 0)
            self.assertEqual(checker.main([str(self.root), '--require', 'german']), 1)
            self.assertEqual(checker.main([str(self.root / 'absent')]), 2)

    def test_checked_in_flash_tables_include_every_registered_language(self):
        result = audit_flash(Path(__file__).resolve().parents[1] / 'flash')
        self.assertGreater(result['checked'], 200)
        self.assertEqual(result['issues'], [])

    def flash_fixture(self, row):
        folder = self.root / 'lobby_menu/src/survarium/constants'
        folder.mkdir(parents=True, exist_ok=True)
        actual = Path(__file__).resolve().parents[1] / 'flash/lobby_menu/src/survarium/constants/Language.as'
        (folder / 'Language.as').write_text(actual.read_text(encoding='utf-8'), encoding='utf-8')
        (self.root / 'Words.as').write_text('var words:Array = ' + json.dumps(row) + ';', encoding='utf-8')

    def test_flash_check_catches_omitted_columns_and_bad_text_in_each_added_language(self):
        row = ['Play {n}', 'Грати {n}', 'Играть {n}', 'Spielen {n}', 'Graj {n}',
               'Igraj {n}', 'Hrát {n}', 'Játék {n}', 'Pelaa {n}']
        self.flash_fixture(row[:-1])
        self.assertTrue(any('expected 9 language columns' in issue
                            for issue in audit_flash(self.root)['issues']))
        for index in range(5, len(LANGUAGES)):
            for replacement, error in [('', 'empty'), ('Play', 'substitutions'),
                                        ('Играть {n}', 'Cyrillic')]:
                with self.subTest(language=LANGUAGES[index][0], error=error):
                    changed = row.copy()
                    changed[index] = replacement
                    self.flash_fixture(changed)
                    issues = audit_flash(self.root)['issues']
                    self.assertTrue(any(error in issue and LANGUAGES[index][0] in issue for issue in issues))

    def test_flash_constant_indices_cannot_drift_from_registry(self):
        self.flash_fixture(['Same'] * len(LANGUAGES))
        path = self.root / 'lobby_menu/src/survarium/constants/Language.as'
        path.write_text(path.read_text().replace('FI:int = 8', 'FI:int = 9'))
        self.assertTrue(any('expected language indices' in issue for issue in audit_flash(self.root)['issues']))

    def test_added_dictionary_labels_match_flash_detectors(self):
        here = Path(__file__).resolve().parent
        flash = here.parent / 'flash'
        def table(path, name):
            text = (flash / path).read_text(encoding='utf-8')
            return json.loads(re.search(name + r':Array\s*=\s*(\[.*?\]);', text)[1])
        friends = table('lobby_menu/src/survarium/rooms/RoomsText.as', 'FRIENDS')
        cancel = table('lobby_menu/src/survarium/messages/MessageMoneyExchange.as', 'CANCEL')
        login = (flash / 'login_menu/src/SrpLoginHint.as').read_text(encoding='utf-8')
        for index, (language, code) in enumerate(LANGUAGES[5:], 5):
            with self.subTest(language=language):
                translated = read_translations(here / f'localization/{language}.tsv')
                self.assertEqual(translated['st_btn_friends'], friends[index])
                self.assertEqual(translated['st_cancel'], cancel[index])
                self.assertIn(json.dumps(translated['st_registration_str'], ensure_ascii=False) + ':', login)
                for path in ('gsearch/MatchMakingSettings.as', 'controls/NotificationListScrollContent.as'):
                    source = (flash / 'lobby_menu/src/survarium' / path).read_text(encoding='utf-8')
                    self.assertRegex(source, r'if\(word == "' + re.escape(friends[index]) +
                                     r'"\)\s*\{\s*return Language\.' + code + r';')

    def test_all_added_dictionaries_cover_the_same_keys_and_formats(self):
        here = Path(__file__).resolve().parent / 'localization'
        baseline = read_translations(here / 'serbian.tsv')
        for language in TRANSLATED_LANGUAGES:
            with self.subTest(language=language):
                translated = read_translations(here / f'{language}.tsv')
                report = checker.compare(baseline, translated)
                for field in ('missing', 'extra', 'empty', 'placeholders', 'markup'):
                    self.assertEqual(report[field], [], field)
                keep = (here / f'{language}_keep.txt').read_text(encoding='utf-8').splitlines()
                self.assertFalse(set(keep) & translated.keys())
                self.assertEqual(keep, (here / 'serbian_keep.txt').read_text(encoding='utf-8').splitlines())
                encoded = encode_strings(translated)
                self.assertEqual(read_strings(encoded), (translated, []))

    def test_new_language_alphabets_survive_native_roundtrip(self):
        text = 'ÁČĎÉĚÍŇÓŘŠŤÚŮÝŽ áčďéěíňóřšťúůýž ÁÉÍÓÖŐÚÜŰ áéíóöőúüű ÅÄÖ åäö'
        self.assertEqual(read_strings(encode_strings({'st_letters': text})), ({'st_letters': text}, []))

    def test_content_edit_compiles_dictionary_and_copies_support_files_exactly(self):
        source = self.dictionary('english', {'st_test': 'Hello %d'})
        support = b'opaque support data\xff\x00'
        (source.parent / 'eula').write_bytes(support)
        (self.root / 'sr.tsv').write_text('test\tZdravo %d\n', encoding='utf-8')
        (self.root / 'keep.txt').write_text('')
        edit = self.root / 'edit.json'
        edit.write_text(json.dumps({
            'copy': [{'files': 'localization/english/*', 'from': 'localization/english/',
                      'to': 'localization/serbian/'}],
            'translate': [{'file': 'localization/english/localization',
                           'target': 'localization/serbian/localization', 'translations': 'sr.tsv',
                           'keep': 'keep.txt', 'latin': True}]}))
        script = Path(__file__).resolve().parents[1] / 'scripts/content-edit/content_edit.py'
        output = self.root / 'result'
        run = subprocess.run([sys.executable, '-I', '-S', str(script), str(edit), str(self.root), str(output)],
                             capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
        self.assertEqual(read_strings(output / 'localization/serbian/localization')[0], {'st_test': 'Zdravo %d'})
        self.assertEqual((output / 'localization/serbian/eula').read_bytes(), support)
        self.assertIn('0 unexpected differences', run.stdout)


if __name__ == '__main__':
    unittest.main()
