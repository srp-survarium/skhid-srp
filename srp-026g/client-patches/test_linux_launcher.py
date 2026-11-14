#!/usr/bin/env python3
"""Exercise the release launcher with mocked Wine and a stand-in game executable."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pwd
import shutil
import subprocess
import sys
import tempfile
import unittest

import make_client_026g as overlay

PROBE = r'C:\windows\syswow64\cmd.exe'
SCRIPT = Path(__file__).resolve().parent / 'play_skhid.sh'
STARTER = Path(__file__).resolve().parent / 'Skhid.sh'
FIXTURE = b'MZ stand-in game executable'
GECKO_MSI = b'Gecko installer fixture'
GECKO_SHA = '26cecc47706b091908f7f814bddb074c61beb8063318e9efc5a7f789857793d6'
VIEWER_EXE = b'PowerPoint Viewer fixture'
VIEWER_SHA = 'c4e753548d3092ffd7dd3849105e0a26d9b5a1afe46e6e667fe7c6887893701f'
CALIBRI_FACES = ('calibri', 'calibrib', 'calibrii', 'calibriz')


class LauncherTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='skhid-bash-')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / "game with spaces & apostrophe's"
        self.bin.mkdir()
        self.script = self.bin / 'play_skhid.sh'
        self.drm = self.root / 'drm'
        self.drm.mkdir()
        self.ntsync = self.root / 'ntsync'
        self.script.write_text(SCRIPT.read_text().replace('/sys/class/drm', str(self.drm))
            .replace('/dev/ntsync', str(self.ntsync))
            .replace(GECKO_SHA, hashlib.sha256(GECKO_MSI).hexdigest())
            .replace(VIEWER_SHA, hashlib.sha256(VIEWER_EXE).hexdigest()))
        self.config = self.bin / 'wineprefix/drive_c/users' / pwd.getpwuid(os.getuid()).pw_name / 'Documents/Survarium/user.cfg'
        self.tools = self.root / 'tools'
        self.tools.mkdir()
        self.calls = self.root / 'calls.jsonl'
        # Only mocked dependencies plus explicitly allowed core tools in PATH.
        for tool in ('bash', 'dirname', 'flock', 'timeout', 'rm', 'mkdir', 'touch', 'id', 'awk', 'grep', 'mktemp', 'sha256sum', 'ln', 'cp', 'mv', 'tr'):
            (self.tools / tool).symlink_to(shutil.which(tool))
        stub = '#!' + sys.executable + '\n' + '''import json,os,pathlib,sys
name=pathlib.Path(sys.argv[0]).name
prefix=pathlib.Path(os.environ['WINEPREFIX'])
gecko=prefix/'drive_c/windows/syswow64/gecko/2.47.4/wine_gecko'
def install_gecko():
 gecko.mkdir(parents=True,exist_ok=True)
 (gecko/'VERSION').write_text('Wine Gecko 2.47.4')
 (gecko/'xul.dll').write_bytes(b'Gecko runtime')
if name=='wine' and sys.argv[1]=='wineboot.exe':
 prefix.mkdir(parents=True,exist_ok=True)
with open(os.environ['TEST_CALLS'],'a') as f:
 f.write(json.dumps(dict(name=name,args=sys.argv[1:],cwd=os.getcwd(),prefix=str(prefix),dll=os.environ.get('WINEDLLOVERRIDES'),arch=os.environ.get('WINEARCH'),crash_dump=os.environ.get('SKHID_CRASH_DUMP'))) + '\\n')
if name=='wine' and sys.argv[1]=='survarium_skhid.exe' and os.environ.get('TEST_CRASH'):
 pathlib.Path('SRP-crash-test-0.dmp').write_bytes(b'MDMP')
 pathlib.Path('SRP-crash-test-0.txt').write_text('Reporter:  local build, full-memory dumps')
if os.environ.get('TEST_FAIL')==name+':'+sys.argv[1]:
 sys.exit(1)
if name=='wine' and sys.argv[1]=='--version':
 print(os.environ.get('TEST_WINE_VERSION','wine-11.8 (Staging)'))
 sys.exit(0)
if os.environ.get('WINEARCH')!='wow64':
 sys.exit('modern WoW64 was not forced')
if name=='wineserver':
 # Delayed registry creation ensures the script waits for wineserver.
 (prefix/'system.reg').write_text('WINE REGISTRY Version 2\\n#arch='+os.environ.get('TEST_PREFIX_ARCH','win64')+'\\n')
 mshtml=prefix/'drive_c/windows/syswow64/mshtml.dll'
 mshtml.parent.mkdir(parents=True,exist_ok=True)
 mshtml.write_bytes(b'MZ Wine Gecko '+os.environ.get('TEST_GECKO_VERSION','2.47.4').encode())
 (prefix/'drive_c/windows/Fonts').mkdir(parents=True,exist_ok=True)
 if os.environ.get('TEST_GECKO_READY','1')=='1': install_gecko()
elif name=='curl':
 output=pathlib.Path(sys.argv[sys.argv.index('--output')+1])
 if any('PowerPointViewer' in argument for argument in sys.argv[1:]):
  output.write_bytes(b'corrupt' if os.environ.get('TEST_VIEWER_CORRUPT') else b'PowerPoint Viewer fixture')
 else:
  output.write_bytes(b'corrupt' if os.environ.get('TEST_GECKO_CORRUPT') else b'Gecko installer fixture')
elif name=='cabextract':
 if os.environ.get('TEST_CALIBRI_FAIL')=='1':
  sys.exit(1)
 folder=pathlib.Path(sys.argv[sys.argv.index('-d')+1])
 wanted=sys.argv[sys.argv.index('-F')+1]
 if wanted=='ppviewer.cab':
  (folder/wanted).write_bytes(b'cabinet fixture')
 elif wanted=='CALIBRI*.TTF':
  for face in ('CALIBRI','CALIBRIB','CALIBRII','CALIBRIZ'):
   (folder/(face+'.TTF')).write_bytes(face.encode())
elif name=='wine' and sys.argv[1]=='msiexec':
 if not os.environ.get('TEST_GECKO_INCOMPLETE'): install_gecko()
elif name=='winetricks' and os.environ.get('TEST_DXVK_FAIL')=='1':
 sys.exit(1)
'''
        for name in ('wine', 'wineserver', 'winetricks', 'curl', 'cabextract'):
            path = self.tools / name
            path.write_text(stub)
            path.chmod(0o755)
        self.env = dict(os.environ, PATH=str(self.tools), TEST_CALLS=str(self.calls))
        self.env['HOME'] = str(self.root / 'home')
        for name in ('XDG_CONFIG_HOME', 'NIRI_SOCKET', 'WAYLAND_DISPLAY'):
            self.env.pop(name, None)
        self.shared_config = self.root / 'home/.config/skhid/user.cfg'
        self.env['XDG_SESSION_TYPE'] = 'x11'

    def display(self, name, modes, status='connected', enabled='enabled'):
        output = self.drm / name
        output.mkdir()
        for key, value in dict(status=status, enabled=enabled, modes=modes).items():
            (output / key).write_text(value + '\n')

    def xrandr(self, output, exit_code=0):
        tool = self.tools / 'xrandr'
        tool.write_text('#!' + sys.executable + '\nimport sys\nprint(' + repr(output) + ')\nsys.exit(' + str(exit_code) + ')\n')
        tool.chmod(0o755)

    def modinfo(self, exit_code):
        tool = self.tools / 'modinfo'
        tool.write_text('#!' + sys.executable + '\nimport sys\nsys.exit(' + str(exit_code) + ')\n')
        tool.chmod(0o755)

    def copy_exe(self):
        self.exe = self.bin / 'survarium_skhid.exe'
        self.exe.write_bytes(FIXTURE)

    def run_script(self, *arguments):
        return subprocess.run([str(self.tools / 'bash'), str(self.script), *arguments],
            cwd=self.root, env=self.env, text=True, capture_output=True)

    @staticmethod
    def installs_calibri(call):
        return call['name'] == 'cabextract' or any('PowerPointViewer' in a or a.endswith('calibri.reg') for a in call['args'])

    def recorded_calls(self, compatibility=False, calibri=False):
        calls = [json.loads(line) for line in self.calls.read_text().splitlines()]
        if not calibri:  # the font's own steps have their own tests below
            calls = [c for c in calls if not self.installs_calibri(c)]
        # Existing tests assert setup/game behavior; compatibility cases below
        # explicitly inspect version queries and the mandatory 32-bit probe.
        if compatibility:
            return calls
        return [c for c in calls if c['args'][0] not in ('--version', PROBE)]

    def test_unsupported_or_unrecognized_wine_cannot_change_exe_or_prefix(self):
        self.copy_exe()
        for version in ('wine-9.0', 'wine-10.20 (Staging)', 'unknown', 'wine-110', ''):
            with self.subTest(version=version):
                self.env['TEST_WINE_VERSION'] = version
                self.calls.unlink(missing_ok=True)
                result = self.run_script()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('Wine 11.0 or newer', result.stderr)
                self.assertEqual(self.exe.read_bytes(), FIXTURE)
                self.assertFalse((self.bin/'wineprefix').exists())
                self.assertEqual([c['args'] for c in self.recorded_calls(True)], [['--version']])

    def test_failed_version_query_is_refused(self):
        self.copy_exe()
        self.env['TEST_FAIL'] = 'wine:--version'
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Cannot query Wine', result.stderr)
        self.assertEqual(self.exe.read_bytes(), FIXTURE)
        self.assertFalse((self.bin/'wineprefix').exists())

    def test_modern_wow64_is_forced_and_probed_on_every_start(self):
        self.copy_exe()
        self.env['WINEARCH'] = 'win32'
        for version in ('wine-11.0', 'wine-11.8 (Staging)', 'wine-12.0.1'):
            with self.subTest(version=version):
                self.env['TEST_WINE_VERSION'] = version
                self.calls.unlink(missing_ok=True)
                result = self.run_script()
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                calls = self.recorded_calls(True)
                self.assertEqual(calls[0]['args'], ['--version'])
                self.assertTrue(all(c['arch'] == 'wow64' for c in calls))
                probes = [i for i,c in enumerate(calls) if c['args'][0] == PROBE]
                self.assertEqual(len(probes), 1)
                self.assertEqual(calls[probes[0]]['args'], [PROBE, '/d', '/c', 'exit 0'])
                self.assertLess(probes[0], len(calls)-1)
                self.assertTrue(all(c['name'] != 'winetricks' for c in calls[:probes[0]]))
                self.assertEqual(calls[-1]['args'][0], 'survarium_skhid.exe')

    def test_old_32_bit_or_unknown_prefix_is_preserved_and_refused(self):
        self.copy_exe()
        prefix = self.bin/'wineprefix'
        prefix.mkdir()
        for architecture in ('win32', 'unknown'):
            registry = 'WINE REGISTRY Version 2\n#arch='+architecture+'\n'
            (prefix/'system.reg').write_text(registry)
            (prefix/'.skhid-ready').touch()
            (prefix/'.skhid-dxvk').touch()
            self.calls.unlink(missing_ok=True)
            result = self.run_script()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('unused backup name', result.stderr)
            self.assertEqual((prefix/'system.reg').read_text(), registry)
            self.assertEqual(self.exe.read_bytes(), FIXTURE)
            self.assertEqual([c['args'] for c in self.recorded_calls(True)], [['--version']])

    def test_failed_wow64_probe_never_falls_back_or_installs_dxvk(self):
        self.copy_exe()
        self.env['TEST_FAIL'] = 'wine:'+PROBE
        for _ in range(2):
            self.calls.unlink(missing_ok=True)
            result = self.run_script()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Modern WoW64 could not run', result.stderr)
            self.assertEqual(self.exe.read_bytes(), FIXTURE)
            self.assertFalse((self.bin/'wineprefix/.skhid-dxvk').exists())
            calls = self.recorded_calls(True)
            self.assertEqual(calls[-1]['args'][0], PROBE)
            self.assertTrue(all(c['arch'] == 'wow64' for c in calls))
            self.assertFalse(any(c['name'] == 'winetricks' for c in calls))
        del self.env['TEST_FAIL']
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_new_prefix_must_also_be_64_bit(self):
        self.copy_exe()
        self.env['TEST_PREFIX_ARCH'] = 'win32'
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('not a supported 64-bit prefix', result.stderr)
        self.assertEqual(self.exe.read_bytes(), FIXTURE)
        self.assertFalse((self.bin/'wineprefix/.skhid-ready').exists())
        self.assertFalse(any(c['name'] == 'winetricks' for c in self.recorded_calls(True)))

    def test_missing_available_ntsync_stops_before_wine_setup_and_allows_retry(self):
        self.launcher_files()
        self.modinfo(0)
        for arguments in ((), ('launcher',)):
            with self.subTest(arguments=arguments):
                self.calls.unlink(missing_ok=True)
                result = self.run_script(*arguments)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('sudo modprobe ntsync', result.stderr)
                self.assertIn('run this script again', result.stderr)
                self.assertEqual([c['args'] for c in self.recorded_calls(True)], [['--version']])
                self.assertFalse((self.bin / 'wineprefix').exists())
                self.assertFalse(self.shared_config.exists())
        self.ntsync.write_text('')
        result = self.run_script('launcher')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.recorded_calls()[-1]['args'][0], 'launcher_skhid.exe')

    def test_ntsync_does_not_block_when_loaded_or_unavailable(self):
        self.copy_exe()
        for case, module, device in (('the device is there', 0, True),
                                     ('the kernel has no module', 1, False),
                                     ('modinfo is not installed', None, False)):
            with self.subTest(case=case):
                (self.tools / 'modinfo').unlink(missing_ok=True)
                if module is not None:
                    self.modinfo(module)
                if device:
                    self.ntsync.write_text('')
                else:
                    self.ntsync.unlink(missing_ok=True)
                result = self.run_script()
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertNotIn('ntsync', result.stderr)

    def test_ntsync_is_not_checked_for_a_refused_wine(self):
        self.copy_exe()
        self.modinfo(0)
        self.env['TEST_WINE_VERSION'] = 'wine-9.0'
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn('ntsync', result.stderr)

    def test_wayland_driver_is_chosen_only_under_niri(self):
        self.copy_exe()
        key = 'HKCU\\Software\\Wine\\Drivers'
        reg_add = ['reg', 'add', key, '/v', 'Graphics', '/d', 'wayland,x11', '/f']
        reg_delete = ['reg', 'delete', key, '/v', 'Graphics', '/f']
        user_reg = self.bin / 'wineprefix/user.reg'
        # (niri session, value already in the prefix) -> the one registry call expected
        for niri, already, expected in ((True, False, reg_add), (True, True, None),
                                        (False, True, reg_delete), (False, False, None)):
            with self.subTest(niri=niri, already=already):
                self.modinfo(1)
                self.env['WAYLAND_DISPLAY'] = 'wayland-1' if niri else ''
                self.env['NIRI_SOCKET'] = '/run/user/1000/niri.sock' if niri else ''
                self.calls.unlink(missing_ok=True)
                user_reg.parent.mkdir(exist_ok=True)
                user_reg.write_text('"Graphics"="wayland,x11"\n' if already else '')
                result = self.run_script()
                self.assertEqual(result.returncode, 0, result.stderr)
                args = [c['args'] for c in self.recorded_calls()]
                self.assertEqual([a for a in args if a[0] == 'reg'], [expected] if expected else [])
                if expected:
                    self.assertLess(args.index(expected), args.index(['survarium_skhid.exe', '-minimum_resources_memory_size=768']))

    def test_launcher_mode_starts_the_skhid_launcher_in_the_same_wine(self):
        self.copy_exe()
        for name in ('launcher_skhid.exe', 'updater_skhid.exe'):
            (self.bin / name).write_bytes(b'MZ fixture')
        result = self.run_script('launcher')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.recorded_calls()
        # No game arguments: the launcher passes the language and the rest.
        self.assertEqual(calls[-1]['args'], ['launcher_skhid.exe'])
        self.assertEqual(calls[-1]['cwd'], str(self.bin))
        self.assertIn('d3d11,dxgi=n,b', calls[-1]['dll'])
        self.assertNotIn(['survarium_skhid.exe'], [c['args'][:1] for c in calls])
        self.env['TEST_FAIL'] = 'wine:launcher_skhid.exe'
        result = self.run_script('launcher')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Launcher exited with an error', result.stderr)

    def launcher_files(self):
        self.copy_exe()
        for name in ('launcher_skhid.exe', 'updater_skhid.exe'):
            (self.bin / name).write_bytes(b'MZ fixture')

    def test_gecko_is_installed_quietly_once_and_html_stays_enabled(self):
        self.launcher_files()
        self.env['TEST_GECKO_READY'] = '0'
        result = self.run_script('launcher')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.recorded_calls(True)
        download = next(c for c in calls if c['name'] == 'curl')
        self.assertEqual(download['args'][-1], 'https://dl.winehq.org/wine/wine-gecko/2.47.4/wine-gecko-2.47.4-x86.msi')
        install = next(c for c in calls if c['args'][0] == 'msiexec')
        self.assertEqual(install['args'][1], '/i')
        self.assertEqual(install['args'][-2:], ['/qn', '/norestart'])
        self.assertIn('winedbg.exe=d', install['dll'])
        self.assertTrue(all('mshtml=b' in c['dll'] and 'appwiz.cpl=d' in c['dll'] for c in calls))
        self.assertLess(calls.index(install), len(calls)-1)
        self.assertEqual(calls[-1]['args'], ['launcher_skhid.exe'])
        self.assertFalse(list((self.bin/'wineprefix/drive_c').glob('skhid-gecko.*')))
        (self.tools/'curl').unlink()
        result = self.run_script('launcher')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(sum(c['args'][0] == 'msiexec' for c in self.recorded_calls(True)), 1)

    def test_direct_game_does_not_download_gecko(self):
        self.copy_exe()
        self.env['TEST_GECKO_READY'] = '0'
        (self.tools/'curl').unlink()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertFalse(any(c['args'][0] == 'msiexec' for c in self.recorded_calls(True)))

    def test_failed_gecko_download_or_install_never_opens_launcher_and_retries(self):
        self.launcher_files()
        self.env['TEST_GECKO_READY'] = '0'
        for failure in ('curl:--fail', 'wine:msiexec'):
            with self.subTest(failure=failure):
                self.env['TEST_FAIL'] = failure
                self.calls.unlink(missing_ok=True)
                result = self.run_script('launcher')
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('Run again to retry', result.stderr)
                self.assertFalse(any(c['args'][0] == 'launcher_skhid.exe' for c in self.recorded_calls(True)))
                self.assertFalse(list((self.bin/'wineprefix/drive_c').glob('skhid-gecko.*')))
        del self.env['TEST_FAIL']
        result = self.run_script('launcher')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_corrupt_gecko_download_is_not_executed(self):
        self.launcher_files()
        self.env.update(TEST_GECKO_READY='0', TEST_GECKO_CORRUPT='1')
        result = self.run_script('launcher')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('checksum failed', result.stderr)
        self.assertFalse(any(c['args'][0] in ('msiexec', 'launcher_skhid.exe') for c in self.recorded_calls(True)))
        self.assertFalse(list((self.bin/'wineprefix/drive_c').glob('skhid-gecko.*')))

    def test_incomplete_gecko_install_is_not_cached(self):
        self.launcher_files()
        self.env.update(TEST_GECKO_READY='0', TEST_GECKO_INCOMPLETE='1')
        result = self.run_script('launcher')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('installation is incomplete', result.stderr)
        del self.env['TEST_GECKO_INCOMPLETE']
        result = self.run_script('launcher')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(sum(c['args'][0] == 'msiexec' for c in self.recorded_calls(True)), 2)

    def test_unrecognized_gecko_requirement_does_not_download_wrong_installer(self):
        self.launcher_files()
        self.env['TEST_GECKO_VERSION'] = '9.9.9'
        result = self.run_script('launcher')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('another Gecko version', result.stderr)
        self.assertFalse(any(c['name'] == 'curl' or c['args'][0] == 'launcher_skhid.exe'
                             for c in self.recorded_calls(True)))

    def test_the_starter_beside_game_runs_the_script_with_its_arguments(self):
        # Skhid.sh in the game's folder, the script where the archive puts it.
        install = self.root / "install with spaces & apostrophe's"
        nested = install / 'game/binaries/x86'
        nested.mkdir(parents=True)
        starter = install / 'Skhid.sh'
        starter.write_text(STARTER.read_text())
        # Unpacked in the wrong place: it says where it belongs, runs nothing.
        result = subprocess.run([str(self.tools / 'bash'), str(starter)],
            cwd=self.root, env=self.env, text=True, capture_output=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn('play_skhid.sh was not found', result.stderr)
        self.assertIn('the one that holds the', result.stderr)
        self.assertFalse(self.calls.exists())
        # In its place: the script runs in its own folder, whatever the
        # caller's folder, and gets the arguments.
        (nested / 'play_skhid.sh').write_text(self.script.read_text())
        (nested / 'survarium_skhid.exe').write_bytes(FIXTURE)
        for name in ('launcher_skhid.exe', 'updater_skhid.exe'):
            (nested / name).write_bytes(b'MZ fixture')
        for arguments, started in (((), 'survarium_skhid.exe'), (('launcher',), 'launcher_skhid.exe')):
            result = subprocess.run([str(self.tools / 'bash'), str(starter), *arguments],
                cwd=self.root, env=self.env, text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            last = self.recorded_calls()[-1]
            self.assertEqual(last['args'][0], started)
            self.assertEqual(last['cwd'], str(nested))
        result = subprocess.run([str(self.tools / 'bash'), str(starter), 'other'],
            cwd=self.root, env=self.env, text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Usage', result.stderr)

    def test_launcher_mode_needs_the_launcher_and_its_updater(self):
        self.copy_exe()
        (self.bin / 'launcher_skhid.exe').write_bytes(b'MZ fixture')
        result = self.run_script('launcher')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('updater_skhid.exe', result.stderr)
        self.assertFalse(self.calls.exists())
        self.assertEqual(self.exe.read_bytes(), FIXTURE)

    def test_other_arguments_are_refused(self):
        self.copy_exe()
        for arguments in (('game',), ('launcher', 'more'), ('',  'x')):
            result = self.run_script(*arguments)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Usage', result.stderr)
        self.assertFalse(self.calls.exists())

    def test_missing_dependencies_fail_before_changes(self):
        self.copy_exe()
        for name in ('wine', 'wineserver'):
            tool = self.tools / name
            tool.rename(tool.with_suffix('.disabled'))
            result = self.run_script()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(name.lower(), result.stderr.lower())
            self.assertFalse(self.calls.exists())
            self.assertFalse((self.bin / "wineprefix").exists())
            tool.with_suffix('.disabled').rename(tool)

    def test_winetricks_is_only_required_to_install_dxvk(self):
        self.copy_exe()
        tool = self.tools / 'winetricks'
        disabled = tool.with_suffix('.disabled')
        tool.rename(disabled)
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Install winetricks first', result.stderr)
        self.assertFalse((self.bin / 'wineprefix/.skhid-dxvk').exists())
        self.assertFalse(any(c['args'][0] == 'survarium_skhid.exe'
                             for c in self.recorded_calls()))
        disabled.rename(tool)
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        tool.rename(disabled)
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual([c['args'] for c in self.recorded_calls() if c['name'] == 'winetricks'],
            [['-q', 'dxvk']])
        # A prefix that has DXVK but lost Calibri gets it back without winetricks.
        (self.bin / 'wineprefix/drive_c/windows/Fonts/calibri.ttf').unlink()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue((self.bin / 'wineprefix/drive_c/windows/Fonts/calibri.ttf').exists())

    def test_missing_exe(self):
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('beside', result.stderr)

    def test_generated_full_launcher_uses_embedded_server_and_keeps_exe(self):
        self.copy_exe()
        self.script.write_bytes(overlay.sh('release.example:2234', True))
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.recorded_calls()
        self.assertEqual(calls[-1]['args'], ['survarium_skhid.exe',
            '-minimum_resources_memory_size=768'])
        self.assertEqual(self.exe.read_bytes(), FIXTURE)
        self.assertFalse(self.exe.with_name(self.exe.name + '.before-memory-patch').exists())

    def test_a_db_the_updater_finds_not_updated_keeps_the_game_from_starting(self):
        self.copy_exe()
        self.script.write_bytes(overlay.sh('release.example:2234', True))
        (self.bin / 'updater_skhid.exe').write_bytes(b'MZ updater')
        self.env['TEST_FAIL'] = 'wine:updater_skhid.exe'
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("has not finished updating the game's files", result.stderr)
        self.assertIn('bash Skhid.sh launcher', result.stderr)
        calls = self.recorded_calls()
        self.assertEqual(calls[-1]['args'], ['updater_skhid.exe', 'ready', 'quiet'])
        self.assertFalse(any(call['args'][:1] == ['survarium_skhid.exe'] for call in calls))
        # Ready: the game starts after the check.
        self.env.pop('TEST_FAIL')
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.recorded_calls()
        self.assertEqual([call['args'][:2] for call in calls[-2:]],
                         [['updater_skhid.exe', 'ready'], ['survarium_skhid.exe', '-minimum_resources_memory_size=768']])

    def test_generated_cert_only_launcher_passes_configured_server(self):
        self.copy_exe()
        self.script.write_bytes(overlay.sh('release.example:2234', False))
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.recorded_calls()
        self.assertEqual(calls[-1]['args'], ['survarium_skhid.exe',
            '-minimum_resources_memory_size=768', '-server=release.example:2234'])

    def test_full_launcher_allows_an_explicit_server_override(self):
        self.copy_exe()
        script = overlay.sh('release.example:2234', True)
        self.script.write_bytes(script.replace(b'server=\n', b'server=local.example:4321\n'))
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.recorded_calls()
        self.assertIn('-server=local.example:4321', calls[-1]['args'])

    def test_settings_survive_prefix_recreation(self):
        self.copy_exe()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.config.is_symlink())
        self.assertEqual(self.config.resolve(), self.shared_config)
        custom = b'r_fullscreen off\r\nsnd_volume 0.3\r\n'
        self.config.write_bytes(custom)
        shutil.rmtree(self.bin / 'wineprefix')
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.config.read_bytes(), custom)
        self.assertTrue(self.config.is_symlink())

    def test_xdg_config_home_override_and_invalid_relative_path(self):
        self.copy_exe()
        override = self.root / "settings with spaces & apostrophe's"
        self.env['XDG_CONFIG_HOME'] = str(override)
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.config.resolve(), override / 'skhid/user.cfg')
        self.assertFalse(self.shared_config.exists())
        self.env['XDG_CONFIG_HOME'] = 'relative/path'
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.config.resolve(), self.shared_config)
        self.assertFalse((self.bin / 'relative').exists())

    def test_invalid_shared_settings_stop_before_starting_game(self):
        self.copy_exe()
        self.shared_config.mkdir(parents=True)
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Settings file is not writable', result.stderr)
        self.assertFalse(any(c['args'][0] == 'survarium_skhid.exe' for c in self.recorded_calls()))

    def test_resolution_falls_back_to_1080p(self):
        self.copy_exe()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.config.read_bytes(), b'r_fullscreen on\r\nr_resolution 1920x1080\r\n')

    def test_x11_primary_preferred_resolution_and_preserved_settings(self):
        self.copy_exe()
        self.xrandr('''Screen 0: current 3200 x 1080
HDMI-1 connected 1920x1080+0+0
   1920x1080 60.00*+
DP-1 connected primary 1280x720+1920+0
   2560x1440 144.00+
   1280x720 60.00*
DP-2 connected (normal left inverted right)
   3840x2160 60.00+
''')
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('r_resolution 2560x1440', self.config.read_text())
        custom = b'r_fullscreen off\r\nr_resolution 1600x900\r\nsnd_volume 0.5\r\n'
        self.config.write_bytes(custom)
        stamp = self.config.stat().st_mtime_ns
        # Recovery of Wine setup must preserve existing game preferences too.
        (self.bin / 'wineprefix/.skhid-ready').unlink()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.config.read_bytes(), custom)
        self.assertEqual(self.config.stat().st_mtime_ns, stamp)

    def test_x11_without_primary_uses_first_active_preferred_mode(self):
        self.copy_exe()
        self.xrandr('''DP-1 connected (normal left inverted right)
   3840x2160 60.00+
HDMI-1 connected 1366x768+0+0
   1366x768 60.00*+
''')
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('r_resolution 1366x768', self.config.read_text())

    def test_wayland_uses_physical_preferred_mode(self):
        self.copy_exe()
        self.env['XDG_SESSION_TYPE'] = 'wayland'
        self.xrandr('XWAYLAND0 connected primary 1280x720+0+0\n   1280x720 60.00*+')
        self.display('card0-DP-1', '3840x2160', status='disconnected')
        self.display('card0-DP-2', '3840x2160', enabled='disabled')
        self.display('card0-HDMI-A-1', '2560x1440\n3840x2160\n1920x1080')
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('r_resolution 2560x1440', self.config.read_text())

    def test_failed_xrandr_uses_physical_display(self):
        self.copy_exe()
        self.xrandr('Cannot open display', exit_code=1)
        self.display('card0-HDMI-A-1', '1920x1200')
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('r_resolution 1920x1200', self.config.read_text())

    def test_invalid_display_mode_falls_back(self):
        self.copy_exe()
        self.display('card0-HDMI-A-1', 'invalid')
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('r_resolution 1920x1080', self.config.read_text())

    def test_first_run_and_reuse_leave_executable_untouched(self):
        self.copy_exe()
        before = self.exe.read_bytes()
        stamp = self.exe.stat().st_mtime_ns
        self.exe.chmod(0o444)
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.exe.read_bytes(), before)
        self.assertEqual(self.exe.stat().st_mtime_ns, stamp)
        self.assertFalse(list(self.bin.glob('*.before-memory-patch*')))
        calls = self.recorded_calls()
        self.assertEqual([c['name'] for c in calls],
            ['wine', 'wineserver', 'winetricks', 'wine'])
        self.assertEqual(calls[2]['args'], ['-q', 'dxvk'])
        self.assertEqual(calls[3]['cwd'], str(self.bin))
        # No language set: none passed, the game keeps the one chosen in its Settings.
        self.assertEqual(calls[3]['args'], ['survarium_skhid.exe',
            '-minimum_resources_memory_size=768'])
        self.assertIn('qwave=', calls[3]['dll'])
        self.assertEqual(calls[3]['prefix'], str(self.bin / 'wineprefix'))
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(len(self.recorded_calls()), 5)
        self.assertEqual(self.exe.read_bytes(), before)
        self.assertEqual(self.exe.stat().st_mtime_ns, stamp)

    def test_dxvk_failure_retries_without_launching(self):
        self.copy_exe()
        self.env['TEST_DXVK_FAIL'] = '1'
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('DXVK installation failed', result.stderr)
        self.assertFalse((self.bin / 'wineprefix/.skhid-dxvk').exists())
        self.assertEqual(len(self.recorded_calls()), 3)
        del self.env['TEST_DXVK_FAIL']
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.recorded_calls()
        self.assertEqual([c['name'] for c in calls[-2:]], ['winetricks', 'wine'])

    GAME = ['survarium_skhid.exe', '-minimum_resources_memory_size=768']

    def assert_calibri_warned_and_game_started(self, result):
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('Calibri could not be installed', result.stderr)
        self.assertIn('item levels', result.stderr)
        self.assertIn('The next start tries again', result.stderr)
        self.assertFalse((self.bin / 'wineprefix/drive_c/windows/Fonts/calibri.ttf').exists())
        self.assertEqual(self.recorded_calls()[-1]['args'], self.GAME)

    def assert_calibri_installed(self, result):
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn('Warning', result.stderr)
        self.assertEqual(self.recorded_calls()[-1]['args'], self.GAME)
        fonts = self.bin / 'wineprefix/drive_c/windows/Fonts'
        self.assertEqual(sorted(p.name for p in fonts.iterdir()), [face + '.ttf' for face in CALIBRI_FACES])
        self.assertFalse(list((self.bin / 'wineprefix/drive_c').glob('skhid-calibri.*')))

    def test_calibri_is_installed_from_the_viewer_without_winetricks(self):
        self.copy_exe()
        self.assert_calibri_installed(self.run_script())
        steps = [c for c in self.recorded_calls(calibri=True) if self.installs_calibri(c)]
        self.assertEqual([c['name'] for c in steps], ['curl', 'cabextract', 'cabextract', 'wine'])
        self.assertEqual(steps[3]['args'][:2], ['regedit', '/S'])
        self.assertFalse(any('calibri' in c['args'] for c in self.recorded_calls() if c['name'] == 'winetricks'))

    def test_calibri_failure_warns_launches_and_retries(self):
        self.copy_exe()
        self.env['TEST_CALIBRI_FAIL'] = '1'
        self.assert_calibri_warned_and_game_started(self.run_script())
        del self.env['TEST_CALIBRI_FAIL']
        self.assert_calibri_installed(self.run_script())

    def test_calibri_download_that_fails_its_checksum_is_not_used(self):
        self.copy_exe()
        self.env['TEST_VIEWER_CORRUPT'] = '1'
        self.assert_calibri_warned_and_game_started(self.run_script())
        self.assertFalse(any(c['name'] == 'cabextract' for c in self.recorded_calls(calibri=True)))
        del self.env['TEST_VIEWER_CORRUPT']
        self.assert_calibri_installed(self.run_script())

    def test_a_tool_calibri_needs_missing_warns_launches_and_retries(self):
        self.copy_exe()
        for name in ('curl', 'sha256sum', 'cabextract'):
            with self.subTest(tool=name):
                tool = self.tools / name
                disabled = self.root / (name + '.disabled')
                tool.rename(disabled)
                self.calls.unlink(missing_ok=True)
                result = self.run_script()
                tool_named = f'Install {name} first'
                self.assertNotIn(tool_named, result.stderr)
                self.assert_calibri_warned_and_game_started(result)
                self.assertFalse(any(self.installs_calibri(c) for c in self.recorded_calls(calibri=True)))
                disabled.rename(tool)
        self.assert_calibri_installed(self.run_script())

    def test_wine_setup_failures_retry_before_launching(self):
        self.copy_exe()
        prefix = self.bin / 'wineprefix'
        for failure in ('wine:wineboot.exe', 'wineserver:-w'):
            with self.subTest(failure=failure):
                self.env['TEST_FAIL'] = failure
                self.calls.unlink(missing_ok=True)
                result = self.run_script()
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((prefix / '.skhid-ready').exists())
                self.assertFalse((prefix / '.skhid-dxvk').exists())
                calls = self.recorded_calls()
                self.assertFalse(any(c['name'] == 'winetricks' or
                    c['args'][0] == 'survarium_skhid.exe' for c in calls))
        del self.env['TEST_FAIL']
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue((prefix / '.skhid-ready').exists())
        self.assertTrue((prefix / '.skhid-dxvk').exists())

    def test_game_failure_is_reported_and_allows_relaunch(self):
        self.copy_exe()
        self.env['TEST_FAIL'] = 'wine:survarium_skhid.exe'
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Game exited with an error', result.stderr)
        del self.env['TEST_FAIL']
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.recorded_calls()
        self.assertEqual([c['name'] for c in calls],
            ['wine', 'wineserver', 'winetricks', 'wine', 'wine'])

    def test_full_crash_dumps_reach_the_game_and_its_crash_files_are_listed(self):
        self.copy_exe()
        for tool in ('find', 'sort', 'date'):
            (self.tools / tool).symlink_to(shutil.which(tool))
        old = self.bin / 'SRP-crash-old-0.zip'
        old.write_bytes(b'PK')
        os.utime(old, (1, 1))
        self.env.update(SKHID_CRASH_DUMP='full', TEST_CRASH='1', TEST_FAIL='wine:survarium_skhid.exe')
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Game exited with an error', result.stderr)
        calls = self.recorded_calls()
        game = [c for c in calls if c['args'][:1] == ['survarium_skhid.exe']]
        self.assertEqual([c['crash_dump'] for c in game], ['full'])
        # The script waits for every Wine process (the game the launcher
        # started) before it lists the crash files.
        self.assertEqual(calls[-1]['name'], 'wineserver')
        self.assertEqual(calls[-1]['args'], ['-w'])
        # Each named with what it is for: the .dmp kept, not posted.
        self.assertIn(str(self.bin / 'SRP-crash-test-0.dmp') + '  (the whole memory: keep it, do not post it', result.stdout)
        self.assertIn(str(self.bin / 'SRP-crash-test-0.txt') + '  (the crash report: send this to a new post in #bugreports)', result.stdout)
        self.assertNotIn('SRP-crash-old-0', result.stdout)

    def test_crash_dump_default_and_unknown_values(self):
        self.copy_exe()
        self.env['SKHID_CRASH_DUMP'] = ''
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual({c['crash_dump'] for c in self.recorded_calls()}, {None})
        self.assertNotIn('Crash', result.stdout)
        self.calls.unlink()
        for value in ('Full', 'small', '1'):
            self.env['SKHID_CRASH_DUMP'] = value
            result = self.run_script()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('SKHID_CRASH_DUMP=' + value, result.stderr)
            self.assertFalse(self.calls.exists())

    def test_concurrent_launcher_cannot_start_wine(self):
        self.copy_exe()
        with (self.bin / '.skhid-launcher.lock').open('w') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = self.run_script()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('already running', result.stderr)
            self.assertEqual(self.exe.read_bytes(), FIXTURE)
            self.assertFalse(self.calls.exists())
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_missing_registry_reinitializes_prefix_and_dxvk(self):
        self.copy_exe()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        (self.bin / 'wineprefix/system.reg').unlink()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.recorded_calls()
        self.assertEqual([c['name'] for c in calls[-4:]], ['wine', 'wineserver', 'winetricks', 'wine'])



if __name__ == '__main__':
    unittest.main()
