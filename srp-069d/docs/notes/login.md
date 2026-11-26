# Login server: normal sign-in and Steam mode (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

The login server (`servers/login-server`) answers the client's TCP sign-in. srp checks the account (`AccountAuth` in
`servers/srp/src/lib.rs`): attempt lockout, bans, client version, auto-register of a new name, "already online". Client
facts are 0.69d exe addresses. Nothing here has a setting: both paths are always on.

## 1. Normal sign-in (no `-steam`)

```
C->S 01   u8 method (1), str account, str "0.69d0", str "r1", u32, u32 key CRC, u8 FF, u32 0B, ...
S->C 0D   u8 password_type (1), u8 0, u32 len, DER public key, u32 challenge
C->S 02   u32 n, RSA block: 20 bytes, u8 method, u8 type, str digest, u32 challenge
S->C 0B   u8 0, u32 session, u32 lobby ip, u16 lobby port        (or 0C u8 code [, u8 reason, u32 minutes])
```

The digest is SHA-1 of the password as 40 lowercase hex digits (0x200122BD0, `%08x` x 5). The client hashes the bytes
of its password field as they are (no case or encoding change), and that field keeps at most 47 bytes
(0x200126FE9..0x200127010). A remembered login ("remember me") sends type byte 2 and the stored digest; a typed password
sends type byte 1 and the fresh digest. Both are the same SHA-1 hex, so the server stores `Argon2(digest)` and never sees
the password. The connection may sit silent for 30 s.

## 2. Steam mode (`-steam`)

A client started with `-steam` signs in with method 2 and shows Steam login screens (no name and password form). The
Steam calls of the client (achievements among them) run only in this mode (`login_client+0x610 == 2`). The server signs
such a client in with its **own account name and password**, typed in the client's Steam "Import" panel. The SteamID
and the Steam ticket are never used to grant anything.

```
C->S 01   u8 2, u64 SteamID64 (in place of the account string), the rest as above
S->C 10 04                     the client opens the Import panel; status "invalid user or password" for 4 s
C->S 04   u32 n, n ticket bytes, u8 2, str name, str password      (Import button; password in plain text)
S->C 0B                        granted: the session goes on to the lobby as usual
S->C 10 04                     wrong name or password: the panel again (the client clears the password field)
S->C 0C ...                    other refusals (ban, suspension, lockout, version, already online): the text on the panel
C->S 04   u32 n, ticket, u8 1, u8 0A, str Steam name               ("Create new", no password): answered 10 04
```

* `01` method 2 carries 8 raw bytes (the SteamID64, 0x20011F5DE) where method 1 has the account string. The server answers
  `10 04` at once and asks for nothing else (no `0D`, no ticket request).
* `04` sub 2: the server takes the name as the normal form would send it (at most 99 bytes) and computes the digest the
  normal client would send for that password (SHA-1 lowercase hex of at most 47 bytes), then runs the same account check.
  So one account signs in through both paths with the same password, and a new name is registered by its first Import
  exactly like a first normal sign-in.
* The connection stays open while the player types (5 minutes of silence allowed in this state only; 30 s everywhere
  else) and after every refusal: the client sends `04` on the same connection and never reconnects for it
  (0x200126DDF..0x200126DF2), and neither `10` nor `0C` makes it disconnect (0x20011FBA7, 0x20011F140).
* The password travels in clear in `04` (the client writes the field text raw, 0x20011DF30). The normal path never
  sends it. This cannot change without a client change.
* There is no "remember me" in Steam mode: the password is typed at every start.
* The claimed SteamID64 is stored per account in the table `steam_ids` (account, SteamID64, unix second), replaced at
  every successful Steam-mode sign-in. It is information only: the client sends it unverified, and nothing reads it to
  decide a sign-in. The table is created (IF NOT EXISTS) on every database open, so existing database files get it.
* Logs: `steam sign-in (method 2) claiming SteamID64 <id>` for `01`; `steam import sub <n> of "<name>" ...: granted /
  wrong name or password / refused` for `04`; `claimed SteamID64 <id> stored` after a granted Steam-mode sign-in.

How players start the client in Steam mode from the Steam Play button (or Launch Options / a shortcut): `steam.md`.

## 3. Open points

* The client's own sign-in timeout text (`st_sign_in_timed_out`) was not traced to a timer; whether it can fire while
  the Import panel waits is not known.
* What the fixed byte `0A` in the "Create new" `04` means (only parsed).
* Steam stat limits and the live achievement test with a Steam account are still to do.
