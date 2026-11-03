# Measure round trips from transmission

E5 and E6 stamps are chosen when a message is queued. Measuring elapsed
time from that stamp counts a transport queue stall as network latency.
The inflated RTT then changes clock-lag estimates and resync assignments.

A `Transmission` receipt belongs to the transport and records the first
emission from `Transport::poll`. It has no timestamp while queued and keeps
the first timestamp across retransmissions. Both ordinary and joined sends
can return a receipt; the joined E7-stop/E5 pair preserves its wire order.
The match server uses receipts for E5 and E6 replies. E6 subtracts the
client's encoded hold time; E5 echoes its stamp without an encoded hold.
Queue timestamps remain available for conservative clock projections.
Packet bytes and reliable-delivery behavior are unchanged.

E6 has no exact reply identifier: its first field repeats the previous RTT,
and its second includes the client's hold. Older probes are retained for
the full accepted answer window (up to 5 seconds each for hold and RTT).
If a reply fits probes with different transmission-based RTTs, it supplies
no clock sample. Equal interpretations remain usable. Answered probes and
probes retired by a resync remain ambiguity guards. Only one unsent probe
is queued, so a stalled transport cannot grow this history without bound.

The recorded ping regression has 486 ms of queue delay, 175 ms from emission
to reply, and 39 ms of client hold. The old calculation gives 622 ms;
the corrected RTT is 136 ms. Running both through the real lag sampler and
clock selector reproduces the old assignment 205768 and corrected 205343.
The fixture also rejects unsent, mismatched and impossible replies and
checks wrapping 16-bit stamps.

A review regression sends probes queued at 0 and 1000 ms, emitted at 0 and
1500 ms. The first reply arrives at 1600 ms after 1000 ms of client hold:
its true RTT is 600 ms. Using only the latest probe incorrectly reports
100 ms. The regression fails that implementation and passes with ambiguity
checking; additional cases cover resync, answered probes, equal RTTs,
expiry and a queue stalled longer than the answer window.

Set-time regressions cover 121/622 ms and 1950/1025 ms queue/wire intervals,
for standalone E5 and joined E7/E5. Transport tests force an actual send-window
stall and check emission timing, multipart messages, retransmission,
cancellation, failed joined sends and receipt/transport teardown.

The separate clock-retry fix is not included: this branch retains the
existing assignment policy so the timing repair can be reviewed and tested
independently. Historical packet evidence supports inflated RTT uptake in
additional matches; it does not prove those measurements caused every resync.
No private packet or snapshot is a repository fixture.
