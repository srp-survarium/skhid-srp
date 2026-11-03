# Measure round trips from transmission

E5 and E6 used queue timestamps to measure round trips, counting transport
stalls as network latency. That inflated clock-lag estimates and resync
assignments. Their replies require different repairs.

E5 echoes its stamp unchanged. A transport-owned `Transmission` receipt
records the first emission from `Transport::poll`, including the joined
E7-stop/E5 datagram. Its round trip starts there. The queue timestamp is
retained separately for conservative clock projections.

E6 has no exact reply identifier: `b` is the previous RTT, also displayed
by the client, while the client adds its hold time to `a`. Matching a reply
to the latest queue-stamped probe can borrow the wrong transmission time.
Keeping earlier candidates and rejecting different interpretations also
fails: ordinary queue jitter makes all later replies ambiguous, starving
clock samples indefinitely. Neither strategy is used.

`Transport::send_match_ping` owns a typed pending E6. It writes `a` from a
`PingClock` only when the datagram is emitted, after pacing and channel-window
checks. Its clock origin is the match's monotonic origin. The client adds
its hold, so `now16 - reply.stamp` directly measures the wire RTT, independent
of which probe produced it. `b` and the wire layout retain their meanings.
The latest receipt rejects unsent replies; the existing echo/age checks
reject clearly unrelated replies. A compatible older reply measures its
own RTT. Pending E6s coalesce, including when a resync discards the caller's
receipt, so a stalled connection accumulates at most one unsent ping.
Beginning resync timing cancels that pending ping before queuing E5: a
stale E6 must not acquire the new E5's echo at a shared flush. Already
emitted packets cannot be cancelled; their overlap was possible before.

The wire-driven server regression alternates queue delays of 0 and 1, 10,
50 or 486 ms on a steady 100 ms connection. It reads each emitted E6 and
sends the client's 16 ms hold through `Game::receive`. All 80 replies per
case must refresh the lag sample, crossing a full 16-bit stamp lap. The
previous history guard accepted only the first two of thirty replies in
the independent review's 1/10/50 ms cases.

An older reply with a true 600 ms RTT cannot become a 100 ms sample from a
newer probe: a clearly older echo is ignored, and a compatible older echo
still gives 600 ms. Further cases cover unsent, future and mismatched replies,
stamp wrap, a real channel-window stall, coalescing, cancellation and lobby
rejection. E6 remains unique and sent once.
The submillisecond case uses the wire clock's rounding, rather than
rejecting a client hold that rounds one millisecond above local elapsed time.

The recorded 486 ms queue / 175 ms wire / 39 ms hold fixture gives 136 ms
RTT instead of 622 ms. The real lag sampler and resync selector give 205343
instead of 205768. E5 cases cover 121/622 ms and 1950/1025 ms queue/wire
intervals, standalone and joined with E7. Transport tests also cover
multipart receipts, retransmission, failed joined sends and teardown.

The clock-retry policy is unchanged on this branch. These regressions prove
the timing mechanisms, not the cause of every historical resync. No private
packet or snapshot is included as a repository fixture.
