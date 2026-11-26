# protocol (0.69d)

Message payload encode/decode for the lobby connection of the 0.69d client. Not the datagram transport
(`udp-transport`): every function here works on the bytes after `id x` (the plain class, AC, has no `x`).

- `wire`: `Reader` / `Writer` (little endian, `u8` / `FF u16` strings, bounds checked), traits `Encode` / `Decode`
  (`to_bytes`, strict `from_bytes`).
- `items`: item record (n1 modifiers, n2, n3 decals, n4 installed modules), profile, account body, level fields.
- `s2c`: server to client. `S2c::decode(id, payload)` / `S2c::encode_payload()` / `S2c::to_wire(x)`, `S2c::class()`
  (ordered / unique / plain, `ids::s2c_class`). Pushes 80 85 86 87 88 89 8A 8B 8C 8D 8E 8F 90 91 92 94 95 96 97 9A (no
  clan; the other forms: `Clan9A::from_bytes`) 9B 9C 9D 9E 9F A4 A5 A8 AC AE AF B0 (subs 4..7) B1 B2 B3 B4 B5 B6 B8 BB,
  replies `83` (2F 30 45 46 48 4B 55 5B 60) and `84`.
- `c2s`: client to server, `Request::decode(id, payload)` / `Request::encode()`: 2E 2F 31 32 33 34 35 37 38 39 3A
  45 46 47 (00 learn, 01 reset) 48 49 4A 4B 4C 4D 4E 4F 50 51 53 55 56 57 58 5A 5B 5C 5E 5F 60. Everything else is `Request::Other`.
- `lzma2`: raw LZMA2 as the client's packed readers take it (89, 8A, `83 5B`). Decoding uses `lzma-rs`; encoding writes
  stored chunks (valid for the client's decoder, larger than compressed output).

Each layout carries PROVEN (client address, or seen working in the real client / in client logs) or LIKELY in its doc
comment. Not modelled: the AI block of a player record (`AccountBody.ai_id != 0xFFFFFFFF`,
belongs to the match-options builder), the unpacked `83 5B` blob (carried as bytes), `83 4A`, `62 xx` custom matches
(the lobby reads them from `Request::Other`).

## Tests

`tests/vectors/*.json` hold golden payloads: messages of an earlier implementation that worked with the real client
(pushes, replies, results, social), synthetic samples, and the C->S requests of client logs (the hardware report 0x50
is not stored). None of them holds game data: the 8A vectors are synthetic price rows, the bot names in the results are
made up.

- `tests/golden.rs`: every vector decodes into a typed message and re-encodes to the same bytes (packed messages: the same
  message after a second decode); every request of the real logs parses.
- `tests/units.rs`: wire primitives, limits, LZMA2 over a chunk boundary, framing.

Run: `scripts/cargo-069d-tree-v4.sh test -p protocol -- --test-threads=2` (inside WSL / Linux; see the script header).
