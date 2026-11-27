import asyncio
import socket
import unittest

import forwarder as f


class WireTests(unittest.TestCase):
    def test_address_and_payload_roundtrip(self):
        for client in [('198.51.100.2', 23456), ('198.51.100.3', 23456)]:
            wire = f.header(client, '203.0.113.4', 1236) + b'\x00\xff\x00\xd8'
            self.assertEqual(f.parse(wire, '203.0.113.4', 1236), (client, b'\x00\xff\x00\xd8'))
            for size in range(f.HEADER.size):
                with self.assertRaises(ValueError):
                    f.parse(wire[:size], '203.0.113.4', 1236)
            for index in list(range(16)) + list(range(20, 24)) + [26, 27]:
                bad = bytearray(wire)
                bad[index] ^= 1
                with self.assertRaises(ValueError):
                    f.parse(bad, '203.0.113.4', 1236)

    def test_capacity_and_idle_do_not_evict_active_clients(self):
        peers = f.Peers(limit=2)
        self.assertTrue(peers.touch('a', 0))
        self.assertTrue(peers.touch('b', 1))
        self.assertFalse(peers.touch('c', 2))
        self.assertTrue(peers.touch('a', 119))
        self.assertTrue(peers.touch('c', 122))
        self.assertTrue(peers.contains('a', 122))
        self.assertFalse(peers.contains('b', 122))

    def test_one_address_cannot_hold_every_slot(self):
        slots = f.Slots(limit=3, each=2)
        self.assertTrue(slots.take('a'))
        self.assertTrue(slots.take('a'))
        self.assertFalse(slots.take('a'))
        self.assertTrue(slots.take('b'))
        self.assertFalse(slots.take('c'))
        slots.free('a')
        self.assertTrue(slots.take('c'))
        for ip in 'abc':
            slots.free(ip)
        self.assertEqual(slots.held, {})

    def test_budget(self):
        budget = f.Budget(rate=2, burst=2)
        self.assertTrue(budget.take('a', 0))
        self.assertTrue(budget.take('a', 0))
        self.assertFalse(budget.take('a', 0))
        self.assertTrue(budget.take('a', 0.5))
        self.assertTrue(budget.take('b', 0.5))


class SocketTests(unittest.IsolatedAsyncioTestCase):
    async def test_https_connect_is_fixed_origin_and_byte_transparent(self):
        from unittest.mock import patch
        async def origin(reader, writer):
            writer.write((await reader.readexactly(5)).upper())
            await writer.drain()
            writer.close()
        backend = await asyncio.start_server(origin, '127.0.0.1', 0)
        open_connection = asyncio.open_connection
        calls = []
        async def connect(host, port):
            calls.append((host, port))
            return await open_connection('127.0.0.1', backend.sockets[0].getsockname()[1])
        proxy = f.HTTPS('203.0.113.8', 'skhid.link', 443)
        server = await asyncio.start_server(proxy.accept, '127.0.0.1', 0, limit=4096)
        try:
            with patch('forwarder.asyncio.open_connection', new=connect):
                for authority, status in [('elsewhere.example:443', b'403'), ('skhid.link:22', b'403'), ('skhid.link:443', b'200')]:
                    reader, writer = await open_connection('127.0.0.1', server.sockets[0].getsockname()[1])
                    writer.write(f'CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n\r\n'.encode())
                    await writer.drain()
                    answer = await asyncio.wait_for(reader.readuntil(b'\r\n\r\n'), 2)
                    self.assertIn(status, answer)
                    if status == b'200':
                        writer.write(b'hello')
                        await writer.drain()
                        self.assertEqual(await asyncio.wait_for(reader.readexactly(5), 2), b'HELLO')
                    writer.close()
                    await writer.wait_closed()
            self.assertEqual(calls, [('skhid.link', 443)])
        finally:
            server.close()
            backend.close()
            await server.wait_closed()
            await backend.wait_closed()

    async def test_a_download_longer_than_the_idle_time_is_not_cut(self):
        from unittest.mock import patch
        async def origin(reader, writer):
            for part in b'abcdefgh':
                await asyncio.sleep(0.1)
                writer.write(bytes([part]))
                await writer.drain()
            writer.close()
        backend = await asyncio.start_server(origin, '127.0.0.1', 0)
        open_connection = asyncio.open_connection
        async def connect(_host, _port):
            return await open_connection('127.0.0.1', backend.sockets[0].getsockname()[1])
        proxy = f.HTTPS('203.0.113.8', 'skhid.link', 443)
        server = await asyncio.start_server(proxy.accept, '127.0.0.1', 0, limit=4096)
        try:
            with patch('forwarder.asyncio.open_connection', new=connect), patch('forwarder.IDLE', 0.5):
                reader, writer = await open_connection('127.0.0.1', server.sockets[0].getsockname()[1])
                writer.write(b'CONNECT skhid.link:443 HTTP/1.1\r\n\r\n')
                await asyncio.wait_for(reader.readuntil(b'\r\n\r\n'), 2)
                self.assertEqual(await asyncio.wait_for(reader.read(-1), 3), b'abcdefgh')
                writer.close()
                await writer.wait_closed()
        finally:
            server.close()
            backend.close()
            await server.wait_closed()
            await backend.wait_closed()

    async def test_tcp_header_and_bidirectional_stream(self):
        observed = []
        async def backend(reader, writer):
            observed.append(await reader.readexactly(28))
            data = await reader.readexactly(5)
            writer.write(data.upper())
            await writer.drain()
            writer.close()
        origin = await asyncio.start_server(backend, '127.0.0.1', 0)
        port = origin.sockets[0].getsockname()[1]
        login = f.Login('203.0.113.4', '127.0.0.1', port)
        relay = await asyncio.start_server(login.accept, '127.0.0.1', 0)
        try:
            with self.assertLogs(level='INFO') as logs:
                reader, writer = await asyncio.open_connection('127.0.0.1', relay.sockets[0].getsockname()[1])
                client = writer.get_extra_info('sockname')
                writer.write(b'hello')
                await writer.drain()
                self.assertEqual(await asyncio.wait_for(reader.readexactly(5), 2), b'HELLO')
                self.assertEqual(observed, [f.header(client, '203.0.113.4', port, 0x11)])
                writer.close()
                await writer.wait_closed()
                for _ in range(40):
                    if logs.records:
                        break
                    await asyncio.sleep(0.05)
            self.assertRegex(logs.output[0],
                rf'login 127\.0\.0\.1:{client[1]}: \d+\.\d s, up 5 bytes, down 5 bytes, closed$')
        finally:
            relay.close()
            origin.close()
            await relay.wait_closed()
            await origin.wait_closed()

    async def test_a_closed_connection_gives_its_slot_back(self):
        async def backend(reader, writer):
            await reader.readexactly(28)
            while data := await reader.read(16):
                writer.write(data)
                await writer.drain()
            writer.close()
        origin = await asyncio.start_server(backend, '127.0.0.1', 0)
        login = f.Login('203.0.113.4', '127.0.0.1', origin.sockets[0].getsockname()[1])
        login.slots = f.Slots(limit=8, each=1)
        relay = await asyncio.start_server(login.accept, '127.0.0.1', 0)
        port = relay.sockets[0].getsockname()[1]
        opened = []
        async def echo(source):
            """The byte back, or nothing from a relay that closed at once."""
            reader, writer = await asyncio.open_connection('127.0.0.1', port, local_addr=(source, 0))
            opened.append(writer)
            try:
                writer.write(b'x')
                await writer.drain()
                return await asyncio.wait_for(reader.read(1), 2)
            except ConnectionError:
                return b''
        try:
            self.assertEqual(await echo('127.0.0.1'), b'x')
            self.assertEqual(await echo('127.0.0.1'), b'')
            self.assertEqual(await echo('127.0.0.2'), b'x')
            opened[0].close()
            for _ in range(5):
                if again := await echo('127.0.0.1'):
                    break
                await asyncio.sleep(0.1)
            self.assertEqual(again, b'x')
        finally:
            for writer in opened:
                writer.close()
            relay.close()
            origin.close()
            await relay.wait_closed()
            await origin.wait_closed()

    async def wait_for_logs(self, logs, count):
        for _ in range(40):
            if len(logs.records) >= count:
                return
            await asyncio.sleep(0.05)

    async def test_an_unreached_server_and_a_refusal_are_said(self):
        closed = socket.socket()
        closed.bind(('127.0.0.1', 0))
        port = closed.getsockname()[1]
        closed.close()
        login = f.Login('203.0.113.4', '127.0.0.1', port)
        relay = await asyncio.start_server(login.accept, '127.0.0.1', 0)
        try:
            with self.assertLogs(level='INFO') as logs:
                reader, writer = await asyncio.open_connection('127.0.0.1', relay.sockets[0].getsockname()[1])
                client = writer.get_extra_info('sockname')
                self.assertEqual(await asyncio.wait_for(reader.read(), 2), b'')
                writer.close()
                await self.wait_for_logs(logs, 1)
                login.budget = f.Budget(rate=0, burst=0)
                for _ in range(10):
                    reader, writer = await asyncio.open_connection('127.0.0.1', relay.sockets[0].getsockname()[1])
                    self.assertEqual(await asyncio.wait_for(reader.read(), 2), b'')
                    writer.close()
                await self.wait_for_logs(logs, 3)
            self.assertRegex(logs.output[0],
                rf'login 127\.0\.0\.1:{client[1]}: \d+\.\d s, up 0 bytes, down 0 bytes, '
                rf'not forwarded: the server 127\.0\.0\.1:{port} not reached: \[Errno 111\] Connect call failed')
            self.assertRegex(logs.output[1], r'WARNING:root:login 127\.0\.0\.1:\d+ refused: over the '
                r'allowance of its address \(0/s, burst 0\); 1 such so far$')
            self.assertRegex(logs.output[2], r'; 10 such so far$')
            self.assertEqual(len(logs.output), 3)
        finally:
            relay.close()
            await relay.wait_closed()

    async def test_a_stop_ends_the_open_connections_and_says_so(self):
        import os
        import signal
        held = asyncio.Event()
        async def backend(reader, writer):
            await reader.readexactly(28)
            held.set()
            await reader.read()
            writer.close()
        origin = await asyncio.start_server(backend, '127.0.0.1', 0)
        login = f.Login('203.0.113.4', '127.0.0.1', origin.sockets[0].getsockname()[1])
        relay = await asyncio.start_server(login.accept, '127.0.0.1', 0)
        try:
            with self.assertLogs(level='INFO') as logs:
                stopping = asyncio.create_task(f.stop_on_signal((relay,), (login,)))
                reader, writer = await asyncio.open_connection('127.0.0.1', relay.sockets[0].getsockname()[1])
                client = writer.get_extra_info('sockname')
                await asyncio.wait_for(held.wait(), 2)
                await asyncio.sleep(0)
                os.kill(os.getpid(), signal.SIGTERM)
                await asyncio.wait_for(stopping, 2)
                self.assertEqual(await asyncio.wait_for(reader.read(), 2), b'')
                writer.close()
            self.assertEqual(logs.output[0], 'INFO:root:stopping on SIGTERM: 1 login connections open')
            self.assertRegex(logs.output[1],
                rf'login 127\.0\.0\.1:{client[1]}: \d+\.\d s, up 0 bytes, down 0 bytes, at stop, no reply$')
        finally:
            loop = asyncio.get_running_loop()
            for sig in (signal.SIGTERM, signal.SIGINT):
                loop.remove_signal_handler(sig)
            relay.close()
            origin.close()
            await relay.wait_closed()
            await origin.wait_closed()

    async def test_udp_two_clients_and_unknown_reply_is_dropped(self):
        loop = asyncio.get_running_loop()
        received = asyncio.Queue()
        class Origin(asyncio.DatagramProtocol):
            def connection_made(self, transport):
                self.transport = transport
            def datagram_received(self, data, peer):
                received.put_nowait((data, peer))
                self.transport.sendto(data, peer)
        origin, _ = await loop.create_datagram_endpoint(Origin, local_addr=('127.0.0.1', 0))
        port = origin.get_extra_info('sockname')[1]
        front = f.PublicUDP('203.0.113.4', port)
        backend_protocol = f.BackendUDP(front)
        backend, _ = await loop.create_datagram_endpoint(lambda: backend_protocol,
            remote_addr=('127.0.0.1', port))
        front.backend = backend
        relay, _ = await loop.create_datagram_endpoint(lambda: front, local_addr=('127.0.0.1', 0))
        clients = [socket.socket(socket.AF_INET, socket.SOCK_DGRAM) for _ in range(2)]
        try:
            for client in clients:
                client.bind(('127.0.0.1', 0))
                client.setblocking(False)
                await loop.sock_sendto(client, b'game', relay.get_extra_info('sockname'))
                answer, _ = await asyncio.wait_for(loop.sock_recvfrom(client, 128), 2)
                self.assertEqual(answer, b'game')
                data, peer = await received.get()
                self.assertEqual(f.parse(data, front.public, port), (client.getsockname(), b'game'))
            outsider = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            outsider.bind(('127.0.0.1', 0))
            outsider.setblocking(False)
            try:
                backend_protocol.datagram_received(f.header(outsider.getsockname(), front.public, port) + b'bad', peer)
                with self.assertRaises(TimeoutError):
                    await asyncio.wait_for(loop.sock_recvfrom(outsider, 128), 0.05)
            finally:
                outsider.close()
        finally:
            for client in clients:
                client.close()
            for transport in (relay, backend, origin):
                transport.close()


if __name__ == '__main__':
    unittest.main()
