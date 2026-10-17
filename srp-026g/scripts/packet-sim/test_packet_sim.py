"""Tests of packet-sim: the impairment model, rules and profiles, and UDP/TCP
forwarding through a running simulator to a local echo server.

    python3 -m unittest discover -s srp-026g/scripts/packet-sim -p 'test_*.py'
"""
from __future__ import annotations

import json
import math
from pathlib import Path
import random
import socket
import sys
import tempfile
import threading
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import packet_sim as ps  # noqa: E402


def state(seed: int = 1) -> ps.LinkState:
    return ps.LinkState(random.Random(seed))


def run_link(link: ps.Link, count: int, seed: int = 1, spacing: float = 0.01, size: int = 100):
    """Offer `count` packets `spacing` s apart; (offer time, delivery times, verdict) each."""
    s = state(seed)
    return [(i * spacing, *ps.decide(link, s, i * spacing, size)) for i in range(count)]


class FakeSession:
    def __init__(self, index=1, addr="127.0.0.1:5000", started=0.0, joined_after=0.0):
        self.index, self.label_addr, self.started, self.joined_after = index, addr, started, joined_after


class ModelTest(unittest.TestCase):
    def test_clean_link_passes_everything_at_once(self):
        for t, times, verdict in run_link(ps.Link(), 100):
            self.assertEqual((times, verdict), ([t], "pass"))

    def test_loss_rate_is_close_to_the_setting(self):
        lost = sum(not times for _, times, _ in run_link(ps.Link(loss_pct=3), 20000))
        self.assertAlmostEqual(lost / 20000, 0.03, delta=0.005)

    def test_same_seed_same_verdicts_other_seed_others(self):
        link = ps.Link(loss_pct=10, jitter_ms=5, delay_ms=10, duplicate_pct=5, reorder_pct=5,
                       burst_enter_pct=2, burst_exit_pct=30)
        self.assertEqual(run_link(link, 2000, seed=7), run_link(link, 2000, seed=7))
        self.assertNotEqual(run_link(link, 2000, seed=7), run_link(link, 2000, seed=8))

    def test_one_packets_fate_does_not_shift_the_next_draws(self):
        # A drop event in the middle does not change what happens after it.
        a, b = state(3), state(3)
        link = ps.Link(loss_pct=20)
        out_a = [ps.decide(link, a, i, 10, drop=(i == 5))[1] for i in range(50)]
        out_b = [ps.decide(link, b, i, 10)[1] for i in range(50)]
        self.assertEqual(out_a[6:], out_b[6:])
        self.assertEqual(out_a[5], "drop:event")

    def test_gilbert_elliott_losses_come_in_bursts(self):
        link = ps.Link(burst_enter_pct=1, burst_exit_pct=25, burst_loss_pct=100)
        verdicts = [v for _, _, v in run_link(link, 50000, seed=2)]
        runs, current = [], 0
        for v in verdicts:
            if v == "drop:burst":
                current += 1
            elif current:
                runs.append(current)
                current = 0
        self.assertGreater(len(runs), 100)
        self.assertAlmostEqual(sum(runs) / len(runs), 4.0, delta=0.6)  # mean burst = 100 / exit_pct
        # Stationary loss = enter / (enter + exit) = 1 / 26.
        self.assertAlmostEqual(verdicts.count("drop:burst") / len(verdicts), 1 / 26, delta=0.008)

    def test_delay_and_jitter_keep_order(self):
        # Dense packets: jitter never reorders (a packet waits for the one before).
        rows = run_link(ps.Link(delay_ms=50, jitter_ms=20), 2000, spacing=0.002)
        deliveries = [times[0] for _, times, _ in rows]
        self.assertEqual(deliveries, sorted(deliveries))
        # Sparse packets: the delay is the setting on average.
        rows = run_link(ps.Link(delay_ms=50, jitter_ms=10), 2000, spacing=0.1)
        delays = [times[0] - t for t, times, _ in rows]
        self.assertGreaterEqual(min(delays), 0)
        self.assertAlmostEqual(sum(delays) / len(delays), 0.05, delta=0.002)
        spread = math.sqrt(sum((d - 0.05) ** 2 for d in delays) / len(delays))
        self.assertAlmostEqual(spread, 0.01, delta=0.0015)

    def test_reordered_packets_are_overtaken(self):
        rows = run_link(ps.Link(reorder_pct=10, reorder_gap_ms=30), 1000, spacing=0.005)
        self.assertTrue(any(v == "reorder" for _, _, v in rows))
        deliveries = [times[0] for _, times, _ in rows]
        self.assertNotEqual(deliveries, sorted(deliveries))
        for t, times, verdict in rows:
            if verdict == "reorder":
                self.assertAlmostEqual(times[0] - t, 0.03, delta=1e-9)

    def test_duplicates_have_two_deliveries(self):
        rows = run_link(ps.Link(duplicate_pct=50), 1000)
        dups = [times for _, times, v in rows if v == "duplicate"]
        self.assertAlmostEqual(len(dups) / 1000, 0.5, delta=0.06)
        self.assertTrue(all(len(times) == 2 and times[1] >= times[0] for times in dups))

    def test_rate_cap_serialises_and_tail_drops(self):
        # 1000-byte packets at 80 kbit/s take 100 ms each; offered all at once.
        s = state()
        link = ps.Link(rate_kbps=80, queue_ms=350)
        out = [ps.decide(link, s, 0.0, 1000) for _ in range(6)]
        delivered = [times[0] for times, _ in out if times]
        self.assertEqual([round(t, 3) for t in delivered], [0.1, 0.2, 0.3, 0.4])
        self.assertEqual([v for _, v in out][4:], ["drop:queue", "drop:queue"])

    def test_hold_delivers_at_the_window_end_in_order(self):
        s = state()
        out = [ps.decide(ps.Link(delay_ms=10), s, t, 10, hold_until=2.0) for t in (1.0, 1.5)]
        self.assertEqual(out, [([2.0], "hold"), ([2.0], "hold")])
        after, verdict = ps.decide(ps.Link(), s, 1.9, 10)
        self.assertEqual((after, verdict), ([2.0], "pass"))  # does not overtake the held ones

    def test_link_settings_are_checked(self):
        with self.assertRaises(ValueError):
            ps.Link().merged({"loss_pct": 101})
        with self.assertRaises(ValueError):
            ps.Link().merged({"lost_pct": 1})
        with self.assertRaises(ValueError):
            ps.Link().merged({"delay_ms": -1})
        self.assertEqual(ps.Link().merged({"loss_pct": 3}).loss_pct, 3.0)


class RuleTest(unittest.TestCase):
    def test_window_at_duration_every(self):
        rule = ps.Rule.parse({"action": "drop", "at": 30, "duration": 3, "every": 60}, event=True)
        self.assertIsNone(rule.window(29.9, 0))
        self.assertEqual(rule.window(31, 0), (0, 33))
        self.assertIsNone(rule.window(33.1, 0))
        self.assertEqual(rule.window(91, 0), (1, 93))

    def test_session_anchor(self):
        rule = ps.Rule.parse({"action": "drop", "at": 1, "duration": 4, "anchor": "session"}, event=True)
        self.assertIsNone(rule.window(100.5, 100))
        self.assertEqual(rule.window(101.5, 100), (0, 105))
        self.assertIsNone(rule.window(105.1, 100))

    def test_selectors(self):
        match = lambda selector, **kw: ps.peer_matcher(selector)(FakeSession(**kw))
        self.assertTrue(match("*"))
        self.assertTrue(match("2", index=2))
        self.assertFalse(match("2", index=1))
        self.assertTrue(match("addr:127.0.0.1:5000"))
        self.assertFalse(match("addr:127.0.0.1:5001"))
        self.assertTrue(match("new-after:20", started=25))
        self.assertFalse(match("new-after:20", started=5))
        nxt = ps.peer_matcher("next-after:20")
        port = type("F", (), {"name": "match"})()
        a, b, c = (FakeSession(index=i, started=t) for i, t in ((1, 5), (2, 21), (3, 22)))
        for s in (a, b, c):
            s.forwarder, s.generation = port, 1
        self.assertEqual([nxt(a), nxt(b), nxt(c), nxt(b)], [False, True, False, True])
        self.assertTrue(match("late:10", joined_after=40))
        self.assertFalse(match("late:10", joined_after=3))
        with self.assertRaises(ValueError):
            ps.peer_matcher("third")
        rule = ps.Rule.parse({"peer": "1,3"}, event=False)
        self.assertTrue(rule.selects(FakeSession(index=3)))
        self.assertFalse(rule.selects(FakeSession(index=2)))

    def test_port_and_direction(self):
        rule = ps.Rule.parse({"port": "lobby,match", "dir": "down", "loss_pct": 5}, event=False)
        self.assertTrue(rule.applies("match", "down"))
        self.assertFalse(rule.applies("match", "up"))
        self.assertFalse(rule.applies("login", "down"))

    def test_bad_rules_are_refused(self):
        for bad, event in [({"action": "drop"}, False), ({"at": 3}, False), ({"action": "explode"}, True),
                           ({"dir": "sideways"}, True), ({"duration": 0}, True), ({"every": 1, "duration": 2}, True),
                           ({"frobnicate": 1}, True)]:
            with self.assertRaises(ValueError, msg=bad):
                ps.Rule.parse(bad, event)

    def test_spec_words_and_json(self):
        rule = ps.parse_spec("port=match peer=2 action=drop at=30 duration=3 dir=up", event=True)
        self.assertEqual((rule.port, rule.peer, rule.action, rule.at, rule.duration, rule.dir),
                         ("match", "2", "drop", 30.0, 3.0, "up"))
        rule = ps.parse_spec("dir=down loss_pct=5 delay_ms=40", event=False)
        self.assertEqual(rule.link, {"loss_pct": 5.0, "delay_ms": 40.0})
        rule = ps.parse_spec('{"port": "match", "action": "hold", "duration": 2}', event=True)
        self.assertEqual((rule.action, rule.at), ("hold", 0.0))

    def test_shipped_profiles_load(self):
        names = {p.stem for p in ps.PROFILES.glob("*.json")}
        self.assertTrue({"warp-like", "lossy-3pct", "blackout-during-late-join"} <= names)
        for name in names:
            profile = ps.Profile.load(name)
            self.assertTrue(profile.description, name)

    def test_simulator_layers_rules_and_events(self):
        profile = ps.Profile.parse({
            "link": {"delay_ms": 30},
            "rules": [{"port": "match", "dir": "down", "loss_pct": 7}],
            "events": [{"port": "match", "peer": "late:10", "anchor": "session", "at": 1, "duration": 4,
                        "action": "drop"},
                       {"port": "lobby", "action": "hold", "at": 5, "duration": 2}],
        }, "t")
        sim = ps.Simulator(profile, out=None)
        match, lobby = type("F", (), {"name": "match"})(), type("F", (), {"name": "lobby"})()

        def session(forwarder, **kw):
            s = FakeSession(**kw)
            s.forwarder, s.name, s.generation = forwarder, f"{forwarder.name}#{s.index}", 1
            return s

        early = session(match, started=10, joined_after=0)
        late = session(match, index=2, started=60, joined_after=50)
        link, drop, hold = sim.resolve(early, "down", 62)
        self.assertEqual((link.delay_ms, link.loss_pct, drop, hold), (30, 7, False, 0))
        self.assertTrue(sim.resolve(late, "up", 62)[1])
        self.assertFalse(sim.resolve(late, "up", 60.5)[1])
        self.assertFalse(sim.resolve(late, "up", 65.5)[1])
        self.assertEqual(sim.resolve(session(lobby), "up", 6)[2], 7)


class MessageIdsTest(unittest.TestCase):
    """Datagrams as srp's packet log of a real game (rev 18) recorded them."""

    def test_single_messages_and_keepalives(self):
        self.assertEqual(ps.message_ids(bytes.fromhex("00ff00d8934faf7c80"), "up"), (0xD8,))
        self.assertEqual(ps.message_ids(bytes.fromhex("010080d9934faf7c00808080"), "up"), (0xD9,))
        self.assertEqual(ps.message_ids(bytes.fromhex("000000e3cc16cc16"), "down"), (0xE3,))
        self.assertEqual(ps.message_ids(bytes.fromhex("0d0478fc"), "down"), (0xFC,))
        self.assertEqual(ps.message_ids(b"\x01\x02", "up"), ())

    def test_split_and_multi(self):
        # The first part of a split 0xE4 (static info): FF E4 x part count ...
        part = bytes.fromhex("010140ffe40000020001023900") + bytes(40)
        self.assertEqual(ps.message_ids(part, "down"), (0xE4,))
        # A multi datagram (F=1 in ack0, no tail): two relays and a ping.
        multi = bytes([5, 9, 0x01, 0xEB]) + bytes(10) + bytes([0xEB]) + bytes(10) + bytes([0xE6, 7]) + bytes(4)
        self.assertEqual(ps.message_ids(multi, "down"), (0xEB, 0xEB, 0xE6))
        # Up: a late-join answer (an impossible chunk head, 20 bytes) behind which a DA comes.
        answer = bytes([9, 3, 0x01, 0xE0]) + b"\xff" * 6 + b"\0\0" + bytes(12) + bytes([0xDA])
        self.assertEqual(ps.message_ids(answer, "up"), (0xE0, 0xDA))
        # A snapshot chunk (time 50, offset 0, total 10) joined with a hash report.
        chunk = bytes([9, 3, 0x01, 0xE0]) + (50).to_bytes(4, "little") + (0).to_bytes(2, "little") \
            + (10).to_bytes(2, "little") + bytes(10) + bytes([0xDF]) + bytes(5)
        self.assertEqual(ps.message_ids(chunk, "up"), (0xE0, 0xDF))

    def test_ids_rules_pick_datagrams_and_count_limits(self):
        profile = ps.Profile.parse({"events": [
            {"port": "match", "dir": "down", "action": "drop", "at": 0, "duration": 10, "ids": "E4", "count": 1},
            {"port": "match", "dir": "up", "action": "drop", "at": 0, "duration": 10, "ids": ["da"]}]}, "t")
        sim = ps.Simulator(profile, out=None)
        match = type("F", (), {"name": "match"})()
        s = FakeSession()
        s.forwarder, s.name, s.generation = match, "match#1", 1
        e4 = bytes.fromhex("010140ffe40000020001023900") + bytes(40)
        e3 = bytes.fromhex("000000e3cc16cc16")
        self.assertFalse(sim.resolve(s, "down", 1, e3)[1])
        self.assertTrue(sim.resolve(s, "down", 1, e4)[1])
        self.assertFalse(sim.resolve(s, "down", 2, e4)[1])  # count 1: the second copy passes
        self.assertTrue(sim.resolve(s, "up", 2, bytes([7, 3, 0, 0xDA]))[1])
        self.assertFalse(sim.resolve(s, "up", 2, bytes([7, 3, 0, 0xFC]))[1])
        self.assertFalse(sim.resolve(s, "up", 2)[1])  # no datagram (a TCP chunk): an ids rule does not apply
        with self.assertRaises(ValueError):
            ps.Rule.parse({"action": "set", "count": 2, "loss_pct": 1}, event=True)

    def test_an_event_can_count_from_a_message(self):
        profile = ps.Profile.parse({"events": [
            {"port": "match", "action": "drop", "anchor": "id:DA", "at": 0.5, "duration": 2}]}, "t")
        sim = ps.Simulator(profile, out=None)
        self.assertEqual(sim.anchor_ids, {0xDA})
        match = type("F", (), {"name": "match", "kind": "udp"})()
        s = FakeSession()
        s.forwarder, s.name, s.generation, s.first_seen = match, "match#1", 1, {}
        s.streams = {"up": ps.Stream(random.Random(1)), "down": ps.Stream(random.Random(2))}
        self.assertFalse(sim.resolve(s, "down", 10)[1])  # no 0xDA yet: no window
        sim.offer(s, "up", bytes([7, 3, 0, 0xDA]), lambda data: None)
        seen = s.first_seen[0xDA]
        self.assertFalse(sim.resolve(s, "down", seen + 0.4)[1])
        self.assertTrue(sim.resolve(s, "down", seen + 0.6)[1])
        self.assertFalse(sim.resolve(s, "down", seen + 2.6)[1])
        with self.assertRaises(ValueError):
            ps.Rule.parse({"action": "drop", "anchor": "id:DA,DB"}, event=True)
        self.assertEqual(ps.spec_dict("ids=E4,E8 count=2")["count"], 2)


class XrandrTest(unittest.TestCase):
    def test_adds_and_fixes_the_x11_driver_section(self):
        with tempfile.TemporaryDirectory() as directory:
            reg = Path(directory) / "user.reg"
            reg.write_bytes(b'WINE REGISTRY Version 2\n\n[Software\\\\Wine] 1\n"A"="b"\n')
            self.assertTrue(ps.disable_xrandr(reg))
            self.assertIn(b'[Software\\\\Wine\\\\X11 Driver]', reg.read_bytes())
            self.assertIn(b'"UseXRandR"="N"', reg.read_bytes())
            self.assertFalse(ps.disable_xrandr(reg))
            reg.write_bytes(b'[Software\\\\Wine\\\\X11 Driver] 1\n#time=1\n"UseXRandR"="Y"\n"Other"="1"\n\n[X] 2\n')
            self.assertTrue(ps.disable_xrandr(reg))
            text = reg.read_bytes()
            self.assertNotIn(b'"UseXRandR"="Y"', text)
            self.assertIn(b'#time=1\n"UseXRandR"="N"\n"UseXVidMode"="N"\n"Other"="1"\n\n[X] 2\n', text)


class UdpEcho:
    """Echoes every datagram, and remembers each sender address."""
    def __init__(self):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.bind(("127.0.0.1", 0))
        self.sock.settimeout(0.2)
        self.address = self.sock.getsockname()
        self.senders: set = set()
        self.received = 0
        self.running = True
        threading.Thread(target=self.loop, daemon=True).start()

    def loop(self):
        while self.running:
            try:
                data, addr = self.sock.recvfrom(65536)
            except socket.timeout:
                continue
            except OSError:
                return
            self.senders.add(addr)
            self.received += 1
            self.sock.sendto(data, addr)

    def close(self):
        self.running = False
        self.sock.close()


class TcpEcho:
    def __init__(self):
        self.sock = socket.socket()
        self.sock.bind(("127.0.0.1", 0))
        self.sock.listen()
        self.address = self.sock.getsockname()
        threading.Thread(target=self.loop, daemon=True).start()

    def loop(self):
        while True:
            try:
                conn, _ = self.sock.accept()
            except OSError:
                return
            threading.Thread(target=self.echo, args=(conn,), daemon=True).start()

    @staticmethod
    def echo(conn):
        with conn:
            while data := conn.recv(65536):
                conn.sendall(data)

    def close(self):
        self.sock.close()


class ForwardingTest(unittest.TestCase):
    """A real simulator between local sockets and echo servers."""

    def start(self, profile: dict, **kwargs):
        self.udp_echo, self.tcp_echo = UdpEcho(), TcpEcho()
        self.sim = ps.Simulator(ps.Profile.parse(profile, "test"), out=None, stats_every=0, **kwargs)
        self.udp = self.sim.add("udp", "match", ("127.0.0.1", 0), self.udp_echo.address)
        self.tcp = self.sim.add("tcp", "login", ("127.0.0.1", 0), self.tcp_echo.address)
        self.control = self.sim.listen_control(("127.0.0.1", 0))
        self.thread = threading.Thread(target=self.sim.run, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.sim.stop()
        self.thread.join(5)
        self.sim.shutdown()
        self.udp_echo.close()
        self.tcp_echo.close()

    def client(self):
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.bind(("127.0.0.1", 0))
        sock.settimeout(0.05)
        return sock

    def exchange(self, sock, count: int, gap: float = 0.002, wait: float = 1.0) -> list[bytes]:
        for i in range(count):
            sock.sendto(b"packet %d" % i, self.udp.address)
            time.sleep(gap)
        got, deadline = [], time.monotonic() + wait
        while time.monotonic() < deadline:
            try:
                got.append(sock.recv(65536))
            except socket.timeout:
                if len(got) >= count:
                    break
        return got

    def stats(self, port="match"):
        return {(row["peer"]): row for row in self.sim.stats() if row["port"] == port}

    def test_clean_round_trip_and_one_upstream_port_per_client(self):
        self.start({})
        a, b = self.client(), self.client()
        self.assertEqual(sorted(self.exchange(a, 50)), sorted(b"packet %d" % i for i in range(50)))
        self.assertEqual(len(self.exchange(b, 20)), 20)
        self.assertEqual(len(self.udp_echo.senders), 2)  # srp would see two clients
        stats = self.stats()
        self.assertEqual((stats[1]["up"]["in"], stats[1]["up"]["out"], stats[1]["down"]["out"]), (50, 50, 50))
        self.assertEqual(stats[2]["up"]["in"], 20)
        a.close(), b.close()

    def test_delay_is_applied_each_way(self):
        self.start({"link": {"delay_ms": 40}})
        sock = self.client()
        start = time.monotonic()
        sock.sendto(b"x", self.udp.address)
        sock.settimeout(2)
        self.assertEqual(sock.recv(10), b"x")
        self.assertGreaterEqual(time.monotonic() - start, 0.08)
        sock.close()

    def test_drop_event_for_one_client_only(self):
        self.start({"events": [{"port": "match", "peer": "2", "action": "drop", "at": 0, "duration": 60}]})
        a, b = self.client(), self.client()
        self.assertEqual(len(self.exchange(a, 10)), 10)
        self.assertEqual(self.exchange(b, 10, wait=0.3), [])
        self.assertEqual(self.stats()[2]["up"]["drop:event"], 10)
        a.close(), b.close()

    def test_loss_is_seeded(self):
        counts = []
        for _ in range(2):
            self.start({"seed": 5, "up": {"loss_pct": 30}})
            sock = self.client()
            got = self.exchange(sock, 200, gap=0.001)
            counts.append(sorted(got))
            sock.close()
            self.tearDown()
        self.start({})
        self.assertEqual(counts[0], counts[1])
        self.assertTrue(100 < len(counts[0]) < 180)

    def test_late_joiner_blackout_and_control(self):
        self.start({"events": [{"port": "match", "peer": "late:0.3", "anchor": "session", "at": 0, "duration": 30,
                                "action": "drop"}]})
        first = self.client()
        self.assertEqual(len(self.exchange(first, 5)), 5)
        time.sleep(0.4)
        self.assertEqual(len(self.exchange(first, 5)), 5)  # it keeps playing
        late = self.client()
        self.assertEqual(self.exchange(late, 5, wait=0.3), [])  # the late joiner is blacked out
        self.assertEqual(len(self.exchange(first, 3)), 3)
        stats = self.stats()
        self.assertEqual(stats[2]["up"]["drop:event"], 5)
        self.assertGreater(stats[2]["joined_after"], 0.3)
        self.assertEqual(stats[1]["joined_after"], 0)
        first.close(), late.close()

    def test_ctl_arms_a_drop_for_the_next_joiner(self):
        self.start({})
        control = "%s:%d" % self.control
        first = self.client()
        self.assertEqual(len(self.exchange(first, 3)), 3)
        reply = ps.control_request(control, {"cmd": "event", "event": {
            "port": "match", "action": "drop", "duration": 30, "peer": "new", "anchor": "session"}})
        self.assertTrue(reply["ok"], reply)
        second = self.client()
        self.assertEqual(self.exchange(second, 3, wait=0.3), [])
        self.assertEqual(len(self.exchange(first, 3)), 3)  # the one already there is untouched
        third = self.client()
        self.assertEqual(len(self.exchange(third, 3)), 3)  # and so is one after the next
        rows = ps.control_request(control, {"cmd": "stats"})["stats"]
        by_peer = {row["peer"]: row for row in rows if row["port"] == "match"}
        self.assertEqual(by_peer[2]["up"]["drop:event"], 3)
        self.assertNotIn("drop:event", by_peer[1]["up"])
        self.assertNotIn("drop:event", by_peer[3]["up"])
        # A blackout of everyone from now, for 0.3 s.
        self.assertTrue(ps.control_request(control, {"cmd": "event", "event": {
            "port": "match", "action": "drop", "duration": 0.3}})["ok"])
        self.assertEqual(self.exchange(first, 2, wait=0.1), [])
        time.sleep(0.3)
        self.assertEqual(len(self.exchange(first, 2)), 2)
        bad = ps.control_request(control, {"cmd": "event", "event": {"action": "boom"}})
        self.assertFalse(bad["ok"])
        first.close(), second.close(), third.close()

    def test_tcp_pass_through_and_stall(self):
        self.start({"events": [{"port": "login", "action": "hold", "at": 0, "duration": 0.5}]})
        start = time.monotonic()
        with socket.create_connection(self.tcp.address, timeout=3) as conn:
            conn.sendall(b"hello")
            self.assertEqual(conn.recv(10), b"hello")
        self.assertGreaterEqual(time.monotonic() - start, 0.45)
        with socket.create_connection(self.tcp.address, timeout=3) as conn:
            conn.sendall(b"again" * 1000)
            got = b""
            while len(got) < 5000:
                got += conn.recv(65536)
        self.assertEqual(got, b"again" * 1000)

    def test_log_records_drops(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "sim.jsonl"
            self.start({"up": {"loss_pct": 100}}, log=log)
            sock = self.client()
            self.exchange(sock, 3, wait=0.2)
            sock.close()
            self.sim.stop()
            self.thread.join(5)
            self.sim.shutdown()
            records = [json.loads(line) for line in log.read_text().splitlines()]
            self.assertEqual([r["verdict"] for r in records if "verdict" in r], ["drop:loss"] * 3)
            self.assertTrue(any(r.get("session") == "open" for r in records))


if __name__ == "__main__":
    unittest.main()
