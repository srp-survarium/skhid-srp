"""Check player-folder/ZIP contents without distributing or needing a game binary."""
import contextlib
import hashlib
import io
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest import mock
import zipfile

import make_client_026g as overlay


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='skhid-release-')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.game = self.root / 'install/game'
        self.exe = self.game / 'binaries/x86/survarium.exe'
        self.exe.parent.mkdir(parents=True)
        self.exe.write_bytes(b'original fixture')
        cert = self.game / overlay.CERT_REL
        cert.parent.mkdir(parents=True)
        cert.write_bytes(b'original certificate fixture')
        self.dll = self.root / 'bugtrap.dll'
        dll = bytearray(128)
        dll[:2] = b'MZ'
        dll[0x3c:0x40] = (64).to_bytes(4, 'little')
        dll[64:68] = b'PE\0\0'
        dll[68:70] = (0x14c).to_bytes(2, 'little')
        dll[86:88] = (0x2000).to_bytes(2, 'little')
        dll[88:90] = (0x10b).to_bytes(2, 'little')
        self.dll.write_bytes(dll)
        # The launcher's fixtures: an "original" launcher in the game folder
        # and an updater with the manifest the builder insists on.
        (self.exe.parent / 'survarium_launcher.exe').write_bytes(b'original launcher fixture')
        self.updater = self.root / 'updater_skhid.exe'
        updater = bytearray(dll)
        updater[86:88] = (0x0102).to_bytes(2, 'little')
        self.updater.write_bytes(updater + b"<requestedExecutionLevel level='asInvoker' uiAccess='false'/>")
        self.without_launcher = {'README.txt', 'LINUX.txt',
            'game/binaries/x86/survarium_skhid.exe', 'game/binaries/x86/bugtrap.dll',
            'game/binaries/x86/play_skhid.sh',
            'game/resources/ssl/survarium_login_server.crt'}
        self.names = self.without_launcher | {
            'game/binaries/x86/launcher_skhid.exe', 'game/binaries/x86/updater_skhid.exe',
            'game/binaries/x86/7zr.exe',
            'game/binaries/x86/updater_skhid.url',
        } | {f'game/binaries/x86/launcher_skhid/index-{blank}{code}.html'
             for blank in ('', 'blank-') for code in ('en', 'ru', 'ua', 'de', 'pl')}

    def build(self, output, *options):
        args = ['make_client_026g.py', str(self.game), str(output),
                '--server=release.example:2234', '--localization=polish',
                '--bugtrap=' + str(self.dll), '--updater=' + str(self.updater), *options]
        # Binary patching has its own disassembly suite. Exercise every other
        # part of the real release builder, including source reads and ZIP I/O.
        launcher = mock.patch.object(
            overlay.L, 'build', return_value=(b'patched launcher fixture', 'test launcher'))
        with mock.patch.object(overlay.P, 'build', return_value=(b'patched fixture', 'test patch')) as patcher:
            with launcher as self.launcher, mock.patch('sys.argv', args), \
                    contextlib.redirect_stdout(io.StringIO()) as messages:
                overlay.main()
        self.assertEqual(self.exe.read_bytes(), b'original fixture')
        return patcher.call_args, messages.getvalue()

    def test_folder_and_zip_ship_identical_player_files(self):
        folder, archive = self.root / 'overlay', self.root / 'release.zip'
        self.build(folder)
        self.build(archive, '--pack-zip')
        paths = {p.relative_to(folder).as_posix(): p for p in folder.rglob('*') if p.is_file()}
        self.assertEqual(set(paths), self.names)
        with zipfile.ZipFile(archive) as z:
            self.assertEqual(set(z.namelist()), self.names)
            self.assertIsNone(z.testzip())
            for name, path in paths.items():
                self.assertEqual(z.read(name), path.read_bytes(), name)
                info = z.getinfo(name)
                expected = 0o755 if name.endswith('.sh') else 0o644
                self.assertEqual(stat.S_IMODE(info.external_attr >> 16), expected)
                self.assertEqual(info.compress_type, zipfile.ZIP_DEFLATED)
        script = paths['game/binaries/x86/play_skhid.sh']
        if os.name != 'nt':
            self.assertEqual(stat.S_IMODE(script.stat().st_mode), 0o755)
        self.assertNotIn(b'\r', script.read_bytes())
        self.assertIn(b'localization=polish\nserver=\n', script.read_bytes())
        self.assertIn(b'LINUX.txt', paths['README.txt'].read_bytes())
        # The launcher is built from the game folder's own, for the server's
        # page, and starts the game with the launch line.
        self.assertEqual(self.launcher.call_args.args[0], b'original launcher fixture')
        self.assertEqual(self.launcher.call_args.kwargs, dict(
            site='https://release.example', game_exe='survarium_skhid.exe',
            arguments='-mt_allocator=768 -minimum_resources_memory_size=768'))
        self.assertEqual(paths['game/binaries/x86/launcher_skhid.exe'].read_bytes(), b'patched launcher fixture')
        self.assertEqual(paths['game/binaries/x86/updater_skhid.exe'].read_bytes(), self.updater.read_bytes())
        self.assertEqual(paths['game/binaries/x86/updater_skhid.url'].read_bytes(), b'https://release.example\r\n')
        offline = paths['game/binaries/x86/launcher_skhid/index-ua.html'].read_text(encoding='utf-8')
        self.assertIn('href="https://release.example/"', offline)
        self.assertNotIn('survarium.com', offline)
        readme = paths['README.txt'].read_text(encoding='utf-8')
        # The launcher is the file to start; nothing to type, and the
        # archive is downloaded once. No start scripts on Windows.
        self.assertIn('double-click game\\binaries\\x86\\launcher_skhid.exe in that folder', readme)
        self.assertIn('You download this archive once', readme)
        self.assertIn('asks https://release.example whether', readme)
        self.assertIn('run bash play_skhid.sh launcher', readme)
        self.assertNotIn('.bat', readme)
        self.assertNotIn('Skhid.sh', readme)

    def test_the_same_input_gives_the_same_archive(self):
        # The updater installs whenever the archive's checksum changes, so a
        # build must not differ from the one before by its own doing.
        first, second = self.root / 'first.zip', self.root / 'second.zip'
        self.build(first, '--pack-zip')
        self.build(second, '--pack-zip')
        self.assertEqual(first.read_bytes(), second.read_bytes())

    def test_without_the_launcher_it_is_the_six_player_files(self):
        archive = self.root / 'plain.zip'
        self.build(archive, '--pack-zip', '--no-launcher')
        self.launcher.assert_not_called()
        with zipfile.ZipFile(archive) as z:
            self.assertEqual(set(z.namelist()), self.without_launcher)
            readme = z.read('README.txt').decode('utf-8')
        self.assertIn('start game\\binaries\\x86\\survarium_skhid.exe with the options', readme)
        self.assertNotIn('launcher_skhid', readme)
        self.assertNotIn('.bat', readme)

    def test_the_site_can_be_another_host_than_the_login_server(self):
        folder = self.root / 'other-site'
        self.build(folder, '--site=https://pages.example/srp/')
        self.assertEqual(self.launcher.call_args.kwargs['site'], 'https://pages.example/srp')
        self.assertEqual((folder / 'game/binaries/x86/updater_skhid.url').read_bytes(),
                         b'https://pages.example/srp\r\n')

    def test_an_updater_that_would_ask_for_administrator_rights_is_refused(self):
        self.updater.write_bytes(self.updater.read_bytes().replace(b'asInvoker', b'asWhoever'))
        with self.assertRaises(SystemExit) as refused:
            self.build(self.root / 'refused')
        self.assertIn('asInvoker', str(refused.exception))
        self.assertFalse((self.root / 'refused').exists())

    def test_the_extractor_shipped_is_the_one_the_updater_runs(self):
        # The updater runs 7zr.exe only when its size and sha256 are the two
        # compiled into it: the builder's numbers must be the updater's, and
        # the file in the archive the one with those numbers.
        import hashlib
        import re
        pins = Path(overlay.HERE, '..', '..', 'client', 'updater', 'logic', 'src', 'game.rs').read_text(encoding='utf-8')
        size, digest = re.search(
            r'pub const EXTRACTOR: Pin = Pin \{\s*size: ([0-9_]+),\s*sha256: "([0-9a-f]{64})",', pins).groups()
        self.assertEqual((int(size.replace('_', '')), digest), (overlay.EXTRACTOR_SIZE, overlay.EXTRACTOR_SHA256))
        self.assertIn(f'pub const EXTRACTOR_NAME: &str = "{overlay.EXTRACTOR_NAME}";', pins)
        archive = self.root / 'extractor.zip'
        self.build(archive, '--pack-zip')
        with zipfile.ZipFile(archive) as z:
            shipped = z.read('game/binaries/x86/7zr.exe')
            readme = z.read('README.txt').decode('utf-8')
        self.assertEqual((len(shipped), hashlib.sha256(shipped).hexdigest()),
                         (overlay.EXTRACTOR_SIZE, overlay.EXTRACTOR_SHA256))
        self.assertIn('7zr.exe is the standalone extractor of 7-Zip', readme)

    def test_another_extractor_than_the_pinned_one_is_refused(self):
        other = self.root / '7zr.exe'
        other.write_bytes(bytes(overlay.EXTRACTOR_SIZE))
        with self.assertRaises(SystemExit) as refused:
            self.build(self.root / 'refused', '--extractor=' + str(other))
        self.assertIn('not the extractor the updater is built to run', str(refused.exception))
        self.assertFalse((self.root / 'refused').exists())

    def test_certificate_only_launcher_has_an_explicit_server(self):
        archive = self.root / 'certificate.zip'
        call, _ = self.build(archive, '--pack-zip', '--cert-only')
        self.assertEqual(call.args[1:], ('cert-only', None, None, None))
        # The game's launcher passes the address too: this exe has none inside.
        self.assertEqual(self.launcher.call_args.kwargs['arguments'],
                         '-server=release.example:2234 -mt_allocator=768 -minimum_resources_memory_size=768')
        with zipfile.ZipFile(archive) as z:
            self.assertIn(b'server=release.example:2234\n', z.read('game/binaries/x86/play_skhid.sh'))

    def test_the_default_exe_has_the_late_join_and_can_be_built_without(self):
        code = Path(overlay.P.late_join_bin()).read_bytes()
        call, _ = self.build(self.root / 'with')
        self.assertEqual(call.args[1:], ('section', mock.ANY, 'release.example:2234', code))
        call, _ = self.build(self.root / 'without', '--no-late-join')
        self.assertEqual(call.args[1:], ('section', mock.ANY, 'release.example:2234', None))
        # The other cave has no room for it.
        call, _ = self.build(self.root / 'cave', '--cave=hash-mismatch')
        self.assertEqual(call.args[1:], ('hash-mismatch', mock.ANY, 'release.example:2234', None))

    def test_a_release_names_the_revision_of_the_default_exe(self):
        call, _ = self.build(self.root / 'default')
        self.assertEqual(call.kwargs, {'revision': None})
        code = Path(overlay.P.late_join_bin()).read_bytes()
        call, _ = self.build(self.root / 'release', '--revision=17')
        self.assertEqual((call.args[1:], call.kwargs),
                         (('section', mock.ANY, 'release.example:2234', code), {'revision': 17}))
        refused = (('--revision=17', '--no-late-join'), ('--revision=17', '--cave=hash-mismatch'),
                   ('--revision=017',), ('--revision=x',))
        for n, options in enumerate(refused):
            with self.subTest(options=options), self.assertRaises(SystemExit):
                self.build(self.root / f'refused-{n}', *options)

    def test_windows_checkout_produces_a_unix_launcher(self):
        # Git's core.autocrlf can give the release builder a CRLF template.
        template = Path(overlay.HERE, 'play_skhid.sh').read_bytes().replace(b'\r\n', b'\n')
        Path(self.root, 'play_skhid.sh').write_bytes(template.replace(b'\n', b'\r\n'))
        with mock.patch.object(overlay, 'HERE', str(self.root)):
            for embedded in (False, True):
                with self.subTest(embedded=embedded):
                    script = overlay.sh('release.example:2234', 'polish', embedded)
                    self.assertNotIn(b'\r', script)
                    server = b'' if embedded else b'release.example:2234'
                    self.assertIn(b'localization=polish\nserver=' + server + b'\n', script)

    def test_check_writes_neither_folder_nor_archive(self):
        for options in ((), ('--pack-zip',)):
            with self.subTest(options=options):
                output = self.root / 'not-created' / 'output'
                _, messages = self.build(output, *options, '--check')
                self.assertFalse(output.parent.exists())
                self.assertIn('play_skhid.sh', messages)
                self.assertIn('LINUX.txt', messages)

    def test_no_part_of_resources_db_is_shipped_in_the_overlay(self):
        # Content reaches the players as patches of resources.db alone
        # (client/updater/README.md, "Patches").
        for option in ('--resource-region=lobby_menu=x.bin', '--resource-grow=grow.json',
                       '--resources-out=out'):
            with self.subTest(option=option), self.assertRaises(SystemExit) as refused:
                self.build(self.root / 'refused.zip', '--pack-zip', option)
            self.assertIn('unknown option', str(refused.exception))

    def test_existing_release_is_not_overwritten(self):
        archive = self.root / 'release.zip'
        archive.write_bytes(b'existing release')
        with self.assertRaises(SystemExit):
            self.build(archive, '--pack-zip')
        self.assertEqual(archive.read_bytes(), b'existing release')


if __name__ == '__main__':
    unittest.main()
