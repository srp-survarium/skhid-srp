#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Exercise the release launcher with mocked Wine and synthetic patch-site bytes."""
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

SCRIPT = Path(__file__).resolve().parent / 'play_skhid.sh'
# Only the bytes inspected by the launcher; no game binary is needed.
FIXTURE = bytearray(0xf582d)
FIXTURE[:2] = b'MZ'
FIXTURE[0xf5805:0xf5818] = bytes.fromhex('85ff7719720881fb00000014730f6840010000')
FIXTURE[0xf5822:0xf582d] = bytes.fromhex('8bd75f5e8bc35b83c408c3')
PATCHED_FIXTURE = bytearray(FIXTURE)
PATCHED_FIXTURE[0xf5811] = 0xeb
ORIGINAL = hashlib.sha256(FIXTURE).hexdigest()
PATCHED = hashlib.sha256(PATCHED_FIXTURE).hexdigest()



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
        self.script.write_text(SCRIPT.read_text().replace('/sys/class/drm', str(self.drm)))
        self.config = self.bin / 'wineprefix/drive_c/users' / pwd.getpwuid(os.getuid()).pw_name / 'Documents/Survarium/user.cfg'
        self.tools = self.root / 'tools'
        self.tools.mkdir()
        self.calls = self.root / 'calls.jsonl'
        # Only mocked dependencies plus explicitly allowed core tools in PATH.
        for tool in ('bash', 'dirname', 'sha256sum', 'cut', 'dd', 'od', 'tr', 'cp', 'cmp',
                     'mv', 'mktemp', 'flock', 'timeout', 'rm', 'mkdir', 'touch', 'ls', 'id', 'awk'):
            (self.tools / tool).symlink_to(shutil.which(tool))
        stub = '#!' + sys.executable + '\n' + '''import json,os,pathlib,sys
name=pathlib.Path(sys.argv[0]).name
prefix=pathlib.Path(os.environ['WINEPREFIX'])
if name=='wine' and sys.argv[1]=='wineboot.exe':
 prefix.mkdir(parents=True,exist_ok=True)
with open(os.environ['TEST_CALLS'],'a') as f:
 f.write(json.dumps(dict(name=name,args=sys.argv[1:],cwd=os.getcwd(),prefix=str(prefix),dll=os.environ.get('WINEDLLOVERRIDES'))) + '\\n')
if name=='wineserver':
 # Delayed registry creation ensures the script waits for wineserver.
 (prefix/'system.reg').write_text('registry')
elif name=='winetricks' and os.environ.get('TEST_DXVK_FAIL')=='1':
 sys.exit(1)
'''
        for name in ('wine', 'wineserver', 'winetricks'):
            path = self.tools / name
            path.write_text(stub)
            path.chmod(0o755)
        self.env = dict(os.environ, PATH=str(self.tools), TEST_CALLS=str(self.calls))
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

    def copy_exe(self):
        self.exe = self.bin / 'survarium_skhid.exe'
        self.exe.write_bytes(FIXTURE)

    def run_script(self):
        return subprocess.run([str(self.tools / 'bash'), str(self.script)],
            cwd=self.root, env=self.env, text=True, capture_output=True)

    def test_missing_dependencies_fail_before_changes(self):
        for name in ('wine', 'winetricks', 'wineserver'):
            tool = self.tools / name
            tool.rename(tool.with_suffix('.disabled'))
            result = self.run_script()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(name.lower(), result.stderr.lower())
            self.assertEqual(list(self.bin.iterdir()), [self.script])
            tool.with_suffix('.disabled').rename(tool)

    def test_missing_exe(self):
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('beside', result.stderr)

    def test_generated_full_launcher_uses_embedded_server_and_keeps_exe(self):
        self.copy_exe()
        self.exe.write_bytes(PATCHED_FIXTURE)
        self.script.write_bytes(overlay.sh('release.example:2234', 'polish', True))
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = [json.loads(s) for s in self.calls.read_text().splitlines()]
        self.assertEqual(calls[-1]['args'], ['survarium_skhid.exe', '-localization=polish',
            '-mt_allocator=768', '-minimum_resources_memory_size=768'])
        self.assertEqual(self.exe.read_bytes(), PATCHED_FIXTURE)
        self.assertFalse(self.exe.with_name(self.exe.name + '.before-memory-patch').exists())

    def test_generated_cert_only_launcher_passes_configured_server(self):
        self.copy_exe()
        self.script.write_bytes(overlay.sh('release.example:2234', 'german', False))
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = [json.loads(s) for s in self.calls.read_text().splitlines()]
        self.assertEqual(calls[-1]['args'], ['survarium_skhid.exe', '-localization=german',
            '-mt_allocator=768', '-minimum_resources_memory_size=768', '-server=release.example:2234'])

    def test_full_launcher_allows_an_explicit_server_override(self):
        self.copy_exe()
        script = overlay.sh('release.example:2234', 'english', True)
        self.script.write_bytes(script.replace(b'server=\n', b'server=local.example:4321\n'))
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = [json.loads(s) for s in self.calls.read_text().splitlines()]
        self.assertIn('-server=local.example:4321', calls[-1]['args'])

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

    def test_unknown_exe_unchanged(self):
        exe = self.bin / 'survarium_skhid.exe'
        exe.write_bytes(b'wrong executable')
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(exe.read_bytes(), b'wrong executable')
        self.assertFalse(self.calls.exists())

    def test_patch_first_run_and_reuse(self):
        self.copy_exe()
        before = self.exe.read_bytes()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        after = self.exe.read_bytes()
        self.assertEqual(hashlib.sha256(after).hexdigest(), PATCHED)
        self.assertEqual([i for i, (a, b) in enumerate(zip(before, after)) if a != b], [0xf5811])
        backup = self.exe.with_name(self.exe.name + '.before-memory-patch')
        self.assertEqual(backup.read_bytes(), before)
        calls = [json.loads(s) for s in self.calls.read_text().splitlines()]
        self.assertEqual([c['name'] for c in calls], ['wine', 'wineserver', 'winetricks', 'wine'])
        self.assertEqual(calls[2]['args'], ['-q', 'dxvk'])
        self.assertEqual(calls[3]['cwd'], str(self.bin))
        self.assertEqual(calls[3]['args'], ['survarium_skhid.exe', '-localization=english',
            '-mt_allocator=768', '-minimum_resources_memory_size=768'])
        self.assertIn('qwave=', calls[3]['dll'])
        self.assertEqual(calls[3]['prefix'], str(self.bin / 'wineprefix'))
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(len(self.calls.read_text().splitlines()), 5)
        self.assertEqual(backup.read_bytes(), before)

    def test_dxvk_failure_retries_without_launching(self):
        self.copy_exe()
        self.env['TEST_DXVK_FAIL'] = '1'
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('DXVK installation failed', result.stderr)
        self.assertFalse((self.bin / 'wineprefix/.skhid-dxvk').exists())
        self.assertEqual(len(self.calls.read_text().splitlines()), 3)
        del self.env['TEST_DXVK_FAIL']
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = [json.loads(s) for s in self.calls.read_text().splitlines()]
        self.assertEqual([c['name'] for c in calls[-2:]], ['winetricks', 'wine'])

    def test_missing_registry_reinitializes_prefix_and_dxvk(self):
        self.copy_exe()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        (self.bin / 'wineprefix/system.reg').unlink()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = [json.loads(s) for s in self.calls.read_text().splitlines()]
        self.assertEqual([c['name'] for c in calls[-4:]], ['wine', 'wineserver', 'winetricks', 'wine'])

    def test_preserves_existing_backup(self):
        self.copy_exe()
        backup = self.exe.with_name(self.exe.name + '.before-memory-patch')
        backup.write_bytes(b'unrelated backup')
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(backup.with_name(backup.name + '.~1~').read_bytes(), b'unrelated backup')
        self.assertEqual(hashlib.sha256(backup.read_bytes()).hexdigest(), ORIGINAL)

    def test_updated_exe_preserves_unrelated_changes(self):
        self.copy_exe()
        first_version = self.exe.read_bytes()
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        # Simulate an update restoring the original branch and changing other code/data.
        updated = bytearray(first_version)
        updated[0x1900] ^= 1
        updated.extend(b'new Skhid section or build metadata')
        self.exe.write_bytes(updated)
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        expected = bytearray(updated)
        expected[0xf5811] = 0xeb
        self.assertEqual(self.exe.read_bytes(), expected)
        backup = self.exe.with_name(self.exe.name + '.before-memory-patch')
        self.assertEqual(backup.read_bytes(), updated)
        self.assertEqual(backup.with_name(backup.name + '.~1~').read_bytes(), first_version)
        # An update retaining the patch also works without making another backup.
        expected[-1] ^= 1
        self.exe.write_bytes(expected)
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.exe.read_bytes(), expected)
        self.assertEqual(backup.read_bytes(), updated)

    def test_changed_memory_check_is_rejected(self):
        self.copy_exe()
        original = self.exe.read_bytes()
        for offset in (0xf5805, 0xf5812, 0xf5813, 0xf5822):
            with self.subTest(offset=hex(offset)):
                altered = bytearray(original)
                altered[offset] ^= 1
                self.exe.write_bytes(altered)
                result = self.run_script()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('Memory-check code changed', result.stderr)
                self.assertEqual(self.exe.read_bytes(), altered)
        self.assertFalse(self.calls.exists())


if __name__ == '__main__':
    unittest.main()
