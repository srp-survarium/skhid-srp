# Captured sessions

`captured_session_N_login.jsonl` and `captured_session_N_lobby.jsonl` (N = 1..7) are seven sign-in + lobby sessions of
the real 0.69d client against an earlier reference server, one JSON record per TCP frame (login) or UDP datagram
(lobby). Readers: `crates/udp-transport/tests/golden.rs` (lobby), `servers/login-server/tests/golden.rs` and
`steam.rs`, `servers/srp/tests/handover.rs` (login).

Records (`t` = seconds since the recording server started, `hex` = the bytes on the wire):

```text
lobby  {"n", "t", "dir": "c2s"|"s2c", "hex",
        "msgs": [{"id", "x" (null for plain / low-level ids), "len", "payload"}]   c2s: the recording server's decode
        "pushed": {"id", "cls": "o"|"u", "x", "len"}     s2c: a push ends in this datagram
        "resend_of": <seq>                                s2c: this datagram is a resend
        "notes": [text]}                                  s2c: the recording server's own log lines
login  {"n", "t", "dir": "c2s"|"s2c", "kind": "raw" | "0x0D" | "0x0B", "hex"}  one TCP frame
       {"n", "t", "dir": "note", "kind": "rsa_plain", "offset", "hex"}          the decrypted secure-data block
```

The message order, ids, timing, lengths and the server side are as captured. Values that identified the test PC or
its account were replaced by test values of the same byte length, so no length field, frame length or multipart
split changed:

- Login: the account name is `test@example.com`. The client's RSA public key in the sign-in (`01`) and the server's
  RSA public key in `0D` are **test keys** generated for these files (their private halves are not published); the
  sign-in's key CRC field is the CRC-32 of that test server key, the relation the real capture showed. In the
  secure-data plaintext the 20 random prefix bytes are random test bytes and the stored password digest is the SHA-1
  hex of `test-password`; the RSA block of the secure-data frame is that plaintext encrypted (PKCS#1 v1.5) with the
  test server key.
- Lobby: the client's hardware report (`50`) holds generic values: a made-up Windows build, "Generic x86-64 CPU",
  "Generic GPU" adapters with test PCI ids, zeroed driver versions, random CPU feature bytes and round memory numbers.
  The server never stores this report.
