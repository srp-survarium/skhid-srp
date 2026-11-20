#!/usr/bin/env python3
"""The site through a relay, against a real Caddy (`caddy` on PATH) started with
the compose file's own caddy26 command: the page is told the visitor's address
only by a relay of SRP_RELAYS, never by a visitor's own PROXY header.

127.0.0.2 stands for the relay's tunnel address, 127.0.0.3 for a visitor of the
relay, 127.0.0.1 for a visitor who comes straight to the origin.
"""
import asyncio
import os
import pathlib
import re
import socket
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import forwarder as f

COMPOSE = pathlib.Path(__file__).resolve().parents[2] / "docker-compose.yml"
FORGED = ("198.51.100.7", 40000)
open_connection = asyncio.open_connection


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def caddy_command(caddyfile, page):
    """caddy26's shell command, as compose hands it to sh."""
    folded = re.search(r"\n  caddy26:\n.*?\n      - >-\n(.*?)\n    ports:", COMPOSE.read_text(), re.S).group(1)
    command = " ".join(line.strip() for line in folded.splitlines()).replace("$$", "$")
    for inside, here in (("/tmp/Caddyfile", caddyfile), ("srp26:8088", f"127.0.0.1:{page}")):
        assert inside in command, inside
        command = command.replace(inside, here)
    return command


async def page(reader, writer):
    """Answers with the address srp would take: the last X-Forwarded-For entry.
    Only a request for the path alone is answered, as srp reads it."""
    head = (await reader.readuntil(b"\r\n\r\n")).decode()
    seen = re.search(r"(?im)^x-forwarded-for:.*?([^,\s]+)\s*$", head)
    body = seen.group(1).encode() if seen else b"none"
    status = b"200 OK" if head.startswith("GET / HTTP/1.1\r\n") else b"400 Bad Request"
    writer.write(b"HTTP/1.1 %s\r\nConnection: close\r\nContent-Length: %d\r\n\r\n%s" % (status, len(body), body))
    await writer.drain()
    writer.close()


async def visit(port, source, to="127.0.0.1", first=b"", target=b"/"):
    """The address the page saw, or None where the visitor got no page."""
    try:
        reader, writer = await open_connection(to, port, local_addr=(source, 0))
        writer.write(first + b"GET %s HTTP/1.1\r\nHost: site\r\nConnection: close\r\n\r\n" % target)
        await writer.drain()
        answer = await asyncio.wait_for(reader.read(-1), 10)
        writer.close()
    except (OSError, TimeoutError):
        return None
    head, _, body = answer.partition(b"\r\n\r\n")
    return body.decode() if head.startswith(b"HTTP/1.1 200") else None


class SiteTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.addCleanup(self.folder.cleanup)
        self.page = await asyncio.start_server(page, "127.0.0.1", 0)
        self.port = free_port()

    async def caddy(self, relays):
        home = self.folder.name
        env = dict(os.environ, HOME=home, XDG_DATA_HOME=home, XDG_CONFIG_HOME=home,
                   CADDY_ADMIN=f"unix/{home}/admin", SRP_RELAYS=relays, SRP_WEB_NAMES=f"http://:{self.port}")
        command = caddy_command(f"{home}/Caddyfile", self.page.sockets[0].getsockname()[1])
        process = subprocess.Popen(["sh", "-c", command], env=env, stderr=subprocess.PIPE)
        self.addCleanup(process.wait)
        self.addCleanup(process.stderr.close)
        self.addCleanup(process.terminate)
        for _ in range(100):
            if await visit(self.port, "127.0.0.1") is not None:
                return
            self.assertIsNone(process.poll(), process.stderr.read().decode() if process.poll() is not None else "")
            await asyncio.sleep(0.1)
        self.fail("caddy did not start")

    async def asyncTearDown(self):
        self.page.close()
        await self.page.wait_closed()

    async def test_the_page_sees_a_relay_visitor_and_no_forged_address(self):
        await self.caddy("127.0.0.2=203.0.113.4")
        forged = f.header(FORGED, "203.0.113.4", 443, 0x11)
        self.assertEqual(await visit(self.port, "127.0.0.1"), "127.0.0.1")
        self.assertIn(await visit(self.port, "127.0.0.1", first=forged), (None, "127.0.0.1"))

        async def from_the_tunnel(host, port, **how):
            return await open_connection(host, port, local_addr=("127.0.0.2", 0), **how)
        site = f.Login("203.0.113.4", "127.0.0.1", self.port)
        relay = await asyncio.start_server(site.accept, "127.0.0.1", 0, family=socket.AF_INET)
        try:
            port = relay.sockets[0].getsockname()[1]
            with patch("forwarder.asyncio.open_connection", new=from_the_tunnel):
                self.assertEqual(await visit(port, "127.0.0.3"), "127.0.0.3")
                # Through the relay a forged header is the second one: page bytes, never an address.
                self.assertIn(await visit(port, "127.0.0.3", first=forged), (None, "127.0.0.3"))
        finally:
            relay.close()
            await relay.wait_closed()

    async def test_a_request_with_the_whole_address_is_the_page_too(self):
        # Wine's WinHTTP asks a site behind a proxy (the relay's CONNECT
        # service, the updater's way through it) with the whole address in
        # the request line, inside the tunnel too. Caddy takes it and asks
        # srp for the path alone.
        await self.caddy("")
        whole = b"http://site:%d/" % self.port
        self.assertEqual(await visit(self.port, "127.0.0.1", target=whole), "127.0.0.1")

    async def test_without_relays_no_header_is_read(self):
        await self.caddy("")
        self.assertEqual(await visit(self.port, "127.0.0.1"), "127.0.0.1")
        self.assertIsNone(await visit(self.port, "127.0.0.2", to="127.0.0.2",
                                      first=f.header(FORGED, "203.0.113.4", 443, 0x11)))


if __name__ == "__main__":
    unittest.main()
