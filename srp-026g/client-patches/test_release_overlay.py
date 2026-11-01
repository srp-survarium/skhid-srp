"""Check player-folder/ZIP contents without distributing or needing a game binary."""
import contextlib
import hashlib
import io
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
            'VPN.txt', 'Skhid-network-setup.cmd', 'Skhid-defender-setup.cmd',
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
        for name in (overlay.NETWORK_SETUP, overlay.DEFENDER_SETUP):
            script = (folder / name).read_bytes()
            # Code page 866, switched to before the first Russian line: all
            # above it is ASCII, read in whatever code page the console has.
            switch = script.index(b'\r\nchcp 866 >nul 2>&1\r\n')
            script[:switch].decode('ascii')
            self.assertIsNone(re.search(rb'(?im)^\s*echo', script[:switch]))
            text = texts[name] = script.decode('cp866')
            self.assertEqual(text.encode('cp866'), script)
            self.assertEqual(script.count(b'\n'), script.count(b'\r\n'))
            # Every line a player reads is Russian (the owner, 2026-10-26: only
            # Russians use it): an echo is Russian, or only names and paths, or
            # a --dry-run command.
            for line in text.splitlines():
                said = re.match(r'\s*(?:if [^(]* )?echo[ (.](.*)', line, re.I)
                if said and not said[1].startswith('[dry-run]') and re.search(r'[A-Za-z]{4,} [a-z]{2,}', said[1]):
                    self.assertRegex(said[1], '[А-Яа-яЁё]', line)
            # The elevation: the normal UAC prompt, by Start-Process -Verb RunAs
            # of cmd.exe and the command line a dry run prints.
            self.assertIn('Start-Process -FilePath $env:ComSpec -ArgumentList $env:SKHID_CMDLINE -Verb RunAs', text)
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
        self.assertIn('VPN.txt', (folder / 'LINUX.txt').read_text(encoding='utf-8'))

    def windows_env(self, locale='en_US.UTF-8'):
        """The environment of a Windows program here: this one, or Wine's in a
        prefix of its own (no window is opened). Under Wine, `locale` is the
        Windows': ru_RU.UTF-8 is a Russian one (code page 866), en_US.UTF-8
        an English one (437)."""
        if os.name == 'nt':
            return dict(os.environ)
        prefix = self.root / ('wineprefix-' + locale.split('.')[0])
        prefix.mkdir(exist_ok=True)
        env = {k: v for k, v in os.environ.items()
               if k not in ('DISPLAY', 'WAYLAND_DISPLAY', 'LANGUAGE') and not k.startswith('LC_')}
        env.update(WINEPREFIX=str(prefix), WINEDEBUG='-all', LANG=locale, LC_ALL=locale)
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

    def setup_script(self, top, name, *options, locale='en_US.UTF-8'):
        """The script `name` of the overlay in `top`, run by its name from its
        folder (it finds the folder itself, %~dp0)."""
        return self.cmd(top, name, *options, env=self.windows_env(locale))

    def elevated(self, line, locale='en_US.UTF-8'):
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
        env = self.windows_env(locale)
        env['SKHID_TEST_CMDLINE'] = line[len(prefix):]
        return self.cmd(self.root, 'elevate.cmd', env=env)

    @unittest.skipUnless(os.name == 'nt' or WINE, 'Windows, or Wine for its cmd.exe')
    def test_the_setup_scripts_add_and_remove_exactly_their_changes(self):
        # Folders with a space and brackets, with an &, with a !, in
        # Cyrillic, Ukrainian (і, which code page 866 has not) and Polish, as
        # a player's may be; under Wine on a Russian Windows and on an
        # English one.
        locales = ['en_US.UTF-8']
        if os.name != 'nt' and RUSSIAN:
            locales.insert(0, 'ru_RU.UTF-8')
        for locale in locales:
            for name in ('My Games (x86)', 'Tom & Jerry (x86)', 'Wow! Games (x86)', 'Мои игры! & (x86)',
                         'Віталій (x86)', 'Café Łódź (x86)'):
                with self.subTest(locale=locale, folder=name):
                    self.setup_scripts_in(self.root / locale / name / 'Survarium', locale)

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

    def setup_scripts_in(self, top, locale):
        self.build(top)
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
            return self.setup_script(top, overlay.NETWORK_SETUP, *options, locale=locale)

        def defender(*options):
            return self.setup_script(top, overlay.DEFENDER_SETUP, *options, locale=locale)

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
        self.assertIn('Настройка сети Skhid', added)
        shown([line for line in added if line.startswith('Папка игры:')], [f'Папка игры: "{top_windows}"'])
        for program in PROGRAMS:
            self.assertIn(f'Брандмауэр: {program} разрешены исходящие соединения (TCP и UDP)', added)
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
            elevated = self.elevated(elevate[0], locale)
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
            elevated = self.elevated([line for line in undone if 'elevate:' in line][0], locale)
            self.assertEqual(commands(elevated), commands(undone), '\n'.join(elevated))

        # The Defender exclusion: game\binaries\x86 alone, never the folder
        # above it.
        added = defender('--dry-run')
        self.assertIn('Исключение Skhid в Microsoft Defender', added)
        shown([line for line in added if line.startswith('Папка игры:')], [f'Папка игры: "{top_windows}"'])
        expected = [f'[dry-run] powershell Add-MpPreference -ExclusionPath "{x86}"',
                    f'[dry-run] write "{top_windows}\\Skhid-network-setup.defender":']
        shown(commands(added), expected)
        # What the marker would hold: the path as it is, by the line that
        # writes it.
        writes = next(i for i, line in enumerate(added) if line.startswith('[dry-run] write '))
        shown(added[writes + 1:writes + 2], [x86])
        elevate = [line for line in added if 'elevate:' in line]
        shown(elevate, [f'[dry-run] elevate: cmd.exe /d /c ""{top_windows}\\Skhid-defender-setup.cmd" '
                        '--elevated --dry-run"'])
        if faithful:
            elevated = self.elevated(elevate[0], locale)
            shown(commands(elevated), expected)
            self.assertFalse([line for line in elevated if 'elevate:' in line])
        self.assertEqual(defender('--dry-run'), added)
        # --undo: no exclusion this script (or the network setup before it)
        # did not add.
        undone = defender('--dry-run', '--undo')
        self.assertFalse(commands(undone), '\n'.join(undone))
        self.assertIn('Defender: этот файл не добавлял здесь исключений, удалять нечего.', undone)
        shown([line for line in undone if 'elevate:' in line],
              ['[dry-run] elevate: cmd.exe /d /c ""' + top_windows + '\\Skhid-defender-setup.cmd" '
               '--elevated --undo --dry-run"'])
        # With the marker: the exclusion it names is removed (PowerShell
        # reads it: test_the_defender_marker_keeps_every_letter), in the
        # elevated run too.
        marker = top / 'Skhid-network-setup.defender'
        pointer = 'Defender: исключение папки игры удаляет Skhid-defender-setup.cmd --undo'
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
            elevated = self.elevated([line for line in undone if 'elevate:' in line][0], locale)
            self.assertEqual(commands(elevated), commands(undone), '\n'.join(elevated))
        marker.unlink()

        # Not beside the game: each says where it belongs and changes nothing.
        (top / 'game' / 'binaries' / 'x86' / 'survarium_skhid.exe').unlink()
        for run in (setup, defender):
            lost = run('--dry-run')
            self.assertIn('Не найден survarium_skhid.exe в папке', lost)
            self.assertFalse([line for line in lost if line.startswith('[dry-run] ')])
            self.assertIn('Неизвестный параметр: --firewall-off', run('--firewall-off'))
        self.assertIn('Неизвестный параметр: --defender-yes', setup('--defender-yes'))

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
        (#453: the path in 866 and a line end) still read."""
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
                        self.assertEqual(said, f'Defender: исключение "{path}" удалено.')
                    self.assertEqual(log.read_text(encoding='utf-8').splitlines(), [f'add|{path}', f'remove|{path}'])
                    # Refused: said, and the marker stays (the script deletes
                    # it only after a removal).
                    rc, said = run(remove, path, refuse=True, locale=locale)
                    self.assertEqual(rc, 1, said)
                    if os.name != 'nt':
                        self.assertEqual(said, f'Defender: исключение "{path}" удалить не удалось.')
            # The earlier script's marker: code page 866, a line end.
            path = paths[2]
            log.unlink(missing_ok=True)
            marker.write_bytes(path.encode('cp866') + b'\r\n')
            self.assertEqual(run(remove, path, locale=locale)[0], 0)
            self.assertEqual(log.read_text(encoding='utf-8').splitlines(), [f'remove|{path}'])

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
            exit_program(stubs / 'powershell.exe', powershell)
            env = self.windows_env(locale)
            env['WINEPATH'] = 'Z:' + str(stubs).replace('/', '\\')
            return self.cmd(top, script, *options, env=env)

        # Each script: what it needs the rights for, the end of its "say Yes"
        # line, its own lines (the work done), and those of a run with the
        # rights (the stand-in PowerShell fails: Defender's exclusion is not
        # added, and the firewall's script goes on without it).
        firewall = [f'Брандмауэр: {program} разрешены исходящие соединения (TCP и UDP)' for program in PROGRAMS]
        for script, need, why, prefix, admin_lines in (
                (overlay.NETWORK_SETUP, 'Для правил брандмауэра', 'настроил брандмауэр.', 'Брандмауэр:',
                 firewall + ['Готово. Чтобы убрать всё это: Skhid-network-setup.cmd --undo']),
                (overlay.DEFENDER_SETUP, 'Для исключения в Defender', 'добавил исключение.', 'Defender:',
                 ['Defender: исключение добавить не удалось. Возможно, защитой управляет'])):
            with self.subTest(script=script):
                asks = [need + ' нужны права администратора. Сейчас Windows',
                        'спросит разрешение (контроль учётных записей): нажмите "Да". Тогда этот',
                        'файл запустится ещё раз, в новом окне.']
                declined = ['Ничего не изменено: Windows не дала прав администратора.',
                            'Запустите этот файл ещё раз и в окне Windows нажмите "Да", чтобы он', why]

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
                    self.assertFalse([line for line in accepted if 'Ничего не изменено' in line or 'Нажмите' in line])
                # No at the prompt: nothing changed, run it again and say Yes.
                refused = run(script, 1, 1)
                self.assertEqual(refused[refused.index(asks[0]):][:3], asks, '\n'.join(refused))
                self.assertEqual(refused[refused.index(declined[0]):][:3], declined, '\n'.join(refused))
                self.assertFalse(work(refused))
                self.assertIn('Нажмите любую клавишу, чтобы закрыть окно.', refused)
                # The elevated copy without the rights after all: it asks no more.
                again = run(script, 1, 0, '--elevated')
                self.assertIn('Ничего не изменено: у этого окна нет прав администратора.', again)
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
    def test_the_setup_scripts_speak_russian_in_a_console(self):
        """What a player reads in the window, on a Russian Windows (code page
        866) and on an English one (437): Russian, not the letters of another
        code page; and the console's own code page back at the end. (The
        Russian Windows needs the ru_RU.UTF-8 locale here.)"""
        top = self.root / 'Мои игры' / 'Survarium'
        self.build(top)
        x86 = 'Z:' + str(top / 'game' / 'binaries' / 'x86').replace('/', '\\')
        folder = f'Папка игры: "{x86.rsplit(chr(92) + "game", 1)[0]}"'
        windows = [('en_US.UTF-8', '437')]
        if RUSSIAN:
            windows.insert(0, ('ru_RU.UTF-8', '866'))
        for locale, own in windows:
            env = self.windows_env(locale)
            for script, lines in (
                    (overlay.NETWORK_SETUP, ('Настройка сети Skhid',
                                             'Добавляет правила брандмауэра Windows, которые разрешают трём программам',
                                             'Брандмауэр остаётся включённым.', folder,
                                             'Брандмауэр: survarium_skhid.exe разрешены исходящие соединения (TCP и UDP)',
                                             'Готово. Чтобы убрать всё это: Skhid-network-setup.cmd --undo')),
                    (overlay.DEFENDER_SETUP, ('Исключение Skhid в Microsoft Defender',
                                              'Microsoft Defender может по ошибке принять изменённые файлы игры',
                                              f'  "{x86}"', 'Defender остаётся включённым.', folder,
                                              f'[dry-run] powershell Add-MpPreference -ExclusionPath "{x86}"',
                                              'Готово. Чтобы убрать исключение: Skhid-defender-setup.cmd --undo'))):
                with self.subTest(locale=locale, script=script):
                    seen = self.console(top, env, script + ' --dry-run & chcp')
                    for line in lines:
                        self.assertIn(line, seen, seen)
                    pages = re.findall(r'(?:Active code page|Активная кодовая страница): (\d+)', seen)
                    self.assertEqual(pages, [own], seen)


def exit_program(path, code):
    """A Windows console program that ends with exit code `code` and does
    nothing else: a 32-bit PE whose one section holds `push code; call
    [ExitProcess]` and the import of ExitProcess from kernel32."""
    section_rva, base = 0x1000, 0x400000
    descriptor, lookup, address, hint, dll = 0x40, 0x80, 0x90, 0xA0, 0xC0
    section = bytearray(0x200)
    assert -128 <= code < 128  # push imm8, sign-extended
    section[:8] = bytes([0x6A, code & 0xFF, 0xFF, 0x15]) + (base + section_rva + address).to_bytes(4, 'little')
    struct.pack_into('<5I', section, descriptor, section_rva + lookup, 0, 0, section_rva + dll,
                     section_rva + address)
    struct.pack_into('<I', section, lookup, section_rva + hint)
    struct.pack_into('<I', section, address, section_rva + hint)
    section[hint:hint + 14] = b'\0\0ExitProcess\0'
    section[dll:dll + 13] = b'KERNEL32.dll\0'
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
