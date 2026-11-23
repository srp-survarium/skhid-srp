"""PLAY's route (Automatic, the relay, direct) through real Windows sockets under Wine."""
import socket
import threading
import time


def check(run, updater, x86, started, log, win, survarium):
    """`survarium`: the prefix's Documents\\survarium, where the updater keeps
    the player's route (launcher_skhid.route) and the route the last game
    played by (launcher_skhid.route_played)."""
    choice = survarium / 'launcher_skhid.route'
    played = survarium / 'launcher_skhid.route_played'

    stop = threading.Event()
    enabled = [threading.Event(), threading.Event()]
    for event in enabled:
        event.set()
    sockets = []
    # One direct and one relay endpoint, with matching consecutive ports.
    for _ in range(100):
        first = socket.socket()
        first.bind(('127.0.0.1', 0))
        port = first.getsockname()[1]
        sockets = [first]
        if port > 65532:
            first.close()
            continue
        try:
            other = socket.socket()
            sockets.append(other)
            other.bind(('127.0.0.2', port))
            for host in ['127.0.0.1', '127.0.0.2']:
                for p in [port+1, port+2]:
                    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                    sockets.append(s)
                    s.bind((host, p))
            break
        except OSError:
            for s in sockets:
                s.close()
    else:
        raise AssertionError('could not reserve probe endpoints')
    threads = []
    def tcp(listener):
        while not stop.is_set():
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
    def udp(s, event):
        while not stop.is_set():
            try:
                data, peer = s.recvfrom(2048)
            except (TimeoutError, OSError):
                continue
            if event.is_set():
                s.sendto(data, peer)
    for s in sockets:
        s.settimeout(.1)
    for s in sockets[:2]:
        s.listen()
        t = threading.Thread(target=tcp, args=(s,), daemon=True)
        t.start()
        threads.append(t)
    for index, s in enumerate(sockets[2:]):
        t = threading.Thread(target=udp, args=(s, enabled[index//2]), daemon=True)
        t.start()
        threads.append(t)
    config = x86 / 'updater_skhid.relay'
    config.write_text(f'127.0.0.1:{port}\n127.0.0.2:{port}\n')
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
    try:
        # None chosen yet: decided once by the Windows region (Wine's: the
        # United States) and saved: direct.
        choice.unlink(missing_ok=True)
        played.unlink(missing_ok=True)
        run(updater, 'play', 'quiet', '-minimum_resources_memory_size=768')
        expect(f'127.0.0.1:{port}')
        assert choice.read_bytes() == b'direct\r\n', choice.read_bytes()
        # One round, and the route it played by kept for the next check.
        assert 'play: route check (last game none): direct in 1 round: (1) answered -> Direct(' in log(), log()
        assert played.read_bytes() == b'direct\r\n', played.read_bytes()
        # Direct, the direct route's UDP silent but the relay answering: blocked
        # here, so the choice turns to Automatic by itself (saved) and the game goes
        # through the relay. Its login TCP answers, and the last game played
        # direct: six rounds before the relay is asked.
        enabled[0].clear()
        run(updater, 'play', 'quiet', '-minimum_resources_memory_size=768')
        expect(f'127.0.0.2:{port}')
        assert 'route: diagnosis Some(Blocked)' in log(), log()
        assert choice.read_bytes() == b'automatic\r\n', choice.read_bytes()
        assert 'play: route check (last game direct): direct in 6 rounds: (1) login TCP ok, lobby UDP TimedOut, ' \
            'match UDP TimedOut; (2) ' in log(), log()
        assert '(6) login TCP ok, lobby UDP TimedOut, match UDP TimedOut; relay: answered -> Relay(' in log(), log()
        assert played.read_bytes() == b'relay\r\n', played.read_bytes()
        # Direct, nothing answering either way: no game, and the choice stays.
        enabled[1].clear()
        choice.write_text('direct\r\n')
        run(updater, 'play', 'quiet', '-minimum_resources_memory_size=768', expected=1)
        assert not started.exists(), 'unreachable routes must not launch the game'
        assert choice.read_bytes() == b'direct\r\n', choice.read_bytes()
        # After a game through the relay: five rounds, not six; no game, so
        # the route played by stays.
        assert 'play: route check (last game relay): direct in 5 rounds: ' in log(), log()
        assert '; relay: login TCP ok, lobby UDP TimedOut, match UDP TimedOut -> Unreachable' in log(), log()
        assert played.read_bytes() == b'relay\r\n', played.read_bytes()
        # The relay chosen: through it, unprobed (nothing answers at all).
        choice.write_text('relay\r\n')
        run(updater, 'play', 'quiet', '-minimum_resources_memory_size=768')
        expect(f'127.0.0.2:{port}')
        # Automatic: the first route that answers.
        for event in enabled:
            event.set()
        choice.write_text('automatic\r\n')
        run(updater, 'play', 'quiet', '-minimum_resources_memory_size=768')
        expect(f'127.0.0.1:{port}')
        assert played.read_bytes() == b'direct\r\n', played.read_bytes()
        enabled[0].clear()
        run(updater, 'play', 'quiet', '-minimum_resources_memory_size=768')
        expect(f'127.0.0.2:{port}')
        assert played.read_bytes() == b'relay\r\n', played.read_bytes()
        enabled[1].clear()
        run(updater, 'play', 'quiet', '-minimum_resources_memory_size=768', expected=1)
        assert not started.exists(), 'unreachable routes must not launch the game'
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
        played.unlink(missing_ok=True)
        stop.set()
        for s in sockets:
            s.close()
        for t in threads:
            t.join(timeout=2)
