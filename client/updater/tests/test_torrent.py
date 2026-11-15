"""End-to-end tests of BitTorrent in the updater (`exe/src/torrent.rs`,
libtorrent in `skhid_torrent.dll`; `srp-026g/docs/notes/torrent-downloads.md`):
the real updater_skhid.exe and its DLL, started by the stand-in launcher with
its "Allow torrent" and port (the launcher's 4th and 13th arguments), against
the web server of the other tests, whose ZIP has the revision's torrent
beside the updater (`skhid.torrent`, signed with a throwaway key), a tracker
on 127.0.0.1 (HTTP and UDP), a DHT of libtorrent nodes on 127.0.0.1 (BEP 46:
the release's item), and libtorrent (2.0, Python bindings) as the other
peers: a seed, a leech that takes the folder from the updater, and a peer
that sends wrong bytes (a few lines of Python).

    cd client/updater
    ./build.sh
    python tests/test_torrent.py [-v] [TestName ...]

Windows, or Linux under Wine (as test_db_patches.py, whose harness this
uses). The updater must be built to trust the tests' key and to ask the
tests' trackers and DHT (it asks only its own, never a torrent's):

    SKHID_TEST_PUBLIC_KEY=$(python tests/test_torrent.py --key) \\
        SKHID_TEST_TRACKERS="$(python tests/test_torrent.py --trackers)" \\
        SKHID_TEST_DHT=$(python tests/test_torrent.py --dht) ./build.sh

Python needs libtorrent and cryptography, for example
`nix build --impure --expr '(import <nixpkgs> {}).python3.withPackages (p: [ p.libtorrent-rasterbar p.cryptography ])'`;
the release's item is put with the release's own tool, skhid-dht
(`nix build ./srp-026g#skhid-dht`; SKHID_DHT_TOOL=<its bin/skhid-dht>, else
those tests are skipped). The torrents are made by libtorrent itself (hybrid
v1 + v2, a pad after every file), so the updater reads a real client's
torrent. Nothing leaves this computer: no LSD, UPnP or NAT-PMP, the DHT's
nodes on 127.0.0.1 alone.
"""

import hashlib
import http.server
import os
import random
import re
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import urllib.parse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import test_db_patches as wine  # noqa: E402
from test_db_patches import X86, sha256, windows, windows_path  # noqa: E402
from test_play_gate import game_exe  # noqa: E402

try:
    import libtorrent as lt
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
except ImportError:
    lt = None

# The tests' throwaway signing key (never a release's).
SEED = bytes(range(32))
DOMAIN = b"skhid torrent signature 1\0"
# The release's DHT item (`logic/src/newest.rs`).
SALT = b"skhid-newest 1"


def public_key_hex():
    from cryptography.hazmat.primitives import serialization
    key = Ed25519PrivateKey.from_private_bytes(SEED).public_key()
    return key.public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw).hex()


def item_value(info_sha256, revision, signature):
    """The DHT item's value: the facts of the torrent's signature."""
    return b"d1:i32:" + info_sha256 + b"1:ri%de1:s64:" % revision + signature + b"e"


def item_signed(value, seq):
    """What BEP 44 signs."""
    return b"4:salt%d:%s3:seqi%de1:v" % (len(SALT), SALT, seq) + value


def sign(entry, revision):
    """The release's signatures (`logic/src/signed.rs`, `newest.rs`) added
    to `entry`: the torrent's, and its DHT item's."""
    key = Ed25519PrivateKey.from_private_bytes(SEED)
    info = hashlib.sha256(lt.bencode(entry[b"info"])).digest()
    signature = key.sign(DOMAIN + bytes([revision]) + info)
    dht = key.sign(item_signed(item_value(info, revision, signature), revision))
    entry[b"skhid"] = {b"dht": dht, b"revision": revision, b"signature": signature}


def item_of(torrent):
    """The DHT item of a signed torrent: seq, value, signature."""
    entry = lt.bdecode(torrent)
    skhid = entry[b"skhid"]
    info = hashlib.sha256(lt.bencode(entry[b"info"])).digest()
    revision = skhid[b"revision"]
    return revision, item_value(info, revision, skhid[b"signature"]), skhid[b"dht"]


harness = wine.harness
LATEST, ERROR = harness.LATEST, harness.ERROR
TORRENT_CHECKING, DOWNLOADING = 0x08, 0x09
PIECE = 32 * 1024
DHT_TOOL = os.environ.get("SKHID_DHT_TOOL")


def setUpModule():
    if lt is None:
        raise unittest.SkipTest("no libtorrent or cryptography in this Python (see the docstring)")
    with open(harness.UPDATER, "rb") as file:
        built = file.read()
        if any(text.encode() not in built for text in (public_key_hex(), TRACKERS, DHT)):
            raise unittest.SkipTest("the updater is not built with the tests' key, trackers and DHT (see the docstring)")
    if not os.path.isfile(harness.TORRENT_DLL):
        raise unittest.SkipTest(f"no {harness.TORRENT_DLL}: run build.sh")
    wine.setUpModule()


def tearDownModule():
    global NODES
    if NODES is not None:
        NODES.close()
        NODES = None
    wine.tearDownModule()


def free_port():
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def bencode(value):
    if isinstance(value, int):
        return b"i%de" % value
    if isinstance(value, bytes):
        return b"%d:%s" % (len(value), value)
    if isinstance(value, dict):
        return b"d" + b"".join(bencode(k) + bencode(v) for k, v in sorted(value.items())) + b"e"
    raise TypeError(value)


def compact(peers):
    return b"".join(socket.inet_aton(ip) + struct.pack(">H", port) for ip, port in peers)


# The tests' trackers and the DHT's first node, at fixed ports: the updater
# is built with them (`SKHID_TEST_TRACKERS`, `SKHID_TEST_DHT`).
UDP_PORT, HTTP_PORT, DHT_PORT = 47613, 47614, 47615
TRACKERS = f"udp://127.0.0.1:{UDP_PORT}/announce http://127.0.0.1:{HTTP_PORT}/announce"
DHT = f"127.0.0.1:{DHT_PORT}"
class Tracker:
    """An HTTP tracker and a UDP one (BEP 15) on 127.0.0.1, the updater's
    (`TRACKERS`): both answer `peers`; `asked` says who asked how, with the
    port they said."""

    def __init__(self):
        self.peers = []
        self.asked = []
        tracker = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_GET(self):
                query = urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query, encoding="latin-1")
                tracker.asked.append(("http", int(query["port"][0])))
                body = bencode({b"interval": 60, b"peers": compact(tracker.peers)})
                self.send_response(200)
                self.send_header("Content-Type", "text/plain")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

        self.http = http.server.ThreadingHTTPServer(("127.0.0.1", HTTP_PORT), Handler)
        threading.Thread(target=self.http.serve_forever, daemon=True).start()
        self.udp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.udp.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.udp.bind(("127.0.0.1", UDP_PORT))
        self.udp.settimeout(0.2)
        self.running = True
        threading.Thread(target=self.serve_udp, daemon=True).start()
        self.urls = [f"udp://127.0.0.1:{self.udp.getsockname()[1]}/announce",
                     f"http://127.0.0.1:{self.http.server_address[1]}/announce"]

    def serve_udp(self):
        while self.running:
            try:
                data, sender = self.udp.recvfrom(2048)
            except (socket.timeout, OSError):
                continue
            if len(data) == 16:
                transaction = data[12:16]
                self.udp.sendto(struct.pack(">I", 0) + transaction + struct.pack(">Q", 0x1234), sender)
            elif len(data) >= 98:
                transaction = data[12:16]
                self.asked.append(("udp", struct.unpack(">H", data[96:98])[0]))
                head = struct.pack(">I", 1) + transaction + struct.pack(">III", 60, 0, len(self.peers))
                self.udp.sendto(head + compact(self.peers), sender)

    def close(self):
        self.running = False
        self.http.shutdown()
        self.http.server_close()
        self.udp.close()


def session(port):
    """A libtorrent session of these tests: 127.0.0.1 only, TCP only."""
    return lt.session({
        "listen_interfaces": f"127.0.0.1:{port}",
        "enable_dht": False, "enable_lsd": False, "enable_upnp": False, "enable_natpmp": False,
        "enable_outgoing_utp": False, "enable_incoming_utp": False,
        "out_enc_policy": 2, "in_enc_policy": 1,
        "allow_multiple_connections_per_ip": True,
        "alert_mask": 0,
    })


class Nodes:
    """A DHT of libtorrent nodes on 127.0.0.1, the first on DHT_PORT (the
    updater's way in): `get` reads the release's item from it."""

    def __init__(self):
        self.nodes = [self.node(DHT_PORT, "")] + [self.node(0, DHT) for _ in range(7)]
        for node in self.nodes:
            for other in self.nodes:
                if other is not node:
                    node.add_dht_node(("127.0.0.1", other.listen_port()))
        wait(lambda: all(node.status().dht_nodes >= 3 for node in self.nodes), 60, "a DHT of eight nodes")

    @staticmethod
    def node(port, bootstrap):
        return lt.session({
            "listen_interfaces": f"127.0.0.1:{port}", "enable_dht": True, "dht_bootstrap_nodes": bootstrap,
            "enable_lsd": False, "enable_upnp": False, "enable_natpmp": False,
            "dht_restrict_routing_ips": False, "dht_restrict_search_ips": False,
            "dht_prefer_verified_node_ids": False, "dht_ignore_dark_internet": False,
            "alert_mask": lt.alert.category_t.dht_notification,
        })

    def get(self, seconds=30):
        """The item of the tests' key: (seq, value), else None."""
        node = self.nodes[-1]
        key = bytes.fromhex(public_key_hex())
        until = time.time() + seconds
        while time.time() < until:
            node.dht_get_mutable_item(key, SALT)
            asked = time.time() + 2
            while time.time() < asked:
                for alert in node.pop_alerts():
                    if isinstance(alert, lt.dht_mutable_item_alert) and alert.item:
                        return alert.seq, lt.bencode(alert.item)
                time.sleep(0.1)
        return None

    def close(self):
        self.nodes.clear()


NODES = None


def nodes():
    """The module's DHT, started at its first use."""
    global NODES
    if NODES is None:
        NODES = Nodes()
    return NODES


def put_newest(torrent):
    """The release's item of `torrent` on the tests' DHT, by its tool
    (release/publish-dht's)."""
    seq, value, signature = item_of(torrent)
    nodes()
    result = subprocess.run([DHT_TOOL, "put", public_key_hex(), SALT.decode(), str(seq), value.hex(), signature.hex(),
                             "--bootstrap", DHT, "--loopback", "--seconds", "60"],
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=120)
    assert result.returncode == 0, result.stdout
    return value


def make_torrent(folder, files, trackers, revision=60, signed=True):
    """libtorrent's torrent of `files` (path -> bytes) in `folder`/Survarium,
    signed as the release signs it."""
    for path, data in files.items():
        target = os.path.join(folder, "Survarium", *path.split("/"))
        os.makedirs(os.path.dirname(target), exist_ok=True)
        with open(target, "wb") as file:
            file.write(data)
    storage = lt.file_storage()
    lt.add_files(storage, os.path.join(folder, "Survarium"))
    creator = lt.create_torrent(storage, PIECE)
    for tier, url in enumerate(trackers):
        creator.add_tracker(url, tier)
    lt.set_piece_hashes(creator, folder)
    entry = creator.generate()
    entry.pop(b"creation date", None)
    if signed:
        sign(entry, revision)
    return lt.bencode(entry)


def add(lt_session, torrent, folder):
    params = lt.add_torrent_params()
    params.ti = lt.torrent_info(lt.bdecode(torrent))
    params.save_path = folder
    return lt_session.add_torrent(params)


def wait(done, seconds, what):
    until = time.time() + seconds
    while time.time() < until:
        if done():
            return
        time.sleep(0.1)
    raise AssertionError(f"no {what} in {seconds} s")


def canonical(torrent):
    """`torrent` as the updater keeps it (`logic/src/torrent.rs`
    `canonical`): its info, piece layers and signature, nothing else."""
    entry = lt.bdecode(torrent)
    return lt.bencode({key: value for key, value in entry.items() if key in (b"info", b"piece layers", b"skhid")})


def found(torrent):
    """`torrent` as the updater makes it again from the DHT's item and the
    swarm's info (`logic/src/newest.rs` `torrent`): no piece layers."""
    entry = lt.bdecode(torrent)
    return lt.bencode({b"info": entry[b"info"], b"skhid": entry[b"skhid"]})


class WrongPeer:
    """A peer that says it has every piece and sends 0xAA for each block
    asked: `closed` once the updater ends the connection."""

    def __init__(self, info_hash_v1, pieces):
        self.listener = socket.socket()
        self.listener.bind(("127.0.0.1", 0))
        self.listener.listen()
        self.port = self.listener.getsockname()[1]
        self.closed = False
        self.info_hash, self.pieces = info_hash_v1, pieces
        threading.Thread(target=self.serve, daemon=True).start()

    def serve(self):
        try:
            peer, _ = self.listener.accept()
            peer.settimeout(60)
            peer.recv(68)
            bits = bytearray((self.pieces + 7) // 8)
            for piece in range(self.pieces):
                bits[piece // 8] |= 0x80 >> (piece % 8)
            peer.sendall(b"\x13BitTorrent protocol" + bytes(8) + self.info_hash + b"-XX0000-wrongwrongwr"
                         + struct.pack(">IB", 1 + len(bits), 5) + bits + struct.pack(">IB", 1, 1))
            buffer = b""
            while True:
                data = peer.recv(65536)
                if not data:
                    break
                buffer += data
                while len(buffer) >= 4 and len(buffer) >= 4 + struct.unpack(">I", buffer[:4])[0]:
                    (length,) = struct.unpack(">I", buffer[:4])
                    body, buffer = buffer[4:4 + length], buffer[4 + length:]
                    if body[:1] == b"\x06":
                        index, begin, size = struct.unpack(">III", body[1:13])
                        peer.sendall(struct.pack(">IBII", 9 + size, 7, index, begin) + b"\xaa" * size)
        except OSError:
            pass
        self.closed = True


class TorrentTest(wine.WineTest):
    # The patch step's helpers, not its tests.
    make_patches = wine.PatchTest.make_patches
    patch_files = wine.PatchTest.patch_files
    assert_patches = wine.PatchTest.assert_patches
    store_requests = wine.PatchTest.store_requests

    def setUp(self):
        super().setUp()
        self.tracker = Tracker()
        self.addCleanup(self.tracker.close)
        self.peers_dir = tempfile.mkdtemp(prefix="skhid-torrent-peers-")
        self.sessions = []
        self.launched = None

    def tearDown(self):
        if self.launched:
            self.stop()
        for lt_session in self.sessions:
            del lt_session
        self.sessions.clear()
        super().tearDown()

    def release(self, *numbers, torrent=True, exe=None, size=200_000, **signing):
        """What the server offers: the patches `numbers` (of about `size`
        bytes), and the overlay with the torrent of the folder (the patches,
        two of the overlay's files) beside the updater (`torrent`). The
        torrent, its files and the seed's folder."""
        overlay = self.bundle()
        if exe is not None:
            overlay[f"{X86}/survarium_skhid.exe"] = exe
        patches = self.make_patches(*numbers, size=size)
        files = {f"game/resources.db.patch_{n}": data for n, data in patches.items()}
        files["README.txt"] = overlay["README.txt"]
        files[f"{X86}/bugtrap.dll"] = overlay[f"{X86}/bugtrap.dll"]
        seed_dir = tempfile.mkdtemp(dir=self.peers_dir)
        data = make_torrent(seed_dir, files, self.tracker.urls, **signing)
        if torrent:
            overlay[f"{X86}/skhid.torrent"] = data
        self.offer(overlay)
        return patches, data, seed_dir, files

    def seed(self, torrent, folder):
        port = free_port()
        lt_session = session(port)
        self.sessions.append(lt_session)
        handle = add(lt_session, torrent, folder)
        wait(lambda: handle.status().is_seeding, 60, "seed")
        return port, handle

    def launch(self, allow="torrent", port=None):
        """The stand-in launcher with the updater it started, left running.
        `port`: its 13th argument (a free one when None; "" for none)."""
        self.port = free_port() if port is None else port
        self.out = os.path.join(self.root, "pipe.bin")
        self.stop_file = os.path.join(self.root, "stop")
        self.say_file = os.path.join(self.root, "say")
        for name in (self.out, self.stop_file, self.say_file):
            if os.path.exists(name):
                os.remove(name)
        extra = " ".join(f'"{value}"' for value in ["0.26g0", allow, *[""] * 8, str(self.port)])
        command, env = windows([harness.FAKE_EXE, "launcher", f"SkhidTorrent{os.getpid()}{random.randrange(1 << 30)}",
                                windows_path(self.exe()), windows_path(self.out), windows_path(self.stop_file),
                                "", extra, windows_path(self.say_file)])
        self.launched = wine.subprocess.Popen(command, cwd=self.x86, env=env,
                                              stdout=wine.subprocess.DEVNULL, stderr=wine.subprocess.DEVNULL)

    def frames(self):
        return self.said(self.out)

    def states(self):
        return [frame[1] for frame in self.frames() if frame[0] == 0x02]

    def wait_latest(self, seconds=180):
        try:
            wait(lambda: LATEST in self.states() or ERROR in self.states(), seconds, "end of the update")
        except AssertionError:
            self.fail(f"states {[hex(s) for s in self.states()]}; log:\n{self.log()[-4000:]}")
        self.assertIn(LATEST, self.states(), self.log()[-4000:])

    def say(self, *frames):
        """The launcher says `frames` (payloads) on its pipe."""
        part = self.say_file + ".part"
        with open(part, "wb") as file:
            for payload in frames:
                file.write(struct.pack("<H", len(payload)) + payload)
        os.replace(part, self.say_file)
        wait(lambda: not os.path.exists(self.say_file), 30, "the launcher's words read")

    def stop(self):
        open(self.stop_file, "w").close()
        try:
            self.launched.wait(60)
        except wine.subprocess.TimeoutExpired:
            self.launched.kill()
        self.launched = None

    def leech_from_updater(self, torrent, files):
        """A libtorrent leech that gets the whole torrent from the updater
        alone: every file it got is the torrent's."""
        folder = tempfile.mkdtemp(dir=self.peers_dir)
        lt_session = session(free_port())
        self.sessions.append(lt_session)
        handle = add(lt_session, torrent, folder)
        handle.connect_peer(("127.0.0.1", self.port))
        until = time.time() + 120
        while not handle.status().is_seeding:
            if time.time() > until:
                status = handle.status()
                self.fail(f"the leech has {status.progress:.0%} from the updater; log:\n{self.log()[-3000:]}")
            if not handle.get_peer_info():
                handle.connect_peer(("127.0.0.1", self.port))
            time.sleep(0.5)
        for path, data in files.items():
            with open(os.path.join(folder, "Survarium", *path.split("/")), "rb") as file:
                self.assertEqual(sha256(file.read()), sha256(data), path)

    # --- the tests -------------------------------------------------------

    def bundle(self, **changes):
        """The players' ZIP has the DLL beside the updater."""
        with open(harness.TORRENT_DLL, "rb") as file:
            dll = file.read()
        return wine.WineTest.bundle(self, **{f"{X86}/skhid_torrent.dll": dll, **changes})

    def test_a_missing_patch_comes_by_torrent_and_the_folder_is_then_shared(self):
        patches, torrent, seed_dir, files = self.release(12, 14)
        seed_port, _ = self.seed(torrent, seed_dir)
        self.tracker.peers = [("127.0.0.1", seed_port)]
        self.launch()
        self.wait_latest()
        self.assert_patches(patches)
        self.assertEqual(self.store_requests(), [], "nothing by HTTPS")
        log = self.log()
        self.assertIn("patches: 2 by torrent", log)
        self.assertIn("downloaded by torrent, checksum right", log)
        self.assertIn("torrent: skhid.torrent: signed by the release, revision 60", log)
        # Both trackers asked, with the port the launcher gave (the seed
        # asks them too).
        wait(lambda: {("udp", self.port), ("http", self.port)} <= set(self.tracker.asked), 30, f"both trackers ({self.tracker.asked})")
        # The seed goes; the updater, its launcher open, shares the folder:
        # the patches it got by torrent and the ZIP's files.
        for lt_session in self.sessions:
            lt_session.pause()
        wait(lambda: "torrent: sharing 4 of its 4 files" in self.log(), 60, "sharing")
        self.leech_from_updater(torrent, files)

    def test_a_second_start_shares_what_was_checked_without_hashing_it_again(self):
        _, torrent, _, files = self.release(12)
        self.launch()
        self.wait_latest()
        wait(lambda: "torrent: 3 of its 3 files checked whole" in self.log(), 60, "the check")
        self.stop()
        self.launch()
        self.wait_latest()
        wait(lambda: "torrent: sharing 3 of its 3 files (3 checked before)" in self.log(), 60, "sharing")
        self.leech_from_updater(torrent, files)
        # A file changed since: checked again, and not shared as it is.
        self.stop()
        self.write("README.txt", b"READ ME\r\n")
        self.launch()
        self.wait_latest()
        wait(lambda: self.log().count("checked whole") >= 3, 60, "the third check")
        self.assertIn("(2 checked before)", self.log())

    def test_no_peer_downloads_by_https_and_what_came_so_is_shared(self):
        # The first players of a revision: no peer yet.
        patches, torrent, _, files = self.release(12)
        self.launch()
        self.wait_latest()
        self.assert_patches(patches)
        self.assertEqual(self.store_requests(), ["/store/patch_12"])
        self.assertIn("patches: by torrent: no peer in 20 s: from their addresses", self.log())
        wait(lambda: "torrent: sharing 3 of its 3 files" in self.log(), 60, "sharing")
        self.leech_from_updater(torrent, files)

    def test_a_peer_that_sends_wrong_bytes_is_banned(self):
        patches, torrent, seed_dir, _ = self.release(12)
        info = lt.torrent_info(lt.bdecode(torrent))
        wrong = WrongPeer(bytes.fromhex(str(info.info_hashes().v1)), info.num_pieces())
        seed_port, _ = self.seed(torrent, seed_dir)
        self.tracker.peers = [("127.0.0.1", wrong.port), ("127.0.0.1", seed_port)]
        self.launch()
        self.wait_latest()
        self.assert_patches(patches)
        wait(lambda: wrong.closed, 30, "the wrong peer dropped")
        self.assertEqual(self.store_requests(), [])

    @unittest.skipUnless(DHT_TOOL, "SKHID_DHT_TOOL: release/publish-dht's skhid-dht")
    def test_a_newer_release_comes_from_the_dht_when_the_site_cannot_be_reached(self):
        _, torrent, _, files = self.release(12)
        self.launch()
        self.wait_latest()
        self.stop()
        # The release after it: its item on the DHT, its files at a seed.
        newer_dir = tempfile.mkdtemp(dir=self.peers_dir)
        newer = make_torrent(newer_dir, {**files, "game/new.txt": b"new"}, self.tracker.urls, revision=61)
        seed_port, _ = self.seed(newer, newer_dir)
        self.tracker.peers = [("127.0.0.1", seed_port)]
        put_newest(newer)
        # The update server cannot be asked.
        self.site.manifest_status = 502
        self.site.patch_list_status = 502
        self.launch()
        self.wait_latest()
        log = self.log()
        self.assertIn("torrent: the DHT names revision 61, signed by the release", log)
        self.assertIn("torrent: revision 61's info came from its swarm", log)
        # Kept as the release signed it, without what is outside the
        # signature (its trackers).
        self.assertEqual(self.read(f"{X86}/updater_skhid.found.torrent"), found(newer))
        self.assertNotIn(b"announce", found(newer))

    @unittest.skipUnless(DHT_TOOL, "SKHID_DHT_TOOL: release/publish-dht's skhid-dht")
    def test_an_item_of_another_key_or_an_older_revision_is_not_taken(self):
        self.release(12)
        self.launch()
        self.wait_latest()
        self.stop()
        # Revision 59's item, older than the folder's 60: nothing newer.
        older = make_torrent(tempfile.mkdtemp(dir=self.peers_dir), {"README.txt": b"old"}, self.tracker.urls,
                             revision=59)
        put_newest(older)
        self.site.manifest_status = 502
        self.site.patch_list_status = 502
        self.launch()
        wait(lambda: "torrent: the DHT named no newer release" in self.log(), 120, "the DHT asked")
        self.assertIn("torrent: the DHT named no newer release", self.log())
        self.assertFalse(self.exists(f"{X86}/updater_skhid.found.torrent"))

    def test_the_updater_puts_the_releases_item_on_the_dht(self):
        _, torrent, _, _ = self.release(12)
        dht = nodes()
        self.launch()
        self.wait_latest()
        wait(lambda: "item stored on" in self.log(), 90, "the item put")
        seq, value, _ = item_of(torrent)
        self.assertEqual(dht.get(), (seq, value))

    def test_the_bootstrap_is_shared_from_copies_kept_of_its_files(self):
        # The bootstrap torrent: this updater, its DLL and a 7zr.exe,
        # unsigned, in the ZIP beside the updater (no revision torrent: it
        # is shared alone).
        seven = b"7z stand-in " * 5000
        with open(harness.TORRENT_DLL, "rb") as file:
            dll = file.read()
        bootstrap_files = {f"{X86}/7zr.exe": seven, f"{X86}/skhid_torrent.dll": dll,
                           f"{X86}/updater_skhid.exe": self.updater_bytes}
        bootstrap = make_torrent(tempfile.mkdtemp(dir=self.peers_dir), bootstrap_files, self.tracker.urls,
                                 signed=False)
        self.offer(self.bundle(**{f"{X86}/7zr.exe": seven, f"{X86}/skhid-bootstrap.torrent": bootstrap}))
        self.launch()
        self.wait_latest()
        wait(lambda: "torrent: sharing the bootstrap's 3 of 3 files" in self.log(), 60, "sharing the bootstrap")
        kept = os.path.join(self.x86, "updater_skhid.bootstrap")
        self.assertEqual(sorted(os.listdir(kept)), ["7zr.exe", "skhid_torrent.dll", "updater_skhid.exe"])
        # The file in its place is another now (an update): the kept copy is
        # what is shared.
        self.write(f"{X86}/7zr.exe", b"another 7zr")
        self.leech_from_updater(bootstrap, bootstrap_files)

    @unittest.skipUnless(DHT_TOOL, "SKHID_DHT_TOOL: release/publish-dht's skhid-dht")
    def test_a_bare_folder_installs_everything_from_the_dht_without_the_site(self):
        # The bootstrap's folder: the updater and its DLL alone (and the
        # original game, whose pins are the real files: it is there), the
        # site not answering. The newest release only on the DHT, its files
        # at a seed.
        self.site.manifest_status = 502
        self.site.patch_list_status = 502
        overlay = self.bundle()
        patches = self.make_patches(12)
        files = {**overlay, "game/resources.db.patch_12": patches[12]}
        seed_dir = tempfile.mkdtemp(dir=self.peers_dir)
        newest = make_torrent(seed_dir, files, self.tracker.urls, revision=61)
        seed_port, _ = self.seed(newest, seed_dir)
        self.tracker.peers = [("127.0.0.1", seed_port)]
        put_newest(newest)
        # `check torrent`, as Skhid-get.bat runs it.
        command, env = windows([self.exe(), "check", "torrent", "quiet"])
        code = wine.subprocess.run(command, cwd=self.x86, env=env, timeout=600,
                                   stdout=wine.subprocess.DEVNULL, stderr=wine.subprocess.DEVNULL).returncode
        log = self.log()
        self.assertEqual(code, 0, log[-4000:])
        self.assertIn("torrent: the DHT names revision 61, signed by the release", log)
        for name, data in files.items():
            self.assertEqual(sha256(self.read(name)), sha256(data), name)
        self.assertEqual(self.store_requests(), [])
        state = self.read(f"{X86}/updater_skhid.state").decode()
        info = hashlib.sha256(lt.bencode(lt.bdecode(newest)[b"info"])).hexdigest()
        self.assertIn(f"archive {info}", state)
        self.assertEqual(self.read(f"{X86}/updater_skhid.patches.status"), b"ready\r\n")
        kept = self.read(f"{X86}/updater_skhid.patches").decode()
        self.assertIn('"urls": []', kept)
        # And `ready` (play_skhid.bat's) lets the game start.
        self.assertEqual(self.run_ready(), 0)

    def test_allow_torrent_while_watching_shares_at_once_and_off_stops_at_once(self):
        _, torrent, _, files = self.release(12)
        self.launch(allow="direct")
        self.wait_latest()
        self.assertNotIn("torrent: sharing", self.log())
        # "Allow torrent" ticked: shared before the watch's next ask (10 s),
        # on the launcher's port.
        ticked = time.time()
        self.say(bytes([0x1A, 0]))
        wait(lambda: "torrent: sharing 3 of its 3 files" in self.log(), 30, "sharing")
        self.assertLess(time.time() - ticked, 5, "sharing began only with the watch's next ask")
        self.leech_from_updater(torrent, files)
        # Unticked: nothing listens at once.
        self.say(bytes([0x1A, 1]))
        wait(lambda: "torrent: not allowed by the launcher" in self.log(), 30, "torrent off")
        time.sleep(0.5)
        with self.assertRaises(OSError):
            socket.create_connection(("127.0.0.1", self.port), timeout=5).close()

    def test_allow_torrent_during_an_https_download_moves_the_rest_to_the_torrent(self):
        # A patch of 64 pieces, slow by HTTPS (64 KiB every 0.3 s: about
        # 10 s), at a seed too.
        patches, torrent, seed_dir, _ = self.release(12, size=2 << 20)
        seed_port, _ = self.seed(torrent, seed_dir)
        self.tracker.peers = [("127.0.0.1", seed_port)]
        self.site.patch_slow = 0.3
        self.launch(allow="direct")
        log_file = self.path(f"{X86}/updater_skhid.log")
        wait(lambda: os.path.exists(log_file) and "resources.db.patch_12: address 1 of 1" in self.log(), 120,
             "the download by HTTPS")
        time.sleep(3)
        self.say(bytes([0x1A, 0]))
        self.wait_latest()
        self.assert_patches(patches)
        log = self.log()
        self.assertIn("patches: resources.db.patch_12: address 1 of 1: the launcher turned torrent on: "
                      "the rest by torrent", log)
        self.assertIn("downloaded by torrent, checksum right", log)
        self.assertEqual(self.store_requests(), ["/store/patch_12"], "HTTPS asked once")
        # What came by HTTPS was kept: the torrent found some of the pieces
        # in the part, checked, and fetched only the others.
        found = re.search(r"torrent: (\d+) of the (\d+) pieces to fetch are in the parts already", log)
        self.assertIsNotNone(found, log[-3000:])
        kept, wanted = int(found[1]), int(found[2])
        self.assertTrue(0 < kept < wanted, found[0])

    def test_check_with_nothing_saved_is_https(self):
        # No "Allow torrent" saved by the launcher: `check` is direct.
        patches, _, _, _ = self.release(12)
        command, env = windows([self.exe(), "check", "quiet"])
        code = wine.subprocess.run(command, cwd=self.x86, env=env, timeout=600,
                                   stdout=wine.subprocess.DEVNULL, stderr=wine.subprocess.DEVNULL).returncode
        self.assertEqual(code, 0, self.log()[-4000:])
        self.assert_patches(patches)
        self.assertEqual(self.store_requests(), ["/store/patch_12"])
        self.assertEqual(self.tracker.asked, [])
        self.assertNotIn("torrent: allowed", self.log())

    def test_torrent_not_allowed_is_https_as_before(self):
        patches, _, _, _ = self.release(12)
        self.launch(allow="direct")
        self.wait_latest()
        self.assert_patches(patches)
        self.assertEqual(self.tracker.asked, [])

    def test_without_the_dll_torrent_is_https_as_before(self):
        patches, _, _, _ = self.release(12)
        self.offer(self.bundle(**{f"{X86}/skhid_torrent.dll": None}))
        os.remove(os.path.join(self.x86, "skhid_torrent.dll"))
        self.launch()
        self.wait_latest()
        self.assert_patches(patches)
        self.assertEqual(self.store_requests(), ["/store/patch_12"])
        self.assertIn("torrent: no skhid_torrent.dll: HTTPS only", self.log())
        self.assertEqual(self.tracker.asked, [])

    def test_no_torrent_or_one_not_the_releases_is_https_as_before(self):
        cases = (
            ("no skhid.torrent", dict(torrent=False)),
            ("skhid.torrent: not signed", dict(signed=False)),
            ("of revision 40, older than the 50 this folder has", dict(revision=40)),
        )
        for why, release in cases:
            with self.subTest(why):
                for name in os.listdir(self.path("game")):
                    if name.startswith("resources.db.patch_"):
                        os.remove(self.path(f"game/{name}"))
                self.site.requests.clear()
                self.site.patches.clear()
                # The game says revision 50 (test_play_gate's stand-in exe).
                patches, _, _, _ = self.release(12, exe=game_exe(50), **release)
                self.launch()
                self.wait_latest()
                self.assert_patches(patches)
                self.assertEqual(self.store_requests(), ["/store/patch_12"])
                self.assertIn(why, self.log())
                self.stop()
                self.assertEqual(self.tracker.asked, [])

    def test_with_no_port_from_the_launcher_one_is_picked_kept_and_changed_when_busy(self):
        self.release(12)
        kept = self.path(f"{X86}/updater_skhid.port")

        def listening_on_the_kept_port():
            self.wait_latest()
            wait(lambda: os.path.exists(kept), 30, "the kept port")
            with open(kept) as file:
                port = int(file.read())
            with socket.create_connection(("127.0.0.1", port), timeout=10):
                pass
            self.stop()
            return port

        self.launch(port="")
        first = listening_on_the_kept_port()
        self.assertIn(f"torrent: port {first} picked, kept in updater_skhid.port", self.log())
        # The next start: the same port.
        self.launch(port="")
        self.assertEqual(listening_on_the_kept_port(), first)
        # Another program holds it: another one, kept from then on.
        busy = socket.socket()
        busy.bind(("0.0.0.0", first))
        busy.listen()
        self.addCleanup(busy.close)
        self.launch(port="")
        second = listening_on_the_kept_port()
        self.assertNotEqual(second, first)
        # The launcher's port, when it names one, wins and is not kept.
        named = free_port()
        self.launch(port=named)
        self.wait_latest()
        with socket.create_connection(("127.0.0.1", named), timeout=10):
            pass
        self.stop()
        with open(kept) as file:
            self.assertEqual(int(file.read()), second)

    def test_the_port_the_launcher_asks_for_is_answered(self):
        self.release(12)
        self.launch()
        self.wait_latest()
        busy = socket.socket()
        busy.bind(("127.0.0.1", 0))
        busy.listen()
        self.addCleanup(busy.close)
        free = free_port()
        self.say(struct.pack("<BH", 0x21, busy.getsockname()[1]))
        wait(lambda: any(frame[:1] == b"\x14" for frame in self.frames()), 30, "port busy")
        self.say(struct.pack("<BH", 0x21, free))
        wait(lambda: any(frame[:1] == b"\x15" for frame in self.frames()), 30, "port ok")
        # It listens there now.
        with socket.create_connection(("127.0.0.1", free), timeout=10):
            pass
        # "Allow torrent" off: nothing listens.
        self.say(bytes([0x1A, 1]))
        wait(lambda: "torrent: not allowed by the launcher" in self.log(), 30, "torrent off")
        time.sleep(0.5)
        with self.assertRaises(OSError):
            socket.create_connection(("127.0.0.1", free), timeout=5).close()


if __name__ == "__main__":
    if sys.argv[1:] == ["--key"]:
        print(public_key_hex())
        sys.exit(0)
    if sys.argv[1:] == ["--trackers"]:
        print(TRACKERS)
        sys.exit(0)
    if sys.argv[1:] == ["--dht"]:
        print(DHT)
        sys.exit(0)
    unittest.main()
