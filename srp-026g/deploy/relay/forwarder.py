#!/usr/bin/env python3
"""IPv4 game relay. The backend MUST be reached over the private WireGuard link.

PROXY v2 carries the observed client address; the game stream stays encrypted
end to end. UDP replies use the same envelope with the client's address in its
source field, and are sent only to recently observed clients on that port.
"""
import argparse
import asyncio
from collections import OrderedDict
import ipaddress
import logging
import signal
import socket
import struct
import time

SIGNATURE = b"\r\n\r\n\0\r\nQUIT\n"
HEADER = struct.Struct("!12sBBH4s4sHH")
MAX_DATAGRAM = 4096
MAX_PEERS = 8192
IDLE = 120


def ipv4(text):
    ip = ipaddress.IPv4Address(text)
    if ip.is_unspecified or ip.is_multicast or int(ip) == 0xffffffff:
        raise ValueError("expected a unicast IPv4 address")
    return str(ip)


def header(client, public, port, protocol=0x12):
    return HEADER.pack(SIGNATURE, 0x21, protocol, 12,
                       socket.inet_aton(client[0]), socket.inet_aton(public), client[1], port)


def parse(packet, public, port):
    if len(packet) < HEADER.size:
        raise ValueError("truncated PROXY v2 header")
    sig, command, protocol, length, src, dst, sport, dport = HEADER.unpack_from(packet)
    if (sig, command, protocol, length, dst, dport) != (
            SIGNATURE, 0x21, 0x12, 12, socket.inet_aton(public), port) or sport == 0:
        raise ValueError("invalid PROXY v2 header")
    return (ipv4(socket.inet_ntoa(src)), sport), packet[HEADER.size:]


class Peers:
    """Bounded idle table. A full table retains existing peers until idle."""
    def __init__(self, limit=MAX_PEERS):
        self.limit = limit
        self.seen = OrderedDict()

    def expire(self, now):
        while self.seen and next(iter(self.seen.values())) + IDLE <= now:
            self.seen.popitem(last=False)

    def touch(self, address, now):
        self.expire(now)
        if address not in self.seen and len(self.seen) >= self.limit:
            return False
        self.seen[address] = now
        self.seen.move_to_end(address)
        return True

    def contains(self, address, now):
        self.expire(now)
        return address in self.seen


class Budget:
    """Bounded per-IP token buckets, used before allocating a UDP peer."""
    def __init__(self, rate=1000, burst=2000):
        self.rate, self.burst = rate, burst
        self.buckets = OrderedDict()

    def take(self, ip, now):
        while self.buckets and next(iter(self.buckets.values()))[1] + IDLE <= now:
            self.buckets.popitem(last=False)
        if ip not in self.buckets and len(self.buckets) >= MAX_PEERS:
            return False
        tokens, last = self.buckets.get(ip, (self.burst, now))
        tokens = min(self.burst, tokens + (now - last) * self.rate)
        self.buckets[ip] = (max(0, tokens - 1), now)
        self.buckets.move_to_end(ip)
        return tokens >= 1


class PublicUDP(asyncio.DatagramProtocol):
    def __init__(self, public, port):
        self.public, self.port = public, port
        self.peers = Peers()
        self.budget = Budget()
        self.transport = None
        self.backend = None

    def connection_made(self, transport):
        self.transport = transport

    def datagram_received(self, data, address):
        now = time.monotonic()
        if (len(data) > MAX_DATAGRAM or not self.budget.take(address[0], now)
                or not self.peers.touch(address, now)):
            return
        self.backend.sendto(header(address, self.public, self.port) + data)

    def error_received(self, error):
        logging.debug("public UDP %s: %s", self.port, error)


class BackendUDP(asyncio.DatagramProtocol):
    def __init__(self, front):
        self.front = front

    def datagram_received(self, data, _address):
        try:
            client, payload = parse(data, self.front.public, self.front.port)
        except ValueError:
            return
        if len(payload) <= MAX_DATAGRAM and self.front.peers.contains(client, time.monotonic()):
            self.front.transport.sendto(payload, client)

    def error_received(self, error):
        logging.debug("backend UDP %s: %s", self.front.port, error)


class Login:
    def __init__(self, public, backend, port, limit=256):
        self.public, self.backend, self.port = public, backend, port
        self.limit, self.active = limit, 0
        self.budget = Budget(rate=2, burst=10)

    async def accept(self, reader, writer):
        client = writer.get_extra_info("peername")
        if self.active >= self.limit or not self.budget.take(client[0], time.monotonic()):
            writer.close()
            return
        self.active += 1
        upstream = None
        try:
            remote, upstream = await asyncio.wait_for(
                asyncio.open_connection(self.backend, self.port, family=socket.AF_INET), 10)
            upstream.write(header(client, self.public, self.port, 0x11))
            await upstream.drain()

            async def copy(source, target):
                while data := await asyncio.wait_for(source.read(65536), IDLE):
                    target.write(data)
                    await asyncio.wait_for(target.drain(), 10)
                if target.can_write_eof():
                    target.write_eof()

            # Both halves finish naturally on EOF; errors cancel the sibling.
            async with asyncio.TaskGroup() as group:
                group.create_task(copy(reader, upstream))
                group.create_task(copy(remote, writer))
        except (OSError, TimeoutError, ExceptionGroup) as error:
            logging.debug("login from %s failed: %s", client, error)
        finally:
            self.active -= 1
            writer.close()
            if upstream:
                upstream.close()


class HTTPS(Login):
    """CONNECT to one configured HTTPS origin, never an open proxy."""
    async def accept(self, reader, writer):
        client = writer.get_extra_info("peername")
        if self.active >= self.limit or not self.budget.take(client[0], time.monotonic()):
            writer.close()
            return
        self.active += 1
        upstream = None
        try:
            request = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 10)
            expected = f"CONNECT {self.backend}:443 HTTP/1.1".encode()
            if len(request) > 4096 or request.split(b"\r\n", 1)[0] != expected:
                writer.write(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                await writer.drain()
                return
            remote, upstream = await asyncio.wait_for(asyncio.open_connection(self.backend, 443), 10)
            writer.write(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            await writer.drain()

            async def copy(source, target):
                while data := await asyncio.wait_for(source.read(65536), IDLE):
                    target.write(data)
                    await asyncio.wait_for(target.drain(), 10)
                if target.can_write_eof():
                    target.write_eof()

            async with asyncio.TaskGroup() as group:
                group.create_task(copy(reader, upstream))
                group.create_task(copy(remote, writer))
        except (OSError, TimeoutError, asyncio.IncompleteReadError, asyncio.LimitOverrunError, ExceptionGroup) as error:
            logging.debug("HTTPS tunnel from %s failed: %s", client, error)
        finally:
            self.active -= 1
            writer.close()
            if upstream:
                upstream.close()


async def run_https(args):
    proxy = HTTPS(args.public, args.https_origin, 443, limit=128)
    server = await asyncio.start_server(proxy.accept, args.bind, 8080, family=socket.AF_INET, limit=4096)
    logging.info("HTTPS relay ready for %s:443", args.https_origin)
    stopped = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, stopped.set)
    async with server:
        await stopped.wait()


async def run(args):
    loop = asyncio.get_running_loop()
    transports = []
    login = Login(args.public, args.backend, args.login_port)
    server = await asyncio.start_server(login.accept, args.bind, args.login_port, family=socket.AF_INET)
    https = HTTPS(args.public, args.https_origin, 443, limit=128)
    proxy = await asyncio.start_server(https.accept, args.bind, 8080, family=socket.AF_INET, limit=4096)
    try:
        for port in (args.lobby_port, args.match_port, args.match_port + 1):
            front = PublicUDP(args.public, port)
            backend, _ = await loop.create_datagram_endpoint(
                lambda: BackendUDP(front), remote_addr=(args.backend, port), family=socket.AF_INET)
            transports.append(backend)
            front.backend = backend
            transport, _ = await loop.create_datagram_endpoint(
                lambda: front, local_addr=(args.bind, port), family=socket.AF_INET)
            transports.append(transport)
        logging.info("relay %s -> %s ready: login %s, lobby %s, match %s, voice %s",
                     args.public, args.backend, args.login_port, args.lobby_port,
                     args.match_port, args.match_port + 1)
        stopped = asyncio.Event()
        for sig in (signal.SIGTERM, signal.SIGINT):
            loop.add_signal_handler(sig, stopped.set)
        await stopped.wait()
    finally:
        server.close()
        proxy.close()
        await server.wait_closed()
        await proxy.wait_closed()
        for transport in transports:
            transport.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--public", required=True, type=ipv4)
    parser.add_argument("--https-only", action="store_true")
    parser.add_argument("--backend", required=True, type=ipv4)
    parser.add_argument("--bind", default="0.0.0.0")
    parser.add_argument("--https-origin", default="skhid.link")
    parser.add_argument("--login-port", default=1234, type=int)
    parser.add_argument("--lobby-port", default=1235, type=int)
    parser.add_argument("--match-port", default=1236, type=int)
    args = parser.parse_args()
    if not ipaddress.IPv4Address(args.backend).is_private:
        parser.error("backend must be the private tunnel address")
    ports = [args.login_port, args.lobby_port, args.match_port, args.match_port + 1]
    if any(not 1024 <= p <= 65535 for p in ports) or len(set(ports)) != 4:
        parser.error("use four distinct unprivileged ports (voice is match + 1)")
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
    asyncio.run(run_https(args) if args.https_only else run(args))


if __name__ == "__main__":
    main()
