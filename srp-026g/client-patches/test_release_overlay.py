"""Check player-folder/ZIP contents without distributing or needing a game binary."""
import contextlib
import hashlib
import io
import os
from pathlib import Path
import shutil
import stat
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
            'game/binaries/x86/updater_skhid.url', 'Skhid.bat', 'Skhid.sh',
            'VPN.txt', 'Skhid-network-setup.cmd',
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
        script = (folder / 'Skhid-network-setup.cmd').read_bytes()
        script.decode('ascii')
        self.assertEqual(script.count(b'\n'), script.count(b'\r\n'))
        self.assertIn(b'set "PROGRAMS=' + b' '.join(p.encode() for p in PROGRAMS) + b'"', script)
        # Nothing turned off: no firewall state, no Defender setting but the exclusion.
        for word in (b'state off', b'opmode', b'Set-MpPreference', b'DisableRealtimeMonitoring'):
            self.assertNotIn(word.lower(), script.lower())
        readme = (folder / 'README.txt').read_text(encoding='utf-8')
        self.assertIn('Skhid-network-setup.cmd', readme)
        self.assertIn('VPN.txt', readme)
        self.assertIn('VPN.txt', (folder / 'LINUX.txt').read_text(encoding='utf-8'))

    def windows_env(self):
        """The environment of a Windows program here: this one, or Wine's in a
        prefix of its own (no window is opened)."""
        if os.name == 'nt':
            return dict(os.environ)
        prefix = self.root / 'wineprefix'
        prefix.mkdir(exist_ok=True)
        env = {k: v for k, v in os.environ.items() if k not in ('DISPLAY', 'WAYLAND_DISPLAY')}
        env.update(WINEPREFIX=str(prefix), WINEDEBUG='-all')
        return env

    def cmd(self, folder, *arguments, env=None):
        """cmd.exe (elsewhere Wine's) run in `folder` with `arguments`: what it
        printed, its lines."""
        command = (['cmd.exe'] if os.name == 'nt' else [WINE, 'cmd']) + ['/d', '/c', *arguments]
        done = subprocess.run(command, cwd=folder, env=env or self.windows_env(), stdin=subprocess.DEVNULL,
                              capture_output=True, timeout=600)
        if os.environ.get('SKHID_TEST_DEBUG'):
            print(command, done.returncode, done.stdout, done.stderr)
        return done.stdout.decode('ascii', 'replace').replace('\r\n', '\n').splitlines()

    def network_setup(self, top, *options):
        """Skhid-network-setup.cmd of the overlay in `top`, run by its name
        from its folder (it finds the folder itself, %~dp0)."""
        return self.cmd(top, 'Skhid-network-setup.cmd', *options)

    def elevated(self, line):
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
        env = self.windows_env()
        env['SKHID_TEST_CMDLINE'] = line[len(prefix):]
        return self.cmd(self.root, 'elevate.cmd', env=env)

    @unittest.skipUnless(os.name == 'nt' or WINE, 'Windows, or Wine for its cmd.exe')
    def test_the_network_setup_adds_and_removes_exactly_its_rules(self):
        # Folders with a space and brackets, with an &, with a !, as a
        # player's may be.
        for name in ('My Games (x86)', 'Tom & Jerry (x86)', 'Wow! Games (x86)'):
            with self.subTest(name):
                self.network_setup_in(self.root / name / 'Survarium')

    def network_setup_in(self, top):
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

        added = self.network_setup(top, '--dry-run', '--defender-yes')
        expected = []
        for program, protocol in rules:
            expected += [delete(program, protocol),
                         f'[dry-run] netsh advfirewall firewall add rule name="Skhid {program} {protocol} out" '
                         f'dir=out action=allow program="{x86}\\{program}" protocol={protocol} enable=yes '
                         f'profile=any']
        # The exclusion: game\binaries\x86 alone, never the folder above it.
        expected += ['[dry-run] look for firewall rules that block the three programs',
                     f'[dry-run] powershell Add-MpPreference -ExclusionPath "{x86}"',
                     f'[dry-run] write "{top_windows}\\Skhid-network-setup.defender":']
        self.assertEqual(commands(added), expected, '\n'.join(added))
        # What the marker would hold: the path as it is, by the line that
        # writes it.
        self.assertEqual(added[added.index(expected[-1]) + 1], x86, '\n'.join(added))
        self.assertNotIn(' dir=in ', '\n'.join(added))
        # The command line the elevation starts cmd.exe with, and that run.
        elevate = [line for line in added if 'elevate:' in line]
        self.assertEqual(elevate, [f'[dry-run] elevate: cmd.exe /d /c ""{top_windows}\\Skhid-network-setup.cmd" '
                                   '--elevated --defender-yes --dry-run"'], '\n'.join(added))
        elevated = self.elevated(elevate[0])
        self.assertEqual(commands(elevated), expected, '\n'.join(elevated))
        self.assertFalse([line for line in elevated if 'elevate:' in line])
        # Every rule is removed by its name before it is added: a second run
        # adds none twice.
        self.assertEqual(self.network_setup(top, '--dry-run', '--defender-yes'), added)
        # No Defender exclusion asked for: none added.
        refused = self.network_setup(top, '--dry-run', '--defender-no')
        self.assertIn('Defender: no exclusion added.', refused)
        self.assertFalse([line for line in refused if 'MpPreference' in line])
        # --undo: the same six rules, and no exclusion this script did not add.
        undone = self.network_setup(top, '--dry-run', '--undo')
        self.assertEqual(commands(undone), [delete(*rule) for rule in rules], '\n'.join(undone))
        self.assertIn('Defender: this script added no exclusion here: nothing to remove.', undone)
        self.assertIn('[dry-run] elevate: cmd.exe /d /c ""' + top_windows + '\\Skhid-network-setup.cmd" '
                      '--elevated --undo --dry-run"', undone)
        # The marker as the script writes it (the path, a line end): the
        # exclusion it names is the one removed, an & in the path and all,
        # in the elevated run too.
        (top / 'Skhid-network-setup.defender').write_bytes(x86.encode() + b'\r\n')
        undone = self.network_setup(top, '--dry-run', '--undo')
        removal = [f'[dry-run] powershell Remove-MpPreference -ExclusionPath "{x86}"',
                   f'[dry-run] delete "{top_windows}\\Skhid-network-setup.defender"']
        self.assertEqual(commands(undone)[-2:], removal, '\n'.join(undone))
        elevated = self.elevated([line for line in undone if 'elevate:' in line][0])
        self.assertEqual(commands(elevated), commands(undone), '\n'.join(elevated))
        # Not beside the game: it says where it belongs and changes nothing.
        (top / 'game' / 'binaries' / 'x86' / 'survarium_skhid.exe').unlink()
        lost = self.network_setup(top, '--dry-run')
        self.assertIn('survarium_skhid.exe was not found in', lost)
        self.assertFalse([line for line in lost if line.startswith('[dry-run] ')])
        self.assertIn('Unknown option: --firewall-off', self.network_setup(top, '--firewall-off'))

if __name__ == '__main__':
    unittest.main()
