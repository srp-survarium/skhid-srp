import contextlib
import csv
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import run


class SpawnInspection(unittest.TestCase):
    def test_run_uses_local_launcher_port_array_and_targets_chosen_room(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            binary = root / 'srp-bot'
            binary.touch()
            game_data = root / 'server-game-data.db'
            game_data.touch()
            config = root / 'config.json'
            cfg = {'ports': [2234, 2235, 2236, 18088], 'server': {
                'binary': str(root / 'srp'), 'env': {
                    'SRP_BOT_ACCOUNTS': ','.join(run.NAMES), 'SRP_MIN_PATCH_REVISION': '35',
                    'SRP_GAME_DATA': str(game_data)}}}
            for ports in (cfg['ports'], {'login': 2234}):
                cfg['ports'] = ports
                config.write_text(json.dumps(cfg))
                argv = ['run.py', '--config', str(config), 'run', '4746', '--pin', '1234']
                with patch('sys.argv', argv), patch('run.os.execve') as execute, contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(run.main(), 0)
                execute.assert_called_once()
                invoked_binary, args, child_env = execute.call_args.args
                self.assertEqual(invoked_binary, str(binary))
                self.assertEqual(args, [str(binary), '--server', '127.0.0.1:2234',
                    '--name', 'Spawn1', '--count', '19', '--revision', '35', '--room', '4746', '--pin', '1234'])
                self.assertEqual(child_env['SRP_GAME_DATA'], str(game_data))
                self.assertEqual(child_env.get('PATH'), run.os.environ.get('PATH'))

    def test_configure_preserves_accounts_and_existing_bot_names(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'config.json'
            original = {'accounts': [{'name': 'Owner', 'password': 'test'}],
                        'server': {'env': {'SRP_BOT_ACCOUNTS': 'Existing,spawn1', 'SRP_BLACKOUT': 'on'}}}
            path.write_text(json.dumps(original))
            with contextlib.redirect_stdout(io.StringIO()):
                run.configure(path)
            actual = json.loads(path.read_text())
            self.assertEqual(actual['accounts'], original['accounts'])
            self.assertEqual(actual['server']['env']['SRP_BLACKOUT'], 'on')
            names = actual['server']['env']['SRP_BOT_ACCOUNTS'].lower().split(',')
            self.assertEqual(len(names), 20)
            self.assertIn('existing', names)
            backup, = Path(folder).glob('config.json.before-*')
            self.assertEqual(json.loads(backup.read_text()), original)

    def test_coverage_distinguishes_twenty_players_from_twenty_spawns(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            points = [[i * 4, 9.5, 0] for i in range(20)]
            edits = root / 'edits.json'
            edits.write_text(json.dumps({'set_entry': [
                {'file': 'projects/level_09/day_2/client_project', 'list': 'server_objects/respawn_points',
                 'where': {'full_name': str(i)}, 'to': {'position': p}} for i, p in enumerate(points)]}))
            positions = root / 'positions.csv'
            for duplicate in (False, True):
                with positions.open('w') as f:
                    writer = csv.writer(f)
                    writer.writerow(['match', 'time', 'place', 'alive', 'x', 'y', 'z'])
                    for i, point in enumerate(points):
                        x, y, z = points[0] if duplicate and i == 19 else point
                        writer.writerow(['abc', 1000, i, 1, x, y + 0.9, z])
                with contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(run.coverage(positions, edits), not duplicate)


if __name__ == '__main__':
    unittest.main()
