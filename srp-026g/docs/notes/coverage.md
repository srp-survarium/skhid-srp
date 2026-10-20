# 0.26g coverage: what the client can send, and what srp does with it

Audit of 2026-10-12. Read-only: no code was changed. Client side: the 0.26g exe (build 2777), enumerated with `tools/re-kit` (`callers.py`, `dis.py` and small scans of the same kind). Server side: the code in this tree as it stood during the audit (other agents were editing it).

Handling classes:
- **real**: handled with rules and stored state; the client gets the answer it expects.
- **fixed answer**: answered with constant data.
- **accepted, no effect**: parsed, nothing done.
- **not parsed**: the server does not know the message.
- **refused on purpose**: answered with a refusal text.
- **being changed today**: another agent is changing this today; not judged here.

Confidence: **high** means I read the parser, the dispatch arm and the handler. **medium** means I read the dispatch arm and the handler's header, but not every rule under it in `crates/session/src/v026g/` (`rules.rs`, `items_rules.rs`, `clans.rs`, `premium.rs`, `quests.rs`). No row was tested against a live client.

---

## 1. Summary

### 1.1 Counts

114 client → server messages (each lobby sub-operation is one row).

| class | login | lobby | match | transport | total |
|---|---|---|---|---|---|
| real | 3 | 80 | 9 | 6 | **98** |
| being changed today | 0 | 9 | 0 | 0 | **9** |
| accepted, no effect | 0 | 3 | 1 | 0 | **4** |
| fixed answer | 0 | 1 | 0 | 0 | **1** |
| refused on purpose | 0 | 1 | 0 | 0 | **1** |
| not parsed | 1 | 0 | 0 | 0 | **1** |

The web page takes no message from the game client (section 8).

### 1.2 Client list against the notes

- **In the client and in the notes:** all 114 rows.
- **In the client but missing from the notes:** none. Every writer found in the exe has a row in `lobby_requests.md`, `social.md`, `login_chat.md`, `match.md` or `transport.md`.
- **In the notes but no writer in the client:**
  - `social.md` §1.4 says a "reputation level" notice (type 4) makes the client query `0x3A`. The exe has no writer for `3A`. The two calls there (0x2265F6, 0x226606) send `40` and `39`, as `lobby_requests.md` §3 says. The `social.md` line is wrong. (Corrected in `social.md` on 2026-10-12.)
  - `checkbox_tournament_match_clicked` (callback 41, 0x2A2EC0): no request writer is reached from it. The notes already say so.
  - Transport `FD`: the client has no writer for it (it only reads it). The server accepts one anyway.
- **Small gap in one note:** `lobby_requests.md` §3 lists the generic sender 0x26FE20 for `3C 00`. There is a second, inlined writer at 0x2719A7 (after the `97` results reader). `results.md` §1.6 has it.

### 1.3 How the client list was made, and what it can miss

- **Lobby and chat.** A message is built by `construct_packet` 0x378080. It has 5 direct callers and no jump or pointer to it anywhere in the image:
  - 0x37A102, inside the lobby factory 0x37A0E0;
  - 0x26B2AC (`50`) and 0x2719A7 (`3C`), two inlined copies of the factory;
  - 0x3F2C83 (the match port probes `D9`) and 0x3FD0E7 (the transport's multipart split).
- The factory 0x37A0E0 has **85 direct call sites**, one jump (0x279FE6, the match client's "create message" virtual) and no pointer. All 85 were resolved to an id, and to a sub-operation byte where the id has one. 84 push a constant id. One (0x26FE2D, inside the generic query sender 0x26FE20) takes the id in `al`; that sender has **12 call sites**, all with a constant (`37`, `39`, `3C`, `3E`, `40`, `43`).
- The send function 0x379640 has 86 call sites and one tail jump (0x2707A6). That is 84 factory sites (all but `2E`, which goes out through `connect` 0x379090), the two inlined ones, and the match client's send virtual (0x27A2B0). The numbers agree.
- `connect` 0x379090 has 2 callers: the lobby (`2E`, 0x26F886) and the match (`D8`, 0x27A247).
- **UI callbacks.** The name table at 0x9C56C8 has 112 names (0..111; 8 damage-type names follow). The delegates are registered by 110 calls of 0x283610 (0x281787..0x283179); the name index is the slot number (slot offset / 0x20). Each handler was scanned one call deep for request writers. That scan agrees with the triggers in the notes; where a handler reaches its writer through a longer path the notes' trigger is quoted.
- **Match.** Messages are built through the virtual at vtable 0xA08864 +0x18 (the thunk 0x279FE0 into the factory). A scan for "load vtable+0x18, push a constant, call" finds 8 sites with ids `D8..E1`. `D9` is built inside `set_connection_ports` (0x3F2C83). `DC`/`DD` use a computed id (`0xDC + flag`, 0x27EB24).
- **Login** has its own TCP/TLS path (0x3807D0, 0x380160) and one UDP ping (0x38AB30). These were taken from `login_chat.md` and the function map there; the writers were not searched for again.
- **Transport control.** `new_low_level_packet` 0x3FBDB0 has 5 callers. They push type 0 (`FA`), 1 (`FB`) or 2 (`FC`); none pushes 3 (`FD`).

What could still be missed:
- An id computed at run time other than the two found (`0x26FE20`'s `al`, and `0xDC + flag`). A scan for non-constant pushes before the match virtual was not done.
- A message built by code that calls the factory through a register. No pointer to the factory or to `construct_packet` exists in the image, so this is unlikely.
- The callback → writer map is one call deep and its function ends are guessed from padding. A callback that reaches a writer through three or more calls would show as "none". This does not affect the list of messages, only the "UI action" column.
- The SWF ActionScript was not read again. The lobby cannot send anything without going through one of the 112 callbacks.
- The patched client's snapshot upload (`E0` with a payload) is not in the stock exe. The server reads it (`match_msgs.rs`, `parse_client`).

---

## 2. Everything that is not "real", most visible first

| # | message | class | what a player notices |
|---|---|---|---|
| 1 | `49 05` comment about a player (report or praise) | refused on purpose | The comment window shows the error `st_error_during_storing_user_comment`. Nothing is stored. |
| 2 | `2F` Play, and the queue state `85` it is answered with | being changed today | Not judged. As read: average wait is always 0, the 20 search properties are all 0, the match level is 0, the map is drawn at random from the settings. |
| 3 | `47 00`, `47 02`, `47 05`, `47 06` (buying, premium, bundles) | being changed today | Not judged. The handlers exist and store state; sellers, ranks and prices are in flux. |
| 4 | `42 00` drop query | being changed today | Not judged. As read: logged, no answer. The client asks only after a chat notice srp never sends, so nothing hangs. |
| 5 | `43 00` daily bonus, `46 03` drop card | being changed today | Not judged. Both are answered (`99`, `98`). |
| 6 | `49 0C` pay into a clan registration | being changed today | Not judged (the cost is a clan default). The handler takes silver and stores it. |
| 7 | `45` tutorial video link | fixed answer | Always `9E` with "hide" and an empty address. The video link never appears in the lobby. |
| 8 | `49 24` hide the video link | accepted, no effect | Nothing: the link is never shown, so the button cannot be reached. |
| 9 | `32` after sign-in | accepted, no effect | Nothing. What it asks for is not known. The client waits for no answer. |
| 10 | `46 0A` the tutorial's "choose a weapon" step | **accepted, not answered** since 2026-10-19 | No tutorial (`no-tutorial.md`): no client sends it. It was answered with `89`, `87`, `8B` from 2026-10-12 (`premium.md` §5.1). |
| 11 | `E1` match keep-alive | accepted, no effect | Nothing. It only keeps the link alive, which the transport does on its own. |
| 12 | login UDP `03 <queue id>` queue ping | not parsed | Nothing. The client sends it only after the login answer `25` ("in the online queue"), which srp never sends. The 0.26g stack has no UDP listener on the login port. |

### 2.1 Gaps inside rows that are "real"

These are handled with real rules, but a player can run into an edge that gets no answer.

**2026-10-12, after the audit:** the rows marked **fixed** or **settled** below were worked through against the client (`social.md` §9 has the proof for each). "Settled" means the client was shown to have no way to tell its player, so the server stays silent on purpose.

| message | edge | what a player notices |
|---|---|---|
| `3D` player of the results screen, `41` results again | The match's report is no longer kept (srp restarted, or the report aged out) | **Fixed 2026-10-12.** A report is now kept while a screen of it may be open (the last 64, and any whose screen an account was sent in the last two hours), so a click on a row of an open screen is answered. A restart is no gap: no client's lobby link, so no results screen, outlives srp. `41` is sent only after a chat notice srp never sends. (`social.md` §9.5) |
| `4C` private line | The named player is offline or unknown | **Fixed 2026-10-12.** The sender gets the game's "Player not found." in a message box (`84 49 00 st_player_not_found`), once per name in 10 s. The game has no "offline" text. A player who ignores the sender is not given away: still nothing. (`social.md` §9.4) |
| `4C` any line | More than 10 lines at once, then more than one a second | **Settled 2026-10-12.** The game's spam notice is printed by the client's own filter (0x2083F0: the third line in two seconds on the general, match and team channels mutes for 10 or 5 s, and the line is not sent). That filter is stricter than srp's limit, so a stock client is told by itself before srp would drop a line (tested with the filter as code). The server can not make the client print the notice; on the private, squad and clan channels the client has no filter and the game no text, so a line over the limit there is still dropped in silence. (`social.md` §9.3) |
| `4C` channel 0, 2 or unknown | — | Dropped. The stock UI does not type on these. |
| `4D` friend request | The name is unknown, already a friend, or the target ignores the sender | **Fixed 2026-10-12** for an unknown name: "Player not found." (`84 49 00 st_player_not_found`). Already a friend, already asked, or ignored: still nothing (the game has no text for these, and an ignore is not given away). |
| `4D` squad invite | The target is not in the lobby | **Fixed 2026-10-12.** "Player not found." for a name that is no player in the lobby. A target that ignores the sender: still nothing. |
| `4A` squad actions | Refused (not the commander, not invited) | **Settled 2026-10-12: nothing can be shown.** The client returns at once on `84 4A` (0x21DEA8), `83` has no case for `4A`, and the localization has no text for any squad refusal. The squad is sent again as it is. (`social.md` §9.2, §9.4) |
| `4F` subscription | The language byte is not used | **The owner's rule (2026-10-12), not a gap:** the general channel is one room for all languages, as on 0.21d. Checked end to end and tested with clients of four languages. (`social.md` §9.6) |
| `50` answer to a server notice | — | Ignored on purpose; only requests are kept. |
| `34` squad mate | The account is not a squad mate | No answer. |
| login step 1 | The EULA byte and the queue id are logged and not used | Nothing. |
| `D8` match connect | The debug byte is only logged | Nothing. |

---

## 3. Server → client: what the client can read and srp never sends

| message | what stays dead |
|---|---|
| login `25` added to the online queue | The login queue screen. srp has no queue. |
| login `24` EULA check failed | The EULA window is never forced. |
| login `23` unconfirmed registration, `21` already signed in | These two login texts never show. A second sign-in ends the old link with `AA` instead, which shows the "already online" text on the old client. |
| `A2` from the stats sender with `#mstats [N]` | The writer exists (`lobby/social.rs`, `match_stats_notice`) and nothing calls it. Results are pushed directly as `97`, so no feature is lost. |
| `A2` from the stats sender on channel 4 (drop notice), `A4` type 5 (drop arrived) | "A drop arrived" outside the results screen. This is the path that makes the client send `42`/`43`. Being changed today. |
| `A4` type 13 referral reward | The referral reward window. srp has no referrals. |
| `9E` with "show" and an address | The tutorial video link. |
| `A7` | Nothing: the client reads and ignores it. |
| match `ED`/`EE` (many players' inputs in one message) | Nothing visible. Inputs go one by one (`EB`/`EC`). |
| match `F6` (hash mismatch dump) | Nothing: debug only. |
| transport `FE` | Must never be sent (the client crashes). It is not. |

Everything else in the client's lobby table (`80`, `82`..`85`, `87`..`9F`, `A1`..`A6`, `A8`..`AA`) has a caller in the server.

---

## 4. Placeholder data inside real messages

| message | field | value | visible? |
|---|---|---|---|
| `85` in queue | average wait seconds | 0 | LIKELY yes, if the queue window shows an average. Being changed today (`play.rs`). |
| `85` in queue | 20 search properties | all 0 | Not known. Being changed today. |
| `85` for a ready player in a squad that is not ready | place 1 of 1 | constant | Barely. |
| `82` connect to match | match session (was misread as a game time) | the session the seat is kept by | Sent since 2026-10-12; before it srp sent 0 and a real client was turned away by the match server. |
| match setup | match level | 0 | The client only logs it. Being changed today. |
| `80` signed in | state 0, flag false | constant | No. |
| `9F` online stats | minutes to maintenance | one day when none is set | No (the warning shows under 60). |
| `9E` | show, address | false, empty | Yes: no video link (section 2, row 7). |
| `9A` type 0, a registration | `reg_state` | always 3 | No: the name was checked when reserved. |
| `9C` | fourth byte | 0 | No: the client never reads it. |
| `A3` friend status | "away" | never sent; only lobby, in game, offline | Barely. |
| `A8` premium offer after a match | first u32 | 0 | Not known. Prices are being changed today. |

Not checked field by field: the results screen `97`, the player details `93`, the account body `87`. Their writers are in `rewards.rs` and `wire.rs`; `rewards.rs` is being changed today.

---

## 5. Login (TCP 1234, then TLS)

Server file: `servers/login-server/src/v026g.rs`.

| message | what sends it | client writer at | notes | handling | where | conf. |
|---|---|---|---|---|---|---|
| step 1: `01`, name, version, EULA byte, queue id | the login button | 0x3807D0 | in notes | **real**: the version must be `0.26g0`; the name starts a sign-in. The EULA byte and the queue id are logged only. | `handle_sign_in` | high |
| step 3: `02` + SHA-1 hex (remembered account) | the same, password field left as `******` | 0x380160 | in notes | **real**: checked against the stored argon2id of the digest | `parse_password`, `handle_sign_in` | high |
| step 3: `03` + SHA-1 hex + password (typed) | the same, password typed | 0x380160 | in notes | **real**: the same; a first sign-in registers the account | the same | high |
| UDP `03 <queue id>` | every 10 s while in the login queue | 0x38AB30 | in notes | **not parsed**: no listener; never sent because `25` never is | — | medium |

Refusals use the client's own codes: `0A` wrong name or password, `0C` too many attempts, `12..1F` a ban with its reason, `20` registration closed, `22` wrong version (`refusal_code`, `ban_code`). There is no sign-out message in 0.26g; a session ends with its lobby link.

---

## 6. Lobby (UDP 1235)

Parser: `crates/survarium/src/v026g/lobby/requests.rs`, `parse`. It reads all 94 rows below. A message that cannot be read is logged (`core.rs`, `datagram`); for `46`, `47` and `48` the client is also sent a refusal so its window does not wait (`core.rs`, `unreadable`). The connection is never dropped for a bad message. Extra bytes after a message are logged and the message is handled.

Dispatch: `servers/lobby-server/src/v026g/dispatch.rs`, `Core::handle`. "Writer" is the client function, then the address of the call that creates the message.

### 6.1 Sign-in, Play, queries

| id | UI action | client writer at | handling | where | conf. |
|---|---|---|---|---|---|
| `2E` | connecting to the lobby after login | 0x26F650 / 0x26F69F | **real**: the session must be one the login server made; then the whole account is pushed | `core.rs` `first_datagram`, `sign_in` | high |
| `2F` | Play (`play_button_clicked`) | 0x26FD80 / 0x26FD8D | **being changed today** | `play.rs` `play` | — |
| `30` | Play in clan-match mode | 0x26FDD0 / 0x26FDDD | **real**: kit and team checked, `83 30`, or `84 30` with a text | `clans.rs` `clan_play` | high |
| `31` | Play again while queued; closing a match | 0x2717E0 / 0x2717F4 | **real**: out of the queue (squad and clan team too), `85` state 0; after a match also the results | `play.rs` `leave` | high |
| `32` | automatic, after `80` | 0x26FA00 / 0x26FBEE | **accepted, no effect** | `dispatch.rs` `handle` | high |
| `34` | automatic, for each squad mate shown | 0x26FE70 / 0x26FE7E | **real**: `88` with the mate's active kit | `social.rs` `squad_member` | high |
| `37` | automatic, after a level or referral notice | 0x26FE20 (0x226B39, 0x226D49) | **real**: `8C` | `dispatch.rs` | high |
| `39` | automatic, after a quest or reputation notice | 0x26FE20 (0x22625F, 0x226606) | **real**: `8E` | `dispatch.rs` | high |
| `3B` | the ranking page (`request_players_range`) | 0x270070 / 0x27007E | **real**: `91` from the stored ratings; with start 0 also `8F` and `90`. srp's own, asked only by its changed lobby movie: start 2 000 000 000 is answered with the matches now in a `91`, 2 100 000 000 + key (JOIN of a match) with one `91` row saying what came of it, any other start from 2 000 000 000 on with nothing (lobby.md §2.16.1) | `dispatch.rs` `ranking`, `reserved_ranking`; `join.rs` | high |
| `3C` | automatic, after quest notices, `83 4B`, and the results screen | 0x26FE20 (0x22633D, 0x274EED); inline 0x2719A7 | **real**: quests refreshed, `92` | `dispatch.rs` | high |
| `3D` | clicking a player on the results screen | 0x26FF00 / 0x26FF0E | **real**: the report is kept while a screen of it may be open (2026-10-12) | `dispatch.rs` | high |
| `3E` | automatic, after quest notices and `83 4B` | 0x26FE20 (0x226205, 0x22627B, 0x274EF6) | **real**: `94` | `dispatch.rs` | high |
| `40` | automatic, after quest or reputation notices | 0x26FE20 (0x226243, 0x2265F6) | **real**: `96` | `dispatch.rs` | high |
| `41` | a `#mstats [N]` chat notice | 0x26FF90 / 0x26FF9D | **real** while the report is kept; else no answer. srp never sends the notice, so the client does not ask. | `dispatch.rs` | high |
| `42` | a drop chat notice | 0x270000 / 0x27000D | **being changed today** | `dispatch.rs` | — |
| `43` | a drop chat notice of kind 1 | 0x26FE20 (0x2259BE) | **being changed today** (`99` is sent) | `shop.rs` `daily_bonus` | — |
| `45` | automatic, after `9D` | 0x2768A0 / 0x2768B9 | **fixed answer**: `9E` hide, empty address | `dispatch.rs` | high |

### 6.2 Inventory and items (`46`)

| sub | UI action | client writer at | handling | where | conf. |
|---|---|---|---|---|---|
| `00` | moving items; the ammo window (`relocate_item`, `apply_profile_items`) | 0x2706F0 / 0x27070F | **real**: the move is applied or refused with a text; inventory, account and money pushed | `dispatch.rs`; `rules::move_items` | medium |
| `01` | putting a cosmetic on an item | 0x2707C0 / 0x2707DA | **real**: `83 46 01` or a refusal, then the pushes | `items.rs` `attach_upgrade` | medium |
| `02` | taking a cosmetic off one item (the host is patch 30's) | 0x2708C0 / 0x2708CE | **real**: `83 46 02` | `items.rs` `detach_upgrade` | medium |
| `03` | using a drop card | 0x270AE0 / 0x270AEE | **real**: `98`, then the pushes; one card a finisher of a kept match, else the game's refusal (`drop_cards.md`). The order `<match> <card>` is PROVEN (`results.md` §3); the other is still tried | `dispatch.rs` `use_drop_card`; `rewards::Results::use_drop_card` | medium |
| `04` | using an item (premium voucher) | 0x274400 / 0x27440D | **real**: `83 46 04`, services, pushes | `shop.rs` `use_item` | medium |
| `05` | the decal editor | 0x270950 / 0x270960 | **real**: stored; no answer, as the client expects none | `items.rs` `place_upgrade` | medium |
| `06` | opening the modification window | 0x270DC0 / 0x270DCD | **real**: `83 46 06` | `items.rs` `open_reroll` | medium |
| `07` | changing a modification | 0x270E30 / 0x270E41 | **real**: paid, three choices drawn, `83 46 07` | `items.rs` `reroll_modifier` | medium |
| `08` | picking one of the three | 0x270EE0 / 0x270EED | **real**: `83 46 08` | `items.rs` `choose_modifier` | medium |
| `09` | upgrading an item with a donor | 0x270F90 / 0x270FA1 | **real**: `83 46 09`; a failure is `83 46 09 FE` | `items.rs` `craft` | medium |
| `0A` | the tutorial's "choose a weapon" step | 0x2A45D0 / 0x2A467C | **accepted, not answered** (no tutorial, `no-tutorial.md`; it was `89`, `87`, `8B`, `premium.md` §5.1) | `dispatch.rs` (logged) | high (that an `89` is awaited), LIKELY (the grant) |

### 6.3 Shop (`47`)

| sub | UI action | client writer at | handling | where | conf. |
|---|---|---|---|---|---|
| `00` | buying an item | 0x270BE0 / 0x270BFC | **being changed today** (sellers, ranks) | `dispatch.rs`; `shop.rs` `buy_premium_item` | — |
| `01` | selling an item | 0x272F70 / 0x272F85 | **real**: `83 47 01`, pushes | `dispatch.rs`; `rules::sell` | medium |
| `02` | buying premium or a bundle; from the results screen too | 0x271040 / 0x27105F | **being changed today** (prices) | `shop.rs` `buy_offer` | — |
| `03` | repairing an item | 0x270D00 / 0x270D18 | **real**: silver, gold or a kit; `83 47 03`; a timed repair ends with a notice | `items.rs` `repair`, `item_timers` | medium |
| `04` | gold for silver; silver for gold from srp's window | 0x274390 / 0x27439D | **real**: `83 47 04`, pushes | `dispatch.rs`; `rules::exchange_premium`, `rules::buy_premium` | medium |
| `05` | accepting the premium offer after a match | 0x276AE0 / 0x276AEE | **being changed today** (prices). The offer exists only when `SRP_PREMIUM_DISCOUNT` is set. | `shop.rs` `accept_premium_with_bonuses` | — |
| `06` | accepting the sign-in discount | 0x276BF0 / 0x276BFE | **being changed today** (prices) | `shop.rs` `accept_premium_with_discount` | — |

### 6.4 Skills (`48`), quests and tutorial (`4B`)

| id sub | UI action | client writer at | handling | where | conf. |
|---|---|---|---|---|---|
| `48 00` | applying skills | 0x271100 / 0x27111D | **real**: checked, stored, `8C`, account pushed | `dispatch.rs` `skills_answer`; `rules::set_skills` | medium |
| `48 01` | resetting a branch | 0x271270 / 0x271285 | **real**: paid in silver or gold, `8C` | `rules::reset_skills` | medium |
| `4B 00` | choosing the quest faction | 0x273070 / 0x27307D | **real**: stored, `8E`, `83 4B` | `dispatch.rs`; `quests::choose_faction` | high |
| `4B 01` | tutorial and survival-notes steps | 0x270B70 / 0x270B7D | **accepted, not answered** (no tutorial, `no-tutorial.md`) | `dispatch.rs` (logged) | medium |
| `4B 02` | skipping the tutorial | 0x281660 / 0x2816EA | **accepted, not answered** (no tutorial, `no-tutorial.md`) | `dispatch.rs` (logged) | medium |

### 6.5 Kits and the account (`49 00..05`, `24`)

| sub | UI action | client writer at | handling | where | conf. |
|---|---|---|---|---|---|
| `00` | renaming a kit | 0x26FD00 / 0x26FD0F | **real**: stored; a bad name is refused with `st_change_name_error` | `dispatch.rs`; `rules::rename_kit` | medium |
| `01` | the autobuy and auto-repair boxes | 0x273000 / 0x27300D | **real**: both stored, `83 49 01` | `dispatch.rs` | high |
| `02` | choosing a kit | 0x26FC90 / 0x26FC9D | **real**: `83 49 02`, or a refusal | `dispatch.rs`; `rules::set_active_kit` | high |
| `03` | checking a nickname | 0x2730E0 / 0x2730F0 | **real**: srp's own name rules and "taken", in the client's reasons | `shop.rs` `nickname_refusal` | high |
| `04` | changing the nickname | 0x274310 / 0x27431F | **real**: paid with a voucher or gold, `83 49 04`, `8F` | `shop.rs` `change_nickname` | high |
| `05` | commenting on a player | 0x274470 / 0x274480 | **refused on purpose**: `84 49`, `st_error_during_storing_user_comment` | `dispatch.rs` | high |
| `24` | hiding the video link | 0x274510 / 0x27451E | **accepted, no effect** | `dispatch.rs` | high |

### 6.6 Clans (`49 06..23`)

All are **real**: `servers/lobby-server/src/v026g/clans.rs`, `clan_request`, over the stored clans (`crates/session/src/v026g/clans.rs`). A refusal is `84 49` with the game's text. Confidence medium for all (the store's rules were not read line by line). The clan numbers (cost, members) are being changed today; only `0C` depends on them directly.

| sub | UI action | client writer at | handling |
|---|---|---|---|
| `06` | reserving a name and tag | 0x273150 / 0x273168 | `83 49 06`, the registration pushed |
| `07` | registering the clan | 0x2731F0 / 0x273205 | the clan pushed to all members |
| `08` | checking a name | 0x273240 / 0x273250 | `83 49 08` |
| `09` | checking a tag | 0x2732B0 / 0x2732C0 | `83 49 09` |
| `0A` | answering a clan invite | 0x273320 / 0x273354 | joins or declines; pushes; a clan chat line |
| `0B` | automatic, after `83 49 0F` | 0x2733D0 / 0x273405 | `83 49 0B` when the invite went out |
| `0C` | paying into the registration | 0x273580 / 0x2735B5 | **being changed today** |
| `0D` | leaving, disbanding, cancelling | 0x273610 / 0x273640 | by the player's role; pushes |
| `0E` | opening the clan or team window | 0x273680 / 0x2736B1 | `9A` |
| `0F` | inviting a player | 0x2734F0 / 0x273525 | `83 49 0F`; the invite then goes through `4D` |
| `10` | a rank's permission box | 0x273710 / 0x273749 | stored; ranks pushed |
| `11` | rank down; rank up confirmed | 0x2737F0 / 0x273824 | stored; members pushed |
| `12` | rank up (asks first) | 0x2738A0 / 0x2738D1 | `83 49 12` |
| `13` | kicking a member | 0x273930 / 0x273961 | done; pushes; a clan chat line |
| `14` | creating a rank | 0x2739C0 / 0x2739F1 | stored; ranks pushed |
| `15` | removing a rank | 0x273A50 / 0x273A81 | stored; ranks pushed |
| `16` | the daily message | 0x273AE0 / 0x273B74 | stored; pushed |
| `17` | creating the clan team | 0x273C70 / 0x273CA0 | team pushed |
| `18` | asking to join the team | 0x273CE0 / 0x273D10 | team pushed |
| `19` | inviting to the team | 0x273460 / 0x273495 | `83 49 19` |
| `1A` | answering a team invite | 0x273D50 / 0x273D84 | joins or declines |
| `1B` | leaving the team | 0x273E00 / 0x273E36 | team pushed |
| `1C` | disbanding the team | 0x273E70 / 0x273EA6 | team pushed |
| `1D` | lineup or reserve | 0x273EE0 / 0x273F1A | team pushed |
| `1E` | ousting the commander | 0x273F90 / 0x273FC6 | team pushed |
| `1F` | removing a team member | 0x274000 / 0x274035 | team pushed |
| `20` | automatic, a soldier's ready toggle after `83 30` | 0x274120 / 0x274159 | stored; team pushed |
| `21` | automatic, the commander's ready toggle | 0x274090 / 0x2740C5 | the team waits for another clan's team; a match is made when two wait |
| `22` | naming another team commander | 0x2741B0 / 0x2741E5 | team pushed |
| `23` | closing the clan-match window | 0x274240 / 0x274276 | leaves the team |

### 6.7 Squads (`4A`)

All **real**: `social.rs`, `squad_action`, over the store's squads. The answer is the squad info `A1`. Confidence high.

| sub | UI action | client writer at |
|---|---|---|
| `01` | disbanding | 0x274580 / 0x274595 |
| `02` | automatic, when an invited player accepted | 0x2745D0 / 0x274605 |
| `03` | removing a member | 0x274660 / 0x274674 |
| `04` | leaving | 0x2746D0 / 0x2746E5 |

---

## 7. Chat and friends (`4C..50`, on the lobby link)

Server file: `servers/lobby-server/src/v026g/social.rs` (and `clans.rs` for clan channels and invites). Confidence high.

| id | UI action | client writer at | handling | where |
|---|---|---|---|---|
| `4C` | a chat line | 0x26AD30 / 0x26AEBD | **real**: passed to the players the channel is for (general, private, match, team, squad, clan, clan team); ignore lists kept; a flood limit. Edges: section 2.1. | `chat` |
| `4D` type 0 | adding a friend | 0x26AFB0 / 0x26AFFC | **real**: the request waits for the answer, also across the target's sign-in | `important`, `request` |
| `4D` type 2 | inviting to the squad | the same | **real**: for a player in the lobby | `important` |
| `4D` type 3 | automatic, "membership accepted" | the same | **real**: passed on only for an accepted invite | `important` |
| `4D` long, type 7 | automatic, after `83 49 0F` | 0x26B0A0 / 0x26B0EC | **real**: only an invite the server permitted | `clans.rs` `clan_important` |
| `4D` long, type 10 | automatic, after `83 49 19` | the same | **real**: the same for the clan team | the same |
| `4E 02` | removing a friend | 0x26B470 / 0x26B4A3 | **real**: both lists; `A3 07` to both | `friendship` |
| `4E 03` | ignoring a player | 0x26B510 / 0x26B55F | **real**: `A3 08` | `friendship` |
| `4E 04` | ignoring by name | 0x26B3E0 / 0x26B413 | **real**: `A3 08`; a name that is no account also gets "Player not found." (2026-10-12) | `friendship` |
| `4E 05` | no longer ignoring | 0x26B5C0 / 0x26B5F3 | **real**: `A3 08` | `friendship` |
| `4E 06` | searching players | 0x26B660 / 0x26B693 | **real**: up to 50 names, `A3 06` | `friendship` |
| `4F` | automatic, at sign-in and when a channel id changes | 0x26C320 / 0x26C333 | **real**: the ids are stored; the language byte is not used | `dispatch.rs` |
| `50` | answering a request; automatic for notices | 0x26B1B0 / 0x26B2AC | **real** for requests (friend, squad); a notice's answer is ignored | `important_read` |

---

## 8. Match (UDP 1236)

Server files: `crates/survarium/src/v026g/match_msgs.rs` (`parse_client`), `servers/match-server/src/v026g/server.rs` (`datagram`, `connect`), `game.rs` (`message`). The server relays; the clients simulate. A message that cannot be read is logged at debug level and ignored. An address that sends `D8` for a session with no seat is disconnected. Confidence high for the dispatch, medium for the relay rules.

| id | what sends it | client writer at | handling | where |
|---|---|---|---|---|
| `D8` connect | the client, after `82` | 0x27A1B0 / 0x27A1DD | **real**: seated by its session; `E3` | `server.rs` `connect` |
| `D9` port probe | automatic, after `E3` | 0x3F2B50 / 0x3F2C83 | **real**: `E4` static info | `game.rs` `message` |
| `DA` ready | the level is loaded | 0x27AEAE | **real**: starts the sync (`E5`, `E8`) | `message`, `start_sync` |
| `DB` time reply | automatic, after `E5` and `E6` | 0x27B5E7, 0x27CE7A | **real**: lag measured, clock corrected | `time_reply` |
| `DC` input | the player's input, on the 50 ms grid | 0x27EA90 / 0x27EB24 | **real**: relayed (`EB`), or discarded / moved when late | `input` |
| `DD` input, precise time | the same, off the grid | the same (id = `DC` + flag) | **real**: relayed (`EC`) | `input` |
| `DE` profile input | choosing a kit for the next respawn (LIKELY) | 0x27EB90 / 0x27ECAF | **real**: relayed (`F3`) | `profile_input` |
| `DF` hash | automatic, the world's checksum | 0x27CBF0 / 0x27CDC9 | **real**: compared between clients; a mismatch starts a resync | `hash` |
| `E0` leave | closing the match | 0x27A790 / 0x27A833 | **real**: the player leaves (`EA` to the others) | `leave` |
| `E1` keep-alive | automatic, every second | 0x27CBF0 / 0x27CE3E | **accepted, no effect** | `message` |

---

## 9. Transport (both UDP links)

Server: `crates/udp-transport` (`transport.rs`, `wire.rs`, `ack.rs`). Confidence high for what is listed, medium for timing.

| packet | client writer at | handling |
|---|---|---|
| first datagram (no handshake; carries `2E` or `D8`) | `connect` 0x379090 (callers 0x26F886, 0x27A247) | **real**: a link is made only for a valid first message |
| sequence and ack bytes of every datagram | `send_queued_packets` 0x3FBF70 | **real**: resend, order, duplicates |
| `FA` start disconnect | 0x3FBDB0, type 0 (0x3FE17F) | **real**: confirmed with `FB`; the link ends |
| `FB` confirm disconnect | type 1 (0x3FC11A, 0x3FDDDF) | **real** |
| `FC` keep-alive / ack carrier | type 2 (0x3FC1C9, 0x3FCBED) | **real** |
| `FF` multipart parts | `enqueue` 0x3FD060 / 0x3FD0E7 | **real**: put together |

`FD` (instant disconnect): the client has no writer; the server would obey one. `FE`: never sent by either side.

The lobby link timeout (`SRP_LOBBY_LINK_TIMEOUT_SECONDS`, `core.rs` `step`) is **being changed today**.

---

## 10. Web page (HTTP 8088)

`servers/web-server/src/v026g.rs`. The game client sends it nothing; a browser does. It is real: the admin tabs change the stored 0.26g accounts and tell the lobby, which pushes the account to an open game (`core.rs` `notice`). Its silver and gold exchange is gone since 2026-10-16: silver and gold are exchanged in the lobby's exchange window (`47 04`), and `GET /exchange` only says so. The lobby's `go_to_store`, `go_to_knowledge_base` and `video_tutorial_link_clicked` callbacks reach no request writer; where they open a browser was not traced. The lobby movie no longer calls `go_to_store`: every "Get Gold" opens the exchange window (`buy-gold.md`).

---

## 11. UI callbacks that send nothing

From the table at 0x9C56C8, these handlers reach no request writer (one call deep): `choose_faction_message`, `start_decal_edit`, `cancel_decal_edit`, `start_item_upgrade`, `exit_item_upgrade`, `checkbox_tournament_match_clicked`, `start_friend_message`, `close_victory_screen`, `show_match_statistic`, `ammo_count_updated`, `inventory_content_filled`, `set_mouse_cursor`, `sound_play`, `show_settings`, `chat_enter_start`, `chat_enter_cancel`, `chat_tab_close`, `power_off_button_click`, `go_to_store`, `go_to_knowledge_base`, `system_message_ok_clicked`, `subsystem_activated`, `ammo_container_showed`, `show_daily_reward_window`, `survival_tutorial_clicked`, `tutorial_nickname_ok_clicked`, `video_tutorial_link_clicked`, `set_play_button_enabled`, `handle_drag_start`, `handle_drag_end`. `quest_faction_selected` and `show_tutorial_video_link` have no handler registered.

Not re-checked: `show_match_statistic` (0x29EDA0) could reach `41` by a longer path. The scan found `41` only under `play_button_clicked` (through 0x2252D0) and the `#mstats` parser.

These reach their writer through a confirmation or another path, as the notes say: `cancel_clan_registration`, `leave_clan`, `disband_clan`, `kick_member` (through `system_message_clicked`), `add_friend`, `add_friend_name`, `invite_to_squad` (through `4D`), `drop_upgrade_item`, `repair_item`, `remove_friend`, `set_another_clan_team_commander`, `accept_premium_with_discount`. The scan did not see these links (it follows one call and guesses function ends); the writers themselves are all in the tables above.
