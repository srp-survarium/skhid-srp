# Preserve assigned clocks across resync retries

A resync initially puts the client ahead of its measured clock by a 100 ms
safety margin. Previously only an answered E5 assignment established the
separate `clock_given` baseline. After a timeout, sending the world updated
the lag estimate to include the assigned lead. Retrying without an answer
then added the margin again. Repeated retries could walk the clock forward
by another 100 ms each time.

`AssignedClock` owns the baseline as `Unknown` or `Bound`. Every successfully
queued E5 records the maximum projected assignment. Retries and delayed
answers do not discard or lower it. A new E7 clock correction invalidates
it. Failed sends cannot manufacture an assignment. The existing same-datagram
E7-stop/E5 ordering is preserved.

Queue time intentionally bounds delivery time from below. If an assignment
S queued at Q reaches the client at R >= Q, its uncorrected clock at N is
S + N - R, at most S + N - Q. Keeping the maximum projected assignment also
handles reordered copies. Clock selection still respects the newest own
input plus its margin and the measured client clock.

The production `begin_timing` -> `send_sync` -> `redo_sync` regression
produced leads 347, 447, 547, 647 and 747 ms before the repair. With the
repair all five remain 347 ms. A second test covers newer own inputs,
delayed answers to lower assignments, clock-correction invalidation and
sending after disconnection. These tests use constructed server states;
they prove the retry mechanism, not a complete historical match replay.

Review regressions also pass the three queued E5 payloads through the client
clock/generation model in all six arrival orders, with delivery delays and no
replies. The projected assignment stays above each possible client clock and
the next retry adds no extra margin. Failed sends are checked with an existing
bound and a full reply history, both for an ordinary E5 and one joined with
E7-stop: neither the bound nor a reply-history entry changes.

This server-only change does not alter packet formats or the client binary.
The independent transmission-timing fix addresses queue-inflated RTT; it
is not required for this retry regression to pass.
