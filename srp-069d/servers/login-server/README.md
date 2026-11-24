# login-server (0.69d)

TCP login of the 0.69d client: framing, sign-in `01`/`02`, answers `0B`, `0C`, `0D`, RSA-1024 PKCS#1 v1.5.
The exchange is proven with the real client; the protocol is described in `docs/notes/login.md`.

- `frame.rs`: `u8 len` (1..255) + payload, or `00` + `u32 LE len` + payload; the server answers in the 5-byte form.
- `handshake.rs`: state machine without IO (`Handshake::on_frame`), `Authenticator` trait. The account name is
  parsed from the sign-in request and handed to the authenticator and to the caller in `Outcome`.
- `keys.rs`: the key. The private key path comes from the srp config (`files.login_key`; PEM, PKCS#1 or PKCS#8); the real key is
  never in the repository. Tests generate a throwaway key (`LoginKey::generate_for_tests`).
- `lib.rs`: `handle_connection` / `serve` (thread per connection).

## Proven facts used
- `01` sign-in: `u8 signin_type, str account, str "0.69d0", str "r1", u32, u32 crc32(DER of the key the client holds,
  0 if none), u8 FF, u32 0B, rest` (the rest holds the client's own base64 public key). Strings: `u8 len` or
  `FF u16 len`. The CRC field = CRC-32 of the DER in the server's `0D` (checked on a real-client capture, `tests/golden.rs`).
- `0D` request_secure_data: `u8 password_type (2), u8 0, u32 140, DER RSAPublicKey, u32 challenge`. The client
  buffer is 160 bytes, so the key must be RSA-1024 (a 2048-bit key fails: "send_sign_in_secure_data failed").
- `02` secure data: `u32 128` + one RSA PKCS#1 v1.5 block. Plaintext (67 bytes in the captures): 20 bytes, `01 02`,
  string digest (40 hex), `u32` challenge echoed from `0D`. The server checks the challenge.
- `0B` success: `u8 0, u32 session, u32 lobby ip, u16 lobby port`; the ip is the u32 LE of the host-order value
  (127.0.0.1 = `01 00 00 7F`). Golden frame reproduced byte for byte.
- `0C` failed: reader at 0x20011F140 reads `u8 reason`, and for reasons 2 and 3 `u8` + `u32` more.

## Open
- Which `0C` reason code the client shows for which cause is not traced; `Refusal::BAD_CREDENTIALS = 1` is a placeholder.
- Meaning of the 20 plaintext prefix bytes and of the first plaintext byte pair is not traced (prefix ignored).
- `0E` (digest, 41 bytes) and the other answers `0F..17` are not implemented.
- Tests cover the exchange with a scripted client; the real client against this server is a manual test.
