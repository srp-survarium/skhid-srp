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

The recorded ping regression has 486 ms of queue delay, 175 ms from emission
to reply, and 39 ms of client hold. The old calculation gives 622 ms;
the corrected RTT is 136 ms. Running both through the real lag sampler and
clock selector reproduces the old assignment 205768 and corrected 205343.
The fixture also rejects unsent, mismatched and impossible replies and
checks wrapping 16-bit stamps.

Set-time regressions cover 121/622 ms and 1950/1025 ms queue/wire intervals,
for standalone E5 and joined E7/E5. Transport tests force an actual send-window
stall and check emission timing, multipart messages, retransmission,
cancellation, failed joined sends and receipt/transport teardown.

The separate clock-retry fix is not included: this branch retains the
existing assignment policy so the timing repair can be reviewed and tested
independently. Historical packet evidence supports inflated RTT uptake in
additional matches; it does not prove those measurements caused every resync.
No private packet or snapshot is a repository fixture.
