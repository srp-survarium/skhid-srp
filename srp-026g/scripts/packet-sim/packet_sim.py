#!/usr/bin/env python3
"""packet-sim: a network impairment simulator between the 0.26g game and srp.

It listens on the addresses srp advertises and forwards every client's
datagrams (lobby, match: UDP) and streams (login, web: TCP) to where srp
really listens, impairing them on the way: loss, burst loss (Gilbert-Elliott),
delay and jitter, reordering, duplication, a bandwidth cap, and scripted
events (blackouts, stalls, a changed link) at set times or at a client's
join. Seeded, so the same packets meet the same fate. See README.md beside
this file.

Commands: run (the forwarder alone), local (srp + the forwarder + srp-bot
and headless game clients from a local-clients config), ctl (live control),
profiles, click and shot (drive and look at a headless game display).
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass, field, fields, replace
import heapq
import importlib.util
import itertools
import json
import math
import os
from pathlib import Path
import random
import selectors
import shutil
import signal
import socket
from statistics import NormalDist
import subprocess
import sys
import threading
import time

HERE = Path(__file__).resolve().parent
PROFILES = HERE / "profiles"
UP, DOWN = "up", "down"
# The four services of a local-clients config, in its `ports` order.
SERVICES = (("login", "tcp"), ("lobby", "udp"), ("match", "udp"), ("web", "tcp"))
LISTEN_HOST = "127.0.0.1"  # where the game connects: what srp advertises
TARGET_HOST = "127.0.0.2"  # where srp really listens (all of 127/8 is loopback)


# --------------------------------------------------------------------------
# The impairment model (pure: no sockets, no clock).

@dataclass(frozen=True)
class Link:
    """One direction's impairments. Percentages are 0-100, times in ms."""
    loss_pct: float = 0.0          # independent random loss
    burst_enter_pct: float = 0.0   # Gilbert-Elliott: P(good -> bad) per packet; 0 = off
    burst_exit_pct: float = 30.0   # P(bad -> good) per packet (mean burst = 100/exit packets)
    burst_loss_pct: float = 100.0  # loss while bad
    delay_ms: float = 0.0          # fixed one-way delay
    jitter_ms: float = 0.0         # standard deviation of a normal added delay (order kept)
    reorder_pct: float = 0.0       # a packet held back reorder_gap_ms, so later ones overtake it
    reorder_gap_ms: float = 20.0
    duplicate_pct: float = 0.0     # a second copy, up to max(jitter, 1 ms) later
    rate_kbps: float = 0.0         # bandwidth cap (kilobits/s); 0 = none
    queue_ms: float = 250.0        # at the cap: longest queue before tail drop

    @classmethod
    def keys(cls) -> set[str]:
        return {f.name for f in fields(cls)}

    def merged(self, overrides: dict) -> Link:
        return replace(self, **check_link(overrides)) if overrides else self


def check_link(values: dict) -> dict:
    unknown = set(values) - Link.keys()
    if unknown:
        raise ValueError(f"unknown link setting(s) {sorted(unknown)}; known: {sorted(Link.keys())}")
    out = {}
    for key, value in values.items():
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value < 0:
            raise ValueError(f"{key} must be a number >= 0, not {value!r}")
        if key.endswith("_pct") and value > 100:
            raise ValueError(f"{key} is a percentage (0-100), not {value}")
        out[key] = float(value)
    return out


@dataclass
class LinkState:
    """What one direction of one client remembers between packets."""
    rng: random.Random
    bad: bool = False            # Gilbert-Elliott state
    busy_until: float = 0.0      # the bandwidth cap's queue drains at
    last_delivery: float = 0.0   # in-order delivery: no packet before this


def _normal(u: float, sigma: float) -> float:
    return NormalDist(0.0, sigma).inv_cdf(min(max(u, 1e-9), 1 - 1e-9))


def decide(link: Link, state: LinkState, now: float, size: int,
           drop: bool = False, hold_until: float = 0.0) -> tuple[list[float], str]:
    """When a packet offered at `now` (seconds) is delivered: no times when it
    is lost, two when duplicated. And the verdict, for the stats and the log.

    Every packet draws the same seven variates in the same order whatever
    happens to it, so one packet's fate never shifts the next one's draws:
    the same seed and the same packet sequence give the same verdicts."""
    u_state, u_burst, u_loss, u_jitter, u_reorder, u_dup, u_gap = (state.rng.random() for _ in range(7))
    if drop:
        return [], "drop:event"
    if link.burst_enter_pct > 0:
        state.bad = (u_state >= link.burst_exit_pct / 100) if state.bad else (u_state < link.burst_enter_pct / 100)
        if state.bad and u_burst < link.burst_loss_pct / 100:
            return [], "drop:burst"
    if u_loss < link.loss_pct / 100:
        return [], "drop:loss"
    start = now
    if link.rate_kbps > 0:
        queued = max(now, state.busy_until)
        if queued - now > link.queue_ms / 1000:
            return [], "drop:queue"
        state.busy_until = queued + size * 8 / (link.rate_kbps * 1000)
        start = state.busy_until
    delay = link.delay_ms + (_normal(u_jitter, link.jitter_ms) if link.jitter_ms > 0 else 0.0)
    when = start + max(delay, 0.0) / 1000
    verdict = "pass"
    if u_reorder < link.reorder_pct / 100:
        when += link.reorder_gap_ms / 1000  # later packets do not wait for it
        verdict = "reorder"
    else:
        when = max(when, state.last_delivery)
    if hold_until and hold_until >= when:
        when, verdict = hold_until, "hold"
    if verdict != "reorder":
        state.last_delivery = max(state.last_delivery, when)
    times = [when]
    if u_dup < link.duplicate_pct / 100:
        times.append(when + u_gap * max(link.jitter_ms, 1.0) / 1000)
        verdict = "duplicate" if verdict == "pass" else verdict + "+duplicate"
    return times, verdict


# --------------------------------------------------------------------------
# What a match datagram carries (transport.md §2, §3, §4): the message ids,
# for rules that pick packets by them. Best effort: a datagram it cannot
# read gives the ids read so far (an empty tuple at worst), never an error.

# Server to client: id -> (fixed payload length or None for a length prefix,
# whether a unique/order byte follows the id). ED/EE are worked out from
# their mask (3 bytes in a game with the late join, revision 5 on).
DOWN_IDS = {0xE3: (4, False), 0xE4: (None, True), 0xE5: (7, True), 0xE6: (4, True), 0xE7: (2, True),
            0xE8: (None, True), 0xE9: (2, False), 0xEA: (2, False), 0xEB: (10, False), 0xEC: (11, False),
            0xED: ("mask", False), 0xEE: ("mask", False), 0xEF: (2, False), 0xF0: (4, False),
            0xF1: (1, False), 0xF2: (2, False), 0xF3: (3, False), 0xF4: (2, False), 0xF5: (1, False),
            0xF6: (None, True)}
# Client to server: all fixed, none with a unique byte; an 0xE0 with a
# payload is a patched game's snapshot chunk or late-join answer
# (`upload_len`, as srp's `udp_transport::wire::carrier_len`).
UP_IDS = {0xD8: 5, 0xD9: 4, 0xDA: 0, 0xDB: 6, 0xDC: 9, 0xDD: 10, 0xDE: 2, 0xDF: 5, 0xE0: 0, 0xE1: 0}
LOW_LEVEL = range(0xFA, 0xFF)
MASK_BYTES = 3


def upload_len(rest: bytes) -> int | None:
    """The length of an 0xE0's payload in a multi datagram, or None for a
    plain leave: a late-join answer (an impossible chunk head, 20 bytes) or
    a snapshot chunk (`u32 time, u16 offset, u16 total`, its data)."""
    if len(rest) < 8:
        return None
    if rest[:8] == b"\xff" * 6 + b"\0\0":
        return 20 if len(rest) >= 20 else None
    time, offset, total = int.from_bytes(rest[:4], "little"), int.from_bytes(rest[4:6], "little"), \
        int.from_bytes(rest[6:8], "little")
    if not (time % 50 == 0 and offset % 480 == 0 and offset < total <= 0x8000):
        return None
    length = 8 + min(480, total - offset)
    return length if len(rest) >= length else None


def message_ids(data: bytes, direction: str) -> tuple[int, ...]:
    """The message ids of a 0.26g match datagram, in order (low-level ones,
    0xFA..0xFE, included). Split parts give their message's id."""
    if len(data) < 4:
        return ()
    ack0, end = data[2], len(data)
    if ack0 & 0x80:  # tail ack bytes, read back from the end until one without bit 7
        while end > 3 and data[end - 1] & 0x80:
            end -= 1
        if end <= 3:
            return ()
        multi = data[end - 1] & 1
        end -= 1
    else:
        multi = ack0 & 1
    area = data[3:end]
    ids: list[int] = []
    i = 0
    while i < len(area):
        mid = area[i]
        i += 1
        if mid in LOW_LEVEL:
            ids.append(mid)
            break
        split = mid == 0xFF
        if split:
            if i >= len(area):
                break
            mid = area[i]
            i += 1
        ids.append(mid)
        if not multi:
            break
        if direction == UP:
            if mid not in UP_IDS:
                break
            length, extra = UP_IDS[mid], False
            if mid == 0xE0 and not split:
                length = upload_len(area[i:]) or 0
        else:
            if mid not in DOWN_IDS:
                break
            length, extra = DOWN_IDS[mid]
        i += 1 if extra else 0
        if split:
            if i >= len(area):
                break
            part = area[i]
            i += 1 + (1 if part == 0 else 0)
            if length is not None and length != "mask":
                length = None  # a part of a variable id carries a length in a multi datagram
        if length == "mask":
            head = 1 if mid == 0xED else 2
            if i + head + MASK_BYTES > len(area):
                break
            bits = sum(bin(b).count("1") for b in area[i + head:i + head + MASK_BYTES])
            length = head + MASK_BYTES + 8 * bits
        elif length is None:
            if i >= len(area):
                break
            length = area[i]
            i += 1
            if length == 0xFF:
                if i + 2 > len(area):
                    break
                length = int.from_bytes(area[i:i + 2], "little")
                i += 2
        i += length
    return tuple(ids)


def parse_ids(text) -> frozenset[int]:
    """`E4,E8` / `0xe4 e8` / a list of numbers: message ids (hex)."""
    words = text if isinstance(text, (list, tuple)) else str(text).replace(",", " ").split()
    out = set()
    for word in words:
        value = word if isinstance(word, int) else int(str(word).lower().removeprefix("0x"), 16)
        if not 0 <= value <= 0xFF:
            raise ValueError(f"a message id is a byte, not {word!r}")
        out.add(value)
    if not out:
        raise ValueError("ids names no message id")
    return frozenset(out)


# --------------------------------------------------------------------------
# Rules, events and profiles.

ACTIONS = ("set", "drop", "hold")
ANCHORS = ("start", "session")


@dataclass
class Rule:
    """A link change for some ports, clients and directions: always (a rule,
    `at` is None) or in a time window (an event). `drop` loses everything,
    `hold` stalls it to the window's end (then delivers in order), `set`
    changes the link settings in `link`."""
    port: str = "*"
    peer: str = "*"
    dir: str = "both"
    action: str = "set"
    link: dict = field(default_factory=dict)
    at: float | None = None
    duration: float = math.inf
    every: float = 0.0
    anchor: str = "start"
    label: str = ""
    ids: frozenset | None = None   # only datagrams with one of these message ids (match port)
    count: int = 0                 # drop/hold at most this many datagrams a client, direction and window; 0 = all
    matchers: list = field(default_factory=list, repr=False, compare=False)

    @classmethod
    def parse(cls, data: dict, event: bool) -> Rule:
        data = dict(data)
        scope = {k: data.pop(k) for k in ("port", "peer", "dir", "action", "at", "duration", "every", "anchor", "label",
                                          "count")
                 if k in data}
        ids = parse_ids(data.pop("ids")) if "ids" in data else None
        link = dict(data.pop("link", {}))
        link.update({k: data.pop(k) for k in list(data) if k in Link.keys()})
        if data:
            raise ValueError(f"unknown key(s) {sorted(data)} in {'event' if event else 'rule'}")
        rule = cls(link=check_link(link), ids=ids, **{k: v for k, v in scope.items()})
        rule.port, rule.peer, rule.label = str(rule.port), str(rule.peer), str(rule.label)
        rule.count = int(rule.count)
        if rule.count < 0:
            raise ValueError("count must be >= 0")
        if rule.count and rule.action == "set":
            raise ValueError("count is for drop and hold events")
        if rule.dir not in ("up", "down", "both"):
            raise ValueError(f"dir must be up, down or both, not {rule.dir!r}")
        if rule.action not in ACTIONS:
            raise ValueError(f"action must be one of {ACTIONS}, not {rule.action!r}")
        rule.anchor_id = None
        if str(rule.anchor).lower().startswith("id:"):
            ids = parse_ids(rule.anchor[3:])
            if len(ids) != 1:
                raise ValueError("anchor id:XX names one message id")
            rule.anchor_id = next(iter(ids))
        elif rule.anchor not in ANCHORS:
            raise ValueError(f"anchor must be one of {ANCHORS} or id:XX (a client's first datagram with message "
                             f"XX, either way), not {rule.anchor!r}")
        if event:
            if rule.at is None:
                rule.at = 0.0
            rule.at, rule.duration, rule.every = float(rule.at), float(rule.duration), float(rule.every)
            if rule.at < 0 or not rule.duration > 0 or rule.every < 0:
                raise ValueError("an event needs at >= 0, duration > 0 and every >= 0")
            if rule.every and rule.every < rule.duration:
                raise ValueError("every must be at least the duration")
        elif rule.at is not None or rule.anchor != "start":
            raise ValueError("a rule has no at/anchor; put timed changes under events")
        if not event and rule.action != "set":
            raise ValueError("a rule (always on) can only set; use an event to drop or hold")
        rule.matchers = [peer_matcher(s) for s in rule.peer.split(",")]  # validates, once
        if not rule.label:
            rule.label = f"{rule.action} {rule.port}/{rule.peer}/{rule.dir}" + (f"@{rule.at:g}s" if event else "")
            if rule.ids:
                rule.label += " ids " + ",".join(f"{i:02X}" for i in sorted(rule.ids))
            if rule.count:
                rule.label += f" x{rule.count}"
        return rule

    def applies(self, port: str, direction: str) -> bool:
        return (self.port == "*" or port in self.port.split(",")) and self.dir in ("both", direction)

    def selects(self, session) -> bool:
        return any(matcher(session) for matcher in self.matchers)

    def window(self, t: float, session) -> tuple[int, float] | None:
        """(window number, its end) when the rule is on at `t` for `session`
        (or a session's start time, for `anchor` start and session), else None."""
        if self.at is None:
            return 0, math.inf
        if getattr(self, "anchor_id", None) is not None:
            seen = getattr(session, "first_seen", {}).get(self.anchor_id)
            if seen is None:
                return None
            origin = self.at + seen
        else:
            start = session if isinstance(session, (int, float)) else session.started
            origin = self.at + (start if self.anchor == "session" else 0.0)
        if t < origin:
            return None
        k = math.floor((t - origin) / self.every) if self.every else 0
        end = origin + k * self.every + self.duration
        return (k, end) if t < end else None


def peer_matcher(selector: str):
    """A client selector: * (everyone), N (the N-th client to send on that
    port, from 1), addr:HOST:PORT, new-after:S (a session that began S or
    more seconds after the start), next-after:S (the first of those on
    each port, alone), late:S (a session that began while another client
    of that port had been there S seconds: a late joiner)."""
    selector = selector.strip()
    if selector in ("*", "all"):
        return lambda peer: True
    if selector.isdecimal():
        index = int(selector)
        return lambda peer: peer.index == index
    kind, _, value = selector.partition(":")
    if kind == "addr" and value:
        return lambda peer: peer.label_addr == value
    if kind == "next-after":
        seconds = float(value)
        chosen: dict = {}  # port -> (index, generation) of the first session from `seconds` on

        def first(peer) -> bool:
            if peer.started < seconds:
                return False
            port = getattr(getattr(peer, "forwarder", None), "name", "")
            key = (peer.index, getattr(peer, "generation", 1))
            return chosen.setdefault(port, key) == key
        return first
    if kind in ("new-after", "late"):
        seconds = float(value)
        if kind == "new-after":
            return lambda peer: peer.started >= seconds
        return lambda peer: peer.joined_after >= seconds
    raise ValueError(f"unknown client selector {selector!r} (*, N, addr:HOST:PORT, new-after:S, next-after:S, late:S)")


@dataclass
class Profile:
    name: str = "clean"
    description: str = ""
    seed: int = 1
    rules: list[Rule] = field(default_factory=list)
    events: list[Rule] = field(default_factory=list)

    @classmethod
    def parse(cls, data: dict, name: str) -> Profile:
        data = dict(data)
        profile = cls(name=name, description=str(data.pop("description", "")), seed=int(data.pop("seed", 1)))
        for key, direction in (("link", "both"), ("up", "up"), ("down", "down")):
            if key in data:
                profile.rules.append(Rule.parse({"dir": direction, "link": data.pop(key)}, event=False))
        profile.rules += [Rule.parse(r, event=False) for r in data.pop("rules", [])]
        profile.events += [Rule.parse(e, event=True) for e in data.pop("events", [])]
        if data:
            raise ValueError(f"unknown profile key(s) {sorted(data)}")
        return profile

    @classmethod
    def load(cls, name_or_path: str) -> Profile:
        path = Path(name_or_path)
        if not path.suffix and not path.exists():
            path = PROFILES / f"{name_or_path}.json"
        return cls.parse(json.loads(path.read_text()), path.stem)


def spec_dict(text: str) -> dict:
    """A rule or event as JSON, or as `key=value` words:
    `port=match peer=2 action=drop at=30 duration=3` / `dir=down loss_pct=5`."""
    text = text.strip()
    if text.startswith("{"):
        return json.loads(text)
    data: dict = {}
    for word in text.split():
        key, sep, value = word.partition("=")
        if not sep:
            raise ValueError(f"expected key=value, not {word!r}")
        data[key] = (float(value) if key in Link.keys() or key in ("at", "duration", "every")
                     else int(value) if key == "count" else value)
    return data


def parse_spec(text: str, event: bool) -> Rule:
    return Rule.parse(spec_dict(text), event)


# --------------------------------------------------------------------------
# The forwarder.

class Stream:
    """One direction of one client session: its link state and counters."""
    def __init__(self, rng: random.Random):
        self.state = LinkState(rng)
        self.counts: dict[str, float] = {}

    def count(self, key: str, amount: float = 1) -> None:
        self.counts[key] = self.counts.get(key, 0) + amount


class Session:
    def __init__(self, forwarder, addr, index: int, generation: int, started: float, joined_after: float, seed: int):
        self.forwarder, self.addr, self.index, self.generation = forwarder, addr, index, generation
        self.started = self.last_seen = started
        # How long the longest-present other client of this port had been
        # there when this one came: a late joiner's is the match's age.
        self.joined_after = joined_after
        self.label_addr = f"{addr[0]}:{addr[1]}"
        self.streams = {d: Stream(random.Random(f"{seed}/{forwarder.name}/{index}/{generation}/{d}"))
                        for d in (UP, DOWN)}
        self.upstream: socket.socket | None = None
        self.closed = False
        self.first_seen: dict[int, float] = {}  # message id -> when it first passed here (either way)

    @property
    def name(self) -> str:
        return f"{self.forwarder.name}#{self.index}"

    def via(self) -> str:
        try:
            return "%s:%d" % self.upstream.getsockname()[:2] if self.upstream else "-"
        except OSError:
            return "-"


class UdpForwarder:
    kind = "udp"

    def __init__(self, sim: Simulator, name: str, listen: tuple[str, int], target: tuple[str, int]):
        self.sim, self.name, self.target = sim, name, target
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.bind(listen)
        self.sock.setblocking(False)
        self.address = self.sock.getsockname()
        self.sessions: dict = {}
        self.indices: dict = {}
        self.generations: dict = {}
        sim.selector.register(self.sock, selectors.EVENT_READ, self.on_client)

    def session(self, addr) -> Session:
        session = self.sessions.get(addr)
        if session is not None:
            return session
        now = self.sim.now()
        index = self.indices.setdefault(addr, len(self.indices) + 1)
        generation = self.generations[addr] = self.generations.get(addr, 0) + 1
        present = [now - s.started for s in self.sessions.values() if now - s.last_seen < 2]
        session = Session(self, addr, index, generation, now, max(present, default=0.0), self.sim.seed)
        upstream = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        upstream.connect(self.target)  # one source port per client: srp tells them apart
        upstream.setblocking(False)
        session.upstream = upstream
        session.send_up = upstream.send
        session.send_down = lambda data, sock=self.sock, addr=addr: sock.sendto(data, addr)
        self.sim.selector.register(upstream, selectors.EVENT_READ, lambda s=session: self.on_server(s))
        self.sessions[addr] = session
        self.sim.session_opened(session)
        return session

    def on_client(self) -> None:
        for _ in range(256):
            try:
                data, addr = self.sock.recvfrom(65536)
            except (BlockingIOError, InterruptedError):
                return
            except OSError:
                continue  # an ICMP error of an earlier sendto to a client that is gone
            session = self.session(addr[:2])
            session.last_seen = self.sim.now()
            self.sim.offer(session, UP, data, session.send_up)

    def on_server(self, session: Session) -> None:
        for _ in range(256):
            try:
                data = session.upstream.recv(65536)
            except (BlockingIOError, InterruptedError):
                return
            except OSError:
                session.streams[DOWN].count("srp_unreachable")
                return
            self.sim.offer(session, DOWN, data, session.send_down)

    def expire(self, now: float, idle: float) -> None:
        for addr, session in list(self.sessions.items()):
            if now - session.last_seen > idle:
                self.close(session)

    def close(self, session: Session) -> None:
        if session.closed:
            return
        session.closed = True
        self.sessions.pop(session.addr, None)
        try:
            self.sim.selector.unregister(session.upstream)
        except (KeyError, ValueError):
            pass
        session.upstream.close()
        self.sim.session_closed(session)

    def shutdown(self) -> None:
        for session in list(self.sessions.values()):
            self.close(session)
        self.sim.selector.unregister(self.sock)
        self.sock.close()


class TcpForwarder:
    """TCP pass-through (login, web). TCP retransmits by itself, so loss is
    not modelled: a drop or hold event stalls the stream for its window
    (both directions' bytes wait), and delay_ms delays each chunk."""
    kind = "tcp"

    def __init__(self, sim: Simulator, name: str, listen: tuple[str, int], target: tuple[str, int]):
        self.sim, self.name, self.target = sim, name, target
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind(listen)
        self.sock.listen(64)
        self.address = self.sock.getsockname()
        self.sessions: dict = {}
        self.indices: dict = {}
        self.stopping = False
        threading.Thread(target=self.accept_loop, daemon=True, name=f"tcp-{name}").start()

    def accept_loop(self) -> None:
        while not self.stopping:
            try:
                conn, addr = self.sock.accept()
            except OSError:
                return
            threading.Thread(target=self.handle, args=(conn, addr[:2]), daemon=True).start()

    def handle(self, conn: socket.socket, addr) -> None:
        with self.sim.lock:
            now = self.sim.now()
            index = self.indices.setdefault(addr, len(self.indices) + 1)
            session = Session(self, addr, index, 1, now, 0.0, self.sim.seed)
            self.sessions[addr] = session
        try:
            upstream = socket.create_connection(self.target, timeout=5)
            upstream.settimeout(None)
        except OSError:
            session.streams[UP].count("srp_unreachable")
            conn.close()
            with self.sim.lock:
                self.sessions.pop(addr, None)
            return
        session.upstream = upstream
        with self.sim.lock:
            self.sim.session_opened(session)
        pumps = [threading.Thread(target=self.pump, args=(conn, upstream, session, UP), daemon=True),
                 threading.Thread(target=self.pump, args=(upstream, conn, session, DOWN), daemon=True)]
        for pump in pumps:
            pump.start()
        for pump in pumps:
            pump.join()
        conn.close()
        upstream.close()
        with self.sim.lock:
            session.closed = True
            self.sessions.pop(addr, None)
            self.sim.session_closed(session)

    def pump(self, source: socket.socket, sink: socket.socket, session: Session, direction: str) -> None:
        stream = session.streams[direction]
        try:
            while chunk := source.recv(65536):
                with self.sim.lock:
                    stream.count("in")
                    stream.count("in_bytes", len(chunk))
                while True:
                    with self.sim.lock:
                        now = self.sim.now()
                        link, drop, hold = self.sim.resolve(session, direction, now)
                    if not drop and hold <= now:
                        break
                    with self.sim.lock:
                        stream.count("stalled_s", 0.05)
                    time.sleep(0.05)
                if link.delay_ms:
                    time.sleep(link.delay_ms / 1000)
                sink.sendall(chunk)
                with self.sim.lock:
                    stream.count("out")
                    stream.count("out_bytes", len(chunk))
        except OSError:
            pass
        finally:
            try:
                sink.shutdown(socket.SHUT_WR)
            except OSError:
                pass

    def expire(self, now: float, idle: float) -> None:
        pass

    def shutdown(self) -> None:
        self.stopping = True
        self.sock.close()


class Simulator:
    def __init__(self, profile: Profile, seed: int | None = None, log: Path | None = None, log_all: bool = False,
                 log_bytes: int = 0, stats_every: float = 10.0, session_idle: float = 30.0,
                 out=sys.stderr, clock=time.monotonic):
        self.profile = profile
        self.seed = profile.seed if seed is None else seed
        self.rules = list(profile.rules)
        self.events = list(profile.events)
        self.clock, self.t0 = clock, clock()
        self.selector = selectors.DefaultSelector()
        self.lock = threading.RLock()
        self.heap: list = []
        self.order = itertools.count()
        self.forwarders: list = []
        self.closed: list[Session] = []
        self.hits: set = set()
        self.counted: dict = {}
        self.log = log.open("a", buffering=1) if log else None
        self.log_all, self.log_bytes = log_all, log_bytes
        self.stats_every, self.session_idle, self.out = stats_every, session_idle, out
        self.next_stats = stats_every
        self.stop_flag = threading.Event()
        self.control: socket.socket | None = None
        self.down = False

    def now(self) -> float:
        return self.clock() - self.t0

    @property
    def anchor_ids(self) -> set[int]:
        """The message ids some event counts from (`anchor` id:XX)."""
        return {r.anchor_id for r in itertools.chain(self.rules, self.events) if getattr(r, "anchor_id", None) is not None}

    # -- setup
    def add(self, kind: str, name: str, listen: tuple[str, int], target: tuple[str, int]):
        forwarder = (UdpForwarder if kind == "udp" else TcpForwarder)(self, name, listen, target)
        self.forwarders.append(forwarder)
        return forwarder

    def listen_control(self, address: tuple[str, int]) -> tuple[str, int]:
        self.control = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.control.bind(address)
        self.control.setblocking(False)
        self.selector.register(self.control, selectors.EVENT_READ, self.on_control)
        return self.control.getsockname()

    # -- the model applied
    def resolve(self, session: Session, direction: str, t: float,
                data: bytes | None = None) -> tuple[Link, bool, float]:
        """The link, whether a drop is on, and until when a hold is, for one
        client's direction at `t` (for the datagram `data`: a rule with
        `ids` takes only datagrams with one of them; without `data`, a TCP
        chunk, such a rule does not apply)."""
        link, drop, hold = Link(), False, 0.0
        port = session.forwarder.name
        ids = None
        for rule in itertools.chain(self.rules, self.events):
            if not rule.applies(port, direction):
                continue
            window = rule.window(t, session)
            if window is None or not rule.selects(session):
                continue
            if rule.ids is not None:
                if data is None:
                    continue
                if ids is None:
                    ids = set(message_ids(data, direction))
                if not ids & rule.ids:
                    continue
            if rule.count:
                key = (id(rule), session.name, session.generation, direction, window[0])
                used = self.counted.get(key, 0)
                if used >= rule.count:
                    continue
                self.counted[key] = used + 1
            if rule.at is not None:
                key = (id(rule), session.name, session.generation, direction, window[0])
                if key not in self.hits:
                    self.hits.add(key)
                    self.say(f"event `{rule.label}` hits {session.name} {session.label_addr} {direction}"
                             + (f" until {window[1]:.1f}s" if math.isfinite(window[1]) else ""))
                    self.write_log({"t": round(t, 4), "event": rule.label, "port": port, "peer": session.index,
                                    "addr": session.label_addr, "dir": direction, "until": window[1]})
            if rule.action == "set":
                link = link.merged(rule.link)
            elif rule.action == "drop":
                drop = True
            else:
                hold = max(hold, window[1])
        return link, drop, hold

    def offer(self, session: Session, direction: str, data: bytes, send) -> None:
        t = self.now()
        stream = session.streams[direction]
        if self.anchor_ids and session.forwarder.kind == "udp":
            for mid in self.anchor_ids.intersection(message_ids(data, direction)):
                if mid not in session.first_seen:
                    session.first_seen[mid] = t
                    self.write_log({"t": round(t, 4), "first": f"{mid:02X}", "port": session.forwarder.name,
                                    "peer": session.index, "dir": direction})
        with self.lock:
            link, drop, hold = self.resolve(session, direction, t, data)
        times, verdict = decide(link, stream.state, t, len(data), drop, hold)
        stream.count("in")
        stream.count("in_bytes", len(data))
        if not times:
            stream.count(verdict)
        else:
            if verdict != "pass":
                for part in verdict.split("+"):
                    stream.count(part)
            stream.count("delay_ms", (times[0] - t) * 1000)
            for when in times:
                heapq.heappush(self.heap, (when, next(self.order), send, data, stream))
        if self.log and (self.log_all or verdict != "pass"):
            record = {"t": round(t, 4), "port": session.forwarder.name, "peer": session.index,
                      "addr": session.label_addr, "dir": direction, "len": len(data), "verdict": verdict}
            if times:
                record["delay_ms"] = round((times[0] - t) * 1000, 2)
            if self.log_bytes:
                record["head"] = data[:self.log_bytes].hex()
            if session.forwarder.name == "match":
                record["ids"] = " ".join(f"{i:02X}" for i in message_ids(data, direction))
            self.write_log(record)

    def deliver_due(self) -> float | None:
        now = self.now()
        while self.heap and self.heap[0][0] <= now:
            _, _, send, data, stream = heapq.heappop(self.heap)
            try:
                send(data)
                stream.count("out")
            except OSError:
                stream.count("send_error")
        return self.heap[0][0] - now if self.heap else None

    # -- the loop
    def run(self, duration: float | None = None) -> None:
        end = None if duration is None else self.now() + duration
        while not self.stop_flag.is_set():
            wait = self.deliver_due()
            wait = 0.2 if wait is None else min(max(wait, 0.0), 0.2)
            for key, _ in self.selector.select(wait):
                key.data()
            self.deliver_due()
            now = self.now()
            for forwarder in self.forwarders:
                forwarder.expire(now, self.session_idle)
            if self.stats_every and now >= self.next_stats:
                self.next_stats = now + self.stats_every
                self.print_stats(live=True)
            if end is not None and now >= end:
                break

    def stop(self, *_) -> None:
        self.stop_flag.set()

    def shutdown(self) -> None:
        if self.down:
            return
        self.down = True
        self.deliver_due()
        for forwarder in self.forwarders:
            forwarder.shutdown()
        if self.control:
            self.control.close()
        if self.log:
            self.log.close()
            self.log = None

    # -- control
    def on_control(self) -> None:
        try:
            data, addr = self.control.recvfrom(65536)
        except OSError:
            return
        try:
            reply = self.command(json.loads(data))
        except (ValueError, KeyError, TypeError) as error:
            reply = {"ok": False, "error": str(error)}
        try:
            self.control.sendto(json.dumps(reply).encode(), addr)
        except OSError:
            pass

    def command(self, request: dict) -> dict:
        cmd = request["cmd"]
        if cmd == "stats":
            return {"ok": True, "t": round(self.now(), 3), "stats": self.stats()}
        if cmd == "event":
            spec = dict(request["event"])
            now = self.now()
            if spec.get("peer") == "new":
                # The next client to come on that port, alone (unrounded:
                # it may come within the millisecond).
                spec["peer"] = f"next-after:{now!r}"
            if spec.get("anchor", "start") == "start":
                spec["at"] = float(spec.get("at", 0)) + now  # ctl's `at` counts from now
            rule = Rule.parse(spec, event=True)
            with self.lock:
                self.events.append(rule)
            self.say(f"ctl: added event `{rule.label}`")
            return {"ok": True, "label": rule.label, "events": len(self.events)}
        if cmd == "clear":
            with self.lock:
                self.events = list(self.profile.events)
            self.say("ctl: cleared ctl events")
            return {"ok": True}
        if cmd == "set":
            rule = Rule.parse(request["rule"], event=False)
            with self.lock:
                self.rules.append(rule)
            self.say(f"ctl: added rule `{rule.label}`")
            return {"ok": True, "label": rule.label}
        raise ValueError(f"unknown command {cmd!r} (stats, event, set, clear)")

    # -- reporting
    def say(self, text: str) -> None:
        if self.out:
            print(f"[{self.now():8.1f}s] {text}", file=self.out, flush=True)

    def write_log(self, record: dict) -> None:
        if self.log:
            self.log.write(json.dumps(record) + "\n")

    def session_opened(self, session: Session) -> None:
        late = (f", {session.joined_after:.1f}s after the first client still here"
                if session.joined_after >= 0.05 else "")
        self.say(f"{session.forwarder.kind} {session.name}: client {session.label_addr} "
                 f"via {session.via()} -> {session.forwarder.target[0]}:{session.forwarder.target[1]}{late}")
        self.write_log({"t": round(self.now(), 4), "session": "open", "port": session.forwarder.name,
                        "peer": session.index, "addr": session.label_addr, "via": session.via(),
                        "joined_after": round(session.joined_after, 3)})

    def session_closed(self, session: Session) -> None:
        self.closed.append(session)
        self.write_log({"t": round(self.now(), 4), "session": "close", "port": session.forwarder.name,
                        "peer": session.index, "addr": session.label_addr})

    def sessions(self) -> list[Session]:
        with self.lock:
            live = [s for f in self.forwarders for s in list(f.sessions.values())]
            return self.closed + live

    def stats(self) -> list[dict]:
        rows = []
        for s in self.sessions():
            rows.append({"port": s.forwarder.name, "kind": s.forwarder.kind, "peer": s.index, "addr": s.label_addr,
                         "generation": s.generation, "started": round(s.started, 3), "closed": s.closed,
                         "joined_after": round(s.joined_after, 3),
                         # dict() copies at once: the loop thread may be counting.
                         **{d: {k: round(v, 3) for k, v in dict(st.counts).items()} for d, st in s.streams.items()}})
        return rows

    def print_stats(self, live: bool = False) -> None:
        if not self.out:
            return
        rows = [s for s in self.sessions() if not live or not s.closed]
        if not rows:
            return
        self.say("stats" if live else "final stats")
        for s in rows:
            parts = []
            for direction, stream in s.streams.items():
                c = stream.counts
                drops = {k[5:]: int(v) for k, v in c.items() if k.startswith("drop:")}
                text = f"{direction} {int(c.get('in', 0))}>{int(c.get('out', 0))}"
                if drops:
                    text += " drop " + "/".join(f"{k} {v}" for k, v in sorted(drops.items()))
                for key in ("duplicate", "reorder", "hold", "send_error", "srp_unreachable"):
                    if c.get(key):
                        text += f" {key} {int(c[key])}"
                if c.get("stalled_s"):
                    text += f" stalled {c['stalled_s']:.1f}s"
                delivered = c.get("in", 0) - sum(drops.values())
                if c.get("delay_ms") and delivered > 0:
                    text += f" +{c['delay_ms'] / delivered:.0f}ms"
                parts.append(text)
            state = " (closed)" if s.closed else ""
            print(f"    {s.name:9} {s.label_addr:21} {' | '.join(parts)}{state}", file=self.out, flush=True)


# --------------------------------------------------------------------------
# Command line.

def address(text: str, default_host: str = LISTEN_HOST) -> tuple[str, int]:
    host, sep, port = text.rpartition(":")
    return (host if sep and host else default_host), int(port)


def forward_spec(text: str) -> tuple[str, tuple[str, int], tuple[str, int]]:
    """NAME=[HOST:]PORT/[HOST:]PORT: listen on the first, forward to the second."""
    name, sep, rest = text.partition("=")
    listen, slash, target = rest.partition("/")
    if not sep or not slash or not name:
        raise argparse.ArgumentTypeError(f"expected NAME=[HOST:]PORT/[HOST:]PORT, not {text!r}")
    return name, address(listen, LISTEN_HOST), address(target, TARGET_HOST)


def add_sim_arguments(parser: argparse.ArgumentParser) -> None:
    group = parser.add_argument_group("impairments")
    group.add_argument("--profile", default="clean", help="a name under profiles/ or a JSON file (default clean)")
    group.add_argument("--seed", type=int, help="the random seed (default: the profile's)")
    group.add_argument("--set", action="append", default=[], metavar="SPEC",
                       help="an always-on rule after the profile's: 'loss_pct=3 delay_ms=40' with optional "
                            "port=, peer=, dir= (or JSON); repeatable")
    group.add_argument("--event", action="append", default=[], metavar="SPEC",
                       help="a timed event: 'port=match action=drop at=30 duration=3 [peer=2] [dir=up] "
                            "[every=60] [anchor=session]' (or JSON); repeatable")
    out = parser.add_argument_group("output")
    out.add_argument("--log", type=Path, help="JSON lines: every drop/hold/duplicate/reorder, sessions, events")
    out.add_argument("--log-all", action="store_true", help="log every packet, passed ones too")
    out.add_argument("--log-bytes", type=int, default=0, metavar="N", help="log the first N bytes of each, as hex")
    out.add_argument("--stats-every", type=float, default=10.0, metavar="S", help="live stats period (0: off)")
    out.add_argument("--stats-json", type=Path, help="write the final per-client stats here")
    out.add_argument("--control", metavar="[HOST:]PORT", help="a UDP control port for `ctl` (loopback)")
    parser.add_argument("--session-idle", type=float, default=30.0, metavar="S",
                        help="a UDP client silent this long is forgotten; its next packet starts a new session")
    parser.add_argument("--duration", type=float, metavar="S", help="stop after S seconds")


def build_simulator(args) -> Simulator:
    profile = Profile.load(args.profile)
    profile.rules += [parse_spec(s, event=False) for s in args.set]
    profile.events += [parse_spec(s, event=True) for s in args.event]
    if args.log:
        args.log.parent.mkdir(parents=True, exist_ok=True)
    return Simulator(profile, seed=args.seed, log=args.log, log_all=args.log_all, log_bytes=args.log_bytes,
                     stats_every=args.stats_every, session_idle=args.session_idle)


def run_simulator(sim: Simulator, args) -> None:
    if args.control:
        host, port = sim.listen_control(address(args.control, "127.0.0.1"))
        sim.say(f"control on {host}:{port} (packet_sim.py ctl --control {host}:{port} stats)")
    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, sim.stop)
    sim.say(f"profile {sim.profile.name} (seed {sim.seed}): {sim.profile.description or '-'}")
    try:
        sim.run(args.duration)
    finally:
        sim.print_stats()
        if args.stats_json:
            args.stats_json.write_text(json.dumps(sim.stats(), indent=1) + "\n")
        sim.shutdown()


def srp_forwards(ports: list[int], listen_host: str, target_host: str):
    return [(kind, name, (listen_host, port), (target_host, port)) for (name, kind), port in zip(SERVICES, ports)]


def cmd_run(args) -> int:
    sim = build_simulator(args)
    forwards = []
    if args.srp_ports:
        ports = [int(p) for p in args.srp_ports.split(",")]
        if len(ports) != 4:
            raise ValueError("--srp-ports takes login,lobby,match,web")
        forwards += srp_forwards(ports, args.listen_host, args.target_host)
    forwards += [("udp", *forward_spec(s)) for s in args.udp] + [("tcp", *forward_spec(s)) for s in args.tcp]
    if not forwards:
        raise ValueError("nothing to forward: give --srp-ports and/or --udp/--tcp")
    for kind, name, listen, target in forwards:
        forwarder = sim.add(kind, name, listen, target)
        sim.say(f"{kind} {name}: {forwarder.address[0]}:{forwarder.address[1]} -> {target[0]}:{target[1]}")
    run_simulator(sim, args)
    return 0


def control_request(control: str, request: dict, timeout: float = 2.0) -> dict:
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.settimeout(timeout)
        sock.sendto(json.dumps(request).encode(), address(control, "127.0.0.1"))
        return json.loads(sock.recv(1 << 20))


def cmd_ctl(args) -> int:
    if not args.control:
        raise ValueError("give --control HOST:PORT (or PACKET_SIM_CONTROL)")
    if args.action == "stats":
        request = {"cmd": "stats"}
    elif args.action == "clear":
        request = {"cmd": "clear"}
    elif args.action == "event":
        request = {"cmd": "event", "event": spec_dict(" ".join(args.words))}
    elif args.action == "set":
        request = {"cmd": "set", "rule": spec_dict(" ".join(args.words))}
    else:  # drop / hold PORT SECONDS
        if len(args.words) != 2:
            raise ValueError(f"ctl {args.action} PORT SECONDS")
        event = {"port": args.words[0], "action": args.action, "duration": float(args.words[1]),
                 "peer": args.peer, "dir": args.dir, "at": args.after}
        if args.ids:
            event["ids"] = args.ids
        if args.count:
            event["count"] = args.count
        if args.on_join:
            event.update(peer="new" if args.peer == "*" else args.peer, anchor="session")
        if args.from_id:
            event["anchor"] = f"id:{args.from_id}"
        request = {"cmd": "event", "event": event}
    reply = control_request(args.control, request)
    if args.action == "stats" and reply.get("ok") and not args.json:
        print(f"t={reply['t']}s")
        for row in reply["stats"]:
            parts = [f"{d} " + " ".join(f"{k}={v:g}" for k, v in sorted(row[d].items())) for d in (UP, DOWN)]
            print(f"{row['port']}#{row['peer']} {row['addr']}{' (closed)' if row['closed'] else ''}: " + " | ".join(parts))
    else:
        print(json.dumps(reply))
    return 0 if reply.get("ok") else 1


def cmd_profiles(args) -> int:
    for path in sorted(PROFILES.glob("*.json")):
        profile = Profile.load(str(path))
        print(f"{profile.name:28} {profile.description}")
    return 0


# --------------------------------------------------------------------------
# `local`: srp + the simulator + bots + headless game clients.

def load_run_py(path: Path):
    """The local-clients launcher as a module, to reuse its config reader,
    prefix preparation and stop (it lives on tools/local-game-clients)."""
    if not path.is_file():
        raise ValueError(f"{path} is missing: it lives on the branch tools/local-game-clients "
                         "(the main checkout has a copy); give --run-py")
    spec = importlib.util.spec_from_file_location("local_clients_run", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


XRANDR_SECTION = b"[Software\\\\Wine\\\\X11 Driver]"
XRANDR_VALUES = [b'"UseXRandR"="N"', b'"UseXVidMode"="N"']


def disable_xrandr(user_reg: Path) -> bool:
    """DXVK on Xvfb needs HKCU\\Software\\Wine\\X11 Driver UseXRandR=N (and
    UseXVidMode=N). Edits an inactive prefix's user.reg; True when changed."""
    text = user_reg.read_bytes()
    lines = text.split(b"\n")
    start = next((i for i, line in enumerate(lines) if line.startswith(XRANDR_SECTION)), None)
    if start is None:
        tail = b"" if text.endswith(b"\n\n") else (b"\n" if text.endswith(b"\n") else b"\n\n")
        user_reg.write_bytes(text + tail + XRANDR_SECTION + b" %d\n" % int(time.time()) + b"\n".join(XRANDR_VALUES) + b"\n")
        return True
    end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith(b"[")), len(lines))
    body = lines[start + 1:end]
    if all(value in body for value in XRANDR_VALUES):
        return False
    body = [line for line in body if not line.startswith((b'"UseXRandR"=', b'"UseXVidMode"='))]
    insert = 1 if body and body[0].startswith(b"#time=") else 0
    body[insert:insert] = XRANDR_VALUES
    user_reg.write_bytes(b"\n".join(lines[:start + 1] + body + lines[end:]))
    return True


def port_free(host: str, port: int, kind: str) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM if kind == "udp" else socket.SOCK_STREAM) as sock:
        try:
            sock.bind((host, port))
            return True
        except OSError:
            return False


def spawn(command: list[str], cwd: Path, env: dict, log: Path) -> subprocess.Popen:
    log.parent.mkdir(parents=True, exist_ok=True)
    with log.open("ab") as stream:
        return subprocess.Popen(command, cwd=cwd, env=env, stdout=stream, stderr=subprocess.STDOUT,
                                stdin=subprocess.DEVNULL, start_new_session=True)


def stop_process(process: subprocess.Popen | None, name: str, sim: Simulator, sig=signal.SIGINT) -> None:
    if process is None or process.poll() is not None:
        return
    try:
        os.killpg(process.pid, sig)
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
    except ProcessLookupError:
        pass
    sim.say(f"stopped {name}")


def x_display_number(display: str) -> int:
    if not display.startswith(":") or not display[1:].isdecimal():
        raise ValueError(f"a display is :N, not {display!r}")
    return int(display[1:])


def cmd_local(args) -> int:
    config_path = args.config.resolve()
    data = json.loads(config_path.read_text())
    ports = data.get("ports", [1234, 1235, 1236, 8088])
    server = data.get("server")
    if not server:
        raise ValueError("the config has no server: packet-sim starts srp itself")
    resolve = lambda value: (config_path.parent / value).resolve()
    binary, workdir = resolve(server["binary"]), resolve(server["working_dir"])
    runtime = config_path.parent / "runtime"
    work = runtime / "packet-sim"
    work.mkdir(parents=True, exist_ok=True)
    if args.log is None:
        args.log = work / "sim.jsonl"
    if args.control is None:
        args.control = f"127.0.0.1:{ports[2] + 10}"

    displays: list[str] = []
    run_py = None
    if args.clients:
        if not args.display:
            raise ValueError("--clients needs --display :N (a headless X display, never the desktop's)")
        first = x_display_number(args.display)
        displays = [f":{first + i}" for i in range(args.clients)] if args.display_per_client else [args.display] * args.clients
        for display in set(displays):
            if display in (":0", os.environ.get("DISPLAY")):
                raise ValueError(f"{display} is this session's own display; the game runs on a headless one only")
            socket_path = Path(f"/tmp/.X11-unix/X{display[1:]}")
            if args.xvfb and socket_path.exists():
                raise ValueError(f"{display} is taken ({socket_path}); pick another --display")
            if not args.xvfb and not socket_path.exists():
                raise ValueError(f"no X server on {display}: start Xvfb there or pass --xvfb")
        run_py = load_run_py(args.run_py)

    for (name, kind), port in zip(SERVICES, ports):
        for host in (LISTEN_HOST, TARGET_HOST):
            if not port_free(host, port, kind):
                raise ValueError(f"{kind} {host}:{port} ({name}) is taken: another server runs; change `ports` in the copy")

    sim = build_simulator(args)
    for kind, name, listen, target in srp_forwards(ports, LISTEN_HOST, TARGET_HOST):
        sim.add(kind, name, listen, target)
        sim.say(f"{kind} {name}: {listen[0]}:{listen[1]} -> {target[0]}:{target[1]}")

    env = dict(os.environ, **server.get("env", {}))
    bots = [f"{args.bot_name}{i + 1}" for i in range(args.bots)]
    if bots:
        named = [n for n in env.get("SRP_BOT_ACCOUNTS", "").split(",") if n]
        env["SRP_BOT_ACCOUNTS"] = ",".join(dict.fromkeys(named + bots))
    # run.py's fixed settings, except: srp listens on 127.0.0.2 and tells
    # every client 127.0.0.1 (the simulator); local peers off, or srp would
    # tell a loopback client the address the simulator reached it at.
    env.update(SRP_DATA_DIR=str(config_path.parent / "data"), SRP_DB="on", SRP_REGISTRATION="open",
               SRP_REGISTRATIONS_PER_DAY="100", SRP_TUTORIAL="on", SRP_LOG_FILES="off",
               SRP_BIND_HOST=TARGET_HOST, SRP_PUBLIC_HOST=LISTEN_HOST, SRP_WEB_BIND=TARGET_HOST,
               SRP_LOCAL_PEERS="off", SRP_PACKET_LOG=str(runtime / "packets"),
               SRP_SNAPSHOT_DIR=str(runtime / "snapshots"))
    for (name, _), port in zip(SERVICES, ports):
        env[f"SRP_{name.upper()}_PORT"] = str(port)
        env[f"SRP_{name.upper()}_BIND"] = TARGET_HOST
        env[f"SRP_{name.upper()}_PUBLIC_HOST"] = LISTEN_HOST

    if not 0 <= args.late_bots <= args.bots:
        raise ValueError("--late-bots counts some of the --bots")
    srp = None
    bot_processes: list[tuple[str, subprocess.Popen]] = []
    timers: list[threading.Timer] = []
    xservers: list[subprocess.Popen] = []
    run_config = None
    try:
        srp = spawn([str(binary)], workdir, env, work / "server.log")
        sim.say(f"srp {binary} (pid {srp.pid}) on {TARGET_HOST}, log {work / 'server.log'}")
        deadline = time.monotonic() + 30
        while True:
            if srp.poll() is not None:
                raise RuntimeError(f"srp exited; see {work / 'server.log'}")
            try:
                with socket.create_connection((TARGET_HOST, ports[0]), timeout=.2):
                    break
            except OSError:
                if time.monotonic() > deadline:
                    raise RuntimeError(f"srp did not open its login port; see {work / 'server.log'}")
                time.sleep(.2)
        if bots:
            bot_bin = args.bot_bin or binary.with_name("srp-bot")
            revision = args.bot_revision or server.get("env", {}).get("SRP_MIN_PATCH_REVISION", "18")
            bot_env = dict(os.environ, **server.get("env", {}))

            def start_bot(name: str) -> None:
                # One process a bot: bots of one process never queue
                # together (srp::bot), separate ones make a match.
                with sim.lock:  # not after the teardown took the list
                    if sim.stop_flag.is_set():
                        return
                    process = spawn([str(bot_bin), "--server", f"{LISTEN_HOST}:{ports[0]}", "--name", name,
                                     "--revision", str(revision), *args.bot_arg], workdir, bot_env,
                                    work / f"{name}.log")
                    bot_processes.append((name, process))
                sim.say(f"srp-bot {name} (revision {revision}, pid {process.pid}) through the simulator, "
                        f"log {work / (name + '.log')}")

            for name in bots[:args.bots - args.late_bots]:
                start_bot(name)
            for name in bots[args.bots - args.late_bots:]:
                timer = threading.Timer(args.late_after, start_bot, (name,))
                timer.daemon = True
                timer.start()
                timers.append(timer)
            if args.late_bots:
                sim.say(f"late joiner(s) {', '.join(bots[args.bots - args.late_bots:])} at {args.late_after:g}s")
        if args.clients:
            xvfb = args.xvfb_bin or shutil.which("Xvfb")
            if args.xvfb:
                if not xvfb:
                    raise ValueError("no Xvfb on PATH: give --xvfb-bin (nix: nixpkgs#xvfb)")
                for display in dict.fromkeys(displays):
                    xservers.append(spawn([xvfb, display, "-screen", "0", f"{data.get('resolution', '1280x720')}x24",
                                           "-nolisten", "tcp"], work, dict(os.environ), work / f"xvfb{display[1:]}.log"))
                    sim.say(f"Xvfb {display} (pid {xservers[-1].pid})")
                time.sleep(1)
            run_config = run_py.Config.read(config_path)
            if args.clients > len(run_config.accounts):
                raise ValueError(f"the config has {len(run_config.accounts)} accounts")
            with run_py.locked(run_config):
                state = run_py.read_state(run_config)
                if state["clients"] or state.get("server"):
                    raise ValueError("run.py records a run for this config: `run.py --config ... stop` first")
                for index in range(args.clients):
                    run_py.prepare_prefix(run_config, index)
                    disable_xrandr(run_config.prefix(index) / "user.reg")
                for index in range(args.clients):
                    if index and args.client_delay:
                        time.sleep(args.client_delay)
                    client_env = run_config.client_environment(index)
                    for key in ("WAYLAND_DISPLAY", "XDG_SESSION_TYPE"):
                        client_env.pop(key, None)
                    client_env["DISPLAY"] = displays[index]
                    state["clients"][str(index)] = {}
                    run_py.write_json(run_py.state_path(run_config), state)
                    state["clients"][str(index)] = run_py.launch(
                        run_config.client_command(index), run_config.game / "binaries/x86", client_env,
                        run_config.runtime / f"client-{index + 1}.log")
                    run_py.write_json(run_py.state_path(run_config), state)
                    sim.say(f"client {index + 1}: {run_config.accounts[index].name} on {displays[index]}")
        sim.say(f"ready: the game's -server=127.0.0.1:{ports[0]}; Ctrl-C stops everything this started")
        run_simulator(sim, args)
    finally:
        if run_config is not None:
            try:
                with run_py.locked(run_config):
                    run_py.stop(run_config, run_py.read_state(run_config))
                sim.say("stopped the game clients")
            except Exception as error:  # noqa: BLE001 - still stop the rest
                print(f"could not stop the clients ({error}); run run.py --config {config_path} stop", file=sys.stderr)
        with sim.lock:
            sim.stop()
        for timer in timers:
            timer.cancel()
        for name, process in list(bot_processes):
            stop_process(process, f"srp-bot {name}", sim)
        stop_process(srp, "srp", sim)
        for xserver in xservers:
            stop_process(xserver, "Xvfb", sim, signal.SIGTERM)
        sim.shutdown()
    return 0


# --------------------------------------------------------------------------
# Headless display helpers.

def cmd_click(args) -> int:
    """The game reads the mouse as DirectInput deltas: absolute moves do not
    reach it. Home the pointer with large relative moves, then move
    relatively to X,Y in small steps and click."""
    xdotool = args.xdotool or shutil.which("xdotool")
    if not xdotool:
        raise ValueError("no xdotool on PATH: give --xdotool (nix: nixpkgs#xdotool)")
    if args.display == os.environ.get("DISPLAY"):
        raise ValueError("refusing to drive this session's own display")
    env = dict(os.environ, DISPLAY=args.display)
    run = lambda *words: subprocess.run([xdotool, *words], env=env, check=True)
    run("mousemove", "640", "360")
    for _ in range(6):
        run("mousemove_relative", "--", "-600", "-400")
        time.sleep(0.05)
    time.sleep(0.2)
    x, y = args.x, args.y
    while x > 0 or y > 0:
        dx, dy = min(x, 200), min(y, 200)
        run("mousemove_relative", "--", str(dx), str(dy))
        x, y = x - dx, y - dy
        time.sleep(0.05)
    time.sleep(0.3)
    run("mousedown", "1")
    time.sleep(0.2)
    run("mouseup", "1")
    return 0


def cmd_shot(args) -> int:
    xwd, magick = args.xwd or shutil.which("xwd"), shutil.which("magick")
    if not xwd or not magick:
        raise ValueError("needs xwd (nixpkgs#xorg.xwd) and ImageMagick's magick on PATH")
    grab = subprocess.run([xwd, "-root", "-silent", "-display", args.display], check=True, capture_output=True)
    # The PNG comes back on stdout and is written here: a magick that runs
    # in a sandbox of its own (this system's) cannot open a path under /tmp.
    png = subprocess.run([magick, "xwd:-", "png:-"], input=grab.stdout, check=True, capture_output=True)
    args.file.write_bytes(png.stdout)
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)

    run = sub.add_parser("run", help="forward and impair (srp started elsewhere)")
    add_sim_arguments(run)
    fwd = run.add_argument_group("forwarding")
    fwd.add_argument("--srp-ports", metavar="LOGIN,LOBBY,MATCH,WEB",
                     help="forward srp's four ports: listen on --listen-host, forward to the same port on --target-host")
    fwd.add_argument("--listen-host", default=LISTEN_HOST)
    fwd.add_argument("--target-host", default=TARGET_HOST)
    fwd.add_argument("--udp", action="append", default=[], metavar="NAME=[HOST:]PORT/[HOST:]PORT")
    fwd.add_argument("--tcp", action="append", default=[], metavar="NAME=[HOST:]PORT/[HOST:]PORT")
    run.set_defaults(func=cmd_run)

    local = sub.add_parser("local", help="srp + the simulator (+ srp-bot, + headless game clients) from a "
                                         "local-clients config")
    local.add_argument("--config", type=Path, required=True,
                       help="a run.py config (a copy of private/local-clients/config.json, own ports)")
    add_sim_arguments(local)
    who = local.add_argument_group("players")
    who.add_argument("--bots", type=int, default=0, metavar="N",
                     help="srp-bot players through the simulator, one process each (so they make a match)")
    who.add_argument("--late-bots", type=int, default=0, metavar="M",
                     help="of the --bots, the last M start --late-after seconds later: late joiners")
    who.add_argument("--late-after", type=float, default=40.0, metavar="S")
    who.add_argument("--bot-name", default="simbot-", help="bots are named PREFIX1..PREFIXN (default simbot-)")
    who.add_argument("--bot-revision", help="srp-bot --revision (default: SRP_MIN_PATCH_REVISION or 18)")
    who.add_argument("--bot-bin", type=Path, help="default: srp-bot beside the server binary")
    who.add_argument("--bot-arg", action="append", default=[], help="an extra srp-bot argument (repeatable)")
    who.add_argument("--clients", type=int, default=0, metavar="N",
                     help="game windows (headless only: needs --display; ask the owner first)")
    who.add_argument("--display", help="the headless X display, :N")
    who.add_argument("--display-per-client", action="store_true",
                     help="client i on display :N+i (windows do not cover one another)")
    who.add_argument("--xvfb", action="store_true", help="start Xvfb on the display(s), stop it at the end")
    who.add_argument("--xvfb-bin")
    who.add_argument("--client-delay", type=float, default=0.0, metavar="S", help="seconds between client starts")
    who.add_argument("--run-py", type=Path, default=HERE.parent / "local-clients/run.py")
    local.set_defaults(func=cmd_local)

    ctl = sub.add_parser("ctl", help="talk to a running simulator's --control port")
    ctl.add_argument("--control", default=os.environ.get("PACKET_SIM_CONTROL"), metavar="[HOST:]PORT")
    ctl.add_argument("--json", action="store_true", help="print stats as JSON")
    ctl.add_argument("action", choices=("stats", "event", "set", "clear", "drop", "hold"))
    ctl.add_argument("words", nargs="*", help="event/set: a spec; drop/hold: PORT SECONDS")
    ctl.add_argument("--peer", default="*", help="drop/hold: which clients (default all)")
    ctl.add_argument("--dir", default="both", choices=("up", "down", "both"))
    ctl.add_argument("--after", type=float, default=0.0, metavar="S",
                     help="drop/hold: start S seconds from now (from the join with --on-join)")
    ctl.add_argument("--ids", metavar="E4,E8", help="drop/hold: only match datagrams with one of these message ids")
    ctl.add_argument("--count", type=int, default=0, metavar="N",
                     help="drop/hold: at most N datagrams a client and direction (0: all in the window)")
    ctl.add_argument("--from-id", metavar="XX",
                     help="drop/hold: count --after from the client's first datagram with message XX (e.g. DA)")
    ctl.add_argument("--on-join", action="store_true",
                     help="drop/hold: arm it for the next client that starts on PORT (a late joiner)")
    ctl.set_defaults(func=cmd_ctl)

    profiles = sub.add_parser("profiles", help="list the shipped profiles")
    profiles.set_defaults(func=cmd_profiles)

    click = sub.add_parser("click", help="click a headless game window at X,Y (DirectInput-safe)")
    click.add_argument("--display", required=True)
    click.add_argument("--xdotool")
    click.add_argument("x", type=int)
    click.add_argument("y", type=int)
    click.set_defaults(func=cmd_click)

    shot = sub.add_parser("shot", help="a PNG of a headless display")
    shot.add_argument("--display", required=True)
    shot.add_argument("--xwd")
    shot.add_argument("file", type=Path)
    shot.set_defaults(func=cmd_shot)

    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except (OSError, ValueError, RuntimeError, KeyError, subprocess.SubprocessError) as error:
        print(f"packet-sim: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
