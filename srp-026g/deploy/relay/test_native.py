"""Real TCP/UDP sockets against the compiled native relay; no game launches."""
import os
import queue
import select
import socket
import subprocess
import threading
import time
import unittest

import forwarder as f


@unittest.skipUnless(os.environ.get('SKHID_NATIVE_RELAY'), 'native binary supplied by tools/check')
class NativeTests(unittest.TestCase):
    def setUp(self):
        self.stopped = threading.Event()
        self.observed = queue.Queue()
        self.sockets = []
        self.threads = []
        # Reserve all backend ports before starting the relay on 127.0.0.2.
        for _ in range(100):
            tcp = socket.socket()
            tcp.bind(('127.0.0.1', 0))
            port = tcp.getsockname()[1]
            if port > 65532:
                tcp.close()
                continue
            udp = []
            try:
                for p in range(port+1, port+4):
                    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                    udp.append(s)
                    s.bind(('127.0.0.1', p))
                break
            except OSError:
                tcp.close()
                for s in udp:
                    s.close()
        else:
            self.fail('could not reserve four test ports')
        self.port = port
        self.sockets = [tcp, *udp]
        tcp.listen()
        for s in self.sockets:
            s.settimeout(0.1)
        def read_exact(s, size):
            data = b''
            while len(data) < size:
                part = s.recv(size-len(data))
                if not part:
                    raise EOFError
                data += part
            return data
        def tcp_backend():
            while not self.stopped.is_set():
                try:
                    client, _ = tcp.accept()
                except (TimeoutError, OSError):
                    continue
                with client:
                    client.settimeout(2)
                    header = read_exact(client, 28)
                    self.observed.put(('tcp', header))
                    data = read_exact(client, 5)
                    client.sendall(data.upper())
        def udp_backend(s):
            while not self.stopped.is_set():
                try:
                    data, peer = s.recvfrom(8192)
                except (TimeoutError, OSError):
                    continue
                self.observed.put(('udp', data, peer, s))
                s.sendto(data, peer)
        for target, args in [(tcp_backend, ())] + [(udp_backend, (s,)) for s in udp]:
            thread = threading.Thread(target=target, args=args, daemon=True)
            thread.start()
            self.threads.append(thread)
        self.process = subprocess.Popen([os.environ['SKHID_NATIVE_RELAY'], '--public', '203.0.113.8',
            '--backend', '127.0.0.1', '--bind', '127.0.0.2', '--login-port', str(port)],
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        if not select.select([self.process.stderr], [], [], 5)[0]:
            self.tearDown()
            self.fail('native relay startup timed out')
        ready = self.process.stderr.readline()
        if b'ready:' not in ready:
            self.tearDown()
            self.fail(f'native relay startup: {ready!r}')

    def tearDown(self):
        if getattr(self, 'process', None):
            self.process.terminate()
            self.process.wait(timeout=5)
            self.process.stderr.close()
        self.stopped.set()
        for s in self.sockets:
            s.close()
        for thread in self.threads:
            thread.join(timeout=2)

    def test_login_preserves_stream_and_observed_client(self):
        with socket.create_connection(('127.0.0.2', self.port), timeout=2) as client:
            address = client.getsockname()
            client.sendall(b'hello')
            self.assertEqual(client.recv(5), b'HELLO')
            kind, header = self.observed.get(timeout=2)
            self.assertEqual(kind, 'tcp')
            self.assertEqual(header, f.header(address, '203.0.113.8', self.port, 0x11))

    def test_each_login_and_flow_is_reported(self):
        with socket.create_connection(('127.0.0.2', self.port), timeout=2) as client:
            login = client.getsockname()
            client.sendall(b'hello')
            self.assertEqual(client.recv(5), b'HELLO')
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as client:
            client.bind(('127.0.0.1', 0))
            client.settimeout(2)
            client.sendto(b'ping', ('127.0.0.2', self.port+2))
            self.assertEqual(client.recv(8192), b'ping')
            match = client.getsockname()
        self.assertTrue(select.select([self.process.stderr], [], [], 2)[0])
        self.assertRegex(self.process.stderr.readline().decode(),
            rf'^login 127\.0\.0\.1:{login[1]}: \d+\.\d s, up 5 bytes, down 5 bytes, closed\n$')
        self.process.terminate()
        self.assertEqual(self.process.wait(timeout=5), 0)
        self.assertRegex(self.process.stderr.read().decode(),
            r'^stopping on SIGTERM: 0 lobby, 1 match, 0 voice flows and 0 login connections open\n'
            rf'match 127\.0\.0\.1:{match[1]} at stop: \d+\.\d s, '
            r'up 1 packets 4 bytes, down 1 packets 4 bytes\n$')

    def test_a_server_that_is_away_is_said(self):
        # The origin's login and match ports closed: srp down.
        for s in (self.sockets[0], self.sockets[2]):
            s.close()
        # Gone once the backend threads' waits (0.1 s) return.
        time.sleep(0.3)
        with socket.create_connection(('127.0.0.2', self.port), timeout=2) as client:
            login = client.getsockname()
            self.assertEqual(client.recv(5), b'')
        self.assertTrue(select.select([self.process.stderr], [], [], 2)[0])
        self.assertEqual(self.process.stderr.readline().decode(),
            f'login 127.0.0.1:{login[1]} not forwarded: the server 127.0.0.1:{self.port} '
            'not reached: Connection refused (os error 111)\n')
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as client:
            client.bind(('127.0.0.1', 0))
            for _ in range(2):
                client.sendto(b'ping', ('127.0.0.2', self.port+2))
                time.sleep(0.1)
        self.assertTrue(select.select([self.process.stderr], [], [], 2)[0])
        self.assertEqual(self.process.stderr.readline().decode(),
            "match: the server's socket failed: Connection refused (os error 111); 1 such so far\n")

    def test_all_udp_ports_multiple_clients_and_unobserved_destination(self):
        for port in range(self.port+1, self.port+4):
            for i in range(3):
                with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as client:
                    client.bind(('127.0.0.1', 0))
                    client.settimeout(2)
                    payload = bytes([i]) + b'\x00\xff\x00\xd8' * 100
                    client.sendto(payload, ('127.0.0.2', port))
                    reply, source = client.recvfrom(8192)
                    self.assertEqual((reply, source), (payload, ('127.0.0.2', port)))
                    _, wire, upstream, backend = self.observed.get(timeout=2)
                    self.assertEqual(f.parse(wire, '203.0.113.8', port), (client.getsockname(), payload))
            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as stranger:
                stranger.bind(('127.0.0.1', 0))
                stranger.settimeout(0.05)
                backend.sendto(f.header(stranger.getsockname(), '203.0.113.8', port) + b'forged', upstream)
                with self.assertRaises(TimeoutError):
                    stranger.recvfrom(8192)

    def test_sustained_packets_have_no_loss_or_cross_client_delivery(self):
        # 900 packets across lobby/match/voice, paced below each IP budget.
        clients = [socket.socket(socket.AF_INET, socket.SOCK_DGRAM) for _ in range(3)]
        try:
            for client in clients:
                client.bind(('127.0.0.1', 0))
                client.settimeout(1)
            for sequence in range(300):
                for i, client in enumerate(clients):
                    payload = sequence.to_bytes(4, 'little') + bytes([i]) * 400
                    client.sendto(payload, ('127.0.0.2', self.port+1+i))
                    reply, _ = client.recvfrom(8192)
                    self.assertEqual(reply, payload)
                time.sleep(0.001)
        finally:
            for client in clients:
                client.close()
