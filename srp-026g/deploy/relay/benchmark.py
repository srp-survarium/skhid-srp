#!/usr/bin/env python3
"""Measure native forwarding RTT against a local echo backend on the relay host.

This excludes Internet routing and WireGuard: it isolates userspace forwarding.
Run with SKHID_NATIVE_RELAY=/opt/skhid-relay/run. The echo backend is the same
Python thread for both measurements, and all test sockets bind loopback only.
"""
import json
import os
import socket
import statistics
import time

import forwarder as wire
from test_native import NativeTests


def sample(address, payload, count=1000):
    timings = []
    lost = 0
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as client:
        client.bind(('127.0.0.1', 0))
        client.settimeout(0.25)
        for _ in range(count):
            start = time.perf_counter_ns()
            client.sendto(payload, address)
            try:
                data, _ = client.recvfrom(8192)
                if data != payload:
                    lost += 1
                    continue
                timings.append((time.perf_counter_ns()-start)/1_000_000)
            except TimeoutError:
                lost += 1
    values = sorted(timings)
    def percentile(fraction):
        return values[min(len(values)-1, int(len(values)*fraction))] if values else None
    return dict(sent=count, lost=lost, median_ms=statistics.median(values) if values else None,
                p95_ms=percentile(.95), p99_ms=percentile(.99), max_ms=max(values, default=None))


def main():
    if not os.environ.get('SKHID_NATIVE_RELAY'):
        raise SystemExit('set SKHID_NATIVE_RELAY to the installed native executable/runner')
    harness = NativeTests()
    harness.setUp()
    try:
        payload = bytes(range(256))*2
        # Baseline goes straight to the very same backend, with the forwarding
        # header the backend sees during the relay measurement.
        baseline = sample(('127.0.0.1', harness.port+2),
                          wire.header(('127.0.0.1', 40000), '203.0.113.8', harness.port+2)+payload)
        relayed = sample(('127.0.0.2', harness.port+2), payload)
        print(json.dumps(dict(payload_bytes=len(payload), direct_backend=baseline,
            native_relay=relayed, median_added_ms=relayed['median_ms']-baseline['median_ms']), indent=2))
    finally:
        harness.tearDown()


if __name__ == '__main__':
    main()
