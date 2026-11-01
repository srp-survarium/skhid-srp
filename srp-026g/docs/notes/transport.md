# 0.26g match UDP transport (vs 0.21d)

**Version scope:** client **0.26g (build 2777, Jan 2015, no PDB)** compared against **0.21d (build 2010, Apr 2014, PDB symbols)**. Baseline: `srp-021d/docs/notes/transport_spec.md` (cited below as "021d spec §n") and `srp-021d/servers/match-server/src/transport.rs`.

- **PROVEN** = read in the 0.26g disassembly at the VA given, and matched to the 0.21d PDB symbol/VA where one exists.
- **LIKELY** = a strong inference from the code around it.
- **GUESS** = plausible, not checked.
- 0.26g functions have no symbols. They are named by the `__FUNCSIG__`/assert strings they reference, or by matching their code to the 0.21d function.
- Image base 0x10000 in both builds. `conn` = `udp_match_connection` (0.26g: `udp_match_client + 8`). `pkt` = `udp_match_packet`.

## 0. Function map (0.21d PDB → 0.26g)

| role | 0.21d (PDB) | 0.26g | how identified |
|---|---|---|---|
| `udp_match_client::handle_receive` | 0x2868A5 | 0x3F2050 | FUNCSIG string a2ace8 |
| `udp_match_client::connect_impl` | 0x286B1F | 0x3F2610 | FUNCSIG a2adc0 |
| `udp_match_client::start_receiving` | 0x286AA9 | 0x3F2590 | same shape, buffer 0x1FC |
| `udp_match_client::set_connection_ports` | 0x287002 | 0x3F2B50 | writes the filter/random ranges, loops ports |
| random port pick (`send_packet` part) | 0x2874B4 | 0x3F3010 | uses `client+0x131C/0x1320` |
| `process_incoming_packet` | 0x28797D | 0x3F3BB0 | called from handle_receive; parse/ack/dispatch |
| header parse (inlined in 0.21d) | 0x2879BB.. | 0x3FA0A0 | — |
| `try_parse_single_message` (new) | — | 0x3FA240 | FUNCSIG a2c300 |
| parse loop | — | 0x3FAF40 | — |
| `call_predicate` (per message) | 0x288154 | 0x3F4720 | — |
| `process_multipacket_message` | 0x288717 | 0x3F4DA0 (`process_multipacket_unique_message`), 0x3F4A50 (`..._reliable_unique_ordered_message`) | FUNCSIG a2b1a8 / a2aff8 |
| `update_acknowledgements` | 0x28D884 | 0x3FD7E0 (`update_local_acknowledgements`) | FUNCSIG a2d358 |
| `acknowledge_packets` | 0x28D637 | 0x3FD690 | walks the ack bits, deletes unacked |
| `fill_packet_header` | 0x28B887 | 0x3FB390 | — |
| ack-field encoder (new) | — | 0x3FF580 | — |
| `send_packets_list` | 0x28BC48 | 0x3FB4F0 | assert string a2cdc4 |
| `send_queued_packets` | 0x28C0BF | 0x3FBF70 | "disconnected by timeout" a2ce84 |
| `new_low_level_packet` | 0x28BFA6 | 0x3FBDB0 | `id = type - 6` |
| `process_low_level_message` | 0x28DD36 | 0x3FDCF0 | FUNCSIG a2d4e0 |
| `connect` | 0x28CE00 | 0x3FCC40 | FUNCSIG a2d1b8 |
| `enqueue_impl` | 0x28CF07 | 0x3FCE30 | — |
| `enqueue(id, reader, size)` (split) | 0x28D0B9 | 0x3FD060 | — |
| `construct_multipacket_part` | 0x28CFC1 | 0x3FCF60 | — |
| `instant_disconnect` | 0x28E2FF | 0x3FDF30 | — |
| `disconnect` | 0x28E563 | 0x3FE000 | FUNCSIG a2d608 |
| `construct_packet` (id + order placeholder) | 0x238AAF | 0x378080 | — |
| `get_sending_message_info` | 0x1A144C (table 0x1A1494) | 0x279A60 (table 0x279AE0) | vtable 0xA08414 [0] |
| `get_received_message_info` | 0x1A14B4 (table 0x1A14F8) | 0x279B10 (table 0x279BF0) | vtable 0xA08414 [1] |
| `get_message_length` (new) | — | 0x279670 (table 0x2799E0) | vtable 0xA08414 [2] |
| `match_client::connect` (first message) | 0x1A1354 | 0x27A1B0 | — |
| match client creation (timeouts) | `udp_match_client` ctor 0x2863A8 | 0x27A0F6–0x27A147 | — |

## 1. Comparison table: what srp-021d `transport.rs` must change

| # | item | 0.21d | 0.26g | status | transport.rs change |
|---|---|---|---|---|---|
| 1 | header | 12 B: `u16 seq, u16 rseq, u64 ack_multi` | 3 B: `u8 seq, u8 rseq, u8 ack0`, plus 0–17 ack bytes **at the end** of the datagram | **changed** (PROVEN) | Rewrite `datagram()` and the header parse in `receive()`. |
| 2 | sequence width | u16, half range 0x8000 | **u8, half range 128** | **changed** (PROVEN) | `SN16` → an 8-bit type; `is_newer` mod 256. |
| 3 | ack window | 64 bits; wire bit 63 = rseq−1 | **128 bits**, carried as a 7-bit varint; only `7n+7` bits are sent (n = tail bytes) | **changed** (PROVEN) | New encoder and decoder (§2). |
| 4 | single/multi flag | bit 0 of byte 4 | **bit 0 of the terminating ack byte** (ack0 if n=0, else the last byte of the datagram) | **changed** (PROVEN) | — |
| 5 | header-only datagram | crashes the client (multi=0) / ignored | **parse fails; the whole datagram is dropped** (no acks processed) | **changed** (PROVEN) | Never send one; keepalive = low-level 0xFC. |
| 6 | datagram whose rseq is newer than the client's last sent seq | acks ignored, messages processed | **whole datagram dropped** (0x3F1C40) | **changed** (PROVEN) | Only ever echo a received client seq. |
| 7 | message ids | S→C 0x50..0x59, C→S 0x40..0x47 | S→C **0xE3..0xF6**, C→S **0xD8..0xE1** | **changed** (PROVEN) | New id tables. |
| 8 | message header | `id [u16 order]` | `id [u8 order | u8 unique]` | **changed** (PROVEN) | 1-byte order/unique id. |
| 9 | ordering | ordered ids (0x51..) on channel 0 | match: **no id is ordered**; 6 S→C ids are **unique** (transport dedup), all the rest plain. **Lobby: every id is unique, and most S→C ids are also ordered (§4.1)** | **changed** (PROVEN) | Match: a u8 unique counter for E4–E8, F6. Lobby: a unique counter and an ordered channel with its own u8 counter. |
| 10 | length in a multi datagram | every message: `u8 len` or `00 u16`, counting id+order+payload | **only variable ids** (E4, E8, F6) get a prefix: `u8 len` (≤254) or `FF u16` (≥255), counting **payload only**. Fixed ids have **no prefix** | **changed** (PROVEN) | New `prefixed_len`; a fixed-size table per id (§4). |
| 11 | single-datagram message | rest of the datagram | rest of the datagram (minus tail ack bytes); a fixed-length id must match **exactly** or the datagram is dropped | **changed** (PROVEN) | Send exact payload sizes. |
| 12 | low-level messages | multi datagram `01 tt` | **one id byte 0xFA..0xFD**, last (or only) message, no payload | **changed** (PROVEN) | `FA` initiate, `FB` confirm, `FC` continuous flow, **`FD` new: instant disconnect**. |
| 13 | multipart | `FF id u16order part [count]`, ordered channel only | `FF id u8x part [count]`, x = the order id of an ordered id, else the unique id; data 500/501 B minus tail ack bytes | **changed** (PROVEN) | Match: parts only for E4/E8/F6 (unique id). Lobby: any large message (order id or unique id, §4.1). |
| 14 | resend timing | `uniform[a−b, a+b]` | **fixed** `Y` ms; and a **lifetime** `X` after which the sender drops the message | **changed** (PROVEN) | Per-id `(lifetime, resend)` (§4). |
| 15 | unacked limit | 4096 (`max_unacknowledged_packets_count`) | **256** default (same option; hint "32 - 1024") | **changed** (PROVEN) | — (it limits the client, not the server). |
| 16 | client send window | stops after ~32K unacked datagrams | **stops when its next seq is ≥128 ahead of the newest acked** | **changed** (PROVEN) | Ack every client datagram, keep it well under 128. |
| 17 | joining (C→S) | on by default (`disable_joining_net_packets`) | **off by default**; on only with `-join_net_messages` | **changed** (LIKELY) — **wrong: another server saw real clients join by default** (a message bundled with 0xDA, live) | Parse multi datagrams from the client; since 2026-10-12 the parser also takes a snapshot chunk (0xE0 with a payload) inside one, by the chunk's own length. |
| 18 | max datagram / receive buffer | 0x1FC = 508 | 0x1FC = 508 (incl. tail ack bytes) | unchanged (PROVEN) | `MAX_DATAGRAM` stays. |
| 19 | max messages per datagram | unbounded | 252 (`fixed_vector<message_data,252>`, overflow = fatal assert) | **new** (PROVEN) | Cap at 252. |
| 20 | timeout | 5000 ms (120000 under a debugger) | match: same values; **lobby: 5000 ms always** | unchanged (PROVEN, §6) | — |
| 21 | idle / continuous-flow | 100 ms | match 100 ms; **lobby 500 ms** | unchanged for matches (PROVEN, §6) | — |
| 22 | disconnect timers | initiate every 100 ms; peer-initiated: confirm every 100 ms, gone after 500 ms | same | unchanged (PROVEN) | — |
| 23 | connect | first message 0x40 `{u32 session, u8 debug}` | first message **0xD8** `{u32 session, u8 flag (0x00/0xFF)}` | **changed id** (PROVEN) | Accept D8. |
| 24 | ports | 0x50 `{u16 min,u16 max}`; client sends 0x41 `{u32 session}` per port; random port in range | handler 0x27B830 reads `{u16, u16}`; client sends **0xD9** `{u32 session}` per port; random port in range | unchanged mechanism (PROVEN) | Same rule: min = max = server port. |
| 25 | seq ring per message | last 8 seqs | last 8 seqs (9-slot ring of bytes at pkt+0x5C/0x60/0x64) | unchanged (PROVEN) | — |
| 26 | `r == A` quirk | acks ignored | acks ignored | unchanged (PROVEN) | Advance rseq to deliver new acks. |
| 27 | first seq | must not be 0xFFFF | **must not be 0xFF** (client's initial R) | changed width (PROVEN) | Start the server seq at 0. |
| 28 | QoS (qwave.dll) | — | client-local traffic shaping | **new, client-only** (LIKELY) | none |

## 2. Datagram header and acks

### 2.1 Layout — PROVEN

Reader: `0x3FA0A0` (header parse, called at 0x3F3C25). Writer: `fill_packet_header` 0x3FB390 + encoder 0x3FF580. 0.21d equivalent: 021d spec §1 (`fill_packet_header` 0x28B887, `process_incoming_packet` 0x28797D).

```
off 0   u8  local_seq   sender's seq of this datagram (0x3FB3ED)
off 1   u8  remote_seq  newest peer seq received, conn+0x170 (0x3FB3EF)
off 2   u8  ack0        first ack byte
off 3.. messages        (single or multi format, §3)
end-n.. tail ack bytes, written in reverse: t_n ... t_2 t_1   (t_1 is the LAST byte of the datagram)
```

- **n = number of tail ack bytes, 0..17.**
- The reader gets n from the ack bytes themselves: bit 7 of a byte = "another ack byte follows". After ack0 it reads backwards from the end of the datagram (0x3FA1A1–0x3FA1B4). Then it cuts n bytes off the message area: `size − (count−1)/7` (0x3F3CCA–0x3F3CE9).
- If the tail would overlap the first message byte, the header is rejected (0x3FA1B7).
- A datagram shorter than 3 bytes fails the header parse. It goes to the statistics object and is dropped (0x3F3C31).

### 2.2 Ack bit encoding — PROVEN

Let `R` = remote_seq, and `b(k)` = 1 if the sender has received peer seq `R−k` (k = 1..127). `R` itself is always implied (bit 127 = 1, 0x3FA109–0x3FA11A). `F` = 1 for a **multi** datagram, 0 for a **single** one.

**n = 0:**
- `ack0 = F | Σ_{j=1..6} b(7−j) << j`. Bit 7 = 0, bit 6 = b(1), …, bit 1 = b(6).
- **6 ack bits.**

**n ≥ 1:**
- `ack0 = 0x80 | Σ_{j=0..6} b(7−j) << j`. Bit 6 = b(1), …, bit 0 = b(7).
- `t_i` (1 ≤ i < n) `= 0x80 | Σ_{j=0..6} b(7i+7−j) << j`.
- `t_n = F | Σ_{j=1..6} b(7n+7−j) << j`. Bit 7 = 0, and b(7n+7) is not carried.
- **7n+6 ack bits.**

**Count.** The reader calls the number of valid bits including R `count = 7n+7`, clamped to 127.

**Encoder** (0x3FF580; 0x3FF250 = `recv_bits << 7`):
- The first byte is the top byte of the 128-bit received set.
- If count ≤ 7, bit 7 is cleared (0x3FF5D8).
- Otherwise it emits `ceil((count−7)/7)` 7-bit groups with bit 7 set, then clears bit 7 of the last group (0x3FF6B3–0x3FF6F5).
- `fill_packet_header`, 0x3FB408–0x3FB4E0:
  - If n = 0, it clears ack0 bit 7 and puts F in ack0 bit 0.
  - Otherwise it puts F in bit 0 of byte[n], writes ack0 at offset 2, and appends byte[n], …, byte[1] to the end of the packet.
- F = `1 − ((pkt+0x98 >> 5) & 1)`: pkt+0x98 bit 5 = "single message packet". `send_packets_list` clears that bit on joined datagrams (0x3FB995).
- **The polarity is as in 0.21d: 1 = multi.**

**Reader** (0x3FA14B–0x3FA22A):
- A byte with bit 7 set adds `(b & 0x7F) << 1 << (0x78 − count)`.
- The final byte adds `(b >> 1) << (0x7A − count)`.
- `single = !(final & 1)`.

**The client chooses n adaptively ("ack of acks"):**
- conn+0x170 is a received-seq tracker:
  - +0x00 u8 newest R (init 0xFF)
  - +0x08 u128 received bits
  - +0x30 u8 needed depth, capped at 120 (0x3FF521)
  - +0x34/+0x38, ring of 129 × 32-byte entries at +0x3C: the ack state sent with each datagram
- When the peer acks one of the client's datagrams, the depth shrinks (0x3FF2D0, called from `acknowledge_packets` 0x3FD721).
  - **How the depth moves** (read 2026-10-13). PROVEN. A newly received seq adds its distance from the newest one to the depth (0x3FF4FA..0x3FF513: one for each datagram in order); one older than the newest raises it to that distance at least (0x3FF4CD..0x3FF4F8); the cap is 120.
  - When the peer acks the client's **newest** sent datagram the ring is empty and the depth is set to **0** (0x3FF2DE). Else it is worked out from the ack states of the oldest datagram still unacked and the newest sent (0x3FF385..0x3FF47E).
  - So what arrives between a client datagram and the server's ack of it is never named by the depth, only by the 6 bits that always go. A client that sends often is not touched by it; one that sends a pure ack every 100 ms and has a round trip of `r` ms leaves about `r / 100` of what it receives unacked. LIVE 2026-10-13: a waiting game with a round trip of 45 ms acked 53 % of the server's datagrams, 6, 13 or 20 bits a datagram.
- If the full n does not fit in a datagram, the client sends fewer tail bytes. If no datagram of the flush carried the full set, it adds one extra continuous-flow datagram (0x3FCA27–0x3FCA53, 0x3FCBCD–0x3FCBF4).
- **The server may use any n from 0 to 17** (LIKELY from the reader).
- Recommended: send enough tail bytes to cover every client seq received since the newest server datagram the client has acked. Simpler: a fixed n = 2 (20 back-bits) at a ≤100 ms cadence.

### 2.3 Receive order — PROVEN (`process_incoming_packet` 0x3F3BB0)

1. `last_recv = now` (conn+0x13C, xchg at 0x3F3BD2). This runs **before** any parsing, for every datagram that passes the source filter (0.21d: the same).
2. Parse the header (0x3FA0A0). On failure: statistics, then drop.
3. 0x3F1C40 drops the datagram if `local_seq == R` (conn+0x170), or if `remote_seq` is **newer** than the client's last sent seq L (conn+0x11D0). 0.21d only ignored the acks in that case.
4. Parse every message into `fixed_vector<message_data,252>` (0x3FAF40 → 0x3FA240).
   - On failure the seq goes into a second tracker (conn+0x188, 0x3F3D3F). The datagram is dropped: **no acks are processed and its seq is not acked.**
   - **An empty message area is a failure** (0x3FAF55).
5. `update_local_acknowledgements(r, acks, count)` 0x3FD7E0.
6. Record `local_seq` in the received set (0x3FD7A0 → 0x3F3AA0 / 0x3FF4A0). If it is already set (older seq within 128, bit set), the messages are dropped as a duplicate.
   - **Seqs ≥128 older are accepted** (0x3F3AA0: the mask becomes 0). This is the same quirk as 0.21d.
   - Exactly, with `d = (seq − R) mod 256` (record 0x3F3950, re-read 2026-10-14): `d` 1..127 → the new newest, window shifted by `d`, no gap limit; `d == 128` and seq numerically below R → also the new newest, window reset to that seq alone; `d == 128` and seq numerically above → delivered, nothing recorded, R unchanged; `d` 129..255 (older by 1..127) → recorded and delivered if its bit is clear, a duplicate if set. (`d == 0` never gets here: step 3.) So a datagram of ours that arrives 129 to 255 of ours late (or 128, as the lower number) becomes the client's newest, and the client echoes that old seq from then on (§11.2). PROVEN.
7. Dispatch each message:
   - `message_data+0x18` bit 0 = low-level → 0x3FDCF0.
   - Otherwise → 0x3F4720.
   - It stops if the state becomes 3 (0x3F3EC9–0x3F3F1C).

### 2.4 Ack processing (`update_local_acknowledgements` 0x3FD7E0) — PROVEN

State:
- `A` = conn+0x11D1, newest own seq acked (init 0xFF).
- `acked_bits` = conn+0x160 (u128).
- `L` = conn+0x11D0, last own seq sent (init 0xFF).

Cases:
- **r older than or equal to A:**
  - `r == A` → nothing is processed (0x3FD8F4). This is the same as 0.21d.
  - Otherwise, if `A−r < 128`, the new bits are `acks XOR (acked_bits << (A−r))`. They are processed, and only r's bit is added to `acked_bits` (0x3FD8FA–0x3FDA8A).
  - If `A−r ≥ 128`, it logs "skip updating acknowledgements - not enough bits" (a2d2c0).
- **r newer than A** (0x3FDA99), with `d = r−A`:
  - `shifted = acked_bits >> d`.
  - If `count > d`:
    - `below = (acked_bits << (count−d)) >> count`, i.e. the old acked bits below the transmitted window.
    - `new = acks | below`.
    - If `(shifted & new) != shifted` → log "inconsistency in acknowledgements bits" (a2d418), and the whole ack is ignored.
  - Else (`count ≤ d`): `new = shifted | acks`, with no check.
  - Then `acked_bits = new`, `A = r`, and `acknowledge_packets(r, count, new XOR shifted)` 0x3FD690.
- **Bits outside the transmitted window are "unknown", not "missing".** So a server may send a short window (small n) without breaking consistency. It must still never drop a seq it reported earlier that is inside the window it sends now. **(The 0.21d monotonic rule still applies.)**
- `acknowledge_packets` 0x3FD690:
  - It walks `count` bits from bit 127 (= r) downward.
  - For each set bit it deletes every unacked packet whose last-8-seq ring contains that seq (0x3FE640).
  - It also updates the ack-of-ack depth (0x3FF2D0).

### 2.5 Sending side constraints — PROVEN (`send_queued_packets` 0x3FBF70)

- **Send window.** Before each datagram the client checks the next seq `L+1` against `A`. It sends only while `(L+1−A) mod 256` is in 1..127 (0x3FC2C3–0x3FC2FB). Otherwise the rest of the queue is put back (0x3FCB81).
  - **Consequence:** if the server stops acking, the client sends at most 127 datagrams and then nothing at all, **not even continuous flows**.
  - 0.21d's limit was ~32K (021d spec §4.1 step 7).
- **Unacked limit.** The client counts the packets on its unacked list (conn+0x80). If the count is ≥ `max_unacknowledged_packets_count` it disconnects with `instant_disconnect(1)` (0x3FC146–0x3FC158).
  - The global is at 0xAEFA44, static init **0x100 = 256** (0.21d: 0x8DD970 = 4096).
  - It is overridden from the command line on first use (option object 0xAA7050, descriptor 0xAA7260: name a2c2a4, group "network", description a2c248, hint "32 - 1024"; lazy read 0x3FC127–0x3FC141 via 0x141B0). This explains the "data table at 0xAA7260" reference.
- **Datagram size.**
  - A packet larger than 0x1FC is logged as "message size violation for packet" (a2d078) and the client asserts (0x3FC310, 0x3FC398–0x3FC9B6).
  - The free space for joining and for tail bytes is `0x1FC − size` (0x3FCA27).
- **Joining.**
  - Only if the option `join_net_messages` (0xAA6E20; descriptor 0xAA7030: "try to merge packets before sending", "0 or 1") is set (0x3FCA57–0x3FCA7D), and only packets with pkt+0x98 bit 6.
  - Joined packets must have the same destination port pkt+0x90 and must fit (0x3FCAC0–0x3FCB13).
  - The option reader 0x145C0 returns false when the key is absent. **So by default every client datagram holds exactly one message (single format)** (LIKELY: the option state codes are inferred).
- **Ordering of sends.** As in 0.21d, the queue is sorted by size (0x3FE580) and popped from the back.
  - So a datagram holds a tick's messages largest first, not in the order they were sent (PROVEN live, 2026-10-21: the lobby movie's `drop_upgrade_item` then `relocate_upgrade_item` came as `46 01`, unique id 22h, before `46 02`, 21h). A unique id is taken in send order: the server's transport hands a client datagram's unique messages to the lobby in unique-id order (`udp-transport` `in_send_order`; test `a_datagrams_unique_messages_reach_the_caller_in_their_send_order`); other messages keep their place.

## 3. Message encoding

### 3.1 Per-message layout — PROVEN (`try_parse_single_message` 0x3FA240)

```
normal:     id [x] [len] payload
multipart:  FF id [x] part [count] [len] data          (count only when part == 0)
low-level:  id (0xFA..0xFE), nothing after it
```

- **id**:
  - `id == 0xFF` → multipart. At least 2 bytes must remain; then the real id is read (0x3FA32B–0x3FA40E).
  - `0xFA ≤ id ≤ 0xFE` → low-level (0x3FA484). It must end exactly at the end of the message area, else "non-empty low level packet" (a2c4e4). Its `message_data` gets flag bit 0 (0x3FA538–0x3FA571).
  - Otherwise the id must lie in `[getter+4, getter+5]` = [0xE3, 0xF6] on the client, else "invalid message type" (a2c514). **0.26g bounds-checks the id; 0.21d did not.**
- **x** (one byte):
  - `flags & 0x40` → u8 **order id** (0x3FA619, 0x3FA6E1).
  - Else, `flags & 0x20` → u8 **unique id** (0x3FA6EC, 0x3FA7B4).
  - Never both.
  - The sender writes a placeholder 0xFF in `construct_packet` 0x378080 (0x3780E5–0x37810B) and patches it at `enqueue_impl` 0x3FCE30.
- **part / count**: u8 each (0x3FA896, 0x3FA96E).
- **len**: `len = get_message_length(id, reader, first = (not multipart || part == 0), &ok)`. `!ok` → error a2c668 (0x3FA978–0x3FA9C4). Then:

| datagram | `len != 0xFFFF` (fixed) | `len == 0xFFFF` (variable) |
|---|---|---|
| single | the remaining bytes must equal len exactly, else "single message packet, but extra space is found" (a2c6e0, 0x3FAA95–0x3FAAB0) | the message is all the remaining bytes (0x3FAB72) |
| multi | no prefix is read. len is taken as is, **with no bounds check** (0x3FABE9 → 0x3FAB80) | read `u8 L`. If `L ≤ 0xFE` → length L. If `L == 0xFF` → read `u16 L2`; `L2 ≤ 0xFE` is an error "3 bytes used, but one byte must be used instead" (a2c860). The length must be ≤ remaining (a2c8f8). (0x3FACBC–0x3FAE78) |

- **len counts the payload only**, after id/x/part/count. In 0.21d the prefix counted id+order+payload.
- The sender (`send_packets_list` 0x3FB9B0–0x3FBC1A, prefix size helper 0x3FBF20) mirrors this:
  - `flags & 0x80` (fixed) → copies the message raw.
  - Else → copies the header bytes, then `len = size − hdr − 3` as `u8` if ≤0xFE, else `FF u16`.
  - hdr = 1, or 4 for part 0 / 3 for other parts, +1 if x is present.
- **Info flags byte** (0.26g):

| bit | meaning |
|---|---|
| 7 | fixed length / no length prefix (all fixed-size ids have it) |
| 6 | ordered |
| 5 | unique |
| 0–4 | channel (0x1F = none) |

  - 0.21d's layout was: bit 7 ordered, bit 6 reliable, 0–5 channel.
  - In 0.26g, reliability is decided by the lifetime field (§4).

### 3.2 Single vs multi — PROVEN

- **Single** (F=0): exactly one message, no prefix (it may be fixed, variable, multipart or low-level).
- **Multi** (F=1): messages back to back, each per §3.1. At most **252** messages (fixed_vector capacity; `push_back` overflow asserts, 0x3FE383).
- Unlike 0.21d, a multi datagram may carry a single message, and nothing special happens when the first message fills the datagram.

### 3.3 Delivery semantics — PROVEN (0x3F4720)

**Plain** (no 0x20/0x40):
- Delivered immediately (0x3F47DF → 0x3F1F20).
- **No dedup.** A resent reliable plain message is delivered again (as in 0.21d).

**Unique** (0x20, without 0x40):
- There is one unique channel at conn+0x30·(ch+1) (ctor 0x3F9CC0). Its fields:
  - +0x15 u8 next unique id to send (init 0)
  - +0x18 tracker: u8 newest (init 0xFF) + u128 bits (+0x20)
  - +0x04 rbtree of multipart assemblies, keyed by unique id (pkt+0x75)
- On receive:
  - If the unique id is already in the 128-wide window, the message is dropped (0x3F47AA).
  - Otherwise it is recorded (0x3F1C20 → 0x3F3950) and delivered.
- Ids ≥128 behind the newest count as new (0x3F3AA0). So **never have ≥128 unique ids outstanding.**

**Ordered** (0x40):
- Channel at conn+0x30·ch (ctor 0x3F9C20): +0x15 next send (0), +0x2C received (0xFF).
- With flags 0x40 **alone** (no id has them), a single message would be delivered directly without ordering: the dispatcher tests bit 5 first (0x3F4761) and falls into the plain path 0x3F47DF.
- With flags **0x60** (lobby), single and split messages are both held and delivered in order id order: §4.1.
- **No id in either match table has bit 6, so matches do not use ordering.
  The lobby does**: see §4.1.

**Sender side** (`enqueue_impl` 0x3FCE30) — PROVEN:
- `flags & 0x40` (ordered, so also 0x60): write the ordered channel's counter `[conn + 0x30·ch + 0x15]` at data+4, copy it to pkt+0x94, and increment it (0x3FCE40–0x3FCE84). **The unique counter is not touched.**
- Else `flags & 0x20` (unique): 0x3FE600 does the same with the unique channel's counter `[conn + 0x30·(ch+1) + 0x15]` (pkt+0x95) (0x3FCE89–0x3FCE9D).
- Data+4 is right after the 3-byte header placeholder and the id.
- So there are two independent one-byte counters, both starting at 0 (channel ctors 0x3F9C20 and 0x3F9CC0 write 0 to +0x15). A split message takes one value for all its parts (§5).

### 3.4 Low-level messages — PROVEN

`new_low_level_packet(type)` 0x3FBDB0:
- id = `type − 6`, flags 0x1F, lifetime 0 (send-once), dest port conn+0x158 (0xFFFF = random).
- Table 0x3FDF1C.

| id | 0.21d type | client on receive (0x3FDCF0) |
|---|---|---|
| **0xFA** initiate_disconnection | 0 | In state 0: state = 2, conn+0x14C = now, queue `FB` (0x3FDD1F–0x3FDDE7). Otherwise logged. |
| **0xFB** confirm_disconnection | 1 | In state 1: `instant_disconnect` (0x3FDEB0). Otherwise "skip confirming disconnection - we didn't initiated it" (a2d590). |
| **0xFC** continuous_flow | 2 | Calls the functor at conn+0xD0 if set; no reply (0x3FDEC7). |
| **0xFD** — **new** | 3 | Calls the functor at conn+0xF0, then `instant_disconnect(4)` at once, **in any state** (0x3FDEED–0x3FDF0E). The server can use it to kick without the 500 ms handshake. |
| 0xFE | — | The jump table entry is 0xCCCCCCCC: **never send it** (crash). |

- **A keepalive/pure ack is `s r ack0 FC`** (4 bytes plus any tail ack bytes, F=0).

## 4. Per-message info tables — PROVEN

Struct `{u32 lifetime_ms; u16 resend_ms; u8 flags}` (0x279A60 / 0x279B10; copied to pkt+0x88/+0x8C/+0x8E by `construct_packet` 0x378080).

**How the fields are used:**
- `send_packets_list` 0x3FB5BD–0x3FB619:
  - `lifetime == 0` → sent once, never tracked (unreliable).
  - Otherwise `resend_at = now + resend_ms` (pkt+0x74, fixed with no jitter) and, on the first send, `expire_at = now + lifetime` (saturating; pkt+0x84).
- 0x3FE520 requeues the packet when `now ≥ resend_at`.
- 0x3FE470 deletes it from the unacked list once `now ≥ expire_at`. It is **silently dropped; no disconnect**.
- `lifetime = 0xFFFFFFFF` = reliable forever.
- Send-count logging: ≥24 sends logs "ERROR!!!!" (a2cb14). 0xFFFF sends asserts "message send count exceeds maximum number" (a2cd94).

`get_message_length` 0x279670 (table 0x2799E0; returns the payload length, 0xFFFF = variable; `ok` defaults to 1):

**C→S** (`get_sending_message_info`, table 0x279AE0; range [0xD8, 0xE1] at getter+6/+7, 0x2792D4):

| id | payload len | lifetime | resend | flags | class |
|---|---|---|---|---|---|
| D8 connect | 5 | ∞ | 500 | 0x9F | reliable, plain |
| D9 port probe | 4 | ∞ | 500 | 0x9F | reliable, plain |
| DA | 0 | ∞ | 500 | 0x9F | reliable, plain |
| DB | 6 | 0 | 0 | 0x9F | send-once |
| DC | 9 | 500 | 50 | 0x9F | resend every 50 ms for 500 ms |
| DD | 10 | 500 | 50 | 0x9F | resend every 50 ms for 500 ms |
| DE | 2 | ∞ | 50 | 0x9F | reliable, plain |
| DF | 5 | 0 | 0 | 0x9F | send-once |
| E0 | 0 | ∞ | 500 | 0x9F | reliable, plain |
| E1 | 0 | 0 | 0 | 0x9F | send-once |

- All C→S messages are fixed length and plain. **In a multi datagram from the client, the messages are concatenated with no length bytes.** The server must use these lengths.
- **The server must dedup** D8, D9, DA, DE and E0 at application level, as in 0.21d.
- **One exception to the fixed lengths, srp's own:** the patched client's snapshot chunks are E0 with a payload, always alone in a single datagram (client_patch.md §3.4). `udp_transport::wire::parse` takes a single-format E0 from a client with whatever fills the message area (fixed 2026-10-12: it refused such a datagram as a length mismatch, so no upload would have got through).

**S→C** (`get_received_message_info`, table 0x279BF0; range [0xE3, 0xF6] at getter+4/+5, 0x2792CC). The server should mirror these values:

| id | payload len | lifetime | resend | flags | class |
|---|---|---|---|---|---|
| E3 (ports `{u16,u16}`, handler 0x27B830) | 4 | ∞ | 500 | 0x9F | reliable, plain |
| E4 | var (needs ≥1 B) | ∞ | 500 | 0x20 | reliable, **unique**, length-prefixed |
| E5 | 7 | 0 | 0 | 0xA0 | send-once, **unique** |
| E6 | 4 | 0 | 0 | 0xA0 | send-once, **unique** |
| E7 | 2 | 0 | 0 | 0xA0 | send-once, **unique** |
| E8 | var (first part needs >2 B remaining) | 1750 | 500 | 0x20 | **unique**, length-prefixed, 1.75 s lifetime |
| E9 | 2 | 250 | 50 | 0x9F | short-lived |
| EA | 2 | 250 | 50 | 0x9F | short-lived |
| EB | 10 | 250 | 50 | 0x9F | short-lived |
| EC | 11 | 250 | 50 | 0x9F | short-lived |
| ED | `1 + B + 8·popcount(mask)` | 0 | 0 | 0x9F | send-once, length from content |
| EE | `2 + B + 8·popcount(mask)` | 0 | 0 | 0x9F | send-once, length from content |
| EF | 2 | 250 | 50 | 0x9F | short-lived |
| F0 | 4 | 250 | 50 | 0x9F | short-lived |
| F1 | 1 | 250 | 50 | 0x9F | short-lived |
| F2 | 2 | 250 | 50 | 0x9F | short-lived |
| F3 | 3 | ∞ | 50 | 0x9F | reliable, plain |
| F4 | 2 | ∞ | 50 | 0x9F | reliable, plain |
| F5 | 1 | ∞ | 50 | 0x9F | reliable, plain |
| F6 | var (needs ≥1 B) | ∞ | 500 | 0x20 | reliable, **unique**, length-prefixed |
| E2 | — | — | — | — | the jump table entry is 0: never send it |

**ED/EE length** (0x27975B–0x2799A2):
- The payload is `[1 or 2 bytes] [B-byte player mask] [8 bytes per set bit]`, with `B = getter+8`.
- B is set by 0x279610 to `(players−1)/8 + 1`; it is 0 at construction (0x2792E2).
- If the payload is too short → `ok = 0` (logs a08008/a081d8). "multiple players inputs changed" strings. **(Game layer: see match.md.)**

**Notes:**
- Lifetimes and resend times apply to the sender. For S→C they are the server's choice. The client only uses `flags`, and lifetime only for multipart expiry (LIKELY).
- The unique ids E4, E5, E6, E7, E8 and F6 all use unique channel 0 and **share one u8 counter** starting at 0. A multipart message consumes one id.

### 4.1 The lobby connection has its own table, and it uses ordering (added 2026-10-12) — PROVEN

Everything above in §4 is the **match** client's table object (vtable
0xA08414). The lobby connection runs on the same transport code with a
second table object, set up in the `lobby_client` constructor at 0x26F15B:
vtable **0xA07FA8**, received ids **[0x80, 0xAA]** (getter+4/+5), sent ids
**[0x2E, 0x50]** (getter+6/+7).

- **Length** (vtable [2] = 0x26E390): always returns 0xFFFF with `ok = 1`.
  **Every lobby message is variable length** in both directions. So in a
  multi datagram each one has a length prefix (`u8`, or `FF u16`), and in a
  single datagram it is the rest of the datagram (§3.1).
- **Info** (vtable [0] and [1] are the same function, 0x26E370 → 0x26E260;
  class byte per id at 0x26E2F0 indexed by `id − 0x2E`, jump table
  0x26E2D8). No lobby id has bit 7 (fixed length).

| ids | lifetime | resend | flags | class |
|---|---|---|---|---|
| C→S 2E..50 (all the client sends) | ∞ | 1000 | 0x20 | reliable, **unique** |
| S→C 80, 81, 82, 9A, A0, A1, A2, A4, AA | ∞ | 1000 | 0x20 | reliable, **unique** |
| S→C 83, 84, 85, 87..99, 9B..9E, A3, A5, A6, A8, A9 | ∞ | 1000 | **0x60** | reliable, **unique and ordered** |
| S→C 9F | 0 | 0 | 0x1F | send-once, plain (no x byte) |
| 51..7F, **86**, **A7** | — | — | — | the jump table entry is 0: asking for the info of these ids is a jump to address 0 |

Consequences:
- Every lobby message except 9F has the x byte: `id x [len] payload`. The
  sign-in is `2E <unique> <u32 session> <bool>`; the layouts in lobby.md and
  lobby_requests.md start after x.
- **Never send 0xA7 or 0x86: the client jumps to address 0.** PROVEN:
  `try_parse_single_message` reads the id (0x3FA32B, or 0x3FA408 for the
  real id of a part), checks it against `[getter+4, getter+5]`
  (0x3FA412–0x3FA41E) and, when it is in range, goes straight to 0x3FA5E4,
  which calls vtable slot 1 with the id (`mov eax,[esi]; mov eax,[eax+4];
  call eax`, 0x3FA5E8–0x3FA5F5). For the lobby table that is 0x26E370 →
  0x26E260: `movzx edx, byte [id + 0x26E2C2]; jmp [edx*4 + 0x26E2D8]`. The
  class byte of 0x86 and 0xA7 is 5 and entry 5 of the jump table is 0. The
  game handler's "read and ignored" for 0xA7 (lobby.md) is never reached.
  The same happens for a part (`FF 86 …`).
- An id outside 0x80..0xAA fails the range check above ("invalid message
  type", 0x3FA424), so the datagram is dropped before the lobby handler's
  unchecked jump table (lobby.md H1) is reached. PROVEN (the range bytes
  are the lobby table object's, written at 0x26F15B–0x26F167).
- The class bytes (0x26E2F0, index id − 0x2E; jump table 0x26E2D8 =
  {0x26E2AD, 0x26E274, 0x26E2C1, 0x26E288, 0x26E29C, 0}): 0 and 1 = flags
  0x20, 2 and 3 = flags 0x60, 4 = plain send-once (0x9F only), 5 = null.
  Read from the exe: 2E..48 and 4C..50 class 0; 49..4B class 1; 51..7F
  class 5; then as in the table above. PROVEN.
- The server must dedup client messages by their unique id (128-wide
  window), and keep fewer than 128 outstanding.

**Ordered delivery (flags 0x60).** The dispatcher 0x3F4720 tests bit 5
first; with bit 6 also set it goes to 0x3F480A (not the plain path at
0x3F47DF that §3.3 describes for a bare 0x40):
- The x byte is an **order id** from the ordered channel `conn + 0x30·ch`
  (ch = flags & 0x1F = 0), separate from the unique channel's counter.
- 0x210000 compares it with the channel's last delivered id (+0x2C, init
  0xFF): not newer (mod 256, half range 128) → dropped.
- Already waiting in the channel's tree (+0x1C, keyed by the id at node
  +0x74) → dropped.
- Otherwise the message is copied into a packet and 0x3F5150 inserts it,
  then delivers **in order**: while the tree holds id `last + 1`, deliver
  it (0x3F6690), erase it and `inc byte [channel+0x2C]` (loop
  0x3F51F7..0x3F5318). A message whose predecessor is missing waits.
- So the first ordered message must carry order id 0, and each next one
  +1. The unique-class ids (80, 82, A1, A2 …) use the other counter and
  are **not** ordered against these: 0x80 can overtake nothing, but a 0x82
  or 0xA1 can arrive before an earlier 0x85/0x87.
- The compare 0x210000(`a` = last delivered, `b` = order id) returns
  "drop" when `b` is not newer than `a`: for `a < b` when `a + 128 <= b`,
  for `a >= b` when `b + 128 > a`. So an id 1..127 ahead is taken; 128
  ahead is taken only when the byte wrapped (`b < a`); anything else is
  **dropped silently**. PROVEN.

**Split ordered messages** (`process_multipacket_reliable_unique_ordered_message`
0x3F4A50, called from 0x3F473E when the message has the multipart flag
`message_data+0x18` bit 1 and `flags & 0x40`) — PROVEN, traced 2026-10-12:
1. The same two drops as a single message: order id not newer than the
   last delivered (inline at 0x3F4A77–0x3F4A9E), or already complete and
   waiting in the tree at channel+0x1C (0x3F4AA4–0x3F4AD3).
2. Parts are **keyed by the order id** (`message_data+0x14`): one assembly
   per order id in a second tree at channel+0x04 (lookup
   0x3F4AD9–0x3F4B18). The assembly is the packet of the first part that
   arrived (node = pkt+0x20, key at node+0x74 = pkt+0x94, part count at
   node+0x77 = pkt+0x97, list of parts at node+0x10).
3. **Expiry check** (0x3F4B1A–0x3F4CAA). Only when the order id is newer
   than `channel+0x14` (the newest order id seen in a part; ctor value 0,
   0xFF after a reset 0x3F9CB6) and has no assembly yet: `channel+0x14`
   becomes this id; then the assembly with the next larger key (or, if
   none, the tree's first) is compared with it (0x210000 at 0x3F4BD6). If
   that key counts as *newer* than the new id, the client logs "ordered
   message has been EXPIRED: %d, but %d" (a2afb8) and calls
   `instant_disconnect(5)` (0x3F4C9C). With step 1 in front (both ids are
   within 127 of the last delivered one, so within 126 of each other) this
   cannot fire for ids that passed step 1. **It is a second line of
   defence, not the real hazard**: see "what a sender must respect" below.
4. A part whose index is already in the assembly is dropped
   (0x3F4CB5–0x3F4CD9, find 0x3F7090).
5. Otherwise a packet is taken from the pool (0x279D80, init 0x278D00) with
   the part index at pkt+0x1C, the id at +0x96 and the order id at +0x94;
   a new assembly is inserted (0x3F5C30). **Part 0 writes its `count` to
   the assembly** (0x3F4D28–0x3F4D35); until then the count is **0xFF**
   (0x278D92), so parts may arrive in any order. The data is copied to
   pkt+0x2A8 and the part linked in (0x37A160).
6. When the number of parts ≥ count (0x3F4D64–0x3F4D6A) the assembly is
   unlinked from the parts tree (0x3F5CC0) and handed to **0x3F5150, the
   same function single ordered messages go to**.

**In-order delivery** (0x3F5150) — PROVEN: it inserts the message into the
tree at channel+0x1C (0x3F51E6), then loops (0x3F51F7–0x3F531C): look up
`last + 1` (wrapping from the last key to the tree's first); if it is
there, deliver it (0x3F6690), erase it (0x3F6560) and `inc byte
[channel+0x2C]`; stop at the first gap. **Single and split ordered messages
share this one tree and one sequence**: a completed split message with
order id n is delivered after n−1 and before n+1 whatever arrived first.

**Limits** — PROVEN where an address is given:
- Parts: `u8` index and `u8` count, so at most 255 parts (the server uses
  496 + 254 × 497 bytes at most; §5).
- Ids: one byte, window as above. No count limit of its own on waiting
  messages or assemblies; each takes a packet from the connection's pool
  (conn+0x134; pool size: open question 1).
- Nothing expires by time: an incomplete assembly waits for ever, and
  blocks every later order id.

**What a sender must respect** (the server mirrors it in
`crates/udp-transport`, `CHANNEL_WINDOW`):
- The first ordered message has order id 0, each next one +1, counted
  apart from the unique ids.
- **Never put an order id on the wire that is more than 127 past the
  oldest one the client may not have delivered yet.** The client drops it
  in step 1 *and still acks the datagram*, so it is never sent again, and
  the ordered channel then waits for that id for ever (every later push is
  held). Unique ids have the same limit for a different reason (§3.3: an
  id 128 behind the newest counts as new, so a resent copy is delivered a
  second time).
- A copy of an already delivered message that arrives late (resent before
  the ack came, or duplicated on the way) must still be inside the window,
  or it is taken for a *future* id and waits in the tree, where it would
  later be delivered in place of the real message with that id. The
  server keeps an id in its window for 500 ms after its ack for that.
  LIKELY a real hazard only at rates above ~200 ordered messages a second.

**Timeouts and keep-alive of the lobby link** — PROVEN, traced 2026-10-12.
Both clients are built by the same function 0x378A40 (lobby: called at
0x26F10B; match: 0x27A147). Its stack arguments 7 and 8 are the timeout and
the idle time: 0x378A40 binds its arguments 4, 6, 7, 8, 9, 10 (0x378C23–
0x378C54, bind 0x37A230) to 0x3787C0, which passes bound 3 and 4 (and 5 in
eax, the QoS bandwidth) to 0x385910 (0x3787FF–0x37881F), which calls the
`udp_match_client` ctor 0x3F1C90 with `ecx` = bound 3 and `eax` = bound 4
(0x3859EE–0x3859FE); that ctor pushes them as arguments 3 and 4 of the
connection ctor 0x3F9D60 (0x3F1C9F–0x3F1CBB), which stores argument 3 at
conn+0x140 (timeout) and argument 4 at conn+0x148 (idle)
(0x3F9DE5–0x3F9E03).

| | timeout (arg 7 → conn+0x140) | idle (arg 8 → conn+0x148) | first received id (arg 6) |
|---|---|---|---|
| match, 0x27A0F6–0x27A147 | 5000 ms, 120000 under a debugger | 100 ms (`push 0x64`) | 0xE3 |
| lobby, 0x26F0C5–0x26F10B | **5000 ms always** (`push 0x1388`, 0x26F0ED) | **500 ms** (`push 0x1F4`, 0x26F0E8) | 0x80 |

- The lobby link runs the same `send_queued_packets` 0x3FBF70, so it sends
  the same low-level `FC` when it had nothing to send for the idle time,
  here every **500 ms**, and gives up after 5 s without a datagram.
- A lobby message is resent after 1000 ms (table above), so a server
  should ack every lobby datagram at once rather than wait for its own
  next message: the client's unacked request and its `FC`s otherwise
  repeat.
- The same code also means the lobby client stops sending when 127
  datagrams are unacked (§2.5).

## 5. Multipart — PROVEN

**Split rule** (0x3F9D00): a message is split when `payload + 4 + (x ? 1 : 0) + n_tail > 0x1FC`, where n_tail is the current number of tail ack bytes. So **a single message carries at most 503 payload bytes with x, 504 without, minus n_tail.**

**Sender** (`enqueue` 0x3FD060, `construct_multipacket_part` 0x3FCF60):
- Every part is `FF id x part_index`; part 0 also has `count`. x is the same for every part (0x3FCFE3–0x3FD047).
- **Which counter x comes from** (0x3FD16C–0x3FD265): `flags & 0x40` → the ordered channel's counter `[conn + 0x30·ch + 0x15]` (0x3FD177–0x3FD1B8); otherwise the unique channel's `[conn + 0x30·ch + 0x45]` (0x3FD1EF–0x3FD230), **whatever the unique flag says**. The counter is incremented once per message (0x3FD261–0x3FD265), not per part.
- Part 0 data = `0x1FB − 7 − n_tail` = **500 − n_tail** (0x3FD2B5–0x3FD2CF); the count byte is written last, before the data (0x3FD43D–0x3FD45E).
- Later parts = `min(rest, 0x1FC − 7 − n_tail)` = **501 − n_tail** (0x3FD388–0x3FD3AD). PROVEN.
- n_tail here is the tail the connection currently sends (conn+0x1A0, the "needed depth": `(depth − 7 + 6) / 7` for depth > 7, else 0; 0x3FD284–0x3FD2B2), the same value the split rule uses.
- The data is the payload only.
- Each part is its own packet, with the id's lifetime and resend values.
- A **plain** id (no 0x20/0x40) cannot be split: the sender writes the x byte in every part, but the reader only reads x for ids with 0x20 or 0x40 (§3.1), so the part would be misread. No plain id is ever large enough.

**Receiver** (`process_multipacket_unique_message` 0x3F4DA0):
- Drops a part if its unique id is already complete (0x3F4DD4).
- Moves the window forward when the unique id is newer. It removes assemblies whose id is now outside the window, logging "removing expired unique multipacket message: %d, but %d" (a2b170).
- Inserts the part by index (duplicates are dropped). Part 0 sets `count` (pkt+0x77, 0x3F50A8); before that it is 0xFF (packet init 0x278D92), so the order of arrival does not matter.
- When `parts ≥ count` it marks the unique id received and delivers the concatenation (0x3F5100–0x3F513D).
- Limits: u8 count, so ≤255 parts. Max message ≈ 255 × 500 ≈ 125 KB.
- Parts of any size are accepted (LIKELY: no size check in either receiver).
- **No time limit** (read 2026-10-13). An unfinished unique message is removed in two places only: here, when a unique id 128 or more newer comes (0x3F500C), and when the connection is reset (0x3FE15F..0x3FE220). The third caller of the remover (0x3F52F2) is the ordered channel's delivery. PROVEN for the direct calls. So the lifetime of a split message is its sender's alone, and parts may come spread over any time.
- **Each part taken holds a pool packet**: 0x279D80 takes one (in use `[pool+0x30]` against the size `[pool+0x34]`), and when the free list is empty it returns null, which this function tests only to skip the constructor (0x3F506B) and then writes through (`mov [edi+0x1c],dl` at 0x3F5075): a crash. PROVEN. That it is the 256-packet pool the client's own sends come from (client_patch.md §3.4) is LIKELY (the pool pointer is conn+0x134; its allocation was not traced). So a sender must not leave many unfinished messages in the client: each one keeps its parts until 128 newer unique ids have come.
- Split messages of an **ordered** id go to 0x3F4A50 instead, keyed by the order id: §4.1. In a multi datagram a part of a variable id carries a length prefix (`get_message_length` returns 0xFFFF for E4/E8/F6 when the part is not first or the message is variable).

## 6. Timers, resend, keepalive — PROVEN unless marked

**Timeout:**
- `udp_match_connection` ctor 0x3F9D60 stores timeout → conn+0x140 and idle → conn+0x148.
- The match client is created at 0x27A0F6–0x27A147 with `eax = IsDebuggerPresent() == 1 ? 120000 : 5000` (0x27A118–0x27A127: `0x1D4C0 + (−115000 if not debugged)`) and `push 0x64` (100).
- These are the same values as 0.21d (`udp_match_client` ctor 0x2863A8). The parameter → field mapping through the bind chain (0x378A40 → 0x3787C0 → 0x385910 → 0x3F1C90 → 0x3F9D60) is **PROVEN**: traced in §4.1, which also has the lobby's values (5000 ms always, idle 500 ms).

**State switch** (0x3FBF8F, table 0x3FCC20):

| state | behaviour |
|---|---|
| 0 | `last_recv != 0 && now ≥ last_recv + timeout` → "client [%s] disconnected by timeout" (a2ce84), `instant_disconnect(0)` |
| 1 | queue `FA` every idle interval while the send queue is empty |
| 2 | after `now ≥ conn+0x14C + 500` → `instant_disconnect`; else queue `FB` every idle interval |

- After the switch: unacked limit; expire (0x3FE470); resend due (0x3FE520).
- If the send queue is empty and `now ≥ last_send + idle` → queue `FC` (0x3FC1B1–0x3FC1D5).
- `connect` 0x3FCC40 sets state 0 and `last_recv = now`, then queues the connect packet. **There is no handshake, as in 0.21d.**
- `instant_disconnect` 0x3FDF30 resets every seq to 0xFF, the trackers, and both channel trees.

### 6.1 Who drives the lobby link, and when a client is silent — traced 2026-10-12

**The link is ticked from the logic thread, in every state.** PROVEN unless marked.

- A tick of a link is `0x379760(lobby_client+8, now)`: it posts the functor 0x378150 (which calls 0x3F33F0 → `send_queued_packets` unless the client's state at +0xB01F4 is 3) into the network world's queue (`[+0x1E55B8]+0x94`, an `xchg` list). The network thread runs the queue at the top of `network_world::tick` 0x110B80 (0x110D30) and then polls the sockets for up to 40 ms. That the network world has a thread of its own is LIKELY (the 0.100b source's `engine_world::network`; the queue is a cross-thread one).
- **Game running, window active:** `game::tick` 0x1EC890 calls `network_client::tick` (vtable +0x18) at 0x1ECBFF..0x1ECC22. It ticks the lobby link when 500 ms have passed since the last tick (0x27B3A3..0x27B3E1; the time is kept at lobby_client+0x1EE608), before anything of the match. So the lobby link is ticked the same way **in the lobby, while a level loads and in a match**. (`game::tick` has two early returns, 0x1EC9DD and 0x1ECAC9, whose conditions were not read: LIKELY not taken in normal play.)
- **Window not active (alt-tab, minimised):** `engine_world::logic_tick` 0x1A030 does not call `game::tick` once the window was shown (flag engine_world+0x313, set at 0x18A0F) and `is_application_active` (0x1A0E1..0x1A0EE) is false. It calls the game's vtable +0xC instead (0x1A122..0x1A134): `game::on_waiting_for_render` 0x1EDDE0 → `network_client` vtable +0x1C = 0x27D010 → 0x27CFB0, which sends the match's queued messages (0x27CBF0) and **ticks the lobby link by the same 500 ms rule** (0x27CF60..0x27CFA2). 0.21d has the same branch (`logic_tick` 0x164FC → `game::on_waiting_for_render` 0x1547E2).
- **Logic thread ahead of the renderer:** the wait loop of `engine_world::logic` 0x1A390 calls the same `on_waiting_for_render` on each turn (0x1A4C5..0x1A4D7).
- So a healthy client's lobby link sends something at least every ~500 ms (a request, or the low-level `FC` when the queue is empty and 500 ms passed, §6) **whatever the window does**. The 0.21d lobby was TCP and had no such timer; its match link was driven the same way.
- **It is silent only while the logic thread itself is held up** in one long call, the machine sleeps, or the game or its network is gone. Loading a level sleeps 10 ms per logic tick (`is_loading`, 0x1A04A..0x1A06E) and can hold the thread for long: LIKELY (not measured; upstream saw 51-58 s stalls of unfocused clients under Wine, and live 0.21d clients froze on a fullscreen alt-tab).
- While the logic thread is held up the network thread still takes datagrams, so `last_recv` stays fresh as long as the server keeps sending (it sends `FC` once a second to a client that acks nothing, §11): the client does **not** time the server out when it wakes. LIKELY (follows from the two threads; not run).

**When the server is silent** — PROVEN unless marked.

- 5000 ms without a datagram (checked only when the link is ticked): "client [%s] disconnected by timeout", `instant_disconnect(0)` (§6).
- The lobby client's two connection callbacks, 0x26F960 and 0x26F9B0 (bound at 0x26F7C1 and 0x26F75B; which is the error one and which the disconnect one was not told apart), both call Flash `root.show_sync` (a black backdrop with the "server sync" clip over the lobby; nothing can be clicked), add 1 to the retry count lobby_client+0x1E5780 and set the reconnect flag +0x1E5784.
- `network_client::tick` (so only with the window active), while the flag is set and the login client is signed in (0x27B204..0x27B23D): every 5000 ms (0x27B293..0x27B2A1) it logs "LOBBY: try reconnect" and calls 0x21BD90 → 0x21BDE0 → `lobby_client::connect` 0x26F650 with the lobby's address and **the session it has**: a new link, whose first message is the sign-in `2E`. A try that fails counts one more.
- When the count is over 5 (0x27B24C..0x27B253: after the fifth failed try, about half a minute) it calls `login_client::sign_out` 0x376BB0 and 0x1ED260(game, 0): the login screen. LIKELY (0x1ED260 was read only at its start).
- The sign-in's answer 0x80 sets the count again (0x26FBAB, LIKELY to 0).
- A sign-in that the server refuses (`FD`) is one more failed try.

**What the game is when it connects again (added 2026-10-12).** PROVEN unless marked.

- There is **one path**: the first connect and every later one are the same "LOBBY: try reconnect" → 0x21BD90 → `lobby_client::connect` 0x26F650 (the client's log has that line before its first sign-in too). 0x26F650 copies the 0x50 bytes of connect data to lobby_client+0x1E5738, keeps the retry count (+0x1E5780 is read before the copy and written back after), makes the link and queues `2E <u32 session> <bool need_shop_items>`. **It resets nothing of the game**: the lobby client, the inventory vector (+0x1EA978), the shop array (+0x1EE5F4), the daily bonus block (+0x1EE614), the lobby menu and its Flash movie are the same objects with what they held. The only thing the sign-in says about it is `need_shop_items` = `00` when the shop array is there (0x26F6D0: `cmp [ebp+0x1EE5F4],0; setnz bl; dec bl`).
- The answer `80` sets the retry count to 0 (`mov [esi+0x1E5780],edi` with edi = 0 at 0x26FBAE: PROVEN now) and the client state (+0x1E5790) from its first byte.
- So the protocol has no "resume": **a game that connects again expects the whole sign-in again**, and reads it with the same readers as the first time. The readers that were read replace what is held (`99` overwrites its block and resizes its item's two vectors to the counts it reads, 0x1F1500 / 0x1F1610: PROVEN; the inventory `89` gives its vector the count it reads, 0x27047F → 0x276CC0: LIKELY a resize, read only at the call). LIVE 2026-10-12: the game that connected to the restarted server took `80`, then `8F 90 8B 96 8E 8D 94 8A 89 87 8C 92 9C 9D 9F 9B 85 A3 A3` of its second sign-in with no complaint and died in the `99` that followed, of that message's own fault (results.md §4.1), not of being a second sign-in. That the rest (`A1`, a results screen, an admin text) is taken a second time as well is LIKELY (same readers; not seen live yet).
- What a second sign-in must not do is count on a fresh game: a message that is wrong but harmless the first time (a reader that runs past the end and finds zeros) is not harmless the second.

**What srp does** (`servers/lobby-server/src/v026g/away.rs`; rules that are srp's own are marked):

| step | when | what |
|---|---|---|
| the link goes | 30 s of silence (`SRP_LOBBY_LINK_TIMEOUT_SECONDS`); 120 s for a player in a match that runs (`SRP_LOBBY_MATCH_LINK_TIMEOUT_SECONDS`) [choice] | out of the queue, not ready in its squad, out of its clan team's wait, offline for its friends. The session stays |
| the session waits | 60 s (`SRP_LOBBY_SESSION_GRACE_SECONDS`) [choice: the client's five tries]; for a player whose match still runs the wait starts at the match's end | a sign-in with the session is taken back as it was: in its match if that still runs (no second 0x82), else in the lobby with the results it has not seen. The store takes a player unheard of for 30 s out of its squad |
| the session ends | the wait is over, or the account signed in anew | a sign-in with it is refused (`FD`) |
| the server came up again | sessions restored from the database; no link, nobody away | a sign-in with a restored session is a sign-in like the first: the whole of it, without `95` when the game says it has the shop. The same for a client taken back within the wait. Tested with a model game that keeps what it holds (`Client::reconnect`, `tests/core_tests/reconnect_tests.rs`) |

A client that leaves in words (`FA`) ends its session at once, as before. 30 s stays the lobby's timeout because the client keeps talking in every state; the longer one for a match is for a level that loads, and because a link dropped in a match makes the client sign in to the lobby again in the middle of it (which no live client was seen to do).

## 7. Connection setup, ports — PROVEN

**`connect_impl` 0x3F2610:**
- Socket bound to 0.0.0.0:0 (0x3F2641–0x3F266F), SO_RCVBUF/SO_SNDBUF = 0x20000 (0x3F2679–0x3F2692).
- Source filter `[client+0x1D00, +0x1D02] = [port, port]`; random range `client+0x131C/0x1320 = {port, port}` (0x3F28DD–0x3F2932).
- Then `connect` and `start_receiving` (receive buffer **0x1FC**, 0x3F25D9). The filter check is in handle_receive 0x3F2338–0x3F2393.

**First message:**
- `match_client::connect` 0x27A1B0 builds id **0xD8**, `u32 session` (0x27A1F1–0x27A207), `u8 flag` = `global 0x1F1D32C != 0 ? 0xFF : 0x00` (0x27A213–0x27A23D).
- The flag is LIKELY `g_debug_hash_mismatches`, as in 0.21d 0x40. **Note it is 0xFF, not 0x01, when set.**
- First client datagram: `00 FF 00 D8 <u32 session> 00` (9 bytes). LIKELY: built from the writer rules; the initial client R = 0xFF and an empty set give ack0 = 0.

**Ports:**
- S→C **0xE3** `{u16 min, u16 max}` (handler 0x27B830) calls `set_connection_ports` 0x3F2B50 through vtable+0xC with id **0xD9** and the session.
- That sets the filter to `[min, max]` and the random range to `{min, max}`. For each port p in [min..max] it queues **D9 `{u32 session}`** with dest port p (0x3F2C4A–0x3F2D5A).
- Later traffic goes to a random port in the range (0x3F3010).
- **Same mechanism as 0.21d.** Recommended: `min = max = server port`.
- The optional QoS flow is set up here too, if enabled (0x3F2B95–0x3F2C12).

## 8. QoS controller — LIKELY client-only

- `qos_controller` (0x3F9250 loads qwave.dll; `create_qos_handle` 0x3F9991, `add_client_to_flow_impl` 0x3F970A, `set_flow_bandwidth` 0x3F93A9; traffic types best_effort … control).
- It is created when the `qos` option (0xAA1090 / 0xAA12A0, "set usage of Quality of Service") is given. The bandwidth comes from `match_bandwidth_in_kbits` (0xAA14F0 / 0xAA1700, default 128 → ×1024, 0x27A0A2–0x27A0C2).
- It calls Windows qWAVE flow APIs on the client's own socket. **No bytes on the wire change; nothing for the server to do.**

## 9. Worked bytes (server side)

**(a) Keepalive.** The server has received client seqs 5, 4 and 2. Server seq 9, single, n=0:
- ack0 bit 6 = b(1) = seq 4 = 1; bit 5 = b(2) = 3 = 0; bit 4 = b(3) = 2 = 1. So ack0 = 0x50.

```
09 05 50 FC
```

**(b) E3 ports `{1236, 1236}`.** Server seq 0, after the client's D8 (seq 0), single, n=0:

```
00 00 00 E3 D4 04 D4 04
```

**(c) Multi: E9 `{2 B}` + F6 (unique id 3, 300 B payload).** n=1:
- Bytes: `s r ack0(0x80|bits) | E9 p0 p1 | F6 03 FF 2C 01 <300 B> | t1(bit0=1)`.
- F lives in t1 bit 0.

## 10. Open questions

1. **Packet pool size.** The pool uses 0x2C0-byte packets (0x279C40: `count = bytes / 704`). The byte size passed at 0x279229 was not resolved. The 0.21d 4096-packet limit was not re-checked.
2. ~~Part data size for parts ≥ 1.~~ Settled 2026-10-12: 501 − n_tail (§5).
3. **The `qos` option reader polarity** at 0x27A095. Irrelevant on the wire.
4. **`join_net_messages` default.** "Off" rests on reading `0x145C0` state 1 as "key absent"; live evidence from another server says clients do join, so that reading is wrong. The server parses multi datagrams, a joined snapshot chunk too. Tail ack bytes: the reader 0x3FA157..0x3FA1C3 has no limit but the datagram's length; `ack::decode` takes up to 18 (2026-10-12), `encode` still sends at most 17.
5. **0x188 tracker** (corrupted-seq set). It does not appear to feed the header; not checked further.
6. **Client flush cadence in a match.** Not re-traced; 0.21d was ~100–200 ms.
7. **What D8's flag byte (0x1F1D32C) is.** Taken to be `g_debug_hash_mismatches` by analogy with 0.21d.
8. **Whether the client tolerates duplicate plain S→C reliables** (E3, F3–F5). The transport delivers duplicates; game-layer tolerance is not checked.
9. **State 4** (checked at 0x3FCBFC → `instant_disconnect` after a flush): where it is set was not traced.
10. **How many waiting ordered messages and assemblies the packet pool holds** (see 1). The server keeps at most 100 order ids and 100 unique ids in flight, and a real lobby sends far fewer.
11. **Whether the real client limits its own unique ids in flight.** Nothing in `enqueue` does. A client that had 128 lobby requests unacked would break the server's dedup window; the lobby UI cannot produce that.
12. **`channel+0x14` after a reconnect.** The ctor writes 0, the reset (0x3F9C60, from `instant_disconnect`) 0xFF. It only matters for the expiry check, which cannot fire for a sender that keeps the window (§4.1).
13. **The lobby's arguments 9–14 of 0x378A40** (a bandwidth-like value, a bool, a functor, 0x1F1D9E8, 0x2E, 0x80) were not followed; 0x2E and 0x80 are the first sent and received ids, as 0xD8 and 0xE3 are for the match.
14. **The client acks a message through one-byte seqs** (ring of 8, §1 row 25). A message still unacked after 256 later datagrams can be deleted by the ack of a newer datagram with the same seq byte. Read from the ring's width, not tested live; the server's own ring holds full datagram numbers, so only the client's sends are exposed, and only at send rates far above a real client's.

## 11. The server's implementation (added 2026-10-12)

`srp-026g/crates/udp-transport` implements this note for both tables. What
it adds on its own judgement, to stay inside the client's 128-wide windows:

| rule | value | why |
|---|---|---|
| data waits when this many datagrams are past the newest acked one | 100 | the client acks 127 seqs back at most |
| unique / order id may run this far past the oldest unacked one | 100 | §3.3 (unique), §4.1 (ordered) |
| an id still counts as unacked after its ack, its one send or its expiry | 500 ms | a late copy must still be inside the window |
| datagrams with messages at once, then one per | 48, 1 ms | a datagram arriving ≥128 seqs late is taken as new (§2.3 step 6) |
| tail ack bytes | enough for every client seq past the last one the client is known to have had acked, plus 14; cut to fit a full datagram (4 always fit) | §2.2, §2.4 (`r == A` quirk) |
| keep-alive `FC` | 20 ms after a client datagram if nothing else carried the ack; else every 100 ms (match) / 250 ms (lobby); 1 s towards a client that acks nothing | §2.5, §6 |
| part sizes | 496 (part 0) and 497 | 500/501 less 4 tail bytes |
| parts of split messages at once, then one per (match link; the lobby has no limit) | 4, 3 ms | 28 full datagrams in one millisecond are more than some lines carry (live 2026-10-13, match.md §27); the client takes parts at any pace (§5) |
| a message's lifetime or resend time, where its sender says so | `send_lasting`, `send_kept_every` | the client's table values are the sender's own matter (§4) |
| a message's end at a moment, counted from when it is queued | `send_until` | §11.3 |
| short-lived messages wait for others to share their datagram (match link; lobby 0) | 15 ms (`SRP_RELAY_PACK_MS`) | §11.1 |
| lost serials remembered for `is_lost` | 4096 | a stall of a few seconds ends hundreds of relays before their sender looks |

### 11.1 Packing short-lived messages (added 2026-10-14)

Each relay used to go in a datagram of its own as soon as it was queued:
100 to 220 datagrams a second to each client of a full match. The one-byte
seq then lapped every 1.2 to 2.5 s, and the client's acks, which always
carry 6 bits and more only as the server's acks of them allow, named a
smaller share of our datagrams (99.3 to 99.8 % at 60 to 100 datagrams a
second, 87 to 92 % at 160 to 220).

Now a short-lived message (the table's 250 ms ids: EB, EC, E9, EA, EF,
F0, F1, F2) waits until `Config::gather` (15 ms on the match link) has
passed since the last datagram with messages. Everything queued then goes
together, as many to a datagram as fit (an EB is 11 bytes: some 44 to a
datagram). It goes at once, and takes the waiting ones along, when:

- anything else is queued: E5, E6 and E7 (a wait would skew the round
  trip and clock samples taken with them), E8, F3 to F6, E3, E4. A
  short-lived id kept until acked (`send_kept`, `send_kept_every`: a
  resync's replay, the relays to a resynced or loading game) is packed
  as any relay (the test looks at the table's lifetime, not the
  sender's). From 2026-10-14 (f3c93e1) to 2026-10-15 kept relays went at
  once, one per datagram: to a resynced game at 20 players some 100 to
  200 datagrams a second, more than its acks of the newest 6 seqs cover
  (match.md §30), so its replay was never all acked;
- a part of a split message may go;
- the queue fills a datagram;
- a client datagram has waited `ack_delay` (20 ms) for its ack: the ack
  goes on the messages, not on a bare `FC` in front of them.

Resends ride on the next such datagram. The client reads a relay in a
multi datagram with the same handler as a lone one (§3.1), live games
already acked multi datagrams of 20 to 60 messages, and one bad message
drops the whole datagram, so every message still passes the table checks
before it is queued. With a server that looks every 5 ms a relay waits
15 to 20 ms at most; a link then carries some 50 to 70 datagrams a second
plus the pings, clock messages and acks, whatever the number of players.

### 11.4 Two messages in one datagram; acks of copies (added 2026-10-14)

`send_joined(first, second)` queues two whole messages that go in one
datagram, `first` right ahead of `second`: the packer takes them together
or not at all (a datagram that has no room for both closes before the
first). The client handles the messages of a datagram in order (§2.3 step
7), and a datagram lost loses both. The match server sends a 0xE7 of
offset 0 ahead of a 0xE5 so while a clock correction may still run
(match.md §29): alone after the 0xE5 it would be wrong. Both are send-once
ids, so neither goes again by itself.

A message is acked by an ack of any datagram it went in; the server
remembers the newest 32 of them (`SEQS_PER_MESSAGE`; 8, the client's own
ring for its messages, until 2026-10-14). A kept relay goes again every
50 ms, so with 8 a round trip of 400 ms and more left every ack naming a
copy already forgotten, and the message was never acked. 32 cover 1.6 s.

### 11.5 Whether a message is delivered, in O(1) (added 2026-10-15)

`is_delivered(serial)` walked the whole queue (pending and unacked). The
match server asks it for every entry of a resync's gate and of
`check_in_flight` on every poll: O(queue × entries), with hundreds of each
at 20 players. The transport now counts the queue entries of each serial
(`outstanding`; the parts of a split message one each), kept on every
send, ack, send-once, expiry, `forget` and disconnect, and the answer is a
map lookup. A test walks the queue after every step and compares.

### 11.2 A late datagram of ours from the lap before (added 2026-10-14)

Live, 2026-10-14: a datagram of ours arrived some 810 ms late, after more
than 128 later ones (§2.3 step 6). The client took it for its newest and
from then on echoed its seq as `rseq`, which read here as 1 to 128 *past*
our newest seq: a seq not sent yet. Every ack was ignored, data stopped at
100 datagrams past the newest acked one, only `FC` went for 1.5 s, and then
1.5 s old relays went out.

Now, when the client's newest datagram (one older than another already
received never counts: a late client datagram carries an old `rseq`)
echoes a seq `a` (1..=128) past our newest, and that seq is one we sent a
lap before which is older than the newest the client had acked, our seq
skips `a` ahead: the next datagram is the echoed seq plus one, which the
client takes as newer by one (it has no gap check). Its window still holds
the datagrams it had before the late one, at the distance the skip gives
them, so its acks name our datagrams by the right numbers again. The
skipped numbers have no datagram: their acks give no round trip, and the
fast resend counts from the newest acked datagram actually sent. Each skip
is counted (`Transport::skips`), logged per player and noted in the packet
log. A tool that numbers our datagrams must unwrap their seq bytes (a
match recording stores every datagram whole, seq included), not count
them.

Not skipped: a client that heard nothing from us for 128 and more of our
datagrams also echoes a seq that reads as ahead, but its true newest; its
window holds older datagrams under the same seqs, and a skip would take
them as acks of ours. Data waits for it as before.

Left as is: when we send `a` or more datagrams between the late one's
arrival and its echo reaching us, ours under the echoed seq is dropped by
the client (§2.3 step 3) and the echo reads as its ack: its messages count
as delivered. A message that also went in another datagram the client had
is not lost by that; one that went only there is lost unreported. With a
round trip of some 100 ms and 50 to 70 datagrams a second (§11.1) that
needs the late datagram to come more than some 245 of ours late.

### 11.3 Lifetimes counted from the send (added 2026-10-14)

A message with a limited lifetime ends that long after its **first send**
(§4, as the client does). In a stall nothing is sent, so nothing ends: a
relay queued during 1.5 s without acks went out 1.5 s old. And 4 desyncs of
2026-10-14 were a relay whose every copy was lost within its 250 ms on a
line losing 30 to 60 %, while the game's history (500 ms) could still have
used a later copy.

`send_until(id, payload, deadline)` sets the end when the message is
queued; until then it goes again at the table's pace (50 ms for the short
ids). A message not sent by its deadline is lost unsent; one that went
lives at least the table's lifetime from its first send (the end is
`max(first send + 250 ms, deadline)`), so a deadline never gives fewer
copies than before. The match server uses it for:

- relays to a playing game (EB, EC): the tick's time + 500 ms on the match
  clock, less half the game's round trip (50 ms at least), so the last
  copy still arrives while the game's history has the tick. One that
  arrives later is moved to a tick of the game's own choosing (match.md
  section 5.1). The round trip is the median of the game's latest 32
  samples, not the last one: on a spiky line one long sample would cut
  every relay of the next second to the 250 ms floor. For an input in
  time that is 250 to 450 ms instead of 250 ms from the first send.
- drop orders to the input's owner (F1, F2 and EF, F0 from the input
  handler): 450 ms after they are queued. The owner's world time is not
  known here (its inputs are late because its clock or its line is), and
  a late one is refused quietly (match.md section 5.2).

Relays to a game that is loading a world stay kept until acked (its world
starts behind the inputs it gets meanwhile), and the drops and roster
changes of a sync keep the table's 250 ms from the first send. A relay or
drop order that ends unacked still marks its game as out of step from its
tick (`check_in_flight`), now some 200 ms later than before.

