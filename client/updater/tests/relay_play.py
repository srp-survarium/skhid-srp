"""PLAY's route (Automatic, the relay, direct) through real Windows sockets under Wine."""
import socket
import threading
import time

# A PLAY whose own route check fails: the check (seconds, relay_client::STALL)
# and the diagnosis's asks of the site (5 s each at most).
CHECKS = 60


class Routes:
    """A stand-in direct route (127.0.0.1) and relay (127.0.0.2) with the same
    ports: login TCP, which echoes 64 KiB at most, and lobby/match UDP, which
    echo while `udp[0]` (direct) and `udp[1]` (the relay) are set, and stay
    silent otherwise."""

    def __init__(self):
        self.stop = threading.Event()
        self.udp = [threading.Event(), threading.Event()]
        for event in self.udp:
            event.set()
        for _ in range(100):
            first = socket.socket()
            first.bind(('127.0.0.1', 0))
            self.port = first.getsockname()[1]
            self.sockets = [first]
            if self.port > 65532:
                first.close()
                continue
            try:
                other = socket.socket()
                self.sockets.append(other)
                other.bind(('127.0.0.2', self.port))
                for host in ['127.0.0.1', '127.0.0.2']:
                    for p in [self.port+1, self.port+2]:
                        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                        self.sockets.append(s)
                        s.bind((host, p))
                break
            except OSError:
                for s in self.sockets:
                    s.close()
        else:
            raise AssertionError('could not reserve probe endpoints')
        self.direct = f'127.0.0.1:{self.port}'
        self.relay = f'127.0.0.2:{self.port}'
        self.threads = []
        for s in self.sockets:
            s.settimeout(.1)
        for s in self.sockets[:2]:
            s.listen()
            self.start(self.tcp, s)
        for index, s in enumerate(self.sockets[2:]):
            self.start(self.echo, s, self.udp[index//2])

    def start(self, target, *args):
        thread = threading.Thread(target=target, args=args, daemon=True)
        thread.start()
        self.threads.append(thread)

    def tcp(self, listener):
        while not self.stop.is_set():
            try:
                client, _ = listener.accept()
            except (TimeoutError, OSError):
                continue
            # The server's echo: what comes, as it comes, 64 KiB at most.
            with client:
                client.settimeout(2)
                left = 64 * 1024
                try:
                    while left:
                        part = client.recv(left)
                        if not part:
                            break
                        client.sendall(part)
                        left -= len(part)
                except OSError:
                    pass

    def echo(self, s, event):
        while not self.stop.is_set():
            try:
                data, peer = s.recvfrom(2048)
            except (TimeoutError, OSError):
                continue
            if event.is_set():
                s.sendto(data, peer)

    def config(self):
        """`updater_skhid.relay`'s text."""
        return f'{self.direct}\n{self.relay}\n'

    def close(self):
        self.stop.set()
        for s in self.sockets:
            s.close()
        for t in self.threads:
            t.join(timeout=2)


def check(run, updater, x86, started, log, win, survarium):
    """`survarium`: the prefix's Documents\\survarium, where the updater keeps
    the player's route (launcher_skhid.route) and the route found for PLAY
    in Automatic (launcher_skhid.route_found)."""
    choice = survarium / 'launcher_skhid.route'
    found = survarium / 'launcher_skhid.route_found'
    routes = Routes()
    config = x86 / 'updater_skhid.relay'
    config.write_text(routes.config())

    def expect(host):
        for _ in range(100):
            if started.exists():
                lines = started.read_bytes().decode('utf-16').splitlines()
                if len(lines) >= 2:
                    command, folder = lines[:2]
                    assert f'-server={host}' in command, command
                    assert '-minimum_resources_memory_size=768' in command, command
                    assert ' quiet' not in command, command
                    assert folder.lower().rstrip('\\') == win(x86).lower(), folder
                    started.unlink()
                    return
            time.sleep(.05)
        raise AssertionError('PLAY did not start stand-in game: '+log())

    def play(expected=0):
        """PLAY, and how long it took."""
        begun = time.monotonic()
        run(updater, 'play', 'quiet', '-minimum_resources_memory_size=768', expected=expected, timeout=CHECKS)
        return time.monotonic() - begun

    try:
        # None chosen yet: decided once by the Windows region (Wine's: the
        # United States) and saved: direct, unchecked (the launcher's check
        # looks for a block).
        choice.unlink(missing_ok=True)
        found.unlink(missing_ok=True)
        routes.udp[0].clear()
        play()
        expect(routes.direct)
        assert choice.read_bytes() == b'direct\r\n', choice.read_bytes()
        assert f'play: Direct {routes.direct} (chosen), unchecked' in log(), log()
        assert not found.exists()
        # Automatic with no route found yet (no launcher has checked): PLAY
        # checks itself. The direct route's login TCP answers and its UDP
        # never: five rounds, then the relay, in seconds.
        choice.write_text('automatic\r\n')
        took = play()
        expect(routes.relay)
        assert 'play: no route found yet: PLAY checks' in log(), log()
        assert 'play: route check (found before: none): direct in 5 rounds: (1) login TCP ok, lobby UDP TimedOut, ' \
            'match UDP TimedOut; (2) ' in log(), log()
        assert '(5) login TCP ok, lobby UDP TimedOut, match UDP TimedOut; relay: answered -> Relay(' in log(), log()
        assert took < 15, took
        assert found.read_bytes() == b'relay\r\n', found.read_bytes()
        # Found: PLAY starts by it at once, whatever answers now.
        routes.udp[0].set()
        took = play()
        expect(routes.relay)
        assert f'play: Relay {routes.relay} (relay), unchecked' in log(), log()
        assert took < 5, took
        found.write_text('direct\r\n')
        play()
        expect(routes.direct)
        # A direct link lost in the last game: one game through the relay.
        found.write_text('direct-lost\r\n')
        play()
        expect(routes.relay)
        assert found.read_bytes() == b'relay\r\n', found.read_bytes()
        # Nothing answers and nothing found: no game, nothing found.
        found.unlink()
        routes.udp[0].clear()
        routes.udp[1].clear()
        play(expected=1)
        assert not started.exists(), 'unreachable routes must not launch the game'
        assert '; relay: login TCP ok, lobby UDP TimedOut, match UDP TimedOut -> Unreachable' in log(), log()
        assert not found.exists()
        # The relay chosen: through it, unprobed (nothing answers at all).
        choice.write_text('relay\r\n')
        play()
        expect(routes.relay)
        run(updater, 'play', 'quiet', '-minimum_resources_memory_size=768', '-server=127.0.0.9:1234')
        expect('127.0.0.9:1234')
        # The scripts' form: the updater stays until the game has ended (the
        # stand-in's `hold` runs until its release file is there).
        file, held, release = x86 / 'wait.file', x86 / 'wait.held', x86 / 'wait.release'
        file.write_bytes(b'')
        problems = []
        def waiting():
            try:
                run(updater, 'play', 'wait', 'quiet', 'hold', win(file), win(held), win(release),
                    '-server=127.0.0.9:1234')
            except BaseException as error:
                problems.append(error)
        script = threading.Thread(target=waiting)
        script.start()
        for _ in range(400):
            if held.exists() or not script.is_alive():
                break
            time.sleep(.05)
        assert held.exists(), f'the stand-in game did not start: {problems} {log()}'
        time.sleep(.5)
        assert script.is_alive(), 'play wait ended before the game did'
        release.write_bytes(b'')
        script.join(timeout=30)
        assert not script.is_alive() and not problems, problems
        started.unlink(missing_ok=True)
    finally:
        config.unlink(missing_ok=True)
        choice.unlink(missing_ok=True)
        found.unlink(missing_ok=True)
        routes.close()
