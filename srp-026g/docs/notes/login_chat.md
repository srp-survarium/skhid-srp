# 0.26g vs 0.21d: login, browser, sessions/presence, chat (messaging)

**Version scope:** client **0.26g** (build 2777, "0.26g0", compiled 2015-01-14, exe `downloads/v026g/extracted/Survarium/game/binaries/x86/survarium.exe`, no PDB) compared with client **0.21d** (build 2010, April 2014, with PDB). Every statement below is about 0.26g relative to 0.21d. Addresses are VAs with image base 0x10000 in both builds.

Tags:
- **PROVEN**: read in the 0.26g disassembly or data. The 0.26g VA and/or string is given, plus the matching 0.21d symbol or VA.
- **LIKELY**: strong inference from code shape, not followed to the last instruction or not live-tested.
- **GUESS**: plausible, but weakly supported.

0.21d baseline: `srp-021d/docs/notes/login.md`, `lobby.md` §4, `lobby_spec.md` §1.1, §1.5 and §5.

How 0.26g functions were found without a PDB: through the assert/log strings that survive in the exe. Examples are `vostok::network::login_client_impl::on_sign_in_answer_received` at 0xa22118 (referenced from 0x37fe5b) and `messaging_client received unknown message:%d` at 0xa07154. Call structure and constants were then matched against the named 0.21d functions.

---

## 0. TL;DR

| Area | 0.21d | 0.26g | srp impact |
|---|---|---|---|
| Command-line key for the login server | `-client=host:port`; with no key, falls back to `188.93.23.27:65000` | **`-server=host:port`**. No fallback: "Game server hasn't been specified. Please, start the game from the launcher." | Launch with `-server=` |
| Browser server (HTTP `/hello`) | used, port 80 | **gone**. No `http_client` in the exe. | browser-server unused |
| Sign-in step 1 (plain) | `01 len email ver[12]="0.21d" eula u32 queue` | **same layout**, ver = `"0.26g0"` | version string only |
| Step 2 (plain, 1 byte) | `0B` → TLS | same | none |
| TLS | OpenSSL 1.0.0g, verify against `../../resources/ssl/survarium_login_server.crt` | **OpenSSL 1.0.1h**. Same file path, same "accept preverified" callback. | Copy srp's cert over the shipped 0.26g cert (different key, expired 2015) |
| Step 3 (TLS, password) | `len pw` | **`02 digest[40]`** (remembered login) or **`03 digest[40] len pw`** (typed). digest = lowercase hex SHA-1(password). | **Must change** |
| Reply 0x08 | `08 L1 browser L2 query u32 session` + 12 bytes (lobby and messaging ip:port) | **`08 u32 session u32 lobby_ip u16 lobby_port`** (11 bytes) | **Must change** |
| Login reply enum | 0x08..0x16, invalid 0x1F | renumbered from 0x11 up: ban reasons 0x12..0x1F, then 0x20..0x25; invalid 0x2D | update constants |
| Presence ping after sign-in | UDP `[03][u32 session]` every 10 s to the login port | **removed**. Only the online-queue ping remains: `[03][u32 queue_id]` (was `[04]`). | srp presence listener gets nothing |
| Sign-out | TCP `[02][u32 session]`, TLS, `len pw` | **no network traffic**. The client just closes and resets locally. | handle_sign_out unused |
| Messaging (chat) transport | its own TCP connection (browser type 4), sign-in `C3`/`CA` | **carried inside the lobby connection**. No messaging sign-in. | **rewrite of the transport layer** |
| Chat message ids | c→s C1..C6, s→c C9/CB/CC | c→s **4C..50**, s→c **A2..A4** | renumber |
| Chat layouts | — | text, friend list and important message gain a second string (clan tag); subscription gains a 10th u32 and a language byte; new channel 9; clan system messages; new important-message types; the client never queries its friend or ignore list | changes listed in §5 |

---

## 1. Finding the servers, command line

**PROVEN.** The login-server key is now named **`server`**, with help text "connect to server" (strings 0x9fa590 and 0x9fa57c). The key object is at 0xa9c550 (name pointer at +0x210 = 0xa9c760; read at 0x1eae4d `mov ecx,0xa9c550; call 0xddeb0`). The 0.21d key was `client` (0x809088, `s_net_login_client`, set up at 0x68C0EA). The string `client` still exists in 0.26g (0xa2aaa8), but only OpenSSL's purpose table and UDP code use it. No key uses it.

**PROVEN.** In 0.26g `game::on_base_resources_created` (the function around 0x1ea9f0..0x1eb2c1, log "create network client" 0x9fa8b4 at 0x1eaf13), these are checked in order:
1. If `-server` is set and its value contains `':'` (0x1eae96 `strchr(...,':')`), the network client is created with that value. This is the same logic as 0.21d 0x1528D0.
2. Otherwise, if the replay key (0xa9c9b0) is set, a replay client is created.
3. **Otherwise the game stops** with "Game server hasn't been specified.\r\nPlease, start the game from the launcher." (0x9fa990, pushed at 0x1eb25b).

The hardcoded `188.93.23.27:65000` (0.21d 0x809450) **does not exist** in 0.26g. There is therefore no default login port. The port is whatever `-server` gives.

**PROVEN.** `-spectator` ("connect as spectator", 0x9fa598/0x9fa5b0) is still present.

**PROVEN.** These are new: config/command-line variables `account_name`, `account_password` and `account_digest` (0xa21968, 0xa21978, 0xa2198c). Command-line keys `account_name` and `account_password` (key objects 0xaa4910 and 0xaa4b40) copy into globals 0x1f1da48 and 0x1f1dac8 (`login_client` ctor 0x3762d0). The persistent `account_digest` lives in 0x1f1db48. So `-account_name=... -account_password=...` can pre-fill the login. The launcher normally passes them **(LIKELY)**.

Use: `survarium.exe -server=<srp host>:<login port>` from `game\binaries\x86` (the CWD matters for the cert path, §3).

---

## 2. Browser server: removed

**PROVEN.** 0.26g contains:
- no `vostok::network::http_client` or `network_core::http_client` class name (0.21d has 16 RTTI or log strings with `http_client`; 0.26g has 0);
- no `%s&type=%d&local_ip=%s&login_ip=%s&ver=%s` format string (0.21d 0x80d320);
- no `on_http_result_ready` or `on_http_error`.

The lobby address now comes straight from the login reply 0x08 (§4.1). `network_client` 0x21bd90 does `sprintf("%s:%d", login_impl+0x298 /*ip string*/, login_impl+0x2a8 /*u16 port*/)`, using `%s:%d` at 0x9ffa1c. It then calls 0x21bde0. That connects the lobby with the session id from login_impl+0x188 (0x21bdef), through 0x26f650.

There is no messaging address at all, because messaging rides inside the lobby connection (§5.1).

**srp:** the unused `servers/browser-server` crate has been removed from this workspace; the 0.21d copy remains in `srp-021d/servers/browser-server`. The 0.26g login reply carries the lobby address directly.

---

## 3. TLS and certificate

- **PROVEN.** OpenSSL is now **1.0.1h (5 Jun 2014)** (string 0x8b3f90 and others), replacing 1.0.0g. TLS 1.1 and 1.2 are available to the client.
- **PROVEN.** Verify file path unchanged: `../../resources/ssl/survarium_login_server.crt` (0xa21e98). It is loaded by `load_verify_file` in the `login_client_impl` ctor (0x37c179).
- **PROVEN.** The context sets verify_peer (0x37c1e3 `push 1`). The callback is 0x37c2b0 `mov al,[esp+4]; ret 8`, which returns `preverified`. This is identical to 0.21d `verify_ssl_certificate` 0x23B2A5. There is no host-name check.
- **LIKELY.** The method is still SSLv23 (negotiating). I did not trace the asio `context` method constant.
- **PROVEN** (file comparison). The shipped 0.26g `game/resources/ssl/survarium_login_server.crt` (md5 a502dd5a…) is **a different certificate and key** from 0.21d's:
  - serial CABBABA253259C7D;
  - valid 2014-05-09 to **2015-05-09** (expired);
  - public key md5 1af7fcc9… The 0.21d original and srp's re-issue both have 88d60605….
  - The subject is the same "Internet Widgits" dummy.
- **LIKELY (works as for 0.21d).** Because the client trusts whatever is in that file and accepts any chain that verifies against it, overwrite the 0.26g file with srp's `certs/survarium_login_server.crt` (md5 a62e1639…, already in the 0.21d install). Our private key matches that cert; we do not have the key for the 0.26g one.
- **PROVEN.** New client-side error text: "game: SSL certificate verification failed" (0x9ff578). A bad cert now shows a specific message.

---

## 4. Login protocol, byte for byte

### Function map

| 0.21d (PDB) | 0.21d VA | 0.26g VA | evidence |
|---|---|---|---|
| `login_client_impl::sign_in` | 0x23E87D | 0x380940 | copies sign_in_info into +0x256 host[64], +0x296 port, +0x198 email[100], +0x1fc pw[48], **+0x22c digest[41]**, +0x255 eula |
| `sign_in_on_connected` | 0x23E759 | 0x3807D0 | builds step 1, error code 0x2D |
| `on_sign_in_written` | 0x23E5D6 | 0x380590 | string 0xa225e0; reads 1 byte (0x380775) |
| `on_user_name_answer_received` | 0x23E430 | 0x380340 | string 0xa22480 |
| `on_sign_in_handshaked` | 0x23E34F | 0x380160 | builds step 3 |
| `on_sign_in_password_written` | 0x23E1CE | 0x37FF20 | string 0xa222e8; read 0x52 (0x38010d) |
| `on_sign_in_answer_received` | 0x23DCE1 | 0x37FB10 | string 0xa22118 |
| `schedule_ping` / `ping` | 0x245D3D / 0x245DA3 | 0x38AAC0 / 0x38AB30 | 10 s timer 0x989680 |
| `start_ping_answer_acception` / `process_ping_answer` | 0x245E51 / 0x245E8F | 0x38ABB0 / 0x38AC00 | string 0xa22d10 `process_ping_online_queue_answer` |
| `verify_ssl_certificate` | 0x23B2A5 | 0x37C2B0 | |
| `login_client::sign_in` (wrapper) | — | ~0x3767BE..0x376AB6 | string 0xa21a0c |
| `login_client::sign_out` (wrapper) | — | 0x376BB0 | string 0xa21a34 |
| `login_client_impl::sign_out` | 0x241A0D | 0x385860 | no network I/O (§4.6) |
| `network_client::on_connected_to_login` | 0x16D2C6 | 0x21A6C0 | string 0x9ff668 |
| `login_menu::set_status` | — | jump table at 0x21f2a4 (code ~0x21EED0) | status strings |

The sign-in information is no longer pointer-based. 0.26g `sign_in` takes (host[64] + u16 port) and a block of { email[100] @0, password[48] @0x64, digest[41] @0x94, eula u8 @0xbd } **PROVEN** (0x380940).

### 4.1 Step 1, client → server (plain TCP): unchanged layout

**PROVEN** (0x3807D0..0x3808A0):

```
01 | u8 n | email[n] | version[12] = "0.26g0" + 6 zero bytes | u8 connect_from_eula_wnd | u32 LE online_queue_order_id
```

- The version is copied as 4+2+1 bytes from 0x9dae98 (`"0.26g0"`) into a zeroed 12-byte field (0x380847..0x380889).
- **srp's client patch** (since 2026-10-12, `client_patch.md` §6) puts its revision into byte 7 of the field, past the text's zero: `0.26g0 00 NN 00 00 00 00`. The login server reads it (`patch_revision`) and writes it in the log; an exe without that patch says 0.
- Total 19 + n, capped at 0xA7 (0x3808dd), the same as 0.21d.
- The email buffer is still 100 bytes (`sign_in` 0x3809a1 `cmp esi,0x64`).
- The queue id is login_impl+0x18c; it is −1 unless the client is rejoining the queue (ctor 0x37c126).

The 0.21d failure code for this step was 0x1F; it is now 0x2D (0x3807fe).

### 4.2 Step 2, server → client (plain, 1 byte): unchanged

**PROVEN** (0x3804f3):
- `0x0B` → TLS handshake.
- `0x0A` → callback with 0x0A (invalid user or password).
- Anything else → callback with that byte.

### 4.3 Step 3, client → server (TLS): **changed**

**PROVEN** (`on_sign_in_handshaked` 0x380160..0x3802FE). It compares the stored password (+0x1fc) with `"******"` (0x9fff00):

| case | bytes |
|---|---|
| password field == `"******"` (the login screen used the remembered account) | `02` + 40 ASCII hex chars of digest (+0x22c) = **41 bytes** |
| otherwise (password typed) | `03` + 40 hex digest + `u8 len` + password bytes (≤ 47) |

0.21d sent only `u8 len + password` (0x23E393).

**What the digest is** (`login_client::sign_in` wrapper 0x37687D..0x3769B3, **PROVEN** except where marked):
- `0x376120` is SHA-1. It uses the init constants 67452301 / EFCDAB89 / 98BADCFE / 10325476 / C3D2E1F0 (0x37613a), with update at 0x375a90 and final at 0x375f10.
- `0x3761f0` formats the 5 hash words with `"%08x"` (0xa21960) into 40 lowercase hex characters.
- `0x376230` parses them back.
- **Typed password:** digest_sent = hex(SHA1(password bytes)) (0x376957 → 0x376967 into login_client+0x109, which becomes sign_in_info+0x94). The plaintext is sent as well.
- **Remembered:** the client stores `account_digest` = hex(SHA1(password) XOR machine_key). machine_key = SHA1 of the first volume GUID name from `FindFirstVolumeA` (import 0x8702a8), or of the fallback string `d59265a4-7a87-43b4-a72d-54130a940063` (0xa21938) when that fails (0x376180). On a remembered login the client un-XORs it (0x376919..0x37692D) and sends the same `hex(SHA1(password))` with no plaintext.
- **PROVEN** (disassembly, 2026-10-12). The output is the standard SHA-1 lowercase hex digest. The words are boost-style SHA-1 H0..H4, each printed with `%08x` (0x3761F0). The block function 0x375AE0 reads big-endian words and uses the standard constants; the final 0x375F10 pads with `80`, zeros and the bit count big-endian. The input is the password's bytes up to its zero (0x376941..0x376957).

**Consequence for srp:** a returning user (password `******`) gives the server **only the SHA-1 hex**. The server must be able to authenticate from `sha1_hex(password)` alone. srp stores argon2id of the plaintext, so it should store argon2id(sha1_hex(password)) instead, or as well. On a `03` sign-in, the plaintext is available to verify or migrate the old hash.

### 4.4 Step 4, server → client (TLS): read size unchanged, layout **changed**

**PROVEN.** The client reads one `async_read_some` of up to **0x52 = 82** bytes into login_impl+0x2aa (0x38010d). This is unchanged.

Dispatch in `on_sign_in_answer_received` 0x37FB10 is on byte 0:

- **0x08 connection_info** (0x37FD26) **PROVEN**:
  ```
  08 | u32 LE session_id | u32 lobby_ip | u16 LE lobby_port        (11 bytes)
  ```
  - The session goes to +0x188.
  - The IP is passed through the delay-loaded import [0xaefff4] (byte swap, htonl/ntohl), then `address_v4::to_string` (0x35ee0), into the string at +0x298.
  - The port goes to +0x2a8 raw, with no ntohs.
  - Then state = 3, and the callback runs with 8.
  - **No UDP ping socket is connected and no ping is scheduled** (contrast 0.21d 0x23DFC9..0x23E0AD).
  - The IP encoding is the same pattern as 0.21d's lobby IP in the 0x08 tail: the wire carries `u32::from(ip).to_le_bytes()` (127.0.0.1 = `01 00 00 7F`). **PROVEN** (2026-10-12): the import at [0xaefff4] is WS2_32 ordinal 8 (`htonl`, from the delay-import table), and its result is the `in_addr` given to `inet_ntop(AF_INET)` (0x35EE0 -> 0x35E40 -> 0x35D40).
  - There is **no browser address, no initial query and no messaging address**.
  - `network_client` then connects the lobby to `ip:port` (§2). The lobby is a UDP `udp_match_connection`-style link in 0.26g; see the lobby notes.
- **0x25 added_to_online_queue** (0x37FB7F) **PROVEN**:
  - Layout `25 | 1 byte skipped | u32 queue_id | u32 place | u32 total`, the same as 0.21d's 0x16.
  - The client resolves UDP `<login host>:<m_host_port>` (itoa of +0x296, 0x37fbcf), starts the 10 s queue ping and listens for answers.
- **Any other byte** → callback with that byte. For mapping, see §4.5.

### 4.5 Login server message ids (0.26g)

**PROVEN** from `network_client::on_connected_to_login` 0x21A6C0. That function does `switch(code-8)` over the table at 0x21bd04 (0x1E entries) and calls `login_menu::set_status(n)` (0x1ed260). The status texts come from the set_status table at 0x21f2a4. The names are the 0.21d PDB names where the role matches, and otherwise the client's own log strings.

| id 0.26g | id 0.21d | meaning (client string) | client status → UI |
|---|---|---|---|
| 0x08 | 0x08 | connection_info | switch_to_lobby (0x21abff) |
| 0x0A | 0x0A | "sign in: invalid user name or password" | 1 `st_login_menu_status_invalid_user_or_password` |
| 0x0B | 0x0B | valid user name (step 2 only) | — |
| 0x0C | 0x0C | "sign in: attempt interval is violated" | 2 |
| 0x09, 0x0D..0x11 | 0x09, 0x0D..0x11 | not handled by the UI (default → status 0 "error connection") | 0 |
| **0x12..0x1F** | (0x11 `user_banned` had no reason) | **"sign in: ban_reason_X"**, in this order: bad_language, insult, faschism, politics, flood, advertising, discuss_admins, reallife_treating, account_trading, incorrect_info, administrator, nickname, cheating, teamkilling | 0x0C..0x19 → `st_login_menu_status_user_banned` + `st_ban_reason_X` |
| 0x20 | 0x12 | "user access level restriction" | 6 |
| 0x21 | 0x13 | "user already signed in" | 8 `…sign_in_already_online` |
| 0x22 | 0x14 | "invalid version" | 9 `…invalid_version` |
| **0x23** | — (new) | "sign in: user unconfirned registration" | 7 `…user_not_confirmed` |
| 0x24 | 0x15 | EULA check failed | 0x0A → `root.show_eula_wnd` |
| 0x25 | 0x16 | added to online queue | 0x0B → `update_online_queue` |
| 0x26..0x2C | — | **unknown**: not referenced by the client | — |
| 0x2D | 0x1F | invalid_message_type (the client's own failure code, 0x3807fe) | — |

**LIKELY.** Ids 0x0D..0x11 keep their 0.21d meanings (sign_out_successful, occupied_user_name, send_sign_up_info, sign_up_successful, user_banned). Nothing in the client reads them.

**srp mapping:**

| srp constant | 0.26g id |
|---|---|
| `INVALID_USER_NAME_OR_PASSWORD` | 0x0A, unchanged |
| `TOO_MANY_ATTEMPTS` | 0x0C, unchanged |
| `REGISTRATION_CLOSED` (access level) | 0x12 → **0x20** |
| `USER_BANNED` | 0x11 → **one of 0x12..0x1F**. 0x11 now shows a plain connection error. Use 0x1E (cheating) or 0x1C (administrator); 0x1B is incorrect_info (checked against the tables 0x21bd04 and 0x21f2a4: code N shows status N − 6). The client shows `st_login_menu_status_user_banned` then `st_ban_reason_X`, joined with no space. Stock texts: the header names prohibited software ("You are banned (Use / distribution / advertisement of prohibited software).") for every reason, and 0x1C is "Pretending to be administrator of the game"; `client-patches/localization/*.tsv` makes them "You are banned. " and "Decision of the administration." (srp's fallback for a reason with no keyword). |
| "already online" | 0x13 → **0x21** |
| invalid version | 0x14 → **0x22** |

### 4.6 Sign-out: no network traffic

**PROVEN.**
- `login_client::sign_out` (0x376BB0) posts an order to the impl. Its functor is 0x385860.
- 0x385860 sets state = 1, closes the connection (0x37c340) and calls the callback with code 8. No socket is written.
- The 0.21d source file `login_client_impl_sign_out.cpp` and its four functions (0x2415EE..0x241A0D, and the `on_sign_out_*` strings) are gone from 0.26g.

**srp:** `handle_sign_out` is never reached. Sessions must end some other way, for example when the lobby connection drops or times out.

### 4.7 Presence and queue pings

**PROVEN** (`ping` 0x38AB30):
- The client sends only when state == 2 (queued) and queue_id != −1.
- Payload: `[0x03][u32 LE queue_id]` (5 bytes), to UDP `<login ip>:<login port>`, every 10 s (0x38aaea `0x989680`).
- The 0.21d `ping_online` `[03][u32 session]` in state 3 (0.21d 0x245DE7..0x245E03) **no longer exists**.
- The queue ping id changed from 0x04 to 0x03.
- Answer: a 9-byte datagram `[?][u32 place][u32 total]` (0x38ac5e `cmp esi,9`). Unchanged.

**srp:** `run_presence` / `hear_ping` will get nothing from a signed-in 0.26g client. Worse, a 0.26g queue ping `[03][u32 queue_id]` looks exactly like a 0.21d online ping `[03][u32 session]`. srp never sends 0x25, so this does not arise unless the queue is implemented.

### 4.8 Login reply sizes and limits

- **PROVEN.** Reply buffer: 82 bytes. A 0x08 reply is 11 bytes.
- **PROVEN.** Password: ≤ 47 characters (48-byte buffer, 0x3809dc). Digest buffer: 41 bytes (0x380a13).

---

## 5. Messaging (chat)

### 5.1 Transport: inside the lobby connection

**PROVEN.**
- The 0.21d messaging connection is gone. These strings are absent from 0.26g: `messaging_client::on_error`, `.\messaging_client_sign_in.cpp`, `messaging_client::sign_in_on_packet_received` and `tcp_packet_client`.
- 0.26g `messaging_client::process_incomming_packet(const unsigned char, buffer_reader&)` (string 0xa07188, code 0x26ABE0) receives the message id as a separate argument.
- Its only caller is the lobby dispatcher `network_client` 0x274A00. That function does: `if ((u8)(id + 0x5E) <= 2)`, i.e. id in 0xA2..0xA4, pass to messaging (0x274a14..0x274a2b); otherwise `switch(id-0x80)` over the lobby table at 0x275138.
- Sends go through the lobby connection object (network_client+8): `0x37a0e0(conn, id)` allocates a message, `0x379640` sends it.
- If the lobby connection state is not 0 or 4, typing shows "not connected to messaging server..." (0xa07240) as a `System` line (0x26af62).
- **There is no messaging sign-in.** The 0.21d `C3 <session> 05` / `CA <name>` exchange does not exist. The lobby sign-in carries the session.
- After the lobby sign-in the client sends the subscription (0x4F) itself (call at 0x26fbe2, inside the lobby sign-in handler around 0x26FA00).
- **LIKELY.** The chat name the 0.21d client got from `CA` now comes from lobby data. Not traced.

The framing of these messages is the lobby transport's, which is not TCP `tcp_packet` any more. See the lobby/transport notes. Below, "id" is the message id byte that the lobby transport delivers.

### 5.2 Id map

| direction | 0.21d id | 0.26g id | name (0.26g net-stats row strings 0x9ff100.., 0x9ff278..) | 0.26g code |
|---|---|---|---|---|
| c→s | 0xC1 | **0x4C** | message | `on_message_typed` 0x26AD30 (push 0x4c at 0x26aeb8) |
| c→s | 0xC2 | **0x4D** | important message | 0x26AFB0 (4 fields), 0x26B0A0 (5 fields) |
| c→s | 0xC3 | — | (sign_in_info) **removed** | — |
| c→s | 0xC4 | **0x4E** | friendship_action | 0x26B3E0, 0x26B470, 0x26B510, 0x26B5C0, 0x26B660 |
| c→s | 0xC5 | **0x4F** | subscription | 0x26C320 |
| c→s | 0xC6 | **0x50** | important_message_read | `read_important_message` 0x26B1B0 (push 0x50 at 0x26b2a7) |
| s→c | 0xC9 | **0xA2** | message | `process_incoming_text_message` 0x26B6F0 |
| s→c | 0xCA | — | (sign_in_info) **removed** | — |
| s→c | 0xCB | **0xA3** | friendship status | inline in 0x26ABE0 (0x26acce) |
| s→c | 0xCC | **0xA4** | important message | 0x26BBF0 |

The client→server ids 0x4C..0x50 sit right after the lobby client ids. The send-stats rows 0x20..0x24 (registration 0x2187FA..0x218866) list them in this order. **PROVEN** for 0x4C..0x50 from the `push imm8` before each `call 0x37a0e0` or `0x378080`.

### 5.3 c→s 0x4C message (text): layout unchanged

**PROVEN** (0x26AEB5..0x26AF50):

```
4C | u32 channel_id | string receiver | u8 channel | string body (≤255)
```

This is the same as 0.21d 0xC1 (0x19B0FF). The channel_id per channel comes from the jump table at 0x26af8c, indexed by channel−1:

| channel | channel_id | note |
|---|---|---|
| 1 general | messaging+0x50 (language channel, §5.6) | |
| 2 | 0 | |
| 3 **clan** | messaging+0x58 (clan id) | **new**: not sent if 0. 0.21d never sent clan. |
| 4 private | 0 | |
| 5 match | match_id (network_client+0x1e57a4) | not sent if −1 |
| 6/7 team | match_id; the channel byte becomes 6 + (team ≠ 0) | as in 0.21d |
| 8 squad | messaging+0x6c (squad id) | not sent if 0 |
| **9 (new)** | messaging+0x70 | not sent if 0. Set from the lobby clan message (§5.6). Meaning unknown. |

**PROVEN.** The channel enum now has **10 values (0..9)**. 0.21d had 9.

### 5.4 c→s 0x4D important message

**PROVEN.** There are two forms:
- 4 fields (0x26AFB0): `4D | u32 0 | string target_name | u8 type | string text`. This is the same as 0.21d 0xC2. Used for type 0 (add friend; callers 0x26b3cc, 0x28c1e5), type 2 (squad invite, 0x2a5cad) and type 3 (squad accepted, 0x28c698).
- 5 fields (0x26B0A0): `4D | u32 0 | string target_name | u8 type | string text | u32 extra`. **New**, used for type 7 (clan invite, caller 0x275f77) and type 10 (clan team request, caller 0x2760d5).
- **GUESS.** `extra` is the clan or team id.

The server can tell the forms apart by type (7 and 10 carry the extra u32) or by the remaining length.

### 5.5 c→s 0x4E friendship_action

**PROVEN** action bytes and bodies:
- `02 u32 id` remove_friend (0x26B470)
- `03 u32 id` add_ignorable (0x26B510)
- `04 string name` add_ignorable_by_name (0x26B3E0)
- `05 u32 id` remove_ignorable (0x26B5C0)
- `06 string name` find_players (0x26B660)

These numbers and bodies match 0.21d. Adding a friend still goes through 0x4D type 0.

**PROVEN.** **The client never sends actions 7, 8 or 9** (query friend list, query ignore list, update statuses). 0.21d sent `C4 07` and `C4 08` right after messaging sign-in, and `C4 07` again after each `'4'` result. The five call sites above are the only `push 0x4E` in the client; no generic sender takes 0x4E. **LIKELY:** the server must push 0xA3 07, 0xA3 08 and 0xA3 09 unsolicited, after the lobby sign-in and after every change.

### 5.6 c→s 0x4F subscription: **changed**

**PROVEN** (0x26C320):

```
4F | u32[10] from messaging+0x4c (40 bytes, memcpy) | u8 language
```

0.21d sent 9 × u32 with no language byte.

Slot meanings:
- [0] = 0
- [1] = general channel id. The ctor 0x26C170 sets it to **1 when the localization is `russian` or `ukrainian`, else 2**. 0.21d used 2 only for `english`.
- [2] = 0xFFFFFFFF
- [3] = **clan id** (set from the lobby clan message: 0x2721fc and 0x27273a, values from network_client+0x1ec98c)
- [4] = 1
- [5] = match_id
- [6] = team-1 slot, [7] = team-2 slot (`assign_match_channel_order` 0x26C2E0, called from 0x274aed and 0x274b18)
- [8] = squad id (0x2747b4)
- [9] = **new** channel-9 id (0x272d45, from network_client+0x1ee5c0, inside the lobby handler 0x271FD0 for lobby id 0x9A)

The trailing language byte (0x26c36e..0x26c47f) is mapped from the localization string: russian 1, english 0, ukrainian 2, german 3, polish 4, anything else 1.

The subscription is re-sent whenever a slot changes. The first one goes right after the lobby sign-in (0x26fbe2).

**srp (the owner's rule, 2026-10-12): one general room for every language, as on 0.21d.** The room id of slot 1 and the language byte are stored and not used to choose who gets a general line; `A2` carries no room id, so the client does not split the rooms either. Do not "fix" this (`social.md` §9.6).

### 5.7 c→s 0x50 important_message_read

**PROVEN** (0x26B1B0): `50 | u32 message_id | u8 answer` (0 accept, 1 decline). Same as 0.21d 0xC6.

### 5.8 s→c 0xA2 message (text): **changed**

**PROVEN** (0x26B6F0):

```
A2 | u8 sender_type | u32 sender_id | string sender_name (≤63)
   | [ string sender_tag (≤15: a 16-byte stack buffer, social.md §1.2)  — ONLY when sender_type == 5 ]   ← new
   | u8 channel | string body
```

- The ignore-list drop happens before the name is read: if sender_type == 5 and the id is ignored, nothing more is read (0x26b72d..0x26b74e). This is the same as 0.21d.
- **LIKELY:** sender_tag is the clan tag (§5.9 shows "[%s] %s").
- The tag is passed to the chat line with the name (0x26b8ff..0x26b916, `add_message(channel, body, name, tag)`).

Handling by sender_type:

| sender_type | channel | effect | evidence |
|---|---|---|---|
| 8 (stats/system) | 3 | **clan system message** (0x26B940): body `"<digit>\x0E<args>"`. 0 = `st_clan_member_joined`, 1 = `st_clan_member_left`, 2 = `st_clan_member_kicked`, 3 = `st_clan_level_up_1/2`. Sender shown as `st_clan_system_message_sender`. | PROVEN codes and strings; argument format GUESS |
| 9 | 3 | same clan system message | PROVEN (0x26b8aa..0x26b8c8) |
| 8 | 4 | `lobby_menu::on_additional_drop_arrived(body)` (0x225950, log "query drop: %s"): parses the body and sends lobby queries | PROVEN; body format not traced |
| 8 | other | if the body contains `#mstats`: `on_stats_message_arrived` (0x225810) parses `[N]` and queries match stats N, as in 0.21d. Otherwise dropped. | PROVEN |
| other | any | censor option, then the chat line | PROVEN |

### 5.9 s→c 0xA3 friendship status: **changed**

**PROVEN.** Dispatch in 0x26ABE0 at 0x26acce: `u8 action`, then:

| action | body | 0.26g reader | change vs 0.21d |
|---|---|---|---|
| 7 friend list | `u16 n`, n × { u32 id, string name (≤63), **string tag (≤15)**, u8 status } | 0x26C540 (entry 0x5c bytes: id +4, name +8, tag +0x48 [16 bytes], status +0x58) | **+ tag string** |
| 8 ignore list | `u16 n`, n × { u32 id, string name } | 0x26C620 | same |
| 9 statuses | `u16 n`, n × { u32 id, u8 status }. n must equal the friend count, **otherwise silently ignored** (0.21d re-queried) | 0x26C4A0 | mismatch handling changed |
| **anything else** (0..6) | **`u16 n`, n × { u32 id, string name, string tag }**, read as a found-players list into messaging+0x2c | 0x26C6C0 | **0.21d read `u8 result` for 0..5.** In 0.26g any such reply is parsed as a list. |

Then `lobby_menu::on_friendship_status_received(action)` (0x21d990) runs.

**Warning (PROVEN).** The client copies strings with `memcpy(len)` and no bounds check (0x26c5bc, 0x26c5da). The friend-entry tag field is **16 bytes**: keep tags ≤ 15. Names must be ≤ 63.

**srp:**
- Do not send `u8 result` replies. srp's current `CB <action 0..5> '4'` would be misread as a list with n = 0x??34.
- Reply to find (6) with a found list that includes tags.
- After add/remove, push a fresh 7 or 8 list.

### 5.10 s→c 0xA4 important message: **changed**

**PROVEN** (0x26BBF0):

```
A4 | u8 sender_type | u32 sender_id | string sender_name (≤63) | string sender_tag (≤15, social.md §1.4)  ← new
   | u8 type | string text | u32 message_id
```

A message_id that is already pending is ignored. Otherwise `lobby_menu::important_message_arrived(id, type, text, name, tag)` (0x226020) runs.

Auto-handling in messaging (jump table at 0x26bda0, types 1..5):
- 1 quest, 4 reputation unlocked, 5 drop: auto `50 id 00`.
- 3 squad accepted: auto `50 id 00`, then add squad member (0x2745d0).
- 2 squad invite: waits for the user.

Types known to `important_message_arrived` 0x226020 (**PROVEN**, `cmp bl,N` and log strings):

| type | meaning | evidence |
|---|---|---|
| 0 | friend request (`st_add_to_friends_request`, shows `"[tag] name"` via `"[%s] %s"` 0x9fb140) | 0x226e86.. |
| 1 | quest (`#+q`, `#!q`, `#q!`) | 0x226045 |
| 2 | squad invite (`st_add_to_squad_request`) | 0x226ef9 |
| 3 | squad invite accepted | messaging 0x26bd74 |
| 4 | reputation level unlocked (`#ru`) | 0x2264ba |
| 5 | drop arrived | 0x226785 |
| **6** | **item repaired (`#ir`)** | 0x226848 (new) |
| **7** | **clan invite (`st_add_to_clan_request`, text `%s "%s"`)** | 0x226f77 (new) |
| **10** | **clan team request (`st_add_to_clan_team_request`)** | 0x226369 (new) |
| **13** | **referral reward (reward, reward_type, …)** | 0x226953 (new) |
| **14** | **skill point reward (skill_level, reward_id, reward_amount)** | 0x226b5b (new) |
| 8, 9, 11, 12 | not handled | — |

The text formats for the new types (key/value parsing with `id`, `level`, `faction_id`, `nickname`, `reward…`) were not traced: **GUESS** that they are in the same `#tag` style.

### 5.11 Stats notices

**What the sender is told (added 2026-10-12).** The client's chat has its own spam filter and prints its own notice (0x2083F0); a name that is no player is answered by srp with `84 49 00 st_player_not_found`, the one text the game has for it and the one message that shows it. Details and the proof: `social.md` §9.

**PROVEN.** `#mstats` (0x9c9e08) still triggers the match-stats query (§5.8). The 0.100b tag notices (`#pc`, `#mc`, `#q`, `#e`, …) were already dead in 0.21d. 0.26g adds `#ir` and keeps `#+q`/`#!q`/`#q!`/`#ru`, but these appear only inside **important messages**, not chat. The new sender_type 8 / channel 4 "additional drop" message and the clan system messages are listed above.

---

## 6. What srp-021d must change for 0.26g

| # | Area | Status vs 0.21d | srp file and function | Change |
|---|---|---|---|---|
| 1 | Launch | changed | docs, bootstrap scripts | `-server=host:1234`, not `-client=` |
| 2 | Browser server | removed | `servers/browser-server` | not needed for 0.26g |
| 3 | Sign-in parse | same layout | `login-server/src/lib.rs` `sign_in_length`, `parse_sign_in` | accept version `"0.26g0"` (12-byte field as in 0.21d) |
| 4 | Password message | **changed** | `password_length`, `handle_sign_in` | read `u8 kind`: `02` + 40 hex, or `03` + 40 hex + `u8 len` + pw |
| 5 | Credential check | **changed** | `session` crate (`password.rs`, account repository) | verify by `sha1_hex(password)`; store argon2id(sha1_hex), migrating when plaintext arrives (`03`) |
| 6 | Reply 0x08 | **changed** | `handle_sign_in`, `lobby_addresses` | send `08 u32 session u32 ip_le u16 port`, where ip:port is the **0.26g lobby** (UDP) address. Drop the browser address and query. |
| 7 | Reply codes | renumbered | constants near `INVALID_USER_NAME_OR_PASSWORD` | banned → 0x12..0x1F with a reason; access level 0x20; already online 0x21; invalid version 0x22; unconfirmed 0x23 |
| 8 | Presence UDP | removed | `run_presence`, `hear_ping` | not needed. Session liveness must come from the lobby link. |
| 9 | Sign-out | removed | `handle_sign_out` | unused; end sessions on lobby disconnect |
| 10 | TLS cert | changed file | install step | copy `certs/survarium_login_server.crt` over `game/resources/ssl/survarium_login_server.crt` |
| 11 | Messaging transport | **rewritten** | `lobby-server/src/messaging_server/mod.rs` (the TCP accept, sign-in, `send`), the first-byte sniffing in the lobby | serve chat inside the 0.26g lobby connection: ids 0x4C..0x50 in, 0xA2..0xA4 out; no 0xC3/0xCA |
| 12 | Message ids | renumbered | `messaging_server/message.rs` `parse_frame`, `ServerMessage::serialize` | C1→4C, C2→4D, C4→4E, C5→4F, C6→50; C9→A2, CB→A3, CC→A4 |
| 13 | Subscription | changed | `message.rs` (`[u32; 9]`), `mod.rs` `subscribe` | 10 × u32 + u8 language; general id 1 for ru/ua and 2 otherwise; slots 3 (clan) and 9 (new) |
| 14 | Text 0xA2 | changed | `ServerMessage` text serializer | add `string tag` after the name when sender_type == 5 |
| 15 | Friends 0xA3 | changed | `friendship_reply`, `write_accounts` | add a tag string in lists 7 and 6; never send `u8 result`; push 7, 8 and 9 unsolicited (after lobby sign-in and after changes) |
| 16 | Important 0xA4 | changed | `important_message` | add `string tag` after the sender name; parse the 5-field 0x4D for types 7 and 10 |
| 17 | Channels | +1 | `receives` | channel 9; clan channel 3 now usable |

---

## 7. Open questions

1. What do login ids 0x26..0x2C mean? The client never references them, so they are not needed.
2. The exact framing and transport of the lobby connection that now carries chat (UDP `udp_match_connection`-style). This belongs to the lobby/transport study, which must also say whether chat rides a reliable ordered channel.
3. Where does the client get its own chat name now that `CA` is gone? Probably the lobby sign-in or base info.
4. What is channel 9? Its id is messaging+0x70, set from lobby message 0x9A (handler 0x271FD0, network_client+0x1ee5c0); clan-related (GUESS: clan team or alliance).
5. Meaning of the extra u32 in the 5-field 0x4D (clan or team id, GUESS), and the text formats of important-message types 6, 7, 10, 13 and 14.
6. Arguments of the clan system message after `\x0E`, and the body format of the sender_type 8 / channel 4 "additional drop" notice.
7. TLS method constant not traced (LIKELY SSLv23). It needs a live handshake test with srp's OpenSSL against OpenSSL 1.0.1h.
8. **Answered (2026-10-12, PROVEN):** the other uses of `"0.26g0"` (0x9dae98; all four references to the string are these and §4.1's) are not network uses:
   - 0x12395: `sprintf("%s (%d)", version, build)` (format 0x9daea0) in preinitialize.
   - 0x1ea69b: copied into a 7-byte buffer for the title `"%s v%s - Copyright© VOSTOK GAMES. …"` (0x9fa814).
   - 0x1f0647: compared with the version in a replay's header; another one stops with "Replay of other engine version is used" (0x9faedc).
   - A second copy of the string stands at 0xa711b4, after `survarium_version_header_mark!!` (0xa71194), with no reference in the code: for a program that reads the exe (the launcher has the same mark).

   So the login step-1 field is the only thing a client tells a server about its build: the lobby sign-in `2E` is the session and one flag (lobby.md, "Sign-in `2E`"), the match connect `D8` the session and the debug byte (match.md §1). No message carries a hash or a checksum of the exe or of its files (the `DF` hash is the world's). What this means for telling client patches apart: `client_patch.md` §6.
9. Does srp's IP encoding in 0x08 match the client byte-for-byte? It is the same code pattern as 0.21d. Needs a live test.
10. The launcher (`game/launcher`) is what passes `-server=` (and possibly `-account_name` / `-account_password`). Its config was not examined.
