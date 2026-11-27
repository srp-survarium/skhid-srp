# udp-transport (0.69d)

Reliable-UDP transport of the 0.69d lobby connection, server side (and a client role for scripted tests).
Sans IO: `Transport::receive(datagram, now)` -> events, `send(id, payload)`, `poll(now)` -> datagrams.

The structure (`seq`, `ack`, `table`, `wire`, `transport`, same `receive`/`send`/`poll` shape, `Event`,
`Config`) follows the 0.26g `udp-transport` crate; the code is written for 0.69d, because the wire format
differs (9-bit seqs, other ack encoding, other multipart format, per-id classes). `Transport` behaves like the
earlier reference implementation that worked with the real client (resend after 1 s with a new seq, entry kept
until any copy is acked, 440-byte parts). The 0.26g-only parts (upload carriers, match tables, packet log, pacing
guard) are not part of this crate.

## Proven facts used (read in the client code; addresses in the module docs)
- Header: byte0 = seq low 8 bits, byte1 = R low 8 bits, byte2 = ack0; tail bytes at the END of the datagram,
  read from the last byte backwards until one has bit 7 clear (final byte). Non-final byte = 7 ack bits, final
  byte = 4 ack bits (bits 6..3) + bit 2 = R bit 8, bit 1 = seq bit 8, bit 0 = F (1 multi, 0 single).
  Stream bit m acks seq `(R-1-m) mod 512`, R is always acked. At most 35 tail bytes. `0x1FF` = "nothing yet".
- Seqs are 9 bit (`& 0x1FF`), cyclic comparison with half window 256; a resent datagram needs a new seq.
- One message: type byte (`FF` = multipart prefix, then the id), `FA..FD` low-level (one byte; `FC` keepalive),
  `FE` and anything outside the table range is invalid (the client drops the whole datagram, no ack).
  `id [x] [part [count]] data`; single datagram: data = rest; multi datagram: `u8 len` or `FF u16 len` first.
  x only for unique/ordered ids; with `FF`: `part`, and `count` only in part 0.
- Classes: C->S `2E..35` unique, `36` plain, `37..3B` unique, `3C..44` plain, `45..60` unique, `61` plain, `62`
  unique. S->C table in `src/table.rs` (unique: 80 81 82 9A 9E AD AE AF B1 B7 B9..BF C1 C2; plain: A2 A3 AC B4
  C0 C3; the rest of 83..C3 ordered). Unique and ordered ids have separate counters, plain have none.
- Tested on recorded lobby sessions (`tests/golden/*_lobby.jsonl`): all ~22 000 datagrams of both directions
  decode and re-encode byte-identically; the client messages equal the recording server's decode; every server
  push has the class the table gives; every multipart message re-splits into the same parts as were sent; one
  session's client side replays through a server `Transport` with nothing rejected.

## Not proven / open
- Disconnect handling (`FA` answered by `FB`, `FB`/`FD` end the peer) is carried over from 0.26g by analogy.
- Multi-format datagrams are decoded and encoded, but `Transport::poll` sends one message per datagram.
- Only the lobby table is here; the match connection (other table, ids `DD..E0`) is in `match-server`.
- Resend pacing is a fixed 1 s, no fast resend and no send window.
