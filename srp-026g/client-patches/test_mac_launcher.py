#!/usr/bin/env python3
"""The macOS launcher's own logic, run on any host with stub commands.

play_skhid_mac.sh is sourced without its last line (`main "$@"`), so each test
calls one function with the commands it uses replaced by small stubs.
"""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import textwrap
import unittest

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / 'play_skhid_mac.sh'
STARTER = HERE / 'Skhid.sh'


class MacLauncherTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix='skhid-mac-')
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.stubs = self.root / 'stubs'
        self.stubs.mkdir()
        self.calls = self.root / 'calls'
        self.functions = self.root / 'functions.sh'
        self.functions.write_text(SCRIPT.read_text().rsplit('main "$@"', 1)[0])
        if not shutil.which('shasum'):  # macOS has it; Linux hosts run the tests with sha256sum
            self.stub('shasum', '[[ $1 == -a && $2 == 256 ]] && shift 2; exec sha256sum "$@"\n')

    def stub(self, name, body):
        path = self.stubs / name
        path.write_text('#!/bin/bash\n' + textwrap.dedent(body))
        path.chmod(0o755)

    def run_bash(self, code, **env):
        environ = dict(os.environ, PATH=f'{self.stubs}:{os.environ["PATH"]}', CALLS=str(self.calls),
                       HOME=str(self.root / 'home'), **env)
        return subprocess.run(['bash', '-c', f'source "{self.functions}"\n{code}'], cwd=self.root,
                              env=environ, capture_output=True, text=True, timeout=60)

    def test_resolution_is_the_main_display_or_a_safe_default(self):
        self.stub('osascript', 'echo 1512x982\n')
        self.assertEqual(self.run_bash('detect_resolution').stdout.strip(), '1512x982')
        self.stub('osascript', 'echo "execution error"; exit 1\n')
        self.assertEqual(self.run_bash('detect_resolution').stdout.strip(), '1440x900')

    def test_set_reg_skips_a_value_already_set(self):
        self.stub('wine', 'echo "$@" >> "$CALLS"\n')
        prefix = self.root / 'prefix'
        prefix.mkdir()
        (prefix / 'user.reg').write_text('"RetinaMode"="y"\n')
        code = f'WINE=wine WINEPREFIX="{prefix}" log=/dev/null; set_reg "Software\\Wine\\Mac Driver" '
        self.run_bash(code + 'RetinaMode y')
        self.assertFalse(self.calls.exists())
        self.run_bash(code + 'LeftOptionIsAlt y')
        self.assertIn('LeftOptionIsAlt', self.calls.read_text())

    def test_the_runtime_folder_follows_its_pins(self):
        first = self.run_bash('runtime_dir').stdout.strip()
        self.assertEqual(self.run_bash('runtime_dir').stdout.strip(), first)
        changed = self.run_bash('MOUSE_FILES="$MOUSE_FILES x"; runtime_dir').stdout.strip()
        self.assertNotEqual(changed, first)
        self.assertTrue(first.endswith(f'/Library/Application Support/Skhid/runtime/{first.rsplit("/", 1)[1]}'))

    def test_a_stuck_wine_command_is_stopped(self):
        self.stub('wineserver', 'echo "$@" >> "$CALLS"\n')
        result = self.run_bash('WINESERVER=wineserver; wine_within 1 sleep 30 || echo "status $?"')
        self.assertIn('status 124', result.stdout)
        self.assertEqual(self.calls.read_text().split(), ['-k'])
        self.assertIn('status 0', self.run_bash('WINESERVER=wineserver; wine_within 5 true; echo "status $?"').stdout)

    def test_crash_files_of_this_run_are_listed(self):
        self.stub('wineserver', 'exit 0\n')
        game = self.root / 'game'
        game.mkdir()
        (game / 'SRP-crash-old.zip').write_bytes(b'old')
        os.utime(game / 'SRP-crash-old.zip', (1, 1))
        code = f'''WINESERVER=wineserver WINEPREFIX="{self.root}/prefix" WINE_USER=crossover
            since=$(mktemp "{self.root}/since.XXXXXX"); touch -t 200001010000 "$since"
            cd "{game}"; : > SRP-crash-new.zip; : > SRP-crash-new.dmp; print_crashes "$since"'''
        out = self.run_bash(code).stdout
        self.assertIn('SRP-crash-new.zip  (a crash ZIP', out)
        self.assertIn('SRP-crash-new.dmp  (the whole memory', out)
        self.assertNotIn('SRP-crash-old.zip', out)

    def test_main_refuses_other_systems_and_arguments(self):
        game = self.root / 'x86'
        game.mkdir()
        (game / 'survarium_skhid.exe').write_bytes(b'MZ')
        (game / 'play_skhid_mac.sh').write_text(SCRIPT.read_text())
        env = dict(os.environ, PATH=f'{self.stubs}:{os.environ["PATH"]}')
        def run(*args):
            return subprocess.run(['bash', str(game / 'play_skhid_mac.sh'), *args], capture_output=True,
                                  text=True, env=env, timeout=60).stderr
        self.stub('uname', '[[ $1 == -s ]] && echo Linux || echo x86_64\n')
        self.assertIn('on Linux use play_skhid.sh', run())
        self.stub('uname', '[[ $1 == -s ]] && echo Darwin || echo x86_64\n')
        self.assertIn('Apple silicon (M1 or newer)', run())
        self.assertIn('Usage: bash play_skhid_mac.sh [launcher]', run('other'))

    def test_the_starter_picks_the_mac_script_on_macos(self):
        install = self.root / 'install'
        x86 = install / 'game/binaries/x86'
        x86.mkdir(parents=True)
        (install / 'Skhid.sh').write_text(STARTER.read_text())
        for name in ('play_skhid.sh', 'play_skhid_mac.sh'):
            (x86 / name).write_text(f'echo {name} "$@"\n')
        self.stub('uname', 'echo Darwin\n')
        result = subprocess.run(['bash', str(install / 'Skhid.sh'), 'launcher'], capture_output=True, text=True,
                                env=dict(os.environ, PATH=f'{self.stubs}:{os.environ["PATH"]}'), timeout=60)
        self.assertEqual(result.stdout.strip(), 'play_skhid_mac.sh launcher')


if __name__ == '__main__':
    unittest.main()
