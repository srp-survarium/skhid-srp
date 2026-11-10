# The sign-in cipher (patch 139)

The password step of a 0.26g sign-in goes through srp's own cipher. The game's
protocol had a TLS handshake there (OpenSSL 1.0.1h of 2014, a self-signed
certificate; `login_chat.md` section 3).

Why: the owner's decision of 2026-11-10. Some Russian providers block the
server; a handshake of ours has no decade-old TLS fingerprint to recognise.
It does nothing against a block of the server's address, and lobby and
matches (UDP) are as they were.

## What is said

- The plain steps are unchanged: `01` + user name + version, answered with one
  byte; `0B` (user name valid) starts the cipher.
- Game to server: its hello, an X25519 public key new for this sign-in (32
  bytes).
- Server to game: its hello, the same (32 bytes).
- Both derive two keys, one per direction: HKDF-SHA-256 with the salt
  SHA-256(`skhid sign-in cipher 1` | S | game's hello | server's hello) over
  DH(game's key, S) | DH(game's key, server's key), info `keys`. S is the
  server's lasting public key, which the game is built with.
- Then records, either way: u16 LE length, the message sealed with
  ChaCha20-Poly1305 (the nonce the record's number in its direction, the
  length authenticated), its 16-byte tag. A message is at most 255 bytes.
- The messages are the game's own: the password message (`02`/`03`, digest,
  password) and the server's answer, each one record.

This is Noise's NK pattern. Only the holder of S's secret half derives the
keys, so only srp reads the password; a recorded sign-in sent again opens
nothing, the server's hello being new each time.

## Where it is

- `crates/sign-in-cipher`: the cipher, pure Rust without dependencies (the game
  links it): X25519, ChaCha20-Poly1305, SHA-256/HMAC/HKDF with their RFCs'
  test vectors; `SKHID`, the server's public key; `game`, the game's side as
  a machine answering OpenSSL's calls.
- `servers/login-server/src/sealed.rs`: the server's side over the TCP
  stream, and `ask`, the game's side for srp's own clients (bots, tests).
- The server's secret key is no file of the repository (whoever holds it
  can pass for the server): srp takes it from `SRP_SIGN_IN_KEY`, 64 hex
  characters, and does not start without it. The live host's and the test
  environment's `.env` hold it (the compose file passes it on); on the
  owner's machine it is `~/.config/skhid/sign_in_cipher.key`, which
  `scripts/local-clients/run.py` hands a local server. A server with
  another key says so at its start, with the public half: only a game built
  with that half signs in to it (`srp-bot`: `SRP_BOT_SERVER_KEY`). The
  tests and their bots use a key of their own (`sealed::test_key`).
- The game, patch 139 (`client-patches/rust/patches/src/sign_in_cipher.rs`,
  `vostok/openssl.rs`): the game's sign-in code is untouched. Its stream
  (boost.asio's SSL engine, one for the game's run) reaches OpenSSL through
  `SSL_connect`, `SSL_read`, `SSL_write` and `SSL_get_error`, and moves the
  bytes itself through a BIO pair; the patch answers those four calls after
  their prologue (`openssl.*` in `sites_026g.tsv`) with the machine, which
  reads and writes the SSL's BIOs. `sign_in.cipher_begin` (0x380504, the user
  name taken) starts a sign-in's cipher, whatever an abandoned one left. A
  failed call queues an OpenSSL error, so the game shows its own
  connection failure. The game still loads its certificate file at start;
  the file stays in the players' download.

## One version

A game without patch 139 cannot sign in to this server, and a game with it
cannot sign in to an older one. The release that ships both sets the
server's minimum patch revision to its own, as every release does, so an
older game is told "outdated version" at the plain step and updates.

## Checked

- `cargo test -p sign-in-cipher -p login-server -p srp` (the RFC vectors, the
  handshake's refusals, the machine, sign-ins end to end with srp's bots).
- `client-patches/check_sign_in_cipher_026g.py` (Unicorn and Python's
  `cryptography`): the patched exe's four functions run a sign-in against
  that library's X25519, HKDF and ChaCha20-Poly1305 (a key made for the
  run, put where the exe holds `SKHID`), and each failure fails with one
  queued error.
- Played 2026-11-10: two games (Wine) signed in to a server on another
  machine over the internet, registered their accounts and played a match.
  No Windows game and no player behind a blocking provider has tried it.
