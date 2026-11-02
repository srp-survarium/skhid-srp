"""Check player-folder/ZIP contents without distributing or needing a game binary."""
import contextlib
import hashlib
import io
import json
import locale
import os
from pathlib import Path
import re
import select
import shutil
import stat
import struct
import subprocess
import tempfile
import time
import unittest
from unittest import mock
import zipfile

import make_client_026g as overlay

# The programs that open connections to the Skhid hosts: the ones VPN.txt
# and Skhid-network-setup.cmd name.
PROGRAMS = ('launcher_skhid.exe', 'survarium_skhid.exe', 'updater_skhid.exe')
WINE = shutil.which('wine')


def unix_locale(name):
    """Whether this system has the locale `name`: Wine takes a Russian
    Windows's code page and its Unix file names' encoding from it."""
    if os.name == 'nt':
        return True
    kept = locale.setlocale(locale.LC_CTYPE)
    try:
        locale.setlocale(locale.LC_CTYPE, name)
        return True
    except locale.Error:
        return False
    finally:
        locale.setlocale(locale.LC_CTYPE, kept)


# A Russian Windows, under Wine: ru_RU.UTF-8 (on NixOS, nix-shell -p glibcLocales).
RUSSIAN = unix_locale('ru_RU.UTF-8')
# The Windows display languages the setup scripts speak Russian in (any
# other, or none: English).
RUSSIAN_UI = ('ru', 'uk', 'be')
# What runs the script's PowerShell code: Windows PowerShell, elsewhere pwsh
# (PowerShell 7; on NixOS nix shell nixpkgs#powershell).
POWERSHELL = shutil.which('powershell') if os.name == 'nt' else shutil.which('pwsh')

# A program that writes down how it was started (client/updater/tests).
STAND_IN = Path(overlay.HERE, '..', '..', 'client', 'updater', 'tests', 'fake_exe', 'target',
                'i686-win7-windows-msvc', 'release', 'fake-exe.exe').resolve()


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
            'game/binaries/x86/play_skhid.bat', 'game/binaries/x86/play_skhid.sh',
            'game/resources/ssl/survarium_login_server.crt'}
        self.names = self.without_launcher | {
            'game/binaries/x86/launcher_skhid.exe', 'game/binaries/x86/updater_skhid.exe',
            'game/binaries/x86/7zr.exe',
            'game/binaries/x86/updater_skhid.url', 'Skhid.bat', 'Skhid-without-updater.bat', 'Skhid.sh',
            'VPN.txt', 'Skhid-network-setup.cmd', 'Skhid-defender-setup.cmd', 'Skhid-collect-logs.cmd',
        } | {f'game/binaries/x86/launcher_skhid/index-{blank}{code}.html'
             for blank in ('', 'blank-') for code in ('en', 'ru', 'ua', 'de', 'pl')}

    def build(self, output, *options):
        args = ['make_client_026g.py', str(self.game), str(output),
                '--server=release.example:2234',
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

    def test_scripts_pass_no_language(self):
        output = self.root / 'default'
        self.build(output)
        windows = (output / 'game/binaries/x86/play_skhid.bat').read_bytes()
        linux = (output / overlay.LINUX_LAUNCHER).read_bytes()
        self.assertNotIn(b'LANGUAGE', windows)
        self.assertNotIn(b'localization', windows)
        self.assertNotIn(b'localization', linux)
        self.assertRegex(windows, rb'start "" survarium_skhid\.exe -(server|mt_allocator)=')
        refused = self.root / 'refused'
        with self.assertRaises(SystemExit):
            self.build(refused, '--localization=polish')

    def test_without_the_updater_the_game_starts_as_the_launcher_starts_it(self):
        output = self.root / 'default'
        self.build(output)
        alone = (output / overlay.WITHOUT_UPDATER).read_text(encoding='ascii')
        # The launcher's arguments, and nothing about the language (patch 79
        # sets it).
        [start] = [line for line in alone.splitlines() if line.startswith('start ')]
        self.assertEqual(start, 'start "" survarium_skhid.exe ' + self.launcher.call_args.kwargs['arguments'])
        self.assertEqual(start, 'start "" ' + overlay.launch_line('release.example:2234', embedded=True))
        self.assertNotIn('localization', alone.lower())
        self.assertNotIn('LANGUAGE', alone)

    def test_the_releases_addresses_are_written_into_the_file_without_the_updater(self):
        listed, game = self.root / 'patches.tsv', self.root / 'game.tsv'
        game_url = 'https://drive.usercontent.google.com/download?id=x&export=download&confirm=t'
        game.write_text('# a comment\n'
                        f'survarium-026g.7z\t{"7" * 64}\t2906544103\thttp://203.0.113.5:8090/g.7z {game_url}\n',
                        encoding='ascii')
        listed.write_text(f'resources.db.patch_12\t{"a" * 64}\t86199749\thttp://203.0.113.5:8090/resources.db.patch_12\n',
                          encoding='ascii')
        output = self.root / 'default'
        self.build(output, '--downloads=' + str(game), '--downloads=' + str(listed))
        alone = (output / overlay.WITHOUT_UPDATER).read_bytes()
        alone.decode('ascii')
        self.assertEqual(alone.count(b'\n'), alone.count(b'\r\n'))
        self.assertTrue(alone.endswith(
            f'\r\n#skhid-file survarium-026g.7z {"7" * 64} 2906544103 http://203.0.113.5:8090/g.7z {game_url}\r\n'
            f'#skhid-file resources.db.patch_12 {"a" * 64} 86199749 '
            'http://203.0.113.5:8090/resources.db.patch_12\r\n'.encode()))
        # Read where its PowerShell code reads them: after its last line.
        script = Path(overlay.HERE, overlay.WITHOUT_UPDATER).read_bytes().replace(b'\r\n', b'\n')
        self.assertTrue(alone.replace(b'\r\n', b'\n').startswith(script.rstrip(b'\n') + b'\n#skhid-file '))
        listed.write_text('resources.db.patch_12\taa\t1\thttp://h/p\n', encoding='ascii')
        with self.assertRaises(SystemExit):
            self.build(self.root / 'refused', '--downloads=' + str(listed))

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
        self.assertIn(b'\nserver=\n', script.read_bytes())
        self.assertIn(b'LINUX.txt', paths['README.txt'].read_bytes())
        # The launcher is built from the game folder's own, for the server's
        # page, and starts the game with what play_skhid.bat starts it with.
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
        # One file to start, beside `game`; nothing to type, and the archive
        # is downloaded once.
        self.assertIn('double-click Skhid.bat in that folder', readme)
        self.assertIn('You download this archive once', readme)
        self.assertNotIn('double-click game\\binaries', readme)
        self.assertIn('asks https://release.example whether', readme)
        starter = paths['Skhid.bat'].read_bytes()
        self.assertIn(b'start "" /d "%~dp0game\\binaries\\x86" "%~dp0game\\binaries\\x86\\launcher_skhid.exe"\r\n',
                      starter)
        self.assertEqual(starter.count(b'\n'), starter.count(b'\r\n'))
        # Without the updater: the file as it is, in ASCII with CRLF line ends.
        alone = paths['Skhid-without-updater.bat'].read_bytes()
        alone.decode('ascii')
        self.assertEqual(alone.count(b'\n'), alone.count(b'\r\n'))
        self.assertEqual(alone.replace(b'\r\n', b'\n'),
                         Path(overlay.HERE, 'Skhid-without-updater.bat').read_bytes().replace(b'\r\n', b'\n'))
        # The original game it downloads is the one the updater pins, and the
        # one of the release's list.
        pins = Path(overlay.HERE, '..', '..', 'client', 'updater', 'logic', 'src', 'game.rs').read_text(encoding='utf-8')
        [(size, digest)] = re.findall(r'pub const ARCHIVE: Pin = Pin \{\s*size: ([0-9_]+),\s*sha256: "([0-9a-f]{64})",', pins)
        size = size.replace('_', '')
        self.assertIn(b'\r\nset GAME_SHA=' + digest.encode() + b'\r\n', alone)
        self.assertIn(b'\r\nset GAME_SIZE=' + size.encode() + b'\r\n', alone)
        rows = Path(overlay.HERE, '..', '..', 'release', 'game.tsv').read_text(encoding='ascii').splitlines()
        [game] = [row.split('\t') for row in rows if not row.startswith('#')]
        self.assertEqual(game[:3], ['survarium-026g.7z', digest, size])
        self.assertIn(overlay.GAME_URL, game[3].split(' '))
        # No --downloads: no address written in.
        self.assertNotIn(b'\n#skhid-file ', alone)
        self.assertIn('start Skhid-without-updater.bat', ' '.join(readme.split()))
        # The same for Linux: a Bash script with Unix line ends.
        linux = paths['Skhid.sh'].read_bytes()
        self.assertTrue(linux.startswith(b'#!/usr/bin/env bash\n'))
        self.assertNotIn(b'\r', linux)
        self.assertIn(b'game/binaries/x86/play_skhid.sh', linux)
        self.assertIn('run bash Skhid.sh (the game) or bash Skhid.sh launcher', readme)

    @unittest.skipUnless(os.name == 'nt' and STAND_IN.is_file(),
                         'Windows, and the stand-in exe of client/updater/tests (build.sh there)')
    def test_the_starter_starts_the_launcher_from_its_folder(self):
        # A path with a space and a bracket, as a player's folder may have.
        install = self.root / 'My games (old)' / 'Survarium'
        folder = install / 'game' / 'binaries' / 'x86'
        folder.mkdir(parents=True)
        (install / 'Skhid.bat').write_bytes(overlay.starter())
        run = lambda: subprocess.run(  # noqa: E731
            ['cmd.exe', '/d', '/c', str(install / 'Skhid.bat')], cwd=self.root, stdin=subprocess.DEVNULL,
            capture_output=True, timeout=60)
        # Unpacked in the wrong place: it says where it belongs.
        missing = run()
        self.assertEqual(missing.returncode, 1)
        self.assertIn(b'launcher_skhid.exe was not found', missing.stdout)
        self.assertIn(b'the one that holds the', missing.stdout)
        # In its place: the launcher is started, in its own folder.
        launcher = folder / 'launcher_skhid.exe'
        shutil.copyfile(STAND_IN, launcher)
        started = run()
        self.assertEqual((started.returncode, started.stdout, started.stderr), (0, b'', b''))
        note = Path(str(launcher) + '.started.txt')
        for _ in range(100):
            if note.is_file() and note.stat().st_size:
                break
            time.sleep(0.1)
        command_line, working_folder = note.read_bytes().decode('utf-16').splitlines()[:2]
        self.assertEqual(command_line.strip().strip('"').lower(), str(launcher).lower())
        self.assertEqual(working_folder.lower(), str(folder).lower())

    def test_the_same_input_gives_the_same_archive(self):
        # The updater installs whenever the archive's checksum changes, so a
        # build must not differ from the one before by its own doing.
        first, second = self.root / 'first.zip', self.root / 'second.zip'
        self.build(first, '--pack-zip')
        self.build(second, '--pack-zip')
        self.assertEqual(first.read_bytes(), second.read_bytes())

    def test_without_the_launcher_it_is_the_seven_files_of_before(self):
        archive = self.root / 'plain.zip'
        self.build(archive, '--pack-zip', '--no-launcher')
        self.launcher.assert_not_called()
        with zipfile.ZipFile(archive) as z:
            self.assertEqual(set(z.namelist()), self.without_launcher)
            readme = z.read('README.txt').decode('utf-8')
        self.assertIn('double-click game\\binaries\\x86\\play_skhid.bat', readme)
        self.assertNotIn('launcher_skhid', readme)

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
        self.assertEqual(call.kwargs, {'revision': None, 'required_patches': None})
        code = Path(overlay.P.late_join_bin()).read_bytes()
        call, _ = self.build(self.root / 'release', '--revision=17')
        self.assertEqual((call.args[1:], call.kwargs),
                         (('section', mock.ANY, 'release.example:2234', code), {'revision': 17, 'required_patches': None}))
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
                    script = overlay.sh('release.example:2234', embedded)
                    self.assertNotIn(b'\r', script)
                    server = b'' if embedded else b'release.example:2234'
                    self.assertIn(b'\nserver=' + server + b'\n', script)

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

    def test_the_network_help_names_the_three_programs(self):
        folder = self.root / 'overlay'
        self.build(folder)
        vpn = (folder / 'VPN.txt').read_bytes()
        self.assertTrue(vpn.startswith(b'\xef\xbb\xbf'))
        self.assertEqual(vpn.count(b'\n'), vpn.count(b'\r\n'))
        text = vpn.decode('utf-8')
        english, russian = text.split('SKHID - НЕТ СВЯЗИ С СЕРВЕРОМ?')
        for half in (english, russian):
            for program in PROGRAMS:
                self.assertIn('game\\binaries\\x86\\' + program, half)
                # HAPP's example, the first one, by its full path.
                self.assertIn('C:\\Games\\Survarium\\game\\binaries\\x86\\' + program, half)
            self.assertLess(half.index('HAPP'), half.index('v2rayN'))
            for host_or_port in ('skhid.link', 'TCP 443', 'TCP 8090', 'TCP 1234', 'UDP 1235', 'UDP 1236',
                                 'https://skhid.link/patches', 'Skhid-network-setup.cmd', 'LINUX.txt'):
                self.assertIn(host_or_port, half)
        # Public names only: no address, no other host.
        self.assertNotRegex(text, r'\d+\.\d+\.\d+\.\d+')
        readme = (folder / 'README.txt').read_text(encoding='utf-8')
        texts = {}
        for name in (overlay.NETWORK_SETUP, overlay.DEFENDER_SETUP, overlay.COLLECT_LOGS):
            script = (folder / name).read_bytes()
            # Code page 866, switched to before the first Russian line: all
            # above it is ASCII, read in whatever code page the console has.
            switch = script.index(b'\r\nchcp 866 >nul 2>&1\r\n')
            script[:switch].decode('ascii')
            self.assertIsNone(re.search(rb'(?im)^\s*echo', script[:switch]))
            text = texts[name] = script.decode('cp866')
            self.assertEqual(text.encode('cp866'), script)
            self.assertEqual(script.count(b'\n'), script.count(b'\r\n'))
            # Every line a player reads is in the language of the Windows (the
            # owner, 2026-11-01): a %RU% line is Russian, a %EN% line English;
            # an echo is only names and paths, or a --dry-run command.
            for line in text.splitlines():
                said = re.match(r'\s*(?:if [^(]* )?(echo|%RU%|%EN%)[ (.](.*)', line, re.I)
                if not said:
                    continue
                if said[1] == '%RU%':
                    self.assertRegex(said[2], '[А-Яа-яЁё]|^=+$', line)
                elif said[1] == '%EN%':
                    self.assertTrue(said[2].isascii(), line)
                elif not said[2].startswith('[dry-run]'):
                    self.assertNotRegex(said[2], r'[A-Za-z]{4,} [a-z]{2,}|[А-Яа-яЁё]', line)
            if name == overlay.COLLECT_LOGS:
                # Reads only, as the player: no elevation, nothing of Defender
                # or the firewall changed, no file of the game written or
                # deleted (its own folder and ZIP alone).
                for word in ('RunAs', 'Add-MpPreference', 'Remove-MpPreference', 'netsh', 'New-NetFirewallRule',
                             'Remove-Item', 'Move-Item', 'Set-Content', 'Out-File'):
                    self.assertNotIn(word, text)
            else:
                # The elevation: the normal UAC prompt, by Start-Process -Verb RunAs
                # of cmd.exe and the command line a dry run prints.
                self.assertIn('Start-Process -FilePath $env:ComSpec -ArgumentList $env:SKHID_CMDLINE -Verb RunAs',
                              text)
            self.assertIn('cd /d "%~dp0"', text)
            # Its folder is taken before the options are shifted (shift moves %0).
            self.assertLess(text.index('set "TOP=%~dp0"'), text.index('\r\nshift\r\n'))
            self.assertLess(text.index('set "SKHID_SELF=%~f0"'), text.index('\r\nshift\r\n'))
            # Nothing turned off: no firewall state, no Defender setting but the exclusion.
            for word in (b'state off', b'opmode', b'Set-MpPreference', b'DisableRealtimeMonitoring'):
                self.assertNotIn(word.lower(), script.lower())
            self.assertIn(name, readme)
        network, defender = texts[overlay.NETWORK_SETUP], texts[overlay.DEFENDER_SETUP]
        self.assertRegex(network, r"Write-Output \('ВНИМАНИЕ: [^']*'")
        self.assertRegex(network, r"Write-Output \('WARNING: [^']*'")
        self.assertIn('set "PROGRAMS=' + ' '.join(PROGRAMS) + '"', network)
        # Each does its one thing: the firewall's no Defender, Defender's no firewall.
        self.assertNotIn('MpPreference', network)
        self.assertNotIn('netsh', defender)
        # Only PowerShell writes the Defender marker (cmd would write it in
        # code page 866, losing a Ukrainian or Polish letter). Its name is the
        # one of the network setup that asked for the exclusion before: an
        # exclusion made by that one is undone by this one.
        self.assertNotRegex(defender, r'>\s*"%MARKER%"')
        self.assertIn('set "MARKER=%TOP%\\Skhid-network-setup.defender"', defender)
        self.assertIn('VPN.txt', readme)
        # Where the collected logs go: the bug-reports invite (owner, 2026-11-02).
        self.assertIn('Send\nthat file to us on Discord: https://discord.gg/QUmJg3gBhg', readme)
        self.assertIn('VPN.txt', (folder / 'LINUX.txt').read_text(encoding='utf-8'))

    def windows_env(self, locale='en_US.UTF-8', ui=None):
        """The environment of a Windows program here: this one, or Wine's in a
        prefix of its own (no window is opened). Under Wine, `locale` is the
        Windows': ru_RU.UTF-8 is a Russian one (code page 866), en_US.UTF-8
        an English one (437). `ui`, when given, is the display language the
        setup scripts are told: a stand-in powershell.exe first in the PATH
        prints it ('': nothing) and does nothing else."""
        if os.name == 'nt':
            env = dict(os.environ)
        else:
            prefix = self.root / ('wineprefix-' + locale.split('.')[0])
            prefix.mkdir(exist_ok=True)
            env = {k: v for k, v in os.environ.items()
                   if k not in ('DISPLAY', 'WAYLAND_DISPLAY', 'LANGUAGE') and not k.startswith('LC_')}
            env.update(WINEPREFIX=str(prefix), WINEDEBUG='-all', LANG=locale, LC_ALL=locale)
        if ui is not None:
            stubs = self.root / ('ui-' + (ui or 'none'))
            if not stubs.exists():
                stubs.mkdir()
                exit_program(stubs / 'powershell.exe', 0, ui)
            if os.name == 'nt':
                env['PATH'] = str(stubs) + os.pathsep + env['PATH']
            else:
                env['WINEPATH'] = 'Z:' + str(stubs).replace('/', '\\')
        return env

    def cmd(self, folder, *arguments, env=None):
        """cmd.exe (elsewhere Wine's) run in `folder` with `arguments`: what it
        printed, its lines (Skhid-network-setup.cmd prints in code page 866)."""
        command = (['cmd.exe'] if os.name == 'nt' else [WINE, 'cmd']) + ['/d', '/c', *arguments]
        done = subprocess.run(command, cwd=folder, env=env or self.windows_env(), stdin=subprocess.DEVNULL,
                              capture_output=True, timeout=600)
        if os.environ.get('SKHID_TEST_DEBUG'):
            print(command, done.returncode, done.stdout, done.stderr)
        return done.stdout.decode('cp866').replace('\r\n', '\n').splitlines()

    def setup_script(self, top, name, *options, locale='en_US.UTF-8', ui='ru'):
        """The script `name` of the overlay in `top`, run by its name from its
        folder (it finds the folder itself, %~dp0)."""
        return self.cmd(top, name, *options, env=self.windows_env(locale, ui))

    def elevated(self, line, locale='en_US.UTF-8', ui='ru'):
        """The elevated run of a dry run's `elevate:` line, as Start-Process
        starts it (cmd.exe and that command line), without asking for
        administrator rights, from another folder (an elevated one starts in
        System32). The line reaches cmd.exe as it is: a script of the test's
        hands it over by delayed expansion, which nothing parses again."""
        harness = self.root / 'elevate.cmd'
        harness.write_bytes(b'@echo off\r\nsetlocal EnableDelayedExpansion\r\n'
                            b'"%ComSpec%" !SKHID_TEST_CMDLINE!\r\n')
        prefix = '[dry-run] elevate: cmd.exe '
        self.assertTrue(line.startswith(prefix), line)
        env = self.windows_env(locale, ui)
        env['SKHID_TEST_CMDLINE'] = line[len(prefix):]
        return self.cmd(self.root, 'elevate.cmd', env=env)

    @unittest.skipUnless(os.name == 'nt' or WINE, 'Windows, or Wine for its cmd.exe')
    def test_the_setup_scripts_add_and_remove_exactly_their_changes(self):
        # Folders with a space and brackets, with an &, with a !, in
        # Cyrillic, Ukrainian (і, which code page 866 has not) and Polish, as
        # a player's may be; under Wine on a Russian Windows and on an
        # English one. Each in another display language: Russian for ru, uk
        # and be, English for the others and when PowerShell says nothing.
        locales = ['en_US.UTF-8']
        if os.name != 'nt' and RUSSIAN:
            locales.insert(0, 'ru_RU.UTF-8')
        for locale in locales:
            for name, ui in (('My Games (x86)', 'ru'), ('Tom & Jerry (x86)', 'en'), ('Wow! Games (x86)', 'uk'),
                             ('Мои игры! & (x86)', ''), ('Віталій (x86)', 'be'), ('Café Łódź (x86)', 'de')):
                with self.subTest(locale=locale, folder=name, ui=ui):
                    self.setup_scripts_in(self.root / locale / name / 'Survarium', locale, ui)

    def assertShown(self, lines, expected, locale):
        """`lines` (what the script printed into a pipe) are `expected`, but
        for the letters the pipe cannot carry: a path's letters outside code
        page 866 (Wine's Russian Windows, and Windows after the script's
        chcp), and any but ASCII on Wine's English one (437, where cmd's own
        bytes pass as they are but a path is written in 437)."""
        def pattern(line):
            def letter(c):
                if c.isascii():
                    return re.escape(c)
                if os.name == 'nt' or locale.startswith('ru'):
                    try:
                        c.encode('cp866')
                        return re.escape(c)
                    except UnicodeEncodeError:
                        pass
                return '.'
            return ''.join(map(letter, line))
        self.assertEqual(len(lines), len(expected), '\n'.join(lines))
        for line, want in zip(lines, expected):
            self.assertRegex(line, '^' + pattern(want) + '$', '\n'.join(lines))

    def setup_scripts_in(self, top, locale, ui):
        self.build(top)

        def said(russian, english):
            return russian if ui in RUSSIAN_UI else english
        x86 = str(top / 'game' / 'binaries' / 'x86')
        if os.name != 'nt':
            x86 = 'Z:' + x86.replace('/', '\\')
        top_windows = x86.rsplit('\\game\\', 1)[0]
        # Out only, TCP and UDP: nothing comes in.
        rules = [(program, protocol) for program in PROGRAMS for protocol in ('TCP', 'UDP')]

        def delete(program, protocol):
            return f'[dry-run] netsh advfirewall firewall delete rule name="Skhid {program} {protocol} out"'

        def commands(lines):
            return [line for line in lines if line.startswith('[dry-run] ') and 'elevate:' not in line]

        def setup(*options):
            return self.setup_script(top, overlay.NETWORK_SETUP, *options, locale=locale, ui=ui)

        def defender(*options):
            return self.setup_script(top, overlay.DEFENDER_SETUP, *options, locale=locale, ui=ui)

        # The restore of the files Defender deleted: from the server
        # updater_skhid.url names, else skhid.link.
        def restore(site):
            return [f'[dry-run] download {site}/download, check it against the sha256 of {site}/api/client-files',
                    f'[dry-run] put back its files missing in "{top_windows}", overwriting none']
        site = (top / 'game' / 'binaries' / 'x86' / 'updater_skhid.url').read_text().strip()

        def shown(lines, expected):
            self.assertShown(lines, expected, locale)

        # Whether the pipe carries the folder letter for letter: then the
        # printed elevation line can be run as it is.
        try:
            top_windows.encode('ascii' if os.name != 'nt' and locale.startswith('en') else 'cp866')
            faithful = True
        except UnicodeEncodeError:
            faithful = False

        # The firewall.
        added = setup('--dry-run')
        self.assertIn(said('Настройка сети Skhid', 'Skhid network setup'), added)
        folder = said('Папка игры: ', 'Game folder: ')
        shown([line for line in added if line.startswith(folder)], [f'{folder}"{top_windows}"'])
        for program in PROGRAMS:
            self.assertIn(said(f'Брандмауэр: {program} разрешены исходящие соединения (TCP и UDP)',
                               f'Firewall: {program} is allowed outgoing connections (TCP and UDP)'), added)
        expected = []
        for program, protocol in rules:
            expected += [delete(program, protocol),
                         f'[dry-run] netsh advfirewall firewall add rule name="Skhid {program} {protocol} out" '
                         f'dir=out action=allow program="{x86}\\{program}" protocol={protocol} enable=yes '
                         f'profile=any']
        expected += ['[dry-run] look for firewall rules that block the three programs']
        shown(commands(added), expected)
        self.assertNotIn(' dir=in ', '\n'.join(added))
        # The command line the elevation starts cmd.exe with, and that run.
        elevate = [line for line in added if 'elevate:' in line]
        shown(elevate, [f'[dry-run] elevate: cmd.exe /d /c ""{top_windows}\\Skhid-network-setup.cmd" '
                        '--elevated --dry-run"'])
        if faithful:
            elevated = self.elevated(elevate[0], locale, ui)
            shown(commands(elevated), expected)
            self.assertFalse([line for line in elevated if 'elevate:' in line])
        # Every rule is removed by its name before it is added: a second run
        # adds none twice.
        self.assertEqual(setup('--dry-run'), added)
        # --undo: the same six rules.
        undone = setup('--dry-run', '--undo')
        shown(commands(undone), [delete(*rule) for rule in rules])
        shown([line for line in undone if 'elevate:' in line],
              ['[dry-run] elevate: cmd.exe /d /c ""' + top_windows + '\\Skhid-network-setup.cmd" '
               '--elevated --undo --dry-run"'])
        if faithful:
            elevated = self.elevated([line for line in undone if 'elevate:' in line][0], locale, ui)
            self.assertEqual(commands(elevated), commands(undone), '\n'.join(elevated))

        # The Defender exclusion: game\binaries\x86 alone, never the folder
        # above it; then the files Defender deleted.
        added = defender('--dry-run')
        self.assertIn(said('Исключение Skhid в Microsoft Defender', 'Skhid exclusion in Microsoft Defender'), added)
        shown([line for line in added if line.startswith(folder)], [f'{folder}"{top_windows}"'])
        expected = [f'[dry-run] powershell Add-MpPreference -ExclusionPath "{x86}"',
                    f'[dry-run] write "{top_windows}\\Skhid-network-setup.defender":'] + restore(site)
        shown(commands(added), expected)
        # What the marker would hold: the path as it is, by the line that
        # writes it.
        writes = next(i for i, line in enumerate(added) if line.startswith('[dry-run] write '))
        shown(added[writes + 1:writes + 2], [x86])
        elevate = [line for line in added if 'elevate:' in line]
        shown(elevate, [f'[dry-run] elevate: cmd.exe /d /c ""{top_windows}\\Skhid-defender-setup.cmd" '
                        '--elevated --dry-run"'])
        if faithful:
            elevated = self.elevated(elevate[0], locale, ui)
            shown(commands(elevated), expected)
            self.assertFalse([line for line in elevated if 'elevate:' in line])
        self.assertEqual(defender('--dry-run'), added)
        # --undo: no exclusion this script (or the network setup before it)
        # did not add.
        undone = defender('--dry-run', '--undo')
        self.assertFalse(commands(undone), '\n'.join(undone))
        self.assertIn(said('Defender: этот файл не добавлял здесь исключений, удалять нечего.',
                           'Defender: this file added no exclusion here, nothing to remove.'), undone)
        shown([line for line in undone if 'elevate:' in line],
              ['[dry-run] elevate: cmd.exe /d /c ""' + top_windows + '\\Skhid-defender-setup.cmd" '
               '--elevated --undo --dry-run"'])
        # With the marker: the exclusion it names is removed (PowerShell
        # reads it: test_the_defender_marker_keeps_every_letter), in the
        # elevated run too.
        marker = top / 'Skhid-network-setup.defender'
        pointer = said('Defender: исключение папки игры удаляет Skhid-defender-setup.cmd --undo',
                       "Defender: the game folder's exclusion is removed by Skhid-defender-setup.cmd --undo")
        self.assertNotIn(pointer, setup('--dry-run', '--undo'))
        marker.write_bytes(b'\xef\xbb\xbf' + x86.encode())
        # The network setup, which made it once, says which file removes it.
        self.assertIn(pointer, setup('--dry-run', '--undo'))
        undone = defender('--dry-run', '--undo')
        shown(commands(undone),
              [f'[dry-run] powershell Remove-MpPreference -ExclusionPath (the path in "{top_windows}'
               '\\Skhid-network-setup.defender")',
               f'[dry-run] delete "{top_windows}\\Skhid-network-setup.defender"'])
        if faithful:
            elevated = self.elevated([line for line in undone if 'elevate:' in line][0], locale, ui)
            self.assertEqual(commands(elevated), commands(undone), '\n'.join(elevated))
        marker.unlink()

        # Defender deleted the exe: the Defender setup knows its place by the
        # updater's address still, and puts the exe back.
        exe = top / 'game' / 'binaries' / 'x86' / 'survarium_skhid.exe'
        kept = exe.read_bytes()
        exe.unlink()
        shown(commands(defender('--dry-run')), expected)
        # Not beside the game: each says where it belongs and changes nothing.
        (top / 'game' / 'binaries' / 'x86' / 'updater_skhid.url').unlink()
        for run in (setup, defender):
            lost = run('--dry-run')
            self.assertIn(said('Не найден survarium_skhid.exe в папке', 'survarium_skhid.exe is not in the folder'),
                          lost)
            # The network setup names the usual cause.
            self.assertEqual(said('Если его удалил Microsoft Defender, запустите Skhid-defender-setup.cmd.',
                                  'If Microsoft Defender deleted it, run Skhid-defender-setup.cmd.') in lost,
                             run is setup)
            self.assertFalse([line for line in lost if line.startswith('[dry-run] ')])
            self.assertIn(said('Неизвестный параметр: --firewall-off', 'Unknown option: --firewall-off'),
                          run('--firewall-off'))
        self.assertIn(said('Неизвестный параметр: --defender-yes', 'Unknown option: --defender-yes'),
                      setup('--defender-yes'))
        # Without the updater's address, the server is skhid.link.
        exe.write_bytes(kept)
        shown(commands(defender('--dry-run'))[2:], restore('https://skhid.link'))
        # An address the updater would not take either (plain http elsewhere
        # than to this computer): skhid.link. (Once per Windows: it is not the
        # folder's.)
        if ui == 'ru':
            url = top / 'game' / 'binaries' / 'x86' / 'updater_skhid.url'
            for written, used in (('http://skhid.example', 'https://skhid.link'),
                                  ('http://localhost.example', 'https://skhid.link'),
                                  ('https:///x', 'https://skhid.link'),
                                  ('http://127.0.0.1:8080', 'http://127.0.0.1:8080'),
                                  ('https://other.example:8443', 'https://other.example:8443')):
                url.write_bytes(written.encode() + b'\r\n')
                shown(commands(defender('--dry-run'))[2:], restore(used))

    def powershell_of(self, word):
        """The PowerShell code of the shipped Defender script's powershell
        line with `word` in it: what cmd hands to powershell -Command (no % in
        it, so cmd changes nothing)."""
        folder = self.root / 'overlay'
        if not folder.exists():
            self.build(folder)
        script = (folder / overlay.DEFENDER_SETUP).read_bytes().decode('cp866')
        [code] = [re.fullmatch(r'powershell -NoProfile -ExecutionPolicy Bypass -Command "(.*)"', line)[1]
                  for line in script.splitlines() if line.startswith('powershell ') and word in line]
        self.assertNotIn('%', code)
        return code

    @unittest.skipUnless(POWERSHELL, 'PowerShell (pwsh elsewhere than on Windows)')
    def test_the_defender_marker_keeps_every_letter(self):
        """The marker of the exclusion, written by the add's PowerShell and
        read by the undo's, with stand-ins for the Defender cmdlets (no
        exclusion is made): a path's every letter, Ukrainian and Polish too,
        which code page 866 has not; and a marker of the earlier script
        (#453: the path in 866 and a line end) still read. What the undo says
        is Russian on a Russian Windows (SKHID_RU), English on another."""
        add, remove = self.powershell_of('Add-MpPreference'), self.powershell_of('Remove-MpPreference')
        stand_ins = (
            "$log = $env:SKHID_TEST_LOG; $utf8 = [Text.UTF8Encoding]::new($false); "
            "function Get-MpPreference { [CmdletBinding()] param() [pscustomobject]@{ ExclusionPath = @() } }; "
            "function Add-MpPreference { [CmdletBinding()] param([string]$ExclusionPath) "
            "[IO.File]::AppendAllText($log, 'add|' + $ExclusionPath + [char]10, $utf8) }; "
            "function Remove-MpPreference { [CmdletBinding()] param([string]$ExclusionPath) "
            "if ($env:SKHID_TEST_REFUSE) { throw 'refused' }; "
            "[IO.File]::AppendAllText($log, 'remove|' + $ExclusionPath + [char]10, $utf8) }; ")
        marker, log = self.root / 'marker.defender', self.root / 'defender.log'

        def run(code, path, refuse=False, locale='en_US.UTF-8'):
            env = dict(os.environ, SKHID_EXCLUDED=path, SKHID_MARKER=str(marker), SKHID_TEST_LOG=str(log),
                       LANG=locale, LC_ALL=locale)
            env.pop('SKHID_TEST_REFUSE', None)
            if refuse:
                env['SKHID_TEST_REFUSE'] = '1'
            env.pop('SKHID_RU', None)
            if locale.startswith('ru'):
                env['SKHID_RU'] = '1'
            done = subprocess.run([POWERSHELL, '-NoProfile', '-NonInteractive', '-Command', stand_ins + code],
                                  env=env, capture_output=True, timeout=300)
            said = done.stdout.decode('oem' if os.name == 'nt' else 'utf-8', 'replace')
            return done.returncode, said.strip()

        paths = ('C:\\Users\\Віталій\\Ігри & Fun!\\Survarium\\game\\binaries\\x86',
                 'D:\\Gry\\Café Łódź (x86)\\Survarium\\game\\binaries\\x86',
                 'C:\\Игры\\Мои игры! & (x86)\\Survarium\\game\\binaries\\x86')
        for locale in ('ru_RU.UTF-8', 'en_US.UTF-8'):
            for path in paths:
                with self.subTest(locale=locale, path=path):
                    log.unlink(missing_ok=True)
                    marker.unlink(missing_ok=True)
                    self.assertEqual(run(add, path, locale=locale), (0, ''))
                    self.assertEqual(marker.read_bytes(), b'\xef\xbb\xbf' + path.encode())
                    rc, said = run(remove, path, locale=locale)
                    self.assertEqual(rc, 0, said)
                    if os.name != 'nt':
                        self.assertEqual(said, f'Defender: исключение "{path}" удалено.' if locale.startswith('ru')
                                         else f'Defender: the exclusion "{path}" is removed.')
                    self.assertEqual(log.read_text(encoding='utf-8').splitlines(), [f'add|{path}', f'remove|{path}'])
                    # Refused: said, and the marker stays (the script deletes
                    # it only after a removal).
                    rc, said = run(remove, path, refuse=True, locale=locale)
                    self.assertEqual(rc, 1, said)
                    if os.name != 'nt':
                        self.assertEqual(said, f'Defender: исключение "{path}" удалить не удалось.'
                                         if locale.startswith('ru')
                                         else f'Defender: the exclusion "{path}" could not be removed.')
            # The earlier script's marker: code page 866, a line end.
            path = paths[2]
            log.unlink(missing_ok=True)
            marker.write_bytes(path.encode('cp866') + b'\r\n')
            self.assertEqual(run(remove, path, locale=locale)[0], 0)
            self.assertEqual(log.read_text(encoding='utf-8').splitlines(), [f'remove|{path}'])

    @unittest.skipUnless(POWERSHELL, 'PowerShell (pwsh elsewhere than on Windows)')
    def test_the_defender_setup_puts_back_the_files_defender_deleted(self):
        """The restore after the exclusion, by its PowerShell against a server
        here: the missing updater_skhid.exe put back from the ZIP, a file
        that is there left as it is, nothing of the download left (nor of an
        earlier run's, whose ZIP is used again when it is the one announced);
        a file gone again before it is in place said; a ZIP
        that is not the one announced, a staging folder that cannot be
        removed, or no server, changes nothing."""
        import http.server
        import threading
        restore = self.powershell_of('/api/client-files')
        # Defender deleting the staged copy of SKHID_TEST_VANISH just before it
        # is moved: a stand-in for Test-Path, which the move asks first.
        vanish = ("function Test-Path { param([string]$LiteralPath) "
                  "$there = Microsoft.PowerShell.Management\\Test-Path -LiteralPath $LiteralPath; "
                  "if ($env:SKHID_TEST_VANISH -and -not $there -and $f -and $LiteralPath -eq $t "
                  "-and $LiteralPath.EndsWith($env:SKHID_TEST_VANISH) -and [IO.File]::Exists($f)) "
                  "{ [IO.File]::Delete($f) }; $there }; ")
        packed = io.BytesIO()
        with zipfile.ZipFile(packed, 'w') as archive:
            for name in ('Skhid.bat', 'game/binaries/x86/survarium_skhid.exe', 'game/binaries/x86/updater_skhid.exe'):
                archive.writestr(name, 'from the server: ' + name)
        packed = packed.getvalue()
        announced = [hashlib.sha256(packed).hexdigest()]
        downloads = []

        class Site(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                downloads.append(self.path)
                body = {'/api/client-files': f'skhid-client-files 1\nsha256 {announced[0]}\nsize {len(packed)}\n'
                        'min-revision 0\n'.encode(), '/download': packed}.get(self.path, b'')
                self.send_response(200 if body else 404)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *args):
                pass
        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Site)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        top = self.root / 'Мои игры! & (x86)' / 'Survarium'
        x86 = top / 'game' / 'binaries' / 'x86'
        x86.mkdir(parents=True)
        (top / 'Skhid.bat').write_bytes(b"the player's starter")
        (x86 / 'survarium_skhid.exe').write_bytes(b"the player's exe")
        updater = x86 / 'updater_skhid.exe'
        updater_name = os.path.join('game', 'binaries', 'x86', 'updater_skhid.exe')
        stage = x86 / '.skhid-restore'

        def run(ru, site=f'http://127.0.0.1:{server.server_port}/', gone=None):
            env = dict(os.environ, SKHID_TOP=str(top), SKHID_EXCLUDED=str(x86), SKHID_SITE=site)
            for name in ('SKHID_RU', 'SKHID_TEST_VANISH'):
                env.pop(name, None)
            if ru:
                env['SKHID_RU'] = '1'
            if gone:
                env['SKHID_TEST_VANISH'] = gone
            done = subprocess.run([POWERSHELL, '-NoProfile', '-NonInteractive', '-Command', vanish + restore],
                                  env=env, capture_output=True, timeout=300)
            said = done.stdout.decode('oem' if os.name == 'nt' else 'utf-8', 'replace')
            return done.returncode, said.strip().splitlines()

        def files():
            return {str(path.relative_to(top)): path.read_bytes() for path in top.rglob('*') if path.is_file()}

        def nothing_left():
            self.assertEqual(sorted(path.name for path in top.rglob('.skhid-restore*')), [])
        player = files()
        # An earlier run's leftovers, a stale updater among them: gone, and
        # the updater put back is the server's.
        (stage / 'game' / 'binaries' / 'x86').mkdir(parents=True)
        (stage / 'game' / 'binaries' / 'x86' / 'updater_skhid.exe').write_bytes(b'stale')
        (x86 / '.skhid-restore.zip').write_bytes(b'stale')
        self.assertEqual(run(True), (0, ['Восстановлены файлы Skhid:', '  ' + updater_name]))
        self.assertEqual(files(), dict(player, **{updater_name: b'from the server: game/binaries/x86/updater_skhid.exe'}))
        nothing_left()
        self.assertEqual(downloads.count('/download'), 1)
        # A ZIP left that is the one announced: used again, not downloaded.
        updater.unlink()
        (x86 / '.skhid-restore.zip').write_bytes(packed)
        self.assertEqual(run(False), (0, ['Restored Skhid files:', '  ' + updater_name]))
        self.assertEqual(downloads.count('/download'), 1)
        nothing_left()
        self.assertEqual(run(False), (0, ['No Skhid file is missing, nothing to restore.']))
        # Deleted again before it is in place: said, not taken for done.
        updater.unlink()
        self.assertEqual(run(False, gone='updater_skhid.exe'),
                         (1, ['Not restored (Defender may have deleted them again):', '  ' + updater_name]))
        self.assertEqual(files(), player)
        nothing_left()
        # A staging folder left that cannot be removed: nothing downloaded.
        if os.name != 'nt':
            locked = stage / 'locked'
            locked.mkdir(parents=True)
            (locked / 'file').write_bytes(b'stale')
            locked.chmod(0o500)
            try:
                self.assertEqual(run(False), (1, ['Restore: .skhid-restore, left from an earlier run, could not be '
                                                  'removed. Nothing changed.']))
                self.assertFalse(updater.exists())
            finally:
                locked.chmod(0o700)
            shutil.rmtree(stage)
        announced[0] = '0' * 64
        self.assertEqual(run(False), (1, ['Restore: the download is damaged (wrong sha256). Nothing changed.']))
        self.assertEqual(run(True), (1, ['Восстановление: скачанный архив повреждён (не тот sha256). '
                                         'Ничего не изменено.']))
        self.assertEqual(files(), player)
        nothing_left()
        server.shutdown()
        server.server_close()
        site = f'http://127.0.0.1:{server.server_port}'
        self.assertEqual(run(False, site), (1, [f'Restore: could not download the Skhid files from {site}. '
                                                'Nothing changed.']))
        self.assertEqual(files(), player)
        nothing_left()

    @unittest.skipUnless(os.name == 'nt' or WINE, 'Windows, or Wine for its cmd.exe')
    def test_the_log_collection_says_what_it_takes(self):
        """Skhid-collect-logs.cmd --dry-run, by its name from its folder (an
        & and a ! in it, Cyrillic on a Russian Windows), in Russian and in
        English: what it takes and never takes, where it looks, and nothing
        run (no PowerShell but the language's, no elevation). Out of place,
        or with an unknown option, it says so."""
        if os.name == 'nt' or RUSSIAN:
            top, locale = self.root / 'Мои игры! & (x86)' / 'Survarium', 'ru_RU.UTF-8'
        else:
            top, locale = self.root / 'Wow! & (x86)' / 'Survarium', 'en_US.UTF-8'
        self.build(top)
        x86 = str(top / 'game' / 'binaries' / 'x86')
        if os.name != 'nt':
            x86 = 'Z:' + x86.replace('/', '\\')
        top_windows = x86.rsplit('\\game\\', 1)[0]
        dry = [f'[dry-run] logs: Documents\\Survarium, "{x86}", "{top_windows}\\game", "{top_windows}", '
               f'"{top_windows}\\temp"',
               f'[dry-run] crash reports: SRP-crash-*.txt in "{x86}" and the temporary folder',
               '[dry-run] sha256: game\\binaries\\x86 programs, the certificate, game\\resources.db, '
               'game\\resources.db.patch_*',
               "[dry-run] servers: updater_skhid.url's (else https://skhid.link) /api/client-files, /patches, /game "
               'and their hosts',
               '[dry-run] write: Desktop\\Skhid-logs-<date>-<time>.zip']
        for ui in ('ru', 'en'):
            with self.subTest(ui=ui):
                def run(*options):
                    return self.setup_script(top, overlay.COLLECT_LOGS, *options, locale=locale, ui=ui)

                def said(russian, english):
                    return russian if ui in RUSSIAN_UI else english
                lines = run('--dry-run')
                for line in (said('Сбор логов Skhid', 'Skhid log collection'),
                             said('Пароли и сохранённый вход (user.cfg), дампы памяти и другие файлы компьютера',
                                  'It never takes passwords, the saved sign-in (user.cfg), memory dumps or any'),
                             said('Пробный запуск: ничего не собирается, только выводится, что было бы собрано.',
                                  'Dry run: nothing is collected, only what would be is printed.')):
                    self.assertIn(line, lines, '\n'.join(lines))
                folder = said('Папка игры: ', 'Game folder: ')
                self.assertShown([line for line in lines if line.startswith(folder)], [f'{folder}"{top_windows}"'],
                                 locale)
                self.assertShown([line for line in lines if line.startswith('[dry-run]')], dry, locale)
                self.assertIn(said('Неизвестный параметр: --upload', 'Unknown option: --upload'), run('--upload'))
        # Without the game's program folder: where the file belongs.
        shutil.rmtree(top / 'game')
        lost = self.setup_script(top, overlay.COLLECT_LOGS, '--dry-run', locale=locale, ui='en')
        self.assertIn('Put this file beside Skhid.bat, in the folder that holds the folder', lost)
        self.assertFalse([line for line in lost if line.startswith('[dry-run]')])

    @unittest.skipUnless(POWERSHELL and os.name != 'nt',
                         "PowerShell 7 (on Windows it would read the tester's Documents and write on their Desktop)")
    def test_the_log_collection_takes_the_logs_and_no_secret(self):
        """The shipped script's PowerShell, started as the cmd starts it,
        against a game folder here, a Documents and a temporary folder of the
        test's and a server here: one ZIP on the Desktop, nothing of the game
        folder changed; the logs with every credential line (and its record)
        removed and the game's own code page kept; the crash reports' text,
        never a dump, user.cfg or a file beside the game; the sha256 of the
        patches against the server's list, the exe's revision, each server's
        answer and the sign-in's TCP; in Russian and in English."""
        import http.server
        import socket
        import threading
        top = self.root / 'Мои игры! & (x86)' / 'Survarium'
        self.build(top)
        x86 = top / 'game' / 'binaries' / 'x86'
        script = top / overlay.COLLECT_LOGS
        [loader] = [re.fullmatch(r'powershell -NoProfile -ExecutionPolicy Bypass -Command "(.*)"', line)[1]
                    for line in script.read_bytes().decode('cp866').splitlines()
                    if line.startswith('powershell ') and 'skhid-powershell' in line]
        home, temp = self.root / 'home', self.root / 'tmp'
        temp.mkdir()
        # .NET names a special folder only when it is there.
        for name in ('Desktop', 'Documents'):
            (home / name).mkdir(parents=True)
        env = {k: v for k, v in os.environ.items() if not k.startswith(('XDG_', 'SKHID_'))}
        env.update(HOME=str(home), TMPDIR=str(temp), SKHID_SELF=str(script), SKHID_TOP=str(top), LANG='C.UTF-8')
        asked = subprocess.run([POWERSHELL, '-NoProfile', '-NonInteractive', '-Command',
                                "[Environment]::GetFolderPath('MyDocuments'); [Environment]::GetFolderPath('Desktop')"],
                               env=env, capture_output=True, timeout=300, check=True)
        documents, desktop = (Path(line) for line in asked.stdout.decode().splitlines())
        for folder in (documents, desktop):
            self.assertTrue(folder.is_relative_to(home), folder)
        own = documents / 'Survarium'
        own.mkdir(parents=True)

        def record(clock, text):
            return f'core <info>\t[{clock}]\t'.encode() + text + b'\r\n'
        # The game's log: a command line with the password, its record's next
        # line, and a line in the game's code page (cp1251).
        player = (record('12:00:00.000', b'started')
                  + record('12:00:00.001', b'command line : -account_name=player -account_password=hunter2')
                  + b'  hunter3 continued\r\n'
                  + record('12:00:01.000', 'Динамики'.encode('cp1251')))
        (own / 'Survarium_player.log').write_bytes(player)
        (own / 'user.cfg').write_bytes(b'account_digest 0123secretdigest\r\nlocalization russian\r\n')
        (own / 'Survarium.journal').write_bytes(b'journal\0binary')
        (own / 'replays').mkdir()
        (own / 'replays' / 'match.sur').write_bytes(b'replay')
        (own / 'engine.dmp').write_bytes(b'MDMP engine-secret')
        (x86 / 'updater_skhid.log').write_bytes(b'2026-11-02 10:00:00 patches: GET https://user:pa55@files.example/x\n'
                                                b'2026-11-02 10:00:01 ok\n')
        (x86 / 'srp_patches.log').write_bytes(b'pid 7 pickup: refused, no drop to take\r\n')
        (x86 / 'SRP-crash-20261102-1.txt').write_bytes(b'Exception: 0xC0000005\r\n')
        (x86 / 'SRP-crash-20261102-1.dmp').write_bytes(b'MDMP dump-secret')
        (x86 / 'SRP-crash-20261102-1.zip').write_bytes(b'PK dump-secret')
        (temp / 'SRP-crash-20261101-2.txt').write_bytes(b'Exception: 0x80000003\r\n')
        (temp / 'unrelated.log').write_bytes(b'not the game')
        (top / 'skhid-download.log').write_bytes(b'2026-11-02 09:00:00 skhid-game  ok  here: done\r\n')
        (top / 'my-taxes.pdf').write_bytes(b'private')
        # A log over 8 MB: its end, from a whole record.
        big = b''.join(record(f'13:{i // 60 % 60:02}:{i % 60:02}.000', b'x' * 200) for i in range(45000))
        (top / 'game' / 'Survarium_big.log').write_bytes(big)
        (top / 'game' / 'resources.db').write_bytes(b'stock db')
        (top / 'game' / 'resources.db.patch_1').write_bytes(b'one')
        (top / 'game' / 'resources.db.patch_2').write_bytes(b'two, damaged')
        # An exe whose revision site says 42 (client/updater/logic/src/revision.rs).
        exe = bytearray(0x1400)
        exe[:2] = b'MZ'
        struct.pack_into('<I', exe, 0x3C, 0x40)
        exe[0x40:0x44] = b'PE\0\0'
        struct.pack_into('<HH', exe, 0x44, 0x14C, 1)
        struct.pack_into('<H', exe, 0x54, 0xE0)
        struct.pack_into('<HI', exe, 0x58, 0x10B, 0)
        struct.pack_into('<I', exe, 0x58 + 28, 0x10000)
        struct.pack_into('<4I', exe, 0x138 + 8, 0x1000, 0x370000, 0x1000, 0x400)
        exe[0xC62:0xC68] = bytes([0xC6, 0x44, 0x24, 0x27, 42, 0x90])
        (x86 / 'survarium_skhid.exe').write_bytes(exe)

        with socket.socket() as closed:
            closed.bind(('127.0.0.1', 0))
            nowhere = closed.getsockname()[1]
        asked_for = []

        class Site(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                asked_for.append(self.path)
                port = self.server.server_port
                # Another host than the site's (localhost), and one that refuses.
                patches = {f'patch_{n}': {'sha256': sha, 'urls': [f'http://localhost:{port}/p{n}',
                                                                   f'http://127.0.0.1:{nowhere}/p{n}']}
                           for n, sha in ((1, hashlib.sha256(b'one').hexdigest()), (2, hashlib.sha256(b'two').hexdigest()),
                                          (3, '3' * 64))}
                body = {'/api/client-files': b'skhid-client-files 1\nsha256 ' + b'a' * 64 + b'\n',
                        '/patches': json.dumps(patches).encode(),
                        '/game': json.dumps({'sha256': 'b' * 64, 'size': 1,
                                             'urls': [f'http://localhost:{port}/game.7z']}).encode(),
                        '/p1': b'one', '/game.7z': b'7z'}.get(self.path, b'')
                self.send_response(200 if body else 404)
                self.send_header('Content-Type', 'text/plain')
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *args):
                pass
        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Site)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        port = server.server_port
        (x86 / 'updater_skhid.url').write_bytes(f'http://127.0.0.1:{port}\r\n'.encode())
        bat = x86 / 'play_skhid.bat'
        bat.write_bytes(re.sub(rb'embedded in the exe: \S+', f'embedded in the exe: 127.0.0.1:{port}'.encode(),
                               bat.read_bytes()))

        def tree():
            return {str(path.relative_to(top)): path.read_bytes() if path.is_file() else None
                    for path in top.rglob('*')}
        before = tree()
        for ru in (True, False):
            with self.subTest(ru=ru):
                for zip_path in desktop.glob('Skhid-logs-*.zip'):
                    zip_path.unlink()
                run_env = dict(env)
                if ru:
                    run_env['SKHID_RU'] = '1'
                done = subprocess.run([POWERSHELL, '-NoProfile', '-NonInteractive', '-Command', loader],
                                      env=run_env, capture_output=True, timeout=600)
                said = done.stdout.decode('utf-8', 'replace').splitlines()
                self.assertEqual(done.returncode, 0, '\n'.join(said) + done.stderr.decode('utf-8', 'replace'))
                self.assertEqual(tree(), before)
                [made] = list(desktop.glob('Skhid-logs-*'))
                self.assertTrue(made.name.endswith('.zip'), made)
                if ru:
                    self.assertIn('Готово. Файл с логами:', said)
                    self.assertIn('Отправьте его нам в Discord: https://discord.gg/QUmJg3gBhg', said)
                else:
                    self.assertIn('Done. The file with the logs:', said)
                    self.assertIn('Send it to us on Discord: https://discord.gg/QUmJg3gBhg', said)
                self.assertIn('  ' + str(made), said)
                with zipfile.ZipFile(made) as archive:
                    entries = {name: archive.read(name) for name in archive.namelist()}
                self.assertEqual(set(entries), {
                    'report.txt', 'folder.txt', 'logs/documents/Survarium_player.log',
                    'logs/game-x86/updater_skhid.log', 'logs/game-x86/srp_patches.log',
                    'logs/game/Survarium_big.log', 'logs/game-folder/skhid-download.log',
                    'crash/game-x86/SRP-crash-20261102-1.txt', 'crash/temp/SRP-crash-20261101-2.txt',
                    'state/game-x86/updater_skhid.url', 'state/game-x86/play_skhid.bat'})
                for secret in (b'hunter2', b'hunter3', b'pa55', b'secretdigest', b'engine-secret', b'dump-secret',
                               b'my-taxes', b'match.sur', b'not the game', b'user.cfg'):
                    for name, data in entries.items():
                        if name != 'report.txt' or secret != b'user.cfg':
                            self.assertNotIn(secret, data, name)
                removed = b'[Skhid: sensitive log record removed]\r\n'
                self.assertEqual(entries['logs/documents/Survarium_player.log'],
                                 record('12:00:00.000', b'started') + removed
                                 + record('12:00:01.000', 'Динамики'.encode('cp1251')))
                self.assertEqual(entries['logs/game-x86/updater_skhid.log'],
                                 removed + b'2026-11-02 10:00:01 ok\n')
                self.assertEqual(entries['logs/game-x86/srp_patches.log'], (x86 / 'srp_patches.log').read_bytes())
                tail = entries['logs/game/Survarium_big.log']
                self.assertLessEqual(len(tail), 8 * 1024 * 1024)
                self.assertTrue(big.endswith(tail) and tail.startswith(b'core <info>\t['), tail[:80])
                report = entries['report.txt'].decode('utf-8-sig').replace('\\', '/')
                for line in (f'  the updater asks: http://127.0.0.1:{port}',
                             f'  GET http://127.0.0.1:{port}/api/client-files: HTTP 200, text/plain',
                             f'  GET http://localhost:{port}/p1: HTTP 200, text/plain',
                             f'  TCP 127.0.0.1:{port} (sign-in): connected',
                             '  game/resources.db.patch_3: in the list, MISSING here',
                             '  not taken: game-x86/SRP-crash-20261102-1.dmp',
                             '  not taken: game-x86/SRP-crash-20261102-1.zip',
                             '  not taken: documents/engine.dmp',
                             'logs/documents/Survarium.journal'):
                    self.assertIn(line, report, report)
                self.assertRegex(report, rf'  GET http://127\.0\.0\.1:{nowhere}/p1: (?!HTTP)')
                self.assertRegex(report, r'game/binaries/x86/survarium_skhid\.exe  5120 bytes, [^\n]*patch revision 42')
                self.assertRegex(report, r'game/resources\.db\.patch_1  3 bytes, [^\n]*, matches the list')
                self.assertRegex(report, r'game/resources\.db\.patch_2  12 bytes, [^\n]*, DIFFERS from the list')
                self.assertRegex(report, r'logs/documents/Survarium\.journal  \d+ bytes, [^\n]*, not taken: not text')
                self.assertRegex(report, r'logs/documents/Survarium_player\.log  \d+ bytes, [^\n]*, 2 lines removed')
                listing = entries['folder.txt'].decode('utf-8-sig').replace('\\', '/')
                for name in ('game/binaries/x86/survarium_skhid.exe', 'game/resources.db.patch_2', 'Skhid.bat',
                             overlay.COLLECT_LOGS, '(1 other files and folders beside game, not listed)'):
                    self.assertIn(name, listing)
        # Each host once a run, by its first address: the game's, on the
        # patches' host, was not asked again.
        self.assertEqual(asked_for.count('/p1'), 2)
        self.assertNotIn('/p2', asked_for)
        self.assertNotIn('/game.7z', asked_for)

    @unittest.skipUnless(WINE and os.name != 'nt', "Wine (on Windows, a real run would change its firewall)")
    def test_the_setup_scripts_ask_for_administrator_rights(self):
        """The real run's first step, with stand-ins for fltmc (administrator
        rights or not) and for the PowerShell that asks for them (the UAC
        prompt answered Yes or No): what the player is told, and that nothing
        else happens until the rights are there. In a Cyrillic folder with a
        ! and an & on a Russian Windows (without its locale, Wine's English
        one and a folder it can print)."""
        if RUSSIAN:
            top, locale = self.root / 'Мои игры! & (x86)' / 'Survarium', 'ru_RU.UTF-8'
        else:
            top, locale = self.root / 'Wow! & (x86)' / 'Survarium', 'en_US.UTF-8'
        self.build(top)
        stubs = self.root / 'stubs'

        def run(script, fltmc, powershell, *options):
            shutil.rmtree(stubs, ignore_errors=True)
            stubs.mkdir()
            exit_program(stubs / 'fltmc.exe', fltmc)
            # PowerShell also says the display language (the elevation's
            # stand-in prints it too: a line no check looks for).
            exit_program(stubs / 'powershell.exe', powershell, ui)
            env = self.windows_env(locale)
            env['WINEPATH'] = 'Z:' + str(stubs).replace('/', '\\')
            return self.cmd(top, script, *options, env=env)

        # Each script: the line of what needs the rights, the end of its "say Yes"
        # line, its own lines (the work done), and those of a run with the
        # rights (the stand-in PowerShell fails: Defender's exclusion is not
        # added, and the firewall's script goes on without it); in Russian
        # and in English.
        for ui in ('ru', 'en'):
            if ui == 'ru':
                firewall = [f'Брандмауэр: {program} разрешены исходящие соединения (TCP и UDP)' for program in PROGRAMS]
                scripts = (
                    (overlay.NETWORK_SETUP, 'Для правил брандмауэра нужны права администратора. Сейчас Windows',
                     'настроил брандмауэр.', 'Брандмауэр:',
                     firewall + ['Готово. Чтобы убрать всё это: Skhid-network-setup.cmd --undo']),
                    (overlay.DEFENDER_SETUP, 'Для исключения в Defender нужны права администратора. Сейчас Windows',
                     'добавил исключение.', 'Defender:',
                     ['Defender: исключение добавить не удалось. Возможно, защитой управляет']))
                asks_tail = ['спросит разрешение (контроль учётных записей): нажмите "Да". Тогда этот',
                             'файл запустится ещё раз, в новом окне.']
                declined_head = ['Ничего не изменено: Windows не дала прав администратора.',
                                 'Запустите этот файл ещё раз и в окне Windows нажмите "Да", чтобы он']
                unchanged, press = 'Ничего не изменено', 'Нажмите любую клавишу, чтобы закрыть окно.'
                not_admin = 'Ничего не изменено: у этого окна нет прав администратора.'
            else:
                firewall = [f'Firewall: {program} is allowed outgoing connections (TCP and UDP)' for program in PROGRAMS]
                scripts = (
                    (overlay.NETWORK_SETUP, 'The firewall rules need administrator rights. Windows will now ask',
                     'sets up the firewall.', 'Firewall:',
                     firewall + ['Done. To remove all this: Skhid-network-setup.cmd --undo']),
                    (overlay.DEFENDER_SETUP, 'The Defender exclusion needs administrator rights. Windows will now ask',
                     'adds the exclusion.', 'Defender:',
                     ['Defender: the exclusion could not be added. Perhaps another antivirus']))
                asks_tail = ['for permission (User Account Control): click "Yes". Then this file',
                             'will start again, in a new window.']
                declined_head = ['Nothing changed: Windows gave no administrator rights.',
                                 'Start this file again and click "Yes" in the Windows prompt, so that it']
                unchanged, press = 'Nothing changed', 'Press any key to close the window.'
                not_admin = 'Nothing changed: this window has no administrator rights.'
            for script, need, why, prefix, admin_lines in scripts:
                with self.subTest(script=script, ui=ui):
                    asks = [need, *asks_tail]
                    declined = [*declined_head, why]

                    def work(lines):
                        return [line for line in lines if line.startswith(prefix)]

                    # No rights, Yes at the prompt: the elevated copy (a new
                    # window) does the work; this one ends at once, with nothing
                    # done and no pause. A negative exit code of fltmc is no
                    # rights either (an "if errorlevel 1" would take it for them).
                    for no_rights in (1, -5):
                        accepted = run(script, no_rights, 0)
                        self.assertEqual(accepted[accepted.index(asks[0]):][:3], asks, '\n'.join(accepted))
                        self.assertFalse(work(accepted), '\n'.join(accepted))
                        self.assertFalse([line for line in accepted if unchanged in line or press in line])
                    # No at the prompt: nothing changed, run it again and say Yes.
                    refused = run(script, 1, 1)
                    self.assertEqual(refused[refused.index(asks[0]):][:3], asks, '\n'.join(refused))
                    self.assertEqual(refused[refused.index(declined[0]):][:3], declined, '\n'.join(refused))
                    self.assertFalse(work(refused))
                    self.assertIn(press, refused)
                    # The elevated copy without the rights after all: it asks no more.
                    again = run(script, 1, 0, '--elevated')
                    self.assertIn(not_admin, again)
                    self.assertNotIn(asks[0], again)
                    self.assertFalse(work(again))
                    # With the rights: no question, the work (Wine's netsh may
                    # refuse the rules; the script goes on) and the rest.
                    admin = run(script, 0, 1)
                    self.assertNotIn(asks[0], admin)
                    for line in admin_lines:
                        self.assertIn(line, admin, '\n'.join(admin))
                    self.assertEqual(work(admin), [line for line in admin_lines if line.startswith(prefix)],
                                     '\n'.join(admin))

    def console(self, folder, env, command):
        """What Wine's console shows on a terminal for cmd /d /c `command` in
        `folder` (its text, without the terminal's escapes)."""
        import pty
        import termios
        import fcntl
        pid, fd = pty.fork()
        if pid == 0:
            try:
                fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', 50, 250, 0, 0))
                os.chdir(folder)
                os.execve(WINE, [WINE, 'cmd', '/d', '/c', command], env)
            finally:
                os._exit(127)
        seen, deadline = b'', time.monotonic() + 180
        escapes = re.compile(r'\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b[()][0-9A-Za-z]|\x1b[=>]|\r')
        try:
            while time.monotonic() < deadline:
                if select.select([fd], [], [], 0.5)[0]:
                    try:
                        data = os.read(fd, 4096)
                    except OSError:
                        break
                    if not data:
                        break
                    seen += data
            else:
                os.kill(pid, 9)
        finally:
            os.close(fd)
            os.waitpid(pid, 0)
        self.assertLess(time.monotonic(), deadline, repr(seen))
        return escapes.sub('', seen.decode('utf-8', 'replace'))

    @unittest.skipUnless(WINE and os.name != 'nt', "Wine's console on a terminal")
    def test_the_setup_scripts_speak_the_players_language_in_a_console(self):
        """What a player reads in the window, on a Russian Windows (code page
        866) and on an English one (437): Russian, not the letters of another
        code page, or English, by the display language; and the console's own
        code page back at the end. (The Russian Windows needs the ru_RU.UTF-8
        locale here.)"""
        top = self.root / 'Мои игры' / 'Survarium'
        self.build(top)
        x86 = 'Z:' + str(top / 'game' / 'binaries' / 'x86').replace('/', '\\')
        top_windows = x86.rsplit(chr(92) + "game", 1)[0]
        windows = [('en_US.UTF-8', '437')]
        if RUSSIAN:
            windows.insert(0, ('ru_RU.UTF-8', '866'))
        said = {
            'ru': ((overlay.NETWORK_SETUP, ('Настройка сети Skhid',
                                            'Добавляет правила брандмауэра Windows, которые разрешают трём программам',
                                            'Брандмауэр остаётся включённым.', f'Папка игры: "{top_windows}"',
                                            'Брандмауэр: survarium_skhid.exe разрешены исходящие соединения (TCP и UDP)',
                                            'Готово. Чтобы убрать всё это: Skhid-network-setup.cmd --undo')),
                   (overlay.DEFENDER_SETUP, ('Исключение Skhid в Microsoft Defender',
                                             'Microsoft Defender может по ошибке принять изменённые файлы игры',
                                             f'  "{x86}"', 'Defender остаётся включённым.',
                                             f'Папка игры: "{top_windows}"',
                                             f'[dry-run] powershell Add-MpPreference -ExclusionPath "{x86}"',
                                             'Готово. Чтобы убрать исключение: Skhid-defender-setup.cmd --undo'))),
            'en': ((overlay.NETWORK_SETUP, ('Skhid network setup',
                                            "Adds Windows Firewall rules that allow the game's three programs in",
                                            'The firewall stays on.', f'Game folder: "{top_windows}"',
                                            'Firewall: survarium_skhid.exe is allowed outgoing connections (TCP and UDP)',
                                            'Done. To remove all this: Skhid-network-setup.cmd --undo')),
                   (overlay.DEFENDER_SETUP, ('Skhid exclusion in Microsoft Defender',
                                             "Microsoft Defender may mistake the game's changed files",
                                             f'  "{x86}"', 'Defender stays on.', f'Game folder: "{top_windows}"',
                                             f'[dry-run] powershell Add-MpPreference -ExclusionPath "{x86}"',
                                             'Done. To remove the exclusion: Skhid-defender-setup.cmd --undo'))),
        }
        for (locale, own), ui in ((window, ui) for window in windows for ui in said):
            env = self.windows_env(locale, ui)
            for script, lines in said[ui]:
                with self.subTest(locale=locale, script=script, ui=ui):
                    seen = self.console(top, env, script + ' --dry-run & chcp')
                    for line in lines:
                        self.assertIn(line, seen, seen)
                    pages = re.findall(r'(?:Active code page|Активная кодовая страница): (\d+)', seen)
                    self.assertEqual(pages, [own], seen)


def exit_program(path, code, said=''):
    """A Windows console program that writes the line `said` (if any) to its
    standard output, ends with exit code `code` and does nothing else: a
    32-bit PE whose one section holds `WriteFile(GetStdHandle(-11), ...);
    ExitProcess(code)` and the imports of the three from kernel32."""
    section_rva, base = 0x1000, 0x400000
    descriptor, lookup, address, hints, dll, written, text = 0x40, 0x80, 0x90, 0xA0, 0xE0, 0xF0, 0x100
    section = bytearray(0x200)
    line = (said + '\r\n').encode('ascii') if said else b''
    assert -128 <= code < 128 and len(line) < 128  # push imm8, sign-extended

    def at(offset):
        return (base + section_rva + offset).to_bytes(4, 'little')
    # The import address table: ExitProcess, GetStdHandle, WriteFile.
    section[:37] = (bytes([0x6A, 0xF5, 0xFF, 0x15]) + at(address + 4)  # GetStdHandle(STD_OUTPUT_HANDLE)
                    + bytes([0x6A, 0, 0x68]) + at(written) + bytes([0x6A, len(line), 0x68]) + at(text)
                    + bytes([0x50, 0xFF, 0x15]) + at(address + 8)  # WriteFile(it, text, len, &written, 0)
                    + bytes([0x6A, code & 0xFF, 0xFF, 0x15]) + at(address))  # ExitProcess(code)
    struct.pack_into('<5I', section, descriptor, section_rva + lookup, 0, 0, section_rva + dll,
                     section_rva + address)
    for i, name in enumerate((b'ExitProcess', b'GetStdHandle', b'WriteFile')):
        struct.pack_into('<I', section, lookup + 4 * i, section_rva + hints + 0x10 * i)
        struct.pack_into('<I', section, address + 4 * i, section_rva + hints + 0x10 * i)
        section[hints + 0x10 * i:hints + 0x10 * i + len(name) + 3] = b'\0\0' + name + b'\0'
    section[dll:dll + 13] = b'KERNEL32.dll\0'
    section[text:text + len(line)] = line
    headers = bytearray(0x200)
    headers[:2] = b'MZ'
    struct.pack_into('<I', headers, 0x3C, 0x40)
    headers[0x40:0x44] = b'PE\0\0'
    # The file header: i386, one section, the optional header's size, an
    # executable without relocations.
    struct.pack_into('<HHIIIHH', headers, 0x44, 0x14C, 1, 0, 0, 0, 0xE0, 0x0103)
    optional = 0x58
    struct.pack_into('<HBB9I6H4I2H6I', headers, optional,
                     0x10B, 0, 0, 0x200, 0, 0, section_rva, section_rva, section_rva,
                     base, 0x1000, 0x200, 4, 0, 0, 0, 4, 0, 0, 0x2000, 0x200, 0, 3, 0,
                     0x100000, 0x1000, 0x100000, 0x1000, 0, 16)
    struct.pack_into('<II', headers, optional + 96 + 8, section_rva + descriptor, 40)
    struct.pack_into('<8sIIIIIIHHI', headers, optional + 0xE0, b'.text', 0x200, section_rva, 0x200, 0x200,
                     0, 0, 0, 0, 0xE0000060)
    path.write_bytes(bytes(headers + section))


if __name__ == '__main__':
    unittest.main()
