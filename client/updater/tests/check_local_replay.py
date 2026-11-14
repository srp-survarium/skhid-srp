#!/usr/bin/env python3
"""Wine integration: local .sur launch and Explorer registration in a NEW prefix,
and that "Save replay" beside it saves a .srpreplay and starts no game.

Run after build.sh: python tests/check_local_replay.py --wine /path/to/wine
Uses only a stand-in game. Keeps its temporary directory as evidence.
"""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import threading
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--wine', required=True)
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    root = Path(tempfile.mkdtemp(prefix='skhid-replay-test-', dir='/tmp'))
    env = dict(os.environ, WINEPREFIX=str(root / 'prefix'), WINEDEBUG='-all',
               WINEDLLOVERRIDES='winemenubuilder.exe=d')
    env.pop('WINEARCH', None)
    top = root / 'My Games' / 'Skhid'
    game = top / 'game'
    x86 = game / 'binaries' / 'x86'
    x86.mkdir(parents=True)
    updater = x86 / 'updater_skhid.exe'
    fake = here / 'fake_exe/target/i686-win7-windows-msvc/release/fake-exe.exe'
    shutil.copy2(here.parent / 'exe/target/i686-win7-windows-msvc/release/updater_skhid.exe', updater)
    shutil.copy2(fake, x86 / 'survarium_skhid.exe')
    (x86 / 'updater_skhid.patches').write_text('{}')
    status = x86 / 'updater_skhid.patches.status'
    status.write_text('ready\r\n')
    started = x86 / 'survarium_skhid.exe.started.txt'
    log = x86 / 'updater_skhid.log'

    def win(path):
        return 'Z:' + str(path).replace('/', '\\')

    def run(*argv, expected=0):
        # Wine's persistent server/children may inherit stdout. A regular file
        # avoids waiting for pipe EOF after the invoked process has exited.
        with tempfile.TemporaryFile() as output:
            done = subprocess.run([args.wine, *map(str, argv)], env=env, cwd=root,
                                  stdout=output, stderr=subprocess.STDOUT, timeout=60)
            output.seek(0)
            data = output.read()
        assert done.returncode == expected, (argv, done.returncode, data,
                                            log.read_text() if log.exists() else '')
        return data.decode('utf-8', errors='replace')

    def expect_game(path):
        for _ in range(100):
            if started.exists():
                lines = started.read_bytes().decode('utf-16').splitlines()
                if len(lines) >= 2:
                    command, folder = lines[:2]
                    assert command == (f'"{win(x86 / "survarium_skhid.exe")}" '
                                       f'-match_replay="{path}"'), command
                    assert folder.lower().rstrip('\\') == win(x86).lower()
                    started.unlink()
                    return
            time.sleep(.05)
        raise AssertionError('stand-in game did not start: ' + log.read_text())

    run('wineboot', '-u')
    wineserver = str(Path(args.wine).with_name('wineserver'))
    file = root / 'Downloaded matches' / 'match & 1.SUR'
    file.parent.mkdir()
    file.write_bytes(b'test replay fixture; stand-in game only')
    digest = hashlib.sha256(file.read_bytes()).hexdigest()
    for argv in [('open-replay', win(file), 'quiet'), (win(file), 'quiet')]:
        run(updater, *argv)
        expect_game(win(file))
    assert not (game / 'replays').exists(), 'local launch must not populate or prune cache'

    directory = file.parent / 'directory.sur'
    directory.mkdir()
    for argv, problem in [
        (('open-replay', r'Z:\full\path\to\x.sur', 'quiet'), 'Open'),
        (('open-replay', win(directory), 'quiet'), 'Open'),
        (('open-replay', win(file), 'extra', 'quiet'), 'BadFile'),
        (('open-replay', win(file.with_suffix('.exe')), 'quiet'), 'BadFile'),
        (('open-replay', 'quiet'), 'BadFile'),
    ]:
        run(updater, *argv, expected=1)
        assert f'FAILED ({problem}' in log.read_text()
        assert not started.exists()
    status.write_text('blocked unfinished update\r\n')
    run(updater, 'open-replay', win(file), 'quiet', expected=1)
    assert 'FAILED (NotReady)' in log.read_text()
    assert not started.exists()
    status.write_text('ready\r\n')

    # Registration leaves another program's extension default untouched.
    ext = r'HKCU\Software\Classes\.sur'
    run('reg', 'add', ext, '/ve', '/d', 'Other.Replay', '/f')
    for _ in range(2):
        run(updater, 'register-replays', 'quiet')
    assert 'Other.Replay' in run('reg', 'query', ext, '/ve')
    handler = run('reg', 'query', r'HKCU\Software\Classes\Skhid.Replay.1\shell\open\command', '/ve')
    assert f'"{win(updater)}" open-replay "%1"' in handler
    assert 'Skhid.Replay.1' in run('reg', 'query', ext + r'\OpenWithProgids')
    assert 'Skhid Replay' in run('reg', 'query', r'HKCU\Software\RegisteredApplications', '/v', 'Skhid Replay')
    # Wine's equivalent of choosing the handler, followed by the actual Shell open.
    run('reg', 'add', ext, '/ve', '/d', 'Skhid.Replay.1', '/f')
    # Wine 11.8 resolves HKCR from Machine\Software\Classes only (confirmed
    # with +reg tracing), without Windows' HKCU merge. Mirror our exact values
    # INSIDE THIS DISPOSABLE PREFIX to exercise ShellExecute's command dispatch.
    # The production updater never writes HKLM/HKCR or changes an existing default.
    for key in ['.sur', 'Skhid.Replay.1']:
        run('reg', 'copy', 'HKCU\\Software\\Classes\\' + key,
            'HKCR\\' + key, '/s', '/f')
    run(fake, 'open-replay-file', win(file))
    expect_game(win(file))

    # "Save replay" (`replay <id> <token> <sha256> <size>`) saves the server's
    # .srpreplay into game\replays and starts no game. The loopback service
    # then receives no request of a local file's.
    head = b'format 1\nmatch 1234abcd\nmade 1794174000\nmap 24\nmap_slug Rudnya\n'
    body = b'SRPRPL01' + struct.pack('<I', len(head)) + head + b'stand-in events and replay'
    requests = []
    class Server(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            self.send_response(200)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        def log_message(self, *_):
            pass
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Server)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    (x86 / 'updater_skhid.url').write_text(f'http://127.0.0.1:{server.server_port}')
    try:
        run(updater, 'replay', '1234abcd', '0123456789abcdef' * 2,
            hashlib.sha256(body).hexdigest(), str(len(body)), 'quiet')
        assert requests == ['/replays/' + '0123456789abcdef' * 2], requests
        saved = sorted((game / 'replays').iterdir())
        assert [path.name.endswith('_Rudnya_1234abcd.srpreplay') for path in saved] == [True], saved
        assert saved[0].read_bytes() == body
        assert 'replay: saved ' in log.read_text()
        time.sleep(1)
        assert not started.exists(), 'a save must not start the game'
        requests.clear()
        run(updater, 'open-replay', win(file), 'quiet')
        expect_game(win(file))
        assert not requests
        assert sorted((game / 'replays').iterdir()) == saved
    finally:
        server.shutdown()
        server.server_close()
    assert hashlib.sha256(file.read_bytes()).hexdigest() == digest
    (root / 'result.json').write_text(json.dumps({'passed': True, 'updater_sha256':
        hashlib.sha256(updater.read_bytes()).hexdigest()}, indent=2) + '\n')
    subprocess.run([wineserver, '-k'], env=env, check=False, timeout=10)
    print(f'PASS: local/drop/Shell launch, complete args/cwd, error/gate paths, registration '
          f'and existing default, Save replay saved with no game started, no local downloads. Evidence: {root}')


if __name__ == '__main__':
    main()
