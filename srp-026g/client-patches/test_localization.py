"""Localization checks use synthetic dictionaries, never shipped game assets."""
import contextlib
import io
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

import check_localization_026g as checker
from localization_026g import discover, encode_strings, read_strings, read_translations
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

    def test_checked_in_flash_tables_include_serbian(self):
        result = audit_flash(Path(__file__).resolve().parents[1] / 'flash')
        self.assertGreater(result['checked'], 200)
        self.assertEqual(result['issues'], [])

    def test_flash_check_catches_omitted_sixth_column(self):
        folder = self.root / 'lobby_menu/src/survarium/constants'
        folder.mkdir(parents=True)
        (folder / 'Language.as').write_text('')
        (self.root / 'Words.as').write_text(
            'var words:Array = ["Play", "Грати", "Играть", "Spielen", "Graj"];', encoding='utf-8')
        result = audit_flash(self.root)
        self.assertEqual(result['checked'], 1)
        self.assertIn('expected six language columns', result['issues'][0])

    def test_serbian_labels_match_flash_language_detection(self):
        here = Path(__file__).resolve().parent
        translated = read_translations(here / 'localization/serbian.tsv')
        self.assertEqual(translated['st_btn_friends'], 'Prijatelji')
        self.assertEqual(translated['st_cancel'], 'Otkaži')
        self.assertEqual(translated['st_registration_str'], 'Napravi nalog')

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
