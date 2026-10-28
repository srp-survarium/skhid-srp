"""Localization checks use synthetic dictionaries, never shipped game assets."""
import contextlib
import io
import importlib.util
import json
import re
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

import check_localization_026g as checker
from localization_026g import LANGUAGES, discover, encode_strings, read_strings, read_translations
from build_localization_026g import compile_catalog
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

    def catalog(self, strings=None):
        strings = strings or {'st_test': 'Hello %d'}
        for language, _ in LANGUAGES:
            (self.root / f'{language}.tsv').write_text('key\ttext\n' + ''.join(
                key + '\t' + text + '\n' for key, text in sorted(strings.items())), encoding='utf-8')

    def test_all_languages_are_required_and_every_key_must_be_explicit(self):
        self.catalog()
        (self.root / 'polish.tsv').unlink()
        self.assertEqual(checker.audit_catalog(self.root)['missing_languages'], ['polish'])
        self.catalog()
        (self.root / 'polish.tsv').write_text('key\ttext\nst_other\tOther\n')
        result = checker.audit_catalog(self.root)['languages']['polish']
        self.assertEqual(result['missing'], ['st_test'])
        self.assertEqual(result['extra'], ['st_other'])
        with self.assertRaisesRegex(ValueError, 'invalid localization'):
            compile_catalog(self.root, {})

    def test_new_source_keys_cannot_be_lost_when_compiling_catalog(self):
        self.catalog()
        source = encode_strings({'st_test': 'Hello %d', 'st_new_item': 'New item'})
        with self.assertRaisesRegex(ValueError, 'source keys missing.*st_new_item'):
            compile_catalog(self.root, {'english': source})
        self.catalog({'st_test': 'Hello %d', 'st_new_item': 'New item'})
        compiled = compile_catalog(self.root, {'english': source})
        self.assertEqual(set(compiled), {name for name, _ in LANGUAGES} | {'original'})
        self.assertEqual(compiled['original'], compiled['russian'])
        self.assertEqual(read_strings(compiled['english']), (read_strings(source)[0], []))

    def test_tsv_escapes_and_significant_spaces_survive_roundtrip(self):
        table = self.root / 'serbian.tsv'
        table.write_text('key\ttext\nst_test\t Čekaj {USE} %d\\nJoš\\tC:\\\\new \n')
        strings = read_translations(table)
        self.assertEqual(strings['st_test'], ' Čekaj {USE} %d\nJoš\tC:\\new ')
        self.assertEqual(read_strings(encode_strings(strings)), (strings, []))
        for content, error in [('key\ttext\na\tOne\na\tTwo\n', 'duplicate'),
                               ('a\tOne\n', 'header'), ('key\ttext\na\tBad\\q\n', 'escape'),
                               ('key\ttext\na\tBad\\\n', 'escape'),
                               ('key\ttext\na\tNUL\0\n', 'NUL')]:
            table.write_text(content)
            with self.subTest(content=content), self.assertRaisesRegex(ValueError, error):
                read_translations(table)

    def test_catalog_rejects_empty_oversized_and_non_latin_added_text(self):
        for value, error in [('', 'empty'), ('č' * 256, 'too_long'), ('Здраво %d', 'non_latin')]:
            self.catalog()
            (self.root / 'serbian.tsv').write_text('key\ttext\nst_test\t' + value + '\n')
            with self.subTest(error=error):
                self.assertEqual(checker.audit_catalog(self.root)['languages']['serbian'][error], ['st_test'])
                with self.assertRaisesRegex(ValueError, error):
                    compile_catalog(self.root, {})

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
        self.catalog()
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(checker.main(['--catalog', str(self.root)]), 0)
            (self.root / 'german.tsv').unlink()
            self.assertEqual(checker.main(['--catalog', str(self.root)]), 1)
            self.assertEqual(checker.main(['--catalog', str(self.root / 'absent')]), 2)

    def test_compiled_resource_audit_rejects_stale_translation(self):
        self.catalog()
        for language, _ in LANGUAGES:
            self.dictionary(language, {'st_test': 'Hello %d'})
        args = [str(self.root), '--catalog', str(self.root)]
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(checker.main(args), 0)
            self.dictionary('polish', {'st_test': 'Old wording %d'})
            self.assertEqual(checker.main(args), 1)

    def test_manifest_stages_all_localization_inputs(self):
        path = Path(__file__).resolve().parents[2] / 'release/build_content.py'
        spec = importlib.util.spec_from_file_location('build_content', path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        localization = ['localization/english/localization', 'localization/russian/localization',
                        'localization/english/eula', 'localization/flash_movies_localizations.options']
        known = localization + ['gameplay/item', 'flash_movies/lobby_menu.swf']
        self.assertEqual(module.edit_paths({'localization': 'catalog'}, known), sorted(localization))

    def test_content_edits_cannot_bypass_the_catalog_or_reference_missing_text(self):
        release = self.root / 'release'
        edits = self.root / 'srp-026g/scripts/content-edit'
        release.mkdir()
        edits.mkdir(parents=True)
        (edits / 'catalog.json').write_text('{"localization":"catalog"}')
        (edits / 'item.json').write_text(json.dumps({'set_entry': [
            {'file': 'gameplay/item', 'to': {'name': 'st_new_item'}}]}))
        manifest = release / 'content.tsv'
        manifest.write_text('edit\titem.json\t-\titem\nedit\tcatalog.json\t-\ttranslations\n')
        self.assertEqual(checker.audit_content_sources(self.root, {'st_new_item'}), [])
        self.assertTrue(any('st_new_item' in issue for issue in checker.audit_content_sources(self.root, set())))
        manifest.write_text('edit\tcatalog.json\t-\ttranslations\nedit\titem.json\t-\titem\n')
        self.assertTrue(any('must end' in issue for issue in checker.audit_content_sources(self.root, {'st_new_item'})))
        (edits / 'item.json').write_text(json.dumps({'edit_text': [
            {'file': 'localization/polish/localization', 'path': 'strings/st_new_item'}]}))
        self.assertTrue(any('edit dictionary text' in issue
                            for issue in checker.audit_content_sources(self.root, {'st_new_item'})))

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

    def test_flash_numbered_labels_without_cyrillic_still_require_all_languages(self):
        row = ['Loadout {n}', '{n}', '{n}', '{n}', '{n}', 'Komplet {n}']
        self.flash_fixture(row)
        self.assertTrue(any('expected 9 language columns' in issue
                            for issue in audit_flash(self.root)['issues']))
        self.flash_fixture(row + ['Výbava {n}', '{n}. összeállítás', 'Varustesetti {n}'])
        self.assertEqual(audit_flash(self.root)['issues'], [])

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

    def test_all_dictionaries_cover_the_same_keys_and_formats(self):
        here = Path(__file__).resolve().parent / 'localization'
        baseline = read_translations(here / 'english.tsv')
        for language, _ in LANGUAGES:
            with self.subTest(language=language):
                translated = read_translations(here / f'{language}.tsv')
                report = checker.compare(baseline, translated)
                for field in ('missing', 'extra', 'empty', 'placeholders', 'markup'):
                    self.assertEqual(report[field], [], field)
                encoded = encode_strings(translated)
                self.assertEqual(read_strings(encoded), (translated, []))

    def test_new_language_alphabets_survive_native_roundtrip(self):
        text = 'ÁČĎÉĚÍŇÓŘŠŤÚŮÝŽ áčďéěíňóřšťúůýž ÁÉÍÓÖŐÚÜŰ áéíóöőúüű ÅÄÖ åäö'
        self.assertEqual(read_strings(encode_strings({'st_letters': text})), ({'st_letters': text}, []))

    def test_content_edit_compiles_dictionary_and_copies_support_files_exactly(self):
        source = self.dictionary('english', {'st_test': 'Hello %d'})
        support = b'opaque support data\xff\x00'
        (source.parent / 'eula').write_bytes(support)
        self.catalog()
        (self.root / 'serbian.tsv').write_text('key\ttext\nst_test\tZdravo %d\n', encoding='utf-8')
        edit = self.root / 'edit.json'
        edit.write_text(json.dumps({'localization': '.'}))
        script = Path(__file__).resolve().parents[1] / 'scripts/content-edit/content_edit.py'
        output = self.root / 'result'
        run = subprocess.run([sys.executable, '-I', '-S', str(script), str(edit), str(self.root), str(output)],
                             capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
        for language, _ in LANGUAGES:
            expected = read_translations(self.root / f'{language}.tsv')
            self.assertEqual(read_strings(output / f'localization/{language}/localization'), (expected, []))
        self.assertEqual((output / 'localization/original/localization').read_bytes(),
                         (output / 'localization/russian/localization').read_bytes())
        self.assertEqual((output / 'localization/serbian/eula').read_bytes(), support)
        self.assertIn('0 unexpected differences', run.stdout)


if __name__ == '__main__':
    unittest.main()
